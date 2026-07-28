//! Local session runtime backed by a separate worker process.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(unix)]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use sha2::{Digest, Sha256};

use crate::{
    read_welcome, write_hello, BackpressureRoute, BackpressureSummary, Frame, NotificationPayload,
    ProcessExitedPayload, ProcessIdentity, PromptMarkPayload, QueueSource, SessionId,
    SessionMetadata, SessionRuntime, SessionRuntimeError, SessionRuntimeErrorKind,
    SessionRuntimeHandle, SessionRuntimeInput, SessionRuntimeOutput, SessionSpawnRequest,
    TerminalMetadataShapingObservation, TimeoutPayload, FRAME_BELL, FRAME_CWD_CHANGED,
    FRAME_METADATA_SHAPING, FRAME_NOTIFICATION, FRAME_PING, FRAME_PONG, FRAME_PROCESS_EXITED,
    FRAME_PROMPT_MARK, FRAME_PTY_INPUT, FRAME_PTY_OUTPUT, FRAME_RESIZE, FRAME_SET_TIMEOUT,
    FRAME_SHUTDOWN, FRAME_SPAWN_SESSION, FRAME_TITLE_CHANGED,
};

/// Default retained worker egress frames per session in the parent process.
pub const DEFAULT_WORKER_EGRESS_CAPACITY: usize = 64;

const PING_WAIT: Duration = Duration::from_secs(2);
const PING_POLL: Duration = Duration::from_millis(10);
#[cfg(unix)]
const WORKER_STARTUP_TIMEOUT: Duration = Duration::from_secs(2);

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
const UNIX_SOCKET_PATH_MAX_BYTES: usize = 103;
#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
const UNIX_SOCKET_PATH_MAX_BYTES: usize = 107;

/// Options for the local worker process runtime adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerProcessRuntimeOptions {
    /// Path to the worker executable.
    pub worker_path: PathBuf,
    /// Retained worker egress frames per session before parent-side drops.
    pub egress_capacity: usize,
    /// Retained PTY reader chunks configured inside the worker process.
    pub pty_reader_chunk_capacity: usize,
    /// Worker-side shutdown grace in milliseconds.
    pub shutdown_grace_ms: u64,
    /// Worker-side shutdown poll interval in milliseconds.
    pub poll_interval_ms: u64,
    /// Directory for reconnectable worker control sockets.
    pub control_socket_dir: Option<PathBuf>,
}

impl WorkerProcessRuntimeOptions {
    /// Build options for a worker executable path.
    #[must_use]
    pub fn new(worker_path: impl Into<PathBuf>) -> Self {
        Self {
            worker_path: worker_path.into(),
            egress_capacity: DEFAULT_WORKER_EGRESS_CAPACITY,
            pty_reader_chunk_capacity: crate::DEFAULT_PTY_READER_CHUNK_CAPACITY,
            shutdown_grace_ms: 500,
            poll_interval_ms: 10,
            control_socket_dir: None,
        }
    }
}

/// Typed health evidence returned by a local session worker process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerHealth {
    /// Session whose worker responded.
    pub session_id: SessionId,
    /// Worker process identifier.
    pub worker_pid: u32,
    /// Last reconnect timeout seconds applied by the worker.
    pub reconnect_timeout_seconds: Option<u64>,
}

/// Parent-side runtime adapter for one-worker-process-per-session local PTYs.
pub struct WorkerProcessRuntime {
    options: WorkerProcessRuntimeOptions,
    sessions: HashMap<SessionId, WorkerProcessSession>,
    release_on_drop: bool,
}

impl WorkerProcessRuntime {
    /// Build an empty runtime that will launch the supplied worker executable.
    #[must_use]
    pub fn new(worker_path: impl Into<PathBuf>) -> Self {
        Self::with_options(WorkerProcessRuntimeOptions::new(worker_path))
    }

    /// Build an empty runtime with explicit worker process options.
    #[must_use]
    pub fn with_options(options: WorkerProcessRuntimeOptions) -> Self {
        Self {
            options,
            sessions: HashMap::new(),
            release_on_drop: false,
        }
    }

    /// Return worker welcome metadata captured after spawning a session.
    #[must_use]
    pub fn metadata(&self, session_id: &SessionId) -> Option<&SessionMetadata> {
        self.sessions
            .get(session_id)
            .map(|session| &session.metadata)
    }

    /// Return true when the session is owned by a live child worker process.
    #[must_use]
    pub fn is_worker_process(&mut self, session_id: &SessionId) -> bool {
        self.sessions
            .get_mut(session_id)
            .and_then(|session| session.child.as_mut())
            .and_then(|child| child.try_wait().ok().flatten())
            .is_none()
            && self.sessions.contains_key(session_id)
    }

    /// Mark a parent-side consumer attached to worker egress.
    pub fn attach_consumer(&mut self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        self.session_mut(session_id)?;
        Ok(())
    }

    /// Mark a parent-side consumer detached from worker egress.
    ///
    /// No attach/detach wire frames are sent; this is parent-side delivery
    /// registration around the worker egress stream.
    pub fn detach_consumer(&mut self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        self.session_mut(session_id)?;
        Ok(())
    }

    /// Send a ping frame and wait for typed worker health evidence.
    pub fn ping(&mut self, session_id: &SessionId) -> Result<WorkerHealth, SessionRuntimeError> {
        let session = self.session_mut(session_id)?;
        let before = session.pong_count.load(Ordering::Acquire);
        session.control.write_frame(FRAME_PING, &[])?;
        let deadline = Instant::now() + PING_WAIT;
        while Instant::now() < deadline {
            if session.pong_count.load(Ordering::Acquire) > before {
                return session
                    .last_health
                    .lock()
                    .map_err(lock_error)?
                    .clone()
                    .ok_or_else(|| {
                        SessionRuntimeError::new(
                            SessionRuntimeErrorKind::OutputFailed,
                            "worker pong did not carry health payload",
                        )
                    });
            }
            thread::sleep(PING_POLL);
        }
        Err(SessionRuntimeError::new(
            SessionRuntimeErrorKind::OutputFailed,
            "worker ping timed out",
        ))
    }

    /// Send the merged reconnect-timeout primitive to the worker process.
    pub fn set_reconnect_timeout(
        &mut self,
        session_id: &SessionId,
        seconds: u64,
    ) -> Result<(), SessionRuntimeError> {
        let session = self.session_mut(session_id)?;
        session
            .control
            .write_json(FRAME_SET_TIMEOUT, &TimeoutPayload { seconds })
    }

    /// Release worker processes without sending shutdown frames when the daemon is intentionally restarting.
    pub fn release_for_restart(&mut self) {
        self.release_on_drop = true;
    }

    /// Adopt an already-running worker process through its reconnectable control socket.
    #[cfg(unix)]
    pub fn adopt_session(
        &mut self,
        session_id: SessionId,
        process: ProcessIdentity,
        socket_path: impl Into<PathBuf>,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
        if self.sessions.contains_key(&session_id) {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "worker process session already exists",
            ));
        }

        let socket_path = socket_path.into();
        let mut control = UnixStream::connect(&socket_path).map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("connect worker control socket failed: {error}"),
            )
        })?;
        let identity = socket_identity(&socket_path).ok();
        write_hello(&mut control)
            .map_err(|error| runtime_error(SessionRuntimeErrorKind::SpawnFailed, error))?;
        let (sender, receiver) = mpsc::sync_channel(self.options.egress_capacity.max(1));
        let overflow = Arc::new(AtomicUsize::new(0));
        let pong_count = Arc::new(AtomicUsize::new(0));
        let last_health = Arc::new(Mutex::new(None));
        let completion = Arc::new(Mutex::new(WorkerCompletion::default()));
        spawn_stdout_reader(
            control.try_clone().map_err(|error| {
                SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    format!("clone worker control socket failed: {error}"),
                )
            })?,
            sender,
            Arc::clone(&overflow),
            Arc::clone(&pong_count),
            Arc::clone(&last_health),
            Arc::clone(&completion),
        );
        let metadata = SessionMetadata {
            session_uuid: session_id.0.clone(),
            pid: process.pid.unwrap_or_default(),
            rows: 24,
            cols: 80,
            last_output_at: 0,
            title: None,
            cwd: None,
            port: None,
            mode_flags: Default::default(),
            recovery_identity: Some(serde_json::json!({
                "session_uuid": session_id.0,
                "runtime_id": process.runtime_id,
                "worker_control_socket": socket_path,
            })),
        };

        self.sessions.insert(
            session_id.clone(),
            WorkerProcessSession {
                child: None,
                control: WorkerControl::Socket {
                    stream: control,
                    path: socket_path,
                    identity,
                },
                metadata,
                output: receiver,
                overflow,
                pong_count,
                last_health,
                completion,
                egress_capacity: self.options.egress_capacity.max(1),
            },
        );

        Ok(SessionRuntimeHandle {
            request_id: crate::RequestId(format!("{}-adopt", session_id.0)),
            session_id,
            process,
        })
    }

    fn session_mut(
        &mut self,
        session_id: &SessionId,
    ) -> Result<&mut WorkerProcessSession, SessionRuntimeError> {
        self.sessions.get_mut(session_id).ok_or_else(|| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SessionNotFound,
                format!("worker process session not found: {}", session_id.0),
            )
        })
    }
}

impl SessionRuntime for WorkerProcessRuntime {
    fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
        if self.sessions.contains_key(&request.session_id) {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "worker process session already exists",
            ));
        }

        let mut command = Command::new(&self.options.worker_path);
        command
            .arg("--egress-capacity")
            .arg(self.options.egress_capacity.to_string())
            .arg("--pty-reader-capacity")
            .arg(self.options.pty_reader_chunk_capacity.to_string())
            .arg("--shutdown-grace-ms")
            .arg(self.options.shutdown_grace_ms.to_string())
            .arg("--poll-interval-ms")
            .arg(self.options.poll_interval_ms.to_string())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        let socket_path = self
            .options
            .control_socket_dir
            .as_ref()
            .map(|dir| worker_socket_path(dir, &request.session_id))
            .transpose()?;
        #[cfg(not(unix))]
        let socket_path: Option<PathBuf> = None;
        #[cfg(unix)]
        let previous_socket_identity = socket_path
            .as_deref()
            .and_then(|path| socket_identity(path).ok());

        if let Some(path) = &socket_path {
            command
                .arg("--control-socket")
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null());
        } else {
            command.stdin(Stdio::piped()).stdout(Stdio::piped());
        }

        let child = command.spawn().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("spawn worker process failed: {error}"),
            )
        })?;
        let mut pending_worker = PendingWorker::new(child, socket_path.clone());

        let (mut control, mut reader): (WorkerControl, Box<dyn Read + Send>) =
            if let Some(path) = socket_path {
                #[cfg(unix)]
                {
                    let stream = connect_spawned_worker_socket(
                        &path,
                        previous_socket_identity,
                        &mut pending_worker,
                    )?;
                    stream
                        .set_read_timeout(Some(WORKER_STARTUP_TIMEOUT))
                        .map_err(|error| {
                            SessionRuntimeError::new(
                                SessionRuntimeErrorKind::SpawnFailed,
                                format!("configure worker startup timeout failed: {error}"),
                            )
                        })?;
                    let identity = socket_identity(&path).ok();
                    let reader = stream.try_clone().map_err(|error| {
                        SessionRuntimeError::new(
                            SessionRuntimeErrorKind::SpawnFailed,
                            format!("clone worker control socket failed: {error}"),
                        )
                    })?;
                    (
                        WorkerControl::Socket {
                            stream,
                            path,
                            identity,
                        },
                        Box::new(reader) as Box<dyn Read + Send>,
                    )
                }
                #[cfg(not(unix))]
                unreachable!("socket_path is never set on non-unix targets");
            } else {
                let stdin = pending_worker.child_mut().stdin.take().ok_or_else(|| {
                    SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        "worker stdin missing",
                    )
                })?;
                let stdout = pending_worker.child_mut().stdout.take().ok_or_else(|| {
                    SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        "worker stdout missing",
                    )
                })?;
                (
                    WorkerControl::Stdio(stdin),
                    Box::new(stdout) as Box<dyn Read + Send>,
                )
            };

        let startup = (|| {
            control.write_hello()?;
            control.write_json(FRAME_SPAWN_SESSION, &request)?;
            read_welcome(&mut reader)
                .map(|(_, metadata)| metadata)
                .map_err(|error| runtime_error(SessionRuntimeErrorKind::SpawnFailed, error))
        })()
        .map_err(|error: SessionRuntimeError| {
            SessionRuntimeError::new(SessionRuntimeErrorKind::SpawnFailed, error.message)
        });
        let metadata = match startup {
            Ok(metadata) => {
                control.clear_startup_read_timeout()?;
                metadata
            }
            Err(error) => {
                if let Some(diagnostic) = pending_worker.exited_diagnostic() {
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        format!("connect worker control socket failed: {diagnostic}"),
                    ));
                }
                let _ = control.write_frame(FRAME_SHUTDOWN, &[]);
                pending_worker.allow_graceful_exit();
                return Err(error);
            }
        };
        let process = ProcessIdentity {
            pid: Some(metadata.pid),
            runtime_id: metadata
                .recovery_identity
                .as_ref()
                .and_then(|identity| identity.get("runtime_id"))
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| Some(request.session_id.0.clone())),
        };

        let (sender, receiver) = mpsc::sync_channel(self.options.egress_capacity.max(1));
        let overflow = Arc::new(AtomicUsize::new(0));
        let pong_count = Arc::new(AtomicUsize::new(0));
        let last_health = Arc::new(Mutex::new(None));
        let completion = Arc::new(Mutex::new(WorkerCompletion::default()));
        spawn_stdout_reader(
            reader,
            sender,
            Arc::clone(&overflow),
            Arc::clone(&pong_count),
            Arc::clone(&last_health),
            Arc::clone(&completion),
        );

        self.sessions.insert(
            request.session_id.clone(),
            WorkerProcessSession {
                child: Some(pending_worker.take()),
                control,
                metadata,
                output: receiver,
                overflow,
                pong_count,
                last_health,
                completion,
                egress_capacity: self.options.egress_capacity.max(1),
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
                session.control.write_frame(FRAME_PTY_INPUT, &data)
            }
            SessionRuntimeInput::Resize { session_id, size } => {
                let session = self.session_mut(&session_id)?;
                session.control.write_json(FRAME_RESIZE, &size)
            }
            SessionRuntimeInput::Shutdown { session_id } => {
                let session = self.session_mut(&session_id)?;
                session.control.write_frame(FRAME_SHUTDOWN, &[])
            }
        }
    }

    fn drain_output(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        let mut output = Vec::new();
        let completed = {
            let session = self.session_mut(session_id)?;
            let overflow = session.overflow.swap(0, Ordering::AcqRel);
            if overflow > 0 {
                output.push(SessionRuntimeOutput::Backpressure(BackpressureSummary {
                    source: QueueSource::SessionIo,
                    capacity: session.egress_capacity,
                    depth: session.egress_capacity,
                    route: BackpressureRoute {
                        session_id: Some(session_id.clone()),
                        client_id: None,
                        subscription_id: None,
                        plugin_key: None,
                    },
                }));
            }

            while let Ok(event) = session.output.try_recv() {
                output.push(event.into_runtime_output(session_id));
            }

            let completion = session.completion.lock().map_err(lock_error)?;
            let terminal_payload = completion
                .reader_finished
                .then(|| completion.process_exited.clone())
                .flatten();
            drop(completion);
            terminal_payload.filter(|_| match session.child.as_mut() {
                Some(child) => child
                    .try_wait()
                    .ok()
                    .flatten()
                    .is_some_and(|status| status.success()),
                None => true,
            })
        };

        if let Some(payload) = completed {
            if let Some(mut removed) = self.sessions.remove(session_id) {
                if let Some(mut child) = removed.child.take() {
                    let _ = child.wait();
                }
                removed.control.cleanup();
            }
            output.push(SessionRuntimeOutput::ProcessExited {
                session_id: session_id.clone(),
                payload,
            });
        }

        Ok(output)
    }
}

impl Drop for WorkerProcessRuntime {
    fn drop(&mut self) {
        if self.release_on_drop {
            return;
        }
        for (_, mut session) in self.sessions.drain() {
            let _ = session.control.write_frame(FRAME_SHUTDOWN, &[]);
            if let Some(mut child) = session.child.take() {
                let _ = child.wait();
            }
            session.control.cleanup();
        }
    }
}

struct WorkerProcessSession {
    child: Option<Child>,
    control: WorkerControl,
    metadata: SessionMetadata,
    output: Receiver<WorkerOutputEvent>,
    overflow: Arc<AtomicUsize>,
    pong_count: Arc<AtomicUsize>,
    last_health: Arc<Mutex<Option<WorkerHealth>>>,
    completion: Arc<Mutex<WorkerCompletion>>,
    egress_capacity: usize,
}

#[derive(Default)]
struct WorkerCompletion {
    process_exited: Option<ProcessExitedPayload>,
    reader_finished: bool,
}

enum WorkerControl {
    Stdio(ChildStdin),
    #[cfg(unix)]
    Socket {
        stream: UnixStream,
        path: PathBuf,
        identity: Option<SocketIdentity>,
    },
}

impl WorkerControl {
    fn clear_startup_read_timeout(&self) -> Result<(), SessionRuntimeError> {
        #[cfg(unix)]
        if let Self::Socket { stream, .. } = self {
            stream.set_read_timeout(None).map_err(|error| {
                SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    format!("clear worker startup timeout failed: {error}"),
                )
            })?;
        }
        Ok(())
    }

    fn write_hello(&mut self) -> Result<(), SessionRuntimeError> {
        match self {
            Self::Stdio(stdin) => write_hello(stdin)
                .map_err(|error| runtime_error(SessionRuntimeErrorKind::SpawnFailed, error)),
            #[cfg(unix)]
            Self::Socket { stream, .. } => write_hello(stream)
                .map_err(|error| runtime_error(SessionRuntimeErrorKind::SpawnFailed, error)),
        }
    }

    fn write_frame(&mut self, frame_type: u8, payload: &[u8]) -> Result<(), SessionRuntimeError> {
        match self {
            Self::Stdio(stdin) => write_frame(stdin, frame_type, payload),
            #[cfg(unix)]
            Self::Socket { stream, .. } => write_frame(stream, frame_type, payload),
        }
    }

    fn write_json<T: Serialize>(
        &mut self,
        frame_type: u8,
        payload: &T,
    ) -> Result<(), SessionRuntimeError> {
        match self {
            Self::Stdio(stdin) => write_json(stdin, frame_type, payload),
            #[cfg(unix)]
            Self::Socket { stream, .. } => write_json(stream, frame_type, payload),
        }
    }

    fn cleanup(&self) {
        #[cfg(unix)]
        if let Self::Socket {
            path,
            identity: Some(identity),
            ..
        } = self
        {
            let _ = remove_socket_if_unchanged(path, identity);
        }
    }
}

#[cfg(unix)]
fn worker_socket_path(
    dir: &std::path::Path,
    session_id: &SessionId,
) -> Result<PathBuf, SessionRuntimeError> {
    let digest = Sha256::digest(session_id.0.as_bytes());
    let basename = format!("{}.sock", URL_SAFE_NO_PAD.encode(&digest[..16]));
    let path = dir.join(basename);
    validate_worker_socket_path(&path)?;
    Ok(path)
}

#[cfg(unix)]
fn validate_worker_socket_path(path: &std::path::Path) -> Result<(), SessionRuntimeError> {
    let path_bytes = path.as_os_str().as_bytes().len();
    if path_bytes > UNIX_SOCKET_PATH_MAX_BYTES {
        return Err(SessionRuntimeError::new(
            SessionRuntimeErrorKind::SpawnFailed,
            format!(
                "worker control socket path is {path_bytes} bytes; maximum is \
                 {UNIX_SOCKET_PATH_MAX_BYTES} bytes"
            ),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn connect_spawned_worker_socket(
    path: &std::path::Path,
    previous_identity: Option<SocketIdentity>,
    pending_worker: &mut PendingWorker,
) -> Result<UnixStream, SessionRuntimeError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_error = None;
    loop {
        if let Some(diagnostic) = pending_worker.exited_diagnostic() {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("connect worker control socket failed: {diagnostic}"),
            ));
        }
        match socket_identity(path) {
            Ok(identity) if Some(identity) != previous_identity => {
                match UnixStream::connect(path) {
                    Ok(stream) => return Ok(stream),
                    Err(error) => last_error = Some(error),
                }
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidInput
                ) => {}
            Err(error) => last_error = Some(error),
        }
        if Instant::now() >= deadline {
            let detail = last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "worker endpoint did not become ready".to_string());
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("connect worker control socket failed: {detail}"),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SocketIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn socket_identity(path: &std::path::Path) -> std::io::Result<SocketIdentity> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "worker control endpoint is not a socket",
        ));
    }
    Ok(SocketIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(unix)]
fn remove_socket_if_unchanged(
    path: &std::path::Path,
    expected: &SocketIdentity,
) -> std::io::Result<bool> {
    let current = match socket_identity(path) {
        Ok(identity) => identity,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if &current != expected {
        return Ok(false);
    }
    std::fs::remove_file(path)?;
    Ok(true)
}

struct PendingWorker {
    child: Option<Child>,
    graceful_shutdown: bool,
    #[cfg(unix)]
    socket_path: Option<PathBuf>,
}

impl PendingWorker {
    fn new(child: Child, socket_path: Option<PathBuf>) -> Self {
        #[cfg(not(unix))]
        let _ = socket_path;
        Self {
            child: Some(child),
            graceful_shutdown: false,
            #[cfg(unix)]
            socket_path,
        }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("pending worker child")
    }

    fn take(mut self) -> Child {
        #[cfg(unix)]
        self.socket_path.take();
        self.child.take().expect("pending worker child")
    }

    fn allow_graceful_exit(&mut self) {
        self.graceful_shutdown = true;
    }

    fn exited_diagnostic(&mut self) -> Option<String> {
        let child = self.child.as_mut()?;
        let status = child.try_wait().ok().flatten()?;
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        let stderr = stderr.trim();
        Some(if stderr.is_empty() {
            format!("worker process exited before startup completed ({status})")
        } else {
            stderr.to_string()
        })
    }
}

impl Drop for PendingWorker {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if self.graceful_shutdown {
                let deadline = Instant::now() + Duration::from_millis(500);
                while Instant::now() < deadline {
                    if child.try_wait().ok().flatten().is_some() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
        #[cfg(unix)]
        if let Some(path) = &self.socket_path {
            if let Ok(identity) = socket_identity(path) {
                if UnixStream::connect(path)
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionRefused)
                {
                    let _ = remove_socket_if_unchanged(path, &identity);
                }
            }
        }
    }
}

enum WorkerOutputEvent {
    PtyOutput(Vec<u8>),
    TitleChanged(String),
    CwdChanged(String),
    PromptMark(PromptMarkPayload),
    Bell,
    Notification(NotificationPayload),
    MetadataShaping(TerminalMetadataShapingObservation),
}

impl WorkerOutputEvent {
    fn into_runtime_output(self, session_id: &SessionId) -> SessionRuntimeOutput {
        match self {
            Self::PtyOutput(data) => SessionRuntimeOutput::PtyOutput {
                session_id: session_id.clone(),
                data,
            },
            Self::TitleChanged(title) => SessionRuntimeOutput::TitleChanged {
                session_id: session_id.clone(),
                title,
            },
            Self::CwdChanged(cwd) => SessionRuntimeOutput::CwdChanged {
                session_id: session_id.clone(),
                cwd,
            },
            Self::PromptMark(payload) => SessionRuntimeOutput::PromptMark {
                session_id: session_id.clone(),
                payload,
            },
            Self::Bell => SessionRuntimeOutput::Bell {
                session_id: session_id.clone(),
            },
            Self::Notification(payload) => SessionRuntimeOutput::Notification {
                session_id: session_id.clone(),
                payload,
            },
            Self::MetadataShaping(observation) => {
                SessionRuntimeOutput::MetadataShaping(observation)
            }
        }
    }
}

fn spawn_stdout_reader(
    mut stdout: impl Read + Send + 'static,
    sender: SyncSender<WorkerOutputEvent>,
    overflow: Arc<AtomicUsize>,
    pong_count: Arc<AtomicUsize>,
    last_health: Arc<Mutex<Option<WorkerHealth>>>,
    completion: Arc<Mutex<WorkerCompletion>>,
) {
    thread::spawn(move || {
        while let Ok(frame) = read_frame(&mut stdout) {
            match frame.frame_type {
                FRAME_PTY_OUTPUT => send_worker_output(
                    &sender,
                    &overflow,
                    WorkerOutputEvent::PtyOutput(frame.payload),
                ),
                FRAME_PROCESS_EXITED => {
                    if let Ok(payload) = serde_json::from_slice(&frame.payload) {
                        if let Ok(mut state) = completion.lock() {
                            state.process_exited = Some(payload);
                        }
                    }
                }
                FRAME_TITLE_CHANGED => {
                    if let Ok(title) = String::from_utf8(frame.payload) {
                        send_worker_output(
                            &sender,
                            &overflow,
                            WorkerOutputEvent::TitleChanged(title),
                        );
                    }
                }
                FRAME_CWD_CHANGED => {
                    if let Ok(cwd) = String::from_utf8(frame.payload) {
                        send_worker_output(&sender, &overflow, WorkerOutputEvent::CwdChanged(cwd));
                    }
                }
                FRAME_PROMPT_MARK => {
                    if let Ok(payload) = serde_json::from_slice(&frame.payload) {
                        send_worker_output(
                            &sender,
                            &overflow,
                            WorkerOutputEvent::PromptMark(payload),
                        );
                    }
                }
                FRAME_BELL => {
                    send_worker_output(&sender, &overflow, WorkerOutputEvent::Bell);
                }
                FRAME_NOTIFICATION => {
                    if let Ok(payload) = serde_json::from_slice(&frame.payload) {
                        send_worker_output(
                            &sender,
                            &overflow,
                            WorkerOutputEvent::Notification(payload),
                        );
                    }
                }
                FRAME_METADATA_SHAPING => {
                    if let Ok(observation) = serde_json::from_slice(&frame.payload) {
                        send_worker_output(
                            &sender,
                            &overflow,
                            WorkerOutputEvent::MetadataShaping(observation),
                        );
                    }
                }
                FRAME_PONG => {
                    if let Ok(health) = serde_json::from_slice(&frame.payload) {
                        if let Ok(mut slot) = last_health.lock() {
                            *slot = Some(health);
                        }
                    }
                    pong_count.fetch_add(1, Ordering::AcqRel);
                }
                _ => {}
            }
        }
        if let Ok(mut state) = completion.lock() {
            state.reader_finished = true;
        }
    });
}

fn send_worker_output(
    sender: &SyncSender<WorkerOutputEvent>,
    overflow: &AtomicUsize,
    event: WorkerOutputEvent,
) {
    match sender.try_send(event) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            overflow.fetch_add(1, Ordering::AcqRel);
        }
        Err(TrySendError::Disconnected(_)) => {}
    }
}

fn read_frame(stream: &mut impl Read) -> Result<Frame, SessionRuntimeError> {
    let mut len_buf = [0; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|error| runtime_error(SessionRuntimeErrorKind::OutputFailed, error))?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > crate::MAX_FRAME_LEN {
        return Err(SessionRuntimeError::new(
            SessionRuntimeErrorKind::OutputFailed,
            "worker emitted invalid frame length",
        ));
    }
    let mut body = vec![0; len];
    stream
        .read_exact(&mut body)
        .map_err(|error| runtime_error(SessionRuntimeErrorKind::OutputFailed, error))?;
    Ok(Frame {
        frame_type: body[0],
        payload: body[1..].to_vec(),
    })
}

fn write_frame(
    stream: &mut impl Write,
    frame_type: u8,
    payload: &[u8],
) -> Result<(), SessionRuntimeError> {
    let frame = crate::encode_frame(frame_type, payload)
        .map_err(|error| runtime_error(SessionRuntimeErrorKind::InputFailed, error))?;
    stream
        .write_all(&frame)
        .and_then(|_| stream.flush())
        .map_err(|error| runtime_error(SessionRuntimeErrorKind::InputFailed, error))
}

fn write_json<T: Serialize>(
    stream: &mut impl Write,
    frame_type: u8,
    payload: &T,
) -> Result<(), SessionRuntimeError> {
    let frame = crate::encode_json(frame_type, payload)
        .map_err(|error| runtime_error(SessionRuntimeErrorKind::InputFailed, error))?;
    stream
        .write_all(&frame)
        .and_then(|_| stream.flush())
        .map_err(|error| runtime_error(SessionRuntimeErrorKind::InputFailed, error))
}

fn runtime_error(
    kind: SessionRuntimeErrorKind,
    error: impl std::fmt::Display,
) -> SessionRuntimeError {
    SessionRuntimeError::new(kind, error.to_string())
}

fn lock_error<T>(_error: std::sync::PoisonError<T>) -> SessionRuntimeError {
    SessionRuntimeError::new(
        SessionRuntimeErrorKind::OutputFailed,
        "worker health lock poisoned",
    )
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::Path;

    use super::{
        worker_socket_path, ProcessIdentity, SessionId, SessionRuntimeErrorKind,
        WorkerProcessRuntime, UNIX_SOCKET_PATH_MAX_BYTES,
    };

    #[test]
    fn worker_socket_names_are_bounded_distinct_full_id_digests() {
        let root = Path::new("/tmp/bcd-endpoint");
        let ids = [
            SessionId("123e4567-e89b-12d3-a456-426614174000".to_string()),
            SessionId(format!("sess-long-{}", "identifier-".repeat(100))),
            SessionId("old/sanitizer/collision".to_string()),
            SessionId("old?sanitizer?collision".to_string()),
        ];
        let paths: Vec<_> = ids
            .iter()
            .map(|id| worker_socket_path(root, id).expect("bounded socket path"))
            .collect();
        let basename_lengths: Vec<_> = paths
            .iter()
            .map(|path| path.file_name().expect("basename").len())
            .collect();

        assert!(basename_lengths
            .windows(2)
            .all(|lengths| lengths[0] == lengths[1]));
        assert!(paths
            .iter()
            .enumerate()
            .all(|(index, path)| !paths[index + 1..].contains(path)));
        assert!(paths
            .iter()
            .all(|path| path.file_name().expect("basename").len() == 27));
    }

    #[test]
    fn worker_socket_path_enforces_platform_byte_capacity() {
        let session_id = SessionId("123e4567-e89b-12d3-a456-426614174000".to_string());
        let basename_len = worker_socket_path(Path::new("/"), &session_id)
            .expect("short path")
            .file_name()
            .expect("basename")
            .len();
        let root_len = UNIX_SOCKET_PATH_MAX_BYTES - basename_len - 1;
        let fitting_root = format!("/{}", "r".repeat(root_len - 1));
        let overlong_root = format!("{fitting_root}x");

        let fitting = worker_socket_path(Path::new(&fitting_root), &session_id)
            .expect("platform maximum should fit");
        assert_eq!(
            fitting.as_os_str().as_encoded_bytes().len(),
            UNIX_SOCKET_PATH_MAX_BYTES
        );
        let error = worker_socket_path(Path::new(&overlong_root), &session_id)
            .expect_err("path beyond platform maximum must fail");
        assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
        assert!(error.message.starts_with("worker control socket path is "));
        assert!(!error
            .message
            .starts_with("connect worker control socket failed: "));
    }

    #[test]
    fn adoption_preserves_the_stable_connect_failure_contract() {
        let mut runtime = WorkerProcessRuntime::new("/missing/worker");
        let error = runtime
            .adopt_session(
                SessionId("missing-adoption".to_string()),
                ProcessIdentity {
                    pid: Some(std::process::id()),
                    runtime_id: Some("live-process-identity".to_string()),
                },
                "/tmp/botster-missing-adoption-worker.sock",
            )
            .expect_err("missing adopted endpoint must fail");

        assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
        assert!(error
            .message
            .starts_with("connect worker control socket failed: "));
    }
}
