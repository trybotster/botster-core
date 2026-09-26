//! Core daemon supervisor and typed API implementation.

#[cfg(test)]
use std::cell::Cell;
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    hash::{Hash, Hasher},
    io,
    ops::Bound::{Excluded, Included, Unbounded},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::Arc,
    time::{Duration, Instant},
    time::{SystemTime, UNIX_EPOCH},
};

use botster_core::contract::terminal_adapter::TerminalRouteCloseReason;
use botster_core::contract::terminal_wake::{
    TerminalWakeBatch, TerminalWakeSource, TerminalWakeWait, WakingTerminalAdapter,
};
use botster_core::engine::multiplexer::MultiplexerEngineError;
use botster_core::runtime::{
    ReservedSessionSpawnError, SessionReservation, SessionReservationRefusal,
    SessionReservationRelease,
};
use botster_core::TerminalScreenSize;
use botster_core::{
    BindTerminalAdapterError, BotsterEngineObservation, BotsterEngineOutput, ClientId, CoreSession,
    DefaultBotsterEngine, DefaultBotsterEngineError, DetachTerminalSubscriptionResult, EnvelopeId,
    EnvelopeTarget, ModeFlags, NotificationId, NotificationInbox, QueueSource, RequestId,
    ResizeAckHold, ResizePayload, RoutedEnvelopeQueueConfig, RoutedEnvelopeRouter, ScreenReady,
    SessionId, SessionIoEvent, SessionLifecycleState, SessionRuntimeError, SessionRuntimeErrorKind,
    SessionWorkerHealthReason, SessionWorkerStaleReason, SubscriptionId, TerminalBackendError,
    TerminalCapabilitySet, TerminalColorProfile, TerminalSubscriptionGeneration,
    TerminalSubscriptionInventory, TerminalSubscriptionInventoryError, TransportEgress,
    WorkerBackedBotsterEngine, WorkerProcessRuntimeOptions,
};
use botster_terminal_ghostty::{GhosttyAdapterConfig, GhosttyTerminal, GhosttyTerminalError};
use botster_terminal_protocol::HistoryUnavailableReason;
use thiserror::Error;

use crate::operation::{
    CaptureId, CaptureOwner, ModeFlagsReadback, ReservedSpawnResult, RetainedTerminal,
    ScreenReadback, SnapshotCapture, SnapshotPage, CAPTURE_IDLE_TTL_SECONDS,
    MAX_OPEN_CAPTURES_PER_CLIENT, MAX_PENDING_READBACKS_PER_SESSION, MAX_PENDING_SPAWNS,
    SNAPSHOT_PAGE_BYTES,
};
use crate::wake_pump::{WakePumpControl, WakePumpError, WakePumpState, WakePumpWait};

use crate::api::{
    reserved_observe_slice_error, sanitize_observe_slice_error_message,
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest, AttachedSession,
    CaptureSnapshotRequest, DaemonHealth, DaemonSession, DaemonStatus, DrainNotificationsRequest,
    DrainNotificationsResult, DrainResult, DrainRoutedEnvelopesRequest, DrainRoutedEnvelopesResult,
    GuardedWriteRequest, GuardedWriteResult, LifecycleBaselineBudget, NotificationStatusResult,
    ObserveLifecycleBudget, ObserveLifecycleCursor, ObserveLifecyclePassId, ObserveLifecycleSlice,
    ObserveLifecycleSliceError, PostNotificationRequest, PostNotificationResult,
    PublishRoutedEnvelopeRequest, PublishRoutedEnvelopeResult, PumpWokenOutcome,
    ReadModeFlagsRequest, ReadScreenRequest, RoutedEnvelopeDeliveryStateResult,
    SessionAdoptionReport, SessionAdoptionState, SessionLifecycleBaseline,
    SessionLifecycleBaselinePage, SessionLifecycleChange, SessionLifecycleChangeKind,
    SessionLifecycleChanges, SessionLifecycleCursor, SessionLifecycleLookup, SessionLifecyclePage,
    SessionLifecyclePageError, SessionLifecycleRecord, SessionLifecycleResyncReason,
    SessionLifecycleSourceId, SessionRegistryStateLookup, SpawnSessionRequest,
};
use crate::guarded_write::{decide_guarded_write, GuardedWriteDecision, GuardedWriteDeliveryState};
use crate::registry::{
    command_label, RegistryRecord, RegistrySessionState, SessionRegistry, SessionRegistryError,
};

/// Default Ghostty scrollback page-allocation byte budget for daemon sessions.
///
/// Ghostty quantizes this budget into terminal pages, so effective retained
/// lines depend on terminal width. At this 10 MB budget, warm 24x80 sessions
/// currently converge near a 9.0 MiB opaque snapshot frame per attaching client
/// after scrollback saturation.
pub const DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES: usize = 10_000_000;

/// Default number of ordered lifecycle changes retained for replay.
pub const DEFAULT_LIFECYCLE_JOURNAL_CAPACITY: usize = 1_024;

/// Default bound for a correlated worker reply behind a pending operation.
pub const DEFAULT_WORKER_REPLY_TIMEOUT: Duration = botster_core::DEFAULT_WORKER_REPLY_TIMEOUT;

/// Default retention policy for ended-session terminal history.
///
/// Hub supplies production values; these defaults keep a small daemon bounded.
pub const DEFAULT_RETENTION_POLICY: RetentionPolicy = RetentionPolicy {
    max_object_bytes: 16 * 1024 * 1024,
    max_total_bytes: 256 * 1024 * 1024,
    max_sessions: 256,
};

/// Bound for an orderly session shutdown behind a pending operation.
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(2);

const TERMINAL_COMMIT_REARM_LIMIT: u8 = 3;

static NEXT_LIFECYCLE_SOURCE_ORDINAL: AtomicU64 = AtomicU64::new(1);
static NEXT_OBSERVE_PASS_ORDINAL: AtomicU64 = AtomicU64::new(1);

/// Daemon configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreDaemonConfig {
    /// Caller-chosen data directory for registry metadata.
    pub data_dir: PathBuf,
    /// Logical daemon client queue capacity.
    pub client_queue_capacity: usize,
    /// Optional worker process executable for worker-backed durable sessions.
    pub worker_path: Option<PathBuf>,
    /// Bounded per-target routed-envelope queue settings.
    pub routed_envelope_queue: RoutedEnvelopeQueueConfig,
    /// Ghostty scrollback page-allocation byte budget for each daemon session.
    pub ghostty_max_scrollback_bytes: usize,
    /// Maximum ordered lifecycle changes retained for slow consumers.
    pub lifecycle_journal_capacity: usize,
    /// Optional host-supplied terminal color profile for new Ghostty sessions.
    ///
    /// This is a policy-free configuration seam: `CoreDaemon` does not invent
    /// presentation defaults. Hosts outside this repository supply color policy
    /// when OSC 10/11/12 replies or palette defaults are required.
    pub terminal_color_profile: Option<TerminalColorProfile>,
    /// Bound for correlated worker replies behind pending operations.
    pub worker_reply_timeout: Duration,
    /// Retention policy for ended-session terminal history.
    pub retention: RetentionPolicy,
    /// Test-only: hold after PTY read while still in the reader critical section.
    pub test_hold_after_read_ms: Option<u64>,
    /// Test-only: force write WouldBlock until this Unix ms.
    pub test_write_block_until_unix_ms: Option<u64>,
    /// Test-only: cap each write() to this many bytes (partial-write proofs).
    pub test_write_max_chunk: Option<usize>,
    /// Test-only: single-queue fence capacity override (overflow proofs).
    pub test_pending_capacity: Option<usize>,
    /// Test-only: hold after fence enqueue while still under the critical fence.
    pub test_hold_after_enqueue_ms: Option<u64>,
    /// Retained PTY reader chunks inside the worker process (tests may set 1).
    pub pty_reader_chunk_capacity: Option<usize>,
    /// Test-only parent worker egress capacity.
    pub test_worker_egress_capacity: Option<usize>,
    /// Test-only: hold parent-side resize acknowledgements for one worker session.
    pub test_resize_ack_hold: Option<ResizeAckHold>,
    /// Test-only: hold after FRAME_PROCESS_EXITED with stdout still open.
    pub test_hold_before_exit_ms: Option<u64>,
    /// Test-only: worker process exit code after the payload is flushed.
    pub test_exit_code: Option<i32>,
    /// Test-only: add this duration after each counted baseline step.
    #[cfg(test)]
    pub test_baseline_elapsed_per_op: Option<Duration>,
}

impl CoreDaemonConfig {
    /// Build a config with the default bounded client queue capacity.
    #[must_use]
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            client_queue_capacity: QueueSource::ClientWorker.default_capacity(),
            worker_path: None,
            routed_envelope_queue: RoutedEnvelopeQueueConfig::default(),
            ghostty_max_scrollback_bytes: DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES,
            lifecycle_journal_capacity: DEFAULT_LIFECYCLE_JOURNAL_CAPACITY,
            terminal_color_profile: None,
            worker_reply_timeout: DEFAULT_WORKER_REPLY_TIMEOUT,
            retention: DEFAULT_RETENTION_POLICY,
            test_hold_after_read_ms: None,
            test_write_block_until_unix_ms: None,
            test_write_max_chunk: None,
            test_pending_capacity: None,
            test_hold_after_enqueue_ms: None,
            pty_reader_chunk_capacity: None,
            test_worker_egress_capacity: None,
            test_resize_ack_hold: None,
            test_hold_before_exit_ms: None,
            test_exit_code: None,
            #[cfg(test)]
            test_baseline_elapsed_per_op: None,
        }
    }

    /// Override the correlated worker reply bound.
    #[must_use]
    pub const fn with_worker_reply_timeout(mut self, timeout: Duration) -> Self {
        self.worker_reply_timeout = timeout;
        self
    }

    /// Supply the host retention policy for ended-session history.
    #[must_use]
    pub const fn with_retention_policy(mut self, policy: RetentionPolicy) -> Self {
        self.retention = policy;
        self
    }

    /// Set the test-only after-read publication hold for unpublished-chunk proofs.
    #[must_use]
    pub const fn with_test_hold_after_read_ms(mut self, hold_ms: Option<u64>) -> Self {
        self.test_hold_after_read_ms = hold_ms;
        self
    }

    /// Set the test-only write backpressure bound for deadline proofs.
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

    /// Override worker PTY reader chunk capacity (single-queue capacity proofs).
    #[must_use]
    pub const fn with_pty_reader_chunk_capacity(mut self, capacity: Option<usize>) -> Self {
        self.pty_reader_chunk_capacity = capacity;
        self
    }

    /// Set the test-only parent worker egress capacity.
    #[must_use]
    pub const fn with_test_worker_egress_capacity(mut self, capacity: Option<usize>) -> Self {
        self.test_worker_egress_capacity = capacity;
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

    /// Expire baseline elapsed after this many counted index, load, clone, or encode steps.
    #[cfg(test)]
    #[must_use]
    pub const fn with_test_baseline_elapsed_per_op(mut self, per_op: Duration) -> Self {
        self.test_baseline_elapsed_per_op = Some(per_op);
        self
    }

    /// Set test-only single-queue fence capacity for overflow proofs.
    #[must_use]
    pub const fn with_test_pending_capacity(mut self, capacity: Option<usize>) -> Self {
        self.test_pending_capacity = capacity;
        self
    }

    /// Set test-only post-enqueue hold while still under the admission fence.
    #[must_use]
    pub const fn with_test_hold_after_enqueue_ms(mut self, hold_ms: Option<u64>) -> Self {
        self.test_hold_after_enqueue_ms = hold_ms;
        self
    }

    /// Use worker process backed sessions through the supplied worker executable.
    #[must_use]
    pub fn with_worker_path(mut self, worker_path: impl Into<PathBuf>) -> Self {
        self.worker_path = Some(worker_path.into());
        self
    }

    /// Use explicit per-target routed-envelope queue settings.
    #[must_use]
    pub const fn with_routed_envelope_queue(mut self, config: RoutedEnvelopeQueueConfig) -> Self {
        self.routed_envelope_queue = config;
        self
    }

    /// Use an explicit Ghostty scrollback page-allocation byte budget.
    #[must_use]
    pub const fn with_ghostty_max_scrollback_bytes(mut self, max_bytes: usize) -> Self {
        self.ghostty_max_scrollback_bytes = max_bytes;
        self
    }

    /// Retain at most this many lifecycle changes for cursor replay.
    #[must_use]
    pub const fn with_lifecycle_journal_capacity(mut self, capacity: usize) -> Self {
        self.lifecycle_journal_capacity = capacity;
        self
    }

    /// Supply a host-owned terminal color profile for new Ghostty sessions.
    ///
    /// Callers outside this repository own presentation policy. Passing a
    /// profile is required for pre-attach OSC 10/11/12 replies that need
    /// configured default colors.
    #[must_use]
    pub fn with_terminal_color_profile(mut self, profile: TerminalColorProfile) -> Self {
        self.terminal_color_profile = Some(profile);
        self
    }
}

/// Daemon API error.
#[derive(Debug, Error)]
pub enum CoreDaemonError {
    /// The reservation operation refused without starting a PTY.
    #[error("session reservation refused: {0:?}")]
    SessionReservation(SessionReservationRefusal),
    /// Core engine error.
    #[error(transparent)]
    Engine(#[from] DefaultBotsterEngineError),
    /// Registry error.
    #[error(transparent)]
    Registry(#[from] SessionRegistryError),
    /// Session id was not found.
    #[error("unknown session: {0:?}")]
    UnknownSession(SessionId),
    /// Session exists but no longer accepts terminal readback.
    #[error("session is not readable: {0:?}")]
    SessionNotReadable(SessionId),
    /// Adoption requires a configured session-worker executable.
    #[error(
        "missing worker path: restart-durable adoption requires CoreDaemonConfig::with_worker_path(...) pointing at botster-session-worker"
    )]
    MissingWorkerPath,
    /// Daemon has shut down.
    #[error("daemon is shut down")]
    Shutdown,
    /// Core did not return the expected screen response.
    ///
    /// This is a defensive guard for future session-event routing changes after
    /// daemon readability checks have accepted the request.
    #[error("screen response missing for request: {0:?}")]
    MissingScreenResponse(RequestId),
    /// Core did not return the expected mode-flags response.
    #[error("mode flags response missing for request: {0:?}")]
    MissingModeFlagsResponse(RequestId),
    /// Bind rejected a terminal adapter.
    #[error(transparent)]
    BindTerminalAdapter(#[from] BindTerminalAdapterError),
    /// The session control plane has failed and admits no new owner.
    #[error("control plane failed for session {0:?}")]
    ControlPlaneFailed(SessionId),
    /// Wake pump lifecycle error.
    #[error(transparent)]
    WakePump(#[from] WakePumpError),
    /// Explicit resize is rejected while ingress resizes remain pending.
    #[error("explicit resize busy: pending ingress resize for session {0:?}")]
    ExplicitResizeBusy(SessionId),
    /// `begin` refused a new operation because a pending limit is reached.
    #[error("pending operation limit reached: {0:?}")]
    PendingLimit(crate::operation::PendingLimitKind),
    /// The operation deadline passed before the worker replied.
    #[error("pending operation deadline expired")]
    DeadlineExpired,
    /// The operation was cancelled by the host before it completed.
    #[error("pending operation cancelled")]
    Cancelled,
    /// The worker link failed while the operation was in flight.
    #[error("worker link failed for session {0:?}")]
    WorkerLinkFailed(SessionId),
    /// Unknown or expired snapshot capture.
    #[error("unknown snapshot capture: {0:?}")]
    UnknownCapture(crate::operation::CaptureId),
    /// Snapshot page index is out of range.
    #[error("snapshot page {page} out of range for capture {capture:?}")]
    SnapshotPageOutOfRange {
        /// Capture that was paged.
        capture: crate::operation::CaptureId,
        /// Requested page.
        page: u32,
    },
}

pub use crate::operation::{
    CoreCompletion, CoreOperation, PendingOperationId, RetentionAccounting, RetentionPolicy,
};

/// One session's retained error from a control-plane observe tick.
#[derive(Debug)]
pub struct ObserveLifecycleSessionError {
    /// Session whose observe step failed.
    pub session_id: SessionId,
    /// Drain, persist, or reconcile error for this session.
    pub error: CoreDaemonError,
}

/// Control-plane result of one [`CoreDaemon::observe_lifecycle`] tick.
///
/// This type carries no terminal bytes, phases, snapshots, attach state, or
/// `ProcessExited` frames. Per-session errors do not abort the remaining pass.
#[derive(Debug, Default)]
pub struct ObserveLifecycleResult {
    /// Errors retained after every live session was attempted.
    pub session_errors: Vec<ObserveLifecycleSessionError>,
}
/// Production core daemon supervisor.
///
/// `CoreDaemon` is intentionally not transferable between threads. A host that
/// needs a data-plane thread must construct and keep the daemon on that thread.
///
/// ```compile_fail
/// use botster_core_daemon::{CoreDaemon, CoreDaemonConfig};
///
/// let daemon = CoreDaemon::new(CoreDaemonConfig::new("."));
/// std::thread::spawn(move || drop(daemon));
/// ```
pub struct CoreDaemon {
    config: CoreDaemonConfig,
    registry: SessionRegistry,
    engine: DaemonEngine,
    notification_inbox: NotificationInbox,
    envelope_router: RoutedEnvelopeRouter,
    pending_drain: Vec<PendingDrainResult>,
    /// Retained ended-session terminal state under the retention policy.
    retained_terminal: HashMap<SessionId, Arc<RetainedTerminal>>,
    /// Ended sessions whose history is not retained, with the reason.
    retained_unavailable: HashMap<SessionId, HistoryUnavailableReason>,
    retention_accounting: RetentionAccounting,
    terminal_commit_obligations: HashMap<SessionId, SessionLifecycleState>,
    terminal_commit_failures: HashMap<SessionId, u8>,
    acknowledged_terminal_inventory_revision: u64,
    lifecycle_source_id: SessionLifecycleSourceId,
    lifecycle_sequence: u64,
    lifecycle_journal: VecDeque<SessionLifecycleChange>,
    journal_advanced: bool,
    observe_pass: Option<ObservePassState>,
    observe_live_sessions: BTreeMap<String, u64>,
    observe_live_generation: u64,
    baseline_freeze: Option<BaselineFreeze>,
    next_pending_operation: u64,
    pending: BTreeMap<PendingOperationId, PendingState>,
    completions: Vec<CoreCompletion>,
    open_captures: HashMap<CaptureId, OpenCapture>,
    next_capture: u64,
    #[cfg(test)]
    observe_index_scans: u64,
    #[cfg(test)]
    baseline_index_scans: u64,
    #[cfg(test)]
    baseline_row_copies: u64,
    #[cfg(test)]
    baseline_page_encodes: u64,
    #[cfg(test)]
    registry_load_all_calls: Cell<u64>,
    running: bool,
    wake_pump: Option<WakePumpState>,
}

struct PendingState {
    kind: PendingKind,
    deadline: Option<Instant>,
    /// The host cancelled this operation; the completion was already
    /// emitted. The entry stays until the runtime finished the work it owns.
    cancelled: bool,
}

enum PendingKind {
    Spawn {
        reservation: Option<SessionReservation>,
        session_id: SessionId,
        metadata: botster_core::CoreSessionMetadata,
        size: ResizePayload,
        label: String,
        now_seconds: u64,
    },
    ShutdownSession {
        session_id: SessionId,
        now_seconds: u64,
    },
    ReadScreen {
        session_id: SessionId,
        probe_id: String,
    },
    ReadModeFlags {
        session_id: SessionId,
        probe_id: String,
    },
    CaptureSnapshot {
        session_id: SessionId,
        owner: CaptureOwner,
        host_capture: u64,
    },
}

struct OpenCapture {
    owner: CaptureOwner,
    bytes: Arc<[u8]>,
    last_touched: Instant,
}

/// Where a readback for one session resolves.
enum ReadbackSource {
    /// The session is live; ask the worker or local backend.
    Live,
    /// The session ended in this incarnation and its history is retained.
    Retained(Arc<RetainedTerminal>),
    /// The session ended and its history is not available.
    Unavailable(HistoryUnavailableReason),
}

struct ObservePassState {
    pass_id: ObserveLifecyclePassId,
    last_visited: Option<SessionId>,
    generation: u64,
    final_session_id: Option<String>,
}

enum NextObserveSession {
    Session(SessionId),
    Complete,
    Elapsed,
}

struct BaselineFreeze {
    snapshot_sequence: SessionLifecycleCursor,
    dir: Option<std::fs::ReadDir>,
    excluded: BTreeSet<String>,
    membership: BTreeMap<String, Option<SessionLifecycleRecord>>,
    index_complete: bool,
}

struct ObserveLifecycleWalk {
    slice: ObserveLifecycleSlice,
    session_errors: Vec<ObserveLifecycleSessionError>,
}

enum DaemonEngine {
    Local(Box<DefaultBotsterEngine>),
    Worker(Box<WorkerBackedBotsterEngine>),
}

fn reservation_error(error: SessionReservationRefusal) -> CoreDaemonError {
    match error {
        SessionReservationRefusal::Capacity => {
            CoreDaemonError::PendingLimit(crate::operation::PendingLimitKind::Spawns)
        }
        error => CoreDaemonError::SessionReservation(error),
    }
}

fn reserved_launch_was_admitted(error: &DefaultBotsterEngineError) -> bool {
    matches!(
        error,
        DefaultBotsterEngineError::Multiplexer(
            MultiplexerEngineError::ReservedSpawn(ReservedSessionSpawnError::Admitted(_))
                | MultiplexerEngineError::InstallationAfterLaunch(_)
        )
    )
}

struct PendingDrainResult {
    session_id: SessionId,
    result: DrainResult,
}

impl CoreDaemon {
    /// Build a daemon with a caller-provided data directory.
    #[must_use]
    pub fn new(config: CoreDaemonConfig) -> Self {
        let registry = SessionRegistry::new(&config.data_dir);
        let ghostty_max_scrollback_bytes = config.ghostty_max_scrollback_bytes;
        let terminal_color_profile = config.terminal_color_profile.clone();
        let engine = config
            .worker_path
            .as_ref()
            .map(|worker_path| {
                let mut options = WorkerProcessRuntimeOptions::new(worker_path);
                options.control_socket_dir = Some(worker_socket_dir(&config.data_dir));
                options.worker_reply_timeout = config.worker_reply_timeout;
                options.test_hold_after_read_ms = config.test_hold_after_read_ms;
                options.test_write_block_until_unix_ms = config.test_write_block_until_unix_ms;
                options.test_write_max_chunk = config.test_write_max_chunk;
                options.test_pending_capacity = config.test_pending_capacity;
                options.test_hold_after_enqueue_ms = config.test_hold_after_enqueue_ms;
                options.ghostty_max_scrollback_bytes = ghostty_max_scrollback_bytes;
                options.terminal_color_profile = terminal_color_profile.clone();
                options.test_resize_ack_hold = config.test_resize_ack_hold.clone();
                options.test_hold_before_exit_ms = config.test_hold_before_exit_ms;
                options.test_exit_code = config.test_exit_code;
                if let Some(capacity) = config.pty_reader_chunk_capacity {
                    options.pty_reader_chunk_capacity = capacity;
                }
                if let Some(capacity) = config.test_worker_egress_capacity {
                    options.egress_capacity = capacity;
                }
                DaemonEngine::Worker(Box::new(WorkerBackedBotsterEngine::with_options(options)))
            })
            .unwrap_or_else(|| {
                DaemonEngine::Local(Box::new(local_engine(
                    ghostty_max_scrollback_bytes,
                    terminal_color_profile,
                )))
            });
        let acknowledged_terminal_inventory_revision = engine.terminal_inventory_revision();
        let envelope_queue = config.routed_envelope_queue.clone();
        Self {
            config,
            registry,
            engine,
            notification_inbox: NotificationInbox::new(),
            envelope_router: RoutedEnvelopeRouter::with_config(envelope_queue),
            pending_drain: Vec::new(),
            retained_terminal: HashMap::new(),
            retained_unavailable: HashMap::new(),
            retention_accounting: RetentionAccounting::default(),
            terminal_commit_obligations: HashMap::new(),
            terminal_commit_failures: HashMap::new(),
            acknowledged_terminal_inventory_revision,
            lifecycle_source_id: new_lifecycle_source_id(),
            lifecycle_sequence: 0,
            lifecycle_journal: VecDeque::new(),
            journal_advanced: false,
            observe_pass: None,
            observe_live_sessions: BTreeMap::new(),
            observe_live_generation: 0,
            baseline_freeze: None,
            next_pending_operation: 1,
            pending: BTreeMap::new(),
            completions: Vec::new(),
            open_captures: HashMap::new(),
            next_capture: 1,
            #[cfg(test)]
            observe_index_scans: 0,
            #[cfg(test)]
            baseline_index_scans: 0,
            #[cfg(test)]
            baseline_row_copies: 0,
            #[cfg(test)]
            baseline_page_encodes: 0,
            #[cfg(test)]
            registry_load_all_calls: Cell::new(0),
            running: true,
            wake_pump: None,
        }
    }

    /// Return the registry handle.
    #[must_use]
    pub const fn registry(&self) -> &SessionRegistry {
        &self.registry
    }

    /// Spawn a session through the existing core local engine path and persist registry metadata.
    pub fn spawn(
        &mut self,
        request: SpawnSessionRequest,
        now_seconds: u64,
    ) -> Result<CoreSession, CoreDaemonError> {
        self.ensure_running()?;
        let session_id = request.request.session_id.clone();
        let size = request
            .request
            .initial_pty_size
            .clone()
            .unwrap_or(ResizePayload { rows: 24, cols: 80 });
        let label = command_label(&request.request.executable);
        let spawn = self
            .engine
            .spawn_session(request.request, request.metadata)?;
        self.track_live_session(&session_id);
        let mut record = RegistryRecord::running(
            session_id,
            Some(spawn.handle.process),
            size,
            label,
            now_seconds,
        );
        record.metadata = spawn.session.metadata.clone();
        self.fence_baseline_before_save(&record.session_id)?;
        if let Some(metadata) = self.engine.worker_metadata(&record.session_id) {
            if let Some(identity) = metadata.recovery_identity.clone() {
                record.observe_restart_contract(identity, now_seconds);
            }
        }
        self.registry.save(&record)?;
        self.append_lifecycle_upsert(&record, Some(spawn.session.lifecycle.clone()));
        Ok(spawn.session)
    }

    /// List durable daemon sessions.
    pub fn list(&self) -> Result<Vec<DaemonSession>, CoreDaemonError> {
        Ok(self
            .load_all_records()?
            .iter()
            .map(DaemonSession::from)
            .collect())
    }

    /// Return a deterministic authoritative lifecycle baseline.
    ///
    /// This compatibility wrapper loads **every** registry row in one call.
    /// Production Stage A hosts must use [`Self::lifecycle_baseline_page`].
    pub fn lifecycle_baseline(&self) -> Result<SessionLifecycleBaseline, CoreDaemonError> {
        let sessions = self
            .load_all_records()?
            .iter()
            .map(|record| self.lifecycle_record(record))
            .collect();
        Ok(SessionLifecycleBaseline {
            cursor: self.lifecycle_cursor(),
            sessions,
        })
    }

    /// Return one page of a frozen lifecycle baseline snapshot.
    ///
    /// `snapshot = None` mints a freeze at the current journal watermark and
    /// walks the registry directory under the supplied item, encoded-byte,
    /// and elapsed budgets. Later pages with that snapshot continue the same
    /// freeze. An incomplete page has `complete = false` and is not finished
    /// ended evidence. Setup-only and index-in-progress yields keep the
    /// freeze identity and set `next = None`. One freeze is cached at a
    /// time; a new mint replaces it. A complete page drops the freeze.
    pub fn lifecycle_baseline_page(
        &mut self,
        snapshot: Option<&SessionLifecycleCursor>,
        after: Option<&SessionId>,
        budget: LifecycleBaselineBudget,
    ) -> Result<SessionLifecycleBaselinePage, SessionLifecyclePageError> {
        let started = Instant::now();
        let mut ops = 0_u64;

        if let Some(requested) = snapshot {
            if requested.source_id != self.lifecycle_source_id {
                return Ok(baseline_resync_page(
                    requested.clone(),
                    SessionLifecycleResyncReason::SourceChanged,
                ));
            }
            match self.baseline_freeze.as_ref() {
                Some(freeze) if freeze.snapshot_sequence == *requested => {}
                _ => {
                    return Ok(baseline_resync_page(
                        requested.clone(),
                        SessionLifecycleResyncReason::SnapshotUnavailable,
                    ));
                }
            }
        } else {
            self.baseline_freeze = Some(BaselineFreeze {
                snapshot_sequence: self.lifecycle_cursor(),
                dir: None,
                excluded: BTreeSet::new(),
                membership: BTreeMap::new(),
                index_complete: false,
            });
        }

        let snapshot_sequence = self
            .baseline_freeze
            .as_ref()
            .expect("freeze exists after mint or match")
            .snapshot_sequence
            .clone();
        let empty = SessionLifecycleBaselinePage {
            snapshot_sequence: snapshot_sequence.clone(),
            sessions: Vec::new(),
            next: None,
            complete: false,
            resync_required: None,
        };
        let minimum_bytes = encoded_lifecycle_baseline_page_len(&empty);
        if budget.max_bytes < minimum_bytes {
            return Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes });
        }
        if self.baseline_elapsed(started, ops) >= budget.max_elapsed {
            return Ok(empty);
        }

        let mut items_used = 0_usize;
        if let Err(()) = self.advance_baseline_index(started, &mut ops, &mut items_used, &budget) {
            self.baseline_freeze = None;
            return Ok(baseline_resync_page(
                snapshot_sequence,
                SessionLifecycleResyncReason::SourceChanged,
            ));
        }

        let index_complete = self
            .baseline_freeze
            .as_ref()
            .is_some_and(|freeze| freeze.index_complete);
        if !index_complete {
            return Ok(empty);
        }

        self.emit_baseline_suffix(after, started, &mut ops, &mut items_used, budget, empty)
    }

    /// Return ordered lifecycle changes after a source cursor.
    ///
    /// Foreign, expired, or future cursors return no partial suffix and set an
    /// explicit resync reason. Recovery is a fresh [`Self::lifecycle_baseline`].
    #[must_use]
    pub fn lifecycle_changes(&self, after: &SessionLifecycleCursor) -> SessionLifecycleChanges {
        let cursor = self.lifecycle_cursor();
        let resync_required = self.lifecycle_resync_reason(after);
        let changes = if resync_required.is_some() {
            Vec::new()
        } else {
            self.lifecycle_journal
                .iter()
                .filter(|change| change.cursor.sequence > after.sequence)
                .cloned()
                .collect()
        };
        SessionLifecycleChanges {
            cursor,
            changes,
            resync_required,
        }
    }

    /// Return one bounded lifecycle page after a source cursor.
    ///
    /// Cursor identity is validated before the successful-page byte budget.
    /// Resync outcomes return empty `changes` and the exact reason even when
    /// `max_bytes` is undersized. They are control outcomes, not successful
    /// pages. A valid cursor whose empty successful page encodes larger than
    /// `max_bytes` returns [`SessionLifecyclePageError::BudgetTooSmall`].
    pub fn lifecycle_changes_page(
        &self,
        after: &SessionLifecycleCursor,
        max_changes: usize,
        max_bytes: usize,
    ) -> Result<SessionLifecyclePage, SessionLifecyclePageError> {
        let source_watermark = self.lifecycle_cursor();
        if let Some(resync_required) = self.lifecycle_resync_reason(after) {
            return Ok(SessionLifecyclePage {
                changes: Vec::new(),
                next: after.clone(),
                source_watermark,
                resync_required: Some(resync_required),
            });
        }

        let empty = SessionLifecyclePage {
            changes: Vec::new(),
            next: after.clone(),
            source_watermark: source_watermark.clone(),
            resync_required: None,
        };
        let minimum_bytes = encoded_lifecycle_page_len(&empty);
        if max_bytes < minimum_bytes {
            return Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes });
        }

        let mut page = empty;
        for change in self
            .lifecycle_journal
            .iter()
            .filter(|change| change.cursor.sequence > after.sequence)
        {
            if page.changes.len() >= max_changes {
                break;
            }
            let mut candidate = page.clone();
            candidate.next = change.cursor.clone();
            candidate.changes.push(change.clone());
            if encoded_lifecycle_page_len(&candidate) > max_bytes {
                break;
            }
            page = candidate;
        }
        Ok(page)
    }

    /// Advance session lifecycle facts without returning terminal Drain results.
    ///
    /// This compatibility wrapper starts a new pass and visits every remaining
    /// live session in one call. Production Stage A hosts must use
    /// [`Self::observe_lifecycle_slice`]. Each session is drained and
    /// reconciled independently. Incidental terminal egress stays on the
    /// pending-drain path for a later [`Self::drain`]. This method does not
    /// call `drain_runtime_all_once`.
    pub fn observe_lifecycle(
        &mut self,
        now_seconds: u64,
    ) -> Result<ObserveLifecycleResult, CoreDaemonError> {
        self.ensure_running()?;
        let walk = self
            .observe_lifecycle_walk(
                now_seconds,
                None,
                ObserveLifecycleBudget {
                    max_sessions: usize::MAX,
                    max_encoded_result_bytes: usize::MAX,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("unbounded observe wrapper cannot exceed the encoded-result budget");
        Ok(ObserveLifecycleResult {
            session_errors: walk.session_errors,
        })
    }

    /// Advance a bounded slice of live sessions in deterministic `SessionId`
    /// order.
    ///
    /// `resume = None` mints a new pass over the ordered live-session index.
    /// `resume = Some(cursor)` continues only when `pass_id` and
    /// `last_visited` both match that snapshot; otherwise the result is a
    /// resync with `complete = false` and no suffix. Later slices walk the
    /// unvisited ordered suffix and do not list or sort the full live set.
    /// Generation tags exclude sessions that appear after mint. Item,
    /// encoded-result, and elapsed limits each stop before remaining sessions
    /// are visited. Elapsed starts at API entry and includes pass setup. A
    /// setup-only yield resumes with `last_visited = None`. Byte
    /// admission uses a reserved 256-`x` error before each visit because
    /// `observe_session` cannot be rolled back.
    ///
    /// Observe does not `try_write` a bound adapter. After it queues frames
    /// onto a bound Ready owner, it emits one coalesced session ingress wake.
    /// Hosts deliver those frames through [`Self::wait_wakes`] and
    /// [`Self::pump_woken`].
    pub fn observe_lifecycle_slice(
        &mut self,
        now_seconds: u64,
        resume: Option<&ObserveLifecycleCursor>,
        budget: ObserveLifecycleBudget,
    ) -> Result<ObserveLifecycleSlice, SessionLifecyclePageError> {
        if !self.running {
            return Ok(ObserveLifecycleSlice {
                pass_id: resume
                    .map(|cursor| cursor.pass_id.clone())
                    .unwrap_or_else(new_observe_pass_id),
                last_visited: None,
                complete: false,
                session_errors: Vec::new(),
                resync_required: Some(SessionLifecycleResyncReason::SourceChanged),
            });
        }
        self.observe_lifecycle_walk(now_seconds, resume, budget)
            .map(|walk| walk.slice)
    }

    /// Reconcile one session and return its control-plane lifecycle row.
    ///
    /// This query is independent of registry size. It visits only `session_id`.
    /// It does not call `observe_lifecycle`, `lifecycle_baseline`, or
    /// `load_all`. Hosts use this for exact-session classification. They
    /// must not classify shutdown from terminal Drain or a capped page walk.
    ///
    /// `Ok(Absent)` means both the registry and the engine lack the session
    /// after the observe attempt. Drain failure, registry I/O, malformed
    /// JSON, and shutdown return `Err`. Absence is not
    /// [`CoreDaemonError::UnknownSession`].
    ///
    /// This query does not `try_write` a bound adapter. When it queues frames
    /// onto a bound Ready owner, it emits one coalesced session ingress wake
    /// for [`Self::wait_wakes`] and [`Self::pump_woken`].
    pub fn observe_session_lifecycle(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<SessionLifecycleLookup, CoreDaemonError> {
        self.ensure_running()?;
        let engine_has_session = self.engine.session(session_id).is_some();
        let live_index_has_session = self.observe_live_sessions.contains_key(&session_id.0);
        if engine_has_session || live_index_has_session {
            self.observe_session(session_id, now_seconds)?;
        }
        match self.registry.load(session_id)? {
            Some(record) => Ok(SessionLifecycleLookup::Found(
                self.lifecycle_record(&record),
            )),
            None if self.engine.session(session_id).is_none() => Ok(SessionLifecycleLookup::Absent),
            None => Err(CoreDaemonError::UnknownSession(session_id.clone())),
        }
    }

    /// Exact non-mutating registry state for one `session_id`.
    ///
    /// This query loads one registry record. It does not drain a runtime,
    /// reconcile a parked process-exit observation, save the registry, append
    /// the lifecycle journal, or raise the coalesced journal-advanced wake.
    /// Hosts that want lifecycle progress still call
    /// [`Self::observe_session_lifecycle`].
    ///
    /// `Ok(Absent)` means both the registry and the engine lack the session.
    /// Registry I/O and shutdown return `Err`. A live engine session without a
    /// registry row is [`CoreDaemonError::UnknownSession`], matching
    /// [`Self::observe_session_lifecycle`].
    pub fn session_registry_state(
        &self,
        session_id: &SessionId,
    ) -> Result<SessionRegistryStateLookup, CoreDaemonError> {
        self.ensure_running()?;
        match self.registry.load(session_id)? {
            Some(record) => Ok(SessionRegistryStateLookup::Found(record.state)),
            None if self.engine.session(session_id).is_none() => {
                Ok(SessionRegistryStateLookup::Absent)
            }
            None => Err(CoreDaemonError::UnknownSession(session_id.clone())),
        }
    }

    /// Take the coalesced journal-advanced wake bit.
    ///
    /// The wake is one pending bit, not a queue. Page and baseline never clear
    /// it. Append always sets it. Safe consumer order is take, page until
    /// caught up or resync, take again, and re-page if that second take is
    /// true.
    #[must_use]
    pub fn take_journal_advanced_wake(&mut self) -> bool {
        std::mem::take(&mut self.journal_advanced)
    }

    /// Record that the next attach for this identity will bind an adapter.
    ///
    /// After a matching [`Self::attach`], `AttachedSession.client_egress` holds
    /// no terminal frame for that route. Those frames stay inside Core until
    /// [`Self::bind_waking_terminal_adapter`] and the next targeted pump.
    pub fn expect_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.engine
            .expect_terminal_adapter(client_id, session_id, subscription_id);
        Ok(())
    }

    /// Retire an unconsumed pre-attach adapter declaration.
    pub fn cancel_expected_terminal_adapter(
        &mut self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.engine
            .cancel_expected_terminal_adapter(client_id, session_id, subscription_id);
        Ok(())
    }

    /// Attach a client through the existing subscription path.
    ///
    /// A local attach returns the complete route-owned bootstrap. A capable
    /// worker attach returns `Attaching`. For a bound route, later targeted
    /// [`Self::wait_wakes`] and [`Self::pump_woken`] calls advance incremental
    /// Snapshot frames, `Attached`, and live output. For an unbound route,
    /// [`Self::drain`] returns those frames. No other client receives them.
    ///
    /// When [`Self::expect_terminal_adapter`] was called for this identity,
    /// `AttachedSession.client_egress` is empty for that route. The host must
    /// bind an adapter and use the targeted wake pump to receive the held
    /// frames.
    pub fn attach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<AttachedSession, CoreDaemonError> {
        let declaration = (
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        );
        let output: Result<_, CoreDaemonError> = (|| {
            self.ensure_running()?;
            self.ensure_session_mutable(&session_id)?;
            self.ensure_control_plane_live(&session_id)?;
            Ok(self.engine.attach_client(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                now_seconds,
            )?)
        })();
        let output = match output {
            Ok(output) => output,
            Err(error) => {
                self.engine.cancel_expected_terminal_adapter(
                    &declaration.0,
                    declaration.1,
                    declaration.2,
                );
                return Err(error);
            }
        };
        self.drop_pending_client_session_egress(&client_id, &session_id);
        let initial_output = drain_result_from_engine_output(output);
        let mut client_egress = Vec::new();
        let mut unmatched_egress = Vec::new();
        for (pending_client, frame) in initial_output.client_egress {
            if pending_client == client_id
                && egress_route(&frame) == Some((&session_id, &subscription_id))
            {
                client_egress.push((pending_client, frame));
            } else {
                unmatched_egress.push((pending_client, frame));
            }
        }
        let pending = DrainResult {
            client_egress: unmatched_egress,
            observations: initial_output.observations,
            backpressure: initial_output.backpressure,
        };
        if !drain_result_is_empty(&pending) {
            self.pending_drain.push(PendingDrainResult {
                session_id: session_id.clone(),
                result: pending,
            });
        }
        Ok(AttachedSession {
            client_id,
            session_id,
            subscription_id,
            client_egress,
        })
    }

    /// Bind a waking adapter. Allocates wake state only after rejection checks pass.
    pub fn bind_waking_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        mut adapter: Box<dyn WakingTerminalAdapter + Send>,
    ) -> Result<(), CoreDaemonError> {
        if let Err(error) = self.ensure_running() {
            adapter.close(TerminalRouteCloseReason::BindRejected);
            drop(adapter);
            return Err(error);
        }
        if let Err(error) = self.ensure_session(&session_id) {
            adapter.close(TerminalRouteCloseReason::BindRejected);
            drop(adapter);
            return Err(error);
        }
        if self.engine.control_plane_failed(&session_id) {
            adapter.close(TerminalRouteCloseReason::BindRejected);
            drop(adapter);
            return Err(BindTerminalAdapterError::ControlPlaneFailed { session_id }.into());
        }
        self.engine.bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id.clone(),
            generation,
            capabilities,
            adapter,
        )?;
        if self
            .engine
            .bound_owner_has_held_frames(&session_id, &subscription_id)
        {
            self.engine.wake_source().notify_session(&session_id);
        }
        Ok(())
    }

    /// Block until adapter or ingress wakes arrive, or `timeout` elapses.
    #[must_use]
    pub fn wait_wakes(&self, timeout: Duration) -> TerminalWakeBatch {
        self.engine
            .wait_wakes(self.clamp_pending_operation_wait(timeout))
    }

    /// Clamp a host wait to every Core deadline: engine paste, resize, and
    /// barrier bounds, and pending-operation deadlines.
    fn clamp_wait(&self, timeout: Duration) -> Duration {
        self.clamp_pending_operation_wait(self.engine.clamp_paste_wait(timeout))
    }

    /// Clamp a host wait to the earliest pending-operation deadline, so an
    /// expiry is reconciled by the next pump instead of the host's timeout.
    fn clamp_pending_operation_wait(&self, timeout: Duration) -> Duration {
        let now = Instant::now();
        self.pending
            .values()
            .filter_map(|state| state.deadline)
            .map(|deadline| deadline.saturating_duration_since(now))
            .fold(timeout, Duration::min)
    }

    /// Number of accepted-but-unacknowledged ingress resizes for one session.
    #[doc(hidden)]
    #[must_use]
    pub fn pending_terminal_resize_len(&self, session_id: &SessionId) -> usize {
        self.engine.pending_terminal_resize_len(session_id)
    }

    /// Durable control-plane state for one worker session.
    #[doc(hidden)]
    #[must_use]
    pub fn control_plane_state(
        &self,
        session_id: &SessionId,
    ) -> botster_core::runtime::ControlPlaneState {
        self.engine.control_plane_state(session_id)
    }

    /// Engine lifecycle for one session. Not a host registry projection.
    #[doc(hidden)]
    #[must_use]
    pub fn engine_session_lifecycle(
        &self,
        session_id: &SessionId,
    ) -> Option<SessionLifecycleState> {
        self.engine
            .session(session_id)
            .map(|session| session.lifecycle.clone())
    }

    /// Mark this daemon as pump-hosted and return its thread-safe control.
    ///
    /// The returned handle has no daemon access. Only the daemon owner thread
    /// can call [`Self::wait_pump`], other Core methods, and [`Self::shutdown`].
    #[must_use]
    pub fn wake_pump_control(&mut self) -> WakePumpControl {
        if self.wake_pump.is_none() {
            let interrupt = self.engine.wake_source().interrupt_handle();
            self.wake_pump = Some(WakePumpState::new(interrupt));
        }
        self.wake_pump
            .as_ref()
            .expect("wake pump state was initialized")
            .control()
    }

    /// Wait for terminal wakes or host control without polling.
    ///
    /// A stop permits at most one final, capacity-capped collision batch. Every
    /// later call returns [`WakePumpWait::Stopped`] without draining the wake
    /// channel. Returning a collision batch does not count as observing the
    /// stopped state. Only one thread may call this method for a daemon.
    #[must_use]
    pub fn wait_pump(&self, timeout: Duration) -> WakePumpWait {
        let Some(state) = &self.wake_pump else {
            return WakePumpWait::Wakes(self.wait_wakes(timeout));
        };

        if state
            .stop_requested
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return self.wait_pump_after_stop(state);
        }

        let waited = self
            .engine
            .wake_source()
            .wait_wakes_interruptible(self.clamp_wait(timeout));
        if state
            .stop_requested
            .load(std::sync::atomic::Ordering::Acquire)
        {
            if state
                .stop_collision_consumed
                .swap(true, std::sync::atomic::Ordering::AcqRel)
            {
                return Self::observe_pump_stopped(state);
            }
            return match waited {
                TerminalWakeWait::Wakes(batch) => WakePumpWait::Wakes(batch),
                TerminalWakeWait::Interrupted | TerminalWakeWait::TimedOut => {
                    self.final_stop_collision_drain(state)
                }
                _ => self.final_stop_collision_drain(state),
            };
        }

        match waited {
            TerminalWakeWait::Wakes(batch) => {
                WakePumpWait::Wakes(self.engine.merge_deadline_wakes(batch))
            }
            TerminalWakeWait::Interrupted => WakePumpWait::Interrupted,
            TerminalWakeWait::TimedOut => WakePumpWait::Wakes(
                self.engine
                    .merge_deadline_wakes(TerminalWakeBatch::default()),
            ),
            _ => WakePumpWait::Wakes(
                self.engine
                    .merge_deadline_wakes(TerminalWakeBatch::default()),
            ),
        }
    }

    fn wait_pump_after_stop(&self, state: &WakePumpState) -> WakePumpWait {
        if state
            .stop_collision_consumed
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            Self::observe_pump_stopped(state)
        } else {
            self.final_stop_collision_drain(state)
        }
    }

    fn final_stop_collision_drain(&self, state: &WakePumpState) -> WakePumpWait {
        match self
            .engine
            .wake_source()
            .wait_wakes_interruptible(Duration::ZERO)
        {
            TerminalWakeWait::Wakes(batch) => WakePumpWait::Wakes(batch),
            TerminalWakeWait::Interrupted | TerminalWakeWait::TimedOut => {
                Self::observe_pump_stopped(state)
            }
            _ => Self::observe_pump_stopped(state),
        }
    }

    fn observe_pump_stopped(state: &WakePumpState) -> WakePumpWait {
        state
            .stop_observed
            .store(true, std::sync::atomic::Ordering::Release);
        WakePumpWait::Stopped
    }

    /// Targeted pump of woken routes. Does not scan unnamed sessions.
    ///
    /// Core commits lifecycle observations and retains unmatched output before
    /// this method returns. Hosts can ignore the content-free outcome.
    pub fn pump_woken(
        &mut self,
        batch: &TerminalWakeBatch,
        now_seconds: u64,
    ) -> Result<PumpWokenOutcome, CoreDaemonError> {
        self.ensure_running()?;
        let terminal_inventory_revision_before = self.acknowledged_terminal_inventory_revision;
        let pumped_routes = batch.adapter_routes.len();
        let mut session_ids: Vec<_> = batch
            .adapter_routes
            .iter()
            .map(|route| route.session_id.clone())
            .chain(batch.ingress_sessions.iter().cloned())
            .collect();
        session_ids.sort_by(|left, right| left.0.cmp(&right.0));
        session_ids.dedup();

        let mut first_error = None;
        let mut pumped_results = Vec::with_capacity(session_ids.len());
        for session_id in session_ids {
            // A launch that is still pending has no engine session to pump.
            // Its wake exists so reconcile_pending below installs it.
            if self.engine.spawn_pending(&session_id) {
                continue;
            }
            let sub_batch = TerminalWakeBatch {
                adapter_routes: batch
                    .adapter_routes
                    .iter()
                    .filter(|route| route.session_id == session_id)
                    .cloned()
                    .collect(),
                ingress_sessions: if batch
                    .ingress_sessions
                    .iter()
                    .any(|candidate| candidate == &session_id)
                {
                    vec![session_id.clone()]
                } else {
                    Vec::new()
                },
            };
            let output = match self.engine.pump_woken(&sub_batch, now_seconds) {
                Ok(output) => output,
                Err(error) => {
                    self.record_terminal_commit_failure(&session_id, None);
                    first_error.get_or_insert_with(|| error.into());
                    continue;
                }
            };
            let result = drain_result_from_engine_output(output);
            debug_assert_drain_owner(&result, &session_id);
            pumped_results.push((session_id, result));
        }

        for (session_id, result) in pumped_results {
            if let Some((rows, cols, resize_at)) =
                self.engine.take_applied_terminal_resize(&session_id)
            {
                if let Err(error) =
                    self.persist_changed_session_size(&session_id, rows, cols, resize_at)
                {
                    self.record_terminal_obligations(&result.observations);
                    self.record_terminal_commit_failure(&session_id, None);
                    self.retain_pending_drain_result(&session_id, result);
                    first_error.get_or_insert(error);
                    continue;
                }
            }
            self.discard_bound_queue_wakes();
            if let Err(error) =
                self.commit_terminal_lifecycle(&session_id, &result.observations, now_seconds)
            {
                self.retain_pending_drain_result(&session_id, result);
                first_error.get_or_insert(error);
                continue;
            }
            self.retain_pending_drain_result(&session_id, result);
        }
        self.reconcile_pending(now_seconds);

        if let Some(error) = first_error {
            Err(error)
        } else {
            let terminal_inventory_revision_after = self.engine.terminal_inventory_revision();
            self.acknowledged_terminal_inventory_revision = terminal_inventory_revision_after;
            Ok(PumpWokenOutcome {
                pumped_routes,
                terminal_inventory_changed: terminal_inventory_revision_before
                    != terminal_inventory_revision_after,
            })
        }
    }

    /// Shared wake source for census and host wait loops.
    #[must_use]
    pub fn wake_source(&self) -> &TerminalWakeSource {
        self.engine.wake_source()
    }

    /// Complete control-plane inventory admitted against caller-owned logical bytes.
    pub fn list_terminal_subscriptions(
        &self,
        max_logical_bytes: usize,
    ) -> Result<TerminalSubscriptionInventory, TerminalSubscriptionInventoryError> {
        self.engine.list_terminal_subscriptions(max_logical_bytes)
    }

    /// Borrow the live client identity and generation for one exact route.
    ///
    /// Uses an allocation-free O(n) scan. Only live owners are returned;
    /// historical routes and pre-attach declarations are excluded. The borrow
    /// ends before any subsequent mutation, so callers can retain scalar
    /// client-match and generation decisions without cloning an inventory.
    #[must_use]
    pub fn terminal_subscription_owner(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<(&ClientId, TerminalSubscriptionGeneration)> {
        self.engine
            .terminal_subscription_owner(session_id, subscription_id)
    }

    /// Exact live generation for one `(session_id, subscription_id)`, or `None`.
    ///
    /// This query is independent of inventory size. It does not clone or sort
    /// the full subscription inventory. It is identity-only: no terminal
    /// bytes, phases, snapshots, or attach state.
    #[must_use]
    pub fn terminal_subscription_generation(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<TerminalSubscriptionGeneration> {
        self.engine
            .terminal_subscription_generation(session_id, subscription_id)
    }

    /// Detach one subscription generation. Mismatch does not delete a newer owner.
    pub fn detach_terminal_subscription(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        now_seconds: u64,
    ) -> Result<DetachTerminalSubscriptionResult, CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(&session_id)?;
        let (result, _) = self.engine.detach_terminal_subscription(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            generation,
            now_seconds,
        )?;
        if matches!(result, DetachTerminalSubscriptionResult::Detached { .. }) {
            self.drop_pending_subscription_egress(&client_id, &session_id, &subscription_id);
        }
        Ok(result)
    }

    /// Detach a client through the existing subscription path.
    pub fn detach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(&session_id)?;
        self.engine.detach_client(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            now_seconds,
        )?;
        self.drop_pending_subscription_egress(&client_id, &session_id, &subscription_id);
        Ok(())
    }
    /// Send PTY input through the existing engine path.
    pub fn input(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session_mutable(&session_id)?;
        self.engine
            .write_bytes(client_id, session_id, data.into(), now_seconds)?;
        Ok(())
    }

    /// Resize a session through the existing engine path and update registry metadata.
    pub fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session_mutable(&session_id)?;
        if self.engine.has_pending_terminal_resizes(&session_id) {
            return Err(CoreDaemonError::ExplicitResizeBusy(session_id));
        }
        let applied_later = self.engine.capture_active(&session_id);
        self.engine
            .resize(client_id, session_id.clone(), rows, cols, now_seconds)?;
        if !applied_later {
            self.persist_session_size(&session_id, rows, cols, now_seconds)?;
        }
        Ok(())
    }

    fn persist_session_size(
        &mut self,
        session_id: &SessionId,
        rows: u16,
        cols: u16,
        updated_at: u64,
    ) -> Result<(), CoreDaemonError> {
        if let Some(mut record) = self.registry.load(session_id)? {
            record.rows = rows;
            record.cols = cols;
            record.updated_at = updated_at;
            self.fence_baseline_before_save(session_id)?;
            self.registry.save(&record)?;
            let lifecycle = self
                .engine
                .session(session_id)
                .map(|session| session.lifecycle.clone());
            self.append_lifecycle_upsert(&record, lifecycle);
        }
        Ok(())
    }

    fn persist_changed_session_size(
        &mut self,
        session_id: &SessionId,
        rows: u16,
        cols: u16,
        updated_at: u64,
    ) -> Result<(), CoreDaemonError> {
        if self
            .registry
            .load(session_id)?
            .is_some_and(|record| record.rows == rows && record.cols == cols)
        {
            return Ok(());
        }
        self.persist_session_size(session_id, rows, cols, updated_at)
    }

    /// Drain one session's runtime output through subscription fanout.
    ///
    /// This method does not `try_write` a bound adapter. After it queues frames
    /// onto a bound Ready owner, it emits one coalesced session ingress wake
    /// for [`Self::wait_wakes`] and [`Self::pump_woken`].
    pub fn drain(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<DrainResult, CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(session_id)?;
        let mut result = self.take_pending_drain(session_id);
        match self.engine.drain_runtime_once(session_id, last_output_at) {
            Ok(outcome) => {
                merge_drain_result(&mut result, drain_result_from_engine_output(outcome))
            }
            Err(error)
                if is_session_not_found(&error) && self.engine_session_exited(session_id) => {}
            Err(error) => {
                self.retain_pending_drain_result(session_id, result);
                return Err(error.into());
            }
        }
        self.notify_bound_queue_wakes();
        if let Err(error) =
            self.commit_terminal_lifecycle(session_id, &result.observations, last_output_at)
        {
            self.retain_pending_drain_result(session_id, result);
            return Err(error);
        }
        Ok(result)
    }
    /// Drain one subscription without consuming frames for another route.
    pub fn drain_subscription(
        &mut self,
        client_id: &ClientId,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        last_output_at: u64,
    ) -> Result<DrainResult, CoreDaemonError> {
        let mut result = self.drain(session_id, last_output_at)?;
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        for (target, frame) in result.client_egress {
            if &target == client_id && egress_route(&frame) == Some((session_id, subscription_id)) {
                matched.push((target, frame));
            } else {
                unmatched.push((target, frame));
            }
        }
        if !unmatched.is_empty() {
            self.pending_drain.push(PendingDrainResult {
                session_id: session_id.clone(),
                result: DrainResult {
                    client_egress: unmatched,
                    observations: Vec::new(),
                    backpressure: Vec::new(),
                },
            });
        }
        result.client_egress = matched;
        Ok(result)
    }

    /// Current retention policy.
    #[must_use]
    pub const fn retention_policy(&self) -> RetentionPolicy {
        self.config.retention
    }

    /// Current retention accounting.
    #[must_use]
    pub const fn retention_accounting(&self) -> RetentionAccounting {
        self.retention_accounting
    }

    /// Start one operation and return at once.
    ///
    /// Operations that need no worker round trip complete before this method
    /// returns and appear in the next [`Self::take_completions`]. The rest are
    /// reconciled by [`Self::pump_woken`]. Nothing here waits.
    pub fn begin(
        &mut self,
        operation: CoreOperation,
    ) -> Result<PendingOperationId, CoreDaemonError> {
        self.ensure_running()?;
        let id = self.allocate_pending_id()?;
        match operation {
            CoreOperation::ReserveSession(session_id) => {
                let result = self
                    .engine
                    .reserve_session_for_request(session_id, id.0, MAX_PENDING_SPAWNS)
                    .map_err(reservation_error);
                self.completions
                    .push(CoreCompletion::ReserveSession { id, result });
            }
            CoreOperation::LookupSessionReservation {
                session_id,
                reserve_operation_id,
            } => {
                let result = self
                    .engine
                    .session_reservation_for_request(&session_id, reserve_operation_id.0)
                    .map_err(reservation_error);
                self.completions
                    .push(CoreCompletion::LookupSessionReservation { id, result });
            }
            CoreOperation::SpawnReserved {
                reservation,
                request,
            } => {
                self.begin_reserved_spawn(id, reservation, request);
            }
            CoreOperation::ReleaseSessionReservation(reservation) => {
                let result = self
                    .engine
                    .release_session_reservation(&reservation)
                    .map_err(reservation_error);
                self.completions
                    .push(CoreCompletion::ReleaseSessionReservation { id, result });
            }
            CoreOperation::Spawn(request) => self.begin_spawn(id, request)?,
            CoreOperation::Adopt(session_id) => {
                let now_seconds = unix_now_seconds();
                let result = self.adopt_session(&session_id, now_seconds);
                self.completions.push(CoreCompletion::Adopt { id, result });
            }
            CoreOperation::ShutdownSession(session_id) => self.begin_shutdown(id, session_id)?,
            CoreOperation::RemoveSession(session_id) => {
                let result = self.remove_session(&session_id);
                self.completions
                    .push(CoreCompletion::RemoveSession { id, result });
            }
            CoreOperation::ReadScreen(request) => self.begin_read_screen(id, request)?,
            CoreOperation::ReadModeFlags(request) => self.begin_read_mode_flags(id, request)?,
            CoreOperation::CaptureSnapshot { request, owner } => {
                self.begin_capture_snapshot(id, request, owner)?;
            }
            CoreOperation::CancelInput {
                route,
                generation,
                operation_id,
            } => {
                let result = match &mut self.engine {
                    DaemonEngine::Local(_) => Ok(false),
                    DaemonEngine::Worker(engine) => engine
                        .cancel_input_operation(&route, generation, operation_id)
                        .map_err(CoreDaemonError::Engine),
                };
                self.completions
                    .push(CoreCompletion::CancelInput { id, result });
            }
        }
        Ok(id)
    }

    /// Cancel one pending operation. Returns `false` when it already finished.
    ///
    /// A cancelled operation completes with [`CoreDaemonError::Cancelled`].
    /// Worker work already sent is not undone.
    pub fn cancel(&mut self, id: PendingOperationId) -> bool {
        if self.pending.get(&id).is_some_and(|state| state.cancelled) {
            return false;
        }
        if let Some(state) = self.pending.get_mut(&id) {
            if let PendingKind::Spawn {
                session_id,
                reservation,
                ..
            } = &state.kind
            {
                // The launch thread still owns a child process. Keep the
                // entry, and the spawn concurrency slot, until the runtime
                // collects and stops that child.
                let session_id = session_id.clone();
                state.cancelled = true;
                if let DaemonEngine::Worker(engine) = &mut self.engine {
                    engine.abandon_spawn(&session_id);
                }
                self.completions.push(match reservation {
                    Some(reservation) => CoreCompletion::SpawnReserved {
                        id,
                        result: ReservedSpawnResult::AdmittedFailure {
                            error: CoreDaemonError::Cancelled,
                            state: reservation.state(),
                        },
                    },
                    None => CoreCompletion::Spawn {
                        id,
                        result: Err(CoreDaemonError::Cancelled),
                    },
                });
                return true;
            }
        }
        let Some(state) = self.pending.remove(&id) else {
            return false;
        };
        let completion = match state.kind {
            PendingKind::Spawn { .. } => CoreCompletion::Spawn {
                id,
                result: Err(CoreDaemonError::Cancelled),
            },
            PendingKind::ShutdownSession { .. } => CoreCompletion::ShutdownSession {
                id,
                result: Err(CoreDaemonError::Cancelled),
            },
            PendingKind::ReadScreen { .. } => CoreCompletion::ReadScreen {
                id,
                result: Err(CoreDaemonError::Cancelled),
            },
            PendingKind::ReadModeFlags { .. } => CoreCompletion::ReadModeFlags {
                id,
                result: Err(CoreDaemonError::Cancelled),
            },
            PendingKind::CaptureSnapshot {
                session_id,
                host_capture,
                ..
            } => {
                if let DaemonEngine::Worker(engine) = &mut self.engine {
                    engine.cancel_host_capture(&session_id, host_capture);
                }
                CoreCompletion::CaptureSnapshot {
                    id,
                    result: Err(CoreDaemonError::Cancelled),
                }
            }
        };
        self.completions.push(completion);
        true
    }

    /// Take every finished operation since the last call.
    pub fn take_completions(&mut self) -> Vec<CoreCompletion> {
        std::mem::take(&mut self.completions)
    }

    /// Whether any operation is still pending.
    #[must_use]
    pub fn has_pending_operations(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Read one page of an open snapshot capture without copying the bytes.
    pub fn read_snapshot_page(
        &mut self,
        capture: &CaptureId,
        page: u32,
    ) -> Result<SnapshotPage, CoreDaemonError> {
        let open = self
            .open_captures
            .get_mut(capture)
            .ok_or_else(|| CoreDaemonError::UnknownCapture(capture.clone()))?;
        let start = (page as usize).saturating_mul(SNAPSHOT_PAGE_BYTES);
        if start >= open.bytes.len() && !(page == 0 && open.bytes.is_empty()) {
            return Err(CoreDaemonError::SnapshotPageOutOfRange {
                capture: capture.clone(),
                page,
            });
        }
        let end = start
            .saturating_add(SNAPSHOT_PAGE_BYTES)
            .min(open.bytes.len());
        open.last_touched = Instant::now();
        Ok(SnapshotPage::new(Arc::clone(&open.bytes), start, end))
    }

    /// Release one open capture.
    pub fn release_capture(&mut self, capture: &CaptureId) -> bool {
        self.open_captures.remove(capture).is_some()
    }

    /// Release every capture held by one owner, for client disconnect.
    pub fn release_owner_captures(&mut self, owner: &CaptureOwner) -> usize {
        let before = self.open_captures.len();
        self.open_captures.retain(|_, open| &open.owner != owner);
        let released_open = before - self.open_captures.len();
        // Queued and active captures of this owner are cancelled as well so a
        // late worker completion cannot reopen a capture for a gone client.
        let pending_ids: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, state)| {
                matches!(&state.kind, PendingKind::CaptureSnapshot { owner: pending, .. } if pending == owner)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut cancelled = 0;
        for id in pending_ids {
            if self.cancel(id) {
                cancelled += 1;
            }
        }
        released_open + cancelled
    }

    fn allocate_pending_id(&mut self) -> Result<PendingOperationId, CoreDaemonError> {
        let id = PendingOperationId(self.next_pending_operation);
        self.next_pending_operation = self.next_pending_operation.checked_add(1).ok_or(
            CoreDaemonError::SessionReservation(SessionReservationRefusal::IdentityExhausted),
        )?;
        Ok(id)
    }

    fn pending_spawns(&self) -> usize {
        self.pending
            .values()
            .filter(|state| matches!(state.kind, PendingKind::Spawn { .. }))
            .count()
    }

    fn pending_readbacks(&self, session_id: &SessionId) -> usize {
        self.pending
            .values()
            .filter(|state| match &state.kind {
                PendingKind::ReadScreen { session_id: id, .. }
                | PendingKind::ReadModeFlags { session_id: id, .. }
                | PendingKind::CaptureSnapshot { session_id: id, .. } => id == session_id,
                _ => false,
            })
            .count()
    }

    fn open_captures_for(&self, owner: &CaptureOwner) -> usize {
        self.open_captures
            .values()
            .filter(|open| &open.owner == owner)
            .count()
            + self
                .pending
                .values()
                .filter(|state| {
                    matches!(&state.kind, PendingKind::CaptureSnapshot { owner: pending, .. } if pending == owner)
                })
                .count()
    }

    fn begin_spawn(
        &mut self,
        id: PendingOperationId,
        request: SpawnSessionRequest,
    ) -> Result<(), CoreDaemonError> {
        let now_seconds = unix_now_seconds();
        let pending_spawns = self
            .pending_spawns()
            .max(self.engine.pending_session_reservations());
        match &mut self.engine {
            DaemonEngine::Local(_) => {
                let result = self.spawn(request, now_seconds);
                self.completions.push(CoreCompletion::Spawn { id, result });
                Ok(())
            }
            DaemonEngine::Worker(engine) => {
                if pending_spawns >= MAX_PENDING_SPAWNS {
                    return Err(CoreDaemonError::PendingLimit(
                        crate::operation::PendingLimitKind::Spawns,
                    ));
                }
                let session_id = request.request.session_id.clone();
                let size = request
                    .request
                    .initial_pty_size
                    .clone()
                    .unwrap_or(ResizePayload { rows: 24, cols: 80 });
                let label = command_label(&request.request.executable);
                engine.begin_spawn(request.request)?;
                self.pending.insert(
                    id,
                    PendingState {
                        kind: PendingKind::Spawn {
                            reservation: None,
                            session_id,
                            metadata: request.metadata,
                            size,
                            label,
                            now_seconds,
                        },
                        deadline: None,
                        cancelled: false,
                    },
                );
                Ok(())
            }
        }
    }

    fn begin_reserved_spawn(
        &mut self,
        id: PendingOperationId,
        reservation: SessionReservation,
        request: SpawnSessionRequest,
    ) {
        if request.request.session_id != *reservation.session_id() {
            self.completions.push(CoreCompletion::SpawnReserved {
                id,
                result: ReservedSpawnResult::Refused {
                    error: reservation_error(SessionReservationRefusal::InvalidToken),
                },
            });
            return;
        }
        let now_seconds = unix_now_seconds();
        let session_id = request.request.session_id.clone();
        let size = request
            .request
            .initial_pty_size
            .clone()
            .unwrap_or(ResizePayload { rows: 24, cols: 80 });
        let label = command_label(&request.request.executable);
        match &mut self.engine {
            DaemonEngine::Worker(engine) => {
                match engine.begin_spawn_reserved(&reservation, request.request, &request.metadata)
                {
                    Ok(()) => {
                        self.pending.insert(
                            id,
                            PendingState {
                                kind: PendingKind::Spawn {
                                    reservation: Some(reservation),
                                    session_id,
                                    metadata: request.metadata,
                                    size,
                                    label,
                                    now_seconds,
                                },
                                deadline: None,
                                cancelled: false,
                            },
                        );
                    }
                    Err(error) => self.completions.push(CoreCompletion::SpawnReserved {
                        id,
                        result: ReservedSpawnResult::Refused {
                            error: error.into(),
                        },
                    }),
                }
            }
            DaemonEngine::Local(engine) => {
                let result = match engine.spawn_reserved_session(
                    &reservation,
                    request.request,
                    request.metadata,
                ) {
                    Ok(spawn) => {
                        match self.finish_spawn_registry(spawn, size, label, now_seconds) {
                            Ok(session) => ReservedSpawnResult::Installed { session },
                            Err(error) => ReservedSpawnResult::AdmittedFailure {
                                error,
                                state: reservation.state(),
                            },
                        }
                    }
                    Err(error) if reserved_launch_was_admitted(&error) => {
                        ReservedSpawnResult::AdmittedFailure {
                            error: error.into(),
                            state: reservation.state(),
                        }
                    }
                    Err(error) => ReservedSpawnResult::Refused {
                        error: error.into(),
                    },
                };
                self.completions
                    .push(CoreCompletion::SpawnReserved { id, result });
            }
        }
    }

    fn finish_spawn_registry(
        &mut self,
        spawn: botster_core::BotsterSpawnOutcome,
        size: ResizePayload,
        label: String,
        now_seconds: u64,
    ) -> Result<CoreSession, CoreDaemonError> {
        let session_id = spawn.session.session_id.clone();
        self.track_live_session(&session_id);
        let mut record = RegistryRecord::running(
            session_id,
            Some(spawn.handle.process),
            size,
            label,
            now_seconds,
        );
        record.metadata = spawn.session.metadata.clone();
        self.fence_baseline_before_save(&record.session_id)?;
        if let Some(metadata) = self.engine.worker_metadata(&record.session_id) {
            if let Some(identity) = metadata.recovery_identity.clone() {
                record.observe_restart_contract(identity, now_seconds);
            }
        }
        self.registry.save(&record)?;
        self.append_lifecycle_upsert(&record, Some(spawn.session.lifecycle.clone()));
        Ok(spawn.session)
    }

    fn begin_shutdown(
        &mut self,
        id: PendingOperationId,
        session_id: SessionId,
    ) -> Result<(), CoreDaemonError> {
        let now_seconds = unix_now_seconds();
        if let DaemonEngine::Local(_) = &self.engine {
            let result = self.shutdown_session(session_id, now_seconds);
            self.completions
                .push(CoreCompletion::ShutdownSession { id, result });
            return Ok(());
        }
        self.ensure_session(&session_id)?;
        let shutdown =
            self.engine
                .shutdown_session(session_id.clone(), "daemon shutdown", now_seconds);
        match shutdown {
            Ok(output) => {
                let drain = drain_result_from_engine_output(output);
                let observations = drain.observations.clone();
                self.retain_pending_drain_result(&session_id, drain);
                self.commit_terminal_lifecycle(&session_id, &observations, now_seconds)?;
            }
            Err(error) => {
                self.completions.push(CoreCompletion::ShutdownSession {
                    id,
                    result: Err(error.into()),
                });
                return Ok(());
            }
        }
        self.pending.insert(
            id,
            PendingState {
                kind: PendingKind::ShutdownSession {
                    session_id,
                    now_seconds,
                },
                deadline: Some(Instant::now() + SHUTDOWN_DEADLINE),
                cancelled: false,
            },
        );
        Ok(())
    }

    fn begin_read_screen(
        &mut self,
        id: PendingOperationId,
        request: ReadScreenRequest,
    ) -> Result<(), CoreDaemonError> {
        let session_id = request.session_id.clone();
        match self.readback_source(&session_id, request.now_seconds)? {
            ReadbackSource::Retained(retained) => {
                self.completions.push(CoreCompletion::ReadScreen {
                    id,
                    result: Ok(ScreenReadback {
                        text: Arc::clone(&retained.screen_text),
                        unavailable: None,
                    }),
                });
                return Ok(());
            }
            ReadbackSource::Unavailable(reason) => {
                self.completions.push(CoreCompletion::ReadScreen {
                    id,
                    result: Ok(ScreenReadback {
                        text: Arc::from(""),
                        unavailable: Some(reason),
                    }),
                });
                return Ok(());
            }
            ReadbackSource::Live => {}
        }
        if self.pending_readbacks(&session_id) >= MAX_PENDING_READBACKS_PER_SESSION {
            return Err(CoreDaemonError::PendingLimit(
                crate::operation::PendingLimitKind::ReadbacksPerSession,
            ));
        }
        match &mut self.engine {
            DaemonEngine::Local(engine) => {
                let result = engine
                    .read_screen(request.request_id.clone(), session_id, request.now_seconds)
                    .map_err(CoreDaemonError::Engine)
                    .and_then(|mut output| {
                        let screen = take_screen_ready(&mut output, &request.request_id)?;
                        self.pending_drain.push(PendingDrainResult {
                            session_id: request.session_id.clone(),
                            result: drain_result_from_engine_output(output),
                        });
                        Ok(ScreenReadback {
                            text: Arc::from(screen.text),
                            unavailable: None,
                        })
                    });
                self.completions
                    .push(CoreCompletion::ReadScreen { id, result });
                Ok(())
            }
            DaemonEngine::Worker(engine) => {
                let probe_id = engine.begin_screen_probe(&session_id)?;
                self.pending.insert(
                    id,
                    PendingState {
                        kind: PendingKind::ReadScreen {
                            session_id,
                            probe_id,
                        },
                        deadline: Some(Instant::now() + self.config.worker_reply_timeout),
                        cancelled: false,
                    },
                );
                Ok(())
            }
        }
    }

    fn begin_read_mode_flags(
        &mut self,
        id: PendingOperationId,
        request: ReadModeFlagsRequest,
    ) -> Result<(), CoreDaemonError> {
        let session_id = request.session_id.clone();
        match self.readback_source(&session_id, request.now_seconds)? {
            ReadbackSource::Retained(retained) => {
                self.completions.push(CoreCompletion::ReadModeFlags {
                    id,
                    result: Ok(ModeFlagsReadback {
                        mode_flags: ModeFlags::from_mode_bits(retained.mode_bits),
                        rows: retained.rows,
                        cols: retained.cols,
                        unavailable: None,
                    }),
                });
                return Ok(());
            }
            ReadbackSource::Unavailable(reason) => {
                self.completions.push(CoreCompletion::ReadModeFlags {
                    id,
                    result: Ok(ModeFlagsReadback {
                        mode_flags: ModeFlags::default(),
                        rows: 0,
                        cols: 0,
                        unavailable: Some(reason),
                    }),
                });
                return Ok(());
            }
            ReadbackSource::Live => {}
        }
        if self.pending_readbacks(&session_id) >= MAX_PENDING_READBACKS_PER_SESSION {
            return Err(CoreDaemonError::PendingLimit(
                crate::operation::PendingLimitKind::ReadbacksPerSession,
            ));
        }
        match &mut self.engine {
            DaemonEngine::Local(engine) => {
                let result = engine
                    .capture_terminal_state(&session_id)
                    .map_err(CoreDaemonError::Engine)
                    .and_then(|(screen, _, mode_flags)| {
                        let mode_flags = mode_flags.map_err(managed_terminal_backend_error)?;
                        Ok(ModeFlagsReadback {
                            mode_flags,
                            rows: screen.size.rows,
                            cols: screen.size.cols,
                            unavailable: None,
                        })
                    });
                self.completions
                    .push(CoreCompletion::ReadModeFlags { id, result });
                Ok(())
            }
            DaemonEngine::Worker(engine) => {
                let probe_id = engine.begin_mode_flags_probe(&session_id)?;
                self.pending.insert(
                    id,
                    PendingState {
                        kind: PendingKind::ReadModeFlags {
                            session_id,
                            probe_id,
                        },
                        deadline: Some(Instant::now() + self.config.worker_reply_timeout),
                        cancelled: false,
                    },
                );
                Ok(())
            }
        }
    }

    fn begin_capture_snapshot(
        &mut self,
        id: PendingOperationId,
        request: CaptureSnapshotRequest,
        owner: CaptureOwner,
    ) -> Result<(), CoreDaemonError> {
        let session_id = request.session_id.clone();
        if self.open_captures_for(&owner) >= MAX_OPEN_CAPTURES_PER_CLIENT {
            return Err(CoreDaemonError::PendingLimit(
                crate::operation::PendingLimitKind::CapturesPerClient,
            ));
        }
        match self.readback_source(&session_id, request.now_seconds)? {
            ReadbackSource::Retained(retained) => {
                let result = match retained.snapshot.as_ref() {
                    Some(snapshot) => Ok(self.open_capture(
                        owner,
                        Arc::clone(snapshot),
                        retained.rows,
                        retained.cols,
                        retained.color_profile.clone(),
                    )),
                    None => Ok(unavailable_capture(
                        HistoryUnavailableReason::CaptureFailed,
                        retained.color_profile.clone(),
                    )),
                };
                self.completions
                    .push(CoreCompletion::CaptureSnapshot { id, result });
                return Ok(());
            }
            ReadbackSource::Unavailable(reason) => {
                self.completions.push(CoreCompletion::CaptureSnapshot {
                    id,
                    result: Ok(unavailable_capture(reason, TerminalColorProfile::default())),
                });
                return Ok(());
            }
            ReadbackSource::Live => {}
        }
        if self.pending_readbacks(&session_id) >= MAX_PENDING_READBACKS_PER_SESSION {
            return Err(CoreDaemonError::PendingLimit(
                crate::operation::PendingLimitKind::ReadbacksPerSession,
            ));
        }
        match &mut self.engine {
            DaemonEngine::Local(engine) => {
                let captured = engine
                    .capture_color_and_snapshot(&session_id)
                    .map_err(CoreDaemonError::Engine);
                let result = match captured {
                    Ok((color_profile, payload)) => {
                        let rows = payload.size.rows;
                        let cols = payload.size.cols;
                        Ok(self.open_capture(
                            owner,
                            Arc::from(payload.bytes),
                            rows,
                            cols,
                            color_profile,
                        ))
                    }
                    Err(error) => Err(error),
                };
                self.completions
                    .push(CoreCompletion::CaptureSnapshot { id, result });
                Ok(())
            }
            DaemonEngine::Worker(engine) => {
                let host_capture = engine.begin_host_capture(&session_id);
                self.pending.insert(
                    id,
                    PendingState {
                        kind: PendingKind::CaptureSnapshot {
                            session_id,
                            owner,
                            host_capture,
                        },
                        deadline: Some(Instant::now() + self.config.worker_reply_timeout),
                        cancelled: false,
                    },
                );
                Ok(())
            }
        }
    }

    fn open_capture(
        &mut self,
        owner: CaptureOwner,
        bytes: Arc<[u8]>,
        rows: u16,
        cols: u16,
        color_profile: TerminalColorProfile,
    ) -> SnapshotCapture {
        let capture_id = CaptureId(format!(
            "capture-{}-{}",
            self.lifecycle_source_id.0, self.next_capture
        ));
        self.next_capture += 1;
        let total_bytes = bytes.len() as u64;
        let pages = bytes.len().div_ceil(SNAPSHOT_PAGE_BYTES).max(1) as u32;
        self.open_captures.insert(
            capture_id.clone(),
            OpenCapture {
                owner,
                bytes,
                last_touched: Instant::now(),
            },
        );
        SnapshotCapture {
            capture_id,
            total_bytes,
            page_bytes: SNAPSHOT_PAGE_BYTES as u32,
            pages,
            rows,
            cols,
            color_profile,
            unavailable: None,
        }
    }

    /// Reconcile worker replies, deadlines, and expiries for pending operations.
    fn reconcile_pending(&mut self, now_seconds: u64) {
        let now = Instant::now();
        // Drain worker probe replies once per session and hand each reply to
        // the pending operation that owns its request id, so concurrent
        // readbacks never consume each other's replies.
        let mut screen_replies: HashMap<String, botster_core::ScreenPayload> = HashMap::new();
        let mut mode_replies: HashMap<String, botster_core::ModeFlagsPayload> = HashMap::new();
        if let DaemonEngine::Worker(engine) = &mut self.engine {
            let mut screen_sessions = HashSet::new();
            let mut mode_sessions = HashSet::new();
            for state in self.pending.values() {
                match &state.kind {
                    PendingKind::ReadScreen { session_id, .. } => {
                        screen_sessions.insert(session_id.clone());
                    }
                    PendingKind::ReadModeFlags { session_id, .. } => {
                        mode_sessions.insert(session_id.clone());
                    }
                    _ => {}
                }
            }
            for session_id in screen_sessions {
                for reply in engine.take_screen_replies(&session_id).unwrap_or_default() {
                    screen_replies.insert(reply.request_id.clone(), reply);
                }
            }
            for session_id in mode_sessions {
                for reply in engine
                    .take_mode_flags_replies(&session_id)
                    .unwrap_or_default()
                {
                    mode_replies.insert(reply.request_id.clone(), reply);
                }
            }
        }
        self.resolve_pending_readbacks(&mut screen_replies, &mut mode_replies);
        let ids: Vec<_> = self.pending.keys().copied().collect();
        for id in ids {
            let Some(state) = self.pending.get(&id) else {
                continue;
            };
            let expired = state.deadline.is_some_and(|deadline| deadline <= now);
            let cancelled = state.cancelled;
            let completion = match &state.kind {
                PendingKind::Spawn {
                    reservation,
                    session_id,
                    metadata,
                    size,
                    label,
                    now_seconds: spawn_at,
                } => {
                    let DaemonEngine::Worker(engine) = &mut self.engine else {
                        continue;
                    };
                    if cancelled {
                        // Completion already emitted. Wait until the runtime
                        // collected and stopped the abandoned launch.
                        match engine.poll_spawn(
                            session_id,
                            metadata.clone(),
                            TerminalScreenSize::new(size.rows, size.cols),
                        ) {
                            Ok(None) => continue,
                            Ok(Some(_)) | Err(_) => {
                                self.pending.remove(&id);
                                continue;
                            }
                        }
                    }
                    let polled = engine.poll_spawn(
                        session_id,
                        metadata.clone(),
                        TerminalScreenSize::new(size.rows, size.cols),
                    );
                    match polled {
                        Ok(None) => continue,
                        Ok(Some(spawn)) => {
                            let reservation = reservation.clone();
                            let (size, label, spawn_at) = (size.clone(), label.clone(), *spawn_at);
                            let result = self.finish_spawn_registry(spawn, size, label, spawn_at);
                            match reservation {
                                Some(reservation) => CoreCompletion::SpawnReserved {
                                    id,
                                    result: match result {
                                        Ok(session) => ReservedSpawnResult::Installed { session },
                                        Err(error) => ReservedSpawnResult::AdmittedFailure {
                                            error,
                                            state: reservation.state(),
                                        },
                                    },
                                },
                                None => CoreCompletion::Spawn { id, result },
                            }
                        }
                        Err(error) => match reservation {
                            Some(reservation) => CoreCompletion::SpawnReserved {
                                id,
                                result: ReservedSpawnResult::AdmittedFailure {
                                    error: error.into(),
                                    state: reservation.state(),
                                },
                            },
                            None => CoreCompletion::Spawn {
                                id,
                                result: Err(error.into()),
                            },
                        },
                    }
                }
                PendingKind::ShutdownSession {
                    session_id,
                    now_seconds: shutdown_at,
                } => {
                    let session_id = session_id.clone();
                    let shutdown_at = *shutdown_at;
                    if self.engine_session_exited(&session_id)
                        || self.engine.session(&session_id).is_none()
                    {
                        let result = self.finish_shutdown_registry(&session_id, shutdown_at);
                        CoreCompletion::ShutdownSession { id, result }
                    } else if expired {
                        CoreCompletion::ShutdownSession {
                            id,
                            result: Err(CoreDaemonError::DeadlineExpired),
                        }
                    } else {
                        continue;
                    }
                }
                PendingKind::ReadScreen {
                    session_id,
                    probe_id,
                } => {
                    let DaemonEngine::Worker(engine) = &mut self.engine else {
                        continue;
                    };
                    let session_id = session_id.clone();
                    let probe_id = probe_id.clone();
                    // Matching replies were resolved before this loop.
                    let matched = screen_replies.remove(&probe_id);
                    match matched {
                        Some(reply) => CoreCompletion::ReadScreen {
                            id,
                            result: match reply.error_kind {
                                Some(error) => Err(CoreDaemonError::Engine(
                                    DefaultBotsterEngineError::TerminalBackendOperation {
                                        operation: "read_screen",
                                        message: error,
                                    },
                                )),
                                None => Ok(ScreenReadback {
                                    text: Arc::from(reply.text),
                                    unavailable: None,
                                }),
                            },
                        },
                        None if engine.session(&session_id).is_none()
                            || engine.worker_link_ended(&session_id) =>
                        {
                            CoreCompletion::ReadScreen {
                                id,
                                result: Err(CoreDaemonError::WorkerLinkFailed(session_id)),
                            }
                        }
                        None if expired => CoreCompletion::ReadScreen {
                            id,
                            result: Err(CoreDaemonError::DeadlineExpired),
                        },
                        None => continue,
                    }
                }
                PendingKind::ReadModeFlags {
                    session_id,
                    probe_id,
                } => {
                    let DaemonEngine::Worker(engine) = &mut self.engine else {
                        continue;
                    };
                    let session_id = session_id.clone();
                    let probe_id = probe_id.clone();
                    let matched = mode_replies.remove(&probe_id);
                    match matched {
                        Some(reply) => CoreCompletion::ReadModeFlags {
                            id,
                            result: match reply.error_kind {
                                Some(error) => Err(CoreDaemonError::Engine(
                                    DefaultBotsterEngineError::TerminalBackendOperation {
                                        operation: "read_mode_flags",
                                        message: error,
                                    },
                                )),
                                None => Ok(ModeFlagsReadback {
                                    mode_flags: reply.mode_flags,
                                    rows: reply.rows,
                                    cols: reply.cols,
                                    unavailable: None,
                                }),
                            },
                        },
                        None if engine.session(&session_id).is_none()
                            || engine.worker_link_ended(&session_id) =>
                        {
                            CoreCompletion::ReadModeFlags {
                                id,
                                result: Err(CoreDaemonError::WorkerLinkFailed(session_id)),
                            }
                        }
                        None if expired => CoreCompletion::ReadModeFlags {
                            id,
                            result: Err(CoreDaemonError::DeadlineExpired),
                        },
                        None => continue,
                    }
                }
                PendingKind::CaptureSnapshot {
                    session_id,
                    owner,
                    host_capture,
                } => {
                    let DaemonEngine::Worker(engine) = &mut self.engine else {
                        continue;
                    };
                    let session_id = session_id.clone();
                    let owner = owner.clone();
                    let host_capture = *host_capture;
                    match engine.take_host_capture(host_capture) {
                        Some(Ok(captured)) => {
                            let rows = captured.size.rows;
                            let cols = captured.size.cols;
                            let capture = self.open_capture(
                                owner,
                                Arc::from(captured.bytes),
                                rows,
                                cols,
                                captured.color_profile,
                            );
                            CoreCompletion::CaptureSnapshot {
                                id,
                                result: Ok(capture),
                            }
                        }
                        Some(Err(message)) => CoreCompletion::CaptureSnapshot {
                            id,
                            result: Err(CoreDaemonError::Engine(
                                DefaultBotsterEngineError::TerminalBackendOperation {
                                    operation: "capture_snapshot",
                                    message,
                                },
                            )),
                        },
                        None if engine.session(&session_id).is_none()
                            || engine.worker_link_ended(&session_id) =>
                        {
                            engine.cancel_host_capture(&session_id, host_capture);
                            CoreCompletion::CaptureSnapshot {
                                id,
                                result: Err(CoreDaemonError::WorkerLinkFailed(session_id)),
                            }
                        }
                        None if expired => {
                            engine.cancel_host_capture(&session_id, host_capture);
                            CoreCompletion::CaptureSnapshot {
                                id,
                                result: Err(CoreDaemonError::DeadlineExpired),
                            }
                        }
                        None => continue,
                    }
                }
            };
            self.pending.remove(&id);
            self.completions.push(completion);
        }
        let _ = now_seconds;
        let ttl = Duration::from_secs(CAPTURE_IDLE_TTL_SECONDS);
        self.open_captures
            .retain(|_, open| now.saturating_duration_since(open.last_touched) < ttl);
    }

    #[cfg(test)]
    fn test_insert_pending_read_screen(
        &mut self,
        session_id: SessionId,
        probe_id: &str,
    ) -> PendingOperationId {
        let id = self.allocate_pending_id().expect("test operation identity");
        self.pending.insert(
            id,
            PendingState {
                kind: PendingKind::ReadScreen {
                    session_id,
                    probe_id: probe_id.to_owned(),
                },
                deadline: None,
                cancelled: false,
            },
        );
        id
    }

    #[cfg(test)]
    fn test_insert_pending_capture(
        &mut self,
        session_id: SessionId,
        owner: CaptureOwner,
        host_capture: u64,
    ) -> PendingOperationId {
        let id = self.allocate_pending_id().expect("test operation identity");
        self.pending.insert(
            id,
            PendingState {
                kind: PendingKind::CaptureSnapshot {
                    session_id,
                    owner,
                    host_capture,
                },
                deadline: None,
                cancelled: false,
            },
        );
        id
    }

    /// Resolve pending readbacks against replies already drained from the
    /// worker. Split out so the distribution rule is testable without a
    /// worker process.
    fn resolve_pending_readbacks(
        &mut self,
        screen_replies: &mut HashMap<String, botster_core::ScreenPayload>,
        mode_replies: &mut HashMap<String, botster_core::ModeFlagsPayload>,
    ) {
        let ids: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, state)| {
                matches!(
                    state.kind,
                    PendingKind::ReadScreen { .. } | PendingKind::ReadModeFlags { .. }
                )
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(state) = self.pending.get(&id) else {
                continue;
            };
            let completion = match &state.kind {
                PendingKind::ReadScreen { probe_id, .. } => {
                    let Some(reply) = screen_replies.remove(probe_id) else {
                        continue;
                    };
                    CoreCompletion::ReadScreen {
                        id,
                        result: match reply.error_kind {
                            Some(error) => Err(CoreDaemonError::Engine(
                                DefaultBotsterEngineError::TerminalBackendOperation {
                                    operation: "read_screen",
                                    message: error,
                                },
                            )),
                            None => Ok(ScreenReadback {
                                text: Arc::from(reply.text),
                                unavailable: None,
                            }),
                        },
                    }
                }
                PendingKind::ReadModeFlags { probe_id, .. } => {
                    let Some(reply) = mode_replies.remove(probe_id) else {
                        continue;
                    };
                    CoreCompletion::ReadModeFlags {
                        id,
                        result: match reply.error_kind {
                            Some(error) => Err(CoreDaemonError::Engine(
                                DefaultBotsterEngineError::TerminalBackendOperation {
                                    operation: "read_mode_flags",
                                    message: error,
                                },
                            )),
                            None => Ok(ModeFlagsReadback {
                                mode_flags: reply.mode_flags,
                                rows: reply.rows,
                                cols: reply.cols,
                                unavailable: None,
                            }),
                        },
                    }
                }
                _ => continue,
            };
            self.pending.remove(&id);
            self.completions.push(completion);
        }
    }

    fn finish_shutdown_registry(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.retain_final_terminal_state(session_id, now_seconds)?;
        if let Some(mut record) = self.registry.load(session_id)? {
            if record.state != RegistrySessionState::Exited {
                record.mark(RegistrySessionState::Exited, now_seconds);
                self.fence_baseline_before_save(session_id)?;
                self.registry.save(&record)?;
                let lifecycle = self
                    .engine
                    .session(session_id)
                    .map(|session| session.lifecycle.clone());
                self.append_lifecycle_upsert(&record, lifecycle);
            }
        }
        self.cleanup_worker_socket_dir_if_empty();
        Ok(())
    }

    /// Resolve where a readback for `session_id` comes from.
    fn readback_source(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<ReadbackSource, CoreDaemonError> {
        let registry_state = self.registry.load(session_id)?.map(|record| record.state);
        if matches!(registry_state, Some(RegistrySessionState::Stale)) {
            return Err(CoreDaemonError::SessionNotReadable(session_id.clone()));
        }
        if let Some(retained) = self.retained_terminal.get(session_id) {
            return Ok(ReadbackSource::Retained(Arc::clone(retained)));
        }
        if let Some(reason) = self.retained_unavailable.get(session_id) {
            return Ok(ReadbackSource::Unavailable(*reason));
        }
        let lifecycle = self
            .engine
            .session(session_id)
            .map(|session| session.lifecycle.clone());
        let Some(lifecycle) = lifecycle else {
            return match registry_state {
                Some(RegistrySessionState::Exited | RegistrySessionState::Stopping) => {
                    // Ended before this incarnation retained anything.
                    Ok(ReadbackSource::Unavailable(
                        HistoryUnavailableReason::Restart,
                    ))
                }
                Some(_) => Err(CoreDaemonError::SessionNotReadable(session_id.clone())),
                None => Err(CoreDaemonError::UnknownSession(session_id.clone())),
            };
        };
        if matches!(lifecycle, SessionLifecycleState::Exited { .. }) {
            self.retain_final_terminal_state(session_id, now_seconds)?;
            return Ok(self
                .retained_terminal
                .get(session_id)
                .map(|retained| ReadbackSource::Retained(Arc::clone(retained)))
                .or_else(|| {
                    self.retained_unavailable
                        .get(session_id)
                        .map(|reason| ReadbackSource::Unavailable(*reason))
                })
                .unwrap_or(ReadbackSource::Unavailable(
                    HistoryUnavailableReason::CaptureFailed,
                )));
        }
        if matches!(
            lifecycle,
            SessionLifecycleState::Stopping | SessionLifecycleState::Failed { .. }
        ) || matches!(
            registry_state,
            Some(RegistrySessionState::Stopping | RegistrySessionState::Exited)
        ) {
            return Err(CoreDaemonError::SessionNotReadable(session_id.clone()));
        }
        Ok(ReadbackSource::Live)
    }

    /// Retain the final terminal state of an exited session under the policy.
    fn retain_final_terminal_state(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        if self.retained_terminal.contains_key(session_id)
            || self.retained_unavailable.contains_key(session_id)
        {
            return Ok(());
        }
        let retained = match &mut self.engine {
            DaemonEngine::Worker(engine) => match engine.take_final_state(session_id) {
                Some(final_state) => {
                    let screen_text: Arc<str> = Arc::from(final_state.state.screen_text);
                    let snapshot: Option<Arc<[u8]>> = final_state.snapshot.map(Arc::from);
                    let bytes =
                        RetainedTerminal::accounted_bytes(&screen_text, snapshot.as_deref());
                    RetainedTerminal {
                        screen_text,
                        snapshot,
                        mode_bits: final_state.state.mode_bits,
                        rows: final_state.state.rows,
                        cols: final_state.state.cols,
                        color_profile: final_state.state.color_profile,
                        exited_at: now_seconds,
                        bytes,
                    }
                }
                None => {
                    self.retained_unavailable
                        .insert(session_id.clone(), HistoryUnavailableReason::CaptureFailed);
                    return Ok(());
                }
            },
            DaemonEngine::Local(engine) => {
                let (screen, snapshot, mode_flags) = engine.capture_terminal_state(session_id)?;
                let color_profile = screen.color_profile.ok_or_else(|| {
                    managed_terminal_backend_error(TerminalBackendError::operation_failed(
                        "color_profile",
                        "terminal did not expose a color profile for retained freeze",
                    ))
                })?;
                let screen_text: Arc<str> = Arc::from(screen.plain_text);
                let snapshot: Option<Arc<[u8]>> = Some(Arc::from(snapshot.bytes));
                let bytes = RetainedTerminal::accounted_bytes(&screen_text, snapshot.as_deref());
                RetainedTerminal {
                    screen_text,
                    snapshot,
                    mode_bits: mode_flags.map(|flags| flags.to_mode_bits()).unwrap_or(0),
                    rows: screen.size.rows,
                    cols: screen.size.cols,
                    color_profile,
                    exited_at: now_seconds,
                    bytes,
                }
            }
        };
        self.admit_retained(session_id, retained);
        Ok(())
    }

    /// Apply the retention policy: refuse oversize objects, evict the oldest.
    fn admit_retained(&mut self, session_id: &SessionId, retained: RetainedTerminal) {
        let policy = self.config.retention;
        // Every unconditional refusal is decided before any eviction so an
        // object that can never fit does not displace valid history.
        if retained.bytes > policy.max_object_bytes
            || retained.bytes > policy.max_total_bytes
            || policy.max_sessions == 0
        {
            self.retention_accounting.oversize_refusals += 1;
            self.retained_unavailable
                .insert(session_id.clone(), HistoryUnavailableReason::Oversize);
            return;
        }
        while !self.retained_terminal.is_empty()
            && (self.retention_accounting.total_bytes + retained.bytes > policy.max_total_bytes
                || self.retention_accounting.sessions + 1 > policy.max_sessions)
        {
            let oldest = self
                .retained_terminal
                .iter()
                .min_by_key(|(id, entry)| (entry.exited_at, id.0.clone()))
                .map(|(id, _)| id.clone());
            let Some(oldest) = oldest else {
                break;
            };
            self.evict_retained(&oldest);
        }
        self.retention_accounting.total_bytes += retained.bytes;
        self.retention_accounting.sessions += 1;
        self.retained_terminal
            .insert(session_id.clone(), Arc::new(retained));
    }

    fn evict_retained(&mut self, session_id: &SessionId) {
        if let Some(entry) = self.retained_terminal.remove(session_id) {
            self.retention_accounting.total_bytes = self
                .retention_accounting
                .total_bytes
                .saturating_sub(entry.bytes);
            self.retention_accounting.sessions =
                self.retention_accounting.sessions.saturating_sub(1);
            self.retention_accounting.evictions += 1;
            self.retained_unavailable
                .insert(session_id.clone(), HistoryUnavailableReason::Evicted);
        }
    }

    fn forget_retained(&mut self, session_id: &SessionId) {
        if let Some(entry) = self.retained_terminal.remove(session_id) {
            self.retention_accounting.total_bytes = self
                .retention_accounting
                .total_bytes
                .saturating_sub(entry.bytes);
            self.retention_accounting.sessions =
                self.retention_accounting.sessions.saturating_sub(1);
        }
        self.retained_unavailable.remove(session_id);
    }

    /// Queue one policy-free notification inbox item.
    pub fn post_notification(
        &mut self,
        request: PostNotificationRequest,
    ) -> Result<PostNotificationResult, CoreDaemonError> {
        self.ensure_running()?;
        let id = self.notification_inbox.post(request.item);
        Ok(PostNotificationResult { id })
    }

    /// Drain deliverable notifications for one target exactly once.
    pub fn drain_notifications(
        &mut self,
        request: DrainNotificationsRequest,
    ) -> Result<DrainNotificationsResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(DrainNotificationsResult {
            items: self.notification_inbox.drain(&request.target, request.now),
        })
    }

    /// Acknowledge one notification inbox item.
    pub fn acknowledge_notification(
        &mut self,
        request: AcknowledgeNotificationRequest,
    ) -> Result<NotificationStatusResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(NotificationStatusResult {
            status: self.notification_inbox.acknowledge(&request.id),
        })
    }

    /// Return notification delivery status without changing daemon state.
    pub fn notification_status(&self, id: &NotificationId) -> NotificationStatusResult {
        NotificationStatusResult {
            status: self.notification_inbox.status(id),
        }
    }

    /// Publish one generic routed envelope through the daemon-owned router.
    pub fn publish_routed_envelope(
        &mut self,
        request: PublishRoutedEnvelopeRequest,
    ) -> Result<PublishRoutedEnvelopeResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(self.envelope_router.publish(request.envelope))
    }

    /// Drain routed envelopes for one target with cursor and limit semantics.
    pub fn drain_routed_envelopes(
        &mut self,
        request: DrainRoutedEnvelopesRequest,
    ) -> Result<DrainRoutedEnvelopesResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(self
            .envelope_router
            .drain(&request.target, request.after, request.limit))
    }

    /// Acknowledge one routed envelope target copy.
    pub fn acknowledge_routed_envelope(
        &mut self,
        request: AcknowledgeRoutedEnvelopeRequest,
    ) -> Result<RoutedEnvelopeDeliveryStateResult, CoreDaemonError> {
        self.ensure_running()?;
        Ok(RoutedEnvelopeDeliveryStateResult {
            state: self
                .envelope_router
                .acknowledge(&request.target, &request.envelope_id),
        })
    }

    /// Return one routed envelope delivery state without changing daemon state.
    pub fn routed_envelope_delivery_state(
        &self,
        target: &EnvelopeTarget,
        envelope_id: &EnvelopeId,
    ) -> RoutedEnvelopeDeliveryStateResult {
        RoutedEnvelopeDeliveryStateResult {
            state: self
                .envelope_router
                .delivery_state(target, envelope_id)
                .cloned(),
        }
    }

    /// Subscribe output by attaching a client and returning its initial output.
    pub fn subscribe_output(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<AttachedSession, CoreDaemonError> {
        self.attach(client_id, session_id, subscription_id, now_seconds)
    }

    /// Evaluate readiness and inject only through the existing PTY input path.
    pub fn guarded_write(
        &mut self,
        request: GuardedWriteRequest,
    ) -> Result<GuardedWriteResult, CoreDaemonError> {
        self.ensure_running()?;
        if self.engine.session(&request.session_id).is_none() {
            return Ok(GuardedWriteResult {
                decision: GuardedWriteDecision::Reject {
                    reason: "unknown session".to_string(),
                },
                states: vec![
                    GuardedWriteDeliveryState::Accepted,
                    GuardedWriteDeliveryState::Rejected,
                ],
            });
        }
        self.ensure_session_mutable(&request.session_id)?;

        let decision = decide_guarded_write(&request.readiness);
        let mut states = vec![GuardedWriteDeliveryState::Accepted];
        match &decision {
            GuardedWriteDecision::Write => {
                self.engine.write_bytes(
                    request.client_id,
                    request.session_id,
                    request.data,
                    request.now_seconds,
                )?;
                states.push(GuardedWriteDeliveryState::Written);
            }
            GuardedWriteDecision::Defer { .. } => {
                states.push(GuardedWriteDeliveryState::Deferred);
            }
            GuardedWriteDecision::Reject { .. } => {
                states.push(GuardedWriteDeliveryState::Rejected);
            }
        }

        Ok(GuardedWriteResult { decision, states })
    }

    /// Return daemon health.
    pub fn health(&self) -> Result<DaemonHealth, CoreDaemonError> {
        Ok(DaemonHealth {
            running: self.running,
            live_sessions: self.engine.list_sessions().len(),
            registry_records: self.load_all_records()?.len(),
            data_dir: self.config.data_dir.display().to_string(),
        })
    }

    /// Return daemon status.
    pub fn status(&self) -> Result<DaemonStatus, CoreDaemonError> {
        Ok(DaemonStatus {
            health: self.health()?,
            sessions: self.list()?,
        })
    }

    /// Scan persisted records for follow-up restart/adoption work.
    pub fn adoption_scan(&self) -> Result<Vec<SessionAdoptionReport>, CoreDaemonError> {
        Ok(self
            .load_all_records()?
            .into_iter()
            .map(|record| {
                let live_candidates = adoption_candidate_count(&self.engine, &record);
                let state = if matches!(
                    record.state,
                    RegistrySessionState::Stopping
                        | RegistrySessionState::Exited
                        | RegistrySessionState::Stale
                ) {
                    SessionAdoptionState::Terminal
                } else if live_candidates > 1 {
                    SessionAdoptionState::DuplicateWorker {
                        candidates: live_candidates,
                    }
                } else if record.protocol_version != botster_core::PROTOCOL_VERSION {
                    SessionAdoptionState::StaleWorker {
                        reason: SessionWorkerStaleReason::IncompatibleProtocol,
                    }
                } else if record.handshake_verified
                    && record.ping_pong_supported
                    && record.recovery_identity.is_some()
                    && self.engine.session(&record.session_id).is_none()
                    && !has_worker_control_socket(&record)
                {
                    SessionAdoptionState::StaleWorker {
                        reason: stale_worker_reason(&record),
                    }
                } else if record.handshake_verified
                    && record.recovery_identity.is_some()
                    && !record.ping_pong_supported
                {
                    SessionAdoptionState::UnhealthyWorker {
                        reason: SessionWorkerHealthReason::MissedHeartbeat,
                    }
                } else if record.handshake_verified
                    && record.ping_pong_supported
                    && record.recovery_identity.is_some()
                {
                    if self.config.worker_path.is_none() {
                        SessionAdoptionState::InProcessDaemonNotRestartDurable
                    } else {
                        SessionAdoptionState::Adoptable
                    }
                } else {
                    SessionAdoptionState::MissingProtocolEvidence
                };
                SessionAdoptionReport { record, state }
            })
            .collect())
    }

    /// Explicitly mark a registry record stale after a read-only adoption scan.
    pub fn mark_stale(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        if let Some(mut record) = self.registry.load(session_id)? {
            if record.state != RegistrySessionState::Stale {
                record.mark(RegistrySessionState::Stale, now_seconds);
                self.fence_baseline_before_save(session_id)?;
                self.registry.save(&record)?;
                let lifecycle = self
                    .engine
                    .session(session_id)
                    .map(|session| session.lifecycle.clone());
                self.append_lifecycle_upsert(&record, lifecycle);
            }
        }
        self.cleanup_worker_socket_dir_if_empty();
        Ok(())
    }

    /// Adopt a live worker-backed session from durable registry metadata.
    pub fn adopt_session(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<CoreSession, CoreDaemonError> {
        self.ensure_running()?;
        if self.config.worker_path.is_none() {
            return Err(CoreDaemonError::MissingWorkerPath);
        }
        let record = self
            .registry
            .load(session_id)?
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))?;
        let process = record
            .process
            .clone()
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))?;
        let socket_path = worker_control_socket(&record)
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))?;
        let supports_snapshot_boundary =
            record.recovery_identity.as_ref().is_some_and(|identity| {
                identity
                    .get("atomic_snapshot_boundary")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    && identity
                        .get("snapshot_delivery")
                        .and_then(serde_json::Value::as_str)
                        == Some("ready_then_history")
            });
        let session = self.engine.adopt_worker_process(
            session_id.clone(),
            process,
            socket_path,
            supports_snapshot_boundary,
            record.metadata.clone(),
        )?;
        self.track_live_session(session_id);
        if let Some(mut record) = self.registry.load(session_id)? {
            record.mark(RegistrySessionState::Running, now_seconds);
            self.fence_baseline_before_save(session_id)?;
            self.registry.save(&record)?;
            self.append_lifecycle_upsert(&record, Some(session.lifecycle.clone()));
        }
        Ok(session)
    }

    /// Forget one already-terminal session and emit a removal change.
    ///
    /// Retention timing is host policy. This method only provides the
    /// policy-free mechanism. It returns `false` without mutation for live or
    /// stopping sessions and `true` after complete terminal cleanup.
    pub fn remove_session(&mut self, session_id: &SessionId) -> Result<bool, CoreDaemonError> {
        self.ensure_running()?;
        let record = self
            .registry
            .load(session_id)?
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))?;
        if !matches!(
            record.state,
            RegistrySessionState::Exited | RegistrySessionState::Stale
        ) || self.engine.session(session_id).is_some_and(|session| {
            !matches!(
                session.lifecycle,
                SessionLifecycleState::Exited { .. } | SessionLifecycleState::Failed { .. }
            )
        }) {
            return Ok(false);
        }

        self.fence_baseline_before_remove(session_id)?;
        self.registry.remove(session_id)?;
        if self.engine.session(session_id).is_some() {
            let forgotten = self.engine.forget_terminal_session(session_id);
            assert!(
                forgotten,
                "terminal removal precondition must match core engine state"
            );
        }
        self.forget_retained(session_id);
        self.terminal_commit_obligations.remove(session_id);
        self.terminal_commit_failures.remove(session_id);
        self.observe_live_sessions.remove(&session_id.0);
        self.drop_pending_drain(session_id);
        self.append_lifecycle_change(SessionLifecycleChangeKind::Removed {
            session_id: session_id.clone(),
        });
        Ok(true)
    }
    /// Release worker processes for an intentional daemon restart without shutting them down.
    pub fn release_for_restart(&mut self) {
        self.engine.release_workers_for_restart();
    }

    /// Shut down one session or all sessions when `session_id` is absent.
    pub fn shutdown(
        &mut self,
        session_id: Option<SessionId>,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        if let Some(session_id) = session_id {
            self.shutdown_session(session_id, now_seconds)?;
            return Ok(());
        }

        if self.wake_pump.as_ref().is_some_and(|state| {
            !state
                .stop_observed
                .load(std::sync::atomic::Ordering::Acquire)
        }) {
            return Err(WakePumpError::StopNotObserved.into());
        }

        let sessions: Vec<_> = self.engine.list_sessions();
        for session in sessions {
            self.shutdown_session(session.session_id, now_seconds)?;
        }
        self.retained_terminal.clear();
        self.retained_unavailable.clear();
        self.retention_accounting = RetentionAccounting::default();
        self.running = false;
        Ok(())
    }
    fn shutdown_session(
        &mut self,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_session(&session_id)?;
        let (shutdown_drain, shutdown_error) =
            match self
                .engine
                .shutdown_session(session_id.clone(), "daemon shutdown", now_seconds)
            {
                Ok(output) => (drain_result_from_engine_output(output), None),
                Err(error) => (DrainResult::default(), Some(error)),
            };
        let shutdown_observations = shutdown_drain.observations.clone();
        self.retain_pending_drain_result(&session_id, shutdown_drain);
        self.commit_terminal_lifecycle(&session_id, &shutdown_observations, now_seconds)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut final_output_drained = self.engine_session_exited(&session_id);
        while !final_output_drained && Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let batch = self.engine.wake_source().wait_wakes_bounded(remaining);
            match self.pump_woken(&batch, now_seconds) {
                Ok(_) => {
                    final_output_drained = self.engine_session_exited(&session_id);
                }
                Err(CoreDaemonError::Engine(error)) if is_session_not_found(&error) => {
                    final_output_drained = self.engine_session_exited(&session_id);
                    if !final_output_drained {
                        return Err(error.into());
                    }
                }
                Err(error) => return Err(error),
            }
        }
        if !final_output_drained {
            if let Some(error) = shutdown_error {
                return Err(error.into());
            }
            return Err(CoreDaemonError::Engine(DefaultBotsterEngineError::Runtime(
                SessionRuntimeError::new(
                    SessionRuntimeErrorKind::ShutdownFailed,
                    format!(
                        "worker session shutdown did not complete before the daemon deadline: {}",
                        session_id.0
                    ),
                ),
            )));
        }

        self.retain_final_terminal_state(&session_id, now_seconds)?;
        if let Some(mut record) = self.registry.load(&session_id)? {
            if record.state != RegistrySessionState::Exited {
                record.mark(RegistrySessionState::Exited, now_seconds);
                self.fence_baseline_before_save(&session_id)?;
                self.registry.save(&record)?;
                let lifecycle = self
                    .engine
                    .session(&session_id)
                    .map(|session| session.lifecycle.clone());
                self.append_lifecycle_upsert(&record, lifecycle);
            }
        }
        self.cleanup_worker_socket_dir_if_empty();
        Ok(())
    }
    fn reconcile_lifecycle_observations(
        &mut self,
        observations: &[BotsterEngineObservation],
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        let mut terminal_transition = false;
        for observation in observations {
            let BotsterEngineObservation::SessionLifecycle { session_id, state } = observation
            else {
                continue;
            };
            let registry_state = match state {
                botster_core::SessionLifecycleState::Stopping => RegistrySessionState::Stopping,
                botster_core::SessionLifecycleState::Exited { .. } => RegistrySessionState::Exited,
                botster_core::SessionLifecycleState::Failed { .. } => RegistrySessionState::Stale,
                botster_core::SessionLifecycleState::Starting
                | botster_core::SessionLifecycleState::Running => RegistrySessionState::Running,
            };
            if let Some(mut record) = self.registry.load(session_id)? {
                if record.state != registry_state {
                    terminal_transition |= matches!(
                        registry_state,
                        RegistrySessionState::Exited | RegistrySessionState::Stale
                    );
                    record.mark(registry_state, now_seconds);
                    self.fence_baseline_before_save(session_id)?;
                    self.registry.save(&record)?;
                    self.append_lifecycle_upsert(&record, Some(state.clone()));
                }
            }
        }
        if terminal_transition {
            self.cleanup_worker_socket_dir_if_empty();
        }
        Ok(())
    }

    fn commit_terminal_lifecycle(
        &mut self,
        session_id: &SessionId,
        observations: &[BotsterEngineObservation],
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        let obligation = self.terminal_commit_obligations.get(session_id).cloned();
        if let Some(state) = obligation {
            let can_advance = match self.obligation_can_advance(session_id, &state) {
                Ok(can_advance) => can_advance,
                Err(error) => {
                    self.record_terminal_commit_failure(session_id, None);
                    return Err(error);
                }
            };
            if can_advance {
                let obligation_observation = BotsterEngineObservation::SessionLifecycle {
                    session_id: session_id.clone(),
                    state,
                };
                if let Err(error) = self.reconcile_lifecycle_observations(
                    std::slice::from_ref(&obligation_observation),
                    now_seconds,
                ) {
                    self.record_terminal_commit_failure(session_id, None);
                    return Err(error);
                }
            }
            self.terminal_commit_obligations.remove(session_id);
        }

        if let Err(error) = self.reconcile_lifecycle_observations(observations, now_seconds) {
            self.record_terminal_obligations(observations);
            self.record_terminal_commit_failure(session_id, None);
            return Err(error);
        }

        let mut touched = vec![session_id.clone()];
        touched.extend(observations.iter().filter_map(|observation| {
            let BotsterEngineObservation::SessionLifecycle { session_id, .. } = observation else {
                return None;
            };
            Some(session_id.clone())
        }));
        touched.sort_by(|left, right| left.0.cmp(&right.0));
        touched.dedup();
        for touched_session in &touched {
            if self.engine_session_exited(touched_session) {
                if let Err(error) = self.retain_final_terminal_state(touched_session, now_seconds) {
                    let state = self
                        .engine
                        .session(touched_session)
                        .map(|session| session.lifecycle.clone());
                    self.record_terminal_commit_failure(touched_session, state);
                    return Err(error);
                }
            }
        }
        for touched_session in touched {
            if self.engine_session_is_terminal(&touched_session)
                && !self.engine.session_has_undelivered_frames(&touched_session)
            {
                self.engine.wake_source().forget_session(&touched_session);
            }
            self.terminal_commit_obligations.remove(&touched_session);
            self.terminal_commit_failures.remove(&touched_session);
        }
        Ok(())
    }

    fn obligation_can_advance(
        &self,
        session_id: &SessionId,
        state: &SessionLifecycleState,
    ) -> Result<bool, CoreDaemonError> {
        let terminal_record = self.registry.load(session_id)?.is_some_and(|record| {
            matches!(
                record.state,
                RegistrySessionState::Exited | RegistrySessionState::Stale
            )
        });
        Ok(!terminal_record
            || !matches!(
                state,
                SessionLifecycleState::Starting
                    | SessionLifecycleState::Running
                    | SessionLifecycleState::Stopping
            ))
    }

    fn record_terminal_obligations(&mut self, observations: &[BotsterEngineObservation]) {
        for observation in observations {
            if let BotsterEngineObservation::SessionLifecycle { session_id, state } = observation {
                self.terminal_commit_obligations
                    .insert(session_id.clone(), state.clone());
            }
        }
    }

    fn record_terminal_commit_failure(
        &mut self,
        session_id: &SessionId,
        obligation: Option<SessionLifecycleState>,
    ) {
        if let Some(state) = obligation {
            self.terminal_commit_obligations
                .insert(session_id.clone(), state);
        }
        let failures = self
            .terminal_commit_failures
            .entry(session_id.clone())
            .or_default();
        *failures = failures.saturating_add(1);
        if *failures < TERMINAL_COMMIT_REARM_LIMIT {
            self.engine.wake_source().notify_session(session_id);
        }
    }

    fn cleanup_worker_socket_dir_if_empty(&self) {
        if self.config.worker_path.is_some() {
            let _ = std::fs::remove_dir(worker_socket_dir(&self.config.data_dir));
        }
    }

    fn ensure_running(&self) -> Result<(), CoreDaemonError> {
        if self.running {
            Ok(())
        } else {
            Err(CoreDaemonError::Shutdown)
        }
    }

    fn load_all_records(&self) -> Result<Vec<RegistryRecord>, SessionRegistryError> {
        #[cfg(test)]
        self.registry_load_all_calls
            .set(self.registry_load_all_calls.get().saturating_add(1));
        self.registry.load_all()
    }

    fn ensure_session(&self, session_id: &SessionId) -> Result<(), CoreDaemonError> {
        self.engine
            .session(session_id)
            .map(|_| ())
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))
    }

    fn ensure_control_plane_live(&self, session_id: &SessionId) -> Result<(), CoreDaemonError> {
        if self.engine.control_plane_failed(session_id) {
            Err(CoreDaemonError::ControlPlaneFailed(session_id.clone()))
        } else {
            Ok(())
        }
    }

    fn ensure_session_mutable(&self, session_id: &SessionId) -> Result<(), CoreDaemonError> {
        let session = self
            .engine
            .session(session_id)
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))?;
        if matches!(
            session.lifecycle,
            SessionLifecycleState::Stopping
                | SessionLifecycleState::Exited { .. }
                | SessionLifecycleState::Failed { .. }
        ) || matches!(
            self.registry.load(session_id)?.map(|record| record.state),
            Some(
                RegistrySessionState::Stopping
                    | RegistrySessionState::Exited
                    | RegistrySessionState::Stale
            )
        ) {
            return Err(CoreDaemonError::SessionNotReadable(session_id.clone()));
        }
        Ok(())
    }

    fn engine_session_exited(&self, session_id: &SessionId) -> bool {
        matches!(
            self.engine
                .session(session_id)
                .map(|session| &session.lifecycle),
            Some(SessionLifecycleState::Exited { .. })
        )
    }

    fn engine_session_is_terminal(&self, session_id: &SessionId) -> bool {
        match self
            .engine
            .session(session_id)
            .map(|session| &session.lifecycle)
        {
            None => true,
            Some(SessionLifecycleState::Exited { .. } | SessionLifecycleState::Failed { .. }) => {
                true
            }
            Some(_) => false,
        }
    }

    fn take_pending_drain(&mut self, session_id: &SessionId) -> DrainResult {
        let mut result = DrainResult::default();
        let mut retained = Vec::new();
        for pending in self.pending_drain.drain(..) {
            if &pending.session_id == session_id {
                merge_drain_result(&mut result, pending.result);
            } else {
                retained.push(pending);
            }
        }
        self.pending_drain = retained;
        result
    }

    fn drop_pending_client_session_egress(&mut self, client_id: &ClientId, session_id: &SessionId) {
        for pending in &mut self.pending_drain {
            pending
                .result
                .client_egress
                .retain(|(pending_client, frame)| {
                    pending_client != client_id || egress_session_id(frame) != Some(session_id)
                });
        }
    }

    fn drop_pending_subscription_egress(
        &mut self,
        client_id: &ClientId,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) {
        for pending in &mut self.pending_drain {
            pending
                .result
                .client_egress
                .retain(|(pending_client, frame)| {
                    pending_client != client_id
                        || egress_route(frame) != Some((session_id, subscription_id))
                });
        }
    }

    fn drop_pending_drain(&mut self, session_id: &SessionId) {
        self.pending_drain
            .retain(|pending| &pending.session_id != session_id);
    }

    fn notify_bound_queue_wakes(&mut self) {
        for session_id in self.engine.take_bound_queue_wake_sessions() {
            self.engine.wake_source().notify_session(&session_id);
        }
    }

    fn discard_bound_queue_wakes(&mut self) {
        let _ = self.engine.take_bound_queue_wake_sessions();
    }

    fn retain_pending_drain_result(&mut self, session_id: &SessionId, pending: DrainResult) {
        if !drain_result_is_empty(&pending) {
            self.pending_drain.push(PendingDrainResult {
                session_id: session_id.clone(),
                result: pending,
            });
        }
    }

    fn lifecycle_record(&self, record: &RegistryRecord) -> SessionLifecycleRecord {
        SessionLifecycleRecord {
            session: DaemonSession::from(record),
            metadata: record.metadata.clone(),
            lifecycle: self
                .engine
                .session(&record.session_id)
                .map(|session| session.lifecycle.clone()),
        }
    }

    fn lifecycle_cursor(&self) -> SessionLifecycleCursor {
        SessionLifecycleCursor {
            source_id: self.lifecycle_source_id.clone(),
            sequence: self.lifecycle_sequence,
        }
    }

    fn lifecycle_resync_reason(
        &self,
        after: &SessionLifecycleCursor,
    ) -> Option<SessionLifecycleResyncReason> {
        if after.source_id != self.lifecycle_source_id {
            Some(SessionLifecycleResyncReason::SourceChanged)
        } else if after.sequence > self.lifecycle_sequence {
            Some(SessionLifecycleResyncReason::CursorAhead)
        } else if self
            .lifecycle_journal
            .front()
            .is_some_and(|oldest| after.sequence < oldest.cursor.sequence.saturating_sub(1))
        {
            Some(SessionLifecycleResyncReason::CursorExpired {
                oldest_available_sequence: self
                    .lifecycle_journal
                    .front()
                    .map_or(self.lifecycle_sequence, |change| change.cursor.sequence),
            })
        } else {
            None
        }
    }

    fn observe_lifecycle_walk(
        &mut self,
        now_seconds: u64,
        resume: Option<&ObserveLifecycleCursor>,
        budget: ObserveLifecycleBudget,
    ) -> Result<ObserveLifecycleWalk, SessionLifecyclePageError> {
        let started = Instant::now();
        let mut pass = match resume {
            None => ObservePassState {
                pass_id: new_observe_pass_id(),
                last_visited: None,
                generation: self.observe_live_generation,
                final_session_id: self
                    .observe_live_sessions
                    .last_key_value()
                    .map(|(id, _)| id.clone()),
            },
            Some(cursor) => {
                let matches_open = self.observe_pass.as_ref().is_some_and(|pass| {
                    pass.pass_id == cursor.pass_id
                        && pass.last_visited.as_ref() == cursor.last_visited.as_ref()
                });
                if !matches_open {
                    return Ok(observe_pass_unavailable(cursor.pass_id.clone()));
                }
                self.observe_pass
                    .take()
                    .expect("open observe pass matched resume identity")
            }
        };

        let mut committed_errors = Vec::new();
        let mut typed_errors = Vec::new();
        let mut remaining_visits = budget.max_sessions;
        let mut visited_this_call = false;
        let mut complete = false;

        loop {
            if pass.final_session_id.is_none() {
                complete = true;
                break;
            }
            if remaining_visits == 0 {
                break;
            }
            let next = match self.next_observe_session(&pass, started, budget.max_elapsed) {
                NextObserveSession::Session(next) => next,
                NextObserveSession::Complete => {
                    complete = true;
                    break;
                }
                NextObserveSession::Elapsed => break,
            };
            let candidate_completes = pass.final_session_id.as_deref() == Some(next.0.as_str());
            let candidate = reserved_observe_slice(
                &pass.pass_id,
                &next,
                candidate_completes,
                &committed_errors,
            );
            let candidate_bytes = encoded_observe_slice_len(&candidate);
            if candidate_bytes > budget.max_encoded_result_bytes {
                if !visited_this_call {
                    let minimum_bytes = encoded_observe_slice_len(&reserved_observe_slice(
                        &pass.pass_id,
                        &next,
                        candidate_completes,
                        &[],
                    ));
                    if resume.is_some() {
                        self.observe_pass = Some(pass);
                    }
                    return Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes });
                }
                break;
            }
            if started.elapsed() >= budget.max_elapsed {
                break;
            }
            remaining_visits = remaining_visits.saturating_sub(1);
            visited_this_call = true;
            if let Err(error) = self.observe_session(&next, now_seconds) {
                committed_errors.push(ObserveLifecycleSliceError {
                    session_id: next.clone(),
                    message: sanitize_observe_slice_error_message(&error.to_string()),
                });
                typed_errors.push(ObserveLifecycleSessionError {
                    session_id: next.clone(),
                    error,
                });
            }
            pass.last_visited = Some(next);
            if candidate_completes {
                complete = true;
                break;
            }
        }

        let pass_id = pass.pass_id.clone();
        let last_visited = pass.last_visited.clone();
        let slice = ObserveLifecycleSlice {
            pass_id,
            last_visited,
            complete,
            session_errors: committed_errors,
            resync_required: None,
        };
        let minimum_bytes = encoded_observe_slice_len(&slice);
        self.observe_pass = Some(pass);
        if minimum_bytes > budget.max_encoded_result_bytes {
            return Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes });
        }
        Ok(ObserveLifecycleWalk {
            slice,
            session_errors: typed_errors,
        })
    }

    fn track_live_session(&mut self, session_id: &SessionId) {
        self.terminal_commit_obligations.remove(session_id);
        self.terminal_commit_failures.remove(session_id);
        self.observe_live_generation = self.observe_live_generation.saturating_add(1);
        self.observe_live_sessions
            .insert(session_id.0.clone(), self.observe_live_generation);
    }

    fn next_observe_session(
        &mut self,
        pass: &ObservePassState,
        started: Instant,
        max_elapsed: Duration,
    ) -> NextObserveSession {
        let Some(final_session_id) = pass.final_session_id.as_ref() else {
            return NextObserveSession::Complete;
        };
        let (next, scans) = {
            let mut scans = 0_u64;
            let mut next = NextObserveSession::Complete;
            let start = pass
                .last_visited
                .as_ref()
                .map_or(Unbounded, |last| Excluded(last.0.clone()));
            for (session_id, generation) in self
                .observe_live_sessions
                .range((start, Included(final_session_id.clone())))
            {
                if started.elapsed() >= max_elapsed {
                    next = NextObserveSession::Elapsed;
                    break;
                }
                scans = scans.saturating_add(1);
                if *generation <= pass.generation {
                    next = NextObserveSession::Session(SessionId(session_id.clone()));
                    break;
                }
            }
            (next, scans)
        };
        #[cfg(test)]
        {
            self.observe_index_scans = self.observe_index_scans.saturating_add(scans);
        }
        #[cfg(not(test))]
        let _ = scans;
        next
    }

    fn advance_baseline_index(
        &mut self,
        started: Instant,
        ops: &mut u64,
        items_used: &mut usize,
        budget: &LifecycleBaselineBudget,
    ) -> Result<(), ()> {
        if self
            .baseline_freeze
            .as_ref()
            .is_some_and(|freeze| freeze.index_complete)
        {
            return Ok(());
        }
        if self
            .baseline_freeze
            .as_ref()
            .is_some_and(|freeze| freeze.dir.is_none())
        {
            match std::fs::read_dir(self.registry.root()) {
                Ok(dir) => {
                    if let Some(freeze) = self.baseline_freeze.as_mut() {
                        freeze.dir = Some(dir);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    if let Some(freeze) = self.baseline_freeze.as_mut() {
                        freeze.index_complete = true;
                    }
                    return Ok(());
                }
                Err(_) => return Err(()),
            }
        }

        loop {
            if *items_used >= budget.max_rows {
                break;
            }
            if self.baseline_elapsed(started, *ops) >= budget.max_elapsed {
                break;
            }
            let next = self
                .baseline_freeze
                .as_mut()
                .and_then(|freeze| freeze.dir.as_mut())
                .and_then(Iterator::next);
            match next {
                None => {
                    *items_used = items_used.saturating_add(1);
                    *ops = ops.saturating_add(1);
                    self.record_baseline_index_scan();
                    if let Some(freeze) = self.baseline_freeze.as_mut() {
                        freeze.dir = None;
                        freeze.index_complete = true;
                    }
                    break;
                }
                Some(Err(_)) => return Err(()),
                Some(Ok(entry)) => {
                    *items_used = items_used.saturating_add(1);
                    *ops = ops.saturating_add(1);
                    self.record_baseline_index_scan();
                    let Some(record) = self.registry.load_entry(&entry).map_err(|_| ())? else {
                        continue;
                    };
                    let id = record.session_id.0;
                    if let Some(freeze) = self.baseline_freeze.as_mut() {
                        if freeze.excluded.contains(&id) || freeze.membership.contains_key(&id) {
                            continue;
                        }
                        freeze.membership.insert(id, None);
                    }
                }
            }
        }
        Ok(())
    }

    fn emit_baseline_suffix(
        &mut self,
        after: Option<&SessionId>,
        started: Instant,
        ops: &mut u64,
        items_used: &mut usize,
        budget: LifecycleBaselineBudget,
        empty: SessionLifecycleBaselinePage,
    ) -> Result<SessionLifecycleBaselinePage, SessionLifecyclePageError> {
        let membership_empty = self
            .baseline_freeze
            .as_ref()
            .is_some_and(|freeze| freeze.membership.is_empty());
        if membership_empty {
            self.baseline_freeze = None;
            return Ok(SessionLifecycleBaselinePage {
                complete: true,
                ..empty
            });
        }

        let mut page = empty;
        let mut cursor = after.cloned();
        let inclusive = after.is_some();
        loop {
            let next_id = self.next_baseline_membership_id(
                cursor.as_ref(),
                inclusive && page.sessions.is_empty(),
            );
            let Some(next_id) = next_id else {
                page.complete = true;
                page.next = None;
                break;
            };
            if *items_used >= budget.max_rows
                || self.baseline_elapsed(started, *ops) >= budget.max_elapsed
            {
                page.next = Some(SessionId(next_id));
                page.complete = false;
                break;
            }
            *items_used = items_used.saturating_add(1);
            let record = match self.materialize_baseline_row(&next_id) {
                Ok(Some(record)) => record,
                Ok(None) => {
                    cursor = Some(SessionId(next_id));
                    continue;
                }
                Err(()) => {
                    self.baseline_freeze = None;
                    return Ok(baseline_resync_page(
                        page.snapshot_sequence,
                        SessionLifecycleResyncReason::SourceChanged,
                    ));
                }
            };
            *ops = ops.saturating_add(1);
            if self.baseline_elapsed(started, *ops) >= budget.max_elapsed {
                page.next = Some(SessionId(next_id));
                page.complete = false;
                break;
            }
            page.sessions.push(record);
            let following =
                self.next_baseline_membership_id(Some(&SessionId(next_id.clone())), false);
            page.complete = following.is_none();
            page.next = following.map(SessionId);
            self.record_baseline_page_encode();
            let encoded = encoded_lifecycle_baseline_page_len(&page);
            *ops = ops.saturating_add(1);
            if encoded > budget.max_bytes {
                page.sessions.pop();
                page.next = Some(SessionId(next_id));
                page.complete = false;
                break;
            }
            cursor = Some(SessionId(next_id));
            if self.baseline_elapsed(started, *ops) >= budget.max_elapsed && !page.complete {
                break;
            }
        }

        self.finalize_baseline_page(page, budget.max_bytes)
    }

    fn finalize_baseline_page(
        &mut self,
        page: SessionLifecycleBaselinePage,
        max_bytes: usize,
    ) -> Result<SessionLifecycleBaselinePage, SessionLifecyclePageError> {
        self.record_baseline_page_encode();
        let encoded = encoded_lifecycle_baseline_page_len(&page);
        if encoded > max_bytes {
            return Err(SessionLifecyclePageError::BudgetTooSmall {
                minimum_bytes: encoded,
            });
        }
        if page.complete {
            self.baseline_freeze = None;
        }
        Ok(page)
    }

    fn next_baseline_membership_id(
        &self,
        after: Option<&SessionId>,
        inclusive: bool,
    ) -> Option<String> {
        let freeze = self.baseline_freeze.as_ref()?;
        let start = match (after, inclusive) {
            (None, _) => Unbounded,
            (Some(id), true) => Included(id.0.clone()),
            (Some(id), false) => Excluded(id.0.clone()),
        };
        freeze
            .membership
            .range((start, Unbounded))
            .map(|(id, _)| id.clone())
            .next()
    }

    fn materialize_baseline_row(
        &mut self,
        session_id: &str,
    ) -> Result<Option<SessionLifecycleRecord>, ()> {
        let cached = self
            .baseline_freeze
            .as_ref()
            .and_then(|freeze| freeze.membership.get(session_id))
            .and_then(Clone::clone);
        if let Some(record) = cached {
            self.record_baseline_row_copy();
            return Ok(Some(record));
        }
        let loaded = self
            .registry
            .load_skip_malformed(&SessionId(session_id.to_string()))
            .map_err(|_| ())?;
        let Some(raw) = loaded else {
            if let Some(freeze) = self.baseline_freeze.as_mut() {
                freeze.membership.remove(session_id);
            }
            return Ok(None);
        };
        let mapped = self.lifecycle_record(&raw);
        self.record_baseline_row_copy();
        if let Some(freeze) = self.baseline_freeze.as_mut() {
            freeze
                .membership
                .insert(session_id.to_string(), Some(mapped.clone()));
        }
        Ok(Some(mapped))
    }

    fn fence_baseline_before_save(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), CoreDaemonError> {
        let Some(freeze) = self.baseline_freeze.as_ref() else {
            return Ok(());
        };
        if freeze.excluded.contains(&session_id.0)
            || freeze
                .membership
                .get(&session_id.0)
                .is_some_and(Option::is_some)
        {
            return Ok(());
        }
        match self.registry.load_skip_malformed(session_id)? {
            Some(record) => {
                let mapped = self.lifecycle_record(&record);
                self.record_baseline_row_copy();
                if let Some(freeze) = self.baseline_freeze.as_mut() {
                    freeze.membership.insert(session_id.0.clone(), Some(mapped));
                }
            }
            None => {
                if let Some(freeze) = self.baseline_freeze.as_mut() {
                    freeze.excluded.insert(session_id.0.clone());
                }
            }
        }
        Ok(())
    }

    fn fence_baseline_before_remove(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), CoreDaemonError> {
        let Some(freeze) = self.baseline_freeze.as_ref() else {
            return Ok(());
        };
        if freeze.excluded.contains(&session_id.0)
            || freeze
                .membership
                .get(&session_id.0)
                .is_some_and(Option::is_some)
        {
            return Ok(());
        }
        if let Some(record) = self.registry.load_skip_malformed(session_id)? {
            let mapped = self.lifecycle_record(&record);
            self.record_baseline_row_copy();
            if let Some(freeze) = self.baseline_freeze.as_mut() {
                freeze.membership.insert(session_id.0.clone(), Some(mapped));
            }
        }
        Ok(())
    }

    fn baseline_elapsed(&self, started: Instant, ops: u64) -> Duration {
        let wall = started.elapsed();
        #[cfg(test)]
        {
            if let Some(per_op) = self.config.test_baseline_elapsed_per_op {
                let extra = per_op.saturating_mul(u32::try_from(ops).unwrap_or(u32::MAX));
                return wall.saturating_add(extra);
            }
        }
        #[cfg(not(test))]
        let _ = ops;
        wall
    }

    fn record_baseline_index_scan(&mut self) {
        #[cfg(test)]
        {
            self.baseline_index_scans = self.baseline_index_scans.saturating_add(1);
        }
    }

    fn record_baseline_row_copy(&mut self) {
        #[cfg(test)]
        {
            self.baseline_row_copies = self.baseline_row_copies.saturating_add(1);
        }
    }

    fn record_baseline_page_encode(&mut self) {
        #[cfg(test)]
        {
            self.baseline_page_encodes = self.baseline_page_encodes.saturating_add(1);
        }
    }

    fn observe_session(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        let output = match self.engine.drain_runtime_once(session_id, now_seconds) {
            Ok(output) => output,
            Err(error)
                if is_session_not_found(&error) && self.engine_session_exited(session_id) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let result = drain_result_from_engine_output(output);
        self.notify_bound_queue_wakes();
        if let Some((rows, cols, resize_at)) = self.engine.take_applied_terminal_resize(session_id)
        {
            if let Err(error) = self.persist_changed_session_size(session_id, rows, cols, resize_at)
            {
                self.retain_pending_drain_result(session_id, result);
                return Err(error);
            }
        }
        if let Err(error) =
            self.commit_terminal_lifecycle(session_id, &result.observations, now_seconds)
        {
            self.retain_pending_drain_result(session_id, result);
            return Err(error);
        }
        self.retain_pending_drain_result(session_id, result);
        self.reconcile_pending(now_seconds);
        Ok(())
    }

    fn append_lifecycle_upsert(
        &mut self,
        record: &RegistryRecord,
        lifecycle: Option<SessionLifecycleState>,
    ) {
        self.append_lifecycle_change(SessionLifecycleChangeKind::Upsert {
            record: SessionLifecycleRecord {
                session: DaemonSession::from(record),
                metadata: record.metadata.clone(),
                lifecycle,
            },
        });
    }

    fn append_lifecycle_change(&mut self, kind: SessionLifecycleChangeKind) {
        self.lifecycle_sequence = self.lifecycle_sequence.saturating_add(1);
        self.lifecycle_journal.push_back(SessionLifecycleChange {
            cursor: self.lifecycle_cursor(),
            kind,
        });
        let capacity = self.config.lifecycle_journal_capacity.max(1);
        while self.lifecycle_journal.len() > capacity {
            self.lifecycle_journal.pop_front();
        }
        self.journal_advanced = true;
    }
}

fn encoded_lifecycle_page_len(page: &SessionLifecyclePage) -> usize {
    serde_json::to_vec(page)
        .expect("session lifecycle page must serialize")
        .len()
}

fn encoded_lifecycle_baseline_page_len(page: &SessionLifecycleBaselinePage) -> usize {
    serde_json::to_vec(page)
        .expect("session lifecycle baseline page must serialize")
        .len()
}

fn baseline_resync_page(
    snapshot_sequence: SessionLifecycleCursor,
    resync_required: SessionLifecycleResyncReason,
) -> SessionLifecycleBaselinePage {
    SessionLifecycleBaselinePage {
        snapshot_sequence,
        sessions: Vec::new(),
        next: None,
        complete: false,
        resync_required: Some(resync_required),
    }
}

fn encoded_observe_slice_len(slice: &ObserveLifecycleSlice) -> usize {
    serde_json::to_vec(slice)
        .expect("observe lifecycle slice must serialize")
        .len()
}

fn reserved_observe_slice(
    pass_id: &ObserveLifecyclePassId,
    next: &SessionId,
    complete: bool,
    committed_errors: &[ObserveLifecycleSliceError],
) -> ObserveLifecycleSlice {
    let mut session_errors = committed_errors.to_vec();
    session_errors.push(reserved_observe_slice_error(next.clone()));
    ObserveLifecycleSlice {
        pass_id: pass_id.clone(),
        last_visited: Some(next.clone()),
        complete,
        session_errors,
        resync_required: None,
    }
}

fn observe_pass_unavailable(pass_id: ObserveLifecyclePassId) -> ObserveLifecycleWalk {
    ObserveLifecycleWalk {
        slice: ObserveLifecycleSlice {
            pass_id,
            last_visited: None,
            complete: false,
            session_errors: Vec::new(),
            resync_required: Some(SessionLifecycleResyncReason::ObservePassUnavailable),
        },
        session_errors: Vec::new(),
    }
}

fn new_observe_pass_id() -> ObserveLifecyclePassId {
    // Fixed width so reserved-error admission has a stable encoded size
    // across resume=None retries after BudgetTooSmall.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let ordinal = NEXT_OBSERVE_PASS_ORDINAL.fetch_add(1, Ordering::Relaxed);
    ObserveLifecyclePassId(format!(
        "{:08x}-{:032x}-{:016x}",
        std::process::id(),
        nanos,
        ordinal
    ))
}

fn egress_session_id(frame: &TransportEgress) -> Option<&SessionId> {
    egress_route(frame).map(|(session_id, _)| session_id)
}

fn debug_assert_drain_owner(result: &DrainResult, session_id: &SessionId) {
    debug_assert!(result
        .client_egress
        .iter()
        .all(|(_, frame)| egress_session_id(frame).is_none_or(|owner| owner == session_id)));
}

fn egress_route(frame: &TransportEgress) -> Option<(&SessionId, &SubscriptionId)> {
    match frame {
        TransportEgress::TerminalOutput {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::Snapshot {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::Scrollback {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::ProcessExit {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::AttachState {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::FocusChanged {
            session_id,
            subscription_id,
            ..
        } => Some((session_id, subscription_id)),
        TransportEgress::Binary { .. }
        | TransportEgress::BoundaryPayload { .. }
        | TransportEgress::Pong { .. }
        | TransportEgress::Close { .. } => None,
    }
}

fn new_lifecycle_source_id() -> SessionLifecycleSourceId {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let ordinal = NEXT_LIFECYCLE_SOURCE_ORDINAL.fetch_add(1, Ordering::Relaxed);
    SessionLifecycleSourceId(format!(
        "{:x}-{:x}-{:x}",
        std::process::id(),
        nanos,
        ordinal
    ))
}

fn local_engine(
    max_scrollback_bytes: usize,
    color_profile: Option<TerminalColorProfile>,
) -> DefaultBotsterEngine {
    DefaultBotsterEngine::with_terminal_backend_factory(move |size| {
        default_ghostty_terminal(size, max_scrollback_bytes, color_profile.clone())
    })
}
fn default_ghostty_terminal(
    size: TerminalScreenSize,
    max_scrollback_bytes: usize,
    color_profile: Option<TerminalColorProfile>,
) -> Result<GhosttyTerminal, GhosttyTerminalError> {
    let mut terminal = GhosttyTerminal::with_config(
        size,
        GhosttyAdapterConfig::with_max_scrollback_bytes(max_scrollback_bytes),
    )?;
    // Apply only host-supplied policy. No in-repo presentation defaults.
    if let Some(profile) = color_profile.as_ref() {
        terminal.apply_color_profile(profile)?;
    }
    Ok(terminal)
}

fn drain_result_from_engine_output(output: BotsterEngineOutput) -> DrainResult {
    let mut observations = output.observations;
    observations.extend(
        output
            .session_events
            .iter()
            .filter_map(|event| match event {
                SessionIoEvent::ProcessExited {
                    session_id,
                    payload,
                } => Some(BotsterEngineObservation::SessionLifecycle {
                    session_id: session_id.clone(),
                    state: SessionLifecycleState::Exited {
                        code: payload.exit_code,
                    },
                }),
                _ => None,
            }),
    );
    DrainResult {
        client_egress: output.client_egress,
        backpressure: observations
            .iter()
            .filter_map(|observation| match observation {
                BotsterEngineObservation::Backpressure(summary) => Some(summary.clone()),
                _ => None,
            })
            .collect(),
        observations,
    }
}

fn merge_drain_result(target: &mut DrainResult, source: DrainResult) {
    target.client_egress.extend(source.client_egress);
    target.observations.extend(source.observations);
    target.backpressure.extend(source.backpressure);
}

fn drain_result_is_empty(result: &DrainResult) -> bool {
    result.client_egress.is_empty()
        && result.observations.is_empty()
        && result.backpressure.is_empty()
}

fn take_screen_ready(
    output: &mut BotsterEngineOutput,
    request_id: &RequestId,
) -> Result<ScreenReady, CoreDaemonError> {
    let position = output
        .session_events
        .iter()
        .position(|event| match event {
            SessionIoEvent::ScreenReady(screen) => &screen.request_id == request_id,
            _ => false,
        })
        .ok_or_else(|| CoreDaemonError::MissingScreenResponse(request_id.clone()))?;
    match output.session_events.remove(position) {
        SessionIoEvent::ScreenReady(screen) => Ok(screen),
        _ => unreachable!("position was selected from a ScreenReady event"),
    }
}

fn managed_terminal_backend_error(error: TerminalBackendError) -> CoreDaemonError {
    let error = match error {
        TerminalBackendError::Unsupported { operation } => {
            botster_core::ManagedSessionRuntimeError::UnsupportedSessionRequest {
                request_kind: operation,
            }
        }
        TerminalBackendError::OperationFailed { operation, message } => {
            botster_core::ManagedSessionRuntimeError::TerminalBackendOperation {
                operation,
                message,
            }
        }
        error => botster_core::ManagedSessionRuntimeError::TerminalBackendOperation {
            operation: "terminal_backend",
            message: error.to_string(),
        },
    };
    CoreDaemonError::Engine(error)
}

fn is_session_not_found(error: &DefaultBotsterEngineError) -> bool {
    matches!(
        error,
        DefaultBotsterEngineError::Runtime(error)
            if error.kind == SessionRuntimeErrorKind::SessionNotFound
    )
}
fn unavailable_capture(
    reason: HistoryUnavailableReason,
    color_profile: TerminalColorProfile,
) -> SnapshotCapture {
    SnapshotCapture {
        capture_id: CaptureId(String::new()),
        total_bytes: 0,
        page_bytes: SNAPSHOT_PAGE_BYTES as u32,
        pages: 0,
        rows: 0,
        cols: 0,
        color_profile,
        unavailable: Some(reason),
    }
}

fn unix_now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

impl DaemonEngine {
    fn spawn_pending(&self, session_id: &SessionId) -> bool {
        match self {
            Self::Local(_) => false,
            Self::Worker(engine) => engine.spawn_pending(session_id),
        }
    }

    fn reserve_session_for_request(
        &self,
        session_id: SessionId,
        request_id: u64,
        limit: usize,
    ) -> Result<SessionReservation, SessionReservationRefusal> {
        match self {
            Self::Local(engine) => {
                engine.reserve_session_for_request(session_id, request_id, limit)
            }
            Self::Worker(engine) => {
                engine.reserve_session_for_request(session_id, request_id, limit)
            }
        }
    }

    fn session_reservation_for_request(
        &self,
        session_id: &SessionId,
        request_id: u64,
    ) -> Result<Option<SessionReservation>, SessionReservationRefusal> {
        match self {
            Self::Local(engine) => engine.session_reservation_for_request(session_id, request_id),
            Self::Worker(engine) => engine.session_reservation_for_request(session_id, request_id),
        }
    }

    fn release_session_reservation(
        &self,
        reservation: &SessionReservation,
    ) -> Result<SessionReservationRelease, SessionReservationRefusal> {
        match self {
            Self::Local(engine) => engine.release_session_reservation(reservation),
            Self::Worker(engine) => engine.release_session_reservation(reservation),
        }
    }

    fn pending_session_reservations(&self) -> usize {
        match self {
            Self::Local(engine) => engine.pending_session_reservations(),
            Self::Worker(engine) => engine.pending_session_reservations(),
        }
    }

    fn session(&self, session_id: &SessionId) -> Option<&CoreSession> {
        match self {
            Self::Local(engine) => engine.session(session_id),
            Self::Worker(engine) => engine.session(session_id),
        }
    }

    fn list_sessions(&self) -> Vec<CoreSession> {
        match self {
            Self::Local(engine) => engine.list_sessions(),
            Self::Worker(engine) => engine.list_sessions(),
        }
    }

    fn worker_metadata(&self, session_id: &SessionId) -> Option<&botster_core::SessionMetadata> {
        match self {
            Self::Local(_) => None,
            Self::Worker(engine) => engine.worker_metadata(session_id),
        }
    }

    fn spawn_session(
        &mut self,
        request: botster_core::SessionSpawnRequest,
        metadata: botster_core::CoreSessionMetadata,
    ) -> Result<botster_core::BotsterSpawnOutcome, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.spawn_session(request, metadata),
            Self::Worker(engine) => engine.spawn_session(request, metadata),
        }
    }

    fn expect_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        match self {
            Self::Local(engine) => {
                engine.expect_terminal_adapter(client_id, session_id, subscription_id)
            }
            Self::Worker(engine) => {
                engine.expect_terminal_adapter(client_id, session_id, subscription_id)
            }
        }
    }

    fn cancel_expected_terminal_adapter(
        &mut self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        match self {
            Self::Local(engine) => {
                engine.cancel_expected_terminal_adapter(client_id, session_id, subscription_id)
            }
            Self::Worker(engine) => {
                engine.cancel_expected_terminal_adapter(client_id, session_id, subscription_id)
            }
        }
    }

    fn attach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => {
                engine.attach_client(client_id, session_id, subscription_id, now_seconds)
            }
            Self::Worker(engine) => {
                engine.attach_client(client_id, session_id, subscription_id, now_seconds)
            }
        }
    }

    fn bind_waking_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        adapter: Box<dyn WakingTerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        match self {
            Self::Local(engine) => engine.bind_waking_terminal_adapter(
                client_id,
                session_id,
                subscription_id,
                generation,
                capabilities,
                adapter,
            ),
            Self::Worker(engine) => engine.bind_waking_terminal_adapter(
                client_id,
                session_id,
                subscription_id,
                generation,
                capabilities,
                adapter,
            ),
        }
    }

    fn wait_wakes(&self, timeout: Duration) -> TerminalWakeBatch {
        match self {
            Self::Local(engine) => engine.wait_wakes(timeout),
            Self::Worker(engine) => engine.wait_wakes(timeout),
        }
    }

    fn clamp_paste_wait(&self, timeout: Duration) -> Duration {
        match self {
            Self::Local(engine) => engine.clamp_paste_wait(timeout),
            Self::Worker(engine) => engine.clamp_paste_wait(timeout),
        }
    }

    fn merge_deadline_wakes(&self, batch: TerminalWakeBatch) -> TerminalWakeBatch {
        match self {
            Self::Local(engine) => engine.merge_deadline_wakes(batch),
            Self::Worker(engine) => engine.merge_deadline_wakes(batch),
        }
    }

    fn has_pending_terminal_resizes(&self, session_id: &SessionId) -> bool {
        match self {
            Self::Local(engine) => engine.has_pending_terminal_resizes(session_id),
            Self::Worker(engine) => engine.has_pending_terminal_resizes(session_id),
        }
    }

    fn pending_terminal_resize_len(&self, session_id: &SessionId) -> usize {
        match self {
            Self::Local(engine) => engine.pending_terminal_resize_len(session_id),
            Self::Worker(engine) => engine.pending_terminal_resize_len(session_id),
        }
    }

    fn control_plane_state(
        &self,
        session_id: &SessionId,
    ) -> botster_core::runtime::ControlPlaneState {
        match self {
            Self::Local(_) => botster_core::runtime::ControlPlaneState::Live,
            Self::Worker(engine) => engine.control_plane_state(session_id),
        }
    }

    fn pump_woken(
        &mut self,
        batch: &TerminalWakeBatch,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.pump_woken(batch, now_seconds),
            Self::Worker(engine) => engine.pump_woken(batch, now_seconds),
        }
    }

    fn wake_source(&self) -> &TerminalWakeSource {
        match self {
            Self::Local(engine) => engine.wake_source(),
            Self::Worker(engine) => engine.wake_source(),
        }
    }

    fn list_terminal_subscriptions(
        &self,
        max_logical_bytes: usize,
    ) -> Result<TerminalSubscriptionInventory, TerminalSubscriptionInventoryError> {
        match self {
            Self::Local(engine) => engine.list_terminal_subscriptions(max_logical_bytes),
            Self::Worker(engine) => engine.list_terminal_subscriptions(max_logical_bytes),
        }
    }

    fn terminal_inventory_revision(&self) -> u64 {
        match self {
            Self::Local(engine) => engine.terminal_inventory_revision(),
            Self::Worker(engine) => engine.terminal_inventory_revision(),
        }
    }

    fn take_bound_queue_wake_sessions(&mut self) -> HashSet<SessionId> {
        match self {
            Self::Local(engine) => engine.take_bound_queue_wake_sessions(),
            Self::Worker(engine) => engine.take_bound_queue_wake_sessions(),
        }
    }

    fn session_has_undelivered_frames(&self, session_id: &SessionId) -> bool {
        match self {
            Self::Local(engine) => engine.session_has_undelivered_frames(session_id),
            Self::Worker(engine) => engine.session_has_undelivered_frames(session_id),
        }
    }

    fn bound_owner_has_held_frames(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        match self {
            Self::Local(engine) => engine.bound_owner_has_held_frames(session_id, subscription_id),
            Self::Worker(engine) => engine.bound_owner_has_held_frames(session_id, subscription_id),
        }
    }

    fn terminal_subscription_owner(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<(&ClientId, TerminalSubscriptionGeneration)> {
        match self {
            Self::Local(engine) => engine.terminal_subscription_owner(session_id, subscription_id),
            Self::Worker(engine) => engine.terminal_subscription_owner(session_id, subscription_id),
        }
    }

    fn terminal_subscription_generation(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<TerminalSubscriptionGeneration> {
        match self {
            Self::Local(engine) => {
                engine.terminal_subscription_generation(session_id, subscription_id)
            }
            Self::Worker(engine) => {
                engine.terminal_subscription_generation(session_id, subscription_id)
            }
        }
    }

    fn detach_terminal_subscription(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        now_seconds: u64,
    ) -> Result<
        (
            DetachTerminalSubscriptionResult,
            botster_core::BotsterEngineOutput,
        ),
        DefaultBotsterEngineError,
    > {
        match self {
            Self::Local(engine) => engine.detach_terminal_subscription(
                client_id,
                session_id,
                subscription_id,
                generation,
                now_seconds,
            ),
            Self::Worker(engine) => engine.detach_terminal_subscription(
                client_id,
                session_id,
                subscription_id,
                generation,
                now_seconds,
            ),
        }
    }

    fn detach_client(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => {
                engine.detach_client(client_id, session_id, subscription_id, now_seconds)
            }
            Self::Worker(engine) => {
                engine.detach_client(client_id, session_id, subscription_id, now_seconds)
            }
        }
    }

    fn control_plane_failed(&self, session_id: &SessionId) -> bool {
        match self {
            Self::Local(_) => false,
            Self::Worker(engine) => matches!(
                engine.control_plane_state(session_id),
                botster_core::runtime::ControlPlaneState::Failed(_)
            ),
        }
    }

    fn write_bytes(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: Vec<u8>,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.write_bytes(client_id, session_id, data, now_seconds),
            Self::Worker(engine) => engine.write_bytes(client_id, session_id, data, now_seconds),
        }
    }

    fn resize(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        rows: u16,
        cols: u16,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.resize(client_id, session_id, rows, cols, now_seconds),
            Self::Worker(engine) => engine.resize(client_id, session_id, rows, cols, now_seconds),
        }
    }
    fn capture_active(&self, session_id: &SessionId) -> bool {
        match self {
            Self::Local(_) => false,
            Self::Worker(engine) => engine.capture_active(session_id),
        }
    }

    fn take_applied_terminal_resize(&mut self, session_id: &SessionId) -> Option<(u16, u16, u64)> {
        match self {
            Self::Local(engine) => engine.take_applied_terminal_resize(session_id),
            Self::Worker(engine) => engine.take_applied_terminal_resize(session_id),
        }
    }

    fn drain_runtime_once(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.drain_runtime_once(session_id, last_output_at),
            Self::Worker(engine) => engine.drain_runtime_once(session_id, last_output_at),
        }
    }

    fn shutdown_session(
        &mut self,
        session_id: SessionId,
        reason: impl Into<String>,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        let reason = reason.into();
        match self {
            Self::Local(engine) => engine.shutdown_session(session_id, reason, now_seconds),
            Self::Worker(engine) => engine.shutdown_session(session_id, reason, now_seconds),
        }
    }

    fn forget_terminal_session(&mut self, session_id: &SessionId) -> bool {
        match self {
            Self::Local(engine) => engine.forget_terminal_session(session_id),
            Self::Worker(engine) => engine.forget_terminal_session(session_id),
        }
    }

    fn adopt_worker_process(
        &mut self,
        session_id: SessionId,
        process: botster_core::ProcessIdentity,
        socket_path: PathBuf,
        supports_snapshot_boundary: bool,
        metadata: botster_core::CoreSessionMetadata,
    ) -> Result<CoreSession, DefaultBotsterEngineError> {
        match self {
            Self::Local(_) => Err(DefaultBotsterEngineError::Runtime(
                botster_core::SessionRuntimeError::new(
                    botster_core::SessionRuntimeErrorKind::SpawnFailed,
                    "missing worker path: local daemon engine cannot adopt worker process",
                ),
            )),
            Self::Worker(engine) => engine
                .adopt_worker_process(
                    session_id,
                    process,
                    socket_path,
                    supports_snapshot_boundary,
                    metadata,
                )
                .map(|outcome| outcome.session),
        }
    }

    fn release_workers_for_restart(&mut self) {
        if let Self::Worker(engine) = self {
            engine.release_workers_for_restart();
        }
    }
}

#[cfg(all(test, unix))]
mod terminal_backend_failure_tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use botster_core::{
        CoreSessionMetadata, ManagedSessionRuntimeError, SpawnEnvironment, SpawnWorkingDirectory,
        TerminalAttachState, TerminalOutputChunk, TerminalScreenRuntime, TerminalScreenState,
        TerminalSnapshotPayload, TransportEgress,
    };

    use super::*;

    #[test]
    #[should_panic]
    fn pump_retention_rejects_foreign_route() {
        let owner = SessionId("owner".to_string());
        let foreign = SessionId("foreign".to_string());
        let mut result = DrainResult::default();
        result.client_egress.push((
            ClientId("foreign-client".to_string()),
            TransportEgress::ProcessExit {
                session_id: foreign,
                subscription_id: SubscriptionId("foreign-sub".to_string()),
                code: Some(0),
            },
        ));

        debug_assert_drain_owner(&result, &owner);
    }

    #[test]
    fn terminal_obligation_does_not_regress_an_exited_row() {
        let data_dir = std::env::temp_dir().join(format!(
            "botster-core-daemon-monotonic-obligation-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let session_id = SessionId("monotonic-obligation".to_string());
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let mut record = RegistryRecord::running(
            session_id.clone(),
            None,
            ResizePayload { rows: 24, cols: 80 },
            "test".to_string(),
            10,
        );
        record.mark(RegistrySessionState::Exited, 11);
        daemon.registry.save(&record).expect("save exited row");
        daemon
            .terminal_commit_obligations
            .insert(session_id.clone(), SessionLifecycleState::Stopping);

        daemon
            .commit_terminal_lifecycle(&session_id, &[], 12)
            .expect("drop stale stopping obligation");

        assert_eq!(
            daemon
                .registry
                .load(&session_id)
                .expect("load exited row")
                .expect("exited row")
                .state,
            RegistrySessionState::Exited
        );
        assert!(!daemon.terminal_commit_obligations.contains_key(&session_id));
        assert!(daemon.lifecycle_journal.is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    struct ControlledGhosttyTerminal {
        inner: GhosttyTerminal,
        fail_resize: Rc<Cell<bool>>,
        fail_snapshot: Rc<Cell<bool>>,
        forced_error: Option<String>,
    }

    impl TerminalScreenRuntime for ControlledGhosttyTerminal {
        fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
            self.inner.write_output(bytes)
        }

        fn resize(&mut self, size: TerminalScreenSize) {
            if self.fail_resize.get() {
                self.forced_error = Some("forced Ghostty resize failure".to_string());
            } else {
                self.inner.resize(size);
                self.forced_error = self.inner.last_error().map(|error| error.to_string());
            }
        }

        fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
            if self.fail_snapshot.get() {
                self.forced_error = Some("forced Ghostty snapshot_export failure".to_string());
                TerminalSnapshotPayload::new(
                    Vec::new(),
                    self.inner.size(),
                    Some("ghostty-terminal-snapshot-v1".to_string()),
                )
            } else {
                let snapshot = self.inner.capture_snapshot();
                self.forced_error = self.inner.last_error().map(|error| error.to_string());
                snapshot
            }
        }

        fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
            self.inner.replay_snapshot(payload);
            self.forced_error = self.inner.last_error().map(|error| error.to_string());
        }

        fn screen_state(&self) -> TerminalScreenState {
            self.inner.screen_state()
        }

        fn mode_flags(&self) -> Result<ModeFlags, TerminalBackendError> {
            self.inner.mode_flags()
        }

        fn last_error(&self) -> Option<String> {
            self.forced_error
                .clone()
                .or_else(|| self.inner.last_error().map(|error| error.to_string()))
        }
    }

    #[test]
    fn core_daemon_resize_surfaces_ghostty_error_without_persisting_geometry() {
        let fail_resize = Rc::new(Cell::new(false));
        let fail_snapshot = Rc::new(Cell::new(false));
        let mut daemon = daemon_with_controlled_ghostty(
            "resize-failure",
            Rc::clone(&fail_resize),
            fail_snapshot,
        );
        let session_id = SessionId("daemon-resize-failure-session".to_string());
        let client_id = ClientId("daemon-resize-failure-client".to_string());
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("spawn session");
        daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                SubscriptionId("daemon-resize-failure-subscription".to_string()),
                11,
            )
            .expect("attach session");

        fail_resize.set(true);
        let error = daemon
            .resize(client_id, session_id.clone(), 40, 120, 12)
            .expect_err("resize backend failure should reach CoreDaemon");
        assert!(matches!(
            error,
            CoreDaemonError::Engine(ManagedSessionRuntimeError::TerminalBackendOperation {
                operation: "resize",
                ref message,
            }) if message == "forced Ghostty resize failure"
        ));
        let session = daemon
            .list()
            .expect("load registry")
            .into_iter()
            .find(|session| session.session_id == session_id)
            .expect("registry session");
        assert_eq!(session.size, ResizePayload { rows: 24, cols: 80 });

        fail_resize.set(false);
        daemon
            .resize(
                ClientId("daemon-resize-failure-client".to_string()),
                session_id.clone(),
                30,
                100,
                13,
            )
            .expect("successful Ghostty resize should recover after the failure");
        let session = daemon
            .list()
            .expect("load registry after successful retry")
            .into_iter()
            .find(|session| session.session_id == session_id)
            .expect("registry session after successful retry");
        assert_eq!(
            session.size,
            ResizePayload {
                rows: 30,
                cols: 100
            }
        );
        daemon
            .shutdown(Some(session_id), 15)
            .expect("shutdown after a recovered resize");
        let _ = std::fs::remove_dir_all(&daemon.config.data_dir);
    }

    #[test]
    fn core_daemon_attach_snapshot_failure_is_atomic_and_retryable() {
        let fail_resize = Rc::new(Cell::new(false));
        let fail_snapshot = Rc::new(Cell::new(true));
        let mut daemon = daemon_with_controlled_ghostty(
            "attach-failure",
            fail_resize,
            Rc::clone(&fail_snapshot),
        );
        let session_id = SessionId("daemon-attach-failure-session".to_string());
        let client_id = ClientId("daemon-attach-failure-client".to_string());
        daemon
            .spawn(spawn_request(&session_id), 20)
            .expect("spawn session");

        let error = daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                SubscriptionId("failed-subscription".to_string()),
                21,
            )
            .expect_err("snapshot export failure should fail attach");
        assert!(matches!(
            error,
            CoreDaemonError::Engine(ManagedSessionRuntimeError::TerminalBackendOperation {
                operation: "capture_snapshot",
                ref message,
            }) if message == "forced Ghostty snapshot_export failure"
        ));
        assert!(daemon.pending_drain.is_empty());

        fail_snapshot.set(false);
        let attached = daemon
            .attach(
                client_id,
                session_id.clone(),
                SubscriptionId("fresh-subscription".to_string()),
                22,
            )
            .expect("fresh subscription should attach after recovery");
        let drained = daemon.drain(&session_id, 23).expect("drain attach events");
        assert!(drained.client_egress.iter().all(|(_, frame)| !matches!(
            frame,
            TransportEgress::AttachState { subscription_id, .. }
                if subscription_id.0 == "failed-subscription"
        )));
        assert!(attached.client_egress.iter().any(|(_, frame)| matches!(
            frame,
            TransportEgress::AttachState {
                subscription_id,
                state: TerminalAttachState::Attached,
                ..
            } if subscription_id.0 == "fresh-subscription"
        )));
        assert!(drained.client_egress.iter().all(|(_, frame)| !matches!(
            frame,
            TransportEgress::AttachState { subscription_id, .. }
                if subscription_id.0 == "fresh-subscription"
        )));
        let _ = daemon.shutdown(Some(session_id), 24);
        let _ = std::fs::remove_dir_all(&daemon.config.data_dir);
    }

    #[test]
    fn failed_final_capture_installs_no_retained_terminal_state() {
        let fail_resize = Rc::new(Cell::new(false));
        let fail_snapshot = Rc::new(Cell::new(false));
        let mut daemon = daemon_with_controlled_ghostty(
            "final-capture-failure",
            fail_resize,
            Rc::clone(&fail_snapshot),
        );
        let session_id = SessionId("daemon-final-capture-failure-session".to_string());
        let client_id = ClientId("daemon-final-capture-failure-client".to_string());
        let subscription_id =
            SubscriptionId("daemon-final-capture-failure-subscription".to_string());
        daemon
            .spawn(spawn_request(&session_id), 30)
            .expect("spawn session");
        daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                31,
            )
            .expect("attach session");
        let _ = daemon
            .drain(&session_id, 32)
            .expect("drain initial attach egress");

        fail_snapshot.set(true);
        let error = daemon
            .shutdown(Some(session_id.clone()), 33)
            .expect_err("failed paired final capture should fail shutdown finalization");
        assert!(matches!(
            error,
            CoreDaemonError::Engine(ManagedSessionRuntimeError::TerminalBackendOperation {
                operation: "capture_snapshot",
                ref message,
            }) if message == "forced Ghostty snapshot_export failure"
        ));
        assert!(!daemon.retained_terminal.contains_key(&session_id));
        let pending_egress = daemon
            .pending_drain
            .iter()
            .filter(|pending| pending.session_id == session_id)
            .flat_map(|pending| &pending.result.client_egress)
            .collect::<Vec<_>>();
        assert!(
            pending_egress.iter().any(|(frame_client_id, frame)| {
                frame_client_id == &client_id
                    && matches!(
                        frame,
                        TransportEgress::ProcessExit {
                            session_id: frame_session_id,
                            subscription_id: frame_subscription_id,
                            ..
                        } if frame_session_id == &session_id
                            && frame_subscription_id == &subscription_id
                    )
            }),
            "capture failure should preserve shutdown recovery egress: {:?}",
            pending_egress
        );
        assert!(!daemon.retained_unavailable.contains_key(&session_id));

        let _ = std::fs::remove_dir_all(&daemon.config.data_dir);
    }

    fn daemon_with_controlled_ghostty(
        label: &str,
        fail_resize: Rc<Cell<bool>>,
        fail_snapshot: Rc<Cell<bool>>,
    ) -> CoreDaemon {
        let data_dir = std::env::temp_dir().join(format!(
            "botster-core-daemon-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let config = CoreDaemonConfig::new(&data_dir);
        let factory_fail_resize = Rc::clone(&fail_resize);
        let factory_fail_snapshot = Rc::clone(&fail_snapshot);
        let engine = DefaultBotsterEngine::with_terminal_backend_factory(move |size| {
            Ok::<_, GhosttyTerminalError>(ControlledGhosttyTerminal {
                inner: default_ghostty_terminal(size, DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES, None)?,
                fail_resize: Rc::clone(&factory_fail_resize),
                fail_snapshot: Rc::clone(&factory_fail_snapshot),
                forced_error: None,
            })
        });
        let mut daemon = CoreDaemon::new(config);
        daemon.engine = DaemonEngine::Local(Box::new(engine));
        daemon
    }

    fn spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
        SpawnSessionRequest {
            request: botster_core::SessionSpawnRequest {
                request_id: RequestId(format!("spawn-{}", session_id.0)),
                session_id: session_id.clone(),
                executable: "sh".to_string(),
                arguments: vec!["-c".to_string(), "while :; do sleep 1; done".to_string()],
                working_directory: SpawnWorkingDirectory {
                    path: ".".to_string(),
                },
                environment: SpawnEnvironment::default(),
                initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
            },
            metadata: CoreSessionMetadata::new(),
        }
    }
}

fn live_session_count(engine: &DaemonEngine, session_id: &SessionId) -> usize {
    engine
        .list_sessions()
        .into_iter()
        .filter(|session| &session.session_id == session_id)
        .count()
}

fn adoption_candidate_count(engine: &DaemonEngine, record: &RegistryRecord) -> usize {
    let live_candidates = live_session_count(engine, &record.session_id);
    let registry_candidate = usize::from(
        record.handshake_verified
            && record.recovery_identity.is_some()
            && record.protocol_version == botster_core::PROTOCOL_VERSION,
    );
    live_candidates.max(registry_candidate) + record.duplicate_worker_candidates
}

fn has_worker_control_socket(record: &RegistryRecord) -> bool {
    worker_control_socket(record).is_some()
}

fn worker_control_socket(record: &RegistryRecord) -> Option<PathBuf> {
    record
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_control_socket"))
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
}

fn worker_socket_dir(data_dir: &PathBuf) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    data_dir.hash(&mut hasher);
    std::env::temp_dir().join(format!("bcd-{:x}", hasher.finish()))
}

fn stale_worker_reason(record: &RegistryRecord) -> SessionWorkerStaleReason {
    if record.process.is_some() {
        SessionWorkerStaleReason::WorkerDied
    } else {
        SessionWorkerStaleReason::ProcessMissing
    }
}

#[cfg(test)]
mod pending_operation_tests {
    use super::*;

    /// Every daemon adapter close names its reason.
    #[test]
    fn every_daemon_adapter_close_passes_bind_rejected() {
        use botster_core_test_support::close_sites::{route_close_sites, CloseSite};
        assert_eq!(
            route_close_sites(include_str!("daemon.rs")),
            vec![CloseSite::new("bind_waking_terminal_adapter", "BindRejected"); 3]
        );
    }

    fn daemon(label: &str) -> CoreDaemon {
        let data_dir = std::env::temp_dir().join(format!(
            "botster-pending-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        CoreDaemon::new(CoreDaemonConfig::new(data_dir))
    }

    fn reply(id: &str, text: &str) -> botster_core::ScreenPayload {
        botster_core::ScreenPayload {
            request_id: id.to_owned(),
            text: text.to_owned(),
            error_kind: None,
        }
    }

    #[test]
    fn two_readbacks_in_one_pump_each_keep_their_own_reply() {
        let mut daemon = daemon("readbacks");
        let session = SessionId("s".into());
        let a = daemon.test_insert_pending_read_screen(session.clone(), "probe-a");
        let b = daemon.test_insert_pending_read_screen(session.clone(), "probe-b");
        let mut screen: HashMap<_, _> = [
            ("probe-a".to_owned(), reply("probe-a", "A")),
            ("probe-b".to_owned(), reply("probe-b", "B")),
        ]
        .into_iter()
        .collect();
        let mut modes = HashMap::new();

        daemon.resolve_pending_readbacks(&mut screen, &mut modes);

        let mut texts: Vec<(PendingOperationId, String)> = daemon
            .take_completions()
            .into_iter()
            .map(|completion| match completion {
                CoreCompletion::ReadScreen { id, result } => {
                    (id, result.expect("reply").text.to_string())
                }
                other => panic!("unexpected completion {:?}", other.id()),
            })
            .collect();
        texts.sort();
        assert_eq!(texts, vec![(a, "A".to_owned()), (b, "B".to_owned())]);
        assert!(daemon.pending.is_empty());
        assert!(screen.is_empty(), "every reply was delivered to its owner");
    }

    #[test]
    fn a_reply_for_the_other_readback_is_not_consumed_by_the_first() {
        let mut daemon = daemon("readback-order");
        let session = SessionId("s".into());
        let _a = daemon.test_insert_pending_read_screen(session.clone(), "probe-a");
        let b = daemon.test_insert_pending_read_screen(session, "probe-b");
        let mut screen: HashMap<_, _> = [("probe-b".to_owned(), reply("probe-b", "B"))]
            .into_iter()
            .collect();

        daemon.resolve_pending_readbacks(&mut screen, &mut HashMap::new());

        let completed: Vec<_> = daemon
            .take_completions()
            .into_iter()
            .map(|completion| completion.id())
            .collect();
        assert_eq!(completed, vec![b]);
        assert_eq!(daemon.pending.len(), 1, "A still waits for its own reply");
    }

    #[test]
    fn owner_release_cancels_pending_and_open_captures() {
        let mut daemon = daemon("owner-release");
        let owner = CaptureOwner("client-a".into());
        let other = CaptureOwner("client-b".into());
        let session = SessionId("s".into());
        let pending = daemon.test_insert_pending_capture(session.clone(), owner.clone(), 1);
        let kept = daemon.test_insert_pending_capture(session, other.clone(), 2);
        let open = daemon.open_capture(
            owner.clone(),
            Arc::from(vec![1u8, 2, 3].into_boxed_slice()),
            24,
            80,
            TerminalColorProfile::default(),
        );

        let released = daemon.release_owner_captures(&owner);

        assert_eq!(released, 2);
        assert!(!daemon.open_captures.contains_key(&open.capture_id));
        assert!(!daemon.pending.contains_key(&pending));
        assert!(daemon.pending.contains_key(&kept));
        let cancelled: Vec<_> = daemon
            .take_completions()
            .into_iter()
            .filter(|completion| {
                matches!(
                    completion,
                    CoreCompletion::CaptureSnapshot {
                        result: Err(CoreDaemonError::Cancelled),
                        ..
                    }
                )
            })
            .map(|completion| completion.id())
            .collect();
        assert_eq!(cancelled, vec![pending]);
        assert_eq!(daemon.open_captures_for(&owner), 0);
    }

    #[test]
    fn host_waits_end_at_the_earliest_pending_operation_deadline() {
        let mut daemon = daemon("pending-deadline-clamp");
        let long = Duration::from_secs(30);
        assert_eq!(
            daemon.clamp_wait(long),
            long,
            "no pending deadline, no clamp"
        );
        let id = daemon
            .allocate_pending_id()
            .expect("test operation identity");
        let bound = Duration::from_millis(50);
        daemon.pending.insert(
            id,
            PendingState {
                kind: PendingKind::ReadScreen {
                    session_id: SessionId("s".into()),
                    probe_id: "probe".into(),
                },
                deadline: Some(Instant::now() + bound),
                cancelled: false,
            },
        );

        assert!(daemon.clamp_wait(long) <= bound);
        assert!(daemon.clamp_pending_operation_wait(long) <= bound);
    }

    #[test]
    fn a_cancelled_spawn_keeps_its_slot_until_the_launch_is_collected() {
        let mut daemon = daemon("cancelled-spawn");
        let id = daemon
            .allocate_pending_id()
            .expect("test operation identity");
        daemon.pending.insert(
            id,
            PendingState {
                kind: PendingKind::Spawn {
                    reservation: None,
                    session_id: SessionId("s".into()),
                    metadata: botster_core::CoreSessionMetadata::new(),
                    size: ResizePayload { rows: 24, cols: 80 },
                    label: "sh".into(),
                    now_seconds: 1,
                },
                deadline: None,
                cancelled: false,
            },
        );

        assert!(daemon.cancel(id));

        assert_eq!(daemon.pending_spawns(), 1, "slot stays reserved");
        assert!(daemon.pending[&id].cancelled);
        assert!(!daemon.cancel(id), "a second cancel is a no-op");
        let completions = daemon
            .take_completions()
            .into_iter()
            .filter(|completion| {
                matches!(
                    completion,
                    CoreCompletion::Spawn {
                        result: Err(CoreDaemonError::Cancelled),
                        ..
                    }
                )
            })
            .count();
        assert_eq!(completions, 1, "exactly one Cancelled completion");
    }
}

#[cfg(test)]
mod retention_admission_tests {
    use super::*;

    fn retained(bytes: usize, exited_at: u64) -> RetainedTerminal {
        RetainedTerminal {
            screen_text: Arc::from(""),
            snapshot: Some(Arc::from(vec![0u8; bytes].into_boxed_slice())),
            mode_bits: 0,
            rows: 24,
            cols: 80,
            color_profile: TerminalColorProfile::default(),
            exited_at,
            bytes,
        }
    }

    #[test]
    fn an_object_that_can_never_fit_does_not_evict_valid_history() {
        let data_dir = std::env::temp_dir().join(format!(
            "botster-retention-admission-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let config = CoreDaemonConfig::new(&data_dir).with_retention_policy(RetentionPolicy {
            max_object_bytes: 16 * 1024 * 1024,
            max_total_bytes: 8 * 1024 * 1024,
            max_sessions: 8,
        });
        let mut daemon = CoreDaemon::new(config);
        let small = SessionId("small".to_string());
        let large = SessionId("large".to_string());
        daemon.admit_retained(&small, retained(1024, 1));

        daemon.admit_retained(&large, retained(10 * 1024 * 1024, 2));

        assert!(daemon.retained_terminal.contains_key(&small));
        assert_eq!(
            daemon.retained_unavailable.get(&large),
            Some(&HistoryUnavailableReason::Oversize)
        );
        assert_eq!(daemon.retention_accounting.evictions, 0);
        assert_eq!(daemon.retention_accounting.oversize_refusals, 1);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[cfg(all(test, unix))]
mod observe_pass_snapshot_tests {
    use super::*;
    use botster_core::{CoreSessionMetadata, SpawnEnvironment, SpawnWorkingDirectory};

    #[test]
    fn first_pass_can_yield_before_scanning_a_large_live_index() {
        let data_dir = std::env::temp_dir().join(format!(
            "botster-observe-large-index-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        for index in 0..100_000_u64 {
            daemon.observe_live_generation = index + 1;
            daemon
                .observe_live_sessions
                .insert(format!("session-{index:06}"), index + 1);
        }

        let first_slice = daemon
            .observe_lifecycle_slice(
                12,
                None,
                ObserveLifecycleBudget {
                    max_sessions: usize::MAX,
                    max_encoded_result_bytes: usize::MAX,
                    max_elapsed: Duration::ZERO,
                },
            )
            .expect("first elapsed yield");
        assert!(!first_slice.complete);
        assert!(first_slice.last_visited.is_none());
        assert!(first_slice.resync_required.is_none());
        assert_eq!(daemon.observe_index_scans, 0);
        let resume = ObserveLifecycleCursor {
            pass_id: first_slice.pass_id,
            last_visited: None,
        };
        let resumed = daemon
            .observe_lifecycle_slice(
                13,
                Some(&resume),
                ObserveLifecycleBudget {
                    max_sessions: usize::MAX,
                    max_encoded_result_bytes: usize::MAX,
                    max_elapsed: Duration::ZERO,
                },
            )
            .expect("resumed elapsed yield");
        assert!(!resumed.complete);
        assert!(resumed.last_visited.is_none());
        assert_eq!(daemon.observe_index_scans, 0);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn resume_scans_only_the_unvisited_live_suffix() {
        let data_dir = std::env::temp_dir().join(format!(
            "botster-observe-list-count-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let first = SessionId("a-observe-list".to_string());
        let second = SessionId("b-observe-list".to_string());
        daemon
            .spawn(snapshot_spawn_request(&first), 10)
            .expect("first spawn");
        daemon
            .spawn(snapshot_spawn_request(&second), 11)
            .expect("second spawn");
        let budget = ObserveLifecycleBudget {
            max_sessions: 1,
            max_encoded_result_bytes: 16 * 1024,
            max_elapsed: Duration::MAX,
        };
        let yielded = daemon
            .observe_lifecycle_slice(
                12,
                None,
                ObserveLifecycleBudget {
                    max_sessions: 1,
                    max_encoded_result_bytes: 16 * 1024,
                    max_elapsed: Duration::ZERO,
                },
            )
            .expect("yield before first visit");
        assert_eq!(daemon.observe_index_scans, 0);
        let resume = ObserveLifecycleCursor {
            pass_id: yielded.pass_id,
            last_visited: None,
        };
        let first_slice = daemon
            .observe_lifecycle_slice(13, Some(&resume), budget)
            .expect("resume");
        assert_eq!(daemon.observe_index_scans, 1);
        assert_eq!(first_slice.last_visited.as_ref(), Some(&first));
        let resume = ObserveLifecycleCursor {
            pass_id: first_slice.pass_id.clone(),
            last_visited: first_slice.last_visited.clone(),
        };
        let second_slice = daemon
            .observe_lifecycle_slice(14, Some(&resume), budget)
            .expect("second resume");
        assert_eq!(daemon.observe_index_scans, 2);
        assert_eq!(second_slice.last_visited.as_ref(), Some(&second));
        assert!(second_slice.complete);
        daemon.shutdown(Some(first), 20).ok();
        daemon.shutdown(Some(second), 21).ok();
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn exact_session_lookup_does_not_scan_a_large_registry() {
        let data_dir = std::env::temp_dir().join(format!(
            "botster-exact-observe-scans-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let live = SessionId("z-exact-observe-live".to_string());
        daemon
            .spawn(snapshot_spawn_request(&live), 10)
            .expect("one live session so an observe walk would increment scans");
        for index in 0..257_u32 {
            let dummy = SessionId(format!("a-dummy-{index:03}"));
            daemon
                .registry
                .save(&RegistryRecord::running(
                    dummy,
                    None,
                    ResizePayload { rows: 24, cols: 80 },
                    "dummy".to_string(),
                    10,
                ))
                .expect("dummy registry row");
        }
        let target = SessionId("a-dummy-256".to_string());
        let looked_up = daemon
            .observe_session_lifecycle(&target, 20)
            .expect("exact query");
        match looked_up {
            SessionLifecycleLookup::Found(record) => {
                assert_eq!(record.session.session_id, target);
            }
            other => panic!("expected Found for the 257th dummy row, got {other:?}"),
        }
        assert_eq!(
            daemon.registry.test_load_all_calls(),
            0,
            "exact query must not call SessionRegistry::load_all"
        );
        assert_eq!(daemon.registry_load_all_calls.get(), 0);
        assert_eq!(daemon.observe_index_scans, 0);
        assert_eq!(daemon.baseline_index_scans, 0);
        daemon
            .registry
            .load_all()
            .expect("ablation: a direct load_all must increment the registry counter");
        assert_eq!(
            daemon.registry.test_load_all_calls(),
            1,
            "SessionRegistry::load_all must count a direct collection scan"
        );
        daemon.shutdown(Some(live), 40).ok();
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn exact_registry_state_lookup_does_not_scan_a_large_registry() {
        let data_dir = std::env::temp_dir().join(format!(
            "botster-exact-registry-state-scans-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let live = SessionId("z-exact-registry-state-live".to_string());
        daemon
            .spawn(snapshot_spawn_request(&live), 10)
            .expect("one live session so an observe walk would increment scans");
        for index in 0..257_u32 {
            let dummy = SessionId(format!("a-dummy-{index:03}"));
            daemon
                .registry
                .save(&RegistryRecord::running(
                    dummy,
                    None,
                    ResizePayload { rows: 24, cols: 80 },
                    "dummy".to_string(),
                    10,
                ))
                .expect("dummy registry row");
        }
        let target = SessionId("a-dummy-256".to_string());
        let looked_up = daemon
            .session_registry_state(&target)
            .expect("exact registry-state query");
        match looked_up {
            SessionRegistryStateLookup::Found(state) => {
                assert_eq!(state, RegistrySessionState::Running);
            }
            other => panic!("expected Found(Running) for the 257th dummy row, got {other:?}"),
        }
        assert_eq!(
            daemon.registry.test_load_all_calls(),
            0,
            "exact query must not call SessionRegistry::load_all"
        );
        assert_eq!(daemon.registry_load_all_calls.get(), 0);
        assert_eq!(daemon.observe_index_scans, 0);
        assert_eq!(daemon.baseline_index_scans, 0);
        daemon
            .registry
            .load_all()
            .expect("ablation: a direct load_all must increment the registry counter");
        assert_eq!(
            daemon.registry.test_load_all_calls(),
            1,
            "SessionRegistry::load_all must count a direct collection scan"
        );
        daemon.shutdown(Some(live), 40).ok();
        let _ = std::fs::remove_dir_all(data_dir);
    }

    fn snapshot_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
        SpawnSessionRequest {
            request: botster_core::SessionSpawnRequest {
                request_id: RequestId(format!("spawn-{}", session_id.0)),
                session_id: session_id.clone(),
                executable: "sh".to_string(),
                arguments: vec!["-c".to_string(), "while :; do sleep 1; done".to_string()],
                working_directory: SpawnWorkingDirectory {
                    path: ".".to_string(),
                },
                environment: SpawnEnvironment::default(),
                initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
            },
            metadata: CoreSessionMetadata::new(),
        }
    }
}

#[cfg(test)]
mod baseline_freeze_bound_tests {
    use super::*;
    use botster_core::SessionId;

    /// One counted op expires a matching `max_elapsed` even if wall time is
    /// zero. The value is larger than workspace-load scheduling jitter so
    /// the first check at `ops = 0` still has slack.
    const TEST_ELAPSED_STEP: Duration = Duration::from_secs(60);

    fn seed_records(daemon: &CoreDaemon, count: usize) {
        for index in 0..count {
            let record = RegistryRecord::running(
                SessionId(format!("sess-{index:04}")),
                None,
                ResizePayload { rows: 24, cols: 80 },
                "seed".to_string(),
                1,
            );
            daemon.registry.save(&record).expect("seed");
        }
    }

    fn data_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "botster-baseline-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn finish_index(daemon: &mut CoreDaemon, _count: usize) -> SessionLifecycleCursor {
        let mut snapshot = None;
        loop {
            let page = daemon
                .lifecycle_baseline_page(
                    snapshot.as_ref(),
                    None,
                    LifecycleBaselineBudget {
                        max_rows: 1,
                        max_bytes: 64 * 1024,
                        max_elapsed: Duration::MAX,
                    },
                )
                .expect("finish index");
            assert!(!page.complete);
            assert!(page.sessions.is_empty());
            snapshot = Some(page.snapshot_sequence.clone());
            if daemon
                .baseline_freeze
                .as_ref()
                .is_some_and(|freeze| freeze.index_complete)
            {
                return page.snapshot_sequence;
            }
        }
    }

    fn materialized_rows(daemon: &CoreDaemon) -> usize {
        daemon
            .baseline_freeze
            .as_ref()
            .map(|freeze| {
                freeze
                    .membership
                    .values()
                    .filter(|row| row.is_some())
                    .count()
            })
            .unwrap_or(0)
    }

    fn empty_page_minimum(daemon: &mut CoreDaemon, snapshot: &SessionLifecycleCursor) -> usize {
        match daemon.lifecycle_baseline_page(
            Some(snapshot),
            None,
            LifecycleBaselineBudget {
                max_rows: usize::MAX,
                max_bytes: 0,
                max_elapsed: Duration::MAX,
            },
        ) {
            Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
            other => panic!("expected BudgetTooSmall, got {other:?}"),
        }
    }

    fn assert_page_within_budget(page: &SessionLifecycleBaselinePage, max_bytes: usize) {
        let encoded = encoded_lifecycle_baseline_page_len(page);
        assert!(
            encoded <= max_bytes,
            "returned page encoded {encoded} bytes, budget {max_bytes}"
        );
    }

    fn continuation_too_small(
        daemon: &mut CoreDaemon,
        snapshot: &SessionLifecycleCursor,
        after: Option<&SessionId>,
        max_bytes: usize,
    ) -> usize {
        match daemon.lifecycle_baseline_page(
            Some(snapshot),
            after,
            LifecycleBaselineBudget {
                max_rows: usize::MAX,
                max_bytes,
                max_elapsed: Duration::MAX,
            },
        ) {
            Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => {
                assert!(minimum_bytes > max_bytes);
                minimum_bytes
            }
            other => panic!("expected BudgetTooSmall, got {other:?}"),
        }
    }

    #[test]
    fn setup_only_elapsed_does_not_scan_or_copy() {
        let data_dir = data_dir("setup-only");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        seed_records(&daemon, 8);
        let page = daemon
            .lifecycle_baseline_page(
                None,
                None,
                LifecycleBaselineBudget {
                    max_rows: usize::MAX,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::ZERO,
                },
            )
            .expect("setup-only");
        assert!(!page.complete);
        assert!(page.sessions.is_empty());
        assert!(page.next.is_none());
        assert_eq!(daemon.baseline_index_scans, 0);
        assert_eq!(daemon.baseline_row_copies, 0);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn first_call_item_limit_examines_one_directory_entry() {
        let data_dir = data_dir("index-item");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        seed_records(&daemon, 8);
        let page = daemon
            .lifecycle_baseline_page(
                None,
                None,
                LifecycleBaselineBudget {
                    max_rows: 1,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("index item");
        assert!(!page.complete);
        assert!(page.sessions.is_empty());
        assert_page_within_budget(&page, 64 * 1024);
        assert_eq!(daemon.baseline_index_scans, 1);
        assert_eq!(daemon.baseline_row_copies, 0);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn first_suffix_item_zero_uses_continuation_minimum() {
        let data_dir = data_dir("first-suffix-item-zero");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        seed_records(&daemon, 8);
        let snapshot = finish_index(&mut daemon, 8);
        let empty_minimum = empty_page_minimum(&mut daemon, &snapshot);
        let continuation_minimum = match daemon.lifecycle_baseline_page(
            Some(&snapshot),
            None,
            LifecycleBaselineBudget {
                max_rows: 0,
                max_bytes: empty_minimum,
                max_elapsed: Duration::MAX,
            },
        ) {
            Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
            other => panic!("expected BudgetTooSmall, got {other:?}"),
        };
        let page = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                None,
                LifecycleBaselineBudget {
                    max_rows: 0,
                    max_bytes: continuation_minimum,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("smallest item-yield continuation");
        assert!(!page.complete);
        assert!(page.sessions.is_empty());
        assert!(page.next.is_some());
        assert_page_within_budget(&page, continuation_minimum);
        assert_eq!(
            encoded_lifecycle_baseline_page_len(&page),
            continuation_minimum
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mid_work_elapsed_hook_stops_after_partial_index_progress() {
        let data_dir = data_dir("elapsed-hook");
        let mut daemon = CoreDaemon::new(
            CoreDaemonConfig::new(&data_dir).with_test_baseline_elapsed_per_op(TEST_ELAPSED_STEP),
        );
        seed_records(&daemon, 8);
        let minted = daemon
            .lifecycle_baseline_page(
                None,
                None,
                LifecycleBaselineBudget {
                    max_rows: usize::MAX,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::ZERO,
                },
            )
            .expect("setup mint");
        assert_eq!(daemon.baseline_index_scans, 0);
        let page = daemon
            .lifecycle_baseline_page(
                Some(&minted.snapshot_sequence),
                None,
                LifecycleBaselineBudget {
                    max_rows: usize::MAX,
                    max_bytes: 64 * 1024,
                    max_elapsed: TEST_ELAPSED_STEP,
                },
            )
            .expect("one counted op");
        assert!(!page.complete);
        assert!(page.sessions.is_empty());
        assert_eq!(daemon.baseline_index_scans, 1);
        assert_eq!(daemon.baseline_row_copies, 0);
        let later = daemon
            .lifecycle_baseline_page(
                Some(&minted.snapshot_sequence),
                None,
                LifecycleBaselineBudget {
                    max_rows: usize::MAX,
                    max_bytes: 64 * 1024,
                    max_elapsed: TEST_ELAPSED_STEP,
                },
            )
            .expect("later suffix");
        assert!(!later.complete);
        assert_eq!(daemon.baseline_index_scans, 2);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn later_page_item_limit_copies_one_suffix_row() {
        let data_dir = data_dir("later-item");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        seed_records(&daemon, 4);
        let snapshot = finish_index(&mut daemon, 4);
        let scans_after_index = daemon.baseline_index_scans;
        let encodes_after_index = daemon.baseline_page_encodes;
        let first = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                None,
                LifecycleBaselineBudget {
                    max_rows: 1,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("first suffix row");
        assert_eq!(first.sessions.len(), 1);
        assert!(!first.complete);
        assert_page_within_budget(&first, 64 * 1024);
        assert_eq!(daemon.baseline_row_copies, 1);
        assert_eq!(daemon.baseline_page_encodes, encodes_after_index + 2);
        assert_eq!(materialized_rows(&daemon), 1);
        let scans_after_first = daemon.baseline_index_scans;
        assert_eq!(scans_after_first, scans_after_index);
        let second = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                first.next.as_ref(),
                LifecycleBaselineBudget {
                    max_rows: 1,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("second suffix row");
        assert_eq!(second.sessions.len(), 1);
        assert_ne!(
            second.sessions[0].session.session_id,
            first.sessions[0].session.session_id
        );
        assert_page_within_budget(&second, 64 * 1024);
        assert_eq!(daemon.baseline_index_scans, scans_after_first);
        assert_eq!(daemon.baseline_row_copies, 2);
        assert_eq!(daemon.baseline_page_encodes, encodes_after_index + 4);
        assert_eq!(materialized_rows(&daemon), 2);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn first_suffix_elapsed_stops_before_encode() {
        let data_dir = data_dir("first-suffix-elapsed");
        let mut daemon = CoreDaemon::new(
            CoreDaemonConfig::new(&data_dir).with_test_baseline_elapsed_per_op(TEST_ELAPSED_STEP),
        );
        seed_records(&daemon, 8);
        let snapshot = finish_index(&mut daemon, 8);
        assert!(daemon.baseline_index_scans >= 8);
        let scans_after_index = daemon.baseline_index_scans;
        let encodes_after_index = daemon.baseline_page_encodes;
        assert_eq!(daemon.baseline_row_copies, 0);
        let page = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                None,
                LifecycleBaselineBudget {
                    max_rows: usize::MAX,
                    max_bytes: 64 * 1024,
                    max_elapsed: TEST_ELAPSED_STEP,
                },
            )
            .expect("elapsed after materialize");
        assert!(!page.complete);
        assert!(page.sessions.is_empty());
        assert!(page.next.is_some());
        assert_page_within_budget(&page, 64 * 1024);
        assert_eq!(daemon.baseline_index_scans, scans_after_index);
        assert_eq!(daemon.baseline_row_copies, 1);
        assert_eq!(
            daemon.baseline_page_encodes,
            encodes_after_index + 1,
            "elapsed must not encode the rejected row; only the continuation page"
        );
        assert_eq!(materialized_rows(&daemon), 1);
        let continued = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                page.next.as_ref(),
                LifecycleBaselineBudget {
                    max_rows: 1,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("continue after elapsed yield");
        assert_eq!(continued.sessions.len(), 1);
        assert_eq!(
            continued.sessions[0].session.session_id,
            page.next
                .clone()
                .expect("elapsed yield names the unencoded row")
        );
        assert_ne!(continued.next, page.next);
        assert_page_within_budget(&continued, 64 * 1024);
        assert_eq!(daemon.baseline_row_copies, 2);
        assert_eq!(daemon.baseline_page_encodes, encodes_after_index + 3);
        assert_eq!(materialized_rows(&daemon), 1);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn later_suffix_elapsed_stops_before_encode() {
        let data_dir = data_dir("later-suffix-elapsed");
        let mut daemon = CoreDaemon::new(
            CoreDaemonConfig::new(&data_dir).with_test_baseline_elapsed_per_op(TEST_ELAPSED_STEP),
        );
        seed_records(&daemon, 8);
        let snapshot = finish_index(&mut daemon, 8);
        let encodes_after_index = daemon.baseline_page_encodes;
        let first = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                None,
                LifecycleBaselineBudget {
                    max_rows: 1,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("first suffix row");
        assert_eq!(first.sessions.len(), 1);
        assert_page_within_budget(&first, 64 * 1024);
        assert_eq!(daemon.baseline_row_copies, 1);
        assert_eq!(daemon.baseline_page_encodes, encodes_after_index + 2);
        let page = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                first.next.as_ref(),
                LifecycleBaselineBudget {
                    max_rows: usize::MAX,
                    max_bytes: 64 * 1024,
                    max_elapsed: TEST_ELAPSED_STEP,
                },
            )
            .expect("later elapsed after materialize");
        assert!(!page.complete);
        assert!(page.sessions.is_empty());
        assert_eq!(page.next, first.next);
        assert_page_within_budget(&page, 64 * 1024);
        assert_eq!(daemon.baseline_row_copies, 2);
        assert_eq!(daemon.baseline_page_encodes, encodes_after_index + 3);
        assert_eq!(materialized_rows(&daemon), 2);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn first_suffix_byte_limit_stops_before_remaining_rows() {
        let data_dir = data_dir("first-suffix-bytes");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        seed_records(&daemon, 8);
        let snapshot = finish_index(&mut daemon, 8);
        let empty_minimum = empty_page_minimum(&mut daemon, &snapshot);
        let continuation_minimum =
            continuation_too_small(&mut daemon, &snapshot, None, empty_minimum);
        let page = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                None,
                LifecycleBaselineBudget {
                    max_rows: usize::MAX,
                    max_bytes: continuation_minimum,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("smallest accepted continuation budget");
        assert!(!page.complete);
        assert!(page.sessions.is_empty());
        assert!(page.next.is_some());
        assert_page_within_budget(&page, continuation_minimum);
        assert_eq!(
            encoded_lifecycle_baseline_page_len(&page),
            continuation_minimum
        );
        assert_eq!(materialized_rows(&daemon), 1);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn later_suffix_byte_limit_stops_before_remaining_rows() {
        let data_dir = data_dir("later-suffix-bytes");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        seed_records(&daemon, 8);
        let snapshot = finish_index(&mut daemon, 8);
        let first = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                None,
                LifecycleBaselineBudget {
                    max_rows: 1,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("first suffix row");
        assert_eq!(first.sessions.len(), 1);
        assert_page_within_budget(&first, 64 * 1024);
        let empty_minimum = empty_page_minimum(&mut daemon, &snapshot);
        let continuation_minimum =
            continuation_too_small(&mut daemon, &snapshot, first.next.as_ref(), empty_minimum);
        let page = daemon
            .lifecycle_baseline_page(
                Some(&snapshot),
                first.next.as_ref(),
                LifecycleBaselineBudget {
                    max_rows: usize::MAX,
                    max_bytes: continuation_minimum,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("smallest accepted later continuation budget");
        assert!(!page.complete);
        assert!(page.sessions.is_empty());
        assert_eq!(page.next, first.next);
        assert_page_within_budget(&page, continuation_minimum);
        assert_eq!(
            encoded_lifecycle_baseline_page_len(&page),
            continuation_minimum
        );
        assert_eq!(materialized_rows(&daemon), 2);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
