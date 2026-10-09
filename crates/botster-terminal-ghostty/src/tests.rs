//! Tests of the binding. The terminal's own behaviour is the oracle: the tests feed the sequence that a program writes
//! and compare what the binding reports with the values that the sequence carries.

use botster_core_contract::prelude::{CellPx, LostKind, NotificationSource, PromptMarkKind};
use botster_route_codec::prelude::{ModeFlags, MouseEncoding, MouseTracking};

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
        assert_eq!(
            events_of(sequence.as_bytes()),
            vec![TerminalEvent::Title(title.to_owned())]
        );
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
    let events =
        events_of(b"\x1b]133;A\x07\x1b]133;B\x07\x1b]133;C\x07\x1b]133;D;3\x07\x1b]133;D\x07");
    assert_eq!(
        events,
        vec![
            TerminalEvent::PromptMark {
                mark: PromptMarkKind::PromptStart,
                exit_code: None
            },
            TerminalEvent::PromptMark {
                mark: PromptMarkKind::CommandStart,
                exit_code: None
            },
            TerminalEvent::PromptMark {
                mark: PromptMarkKind::CommandExecuted,
                exit_code: None
            },
            TerminalEvent::PromptMark {
                mark: PromptMarkKind::CommandFinished,
                exit_code: Some(3)
            },
            TerminalEvent::PromptMark {
                mark: PromptMarkKind::CommandFinished,
                exit_code: None
            },
        ]
    );
}

// ---- clipboard writes (Core Amendment 13, A13-1, A13-1b) ----

/// The one clipboard write that `bytes` produces.
fn clipboard_write_of(terminal: &mut Terminal, bytes: &[u8]) -> ClipboardWrite {
    clipboard_write_and_acks(terminal, bytes).0
}

/// The one clipboard write that `bytes` produces, and the acknowledgements of the drain.
fn clipboard_write_and_acks(
    terminal: &mut Terminal,
    bytes: &[u8],
) -> (ClipboardWrite, Vec<Vec<u8>>) {
    terminal.vt_write(bytes);
    let mut drained = terminal.drain_events();
    assert_eq!(drained.events.len(), 1, "{:?}", drained.events);
    match drained.events.remove(0) {
        TerminalEvent::ClipboardWrite(write) => (write, drained.clipboard_acks),
        other => panic!("expected a clipboard write, got {other:?}"),
    }
}

fn entry(mime: &str, bytes: &[u8]) -> ClipboardEntry {
    ClipboardEntry {
        mime: mime.to_owned(),
        bytes: bytes.to_vec(),
    }
}

#[test]
fn osc_52_writes_report_the_selection_location_terminator_and_the_decoded_bytes() {
    // "aGk=" is "hi". The selection is the string as written; the location is the library's mapping of it.
    for (selection, location) in [
        ("c", ClipboardLocation::Standard),
        ("p", ClipboardLocation::Primary),
        ("s", ClipboardLocation::Selection),
        ("q", ClipboardLocation::Standard),
        ("0", ClipboardLocation::Standard),
        ("s0", ClipboardLocation::Selection),
        ("cp", ClipboardLocation::Standard),
        ("cpqs01234567", ClipboardLocation::Standard),
    ] {
        for (terminator, expected) in [("\x07", Terminator::Bel), ("\x1b\\", Terminator::St)] {
            let sequence = format!("\x1b]52;{selection};aGk={terminator}");
            let write = clipboard_write_of(&mut terminal(), sequence.as_bytes());
            assert_eq!(write.selection.as_deref(), Some(selection));
            assert_eq!(write.location, location, "selection {selection:?}");
            assert_eq!(write.terminator, expected);
            assert_eq!(write.contents, Some(vec![entry("text/plain", b"hi")]));
            assert_eq!((write.total_bytes, write.too_large), (2, false));
        }
    }
    // A program that leaves the selection out gives none, and the location says where the write goes.
    let write = clipboard_write_of(&mut terminal(), b"\x1b]52;;aGk=\x07");
    assert_eq!(write.selection, None);
    assert_eq!(write.location, ClipboardLocation::Standard);
}

#[test]
fn an_osc_52_clear_has_no_entries_and_is_not_an_empty_value() {
    let clear = clipboard_write_of(&mut terminal(), b"\x1b]52;s;\x1b\\");
    assert_eq!(clear.contents, Some(Vec::new()));
    assert_eq!(clear.total_bytes, 0);
    assert!(!clear.too_large);

    // A kitty write of one representation with no bytes is one entry with empty bytes.
    let mut terminal = terminal();
    terminal.vt_write(b"\x1b]5522;type=write:id=1\x1b\\");
    terminal.vt_write(b"\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;\x1b\\");
    let empty = clipboard_write_of(&mut terminal, b"\x1b]5522;type=wdata\x1b\\");
    assert_eq!(empty.contents, Some(vec![entry("text/plain", b"")]));
    assert_ne!(empty.contents, clear.contents);
}

#[test]
fn osc_1337_copy_is_a_clipboard_write_without_a_selection_or_an_acknowledgement() {
    let write = clipboard_write_of(&mut terminal(), b"\x1b]1337;Copy=:aVRlcm0y\x1b\\");
    assert_eq!(write.selection, None);
    assert_eq!(write.contents, Some(vec![entry("text/plain", b"iTerm2")]));
}

/// The transaction of an OSC 5522 write with two representations, one in two chunks. It returns the write and the
/// acknowledgements of the drain.
fn kitty_write(terminal: &mut Terminal) -> (ClipboardWrite, Vec<Vec<u8>>) {
    terminal.vt_write(b"\x1b]5522;type=write:id=42\x1b\\");
    terminal.vt_write(b"\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;R2hvc3Q=\x1b\\");
    terminal.vt_write(b"\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;dHk=\x1b\\");
    terminal.vt_write(b"\x1b]5522;type=wdata:mime=dGV4dC9odG1s;PGI+aGk8L2I+\x1b\\");
    clipboard_write_and_acks(terminal, b"\x1b]5522;type=wdata\x1b\\")
}

#[test]
fn osc_52_and_osc_1337_writes_have_no_acknowledgement() {
    let mut terminal = terminal();
    terminal.vt_write(b"\x1b]52;c;aGk=\x07\x1b]1337;Copy=:aVRlcm0y\x1b\\");
    let drained = terminal.drain_events();
    assert_eq!(drained.events.len(), 2);
    assert!(drained.clipboard_acks.is_empty());
    assert!(drained.pty_writes.is_empty());
}

#[test]
fn an_osc_5522_write_keeps_every_representation_in_order_and_returns_the_acknowledgement_as_a_value(
) {
    let mut terminal = terminal();
    let (write, acks) = kitty_write(&mut terminal);
    assert_eq!(write.selection, None);
    assert_eq!(write.location, ClipboardLocation::Standard);
    assert_eq!(
        write.contents,
        Some(vec![
            entry("text/plain", b"Ghostty"),
            entry("text/html", b"<b>hi</b>")
        ])
    );
    assert_eq!((write.total_bytes, write.too_large), (16, false));
    // The model wrote one acknowledgement while it handled the reply. It is its own item, and nothing went to the pty.
    assert_eq!(acks.len(), 1);
    assert!(!acks[0].is_empty());
    let drained = terminal.drain_events();
    assert!(drained.pty_writes.is_empty());
    assert_eq!(drained.unrouted_queries, 0);
}

#[test]
fn a_write_over_the_limit_is_surfaced_without_its_bytes_and_the_model_gets_io_error() {
    let mut within = terminal();
    let (_, ok) = kitty_write(&mut within);

    let mut terminal = terminal();
    // The limit counts the bytes of all representations: 7 + 9 = 16.
    terminal.set_clipboard_limit(15);
    let (over, over_acks) = kitty_write(&mut terminal);
    assert!(over.too_large);
    assert_eq!(over.contents, None);
    assert_eq!(over.total_bytes, 16);
    // The program is told a different status (IO_ERROR, not DONE), and the answer is still a value.
    assert_eq!(over_acks.len(), 1);
    assert_ne!(over_acks, ok);

    // At the limit it is within.
    let mut at_limit = self::terminal();
    at_limit.set_clipboard_limit(16);
    assert!(!kitty_write(&mut at_limit).0.too_large);
}

/// An OSC 5522 write with id 9: one chunk of `len` bytes for each `(mime, len)` in order, then a `walias` of the first
/// MIME type for each name in `aliases`, then the commit.
fn kitty_write_of(chunks: &[(&str, usize)], aliases: &[&str]) -> Vec<u8> {
    use base64::Engine;
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut out = b"\x1b]5522;type=write:id=9\x1b\\".to_vec();
    for &(mime, len) in chunks {
        let (mime, data) = (b64(mime.as_bytes()), b64(&vec![b'x'; len]));
        out.extend_from_slice(format!("\x1b]5522;type=wdata:mime={mime};{data}\x1b\\").as_bytes());
    }
    for alias in aliases {
        let (mime, alias) = (b64(chunks[0].0.as_bytes()), b64(alias.as_bytes()));
        out.extend_from_slice(
            format!("\x1b]5522;type=walias:mime={mime};{alias}\x1b\\").as_bytes(),
        );
    }
    out.extend_from_slice(b"\x1b]5522;type=wdata\x1b\\");
    out
}

/// The one clipboard write of `bytes`, its acknowledgements, and whether anything went to the pty.
fn kitty_outcome(terminal: &mut Terminal, bytes: &[u8]) -> (ClipboardWrite, Vec<Vec<u8>>, bool) {
    terminal.vt_write(bytes);
    let mut drained = terminal.drain_events();
    assert_eq!(drained.events.len(), 1, "{:?}", drained.events);
    let TerminalEvent::ClipboardWrite(write) = drained.events.remove(0) else {
        panic!("expected a clipboard write, got {:?}", drained.events);
    };
    (
        write,
        drained.clipboard_acks,
        !drained.pty_writes.is_empty(),
    )
}

#[test]
fn the_clipboard_limit_decides_on_the_decoded_size_then_the_contents_size() {
    // Core A14 (final33): step 1 on the decoded size (the model's report, with the model's limit set to the bound),
    // step 2 on the contents size (aliases at their full length). Every expected size is derived from the input.
    const LIMIT: usize = 30;
    let mut terminal = terminal();
    terminal.set_clipboard_limit(LIMIT);

    // Within: a write of exactly the bound is carried (A14-3), and the model gets SUCCESS.
    let (within, within_acks, pty) = kitty_outcome(
        &mut terminal,
        &kitty_write_of(&[("text/plain", LIMIT)], &[]),
    );
    assert!(!within.too_large && !pty);
    assert_eq!(
        within.contents,
        Some(vec![entry("text/plain", &[b'x'; LIMIT])])
    );
    assert_eq!(within.total_bytes, LIMIT as u64);
    assert_eq!(within_acks.len(), 1);

    // Step 2: the model keeps 20 bytes, and the alias makes the contents 40. TooLarge with the contents size; the
    // binding answers IO_ERROR, a different acknowledgement.
    let half = LIMIT * 2 / 3;
    let (alias, alias_acks, pty) = kitty_outcome(
        &mut terminal,
        &kitty_write_of(&[("text/plain", half)], &["TEXT"]),
    );
    assert!(alias.too_large && !pty);
    assert_eq!(alias.contents, None);
    assert_eq!(alias.total_bytes, 2 * half as u64);
    assert_eq!(alias_acks.len(), 1);
    assert_ne!(alias_acks, within_acks);

    // Step 1: text/plain returns after text/html and replaces its first region. The final contents would be 2
    // bytes, but the decoded size is over the bound, so the model kept nothing and reports the decoded size. The
    // model answers nothing itself: the only acknowledgement is the binding's IO_ERROR, the same as for the alias
    // write, and nothing goes to the pty. (At libghostty's default limit this write would be carried: the bound,
    // not the model's default, decides.)
    let replaced = [("text/plain", LIMIT), ("text/html", 1), ("text/plain", 1)];
    let (over, over_acks, pty) =
        kitty_outcome(&mut terminal, &kitty_write_of(&replaced, &["TEXT"]));
    assert!(over.too_large && !pty);
    assert_eq!(over.contents, None);
    assert_eq!(
        over.total_bytes,
        replaced.iter().map(|c| c.1 as u64).sum::<u64>()
    );
    assert_eq!(over_acks, alias_acks);

    // Step 1 over many chunks: the model counts past its limit, every chunk.
    let chunks = [("text/plain", LIMIT / 2); 4];
    let (counted, _, _) = kitty_outcome(&mut terminal, &kitty_write_of(&chunks, &[]));
    assert!(counted.too_large);
    assert_eq!(
        counted.total_bytes,
        chunks.iter().map(|c| c.1 as u64).sum::<u64>()
    );
}

#[test]
fn an_acknowledgement_survives_a_full_event_buffer_with_no_host() {
    // The event buffer fills with bells in one model step, and the clipboard write that follows is dropped as an
    // event (class D). Its acknowledgement is a separate item, and it is still delivered (A13-1b).
    let mut terminal = terminal();
    let mut input = vec![0x07u8; MAX_BUFFERED_EVENTS + 10];
    input.extend_from_slice(b"\x1b]5522;type=write:id=7\x1b\\");
    input.extend_from_slice(b"\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;aGk=\x1b\\");
    input.extend_from_slice(b"\x1b]5522;type=wdata\x1b\\");
    terminal.vt_write(&input);
    let drained = terminal.drain_events();
    assert_eq!(drained.events.len(), MAX_BUFFERED_EVENTS);
    assert!(drained.dropped > 0);
    assert!(drained.dropped_kinds.contains(&LostKind::ClipboardWrite));
    assert!(!drained
        .events
        .iter()
        .any(|e| matches!(e, TerminalEvent::ClipboardWrite(_))));
    assert_eq!(drained.clipboard_acks.len(), 1);
    assert!(!drained.clipboard_acks[0].is_empty());
}

#[test]
fn acknowledgements_come_in_the_order_of_their_writes_and_a_backlog_stops_the_write() {
    let mut terminal = terminal();
    let commit = |id: u32| {
        format!(
            "\x1b]5522;type=write:id={id}\x1b\\\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;aGk=\x1b\\\x1b]5522;type=wdata\x1b\\"
        )
    };
    let input = [commit(1), commit(2), commit(3)].concat();
    terminal.vt_write(input.as_bytes());
    let acks = terminal.drain_events().clipboard_acks;
    assert_eq!(acks.len(), 3);
    // Each acknowledgement is the library's answer to its own write: the one that a fresh terminal gives to that write
    // alone. The three answers differ, so the comparison also checks the order.
    let isolated: Vec<Vec<u8>> = (1..=3)
        .map(|id| {
            let mut fresh = self::terminal();
            fresh.vt_write(commit(id).as_bytes());
            let mut alone = fresh.drain_events().clipboard_acks;
            assert_eq!(alone.len(), 1);
            alone.remove(0)
        })
        .collect();
    assert!(isolated[0] != isolated[1] && isolated[1] != isolated[2] && isolated[0] != isolated[2]);
    assert_eq!(acks, isolated);

    // Past the backlog limit, the PTY write path refuses until the caller drains.
    terminal.set_ack_backlog_limit(10);
    terminal.vt_write(commit(4).as_bytes());
    assert_eq!(terminal.vt_write_until_query(b"x"), Err(Error::AckBacklog));
    assert_eq!(terminal.drain_events().clipboard_acks.len(), 1);
    assert!(terminal.vt_write_until_query(b"x").is_ok());
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
    assert_eq!(
        terminal.modes(),
        ModeFlags {
            ..terminal_default()
        }
    );
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
    for line in header
        .lines()
        .filter(|l| l.starts_with("#define GHOSTTY_MODE_"))
    {
        let Some(start) = line.find("ghostty_mode_new(") else {
            continue;
        };
        let args = &line[start + "ghostty_mode_new(".len()..];
        let args = &args[..args.find(')').unwrap()];
        let (value, ansi) = args.split_once(',').unwrap();
        in_header.insert((value.trim().parse::<u16>().unwrap(), ansi.trim() == "true"));
    }
    assert!(
        in_header.len() > 30,
        "the header parse found {} modes",
        in_header.len()
    );

    let mut covered: BTreeSet<(u16, bool)> = modes::OTHER_MODES.iter().copied().collect();
    covered.extend(modes::normative_dec().iter().map(|&value| (value, false)));
    assert_eq!(
        in_header, covered,
        "a mode of the pinned header is in neither list, or a listed mode is not in it"
    );
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
        assert_eq!(
            Terminal::new(&size(cols, rows), History::On).err(),
            Some(Error::InvalidValue),
            "{cols}x{rows}"
        );
    }
    let mut terminal = terminal();
    assert_eq!(terminal.resize(&size(0, 5)), Err(Error::InvalidValue));
    // The refusal leaves the size as it was.
    assert_eq!((terminal.cols(), terminal.rows()), (80, 24));
}

#[test]
fn a_cell_pixel_size_is_taken_when_given() {
    let with_px = Size {
        rows: 24,
        cols: 80,
        cell_px: Some(CellPx {
            width: 9,
            height: 18,
        }),
    };
    let mut terminal = Terminal::new(&with_px, History::On).unwrap();
    assert_eq!((terminal.cols(), terminal.rows()), (80, 24));
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
    assert_eq!(
        terminal.cursor(),
        CursorCell {
            row: 0,
            col: 0,
            visible: true
        }
    );

    terminal.vt_write(b"abc");
    assert_eq!(
        terminal.cursor(),
        CursorCell {
            row: 0,
            col: 3,
            visible: true
        }
    );

    terminal.vt_write("\r\n日本".as_bytes());
    // Two wide characters are four cells.
    assert_eq!(
        terminal.cursor(),
        CursorCell {
            row: 1,
            col: 4,
            visible: true
        }
    );

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

// ---- review finding P39: constants, defaults and the byte accounting of the event buffer ----

/// The value of `#define <name> (1 << n)` or of `<name> = <n>,` in a pinned header.
fn header_value(header: &str, name: &str) -> i64 {
    for line in header.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix(&format!("#define {name} ")) {
            let shift = rest
                .trim()
                .trim_start_matches("(1 <<")
                .trim_end_matches(')');
            return 1 << shift.trim().parse::<u32>().unwrap();
        }
        if let Some(rest) = line.strip_prefix(&format!("{name} =")) {
            return rest.trim().trim_end_matches(',').parse().unwrap();
        }
    }
    panic!("{name} is not in the header");
}

#[test]
fn the_declared_result_codes_and_modifier_bits_are_those_of_the_pinned_headers() {
    let terminal = include_str!("../vendor/ghostty/include/ghostty/vt/terminal.h");
    assert_eq!(
        header_value(terminal, "GHOSTTY_TERMINAL_DATA_VT_PROCESSING_ERROR"),
        i64::from(sys::data::VT_PROCESSING_ERROR)
    );
    let types = include_str!("../vendor/ghostty/include/ghostty/vt/types.h");
    for (name, value) in [
        ("GHOSTTY_SUCCESS", sys::SUCCESS),
        ("GHOSTTY_OUT_OF_MEMORY", sys::OUT_OF_MEMORY),
        ("GHOSTTY_INVALID_VALUE", sys::INVALID_VALUE),
        ("GHOSTTY_OUT_OF_SPACE", sys::OUT_OF_SPACE),
        ("GHOSTTY_NO_VALUE", sys::NO_VALUE),
    ] {
        assert_eq!(header_value(types, name), i64::from(value), "{name}");
    }
    let key_event = include_str!("../vendor/ghostty/include/ghostty/vt/key/event.h");
    for (name, value) in [
        ("GHOSTTY_MODS_SHIFT", sys::mods::SHIFT),
        ("GHOSTTY_MODS_CTRL", sys::mods::CTRL),
        ("GHOSTTY_MODS_ALT", sys::mods::ALT),
        ("GHOSTTY_MODS_SUPER", sys::mods::SUPER),
        ("GHOSTTY_MODS_CAPS_LOCK", sys::mods::CAPS_LOCK),
        ("GHOSTTY_MODS_NUM_LOCK", sys::mods::NUM_LOCK),
        ("GHOSTTY_MODS_HYPER", sys::mods::HYPER),
        ("GHOSTTY_MODS_META", sys::mods::META),
    ] {
        assert_eq!(header_value(key_event, name), i64::from(value), "{name}");
    }
}

#[test]
fn the_defaults_that_the_contract_names_are_its_default_limits() {
    let limits = botster_core_contract::prelude::CoreLimits::default();
    assert_eq!(DEFAULT_QUERY_REQUEST_BYTES as u64, limits.max_query_bytes);
    assert_eq!(DEFAULT_CLIPBOARD_BYTES as u64, limits.clipboard_bytes);
    assert_eq!(MAX_SHADOW_REPLY_BYTES as u64, limits.max_query_reply_bytes);

    // A new terminal applies the clipboard default: a write one byte over it is too large.
    let mut terminal = terminal();
    let base64 = |n: usize| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(vec![b'x'; n])
    };
    let at = clipboard_write_of(
        &mut terminal,
        format!("\x1b]52;c;{}\x1b\\", base64(DEFAULT_CLIPBOARD_BYTES)).as_bytes(),
    );
    assert!(!at.too_large);
    let over = clipboard_write_of(
        &mut terminal,
        format!("\x1b]52;c;{}\x1b\\", base64(DEFAULT_CLIPBOARD_BYTES + 1)).as_bytes(),
    );
    assert!(over.too_large);
}

fn commit_with_id(id: u32) -> Vec<u8> {
    format!(
        "\x1b]5522;type=write:id={id}\x1b\\\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;aGk=\x1b\\\x1b]5522;type=wdata\x1b\\"
    )
    .into_bytes()
}

#[test]
fn the_default_backlog_takes_many_acknowledgements_and_a_backlog_at_the_limit_is_not_over_it() {
    // Clipboard writes give one acknowledgement each. Many kilobytes of them stay below the default (1 MiB).
    let mut terminal = terminal();
    for id in 0..200 {
        terminal.vt_write(&commit_with_id(id));
    }
    assert!(terminal.vt_write_until_query(b"x").is_ok());
    let acks = terminal.drain_events().clipboard_acks;
    assert_eq!(acks.len(), 200);
    assert!(acks.iter().map(Vec::len).sum::<usize>() > 4096);

    // A backlog exactly at the limit is not over it; one more acknowledgement is.
    let mut terminal = self::terminal();
    terminal.vt_write(&commit_with_id(1));
    let one = terminal.drain_events().clipboard_acks[0].len();
    terminal.set_ack_backlog_limit(one);
    terminal.vt_write(&commit_with_id(1));
    assert!(terminal.vt_write_until_query(b"x").is_ok());
    terminal.vt_write(&commit_with_id(1));
    assert_eq!(terminal.vt_write_until_query(b"x"), Err(Error::AckBacklog));
}

#[test]
fn the_library_keeps_the_scrollback_limit_of_the_history_setting() {
    let limit = |history: History| {
        let terminal = Terminal::new(&size(80, 24), history).unwrap();
        let mut bytes: usize = usize::MAX;
        // SAFETY: the handle is live, and the key writes a `size_t`.
        let code = unsafe {
            sys::ghostty_terminal_get(
                terminal.handle.as_ptr(),
                sys::data::SCROLLBACK_MAX_BYTES,
                (&mut bytes as *mut usize).cast(),
            )
        };
        assert_eq!(code, sys::SUCCESS);
        bytes
    };
    // `History::On` bounds the scrollback at 16 MiB (`HISTORY_MAX_BYTES`); `Off` keeps none.
    assert_eq!(limit(History::On), 16 * 1024 * 1024);
    assert_eq!(limit(History::Off), 0);
}

#[test]
fn the_row_after_the_last_has_no_cells() {
    let terminal = terminal();
    assert!(terminal.row_cells(23).is_some());
    assert_eq!(terminal.row_cells(24), None);
}

/// The bytes that the event buffer counts for an event: the text of a notification (title and body), and the MIME
/// types and contents of a clipboard write.
fn counted_bytes(event: &TerminalEvent) -> usize {
    match event {
        TerminalEvent::Notification { title, body, .. } => {
            title.as_ref().map_or(0, String::len) + body.len()
        }
        TerminalEvent::ClipboardWrite(write) => {
            write.selection.as_ref().map_or(0, String::len)
                + write
                    .contents
                    .as_ref()
                    .map_or(0, |c| c.iter().map(|e| e.mime.len() + e.bytes.len()).sum())
        }
        other => panic!("{other:?}"),
    }
}

/// An OSC 52 write with no selection that the buffer counts as `n` bytes (its MIME type and its contents).
fn clipboard_write_counted_as(n: usize) -> Vec<u8> {
    use base64::Engine;
    let mime = match &events_of(b"\x1b]52;;aGk=\x1b\\")[0] {
        TerminalEvent::ClipboardWrite(write) => write.contents.as_ref().unwrap()[0].mime.len(),
        other => panic!("{other:?}"),
    };
    let text = base64::engine::general_purpose::STANDARD.encode(vec![b'x'; n - mime]);
    format!("\x1b]52;;{text}\x1b\\").into_bytes()
}

/// Clipboard writes and then `last` make exactly `MAX_BUFFERED_BYTES` in the buffer, and all are kept; a further
/// notification of one byte is dropped. A notification is at most 2048 bytes (the library's OSC buffer), so the
/// writes fill the rest.
fn last_event_fills_the_byte_bound(last: &[u8]) {
    // The documented bound of the buffer: 16 MiB of event text between two drains.
    assert_eq!(MAX_BUFFERED_BYTES, 16 * 1024 * 1024);
    let mut probe = terminal();
    probe.vt_write(last);
    let last_size = counted_bytes(&probe.drain_events().events[0]);

    let mut rest = MAX_BUFFERED_BYTES - last_size;
    let mut input = Vec::new();
    while rest > 0 {
        // Pieces of 8 KiB, and a last piece that still holds its MIME type.
        let piece = if rest >= 16 * 1024 { 8 * 1024 } else { rest };
        input.extend_from_slice(&clipboard_write_counted_as(piece));
        rest -= piece;
    }
    input.extend_from_slice(last);

    let mut terminal = terminal();
    terminal.vt_write(&input);
    let drained = terminal.drain_events();
    assert_eq!(drained.dropped, 0, "exactly at the bound");
    assert_eq!(
        drained.events.iter().map(counted_bytes).sum::<usize>(),
        MAX_BUFFERED_BYTES
    );
    assert!(drained.events.len() < MAX_BUFFERED_EVENTS);

    terminal.vt_write(&input);
    terminal.vt_write(b"\x1b]9;x\x1b\\");
    let drained = terminal.drain_events();
    assert_eq!(drained.dropped, 1, "one byte over the bound");
    assert!(drained.dropped_kinds.contains(&LostKind::Notification));
}

#[test]
fn the_event_buffer_counts_the_bytes_of_notifications_and_clipboard_writes() {
    // OSC 9: the body. OSC 777: the title and the body. OSC 52 with no selection: the MIME type and the contents.
    last_event_fills_the_byte_bound(format!("\x1b]9;{}\x1b\\", "x".repeat(1000)).as_bytes());
    last_event_fills_the_byte_bound(
        format!("\x1b]777;notify;{};b\x1b\\", "t".repeat(999)).as_bytes(),
    );
    last_event_fills_the_byte_bound(&clipboard_write_counted_as(5000));
}

#[test]
fn an_invalid_byte_inside_a_reported_string_is_a_replacement_character() {
    // The library passes the bytes of a notification as they are. A byte that is not UTF-8 inside it becomes U+FFFD
    // and the rest is kept; only an incomplete character at the end is cut.
    let body = b"ab\xffcd";
    let events = events_of(&[&b"\x1b]9;"[..], body, b"\x1b\\"].concat());
    match &events[..] {
        [TerminalEvent::Notification { body: text, .. }] => {
            assert_eq!(text, &String::from_utf8_lossy(body));
        }
        other => panic!("{other:?}"),
    }
}
