//! Terminal stream scheme 2: one immutable binary body per session event.
//!
//! A [`TerminalFrame`] owns one complete `TerminalBody` in an `Arc<[u8]>`.
//! Core builds the body once per session event and shares it with every
//! route. Hub copies the bytes to a transport and never decodes the body.
//!
//! ```text
//! TerminalBody = [u8 scheme = 2][u8 kind][u16 LE flags][u32 LE body_len][body]
//! ```
//!
//! Semantic decode of `body` lives in `botster-terminal-protocol-client`.

use std::sync::Arc;

/// Scheme byte at offset 0 of every `TerminalBody`. Exact equality.
pub const TERMINAL_STREAM_SCHEME_VERSION: u8 = 2;
/// Fixed `TerminalBody` header length in bytes.
pub const TERMINAL_BODY_HEADER_BYTES: usize = 8;
/// Egress queue ceiling per route in frames. Overflow emits `ROUTE_RESYNC`.
pub const MAX_ROUTE_EGRESS_FRAMES: usize = 64;
/// Egress queue ceiling per route in queued `TerminalBody` bytes.
pub const MAX_ROUTE_EGRESS_BYTES: usize = 4 * 1024 * 1024;
/// Largest `body` a producer may place in one frame.
///
/// One frame must fit the per-route egress byte budget on its own. Snapshot
/// history is delivered as page-sized frames, never as one oversized frame.
pub const MAX_TERMINAL_BODY_BYTES: usize = MAX_ROUTE_EGRESS_BYTES - TERMINAL_BODY_HEADER_BYTES;

/// Frame kind at byte offset 1 of every `TerminalBody`.
///
/// Kinds 1 through 6 are shared session-event bodies. Kinds 16 through 19 are
/// personalized route bodies. Values are frozen wire bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TerminalKind {
    /// Raw PTY bytes.
    Output = 1,
    /// Raw GHOSTSNP bytes for the ready phase of a worker export.
    SnapshotReady = 2,
    /// Raw GHOSTSNP bytes for one history page.
    SnapshotHistory = 3,
    /// Snapshot delivery is complete. Empty body.
    SnapshotFinish = 4,
    /// Session process exited. Body `[u8 has_code][i32 LE code]`.
    ProcessExit = 5,
    /// Terminal modes changed. Body `[u32 LE mode_bits][u16 LE rows][u16 LE cols]`.
    Modes = 6,
    /// Route attach state. Body `[u8 state]`.
    AttachState = 16,
    /// Result of one input operation on this route.
    InputResult = 17,
    /// Retained history is unavailable. Body `[u8 reason]`.
    HistoryUnavailable = 18,
    /// Route egress overflowed. Empty body. The route restarts at `SnapshotReady`.
    RouteResync = 19,
}

impl TerminalKind {
    /// Every published kind, in wire-value order.
    pub const ALL: &'static [Self] = &[
        Self::Output,
        Self::SnapshotReady,
        Self::SnapshotHistory,
        Self::SnapshotFinish,
        Self::ProcessExit,
        Self::Modes,
        Self::AttachState,
        Self::InputResult,
        Self::HistoryUnavailable,
        Self::RouteResync,
    ];

    /// Decode a kind byte.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            1 => Self::Output,
            2 => Self::SnapshotReady,
            3 => Self::SnapshotHistory,
            4 => Self::SnapshotFinish,
            5 => Self::ProcessExit,
            6 => Self::Modes,
            16 => Self::AttachState,
            17 => Self::InputResult,
            18 => Self::HistoryUnavailable,
            19 => Self::RouteResync,
            _ => return None,
        })
    }

    /// Wire byte for this kind.
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        self as u8
    }

    /// Whether one body is shared by every route of the session.
    ///
    /// Personalized kinds carry route-specific state and are built per route.
    #[must_use]
    pub const fn is_shared(self) -> bool {
        (self as u8) < 16
    }

    /// Stable snake_case name used by generated TypeScript and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Output => "output",
            Self::SnapshotReady => "snapshot_ready",
            Self::SnapshotHistory => "snapshot_history",
            Self::SnapshotFinish => "snapshot_finish",
            Self::ProcessExit => "process_exit",
            Self::Modes => "modes",
            Self::AttachState => "attach_state",
            Self::InputResult => "input_result",
            Self::HistoryUnavailable => "history_unavailable",
            Self::RouteResync => "route_resync",
        }
    }
}

/// One complete immutable `TerminalBody`.
///
/// Cloning a frame clones the `Arc`, not the bytes. Hub adapters forward
/// [`Self::as_bytes`] and never inspect anything after the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFrame {
    kind: TerminalKind,
    bytes: Arc<[u8]>,
}

impl TerminalFrame {
    /// Build a frame by prefixing `body` with the scheme 2 header.
    ///
    /// This is the one allocation per session event on the output path.
    pub fn new(kind: TerminalKind, flags: u16, body: &[u8]) -> Result<Self, TerminalFrameError> {
        if body.len() > MAX_TERMINAL_BODY_BYTES {
            return Err(TerminalFrameError::BodyTooLarge {
                max: MAX_TERMINAL_BODY_BYTES,
                actual: body.len(),
            });
        }
        let mut bytes = Vec::with_capacity(TERMINAL_BODY_HEADER_BYTES + body.len());
        bytes.push(TERMINAL_STREAM_SCHEME_VERSION);
        bytes.push(kind.as_byte());
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(body);
        Ok(Self {
            kind,
            bytes: Arc::from(bytes),
        })
    }

    /// Build a frame with an empty body and zero flags.
    pub fn empty(kind: TerminalKind) -> Result<Self, TerminalFrameError> {
        Self::new(kind, 0, &[])
    }

    /// Validate complete `TerminalBody` bytes received from a transport.
    ///
    /// Validates the header only. Does not decode the body.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TerminalFrameError> {
        let kind = validate_header(bytes)?;
        Ok(Self {
            kind,
            bytes: Arc::from(bytes),
        })
    }

    /// Adopt already-shared `TerminalBody` bytes after validating the header.
    pub fn from_shared(bytes: Arc<[u8]>) -> Result<Self, TerminalFrameError> {
        let kind = validate_header(&bytes)?;
        Ok(Self { kind, bytes })
    }

    /// Frame kind from the header.
    #[must_use]
    pub const fn kind(&self) -> TerminalKind {
        self.kind
    }

    /// Flags from the header. Reserved; producers write zero.
    #[must_use]
    pub fn flags(&self) -> u16 {
        u16::from_le_bytes([self.bytes[2], self.bytes[3]])
    }

    /// Complete `TerminalBody` bytes: header plus body.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Shared handle to the complete `TerminalBody` bytes.
    #[must_use]
    pub fn shared_bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }

    /// Complete `TerminalBody` length in bytes. Egress budgets count this value.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the complete `TerminalBody` is empty. Never true for a valid frame.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Body bytes after the fixed header.
    ///
    /// Hub must not call this method. It exists for the semantic decoder in
    /// `botster-terminal-protocol-client` and for Core producers.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.bytes[TERMINAL_BODY_HEADER_BYTES..]
    }
}

fn validate_header(bytes: &[u8]) -> Result<TerminalKind, TerminalFrameError> {
    if bytes.len() < TERMINAL_BODY_HEADER_BYTES {
        return Err(TerminalFrameError::TruncatedHeader);
    }
    if bytes[0] != TERMINAL_STREAM_SCHEME_VERSION {
        return Err(TerminalFrameError::WrongSchemeVersion { found: bytes[0] });
    }
    let kind = TerminalKind::from_byte(bytes[1])
        .ok_or(TerminalFrameError::UnknownKind { found: bytes[1] })?;
    let declared = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let remaining = bytes.len() - TERMINAL_BODY_HEADER_BYTES;
    if declared != remaining {
        return Err(TerminalFrameError::BodyLengthMismatch {
            declared,
            remaining,
        });
    }
    if remaining > MAX_TERMINAL_BODY_BYTES {
        return Err(TerminalFrameError::BodyTooLarge {
            max: MAX_TERMINAL_BODY_BYTES,
            actual: remaining,
        });
    }
    Ok(kind)
}

/// Header validation or construction failure for a scheme 2 terminal frame.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalFrameError {
    /// Fewer than eight header bytes arrived.
    #[error("terminal frame header is truncated")]
    TruncatedHeader,
    /// Byte 0 is not the current scheme version.
    #[error("unsupported terminal stream scheme version {found}")]
    WrongSchemeVersion {
        /// Observed scheme byte.
        found: u8,
    },
    /// Byte 1 is not a published kind.
    #[error("unsupported terminal frame kind {found}")]
    UnknownKind {
        /// Observed kind byte.
        found: u8,
    },
    /// Declared body length does not equal the remaining byte count.
    #[error("terminal frame body length mismatch: declared {declared}, remaining {remaining}")]
    BodyLengthMismatch {
        /// Length field from the header.
        declared: usize,
        /// Bytes after the eight-byte header.
        remaining: usize,
    },
    /// Body exceeds the single-frame ceiling.
    #[error("terminal frame body too large: max {max}, actual {actual}")]
    BodyTooLarge {
        /// Maximum body length.
        max: usize,
        /// Observed body length.
        actual: usize,
    },
}
