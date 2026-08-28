//! Production core daemon supervisor over policy-free Botster core primitives.
//!
//! The daemon owns durable registry metadata, session supervision/adoption
//! state, readiness-gated writes, and a typed host API. Session workers still
//! own PTYs and terminal/session evidence. Hubs and embedders own auth, product
//! policy, copy, cloud, and UI decisions.

pub mod api;
pub mod daemon;
pub mod guarded_write;
pub mod registry;

pub use api::{
    is_observe_slice_error_message_byte, reserved_observe_slice_error,
    sanitize_observe_slice_error_message, AcknowledgeNotificationRequest,
    AcknowledgeRoutedEnvelopeRequest, AttachedSession, CaptureColorAndSnapshotRequest,
    CaptureColorAndSnapshotResult, CaptureSnapshotRequest, CaptureSnapshotResult, DaemonHealth,
    DaemonSession, DaemonStatus, DrainNotificationsRequest, DrainNotificationsResult, DrainResult,
    DrainRoutedEnvelopesRequest, DrainRoutedEnvelopesResult, GuardedWriteRequest,
    GuardedWriteResult, LifecycleBaselineBudget, NotificationStatusResult, ObserveLifecycleBudget,
    ObserveLifecycleCursor, ObserveLifecyclePassId, ObserveLifecycleSlice,
    ObserveLifecycleSliceError, PostNotificationRequest, PostNotificationResult,
    PublishRoutedEnvelopeRequest, PublishRoutedEnvelopeResult, PumpWokenOutcome,
    ReadModeFlagsRequest, ReadModeFlagsResult, ReadScreenRequest, ReadScreenResult,
    RoutedEnvelopeDeliveryStateResult, SessionAdoptionReport, SessionAdoptionState,
    SessionLifecycleBaseline, SessionLifecycleBaselinePage, SessionLifecycleChange,
    SessionLifecycleChangeKind, SessionLifecycleChanges, SessionLifecycleCursor,
    SessionLifecycleLookup, SessionLifecyclePage, SessionLifecyclePageError,
    SessionLifecycleRecord, SessionLifecycleResyncReason, SessionLifecycleSourceId,
    SessionRegistryStateLookup, SpawnSessionRequest,
    OBSERVE_LIFECYCLE_SLICE_MAX_ERROR_MESSAGE_BYTES,
};
pub use botster_core::{
    BindTerminalAdapterError, DetachTerminalSubscriptionResult, TerminalCapabilitySet,
    TerminalCapabilitySetError, TerminalSubscriptionGeneration, TerminalSubscriptionRecord,
    TerminalWakeBatch, TerminalWakeKind, TerminalWakeRoute, TerminalWakeSink, TerminalWakeSource,
    WakingTerminalAdapter, WAKE_QUEUE_CAPACITY,
};
pub use daemon::{
    CoreDaemon, CoreDaemonConfig, CoreDaemonError, ModeGatedInputOutcome, ObserveLifecycleResult,
    ObserveLifecycleSessionError,
};
pub use daemon::{DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES, DEFAULT_LIFECYCLE_JOURNAL_CAPACITY};
pub use guarded_write::{
    GuardedWriteDecision, GuardedWriteDeliveryState, PromptEvidence, ReadinessEvidence,
    SafeWriteIndicator, SnapshotEvidence,
};
pub use registry::{RegistryRecord, RegistrySessionState, SessionRegistry, SessionRegistryError};
