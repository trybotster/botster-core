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
/// Session to daemon data plane: terminal mode changed.
pub const FRAME_MODE_CHANGED: u8 = 0x12;
/// Session to daemon data plane: working directory changed.
pub const FRAME_CWD_CHANGED: u8 = 0x13;
/// Session to daemon data plane: semantic prompt action detected.
pub const FRAME_PROMPT_MARK: u8 = 0x14;
/// Session to daemon data plane: OSC notification detected.
pub const FRAME_NOTIFICATION: u8 = 0x15;
/// Data-plane peer to session: replace terminal color profile.
pub const FRAME_SET_COLOR_PROFILE: u8 = 0x16;

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

/// Incremental terminal mode change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeChanged {
    /// Kitty keyboard protocol toggled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kitty_enabled: Option<bool>,
    /// Cursor visibility changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_visible: Option<bool>,
    /// Bracketed paste mode toggled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bracketed_paste: Option<bool>,
    /// Mouse tracking mode bitmask changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_mode: Option<u8>,
    /// Alternate screen buffer toggled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt_screen: Option<bool>,
    /// Focus reporting mode toggled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_reporting: Option<bool>,
    /// Application cursor keys mode toggled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_cursor: Option<bool>,
}

/// Build a full replay mode-change payload from current mode flags.
pub fn mode_changed_from_flags(flags: ModeFlags) -> ModeChanged {
    ModeChanged {
        kitty_enabled: Some(flags.kitty_enabled),
        cursor_visible: Some(flags.cursor_visible),
        bracketed_paste: Some(flags.bracketed_paste),
        mouse_mode: Some(flags.mouse_mode),
        alt_screen: Some(flags.alt_screen),
        focus_reporting: Some(flags.focus_reporting),
        application_cursor: Some(flags.application_cursor),
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
