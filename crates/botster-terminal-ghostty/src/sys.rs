//! Minimal handwritten libghostty-vt declarations used by the safe adapter.

use std::ffi::{c_int, c_void};

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
