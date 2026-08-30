//! Hub-safe types-only terminal protocol.
//!
//! This crate is the only terminal-protocol Rust surface Hub adapters may
//! depend on. It exports compatibility descriptors, request types Hub may
//! forward, and an opaque [`TerminalFrame`].
//!
//! Semantic Snapshot, phase, AttachState, TerminalOutput, and ProcessExit
//! types live in `botster-terminal-protocol-client`. Hub must not depend on
//! that crate.
//!
//! Public items are the allowlist below. Adding a public name is a contract
//! change.

mod capabilities;
mod compatibility;
mod frame;
mod input_frame;
mod requests;

pub use capabilities::{TerminalCapabilitySet, TerminalCapabilitySetError};
pub use compatibility::{
    ensure_compatible, TerminalCompatibility, TerminalCompatibilityError,
    TerminalCompatibilityRequirement,
};
pub use frame::{TerminalFrame, TerminalFrameError};
pub use input_frame::{
    TerminalInputFrame, TerminalInputFrameError, MAX_INPUT_DATA_BYTES, MAX_MODE_GATED_DATA_BYTES,
    MAX_TERMINAL_INPUT_BODY_BYTES, MAX_TERMINAL_INPUT_FRAME_BYTES, MODE_GATED_PREFIX_BYTES,
    RESIZE_BODY_BYTES, TERMINAL_INPUT_SCHEME_VERSION,
};
pub use requests::{Attach, Detach, Resize, SendInput};

/// Independent terminal protocol name. Not a Hub daemon revision.
pub const PROTOCOL: &str = "botster-terminal-v1";
/// Exact protocol version for this plane.
pub const PROTOCOL_VERSION: u16 = 1;
/// Current terminal-plane conformance fixture revision.
pub const CONFORMANCE_FIXTURE_REVISION: u16 = 2;
/// Oldest terminal-plane conformance revision the default requirement accepts.
pub const DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION: u16 = 1;
/// Live terminal streaming feature token.
pub const FEATURE_TERMINAL_STREAMING: &str = "terminal_streaming";
/// Resize request feature token.
pub const FEATURE_RESIZE: &str = "resize";
/// Optional READY-then-history snapshot delivery token.
pub const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY: &str =
    "snapshot_delivery=ready_then_history";
/// Advertised duplex opaque-byte transport token.
///
/// [`TerminalCompatibilityRequirement::for_duplex_binary_transport`] is the
/// explicit requirement constructor.
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
    "TERMINAL_INPUT_SCHEME_VERSION",
    "MAX_TERMINAL_INPUT_BODY_BYTES",
    "MAX_TERMINAL_INPUT_FRAME_BYTES",
    "MAX_INPUT_DATA_BYTES",
    "MAX_MODE_GATED_DATA_BYTES",
    "MODE_GATED_PREFIX_BYTES",
    "RESIZE_BODY_BYTES",
    "PUBLIC_API_ALLOWLIST",
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
    "TerminalFrame",
    "TerminalFrameError",
    "TerminalInputFrame",
    "TerminalInputFrameError",
];
