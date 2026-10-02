//! The C ABI of libghostty-vt: the functions and types that this crate calls, declared by hand from the headers of the
//! pinned Ghostty (`vendor/ghostty/include/ghostty/vt/`). Everything here is `unsafe` to call. The safe wrappers in the
//! rest of the crate state the invariant that each call relies on.
//!
//! Stolen-From: botster-core@72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c:crates/botster-terminal-ghostty/src/sys.rs (the
//! idea of a hand-declared `sys` module and its result codes; every declaration is written again against the pin).

#![allow(non_camel_case_types)]

use std::ffi::c_void;

/// `GhosttyResult`.
pub type Result = i32;
pub const SUCCESS: Result = 0;
pub const OUT_OF_MEMORY: Result = -1;
pub const INVALID_VALUE: Result = -2;
pub const OUT_OF_SPACE: Result = -3;
pub const NO_VALUE: Result = -4;

/// `GhosttyTerminal`: an opaque handle.
pub type Terminal = *mut c_void;

/// `GhosttyTerminalData` keys that this crate reads.
pub mod data {
    pub const COLS: i32 = 1;
    pub const ROWS: i32 = 2;
}

extern "C" {
    pub fn ghostty_terminal_new(allocator: *const c_void, terminal: *mut Terminal, cols: u16, rows: u16) -> Result;
    pub fn ghostty_terminal_free(terminal: Terminal);
    pub fn ghostty_terminal_resize(
        terminal: Terminal,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result;
    pub fn ghostty_terminal_vt_write(terminal: Terminal, data: *const u8, len: usize);
    pub fn ghostty_terminal_get(terminal: Terminal, data: i32, out: *mut c_void) -> Result;
}
