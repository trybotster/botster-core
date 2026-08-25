//! Default local PTY-backed process runtime with process-group cleanup.

use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    /// When the fence pending queue is full the reader waits outside the fence
    /// critical section (lossless ordinary pressure + OS PTY backpressure). It
    /// does **not** treat capacity alone as a sticky mode-authority failure.
    pub pty_reader_chunk_capacity: usize,
    /// Test-only: hold after a successful PTY read while still inside the reader
    /// critical section, before leave_critical (still unpublished on the fence).
    pub test_hold_after_read_ms: Option<u64>,
    /// Test-only: force write attempts to return `WouldBlock` until this Unix ms.
    pub test_write_block_until_unix_ms: Option<u64>,
    /// Test-only: cap each `write()` call to this many bytes (partial-write proofs).
    pub test_write_max_chunk: Option<usize>,
    /// Test-only: override fence pending capacity (pressure / forced-loss proofs).
    pub test_pending_capacity: Option<usize>,
    /// Test-only: hold after successful fence enqueue while still critical
    /// (single-queue hold proofs; must stay under the fence the barrier waits on).
    pub test_hold_after_enqueue_ms: Option<u64>,
    /// Test-only: fail the next PTY write or resize so apply can prove fail-closed teardown.
    pub test_fail_pty_writes: bool,
}

impl Default for LocalProcessRuntimeOptions {
    fn default() -> Self {
        Self {
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            poll_interval: POLL_INTERVAL,
            pty_reader_chunk_capacity: DEFAULT_PTY_READER_CHUNK_CAPACITY,
            test_hold_after_read_ms: None,
            test_write_block_until_unix_ms: None,
            test_write_max_chunk: None,
            test_pending_capacity: None,
            test_hold_after_enqueue_ms: None,
            test_fail_pty_writes: false,
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
        let reader_capacity = self.options.pty_reader_chunk_capacity.max(1);
        // Single fence-owned FIFO. Capacity bounds retained unpublished chunks.
        // Ordinary pressure waits outside the fence critical section (lossless).
        // Sticky fail-closed mode authority is reserved for true loss / reader
        // failure (Failed event or unit-only set_overflow_error).
        let pending_capacity = self
            .options
            .test_pending_capacity
            .unwrap_or_else(|| reader_capacity.saturating_mul(8).max(256))
            .max(1);
        let reader_fence = Arc::new(ReaderFence {
            state: Mutex::new(ReaderFenceState::default()),
            cv: Condvar::new(),
            pending_cv: Condvar::new(),
            test_hold_after_read_ms: self.options.test_hold_after_read_ms,
            test_hold_after_enqueue_ms: self.options.test_hold_after_enqueue_ms,
            pending: Mutex::new(VecDeque::new()),
            pending_capacity,
            overflow_error: Mutex::new(None),
            reader_finished: AtomicBool::new(false),
        });
        let (output_pressure, output_capacity) =
            spawn_reader(reader, reader_capacity, Arc::clone(&reader_fence));

        self.registry.insert(
            request.session_id.clone(),
            LocalSession {
                master: pty_pair.master,
                writer,
                residual_reader,
                child,
                output_pressure,
                output_capacity,
                process_group,
                exit_payload: None,
                outputs: Vec::new(),
                exit_output_queued: false,
                process_group_cleanup_requested: false,
                reader_disconnected: false,
                pending_reader_error: None,
                authority_failed: None,
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
        // Single fence-owned FIFO plus residual OS bytes (newest). Retained
        // events are delivered even when authority is sticky-failed so the
        // worker can apply them; callers must then call
        // [`Self::ensure_mode_authority`] before probe success or admit write.
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
            self.session.authority_failed.as_deref(),
        ) {
            queue_exit_output(&mut self.session, &self.session_id, None);
        }
        let mut queued = self.session.outputs.drain(..).collect::<Vec<_>>();
        output.append(&mut queued);
        if output.is_empty() {
            self.ensure_mode_authority()?;
        }
        Ok(output)
    }

    /// Fail closed when mode authority is incomplete (sticky overflow, etc.).
    ///
    /// Call after applying retained drain output and before token compare or
    /// PTY write so the first post-overflow gated operation cannot succeed.
    pub fn ensure_mode_authority(&self) -> Result<(), SessionRuntimeError> {
        if let Some(message) = sticky_session_error(&self.session) {
            return Err(reader_error(message));
        }
        Ok(())
    }

    /// Write raw PTY input while the reader remains paused.
    ///
    /// When `deadline_unix_ms` is set, the write fails closed at or after that
    /// wall-clock instant and does not keep retrying past the deadline.
    /// Returns the number of bytes written. Complete success is `Ok(data.len())`.
    /// `Err` with [`PtyWriteFailure::bytes_written`] `> 0` is an explicit partial write.
    pub fn write_input(
        &mut self,
        data: &[u8],
        deadline_unix_ms: Option<u64>,
    ) -> Result<usize, PtyWriteFailure> {
        let hooks = Arc::clone(&self.session.write_test_hooks);
        write_all_blocking(
            &mut self.session.writer,
            data,
            deadline_unix_ms,
            Some(hooks.as_ref()),
        )
    }

    /// Resize the PTY while the reader remains paused.
    pub fn resize(&mut self, size: ResizePayload) -> Result<(), SessionRuntimeError> {
        self.session
            .master
            .resize(pty_size(Some(&size)))
            .map_err(|error| {
                SessionRuntimeError::new(
                    SessionRuntimeErrorKind::InputFailed,
                    format!("resize pty failed: {error}"),
                )
            })
    }
}

/// Failure from a deadline-aware PTY write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyWriteFailure {
    /// Human-readable failure detail.
    pub message: String,
    /// Bytes successfully written before the failure.
    pub bytes_written: usize,
}

impl PtyWriteFailure {
    fn new(message: impl Into<String>, bytes_written: usize) -> Self {
        Self {
            message: message.into(),
            bytes_written,
        }
    }

    fn into_runtime_error(self) -> SessionRuntimeError {
        SessionRuntimeError::new(SessionRuntimeErrorKind::InputFailed, self.message)
    }
}

impl std::fmt::Display for PtyWriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
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
        if session.write_test_hooks.fail_writes {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::InputFailed,
                "test_fail_pty_writes",
            ));
        }
        let hooks = Arc::clone(&session.write_test_hooks);
        write_all_blocking(&mut session.writer, data, None, Some(hooks.as_ref()))
            .map(|_| ())
            .map_err(PtyWriteFailure::into_runtime_error)
    }

    fn resize(
        &self,
        session_id: &SessionId,
        size: ResizePayload,
    ) -> Result<(), SessionRuntimeError> {
        let session = self.session(session_id)?;
        let session = lock_session(&session)?;
        if session.write_test_hooks.fail_writes {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::InputFailed,
                "test_fail_pty_writes",
            ));
        }
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

        // Deliver retained output before sticky authority / reader failure.
        let mut output = drain_reader_output(&mut session, session_id)?;
        harvest_session(&mut session)?;
        if session.exit_payload.is_some() {
            request_process_group_cleanup(&mut session)?;
        }

        if reader_finalization_complete(
            session.reader_disconnected,
            session.pending_reader_error.as_deref(),
            session.authority_failed.as_deref(),
        ) {
            queue_exit_output(&mut session, session_id, None);
        }
        let mut queued_output = session.outputs.drain(..).collect();
        output.append(&mut queued_output);

        if output.is_empty() {
            if let Some(message) = sticky_session_error(&session) {
                return Err(reader_error(message));
            }
        }

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
    output_pressure: Arc<ReaderPressure>,
    output_capacity: usize,
    process_group: Option<i32>,
    exit_payload: Option<ProcessExitedPayload>,
    outputs: Vec<SessionRuntimeOutput>,
    exit_output_queued: bool,
    process_group_cleanup_requested: bool,
    reader_disconnected: bool,
    pending_reader_error: Option<String>,
    /// Sticky mode-authority failure (overflow, etc.). Never cleared for the
    /// session lifetime. Probes and drains fail closed after retained output.
    authority_failed: Option<String>,
    reader_fence: Arc<ReaderFence>,
    write_test_hooks: Arc<WriteTestHooks>,
}

struct ReaderFence {
    state: Mutex<ReaderFenceState>,
    cv: Condvar,
    /// Wakes the reader after drain frees fence pending capacity.
    pending_cv: Condvar,
    test_hold_after_read_ms: Option<u64>,
    /// Hold after successful enqueue while still in critical (tests only).
    test_hold_after_enqueue_ms: Option<u64>,
    /// Single ownership queue for reader PTY events.
    pending: Mutex<VecDeque<ReaderEvent>>,
    pending_capacity: usize,
    /// Sticky mode-authority failure for true loss (reader I/O failure, or
    /// unit-test injection via set_overflow_error). Ordinary capacity pressure
    /// never sets this. Promoted into session `authority_failed` on drain.
    overflow_error: Mutex<Option<String>>,
    /// Reader thread has exited (EOF/error stop).
    reader_finished: AtomicBool,
}

#[derive(Default)]
struct WriteTestHooks {
    force_would_block_until_unix_ms: Option<u64>,
    max_chunk: Option<usize>,
    writes_completed: AtomicUsize,
    fail_writes: bool,
}

impl WriteTestHooks {
    fn from_options(options: &LocalProcessRuntimeOptions) -> Self {
        Self {
            force_would_block_until_unix_ms: options.test_write_block_until_unix_ms,
            max_chunk: options.test_write_max_chunk,
            writes_completed: AtomicUsize::new(0),
            fail_writes: options.test_fail_pty_writes,
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

    /// Enqueue without dropping. Updates pressure depth under the same lock so
    /// a concurrent drain cannot observe an enqueued event with depth lag.
    /// Returns `Err(event)` when at capacity (caller waits outside critical).
    fn push_pending(
        &self,
        event: ReaderEvent,
        pressure: &ReaderPressure,
        pressure_capacity: usize,
    ) -> Result<(), ReaderEvent> {
        let Ok(mut pending) = self.pending.lock() else {
            return Err(event);
        };
        if pending.len() >= self.pending_capacity {
            // Still mark pressured so drains observe ordinary reader pressure
            // even while the lossless wait path holds the chunk.
            let depth = pending.len().max(1);
            pressure.depth.store(depth, Ordering::Release);
            if depth >= pressure_capacity.max(1) {
                pressure.pressured.store(true, Ordering::Release);
            }
            return Err(event);
        }
        pending.push_back(event);
        let depth = pending.len();
        pressure.depth.store(depth, Ordering::Release);
        if depth >= pressure_capacity.max(1) {
            pressure.pressured.store(true, Ordering::Release);
        }
        Ok(())
    }

    /// Wait until fence pending has free capacity. Must not be called while the
    /// reader holds the fence critical section (would block mode barriers).
    fn wait_for_pending_space(&self) {
        let Ok(mut pending) = self.pending.lock() else {
            return;
        };
        while pending.len() >= self.pending_capacity {
            let Ok(guard) = self.pending_cv.wait(pending) else {
                return;
            };
            pending = guard;
        }
    }

    /// Drain all pending events and sync pressure depth under the same lock.
    fn take_pending(&self, pressure: &ReaderPressure) -> Vec<ReaderEvent> {
        let Ok(mut pending) = self.pending.lock() else {
            return Vec::new();
        };
        let events: Vec<_> = pending.drain(..).collect();
        pressure.depth.store(pending.len(), Ordering::Release);
        // Wake any reader waiting for free capacity (lossless pressure path).
        self.pending_cv.notify_all();
        events
    }

    fn take_overflow_error(&self) -> Option<String> {
        self.overflow_error
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }

    /// Internal unit-test forced-loss seam only. Production never calls this;
    /// ordinary capacity pressure waits losslessly instead.
    #[allow(dead_code)]
    fn set_overflow_error(&self, message: impl Into<String>) {
        if let Ok(mut slot) = self.overflow_error.lock() {
            if slot.is_none() {
                *slot = Some(message.into());
            }
        }
    }

    fn mark_reader_finished(&self) {
        self.reader_finished.store(true, Ordering::Release);
    }

    fn reader_finished(&self) -> bool {
        self.reader_finished.load(Ordering::Acquire)
    }
}
#[derive(Debug)]
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
    // Single fence-owned FIFO only.
    let mut output = Vec::new();
    if session
        .output_pressure
        .pressured
        .swap(false, Ordering::AcqRel)
    {
        let depth = session
            .output_pressure
            .depth
            .load(Ordering::Acquire)
            .min(session.output_capacity);
        output.push(SessionRuntimeOutput::Backpressure(BackpressureSummary {
            source: QueueSource::SessionIo,
            capacity: session.output_capacity,
            depth,
            route: BackpressureRoute {
                session_id: Some(session_id.clone()),
                client_id: None,
                subscription_id: None,
                plugin_key: None,
            },
        }));
    }
    let (pending_out, fence_failure) =
        drain_fence_pending(&session.reader_fence, session_id, &session.output_pressure);
    output.extend(pending_out);
    if let Some(message) = fence_failure {
        session.authority_failed.get_or_insert(message);
    }
    if let Some(message) = session.reader_fence.take_overflow_error() {
        session.authority_failed.get_or_insert(message);
    }
    if let Some(message) = session.pending_reader_error.take() {
        session.authority_failed.get_or_insert(message);
    }
    if session.reader_fence.reader_finished() {
        session.reader_disconnected = true;
    }
    Ok(output)
}

fn sticky_session_error(session: &LocalSession) -> Option<String> {
    session
        .authority_failed
        .clone()
        .or_else(|| session.pending_reader_error.clone())
}

fn drain_fence_pending(
    fence: &ReaderFence,
    session_id: &SessionId,
    pressure: &ReaderPressure,
) -> (Vec<SessionRuntimeOutput>, Option<String>) {
    let mut output = Vec::new();
    let mut failure = None;
    for event in fence.take_pending(pressure) {
        match event {
            ReaderEvent::Output(data) => {
                output.push(SessionRuntimeOutput::PtyOutput {
                    session_id: session_id.clone(),
                    data,
                });
            }
            ReaderEvent::Failed(message) => {
                // Deliver preceding (and later) Output events; latch failure.
                failure = Some(message);
            }
        }
    }
    (output, failure)
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

fn reader_error(message: String) -> SessionRuntimeError {
    SessionRuntimeError::new(SessionRuntimeErrorKind::OutputFailed, message)
}

fn reader_finalization_complete(
    reader_disconnected: bool,
    pending_reader_error: Option<&str>,
    authority_failed: Option<&str>,
) -> bool {
    reader_disconnected && pending_reader_error.is_none() && authority_failed.is_none()
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
) -> (Arc<ReaderPressure>, usize) {
    let capacity = capacity.max(1);
    let pressure = Arc::new(ReaderPressure::default());
    let reader_pressure = Arc::clone(&pressure);
    let reader_fence = Arc::clone(&fence);
    thread::spawn(move || {
        let mut buffer = [0; PTY_READER_BUFFER_BYTES];
        loop {
            // Critical for capture into fence-owned pending only.
            reader_fence.enter_critical();
            let read_result = reader.read(&mut buffer);
            match read_result {
                Ok(0) => {
                    reader_fence.mark_reader_finished();
                    reader_fence.leave_critical();
                    break;
                }
                Ok(bytes_read) => {
                    let mut event = ReaderEvent::Output(buffer[..bytes_read].to_vec());
                    if let Some(hold_ms) = reader_fence.test_hold_after_read_ms {
                        if hold_ms > 0 {
                            thread::sleep(Duration::from_millis(hold_ms));
                        }
                    }
                    // Enqueue + depth under one lock (see push_pending). On
                    // capacity pressure, leave critical before waiting so a
                    // mode barrier can progress; never block under critical.
                    loop {
                        match reader_fence.push_pending(event, &reader_pressure, capacity) {
                            Ok(()) => {
                                if let Some(hold_ms) = reader_fence.test_hold_after_enqueue_ms {
                                    if hold_ms > 0 {
                                        thread::sleep(Duration::from_millis(hold_ms));
                                    }
                                }
                                reader_fence.leave_critical();
                                break;
                            }
                            Err(returned) => {
                                // Ordinary pressure is always lossless.
                                event = returned;
                                reader_fence.leave_critical();
                                reader_fence.wait_for_pending_space();
                                reader_fence.enter_critical();
                            }
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    reader_fence.leave_critical();
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                    reader_fence.leave_critical();
                }
                Err(error) if is_terminal_closed(&error) => {
                    reader_fence.mark_reader_finished();
                    reader_fence.leave_critical();
                    break;
                }
                Err(error) => {
                    // True reader failure: retain Failed when possible (wait
                    // outside critical for space), then stop. Sticky authority
                    // follows from the Failed event on drain — not capacity.
                    let mut event = ReaderEvent::Failed(format!("read pty output failed: {error}"));
                    loop {
                        match reader_fence.push_pending(event, &reader_pressure, capacity) {
                            Ok(()) => break,
                            Err(returned) => {
                                event = returned;
                                reader_fence.leave_critical();
                                reader_fence.wait_for_pending_space();
                                reader_fence.enter_critical();
                            }
                        }
                    }
                    reader_fence.mark_reader_finished();
                    reader_fence.leave_critical();
                    break;
                }
            }
        }
    });
    (pressure, capacity)
}

fn write_all_blocking(
    writer: &mut Box<dyn Write + Send>,
    data: &[u8],
    deadline_unix_ms: Option<u64>,
    write_test_hooks: Option<&WriteTestHooks>,
) -> Result<usize, PtyWriteFailure> {
    if data.is_empty() {
        return Ok(0);
    }
    if deadline_reached(deadline_unix_ms) {
        return Err(PtyWriteFailure::new(
            "write pty input failed: deadline_exceeded before write",
            0,
        ));
    }
    let mut offset = 0;
    while offset < data.len() {
        if deadline_reached(deadline_unix_ms) {
            return Err(PtyWriteFailure::new(
                format!(
                    "write pty input failed: deadline_exceeded after {offset} of {} bytes",
                    data.len()
                ),
                offset,
            ));
        }
        if force_write_would_block(write_test_hooks) {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        let end = match write_test_hooks.and_then(|hooks| hooks.max_chunk) {
            Some(max) if max > 0 => (offset + max).min(data.len()),
            _ => data.len(),
        };
        match writer.write(&data[offset..end]) {
            Ok(0) => {
                return Err(PtyWriteFailure::new(
                    "write pty input failed: wrote zero bytes",
                    offset,
                ))
            }
            Ok(written) => {
                offset += written;
                if let Some(hooks) = write_test_hooks {
                    hooks.writes_completed.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::Interrupted =>
            {
                if deadline_reached(deadline_unix_ms) {
                    return Err(PtyWriteFailure::new(
                        format!(
                            "write pty input failed: deadline_exceeded after {offset} of {} bytes",
                            data.len()
                        ),
                        offset,
                    ));
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => {
                return Err(PtyWriteFailure::new(
                    format!("write pty input failed: {error}"),
                    offset,
                ))
            }
        }
    }
    loop {
        if deadline_reached(deadline_unix_ms) {
            // All payload bytes were accepted by the kernel; flush timeout is
            // still a complete delivery of the request payload.
            return Ok(offset);
        }
        if force_write_would_block(write_test_hooks) {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        match writer.flush() {
            Ok(()) => return Ok(offset),
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::Interrupted =>
            {
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => {
                return Err(PtyWriteFailure::new(
                    format!("flush pty input failed: {error}"),
                    offset,
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
    let Some(hooks) = write_test_hooks else {
        return false;
    };
    // After the first successful write chunk, optional deadline backpressure
    // forces WouldBlock so partial-write proofs can cross the deadline.
    if hooks.max_chunk.is_some() && hooks.writes_completed.load(Ordering::Relaxed) > 0 {
        if let Some(until) = hooks.force_would_block_until_unix_ms {
            return unix_now_ms() < until;
        }
    }
    match hooks.force_would_block_until_unix_ms {
        Some(until) if hooks.max_chunk.is_none() => unix_now_ms() < until,
        _ => false,
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

    fn test_fence(pending_capacity: usize) -> Arc<ReaderFence> {
        Arc::new(ReaderFence {
            state: Mutex::new(ReaderFenceState::default()),
            cv: Condvar::new(),
            pending_cv: Condvar::new(),
            test_hold_after_read_ms: None,
            test_hold_after_enqueue_ms: None,
            pending: Mutex::new(VecDeque::new()),
            pending_capacity,
            overflow_error: Mutex::new(None),
            reader_finished: AtomicBool::new(false),
        })
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

    #[test]
    fn single_queue_fifo_order_is_preserved() {
        let fence = test_fence(4);
        let pressure = Arc::new(ReaderPressure::default());
        let first = b"\x1b[?1000h".to_vec();
        let second = b"\x1b[?1000l".to_vec();
        fence
            .push_pending(ReaderEvent::Output(first.clone()), &pressure, 4)
            .expect("first");
        fence
            .push_pending(ReaderEvent::Output(second.clone()), &pressure, 4)
            .expect("second");
        assert_eq!(pressure.depth.load(Ordering::Acquire), 2);
        let (out, fail) = drain_fence_pending(&fence, &test_session_id(), &pressure);
        assert!(fail.is_none());
        assert_eq!(pressure.depth.load(Ordering::Acquire), 0);
        let chunks: Vec<Vec<u8>> = out
            .into_iter()
            .filter_map(|o| match o {
                SessionRuntimeOutput::PtyOutput { data, .. } => Some(data),
                _ => None,
            })
            .collect();
        assert_eq!(chunks, vec![first, second]);
    }

    #[test]
    fn single_queue_reader_source_prohibits_dual_buffer_transfer() {
        // Cold-migration regression for the dual-buffer transfer design.
        // Historical defective SHA df38c218092f59377bec12457840b0a7512bd294 still
        // contains these symbols; the normal-drain integration body alone can
        // pass there. This source guard fails on that mutant and passes on the
        // single-queue production reader.
        // Inspect production source only (exclude this test module so the ban
        // list literals cannot self-match).
        let full = include_str!("local_process.rs");
        let production = full
            .split("#[cfg(test)]")
            .next()
            .expect("production local_process.rs");
        // Construct banned fragments so this test body does not embed the
        // complete production symbols as contiguous ban targets either.
        let banned = [
            format!("{}{}", "try_flush_pending", "_to_channel"),
            format!("{}{}", "pending-to-", "channel"),
            format!("{}{}", "dual ", "buffers"),
            format!("{}{}", "dual-", "buffer"),
            format!("{}{}", "channel-before-", "pending"),
            format!("{}{}", "channel holds ", "older"),
            format!("{}{}", "fence pending holds ", "newer"),
            format!("{}{}", "Channel first ", "(older"),
        ];
        for term in &banned {
            assert!(
                !production.contains(term.as_str()),
                "local_process.rs production source must not reintroduce dual-buffer \
                 transfer symbol `{term}` (red on dual-buffer SHA df38c218; green on HEAD)"
            );
        }
    }

    #[test]
    fn concurrent_drain_does_not_leak_reader_depth() {
        // Enqueue+depth under one lock: after a concurrent-style drain of the
        // only event, depth must be zero (not left at 1 from a late increment).
        let fence = test_fence(4);
        let pressure = Arc::new(ReaderPressure::default());
        fence
            .push_pending(ReaderEvent::Output(b"one".to_vec()), &pressure, 4)
            .expect("push");
        assert_eq!(pressure.depth.load(Ordering::Acquire), 1);
        let (out, fail) = drain_fence_pending(&fence, &test_session_id(), &pressure);
        assert!(fail.is_none());
        assert_eq!(out.len(), 1);
        assert_eq!(
            pressure.depth.load(Ordering::Acquire),
            0,
            "depth must match empty queue after drain"
        );
        // A second push after drain tracks occupancy again.
        fence
            .push_pending(ReaderEvent::Output(b"two".to_vec()), &pressure, 4)
            .expect("second push");
        assert_eq!(pressure.depth.load(Ordering::Acquire), 1);
        let _ = drain_fence_pending(&fence, &test_session_id(), &pressure);
        assert_eq!(pressure.depth.load(Ordering::Acquire), 0);
    }

    #[test]
    fn pending_overflow_does_not_drop_prior_events() {
        let fence = test_fence(1);
        let pressure = Arc::new(ReaderPressure::default());
        fence
            .push_pending(ReaderEvent::Output(b"keep-me".to_vec()), &pressure, 1)
            .expect("first fits");
        assert_eq!(pressure.depth.load(Ordering::Acquire), 1);
        let rejected = fence
            .push_pending(ReaderEvent::Output(b"overflow".to_vec()), &pressure, 1)
            .expect_err("second must not drop first");
        assert!(matches!(
            rejected,
            ReaderEvent::Output(data) if data == b"overflow"
        ));
        assert_eq!(pressure.depth.load(Ordering::Acquire), 1);
        let pending = fence.pending.lock().expect("lock");
        assert_eq!(pending.len(), 1);
        assert!(matches!(
            pending.front(),
            Some(ReaderEvent::Output(data)) if data == b"keep-me"
        ));
        // Ordinary pressure marks pressured without latching overflow.
        assert!(pressure.pressured.load(Ordering::Acquire));
        assert!(fence.take_overflow_error().is_none());
    }

    #[test]
    fn pending_full_wait_is_lossless_after_drain() {
        // Capacity-full try-push returns the event; after drain frees space the
        // same event enqueues without sticky overflow (ordinary pressure path).
        let fence = test_fence(1);
        let pressure = Arc::new(ReaderPressure::default());
        fence
            .push_pending(ReaderEvent::Output(b"first".to_vec()), &pressure, 1)
            .expect("first");
        let rejected = fence
            .push_pending(ReaderEvent::Output(b"second".to_vec()), &pressure, 1)
            .expect_err("full");
        assert!(fence.take_overflow_error().is_none());

        let fence_w = Arc::clone(&fence);
        let pressure_w = Arc::clone(&pressure);
        let waiter = thread::spawn(move || {
            let mut event = rejected;
            loop {
                match fence_w.push_pending(event, &pressure_w, 1) {
                    Ok(()) => break,
                    Err(returned) => {
                        event = returned;
                        fence_w.wait_for_pending_space();
                    }
                }
            }
        });

        thread::sleep(Duration::from_millis(20));
        let (out, fail) = drain_fence_pending(&fence, &test_session_id(), &pressure);
        assert!(fail.is_none());
        assert_eq!(out.len(), 1);
        waiter.join().expect("waiter");
        assert!(fence.take_overflow_error().is_none());
        let (out2, fail2) = drain_fence_pending(&fence, &test_session_id(), &pressure);
        assert!(fail2.is_none());
        assert_eq!(out2.len(), 1, "second chunk retained after wait+drain");
    }

    #[test]
    fn overflow_latches_sticky_authority_after_retained_drain() {
        // Forced-loss / true-loss sticky path (internal set_overflow_error seam —
        // not ordinary capacity pressure, not a public production flag).
        let fence = test_fence(1);
        let pressure = Arc::new(ReaderPressure::default());
        fence
            .push_pending(ReaderEvent::Output(b"keep".to_vec()), &pressure, 1)
            .expect("push");
        fence.set_overflow_error("pty reader buffer overflow: mode authority incomplete");
        let (out, fail) = drain_fence_pending(&fence, &test_session_id(), &pressure);
        assert!(fail.is_none());
        assert_eq!(out.len(), 1);
        assert_eq!(pressure.depth.load(Ordering::Acquire), 0);
        let sticky = fence.take_overflow_error();
        assert!(sticky.as_deref().unwrap_or("").contains("overflow"));
        assert!(fence.take_overflow_error().is_none());
    }

    #[test]
    fn reader_failed_event_latches_sticky_failure_after_retained_output() {
        // True reader failure is retained as Failed and latches on drain without
        // dropping prior Output events (mode-authority fail-closed after true loss).
        let fence = test_fence(4);
        let pressure = Arc::new(ReaderPressure::default());
        fence
            .push_pending(ReaderEvent::Output(b"keep".to_vec()), &pressure, 4)
            .expect("output");
        fence
            .push_pending(
                ReaderEvent::Failed("read pty output failed: injected".to_string()),
                &pressure,
                4,
            )
            .expect("failed");
        let (out, fail) = drain_fence_pending(&fence, &test_session_id(), &pressure);
        assert_eq!(
            out.len(),
            1,
            "retained output delivered before failure latch"
        );
        assert_eq!(
            fail.as_deref(),
            Some("read pty output failed: injected"),
            "Failed event must latch sticky failure for ensure_mode_authority"
        );
    }

    #[test]
    fn reader_finished_marks_disconnected_for_finalization() {
        let fence = test_fence(2);
        let pressure = Arc::new(ReaderPressure::default());
        fence
            .push_pending(ReaderEvent::Output(b"tail".to_vec()), &pressure, 2)
            .expect("push");
        fence.mark_reader_finished();
        assert!(fence.reader_finished());
        let (out, _) = drain_fence_pending(&fence, &test_session_id(), &pressure);
        assert_eq!(out.len(), 1);
        assert!(reader_finalization_complete(true, None, None,));
    }

    #[test]
    fn write_all_blocking_reports_partial_bytes_after_deadline() {
        struct ChunkWriter {
            limit: usize,
            written: usize,
        }
        impl Write for ChunkWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if self.written >= self.limit {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "blocked"));
                }
                let n = 1.min(buf.len());
                self.written += n;
                Ok(n)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut writer: Box<dyn Write + Send> = Box::new(ChunkWriter {
            limit: 1,
            written: 0,
        });
        let hooks = WriteTestHooks {
            force_would_block_until_unix_ms: Some(unix_now_ms() + 5_000),
            max_chunk: Some(1),
            writes_completed: AtomicUsize::new(0),
            fail_writes: false,
        };
        let deadline = unix_now_ms() + 30;
        let err = write_all_blocking(&mut writer, b"abcdef", Some(deadline), Some(&hooks))
            .expect_err("must partial-fail");
        assert_eq!(err.bytes_written, 1);
        assert!(
            err.message.contains("deadline"),
            "expected deadline detail, got {}",
            err.message
        );
    }
}
