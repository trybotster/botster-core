//! `requires.features` is checked after `setup` opens the default handle, because a subject reports its features through that handle
//! (Core A2-6). Both outcomes are proven: a present feature runs the transcript, and an absent one is `not_applicable`, never a pass.

use botster_conformance::{run_transcript, Limits, Outcome, SeedSet, Transcript};
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::{driver_for, CoreSchemas};
use serde_json::json;

fn outcome(feature: &str) -> Outcome {
    let t: Transcript = serde_json::from_value(json!({
        "id": "conf::lc_1_open_needs_worker_path", "contract": "core", "clause": "LC-1", "subject": "core",
        "requires": { "features": [feature] },
        "steps": [{ "call": { "method": "get", "args": { "id": "nobody" } }, "expect_error": { "code": "UnknownSession" }, "because": "Core ER-0" }]
    }))
    .unwrap();
    let make = |seed: u64| driver_for(Box::new(FakeCoreHarness::new(seed)));
    run_transcript(
        &t,
        &make,
        &SeedSet::parse("0").unwrap(),
        &CoreSchemas,
        &Limits::default(),
    )
}

#[test]
fn a_feature_that_the_subject_has_runs_the_transcript() {
    assert_eq!(outcome("route_transport:stream"), Outcome::Passed);
}

#[test]
fn a_feature_that_the_subject_lacks_is_not_applicable() {
    assert_eq!(
        outcome("a_feature_that_no_subject_has"),
        Outcome::NotApplicable {
            feature: "a_feature_that_no_subject_has".into()
        }
    );
}
