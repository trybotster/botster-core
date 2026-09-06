//! Minimal handwritten libghostty-vt declarations used by the safe adapter.

use std::ffi::{c_char, c_int, c_void};

/// Result code returned by libghostty-vt APIs.
pub(crate) type GhosttyResult = c_int;

/// Successful libghostty-vt result code.
pub(crate) const GHOSTTY_SUCCESS: GhosttyResult = 0;

/// Operation failed due to an invalid value.
pub(crate) const GHOSTTY_INVALID_VALUE: GhosttyResult = -2;

/// Buffer capacity was too small for the requested write.
pub(crate) const GHOSTTY_OUT_OF_SPACE: GhosttyResult = -3;

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

/// Synchronous byte destination callback used by streaming snapshot encode.
pub(crate) type GhosttyWriterFn =
    unsafe extern "C" fn(userdata: *mut c_void, data: *const u8, len: usize) -> bool;

/// Byte destination passed by value to libghostty-vt.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GhosttyWriter {
    pub(crate) write: Option<GhosttyWriterFn>,
    pub(crate) userdata: *mut c_void,
}

/// Synchronous byte source callback used by incremental snapshot decode.
pub(crate) type GhosttyReaderFn = unsafe extern "C" fn(
    userdata: *mut c_void,
    buffer: *mut u8,
    capacity: usize,
    out_read: *mut usize,
) -> bool;

/// Byte source passed by value to libghostty-vt.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GhosttyReader {
    pub(crate) read: Option<GhosttyReaderFn>,
    pub(crate) userdata: *mut c_void,
}

/// Snapshot decoder option identifier.
pub(crate) type GhosttySnapshotDecoderOption = c_int;

/// Maximum accepted snapshot continuation bytes (`size_t*`).
pub(crate) const GHOSTTY_SNAPSHOT_DECODER_OPT_MAX_CONTINUATION_BYTES: GhosttySnapshotDecoderOption =
    0;

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

/// Terminal columns (`uint16_t*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_COLS: GhosttyTerminalData = 1;

/// Terminal rows (`uint16_t*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_ROWS: GhosttyTerminalData = 2;

/// Cursor visibility (`bool*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_CURSOR_VISIBLE: GhosttyTerminalData = 7;

/// Kitty keyboard protocol flags (`GhosttyKittyKeyFlags*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_KITTY_KEYBOARD_FLAGS: GhosttyTerminalData = 8;

/// Scrollbar state (`GhosttyTerminalScrollbar*`).
pub(crate) const GHOSTTY_TERMINAL_DATA_SCROLLBAR: GhosttyTerminalData = 9;

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

/// Scrollbar geometry returned by `GHOSTTY_TERMINAL_DATA_SCROLLBAR`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GhosttyTerminalScrollbar {
    pub(crate) total: u64,
    pub(crate) offset: u64,
    pub(crate) len: u64,
}

/// Scroll viewport behavior tag for `ghostty_terminal_scroll_viewport`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum GhosttyTerminalScrollViewportTag {
    Top = 0,
    Bottom = 1,
    Delta = 2,
    Row = 3,
}

/// Scroll viewport value payload.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) union GhosttyTerminalScrollViewportValue {
    pub(crate) delta: isize,
    pub(crate) row: usize,
    pub(crate) _padding: [u64; 2],
}

/// Tagged union for scroll viewport behavior.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GhosttyTerminalScrollViewport {
    pub(crate) tag: GhosttyTerminalScrollViewportTag,
    pub(crate) value: GhosttyTerminalScrollViewportValue,
}

/// Opaque cell value (`uint64_t`).
pub(crate) type GhosttyCell = u64;

/// Cell wide property.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum GhosttyCellWide {
    Narrow = 0,
    Wide = 1,
    SpacerTail = 2,
    SpacerHead = 3,
}

/// Cell data kind for `ghostty_cell_get`.
pub(crate) type GhosttyCellData = c_int;

/// Wide property of a cell (`GhosttyCellWide*`).
pub(crate) const GHOSTTY_CELL_DATA_WIDE: GhosttyCellData = 3;

/// Style color tags.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum GhosttyStyleColorTag {
    None = 0,
    Palette = 1,
    Rgb = 2,
}

/// Style color value union.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) union GhosttyStyleColorValue {
    pub(crate) palette: u8,
    pub(crate) rgb: GhosttyColorRgb,
    pub(crate) _padding: u64,
}

/// Tagged style color.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GhosttyStyleColor {
    pub(crate) tag: GhosttyStyleColorTag,
    pub(crate) value: GhosttyStyleColorValue,
}

/// Terminal cell style (sized struct).
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GhosttyStyle {
    pub(crate) size: usize,
    pub(crate) fg_color: GhosttyStyleColor,
    pub(crate) bg_color: GhosttyStyleColor,
    pub(crate) underline_color: GhosttyStyleColor,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) faint: bool,
    pub(crate) blink: bool,
    pub(crate) inverse: bool,
    pub(crate) invisible: bool,
    pub(crate) strikethrough: bool,
    pub(crate) overline: bool,
    pub(crate) underline: c_int,
}

/// Caller-provided byte buffer for grapheme UTF-8 export.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GhosttyBuffer {
    pub(crate) ptr: *mut u8,
    pub(crate) cap: usize,
    pub(crate) len: usize,
}

/// Opaque render state handle.
pub(crate) type GhosttyRenderState = *mut c_void;

/// Opaque render-state row iterator handle.
pub(crate) type GhosttyRenderStateRowIterator = *mut c_void;

/// Opaque render-state row cells handle.
pub(crate) type GhosttyRenderStateRowCells = *mut c_void;

/// Cursor visual style from render state.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum GhosttyRenderStateCursorVisualStyle {
    Bar = 0,
    Block = 1,
    Underline = 2,
    BlockHollow = 3,
}

/// Queryable render-state data kinds.
pub(crate) type GhosttyRenderStateData = c_int;

pub(crate) const GHOSTTY_RENDER_STATE_DATA_COLS: GhosttyRenderStateData = 1;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_ROWS: GhosttyRenderStateData = 2;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR: GhosttyRenderStateData = 4;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_COLOR_BACKGROUND: GhosttyRenderStateData = 5;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_COLOR_FOREGROUND: GhosttyRenderStateData = 6;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_CURSOR_VISUAL_STYLE: GhosttyRenderStateData = 10;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_CURSOR_VISIBLE: GhosttyRenderStateData = 11;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_HAS_VALUE: GhosttyRenderStateData = 14;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_X: GhosttyRenderStateData = 15;
pub(crate) const GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_Y: GhosttyRenderStateData = 16;

/// Queryable render-state row data kinds.
pub(crate) type GhosttyRenderStateRowData = c_int;

pub(crate) const GHOSTTY_RENDER_STATE_ROW_DATA_CELLS: GhosttyRenderStateRowData = 3;

/// Queryable render-state row-cell data kinds.
pub(crate) type GhosttyRenderStateRowCellsData = c_int;

pub(crate) const GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_RAW: GhosttyRenderStateRowCellsData = 1;
pub(crate) const GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE: GhosttyRenderStateRowCellsData = 2;
pub(crate) const GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_BG_COLOR: GhosttyRenderStateRowCellsData = 5;
pub(crate) const GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_FG_COLOR: GhosttyRenderStateRowCellsData = 6;
pub(crate) const GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_UTF8:
    GhosttyRenderStateRowCellsData = 9;

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
    /// Return the process-lifetime JSON manifest for the linked C ABI.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn ghostty_type_json() -> *const c_char;

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

    /// Scroll the terminal viewport.
    pub(crate) fn ghostty_terminal_scroll_viewport(
        terminal: GhosttyTerminal,
        behavior: GhosttyTerminalScrollViewport,
    );

    /// Query a single cell field.
    pub(crate) fn ghostty_cell_get(
        cell: GhosttyCell,
        data: GhosttyCellData,
        out: *mut c_void,
    ) -> GhosttyResult;

    /// Create an empty render state.
    pub(crate) fn ghostty_render_state_new(
        allocator: *const c_void,
        state: *mut GhosttyRenderState,
    ) -> GhosttyResult;

    /// Free a render state.
    pub(crate) fn ghostty_render_state_free(state: GhosttyRenderState);

    /// Update a render state from a terminal (begin+end convenience).
    pub(crate) fn ghostty_render_state_update(
        state: GhosttyRenderState,
        terminal: GhosttyTerminal,
    ) -> GhosttyResult;

    /// Query a single render-state field.
    pub(crate) fn ghostty_render_state_get(
        state: GhosttyRenderState,
        data: GhosttyRenderStateData,
        out: *mut c_void,
    ) -> GhosttyResult;

    /// Create a reusable row iterator handle.
    pub(crate) fn ghostty_render_state_row_iterator_new(
        allocator: *const c_void,
        out_iterator: *mut GhosttyRenderStateRowIterator,
    ) -> GhosttyResult;

    /// Free a row iterator.
    pub(crate) fn ghostty_render_state_row_iterator_free(iterator: GhosttyRenderStateRowIterator);

    /// Advance a row iterator.
    pub(crate) fn ghostty_render_state_row_iterator_next(
        iterator: GhosttyRenderStateRowIterator,
    ) -> bool;

    /// Query the current row in a row iterator.
    pub(crate) fn ghostty_render_state_row_get(
        iterator: GhosttyRenderStateRowIterator,
        data: GhosttyRenderStateRowData,
        out: *mut c_void,
    ) -> GhosttyResult;

    /// Create a reusable row-cells handle.
    pub(crate) fn ghostty_render_state_row_cells_new(
        allocator: *const c_void,
        out_cells: *mut GhosttyRenderStateRowCells,
    ) -> GhosttyResult;

    /// Advance a row-cells iterator.
    pub(crate) fn ghostty_render_state_row_cells_next(cells: GhosttyRenderStateRowCells) -> bool;

    /// Query the current cell in a row-cells iterator.
    pub(crate) fn ghostty_render_state_row_cells_get(
        cells: GhosttyRenderStateRowCells,
        data: GhosttyRenderStateRowCellsData,
        out: *mut c_void,
    ) -> GhosttyResult;

    /// Free a row-cells handle.
    pub(crate) fn ghostty_render_state_row_cells_free(cells: GhosttyRenderStateRowCells);

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

    /// Encode a terminal snapshot through a synchronous writer callback.
    pub(crate) fn ghostty_snapshot_encode(
        terminal: GhosttyTerminal,
        writer: GhosttyWriter,
    ) -> GhosttyResult;

    /// Create a one-shot snapshot decoder over a caller-owned buffer.
    pub(crate) fn ghostty_snapshot_decoder_new_buf(
        allocator: *const c_void,
        decoder: *mut GhosttySnapshotDecoder,
        ptr: *const u8,
        len: usize,
    ) -> GhosttyResult;

    /// Create an incremental decoder over a synchronous reader callback.
    pub(crate) fn ghostty_snapshot_decoder_new(
        allocator: *const c_void,
        decoder: *mut GhosttySnapshotDecoder,
        reader: GhosttyReader,
    ) -> GhosttyResult;

    /// Set an incremental decoder option before decoding starts.
    pub(crate) fn ghostty_snapshot_decoder_set(
        decoder: GhosttySnapshotDecoder,
        option: GhosttySnapshotDecoderOption,
        value: *const c_void,
    ) -> GhosttyResult;

    /// Decode and validate the renderable prefix through READY.
    pub(crate) fn ghostty_snapshot_decoder_ready(
        decoder: GhosttySnapshotDecoder,
        terminal: *mut GhosttyTerminal,
    ) -> GhosttyResult;

    /// Decode one history PAGE or validate FINISH.
    pub(crate) fn ghostty_snapshot_decoder_next(decoder: GhosttySnapshotDecoder) -> GhosttyResult;

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

// ---------------------------------------------------------------------------
// Input encoders: key, mouse, focus, paste (ghostty/vt/key, mouse, focus, paste)
// ---------------------------------------------------------------------------

/// Opaque key event handle owned by libghostty-vt.
pub(crate) type GhosttyKeyEvent = *mut c_void;
/// Opaque key encoder handle owned by libghostty-vt.
pub(crate) type GhosttyKeyEncoder = *mut c_void;
/// Opaque mouse event handle owned by libghostty-vt.
pub(crate) type GhosttyMouseEvent = *mut c_void;
/// Opaque mouse encoder handle owned by libghostty-vt.
pub(crate) type GhosttyMouseEncoder = *mut c_void;

/// `GhosttyKeyAction` (`int`).
pub(crate) type GhosttyKeyAction = c_int;
pub(crate) const GHOSTTY_KEY_ACTION_RELEASE: GhosttyKeyAction = 0;
pub(crate) const GHOSTTY_KEY_ACTION_PRESS: GhosttyKeyAction = 1;
pub(crate) const GHOSTTY_KEY_ACTION_REPEAT: GhosttyKeyAction = 2;

/// `GhosttyMods` (`uint16_t`). Bit layout matches `botster_terminal_protocol::terminal_mods`.
pub(crate) type GhosttyMods = u16;

/// `GhosttyKey` (`int`). Values follow `ghostty/vt/key/event.h` declaration order.
pub(crate) type GhosttyKey = c_int;
pub(crate) const GHOSTTY_KEY_UNIDENTIFIED: GhosttyKey = 0;
pub(crate) const GHOSTTY_KEY_BACKQUOTE: GhosttyKey = 1;
pub(crate) const GHOSTTY_KEY_BACKSLASH: GhosttyKey = 2;
pub(crate) const GHOSTTY_KEY_BRACKET_LEFT: GhosttyKey = 3;
pub(crate) const GHOSTTY_KEY_BRACKET_RIGHT: GhosttyKey = 4;
pub(crate) const GHOSTTY_KEY_COMMA: GhosttyKey = 5;
pub(crate) const GHOSTTY_KEY_DIGIT_0: GhosttyKey = 6;
pub(crate) const GHOSTTY_KEY_DIGIT_1: GhosttyKey = 7;
pub(crate) const GHOSTTY_KEY_DIGIT_2: GhosttyKey = 8;
pub(crate) const GHOSTTY_KEY_DIGIT_3: GhosttyKey = 9;
pub(crate) const GHOSTTY_KEY_DIGIT_4: GhosttyKey = 10;
pub(crate) const GHOSTTY_KEY_DIGIT_5: GhosttyKey = 11;
pub(crate) const GHOSTTY_KEY_DIGIT_6: GhosttyKey = 12;
pub(crate) const GHOSTTY_KEY_DIGIT_7: GhosttyKey = 13;
pub(crate) const GHOSTTY_KEY_DIGIT_8: GhosttyKey = 14;
pub(crate) const GHOSTTY_KEY_DIGIT_9: GhosttyKey = 15;
pub(crate) const GHOSTTY_KEY_EQUAL: GhosttyKey = 16;
pub(crate) const GHOSTTY_KEY_INTL_BACKSLASH: GhosttyKey = 17;
pub(crate) const GHOSTTY_KEY_INTL_RO: GhosttyKey = 18;
pub(crate) const GHOSTTY_KEY_INTL_YEN: GhosttyKey = 19;
pub(crate) const GHOSTTY_KEY_A: GhosttyKey = 20;
pub(crate) const GHOSTTY_KEY_B: GhosttyKey = 21;
pub(crate) const GHOSTTY_KEY_C: GhosttyKey = 22;
pub(crate) const GHOSTTY_KEY_D: GhosttyKey = 23;
pub(crate) const GHOSTTY_KEY_E: GhosttyKey = 24;
pub(crate) const GHOSTTY_KEY_F: GhosttyKey = 25;
pub(crate) const GHOSTTY_KEY_G: GhosttyKey = 26;
pub(crate) const GHOSTTY_KEY_H: GhosttyKey = 27;
pub(crate) const GHOSTTY_KEY_I: GhosttyKey = 28;
pub(crate) const GHOSTTY_KEY_J: GhosttyKey = 29;
pub(crate) const GHOSTTY_KEY_K: GhosttyKey = 30;
pub(crate) const GHOSTTY_KEY_L: GhosttyKey = 31;
pub(crate) const GHOSTTY_KEY_M: GhosttyKey = 32;
pub(crate) const GHOSTTY_KEY_N: GhosttyKey = 33;
pub(crate) const GHOSTTY_KEY_O: GhosttyKey = 34;
pub(crate) const GHOSTTY_KEY_P: GhosttyKey = 35;
pub(crate) const GHOSTTY_KEY_Q: GhosttyKey = 36;
pub(crate) const GHOSTTY_KEY_R: GhosttyKey = 37;
pub(crate) const GHOSTTY_KEY_S: GhosttyKey = 38;
pub(crate) const GHOSTTY_KEY_T: GhosttyKey = 39;
pub(crate) const GHOSTTY_KEY_U: GhosttyKey = 40;
pub(crate) const GHOSTTY_KEY_V: GhosttyKey = 41;
pub(crate) const GHOSTTY_KEY_W: GhosttyKey = 42;
pub(crate) const GHOSTTY_KEY_X: GhosttyKey = 43;
pub(crate) const GHOSTTY_KEY_Y: GhosttyKey = 44;
pub(crate) const GHOSTTY_KEY_Z: GhosttyKey = 45;
pub(crate) const GHOSTTY_KEY_MINUS: GhosttyKey = 46;
pub(crate) const GHOSTTY_KEY_PERIOD: GhosttyKey = 47;
pub(crate) const GHOSTTY_KEY_QUOTE: GhosttyKey = 48;
pub(crate) const GHOSTTY_KEY_SEMICOLON: GhosttyKey = 49;
pub(crate) const GHOSTTY_KEY_SLASH: GhosttyKey = 50;
pub(crate) const GHOSTTY_KEY_ALT_LEFT: GhosttyKey = 51;
pub(crate) const GHOSTTY_KEY_ALT_RIGHT: GhosttyKey = 52;
pub(crate) const GHOSTTY_KEY_BACKSPACE: GhosttyKey = 53;
pub(crate) const GHOSTTY_KEY_CAPS_LOCK: GhosttyKey = 54;
pub(crate) const GHOSTTY_KEY_CONTEXT_MENU: GhosttyKey = 55;
pub(crate) const GHOSTTY_KEY_CONTROL_LEFT: GhosttyKey = 56;
pub(crate) const GHOSTTY_KEY_CONTROL_RIGHT: GhosttyKey = 57;
pub(crate) const GHOSTTY_KEY_ENTER: GhosttyKey = 58;
pub(crate) const GHOSTTY_KEY_META_LEFT: GhosttyKey = 59;
pub(crate) const GHOSTTY_KEY_META_RIGHT: GhosttyKey = 60;
pub(crate) const GHOSTTY_KEY_SHIFT_LEFT: GhosttyKey = 61;
pub(crate) const GHOSTTY_KEY_SHIFT_RIGHT: GhosttyKey = 62;
pub(crate) const GHOSTTY_KEY_SPACE: GhosttyKey = 63;
pub(crate) const GHOSTTY_KEY_TAB: GhosttyKey = 64;
pub(crate) const GHOSTTY_KEY_CONVERT: GhosttyKey = 65;
pub(crate) const GHOSTTY_KEY_KANA_MODE: GhosttyKey = 66;
pub(crate) const GHOSTTY_KEY_NON_CONVERT: GhosttyKey = 67;
pub(crate) const GHOSTTY_KEY_DELETE: GhosttyKey = 68;
pub(crate) const GHOSTTY_KEY_END: GhosttyKey = 69;
pub(crate) const GHOSTTY_KEY_HELP: GhosttyKey = 70;
pub(crate) const GHOSTTY_KEY_HOME: GhosttyKey = 71;
pub(crate) const GHOSTTY_KEY_INSERT: GhosttyKey = 72;
pub(crate) const GHOSTTY_KEY_PAGE_DOWN: GhosttyKey = 73;
pub(crate) const GHOSTTY_KEY_PAGE_UP: GhosttyKey = 74;
pub(crate) const GHOSTTY_KEY_ARROW_DOWN: GhosttyKey = 75;
pub(crate) const GHOSTTY_KEY_ARROW_LEFT: GhosttyKey = 76;
pub(crate) const GHOSTTY_KEY_ARROW_RIGHT: GhosttyKey = 77;
pub(crate) const GHOSTTY_KEY_ARROW_UP: GhosttyKey = 78;
pub(crate) const GHOSTTY_KEY_NUM_LOCK: GhosttyKey = 79;
pub(crate) const GHOSTTY_KEY_NUMPAD_0: GhosttyKey = 80;
pub(crate) const GHOSTTY_KEY_NUMPAD_1: GhosttyKey = 81;
pub(crate) const GHOSTTY_KEY_NUMPAD_2: GhosttyKey = 82;
pub(crate) const GHOSTTY_KEY_NUMPAD_3: GhosttyKey = 83;
pub(crate) const GHOSTTY_KEY_NUMPAD_4: GhosttyKey = 84;
pub(crate) const GHOSTTY_KEY_NUMPAD_5: GhosttyKey = 85;
pub(crate) const GHOSTTY_KEY_NUMPAD_6: GhosttyKey = 86;
pub(crate) const GHOSTTY_KEY_NUMPAD_7: GhosttyKey = 87;
pub(crate) const GHOSTTY_KEY_NUMPAD_8: GhosttyKey = 88;
pub(crate) const GHOSTTY_KEY_NUMPAD_9: GhosttyKey = 89;
pub(crate) const GHOSTTY_KEY_NUMPAD_ADD: GhosttyKey = 90;
pub(crate) const GHOSTTY_KEY_NUMPAD_BACKSPACE: GhosttyKey = 91;
pub(crate) const GHOSTTY_KEY_NUMPAD_CLEAR: GhosttyKey = 92;
pub(crate) const GHOSTTY_KEY_NUMPAD_CLEAR_ENTRY: GhosttyKey = 93;
pub(crate) const GHOSTTY_KEY_NUMPAD_COMMA: GhosttyKey = 94;
pub(crate) const GHOSTTY_KEY_NUMPAD_DECIMAL: GhosttyKey = 95;
pub(crate) const GHOSTTY_KEY_NUMPAD_DIVIDE: GhosttyKey = 96;
pub(crate) const GHOSTTY_KEY_NUMPAD_ENTER: GhosttyKey = 97;
pub(crate) const GHOSTTY_KEY_NUMPAD_EQUAL: GhosttyKey = 98;
pub(crate) const GHOSTTY_KEY_NUMPAD_MEMORY_ADD: GhosttyKey = 99;
pub(crate) const GHOSTTY_KEY_NUMPAD_MEMORY_CLEAR: GhosttyKey = 100;
pub(crate) const GHOSTTY_KEY_NUMPAD_MEMORY_RECALL: GhosttyKey = 101;
pub(crate) const GHOSTTY_KEY_NUMPAD_MEMORY_STORE: GhosttyKey = 102;
pub(crate) const GHOSTTY_KEY_NUMPAD_MEMORY_SUBTRACT: GhosttyKey = 103;
pub(crate) const GHOSTTY_KEY_NUMPAD_MULTIPLY: GhosttyKey = 104;
pub(crate) const GHOSTTY_KEY_NUMPAD_PAREN_LEFT: GhosttyKey = 105;
pub(crate) const GHOSTTY_KEY_NUMPAD_PAREN_RIGHT: GhosttyKey = 106;
pub(crate) const GHOSTTY_KEY_NUMPAD_SUBTRACT: GhosttyKey = 107;
pub(crate) const GHOSTTY_KEY_NUMPAD_SEPARATOR: GhosttyKey = 108;
pub(crate) const GHOSTTY_KEY_NUMPAD_UP: GhosttyKey = 109;
pub(crate) const GHOSTTY_KEY_NUMPAD_DOWN: GhosttyKey = 110;
pub(crate) const GHOSTTY_KEY_NUMPAD_RIGHT: GhosttyKey = 111;
pub(crate) const GHOSTTY_KEY_NUMPAD_LEFT: GhosttyKey = 112;
pub(crate) const GHOSTTY_KEY_NUMPAD_BEGIN: GhosttyKey = 113;
pub(crate) const GHOSTTY_KEY_NUMPAD_HOME: GhosttyKey = 114;
pub(crate) const GHOSTTY_KEY_NUMPAD_END: GhosttyKey = 115;
pub(crate) const GHOSTTY_KEY_NUMPAD_INSERT: GhosttyKey = 116;
pub(crate) const GHOSTTY_KEY_NUMPAD_DELETE: GhosttyKey = 117;
pub(crate) const GHOSTTY_KEY_NUMPAD_PAGE_UP: GhosttyKey = 118;
pub(crate) const GHOSTTY_KEY_NUMPAD_PAGE_DOWN: GhosttyKey = 119;
pub(crate) const GHOSTTY_KEY_ESCAPE: GhosttyKey = 120;
pub(crate) const GHOSTTY_KEY_F1: GhosttyKey = 121;
pub(crate) const GHOSTTY_KEY_F2: GhosttyKey = 122;
pub(crate) const GHOSTTY_KEY_F3: GhosttyKey = 123;
pub(crate) const GHOSTTY_KEY_F4: GhosttyKey = 124;
pub(crate) const GHOSTTY_KEY_F5: GhosttyKey = 125;
pub(crate) const GHOSTTY_KEY_F6: GhosttyKey = 126;
pub(crate) const GHOSTTY_KEY_F7: GhosttyKey = 127;
pub(crate) const GHOSTTY_KEY_F8: GhosttyKey = 128;
pub(crate) const GHOSTTY_KEY_F9: GhosttyKey = 129;
pub(crate) const GHOSTTY_KEY_F10: GhosttyKey = 130;
pub(crate) const GHOSTTY_KEY_F11: GhosttyKey = 131;
pub(crate) const GHOSTTY_KEY_F12: GhosttyKey = 132;
pub(crate) const GHOSTTY_KEY_F13: GhosttyKey = 133;
pub(crate) const GHOSTTY_KEY_F14: GhosttyKey = 134;
pub(crate) const GHOSTTY_KEY_F15: GhosttyKey = 135;
pub(crate) const GHOSTTY_KEY_F16: GhosttyKey = 136;
pub(crate) const GHOSTTY_KEY_F17: GhosttyKey = 137;
pub(crate) const GHOSTTY_KEY_F18: GhosttyKey = 138;
pub(crate) const GHOSTTY_KEY_F19: GhosttyKey = 139;
pub(crate) const GHOSTTY_KEY_F20: GhosttyKey = 140;
pub(crate) const GHOSTTY_KEY_F21: GhosttyKey = 141;
pub(crate) const GHOSTTY_KEY_F22: GhosttyKey = 142;
pub(crate) const GHOSTTY_KEY_F23: GhosttyKey = 143;
pub(crate) const GHOSTTY_KEY_F24: GhosttyKey = 144;
pub(crate) const GHOSTTY_KEY_F25: GhosttyKey = 145;
pub(crate) const GHOSTTY_KEY_FN: GhosttyKey = 146;
pub(crate) const GHOSTTY_KEY_FN_LOCK: GhosttyKey = 147;
pub(crate) const GHOSTTY_KEY_PRINT_SCREEN: GhosttyKey = 148;
pub(crate) const GHOSTTY_KEY_SCROLL_LOCK: GhosttyKey = 149;
pub(crate) const GHOSTTY_KEY_PAUSE: GhosttyKey = 150;
pub(crate) const GHOSTTY_KEY_BROWSER_BACK: GhosttyKey = 151;
pub(crate) const GHOSTTY_KEY_BROWSER_FAVORITES: GhosttyKey = 152;
pub(crate) const GHOSTTY_KEY_BROWSER_FORWARD: GhosttyKey = 153;
pub(crate) const GHOSTTY_KEY_BROWSER_HOME: GhosttyKey = 154;
pub(crate) const GHOSTTY_KEY_BROWSER_REFRESH: GhosttyKey = 155;
pub(crate) const GHOSTTY_KEY_BROWSER_SEARCH: GhosttyKey = 156;
pub(crate) const GHOSTTY_KEY_BROWSER_STOP: GhosttyKey = 157;
pub(crate) const GHOSTTY_KEY_EJECT: GhosttyKey = 158;
pub(crate) const GHOSTTY_KEY_LAUNCH_APP_1: GhosttyKey = 159;
pub(crate) const GHOSTTY_KEY_LAUNCH_APP_2: GhosttyKey = 160;
pub(crate) const GHOSTTY_KEY_LAUNCH_MAIL: GhosttyKey = 161;
pub(crate) const GHOSTTY_KEY_MEDIA_PLAY_PAUSE: GhosttyKey = 162;
pub(crate) const GHOSTTY_KEY_MEDIA_SELECT: GhosttyKey = 163;
pub(crate) const GHOSTTY_KEY_MEDIA_STOP: GhosttyKey = 164;
pub(crate) const GHOSTTY_KEY_MEDIA_TRACK_NEXT: GhosttyKey = 165;
pub(crate) const GHOSTTY_KEY_MEDIA_TRACK_PREVIOUS: GhosttyKey = 166;
pub(crate) const GHOSTTY_KEY_POWER: GhosttyKey = 167;
pub(crate) const GHOSTTY_KEY_SLEEP: GhosttyKey = 168;
pub(crate) const GHOSTTY_KEY_AUDIO_VOLUME_DOWN: GhosttyKey = 169;
pub(crate) const GHOSTTY_KEY_AUDIO_VOLUME_MUTE: GhosttyKey = 170;
pub(crate) const GHOSTTY_KEY_AUDIO_VOLUME_UP: GhosttyKey = 171;
pub(crate) const GHOSTTY_KEY_WAKE_UP: GhosttyKey = 172;
pub(crate) const GHOSTTY_KEY_COPY: GhosttyKey = 173;
pub(crate) const GHOSTTY_KEY_CUT: GhosttyKey = 174;
pub(crate) const GHOSTTY_KEY_PASTE: GhosttyKey = 175;

/// `GhosttyKeyEncoderOption` (`int`).
pub(crate) type GhosttyKeyEncoderOption = c_int;
/// macOS option-as-alt setting (`GhosttyOptionAsAlt`).
pub(crate) const GHOSTTY_KEY_ENCODER_OPT_MACOS_OPTION_AS_ALT: GhosttyKeyEncoderOption = 6;
/// `GhosttyOptionAsAlt` (`int`).
pub(crate) type GhosttyOptionAsAlt = c_int;
pub(crate) const GHOSTTY_OPTION_AS_ALT_FALSE: GhosttyOptionAsAlt = 0;

/// `GhosttyMouseAction` (`int`).
pub(crate) type GhosttyMouseAction = c_int;
pub(crate) const GHOSTTY_MOUSE_ACTION_PRESS: GhosttyMouseAction = 0;
pub(crate) const GHOSTTY_MOUSE_ACTION_RELEASE: GhosttyMouseAction = 1;
pub(crate) const GHOSTTY_MOUSE_ACTION_MOTION: GhosttyMouseAction = 2;

/// `GhosttyMouseButton` (`int`).
pub(crate) type GhosttyMouseButton = c_int;
pub(crate) const GHOSTTY_MOUSE_BUTTON_LEFT: GhosttyMouseButton = 1;
pub(crate) const GHOSTTY_MOUSE_BUTTON_RIGHT: GhosttyMouseButton = 2;
pub(crate) const GHOSTTY_MOUSE_BUTTON_MIDDLE: GhosttyMouseButton = 3;
pub(crate) const GHOSTTY_MOUSE_BUTTON_FOUR: GhosttyMouseButton = 4;
pub(crate) const GHOSTTY_MOUSE_BUTTON_FIVE: GhosttyMouseButton = 5;
pub(crate) const GHOSTTY_MOUSE_BUTTON_SIX: GhosttyMouseButton = 6;
pub(crate) const GHOSTTY_MOUSE_BUTTON_SEVEN: GhosttyMouseButton = 7;
pub(crate) const GHOSTTY_MOUSE_BUTTON_EIGHT: GhosttyMouseButton = 8;
pub(crate) const GHOSTTY_MOUSE_BUTTON_NINE: GhosttyMouseButton = 9;
pub(crate) const GHOSTTY_MOUSE_BUTTON_TEN: GhosttyMouseButton = 10;
pub(crate) const GHOSTTY_MOUSE_BUTTON_ELEVEN: GhosttyMouseButton = 11;

/// Mouse position in surface-space pixels.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GhosttyMousePosition {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

/// Renderer geometry context for `GHOSTTY_MOUSE_ENCODER_OPT_SIZE`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct GhosttyMouseEncoderSize {
    pub(crate) size: usize,
    pub(crate) screen_width: u32,
    pub(crate) screen_height: u32,
    pub(crate) cell_width: u32,
    pub(crate) cell_height: u32,
    pub(crate) padding_top: u32,
    pub(crate) padding_bottom: u32,
    pub(crate) padding_right: u32,
    pub(crate) padding_left: u32,
}

/// `GhosttyMouseEncoderOption` (`int`).
pub(crate) type GhosttyMouseEncoderOption = c_int;
/// Renderer size context (`GhosttyMouseEncoderSize`).
pub(crate) const GHOSTTY_MOUSE_ENCODER_OPT_SIZE: GhosttyMouseEncoderOption = 2;
/// Whether any mouse button is currently pressed (`bool`).
pub(crate) const GHOSTTY_MOUSE_ENCODER_OPT_ANY_BUTTON_PRESSED: GhosttyMouseEncoderOption = 3;
/// Motion deduplication by last cell (`bool`).
pub(crate) const GHOSTTY_MOUSE_ENCODER_OPT_TRACK_LAST_CELL: GhosttyMouseEncoderOption = 4;

/// `GhosttyFocusEvent` (`int`).
pub(crate) type GhosttyFocusEvent = c_int;
pub(crate) const GHOSTTY_FOCUS_GAINED: GhosttyFocusEvent = 0;
pub(crate) const GHOSTTY_FOCUS_LOST: GhosttyFocusEvent = 1;

unsafe extern "C" {
    pub(crate) fn ghostty_key_event_new(
        allocator: *const c_void,
        event: *mut GhosttyKeyEvent,
    ) -> GhosttyResult;
    pub(crate) fn ghostty_key_event_free(event: GhosttyKeyEvent);
    pub(crate) fn ghostty_key_event_set_action(event: GhosttyKeyEvent, action: GhosttyKeyAction);
    pub(crate) fn ghostty_key_event_set_key(event: GhosttyKeyEvent, key: GhosttyKey);
    pub(crate) fn ghostty_key_event_set_mods(event: GhosttyKeyEvent, mods: GhosttyMods);
    pub(crate) fn ghostty_key_event_set_consumed_mods(
        event: GhosttyKeyEvent,
        consumed_mods: GhosttyMods,
    );
    pub(crate) fn ghostty_key_event_set_composing(event: GhosttyKeyEvent, composing: bool);
    pub(crate) fn ghostty_key_event_set_utf8(
        event: GhosttyKeyEvent,
        utf8: *const c_char,
        len: usize,
    );
    pub(crate) fn ghostty_key_event_set_unshifted_codepoint(event: GhosttyKeyEvent, codepoint: u32);

    pub(crate) fn ghostty_key_encoder_new(
        allocator: *const c_void,
        encoder: *mut GhosttyKeyEncoder,
    ) -> GhosttyResult;
    pub(crate) fn ghostty_key_encoder_free(encoder: GhosttyKeyEncoder);
    pub(crate) fn ghostty_key_encoder_setopt(
        encoder: GhosttyKeyEncoder,
        option: GhosttyKeyEncoderOption,
        value: *const c_void,
    );
    pub(crate) fn ghostty_key_encoder_setopt_from_terminal(
        encoder: GhosttyKeyEncoder,
        terminal: GhosttyTerminal,
    );
    pub(crate) fn ghostty_key_encoder_encode(
        encoder: GhosttyKeyEncoder,
        event: GhosttyKeyEvent,
        out_buf: *mut c_char,
        out_buf_size: usize,
        out_len: *mut usize,
    ) -> GhosttyResult;

    pub(crate) fn ghostty_mouse_event_new(
        allocator: *const c_void,
        event: *mut GhosttyMouseEvent,
    ) -> GhosttyResult;
    pub(crate) fn ghostty_mouse_event_free(event: GhosttyMouseEvent);
    pub(crate) fn ghostty_mouse_event_set_action(
        event: GhosttyMouseEvent,
        action: GhosttyMouseAction,
    );
    pub(crate) fn ghostty_mouse_event_set_button(
        event: GhosttyMouseEvent,
        button: GhosttyMouseButton,
    );
    pub(crate) fn ghostty_mouse_event_clear_button(event: GhosttyMouseEvent);
    pub(crate) fn ghostty_mouse_event_set_mods(event: GhosttyMouseEvent, mods: GhosttyMods);
    pub(crate) fn ghostty_mouse_event_set_position(
        event: GhosttyMouseEvent,
        position: GhosttyMousePosition,
    );

    pub(crate) fn ghostty_mouse_encoder_new(
        allocator: *const c_void,
        encoder: *mut GhosttyMouseEncoder,
    ) -> GhosttyResult;
    pub(crate) fn ghostty_mouse_encoder_free(encoder: GhosttyMouseEncoder);
    pub(crate) fn ghostty_mouse_encoder_setopt(
        encoder: GhosttyMouseEncoder,
        option: GhosttyMouseEncoderOption,
        value: *const c_void,
    );
    pub(crate) fn ghostty_mouse_encoder_setopt_from_terminal(
        encoder: GhosttyMouseEncoder,
        terminal: GhosttyTerminal,
    );
    pub(crate) fn ghostty_mouse_encoder_reset(encoder: GhosttyMouseEncoder);
    pub(crate) fn ghostty_mouse_encoder_encode(
        encoder: GhosttyMouseEncoder,
        event: GhosttyMouseEvent,
        out_buf: *mut c_char,
        out_buf_size: usize,
        out_len: *mut usize,
    ) -> GhosttyResult;

    pub(crate) fn ghostty_focus_encode(
        event: GhosttyFocusEvent,
        buf: *mut c_char,
        buf_len: usize,
        out_written: *mut usize,
    ) -> GhosttyResult;

    pub(crate) fn ghostty_paste_is_safe(data: *const c_char, len: usize) -> bool;
    pub(crate) fn ghostty_paste_encode(
        data: *mut c_char,
        data_len: usize,
        bracketed: bool,
        buf: *mut c_char,
        buf_len: usize,
        out_written: *mut usize,
    ) -> GhosttyResult;
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;
    use std::mem::{align_of, offset_of, size_of};

    use serde_json::Value;

    use super::*;
    use crate::{
        GHOSTTY_ABI_SCHEMA_VERSION, GHOSTTY_LIB_VERSION, GHOSTTY_SOURCE_COMMIT,
        GHOSTTY_SOURCE_REPOSITORY,
    };

    const REQUIRED_ABI: &str = include_str!("../fixtures/abi/ghostty-eb72ec6-required.json");

    #[test]
    fn handwritten_ffi_matches_the_pinned_abi_manifest() {
        let fixture: Value = serde_json::from_str(REQUIRED_ABI).expect("parse ABI fixture");
        let manifest_ptr = unsafe { ghostty_type_json() };
        assert!(!manifest_ptr.is_null(), "Ghostty ABI manifest must exist");
        let manifest = unsafe { CStr::from_ptr(manifest_ptr) }
            .to_str()
            .expect("Ghostty ABI manifest is UTF-8");
        let manifest: Value = serde_json::from_str(manifest).expect("parse Ghostty ABI manifest");

        assert_eq!(fixture["schema"], GHOSTTY_ABI_SCHEMA_VERSION);
        assert_eq!(fixture["source_repository"], GHOSTTY_SOURCE_REPOSITORY);
        assert_eq!(fixture["source_commit"], GHOSTTY_SOURCE_COMMIT);
        assert_eq!(fixture["library_version"], GHOSTTY_LIB_VERSION);
        assert_eq!(manifest["schema"], fixture["schema"]);
        assert_eq!(manifest["library_version"], fixture["library_version"]);
        assert_eq!(manifest["commit"], fixture["source_commit"]);
        assert_eq!(manifest["abi"]["pointer_size"], size_of::<*const ()>());
        assert_eq!(manifest["abi"]["usize_size"], size_of::<usize>());

        let layouts = fixture["layouts_64"]
            .as_object()
            .expect("fixture layouts are an object");
        for (name, expected) in layouts {
            let actual = &manifest["types"][name];
            assert_eq!(actual["kind"], "struct", "{name} kind changed");
            assert_eq!(actual["size"], expected["size"], "{name} size changed");
            assert_eq!(
                actual["align"], expected["align"],
                "{name} alignment changed"
            );
            for (field, offset) in expected["fields"]
                .as_object()
                .expect("fixture fields are an object")
            {
                assert_eq!(
                    actual["fields"][field]["offset"], *offset,
                    "{name}.{field} offset changed"
                );
            }
        }

        for (name, values) in fixture["enum_values"]
            .as_object()
            .expect("fixture enum values are an object")
        {
            let actual = &manifest["types"][name];
            assert_eq!(actual["kind"], "enum", "{name} kind changed");
            assert_eq!(actual["size"], size_of::<c_int>(), "{name} size changed");
            for (variant, value) in values.as_object().expect("enum fixture is an object") {
                assert_eq!(
                    actual["values"][variant], *value,
                    "{name}.{variant} value changed"
                );
            }
        }

        assert_rust_layouts();
    }

    fn assert_rust_layouts() {
        macro_rules! layout {
            ($ty:ty, $size:expr, $align:expr, {$($field:ident: $offset:expr),* $(,)?}) => {{
                assert_eq!(size_of::<$ty>(), $size, "{} Rust size changed", stringify!($ty));
                assert_eq!(align_of::<$ty>(), $align, "{} Rust alignment changed", stringify!($ty));
                $(assert_eq!(offset_of!($ty, $field), $offset, "{}.{} Rust offset changed", stringify!($ty), stringify!($field));)*
            }};
        }

        layout!(GhosttyBuffer, 24, 8, { ptr: 0, cap: 8, len: 16 });
        layout!(GhosttyColorRgb, 3, 1, { r: 0, g: 1, b: 2 });
        layout!(GhosttyFormatterScreenExtra, 16, 8, { size: 0, cursor: 8, style: 9, hyperlink: 10, protection: 11, kitty_keyboard: 12, charsets: 13 });
        layout!(GhosttyFormatterTerminalExtra, 32, 8, { size: 0, palette: 8, modes: 9, scrolling_region: 10, tabstops: 11, pwd: 12, keyboard: 13, screen: 16 });
        layout!(GhosttyFormatterTerminalOptions, 56, 8, { size: 0, emit: 8, unwrap: 12, trim: 13, extra: 16, selection: 48 });
        layout!(GhosttyMousePosition, 8, 4, { x: 0, y: 4 });
        layout!(GhosttyMouseEncoderSize, 40, 8, { size: 0, screen_width: 8, screen_height: 12, cell_width: 16, cell_height: 20, padding_top: 24, padding_bottom: 28, padding_right: 32, padding_left: 36 });
        layout!(GhosttyReader, 16, 8, { read: 0, userdata: 8 });
        layout!(GhosttyStyle, 72, 8, { size: 0, fg_color: 8, bg_color: 24, underline_color: 40, bold: 56, italic: 57, faint: 58, blink: 59, inverse: 60, invisible: 61, strikethrough: 62, overline: 63, underline: 64 });
        layout!(GhosttyStyleColor, 16, 8, { tag: 0, value: 8 });
        layout!(GhosttyTerminalModeConfig, 4, 2, { mode: 0, value: 2 });
        layout!(GhosttyTerminalScrollViewport, 24, 8, { tag: 0, value: 8 });
        layout!(GhosttyTerminalScrollbar, 24, 8, { total: 0, offset: 8, len: 16 });
        layout!(GhosttyWriter, 16, 8, { write: 0, userdata: 8 });
    }
}
