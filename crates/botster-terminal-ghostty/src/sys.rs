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
    pub fn ghostty_terminal_set(terminal: Terminal, option: i32, value: *const c_void) -> Result;
}
