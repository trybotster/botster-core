//! In-memory one-slot sink used as the baseline published driver.

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
