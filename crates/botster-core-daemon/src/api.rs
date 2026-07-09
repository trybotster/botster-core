//! Typed daemon API contracts.

use botster_core::{
    BackpressureSummary, BotsterEngineObservation, ClientId, CoreSessionMetadata, EnvelopeCursor,
    EnvelopeDeliveryState, EnvelopeId, EnvelopeTarget, NotificationDeliveryStatus, NotificationId,
    NotificationItem, NotificationTarget, NotificationTimestamp, ProcessIdentity, ResizePayload,
    RoutedEnvelope, RoutedEnvelopeDrainOutcome, RoutedEnvelopePublishOutcome, SessionId,
    SessionSpawnRequest, SessionWorkerHealthReason, SessionWorkerStaleReason, SubscriptionId,
    TransportEgress,
};
use serde::{Deserialize, Serialize};

use crate::guarded_write::{GuardedWriteDecision, GuardedWriteDeliveryState, ReadinessEvidence};
use crate::registry::{RegistryRecord, RegistrySessionState};

/// Host request to spawn a daemon-owned session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSessionRequest {
    /// Policy-resolved core spawn request.
    pub request: SessionSpawnRequest,
    /// Host-provided core session metadata.
    pub metadata: CoreSessionMetadata,
}

/// Session summary visible through daemon list and health APIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonSession {
    /// Session id.
    pub session_id: SessionId,
    /// Durable registry state.
    pub registry_state: RegistrySessionState,
    /// Last known terminal size.
    pub size: ResizePayload,
    /// Process identity when known.
    pub process: Option<ProcessIdentity>,
    /// Last registry update timestamp.
    pub updated_at: u64,
}

/// Result of attaching a client to a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachedSession {
    /// Attached client id.
    pub client_id: ClientId,
    /// Session id.
    pub session_id: SessionId,
    /// Subscription id used for output routing.
    pub subscription_id: SubscriptionId,
}

/// Output drained through the daemon.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrainResult {
    /// Egress frames routed to clients.
    ///
    /// Attach may produce initial subscription history immediately inside the
    /// core engine. The daemon retains that attach output and exposes it here
    /// so embedders can observe history replay through the same drain surface
    /// as live runtime output.
    pub client_egress: Vec<(ClientId, TransportEgress)>,
    /// Core observations from the drain.
    pub observations: Vec<BotsterEngineObservation>,
    /// Backpressure summaries observed while draining.
    pub backpressure: Vec<BackpressureSummary>,
}

/// Host request to queue one generic notification inbox item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostNotificationRequest {
    /// Core notification item to queue.
    pub item: NotificationItem,
}

/// Result of queueing one notification inbox item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostNotificationResult {
    /// Stable notification id returned by the inbox.
    pub id: NotificationId,
}

/// Host request to drain deliverable notifications for one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrainNotificationsRequest {
    /// Target whose inbox should be drained.
    pub target: NotificationTarget,
    /// Deterministic timestamp used for expiry checks.
    pub now: NotificationTimestamp,
}

/// Result of draining notification inbox items.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrainNotificationsResult {
    /// Items delivered to the target.
    pub items: Vec<NotificationItem>,
}

/// Host request to acknowledge one notification inbox item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcknowledgeNotificationRequest {
    /// Notification id to acknowledge.
    pub id: NotificationId,
}

/// Result of acknowledging or querying one notification inbox item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationStatusResult {
    /// Current status when the notification id is known.
    pub status: Option<NotificationDeliveryStatus>,
}

/// Host request to publish one routed envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishRoutedEnvelopeRequest {
    /// Core routed envelope to publish.
    pub envelope: RoutedEnvelope,
}

/// Host request to drain routed envelopes for one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrainRoutedEnvelopesRequest {
    /// Target queue to drain.
    pub target: EnvelopeTarget,
    /// Optional cursor the caller has already observed.
    pub after: Option<EnvelopeCursor>,
    /// Maximum number of envelopes to deliver.
    pub limit: usize,
}

/// Host request to acknowledge one routed envelope for one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcknowledgeRoutedEnvelopeRequest {
    /// Target whose delivered copy should be acknowledged.
    pub target: EnvelopeTarget,
    /// Envelope id to acknowledge.
    pub envelope_id: EnvelopeId,
}

/// Result of acknowledging or querying one routed envelope target copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedEnvelopeDeliveryStateResult {
    /// Current delivery state when the target/envelope pair is known.
    pub state: Option<EnvelopeDeliveryState>,
}

/// Result of publishing one routed envelope.
pub type PublishRoutedEnvelopeResult = RoutedEnvelopePublishOutcome;

/// Result of draining routed envelopes.
pub type DrainRoutedEnvelopesResult = RoutedEnvelopeDrainOutcome;

/// Host request for readiness-gated PTY input or notification text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedWriteRequest {
    /// Session to write into.
    pub session_id: SessionId,
    /// Client identity used for routing the eventual PTY input.
    pub client_id: ClientId,
    /// Bytes to inject when readiness evidence permits it.
    pub data: Vec<u8>,
    /// Host-supplied readiness evidence available to the daemon.
    ///
    /// The daemon validates this evidence fail-closed. It does not treat caller
    /// assertions as downstream delivery proof.
    pub readiness: ReadinessEvidence,
    /// Logical timestamp supplied by the host scheduler.
    pub now_seconds: u64,
}

/// Guarded write result with explicit state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedWriteResult {
    /// Final decision from the readiness gate.
    pub decision: GuardedWriteDecision,
    /// States observed while processing this write.
    pub states: Vec<GuardedWriteDeliveryState>,
}

/// Daemon health surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonHealth {
    /// Whether the daemon accepts new commands.
    pub running: bool,
    /// Number of sessions known to the in-process engine.
    pub live_sessions: usize,
    /// Number of persisted registry records.
    pub registry_records: usize,
    /// Registry data root, scrubbed to the caller-provided path string.
    pub data_dir: String,
}

/// Human/debug status output for the thin CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatus {
    /// Health summary.
    pub health: DaemonHealth,
    /// Durable registry records.
    pub sessions: Vec<DaemonSession>,
}

/// Adoption state for one durable registry record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionAdoptionState {
    /// Record has enough protocol metadata to attempt live worker adoption.
    Adoptable,
    /// Record has restart evidence, but this daemon has no session-worker path.
    InProcessDaemonNotRestartDurable,
    /// Record is terminal and should not be adopted.
    Terminal,
    /// Record is missing the restart-contract liveness/recovery evidence.
    MissingProtocolEvidence,
    /// Record has protocol evidence but no matching live worker in this daemon supervisor.
    StaleWorker {
        /// Stable stale-worker classification.
        reason: SessionWorkerStaleReason,
    },
    /// Worker is present but currently unhealthy.
    UnhealthyWorker {
        /// Stable unhealthy-worker classification.
        reason: SessionWorkerHealthReason,
    },
    /// More than one live worker candidate claims the same session identity.
    DuplicateWorker {
        /// Number of live candidates found for this session.
        candidates: usize,
    },
}

/// Adoption scan result for one record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAdoptionReport {
    /// Durable record.
    pub record: RegistryRecord,
    /// Adoption state.
    pub state: SessionAdoptionState,
}

impl From<&RegistryRecord> for DaemonSession {
    fn from(record: &RegistryRecord) -> Self {
        Self {
            session_id: record.session_id.clone(),
            registry_state: record.state.clone(),
            size: ResizePayload {
                rows: record.rows,
                cols: record.cols,
            },
            process: record.process.clone(),
            updated_at: record.updated_at,
        }
    }
}
