//! `pty_input` on a route (lead, plans 23aw, 23bc and 23bd): the log is one segment whose op is the unattributed label. A
//! literal part compares the concatenated log, so it passes; a relation part takes one segment and compares its op, so it
//! can never pass on the label, including `written_without_op`, which wants no op.
//!
//! Clause: Core OU-1, DP-4.

use botster_conformance::report::describe;
use botster_conformance::{load_dir, run_transcript, Limits, SeedSet, Selection};
use botster_core_conformance::{driver_for, CoreSchemas, CORE_TRANSCRIPTS};
use botster_core_testkit::TestkitHarness;
use serde_json::json;

/// `ou_1_attach_hands_transport_to_worker` sends `78` on its route as `bytes` with op "1" (step 9), and step 12 reads the
/// route's log. The testkit does not know which op wrote the log's bytes, so the log has no known attribution.
const ID: &str = "conf::ou_1_attach_hands_transport_to_worker";
const READ: usize = 12;

fn run_with(parts: serde_json::Value) -> Result<(), String> {
    let transcripts = load_dir(&CORE_TRANSCRIPTS).expect("the Core transcripts load");
    let mut transcript = transcripts
        .into_iter()
        .find(|t| t.id == ID)
        .expect("the transcript at the pinned tag");
    let step = &mut transcript.steps[READ];
    assert!(step.get("route_expect_pty").is_some(), "{step}");
    step["route_expect_pty"] = parts;
    // Only the read is under test: the steps after it are cut.
    transcript.steps.truncate(READ + 1);
    let seeds = Selection::from_env().seeds(&SeedSet::from_env());
    let outcome = run_transcript(
        &transcript,
        &|seed| driver_for(Box::new(TestkitHarness::new(seed))),
        &seeds,
        &CoreSchemas,
        &Limits::default(),
    );
    if outcome.is_pass() {
        Ok(())
    } else {
        Err(describe(ID, &outcome))
    }
}

#[test]
fn a_literal_part_passes_on_the_route_log() {
    run_with(json!([{"hex": "78"}])).unwrap();
}

#[test]
fn a_relation_part_never_passes_on_the_unattributed_op() {
    // The log has no known attribution, which is not "written without an op". A segment with a null op would falsely
    // match `written_without_op`, because the route sent the byte with op "1". The label is not a null op, so it fails.
    let failure = run_with(json!([{"written_without_op": true}])).unwrap_err();
    assert!(failure.contains("testkit:unattributed"), "{failure}");
    let failure = run_with(json!([{"written_by_op": "1"}])).unwrap_err();
    assert!(failure.contains("testkit:unattributed"), "{failure}");
}
