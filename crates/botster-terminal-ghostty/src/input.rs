//! Key, mouse, focus, and paste encoding over libghostty-vt.
//!
//! The worker owns one [`InputEncoders`] per terminal. Every encode borrows
//! the terminal only to read its current modes through
//! `ghostty_*_encoder_setopt_from_terminal`, then appends the escape bytes to
//! a caller-owned buffer. Neutral `TerminalKey` and button values map to
//! Ghostty values through explicit `match` arms.

use std::ffi::{c_char, c_void};
use std::ptr;

use botster_terminal_protocol::{
    TerminalKey, TerminalKeyAction, TerminalMouseAction, TerminalMouseButton,
};

use crate::native::GhosttyTerminalError;
use crate::sys::*;

/// Initial spare capacity requested before an encode call.
const ENCODE_RESERVE_BYTES: usize = 64;

/// One key event for the Ghostty key encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GhosttyKeyInput<'a> {
    /// Press, release, or repeat.
    pub action: TerminalKeyAction,
    /// Physical key.
    pub key: TerminalKey,
    /// Held modifiers, `terminal_mods` bit layout.
    pub mods: u16,
    /// Modifiers the platform consumed to produce `text`.
    pub consumed_mods: u16,
    /// Whether the event is part of an IME composition.
    pub composing: bool,
    /// Unshifted codepoint, or 0 when unknown.
    pub unshifted_codepoint: u32,
    /// Layout text without control transformations. May be empty.
    pub text: &'a str,
}

/// One mouse event for the Ghostty mouse encoder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GhosttyMouseInput {
    /// Press, release, or motion.
    pub action: TerminalMouseAction,
    /// Button for press and release, held button for motion, or none.
    pub button: Option<TerminalMouseButton>,
    /// Held modifiers, `terminal_mods` bit layout.
    pub mods: u16,
    /// Surface x in pixels under the current [`GhosttyMouseGeometry`].
    pub x_px: f32,
    /// Surface y in pixels under the current [`GhosttyMouseGeometry`].
    pub y_px: f32,
}

/// Rendered geometry the mouse encoder uses to map pixels to cells.
///
/// A client without pixel geometry uses one-pixel cells: `cell_width = 1`,
/// `cell_height = 1`, `screen_width = cols`, `screen_height = rows`, and
/// positions `x_px = col + 0.5`, `y_px = row + 0.5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GhosttyMouseGeometry {
    /// Full surface width in pixels.
    pub screen_width: u32,
    /// Full surface height in pixels.
    pub screen_height: u32,
    /// Cell width in pixels. Must be nonzero.
    pub cell_width: u32,
    /// Cell height in pixels. Must be nonzero.
    pub cell_height: u32,
}

impl GhosttyMouseGeometry {
    /// One-pixel-per-cell geometry for clients without pixel information.
    #[must_use]
    pub const fn cells(rows: u16, cols: u16) -> Self {
        Self {
            screen_width: cols as u32,
            screen_height: rows as u32,
            cell_width: 1,
            cell_height: 1,
        }
    }

    /// Geometry from cell counts and surface pixels; falls back to
    /// [`Self::cells`] when either pixel dimension is zero.
    #[must_use]
    pub const fn from_surface(rows: u16, cols: u16, width_px: u32, height_px: u32) -> Self {
        if width_px == 0 || height_px == 0 || rows == 0 || cols == 0 {
            return Self::cells(rows, cols);
        }
        let cell_width = width_px / cols as u32;
        let cell_height = height_px / rows as u32;
        if cell_width == 0 || cell_height == 0 {
            return Self::cells(rows, cols);
        }
        Self {
            screen_width: width_px,
            screen_height: height_px,
            cell_width,
            cell_height,
        }
    }

    /// Pixel center of a cell under this geometry.
    #[must_use]
    pub fn cell_center(&self, col: u16, row: u16) -> (f32, f32) {
        let x = col as f32 * self.cell_width as f32 + self.cell_width as f32 / 2.0;
        let y = row as f32 * self.cell_height as f32 + self.cell_height as f32 / 2.0;
        (x, y)
    }
}

/// Owned Ghostty key and mouse encoder handles for one terminal.
pub(crate) struct InputEncoders {
    key_event: GhosttyKeyEvent,
    key_encoder: GhosttyKeyEncoder,
    mouse_event: GhosttyMouseEvent,
    mouse_encoder: GhosttyMouseEncoder,
    any_button_pressed: bool,
}

impl InputEncoders {
    pub(crate) fn new() -> Result<Self, GhosttyTerminalError> {
        let mut encoders = Self {
            key_event: ptr::null_mut(),
            key_encoder: ptr::null_mut(),
            mouse_event: ptr::null_mut(),
            mouse_encoder: ptr::null_mut(),
            any_button_pressed: false,
        };
        let result = unsafe { ghostty_key_event_new(ptr::null(), &mut encoders.key_event) };
        check(result, "key_event_new", encoders.key_event)?;
        let result = unsafe { ghostty_key_encoder_new(ptr::null(), &mut encoders.key_encoder) };
        check(result, "key_encoder_new", encoders.key_encoder)?;
        let result = unsafe { ghostty_mouse_event_new(ptr::null(), &mut encoders.mouse_event) };
        check(result, "mouse_event_new", encoders.mouse_event)?;
        let result = unsafe { ghostty_mouse_encoder_new(ptr::null(), &mut encoders.mouse_encoder) };
        check(result, "mouse_encoder_new", encoders.mouse_encoder)?;
        let track_last_cell = true;
        unsafe {
            ghostty_mouse_encoder_setopt(
                encoders.mouse_encoder,
                GHOSTTY_MOUSE_ENCODER_OPT_TRACK_LAST_CELL,
                (&raw const track_last_cell).cast(),
            );
        }
        Ok(encoders)
    }

    /// Encode one key event against the terminal's current modes.
    ///
    /// Returns the number of bytes appended. Zero is a valid result: an
    /// unmodified modifier press produces no sequence.
    pub(crate) fn encode_key(
        &mut self,
        terminal: *mut c_void,
        input: &GhosttyKeyInput<'_>,
        out: &mut Vec<u8>,
    ) -> Result<usize, GhosttyTerminalError> {
        unsafe {
            ghostty_key_encoder_setopt_from_terminal(self.key_encoder, terminal);
            let option_as_alt = GHOSTTY_OPTION_AS_ALT_FALSE;
            ghostty_key_encoder_setopt(
                self.key_encoder,
                GHOSTTY_KEY_ENCODER_OPT_MACOS_OPTION_AS_ALT,
                (&raw const option_as_alt).cast(),
            );
            ghostty_key_event_set_action(self.key_event, key_action(input.action));
            ghostty_key_event_set_key(self.key_event, ghostty_key(input.key));
            ghostty_key_event_set_mods(self.key_event, input.mods);
            ghostty_key_event_set_consumed_mods(self.key_event, input.consumed_mods);
            ghostty_key_event_set_composing(self.key_event, input.composing);
            ghostty_key_event_set_unshifted_codepoint(self.key_event, input.unshifted_codepoint);
            let text = usable_key_text(input.text);
            if text.is_empty() {
                ghostty_key_event_set_utf8(self.key_event, ptr::null(), 0);
            } else {
                ghostty_key_event_set_utf8(
                    self.key_event,
                    text.as_ptr().cast::<c_char>(),
                    text.len(),
                );
            }
        }
        let key_encoder = self.key_encoder;
        let key_event = self.key_event;
        let written = append_encoded(out, "key_encode", |buf, cap, len| unsafe {
            ghostty_key_encoder_encode(key_encoder, key_event, buf, cap, len)
        })?;
        // The event must not keep a pointer into the caller's text after return.
        unsafe { ghostty_key_event_set_utf8(self.key_event, ptr::null(), 0) };
        Ok(written)
    }

    /// Encode one mouse event against the terminal's current tracking mode
    /// and format. Returns the number of bytes appended; zero when the
    /// terminal is not tracking this event.
    pub(crate) fn encode_mouse(
        &mut self,
        terminal: *mut c_void,
        input: &GhosttyMouseInput,
        out: &mut Vec<u8>,
    ) -> Result<usize, GhosttyTerminalError> {
        match (input.action, input.button) {
            (TerminalMouseAction::Press, Some(button)) if !is_wheel(button) => {
                self.any_button_pressed = true;
            }
            (TerminalMouseAction::Release, _) => self.any_button_pressed = false,
            _ => {}
        }
        unsafe {
            ghostty_mouse_encoder_setopt_from_terminal(self.mouse_encoder, terminal);
            let any_button_pressed = self.any_button_pressed;
            ghostty_mouse_encoder_setopt(
                self.mouse_encoder,
                GHOSTTY_MOUSE_ENCODER_OPT_ANY_BUTTON_PRESSED,
                (&raw const any_button_pressed).cast(),
            );
            ghostty_mouse_event_set_action(self.mouse_event, mouse_action(input.action));
            match input.button {
                Some(button) => {
                    ghostty_mouse_event_set_button(self.mouse_event, ghostty_mouse_button(button));
                }
                None => ghostty_mouse_event_clear_button(self.mouse_event),
            }
            ghostty_mouse_event_set_mods(self.mouse_event, input.mods);
            ghostty_mouse_event_set_position(
                self.mouse_event,
                GhosttyMousePosition {
                    x: input.x_px,
                    y: input.y_px,
                },
            );
        }
        let mouse_encoder = self.mouse_encoder;
        let mouse_event = self.mouse_event;
        append_encoded(out, "mouse_encode", |buf, cap, len| unsafe {
            ghostty_mouse_encoder_encode(mouse_encoder, mouse_event, buf, cap, len)
        })
    }

    /// Install renderer geometry so pixel positions map to cells.
    pub(crate) fn set_mouse_geometry(&mut self, geometry: GhosttyMouseGeometry) {
        let size = GhosttyMouseEncoderSize {
            size: std::mem::size_of::<GhosttyMouseEncoderSize>(),
            screen_width: geometry.screen_width,
            screen_height: geometry.screen_height,
            cell_width: geometry.cell_width.max(1),
            cell_height: geometry.cell_height.max(1),
            padding_top: 0,
            padding_bottom: 0,
            padding_right: 0,
            padding_left: 0,
        };
        unsafe {
            ghostty_mouse_encoder_setopt(
                self.mouse_encoder,
                GHOSTTY_MOUSE_ENCODER_OPT_SIZE,
                (&raw const size).cast(),
            );
            ghostty_mouse_encoder_reset(self.mouse_encoder);
        }
    }
}

impl Drop for InputEncoders {
    fn drop(&mut self) {
        unsafe {
            if !self.mouse_encoder.is_null() {
                ghostty_mouse_encoder_free(self.mouse_encoder);
            }
            if !self.mouse_event.is_null() {
                ghostty_mouse_event_free(self.mouse_event);
            }
            if !self.key_encoder.is_null() {
                ghostty_key_encoder_free(self.key_encoder);
            }
            if !self.key_event.is_null() {
                ghostty_key_event_free(self.key_event);
            }
        }
    }
}

/// Encode a focus report (`CSI I` or `CSI O`). The caller checks that the
/// terminal has focus reporting enabled before writing the bytes.
pub fn encode_focus(focused: bool, out: &mut Vec<u8>) -> Result<usize, GhosttyTerminalError> {
    let event = if focused {
        GHOSTTY_FOCUS_GAINED
    } else {
        GHOSTTY_FOCUS_LOST
    };
    append_encoded(out, "focus_encode", |buf, cap, len| unsafe {
        ghostty_focus_encode(event, buf, cap, len)
    })
}

/// Whether paste content is safe without the client's explicit override.
#[must_use]
pub fn paste_is_safe(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    unsafe { ghostty_paste_is_safe(data.as_ptr().cast::<c_char>(), data.len()) }
}

/// Encode paste content for the PTY. `data` is modified in place by Ghostty
/// (unsafe control bytes become spaces). Returns the bytes appended.
pub fn encode_paste(
    data: &mut [u8],
    bracketed: bool,
    out: &mut Vec<u8>,
) -> Result<usize, GhosttyTerminalError> {
    let data_ptr = if data.is_empty() {
        ptr::null_mut()
    } else {
        data.as_mut_ptr().cast::<c_char>()
    };
    let data_len = data.len();
    out.reserve(data_len + 16);
    append_encoded(out, "paste_encode", |buf, cap, len| unsafe {
        ghostty_paste_encode(data_ptr, data_len, bracketed, buf, cap, len)
    })
}

/// Call `encode` into the spare capacity of `out`, growing once on
/// `GHOSTTY_OUT_OF_SPACE`, and commit the written length.
fn append_encoded(
    out: &mut Vec<u8>,
    operation: &'static str,
    mut encode: impl FnMut(*mut c_char, usize, *mut usize) -> GhosttyResult,
) -> Result<usize, GhosttyTerminalError> {
    out.reserve(ENCODE_RESERVE_BYTES);
    loop {
        let start = out.len();
        let capacity = out.capacity() - start;
        let mut written = 0usize;
        let buf = unsafe { out.as_mut_ptr().add(start).cast::<c_char>() };
        let result = encode(buf, capacity, &mut written);
        if result == GHOSTTY_SUCCESS {
            if written > capacity {
                return Err(GhosttyTerminalError::operation(
                    operation,
                    GHOSTTY_INVALID_VALUE,
                ));
            }
            unsafe { out.set_len(start + written) };
            return Ok(written);
        }
        if result == GHOSTTY_OUT_OF_SPACE && written > capacity {
            out.reserve(written);
            continue;
        }
        return Err(GhosttyTerminalError::operation(operation, result));
    }
}

fn check(
    result: GhosttyResult,
    operation: &'static str,
    handle: *mut c_void,
) -> Result<(), GhosttyTerminalError> {
    if result != GHOSTTY_SUCCESS {
        return Err(GhosttyTerminalError::operation(operation, result));
    }
    if handle.is_null() {
        return Err(GhosttyTerminalError::NullHandle { operation });
    }
    Ok(())
}

/// Ghostty rejects C0 controls, DEL, and macOS PUA function codes as text.
fn usable_key_text(text: &str) -> &str {
    let unusable = text
        .chars()
        .any(|character| character.is_control() || ('\u{F700}'..='\u{F8FF}').contains(&character));
    if unusable {
        ""
    } else {
        text
    }
}

const fn is_wheel(button: TerminalMouseButton) -> bool {
    matches!(
        button,
        TerminalMouseButton::WheelUp
            | TerminalMouseButton::WheelDown
            | TerminalMouseButton::WheelLeft
            | TerminalMouseButton::WheelRight
    )
}

const fn key_action(action: TerminalKeyAction) -> GhosttyKeyAction {
    match action {
        TerminalKeyAction::Press => GHOSTTY_KEY_ACTION_PRESS,
        TerminalKeyAction::Release => GHOSTTY_KEY_ACTION_RELEASE,
        TerminalKeyAction::Repeat => GHOSTTY_KEY_ACTION_REPEAT,
    }
}

const fn mouse_action(action: TerminalMouseAction) -> GhosttyMouseAction {
    match action {
        TerminalMouseAction::Press => GHOSTTY_MOUSE_ACTION_PRESS,
        TerminalMouseAction::Release => GHOSTTY_MOUSE_ACTION_RELEASE,
        TerminalMouseAction::Motion => GHOSTTY_MOUSE_ACTION_MOTION,
    }
}

const fn ghostty_mouse_button(button: TerminalMouseButton) -> GhosttyMouseButton {
    match button {
        TerminalMouseButton::Left => GHOSTTY_MOUSE_BUTTON_LEFT,
        TerminalMouseButton::Right => GHOSTTY_MOUSE_BUTTON_RIGHT,
        TerminalMouseButton::Middle => GHOSTTY_MOUSE_BUTTON_MIDDLE,
        TerminalMouseButton::WheelUp => GHOSTTY_MOUSE_BUTTON_FOUR,
        TerminalMouseButton::WheelDown => GHOSTTY_MOUSE_BUTTON_FIVE,
        TerminalMouseButton::WheelLeft => GHOSTTY_MOUSE_BUTTON_SIX,
        TerminalMouseButton::WheelRight => GHOSTTY_MOUSE_BUTTON_SEVEN,
        TerminalMouseButton::Button8 => GHOSTTY_MOUSE_BUTTON_EIGHT,
        TerminalMouseButton::Button9 => GHOSTTY_MOUSE_BUTTON_NINE,
        TerminalMouseButton::Button10 => GHOSTTY_MOUSE_BUTTON_TEN,
        TerminalMouseButton::Button11 => GHOSTTY_MOUSE_BUTTON_ELEVEN,
    }
}

/// Explicit neutral-key to Ghostty-key map. Never relies on numeric order.
const fn ghostty_key(key: TerminalKey) -> GhosttyKey {
    match key {
        TerminalKey::Unidentified => GHOSTTY_KEY_UNIDENTIFIED,
        TerminalKey::Backquote => GHOSTTY_KEY_BACKQUOTE,
        TerminalKey::Backslash => GHOSTTY_KEY_BACKSLASH,
        TerminalKey::BracketLeft => GHOSTTY_KEY_BRACKET_LEFT,
        TerminalKey::BracketRight => GHOSTTY_KEY_BRACKET_RIGHT,
        TerminalKey::Comma => GHOSTTY_KEY_COMMA,
        TerminalKey::Digit0 => GHOSTTY_KEY_DIGIT_0,
        TerminalKey::Digit1 => GHOSTTY_KEY_DIGIT_1,
        TerminalKey::Digit2 => GHOSTTY_KEY_DIGIT_2,
        TerminalKey::Digit3 => GHOSTTY_KEY_DIGIT_3,
        TerminalKey::Digit4 => GHOSTTY_KEY_DIGIT_4,
        TerminalKey::Digit5 => GHOSTTY_KEY_DIGIT_5,
        TerminalKey::Digit6 => GHOSTTY_KEY_DIGIT_6,
        TerminalKey::Digit7 => GHOSTTY_KEY_DIGIT_7,
        TerminalKey::Digit8 => GHOSTTY_KEY_DIGIT_8,
        TerminalKey::Digit9 => GHOSTTY_KEY_DIGIT_9,
        TerminalKey::Equal => GHOSTTY_KEY_EQUAL,
        TerminalKey::IntlBackslash => GHOSTTY_KEY_INTL_BACKSLASH,
        TerminalKey::IntlRo => GHOSTTY_KEY_INTL_RO,
        TerminalKey::IntlYen => GHOSTTY_KEY_INTL_YEN,
        TerminalKey::KeyA => GHOSTTY_KEY_A,
        TerminalKey::KeyB => GHOSTTY_KEY_B,
        TerminalKey::KeyC => GHOSTTY_KEY_C,
        TerminalKey::KeyD => GHOSTTY_KEY_D,
        TerminalKey::KeyE => GHOSTTY_KEY_E,
        TerminalKey::KeyF => GHOSTTY_KEY_F,
        TerminalKey::KeyG => GHOSTTY_KEY_G,
        TerminalKey::KeyH => GHOSTTY_KEY_H,
        TerminalKey::KeyI => GHOSTTY_KEY_I,
        TerminalKey::KeyJ => GHOSTTY_KEY_J,
        TerminalKey::KeyK => GHOSTTY_KEY_K,
        TerminalKey::KeyL => GHOSTTY_KEY_L,
        TerminalKey::KeyM => GHOSTTY_KEY_M,
        TerminalKey::KeyN => GHOSTTY_KEY_N,
        TerminalKey::KeyO => GHOSTTY_KEY_O,
        TerminalKey::KeyP => GHOSTTY_KEY_P,
        TerminalKey::KeyQ => GHOSTTY_KEY_Q,
        TerminalKey::KeyR => GHOSTTY_KEY_R,
        TerminalKey::KeyS => GHOSTTY_KEY_S,
        TerminalKey::KeyT => GHOSTTY_KEY_T,
        TerminalKey::KeyU => GHOSTTY_KEY_U,
        TerminalKey::KeyV => GHOSTTY_KEY_V,
        TerminalKey::KeyW => GHOSTTY_KEY_W,
        TerminalKey::KeyX => GHOSTTY_KEY_X,
        TerminalKey::KeyY => GHOSTTY_KEY_Y,
        TerminalKey::KeyZ => GHOSTTY_KEY_Z,
        TerminalKey::Minus => GHOSTTY_KEY_MINUS,
        TerminalKey::Period => GHOSTTY_KEY_PERIOD,
        TerminalKey::Quote => GHOSTTY_KEY_QUOTE,
        TerminalKey::Semicolon => GHOSTTY_KEY_SEMICOLON,
        TerminalKey::Slash => GHOSTTY_KEY_SLASH,
        TerminalKey::AltLeft => GHOSTTY_KEY_ALT_LEFT,
        TerminalKey::AltRight => GHOSTTY_KEY_ALT_RIGHT,
        TerminalKey::Backspace => GHOSTTY_KEY_BACKSPACE,
        TerminalKey::CapsLock => GHOSTTY_KEY_CAPS_LOCK,
        TerminalKey::ContextMenu => GHOSTTY_KEY_CONTEXT_MENU,
        TerminalKey::ControlLeft => GHOSTTY_KEY_CONTROL_LEFT,
        TerminalKey::ControlRight => GHOSTTY_KEY_CONTROL_RIGHT,
        TerminalKey::Enter => GHOSTTY_KEY_ENTER,
        TerminalKey::MetaLeft => GHOSTTY_KEY_META_LEFT,
        TerminalKey::MetaRight => GHOSTTY_KEY_META_RIGHT,
        TerminalKey::ShiftLeft => GHOSTTY_KEY_SHIFT_LEFT,
        TerminalKey::ShiftRight => GHOSTTY_KEY_SHIFT_RIGHT,
        TerminalKey::Space => GHOSTTY_KEY_SPACE,
        TerminalKey::Tab => GHOSTTY_KEY_TAB,
        TerminalKey::Convert => GHOSTTY_KEY_CONVERT,
        TerminalKey::KanaMode => GHOSTTY_KEY_KANA_MODE,
        TerminalKey::NonConvert => GHOSTTY_KEY_NON_CONVERT,
        TerminalKey::Delete => GHOSTTY_KEY_DELETE,
        TerminalKey::End => GHOSTTY_KEY_END,
        TerminalKey::Help => GHOSTTY_KEY_HELP,
        TerminalKey::Home => GHOSTTY_KEY_HOME,
        TerminalKey::Insert => GHOSTTY_KEY_INSERT,
        TerminalKey::PageDown => GHOSTTY_KEY_PAGE_DOWN,
        TerminalKey::PageUp => GHOSTTY_KEY_PAGE_UP,
        TerminalKey::ArrowDown => GHOSTTY_KEY_ARROW_DOWN,
        TerminalKey::ArrowLeft => GHOSTTY_KEY_ARROW_LEFT,
        TerminalKey::ArrowRight => GHOSTTY_KEY_ARROW_RIGHT,
        TerminalKey::ArrowUp => GHOSTTY_KEY_ARROW_UP,
        TerminalKey::NumLock => GHOSTTY_KEY_NUM_LOCK,
        TerminalKey::Numpad0 => GHOSTTY_KEY_NUMPAD_0,
        TerminalKey::Numpad1 => GHOSTTY_KEY_NUMPAD_1,
        TerminalKey::Numpad2 => GHOSTTY_KEY_NUMPAD_2,
        TerminalKey::Numpad3 => GHOSTTY_KEY_NUMPAD_3,
        TerminalKey::Numpad4 => GHOSTTY_KEY_NUMPAD_4,
        TerminalKey::Numpad5 => GHOSTTY_KEY_NUMPAD_5,
        TerminalKey::Numpad6 => GHOSTTY_KEY_NUMPAD_6,
        TerminalKey::Numpad7 => GHOSTTY_KEY_NUMPAD_7,
        TerminalKey::Numpad8 => GHOSTTY_KEY_NUMPAD_8,
        TerminalKey::Numpad9 => GHOSTTY_KEY_NUMPAD_9,
        TerminalKey::NumpadAdd => GHOSTTY_KEY_NUMPAD_ADD,
        TerminalKey::NumpadBackspace => GHOSTTY_KEY_NUMPAD_BACKSPACE,
        TerminalKey::NumpadClear => GHOSTTY_KEY_NUMPAD_CLEAR,
        TerminalKey::NumpadClearEntry => GHOSTTY_KEY_NUMPAD_CLEAR_ENTRY,
        TerminalKey::NumpadComma => GHOSTTY_KEY_NUMPAD_COMMA,
        TerminalKey::NumpadDecimal => GHOSTTY_KEY_NUMPAD_DECIMAL,
        TerminalKey::NumpadDivide => GHOSTTY_KEY_NUMPAD_DIVIDE,
        TerminalKey::NumpadEnter => GHOSTTY_KEY_NUMPAD_ENTER,
        TerminalKey::NumpadEqual => GHOSTTY_KEY_NUMPAD_EQUAL,
        TerminalKey::NumpadMemoryAdd => GHOSTTY_KEY_NUMPAD_MEMORY_ADD,
        TerminalKey::NumpadMemoryClear => GHOSTTY_KEY_NUMPAD_MEMORY_CLEAR,
        TerminalKey::NumpadMemoryRecall => GHOSTTY_KEY_NUMPAD_MEMORY_RECALL,
        TerminalKey::NumpadMemoryStore => GHOSTTY_KEY_NUMPAD_MEMORY_STORE,
        TerminalKey::NumpadMemorySubtract => GHOSTTY_KEY_NUMPAD_MEMORY_SUBTRACT,
        TerminalKey::NumpadMultiply => GHOSTTY_KEY_NUMPAD_MULTIPLY,
        TerminalKey::NumpadParenLeft => GHOSTTY_KEY_NUMPAD_PAREN_LEFT,
        TerminalKey::NumpadParenRight => GHOSTTY_KEY_NUMPAD_PAREN_RIGHT,
        TerminalKey::NumpadSubtract => GHOSTTY_KEY_NUMPAD_SUBTRACT,
        TerminalKey::NumpadSeparator => GHOSTTY_KEY_NUMPAD_SEPARATOR,
        TerminalKey::NumpadUp => GHOSTTY_KEY_NUMPAD_UP,
        TerminalKey::NumpadDown => GHOSTTY_KEY_NUMPAD_DOWN,
        TerminalKey::NumpadRight => GHOSTTY_KEY_NUMPAD_RIGHT,
        TerminalKey::NumpadLeft => GHOSTTY_KEY_NUMPAD_LEFT,
        TerminalKey::NumpadBegin => GHOSTTY_KEY_NUMPAD_BEGIN,
        TerminalKey::NumpadHome => GHOSTTY_KEY_NUMPAD_HOME,
        TerminalKey::NumpadEnd => GHOSTTY_KEY_NUMPAD_END,
        TerminalKey::NumpadInsert => GHOSTTY_KEY_NUMPAD_INSERT,
        TerminalKey::NumpadDelete => GHOSTTY_KEY_NUMPAD_DELETE,
        TerminalKey::NumpadPageUp => GHOSTTY_KEY_NUMPAD_PAGE_UP,
        TerminalKey::NumpadPageDown => GHOSTTY_KEY_NUMPAD_PAGE_DOWN,
        TerminalKey::Escape => GHOSTTY_KEY_ESCAPE,
        TerminalKey::F1 => GHOSTTY_KEY_F1,
        TerminalKey::F2 => GHOSTTY_KEY_F2,
        TerminalKey::F3 => GHOSTTY_KEY_F3,
        TerminalKey::F4 => GHOSTTY_KEY_F4,
        TerminalKey::F5 => GHOSTTY_KEY_F5,
        TerminalKey::F6 => GHOSTTY_KEY_F6,
        TerminalKey::F7 => GHOSTTY_KEY_F7,
        TerminalKey::F8 => GHOSTTY_KEY_F8,
        TerminalKey::F9 => GHOSTTY_KEY_F9,
        TerminalKey::F10 => GHOSTTY_KEY_F10,
        TerminalKey::F11 => GHOSTTY_KEY_F11,
        TerminalKey::F12 => GHOSTTY_KEY_F12,
        TerminalKey::F13 => GHOSTTY_KEY_F13,
        TerminalKey::F14 => GHOSTTY_KEY_F14,
        TerminalKey::F15 => GHOSTTY_KEY_F15,
        TerminalKey::F16 => GHOSTTY_KEY_F16,
        TerminalKey::F17 => GHOSTTY_KEY_F17,
        TerminalKey::F18 => GHOSTTY_KEY_F18,
        TerminalKey::F19 => GHOSTTY_KEY_F19,
        TerminalKey::F20 => GHOSTTY_KEY_F20,
        TerminalKey::F21 => GHOSTTY_KEY_F21,
        TerminalKey::F22 => GHOSTTY_KEY_F22,
        TerminalKey::F23 => GHOSTTY_KEY_F23,
        TerminalKey::F24 => GHOSTTY_KEY_F24,
        TerminalKey::F25 => GHOSTTY_KEY_F25,
        TerminalKey::Fn => GHOSTTY_KEY_FN,
        TerminalKey::FnLock => GHOSTTY_KEY_FN_LOCK,
        TerminalKey::PrintScreen => GHOSTTY_KEY_PRINT_SCREEN,
        TerminalKey::ScrollLock => GHOSTTY_KEY_SCROLL_LOCK,
        TerminalKey::Pause => GHOSTTY_KEY_PAUSE,
        TerminalKey::BrowserBack => GHOSTTY_KEY_BROWSER_BACK,
        TerminalKey::BrowserFavorites => GHOSTTY_KEY_BROWSER_FAVORITES,
        TerminalKey::BrowserForward => GHOSTTY_KEY_BROWSER_FORWARD,
        TerminalKey::BrowserHome => GHOSTTY_KEY_BROWSER_HOME,
        TerminalKey::BrowserRefresh => GHOSTTY_KEY_BROWSER_REFRESH,
        TerminalKey::BrowserSearch => GHOSTTY_KEY_BROWSER_SEARCH,
        TerminalKey::BrowserStop => GHOSTTY_KEY_BROWSER_STOP,
        TerminalKey::Eject => GHOSTTY_KEY_EJECT,
        TerminalKey::LaunchApp1 => GHOSTTY_KEY_LAUNCH_APP_1,
        TerminalKey::LaunchApp2 => GHOSTTY_KEY_LAUNCH_APP_2,
        TerminalKey::LaunchMail => GHOSTTY_KEY_LAUNCH_MAIL,
        TerminalKey::MediaPlayPause => GHOSTTY_KEY_MEDIA_PLAY_PAUSE,
        TerminalKey::MediaSelect => GHOSTTY_KEY_MEDIA_SELECT,
        TerminalKey::MediaStop => GHOSTTY_KEY_MEDIA_STOP,
        TerminalKey::MediaTrackNext => GHOSTTY_KEY_MEDIA_TRACK_NEXT,
        TerminalKey::MediaTrackPrevious => GHOSTTY_KEY_MEDIA_TRACK_PREVIOUS,
        TerminalKey::Power => GHOSTTY_KEY_POWER,
        TerminalKey::Sleep => GHOSTTY_KEY_SLEEP,
        TerminalKey::AudioVolumeDown => GHOSTTY_KEY_AUDIO_VOLUME_DOWN,
        TerminalKey::AudioVolumeMute => GHOSTTY_KEY_AUDIO_VOLUME_MUTE,
        TerminalKey::AudioVolumeUp => GHOSTTY_KEY_AUDIO_VOLUME_UP,
        TerminalKey::WakeUp => GHOSTTY_KEY_WAKE_UP,
        TerminalKey::Copy => GHOSTTY_KEY_COPY,
        TerminalKey::Cut => GHOSTTY_KEY_CUT,
        TerminalKey::Paste => GHOSTTY_KEY_PASTE,
    }
}
