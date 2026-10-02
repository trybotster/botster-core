//! Tests of the encoders. No test writes the bytes of a terminal sequence by hand: libghostty is the oracle (BUILD.md,
//! architecture rule 2). The tests compare one path of the binding with another, with a native function, or check a
//! property of the bytes.

use botster_core_contract::prelude::{CellPx, KeyInput, MouseInput, Size, UnsupportedWhat};
use botster_route_codec::prelude::{
    Key, KeyEvent, ModeFlags, Modifier, MouseAction, MouseButton, MouseEncoding, MouseTracking,
    NamedKey,
};

use super::*;

fn size(cols: u32, rows: u32) -> Size {
    Size {
        rows,
        cols,
        cell_px: None,
    }
}

fn terminal() -> Terminal {
    Terminal::new(&size(80, 24), History::On).unwrap()
}

fn key(key: Key, mods: &[Modifier]) -> KeyInput {
    KeyInput {
        key,
        shifted_key: None,
        base_layout_key: None,
        mods: mods.to_vec(),
        event: KeyEvent::Press,
        text: None,
        repeat: None,
    }
}

fn named(name: &str, mods: &[Modifier]) -> KeyInput {
    key(Key::Named(NamedKey(name.to_owned())), mods)
}

fn character(c: char, mods: &[Modifier]) -> KeyInput {
    key(Key::Char(c.to_string()), mods)
}

fn mouse(action: MouseAction, button: MouseButton, col: u32, row: u32) -> MouseInput {
    MouseInput {
        action,
        button,
        row,
        col,
        x: None,
        y: None,
        mods: vec![],
        notches: None,
    }
}

/// Modes after a terminal received `bytes`.
fn terminal_with(bytes: &[u8]) -> Terminal {
    let mut terminal = terminal();
    terminal.vt_write(bytes);
    terminal
}

// ---- the terminal path and the explicit-modes path agree ----

#[test]
fn the_terminal_and_the_explicit_modes_give_the_same_key_bytes() {
    // Modes are set by the sequences that a program writes; the explicit path gets the flags that were read back.
    let setups: [&[u8]; 5] = [b"", b"\x1b[?1h", b"\x1b[?66h", b"\x1b[>1u", b"\x1b[>31u"];
    let keys = [
        named("arrow_up", &[]),
        named("arrow_up", &[Modifier::Ctrl]),
        named("enter", &[]),
        named("f5", &[Modifier::Shift]),
        named("kp_5", &[]),
        character('a', &[Modifier::Ctrl]),
        character('a', &[Modifier::Alt]),
        {
            let mut k = character('a', &[Modifier::Shift]);
            k.text = Some("A".into());
            k
        },
    ];
    for setup in setups {
        let terminal = terminal_with(setup);
        let modes = terminal.modes();
        for input in &keys {
            assert_eq!(
                terminal.encode_key(input),
                encode_key_with_modes(&modes, input),
                "setup {setup:?} key {input:?}"
            );
        }
    }
}

#[test]
fn modes_change_the_bytes_that_the_library_writes() {
    let plain = terminal();
    let application = terminal_with(b"\x1b[?1h");
    let up = named("arrow_up", &[]);
    // Application cursor keys change the cursor key sequence.
    assert_ne!(
        plain.encode_key(&up).unwrap(),
        application.encode_key(&up).unwrap()
    );

    // Kitty flags change a key that they disambiguate: escape.
    let kitty = terminal_with(b"\x1b[>1u");
    let escape = named("escape", &[]);
    assert_ne!(
        plain.encode_key(&escape).unwrap(),
        kitty.encode_key(&escape).unwrap()
    );

    // The keypad in application mode changes the keypad enter key, once mode 1035 (ignore keypad with NumLock, on by
    // default) is off.
    let keypad = terminal_with(b"\x1b[?1035l\x1b[?66h");
    let kp = named("kp_enter", &[]);
    assert_ne!(
        plain.encode_key(&kp).unwrap(),
        keypad.encode_key(&kp).unwrap()
    );
}

// ---- keys (IN-9, 5.1A) ----

#[test]
fn a_named_key_outside_the_set_is_unsupported() {
    let terminal = terminal();
    assert_eq!(
        terminal.encode_key(&named("not_a_key", &[])),
        Err(EncodeError::Unsupported(UnsupportedWhat::NamedKey))
    );
}

#[test]
fn every_named_key_of_the_contract_has_a_library_key() {
    for name in botster_route_codec::prelude::named_key_names() {
        assert!(
            encode::named_key(&name).is_some(),
            "no library key for {name}"
        );
    }
    assert_eq!(encode::named_key("f0"), None);
    assert_eq!(encode::named_key("f36"), None);
}

#[test]
fn the_named_key_table_matches_the_header_of_the_pinned_ghostty() {
    use std::collections::BTreeMap;

    // The header lists `GHOSTTY_KEY_*` in order from 0, with explicit values from time to time.
    let header = include_str!("../vendor/ghostty/include/ghostty/vt/key/event.h");
    let start = header.find("GHOSTTY_KEY_UNIDENTIFIED").unwrap();
    let body = &header[start
        ..header[start..]
            .find("} GhosttyKey;")
            .map(|i| start + i)
            .unwrap()];
    let mut values = BTreeMap::new();
    let mut next = 0i32;
    for token in body.split(',') {
        let token = token.trim();
        let Some(name) = token
            .split_whitespace()
            .find(|word| word.starts_with("GHOSTTY_KEY_"))
        else {
            continue;
        };
        if let Some((_, value)) = token.split_once('=') {
            // `GHOSTTY_KEY_MAX_VALUE = GHOSTTY_ENUM_MAX_VALUE` is not a number.
            if let Some(number) = value
                .split_whitespace()
                .next()
                .and_then(|word| word.parse().ok())
            {
                next = number;
            }
        }
        values.insert(name.to_owned(), next);
        next += 1;
    }

    let expect = |name: &str, header_name: &str| {
        let key = encode::named_key(name).unwrap_or_else(|| panic!("no key for {name}"));
        assert_eq!(
            values.get(header_name),
            Some(&key),
            "{name} is {header_name}"
        );
    };
    for (name, key) in encode::NAMED_KEYS {
        let header_name = match *name {
            "kp_decimal" => "GHOSTTY_KEY_NUMPAD_DECIMAL".to_owned(),
            "kp_divide" => "GHOSTTY_KEY_NUMPAD_DIVIDE".to_owned(),
            "kp_multiply" => "GHOSTTY_KEY_NUMPAD_MULTIPLY".to_owned(),
            "kp_subtract" => "GHOSTTY_KEY_NUMPAD_SUBTRACT".to_owned(),
            "kp_add" => "GHOSTTY_KEY_NUMPAD_ADD".to_owned(),
            "kp_enter" => "GHOSTTY_KEY_NUMPAD_ENTER".to_owned(),
            "kp_equal" => "GHOSTTY_KEY_NUMPAD_EQUAL".to_owned(),
            "menu" => "GHOSTTY_KEY_CONTEXT_MENU".to_owned(),
            "left_super" => "GHOSTTY_KEY_META_LEFT".to_owned(),
            "right_super" => "GHOSTTY_KEY_META_RIGHT".to_owned(),
            "left_shift" => "GHOSTTY_KEY_SHIFT_LEFT".to_owned(),
            "right_shift" => "GHOSTTY_KEY_SHIFT_RIGHT".to_owned(),
            "left_control" => "GHOSTTY_KEY_CONTROL_LEFT".to_owned(),
            "right_control" => "GHOSTTY_KEY_CONTROL_RIGHT".to_owned(),
            "left_alt" => "GHOSTTY_KEY_ALT_LEFT".to_owned(),
            "right_alt" => "GHOSTTY_KEY_ALT_RIGHT".to_owned(),
            other if other.starts_with("kp_") => format!("GHOSTTY_KEY_NUMPAD_{}", &other[3..]),
            other => format!("GHOSTTY_KEY_{}", other.to_uppercase()),
        };
        assert_eq!(
            values.get(&header_name),
            Some(key),
            "{name} is {header_name}"
        );
    }
    for number in 1..=35 {
        expect(&format!("f{number}"), &format!("GHOSTTY_KEY_F{number}"));
    }
}

#[test]
fn ctrl_and_a_letter_is_a_key_of_its_own_for_every_letter() {
    // The library decides the bytes. The test states only what must hold of any encoding: each letter gives a
    // sequence, no two letters give the same one, and none is the plain letter.
    let terminal = terminal();
    let mut seen = std::collections::BTreeSet::new();
    let mut one_byte = 0;
    for letter in 'a'..='z' {
        let bytes = terminal
            .encode_key(&character(letter, &[Modifier::Ctrl]))
            .unwrap();
        assert!(!bytes.is_empty(), "ctrl+{letter}");
        let mut plain = character(letter, &[]);
        plain.text = Some(letter.to_string());
        assert_ne!(bytes, terminal.encode_key(&plain).unwrap());
        if bytes.len() == 1 {
            one_byte += 1;
        }
        assert!(seen.insert(bytes), "ctrl+{letter} repeats another letter");
    }
    // Most letters have a one-byte control code. The ones that share a code with another key (ctrl+i, ctrl+m) do not.
    assert!(one_byte >= 20, "{one_byte} letters gave one byte");
}

#[test]
fn alt_and_a_character_prefixes_the_base_character_when_the_mode_asks_for_it() {
    // DEC mode 1036 makes Alt send an escape prefix (it is in other_modes). Core's Alt is the Alt key, on every
    // operating system, so the macOS option setting must not hide the prefix.
    let terminal = terminal_with(b"\x1b[?1036h");
    let mut plain_key = character('a', &[]);
    plain_key.text = Some("a".to_owned());
    let plain = terminal.encode_key(&plain_key).unwrap();
    let alt = terminal
        .encode_key(&character('a', &[Modifier::Alt]))
        .unwrap();
    // One prefix byte, then the same bytes as the plain key.
    assert_eq!(alt.len(), plain.len() + 1);
    assert_eq!(&alt[1..], &plain[..]);
}

#[test]
fn text_is_written_as_it_is_in_legacy_mode() {
    let terminal = terminal();
    for text in ["a", "A", "é", "日", "😀"] {
        let mut input = character('x', &[]);
        input.text = Some(text.to_owned());
        assert_eq!(
            terminal.encode_key(&input).unwrap(),
            text.as_bytes(),
            "text {text:?}"
        );
    }
}

#[test]
fn shift_with_no_text_uses_the_supplied_shifted_key_else_ascii_else_nothing() {
    let terminal = terminal();

    // Supplied: the same bytes as the key that carries that text.
    let mut supplied = character('a', &[Modifier::Shift]);
    supplied.shifted_key = Some(Key::Char("A".into()));
    let mut with_text = character('a', &[Modifier::Shift]);
    with_text.text = Some("A".into());
    assert_eq!(
        terminal.encode_key(&supplied).unwrap(),
        terminal.encode_key(&with_text).unwrap()
    );

    // A non-US layout: the supplied key decides.
    let mut accented = character('é', &[Modifier::Shift]);
    accented.shifted_key = Some(Key::Char("É".into()));
    assert_eq!(terminal.encode_key(&accented).unwrap(), "É".as_bytes());

    // Not supplied: a to z become capitals.
    let mut capital = character('q', &[Modifier::Shift]);
    capital.text = Some("Q".to_owned());
    assert_eq!(
        terminal
            .encode_key(&character('q', &[Modifier::Shift]))
            .unwrap(),
        terminal.encode_key(&capital).unwrap()
    );

    // Nothing else is guessed.
    for c in ['1', ';', 'é'] {
        assert_eq!(
            terminal.encode_key(&character(c, &[Modifier::Shift])),
            Err(EncodeError::Unsupported(UnsupportedWhat::ProducedText)),
            "shift+{c}"
        );
    }
}

#[test]
fn a_release_is_not_reported_in_legacy_mode_and_is_in_kitty_mode_with_event_types() {
    let legacy = terminal();
    let mut release = named("arrow_up", &[]);
    release.event = KeyEvent::Release;
    assert_eq!(legacy.encode_key(&release), Err(EncodeError::NotReported));

    // Flags 1 (disambiguate) and 2 (report event types).
    let kitty = terminal_with(b"\x1b[>3u");
    assert!(!kitty.encode_key(&release).unwrap().is_empty());
    // Flag 1 alone does not report releases.
    let flag_one = terminal_with(b"\x1b[>1u");
    assert_eq!(flag_one.encode_key(&release), Err(EncodeError::NotReported));
}

#[test]
fn hyper_and_meta_reach_the_kitty_encoder_as_distinct_modifiers() {
    let terminal = terminal_with(b"\x1b[>1u");
    let bytes = |mods: &[Modifier]| terminal.encode_key(&character('a', mods)).unwrap();
    let hyper = bytes(&[Modifier::Hyper]);
    let meta = bytes(&[Modifier::Meta]);
    let super_ = bytes(&[Modifier::Super]);
    assert_ne!(hyper, meta);
    assert_ne!(hyper, super_);
    assert_ne!(meta, super_);
    assert_ne!(hyper, bytes(&[]));
}

#[test]
fn kitty_alternate_keys_are_reported_only_when_supplied() {
    // Flags 1 and 4: disambiguate and report alternate keys.
    let terminal = terminal_with(b"\x1b[>5u");
    let plain = character('a', &[Modifier::Ctrl]);
    let mut with_base = plain.clone();
    with_base.base_layout_key = Some(Key::Char("q".into()));
    assert_ne!(
        terminal.encode_key(&plain).unwrap(),
        terminal.encode_key(&with_base).unwrap()
    );
    // The base layout key is the only difference: removing it again gives the first bytes.
    with_base.base_layout_key = None;
    assert_eq!(
        terminal.encode_key(&plain).unwrap(),
        terminal.encode_key(&with_base).unwrap()
    );
}

#[test]
fn f26_to_f35_are_keys() {
    let terminal = terminal_with(b"\x1b[>1u");
    let mut seen = std::collections::BTreeSet::new();
    for number in 1..=35 {
        let bytes = terminal
            .encode_key(&named(&format!("f{number}"), &[]))
            .unwrap();
        assert!(!bytes.is_empty(), "f{number}");
        assert!(seen.insert(bytes), "f{number} repeats another key");
    }
}

// ---- mouse (IN-9, 5.1A, R-13) ----

fn mouse_modes(tracking: MouseTracking, encoding: MouseEncoding) -> ModeFlags {
    ModeFlags {
        mouse_tracking: tracking,
        mouse_encoding: encoding,
        ..ModeFlags::default()
    }
}

#[test]
fn the_terminal_and_the_explicit_modes_give_the_same_mouse_bytes() {
    let setups: [&[u8]; 4] = [
        b"\x1b[?1000h",
        b"\x1b[?1000h\x1b[?1006h",
        b"\x1b[?1003h\x1b[?1015h",
        b"\x1b[?1000h\x1b[?1005h",
    ];
    for setup in setups {
        let terminal = terminal_with(setup);
        let modes = terminal.modes();
        let size = size(80, 24);
        for input in [
            mouse(MouseAction::Press, MouseButton::Left, 3, 4),
            mouse(MouseAction::Release, MouseButton::Left, 3, 4),
            mouse(MouseAction::Wheel, MouseButton::WheelUp, 3, 4),
        ] {
            assert_eq!(
                terminal.encode_mouse(&input),
                encode_mouse_with_modes(&modes, &size, &input),
                "{setup:?} {input:?}"
            );
        }
    }
}

#[test]
fn nothing_is_reported_when_tracking_is_off() {
    let terminal = terminal();
    assert_eq!(
        terminal.encode_mouse(&mouse(MouseAction::Press, MouseButton::Left, 1, 1)),
        Err(EncodeError::NotReported)
    );
}

#[test]
fn a_cell_outside_the_screen_is_reported_as_given_whatever_the_screen_size() {
    let modes = mouse_modes(MouseTracking::AnyEvent, MouseEncoding::Sgr);
    for action in [MouseAction::Press, MouseAction::Release, MouseAction::Move] {
        let input = mouse(action, MouseButton::Left, 300, 5);
        let small = encode_mouse_with_modes(&modes, &size(80, 24), &input).unwrap();
        let large = encode_mouse_with_modes(&modes, &size(500, 100), &input).unwrap();
        assert!(!small.is_empty());
        assert_eq!(small, large, "{action:?}");
    }
    // The cell is in the bytes: another column gives other bytes.
    let a = encode_mouse_with_modes(
        &modes,
        &size(80, 24),
        &mouse(MouseAction::Press, MouseButton::Left, 300, 5),
    )
    .unwrap();
    let b = encode_mouse_with_modes(
        &modes,
        &size(80, 24),
        &mouse(MouseAction::Press, MouseButton::Left, 301, 5),
    )
    .unwrap();
    assert_ne!(a, b);
}

#[test]
fn the_edge_cells_of_the_legacy_formats_are_classified_with_the_library_boundary() {
    // X10: the library encodes cell 222 and refuses 223.
    let x10 = mouse_modes(MouseTracking::Normal, MouseEncoding::X10);
    let at = |col: u32, row: u32| {
        encode_mouse_with_modes(
            &x10,
            &size(80, 24),
            &mouse(MouseAction::Press, MouseButton::Left, col, row),
        )
    };
    assert!(at(222, 222).is_ok());
    assert_eq!(
        at(223, 0),
        Err(EncodeError::Unsupported(UnsupportedWhat::Coordinate))
    );
    assert_eq!(
        at(0, 223),
        Err(EncodeError::Unsupported(UnsupportedWhat::Coordinate))
    );

    // UTF-8: the library encodes cell 2014 and refuses 2015.
    let utf8 = mouse_modes(MouseTracking::Normal, MouseEncoding::Utf8);
    let at = |col: u32, row: u32| {
        encode_mouse_with_modes(
            &utf8,
            &size(80, 24),
            &mouse(MouseAction::Press, MouseButton::Left, col, row),
        )
    };
    assert!(at(2014, 2014).is_ok());
    assert_eq!(
        at(2015, 0),
        Err(EncodeError::Unsupported(UnsupportedWhat::Coordinate))
    );

    // SGR has no limit.
    let sgr = mouse_modes(MouseTracking::Normal, MouseEncoding::Sgr);
    assert!(encode_mouse_with_modes(
        &sgr,
        &size(80, 24),
        &mouse(MouseAction::Press, MouseButton::Left, u32::MAX, u32::MAX)
    )
    .is_ok());
}

#[test]
fn sgr_pixels_needs_a_pixel_position_and_reports_it_without_an_offset() {
    let modes = mouse_modes(MouseTracking::Normal, MouseEncoding::SgrPixels);
    let cells = size(80, 24);

    // No x and y: Core never invents pixels from a cell.
    assert_eq!(
        encode_mouse_with_modes(
            &modes,
            &cells,
            &mouse(MouseAction::Press, MouseButton::Left, 3, 4)
        ),
        Err(EncodeError::Unsupported(UnsupportedWhat::PixelPosition))
    );

    // With pixels, a zero-based position that moves by one pixel moves the report by one pixel (R-13: no +1 and no
    // cell conversion). The two reports differ in the x parameter only.
    let at = |x: u32| {
        let mut input = mouse(MouseAction::Press, MouseButton::Left, 3, 4);
        input.x = Some(x);
        input.y = Some(7);
        encode_mouse_with_modes(
            &modes,
            &Size {
                rows: 24,
                cols: 80,
                cell_px: Some(CellPx {
                    width: 9,
                    height: 18,
                }),
            },
            &input,
        )
        .unwrap()
    };
    let (a, b) = (at(40), at(41));
    let text = |bytes: &[u8]| String::from_utf8(bytes.to_vec()).unwrap();
    let numbers = |bytes: &[u8]| -> Vec<i64> {
        text(bytes)
            .trim_start_matches(|c: char| !c.is_ascii_digit())
            .split(|c: char| !c.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse().unwrap())
            .collect()
    };
    let (na, nb) = (numbers(&a), numbers(&b));
    assert_eq!(na.len(), 3);
    assert_eq!(nb[1] - na[1], 1, "x moved by one pixel");
    assert_eq!(na[0], nb[0]);
    assert_eq!(na[2], nb[2]);
    // The reported pixels are the given ones, as zero-based values.
    assert_eq!((na[1], na[2]), (40, 7));
}

#[test]
fn wheel_notches_are_that_many_reports() {
    let modes = mouse_modes(MouseTracking::Normal, MouseEncoding::Sgr);
    let single = encode_mouse_with_modes(
        &modes,
        &size(80, 24),
        &mouse(MouseAction::Wheel, MouseButton::WheelUp, 2, 2),
    )
    .unwrap();
    for notches in [1u32, 2, 5] {
        let mut input = mouse(MouseAction::Wheel, MouseButton::WheelUp, 2, 2);
        input.notches = Some(notches);
        let bytes = encode_mouse_with_modes(&modes, &size(80, 24), &input).unwrap();
        assert_eq!(bytes, single.repeat(notches as usize));
    }
}

#[test]
fn the_four_wheel_directions_and_the_two_extra_buttons_are_distinct_reports() {
    let modes = mouse_modes(MouseTracking::Normal, MouseEncoding::Sgr);
    let report = |action: MouseAction, button: MouseButton| {
        encode_mouse_with_modes(&modes, &size(80, 24), &mouse(action, button, 1, 1)).unwrap()
    };
    let mut seen = std::collections::BTreeSet::new();
    for button in [
        MouseButton::WheelUp,
        MouseButton::WheelDown,
        MouseButton::WheelLeft,
        MouseButton::WheelRight,
    ] {
        assert!(
            seen.insert(report(MouseAction::Wheel, button)),
            "{button:?}"
        );
    }
    for button in [
        MouseButton::Left,
        MouseButton::Middle,
        MouseButton::Right,
        MouseButton::Back,
        MouseButton::Forward,
    ] {
        assert!(
            seen.insert(report(MouseAction::Press, button)),
            "{button:?}"
        );
    }
}

// ---- focus and paste (IN-8, IN-9) ----

#[test]
fn focus_is_written_only_when_focus_reporting_is_on() {
    let off = terminal();
    assert_eq!(off.encode_focus(true), None);
    assert_eq!(off.encode_focus(false), None);

    let on = terminal_with(b"\x1b[?1004h");
    let gained = on.encode_focus(true).unwrap();
    let lost = on.encode_focus(false).unwrap();
    assert!(!gained.is_empty());
    assert_ne!(gained, lost);
}

#[test]
fn the_paste_frame_is_what_the_library_writes_around_an_empty_paste() {
    assert_eq!(terminal().paste_frame(), None);

    let bracketed = terminal_with(b"\x1b[?2004h");
    let (prefix, suffix) = bracketed.paste_frame().unwrap();
    assert!(!prefix.is_empty() && !suffix.is_empty());

    // The library's own paste encoder is the oracle: an empty paste is the prefix and then the suffix.
    let mut out = [0u8; 64];
    let mut written = 0usize;
    // SAFETY: a null payload of length 0 is an empty paste, and the buffer holds 64 bytes.
    let code = unsafe {
        sys::ghostty_paste_encode(
            std::ptr::null_mut(),
            0,
            true,
            out.as_mut_ptr(),
            out.len(),
            &mut written,
        )
    };
    assert_eq!(code, sys::SUCCESS);
    assert_eq!([prefix, suffix].concat(), &out[..written]);
}

#[test]
fn a_pixel_position_that_an_f32_cannot_hold_is_refused_not_changed() {
    let modes = mouse_modes(MouseTracking::AnyEvent, MouseEncoding::SgrPixels);
    let cells = Size {
        rows: 24,
        cols: 80,
        cell_px: Some(CellPx {
            width: 9,
            height: 18,
        }),
    };
    let report = |action: MouseAction, button: MouseButton, x: u32, y: u32| {
        let mut input = mouse(action, button, 3, 4);
        input.x = Some(x);
        input.y = Some(y);
        encode_mouse_with_modes(&modes, &cells, &input)
    };
    let digits = |bytes: &[u8]| -> Vec<u64> {
        bytes
            .split(|b| !b.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .map(|part| std::str::from_utf8(part).unwrap().parse().unwrap())
            .collect()
    };

    // Releases are reported outside the viewport, so they carry any position that the library takes. Every integer up
    // to 2^24 is exact in an f32; above it only even numbers are, then multiples of four, and so on. The library's
    // integer result ends at i32::MAX, and 2^31 - 128 is the last f32 below it.
    let exact: u32 = 1 << 24;
    for value in [
        0,
        1,
        exact - 1,
        exact,
        exact + 2,
        exact + 4,
        (1 << 31) - 128,
    ] {
        let bytes = report(MouseAction::Release, MouseButton::Left, value, value).unwrap();
        assert!(
            digits(&bytes)[1..].iter().all(|n| *n == u64::from(value)),
            "release at {value}"
        );
    }
    // A press and a move inside the viewport carry theirs too.
    for (action, button) in [
        (MouseAction::Press, MouseButton::Left),
        (MouseAction::Move, MouseButton::None),
    ] {
        let bytes = report(action, button, 700, 300).unwrap();
        assert_eq!(digits(&bytes)[1..], [700, 300], "{action:?}");
    }

    // A position that an f32 rounds, one above the i32 range, and u32::MAX are refused for every action, releases
    // included, on each axis.
    for value in [
        exact + 1,
        exact + 3,
        exact + 5,
        i32::MAX as u32,
        (1 << 31),
        (1 << 31) + 128,
        u32::MAX,
    ] {
        for (action, button) in [
            (MouseAction::Press, MouseButton::Left),
            (MouseAction::Release, MouseButton::Left),
            (MouseAction::Move, MouseButton::None),
        ] {
            let expected = Err(EncodeError::Unsupported(UnsupportedWhat::Coordinate));
            assert_eq!(report(action, button, value, 0), expected, "x {value}");
            assert_eq!(report(action, button, 0, value), expected, "y {value}");
        }
    }
}

#[test]
fn sgr_pixels_without_a_cell_size_reports_the_position_as_given() {
    let modes = mouse_modes(MouseTracking::Normal, MouseEncoding::SgrPixels);
    // The spawn size has no cell size, so the pixel screen is unknown and no position is outside it.
    let no_cell = size(80, 24);
    let mut press = mouse(MouseAction::Press, MouseButton::Left, 3, 4);
    press.x = Some(100);
    press.y = Some(200);
    let bytes = encode_mouse_with_modes(&modes, &no_cell, &press).unwrap();
    let numbers: Vec<u64> = bytes
        .split(|b| !b.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| std::str::from_utf8(part).unwrap().parse().unwrap())
        .collect();
    assert_eq!(numbers[1..], [100, 200]);
}

#[test]
fn a_modifier_key_alone_is_a_named_key_refusal_unless_kitty_flag_8_is_on() {
    let off = terminal();
    let on = terminal_with(b"\x1b[>8u");
    for name in [
        "left_shift",
        "left_control",
        "left_alt",
        "left_super",
        "right_shift",
        "right_control",
        "right_alt",
        "right_super",
    ] {
        assert_eq!(
            off.encode_key(&named(name, &[])),
            Err(EncodeError::Unsupported(UnsupportedWhat::NamedKey)),
            "{name} with flag 8 off"
        );
        let bytes = on.encode_key(&named(name, &[])).unwrap();
        assert!(!bytes.is_empty(), "{name} with flag 8 on");
    }
    // Flag 1 alone does not report modifier keys.
    let flag_one = terminal_with(b"\x1b[>1u");
    assert_eq!(
        flag_one.encode_key(&named("left_shift", &[])),
        Err(EncodeError::Unsupported(UnsupportedWhat::NamedKey))
    );
}
