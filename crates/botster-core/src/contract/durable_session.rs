//! Durable session-worker protocol and restart contract shapes.
//!
//! These contracts describe the production topology where a local core daemon
//! supervises independent session workers that own PTYs and child processes.
//! The types are intentionally policy-free and sit above the byte-frame
//! [`session_protocol`](crate::session_protocol) layer and the current
//! [`SessionIoRequest`](crate::SessionIoRequest) / [`SessionIoEvent`](crate::SessionIoEvent)
//! data-plane vocabulary.

use serde::{Deserialize, Serialize};

use crate::actor::{BackpressureSummary, DeliveryLag, QueueSource, SessionLifecycleState};
use crate::client::ClientId;
use crate::notification::{NotificationDeliveryStatus, NotificationId};
use crate::session::{CoreSessionMetadata, RequestId, SessionActivity, SessionId, SubscriptionId};
use crate::session_protocol::ModeFlags;
use crate::terminal_screen::{TerminalScreenSize, TerminalSnapshotPayload};

/// Current durable session-worker protocol contract version.
pub const DURABLE_SESSION_PROTOCOL_VERSION: u16 = 1;

/// Stable identifier for an independent session worker process.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionWorkerId(pub String);

/// Version and compatibility metadata exchanged by daemons and workers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableSessionProtocolVersion {
    /// Protocol version spoken by this peer.
    pub current: u16,
    /// Oldest protocol version this peer can accept.
    pub minimum_compatible: u16,
    /// Human-readable implementation label such as `botster-core`.
    pub implementation: String,
    /// Capabilities supported by this peer.
    #[serde(default)]
    pub capabilities: Vec<SessionWorkerCapability>,
}

impl DurableSessionProtocolVersion {
    /// Build the current core protocol metadata.
    #[must_use]
    pub fn current(implementation: impl Into<String>) -> Self {
        Self {
            current: DURABLE_SESSION_PROTOCOL_VERSION,
            minimum_compatible: DURABLE_SESSION_PROTOCOL_VERSION,
            implementation: implementation.into(),
            capabilities: Vec::new(),
        }
    }

    /// Whether this peer can speak to another durable session protocol peer.
    #[must_use]
    pub const fn is_compatible_with(&self, other: &Self) -> bool {
        self.current >= other.minimum_compatible && other.current >= self.minimum_compatible
    }
}

/// Durable worker protocol capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionWorkerCapability {
    /// Worker can return an opaque terminal snapshot.
    SnapshotHandoff,
    /// Worker can return a plain screen view.
    PlainScreen,
    /// Worker can report terminal mode flags.
    ModeFlags,
    /// Worker can be adopted by a restarted daemon.
    DaemonRestartAdoption,
    /// Worker can emit heartbeat and health observations.
    Heartbeat,
    /// Worker can accept guarded session-visible writes.
    GuardedSessionWrites,
}

/// Process identity supplied by a host or runtime without local path metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerProcessIdentity {
    /// Operating-system process id when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Host-owned opaque boot or process-generation label.
    pub process_generation: String,
    /// Deterministic Unix timestamp for worker birth.
    pub born_at: u64,
}

/// Durable identity that binds one worker process to one core session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerIdentity {
    /// Stable worker id.
    pub worker_id: SessionWorkerId,
    /// Stable session id owned by core.
    pub session_id: SessionId,
    /// Process identity for adoption and stale-worker checks.
    pub process: SessionWorkerProcessIdentity,
    /// Durable protocol version metadata.
    pub protocol: DurableSessionProtocolVersion,
    /// Monotonic adoption generation for this session-worker relationship.
    pub adoption_generation: u64,
}

/// Policy-free worker spawn request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerSpawnRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session the worker must own.
    pub session_id: SessionId,
    /// Host-owned spawn reference. Core does not parse executable, cwd, env, or product config.
    pub host_spawn_ref: String,
    /// Initial terminal size when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_size: Option<TerminalScreenSize>,
    /// Small host classification metadata.
    #[serde(default)]
    pub metadata: CoreSessionMetadata,
    /// Protocol metadata expected from the worker.
    pub protocol: DurableSessionProtocolVersion,
}

/// Spawn result that returns durable worker identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerSpawned {
    /// Original request id.
    pub request_id: RequestId,
    /// Durable worker identity.
    pub identity: SessionWorkerIdentity,
}

/// Request to adopt a worker after core daemon restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerAdoptRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Worker identity claimed by the candidate worker.
    pub identity: SessionWorkerIdentity,
    /// Session the daemon expected to recover.
    pub expected_session_id: SessionId,
    /// Protocol metadata spoken by the restarted daemon.
    pub daemon_protocol: DurableSessionProtocolVersion,
    /// Last heartbeat known to the adopting daemon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at: Option<u64>,
}

/// Adoption outcome for a restarted daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionWorkerAdoptionVerdict {
    /// Worker belongs to the expected session and can be adopted.
    Adopted {
        /// Adopted worker identity.
        identity: SessionWorkerIdentity,
    },
    /// Worker is alive but cannot be attached to the expected session.
    Rejected {
        /// Stable rejection reason.
        reason: SessionWorkerStaleReason,
    },
}

/// Attach request from a client or local stream consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerAttachRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Client being attached.
    pub client_id: ClientId,
    /// Session being attached.
    pub session_id: SessionId,
    /// Subscription identity.
    pub subscription_id: SubscriptionId,
    /// Desired terminal size.
    pub size: TerminalScreenSize,
    /// Snapshot behavior for initial handoff.
    pub snapshot_strategy: SnapshotHandoffStrategy,
}

/// Detach summary for a client subscription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerDetached {
    /// Session that was detached.
    pub session_id: SessionId,
    /// Subscription that was detached.
    pub subscription_id: SubscriptionId,
    /// Whether the worker process remains alive after detach.
    pub worker_retained: bool,
}

/// Initial snapshot behavior requested by an attaching client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotHandoffStrategy {
    /// Deliver snapshot before live output when supported.
    SnapshotBeforeLiveOutput,
    /// Attach only to future live output.
    LiveOnly,
    /// Reuse caller-owned cached state and then stream live output.
    CallerCached,
}

/// Worker heartbeat emitted by a live session worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerHeartbeat {
    /// Worker producing the heartbeat.
    pub identity: SessionWorkerIdentity,
    /// Deterministic heartbeat timestamp.
    pub at: u64,
    /// Current lifecycle state.
    pub lifecycle: SessionLifecycleState,
    /// Current session activity.
    pub activity: SessionActivity,
    /// Queue pressure observed by the worker path.
    #[serde(default)]
    pub pressure: Vec<BackpressureSummary>,
}

/// Health state derived from heartbeats and worker observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionWorkerHealth {
    /// Worker is alive and usable.
    Healthy {
        /// Last known heartbeat timestamp.
        last_heartbeat_at: u64,
    },
    /// Worker is alive but one or more queues or handoff paths are degraded.
    Unhealthy {
        /// Last known heartbeat.
        last_heartbeat_at: u64,
        /// Stable reason.
        reason: SessionWorkerHealthReason,
    },
    /// Worker is stale or lost and should not be treated as durable.
    Stale {
        /// Stable stale classification.
        reason: SessionWorkerStaleReason,
    },
}

/// Unhealthy-but-present worker reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionWorkerHealthReason {
    /// Worker missed the caller-owned heartbeat budget.
    MissedHeartbeat,
    /// Output or control queues are backpressured.
    Backpressured,
    /// Snapshot handoff is temporarily unavailable.
    SnapshotUnavailable,
    /// Child process is no longer running.
    ChildExited,
    /// Host-owned reason string.
    Other(String),
}

/// Stale-worker classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionWorkerStaleReason {
    /// Worker identity does not match the expected session.
    IdentityMismatch,
    /// Protocol versions cannot interoperate.
    IncompatibleProtocol,
    /// Worker missed the adoption or heartbeat deadline.
    HeartbeatExpired,
    /// Worker process is gone.
    ProcessMissing,
    /// Worker died before adoption could complete.
    WorkerDied,
    /// Host-owned reason string.
    Other(String),
}

/// Shutdown request sent to a session worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerShutdownRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session being shut down.
    pub session_id: SessionId,
    /// Shutdown style.
    pub mode: SessionWorkerShutdownMode,
    /// Host-owned reason.
    pub reason: String,
}

/// Session-worker shutdown style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionWorkerShutdownMode {
    /// Ask the worker to stop cleanly.
    Graceful,
    /// Host or supervisor may terminate the worker.
    Forced,
}

/// Worker failure report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerFailure {
    /// Worker identity when still known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<SessionWorkerIdentity>,
    /// Session affected by the failure.
    pub session_id: SessionId,
    /// Stable stale classification.
    pub reason: SessionWorkerStaleReason,
    /// Final durability boundary reached by this failure.
    pub durability: RestartSurvival,
}

/// Restart boundary being evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartBoundary {
    /// Product hub or host process restarted while core daemon and worker survive.
    HubRestart,
    /// Core daemon restarted and successfully adopted a live worker.
    CoreDaemonRestartAdopted,
    /// Core daemon restarted but adoption failed.
    CoreDaemonRestartAdoptionFailed,
    /// Session worker process died.
    SessionWorkerDeath,
}

/// Survival contract for one restart boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartSurvival {
    /// Session and worker should remain live.
    Survives,
    /// Session may survive only with degraded state.
    SurvivesDegraded,
    /// Session is not durable past this boundary.
    Dies,
}

/// Restart matrix for the durable session model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurableRestartSemantics {
    /// Survival for hub restart.
    pub hub_restart: RestartSurvival,
    /// Survival for core daemon restart when adoption succeeds.
    pub core_daemon_restart_adopted: RestartSurvival,
    /// Survival for core daemon restart when adoption fails.
    pub core_daemon_restart_adoption_failed: RestartSurvival,
    /// Survival when the worker process dies.
    pub session_worker_death: RestartSurvival,
}

impl DurableRestartSemantics {
    /// Durable session-worker north-star survival matrix.
    #[must_use]
    pub const fn durable_worker_contract() -> Self {
        Self {
            hub_restart: RestartSurvival::Survives,
            core_daemon_restart_adopted: RestartSurvival::Survives,
            core_daemon_restart_adoption_failed: RestartSurvival::SurvivesDegraded,
            session_worker_death: RestartSurvival::Dies,
        }
    }

    /// Return survival behavior for one boundary.
    #[must_use]
    pub const fn survival_for(&self, boundary: RestartBoundary) -> RestartSurvival {
        match boundary {
            RestartBoundary::HubRestart => self.hub_restart,
            RestartBoundary::CoreDaemonRestartAdopted => self.core_daemon_restart_adopted,
            RestartBoundary::CoreDaemonRestartAdoptionFailed => {
                self.core_daemon_restart_adoption_failed
            }
            RestartBoundary::SessionWorkerDeath => self.session_worker_death,
        }
    }
}

/// Readiness evidence observed by core and evaluated by host policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionReadinessEvidence {
    /// Session being evaluated.
    pub session_id: SessionId,
    /// Deterministic observation timestamp.
    pub observed_at: u64,
    /// Current terminal mode flags when available.
    ///
    /// Cursor visibility is available through `ModeFlags::cursor_visible`
    /// where a runtime adapter reports mode flags. Runtime adapters that only
    /// expose plain screen text must leave this unset rather than claiming a
    /// cursor observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_flags: Option<ModeFlags>,
    /// Current plain screen text summary when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen_text: Option<String>,
    /// Whether a semantic prompt mark indicates the agent is waiting for an answer.
    pub waiting_for_answer: bool,
    /// Whether terminal/session state suggests a write would interrupt active prompt work.
    pub unsafe_to_interrupt: bool,
    /// Whether an initial or recovery snapshot is still pending.
    pub snapshot_pending: bool,
    /// Current worker health.
    pub worker_health: SessionWorkerHealth,
    /// Activity observations used by deterministic scheduling.
    #[serde(default)]
    pub activity: SessionActivity,
    /// Host semantic hints. Core serializes these but does not interpret policy.
    #[serde(default)]
    pub semantic_hints: Vec<String>,
}

/// Policy verdict supplied by a host for a guarded write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardedSessionWritePolicy {
    /// Write may be routed now.
    Allow,
    /// Core should queue the write before routing.
    Queue,
    /// Core should defer until later readiness evidence is available.
    Defer,
    /// Core should reject the write.
    Reject,
}

/// Session-visible write primitive. Raw PTY input is distinct from annotations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GuardedSessionWritePrimitive {
    /// Raw PTY input bytes.
    PtyInput {
        /// Input bytes to send to the PTY.
        data: Vec<u8>,
    },
    /// Host/plugin-authorized annotation intended to appear in the session stream.
    SessionAnnotation {
        /// Synthetic annotation body.
        body: String,
    },
    /// Notification content intended to become session-visible.
    SessionNotification {
        /// Notification id linked to the host/plugin notification surface.
        notification_id: NotificationId,
        /// Synthetic notification body.
        body: String,
    },
}

/// Guarded write request evaluated against readiness evidence and host policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardedSessionWriteRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Target session.
    pub session_id: SessionId,
    /// Write primitive.
    pub primitive: GuardedSessionWritePrimitive,
    /// Readiness evidence snapshot used for this decision.
    pub readiness: SessionReadinessEvidence,
    /// Host-supplied policy verdict.
    pub host_policy: GuardedSessionWritePolicy,
    /// Optional deadline after which queued/deferred work should be re-evaluated or rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defer_until: Option<u64>,
}

/// Explicit state transition for a guarded session write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GuardedSessionWriteState {
    /// Request was accepted for evaluation but no write has been routed yet.
    Accepted {
        /// Request correlation id.
        request_id: RequestId,
    },
    /// Request is queued in the existing session-I/O pressure path.
    Queued {
        /// Request correlation id.
        request_id: RequestId,
        /// Optional pressure observation for the queue route.
        pressure: Option<BackpressureSummary>,
    },
    /// Request is deferred until readiness or policy changes.
    Deferred {
        /// Request correlation id.
        request_id: RequestId,
        /// Stable deferral reason.
        reason: GuardedSessionWriteDeferralReason,
    },
    /// Request was rejected before writing.
    Rejected {
        /// Request correlation id.
        request_id: RequestId,
        /// Stable rejection reason.
        reason: GuardedSessionWriteRejectionReason,
    },
    /// Bytes or annotation content were injected into the worker path.
    Written {
        /// Request correlation id.
        request_id: RequestId,
        /// Deterministic write timestamp.
        written_at: u64,
    },
    /// Delivery was acknowledged where the implementation can prove it.
    Acknowledged {
        /// Request correlation id.
        request_id: RequestId,
        /// Deterministic acknowledgement timestamp.
        acknowledged_at: u64,
        /// Linked notification status when the write corresponds to a notification.
        notification_status: Option<NotificationDeliveryStatus>,
    },
}

/// Deferral reason for a guarded write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardedSessionWriteDeferralReason {
    /// The agent appears to be waiting for an answer.
    WaitingForAnswer,
    /// Cursor/mode/screen state indicates an unsafe moment.
    UnsafePromptState,
    /// A snapshot or attach handoff is pending.
    SnapshotPending,
    /// The session worker path is backpressured.
    Backpressured,
    /// Host-owned reason string.
    Other(String),
}

/// Rejection reason for a guarded write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardedSessionWriteRejectionReason {
    /// Host policy rejected the write.
    HostPolicyRejected,
    /// Session is not healthy enough for the write primitive.
    WorkerUnhealthy,
    /// Worker or queue route is stale.
    StaleWorker,
    /// Payload cannot be represented by the requested primitive.
    InvalidPayload,
    /// Host-owned reason string.
    Other(String),
}

/// Queue and slow-consumer contract for durable worker output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionWorkerQueueLimits {
    /// Existing queue source used for the durable worker path.
    pub source: QueueSource,
    /// Maximum queued output frames.
    pub output_frame_capacity: usize,
    /// Maximum queued output bytes.
    pub output_byte_capacity: usize,
    /// Pressure observations surfaced through the existing actor vocabulary.
    #[serde(default)]
    pub pressure: Vec<BackpressureSummary>,
    /// Accepted-but-slow observations surfaced through existing lag vocabulary.
    #[serde(default)]
    pub lag: Vec<DeliveryLag>,
    /// Slow-consumer behavior.
    pub slow_consumer: SlowConsumerBehavior,
}

/// Slow-consumer behavior for bounded output paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlowConsumerBehavior {
    /// Preserve order and backpressure or reject when full.
    PreserveOrderAndBackpressure,
    /// Drop only live output after authoritative snapshot handoff is preserved.
    DropLiveOutputAfterSnapshot,
    /// Detach the slow consumer while retaining the worker.
    DetachConsumer,
}

/// Output or snapshot handoff frame emitted by a session worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionWorkerOutputFrame {
    /// Terminal output bytes from the worker.
    TerminalBytes {
        /// Session that emitted bytes.
        session_id: SessionId,
        /// Output bytes.
        data: Vec<u8>,
    },
    /// Snapshot handoff payload.
    Snapshot {
        /// Session that produced the snapshot.
        session_id: SessionId,
        /// Snapshot payload.
        snapshot: TerminalSnapshotPayload,
    },
    /// Plain screen handoff payload when available.
    PlainScreen {
        /// Session that produced the screen.
        session_id: SessionId,
        /// Screen text.
        text: String,
        /// Terminal size.
        size: TerminalScreenSize,
    },
}

/// Thin daemon CLI operation names. The CLI wraps typed daemon control; it is not the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonCliOperation {
    /// Start the local core daemon.
    Start,
    /// Read typed daemon status.
    Status,
    /// List known sessions.
    SessionList,
    /// Attach or stream a session.
    AttachOrStream,
    /// Request daemon shutdown.
    Shutdown,
    /// Read daemon or worker health.
    Health,
}

/// Daemon control operation contract for local IPC/library embedders.
///
/// This is a typed contract over the same core facade concepts documented in
/// `engine::command`; it is not an executable router and must not parse CLI output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonControlOperation {
    /// Start daemon supervision.
    Start {
        /// Request correlation id.
        request_id: RequestId,
    },
    /// Read daemon status.
    Status {
        /// Request correlation id.
        request_id: RequestId,
    },
    /// List sessions through the typed engine facade.
    SessionList {
        /// Request correlation id.
        request_id: RequestId,
    },
    /// Attach or stream one session.
    AttachStream {
        /// Request correlation id.
        request_id: RequestId,
        /// Session to attach or stream.
        session_id: SessionId,
        /// Client receiving the stream.
        client_id: ClientId,
        /// Subscription identity for the stream.
        subscription_id: SubscriptionId,
    },
    /// Shut down daemon supervision.
    Shutdown {
        /// Request correlation id.
        request_id: RequestId,
        /// Host-owned shutdown reason.
        reason: String,
    },
    /// Read daemon and worker health.
    Health {
        /// Request correlation id.
        request_id: RequestId,
    },
}

/// Outcome from a daemon control operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonControlOutcome {
    /// Operation was accepted.
    Accepted {
        /// Request correlation id.
        request_id: RequestId,
    },
    /// Operation was rejected.
    Rejected {
        /// Request correlation id.
        request_id: RequestId,
        /// Stable rejection reason.
        reason: String,
    },
    /// Health information was returned.
    Health {
        /// Request correlation id.
        request_id: RequestId,
        /// Worker health observations included in the response.
        workers: Vec<SessionWorkerHealth>,
    },
}
