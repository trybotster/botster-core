//! The comparator of `oracle_resume` (`docs/core-testkit-controls.md`, "The comparator and the real-tier form of
//! `oracle_resume`"; steward rulings R-62 and R-67). Both tiers use it.
//!
//! It compares two oracle terminals (pinned libghostty, R-7) field by field through libghostty's own state. It never runs the
//! subject's snapshot encoder (GHOSTSNP), and it touches only oracle terminals that the harness owns, never the subject: the
//! saved cursor is read by DECRC (`1b38`) on each compared terminal, after every other reading (R-62 F3b).
//!
//! The fields and their readings:
//!
//! | field | reading |
//! |---|---|
//! | `size` | rows and columns |
//! | `cell_text` | the plain text of the screen and its scrollback, and the raw cells of each visible row |
//! | `cell_attributes` | [`Terminal::cell_attributes`] of every cell of the screen and its scrollback |
//! | `hyperlinks` | [`Terminal::cell_hyperlink_uri`] of every cell of the screen and its scrollback |
//! | `cursor_position`, `cursor_visible` | [`Terminal::cursor`] |
//! | `cursor_pending_wrap` | [`Terminal::cursor_pending_wrap`] |
//! | `cursor_style` | [`Terminal::cursor_appearance`]: shape and blink |
//! | `cursor_pen` | [`Terminal::cursor_pen`]: the rendition of the next printed cell |
//! | `modes` | [`Terminal::modes`]: every mode that libghostty tracks |
//! | `colors` | [`Terminal::colors`]: the palette and the dynamic colors, current and default |
//! | `title`, `cwd` | [`Terminal::title`], [`Terminal::cwd`] |
//! | `tab_stops_margins_charsets` | [`Terminal::formatter_extras`]: libghostty's formatter, not the encoder (R-62 F3a) |
//! | `saved_cursor` | after DECRC: the cursor's position, pending wrap, style and pen, the modes (origin mode) and the charsets |

use botster_core_contract::prelude::{ColorProfile, Size};
use botster_terminal_ghostty::{Error, History, Terminal};
use std::collections::BTreeMap;

/// DECRC: restore the saved cursor.
const DECRC: &[u8] = b"\x1b8";

/// The readings of one terminal, by field. Each value is the `Debug` form of what libghostty gave.
type Readings = BTreeMap<&'static str, String>;

/// Every row of the screen and its scrollback, from the oldest: the reads give `InvalidValue` past the last row.
fn rows_with_history(terminal: &Terminal) -> Result<u32, Error> {
    let mut rows = 0u32;
    loop {
        match terminal.cell_attributes(rows, 0, true) {
            Ok(_) => rows += 1,
            Err(Error::InvalidValue) => return Ok(rows),
            Err(error) => return Err(error),
        }
    }
}

fn read(terminal: &mut Terminal) -> Result<Readings, Error> {
    let rows = rows_with_history(terminal)?;
    let cols = u32::from(terminal.cols());
    let mut attributes = Vec::new();
    let mut hyperlinks = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            attributes.push(terminal.cell_attributes(row, col, true)?);
            hyperlinks.push(terminal.cell_hyperlink_uri(row, col, true)?);
        }
    }
    let visible: Vec<Option<Vec<String>>> = (0..u32::from(terminal.rows()))
        .map(|row| terminal.row_cells(row))
        .collect();
    let cursor = terminal.cursor();
    let mut readings = Readings::new();
    readings.insert("size", format!("{:?}", (terminal.rows(), terminal.cols())));
    readings.insert(
        "cell_text",
        format!("{:?}", (terminal.screen_text(true)?, visible)),
    );
    readings.insert("cell_attributes", format!("{attributes:?}"));
    readings.insert("hyperlinks", format!("{hyperlinks:?}"));
    readings.insert("cursor_position", format!("{:?}", (cursor.row, cursor.col)));
    readings.insert("cursor_visible", format!("{:?}", cursor.visible));
    readings.insert(
        "cursor_pending_wrap",
        format!("{:?}", terminal.cursor_pending_wrap()?),
    );
    readings.insert(
        "cursor_style",
        format!("{:?}", terminal.cursor_appearance()?),
    );
    readings.insert("cursor_pen", format!("{:?}", terminal.cursor_pen()?));
    readings.insert("modes", format!("{:?}", terminal.modes()));
    readings.insert("colors", format!("{:?}", terminal.colors()?));
    readings.insert("title", format!("{:?}", terminal.title()));
    readings.insert("cwd", format!("{:?}", terminal.cwd()));
    readings.insert(
        "tab_stops_margins_charsets",
        format!("{:?}", terminal.formatter_extras()?),
    );
    Ok(readings)
}

/// What DECRC restores, read after it: the cursor's position, pending wrap, style and pen, the modes (origin mode) and the
/// charsets (with the tab stops and margins of the same formatter reading).
fn read_saved_cursor(terminal: &mut Terminal) -> Result<String, Error> {
    terminal.vt_write(DECRC);
    let cursor = terminal.cursor();
    Ok(format!(
        "{:?}",
        (
            (cursor.row, cursor.col),
            terminal.cursor_pending_wrap()?,
            terminal.cursor_appearance()?,
            terminal.cursor_pen()?,
            terminal.modes(),
            terminal.formatter_extras()?,
        )
    ))
}

/// The fields in which `a` and `b` differ, in the order of their names; none when they are equal. Both terminals must be
/// the harness's own oracles: the saved-cursor reading applies DECRC to each, last.
///
/// # Errors
/// A libghostty read failed.
pub(crate) fn differences(a: &mut Terminal, b: &mut Terminal) -> Result<Vec<&'static str>, Error> {
    let (mut left, mut right) = (read(a)?, read(b)?);
    left.insert("saved_cursor", read_saved_cursor(a)?);
    right.insert("saved_cursor", read_saved_cursor(b)?);
    Ok(left
        .iter()
        .filter(|(field, value)| right.get(*field) != Some(*value))
        .map(|(field, _)| *field)
        .collect())
}

/// O_raw (R-62): a fresh oracle terminal of the session's size and color profile that applies `output`, every output byte
/// of the session from its start, in order and unchanged (the oracle never models the line discipline, R-62 Q3).
///
/// # Errors
/// libghostty refused the size or the profile.
pub(crate) fn raw_oracle(
    size: &Size,
    profile: Option<&ColorProfile>,
    output: &[u8],
) -> Result<Terminal, Error> {
    let mut terminal = Terminal::new(size, History::On)?;
    if let Some(profile) = profile {
        terminal.set_color_profile(profile)?;
    }
    terminal.vt_write(output);
    Ok(terminal)
}

/// `oracle_resume`'s answer on both tiers (R-62, R-67): true only if both comparisons hold. (1) `resumed` (O_resume: the
/// capture's pages with every output byte after the capture's cut) equals O_raw. (2) `model` (the session's model, decoded
/// from its own capture or snapshot into an oracle terminal) equals O_raw. `raw` makes a fresh O_raw for each comparison,
/// because the saved-cursor reading changes the oracles that it compares.
///
/// # Errors
/// `raw` or a libghostty read failed.
pub(crate) fn resume_holds(
    mut resumed: Terminal,
    mut model: Terminal,
    raw: impl Fn() -> Result<Terminal, Error>,
) -> Result<bool, Error> {
    Ok(differences(&mut resumed, &mut raw()?)?.is_empty()
        && differences(&mut model, &mut raw()?)?.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal(bytes: &[u8]) -> Terminal {
        let mut terminal = Terminal::new(
            &Size {
                rows: 4,
                cols: 10,
                cell_px: None,
            },
            History::On,
        )
        .unwrap();
        terminal.vt_write(bytes);
        terminal
    }

    /// The output of `st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut` at `contracts-v0.1.36`:
    /// steps 6 (before the capture) and 13 (after it).
    const BEFORE: &[u8] =
        b"\x1b[3;5H\x1b[1;31m\x1b7\x1b[1;10H\x1bH\x1b[2;4r\x1b(0\x1b[0m\x1b]2;r62-st6b-dec-1\x1b\\";
    const AFTER: &[u8] = b"\x1b8x\tlqk\x1bD\x1bD\x1bD\x1b(By\x1b]2;r62-st6b-dec-2\x1b\\";

    fn size() -> Size {
        Size {
            rows: 4,
            cols: 10,
            cell_px: None,
        }
    }

    /// A terminal that applied `bytes`, through its snapshot: the pages of a capture of that state.
    fn captured(bytes: &[u8]) -> Terminal {
        let pages = terminal(bytes).snapshot().unwrap();
        Terminal::from_snapshot(&pages, History::On, None).unwrap()
    }

    fn raw() -> Result<Terminal, Error> {
        raw_oracle(&size(), None, &[BEFORE, AFTER].concat())
    }

    /// R-62 evidence: a capture with one field removed (the saved cursor) gives `false` through comparison 1 and through
    /// comparison 2; the whole capture gives `true`.
    #[test]
    fn a_capture_without_the_saved_cursor_fails_each_comparison() {
        let resumed = |pages: Terminal| {
            let mut pages = pages;
            pages.vt_write(AFTER);
            pages
        };
        // DECSC (`1b37`) occurs once in the step: the capture that loses it has no saved cursor.
        let at = BEFORE.windows(2).position(|w| w == b"\x1b7").unwrap();
        assert_eq!(BEFORE.windows(2).filter(|w| *w == b"\x1b7").count(), 1);
        let without: Vec<u8> = [&BEFORE[..at], &BEFORE[at + 2..]].concat();
        let model = || captured(&[BEFORE, AFTER].concat());
        assert!(resume_holds(resumed(captured(BEFORE)), model(), raw).unwrap());
        assert!(!resume_holds(resumed(captured(&without)), model(), raw).unwrap());
        let mut without_after = without.clone();
        without_after.extend_from_slice(AFTER);
        assert!(!resume_holds(resumed(captured(BEFORE)), captured(&without_after), raw).unwrap());
    }

    #[test]
    fn the_raw_oracle_takes_the_size_and_the_profile() {
        let profile = ColorProfile {
            foreground: botster_core_contract::prelude::Rgb { r: 1, g: 2, b: 3 },
            background: botster_core_contract::prelude::Rgb { r: 4, g: 5, b: 6 },
            cursor: None,
            palette: None,
        };
        let mut plain = raw_oracle(&size(), None, b"x").unwrap();
        let mut colored = raw_oracle(&size(), Some(&profile), b"x").unwrap();
        assert_eq!(
            differences(&mut plain, &mut colored).unwrap(),
            vec!["colors"]
        );
        let mut expected = terminal(b"x");
        assert!(
            differences(&mut raw_oracle(&size(), None, b"x").unwrap(), &mut expected)
                .unwrap()
                .is_empty()
        );
        let wide = Size { cols: 11, ..size() };
        assert!(differences(
            &mut raw_oracle(&wide, None, b"x").unwrap(),
            &mut terminal(b"x")
        )
        .unwrap()
        .contains(&"size"));
    }

    #[test]
    fn equal_terminals_have_no_difference() {
        let bytes = b"\x1b[1;31mred\x1b[0m\x1b]8;;https://example.test\x1b\\link\x1b]8;;\x1b\\\x1b]2;title\x1b\\";
        assert_eq!(
            differences(&mut terminal(bytes), &mut terminal(bytes)).unwrap(),
            Vec::<&str>::new()
        );
    }

    /// R-62 (the comparator-completeness test): for each field of the list, two oracle terminals that differ only in that
    /// field compare unequal. Each pair ends with the cursor at the same place and the same pen, so the other fields stay
    /// equal; the expected set names every field that libghostty reads the change through (the saved-cursor reading repeats
    /// the modes and the formatter's reading).
    #[test]
    fn two_terminals_that_differ_in_one_field_compare_unequal() {
        let cases: &[(&[u8], &[u8], &[&str])] = &[
            (b"\x1b[2;2HX\x1b[H", b"\x1b[2;2HY\x1b[H", &["cell_text"]),
            (
                b"\x1b[2;2H\x1b[1mX\x1b[0m\x1b[H",
                b"\x1b[2;2HX\x1b[H",
                &["cell_attributes"],
            ),
            (
                b"\x1b]8;;https://a.test\x1b\\X\x1b]8;;\x1b\\\x1b[H",
                b"\x1b]8;;https://b.test\x1b\\X\x1b]8;;\x1b\\\x1b[H",
                &["hyperlinks"],
            ),
            (b"\x1b[2;3H", b"\x1b[3;2H", &["cursor_position"]),
            (
                b"\x1b[1;10HX",
                b"\x1b[1;10HX\x1b[1;10H",
                &["cursor_pending_wrap"],
            ),
            (
                b"\x1b[?25l",
                b"",
                &["cursor_visible", "modes", "saved_cursor"],
            ),
            // DECSCUSR also sets the blink mode (DEC 12).
            (b"\x1b[3 q", b"", &["cursor_style", "modes", "saved_cursor"]),
            (b"\x1b[1;31m", b"", &["cursor_pen"]),
            (b"\x1b[?2004h", b"", &["modes", "saved_cursor"]),
            (b"\x1b]4;1;rgb:12/34/56\x1b\\", b"", &["colors"]),
            (b"\x1b]11;rgb:12/34/56\x1b\\", b"", &["colors"]),
            (b"\x1b]2;one\x1b\\", b"\x1b]2;two\x1b\\", &["title"]),
            (
                b"\x1b]7;file:///one\x1b\\",
                b"\x1b]7;file:///two\x1b\\",
                &["cwd"],
            ),
            (
                b"\x1b[3g",
                b"",
                &["tab_stops_margins_charsets", "saved_cursor"],
            ),
            (
                b"\x1b[2;3r\x1b[H",
                b"\x1b[H",
                &["tab_stops_margins_charsets", "saved_cursor"],
            ),
            // With no saved cursor, DECRC resets the charsets, so only the live reading differs.
            (b"\x1b(0", b"", &["tab_stops_margins_charsets"]),
            (
                b"\x1b[2;2H\x1b7\x1b[H",
                b"\x1b[3;3H\x1b7\x1b[H",
                &["saved_cursor"],
            ),
            (b"\x1b[1m\x1b7\x1b[0m", b"\x1b7", &["saved_cursor"]),
            (b"\x1b(0\x1b7\x1b(B", b"\x1b(B\x1b7", &["saved_cursor"]),
            (
                b"\x1b[?6h\x1b7\x1b[?6l\x1b[H",
                b"\x1b7\x1b[H",
                &["saved_cursor"],
            ),
        ];
        for (a, b, fields) in cases {
            let mut expected = fields.to_vec();
            expected.sort_unstable();
            assert_eq!(
                differences(&mut terminal(a), &mut terminal(b)).unwrap(),
                expected,
                "{a:?} against {b:?}"
            );
        }
    }

    #[test]
    fn scrollback_cells_are_compared() {
        let a = b"1\r\n2\r\n3\r\n4\r\n5\r\n\x1b[H";
        let b = b"0\r\n2\r\n3\r\n4\r\n5\r\n\x1b[H";
        assert_eq!(
            differences(&mut terminal(a), &mut terminal(b)).unwrap(),
            vec!["cell_text"]
        );
        assert!(rows_with_history(&terminal(a)).unwrap() > 4);
    }
}
