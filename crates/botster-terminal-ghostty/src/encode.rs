//! The encoders (IN-8, IN-9, 5.1A): a key, a mouse event, a focus event and the paste frame, written by libghostty from
//! the modes of the model. This module maps the contract's input types to the library's events and the library's zero
//! output to the contract's typed zero. It writes no terminal byte.

use std::ffi::c_void;

use botster_core_contract::prelude::{CellPx, KeyInput, MouseInput, Size, UnsupportedWhat};
use botster_route_codec::prelude::{
    Key, KeyEvent, ModeFlags, Modifier, MouseAction, MouseButton, MouseEncoding, MouseTracking,
};

use crate::sys;

/// Why an event gave no bytes (IN-9 typed zero).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The event has no encoding that Core can write, for the reason that `what` names (`NotWritten(Unsupported)`).
    Unsupported(UnsupportedWhat),
    /// The event is valid and the active modes do not report it (`NotWritten(NotReported)`).
    NotReported,
}

/// The state of the model that an encoder needs. The terminal fills it from the library; the free functions fill it from
/// a `ModeFlags`, which cannot carry every field (see `from_mode_flags`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct EncoderState {
    cursor_key_application: bool,
    keypad_key_application: bool,
    ignore_keypad_with_numlock: bool,
    alt_esc_prefix: bool,
    modify_other_keys_state_2: bool,
    backarrow_key_mode: bool,
    kitty_flags: u8,
    mouse_event: i32,
    mouse_format: i32,
}

impl EncoderState {
    /// The state that `ModeFlags` carries. The library keeps three more fields that the contract's `ModeFlags` does not
    /// name: xterm modifyOtherKeys state 2 (not a mode, set by `CSI > 4 ; 2 m`), and the macOS option-as-alt setting
    /// (not terminal state). Both are off here. DEC modes 67, 1035 and 1036 are in `other_modes`.
    pub(crate) fn from_mode_flags(modes: &ModeFlags) -> Self {
        let other = |name: &str| modes.other_modes.get(name).copied().unwrap_or(false);
        Self {
            cursor_key_application: modes.application_cursor_keys,
            keypad_key_application: modes.application_keypad,
            ignore_keypad_with_numlock: other("dec_1035"),
            alt_esc_prefix: other("dec_1036"),
            modify_other_keys_state_2: other(crate::modes::MODIFY_OTHER_KEYS_2),
            backarrow_key_mode: other("dec_67"),
            kitty_flags: modes.kitty_flags,
            mouse_event: match modes.mouse_tracking {
                MouseTracking::None | MouseTracking::Other => sys::mouse_event::NONE,
                MouseTracking::X10 => sys::mouse_event::X10,
                MouseTracking::Normal => sys::mouse_event::NORMAL,
                MouseTracking::ButtonEvent => sys::mouse_event::BUTTON,
                MouseTracking::AnyEvent => sys::mouse_event::ANY,
            },
            mouse_format: match modes.mouse_encoding {
                MouseEncoding::X10 | MouseEncoding::Other => sys::mouse_format::X10,
                MouseEncoding::Utf8 => sys::mouse_format::UTF8,
                MouseEncoding::Sgr => sys::mouse_format::SGR,
                MouseEncoding::Urxvt => sys::mouse_format::URXVT,
                MouseEncoding::SgrPixels => sys::mouse_format::SGR_PIXELS,
            },
        }
    }
}

/// Where an encoder gets its state: the live terminal, or an explicit state.
#[derive(Clone, Copy)]
pub(crate) enum Source {
    Terminal(sys::Terminal),
    State(EncoderState),
}

// ---- keys ----

/// The libghostty key of a named key. The values are those of `GhosttyKey` in the header of the pinned Ghostty; a test
/// compares every entry with the header.
pub(crate) const NAMED_KEYS: &[(&str, i32)] = &[
    ("enter", 58),
    ("tab", 64),
    ("backspace", 53),
    ("escape", 120),
    ("insert", 72),
    ("delete", 68),
    ("home", 71),
    ("end", 69),
    ("page_up", 74),
    ("page_down", 73),
    ("arrow_up", 78),
    ("arrow_down", 75),
    ("arrow_left", 76),
    ("arrow_right", 77),
    ("kp_0", 80),
    ("kp_1", 81),
    ("kp_2", 82),
    ("kp_3", 83),
    ("kp_4", 84),
    ("kp_5", 85),
    ("kp_6", 86),
    ("kp_7", 87),
    ("kp_8", 88),
    ("kp_9", 89),
    ("kp_decimal", 95),
    ("kp_divide", 96),
    ("kp_multiply", 104),
    ("kp_subtract", 107),
    ("kp_add", 90),
    ("kp_enter", 97),
    ("kp_equal", 98),
    ("caps_lock", 54),
    ("scroll_lock", 149),
    ("num_lock", 79),
    ("print_screen", 148),
    ("pause", 150),
    ("menu", 55),
    ("left_shift", 61),
    ("left_control", 56),
    ("left_alt", 51),
    ("left_super", 59),
    ("right_shift", 62),
    ("right_control", 57),
    ("right_alt", 52),
    ("right_super", 60),
];

/// The named modifier keys. Alone, they encode only when the kitty flag 8 is on.
const MODIFIER_KEYS: &[&str] = &[
    "left_shift",
    "left_control",
    "left_alt",
    "left_super",
    "right_shift",
    "right_control",
    "right_alt",
    "right_super",
];

/// The kitty keyboard flag "report all keys as escape codes".
const KITTY_REPORT_ALL: u8 = 8;

/// `GhosttyKey` of `f1` to `f25` follows F1 in order. The keys `f26` to `f35` come after every other key.
const KEY_F1: i32 = 121;
const KEY_F26: i32 = 176;

/// The libghostty key of a named key, or `None` for a name outside the contract's set.
pub(crate) fn named_key(name: &str) -> Option<i32> {
    if let Some(number) = name.strip_prefix('f').and_then(|n| n.parse::<i32>().ok()) {
        return match number {
            1..=25 => Some(KEY_F1 + number - 1),
            26..=35 => Some(KEY_F26 + number - 26),
            _ => None,
        };
    }
    NAMED_KEYS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, key)| *key)
}

fn one_scalar(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(first)
}

fn mod_bits(mods: &[Modifier]) -> u16 {
    mods.iter().fold(0, |bits, modifier| {
        bits | match modifier {
            Modifier::Shift => sys::mods::SHIFT,
            Modifier::Alt => sys::mods::ALT,
            Modifier::Ctrl => sys::mods::CTRL,
            Modifier::Meta => sys::mods::META,
            Modifier::Super => sys::mods::SUPER,
            Modifier::Hyper => sys::mods::HYPER,
            Modifier::CapsLock => sys::mods::CAPS_LOCK,
            Modifier::NumLock => sys::mods::NUM_LOCK,
        }
    })
}

/// Whether a constructor of the library made its handle: SUCCESS and a handle. A failure (OUT_OF_MEMORY) is refused,
/// and a null handle is refused even with SUCCESS, so a broken promise never becomes a null handle.
pub(crate) fn created(code: sys::Result, handle: *mut c_void) -> bool {
    code == sys::SUCCESS && !handle.is_null()
}

struct KeyEncoder(sys::KeyEncoder);

impl KeyEncoder {
    fn new() -> Result<Self, crate::Error> {
        let mut encoder: sys::KeyEncoder = std::ptr::null_mut();
        // SAFETY: a valid out pointer; a null allocator selects the default; the handle is freed in `Drop`.
        let code = unsafe { sys::ghostty_key_encoder_new(std::ptr::null(), &mut encoder) };
        if !created(code, encoder) {
            return Err(crate::Error::OutOfMemory);
        }
        Ok(Self(encoder))
    }

    fn configure(&self, source: Source) {
        // Core's Alt is the Alt key. The library's macOS option-as-alt setting decides whether Option acts as Alt, and it
        // is reset by `setopt_from_terminal`, so it is set after the terminal state, on every operating system.
        let option_as_alt: i32 = 1; // GHOSTTY_OPTION_AS_ALT_TRUE
        match source {
            // SAFETY: the encoder and the terminal are live; the call copies the terminal's state.
            Source::Terminal(terminal) => unsafe {
                sys::ghostty_key_encoder_setopt_from_terminal(self.0, terminal)
            },
            Source::State(state) => {
                let set_bool = |option: i32, value: bool| {
                    // SAFETY: the encoder is live, and the option takes a `bool`.
                    unsafe {
                        sys::ghostty_key_encoder_setopt(
                            self.0,
                            option,
                            (&value as *const bool).cast(),
                        )
                    }
                };
                set_bool(
                    sys::key_opt::CURSOR_KEY_APPLICATION,
                    state.cursor_key_application,
                );
                set_bool(
                    sys::key_opt::KEYPAD_KEY_APPLICATION,
                    state.keypad_key_application,
                );
                set_bool(
                    sys::key_opt::IGNORE_KEYPAD_WITH_NUMLOCK,
                    state.ignore_keypad_with_numlock,
                );
                set_bool(sys::key_opt::ALT_ESC_PREFIX, state.alt_esc_prefix);
                set_bool(
                    sys::key_opt::MODIFY_OTHER_KEYS_STATE_2,
                    state.modify_other_keys_state_2,
                );
                set_bool(sys::key_opt::BACKARROW_KEY_MODE, state.backarrow_key_mode);
                // SAFETY: the encoder is live, and KITTY_FLAGS takes a `u8` bitmask.
                unsafe {
                    sys::ghostty_key_encoder_setopt(
                        self.0,
                        sys::key_opt::KITTY_FLAGS,
                        (&state.kitty_flags as *const u8).cast(),
                    );
                }
            }
        }
        // SAFETY: the encoder is live, and the option takes a `GhosttyOptionAsAlt` (an `int`).
        unsafe {
            sys::ghostty_key_encoder_setopt(
                self.0,
                sys::key_opt::MACOS_OPTION_AS_ALT,
                (&option_as_alt as *const i32).cast(),
            );
        }
    }
}

impl Drop for KeyEncoder {
    fn drop(&mut self) {
        // SAFETY: the encoder is live and freed once.
        unsafe { sys::ghostty_key_encoder_free(self.0) }
    }
}

struct KeyEventHandle(sys::KeyEvent);

impl Drop for KeyEventHandle {
    fn drop(&mut self) {
        // SAFETY: the event is live and freed once.
        unsafe { sys::ghostty_key_event_free(self.0) }
    }
}

/// Build the library's event from a key input. The event borrows `text`, so it must not outlive it.
fn key_event(
    input: &KeyInput,
    text: &str,
    base_text: &mut [u8; 4],
) -> Result<KeyEventHandle, EncodeError> {
    let mut event: sys::KeyEvent = std::ptr::null_mut();
    // SAFETY: a valid out pointer; a null allocator selects the default.
    let code = unsafe { sys::ghostty_key_event_new(std::ptr::null(), &mut event) };
    if !created(code, event) {
        return Err(EncodeError::Unsupported(UnsupportedWhat::Other));
    }
    let handle = KeyEventHandle(event);

    let action = match input.event {
        KeyEvent::Press => sys::key_action::PRESS,
        KeyEvent::Repeat => sys::key_action::REPEAT,
        KeyEvent::Release => sys::key_action::RELEASE,
    };
    let bits = mod_bits(&input.mods);
    let has_text = !text.is_empty();

    // The key: a named key is a library key. A character key is identified by its unshifted codepoint.
    let (key, unshifted, utf8): (i32, u32, &str) = match &input.key {
        Key::Named(name) => {
            let key =
                named_key(&name.0).ok_or(EncodeError::Unsupported(UnsupportedWhat::NamedKey))?;
            (key, 0, text)
        }
        Key::Char(base) => {
            let base = one_scalar(base).ok_or(EncodeError::Unsupported(UnsupportedWhat::Other))?;
            // With ctrl or alt, and no text, the rule applies to the base character (5.1A legacy rule ii), which is the
            // text that the library uses for the control code and the escape prefix.
            let rule_ii = !has_text && bits & (sys::mods::CTRL | sys::mods::ALT) != 0;
            let utf8 = if rule_ii {
                &*base.encode_utf8(base_text)
            } else {
                text
            };
            (0, u32::from(base), utf8)
        }
    };

    // SAFETY: the event is live. `utf8` is valid for the whole encode (the caller keeps `text` and `base_text`).
    unsafe {
        sys::ghostty_key_event_set_action(event, action);
        sys::ghostty_key_event_set_key(event, key);
        sys::ghostty_key_event_set_mods(event, bits);
        // Shift is consumed by text that the key produced: the text already carries it.
        let consumed = if has_text { bits & sys::mods::SHIFT } else { 0 };
        sys::ghostty_key_event_set_consumed_mods(event, consumed);
        sys::ghostty_key_event_set_utf8(event, utf8.as_ptr(), utf8.len());
        sys::ghostty_key_event_set_unshifted_codepoint(event, unshifted);
    }

    // The alternate keys are used as given and never derived.
    let alternate = |key: &Option<Key>| -> Result<u32, EncodeError> {
        match key {
            None => Ok(0),
            Some(Key::Char(text)) => one_scalar(text)
                .map(u32::from)
                .ok_or(EncodeError::Unsupported(UnsupportedWhat::Other)),
            Some(Key::Named(_)) => Err(EncodeError::Unsupported(UnsupportedWhat::Other)),
        }
    };
    let shifted = alternate(&input.shifted_key)?;
    let base_layout = alternate(&input.base_layout_key)?;
    // SAFETY: the event is live.
    unsafe {
        sys::ghostty_key_event_set_shifted_key(event, shifted);
        sys::ghostty_key_event_set_base_layout_key(event, base_layout);
    }
    Ok(handle)
}

pub(crate) fn encode_key(source: Source, input: &KeyInput) -> Result<Vec<u8>, EncodeError> {
    let encoder =
        KeyEncoder::new().map_err(|_| EncodeError::Unsupported(UnsupportedWhat::Other))?;
    encoder.configure(source);

    let text = input.text.as_deref().unwrap_or("");
    let mut base_text = [0u8; 4];
    let event = key_event(input, text, &mut base_text)?;

    let bytes = run_encoder(|buf, len, written| {
        // SAFETY: the encoder and the event are live; the buffer holds `len` bytes.
        unsafe { sys::ghostty_key_encoder_encode(encoder.0, event.0, buf, len, written) }
    })?;
    if !bytes.is_empty() {
        return Ok(bytes);
    }

    // Zero bytes. Legacy Shift with a character key and no text is the one case that has a contract reason of its own:
    // there is no layout guess (5.1A legacy rule iii).
    let state_kitty = match source {
        Source::State(state) => state.kitty_flags,
        Source::Terminal(terminal) => crate::modes::mode_flags(terminal).kitty_flags,
    };
    // A modifier key alone gives no bytes unless the kitty flag "report all keys as escape codes" (8) is on. That is
    // a defined refusal of the named key (steward ruling R-14.1), not an unreported result.
    if state_kitty & KITTY_REPORT_ALL == 0 {
        if let Key::Named(name) = &input.key {
            if MODIFIER_KEYS.contains(&name.0.as_str()) {
                return Err(EncodeError::Unsupported(UnsupportedWhat::NamedKey));
            }
        }
    }
    let shift = input.mods.contains(&Modifier::Shift);
    let ctrl_or_alt = input
        .mods
        .iter()
        .any(|m| matches!(m, Modifier::Ctrl | Modifier::Alt));
    if state_kitty == 0
        && matches!(input.event, KeyEvent::Press | KeyEvent::Repeat)
        && shift
        && !ctrl_or_alt
        && text.is_empty()
        && matches!(input.key, Key::Char(_))
    {
        return Err(EncodeError::Unsupported(UnsupportedWhat::ProducedText));
    }
    Err(EncodeError::NotReported)
}

/// Call an encode function with a probe and then with a buffer of the size that it asks for. With an empty buffer, the
/// library's encoders return SUCCESS when there is nothing to write and OUT_OF_SPACE with the size otherwise.
fn run_encoder(
    mut call: impl FnMut(*mut u8, usize, *mut usize) -> sys::Result,
) -> Result<Vec<u8>, EncodeError> {
    let mut needed: usize = 0;
    match call(std::ptr::null_mut(), 0, &mut needed) {
        sys::SUCCESS => return Ok(Vec::new()),
        sys::OUT_OF_SPACE => {}
        _ => return Err(EncodeError::Unsupported(UnsupportedWhat::Other)),
    }
    let mut buffer = vec![0u8; needed];
    let mut written: usize = 0;
    let code = call(buffer.as_mut_ptr(), needed, &mut written);
    if code != sys::SUCCESS {
        return Err(EncodeError::Unsupported(UnsupportedWhat::Other));
    }
    buffer.truncate(written);
    Ok(buffer)
}

// ---- mouse ----

struct MouseEncoder(sys::MouseEncoder);

impl Drop for MouseEncoder {
    fn drop(&mut self) {
        // SAFETY: the encoder is live and freed once.
        unsafe { sys::ghostty_mouse_encoder_free(self.0) }
    }
}

struct MouseEventHandle(sys::MouseEvent);

impl Drop for MouseEventHandle {
    fn drop(&mut self) {
        // SAFETY: the event is live and freed once.
        unsafe { sys::ghostty_mouse_event_free(self.0) }
    }
}

/// The encoder's size context: the screen in pixels from the cell size, with no padding. Without a cell size the cell is
/// one pixel, so a pixel position is reported as given.
/// The size that the mouse encoder works with. Without a cell size, the pixel screen is not known: under SGR pixels
/// the position is reported as given (R-13), so the screen is made as large as the library allows, and no position is
/// outside it. For the other formats the screen is the grid at one pixel per cell.
fn encoder_size(
    cols: u32,
    rows: u32,
    cell_px: Option<CellPx>,
    unbounded_pixels: bool,
) -> sys::MouseEncoderSize {
    let (cell_width, cell_height) =
        cell_px.map_or((1, 1), |px| (px.width.max(1), px.height.max(1)));
    let (screen_width, screen_height) = if cell_px.is_none() && unbounded_pixels {
        (u32::MAX, u32::MAX)
    } else {
        (
            cols.saturating_mul(cell_width),
            rows.saturating_mul(cell_height),
        )
    };
    sys::MouseEncoderSize {
        size: std::mem::size_of::<sys::MouseEncoderSize>(),
        screen_width,
        screen_height,
        cell_width,
        cell_height,
        padding_top: 0,
        padding_bottom: 0,
        padding_right: 0,
        padding_left: 0,
    }
}

/// The largest cell that each format can express, as the library encodes it (X10: 222, UTF-8: 2014). A test compares
/// these with the library at the boundary. They only classify a zero result as `Unsupported(Coordinate)`.
const X10_MAX_CELL: u32 = 222;
const UTF8_MAX_CELL: u32 = 2014;

/// Whether the library takes a pixel position exactly: an `f32` holds it, and it is within the `i32` range of the
/// library's integer result.
fn pixel_is_representable(pixel: u32) -> bool {
    f64::from(pixel as f32) == f64::from(pixel) && i32::try_from(pixel).is_ok()
}

pub(crate) fn encode_mouse(
    source: Source,
    size: &Size,
    input: &MouseInput,
) -> Result<Vec<u8>, EncodeError> {
    let (event_mode, format) = match source {
        Source::Terminal(terminal) => {
            let modes = crate::modes::mode_flags(terminal);
            let state = EncoderState::from_mode_flags(&modes);
            (state.mouse_event, state.mouse_format)
        }
        Source::State(state) => (state.mouse_event, state.mouse_format),
    };

    // Under SGR pixels a position in pixels is required: Core never invents pixels from a cell.
    if format == sys::mouse_format::SGR_PIXELS && (input.x.is_none() || input.y.is_none()) {
        return Err(EncodeError::Unsupported(UnsupportedWhat::PixelPosition));
    }

    // The library takes a pixel position as `f32` and converts it to an `i32`. A position that an `f32` does not hold
    // exactly, or that is above the `i32` range, is refused, never changed (5.1A). Some integers above 2^24 are exact
    // (the next one is 2^24 + 2) and are passed on.
    if format == sys::mouse_format::SGR_PIXELS
        && [input.x, input.y]
            .iter()
            .flatten()
            .any(|p| !pixel_is_representable(*p))
    {
        return Err(EncodeError::Unsupported(UnsupportedWhat::Coordinate));
    }

    let mut encoder: sys::MouseEncoder = std::ptr::null_mut();
    // SAFETY: a valid out pointer; a null allocator selects the default.
    let code = unsafe { sys::ghostty_mouse_encoder_new(std::ptr::null(), &mut encoder) };
    if !created(code, encoder) {
        return Err(EncodeError::Unsupported(UnsupportedWhat::Other));
    }
    let encoder = MouseEncoder(encoder);
    let mouse_size = encoder_size(
        size.cols,
        size.rows,
        size.cell_px,
        format == sys::mouse_format::SGR_PIXELS,
    );
    let any_button_pressed = !matches!(input.button, MouseButton::None);
    // SAFETY: the encoder is live; each option takes the type that is passed, and the library copies the values.
    unsafe {
        sys::ghostty_mouse_encoder_setopt(
            encoder.0,
            sys::mouse_opt::EVENT,
            (&event_mode as *const i32).cast(),
        );
        sys::ghostty_mouse_encoder_setopt(
            encoder.0,
            sys::mouse_opt::FORMAT,
            (&format as *const i32).cast(),
        );
        sys::ghostty_mouse_encoder_setopt(
            encoder.0,
            sys::mouse_opt::SIZE,
            (&mouse_size as *const sys::MouseEncoderSize).cast(),
        );
        sys::ghostty_mouse_encoder_setopt(
            encoder.0,
            sys::mouse_opt::ANY_BUTTON_PRESSED,
            (&any_button_pressed as *const bool).cast(),
        );
    }

    let mut event: sys::MouseEvent = std::ptr::null_mut();
    // SAFETY: a valid out pointer; a null allocator selects the default.
    let code = unsafe { sys::ghostty_mouse_event_new(std::ptr::null(), &mut event) };
    if !created(code, event) {
        return Err(EncodeError::Unsupported(UnsupportedWhat::Other));
    }
    let event = MouseEventHandle(event);

    let action = match input.action {
        MouseAction::Press | MouseAction::Wheel => sys::mouse_action::PRESS,
        MouseAction::Release => sys::mouse_action::RELEASE,
        MouseAction::Move => sys::mouse_action::MOTION,
    };
    let button = match input.button {
        MouseButton::Left => Some(sys::mouse_button::LEFT),
        MouseButton::Middle => Some(sys::mouse_button::MIDDLE),
        MouseButton::Right => Some(sys::mouse_button::RIGHT),
        MouseButton::Back => Some(sys::mouse_button::EIGHT),
        MouseButton::Forward => Some(sys::mouse_button::NINE),
        MouseButton::WheelUp => Some(sys::mouse_button::FOUR),
        MouseButton::WheelDown => Some(sys::mouse_button::FIVE),
        MouseButton::WheelLeft => Some(sys::mouse_button::SIX),
        MouseButton::WheelRight => Some(sys::mouse_button::SEVEN),
        MouseButton::None => None,
    };
    // The encodings carry shift, alt and ctrl only.
    let mods = mod_bits(&input.mods) & (sys::mods::SHIFT | sys::mods::ALT | sys::mods::CTRL);
    // SAFETY: the event is live.
    unsafe {
        sys::ghostty_mouse_event_set_action(event.0, action);
        match button {
            Some(button) => sys::ghostty_mouse_event_set_button(event.0, button),
            None => sys::ghostty_mouse_event_clear_button(event.0),
        }
        sys::ghostty_mouse_event_set_mods(event.0, mods);
        sys::ghostty_mouse_event_set_cell(
            event.0,
            sys::MouseCell {
                col: input.col,
                row: input.row,
            },
        );
        if let (Some(x), Some(y)) = (input.x, input.y) {
            sys::ghostty_mouse_event_set_position(
                event.0,
                sys::MousePosition {
                    x: x as f32,
                    y: y as f32,
                },
            );
        }
    }

    let reports = if input.action == MouseAction::Wheel {
        input.notches.unwrap_or(1).max(1)
    } else {
        1
    };
    let mut out = Vec::new();
    for _ in 0..reports {
        let bytes = run_encoder(|buf, len, written| {
            // SAFETY: the encoder and the event are live; the buffer holds `len` bytes.
            unsafe { sys::ghostty_mouse_encoder_encode(encoder.0, event.0, buf, len, written) }
        })?;
        out.extend_from_slice(&bytes);
    }
    if !out.is_empty() {
        return Ok(out);
    }

    let unexpressible = match format {
        sys::mouse_format::X10 => input.col > X10_MAX_CELL || input.row > X10_MAX_CELL,
        sys::mouse_format::UTF8 => input.col > UTF8_MAX_CELL || input.row > UTF8_MAX_CELL,
        _ => false,
    };
    if event_mode != sys::mouse_event::NONE && unexpressible {
        return Err(EncodeError::Unsupported(UnsupportedWhat::Coordinate));
    }
    Err(EncodeError::NotReported)
}

// ---- focus and paste ----

/// The bytes of a focus event, or `None` when focus reporting is off (IN-9).
pub(crate) fn encode_focus(focus_reporting: bool, focused: bool) -> Option<Vec<u8>> {
    if !focus_reporting {
        return None;
    }
    let event = if focused {
        sys::focus::GAINED
    } else {
        sys::focus::LOST
    };
    run_encoder(|buf, len, written| {
        // SAFETY: the buffer holds `len` bytes, or is null with length 0 to ask for the size.
        unsafe { sys::ghostty_focus_encode(event, buf, len, written) }
    })
    .ok()
    .filter(|bytes| !bytes.is_empty())
}

/// The marker bytes around a paste when bracketed paste is on, else `None` (IN-8). The payload is never touched.
pub(crate) fn paste_frame(bracketed: bool) -> Option<(Vec<u8>, Vec<u8>)> {
    if !bracketed {
        return None;
    }
    let mut frame = sys::PasteFrame {
        prefix: sys::GString {
            ptr: std::ptr::null(),
            len: 0,
        },
        suffix: sys::GString {
            ptr: std::ptr::null(),
            len: 0,
        },
    };
    // SAFETY: `frame` is a valid out pointer. The library stores strings of static data.
    unsafe { sys::ghostty_paste_frame(true, &mut frame) };
    // SAFETY: the strings are static and valid for the life of the process.
    Some(unsafe { (frame.prefix.bytes().to_vec(), frame.suffix.bytes().to_vec()) })
}

// ---- the worst case over every mode (5.1A) ----

/// The bits of `ModeFlags.kitty_flags` that 5.1A defines (1, 2, 4, 8 and 16).
const KITTY_FLAG_BITS: u32 = 5;

/// The key modes of `EncoderState` besides the kitty flags.
const KEY_MODE_BITS: u32 = 6;

impl EncoderState {
    /// Every state that the key encoder reads: each of the six key modes on and off, with each of the 32 kitty flag
    /// combinations (5.1A: "any legacy or kitty flag combination"). The kitty flags are the low bits, so each run of 32
    /// states holds every flag combination, and a key whose associated text is over a limit reaches a state that reports
    /// it within the first run.
    fn every_key_state() -> impl Iterator<Item = EncoderState> {
        (0u32..1 << (KEY_MODE_BITS + KITTY_FLAG_BITS)).map(|bits| {
            let mode = |bit: u32| bits & (1 << (KITTY_FLAG_BITS + bit)) != 0;
            EncoderState {
                cursor_key_application: mode(0),
                keypad_key_application: mode(1),
                ignore_keypad_with_numlock: mode(2),
                alt_esc_prefix: mode(3),
                modify_other_keys_state_2: mode(4),
                backarrow_key_mode: mode(5),
                kitty_flags: (bits & ((1 << KITTY_FLAG_BITS) - 1)) as u8,
                mouse_event: sys::mouse_event::NONE,
                mouse_format: sys::mouse_format::X10,
            }
        })
    }

    /// Every state that the mouse encoder reads: each tracking mode with each format (5.1A: "any mouse encoding").
    fn every_mouse_state() -> impl Iterator<Item = EncoderState> {
        const EVENTS: [i32; 5] = [
            sys::mouse_event::NONE,
            sys::mouse_event::X10,
            sys::mouse_event::NORMAL,
            sys::mouse_event::BUTTON,
            sys::mouse_event::ANY,
        ];
        const FORMATS: [i32; 5] = [
            sys::mouse_format::X10,
            sys::mouse_format::UTF8,
            sys::mouse_format::SGR,
            sys::mouse_format::URXVT,
            sys::mouse_format::SGR_PIXELS,
        ];
        EVENTS.into_iter().flat_map(|mouse_event| {
            FORMATS.into_iter().map(move |mouse_format| EncoderState {
                cursor_key_application: false,
                keypad_key_application: false,
                ignore_keypad_with_numlock: false,
                alt_esc_prefix: false,
                modify_other_keys_state_2: false,
                backarrow_key_mode: false,
                kitty_flags: 0,
                mouse_event,
                mouse_format,
            })
        })
    }
}

/// The length that an encode call would write, from its size probe (see `run_encoder`). A call that fails writes nothing.
fn encoded_len(mut call: impl FnMut(*mut u8, usize, *mut usize) -> sys::Result) -> u64 {
    let mut needed: usize = 0;
    match call(std::ptr::null_mut(), 0, &mut needed) {
        sys::OUT_OF_SPACE => needed as u64,
        _ => 0,
    }
}

/// The longest sequence that libghostty writes for one event of `input` in any state of the key encoder (5.1A: the
/// worst-case bound, "including every modifier parameter and associated text"). It stops at the first state that
/// writes more than `limit` and returns that length, so a key that is too large costs no more states.
pub fn longest_key_sequence(input: &KeyInput, limit: u64) -> Result<u64, crate::Error> {
    let encoder = KeyEncoder::new()?;
    let text = input.text.as_deref().unwrap_or("");
    let mut base_text = [0u8; 4];
    // A key that has no event is never written in any state.
    let Ok(event) = key_event(input, text, &mut base_text) else {
        return Ok(0);
    };
    let mut longest = 0;
    for state in EncoderState::every_key_state() {
        encoder.configure(Source::State(state));
        longest = longest.max(encoded_len(|buf, len, written| {
            // SAFETY: the encoder and the event are live; the buffer holds `len` bytes, or is null with length 0.
            unsafe { sys::ghostty_key_encoder_encode(encoder.0, event.0, buf, len, written) }
        }));
        if longest > limit {
            break;
        }
    }
    Ok(longest)
}

/// The longest report that libghostty writes for one notch or one event of `input` in any state of the mouse encoder
/// (5.1A: "the longest report of any mouse encoding"). The screen is the largest that the encoder takes, so no position
/// is outside it.
pub fn longest_mouse_report(input: &MouseInput) -> u64 {
    let one = MouseInput {
        notches: None,
        ..input.clone()
    };
    let screen = Size {
        rows: u32::MAX,
        cols: u32::MAX,
        cell_px: None,
    };
    EncoderState::every_mouse_state()
        .filter_map(|state| encode_mouse(Source::State(state), &screen, &one).ok())
        .map(|bytes| bytes.len() as u64)
        .max()
        .unwrap_or(0)
}

/// The longest report that libghostty writes for a focus event in any mode (it writes one only with focus reporting on).
pub fn longest_focus_report(focused: bool) -> u64 {
    encode_focus(true, focused).map_or(0, |bytes| bytes.len() as u64)
}

// A pointer to a `c_void` is the type of every handle above.
const _: () = {
    let _ = std::mem::size_of::<*mut c_void>();
};
