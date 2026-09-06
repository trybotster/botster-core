//! Semantic encode and decode for scheme 2 terminal input frames.
//!
//! Every command carries a client-chosen `operation_id`. Ids are strictly
//! increasing within one route attach generation, starting at 1. Paste
//! continuation frames (`PasteChunk`, `PasteCommit`, `PasteAbort`) carry the
//! id of the active paste; that repeated id is the only permitted equality.

use botster_terminal_protocol::{
    TerminalInputFrame, TerminalInputFrameError, TerminalInputKind, TerminalKey, TerminalKeyAction,
    TerminalMouseAction, TerminalMouseButton, FOCUS_BODY_BYTES, INPUT_HEADER_BYTES,
    KEY_PREFIX_BYTES, MAX_PASTE_BYTES, MAX_PASTE_CHUNK_DATA_BYTES, MAX_RAW_INPUT_BYTES,
    MAX_TERMINAL_INPUT_BODY_BYTES, MOUSE_BODY_BYTES, PASTE_ABORT_BODY_BYTES,
    PASTE_BEGIN_BODY_BYTES, PASTE_CHUNK_PREFIX_BYTES, PASTE_COMMIT_BODY_BYTES, RESIZE_BODY_BYTES,
    TERMINAL_INPUT_SCHEME_VERSION,
};

/// Semantic terminal input command. Mirrors the wire kinds one to one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInputCommand {
    /// Explicit raw PTY bytes. Never mode-encoded by the worker.
    RawBytes {
        /// Client-chosen monotonic operation id.
        operation_id: u64,
        /// Bytes written to the PTY as given. May be non-UTF-8.
        data: Vec<u8>,
    },
    /// Physical key event encoded by the worker key encoder.
    Key {
        /// Client-chosen monotonic operation id.
        operation_id: u64,
        /// Press, release, or repeat.
        action: TerminalKeyAction,
        /// Physical key identity.
        key: TerminalKey,
        /// Held modifiers from [`botster_terminal_protocol::terminal_mods`].
        mods: u16,
        /// Modifiers the platform already consumed to produce `text`.
        consumed_mods: u16,
        /// Whether this event is part of an IME composition.
        composing: bool,
        /// Unshifted Unicode codepoint for the key, or 0 when unknown.
        unshifted_codepoint: u32,
        /// Layout-dependent UTF-8 text without control transformations.
        text: String,
    },
    /// Mouse event with cell and pixel position.
    Mouse {
        /// Client-chosen monotonic operation id.
        operation_id: u64,
        /// Press, release, or motion.
        action: TerminalMouseAction,
        /// Button for press and release, held button for motion, or none.
        button: Option<TerminalMouseButton>,
        /// Held modifiers from [`botster_terminal_protocol::terminal_mods`].
        mods: u16,
        /// Zero-based cell column.
        col: u16,
        /// Zero-based cell row.
        row: u16,
        /// Surface pixel x, or 0 when the client has no pixel geometry.
        x_px: u32,
        /// Surface pixel y, or 0 when the client has no pixel geometry.
        y_px: u32,
    },
    /// Focus gained or lost.
    Focus {
        /// Client-chosen monotonic operation id.
        operation_id: u64,
        /// Whether the terminal gained focus.
        focused: bool,
    },
    /// Terminal geometry in cells and pixels.
    Resize {
        /// Client-chosen monotonic operation id.
        operation_id: u64,
        /// Rows.
        rows: u16,
        /// Columns.
        cols: u16,
        /// Surface width in pixels, or 0 when unknown.
        width_px: u32,
        /// Surface height in pixels, or 0 when unknown.
        height_px: u32,
    },
    /// Start one bounded paste.
    PasteBegin {
        /// Client-chosen monotonic operation id.
        operation_id: u64,
        /// Exact complete paste content length.
        total_len: u32,
        /// Allow content that `ghostty_paste_is_safe` rejects.
        allow_unsafe: bool,
    },
    /// One ordered paste chunk for the active paste.
    PasteChunk {
        /// Operation id of the active paste.
        operation_id: u64,
        /// Zero-based ordered chunk index.
        index: u32,
        /// Chunk bytes. May be non-UTF-8.
        data: Vec<u8>,
    },
    /// Commit the assembled active paste.
    PasteCommit {
        /// Operation id of the active paste.
        operation_id: u64,
    },
    /// Abort the active paste before worker submission.
    PasteAbort {
        /// Operation id of the active paste.
        operation_id: u64,
    },
}

impl TerminalInputCommand {
    /// Wire kind for this command.
    #[must_use]
    pub const fn kind(&self) -> TerminalInputKind {
        match self {
            Self::RawBytes { .. } => TerminalInputKind::RawBytes,
            Self::Key { .. } => TerminalInputKind::Key,
            Self::Mouse { .. } => TerminalInputKind::Mouse,
            Self::Focus { .. } => TerminalInputKind::Focus,
            Self::Resize { .. } => TerminalInputKind::Resize,
            Self::PasteBegin { .. } => TerminalInputKind::PasteBegin,
            Self::PasteChunk { .. } => TerminalInputKind::PasteChunk,
            Self::PasteCommit { .. } => TerminalInputKind::PasteCommit,
            Self::PasteAbort { .. } => TerminalInputKind::PasteAbort,
        }
    }

    /// Operation id carried by this command.
    #[must_use]
    pub const fn operation_id(&self) -> u64 {
        match self {
            Self::RawBytes { operation_id, .. }
            | Self::Key { operation_id, .. }
            | Self::Mouse { operation_id, .. }
            | Self::Focus { operation_id, .. }
            | Self::Resize { operation_id, .. }
            | Self::PasteBegin { operation_id, .. }
            | Self::PasteChunk { operation_id, .. }
            | Self::PasteCommit { operation_id }
            | Self::PasteAbort { operation_id } => *operation_id,
        }
    }

    /// Whether this command continues an active paste instead of opening a
    /// new operation.
    #[must_use]
    pub const fn continues_paste(&self) -> bool {
        matches!(
            self,
            Self::PasteChunk { .. } | Self::PasteCommit { .. } | Self::PasteAbort { .. }
        )
    }

    /// Client payload bytes this command retains until its result arrives.
    #[must_use]
    pub fn payload_bytes(&self) -> usize {
        match self {
            Self::RawBytes { data, .. } | Self::PasteChunk { data, .. } => data.len(),
            Self::Key { text, .. } => text.len(),
            _ => 0,
        }
    }
}

/// Encode a semantic command into an opaque input frame.
pub fn encode_terminal_input(
    command: &TerminalInputCommand,
) -> Result<TerminalInputFrame, TerminalInputEncodeError> {
    let kind = command.kind();
    let body: Vec<u8> = match command {
        TerminalInputCommand::RawBytes { data, .. } => {
            check_len(kind, MAX_RAW_INPUT_BYTES, data.len())?;
            data.clone()
        }
        TerminalInputCommand::Key {
            action,
            key,
            mods,
            consumed_mods,
            composing,
            unshifted_codepoint,
            text,
            ..
        } => {
            check_len(
                kind,
                MAX_TERMINAL_INPUT_BODY_BYTES as usize - KEY_PREFIX_BYTES,
                text.len(),
            )?;
            let mut body = Vec::with_capacity(KEY_PREFIX_BYTES + text.len());
            body.push(action.as_byte());
            body.extend_from_slice(&key.as_u16().to_be_bytes());
            body.extend_from_slice(&mods.to_be_bytes());
            body.extend_from_slice(&consumed_mods.to_be_bytes());
            body.push(u8::from(*composing));
            body.extend_from_slice(&unshifted_codepoint.to_be_bytes());
            body.extend_from_slice(text.as_bytes());
            body
        }
        TerminalInputCommand::Mouse {
            action,
            button,
            mods,
            col,
            row,
            x_px,
            y_px,
            ..
        } => {
            let mut body = Vec::with_capacity(MOUSE_BODY_BYTES);
            body.push(action.as_byte());
            body.push(u8::from(button.is_some()));
            body.push(button.map_or(0, TerminalMouseButton::as_byte));
            body.extend_from_slice(&mods.to_be_bytes());
            body.extend_from_slice(&col.to_be_bytes());
            body.extend_from_slice(&row.to_be_bytes());
            body.extend_from_slice(&x_px.to_be_bytes());
            body.extend_from_slice(&y_px.to_be_bytes());
            body
        }
        TerminalInputCommand::Focus { focused, .. } => vec![u8::from(*focused)],
        TerminalInputCommand::Resize {
            rows,
            cols,
            width_px,
            height_px,
            ..
        } => {
            let mut body = Vec::with_capacity(RESIZE_BODY_BYTES);
            body.extend_from_slice(&rows.to_be_bytes());
            body.extend_from_slice(&cols.to_be_bytes());
            body.extend_from_slice(&width_px.to_be_bytes());
            body.extend_from_slice(&height_px.to_be_bytes());
            body
        }
        TerminalInputCommand::PasteBegin {
            total_len,
            allow_unsafe,
            ..
        } => {
            let mut body = Vec::with_capacity(PASTE_BEGIN_BODY_BYTES);
            body.extend_from_slice(&total_len.to_be_bytes());
            body.push(u8::from(*allow_unsafe));
            body
        }
        TerminalInputCommand::PasteChunk { index, data, .. } => {
            check_len(kind, MAX_PASTE_CHUNK_DATA_BYTES, data.len())?;
            let mut body = Vec::with_capacity(PASTE_CHUNK_PREFIX_BYTES + data.len());
            body.extend_from_slice(&index.to_be_bytes());
            body.extend_from_slice(data);
            body
        }
        TerminalInputCommand::PasteCommit { .. } | TerminalInputCommand::PasteAbort { .. } => {
            Vec::new()
        }
    };
    let body_len =
        u16::try_from(body.len()).map_err(|_| TerminalInputEncodeError::PayloadTooLarge {
            kind,
            max: MAX_TERMINAL_INPUT_BODY_BYTES as usize,
            actual: body.len(),
        })?;
    let mut bytes = Vec::with_capacity(INPUT_HEADER_BYTES + body.len());
    bytes.push(TERMINAL_INPUT_SCHEME_VERSION);
    bytes.push(kind.as_byte());
    bytes.extend_from_slice(&body_len.to_be_bytes());
    bytes.extend_from_slice(&command.operation_id().to_be_bytes());
    bytes.extend_from_slice(&body);
    TerminalInputFrame::from_vec(bytes).map_err(TerminalInputEncodeError::Frame)
}

fn check_len(
    kind: TerminalInputKind,
    max: usize,
    actual: usize,
) -> Result<(), TerminalInputEncodeError> {
    if actual > max {
        return Err(TerminalInputEncodeError::PayloadTooLarge { kind, max, actual });
    }
    Ok(())
}

/// Decode an opaque input frame into a semantic command.
pub fn decode_terminal_input(
    frame: &TerminalInputFrame,
) -> Result<TerminalInputCommand, TerminalInputDecodeError> {
    let kind = frame.kind();
    let operation_id = frame.operation_id();
    let body = &frame.as_bytes()[INPUT_HEADER_BYTES..];
    match kind {
        TerminalInputKind::RawBytes => Ok(TerminalInputCommand::RawBytes {
            operation_id,
            data: body.to_vec(),
        }),
        TerminalInputKind::Key => {
            if body.len() < KEY_PREFIX_BYTES {
                return Err(TerminalInputDecodeError::TruncatedBody { kind });
            }
            let action = TerminalKeyAction::from_byte(body[0]).ok_or(
                TerminalInputDecodeError::UnknownValue {
                    field: "action",
                    found: u32::from(body[0]),
                },
            )?;
            let key_value = u16::from_be_bytes([body[1], body[2]]);
            let key =
                TerminalKey::from_u16(key_value).ok_or(TerminalInputDecodeError::UnknownValue {
                    field: "key",
                    found: u32::from(key_value),
                })?;
            let composing = match body[7] {
                0 => false,
                1 => true,
                found => {
                    return Err(TerminalInputDecodeError::UnknownValue {
                        field: "composing",
                        found: u32::from(found),
                    })
                }
            };
            let text = std::str::from_utf8(&body[KEY_PREFIX_BYTES..])
                .map_err(|_| TerminalInputDecodeError::InvalidText)?
                .to_owned();
            Ok(TerminalInputCommand::Key {
                operation_id,
                action,
                key,
                mods: u16::from_be_bytes([body[3], body[4]]),
                consumed_mods: u16::from_be_bytes([body[5], body[6]]),
                composing,
                unshifted_codepoint: u32::from_be_bytes([body[8], body[9], body[10], body[11]]),
                text,
            })
        }
        TerminalInputKind::Mouse => {
            expect_len(kind, MOUSE_BODY_BYTES, body.len())?;
            let action = TerminalMouseAction::from_byte(body[0]).ok_or(
                TerminalInputDecodeError::UnknownValue {
                    field: "action",
                    found: u32::from(body[0]),
                },
            )?;
            let button = match body[1] {
                0 => None,
                1 => Some(TerminalMouseButton::from_byte(body[2]).ok_or(
                    TerminalInputDecodeError::UnknownValue {
                        field: "button",
                        found: u32::from(body[2]),
                    },
                )?),
                found => {
                    return Err(TerminalInputDecodeError::UnknownValue {
                        field: "has_button",
                        found: u32::from(found),
                    })
                }
            };
            Ok(TerminalInputCommand::Mouse {
                operation_id,
                action,
                button,
                mods: u16::from_be_bytes([body[3], body[4]]),
                col: u16::from_be_bytes([body[5], body[6]]),
                row: u16::from_be_bytes([body[7], body[8]]),
                x_px: u32::from_be_bytes([body[9], body[10], body[11], body[12]]),
                y_px: u32::from_be_bytes([body[13], body[14], body[15], body[16]]),
            })
        }
        TerminalInputKind::Focus => {
            expect_len(kind, FOCUS_BODY_BYTES, body.len())?;
            let focused = match body[0] {
                0 => false,
                1 => true,
                found => {
                    return Err(TerminalInputDecodeError::UnknownValue {
                        field: "focused",
                        found: u32::from(found),
                    })
                }
            };
            Ok(TerminalInputCommand::Focus {
                operation_id,
                focused,
            })
        }
        TerminalInputKind::Resize => {
            expect_len(kind, RESIZE_BODY_BYTES, body.len())?;
            Ok(TerminalInputCommand::Resize {
                operation_id,
                rows: u16::from_be_bytes([body[0], body[1]]),
                cols: u16::from_be_bytes([body[2], body[3]]),
                width_px: u32::from_be_bytes([body[4], body[5], body[6], body[7]]),
                height_px: u32::from_be_bytes([body[8], body[9], body[10], body[11]]),
            })
        }
        TerminalInputKind::PasteBegin => {
            expect_len(kind, PASTE_BEGIN_BODY_BYTES, body.len())?;
            let allow_unsafe = match body[4] {
                0 => false,
                1 => true,
                found => {
                    return Err(TerminalInputDecodeError::UnknownValue {
                        field: "allow_unsafe",
                        found: u32::from(found),
                    })
                }
            };
            Ok(TerminalInputCommand::PasteBegin {
                operation_id,
                total_len: u32::from_be_bytes([body[0], body[1], body[2], body[3]]),
                allow_unsafe,
            })
        }
        TerminalInputKind::PasteChunk => {
            if body.len() < PASTE_CHUNK_PREFIX_BYTES {
                return Err(TerminalInputDecodeError::TruncatedBody { kind });
            }
            Ok(TerminalInputCommand::PasteChunk {
                operation_id,
                index: u32::from_be_bytes([body[0], body[1], body[2], body[3]]),
                data: body[PASTE_CHUNK_PREFIX_BYTES..].to_vec(),
            })
        }
        TerminalInputKind::PasteCommit => {
            expect_len(kind, PASTE_COMMIT_BODY_BYTES, body.len())?;
            Ok(TerminalInputCommand::PasteCommit { operation_id })
        }
        TerminalInputKind::PasteAbort => {
            expect_len(kind, PASTE_ABORT_BODY_BYTES, body.len())?;
            Ok(TerminalInputCommand::PasteAbort { operation_id })
        }
    }
}

fn expect_len(
    kind: TerminalInputKind,
    expected: usize,
    actual: usize,
) -> Result<(), TerminalInputDecodeError> {
    if expected != actual {
        return Err(TerminalInputDecodeError::BodyLength {
            kind,
            expected,
            actual,
        });
    }
    Ok(())
}

/// Encode one complete paste into `PasteBegin`, ordered `PasteChunk` frames,
/// and `PasteCommit`, all carrying `operation_id`.
pub fn encode_paste(
    operation_id: u64,
    allow_unsafe: bool,
    data: &[u8],
) -> Result<Vec<TerminalInputFrame>, TerminalInputEncodeError> {
    if data.is_empty() {
        return Err(TerminalInputEncodeError::EmptyPaste);
    }
    if data.len() > MAX_PASTE_BYTES {
        return Err(TerminalInputEncodeError::PayloadTooLarge {
            kind: TerminalInputKind::PasteBegin,
            max: MAX_PASTE_BYTES,
            actual: data.len(),
        });
    }
    let mut frames = Vec::with_capacity(data.len().div_ceil(MAX_PASTE_CHUNK_DATA_BYTES) + 2);
    frames.push(encode_terminal_input(&TerminalInputCommand::PasteBegin {
        operation_id,
        total_len: data.len() as u32,
        allow_unsafe,
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

/// Encode one `PasteAbort` frame for the active paste.
pub fn encode_paste_abort(
    operation_id: u64,
) -> Result<TerminalInputFrame, TerminalInputEncodeError> {
    encode_terminal_input(&TerminalInputCommand::PasteAbort { operation_id })
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
    #[error("terminal input body is truncated for kind {kind:?}")]
    TruncatedBody {
        /// Kind being decoded.
        kind: TerminalInputKind,
    },
    /// A fixed body has the wrong length.
    #[error("terminal input body length for {kind:?} must be {expected}, got {actual}")]
    BodyLength {
        /// Kind being decoded.
        kind: TerminalInputKind,
        /// Required body length.
        expected: usize,
        /// Observed body length.
        actual: usize,
    },
    /// An enum or flag field holds an unpublished value.
    #[error("unsupported {field} value {found}")]
    UnknownValue {
        /// Field name.
        field: &'static str,
        /// Observed value.
        found: u32,
    },
    /// Key text is not UTF-8.
    #[error("key text is not UTF-8")]
    InvalidText,
}
