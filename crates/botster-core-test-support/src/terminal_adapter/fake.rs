//! In-memory one-slot sink used as the baseline published driver.

use std::sync::{Arc, Mutex};

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError,
};
use botster_terminal_protocol::TerminalFrame;

use super::core::OneSlotCore;
use super::TerminalAdapterHarnessDriver;

/// In-memory one-slot terminal adapter.
///
/// This is a Core-owned test adapter. It is not a production transport.
#[derive(Debug, Default)]
pub struct FakeTerminalAdapter {
    inner: OneSlotCore,
}

impl TerminalAdapter for FakeTerminalAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        self.inner.try_write(frame)
    }

    fn close(&mut self) {
        self.inner.close();
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        self.inner.pressure()
    }
}

impl TerminalAdapterHarnessDriver for FakeTerminalAdapter {
    type Adapter = Self;

    fn adapter(&mut self) -> &mut Self::Adapter {
        self
    }

    fn force_would_block(&mut self) {
        self.inner.force_would_block();
    }

    fn clear_would_block(&mut self) {
        self.inner.clear_would_block();
    }

    fn complete_active_write(&mut self) {
        if self.inner.is_closed() {
            return;
        }
        if let Some(bytes) = self.inner.take_active() {
            self.inner.push_delivered(bytes);
        }
    }

    fn force_closed(&mut self) {
        self.inner.close();
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        self.inner.delivered()
    }
}

/// Shared Fake adapter handle so tests can complete writes after bind.
#[derive(Clone, Debug, Default)]
pub struct SharedFakeTerminalAdapter {
    inner: Arc<Mutex<FakeTerminalAdapter>>,
    auto_complete: bool,
}

impl SharedFakeTerminalAdapter {
    /// Build a shared Fake that occupies the one-slot write until completed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a shared Fake that completes each accepted write immediately.
    #[must_use]
    pub fn auto_complete() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FakeTerminalAdapter::default())),
            auto_complete: true,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeTerminalAdapter> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}

impl TerminalAdapter for SharedFakeTerminalAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        let result = self.lock().try_write(frame);
        if result.is_ok() && self.auto_complete {
            self.complete_active_write();
        }
        result
    }

    fn close(&mut self) {
        self.lock().close();
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        self.lock().pressure()
    }
}

impl TerminalAdapterHarnessDriver for SharedFakeTerminalAdapter {
    type Adapter = Self;

    fn adapter(&mut self) -> &mut Self::Adapter {
        self
    }

    fn force_would_block(&mut self) {
        self.lock().force_would_block();
    }

    fn clear_would_block(&mut self) {
        self.lock().clear_would_block();
    }

    fn complete_active_write(&mut self) {
        self.lock().complete_active_write();
    }

    fn force_closed(&mut self) {
        self.lock().force_closed();
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        // The driver trait returns a borrow. Shared observation uses
        // [`Self::snapshot_delivered_frame_bytes`].
        &[]
    }
}

impl SharedFakeTerminalAdapter {
    /// Copy of completed frame bytes after bind.
    #[must_use]
    pub fn snapshot_delivered_frame_bytes(&self) -> Vec<Vec<u8>> {
        self.lock().delivered_frame_bytes().to_vec()
    }

    /// Current adapter pressure after bind.
    #[must_use]
    pub fn snapshot_pressure(&self) -> TerminalAdapterPressure {
        self.lock().pressure()
    }

    /// Complete the active write after bind without needing `&mut self`.
    pub fn complete_write(&self) {
        self.lock().complete_active_write();
    }

    /// Force transport-side close after bind without needing `&mut self`.
    pub fn close_transport(&self) {
        self.lock().force_closed();
    }

    /// Force would-block after bind without needing `&mut self`.
    pub fn block_writes(&self) {
        self.lock().force_would_block();
    }
}
