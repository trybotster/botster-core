//! Hub-safe terminal protocol: scheme 2 stream frames, scheme 2 input frames,
//! route identity, key and mouse vocabulary, and compatibility descriptors.
//!
//! This crate is the only terminal-protocol Rust surface Hub adapters may
//! depend on. Hub forwards [`TerminalFrame`] bytes inside a
//! [`RoutedTerminalFrame`] and validates only the [`TerminalInputFrame`]
//! header. Semantic client commands and event decoding live in
//! `botster-terminal-protocol-client`; Hub must not depend on that crate.
//!
//! Public items are the allowlist below. Adding a public name is a contract
//! change.

mod capabilities;
mod codec;
mod compatibility;
mod frame;
mod input_frame;
mod keys;
mod requests;
mod route;

pub use capabilities::{TerminalCapabilitySet, TerminalCapabilitySetError};
pub use codec::{
    decode_attach_state, decode_history_unavailable, decode_input_result, decode_modes,
    decode_process_exit, decode_route_resync, encode_attach_state, encode_history_unavailable,
    encode_input_result, encode_modes, encode_output, encode_process_exit, encode_route_resync,
    encode_snapshot_finish, encode_snapshot_history, encode_snapshot_ready, mode_bits,
    AttachStateCode, HistoryUnavailableReason, InputOutcome, InputResultBody, ModesBody,
    ProcessExitBody, RouteResyncBody, TerminalBodyError, ATTACH_STATE_BODY_BYTES,
    HISTORY_UNAVAILABLE_BODY_BYTES, INPUT_RESULT_PREFIX_BYTES, MAX_INPUT_RESULT_DETAIL_BYTES,
    MODES_BODY_BYTES, PROCESS_EXIT_BODY_BYTES, ROUTE_RESYNC_BODY_BYTES,
};
pub use compatibility::{
    ensure_compatible, TerminalCompatibility, TerminalCompatibilityError,
    TerminalCompatibilityRequirement,
};
pub use frame::{
    TerminalFrame, TerminalFrameError, TerminalKind, MAX_ROUTE_EGRESS_BYTES,
    MAX_ROUTE_EGRESS_FRAMES, MAX_TERMINAL_BODY_BYTES, TERMINAL_BODY_HEADER_BYTES,
    TERMINAL_STREAM_SCHEME_VERSION,
};
pub use input_frame::{
    TerminalInputFrame, TerminalInputFrameError, TerminalInputKind, FOCUS_BODY_BYTES,
    INPUT_HEADER_BYTES, KEY_PREFIX_BYTES, MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION,
    MAX_ENCODED_INPUT_BYTES, MAX_INPUT_OPERATIONS_PER_CLIENT, MAX_INPUT_OPERATIONS_PER_SESSION,
    MAX_PASTE_BYTES, MAX_PASTE_CHUNKS, MAX_PASTE_CHUNK_DATA_BYTES, MAX_RAW_INPUT_BYTES,
    MAX_RETAINED_INPUT_BYTES_PER_CLIENT, MAX_RETAINED_INPUT_BYTES_PER_SESSION,
    MAX_TERMINAL_INPUT_BODY_BYTES, MAX_TERMINAL_INPUT_FRAME_BYTES, MOUSE_BODY_BYTES,
    PASTE_ABORT_BODY_BYTES, PASTE_BEGIN_BODY_BYTES, PASTE_CHUNK_PREFIX_BYTES,
    PASTE_COMMIT_BODY_BYTES, RESIZE_BODY_BYTES, TERMINAL_INPUT_SCHEME_VERSION,
};
pub use keys::{
    terminal_mods, TerminalKey, TerminalKeyAction, TerminalMouseAction, TerminalMouseButton,
};
pub use requests::{Attach, Detach, Resize, SendInput};
pub use route::{RouteId, RouteIdError, RoutedTerminalFrame, MAX_ROUTE_ID_BYTES};

/// Independent terminal protocol name. Not a Hub daemon revision.
pub const PROTOCOL: &str = "botster-terminal-v2";
/// Exact protocol version for this plane.
pub const PROTOCOL_VERSION: u16 = 2;
/// Current terminal-plane conformance fixture revision.
pub const CONFORMANCE_FIXTURE_REVISION: u16 = 4;
/// Oldest terminal-plane conformance revision the default requirement accepts.
pub const DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION: u16 = 4;
/// Live terminal streaming feature token.
pub const FEATURE_TERMINAL_STREAMING: &str = "terminal_streaming";
/// Resize request feature token.
pub const FEATURE_RESIZE: &str = "resize";
/// READY-then-history snapshot delivery token. Always advertised on scheme 2.
pub const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY: &str =
    "snapshot_delivery=ready_then_history";
/// Required duplex opaque-byte transport token.
pub const FEATURE_TRANSPORT_DUPLEX_BINARY: &str = "transport=duplex_binary";

/// Published public item names for the Hub-consumable crate.
///
/// Tests compare this list to every public item in `src/`.
pub const PUBLIC_API_ALLOWLIST: &[&str] = &[
    "PROTOCOL",
    "PROTOCOL_VERSION",
    "CONFORMANCE_FIXTURE_REVISION",
    "DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION",
    "FEATURE_TERMINAL_STREAMING",
    "FEATURE_RESIZE",
    "FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY",
    "FEATURE_TRANSPORT_DUPLEX_BINARY",
    "PUBLIC_API_ALLOWLIST",
    "TERMINAL_STREAM_SCHEME_VERSION",
    "TERMINAL_BODY_HEADER_BYTES",
    "MAX_ROUTE_EGRESS_FRAMES",
    "MAX_ROUTE_EGRESS_BYTES",
    "MAX_TERMINAL_BODY_BYTES",
    "TerminalKind",
    "TerminalFrame",
    "TerminalFrameError",
    "MAX_ROUTE_ID_BYTES",
    "RouteId",
    "RouteIdError",
    "RoutedTerminalFrame",
    "PROCESS_EXIT_BODY_BYTES",
    "MODES_BODY_BYTES",
    "ATTACH_STATE_BODY_BYTES",
    "HISTORY_UNAVAILABLE_BODY_BYTES",
    "INPUT_RESULT_PREFIX_BYTES",
    "MAX_INPUT_RESULT_DETAIL_BYTES",
    "mode_bits",
    "AttachStateCode",
    "HistoryUnavailableReason",
    "InputOutcome",
    "ModesBody",
    "ProcessExitBody",
    "InputResultBody",
    "TerminalBodyError",
    "encode_output",
    "encode_snapshot_ready",
    "encode_snapshot_history",
    "encode_snapshot_finish",
    "encode_route_resync",
    "decode_route_resync",
    "RouteResyncBody",
    "ROUTE_RESYNC_BODY_BYTES",
    "encode_process_exit",
    "encode_modes",
    "encode_attach_state",
    "encode_history_unavailable",
    "encode_input_result",
    "decode_process_exit",
    "decode_modes",
    "decode_attach_state",
    "decode_history_unavailable",
    "decode_input_result",
    "TERMINAL_INPUT_SCHEME_VERSION",
    "INPUT_HEADER_BYTES",
    "MAX_TERMINAL_INPUT_BODY_BYTES",
    "MAX_TERMINAL_INPUT_FRAME_BYTES",
    "MAX_RAW_INPUT_BYTES",
    "KEY_PREFIX_BYTES",
    "MOUSE_BODY_BYTES",
    "FOCUS_BODY_BYTES",
    "RESIZE_BODY_BYTES",
    "PASTE_BEGIN_BODY_BYTES",
    "PASTE_CHUNK_PREFIX_BYTES",
    "MAX_PASTE_CHUNK_DATA_BYTES",
    "PASTE_COMMIT_BODY_BYTES",
    "PASTE_ABORT_BODY_BYTES",
    "MAX_PASTE_BYTES",
    "MAX_PASTE_CHUNKS",
    "MAX_INPUT_OPERATIONS_PER_SESSION",
    "MAX_RETAINED_INPUT_BYTES_PER_SESSION",
    "MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION",
    "MAX_INPUT_OPERATIONS_PER_CLIENT",
    "MAX_RETAINED_INPUT_BYTES_PER_CLIENT",
    "MAX_ENCODED_INPUT_BYTES",
    "TerminalInputKind",
    "TerminalInputFrame",
    "TerminalInputFrameError",
    "TerminalKey",
    "TerminalKeyAction",
    "TerminalMouseAction",
    "TerminalMouseButton",
    "terminal_mods",
    "TerminalCapabilitySet",
    "TerminalCapabilitySetError",
    "TerminalCompatibility",
    "TerminalCompatibilityError",
    "TerminalCompatibilityRequirement",
    "ensure_compatible",
    "Attach",
    "Detach",
    "SendInput",
    "Resize",
];
