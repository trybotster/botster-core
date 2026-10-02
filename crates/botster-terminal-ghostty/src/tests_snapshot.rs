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

/// A terminal that the library's decoder restored from a snapshot. It keeps the unfinished parser input, so that it
/// takes the rest of the program's output as the original terminal would.
struct Restored(sys::Terminal);

impl Restored {
    fn from(bytes: &[u8]) -> Self {
        let mut decoder: sys::SnapshotDecoder = std::ptr::null_mut();
        let mut restored: sys::Terminal = std::ptr::null_mut();
        let (limit, retain) = (CONTINUATION_LIMIT, true);
        // SAFETY: the bytes outlive the decoder, the options take the types that are passed, and the decoder is freed
        // here. The restored terminal is caller-owned and freed in `Drop`.
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
            sys::ghostty_snapshot_decoder_free(decoder);
        }
        Self(restored)
    }

    fn write(&mut self, bytes: &[u8]) {
        // SAFETY: the handle is live, and the slice is valid for its length.
        unsafe { sys::ghostty_terminal_vt_write(self.0, bytes.as_ptr(), bytes.len()) }
    }

    fn snapshot(&self) -> Vec<u8> {
        // SAFETY: the handle is live, the first call asks for the size, and the buffer holds that many bytes.
        unsafe {
            let mut needed = 0usize;
            assert_eq!(
                sys::ghostty_snapshot_encode_buf(self.0, std::ptr::null_mut(), 0, &mut needed),
                sys::OUT_OF_SPACE
            );
            let mut out = vec![0u8; needed];
            let mut written = 0usize;
            assert_eq!(
                sys::ghostty_snapshot_encode_buf(self.0, out.as_mut_ptr(), out.len(), &mut written),
                sys::SUCCESS
            );
            out.truncate(written);
            out
        }
    }
}

impl Drop for Restored {
    fn drop(&mut self) {
        // SAFETY: the handle is live and freed once.
        unsafe { sys::ghostty_terminal_free(self.0) }
    }
}

/// Restore `bytes` with the library's decoder and encode the result again.
fn restore_and_encode(bytes: &[u8]) -> Vec<u8> {
    Restored::from(bytes).snapshot()
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
fn the_format_is_the_envelope_of_a_snapshot() {
    // The name and the version are the envelope of a real snapshot.
    let format = snapshot_format();
    let snapshot = terminal().snapshot().unwrap();
    assert_eq!(format.name.as_bytes(), &snapshot[..8]);
    assert_eq!(
        format.version,
        u32::from(u16::from_le_bytes([snapshot[8], snapshot[9]]))
    );
    assert!(!format.name.is_empty() && format.version > 0);
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

/// A small corpus that stops inside every kind of unfinished input (a CSI, an OSC, a DCS, a UTF-8 sequence, a string
/// that ends with an ESC) and sets the saved state that a snapshot must carry: the saved cursor, tab stops, margins and
/// the character sets.
fn corpus() -> Vec<Vec<u8>> {
    vec![
        "plain \u{e9}\u{4e2d}\u{1f600} text\r\nsecond line"
            .as_bytes()
            .to_vec(),
        b"\x1b[1;31;44mcolor\x1b[0m \x1b[3;5H\x1b[2Kx\x1b[?25l".to_vec(),
        b"\x1b]2;a long title\x07\x1b]8;;http://example.test\x1b\\link\x1b]8;;\x1b\\".to_vec(),
        b"\x1bP$qm\x1b\\\x1bP+q544e\x1b\\after".to_vec(),
        b"\x1b]52;c;aGVsbG8=\x1b\\\x1b[?1049halt\x1b[?1049l".to_vec(),
        // The saved cursor (DECSC and DECRC) with a style, then a move and a restore.
        b"\x1b[4;9H\x1b[1;32m\x1b7\x1b[10;1H\x1b[0mmoved\x1b8restored".to_vec(),
        // Tab stops: clear all, set two, and tab over them.
        b"\x1b[3g\x1b[1;7H\x1bH\x1b[1;19H\x1bH\r\tA\tB\tC".to_vec(),
        // Margins: top and bottom, then left and right, then text and a scroll inside them.
        b"\x1b[2;8r\x1b[?69h\x1b[5;30s\x1b[2;5Hinside\nmargins\n\n\n\n\n\n\nscrolled".to_vec(),
        // Character sets: DEC graphics in G0, shifted out to G1, and back.
        b"\x1b(0lqk\x1b)B\x0eabc\x0f\x1b(Bxyz".to_vec(),
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

#[test]
fn a_restored_terminal_takes_the_rest_of_the_output_at_every_byte_offset_like_the_whole_input() {
    for (index, input) in corpus().iter().enumerate() {
        let mut whole = terminal();
        whole.vt_write(input);
        let expected = whole.snapshot().unwrap();

        for offset in 0..=input.len() {
            let mut before = terminal();
            before.vt_write(&input[..offset]);
            let snapshot = before.snapshot().unwrap_or_else(|e| {
                panic!("corpus {index}, offset {offset}: {e:?}");
            });

            // The restored terminal, not the original, takes the suffix.
            let mut restored = Restored::from(&snapshot);
            restored.write(&input[offset..]);
            assert_eq!(
                restored.snapshot(),
                expected,
                "corpus {index}, offset {offset}"
            );
        }
    }
}
