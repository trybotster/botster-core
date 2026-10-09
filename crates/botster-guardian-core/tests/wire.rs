//! The guardian's log chunks on the wire (plan 3; Core SV-9).

use botster_core_link::frame::{encode_frame, FrameDecoder, DEFAULT_MAX_PAYLOAD};
use botster_guardian_core::wire::{log_chunks, LogChunk, LOG_CHUNK_BYTES, LOG_FRAME};
use std::num::NonZeroUsize;

/// Core SV-9: no clause fixes the chunk size. At the guardian's size, at the size that the `/` mutant of
/// `LOG_CHUNK_BYTES` gives (`.cargo/mutants.toml`), and at the minimum, a tail splits into link frames that carry every
/// byte in order with contiguous offsets.
#[test]
fn log_chunks_carry_every_byte_at_each_chunk_size() {
    let offset = 1000;
    for max in [
        LOG_CHUNK_BYTES,
        NonZeroUsize::new(DEFAULT_MAX_PAYLOAD as usize / 8).unwrap(),
        NonZeroUsize::MIN,
    ] {
        let tail: Vec<u8> = (0..max.get() * 2 + 1).map(|i| i as u8).collect();
        let mut decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
        let mut received = Vec::new();
        for chunk in log_chunks(offset, &tail, max) {
            assert!((1..=max.get()).contains(&chunk.bytes.len()));
            assert_eq!(chunk.offset, offset + received.len() as u64);
            let mut frame = Vec::new();
            encode_frame(LOG_FRAME, &chunk.encode(), DEFAULT_MAX_PAYLOAD, &mut frame).unwrap();
            assert_eq!(decoder.push(&frame), frame.len());
            let decoded = decoder.next_frame().unwrap().unwrap();
            assert_eq!(decoded.kind, LOG_FRAME);
            assert_eq!(LogChunk::decode(&decoded.payload), Some(chunk.clone()));
            received.extend_from_slice(&chunk.bytes);
        }
        assert_eq!(received, tail, "{max}");
    }
}
