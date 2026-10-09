//! The bounded log ring (Core SV-9).

use std::collections::VecDeque;

/// The newest `capacity` bytes of the service's stdout and stderr, positioned in the whole output stream.
#[derive(Debug)]
pub(crate) struct LogRing {
    bytes: VecDeque<u8>,
    capacity: usize,
    /// The number of bytes the service wrote in total.
    end: u64,
}

impl LogRing {
    pub(crate) fn new(capacity: usize) -> Self {
        LogRing {
            bytes: VecDeque::new(),
            capacity,
            end: 0,
        }
    }

    pub(crate) fn push(&mut self, bytes: &[u8]) {
        self.end += bytes.len() as u64;
        let kept = &bytes[bytes.len().saturating_sub(self.capacity)..];
        let evicted = (self.bytes.len() + kept.len()).saturating_sub(self.capacity);
        self.bytes.drain(..evicted);
        self.bytes.extend(kept);
    }

    pub(crate) fn end(&self) -> u64 {
        self.end
    }

    /// The retained bytes at and after `offset` (at most [`LogRing::end`]), with the offset of the first one. Bytes older
    /// than the ring are gone, so an older `offset` starts at the oldest retained byte.
    pub(crate) fn since(&self, offset: u64) -> (u64, Vec<u8>) {
        let start = self.end - self.bytes.len() as u64;
        let from = offset.max(start);
        let skip = usize::try_from(from - start).expect("the skip is within the ring");
        (from, self.bytes.iter().skip(skip).copied().collect())
    }
}
