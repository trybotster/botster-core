//! Terminal screen and opaque snapshot contracts.

use serde::{Deserialize, Serialize};

use super::actor::{PreparedSnapshotRequest, SnapshotReady};
use super::session::{RequestId, SessionId};
use super::session_protocol::{ModeFlags, TerminalColorProfile};

/// Terminal dimensions associated with screen state and snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalScreenSize {
    /// Terminal row count.
    pub rows: u16,
    /// Terminal column count.
    pub cols: u16,
}

impl TerminalScreenSize {
    /// Build terminal dimensions.
    #[must_use]
    pub const fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }
}

/// Normalized terminal output bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalOutputChunk {
    /// Raw terminal bytes preserved for downstream processing.
    pub bytes: Vec<u8>,
}

impl TerminalOutputChunk {
    /// Build a normalized output chunk without interpreting the bytes.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

/// Correlation-free opaque terminal snapshot payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSnapshotPayload {
    /// Raw snapshot bytes.
    pub bytes: Vec<u8>,
    /// Terminal dimensions represented by the snapshot.
    pub size: TerminalScreenSize,
    /// Optional host-owned format label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

impl TerminalSnapshotPayload {
    /// Build an opaque snapshot payload.
    #[must_use]
    pub fn new(bytes: Vec<u8>, size: TerminalScreenSize, format: Option<String>) -> Self {
        Self {
            bytes,
            size,
            format,
        }
    }

    /// Convert a session-worker snapshot carrier into a reusable payload value.
    ///
    /// Existing correlated carriers do not store the optional host-owned format
    /// label, so callers that need it must reattach that label outside the
    /// carrier round trip.
    #[must_use]
    pub fn from_snapshot_ready(snapshot: SnapshotReady) -> Self {
        Self {
            bytes: snapshot.data,
            size: TerminalScreenSize::new(snapshot.rows, snapshot.cols),
            format: None,
        }
    }

    /// Convert this payload into the existing session-worker snapshot carrier.
    ///
    /// The optional host-owned format label is not preserved because
    /// `SnapshotReady` carries only request/session correlation, bytes, and
    /// dimensions.
    #[must_use]
    pub fn into_snapshot_ready(
        self,
        request_id: RequestId,
        session_id: SessionId,
    ) -> SnapshotReady {
        SnapshotReady {
            request_id,
            session_id,
            data: self.bytes,
            rows: self.size.rows,
            cols: self.size.cols,
        }
    }

    /// Convert this payload into an existing prepared-snapshot request carrier.
    ///
    /// The optional host-owned format label is not preserved because
    /// `PreparedSnapshotRequest` carries only request/session correlation,
    /// bytes, and recovery intent.
    #[must_use]
    pub fn into_prepared_snapshot_request(
        self,
        request_id: RequestId,
        session_id: SessionId,
        recovery: bool,
    ) -> PreparedSnapshotRequest {
        PreparedSnapshotRequest {
            request_id,
            session_id,
            snapshot: self.bytes,
            recovery,
        }
    }
}

/// Synchronous terminal screen state read from a runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalScreenState {
    /// Current terminal dimensions.
    pub size: TerminalScreenSize,
    /// Plain text view of the visible screen.
    pub plain_text: String,
    /// Current terminal title, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Current terminal working directory, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Current terminal mode flags.
    #[serde(default)]
    pub mode_flags: ModeFlags,
    /// Current terminal color profile, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_profile: Option<TerminalColorProfile>,
}

impl TerminalScreenState {
    /// Build terminal screen state.
    #[must_use]
    pub fn new(size: TerminalScreenSize, plain_text: String) -> Self {
        Self {
            size,
            plain_text,
            title: None,
            cwd: None,
            mode_flags: ModeFlags::default(),
            color_profile: None,
        }
    }
}

/// Lifecycle observation emitted by the terminal screen engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalScreenHook {
    /// Output bytes were accepted and preserved.
    OutputNormalized {
        /// Number of bytes accepted.
        bytes: usize,
    },
    /// Terminal dimensions were updated.
    Resized {
        /// New terminal dimensions.
        size: TerminalScreenSize,
    },
    /// A snapshot was captured.
    SnapshotCaptured {
        /// Snapshot dimensions.
        size: TerminalScreenSize,
    },
    /// A snapshot was replayed.
    SnapshotReplayed {
        /// Snapshot dimensions.
        size: TerminalScreenSize,
    },
    /// Screen state was read.
    ScreenRead {
        /// Screen dimensions.
        size: TerminalScreenSize,
    },
}
