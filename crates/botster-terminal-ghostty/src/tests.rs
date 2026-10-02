//! Tests of the binding. The terminal's own behaviour is the oracle: the tests feed the sequence that a program writes
//! and compare what the binding reports with the values that the sequence carries.

use botster_core_contract::prelude::{CellPx, LostKind, NotificationSource, PromptMarkKind};
use botster_route_codec::prelude::{ModeFlags, MouseEncoding, MouseTracking};

use super::*;

fn size(cols: u32, rows: u32) -> Size {
    Size { rows, cols, cell_px: None }
}

fn terminal() -> Terminal {
    Terminal::new(&size(80, 24), History::On).unwrap()
}

/// The events of writing `bytes`.
fn events_of(bytes: &[u8]) -> Vec<TerminalEvent> {
    let mut terminal = terminal();
    terminal.vt_write(bytes);
    let drained = terminal.drain_events();
    assert_eq!(drained.dropped, 0);
    drained.events
}

#[test]
fn a_new_terminal_has_its_size_and_takes_output() {
    let mut terminal = terminal();
    assert_eq!((terminal.cols(), terminal.rows()), (80, 24));
    terminal.vt_write(b"hello");
    terminal.resize(&size(100, 30)).unwrap();
    assert_eq!((terminal.cols(), terminal.rows()), (100, 30));
}

#[test]
fn terminal_is_send_and_not_sync() {
    fn is_send<T: Send>() {}
    is_send::<Terminal>();
    static_assertions::assert_not_impl_any!(Terminal: Sync);
}

#[test]
fn the_zig_package_list_is_the_one_that_the_prefetch_script_reads() {
    // build_data.rs holds the only list. The script reads it with sed, so each entry sits on its own line.
    let data = include_str!("../build_data.rs");
    let listed = data
        .lines()
        .filter(|line| line.trim_start().starts_with('"') && line.trim_end().ends_with("\","))
        .count();
    // 6 build arguments, 7 packages and the 2 of them that the zon files name.
    assert_eq!(listed, 15);
}

// ---- title, cwd, bell (EV-1, EV-7) ----

#[test]
fn osc_0_and_osc_2_set_the_title_and_osc_1_does_not() {
    let title = "my title";
    for number in [0, 2] {
        let sequence = format!("\x1b]{number};{title}\x07");
        assert_eq!(events_of(sequence.as_bytes()), vec![TerminalEvent::Title(title.to_owned())]);
    }
    // E2-1: OSC 1 gives no TitleChanged, and the title stays empty.
    let mut terminal = terminal();
    terminal.vt_write(format!("\x1b]1;{title}\x07").as_bytes());
    assert!(terminal.drain_events().events.is_empty());
    assert_eq!(terminal.title(), "");
}

#[test]
fn the_title_and_the_cwd_are_readable_after_the_event() {
    let mut terminal = terminal();
    terminal.vt_write(b"\x1b]2;shell\x07\x1b]7;file://host/tmp/dir\x07");
    let events = terminal.drain_events().events;
    assert_eq!(events.len(), 2);
    assert_eq!(terminal.title(), "shell");
    assert_eq!(terminal.cwd(), "file://host/tmp/dir");
    assert_eq!(events[1], TerminalEvent::Cwd(terminal.cwd()));
}

#[test]
fn a_title_that_the_library_cut_inside_a_character_keeps_its_complete_prefix() {
    // The library cuts a title at 1024 bytes. Put a 3-byte character across that cut.
    let prefix = "a".repeat(1023);
    let sequence = format!("\x1b]0;{prefix}€\x07");
    let events = events_of(sequence.as_bytes());
    match events.as_slice() {
        [TerminalEvent::Title(title)] => {
            assert_eq!(title, &prefix);
        }
        other => panic!("expected one title, got {other:?}"),
    }
}

#[test]
fn bel_is_a_bell_event() {
    assert_eq!(events_of(b"a\x07b"), vec![TerminalEvent::Bell]);
}

// ---- notifications and prompt marks (A2-4, A3-1) ----

#[test]
fn osc_9_and_osc_777_are_notifications_with_their_source() {
    assert_eq!(
        events_of(b"\x1b]9;build done\x07"),
        vec![TerminalEvent::Notification {
            source: NotificationSource::Osc9,
            title: None,
            body: "build done".to_owned(),
        }]
    );
    assert_eq!(
        events_of(b"\x1b]777;notify;Tests;all passed\x07"),
        vec![TerminalEvent::Notification {
            source: NotificationSource::Osc777,
            title: Some("Tests".to_owned()),
            body: "all passed".to_owned(),
        }]
    );
    // An OSC 777 notify with an empty title is still OSC 777 and keeps its empty title.
    assert_eq!(
        events_of(b"\x1b]777;notify;;body only\x07"),
        vec![TerminalEvent::Notification {
            source: NotificationSource::Osc777,
            title: Some(String::new()),
            body: "body only".to_owned(),
        }]
    );
}

#[test]
fn osc_133_marks_and_the_exit_code() {
    let events = events_of(b"\x1b]133;A\x07\x1b]133;B\x07\x1b]133;C\x07\x1b]133;D;3\x07\x1b]133;D\x07");
    assert_eq!(
        events,
        vec![
            TerminalEvent::PromptMark { mark: PromptMarkKind::PromptStart, exit_code: None },
            TerminalEvent::PromptMark { mark: PromptMarkKind::CommandStart, exit_code: None },
            TerminalEvent::PromptMark { mark: PromptMarkKind::CommandExecuted, exit_code: None },
            TerminalEvent::PromptMark { mark: PromptMarkKind::CommandFinished, exit_code: Some(3) },
            TerminalEvent::PromptMark { mark: PromptMarkKind::CommandFinished, exit_code: None },
        ]
    );
}

// ---- clipboard writes (EV-3, EV-7) ----

#[test]
fn osc_52_writes_report_the_selection_as_written_and_the_decoded_bytes() {
    // "aGk=" is "hi".
    for selection in ["c", "p", "q", "s", "0", "7", "s0", "cp", "cpqs01234567"] {
        for terminator in ["\x07", "\x1b\\"] {
            let sequence = format!("\x1b]52;{selection};aGk={terminator}");
            assert_eq!(
                events_of(sequence.as_bytes()),
                vec![TerminalEvent::ClipboardWrite { selection: selection.to_owned(), bytes: b"hi".to_vec() }],
                "selection {selection:?}"
            );
        }
    }
    // A program that leaves the selection out writes `s0`.
    assert_eq!(
        events_of(b"\x1b]52;;aGk=\x07"),
        vec![TerminalEvent::ClipboardWrite { selection: "s0".to_owned(), bytes: b"hi".to_vec() }]
    );
}

// ---- modes (5.1A, Core erratum 2) ----

#[test]
fn a_fresh_terminal_has_the_default_modes() {
    let modes = terminal().modes();
    assert!(modes.cursor_visible);
    assert!(!modes.alt_screen);
    assert!(!modes.bracketed_paste);
    assert!(!modes.application_cursor_keys);
    assert!(!modes.application_keypad);
    assert!(!modes.focus_reporting);
    assert_eq!(modes.kitty_flags, 0);
    assert_eq!(modes.mouse_tracking, MouseTracking::None);
    assert_eq!(modes.mouse_encoding, MouseEncoding::X10);
}

#[test]
fn each_normative_mode_follows_its_sequence() {
    let mut terminal = terminal();
    terminal.vt_write(b"\x1b[?2004h\x1b[?1h\x1b[?66h\x1b[?1004h\x1b[?25l\x1b[?1049h");
    let modes = terminal.modes();
    assert!(modes.bracketed_paste);
    assert!(modes.application_cursor_keys);
    assert!(modes.application_keypad);
    assert!(modes.focus_reporting);
    assert!(!modes.cursor_visible);
    assert!(modes.alt_screen);

    terminal.vt_write(b"\x1b[?2004l\x1b[?1l\x1b[?66l\x1b[?1004l\x1b[?25h\x1b[?1049l");
    assert_eq!(terminal.modes(), ModeFlags { ..terminal_default() });
}

fn terminal_default() -> ModeFlags {
    terminal().modes()
}

#[test]
fn the_active_mouse_tracking_is_the_last_command_not_the_mode_bits() {
    // The same mode bits, two histories, two active modes.
    let mut a = terminal();
    a.vt_write(b"\x1b[?1000h\x1b[?1003h");
    let mut b = terminal();
    b.vt_write(b"\x1b[?1003h\x1b[?1000h");
    assert_eq!(a.modes().mouse_tracking, MouseTracking::AnyEvent);
    assert_eq!(b.modes().mouse_tracking, MouseTracking::Normal);

    let mut each = terminal();
    for (sequence, expected) in [
        (&b"\x1b[?9h"[..], MouseTracking::X10),
        (b"\x1b[?9l\x1b[?1000h", MouseTracking::Normal),
        (b"\x1b[?1000l\x1b[?1002h", MouseTracking::ButtonEvent),
        (b"\x1b[?1002l\x1b[?1003h", MouseTracking::AnyEvent),
        (b"\x1b[?1003l", MouseTracking::None),
    ] {
        each.vt_write(sequence);
        assert_eq!(each.modes().mouse_tracking, expected);
    }
}

#[test]
fn the_active_mouse_encoding_follows_the_format_modes() {
    let mut terminal = terminal();
    for (sequence, expected) in [
        (&b"\x1b[?1005h"[..], MouseEncoding::Utf8),
        (b"\x1b[?1005l\x1b[?1006h", MouseEncoding::Sgr),
        (b"\x1b[?1006l\x1b[?1015h", MouseEncoding::Urxvt),
        (b"\x1b[?1015l\x1b[?1016h", MouseEncoding::SgrPixels),
        (b"\x1b[?1016l", MouseEncoding::X10),
    ] {
        terminal.vt_write(sequence);
        assert_eq!(terminal.modes().mouse_encoding, expected);
    }
}

#[test]
fn kitty_flags_are_the_top_of_the_stack() {
    let mut terminal = terminal();
    // CSI > flags u pushes, CSI < u pops. The values are the flags being pushed.
    terminal.vt_write(b"\x1b[>1u");
    assert_eq!(terminal.modes().kitty_flags, 1);
    terminal.vt_write(b"\x1b[>31u");
    assert_eq!(terminal.modes().kitty_flags, 31);
    terminal.vt_write(b"\x1b[<u");
    assert_eq!(terminal.modes().kitty_flags, 1);
    terminal.vt_write(b"\x1b[<u");
    assert_eq!(terminal.modes().kitty_flags, 0);
}

#[test]
fn other_modes_are_tracked_model_modes_and_an_unknown_number_changes_nothing() {
    let mut terminal = terminal();
    let before = terminal.modes();

    // A mode that the model tracks and that no normative field covers.
    terminal.vt_write(b"\x1b[?2026h");
    let after = terminal.modes();
    assert_ne!(before, after);
    assert_eq!(after.other_modes.get("dec_2026"), Some(&true));
    assert_eq!(before.other_modes.get("dec_2026"), Some(&false));
    // Every other field is unchanged.
    let mut expected = before.clone();
    expected.other_modes.insert("dec_2026".to_owned(), true);
    assert_eq!(after, expected);

    // A mode number that the model does not know has no state: no field changes.
    terminal.vt_write(b"\x1b[?9999h\x1b[?31337h");
    assert_eq!(terminal.modes(), after);
}

#[test]
fn the_mode_lists_cover_every_mode_of_the_pinned_header() {
    use std::collections::BTreeSet;

    // Every `ghostty_mode_new(n, ansi)` of modes.h is either normative or in other_modes.
    let header = include_str!("../vendor/ghostty/include/ghostty/vt/modes.h");
    let mut in_header = BTreeSet::new();
    for line in header.lines().filter(|l| l.starts_with("#define GHOSTTY_MODE_")) {
        let Some(start) = line.find("ghostty_mode_new(") else { continue };
        let args = &line[start + "ghostty_mode_new(".len()..];
        let args = &args[..args.find(')').unwrap()];
        let (value, ansi) = args.split_once(',').unwrap();
        in_header.insert((value.trim().parse::<u16>().unwrap(), ansi.trim() == "true"));
    }
    assert!(in_header.len() > 30, "the header parse found {} modes", in_header.len());

    let mut covered: BTreeSet<(u16, bool)> = modes::OTHER_MODES.iter().copied().collect();
    covered.extend(modes::normative_dec().iter().map(|&value| (value, false)));
    assert_eq!(in_header, covered, "a mode of the pinned header is in neither list, or a listed mode is not in it");
}

// ---- the event buffer is bounded and drops loudly (EV-2) ----

#[test]
fn a_full_buffer_drops_events_and_says_which_kinds() {
    let mut terminal = terminal();
    let bells = MAX_BUFFERED_EVENTS + 904;
    terminal.vt_write(&vec![0x07; bells]);
    let drained = terminal.drain_events();
    assert_eq!(drained.events.len(), MAX_BUFFERED_EVENTS);
    assert_eq!(drained.dropped, 904);
    assert!(drained.dropped_kinds.contains(&LostKind::Bell));

    // The next drain starts empty.
    terminal.vt_write(b"\x07");
    let next = terminal.drain_events();
    assert_eq!(next.events.len(), 1);
    assert_eq!(next.dropped, 0);
    assert!(next.dropped_kinds.is_empty());
}

// ---- sizes (SZ-1, SZ-3) ----

#[test]
fn a_size_the_library_cannot_hold_is_refused() {
    for (cols, rows) in [(0, 24), (80, 0), (65_536, 24), (80, 65_536)] {
        assert_eq!(Terminal::new(&size(cols, rows), History::On).err(), Some(Error::InvalidValue), "{cols}x{rows}");
    }
    let mut terminal = terminal();
    assert_eq!(terminal.resize(&size(0, 5)), Err(Error::InvalidValue));
    // The refusal leaves the size as it was.
    assert_eq!((terminal.cols(), terminal.rows()), (80, 24));
}

#[test]
fn a_cell_pixel_size_is_taken_when_given() {
    let with_px = Size { rows: 24, cols: 80, cell_px: Some(CellPx { width: 9, height: 18 }) };
    let mut terminal = Terminal::new(&with_px, History::On).unwrap();
    assert_eq!((terminal.cols(), terminal.rows()), (80, 24));
    terminal.resize(&Size { rows: 30, cols: 100, cell_px: Some(CellPx { width: 10, height: 20 }) }).unwrap();
    assert_eq!((terminal.cols(), terminal.rows()), (100, 30));
}

// ---- screen, cursor and row reads (ST-2, ST-3) ----

#[test]
fn the_screen_text_is_the_plain_text_of_the_visible_screen() {
    let mut terminal = terminal();
    terminal.vt_write(b"first line\r\nsecond line");
    let screen = terminal.screen_text(false).unwrap();
    assert!(!screen.history_unavailable);
    assert_eq!(screen.text, "first line\nsecond line");
}

#[test]
fn history_adds_the_scrollback_and_off_says_that_it_is_unavailable() {
    // A 3-row screen with 6 lines has 3 lines of scrollback.
    let lines = "line1\r\nline2\r\nline3\r\nline4\r\nline5\r\nline6";

    let mut with_history = Terminal::new(&size(20, 3), History::On).unwrap();
    with_history.vt_write(lines.as_bytes());
    let visible = with_history.screen_text(false).unwrap();
    let all = with_history.screen_text(true).unwrap();
    assert_eq!(visible.text, "line4\nline5\nline6");
    assert_eq!(all.text, "line1\nline2\nline3\nline4\nline5\nline6");
    assert!(!all.history_unavailable);

    let mut without = Terminal::new(&size(20, 3), History::Off).unwrap();
    without.vt_write(lines.as_bytes());
    let asked = without.screen_text(true).unwrap();
    // The text is the visible screen, not an empty text that stands for it, and the flag says so.
    assert_eq!(asked.text, "line4\nline5\nline6");
    assert!(asked.history_unavailable);
    // Asking for no history is not "unavailable".
    assert!(!without.screen_text(false).unwrap().history_unavailable);
}

#[test]
fn the_cursor_is_in_zero_based_cells_and_a_wide_character_takes_two() {
    let mut terminal = terminal();
    assert_eq!(terminal.cursor(), CursorCell { row: 0, col: 0, visible: true });

    terminal.vt_write(b"abc");
    assert_eq!(terminal.cursor(), CursorCell { row: 0, col: 3, visible: true });

    terminal.vt_write("\r\n日本".as_bytes());
    // Two wide characters are four cells.
    assert_eq!(terminal.cursor(), CursorCell { row: 1, col: 4, visible: true });

    terminal.vt_write(b"\x1b[?25l");
    assert!(!terminal.cursor().visible);
}

#[test]
fn row_cells_hold_text_spaces_and_wide_characters() {
    let mut terminal = Terminal::new(&size(10, 3), History::On).unwrap();
    terminal.vt_write("a日b".as_bytes());
    let cells = terminal.row_cells(0).unwrap();
    assert_eq!(cells.len(), 10);
    // a, the wide character, the empty second cell of it, b, then empty cells that read as spaces.
    assert_eq!(&cells[..4], ["a", "日", "", "b"]);
    assert!(cells[4..].iter().all(|cell| cell == " "));

    // The untrimmed text of the row has one space per empty cell and none for the second half of a wide character.
    assert_eq!(cells.concat(), format!("a日b{}", " ".repeat(6)));

    // A written trailing space is a real cell.
    terminal.vt_write(b"\r\nx ");
    assert_eq!(terminal.row_cells(1).unwrap()[..3], ["x", " ", " "]);

    // Rows outside the screen have no cells.
    assert_eq!(terminal.row_cells(3), None);
    assert_eq!(terminal.row_cells(u32::MAX), None);
}

#[test]
fn a_grapheme_cluster_is_one_cell_of_text() {
    let mut terminal = Terminal::new(&size(10, 2), History::On).unwrap();
    // "e" followed by a combining acute accent is one cell of two codepoints.
    terminal.vt_write("e\u{301}x".as_bytes());
    let cells = terminal.row_cells(0).unwrap();
    assert_eq!(cells[0], "e\u{301}");
    assert_eq!(cells[1], "x");
}
