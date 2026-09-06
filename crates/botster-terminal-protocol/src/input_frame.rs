//! Terminal input scheme 2: opaque compact-binary input frames.
//!
//! Hub validates the 12-byte header and forwards the bytes. It must not decode
//! the body. Semantic encode and decode live in `botster-terminal-protocol-client`.
//!
//! ```text
//! InputFrame = [u8 scheme = 2][u8 kind][u16 BE body_len][u64 BE operation_id][body]
//! ```

/// Scheme version. Byte 0 uses exact equality.
pub const TERMINAL_INPUT_SCHEME_VERSION: u8 = 2;
/// Fixed header: scheme, kind, body length, operation id.
pub const INPUT_HEADER_BYTES: usize = 12;
/// `u16` body ceiling.
pub const MAX_TERMINAL_INPUT_BODY_BYTES: u16 = 65_535;
/// Header plus the body ceiling.
pub const MAX_TERMINAL_INPUT_FRAME_BYTES: usize =
    INPUT_HEADER_BYTES + MAX_TERMINAL_INPUT_BODY_BYTES as usize;
/// `RAW_BYTES` payload ceiling. The body is payload only.
pub const MAX_RAW_INPUT_BYTES: usize = MAX_TERMINAL_INPUT_BODY_BYTES as usize;
/// Fixed `KEY` prefix before `utf8`:
/// `[u8 action][u16 key][u16 mods][u16 consumed_mods][u8 composing][u32 unshifted_codepoint]`.
pub const KEY_PREFIX_BYTES: usize = 12;
/// Exact `MOUSE` body:
/// `[u8 action][u8 has_button][u8 button][u16 mods][u16 col][u16 row][u32 x_px][u32 y_px]`.
pub const MOUSE_BODY_BYTES: usize = 17;
/// Exact `FOCUS` body: `[u8 focused]`.
pub const FOCUS_BODY_BYTES: usize = 1;
/// Exact `RESIZE` body: `[u16 rows][u16 cols][u32 width_px][u32 height_px]`.
pub const RESIZE_BODY_BYTES: usize = 12;
/// Exact `PASTE_BEGIN` body: `[u32 total_len][u8 allow_unsafe]`.
pub const PASTE_BEGIN_BODY_BYTES: usize = 5;
/// `PASTE_CHUNK` prefix: `[u32 index]`.
pub const PASTE_CHUNK_PREFIX_BYTES: usize = 4;
/// `PASTE_CHUNK` data ceiling after the index.
pub const MAX_PASTE_CHUNK_DATA_BYTES: usize =
    MAX_TERMINAL_INPUT_BODY_BYTES as usize - PASTE_CHUNK_PREFIX_BYTES;
/// Exact `PASTE_COMMIT` body.
pub const PASTE_COMMIT_BODY_BYTES: usize = 0;
/// Exact `PASTE_ABORT` body.
pub const PASTE_ABORT_BODY_BYTES: usize = 0;
/// Maximum complete paste content for one operation.
pub const MAX_PASTE_BYTES: usize = 1_048_576;
/// Maximum chunk count for one complete paste.
pub const MAX_PASTE_CHUNKS: usize = MAX_PASTE_BYTES.div_ceil(MAX_PASTE_CHUNK_DATA_BYTES);
/// Outstanding input operations per session.
pub const MAX_INPUT_OPERATIONS_PER_SESSION: usize = 32;
/// Retained input payload bytes per session, across every route.
pub const MAX_RETAINED_INPUT_BYTES_PER_SESSION: usize = 2 * MAX_PASTE_BYTES;
/// Pastes assembling at one time on one route.
pub const MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION: usize = 1;
/// Outstanding input operations per client, across every session.
pub const MAX_INPUT_OPERATIONS_PER_CLIENT: usize = 128;
/// Retained input payload bytes per client.
pub const MAX_RETAINED_INPUT_BYTES_PER_CLIENT: usize = 8 * MAX_PASTE_BYTES;
/// Encoded PTY bytes for one operation, including encoder-added markers.
pub const MAX_ENCODED_INPUT_BYTES: usize = MAX_PASTE_BYTES + 64;

/// Input kind at byte offset 1. Values are frozen wire bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TerminalInputKind {
    /// Explicit raw PTY bytes. Never mode-encoded.
    RawBytes = 1,
    /// Physical key event for the worker key encoder.
    Key = 2,
    /// Mouse event with cell and pixel position.
    Mouse = 3,
    /// Focus gained or lost.
    Focus = 4,
    /// Cell and pixel geometry.
    Resize = 5,
    /// Start one bounded paste.
    PasteBegin = 6,
    /// One ordered paste chunk.
    PasteChunk = 7,
    /// Commit the assembled paste.
    PasteCommit = 8,
    /// Abort the assembling paste.
    PasteAbort = 9,
}

impl TerminalInputKind {
    /// Every published kind, in wire-value order.
    pub const ALL: &'static [Self] = &[
        Self::RawBytes,
        Self::Key,
        Self::Mouse,
        Self::Focus,
        Self::Resize,
        Self::PasteBegin,
        Self::PasteChunk,
        Self::PasteCommit,
        Self::PasteAbort,
    ];

    /// Decode a kind byte.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        Some(match byte {
            1 => Self::RawBytes,
            2 => Self::Key,
            3 => Self::Mouse,
            4 => Self::Focus,
            5 => Self::Resize,
            6 => Self::PasteBegin,
            7 => Self::PasteChunk,
            8 => Self::PasteCommit,
            9 => Self::PasteAbort,
            _ => return None,
        })
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
            Self::RawBytes => "raw_bytes",
            Self::Key => "key",
            Self::Mouse => "mouse",
            Self::Focus => "focus",
            Self::Resize => "resize",
            Self::PasteBegin => "paste_begin",
            Self::PasteChunk => "paste_chunk",
            Self::PasteCommit => "paste_commit",
            Self::PasteAbort => "paste_abort",
        }
    }
}

/// Opaque terminal input frame.
///
/// Validate the header only. Do not inspect the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInputFrame {
    bytes: Vec<u8>,
}

impl TerminalInputFrame {
    /// Validate the header only. Does not decode the body.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TerminalInputFrameError> {
        Self::validate(bytes)?;
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }

    /// Validate the header and adopt an owned buffer without a copy.
    pub fn from_vec(bytes: Vec<u8>) -> Result<Self, TerminalInputFrameError> {
        Self::validate(&bytes)?;
        Ok(Self { bytes })
    }

    fn validate(bytes: &[u8]) -> Result<(), TerminalInputFrameError> {
        if bytes.len() < INPUT_HEADER_BYTES {
            return Err(TerminalInputFrameError::TruncatedHeader);
        }
        let scheme = bytes[0];
        if scheme != TERMINAL_INPUT_SCHEME_VERSION {
            return Err(TerminalInputFrameError::WrongSchemeVersion { found: scheme });
        }
        let kind = bytes[1];
        if TerminalInputKind::from_byte(kind).is_none() {
            return Err(TerminalInputFrameError::UnknownKind { found: kind });
        }
        let declared = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
        let remaining = bytes.len() - INPUT_HEADER_BYTES;
        if declared != remaining {
            return Err(TerminalInputFrameError::BodyLengthMismatch {
                declared,
                remaining,
            });
        }
        Ok(())
    }

    /// Kind from the header.
    #[must_use]
    pub fn kind(&self) -> TerminalInputKind {
        TerminalInputKind::from_byte(self.bytes[1]).unwrap_or(TerminalInputKind::RawBytes)
    }

    /// Client-chosen operation id from the header.
    #[must_use]
    pub fn operation_id(&self) -> u64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.bytes[4..12]);
        u64::from_be_bytes(buf)
    }

    /// Body length from the header. Hub uses this value for byte accounting.
    #[must_use]
    pub fn body_len(&self) -> usize {
        self.bytes.len() - INPUT_HEADER_BYTES
    }

    /// Emit the exact wire bytes for forwarding.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// Borrow the exact wire bytes for forwarding without a copy.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Take ownership of the exact wire bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Header validation failure for an opaque terminal input frame.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalInputFrameError {
    /// Fewer than twelve header bytes arrived.
    #[error("terminal input frame header is truncated")]
    TruncatedHeader,
    /// Byte 0 is not the current scheme version.
    #[error("unsupported terminal input scheme version {found}")]
    WrongSchemeVersion {
        /// Observed scheme byte.
        found: u8,
    },
    /// Byte 1 is not a known kind tag.
    #[error("unsupported terminal input kind {found}")]
    UnknownKind {
        /// Observed kind tag.
        found: u8,
    },
    /// Declared body length does not equal the remaining byte count.
    #[error("terminal input body length mismatch: declared {declared}, remaining {remaining}")]
    BodyLengthMismatch {
        /// Length field from the header.
        declared: usize,
        /// Bytes after the twelve-byte header.
        remaining: usize,
    },
}
