//! Shared one-slot adapter state for published test drivers.

use std::collections::VecDeque;

use botster_core::contract::terminal_adapter::{
    TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
    MIN_ADAPTER_INGRESS_BUFFER_FRAMES,
};
use botster_core::contract::terminal_wake::{TerminalWakeKind, TerminalWakeSink};
use botster_terminal_protocol::TerminalFrame;

#[derive(Debug, Default)]
pub(super) struct OneSlotCore {
    closed: bool,
    would_block: bool,
    active: Option<Vec<u8>>,
    delivered: Vec<Vec<u8>>,
    ingress: VecDeque<Vec<u8>>,
    ingress_partial: Option<Vec<u8>>,
    lost_pending: bool,
    wake_sink: Option<TerminalWakeSink>,
    closed_woke: bool,
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
        self.ingress.clear();
        self.ingress_partial = None;
        self.lost_pending = false;
        if !self.closed_woke {
            self.closed_woke = true;
            if let Some(sink) = &self.wake_sink {
                let _ = sink.wake(TerminalWakeKind::Closed);
            }
        }
    }

    pub(super) fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
        self.wake_sink = Some(sink);
    }

    fn emit_writable(&self) {
        if self.closed {
            return;
        }
        if let Some(sink) = &self.wake_sink {
            let _ = sink.wake(TerminalWakeKind::Writable);
        }
    }

    pub(super) fn try_read(&mut self) -> TerminalIngress {
        if self.closed {
            return TerminalIngress::Closed;
        }
        if self.lost_pending {
            self.lost_pending = false;
            return TerminalIngress::Lost;
        }
        match self.ingress.pop_front() {
            Some(frame) => TerminalIngress::Frame(frame),
            None => TerminalIngress::Empty,
        }
    }

    pub(super) fn inject_ingress_frame(&mut self, bytes: Vec<u8>) {
        if self.closed {
            return;
        }
        if self.ingress.len() >= MIN_ADAPTER_INGRESS_BUFFER_FRAMES {
            self.lost_pending = true;
            return;
        }
        self.ingress.push_back(bytes);
        self.emit_writable();
    }

    pub(super) fn inject_ingress_partial(&mut self, bytes: Vec<u8>) {
        if self.closed {
            return;
        }
        self.ingress_partial = Some(bytes);
    }

    pub(super) fn complete_ingress_partial(&mut self) {
        if let Some(bytes) = self.ingress_partial.take() {
            self.inject_ingress_frame(bytes);
        }
    }

    pub(super) fn drop_buffered_ingress_frame(&mut self) {
        if self.closed {
            return;
        }
        if self.ingress.pop_back().is_some() {
            self.lost_pending = true;
            self.emit_writable();
        }
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
        self.emit_writable();
    }

    pub(super) fn is_closed(&self) -> bool {
        self.closed
    }

    pub(super) fn take_active(&mut self) -> Option<Vec<u8>> {
        let taken = self.active.take();
        if taken.is_some() {
            self.emit_writable();
        }
        taken
    }

    pub(super) fn push_delivered(&mut self, bytes: Vec<u8>) {
        self.delivered.push(bytes);
    }

    pub(super) fn delivered(&self) -> &[Vec<u8>] {
        &self.delivered
    }
}

impl Drop for OneSlotCore {
    fn drop(&mut self) {
        self.close();
    }
}
