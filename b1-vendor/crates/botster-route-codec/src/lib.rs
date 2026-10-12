//! The route codec of the Hub→client contract, revision 36 (`frozen/final6/route-codec-v1-extract-rev36.md`).
//!
//! The Hub contract owns this text and Core adopts it verbatim. This crate holds the frame types of A.4 with their type bytes,
//! the thin framing of TS-3, the JSON route bodies under the A.1 rules, the reason sets of A.2 and A.2b, the input shapes of A.5,
//! `ModeFlags` of A.6 and the `output_dropped` frame of TS-4c. Every doc comment cites clause ids and never copies contract prose.
//!
//! Frames have two forms. The **wire form** is `[u8 type][body]`, delimited by the binding (`wire`). The **decoded form** is a JSON
//! value that carries a `frame` tag and the body fields. Binary payloads in the decoded form are `{"$bytes_hex": "<hex>"}`.
//! Conformance transcripts and the generated schemas use the decoded form.

mod bytes;
mod error;
mod frames;
mod input;
mod types;
mod wire;

#[cfg(test)]
mod props;

/// The schema generation and the strict JSON parser, for the repo tooling and the conformance runners. Not part of the stable surface.
#[cfg(feature = "unstable")]
pub mod schema;
#[cfg(not(feature = "unstable"))]
#[allow(dead_code)]
mod schema;
#[cfg(feature = "unstable")]
pub mod strict;
#[cfg(not(feature = "unstable"))]
mod strict;

/// The names that a user of the codec needs.
pub mod prelude {
    pub use crate::bytes::{
        base64_decode, base64_encode, file_component, hex_decode, hex_encode, HexBytes, Op, Payload,
    };
    pub use crate::error::{DecodeDetail, DecodeError, ProtocolErrorCode};
    pub use crate::frames::{
        Attached, BaselineBegin, BaselineEnd, DroppedNotification, Exit, Inflated, InputDone,
        InputRefused, Modes, NotificationSource, OutputDropped, Resync, RouteClosed, TerminalQuery,
        ToClient, ToWorker, ROUTE_FEATURES,
    };
    pub use crate::input::{
        named_key_names, parse_input, ColorScheme, Input, Key, KeyEvent, Modifier, MouseAction,
        MouseButton, NamedKey, ParsedInput, QueryReply,
    };
    pub use crate::types::{
        AttachFailedReason, CloseReason, FrameBounds, HistoryIncomplete, HistoryState,
        HistoryUnavailable, LifecycleClass, ModeFlags, MouseEncoding, MouseTracking, QueryKind,
        RefusalReason, ResyncReason, RouteEndReason, RouteLimits, SessionState,
    };
    pub use crate::wire::{
        bound_of, chunk_wrap, deflate_output, inflate_output, stream_wrap, ChunkReader, Decoded,
        StreamReader, TYPE_ATTACHED, TYPE_BASELINE_BEGIN, TYPE_BASELINE_END, TYPE_BYTES,
        TYPE_FILE_CHUNK, TYPE_HISTORY, TYPE_INPUT, TYPE_INPUT_DONE, TYPE_INPUT_REFUSED, TYPE_LIVE,
        TYPE_MODES, TYPE_OUTPUT, TYPE_OUTPUT_DEFLATE, TYPE_OUTPUT_DROPPED, TYPE_PASTE_CHUNK,
        TYPE_RESYNC, TYPE_ROUTE_CLOSED, TYPE_SCREEN, TYPE_TERMINAL_QUERY, TYPE_TEXT,
    };
}
