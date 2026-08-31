//! Scheduling-neutral managed session runtime over core engine primitives.

use std::cell::RefCell;
use std::collections::HashSet;
use std::error::Error;
use std::rc::Rc;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::contract::actor::SessionLifecycleState;
use crate::contract::actor::{
    MailboxSendFailureReason, ModeFlagsReady, PreparedSnapshotReady, PreparedSnapshotRequest,
    QueueSource, ScreenReady, SendFileFailed, SendFileRequest, SendFileWritten, SessionIoRequest,
    SnapshotReady,
};
use crate::contract::terminal_adapter::TerminalAdapter;
use crate::contract::terminal_subscription::{
    BindTerminalAdapterError, DetachTerminalSubscriptionResult, TerminalCapabilitySet,
    TerminalInputDelivery, TerminalSubscriptionGeneration, TerminalSubscriptionRecord,
};
use crate::contract::terminal_wake::{
    TerminalWakeBatch, TerminalWakeSource, WakingTerminalAdapter,
};
use crate::engine::client_worker::{ClientWorker, OwnerKey};
use crate::engine::command::EngineSessionInspection;
use crate::engine::multiplexer::{
    MultiplexerEngine, MultiplexerEngineError, MultiplexerEngineObservation,
    MultiplexerEngineOutcome, MultiplexerSpawnOutcome,
};
use crate::engine::session_worker::SessionWorkerRuntime;
use crate::engine::terminal_screen::{
    PlainTerminalScreenRuntime, TerminalScreenEngine, TerminalScreenRuntime,
};
#[cfg(feature = "local-runtime")]
use crate::runtime::ProcessIdentity;
#[cfg(feature = "local-runtime")]
use crate::runtime::{
    ControlAdmission, ControlPlaneState, GatedPoll, LocalProcessRuntime, WorkerProcessRuntime,
    WorkerProcessRuntimeOptions, DEFAULT_MODE_GATED_INPUT_TIMEOUT,
};
use crate::runtime::{
    SessionRuntime, SessionRuntimeError, SessionRuntimeErrorKind, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest,
};
use crate::session::{
    CoreSessionMetadata, RequestId, SessionActivityStatus, SessionId, SubscriptionId,
};
use crate::session_protocol::{
    ModeFlags, ModeFreshnessToken, ModeGatedPtyInputResult, ResizePayload, TerminalColorProfile,
};
use crate::terminal_screen::{
    TerminalBackendError, TerminalScreenSize, TerminalScreenState, TerminalSnapshotPayload,
};
use crate::transport::TransportIngress;
use crate::ClientId;
use botster_terminal_protocol_client::{
    TerminalInputCommand, TerminalInputKind, TerminalInputRejection, TerminalInputResult,
    TerminalModeFlags,
};

/// Host-visible error from managed session runtime coordination.
#[derive(Debug, Error)]
pub enum ManagedSessionRuntimeError {
    /// The assembled multiplexer rejected the operation.
    #[error(transparent)]
    Multiplexer(#[from] MultiplexerEngineError),
    /// The host session runtime rejected input or output work.
    #[error(transparent)]
    Runtime(#[from] SessionRuntimeError),
    /// The managed runtime cannot produce a terminal-state response.
    #[error("managed session runtime does not support {request_kind}")]
    UnsupportedSessionRequest {
        /// Stable request kind that requires host-owned terminal state.
        request_kind: &'static str,
    },
    /// A host-supplied terminal backend could not be constructed.
    #[error("managed session runtime could not construct terminal backend")]
    TerminalBackendConstruction {
        /// Backend construction failure from the host adapter.
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    /// A host-supplied terminal backend reported an operation failure.
    #[error("managed session terminal backend failed during {operation}: {message}")]
    TerminalBackendOperation {
        /// Backend operation that reported the failure.
        operation: &'static str,
        /// Backend-owned error message.
        message: String,
    },
}

type TerminalBackendFactory<T> =
    Rc<dyn Fn(TerminalScreenSize) -> Result<T, Box<dyn Error + Send + Sync>>>;

#[cfg(feature = "local-runtime")]
#[derive(Clone, Copy)]
enum DeliveryApply {
    Global,
    Targeted,
}

/// Scheduling-neutral coordinator for one or more managed live sessions.
///
/// Hosts still choose the executor, thread, or event loop that calls these
/// methods. This type defines the reusable semantics for routing client writes
/// into `SessionRuntimeInput` and draining runtime output through the existing
/// session worker and subscription multiplexer path. Terminal snapshot and
/// screen reads come from core-owned state updated by drained runtime output.
/// Bound-adapter egress is owned by the embedded [`ClientWorker`].
pub struct ManagedSessionRuntime<R, T = PlainTerminalScreenRuntime>
where
    R: SessionRuntime,
    T: TerminalScreenRuntime,
{
    engine: MultiplexerEngine<R, SessionRuntimeWorkerAdapter<T>>,
    terminal_backend_factory: TerminalBackendFactory<T>,
    client_worker: ClientWorker,
    wake_source: TerminalWakeSource,
    pending_input_teardowns: Vec<crate::engine::client_worker::ClientWorkerTeardown>,
}

impl<R> ManagedSessionRuntime<R, PlainTerminalScreenRuntime>
where
    R: SessionRuntime,
{
    /// Build a managed runtime around a host session runtime.
    #[must_use]
    pub fn new(runtime: R) -> Self {
        Self::with_terminal_backend_factory(runtime, |size| {
            Ok::<_, std::convert::Infallible>(PlainTerminalScreenRuntime::new(size))
        })
    }
}

#[cfg(feature = "local-runtime")]
impl ManagedSessionRuntime<WorkerProcessRuntime, PlainTerminalScreenRuntime> {
    /// Build a worker-process managed runtime with the plain terminal backend.
    ///
    /// First-party production hosts that want a concrete terminal backend should use
    /// a host profile such as `botster-core-daemon`'s default feature path or call
    /// [`ManagedSessionRuntime::with_terminal_backend_factory`] directly.
    #[must_use]
    pub fn with_worker_process(worker_path: impl Into<std::path::PathBuf>) -> Self {
        Self::new(WorkerProcessRuntime::new(worker_path))
    }

    /// Build a worker-process managed runtime with explicit options and the plain
    /// terminal backend.
    ///
    /// First-party production hosts that want a concrete terminal backend should use
    /// a host profile such as `botster-core-daemon`'s default feature path or call
    /// [`ManagedSessionRuntime::with_terminal_backend_factory`] directly.
    #[must_use]
    pub fn with_worker_process_options(options: WorkerProcessRuntimeOptions) -> Self {
        Self::new(WorkerProcessRuntime::with_options(options))
    }
}

#[cfg(feature = "local-runtime")]
impl<T> ManagedSessionRuntime<WorkerProcessRuntime, T>
where
    T: TerminalScreenRuntime + 'static,
{
    /// Synchronize the parent terminal with one atomic worker snapshot boundary.
    pub fn synchronize_worker_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<(MultiplexerEngineOutcome, TerminalSnapshotPayload), ManagedSessionRuntimeError>
    {
        let (snapshot, output) = self
            .engine
            .session_runtime_mut()
            .capture_snapshot_boundary(session_id)?;
        let mut outcome = self.route_runtime_outputs(session_id, output, last_output_at)?;
        let worker = self.engine_worker(session_id).ok_or_else(|| {
            MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            }
        })?;
        worker.replay_snapshot(snapshot.clone())?;
        self.route_pending_runtime_events(&mut outcome)?;
        Ok((outcome, snapshot))
    }

    /// Return whether one worker supports atomic snapshot boundaries.
    pub fn worker_supports_snapshot_boundary(
        &self,
        session_id: &SessionId,
    ) -> Result<bool, ManagedSessionRuntimeError> {
        Ok(self
            .engine
            .session_runtime()
            .supports_snapshot_boundary(session_id)?)
    }

    /// Adopt a live worker process through its reopenable control endpoint.
    pub fn adopt_worker_process(
        &mut self,
        session_id: SessionId,
        process: ProcessIdentity,
        socket_path: impl Into<std::path::PathBuf>,
        supports_snapshot_boundary: bool,
        metadata: CoreSessionMetadata,
    ) -> Result<MultiplexerSpawnOutcome, ManagedSessionRuntimeError> {
        if !metadata.is_within_encoded_len_limit() {
            return Err(ManagedSessionRuntimeError::Multiplexer(
                MultiplexerEngineError::MetadataTooLarge,
            ));
        }
        let handle = self.engine.session_runtime_mut().adopt_session(
            session_id,
            process,
            socket_path,
            supports_snapshot_boundary,
        )?;
        let terminal = (self.terminal_backend_factory)(TerminalScreenSize::new(24, 80))
            .map_err(|source| ManagedSessionRuntimeError::TerminalBackendConstruction { source })?;
        Ok(self.engine.adopt_session(
            handle,
            metadata,
            SessionRuntimeWorkerAdapter::new(terminal),
        )?)
    }

    /// Release worker processes for an intentional daemon restart.
    pub fn release_workers_for_restart(&mut self) {
        self.engine.session_runtime_mut().release_for_restart();
    }

    /// Durable control-plane state for one worker session.
    #[must_use]
    pub fn control_plane_state(&self, session_id: &SessionId) -> ControlPlaneState {
        self.engine
            .session_runtime()
            .control_plane_state(session_id)
    }

    /// Sweep the writer and intake frames without applying or pumping output.
    ///
    /// Incremental attach owns the only `pump_session_output` on that tick.
    /// A second pump here can dequeue two snapshot pages and emit both.
    pub fn prepare_terminal_input(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), ManagedSessionRuntimeError> {
        if let Some(error) = self
            .engine
            .session_runtime()
            .consume_control_writer_failure(session_id)
        {
            self.engine
                .session_runtime_mut()
                .mark_control_plane_failed(session_id, error);
            let teardowns = self.client_worker.teardown_session(session_id);
            self.retain_input_teardowns(session_id, teardowns);
            return Ok(());
        }
        let teardowns = self.client_worker.intake_terminal_input();
        self.retain_input_teardowns(session_id, teardowns);
        Ok(())
    }

    /// Apply adapter ingress at the top of the production tick.
    ///
    /// Returns `Ok` when the tick machinery completed. Per-command failures
    /// stay owner-scoped and do not abort the shared drain.
    pub fn apply_terminal_input(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<(), ManagedSessionRuntimeError> {
        if let Some(error) = self
            .engine
            .session_runtime()
            .consume_control_writer_failure(session_id)
        {
            self.engine
                .session_runtime_mut()
                .mark_control_plane_failed(session_id, error);
            let teardowns = self.client_worker.teardown_session(session_id);
            self.retain_input_teardowns(session_id, teardowns);
            return Ok(());
        }

        let mut teardowns = Vec::new();
        match self
            .engine
            .session_runtime_mut()
            .poll_mode_gated_pty_input(session_id)
        {
            Ok(GatedPoll::Ready(result)) => {
                if let Some(teardown) = self.complete_gated_result(
                    session_id,
                    TerminalInputKind::ModeGatedInput,
                    result,
                ) {
                    teardowns.push(teardown);
                }
            }
            Ok(GatedPoll::TimedOut) => {
                if let Some(teardown) = self.complete_gated_timeout(session_id) {
                    teardowns.push(teardown);
                }
            }
            Ok(GatedPoll::Idle | GatedPoll::Pending) => {}
            Err(_) => {}
        }

        teardowns.extend(self.client_worker.intake_terminal_input());

        let mut holding = self.engine.session_runtime().sessions_holding_gated();
        holding.extend(self.client_worker.sessions_awaiting_gated());
        let deliveries = self.client_worker.take_terminal_input(&holding);
        for delivery in deliveries {
            match self.apply_one_delivery(delivery, last_output_at, DeliveryApply::Global) {
                Ok(_) => {}
                Err(teardown) => teardowns.push(teardown),
            }
        }
        self.retain_input_teardowns(session_id, teardowns);
        Ok(())
    }

    pub(crate) fn apply_woken_terminal_input(
        &mut self,
        batch: &TerminalWakeBatch,
        last_output_at: u64,
        deferred_sessions: &HashSet<SessionId>,
        outcome: &mut MultiplexerEngineOutcome,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let named_sessions: HashSet<_> = batch
            .adapter_routes
            .iter()
            .map(|route| route.session_id.clone())
            .chain(batch.ingress_sessions.iter().cloned())
            .collect();
        let mut teardowns = Vec::new();
        let mut failed_sessions = HashSet::new();

        for session_id in &named_sessions {
            if let Some(error) = self
                .engine
                .session_runtime()
                .consume_control_writer_failure(session_id)
            {
                self.engine
                    .session_runtime_mut()
                    .mark_control_plane_failed(session_id, error);
                teardowns.extend(self.client_worker.teardown_session(session_id));
                failed_sessions.insert(session_id.clone());
            }
        }

        let awaiting = self.client_worker.sessions_awaiting_gated();
        for session_id in named_sessions.intersection(&awaiting) {
            if failed_sessions.contains(session_id) {
                continue;
            }
            match self
                .engine
                .session_runtime_mut()
                .poll_mode_gated_pty_input(session_id)
            {
                Ok(GatedPoll::Ready(result)) => {
                    if let Some(teardown) = self.complete_gated_result(
                        session_id,
                        TerminalInputKind::ModeGatedInput,
                        result,
                    ) {
                        teardowns.push(teardown);
                    }
                }
                Ok(GatedPoll::TimedOut) => {
                    if let Some(teardown) = self.complete_gated_timeout(session_id) {
                        teardowns.push(teardown);
                    }
                }
                Ok(GatedPoll::Idle | GatedPoll::Pending) | Err(_) => {}
            }
        }

        let mut keys = self.client_worker.adapter_route_keys(batch);
        keys.extend(self.client_worker.parked_route_keys(batch));
        let mut seen = HashSet::new();
        keys.retain(|key| seen.insert(key.clone()));
        let mut held = self.engine.session_runtime().sessions_holding_gated();
        held.extend(self.client_worker.sessions_awaiting_gated());
        let mut full_sessions = HashSet::new();

        for key in keys {
            if failed_sessions.contains(&key.session_id) || full_sessions.contains(&key.session_id)
            {
                continue;
            }
            if !self.client_worker.has_terminal_input(&key) {
                self.client_worker.clear_capacity_parked(&key);
                continue;
            }
            if deferred_sessions.contains(&key.session_id) {
                self.client_worker.park_for_capacity(&key);
                continue;
            }
            for _ in 0..crate::engine::client_worker::APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK {
                match self
                    .engine
                    .session_runtime()
                    .probe_ordinary(&key.session_id)
                {
                    ControlAdmission::Ready => {
                        let Some(delivery) =
                            self.client_worker.take_one_terminal_input(&key, &mut held)
                        else {
                            self.client_worker.clear_capacity_parked(&key);
                            break;
                        };
                        match self.apply_one_delivery(
                            delivery,
                            last_output_at,
                            DeliveryApply::Targeted,
                        ) {
                            Ok(Some(step)) => append_outcome(outcome, step),
                            Ok(None) => {}
                            Err(teardown) => {
                                teardowns.push(teardown);
                                break;
                            }
                        }
                    }
                    ControlAdmission::Full => {
                        self.client_worker.park_for_capacity(&key);
                        full_sessions.insert(key.session_id.clone());
                        break;
                    }
                    ControlAdmission::Sealed => {
                        if let Some(teardown) = self.client_worker.hard_stop_owner(&key) {
                            teardowns.push(teardown);
                        }
                        break;
                    }
                }
            }
        }
        self.pending_input_teardowns.extend(teardowns);
        Ok(())
    }

    fn apply_one_delivery(
        &mut self,
        delivery: TerminalInputDelivery,
        last_output_at: u64,
        apply: DeliveryApply,
    ) -> Result<Option<MultiplexerEngineOutcome>, crate::engine::client_worker::ClientWorkerTeardown>
    {
        let kind = match &delivery.command {
            TerminalInputCommand::Input { .. } => TerminalInputKind::Input,
            TerminalInputCommand::ModeGatedInput { .. } => TerminalInputKind::ModeGatedInput,
            TerminalInputCommand::Resize { .. } => TerminalInputKind::Resize,
        };
        let session_id = delivery.session_id.clone();
        let subscription_id = delivery.subscription_id.clone();
        let client_id = delivery.client_id.clone();
        let mut targeted_outcome = None;
        let result = match delivery.command {
            TerminalInputCommand::Input { data } => {
                let ingress = TransportIngress::TerminalInput {
                    session_id: session_id.clone(),
                    data: data.clone(),
                };
                let applied = match apply {
                    DeliveryApply::Global => {
                        self.handle_client_ingress(client_id, ingress, last_output_at)
                    }
                    DeliveryApply::Targeted => {
                        self.apply_targeted_client_ingress(client_id, ingress, last_output_at)
                    }
                };
                match applied {
                    Ok(outcome) => {
                        if matches!(apply, DeliveryApply::Targeted) {
                            targeted_outcome = Some(outcome);
                        }
                        input_result_ok(kind, data.len())
                    }
                    Err(_) => {
                        return match owner_apply_teardown(
                            &mut self.client_worker,
                            &session_id,
                            &subscription_id,
                        ) {
                            Err(teardown) => Err(teardown),
                            Ok(()) => Ok(None),
                        };
                    }
                }
            }
            TerminalInputCommand::Resize { rows, cols } => {
                let ingress = TransportIngress::Resize {
                    session_id: session_id.clone(),
                    rows,
                    cols,
                };
                let applied = match apply {
                    DeliveryApply::Global => {
                        self.handle_client_ingress(client_id, ingress, last_output_at)
                    }
                    DeliveryApply::Targeted => {
                        self.apply_targeted_client_ingress(client_id, ingress, last_output_at)
                    }
                };
                match applied {
                    Ok(outcome) => {
                        if matches!(apply, DeliveryApply::Targeted) {
                            targeted_outcome = Some(outcome);
                        }
                        input_result_ok(kind, 0)
                    }
                    Err(_) => {
                        return match owner_apply_teardown(
                            &mut self.client_worker,
                            &session_id,
                            &subscription_id,
                        ) {
                            Err(teardown) => Err(teardown),
                            Ok(()) => Ok(None),
                        };
                    }
                }
            }
            TerminalInputCommand::ModeGatedInput {
                mode_generation,
                mode_revision,
                data,
            } => {
                match self
                    .engine
                    .session_runtime_mut()
                    .submit_mode_gated_pty_input(
                        &session_id,
                        ModeFreshnessToken {
                            mode_generation,
                            mode_revision,
                        },
                        data,
                    ) {
                    Ok(request_id) => {
                        let deadline = Instant::now()
                            + DEFAULT_MODE_GATED_INPUT_TIMEOUT
                            + Duration::from_secs(1);
                        self.client_worker.set_awaiting_gated(
                            &session_id,
                            &subscription_id,
                            request_id,
                            deadline,
                        );
                        return Ok(None);
                    }
                    Err(error)
                        if error.message.contains("control queue full")
                            || error.message.contains("control plane sealed")
                            || error.message.contains("already in flight") =>
                    {
                        if error.message.contains("already in flight") {
                            input_result_rejected(kind, TerminalInputRejection::SessionNotWritable)
                        } else {
                            return owner_apply_teardown_outcome(
                                &mut self.client_worker,
                                &session_id,
                                &subscription_id,
                            );
                        }
                    }
                    Err(_) => {
                        return owner_apply_teardown_outcome(
                            &mut self.client_worker,
                            &session_id,
                            &subscription_id,
                        );
                    }
                }
            }
        };
        let result = with_subscription(result, &subscription_id);
        if self
            .client_worker
            .enqueue_input_result(&session_id, &subscription_id, &result)
            .is_err()
        {
            return owner_apply_teardown_outcome(
                &mut self.client_worker,
                &session_id,
                &subscription_id,
            );
        }
        Ok(targeted_outcome)
    }

    fn complete_gated_result(
        &mut self,
        session_id: &SessionId,
        kind: TerminalInputKind,
        result: ModeGatedPtyInputResult,
    ) -> Option<crate::engine::client_worker::ClientWorkerTeardown> {
        let (subscription_id, _) = self.take_matching_gated(session_id, &result.request_id)?;
        let mapped = with_subscription(map_gated_result(kind, result), &subscription_id);
        if self
            .client_worker
            .enqueue_input_result(session_id, &subscription_id, &mapped)
            .is_err()
        {
            return self.client_worker.detach_live(session_id, &subscription_id);
        }
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id,
        };
        if self.client_worker.has_terminal_input(&key) {
            self.client_worker.park_for_capacity(&key);
        }
        None
    }

    fn complete_gated_timeout(
        &mut self,
        session_id: &SessionId,
    ) -> Option<crate::engine::client_worker::ClientWorkerTeardown> {
        let (subscription_id, _) = self.take_any_gated(session_id)?;
        let mapped = with_subscription(
            input_result_rejected(
                TerminalInputKind::ModeGatedInput,
                TerminalInputRejection::Timeout,
            ),
            &subscription_id,
        );
        if self
            .client_worker
            .enqueue_input_result(session_id, &subscription_id, &mapped)
            .is_err()
        {
            return self.client_worker.detach_live(session_id, &subscription_id);
        }
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id,
        };
        if self.client_worker.has_terminal_input(&key) {
            self.client_worker.park_for_capacity(&key);
        }
        None
    }

    fn take_matching_gated(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> Option<(SubscriptionId, crate::engine::client_worker::GatedWait)> {
        let records = self.client_worker.list_terminal_subscriptions();
        for record in records {
            if &record.session_id != session_id {
                continue;
            }
            if self
                .client_worker
                .awaiting_gated(session_id, &record.subscription_id)
                .is_some_and(|wait| wait.request_id == request_id)
            {
                let wait = self
                    .client_worker
                    .clear_awaiting_gated(session_id, &record.subscription_id)?;
                return Some((record.subscription_id, wait));
            }
        }
        None
    }

    fn take_any_gated(
        &mut self,
        session_id: &SessionId,
    ) -> Option<(SubscriptionId, crate::engine::client_worker::GatedWait)> {
        let records = self.client_worker.list_terminal_subscriptions();
        for record in records {
            if &record.session_id != session_id {
                continue;
            }
            if let Some(wait) = self
                .client_worker
                .clear_awaiting_gated(session_id, &record.subscription_id)
            {
                return Some((record.subscription_id, wait));
            }
        }
        None
    }

    fn retain_input_teardowns(
        &mut self,
        _session_id: &SessionId,
        teardowns: Vec<crate::engine::client_worker::ClientWorkerTeardown>,
    ) {
        self.pending_input_teardowns.extend(teardowns);
    }
}

#[cfg(feature = "local-runtime")]
impl<T> ManagedSessionRuntime<LocalProcessRuntime, T>
where
    T: TerminalScreenRuntime + 'static,
{
    /// Apply adapter ingress for the in-process local runtime.
    pub fn apply_terminal_input(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let _ = session_id;
        let mut teardowns = self.client_worker.intake_terminal_input();
        let deliveries = self.client_worker.take_terminal_input(&HashSet::new());
        for delivery in deliveries {
            match self.apply_one_local_delivery(delivery, last_output_at) {
                Ok(()) => {}
                Err(teardown) => teardowns.push(teardown),
            }
        }
        self.pending_input_teardowns.extend(teardowns);
        Ok(())
    }

    pub(crate) fn apply_woken_terminal_input(
        &mut self,
        batch: &TerminalWakeBatch,
        last_output_at: u64,
        outcome: &mut MultiplexerEngineOutcome,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let mut teardowns = Vec::new();
        let mut held = HashSet::new();
        for key in self.client_worker.adapter_route_keys(batch) {
            for _ in 0..crate::engine::client_worker::APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK {
                let Some(delivery) = self.client_worker.take_one_terminal_input(&key, &mut held)
                else {
                    break;
                };
                match self.apply_one_local_delivery_targeted(delivery, last_output_at) {
                    Ok(step) => append_outcome(outcome, step),
                    Err(teardown) => {
                        teardowns.push(teardown);
                        break;
                    }
                }
            }
        }
        self.pending_input_teardowns.extend(teardowns);
        Ok(())
    }

    fn apply_one_local_delivery_targeted(
        &mut self,
        delivery: TerminalInputDelivery,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, crate::engine::client_worker::ClientWorkerTeardown> {
        let session_id = delivery.session_id.clone();
        let subscription_id = delivery.subscription_id.clone();
        let client_id = delivery.client_id;
        let (ingress, result) = match delivery.command {
            TerminalInputCommand::Input { data } => (
                TransportIngress::TerminalInput {
                    session_id: session_id.clone(),
                    data: data.clone(),
                },
                input_result_ok(TerminalInputKind::Input, data.len()),
            ),
            TerminalInputCommand::Resize { rows, cols } => (
                TransportIngress::Resize {
                    session_id: session_id.clone(),
                    rows,
                    cols,
                },
                input_result_ok(TerminalInputKind::Resize, 0),
            ),
            TerminalInputCommand::ModeGatedInput { .. } => {
                let result = with_subscription(
                    input_result_rejected(
                        TerminalInputKind::ModeGatedInput,
                        TerminalInputRejection::SessionNotWritable,
                    ),
                    &subscription_id,
                );
                if self
                    .client_worker
                    .enqueue_input_result(&session_id, &subscription_id, &result)
                    .is_err()
                {
                    return self
                        .client_worker
                        .detach_live(&session_id, &subscription_id)
                        .map_or_else(|| Ok(MultiplexerEngineOutcome::empty()), Err);
                }
                return Ok(MultiplexerEngineOutcome::empty());
            }
        };
        let outcome = match self.apply_targeted_client_ingress(client_id, ingress, last_output_at) {
            Ok(outcome) => outcome,
            Err(_) => {
                return self
                    .client_worker
                    .detach_live(&session_id, &subscription_id)
                    .map_or_else(|| Ok(MultiplexerEngineOutcome::empty()), Err);
            }
        };
        let result = with_subscription(result, &subscription_id);
        if self
            .client_worker
            .enqueue_input_result(&session_id, &subscription_id, &result)
            .is_err()
        {
            return self
                .client_worker
                .detach_live(&session_id, &subscription_id)
                .map_or_else(|| Ok(MultiplexerEngineOutcome::empty()), Err);
        }
        Ok(outcome)
    }

    fn apply_one_local_delivery(
        &mut self,
        delivery: crate::contract::terminal_subscription::TerminalInputDelivery,
        last_output_at: u64,
    ) -> Result<(), crate::engine::client_worker::ClientWorkerTeardown> {
        let session_id = delivery.session_id.clone();
        let subscription_id = delivery.subscription_id.clone();
        let client_id = delivery.client_id.clone();
        let result = match delivery.command {
            TerminalInputCommand::Input { data } => {
                match self.handle_client_ingress(
                    client_id,
                    TransportIngress::TerminalInput {
                        session_id: session_id.clone(),
                        data: data.clone(),
                    },
                    last_output_at,
                ) {
                    Ok(_) => input_result_ok(TerminalInputKind::Input, data.len()),
                    Err(_) => {
                        return owner_apply_teardown(
                            &mut self.client_worker,
                            &session_id,
                            &subscription_id,
                        );
                    }
                }
            }
            TerminalInputCommand::Resize { rows, cols } => {
                match self.handle_client_ingress(
                    client_id,
                    TransportIngress::Resize {
                        session_id: session_id.clone(),
                        rows,
                        cols,
                    },
                    last_output_at,
                ) {
                    Ok(_) => input_result_ok(TerminalInputKind::Resize, 0),
                    Err(_) => {
                        return owner_apply_teardown(
                            &mut self.client_worker,
                            &session_id,
                            &subscription_id,
                        );
                    }
                }
            }
            TerminalInputCommand::ModeGatedInput { .. } => input_result_rejected(
                TerminalInputKind::ModeGatedInput,
                TerminalInputRejection::SessionNotWritable,
            ),
        };
        let result = with_subscription(result, &subscription_id);
        if self
            .client_worker
            .enqueue_input_result(&session_id, &subscription_id, &result)
            .is_err()
        {
            return owner_apply_teardown(&mut self.client_worker, &session_id, &subscription_id);
        }
        Ok(())
    }
}

impl<R, T> ManagedSessionRuntime<R, T>
where
    R: SessionRuntime,
    T: TerminalScreenRuntime + 'static,
{
    /// Build a managed runtime with a host-supplied terminal backend factory.
    ///
    /// The factory is called once per spawned session with that session's
    /// initial PTY size, or the managed runtime's default terminal size.
    pub fn with_terminal_backend_factory<E, F>(runtime: R, factory: F) -> Self
    where
        E: Error + Send + Sync + 'static,
        F: Fn(TerminalScreenSize) -> Result<T, E> + 'static,
    {
        let wake_source = TerminalWakeSource::new();
        let mut client_worker = ClientWorker::new();
        client_worker.set_wake_source(wake_source.clone());
        Self {
            engine: MultiplexerEngine::new(runtime),
            terminal_backend_factory: Rc::new(move |size| {
                factory(size).map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)
            }),
            client_worker,
            wake_source,
            pending_input_teardowns: Vec::new(),
        }
    }

    /// Share one wake source with the session runtime and ClientWorker.
    #[must_use]
    pub fn with_shared_wake_source(mut self, source: TerminalWakeSource) -> Self {
        self.client_worker.set_wake_source(source.clone());
        self.wake_source = source;
        self
    }

    /// Host wait source for adapter and ingress wakes.
    #[must_use]
    pub fn wake_source(&self) -> &TerminalWakeSource {
        &self.wake_source
    }

    /// Return a recorded session from the assembled core engine.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&crate::CoreSession> {
        self.engine.session(session_id)
    }

    /// Return sessions currently recorded by the managed engine.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<crate::CoreSession> {
        self.engine.list_sessions()
    }

    /// Forget all managed engine state for one terminal session.
    pub fn forget_terminal_session(&mut self, session_id: &SessionId) -> bool {
        self.wake_source.forget_session(session_id);
        self.pending_input_teardowns
            .extend(self.client_worker.teardown_session(session_id));
        let mut outcome = MultiplexerEngineOutcome::empty();
        let _ = self.apply_client_worker(&mut outcome);
        self.engine.forget_terminal_session(session_id)
    }

    /// Return the host session runtime adapter.
    #[must_use]
    pub const fn session_runtime(&self) -> &R {
        self.engine.session_runtime()
    }

    /// Return a mutable host session runtime adapter.
    pub const fn session_runtime_mut(&mut self) -> &mut R {
        self.engine.session_runtime_mut()
    }

    /// Spawn a session and install a runtime-backed session worker adapter.
    pub fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
    ) -> Result<MultiplexerSpawnOutcome, ManagedSessionRuntimeError> {
        let size = request
            .initial_pty_size
            .as_ref()
            .map(|size| TerminalScreenSize::new(size.rows, size.cols))
            .unwrap_or_else(|| TerminalScreenSize::new(24, 80));
        let terminal = (self.terminal_backend_factory)(size)
            .map_err(|source| ManagedSessionRuntimeError::TerminalBackendConstruction { source })?;

        Ok(self.engine.spawn_session(
            request,
            metadata,
            SessionRuntimeWorkerAdapter::new(terminal),
        )?)
    }

    /// Route one client ingress frame through the existing multiplexer path.
    pub fn handle_client_ingress(
        &mut self,
        client_id: ClientId,
        ingress: TransportIngress,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        reject_unsupported_ingress(&ingress)?;
        let backend_operation = terminal_backend_ingress_operation(&ingress);
        let mut outcome =
            match self
                .engine
                .handle_client_ingress(client_id.clone(), ingress.clone(), now_seconds)
            {
                Ok(outcome) => outcome,
                Err(error) => {
                    if let Some((session_id, operation)) = backend_operation {
                        self.ensure_terminal_backend_ok(&session_id, operation)?;
                    }
                    return Err(error.into());
                }
            };
        let mut extra_teardowns = Vec::new();
        if let TransportIngress::SubscribeSession {
            client_id: ref subscribe_client,
            ref session_id,
            ref subscription_id,
        } = ingress
        {
            let (_, replacements) = self.client_worker.record_attach(
                subscribe_client.clone(),
                session_id.clone(),
                subscription_id.clone(),
            );
            extra_teardowns.extend(replacements);
        }
        if let TransportIngress::UnsubscribeSession {
            session_id,
            subscription_id,
            ..
        } = &ingress
        {
            extra_teardowns.extend(self.client_worker.detach_live(session_id, subscription_id));
        }
        self.flush_runtime_inputs()?;
        self.apply_client_worker_with(&mut outcome, extra_teardowns)?;
        Ok(outcome)
    }

    fn apply_targeted_client_ingress(
        &mut self,
        client_id: ClientId,
        ingress: TransportIngress,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        reject_unsupported_ingress(&ingress)?;
        let session_id = match &ingress {
            TransportIngress::TerminalInput { session_id, .. }
            | TransportIngress::Resize { session_id, .. } => session_id.clone(),
            _ => {
                return Err(ManagedSessionRuntimeError::UnsupportedSessionRequest {
                    request_kind: "targeted terminal ingress",
                })
            }
        };
        let backend_operation = terminal_backend_ingress_operation(&ingress);
        let outcome = match self
            .engine
            .handle_client_ingress(client_id, ingress, now_seconds)
        {
            Ok(outcome) => outcome,
            Err(error) => {
                if let Some((backend_session_id, operation)) = backend_operation {
                    self.ensure_terminal_backend_ok(&backend_session_id, operation)?;
                }
                return Err(error.into());
            }
        };
        self.flush_runtime_inputs_for_session(&session_id)?;
        Ok(outcome)
    }

    pub(crate) fn attach_snapshot(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        snapshot: Vec<u8>,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut outcome = self.engine.attach_snapshot(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            snapshot,
        )?;
        let (_, replacements) =
            self.client_worker
                .record_attach(client_id, session_id, subscription_id);
        self.apply_client_worker_with(&mut outcome, replacements)?;
        Ok(outcome)
    }

    pub(crate) fn begin_snapshot_attach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut outcome = self.engine.begin_snapshot_attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )?;
        let (_, replacements) =
            self.client_worker
                .record_attach(client_id, session_id, subscription_id);
        self.apply_client_worker_with(&mut outcome, replacements)?;
        Ok(outcome)
    }

    pub(crate) fn snapshot_attach_frame(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        data: Vec<u8>,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut outcome =
            self.engine
                .snapshot_attach_frame(client_id, session_id, subscription_id, data)?;
        self.apply_client_worker(&mut outcome)?;
        Ok(outcome)
    }

    pub(crate) fn complete_snapshot_attach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        history_incomplete: bool,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut outcome = self.engine.complete_snapshot_attach(
            client_id,
            session_id,
            subscription_id,
            history_incomplete,
        )?;
        self.apply_client_worker(&mut outcome)?;
        Ok(outcome)
    }

    pub(crate) fn note_snapshot_phase(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        phase: crate::WorkerSnapshotPhase,
    ) {
        self.client_worker
            .note_snapshot_phase(session_id, subscription_id, phase);
    }

    /// Record that the next attach for this identity will bind an adapter.
    pub fn expect_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        self.client_worker
            .expect_terminal_adapter(client_id, session_id, subscription_id);
    }

    /// Retire an unconsumed pre-attach adapter declaration.
    pub fn cancel_expected_terminal_adapter(
        &mut self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        self.client_worker
            .cancel_expected_terminal_adapter(client_id, session_id, subscription_id);
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
        self.client_worker.bind_terminal_adapter(
            &client_id,
            session_id,
            subscription_id,
            generation,
            capabilities,
            adapter,
        )
    }

    /// Bind a waking adapter. Allocates wake state only after rejection checks pass.
    pub fn bind_waking_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        adapter: Box<dyn WakingTerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        self.client_worker.bind_waking_terminal_adapter(
            &client_id,
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
        self.client_worker.list_terminal_subscriptions()
    }

    /// Detach the live generation if present.
    pub fn detach_live_subscription(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        self.handle_client_ingress(
            client_id.clone(),
            TransportIngress::UnsubscribeSession {
                client_id,
                session_id,
                subscription_id,
            },
            now_seconds,
        )
    }

    /// Generation-aware detach.
    pub fn detach_terminal_subscription(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        now_seconds: u64,
    ) -> Result<
        (DetachTerminalSubscriptionResult, MultiplexerEngineOutcome),
        ManagedSessionRuntimeError,
    > {
        let result = match self
            .client_worker
            .live_generation(&session_id, &subscription_id)
        {
            None => DetachTerminalSubscriptionResult::AlreadyGone,
            Some(live) if live != generation => {
                DetachTerminalSubscriptionResult::GenerationMismatch {
                    live,
                    requested: generation,
                }
            }
            Some(live) => DetachTerminalSubscriptionResult::Detached { generation: live },
        };
        let outcome = match result {
            DetachTerminalSubscriptionResult::Detached { .. } => self.handle_client_ingress(
                client_id.clone(),
                TransportIngress::UnsubscribeSession {
                    client_id,
                    session_id,
                    subscription_id,
                },
                now_seconds,
            )?,
            _ => MultiplexerEngineOutcome::empty(),
        };
        Ok((result, outcome))
    }

    /// Whether this subscription still has a live inventory row.
    #[must_use]
    pub fn has_terminal_subscription(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.client_worker
            .has_subscription(session_id, subscription_id)
    }

    /// Whether the live inventory owner is exactly this client and subscription.
    #[must_use]
    pub fn terminal_subscription_matches(
        &self,
        session_id: &SessionId,
        client_id: &ClientId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.client_worker
            .list_terminal_subscriptions()
            .iter()
            .any(|row| {
                &row.session_id == session_id
                    && &row.client_id == client_id
                    && &row.subscription_id == subscription_id
            })
    }

    /// Live generation for a subscription, if any.
    #[must_use]
    pub fn terminal_subscription_generation(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<TerminalSubscriptionGeneration> {
        self.client_worker
            .live_generation(session_id, subscription_id)
    }

    /// Whether a bound adapter is still held.
    #[must_use]
    pub fn adapter_is_bound(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.client_worker
            .adapter_is_bound(session_id, subscription_id)
    }

    pub(crate) fn capture_parent_snapshot(
        &mut self,
        session_id: &SessionId,
    ) -> Result<TerminalSnapshotPayload, ManagedSessionRuntimeError> {
        self.engine_worker(session_id)
            .ok_or_else(|| MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            })?
            .capture_snapshot_payload()
    }

    /// Route one session I/O request through the existing session worker path.
    pub fn handle_session_request(
        &mut self,
        request: SessionIoRequest,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        self.handle_session_request_with(request, now_seconds, true)
    }

    pub(crate) fn handle_session_request_with(
        &mut self,
        request: SessionIoRequest,
        now_seconds: u64,
        pump_bound: bool,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        reject_unsupported_session_request(&request)?;
        if let SessionIoRequest::GetModeFlags { session_id, .. } = &request {
            let worker = self
                .engine
                .session_worker_runtime_mut(session_id)
                .ok_or_else(|| MultiplexerEngineError::UnknownSession {
                    session_id: session_id.clone(),
                })?;
            worker.prepare_mode_flags()?;
        }
        if let SessionIoRequest::SetColorProfile {
            session_id,
            color_profile,
        } = &request
        {
            let worker = self
                .engine
                .session_worker_runtime_mut(session_id)
                .ok_or_else(|| MultiplexerEngineError::UnknownSession {
                    session_id: session_id.clone(),
                })?;
            worker.prepare_color_profile(color_profile.clone())?;
        }
        let mut outcome = self.engine.handle_session_request(request, now_seconds)?;
        self.flush_runtime_inputs()?;
        if pump_bound {
            self.apply_client_worker(&mut outcome)?;
        }
        Ok(outcome)
    }

    /// Report client-side backpressure through the managed engine path.
    pub fn report_backpressure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        Ok(self
            .engine
            .report_backpressure(client_id, session_id, source, capacity, depth)?)
    }

    /// Report accepted-but-slow delivery through the managed engine path.
    pub fn report_delivery_lag(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        capacity: usize,
        depth: usize,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        Ok(self.engine.report_delivery_lag(
            client_id,
            session_id,
            subscription_id,
            source,
            capacity,
            depth,
        )?)
    }

    /// Report a failed delivery attempt through the managed engine path.
    pub fn report_delivery_failure(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        source: QueueSource,
        reason: MailboxSendFailureReason,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        Ok(self.engine.report_delivery_failure(
            client_id,
            session_id,
            subscription_id,
            source,
            reason,
        )?)
    }

    /// Drain currently available runtime output once for a session.
    pub fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        self.drain_runtime_once_with(session_id, last_output_at, true)
    }

    /// Drain runtime output without pumping bound adapters (readback path).
    pub fn drain_runtime_once_without_pump(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        self.drain_runtime_once_with(session_id, last_output_at, false)
    }

    fn drain_runtime_once_with(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
        pump: bool,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut outcome = match self.drain_runtime_output_for_session(session_id, last_output_at) {
            Ok(outcome) => outcome,
            Err(ManagedSessionRuntimeError::Runtime(error))
                if error.kind == SessionRuntimeErrorKind::SessionNotFound
                    && self.session_exited(session_id) =>
            {
                MultiplexerEngineOutcome::empty()
            }
            Err(error) => return Err(error),
        };
        self.route_pending_runtime_events(&mut outcome)?;
        if pump {
            self.apply_client_worker(&mut outcome)?;
        }

        Ok(outcome)
    }

    /// Drain currently available runtime output once for every live session.
    ///
    /// One call is one host scheduling tick: each currently recorded session is
    /// attempted at most once, then pending worker runtime events are routed
    /// once for the whole aggregate pass.
    pub fn drain_runtime_all_once(
        &mut self,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let session_ids = self.engine_session_ids();
        let mut outcome = MultiplexerEngineOutcome::empty();

        for session_id in session_ids {
            match self.drain_runtime_output_for_session(&session_id, last_output_at) {
                Ok(step) => append_outcome(&mut outcome, step),
                Err(ManagedSessionRuntimeError::Runtime(error))
                    if error.kind == SessionRuntimeErrorKind::SessionNotFound
                        && self.session_exited(&session_id) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }

        self.route_pending_runtime_events(&mut outcome)?;
        self.apply_client_worker(&mut outcome)?;

        Ok(outcome)
    }

    fn drain_runtime_output_for_session(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let outputs = self.engine.session_runtime_mut().drain_output(session_id)?;
        self.route_runtime_outputs(session_id, outputs, last_output_at)
    }

    fn route_runtime_outputs(
        &mut self,
        session_id: &SessionId,
        outputs: Vec<SessionRuntimeOutput>,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut outcome = MultiplexerEngineOutcome::empty();

        // Runtime drains are output-only; worker input buffers are populated by
        // request routing paths and are flushed by those mutators.
        for output in outputs {
            let runtime_event = match output {
                SessionRuntimeOutput::PtyOutput { session_id, data } => {
                    if let Some(worker) = self.engine_worker(&session_id) {
                        worker.record_output(&session_id, &data);
                    }
                    crate::SessionWorkerRuntimeEvent::TerminalBytes {
                        session_id,
                        data,
                        last_output_at,
                    }
                }
                SessionRuntimeOutput::ProcessExited {
                    session_id,
                    payload,
                } => crate::SessionWorkerRuntimeEvent::ProcessExited {
                    session_id,
                    payload,
                },
                SessionRuntimeOutput::TitleChanged { session_id, title } => {
                    crate::SessionWorkerRuntimeEvent::TitleChanged { session_id, title }
                }
                SessionRuntimeOutput::CwdChanged { session_id, cwd } => {
                    crate::SessionWorkerRuntimeEvent::CwdChanged { session_id, cwd }
                }
                SessionRuntimeOutput::PromptMark {
                    session_id,
                    payload,
                } => crate::SessionWorkerRuntimeEvent::PromptMark {
                    session_id,
                    payload,
                },
                SessionRuntimeOutput::Bell { session_id } => {
                    crate::SessionWorkerRuntimeEvent::Bell { session_id }
                }
                SessionRuntimeOutput::Notification {
                    session_id,
                    payload,
                } => crate::SessionWorkerRuntimeEvent::Notification {
                    session_id,
                    payload,
                },
                SessionRuntimeOutput::Backpressure(summary) => {
                    outcome
                        .observations
                        .push(MultiplexerEngineObservation::Backpressure(summary));
                    continue;
                }
                SessionRuntimeOutput::MetadataShaping(_) => {
                    continue;
                }
            };
            let step = self.engine.handle_runtime_event(runtime_event)?;
            append_outcome(&mut outcome, step);
        }

        // write_pty replies queued during record_output must reach the child
        // PTY even when no client-facing request mutator flushes inputs.
        self.flush_runtime_inputs_for_session(session_id)?;

        Ok(outcome)
    }

    pub(crate) fn route_worker_boundary_outputs(
        &mut self,
        session_id: &SessionId,
        outputs: Vec<SessionRuntimeOutput>,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut outcome = self.route_runtime_outputs(session_id, outputs, last_output_at)?;
        self.apply_client_worker(&mut outcome)?;
        Ok(outcome)
    }

    /// Pump bound adapters without draining new runtime output.
    pub fn pump_bound_adapters(
        &mut self,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut outcome = MultiplexerEngineOutcome::empty();
        self.apply_client_worker(&mut outcome)?;
        Ok(outcome)
    }

    /// Targeted pump of woken routes. Never falls back to a global adapter scan.
    pub fn pump_woken(
        &mut self,
        batch: &TerminalWakeBatch,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let (outcome, sessions) = self.pump_woken_phase_one(batch, now_seconds)?;
        self.pump_woken_phase_three(batch, outcome, &sessions)
    }

    pub(crate) fn pump_woken_phase_one(
        &mut self,
        batch: &TerminalWakeBatch,
        now_seconds: u64,
    ) -> Result<(MultiplexerEngineOutcome, HashSet<SessionId>), ManagedSessionRuntimeError> {
        let mut outcome = MultiplexerEngineOutcome::empty();
        let mut sessions = HashSet::new();
        for route in &batch.adapter_routes {
            sessions.insert(route.session_id.clone());
        }
        for session_id in &batch.ingress_sessions {
            sessions.insert(session_id.clone());
        }
        let intake = self.client_worker.intake_woken(batch);
        self.pending_input_teardowns.extend(intake);
        for session_id in &sessions {
            match self.drain_runtime_output_for_session(session_id, now_seconds) {
                Ok(step) => append_outcome(&mut outcome, step),
                Err(ManagedSessionRuntimeError::Runtime(error))
                    if error.kind == SessionRuntimeErrorKind::SessionNotFound
                        && self.session_exited(session_id) => {}
                Err(error) => return Err(error),
            }
        }
        for session_id in &sessions {
            self.route_pending_runtime_events_for(session_id, &mut outcome)?;
        }
        Ok((outcome, sessions))
    }

    pub(crate) fn pump_woken_phase_three(
        &mut self,
        batch: &TerminalWakeBatch,
        mut outcome: MultiplexerEngineOutcome,
        sessions: &HashSet<SessionId>,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let mut teardowns = self
            .client_worker
            .ingest_bound_terminal_frames(&mut outcome.client_egress);
        teardowns.extend(self.client_worker.pump_woken(batch));
        let (owned_teardowns, foreign_teardowns) =
            std::mem::take(&mut self.pending_input_teardowns)
                .into_iter()
                .partition(|teardown| sessions.contains(&teardown.session_id));
        self.pending_input_teardowns = foreign_teardowns;
        teardowns.splice(0..0, owned_teardowns);
        self.unsubscribe_owner_teardowns(&mut outcome, &mut teardowns)?;
        Ok(outcome)
    }

    /// Block until adapter or ingress wakes arrive, or `timeout` elapses.
    #[must_use]
    pub fn wait_wakes(&self, timeout: Duration) -> TerminalWakeBatch {
        self.wake_source.wait_wakes(timeout)
    }

    /// Classify one session's activity at the provided clock value.
    pub fn classify_activity(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<SessionActivityStatus, ManagedSessionRuntimeError> {
        Ok(self.engine.classify_session_activity(
            session_id,
            now_seconds,
            active_threshold_seconds,
        )?)
    }

    /// Inspect one session's lifecycle and activity through the managed engine.
    pub fn inspect_session(
        &self,
        session_id: &SessionId,
        now_seconds: u64,
        active_threshold_seconds: u64,
    ) -> Result<EngineSessionInspection, ManagedSessionRuntimeError> {
        Ok(EngineSessionInspection {
            session: self
                .session(session_id)
                .ok_or_else(|| MultiplexerEngineError::UnknownSession {
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

    /// Read a session's plain screen state through the existing worker path.
    pub fn read_screen(
        &mut self,
        request_id: RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let output = self.handle_session_request_with(
            SessionIoRequest::GetScreen {
                request_id,
                session_id: session_id.clone(),
            },
            now_seconds,
            false,
        )?;
        self.ensure_terminal_backend_ok(&session_id, "screen_state")?;
        Ok(output)
    }

    /// Capture a session snapshot through the existing worker path.
    pub fn capture_snapshot(
        &mut self,
        request_id: RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let output = self.handle_session_request_with(
            SessionIoRequest::GetSnapshot {
                request_id,
                session_id: session_id.clone(),
            },
            now_seconds,
            false,
        )?;
        self.ensure_terminal_backend_ok(&session_id, "capture_snapshot")?;
        Ok(output)
    }

    /// Capture the reusable opaque terminal snapshot payload for one session.
    pub fn capture_snapshot_payload(
        &mut self,
        session_id: &SessionId,
    ) -> Result<TerminalSnapshotPayload, ManagedSessionRuntimeError> {
        let worker = self
            .engine
            .session_worker_runtime_mut(session_id)
            .ok_or_else(|| MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            })?;
        worker.capture_snapshot_payload()
    }

    /// Capture screen state, an opaque snapshot, and a separate verified mode read.
    ///
    /// Screen state includes the backend-owned color profile when available; the
    /// color profile and snapshot are read under one terminal borrow so consumers
    /// can project an atomic colors+snapshot boundary.
    pub fn capture_terminal_state(
        &mut self,
        session_id: &SessionId,
    ) -> Result<
        (
            TerminalScreenState,
            TerminalSnapshotPayload,
            Result<ModeFlags, TerminalBackendError>,
        ),
        ManagedSessionRuntimeError,
    > {
        let worker = self
            .engine
            .session_worker_runtime_mut(session_id)
            .ok_or_else(|| MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            })?;
        worker.capture_terminal_state()
    }

    /// Capture backend-owned colors and opaque snapshot under one terminal borrow.
    pub fn capture_color_and_snapshot(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(TerminalColorProfile, TerminalSnapshotPayload), ManagedSessionRuntimeError> {
        let worker = self
            .engine
            .session_worker_runtime_mut(session_id)
            .ok_or_else(|| MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            })?;
        worker.capture_color_and_snapshot()
    }

    /// Replay or prepare a snapshot through the existing worker path.
    pub fn replay_snapshot(
        &mut self,
        request: PreparedSnapshotRequest,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        self.handle_session_request(SessionIoRequest::PrepareSnapshot(request), now_seconds)
    }

    /// Shut down a managed session through the worker/runtime path.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let previous_lifecycle = self
            .engine
            .session(&session_id)
            .map(|session| session.lifecycle.clone())
            .ok_or_else(|| MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            })?;
        if matches!(
            &previous_lifecycle,
            SessionLifecycleState::Exited { .. } | SessionLifecycleState::Stopping
        ) {
            self.pending_input_teardowns
                .extend(self.client_worker.teardown_session(&session_id));
            let mut outcome =
                self.engine
                    .shutdown_session(session_id.clone(), reason, now_seconds)?;
            self.apply_client_worker(&mut outcome)?;
            return Ok(outcome);
        }
        let outcome = self
            .engine
            .shutdown_session(session_id.clone(), reason, now_seconds)?;

        if let Err(failure) = self.flush_runtime_inputs_for_session(&session_id) {
            self.engine
                .rollback_shutdown_session(&session_id, previous_lifecycle)?;
            self.cancel_queued_shutdown(&session_id);
            return Err(failure.into());
        }

        self.flush_remaining_runtime_inputs(&session_id)?;
        Ok(outcome)
    }

    fn apply_client_worker(
        &mut self,
        outcome: &mut MultiplexerEngineOutcome,
    ) -> Result<(), ManagedSessionRuntimeError> {
        self.apply_client_worker_with(outcome, Vec::new())
    }

    fn apply_client_worker_with(
        &mut self,
        outcome: &mut MultiplexerEngineOutcome,
        mut teardowns: Vec<crate::engine::client_worker::ClientWorkerTeardown>,
    ) -> Result<(), ManagedSessionRuntimeError> {
        teardowns.extend(
            self.client_worker
                .ingest_bound_terminal_frames(&mut outcome.client_egress),
        );
        teardowns.extend(self.client_worker.pump());
        teardowns.splice(0..0, std::mem::take(&mut self.pending_input_teardowns));
        self.unsubscribe_owner_teardowns(outcome, &mut teardowns)
    }

    fn unsubscribe_owner_teardowns(
        &mut self,
        outcome: &mut MultiplexerEngineOutcome,
        teardowns: &mut Vec<crate::engine::client_worker::ClientWorkerTeardown>,
    ) -> Result<(), ManagedSessionRuntimeError> {
        for teardown in teardowns.drain(..) {
            if let Some(request_id) = &teardown.awaiting_gated {
                let _ = self
                    .engine
                    .session_runtime_mut()
                    .cancel_mode_gated_pty_input(&teardown.session_id, request_id);
            }
            let step = self.engine.handle_client_ingress(
                teardown.client_id.clone(),
                TransportIngress::UnsubscribeSession {
                    client_id: teardown.client_id,
                    session_id: teardown.session_id,
                    subscription_id: teardown.subscription_id,
                },
                0,
            )?;
            append_outcome(outcome, step);
        }
        Ok(())
    }

    fn flush_runtime_inputs(&mut self) -> Result<(), SessionRuntimeError> {
        let session_ids = self.engine_session_ids();
        for session_id in session_ids {
            self.flush_runtime_inputs_for_session(&session_id)?;
        }
        Ok(())
    }

    fn flush_runtime_inputs_for_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), SessionRuntimeError> {
        let inputs = self
            .engine_worker(session_id)
            .map(SessionRuntimeWorkerAdapter::drain_inputs)
            .unwrap_or_default();
        let mut inputs = inputs.into_iter();
        while let Some(input) = inputs.next() {
            if let Err(error) = self.engine.session_runtime_mut().send_input(input.clone()) {
                if let Some(worker) = self.engine_worker(session_id) {
                    if error.message.contains("control queue full") {
                        worker.prepend_inputs(std::iter::once(input).chain(inputs));
                    } else {
                        worker.prepend_inputs(inputs);
                    }
                }
                return Err(error);
            }
        }
        Ok(())
    }

    fn flush_remaining_runtime_inputs(
        &mut self,
        completed_session_id: &SessionId,
    ) -> Result<(), SessionRuntimeError> {
        for session_id in self.engine_session_ids() {
            if &session_id != completed_session_id {
                self.flush_runtime_inputs_for_session(&session_id)?;
            }
        }
        Ok(())
    }

    fn cancel_queued_shutdown(&mut self, session_id: &SessionId) {
        if let Some(worker) = self.engine_worker(session_id) {
            worker.cancel_shutdown(session_id);
        }
    }

    fn route_pending_runtime_events(
        &mut self,
        outcome: &mut MultiplexerEngineOutcome,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let session_ids = self.engine_session_ids();
        for session_id in session_ids {
            let events = self
                .engine_worker(&session_id)
                .map(SessionRuntimeWorkerAdapter::drain_pending_runtime_events)
                .unwrap_or_default();
            for event in events {
                let step = self.engine.handle_runtime_event(event)?;
                append_outcome(outcome, step);
            }
        }
        Ok(())
    }

    fn route_pending_runtime_events_for(
        &mut self,
        session_id: &SessionId,
        outcome: &mut MultiplexerEngineOutcome,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let events = self
            .engine_worker(session_id)
            .map(SessionRuntimeWorkerAdapter::drain_pending_runtime_events)
            .unwrap_or_default();
        for event in events {
            let step = self.engine.handle_runtime_event(event)?;
            append_outcome(outcome, step);
        }
        Ok(())
    }

    fn engine_session_ids(&self) -> Vec<SessionId> {
        self.engine.session_ids()
    }

    fn engine_worker(
        &mut self,
        session_id: &SessionId,
    ) -> Option<&mut SessionRuntimeWorkerAdapter<T>> {
        self.engine.session_worker_runtime_mut(session_id)
    }

    fn session_exited(&self, session_id: &SessionId) -> bool {
        matches!(
            self.session(session_id).map(|session| &session.lifecycle),
            Some(SessionLifecycleState::Exited { .. })
        )
    }

    fn ensure_terminal_backend_ok(
        &mut self,
        session_id: &SessionId,
        operation: &'static str,
    ) -> Result<(), ManagedSessionRuntimeError> {
        if let Some(message) = self
            .engine_worker(session_id)
            .and_then(|worker| worker.last_terminal_error())
        {
            return Err(ManagedSessionRuntimeError::TerminalBackendOperation {
                operation,
                message,
            });
        }
        Ok(())
    }
}

fn owner_apply_teardown(
    worker: &mut ClientWorker,
    session_id: &SessionId,
    subscription_id: &SubscriptionId,
) -> Result<(), crate::engine::client_worker::ClientWorkerTeardown> {
    match worker.detach_live(session_id, subscription_id) {
        Some(teardown) => Err(teardown),
        None => Ok(()),
    }
}

#[cfg(feature = "local-runtime")]
fn owner_apply_teardown_outcome(
    worker: &mut ClientWorker,
    session_id: &SessionId,
    subscription_id: &SubscriptionId,
) -> Result<Option<MultiplexerEngineOutcome>, crate::engine::client_worker::ClientWorkerTeardown> {
    owner_apply_teardown(worker, session_id, subscription_id).map(|()| None)
}

fn with_subscription(
    mut result: TerminalInputResult,
    subscription_id: &SubscriptionId,
) -> TerminalInputResult {
    result.subscription_id = subscription_id.0.clone();
    result
}

fn input_result_ok(kind: TerminalInputKind, bytes_written: usize) -> TerminalInputResult {
    TerminalInputResult {
        subscription_id: String::new(),
        kind,
        admitted: true,
        bytes_written,
        mode_generation: 0,
        mode_revision: 0,
        mode_flags: empty_terminal_mode_flags(),
        rejection: None,
    }
}

fn input_result_rejected(
    kind: TerminalInputKind,
    rejection: TerminalInputRejection,
) -> TerminalInputResult {
    TerminalInputResult {
        subscription_id: String::new(),
        kind,
        admitted: false,
        bytes_written: 0,
        mode_generation: 0,
        mode_revision: 0,
        mode_flags: empty_terminal_mode_flags(),
        rejection: Some(rejection),
    }
}

fn empty_terminal_mode_flags() -> TerminalModeFlags {
    TerminalModeFlags {
        kitty_enabled: false,
        cursor_visible: false,
        bracketed_paste: false,
        mouse_mode: 0,
        alt_screen: false,
        focus_reporting: false,
        application_cursor: false,
    }
}

fn map_gated_result(
    kind: TerminalInputKind,
    result: ModeGatedPtyInputResult,
) -> TerminalInputResult {
    let rejection = if result.admitted {
        None
    } else if result.error_kind.as_deref() == Some("partial_write")
        || result
            .error_kind
            .as_deref()
            .is_some_and(|kind| kind.starts_with("partial_write:"))
    {
        Some(TerminalInputRejection::PartialWrite)
    } else if matches!(
        result.error_kind.as_deref(),
        Some("deadline_exceeded") | Some("cancelled")
    ) {
        Some(TerminalInputRejection::Timeout)
    } else if result.error_kind.is_none() && result.bytes_written == 0 {
        Some(TerminalInputRejection::StaleMode)
    } else {
        Some(TerminalInputRejection::SessionNotWritable)
    };
    TerminalInputResult {
        subscription_id: String::new(),
        kind,
        admitted: result.admitted,
        bytes_written: result.bytes_written,
        mode_generation: result.mode_freshness.mode_generation,
        mode_revision: result.mode_freshness.mode_revision,
        mode_flags: terminal_mode_flags_from(result.mode_flags),
        rejection,
    }
}

/// Total mapping from Core `ModeFlags` to the client-facing copy.
pub fn terminal_mode_flags_from(flags: ModeFlags) -> TerminalModeFlags {
    TerminalModeFlags {
        kitty_enabled: flags.kitty_enabled,
        cursor_visible: flags.cursor_visible,
        bracketed_paste: flags.bracketed_paste,
        mouse_mode: flags.mouse_mode,
        alt_screen: flags.alt_screen,
        focus_reporting: flags.focus_reporting,
        application_cursor: flags.application_cursor,
    }
}

fn append_outcome(target: &mut MultiplexerEngineOutcome, source: MultiplexerEngineOutcome) {
    target.client_egress.extend(source.client_egress);
    target.session_requests.extend(source.session_requests);
    target
        .client_control_frames
        .extend(source.client_control_frames);
    target.session_events.extend(source.session_events);
    target.observations.extend(source.observations);
}

/// Session worker adapter that converts PTY I/O and lifecycle operations into runtime inputs.
#[derive(Debug)]
pub(crate) struct SessionRuntimeWorkerAdapter<T>
where
    T: TerminalScreenRuntime,
{
    state: Rc<RefCell<SessionRuntimeWorkerState<T>>>,
}

impl<T> Clone for SessionRuntimeWorkerAdapter<T>
where
    T: TerminalScreenRuntime,
{
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<T> SessionRuntimeWorkerAdapter<T>
where
    T: TerminalScreenRuntime,
{
    /// Build an adapter with core-owned terminal state.
    #[must_use]
    pub(crate) fn new(terminal: T) -> Self {
        Self {
            state: Rc::new(RefCell::new(SessionRuntimeWorkerState {
                inputs: Vec::new(),
                terminal: TerminalScreenEngine::new(terminal),
                pending_runtime_events: Vec::new(),
                prepared_mode_flags: None,
                mode_generation: new_mode_generation(),
                mode_revision: 1,
                last_mode_flags: ModeFlags::default(),
            })),
        }
    }

    /// Record runtime output in terminal state before live fanout.
    ///
    /// When the backend generates PTY query replies (for example OSC color
    /// probe responses owned by the session terminal runtime), those bytes are
    /// queued as session PTY input so the child receives them without a client.
    pub(crate) fn record_output(&mut self, session_id: &SessionId, data: &[u8]) {
        let mut state = self.state.borrow_mut();
        state.terminal.normalize_output(data);
        if let Ok(flags) = state.terminal.runtime().mode_flags() {
            let _ = state.observe_mode_flags(&flags);
        }
        let pty_replies = state.terminal.runtime_mut().drain_pty_writes();
        if !pty_replies.is_empty() {
            state.inputs.push(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: pty_replies,
            });
        }
    }

    /// Drain pending runtime inputs recorded by worker operations.
    pub(crate) fn drain_inputs(&mut self) -> Vec<SessionRuntimeInput> {
        self.state.borrow_mut().inputs.drain(..).collect()
    }

    pub(crate) fn prepend_inputs(&mut self, inputs: impl IntoIterator<Item = SessionRuntimeInput>) {
        let mut state = self.state.borrow_mut();
        let mut retained = inputs.into_iter().collect::<Vec<_>>();
        retained.append(&mut state.inputs);
        state.inputs = retained;
    }

    pub(crate) fn cancel_shutdown(&mut self, session_id: &SessionId) {
        let mut state = self.state.borrow_mut();
        if let Some(index) = state.inputs.iter().rposition(|input| {
            matches!(
                input,
                SessionRuntimeInput::Shutdown {
                    session_id: queued_session_id
                } if queued_session_id == session_id
            )
        }) {
            state.inputs.remove(index);
        }
    }

    /// Drain pending worker events that must pass through the worker engine.
    pub(crate) fn drain_pending_runtime_events(&mut self) -> Vec<crate::SessionWorkerRuntimeEvent> {
        self.state
            .borrow_mut()
            .pending_runtime_events
            .drain(..)
            .collect()
    }

    pub(crate) fn capture_snapshot_payload(
        &mut self,
    ) -> Result<TerminalSnapshotPayload, ManagedSessionRuntimeError> {
        let mut state = self.state.borrow_mut();
        let snapshot = state
            .terminal
            .capture_snapshot()
            .snapshot
            .expect("terminal screen engine captures a snapshot");
        if let Some(message) = state.terminal.runtime().last_error() {
            return Err(ManagedSessionRuntimeError::TerminalBackendOperation {
                operation: "capture_snapshot",
                message,
            });
        }
        Ok(snapshot)
    }

    #[cfg(feature = "local-runtime")]
    pub(crate) fn replay_snapshot(
        &mut self,
        snapshot: TerminalSnapshotPayload,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let mut state = self.state.borrow_mut();
        state.terminal.replay_snapshot(snapshot);
        if let Some(message) = state.terminal.runtime().last_error() {
            return Err(ManagedSessionRuntimeError::TerminalBackendOperation {
                operation: "replay_snapshot",
                message,
            });
        }
        Ok(())
    }

    pub(crate) fn capture_terminal_state(
        &mut self,
    ) -> Result<
        (
            TerminalScreenState,
            TerminalSnapshotPayload,
            Result<ModeFlags, TerminalBackendError>,
        ),
        ManagedSessionRuntimeError,
    > {
        let mut state = self.state.borrow_mut();
        // screen_state populates color_profile under the same terminal borrow
        // used for the snapshot capture below.
        let screen = state
            .terminal
            .screen_state()
            .screen
            .expect("terminal screen engine reads screen state");
        let mode_flags = state.terminal.runtime().mode_flags();
        let snapshot = state
            .terminal
            .capture_snapshot()
            .snapshot
            .expect("terminal screen engine captures a snapshot");
        if let Some(message) = state.terminal.runtime().last_error() {
            return Err(ManagedSessionRuntimeError::TerminalBackendOperation {
                operation: "capture_snapshot",
                message,
            });
        }
        Ok((screen, snapshot, mode_flags))
    }

    pub(crate) fn capture_color_and_snapshot(
        &mut self,
    ) -> Result<(TerminalColorProfile, TerminalSnapshotPayload), ManagedSessionRuntimeError> {
        let mut state = self.state.borrow_mut();
        // Color profile and opaque snapshot share one exclusive terminal borrow
        // so the CoreDaemon atomic dual-return cannot observe a race.
        let color_profile = state
            .terminal
            .runtime()
            .color_profile()
            .map_err(managed_terminal_backend_error)?
            .ok_or_else(|| ManagedSessionRuntimeError::TerminalBackendOperation {
                operation: "color_profile",
                message: "terminal did not expose a color profile".to_string(),
            })?;
        let snapshot = state
            .terminal
            .capture_snapshot()
            .snapshot
            .expect("terminal screen engine captures a snapshot");
        if let Some(message) = state.terminal.runtime().last_error() {
            return Err(ManagedSessionRuntimeError::TerminalBackendOperation {
                operation: "capture_snapshot",
                message,
            });
        }
        Ok((color_profile, snapshot))
    }

    fn prepare_mode_flags(&mut self) -> Result<(), ManagedSessionRuntimeError> {
        let mut state = self.state.borrow_mut();
        let flags = state
            .terminal
            .runtime()
            .mode_flags()
            .map_err(managed_terminal_backend_error)?;
        state.prepared_mode_flags = Some(flags);
        Ok(())
    }

    fn prepare_color_profile(
        &mut self,
        color_profile: TerminalColorProfile,
    ) -> Result<(), ManagedSessionRuntimeError> {
        self.state
            .borrow_mut()
            .terminal
            .runtime_mut()
            .set_color_profile(color_profile)
            .map_err(managed_terminal_backend_error)
    }

    pub(crate) fn last_terminal_error(&self) -> Option<String> {
        self.state.borrow().terminal.runtime().last_error()
    }
}

#[derive(Debug)]
struct SessionRuntimeWorkerState<T>
where
    T: TerminalScreenRuntime,
{
    inputs: Vec<SessionRuntimeInput>,
    terminal: TerminalScreenEngine<T>,
    pending_runtime_events: Vec<crate::SessionWorkerRuntimeEvent>,
    prepared_mode_flags: Option<ModeFlags>,
    mode_generation: u64,
    mode_revision: u64,
    last_mode_flags: ModeFlags,
}

impl<T> SessionRuntimeWorkerState<T>
where
    T: TerminalScreenRuntime,
{
    fn observe_mode_flags(&mut self, mode_flags: &ModeFlags) -> ModeFreshnessToken {
        if mode_flags != &self.last_mode_flags {
            if self.mode_revision == u64::MAX {
                // Overflow is fail-closed for gated admit; keep the saturated
                // revision so probes remain self-consistent within the epoch.
                self.mode_revision = u64::MAX;
            } else {
                self.mode_revision = self.mode_revision.saturating_add(1);
            }
            self.last_mode_flags = mode_flags.clone();
        }
        ModeFreshnessToken {
            mode_generation: self.mode_generation,
            mode_revision: self.mode_revision,
        }
    }
}

fn new_mode_generation() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(1);
    let mixed = nanos
        ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (std::ptr::from_ref(&nanos) as usize as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    if mixed == 0 {
        1
    } else {
        mixed
    }
}

impl<T> SessionWorkerRuntime for SessionRuntimeWorkerAdapter<T>
where
    T: TerminalScreenRuntime,
{
    fn write_input(&mut self, session_id: &SessionId, data: &[u8]) {
        self.state
            .borrow_mut()
            .inputs
            .push(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: data.to_vec(),
            });
    }

    fn resize(
        &mut self,
        session_id: &SessionId,
        rows: u16,
        cols: u16,
    ) -> Result<(), SessionRuntimeError> {
        let mut state = self.state.borrow_mut();
        state.terminal.resize(TerminalScreenSize::new(rows, cols));
        if let Some(message) = state.terminal.runtime().last_error() {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                message,
            ));
        }
        state.inputs.push(SessionRuntimeInput::Resize {
            session_id: session_id.clone(),
            size: ResizePayload { rows, cols },
        });
        Ok(())
    }

    fn snapshot(&mut self, request_id: RequestId, session_id: SessionId) -> SnapshotReady {
        let snapshot = self
            .state
            .borrow_mut()
            .terminal
            .capture_snapshot()
            .snapshot
            .expect("terminal screen engine captures a snapshot");
        snapshot.into_snapshot_ready(request_id, session_id)
    }

    fn request_initial_snapshot(
        &mut self,
        request: crate::InitialSnapshotRequest,
    ) -> Result<(), SessionRuntimeError> {
        let snapshot = self
            .state
            .borrow_mut()
            .terminal
            .capture_snapshot()
            .snapshot
            .expect("terminal screen engine captures a snapshot");
        if let Some(message) = self.state.borrow().terminal.runtime().last_error() {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                message,
            ));
        }
        self.state.borrow_mut().pending_runtime_events.push(
            crate::SessionWorkerRuntimeEvent::InitialSnapshotReady(crate::InitialSnapshotReady {
                request_id: request.request_id,
                session_id: request.session_id,
                client_id: request.client_id,
                subscription_id: request.subscription_id,
                snapshot: snapshot.bytes,
                rows: snapshot.size.rows,
                cols: snapshot.size.cols,
            }),
        );
        Ok(())
    }

    fn send_file(&mut self, request: SendFileRequest) -> Result<SendFileWritten, SendFileFailed> {
        Ok(SendFileWritten {
            request_id: request.request_id,
            session_id: request.session_id,
            bytes: request.data.len(),
            storage_ref: None,
        })
    }

    fn prepare_snapshot(
        &mut self,
        request: crate::PreparedSnapshotRequest,
    ) -> PreparedSnapshotReady {
        PreparedSnapshotReady {
            request_id: request.request_id,
            session_id: request.session_id,
            uncompressed_len: request.snapshot.len(),
            payload: request.snapshot,
            recovery: request.recovery,
        }
    }

    fn mode_flags(
        &mut self,
        request_id: RequestId,
        session_id: SessionId,
    ) -> Result<ModeFlagsReady, SessionRuntimeError> {
        let mut state = self.state.borrow_mut();
        let mode_flags = state.prepared_mode_flags.take().ok_or_else(|| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                "mode flags were not primed before routing",
            )
        })?;
        let mode_freshness = state.observe_mode_flags(&mode_flags);
        Ok(ModeFlagsReady {
            request_id,
            session_id,
            mode_flags,
            mode_freshness,
        })
    }

    fn screen(&mut self, request_id: RequestId, session_id: SessionId) -> ScreenReady {
        let screen = self
            .state
            .borrow()
            .terminal
            .screen_state()
            .screen
            .expect("terminal screen engine reads screen state");
        ScreenReady {
            request_id,
            session_id,
            text: screen.plain_text,
        }
    }

    fn set_color_profile(
        &mut self,
        _session_id: &SessionId,
        _color_profile: TerminalColorProfile,
    ) -> Result<(), SessionRuntimeError> {
        // Color profile apply is primed through prepare_color_profile so
        // Unsupported backends map to ManagedSessionRuntimeError::UnsupportedSessionRequest.
        Ok(())
    }

    fn shutdown(
        &mut self,
        session_id: &SessionId,
        _reason: &str,
    ) -> Result<Vec<crate::SessionWorkerRuntimeEvent>, SessionRuntimeError> {
        self.state
            .borrow_mut()
            .inputs
            .push(SessionRuntimeInput::Shutdown {
                session_id: session_id.clone(),
            });
        Ok(Vec::new())
    }
}

fn reject_unsupported_ingress(
    ingress: &TransportIngress,
) -> Result<(), ManagedSessionRuntimeError> {
    match ingress {
        TransportIngress::SendFile { .. } => unsupported("send_file"),
        TransportIngress::SubscribeSession { .. }
        | TransportIngress::UnsubscribeSession { .. }
        | TransportIngress::TerminalInput { .. }
        | TransportIngress::Resize { .. }
        | TransportIngress::RequestSnapshot { .. }
        | TransportIngress::Focus { .. }
        | TransportIngress::Heartbeat { .. }
        | TransportIngress::BoundaryPayload { .. }
        | TransportIngress::ClientState { .. }
        | TransportIngress::Ping { .. } => Ok(()),
    }
}

fn terminal_backend_ingress_operation(
    ingress: &TransportIngress,
) -> Option<(SessionId, &'static str)> {
    match ingress {
        TransportIngress::Resize { session_id, .. } => Some((session_id.clone(), "resize")),
        TransportIngress::SubscribeSession { session_id, .. } => {
            Some((session_id.clone(), "capture_snapshot"))
        }
        _ => None,
    }
}

fn reject_unsupported_session_request(
    request: &SessionIoRequest,
) -> Result<(), ManagedSessionRuntimeError> {
    match request {
        SessionIoRequest::SendFile(_) => unsupported("send_file"),
        SessionIoRequest::PrepareSnapshot(_) => unsupported("prepare_snapshot"),
        SessionIoRequest::SubscribeTerminal { .. }
        | SessionIoRequest::GetSnapshot { .. }
        | SessionIoRequest::GetInitialSnapshot(_)
        | SessionIoRequest::GetModeFlags { .. }
        | SessionIoRequest::GetScreen { .. }
        | SessionIoRequest::SetColorProfile { .. }
        | SessionIoRequest::UnsubscribeTerminal { .. }
        | SessionIoRequest::PtyInput { .. }
        | SessionIoRequest::Resize { .. }
        | SessionIoRequest::Shutdown { .. } => Ok(()),
    }
}

fn managed_terminal_backend_error(error: TerminalBackendError) -> ManagedSessionRuntimeError {
    match error {
        TerminalBackendError::Unsupported { operation } => {
            ManagedSessionRuntimeError::UnsupportedSessionRequest {
                request_kind: operation,
            }
        }
        TerminalBackendError::OperationFailed { operation, message } => {
            ManagedSessionRuntimeError::TerminalBackendOperation { operation, message }
        }
    }
}

fn unsupported(request_kind: &'static str) -> Result<(), ManagedSessionRuntimeError> {
    Err(ManagedSessionRuntimeError::UnsupportedSessionRequest { request_kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FailingInputRuntime {
        sessions: Vec<SessionId>,
        attempts: Vec<SessionRuntimeInput>,
        delivered: Vec<SessionRuntimeInput>,
        fail_next: Option<SessionRuntimeInput>,
        fail_message: Option<&'static str>,
    }

    impl FailingInputRuntime {
        fn fail_next(&mut self, input: SessionRuntimeInput) {
            self.fail_next = Some(input);
            self.fail_message = Some("forced input failure");
        }

        fn fail_next_full(&mut self, input: SessionRuntimeInput) {
            self.fail_next = Some(input);
            self.fail_message = Some("control queue full");
        }
    }

    impl SessionRuntime for FailingInputRuntime {
        fn spawn_session(
            &mut self,
            request: SessionSpawnRequest,
        ) -> Result<crate::SessionRuntimeHandle, SessionRuntimeError> {
            self.sessions.push(request.session_id.clone());
            Ok(crate::SessionRuntimeHandle {
                request_id: request.request_id,
                session_id: request.session_id,
                process: crate::ProcessIdentity {
                    pid: None,
                    runtime_id: None,
                },
            })
        }

        fn send_input(&mut self, input: SessionRuntimeInput) -> Result<(), SessionRuntimeError> {
            self.attempts.push(input.clone());
            if self.fail_next.as_ref() == Some(&input) {
                self.fail_next = None;
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::InputFailed,
                    self.fail_message.take().unwrap_or("forced input failure"),
                ));
            }
            self.delivered.push(input);
            Ok(())
        }

        fn drain_output(
            &mut self,
            _session_id: &SessionId,
        ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
            Ok(Vec::new())
        }
    }

    fn test_spawn_request(session_id: &str) -> SessionSpawnRequest {
        SessionSpawnRequest {
            request_id: RequestId(format!("{session_id}-spawn")),
            session_id: SessionId(session_id.to_string()),
            executable: "test-shell".to_string(),
            arguments: Vec::new(),
            working_directory: crate::SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: crate::SpawnEnvironment::default(),
            initial_pty_size: None,
        }
    }

    #[test]
    fn unprimed_mode_read_returns_typed_error_instead_of_panicking() {
        let mut adapter = SessionRuntimeWorkerAdapter::new(PlainTerminalScreenRuntime::default());

        let error = adapter
            .mode_flags(
                RequestId("unprimed-mode".to_string()),
                SessionId("unprimed-session".to_string()),
            )
            .expect_err("unprimed mode read should fail");

        assert_eq!(error.kind, SessionRuntimeErrorKind::OutputFailed);
        assert_eq!(error.message, "mode flags were not primed before routing");
    }

    #[test]
    fn target_input_failure_rolls_back_shutdown_and_preserves_only_unattempted_tail() {
        let session_id = SessionId("target".to_string());
        let failed_input = SessionRuntimeInput::PtyInput {
            session_id: session_id.clone(),
            data: b"before-shutdown".to_vec(),
        };
        let retained_input = SessionRuntimeInput::Resize {
            session_id: session_id.clone(),
            size: crate::ResizePayload {
                rows: 40,
                cols: 120,
            },
        };
        let shutdown = SessionRuntimeInput::Shutdown {
            session_id: session_id.clone(),
        };
        let mut runtime = ManagedSessionRuntime::new(FailingInputRuntime::default());
        runtime
            .spawn_session(
                test_spawn_request(&session_id.0),
                CoreSessionMetadata::new(),
            )
            .expect("spawn target");
        {
            let worker = runtime.engine_worker(&session_id).expect("target worker");
            worker.write_input(&session_id, b"before-shutdown");
            worker
                .resize(&session_id, 40, 120)
                .expect("queue retained resize");
        }
        runtime
            .session_runtime_mut()
            .fail_next(failed_input.clone());

        let error = runtime
            .shutdown_session(session_id.clone(), "test shutdown", 10)
            .expect_err("pre-shutdown input failure should propagate");
        assert!(matches!(
            error,
            ManagedSessionRuntimeError::Runtime(SessionRuntimeError {
                kind: SessionRuntimeErrorKind::InputFailed,
                ..
            })
        ));
        assert_eq!(
            runtime
                .session(&session_id)
                .map(|session| &session.lifecycle),
            Some(&SessionLifecycleState::Running)
        );

        runtime
            .flush_runtime_inputs()
            .expect("unattempted resize should remain reachable");
        runtime
            .shutdown_session(session_id, "retry shutdown", 11)
            .expect("fresh shutdown should remain retryable");
        assert_eq!(
            runtime.session_runtime().attempts,
            vec![failed_input, retained_input.clone(), shutdown.clone()]
        );
        assert_eq!(
            runtime.session_runtime().delivered,
            vec![retained_input, shutdown]
        );
    }

    #[test]
    fn transient_queue_full_retains_failed_input_then_remainder_in_order() {
        let session_id = SessionId("transient-full".to_string());
        let failed_input = SessionRuntimeInput::PtyInput {
            session_id: session_id.clone(),
            data: b"first".to_vec(),
        };
        let remainder = SessionRuntimeInput::Resize {
            session_id: session_id.clone(),
            size: crate::ResizePayload { rows: 30, cols: 90 },
        };
        let mut runtime = ManagedSessionRuntime::new(FailingInputRuntime::default());
        runtime
            .spawn_session(
                test_spawn_request(&session_id.0),
                CoreSessionMetadata::new(),
            )
            .expect("spawn");
        {
            let worker = runtime.engine_worker(&session_id).expect("worker");
            worker.write_input(&session_id, b"first");
            worker.resize(&session_id, 30, 90).expect("queue resize");
        }
        runtime
            .session_runtime_mut()
            .fail_next_full(failed_input.clone());

        let error = runtime
            .flush_runtime_inputs_for_session(&session_id)
            .expect_err("first admission is transiently full");
        assert_eq!(error.message, "control queue full");
        runtime
            .flush_runtime_inputs_for_session(&session_id)
            .expect("capacity retry");
        assert_eq!(
            runtime.session_runtime().attempts,
            vec![
                failed_input.clone(),
                failed_input.clone(),
                remainder.clone()
            ]
        );
        assert_eq!(
            runtime.session_runtime().delivered,
            vec![failed_input, remainder]
        );
    }

    #[test]
    fn cross_session_failure_propagates_without_rolling_back_delivered_target_shutdown() {
        let target_id = SessionId("target".to_string());
        let other_id = SessionId("other".to_string());
        let shutdown = SessionRuntimeInput::Shutdown {
            session_id: target_id.clone(),
        };
        let failed_other = SessionRuntimeInput::PtyInput {
            session_id: other_id.clone(),
            data: b"other-input".to_vec(),
        };
        let retained_other = SessionRuntimeInput::Resize {
            session_id: other_id.clone(),
            size: crate::ResizePayload { rows: 30, cols: 90 },
        };
        let mut runtime = ManagedSessionRuntime::new(FailingInputRuntime::default());
        runtime
            .spawn_session(test_spawn_request(&target_id.0), CoreSessionMetadata::new())
            .expect("spawn target");
        runtime
            .spawn_session(test_spawn_request(&other_id.0), CoreSessionMetadata::new())
            .expect("spawn other");
        {
            let worker = runtime.engine_worker(&other_id).expect("other worker");
            worker.write_input(&other_id, b"other-input");
            worker
                .resize(&other_id, 30, 90)
                .expect("queue retained other resize");
        }
        runtime
            .session_runtime_mut()
            .fail_next(failed_other.clone());

        let error = runtime
            .shutdown_session(target_id.clone(), "target shutdown", 10)
            .expect_err("other session failure should propagate");
        assert!(matches!(
            error,
            ManagedSessionRuntimeError::Runtime(SessionRuntimeError {
                kind: SessionRuntimeErrorKind::InputFailed,
                ..
            })
        ));
        assert_eq!(
            runtime
                .session(&target_id)
                .map(|session| &session.lifecycle),
            Some(&SessionLifecycleState::Stopping)
        );

        runtime
            .flush_runtime_inputs()
            .expect("other unattempted tail should remain reachable");
        assert_eq!(
            runtime.session_runtime().attempts,
            vec![shutdown.clone(), failed_other, retained_other.clone()]
        );
        assert_eq!(
            runtime.session_runtime().delivered,
            vec![shutdown, retained_other]
        );
    }
}
