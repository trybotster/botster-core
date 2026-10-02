//! Tests of the snapshot and the identity. The restore is the library's decoder, so a snapshot that restores and
//! encodes to the same bytes is its own oracle.

use botster_core_contract::prelude::Size;

use super::*;

fn terminal() -> Terminal {
    Terminal::new(&Size { rows: 10, cols: 40, cell_px: None }, History::On).unwrap()
}

/// Restore `bytes` with the library's decoder and encode the result again.
fn restore_and_encode(bytes: &[u8]) -> Vec<u8> {
    let mut decoder: sys::SnapshotDecoder = std::ptr::null_mut();
    let mut restored: sys::Terminal = std::ptr::null_mut();
    // SAFETY: the bytes outlive the decoder, and every handle is freed below.
    unsafe {
        assert_eq!(sys::ghostty_snapshot_decoder_new_buf(std::ptr::null(), &mut decoder, bytes.as_ptr(), bytes.len()), sys::SUCCESS);
        // Keep the continuation on the restored terminal, so that it can be encoded again.
        let (limit, retain) = (CONTINUATION_LIMIT, true);
        assert_eq!(sys::ghostty_snapshot_decoder_set(decoder, sys::snapshot_opt::MAX_CONTINUATION_BYTES, (&limit as *const usize).cast()), sys::SUCCESS);
        assert_eq!(sys::ghostty_snapshot_decoder_set(decoder, sys::snapshot_opt::RETAIN_CONTINUATION, (&retain as *const bool).cast()), sys::SUCCESS);
        assert_eq!(sys::ghostty_snapshot_decoder_decode(decoder, &mut restored), sys::SUCCESS);
        let mut needed = 0usize;
        assert_eq!(sys::ghostty_snapshot_encode_buf(restored, std::ptr::null_mut(), 0, &mut needed), sys::OUT_OF_SPACE);
        let mut out = vec![0u8; needed];
        let mut written = 0usize;
        assert_eq!(sys::ghostty_snapshot_encode_buf(restored, out.as_mut_ptr(), out.len(), &mut written), sys::SUCCESS);
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
fn an_unfinished_sequence_beyond_the_limit_gives_no_snapshot_and_a_terminated_one_gives_one_again() {
    let mut terminal = terminal();
    let mut input = b"\x1bP".to_vec();
    input.resize(2 + CONTINUATION_LIMIT + 4096, b'a');
    terminal.vt_write(&input);
    assert_eq!(terminal.snapshot(), Err(SnapshotError::ContinuationUnavailable));
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

    // `tic` is a part of the build host (ncurses). Without it, the test cannot run and says so.
    let Ok(tic) = std::process::Command::new("tic").arg("-V").output() else {
        panic!("tic is not installed, and the identity test needs it");
    };
    assert!(tic.status.success());
    let dir = std::env::temp_dir().join(format!("botster-terminfo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("entry.src");
    std::fs::write(&source, &identity.terminfo_source).unwrap();
    let output = std::process::Command::new("tic").arg("-x").arg("-o").arg(&dir).arg(&source).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    // The compiled entry is under a directory named by the first letter (or its hex on macOS).
    let found = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).any(|e| e.path().is_dir());
    assert!(found);
    std::fs::remove_dir_all(&dir).unwrap();
}
