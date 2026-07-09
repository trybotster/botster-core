//! Start-here surface for library embeds: spawn → attach → drain → input → shutdown.
//!
//! Prefer this module (or the matching paths under [`crate::engine`],
//! [`crate::runtime`], and [`crate::contract`]) over scanning every crate-root
//! re-export. Advanced power remains available through those modules; it is
//! intentionally not the default discovery path.
//!
//! # Choose a host path
//!
//! | Path | When | Entry |
//! | --- | --- | --- |
//! | **Library** | In-process embeds, tests, short-lived hosts | `DefaultBotsterEngine` (default features) or [`BotsterEngine`] with a custom runtime |
//! | **Production** | Durable local supervision | sibling crate `botster_core_daemon::CoreDaemon` + `botster-session-worker` |
//!
//! Production hosts still use core types from this prelude for ids, spawn
//! requests, and (when needed) durable-session protocol vocabulary under
//! [`crate::contract::durable_session`]. The daemon owns registry, adoption,
//! and the spawn/attach/drain/input/shutdown loop on the production path.
//!
//! # Feature gating
//!
//! - Default features enable `local-runtime` and export
//!   `DefaultBotsterEngine` / `DefaultEngineCommand`.
//! - With `default-features = false`, those local-runtime items are absent.
//!   Contract-only embeds keep [`BotsterEngine`], ids, spawn request types, and
//!   command vocabulary from this prelude without pulling in `portable-pty`.
//!
//! # Host event loop
//!
//! Core is synchronous. The host supplies clocks (`now_seconds`), drains
//! regularly (`drain_runtime_once` / `drain_runtime_all_once`), delivers client
//! egress, and applies backpressure reporting when the transport is slow. See
//! the repository README host event-loop section and
//! `docs/architecture/engine-command-surface.md`.
//!
//! # Always-available lifecycle types
//!
//! These compile with default features and with `default-features = false`:
//!
//! ```
//! use botster_core::prelude::*;
//!
//! let session_id = SessionId("session-1".into());
//! let client_id = ClientId("client-1".into());
//! let subscription_id = SubscriptionId("sub-1".into());
//! let request_id = RequestId("spawn-1".into());
//! let metadata = CoreSessionMetadata::new();
//! let request = SessionSpawnRequest {
//!     request_id,
//!     session_id,
//!     executable: "printf".into(),
//!     arguments: vec!["hello".into()],
//!     working_directory: SpawnWorkingDirectory {
//!         path: "/workspace".into(),
//!     },
//!     environment: SpawnEnvironment::default(),
//!     initial_pty_size: None,
//! };
//! let _ = (client_id, subscription_id, metadata, request);
//! let _facade = std::any::type_name::<BotsterEngine<(), ()>>();
//! let _command = std::any::type_name::<EngineCommand<()>>();
//! ```
//!
//! # Library lifecycle (default features / `local-runtime`)
//!
//! With default features, complete spawn → attach → drain → input → shutdown
//! through `DefaultBotsterEngine` (or `DefaultBotsterEngine::worker_backed`
//! when sessions should outlive the parent process):
//!
//! ```no_run
//! # #[cfg(feature = "local-runtime")]
//! # {
//! use botster_core::prelude::*;
//!
//! fn embed() -> Result<(), DefaultBotsterEngineError> {
//!     let mut engine = DefaultBotsterEngine::new();
//!     // Or: DefaultBotsterEngine::worker_backed("/path/to/botster-session-worker");
//!
//!     let session_id = SessionId("session-1".into());
//!     let client_id = ClientId("client-1".into());
//!     let subscription_id = SubscriptionId("sub-1".into());
//!     let now = 1_700_000_000;
//!
//!     let _spawn = engine.spawn_session(
//!         SessionSpawnRequest {
//!             request_id: RequestId("spawn-1".into()),
//!             session_id: session_id.clone(),
//!             executable: "printf".into(),
//!             arguments: vec!["hello".into()],
//!             working_directory: SpawnWorkingDirectory {
//!                 path: "/workspace".into(),
//!             },
//!             environment: SpawnEnvironment::default(),
//!             initial_pty_size: None,
//!         },
//!         CoreSessionMetadata::new(),
//!     )?;
//!
//!     let _attach = engine.attach_client(
//!         client_id.clone(),
//!         session_id.clone(),
//!         subscription_id,
//!         now,
//!     )?;
//!
//!     // Host loop: drain often enough that queues do not stall.
//!     let _drained = engine.drain_runtime_once(&session_id, now)?;
//!     // deliver drained.client_egress (and other outcomes) to clients
//!
//!     let _input = engine.write_bytes(client_id, session_id.clone(), b"next\n", now)?;
//!     let _more = engine.drain_runtime_once(&session_id, now)?;
//!
//!     let _shutdown = engine.shutdown_session(session_id, "done", now)?;
//!     Ok(())
//! }
//! # let _ = embed;
//! # }
//! ```
//!
//! Typed command dispatch (same facade) uses `DefaultEngineCommand` with
//! `DefaultBotsterEngine::execute_command`, or `EngineCommand` with
//! `BotsterEngine::execute_command` for custom runtimes.
//!
//! # Production lifecycle (sibling crate)
//!
//! ```text
//! use botster_core_daemon::{CoreDaemon, CoreDaemonConfig};
//! // CoreDaemon::spawn / attach / drain / input / shutdown
//! // Prefer CoreDaemonConfig::with_worker_path(session_worker_exe)
//! ```
//!
//! # What not to start with
//!
//! - `MultiplexerEngine` — lower-level assembly under the facades
//! - `session_protocol` — raw process wire frames
//! - capability runtime modules — plugin side-channel I/O, not session lifecycle
//! - flat crate-root re-exports of every contract type — still available for
//!   compatibility; prefer this prelude or `contract::*` / `engine` / `package`
//!   / `runtime` modules for new code
//!
//! # Migration
//!
//! No root re-exports were removed in the prelude introduction. Existing
//! `use botster_core::{...}` imports keep compiling. New embedders should
//! import from `botster_core::prelude` (or the module paths above). If a future
//! release narrows crate-root flat re-exports, migrate by:
//!
//! 1. `use botster_core::prelude::*;` for the lifecycle types listed here
//! 2. `botster_core::contract::<module>` for wire/UI/entity/session contracts
//! 3. `botster_core::engine` for facade and advanced engine modules
//! 4. `botster_core::runtime` for spawn requests and runtime traits
//! 5. `botster_core::package` / `botster_core::identity` for package and crypto

// --- Session / client identifiers and metadata ---
pub use crate::client::ClientId;
pub use crate::session::{
    CoreSession, CoreSessionMetadata, RequestId, SessionActivityStatus, SessionId, SubscriptionId,
};

// --- Spawn request shapes (host-resolved, policy-free) ---
pub use crate::runtime::{
    SessionSpawnRequest, SpawnEnvironment, SpawnEnvironmentVariable, SpawnWorkingDirectory,
};

// --- Canonical engine facade and typed command surface ---
pub use crate::engine::{
    BotsterEngine, BotsterEngineError, BotsterEngineObservation, BotsterEngineOutput,
    BotsterSpawnOutcome, EngineClientId, EngineCommand, EngineCommandError, EngineCommandEvent,
    EngineCommandKind, EngineCommandOutcome, EngineCommandResult, EngineNotificationId,
    EngineNotificationItem, EngineNotificationTarget, EngineReplaySnapshotRequest, EngineRequestId,
    EngineSessionId, EngineSessionInspection, EngineSessionIoRequest, EngineSpawnSessionMetadata,
    EngineSpawnSessionRequest, EngineSpawnSessionResult, EngineSubscriptionId,
    ENGINE_COMMAND_KINDS,
};

#[cfg(feature = "local-runtime")]
pub use crate::engine::{
    DefaultBotsterEngine, DefaultBotsterEngineError, DefaultEngineCommand,
    WorkerBackedBotsterEngine, WorkerBackedBotsterEngineError,
};

// --- Local runtime adapters (default-feature local path) ---
#[cfg(feature = "local-runtime")]
pub use crate::runtime::{
    LocalProcessRuntime, LocalProcessRuntimeOptions, LocalProcessWorkerRuntime, WorkerHealth,
    WorkerProcessRuntime, WorkerProcessRuntimeOptions,
};

// --- Runtime traits for custom embeds (always available) ---
pub use crate::runtime::{
    ProcessIdentity, SessionRuntime, SessionRuntimeError, SessionRuntimeErrorKind,
    SessionRuntimeHandle, SessionRuntimeInput, SessionRuntimeOutput,
};

// --- Outcome-adjacent transport frames commonly inspected after drain ---
pub use crate::transport::{TransportEgress, TransportIngress};

// --- Durable-session protocol vocabulary pointers (daemon/worker plane) ---
//
// Prefer `botster_core_daemon::CoreDaemon` for production supervision. These
// types are the shared vocabulary spoken on that path, not a second engine.
pub use crate::durable_session::{
    DaemonControlOperation, GuardedSessionWriteRequest, SessionWorkerAttachRequest,
    SessionWorkerShutdownRequest, SessionWorkerSpawnRequest, DURABLE_SESSION_PROTOCOL_VERSION,
};
