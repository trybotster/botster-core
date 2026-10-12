//! The Hub→client conformance runner (design 6.4). This feature holds the route step driver (`route`) and the codec transcripts.

// Failure values carry whole JSON documents for the report, as in `botster-conformance`. They occur on a cold path.
#![allow(clippy::result_large_err)]

pub mod hc;
pub mod route;

use include_dir::{include_dir, Dir};

/// The codec transcripts (`conformance/codec`), embedded.
pub static CODEC_TRANSCRIPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/codec");

include!(concat!(env!("OUT_DIR"), "/codec_tests.rs"));

include!(concat!(env!("OUT_DIR"), "/hc_hub_tests.rs"));

include!(concat!(env!("OUT_DIR"), "/hc_client_tests.rs"));

/// The UI transcripts (`conformance/ui`), embedded. The ones whose subject is the Hub run in this crate (`ui_hub_conformance_tests!`).
pub static UI_TRANSCRIPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/ui");

include!(concat!(env!("OUT_DIR"), "/ui_hub_tests.rs"));
