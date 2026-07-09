//! Minimal handwritten libghostty-vt declarations used by the safe adapter.

use std::ffi::{c_int, c_void};

/// Result code returned by libghostty-vt APIs.
pub(crate) type GhosttyResult = c_int;

/// Successful libghostty-vt result code.
pub(crate) const GHOSTTY_SUCCESS: GhosttyResult = 0;

/// Opaque terminal handle owned by libghostty-vt.
pub(crate) type GhosttyTerminal = *mut c_void;

/// Opaque formatter handle owned by libghostty-vt.
pub(crate) type GhosttyFormatter = *mut c_void;

/// Terminal initialization options for `ghostty_terminal_new`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GhosttyTerminalOptions {
    /// Terminal width in cells.
    pub(crate) cols: u16,
    /// Terminal height in cells.
    pub(crate) rows: u16,
    /// Maximum scrollback page-allocation bytes retained by Ghostty.
    pub(crate) max_scrollback: usize,
}

/// Formatter output format.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GhosttyFormatterFormat {
    /// Plain text output.
    Plain = 0,
}

/// Extra per-screen formatter options.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GhosttyFormatterScreenExtra {
    pub(crate) size: usize,
    pub(crate) cursor: bool,
    pub(crate) style: bool,
    pub(crate) hyperlink: bool,
    pub(crate) protection: bool,
    pub(crate) kitty_keyboard: bool,
    pub(crate) charsets: bool,
}

/// Extra terminal formatter options.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GhosttyFormatterTerminalExtra {
    pub(crate) size: usize,
    pub(crate) palette: bool,
    pub(crate) modes: bool,
    pub(crate) scrolling_region: bool,
    pub(crate) tabstops: bool,
    pub(crate) pwd: bool,
    pub(crate) keyboard: bool,
    pub(crate) screen: GhosttyFormatterScreenExtra,
}

/// Terminal formatter options.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GhosttyFormatterTerminalOptions {
    pub(crate) size: usize,
    pub(crate) emit: GhosttyFormatterFormat,
    pub(crate) unwrap: bool,
    pub(crate) trim: bool,
    pub(crate) extra: GhosttyFormatterTerminalExtra,
}

unsafe extern "C" {
    /// Create a new libghostty-vt terminal.
    pub(crate) fn ghostty_terminal_new(
        allocator: *const c_void,
        terminal: *mut GhosttyTerminal,
        options: GhosttyTerminalOptions,
    ) -> GhosttyResult;

    /// Free a libghostty-vt terminal.
    pub(crate) fn ghostty_terminal_free(terminal: GhosttyTerminal);

    /// Resize a libghostty-vt terminal.
    pub(crate) fn ghostty_terminal_resize(
        terminal: GhosttyTerminal,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> GhosttyResult;

    /// Feed VT bytes to a libghostty-vt terminal.
    pub(crate) fn ghostty_terminal_vt_write(terminal: GhosttyTerminal, data: *const u8, len: usize);

    /// Export an opaque terminal snapshot into a Ghostty-allocated buffer.
    pub(crate) fn ghostty_terminal_snapshot_export(
        terminal: GhosttyTerminal,
        allocator: *const c_void,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> GhosttyResult;

    /// Import an opaque terminal snapshot previously exported by Ghostty.
    pub(crate) fn ghostty_terminal_snapshot_import(
        terminal: GhosttyTerminal,
        data: *const u8,
        data_len: usize,
    ) -> GhosttyResult;

    /// Create a formatter for a terminal's active screen.
    pub(crate) fn ghostty_formatter_terminal_new(
        allocator: *const c_void,
        formatter: *mut GhosttyFormatter,
        terminal: GhosttyTerminal,
        options: GhosttyFormatterTerminalOptions,
    ) -> GhosttyResult;

    /// Format terminal content into a Ghostty-allocated buffer.
    pub(crate) fn ghostty_formatter_format_alloc(
        formatter: GhosttyFormatter,
        allocator: *const c_void,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> GhosttyResult;

    /// Free a formatter.
    pub(crate) fn ghostty_formatter_free(formatter: GhosttyFormatter);

    /// Free a Ghostty-allocated buffer.
    pub(crate) fn ghostty_free(allocator: *const c_void, ptr: *mut u8, len: usize);
}
