//! Architecture contract tests for the Ghostty terminal adapter boundary.

use botster_core::contract::terminal_screen::{
    TerminalOutputChunk, TerminalScreenSize, TerminalScreenState, TerminalSnapshotPayload,
};
use botster_core::engine::TerminalScreenRuntime;
use botster_terminal_ghostty::{
    GhosttyAdapterConfig, GhosttyTerminalRuntime, GHOSTTY_SNAPSHOT_FORMAT,
};

#[derive(Debug, Clone)]
struct FakeGhosttyRuntime {
    size: TerminalScreenSize,
    bytes: Vec<u8>,
}

impl Default for FakeGhosttyRuntime {
    fn default() -> Self {
        Self {
            size: TerminalScreenSize::new(24, 80),
            bytes: Vec::new(),
        }
    }
}

impl TerminalScreenRuntime for FakeGhosttyRuntime {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        self.bytes.extend_from_slice(bytes);
        TerminalOutputChunk::new(bytes.to_vec())
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        self.size = size;
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        TerminalSnapshotPayload::new(
            self.bytes.clone(),
            self.size,
            Some(GhosttyAdapterConfig::default().snapshot_format().to_owned()),
        )
    }

    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
        self.bytes = payload.bytes;
        self.size = payload.size;
    }

    fn screen_state(&self) -> TerminalScreenState {
        TerminalScreenState::new(self.size, String::from_utf8_lossy(&self.bytes).into_owned())
    }
}

fn accepts_ghostty_runtime<R: GhosttyTerminalRuntime>(runtime: &mut R) -> TerminalSnapshotPayload {
    runtime.write_output(b"hello");
    runtime.capture_snapshot()
}

#[test]
fn ghostty_marker_uses_existing_terminal_screen_runtime_seam() {
    let mut runtime = FakeGhosttyRuntime::default();

    let snapshot = accepts_ghostty_runtime(&mut runtime);

    assert_eq!(snapshot.bytes, b"hello");
    assert_eq!(snapshot.size, TerminalScreenSize::new(24, 80));
    assert_eq!(snapshot.format.as_deref(), Some(GHOSTTY_SNAPSHOT_FORMAT));
}

#[test]
fn ghostty_adapter_config_names_the_reserved_snapshot_format() {
    let config = GhosttyAdapterConfig::default();

    assert_eq!(config.snapshot_format(), GHOSTTY_SNAPSHOT_FORMAT);
}
