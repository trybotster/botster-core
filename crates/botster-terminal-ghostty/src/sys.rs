//! Minimal libghostty-vt declarations used only by the feature-gated linkage
//! smoke test.

use std::ffi::{c_int, c_void};

/// Result code returned by libghostty-vt APIs.
pub type GhosttyResult = c_int;

/// Successful libghostty-vt result code.
pub const GHOSTTY_SUCCESS: GhosttyResult = 0;

/// Opaque terminal handle owned by libghostty-vt.
pub type GhosttyTerminal = *mut c_void;

/// Terminal initialization options for `ghostty_terminal_new`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GhosttyTerminalOptions {
    /// Terminal width in cells.
    pub cols: u16,
    /// Terminal height in cells.
    pub rows: u16,
    /// Maximum scrollback lines retained by Ghostty.
    pub max_scrollback: usize,
}

unsafe extern "C" {
    /// Create a new libghostty-vt terminal.
    pub fn ghostty_terminal_new(
        allocator: *const c_void,
        terminal: *mut GhosttyTerminal,
        options: GhosttyTerminalOptions,
    ) -> GhosttyResult;

    /// Free a libghostty-vt terminal.
    pub fn ghostty_terminal_free(terminal: GhosttyTerminal);

    /// Feed VT bytes to a libghostty-vt terminal.
    pub fn ghostty_terminal_vt_write(terminal: GhosttyTerminal, data: *const u8, len: usize);
}
