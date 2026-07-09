//! Terminal screen engine over a host-owned runtime.

use crate::contract::terminal_screen::{
    TerminalOutputChunk, TerminalScreenHook, TerminalScreenSize, TerminalScreenState,
    TerminalSnapshotPayload,
};

/// Runtime operations used by the reusable terminal screen engine.
pub trait TerminalScreenRuntime {
    /// Accept terminal output bytes and return the preserved normalized chunk.
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk;

    /// Resize the runtime screen.
    fn resize(&mut self, size: TerminalScreenSize);

    /// Capture an opaque snapshot.
    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload;

    /// Replay an opaque snapshot.
    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload);

    /// Read current screen state.
    fn screen_state(&self) -> TerminalScreenState;

    /// Return the most recent backend operation error, if the runtime records one.
    fn last_error(&self) -> Option<String> {
        None
    }
}

impl TerminalScreenRuntime for Box<dyn TerminalScreenRuntime> {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        self.as_mut().write_output(bytes)
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        self.as_mut().resize(size);
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        self.as_mut().capture_snapshot()
    }

    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
        self.as_mut().replay_snapshot(payload);
    }

    fn screen_state(&self) -> TerminalScreenState {
        self.as_ref().screen_state()
    }

    fn last_error(&self) -> Option<String> {
        self.as_ref().last_error()
    }
}

/// Maximum retained bytes for the plain fallback terminal state.
///
/// Live output chunks are returned unchanged; this cap only constrains the
/// retained tail used by plain snapshots and screen reads.
pub const PLAIN_TERMINAL_SCREEN_MAX_BYTES: usize = 1024 * 1024;

/// Minimal bounded terminal state runtime for core-managed session snapshots.
#[derive(Debug, Clone)]
pub struct PlainTerminalScreenRuntime {
    size: TerminalScreenSize,
    bytes: Vec<u8>,
    plain_text: String,
    format: Option<String>,
}

impl Default for PlainTerminalScreenRuntime {
    fn default() -> Self {
        Self {
            size: TerminalScreenSize::new(24, 80),
            bytes: Vec::new(),
            plain_text: String::new(),
            format: Some("plain-opaque-v1".to_string()),
        }
    }
}

impl PlainTerminalScreenRuntime {
    /// Build an empty plain terminal state runtime.
    #[must_use]
    pub fn new(size: TerminalScreenSize) -> Self {
        Self {
            size,
            bytes: Vec::new(),
            plain_text: String::new(),
            format: Some("plain-opaque-v1".to_string()),
        }
    }

    fn refresh_plain_text(&mut self) {
        self.plain_text = String::from_utf8_lossy(&self.bytes).into_owned();
    }

    fn retain_bounded_tail(&mut self) {
        if self.bytes.len() > PLAIN_TERMINAL_SCREEN_MAX_BYTES {
            let excess = self.bytes.len() - PLAIN_TERMINAL_SCREEN_MAX_BYTES;
            self.bytes.drain(..excess);
        }
    }
}

impl TerminalScreenRuntime for PlainTerminalScreenRuntime {
    fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
        self.bytes.extend_from_slice(bytes);
        self.retain_bounded_tail();
        self.refresh_plain_text();
        TerminalOutputChunk::new(bytes.to_vec())
    }

    fn resize(&mut self, size: TerminalScreenSize) {
        self.size = size;
    }

    fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
        TerminalSnapshotPayload::new(self.bytes.clone(), self.size, self.format.clone())
    }

    fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
        self.bytes = payload.bytes;
        self.retain_bounded_tail();
        self.size = payload.size;
        self.format = payload.format;
        self.refresh_plain_text();
    }

    fn screen_state(&self) -> TerminalScreenState {
        TerminalScreenState::new(self.size, self.plain_text.clone())
    }
}

/// Outcome produced by one terminal screen engine operation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalScreenOutcome {
    /// Lifecycle hooks emitted by the operation.
    pub hooks: Vec<TerminalScreenHook>,
    /// Normalized output chunk, when output was written.
    pub output: Option<TerminalOutputChunk>,
    /// Snapshot payload, when a snapshot was captured.
    pub snapshot: Option<TerminalSnapshotPayload>,
    /// Screen state, when state was read.
    pub screen: Option<TerminalScreenState>,
}

/// Reusable terminal screen engine with a host-supplied runtime.
#[derive(Debug, Clone)]
pub struct TerminalScreenEngine<R> {
    runtime: R,
}

impl<R> TerminalScreenEngine<R>
where
    R: TerminalScreenRuntime,
{
    /// Build a terminal screen engine.
    pub fn new(runtime: R) -> Self {
        Self { runtime }
    }

    /// Return an immutable view of the runtime adapter.
    pub const fn runtime(&self) -> &R {
        &self.runtime
    }

    /// Return a mutable view of the runtime adapter.
    pub fn runtime_mut(&mut self) -> &mut R {
        &mut self.runtime
    }

    /// Normalize output bytes through the runtime.
    pub fn normalize_output(&mut self, bytes: &[u8]) -> TerminalScreenOutcome {
        let output = self.runtime.write_output(bytes);
        let output_len = output.bytes.len();

        TerminalScreenOutcome {
            hooks: vec![TerminalScreenHook::OutputNormalized { bytes: output_len }],
            output: Some(output),
            snapshot: None,
            screen: None,
        }
    }

    /// Resize the runtime screen.
    pub fn resize(&mut self, size: TerminalScreenSize) -> TerminalScreenOutcome {
        self.runtime.resize(size);

        TerminalScreenOutcome {
            hooks: vec![TerminalScreenHook::Resized { size }],
            output: None,
            snapshot: None,
            screen: None,
        }
    }

    /// Capture an opaque snapshot through the runtime.
    pub fn capture_snapshot(&mut self) -> TerminalScreenOutcome {
        let snapshot = self.runtime.capture_snapshot();
        let size = snapshot.size;

        TerminalScreenOutcome {
            hooks: vec![TerminalScreenHook::SnapshotCaptured { size }],
            output: None,
            snapshot: Some(snapshot),
            screen: None,
        }
    }

    /// Replay an opaque snapshot through the runtime.
    pub fn replay_snapshot(&mut self, snapshot: TerminalSnapshotPayload) -> TerminalScreenOutcome {
        let size = snapshot.size;
        self.runtime.replay_snapshot(snapshot);

        TerminalScreenOutcome {
            hooks: vec![TerminalScreenHook::SnapshotReplayed { size }],
            output: None,
            snapshot: None,
            screen: None,
        }
    }

    /// Read current screen state through the runtime.
    pub fn screen_state(&self) -> TerminalScreenOutcome {
        let screen = self.runtime.screen_state();
        let size = screen.size;

        TerminalScreenOutcome {
            hooks: vec![TerminalScreenHook::ScreenRead { size }],
            output: None,
            snapshot: None,
            screen: Some(screen),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PlainTerminalScreenRuntime, TerminalScreenEngine, TerminalScreenSize,
        PLAIN_TERMINAL_SCREEN_MAX_BYTES,
    };

    #[test]
    fn plain_terminal_snapshot_retains_bounded_tail_with_format_label() {
        let mut engine = TerminalScreenEngine::new(PlainTerminalScreenRuntime::new(
            TerminalScreenSize::new(24, 80),
        ));
        let excess = b"discarded-prefix";
        let retained = vec![b'x'; PLAIN_TERMINAL_SCREEN_MAX_BYTES];
        let mut output = excess.to_vec();
        output.extend_from_slice(&retained);

        engine.normalize_output(&output);
        let snapshot = engine
            .capture_snapshot()
            .snapshot
            .expect("plain runtime should capture a snapshot");

        assert_eq!(snapshot.bytes.len(), PLAIN_TERMINAL_SCREEN_MAX_BYTES);
        assert_eq!(snapshot.bytes, retained);
        assert_eq!(snapshot.format.as_deref(), Some("plain-opaque-v1"));
    }
}
