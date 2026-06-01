//! Ghostty shadow-terminal adapter boundary for Botster hosts.
//!
//! `botster-core` intentionally keeps the reusable terminal screen contract
//! backend-neutral. This crate is the home for Botster's blessed core-side
//! Ghostty shadow-terminal path: the future concrete adapter that owns
//! authoritative terminal screen and snapshot truth for tmux-like attach,
//! detach, recovery, and replay behavior.
//!
//! The public surface here is scaffold-only. It documents the crate boundary
//! and ties future Ghostty work to [`TerminalScreenRuntime`] without exposing a
//! fake runtime that returns placeholder behavior.
//!
//! Enabling the `libghostty-vt` feature builds the pinned trybotster Ghostty
//! fork from `vendor/ghostty` and links its static `libghostty-vt` archive.
//! Default builds leave that native path disabled so workspace tests do not
//! require Ghostty or Zig.
//!
//! restty remains a web/client rendering path. Clients may consume terminal
//! state and streams, but restty must not become core shadow-terminal
//! infrastructure or the authoritative parser/snapshot owner.
//!
//! ```
//! use botster_core::contract::terminal_screen::{
//!     TerminalOutputChunk, TerminalScreenSize, TerminalScreenState,
//!     TerminalSnapshotPayload,
//! };
//! use botster_core::engine::TerminalScreenRuntime;
//! use botster_terminal_ghostty::{GhosttyAdapterConfig, GhosttyTerminalRuntime};
//!
//! struct AdapterShape;
//!
//! impl TerminalScreenRuntime for AdapterShape {
//!     fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
//!         TerminalOutputChunk::new(bytes.to_vec())
//!     }
//!
//!     fn resize(&mut self, _size: TerminalScreenSize) {}
//!
//!     fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
//!         TerminalSnapshotPayload::new(
//!             Vec::new(),
//!             TerminalScreenSize::new(24, 80),
//!             Some(GhosttyAdapterConfig::default().snapshot_format().to_owned()),
//!         )
//!     }
//!
//!     fn replay_snapshot(&mut self, _payload: TerminalSnapshotPayload) {}
//!
//!     fn screen_state(&self) -> TerminalScreenState {
//!         TerminalScreenState::new(TerminalScreenSize::new(24, 80), String::new())
//!     }
//! }
//!
//! fn accepts_ghostty_runtime<R: GhosttyTerminalRuntime>(_runtime: &R) {}
//!
//! accepts_ghostty_runtime(&AdapterShape);
//! ```

use botster_core::engine::TerminalScreenRuntime;

/// Minimal libghostty-vt FFI used by feature-gated native build tests.
#[cfg(feature = "libghostty-vt")]
pub mod sys;

/// Snapshot format label reserved for Ghostty-owned opaque snapshot payloads.
pub const GHOSTTY_SNAPSHOT_FORMAT: &str = "ghostty";

/// Configuration for a future Ghostty-backed terminal screen adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GhosttyAdapterConfig {
    snapshot_format: &'static str,
}

impl GhosttyAdapterConfig {
    /// Build the default Ghostty adapter configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            snapshot_format: GHOSTTY_SNAPSHOT_FORMAT,
        }
    }

    /// Return the host-owned snapshot format label for Ghostty payloads.
    #[must_use]
    pub const fn snapshot_format(self) -> &'static str {
        self.snapshot_format
    }
}

impl Default for GhosttyAdapterConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Marker for runtimes that implement Botster's Ghostty shadow-terminal path.
///
/// This trait deliberately adds no behavior beyond [`TerminalScreenRuntime`].
/// The authoritative runtime contract remains in `botster-core`; this crate
/// only names the concrete Ghostty adapter home.
pub trait GhosttyTerminalRuntime: TerminalScreenRuntime {}

impl<T> GhosttyTerminalRuntime for T where T: TerminalScreenRuntime {}
