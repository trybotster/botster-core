//! Isolated Hub-shaped consumer that implements the published adapter contract.

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError,
};
use botster_core_test_support::terminal_adapter::{
    assert_terminal_adapter_conformance, TerminalAdapterHarnessDriver,
};
use botster_terminal_protocol::TerminalFrame;

/// Minimal external adapter. Not a published Core driver.
#[derive(Default)]
pub struct HubShapedTerminalAdapter {
    closed: bool,
    would_block: bool,
    active: Option<Vec<u8>>,
    delivered: Vec<Vec<u8>>,
}

impl TerminalAdapter for HubShapedTerminalAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        if self.closed {
            return Err(TerminalAdapterWriteError::Closed);
        }
        if self.active.is_some() {
            return Err(TerminalAdapterWriteError::Full);
        }
        if self.would_block {
            return Err(TerminalAdapterWriteError::WouldBlock);
        }
        self.active = Some(frame.to_bytes().expect("serialize accepted frame"));
        Ok(())
    }

    fn close(&mut self) {
        self.closed = true;
        self.active = None;
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        if self.closed {
            TerminalAdapterPressure::Closed
        } else if self.active.is_some() {
            TerminalAdapterPressure::Full
        } else if self.would_block {
            TerminalAdapterPressure::WouldBlock
        } else {
            TerminalAdapterPressure::Ready
        }
    }
}

impl TerminalAdapterHarnessDriver for HubShapedTerminalAdapter {
    type Adapter = Self;

    fn adapter(&mut self) -> &mut Self::Adapter {
        self
    }

    fn force_would_block(&mut self) {
        self.would_block = true;
    }

    fn clear_would_block(&mut self) {
        self.would_block = false;
    }

    fn complete_active_write(&mut self) {
        if self.closed {
            return;
        }
        if let Some(bytes) = self.active.take() {
            self.delivered.push(bytes);
        }
    }

    fn force_closed(&mut self) {
        self.closed = true;
        self.active = None;
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        &self.delivered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_shaped_consumer_adapter_passes_published_harness() {
        let mut driver = HubShapedTerminalAdapter::default();
        assert_terminal_adapter_conformance(&mut driver);
    }
}
