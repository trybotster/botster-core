//! Local session runtime backed by a separate worker process.
//!
//! The worker owns the only Ghostty parser for its session. This adapter
//! forwards binary control frames, demultiplexes worker egress into typed
//! per-session queues, and never blocks the engine on a worker reply except
//! for the diagnostic ping RPC.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, ErrorKind, Read, Write};
use std::path::PathBuf;
#[cfg(unix)]
use std::process::ChildStdout;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::{
    ReservedSessionSpawnError, SessionAdmission, SessionReservation, SessionReservationState,
};

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
use botster_terminal_protocol::{
    decode_input_result, decode_modes, InputResultBody, ModesBody, TerminalFrame,
};
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
    decode_final_state, encode_worker_input_operation, read_startup_reply, read_welcome,
    split_worker_operation_key, write_hello, BackpressureRoute, BackpressureSummary, ClientId,
    Frame, ModeFlagsPayload, NotificationPayload, ProcessExitedPayload, ProcessIdentity,
    PromptMarkPayload, QueueSource, ScreenPayload, SessionId, SessionMetadata, SessionRuntime,
    SessionRuntimeError, SessionRuntimeErrorKind, SessionRuntimeHandle, SessionRuntimeInput,
    SessionRuntimeOutput, SessionSpawnRequest, StartupFailureOutcome, StartupFailureReport,
    StartupReply, SubscriptionId, TerminalMetadataShapingObservation, TimeoutPayload,
    WorkerFinalState, WorkerInputKind, WorkerProbeRequest, WorkerSnapshotRequest,
    WorkerSnapshotResult, FRAME_BELL, FRAME_CWD_CHANGED, FRAME_FINAL_STATE, FRAME_GET_MODE_FLAGS,
    FRAME_GET_SCREEN, FRAME_INPUT_CANCEL, FRAME_INPUT_OPERATION, FRAME_INPUT_RESULT,
    FRAME_METADATA_SHAPING, FRAME_MODES_CHANGED, FRAME_MODE_FLAGS, FRAME_NOTIFICATION, FRAME_PING,
    FRAME_PONG, FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK, FRAME_PTY_INPUT, FRAME_PTY_OUTPUT,
    FRAME_RESIZE, FRAME_RESIZE_APPLIED, FRAME_SCREEN, FRAME_SET_TIMEOUT, FRAME_SHUTDOWN,
    FRAME_SNAPSHOT, FRAME_SPAWN_SESSION, FRAME_TITLE_CHANGED, PROTOCOL_VERSION,
};

/// Default retained worker egress frames per session in the parent process.
pub const DEFAULT_WORKER_EGRESS_CAPACITY: usize = 64;

/// Default bound for a correlated worker reply: handshake reads, resize
/// acknowledgements, and Core pending readbacks.
pub const DEFAULT_WORKER_REPLY_TIMEOUT: Duration = Duration::from_secs(5);

const PING_WAIT: Duration = Duration::from_secs(2);
const PING_POLL: Duration = Duration::from_millis(10);
const WORKER_REAP_GRACE: Duration = Duration::from_secs(2);
const WORKER_REAP_POLL: Duration = Duration::from_millis(10);
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

/// Test-only probe of two parent-side decisions a test cannot otherwise
/// observe: the capture barrier release, and how the parent reader routed
/// each worker PTY output event. Events come from the decision sites
/// themselves. Unconfigured, each site costs one `Option` check.
#[derive(Clone)]
pub struct WorkerRouteProbe {
    sender: mpsc::Sender<WorkerRouteProbeEvent>,
    identity: Arc<()>,
}

/// One decision reported by a [`WorkerRouteProbe`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerRouteProbeEvent {
    /// The parent queued the release of a capture's worker barrier.
    CaptureReleaseSent {
        /// Session whose capture was released.
        session_id: crate::SessionId,
    },
    /// The parent reader read the worker's `FRAME_PROCESS_EXITED` and stored
    /// it for the next drain.
    ProcessExitRead {
        /// Session whose child exited.
        session_id: crate::SessionId,
    },
    /// The parent reader routed one worker PTY output event.
    PtyOutputRouted {
        /// Session the output belongs to.
        session_id: crate::SessionId,
        /// The routing decision.
        routing: PtyOutputRouting,
    },
}

/// How the parent reader routed one worker PTY output event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtyOutputRouting {
    /// Queued for the engine.
    Enqueued,
    /// The queue was full with a consumer attached: the reader waits for
    /// space. `Enqueued` follows when the output is queued.
    Stalled,
    /// The queue was full with no consumer: the output was dropped and
    /// counted as overflow.
    Dropped,
}

impl WorkerRouteProbe {
    /// Create a probe and the receiver its events arrive on.
    #[must_use]
    pub fn channel() -> (Self, Receiver<WorkerRouteProbeEvent>) {
        let (sender, receiver) = mpsc::channel();
        (
            Self {
                sender,
                identity: Arc::new(()),
            },
            receiver,
        )
    }

    fn report(&self, event: WorkerRouteProbeEvent) {
        let _ = self.sender.send(event);
    }
}

impl PartialEq for WorkerRouteProbe {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

impl Eq for WorkerRouteProbe {}

impl std::fmt::Debug for WorkerRouteProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerRouteProbe").finish_non_exhaustive()
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
    /// Bound for correlated worker replies and pending readback deadlines.
    pub worker_reply_timeout: Duration,
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
    /// Test-only: hold `FRAME_RESIZE_APPLIED` in the parent reader for one session.
    pub test_resize_ack_hold: Option<ResizeAckHold>,
    /// Test-only: report capture releases and PTY output routing decisions.
    pub test_route_probe: Option<WorkerRouteProbe>,
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
            worker_reply_timeout: DEFAULT_WORKER_REPLY_TIMEOUT,
            test_hold_after_read_ms: None,
            test_write_block_until_unix_ms: None,
            test_write_max_chunk: None,
            test_pending_capacity: None,
            test_hold_after_enqueue_ms: None,
            test_resize_ack_hold: None,
            test_route_probe: None,
            test_hold_before_exit_ms: None,
            test_exit_code: None,
            ghostty_max_scrollback_bytes: 10_000_000,
            terminal_color_profile: None,
        }
    }

    /// Override the correlated worker reply bound.
    #[must_use]
    pub const fn with_worker_reply_timeout(mut self, timeout: Duration) -> Self {
        self.worker_reply_timeout = timeout;
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

/// Final worker terminal state kept by the parent after exit.
///
/// Retained until [`WorkerProcessRuntime::take_final_state`] moves it into the
/// daemon retention table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedWorkerFinalState {
    /// Decoded final-state header.
    pub state: WorkerFinalState,
    /// Raw GHOSTSNP bytes when the worker exported one.
    pub snapshot: Option<Vec<u8>>,
}

/// Nonblocking view of one asynchronous worker spawn.
#[derive(Debug)]
pub enum WorkerSpawnPoll {
    /// The launch thread has not finished the worker handshake.
    Pending,
    /// The worker is installed and its session is live.
    Ready(SessionRuntimeHandle),
    /// The launch failed. Reservation state determines whether Core can release the identity.
    Failed(SessionRuntimeError),
}

struct PendingSpawn {
    reservation: SessionReservation,
    receiver: Option<
        Receiver<(
            SessionSpawnRequest,
            Result<LaunchedWorker, SessionRuntimeError>,
        )>,
    >,
    cleanup_error: Option<SessionRuntimeError>,
    /// The host cancelled the operation. The launched worker, when it
    /// arrives, is stopped and reaped instead of installed.
    abandoned: bool,
}

impl Drop for PendingSpawn {
    fn drop(&mut self) {
        self.reservation.pending_collected();
    }
}

/// A worker whose process is running and whose handshake succeeded, before
/// the parent reader and writer threads exist.
struct LaunchedWorker {
    /// `None` only after [`Self::into_parts`] transferred ownership to an
    /// installed session. Every other drop path stops and reaps the child.
    parts: Option<LaunchedParts>,
}

struct LaunchedParts {
    admission: Option<SessionReservation>,
    wake_handle: Option<SessionWakeHandle>,
    child: Child,
    control: WorkerControl,
    reader: Box<dyn Read + Send>,
    metadata: SessionMetadata,
    process: ProcessIdentity,
    supports_snapshot_boundary: bool,
}

#[cfg(test)]
static LAUNCHED_WORKER_DISCARDS: AtomicUsize = AtomicUsize::new(0);

impl LaunchedWorker {
    fn new(parts: LaunchedParts) -> Self {
        Self { parts: Some(parts) }
    }

    /// Transfer ownership to the installer. Cleanup responsibility leaves
    /// this guard only through this call.
    fn into_parts(mut self) -> LaunchedParts {
        self.parts
            .take()
            .expect("a launched worker is consumed at most once")
    }
}

impl Drop for LaunchedWorker {
    /// Transfer an uninstalled worker to cleanup without waiting here.
    fn drop(&mut self) {
        let Some(parts) = self.parts.take() else {
            return;
        };
        #[cfg(test)]
        LAUNCHED_WORKER_DISCARDS.fetch_add(1, Ordering::AcqRel);
        thread::spawn(move || cleanup_uninstalled_worker(parts));
    }
}

fn cleanup_uninstalled_worker(mut parts: LaunchedParts) {
    let mut session_ended = false;
    if parts.control.write_frame(FRAME_SHUTDOWN, &[]).is_ok() {
        while let Ok(frame) = read_frame(&mut parts.reader) {
            if frame.frame_type == FRAME_PROCESS_EXITED
                && serde_json::from_slice::<crate::ProcessExitedPayload>(&frame.payload).is_ok()
            {
                session_ended = true;
                break;
            }
        }
    }
    let _ = parts.child.kill();
    let worker_reaped = parts.child.wait().is_ok();
    parts.control.cleanup();
    if let Some(admission) = &parts.admission {
        if session_ended && worker_reaped {
            admission.observe_process_exit();
        } else {
            admission.cleanup_unconfirmed();
        }
    }
    notify_session_wake(&parts.wake_handle);
}

/// Whether a snapshot barrier cancel entered the worker control queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCancelAdmission {
    /// The cancel is queued; the runtime no longer tracks the request.
    Accepted,
    /// Every control slot is occupied. The request stays outstanding and
    /// the caller retains the obligation to retry.
    QueueFull,
    /// The control queue is sealed: a shutdown frame is already queued and
    /// ends the worker, barrier included. The request stays outstanding.
    Sealed,
}

/// Parent-side runtime adapter for one-worker-process-per-session local PTYs.
pub struct WorkerProcessRuntime {
    admission: SessionAdmission,
    options: WorkerProcessRuntimeOptions,
    sessions: HashMap<SessionId, WorkerProcessSession>,
    pending_spawns: HashMap<SessionId, PendingSpawn>,
    retained_final_states: HashMap<SessionId, RetainedWorkerFinalState>,
    wake_source: Option<TerminalWakeSource>,
    release_on_drop: bool,
    fail_next_start_writer: bool,
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
            admission: SessionAdmission::default(),
            options,
            sessions: HashMap::new(),
            pending_spawns: HashMap::new(),
            retained_final_states: HashMap::new(),
            wake_source: None,
            release_on_drop: false,
            fail_next_start_writer: false,
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

    /// Bound for correlated worker replies. Pending readbacks and ingress
    /// resize acknowledgements derive their deadlines from it.
    #[must_use]
    pub fn worker_reply_timeout(&self) -> Duration {
        self.options.worker_reply_timeout
    }

    /// Enqueue one client input operation for worker encoding and PTY write.
    ///
    /// `key` is the parent-unique correlation key echoed in the result. The
    /// worker replies exactly once per accepted frame, or the link fails.
    pub fn submit_input_operation(
        &mut self,
        session_id: &SessionId,
        key: u64,
        operation_id: u64,
        kind: WorkerInputKind,
        body: &[u8],
    ) -> Result<(), SessionRuntimeError> {
        let payload = encode_worker_input_operation(key, operation_id, kind, body);
        self.session_mut(session_id)?.enqueue_frame(
            ControlFrameClass::Ordinary,
            FRAME_INPUT_OPERATION,
            &payload,
        )
    }

    /// Ask the worker to abandon the unwritten remainder of one operation.
    ///
    /// The worker still replies once, with `Cancelled` when it caught the
    /// operation in time or with the terminal outcome it already reached.
    pub fn cancel_input_operation(
        &mut self,
        session_id: &SessionId,
        key: u64,
    ) -> Result<(), SessionRuntimeError> {
        self.session_mut(session_id)?.enqueue_frame(
            ControlFrameClass::Cancel,
            FRAME_INPUT_CANCEL,
            &key.to_le_bytes(),
        )
    }

    /// Take correlated input results in worker FIFO order.
    pub fn take_input_results(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<(u64, InputResultBody)>, SessionRuntimeError> {
        self.pump_session_output(session_id)?;
        Ok(self
            .session_mut(session_id)?
            .input_results
            .drain(..)
            .collect())
    }

    /// Take worker mode transitions in FIFO order.
    pub fn take_mode_changes(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<ModesBody>, SessionRuntimeError> {
        self.pump_session_output(session_id)?;
        Ok(self
            .session_mut(session_id)?
            .mode_changes
            .drain(..)
            .collect())
    }

    /// Latest worker-reported modes, or `None` before the worker reported any
    /// in this daemon incarnation.
    #[must_use]
    pub fn latest_modes(&self, session_id: &SessionId) -> Option<ModesBody> {
        self.sessions
            .get(session_id)
            .and_then(|session| session.latest_modes)
    }

    /// Start one correlated mode-flags probe. The reply arrives through
    /// [`Self::take_mode_flags_replies`].
    pub fn begin_mode_flags_probe(
        &mut self,
        session_id: &SessionId,
    ) -> Result<String, SessionRuntimeError> {
        let request_id = next_request_id("mode-flags");
        self.session_mut(session_id)?.enqueue_json(
            ControlFrameClass::Ordinary,
            FRAME_GET_MODE_FLAGS,
            &WorkerProbeRequest {
                request_id: request_id.clone(),
            },
        )?;
        Ok(request_id)
    }

    /// Take correlated mode-flags replies in worker FIFO order.
    pub fn take_mode_flags_replies(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<ModeFlagsPayload>, SessionRuntimeError> {
        self.pump_session_output(session_id)?;
        Ok(self
            .session_mut(session_id)?
            .mode_flags_replies
            .drain(..)
            .collect())
    }

    /// Start one correlated plain-text screen probe. The reply arrives
    /// through [`Self::take_screen_replies`].
    pub fn begin_screen_probe(
        &mut self,
        session_id: &SessionId,
    ) -> Result<String, SessionRuntimeError> {
        let request_id = next_request_id("screen");
        self.session_mut(session_id)?.enqueue_json(
            ControlFrameClass::Ordinary,
            FRAME_GET_SCREEN,
            &WorkerProbeRequest {
                request_id: request_id.clone(),
            },
        )?;
        Ok(request_id)
    }

    /// Take correlated screen replies in worker FIFO order.
    pub fn take_screen_replies(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<ScreenPayload>, SessionRuntimeError> {
        self.pump_session_output(session_id)?;
        Ok(self
            .session_mut(session_id)?
            .screen_replies
            .drain(..)
            .collect())
    }

    /// Take the final worker state retained when the session exited.
    ///
    /// Returns `None` when the worker ended without `FRAME_FINAL_STATE` or the
    /// state was already taken.
    pub fn take_final_state(&mut self, session_id: &SessionId) -> Option<RetainedWorkerFinalState> {
        self.retained_final_states.remove(session_id)
    }

    /// Start one worker spawn on a helper thread and return immediately.
    ///
    /// The session id is reserved until [`Self::poll_spawn`] reports a
    /// terminal state. The session wake fires when the launch finishes.
    pub fn begin_spawn(&mut self, request: SessionSpawnRequest) -> Result<(), SessionRuntimeError> {
        let reservation = self
            .admission
            .reserve_implicit(request.session_id.clone())?;
        let result = self.begin_spawn_reserved(&reservation, request);
        if result.is_err() {
            let _ = self.admission.release(&reservation);
        }
        result
    }

    /// Start the launch that owns this reservation.
    pub fn begin_spawn_reserved(
        &mut self,
        reservation: &SessionReservation,
        request: SessionSpawnRequest,
    ) -> Result<(), SessionRuntimeError> {
        let session_id = request.session_id.clone();
        if self.sessions.contains_key(&session_id) || self.pending_spawns.contains_key(&session_id)
        {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "worker process session already exists",
            ));
        }
        self.admission
            .begin_launch(reservation, &session_id, None)?;
        reservation.pending_started();
        let options = self.options.clone();
        let wake_handle = self
            .wake_source
            .as_ref()
            .map(|source| source.session_handle(session_id.clone()));
        let (sender, receiver) = mpsc::sync_channel(1);
        let launch_reservation = reservation.clone();
        thread::spawn(move || {
            let result =
                launch_reserved_worker(&options, &request, launch_reservation, wake_handle.clone());
            // A failed send drops the guard here; a queued guard drops with
            // the receiver. Both paths stop and reap the child.
            let _ = sender.send((request, result));
            notify_session_wake(&wake_handle);
        });
        self.pending_spawns.insert(
            session_id,
            PendingSpawn {
                reservation: reservation.clone(),
                receiver: Some(receiver),
                cleanup_error: None,
                abandoned: false,
            },
        );
        Ok(())
    }

    /// Mark a pending spawn abandoned. The launch thread keeps running; when
    /// its result arrives, [`Self::poll_spawn`] stops and reaps the child
    /// instead of installing it and reports `Failed`. Returns `false` when no
    /// spawn is pending for the session.
    pub fn abandon_spawn(&mut self, session_id: &SessionId) -> bool {
        match self.pending_spawns.get_mut(session_id) {
            Some(pending) => {
                pending.abandoned = true;
                true
            }
            None => false,
        }
    }

    /// Poll one spawn started by [`Self::begin_spawn`]. Never blocks.
    pub fn poll_spawn(&mut self, session_id: &SessionId) -> WorkerSpawnPoll {
        let Some(pending) = self.pending_spawns.get(session_id) else {
            return WorkerSpawnPoll::Failed(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SessionNotFound,
                format!("no pending worker spawn for session {}", session_id.0),
            ));
        };
        let abandoned = pending.abandoned;
        let reservation = pending.reservation.clone();
        let Some(receiver) = &pending.receiver else {
            match reservation.execution_state() {
                SessionReservationState::Launching => return WorkerSpawnPoll::Pending,
                SessionReservationState::Ended | SessionReservationState::CleanupUnconfirmed => {
                    let mut pending = self
                        .pending_spawns
                        .remove(session_id)
                        .expect("pending cleanup");
                    pending.reservation.pending_collected();
                    if reservation.state() == SessionReservationState::Ended {
                        self.forget_session_wake(session_id);
                        let _ = self.admission.retire_implicit(&reservation);
                    }
                    return WorkerSpawnPoll::Failed(
                        pending.cleanup_error.take().expect("cleanup failure"),
                    );
                }
                _ => return WorkerSpawnPoll::Pending,
            }
        };
        match receiver.try_recv() {
            Ok((_, Ok(launched))) if abandoned => {
                drop(launched);
                let pending = self
                    .pending_spawns
                    .get_mut(session_id)
                    .expect("pending launch");
                pending.receiver = None;
                pending.cleanup_error = Some(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    "worker spawn was abandoned before it finished",
                ));
                WorkerSpawnPoll::Pending
            }
            Ok((request, Ok(launched))) => {
                self.pending_spawns.remove(session_id);
                match self.install_launched(request, launched) {
                    Ok(handle) => WorkerSpawnPoll::Ready(handle),
                    Err(error) => WorkerSpawnPoll::Failed(error),
                }
            }
            Ok((_, Err(error))) => {
                self.pending_spawns.remove(session_id);
                if reservation.state() == SessionReservationState::Ended {
                    self.forget_session_wake(session_id);
                    let _ = self.admission.retire_implicit(&reservation);
                }
                WorkerSpawnPoll::Failed(error)
            }
            Err(TryRecvError::Empty) => WorkerSpawnPoll::Pending,
            Err(TryRecvError::Disconnected) => {
                self.pending_spawns.remove(session_id);
                reservation.launch_failed();
                WorkerSpawnPoll::Failed(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    "worker launch thread ended without a result",
                ))
            }
        }
    }

    /// Whether a spawn for this session is still on the launch thread.
    #[must_use]
    pub fn has_pending_spawn(&self, session_id: &SessionId) -> bool {
        self.pending_spawns.contains_key(session_id)
    }

    /// Start one worker-owned snapshot encode without waiting for READY.
    pub fn begin_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
    ) -> Result<String, SessionRuntimeError> {
        let request_id = next_request_id("snapshot");
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
                let events: Vec<_> = session
                    .pending_output
                    .drain(..boundary_len.min(session.pending_output.len()))
                    .collect();
                before_ready.extend(
                    events
                        .into_iter()
                        .filter_map(|event| session.state_order.admit(event))
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
    ///
    /// The cancel travels in a reserved control slot so ordinary traffic
    /// cannot starve it. When even the reserved slots are occupied the
    /// request stays outstanding and the caller must retry; the runtime
    /// never forgets a barrier it has not asked the worker to release.
    pub fn cancel_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> Result<SnapshotCancelAdmission, SessionRuntimeError> {
        let session = self.session_mut(session_id)?;
        let frame = crate::encode_json(
            crate::FRAME_GET_SNAPSHOT,
            &WorkerSnapshotRequest {
                request_id: request_id.to_owned(),
                cancel: true,
                complete: false,
            },
        )
        .map_err(|error| runtime_error(SessionRuntimeErrorKind::InputFailed, error))?;
        match session
            .control_queue
            .admit(ControlFrameClass::Cancel, frame)
        {
            Ok(()) => {
                session.outstanding_snapshot_request = None;
                session.snapshot_boundary.clear();
                Ok(SnapshotCancelAdmission::Accepted)
            }
            Err(ControlQueueAdmitError::ControlQueueFull) => Ok(SnapshotCancelAdmission::QueueFull),
            Err(ControlQueueAdmitError::Sealed) => Ok(SnapshotCancelAdmission::Sealed),
        }
    }

    /// Whether `request_id` is the barrier request this session still holds.
    ///
    /// Request ids come from one daemon-process-local allocator (wall-clock
    /// nanoseconds plus an ordinal) and are distinct until the ordinal wraps.
    /// A session installed by respawn or adoption starts with no outstanding
    /// request and can only receive newly allocated ids, so within one daemon
    /// process a match names the same worker session that began the barrier.
    /// This is not a persistent worker-incarnation id across daemon restarts;
    /// pending cancels live in memory, so none is needed.
    #[must_use]
    pub fn snapshot_request_is_outstanding(
        &self,
        session_id: &SessionId,
        request_id: &str,
    ) -> bool {
        self.sessions.get(session_id).is_some_and(|session| {
            session.outstanding_snapshot_request.as_deref() == Some(request_id)
        })
    }

    /// Ask the worker to apply any staged resize and release the PTY barrier.
    ///
    /// Never waits. [`Self::poll_snapshot_boundary`] reports `complete` when
    /// the worker confirms the release, or carries the release error.
    pub fn complete_snapshot_boundary(
        &mut self,
        session_id: &SessionId,
        request_id: &str,
    ) -> Result<(), SessionRuntimeError> {
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
        if let Some(probe) = &self.options.test_route_probe {
            probe.report(WorkerRouteProbeEvent::CaptureReleaseSent {
                session_id: session_id.clone(),
            });
        }
        Ok(())
    }

    /// Whether the runtime still holds this session: a live one, or an exited
    /// one kept for a capture it owes.
    #[must_use]
    pub fn holds_session(&self, session_id: &SessionId) -> bool {
        self.sessions.contains_key(session_id)
    }

    /// Hold an exited session, and its worker, while the engine still owes a
    /// capture on it. Releasing the hold of an exited session wakes it, so
    /// the next drain removes it and shuts its worker down.
    pub fn set_exit_hold(&mut self, session_id: &SessionId, hold: bool) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            let released = session.exit_hold && !hold;
            session.exit_hold = hold;
            if released && session.exit_reported {
                notify_session_wake(&session.wake_handle);
            }
        }
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

    /// Fail the control plane and end the control link to the worker.
    ///
    /// This is the explicit cleanup path for a worker that no longer accepts
    /// control frames: the queue is sealed, the write half is hard-stopped
    /// (the child is killed on stdio control; the socket write half is shut
    /// on socket control), and the session's outstanding barrier request is
    /// dropped. The worker's control reader observes end of stream and
    /// releases any active snapshot barrier itself; the parent no longer owns
    /// a request the worker can still be asked about.
    ///
    /// Exactly once per session: a later call on a failed plane is a no-op,
    /// so the writer-failure sweep and the barrier deadline path cannot
    /// double-clean. Recovery is respawn only.
    pub fn fail_control_plane(&mut self, session_id: &SessionId, error: ControlWriterError) {
        let Some(session) = self.sessions.get_mut(session_id) else {
            return;
        };
        if matches!(session.control_plane, ControlPlaneState::Failed(_)) {
            return;
        }
        session.control_plane = ControlPlaneState::Failed(error);
        session.control_queue.seal();
        session.control.hard_stop_write(session.child.as_mut());
        session.outstanding_snapshot_request = None;
        session.snapshot_boundary.clear();
    }

    #[cfg(test)]
    pub(crate) fn test_has_session(&self, session_id: &SessionId) -> bool {
        self.sessions.contains_key(session_id)
    }

    /// Record a writer failure as the writer thread would. Crate tests use it
    /// to model a sealed queue whose seal came from a failed write.
    #[cfg(test)]
    pub(crate) fn test_fail_control_writer(
        &self,
        session_id: &SessionId,
        error: ControlWriterError,
    ) {
        if let Some(session) = self.sessions.get(session_id) {
            session.control_queue.seal();
            session.writer_slot.set(ControlWriterOutcome::Failed {
                error,
                consumed: false,
            });
        }
    }

    /// Forget a session installed by [`Self::insert_test_session`].
    #[cfg(test)]
    pub(crate) fn remove_test_session(&mut self, session_id: &SessionId) {
        if let Some(session) = self.sessions.remove(session_id) {
            if let Some(admission) = session.admission.as_ref() {
                admission.runtime_removed();
                admission.runtime_ended();
                self.admission
                    .retire_implicit(admission)
                    .expect("retire test admission");
            }
        }
    }

    /// Install a session with no child, no writer, and an undrained control
    /// queue. Crate tests use it to exercise control-queue admission paths.
    #[cfg(test)]
    pub(crate) fn insert_test_session(&mut self, session_id: SessionId) {
        self.insert_test_session_with_control(session_id, WorkerControl::ReleasedStdio);
    }

    /// Install a test session whose control link is one end of a socket
    /// pair, as after `take_write_half` released it. Returns the worker-side
    /// end so a test can observe end of stream on the link.
    #[cfg(test)]
    pub(crate) fn insert_test_socket_session(&mut self, session_id: SessionId) -> UnixStream {
        let (parent, worker) = UnixStream::pair().expect("socket pair");
        self.insert_test_session_with_control(
            session_id,
            WorkerControl::ReleasedSocket {
                stream: parent,
                path: PathBuf::from("/nonexistent/test-control.sock"),
                identity: None,
            },
        );
        worker
    }

    #[cfg(test)]
    fn insert_test_session_with_control(&mut self, session_id: SessionId, control: WorkerControl) {
        let (_sender, receiver) = mpsc::channel();
        let admission = self
            .admission
            .reserve_implicit(session_id.clone())
            .expect("test admission");
        self.admission
            .begin_launch(&admission, &session_id, None)
            .expect("test launch");
        admission.runtime_installed();
        admission.runtime_installing();
        let session = WorkerProcessSession {
            admission: Some(admission),
            child: None,
            control,
            control_queue: ControlQueue::new(),
            writer_slot: ControlWriterSlot::running(),
            control_plane: ControlPlaneState::Live,
            writer: None,
            wake_handle: None,
            metadata: SessionMetadata {
                session_uuid: session_id.0.clone(),
                pid: 0,
                rows: 24,
                cols: 80,
                last_output_at: 0,
                title: None,
                cwd: None,
                port: None,
                mode_flags: Default::default(),
                recovery_identity: None,
            },
            output: receiver,
            overflow: Arc::new(ReaderOverflow::default()),
            state_order: StateOrder::default(),
            pong_count: Arc::new(AtomicUsize::new(0)),
            last_health: Arc::new(Mutex::new(None)),
            completion: Arc::new(Mutex::new(WorkerCompletion {
                process_exited: None,
                final_state: None,
                reader_finished: false,
            })),
            latest_modes: None,
            input_results: VecDeque::new(),
            mode_changes: VecDeque::new(),
            mode_flags_replies: VecDeque::new(),
            screen_replies: VecDeque::new(),
            pending_output: VecDeque::new(),
            applied_resizes: VecDeque::new(),
            snapshot_boundary: VecDeque::new(),
            outstanding_snapshot_request: None,
            exit_hold: false,
            exit_reported: false,
            supports_snapshot_boundary: true,
            egress_capacity: 1,
            stall: Arc::new(EgressStall::new()),
        };
        self.sessions.insert(session_id, session);
    }

    fn pump_session_output(&mut self, session_id: &SessionId) -> Result<(), SessionRuntimeError> {
        let session = self.session_mut(session_id)?;
        let mut drained = false;
        while let Ok(event) = session.output.try_recv() {
            drained = true;
            match event {
                WorkerChannelEvent::Output(output) => session.pending_output.push_back(output),
                WorkerChannelEvent::ModeFlags(payload) => {
                    if payload.error_kind.is_none() {
                        session.latest_modes = Some(ModesBody {
                            mode_bits: payload.mode_flags.to_mode_bits(),
                            rows: payload.rows,
                            cols: payload.cols,
                        });
                    }
                    session.mode_flags_replies.push_back(payload);
                }
                WorkerChannelEvent::Screen(payload) => session.screen_replies.push_back(payload),
                WorkerChannelEvent::InputResult(key, result) => {
                    session.input_results.push_back((key, result));
                }
                WorkerChannelEvent::ModesChanged(modes) => {
                    session.latest_modes = Some(modes);
                    session.mode_changes.push_back(modes);
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

    /// Whether the worker egress reader has ended (EOF or link failure).
    pub fn session_reader_finished(
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
        let reservation = self.admission.reserve_implicit(session_id.clone())?;
        let result = self.adopt_session_reserved(
            &reservation,
            session_id,
            process,
            socket_path,
            supports_snapshot_boundary,
        );
        if result.is_err() {
            let _ = self.admission.release(&reservation);
        }
        result
    }

    /// Adopt only the worker that owns the supplied reservation.
    #[cfg(unix)]
    pub fn adopt_session_reserved(
        &mut self,
        reservation: &SessionReservation,
        session_id: SessionId,
        process: ProcessIdentity,
        socket_path: impl Into<PathBuf>,
        supports_snapshot_boundary: bool,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
        if self.sessions.contains_key(&session_id) || self.pending_spawns.contains_key(&session_id)
        {
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                "worker process session already exists",
            ));
        }
        self.admission
            .begin_launch(reservation, &session_id, None)?;
        let result = self.adopt_reserved_inner(
            reservation,
            session_id,
            process,
            socket_path.into(),
            supports_snapshot_boundary,
        );
        if result.is_err() {
            reservation.launch_failed();
        }
        result
    }

    #[cfg(unix)]
    fn adopt_reserved_inner(
        &mut self,
        reservation: &SessionReservation,
        session_id: SessionId,
        process: ProcessIdentity,
        socket_path: PathBuf,
        supports_snapshot_boundary: bool,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
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
            .set_read_timeout(Some(self.options.worker_reply_timeout))
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
        if process.pid.is_some_and(|pid| pid == metadata.pid) {
            reservation.capture_process_group(metadata_process_group(&metadata));
        }
        let reader = control.try_clone().map_err(|error| {
            SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("clone worker control socket failed: {error}"),
            )
        })?;
        // The adopted welcome repeats spawn-time modes. Probe for live ones.
        self.install_session(
            session_id.clone(),
            Some(reservation.clone()),
            None,
            WorkerControl::Socket {
                stream: control,
                path: socket_path,
                identity,
            },
            Box::new(reader),
            metadata,
            supports_snapshot_boundary,
            None,
        )?;
        self.begin_mode_flags_probe(&session_id)?;

        Ok(SessionRuntimeHandle {
            request_id: crate::RequestId(format!("{}-adopt", session_id.0)),
            session_id,
            process,
        })
    }

    fn install_launched(
        &mut self,
        request: SessionSpawnRequest,
        launched: LaunchedWorker,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
        let parts = launched.into_parts();
        let seed_modes = ModesBody {
            mode_bits: parts.metadata.mode_flags.to_mode_bits(),
            rows: parts.metadata.rows,
            cols: parts.metadata.cols,
        };
        self.install_session(
            request.session_id.clone(),
            parts.admission,
            Some(parts.child),
            parts.control,
            parts.reader,
            parts.metadata,
            parts.supports_snapshot_boundary,
            Some(seed_modes),
        )?;
        Ok(SessionRuntimeHandle {
            request_id: request.request_id,
            session_id: request.session_id,
            process: parts.process,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn install_session(
        &mut self,
        session_id: SessionId,
        admission: Option<SessionReservation>,
        child: Option<Child>,
        control: WorkerControl,
        reader: Box<dyn Read + Send>,
        metadata: SessionMetadata,
        supports_snapshot_boundary: bool,
        latest_modes: Option<ModesBody>,
    ) -> Result<(), SessionRuntimeError> {
        let (sender, receiver) = mpsc::sync_channel(self.options.egress_capacity.max(1));
        let overflow = Arc::new(ReaderOverflow::default());
        let pong_count = Arc::new(AtomicUsize::new(0));
        let last_health = Arc::new(Mutex::new(None));
        let completion = Arc::new(Mutex::new(WorkerCompletion::default()));
        let stall = Arc::new(EgressStall::new());
        let wake_handle = self
            .wake_source
            .as_ref()
            .map(|source| source.session_handle(session_id.clone()));
        spawn_stdout_reader(
            reader,
            sender,
            Arc::clone(&overflow),
            Arc::clone(&pong_count),
            Arc::clone(&last_health),
            Arc::clone(&completion),
            Arc::clone(&stall),
            wake_handle.clone(),
            session_id.clone(),
            self.options.test_resize_ack_hold.clone(),
            self.options.test_route_probe.clone(),
        );
        let mut session = WorkerProcessSession {
            admission,
            child,
            control,
            control_queue: ControlQueue::new(),
            writer_slot: ControlWriterSlot::running(),
            control_plane: ControlPlaneState::Live,
            writer: None,
            wake_handle,
            metadata,
            output: receiver,
            overflow,
            state_order: StateOrder::default(),
            pong_count,
            last_health,
            completion,
            latest_modes,
            input_results: VecDeque::new(),
            mode_changes: VecDeque::new(),
            mode_flags_replies: VecDeque::new(),
            screen_replies: VecDeque::new(),
            pending_output: VecDeque::new(),
            applied_resizes: VecDeque::new(),
            snapshot_boundary: VecDeque::new(),
            outstanding_snapshot_request: None,
            exit_hold: false,
            exit_reported: false,
            supports_snapshot_boundary,
            egress_capacity: self.options.egress_capacity.max(1),
            stall,
        };
        if let Err(error) = self.start_writer_or_forget(&mut session, &session_id) {
            if let Some(admission) = &session.admission {
                admission.cleanup_unconfirmed();
            }
            // The session was never published; stop the worker we own.
            session.close_before_blocking_shutdown();
            session.shutdown_control();
            if let Some(mut child) = session.child.take() {
                let _ = child.kill();
                match child.try_wait() {
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => reap_worker_child_in_background(child),
                }
            }
            session.control.cleanup();
            return Err(error);
        }
        if let Some(admission) = &session.admission {
            admission.runtime_installing();
            admission.runtime_installed();
            admission.pending_collected();
        }
        self.sessions.insert(session_id, session);
        Ok(())
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

/// Spawn the reserved worker process and complete the hello/welcome handshake.
///
/// Runs without the runtime lock so [`WorkerProcessRuntime::begin_spawn`] can
/// move it to a helper thread. Blocks up to the worker startup timeout.
fn launch_reserved_worker(
    options: &WorkerProcessRuntimeOptions,
    request: &SessionSpawnRequest,
    admission: SessionReservation,
    wake_handle: Option<SessionWakeHandle>,
) -> Result<LaunchedWorker, SessionRuntimeError> {
    let result = launch_worker_inner(options, request, Some(admission.clone()), wake_handle);
    if result.is_err() {
        admission.launch_failed();
    }
    result
}

fn launch_worker_inner(
    options: &WorkerProcessRuntimeOptions,
    request: &SessionSpawnRequest,
    admission: Option<SessionReservation>,
    wake_handle: Option<SessionWakeHandle>,
) -> Result<LaunchedWorker, SessionRuntimeError> {
    let mut command = Command::new(&options.worker_path);
    command
        .arg("--egress-capacity")
        .arg(options.egress_capacity.to_string())
        .arg("--pty-reader-capacity")
        .arg(options.pty_reader_chunk_capacity.to_string())
        .arg("--ghostty-max-scrollback-bytes")
        .arg(options.ghostty_max_scrollback_bytes.to_string());
    if let Some(profile) = options.terminal_color_profile.as_ref() {
        command
            .arg("--terminal-color-profile")
            .arg(serde_json::to_string(profile).map_err(|error| {
                SessionRuntimeError::new(SessionRuntimeErrorKind::SpawnFailed, error.to_string())
            })?);
    }
    command
        .arg("--shutdown-grace-ms")
        .arg(options.shutdown_grace_ms.to_string())
        .arg("--poll-interval-ms")
        .arg(options.poll_interval_ms.to_string())
        .stderr(Stdio::piped());
    if let Some(hold_ms) = options.test_hold_after_read_ms {
        command
            .arg("--test-hold-after-read-ms")
            .arg(hold_ms.to_string());
    }
    if let Some(until) = options.test_write_block_until_unix_ms {
        command
            .arg("--test-write-block-until-unix-ms")
            .arg(until.to_string());
    }
    if let Some(max_chunk) = options.test_write_max_chunk {
        command
            .arg("--test-write-max-chunk")
            .arg(max_chunk.to_string());
    }
    if let Some(capacity) = options.test_pending_capacity {
        command
            .arg("--test-pending-capacity")
            .arg(capacity.to_string());
    }
    if let Some(hold_ms) = options.test_hold_after_enqueue_ms {
        command
            .arg("--test-hold-after-enqueue-ms")
            .arg(hold_ms.to_string());
    }
    if let Some(hold_ms) = options.test_hold_before_exit_ms {
        command
            .arg("--test-hold-before-exit-ms")
            .arg(hold_ms.to_string());
    }
    if let Some(exit_code) = options.test_exit_code {
        command.arg("--test-exit-code").arg(exit_code.to_string());
    }

    #[cfg(unix)]
    let socket_path = options
        .control_socket_dir
        .as_ref()
        .map(|dir| worker_socket_path(dir, &request.session_id))
        .transpose()?;
    #[cfg(not(unix))]
    let socket_path: Option<PathBuf> = None;
    let _socket_mode = socket_path.is_some();
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

    let (mut control, mut reader): (WorkerControl, Box<dyn Read + Send>) = if let Some(path) =
        socket_path
    {
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
            SessionRuntimeError::new(SessionRuntimeErrorKind::SpawnFailed, "worker stdin missing")
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
        if let Some(admission) = &admission {
            admission.creation_possible();
        }
        control.write_json(FRAME_SPAWN_SESSION, request)?;
        read_startup_reply(&mut reader)
            .map_err(|error| runtime_error(SessionRuntimeErrorKind::SpawnFailed, error))
    })()
    .map_err(|error: SessionRuntimeError| {
        SessionRuntimeError::new(SessionRuntimeErrorKind::SpawnFailed, error.message)
    });
    let child_id = pending_worker.child_id();
    let metadata = match startup {
        Ok(StartupReply::StartupFailure(report)) => {
            apply_startup_failure(admission.as_ref(), request, child_id, &report);
            return Err(SessionRuntimeError::new(
                SessionRuntimeErrorKind::SpawnFailed,
                format!("worker startup failed: {}", report.message),
            ));
        }
        Ok(StartupReply::Welcome {
            version: peer_version,
            metadata,
        }) => {
            if peer_version != PROTOCOL_VERSION {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    format!("unsupported worker protocol version: {peer_version}"),
                ));
            }
            if metadata.session_uuid != request.session_id.0 {
                return Err(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::SpawnFailed,
                    "worker welcome identified a different session",
                ));
            }
            let worker_pid = metadata
                .recovery_identity
                .as_ref()
                .and_then(|identity| identity.get("worker_pid"))
                .and_then(serde_json::Value::as_u64);
            if worker_pid != Some(u64::from(child_id)) {
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
    if let Some(admission) = &admission {
        admission.capture_process_group(metadata_process_group(&metadata));
    }
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
    let supports_snapshot_boundary = metadata.recovery_identity.as_ref().is_some_and(|identity| {
        identity
            .get("atomic_snapshot_boundary")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
            && identity
                .get("snapshot_delivery")
                .and_then(serde_json::Value::as_str)
                == Some("ready_then_history")
    });
    Ok(LaunchedWorker::new(LaunchedParts {
        admission,
        wake_handle,
        child: pending_worker.take(),
        control,
        reader,
        metadata,
        process,
        supports_snapshot_boundary,
    }))
}

impl SessionRuntime for WorkerProcessRuntime {
    fn session_admission(&self) -> Option<&SessionAdmission> {
        Some(&self.admission)
    }

    fn spawn_reserved(
        &mut self,
        reservation: &SessionReservation,
        request: SessionSpawnRequest,
    ) -> Result<SessionRuntimeHandle, ReservedSessionSpawnError> {
        if self.sessions.contains_key(&request.session_id)
            || self.pending_spawns.contains_key(&request.session_id)
        {
            return Err(ReservedSessionSpawnError::Refused(
                super::SessionReservationRefusal::Occupied.into(),
            ));
        }
        self.admission
            .begin_launch(reservation, &request.session_id, None)
            .map_err(|error| ReservedSessionSpawnError::Refused(error.into()))?;
        let launched = launch_reserved_worker(
            &self.options,
            &request,
            reservation.clone(),
            self.wake_source
                .as_ref()
                .map(|source| source.session_handle(request.session_id.clone())),
        )
        .map_err(ReservedSessionSpawnError::Admitted)?;
        self.install_launched(request, launched)
            .map_err(ReservedSessionSpawnError::Admitted)
    }

    /// Spawn synchronously: launch on this thread, then install.
    ///
    /// Core pending spawns use [`WorkerProcessRuntime::begin_spawn`] instead so
    /// the engine thread never waits on worker startup.
    fn spawn_session(
        &mut self,
        request: SessionSpawnRequest,
    ) -> Result<SessionRuntimeHandle, SessionRuntimeError> {
        let reservation = self
            .admission
            .reserve_implicit(request.session_id.clone())?;
        let result = self.spawn_reserved(&reservation, request);
        if result.is_err() {
            let _ = self.admission.release(&reservation);
        }
        result.map_err(ReservedSessionSpawnError::into_runtime_error)
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

    fn drain_output(
        &mut self,
        session_id: &SessionId,
    ) -> Result<Vec<SessionRuntimeOutput>, SessionRuntimeError> {
        // Demux correlated replies before yielding output so typed queues
        // and pending buffers stay ordered.
        self.pump_session_output(session_id)?;
        let mut output = Vec::new();
        let completed = {
            let session = self.session_mut(session_id)?;
            session.take_reader_output(session_id, &mut output);
            #[cfg(test)]
            before_completion_read(session);
            let completion = session.completion.lock().map_err(lock_error)?;
            completion.process_exited.clone()
        };

        if let Some(payload) = completed {
            // The reader stores FRAME_PROCESS_EXITED only after earlier frames
            // were accepted into the channel and earlier drops were recorded.
            // Re-pump and take the overflow once more, so a raced last PTY
            // chunk or a drop recorded after the first take is not lost when
            // the session is removed.
            self.pump_session_output(session_id)?;
            let session = self.session_mut(session_id)?;
            session.take_reader_output(session_id, &mut output);
            let first_report = !session.exit_reported;
            session.exit_reported = true;
            // A capture the engine still owes a route is served from the
            // worker's final terminal model; removal shuts that worker down.
            let held = session.exit_hold || session.outstanding_snapshot_request.is_some();
            // Map removal transfers wake-retirement ownership to CoreDaemon.
            if let Some(mut removed) = (!held).then(|| self.sessions.remove(session_id)).flatten() {
                if let Some(admission) = &removed.admission {
                    admission.runtime_removed();
                    admission.observe_process_exit();
                    let _ = self.admission.retire_implicit(admission);
                }
                let final_state = removed
                    .completion
                    .lock()
                    .ok()
                    .and_then(|mut completion| completion.final_state.take());
                if let Some(final_state) = final_state {
                    self.retained_final_states
                        .insert(session_id.clone(), final_state);
                }
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
            if first_report {
                output.push(SessionRuntimeOutput::ProcessExited {
                    session_id: session_id.clone(),
                    payload,
                });
            }
        }

        Ok(output)
    }
}

fn apply_startup_failure(
    admission: Option<&SessionReservation>,
    request: &SessionSpawnRequest,
    worker_pid: u32,
    report: &StartupFailureReport,
) {
    let Some(admission) = admission else {
        return;
    };
    if report.session_id != request.session_id.0
        || report.request_id != request.request_id.0
        || report.worker_pid != worker_pid
    {
        admission.cleanup_unconfirmed();
        return;
    }
    match &report.outcome {
        StartupFailureOutcome::NotCreated => admission.runtime_ended(),
        StartupFailureOutcome::Created {
            process_group_id, ..
        } => {
            admission.mark_startup_created();
            admission.capture_process_group(*process_group_id);
            admission.cleanup_unconfirmed();
        }
    }
}

fn metadata_process_group(metadata: &SessionMetadata) -> Option<i32> {
    metadata
        .recovery_identity
        .as_ref()?
        .get("process_group_id")?
        .as_i64()
        .and_then(|group| i32::try_from(group).ok())
        .filter(|group| *group > 1)
}

impl Drop for WorkerProcessRuntime {
    fn drop(&mut self) {
        // Dropping the receivers drops any queued launch guard, which stops
        // and reaps its child; a launch still running drops its guard when
        // its send fails.
        self.pending_spawns.clear();
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
    admission: Option<SessionReservation>,
    child: Option<Child>,
    control: WorkerControl,
    control_queue: ControlQueue,
    writer_slot: ControlWriterSlot,
    control_plane: ControlPlaneState,
    writer: Option<thread::JoinHandle<()>>,
    wake_handle: Option<SessionWakeHandle>,
    metadata: SessionMetadata,
    output: Receiver<WorkerChannelEvent>,
    overflow: Arc<ReaderOverflow>,
    /// Newest title/cwd emitted, so a stale queued value never follows it.
    state_order: StateOrder,
    pong_count: Arc<AtomicUsize>,
    last_health: Arc<Mutex<Option<WorkerHealth>>>,
    completion: Arc<Mutex<WorkerCompletion>>,
    latest_modes: Option<ModesBody>,
    input_results: VecDeque<(u64, InputResultBody)>,
    mode_changes: VecDeque<ModesBody>,
    mode_flags_replies: VecDeque<ModeFlagsPayload>,
    screen_replies: VecDeque<ScreenPayload>,
    pending_output: VecDeque<WorkerOutputEvent>,
    applied_resizes: VecDeque<crate::ResizePayload>,
    snapshot_boundary: VecDeque<(WorkerSnapshotResult, usize)>,
    outstanding_snapshot_request: Option<String>,
    /// The engine still has a capture for this session, active or queued.
    /// After the exit the worker keeps serving captures from its final
    /// terminal model, so the session is removed only once no hold remains.
    exit_hold: bool,
    /// `ProcessExited` was reported; it is reported once.
    exit_reported: bool,
    supports_snapshot_boundary: bool,
    egress_capacity: usize,
    stall: Arc<EgressStall>,
}

impl WorkerProcessSession {
    /// Take what the reader queued or could not deliver, in order: the PTY
    /// drop summary, the queued events, then the coalesced state values and
    /// the lost-event summary, which are newer than the queued events.
    fn take_reader_output(
        &mut self,
        session_id: &SessionId,
        output: &mut Vec<SessionRuntimeOutput>,
    ) {
        let overflow = self.overflow.pty.swap(0, Ordering::AcqRel);
        if overflow > 0 {
            output.push(SessionRuntimeOutput::Backpressure(BackpressureSummary {
                source: QueueSource::SessionIo,
                capacity: self.egress_capacity,
                depth: self.egress_capacity,
                route: BackpressureRoute {
                    session_id: Some(session_id.clone()),
                    client_id: None,
                    subscription_id: None,
                    plugin_key: None,
                },
            }));
        }
        while let Some(event) = self.pending_output.pop_front() {
            if let Some(event) = self.state_order.admit(event) {
                output.push(event.into_runtime_output(session_id));
            }
        }
        output.extend(self.overflow.take_outputs(
            session_id,
            self.egress_capacity,
            &mut self.state_order,
        ));
    }

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

#[derive(Default)]
struct WorkerCompletion {
    process_exited: Option<ProcessExitedPayload>,
    final_state: Option<RetainedWorkerFinalState>,
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
    /// A title, stamped with the reader's state sequence.
    TitleChanged(u64, String),
    /// A working directory, stamped with the reader's state sequence.
    CwdChanged(u64, String),
    PromptMark(PromptMarkPayload),
    Bell,
    Notification(NotificationPayload),
    MetadataShaping(TerminalMetadataShapingObservation),
}

enum WorkerChannelEvent {
    Output(WorkerOutputEvent),
    ModeFlags(ModeFlagsPayload),
    Screen(ScreenPayload),
    InputResult(u64, InputResultBody),
    ModesChanged(ModesBody),
    ResizeApplied(crate::ResizePayload),
    Snapshot(WorkerSnapshotResult),
}

impl WorkerOutputEvent {
    fn into_runtime_output(self, session_id: &SessionId) -> SessionRuntimeOutput {
        match self {
            Self::PtyOutput(data) => SessionRuntimeOutput::PtyOutput {
                session_id: session_id.clone(),
                data,
            },
            Self::TitleChanged(_, title) => SessionRuntimeOutput::TitleChanged {
                session_id: session_id.clone(),
                title,
            },
            Self::CwdChanged(_, cwd) => SessionRuntimeOutput::CwdChanged {
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
    overflow: Arc<ReaderOverflow>,
    pong_count: Arc<AtomicUsize>,
    last_health: Arc<Mutex<Option<WorkerHealth>>>,
    completion: Arc<Mutex<WorkerCompletion>>,
    stall: Arc<EgressStall>,
    wake_handle: Option<SessionWakeHandle>,
    session_id: crate::SessionId,
    resize_ack_hold: Option<ResizeAckHold>,
    route_probe: Option<WorkerRouteProbe>,
) {
    thread::spawn(move || {
        while let Ok(frame) = read_frame(&mut stdout) {
            match frame.frame_type {
                FRAME_PTY_OUTPUT => {
                    route_worker_event(
                        &sender,
                        &overflow,
                        &stall,
                        &wake_handle,
                        WorkerChannelEvent::Output(WorkerOutputEvent::PtyOutput(frame.payload)),
                        |routing| {
                            if let Some(probe) = &route_probe {
                                probe.report(WorkerRouteProbeEvent::PtyOutputRouted {
                                    session_id: session_id.clone(),
                                    routing,
                                });
                            }
                        },
                    );
                }
                FRAME_PROCESS_EXITED => {
                    if let Ok(payload) = serde_json::from_slice(&frame.payload) {
                        if let Ok(mut state) = completion.lock() {
                            state.process_exited = Some(payload);
                        }
                    }
                    if let Some(probe) = &route_probe {
                        probe.report(WorkerRouteProbeEvent::ProcessExitRead {
                            session_id: session_id.clone(),
                        });
                    }
                    notify_session_wake(&wake_handle);
                }
                FRAME_FINAL_STATE => {
                    if let Ok((state, snapshot)) = decode_final_state(&frame.payload) {
                        let snapshot = state.has_snapshot.then(|| snapshot.to_vec());
                        if let Ok(mut completion) = completion.lock() {
                            completion.final_state =
                                Some(RetainedWorkerFinalState { state, snapshot });
                        }
                    }
                }
                FRAME_TITLE_CHANGED => {
                    if let Ok(title) = String::from_utf8(frame.payload) {
                        let seq = overflow.next_state_seq();
                        send_worker_event(
                            &sender,
                            &overflow,
                            &stall,
                            &wake_handle,
                            WorkerChannelEvent::Output(WorkerOutputEvent::TitleChanged(seq, title)),
                        );
                    }
                }
                FRAME_CWD_CHANGED => {
                    if let Ok(cwd) = String::from_utf8(frame.payload) {
                        let seq = overflow.next_state_seq();
                        send_worker_event(
                            &sender,
                            &overflow,
                            &stall,
                            &wake_handle,
                            WorkerChannelEvent::Output(WorkerOutputEvent::CwdChanged(seq, cwd)),
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
                    if let Ok(payload) = serde_json::from_slice::<ModeFlagsPayload>(&frame.payload)
                    {
                        send_correlated(
                            &sender,
                            &wake_handle,
                            WorkerChannelEvent::ModeFlags(payload),
                        );
                    }
                }
                FRAME_SCREEN => {
                    if let Ok(payload) = serde_json::from_slice::<ScreenPayload>(&frame.payload) {
                        send_correlated(&sender, &wake_handle, WorkerChannelEvent::Screen(payload));
                    }
                }
                FRAME_INPUT_RESULT => {
                    let decoded =
                        split_worker_operation_key(&frame.payload)
                            .ok()
                            .and_then(|(key, body)| {
                                TerminalFrame::from_bytes(body)
                                    .ok()
                                    .and_then(|frame| decode_input_result(&frame).ok())
                                    .map(|result| (key, result))
                            });
                    if let Some((key, result)) = decoded {
                        // Results are correlated and must not be dropped under
                        // egress pressure: a lost result leaks a client slot.
                        if sender
                            .send(WorkerChannelEvent::InputResult(key, result))
                            .is_err()
                        {
                            break;
                        }
                        notify_session_wake(&wake_handle);
                    }
                }
                FRAME_MODES_CHANGED => {
                    if let Some(modes) = TerminalFrame::from_bytes(&frame.payload)
                        .ok()
                        .and_then(|frame| decode_modes(&frame).ok())
                    {
                        if sender
                            .send(WorkerChannelEvent::ModesChanged(modes))
                            .is_err()
                        {
                            break;
                        }
                        notify_session_wake(&wake_handle);
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

/// The newest title and cwd a session has emitted, by reader stamp. A value
/// older than the one already emitted is stale and is dropped, whether it
/// arrives from the channel or from an overflow slot.
#[derive(Default)]
struct StateOrder {
    title: u64,
    cwd: u64,
}

impl StateOrder {
    fn admit(&mut self, event: WorkerOutputEvent) -> Option<WorkerOutputEvent> {
        let (newest, seq) = match &event {
            WorkerOutputEvent::TitleChanged(seq, _) => (&mut self.title, *seq),
            WorkerOutputEvent::CwdChanged(seq, _) => (&mut self.cwd, *seq),
            _ => return Some(event),
        };
        if seq <= *newest {
            return None;
        }
        *newest = seq;
        Some(event)
    }
}

/// What the worker reader could not deliver: the channel was full and no
/// consumer could be stalled for.
///
/// Live PTY chunks are dropped and counted (the worker's terminal model
/// already holds them). Title and cwd are state: the latest value waits in a
/// slot, and a newer value the channel accepts supersedes it. Occurrence
/// events are counted and reported as a `SessionEvents` summary, never
/// silently lost.
#[derive(Default)]
struct ReaderOverflow {
    pty: AtomicUsize,
    events: AtomicUsize,
    /// Orders title and cwd values as the reader read them. A queued value
    /// and a slot value can reach a drain in either order; the stamp keeps
    /// an older one from following a newer one.
    state_seq: std::sync::atomic::AtomicU64,
    title: Mutex<Option<(u64, String)>>,
    cwd: Mutex<Option<(u64, String)>>,
}

impl ReaderOverflow {
    /// The next title/cwd stamp, in the order the reader reads them.
    fn next_state_seq(&self) -> u64 {
        self.state_seq.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn record_drop(&self, event: WorkerChannelEvent) {
        match event {
            WorkerChannelEvent::Output(WorkerOutputEvent::PtyOutput(_)) => {
                self.pty.fetch_add(1, Ordering::AcqRel);
            }
            WorkerChannelEvent::Output(WorkerOutputEvent::TitleChanged(seq, title)) => {
                if let Ok(mut slot) = self.title.lock() {
                    *slot = Some((seq, title));
                }
            }
            WorkerChannelEvent::Output(WorkerOutputEvent::CwdChanged(seq, cwd)) => {
                if let Ok(mut slot) = self.cwd.lock() {
                    *slot = Some((seq, cwd));
                }
            }
            _ => {
                self.events.fetch_add(1, Ordering::AcqRel);
            }
        }
    }

    /// The slot a state event would supersede once the channel accepts it.
    fn state_slot(&self, event: &WorkerChannelEvent) -> Option<&Mutex<Option<(u64, String)>>> {
        match event {
            WorkerChannelEvent::Output(WorkerOutputEvent::TitleChanged(..)) => Some(&self.title),
            WorkerChannelEvent::Output(WorkerOutputEvent::CwdChanged(..)) => Some(&self.cwd),
            _ => None,
        }
    }

    /// Coalesced state values and the lost-event summary, after the queued
    /// output they are newer than.
    fn take_outputs(
        &self,
        session_id: &SessionId,
        capacity: usize,
        order: &mut StateOrder,
    ) -> Vec<SessionRuntimeOutput> {
        let mut output = Vec::new();
        if let Some((seq, title)) = self.title.lock().ok().and_then(|mut slot| slot.take()) {
            if let Some(event) = order.admit(WorkerOutputEvent::TitleChanged(seq, title)) {
                output.push(event.into_runtime_output(session_id));
            }
        }
        if let Some((seq, cwd)) = self.cwd.lock().ok().and_then(|mut slot| slot.take()) {
            if let Some(event) = order.admit(WorkerOutputEvent::CwdChanged(seq, cwd)) {
                output.push(event.into_runtime_output(session_id));
            }
        }
        let lost = self.events.swap(0, Ordering::AcqRel);
        if lost > 0 {
            output.push(SessionRuntimeOutput::Backpressure(BackpressureSummary {
                source: QueueSource::SessionEvents,
                capacity,
                depth: lost,
                route: BackpressureRoute {
                    session_id: Some(session_id.clone()),
                    client_id: None,
                    subscription_id: None,
                    plugin_key: None,
                },
            }));
        }
        output
    }
}

/// Deliver a correlated reply. It answers one outstanding request, so it is
/// bounded, and its caller waits for it: it blocks for space, never drops.
fn send_correlated(
    sender: &SyncSender<WorkerChannelEvent>,
    wake_handle: &Option<SessionWakeHandle>,
    event: WorkerChannelEvent,
) {
    #[cfg(test)]
    if let Some(attempt) = CORRELATED_ATTEMPT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        let _ = attempt.send(());
    }
    if sender.send(event).is_ok() {
        notify_session_wake(wake_handle);
    }
}

/// Test seam: reports each correlated reply just before its blocking send.
#[cfg(test)]
static CORRELATED_ATTEMPT: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);

fn send_worker_event(
    sender: &SyncSender<WorkerChannelEvent>,
    overflow: &ReaderOverflow,
    stall: &EgressStall,
    wake_handle: &Option<SessionWakeHandle>,
    event: WorkerChannelEvent,
) {
    route_worker_event(sender, overflow, stall, wake_handle, event, |_| {});
}

/// Route one worker event and report each routing decision from the branch
/// that makes it.
///
/// Live PTY bytes and semantic events are not replayable. While a parent
/// consumer is attached, stall instead of dropping them. Detach must stop the
/// stall so cancel and detached workers can still make progress. Wait on the
/// drain/detach condvar; a blocking send cannot observe detach.
fn route_worker_event(
    sender: &SyncSender<WorkerChannelEvent>,
    overflow: &ReaderOverflow,
    stall: &EgressStall,
    wake_handle: &Option<SessionWakeHandle>,
    event: WorkerChannelEvent,
    mut report: impl FnMut(PtyOutputRouting),
) {
    let superseded = overflow.state_slot(&event);
    let mut event = event;
    let mut seen_seq = 0;
    let mut stalled = false;
    loop {
        // A state value holds its slot across the send and the clear. A drain
        // takes the slot under the same lock, so it takes either the older
        // value before the newer one is queued, or nothing: a stale value can
        // never be published after the newer one. The lock is released before
        // any wait, because the drain is what frees space.
        let slot = superseded.map(|slot| {
            slot.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        });
        match sender.try_send(event) {
            Ok(()) => {
                // A newer state value is queued; an older waiting one is stale.
                if let Some(mut slot) = slot {
                    #[cfg(test)]
                    pause_state_publication();
                    *slot = None;
                }
                notify_session_wake(wake_handle);
                report(PtyOutputRouting::Enqueued);
                return;
            }
            Err(TrySendError::Full(returned)) => {
                drop(slot);
                if !stall.owners_present() {
                    overflow.record_drop(returned);
                    notify_session_wake(wake_handle);
                    report(PtyOutputRouting::Dropped);
                    return;
                }
                event = returned;
                if !stalled {
                    stalled = true;
                    report(PtyOutputRouting::Stalled);
                }
                match stall.wait_for_space_or_detach(&mut seen_seq) {
                    StallWait::Retry | StallWait::Detached => {}
                    StallWait::Closed => return,
                }
            }
            Err(TrySendError::Disconnected(_)) => return,
        }
    }
}

/// Test seam: runs in `drain_output` after the first take of reader output
/// and before the completion read.
#[cfg(test)]
#[allow(clippy::type_complexity)]
static BEFORE_COMPLETION_READ: Mutex<Option<Box<dyn FnOnce(&WorkerProcessSession) + Send>>> =
    Mutex::new(None);

#[cfg(test)]
fn before_completion_read(session: &WorkerProcessSession) {
    let hook = BEFORE_COMPLETION_READ
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(hook) = hook {
        hook(session);
    }
}

/// Test seam: holds a state publication between its queued send and the
/// clear of its superseded slot.
#[cfg(test)]
static STATE_PUBLICATION_PAUSE: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>> =
    Mutex::new(None);

#[cfg(test)]
fn pause_state_publication() {
    let pause = STATE_PUBLICATION_PAUSE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some((reached, release)) = pause {
        let _ = reached.send(());
        let _ = release.recv();
    }
}

fn next_request_id(label: &str) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let ordinal = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{label}-{nanos}-{ordinal}")
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

    /// Wait until the worker can take more bytes or `timeout` passes.
    ///
    /// The stdio pipe is non-blocking, so readiness comes from `poll`. A
    /// socket write already blocked for the slice through its write timeout.
    fn wait_writable(&self, timeout: Duration) -> io::Result<()> {
        match self {
            Self::Stdio(stdin) => wait_fd_writable(stdin.as_raw_fd(), timeout),
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

    /// End the worker-facing control link from the writer's own handle.
    ///
    /// Dropping a stdio handle closes the pipe, so the worker reads EOF. A
    /// socket is shared with the session's shutdown clone, so dropping this
    /// handle alone keeps it open; shutting the write direction here reaches
    /// the worker regardless of what the parent still holds.
    fn end_link(&mut self) {
        match self {
            Self::Stdio(_) => {}
            #[cfg(unix)]
            Self::Socket(stream) => {
                let _ = stream.shutdown(Shutdown::Write);
            }
        }
    }
}

/// Block until `fd` is writable, hangs up, or `timeout` passes. The caller
/// retries the write and keeps its own deadline.
fn wait_fd_writable(fd: std::os::unix::io::RawFd, timeout: Duration) -> io::Result<()> {
    let mut poll_fd = libc::pollfd {
        fd,
        events: libc::POLLOUT,
        revents: 0,
    };
    let millis = libc::c_int::try_from(timeout.as_millis().max(1)).unwrap_or(libc::c_int::MAX);
    let result = unsafe { libc::poll(&mut poll_fd, 1, millis) };
    if result < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != ErrorKind::Interrupted {
            return Err(error);
        }
    }
    Ok(())
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
    // Every writer failure ends the worker-facing link here, at the one
    // boundary all three failure paths share (prepare, ordinary/cancel write,
    // terminal write). Queue admission is not delivery: a frame admitted
    // before the failure, a barrier cancel included, never reached the
    // worker, and the worker's control reader must observe EOF so it
    // releases whatever it still holds for this parent. This does not depend
    // on a session wake or on any pending capture state.
    if let Err(error) = write.prepare() {
        queue.seal();
        write.end_link();
        slot.set(ControlWriterOutcome::Failed {
            error: ControlWriterError::WriteError(error.to_string()),
            consumed: false,
        });
        notify_session_wake(&wake_handle);
        return;
    }
    loop {
        let Some((class, frame, freed_capacity)) = queue.pop_with_capacity_transition() else {
            slot.set(ControlWriterOutcome::Stopped);
            return;
        };
        if freed_capacity {
            // Ordinary or reserved admission reopened: wake the session so a
            // refused admitter (parked input, a pending barrier cancel) retries.
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
                write.end_link();
                slot.set(ControlWriterOutcome::Failed {
                    error,
                    consumed: false,
                });
                // Terminal-class shutdown is teardown, not session ingress:
                // no wake, but the link above is already ended.
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
                write
                    .wait_writable(slice)
                    .map_err(|error| ControlWriterError::WriteError(error.to_string()))?;
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
    use crate::runtime::control_queue::{
        ControlFrameClass, ControlQueue, ControlWriterError, ControlWriterOutcome,
        ControlWriterSlot,
    };

    use super::{
        apply_startup_failure, remove_socket_if_unchanged, run_control_writer, socket_identity,
        worker_socket_path, ProcessIdentity, SessionAdmission, SessionId, SessionReservationState,
        SessionRuntimeErrorKind, SessionSpawnRequest, WorkerProcessRuntime, WorkerWriteHalf,
        FRAME_PTY_INPUT, FRAME_SHUTDOWN, UNIX_SOCKET_PATH_MAX_BYTES,
    };

    mod lossless_events {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::mpsc;

        use super::super::{
            send_worker_event, EgressStall, QueueSource, ReaderOverflow, SessionId,
            SessionRuntimeOutput, StateOrder, WorkerChannelEvent, WorkerOutputEvent,
            STATE_PUBLICATION_PAUSE,
        };

        fn session() -> SessionId {
            SessionId("lossless-events".into())
        }

        /// A title stamped in creation order, as the reader stamps them.
        fn title(value: &str) -> WorkerChannelEvent {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            WorkerChannelEvent::Output(WorkerOutputEvent::TitleChanged(
                NEXT.fetch_add(1, Ordering::Relaxed),
                value.into(),
            ))
        }

        fn bell() -> WorkerChannelEvent {
            WorkerChannelEvent::Output(WorkerOutputEvent::Bell)
        }

        /// A one-slot channel that is already full.
        fn full_channel() -> (
            mpsc::SyncSender<WorkerChannelEvent>,
            mpsc::Receiver<WorkerChannelEvent>,
        ) {
            let (sender, receiver) = mpsc::sync_channel(1);
            sender
                .try_send(WorkerChannelEvent::Output(WorkerOutputEvent::PtyOutput(
                    b"fill".to_vec(),
                )))
                .expect("fill the slot");
            (sender, receiver)
        }

        fn events_lost(outputs: &[SessionRuntimeOutput]) -> Option<usize> {
            outputs.iter().find_map(|output| match output {
                SessionRuntimeOutput::Backpressure(summary)
                    if summary.source == QueueSource::SessionEvents =>
                {
                    Some(summary.depth)
                }
                _ => None,
            })
        }

        /// Serializes tests that install the process-wide correlated seam.
        static CORRELATED_SEAM: std::sync::Mutex<()> = std::sync::Mutex::new(());

        /// Feed one encoded frame to the real stdout reader over a channel
        /// that is already full, with no consumer attached, and wait until the
        /// reader attempts the reply while the channel is still full.
        fn reader_over_full_channel(frame: Vec<u8>) -> mpsc::Receiver<WorkerChannelEvent> {
            let (sender, receiver) = full_channel();
            let (attempt_sender, attempted) = mpsc::channel();
            *super::super::CORRELATED_ATTEMPT
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(attempt_sender);
            super::super::spawn_stdout_reader(
                std::io::Cursor::new(frame),
                sender,
                std::sync::Arc::new(ReaderOverflow::default()),
                std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                std::sync::Arc::new(std::sync::Mutex::new(None)),
                std::sync::Arc::new(std::sync::Mutex::new(
                    super::super::WorkerCompletion::default(),
                )),
                std::sync::Arc::new(EgressStall::new()),
                None,
                session(),
                None,
                None,
            );
            // timer: deadline — the reader must reach the reply; a drop never does
            let reached = attempted.recv_timeout(std::time::Duration::from_secs(5));
            *super::super::CORRELATED_ATTEMPT
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            reached.expect("the reader reached the correlated reply on a full channel");
            receiver
        }

        /// The reply behind the fill, once the fill is taken.
        fn reply_after_fill(receiver: &mpsc::Receiver<WorkerChannelEvent>) -> WorkerChannelEvent {
            // timer: deadline — the filled slot is already queued
            let fill = receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("the fill");
            assert!(matches!(
                fill,
                WorkerChannelEvent::Output(WorkerOutputEvent::PtyOutput(_))
            ));
            // timer: deadline — the correlated reply must follow; a drop never sends it
            receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("a correlated reply is never dropped")
        }

        #[test]
        fn a_screen_reply_on_a_full_channel_reaches_its_caller() {
            let _seam = CORRELATED_SEAM
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let frame = crate::contract::session_protocol::encode_json(
                super::super::FRAME_SCREEN,
                &crate::contract::session_protocol::ScreenPayload {
                    request_id: "screen-1".into(),
                    text: "hello".into(),
                    error_kind: None,
                },
            )
            .expect("encode screen reply");
            let receiver = reader_over_full_channel(frame);
            assert!(matches!(
                reply_after_fill(&receiver),
                WorkerChannelEvent::Screen(payload) if payload.request_id == "screen-1"
            ));
        }

        #[test]
        fn a_mode_flags_reply_on_a_full_channel_reaches_its_caller() {
            let _seam = CORRELATED_SEAM
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let frame = crate::contract::session_protocol::encode_json(
                super::super::FRAME_MODE_FLAGS,
                &crate::contract::session_protocol::ModeFlagsPayload {
                    request_id: "modes-1".into(),
                    mode_flags: Default::default(),
                    rows: 24,
                    cols: 80,
                    error_kind: None,
                },
            )
            .expect("encode mode-flags reply");
            let receiver = reader_over_full_channel(frame);
            assert!(matches!(
                reply_after_fill(&receiver),
                WorkerChannelEvent::ModeFlags(payload) if payload.request_id == "modes-1"
            ));
        }

        #[test]
        fn a_burst_of_bells_with_no_consumer_is_counted_never_silently_lost() {
            let (sender, _receiver) = full_channel();
            let stall = EgressStall::new();
            let overflow = ReaderOverflow::default();
            for _ in 0..5 {
                send_worker_event(&sender, &overflow, &stall, &None, bell());
            }
            let outputs = overflow.take_outputs(&session(), 1, &mut StateOrder::default());
            assert_eq!(events_lost(&outputs), Some(5), "{outputs:?}");
            assert_eq!(
                events_lost(&overflow.take_outputs(&session(), 1, &mut StateOrder::default())),
                None
            );
        }

        #[test]
        fn a_burst_of_bells_with_a_consumer_reaches_it_in_full() {
            let (sender, receiver) = full_channel();
            let stall = std::sync::Arc::new(EgressStall::new());
            stall.insert_direct().expect("attach a consumer");
            let overflow = std::sync::Arc::new(ReaderOverflow::default());
            let reader_stall = std::sync::Arc::clone(&stall);
            let reader_overflow = std::sync::Arc::clone(&overflow);
            let reader = std::thread::spawn(move || {
                for _ in 0..5 {
                    send_worker_event(&sender, &reader_overflow, &reader_stall, &None, bell());
                }
            });
            let mut bells = 0;
            while bells < 5 {
                // timer: deadline — the stalled reader must deliver every bell
                let event = receiver
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("the stalled reader delivers the next event");
                stall.note_space();
                if matches!(event, WorkerChannelEvent::Output(WorkerOutputEvent::Bell)) {
                    bells += 1;
                }
            }
            reader.join().expect("reader");
            assert_eq!(
                events_lost(&overflow.take_outputs(&session(), 1, &mut StateOrder::default())),
                None
            );
        }

        #[test]
        fn a_title_change_under_a_full_channel_converges_to_the_latest_title() {
            let (sender, receiver) = full_channel();
            let stall = EgressStall::new();
            let overflow = ReaderOverflow::default();
            for value in ["a", "b", "c"] {
                send_worker_event(&sender, &overflow, &stall, &None, title(value));
            }
            let _ = receiver.try_recv();
            let outputs = overflow.take_outputs(&session(), 1, &mut StateOrder::default());
            assert!(
                matches!(
                    outputs.as_slice(),
                    [SessionRuntimeOutput::TitleChanged { title, .. }] if title == "c"
                ),
                "{outputs:?}"
            );

            // A dropped title is superseded by a newer one the channel takes.
            send_worker_event(&sender, &overflow, &stall, &None, title("d"));
            send_worker_event(&sender, &overflow, &stall, &None, title("e"));
            let _ = receiver.try_recv();
            send_worker_event(&sender, &overflow, &stall, &None, title("f"));
            assert!(overflow
                .take_outputs(&session(), 1, &mut StateOrder::default())
                .is_empty());
            assert!(matches!(
                receiver.try_recv(),
                Ok(WorkerChannelEvent::Output(WorkerOutputEvent::TitleChanged(_, title))) if title == "f"
            ));
        }

        /// A drain between a newer title's queued send and the clear of the
        /// older slot must not publish the older title after the newer one:
        /// the reader holds the slot across its send and its clear.
        #[test]
        fn a_drain_between_a_queued_title_and_its_slot_clear_cannot_regress_the_title() {
            let (sender, receiver) = full_channel();
            let stall = EgressStall::new();
            let overflow = std::sync::Arc::new(ReaderOverflow::default());
            // "old" is dropped into the slot; then the channel has room.
            send_worker_event(&sender, &overflow, &stall, &None, title("old"));
            let _ = receiver.try_recv();

            let (reached_tx, reached_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            *STATE_PUBLICATION_PAUSE
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((reached_tx, release_rx));
            let reader = {
                let overflow = std::sync::Arc::clone(&overflow);
                std::thread::spawn(move || {
                    send_worker_event(&sender, &overflow, &stall, &None, title("new"));
                })
            };
            // timer: deadline — the reader must reach its paused publication
            reached_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("the reader queued the newer title");

            // "new" is queued and the reader has not cleared the slot yet. A
            // drain takes the slot under its lock, so it must wait here.
            assert!(
                overflow.title.try_lock().is_err(),
                "a drain could take the stale slot between the queued title and its clear"
            );
            release_tx.send(()).expect("release the reader");
            reader.join().expect("reader");
            assert!(matches!(
                receiver.try_recv(),
                Ok(WorkerChannelEvent::Output(WorkerOutputEvent::TitleChanged(_, title))) if title == "new"
            ));
            assert!(
                overflow
                    .take_outputs(&session(), 1, &mut StateOrder::default())
                    .is_empty(),
                "the superseded title must not be published"
            );
        }

        /// A drain pumps the channel, then takes overflow. Between the two,
        /// the reader queues title A and then records the newer title B in
        /// overflow (the one-slot channel is full). The drain emits B; the
        /// next drain's pump finds A. A is older than B, so it is dropped:
        /// the title never regresses. The same holds for cwd.
        #[test]
        fn a_queued_title_older_than_an_emitted_overflow_title_is_dropped() {
            let (sender, receiver) = mpsc::sync_channel(1);
            let stall = EgressStall::new();
            let overflow = ReaderOverflow::default();
            let mut order = StateOrder::default();
            let mut emitted = Vec::new();
            let mut pending = std::collections::VecDeque::new();
            let take = |pending: &mut std::collections::VecDeque<WorkerOutputEvent>,
                        order: &mut StateOrder,
                        emitted: &mut Vec<String>| {
                let mut output = Vec::new();
                while let Some(event) = pending.pop_front() {
                    if let Some(event) = order.admit(event) {
                        output.push(event.into_runtime_output(&session()));
                    }
                }
                output.extend(overflow.take_outputs(&session(), 1, order));
                emitted.extend(output.into_iter().filter_map(|output| match output {
                    SessionRuntimeOutput::TitleChanged { title, .. } => Some(title),
                    _ => None,
                }));
            };

            // The first drain's pump finds the channel empty.
            while let Ok(WorkerChannelEvent::Output(event)) = receiver.try_recv() {
                pending.push_back(event);
            }
            // The reader queues A, then B finds the channel full.
            send_worker_event(&sender, &overflow, &stall, &None, title("A"));
            send_worker_event(&sender, &overflow, &stall, &None, title("B"));
            // The first drain takes overflow: B.
            take(&mut pending, &mut order, &mut emitted);
            // The next drain pumps A out of the channel.
            while let Ok(WorkerChannelEvent::Output(event)) = receiver.try_recv() {
                pending.push_back(event);
            }
            take(&mut pending, &mut order, &mut emitted);

            assert_eq!(
                emitted,
                vec!["B".to_string()],
                "the title must not regress to A"
            );
        }
    }

    /// A drop the reader records after `drain_output` took the overflow, but
    /// before the exit it then reads, is reported before the session is
    /// removed, never lost with it.
    #[test]
    fn overflow_recorded_just_before_the_exit_is_reported_before_removal() {
        use crate::runtime::SessionRuntime;

        let path = Path::new("/tmp").join(format!(
            "botster-exit-overflow-{}-{}.sock",
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
                session_uuid: "exit-overflow".to_string(),
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

        let session_id = SessionId("exit-overflow".to_string());
        let mut runtime = WorkerProcessRuntime::new("/missing/worker");
        runtime
            .adopt_session(
                session_id.clone(),
                ProcessIdentity {
                    pid: Some(std::process::id()),
                    runtime_id: Some("exit-overflow".to_string()),
                },
                &path,
                false,
            )
            .expect("adopt the fake worker");

        // Between the first take and the completion read, the reader records
        // one PTY drop and then the exit, as its thread does in order.
        *super::BEFORE_COMPLETION_READ
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Box::new(|session| {
            session
                .overflow
                .record_drop(super::WorkerChannelEvent::Output(
                    super::WorkerOutputEvent::PtyOutput(b"lost".to_vec()),
                ));
            session
                .completion
                .lock()
                .expect("completion")
                .process_exited = Some(crate::ProcessExitedPayload {
                exit_code: Some(0),
                signal: None,
            });
        }));
        let output = runtime.drain_output(&session_id).expect("drain");
        assert!(
            output.iter().any(|event| matches!(
                event,
                super::SessionRuntimeOutput::Backpressure(summary)
                    if summary.source == super::QueueSource::SessionIo
            )),
            "the final PTY drop must be reported: {output:?}"
        );
        assert!(output
            .iter()
            .any(|event| matches!(event, super::SessionRuntimeOutput::ProcessExited { .. })));

        close_sender.send(()).expect("close worker server");
        server.join().expect("worker server");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_full_stdio_pipe_waits_for_the_reader_to_drain() {
        use std::process::{Command, Stdio};

        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cat");
        let mut write = WorkerWriteHalf::Stdio(child.stdin.take().expect("piped stdin"));
        write.prepare().expect("non-blocking stdin");
        // Larger than a pipe buffer, so the writer meets a full pipe.
        let payload = vec![b'x'; 1024 * 1024];

        super::write_control_bytes(&mut write, &payload).expect("the reader drains the pipe");

        drop(write);
        child.wait().expect("cat exits at EOF");
    }

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
    fn a_launched_worker_queued_in_a_dropped_channel_is_discarded() {
        use std::process::{Command, Stdio};
        use std::sync::atomic::Ordering;

        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleeper");
        let stdin = child.stdin.take().expect("piped stdin");
        let pid = child.id();
        let launched = super::LaunchedWorker::new(super::LaunchedParts {
            admission: None,
            wake_handle: None,
            child,
            control: super::WorkerControl::Stdio(stdin),
            reader: Box::new(std::io::empty()),
            metadata: crate::SessionMetadata {
                session_uuid: "guard".to_string(),
                pid,
                rows: 24,
                cols: 80,
                last_output_at: 0,
                title: None,
                cwd: None,
                port: None,
                mode_flags: Default::default(),
                recovery_identity: None,
            },
            process: ProcessIdentity {
                pid: Some(pid),
                runtime_id: None,
            },
            supports_snapshot_boundary: true,
        });
        let before = super::LAUNCHED_WORKER_DISCARDS.load(Ordering::Acquire);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        sender.send(launched).expect("queue the guard");

        // The receiver goes away with the guard still queued: the send had
        // already succeeded, so only the guard's own drop can clean up.
        drop(receiver);
        drop(sender);

        assert_eq!(
            super::LAUNCHED_WORKER_DISCARDS.load(Ordering::Acquire),
            before + 1,
            "dropping a queued launch must run its cleanup"
        );
    }

    #[test]
    fn a_barrier_cancel_uses_a_reserved_slot_and_stays_outstanding_when_refused() {
        let mut runtime = WorkerProcessRuntime::new("/missing/botster-session-worker");
        let session = SessionId("saturated".to_string());
        runtime.insert_test_session(session.clone());
        let request_id = runtime
            .begin_snapshot_boundary(&session)
            .expect("begin barrier");
        let queue = runtime.test_control_queue(&session).expect("session queue");
        // Ordinary capacity is full, yet the reserved slots stay usable.
        while queue.admit(ControlFrameClass::Ordinary, vec![0]).is_ok() {}
        assert_eq!(
            runtime
                .cancel_snapshot_boundary(&session, &request_id)
                .expect("cancel with reserved slot"),
            super::SnapshotCancelAdmission::Accepted
        );
        assert!(
            !runtime.snapshot_request_is_outstanding(&session, &request_id),
            "an accepted cancel clears the outstanding request"
        );

        // Drain one ordinary slot so a new barrier can begin, then occupy
        // every reserved slot as well.
        assert!(queue.pop().is_some());
        let request_id = runtime
            .begin_snapshot_boundary(&session)
            .expect("begin a second barrier");
        while queue.admit(ControlFrameClass::Cancel, vec![0]).is_ok() {}
        assert_eq!(
            runtime
                .cancel_snapshot_boundary(&session, &request_id)
                .expect("refusal is not an error"),
            super::SnapshotCancelAdmission::QueueFull
        );
        let refused = runtime
            .begin_snapshot_boundary(&session)
            .expect_err("the barrier is still outstanding");
        assert!(refused.message.contains("already in flight"));

        // One drained slot lets the retry through.
        assert!(queue.pop().is_some());
        assert_eq!(
            runtime
                .cancel_snapshot_boundary(&session, &request_id)
                .expect("retry"),
            super::SnapshotCancelAdmission::Accepted
        );
        assert!(runtime.begin_snapshot_boundary(&session).is_ok());
    }

    fn wakes_after_writer_drains(ordinary: usize, cancel: usize) -> bool {
        let source = TerminalWakeSource::new();
        let session = SessionId("control-writer-capacity".to_string());
        let handle = source.session_handle(session.clone());
        let queue = ControlQueue::new();
        for _ in 0..ordinary {
            let frame = crate::encode_frame(FRAME_PTY_INPUT, b"x").expect("frame");
            queue
                .admit(ControlFrameClass::Ordinary, frame)
                .expect("admit");
        }
        for _ in 0..cancel {
            let frame =
                crate::encode_frame(super::FRAME_INPUT_CANCEL, &0u64.to_le_bytes()).expect("frame");
            queue
                .admit(ControlFrameClass::Cancel, frame)
                .expect("admit");
        }
        let (writer, mut peer) = UnixStream::pair().expect("socket pair");
        let sink = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(read) = std::io::Read::read(&mut peer, &mut buf) {
                if read == 0 {
                    break;
                }
            }
        });
        let write_queue = queue.clone();
        let writer_thread = std::thread::spawn(move || {
            run_control_writer(
                write_queue,
                WorkerWriteHalf::Socket(writer),
                ControlWriterSlot::running(),
                Some(handle),
            );
        });
        // Let the writer drain everything, then seal so it exits.
        while !queue.is_empty() {
            std::thread::yield_now();
        }
        queue.seal();
        writer_thread.join().expect("writer");
        sink.join().expect("sink");
        let batch = source.wait_wakes(Duration::from_millis(0));
        batch.ingress_sessions.iter().any(|id| id == &session)
    }

    #[test]
    fn draining_reserved_frames_from_a_full_queue_wakes_the_session() {
        assert!(wakes_after_writer_drains(
            20,
            crate::runtime::WORKER_CONTROL_QUEUE_FRAMES - 20
        ));
    }

    #[test]
    fn draining_a_queue_that_was_never_full_wakes_nobody() {
        assert!(!wakes_after_writer_drains(3, 1));
    }

    #[test]
    fn failing_the_control_plane_ends_the_link_and_drops_the_barrier_request() {
        let mut runtime = WorkerProcessRuntime::new("/missing/botster-session-worker");
        let session = SessionId("failed-plane".to_string());
        runtime.insert_test_session(session.clone());
        let request_id = runtime
            .begin_snapshot_boundary(&session)
            .expect("begin barrier");
        assert!(runtime.snapshot_request_is_outstanding(&session, &request_id));

        runtime.fail_control_plane(&session, super::ControlWriterError::DeadlineExpired);

        assert!(!runtime.snapshot_request_is_outstanding(&session, &request_id));
        assert_eq!(
            runtime.control_plane_state(&session),
            super::ControlPlaneState::Failed(super::ControlWriterError::DeadlineExpired)
        );
        assert!(runtime
            .test_control_queue(&session)
            .expect("queue")
            .is_sealed());
        assert!(matches!(
            runtime.cancel_snapshot_boundary(&session, &request_id),
            Ok(super::SnapshotCancelAdmission::Sealed)
        ));
    }

    #[test]
    fn failing_the_control_plane_is_exactly_once_and_shuts_the_socket_link() {
        use std::io::Read;

        let mut runtime = WorkerProcessRuntime::new("/missing/botster-session-worker");
        let session = SessionId("socket-plane".to_string());
        let mut peer = runtime.insert_test_socket_session(session.clone());
        peer.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("bounded read");
        let request_id = runtime
            .begin_snapshot_boundary(&session)
            .expect("begin barrier");
        assert_eq!(
            runtime
                .cancel_snapshot_boundary(&session, &request_id)
                .expect("cancel"),
            super::SnapshotCancelAdmission::Accepted
        );

        runtime.fail_control_plane(&session, super::ControlWriterError::PeerClosed);
        // A second failure report must not restate the cause or re-run cleanup.
        runtime.fail_control_plane(&session, super::ControlWriterError::DeadlineExpired);

        assert_eq!(
            runtime.control_plane_state(&session),
            super::ControlPlaneState::Failed(super::ControlWriterError::PeerClosed)
        );
        let mut buf = [0u8; 8];
        assert_eq!(
            peer.read(&mut buf).expect("read within the bound"),
            0,
            "the worker end observes EOF"
        );
    }

    /// Run the writer against a peer that never reads, so the write deadline
    /// fails the writer mid-frame. Returns the writer outcome and whether the
    /// peer then observed end of stream within a bound.
    fn writer_deadline_failure_ends_link(
        class: ControlFrameClass,
        frame_type: u8,
    ) -> (ControlWriterOutcome, bool) {
        use std::io::Read;

        let queue = ControlQueue::new();
        // Far larger than any socket buffer: the write cannot complete.
        let frame = crate::encode_frame(frame_type, &vec![0u8; 4 * 1024 * 1024]).expect("frame");
        queue.admit(class, frame).expect("admit");
        let (writer, mut peer) = UnixStream::pair().expect("socket pair");
        // Bound the peer read before the writer can touch the socket.
        peer.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("bounded read");
        let slot = ControlWriterSlot::running();
        let writer_slot = slot.clone();
        let writer_thread = std::thread::spawn(move || {
            run_control_writer(queue, WorkerWriteHalf::Socket(writer), writer_slot, None);
        });
        writer_thread.join().expect("writer thread");
        let outcome = slot.get();
        let mut buf = vec![0u8; 64 * 1024];
        let mut saw_eof = false;
        loop {
            match peer.read(&mut buf) {
                Ok(0) => {
                    saw_eof = true;
                    break;
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        (outcome, saw_eof)
    }

    #[test]
    fn an_ordinary_write_failure_ends_the_worker_link_at_the_writer() {
        let (outcome, saw_eof) =
            writer_deadline_failure_ends_link(ControlFrameClass::Ordinary, FRAME_PTY_INPUT);
        assert!(matches!(
            outcome,
            ControlWriterOutcome::Failed {
                error: ControlWriterError::DeadlineExpired,
                ..
            }
        ));
        assert!(saw_eof, "the worker end must observe EOF after the failure");
    }

    #[test]
    fn a_terminal_write_failure_ends_the_worker_link_without_a_wake() {
        let (outcome, saw_eof) =
            writer_deadline_failure_ends_link(ControlFrameClass::Terminal, FRAME_SHUTDOWN);
        assert!(matches!(
            outcome,
            ControlWriterOutcome::Failed {
                error: ControlWriterError::DeadlineExpired,
                ..
            }
        ));
        assert!(saw_eof, "shutdown teardown must still end the link");
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
    fn adoption_starts_without_trusted_welcome_modes() {
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
            runtime.latest_modes(&session_id),
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

    fn spawn_request(session: &str, request: &str) -> SessionSpawnRequest {
        SessionSpawnRequest {
            request_id: crate::RequestId(request.to_string()),
            session_id: SessionId(session.to_string()),
            executable: "sh".to_string(),
            arguments: vec!["-c".to_string(), "exit 0".to_string()],
            working_directory: crate::SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: crate::SpawnEnvironment::default(),
            initial_pty_size: None,
        }
    }

    #[test]
    fn stale_spf1_cannot_mutate_a_newer_reservation() {
        let admission = SessionAdmission::synchronous();
        let request = spawn_request("spf1-stale", "req-1");
        let first = admission
            .reserve(request.session_id.clone())
            .expect("first reservation");
        admission
            .begin_launch(&first, first.session_id(), None)
            .expect("launch first");
        first.creation_possible();
        let report = crate::StartupFailureReport {
            request_id: request.request_id.0.clone(),
            session_id: request.session_id.0.clone(),
            worker_pid: 7,
            message: "not created".to_string(),
            outcome: crate::StartupFailureOutcome::NotCreated,
        };
        apply_startup_failure(Some(&first), &request, 7, &report);
        assert!(!first.startup_created_child());
        admission.release(&first).expect("release first");
        let second = admission
            .reserve(request.session_id.clone())
            .expect("second generation");
        apply_startup_failure(Some(&first), &request, 7, &report);
        first.cleanup_unconfirmed();
        first.runtime_ended();
        assert_eq!(second.state(), SessionReservationState::Reserved);
        assert_eq!(first.state(), SessionReservationState::Released);
    }

    #[test]
    fn spf1_identity_mismatch_is_unknown() {
        let admission = SessionAdmission::synchronous();
        let request = spawn_request("spf1-mismatch", "req-2");
        let reservation = admission
            .reserve(request.session_id.clone())
            .expect("reserve");
        admission
            .begin_launch(&reservation, reservation.session_id(), None)
            .expect("launch");
        reservation.creation_possible();
        let report = crate::StartupFailureReport {
            request_id: request.request_id.0.clone(),
            session_id: request.session_id.0.clone(),
            worker_pid: 1,
            message: "mismatch".to_string(),
            outcome: crate::StartupFailureOutcome::NotCreated,
        };
        apply_startup_failure(Some(&reservation), &request, 9, &report);
        assert_eq!(
            reservation.execution_state(),
            SessionReservationState::CleanupUnconfirmed
        );
    }
}
