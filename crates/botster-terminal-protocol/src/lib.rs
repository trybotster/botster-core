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

mod compatibility;
mod frame;
mod requests;

pub use compatibility::{
    ensure_compatible, TerminalCompatibility, TerminalCompatibilityError,
    TerminalCompatibilityRequirement,
};
pub use frame::{TerminalFrame, TerminalFrameError};
pub use requests::{Attach, Detach, Resize, SendInput};

/// Independent terminal protocol name. Not a Hub daemon revision.
pub const PROTOCOL: &str = "botster-terminal-v1";
/// Exact protocol version for this plane.
pub const PROTOCOL_VERSION: u16 = 1;
/// Current terminal-plane conformance fixture revision.
pub const CONFORMANCE_FIXTURE_REVISION: u16 = 1;
/// Oldest terminal-plane conformance revision the default requirement accepts.
pub const DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION: u16 = 1;
/// Live terminal streaming feature token.
pub const FEATURE_TERMINAL_STREAMING: &str = "terminal_streaming";
/// Resize request feature token.
pub const FEATURE_RESIZE: &str = "resize";
/// Optional READY-then-history snapshot delivery token.
pub const FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY: &str =
    "snapshot_delivery=ready_then_history";

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
    "PUBLIC_API_ALLOWLIST",
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
];
