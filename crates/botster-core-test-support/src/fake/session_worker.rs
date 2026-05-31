//! Fake session worker runtime and mailbox helpers.

use botster_core::{
    BackpressureRoute, MailboxSendFailure, MailboxSendFailureReason, ModeFlags, ModeFlagsReady,
    PreparedSnapshotReady, PreparedSnapshotRequest, QueueSource, RequestId, ScreenReady,
    SendFileFailed, SendFileRequest, SendFileWritten, SessionId, SessionIoRequest,
    SessionRuntimeError, SessionWorkerRuntime, SessionWorkerRuntimeEvent, SnapshotReady,
    TerminalColorProfile,
};

/// Runtime command recorded by the fake session runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCommand {
    /// Terminal input was written.
    WriteInput {
        /// Session receiving input.
        session_id: SessionId,
        /// Input bytes.
        data: Vec<u8>,
    },
    /// Terminal was resized.
    Resize {
        /// Session being resized.
        session_id: SessionId,
        /// Row count.
        rows: u16,
        /// Column count.
        cols: u16,
    },
    /// Initial snapshot was requested.
    RequestInitialSnapshot {
        /// Request id.
        request_id: RequestId,
        /// Session being snapshotted.
        session_id: SessionId,
        /// Row count used by the request.
        rows: u16,
        /// Column count used by the request.
        cols: u16,
    },
    /// Color profile was replaced.
    SetColorProfile {
        /// Session receiving the profile.
        session_id: SessionId,
        /// Profile payload.
        color_profile: TerminalColorProfile,
    },
    /// Session runtime was shut down.
    Shutdown {
        /// Session being shut down.
        session_id: SessionId,
        /// Shutdown reason.
        reason: String,
    },
}

/// Fake runtime for session worker conformance tests.
#[derive(Debug, Clone)]
pub struct FakeSessionWorkerRuntime {
    commands: Vec<RuntimeCommand>,
    rows: u16,
    cols: u16,
    snapshot: Vec<u8>,
    send_storage_ref: Option<String>,
}

impl Default for FakeSessionWorkerRuntime {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            rows: 24,
            cols: 80,
            snapshot: b"snapshot".to_vec(),
            send_storage_ref: Some("opaque-send-file-1".to_string()),
        }
    }
}

impl FakeSessionWorkerRuntime {
    /// Build a fake runtime with default terminal state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Commands recorded by this runtime.
    #[must_use]
    pub const fn commands(&self) -> &Vec<RuntimeCommand> {
        &self.commands
    }
}

impl SessionWorkerRuntime for FakeSessionWorkerRuntime {
    fn write_input(&mut self, session_id: &SessionId, data: &[u8]) {
        self.commands.push(RuntimeCommand::WriteInput {
            session_id: session_id.clone(),
            data: data.to_vec(),
        });
    }

    fn resize(&mut self, session_id: &SessionId, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        self.commands.push(RuntimeCommand::Resize {
            session_id: session_id.clone(),
            rows,
            cols,
        });
    }

    fn snapshot(&mut self, request_id: RequestId, session_id: SessionId) -> SnapshotReady {
        SnapshotReady {
            request_id,
            session_id,
            data: self.snapshot.clone(),
            rows: self.rows,
            cols: self.cols,
        }
    }

    fn request_initial_snapshot(&mut self, request: botster_core::InitialSnapshotRequest) {
        self.commands.push(RuntimeCommand::RequestInitialSnapshot {
            request_id: request.request_id,
            session_id: request.session_id,
            rows: request.rows,
            cols: request.cols,
        });
    }

    fn send_file(&mut self, request: SendFileRequest) -> Result<SendFileWritten, SendFileFailed> {
        Ok(SendFileWritten {
            request_id: request.request_id,
            session_id: request.session_id,
            bytes: request.data.len(),
            storage_ref: self.send_storage_ref.clone(),
        })
    }

    fn prepare_snapshot(&mut self, request: PreparedSnapshotRequest) -> PreparedSnapshotReady {
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
            mode_flags: ModeFlags {
                cursor_visible: true,
                ..ModeFlags::default()
            },
        }
    }

    fn screen(&mut self, request_id: RequestId, session_id: SessionId) -> ScreenReady {
        ScreenReady {
            request_id,
            session_id,
            text: "screen".to_string(),
        }
    }

    fn set_color_profile(&mut self, session_id: &SessionId, color_profile: TerminalColorProfile) {
        self.commands.push(RuntimeCommand::SetColorProfile {
            session_id: session_id.clone(),
            color_profile,
        });
    }

    fn shutdown(
        &mut self,
        session_id: &SessionId,
        reason: &str,
    ) -> Result<Vec<SessionWorkerRuntimeEvent>, SessionRuntimeError> {
        self.commands.push(RuntimeCommand::Shutdown {
            session_id: session_id.clone(),
            reason: reason.to_string(),
        });
        Ok(Vec::new())
    }
}

/// Fake bounded mailbox that reports core queue failures.
#[derive(Debug, Clone)]
pub struct FakeSessionIoMailbox {
    capacity: usize,
    closed: bool,
    requests: Vec<SessionIoRequest>,
    route: BackpressureRoute,
}

impl FakeSessionIoMailbox {
    /// Build a fake session I/O mailbox.
    #[must_use]
    pub fn new(capacity: usize, route: BackpressureRoute) -> Self {
        Self {
            capacity,
            closed: false,
            requests: Vec::new(),
            route,
        }
    }

    /// Close this mailbox.
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// Queued requests.
    #[must_use]
    pub const fn requests(&self) -> &Vec<SessionIoRequest> {
        &self.requests
    }

    /// Send a request into the fake mailbox.
    pub fn send(&mut self, request: SessionIoRequest) -> Result<(), MailboxSendFailure> {
        if self.closed {
            return Err(self.failure(MailboxSendFailureReason::QueueClosed));
        }

        if self.requests.len() >= self.capacity {
            return Err(self.failure(MailboxSendFailureReason::QueueFull));
        }

        self.requests.push(request);
        Ok(())
    }

    fn failure(&self, reason: MailboxSendFailureReason) -> MailboxSendFailure {
        MailboxSendFailure {
            source: QueueSource::SessionIo,
            route: self.route.clone(),
            reason,
        }
    }
}
