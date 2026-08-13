//! Semantic terminal protocol types for TUI and generated TypeScript.
//!
//! Hub must not depend on this crate. Hub adapters depend only on
//! `botster-terminal-protocol` and forward [`TerminalFrame`] bytes.

mod events;
mod typescript;

pub use botster_terminal_protocol::{
    ensure_compatible, Attach, Detach, Resize, SendInput, TerminalCompatibility,
    TerminalCompatibilityError, TerminalCompatibilityRequirement, TerminalFrame,
    TerminalFrameError, CONFORMANCE_FIXTURE_REVISION, DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION,
    FEATURE_RESIZE, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY, FEATURE_TERMINAL_STREAMING,
    PROTOCOL, PROTOCOL_VERSION,
};
pub use events::{
    AttachState, AttachStateKind, EnvelopeError, PayloadEncoding, ProcessExit, Snapshot,
    SnapshotPhase, TerminalEvent, TerminalOutput,
};
pub use typescript::terminal_protocol_typescript;
