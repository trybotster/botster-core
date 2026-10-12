//! `from_make` is a partial-suite convenience (design 6.2): a transcript that needs one default handle runs, and every other
//! transcript gives `unsupported_control`, which is not a pass.

use botster_conformance::{run_transcript, Limits, Outcome, SeedSet};
use botster_core_conformance::{driver_for, from_make, CoreSchemas, CORE_TRANSCRIPTS};
use botster_core_contract::prelude::*;
use botster_fake_core::{FakeCore, FakeStore};

fn outcome(id: &str) -> Outcome {
    let transcripts = botster_conformance::load_dir(&CORE_TRANSCRIPTS).unwrap();
    let t = transcripts.iter().find(|t| t.id == id).expect("transcript");
    let make = |seed: u64| {
        let make = move || -> Box<dyn CoreApi> {
            let config = OpenConfig {
                data_dir: "d".into(),
                worker_path: Some("w".into()),
                limits: CoreLimits::default(),
            };
            Box::new(FakeCore::open(&FakeStore::new(), config, seed).unwrap())
        };
        driver_for(Box::new(from_make(make)))
    };
    run_transcript(
        t,
        &make,
        &SeedSet::local(),
        &CoreSchemas,
        &Limits::default(),
    )
}

#[test]
fn a_one_handle_transcript_runs() {
    assert_eq!(outcome("conf::lc_3_duplicate_id"), Outcome::Passed);
}

#[test]
fn a_transcript_that_needs_more_is_unsupported_not_passed() {
    assert!(matches!(
        outcome("conf::lc_2_data_dir_is_exclusive"),
        Outcome::UnsupportedControl { .. }
    ));
    assert!(matches!(
        outcome("conf::lm_1_zero_limit_is_invalid_config"),
        Outcome::UnsupportedControl { .. }
    ));
}
