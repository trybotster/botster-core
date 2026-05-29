//! Fake runtime adapters for downstream conformance tests.

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
