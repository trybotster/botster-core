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

use botster_terminal_ghostty::{Error, Terminal};
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

#[cfg(test)]
mod tests {
    use super::*;
    use botster_core_contract::prelude::Size;
    use botster_terminal_ghostty::History;

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
