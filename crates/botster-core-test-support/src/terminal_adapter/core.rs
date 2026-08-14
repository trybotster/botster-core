//! Shared one-slot adapter state for published test drivers.

use botster_core::contract::terminal_adapter::{
    TerminalAdapterPressure, TerminalAdapterWriteError,
};
use botster_terminal_protocol::TerminalFrame;

#[derive(Debug, Default)]
pub(super) struct OneSlotCore {
    closed: bool,
    would_block: bool,
    active: Option<Vec<u8>>,
    delivered: Vec<Vec<u8>>,
}

impl OneSlotCore {
    pub(super) fn try_write(
        &mut self,
        frame: &TerminalFrame,
    ) -> Result<(), TerminalAdapterWriteError> {
        if self.closed {
            return Err(TerminalAdapterWriteError::Closed);
        }
        if self.active.is_some() {
            return Err(TerminalAdapterWriteError::Full);
        }
        if self.would_block {
            return Err(TerminalAdapterWriteError::WouldBlock);
        }
        let bytes = frame.to_bytes().expect("fixture TerminalFrame serializes");
        self.active = Some(bytes);
        Ok(())
    }

    pub(super) fn close(&mut self) {
        self.closed = true;
        self.active = None;
    }

    pub(super) fn pressure(&self) -> TerminalAdapterPressure {
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

    pub(super) fn force_would_block(&mut self) {
        self.would_block = true;
    }

    pub(super) fn clear_would_block(&mut self) {
        self.would_block = false;
    }

    pub(super) fn is_closed(&self) -> bool {
        self.closed
    }

    pub(super) fn take_active(&mut self) -> Option<Vec<u8>> {
        self.active.take()
    }

    pub(super) fn push_delivered(&mut self, bytes: Vec<u8>) {
        self.delivered.push(bytes);
    }

    pub(super) fn delivered(&self) -> &[Vec<u8>] {
        &self.delivered
    }
}
