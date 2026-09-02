//! Opaque compact-binary terminal input frames.
//!
//! Hub may construct, forward, and emit these frames. It must not decode the
//! body. Semantic encode and decode live in `botster-terminal-protocol-client`.

/// Scheme version. Byte 0 uses exact equality.
pub const TERMINAL_INPUT_SCHEME_VERSION: u8 = 1;
/// `u16` body ceiling.
pub const MAX_TERMINAL_INPUT_BODY_BYTES: u16 = 65_535;
/// Four-byte header plus the body ceiling.
pub const MAX_TERMINAL_INPUT_FRAME_BYTES: usize = 4 + MAX_TERMINAL_INPUT_BODY_BYTES as usize;
/// `input` data ceiling. The body is data only.
pub const MAX_INPUT_DATA_BYTES: usize = MAX_TERMINAL_INPUT_BODY_BYTES as usize;
/// Two `u64` freshness values at the start of a mode-gated body.
pub const MODE_GATED_PREFIX_BYTES: usize = 16;
/// Mode-gated data ceiling after the freshness prefix.
pub const MAX_MODE_GATED_DATA_BYTES: usize =
    MAX_TERMINAL_INPUT_BODY_BYTES as usize - MODE_GATED_PREFIX_BYTES;
/// Exact resize body: `rows: u16` plus `cols: u16`.
pub const RESIZE_BODY_BYTES: usize = 4;
/// Exact paste-begin body: operation id, freshness token, and total length.
pub const PASTE_BEGIN_BODY_BYTES: usize = 24;
/// Paste-chunk prefix: operation id and zero-based chunk index.
pub const PASTE_CHUNK_PREFIX_BYTES: usize = 8;
/// Paste-chunk data ceiling after the operation id and chunk index.
pub const MAX_PASTE_CHUNK_DATA_BYTES: usize =
    MAX_TERMINAL_INPUT_BODY_BYTES as usize - PASTE_CHUNK_PREFIX_BYTES;
/// Exact paste-commit body: operation id.
pub const PASTE_COMMIT_BODY_BYTES: usize = 4;
/// Exact paste-abort body: operation id.
pub const PASTE_ABORT_BODY_BYTES: usize = 4;
/// Maximum complete paste content held by one subscription owner.
pub const MAX_PASTE_BYTES: usize = 1_048_576;
/// Maximum chunk count for one complete paste.
pub const MAX_PASTE_CHUNKS: usize = MAX_PASTE_BYTES.div_ceil(MAX_PASTE_CHUNK_DATA_BYTES);

const HEADER_BYTES: usize = 4;
const KIND_INPUT: u8 = 1;
const KIND_MODE_GATED_INPUT: u8 = 2;
const KIND_RESIZE: u8 = 3;
const KIND_PASTE_BEGIN: u8 = 4;
const KIND_PASTE_CHUNK: u8 = 5;
const KIND_PASTE_COMMIT: u8 = 6;
const KIND_PASTE_ABORT: u8 = 7;

/// Opaque terminal input frame.
///
/// Validate the header only. Do not inspect the payload, freshness values,
/// rows, or cols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInputFrame {
    bytes: Vec<u8>,
}

impl TerminalInputFrame {
    /// Validate the header only. Does not decode the body.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TerminalInputFrameError> {
        if bytes.len() < HEADER_BYTES {
            return Err(TerminalInputFrameError::TruncatedHeader);
        }
        let scheme = bytes[0];
        if scheme != TERMINAL_INPUT_SCHEME_VERSION {
            return Err(TerminalInputFrameError::WrongSchemeVersion { found: scheme });
        }
        let kind = bytes[1];
        if !matches!(
            kind,
            KIND_INPUT
                | KIND_MODE_GATED_INPUT
                | KIND_RESIZE
                | KIND_PASTE_BEGIN
                | KIND_PASTE_CHUNK
                | KIND_PASTE_COMMIT
                | KIND_PASTE_ABORT
        ) {
            return Err(TerminalInputFrameError::UnknownKind { found: kind });
        }
        let declared = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
        let remaining = bytes.len() - HEADER_BYTES;
        if declared != remaining {
            return Err(TerminalInputFrameError::BodyLengthMismatch {
                declared,
                remaining,
            });
        }
        Ok(Self {
            bytes: bytes.to_vec(),
        })
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
}

/// Header validation failure for an opaque terminal input frame.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalInputFrameError {
    /// Fewer than four header bytes arrived.
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
        /// Bytes after the four-byte header.
        remaining: usize,
    },
}
