//! Semantic terminal protocol types for TUI and generated TypeScript.
//!
//! Hub must not depend on this crate. Hub adapters depend only on
//! `botster-terminal-protocol` and forward [`TerminalFrame`] bytes.

mod events;
mod input;
mod typescript;

pub use botster_terminal_protocol::{
    ensure_compatible, Attach, Detach, Resize, SendInput, TerminalCompatibility,
    TerminalCompatibilityError, TerminalCompatibilityRequirement, TerminalFrame,
    TerminalFrameError, TerminalInputFrame, TerminalInputFrameError, CONFORMANCE_FIXTURE_REVISION,
    DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION, FEATURE_RESIZE,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY, FEATURE_TERMINAL_STREAMING,
    FEATURE_TRANSPORT_DUPLEX_BINARY, MAX_INPUT_DATA_BYTES, MAX_MODE_GATED_DATA_BYTES,
    MAX_PASTE_BYTES, MAX_PASTE_CHUNKS, MAX_PASTE_CHUNK_DATA_BYTES, MAX_TERMINAL_INPUT_BODY_BYTES,
    MAX_TERMINAL_INPUT_FRAME_BYTES, MODE_GATED_PREFIX_BYTES, PASTE_ABORT_BODY_BYTES,
    PASTE_BEGIN_BODY_BYTES, PASTE_CHUNK_PREFIX_BYTES, PASTE_COMMIT_BODY_BYTES, PROTOCOL,
    PROTOCOL_VERSION, RESIZE_BODY_BYTES, TERMINAL_INPUT_SCHEME_VERSION,
};
pub use events::{
    AttachState, AttachStateKind, EnvelopeError, PayloadEncoding, ProcessExit, Snapshot,
    SnapshotPhase, TerminalEvent, TerminalInputKind, TerminalInputRejection, TerminalInputResult,
    TerminalModeFlags, TerminalOutput,
};
pub use input::{
    decode_terminal_input, encode_paste, encode_paste_abort, encode_terminal_input,
    TerminalInputCommand, TerminalInputDecodeError, TerminalInputEncodeError,
};
pub use typescript::terminal_protocol_typescript;
