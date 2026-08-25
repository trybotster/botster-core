//! Semantic encode and decode for compact-binary terminal input frames.

use botster_terminal_protocol::{
    TerminalInputFrame, TerminalInputFrameError, MAX_INPUT_DATA_BYTES, MAX_MODE_GATED_DATA_BYTES,
    MODE_GATED_PREFIX_BYTES, RESIZE_BODY_BYTES, TERMINAL_INPUT_SCHEME_VERSION,
};

use crate::events::TerminalInputKind;

const KIND_INPUT: u8 = 1;
const KIND_MODE_GATED_INPUT: u8 = 2;
const KIND_RESIZE: u8 = 3;
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
    };
    let body_len = u16::try_from(body.len()).map_err(|_| TerminalInputEncodeError::PayloadTooLarge {
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
                return Err(TerminalInputDecodeError::ResizeBodyLength {
                    actual: body.len(),
                });
            }
            let rows = u16::from_be_bytes([body[0], body[1]]);
            let cols = u16::from_be_bytes([body[2], body[3]]);
            Ok(TerminalInputCommand::Resize { rows, cols })
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
        }
    }
}

/// Fallible encode error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalInputEncodeError {
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
}
