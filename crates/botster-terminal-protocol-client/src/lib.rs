//! Semantic scheme 2 terminal protocol for TUI, Web codegen, and Core.
//!
//! Hub must not depend on this crate. Hub adapters depend only on
//! `botster-terminal-protocol` and forward [`TerminalFrame`] bytes.

mod events;
mod input;
mod typescript;

pub use botster_terminal_protocol::{
    decode_attach_state, decode_history_unavailable, decode_input_result, decode_modes,
    decode_process_exit, encode_attach_state, encode_history_unavailable, encode_input_result,
    encode_modes, encode_output, encode_process_exit, encode_route_resync, encode_snapshot_finish,
    encode_snapshot_history, encode_snapshot_ready, ensure_compatible, mode_bits, terminal_mods,
    Attach, AttachStateCode, Detach, HistoryUnavailableReason, InputOutcome, InputResultBody,
    ModesBody, ProcessExitBody, Resize, RouteId, RouteIdError, RoutedTerminalFrame, SendInput,
    TerminalBodyError, TerminalCompatibility, TerminalCompatibilityError,
    TerminalCompatibilityRequirement, TerminalFrame, TerminalFrameError, TerminalInputFrame,
    TerminalInputFrameError, TerminalInputKind, TerminalKey, TerminalKeyAction, TerminalKind,
    TerminalMouseAction, TerminalMouseButton, CONFORMANCE_FIXTURE_REVISION,
    DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION, FEATURE_RESIZE,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY, FEATURE_TERMINAL_STREAMING,
    FEATURE_TRANSPORT_DUPLEX_BINARY, FOCUS_BODY_BYTES, INPUT_HEADER_BYTES, KEY_PREFIX_BYTES,
    MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION, MAX_ENCODED_INPUT_BYTES,
    MAX_INPUT_OPERATIONS_PER_CLIENT, MAX_INPUT_OPERATIONS_PER_SESSION,
    MAX_INPUT_RESULT_DETAIL_BYTES, MAX_PASTE_BYTES, MAX_PASTE_CHUNKS, MAX_PASTE_CHUNK_DATA_BYTES,
    MAX_RAW_INPUT_BYTES, MAX_RETAINED_INPUT_BYTES_PER_CLIENT, MAX_RETAINED_INPUT_BYTES_PER_SESSION,
    MAX_ROUTE_EGRESS_BYTES, MAX_ROUTE_EGRESS_FRAMES, MAX_ROUTE_ID_BYTES, MAX_TERMINAL_BODY_BYTES,
    MAX_TERMINAL_INPUT_BODY_BYTES, MAX_TERMINAL_INPUT_FRAME_BYTES, MOUSE_BODY_BYTES,
    PASTE_ABORT_BODY_BYTES, PASTE_BEGIN_BODY_BYTES, PASTE_CHUNK_PREFIX_BYTES,
    PASTE_COMMIT_BODY_BYTES, PROTOCOL, PROTOCOL_VERSION, RESIZE_BODY_BYTES,
    TERMINAL_BODY_HEADER_BYTES, TERMINAL_INPUT_SCHEME_VERSION, TERMINAL_STREAM_SCHEME_VERSION,
};
pub use events::{
    decode_shared_terminal_body, decode_terminal_body, decode_terminal_event, TerminalEvent,
    TerminalEventError, TerminalModeFlags,
};
pub use input::{
    decode_terminal_input, encode_paste, encode_paste_abort, encode_terminal_input,
    TerminalInputCommand, TerminalInputDecodeError, TerminalInputEncodeError,
};
pub use typescript::terminal_protocol_typescript;
