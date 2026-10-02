//! The C ABI of libghostty-vt: the functions and types that this crate calls, declared by hand from the headers of the
//! pinned Ghostty (`vendor/ghostty/include/ghostty/vt/`). Everything here is `unsafe` to call. The safe wrappers in the
//! rest of the crate state the invariant that each call relies on.
//!
//! Stolen-From: botster-core@72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c:crates/botster-terminal-ghostty/src/sys.rs (the
//! idea of a hand-declared `sys` module and its result codes; every declaration is written again against the pin).

#![allow(non_camel_case_types, dead_code)]

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

/// `GhosttyString`: a borrowed byte string.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GString {
    pub ptr: *const u8,
    pub len: usize,
}

impl GString {
    /// The bytes. An empty string may carry a null pointer.
    ///
    /// # Safety
    /// The pointer must be valid for `len` bytes for the lifetime that the caller picks.
    pub unsafe fn bytes<'a>(self) -> &'a [u8] {
        if self.len == 0 || self.ptr.is_null() {
            &[]
        } else {
            std::slice::from_raw_parts(self.ptr, self.len)
        }
    }
}

/// `GhosttyMode`: a packed mode, with the ANSI flag in bit 15.
pub type Mode = u16;

pub const fn mode_new(value: u16, ansi: bool) -> Mode {
    (value & 0x7FFF) | ((ansi as u16) << 15)
}

/// `GhosttyTerminalModeConfig`.
#[repr(C)]
pub struct ModeConfig {
    pub mode: Mode,
    pub value: bool,
}

/// `GhosttyTerminalData` keys that this crate reads.
pub mod data {
    pub const COLS: i32 = 1;
    pub const ROWS: i32 = 2;
    pub const CURSOR_X: i32 = 3;
    pub const CURSOR_Y: i32 = 4;
    pub const TOTAL_ROWS: i32 = 14;
    pub const ACTIVE_SCREEN: i32 = 6;
    pub const CURSOR_VISIBLE: i32 = 7;
    pub const KITTY_KEYBOARD_FLAGS: i32 = 8;
    pub const TITLE: i32 = 12;
    pub const PWD: i32 = 13;
    pub const MODE: i32 = 37;
    pub const MOUSE_EVENT: i32 = 43;
    pub const MOUSE_FORMAT: i32 = 44;
}

/// `GhosttyTerminalOption` keys that this crate sets.
pub mod opt {
    pub const USERDATA: i32 = 0;
    pub const BELL: i32 = 2;
    pub const TITLE_CHANGED: i32 = 5;
    pub const PWD_CHANGED: i32 = 25;
    pub const CLIPBOARD_WRITE: i32 = 26;
    pub const DESKTOP_NOTIFICATION: i32 = 29;
    pub const SEMANTIC_PROMPT: i32 = 42;
    pub const SCROLLBACK_MAX_BYTES: i32 = 27;
}

/// `GhosttyTerminalScreen`.
pub const SCREEN_ALTERNATE: i32 = 1;

/// `GhosttyMouseTrackingMode`.
pub mod mouse_event {
    pub const NONE: i32 = 0;
    pub const X10: i32 = 1;
    pub const NORMAL: i32 = 2;
    pub const BUTTON: i32 = 3;
    pub const ANY: i32 = 4;
}

/// `GhosttyMouseFormat`.
pub mod mouse_format {
    pub const X10: i32 = 0;
    pub const UTF8: i32 = 1;
    pub const SGR: i32 = 2;
    pub const URXVT: i32 = 3;
    pub const SGR_PIXELS: i32 = 4;
}

/// `GhosttyTerminalNotificationSource`.
pub mod notification_source {
    pub const OSC9: i32 = 0;
    pub const OSC777: i32 = 1;
}

/// `GhosttySemanticPromptKind`.
pub mod semantic_prompt_kind {
    pub const PROMPT_START: i32 = 1;
    pub const INPUT_START: i32 = 2;
    pub const OUTPUT_START: i32 = 3;
    pub const COMMAND_END: i32 = 4;
}

/// `GhosttyClipboardWriteResult`.
pub const CLIPBOARD_WRITE_SUCCESS: i32 = 0;

/// `GhosttyTerminalDesktopNotification`.
#[repr(C)]
pub struct DesktopNotification {
    pub size: usize,
    pub title: GString,
    pub body: GString,
    pub source: i32,
}

/// `GhosttyTerminalSemanticPrompt`.
#[repr(C)]
pub struct SemanticPrompt {
    pub size: usize,
    pub kind: i32,
    pub prompt_kind: i32,
    pub has_exit_code: bool,
    pub exit_code: i32,
    pub command: GString,
    pub error: GString,
}

/// `GhosttyClipboardContent`.
#[repr(C)]
pub struct ClipboardContent {
    pub mime: GString,
    pub data: GString,
}

/// `GhosttyClipboardWriteReply`.
#[repr(C)]
pub struct ClipboardWriteReply {
    pub size: usize,
    pub result: i32,
    pub remember: bool,
}

pub type ClipboardWriteReplyFn = unsafe extern "C" fn(*const ClipboardWrite, *const ClipboardWriteReply);

/// `GhosttyClipboardWrite`.
#[repr(C)]
pub struct ClipboardWrite {
    pub size: usize,
    pub location: i32,
    pub contents: *const ClipboardContent,
    pub contents_len: usize,
    pub name: GString,
    pub granted: bool,
    pub can_remember: bool,
    pub ctx: *const c_void,
    pub reply: ClipboardWriteReplyFn,
    pub selection: GString,
    pub terminator: i32,
}

pub type BellFn = unsafe extern "C" fn(Terminal, *mut c_void);
pub type TitleChangedFn = unsafe extern "C" fn(Terminal, *mut c_void);
pub type PwdChangedFn = unsafe extern "C" fn(Terminal, *mut c_void);
pub type DesktopNotificationFn = unsafe extern "C" fn(Terminal, *mut c_void, *const DesktopNotification);
pub type SemanticPromptFn = unsafe extern "C" fn(Terminal, *mut c_void, *const SemanticPrompt);
pub type ClipboardWriteFn = unsafe extern "C" fn(Terminal, *mut c_void, *const ClipboardWrite);

/// `GhosttyPointTag`.
pub mod point_tag {
    pub const ACTIVE: i32 = 0;
    pub const SCREEN: i32 = 2;
}

/// `GhosttyPointCoordinate`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PointCoordinate {
    pub x: u16,
    pub y: u32,
}

/// `GhosttyPointValue`.
#[repr(C)]
#[derive(Clone, Copy)]
pub union PointValue {
    pub coordinate: PointCoordinate,
    pub padding: [u64; 2],
}

/// `GhosttyPoint`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Point {
    pub tag: i32,
    pub value: PointValue,
}

impl Point {
    pub fn new(tag: i32, x: u16, y: u32) -> Self {
        Self { tag, value: PointValue { coordinate: PointCoordinate { x, y } } }
    }
}

/// `GhosttyGridRef`: set `size` before use.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GridRef {
    pub size: usize,
    pub node: *mut c_void,
    pub x: u16,
    pub y: u16,
}

impl GridRef {
    pub fn empty() -> Self {
        Self { size: std::mem::size_of::<Self>(), node: std::ptr::null_mut(), x: 0, y: 0 }
    }
}

/// `GhosttyCell`.
pub type Cell = u64;

/// `GhosttyCellData` keys that this crate reads.
pub mod cell_data {
    pub const WIDE: i32 = 3;
    pub const HAS_TEXT: i32 = 4;
}

/// `GhosttyCellWide`.
pub mod cell_wide {
    pub const SPACER_TAIL: i32 = 2;
    pub const SPACER_HEAD: i32 = 3;
}

/// `GhosttySelection`.
#[repr(C)]
pub struct Selection {
    pub size: usize,
    pub start: GridRef,
    pub end: GridRef,
    pub rectangle: bool,
}

/// `GhosttyFormatterScreenExtra`.
#[repr(C)]
pub struct FormatterScreenExtra {
    pub size: usize,
    pub cursor: bool,
    pub style: bool,
    pub hyperlink: bool,
    pub protection: bool,
    pub kitty_keyboard: bool,
    pub charsets: bool,
}

/// `GhosttyFormatterTerminalExtra`.
#[repr(C)]
pub struct FormatterTerminalExtra {
    pub size: usize,
    pub palette: bool,
    pub modes: bool,
    pub scrolling_region: bool,
    pub tabstops: bool,
    pub pwd: bool,
    pub keyboard: bool,
    pub screen: FormatterScreenExtra,
}

/// `GhosttyFormatterTerminalOptions`.
#[repr(C)]
pub struct FormatterTerminalOptions {
    pub size: usize,
    pub emit: i32,
    pub unwrap: bool,
    pub trim: bool,
    pub extra: FormatterTerminalExtra,
    pub selection: *const Selection,
}

/// `GhosttyFormatterFormat`.
pub const FORMATTER_FORMAT_PLAIN: i32 = 0;

/// `GhosttyFormatter`: an opaque handle.
pub type Formatter = *mut c_void;

/// `GhosttyMods` bits.
pub mod mods {
    pub const SHIFT: u16 = 1 << 0;
    pub const CTRL: u16 = 1 << 1;
    pub const ALT: u16 = 1 << 2;
    pub const SUPER: u16 = 1 << 3;
    pub const CAPS_LOCK: u16 = 1 << 4;
    pub const NUM_LOCK: u16 = 1 << 5;
    pub const HYPER: u16 = 1 << 10;
    pub const META: u16 = 1 << 11;
}

/// `GhosttyKeyAction`.
pub mod key_action {
    pub const RELEASE: i32 = 0;
    pub const PRESS: i32 = 1;
    pub const REPEAT: i32 = 2;
}

/// `GhosttyKeyEncoderOption`.
pub mod key_opt {
    pub const CURSOR_KEY_APPLICATION: i32 = 0;
    pub const KEYPAD_KEY_APPLICATION: i32 = 1;
    pub const IGNORE_KEYPAD_WITH_NUMLOCK: i32 = 2;
    pub const ALT_ESC_PREFIX: i32 = 3;
    pub const MODIFY_OTHER_KEYS_STATE_2: i32 = 4;
    pub const KITTY_FLAGS: i32 = 5;
    pub const MACOS_OPTION_AS_ALT: i32 = 6;
    pub const BACKARROW_KEY_MODE: i32 = 7;
}

/// `GhosttyMouseEncoderOption`.
pub mod mouse_opt {
    pub const EVENT: i32 = 0;
    pub const FORMAT: i32 = 1;
    pub const SIZE: i32 = 2;
    pub const ANY_BUTTON_PRESSED: i32 = 3;
}

/// `GhosttyMouseAction`.
pub mod mouse_action {
    pub const PRESS: i32 = 0;
    pub const RELEASE: i32 = 1;
    pub const MOTION: i32 = 2;
}

/// `GhosttyMouseButton`.
pub mod mouse_button {
    pub const LEFT: i32 = 1;
    pub const RIGHT: i32 = 2;
    pub const MIDDLE: i32 = 3;
    pub const FOUR: i32 = 4;
    pub const FIVE: i32 = 5;
    pub const SIX: i32 = 6;
    pub const SEVEN: i32 = 7;
    pub const EIGHT: i32 = 8;
    pub const NINE: i32 = 9;
}

/// `GhosttyMouseEncoderSize`.
#[repr(C)]
pub struct MouseEncoderSize {
    pub size: usize,
    pub screen_width: u32,
    pub screen_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub padding_right: u32,
    pub padding_left: u32,
}

/// `GhosttyMousePosition`.
#[repr(C)]
pub struct MousePosition {
    pub x: f32,
    pub y: f32,
}

/// `GhosttyMouseCell`.
#[repr(C)]
pub struct MouseCell {
    pub col: u32,
    pub row: u32,
}

/// `GhosttyPasteFrame`.
#[repr(C)]
pub struct PasteFrame {
    pub prefix: GString,
    pub suffix: GString,
}

pub type KeyEncoder = *mut c_void;
pub type KeyEvent = *mut c_void;
pub type MouseEncoder = *mut c_void;
pub type MouseEvent = *mut c_void;

/// `GhosttyFocusEvent`.
pub mod focus {
    pub const GAINED: i32 = 0;
    pub const LOST: i32 = 1;
}

extern "C" {
    pub fn ghostty_key_encoder_new(allocator: *const c_void, encoder: *mut KeyEncoder) -> Result;
    pub fn ghostty_key_encoder_free(encoder: KeyEncoder);
    pub fn ghostty_key_encoder_setopt(encoder: KeyEncoder, option: i32, value: *const c_void);
    pub fn ghostty_key_encoder_setopt_from_terminal(encoder: KeyEncoder, terminal: Terminal);
    pub fn ghostty_key_encoder_encode(
        encoder: KeyEncoder,
        event: KeyEvent,
        out_buf: *mut u8,
        out_buf_size: usize,
        out_len: *mut usize,
    ) -> Result;
    pub fn ghostty_key_event_new(allocator: *const c_void, event: *mut KeyEvent) -> Result;
    pub fn ghostty_key_event_free(event: KeyEvent);
    pub fn ghostty_key_event_set_action(event: KeyEvent, action: i32);
    pub fn ghostty_key_event_set_key(event: KeyEvent, key: i32);
    pub fn ghostty_key_event_set_mods(event: KeyEvent, mods: u16);
    pub fn ghostty_key_event_set_consumed_mods(event: KeyEvent, mods: u16);
    pub fn ghostty_key_event_set_utf8(event: KeyEvent, utf8: *const u8, len: usize);
    pub fn ghostty_key_event_set_unshifted_codepoint(event: KeyEvent, codepoint: u32);
    pub fn ghostty_key_event_set_shifted_key(event: KeyEvent, codepoint: u32);
    pub fn ghostty_key_event_set_base_layout_key(event: KeyEvent, codepoint: u32);

    pub fn ghostty_mouse_encoder_new(allocator: *const c_void, encoder: *mut MouseEncoder) -> Result;
    pub fn ghostty_mouse_encoder_free(encoder: MouseEncoder);
    pub fn ghostty_mouse_encoder_setopt(encoder: MouseEncoder, option: i32, value: *const c_void);
    pub fn ghostty_mouse_encoder_setopt_from_terminal(encoder: MouseEncoder, terminal: Terminal);
    pub fn ghostty_mouse_encoder_encode(
        encoder: MouseEncoder,
        event: MouseEvent,
        out_buf: *mut u8,
        out_buf_size: usize,
        out_len: *mut usize,
    ) -> Result;
    pub fn ghostty_mouse_event_new(allocator: *const c_void, event: *mut MouseEvent) -> Result;
    pub fn ghostty_mouse_event_free(event: MouseEvent);
    pub fn ghostty_mouse_event_set_action(event: MouseEvent, action: i32);
    pub fn ghostty_mouse_event_set_button(event: MouseEvent, button: i32);
    pub fn ghostty_mouse_event_clear_button(event: MouseEvent);
    pub fn ghostty_mouse_event_set_mods(event: MouseEvent, mods: u16);
    pub fn ghostty_mouse_event_set_position(event: MouseEvent, position: MousePosition);
    pub fn ghostty_mouse_event_set_cell(event: MouseEvent, cell: MouseCell);

    pub fn ghostty_paste_encode(
        data: *mut u8,
        data_len: usize,
        bracketed: bool,
        out: *mut u8,
        out_len: usize,
        out_written: *mut usize,
    ) -> Result;
    pub fn ghostty_focus_encode(event: i32, buf: *mut u8, buf_len: usize, out_written: *mut usize) -> Result;
    pub fn ghostty_paste_frame(bracketed: bool, out: *mut PasteFrame);

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
    pub fn ghostty_terminal_set(terminal: Terminal, option: i32, value: *const c_void) -> Result;
    pub fn ghostty_terminal_grid_ref(terminal: Terminal, point: Point, out_ref: *mut GridRef) -> Result;
    pub fn ghostty_grid_ref_cell(grid_ref: *const GridRef, out_cell: *mut Cell) -> Result;
    pub fn ghostty_grid_ref_graphemes(
        grid_ref: *const GridRef,
        buf: *mut u32,
        buf_len: usize,
        out_len: *mut usize,
    ) -> Result;
    pub fn ghostty_cell_get(cell: Cell, data: i32, out: *mut c_void) -> Result;
    pub fn ghostty_formatter_terminal_new(
        allocator: *const c_void,
        formatter: *mut Formatter,
        terminal: Terminal,
        options: FormatterTerminalOptions,
    ) -> Result;
    pub fn ghostty_formatter_format_buf(
        formatter: Formatter,
        buf: *mut u8,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result;
    pub fn ghostty_formatter_free(formatter: Formatter);
}
