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

#[test]
fn restored_cell_attributes_colors_and_cursor_appearance_match_the_source() {
    let mut source = terminal();
    for input in [
        &b"\x1b[1;3;4;5;7;8;9;53;38;2;11;22;33;48;5;123;58;2;44;55;66mA"[..],
        &b"\x1b[0;2;4:3;38;5;45;48;2;77;88;99mB\x1b[0m"[..],
        "界".as_bytes(),
        &b"\x1b[1\"qP\x1b[0\"q\x1b]4;2;rgb:12/34/56\x1b\\"[..],
        &b"\x1b]10;rgb:34/56/78\x1b\\\x1b]11;rgb:45/67/89\x1b\\\x1b]12;rgb:56/78/9a\x1b\\"[..],
        &b"\x1b[5 q"[..],
        &b"\x1b[2 q"[..],
        &b"\x1b[3 q\x1b[44m\x1b[K"[..],
    ] {
        source.vt_write(input);
        let before = source.snapshot().unwrap();
        let mut restored = Terminal::from_snapshot(&before, History::On, None).unwrap();
        assert_eq!(source.colors().unwrap(), restored.colors().unwrap());
        assert_eq!(
            source.cursor_appearance().unwrap(),
            restored.cursor_appearance().unwrap()
        );
        for row in 0..10 {
            for col in 0..40 {
                assert_eq!(
                    source.cell_attributes(row, col, false).unwrap(),
                    restored.cell_attributes(row, col, false).unwrap()
                );
            }
        }
        assert_eq!(source.snapshot().unwrap(), before);
        assert_eq!(restored.snapshot().unwrap(), before);
    }
    for (row, col) in [(10, 0), (0, 40), (0, u32::MAX)] {
        assert_eq!(
            source.cell_attributes(row, col, false),
            Err(Error::InvalidValue)
        );
    }
}

#[test]
fn color_and_cursor_reads_change_independently() {
    let mut source = terminal();
    let cell = source.cell_attributes(0, 0, false).unwrap();
    let colors = source.colors().unwrap();
    let cursor = source.cursor_appearance().unwrap();
    source.vt_write(b"\x1b]4;2;rgb:12/34/56\x1b\\");
    assert_ne!(source.colors().unwrap(), colors);
    assert_eq!(source.cursor_appearance().unwrap(), cursor);
    assert_eq!(source.cell_attributes(0, 0, false).unwrap(), cell);
    let colors = source.colors().unwrap();
    source.vt_write(b"\x1b[5 q");
    assert_ne!(source.cursor_appearance().unwrap(), cursor);
    assert_eq!(source.colors().unwrap(), colors);
    assert_eq!(source.cell_attributes(0, 0, false).unwrap(), cell);
}

#[test]
fn history_cell_reads_match_the_restored_history() {
    let mut source = terminal();
    source
        .vt_write(b"\x1b]8;;https://example.test/history\x1b\\\x1b[1mhistory\x1b]8;;\x1b\\\x1b[0m");
    let uri = source.hyperlink_uri(0, 0).unwrap();
    let attributes = source.cell_attributes(0, 0, false).unwrap();
    for _ in 0..12 {
        source.vt_write(b"\r\n");
    }
    let restored = Terminal::from_snapshot(&source.snapshot().unwrap(), History::On, None).unwrap();
    assert_eq!(source.cell_hyperlink_uri(0, 0, true).unwrap(), uri);
    assert_eq!(restored.cell_hyperlink_uri(0, 0, true).unwrap(), uri);
    assert_eq!(source.cell_attributes(0, 0, true).unwrap(), attributes);
    assert_eq!(restored.cell_attributes(0, 0, true).unwrap(), attributes);
}

/// A terminal that the library's decoder restored from a snapshot. It keeps the unfinished parser input, so that it
/// takes the rest of the program's output as the original terminal would.
struct Restored(sys::Terminal, Option<Terminal>);

impl Restored {
    /// A restore with the session's image storage limit (zero): the restored model ignores image sequences too.
    fn from(bytes: &[u8]) -> Self {
        let terminal = Terminal::from_snapshot(bytes, History::On, None).unwrap();
        Self(terminal.handle.as_ptr(), Some(terminal))
    }

    /// A restore that leaves the image storage limit at the library default. It is the control of the image-resume test:
    /// it shows that the restored model stores images unless the decoder takes the session's limit.
    fn with_the_library_default_image_limit(bytes: &[u8]) -> Self {
        Self::decode(bytes, None)
    }

    fn decode(bytes: &[u8], image_limit: Option<u64>) -> Self {
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
            if let Some(image_limit) = image_limit {
                assert_eq!(
                    sys::ghostty_snapshot_decoder_set(
                        decoder,
                        sys::snapshot_opt::KITTY_IMAGE_STORAGE_LIMIT,
                        (&image_limit as *const u64).cast()
                    ),
                    sys::SUCCESS
                );
            }
            assert_eq!(
                sys::ghostty_snapshot_decoder_decode(decoder, &mut restored),
                sys::SUCCESS
            );
            sys::ghostty_snapshot_decoder_free(decoder);
        }
        Self(restored, None)
    }

    /// The Kitty image storage limit of the active screen of the restored terminal.
    fn image_limit(&self) -> u64 {
        let mut limit: u64 = u64::MAX;
        // SAFETY: the handle is live, and the key writes a `uint64_t`.
        let code = unsafe {
            sys::ghostty_terminal_get(
                self.0,
                sys::data::KITTY_IMAGE_STORAGE_LIMIT,
                (&mut limit as *mut u64).cast(),
            )
        };
        assert_eq!(code, sys::SUCCESS);
        limit
    }

    fn stores_image(&self, id: u32) -> bool {
        stores_image(self.0, id)
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
        if self.1.is_none() {
            unsafe { sys::ghostty_terminal_free(self.0) }
        }
    }
}

/// Whether the image storage of the active screen holds an image with this id, from the library.
fn stores_image(terminal: sys::Terminal, id: u32) -> bool {
    let mut graphics: sys::KittyGraphics = std::ptr::null_mut();
    // SAFETY: the handle is live, the key writes a `GhosttyKittyGraphics`, and the borrowed storage is read before the
    // next call that changes the terminal.
    unsafe {
        assert_eq!(
            sys::ghostty_terminal_get(
                terminal,
                sys::data::KITTY_GRAPHICS,
                (&mut graphics as *mut sys::KittyGraphics).cast()
            ),
            sys::SUCCESS,
            "the library is built with Kitty graphics"
        );
        !sys::ghostty_kitty_graphics_image(graphics, id).is_null()
    }
}

/// Restore `bytes` with the library's decoder and encode the result again.
fn restore_and_encode(bytes: &[u8]) -> Vec<u8> {
    Restored::from(bytes).snapshot()
}

#[test]
fn the_public_decoder_keeps_state_and_registered_callbacks() {
    let mut source = terminal();
    source.vt_write(
        b"\x1b[?1049h\x1b[?25l\x1b[?1h\x1b]2;title\x07\x1b]7;file:///tmp\x07\x1b[32mtext",
    );
    let snapshot = source.snapshot().unwrap();
    let mut restored = Terminal::from_snapshot(&snapshot, History::On, None).unwrap();
    assert_eq!(restored.snapshot().unwrap(), snapshot);
    assert_eq!(
        restored.screen_text(true).unwrap(),
        source.screen_text(true).unwrap()
    );
    assert_eq!(restored.modes(), source.modes());
    assert_eq!(restored.cursor(), source.cursor());
    assert_eq!(restored.title(), source.title());
    assert_eq!(restored.cwd(), source.cwd());
    source.drain_events();
    let suffix = b"\x1b]2;changed\x07\x1b]777;notify;t;b\x07\x07";
    source.vt_write(suffix);
    restored.vt_write(suffix);
    assert_eq!(restored.drain_events(), source.drain_events());
    assert_eq!(restored.snapshot().unwrap(), source.snapshot().unwrap());
}

#[test]
fn the_public_decoder_returns_typed_errors() {
    let mut bytes = terminal().snapshot().unwrap();
    let version = u16::try_from(snapshot_format().version)
        .unwrap()
        .wrapping_add(1);
    bytes[8..10].copy_from_slice(&version.to_le_bytes());
    assert!(matches!(Terminal::from_snapshot(&bytes, History::On, None),
        Err(SnapshotDecodeError::UnsupportedVersion { version: actual }) if actual == version));
    for bytes in [Vec::new(), bytes[..8].to_vec(), vec![0; bytes.len()]] {
        assert!(matches!(
            Terminal::from_snapshot(&bytes, History::On, None),
            Err(SnapshotDecodeError::Library(Error::InvalidValue))
        ));
    }
}

#[test]
fn continuation_and_processing_status_come_from_the_library() {
    let mut source = terminal();
    assert!(
        matches!(source.continuation().unwrap(), Continuation::Retained(bytes) if bytes.is_empty())
    );
    source.vt_write(b"\x1b]2;unfinished");
    let pending = source.continuation().unwrap();
    assert!(matches!(&pending, Continuation::Retained(bytes) if !bytes.is_empty()));
    let snapshot = source.snapshot().unwrap();
    let restored = Terminal::from_snapshot(&snapshot, History::On, None).unwrap();
    assert_eq!(restored.continuation().unwrap(), pending);
    assert!(!source.vt_processing_error().unwrap());
    source.set_continuation_max_bytes(0).unwrap();
    assert_eq!(source.continuation().unwrap(), Continuation::Unavailable);
    source.vt_write(b"\x07");
    source
        .set_continuation_max_bytes(CONTINUATION_LIMIT)
        .unwrap();
    assert!(
        matches!(source.continuation().unwrap(), Continuation::Retained(bytes) if bytes.is_empty())
    );
    source.set_continuation_max_bytes(8).unwrap();
    source.vt_write(b"\x1b]2;more-than-eight");
    assert_eq!(source.continuation().unwrap(), Continuation::Unavailable);
    assert!(
        !source.vt_processing_error().unwrap(),
        "a configured limit is not a processing error"
    );
}

#[test]
fn hyperlink_reads_match_the_library_before_and_after_restore() {
    let mut source = terminal();
    source.vt_write(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\ plain");
    let restored = Terminal::from_snapshot(&source.snapshot().unwrap(), History::On, None).unwrap();
    let uri = source.hyperlink_uri(0, 0).unwrap();
    assert!(!uri.is_empty());
    for col in 0..4 {
        assert_eq!(source.hyperlink_uri(0, col).unwrap(), uri);
        assert_eq!(restored.hyperlink_uri(0, col).unwrap(), uri);
    }
    assert!(source.hyperlink_uri(0, 5).unwrap().is_empty());
    for (row, col) in [(u32::MAX, 0), (0, u32::MAX), (0, u32::from(source.cols()))] {
        assert_eq!(source.hyperlink_uri(row, col), Err(Error::InvalidValue));
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

/// Kitty graphics input: image 1 in one command, then image 2 in two chunks, with text between. The session has image
/// storage off, so neither leaves any state, before or after a restore.
const IMAGES: &[u8] =
    b"top\x1b_Ga=T,f=24,s=1,v=1,i=1;AAAA\x1b\\mid\x1b_Ga=t,f=24,s=1,v=1,i=2,m=1;AAAA\x1b\\\x1b_Gm=0;\x1b\\end";

#[test]
fn public_graphics_reads_match_the_native_storage_with_and_without_images() {
    let mut source = terminal();
    let limit = 1024 * 1024u64;
    // SAFETY: the option takes a uint64_t and the terminal handle is live.
    unsafe {
        check(sys::ghostty_terminal_set(
            source.handle.as_ptr(),
            sys::opt::KITTY_IMAGE_STORAGE_LIMIT,
            (&limit as *const u64).cast(),
        ))
        .unwrap();
    }
    source.vt_write(IMAGES);
    assert!(stores_image(source.handle.as_ptr(), 1));
    let restored = Terminal::from_snapshot(&source.snapshot().unwrap(), History::On, None).unwrap();
    for model in [&source, &restored] {
        let mut native_limit = u64::MAX;
        // SAFETY: the data key writes a uint64_t into this live output pointer.
        unsafe {
            check(sys::ghostty_terminal_get(
                model.handle.as_ptr(),
                sys::data::KITTY_IMAGE_STORAGE_LIMIT,
                (&mut native_limit as *mut u64).cast(),
            ))
            .unwrap();
        }
        assert_eq!(model.image_storage_limit().unwrap(), native_limit);
        for id in [0, 1, 2, 3, u32::MAX] {
            assert_eq!(
                model.has_image(id).unwrap(),
                stores_image(model.handle.as_ptr(), id)
            );
        }
    }
    assert_ne!(
        source.image_storage_limit().unwrap(),
        restored.image_storage_limit().unwrap()
    );
}

#[test]
fn both_public_constructors_reject_image_storage_before_any_input() {
    let fresh = terminal();
    let restored = Terminal::from_snapshot(&fresh.snapshot().unwrap(), History::On, None).unwrap();
    for mut model in [fresh, restored] {
        assert_eq!(model.image_storage_limit().unwrap(), 0);
        model.vt_write(IMAGES);
        for id in [1, 2] {
            assert!(!model.has_image(id).unwrap());
            assert!(!stores_image(model.handle.as_ptr(), id));
        }
        model.vt_write(b"\x1b[?1049h");
        assert_eq!(model.image_storage_limit().unwrap(), 0);
        model.vt_write(IMAGES);
        for id in [1, 2] {
            assert!(!model.has_image(id).unwrap());
        }
    }
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
        IMAGES.to_vec(),
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

            // The restored terminal, not the original, takes the suffix. Its image storage limit is the session's
            // (zero) before and after the suffix, on both screens.
            let mut restored = Restored::from(&snapshot);
            assert_eq!(restored.image_limit(), 0, "corpus {index}, offset {offset}");
            restored.write(&input[offset..]);
            assert_eq!(restored.image_limit(), 0, "corpus {index}, offset {offset}");
            assert_eq!(
                restored.snapshot(),
                expected,
                "corpus {index}, offset {offset}"
            );
            // The alternate screen of the restored terminal has the same limit. This comes after the comparison, because
            // entering it changes the snapshot.
            restored.write(b"\x1b[?1049h");
            assert_eq!(
                restored.image_limit(),
                0,
                "corpus {index}, offset {offset} on the alternate screen"
            );
        }
    }
}

#[test]
fn a_restored_terminal_stores_no_image_at_any_byte_offset_where_a_default_restore_stores_one() {
    // The session's model stores neither image.
    let mut whole = terminal();
    whole.vt_write(IMAGES);
    assert!(!stores_image(whole.handle.as_ptr(), 1) && !stores_image(whole.handle.as_ptr(), 2));

    // The input of image 2 starts after this offset (the stimulus is `...mid` and then image 2).
    let image_2_start = IMAGES.windows(3).position(|bytes| bytes == b"mid").unwrap() + 3;
    let empty_session = terminal().snapshot().unwrap();

    for offset in 0..=IMAGES.len() {
        let mut before = terminal();
        before.vt_write(&IMAGES[..offset]);
        let snapshot = before
            .snapshot()
            .unwrap_or_else(|e| panic!("offset {offset}: {e:?}"));

        // The restore with the session's limit takes the rest of the input and stores no image, on either screen.
        let mut restored = Restored::from(&snapshot);
        restored.write(&IMAGES[offset..]);
        assert!(!restored.stores_image(1), "offset {offset}");
        assert!(!restored.stores_image(2), "offset {offset}");
        restored.write(b"\x1b[?1049h");
        restored.write(IMAGES);
        assert!(
            !restored.stores_image(1),
            "offset {offset}, alternate screen"
        );
        assert!(
            !restored.stores_image(2),
            "offset {offset}, alternate screen"
        );

        // The control: a restore at the library default limit stores the images that it completes after the restore.
        // The snapshot carries no image state, so an image that the session completed before the offset leaves
        // nothing. Where the library completes image 1 is the library's: a model at the default limit that takes only
        // the prefix has stored it exactly when the session completed it before the offset.
        let mut reference = Restored::with_the_library_default_image_limit(&empty_session);
        reference.write(&IMAGES[..offset]);
        let mut control = Restored::with_the_library_default_image_limit(&snapshot);
        control.write(&IMAGES[offset..]);
        assert_eq!(
            control.stores_image(1),
            !reference.stores_image(1),
            "offset {offset}"
        );
        if offset <= image_2_start {
            assert!(control.stores_image(2), "offset {offset}");
        }
        control.write(b"\x1b[?1049h");
        control.write(IMAGES);
        assert!(
            control.stores_image(1) && control.stores_image(2),
            "offset {offset}, alternate screen"
        );
    }
}

// ---- review finding P39 ----

#[test]
fn the_identity_is_the_first_name_of_a_whole_terminfo_entry() {
    let identity = terminal_identity();
    // The entry starts with its names, separated by `|`; `term` is the first one.
    let names = identity
        .terminfo_source
        .lines()
        .find(|line| !line.starts_with('#') && !line.trim().is_empty())
        .unwrap();
    assert_eq!(names.split('|').next().unwrap(), identity.term);
    // The entry lists capabilities, one per line after the names.
    assert!(identity.terminfo_source.lines().count() > 20);
}

#[test]
fn an_unfinished_sequence_of_half_the_continuation_limit_is_in_the_snapshot() {
    // `CONTINUATION_LIMIT` is 1 MiB (A8-2): half a mebibyte of an unfinished DCS still has a snapshot that restores.
    let mut terminal = terminal();
    let mut input = b"\x1bP".to_vec();
    input.resize(512 * 1024, b'a');
    terminal.vt_write(&input);
    let snapshot = terminal.snapshot().unwrap();
    assert_eq!(restore_and_encode(&snapshot), snapshot);
}
