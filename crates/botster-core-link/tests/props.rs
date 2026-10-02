//! The bolero harness of the control-link decoders (BUILD.md "Adopted tools"; plan section 8 step 9).
//!
//! `link_decoder` runs as a property test on stable (`cargo test`) and as a fuzzer at landing:
//! `cargo +<nightly> bolero test -p botster-core-link --test props link_decoder -T 60s`.
//! A crash input is committed under `__fuzz__/` as a regression case.

use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType, HEADER_LEN};
use botster_core_link::hello::Hello;

const MAX_PAYLOAD: u32 = 64;

/// Cases of the default tier. The seed is pinned by `cargo xtask` through `BOLERO_RANDOM_SEED` (bolero has no API for it).
const CASES: usize = 256;

/// Feeds `bytes` to a decoder in chunks of `step` bytes, the way a driver does (push, take frames, push the rest), and
/// returns every frame and whether the decoder refused. The buffer never exceeds one maximal frame.
fn decode_in_chunks(bytes: &[u8], step: usize) -> (Vec<(FrameType, Vec<u8>)>, bool) {
    let mut decoder = FrameDecoder::new(MAX_PAYLOAD);
    let mut frames = Vec::new();
    for chunk in bytes.chunks(step.max(1)) {
        let mut rest = chunk;
        while !rest.is_empty() {
            let took = decoder.push(rest);
            rest = &rest[took..];
            assert!(
                decoder.buffered() <= HEADER_LEN + MAX_PAYLOAD as usize,
                "the buffer is above one frame"
            );
            match decoder.next_frame() {
                Ok(Some(frame)) => frames.push((frame.kind, frame.payload)),
                Ok(None) => assert!(
                    rest.is_empty(),
                    "the decoder took too little with no frame to give"
                ),
                Err(_) => return (frames, true),
            }
        }
    }
    (frames, false)
}

/// For arbitrary bytes: nothing panics; a frame is at most the bound; the frames re-encode to the bytes that were consumed;
/// the chunk size never changes the result; a hello that decodes survives an encode and decode.
#[test]
fn link_decoder() {
    bolero::check!()
        .with_iterations(CASES)
        .for_each(|input: &[u8]| {
            let Some((first, bytes)) = input.split_first() else {
                return;
            };
            let step = 1 + (*first as usize % 9);

            let (whole, whole_failed) = decode_in_chunks(bytes, bytes.len().max(1));
            let (chunked, chunked_failed) = decode_in_chunks(bytes, step);
            assert_eq!(whole, chunked, "the chunk size changed the frames");
            assert_eq!(
                whole_failed, chunked_failed,
                "the chunk size changed the error"
            );

            let mut reencoded = Vec::new();
            for (kind, payload) in &whole {
                assert!(payload.len() <= MAX_PAYLOAD as usize);
                encode_frame(*kind, payload, MAX_PAYLOAD, &mut reencoded).unwrap();
            }
            assert!(
                bytes.starts_with(&reencoded),
                "the frames are not the input"
            );
            if !whole_failed {
                let rest = bytes.len() - reencoded.len();
                assert!(
                    rest < HEADER_LEN + MAX_PAYLOAD as usize,
                    "an incomplete frame is bounded"
                );
            }

            if let Ok(hello) = Hello::decode(bytes) {
                let mut again = Vec::new();
                hello.encode(&mut again).unwrap();
                assert_eq!(
                    Hello::decode(&again),
                    Ok(hello),
                    "a hello does not survive an encode and a decode"
                );
            }
        });
}

/// A valid hello need not be canonical: whitespace, another field order and an unknown field decode, and the decoded value
/// survives an encode and a decode (the property of `link_decoder`, kept deterministic).
#[test]
fn a_noncanonical_hello_survives_a_round_trip() {
    let proof = "ab".repeat(32);
    let text = format!(
        " {{ \"host_epoch\" : 7 , \"later\" : [1, {{\"x\": null}}], \"proof\":\"{proof}\",\"instance\":\"i\",\"protocol\":1,\"magic\":\"BCLK\" }} "
    );
    let hello = Hello::decode(text.as_bytes()).expect("a noncanonical hello decodes");
    let mut again = Vec::new();
    hello.encode(&mut again).unwrap();
    assert_ne!(again, text.as_bytes(), "the encoder emits its own form");
    assert_eq!(Hello::decode(&again), Ok(hello));
}

/// For arbitrary bytes: neither message decoder panics, and a message that decodes survives an encode and a decode to the
/// same value (the property of `link_decoder`, for the messages that follow the hello).
#[test]
fn msg_decoder() {
    use botster_core_link::msg::{HostMsg, WorkerMsg};
    bolero::check!()
        .with_iterations(CASES)
        .for_each(|bytes: &[u8]| {
            if let Ok(msg) = HostMsg::decode(bytes) {
                let mut again = Vec::new();
                msg.encode(&mut again);
                assert_eq!(
                    HostMsg::decode(&again),
                    Ok(msg),
                    "a host message does not survive a round trip"
                );
            }
            if let Ok(msg) = WorkerMsg::decode(bytes) {
                let mut again = Vec::new();
                msg.encode(&mut again);
                assert_eq!(
                    WorkerMsg::decode(&again),
                    Ok(msg),
                    "a worker message does not survive a round trip"
                );
            }
        });
}

/// A message with every field in a noncanonical order, with an unknown field, decodes and survives a round trip.
#[test]
fn a_noncanonical_message_survives_a_round_trip() {
    use botster_core_link::msg::WorkerMsg;
    let msg = WorkerMsg::decode(br#" { "later": {"x": [1]}, "signal": 9, "t": "exited" } "#)
        .expect("a noncanonical message decodes");
    let mut again = Vec::new();
    msg.encode(&mut again);
    assert_eq!(WorkerMsg::decode(&again), Ok(msg));
}
