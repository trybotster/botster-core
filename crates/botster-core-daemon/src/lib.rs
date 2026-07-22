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
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest, AttachedSession,
    CaptureSnapshotRequest, CaptureSnapshotResult, DaemonHealth, DaemonSession, DaemonStatus,
    DrainNotificationsRequest, DrainNotificationsResult, DrainResult, DrainRoutedEnvelopesRequest,
    DrainRoutedEnvelopesResult, GuardedWriteRequest, GuardedWriteResult, NotificationStatusResult,
    PostNotificationRequest, PostNotificationResult, PublishRoutedEnvelopeRequest,
    PublishRoutedEnvelopeResult, ReadModeFlagsRequest, ReadModeFlagsResult, ReadScreenRequest,
    ReadScreenResult, RoutedEnvelopeDeliveryStateResult, SessionAdoptionReport,
    SessionAdoptionState, SessionLifecycleBaseline, SessionLifecycleChange,
    SessionLifecycleChangeKind, SessionLifecycleChanges, SessionLifecycleCursor,
    SessionLifecycleRecord, SessionLifecycleResyncReason, SessionLifecycleSourceId,
    SpawnSessionRequest,
};
pub use daemon::{CoreDaemon, CoreDaemonConfig, CoreDaemonError};
pub use daemon::{DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES, DEFAULT_LIFECYCLE_JOURNAL_CAPACITY};
pub use guarded_write::{
    GuardedWriteDecision, GuardedWriteDeliveryState, PromptEvidence, ReadinessEvidence,
    SafeWriteIndicator, SnapshotEvidence,
};
pub use registry::{RegistryRecord, RegistrySessionState, SessionRegistry, SessionRegistryError};
