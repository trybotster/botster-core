//! Core daemon supervisor and typed API implementation.

use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet, HashMap, VecDeque},
    hash::{Hash, Hasher},
    io,
    ops::Bound::{Excluded, Included, Unbounded},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
    time::{SystemTime, UNIX_EPOCH},
};

use botster_core::contract::terminal_adapter::TerminalAdapter;
use botster_core::TerminalScreenSize;
use botster_core::{
    BindTerminalAdapterError, BotsterEngineObservation, BotsterEngineOutput, ClientId, CoreSession,
    DefaultBotsterEngine, DefaultBotsterEngineError, DetachTerminalSubscriptionResult, EnvelopeId,
    EnvelopeTarget, ModeFlags, ModeFlagsReady, ModeFreshnessToken, ModeGatedPtyInputResult,
    NotificationId, NotificationInbox, QueueSource, RequestId, ResizePayload,
    RoutedEnvelopeQueueConfig, RoutedEnvelopeRouter, ScreenReady, SessionId, SessionIoEvent,
    SessionLifecycleState, SessionRuntimeError, SessionRuntimeErrorKind, SessionWorkerHealthReason,
    SessionWorkerStaleReason, SubscriptionId, TerminalBackendError, TerminalCapabilitySet,
    TerminalColorProfile, TerminalScreenState, TerminalSnapshotPayload,
    TerminalSubscriptionGeneration, TerminalSubscriptionRecord, TransportEgress,
    WorkerBackedBotsterEngine, WorkerProcessRuntimeOptions,
};
use botster_terminal_ghostty::{GhosttyAdapterConfig, GhosttyTerminal, GhosttyTerminalError};
use thiserror::Error;

use crate::api::{
    reserved_observe_slice_error, sanitize_observe_slice_error_message,
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest, AttachedSession,
    CaptureColorAndSnapshotRequest, CaptureColorAndSnapshotResult, CaptureSnapshotRequest,
    CaptureSnapshotResult, DaemonHealth, DaemonSession, DaemonStatus, DrainNotificationsRequest,
    DrainNotificationsResult, DrainResult, DrainRoutedEnvelopesRequest, DrainRoutedEnvelopesResult,
    GuardedWriteRequest, GuardedWriteResult, LifecycleBaselineBudget, NotificationStatusResult,
    ObserveLifecycleBudget, ObserveLifecycleCursor, ObserveLifecyclePassId, ObserveLifecycleSlice,
    ObserveLifecycleSliceError, PostNotificationRequest, PostNotificationResult,
    PublishRoutedEnvelopeRequest, PublishRoutedEnvelopeResult, ReadModeFlagsRequest,
    ReadModeFlagsResult, ReadScreenRequest, ReadScreenResult, RoutedEnvelopeDeliveryStateResult,
    SessionAdoptionReport, SessionAdoptionState, SessionLifecycleBaseline,
    SessionLifecycleBaselinePage, SessionLifecycleChange, SessionLifecycleChangeKind,
    SessionLifecycleChanges, SessionLifecycleCursor, SessionLifecyclePage,
    SessionLifecyclePageError, SessionLifecycleRecord, SessionLifecycleResyncReason,
    SessionLifecycleSourceId, SpawnSessionRequest,
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
    /// Parent wait bound for correlated mode-gated PTY input RPC.
    pub mode_gated_input_timeout: Duration,
    /// Optional per-request worker admit hold for deterministic race tests.
    pub test_mode_gated_hold_ms: Option<u64>,
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
    /// Test-only: fail snapshot history after READY.
    pub test_fail_snapshot_history_after_ready: bool,
    /// Test-only: make observe's per-session runtime drain fail for this id.
    pub test_fail_runtime_drain_for: Option<SessionId>,
    /// Test-only: `Display` text for the injected observe drain failure.
    pub test_fail_runtime_drain_message: Option<String>,
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
            mode_gated_input_timeout: botster_core::DEFAULT_MODE_GATED_INPUT_TIMEOUT,
            test_mode_gated_hold_ms: None,
            test_hold_after_read_ms: None,
            test_write_block_until_unix_ms: None,
            test_write_max_chunk: None,
            test_pending_capacity: None,
            test_hold_after_enqueue_ms: None,
            pty_reader_chunk_capacity: None,
            test_worker_egress_capacity: None,
            test_fail_snapshot_history_after_ready: false,
            test_fail_runtime_drain_for: None,
            test_fail_runtime_drain_message: None,
            #[cfg(test)]
            test_baseline_elapsed_per_op: None,
        }
    }

    /// Override the mode-gated input RPC wait bound (tests may use a short timeout).
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

    /// Fail snapshot history after READY for a worker integration test.
    #[must_use]
    pub const fn with_test_fail_snapshot_history_after_ready(mut self, enabled: bool) -> Self {
        self.test_fail_snapshot_history_after_ready = enabled;
        self
    }

    /// Fail observe's per-session runtime drain for this session id.
    #[must_use]
    pub fn with_test_fail_runtime_drain_for(mut self, session_id: Option<SessionId>) -> Self {
        self.test_fail_runtime_drain_for = session_id;
        self
    }

    /// Override the injected observe drain failure `Display` text.
    #[must_use]
    pub fn with_test_fail_runtime_drain_message(mut self, message: Option<String>) -> Self {
        self.test_fail_runtime_drain_message = message;
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

/// Outcome of [`CoreDaemon::mode_gated_input`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeGatedInputOutcome {
    /// Plain input path wrote bytes without a freshness token.
    PlainWritten,
    /// Correlated mode-gated worker admit result.
    Gated(ModeGatedPtyInputResult),
}

/// Daemon API error.
#[derive(Debug, Error)]
pub enum CoreDaemonError {
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
}

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
pub struct CoreDaemon {
    config: CoreDaemonConfig,
    registry: SessionRegistry,
    engine: DaemonEngine,
    notification_inbox: NotificationInbox,
    envelope_router: RoutedEnvelopeRouter,
    pending_drain: Vec<PendingDrainResult>,
    retained_terminal: HashMap<SessionId, RetainedTerminalState>,
    last_mode_freshness: HashMap<SessionId, ModeFreshnessToken>,
    lifecycle_source_id: SessionLifecycleSourceId,
    lifecycle_sequence: u64,
    lifecycle_journal: VecDeque<SessionLifecycleChange>,
    journal_advanced: bool,
    observe_pass: Option<ObservePassState>,
    observe_live_sessions: BTreeMap<String, u64>,
    observe_live_generation: u64,
    baseline_freeze: Option<BaselineFreeze>,
    #[cfg(test)]
    observe_index_scans: u64,
    #[cfg(test)]
    baseline_index_scans: u64,
    #[cfg(test)]
    baseline_row_copies: u64,
    #[cfg(test)]
    baseline_page_encodes: u64,
    running: bool,
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

struct PendingDrainResult {
    session_id: SessionId,
    result: DrainResult,
}

#[derive(Clone)]
struct RetainedTerminalState {
    screen_text: String,
    snapshot: TerminalSnapshotPayload,
    mode_flags: Result<ModeFlags, TerminalBackendError>,
    mode_freshness: ModeFreshnessToken,
    /// Ghostty-owned colors frozen with the snapshot under one terminal borrow.
    color_profile: TerminalColorProfile,
}

enum ReadbackResolution {
    Live,
    Retained(RetainedTerminalState),
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
                options.mode_gated_input_timeout = config.mode_gated_input_timeout;
                options.test_mode_gated_hold_ms = config.test_mode_gated_hold_ms;
                options.test_hold_after_read_ms = config.test_hold_after_read_ms;
                options.test_write_block_until_unix_ms = config.test_write_block_until_unix_ms;
                options.test_write_max_chunk = config.test_write_max_chunk;
                options.test_pending_capacity = config.test_pending_capacity;
                options.test_hold_after_enqueue_ms = config.test_hold_after_enqueue_ms;
                options.ghostty_max_scrollback_bytes = ghostty_max_scrollback_bytes;
                options.terminal_color_profile = terminal_color_profile.clone();
                options.test_fail_snapshot_history_after_ready =
                    config.test_fail_snapshot_history_after_ready;
                if let Some(capacity) = config.pty_reader_chunk_capacity {
                    options.pty_reader_chunk_capacity = capacity;
                }
                if let Some(capacity) = config.test_worker_egress_capacity {
                    options.egress_capacity = capacity;
                }
                DaemonEngine::Worker(Box::new(worker_engine(
                    options,
                    ghostty_max_scrollback_bytes,
                    terminal_color_profile.clone(),
                )))
            })
            .unwrap_or_else(|| {
                DaemonEngine::Local(Box::new(local_engine(
                    ghostty_max_scrollback_bytes,
                    terminal_color_profile,
                )))
            });
        let envelope_queue = config.routed_envelope_queue.clone();
        Self {
            config,
            registry,
            engine,
            notification_inbox: NotificationInbox::new(),
            envelope_router: RoutedEnvelopeRouter::with_config(envelope_queue),
            pending_drain: Vec::new(),
            retained_terminal: HashMap::new(),
            last_mode_freshness: HashMap::new(),
            lifecycle_source_id: new_lifecycle_source_id(),
            lifecycle_sequence: 0,
            lifecycle_journal: VecDeque::new(),
            journal_advanced: false,
            observe_pass: None,
            observe_live_sessions: BTreeMap::new(),
            observe_live_generation: 0,
            baseline_freeze: None,
            #[cfg(test)]
            observe_index_scans: 0,
            #[cfg(test)]
            baseline_index_scans: 0,
            #[cfg(test)]
            baseline_row_copies: 0,
            #[cfg(test)]
            baseline_page_encodes: 0,
            running: true,
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
            .registry
            .load_all()?
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
            .registry
            .load_all()?
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

    /// Attach a client through the existing subscription path.
    ///
    /// A local attach returns the complete route-owned bootstrap. A capable
    /// worker attach returns `Attaching`. Later [`Self::drain`] calls return
    /// route-owned incremental Snapshot frames, `Attached`, and then live
    /// output. No other client receives these route-owned frames.
    pub fn attach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        now_seconds: u64,
    ) -> Result<AttachedSession, CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session_mutable(&session_id)?;
        let output = self.engine.attach_client(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            now_seconds,
        )?;
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

    /// Bind a content-blind adapter to a live attach generation.
    ///
    /// After bind, this route's terminal frames leave only through the adapter.
    /// `drain` / `drain_subscription` do not also return those terminal frames.
    pub fn bind_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        adapter: Box<dyn TerminalAdapter + Send>,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session(&session_id)?;
        self.engine.bind_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            generation,
            capabilities,
            adapter,
        )?;
        Ok(())
    }

    /// Control-plane subscription inventory. No terminal state is included.
    #[must_use]
    pub fn list_terminal_subscriptions(&self) -> Vec<TerminalSubscriptionRecord> {
        self.engine.list_terminal_subscriptions()
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
        let resize_is_queued = self.engine.incremental_attach_active(&session_id);
        self.engine
            .resize(client_id, session_id.clone(), rows, cols, now_seconds)?;
        if !resize_is_queued {
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

    /// Drain one session's runtime output through subscription fanout.
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
        if let Some((rows, cols, resize_at)) = self.engine.take_applied_attach_resize(session_id) {
            self.persist_session_size(session_id, rows, cols, resize_at)?;
        }
        self.reconcile_lifecycle_observations(&result.observations, last_output_at)?;
        if self.engine_session_exited(session_id)
            && !self.retained_terminal.contains_key(session_id)
        {
            if let Err(error) = self.retain_final_terminal_state(session_id) {
                self.retain_pending_drain_result(session_id, result);
                return Err(error);
            }
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

    /// Read the current terminal screen through the production daemon path.
    ///
    /// Worker-backed sessions update the daemon-owned terminal shadow while
    /// runtime output is drained. This method drains before reading so callers
    /// do not need an explicit pre-read drain. Any client egress or
    /// observations produced by that internal drain are retained for the next
    /// explicit [`Self::drain`] call.
    pub fn read_screen(
        &mut self,
        request: ReadScreenRequest,
    ) -> Result<ReadScreenResult, CoreDaemonError> {
        self.ensure_running()?;
        if let ReadbackResolution::Retained(retained) =
            self.resolve_readback(&request.session_id, request.now_seconds)?
        {
            return Ok(ReadScreenResult {
                screen: ScreenReady {
                    request_id: request.request_id,
                    session_id: request.session_id,
                    text: retained.screen_text,
                },
            });
        }
        let mut output = self.engine.read_screen(
            request.request_id.clone(),
            request.session_id.clone(),
            request.now_seconds,
        )?;
        let screen = take_screen_ready(&mut output, &request.request_id)?;
        self.retain_pending_drain_result(
            &request.session_id,
            drain_result_from_engine_output(output),
        );
        Ok(ReadScreenResult { screen })
    }

    /// Read authoritative terminal mode flags through the production daemon path.
    pub fn read_mode_flags(
        &mut self,
        request: ReadModeFlagsRequest,
    ) -> Result<ReadModeFlagsResult, CoreDaemonError> {
        self.ensure_running()?;
        if let ReadbackResolution::Retained(retained) =
            self.resolve_readback(&request.session_id, request.now_seconds)?
        {
            let mode_flags = retained
                .mode_flags
                .map_err(managed_terminal_backend_error)?;
            return Ok(ReadModeFlagsResult {
                mode_flags: ModeFlagsReady {
                    request_id: request.request_id,
                    session_id: request.session_id,
                    mode_flags,
                    mode_freshness: retained.mode_freshness,
                },
            });
        }
        let mut output = self.engine.read_mode_flags(
            request.request_id.clone(),
            request.session_id.clone(),
            request.now_seconds,
        )?;
        let mode_flags = take_mode_flags_ready(&mut output, &request.request_id)?;
        self.last_mode_freshness
            .insert(request.session_id.clone(), mode_flags.mode_freshness);
        self.retain_pending_drain_result(
            &request.session_id,
            drain_result_from_engine_output(output),
        );
        Ok(ReadModeFlagsResult { mode_flags })
    }

    /// Admit mode-dependent PTY input under the worker atomic mode-gated path.
    ///
    /// When `expected_mode_freshness` is `None`, this is identical to
    /// [`Self::input`] (plain `FRAME_PTY_INPUT`). When `Some`, the production
    /// worker-backed path uses correlated mode-gated RPC; the worker is the
    /// correctness boundary. Parent drain is optimization-only.
    pub fn mode_gated_input(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        data: impl Into<Vec<u8>>,
        expected_mode_freshness: Option<ModeFreshnessToken>,
        now_seconds: u64,
    ) -> Result<ModeGatedInputOutcome, CoreDaemonError> {
        self.ensure_running()?;
        self.ensure_session_mutable(&session_id)?;
        let data = data.into();
        let Some(expected) = expected_mode_freshness else {
            self.engine
                .write_bytes(client_id, session_id, data, now_seconds)?;
            return Ok(ModeGatedInputOutcome::PlainWritten);
        };

        // Optimization-only pre-admit drain. Correctness is worker atomic admit.
        let _ = self.drain_runtime_for_readback(&session_id, now_seconds);

        let result = self
            .engine
            .mode_gated_pty_input(session_id.clone(), expected, data)?;
        self.last_mode_freshness
            .insert(session_id, result.mode_freshness);
        Ok(ModeGatedInputOutcome::Gated(result))
    }

    /// Capture the current terminal snapshot through the production daemon path.
    ///
    /// The payload is Ghostty-owned opaque terminal state (`GHOSTSNP` /
    /// `ghostty-terminal-snapshot-v1`). Scrollback retention is governed by
    /// [`CoreDaemonConfig::ghostty_max_scrollback_bytes`] (default 10 MB of
    /// Ghostty page-allocation budget). Ghostty stores page-quantized parsed
    /// terminal state rather than a raw PTY byte tail, so effective retained
    /// lines depend on terminal width.
    pub fn capture_snapshot(
        &mut self,
        request: CaptureSnapshotRequest,
    ) -> Result<CaptureSnapshotResult, CoreDaemonError> {
        self.ensure_running()?;
        if let ReadbackResolution::Retained(retained) =
            self.resolve_readback(&request.session_id, request.now_seconds)?
        {
            let payload = retained.snapshot;
            let snapshot = payload
                .clone()
                .into_snapshot_ready(request.request_id, request.session_id);
            return Ok(CaptureSnapshotResult { snapshot, payload });
        }
        let payload = self.engine.capture_snapshot_payload(&request.session_id)?;
        let snapshot = payload
            .clone()
            .into_snapshot_ready(request.request_id, request.session_id);
        Ok(CaptureSnapshotResult { snapshot, payload })
    }

    /// Capture current colors and GHOSTSNP from one terminal ownership section.
    ///
    /// This is the Hub-facing production ordering boundary: palette/special
    /// colors and the opaque snapshot are taken under the same session terminal
    /// borrow after the drain-before-read path used by other readbacks. Host
    /// `terminal_color_profile` remains spawn/initial baseline only; after
    /// session start Ghostty owns current colors (including OSC mutations).
    /// Retained post-exit freezes serve the same paired record without
    /// re-entering a live terminal.
    pub fn capture_color_and_snapshot(
        &mut self,
        request: CaptureColorAndSnapshotRequest,
    ) -> Result<CaptureColorAndSnapshotResult, CoreDaemonError> {
        self.ensure_running()?;
        if let ReadbackResolution::Retained(retained) =
            self.resolve_readback(&request.session_id, request.now_seconds)?
        {
            let payload = retained.snapshot;
            let snapshot = payload
                .clone()
                .into_snapshot_ready(request.request_id, request.session_id);
            return Ok(CaptureColorAndSnapshotResult {
                color_profile: retained.color_profile,
                snapshot,
                payload,
            });
        }
        let (color_profile, payload) = self
            .engine
            .capture_color_and_snapshot(&request.session_id)?;
        let snapshot = payload
            .clone()
            .into_snapshot_ready(request.request_id, request.session_id);
        Ok(CaptureColorAndSnapshotResult {
            color_profile,
            snapshot,
            payload,
        })
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
            registry_records: self.registry.load_all()?.len(),
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
            .registry
            .load_all()?
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
        self.retained_terminal.remove(session_id);
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

        let sessions: Vec<_> = self.engine.list_sessions();
        for session in sessions {
            self.shutdown_session(session.session_id, now_seconds)?;
        }
        self.retained_terminal.clear();
        self.running = false;
        Ok(())
    }

    fn shutdown_session(
        &mut self,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<(), CoreDaemonError> {
        self.ensure_session(&session_id)?;
        let (mut shutdown_drain, shutdown_error) =
            match self
                .engine
                .shutdown_session(session_id.clone(), "daemon shutdown", now_seconds)
            {
                Ok(output) => (drain_result_from_engine_output(output), None),
                Err(error) => (DrainResult::default(), Some(error)),
            };
        self.reconcile_lifecycle_observations(&shutdown_drain.observations, now_seconds)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut final_output_drained = self.engine_session_exited(&session_id);
        while !final_output_drained && Instant::now() < deadline {
            match self.engine.drain_runtime_once(&session_id, now_seconds) {
                Ok(output) => {
                    let drained = drain_result_from_engine_output(output);
                    self.reconcile_lifecycle_observations(&drained.observations, now_seconds)?;
                    merge_drain_result(&mut shutdown_drain, drained);
                    final_output_drained = self.engine_session_exited(&session_id);
                }
                Err(error) if is_session_not_found(&error) => {
                    final_output_drained = self.engine_session_exited(&session_id);
                    if !final_output_drained {
                        return Err(error.into());
                    }
                }
                Err(error) => return Err(error.into()),
            }
            if !final_output_drained {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if !final_output_drained {
            self.retain_pending_drain_result(&session_id, shutdown_drain);
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

        self.retain_pending_drain_result(&session_id, shutdown_drain);
        self.retain_final_terminal_state(&session_id)?;
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

    fn ensure_session(&self, session_id: &SessionId) -> Result<(), CoreDaemonError> {
        self.engine
            .session(session_id)
            .map(|_| ())
            .ok_or_else(|| CoreDaemonError::UnknownSession(session_id.clone()))
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

    fn resolve_readback(
        &mut self,
        session_id: &SessionId,
        now_seconds: u64,
    ) -> Result<ReadbackResolution, CoreDaemonError> {
        let registry_state = self.registry.load(session_id)?.map(|record| record.state);
        if matches!(registry_state, Some(RegistrySessionState::Stale)) {
            self.retained_terminal.remove(session_id);
            return Err(CoreDaemonError::SessionNotReadable(session_id.clone()));
        }

        let lifecycle = self
            .engine
            .session(session_id)
            .map(|session| session.lifecycle.clone());
        if let Some(retained) = self.retained_terminal.get(session_id) {
            if matches!(registry_state, Some(RegistrySessionState::Exited))
                || matches!(lifecycle, Some(SessionLifecycleState::Exited { .. }))
            {
                return Ok(ReadbackResolution::Retained(retained.clone()));
            }
        }

        let Some(lifecycle) = lifecycle else {
            return if matches!(
                registry_state,
                Some(
                    RegistrySessionState::Stopping
                        | RegistrySessionState::Exited
                        | RegistrySessionState::Stale
                )
            ) {
                Err(CoreDaemonError::SessionNotReadable(session_id.clone()))
            } else {
                Err(CoreDaemonError::UnknownSession(session_id.clone()))
            };
        };
        if matches!(
            lifecycle,
            SessionLifecycleState::Stopping
                | SessionLifecycleState::Exited { .. }
                | SessionLifecycleState::Failed { .. }
        ) || matches!(
            registry_state,
            Some(RegistrySessionState::Stopping | RegistrySessionState::Exited)
        ) {
            return Err(CoreDaemonError::SessionNotReadable(session_id.clone()));
        }

        self.drain_runtime_for_readback(session_id, now_seconds)?;
        if self.engine_session_exited(session_id) {
            self.retain_final_terminal_state(session_id)?;
            return Ok(ReadbackResolution::Retained(
                self.retained_terminal
                    .get(session_id)
                    .expect("final terminal state was just retained")
                    .clone(),
            ));
        }
        if matches!(
            self.engine
                .session(session_id)
                .map(|session| &session.lifecycle),
            Some(SessionLifecycleState::Stopping | SessionLifecycleState::Failed { .. })
        ) {
            return Err(CoreDaemonError::SessionNotReadable(session_id.clone()));
        }
        Ok(ReadbackResolution::Live)
    }

    fn retain_final_terminal_state(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(), CoreDaemonError> {
        if self.retained_terminal.contains_key(session_id) {
            return Ok(());
        }
        let (screen, snapshot, mode_flags) = self.engine.capture_terminal_state(session_id)?;
        let mode_freshness = self
            .last_mode_freshness
            .get(session_id)
            .copied()
            .unwrap_or_default();
        let color_profile = screen.color_profile.ok_or_else(|| {
            managed_terminal_backend_error(TerminalBackendError::operation_failed(
                "color_profile",
                "terminal did not expose a color profile for retained freeze",
            ))
        })?;
        self.retained_terminal.insert(
            session_id.clone(),
            RetainedTerminalState {
                screen_text: screen.plain_text,
                snapshot,
                mode_flags,
                mode_freshness,
                color_profile,
            },
        );
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

    fn drain_runtime_for_readback(
        &mut self,
        session_id: &SessionId,
        last_output_at: u64,
    ) -> Result<(), CoreDaemonError> {
        let output = self.engine.drain_runtime_once(session_id, last_output_at)?;
        let pending = drain_result_from_engine_output(output);
        self.retain_pending_drain_result(session_id, pending);
        Ok(())
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
                    let path = entry.path();
                    if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                        continue;
                    }
                    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                        continue;
                    };
                    let id = stem.to_string();
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
            *ops = ops.saturating_add(1);
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
            let mut candidate = page.clone();
            candidate.sessions.push(record);
            let following =
                self.next_baseline_membership_id(Some(&SessionId(next_id.clone())), false);
            candidate.complete = following.is_none();
            candidate.next = following.map(SessionId);
            *ops = ops.saturating_add(1);
            self.record_baseline_page_encode();
            if encoded_lifecycle_baseline_page_len(&candidate) > budget.max_bytes {
                page.next = Some(SessionId(next_id));
                page.complete = false;
                break;
            }
            page = candidate;
            cursor = Some(SessionId(next_id));
            if self.baseline_elapsed(started, *ops) >= budget.max_elapsed && !page.complete {
                break;
            }
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
        let output = if self
            .config
            .test_fail_runtime_drain_for
            .as_ref()
            .is_some_and(|failed| failed == session_id)
        {
            return Err(CoreDaemonError::Engine(DefaultBotsterEngineError::Runtime(
                SessionRuntimeError::new(
                    SessionRuntimeErrorKind::OutputFailed,
                    self.config
                        .test_fail_runtime_drain_message
                        .clone()
                        .unwrap_or_else(|| {
                            format!("test-injected observe drain failure: {}", session_id.0)
                        }),
                ),
            )));
        } else {
            match self.engine.drain_runtime_once(session_id, now_seconds) {
                Ok(output) => output,
                Err(error)
                    if is_session_not_found(&error) && self.engine_session_exited(session_id) =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error.into()),
            }
        };
        let result = drain_result_from_engine_output(output);
        if let Some((rows, cols, resize_at)) = self.engine.take_applied_attach_resize(session_id) {
            if let Err(error) = self.persist_session_size(session_id, rows, cols, resize_at) {
                self.retain_pending_drain_result(session_id, result);
                return Err(error);
            }
        }
        if let Err(error) = self.reconcile_lifecycle_observations(&result.observations, now_seconds)
        {
            self.retain_pending_drain_result(session_id, result);
            return Err(error);
        }
        if self.engine_session_exited(session_id)
            && !self.retained_terminal.contains_key(session_id)
        {
            if let Err(error) = self.retain_final_terminal_state(session_id) {
                self.retain_pending_drain_result(session_id, result);
                return Err(error);
            }
        }
        self.retain_pending_drain_result(session_id, result);
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

fn worker_engine(
    options: WorkerProcessRuntimeOptions,
    max_scrollback_bytes: usize,
    color_profile: Option<TerminalColorProfile>,
) -> WorkerBackedBotsterEngine {
    WorkerBackedBotsterEngine::with_options_and_terminal_backend_factory(options, move |size| {
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

fn take_mode_flags_ready(
    output: &mut BotsterEngineOutput,
    request_id: &RequestId,
) -> Result<ModeFlagsReady, CoreDaemonError> {
    let position = output
        .session_events
        .iter()
        .position(|event| match event {
            SessionIoEvent::ModeFlagsReady(mode_flags) => &mode_flags.request_id == request_id,
            _ => false,
        })
        .ok_or_else(|| CoreDaemonError::MissingModeFlagsResponse(request_id.clone()))?;
    match output.session_events.remove(position) {
        SessionIoEvent::ModeFlagsReady(mode_flags) => Ok(mode_flags),
        _ => unreachable!("position was selected from a ModeFlagsReady event"),
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

impl DaemonEngine {
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

    fn bind_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        adapter: Box<dyn TerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        match self {
            Self::Local(engine) => engine.bind_terminal_adapter(
                client_id,
                session_id,
                subscription_id,
                generation,
                capabilities,
                adapter,
            ),
            Self::Worker(engine) => engine.bind_terminal_adapter(
                client_id,
                session_id,
                subscription_id,
                generation,
                capabilities,
                adapter,
            ),
        }
    }

    fn list_terminal_subscriptions(&self) -> Vec<TerminalSubscriptionRecord> {
        match self {
            Self::Local(engine) => engine.list_terminal_subscriptions(),
            Self::Worker(engine) => engine.list_terminal_subscriptions(),
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

    fn mode_gated_pty_input(
        &mut self,
        session_id: SessionId,
        expected: ModeFreshnessToken,
        data: Vec<u8>,
    ) -> Result<ModeGatedPtyInputResult, CoreDaemonError> {
        match self {
            Self::Local(_) => Err(CoreDaemonError::Engine(
                DefaultBotsterEngineError::Runtime(SessionRuntimeError::new(
                    SessionRuntimeErrorKind::InputFailed,
                    "mode-gated input requires a worker-backed daemon (CoreDaemonConfig::with_worker_path)",
                )),
            )),
            Self::Worker(engine) => engine
                .mode_gated_pty_input(session_id, expected, data)
                .map_err(CoreDaemonError::Engine),
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

    fn incremental_attach_active(&self, session_id: &SessionId) -> bool {
        match self {
            Self::Local(_) => false,
            Self::Worker(engine) => engine.incremental_attach_active(session_id),
        }
    }

    fn take_applied_attach_resize(&mut self, session_id: &SessionId) -> Option<(u16, u16, u64)> {
        match self {
            Self::Local(_) => None,
            Self::Worker(engine) => engine.take_applied_attach_resize(session_id),
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

    fn read_screen(
        &mut self,
        request_id: RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.read_screen(request_id, session_id, now_seconds),
            Self::Worker(engine) => engine.read_screen(request_id, session_id, now_seconds),
        }
    }

    fn read_mode_flags(
        &mut self,
        request_id: RequestId,
        session_id: SessionId,
        now_seconds: u64,
    ) -> Result<botster_core::BotsterEngineOutput, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.read_mode_flags(request_id, session_id, now_seconds),
            Self::Worker(engine) => engine.read_mode_flags(request_id, session_id, now_seconds),
        }
    }

    fn capture_snapshot_payload(
        &mut self,
        session_id: &SessionId,
    ) -> Result<botster_core::TerminalSnapshotPayload, DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.capture_snapshot_payload(session_id),
            Self::Worker(engine) => engine.capture_snapshot_payload(session_id),
        }
    }

    fn capture_terminal_state(
        &mut self,
        session_id: &SessionId,
    ) -> Result<
        (
            TerminalScreenState,
            TerminalSnapshotPayload,
            Result<ModeFlags, TerminalBackendError>,
        ),
        DefaultBotsterEngineError,
    > {
        match self {
            Self::Local(engine) => engine.capture_terminal_state(session_id),
            Self::Worker(engine) => engine.capture_terminal_state(session_id),
        }
    }

    fn capture_color_and_snapshot(
        &mut self,
        session_id: &SessionId,
    ) -> Result<(TerminalColorProfile, TerminalSnapshotPayload), DefaultBotsterEngineError> {
        match self {
            Self::Local(engine) => engine.capture_color_and_snapshot(session_id),
            Self::Worker(engine) => engine.capture_color_and_snapshot(session_id),
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
        let live_modes = daemon
            .read_mode_flags(ReadModeFlagsRequest {
                request_id: RequestId("live-modes".to_string()),
                session_id: session_id.clone(),
                now_seconds: 14,
            })
            .expect("live mode flags");
        daemon
            .shutdown(Some(session_id.clone()), 15)
            .expect("shutdown should retain terminal state");
        let retained_modes = daemon
            .read_mode_flags(ReadModeFlagsRequest {
                request_id: RequestId("retained-modes".to_string()),
                session_id,
                now_seconds: 16,
            })
            .expect("retained mode flags");
        assert_eq!(
            live_modes.mode_flags.mode_flags.mouse_mode,
            retained_modes.mode_flags.mode_flags.mouse_mode
        );
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
        assert!(matches!(
            daemon.read_screen(ReadScreenRequest {
                request_id: RequestId("failed-final-screen".to_string()),
                session_id: session_id.clone(),
                now_seconds: 34,
            }),
            Err(CoreDaemonError::SessionNotReadable(session)) if session == session_id
        ));

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
        CoreDaemon {
            registry: SessionRegistry::new(&data_dir),
            engine: DaemonEngine::Local(Box::new(engine)),
            config,
            notification_inbox: NotificationInbox::new(),
            envelope_router: RoutedEnvelopeRouter::new(),
            pending_drain: Vec::new(),
            retained_terminal: HashMap::new(),
            last_mode_freshness: HashMap::new(),
            lifecycle_source_id: new_lifecycle_source_id(),
            lifecycle_sequence: 0,
            lifecycle_journal: VecDeque::new(),
            journal_advanced: false,
            observe_pass: None,
            observe_live_sessions: BTreeMap::new(),
            observe_live_generation: 0,
            baseline_freeze: None,
            observe_index_scans: 0,
            baseline_index_scans: 0,
            baseline_row_copies: 0,
            baseline_page_encodes: 0,
            running: true,
        }
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
        assert_eq!(daemon.baseline_index_scans, 1);
        assert_eq!(daemon.baseline_row_copies, 0);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn mid_work_elapsed_hook_stops_after_partial_index_progress() {
        let data_dir = data_dir("elapsed-hook");
        let mut daemon = CoreDaemon::new(
            CoreDaemonConfig::new(&data_dir)
                .with_test_baseline_elapsed_per_op(Duration::from_millis(1)),
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
                    max_elapsed: Duration::from_millis(1),
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
                    max_elapsed: Duration::from_millis(1),
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
        let indexed = daemon
            .lifecycle_baseline_page(
                None,
                None,
                LifecycleBaselineBudget {
                    max_rows: 4,
                    max_bytes: 64 * 1024,
                    max_elapsed: Duration::MAX,
                },
            )
            .expect("finish index");
        assert!(!indexed.complete);
        assert!(indexed.sessions.is_empty());
        assert_eq!(daemon.baseline_index_scans, 4);
        let first = daemon
            .lifecycle_baseline_page(
                Some(&indexed.snapshot_sequence),
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
        assert_eq!(daemon.baseline_row_copies, 1);
        let scans_after_first = daemon.baseline_index_scans;
        let second = daemon
            .lifecycle_baseline_page(
                Some(&indexed.snapshot_sequence),
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
        assert_eq!(daemon.baseline_index_scans, scans_after_first);
        assert_eq!(daemon.baseline_row_copies, 2);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
