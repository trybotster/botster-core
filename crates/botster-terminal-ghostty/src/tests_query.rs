//! Tests of the queries (EV-8). libghostty finds the query, its kind and its bytes and computes the shadow reply; the
//! tests check what the binding keeps against the input that was fed and against the model's own state.

use botster_core_contract::prelude::{CellPx, Size};
use botster_route_codec::prelude::QueryKind as Label;

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

fn terminal_with_px() -> Terminal {
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

/// Every digit run of a reply, in order.
fn numbers(bytes: &[u8]) -> Vec<i64> {
    let mut out = Vec::new();
    let mut current: Option<i64> = None;
    for &b in bytes {
        if b.is_ascii_digit() {
            current = Some(current.unwrap_or(0) * 10 + i64::from(b - b'0'));
        } else if let Some(n) = current.take() {
            out.push(n);
        }
    }
    out.extend(current);
    out
}

/// The sequences of the query kinds, with the kind that the library reports for each and whether it has a shadow
/// reply with the default terminal. The sequences are the stimulus; the assertions compare with the input.
fn cases() -> Vec<(&'static [u8], QueryKind, bool)> {
    vec![
        (b"\x1b[c", QueryKind::DeviceAttributesPrimary, true),
        (b"\x1b[>c", QueryKind::DeviceAttributesSecondary, true),
        (b"\x1b[=c", QueryKind::DeviceAttributesTertiary, true),
        (b"\x1b[5n", QueryKind::OperatingStatus, true),
        (b"\x1b[6n", QueryKind::CursorPosition, true),
        (b"\x1b[?996n", QueryKind::ColorScheme, false),
        (b"\x1b[?998n", QueryKind::Visibility, true),
        (b"\x05", QueryKind::Enquiry, false),
        (b"\x1b[?u", QueryKind::KittyKeyboard, true),
        (b"\x1b[4$p", QueryKind::ModeReport, true),
        (b"\x1b[?25$p", QueryKind::ModeReport, true),
        (b"\x1b[>q", QueryKind::Xtversion, true),
        (b"\x1b[11t", QueryKind::Size11T, false),
        (b"\x1b[13t", QueryKind::Size13T, false),
        (b"\x1b[15t", QueryKind::Size15T, false),
        (b"\x1b[19t", QueryKind::Size19T, false),
        (b"\x1b[20t", QueryKind::Size20T, false),
        (b"\x1b[21t", QueryKind::Size21T, false),
        (b"\x1b[14;2t", QueryKind::Size14Of2T, false),
        (b"\x1b[13;2t", QueryKind::Size13Of2T, false),
        (b"\x1bP$qm\x1b\\", QueryKind::Decrqss, true),
        (b"\x1bP+q544e\x1b\\", QueryKind::Xtgettcap, false),
        (b"\x1b]52;c;?\x1b\\", QueryKind::ClipboardRead, false),
    ]
}

#[test]
fn the_query_kind_table_matches_the_header_of_the_pinned_ghostty() {
    use std::collections::BTreeMap;

    let header = include_str!("../vendor/ghostty/include/ghostty/vt/terminal.h");
    let mut in_header = BTreeMap::new();
    for line in header.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("GHOSTTY_TERMINAL_QUERY_") {
            let Some((name, value)) = rest.split_once('=') else {
                continue;
            };
            let Ok(value) = value.trim().trim_end_matches(',').parse::<i32>() else {
                continue;
            };
            in_header.insert(format!("GHOSTTY_TERMINAL_QUERY_{}", name.trim()), value);
        }
    }
    // The invalid kind (0) is in the header and not in the table; the table lists every other kind.
    in_header.remove("GHOSTTY_TERMINAL_QUERY_INVALID");
    let table: BTreeMap<String, i32> = query::KINDS
        .iter()
        .map(|(v, _, name)| ((*name).to_owned(), *v))
        .collect();
    assert_eq!(in_header, table);
}

#[test]
fn every_query_stops_the_write_with_its_exact_bytes_and_kind() {
    for (sequence, kind, _) in cases() {
        let mut terminal = terminal();
        let mut input = b"abc".to_vec();
        input.extend_from_slice(sequence);
        input.extend_from_slice(b"tail");

        let step = terminal.vt_write_until_query(&input).unwrap();
        let query = step
            .query
            .unwrap_or_else(|| panic!("no query for {sequence:?}"));
        assert_eq!(query.kind, kind, "{sequence:?}");
        // It stops right after the sequence, and the request is the sequence as it was fed.
        assert_eq!(step.consumed, 3 + sequence.len(), "{sequence:?}");
        assert_eq!(
            query.request.as_deref(),
            Some(&input[3..step.consumed]),
            "{sequence:?}"
        );
        assert!(!query.request_truncated);
    }
}

#[test]
fn the_shadow_reply_exists_for_what_the_library_answers_and_is_empty_otherwise() {
    for (sequence, _, answered) in cases() {
        let mut terminal = terminal();
        let step = terminal.vt_write_until_query(sequence).unwrap();
        let query = step.query.unwrap();
        assert_eq!(!query.shadow_reply.is_empty(), answered, "{sequence:?}");
        assert!(!query.shadow_reply_overflow);
        // The reply is held: nothing went to the pty.
        let drained = terminal.drain_events();
        assert!(drained.pty_writes.is_empty(), "{sequence:?}");
        assert!(
            drained.events.is_empty() || sequence == b"\x1b]52;c;?\x1b\\",
            "{sequence:?}"
        );
    }
}

#[test]
fn the_model_takes_no_byte_past_the_query_and_the_reply_is_from_the_query_point() {
    let mut terminal = terminal();
    // Put the cursor at row 3, column 7, ask for the position, and then write more text.
    let input = b"\x1b[3;7H\x1b[6nXYZ";
    let step = terminal.vt_write_until_query(input).unwrap();
    let query = step.query.unwrap();
    assert_eq!(query.kind, QueryKind::CursorPosition);
    assert_eq!(step.consumed, input.len() - 3);

    // The text after the query was not taken: the screen is still empty and the cursor has not moved.
    assert_eq!(terminal.screen_text(false).unwrap().text, "");
    let cursor = terminal.cursor();
    assert_eq!((cursor.row, cursor.col), (2, 6));

    // The held reply names the cursor of the query point, one-based.
    assert_eq!(
        numbers(&query.shadow_reply),
        vec![i64::from(cursor.row) + 1, i64::from(cursor.col) + 1]
    );

    // Offering the rest takes it, and the held reply does not change.
    let rest = terminal
        .vt_write_until_query(&input[step.consumed..])
        .unwrap();
    assert_eq!(rest.consumed, 3);
    assert!(rest.query.is_none());
    assert_eq!(
        terminal
            .screen_text(false)
            .unwrap()
            .text
            .trim_start_matches('\n')
            .trim(),
        "XYZ"
    );
    assert_eq!(numbers(&query.shadow_reply), vec![3, 7]);
}

#[test]
fn a_plain_write_counts_the_queries_it_meets_and_buffers_none() {
    let mut terminal = terminal();
    let mut input = b"\x1b]2;t\x07".to_vec();
    for _ in 0..20_000 {
        input.extend_from_slice(b"\x1b[5n");
    }
    input.push(0x07);
    terminal.vt_write(&input);
    let drained = terminal.drain_events();
    // The other events are kept, and the queries are only counted: the buffer holds none of them.
    assert_eq!(drained.events.len(), 2);
    assert!(matches!(drained.events[0], TerminalEvent::Title(_)));
    assert_eq!(drained.events[1], TerminalEvent::Bell);
    assert_eq!(drained.unrouted_queries, 20_000);
    assert!(drained.pty_writes.len() <= MAX_SHADOW_REPLY_BYTES);
    // The count restarts at each drain.
    assert_eq!(terminal.drain_events().unrouted_queries, 0);
}

#[test]
fn the_shadow_never_answers_a_clipboard_read_and_the_selection_and_terminator_are_kept() {
    for (selection, expected) in [("c", "c"), ("s0", "s0"), ("cp", "cp")] {
        for (terminator, bel) in [("\x1b\\", false), ("\x07", true)] {
            let mut terminal = terminal();
            let input = format!("\x1b]52;{selection};?{terminator}");
            let step = terminal.vt_write_until_query(input.as_bytes()).unwrap();
            let query = step.query.unwrap();
            assert_eq!(query.kind, QueryKind::ClipboardRead);
            assert_eq!(query.selection.as_deref(), Some(expected));
            assert_eq!(
                query.terminator,
                Some(if bel { Terminator::Bel } else { Terminator::St })
            );
            assert!(query.shadow_reply.is_empty());
            assert!(terminal.drain_events().pty_writes.is_empty());
        }
    }
    // A selection that the program left out is `s0`.
    let mut terminal = terminal();
    let query = terminal
        .vt_write_until_query(b"\x1b]52;;?\x07")
        .unwrap()
        .query
        .unwrap();
    assert_eq!(query.selection.as_deref(), Some("s0"));
}

#[test]
fn the_size_reports_of_the_shadow_follow_the_size_and_the_cell_size() {
    // With a cell size, the pixel reports are the cells times the cell size, height before width.
    let mut terminal = terminal_with_px();
    let text_area = terminal
        .vt_write_until_query(b"\x1b[14t")
        .unwrap()
        .query
        .unwrap();
    assert_eq!(&numbers(&text_area.shadow_reply)[1..], [24 * 18, 80 * 9]);
    let cell = terminal
        .vt_write_until_query(b"\x1b[16t")
        .unwrap()
        .query
        .unwrap();
    assert_eq!(&numbers(&cell.shadow_reply)[1..], [18, 9]);
    let chars = terminal
        .vt_write_until_query(b"\x1b[18t")
        .unwrap()
        .query
        .unwrap();
    assert_eq!(&numbers(&chars.shadow_reply)[1..], [24, 80]);

    // After a resize, they follow.
    terminal
        .resize(&Size {
            rows: 30,
            cols: 100,
            cell_px: Some(CellPx {
                width: 10,
                height: 20,
            }),
        })
        .unwrap();
    let again = terminal
        .vt_write_until_query(b"\x1b[14t")
        .unwrap()
        .query
        .unwrap();
    assert_eq!(&numbers(&again.shadow_reply)[1..], [30 * 20, 100 * 10]);

    // Without a cell size, the shadow has no answer for them.
    let mut without = self::terminal();
    let none = without
        .vt_write_until_query(b"\x1b[14t")
        .unwrap()
        .query
        .unwrap();
    assert!(none.shadow_reply.is_empty());
}

#[test]
fn a_reply_outside_a_query_is_a_pty_write_and_never_lost() {
    // Mode 2048 asks for an in-band size report when it is set.
    let mut terminal = terminal_with_px();
    terminal.vt_write(b"\x1b[?2048h");
    let drained = terminal.drain_events();
    assert!(!drained.pty_writes.is_empty());
    assert_eq!(drained.unrouted_queries, 0);
}

#[test]
fn a_long_request_is_truncated_at_the_limit_and_says_so() {
    let mut terminal = terminal();
    terminal.set_query_request_limit(8).unwrap();
    let input = b"\x1b]4;1;?;2;?;3;?;4;?\x07";
    let query = terminal.vt_write_until_query(input).unwrap().query.unwrap();
    assert_eq!(query.kind, QueryKind::OscColor);
    assert!(query.request_truncated);
    assert_eq!(query.request.as_deref(), Some(&input[..8]));
}

#[test]
fn a_trailing_esc_of_an_unfinished_string_is_offered_again() {
    let mut terminal = terminal();
    let sequence = b"\x1b]52;c;?\x1b\\";
    // The input ends after the ESC that may start the ST.
    let step = terminal
        .vt_write_until_query(&sequence[..sequence.len() - 1])
        .unwrap();
    assert!(step.query.is_none());
    assert_eq!(step.consumed, sequence.len() - 2);

    let rest = terminal
        .vt_write_until_query(&sequence[step.consumed..])
        .unwrap();
    assert_eq!(rest.query.unwrap().request.as_deref(), Some(&sequence[..]));
}

#[test]
fn the_label_of_a_query_is_the_contract_kind() {
    let label = |sequence: &[u8]| {
        terminal()
            .vt_write_until_query(sequence)
            .unwrap()
            .query
            .unwrap()
            .label()
    };
    assert_eq!(label(b"\x1b[14t"), Some(Label::TextAreaPixels));
    assert_eq!(label(b"\x1b[14;2t"), Some(Label::WindowPixels));
    assert_eq!(label(b"\x1b[16t"), Some(Label::CellPixels));
    assert_eq!(label(b"\x1b[15t"), Some(Label::ScreenPixels));
    assert_eq!(label(b"\x1b[19t"), Some(Label::ScreenChars));
    assert_eq!(label(b"\x1b[11t"), Some(Label::WindowState));
    assert_eq!(
        label(b"\x1b[13t"),
        Some(Label::WindowPosition {
            area: "window".into()
        })
    );
    assert_eq!(
        label(b"\x1b[13;2t"),
        Some(Label::WindowPosition {
            area: "text_area".into()
        })
    );
    assert_eq!(label(b"\x1b[21t"), Some(Label::WindowTitle));
    assert_eq!(label(b"\x1b[20t"), Some(Label::IconLabel));
    assert_eq!(label(b"\x1b[?996n"), Some(Label::ColorScheme));
    assert_eq!(
        label(b"\x1b]52;q;?\x07"),
        Some(Label::ClipboardRead {
            selection: "q".into()
        })
    );
    // CSI 18 t and the others have no typed kind.
    assert_eq!(label(b"\x1b[18t"), None);
    assert_eq!(label(b"\x1b[5n"), None);
    assert_eq!(label(b"\x1b[c"), None);
}

#[test]
fn the_shadow_answerable_kinds_follow_the_cell_size_and_never_include_a_clipboard_read() {
    use botster_route_codec::prelude::QueryKind as Label;

    let without = terminal().shadow_answerable_kinds();
    let with = terminal_with_px().shadow_answerable_kinds();
    assert!(!without.contains(&Label::TextAreaPixels));
    assert!(with.contains(&Label::TextAreaPixels));
    assert!(with.contains(&Label::CellPixels));
    // Every kind that the host-less shadow answers, the one with a cell size answers too.
    assert!(without.iter().all(|kind| with.contains(kind)));
    assert!(!with
        .iter()
        .any(|kind| matches!(kind, Label::ClipboardRead { .. })));
}
