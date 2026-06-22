//! Ergonomic embeddable Botster engine facade.

use crate::actor::{
    MailboxSendFailureReason, PluginCleanupResult, PluginInvocationRequest, PluginKey,
    PluginReloadSpec, PluginTimerCancellationResult, PluginTimerId, PluginTimerSchedule,
    PluginUnloadSpec, PreparedSnapshotRequest, QueueSource,
};
use crate::contract::notification::{
    NotificationId, NotificationItem, NotificationTarget, NotificationTimestamp,
};
use crate::contract::transport::TransportIngress;
#[cfg(feature = "local-runtime")]
use crate::engine::command::DefaultEngineCommand;
use crate::engine::command::{
    EngineCommand, EngineCommandError, EngineCommandOutcome, EngineSessionInspection,
};
#[cfg(feature = "local-runtime")]
use crate::engine::managed_session_runtime::{ManagedSessionRuntime, ManagedSessionRuntimeError};
use crate::engine::multiplexer::{
    MultiplexerEngine, MultiplexerEngineError, MultiplexerEngineObservation,
    MultiplexerEngineOutcome, MultiplexerSpawnOutcome,
};
use crate::engine::plugin_timer::{
    PluginTimerDrainOutcome, PluginTimerScheduleOutcome, PluginTimerScheduler,
};
use crate::engine::plugin_worker::{
    PluginInvocationOutcome, PluginWorkerEngine, PluginWorkerEngineConfig, PluginWorkerRegistration,
};
use crate::engine::session_worker::{SessionWorkerRuntime, SessionWorkerRuntimeEvent};
#[cfg(feature = "local-runtime")]
use crate::runtime::{LocalProcessRuntime, WorkerProcessRuntime, WorkerProcessRuntimeOptions};
use crate::runtime::{ProcessIdentity, SessionRuntime, SessionSpawnRequest};
use crate::session::{CoreSession, CoreSessionMetadata, SessionActivityStatus, SessionId};
use crate::{ClientId, SessionMetadata, SubscriptionId};

/// Facade-level error for ergonomic Botster engine operations.
pub type BotsterEngineError = MultiplexerEngineError;

/// Observable state change emitted by the ergonomic Botster engine.
pub type BotsterEngineObservation = MultiplexerEngineObservation;

/// Result of a successful session spawn through the ergonomic Botster engine.
pub type BotsterSpawnOutcome = MultiplexerSpawnOutcome;

/// Accumulated output from one ergonomic Botster engine operation.
pub type BotsterEngineOutput = MultiplexerEngineOutcome;

/// Default local PTY-backed engine error.
#[cfg(feature = "local-runtime")]
pub type DefaultBotsterEngineError = ManagedSessionRuntimeError;

/// Worker-backed local PTY engine error.
#[cfg(feature = "local-runtime")]
pub type WorkerBackedBotsterEngineError = ManagedSessionRuntimeError;

/// Public default local PTY-backed Botster engine facade.
///
/// This is the policy-free default path for embedders that want to run a real
/// local process without supplying custom runtime adapters. Hosts still provide
/// explicit spawn requests; the facade only wires the local process runtime
/// through the managed session worker and subscription fanout path.
#[cfg(feature = "local-runtime")]
pub struct DefaultBotsterEngine {
    runtime: ManagedSessionRuntime<LocalProcessRuntime>,
}

/// Public local PTY-backed engine facade whose live PTY is owned by a worker process.
#[cfg(feature = "local-runtime")]
pub struct WorkerBackedBotsterEngine {
    runtime: ManagedSessionRuntime<WorkerProcessRuntime>,
}

#[cfg(feature = "local-runtime")]
impl DefaultBotsterEngine {
    /// Build an empty local PTY-backed engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runtime: ManagedSessionRuntime::new(LocalProcessRuntime::new()),
        }
    }

    /// Build an empty local engine whose sessions are owned by worker processes.
    #[must_use]
    pub fn worker_backed(worker_path: impl Into<std::path::PathBuf>) -> WorkerBackedBotsterEngine {
        WorkerBackedBotsterEngine::new(worker_path)
    }

    /// Return a recorded session.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        self.runtime.session(session_id)
    }

    /// Return sessions currently recorded by the local command facade.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<CoreSession> {
        self.runtime.list_sessions()
    }

    /// Return the local process runtime adapter.
    #[must_use]
    pub const fn session_runtime(&self) -> &LocalProcessRuntime {
        self.runtime.session_runtime()
    }

    /// Spawn a local PTY-backed session with an explicit host-owned request.
    pub fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
    ) -> Result<BotsterSpawnOutcome, DefaultBotsterEngineError> {
        self.runtime.spawn_session(request, metadata)
    }

    /// Execute one typed command through the default local engine facade.
    pub fn execute_command(
        &mut self,
        command: DefaultEngineCommand,
    ) -> Result<EngineCommandOutcome, EngineCommandError<DefaultBotsterEngineError>> {
        let kind = command.kind();
        match command {
            DefaultEngineCommand::SpawnSession { request, metadata } => self
                .spawn_session(request, metadata)
                .map(EngineCommandOutcome::SpawnSession),
            DefaultEngineCommand::AttachClient {
                client_id,
                session_id,
                subscription_id,
                now_seconds,
            } => self
                .attach_client(client_id, session_id, subscription_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::DetachClient {
                client_id,
                session_id,
                subscription_id,
                now_seconds,
            } => self
                .detach_client(client_id, session_id, subscription_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::SendInput {
                client_id,
                session_id,
                data,
                now_seconds,
            } => self
                .write_bytes(client_id, session_id, data, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::Resize {
                client_id,
                session_id,
                rows,
                cols,
                now_seconds,
            } => self
                .resize(client_id, session_id, rows, cols, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::ListSessions => {
                Ok(EngineCommandOutcome::Sessions(self.list_sessions()))
            }
            DefaultEngineCommand::InspectSession {
                session_id,
                now_seconds,
                active_threshold_seconds,
            } => self
                .inspect_session(&session_id, now_seconds, active_threshold_seconds)
                .map(EngineCommandOutcome::Inspection),
            DefaultEngineCommand::ReadScreen {
                request_id,
                session_id,
                now_seconds,
            } => self
                .read_screen(request_id, session_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::CaptureSnapshot {
                request_id,
                session_id,
                now_seconds,
            } => self
                .capture_snapshot(request_id, session_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::ReplaySnapshot {
                request,
                now_seconds,
            } => self
                .replay_snapshot(request, now_seconds)
                .map(EngineCommandOutcome::Output),
            DefaultEngineCommand::Shutdown {
                session_id,
                reason,
                now_seconds,
            } => self
                .shutdown_session(session_id, reason, now_seconds)
                .map(EngineCommandOutcome::Output),
        }
        .map_err(|source| EngineCommandError::new(kind, source))
    }

    /// Attach a client to a session stream.
    pub fn attach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Detach a client from a session stream.
    pub fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::UnsubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Write terminal bytes from a client into the local process runtime.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::TerminalInput {
                session_id,
                data: data.into(),
            },
            now_seconds,
        )
    }

    /// Resize a session terminal from a client-facing path.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::Resize {
                session_id,
                rows,
                cols,
            },
            now_seconds,
        )
    }

    /// Drain currently available local runtime output through subscription fanout.
    pub fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.drain_runtime_once(session_id, last_output_at)
    }

    /// Drain currently available local runtime output once for every live session.
    pub fn drain_runtime_all_once(
        &mut self,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.drain_runtime_all_once(last_output_at)
    }

    /// Report client-side backpressure through the default local engine path.
    pub fn report_backpressure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .report_backpressure(client_id, session_id, source, capacity, depth)
    }

    /// Report accepted-but-slow delivery through the default local engine path.
    pub fn report_delivery_lag(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.report_delivery_lag(
            client_id,
            session_id,
            subscription_id,
            source,
            capacity,
            depth,
        )
    }

    /// Report a failed delivery attempt through the default local engine path.
    pub fn report_delivery_failure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        reason: MailboxSendFailureReason,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .report_delivery_failure(client_id, session_id, subscription_id, source, reason)
    }

    /// Classify one session's activity at the provided clock value.
    pub fn classify_activity(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<SessionActivityStatus, DefaultBotsterEngineError> {
        self.runtime
            .classify_activity(session_id, now_seconds, active_threshold_seconds)
    }

    /// Inspect one session's lifecycle and activity through the command facade.
    pub fn inspect_session(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<EngineSessionInspection, DefaultBotsterEngineError> {
        self.runtime
            .inspect_session(session_id, now_seconds, active_threshold_seconds)
    }

    /// Read a session's plain screen state where the managed runtime supports it.
    pub fn read_screen(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .read_screen(request_id, session_id, now_seconds)
    }

    /// Capture a session snapshot where the managed runtime supports it.
    pub fn capture_snapshot(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .capture_snapshot(request_id, session_id, now_seconds)
    }

    /// Replay or prepare a snapshot where the managed runtime supports it.
    pub fn replay_snapshot(
        &mut self,
        request: PreparedSnapshotRequest,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.replay_snapshot(request, now_seconds)
    }

    /// Shut down one local PTY-backed session through the managed runtime path.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime
            .shutdown_session(session_id, reason, now_seconds)
    }
}

#[cfg(feature = "local-runtime")]
impl WorkerBackedBotsterEngine {
    /// Build an empty worker-backed local PTY engine.
    #[must_use]
    pub fn new(worker_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            runtime: ManagedSessionRuntime::with_worker_process(worker_path),
        }
    }

    /// Build an empty worker-backed local PTY engine with explicit options.
    #[must_use]
    pub fn with_options(options: WorkerProcessRuntimeOptions) -> Self {
        Self {
            runtime: ManagedSessionRuntime::with_worker_process_options(options),
        }
    }

    /// Return a recorded session.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        self.runtime.session(session_id)
    }

    /// Return sessions currently recorded by the worker-backed facade.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<CoreSession> {
        self.runtime.list_sessions()
    }

    /// Return the worker process runtime adapter.
    #[must_use]
    pub const fn session_runtime(&self) -> &WorkerProcessRuntime {
        self.runtime.session_runtime()
    }

    /// Return the worker process runtime adapter mutably.
    pub const fn session_runtime_mut(&mut self) -> &mut WorkerProcessRuntime {
        self.runtime.session_runtime_mut()
    }

    /// Return worker welcome metadata captured after spawning a session.
    #[must_use]
    pub fn worker_metadata(&self, session_id: &SessionId) -> Option<&SessionMetadata> {
        self.runtime.session_runtime().metadata(session_id)
    }

    /// Adopt a live worker process through its reconnectable control endpoint.
    pub fn adopt_worker_process(
        &mut self,
        session_id: SessionId,
        process: ProcessIdentity,
        socket_path: impl Into<std::path::PathBuf>,
        metadata: CoreSessionMetadata,
    ) -> Result<BotsterSpawnOutcome, WorkerBackedBotsterEngineError> {
        self.runtime
            .adopt_worker_process(session_id, process, socket_path, metadata)
    }

    /// Release workers without sending shutdown frames for an intentional daemon restart.
    pub fn release_workers_for_restart(&mut self) {
        self.runtime.release_workers_for_restart();
    }

    /// Spawn a local session whose PTY is owned by a worker process.
    pub fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
    ) -> Result<BotsterSpawnOutcome, WorkerBackedBotsterEngineError> {
        self.runtime.spawn_session(request, metadata)
    }

    /// Attach a client to a session stream.
    pub fn attach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime
            .session_runtime_mut()
            .attach_consumer(&session_id)?;
        self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Detach a client from a session stream.
    pub fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime
            .session_runtime_mut()
            .detach_consumer(&session_id)?;
        self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::UnsubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Write terminal bytes from a client into the worker-owned PTY.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::TerminalInput {
                session_id,
                data: data.into(),
            },
            now_seconds,
        )
    }

    /// Resize a session terminal from a client-facing path.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::Resize {
                session_id,
                rows,
                cols,
            },
            now_seconds,
        )
    }

    /// Drain currently available worker process output through subscription fanout.
    pub fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime.drain_runtime_once(session_id, last_output_at)
    }

    /// Shut down a worker-owned session.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime
            .shutdown_session(session_id, reason, now_seconds)
    }
}

#[cfg(feature = "local-runtime")]
impl Default for DefaultBotsterEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Ergonomic embeddable core API for tmux-like Botster consumers.
///
/// Hosts still provide concrete runtime adapters and policy-resolved spawn
/// requests. This facade only turns common session/client/plugin operations
/// into method calls over the lower-level `MultiplexerEngine`.
///
/// # Example
///
/// This example uses test-support fakes so the host-owned PTY process and
/// plugin callback policy stay outside `botster-core`.
///
/// ```
/// # use std::sync::Arc;
/// # use botster_core::{
/// #     BotsterEngine, BoundaryJson, ClientId, CoreSessionMetadata, ExtensionEntrypoint,
/// #     ExtensionKind, ExtensionRuntime, InitialSnapshotReady, NotificationContent,
/// #     NotificationId, NotificationItem, NotificationSeverity, NotificationSource,
/// #     NotificationTarget, NotificationTimestamp, PackageManifest, PluginHandlerKind,
/// #     PluginHandlerRef, PluginHandlerRegistration, PluginInvocationContext,
/// #     PluginInvocationRequest, PluginInvocationResult, PluginKey, PluginLoadSpec,
/// #     PluginWorkerRegistration, RequestId, SessionActivityStatus, SessionId,
/// #     SessionSpawnRequest, SessionWorkerRuntimeEvent, SpawnEnvironment, SpawnWorkingDirectory,
/// #     SubscriptionId, TransportEgress,
/// # };
/// # use botster_core_test_support::fake::{
/// #     FakePluginRuntime, FakeSessionRuntime, FakeSessionWorkerRuntime,
/// # };
/// let session_id = SessionId("docs-session".to_string());
/// let client_id = ClientId("docs-client".to_string());
/// let subscription_id = SubscriptionId("docs-subscription".to_string());
///
/// let mut engine: BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime> =
///     BotsterEngine::new(FakeSessionRuntime::new());
///
/// engine.spawn_session(
///     SessionSpawnRequest {
///         request_id: RequestId("docs-spawn".to_string()),
///         session_id: session_id.clone(),
///         executable: "fake-shell".to_string(),
///         arguments: vec!["--login".to_string()],
///         working_directory: SpawnWorkingDirectory {
///             path: "/workspace".to_string(),
///         },
///         environment: SpawnEnvironment::default(),
///         initial_pty_size: None,
///     },
///     CoreSessionMetadata::new(),
///     FakeSessionWorkerRuntime::new(),
/// )?;
///
/// engine.attach_client(client_id.clone(), session_id.clone(), subscription_id.clone(), 1)?;
/// engine.handle_runtime_event(SessionWorkerRuntimeEvent::InitialSnapshotReady(
///     InitialSnapshotReady {
///         request_id: RequestId("docs-initial".to_string()),
///         session_id: session_id.clone(),
///         client_id: client_id.clone(),
///         subscription_id: subscription_id.clone(),
///         snapshot: Vec::new(),
///         rows: 24,
///         cols: 80,
///     },
/// ))?;
/// engine.write_bytes(client_id.clone(), session_id.clone(), b"echo docs\n".to_vec(), 2)?;
/// engine.resize(client_id.clone(), session_id.clone(), 40, 120, 3)?;
///
/// let output = engine.receive_output(session_id.clone(), b"docs output\n".to_vec(), 4)?;
/// assert!(output.client_egress.iter().any(|(_, frame)| {
///     matches!(frame, TransportEgress::TerminalOutput { data, .. } if data == b"docs output\n")
/// }));
///
/// let notification_id = engine.post_notification(NotificationItem::message(
///     NotificationId("docs-notification".to_string()),
///     NotificationTarget::Session(session_id.clone()),
///     NotificationSeverity::Info,
///     NotificationSource {
///         label: "docs-host".to_string(),
///         plugin_key: None,
///     },
///     NotificationContent {
///         title: "Docs notice".to_string(),
///         body: None,
///         extension: None,
///     },
///     NotificationTimestamp(5),
/// ));
/// let notifications = engine.drain_notifications(
///     NotificationTarget::Session(session_id.clone()),
///     NotificationTimestamp(6),
/// );
/// assert_eq!(notifications[0].id, notification_id);
///
/// let plugin_key = PluginKey("docs-plugin".to_string());
/// let handler = PluginHandlerRef {
///     plugin_key: plugin_key.clone(),
///     kind: PluginHandlerKind::Command,
///     handler_id: "run".to_string(),
/// };
/// engine.load_plugin(PluginWorkerRegistration {
///     load: PluginLoadSpec {
///         plugin_key: plugin_key.clone(),
///         package: plugin_key.0.clone(),
///         entrypoint: "plugin.lua".to_string(),
///         descriptors: Vec::new(),
///         metadata: None,
///     },
///     manifest: PackageManifest {
///         name: plugin_key.0.clone(),
///         version: "0.1.0".to_string(),
///         kind: ExtensionKind::Plugin,
///         botster: ">=0.1.0".to_string(),
///         source: None,
///         capabilities: Vec::new(),
///         entrypoints: vec![ExtensionEntrypoint {
///             runtime: ExtensionRuntime::Lua,
///             path: "plugin.lua".to_string(),
///             bootstrap: false,
///         }],
///         host_profile: None,
///     },
///     runtime: Arc::new(FakePluginRuntime::success("ok")),
///     handlers: vec![PluginHandlerRegistration {
///         handler: handler.clone(),
///         required_capability: None,
///     }],
///     resources: Vec::new(),
/// });
/// let plugin_result = engine.invoke_plugin(PluginInvocationRequest {
///     request_id: RequestId("docs-plugin-request".to_string()),
///     handler,
///     timeout_ms: 1_000,
///     context: PluginInvocationContext {
///         client_id: Some(client_id.clone()),
///         session_id: Some(session_id.clone()),
///         subscription_id: Some(subscription_id.clone()),
///         surface_id: None,
///         origin: Some("docs-host".to_string()),
///         metadata: None,
///     },
///     payload: BoundaryJson(serde_json::json!({ "command": "run" })),
/// });
/// assert!(matches!(plugin_result.result, PluginInvocationResult::Completed(_)));
///
/// assert_eq!(
///     engine.classify_activity(&session_id, 5, 10)?,
///     SessionActivityStatus::Active
/// );
/// engine.shutdown_session(session_id, "docs complete", 7)?;
/// # Ok::<(), botster_core::BotsterEngineError>(())
/// ```
#[derive(Clone)]
pub struct BotsterEngine<R, W> {
    multiplexer: MultiplexerEngine<R, W>,
}

impl<R, W> BotsterEngine<R, W>
where
    R: SessionRuntime,
    W: SessionWorkerRuntime,
{
    /// Build an engine with a host session runtime and default plugin settings.
    pub fn new(session_runtime: R) -> Self {
        Self {
            multiplexer: MultiplexerEngine::new(session_runtime),
        }
    }

    /// Build an engine with explicit plugin worker settings.
    pub fn with_plugin_config(session_runtime: R, plugin_config: PluginWorkerEngineConfig) -> Self {
        Self {
            multiplexer: MultiplexerEngine::with_plugin_config(session_runtime, plugin_config),
        }
    }

    /// Return a recorded session.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        self.multiplexer.session(session_id)
    }

    /// Return sessions currently recorded by the command facade.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<CoreSession> {
        self.multiplexer.list_sessions()
    }

    /// Return the host runtime adapter.
    #[must_use]
    pub const fn session_runtime(&self) -> &R {
        self.multiplexer.session_runtime()
    }

    /// Return the plugin worker engine.
    ///
    /// Hosts that need reload or unload cleanup should call
    /// [`Self::reload_plugin`] or [`Self::unload_plugin`] so scheduler-owned
    /// timer resources are cleaned with worker-owned resources.
    #[must_use]
    pub const fn plugin_workers(&self) -> &PluginWorkerEngine {
        self.multiplexer.plugin_workers()
    }

    /// Return the plugin timer scheduler.
    #[must_use]
    pub const fn plugin_timers(&self) -> &PluginTimerScheduler {
        self.multiplexer.plugin_timers()
    }

    /// Return the lower-level assembled multiplexer engine.
    #[must_use]
    pub const fn multiplexer(&self) -> &MultiplexerEngine<R, W> {
        &self.multiplexer
    }

    /// Spawn a session, record core state, and install its worker adapter.
    pub fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
        worker_runtime: W,
    ) -> Result<BotsterSpawnOutcome, BotsterEngineError> {
        self.multiplexer
            .spawn_session(request, metadata, worker_runtime)
    }

    /// Execute one typed command through the public engine facade.
    pub fn execute_command(
        &mut self,
        command: EngineCommand<W>,
    ) -> Result<EngineCommandOutcome, EngineCommandError<BotsterEngineError>> {
        let kind = command.kind();
        match command {
            EngineCommand::SpawnSession {
                request,
                metadata,
                worker_runtime,
            } => self
                .spawn_session(request, metadata, worker_runtime)
                .map(EngineCommandOutcome::SpawnSession),
            EngineCommand::AttachClient {
                client_id,
                session_id,
                subscription_id,
                now_seconds,
            } => self
                .attach_client(client_id, session_id, subscription_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::DetachClient {
                client_id,
                session_id,
                subscription_id,
                now_seconds,
            } => self
                .detach_client(client_id, session_id, subscription_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::SendInput {
                client_id,
                session_id,
                data,
                now_seconds,
            } => self
                .write_bytes(client_id, session_id, data, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::Resize {
                client_id,
                session_id,
                rows,
                cols,
                now_seconds,
            } => self
                .resize(client_id, session_id, rows, cols, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::ListSessions => Ok(EngineCommandOutcome::Sessions(self.list_sessions())),
            EngineCommand::InspectSession {
                session_id,
                now_seconds,
                active_threshold_seconds,
            } => self
                .inspect_session(&session_id, now_seconds, active_threshold_seconds)
                .map(EngineCommandOutcome::Inspection),
            EngineCommand::ReadScreen {
                request_id,
                session_id,
                now_seconds,
            } => self
                .read_screen(request_id, session_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::CaptureSnapshot {
                request_id,
                session_id,
                now_seconds,
            } => self
                .capture_snapshot(request_id, session_id, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::ReplaySnapshot {
                request,
                now_seconds,
            } => self
                .replay_snapshot(request, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::Shutdown {
                session_id,
                reason,
                now_seconds,
            } => self
                .shutdown_session(session_id, reason, now_seconds)
                .map(EngineCommandOutcome::Output),
            EngineCommand::PostNotification { item } => Ok(
                EngineCommandOutcome::NotificationPosted(self.post_notification(item)),
            ),
            EngineCommand::DrainNotifications { target, now } => Ok(
                EngineCommandOutcome::NotificationsDrained(self.drain_notifications(target, now)),
            ),
            EngineCommand::LoadPlugin { registration } => {
                let plugin_key = registration.load.plugin_key.clone();
                self.load_plugin(registration);
                Ok(EngineCommandOutcome::PluginLoaded(plugin_key))
            }
            EngineCommand::ReloadPlugin { spec, registration } => Ok(
                EngineCommandOutcome::PluginReloaded(self.reload_plugin(spec, registration)),
            ),
            EngineCommand::UnloadPlugin { spec } => Ok(EngineCommandOutcome::PluginUnloaded(
                self.unload_plugin(spec),
            )),
            EngineCommand::InvokePlugin { request } => Ok(EngineCommandOutcome::PluginInvoked(
                self.invoke_plugin(request),
            )),
        }
        .map_err(|source| EngineCommandError::new(kind, source))
    }

    /// Attach a client to a session stream.
    pub fn attach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Detach a client from a session stream.
    pub fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_client_ingress(
            client_id.clone(),
            TransportIngress::UnsubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Write terminal bytes from a client into a session.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_client_ingress(
            client_id,
            TransportIngress::TerminalInput {
                session_id,
                data: data.into(),
            },
            now_seconds,
        )
    }

    /// Resize a session terminal from a client-facing path.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_client_ingress(
            client_id,
            TransportIngress::Resize {
                session_id,
                rows,
                cols,
            },
            now_seconds,
        )
    }

    /// Receive terminal output bytes from the host runtime.
    pub fn receive_output(
        &mut self,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
            session_id,
            data: data.into(),
            last_output_at,
        })
    }

    /// Read a session's plain screen state through the session worker path.
    pub fn read_screen(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_session_request(
            crate::SessionIoRequest::GetScreen {
                request_id,
                session_id,
            },
            now_seconds,
        )
    }

    /// Capture a session snapshot through the session worker path.
    pub fn capture_snapshot(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_session_request(
            crate::SessionIoRequest::GetSnapshot {
                request_id,
                session_id,
            },
            now_seconds,
        )
    }

    /// Replay or prepare a snapshot through the session worker path.
    pub fn replay_snapshot(
        &mut self,
        request: PreparedSnapshotRequest,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_session_request(
            crate::SessionIoRequest::PrepareSnapshot(request),
            now_seconds,
        )
    }

    /// Route one runtime-originated session worker event.
    pub fn handle_runtime_event(
        &mut self,
        event: SessionWorkerRuntimeEvent,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.handle_runtime_event(event)
    }

    /// Report client-side backpressure through the public engine facade.
    pub fn report_backpressure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer
            .report_backpressure(client_id, session_id, source, capacity, depth)
    }

    /// Report accepted-but-slow delivery through the public engine facade.
    pub fn report_delivery_lag(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.report_delivery_lag(
            client_id,
            session_id,
            subscription_id,
            source,
            capacity,
            depth,
        )
    }

    /// Report a failed delivery attempt through the public engine facade.
    pub fn report_delivery_failure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        reason: MailboxSendFailureReason,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer.report_delivery_failure(
            client_id,
            session_id,
            subscription_id,
            source,
            reason,
        )
    }

    /// Queue a notification item in the core inbox.
    pub fn post_notification(&mut self, item: NotificationItem) -> NotificationId {
        self.multiplexer.post_notification(item)
    }

    /// Drain deliverable notifications for one target.
    pub fn drain_notifications(
        &mut self,
        target: NotificationTarget,
        now: NotificationTimestamp,
    ) -> Vec<NotificationItem> {
        self.multiplexer.drain_notifications(target, now)
    }

    /// Load or replace one plugin worker.
    pub fn load_plugin(&self, registration: PluginWorkerRegistration) {
        self.multiplexer.load_plugin(registration);
    }

    /// Reload one plugin and cleanup scheduler-owned timer resources.
    pub fn reload_plugin(
        &self,
        spec: PluginReloadSpec,
        registration: PluginWorkerRegistration,
    ) -> PluginCleanupResult {
        self.multiplexer.reload_plugin(spec, registration)
    }

    /// Unload one plugin and cleanup scheduler-owned timer resources.
    pub fn unload_plugin(&self, spec: PluginUnloadSpec) -> PluginCleanupResult {
        self.multiplexer.unload_plugin(spec)
    }

    /// Invoke a registered plugin handler.
    pub fn invoke_plugin(&self, request: PluginInvocationRequest) -> PluginInvocationOutcome {
        self.multiplexer.invoke_plugin(request)
    }

    /// Schedule plugin timer work without invoking plugin code inline.
    pub fn schedule_plugin_timer(
        &self,
        schedule: PluginTimerSchedule,
    ) -> PluginTimerScheduleOutcome {
        self.multiplexer.schedule_plugin_timer(schedule)
    }

    /// Cancel one plugin timer by handle.
    pub fn cancel_plugin_timer(
        &self,
        request_id: crate::RequestId,
        plugin_key: &PluginKey,
        timer_id: &PluginTimerId,
    ) -> PluginTimerCancellationResult {
        self.multiplexer
            .cancel_plugin_timer(request_id, plugin_key, timer_id)
    }

    /// Drain due plugin timers through the existing plugin worker engine.
    pub fn drain_plugin_timers_due(&self, now_ms: u64) -> PluginTimerDrainOutcome {
        self.multiplexer.drain_plugin_timers_due(now_ms)
    }

    /// Classify one session's activity at the provided clock value.
    pub fn classify_activity(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<SessionActivityStatus, BotsterEngineError> {
        self.multiplexer.classify_session_activity(
            session_id,
            now_seconds,
            active_threshold_seconds,
        )
    }

    /// Inspect one session's lifecycle and activity through the command facade.
    pub fn inspect_session(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<EngineSessionInspection, BotsterEngineError> {
        Ok(EngineSessionInspection {
            session: self
                .session(session_id)
                .ok_or_else(|| BotsterEngineError::UnknownSession {
                    session_id: session_id.clone(),
                })?
                .clone(),
            activity_status: self.classify_activity(
                session_id,
                now_seconds,
                active_threshold_seconds,
            )?,
        })
    }

    /// Shut down one session worker and update core lifecycle state.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, BotsterEngineError> {
        self.multiplexer
            .shutdown_session(session_id, reason, now_seconds)
    }
}

impl<R, W> Default for BotsterEngine<R, W>
where
    R: SessionRuntime + Default,
    W: SessionWorkerRuntime,
{
    fn default() -> Self {
        Self::new(R::default())
    }
}
