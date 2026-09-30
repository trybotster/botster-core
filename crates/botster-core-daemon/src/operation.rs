//! Core pending operations, completions, and retention policy types.
//!
//! Every `CoreDaemon` call that touches a worker, the filesystem, or a slow
//! path is a pending operation: `begin` returns a [`PendingOperationId`] at
//! once, `pump_woken` reconciles worker replies and expired deadlines, and
//! `take_completions` returns one [`CoreCompletion`] per finished operation.
//! Nothing on the shared pump waits.

use std::sync::Arc;

use botster_core::{CoreSession, ModeFlags, SessionId, TerminalColorProfile};
use botster_terminal_protocol::{HistoryUnavailableReason, RouteId};
use serde::{Deserialize, Serialize};

use crate::api::{
    CaptureSnapshotRequest, HostInputRequest, ReadCursorRequest, ReadModeFlagsRequest,
    ReadScreenRequest, SpawnSessionRequest,
};
use crate::daemon::CoreDaemonError;

pub use botster_core::runtime::{
    SessionReservation, SessionReservationRefusal, SessionReservationRelease,
    SessionReservationState,
};

/// Maximum pending `Spawn` operations per daemon.
pub const MAX_PENDING_SPAWNS: usize = 4;
/// Maximum pending readbacks (`ReadScreen`, `ReadModeFlags`, `ReadCursor`, `CaptureSnapshot`) per session.
pub const MAX_PENDING_READBACKS_PER_SESSION: usize = 8;
/// Maximum open snapshot captures per client.
pub const MAX_OPEN_CAPTURES_PER_CLIENT: usize = 4;
/// Snapshot page size returned by `read_snapshot_page`.
pub const SNAPSHOT_PAGE_BYTES: usize = 256 * 1024;

/// Identity of one pending operation, unique for the daemon lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PendingOperationId(pub u64);

/// Identity of one in-memory snapshot capture held for paging.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CaptureId(pub String);

/// Host that owns a pending readback or capture, for per-client limits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CaptureOwner(pub String);

/// One operation the host asks Core to run off the pump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreOperation {
    /// Reserve an identity before the host publishes context or launches a PTY.
    ReserveSession(SessionId),
    /// Recover only the original reserve operation for this identity.
    LookupSessionReservation {
        /// Exact session identity.
        session_id: SessionId,
        /// Operation that originally obtained the reservation.
        reserve_operation_id: PendingOperationId,
    },
    /// Launch after the host publishes context under the reservation.
    SpawnReserved {
        /// Reservation returned by `ReserveSession`.
        reservation: SessionReservation,
        /// The request must name the reserved session.
        request: SpawnSessionRequest,
    },
    /// Release an unused reservation or authoritatively ended execution.
    ReleaseSessionReservation(SessionReservation),
    /// Launch a session. `begin` returns before the worker launch completes.
    Spawn(SpawnSessionRequest),
    /// Adopt a live worker from registry metadata.
    Adopt(SessionId),
    /// Request an orderly shutdown of one session.
    ShutdownSession(SessionId),
    /// Forget one already-terminal session.
    RemoveSession(SessionId),
    /// Release an ended session's engine state so the same id can be
    /// reserved and spawned again in place; the registry row stays.
    ReleaseEndedSession(SessionId),
    /// Read the plain text screen.
    ReadScreen(ReadScreenRequest),
    /// Read authoritative mode flags.
    ReadModeFlags(ReadModeFlagsRequest),
    /// Read the cursor and its row in one model read.
    ReadCursor(ReadCursorRequest),
    /// Write bytes to a session for the host, with no client identity. Never
    /// human input: it moves no input edge.
    HostInput(HostInputRequest),
    /// Capture a GHOSTSNP snapshot for paging by `read_snapshot_page`.
    CaptureSnapshot {
        /// Capture request.
        request: CaptureSnapshotRequest,
        /// Owner counted against [`MAX_OPEN_CAPTURES_PER_CLIENT`].
        owner: CaptureOwner,
    },
    /// Cancel one in-flight input operation on one route.
    CancelInput {
        /// Route that submitted the operation.
        route: RouteId,
        /// Attach generation of the route.
        generation: u64,
        /// Client operation id.
        operation_id: u64,
    },
}

/// Plain text screen readback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenReadback {
    /// Screen text. Shared with the retained object when the session ended.
    pub text: Arc<str>,
    /// Set when history is unavailable; `text` is then empty.
    pub unavailable: Option<HistoryUnavailableReason>,
}

/// How a host input write ended.
///
/// The worker answers once per operation. `outcome` is its typed answer:
/// `Written`, `RejectedLaneFull` when its input lane was full (nothing was
/// written), `Cancelled` after a cancel, or another refusal. A cancel or a
/// failure after a partial write reports the bytes actually written in
/// `written_pty_bytes`; a link failure leaves the counts unknown (`None`) and
/// the outcome `OutcomeUnknown`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInputOutcome {
    /// The worker's typed answer.
    pub outcome: botster_terminal_protocol::InputOutcome,
    /// Payload bytes the worker accepted, when it said.
    pub accepted_payload_bytes: Option<u64>,
    /// Bytes written to the PTY, when the worker said.
    pub written_pty_bytes: Option<u64>,
    /// Bounded diagnostic text.
    pub detail: String,
}

/// Cursor readback: the cursor and its row from one Ghostty model read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorReadback {
    /// The session's output counter (the one `session_edges` returns) when
    /// the read was issued. The read reflects at least every output chunk
    /// counted by then: the worker had sent each one before it read the
    /// request. It may also reflect later chunks.
    pub output_seq: u64,
    /// Cursor row and column (0-based, in cells), visibility, the row's
    /// trimmed text, and the untrimmed text left of the cursor.
    pub cursor: botster_core::CursorRow,
}

/// Mode flags readback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeFlagsReadback {
    /// Current or final mode flags.
    pub mode_flags: ModeFlags,
    /// Rows at the time of the read.
    pub rows: u16,
    /// Columns at the time of the read.
    pub cols: u16,
    /// Set when history is unavailable; flags are then default.
    pub unavailable: Option<HistoryUnavailableReason>,
}

/// Snapshot capture summary. Bytes are read through `read_snapshot_page`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotCapture {
    /// Capture handle valid until expiry, release, or owner close.
    pub capture_id: CaptureId,
    /// Total GHOSTSNP bytes.
    pub total_bytes: u64,
    /// Bytes per page, [`SNAPSHOT_PAGE_BYTES`] for every page but the last.
    pub page_bytes: u32,
    /// Page count.
    pub pages: u32,
    /// Rows represented by the snapshot.
    pub rows: u16,
    /// Columns represented by the snapshot.
    pub cols: u16,
    /// Ghostty palette and special colors frozen with the snapshot.
    pub color_profile: TerminalColorProfile,
    /// Set when history is unavailable; `pages` is then zero.
    pub unavailable: Option<HistoryUnavailableReason>,
}

/// One page of an open snapshot capture, shared with the capture buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPage {
    bytes: Arc<[u8]>,
    start: usize,
    end: usize,
}

impl SnapshotPage {
    /// Build a page view over shared capture bytes.
    #[must_use]
    pub fn new(bytes: Arc<[u8]>, start: usize, end: usize) -> Self {
        Self { bytes, start, end }
    }

    /// Page bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[self.start..self.end]
    }

    /// Whether this is the last page of the capture.
    #[must_use]
    pub fn is_last(&self) -> bool {
        self.end == self.bytes.len()
    }
}

/// Time an open capture stays readable without a page read or release.
pub const CAPTURE_IDLE_TTL_SECONDS: u64 = 60;

/// A reserved launch preserves the distinction between refusal and admission.
#[derive(Debug)]
pub enum ReservedSpawnResult {
    /// Core did not consume the supplied reservation during this operation.
    /// A valid unused reservation remains held until explicit release.
    Refused {
        /// The refusal reason.
        error: CoreDaemonError,
    },
    /// Core admitted the launch, but did not complete installation and persistence.
    /// The host must obtain a release receipt before it treats cleanup as complete.
    AdmittedFailure {
        /// The launch, installation, or persistence error.
        error: CoreDaemonError,
        /// Advisory ownership state when Core produced the completion.
        state: SessionReservationState,
    },
    /// The installed session now owns the identity and its registry record.
    Installed {
        /// The installed session.
        session: CoreSession,
    },
}

/// Result of one finished operation.
#[derive(Debug)]
pub enum CoreCompletion {
    /// Core granted a reservation or definitively refused without launching.
    ReserveSession {
        /// Operation identity.
        id: PendingOperationId,
        /// The reservation or a typed refusal, including `PendingLimitKind::Spawns`.
        result: Result<SessionReservation, CoreDaemonError>,
    },
    /// Core looked up the original reserve operation in the retained record.
    LookupSessionReservation {
        /// Operation identity.
        id: PendingOperationId,
        /// `None` means this record no longer owns that original operation.
        result: Result<Option<SessionReservation>, CoreDaemonError>,
    },
    /// A reserved launch reached a typed result.
    SpawnReserved {
        /// Operation identity.
        id: PendingOperationId,
        /// Refusal, admitted failure, or installed session.
        result: ReservedSpawnResult,
    },
    /// Core issued a release receipt or retained execution ownership.
    ReleaseSessionReservation {
        /// Operation identity.
        id: PendingOperationId,
        /// Only `Released` authorizes the host to treat the reservation as released.
        result: Result<SessionReservationRelease, CoreDaemonError>,
    },
    /// `Spawn` finished.
    Spawn {
        /// Operation identity.
        id: PendingOperationId,
        /// Spawned session or failure.
        result: Result<CoreSession, CoreDaemonError>,
    },
    /// `Adopt` finished.
    Adopt {
        /// Operation identity.
        id: PendingOperationId,
        /// Adopted session or failure.
        result: Result<CoreSession, CoreDaemonError>,
    },
    /// `ShutdownSession` finished.
    ShutdownSession {
        /// Operation identity.
        id: PendingOperationId,
        /// Success or failure.
        result: Result<(), CoreDaemonError>,
    },
    /// `RemoveSession` finished.
    RemoveSession {
        /// Operation identity.
        id: PendingOperationId,
        /// `true` when the session was removed, `false` when it was still live.
        result: Result<bool, CoreDaemonError>,
    },
    /// `ReleaseEndedSession` finished.
    ReleaseEndedSession {
        /// Operation identity.
        id: PendingOperationId,
        /// `true` when the session was released, `false` when it was still
        /// live or its row was not ended.
        result: Result<bool, CoreDaemonError>,
    },
    /// `ReadScreen` finished.
    ReadScreen {
        /// Operation identity.
        id: PendingOperationId,
        /// Screen text or failure.
        result: Result<ScreenReadback, CoreDaemonError>,
    },
    /// `ReadModeFlags` finished.
    ReadModeFlags {
        /// Operation identity.
        id: PendingOperationId,
        /// Mode flags or failure.
        result: Result<ModeFlagsReadback, CoreDaemonError>,
    },
    /// `ReadCursor` finished.
    ReadCursor {
        /// Operation identity.
        id: PendingOperationId,
        /// Cursor read or failure. An ended session is
        /// [`CoreDaemonError::SessionEnded`].
        result: Result<CursorReadback, CoreDaemonError>,
    },
    /// `HostInput` finished.
    HostInput {
        /// Operation identity.
        id: PendingOperationId,
        /// The worker's answer, or the failure to get one.
        result: Result<HostInputOutcome, CoreDaemonError>,
    },
    /// `CaptureSnapshot` finished.
    CaptureSnapshot {
        /// Operation identity.
        id: PendingOperationId,
        /// Capture summary or failure.
        result: Result<SnapshotCapture, CoreDaemonError>,
    },
    /// `CancelInput` was delivered. The route receives the `INPUT_RESULT`.
    CancelInput {
        /// Operation identity.
        id: PendingOperationId,
        /// `true` when the operation was still in flight.
        result: Result<bool, CoreDaemonError>,
    },
}

impl CoreCompletion {
    /// Operation identity of this completion.
    #[must_use]
    pub const fn id(&self) -> PendingOperationId {
        match self {
            Self::ReserveSession { id, .. }
            | Self::LookupSessionReservation { id, .. }
            | Self::SpawnReserved { id, .. }
            | Self::ReleaseSessionReservation { id, .. }
            | Self::Spawn { id, .. }
            | Self::Adopt { id, .. }
            | Self::ShutdownSession { id, .. }
            | Self::RemoveSession { id, .. }
            | Self::ReleaseEndedSession { id, .. }
            | Self::ReadScreen { id, .. }
            | Self::ReadModeFlags { id, .. }
            | Self::ReadCursor { id, .. }
            | Self::HostInput { id, .. }
            | Self::CaptureSnapshot { id, .. }
            | Self::CancelInput { id, .. } => *id,
        }
    }
}

/// Which pending limit `begin` hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingLimitKind {
    /// [`MAX_PENDING_SPAWNS`] reached.
    Spawns,
    /// [`MAX_PENDING_READBACKS_PER_SESSION`] reached.
    ReadbacksPerSession,
    /// [`MAX_OPEN_CAPTURES_PER_CLIENT`] reached.
    CapturesPerClient,
}

/// Retention policy for ended-session terminal history. Hub supplies values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Largest retained object; larger objects are not stored.
    pub max_object_bytes: usize,
    /// Aggregate retained bytes across sessions.
    pub max_total_bytes: usize,
    /// Aggregate retained sessions.
    pub max_sessions: usize,
}

/// Current retention accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RetentionAccounting {
    /// Retained bytes across sessions.
    pub total_bytes: usize,
    /// Retained sessions.
    pub sessions: usize,
    /// Evictions since daemon start.
    pub evictions: u64,
    /// Objects refused as oversize since daemon start.
    pub oversize_refusals: u64,
}

/// Retained final terminal state for one ended session.
///
/// Readback returns fields through `Arc` clones only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedTerminal {
    /// Final plain text screen.
    pub screen_text: Arc<str>,
    /// Final GHOSTSNP bytes, when export succeeded.
    pub snapshot: Option<Arc<[u8]>>,
    /// Final scheme 2 mode bits.
    pub mode_bits: u32,
    /// Final rows.
    pub rows: u16,
    /// Final columns.
    pub cols: u16,
    /// Final Ghostty palette and special colors.
    pub color_profile: TerminalColorProfile,
    /// Exit time in seconds, from the host clock at retention.
    pub exited_at: u64,
    /// Accounted bytes: screen text plus snapshot plus a 256-byte allowance.
    pub bytes: usize,
}

impl RetainedTerminal {
    /// Metadata allowance counted for every retained object.
    pub const METADATA_ALLOWANCE_BYTES: usize = 256;

    /// Accounted size for a screen and optional snapshot.
    #[must_use]
    pub fn accounted_bytes(screen_text: &str, snapshot: Option<&[u8]>) -> usize {
        screen_text.len() + snapshot.map_or(0, <[u8]>::len) + Self::METADATA_ALLOWANCE_BYTES
    }
}
