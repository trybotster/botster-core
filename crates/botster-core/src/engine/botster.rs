//! Ergonomic embeddable Botster engine facade.

#[cfg(feature = "local-runtime")]
use std::collections::{HashMap, HashSet, VecDeque};

use crate::actor::{
    MailboxSendFailureReason, PluginAdmissionResult, PluginCleanupResult, PluginCompletionDrain,
    PluginInvocationClass, PluginInvocationRequest, PluginKey, PluginReloadSpec,
    PluginTimerCancellationResult, PluginTimerId, PluginTimerSchedule, PluginUnloadSpec,
    PreparedSnapshotRequest, QueueSource,
};
use crate::contract::notification::{
    NotificationId, NotificationItem, NotificationTarget, NotificationTimestamp,
};
use crate::contract::terminal_adapter::TerminalAdapter;
use crate::contract::terminal_subscription::{
    BindTerminalAdapterError, DetachTerminalSubscriptionResult, TerminalCapabilitySet,
    TerminalSubscriptionGeneration, TerminalSubscriptionRecord,
};
use crate::contract::transport::{TransportEgress, TransportIngress};
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
use crate::engine::terminal_screen::TerminalScreenRuntime;
#[cfg(feature = "local-runtime")]
use crate::runtime::ProcessIdentity;
#[cfg(feature = "local-runtime")]
use crate::runtime::{LocalProcessRuntime, WorkerProcessRuntime, WorkerProcessRuntimeOptions};
use crate::runtime::{SessionRuntime, SessionSpawnRequest};
use crate::session::{CoreSession, CoreSessionMetadata, SessionActivityStatus, SessionId};
#[cfg(feature = "local-runtime")]
use crate::terminal_screen::TerminalScreenSize;
#[cfg(feature = "local-runtime")]
use crate::terminal_screen::TerminalSnapshotPayload;
#[cfg(feature = "local-runtime")]
use crate::SessionMetadata;
use crate::{ClientId, SubscriptionId};

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
    runtime: ManagedSessionRuntime<LocalProcessRuntime, Box<dyn TerminalScreenRuntime>>,
}

/// Public local PTY-backed engine facade whose live PTY is owned by a worker process.
#[cfg(feature = "local-runtime")]
pub struct WorkerBackedBotsterEngine {
    runtime: ManagedSessionRuntime<WorkerProcessRuntime, Box<dyn TerminalScreenRuntime>>,
    incremental_attaches: HashMap<SessionId, IncrementalAttach>,
    applied_attach_resizes: HashMap<SessionId, (u16, u16, u64)>,
}

#[cfg(feature = "local-runtime")]
struct IncrementalAttach {
    client_id: ClientId,
    subscription_id: SubscriptionId,
    request_id: String,
    ready: bool,
    pending: VecDeque<(ClientId, SubscriptionId)>,
    queued_input: Vec<(ClientId, Vec<u8>, u64)>,
    queued_resize: Option<(ClientId, u16, u16, u64)>,
}

#[cfg(feature = "local-runtime")]
impl IncrementalAttach {
    fn replace_pending_client(
        &mut self,
        client_id: &ClientId,
        subscription_id: SubscriptionId,
    ) -> usize {
        let removed = self
            .pending
            .iter()
            .filter(|(pending_client, _)| pending_client == client_id)
            .count();
        self.pending
            .retain(|(pending_client, _)| pending_client != client_id);
        self.pending.push_back((client_id.clone(), subscription_id));
        removed
    }

    fn drop_pending_client(&mut self, client_id: &ClientId) -> usize {
        let removed = self
            .pending
            .iter()
            .filter(|(pending_client, _)| pending_client == client_id)
            .count();
        self.pending
            .retain(|(pending_client, _)| pending_client != client_id);
        removed
    }

    fn discard_client_queues(&mut self, client_id: &ClientId) {
        self.queued_input.retain(|(owner, _, _)| owner != client_id);
        if self
            .queued_resize
            .as_ref()
            .is_some_and(|(owner, ..)| owner == client_id)
        {
            self.queued_resize = None;
        }
    }

    fn discard_replaced_owner_queues(&mut self, replaced_client: &ClientId, new_client: &ClientId) {
        self.discard_client_queues(replaced_client);
        self.discard_client_queues(new_client);
    }
}

#[cfg(all(test, feature = "local-runtime"))]
mod incremental_pending_tests {
    use super::{ClientId, IncrementalAttach, SubscriptionId};
    use std::collections::VecDeque;

    #[test]
    fn replace_pending_client_drops_that_clients_older_tuples() {
        let mut attach = IncrementalAttach {
            client_id: ClientId("active".to_string()),
            subscription_id: SubscriptionId("active-sub".to_string()),
            request_id: "req".to_string(),
            ready: false,
            pending: VecDeque::from([
                (
                    ClientId("pending".to_string()),
                    SubscriptionId("old-sub".to_string()),
                ),
                (
                    ClientId("other".to_string()),
                    SubscriptionId("other-sub".to_string()),
                ),
            ]),
            queued_input: Vec::new(),
            queued_resize: None,
        };
        let removed = attach.replace_pending_client(
            &ClientId("pending".to_string()),
            SubscriptionId("new-sub".to_string()),
        );
        assert_eq!(removed, 1);
        assert_eq!(
            attach.pending.into_iter().collect::<Vec<_>>(),
            vec![
                (
                    ClientId("other".to_string()),
                    SubscriptionId("other-sub".to_string()),
                ),
                (
                    ClientId("pending".to_string()),
                    SubscriptionId("new-sub".to_string()),
                ),
            ]
        );
    }

    #[test]
    fn drop_pending_client_keeps_other_clients() {
        let mut attach = IncrementalAttach {
            client_id: ClientId("active".to_string()),
            subscription_id: SubscriptionId("active-sub".to_string()),
            request_id: "req".to_string(),
            ready: false,
            pending: VecDeque::from([
                (
                    ClientId("takeover".to_string()),
                    SubscriptionId("old-sub".to_string()),
                ),
                (
                    ClientId("sibling".to_string()),
                    SubscriptionId("sibling-sub".to_string()),
                ),
            ]),
            queued_input: vec![(ClientId("sibling".to_string()), b"keep".to_vec(), 1)],
            queued_resize: Some((ClientId("sibling".to_string()), 30, 100, 2)),
        };
        let removed = attach.drop_pending_client(&ClientId("takeover".to_string()));
        attach.discard_replaced_owner_queues(
            &ClientId("active".to_string()),
            &ClientId("takeover".to_string()),
        );
        assert_eq!(removed, 1);
        assert_eq!(
            attach.pending.into_iter().collect::<Vec<_>>(),
            vec![(
                ClientId("sibling".to_string()),
                SubscriptionId("sibling-sub".to_string()),
            )]
        );
        assert_eq!(attach.queued_input.len(), 1);
        assert_eq!(
            attach.queued_resize,
            Some((ClientId("sibling".to_string()), 30, 100, 2))
        );
    }

    #[test]
    fn discard_client_queues_drops_only_that_client() {
        let mut attach = IncrementalAttach {
            client_id: ClientId("active".to_string()),
            subscription_id: SubscriptionId("active-sub".to_string()),
            request_id: "req".to_string(),
            ready: false,
            pending: VecDeque::new(),
            queued_input: vec![
                (ClientId("failed".to_string()), b"stale".to_vec(), 1),
                (ClientId("kept".to_string()), b"keep".to_vec(), 2),
            ],
            queued_resize: Some((ClientId("failed".to_string()), 30, 100, 3)),
        };
        attach.discard_client_queues(&ClientId("failed".to_string()));
        assert_eq!(attach.queued_input.len(), 1);
        assert_eq!(attach.queued_input[0].0, ClientId("kept".to_string()));
        assert_eq!(attach.queued_resize, None);
    }
}

#[cfg(feature = "local-runtime")]
fn runtime_with_plain_terminal_backend<R>(
    runtime: R,
) -> ManagedSessionRuntime<R, Box<dyn TerminalScreenRuntime>>
where
    R: SessionRuntime,
{
    ManagedSessionRuntime::with_terminal_backend_factory(runtime, |size| {
        Ok::<_, std::convert::Infallible>(Box::new(
            crate::engine::terminal_screen::PlainTerminalScreenRuntime::new(size),
        ) as Box<dyn TerminalScreenRuntime>)
    })
}

#[cfg(feature = "local-runtime")]
fn runtime_with_boxed_terminal_backend<E, T, F, R>(
    runtime: R,
    factory: F,
) -> ManagedSessionRuntime<R, Box<dyn TerminalScreenRuntime>>
where
    E: std::error::Error + Send + Sync + 'static,
    T: TerminalScreenRuntime + 'static,
    F: Fn(TerminalScreenSize) -> Result<T, E> + 'static,
    R: SessionRuntime,
{
    ManagedSessionRuntime::with_terminal_backend_factory(runtime, move |size| {
        factory(size).map(|terminal| Box::new(terminal) as Box<dyn TerminalScreenRuntime>)
    })
}

#[cfg(feature = "local-runtime")]
impl DefaultBotsterEngine {
    /// Build an empty local PTY-backed engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            runtime: runtime_with_plain_terminal_backend(LocalProcessRuntime::new()),
        }
    }

    /// Build an empty local PTY-backed engine with a host-supplied terminal backend.
    ///
    /// This keeps the facade monomorphic while letting first-party host
    /// profiles install a concrete terminal parser/snapshot backend.
    pub fn with_terminal_backend_factory<E, T, F>(factory: F) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
        T: TerminalScreenRuntime + 'static,
        F: Fn(TerminalScreenSize) -> Result<T, E> + 'static,
    {
        Self {
            runtime: runtime_with_boxed_terminal_backend(LocalProcessRuntime::new(), factory),
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

    /// Forget all local engine state for one terminal session.
    pub fn forget_terminal_session(&mut self, session_id: &SessionId) -> bool {
        self.runtime.forget_terminal_session(session_id)
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
        let mut output = self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::SubscribeSession {
                client_id,
                session_id: session_id.clone(),
                subscription_id,
            },
            now_seconds,
        )?;
        let initial_snapshot = self.runtime.drain_runtime_once(&session_id, now_seconds)?;
        output.client_egress.extend(initial_snapshot.client_egress);
        output
            .session_requests
            .extend(initial_snapshot.session_requests);
        output
            .client_control_frames
            .extend(initial_snapshot.client_control_frames);
        output
            .session_events
            .extend(initial_snapshot.session_events);
        output.observations.extend(initial_snapshot.observations);
        Ok(output)
    }

    /// Bind a content-blind adapter to a live attach generation.
    pub fn bind_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        adapter: Box<dyn TerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        self.runtime.bind_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            generation,
            capabilities,
            adapter,
        )
    }

    /// Control-plane subscription inventory.
    #[must_use]
    pub fn list_terminal_subscriptions(&self) -> Vec<TerminalSubscriptionRecord> {
        self.runtime.list_terminal_subscriptions()
    }

    /// Detach one live generation if present.
    pub fn detach_terminal_subscription(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        now_seconds: u64,
    ) -> Result<(DetachTerminalSubscriptionResult, BotsterEngineOutput), DefaultBotsterEngineError>
    {
        self.runtime.detach_terminal_subscription(
            client_id,
            session_id,
            subscription_id,
            generation,
            now_seconds,
        )
    }

    /// Live generation for a subscription, if any.
    #[must_use]
    pub fn terminal_subscription_generation(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<TerminalSubscriptionGeneration> {
        self.runtime
            .terminal_subscription_generation(session_id, subscription_id)
    }

    /// Whether a bound adapter is still held.
    #[must_use]
    pub fn adapter_is_bound(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.runtime.adapter_is_bound(session_id, subscription_id)
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

    /// Read authoritative terminal mode flags through the managed session path.
    pub fn read_mode_flags(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, DefaultBotsterEngineError> {
        self.runtime.handle_session_request(
            crate::SessionIoRequest::GetModeFlags {
                request_id,
                session_id,
            },
            now_seconds,
        )
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

    /// Capture a reusable opaque snapshot payload for one session.
    pub fn capture_snapshot_payload(
        &mut self,
        session_id: &SessionId,
    ) -> Result<TerminalSnapshotPayload, DefaultBotsterEngineError> {
        self.runtime.capture_snapshot_payload(session_id)
    }

    /// Capture screen, snapshot, and authoritative mode read from one terminal shadow.
    pub fn capture_terminal_state(
        &mut self,
        session_id: &SessionId,
    ) -> Result<
        (
            crate::TerminalScreenState,
            TerminalSnapshotPayload,
            Result<crate::ModeFlags, crate::TerminalBackendError>,
        ),
        DefaultBotsterEngineError,
    > {
        self.runtime.capture_terminal_state(session_id)
    }

    /// Capture colors and GHOSTSNP under one terminal ownership section.
    pub fn capture_color_and_snapshot(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(crate::TerminalColorProfile, TerminalSnapshotPayload), DefaultBotsterEngineError>
    {
        self.runtime.capture_color_and_snapshot(session_id)
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
            runtime: runtime_with_plain_terminal_backend(WorkerProcessRuntime::new(worker_path)),
            incremental_attaches: HashMap::new(),
            applied_attach_resizes: HashMap::new(),
        }
    }

    /// Build an empty worker-backed local PTY engine with explicit options.
    #[must_use]
    pub fn with_options(options: WorkerProcessRuntimeOptions) -> Self {
        Self {
            runtime: runtime_with_plain_terminal_backend(WorkerProcessRuntime::with_options(
                options,
            )),
            incremental_attaches: HashMap::new(),
            applied_attach_resizes: HashMap::new(),
        }
    }

    /// Build an empty worker-backed local PTY engine with explicit options and
    /// a host-supplied terminal backend.
    pub fn with_options_and_terminal_backend_factory<E, T, F>(
        options: WorkerProcessRuntimeOptions,
        factory: F,
    ) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
        T: TerminalScreenRuntime + 'static,
        F: Fn(TerminalScreenSize) -> Result<T, E> + 'static,
    {
        Self {
            runtime: runtime_with_boxed_terminal_backend(
                WorkerProcessRuntime::with_options(options),
                factory,
            ),
            incremental_attaches: HashMap::new(),
            applied_attach_resizes: HashMap::new(),
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

    /// Forget all worker-backed engine state for one terminal session.
    pub fn forget_terminal_session(&mut self, session_id: &SessionId) -> bool {
        self.runtime.forget_terminal_session(session_id)
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
        supports_snapshot_boundary: bool,
        metadata: CoreSessionMetadata,
    ) -> Result<BotsterSpawnOutcome, WorkerBackedBotsterEngineError> {
        self.runtime.adopt_worker_process(
            session_id,
            process,
            socket_path,
            supports_snapshot_boundary,
            metadata,
        )
    }

    /// Release workers without sending shutdown frames for an intentional daemon restart.
    pub fn release_workers_for_restart(&mut self) {
        for (session_id, attach) in std::mem::take(&mut self.incremental_attaches) {
            let _ = self
                .runtime
                .session_runtime_mut()
                .cancel_snapshot_boundary(&session_id, &attach.request_id);
        }
        self.applied_attach_resizes.clear();
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
        let supports_snapshot_boundary = self
            .runtime
            .worker_supports_snapshot_boundary(&session_id)?;
        if supports_snapshot_boundary {
            if let Some((current_client, current_subscription)) = self
                .incremental_attaches
                .get(&session_id)
                .map(|attach| (attach.client_id.clone(), attach.subscription_id.clone()))
            {
                if current_client == client_id && current_subscription != subscription_id {
                    self.detach_client(
                        client_id.clone(),
                        session_id.clone(),
                        current_subscription,
                        now_seconds,
                    )?;
                } else if current_client != client_id && current_subscription == subscription_id {
                    return self.takeover_current_incremental_attach(
                        client_id,
                        session_id,
                        subscription_id,
                    );
                }
            }
            if self.incremental_attaches.contains_key(&session_id) {
                let queued = self
                    .incremental_attaches
                    .get(&session_id)
                    .expect("incremental attach was checked above")
                    .pending
                    .len();
                if queued.saturating_add(1) >= QueueSource::ClientWorker.default_capacity() {
                    return Err(ManagedSessionRuntimeError::Runtime(
                        crate::SessionRuntimeError::new(
                            crate::SessionRuntimeErrorKind::OutputFailed,
                            "incremental attach queue is full; retry after drain",
                        ),
                    ));
                }
                self.incremental_attaches
                    .get_mut(&session_id)
                    .expect("incremental attach was checked above")
                    .replace_pending_client(&client_id, subscription_id.clone());
                let output = self.runtime.begin_snapshot_attach(
                    client_id.clone(),
                    session_id.clone(),
                    subscription_id,
                )?;
                self.sync_worker_consumers(&session_id)?;
                return Ok(output);
            }
            let output = self.runtime.begin_snapshot_attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
            )?;
            let request_id = match self
                .runtime
                .session_runtime_mut()
                .begin_snapshot_boundary(&session_id)
            {
                Ok(request_id) => request_id,
                Err(error) => {
                    let _ = self.runtime.detach_live_subscription(
                        client_id,
                        session_id.clone(),
                        subscription_id,
                        now_seconds,
                    );
                    let _ = self.sync_worker_consumers(&session_id);
                    return Err(error.into());
                }
            };
            self.incremental_attaches.insert(
                session_id.clone(),
                IncrementalAttach {
                    client_id,
                    subscription_id,
                    request_id,
                    ready: false,
                    pending: VecDeque::new(),
                    queued_input: Vec::new(),
                    queued_resize: None,
                },
            );
            self.sync_worker_consumers(&session_id)?;
            return Ok(output);
        }

        let (mut output, attach_snapshot) = {
            let output = self.runtime.drain_runtime_once(&session_id, now_seconds)?;
            let snapshot = self.runtime.capture_parent_snapshot(&session_id)?;
            (output, snapshot)
        };
        output.client_egress.retain(|(routed_client, frame)| {
            routed_client != &client_id
                || !matches!(
                    frame,
                    TransportEgress::TerminalOutput {
                        session_id: routed_session,
                        ..
                    } if routed_session == &session_id
                )
        });
        let attach = self.runtime.attach_snapshot(
            client_id,
            session_id.clone(),
            subscription_id,
            attach_snapshot.bytes,
        )?;
        output.client_egress.extend(attach.client_egress);
        output.session_requests.extend(attach.session_requests);
        output
            .client_control_frames
            .extend(attach.client_control_frames);
        output.session_events.extend(attach.session_events);
        output.observations.extend(attach.observations);
        self.sync_worker_consumers(&session_id)?;
        Ok(output)
    }

    /// Bind a content-blind adapter to a live attach generation.
    pub fn bind_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        adapter: Box<dyn TerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        self.runtime.bind_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            generation,
            capabilities,
            adapter,
        )
    }

    /// Control-plane subscription inventory.
    #[must_use]
    pub fn list_terminal_subscriptions(&self) -> Vec<TerminalSubscriptionRecord> {
        self.runtime.list_terminal_subscriptions()
    }

    /// Detach one live generation if present.
    pub fn detach_terminal_subscription(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        now_seconds: u64,
    ) -> Result<
        (DetachTerminalSubscriptionResult, BotsterEngineOutput),
        WorkerBackedBotsterEngineError,
    > {
        if self
            .runtime
            .list_terminal_subscriptions()
            .iter()
            .any(|row| {
                row.session_id == session_id
                    && row.subscription_id == subscription_id
                    && row.generation == generation
            })
        {
            if let Some(mut attach) = self.incremental_attaches.remove(&session_id) {
                if attach.client_id == client_id && attach.subscription_id == subscription_id {
                    self.runtime
                        .session_runtime_mut()
                        .cancel_snapshot_boundary(&session_id, &attach.request_id)?;
                    self.promote_pending_fail_closed(attach, &session_id);
                } else {
                    attach
                        .pending
                        .retain(|(pending_client, pending_subscription)| {
                            pending_client != &client_id || pending_subscription != &subscription_id
                        });
                    attach.discard_client_queues(&client_id);
                    self.incremental_attaches.insert(session_id.clone(), attach);
                }
            }
        }
        let result = self.runtime.detach_terminal_subscription(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            now_seconds,
        );
        let _ = self.sync_worker_consumers(&session_id);
        result
    }

    /// Live generation for a subscription, if any.
    #[must_use]
    pub fn terminal_subscription_generation(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<TerminalSubscriptionGeneration> {
        self.runtime
            .terminal_subscription_generation(session_id, subscription_id)
    }

    /// Whether a bound adapter is still held.
    #[must_use]
    pub fn adapter_is_bound(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.runtime.adapter_is_bound(session_id, subscription_id)
    }

    /// Detach a client from a session stream.
    pub fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        if let Some(mut attach) = self.incremental_attaches.remove(&session_id) {
            if attach.client_id == client_id && attach.subscription_id == subscription_id {
                self.runtime
                    .session_runtime_mut()
                    .cancel_snapshot_boundary(&session_id, &attach.request_id)?;
                attach.discard_client_queues(&client_id);
                self.promote_pending_fail_closed(attach, &session_id);
            } else {
                attach
                    .pending
                    .retain(|(pending_client, pending_subscription)| {
                        pending_client != &client_id || pending_subscription != &subscription_id
                    });
                attach.discard_client_queues(&client_id);
                self.incremental_attaches.insert(session_id.clone(), attach);
            }
        }
        let output = self.runtime.handle_client_ingress(
            client_id.clone(),
            TransportIngress::UnsubscribeSession {
                client_id,
                session_id: session_id.clone(),
                subscription_id,
            },
            now_seconds,
        );
        let _ = self.sync_worker_consumers(&session_id);
        output
    }

    /// Write terminal bytes from a client into the worker-owned PTY.
    pub fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        let data = data.into();
        if let Some(attach) = self.incremental_attaches.get_mut(&session_id) {
            if attach.queued_input.len() >= QueueSource::ClientWorker.default_capacity() {
                return Err(ManagedSessionRuntimeError::Runtime(
                    crate::SessionRuntimeError::new(
                        crate::SessionRuntimeErrorKind::InputFailed,
                        "incremental attach input queue is full; retry after drain",
                    ),
                ));
            }
            attach.queued_input.push((client_id, data, now_seconds));
            return Ok(BotsterEngineOutput::empty());
        }
        self.runtime.handle_client_ingress(
            client_id,
            TransportIngress::TerminalInput { session_id, data },
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
        if let Some(attach) = self.incremental_attaches.get_mut(&session_id) {
            attach.queued_resize = Some((client_id, rows, cols, now_seconds));
            return Ok(BotsterEngineOutput::empty());
        }
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

    /// Return whether this session currently queues resize requests for attach.
    #[must_use]
    pub fn incremental_attach_active(&self, session_id: &SessionId) -> bool {
        self.incremental_attaches.contains_key(session_id)
    }

    /// Take the latest resize that the worker applied inside an attach barrier.
    pub fn take_applied_attach_resize(
        &mut self,
        session_id: &SessionId,
    ) -> Option<(u16, u16, u64)> {
        self.applied_attach_resizes.remove(session_id)
    }

    /// Drain currently available worker process output through subscription fanout.
    pub fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        let Some(mut attach) = self.incremental_attaches.remove(session_id) else {
            return self.runtime.drain_runtime_once(session_id, last_output_at);
        };
        let poll = match self
            .runtime
            .session_runtime_mut()
            .poll_snapshot_boundary(session_id, &attach.request_id)
        {
            Ok(poll) => poll,
            Err(error) => {
                let not_found = error.kind == crate::SessionRuntimeErrorKind::SessionNotFound;
                let _ = self
                    .runtime
                    .session_runtime_mut()
                    .cancel_snapshot_boundary(session_id, &attach.request_id);
                if not_found {
                    self.promote_pending_fail_closed(attach, session_id);
                    let mut output = self
                        .runtime
                        .drain_runtime_once(session_id, last_output_at)?;
                    let pumped = self.runtime.pump_bound_adapters()?;
                    append_engine_output(&mut output, pumped);
                    return Ok(output);
                }
                return Err(error.into());
            }
        };
        let mut output = match self.runtime.route_worker_boundary_outputs(
            session_id,
            poll.before_ready,
            last_output_at,
        ) {
            Ok(output) => output,
            Err(error) => {
                let _ = self
                    .runtime
                    .session_runtime_mut()
                    .cancel_snapshot_boundary(session_id, &attach.request_id);
                return Err(error);
            }
        };
        suppress_attach_terminal_output(&mut output, session_id, &attach);

        let mut history_incomplete = false;
        let mut finished = false;
        for frame in poll.frames {
            let error = frame
                .error_kind
                .or_else(|| {
                    frame
                        .phase
                        .is_none()
                        .then(|| "worker snapshot frame omitted its phase".to_string())
                })
                .or_else(|| {
                    frame
                        .snapshot
                        .is_none()
                        .then(|| "worker snapshot frame omitted its bytes".to_string())
                });
            if let Some(error) = error {
                if !attach.ready {
                    let _ = self
                        .runtime
                        .session_runtime_mut()
                        .cancel_snapshot_boundary(session_id, &attach.request_id);
                    let _ = self.runtime.handle_client_ingress(
                        attach.client_id.clone(),
                        TransportIngress::UnsubscribeSession {
                            client_id: attach.client_id.clone(),
                            session_id: session_id.clone(),
                            subscription_id: attach.subscription_id.clone(),
                        },
                        last_output_at,
                    );
                    self.promote_pending_fail_closed(attach, session_id);
                    return Err(ManagedSessionRuntimeError::Runtime(
                        crate::SessionRuntimeError::new(
                            crate::SessionRuntimeErrorKind::OutputFailed,
                            error,
                        ),
                    ));
                }
                history_incomplete = true;
                finished = true;
                break;
            }
            let phase = frame.phase.expect("snapshot phase was validated above");
            let snapshot = frame.snapshot.expect("snapshot bytes were validated above");
            self.runtime
                .note_snapshot_phase(session_id, &attach.subscription_id, phase);
            let frame_output = match self.runtime.snapshot_attach_frame(
                attach.client_id.clone(),
                session_id.clone(),
                attach.subscription_id.clone(),
                snapshot.bytes,
            ) {
                Ok(output) => output,
                Err(error) => {
                    let _ = self
                        .runtime
                        .session_runtime_mut()
                        .cancel_snapshot_boundary(session_id, &attach.request_id);
                    return Err(error);
                }
            };
            append_engine_output(&mut output, frame_output);
            match phase {
                crate::WorkerSnapshotPhase::Ready => attach.ready = true,
                crate::WorkerSnapshotPhase::History => {}
                crate::WorkerSnapshotPhase::Finish => finished = true,
            }
        }

        if !finished {
            // Snapshot polling does not call drain_output. Live PTY bytes and
            // ProcessExited stay in the worker session until that drain. A bound
            // adapter has no other consumer, so pull those frames now. Unbound
            // attach keeps one snapshot frame per host tick.
            if self
                .runtime
                .adapter_is_bound(session_id, &attach.subscription_id)
            {
                let live = self
                    .runtime
                    .drain_runtime_once(session_id, last_output_at)?;
                append_engine_output(&mut output, live);
                if matches!(
                    self.runtime
                        .session(session_id)
                        .map(|session| &session.lifecycle),
                    None | Some(crate::SessionLifecycleState::Exited { .. })
                        | Some(crate::SessionLifecycleState::Failed { .. })
                ) {
                    let _ = self
                        .runtime
                        .session_runtime_mut()
                        .cancel_snapshot_boundary(session_id, &attach.request_id);
                    self.promote_pending_fail_closed(attach, session_id);
                    self.reconcile_incremental_attach_after_teardown(session_id)?;
                    let pumped = self.runtime.pump_bound_adapters()?;
                    append_engine_output(&mut output, pumped);
                    return Ok(output);
                }
            }
            self.incremental_attaches.insert(session_id.clone(), attach);
            self.reconcile_incremental_attach_after_teardown(session_id)?;
            let pumped = self.runtime.pump_bound_adapters()?;
            append_engine_output(&mut output, pumped);
            return Ok(output);
        }

        let applied_resize = attach.queued_resize.take();
        if let Some((resize_client, rows, cols, resize_at)) = applied_resize.as_ref() {
            let resize_output = match self.runtime.handle_client_ingress(
                resize_client.clone(),
                TransportIngress::Resize {
                    session_id: session_id.clone(),
                    rows: *rows,
                    cols: *cols,
                },
                *resize_at,
            ) {
                Ok(output) => output,
                Err(error) => {
                    let _ = self
                        .runtime
                        .session_runtime_mut()
                        .cancel_snapshot_boundary(session_id, &attach.request_id);
                    return Err(error);
                }
            };
            append_engine_output(&mut output, resize_output);
        }
        self.runtime
            .session_runtime_mut()
            .complete_snapshot_boundary(session_id, &attach.request_id)?;
        if let Some((_, rows, cols, resize_at)) = applied_resize {
            self.applied_attach_resizes
                .insert(session_id.clone(), (rows, cols, resize_at));
        }
        let attached = self.runtime.complete_snapshot_attach(
            attach.client_id.clone(),
            session_id.clone(),
            attach.subscription_id.clone(),
            history_incomplete,
        )?;
        append_engine_output(&mut output, attached);
        self.sync_worker_consumers(session_id)?;

        // Barrier release can leave producer bytes in the capacity-one worker
        // egress. Drain them as live output after Attached so the child can
        // consume the later FRAME_PTY_INPUT instead of only echoing it.
        let leftover = self
            .runtime
            .drain_runtime_once(session_id, last_output_at)?;
        append_engine_output(&mut output, leftover);

        let mut deferred_input = Vec::new();
        for (input_client, data, input_at) in std::mem::take(&mut attach.queued_input) {
            if attach
                .pending
                .iter()
                .any(|(pending_client, _)| pending_client == &input_client)
            {
                deferred_input.push((input_client, data, input_at));
                continue;
            }
            let input_output = self.runtime.handle_client_ingress(
                input_client,
                TransportIngress::TerminalInput {
                    session_id: session_id.clone(),
                    data,
                },
                input_at,
            )?;
            append_engine_output(&mut output, input_output);
        }

        attach.queued_input = deferred_input;
        attach.queued_resize = None;
        self.promote_pending_fail_closed(attach, session_id);

        let mut live = self
            .runtime
            .drain_runtime_once(session_id, last_output_at)?;
        if let Some(current) = self.incremental_attaches.get(session_id) {
            if !self
                .runtime
                .adapter_is_bound(session_id, &current.subscription_id)
            {
                suppress_attach_terminal_output(&mut live, session_id, current);
            }
        }
        append_engine_output(&mut output, live);
        self.reconcile_incremental_attach_after_teardown(session_id)?;
        let pumped = self.runtime.pump_bound_adapters()?;
        append_engine_output(&mut output, pumped);
        Ok(output)
    }

    fn takeover_current_incremental_attach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        let Some(mut attach) = self.incremental_attaches.remove(&session_id) else {
            return self.attach_client(client_id, session_id, subscription_id, 0);
        };
        let replaced_client = attach.client_id.clone();
        let replaced_subscription = attach.subscription_id.clone();
        let stale_request_id = attach.request_id.clone();
        if let Err(error) = self
            .runtime
            .session_runtime_mut()
            .cancel_snapshot_boundary(&session_id, &stale_request_id)
        {
            self.incremental_attaches.insert(session_id, attach);
            return Err(error.into());
        }
        let request_id = match self
            .runtime
            .session_runtime_mut()
            .begin_snapshot_boundary(&session_id)
        {
            Ok(request_id) => request_id,
            Err(error) => {
                self.discard_takeover_pending(&mut attach, &session_id, &client_id)?;
                return self.fail_closed_cancelled_takeover(
                    attach,
                    session_id,
                    replaced_client,
                    replaced_subscription,
                    error,
                );
            }
        };
        self.discard_takeover_pending(&mut attach, &session_id, &client_id)?;
        attach.discard_replaced_owner_queues(&replaced_client, &client_id);
        let output = match self.runtime.begin_snapshot_attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        ) {
            Ok(output) => output,
            Err(error) => {
                let _ = self
                    .runtime
                    .session_runtime_mut()
                    .cancel_snapshot_boundary(&session_id, &request_id);
                return self.fail_closed_cancelled_takeover(
                    attach,
                    session_id,
                    replaced_client,
                    replaced_subscription,
                    error,
                );
            }
        };
        attach.client_id = client_id;
        attach.subscription_id = subscription_id;
        attach.request_id = request_id;
        attach.ready = false;
        self.incremental_attaches.insert(session_id.clone(), attach);
        self.sync_worker_consumers(&session_id)?;
        Ok(output)
    }

    fn discard_takeover_pending(
        &mut self,
        attach: &mut IncrementalAttach,
        session_id: &SessionId,
        client_id: &ClientId,
    ) -> Result<(), WorkerBackedBotsterEngineError> {
        let _ = session_id;
        attach.drop_pending_client(client_id);
        Ok(())
    }

    fn fail_closed_cancelled_takeover(
        &mut self,
        mut attach: IncrementalAttach,
        session_id: SessionId,
        replaced_client: ClientId,
        replaced_subscription: SubscriptionId,
        error: impl Into<WorkerBackedBotsterEngineError>,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        let current_client = attach.client_id.clone();
        attach.discard_replaced_owner_queues(&replaced_client, &current_client);
        let _ = self.runtime.detach_live_subscription(
            replaced_client,
            session_id.clone(),
            replaced_subscription,
            0,
        );
        self.promote_pending_fail_closed(attach, &session_id);
        let _ = self.sync_worker_consumers(&session_id);
        Err(error.into())
    }

    fn promote_pending_fail_closed(
        &mut self,
        mut attach: IncrementalAttach,
        session_id: &SessionId,
    ) {
        let outgoing = attach.client_id.clone();
        attach.discard_client_queues(&outgoing);
        while let Some((next_client, next_subscription)) = attach.pending.pop_front() {
            match self
                .runtime
                .session_runtime_mut()
                .begin_snapshot_boundary(session_id)
            {
                Ok(request_id) => {
                    attach.client_id = next_client;
                    attach.subscription_id = next_subscription;
                    attach.request_id = request_id;
                    attach.ready = false;
                    self.incremental_attaches.insert(session_id.clone(), attach);
                    let _ = self.sync_worker_consumers(session_id);
                    return;
                }
                Err(_) => {
                    attach.discard_client_queues(&next_client);
                    let _ = self.runtime.detach_live_subscription(
                        next_client,
                        session_id.clone(),
                        next_subscription,
                        0,
                    );
                }
            }
        }
        let _ = self.sync_worker_consumers(session_id);
    }

    fn sync_worker_consumers(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), WorkerBackedBotsterEngineError> {
        // Stall only after Attached. An in-progress incremental owner still
        // has an inventory row, but the parent may stop pumping at READY.
        let mut excluded = HashSet::new();
        if let Some(attach) = self.incremental_attaches.get(session_id) {
            excluded.insert((attach.client_id.clone(), attach.subscription_id.clone()));
            excluded.extend(attach.pending.iter().cloned());
        }
        let owners = self
            .runtime
            .list_terminal_subscriptions()
            .into_iter()
            .filter(|row| {
                row.session_id == *session_id
                    && !excluded.contains(&(row.client_id.clone(), row.subscription_id.clone()))
            })
            .map(|row| (row.client_id, row.subscription_id))
            .collect::<Vec<_>>();
        self.runtime
            .session_runtime_mut()
            .replace_named_consumers(session_id, owners)
            .map_err(Into::into)
    }

    fn reconcile_incremental_attach_after_teardown(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), WorkerBackedBotsterEngineError> {
        let Some(attach) = self.incremental_attaches.get(session_id) else {
            return Ok(());
        };
        if self.runtime.terminal_subscription_matches(
            session_id,
            &attach.client_id,
            &attach.subscription_id,
        ) {
            return Ok(());
        }
        let attach = self
            .incremental_attaches
            .remove(session_id)
            .expect("incremental attach existed above");
        self.runtime
            .session_runtime_mut()
            .cancel_snapshot_boundary(session_id, &attach.request_id)?;
        self.promote_pending_fail_closed(attach, session_id);
        Ok(())
    }

    /// Read a session's plain screen state through the worker-backed managed runtime.
    pub fn read_screen(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        self.runtime
            .read_screen(request_id, session_id, now_seconds)
    }

    /// Read authoritative terminal mode flags through the worker-backed path.
    pub fn read_mode_flags(
        &mut self,
        request_id: crate::RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        // Worker-backed production path must use worker Ghostty token authority.
        // Do not silently substitute parent-shadow freshness on probe failure.
        let _ = now_seconds;
        let payload = self
            .runtime
            .session_runtime_mut()
            .read_mode_flags(&session_id)
            .map_err(WorkerBackedBotsterEngineError::from)?;
        let mut outcome = BotsterEngineOutput::empty();
        outcome
            .session_events
            .push(crate::SessionIoEvent::ModeFlagsReady(
                crate::ModeFlagsReady {
                    request_id,
                    session_id,
                    mode_flags: payload.mode_flags,
                    mode_freshness: payload.mode_freshness,
                },
            ));
        Ok(outcome)
    }

    /// Correlated mode-gated PTY input against the worker atomic admit barrier.
    pub fn mode_gated_pty_input(
        &mut self,
        session_id: SessionId,
        expected: crate::ModeFreshnessToken,
        data: Vec<u8>,
    ) -> Result<crate::ModeGatedPtyInputResult, WorkerBackedBotsterEngineError> {
        self.runtime
            .session_runtime_mut()
            .mode_gated_pty_input(&session_id, expected, data)
            .map_err(WorkerBackedBotsterEngineError::from)
    }

    /// Capture a reusable opaque snapshot payload for one worker-backed session.
    pub fn capture_snapshot_payload(
        &mut self,
        session_id: &SessionId,
    ) -> Result<TerminalSnapshotPayload, WorkerBackedBotsterEngineError> {
        self.runtime.capture_snapshot_payload(session_id)
    }

    /// Capture screen, snapshot, and authoritative mode read from one terminal shadow.
    pub fn capture_terminal_state(
        &mut self,
        session_id: &SessionId,
    ) -> Result<
        (
            crate::TerminalScreenState,
            TerminalSnapshotPayload,
            Result<crate::ModeFlags, crate::TerminalBackendError>,
        ),
        WorkerBackedBotsterEngineError,
    > {
        self.runtime.capture_terminal_state(session_id)
    }

    /// Capture colors and GHOSTSNP under one terminal ownership section.
    pub fn capture_color_and_snapshot(
        &mut self,
        session_id: &SessionId,
    ) -> Result<
        (crate::TerminalColorProfile, TerminalSnapshotPayload),
        WorkerBackedBotsterEngineError,
    > {
        self.runtime.capture_color_and_snapshot(session_id)
    }

    /// Shut down a worker-owned session.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<BotsterEngineOutput, WorkerBackedBotsterEngineError> {
        if let Some(attach) = self.incremental_attaches.remove(&session_id) {
            self.runtime
                .session_runtime_mut()
                .cancel_snapshot_boundary(&session_id, &attach.request_id)?;
        }
        self.applied_attach_resizes.remove(&session_id);
        self.runtime
            .shutdown_session(session_id, reason, now_seconds)
    }
}

#[cfg(feature = "local-runtime")]
fn append_engine_output(target: &mut BotsterEngineOutput, source: BotsterEngineOutput) {
    target.client_egress.extend(source.client_egress);
    target.session_requests.extend(source.session_requests);
    target
        .client_control_frames
        .extend(source.client_control_frames);
    target.session_events.extend(source.session_events);
    target.observations.extend(source.observations);
}

fn suppress_attach_terminal_output(
    output: &mut BotsterEngineOutput,
    session_id: &SessionId,
    attach: &IncrementalAttach,
) {
    output.client_egress.retain(|(routed_client, frame)| {
        let TransportEgress::TerminalOutput {
            session_id: routed_session,
            subscription_id: routed_subscription,
            ..
        } = frame
        else {
            return true;
        };
        if routed_session != session_id {
            return true;
        }
        let active =
            routed_client == &attach.client_id && routed_subscription == &attach.subscription_id;
        let pending = attach.pending.iter().any(|(client, subscription)| {
            routed_client == client && routed_subscription == subscription
        });
        !active && !pending
    });
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
///         dependencies: Vec::new(),
///         features: Vec::new(),
///         host_profile: None,
///         configuration: None,
///         runnable_entrypoints: Vec::new(),
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

    /// Admit one plugin invocation without waiting for execution or completion.
    pub fn try_admit_plugin(
        &self,
        class: PluginInvocationClass,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        self.multiplexer.try_admit_plugin(class, request)
    }

    /// Drain previously published async plugin completions without waiting.
    pub fn drain_plugin_completions(
        &self,
        max_items: usize,
        max_bytes: usize,
    ) -> PluginCompletionDrain {
        self.multiplexer
            .drain_plugin_completions(max_items, max_bytes)
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

#[cfg(all(test, unix, feature = "local-runtime"))]
mod takeover_fail_closed_tests;
