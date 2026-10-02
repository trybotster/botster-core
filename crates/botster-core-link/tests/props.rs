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

/// Feeds `bytes` to a decoder in chunks of `step` bytes and returns every frame and the error, if any.
fn decode_in_chunks(bytes: &[u8], step: usize) -> (Vec<(FrameType, Vec<u8>)>, bool) {
    let mut decoder = FrameDecoder::new(MAX_PAYLOAD);
    let mut frames = Vec::new();
    for chunk in bytes.chunks(step.max(1)) {
        decoder.push(chunk);
        loop {
            match decoder.next_frame() {
                Ok(Some(frame)) => frames.push((frame.kind, frame.payload)),
                Ok(None) => break,
                Err(_) => return (frames, true),
            }
        }
    }
    (frames, false)
}

/// For arbitrary bytes: nothing panics; a frame is at most the bound; the frames re-encode to the bytes that were consumed;
/// the chunk size never changes the result; a hello that decodes encodes back to the same bytes.
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
                    again, bytes,
                    "a hello is not the bytes that it decoded from"
                );
            }
        });
}
