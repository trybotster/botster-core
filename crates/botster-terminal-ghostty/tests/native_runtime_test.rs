//! Feature-gated runtime tests for the safe libghostty-vt adapter.

#![cfg(feature = "libghostty-vt")]

use botster_core::contract::terminal_screen::{TerminalScreenSize, TerminalSnapshotPayload};
use botster_core::engine::TerminalScreenRuntime;
use botster_terminal_ghostty::{GhosttyTerminal, GhosttyTerminalRuntime, GHOSTTY_SNAPSHOT_FORMAT};

fn runtime() -> GhosttyTerminal {
    GhosttyTerminal::new(TerminalScreenSize::new(24, 80)).expect("create Ghostty terminal")
}

fn accepts_runtime<R: GhosttyTerminalRuntime>(runtime: &mut R) -> TerminalSnapshotPayload {
    runtime.write_output(b"runtime path");
    runtime.capture_snapshot()
}

#[test]
fn safe_wrapper_constructs_drops_and_implements_runtime_marker() {
    let mut runtime = runtime();

    let snapshot = accepts_runtime(&mut runtime);

    assert_eq!(snapshot.size, TerminalScreenSize::new(24, 80));
    assert_eq!(snapshot.format.as_deref(), Some(GHOSTTY_SNAPSHOT_FORMAT));
}

#[test]
fn write_output_preserves_bytes_and_updates_plain_screen() {
    let mut runtime = runtime();

    let output = runtime.write_output(b"hello from botster");
    let screen = runtime.screen_state();

    assert_eq!(output.bytes, b"hello from botster");
    assert!(screen.plain_text.contains("hello from botster"));
    assert_eq!(runtime.last_error(), None);
}

#[test]
fn resize_maps_terminal_screen_size_to_ghostty() {
    let mut runtime = runtime();

    runtime.resize(TerminalScreenSize::new(12, 5));
    runtime.write_output(b"abcdef");
    let snapshot = runtime.capture_snapshot();
    let screen = runtime.screen_state();
    let lines: Vec<_> = screen.plain_text.lines().collect();

    assert_eq!(runtime.size(), TerminalScreenSize::new(12, 5));
    assert_eq!(snapshot.size, TerminalScreenSize::new(12, 5));
    assert_eq!(lines.first(), Some(&"abcde"));
    assert_eq!(lines.get(1), Some(&"f"));
    assert_eq!(runtime.last_error(), None);
}

#[test]
fn snapshot_round_trips_through_opaque_payload() {
    let mut source = runtime();
    source.write_output(b"snapshot text");
    let snapshot = source
        .export_snapshot()
        .expect("export complete Ghostty snapshot");

    let mut restored = runtime();
    restored
        .import_snapshot(&snapshot)
        .expect("import Ghostty snapshot");

    assert!(restored.screen_state().plain_text.contains("snapshot text"));
    assert_eq!(restored.last_error(), None);
}

#[test]
fn fallible_import_records_error_without_panicking_trait_replay() {
    let mut runtime = runtime();
    let invalid = TerminalSnapshotPayload::new(
        b"not a ghostty snapshot".to_vec(),
        TerminalScreenSize::new(24, 80),
        Some(GHOSTTY_SNAPSHOT_FORMAT.to_owned()),
    );

    assert!(runtime.import_snapshot(&invalid).is_err());
    runtime.replay_snapshot(invalid);

    assert!(runtime.last_error().is_some());
}

#[test]
fn successful_operations_clear_prior_last_error() {
    let mut runtime = runtime();
    let invalid = TerminalSnapshotPayload::new(
        b"not a ghostty snapshot".to_vec(),
        TerminalScreenSize::new(24, 80),
        Some(GHOSTTY_SNAPSHOT_FORMAT.to_owned()),
    );

    runtime.replay_snapshot(invalid.clone());
    assert!(runtime.last_error().is_some());

    runtime.write_output(b"ok");
    assert_eq!(runtime.last_error(), None);

    runtime.replay_snapshot(invalid);
    assert!(runtime.last_error().is_some());

    assert!(runtime.screen_state().plain_text.contains("ok"));
    assert_eq!(runtime.last_error(), None);
}
