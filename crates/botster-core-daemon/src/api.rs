//! Typed daemon API contracts.

use botster_core::{
    BackpressureSummary, BotsterEngineObservation, ClientId, CoreSessionMetadata, EnvelopeCursor,
    EnvelopeDeliveryState, EnvelopeId, EnvelopeTarget, ModeFlagsReady, NotificationDeliveryStatus,
    NotificationId, NotificationItem, NotificationTarget, NotificationTimestamp, ProcessIdentity,
    RequestId, ResizePayload, RoutedEnvelope, RoutedEnvelopeDrainOutcome,
    RoutedEnvelopePublishOutcome, ScreenReady, SessionId, SessionLifecycleState,
    SessionSpawnRequest, SessionWorkerHealthReason, SessionWorkerStaleReason, SnapshotReady,
    SubscriptionId, TerminalColorProfile, TerminalSnapshotPayload, TransportEgress,
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

/// Opaque identity for one in-memory daemon lifecycle source generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionLifecycleSourceId(pub String);

/// Position in one daemon lifecycle source generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLifecycleCursor {
    /// Source generation that issued this cursor.
    pub source_id: SessionLifecycleSourceId,
    /// Monotonic sequence observed within the source generation.
    pub sequence: u64,
}

/// Authoritative session row exposed to lifecycle-source consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLifecycleRecord {
    /// Durable daemon registry facts.
    pub session: DaemonSession,
    /// Opaque host-owned metadata persisted with the registry row.
    #[serde(default)]
    pub metadata: CoreSessionMetadata,
    /// In-memory core lifecycle state when this daemon owns or adopted the session.
    ///
    /// A fresh daemon may have registry facts before it has adopted a live worker,
    /// so this field is absent until core has verified that runtime generation.
    pub lifecycle: Option<SessionLifecycleState>,
}

/// Deterministic point-in-time lifecycle projection and its journal watermark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLifecycleBaseline {
    /// Cursor immediately after all state represented by this baseline.
    pub cursor: SessionLifecycleCursor,
    /// Sessions ordered by stable [`SessionId`].
    pub sessions: Vec<SessionLifecycleRecord>,
}

/// One material lifecycle projection mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SessionLifecycleChangeKind {
    /// Create or replace the authoritative row for one session.
    Upsert {
        /// Current authoritative session row.
        record: SessionLifecycleRecord,
    },
    /// Forget one already-terminal session at explicit host request.
    Removed {
        /// Stable id of the removed session.
        session_id: SessionId,
    },
}

/// Ordered lifecycle projection mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLifecycleChange {
    /// Cursor identifying this exact change.
    pub cursor: SessionLifecycleCursor,
    /// Material mutation at this cursor.
    pub kind: SessionLifecycleChangeKind,
}

/// Why a lifecycle consumer must discard its cursor and fetch a new baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SessionLifecycleResyncReason {
    /// The cursor belongs to a different daemon source generation.
    SourceChanged,
    /// Required changes were evicted from the bounded journal.
    CursorExpired {
        /// Oldest sequence still retained by this source generation.
        oldest_available_sequence: u64,
    },
    /// The cursor claims a sequence this source has not emitted.
    CursorAhead,
}

/// Changes after a cursor, or an explicit instruction to fetch a fresh baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLifecycleChanges {
    /// Current source watermark.
    pub cursor: SessionLifecycleCursor,
    /// Strictly ordered changes after the requested cursor.
    ///
    /// This is empty when [`Self::resync_required`] is present; callers never
    /// receive a silently truncated suffix.
    pub changes: Vec<SessionLifecycleChange>,
    /// Explicit loss or generation mismatch, when a fresh baseline is required.
    pub resync_required: Option<SessionLifecycleResyncReason>,
}

/// Bounded lifecycle page after a cursor.
///
/// Successful pages have [`Self::resync_required`] unset. Their complete
/// `serde_json` encoding is at most the caller-supplied `max_bytes`.
/// Resync outcomes are control results, not successful pages, and are not
/// required to satisfy the byte budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLifecyclePage {
    /// Strictly ordered changes after the requested cursor.
    ///
    /// Empty when [`Self::resync_required`] is present or when no change
    /// fits the remaining item and encoded-page budget.
    pub changes: Vec<SessionLifecycleChange>,
    /// Cursor immediately after the last included change, or the requested
    /// cursor when the page is empty.
    pub next: SessionLifecycleCursor,
    /// Current source watermark.
    pub source_watermark: SessionLifecycleCursor,
    /// Explicit loss or generation mismatch, when a fresh baseline is required.
    pub resync_required: Option<SessionLifecycleResyncReason>,
}

/// Why a successful lifecycle page cannot be encoded inside `max_bytes`.
///
/// First publication is `#[non_exhaustive]`. Downstream matches must handle
/// [`Self::BudgetTooSmall`] and include a wildcard for later variants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SessionLifecyclePageError {
    /// `max_bytes` is smaller than the encoded empty successful page.
    #[error("lifecycle page budget too small; need at least {minimum_bytes} bytes")]
    BudgetTooSmall {
        /// Exact encoded size of the empty successful page for this metadata.
        minimum_bytes: usize,
    },
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
    /// Initial egress owned by this exact client, session, and subscription route.
    ///
    /// Hosts must deliver these frames as the attach response. A later daemon
    /// drain does not repeat them.
    pub client_egress: Vec<(ClientId, TransportEgress)>,
}

/// Output drained through the daemon.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrainResult {
    /// Egress frames routed to clients.
    ///
    /// This list contains live output and attach output for other routes.
    /// [`AttachedSession::client_egress`] owns initial output for the requested
    /// attach route.
    pub client_egress: Vec<(ClientId, TransportEgress)>,
    /// Core observations from the drain.
    pub observations: Vec<BotsterEngineObservation>,
    /// Backpressure summaries observed while draining.
    pub backpressure: Vec<BackpressureSummary>,
}

/// Result of reading the current daemon-owned terminal screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadScreenResult {
    /// Correlated screen response from the core session contract.
    pub screen: ScreenReady,
}

/// Result of reading authoritative terminal mode flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadModeFlagsResult {
    /// Correlated mode response from the core session contract.
    pub mode_flags: ModeFlagsReady,
}

/// Result of capturing the current daemon-owned terminal snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSnapshotResult {
    /// Correlated snapshot response from the core session contract.
    pub snapshot: SnapshotReady,
    /// Backend-neutral reusable payload, including the runtime-owned format label.
    pub payload: TerminalSnapshotPayload,
}

/// Result of an atomic color-profile + GHOSTSNP capture.
///
/// Both values come from one terminal ownership critical section so attach and
/// reconnect consumers cannot observe colors that disagree with the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureColorAndSnapshotResult {
    /// Ghostty-owned current palette and special colors (reserved indexes).
    pub color_profile: TerminalColorProfile,
    /// Correlated snapshot response from the core session contract.
    pub snapshot: SnapshotReady,
    /// Backend-neutral reusable payload, including the runtime-owned format label.
    pub payload: TerminalSnapshotPayload,
}

/// Host request to read the current terminal screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadScreenRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session to read.
    pub session_id: SessionId,
    /// Logical timestamp used for the internal drain-before-read step.
    pub now_seconds: u64,
}

/// Host request to read authoritative terminal mode flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadModeFlagsRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session to read.
    pub session_id: SessionId,
    /// Logical timestamp used for the internal drain-before-read step.
    pub now_seconds: u64,
}

/// Host request to capture the current terminal snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSnapshotRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session to snapshot.
    pub session_id: SessionId,
    /// Logical timestamp used for the internal drain-before-read step.
    pub now_seconds: u64,
}

/// Host request to capture current colors and GHOSTSNP atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureColorAndSnapshotRequest {
    /// Request correlation id.
    pub request_id: RequestId,
    /// Session to read.
    pub session_id: SessionId,
    /// Logical timestamp used for the internal drain-before-read step.
    pub now_seconds: u64,
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
