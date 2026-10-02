//! The transcripts that the in-process `Worker` (P3 M1) makes pass on the testkit, under the CI seed set.
//!
//! They stay in `conformance/core-pending.txt` until they also pass on the real-process harness (plan 4.2b; `RealCoreHarness`
//! does not exist yet), so the conformance harness does not run them. This test is their testkit proof until then. When an id
//! leaves the pending list, the conformance harness covers it and it is deleted from this list.
//!
//! Clause: Core EV-4, Core LC-3, Core LC-4, Core LC-5, Core AM-2, Core IN-1 to IN-10, Core A2-2, Core ST-7 (with Core A5-1:
//! the real worker machine in-process).

use botster_conformance::report::describe;
use botster_conformance::{load_dir, run_transcript, Limits, SeedSet, Selection};
use botster_core_conformance::{driver_for, CoreSchemas, CORE_TRANSCRIPTS};
use botster_core_testkit::TestkitHarness;

/// Each id needs a real worker: the launch and the end of the payload (EV-4, LC-3, LC-4, LC-5), and the admission point with
/// the host's writes (AM-2, IN-1 to IN-10, A2-2), with the program and process controls of the testkit. The validation ids
/// that the host decides alone (IN-5, IN-9 input checks, ST-7 limits) run here too: they need a started session.
const IDS: &[&str] = &[
    "conf::a2_2_cancel_is_ok_cancelled_never_err",
    "conf::a2_2_cancel_race_reports_the_real_outcome",
    "conf::a2_2_sent_unacknowledged_write_is_unknown_on_link_loss",
    "conf::a2_2_write_input_never_completes_with_err_except_internal",
    "conf::a2_2_write_to_ended_is_ok_not_written_session_ended",
    "conf::am_2_next_write_waits_for_completion_or_partial_end",
    "conf::ev_4_exit_has_signal",
    "conf::in_10_guard_ignores_its_own_admission",
    "conf::in_10_guard_passes_when_unchanged",
    "conf::in_10_terminal_guard_passes_when_model_rev_is_unchanged",
    "conf::in_2_link_loss_is_unknown_not_zero",
    "conf::in_2_partial_is_certain",
    "conf::in_3_host_write_result_is_a_completed_event",
    "conf::in_5_lane_bounds_refuse_at_begin",
    "conf::in_5_oversize_payload_is_payload_too_large",
    "conf::in_6_cancel_reports_progress",
    "conf::in_6_repeat_cancel_is_idempotent_no_slot",
    "conf::in_7_lost_worker_ends_pending",
    "conf::in_7_write_to_ended",
    "conf::in_9_admission_bound_holds_for_every_mode",
    "conf::in_9_explicit_repeat_with_release_or_repeat_event_is_invalid_input",
    "conf::in_9_malformed_input_is_invalid_input_sync",
    "conf::lc_3_create_then_start",
    "conf::lc_3_remove_created",
    "conf::lc_4_start_failure_is_typed",
    "conf::lc_5_stop_ends_payload",
    "conf::st_7_limits_that_cannot_hold_the_protected_records_are_invalid_config",
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
