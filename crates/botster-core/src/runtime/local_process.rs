//! Default local PTY-backed process runtime.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::{
    ProcessExitedPayload, ProcessIdentity, ResizePayload, SessionId, SessionRuntime,
    SessionRuntimeError, SessionRuntimeErrorKind, SessionRuntimeHandle, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest,
};

/// Policy-free local process runtime backed by a PTY.
///
/// This runtime executes the exact executable, arguments, working directory,
/// environment, and PTY size provided by `SessionSpawnRequest`.
#[derive(Default)]
pub struct LocalProcessRuntime {
    sessions: HashMap<SessionId, LocalSession>,
}

struct LocalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    output: Receiver<ReaderEvent>,
    exit_reported: bool,
}

enum ReaderEvent {
    Output(Vec<u8>),
    Failed(String),
}

impl LocalProcessRuntime {
    /// Build an empty local process runtime.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn session_mut(
        &mut self,
        session_id: &SessionId,
    ) -> Result<&mut LocalSession, SessionRuntimeError> {
        self.sessions.get_mut(session_id).ok_or_else(|| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SessionNotFound,
                format!("session not found: {}", session_id.0),
            )
        })
    }
}

impl SessionRuntime for LocalProcessRuntime {
    fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
        let pty_size = pty_size(request.initial_pty_size.as_ref());
        let pty_pair = native_pty_system().openpty(pty_size).map_err(|error| {
            spawn_error(&request.executable, format!("open pty failed: {error}"))
        })?;

        let mut command = CommandBuilder::new(&request.executable);
        command.args(&request.arguments);
        command.cwd(PathBuf::from(&request.working_directory.path));
        for variable in &request.environment.variables {
            command.env(&variable.name, &variable.value);
        }

        let child = pty_pair
            .slave
            .spawn_command(command)
            .map_err(|error| spawn_error(&request.executable, error.to_string()))?;
        let process = ProcessIdentity {
            pid: child.process_id(),
            runtime_id: Some(request.session_id.0.clone()),
        };
        let reader = pty_pair.master.try_clone_reader().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                format!("clone pty reader failed: {error}"),
            )
        })?;
        let writer = pty_pair.master.take_writer().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::InputFailed,
                format!("open pty writer failed: {error}"),
            )
        })?;
        let output = spawn_reader(reader);

        self.sessions.insert(
            request.session_id.clone(),
            LocalSession {
                master: pty_pair.master,
                writer,
                child,
                output,
                exit_reported: false,
            },
        );

        Ok(SessionRuntimeHandle {
            request_id: request.request_id,
            session_id: request.session_id,
            process,
        })
    }

    fn send_input(&mut self, input: SessionRuntimeInput) -> Result<(), SessionRuntimeError> {
        match input {
            SessionRuntimeInput::PtyInput { session_id, data } => {
                let session = self.session_mut(&session_id)?;
                session.writer.write_all(&data).map_err(|error| {
                    SessionRuntimeError::new(
                        SessionRuntimeErrorKind::InputFailed,
                        format!("write pty input failed: {error}"),
                    )
                })?;
                session.writer.flush().map_err(|error| {
                    SessionRuntimeError::new(
                        SessionRuntimeErrorKind::InputFailed,
                        format!("flush pty input failed: {error}"),
                    )
                })
            }
            SessionRuntimeInput::Resize { session_id, size } => {
                let session = self.session_mut(&session_id)?;
                session
                    .master
                    .resize(pty_size(Some(&size)))
                    .map_err(|error| {
                        SessionRuntimeError::new(
                            SessionRuntimeErrorKind::InputFailed,
                            format!("resize pty failed: {error}"),
                        )
                    })
            }
            SessionRuntimeInput::Shutdown { session_id } => {
                let session = self.session_mut(&session_id)?;
                session.child.kill().map_err(|error| {
                    SessionRuntimeError::new(
                        SessionRuntimeErrorKind::InputFailed,
                        format!("kill child process failed: {error}"),
                    )
                })
            }
        }
    }

    fn drain_output(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        let mut output = Vec::new();
        let mut session_exited = false;

        {
            let session = self.session_mut(session_id)?;

            loop {
                match session.output.try_recv() {
                    Ok(ReaderEvent::Output(data)) => output.push(SessionRuntimeOutput::PtyOutput {
                        session_id: session_id.clone(),
                        data,
                    }),
                    Ok(ReaderEvent::Failed(message)) => {
                        return Err(SessionRuntimeError::new(
                            SessionRuntimeErrorKind::OutputFailed,
                            message,
                        ));
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }

            if !session.exit_reported {
                if let Some(status) = session.child.try_wait().map_err(|error| {
                    SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        format!("read child exit status failed: {error}"),
                    )
                })? {
                    session.exit_reported = true;
                    session_exited = true;
                    output.push(SessionRuntimeOutput::ProcessExited {
                        session_id: session_id.clone(),
                        payload: ProcessExitedPayload {
                            exit_code: i32::try_from(status.exit_code()).ok(),
                            signal: None,
                        },
                    });
                }
            }
        }

        if session_exited {
            self.sessions.remove(session_id);
        }

        Ok(output)
    }
}

fn pty_size(size: Option<&ResizePayload>) -> PtySize {
    match size {
        Some(size) => PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        },
        None => PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        },
    }
}

fn spawn_reader(mut reader: Box<dyn Read + Send>) -> Receiver<ReaderEvent> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut buffer = [0; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    if sender
                        .send(ReaderEvent::Output(buffer[..bytes_read].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) if is_terminal_closed(&error) => break,
                Err(error) => {
                    let _ = sender.send(ReaderEvent::Failed(format!(
                        "read pty output failed: {error}"
                    )));
                    break;
                }
            }
        }
    });
    receiver
}

fn is_terminal_closed(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset | io::ErrorKind::UnexpectedEof
    )
}

fn spawn_error(executable: &str, detail: String) -> SessionRuntimeError {
    SessionRuntimeError::new(
        SessionRuntimeErrorKind::SpawnFailed,
        format!("spawn failed for {executable}: {detail}"),
    )
}
