//! What a received client frame is for the input facts (ST-7), and the record state of a client input that the worker
//! refuses or discards. Each mapping is one function (steward rulings R-101 Q2 and R-102); none has a catch-all.
//!
//! - **One client input, one `rev` step, one record.** A frame of `bytes`, `text`, a key, the mouse, focus or a query reply
//!   is one client input. A paste and a file are one client input each, received at their commit: their begin, chunks and
//!   abort are not input (R-102 items 1, 2 and 4), refused or not.
//! - **A refused input** gets its record at receipt and its `not_written{reason}` status update at once (R-101 Q2). The
//!   reason is the IN-2 reason, not the wire reason.

use super::facts::Content;
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::{
    HexBytes, Input, RefusalReason, ToWorker, TYPE_BYTES, TYPE_FILE_CHUNK, TYPE_INPUT,
    TYPE_PASTE_CHUNK, TYPE_TEXT,
};

/// A received client frame, for the facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Receipt {
    /// Not a client input: no `rev` step, no `Activity`, no record (R-102 item 2).
    Part,
    /// One client input. When it is refused at receipt, its record keeps this.
    Input(Kept),
}

/// What the record of a client input that is refused at receipt keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Kept {
    Payload(InputPayload),
    /// A refused `paste_commit` or `file_commit`: kind `paste`, empty payload (R-102 follow-up (a), (c)).
    EmptyPaste,
    /// An input whose payload the worker did not decode.
    Undecoded(InputKind),
}

impl Kept {
    pub(super) fn content(&self) -> Content<'_> {
        match self {
            Kept::Payload(payload) => Content::Payload(payload),
            Kept::EmptyPaste => Content::Empty(InputKind::Paste),
            Kept::Undecoded(kind) => Content::Undecoded(*kind),
        }
    }
}

/// A decoded client frame (R-102).
pub(super) fn of_frame(frame: &ToWorker) -> Receipt {
    match frame {
        ToWorker::Bytes { bytes, .. } => Receipt::Input(Kept::Payload(InputPayload::Bytes {
            bytes: HexBytes(bytes.0.to_vec()),
        })),
        ToWorker::Text { text, .. } => {
            Receipt::Input(Kept::Payload(InputPayload::Text { text: text.clone() }))
        }
        ToWorker::PasteChunk { .. } | ToWorker::FileChunk { .. } => Receipt::Part,
        ToWorker::Input(input) => of_input(input),
    }
}

fn of_input(input: &Input) -> Receipt {
    match input {
        Input::PasteBegin { .. }
        | Input::PasteAbort { .. }
        | Input::FileBegin { .. }
        | Input::FileAbort { .. } => Receipt::Part,
        Input::PasteCommit { .. } | Input::FileCommit { .. } => Receipt::Input(Kept::EmptyPaste),
        Input::Key { .. } => Receipt::Input(Kept::Undecoded(InputKind::Key)),
        Input::Mouse { .. } => Receipt::Input(Kept::Undecoded(InputKind::Mouse)),
        Input::Focus { .. } => Receipt::Input(Kept::Undecoded(InputKind::Focus)),
        Input::ReplyTerminalQuery { .. } => no_kind(),
        // DP-6: a resize is not PTY input (P5's question at the steward, with P3).
        Input::Resize { .. } => Receipt::Part,
    }
}

/// A frame that the codec refused before it gave a payload (`Unsupported`, `Invalid`): its frame type, and for an input
/// frame its `kind` (P5's mapping (1), at the steward).
pub(super) fn of_raw(frame: &[u8]) -> Receipt {
    match frame.first().copied() {
        Some(TYPE_BYTES) => Receipt::Input(Kept::Undecoded(InputKind::Bytes)),
        Some(TYPE_TEXT) => Receipt::Input(Kept::Undecoded(InputKind::Text)),
        Some(TYPE_PASTE_CHUNK | TYPE_FILE_CHUNK) => Receipt::Part,
        Some(TYPE_INPUT) => {
            let kind = serde_json::from_slice::<serde_json::Value>(&frame[1..])
                .ok()
                .and_then(|body| body.get("kind")?.as_str().map(str::to_string));
            match kind.as_deref() {
                Some("paste_begin" | "paste_abort" | "file_begin" | "file_abort" | "resize") => {
                    Receipt::Part
                }
                Some("paste_commit" | "file_commit") => Receipt::Input(Kept::EmptyPaste),
                Some("key") => Receipt::Input(Kept::Undecoded(InputKind::Key)),
                Some("mouse") => Receipt::Input(Kept::Undecoded(InputKind::Mouse)),
                Some("focus") => Receipt::Input(Kept::Undecoded(InputKind::Focus)),
                Some(_) | None => no_kind(),
            }
        }
        // The codec decodes no other client frame type into `Unsupported` or `Invalid`.
        Some(_) | None => no_kind(),
    }
}

/// A client input that has no ST-7 kind (a query reply, an unknown input kind): kind `bytes`, no payload (P5's mapping (1),
/// at the steward).
fn no_kind() -> Receipt {
    Receipt::Input(Kept::Undecoded(InputKind::Bytes))
}

/// The IN-2 reason of a client input that the worker refuses at receipt (steward ruling R-101 Q2; P5's mapping (2)). The
/// reasons that the receipt never decides are listed too: a write's own end (`failed`), and the reasons of query replies and
/// of other workers.
pub(super) fn refused(reason: &RefusalReason) -> NotWrittenReason {
    match reason {
        RefusalReason::NotWritable | RefusalReason::NotReady | RefusalReason::Refused => {
            NotWrittenReason::Refused
        }
        RefusalReason::TooLarge => NotWrittenReason::PayloadTooLarge,
        RefusalReason::ModePreconditionFailed => NotWrittenReason::ModePreconditionFailed,
        RefusalReason::Unsupported { .. } => NotWrittenReason::Unsupported {
            what: UnsupportedWhat::Other,
        },
        RefusalReason::SessionEnded => NotWrittenReason::SessionEnded,
        RefusalReason::Failed
        | RefusalReason::WriteFailed
        | RefusalReason::AlreadyReplied
        | RefusalReason::QueryExpired
        | RefusalReason::UnknownQuery
        | RefusalReason::WrongRoute
        | RefusalReason::InvalidInput
        | RefusalReason::Other => NotWrittenReason::Refused,
    }
}

/// The outcome of a received client input that waits in its route's queue when the route closes: it is discarded, and
/// nothing of it is written (P5's mapping (4), at the steward).
pub(super) fn discarded() -> WriteOutcome {
    WriteOutcome::Cancelled
}
