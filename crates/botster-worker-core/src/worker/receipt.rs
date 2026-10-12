//! What a received client frame is for the input facts (ST-7), and the record state of a client input that the worker
//! refuses or discards. Each mapping is one function (steward rulings R-101 Q2, R-102 and R-103); none has a catch-all.
//!
//! - **One client input, one `rev` step, one record.** A frame of `bytes`, `text`, a key, the mouse, focus or a query reply
//!   is one client input. A paste and a file are one client input each, received at their commit: their begin, chunks and
//!   abort are not input (R-102 items 1, 2 and 4), refused or not. A frame that does not parse closes the route
//!   `BadFrame`: it is not input (R-103 (1)).
//! - **A refused input** gets its record at receipt and its `not_written{reason}` status update at once (R-101 Q2). The
//!   reason is the IN-2 reason, not the wire reason.
//! - **The record keeps what was sent** (R-103 (1)): the 5.1A fields of a key, the mouse or focus; the bytes of a query
//!   reply; the JSON body of a frame of an unknown kind. When no truthful payload exists (text that is not UTF-8, a key
//!   with a value that the contract types do not hold), the record has no payload, and `payload_truncated` stays false: it
//!   only means that a payload was cut.

use super::facts::Content;
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::{
    base64_decode, Decoded, FrameBounds, HexBytes, Input, RefusalReason, ToWorker, TYPE_BYTES,
    TYPE_FILE_CHUNK, TYPE_INPUT, TYPE_PASTE_CHUNK, TYPE_TEXT,
};
use serde_json::Value;

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
    /// An input of this kind with no truthful payload (R-103 (1)).
    NoPayload(InputKind),
}

impl Kept {
    pub(super) fn content(&self) -> Content<'_> {
        match self {
            Kept::Payload(payload) => Content::Payload(payload),
            Kept::EmptyPaste => Content::Empty(InputKind::Paste),
            Kept::NoPayload(kind) => Content::NoPayload(*kind),
        }
    }
}

/// What the codec's result for one client frame is for the facts (R-102, R-103). `raw` is the frame as received, its
/// type byte first.
pub(super) fn of_decoded(decoded: &Decoded<ToWorker>, raw: &[u8]) -> Receipt {
    match decoded {
        Decoded::Frame(frame) => of_frame(frame, raw),
        Decoded::Unsupported { .. } | Decoded::Invalid { .. } => of_raw(raw),
        Decoded::Ignored { .. } => Receipt::Part,
    }
}

/// What the receipt makes of one client frame, as the bolero harness `receipt_decoder` reads it (plan section 8 step
/// 9): `None` when the frame does not decode under `bounds` or is not input (R-102); else the record that the frame
/// keeps when it is refused at receipt, with cursor 0, `at` 0, a client source with no tags and `payload_cap` as
/// `input_event_payload_bytes`. `raw` is the frame as the route's reader gives it, its type byte first.
pub fn record_of_client_frame(
    raw: &[u8],
    bounds: &FrameBounds,
    payload_cap: usize,
) -> Option<InputRecord> {
    let decoded = ToWorker::decode(raw, bounds).ok()?;
    let Receipt::Input(kept) = of_decoded(&decoded, raw) else {
        return None;
    };
    let source = InputSource::Client {
        route_tag: None,
        owner: None,
    };
    Some(super::facts::record_of(
        InputCursor(0),
        source,
        0,
        kept.content(),
        payload_cap,
    ))
}

/// A decoded client frame (R-102, R-103). `raw` is the frame as received, its type byte first.
fn of_frame(frame: &ToWorker, raw: &[u8]) -> Receipt {
    let body = raw.get(1..).unwrap_or_default();
    match frame {
        ToWorker::Bytes { bytes, .. } => Receipt::Input(Kept::Payload(InputPayload::Bytes {
            bytes: HexBytes(bytes.0.to_vec()),
        })),
        ToWorker::Text { text, .. } => {
            Receipt::Input(Kept::Payload(InputPayload::Text { text: text.clone() }))
        }
        ToWorker::PasteChunk { .. } | ToWorker::FileChunk { .. } => Receipt::Part,
        ToWorker::Input(input) => of_input(input, body),
    }
}

fn of_input(input: &Input, body: &[u8]) -> Receipt {
    match input {
        Input::PasteBegin { .. }
        | Input::PasteAbort { .. }
        | Input::FileBegin { .. }
        | Input::FileAbort { .. } => Receipt::Part,
        Input::PasteCommit { .. } | Input::FileCommit { .. } => Receipt::Input(Kept::EmptyPaste),
        Input::Key { .. } => semantic(InputKind::Key, body),
        Input::Mouse { .. } => semantic(InputKind::Mouse, body),
        Input::Focus { .. } => semantic(InputKind::Focus, body),
        Input::ReplyTerminalQuery { .. } => reply(body),
        // DP-6: a resize is not PTY input (P5's question at the steward, with P3).
        Input::Resize { .. } => Receipt::Part,
    }
}

/// A frame that the codec refused before it gave a payload (`Unsupported`, `Invalid`): its frame type, and for an input
/// frame its `kind` (R-103 (1)). The codec gives these only for a frame that parses.
pub(super) fn of_raw(frame: &[u8]) -> Receipt {
    let body = frame.get(1..).unwrap_or_default();
    match frame.first().copied() {
        // A binary frame is the type byte, an 8-byte `op`, then its data (Codec A.4).
        Some(TYPE_BYTES) => Receipt::Input(Kept::Payload(InputPayload::Bytes {
            bytes: HexBytes(frame.get(9..).unwrap_or_default().to_vec()),
        })),
        // R-103 (1): text that is not UTF-8 has no truthful string.
        Some(TYPE_TEXT) => Receipt::Input(Kept::NoPayload(InputKind::Text)),
        Some(TYPE_PASTE_CHUNK | TYPE_FILE_CHUNK) => Receipt::Part,
        Some(TYPE_INPUT) => {
            let kind = serde_json::from_slice::<Value>(body)
                .ok()
                .and_then(|body| body.get("kind")?.as_str().map(str::to_string));
            match kind.as_deref() {
                Some("paste_begin" | "paste_abort" | "file_begin" | "file_abort" | "resize") => {
                    Receipt::Part
                }
                Some("paste_commit" | "file_commit") => Receipt::Input(Kept::EmptyPaste),
                Some("key") => semantic(InputKind::Key, body),
                Some("mouse") => semantic(InputKind::Mouse, body),
                Some("focus") => semantic(InputKind::Focus, body),
                Some("reply_terminal_query") => reply(body),
                // R-103 (1): an unknown kind. The codec gives `Unsupported` or `Invalid` only for a body with a `kind`.
                Some(_) | None => as_sent(body),
            }
        }
        // The codec decodes no other client frame type into `Unsupported` or `Invalid`.
        Some(_) | None => as_sent(body),
    }
}

/// A key, mouse or focus input: its 5.1A fields as sent, read from the frame's body with the contract's types. A body that
/// these types do not hold has no truthful payload (R-103 (1)).
fn semantic(kind: InputKind, body: &[u8]) -> Receipt {
    let payload = match kind {
        InputKind::Key => serde_json::from_slice(body).ok().map(InputPayload::Key),
        InputKind::Mouse => serde_json::from_slice(body).ok().map(InputPayload::Mouse),
        InputKind::Focus => serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|body| body.get("focused")?.as_bool())
            .map(|focused| InputPayload::Focus { focused }),
        _ => None,
    };
    Receipt::Input(payload.map_or(Kept::NoPayload(kind), Kept::Payload))
}

/// A `reply_terminal_query` (R-103 (1), R-105 Q2): kind `bytes`. An `encoded` reply's payload is the decoded bytes of its
/// `bytes_base64`: they are terminal bytes. Every other reply has no terminal bytes at receipt (the encoder makes them at
/// the write), so its payload is the JSON of its `reply` as received. The kind of a written client reply is P4a's (R-102
/// item 5).
fn reply(body: &[u8]) -> Receipt {
    let reply = serde_json::from_slice::<Value>(body).ok().and_then(|body| {
        let reply = body.get("reply")?;
        let encoded = reply
            .get("encoded")
            .and_then(|encoded| encoded.get("bytes_base64")?.as_str())
            .and_then(base64_decode);
        encoded.or_else(|| serde_json::to_vec(reply).ok())
    });
    match reply {
        Some(bytes) => Receipt::Input(Kept::Payload(InputPayload::Bytes {
            bytes: HexBytes(bytes),
        })),
        None => as_sent(body),
    }
}

/// An input with no ST-7 kind and no 5.1A field set (R-103 (1)): kind `bytes`, and its payload is the frame's body as
/// received.
fn as_sent(body: &[u8]) -> Receipt {
    Receipt::Input(Kept::Payload(InputPayload::Bytes {
        bytes: HexBytes(body.to_vec()),
    }))
}

/// The IN-2 reason of a client input that the worker refuses at receipt (steward rulings R-101 Q2 and R-103 (2)). The
/// reasons that the receipt never decides are listed too: a write's own end (`failed`), and the reasons of query replies and
/// of other workers.
pub(super) fn refused(reason: &RefusalReason) -> NotWrittenReason {
    match reason {
        RefusalReason::NotWritable | RefusalReason::NotReady | RefusalReason::Refused => {
            NotWrittenReason::Refused
        }
        RefusalReason::TooLarge => NotWrittenReason::PayloadTooLarge,
        RefusalReason::ModePreconditionFailed => NotWrittenReason::ModePreconditionFailed,
        RefusalReason::Unsupported { what } => NotWrittenReason::Unsupported {
            what: unsupported(what),
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

/// The `UnsupportedWhat` of a refusal at receipt (R-103 (2)): the variant of its cause when one exists. The codec names an
/// unknown named key `key`, and an unknown named key has no encoding (`named_key`). `Other` is for a cause with no
/// variant: an unknown kind (`kind`) or value, text that is not UTF-8 (`utf8`), and a frame that this worker does not
/// implement yet (`input`, `file_chunk`).
fn unsupported(what: &str) -> UnsupportedWhat {
    match what {
        "key" => UnsupportedWhat::NamedKey,
        _ => UnsupportedWhat::Other,
    }
}

/// The outcome of a received client input that waits in its route's queue when the route closes (steward ruling R-105 Q1):
/// nothing of it is written, and no cancel was issued, so it is `not_written`, never `cancelled`. When the session's end
/// closed the route, the reason is `session`, the IN-2 reason (`SessionEnded` or `Stopping`, as R-103 (2)). Every other
/// close is `Refused`.
pub(super) fn discarded(close: &RouteCloseReason, session: NotWrittenReason) -> WriteOutcome {
    let reason = match close {
        // `SessionLost` is the session's end too, though this worker does not close a route with it (the host reports it).
        RouteCloseReason::SessionEnded { .. }
        | RouteCloseReason::SessionRemoved
        | RouteCloseReason::SessionLost => session,
        // `Detached`, `Replaced`, `Revoked`, `PeerClosed`, `WriteFailed`, `BadFrame`, `BadPeer`, `NotConnected`,
        // `SnapshotTooLarge`, `StallTimeout`, `HandoffFailed`, `Other`, and a later reason of the non-exhaustive enum.
        _ => NotWrittenReason::Refused,
    };
    WriteOutcome::NotWritten(reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-105 Q1: a close by the session's end keeps the IN-2 reason; every other close is `Refused`.
    #[test]
    fn a_discarded_input_ends_with_the_reason_of_its_close() {
        let ended = RouteCloseReason::SessionEnded {
            exit: Exit {
                code: Some(0),
                signal: None,
                cause: ExitCause::Other,
            },
        };
        for close in [
            ended,
            RouteCloseReason::SessionRemoved,
            RouteCloseReason::SessionLost,
        ] {
            for session in [NotWrittenReason::SessionEnded, NotWrittenReason::Stopping] {
                assert_eq!(
                    discarded(&close, session),
                    WriteOutcome::NotWritten(session)
                );
            }
        }
        let other = [
            RouteCloseReason::Detached,
            RouteCloseReason::Replaced,
            RouteCloseReason::PeerClosed,
            RouteCloseReason::StallTimeout,
        ];
        for close in other {
            assert_eq!(
                discarded(&close, NotWrittenReason::Stopping),
                WriteOutcome::NotWritten(NotWrittenReason::Refused)
            );
        }
    }

    /// R-103 (1): a refused mouse or focus input keeps its 5.1A fields as sent.
    #[test]
    fn a_refused_mouse_or_focus_input_keeps_its_fields() {
        let frame = |body: Value| {
            let mut f = vec![TYPE_INPUT];
            f.extend(serde_json::to_vec(&body).unwrap());
            f
        };
        let mouse = serde_json::json!({
            "kind": "mouse", "action": "press", "button": "left", "row": 0, "col": 3, "mods": []
        });
        let Receipt::Input(Kept::Payload(InputPayload::Mouse(m))) = of_raw(&frame(mouse)) else {
            panic!("a mouse payload");
        };
        assert_eq!(serde_json::to_value(&m).unwrap()["col"], 3);
        assert!(matches!(
            of_raw(&frame(
                serde_json::json!({"kind": "focus", "focused": true})
            )),
            Receipt::Input(Kept::Payload(InputPayload::Focus { focused: true }))
        ));
    }
}
