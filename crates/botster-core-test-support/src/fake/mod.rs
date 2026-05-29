//! Fake runtime adapters for downstream conformance tests.

pub mod session_worker;

use botster_core::client::ClientId;
use botster_core::session::SubscriptionId;
use botster_core::transport::{TransportEgress, TransportIngress};
use botster_core::{
    ProcessExitedPayload, ProcessIdentity, SessionId, SessionRuntime, SessionRuntimeError,
    SessionRuntimeErrorKind, SessionRuntimeHandle, SessionRuntimeInput, SessionRuntimeOutput,
    SessionSpawnRequest,
};

/// Deterministic fake implementation of the host session runtime contract.
#[derive(Debug, Clone, Default)]
pub struct FakeSessionRuntime {
    spawned: Vec<SessionSpawnRequest>,
    handles: Vec<SessionRuntimeHandle>,
    inputs: Vec<SessionRuntimeInput>,
    outputs: Vec<SessionRuntimeOutput>,
    next_spawn_error: Option<SessionRuntimeError>,
}

impl FakeSessionRuntime {
    /// Create an empty fake runtime.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure the next spawn attempt to fail with a typed runtime error.
    pub fn fail_next_spawn(&mut self, error: SessionRuntimeError) {
        self.next_spawn_error = Some(error);
    }

    /// Return all spawn requests observed by the fake runtime.
    pub fn spawned(&self) -> &[SessionSpawnRequest] {
        &self.spawned
    }

    /// Return all runtime inputs observed by the fake runtime.
    pub fn inputs(&self) -> &[SessionRuntimeInput] {
        &self.inputs
    }

    /// Queue raw PTY output for a session.
    pub fn emit_output(&mut self, session_id: SessionId, data: Vec<u8>) {
        self.outputs
            .push(SessionRuntimeOutput::PtyOutput { session_id, data });
    }

    /// Queue process exit output for a session.
    pub fn emit_exit(&mut self, session_id: SessionId, payload: ProcessExitedPayload) {
        self.outputs.push(SessionRuntimeOutput::ProcessExited {
            session_id,
            payload,
        });
    }

    fn session_exists(&self, session_id: &SessionId) -> bool {
        self.handles
            .iter()
            .any(|handle| &handle.session_id == session_id)
    }

    fn input_session_id(input: &SessionRuntimeInput) -> &SessionId {
        match input {
            SessionRuntimeInput::PtyInput { session_id, .. }
            | SessionRuntimeInput::Resize { session_id, .. }
            | SessionRuntimeInput::Shutdown { session_id } => session_id,
        }
    }
}

impl SessionRuntime for FakeSessionRuntime {
    fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
        if let Some(error) = self.next_spawn_error.take() {
            return Err(error);
        }

        let handle = SessionRuntimeHandle {
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            process: ProcessIdentity {
                pid: Some((self.handles.len() as u32) + 1),
                runtime_id: Some(format!("fake-process-{}", self.handles.len() + 1)),
            },
        };

        self.spawned.push(request);
        self.handles.push(handle.clone());
        Ok(handle)
    }

    fn send_input(&mut self, input: SessionRuntimeInput) -> Result<(), SessionRuntimeError> {
        if !self.session_exists(Self::input_session_id(&input)) {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SessionNotFound,
                "session has not been spawned",
            ));
        }

        self.inputs.push(input);
        Ok(())
    }

    fn drain_output(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        if !self.session_exists(session_id) {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SessionNotFound,
                "session has not been spawned",
            ));
        }

        let mut drained = Vec::new();
        let mut retained = Vec::new();

        for output in self.outputs.drain(..) {
            let output_session_id = match &output {
                SessionRuntimeOutput::PtyOutput { session_id, .. }
                | SessionRuntimeOutput::ProcessExited { session_id, .. } => session_id,
            };

            if output_session_id == session_id {
                drained.push(output);
            } else {
                retained.push(output);
            }
        }

        self.outputs = retained;
        Ok(drained)
    }
}

/// In-memory transport contract recorder for downstream tests.
///
/// The fake records public `botster_core` ingress and egress frames only. It
/// does not simulate hub policy, client workers, PTYs, providers, or plugin
/// execution.
#[derive(Debug, Clone)]
pub struct FakeSessionTransport {
    client_id: ClientId,
    session_id: SessionId,
    subscription_id: SubscriptionId,
    ingress: Vec<TransportIngress>,
    egress: Vec<TransportEgress>,
}

impl FakeSessionTransport {
    /// Create a fake transport recorder for one client/session subscription.
    pub fn new(
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> Self {
        Self {
            client_id,
            session_id,
            subscription_id,
            ingress: Vec::new(),
            egress: Vec::new(),
        }
    }

    /// Return the session id this fake records.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Return the subscription id this fake records.
    pub fn subscription_id(&self) -> &SubscriptionId {
        &self.subscription_id
    }

    /// Record a public subscribe ingress frame.
    pub fn subscribe(&mut self) {
        self.ingress.push(TransportIngress::SubscribeSession {
            client_id: self.client_id.clone(),
            session_id: self.session_id.clone(),
            subscription_id: self.subscription_id.clone(),
        });
    }

    /// Record public terminal input bytes.
    pub fn terminal_input(&mut self, data: impl Into<Vec<u8>>) {
        self.ingress.push(TransportIngress::TerminalInput {
            session_id: self.session_id.clone(),
            data: data.into(),
        });
    }

    /// Record a public snapshot request.
    pub fn request_snapshot(&mut self, request_id: botster_core::RequestId) {
        self.ingress.push(TransportIngress::RequestSnapshot {
            request_id,
            session_id: self.session_id.clone(),
        });
    }

    /// Record public terminal output bytes.
    pub fn terminal_output(&mut self, data: impl Into<Vec<u8>>) {
        self.egress.push(TransportEgress::TerminalOutput {
            session_id: self.session_id.clone(),
            subscription_id: self.subscription_id.clone(),
            data: data.into(),
        });
    }

    /// Recorded ingress frames.
    pub fn ingress(&self) -> &[TransportIngress] {
        &self.ingress
    }

    /// Recorded egress frames.
    pub fn egress(&self) -> &[TransportEgress] {
        &self.egress
    }
}

pub use session_worker::{FakeSessionIoMailbox, FakeSessionWorkerRuntime, RuntimeCommand};
