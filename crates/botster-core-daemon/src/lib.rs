//! Production core daemon supervisor over policy-free Botster core primitives.
//!
//! The daemon owns durable registry metadata, session supervision/adoption
//! state, readiness-gated writes, and a typed host API. Session workers still
//! own PTYs and terminal/session evidence. Hubs and embedders own auth, product
//! policy, copy, cloud, and UI decisions.

pub mod api;
pub mod daemon;
pub mod guarded_write;
pub mod operation;
pub mod registry;
mod wake_pump;

pub use operation::{
    CaptureId, CaptureOwner, CoreCompletion, CoreOperation, ModeFlagsReadback, PendingLimitKind,
    PendingOperationId, RetainedTerminal, RetentionAccounting, RetentionPolicy, ScreenReadback,
    SnapshotCapture, SnapshotPage, CAPTURE_IDLE_TTL_SECONDS, MAX_OPEN_CAPTURES_PER_CLIENT,
    MAX_PENDING_READBACKS_PER_SESSION, MAX_PENDING_SPAWNS, SNAPSHOT_PAGE_BYTES,
};

pub use api::{
    is_observe_slice_error_message_byte, max_session_lifecycle_record_bytes,
    reserved_observe_slice_error, sanitize_observe_slice_error_message,
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest, AttachedSession,
    CaptureSnapshotRequest, DaemonHealth, DaemonSession, DaemonStatus, DrainNotificationsRequest,
    DrainNotificationsResult, DrainResult, DrainRoutedEnvelopesRequest, DrainRoutedEnvelopesResult,
    GuardedWriteRequest, GuardedWriteResult, LifecycleBaselineBudget, LifecycleBaselineStop,
    NotificationStatusResult, ObserveLifecycleBudget, ObserveLifecycleCursor,
    ObserveLifecyclePassId, ObserveLifecycleSlice, ObserveLifecycleSliceError,
    ObserveLifecycleStop, PostNotificationRequest, PostNotificationResult,
    PublishRoutedEnvelopeRequest, PublishRoutedEnvelopeResult, PumpWokenOutcome,
    ReadModeFlagsRequest, ReadScreenRequest, RoutedEnvelopeDeliveryStateResult,
    SessionAdoptionReport, SessionAdoptionState, SessionLifecycleBaseline,
    SessionLifecycleBaselinePage, SessionLifecycleChange, SessionLifecycleChangeKind,
    SessionLifecycleChanges, SessionLifecycleCursor, SessionLifecycleLookup, SessionLifecyclePage,
    SessionLifecyclePageError, SessionLifecycleRecord, SessionLifecycleResyncReason,
    SessionLifecycleSourceId, SessionRegistryStateLookup, SpawnSessionRequest,
    OBSERVE_LIFECYCLE_SLICE_MAX_ERROR_MESSAGE_BYTES,
};
pub use botster_core::{
    BindTerminalAdapterError, DetachTerminalSubscriptionResult, ResizeAckHold, SessionWakeHandle,
    TerminalCapabilitySet, TerminalCapabilitySetError, TerminalSubscriptionGeneration,
    TerminalSubscriptionRecord, TerminalWakeBatch, TerminalWakeInterrupt, TerminalWakeKind,
    TerminalWakeRoute, TerminalWakeSink, TerminalWakeSource, TerminalWakeWait,
    WakingTerminalAdapter, WAKE_QUEUE_CAPACITY,
};
pub use daemon::{
    CoreDaemon, CoreDaemonConfig, CoreDaemonError, ObserveLifecycleResult,
    ObserveLifecycleSessionError, DEFAULT_RETENTION_POLICY, DEFAULT_WORKER_REPLY_TIMEOUT,
};
pub use daemon::{DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES, DEFAULT_LIFECYCLE_JOURNAL_CAPACITY};
pub use guarded_write::{
    GuardedWriteDecision, GuardedWriteDeliveryState, PromptEvidence, ReadinessEvidence,
    SafeWriteIndicator, SnapshotEvidence,
};
pub use registry::{RegistryRecord, RegistrySessionState, SessionRegistry, SessionRegistryError};
pub use wake_pump::{WakePumpControl, WakePumpError, WakePumpWait};
