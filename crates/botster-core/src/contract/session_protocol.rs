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
pub const PROTOCOL_VERSION: u8 = 2;

/// Daemon endpoint to session hello magic.
pub const HELLO_MAGIC: &[u8; 4] = b"SPH1";

/// Session to daemon endpoint welcome magic.
pub const WELCOME_MAGIC: &[u8; 4] = b"SPA1";

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
/// Daemon data plane to session: mode-gated PTY input request (correlated RPC).
pub const FRAME_MODE_GATED_PTY_INPUT: u8 = 0x19;
/// Session to daemon data plane: mode-gated PTY input result (correlated RPC).
pub const FRAME_MODE_GATED_PTY_INPUT_RESULT: u8 = 0x1a;
/// Data-plane peer to session: cancel one in-flight mode-gated request.
pub const FRAME_MODE_GATED_CANCEL: u8 = 0x1b;
/// Session to daemon data plane: the worker applied one resize command.
pub const FRAME_RESIZE_APPLIED: u8 = 0x1c;

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
}

fn bool_is_false(value: &bool) -> bool {
    !*value
}

/// Public freshness token for race-free mode-dependent input admission.
///
/// `mode_generation` is a high-entropy epoch for the current worker mode owner.
/// `mode_revision` counts complete [`ModeFlags`] changes within that epoch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModeFreshnessToken {
    /// Worker/session mode-owner epoch. Changes only on new worker ownership.
    pub mode_generation: u64,
    /// Monotonic complete-`ModeFlags` counter within [`Self::mode_generation`].
    pub mode_revision: u64,
}

/// Correlated mode-gated PTY input request payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeGatedPtyInputRequest {
    /// Parent-issued correlation id for this gated admit attempt.
    pub request_id: String,
    /// Expected complete-mode freshness token from the last successful probe.
    pub expected: ModeFreshnessToken,
    /// Candidate input bytes. Written only when the worker admits the request.
    #[serde(with = "mode_gated_bytes")]
    pub data: Vec<u8>,
    /// Parent wall-clock deadline (Unix epoch milliseconds). Worker must not
    /// write input after this instant even if the token still matches.
    pub deadline_unix_ms: u64,
    /// Optional deterministic hold before the final pre-write drain (tests only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_hold_ms: Option<u64>,
}

/// Correlated mode-gated PTY input result payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeGatedPtyInputResult {
    /// Echo of the request correlation id.
    pub request_id: String,
    /// Whether the worker wrote **all** input bytes to the PTY.
    ///
    /// Clean reject (stale token / deadline before any write): `admitted=false`
    /// and [`Self::bytes_written`] is `0`. Complete success: `admitted=true`
    /// and `bytes_written` equals the request payload length. Partial delivery
    /// uses `admitted=false`, `error_kind=Some("partial_write")`, and a nonzero
    /// `bytes_written` so callers never treat a prefix as a clean reject.
    pub admitted: bool,
    /// Number of request payload bytes actually written to the PTY.
    #[serde(default)]
    pub bytes_written: usize,
    /// Current complete mode flags after the pre-barrier apply.
    pub mode_flags: ModeFlags,
    /// Current mode freshness token after the pre-barrier apply.
    pub mode_freshness: ModeFreshnessToken,
    /// Optional protocol/runtime failure kind (malformed request, overflow, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

/// Cancel one in-flight mode-gated request by exact id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeGatedCancelRequest {
    /// Request id to cancel.
    pub request_id: String,
}

/// Worker mode-flags response payload, including the public freshness token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeFlagsPayload {
    /// Echo of the probe correlation id.
    pub request_id: String,
    /// Current complete mode flags.
    pub mode_flags: ModeFlags,
    /// Current mode freshness token for mode-dependent input.
    pub mode_freshness: ModeFreshnessToken,
    /// Optional probe failure kind. When set, modes are not authoritative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

mod mode_gated_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64_encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        base64_decode(&encoded).map_err(serde::de::Error::custom)
    }

    fn base64_encode(bytes: &[u8]) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        STANDARD.encode(bytes)
    }

    fn base64_decode(encoded: &str) -> Result<Vec<u8>, String> {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        STANDARD
            .decode(encoded)
            .map_err(|error| format!("invalid mode-gated input bytes encoding: {error}"))
    }
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
#[derive(Debug, Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
    discarded_headers: u32,
}

impl FrameDecoder {
    /// Create a new frame decoder.
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(8192),
            discarded_headers: 0,
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
            if len > MAX_FRAME_LEN {
                return Err(ProtocolError::FrameLengthTooLarge {
                    len,
                    max: MAX_FRAME_LEN,
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
