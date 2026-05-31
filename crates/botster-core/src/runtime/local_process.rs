//! Local process-backed session runtime pair.

use std::collections::HashMap;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};

use crate::engine::session_worker::{SessionWorkerRuntime, SessionWorkerRuntimeEvent};
use crate::{
    InitialSnapshotRequest, ModeFlags, ModeFlagsReady, PreparedSnapshotReady,
    PreparedSnapshotRequest, ProcessExitedPayload, ProcessIdentity, RequestId, ScreenReady,
    SendFileFailed, SendFileRequest, SendFileWritten, SessionId, SessionRuntime,
    SessionRuntimeError, SessionRuntimeErrorKind, SessionRuntimeHandle, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest, SnapshotReady, TerminalColorProfile,
};

const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[cfg(unix)]
const SIGTERM: i32 = 15;
#[cfg(unix)]
const SIGKILL: i32 = 9;

#[cfg(unix)]
extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
}

/// Options for local process shutdown behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalProcessRuntimeOptions {
    /// Time to wait after graceful termination before forced cleanup.
    ///
    /// Synchronous shutdown can block the caller for up to roughly
    /// `2 * shutdown_grace`: once after graceful termination and once after
    /// forced cleanup.
    pub shutdown_grace: Duration,
    /// Sleep interval used while polling for process exit.
    pub poll_interval: Duration,
}

impl Default for LocalProcessRuntimeOptions {
    fn default() -> Self {
        Self {
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            poll_interval: POLL_INTERVAL,
        }
    }
}

/// Spawn-side local process session runtime.
#[derive(Debug, Clone)]
pub struct LocalProcessSessionRuntime {
    registry: Arc<LocalProcessRegistryInner>,
    options: LocalProcessRuntimeOptions,
}

impl Default for LocalProcessSessionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalProcessSessionRuntime {
    /// Build a local process runtime with default shutdown behavior.
    #[must_use]
    pub fn new() -> Self {
        Self::with_options(LocalProcessRuntimeOptions::default())
    }

    /// Build a local process runtime with explicit shutdown behavior.
    #[must_use]
    pub fn with_options(options: LocalProcessRuntimeOptions) -> Self {
        Self {
            registry: Arc::new(LocalProcessRegistryInner::default()),
            options,
        }
    }

    /// Build a paired worker runtime backed by the same process registry.
    #[must_use]
    pub fn worker_runtime(&self) -> LocalProcessSessionWorkerRuntime {
        LocalProcessSessionWorkerRuntime {
            registry: Arc::clone(&self.registry),
            options: self.options,
        }
    }
}

impl SessionRuntime for LocalProcessSessionRuntime {
    fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
        let mut command = Command::new(&request.executable);
        command
            .args(&request.arguments)
            .current_dir(&request.working_directory.path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        for variable in &request.environment.variables {
            command.env(&variable.name, &variable.value);
        }

        #[cfg(unix)]
        {
            command.process_group(0);
        }

        let child = command.spawn().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("failed to spawn local process session: {error}"),
            )
        })?;
        let pid = child.id();
        let handle = SessionRuntimeHandle {
            request_id: request.request_id,
            session_id: request.session_id.clone(),
            process: ProcessIdentity {
                pid: Some(pid),
                runtime_id: Some(format!("local-process-{pid}")),
            },
        };

        self.registry.insert(request.session_id, child)?;
        Ok(handle)
    }

    fn send_input(&mut self, input: SessionRuntimeInput) -> Result<(), SessionRuntimeError> {
        match input {
            SessionRuntimeInput::Shutdown { session_id } => {
                self.registry.shutdown_and_queue_output(
                    &session_id,
                    self.options,
                    ShutdownQueueMode::QueueRuntimeOutput,
                )?;
                Ok(())
            }
            SessionRuntimeInput::PtyInput { session_id, .. }
            | SessionRuntimeInput::Resize { session_id, .. } => {
                self.registry.ensure_known(&session_id)
            }
        }
    }

    fn drain_output(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        self.registry.harvest_exited(session_id)?;
        self.registry.drain_runtime_output(session_id)
    }
}

/// Worker-side local process session runtime used by the engine path.
#[derive(Debug, Clone)]
pub struct LocalProcessSessionWorkerRuntime {
    registry: Arc<LocalProcessRegistryInner>,
    options: LocalProcessRuntimeOptions,
}

impl SessionWorkerRuntime for LocalProcessSessionWorkerRuntime {
    fn write_input(&mut self, _session_id: &SessionId, _data: &[u8]) {}

    fn resize(&mut self, _session_id: &SessionId, _rows: u16, _cols: u16) {}

    fn snapshot(&mut self, request_id: RequestId, session_id: SessionId) -> SnapshotReady {
        SnapshotReady {
            request_id,
            session_id,
            data: Vec::new(),
            rows: 0,
            cols: 0,
        }
    }

    fn request_initial_snapshot(&mut self, _request: InitialSnapshotRequest) {}

    fn send_file(&mut self, request: SendFileRequest) -> Result<SendFileWritten, SendFileFailed> {
        Ok(SendFileWritten {
            request_id: request.request_id,
            session_id: request.session_id,
            bytes: request.data.len(),
            storage_ref: None,
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
    ) -> Result<Vec<SessionWorkerRuntimeEvent>, SessionRuntimeError> {
        let payload = self.registry.shutdown_and_queue_output(
            session_id,
            self.options,
            ShutdownQueueMode::DoNotQueueRuntimeOutput,
        )?;

        Ok(payload
            .into_iter()
            .map(|payload| SessionWorkerRuntimeEvent::ProcessExited {
                session_id: session_id.clone(),
                payload,
            })
            .collect())
    }
}

#[derive(Debug, Default)]
struct LocalProcessRegistryInner {
    sessions: Mutex<HashMap<SessionId, LocalProcessSession>>,
}

impl Drop for LocalProcessRegistryInner {
    fn drop(&mut self) {
        let sessions = match self.sessions.get_mut() {
            Ok(sessions) => sessions,
            Err(poisoned) => poisoned.into_inner(),
        };

        for session in sessions.values_mut() {
            let _ = terminate_session(session, LocalProcessRuntimeOptions::default());
        }
    }
}

impl LocalProcessRegistryInner {
    fn insert(&self, session_id: SessionId, child: Child) -> Result<(), SessionRuntimeError> {
        let mut sessions = self.lock()?;
        if sessions.contains_key(&session_id) {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "local process session already exists",
            ));
        }
        sessions.insert(session_id, LocalProcessSession::new(child));
        Ok(())
    }

    fn ensure_known(&self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        let mut sessions = self.lock()?;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| session_not_found(session_id))?;
        if session.exited.is_none() {
            harvest_session(session)?;
        }
        Ok(())
    }

    fn harvest_exited(&self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        let mut sessions = self.lock()?;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| session_not_found(session_id))?;
        harvest_session(session)?;
        queue_runtime_output(session, session_id, None);
        Ok(())
    }

    fn shutdown_and_queue_output(
        &self,
        session_id: &SessionId,
        options: LocalProcessRuntimeOptions,
        queue_mode: ShutdownQueueMode,
    ) -> Result<Option<ProcessExitedPayload>, SessionRuntimeError> {
        let mut sessions = self.lock()?;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| session_not_found(session_id))?;

        let observed = terminate_session(session, options)?;
        if queue_mode == ShutdownQueueMode::QueueRuntimeOutput {
            queue_runtime_output(session, session_id, observed.clone());
        }
        Ok(observed)
    }

    fn drain_runtime_output(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        let mut sessions = self.lock()?;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| session_not_found(session_id))?;
        Ok(session.outputs.drain(..).collect())
    }

    fn lock(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<SessionId, LocalProcessSession>>, SessionRuntimeError> {
        self.sessions.lock().map_err(|_| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::CleanupFailed,
                "local process registry lock poisoned",
            )
        })
    }
}

#[derive(Debug)]
struct LocalProcessSession {
    child: Option<Child>,
    pid: u32,
    exited: Option<ProcessExitedPayload>,
    outputs: Vec<SessionRuntimeOutput>,
    output_queued: bool,
}

impl LocalProcessSession {
    fn new(child: Child) -> Self {
        let pid = child.id();
        Self {
            child: Some(child),
            pid,
            exited: None,
            outputs: Vec::new(),
            output_queued: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownQueueMode {
    QueueRuntimeOutput,
    DoNotQueueRuntimeOutput,
}

fn terminate_session(
    session: &mut LocalProcessSession,
    options: LocalProcessRuntimeOptions,
) -> Result<Option<ProcessExitedPayload>, SessionRuntimeError> {
    if session.exited.is_some() {
        send_forced_signal(session)?;
        return Ok(None);
    }

    harvest_session(session)?;
    if session.exited.is_some() {
        send_forced_signal(session)?;
        return Ok(session.exited.clone());
    }

    send_graceful_signal(session)?;
    if wait_for_exit(session, options.shutdown_grace, options.poll_interval)? {
        send_forced_signal(session)?;
        return Ok(session.exited.clone());
    }

    send_forced_signal(session)?;
    if wait_for_exit(session, options.shutdown_grace, options.poll_interval)? {
        return Ok(session.exited.clone());
    }

    Err(SessionRuntimeError::new(
        SessionRuntimeErrorKind::CleanupFailed,
        "local process did not exit after forced cleanup",
    ))
}

fn harvest_session(session: &mut LocalProcessSession) -> Result<(), SessionRuntimeError> {
    let Some(child) = session.child.as_mut() else {
        return Ok(());
    };

    let Some(status) = child.try_wait().map_err(|error| {
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::OutputFailed,
            format!("failed to inspect local process status: {error}"),
        )
    })?
    else {
        return Ok(());
    };

    session.exited = Some(payload_from_status(status));
    session.child = None;
    Ok(())
}

fn wait_for_exit(
    session: &mut LocalProcessSession,
    grace: Duration,
    poll_interval: Duration,
) -> Result<bool, SessionRuntimeError> {
    let deadline = Instant::now() + grace;
    loop {
        harvest_session(session)?;
        if session.exited.is_some() {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(poll_interval.min(deadline.saturating_duration_since(Instant::now())));
    }
}

fn queue_runtime_output(
    session: &mut LocalProcessSession,
    session_id: &SessionId,
    payload: Option<ProcessExitedPayload>,
) {
    if session.output_queued {
        return;
    }

    let Some(payload) = payload.or_else(|| session.exited.clone()) else {
        return;
    };

    session.outputs.push(SessionRuntimeOutput::ProcessExited {
        session_id: session_id.clone(),
        payload,
    });
    session.output_queued = true;
}

fn payload_from_status(status: ExitStatus) -> ProcessExitedPayload {
    ProcessExitedPayload {
        exit_code: status.code(),
        #[cfg(unix)]
        signal: status.signal(),
        #[cfg(not(unix))]
        signal: None,
    }
}

fn session_not_found(session_id: &SessionId) -> SessionRuntimeError {
    SessionRuntimeError::new(
        SessionRuntimeErrorKind::SessionNotFound,
        format!("local process session not found: {}", session_id.0),
    )
}

#[cfg(unix)]
fn send_graceful_signal(session: &LocalProcessSession) -> Result<(), SessionRuntimeError> {
    signal_process_group(
        session.pid,
        SIGTERM,
        SessionRuntimeErrorKind::ShutdownFailed,
    )
}

#[cfg(not(unix))]
fn send_graceful_signal(session: &mut LocalProcessSession) -> Result<(), SessionRuntimeError> {
    if let Some(child) = session.child.as_mut() {
        child.kill().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::ShutdownFailed,
                format!("failed to terminate local process: {error}"),
            )
        })?;
    }
    Ok(())
}

#[cfg(unix)]
fn send_forced_signal(session: &LocalProcessSession) -> Result<(), SessionRuntimeError> {
    signal_process_group(session.pid, SIGKILL, SessionRuntimeErrorKind::CleanupFailed)
}

#[cfg(not(unix))]
fn send_forced_signal(session: &mut LocalProcessSession) -> Result<(), SessionRuntimeError> {
    if let Some(child) = session.child.as_mut() {
        child.kill().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::CleanupFailed,
                format!("failed to kill local process: {error}"),
            )
        })?;
    }
    Ok(())
}

#[cfg(unix)]
fn signal_process_group(
    pid: u32,
    signal: i32,
    kind: SessionRuntimeErrorKind,
) -> Result<(), SessionRuntimeError> {
    let group = -(pid as i32);
    // The session leader may already be reaped when this runs. We still signal
    // the original process group to clean up TERM-ignoring children; ESRCH below
    // is treated as success when the group is already gone. There is a small
    // PID/PGID reuse window after reap, accepted here to preserve the no-orphan
    // guarantee for local process groups.
    // SAFETY: `kill` is called with a negative process-group id created for
    // the spawned child and a fixed signal number.
    let result = unsafe { kill(group, signal) };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(3) {
        return Ok(());
    }

    Err(SessionRuntimeError::new(
        kind,
        format!("failed to signal local process group: {error}"),
    ))
}
