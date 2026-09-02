//! Semantic encode and decode for compact-binary terminal input frames.

use botster_terminal_protocol::{
    TerminalInputFrame, TerminalInputFrameError, MAX_INPUT_DATA_BYTES, MAX_MODE_GATED_DATA_BYTES,
    MAX_PASTE_BYTES, MAX_PASTE_CHUNK_DATA_BYTES, MODE_GATED_PREFIX_BYTES, PASTE_ABORT_BODY_BYTES,
    PASTE_BEGIN_BODY_BYTES, PASTE_CHUNK_PREFIX_BYTES, PASTE_COMMIT_BODY_BYTES, RESIZE_BODY_BYTES,
    TERMINAL_INPUT_SCHEME_VERSION,
};

use crate::events::TerminalInputKind;

const KIND_INPUT: u8 = 1;
const KIND_MODE_GATED_INPUT: u8 = 2;
const KIND_RESIZE: u8 = 3;
const KIND_PASTE_BEGIN: u8 = 4;
const KIND_PASTE_CHUNK: u8 = 5;
const KIND_PASTE_COMMIT: u8 = 6;
const KIND_PASTE_ABORT: u8 = 7;
const HEADER_BYTES: usize = 4;

/// Semantic terminal input command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInputCommand {
    /// Raw PTY bytes. May be non-UTF-8.
    Input {
        /// Input bytes.
        data: Vec<u8>,
    },
    /// Mode-gated input with two plain freshness fields.
    ModeGatedInput {
        /// Worker ownership epoch.
        mode_generation: u64,
        /// Complete-ModeFlags counter within the epoch.
        mode_revision: u64,
        /// Input bytes.
        data: Vec<u8>,
    },
    /// Desired terminal geometry.
    Resize {
        /// Rows.
        rows: u16,
        /// Columns.
        cols: u16,
    },
    /// Start one bounded atomic paste operation.
    PasteBegin {
        /// Client-chosen monotonic operation id.
        operation_id: u32,
        /// Worker ownership epoch.
        mode_generation: u64,
        /// Complete-ModeFlags counter within the epoch.
        mode_revision: u64,
        /// Exact complete paste content length.
        total_len: u32,
    },
    /// Add one ordered paste chunk.
    PasteChunk {
        /// Operation id from the accepted begin frame.
        operation_id: u32,
        /// Zero-based ordered chunk index.
        index: u32,
        /// Chunk bytes. May be non-UTF-8.
        data: Vec<u8>,
    },
    /// Commit one completely assembled paste.
    PasteCommit {
        /// Operation id from the accepted begin frame.
        operation_id: u32,
    },
    /// Abort one paste before worker submission.
    PasteAbort {
        /// Operation id from the accepted begin frame.
        operation_id: u32,
    },
}

/// Encode a semantic command into an opaque input frame.
pub fn encode_terminal_input(
    command: &TerminalInputCommand,
) -> Result<TerminalInputFrame, TerminalInputEncodeError> {
    let (kind, body) = match command {
        TerminalInputCommand::Input { data } => {
            if data.len() > MAX_INPUT_DATA_BYTES {
                return Err(TerminalInputEncodeError::PayloadTooLarge {
                    kind: TerminalInputKind::Input,
                    max: MAX_INPUT_DATA_BYTES,
                    actual: data.len(),
                });
            }
            (KIND_INPUT, data.clone())
        }
        TerminalInputCommand::ModeGatedInput {
            mode_generation,
            mode_revision,
            data,
        } => {
            if data.len() > MAX_MODE_GATED_DATA_BYTES {
                return Err(TerminalInputEncodeError::PayloadTooLarge {
                    kind: TerminalInputKind::ModeGatedInput,
                    max: MAX_MODE_GATED_DATA_BYTES,
                    actual: data.len(),
                });
            }
            let mut body = Vec::with_capacity(MODE_GATED_PREFIX_BYTES + data.len());
            body.extend_from_slice(&mode_generation.to_be_bytes());
            body.extend_from_slice(&mode_revision.to_be_bytes());
            body.extend_from_slice(data);
            (KIND_MODE_GATED_INPUT, body)
        }
        TerminalInputCommand::Resize { rows, cols } => {
            let mut body = Vec::with_capacity(RESIZE_BODY_BYTES);
            body.extend_from_slice(&rows.to_be_bytes());
            body.extend_from_slice(&cols.to_be_bytes());
            (KIND_RESIZE, body)
        }
        TerminalInputCommand::PasteBegin {
            operation_id,
            mode_generation,
            mode_revision,
            total_len,
        } => {
            let mut body = Vec::with_capacity(PASTE_BEGIN_BODY_BYTES);
            body.extend_from_slice(&operation_id.to_be_bytes());
            body.extend_from_slice(&mode_generation.to_be_bytes());
            body.extend_from_slice(&mode_revision.to_be_bytes());
            body.extend_from_slice(&total_len.to_be_bytes());
            (KIND_PASTE_BEGIN, body)
        }
        TerminalInputCommand::PasteChunk {
            operation_id,
            index,
            data,
        } => {
            if data.len() > MAX_PASTE_CHUNK_DATA_BYTES {
                return Err(TerminalInputEncodeError::PayloadTooLarge {
                    kind: TerminalInputKind::Paste,
                    max: MAX_PASTE_CHUNK_DATA_BYTES,
                    actual: data.len(),
                });
            }
            let mut body = Vec::with_capacity(PASTE_CHUNK_PREFIX_BYTES + data.len());
            body.extend_from_slice(&operation_id.to_be_bytes());
            body.extend_from_slice(&index.to_be_bytes());
            body.extend_from_slice(data);
            (KIND_PASTE_CHUNK, body)
        }
        TerminalInputCommand::PasteCommit { operation_id } => {
            (KIND_PASTE_COMMIT, operation_id.to_be_bytes().to_vec())
        }
        TerminalInputCommand::PasteAbort { operation_id } => {
            (KIND_PASTE_ABORT, operation_id.to_be_bytes().to_vec())
        }
    };
    let body_len =
        u16::try_from(body.len()).map_err(|_| TerminalInputEncodeError::PayloadTooLarge {
            kind: command.kind(),
            max: u16::MAX as usize,
            actual: body.len(),
        })?;
    let mut bytes = Vec::with_capacity(HEADER_BYTES + body.len());
    bytes.push(TERMINAL_INPUT_SCHEME_VERSION);
    bytes.push(kind);
    bytes.extend_from_slice(&body_len.to_be_bytes());
    bytes.extend_from_slice(&body);
    TerminalInputFrame::from_bytes(&bytes).map_err(TerminalInputEncodeError::Frame)
}

/// Decode an opaque input frame into a semantic command.
pub fn decode_terminal_input(
    frame: &TerminalInputFrame,
) -> Result<TerminalInputCommand, TerminalInputDecodeError> {
    let bytes = frame.as_bytes();
    if bytes.len() < HEADER_BYTES {
        return Err(TerminalInputDecodeError::TruncatedBody);
    }
    let kind = bytes[1];
    let body = &bytes[HEADER_BYTES..];
    match kind {
        KIND_INPUT => Ok(TerminalInputCommand::Input {
            data: body.to_vec(),
        }),
        KIND_MODE_GATED_INPUT => {
            if body.len() < MODE_GATED_PREFIX_BYTES {
                return Err(TerminalInputDecodeError::TruncatedBody);
            }
            let mode_generation = u64::from_be_bytes(
                body[..8]
                    .try_into()
                    .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
            );
            let mode_revision = u64::from_be_bytes(
                body[8..16]
                    .try_into()
                    .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
            );
            Ok(TerminalInputCommand::ModeGatedInput {
                mode_generation,
                mode_revision,
                data: body[MODE_GATED_PREFIX_BYTES..].to_vec(),
            })
        }
        KIND_RESIZE => {
            if body.len() != RESIZE_BODY_BYTES {
                return Err(TerminalInputDecodeError::ResizeBodyLength { actual: body.len() });
            }
            let rows = u16::from_be_bytes([body[0], body[1]]);
            let cols = u16::from_be_bytes([body[2], body[3]]);
            Ok(TerminalInputCommand::Resize { rows, cols })
        }
        KIND_PASTE_BEGIN => {
            if body.len() != PASTE_BEGIN_BODY_BYTES {
                return Err(TerminalInputDecodeError::PasteBodyLength {
                    kind: TerminalInputKind::Paste,
                    expected: PASTE_BEGIN_BODY_BYTES,
                    actual: body.len(),
                });
            }
            Ok(TerminalInputCommand::PasteBegin {
                operation_id: u32::from_be_bytes(
                    body[0..4]
                        .try_into()
                        .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
                ),
                mode_generation: u64::from_be_bytes(
                    body[4..12]
                        .try_into()
                        .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
                ),
                mode_revision: u64::from_be_bytes(
                    body[12..20]
                        .try_into()
                        .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
                ),
                total_len: u32::from_be_bytes(
                    body[20..24]
                        .try_into()
                        .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
                ),
            })
        }
        KIND_PASTE_CHUNK => {
            if body.len() < PASTE_CHUNK_PREFIX_BYTES {
                return Err(TerminalInputDecodeError::TruncatedBody);
            }
            Ok(TerminalInputCommand::PasteChunk {
                operation_id: u32::from_be_bytes(
                    body[0..4]
                        .try_into()
                        .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
                ),
                index: u32::from_be_bytes(
                    body[4..8]
                        .try_into()
                        .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
                ),
                data: body[PASTE_CHUNK_PREFIX_BYTES..].to_vec(),
            })
        }
        KIND_PASTE_COMMIT | KIND_PASTE_ABORT => {
            let expected = if kind == KIND_PASTE_COMMIT {
                PASTE_COMMIT_BODY_BYTES
            } else {
                PASTE_ABORT_BODY_BYTES
            };
            if body.len() != expected {
                return Err(TerminalInputDecodeError::PasteBodyLength {
                    kind: TerminalInputKind::Paste,
                    expected,
                    actual: body.len(),
                });
            }
            let operation_id = u32::from_be_bytes(
                body.try_into()
                    .map_err(|_| TerminalInputDecodeError::TruncatedBody)?,
            );
            if kind == KIND_PASTE_COMMIT {
                Ok(TerminalInputCommand::PasteCommit { operation_id })
            } else {
                Ok(TerminalInputCommand::PasteAbort { operation_id })
            }
        }
        found => Err(TerminalInputDecodeError::UnknownKind { found }),
    }
}

impl TerminalInputCommand {
    fn kind(&self) -> TerminalInputKind {
        match self {
            Self::Input { .. } => TerminalInputKind::Input,
            Self::ModeGatedInput { .. } => TerminalInputKind::ModeGatedInput,
            Self::Resize { .. } => TerminalInputKind::Resize,
            Self::PasteBegin { .. }
            | Self::PasteChunk { .. }
            | Self::PasteCommit { .. }
            | Self::PasteAbort { .. } => TerminalInputKind::Paste,
        }
    }
}

/// Encode one complete paste into Begin, ordered Chunk frames, and Commit.
pub fn encode_paste(
    operation_id: u32,
    mode_generation: u64,
    mode_revision: u64,
    data: &[u8],
) -> Result<Vec<TerminalInputFrame>, TerminalInputEncodeError> {
    if data.is_empty() {
        return Err(TerminalInputEncodeError::EmptyPaste);
    }
    if data.len() > MAX_PASTE_BYTES {
        return Err(TerminalInputEncodeError::PayloadTooLarge {
            kind: TerminalInputKind::Paste,
            max: MAX_PASTE_BYTES,
            actual: data.len(),
        });
    }
    let mut frames = Vec::with_capacity(data.len().div_ceil(MAX_PASTE_CHUNK_DATA_BYTES) + 2);
    frames.push(encode_terminal_input(&TerminalInputCommand::PasteBegin {
        operation_id,
        mode_generation,
        mode_revision,
        total_len: data.len() as u32,
    })?);
    for (index, chunk) in data.chunks(MAX_PASTE_CHUNK_DATA_BYTES).enumerate() {
        frames.push(encode_terminal_input(&TerminalInputCommand::PasteChunk {
            operation_id,
            index: index as u32,
            data: chunk.to_vec(),
        })?);
    }
    frames.push(encode_terminal_input(&TerminalInputCommand::PasteCommit {
        operation_id,
    })?);
    Ok(frames)
}

/// Encode one paste abort frame.
pub fn encode_paste_abort(operation_id: u32) -> TerminalInputFrame {
    encode_terminal_input(&TerminalInputCommand::PasteAbort { operation_id })
        .expect("a fixed-size paste abort frame is valid")
}

/// Fallible encode error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalInputEncodeError {
    /// A paste must contain at least one byte.
    #[error("terminal paste payload is empty")]
    EmptyPaste,
    /// Payload exceeds the per-kind ceiling.
    #[error("terminal input payload too large: kind={kind:?} max={max} actual={actual}")]
    PayloadTooLarge {
        /// Command kind that overflowed.
        kind: TerminalInputKind,
        /// Per-kind maximum.
        max: usize,
        /// Observed length.
        actual: usize,
    },
    /// Header validation rejected a constructed frame.
    #[error(transparent)]
    Frame(#[from] TerminalInputFrameError),
}

/// Fallible decode error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalInputDecodeError {
    /// Body is shorter than the kind's fixed prefix.
    #[error("terminal input body is truncated")]
    TruncatedBody,
    /// Kind tag is not one of the three published values.
    #[error("unsupported terminal input kind {found}")]
    UnknownKind {
        /// Observed kind tag.
        found: u8,
    },
    /// Resize body is not exactly four bytes.
    #[error("resize body length must be {RESIZE_BODY_BYTES}, got {actual}")]
    ResizeBodyLength {
        /// Observed body length.
        actual: usize,
    },
    /// A fixed paste body has the wrong length.
    #[error("paste body length must be {expected}, got {actual}")]
    PasteBodyLength {
        /// Semantic kind.
        kind: TerminalInputKind,
        /// Required body length.
        expected: usize,
        /// Observed body length.
        actual: usize,
    },
}
