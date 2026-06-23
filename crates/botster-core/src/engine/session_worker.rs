//! Session worker state machine for typed session I/O requests.

use crate::contract::actor::{
    InitialSnapshotBarrier, InitialSnapshotReady, MailboxSendFailure, ModeFlagsReady,
    PreparedSnapshotReady, ScreenReady, SendFileFailed, SendFileWritten, SessionIoEvent,
    SessionIoRequest, SnapshotReady,
};
use crate::contract::session::SessionId;
use crate::contract::session_protocol::ProcessExitedPayload;
use crate::runtime::SessionRuntimeError;

/// Host-supplied operations used by the reusable session worker engine.
pub trait SessionWorkerRuntime {
    /// Write terminal input bytes to the session runtime.
    fn write_input(&mut self, session_id: &SessionId, data: &[u8]);

    /// Resize the terminal runtime.
    fn resize(&mut self, session_id: &SessionId, rows: u16, cols: u16);

    /// Produce a terminal snapshot for a request.
    fn snapshot(&mut self, request_id: crate::RequestId, session_id: SessionId) -> SnapshotReady;

    /// Request an authoritative initial snapshot of the current terminal state.
    ///
    /// Callers that need a resized snapshot must resize before this request.
    fn request_initial_snapshot(&mut self, request: crate::InitialSnapshotRequest);

    /// Prepare send-file payload storage.
    fn send_file(
        &mut self,
        request: crate::SendFileRequest,
    ) -> Result<SendFileWritten, SendFileFailed>;

    /// Prepare a terminal snapshot payload.
    fn prepare_snapshot(
        &mut self,
        request: crate::PreparedSnapshotRequest,
    ) -> PreparedSnapshotReady;

    /// Read current terminal mode flags.
    fn mode_flags(&mut self, request_id: crate::RequestId, session_id: SessionId)
        -> ModeFlagsReady;

    /// Read plain screen contents.
    fn screen(&mut self, request_id: crate::RequestId, session_id: SessionId) -> ScreenReady;

    /// Replace the terminal color profile.
    fn set_color_profile(
        &mut self,
        session_id: &SessionId,
        color_profile: crate::TerminalColorProfile,
    );

    /// Shut down the runtime side of the session.
    fn shutdown(
        &mut self,
        session_id: &SessionId,
        reason: &str,
    ) -> Result<Vec<SessionWorkerRuntimeEvent>, SessionRuntimeError>;
}

/// Runtime-originated events accepted by the session worker engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionWorkerRuntimeEvent {
    /// Terminal bytes arrived from the runtime.
    TerminalBytes {
        /// Session that emitted bytes.
        session_id: SessionId,
        /// Terminal bytes.
        data: Vec<u8>,
        /// Runtime-owned last-output timestamp.
        last_output_at: u64,
    },
    /// The authoritative initial snapshot is ready.
    InitialSnapshotReady(InitialSnapshotReady),
    /// The child process exited.
    ProcessExited {
        /// Session that exited.
        session_id: SessionId,
        /// Process exit payload.
        payload: ProcessExitedPayload,
    },
}

/// Events and observations produced by a worker engine step.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionWorkerOutcome {
    /// Session I/O events emitted by this step.
    pub events: Vec<SessionIoEvent>,
    /// Mailbox failures observed at this step.
    pub mailbox_failures: Vec<MailboxSendFailure>,
    /// Current output activity timestamp, when known.
    pub last_output_at: Option<u64>,
}

impl SessionWorkerOutcome {
    fn from_events(events: Vec<SessionIoEvent>, last_output_at: Option<u64>) -> Self {
        Self {
            events,
            mailbox_failures: Vec::new(),
            last_output_at,
        }
    }
}

/// Reusable session worker engine over typed core contracts.
#[derive(Debug, Clone)]
pub struct SessionWorkerEngine<R> {
    runtime: R,
    initial_snapshot_barrier: Option<InitialSnapshotBarrier>,
    pending_initial_output: Vec<Vec<u8>>,
    closed: bool,
    last_output_at: Option<u64>,
}

impl<R> SessionWorkerEngine<R>
where
    R: SessionWorkerRuntime,
{
    /// Build a session worker engine with a host-supplied runtime adapter.
    pub fn new(runtime: R) -> Self {
        Self {
            runtime,
            initial_snapshot_barrier: None,
            pending_initial_output: Vec::new(),
            closed: false,
            last_output_at: None,
        }
    }

    /// Return an immutable view of the runtime adapter.
    pub const fn runtime(&self) -> &R {
        &self.runtime
    }

    /// Return a mutable view of the runtime adapter.
    pub const fn runtime_mut(&mut self) -> &mut R {
        &mut self.runtime
    }

    /// Current output activity timestamp.
    pub const fn last_output_at(&self) -> Option<u64> {
        self.last_output_at
    }

    /// Whether the engine has shut down.
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Handle one typed session I/O request.
    pub fn handle_request(
        &mut self,
        request: SessionIoRequest,
    ) -> Result<SessionWorkerOutcome, SessionRuntimeError> {
        if self.closed {
            return Ok(SessionWorkerOutcome::from_events(
                Vec::new(),
                self.last_output_at,
            ));
        }

        let outcome = match request {
            SessionIoRequest::SubscribeTerminal {
                request_id,
                session_id,
                client_id,
                subscription_id,
                rows,
                cols,
            } => {
                self.initial_snapshot_barrier = Some(InitialSnapshotBarrier::new());
                self.runtime
                    .request_initial_snapshot(crate::InitialSnapshotRequest {
                        request_id,
                        session_id,
                        client_id,
                        subscription_id,
                        rows,
                        cols,
                    });
                Ok(SessionWorkerOutcome::from_events(
                    Vec::new(),
                    self.last_output_at,
                ))
            }
            SessionIoRequest::UnsubscribeTerminal { .. } => Ok(SessionWorkerOutcome::from_events(
                Vec::new(),
                self.last_output_at,
            )),
            SessionIoRequest::PtyInput { session_id, data } => {
                self.runtime.write_input(&session_id, &data);
                Ok(SessionWorkerOutcome::from_events(
                    Vec::new(),
                    self.last_output_at,
                ))
            }
            SessionIoRequest::Resize {
                session_id,
                rows,
                cols,
            } => {
                self.runtime.resize(&session_id, rows, cols);
                Ok(SessionWorkerOutcome::from_events(
                    Vec::new(),
                    self.last_output_at,
                ))
            }
            SessionIoRequest::GetSnapshot {
                request_id,
                session_id,
            } => {
                let event =
                    SessionIoEvent::SnapshotReady(self.runtime.snapshot(request_id, session_id));
                Ok(SessionWorkerOutcome::from_events(
                    vec![event],
                    self.last_output_at,
                ))
            }
            SessionIoRequest::GetInitialSnapshot(request) => {
                self.runtime
                    .resize(&request.session_id, request.rows, request.cols);
                self.initial_snapshot_barrier = Some(InitialSnapshotBarrier::new());
                self.runtime.request_initial_snapshot(request);
                Ok(SessionWorkerOutcome::from_events(
                    Vec::new(),
                    self.last_output_at,
                ))
            }
            SessionIoRequest::SendFile(request) => {
                let event = match self.runtime.send_file(request) {
                    Ok(written) => SessionIoEvent::SendFileWritten(written),
                    Err(failed) => SessionIoEvent::SendFileFailed(failed),
                };
                Ok(SessionWorkerOutcome::from_events(
                    vec![event],
                    self.last_output_at,
                ))
            }
            SessionIoRequest::PrepareSnapshot(request) => {
                let event =
                    SessionIoEvent::PreparedSnapshotReady(self.runtime.prepare_snapshot(request));
                Ok(SessionWorkerOutcome::from_events(
                    vec![event],
                    self.last_output_at,
                ))
            }
            SessionIoRequest::GetModeFlags {
                request_id,
                session_id,
            } => {
                let event =
                    SessionIoEvent::ModeFlagsReady(self.runtime.mode_flags(request_id, session_id));
                Ok(SessionWorkerOutcome::from_events(
                    vec![event],
                    self.last_output_at,
                ))
            }
            SessionIoRequest::GetScreen {
                request_id,
                session_id,
            } => {
                let event =
                    SessionIoEvent::ScreenReady(self.runtime.screen(request_id, session_id));
                Ok(SessionWorkerOutcome::from_events(
                    vec![event],
                    self.last_output_at,
                ))
            }
            SessionIoRequest::SetColorProfile {
                session_id,
                color_profile,
            } => {
                self.runtime.set_color_profile(&session_id, color_profile);
                Ok(SessionWorkerOutcome::from_events(
                    Vec::new(),
                    self.last_output_at,
                ))
            }
            SessionIoRequest::Shutdown { session_id, reason } => {
                let runtime_events = self.runtime.shutdown(&session_id, &reason)?;
                self.closed = true;
                let mut events = self.flush_initial_output_events(&session_id);
                for runtime_event in runtime_events {
                    if let SessionWorkerRuntimeEvent::ProcessExited {
                        session_id,
                        payload,
                    } = runtime_event
                    {
                        events.push(SessionIoEvent::ProcessExited {
                            session_id,
                            payload,
                        });
                    }
                }
                events.push(SessionIoEvent::Shutdown { session_id, reason });
                Ok(SessionWorkerOutcome::from_events(
                    events,
                    self.last_output_at,
                ))
            }
        }?;

        Ok(outcome)
    }

    /// Handle one runtime-originated event.
    pub fn handle_runtime_event(
        &mut self,
        event: SessionWorkerRuntimeEvent,
    ) -> SessionWorkerOutcome {
        if self.closed {
            return SessionWorkerOutcome::from_events(Vec::new(), self.last_output_at);
        }

        match event {
            SessionWorkerRuntimeEvent::TerminalBytes {
                session_id,
                data,
                last_output_at,
            } => {
                self.last_output_at = Some(last_output_at);
                if self.initial_snapshot_barrier.is_some() {
                    self.pending_initial_output.push(data);
                    SessionWorkerOutcome::from_events(Vec::new(), self.last_output_at)
                } else {
                    SessionWorkerOutcome::from_events(
                        vec![SessionIoEvent::TerminalBytes { session_id, data }],
                        self.last_output_at,
                    )
                }
            }
            SessionWorkerRuntimeEvent::InitialSnapshotReady(snapshot) => {
                let mut events = self
                    .initial_snapshot_barrier
                    .get_or_insert_with(InitialSnapshotBarrier::new)
                    .deliver_initial_snapshot(snapshot);
                let session_id = match events.first() {
                    Some(SessionIoEvent::InitialSnapshotReady(snapshot)) => {
                        snapshot.session_id.clone()
                    }
                    _ => unreachable!("initial snapshot barrier always emits a snapshot"),
                };
                events.extend(self.flush_initial_output_events(&session_id));
                self.initial_snapshot_barrier = None;
                SessionWorkerOutcome::from_events(events, self.last_output_at)
            }
            SessionWorkerRuntimeEvent::ProcessExited {
                session_id,
                payload,
            } => {
                let mut events = self.flush_initial_output_events(&session_id);
                events.push(SessionIoEvent::ProcessExited {
                    session_id,
                    payload,
                });
                SessionWorkerOutcome::from_events(events, self.last_output_at)
            }
        }
    }

    fn flush_initial_output_events(&mut self, session_id: &SessionId) -> Vec<SessionIoEvent> {
        self.pending_initial_output
            .drain(..)
            .map(|data| SessionIoEvent::TerminalBytes {
                session_id: session_id.clone(),
                data,
            })
            .collect()
    }
}
