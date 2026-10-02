//! Tests of the snapshot and the identity. The restore is the library's decoder, so a snapshot that restores and
//! encodes to the same bytes is its own oracle.

use botster_core_contract::prelude::Size;

use super::*;

fn terminal() -> Terminal {
    Terminal::new(
        &Size {
            rows: 10,
            cols: 40,
            cell_px: None,
        },
        History::On,
    )
    .unwrap()
}

/// Restore `bytes` with the library's decoder and encode the result again.
fn restore_and_encode(bytes: &[u8]) -> Vec<u8> {
    let mut decoder: sys::SnapshotDecoder = std::ptr::null_mut();
    let mut restored: sys::Terminal = std::ptr::null_mut();
    // SAFETY: the bytes outlive the decoder, and every handle is freed below.
    unsafe {
        assert_eq!(
            sys::ghostty_snapshot_decoder_new_buf(
                std::ptr::null(),
                &mut decoder,
                bytes.as_ptr(),
                bytes.len()
            ),
            sys::SUCCESS
        );
        // Keep the continuation on the restored terminal, so that it can be encoded again.
        let (limit, retain) = (CONTINUATION_LIMIT, true);
        assert_eq!(
            sys::ghostty_snapshot_decoder_set(
                decoder,
                sys::snapshot_opt::MAX_CONTINUATION_BYTES,
                (&limit as *const usize).cast()
            ),
            sys::SUCCESS
        );
        assert_eq!(
            sys::ghostty_snapshot_decoder_set(
                decoder,
                sys::snapshot_opt::RETAIN_CONTINUATION,
                (&retain as *const bool).cast()
            ),
            sys::SUCCESS
        );
        assert_eq!(
            sys::ghostty_snapshot_decoder_decode(decoder, &mut restored),
            sys::SUCCESS
        );
        let mut needed = 0usize;
        assert_eq!(
            sys::ghostty_snapshot_encode_buf(restored, std::ptr::null_mut(), 0, &mut needed),
            sys::OUT_OF_SPACE
        );
        let mut out = vec![0u8; needed];
        let mut written = 0usize;
        assert_eq!(
            sys::ghostty_snapshot_encode_buf(restored, out.as_mut_ptr(), out.len(), &mut written),
            sys::SUCCESS
        );
        out.truncate(written);
        sys::ghostty_terminal_free(restored);
        sys::ghostty_snapshot_decoder_free(decoder);
        out
    }
}

#[test]
fn a_snapshot_is_the_same_twice_and_restores_to_the_same_snapshot() {
    let mut terminal = terminal();
    // More lines than rows, so that the scrollback holds some.
    for line in 0..30 {
        terminal.vt_write(format!("line {line}\r\n").as_bytes());
    }
    terminal.vt_write(b"\x1b[?1h\x1b[?2004h\x1b]2;a title\x07\x1b[31mred");
    let first = terminal.snapshot().unwrap();
    assert!(!first.is_empty());
    assert_eq!(first, terminal.snapshot().unwrap());
    assert_eq!(restore_and_encode(&first), first);
}

#[test]
fn an_unfinished_sequence_within_the_limit_is_in_the_snapshot() {
    let mut terminal = terminal();
    terminal.vt_write(b"text\x1b]0;an unfinished ti");
    let snapshot = terminal.snapshot().unwrap();
    assert_eq!(restore_and_encode(&snapshot), snapshot);
}

#[test]
fn an_unfinished_sequence_beyond_the_limit_gives_no_snapshot_and_a_terminated_one_gives_one_again()
{
    let mut terminal = terminal();
    let mut input = b"\x1bP".to_vec();
    input.resize(2 + CONTINUATION_LIMIT + 4096, b'a');
    terminal.vt_write(&input);
    assert_eq!(
        terminal.snapshot(),
        Err(SnapshotError::ContinuationUnavailable)
    );
    // The refusal changed nothing: ending the string returns the parser to ground and the snapshot is available.
    terminal.vt_write(b"\x1b\\");
    assert!(terminal.snapshot().is_ok());
}

#[test]
fn the_format_names_the_limit() {
    assert_eq!(SNAPSHOT_FORMAT.continuation_limit, CONTINUATION_LIMIT);
}

#[test]
fn the_identity_comes_from_the_library_and_tic_compiles_it() {
    let identity = terminal_identity();
    assert!(!identity.term.is_empty());
    assert!(identity.terminfo_source.starts_with(&identity.term));

    // `tic` is a part of the build host (ncurses). Without it, the test cannot run and says so. `-c` checks the whole
    // entry and writes nothing.
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut tic = Command::new("tic")
        .args(["-x", "-c", "/dev/stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tic is not installed, and the identity test needs it");
    tic.stdin
        .take()
        .unwrap()
        .write_all(identity.terminfo_source.as_bytes())
        .unwrap();
    let output = tic.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A small corpus that stops inside every kind of unfinished input: a CSI, an OSC, a DCS, a UTF-8 sequence and a
/// string that ends with an ESC.
fn corpus() -> Vec<Vec<u8>> {
    vec![
        "plain \u{e9}\u{4e2d}\u{1f600} text\r\nsecond line"
            .as_bytes()
            .to_vec(),
        b"\x1b[1;31;44mcolor\x1b[0m \x1b[3;5H\x1b[2Kx\x1b[?25l".to_vec(),
        b"\x1b]2;a long title\x07\x1b]8;;http://example.test\x1b\\link\x1b]8;;\x1b\\".to_vec(),
        b"\x1bP$qm\x1b\\\x1bP+q544e\x1b\\after".to_vec(),
        b"\x1b]52;c;aGVsbG8=\x1b\\\x1b[?1049halt\x1b[?1049l".to_vec(),
    ]
}

#[test]
fn a_snapshot_at_every_byte_offset_restores_and_equals_the_snapshot_of_a_split_write() {
    for (index, input) in corpus().iter().enumerate() {
        let mut whole = terminal();
        whole.vt_write(input);
        let expected = whole.snapshot().unwrap();

        for offset in 0..=input.len() {
            let mut split = terminal();
            split.vt_write(&input[..offset]);
            // The snapshot at the offset exists, whatever the parser holds, and restores to itself.
            let at_offset = split
                .snapshot()
                .unwrap_or_else(|e| panic!("corpus {index}, offset {offset}: {e:?}"));
            assert_eq!(
                restore_and_encode(&at_offset),
                at_offset,
                "corpus {index}, offset {offset}"
            );

            // Writing the rest in a second call gives the same terminal as one write.
            split.vt_write(&input[offset..]);
            assert_eq!(
                split.snapshot().unwrap(),
                expected,
                "corpus {index}, offset {offset}"
            );
        }
    }
}
