//! Local session runtime backed by a separate worker process.

use std::collections::{HashMap, HashSet};
use std::io::{self, ErrorKind, Read, Write};
use std::path::PathBuf;
#[cfg(unix)]
use std::process::ChildStdout;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::net::Shutdown;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{FileTypeExt, MetadataExt};
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(unix)]
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use sha2::{Digest, Sha256};

use crate::contract::terminal_wake::{SessionWakeHandle, TerminalWakeSource};
use crate::runtime::control_queue::{
    write_slice_timeout, ControlAdmission, ControlFrameClass, ControlPlaneState, ControlQueue,
    ControlQueueAdmitError, ControlWriterError, ControlWriterOutcome, ControlWriterSlot,
    WORKER_CONTROL_WRITER_JOIN_BOUND, WORKER_CONTROL_WRITE_TIMEOUT,
};
use crate::{
    read_welcome, write_hello, BackpressureRoute, BackpressureSummary, ClientId, Frame, ModeFlags,
    ModeFlagsPayload, ModeFreshnessToken, ModeGatedCancelRequest, ModeGatedPtyInputRequest,
    ModeGatedPtyInputResult, NotificationPayload, ProcessExitedPayload, ProcessIdentity,
    PromptMarkPayload, QueueSource, SessionId, SessionMetadata, SessionRuntime,
    SessionRuntimeError, SessionRuntimeErrorKind, SessionRuntimeHandle, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest, SubscriptionId, TerminalMetadataShapingObservation,
    TimeoutPayload, WorkerSnapshotRequest, WorkerSnapshotResult, FRAME_BELL, FRAME_CWD_CHANGED,
    FRAME_GET_MODE_FLAGS, FRAME_METADATA_SHAPING, FRAME_MODE_FLAGS, FRAME_MODE_GATED_CANCEL,
    FRAME_MODE_GATED_PTY_INPUT, FRAME_MODE_GATED_PTY_INPUT_RESULT, FRAME_NOTIFICATION, FRAME_PING,
    FRAME_PONG, FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK, FRAME_PTY_INPUT, FRAME_PTY_OUTPUT,
    FRAME_RESIZE, FRAME_RESIZE_APPLIED, FRAME_SET_TIMEOUT, FRAME_SHUTDOWN, FRAME_SNAPSHOT,
    FRAME_SPAWN_SESSION, FRAME_TITLE_CHANGED, PROTOCOL_VERSION,
};

/// Default retained worker egress frames per session in the parent process.
pub const DEFAULT_WORKER_EGRESS_CAPACITY: usize = 64;

/// Default parent wait bound for mode-gated PTY input RPC.
pub const DEFAULT_MODE_GATED_INPUT_TIMEOUT: Duration = Duration::from_secs(5);

/// Extra parent wait after the worker write deadline so a correlated
/// `deadline_exceeded` (or other) result can demux under load before the
/// parent clears the in-flight slot and fails closed as a timeout.
const MODE_GATED_REPLY_GRACE: Duration = Duration::from_secs(1);

/// Correlated id for one in-flight mode-gated request.
pub type GatedRequestId = String;

/// Non-blocking poll of one session's gated lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatedPoll {
    /// No gated request is outstanding.
    Idle,
    /// A request is outstanding and the deadline has not expired.
    Pending,
    /// The worker returned a correlated result.
    Ready(ModeGatedPtyInputResult),
    /// The parent wait expired without a correlated result.
    TimedOut,
}

const PING_WAIT: Duration = Duration::from_secs(2);
const PING_POLL: Duration = Duration::from_millis(10);
const WORKER_REAP_GRACE: Duration = Duration::from_secs(2);
const WORKER_REAP_POLL: Duration = Duration::from_millis(10);
const GATED_POLL: Duration = Duration::from_millis(5);
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

/// Test-only parent-side gate that holds `FRAME_RESIZE_APPLIED` for one session.
#[derive(Clone)]
pub struct ResizeAckHold {
    session_id: crate::SessionId,
    inner: Arc<ResizeAckHoldInner>,
}

struct ResizeAckHoldInner {
    held: Mutex<bool>,
    cvar: Condvar,
}

impl PartialEq for ResizeAckHold {
    fn eq(&self, other: &Self) -> bool {
        self.session_id == other.session_id && Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for ResizeAckHold {}

impl std::fmt::Debug for ResizeAckHold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResizeAckHold")
            .field("session_id", &self.session_id)
            .finish()
    }
}

impl ResizeAckHold {
    /// Create a released gate for `session_id`. Call [`Self::arm`] after attach.
    #[must_use]
    pub fn for_session(session_id: crate::SessionId) -> Self {
        Self {
            session_id,
            inner: Arc::new(ResizeAckHoldInner {
                held: Mutex::new(false),
                cvar: Condvar::new(),
            }),
        }
    }

    /// Session whose reader thread waits on this gate.
    #[must_use]
    pub fn session_id(&self) -> &crate::SessionId {
        &self.session_id
    }

    /// Hold the next matching acknowledgement until [`Self::release`].
    pub fn arm(&self) {
        let Ok(mut held) = self.inner.held.lock() else {
            return;
        };
        *held = true;
    }

    /// Allow the matching reader thread to emit the held acknowledgement.
    pub fn release(&self) {
        let Ok(mut held) = self.inner.held.lock() else {
            return;
        };
        *held = false;
        self.inner.cvar.notify_all();
    }

    fn wait_if_session(&self, session_id: &crate::SessionId) {
        if &self.session_id != session_id {
            return;
        }
        let Ok(mut held) = self.inner.held.lock() else {
            return;
        };
        while *held {
            held = match self.inner.cvar.wait(held) {
                Ok(guard) => guard,
                Err(_) => return,
            };
        }
    }
}

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
    /// Parent wait bound for correlated mode-gated PTY input RPC.
    pub mode_gated_input_timeout: Duration,
    /// Optional per-request worker admit hold for deterministic race tests.
    pub test_mode_gated_hold_ms: Option<u64>,
    /// Test-only: hold after PTY read while still in the reader critical section (worker CLI).
    pub test_hold_after_read_ms: Option<u64>,
    /// Test-only: force write WouldBlock until this Unix ms (worker CLI).
    pub test_write_block_until_unix_ms: Option<u64>,
    /// Test-only: cap each write() to this many bytes (partial-write proofs).
    pub test_write_max_chunk: Option<usize>,
    /// Test-only: single-queue fence capacity override (overflow proofs).
    pub test_pending_capacity: Option<usize>,
    /// Test-only: hold after fence enqueue while still critical.
    pub test_hold_after_enqueue_ms: Option<u64>,
    /// Test-only: fail snapshot encode when the first history PAGE is ready.
    pub test_fail_snapshot_history_after_ready: bool,
    /// Test-only: omit resize acknowledgments after successful worker application.
    pub test_omit_resize_applied: bool,
    /// Test-only: hold `FRAME_RESIZE_APPLIED` in the parent reader for one session.
    pub test_resize_ack_hold: Option<ResizeAckHold>,
    /// Test-only: hold after FRAME_PROCESS_EXITED with stdout still open.
    pub test_hold_before_exit_ms: Option<u64>,
    /// Test-only: worker process exit code after the payload is flushed.
    pub test_exit_code: Option<i32>,
    /// Ghostty scrollback byte budget used by the worker snapshot authority.
    pub ghostty_max_scrollback_bytes: usize,
    /// Optional initial color policy used by the worker snapshot authority.
    pub terminal_color_profile: Option<crate::TerminalColorProfile>,
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
            mode_gated_input_timeout: DEFAULT_MODE_GATED_INPUT_TIMEOUT,
            test_mode_gated_hold_ms: None,
            test_hold_after_read_ms: None,
            test_write_block_until_unix_ms: None,
            test_write_max_chunk: None,
            test_pending_capacity: None,
            test_hold_after_enqueue_ms: None,
            test_fail_snapshot_history_after_ready: false,
            test_omit_resize_applied: false,
            test_resize_ack_hold: None,
            test_hold_before_exit_ms: None,
            test_exit_code: None,
            ghostty_max_scrollback_bytes: 10_000_000,
            terminal_color_profile: None,
        }
    }

    /// Override the mode-gated input wait bound (tests may use a short timeout).
    #[must_use]
    pub const fn with_mode_gated_input_timeout(mut self, timeout: Duration) -> Self {
        self.mode_gated_input_timeout = timeout;
        self
    }

    /// Set a per-request worker admit hold for deterministic race tests.
    #[must_use]
    pub const fn with_test_mode_gated_hold_ms(mut self, hold_ms: Option<u64>) -> Self {
        self.test_mode_gated_hold_ms = hold_ms;
        self
    }

    /// Set the test-only after-read hold for unpublished-chunk race proofs.
    #[must_use]
    pub const fn with_test_hold_after_read_ms(mut self, hold_ms: Option<u64>) -> Self {
        self.test_hold_after_read_ms = hold_ms;
        self
    }

    /// Set the test-only write backpressure deadline for timeout proofs.
    #[must_use]
    pub const fn with_test_write_block_until_unix_ms(mut self, until: Option<u64>) -> Self {
        self.test_write_block_until_unix_ms = until;
        self
    }

    /// Set the test-only per-call write cap for partial-write proofs.
    #[must_use]
    pub const fn with_test_write_max_chunk(mut self, max_chunk: Option<usize>) -> Self {
        self.test_write_max_chunk = max_chunk;
        self
    }

    /// Set the test-only single-queue fence capacity for overflow proofs.
    #[must_use]
    pub const fn with_test_pending_capacity(mut self, capacity: Option<usize>) -> Self {
        self.test_pending_capacity = capacity;
        self
    }

    /// Enable a deterministic post-READY history encode failure.
    #[must_use]
    pub const fn with_test_fail_snapshot_history_after_ready(mut self, enabled: bool) -> Self {
        self.test_fail_snapshot_history_after_ready = enabled;
        self
    }

    /// Set the test-only post-enqueue hold while still under the admission fence.
    #[must_use]
    pub const fn with_test_hold_after_enqueue_ms(mut self, hold_ms: Option<u64>) -> Self {
        self.test_hold_after_enqueue_ms = hold_ms;
        self
    }

    /// Hold after the worker sends FRAME_PROCESS_EXITED with stdout still open.
    #[must_use]
    pub const fn with_test_hold_before_exit_ms(mut self, hold_ms: Option<u64>) -> Self {
        self.test_hold_before_exit_ms = hold_ms;
        self
    }

    /// Exit the worker with this code after the ProcessExited payload is flushed.
    #[must_use]
    pub const fn with_test_exit_code(mut self, exit_code: Option<i32>) -> Self {
        self.test_exit_code = exit_code;
        self
    }

    /// Override retained PTY reader chunk capacity inside the worker process.
    #[must_use]
    pub const fn with_pty_reader_chunk_capacity(mut self, capacity: usize) -> Self {
        self.pty_reader_chunk_capacity = capacity;
        self
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

/// Nonblocking snapshot-boundary updates from one worker-owned encode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerSnapshotBoundaryPoll {
    /// Correlated record-aware snapshot frames in worker FIFO order.
    pub frames: Vec<WorkerSnapshotResult>,
    /// PTY output that precedes READY and is already present in the snapshot.
    pub before_ready: Vec<SessionRuntimeOutput>,
    /// Whether FINISH or an encode error ended the worker barrier.
    pub complete: bool,
}

/// Parent-side runtime adapter for one-worker-process-per-session local PTYs.
pub struct WorkerProcessRuntime {
    options: WorkerProcessRuntimeOptions,
    sessions: HashMap<SessionId, WorkerProcessSession>,
    wake_source: Option<TerminalWakeSource>,
    release_on_drop: bool,
    fail_next_start_writer: bool,
    #[cfg(test)]
    fail_next_snapshot_cancel_count: usize,
    #[cfg(test)]
    fail_next_snapshot_begin_count: usize,
    #[cfg(test)]
    fail_next_pre_ready_error: bool,
}

impl WorkerProcessRuntime {
    /// Return whether one worker supports the atomic snapshot boundary RPC.
    pub fn supports_snapshot_boundary(
        &self,
        session_id: &SessionId,
    ) -> Result<bool, SessionRuntimeError> {
        self.sessions
            .get(session_id)
            .map(|session| session.supports_snapshot_boundary)
            .ok_or_else(|| {
                SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SessionNotFound,
                    format!("worker process session not found: {}", session_id.0),
                )
            })
    }
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
            wake_source: None,
            release_on_drop: false,
            fail_next_start_writer: false,
            #[cfg(test)]
            fail_next_snapshot_cancel_count: 0,
            #[cfg(test)]
            fail_next_snapshot_begin_count: 0,
            #[cfg(test)]
            fail_next_pre_ready_error: false,
        }
    }

    /// Share the engine wake source with worker stdout reader threads.
    #[must_use]
    pub fn with_wake_source(mut self, source: TerminalWakeSource) -> Self {
        self.wake_source = Some(source);
        self
    }

    /// Fail the next `start_writer` after the session wake handle is allocated.
    pub fn fail_next_start_writer(&mut self) {
        self.fail_next_start_writer = true;
    }

    fn forget_session_wake(&self, session_id: &SessionId) {
        if let Some(source) = &self.wake_source {
            source.forget_session(session_id);
        }
    }

    fn start_writer_or_forget(
        &mut self,
        session: &mut WorkerProcessSession,
        session_id: &SessionId,
    ) -> Result<(), SessionRuntimeError> {
        if self.fail_next_start_writer {
            self.fail_next_start_writer = false;
            self.forget_session_wake(session_id);
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "test-injected start_writer failure",
            ));
        }
        session.start_writer().inspect_err(|_| {
            self.forget_session_wake(session_id);
        })
    }

    /// Fail the next snapshot cancel write. Crate tests use this to prove fail-closed takeover.
    #[cfg(test)]
    pub(crate) fn fail_next_snapshot_cancel(&mut self) {
        self.fail_next_snapshot_cancel_count = 1;
    }

    /// Fail the next `count` snapshot begin writes.
    #[cfg(test)]
    pub(crate) fn fail_next_snapshot_begins(&mut self, count: usize) {
        self.fail_next_snapshot_begin_count = count;
    }

    /// Fail the next snapshot poll before READY. Crate tests use this for fail-closed promotion.
    #[cfg(test)]
    pub(crate) fn fail_next_pre_ready_snapshot(&mut self) {
        self.fail_next_pre_ready_error = true;
    }

    /// Cancel the live snapshot request without completing attach. Crate tests use this to reach reconcile.
    #[cfg(test)]
    pub(crate) fn cancel_outstanding_snapshot(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), SessionRuntimeError> {
        let request_id = self
            .session_mut(session_id)?
            .outstanding_snapshot_request
            .clone();
        if let Some(request_id) = request_id {
            self.cancel_snapshot_boundary(session_id, &request_id)?;
        }
        Ok(())
    }

    /// Return worker welcome metadata captured after spawning a session.
    #[must_use]
    pub fn metadata(&self, session_id: &SessionId) -> Option<&SessionMetadata> {
        self.sessions
            .get(session_id)
            .map(|session| &session.metadata)
    }

    /// Clone the session control queue for crate unit tests.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn test_control_queue(&self, session_id: &SessionId) -> Option<ControlQueue> {
        self.sessions
            .get(session_id)
            .map(|session| session.control_queue.clone())
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
    ///
    /// Direct runtime tests use this without a subscription identity. Engine
    /// attach paths should call [`Self::replace_named_consumers`] from live
    /// subscription ownership instead of incrementing a scalar.
    pub fn attach_consumer(&mut self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        self.session_mut(session_id)?.stall.insert_direct()
    }

    /// Mark a parent-side consumer detached from worker egress.
    ///
    /// Removes only the direct test consumer. Named subscription owners are
    /// replaced as a set from live attach state.
    pub fn detach_consumer(&mut self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        self.session_mut(session_id)?.stall.remove_direct()
    }

    /// Replace named live consumers for one session from subscription ownership.
    pub fn replace_named_consumers(
        &mut self,
        session_id: &SessionId,
        owners: impl IntoIterator<Item = (ClientId, SubscriptionId)>,
    ) -> Result<(), SessionRuntimeError> {
        self.session_mut(session_id)?.stall.replace_named(owners)
    }

    /// Send a ping frame and wait for typed worker health evidence.
    pub fn ping(&mut self, session_id: &SessionId) -> Result<WorkerHealth, SessionRuntimeError> {
        let session = self.session_mut(session_id)?;
        let before = session.pong_count.load(Ordering::Acquire);
        session.enqueue_frame(ControlFrameClass::Ordinary, FRAME_PING, &[])?;
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
        session.enqueue_json(
            ControlFrameClass::Ordinary,
            FRAME_SET_TIMEOUT,
            &TimeoutPayload { seconds },
        )
    }

    /// Read worker-authoritative mode flags and freshness token.
    pub fn read_mode_flags(
        &mut self,
        session_id: &SessionId,
    ) -> Result<ModeFlagsPayload, SessionRuntimeError> {
        let request_id = next_gated_request_id();
        {
            let session = self.session_mut(session_id)?;
            if session
                .gated_in_flight
                .lock()
                .map_err(lock_error)?
                .is_some()
            {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::InputFailed,
                    "mode-gated request already in flight for session",
                ));
            }
            *session.mode_flags_slot.lock().map_err(lock_error)? = None;
            *session.outstanding_mode_probe.lock().map_err(lock_error)? = Some(request_id.clone());
            session.enqueue_json(
                ControlFrameClass::Ordinary,
                FRAME_GET_MODE_FLAGS,
                &ModeFlagsProbeRequest {
                    request_id: request_id.clone(),
                },
            )?;
        }
        let deadline = Instant::now() + self.options.mode_gated_input_timeout;
        loop {
            self.pump_session_output(session_id)?;
            let matched = {
                let session = self.session_mut(session_id)?;
                let mut slot = session.mode_flags_slot.lock().map_err(lock_error)?;
                match slot.take() {
                    Some(payload) if payload.request_id == request_id => {
                        *session.outstanding_mode_probe.lock().map_err(lock_error)? = None;
                        Some(payload)
                    }
                    Some(_) => None, // stale/mismatched probe reply
                    None => None,
                }
            };
            if let Some(payload) = matched {
                if let Some(error_kind) = payload.error_kind {
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        error_kind,
                    ));
                }
                return Ok(payload);
            }
            if Instant::now() >= deadline {
                let session = self.session_mut(session_id)?;
                *session.outstanding_mode_probe.lock().map_err(lock_error)? = None;
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::OutputFailed,
                    "worker mode-flags probe timed out",
                ));
            }
            if self.session_reader_finished(session_id)? {
                let session = self.session_mut(session_id)?;
                *session.outstanding_mode_probe.lock().map_err(lock_error)? = None;
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::OutputFailed,
                    "worker disconnected before mode-flags reply",
                ));
            }
            thread::sleep(GATED_POLL);
        }
    }

    /// Return flags for the latest token decoded in this daemon incarnation.
    #[must_use]
    pub fn latest_mode_for(
        &self,
        session_id: &SessionId,
        token: ModeFreshnessToken,
    ) -> Option<ModeFlags> {
        self.sessions
            .get(session_id)?
            .latest_mode
            .lock()
            .ok()?
            .as_ref()
            .filter(|(latest, _)| *latest == token)
            .map(|(_, flags)| flags.clone())
    }

    /// Capture a worker-owned snapshot after all pre-boundary PTY bytes.
    ///
    /// The returned output precedes the snapshot boundary on the worker's
    /// protected FIFO. Output after the boundary remains queued for a later drain.
    pub fn capture_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(crate::TerminalSnapshotPayload, Vec<SessionRuntimeOutput>), SessionRuntimeError>
    {
        let request_id = self.begin_snapshot_boundary(session_id)?;

        let deadline = Instant::now() + self.options.mode_gated_input_timeout;
        let mut bytes = Vec::new();
        let mut before_ready = Vec::new();
        loop {
            let poll = match self.poll_snapshot_boundary(session_id, &request_id) {
                Ok(poll) => poll,
                Err(error) => {
                    let _ = self.cancel_snapshot_boundary(session_id, &request_id);
                    return Err(error);
                }
            };
            before_ready.extend(poll.before_ready);
            for frame in poll.frames {
                if let Some(error) = frame.error_kind {
                    let _ = self.cancel_snapshot_boundary(session_id, &request_id);
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        error,
                    ));
                }
                let Some(phase) = frame.phase else {
                    let _ = self.cancel_snapshot_boundary(session_id, &request_id);
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        "worker snapshot frame omitted its phase",
                    ));
                };
                let Some(snapshot) = frame.snapshot else {
                    let _ = self.cancel_snapshot_boundary(session_id, &request_id);
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        "worker snapshot frame omitted its bytes",
                    ));
                };
                let size = snapshot.size;
                let format = snapshot.format;
                bytes.extend(snapshot.bytes);
                if phase == crate::WorkerSnapshotPhase::Finish {
                    if let Err(error) = self.complete_snapshot_boundary(session_id, &request_id) {
                        let _ = self.cancel_snapshot_boundary(session_id, &request_id);
                        return Err(error);
                    }
                    return Ok((
                        crate::TerminalSnapshotPayload::new(bytes, size, format),
                        before_ready,
                    ));
                }
            }
            if Instant::now() >= deadline {
                let _ = self.cancel_snapshot_boundary(session_id, &request_id);
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::OutputFailed,
                    "worker snapshot request timed out",
                ));
            }
            match self.session_reader_finished(session_id) {
                Ok(true) => {
                    let _ = self.cancel_snapshot_boundary(session_id, &request_id);
                    self.session_mut(session_id)?.outstanding_snapshot_request = None;
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        "worker disconnected before snapshot response",
                    ));
                }
                Ok(false) => {}
                Err(error) => {
                    let _ = self.cancel_snapshot_boundary(session_id, &request_id);
                    return Err(error);
                }
            }
            thread::sleep(GATED_POLL);
        }
    }

    /// Start one worker-owned snapshot encode without waiting for READY.
    pub fn begin_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
    ) -> Result<String, SessionRuntimeError> {
        #[cfg(test)]
        if self.fail_next_snapshot_begin_count > 0 {
            self.fail_next_snapshot_begin_count -= 1;
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                "injected snapshot begin failure",
            ));
        }
        let request_id = next_gated_request_id();
        let session = self.session_mut(session_id)?;
        if session.outstanding_snapshot_request.is_some() {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                "worker snapshot request already in flight",
            ));
        }
        session.snapshot_boundary.clear();
        session.enqueue_json(
            ControlFrameClass::Ordinary,
            crate::FRAME_GET_SNAPSHOT,
            &WorkerSnapshotRequest {
                request_id: request_id.clone(),
                cancel: false,
                complete: false,
            },
        )?;
        session.outstanding_snapshot_request = Some(request_id.clone());
        Ok(request_id)
    }

    /// Poll ordered frames for one in-progress worker snapshot encode.
    pub fn poll_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> Result<WorkerSnapshotBoundaryPoll, SessionRuntimeError> {
        self.pump_session_output(session_id)?;
        #[cfg(test)]
        if self.fail_next_pre_ready_error {
            self.fail_next_pre_ready_error = false;
            return Ok(WorkerSnapshotBoundaryPoll {
                frames: vec![crate::WorkerSnapshotResult {
                    request_id: request_id.to_owned(),
                    snapshot: None,
                    phase: None,
                    error_kind: Some("injected pre-ready failure".to_string()),
                    barrier_released: false,
                }],
                before_ready: Vec::new(),
                complete: false,
            });
        }
        let session = self.session_mut(session_id)?;
        if session.outstanding_snapshot_request.as_deref() != Some(request_id) {
            return Ok(WorkerSnapshotBoundaryPoll {
                frames: Vec::new(),
                before_ready: Vec::new(),
                complete: true,
            });
        }

        let mut frames = Vec::new();
        let mut before_ready = Vec::new();
        let mut complete = false;
        while let Some((frame, boundary_len)) = session.snapshot_boundary.pop_front() {
            if frame.request_id != request_id {
                continue;
            }
            if frame.phase == Some(crate::WorkerSnapshotPhase::Ready) {
                before_ready.extend(
                    session
                        .pending_output
                        .drain(..boundary_len.min(session.pending_output.len()))
                        .map(|event| event.into_runtime_output(session_id)),
                );
            }
            if frame.barrier_released {
                if frame.error_kind.is_some() {
                    frames.push(frame);
                }
                session.outstanding_snapshot_request = None;
                complete = true;
                break;
            }
            frames.push(frame);
        }
        Ok(WorkerSnapshotBoundaryPoll {
            frames,
            before_ready,
            complete,
        })
    }

    /// Cancel one in-progress encode and release the worker PTY barrier.
    pub fn cancel_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> Result<(), SessionRuntimeError> {
        #[cfg(test)]
        if self.fail_next_snapshot_cancel_count > 0 {
            self.fail_next_snapshot_cancel_count -= 1;
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::OutputFailed,
                "injected snapshot cancel failure",
            ));
        }
        let session = self.session_mut(session_id)?;
        session.enqueue_json(
            ControlFrameClass::Ordinary,
            crate::FRAME_GET_SNAPSHOT,
            &WorkerSnapshotRequest {
                request_id: request_id.to_owned(),
                cancel: true,
                complete: false,
            },
        )?;
        session.outstanding_snapshot_request = None;
        session.snapshot_boundary.clear();
        Ok(())
    }

    /// Apply any staged resize and wait for the worker to release the PTY barrier.
    pub fn complete_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> Result<(), SessionRuntimeError> {
        {
            let session = self.session_mut(session_id)?;
            session.enqueue_json(
                ControlFrameClass::Ordinary,
                crate::FRAME_GET_SNAPSHOT,
                &WorkerSnapshotRequest {
                    request_id: request_id.to_owned(),
                    cancel: false,
                    complete: true,
                },
            )?;
        }
        let deadline = Instant::now() + self.options.mode_gated_input_timeout;
        loop {
            let poll = match self.poll_snapshot_boundary(session_id, request_id) {
                Ok(poll) => poll,
                Err(error) => {
                    let _ = self.cancel_snapshot_boundary(session_id, request_id);
                    return Err(error);
                }
            };
            if let Some(error) = poll.frames.into_iter().find_map(|frame| frame.error_kind) {
                let _ = self.cancel_snapshot_boundary(session_id, request_id);
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::OutputFailed,
                    error,
                ));
            }
            if poll.complete {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let _ = self.cancel_snapshot_boundary(session_id, request_id);
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::OutputFailed,
                    "worker snapshot barrier release timed out",
                ));
            }
            match self.session_reader_finished(session_id) {
                Ok(true) => {
                    let _ = self.cancel_snapshot_boundary(session_id, request_id);
                    self.session_mut(session_id)?.outstanding_snapshot_request = None;
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        "worker disconnected before snapshot barrier release",
                    ));
                }
                Ok(false) => {}
                Err(error) => {
                    let _ = self.cancel_snapshot_boundary(session_id, request_id);
                    return Err(error);
                }
            }
            thread::sleep(GATED_POLL);
        }
    }

    /// Submit one mode-gated PTY input and return immediately.
    ///
    /// Claims the per-session lane and enqueues `FRAME_MODE_GATED_PTY_INPUT`.
    /// Does not wait for a worker reply and does not write the control socket.
    pub fn submit_mode_gated_pty_input(
        &mut self,
        session_id: &SessionId,
        expected: ModeFreshnessToken,
        data: Vec<u8>,
    ) -> Result<GatedRequestId, SessionRuntimeError> {
        let request_id = next_gated_request_id();
        let timeout = self.options.mode_gated_input_timeout;
        let test_hold_ms = self.options.test_mode_gated_hold_ms;
        let deadline_unix_ms = unix_now_ms().saturating_add(timeout.as_millis() as u64);
        let parent_deadline = Instant::now() + timeout + MODE_GATED_REPLY_GRACE;
        let session = self.session_mut(session_id)?;
        let mut in_flight = session.gated_in_flight.lock().map_err(lock_error)?;
        if in_flight.is_some() {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::InputFailed,
                "mode-gated request already in flight for session",
            ));
        }
        *in_flight = Some(GatedInFlight {
            request_id: request_id.clone(),
            result: None,
            cancelled: false,
            parent_deadline,
        });
        drop(in_flight);
        if let Err(error) = session.enqueue_json(
            ControlFrameClass::Ordinary,
            FRAME_MODE_GATED_PTY_INPUT,
            &ModeGatedPtyInputRequest {
                request_id: request_id.clone(),
                expected,
                data,
                deadline_unix_ms,
                test_hold_ms,
            },
        ) {
            if let Ok(mut in_flight) = session.gated_in_flight.lock() {
                *in_flight = None;
            }
            return Err(error);
        }
        Ok(request_id)
    }

    /// Pump output once and inspect the gated slot. Never sleeps.
    pub fn poll_mode_gated_pty_input(
        &mut self,
        session_id: &SessionId,
    ) -> Result<GatedPoll, SessionRuntimeError> {
        self.pump_session_output(session_id)?;
        let session = self.session_mut(session_id)?;
        let mut in_flight = session.gated_in_flight.lock().map_err(lock_error)?;
        let Some(slot) = in_flight.as_mut() else {
            return Ok(GatedPoll::Idle);
        };
        if let Some(result) = slot.result.take() {
            *in_flight = None;
            return result.map(GatedPoll::Ready);
        }
        if Instant::now() >= slot.parent_deadline {
            *in_flight = None;
            return Ok(GatedPoll::TimedOut);
        }
        Ok(GatedPoll::Pending)
    }

    /// Enqueue a cancel for one abandoned gated request. Never writes the socket.
    ///
    /// The parent lane stays occupied until the correlated reply arrives or the
    /// parent wait expires.
    pub fn cancel_mode_gated_pty_input(
        &mut self,
        session_id: &SessionId,
        request_id: &GatedRequestId,
    ) -> Result<(), SessionRuntimeError> {
        self.enqueue_gated_cancel(session_id, request_id)
    }

    fn enqueue_gated_cancel(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> Result<(), SessionRuntimeError> {
        let session = self.session_mut(session_id)?;
        let mut in_flight = session.gated_in_flight.lock().map_err(lock_error)?;
        match in_flight.as_mut() {
            Some(slot) if slot.request_id == request_id => {
                if slot.cancelled {
                    return Ok(());
                }
                slot.cancelled = true;
            }
            _ => return Ok(()),
        }
        drop(in_flight);
        session.enqueue_json(
            ControlFrameClass::Cancel,
            FRAME_MODE_GATED_CANCEL,
            &ModeGatedCancelRequest {
                request_id: request_id.to_owned(),
            },
        )
    }

    /// Whether the session gated lane is occupied, including a cancelled hold.
    #[must_use]
    pub fn has_gated_in_flight(&self, session_id: &SessionId) -> bool {
        self.sessions
            .get(session_id)
            .and_then(|session| session.gated_in_flight.lock().ok())
            .is_some_and(|slot| slot.is_some())
    }

    /// Sessions whose gated lane is occupied. Stage B uses the full set because
    /// intake walks every live owner, not only the drained session.
    #[must_use]
    pub fn sessions_holding_gated(&self) -> HashSet<SessionId> {
        self.sessions
            .iter()
            .filter(|(_, session)| {
                session
                    .gated_in_flight
                    .lock()
                    .ok()
                    .is_some_and(|slot| slot.is_some())
            })
            .map(|(session_id, _)| session_id.clone())
            .collect()
    }

    /// Current durable control-plane state.
    #[must_use]
    pub fn control_plane_state(&self, session_id: &SessionId) -> ControlPlaneState {
        self.sessions
            .get(session_id)
            .map(|session| session.control_plane.clone())
            .unwrap_or(ControlPlaneState::Live)
    }

    /// Probe whether one ordinary frame can enter a session control queue.
    #[must_use]
    pub(crate) fn probe_ordinary(&self, session_id: &SessionId) -> ControlAdmission {
        self.sessions
            .get(session_id)
            .map(|session| session.control_queue.probe_ordinary())
            .unwrap_or(ControlAdmission::Sealed)
    }

    /// Observe the writer outcome without consuming a failure.
    #[must_use]
    pub fn control_writer_outcome(&self, session_id: &SessionId) -> ControlWriterOutcome {
        self.sessions
            .get(session_id)
            .map(|session| session.writer_slot.get())
            .unwrap_or(ControlWriterOutcome::Stopped)
    }

    /// Consume a writer failure once. Later calls return `None`.
    pub fn consume_control_writer_failure(
        &self,
        session_id: &SessionId,
    ) -> Option<ControlWriterError> {
        self.sessions
            .get(session_id)
            .and_then(|session| session.writer_slot.consume_failure())
    }

    /// Record a durable control-plane failure. Recovery is respawn only.
    pub fn mark_control_plane_failed(&mut self, session_id: &SessionId, error: ControlWriterError) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.control_plane = ControlPlaneState::Failed(error);
        }
    }

    /// Parent wait bound reused by pending ingress resize deadlines.
    #[must_use]
    pub fn mode_gated_input_timeout(&self) -> Duration {
        self.options.mode_gated_input_timeout
    }

    /// Correlated mode-gated PTY input against the worker atomic admit barrier.
    ///
    /// Interleaved PTY/metadata frames continue normal demux into pending
    /// output. Only a matching result completes the wait. Fail closed on
    /// timeout, disconnect, exit, malformed reply, and concurrent gated calls.
    pub fn mode_gated_pty_input(
        &mut self,
        session_id: &SessionId,
        expected: ModeFreshnessToken,
        data: Vec<u8>,
    ) -> Result<ModeGatedPtyInputResult, SessionRuntimeError> {
        let _request_id = self.submit_mode_gated_pty_input(session_id, expected, data)?;
        loop {
            match self.poll_mode_gated_pty_input(session_id)? {
                GatedPoll::Ready(result) => return Ok(result),
                GatedPoll::TimedOut => {
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        "mode-gated input timed out",
                    ));
                }
                GatedPoll::Idle => {
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::OutputFailed,
                        "mode-gated lane released before a result",
                    ));
                }
                GatedPoll::Pending => {}
            }
            if self.session_reader_finished(session_id)? {
                if let Ok(session) = self.session_mut(session_id) {
                    if let Ok(mut in_flight) = session.gated_in_flight.lock() {
                        *in_flight = None;
                    }
                }
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::OutputFailed,
                    "worker disconnected before mode-gated result",
                ));
            }
            if let Ok(session) = self.session_mut(session_id) {
                if let Some(child) = session.child.as_mut() {
                    if child.try_wait().ok().flatten().is_some() {
                        if let Ok(mut in_flight) = session.gated_in_flight.lock() {
                            *in_flight = None;
                        }
                        return Err(SessionRuntimeError::new(
                            SessionRuntimeErrorKind::OutputFailed,
                            "worker process exited before mode-gated result",
                        ));
                    }
                }
            }
            thread::sleep(GATED_POLL);
        }
    }

    fn pump_session_output(&mut self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        let session = self.session_mut(session_id)?;
        let mut drained = false;
        while let Ok(event) = session.output.try_recv() {
            drained = true;
            match event {
                WorkerChannelEvent::Output(output) => session.pending_output.push_back(output),
                WorkerChannelEvent::ModeFlags(payload) => {
                    *session.mode_flags_slot.lock().map_err(lock_error)? = Some(payload);
                }
                WorkerChannelEvent::ModeGatedResult(result) => {
                    let mut in_flight = session.gated_in_flight.lock().map_err(lock_error)?;
                    match in_flight.as_mut() {
                        Some(slot) if slot.request_id == result.request_id => {
                            slot.result = Some(Ok(result));
                        }
                        Some(_) => {
                            // Stale/mismatched request_id: ignore and keep waiting.
                        }
                        None => {
                            // No outstanding wait: drop stale result.
                        }
                    }
                }
                WorkerChannelEvent::Snapshot(result) => {
                    if session.outstanding_snapshot_request.as_ref() == Some(&result.request_id) {
                        let may_have_more = !result.barrier_released;
                        session
                            .snapshot_boundary
                            .push_back((result, session.pending_output.len()));
                        // The stdout reader can coalesce several snapshot wakes
                        // before this poll consumes the first frame. Rearm one
                        // session wake so the client-paced boundary can advance.
                        if may_have_more {
                            notify_session_wake(&session.wake_handle);
                        }
                        // Return one snapshot transport frame per parent poll.
                        // This preserves client-paced, bounded history delivery.
                        break;
                    }
                }
                WorkerChannelEvent::ResizeApplied(size) => {
                    session.applied_resizes.push_back(size);
                }
                WorkerChannelEvent::MalformedModeGated {
                    request_id,
                    message,
                } => {
                    let mut in_flight = session.gated_in_flight.lock().map_err(lock_error)?;
                    if let Some(slot) = in_flight.as_mut() {
                        if request_id.is_empty() || slot.request_id == request_id {
                            slot.result = Some(Err(SessionRuntimeError::new(
                                SessionRuntimeErrorKind::OutputFailed,
                                message,
                            )));
                        }
                    }
                }
            }
        }
        if drained {
            session.stall.note_space();
        }
        Ok(())
    }

    pub(crate) fn take_resize_applied(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<crate::ResizePayload>, SessionRuntimeError> {
        self.pump_session_output(session_id)?;
        Ok(self
            .session_mut(session_id)?
            .applied_resizes
            .drain(..)
            .collect())
    }

    fn session_reader_finished(
        &mut self,
        session_id: &SessionId,
    ) -> Result<bool, SessionRuntimeError> {
        let session = self.session_mut(session_id)?;
        let completion = session.completion.lock().map_err(lock_error)?;
        Ok(completion.reader_finished)
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
        supports_snapshot_boundary: bool,
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
        control
            .set_read_timeout(Some(self.options.mode_gated_input_timeout))
            .map_err(|error| {
                SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    format!("set adopted worker handshake timeout failed: {error}"),
                )
            })?;
        let (peer_version, metadata) = read_welcome(&mut control)
            .map_err(|error| runtime_error(SessionRuntimeErrorKind::SpawnFailed, error))?;
        if peer_version != PROTOCOL_VERSION {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("unsupported worker protocol version: {peer_version}"),
            ));
        }
        control.set_read_timeout(None).map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("clear adopted worker handshake timeout failed: {error}"),
            )
        })?;
        if metadata.session_uuid != session_id.0 {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "adopted worker welcome identified a different session",
            ));
        }
        let (sender, receiver) = mpsc::sync_channel(self.options.egress_capacity.max(1));
        let overflow = Arc::new(AtomicUsize::new(0));
        let pong_count = Arc::new(AtomicUsize::new(0));
        let last_health = Arc::new(Mutex::new(None));
        let completion = Arc::new(Mutex::new(WorkerCompletion::default()));
        let stall = Arc::new(EgressStall::new());
        let latest_mode = Arc::new(Mutex::new(None));
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
            Arc::clone(&stall),
            Arc::clone(&latest_mode),
            self.wake_source
                .as_ref()
                .map(|source| source.session_handle(session_id.clone())),
            session_id.clone(),
            self.options.test_resize_ack_hold.clone(),
        );
        let mut session = WorkerProcessSession {
            child: None,
            control: WorkerControl::Socket {
                stream: control,
                path: socket_path,
                identity,
            },
            control_queue: ControlQueue::new(),
            writer_slot: ControlWriterSlot::running(),
            control_plane: ControlPlaneState::Live,
            writer: None,
            wake_handle: self
                .wake_source
                .as_ref()
                .map(|source| source.session_handle(session_id.clone())),
            metadata,
            output: receiver,
            overflow,
            pong_count,
            last_health,
            completion,
            gated_in_flight: Arc::new(Mutex::new(None)),
            mode_flags_slot: Arc::new(Mutex::new(None)),
            latest_mode,
            outstanding_mode_probe: Arc::new(Mutex::new(None)),
            pending_output: std::collections::VecDeque::new(),
            applied_resizes: std::collections::VecDeque::new(),
            snapshot_boundary: std::collections::VecDeque::new(),
            outstanding_snapshot_request: None,
            supports_snapshot_boundary,
            egress_capacity: self.options.egress_capacity.max(1),
            stall,
        };
        self.start_writer_or_forget(&mut session, &session_id)?;
        self.sessions.insert(session_id.clone(), session);

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
            .arg("--ghostty-max-scrollback-bytes")
            .arg(self.options.ghostty_max_scrollback_bytes.to_string());
        if let Some(profile) = self.options.terminal_color_profile.as_ref() {
            command
                .arg("--terminal-color-profile")
                .arg(serde_json::to_string(profile).map_err(|error| {
                    SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        error.to_string(),
                    )
                })?);
        }
        command
            .arg("--shutdown-grace-ms")
            .arg(self.options.shutdown_grace_ms.to_string())
            .arg("--poll-interval-ms")
            .arg(self.options.poll_interval_ms.to_string())
            .stderr(Stdio::piped());
        if let Some(hold_ms) = self.options.test_hold_after_read_ms {
            command
                .arg("--test-hold-after-read-ms")
                .arg(hold_ms.to_string());
        }
        if let Some(until) = self.options.test_write_block_until_unix_ms {
            command
                .arg("--test-write-block-until-unix-ms")
                .arg(until.to_string());
        }
        if let Some(max_chunk) = self.options.test_write_max_chunk {
            command
                .arg("--test-write-max-chunk")
                .arg(max_chunk.to_string());
        }
        if let Some(capacity) = self.options.test_pending_capacity {
            command
                .arg("--test-pending-capacity")
                .arg(capacity.to_string());
        }
        if let Some(hold_ms) = self.options.test_hold_after_enqueue_ms {
            command
                .arg("--test-hold-after-enqueue-ms")
                .arg(hold_ms.to_string());
        }
        if self.options.test_fail_snapshot_history_after_ready {
            command.arg("--test-fail-snapshot-history-after-ready");
        }
        if self.options.test_omit_resize_applied {
            command.arg("--test-omit-resize-applied");
        }
        if let Some(hold_ms) = self.options.test_hold_before_exit_ms {
            command
                .arg("--test-hold-before-exit-ms")
                .arg(hold_ms.to_string());
        }
        if let Some(exit_code) = self.options.test_exit_code {
            command.arg("--test-exit-code").arg(exit_code.to_string());
        }

        #[cfg(unix)]
        let socket_path = self
            .options
            .control_socket_dir
            .as_ref()
            .map(|dir| worker_socket_path(dir, &request.session_id))
            .transpose()?;
        #[cfg(not(unix))]
        let socket_path: Option<PathBuf> = None;
        let socket_mode = socket_path.is_some();
        if let Some(path) = &socket_path {
            command
                .arg("--control-socket")
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped());
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
                    pending_worker.wait_for_socket_readiness()?;
                    let stream = connect_spawned_worker_socket(&path, &mut pending_worker)?;
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
                .map_err(|error| runtime_error(SessionRuntimeErrorKind::SpawnFailed, error))
        })()
        .map_err(|error: SessionRuntimeError| {
            SessionRuntimeError::new(SessionRuntimeErrorKind::SpawnFailed, error.message)
        });
        let metadata = match startup {
            Ok((peer_version, metadata)) => {
                if peer_version != PROTOCOL_VERSION {
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        format!("unsupported worker protocol version: {peer_version}"),
                    ));
                }
                let worker_pid = metadata
                    .recovery_identity
                    .as_ref()
                    .and_then(|identity| identity.get("worker_pid"))
                    .and_then(serde_json::Value::as_u64);
                if socket_mode && worker_pid != Some(u64::from(pending_worker.child_id())) {
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        "worker welcome did not identify the spawned child",
                    ));
                }
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
        let supports_snapshot_boundary =
            metadata.recovery_identity.as_ref().is_some_and(|identity| {
                identity
                    .get("atomic_snapshot_boundary")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    && identity
                        .get("snapshot_delivery")
                        .and_then(serde_json::Value::as_str)
                        == Some("ready_then_history")
            });

        let (sender, receiver) = mpsc::sync_channel(self.options.egress_capacity.max(1));
        let overflow = Arc::new(AtomicUsize::new(0));
        let pong_count = Arc::new(AtomicUsize::new(0));
        let last_health = Arc::new(Mutex::new(None));
        let completion = Arc::new(Mutex::new(WorkerCompletion::default()));
        let stall = Arc::new(EgressStall::new());
        let latest_mode = Arc::new(Mutex::new(None));
        spawn_stdout_reader(
            reader,
            sender,
            Arc::clone(&overflow),
            Arc::clone(&pong_count),
            Arc::clone(&last_health),
            Arc::clone(&completion),
            Arc::clone(&stall),
            Arc::clone(&latest_mode),
            self.wake_source
                .as_ref()
                .map(|source| source.session_handle(request.session_id.clone())),
            request.session_id.clone(),
            self.options.test_resize_ack_hold.clone(),
        );

        let mut session = WorkerProcessSession {
            child: Some(pending_worker.take()),
            control,
            control_queue: ControlQueue::new(),
            writer_slot: ControlWriterSlot::running(),
            control_plane: ControlPlaneState::Live,
            writer: None,
            wake_handle: self
                .wake_source
                .as_ref()
                .map(|source| source.session_handle(request.session_id.clone())),
            metadata,
            output: receiver,
            overflow,
            pong_count,
            last_health,
            completion,
            gated_in_flight: Arc::new(Mutex::new(None)),
            mode_flags_slot: Arc::new(Mutex::new(None)),
            latest_mode,
            outstanding_mode_probe: Arc::new(Mutex::new(None)),
            pending_output: std::collections::VecDeque::new(),
            applied_resizes: std::collections::VecDeque::new(),
            snapshot_boundary: std::collections::VecDeque::new(),
            outstanding_snapshot_request: None,
            supports_snapshot_boundary,
            egress_capacity: self.options.egress_capacity.max(1),
            stall,
        };
        self.start_writer_or_forget(&mut session, &request.session_id)?;
        self.sessions.insert(request.session_id.clone(), session);

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
                session.enqueue_frame(ControlFrameClass::Ordinary, FRAME_PTY_INPUT, &data)
            }
            SessionRuntimeInput::Resize { session_id, size } => {
                let session = self.session_mut(&session_id)?;
                session.enqueue_json(ControlFrameClass::Ordinary, FRAME_RESIZE, &size)
            }
            SessionRuntimeInput::Shutdown { session_id } => {
                let session = self.session_mut(&session_id)?;
                session.enqueue_frame(ControlFrameClass::Terminal, FRAME_SHUTDOWN, &[])
            }
        }
    }

    fn cancel_mode_gated_pty_input(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> Result<(), SessionRuntimeError> {
        WorkerProcessRuntime::enqueue_gated_cancel(self, session_id, request_id)
    }

    fn drain_output(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        // Demux interleaved gated/mode-flags frames before yielding output so
        // outstanding RPC waits and pending buffers stay ordered.
        self.pump_session_output(session_id)?;
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

            while let Some(event) = session.pending_output.pop_front() {
                output.push(event.into_runtime_output(session_id));
            }

            let completion = session.completion.lock().map_err(lock_error)?;
            completion.process_exited.clone()
        };

        if let Some(payload) = completed {
            // The reader stores FRAME_PROCESS_EXITED only after earlier frames
            // were accepted into the channel. Re-pump once so a raced last
            // PTY chunk is not dropped when the session is removed.
            self.pump_session_output(session_id)?;
            {
                let session = self.session_mut(session_id)?;
                while let Some(event) = session.pending_output.pop_front() {
                    output.push(event.into_runtime_output(session_id));
                }
            }
            // Map removal transfers wake-retirement ownership to CoreDaemon.
            if let Some(mut removed) = self.sessions.remove(session_id) {
                removed.close_before_blocking_shutdown();
                removed.shutdown_control();
                if let Some(mut child) = removed.child.take() {
                    match child.try_wait() {
                        Ok(Some(_)) => {}
                        Ok(None) | Err(_) => reap_worker_child_in_background(child),
                    }
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
        if let Some(source) = &self.wake_source {
            // Map membership means that the runtime still owns wake retirement.
            // Exit delivery removes the session and transfers ownership to CoreDaemon.
            for session_id in self.sessions.keys() {
                source.forget_session(session_id);
            }
        }
        for (_, mut session) in self.sessions.drain() {
            session.close_before_blocking_shutdown();
            if let Some(request_id) = session.outstanding_snapshot_request.take() {
                let _ = session.enqueue_json(
                    ControlFrameClass::Ordinary,
                    crate::FRAME_GET_SNAPSHOT,
                    &WorkerSnapshotRequest {
                        request_id,
                        cancel: true,
                        complete: false,
                    },
                );
            }
            session.shutdown_control();
            if let Some(mut child) = session.child.take() {
                let _ = child.kill();
                match child.try_wait() {
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => reap_worker_child_in_background(child),
                }
            }
            session.control.cleanup();
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ConsumerKey {
    Direct,
    Named {
        client: String,
        subscription: String,
    },
}

enum StallWait {
    Retry,
    Detached,
    Closed,
}

struct EgressStallState {
    owners: HashSet<ConsumerKey>,
    drain_seq: u64,
    closed: bool,
}

/// Detach-aware wait for attached live PTY stall.
///
/// Wakes when the parent drains a slot, the live owner set becomes empty, or
/// the session is dropped. A blocking `SyncSender::send` cannot observe detach.
struct EgressStall {
    state: Mutex<EgressStallState>,
    wait: Condvar,
}

impl EgressStall {
    fn new() -> Self {
        Self {
            state: Mutex::new(EgressStallState {
                owners: HashSet::new(),
                drain_seq: 0,
                closed: false,
            }),
            wait: Condvar::new(),
        }
    }

    fn insert_direct(&self) -> Result<(), SessionRuntimeError> {
        let mut state = self.state.lock().map_err(lock_error)?;
        state.owners.insert(ConsumerKey::Direct);
        self.wait.notify_all();
        Ok(())
    }

    fn remove_direct(&self) -> Result<(), SessionRuntimeError> {
        let mut state = self.state.lock().map_err(lock_error)?;
        state.owners.remove(&ConsumerKey::Direct);
        self.wait.notify_all();
        Ok(())
    }

    fn replace_named(
        &self,
        owners: impl IntoIterator<Item = (ClientId, SubscriptionId)>,
    ) -> Result<(), SessionRuntimeError> {
        let mut state = self.state.lock().map_err(lock_error)?;
        state
            .owners
            .retain(|key| matches!(key, ConsumerKey::Direct));
        for (client_id, subscription_id) in owners {
            state.owners.insert(ConsumerKey::Named {
                client: client_id.0,
                subscription: subscription_id.0,
            });
        }
        self.wait.notify_all();
        Ok(())
    }

    fn note_space(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.drain_seq = state.drain_seq.wrapping_add(1);
            self.wait.notify_all();
        }
    }

    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            self.wait.notify_all();
        }
    }

    fn owners_present(&self) -> bool {
        self.state
            .lock()
            .map(|state| !state.closed && !state.owners.is_empty())
            .unwrap_or(false)
    }

    fn wait_for_space_or_detach(&self, seen_seq: &mut u64) -> StallWait {
        let Ok(state) = self.state.lock() else {
            return StallWait::Closed;
        };
        if state.closed {
            return StallWait::Closed;
        }
        if state.owners.is_empty() {
            return StallWait::Detached;
        }
        if state.drain_seq != *seen_seq {
            *seen_seq = state.drain_seq;
            return StallWait::Retry;
        }
        let Ok(state) = self.wait.wait(state) else {
            return StallWait::Closed;
        };
        *seen_seq = state.drain_seq;
        if state.closed {
            StallWait::Closed
        } else if state.owners.is_empty() {
            StallWait::Detached
        } else {
            StallWait::Retry
        }
    }
}

struct WorkerProcessSession {
    child: Option<Child>,
    control: WorkerControl,
    control_queue: ControlQueue,
    writer_slot: ControlWriterSlot,
    control_plane: ControlPlaneState,
    writer: Option<thread::JoinHandle<()>>,
    wake_handle: Option<SessionWakeHandle>,
    metadata: SessionMetadata,
    output: Receiver<WorkerChannelEvent>,
    overflow: Arc<AtomicUsize>,
    pong_count: Arc<AtomicUsize>,
    last_health: Arc<Mutex<Option<WorkerHealth>>>,
    completion: Arc<Mutex<WorkerCompletion>>,
    gated_in_flight: Arc<Mutex<Option<GatedInFlight>>>,
    mode_flags_slot: Arc<Mutex<Option<ModeFlagsPayload>>>,
    latest_mode: Arc<Mutex<Option<(ModeFreshnessToken, ModeFlags)>>>,
    outstanding_mode_probe: Arc<Mutex<Option<String>>>,
    pending_output: std::collections::VecDeque<WorkerOutputEvent>,
    applied_resizes: std::collections::VecDeque<crate::ResizePayload>,
    snapshot_boundary: std::collections::VecDeque<(WorkerSnapshotResult, usize)>,
    outstanding_snapshot_request: Option<String>,
    supports_snapshot_boundary: bool,
    egress_capacity: usize,
    stall: Arc<EgressStall>,
}

impl WorkerProcessSession {
    /// Wake stall waiters before child.wait() or other blocking close work.
    ///
    /// Attached capacity-one pressure can fill the worker pipe while the
    /// parent stdout reader waits on the condvar. Closing after wait deadlocks
    /// shutdown.
    fn close_before_blocking_shutdown(&self) {
        self.stall.close();
    }

    fn start_writer(&mut self) -> Result<(), SessionRuntimeError> {
        let write = self.control.take_write_half()?;
        let queue = self.control_queue.clone();
        let slot = self.writer_slot.clone();
        let wake_handle = self.wake_handle.clone();
        self.writer = Some(thread::spawn(move || {
            run_control_writer(queue, write, slot, wake_handle);
        }));
        Ok(())
    }

    fn enqueue_frame(
        &self,
        class: ControlFrameClass,
        frame_type: u8,
        payload: &[u8],
    ) -> Result<(), SessionRuntimeError> {
        let frame = crate::encode_frame(frame_type, payload)
            .map_err(|error| runtime_error(SessionRuntimeErrorKind::InputFailed, error))?;
        self.admit_encoded(class, frame)
    }

    fn enqueue_json<T: Serialize>(
        &self,
        class: ControlFrameClass,
        frame_type: u8,
        payload: &T,
    ) -> Result<(), SessionRuntimeError> {
        let frame = crate::encode_json(frame_type, payload)
            .map_err(|error| runtime_error(SessionRuntimeErrorKind::InputFailed, error))?;
        self.admit_encoded(class, frame)
    }

    fn admit_encoded(
        &self,
        class: ControlFrameClass,
        frame: Vec<u8>,
    ) -> Result<(), SessionRuntimeError> {
        self.control_queue.admit(class, frame).map_err(|error| {
            let message = match error {
                ControlQueueAdmitError::ControlQueueFull => "control queue full",
                ControlQueueAdmitError::Sealed => "control plane sealed",
            };
            SessionRuntimeError::new(SessionRuntimeErrorKind::InputFailed, message)
        })
    }

    fn shutdown_control(&mut self) {
        let _ = self.enqueue_frame(ControlFrameClass::Terminal, FRAME_SHUTDOWN, &[]);
        self.control.hard_stop_write(self.child.as_mut());
        self.join_writer();
    }

    fn join_writer(&mut self) {
        let deadline = Instant::now() + WORKER_CONTROL_WRITER_JOIN_BOUND;
        while Instant::now() < deadline {
            if !matches!(self.writer_slot.get(), ControlWriterOutcome::Running) {
                if let Some(handle) = self.writer.take() {
                    let _ = handle.join();
                }
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        self.writer.take();
    }
}

impl Drop for WorkerProcessSession {
    fn drop(&mut self) {
        self.stall.close();
    }
}

struct GatedInFlight {
    request_id: String,
    result: Option<Result<ModeGatedPtyInputResult, SessionRuntimeError>>,
    cancelled: bool,
    parent_deadline: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModeFlagsProbeRequest {
    request_id: String,
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
    ReleasedStdio,
    #[cfg(unix)]
    ReleasedSocket {
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
            Self::ReleasedStdio => Err(released_control_error()),
            #[cfg(unix)]
            Self::ReleasedSocket { .. } => Err(released_control_error()),
        }
    }

    fn write_frame(&mut self, frame_type: u8, payload: &[u8]) -> Result<(), SessionRuntimeError> {
        match self {
            Self::Stdio(stdin) => write_frame(stdin, frame_type, payload),
            #[cfg(unix)]
            Self::Socket { stream, .. } => write_frame(stream, frame_type, payload),
            Self::ReleasedStdio => Err(released_control_error()),
            #[cfg(unix)]
            Self::ReleasedSocket { .. } => Err(released_control_error()),
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
            Self::ReleasedStdio => Err(released_control_error()),
            #[cfg(unix)]
            Self::ReleasedSocket { .. } => Err(released_control_error()),
        }
    }

    fn take_write_half(&mut self) -> Result<WorkerWriteHalf, SessionRuntimeError> {
        match std::mem::replace(self, Self::ReleasedStdio) {
            Self::Stdio(stdin) => Ok(WorkerWriteHalf::Stdio(stdin)),
            #[cfg(unix)]
            Self::Socket {
                stream,
                path,
                identity,
            } => {
                let shutdown = stream.try_clone().map_err(|error| {
                    SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        format!("clone worker control socket for shutdown failed: {error}"),
                    )
                })?;
                *self = Self::ReleasedSocket {
                    stream: shutdown,
                    path,
                    identity,
                };
                Ok(WorkerWriteHalf::Socket(stream))
            }
            Self::ReleasedStdio => Err(released_control_error()),
            #[cfg(unix)]
            Self::ReleasedSocket {
                stream,
                path,
                identity,
            } => {
                *self = Self::ReleasedSocket {
                    stream,
                    path,
                    identity,
                };
                Err(released_control_error())
            }
        }
    }

    fn hard_stop_write(&self, child: Option<&mut Child>) {
        match self {
            Self::ReleasedStdio | Self::Stdio(_) => {
                if let Some(child) = child {
                    let _ = child.kill();
                }
            }
            #[cfg(unix)]
            Self::ReleasedSocket { stream, .. } | Self::Socket { stream, .. } => {
                let _ = stream.shutdown(Shutdown::Write);
            }
        }
    }

    fn cleanup(&self) {
        #[cfg(unix)]
        match self {
            Self::Socket {
                path,
                identity: Some(identity),
                ..
            }
            | Self::ReleasedSocket {
                path,
                identity: Some(identity),
                ..
            } => {
                let _ = remove_socket_if_unchanged(path, identity);
            }
            _ => {}
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
    pending_worker: &mut PendingWorker,
) -> Result<UnixStream, SessionRuntimeError> {
    let deadline = Instant::now() + WORKER_STARTUP_TIMEOUT;
    loop {
        if let Some(diagnostic) = pending_worker.exited_diagnostic() {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("connect worker control socket failed: {diagnostic}"),
            ));
        }
        let error = match UnixStream::connect(path) {
            Ok(stream) => return Ok(stream),
            Err(error) => error,
        };
        if Instant::now() >= deadline {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("connect worker control socket failed: {error}"),
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
    ctime: i64,
    ctime_nsec: i64,
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
        ctime: metadata.ctime(),
        ctime_nsec: metadata.ctime_nsec(),
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

    fn child_id(&self) -> u32 {
        self.child.as_ref().expect("pending worker child").id()
    }

    #[cfg(unix)]
    fn wait_for_socket_readiness(&mut self) -> Result<(), SessionRuntimeError> {
        let stdout = self.child_mut().stdout.take().ok_or_else(|| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "worker readiness stdout missing",
            )
        })?;
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let _ = sender.send(read_worker_readiness(stdout));
        });
        let deadline = Instant::now() + WORKER_STARTUP_TIMEOUT;
        loop {
            match receiver.recv_timeout(Duration::from_millis(10)) {
                Ok(Ok(readiness)) => {
                    let expected = format!("botster-session-worker-ready {}", self.child_id());
                    if readiness == expected {
                        return Ok(());
                    }
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        format!("worker readiness identity mismatch: {readiness:?}"),
                    ));
                }
                Ok(Err(error)) => loop {
                    if let Some(diagnostic) = self.exited_diagnostic() {
                        return Err(SessionRuntimeError::new(
                            SessionRuntimeErrorKind::SpawnFailed,
                            format!("connect worker control socket failed: {diagnostic}"),
                        ));
                    }
                    if Instant::now() >= deadline {
                        return Err(SessionRuntimeError::new(
                                SessionRuntimeErrorKind::SpawnFailed,
                                format!(
                                    "connect worker control socket failed: read worker readiness failed: {error}"
                                ),
                            ));
                    }
                    thread::sleep(Duration::from_millis(10));
                },
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(SessionRuntimeError::new(
                        SessionRuntimeErrorKind::SpawnFailed,
                        "worker readiness channel disconnected",
                    ));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if let Some(diagnostic) = self.exited_diagnostic() {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    format!("connect worker control socket failed: {diagnostic}"),
                ));
            }
            if Instant::now() >= deadline {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    "connect worker control socket failed: worker readiness timed out",
                ));
            }
        }
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

#[cfg(unix)]
fn read_worker_readiness(mut stdout: ChildStdout) -> std::io::Result<String> {
    const MAX_READINESS_BYTES: usize = 128;
    let mut bytes = Vec::with_capacity(MAX_READINESS_BYTES);
    while bytes.len() < MAX_READINESS_BYTES {
        let mut byte = [0_u8; 1];
        match stdout.read(&mut byte)? {
            0 => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "worker readiness closed before newline",
                ))
            }
            _ if byte[0] == b'\n' => {
                return String::from_utf8(bytes)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
            }
            _ => bytes.push(byte[0]),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "worker readiness exceeded maximum length",
    ))
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

enum WorkerChannelEvent {
    Output(WorkerOutputEvent),
    ModeFlags(ModeFlagsPayload),
    ModeGatedResult(ModeGatedPtyInputResult),
    ResizeApplied(crate::ResizePayload),
    Snapshot(WorkerSnapshotResult),
    MalformedModeGated { request_id: String, message: String },
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

fn reap_worker_child_in_background(mut child: Child) {
    thread::spawn(move || {
        let deadline = Instant::now() + WORKER_REAP_GRACE;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    thread::sleep(WORKER_REAP_POLL);
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    });
}

fn notify_session_wake(handle: &Option<SessionWakeHandle>) {
    if let Some(handle) = handle {
        handle.notify();
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_stdout_reader(
    mut stdout: impl Read + Send + 'static,
    sender: SyncSender<WorkerChannelEvent>,
    overflow: Arc<AtomicUsize>,
    pong_count: Arc<AtomicUsize>,
    last_health: Arc<Mutex<Option<WorkerHealth>>>,
    completion: Arc<Mutex<WorkerCompletion>>,
    stall: Arc<EgressStall>,
    latest_mode: Arc<Mutex<Option<(ModeFreshnessToken, ModeFlags)>>>,
    wake_handle: Option<SessionWakeHandle>,
    session_id: crate::SessionId,
    resize_ack_hold: Option<ResizeAckHold>,
) {
    thread::spawn(move || {
        while let Ok(frame) = read_frame(&mut stdout) {
            match frame.frame_type {
                FRAME_PTY_OUTPUT => send_worker_event(
                    &sender,
                    &overflow,
                    &stall,
                    &wake_handle,
                    WorkerChannelEvent::Output(WorkerOutputEvent::PtyOutput(frame.payload)),
                ),
                FRAME_PROCESS_EXITED => {
                    if let Ok(payload) = serde_json::from_slice(&frame.payload) {
                        if let Ok(mut state) = completion.lock() {
                            state.process_exited = Some(payload);
                        }
                    }
                    notify_session_wake(&wake_handle);
                }
                FRAME_TITLE_CHANGED => {
                    if let Ok(title) = String::from_utf8(frame.payload) {
                        send_worker_event(
                            &sender,
                            &overflow,
                            &stall,
                            &wake_handle,
                            WorkerChannelEvent::Output(WorkerOutputEvent::TitleChanged(title)),
                        );
                    }
                }
                FRAME_CWD_CHANGED => {
                    if let Ok(cwd) = String::from_utf8(frame.payload) {
                        send_worker_event(
                            &sender,
                            &overflow,
                            &stall,
                            &wake_handle,
                            WorkerChannelEvent::Output(WorkerOutputEvent::CwdChanged(cwd)),
                        );
                    }
                }
                FRAME_PROMPT_MARK => {
                    if let Ok(payload) = serde_json::from_slice(&frame.payload) {
                        send_worker_event(
                            &sender,
                            &overflow,
                            &stall,
                            &wake_handle,
                            WorkerChannelEvent::Output(WorkerOutputEvent::PromptMark(payload)),
                        );
                    }
                }
                FRAME_BELL => {
                    send_worker_event(
                        &sender,
                        &overflow,
                        &stall,
                        &wake_handle,
                        WorkerChannelEvent::Output(WorkerOutputEvent::Bell),
                    );
                }
                FRAME_NOTIFICATION => {
                    if let Ok(payload) = serde_json::from_slice(&frame.payload) {
                        send_worker_event(
                            &sender,
                            &overflow,
                            &stall,
                            &wake_handle,
                            WorkerChannelEvent::Output(WorkerOutputEvent::Notification(payload)),
                        );
                    }
                }
                FRAME_METADATA_SHAPING => {
                    if let Ok(observation) = serde_json::from_slice(&frame.payload) {
                        send_worker_event(
                            &sender,
                            &overflow,
                            &stall,
                            &wake_handle,
                            WorkerChannelEvent::Output(WorkerOutputEvent::MetadataShaping(
                                observation,
                            )),
                        );
                    }
                }
                FRAME_MODE_FLAGS => {
                    match serde_json::from_slice::<ModeFlagsPayload>(&frame.payload) {
                        Ok(payload) => {
                            if payload.error_kind.is_none() {
                                if let Ok(mut latest) = latest_mode.lock() {
                                    *latest =
                                        Some((payload.mode_freshness, payload.mode_flags.clone()));
                                }
                            }
                            send_worker_event(
                                &sender,
                                &overflow,
                                &stall,
                                &wake_handle,
                                WorkerChannelEvent::ModeFlags(payload),
                            );
                        }
                        Err(error) => {
                            let _ = error;
                            // Malformed probe replies fail closed on the waiter timeout path.
                        }
                    }
                }
                FRAME_MODE_GATED_PTY_INPUT_RESULT => {
                    match serde_json::from_slice::<ModeGatedPtyInputResult>(&frame.payload) {
                        Ok(result) => {
                            if let Ok(mut latest) = latest_mode.lock() {
                                *latest = Some((result.mode_freshness, result.mode_flags.clone()));
                            }
                            send_worker_event(
                                &sender,
                                &overflow,
                                &stall,
                                &wake_handle,
                                WorkerChannelEvent::ModeGatedResult(result),
                            )
                        }
                        Err(error) => send_worker_event(
                            &sender,
                            &overflow,
                            &stall,
                            &wake_handle,
                            WorkerChannelEvent::MalformedModeGated {
                                request_id: String::new(),
                                message: format!("malformed mode-gated result: {error}"),
                            },
                        ),
                    }
                }
                FRAME_RESIZE_APPLIED => {
                    if let Ok(size) = serde_json::from_slice(&frame.payload) {
                        if let Some(hold) = &resize_ack_hold {
                            hold.wait_if_session(&session_id);
                        }
                        if sender
                            .send(WorkerChannelEvent::ResizeApplied(size))
                            .is_err()
                        {
                            break;
                        }
                        notify_session_wake(&wake_handle);
                    }
                }
                FRAME_SNAPSHOT => {
                    if let Ok(result) =
                        serde_json::from_slice::<WorkerSnapshotResult>(&frame.payload)
                    {
                        if sender.send(WorkerChannelEvent::Snapshot(result)).is_err() {
                            break;
                        }
                        notify_session_wake(&wake_handle);
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
        notify_session_wake(&wake_handle);
    });
}

fn send_worker_event(
    sender: &SyncSender<WorkerChannelEvent>,
    overflow: &AtomicUsize,
    stall: &EgressStall,
    wake_handle: &Option<SessionWakeHandle>,
    event: WorkerChannelEvent,
) {
    // Live PTY bytes are not replayable. While a parent consumer is attached,
    // stall instead of dropping them. Detach must stop the stall so cancel and
    // detached workers can still make progress. Wait on the drain/detach
    // condvar; a blocking send cannot observe detach.
    if matches!(
        event,
        WorkerChannelEvent::Output(WorkerOutputEvent::PtyOutput(_))
    ) {
        let mut event = event;
        let mut seen_seq = 0;
        loop {
            if !stall.owners_present() {
                match sender.try_send(event) {
                    Ok(()) => {
                        notify_session_wake(wake_handle);
                        return;
                    }
                    Err(TrySendError::Full(_)) => {
                        overflow.fetch_add(1, Ordering::AcqRel);
                        return;
                    }
                    Err(TrySendError::Disconnected(_)) => return,
                }
            }
            match sender.try_send(event) {
                Ok(()) => {
                    notify_session_wake(wake_handle);
                    return;
                }
                Err(TrySendError::Full(returned)) => {
                    event = returned;
                    match stall.wait_for_space_or_detach(&mut seen_seq) {
                        StallWait::Retry => {}
                        StallWait::Detached => match sender.try_send(event) {
                            Ok(()) => {
                                notify_session_wake(wake_handle);
                                return;
                            }
                            Err(TrySendError::Full(_)) => {
                                overflow.fetch_add(1, Ordering::AcqRel);
                                return;
                            }
                            Err(TrySendError::Disconnected(_)) => return,
                        },
                        StallWait::Closed => return,
                    }
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
    }
    match sender.try_send(event) {
        Ok(()) => notify_session_wake(wake_handle),
        Err(TrySendError::Full(_)) => {
            overflow.fetch_add(1, Ordering::AcqRel);
        }
        Err(TrySendError::Disconnected(_)) => {}
    }
}

fn next_gated_request_id() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let ordinal = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("mode-gated-{}-{}", nanos, ordinal)
}

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
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

enum WorkerWriteHalf {
    Stdio(ChildStdin),
    #[cfg(unix)]
    Socket(UnixStream),
}

impl Write for WorkerWriteHalf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Stdio(stdin) => stdin.write(buf),
            #[cfg(unix)]
            Self::Socket(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Stdio(stdin) => stdin.flush(),
            #[cfg(unix)]
            Self::Socket(stream) => stream.flush(),
        }
    }
}

impl WorkerWriteHalf {
    fn prepare(&mut self) -> io::Result<()> {
        match self {
            Self::Stdio(stdin) => set_fd_nonblocking(stdin.as_raw_fd()),
            #[cfg(unix)]
            Self::Socket(_) => Ok(()),
        }
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Stdio(_) => Ok(()),
            #[cfg(unix)]
            Self::Socket(stream) => stream.set_write_timeout(timeout),
        }
    }
}

fn set_fd_nonblocking(fd: std::os::unix::io::RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn run_control_writer(
    queue: ControlQueue,
    mut write: WorkerWriteHalf,
    slot: ControlWriterSlot,
    wake_handle: Option<SessionWakeHandle>,
) {
    if let Err(error) = write.prepare() {
        queue.seal();
        slot.set(ControlWriterOutcome::Failed {
            error: ControlWriterError::WriteError(error.to_string()),
            consumed: false,
        });
        notify_session_wake(&wake_handle);
        return;
    }
    loop {
        let Some((class, frame, freed_ordinary_capacity)) = queue.pop_with_capacity_transition()
        else {
            slot.set(ControlWriterOutcome::Stopped);
            return;
        };
        if freed_ordinary_capacity {
            notify_session_wake(&wake_handle);
        }
        match write_control_bytes(&mut write, &frame) {
            Ok(()) => {
                if class == ControlFrameClass::Terminal {
                    slot.set(ControlWriterOutcome::Stopped);
                    return;
                }
            }
            Err(error) => {
                queue.seal();
                slot.set(ControlWriterOutcome::Failed {
                    error,
                    consumed: false,
                });
                // Terminal-class shutdown is teardown, not session ingress.
                if class != ControlFrameClass::Terminal {
                    notify_session_wake(&wake_handle);
                }
                return;
            }
        }
    }
}

fn write_control_bytes(
    write: &mut WorkerWriteHalf,
    bytes: &[u8],
) -> Result<(), ControlWriterError> {
    let deadline = Instant::now() + WORKER_CONTROL_WRITE_TIMEOUT;
    let mut written = 0;
    while written < bytes.len() {
        let now = Instant::now();
        let Some(slice) = write_slice_timeout(deadline, now) else {
            return Err(ControlWriterError::DeadlineExpired);
        };
        write
            .set_write_timeout(Some(slice))
            .map_err(|error| ControlWriterError::WriteError(error.to_string()))?;
        match write.write(&bytes[written..]) {
            Ok(0) => return Err(ControlWriterError::PeerClosed),
            Ok(count) => written += count,
            Err(error)
                if error.kind() == ErrorKind::WouldBlock
                    || error.kind() == ErrorKind::TimedOut
                    || error.kind() == ErrorKind::Interrupted =>
            {
                thread::sleep(slice.min(Duration::from_millis(5)));
            }
            Err(error)
                if error.kind() == ErrorKind::BrokenPipe
                    || error.kind() == ErrorKind::ConnectionReset
                    || error.kind() == ErrorKind::UnexpectedEof =>
            {
                return Err(ControlWriterError::PeerClosed);
            }
            Err(error) => return Err(ControlWriterError::WriteError(error.to_string())),
        }
    }
    if write_slice_timeout(deadline, Instant::now()).is_none() {
        return Err(ControlWriterError::DeadlineExpired);
    }
    match write.flush() {
        Ok(()) => Ok(()),
        Err(error)
            if error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::TimedOut =>
        {
            Ok(())
        }
        Err(error)
            if error.kind() == ErrorKind::BrokenPipe
                || error.kind() == ErrorKind::ConnectionReset =>
        {
            Err(ControlWriterError::PeerClosed)
        }
        Err(error) => Err(ControlWriterError::WriteError(error.to_string())),
    }
}

fn released_control_error() -> SessionRuntimeError {
    SessionRuntimeError::new(
        SessionRuntimeErrorKind::InputFailed,
        "control write half moved to writer thread",
    )
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
    use std::io::Write;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use crate::contract::terminal_wake::TerminalWakeSource;
    use crate::runtime::control_queue::{ControlFrameClass, ControlQueue, ControlWriterSlot};

    use super::{
        remove_socket_if_unchanged, run_control_writer, socket_identity, worker_socket_path,
        ProcessIdentity, SessionId, SessionRuntimeErrorKind, WorkerProcessRuntime, WorkerWriteHalf,
        FRAME_PTY_INPUT, FRAME_SHUTDOWN, UNIX_SOCKET_PATH_MAX_BYTES,
    };

    fn closed_peer_write_half() -> WorkerWriteHalf {
        let (writer, peer) = UnixStream::pair().expect("socket pair");
        drop(peer);
        WorkerWriteHalf::Socket(writer)
    }

    fn wake_after_control_write_failure(class: ControlFrameClass, frame_type: u8) -> bool {
        let source = TerminalWakeSource::new();
        let session = SessionId("control-writer-wake".to_string());
        let handle = source.session_handle(session.clone());
        let queue = ControlQueue::new();
        let frame = crate::encode_frame(frame_type, b"x").expect("control frame");
        queue.admit(class, frame).expect("admit");
        run_control_writer(
            queue,
            closed_peer_write_half(),
            ControlWriterSlot::running(),
            Some(handle),
        );
        let batch = source.wait_wakes(Duration::from_millis(0));
        batch.ingress_sessions.iter().any(|id| id == &session)
    }

    #[test]
    fn ordinary_control_write_failure_notifies_session() {
        assert!(wake_after_control_write_failure(
            ControlFrameClass::Ordinary,
            FRAME_PTY_INPUT
        ));
    }

    #[test]
    fn terminal_shutdown_write_failure_does_not_notify_session() {
        assert!(!wake_after_control_write_failure(
            ControlFrameClass::Terminal,
            FRAME_SHUTDOWN
        ));
    }

    #[test]
    fn cleanup_identity_includes_socket_lifetime_metadata() {
        let path = Path::new("/tmp").join(format!(
            "bri-{}-{}.sock",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let listener = UnixListener::bind(&path).expect("bind socket");
        let mut earlier_lifetime = socket_identity(&path).expect("socket identity");
        earlier_lifetime.ctime_nsec ^= 1;

        assert!(!remove_socket_if_unchanged(&path, &earlier_lifetime)
            .expect("mismatched lifetime must be preserved"));
        assert!(path.exists());

        drop(listener);
        let _ = std::fs::remove_file(path);
    }

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
                false,
            )
            .expect_err("missing adopted endpoint must fail");

        assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
        assert!(error
            .message
            .starts_with("connect worker control socket failed: "));
    }

    #[test]
    fn adoption_starts_without_a_live_mode_record() {
        let path = Path::new("/tmp").join(format!(
            "botster-mode-adoption-{}-{}.sock",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let listener = UnixListener::bind(&path).expect("bind worker socket");
        let (close_sender, close_receiver) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept parent");
            crate::read_hello(&mut stream).expect("read parent hello");
            let metadata = crate::SessionMetadata {
                session_uuid: "mode-adoption".to_string(),
                pid: std::process::id(),
                rows: 24,
                cols: 80,
                last_output_at: 0,
                title: None,
                cwd: None,
                port: None,
                mode_flags: Default::default(),
                recovery_identity: None,
            };
            let bytes = crate::encode_welcome(crate::PROTOCOL_VERSION, &metadata)
                .expect("encode worker welcome");
            stream.write_all(&bytes).expect("write worker welcome");
            close_receiver.recv().expect("receive close signal");
        });

        let session_id = SessionId("mode-adoption".to_string());
        let mut runtime = WorkerProcessRuntime::new("/missing/worker");
        runtime
            .adopt_session(
                session_id.clone(),
                ProcessIdentity {
                    pid: Some(std::process::id()),
                    runtime_id: Some("mode-adoption".to_string()),
                },
                &path,
                false,
            )
            .expect("adopt current worker protocol");

        assert_eq!(
            runtime.latest_mode_for(&session_id, crate::ModeFreshnessToken::default()),
            None,
            "adoption must not trust mode flags from the welcome metadata"
        );

        runtime.release_for_restart();
        close_sender.send(()).expect("close worker server");
        server.join().expect("worker server");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn adoption_rejects_a_worker_from_the_previous_protocol() {
        let path = Path::new("/tmp").join(format!(
            "botster-old-worker-{}-{}.sock",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let listener = UnixListener::bind(&path).expect("bind old worker socket");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept parent");
            crate::read_hello(&mut stream).expect("read parent hello");
            let metadata = crate::SessionMetadata {
                session_uuid: "old-worker-adoption".to_string(),
                pid: std::process::id(),
                rows: 24,
                cols: 80,
                last_output_at: 0,
                title: None,
                cwd: None,
                port: None,
                mode_flags: Default::default(),
                recovery_identity: None,
            };
            let bytes = crate::encode_welcome(crate::PROTOCOL_VERSION - 1, &metadata)
                .expect("encode old welcome");
            stream.write_all(&bytes).expect("write old welcome");
        });

        let mut runtime = WorkerProcessRuntime::new("/missing/worker");
        let error = runtime
            .adopt_session(
                SessionId("old-worker-adoption".to_string()),
                ProcessIdentity {
                    pid: Some(std::process::id()),
                    runtime_id: Some("old-worker-adoption".to_string()),
                },
                &path,
                false,
            )
            .expect_err("old worker protocol must fail adoption");
        server.join().expect("old worker server");
        let _ = std::fs::remove_file(path);

        assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
        assert_eq!(error.message, "unsupported worker protocol version: 2");
    }
}
