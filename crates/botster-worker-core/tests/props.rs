//! The bolero harness of the worker's client-frame receipt (Core ST-7; steward rulings R-102 and R-103; BUILD.md
//! "Adopted tools"; plan section 8 step 9). The receipt reads a refused frame's raw JSON body into its record.
//!
//! `receipt_decoder` runs as a property test on stable (`cargo test`) and as a fuzzer at landing:
//! `cargo +<nightly> bolero test -p botster-worker-core --test props receipt_decoder -T 60s`.
//! A crash input is committed under `__fuzz__/` as a regression case.

use botster_core_contract::prelude::*;
use botster_route_codec::prelude::{
    FrameBounds, TYPE_BYTES, TYPE_FILE_CHUNK, TYPE_INPUT, TYPE_PASTE_CHUNK, TYPE_TEXT,
};
use botster_worker_core::worker::record_of_client_frame;

/// Cases of the default tier. The seed is pinned by `cargo xtask` through `BOLERO_RANDOM_SEED` (bolero has no API for
/// it).
const CASES: usize = 256;

/// The client frame types; the first input byte picks one, or keeps its own value for a type that no frame has.
const TYPES: [u8; 5] = [
    TYPE_BYTES,
    TYPE_TEXT,
    TYPE_INPUT,
    TYPE_PASTE_CHUNK,
    TYPE_FILE_CHUNK,
];

const BOUNDS: FrameBounds = FrameBounds {
    max_frame: 4096,
    max_screen: 4096,
    max_history: 4096,
};

/// For arbitrary frames: the receipt never panics. A record is `pending` with a null `pty_bytes`, keeps at most
/// `payload_cap` payload bytes, is `payload_truncated` only with a payload cut at the cap (R-103), and survives a JSON
/// encode and decode, the form in which the link carries it.
#[test]
fn receipt_decoder() {
    bolero::check!()
        .with_iterations(CASES)
        .for_each(|input: &[u8]| {
            let [kind, cap, body @ ..] = input else {
                return;
            };
            let kind = TYPES.get(usize::from(*kind)).copied().unwrap_or(*kind);
            let cap = usize::from(*cap % 64);
            let frame = [&[kind], body].concat();
            let Some(record) = record_of_client_frame(&frame, &BOUNDS, cap) else {
                return;
            };
            assert_eq!(record.state, InputRecordState::Pending);
            assert_eq!(record.pty_bytes, None);
            let kept = record.payload.as_ref().map(|p| p.0.len());
            assert!(kept.is_none_or(|n| n <= cap), "{kept:?} > {cap}");
            if record.payload_truncated {
                assert_eq!(kept, Some(cap), "a cut payload is cut at the cap");
            }
            let json = serde_json::to_vec(&record).unwrap();
            let again: InputRecord = serde_json::from_slice(&json).unwrap();
            assert_eq!(again, record, "a record does not survive a round trip");
        });
}

/// A refused key input whose body the contract types do not hold has no payload, and is not `payload_truncated`; an
/// unknown kind keeps its body as bytes (R-103 (1)); a paste's begin is not input (R-102).
#[test]
fn the_receipt_reads_what_the_frame_holds() {
    let input = |body: &str| [&[TYPE_INPUT][..], body.as_bytes()].concat();
    let paste_begin = input(r#"{"kind":"paste_begin","op":1,"bytes":4}"#);
    assert_eq!(record_of_client_frame(&paste_begin, &BOUNDS, 64), None);
    let body = r#"{"kind":"later","op":1}"#;
    let later = record_of_client_frame(&input(body), &BOUNDS, 64).unwrap();
    assert_eq!(later.kind, InputKind::Bytes);
    assert_eq!(later.payload.unwrap().0, body.as_bytes());
}

// This test binary was built in the worktree where it runs (bug catalog B13).
botster_test_process::built_in_test!();
