//! Bounded frame encoding.

use std::sync::atomic::{AtomicUsize, Ordering};

use serde::ser::{Serialize, SerializeSeq, Serializer};

use super::{encode_json_bounded, FRAME_LOAD};
use crate::contract::session_protocol::{FrameDecoder, ProtocolError};

/// A long sequence that counts how many of its elements were serialized.
struct CountingSeq<'a> {
    len: usize,
    serialized: &'a AtomicUsize,
}

impl Serialize for CountingSeq<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len))?;
        for index in 0..self.len {
            seq.serialize_element(&index)?;
            self.serialized.fetch_add(1, Ordering::SeqCst);
        }
        seq.end()
    }
}

#[test]
fn serialization_stops_at_the_frame_bound() {
    const BOUND: usize = 1024;
    const ELEMENTS: usize = 1_000_000;
    let serialized = AtomicUsize::new(0);

    let result = encode_json_bounded(
        FRAME_LOAD,
        &CountingSeq {
            len: ELEMENTS,
            serialized: &serialized,
        },
        BOUND,
    );

    assert!(matches!(
        result,
        Err(ProtocolError::FrameEncodeTooLarge { len, max: BOUND }) if len > BOUND
    ));
    // Each element costs at least two bytes ("n,"), so at most BOUND / 2
    // elements can have been written before the refusal.
    assert!(serialized.load(Ordering::SeqCst) <= BOUND / 2);
}

#[test]
fn a_frame_at_the_bound_encodes_without_spare_capacity_and_round_trips() {
    // "\"aaa…\"" is the payload; the type byte makes the frame one longer.
    let text = "a".repeat(98);
    let bound = text.len() + 2 + 1;

    let frame = encode_json_bounded(FRAME_LOAD, &text, bound).expect("fits exactly");
    assert!(frame.capacity() <= 4 + bound);

    let mut decoder = FrameDecoder::with_max_len(bound);
    let frames = decoder.feed(&frame).expect("decodes");
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].frame_type, FRAME_LOAD);
    assert_eq!(frames[0].json::<String>().expect("json"), text);

    assert!(matches!(
        encode_json_bounded(FRAME_LOAD, &text, bound - 1),
        Err(ProtocolError::FrameEncodeTooLarge { .. })
    ));
}
