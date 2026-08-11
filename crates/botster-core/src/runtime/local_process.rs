//! Default local PTY-backed process runtime with process-group cleanup.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::engine::session_worker::{SessionWorkerRuntime, SessionWorkerRuntimeEvent};
use crate::{
    BackpressureRoute, BackpressureSummary, InitialSnapshotRequest, ModeFlagsReady,
    PreparedSnapshotReady, PreparedSnapshotRequest, ProcessExitedPayload, ProcessIdentity,
    QueueSource, RequestId, ResizePayload, ScreenReady, SendFileFailed, SendFileRequest,
    SendFileWritten, SessionId, SessionRuntime, SessionRuntimeError, SessionRuntimeErrorKind,
    SessionRuntimeHandle, SessionRuntimeInput, SessionRuntimeOutput, SessionSpawnRequest,
    SnapshotReady, TerminalColorProfile,
};

const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const PTY_READER_BUFFER_BYTES: usize = 8192;
/// Default retained PTY reader chunks per session.
///
/// Chunks are at most 8192 bytes, so this bounds retained reader memory to
/// roughly 512 KiB per live local PTY session before OS-level PTY backpressure
/// slows the child process.
pub const DEFAULT_PTY_READER_CHUNK_CAPACITY: usize = 64;

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
    /// Retained PTY reader chunks per session before the reader blocks.
    ///
    /// Values below one are clamped to one chunk when the reader starts.
    pub pty_reader_chunk_capacity: usize,
    /// Test-only: hold after a successful PTY read while still inside the reader
    /// critical section, before channel publication.
    pub test_hold_after_read_ms: Option<u64>,
    /// Test-only: force write attempts to return `WouldBlock` until this Unix ms.
    pub test_write_block_until_unix_ms: Option<u64>,
}

impl Default for LocalProcessRuntimeOptions {
    fn default() -> Self {
        Self {
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            poll_interval: POLL_INTERVAL,
            pty_reader_chunk_capacity: DEFAULT_PTY_READER_CHUNK_CAPACITY,
            test_hold_after_read_ms: None,
            test_write_block_until_unix_ms: None,
        }
    }
}

/// Policy-free local process runtime backed by a PTY.
///
/// This runtime executes the exact executable, arguments, working directory,
/// environment, and PTY size provided by `SessionSpawnRequest`. On Unix, it
/// also uses the PTY process group leader to terminate the whole process group
/// during shutdown so child processes are not orphaned.
#[derive(Clone)]
pub struct LocalProcessRuntime {
    registry: Arc<LocalProcessRegistry>,
    options: LocalProcessRuntimeOptions,
    write_test_hooks: Arc<WriteTestHooks>,
}

impl Default for LocalProcessRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalProcessRuntime {
    /// Build an empty local process runtime with default shutdown behavior.
    #[must_use]
    pub fn new() -> Self {
        Self::with_options(LocalProcessRuntimeOptions::default())
    }

    /// Build an empty local process runtime with explicit shutdown behavior.
    #[must_use]
    pub fn with_options(options: LocalProcessRuntimeOptions) -> Self {
        Self {
            registry: Arc::new(LocalProcessRegistry::default()),
            write_test_hooks: Arc::new(WriteTestHooks::from_options(&options)),
            options,
        }
    }

    /// Build a paired worker runtime backed by the same process registry.
    #[must_use]
    pub fn worker_runtime(&self) -> LocalProcessWorkerRuntime {
        LocalProcessWorkerRuntime {
            registry: Arc::clone(&self.registry),
            options: self.options,
        }
    }

    /// Run `body` with the PTY reader paused and exclusive session I/O ownership.
    ///
    /// The reader thread stops issuing new PTY reads before `body` runs, so
    /// drained output, Ghostty apply, token comparison, and PTY write can form
    /// one atomic admission barrier.
    pub fn with_pty_io_barrier<R, F>(
        &mut self,
        session_id: &SessionId,
        body: F,
    ) -> Result<R, SessionRuntimeError>
    where
        F: FnOnce(&mut PtyIoBarrier<'_>) -> Result<R, SessionRuntimeError>,
    {
        self.registry.with_pty_io_barrier(session_id, body)
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
        let pid = child.process_id();
        let process_group = process_group_leader(pty_pair.master.as_ref(), pid);
        let process = ProcessIdentity {
            pid,
            runtime_id: Some(request.session_id.0.clone()),
        };
        // Non-blocking PTY reads let the admission fence pause the reader without
        // waiting for the child to produce data or exit.
        set_master_nonblocking(pty_pair.master.as_ref())?;
        let reader = pty_pair.master.try_clone_reader().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                format!("clone pty reader failed: {error}"),
            )
        })?;
        // Barrier residual reader: while the background reader is paused, the
        // admission path drains the OS PTY buffer through this handle.
        let residual_reader = pty_pair.master.try_clone_reader().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                format!("clone residual pty reader failed: {error}"),
            )
        })?;
        let writer = pty_pair.master.take_writer().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::InputFailed,
                format!("open pty writer failed: {error}"),
            )
        })?;
        let reader_fence = Arc::new(ReaderFence {
            state: Mutex::new(ReaderFenceState::default()),
            cv: Condvar::new(),
            test_hold_after_read_ms: self.options.test_hold_after_read_ms,
        });
        let (output, output_pressure, output_capacity) = spawn_reader(
            reader,
            self.options.pty_reader_chunk_capacity,
            Arc::clone(&reader_fence),
        );

        self.registry.insert(
            request.session_id.clone(),
            LocalSession {
                master: pty_pair.master,
                writer,
                residual_reader,
                child,
                output,
                output_pressure,
                output_capacity,
                process_group,
                exit_payload: None,
                outputs: Vec::new(),
                exit_output_queued: false,
                process_group_cleanup_requested: false,
                reader_disconnected: false,
                pending_reader_error: None,
                reader_fence,
                write_test_hooks: Arc::clone(&self.write_test_hooks),
            },
        )?;

        Ok(SessionRuntimeHandle {
            request_id: request.request_id,
            session_id: request.session_id,
            process,
        })
    }

    fn send_input(&mut self, input: SessionRuntimeInput) -> Result<(), SessionRuntimeError> {
        match input {
            SessionRuntimeInput::PtyInput { session_id, data } => {
                self.registry.write_input(&session_id, &data)
            }
            SessionRuntimeInput::Resize { session_id, size } => {
                self.registry.resize(&session_id, size)
            }
            SessionRuntimeInput::Shutdown { session_id } => self
                .registry
                .shutdown_session(&session_id, self.options)
                .map(|_| ()),
        }
    }

    fn drain_output(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        self.registry.drain_output(session_id)
    }
}

/// Exclusive PTY I/O handle available inside [`LocalProcessRuntime::with_pty_io_barrier`].
pub struct PtyIoBarrier<'a> {
    session: MutexGuard<'a, LocalSession>,
    session_id: SessionId,
}

impl PtyIoBarrier<'_> {
    /// Drain currently queued reader output while the reader remains paused.
    ///
    /// Also drains residual bytes still sitting in the OS PTY buffer through a
    /// dedicated residual reader so a paused background reader cannot hide
    /// pre-write mode changes.
    pub fn drain_output(&mut self) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        if let Some(message) = self.session.pending_reader_error.take() {
            return Err(reader_error(message));
        }
        let mut output = drain_reader_output(&mut self.session, &self.session_id)?;
        output.extend(drain_residual_reader(
            &mut self.session.residual_reader,
            &self.session_id,
        )?);
        harvest_session(&mut self.session)?;
        if self.session.exit_payload.is_some() {
            request_process_group_cleanup(&mut self.session)?;
        }
        if reader_finalization_complete(
            self.session.reader_disconnected,
            self.session.pending_reader_error.as_deref(),
        ) {
            queue_exit_output(&mut self.session, &self.session_id, None);
        }
        let mut queued = self.session.outputs.drain(..).collect::<Vec<_>>();
        output.append(&mut queued);
        Ok(output)
    }

    /// Write raw PTY input while the reader remains paused.
    ///
    /// When `deadline_unix_ms` is set, the write fails closed at or after that
    /// wall-clock instant and does not keep retrying past the deadline.
    pub fn write_input(
        &mut self,
        data: &[u8],
        deadline_unix_ms: Option<u64>,
    ) -> Result<(), SessionRuntimeError> {
        let hooks = Arc::clone(&self.session.write_test_hooks);
        write_all_blocking(
            &mut self.session.writer,
            data,
            deadline_unix_ms,
            Some(hooks.as_ref()),
        )
    }
}

/// Worker-side local process runtime used by the engine path.
#[derive(Clone)]
pub struct LocalProcessWorkerRuntime {
    registry: Arc<LocalProcessRegistry>,
    options: LocalProcessRuntimeOptions,
}

impl SessionWorkerRuntime for LocalProcessWorkerRuntime {
    fn write_input(&mut self, session_id: &SessionId, data: &[u8]) {
        let _ = self.registry.write_input(session_id, data);
    }

    fn resize(
        &mut self,
        session_id: &SessionId,
        rows: u16,
        cols: u16,
    ) -> Result<(), SessionRuntimeError> {
        self.registry
            .resize(session_id, ResizePayload { rows, cols })
    }

    fn snapshot(&mut self, request_id: RequestId, session_id: SessionId) -> SnapshotReady {
        let (rows, cols) = self.registry.terminal_size(&session_id).unwrap_or((24, 80));
        SnapshotReady {
            request_id,
            session_id,
            data: Vec::new(),
            rows,
            cols,
        }
    }

    fn request_initial_snapshot(
        &mut self,
        _request: InitialSnapshotRequest,
    ) -> Result<(), SessionRuntimeError> {
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

    fn prepare_snapshot(&mut self, request: PreparedSnapshotRequest) -> PreparedSnapshotReady {
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
        _request_id: RequestId,
        _session_id: SessionId,
    ) -> Result<ModeFlagsReady, SessionRuntimeError> {
        // SessionRuntimeErrorKind has no Unsupported variant. OutputFailed is
        // the narrow existing read-failure category; callers must use the
        // managed terminal backend seam to distinguish unsupported capability.
        Err(SessionRuntimeError::new(
            SessionRuntimeErrorKind::OutputFailed,
            "local process runtime has no authoritative terminal mode backend",
        ))
    }

    fn screen(&mut self, request_id: RequestId, session_id: SessionId) -> ScreenReady {
        ScreenReady {
            request_id,
            session_id,
            text: String::new(),
        }
    }

    fn set_color_profile(
        &mut self,
        _session_id: &SessionId,
        _color_profile: TerminalColorProfile,
    ) -> Result<(), SessionRuntimeError> {
        Ok(())
    }

    fn shutdown(
        &mut self,
        session_id: &SessionId,
        _reason: &str,
    ) -> Result<Vec<SessionWorkerRuntimeEvent>, SessionRuntimeError> {
        self.registry.shutdown_session(session_id, self.options)?;
        Ok(Vec::new())
    }
}

#[derive(Default)]
struct LocalProcessRegistry {
    sessions: Mutex<HashMap<SessionId, LocalSessionHandle>>,
}

impl Drop for LocalProcessRegistry {
    fn drop(&mut self) {
        let sessions = match self.sessions.get_mut() {
            Ok(sessions) => sessions,
            Err(poisoned) => poisoned.into_inner(),
        };

        let sessions: Vec<_> = sessions.drain().map(|(_, session)| session).collect();
        for session in sessions {
            if let Ok(mut session) = session.lock() {
                let _ = terminate_session(&mut session, LocalProcessRuntimeOptions::default());
            }
        }
    }
}

impl LocalProcessRegistry {
    fn insert(
        &self,
        session_id: SessionId,
        session: LocalSession,
    ) -> Result<(), SessionRuntimeError> {
        let mut sessions = self.lock()?;
        if sessions.contains_key(&session_id) {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "local process session already exists",
            ));
        }
        sessions.insert(session_id, Arc::new(Mutex::new(session)));
        Ok(())
    }

    fn write_input(&self, session_id: &SessionId, data: &[u8]) -> Result<(), SessionRuntimeError> {
        let session = self.session(session_id)?;
        let mut session = lock_session(&session)?;
        let hooks = Arc::clone(&session.write_test_hooks);
        write_all_blocking(&mut session.writer, data, None, Some(hooks.as_ref()))
    }

    fn resize(
        &self,
        session_id: &SessionId,
        size: ResizePayload,
    ) -> Result<(), SessionRuntimeError> {
        let session = self.session(session_id)?;
        let session = lock_session(&session)?;
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

    fn terminal_size(&self, session_id: &SessionId) -> Result<(u16, u16), SessionRuntimeError> {
        let session = self.session(session_id)?;
        let session = lock_session(&session)?;
        let size = session.master.get_size().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                format!("read pty size failed: {error}"),
            )
        })?;
        Ok((size.rows, size.cols))
    }

    fn shutdown_session(
        &self,
        session_id: &SessionId,
        options: LocalProcessRuntimeOptions,
    ) -> Result<Option<ProcessExitedPayload>, SessionRuntimeError> {
        let session = self.session(session_id)?;
        let mut session = lock_session(&session)?;
        terminate_session(&mut session, options)
    }

    fn drain_output(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        let session = self.session(session_id)?;
        let mut session = lock_session(&session)?;

        if let Some(message) = session.pending_reader_error.take() {
            return Err(reader_error(message));
        }

        let mut output = drain_reader_output(&mut session, session_id)?;
        harvest_session(&mut session)?;
        if session.exit_payload.is_some() {
            request_process_group_cleanup(&mut session)?;
        }

        if reader_finalization_complete(
            session.reader_disconnected,
            session.pending_reader_error.as_deref(),
        ) {
            queue_exit_output(&mut session, session_id, None);
        }
        let mut queued_output = session.outputs.drain(..).collect();
        output.append(&mut queued_output);

        if session.exit_payload.is_some() && session.exit_output_queued {
            drop(session);
            self.remove(session_id)?;
        }

        Ok(output)
    }

    fn with_pty_io_barrier<R, F>(
        &self,
        session_id: &SessionId,
        body: F,
    ) -> Result<R, SessionRuntimeError>
    where
        F: FnOnce(&mut PtyIoBarrier<'_>) -> Result<R, SessionRuntimeError>,
    {
        let handle = self.session(session_id)?;
        // Pause the reader before taking the session lock so the reader can
        // leave its critical section without contending on the session mutex.
        let fence = {
            let session = lock_session(&handle)?;
            Arc::clone(&session.reader_fence)
        };
        fence.pause_and_wait_idle()?;
        let result = {
            let session = lock_session(&handle)?;
            let mut barrier = PtyIoBarrier {
                session,
                session_id: session_id.clone(),
            };
            body(&mut barrier)
        };
        fence.resume();
        result
    }

    fn lock(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<SessionId, LocalSessionHandle>>, SessionRuntimeError> {
        self.sessions.lock().map_err(|_| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::CleanupFailed,
                "local process registry lock poisoned",
            )
        })
    }

    fn session(&self, session_id: &SessionId) -> Result<LocalSessionHandle, SessionRuntimeError> {
        let sessions = self.lock()?;
        sessions.get(session_id).cloned().ok_or_else(|| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SessionNotFound,
                format!("session not found: {}", session_id.0),
            )
        })
    }

    fn remove(&self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        let mut sessions = self.lock()?;
        sessions.remove(session_id);
        Ok(())
    }
}

type LocalSessionHandle = Arc<Mutex<LocalSession>>;

struct LocalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    residual_reader: Box<dyn Read + Send>,
    child: Box<dyn Child + Send + Sync>,
    output: Receiver<ReaderEvent>,
    output_pressure: Arc<ReaderPressure>,
    output_capacity: usize,
    process_group: Option<i32>,
    exit_payload: Option<ProcessExitedPayload>,
    outputs: Vec<SessionRuntimeOutput>,
    exit_output_queued: bool,
    process_group_cleanup_requested: bool,
    reader_disconnected: bool,
    pending_reader_error: Option<String>,
    reader_fence: Arc<ReaderFence>,
    write_test_hooks: Arc<WriteTestHooks>,
}

struct ReaderFence {
    state: Mutex<ReaderFenceState>,
    cv: Condvar,
    test_hold_after_read_ms: Option<u64>,
}

#[derive(Default)]
struct WriteTestHooks {
    force_would_block_until_unix_ms: Option<u64>,
}

impl WriteTestHooks {
    fn from_options(options: &LocalProcessRuntimeOptions) -> Self {
        Self {
            force_would_block_until_unix_ms: options.test_write_block_until_unix_ms,
        }
    }
}

#[derive(Default)]
struct ReaderFenceState {
    paused: bool,
    in_critical: bool,
}

impl ReaderFence {
    fn pause_and_wait_idle(&self) -> Result<(), SessionRuntimeError> {
        let mut state = self.state.lock().map_err(|_| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::CleanupFailed,
                "local process reader fence lock poisoned",
            )
        })?;
        state.paused = true;
        while state.in_critical {
            state = self.cv.wait(state).map_err(|_| {
                SessionRuntimeError::new(
                    SessionRuntimeErrorKind::CleanupFailed,
                    "local process reader fence wait poisoned",
                )
            })?;
        }
        Ok(())
    }

    fn resume(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.paused = false;
            self.cv.notify_all();
        }
    }

    fn enter_critical(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        while state.paused {
            state.in_critical = false;
            self.cv.notify_all();
            let Ok(guard) = self.cv.wait(state) else {
                return;
            };
            state = guard;
        }
        state.in_critical = true;
    }

    fn leave_critical(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.in_critical = false;
            if state.paused {
                self.cv.notify_all();
            }
        }
    }
}

enum ReaderEvent {
    Output(Vec<u8>),
    Failed(String),
}

fn lock_session(
    session: &LocalSessionHandle,
) -> Result<MutexGuard<'_, LocalSession>, SessionRuntimeError> {
    session.lock().map_err(|_| {
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::CleanupFailed,
            "local process session lock poisoned",
        )
    })
}

#[derive(Default)]
struct ReaderPressure {
    depth: AtomicUsize,
    pressured: AtomicBool,
}

fn terminate_session(
    session: &mut LocalSession,
    options: LocalProcessRuntimeOptions,
) -> Result<Option<ProcessExitedPayload>, SessionRuntimeError> {
    if session.exit_payload.is_some() {
        request_process_group_cleanup(session)?;
        return Ok(None);
    }

    harvest_session(session)?;
    if session.exit_payload.is_some() {
        request_process_group_cleanup(session)?;
        return Ok(session.exit_payload.clone());
    }

    send_graceful_signal(session)?;
    if wait_for_exit(session, options.shutdown_grace, options.poll_interval)? {
        request_process_group_cleanup(session)?;
        return Ok(session.exit_payload.clone());
    }

    request_process_group_cleanup(session)?;
    if wait_for_exit(session, options.shutdown_grace, options.poll_interval)? {
        return Ok(session.exit_payload.clone());
    }

    Err(SessionRuntimeError::new(
        SessionRuntimeErrorKind::CleanupFailed,
        "local process did not exit after forced cleanup",
    ))
}

fn request_process_group_cleanup(session: &mut LocalSession) -> Result<(), SessionRuntimeError> {
    if session.process_group_cleanup_requested {
        return Ok(());
    }

    // Record ownership before signaling so later drain ticks never signal a
    // re-used process-group id a second time.
    session.process_group_cleanup_requested = true;
    send_forced_signal(session)
}

fn harvest_session(session: &mut LocalSession) -> Result<(), SessionRuntimeError> {
    if session.exit_payload.is_some() {
        return Ok(());
    }

    let Some(status) = session.child.try_wait().map_err(|error| {
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::OutputFailed,
            format!("failed to inspect local process status: {error}"),
        )
    })?
    else {
        return Ok(());
    };

    session.exit_payload = Some(ProcessExitedPayload {
        exit_code: i32::try_from(status.exit_code()).ok(),
        signal: signal_number(status.signal()),
    });
    Ok(())
}

fn wait_for_exit(
    session: &mut LocalSession,
    grace: Duration,
    poll_interval: Duration,
) -> Result<bool, SessionRuntimeError> {
    let deadline = Instant::now() + grace;
    loop {
        harvest_session(session)?;
        if session.exit_payload.is_some() {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(poll_interval.min(deadline.saturating_duration_since(Instant::now())));
    }
}

fn queue_exit_output(
    session: &mut LocalSession,
    session_id: &SessionId,
    payload: Option<ProcessExitedPayload>,
) {
    if session.exit_output_queued {
        return;
    }

    let Some(payload) = payload.or_else(|| session.exit_payload.clone()) else {
        return;
    };

    session.exit_payload = Some(payload.clone());
    session.outputs.push(SessionRuntimeOutput::ProcessExited {
        session_id: session_id.clone(),
        payload,
    });
    session.exit_output_queued = true;
}

fn drain_reader_output(
    session: &mut LocalSession,
    session_id: &SessionId,
) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
    drain_reader_events(
        &session.output,
        &session.output_pressure,
        session.output_capacity,
        &mut session.reader_disconnected,
        &mut session.pending_reader_error,
        session_id,
    )
}

fn drain_residual_reader(
    residual_reader: &mut Box<dyn Read + Send>,
    session_id: &SessionId,
) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
    let mut output = Vec::new();
    let mut buffer = [0; PTY_READER_BUFFER_BYTES];
    loop {
        match residual_reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => output.push(SessionRuntimeOutput::PtyOutput {
                session_id: session_id.clone(),
                data: buffer[..bytes_read].to_vec(),
            }),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if is_terminal_closed(&error) => break,
            Err(error) => {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::OutputFailed,
                    format!("residual pty read failed: {error}"),
                ))
            }
        }
    }
    Ok(output)
}

fn drain_reader_events(
    receiver: &Receiver<ReaderEvent>,
    pressure: &ReaderPressure,
    capacity: usize,
    reader_disconnected: &mut bool,
    pending_reader_error: &mut Option<String>,
    session_id: &SessionId,
) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
    let mut output = Vec::new();
    if pressure.pressured.swap(false, Ordering::AcqRel) {
        output.push(SessionRuntimeOutput::Backpressure(BackpressureSummary {
            source: QueueSource::SessionIo,
            capacity,
            depth: pressure.depth.load(Ordering::Acquire).min(capacity),
            route: BackpressureRoute {
                session_id: Some(session_id.clone()),
                client_id: None,
                subscription_id: None,
                plugin_key: None,
            },
        }));
    }
    loop {
        match receiver.try_recv() {
            Ok(ReaderEvent::Output(data)) => {
                decrement_reader_depth(pressure);
                output.push(SessionRuntimeOutput::PtyOutput {
                    session_id: session_id.clone(),
                    data,
                });
            }
            Ok(ReaderEvent::Failed(message)) => {
                decrement_reader_depth(pressure);
                *pending_reader_error = Some(message);
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                *reader_disconnected = true;
                break;
            }
        }
    }

    if output.is_empty() {
        if let Some(message) = pending_reader_error.take() {
            return Err(reader_error(message));
        }
    }

    Ok(output)
}

fn reader_error(message: String) -> SessionRuntimeError {
    SessionRuntimeError::new(SessionRuntimeErrorKind::OutputFailed, message)
}

fn reader_finalization_complete(
    reader_disconnected: bool,
    pending_reader_error: Option<&str>,
) -> bool {
    reader_disconnected && pending_reader_error.is_none()
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

fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    capacity: usize,
    fence: Arc<ReaderFence>,
) -> (Receiver<ReaderEvent>, Arc<ReaderPressure>, usize) {
    let capacity = capacity.max(1);
    let pressure = Arc::new(ReaderPressure::default());
    let reader_pressure = Arc::clone(&pressure);
    let (sender, receiver) = mpsc::sync_channel(capacity);
    thread::spawn(move || {
        let mut buffer = [0; PTY_READER_BUFFER_BYTES];
        loop {
            // Wait while paused, then mark critical around the non-blocking read
            // and channel publication so unpublished chunks never leave the fence.
            fence.enter_critical();
            let read_result = reader.read(&mut buffer);
            match read_result {
                Ok(0) => {
                    fence.leave_critical();
                    break;
                }
                Ok(bytes_read) => {
                    let chunk = buffer[..bytes_read].to_vec();
                    if let Some(hold_ms) = fence.test_hold_after_read_ms {
                        if hold_ms > 0 {
                            // Stay critical: barrier must wait for publication.
                            thread::sleep(Duration::from_millis(hold_ms));
                        }
                    }
                    let publish =
                        send_reader_event(&sender, &reader_pressure, ReaderEvent::Output(chunk));
                    fence.leave_critical();
                    if publish.is_err() {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    fence.leave_critical();
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    fence.leave_critical();
                }
                Err(error) if is_terminal_closed(&error) => {
                    fence.leave_critical();
                    break;
                }
                Err(error) => {
                    let publish = send_reader_event(
                        &sender,
                        &reader_pressure,
                        ReaderEvent::Failed(format!("read pty output failed: {error}")),
                    );
                    fence.leave_critical();
                    let _ = publish;
                    break;
                }
            }
        }
    });
    (receiver, pressure, capacity)
}

fn write_all_blocking(
    writer: &mut Box<dyn Write + Send>,
    data: &[u8],
    deadline_unix_ms: Option<u64>,
    write_test_hooks: Option<&WriteTestHooks>,
) -> Result<(), SessionRuntimeError> {
    if data.is_empty() {
        return Ok(());
    }
    if deadline_reached(deadline_unix_ms) {
        return Err(SessionRuntimeError::new(
            SessionRuntimeErrorKind::InputFailed,
            "write pty input failed: deadline_exceeded before write",
        ));
    }
    let mut offset = 0;
    while offset < data.len() {
        if deadline_reached(deadline_unix_ms) {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::InputFailed,
                format!(
                    "write pty input failed: deadline_exceeded after {offset} of {} bytes",
                    data.len()
                ),
            ));
        }
        if force_write_would_block(write_test_hooks) {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        match writer.write(&data[offset..]) {
            Ok(0) => {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::InputFailed,
                    "write pty input failed: wrote zero bytes",
                ))
            }
            Ok(written) => offset += written,
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::Interrupted =>
            {
                if deadline_reached(deadline_unix_ms) {
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::InputFailed,
                        format!(
                            "write pty input failed: deadline_exceeded after {offset} of {} bytes",
                            data.len()
                        ),
                    ));
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::InputFailed,
                    format!("write pty input failed: {error}"),
                ))
            }
        }
    }
    loop {
        if deadline_reached(deadline_unix_ms) {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::InputFailed,
                "write pty input failed: deadline_exceeded during flush",
            ));
        }
        if force_write_would_block(write_test_hooks) {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        match writer.flush() {
            Ok(()) => return Ok(()),
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::Interrupted =>
            {
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::InputFailed,
                    format!("flush pty input failed: {error}"),
                ))
            }
        }
    }
}

fn deadline_reached(deadline_unix_ms: Option<u64>) -> bool {
    match deadline_unix_ms {
        Some(deadline) => unix_now_ms() >= deadline,
        None => false,
    }
}

fn force_write_would_block(write_test_hooks: Option<&WriteTestHooks>) -> bool {
    match write_test_hooks.and_then(|hooks| hooks.force_would_block_until_unix_ms) {
        Some(until) => unix_now_ms() < until,
        None => false,
    }
}

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn set_master_nonblocking(master: &dyn MasterPty) -> Result<(), SessionRuntimeError> {
    #[cfg(unix)]
    {
        let Some(fd) = master.as_raw_fd() else {
            return Ok(());
        };
        // SAFETY: fd is the live master PTY descriptor owned by this runtime.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("get pty flags failed: {}", io::Error::last_os_error()),
            ));
        }
        let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
        if result < 0 {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("set pty nonblocking failed: {}", io::Error::last_os_error()),
            ));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = master;
        Ok(())
    }
}

fn send_reader_event(
    sender: &SyncSender<ReaderEvent>,
    pressure: &ReaderPressure,
    event: ReaderEvent,
) -> Result<(), ()> {
    pressure.depth.fetch_add(1, Ordering::AcqRel);
    match sender.try_send(event) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(event)) => {
            pressure.pressured.store(true, Ordering::Release);
            sender.send(event).map_err(|_| {
                decrement_reader_depth(pressure);
            })
        }
        Err(TrySendError::Disconnected(_)) => {
            decrement_reader_depth(pressure);
            Err(())
        }
    }
}

fn decrement_reader_depth(pressure: &ReaderPressure) {
    let _ = pressure
        .depth
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |depth| {
            depth.checked_sub(1)
        });
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

#[cfg(unix)]
fn process_group_leader(master: &dyn MasterPty, pid: Option<u32>) -> Option<i32> {
    master
        .process_group_leader()
        .or_else(|| pid.map(|pid| pid as i32))
}

#[cfg(not(unix))]
fn process_group_leader(_master: &dyn MasterPty, _pid: Option<u32>) -> Option<i32> {
    None
}

#[cfg(unix)]
fn send_graceful_signal(session: &LocalSession) -> Result<(), SessionRuntimeError> {
    signal_process_group(
        session.process_group,
        SIGTERM,
        SessionRuntimeErrorKind::ShutdownFailed,
    )
}

#[cfg(not(unix))]
fn send_graceful_signal(session: &mut LocalSession) -> Result<(), SessionRuntimeError> {
    session.child.kill().map_err(|error| {
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::ShutdownFailed,
            format!("failed to terminate local process: {error}"),
        )
    })
}

#[cfg(unix)]
fn send_forced_signal(session: &LocalSession) -> Result<(), SessionRuntimeError> {
    signal_process_group(
        session.process_group,
        SIGKILL,
        SessionRuntimeErrorKind::CleanupFailed,
    )
}

#[cfg(not(unix))]
fn send_forced_signal(session: &mut LocalSession) -> Result<(), SessionRuntimeError> {
    session.child.kill().map_err(|error| {
        SessionRuntimeError::new(
            SessionRuntimeErrorKind::CleanupFailed,
            format!("failed to kill local process: {error}"),
        )
    })
}

#[cfg(unix)]
fn signal_process_group(
    process_group: Option<i32>,
    signal: i32,
    kind: SessionRuntimeErrorKind,
) -> Result<(), SessionRuntimeError> {
    let Some(process_group) = process_group else {
        return Ok(());
    };
    let group = -process_group;
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

fn signal_number(signal: Option<&str>) -> Option<i32> {
    match signal {
        Some(signal) if signal.contains("Killed") || signal.contains("9") => Some(9),
        Some(signal) if signal.contains("Terminated") || signal.contains("15") => Some(15),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session_id() -> SessionId {
        SessionId("reader-finalization-test".to_string())
    }

    #[test]
    fn reader_disconnection_is_completion_only_after_queued_output_is_drained() {
        let pressure = Arc::new(ReaderPressure::default());
        let (sender, receiver) = mpsc::sync_channel(2);
        let mut reader_disconnected = false;
        let mut pending_reader_error = None;

        assert!(drain_reader_events(
            &receiver,
            &pressure,
            2,
            &mut reader_disconnected,
            &mut pending_reader_error,
            &test_session_id(),
        )
        .expect("empty live reader drain")
        .is_empty());
        assert!(!reader_disconnected);
        assert!(!reader_finalization_complete(
            reader_disconnected,
            pending_reader_error.as_deref()
        ));

        send_reader_event(
            &sender,
            &pressure,
            ReaderEvent::Output(b"final-reader-output".to_vec()),
        )
        .expect("queue final reader output");
        let output = drain_reader_events(
            &receiver,
            &pressure,
            2,
            &mut reader_disconnected,
            &mut pending_reader_error,
            &test_session_id(),
        )
        .expect("drain queued final reader output");
        assert!(matches!(
            output.as_slice(),
            [SessionRuntimeOutput::PtyOutput { data, .. }]
                if data == b"final-reader-output"
        ));
        assert!(!reader_disconnected);
        assert!(!reader_finalization_complete(
            reader_disconnected,
            pending_reader_error.as_deref()
        ));

        drop(sender);
        assert!(drain_reader_events(
            &receiver,
            &pressure,
            2,
            &mut reader_disconnected,
            &mut pending_reader_error,
            &test_session_id(),
        )
        .expect("drain disconnected reader")
        .is_empty());
        assert!(reader_disconnected);
        assert!(reader_finalization_complete(
            reader_disconnected,
            pending_reader_error.as_deref()
        ));
    }

    #[test]
    fn reader_failure_is_deferred_until_preceding_output_is_returned() {
        let pressure = Arc::new(ReaderPressure::default());
        let (sender, receiver) = mpsc::sync_channel(2);
        let mut reader_disconnected = false;
        let mut pending_reader_error = None;

        send_reader_event(
            &sender,
            &pressure,
            ReaderEvent::Output(b"bytes-before-failure".to_vec()),
        )
        .expect("queue reader output");
        send_reader_event(
            &sender,
            &pressure,
            ReaderEvent::Failed("controlled reader failure".to_string()),
        )
        .expect("queue reader failure");
        drop(sender);

        let output = drain_reader_events(
            &receiver,
            &pressure,
            2,
            &mut reader_disconnected,
            &mut pending_reader_error,
            &test_session_id(),
        )
        .expect("preceding output is returned before failure");
        assert!(matches!(
            output.as_slice(),
            [SessionRuntimeOutput::PtyOutput { data, .. }]
                if data == b"bytes-before-failure"
        ));
        assert_eq!(
            pending_reader_error.as_deref(),
            Some("controlled reader failure")
        );
        assert!(reader_disconnected);
        assert!(!reader_finalization_complete(
            reader_disconnected,
            pending_reader_error.as_deref()
        ));

        let error = drain_reader_events(
            &receiver,
            &pressure,
            2,
            &mut reader_disconnected,
            &mut pending_reader_error,
            &test_session_id(),
        )
        .expect_err("reader failure is surfaced after preceding output");
        assert_eq!(error.kind, SessionRuntimeErrorKind::OutputFailed);
        assert_eq!(error.message, "controlled reader failure");
        assert!(reader_finalization_complete(
            reader_disconnected,
            pending_reader_error.as_deref()
        ));
    }

    #[test]
    fn worker_without_terminal_backend_errors_instead_of_defaulting_mode_flags() {
        let runtime = LocalProcessRuntime::new();
        let mut worker = runtime.worker_runtime();

        let error = worker
            .mode_flags(RequestId("mode-read".to_string()), test_session_id())
            .expect_err("local process worker has no authoritative mode backend");

        assert_eq!(error.kind, SessionRuntimeErrorKind::OutputFailed);
        assert_eq!(
            error.message,
            "local process runtime has no authoritative terminal mode backend"
        );
    }
}
