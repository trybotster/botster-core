//! Tests of the color profile and of modifyOtherKeys in `other_modes`. The shadow's answers come from the library, so
//! the tests compare answers with each other and with the profile's own numbers.

use botster_core_contract::prelude::{CellPx, ColorProfile, Rgb, Size};

use super::*;

fn terminal() -> Terminal {
    Terminal::new(
        &Size {
            rows: 24,
            cols: 80,
            cell_px: Some(CellPx {
                width: 9,
                height: 18,
            }),
        },
        History::On,
    )
    .unwrap()
}

fn profile(foreground: Rgb, background: Rgb, cursor: Option<Rgb>) -> ColorProfile {
    ColorProfile {
        palette: None,
        foreground,
        background,
        cursor,
    }
}

fn answer(terminal: &mut Terminal, request: &[u8]) -> Vec<u8> {
    terminal
        .vt_write_until_query(request)
        .unwrap()
        .query
        .unwrap()
        .shadow_reply
}

#[test]
fn the_foreground_background_and_cursor_answers_follow_the_profile() {
    let mut first = terminal();
    first
        .set_color_profile(&profile(
            Rgb {
                r: 0x12,
                g: 0x34,
                b: 0x56,
            },
            Rgb {
                r: 0x9a,
                g: 0xbc,
                b: 0xde,
            },
            Some(Rgb {
                r: 0x01,
                g: 0x02,
                b: 0x03,
            }),
        ))
        .unwrap();
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).unwrap().to_lowercase();
    // The answer holds each component of the profile as hex.
    let foreground = text(answer(&mut first, b"\x1b]10;?\x07"));
    assert!(
        foreground.contains("12") && foreground.contains("34") && foreground.contains("56"),
        "{foreground}"
    );
    let background = text(answer(&mut first, b"\x1b]11;?\x07"));
    assert!(
        background.contains("9a") && background.contains("bc") && background.contains("de"),
        "{background}"
    );
    let cursor = text(answer(&mut first, b"\x1b]12;?\x07"));
    assert!(
        cursor.contains("01") && cursor.contains("02") && cursor.contains("03"),
        "{cursor}"
    );

    // Another profile changes the later answers (ev_8_set_color_profile_changes_later_shadow_answers).
    let before = answer(&mut first, b"\x1b]11;?\x07");
    first
        .set_color_profile(&profile(
            Rgb { r: 0, g: 0, b: 0 },
            Rgb {
                r: 0xfe,
                g: 0xdc,
                b: 0xba,
            },
            None,
        ))
        .unwrap();
    let after = answer(&mut first, b"\x1b]11;?\x07");
    assert_ne!(before, after);
    assert!(text(after).contains("fe"));

    // The same profile on another terminal gives the same answer.
    let mut second = terminal();
    second
        .set_color_profile(&profile(
            Rgb { r: 0, g: 0, b: 0 },
            Rgb {
                r: 0xfe,
                g: 0xdc,
                b: 0xba,
            },
            None,
        ))
        .unwrap();
    assert_eq!(
        answer(&mut second, b"\x1b]11;?\x07"),
        answer(&mut first, b"\x1b]11;?\x07")
    );
}

#[test]
fn a_palette_entry_answer_follows_the_profile_and_a_short_palette_is_invalid() {
    let mut palette = vec![Rgb { r: 0, g: 0, b: 0 }; 256];
    palette[5] = Rgb {
        r: 0x7e,
        g: 0x6d,
        b: 0x5c,
    };
    let mut terminal = terminal();
    let mut full = profile(Rgb { r: 0, g: 0, b: 0 }, Rgb { r: 1, g: 1, b: 1 }, None);
    full.palette = Some(palette);
    terminal.set_color_profile(&full).unwrap();
    let reply = String::from_utf8(answer(&mut terminal, b"\x1b]4;5;?\x07"))
        .unwrap()
        .to_lowercase();
    assert!(
        reply.contains("7e") && reply.contains("6d") && reply.contains("5c"),
        "{reply}"
    );

    full.palette = Some(vec![Rgb { r: 0, g: 0, b: 0 }; 255]);
    assert_eq!(terminal.set_color_profile(&full), Err(Error::InvalidValue));
}

#[test]
fn the_shadow_has_no_color_answer_before_a_profile_is_set() {
    let mut terminal = terminal();
    assert!(answer(&mut terminal, b"\x1b]11;?\x07").is_empty());
}

#[test]
fn modify_other_keys_state_2_is_in_other_modes_and_follows_the_sequence() {
    let mut terminal = terminal();
    let name = "xterm_modify_other_keys_2";
    assert_eq!(terminal.modes().other_modes.get(name), Some(&false));
    terminal.vt_write(b"\x1b[>4;2m");
    assert_eq!(terminal.modes().other_modes.get(name), Some(&true));
    terminal.vt_write(b"\x1b[>4;0m");
    assert_eq!(terminal.modes().other_modes.get(name), Some(&false));
}
