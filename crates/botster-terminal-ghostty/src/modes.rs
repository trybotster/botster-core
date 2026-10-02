//! The modes of the model (5.1A `ModeFlags`, Core erratum 2): read from libghostty, never rebuilt from the bytes.

use std::collections::BTreeMap;

use botster_route_codec::prelude::{ModeFlags, MouseEncoding, MouseTracking};

use crate::sys;

/// The DEC modes that a normative `ModeFlags` field covers. They are not repeated in `other_modes`.
// Used by the test that compares the lists with the header, and as the record of what the normative fields cover.
#[cfg_attr(not(test), allow(dead_code))]
const NORMATIVE_DEC: &[u16] = &[
    1,    // application_cursor_keys
    9,    // mouse_tracking (X10)
    25,   // cursor_visible
    66,   // application_keypad
    1000, // mouse_tracking (normal)
    1002, // mouse_tracking (button)
    1003, // mouse_tracking (any)
    1004, // focus_reporting
    1005, // mouse_encoding (UTF-8)
    1006, // mouse_encoding (SGR)
    1015, // mouse_encoding (URXVT)
    1016, // mouse_encoding (SGR pixels)
    2004, // bracketed_paste
    47,   // alt_screen (legacy)
    1047, // alt_screen
    1049, // alt_screen
];

/// The modes that libghostty tracks and that no normative field covers (E2-2): the ones `other_modes` reports. Each is
/// `(value, ansi)`. A test compares this list and `NORMATIVE_DEC` with the header of the pinned Ghostty, so a mode that
/// a pin move adds fails the test instead of going unreported.
pub(crate) const OTHER_MODES: &[(u16, bool)] = &[
    (2, true),
    (4, true),
    (12, true),
    (20, true),
    (3, false),
    (4, false),
    (5, false),
    (6, false),
    (7, false),
    (8, false),
    (12, false),
    (40, false),
    (45, false),
    (67, false),
    (69, false),
    (1007, false),
    (1035, false),
    (1036, false),
    (1039, false),
    (1045, false),
    (1048, false),
    (2026, false),
    (2027, false),
    (2031, false),
    (2033, false),
    (2048, false),
    (5522, false),
];

/// The name of a mode in `other_modes`: `ansi_<n>` or `dec_<n>`.
pub(crate) fn mode_name(value: u16, ansi: bool) -> String {
    format!("{}_{value}", if ansi { "ansi" } else { "dec" })
}

pub(crate) fn read_mode(terminal: sys::Terminal, value: u16, ansi: bool) -> bool {
    let mut config = sys::ModeConfig {
        mode: sys::mode_new(value, ansi),
        value: false,
    };
    // SAFETY: the terminal is live, and `config` is the in/out struct of the MODE key with its mode set.
    let code = unsafe {
        sys::ghostty_terminal_get(
            terminal,
            sys::data::MODE,
            (&mut config as *mut sys::ModeConfig).cast(),
        )
    };
    debug_assert_eq!(code, sys::SUCCESS);
    code == sys::SUCCESS && config.value
}

fn get_i32(terminal: sys::Terminal, key: i32) -> i32 {
    let mut out: i32 = 0;
    // SAFETY: the terminal is live, and `key` is one whose output is a C enum of the size of an `int`.
    let code = unsafe { sys::ghostty_terminal_get(terminal, key, (&mut out as *mut i32).cast()) };
    debug_assert_eq!(code, sys::SUCCESS);
    out
}

fn get_bool(terminal: sys::Terminal, key: i32) -> bool {
    let mut out = false;
    // SAFETY: the terminal is live, and `key` is one whose output is a `bool`.
    let code = unsafe { sys::ghostty_terminal_get(terminal, key, (&mut out as *mut bool).cast()) };
    debug_assert_eq!(code, sys::SUCCESS);
    out
}

/// `GHOSTTY_MOUSE_SHIFT_CAPTURE_ON`.
const SHIFT_CAPTURE_ON: i32 = 2;

pub(crate) fn mode_flags(terminal: sys::Terminal) -> ModeFlags {
    let mut kitty: u8 = 0;
    // SAFETY: the terminal is live, and KITTY_KEYBOARD_FLAGS writes a `u8`.
    let code = unsafe {
        sys::ghostty_terminal_get(
            terminal,
            sys::data::KITTY_KEYBOARD_FLAGS,
            (&mut kitty as *mut u8).cast(),
        )
    };
    debug_assert_eq!(code, sys::SUCCESS);

    let mouse_tracking = match get_i32(terminal, sys::data::MOUSE_EVENT) {
        sys::mouse_event::NONE => MouseTracking::None,
        sys::mouse_event::X10 => MouseTracking::X10,
        sys::mouse_event::NORMAL => MouseTracking::Normal,
        sys::mouse_event::BUTTON => MouseTracking::ButtonEvent,
        sys::mouse_event::ANY => MouseTracking::AnyEvent,
        _ => MouseTracking::Other,
    };
    let mouse_encoding = match get_i32(terminal, sys::data::MOUSE_FORMAT) {
        sys::mouse_format::X10 => MouseEncoding::X10,
        sys::mouse_format::UTF8 => MouseEncoding::Utf8,
        sys::mouse_format::SGR => MouseEncoding::Sgr,
        sys::mouse_format::URXVT => MouseEncoding::Urxvt,
        sys::mouse_format::SGR_PIXELS => MouseEncoding::SgrPixels,
        _ => MouseEncoding::Other,
    };

    let mut other_modes = BTreeMap::new();
    for &(value, ansi) in OTHER_MODES {
        other_modes.insert(mode_name(value, ansi), read_mode(terminal, value, ansi));
    }

    other_modes.insert(
        MODIFY_OTHER_KEYS_2.to_owned(),
        get_bool(terminal, sys::data::MODIFY_OTHER_KEYS_2),
    );
    other_modes.insert(
        MOUSE_SHIFT_CAPTURE.to_owned(),
        get_i32(terminal, sys::data::MOUSE_SHIFT_CAPTURE) == SHIFT_CAPTURE_ON,
    );

    ModeFlags {
        alt_screen: get_i32(terminal, sys::data::ACTIVE_SCREEN) == sys::SCREEN_ALTERNATE,
        cursor_visible: get_bool(terminal, sys::data::CURSOR_VISIBLE),
        bracketed_paste: read_mode(terminal, 2004, false),
        application_cursor_keys: read_mode(terminal, 1, false),
        application_keypad: read_mode(terminal, 66, false),
        focus_reporting: read_mode(terminal, 1004, false),
        // The kitty keyboard flags are the top of the stack, masked to the five defined bits.
        kitty_flags: kitty & 0x1F,
        mouse_tracking,
        mouse_encoding,
        other_modes,
    }
}

#[cfg(test)]
pub(crate) fn normative_dec() -> &'static [u16] {
    NORMATIVE_DEC
}

/// The name in `other_modes` of xterm's modifyOtherKeys state 2 (`CSI > 4 ; 2 m`), read from the terminal's own state
/// (`GHOSTTY_TERMINAL_DATA_MODIFY_OTHER_KEYS_2`, fork patch 9). It holds whatever the kitty keyboard flags are.
pub(crate) const MODIFY_OTHER_KEYS_2: &str = "xterm_modify_other_keys_2";

/// The name in `other_modes` of XTSHIFTESCAPE (`CSI > 1 s`): true when the program turned Shift capture on. The
/// library also tells "never set" from "turned off"; both are false here.
pub(crate) const MOUSE_SHIFT_CAPTURE: &str = "xterm_mouse_shift_capture";
