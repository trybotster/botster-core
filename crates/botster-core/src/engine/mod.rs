//! Embeddable multiplexer engine modules.
//!
//! Engine code owns reusable state machines and routing behavior. Concrete host
//! policy, auth, persistence, cloud federation, and product UI stay outside
//! `botster-core`.
//!
//! # Start here
//!
//! Prefer [`crate::prelude`] and the facade types re-exported from this module
//! (`BotsterEngine`, and with default features `DefaultBotsterEngine`). Lower
//! modules such as [`multiplexer`], [`session_worker`], and
//! [`subscription_multiplexer`] are advanced assembly pieces under the facade.

pub mod botster;
pub mod client_worker;
pub mod command;
pub mod managed_session_runtime;
pub mod multiplexer;
pub mod plugin_timer;
pub mod plugin_worker;
pub mod routed_envelope;
pub mod session_activity;
pub mod session_worker;
pub mod subscription_multiplexer;
pub mod terminal_screen;
pub mod terminal_wake_queue;

pub use botster::{
    BotsterEngine, BotsterEngineError, BotsterEngineObservation, BotsterEngineOutput,
    BotsterSpawnOutcome,
};
#[cfg(feature = "local-runtime")]
pub use botster::{
    DefaultBotsterEngine, DefaultBotsterEngineError, WorkerBackedBotsterEngine,
    WorkerBackedBotsterEngineError,
};
pub use client_worker::{ClientWorker, ClientWorkerTeardown, EnqueueInputResultError};
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
pub use plugin_timer::{PluginTimerDrainOutcome, PluginTimerScheduleOutcome, PluginTimerScheduler};
pub use plugin_worker::{
    PluginHandlerRegistration, PluginInvocationOutcome, PluginWorkerDebugSnapshot,
    PluginWorkerEngine, PluginWorkerEngineConfig, PluginWorkerPluginDebugSnapshot,
    PluginWorkerRegistration,
};
pub use routed_envelope::RoutedEnvelopeRouter;
pub use session_activity::{apply_session_activity_event, classify_session_activity};
pub use session_worker::{
    SessionWorkerEngine, SessionWorkerOutcome, SessionWorkerRuntime, SessionWorkerRuntimeEvent,
};
pub use subscription_multiplexer::{
    SubscriptionMultiplexer, SubscriptionMultiplexerObservation, SubscriptionMultiplexerOutcome,
};
pub use terminal_screen::{TerminalScreenEngine, TerminalScreenOutcome, TerminalScreenRuntime};
