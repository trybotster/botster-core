//! Ordered in-memory byte pipe with one in-flight write.

use std::collections::VecDeque;

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
};
use botster_terminal_protocol::TerminalFrame;

use super::core::OneSlotCore;
use super::TerminalAdapterHarnessDriver;

/// Unix-shaped in-memory adapter.
///
/// Completing an accepted write copies those bytes through an ordered pipe.
/// This is not a real `UnixStream`, listener, or host-auth path.
#[derive(Debug, Default)]
pub struct UnixShapedTerminalAdapter {
    inner: OneSlotCore,
    pipe: VecDeque<u8>,
}

impl TerminalAdapter for UnixShapedTerminalAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        self.inner.try_write(frame)
    }

    fn close(&mut self) {
        self.inner.close();
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        self.inner.pressure()
    }

    fn try_read(&mut self) -> TerminalIngress {
        self.inner.try_read()
    }
}

impl TerminalAdapterHarnessDriver for UnixShapedTerminalAdapter {
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
            self.pipe.extend(bytes.iter().copied());
            let delivered: Vec<u8> = self.pipe.drain(..bytes.len()).collect();
            self.inner.push_delivered(delivered);
        }
    }

    fn force_closed(&mut self) {
        self.inner.close();
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        self.inner.delivered()
    }

    fn inject_ingress_frame(&mut self, bytes: Vec<u8>) {
        self.inner.inject_ingress_frame(bytes);
    }

    fn inject_ingress_partial(&mut self, bytes: Vec<u8>) {
        self.inner.inject_ingress_partial(bytes);
    }

    fn complete_ingress_partial(&mut self) {
        self.inner.complete_ingress_partial();
    }

    fn drop_buffered_ingress_frame(&mut self) {
        self.inner.drop_buffered_ingress_frame();
    }
}
