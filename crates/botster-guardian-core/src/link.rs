//! The guardian side of the control link: hello, authentication and written-byte accounting (plan 3; Core AD-6).

use crate::guardian::GuardianConfig;
use botster_core_link::frame::{
    encode_frame, Frame, FrameDecoder, FrameError, FrameType, DEFAULT_MAX_PAYLOAD,
};
use botster_core_link::hello::{Hello, HelloError};
use botster_core_link::proof::token_proof;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkState {
    AwaitHello,
    Ready,
    Closed,
}

/// One connection at a time. A new connection starts unauthenticated with fresh byte counts.
#[derive(Debug)]
pub(crate) struct Link {
    pub(crate) state: LinkState,
    decoder: FrameDecoder,
    /// The highest host epoch that a valid hello proved (DP-8).
    epoch: u64,
    /// Bytes that this connection's `LinkSend` actions carry, and bytes the driver reports written.
    queued: u64,
    written: u64,
}

impl Link {
    pub(crate) fn new(epoch: u64) -> Self {
        Link {
            state: LinkState::AwaitHello,
            decoder: FrameDecoder::new(DEFAULT_MAX_PAYLOAD),
            epoch,
            queued: 0,
            written: 0,
        }
    }

    /// The guardian's hello for the current epoch.
    pub(crate) fn hello(&self, who: &GuardianConfig) -> Result<Vec<u8>, HelloError> {
        let hello = Hello {
            protocol: who.protocol,
            instance: who.instance.clone(),
            proof: token_proof(&who.token, &who.instance, self.epoch),
            host_epoch: self.epoch,
        };
        let mut payload = Vec::new();
        hello.encode(&mut payload)?;
        Ok(frame(FrameType::HELLO, &payload))
    }

    /// A hello is valid with the guardian's protocol and instance, an epoch no lower than the last proved one, and the
    /// token's proof for that epoch (AD-6, DP-8).
    pub(crate) fn authenticate(&mut self, who: &GuardianConfig, frame: &Frame) -> bool {
        let Some(hello) = (frame.kind == FrameType::HELLO)
            .then(|| Hello::decode(&frame.payload).ok())
            .flatten()
        else {
            return false;
        };
        let valid = hello.protocol == who.protocol
            && hello.instance == who.instance
            && hello.host_epoch >= self.epoch
            && hello.proof == token_proof(&who.token, &who.instance, hello.host_epoch);
        if valid {
            self.epoch = hello.host_epoch;
            self.state = LinkState::Ready;
        }
        valid
    }

    /// Feeds bytes until one frame is complete. Returns the bytes it consumed and that frame.
    pub(crate) fn read(&mut self, bytes: &[u8]) -> (usize, Result<Option<Frame>, FrameError>) {
        let took = self.decoder.push(bytes);
        (took, self.decoder.next_frame())
    }

    /// Counts an outgoing frame. The caller emits it.
    pub(crate) fn queue(&mut self, bytes: &[u8]) {
        self.queued += bytes.len() as u64;
    }

    pub(crate) fn queued(&self) -> u64 {
        self.queued
    }

    pub(crate) fn wrote(&mut self, total: u64) {
        self.written = self.written.max(total);
    }

    pub(crate) fn written(&self) -> u64 {
        self.written
    }

    /// A new transport replaces a closed one.
    pub(crate) fn reconnect(&mut self) {
        *self = Link::new(self.epoch);
    }
}

/// Every guardian frame fits the link bound: reports are small, and log chunks are cut to fit.
pub(crate) fn frame(kind: FrameType, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_frame(kind, payload, DEFAULT_MAX_PAYLOAD, &mut bytes)
        .expect("guardian frames fit the link bound");
    bytes
}
