//! Exact byte layouts for scheme 2 stream bodies and their enum tables.
//!
//! This module is the frozen source for every non-payload body layout. The
//! TypeScript codec in `botster-terminal-protocol-client` is generated from
//! these constants and tables, not maintained by hand.
//!
//! All multi-byte integers in stream bodies are little-endian. Input frames
//! (`input_frame.rs`) use big-endian, matching the existing header order.

use crate::frame::{TerminalFrame, TerminalFrameError, TerminalKind};

/// Body length of `PROCESS_EXIT`: `[u8 has_code][i32 LE code]`.
pub const PROCESS_EXIT_BODY_BYTES: usize = 5;
/// Body length of `MODES`: `[u32 LE mode_bits][u16 LE rows][u16 LE cols]`.
pub const MODES_BODY_BYTES: usize = 8;
/// Body length of `ATTACH_STATE`: `[u8 state]`.
pub const ATTACH_STATE_BODY_BYTES: usize = 1;
/// Body length of `HISTORY_UNAVAILABLE`: `[u8 reason]`.
pub const HISTORY_UNAVAILABLE_BODY_BYTES: usize = 1;
/// `ROUTE_RESYNC` body length: `[u32 LE from_epoch][u32 LE to_epoch]`.
pub const ROUTE_RESYNC_BODY_BYTES: usize = 8;
/// Fixed prefix length of `INPUT_RESULT` before `detail`.
///
/// `[u64 operation_id][u8 outcome][u8 has_accepted][u64 accepted_payload_bytes]`
/// `[u8 has_written][u64 written_pty_bytes][u32 mode_bits][u16 detail_len]`.
pub const INPUT_RESULT_PREFIX_BYTES: usize = 8 + 1 + 1 + 8 + 1 + 8 + 4 + 2;
/// Maximum UTF-8 `detail` bytes on one `INPUT_RESULT`.
pub const MAX_INPUT_RESULT_DETAIL_BYTES: usize = 1024;

/// Mode bit flags carried by `MODES` and `INPUT_RESULT`.
///
/// The `u32` is a bit set. Bits not listed here are zero.
pub mod mode_bits {
    /// Kitty keyboard protocol has at least one flag enabled.
    pub const KITTY_KEYBOARD: u32 = 1 << 0;
    /// Cursor is visible (DECTCEM).
    pub const CURSOR_VISIBLE: u32 = 1 << 1;
    /// Bracketed paste is enabled (mode 2004).
    pub const BRACKETED_PASTE: u32 = 1 << 2;
    /// Normal mouse tracking (mode 1000).
    pub const MOUSE_NORMAL: u32 = 1 << 3;
    /// Any-event mouse tracking (mode 1003).
    pub const MOUSE_ANY: u32 = 1 << 4;
    /// Button-event mouse tracking (mode 1002).
    pub const MOUSE_BUTTON: u32 = 1 << 5;
    /// SGR mouse encoding (mode 1006).
    pub const MOUSE_SGR: u32 = 1 << 6;
    /// Alternate screen is active (mode 1047 or 1049).
    pub const ALT_SCREEN: u32 = 1 << 7;
    /// Focus reporting is enabled (mode 1004).
    pub const FOCUS_REPORTING: u32 = 1 << 8;
    /// Application cursor keys (DECCKM, mode 1).
    pub const APPLICATION_CURSOR: u32 = 1 << 9;

    /// Named bits in definition order, for generated tables.
    pub const ALL: &[(&str, u32)] = &[
        ("KITTY_KEYBOARD", KITTY_KEYBOARD),
        ("CURSOR_VISIBLE", CURSOR_VISIBLE),
        ("BRACKETED_PASTE", BRACKETED_PASTE),
        ("MOUSE_NORMAL", MOUSE_NORMAL),
        ("MOUSE_ANY", MOUSE_ANY),
        ("MOUSE_BUTTON", MOUSE_BUTTON),
        ("MOUSE_SGR", MOUSE_SGR),
        ("ALT_SCREEN", ALT_SCREEN),
        ("FOCUS_REPORTING", FOCUS_REPORTING),
        ("APPLICATION_CURSOR", APPLICATION_CURSOR),
    ];
}

/// Define a `u8` wire enum with a complete inventory and byte decode.
macro_rules! byte_enum {
    (
        $(#[$enum_meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident = $value:literal => $wire:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant = $value,
            )+
        }

        impl $name {
            /// Every published variant, in wire-value order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Decode a wire byte.
            #[must_use]
            pub const fn from_byte(byte: u8) -> Option<Self> {
                match byte {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// Wire byte.
            #[must_use]
            pub const fn as_byte(self) -> u8 {
                self as u8
            }

            /// Stable snake_case name used by generated TypeScript.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }
        }
    };
}

byte_enum! {
    /// `ATTACH_STATE` body value.
    pub enum AttachStateCode {
        /// Attach was requested; no snapshot has been sent.
        Attaching = 1 => "attaching",
        /// Route is attached. `MODES` and `SNAPSHOT_READY` follow.
        Attached = 2 => "attached",
        /// Route was detached by the host or by session teardown.
        Detached = 3 => "detached",
        /// Attach failed before `SNAPSHOT_READY`.
        Failed = 4 => "failed",
    }
}

byte_enum! {
    /// `HISTORY_UNAVAILABLE` body value.
    pub enum HistoryUnavailableReason {
        /// Retained history was evicted by the retention policy.
        Evicted = 1 => "evicted",
        /// The host restarted; ended-session history is not persisted.
        Restart = 2 => "restart",
        /// The ended object exceeded the retention object cap and was not stored.
        Oversize = 3 => "oversize",
        /// The worker snapshot export failed.
        CaptureFailed = 4 => "capture_failed",
    }
}

byte_enum! {
    /// `INPUT_RESULT` outcome byte. Exactly one result per operation.
    pub enum InputOutcome {
        /// `accepted_payload_bytes` and `written_pty_bytes` are both known.
        Written = 1 => "written",
        /// PTY write failed after positive progress; `written_pty_bytes` is known.
        PartialWrite = 2 => "partial_write",
        /// Zero PTY progress; `detail` carries the error.
        WriteFailed = 3 => "write_failed",
        /// Cancel honoured; `written_pty_bytes` reports progress so far.
        Cancelled = 4 => "cancelled",
        /// PTY closed or session exiting; zero progress.
        RejectedNotWritable = 5 => "rejected_not_writable",
        /// Payload exceeds a size ceiling.
        RejectedTooLarge = 6 => "rejected_too_large",
        /// Paste is unsafe and `allow_unsafe` was not set.
        RejectedUnsafePaste = 7 => "rejected_unsafe_paste",
        /// A per-session or per-client lane bound is full.
        RejectedLaneFull = 8 => "rejected_lane_full",
        /// Nonincreasing id, unknown paste operation, or malformed body.
        RejectedProtocol = 9 => "rejected_protocol",
        /// The session ended before the operation ran.
        SessionEnded = 10 => "session_ended",
        /// The worker link failed after admission; progress is unknown.
        OutcomeUnknown = 11 => "outcome_unknown",
    }
}

impl InputOutcome {
    /// Whether a client may resubmit the same payload after this outcome.
    ///
    /// Outcomes with unknown or partial PTY progress are never retried.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::RejectedNotWritable
                | Self::RejectedTooLarge
                | Self::RejectedUnsafePaste
                | Self::RejectedLaneFull
                | Self::RejectedProtocol
                | Self::SessionEnded
        )
    }
}

/// Decoded `MODES` body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ModesBody {
    /// Bit set from [`mode_bits`].
    pub mode_bits: u32,
    /// Terminal rows.
    pub rows: u16,
    /// Terminal columns.
    pub cols: u16,
}

/// Decoded `PROCESS_EXIT` body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessExitBody {
    /// Exit code when the process exited normally.
    pub code: Option<i32>,
}

/// Decoded `INPUT_RESULT` body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputResultBody {
    /// Client-chosen operation id echoed from the input frame.
    pub operation_id: u64,
    /// Outcome for this operation.
    pub outcome: InputOutcome,
    /// Client payload bytes the worker admitted. Absent means unknown.
    pub accepted_payload_bytes: Option<u64>,
    /// Bytes written to the PTY including encoder-added bytes. Absent means unknown.
    pub written_pty_bytes: Option<u64>,
    /// Mode bits after the operation, from [`mode_bits`].
    pub mode_bits: u32,
    /// UTF-8 diagnostic, at most [`MAX_INPUT_RESULT_DETAIL_BYTES`] bytes.
    pub detail: String,
}

/// Body decode failure for a fixed-layout stream frame.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalBodyError {
    /// Frame kind does not match the requested decoder.
    #[error("expected terminal frame kind {expected:?}, got {actual:?}")]
    WrongKind {
        /// Kind the decoder handles.
        expected: TerminalKind,
        /// Kind on the frame.
        actual: TerminalKind,
    },
    /// Body length does not match the fixed layout.
    #[error("terminal body length must be {expected}, got {actual}")]
    BodyLength {
        /// Required body length.
        expected: usize,
        /// Observed body length.
        actual: usize,
    },
    /// A byte-enum field holds an unpublished value.
    #[error("unsupported {field} value {found}")]
    UnknownValue {
        /// Field name.
        field: &'static str,
        /// Observed byte.
        found: u8,
    },
    /// `detail` is not UTF-8 or exceeds its ceiling.
    #[error("input result detail is invalid")]
    InvalidDetail,
    /// Frame construction failed.
    #[error(transparent)]
    Frame(#[from] TerminalFrameError),
}

/// Encode an `OUTPUT` frame from raw PTY bytes. One allocation.
pub fn encode_output(bytes: &[u8]) -> Result<TerminalFrame, TerminalFrameError> {
    TerminalFrame::new(TerminalKind::Output, 0, bytes)
}

/// Encode a `SNAPSHOT_READY` frame from raw GHOSTSNP bytes.
pub fn encode_snapshot_ready(bytes: &[u8]) -> Result<TerminalFrame, TerminalFrameError> {
    TerminalFrame::new(TerminalKind::SnapshotReady, 0, bytes)
}

/// Encode a `SNAPSHOT_HISTORY` frame from one raw GHOSTSNP history page.
pub fn encode_snapshot_history(bytes: &[u8]) -> Result<TerminalFrame, TerminalFrameError> {
    TerminalFrame::new(TerminalKind::SnapshotHistory, 0, bytes)
}

/// Encode an empty `SNAPSHOT_FINISH` frame.
pub fn encode_snapshot_finish() -> Result<TerminalFrame, TerminalFrameError> {
    TerminalFrame::empty(TerminalKind::SnapshotFinish)
}

/// Encode a `ROUTE_RESYNC` frame carrying the epoch transition.
///
/// The envelope `stream_epoch` of this frame must equal `to_epoch`.
pub fn encode_route_resync(
    from_epoch: u32,
    to_epoch: u32,
) -> Result<TerminalFrame, TerminalFrameError> {
    let mut body = [0u8; ROUTE_RESYNC_BODY_BYTES];
    body[..4].copy_from_slice(&from_epoch.to_le_bytes());
    body[4..].copy_from_slice(&to_epoch.to_le_bytes());
    TerminalFrame::new(TerminalKind::RouteResync, 0, &body)
}

/// Decoded `ROUTE_RESYNC` body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteResyncBody {
    /// Epoch the route leaves. Must equal the client's accepted epoch.
    pub from_epoch: u32,
    /// Epoch the route enters. Must equal the frame's envelope epoch.
    pub to_epoch: u32,
}

/// Decode a `ROUTE_RESYNC` body.
pub fn decode_route_resync(frame: &TerminalFrame) -> Result<RouteResyncBody, TerminalBodyError> {
    let body = expect_body(frame, TerminalKind::RouteResync, ROUTE_RESYNC_BODY_BYTES)?;
    Ok(RouteResyncBody {
        from_epoch: u32::from_le_bytes([body[0], body[1], body[2], body[3]]),
        to_epoch: u32::from_le_bytes([body[4], body[5], body[6], body[7]]),
    })
}

/// Encode a `PROCESS_EXIT` frame.
pub fn encode_process_exit(code: Option<i32>) -> Result<TerminalFrame, TerminalFrameError> {
    let mut body = [0u8; PROCESS_EXIT_BODY_BYTES];
    if let Some(code) = code {
        body[0] = 1;
        body[1..5].copy_from_slice(&code.to_le_bytes());
    }
    TerminalFrame::new(TerminalKind::ProcessExit, 0, &body)
}

/// Encode a `MODES` frame.
pub fn encode_modes(modes: ModesBody) -> Result<TerminalFrame, TerminalFrameError> {
    let mut body = [0u8; MODES_BODY_BYTES];
    body[0..4].copy_from_slice(&modes.mode_bits.to_le_bytes());
    body[4..6].copy_from_slice(&modes.rows.to_le_bytes());
    body[6..8].copy_from_slice(&modes.cols.to_le_bytes());
    TerminalFrame::new(TerminalKind::Modes, 0, &body)
}

/// Encode an `ATTACH_STATE` frame.
pub fn encode_attach_state(state: AttachStateCode) -> Result<TerminalFrame, TerminalFrameError> {
    TerminalFrame::new(TerminalKind::AttachState, 0, &[state.as_byte()])
}

/// Encode a `HISTORY_UNAVAILABLE` frame.
pub fn encode_history_unavailable(
    reason: HistoryUnavailableReason,
) -> Result<TerminalFrame, TerminalFrameError> {
    TerminalFrame::new(TerminalKind::HistoryUnavailable, 0, &[reason.as_byte()])
}

/// Encode an `INPUT_RESULT` frame. `detail` is truncated on a UTF-8 boundary.
pub fn encode_input_result(result: &InputResultBody) -> Result<TerminalFrame, TerminalFrameError> {
    let detail = truncate_detail(&result.detail);
    let mut body = Vec::with_capacity(INPUT_RESULT_PREFIX_BYTES + detail.len());
    body.extend_from_slice(&result.operation_id.to_le_bytes());
    body.push(result.outcome.as_byte());
    body.push(u8::from(result.accepted_payload_bytes.is_some()));
    body.extend_from_slice(&result.accepted_payload_bytes.unwrap_or(0).to_le_bytes());
    body.push(u8::from(result.written_pty_bytes.is_some()));
    body.extend_from_slice(&result.written_pty_bytes.unwrap_or(0).to_le_bytes());
    body.extend_from_slice(&result.mode_bits.to_le_bytes());
    body.extend_from_slice(&(detail.len() as u16).to_le_bytes());
    body.extend_from_slice(detail.as_bytes());
    TerminalFrame::new(TerminalKind::InputResult, 0, &body)
}

fn truncate_detail(detail: &str) -> &str {
    if detail.len() <= MAX_INPUT_RESULT_DETAIL_BYTES {
        return detail;
    }
    let mut end = MAX_INPUT_RESULT_DETAIL_BYTES;
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    &detail[..end]
}

/// Decode a `PROCESS_EXIT` body.
pub fn decode_process_exit(frame: &TerminalFrame) -> Result<ProcessExitBody, TerminalBodyError> {
    let body = expect_body(frame, TerminalKind::ProcessExit, PROCESS_EXIT_BODY_BYTES)?;
    let code = match body[0] {
        0 => None,
        1 => Some(i32::from_le_bytes([body[1], body[2], body[3], body[4]])),
        found => {
            return Err(TerminalBodyError::UnknownValue {
                field: "has_code",
                found,
            })
        }
    };
    Ok(ProcessExitBody { code })
}

/// Decode a `MODES` body.
pub fn decode_modes(frame: &TerminalFrame) -> Result<ModesBody, TerminalBodyError> {
    let body = expect_body(frame, TerminalKind::Modes, MODES_BODY_BYTES)?;
    Ok(ModesBody {
        mode_bits: u32::from_le_bytes([body[0], body[1], body[2], body[3]]),
        rows: u16::from_le_bytes([body[4], body[5]]),
        cols: u16::from_le_bytes([body[6], body[7]]),
    })
}

/// Decode an `ATTACH_STATE` body.
pub fn decode_attach_state(frame: &TerminalFrame) -> Result<AttachStateCode, TerminalBodyError> {
    let body = expect_body(frame, TerminalKind::AttachState, ATTACH_STATE_BODY_BYTES)?;
    AttachStateCode::from_byte(body[0]).ok_or(TerminalBodyError::UnknownValue {
        field: "state",
        found: body[0],
    })
}

/// Decode a `HISTORY_UNAVAILABLE` body.
pub fn decode_history_unavailable(
    frame: &TerminalFrame,
) -> Result<HistoryUnavailableReason, TerminalBodyError> {
    let body = expect_body(
        frame,
        TerminalKind::HistoryUnavailable,
        HISTORY_UNAVAILABLE_BODY_BYTES,
    )?;
    HistoryUnavailableReason::from_byte(body[0]).ok_or(TerminalBodyError::UnknownValue {
        field: "reason",
        found: body[0],
    })
}

/// Decode an `INPUT_RESULT` body.
pub fn decode_input_result(frame: &TerminalFrame) -> Result<InputResultBody, TerminalBodyError> {
    expect_kind(frame, TerminalKind::InputResult)?;
    let body = frame.body();
    if body.len() < INPUT_RESULT_PREFIX_BYTES {
        return Err(TerminalBodyError::BodyLength {
            expected: INPUT_RESULT_PREFIX_BYTES,
            actual: body.len(),
        });
    }
    let operation_id = u64_le(&body[0..8]);
    let outcome = InputOutcome::from_byte(body[8]).ok_or(TerminalBodyError::UnknownValue {
        field: "outcome",
        found: body[8],
    })?;
    let accepted_payload_bytes = optional_u64(body[9], &body[10..18], "has_accepted")?;
    let written_pty_bytes = optional_u64(body[18], &body[19..27], "has_written")?;
    let mode_bits = u32::from_le_bytes([body[27], body[28], body[29], body[30]]);
    let detail_len = u16::from_le_bytes([body[31], body[32]]) as usize;
    let detail_bytes = &body[INPUT_RESULT_PREFIX_BYTES..];
    if detail_len != detail_bytes.len() || detail_len > MAX_INPUT_RESULT_DETAIL_BYTES {
        return Err(TerminalBodyError::InvalidDetail);
    }
    let detail = std::str::from_utf8(detail_bytes)
        .map_err(|_| TerminalBodyError::InvalidDetail)?
        .to_owned();
    Ok(InputResultBody {
        operation_id,
        outcome,
        accepted_payload_bytes,
        written_pty_bytes,
        mode_bits,
        detail,
    })
}

fn optional_u64(
    flag: u8,
    bytes: &[u8],
    field: &'static str,
) -> Result<Option<u64>, TerminalBodyError> {
    match flag {
        0 => Ok(None),
        1 => Ok(Some(u64_le(bytes))),
        found => Err(TerminalBodyError::UnknownValue { field, found }),
    }
}

fn u64_le(bytes: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[..8]);
    u64::from_le_bytes(buf)
}

fn expect_kind(frame: &TerminalFrame, expected: TerminalKind) -> Result<(), TerminalBodyError> {
    if frame.kind() != expected {
        return Err(TerminalBodyError::WrongKind {
            expected,
            actual: frame.kind(),
        });
    }
    Ok(())
}

fn expect_body(
    frame: &TerminalFrame,
    expected: TerminalKind,
    len: usize,
) -> Result<&[u8], TerminalBodyError> {
    expect_kind(frame, expected)?;
    let body = frame.body();
    if body.len() != len {
        return Err(TerminalBodyError::BodyLength {
            expected: len,
            actual: body.len(),
        });
    }
    Ok(body)
}
