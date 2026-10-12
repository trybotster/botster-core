//! The route frames of A.4 in their decoded form. Each frame has a `frame` tag, and its body fields sit beside it.

use crate::bytes::{Op, Payload};
use crate::input::Input;
use crate::types::{
    CloseReason, HistoryState, ModeFlags, QueryKind, RefusalReason, ResyncReason, RouteLimits,
};
use bytes::Bytes;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The route features that a worker can offer in `attached.features` (the intersection with the attach's `route_features`).
///
/// Clause: Codec A.4 (`0x0E output_deflate`, `0x0F output_dropped`), TS-1, TS-4c.
pub const ROUTE_FEATURES: &[&str] = &["output_dropped", "output_deflate"];

/// Body of `0x01 attached`.
///
/// Clause: Codec A.4, TS-1, TS-4.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attached {
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<String>(), 0..3)))]
    pub features: Vec<String>,
    pub terminal_format: String,
    pub limits: RouteLimits,
}

/// Body of `0x02 baseline_begin`.
///
/// Clause: Codec A.4, TS-4.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaselineBegin {
    pub rows: u32,
    pub cols: u32,
    pub modes: ModeFlags,
}

/// Body of `0x05 baseline_end`.
///
/// Clause: Codec A.4, TS-4, A.1.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaselineEnd {
    pub history: HistoryState,
}

/// Body of `0x08 modes`: the modes and the size that the worker applied.
///
/// Clause: Codec A.4, TS-4d.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Modes {
    pub modes: ModeFlags,
    pub rows: u32,
    pub cols: u32,
}

/// Body of `0x09 resync`.
///
/// Clause: Codec A.4, TS-4c.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Resync {
    pub reason: ResyncReason,
}

/// Body of `0x0A input_refused`.
///
/// Clause: Codec A.4, TS-5b, TS-5f.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputRefused {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<Op>,
    pub reason: RefusalReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, strategy(proptest::option::of(crate::props::wire_int())))]
    pub written_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Body of `0x0D input_done`.
///
/// Clause: Codec A.4, TS-5b, TS-5f.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputDone {
    pub op: Op,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub written_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Body of `0x0C terminal_query`.
///
/// Clause: Codec A.4, P9.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalQuery {
    /// Opaque to a client, which must not depend on its form (Core EV-8). The worker mints it as a u64 and sends it as a decimal
    /// string, by the rule for a u64 identifier (Codec A.1, Core A19-1). So its widest value is `18446744073709551615`.
    ///
    /// Clause: Core EV-8, Core A19-1, Codec A.1.
    pub query_id: String,
    #[cfg_attr(test, strategy(crate::props::base64_text()))]
    pub request_bytes_base64: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<QueryKind>,
    pub shadow_answerable: bool,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub answer_within_ms: u64,
}

/// The source of a notification in `output_dropped`.
///
/// Clause: Codec A.4, TS-4c.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSource {
    Bell,
    Osc9,
    Osc777,
}

/// The last notification that passed the policy, in `output_dropped`. An empty `title` or `body` is omitted, never sent as `""`.
///
/// Clause: Codec A.4, TS-4c.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DroppedNotification {
    pub source: NotificationSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

/// Body of `0x0F output_dropped`: sent only when `notifications` is at least 1, and before the `resync`.
///
/// Clause: Codec A.4, TS-4c.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OutputDropped {
    #[cfg_attr(test, strategy(1..=crate::strict::MAX_SAFE_INTEGER as u64))]
    pub notifications: u64,
    pub last: DroppedNotification,
}

/// The `exit` beside `session_ended` in `route_closed`.
///
/// Clause: Codec A.4, TS-8.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Exit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i32>,
}

/// Body of `0x0B route_closed`. `exit` is present only when the reason is `session_ended`.
///
/// Clause: Codec A.4, TS-8.
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteClosed {
    pub reason: CloseReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<Exit>,
}

/// The inflated payload of an `output_deflate` frame, kept by the decoder that had to inflate the stream to validate it,
/// so a receiver does not inflate again. It is a cache and not a part of the frame: it is never serialized, and two frames are equal
/// whether or not they hold it.
///
/// Clause: Codec A.4 (0x0E).
#[derive(Debug, Clone, Default)]
pub struct Inflated(Option<Bytes>);

impl Inflated {
    /// Clause: Codec A.4 (0x0E).
    pub fn new(bytes: Bytes) -> Self {
        Inflated(Some(bytes))
    }
}

impl PartialEq for Inflated {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Inflated {}

/// A frame from the worker to the client (A.4, Hub → client).
///
/// Clause: Codec A.4, TS-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum ToClient {
    Attached(Attached),
    BaselineBegin(BaselineBegin),
    Screen {
        payload: Payload,
    },
    History {
        payload: Payload,
    },
    BaselineEnd(BaselineEnd),
    Live,
    Output {
        payload: Payload,
    },
    OutputDeflate {
        // The raw DEFLATE stream, as it travels. (A plain comment: a doc comment would add a description to the schema.)
        payload: Payload,
        /// What a decoder inflated to validate the stream (A.4 0x0E). Not part of the frame.
        #[serde(skip)]
        #[schemars(skip)]
        inflated: Inflated,
    },
    Modes(Modes),
    Resync(Resync),
    InputRefused(InputRefused),
    InputDone(InputDone),
    TerminalQuery(TerminalQuery),
    OutputDropped(OutputDropped),
    RouteClosed(RouteClosed),
}

/// A frame from the client to the worker (A.4, Client → Hub). In a binary frame, `op` 0 means "no op".
///
/// Clause: Codec A.4, A.5, TS-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum ToWorker {
    Input(Input),
    PasteChunk { op: Op, bytes: Payload },
    Bytes { op: Op, bytes: Payload },
    Text { op: Op, text: String },
    FileChunk { op: Op, bytes: Payload },
}

impl ToClient {
    /// An `output_deflate` frame of a DEFLATE stream that the caller made. It holds no inflated cache.
    ///
    /// Clause: Codec A.4 (0x0E).
    pub fn output_deflate(stream: Payload) -> ToClient {
        ToClient::OutputDeflate {
            payload: stream,
            inflated: Inflated::default(),
        }
    }

    /// The inflated payload of a decoded `output_deflate` frame: the bytes that the decoder inflated to validate the stream.
    /// `None` for another frame, and for an `output_deflate` frame that was built and not decoded.
    ///
    /// Clause: Codec A.4 (0x0E), BP-2.
    pub fn inflated(&self) -> Option<&Bytes> {
        match self {
            ToClient::OutputDeflate { inflated, .. } => inflated.0.as_ref(),
            _ => None,
        }
    }
}

impl OutputDropped {
    /// The size of the smallest `output_dropped` frame that a route can need: the type byte and the body with the
    /// largest count, the longest source, `truncated: true`, and no title or body. The count has as many digits as `route_queue_bytes`.
    ///
    /// Clause: Codec TS-4c.
    pub fn min_frame_bytes(route_queue_bytes: u64) -> u64 {
        let digits = route_queue_bytes.to_string().len();
        1 + format!(
            r#"{{"notifications":{},"last":{{"source":"osc777","truncated":true}}}}"#,
            "9".repeat(digits)
        )
        .len() as u64
    }

    /// The frame, shortened to fit `max_frame` bytes (the type byte counts): `body` first, then `title`, at a character boundary, with `truncated: true`.
    /// An empty title or body is omitted. `count` and `source` are never cut.
    ///
    /// Clause: Codec TS-4c.
    pub fn fit(
        count: u64,
        source: NotificationSource,
        title: &str,
        body: &str,
        max_frame: u64,
    ) -> OutputDropped {
        let make = |title: &str, body: &str, truncated: bool| OutputDropped {
            notifications: count,
            last: DroppedNotification {
                source,
                title: (!title.is_empty()).then(|| title.to_string()),
                body: (!body.is_empty()).then(|| body.to_string()),
                truncated: truncated.then_some(true),
            },
        };
        let size = |o: &OutputDropped| ToClient::OutputDropped(o.clone()).encode().len() as u64;
        let full = make(title, body, false);
        if size(&full) <= max_frame {
            return full;
        }
        let (mut title, mut body) = (title.to_string(), body.to_string());
        while !body.is_empty() && size(&make(&title, &body, true)) > max_frame {
            body.pop();
        }
        while !title.is_empty() && size(&make(&title, &body, true)) > max_frame {
            title.pop();
        }
        make(&title, &body, true)
    }
}
