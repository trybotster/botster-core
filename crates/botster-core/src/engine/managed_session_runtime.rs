//! Scheduling-neutral managed session runtime over core engine primitives.

use std::cell::RefCell;
use std::rc::Rc;

use thiserror::Error;

use crate::contract::actor::{
    ModeFlagsReady, PreparedSnapshotReady, ScreenReady, SendFileFailed, SendFileRequest,
    SendFileWritten, SessionIoRequest, SnapshotReady,
};
use crate::engine::multiplexer::{
    MultiplexerEngine, MultiplexerEngineError, MultiplexerEngineOutcome, MultiplexerSpawnOutcome,
};
use crate::engine::session_worker::SessionWorkerRuntime;
use crate::runtime::{
    SessionRuntime, SessionRuntimeError, SessionRuntimeInput, SessionRuntimeOutput,
    SessionSpawnRequest,
};
use crate::session::{CoreSessionMetadata, RequestId, SessionActivityStatus, SessionId};
use crate::session_protocol::{ModeFlags, ResizePayload, TerminalColorProfile};
use crate::transport::TransportIngress;
use crate::ClientId;

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
}

/// Scheduling-neutral coordinator for one or more managed live sessions.
///
/// Hosts still choose the executor, thread, or event loop that calls these
/// methods. This type defines the reusable semantics for routing client writes
/// into `SessionRuntimeInput` and draining runtime output through the existing
/// session worker and subscription multiplexer path. Terminal-state requests
/// such as snapshots and screen reads require a host-owned terminal state
/// worker and are rejected instead of fabricated here.
#[derive(Clone)]
pub struct ManagedSessionRuntime<R>
where
    R: SessionRuntime,
{
    engine: MultiplexerEngine<R, SessionRuntimeWorkerAdapter>,
}

impl<R> ManagedSessionRuntime<R>
where
    R: SessionRuntime,
{
    /// Build a managed runtime around a host session runtime.
    #[must_use]
    pub fn new(runtime: R) -> Self {
        Self {
            engine: MultiplexerEngine::new(runtime),
        }
    }

    /// Return a recorded session from the assembled core engine.
    #[must_use]
    pub fn session(&self, session_id: &SessionId) -> Option<&crate::CoreSession> {
        self.engine.session(session_id)
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
        Ok(self
            .engine
            .spawn_session(request, metadata, SessionRuntimeWorkerAdapter::new())?)
    }

    /// Route one client ingress frame through the existing multiplexer path.
    pub fn handle_client_ingress(
        &mut self,
        client_id: ClientId,
        ingress: TransportIngress,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        reject_unsupported_ingress(&ingress)?;
        let outcome = self
            .engine
            .handle_client_ingress(client_id, ingress, now_seconds)?;
        self.flush_runtime_inputs()?;
        Ok(outcome)
    }

    /// Route one session I/O request through the existing session worker path.
    pub fn handle_session_request(
        &mut self,
        request: SessionIoRequest,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        reject_unsupported_session_request(&request)?;
        let outcome = self.engine.handle_session_request(request, now_seconds)?;
        self.flush_runtime_inputs()?;
        Ok(outcome)
    }

    /// Drain currently available runtime output once for a session.
    pub fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let outputs = self.engine.session_runtime_mut().drain_output(session_id)?;
        let mut outcome = MultiplexerEngineOutcome::empty();

        // Runtime drains are output-only; worker input buffers are populated by
        // request routing paths and are flushed by those mutators.
        for output in outputs {
            let runtime_event = match output {
                SessionRuntimeOutput::PtyOutput { session_id, data } => {
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
            };
            let step = self.engine.handle_runtime_event(runtime_event)?;
            outcome.client_egress.extend(step.client_egress);
            outcome.session_requests.extend(step.session_requests);
            outcome
                .client_control_frames
                .extend(step.client_control_frames);
            outcome.session_events.extend(step.session_events);
            outcome.observations.extend(step.observations);
        }

        Ok(outcome)
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

    /// Shut down a managed session through the worker/runtime path.
    pub fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<MultiplexerEngineOutcome, ManagedSessionRuntimeError> {
        let outcome = self
            .engine
            .shutdown_session(session_id, reason, now_seconds)?;
        self.flush_runtime_inputs()?;
        Ok(outcome)
    }

    fn flush_runtime_inputs(&mut self) -> Result<(), SessionRuntimeError> {
        let session_ids = self.engine_session_ids();
        for session_id in session_ids {
            let inputs = self
                .engine_worker(&session_id)
                .map(SessionRuntimeWorkerAdapter::drain_inputs)
                .unwrap_or_default();
            for input in inputs {
                self.engine.session_runtime_mut().send_input(input)?;
            }
        }
        Ok(())
    }

    fn engine_session_ids(&self) -> Vec<SessionId> {
        self.engine.session_ids()
    }

    fn engine_worker(
        &mut self,
        session_id: &SessionId,
    ) -> Option<&mut SessionRuntimeWorkerAdapter> {
        self.engine.session_worker_runtime_mut(session_id)
    }
}

/// Session worker adapter that converts PTY I/O and lifecycle operations into runtime inputs.
#[derive(Debug, Clone, Default)]
pub(crate) struct SessionRuntimeWorkerAdapter {
    state: Rc<RefCell<SessionRuntimeWorkerState>>,
}

impl SessionRuntimeWorkerAdapter {
    /// Build an adapter with no terminal-state backing.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Drain pending runtime inputs recorded by worker operations.
    pub(crate) fn drain_inputs(&mut self) -> Vec<SessionRuntimeInput> {
        self.state.borrow_mut().inputs.drain(..).collect()
    }
}

#[derive(Debug, Clone)]
struct SessionRuntimeWorkerState {
    inputs: Vec<SessionRuntimeInput>,
    rows: u16,
    cols: u16,
}

impl Default for SessionRuntimeWorkerState {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            rows: 24,
            cols: 80,
        }
    }
}

impl SessionWorkerRuntime for SessionRuntimeWorkerAdapter {
    fn write_input(&mut self, session_id: &SessionId, data: &[u8]) {
        self.state
            .borrow_mut()
            .inputs
            .push(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: data.to_vec(),
            });
    }

    fn resize(&mut self, session_id: &SessionId, rows: u16, cols: u16) {
        let mut state = self.state.borrow_mut();
        state.rows = rows;
        state.cols = cols;
        state.inputs.push(SessionRuntimeInput::Resize {
            session_id: session_id.clone(),
            size: ResizePayload { rows, cols },
        });
    }

    fn snapshot(&mut self, request_id: RequestId, session_id: SessionId) -> SnapshotReady {
        let state = self.state.borrow();
        SnapshotReady {
            request_id,
            session_id,
            data: Vec::new(),
            rows: state.rows,
            cols: state.cols,
        }
    }

    fn request_initial_snapshot(&mut self, _request: crate::InitialSnapshotRequest) {}

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

    fn mode_flags(&mut self, request_id: RequestId, session_id: SessionId) -> ModeFlagsReady {
        ModeFlagsReady {
            request_id,
            session_id,
            mode_flags: ModeFlags::default(),
        }
    }

    fn screen(&mut self, request_id: RequestId, session_id: SessionId) -> ScreenReady {
        ScreenReady {
            request_id,
            session_id,
            text: String::new(),
        }
    }

    fn set_color_profile(&mut self, _session_id: &SessionId, _color_profile: TerminalColorProfile) {
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
        TransportIngress::RequestSnapshot { .. } => unsupported("request_snapshot"),
        TransportIngress::SendFile { .. } => unsupported("send_file"),
        TransportIngress::SubscribeSession { .. }
        | TransportIngress::UnsubscribeSession { .. }
        | TransportIngress::TerminalInput { .. }
        | TransportIngress::Resize { .. }
        | TransportIngress::Focus { .. }
        | TransportIngress::Heartbeat { .. }
        | TransportIngress::BoundaryPayload { .. }
        | TransportIngress::ClientState { .. }
        | TransportIngress::Ping { .. } => Ok(()),
    }
}

fn reject_unsupported_session_request(
    request: &SessionIoRequest,
) -> Result<(), ManagedSessionRuntimeError> {
    match request {
        SessionIoRequest::SubscribeTerminal { .. } => unsupported("subscribe_terminal"),
        SessionIoRequest::GetSnapshot { .. } => unsupported("get_snapshot"),
        SessionIoRequest::GetInitialSnapshot(_) => unsupported("get_initial_snapshot"),
        SessionIoRequest::SendFile(_) => unsupported("send_file"),
        SessionIoRequest::PrepareSnapshot(_) => unsupported("prepare_snapshot"),
        SessionIoRequest::GetModeFlags { .. } => unsupported("get_mode_flags"),
        SessionIoRequest::GetScreen { .. } => unsupported("get_screen"),
        SessionIoRequest::SetColorProfile { .. } => unsupported("set_color_profile"),
        SessionIoRequest::UnsubscribeTerminal { .. }
        | SessionIoRequest::PtyInput { .. }
        | SessionIoRequest::Resize { .. }
        | SessionIoRequest::Shutdown { .. } => Ok(()),
    }
}

fn unsupported(request_kind: &'static str) -> Result<(), ManagedSessionRuntimeError> {
    Err(ManagedSessionRuntimeError::UnsupportedSessionRequest { request_kind })
}
