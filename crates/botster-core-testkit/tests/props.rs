//! The bolero harnesses of the testkit's byte decoders (BUILD.md "Adopted tools"; plan section 8 step 9): the route
//! stream's frame boundaries (`route_client::FrameEnds`, `route_frames_taken`) and the `unix_diag` reply (`sock_diag`).
//!
//! Each runs as a property test on stable (`cargo test`) and as a fuzzer at landing:
//! `cargo +<nightly> bolero test -p botster-core-testkit --test props <harness> -T 60s`.
//! A crash input is committed under `__fuzz__/` as a regression case.

use botster_core_testkit::route_client::FrameEnds;
use botster_core_testkit::sock_diag;

/// Cases of the default tier. The seed is pinned by `cargo xtask` through `BOLERO_RANDOM_SEED` (bolero has no API for it).
const CASES: usize = 256;

/// The frame ends of `bytes` by a second, whole-input reading: a 4-byte big-endian length, then that many bytes.
fn reference_ends(bytes: &[u8]) -> Vec<u64> {
    let mut ends = Vec::new();
    let mut at = 0u64;
    let len = bytes.len() as u64;
    while at + 4 <= len {
        let i = at as usize;
        let body = u64::from(u32::from_be_bytes([
            bytes[i],
            bytes[i + 1],
            bytes[i + 2],
            bytes[i + 3],
        ]));
        let end = at + 4 + body;
        if end > len {
            break;
        }
        ends.push(end);
        at = end;
    }
    ends
}

/// For arbitrary bytes pushed in chunks of any size: the ends are the whole-input reading's, so the chunk size never changes
/// them; each is at least a prefix after the one before; and `taken(n)` is the count of ends at or below `n`, which a frame reaches only with its last byte.
#[test]
fn frame_ends_decoder() {
    bolero::check!()
        .with_iterations(CASES)
        .for_each(|input: &[u8]| {
            let Some((first, bytes)) = input.split_first() else {
                return;
            };
            let step = 1 + (*first as usize % 9);
            let mut chunked = FrameEnds::default();
            for chunk in bytes.chunks(step) {
                chunked.push(chunk);
            }
            let mut whole = FrameEnds::default();
            whole.push(bytes);
            let expected = reference_ends(bytes);
            assert_eq!(
                chunked.ends(),
                expected,
                "the chunk size changed the frames"
            );
            assert_eq!(whole.ends(), expected);
            assert!(
                expected.windows(2).all(|w| w[0] + 4 <= w[1]),
                "each frame has its 4-byte prefix"
            );
            for (k, &end) in expected.iter().enumerate() {
                assert_eq!(
                    whole.taken(end),
                    k + 1,
                    "a frame is taken with its last byte"
                );
                assert_eq!(whole.taken(end - 1), k, "and not one byte before");
            }
            assert_eq!(whole.taken(u64::MAX), expected.len());
            // Well-formed frames whose body sizes come from the input: every frame is complete, so its end is known. Arbitrary
            // bytes rarely start with a length that fits, so this keeps the ends non-empty.
            let mut framed = Vec::new();
            let mut known = Vec::new();
            for &size in bytes.iter().take(64) {
                let body = usize::from(size % 16);
                framed.extend_from_slice(&(body as u32).to_be_bytes());
                framed.resize(framed.len() + body, size);
                known.push(framed.len() as u64);
            }
            let mut built = FrameEnds::default();
            for chunk in framed.chunks(step) {
                built.push(chunk);
            }
            assert_eq!(built.ends(), known, "well-formed frames");
            assert_eq!(built.taken(u64::MAX), known.len());
        });
}

/// One `SOCK_DIAG_BY_FAMILY` answer for socket `ino`, with `attributes` in order.
fn answer(ino: u32, attributes: &[(u16, &[u8])]) -> Vec<u8> {
    let mut body = vec![1, 1, 1, 0];
    body.extend_from_slice(&ino.to_ne_bytes());
    body.extend_from_slice(&[0xff; 8]);
    for (kind, data) in attributes {
        let start = body.len();
        body.extend_from_slice(&((4 + data.len()) as u16).to_ne_bytes());
        body.extend_from_slice(&kind.to_ne_bytes());
        body.extend_from_slice(data);
        body.resize(start + (4 + data.len()).next_multiple_of(4), 0);
    }
    let mut out = ((16 + body.len()) as u32).to_ne_bytes().to_vec();
    out.extend_from_slice(&20u16.to_ne_bytes());
    out.extend_from_slice(&[0; 10]);
    out.extend_from_slice(&body);
    out
}

/// For arbitrary bytes: `parse` never panics, and an answer that it accepts names the socket it was asked for. A well-formed
/// answer built from the input's values (peer, queue, an extra attribute, either order) parses to exactly those values,
/// and the same answer for another socket, or cut short, is an error.
#[test]
fn sock_diag_decoder() {
    bolero::check!()
        .with_iterations(CASES)
        .for_each(|input: &[u8]| {
            let ino = input
                .get(..4)
                .map_or(9, |b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]));
            if let Ok(found) = sock_diag::parse(input, ino) {
                let again = sock_diag::parse(input, ino).unwrap();
                assert_eq!(found, again);
            }
            let word = |at: usize| {
                input
                    .get(at..at + 4)
                    .map_or(at as u32, |b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]]))
            };
            let (peer, unread) = (word(4), word(8));
            let extra = input.get(12..).unwrap_or_default();
            let extra = &extra[..extra.len().min(32)];
            let (peer_bytes, unread_bytes) = (peer.to_ne_bytes(), unread.to_ne_bytes());
            let mut attributes = vec![
                (2u16, &peer_bytes[..]),
                (7u16, extra),
                (4u16, &unread_bytes[..]),
            ];
            if input.first().is_some_and(|b| b & 1 == 1) {
                attributes.reverse();
            }
            let reply = answer(ino, &attributes);
            let found = sock_diag::parse(&reply, ino).unwrap();
            assert_eq!(found, sock_diag::Found { peer, unread });
            assert!(sock_diag::parse(&reply, ino.wrapping_add(1)).is_err());
            assert!(sock_diag::parse(&reply[..reply.len() - 1], ino).is_err());
            // An error message with any error code is an error, never a panic (R319-SI-1: i32::MIN).
            for code in [word(4) as i32, i32::MIN] {
                let mut error = 20u32.to_ne_bytes().to_vec();
                error.extend_from_slice(&2u16.to_ne_bytes());
                error.extend_from_slice(&[0; 10]);
                error.extend_from_slice(&code.to_ne_bytes());
                assert!(sock_diag::parse(&error, ino).is_err());
            }
        });
}
