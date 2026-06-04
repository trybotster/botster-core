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
    AttachedSession, DaemonHealth, DaemonSession, DaemonStatus, DrainResult, GuardedWriteRequest,
    GuardedWriteResult, SessionAdoptionReport, SessionAdoptionState, SpawnSessionRequest,
};
pub use daemon::{CoreDaemon, CoreDaemonConfig, CoreDaemonError};
pub use guarded_write::{
    GuardedWriteDecision, GuardedWriteDeliveryState, PromptEvidence, ReadinessEvidence,
    SafeWriteIndicator, SnapshotEvidence,
};
pub use registry::{RegistryRecord, RegistrySessionState, SessionRegistry, SessionRegistryError};
