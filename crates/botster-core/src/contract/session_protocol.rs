//! Transport-neutral session-process wire protocol contracts.
//!
//! This module defines the reusable protocol data shapes and byte framing used
//! between a Botster session process and its data-plane peer. It intentionally
//! stops at constants, payload contracts, length-prefixed frames, and
//! handshake bytes. Hub recovery, socket lifecycle, process supervision, PTY
//! parsing, and client routing remain outside `botster-core`.

use std::collections::HashMap;
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current session-process protocol version.
pub const PROTOCOL_VERSION: u8 = 3;

/// Daemon endpoint to session hello magic.
pub const HELLO_MAGIC: &[u8; 4] = b"SPH1";

/// Session to daemon endpoint welcome magic.
pub const WELCOME_MAGIC: &[u8; 4] = b"SPA1";

/// Session to daemon endpoint startup-failure magic.
pub const STARTUP_FAILURE_MAGIC: &[u8; 4] = b"SPF1";

/// Core-enforced maximum metadata JSON length for handshake payloads.
pub const MAX_METADATA_LEN: usize = 64 * 1024;

/// Maximum encoded frame body length, including the one-byte frame type.
pub const MAX_FRAME_LEN: usize = 128 * 1024 * 1024;

/// Consecutive caller-discarded headers before the stream is desynchronized.
pub const DESYNC_THRESHOLD: u32 = 100;

/// Daemon data plane to session: raw PTY input bytes.
pub const FRAME_PTY_INPUT: u8 = 0x01;
/// Session to daemon data plane: raw PTY output bytes.
pub const FRAME_PTY_OUTPUT: u8 = 0x02;
/// Data-plane peer to session: resize command.
pub const FRAME_RESIZE: u8 = 0x03;
/// Data-plane peer to session: arm tee log.
pub const FRAME_ARM_TEE: u8 = 0x04;
/// Data-plane peer to session: request an opaque terminal snapshot.
pub const FRAME_GET_SNAPSHOT: u8 = 0x05;
/// Session to daemon data plane: opaque terminal snapshot response.
pub const FRAME_SNAPSHOT: u8 = 0x06;
/// Session to daemon data plane: child process exited.
pub const FRAME_PROCESS_EXITED: u8 = 0x07;
/// Daemon data plane to session: keepalive ping.
pub const FRAME_PING: u8 = 0x08;
/// Session to daemon data plane: keepalive pong.
pub const FRAME_PONG: u8 = 0x09;
/// Data-plane peer to session: request clean shutdown.
pub const FRAME_SHUTDOWN: u8 = 0x0a;
/// Data-plane peer to session: set reconnect timeout.
pub const FRAME_SET_TIMEOUT: u8 = 0x0b;
/// Data-plane peer to session: request terminal mode flags.
pub const FRAME_GET_MODE_FLAGS: u8 = 0x0c;
/// Session to daemon data plane: terminal mode flags response.
pub const FRAME_MODE_FLAGS: u8 = 0x0d;
/// Data-plane peer to session: request plain text screen contents.
pub const FRAME_GET_SCREEN: u8 = 0x0e;
/// Session to daemon data plane: plain text screen response.
pub const FRAME_SCREEN: u8 = 0x0f;
/// Session to daemon data plane: window title changed.
pub const FRAME_TITLE_CHANGED: u8 = 0x10;
/// Session to daemon data plane: bell character received.
pub const FRAME_BELL: u8 = 0x11;
// 0x12 was the legacy pushed terminal mode-change frame. Terminal mode changes
// are no longer public wire events in this core extraction slice.
/// Session to daemon data plane: working directory changed.
pub const FRAME_CWD_CHANGED: u8 = 0x13;
/// Session to daemon data plane: semantic prompt action detected.
pub const FRAME_PROMPT_MARK: u8 = 0x14;
/// Session to daemon data plane: OSC notification detected.
pub const FRAME_NOTIFICATION: u8 = 0x15;
/// Data-plane peer to session: replace terminal color profile.
pub const FRAME_SET_COLOR_PROFILE: u8 = 0x16;
/// Data-plane peer to session process: initial spawn request.
pub const FRAME_SPAWN_SESSION: u8 = 0x17;
/// Session to daemon data plane: payload-free terminal metadata shaping report.
pub const FRAME_METADATA_SHAPING: u8 = 0x18;
// 0x19, 0x1a, and 0x1b were the mode-gated PTY input family. They are deleted
// with scheme 1 and must not be reused.
/// Session to daemon data plane: the worker applied one resize command.
pub const FRAME_RESIZE_APPLIED: u8 = 0x1c;
/// Parent to worker: one input operation.
///
/// Payload: `[u64 LE worker_operation_key][u8 kind][u64 LE operation_id]`
/// `[u32 LE body_len][body]` (see [`encode_worker_input_operation`]). The key
/// is unique per parent process and maps in Core to client, route, route
/// generation, and client operation id. The worker never interprets the key.
pub const FRAME_INPUT_OPERATION: u8 = 0x1d;
/// Worker to parent: exactly one result per input operation.
///
/// Payload: `[u64 LE worker_operation_key][INPUT_RESULT body]` where the body
/// is the scheme 2 kind 17 layout with the client operation id echoed.
pub const FRAME_INPUT_RESULT: u8 = 0x1e;
/// Parent to worker: abandon the unwritten remainder of one operation.
///
/// Payload: `[u64 LE worker_operation_key]`. The worker reports `Cancelled`
/// with the bytes written so far, or ignores an unknown or finished key.
pub const FRAME_INPUT_CANCEL: u8 = 0x1f;
/// Worker to parent: terminal modes or geometry changed (spontaneous).
///
/// Payload: the scheme 2 `MODES` body, `[u32 LE mode_bits][u16 LE rows][u16 LE cols]`.
pub const FRAME_MODES_CHANGED: u8 = 0x20;
/// Worker to parent: final terminal state after the last PTY output and
/// before `FRAME_PROCESS_EXITED`.
///
/// Payload: `[u32 LE header_len][WorkerFinalState JSON][raw GHOSTSNP bytes]`.
/// The snapshot is present when the header reports `has_snapshot`.
pub const FRAME_FINAL_STATE: u8 = 0x21;
/// Parent to worker: request the cursor position and its row's text.
pub const FRAME_GET_CURSOR: u8 = 0x22;
/// Worker to parent: cursor read response, one Ghostty model read.
pub const FRAME_CURSOR: u8 = 0x23;

/// Length of the worker operation key prefix on input frames.
pub const WORKER_OPERATION_KEY_BYTES: usize = 8;

/// Fixed prefix of a `FRAME_INPUT_OPERATION` payload after the worker key:
/// `[u8 kind][u64 LE operation_id][u32 LE body_len]`.
pub const WORKER_INPUT_OPERATION_PREFIX_BYTES: usize = 1 + 8 + 4;

/// Kind of one parent-to-worker input operation.
///
/// Values equal the scheme 2 `TerminalInputKind` bytes for the single-frame
/// kinds. `Paste` reuses the `PASTE_BEGIN` value and carries the complete
/// assembled paste, which can exceed one client frame body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum WorkerInputKind {
    /// Raw PTY bytes. Body is the bytes.
    RawBytes = 1,
    /// Key event. Body is the scheme 2 `KEY` body.
    Key = 2,
    /// Mouse event. Body is the scheme 2 `MOUSE` body.
    Mouse = 3,
    /// Focus event. Body is the scheme 2 `FOCUS` body.
    Focus = 4,
    /// Resize. Body is the scheme 2 `RESIZE` body.
    Resize = 5,
    /// Complete paste. Body is `[u8 allow_unsafe][paste bytes]`.
    Paste = 6,
}

impl WorkerInputKind {
    /// Decode a kind byte.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            1 => Self::RawBytes,
            2 => Self::Key,
            3 => Self::Mouse,
            4 => Self::Focus,
            5 => Self::Resize,
            6 => Self::Paste,
            _ => return None,
        })
    }
}

/// One decoded parent-to-worker input operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerInputOperation<'a> {
    /// Parent-unique operation key.
    pub key: u64,
    /// Client operation id echoed in the result.
    pub operation_id: u64,
    /// Operation kind.
    pub kind: WorkerInputKind,
    /// Kind-specific body.
    pub body: &'a [u8],
}

/// Encode a `FRAME_INPUT_OPERATION` payload.
///
/// Layout: `[u64 LE key][u8 kind][u64 LE operation_id][u32 LE body_len][body]`.
pub fn encode_worker_input_operation(
    key: u64,
    operation_id: u64,
    kind: WorkerInputKind,
    body: &[u8],
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(
        WORKER_OPERATION_KEY_BYTES + WORKER_INPUT_OPERATION_PREFIX_BYTES + body.len(),
    );
    payload.extend_from_slice(&key.to_le_bytes());
    payload.push(kind as u8);
    payload.extend_from_slice(&operation_id.to_le_bytes());
    payload.extend_from_slice(&(body.len() as u32).to_le_bytes());
    payload.extend_from_slice(body);
    payload
}

/// Decode a `FRAME_INPUT_OPERATION` payload.
pub fn decode_worker_input_operation(
    payload: &[u8],
) -> Result<WorkerInputOperation<'_>, ProtocolError> {
    let (key, rest) = split_worker_operation_key(payload)?;
    if rest.len() < WORKER_INPUT_OPERATION_PREFIX_BYTES {
        return Err(ProtocolError::FrameLengthZero);
    }
    let kind = WorkerInputKind::from_byte(rest[0]).ok_or(ProtocolError::FrameLengthZero)?;
    let mut id = [0u8; 8];
    id.copy_from_slice(&rest[1..9]);
    let body_len = u32::from_le_bytes([rest[9], rest[10], rest[11], rest[12]]) as usize;
    let body = &rest[WORKER_INPUT_OPERATION_PREFIX_BYTES..];
    if body.len() != body_len {
        return Err(ProtocolError::FrameLengthTooLarge {
            len: body.len(),
            max: body_len,
        });
    }
    Ok(WorkerInputOperation {
        key,
        operation_id: u64::from_le_bytes(id),
        kind,
        body,
    })
}

/// Worker plain-text screen reply for the correlated `FRAME_GET_SCREEN` RPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenPayload {
    /// Echo of the probe correlation id.
    pub request_id: String,
    /// Plain text of the visible screen.
    pub text: String,
    /// Optional read failure. When set, `text` is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

/// The cursor and its row, read from one Ghostty model state.
///
/// Coordinates are 0-based in the active area, which is the visible screen:
/// Core never scrolls a session's view back. `cursor_col` is a cell column.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorRow {
    /// Cursor row.
    pub row: u16,
    /// Cursor column, in cells.
    pub col: u16,
    /// Whether the cursor is shown.
    pub cursor_visible: bool,
    /// Plain text of the cursor row, trailing blanks trimmed.
    pub row_text: String,
    /// Plain text of the cells strictly left of the cursor on its row,
    /// untrimmed: blanks before the cursor are kept, and each wide character
    /// appears once.
    pub text_before_cursor: String,
}

/// Worker cursor reply for the correlated `FRAME_GET_CURSOR` RPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorPayload {
    /// Echo of the probe correlation id.
    pub request_id: String,
    /// The cursor read. Not authoritative when `error_kind` is set.
    pub cursor: CursorRow,
    /// Optional read failure kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

/// Correlated worker probe request carrying only a request id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerProbeRequest {
    /// Parent-issued correlation id.
    pub request_id: String,
}

/// Typed worker startup failure occupying the welcome slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupFailureReport {
    /// Spawn request identity echoed from the decoded FRAME_SPAWN_SESSION.
    pub request_id: String,
    /// Session identity echoed from the decoded FRAME_SPAWN_SESSION.
    pub session_id: String,
    /// Worker process id that produced this report.
    pub worker_pid: u32,
    /// Diagnostic text. Classification never parses this field.
    pub message: String,
    /// Structural creation evidence.
    pub outcome: StartupFailureOutcome,
}

/// Whether the worker created a session child.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StartupFailureOutcome {
    /// The worker local reservation was still launching. No child was created.
    NotCreated,
    /// A child may have existed. This is not a cleanup receipt.
    Created {
        /// Direct child pid when the worker observed one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        child_pid: Option<u32>,
        /// Positive process group greater than 1, when captured.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        process_group_id: Option<i32>,
    },
}

/// Handshake reply occupying the welcome slot.
#[derive(Debug, Clone, PartialEq)]
pub enum StartupReply {
    /// Ordinary welcome metadata.
    Welcome {
        /// Negotiated protocol version.
        version: u8,
        /// Worker session metadata.
        metadata: SessionMetadata,
    },
    /// Typed startup failure.
    StartupFailure(StartupFailureReport),
}

/// Final worker-owned terminal state retained by the parent after exit.
///
/// The GHOSTSNP bytes travel after this header, never inside JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerFinalState {
    /// Plain text of the visible screen at exit.
    pub screen_text: String,
    /// Whether raw GHOSTSNP bytes follow the header.
    pub has_snapshot: bool,
    /// Scheme 2 mode bits at exit.
    pub mode_bits: u32,
    /// Terminal rows at exit.
    pub rows: u16,
    /// Terminal columns at exit.
    pub cols: u16,
    /// Ghostty palette and special colors at exit.
    pub color_profile: TerminalColorProfile,
    /// Export failure detail when `has_snapshot` is false because export failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Encode a `FRAME_FINAL_STATE` payload: JSON header then raw snapshot bytes.
pub fn encode_final_state(
    state: &WorkerFinalState,
    snapshot: Option<&[u8]>,
) -> Result<Vec<u8>, ProtocolError> {
    let header = serde_json::to_vec(state)?;
    let snapshot = snapshot.unwrap_or(&[]);
    let mut payload = Vec::with_capacity(4 + header.len() + snapshot.len());
    payload.extend_from_slice(&(header.len() as u32).to_le_bytes());
    payload.extend_from_slice(&header);
    payload.extend_from_slice(snapshot);
    Ok(payload)
}

/// Decode a `FRAME_FINAL_STATE` payload into its header and trailing snapshot.
pub fn decode_final_state(payload: &[u8]) -> Result<(WorkerFinalState, &[u8]), ProtocolError> {
    if payload.len() < 4 {
        return Err(ProtocolError::FrameLengthZero);
    }
    let header_len = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize;
    let header_end = 4 + header_len;
    if payload.len() < header_end {
        return Err(ProtocolError::FrameLengthTooLarge {
            len: header_end,
            max: payload.len(),
        });
    }
    let state: WorkerFinalState = serde_json::from_slice(&payload[4..header_end])?;
    Ok((state, &payload[header_end..]))
}

/// Split a `[u64 LE worker_operation_key][rest]` payload.
pub fn split_worker_operation_key(payload: &[u8]) -> Result<(u64, &[u8]), ProtocolError> {
    if payload.len() < WORKER_OPERATION_KEY_BYTES {
        return Err(ProtocolError::FrameLengthZero);
    }
    let mut key = [0u8; WORKER_OPERATION_KEY_BYTES];
    key.copy_from_slice(&payload[..WORKER_OPERATION_KEY_BYTES]);
    Ok((
        u64::from_le_bytes(key),
        &payload[WORKER_OPERATION_KEY_BYTES..],
    ))
}

/// Prefix `rest` with a `u64 LE` worker operation key.
pub fn encode_worker_operation(key: u64, rest: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(WORKER_OPERATION_KEY_BYTES + rest.len());
    payload.extend_from_slice(&key.to_le_bytes());
    payload.extend_from_slice(rest);
    payload
}

/// Correlated request for an atomic worker-owned terminal snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSnapshotRequest {
    /// Parent-issued correlation id.
    pub request_id: String,
    /// Cancel the matching in-progress snapshot encode.
    #[serde(default)]
    pub cancel: bool,
    /// Complete the matching snapshot barrier after any staged resize.
    #[serde(default, skip_serializing_if = "bool_is_false")]
    pub complete: bool,
}

/// Record-aware boundary for one opaque incremental snapshot frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerSnapshotPhase {
    /// Snapshot prefix through READY.
    Ready,
    /// HISTORY records through one PAGE.
    History,
    /// Remaining zero-page HISTORY records through FINISH.
    Finish,
}

/// Correlated worker-owned terminal snapshot response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSnapshotResult {
    /// Echo of the request correlation id.
    pub request_id: String,
    /// Opaque snapshot frame captured after pre-boundary PTY output was applied.
    pub snapshot: Option<crate::TerminalSnapshotPayload>,
    /// Record boundary identified only by the Ghostty authority worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<WorkerSnapshotPhase>,
    /// Worker snapshot failure. The worker remains live when this field is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    /// The worker applied the staged resize and released the PTY barrier.
    #[serde(default, skip_serializing_if = "bool_is_false")]
    pub barrier_released: bool,
    /// Ghostty colors frozen with the snapshot. Present on the FINISH frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_profile: Option<crate::TerminalColorProfile>,
}

fn bool_is_false(value: &bool) -> bool {
    !*value
}

/// Worker mode-flags response payload for the correlated `FRAME_GET_MODE_FLAGS` RPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeFlagsPayload {
    /// Echo of the probe correlation id.
    pub request_id: String,
    /// Current complete mode flags.
    pub mode_flags: ModeFlags,
    /// Terminal rows at the time of the reply.
    #[serde(default)]
    pub rows: u16,
    /// Terminal columns at the time of the reply.
    #[serde(default)]
    pub cols: u16,
    /// Optional probe failure kind. When set, modes are not authoritative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

/// Session metadata sent in the welcome handshake.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier.
    pub session_uuid: String,
    /// PID of the session process.
    pub pid: u32,
    /// Current PTY row count.
    pub rows: u16,
    /// Current PTY column count.
    pub cols: u16,
    /// Unix timestamp of last PTY output.
    pub last_output_at: u64,
    /// Current terminal title from OSC 0/2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Current terminal working directory from OSC 7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Optional HTTP forwarding port assigned to the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Terminal mode flags at handshake time.
    #[serde(default)]
    pub mode_flags: ModeFlags,
    /// Immutable recovery identity captured when the session process was born.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_identity: Option<serde_json::Value>,
}

/// Terminal mode flags reported by a session process.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeFlags {
    /// Kitty keyboard protocol enabled.
    pub kitty_enabled: bool,
    /// Cursor is visible.
    pub cursor_visible: bool,
    /// Bracketed paste mode enabled.
    pub bracketed_paste: bool,
    /// Mouse tracking mode bitmask.
    pub mouse_mode: u8,
    /// Alternate screen buffer active.
    pub alt_screen: bool,
    /// Focus reporting mode enabled.
    #[serde(default)]
    pub focus_reporting: bool,
    /// Application cursor keys mode enabled.
    #[serde(default)]
    pub application_cursor: bool,
}

impl ModeFlags {
    /// Collapse to the scheme 2 `MODES` bit set.
    ///
    /// `mouse_mode` bits are the existing worker mask: 1 normal (1000),
    /// 2 any (1003), 4 button (1002), 8 SGR (1006).
    #[must_use]
    pub const fn to_mode_bits(&self) -> u32 {
        use botster_terminal_protocol::mode_bits;
        let mut bits = 0;
        if self.kitty_enabled {
            bits |= mode_bits::KITTY_KEYBOARD;
        }
        if self.cursor_visible {
            bits |= mode_bits::CURSOR_VISIBLE;
        }
        if self.bracketed_paste {
            bits |= mode_bits::BRACKETED_PASTE;
        }
        if self.mouse_mode & 1 != 0 {
            bits |= mode_bits::MOUSE_NORMAL;
        }
        if self.mouse_mode & 2 != 0 {
            bits |= mode_bits::MOUSE_ANY;
        }
        if self.mouse_mode & 4 != 0 {
            bits |= mode_bits::MOUSE_BUTTON;
        }
        if self.mouse_mode & 8 != 0 {
            bits |= mode_bits::MOUSE_SGR;
        }
        if self.alt_screen {
            bits |= mode_bits::ALT_SCREEN;
        }
        if self.focus_reporting {
            bits |= mode_bits::FOCUS_REPORTING;
        }
        if self.application_cursor {
            bits |= mode_bits::APPLICATION_CURSOR;
        }
        bits
    }

    /// Expand a scheme 2 `MODES` bit set.
    #[must_use]
    pub const fn from_mode_bits(bits: u32) -> Self {
        use botster_terminal_protocol::mode_bits;
        let mut mouse_mode = 0;
        if bits & mode_bits::MOUSE_NORMAL != 0 {
            mouse_mode |= 1;
        }
        if bits & mode_bits::MOUSE_ANY != 0 {
            mouse_mode |= 2;
        }
        if bits & mode_bits::MOUSE_BUTTON != 0 {
            mouse_mode |= 4;
        }
        if bits & mode_bits::MOUSE_SGR != 0 {
            mouse_mode |= 8;
        }
        Self {
            kitty_enabled: bits & mode_bits::KITTY_KEYBOARD != 0,
            cursor_visible: bits & mode_bits::CURSOR_VISIBLE != 0,
            bracketed_paste: bits & mode_bits::BRACKETED_PASTE != 0,
            mouse_mode,
            alt_screen: bits & mode_bits::ALT_SCREEN != 0,
            focus_reporting: bits & mode_bits::FOCUS_REPORTING != 0,
            application_cursor: bits & mode_bits::APPLICATION_CURSOR != 0,
        }
    }
}

/// OSC notification payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationPayload {
    /// Notification title.
    pub title: String,
    /// Notification body text.
    pub body: String,
}

/// Semantic prompt payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptMarkPayload {
    /// Prompt mark/action name.
    pub mark: String,
}

/// RGB color value used by core protocol payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
}

/// Full terminal color profile pushed into a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalColorProfile {
    /// Colors keyed by terminal color index.
    #[serde(default)]
    pub colors: HashMap<u16, Rgb>,
}

/// Child process exit payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessExitedPayload {
    /// Child exit code, when the process exited normally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Terminating signal, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i32>,
}

/// Resize command payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizePayload {
    /// Target row count.
    pub rows: u16,
    /// Target column count.
    pub cols: u16,
}

/// Tee-log command payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeePayload {
    /// Destination log path.
    pub log_path: String,
    /// Maximum bytes to retain.
    pub cap_bytes: u64,
}

/// Reconnect timeout command payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutPayload {
    /// Timeout in seconds.
    pub seconds: u64,
}

/// A decoded session-process frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Wire frame type byte.
    pub frame_type: u8,
    /// Raw frame payload bytes.
    pub payload: Vec<u8>,
}

impl Frame {
    /// Parse this frame payload as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, ProtocolError> {
        serde_json::from_slice(&self.payload).map_err(ProtocolError::Json)
    }
}

/// Session-process protocol error.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// Encoded frame body would exceed the protocol frame cap.
    #[error("frame body too large: {len} bytes exceeds {max} bytes")]
    FrameEncodeTooLarge {
        /// Requested body length.
        len: usize,
        /// Maximum body length.
        max: usize,
    },
    /// Frame length headers must include the one-byte frame type.
    #[error("frame length header was zero")]
    FrameLengthZero,
    /// Frame length header exceeded the protocol frame cap.
    #[error("frame body too large: {len} bytes exceeds {max} bytes")]
    FrameLengthTooLarge {
        /// Header body length.
        len: usize,
        /// Maximum body length.
        max: usize,
    },
    /// Caller-discarded headers crossed the desync threshold.
    #[error("stream desynchronized after {bad_headers} discarded headers")]
    Desynchronized {
        /// Consecutive discarded headers.
        bad_headers: u32,
        /// Desync threshold.
        threshold: u32,
    },
    /// Handshake metadata exceeded the core-enforced metadata cap.
    #[error("metadata too large: {len} bytes exceeds {max} bytes")]
    MetadataTooLarge {
        /// Metadata byte length.
        len: usize,
        /// Maximum metadata byte length.
        max: usize,
    },
    /// Handshake magic bytes did not match the expected side.
    #[error("bad {context} magic: expected {expected:?}, got {got:?}")]
    BadMagic {
        /// Handshake side being decoded.
        context: &'static str,
        /// Expected magic bytes.
        expected: [u8; 4],
        /// Received magic bytes.
        got: [u8; 4],
    },
    /// JSON serialization or parsing failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// IO failed while reading or writing protocol bytes.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Encode a frame as `[u32 LE: payload_len + 1][u8 frame_type][payload]`.
pub fn encode_frame(frame_type: u8, payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let len = payload.len() + 1;
    if len > MAX_FRAME_LEN {
        return Err(ProtocolError::FrameEncodeTooLarge {
            len,
            max: MAX_FRAME_LEN,
        });
    }

    let mut buf = Vec::with_capacity(4 + len);
    buf.extend_from_slice(&(len as u32).to_le_bytes());
    buf.push(frame_type);
    buf.extend_from_slice(payload);
    Ok(buf)
}

/// Encode a frame with no payload.
pub fn encode_empty(frame_type: u8) -> Result<Vec<u8>, ProtocolError> {
    encode_frame(frame_type, &[])
}

/// Encode a frame with a UTF-8 string payload.
pub fn encode_string(frame_type: u8, value: &str) -> Result<Vec<u8>, ProtocolError> {
    encode_frame(frame_type, value.as_bytes())
}

/// Encode a frame with a JSON payload.
pub fn encode_json<T: Serialize>(frame_type: u8, value: &T) -> Result<Vec<u8>, ProtocolError> {
    let payload = serde_json::to_vec(value)?;
    encode_frame(frame_type, &payload)
}

/// Incremental frame decoder.
#[derive(Debug)]
pub struct FrameDecoder {
    buf: Vec<u8>,
    discarded_headers: u32,
    max_len: usize,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder {
    /// Create a new frame decoder with the protocol frame cap.
    pub fn new() -> Self {
        Self::with_max_len(MAX_FRAME_LEN)
    }

    /// Create a decoder that refuses any frame whose length header (type byte
    /// plus payload) exceeds `max_len`, capped at [`MAX_FRAME_LEN`]. The
    /// buffer therefore never holds more than one header plus `max_len` bytes
    /// of an incomplete frame, plus the caller's current read.
    pub fn with_max_len(max_len: usize) -> Self {
        let max_len = max_len.min(MAX_FRAME_LEN);
        Self {
            buf: Vec::with_capacity(8192.min(4 + max_len)),
            discarded_headers: 0,
            max_len,
        }
    }

    /// Feed bytes into the decoder and return every complete frame available.
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<Frame>, ProtocolError> {
        self.buf.extend_from_slice(data);
        let mut frames = Vec::new();

        loop {
            if self.buf.len() < 4 {
                break;
            }

            let len =
                u32::from_le_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
            if len == 0 {
                return Err(ProtocolError::FrameLengthZero);
            }
            if len > self.max_len {
                return Err(ProtocolError::FrameLengthTooLarge {
                    len,
                    max: self.max_len,
                });
            }
            if self.buf.len() < 4 + len {
                break;
            }

            self.discarded_headers = 0;
            let frame_type = self.buf[4];
            let payload = self.buf[5..4 + len].to_vec();
            self.buf.drain(..4 + len);
            frames.push(Frame {
                frame_type,
                payload,
            });
        }

        Ok(frames)
    }

    /// Record one caller-discarded header while attempting stream resync.
    ///
    /// `feed()` fails malformed length headers explicitly. This method exists
    /// only for a higher-level reader that has already chosen to discard bytes
    /// as a recovery tactic; it does not encode hub recovery policy.
    pub fn record_discarded_header(&mut self) -> Result<(), ProtocolError> {
        self.discarded_headers += 1;
        if self.is_desynced() {
            return Err(ProtocolError::Desynchronized {
                bad_headers: self.discarded_headers,
                threshold: DESYNC_THRESHOLD,
            });
        }
        Ok(())
    }

    /// Whether caller-discarded headers reached the desync threshold.
    pub fn is_desynced(&self) -> bool {
        self.discarded_headers >= DESYNC_THRESHOLD
    }
}

/// Encode hub-side hello bytes.
pub fn encode_hello(version: u8) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    buf.extend_from_slice(HELLO_MAGIC);
    buf.push(version);
    buf
}

/// Decode hub-side hello bytes and return the peer protocol version.
pub fn decode_hello(bytes: &[u8]) -> Result<u8, ProtocolError> {
    let mut cursor = std::io::Cursor::new(bytes);
    read_hello(&mut cursor)
}

/// Encode session-side welcome bytes.
pub fn encode_welcome(version: u8, metadata: &SessionMetadata) -> Result<Vec<u8>, ProtocolError> {
    let metadata = serde_json::to_vec(metadata)?;
    if metadata.len() > MAX_METADATA_LEN {
        return Err(ProtocolError::MetadataTooLarge {
            len: metadata.len(),
            max: MAX_METADATA_LEN,
        });
    }

    let mut buf = Vec::with_capacity(9 + metadata.len());
    buf.extend_from_slice(WELCOME_MAGIC);
    buf.push(version);
    buf.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    buf.extend_from_slice(&metadata);
    Ok(buf)
}

/// Decode session-side welcome bytes and return peer version plus metadata.
pub fn decode_welcome(bytes: &[u8]) -> Result<(u8, SessionMetadata), ProtocolError> {
    let mut cursor = std::io::Cursor::new(bytes);
    read_welcome(&mut cursor)
}

/// Write hub-side hello bytes to a stream.
pub fn write_hello(stream: &mut impl Write) -> Result<(), ProtocolError> {
    stream.write_all(&encode_hello(PROTOCOL_VERSION))?;
    stream.flush()?;
    Ok(())
}

/// Read hub-side hello bytes from a stream.
pub fn read_hello(stream: &mut impl Read) -> Result<u8, ProtocolError> {
    let mut magic = [0u8; 4];
    stream.read_exact(&mut magic)?;
    if &magic != HELLO_MAGIC {
        return Err(ProtocolError::BadMagic {
            context: "hello",
            expected: *HELLO_MAGIC,
            got: magic,
        });
    }

    let mut version = [0u8; 1];
    stream.read_exact(&mut version)?;
    Ok(version[0])
}

/// Write session-side welcome bytes to a stream.
pub fn write_welcome(
    stream: &mut impl Write,
    metadata: &SessionMetadata,
) -> Result<(), ProtocolError> {
    let bytes = encode_welcome(PROTOCOL_VERSION, metadata)?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

/// Read session-side welcome bytes from a stream.
pub fn read_welcome(stream: &mut impl Read) -> Result<(u8, SessionMetadata), ProtocolError> {
    let mut magic = [0u8; 4];
    stream.read_exact(&mut magic)?;
    if &magic != WELCOME_MAGIC {
        return Err(ProtocolError::BadMagic {
            context: "welcome",
            expected: *WELCOME_MAGIC,
            got: magic,
        });
    }

    let mut version = [0u8; 1];
    stream.read_exact(&mut version)?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_METADATA_LEN {
        return Err(ProtocolError::MetadataTooLarge {
            len,
            max: MAX_METADATA_LEN,
        });
    }

    let mut json_buf = vec![0u8; len];
    stream.read_exact(&mut json_buf)?;
    let metadata = serde_json::from_slice(&json_buf)?;
    Ok((version[0], metadata))
}

/// Encode a startup-failure frame that fits [`MAX_METADATA_LEN`].
pub fn encode_startup_failure(
    version: u8,
    report: &StartupFailureReport,
) -> Result<Vec<u8>, ProtocolError> {
    let mut report = report.clone();
    loop {
        let json = serde_json::to_vec(&report)?;
        if json.len() <= MAX_METADATA_LEN {
            let mut buf = Vec::with_capacity(9 + json.len());
            buf.extend_from_slice(STARTUP_FAILURE_MAGIC);
            buf.push(version);
            buf.extend_from_slice(&(json.len() as u32).to_le_bytes());
            buf.extend_from_slice(&json);
            return Ok(buf);
        }
        if report.message.is_empty() {
            return Err(ProtocolError::MetadataTooLarge {
                len: json.len(),
                max: MAX_METADATA_LEN,
            });
        }
        let keep = report.message.len() / 2;
        report.message.truncate(keep);
    }
}

/// Write a startup-failure frame to a stream.
pub fn write_startup_failure(
    stream: &mut impl Write,
    report: &StartupFailureReport,
) -> Result<(), ProtocolError> {
    let bytes = encode_startup_failure(PROTOCOL_VERSION, report)?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

fn read_capped_json(stream: &mut impl Read) -> Result<Vec<u8>, ProtocolError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_METADATA_LEN {
        return Err(ProtocolError::MetadataTooLarge {
            len,
            max: MAX_METADATA_LEN,
        });
    }
    let mut json_buf = vec![0u8; len];
    stream.read_exact(&mut json_buf)?;
    Ok(json_buf)
}

/// Read a welcome or SPF1 startup-failure occupying the welcome slot.
pub fn read_startup_reply(stream: &mut impl Read) -> Result<StartupReply, ProtocolError> {
    let mut magic = [0u8; 4];
    stream.read_exact(&mut magic)?;
    let mut version = [0u8; 1];
    stream.read_exact(&mut version)?;
    let json_buf = read_capped_json(stream)?;
    if &magic == WELCOME_MAGIC {
        let metadata = serde_json::from_slice(&json_buf)?;
        Ok(StartupReply::Welcome {
            version: version[0],
            metadata,
        })
    } else if &magic == STARTUP_FAILURE_MAGIC {
        let report = serde_json::from_slice(&json_buf)?;
        Ok(StartupReply::StartupFailure(report))
    } else {
        Err(ProtocolError::BadMagic {
            context: "welcome",
            expected: *WELCOME_MAGIC,
            got: magic,
        })
    }
}
