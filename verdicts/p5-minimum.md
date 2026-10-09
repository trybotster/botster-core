# P5 minimum-Core conformance review

## PR #185 Round 1 — 2026-10-09

- Exact head: `539a4c7b211b9f116434ae6fb6d9b27057154707`.
- Branch: `stage1/p5-minimum-flip-20`.
- Accepted v1 base and parent: `aaac0c0d1f44172ca5d5dd5dd6986c9787be4aa6`.
- Tree: `b864908794c34abc7b09807998b10a54c5a725aa`.
- Risk tier: HIGH under BUILD.md at contracts `56bd0a5347a537d25bbee65a67854e0e317a9b9a`, rule 3.

### M20-F1 — HIGH — CLOSED — The PR now states the mandatory HIGH tier

The reviewer checked the stated tier first. The original body stated STANDARD under the lead's pending-removal ruling.
The delta also changes the shared testkit's worker_transcripts.rs. Rule 3 requires HIGH for that change.
The reviewer asked the lead to settle the boundary and informed P5.
The lead confirmed that the removal ruling covers only pending-list removals, not the shared testkit change.
P5 corrected the PR body to HIGH, rule 3, and sent READY to integration.
The reviewer verified the corrected body and unchanged exact head.
Both package and integration reviews are required. Integration owns its separate closure.

### ID and coverage checks

The reviewer read the complete two-file delta, conformance harness, worker transcript harness, minimum-ID list, probe result, current PR body, and supplied gate.
The pending list removes exactly these 20 IDs and adds none:

- `conf::a2_1_empty_argv_is_invalid_input`
- `conf::a2_1_exit_cause_values`
- `conf::a2_1_missing_cwd_is_start_failed_cwd_missing`
- `conf::a2_1_size_range_zero_is_invalid_input`
- `conf::a2_7_pump_report_fields`
- `conf::ad_1_created_rows_kept`
- `conf::am_1_create_then_start_same_turn`
- `conf::am_1_double_start_refused`
- `conf::er_0_async_errors_arrive_as_completions`
- `conf::er_0_sync_errors_allocate_no_op`
- `conf::lc_1_open_needs_worker_path`
- `conf::lc_2_data_dir_is_exclusive`
- `conf::lc_3_create_then_start`
- `conf::lc_3_duplicate_id`
- `conf::lc_4_start_failure_is_typed`
- `conf::lc_5_stop_ends_payload`
- `conf::lc_7_remove_running_is_wrong_state`
- `conf::lc_9_get_list`
- `conf::lm_1_zero_limit_is_invalid_config`
- `conf::or_1_begin_does_not_complete`

All 20 belong to the approved 70-ID minimum list on stage1/plan.
They exactly equal the probe's PASS set at aaac0c0d.
The probe is `~/botster-sessions/shared/core-stage1/p5-minimum-probe-v1-aaac0c0d.txt`.
Its other results remain six FAIL and 20 UNSUPPORTED_CONTROL. This verdict does not waive those results.

The reviewer checked a distinct passing botster-core::conformance trial for every removed ID in the exact-head gate.
The unchanged conformance harness builds TestkitHarness and passes a trial only when run_transcript returns a passing outcome.
It uses the same environment-derived seed selection as worker_transcripts.rs. The default CI seed set is 0-31.
The three deleted worker IDS are lc_3_create_then_start, lc_4_start_failure_is_typed, and lc_5_stop_ends_payload.
Each now runs in conformance. The cleanup removes duplicate execution without removing its transcript coverage.
The remaining worker IDS and both harness implementations remain unchanged.

The Core ledger, deferred table, copied contracts statuses, dependency pins, and mutation exclusions remain unchanged.
The PR body has the required Prior art note.

### Supplied exact-head gate and scope

The full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/flip20-539a4c7b.log`.
It names the exact reviewed head and accepted v1 base.
The default tier reports 942 tests passed and 655 skipped. It includes all 20 newly active conformance trials.
The slow tier reports 243 tests passed and 962 skipped.
All ten listed CI stages pass. The mutation stage reports No mutants to filter and produces no outcomes.json.
This result establishes no executed mutation campaign. The delta changes lists and comments only.
Fuzz reports no changed crate with a decoder harness and runs no harness. The job and gate exit zero.

The lead permits a TestkitHarness-passing ID to leave pending now for regression protection.
RealCoreHarness will run those IDs later. A failure there remains a finding.
This verdict establishes TestkitHarness evidence for these 20 IDs, not real-process conformance or completion of the 70-ID minimum.
It does not close the separate #176 proof hold or any failed probe ID.

No finding remains open in this HIGH package review. Integration must supply its own CLEAN before merge.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
The lead owns the merge decision.

VERDICT: CLEAN
