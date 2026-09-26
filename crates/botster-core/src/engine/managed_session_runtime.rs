//! Scheduling-neutral managed session runtime over core engine primitives.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::rc::Rc;
use std::time::{Duration, Instant};

use botster_terminal_protocol::{InputOutcome, InputResultBody};
use thiserror::Error;

use crate::contract::actor::SessionLifecycleState;
use crate::contract::actor::{
    MailboxSendFailureReason, ModeFlagsReady, PreparedSnapshotReady, PreparedSnapshotRequest,
    QueueSource, ScreenReady, SendFileFailed, SendFileRequest, SendFileWritten, SessionIoRequest,
    SnapshotReady,
};
use crate::contract::terminal_adapter::TerminalRouteCloseReason;
use crate::contract::terminal_screen::{TerminalKeyEvent, TerminalMouseEvent};
use crate::contract::terminal_subscription::{
    AttachTerminalRouteError, BindTerminalAdapterError, DetachTerminalSubscriptionResult,
    StagedTerminalInput, TerminalCapabilitySet, TerminalSubscriptionGeneration,
    TerminalSubscriptionInventory, TerminalSubscriptionInventoryError,
};
use crate::contract::terminal_wake::{
    TerminalWakeBatch, TerminalWakeSource, WakingTerminalAdapter,
};
use crate::engine::client_worker::{ClientWorker, ClientWorkerTeardown, OwnerKey};
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
    ControlAdmission, ControlPlaneState, ControlWriterError, LocalProcessRuntime,
    WorkerProcessRuntime, WorkerProcessRuntimeOptions, WORKER_CONTROL_QUEUE_FRAMES,
    WORKER_CONTROL_RESERVED_SLOTS,
};
use crate::runtime::{
    SessionReservation, SessionReservationRefusal, SessionReservationRelease, SessionRuntime,
    SessionRuntimeError, SessionRuntimeErrorKind, SessionRuntimeInput, SessionRuntimeOutput,
    SessionSpawnRequest,
};
use crate::session::{
    CoreSessionMetadata, RequestId, SessionActivityStatus, SessionId, SubscriptionId,
};
use crate::session_protocol::{ModeFlags, ResizePayload, TerminalColorProfile, WorkerInputKind};
use crate::terminal_screen::{
    TerminalBackendError, TerminalScreenSize, TerminalScreenState, TerminalSnapshotPayload,
};
use crate::transport::TransportIngress;
use crate::ClientId;
use botster_terminal_protocol_client::{
    decode_input_body, TerminalInputCommand, TerminalInputKind,
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

/// Per-session cap for accepted-but-unacknowledged ingress resizes.
///
/// This is the ordinary worker control-lane capacity. The writer frees a queue
/// slot before the worker acknowledges, so the queue itself cannot bound this
/// collection.
#[cfg(feature = "local-runtime")]
pub const PENDING_INGRESS_RESIZE_CAP: usize =
    WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS;

#[derive(Debug, Clone)]
struct PendingTerminalResize {
    rows: u16,
    cols: u16,
    applied_at: u64,
    deadline: Instant,
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
    terminal_inventory_revision: u64,
    pending_input_teardowns: Vec<ClientWorkerTeardown>,
    /// Worker operation keys torn-down routes left in flight. The worker
    /// path drains these into `FRAME_INPUT_CANCEL`.
    pending_worker_cancels: Vec<(SessionId, u64)>,
    pending_terminal_resizes: HashMap<SessionId, VecDeque<PendingTerminalResize>>,
    applied_terminal_resizes: HashMap<SessionId, (u16, u16, u64)>,
    pending_spawn_adapters: HashMap<SessionId, PendingSpawnAdapter<T>>,
}

struct PendingSpawnAdapter<T: TerminalScreenRuntime> {
    reservation: SessionReservation,
    terminal: SessionRuntimeWorkerAdapter<T>,
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
    pub(crate) fn reconcile_terminal_resize_acknowledgments(
        &mut self,
        session_ids: &HashSet<SessionId>,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let now = Instant::now();
        for session_id in session_ids {
            let sizes = match self
                .engine
                .session_runtime_mut()
                .take_resize_applied(session_id)
            {
                Ok(sizes) => sizes,
                Err(error) if error.kind == SessionRuntimeErrorKind::SessionNotFound => {
                    if self.session(session_id).is_none() || self.session_exited(session_id) {
                        self.pending_terminal_resizes.remove(session_id);
                        continue;
                    }
                    return Err(error.into());
                }
                Err(error) => return Err(error.into()),
            };
            for size in sizes {
                self.acknowledge_terminal_resize(session_id, &size);
            }
            if self.pending_resize_expired(session_id, now) {
                if self.session_is_stopping_or_exited(session_id) {
                    self.pending_terminal_resizes.remove(session_id);
                } else {
                    self.fail_expired_pending_resize(session_id);
                }
            }
        }
        Ok(())
    }

    fn pending_resize_expired(&self, session_id: &SessionId, now: Instant) -> bool {
        self.pending_terminal_resizes
            .get(session_id)
            .and_then(|pending| pending.front())
            .is_some_and(|entry| entry.deadline <= now)
    }

    fn fail_expired_pending_resize(&mut self, session_id: &SessionId) {
        self.pending_terminal_resizes.remove(session_id);
        // The worker stopped acknowledging control frames: end the link, not
        // only the flag, so a worker parked on a barrier is released.
        self.engine
            .session_runtime_mut()
            .fail_control_plane(session_id, ControlWriterError::ResizeAckTimeout);
        let teardowns = self
            .client_worker
            .teardown_session(session_id, TerminalRouteCloseReason::WorkerLinkFailed);
        self.pending_input_teardowns.extend(teardowns);
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
        let reservation = self
            .engine
            .reserve_session_identity(session_id.clone(), false)
            .map_err(SessionRuntimeError::from)?;
        let result = (|| {
            let terminal = (self.terminal_backend_factory)(TerminalScreenSize::new(24, 80))
                .map_err(
                    |source| ManagedSessionRuntimeError::TerminalBackendConstruction { source },
                )?;
            let handle = self.engine.session_runtime_mut().adopt_session_reserved(
                &reservation,
                session_id,
                process,
                socket_path,
                supports_snapshot_boundary,
            )?;
            Ok(self.engine.adopt_reserved_session(
                &reservation,
                handle,
                metadata,
                SessionRuntimeWorkerAdapter::new(terminal),
            )?)
        })();
        if result.is_err() {
            let _ = self.engine.release_session_reservation(&reservation);
        }
        result
    }

    /// Reserve engine identity before an ordinary asynchronous worker launch.
    pub fn begin_spawn(
        &mut self,
        request: SessionSpawnRequest,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let reservation = self
            .engine
            .reserve_session_identity(request.session_id.clone(), false)
            .map_err(SessionRuntimeError::from)?;
        let result = self.begin_reserved_worker(&reservation, request, None);
        if result.is_err() {
            let _ = self.engine.release_session_reservation(&reservation);
        }
        result
    }

    /// Prepare installation before the reserved worker can start its PTY.
    pub fn begin_spawn_reserved(
        &mut self,
        reservation: &SessionReservation,
        request: SessionSpawnRequest,
        metadata: &CoreSessionMetadata,
    ) -> Result<(), ManagedSessionRuntimeError> {
        self.begin_reserved_worker(reservation, request, Some(metadata))
    }

    fn begin_reserved_worker(
        &mut self,
        reservation: &SessionReservation,
        request: SessionSpawnRequest,
        metadata: Option<&CoreSessionMetadata>,
    ) -> Result<(), ManagedSessionRuntimeError> {
        self.engine
            .validate_reservation_owner(reservation)
            .map_err(SessionRuntimeError::from)?;
        if request.session_id != *reservation.session_id()
            || self.engine.session(&request.session_id).is_some()
        {
            return Err(SessionRuntimeError::from(SessionReservationRefusal::InvalidToken).into());
        }
        if metadata.is_some_and(|metadata| !metadata.is_within_encoded_len_limit()) {
            return Err(MultiplexerEngineError::MetadataTooLarge.into());
        }
        let size = request
            .initial_pty_size
            .as_ref()
            .map(|size| TerminalScreenSize::new(size.rows, size.cols))
            .unwrap_or_else(|| TerminalScreenSize::new(24, 80));
        let terminal = (self.terminal_backend_factory)(size)
            .map_err(|source| ManagedSessionRuntimeError::TerminalBackendConstruction { source })?;
        let session_id = request.session_id.clone();
        self.engine
            .session_runtime_mut()
            .begin_spawn_reserved(reservation, request)?;
        self.pending_spawn_adapters.insert(
            session_id,
            PendingSpawnAdapter {
                reservation: reservation.clone(),
                terminal: SessionRuntimeWorkerAdapter::new(terminal),
            },
        );
        Ok(())
    }

    pub(crate) fn discard_pending_spawn(&mut self, session_id: &SessionId) {
        if let Some(pending) = self.pending_spawn_adapters.remove(session_id) {
            if let Some(table) = self.engine.session_runtime().session_admission() {
                let _ = table.retire_implicit(&pending.reservation);
            }
        }
    }

    /// Install a worker whose spawn finished asynchronously.
    ///
    /// The host called `begin_spawn` on the worker runtime and observed
    /// `WorkerSpawnPoll::Ready`; this records core state for the handle.
    pub fn install_spawned_worker(
        &mut self,
        handle: crate::SessionRuntimeHandle,
        metadata: CoreSessionMetadata,
        size: TerminalScreenSize,
    ) -> Result<MultiplexerSpawnOutcome, ManagedSessionRuntimeError> {
        if let Some(pending) = self.pending_spawn_adapters.remove(&handle.session_id) {
            return Ok(self.engine.adopt_reserved_session(
                &pending.reservation,
                handle,
                metadata,
                pending.terminal,
            )?);
        }
        let terminal = (self.terminal_backend_factory)(size)
            .map_err(|source| ManagedSessionRuntimeError::TerminalBackendConstruction { source })?;
        Ok(self.engine.adopt_session(
            handle,
            metadata,
            SessionRuntimeWorkerAdapter::new(terminal),
        )?)
    }

    /// Release worker processes for an intentional daemon restart.
    pub fn release_workers_for_restart(&mut self) {
        self.pending_terminal_resizes.clear();
        self.applied_terminal_resizes.clear();
        self.engine.session_runtime_mut().release_for_restart();
    }

    /// Durable control-plane state for one worker session.
    #[must_use]
    pub fn control_plane_state(&self, session_id: &SessionId) -> ControlPlaneState {
        self.engine
            .session_runtime()
            .control_plane_state(session_id)
    }

    /// Route worker-originated mode changes, input results, cancels, and link
    /// failures for the named sessions into ClientWorker.
    pub(crate) fn reconcile_worker_events(
        &mut self,
        session_ids: &HashSet<SessionId>,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let mut teardowns = Vec::new();
        for session_id in session_ids {
            // Mode changes travel in the ordered output stream; see
            // route_runtime_outputs.
            let results = match self
                .engine
                .session_runtime_mut()
                .take_input_results(session_id)
            {
                Ok(results) => results,
                Err(error) if error.kind == SessionRuntimeErrorKind::SessionNotFound => {
                    teardowns.extend(self.client_worker.fail_in_flight_for_session(
                        session_id,
                        InputOutcome::OutcomeUnknown,
                        "worker session is gone",
                    ));
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            for (key, result) in results {
                if let Some(teardown) = self.client_worker.complete_operation(key, result) {
                    teardowns.push(teardown);
                }
            }
            if self
                .engine
                .session_runtime_mut()
                .session_reader_finished(session_id)?
            {
                teardowns.extend(self.client_worker.fail_in_flight_for_session(
                    session_id,
                    InputOutcome::OutcomeUnknown,
                    "worker link ended before the result",
                ));
            }
        }
        self.flush_worker_cancels();
        self.pending_input_teardowns.extend(teardowns);
        Ok(())
    }

    fn flush_worker_cancels(&mut self) {
        let mut cancels = std::mem::take(&mut self.pending_worker_cancels);
        cancels.extend(self.client_worker.take_cancel_requests());
        for (session_id, key) in cancels {
            let _ = self
                .engine
                .session_runtime_mut()
                .cancel_input_operation(&session_id, key);
        }
    }

    pub(crate) fn apply_woken_terminal_input(
        &mut self,
        batch: &TerminalWakeBatch,
        last_output_at: u64,
        deferred_sessions: &HashSet<SessionId>,
        outcome: &mut MultiplexerEngineOutcome,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let _ = (last_output_at, outcome);
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
                // Queue admission is not delivery: a frame admitted before the
                // failure (a barrier cancel included) never reached the worker.
                // End the control link so the worker's reader observes EOF and
                // releases anything it still holds for this parent.
                self.engine
                    .session_runtime_mut()
                    .fail_control_plane(session_id, error);
                teardowns.extend(self.client_worker.fail_in_flight_for_session(
                    session_id,
                    InputOutcome::OutcomeUnknown,
                    "worker control plane failed",
                ));
                teardowns.extend(
                    self.client_worker
                        .teardown_session(session_id, TerminalRouteCloseReason::WorkerLinkFailed),
                );
                failed_sessions.insert(session_id.clone());
            }
        }

        self.reconcile_worker_events(&named_sessions)?;

        let mut keys = self.client_worker.adapter_route_keys(batch);
        keys.extend(self.client_worker.parked_route_keys(batch));
        let mut seen = HashSet::new();
        keys.retain(|key| seen.insert(key.clone()));
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
                        if self.pending_terminal_resize_len(&key.session_id)
                            >= PENDING_INGRESS_RESIZE_CAP
                            && self.client_worker.terminal_input_head_is_resize(&key)
                        {
                            self.client_worker.park_for_capacity(&key);
                            full_sessions.insert(key.session_id.clone());
                            break;
                        }
                        let Some(staged) = self.client_worker.take_one_terminal_input(&key) else {
                            self.client_worker.clear_capacity_parked(&key);
                            break;
                        };
                        if let Some(teardown) = self.submit_staged_input(staged, last_output_at) {
                            teardowns.push(teardown);
                            break;
                        }
                    }
                    ControlAdmission::Full => {
                        self.client_worker.park_for_capacity(&key);
                        full_sessions.insert(key.session_id.clone());
                        break;
                    }
                    ControlAdmission::Sealed => {
                        if let Some(teardown) = self
                            .client_worker
                            .hard_stop_owner(&key, TerminalRouteCloseReason::WorkerLinkFailed)
                        {
                            teardowns.push(teardown);
                        }
                        break;
                    }
                }
            }
        }
        self.flush_worker_cancels();
        self.pending_input_teardowns.extend(teardowns);
        Ok(())
    }

    /// Forward one staged operation to the worker. Returns a teardown when the
    /// route hard-stopped.
    fn submit_staged_input(
        &mut self,
        staged: StagedTerminalInput,
        last_output_at: u64,
    ) -> Option<ClientWorkerTeardown> {
        let session_id = staged.session_id.clone();
        let resize = if staged.kind == WorkerInputKind::Resize {
            match decode_input_body(TerminalInputKind::Resize, staged.operation_id, &staged.body) {
                Ok(TerminalInputCommand::Resize { rows, cols, .. }) => Some((rows, cols)),
                _ => None,
            }
        } else {
            None
        };
        let submitted = self.engine.session_runtime_mut().submit_input_operation(
            &session_id,
            staged.operation_key,
            staged.operation_id,
            staged.kind,
            &staged.body,
        );
        match submitted {
            Ok(()) => {
                if let Some((rows, cols)) = resize {
                    let deadline =
                        Instant::now() + self.engine.session_runtime().worker_reply_timeout();
                    self.pending_terminal_resizes
                        .entry(session_id)
                        .or_default()
                        .push_back(PendingTerminalResize {
                            rows,
                            cols,
                            applied_at: last_output_at,
                            deadline,
                        });
                }
                None
            }
            Err(error) => {
                let sealed = error.message.contains("control plane sealed");
                let outcome = if error.message.contains("control queue full") {
                    InputOutcome::RejectedLaneFull
                } else if sealed {
                    InputOutcome::RejectedNotWritable
                } else {
                    InputOutcome::WriteFailed
                };
                let result = InputResultBody {
                    operation_id: staged.operation_id,
                    outcome,
                    accepted_payload_bytes: Some(0),
                    written_pty_bytes: Some(0),
                    mode_bits: self
                        .client_worker
                        .session_modes(&session_id)
                        .map(|modes| modes.mode_bits)
                        .unwrap_or(0),
                    detail: error.message,
                };
                let teardown = self
                    .client_worker
                    .complete_operation(staged.operation_key, result);
                if sealed {
                    return teardown.or_else(|| {
                        self.client_worker.hard_stop_owner(
                            &OwnerKey {
                                session_id,
                                subscription_id: staged.subscription_id,
                            },
                            TerminalRouteCloseReason::WorkerLinkFailed,
                        )
                    });
                }
                teardown
            }
        }
    }
}

#[cfg(feature = "local-runtime")]
impl<T> ManagedSessionRuntime<LocalProcessRuntime, T>
where
    T: TerminalScreenRuntime + 'static,
{
    pub(crate) fn apply_woken_terminal_input(
        &mut self,
        batch: &TerminalWakeBatch,
        last_output_at: u64,
        outcome: &mut MultiplexerEngineOutcome,
    ) -> Result<(), ManagedSessionRuntimeError> {
        let mut teardowns = Vec::new();
        for key in self.client_worker.adapter_route_keys(batch) {
            for _ in 0..crate::engine::client_worker::APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK {
                let Some(staged) = self.client_worker.take_one_terminal_input(&key) else {
                    break;
                };
                match self.apply_one_local_input(staged, last_output_at) {
                    Ok(step) => append_outcome(outcome, step),
                    Err(teardown) => {
                        teardowns.push(teardown);
                        break;
                    }
                }
            }
        }
        // Local sessions never hold worker operations; cancels are no-ops.
        let _ = self.client_worker.take_cancel_requests();
        self.pending_worker_cancels.clear();
        self.pending_input_teardowns.extend(teardowns);
        Ok(())
    }

    /// Encode one staged operation with the local terminal backend and write
    /// it to the local PTY.
    fn apply_one_local_input(
        &mut self,
        staged: StagedTerminalInput,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, ClientWorkerTeardown> {
        let session_id = staged.session_id.clone();
        let subscription_id = staged.subscription_id.clone();
        let client_id = staged.client_id.clone();
        let mode_bits = self
            .engine_worker(&session_id)
            .and_then(|worker| worker.mode_bits())
            .unwrap_or(0);
        let finish = |runtime: &mut Self, result: InputResultBody| {
            runtime
                .client_worker
                .complete_operation(staged.operation_key, result)
        };
        let reject = |runtime: &mut Self, outcome: InputOutcome, detail: String| {
            let result = InputResultBody {
                operation_id: staged.operation_id,
                outcome,
                accepted_payload_bytes: Some(0),
                written_pty_bytes: Some(0),
                mode_bits,
                detail,
            };
            match finish(runtime, result) {
                Some(teardown) => Err(teardown),
                None => Ok(MultiplexerEngineOutcome::empty()),
            }
        };
        if staged.kind == WorkerInputKind::Resize {
            let Ok(TerminalInputCommand::Resize { rows, cols, .. }) =
                decode_input_body(TerminalInputKind::Resize, staged.operation_id, &staged.body)
            else {
                return reject(
                    self,
                    InputOutcome::RejectedProtocol,
                    "resize body is malformed".to_owned(),
                );
            };
            let ingress = TransportIngress::Resize {
                session_id: session_id.clone(),
                rows,
                cols,
            };
            let outcome =
                match self.apply_targeted_client_ingress(client_id, ingress, last_output_at) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        return reject(self, InputOutcome::WriteFailed, error.to_string());
                    }
                };
            self.applied_terminal_resizes
                .insert(session_id.clone(), (rows, cols, last_output_at));
            let result = InputResultBody {
                operation_id: staged.operation_id,
                outcome: InputOutcome::Written,
                accepted_payload_bytes: Some(0),
                written_pty_bytes: Some(0),
                mode_bits: self
                    .engine_worker(&session_id)
                    .and_then(|worker| worker.mode_bits())
                    .unwrap_or(mode_bits),
                detail: String::new(),
            };
            return match finish(self, result) {
                Some(teardown) => Err(teardown),
                None => Ok(outcome),
            };
        }
        let encoded = {
            let Some(worker) = self.engine_worker(&session_id) else {
                return reject(
                    self,
                    InputOutcome::SessionEnded,
                    "session is gone".to_owned(),
                );
            };
            worker.encode_local_input(staged.kind, staged.operation_id, &staged.body)
        };
        let encoded = match encoded {
            Ok(encoded) => encoded,
            Err((outcome, detail)) => return reject(self, outcome, detail),
        };
        if encoded.is_empty() {
            let result = InputResultBody {
                operation_id: staged.operation_id,
                outcome: InputOutcome::Written,
                accepted_payload_bytes: Some(staged.accepted_payload_bytes),
                written_pty_bytes: Some(0),
                mode_bits,
                detail: String::new(),
            };
            return match finish(self, result) {
                Some(teardown) => Err(teardown),
                None => Ok(MultiplexerEngineOutcome::empty()),
            };
        }
        let written = encoded.len() as u64;
        let ingress = TransportIngress::TerminalInput {
            session_id: session_id.clone(),
            data: encoded,
        };
        let outcome = match self.apply_targeted_client_ingress(client_id, ingress, last_output_at) {
            Ok(outcome) => outcome,
            Err(error) => {
                return reject(self, InputOutcome::WriteFailed, error.to_string());
            }
        };
        let result = InputResultBody {
            operation_id: staged.operation_id,
            outcome: InputOutcome::Written,
            accepted_payload_bytes: Some(staged.accepted_payload_bytes),
            written_pty_bytes: Some(written),
            mode_bits,
            detail: String::new(),
        };
        let _ = &subscription_id;
        match finish(self, result) {
            Some(teardown) => Err(teardown),
            None => Ok(outcome),
        }
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
            terminal_inventory_revision: 0,
            pending_input_teardowns: Vec::new(),
            pending_worker_cancels: Vec::new(),
            pending_terminal_resizes: HashMap::new(),
            applied_terminal_resizes: HashMap::new(),
            pending_spawn_adapters: HashMap::new(),
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

    /// Bound-route egress owner. Engines push route-personal frames through it.
    pub(crate) fn client_worker_mut(&mut self) -> &mut ClientWorker {
        &mut self.client_worker
    }

    /// Bound-route egress owner, read-only.
    pub(crate) fn client_worker(&self) -> &ClientWorker {
        &self.client_worker
    }

    /// Export the local backend snapshot as stream frames, when the backend
    /// owns a streaming exporter.
    pub(crate) fn capture_local_snapshot_frames(
        &mut self,
        session_id: &SessionId,
    ) -> Result<
        Vec<(
            crate::contract::terminal_screen::TerminalSnapshotFramePhase,
            Vec<u8>,
        )>,
        ManagedSessionRuntimeError,
    > {
        let worker = self.engine_worker(session_id).ok_or_else(|| {
            MultiplexerEngineError::UnknownSession {
                session_id: session_id.clone(),
            }
        })?;
        worker.capture_snapshot_frames()
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
        self.applied_terminal_resizes.remove(session_id);
        self.pending_terminal_resizes.remove(session_id);
        let teardowns = self
            .client_worker
            .teardown_session(session_id, TerminalRouteCloseReason::SessionEnded);
        self.pending_input_teardowns.extend(teardowns);
        let mut outcome = MultiplexerEngineOutcome::empty();
        let _ = self.apply_client_worker(&mut outcome);
        self.engine.forget_terminal_session(session_id)
    }

    /// Take the latest resize applied from targeted terminal input for one session.
    pub fn take_applied_terminal_resize(
        &mut self,
        session_id: &SessionId,
    ) -> Option<(u16, u16, u64)> {
        self.applied_terminal_resizes.remove(session_id)
    }

    /// Whether this session has accepted ingress resizes awaiting acknowledgement.
    #[must_use]
    pub fn has_pending_terminal_resizes(&self, session_id: &SessionId) -> bool {
        self.pending_terminal_resize_len(session_id) > 0
    }

    /// Number of accepted-but-unacknowledged ingress resizes for one session.
    #[must_use]
    pub fn pending_terminal_resize_len(&self, session_id: &SessionId) -> usize {
        self.pending_terminal_resizes
            .get(session_id)
            .map(VecDeque::len)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub(crate) fn test_set_lifecycle(
        &mut self,
        session_id: SessionId,
        state: SessionLifecycleState,
    ) -> Result<(), MultiplexerEngineError> {
        self.engine.test_set_lifecycle(session_id, state)
    }

    #[cfg(test)]
    pub(crate) fn test_insert_expired_pending_resize(&mut self, session_id: SessionId) {
        self.pending_terminal_resizes
            .entry(session_id)
            .or_default()
            .push_back(PendingTerminalResize {
                rows: 31,
                cols: 101,
                applied_at: 0,
                deadline: Instant::now()
                    .checked_sub(Duration::from_secs(1))
                    .unwrap_or_else(Instant::now),
            });
    }

    #[cfg(test)]
    pub(crate) fn test_pending_input_teardown_count(&self) -> usize {
        self.pending_input_teardowns.len()
    }

    #[cfg(test)]
    pub(crate) fn test_bind_owner(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        adapter: Box<dyn WakingTerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        let (_, teardowns) = self
            .client_worker
            .record_attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
            )
            .expect("test route is valid");
        self.pending_input_teardowns.extend(teardowns);
        let generation = self
            .client_worker
            .live_generation(&session_id, &subscription_id)
            .expect("record_attach installs a live generation");
        self.bind_waking_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            generation,
            TerminalCapabilitySet::empty(),
            adapter,
        )
    }

    fn acknowledge_terminal_resize(&mut self, session_id: &SessionId, size: &ResizePayload) {
        let Some(pending) = self.pending_terminal_resizes.get_mut(session_id) else {
            return;
        };
        let Some(front) = pending.front() else {
            return;
        };
        // Explicit CoreDaemon::resize sends FRAME_RESIZE without a pending
        // entry. Its acknowledgement can arrive ahead of an ingress entry and
        // must be skipped when dimensions do not match the front.
        if front.rows != size.rows || front.cols != size.cols {
            return;
        }
        let applied = pending.pop_front().expect("front existed");
        if pending.is_empty() {
            self.pending_terminal_resizes.remove(session_id);
        }
        self.applied_terminal_resizes.insert(
            session_id.clone(),
            (applied.rows, applied.cols, applied.applied_at),
        );
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

    /// Reserve an identity before context publication or process launch.
    pub fn reserve_session_for_request(
        &self,
        session_id: SessionId,
        request_id: u64,
        limit: usize,
    ) -> Result<SessionReservation, SessionReservationRefusal> {
        self.engine
            .reserve_session_for_request(session_id, request_id, limit)
    }

    /// Recover only the matching original reserve request.
    pub fn session_reservation_for_request(
        &self,
        session_id: &SessionId,
        request_id: u64,
    ) -> Result<Option<SessionReservation>, SessionReservationRefusal> {
        self.engine
            .session_reservation_for_request(session_id, request_id)
    }

    /// Return a definitive release or retained execution ownership.
    pub fn release_session_reservation(
        &self,
        reservation: &SessionReservation,
    ) -> Result<SessionReservationRelease, SessionReservationRefusal> {
        self.engine.release_session_reservation(reservation)
    }

    /// Count unresolved reservations owned by this engine.
    #[must_use]
    pub fn pending_session_reservations(&self) -> usize {
        self.engine.pending_session_reservations()
    }

    /// Launch synchronously under the supplied reservation.
    pub fn spawn_reserved_session(
        &mut self,
        reservation: &SessionReservation,
        request: SessionSpawnRequest,
        metadata: CoreSessionMetadata,
    ) -> Result<MultiplexerSpawnOutcome, ManagedSessionRuntimeError> {
        self.engine
            .validate_reservation_owner(reservation)
            .map_err(SessionRuntimeError::from)?;
        let size = request
            .initial_pty_size
            .as_ref()
            .map(|size| TerminalScreenSize::new(size.rows, size.cols))
            .unwrap_or_else(|| TerminalScreenSize::new(24, 80));
        let terminal = (self.terminal_backend_factory)(size)
            .map_err(|source| ManagedSessionRuntimeError::TerminalBackendConstruction { source })?;
        Ok(self.engine.spawn_reserved_session(
            reservation,
            request,
            metadata,
            SessionRuntimeWorkerAdapter::new(terminal),
        )?)
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
            let (_, replacements) = self
                .client_worker
                .record_attach(
                    subscribe_client.clone(),
                    session_id.clone(),
                    subscription_id.clone(),
                )
                .map_err(attach_route_error)?;
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
        if let TransportIngress::Resize { session_id, .. } = &ingress {
            extra_teardowns.extend(self.publish_local_modes_if_changed(session_id));
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
        let resize = matches!(ingress, TransportIngress::Resize { .. });
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
        if let Err(error) = self.flush_runtime_inputs_for_session(&session_id) {
            if !error.message.contains("control queue full") {
                return Err(error.into());
            }
        }
        if resize {
            let teardowns = self.publish_local_modes_if_changed(&session_id);
            self.pending_input_teardowns.extend(teardowns);
        }
        Ok(outcome)
    }

    /// Publish MODES for an in-process engine worker when its mode bits or
    /// screen size differ from the last published MODES.
    fn publish_local_modes_if_changed(
        &mut self,
        session_id: &SessionId,
    ) -> Vec<ClientWorkerTeardown> {
        let Some(worker) = self.engine_worker(session_id) else {
            return Vec::new();
        };
        let Some(mode_bits) = worker.mode_bits() else {
            return Vec::new();
        };
        let size = worker.size();
        let modes = botster_terminal_protocol::ModesBody {
            mode_bits,
            rows: size.rows,
            cols: size.cols,
        };
        if self.client_worker.session_modes(session_id) == Some(modes) {
            return Vec::new();
        }
        self.client_worker.push_session_modes(session_id, modes)
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
        let (_, replacements) = self
            .client_worker
            .record_attach(client_id, session_id, subscription_id)
            .map_err(attach_route_error)?;
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

    /// Complete control-plane inventory admitted against caller-owned logical bytes.
    pub fn list_terminal_subscriptions(
        &self,
        max_logical_bytes: usize,
    ) -> Result<TerminalSubscriptionInventory, TerminalSubscriptionInventoryError> {
        self.client_worker
            .list_terminal_subscriptions(max_logical_bytes)
    }

    /// Monotonic revision of terminal route removals that Core has committed.
    #[must_use]
    pub const fn terminal_inventory_revision(&self) -> u64 {
        self.terminal_inventory_revision
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
            .terminal_subscription_matches(session_id, client_id, subscription_id)
    }

    pub(crate) fn terminal_subscription_owners(
        &self,
    ) -> impl Iterator<Item = (&SessionId, &ClientId, &SubscriptionId)> {
        self.client_worker.terminal_subscription_owners()
    }

    /// Borrow the live client and generation, using an allocation-free O(n) scan.
    #[must_use]
    pub fn terminal_subscription_owner(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<(&ClientId, TerminalSubscriptionGeneration)> {
        self.client_worker
            .terminal_subscription_owner(session_id, subscription_id)
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

    /// Take session ids whose bound Ready queues grew since the last take.
    #[must_use]
    pub fn take_bound_queue_wake_sessions(&mut self) -> HashSet<SessionId> {
        self.client_worker.take_bound_queue_wake_sessions()
    }

    /// Whether any live owner still holds undelivered frames for this session.
    #[must_use]
    pub fn session_has_undelivered_frames(&self, session_id: &SessionId) -> bool {
        self.client_worker
            .session_has_undelivered_frames(session_id)
    }

    /// Whether the bound owner still holds frames that the next pump must flush.
    #[must_use]
    pub fn bound_owner_has_held_frames(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.client_worker
            .bound_owner_has_held_frames(session_id, subscription_id)
    }

    /// Route one session I/O request through the existing session worker path.
    pub fn handle_session_request(
        &mut self,
        request: SessionIoRequest,
        now_seconds: u64,
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
        self.apply_client_worker(&mut outcome)?;
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
        self.apply_client_worker(&mut outcome)?;

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
        let mut teardowns = Vec::new();

        // Runtime drains are output-only; worker input buffers are populated by
        // request routing paths and are flushed by those mutators. Bound routes
        // receive the binary frame here, once per event; the multiplexer path
        // below serves unbound drain consumers.
        for output in outputs {
            let runtime_event = match output {
                SessionRuntimeOutput::ModesChanged { session_id, modes } => {
                    // Delivered at its place in the stream, before the output
                    // that caused it.
                    teardowns.extend(self.client_worker.push_session_modes(&session_id, modes));
                    continue;
                }
                SessionRuntimeOutput::PtyOutput { session_id, data } => {
                    if let Some(worker) = self.engine_worker(&session_id) {
                        worker.record_output(&session_id, &data);
                    }
                    // Like the session worker: a mode change caused by these
                    // bytes reaches the route before the bytes themselves.
                    teardowns.extend(self.publish_local_modes_if_changed(&session_id));
                    teardowns.extend(self.client_worker.push_session_output(&session_id, &data));
                    crate::SessionWorkerRuntimeEvent::TerminalBytes {
                        session_id,
                        data,
                        last_output_at,
                    }
                }
                SessionRuntimeOutput::ProcessExited {
                    session_id,
                    payload,
                } => {
                    teardowns.extend(
                        self.client_worker
                            .push_session_process_exit(&session_id, payload.exit_code),
                    );
                    crate::SessionWorkerRuntimeEvent::ProcessExited {
                        session_id,
                        payload,
                    }
                }
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
        self.pending_input_teardowns.extend(teardowns);

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

    /// Targeted pump of woken routes. Never falls back to a global adapter scan.
    pub fn pump_woken(
        &mut self,
        batch: &TerminalWakeBatch,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let (outcome, sessions) = self.pump_woken_phase_one(batch, now_seconds, &HashSet::new())?;
        self.pump_woken_phase_three(batch, outcome, &sessions)
    }

    pub(crate) fn pump_woken_phase_one(
        &mut self,
        batch: &TerminalWakeBatch,
        now_seconds: u64,
        deferred_sessions: &HashSet<SessionId>,
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
        for session_id in sessions.difference(deferred_sessions) {
            match self.drain_runtime_output_for_session(session_id, now_seconds) {
                Ok(step) => append_outcome(&mut outcome, step),
                Err(ManagedSessionRuntimeError::Runtime(error))
                    if error.kind == SessionRuntimeErrorKind::SessionNotFound
                        && self.session_exited(session_id) => {}
                Err(error) => return Err(error),
            }
        }
        for session_id in sessions.difference(deferred_sessions) {
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
            .filter_bound_terminal_frames(&mut outcome.client_egress);
        teardowns.extend(self.client_worker.pump_woken(batch));
        let (owned_teardowns, foreign_teardowns) =
            std::mem::take(&mut self.pending_input_teardowns)
                .into_iter()
                .partition(|teardown| sessions.contains(&teardown.session_id));
        self.pending_input_teardowns = foreign_teardowns;
        teardowns.splice(0..0, owned_teardowns);
        self.unsubscribe_owner_teardowns(&mut outcome, &mut teardowns)?;
        let _ = self.client_worker.take_bound_queue_wake_sessions();
        Ok(outcome)
    }

    /// Block until adapter or ingress wakes arrive, or `timeout` elapses.
    #[must_use]
    pub fn wait_wakes(&self, timeout: Duration) -> TerminalWakeBatch {
        let batch = self.wake_source.wait_wakes(self.clamp_paste_wait(timeout));
        self.merge_deadline_wakes(batch)
    }

    /// Clamp a host wait so paste, reader-progress, or pending-resize
    /// deadlines cannot be skipped.
    #[must_use]
    pub fn clamp_paste_wait(&self, timeout: Duration) -> Duration {
        self.next_core_deadline()
            .map(|deadline| {
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(timeout)
            })
            .unwrap_or(timeout)
    }

    /// Build the exact targeted wake batch for expired paste, reader-progress,
    /// and resize deadlines.
    #[must_use]
    pub fn expired_paste_wake_batch(&self, now: Instant) -> TerminalWakeBatch {
        let mut adapter_routes = self.client_worker.expired_paste_routes(now);
        for route in self.client_worker.expired_reader_routes(now) {
            if !adapter_routes.contains(&route) {
                adapter_routes.push(route);
            }
        }
        TerminalWakeBatch {
            adapter_routes,
            ingress_sessions: self.expired_pending_resize_sessions(now),
        }
    }

    /// Merge expired paste routes and pending-resize sessions into a wake batch.
    #[must_use]
    pub fn merge_deadline_wakes(&self, mut batch: TerminalWakeBatch) -> TerminalWakeBatch {
        let expired = self.expired_paste_wake_batch(Instant::now());
        if expired.adapter_routes.is_empty() && expired.ingress_sessions.is_empty() {
            return batch;
        }
        batch.adapter_routes.extend(expired.adapter_routes);
        batch.ingress_sessions.extend(expired.ingress_sessions);
        let mut seen_routes = HashSet::new();
        batch
            .adapter_routes
            .retain(|route| seen_routes.insert(route.clone()));
        let mut seen_sessions = HashSet::new();
        batch
            .ingress_sessions
            .retain(|session| seen_sessions.insert(session.clone()));
        batch
    }

    fn next_core_deadline(&self) -> Option<Instant> {
        [
            self.client_worker.next_paste_deadline(),
            self.client_worker.next_reader_deadline(),
            self.next_pending_resize_deadline(),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn next_pending_resize_deadline(&self) -> Option<Instant> {
        self.pending_terminal_resizes
            .iter()
            .filter(|(session_id, _)| self.session(session_id).is_some())
            .filter_map(|(_, pending)| pending.front().map(|entry| entry.deadline))
            .min()
    }

    fn expired_pending_resize_sessions(&self, now: Instant) -> Vec<SessionId> {
        let mut sessions: Vec<_> = self
            .pending_terminal_resizes
            .iter()
            .filter(|(session_id, pending)| {
                self.session(session_id).is_some()
                    && pending.front().is_some_and(|entry| entry.deadline <= now)
            })
            .map(|(session_id, _)| session_id.clone())
            .collect();
        sessions.sort_by(|left, right| left.0.cmp(&right.0));
        sessions
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
        let output = self.handle_session_request(
            SessionIoRequest::GetScreen {
                request_id,
                session_id: session_id.clone(),
            },
            now_seconds,
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
        let output = self.handle_session_request(
            SessionIoRequest::GetSnapshot {
                request_id,
                session_id: session_id.clone(),
            },
            now_seconds,
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
            let teardowns = self
                .client_worker
                .teardown_session(&session_id, TerminalRouteCloseReason::SessionEnded);
            self.pending_input_teardowns.extend(teardowns);
            let mut outcome =
                self.engine
                    .shutdown_session(session_id.clone(), reason, now_seconds)?;
            self.apply_client_worker(&mut outcome)?;
            self.pending_terminal_resizes.remove(&session_id);
            self.applied_terminal_resizes.remove(&session_id);
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
        self.pending_terminal_resizes.remove(&session_id);
        self.applied_terminal_resizes.remove(&session_id);
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
        mut teardowns: Vec<ClientWorkerTeardown>,
    ) -> Result<(), ManagedSessionRuntimeError> {
        teardowns.extend(
            self.client_worker
                .filter_bound_terminal_frames(&mut outcome.client_egress),
        );
        teardowns.splice(0..0, std::mem::take(&mut self.pending_input_teardowns));
        self.unsubscribe_owner_teardowns(outcome, &mut teardowns)
    }

    fn unsubscribe_owner_teardowns(
        &mut self,
        outcome: &mut MultiplexerEngineOutcome,
        teardowns: &mut Vec<ClientWorkerTeardown>,
    ) -> Result<(), ManagedSessionRuntimeError> {
        for teardown in teardowns.iter() {
            self.terminal_inventory_revision = self.terminal_inventory_revision.saturating_add(1);
            self.wake_source.notify_session(&teardown.session_id);
        }
        for teardown in teardowns.drain(..) {
            for key in &teardown.in_flight_keys {
                self.pending_worker_cancels
                    .push((teardown.session_id.clone(), *key));
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

    fn session_is_stopping_or_exited(&self, session_id: &SessionId) -> bool {
        matches!(
            self.session(session_id).map(|session| &session.lifecycle),
            Some(SessionLifecycleState::Exited { .. } | SessionLifecycleState::Stopping)
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

fn attach_route_error(error: AttachTerminalRouteError) -> ManagedSessionRuntimeError {
    ManagedSessionRuntimeError::Runtime(SessionRuntimeError::new(
        SessionRuntimeErrorKind::InputFailed,
        error.to_string(),
    ))
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
        let pty_replies = state.terminal.runtime_mut().drain_pty_writes();
        if !pty_replies.is_empty() {
            state.inputs.push(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: pty_replies,
            });
        }
    }

    /// Current terminal size of the local backend.
    pub(crate) fn size(&self) -> TerminalScreenSize {
        self.state.borrow().terminal.runtime().screen_state().size
    }

    /// Stream frames of the local backend snapshot.
    pub(crate) fn capture_snapshot_frames(
        &mut self,
    ) -> Result<
        Vec<(
            crate::contract::terminal_screen::TerminalSnapshotFramePhase,
            Vec<u8>,
        )>,
        ManagedSessionRuntimeError,
    > {
        let mut frames = Vec::new();
        self.state
            .borrow_mut()
            .terminal
            .runtime_mut()
            .capture_snapshot_frames(&mut |phase, bytes| frames.push((phase, bytes)))
            .map_err(managed_terminal_backend_error)?;
        Ok(frames)
    }

    /// Scheme 2 mode bits of the local backend, when it owns terminal modes.
    pub(crate) fn mode_bits(&self) -> Option<u32> {
        self.state
            .borrow()
            .terminal
            .runtime()
            .mode_flags()
            .ok()
            .map(|flags| flags.to_mode_bits())
    }

    /// Encode one staged operation with the local backend.
    ///
    /// Returns the PTY bytes, or the client-visible rejection.
    pub(crate) fn encode_local_input(
        &mut self,
        kind: WorkerInputKind,
        operation_id: u64,
        body: &[u8],
    ) -> Result<Vec<u8>, (InputOutcome, String)> {
        let mut state = self.state.borrow_mut();
        let backend = state.terminal.runtime_mut();
        let mut out = Vec::new();
        let protocol = |error: botster_terminal_protocol_client::TerminalInputDecodeError| {
            (InputOutcome::RejectedProtocol, error.to_string())
        };
        let failed = |error: TerminalBackendError| (InputOutcome::WriteFailed, error.to_string());
        match kind {
            WorkerInputKind::RawBytes => out.extend_from_slice(body),
            WorkerInputKind::Key => {
                let command = decode_input_body(TerminalInputKind::Key, operation_id, body)
                    .map_err(protocol)?;
                let TerminalInputCommand::Key {
                    action,
                    key,
                    mods,
                    consumed_mods,
                    composing,
                    unshifted_codepoint,
                    text,
                    ..
                } = command
                else {
                    return Err((InputOutcome::RejectedProtocol, "kind mismatch".to_owned()));
                };
                backend
                    .encode_key(
                        &TerminalKeyEvent {
                            action,
                            key,
                            mods,
                            consumed_mods,
                            composing,
                            unshifted_codepoint,
                            text: &text,
                        },
                        &mut out,
                    )
                    .map_err(failed)?;
            }
            WorkerInputKind::Mouse => {
                let command = decode_input_body(TerminalInputKind::Mouse, operation_id, body)
                    .map_err(protocol)?;
                let TerminalInputCommand::Mouse {
                    action,
                    button,
                    mods,
                    col,
                    row,
                    x_px,
                    y_px,
                    ..
                } = command
                else {
                    return Err((InputOutcome::RejectedProtocol, "kind mismatch".to_owned()));
                };
                backend
                    .encode_mouse(
                        &TerminalMouseEvent {
                            action,
                            button,
                            mods,
                            col,
                            row,
                            x_px,
                            y_px,
                        },
                        &mut out,
                    )
                    .map_err(failed)?;
            }
            WorkerInputKind::Focus => {
                let command = decode_input_body(TerminalInputKind::Focus, operation_id, body)
                    .map_err(protocol)?;
                let TerminalInputCommand::Focus { focused, .. } = command else {
                    return Err((InputOutcome::RejectedProtocol, "kind mismatch".to_owned()));
                };
                backend.encode_focus(focused, &mut out).map_err(failed)?;
            }
            WorkerInputKind::Resize => {
                return Err((
                    InputOutcome::RejectedProtocol,
                    "resize is applied by the runtime, not encoded".to_owned(),
                ));
            }
            WorkerInputKind::Paste => {
                let Some((allow_unsafe, data)) = body.split_first() else {
                    return Err((
                        InputOutcome::RejectedProtocol,
                        "paste body is empty".to_owned(),
                    ));
                };
                if *allow_unsafe == 0 && !backend.paste_is_safe(data) {
                    return Err((
                        InputOutcome::RejectedUnsafePaste,
                        "paste contains newlines or a bracketed paste end marker".to_owned(),
                    ));
                }
                let mut scratch = data.to_vec();
                backend
                    .encode_paste(&mut scratch, &mut out)
                    .map_err(failed)?;
            }
        }
        Ok(out)
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
        Ok(ModeFlagsReady {
            request_id,
            session_id,
            mode_flags,
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

    #[cfg(all(unix, feature = "local-runtime"))]
    mod expired_pending_resize_guard {
        use super::*;
        use crate::contract::terminal_adapter::{
            TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
        };
        use crate::contract::terminal_wake::TerminalWakeSink;
        use crate::runtime::ControlWriterError;
        use crate::{SpawnEnvironment, SpawnWorkingDirectory};
        fn worker_path() -> std::path::PathBuf {
            botster_core_test_support::real_worker::WorkerBinary::from_env()
                .unwrap_or_else(|failure| panic!("{failure}"))
                .path
        }

        struct QuietAdapter;

        impl TerminalAdapter for QuietAdapter {
            fn try_write(
                &mut self,
                _frame: &botster_terminal_protocol::RoutedTerminalFrame,
            ) -> Result<(), TerminalAdapterWriteError> {
                Ok(())
            }

            fn close(&mut self, _reason: TerminalRouteCloseReason) {}

            fn pressure(&self) -> TerminalAdapterPressure {
                TerminalAdapterPressure::Ready
            }

            fn try_read(&mut self) -> TerminalIngress {
                TerminalIngress::Empty
            }
        }

        impl crate::contract::terminal_wake::WakingTerminalAdapter for QuietAdapter {
            fn set_wake_sink(&mut self, _sink: TerminalWakeSink) {}
        }

        #[test]
        fn the_host_wait_ends_at_a_dead_readers_deadline_and_names_its_route() {
            let session_id = SessionId("dead-reader-deadline".into());
            let subscription_id = SubscriptionId("dead-reader-deadline-sub".into());
            let mut runtime = ManagedSessionRuntime::with_worker_process(worker_path());
            runtime
                .spawn_session(
                    SessionSpawnRequest {
                        request_id: RequestId("dead-reader-deadline-spawn".into()),
                        session_id: session_id.clone(),
                        executable: "sh".to_string(),
                        arguments: vec!["-c".to_string(), "exec cat".to_string()],
                        working_directory: SpawnWorkingDirectory {
                            path: ".".to_string(),
                        },
                        environment: SpawnEnvironment::default(),
                        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
                    },
                    CoreSessionMetadata::new(),
                )
                .expect("spawn worker session");
            runtime
                .test_bind_owner(
                    ClientId("dead-reader-deadline-client".into()),
                    session_id.clone(),
                    subscription_id.clone(),
                    Box::new(QuietAdapter),
                )
                .expect("bind owner");
            let expired_at = Instant::now()
                .checked_sub(crate::engine::client_worker::READER_PROGRESS_DEADLINE)
                .expect("clock");
            runtime.client_worker.test_start_reader_block(
                &session_id,
                &subscription_id,
                expired_at,
            );

            assert_eq!(
                runtime.clamp_paste_wait(Duration::from_secs(3_600)),
                Duration::ZERO,
                "the host wait may not outlast the reader deadline"
            );
            // timer: deadline — the clamp above makes this wait return at once
            let batch = runtime.wait_wakes(Duration::from_secs(5));
            assert!(batch.adapter_routes.contains(&crate::TerminalWakeRoute {
                session_id,
                subscription_id,
            }));
        }

        #[test]
        fn expired_pending_resize_skips_control_failure_when_engine_is_exited_and_worker_remains() {
            let session_id = SessionId("guard-exited-pending-resize".into());
            let mut runtime = ManagedSessionRuntime::with_worker_process(worker_path());
            runtime
                .spawn_session(
                    SessionSpawnRequest {
                        request_id: RequestId("guard-exited-pending-resize-spawn".into()),
                        session_id: session_id.clone(),
                        executable: "sh".to_string(),
                        arguments: vec!["-c".to_string(), "printf ready; sleep 30".to_string()],
                        working_directory: SpawnWorkingDirectory {
                            path: ".".to_string(),
                        },
                        environment: SpawnEnvironment::default(),
                        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
                    },
                    CoreSessionMetadata::new(),
                )
                .expect("spawn worker session");
            runtime
                .test_bind_owner(
                    ClientId("guard-exited-pending-resize-client".into()),
                    session_id.clone(),
                    SubscriptionId("guard-exited-pending-resize-sub".into()),
                    Box::new(QuietAdapter),
                )
                .expect("bind owner");
            assert!(
                runtime.session_runtime().test_has_session(&session_id),
                "worker map entry must remain so take_resize_applied returns Ok"
            );
            runtime.test_insert_expired_pending_resize(session_id.clone());
            runtime
                .test_set_lifecycle(
                    session_id.clone(),
                    SessionLifecycleState::Exited { code: Some(0) },
                )
                .expect("force engine Exited while the worker entry remains");
            assert!(matches!(
                runtime
                    .session(&session_id)
                    .map(|session| &session.lifecycle),
                Some(SessionLifecycleState::Exited { .. })
            ));
            assert_eq!(runtime.pending_terminal_resize_len(&session_id), 1);
            assert_eq!(runtime.test_pending_input_teardown_count(), 0);

            let mut named = HashSet::new();
            named.insert(session_id.clone());
            runtime
                .reconcile_terminal_resize_acknowledgments(&named)
                .expect("reconcile expired pending against an exited engine session");

            assert!(
                runtime.session_runtime().test_has_session(&session_id),
                "guard test must not go through SessionNotFound"
            );
            assert_eq!(runtime.pending_terminal_resize_len(&session_id), 0);
            assert!(!matches!(
                runtime.control_plane_state(&session_id),
                ControlPlaneState::Failed(ControlWriterError::ResizeAckTimeout)
            ));
            assert_eq!(runtime.test_pending_input_teardown_count(), 0);
            assert!(runtime.adapter_is_bound(
                &session_id,
                &SubscriptionId("guard-exited-pending-resize-sub".into())
            ));
            let _ = runtime.shutdown_session(session_id, "test cleanup", 1);
        }
    }
    #[cfg(all(unix, feature = "local-runtime"))]
    #[test]
    fn a_writer_failure_after_an_accepted_cancel_ends_the_control_link() {
        use std::io::Read;

        let mut runtime =
            ManagedSessionRuntime::with_worker_process("/missing/botster-session-worker");
        let session_id = SessionId("accepted-then-lost".into());
        let mut peer = runtime
            .session_runtime_mut()
            .insert_test_socket_session(session_id.clone());
        peer.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("bounded peer read");
        let request_id = runtime
            .session_runtime_mut()
            .begin_snapshot_boundary(&session_id)
            .expect("begin barrier");
        assert_eq!(
            runtime
                .session_runtime_mut()
                .cancel_snapshot_boundary(&session_id, &request_id)
                .expect("cancel"),
            crate::runtime::SnapshotCancelAdmission::Accepted
        );
        assert!(!runtime
            .session_runtime()
            .snapshot_request_is_outstanding(&session_id, &request_id));

        // The writer dies before the admitted cancel is delivered.
        runtime
            .session_runtime()
            .test_fail_control_writer(&session_id, ControlWriterError::DeadlineExpired);
        let batch = TerminalWakeBatch {
            adapter_routes: Vec::new(),
            ingress_sessions: vec![session_id.clone()],
        };
        let mut outcome = MultiplexerEngineOutcome::empty();
        runtime
            .apply_woken_terminal_input(&batch, 0, &HashSet::new(), &mut outcome)
            .expect("production writer-failure handling");

        assert_eq!(
            runtime.control_plane_state(&session_id),
            ControlPlaneState::Failed(ControlWriterError::DeadlineExpired)
        );
        let mut buf = [0u8; 16];
        assert_eq!(
            peer.read(&mut buf).expect("peer read within the bound"),
            0,
            "the worker side must observe EOF on the control link"
        );
    }
}
