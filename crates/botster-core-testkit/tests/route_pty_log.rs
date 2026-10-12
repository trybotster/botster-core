//! `pty_input` on a route (lead, plans 23aw, 23bc and 23bd): the log is one segment whose op is the unattributed label. A
//! literal part compares the concatenated log, so it passes. Since R-73 (contracts #53, contracts-v0.1.40) the route driver
//! resolves a relation part from the transcript's own sent frames and the byte log: `written_by_op` of the op that sent the
//! bytes passes, and `written_without_op` still fails, because the route sent the byte with an op.
//!
//! Clause: Core OU-1, DP-4; steward ruling R-73.

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
fn a_relation_part_resolves_from_the_sent_frames() {
    // The log has no known attribution, which is not "written without an op". A segment with a null op would falsely
    // match `written_without_op`, because the route sent the byte with op "1". The label is not a null op, so it fails.
    let failure = run_with(json!([{"written_without_op": true}])).unwrap_err();
    assert!(failure.contains("testkit:unattributed"), "{failure}");
    // R-73: the op is resolved from the transcript's frame that sent `78` (op "1"), not from the log's label.
    run_with(json!([{"written_by_op": "1"}])).unwrap();
    // An op that sent nothing on the route does not match.
    run_with(json!([{"written_by_op": "2"}])).unwrap_err();
}

// This test binary was built in the worktree where it runs (bug catalog B13).
botster_test_process::built_in_test!();
