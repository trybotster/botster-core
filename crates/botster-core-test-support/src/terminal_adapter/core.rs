//! Shared one-slot adapter state for published test drivers.

use std::collections::VecDeque;

use botster_core::contract::terminal_adapter::{
    TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
    MIN_ADAPTER_INGRESS_BUFFER_FRAMES,
};
use botster_core::contract::terminal_wake::{TerminalWakeKind, TerminalWakeSink};
use botster_terminal_protocol::RoutedTerminalFrame;

/// One completed delivery with the routing carried by its container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredFrame {
    /// Route id the frame was addressed to.
    pub route: String,
    /// Attachment generation stamped on the container.
    pub generation: u64,
    /// Stream epoch stamped on the container.
    pub stream_epoch: u32,
    /// The scheme 2 `TerminalBody` bytes.
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub(super) struct OneSlotCore {
    closed: bool,
    would_block: bool,
    active: Option<Vec<u8>>,
    active_routing: Option<(String, u64, u32)>,
    delivered: Vec<Vec<u8>>,
    delivered_frames: Vec<DeliveredFrame>,
    ingress: VecDeque<Vec<u8>>,
    ingress_partial: Option<Vec<u8>>,
    lost_pending: bool,
    wake_sink: Option<TerminalWakeSink>,
    closed_woke: bool,
    reads: usize,
    writes: usize,
}

impl OneSlotCore {
    pub(super) fn try_write(
        &mut self,
        frame: &RoutedTerminalFrame,
    ) -> Result<(), TerminalAdapterWriteError> {
        self.writes += 1;
        if self.closed {
            return Err(TerminalAdapterWriteError::Closed);
        }
        if self.active.is_some() {
            return Err(TerminalAdapterWriteError::Full);
        }
        if self.would_block {
            return Err(TerminalAdapterWriteError::WouldBlock);
        }
        // Content-blind: copy only the shared TerminalBody bytes for delivery.
        self.active = Some(frame.frame.as_bytes().to_vec());
        self.active_routing = Some((
            frame.route.as_str().to_string(),
            frame.generation,
            frame.stream_epoch,
        ));
        Ok(())
    }

    pub(super) fn close(&mut self) {
        self.closed = true;
        self.active = None;
        self.active_routing = None;
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
        self.reads += 1;
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
        if let Some((route, generation, stream_epoch)) = self.active_routing.take() {
            self.delivered_frames.push(DeliveredFrame {
                route,
                generation,
                stream_epoch,
                bytes: bytes.clone(),
            });
        }
        self.delivered.push(bytes);
    }

    pub(super) fn delivered(&self) -> &[Vec<u8>] {
        &self.delivered
    }

    pub(super) fn delivered_frames(&self) -> &[DeliveredFrame] {
        &self.delivered_frames
    }

    pub(super) fn read_count(&self) -> usize {
        self.reads
    }

    pub(super) fn write_count(&self) -> usize {
        self.writes
    }

    pub(super) fn wake(&self, kind: TerminalWakeKind) -> bool {
        self.wake_sink.as_ref().is_some_and(|sink| sink.wake(kind))
    }
}

impl Drop for OneSlotCore {
    fn drop(&mut self) {
        self.close();
    }
}
