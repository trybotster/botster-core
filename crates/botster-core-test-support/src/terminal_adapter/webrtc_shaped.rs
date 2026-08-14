//! In-memory one-slot adapter that may split one write into chunks.

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError,
};
use botster_terminal_protocol::TerminalFrame;

use super::core::OneSlotCore;
use super::TerminalAdapterHarnessDriver;

/// Fixed test chunk size. Production Hub chunking is not owned here.
const TEST_CHUNK_SIZE: usize = 8;

/// WebRTC-shaped in-memory adapter.
///
/// Completing an accepted write may split the serialized frame into chunks and
/// reassemble them into one delivered blob. Chunks of frame N never interleave
/// with frame N+1 because only one write is active. This is not a DataChannel,
/// DTLS, SCTP, or Hub crypto path.
#[derive(Debug, Default)]
pub struct WebRtcShapedTerminalAdapter {
    inner: OneSlotCore,
}

impl TerminalAdapter for WebRtcShapedTerminalAdapter {
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

impl TerminalAdapterHarnessDriver for WebRtcShapedTerminalAdapter {
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
            let mut reassembled = Vec::with_capacity(bytes.len());
            for chunk in bytes.chunks(TEST_CHUNK_SIZE) {
                reassembled.extend_from_slice(chunk);
            }
            self.inner.push_delivered(reassembled);
        }
    }

    fn force_closed(&mut self) {
        self.inner.close();
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        self.inner.delivered()
    }
}
