//! Embeddable multiplexer engine modules.
//!
//! Engine code owns reusable state machines and routing behavior. Concrete host
//! policy, auth, persistence, cloud federation, and product UI stay outside
//! `botster-core`.

pub mod multiplexer;
pub mod plugin_worker;
pub mod session_activity;
pub mod session_worker;
pub mod subscription_multiplexer;

pub use multiplexer::{
    MultiplexerEngine, MultiplexerEngineError, MultiplexerEngineObservation,
    MultiplexerEngineOutcome, MultiplexerSpawnOutcome,
};
pub use plugin_worker::{
    PluginHandlerRegistration, PluginWorkerEngine, PluginWorkerEngineConfig,
    PluginWorkerRegistration,
};
pub use session_activity::{apply_session_activity_event, classify_session_activity};
pub use session_worker::{
    SessionWorkerEngine, SessionWorkerOutcome, SessionWorkerRuntime, SessionWorkerRuntimeEvent,
};
pub use subscription_multiplexer::{
    SubscriptionMultiplexer, SubscriptionMultiplexerObservation, SubscriptionMultiplexerOutcome,
};
