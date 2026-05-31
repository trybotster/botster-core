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
