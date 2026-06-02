//! Embeddable multiplexer engine modules.
//!
//! Engine code owns reusable state machines and routing behavior. Concrete host
//! policy, auth, persistence, cloud federation, and product UI stay outside
//! `botster-core`.

pub mod botster;
pub mod command;
pub mod managed_session_runtime;
pub mod multiplexer;
pub mod plugin_worker;
pub mod session_activity;
pub mod session_worker;
pub mod subscription_multiplexer;
pub mod terminal_screen;

pub use botster::{
    BotsterEngine, BotsterEngineError, BotsterEngineObservation, BotsterEngineOutput,
    BotsterSpawnOutcome,
};
#[cfg(feature = "local-runtime")]
pub use botster::{DefaultBotsterEngine, DefaultBotsterEngineError};
#[cfg(feature = "local-runtime")]
pub use command::DefaultEngineCommand;
pub use command::{
    EngineClientId, EngineCommand, EngineCommandError, EngineCommandEvent, EngineCommandKind,
    EngineCommandOutcome, EngineCommandResult, EngineNotificationId, EngineNotificationItem,
    EngineNotificationTarget, EngineReplaySnapshotRequest, EngineRequestId, EngineSessionId,
    EngineSessionInspection, EngineSessionIoRequest, EngineSpawnSessionMetadata,
    EngineSpawnSessionRequest, EngineSpawnSessionResult, EngineSubscriptionId,
    ENGINE_COMMAND_KINDS,
};
pub use managed_session_runtime::{ManagedSessionRuntime, ManagedSessionRuntimeError};
pub use multiplexer::{
    MultiplexerEngine, MultiplexerEngineError, MultiplexerEngineObservation,
    MultiplexerEngineOutcome, MultiplexerSpawnOutcome,
};
pub use plugin_worker::{
    PluginHandlerRegistration, PluginInvocationOutcome, PluginWorkerEngine,
    PluginWorkerEngineConfig, PluginWorkerRegistration,
};
pub use session_activity::{apply_session_activity_event, classify_session_activity};
pub use session_worker::{
    SessionWorkerEngine, SessionWorkerOutcome, SessionWorkerRuntime, SessionWorkerRuntimeEvent,
};
pub use subscription_multiplexer::{
    SubscriptionMultiplexer, SubscriptionMultiplexerObservation, SubscriptionMultiplexerOutcome,
};
pub use terminal_screen::{TerminalScreenEngine, TerminalScreenOutcome, TerminalScreenRuntime};
