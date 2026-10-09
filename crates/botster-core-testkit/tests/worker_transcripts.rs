//! The transcripts that the in-process `Worker` (P3 M1) makes pass on the testkit, under the CI seed set.
//!
//! Plan section 5 (revision 23a): an id whose transcript passes on `TestkitHarness` leaves `conformance/core-pending.txt`,
//! and `RealCoreHarness` (plan 4.2b) runs it later; a real-only id (one that the replacement map classifies `slow`) leaves
//! it only with its passing real-process proof. The ids below are still pending, so the conformance harness does not run
//! them, and this test is their testkit proof until their owners take them out of the pending list. When an id leaves the
//! pending list, the conformance harness covers it and it is deleted from this list.
//!
//! Clause: Core TI-1, Core EV-4, Core LC-3, Core LC-7 (with Core A5-1: the real worker machine in-process).

use botster_conformance::report::describe;
use botster_conformance::{load_dir, run_transcript, Limits, SeedSet, Selection};
use botster_core_conformance::{driver_for, CoreSchemas, CORE_TRANSCRIPTS};
use botster_core_testkit::TestkitHarness;

/// TI-1 reads the pinned binding identity. The lifecycle ids need a real worker: a removal that ends the worker (LC-3,
/// LC-7) and an exit with its code or signal (EV-4).
const IDS: &[&str] = &[
    "conf::a2_8_terminal_identity_names_the_entry",
    "conf::ev_4_exit_has_signal",
    "conf::lc_3_remove_created",
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
