//! Minimal handwritten libghostty-vt declarations used by the safe adapter.

use std::ffi::{c_int, c_void};

/// Result code returned by libghostty-vt APIs.
pub(crate) type GhosttyResult = c_int;

/// Successful libghostty-vt result code.
pub(crate) const GHOSTTY_SUCCESS: GhosttyResult = 0;

/// libghostty-vt reports that an optional value is not configured.
pub(crate) const GHOSTTY_NO_VALUE: GhosttyResult = -4;

/// Opaque terminal handle owned by libghostty-vt.
pub(crate) type GhosttyTerminal = *mut c_void;

/// Packed terminal mode: bits 0-14 are the value and bit 15 is the ANSI flag.
pub(crate) type GhosttyMode = u16;

/// Kitty keyboard protocol flags (`uint8_t`).
pub(crate) type GhosttyKittyKeyFlags = u8;

const fn ghostty_mode(value: u16, ansi: bool) -> GhosttyMode {
    (value & 0x7fff) | ((ansi as u16) << 15)
}

/// DECSET 1 application cursor keys (DECCKM).
pub(crate) const GHOSTTY_MODE_DECCKM: GhosttyMode = ghostty_mode(1, false);

/// DECSET 25 cursor visible (DECTCEM).
pub(crate) const GHOSTTY_MODE_CURSOR_VISIBLE: GhosttyMode = ghostty_mode(25, false);

/// DECSET 1000 normal mouse tracking.
pub(crate) const GHOSTTY_MODE_NORMAL_MOUSE: GhosttyMode = ghostty_mode(1000, false);

/// DECSET 1002 button-event mouse tracking.
pub(crate) const GHOSTTY_MODE_BUTTON_MOUSE: GhosttyMode = ghostty_mode(1002, false);

/// DECSET 1003 any-event mouse tracking.
pub(crate) const GHOSTTY_MODE_ANY_MOUSE: GhosttyMode = ghostty_mode(1003, false);

/// DECSET 1004 focus reporting.
pub(crate) const GHOSTTY_MODE_FOCUS_EVENT: GhosttyMode = ghostty_mode(1004, false);

/// DECSET 1006 SGR mouse encoding.
pub(crate) const GHOSTTY_MODE_SGR_MOUSE: GhosttyMode = ghostty_mode(1006, false);

/// DECSET 1047 alternate screen.
pub(crate) const GHOSTTY_MODE_ALT_SCREEN: GhosttyMode = ghostty_mode(1047, false);

/// DECSET 1049 alternate screen + save cursor + clear.
pub(crate) const GHOSTTY_MODE_ALT_SCREEN_SAVE: GhosttyMode = ghostty_mode(1049, false);

/// DECSET 2004 bracketed paste.
pub(crate) const GHOSTTY_MODE_BRACKETED_PASTE: GhosttyMode = ghostty_mode(2004, false);

/// Opaque formatter handle owned by libghostty-vt.
pub(crate) type GhosttyFormatter = *mut c_void;

/// Opaque snapshot decoder handle owned by libghostty-vt.
pub(crate) type GhosttySnapshotDecoder = *mut c_void;

/// Terminal option identifier accepted by `ghostty_terminal_set`.
pub(crate) type GhosttyTerminalOption = c_int;

/// Embedder userdata pointer passed to every effect callback.
pub(crate) const GHOSTTY_TERMINAL_OPT_USERDATA: GhosttyTerminalOption = 0;

/// Effect callback that receives PTY query responses.
pub(crate) const GHOSTTY_TERMINAL_OPT_WRITE_PTY: GhosttyTerminalOption = 1;

/// Default foreground color (`GhosttyColorRgb*`).
pub(crate) const GHOSTTY_TERMINAL_OPT_COLOR_FOREGROUND: GhosttyTerminalOption = 11;

/// Default background color (`GhosttyColorRgb*`).
pub(crate) const GHOSTTY_TERMINAL_OPT_COLOR_BACKGROUND: GhosttyTerminalOption = 12;

/// Default cursor color (`GhosttyColorRgb*`).
pub(crate) const GHOSTTY_TERMINAL_OPT_COLOR_CURSOR: GhosttyTerminalOption = 13;

/// Default 256-color palette (`GhosttyColorRgb[256]*`).
pub(crate) const GHOSTTY_TERMINAL_OPT_COLOR_PALETTE: GhosttyTerminalOption = 14;

/// Maximum scrollback page-allocation bytes retained by Ghostty (`size_t*`).
pub(crate) const GHOSTTY_TERMINAL_OPT_SCROLLBACK_MAX_BYTES: GhosttyTerminalOption = 27;

/// Maximum retained bytes of an unfinished VT sequence (`size_t*`).
///
/// This must be enabled before any `ghostty_terminal_vt_write` that can leave
/// the parser mid-sequence; otherwise `ghostty_snapshot_encode_alloc` rejects
/// the terminal with `GHOSTTY_INVALID_VALUE`.
pub(crate) const GHOSTTY_TERMINAL_OPT_CONTINUATION_MAX_BYTES: GhosttyTerminalOption = 31;

/// Terminal data identifier accepted by `ghostty_terminal_get`.
pub(crate) type GhosttyTerminalData = c_int;

/// Cursor visibility (`bool*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_CURSOR_VISIBLE: GhosttyTerminalData = 7;

/// Kitty keyboard protocol flags (`GhosttyKittyKeyFlags*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_KITTY_KEYBOARD_FLAGS: GhosttyTerminalData = 8;

/// Effective foreground color (`GhosttyColorRgb*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_COLOR_FOREGROUND: GhosttyTerminalData = 18;

/// Effective background color (`GhosttyColorRgb*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_COLOR_BACKGROUND: GhosttyTerminalData = 19;

/// Effective cursor color (`GhosttyColorRgb*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_COLOR_CURSOR: GhosttyTerminalData = 20;

/// Current palette including OSC overrides (`GhosttyColorRgb[256]*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_COLOR_PALETTE: GhosttyTerminalData = 21;

/// Query a single terminal mode (`GhosttyTerminalModeConfig*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_MODE: GhosttyTerminalData = 37;

/// RGB color layout frozen by libghostty-vt.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GhosttyColorRgb {
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

/// A terminal mode and its boolean value.
///
/// Upstream documents this layout as frozen. `mode` is the caller-provided
/// query input; `value` receives the current mode value on success.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GhosttyTerminalModeConfig {
    /// Mode to query.
    pub(crate) mode: GhosttyMode,
    /// Current value returned by the query.
    pub(crate) value: bool,
}

/// Callback type for `GHOSTTY_TERMINAL_OPT_WRITE_PTY`.
pub(crate) type GhosttyTerminalWritePtyFn = unsafe extern "C" fn(
    terminal: GhosttyTerminal,
    userdata: *mut c_void,
    data: *const u8,
    len: usize,
);

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
    /// Optional `GhosttySelection *` restricting output to a range.
    ///
    /// Null formats the entire screen. This field is not optional in the
    /// layout: `size` is `sizeof` of the whole struct, so omitting it makes
    /// Ghostty read the selection pointer out of bounds.
    pub(crate) selection: *const c_void,
}

unsafe extern "C" {
    /// Create a new libghostty-vt terminal.
    pub(crate) fn ghostty_terminal_new(
        allocator: *const c_void,
        terminal: *mut GhosttyTerminal,
        cols: u16,
        rows: u16,
    ) -> GhosttyResult;

    /// Configure a libghostty-vt terminal option.
    pub(crate) fn ghostty_terminal_set(
        terminal: GhosttyTerminal,
        option: GhosttyTerminalOption,
        value: *const c_void,
    ) -> GhosttyResult;

    /// Read typed data from a libghostty-vt terminal.
    pub(crate) fn ghostty_terminal_get(
        terminal: GhosttyTerminal,
        data: GhosttyTerminalData,
        out: *mut c_void,
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

    /// Encode a terminal snapshot into a Ghostty-allocated buffer.
    ///
    /// The buffer must be released with `ghostty_free` using the same
    /// allocator that produced it.
    pub(crate) fn ghostty_snapshot_encode_alloc(
        terminal: GhosttyTerminal,
        allocator: *const c_void,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> GhosttyResult;

    /// Create a one-shot snapshot decoder over a caller-owned buffer.
    pub(crate) fn ghostty_snapshot_decoder_new_buf(
        allocator: *const c_void,
        decoder: *mut GhosttySnapshotDecoder,
        ptr: *const u8,
        len: usize,
    ) -> GhosttyResult;

    /// Decode a complete snapshot into a newly created, caller-owned terminal.
    pub(crate) fn ghostty_snapshot_decoder_decode(
        decoder: GhosttySnapshotDecoder,
        terminal: *mut GhosttyTerminal,
    ) -> GhosttyResult;

    /// Free a snapshot decoder. This does not free a decoded terminal.
    pub(crate) fn ghostty_snapshot_decoder_free(decoder: GhosttySnapshotDecoder);

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
