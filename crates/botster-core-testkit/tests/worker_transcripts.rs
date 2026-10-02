//! The transcripts that the in-process `Worker` (P3 M1) makes pass on the testkit, under the CI seed set.
//!
//! They stay in `conformance/core-pending.txt` until they also pass on the real-process harness (plan 4.2b; `RealCoreHarness`
//! does not exist yet), so the conformance harness does not run them. This test is their testkit proof until then. When an id
//! leaves the pending list, the conformance harness covers it and it is deleted from this list.
//!
//! Clause: Core LC-3, Core LC-4 (with Core A5-1: the real worker machine in-process).

use botster_conformance::report::describe;
use botster_conformance::{load_dir, run_transcript, Limits, SeedSet, Selection};
use botster_core_conformance::{driver_for, CoreSchemas, CORE_TRANSCRIPTS};
use botster_core_testkit::TestkitHarness;

/// Each id needs a real worker: a payload launched after the hello (LC-3), a launch failure (LC-4), a removal that ends the
/// worker (LC-3, LC-7).
const IDS: &[&str] = &[
    "conf::lc_3_create_then_start",
    "conf::lc_3_remove_created",
    "conf::lc_4_start_failure_is_typed",
];

#[test]
fn the_worker_transcripts_pass_on_the_testkit() {
    let transcripts = load_dir(&CORE_TRANSCRIPTS).expect("the Core transcripts load");
    let seeds = Selection::from_env().seeds(&SeedSet::from_env());
    let mut bad = Vec::new();
    for id in IDS {
        let Some(transcript) = transcripts.iter().find(|t| t.id == *id) else {
            bad.push(format!("{id}: no transcript at the pinned tag"));
            continue;
        };
        let outcome = run_transcript(
            transcript,
            &|seed| driver_for(Box::new(TestkitHarness::new(seed))),
            &seeds,
            &CoreSchemas,
            &Limits::default(),
        );
        if !outcome.is_pass() {
            bad.push(describe(id, &outcome));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
