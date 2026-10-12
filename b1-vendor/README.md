# botster-contracts

The v1 contracts between Botster's parts. Each side builds against these contracts independently and proves it with its own conformance suite. Fixing a bug in one side never requires a change in another.

```
clients (TUI, Web, CLI, agents) ── Hub→client ──┐
                                                 Hub (policy) ── Core→Hub ── Core (headless multiplexer library)
plugins (service processes)    ── Hub→plugin ──┘
client ↔ session worker: route codec (terminal bytes; the Hub is off this path)
```

## Status

**Frozen design, manifest final6** (2026-09-30). The three Sol reviewers (Hub A, Hub B, Core) each gave a joint final ACK on the exact file hashes in [`frozen/final6/MANIFEST.md`](frozen/final6/MANIFEST.md). Completion gates 1 to 5 pass (records in `frozen/final6/audits/`).

**Current manifest: final35** (2026-10-05) = final6 + ui-contract 1.4 (final7) + Hub→client Amendment 4 (final8) + Core Amendment 4 (final9) + ui-contract 1.4 erratum 1 (final10) + Core erratum 1 (final11) + Core Amendment 5 and Hub→client Amendment 5 (final12) + Core Amendment 6 (final13) + Core erratum 2 (final14) + Core Amendment 7 (final15) + Core Amendment 8 (final16) + Core Amendment 9 (final17) + Core Amendment 10 (final18) + Core erratum 3 (final19) + Core Amendment 11 (final20) + Core Amendment 12 (final21) + Core erratum 4 (final22) + Hub→plugin Amendment 3 (final23) + Hub→plugin Amendment 4 (final24) + Hub→plugin Amendment 5 (final25) + Lua binding r9 Amendment 1 (final26) + Hub→plugin Amendment 6 (final27) + Lua binding r9 Amendment 2 (final28) + Hub→plugin Amendment 7 (final29) + Core Amendment 13 (final30) + Hub→plugin Amendment 8 (final31) + Hub→client Amendment 6 (final32) + Core Amendment 14 (final33) + Core Amendment 16 (final34) + Core Amendment 15 (final35), each accepted by the Amendment reviewer (Sol). It lives in [`frozen/current/MANIFEST.md`](frozen/current/MANIFEST.md).

This approves the design only. Build acceptance means the conformance suites and journeys J1 to J7 pass against the real implementations.

## What is here

| Contract | Base file | Amendments |
|---|---|---|
| Core→Hub (Core library) | `core-contract-v1.17.md` | Core A2, Core A3, Core A4, Core errata 1 to 4, Core A5, Core A6, Core A7, Core A8, Core A9, Core A10, Core A11, Core A12, Core A13, Core A14, Core A15, Core A16 |
| Route codec (client ↔ session worker wire; owned by Hub→client, adopted verbatim by Core) | `route-codec-v1-extract-rev36.md` is current; rev26 and rev30 are kept for the hash record | |
| Hub→client | `hub-client-contract-v1-candidate-parity45.md` | HC A1, A2, A3, A4, A5, A6 |
| Hub→plugin | `hub-plugin-contract-v1.1.joint-J4e.md` | HP A1, A2, A3, A4, A5, A6, A7, A8 |
| Hub→plugin Lua binding | `hub-plugin-lua-binding-v1.1-r9.md` (replaces r6) | LB A1, A2 |
| UI (declarative plugin views) | `ui-contract-v1-frozen-20260930.md` | 1.1, 1.2, 1.3, 1.4 (with erratum 1) |
| `botster` CLI spec (a client of Hub→client; informative) | `cli-spec-v1-candidate34.md` | |

All files keep their original names, so every sha256 in the manifest can be checked directly:

```sh
cd frozen/current && shasum -a 256 *.md audits/*.md   # or: cargo xtask manifest
```

## Rules

1. **The frozen files never change.** A change is a new amendment file.
   - **Layout (orchestrator, 2026-09-30).** `frozen/final6/` is the record of the joint freeze. `frozen/current/` holds the current set and is **add-only**: an accepted amendment adds its file, and `frozen/current/MANIFEST.md` is the only file that is ever edited. `contracts.toml` points at `frozen/current` and never moves.
   - **Versions are tags.** Each manifest commit is tagged `manifest-finalN` (`manifest-final6` to `manifest-final35` exist; final7 to final10 were published as `frozen/finalN/` directories at their tags). Check an older version with `git show manifest-finalN:frozen/finalN/MANIFEST.md`.
   - **CI guard.** The `frozen` job fails if any existing file under `frozen/` is modified or deleted, except `frozen/current/MANIFEST.md`.
2. **Amendment path:** a numbered amendment with its clauses and conformance test ids, the owning side's update, a Sol review, and a new manifest entry. Changes found during the build go through this path; no side works around a contract.
3. **What belongs in a contract:** anything a second party must agree on, such as wire formats, schemas, file names and locations, event names, CLI behavior and error codes. Internals that only one side sees are implementation.
4. **Ground up:** contracts state ideal behavior, not today's mechanism. "Today" and "Reusable from" notes are non-normative.
5. **Generated artifacts:** the JSON Schemas named in Hub→plugin Appendix B, and the client kind schemas, are generated from the contract crates. A conformance test fails if code and schema differ.

## Building

The workspace pins its toolchain in `rust-toolchain.toml`. Tooling runs through `cargo xtask <command>` (see [`xtask/README.md`](xtask/README.md)):

```sh
cargo xtask ledger --check    # the test-id ledger, the transcripts, conformance/pending.txt, deferred.txt and withdrawn.txt
cargo xtask manifest          # the sha256 of every frozen file
cargo xtask taint             # banned old-world names in tracked code
cargo xtask clause-lint       # clause citations in the fakes exist in the ledger
cargo xtask codegen --check   # generated artifacts match the code
cargo xtask timers            # no unmarked sleep in test code or fakes
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo xtask test-budget       # the default test tier, with its budgets
cargo xtask test-budget --slow   # the slow tier
```

`cargo xtask ledger` and `cargo xtask codegen` rewrite the checked-in files. CI runs the same checks (`.github/workflows/ci.yml`).

### The merge gate: `cargo xtask ci`

Every job of `.github/workflows/ci.yml` runs `cargo xtask ci --job <name>`, and `cargo xtask ci` runs all the jobs in order, so the workflow and the local
gate cannot drift (a test in `xtask/src/ci.rs` fails when a workflow job runs anything else). While GitHub-hosted Actions are blocked, the gate is one run on the exact head,
on the LAN Linux test host or on the Mac, whichever has the resources (docs/BUILD.md "How each stage runs"):

```sh
~/botster-sessions/shared/tools/botster-gate <worktree>     # the client is ci/remote/botster-gate
```

On Linux the gate runs in the image of `ci/remote/Dockerfile` through `ci/remote/job.sh`; on the Mac it runs through `botsterq run --exclusive`.

| Flag | Effect |
|---|---|
| (none) | Runs the jobs `manifest frozen taint ledger rust slow codegen ts fuzz mutants`; stops at the first failure; prints a PASS, FAIL, SKIPPED or NOT RUN line with the time of each job. |
| `--keep-going` | Runs every job after a failure. |
| `--job <name>` | Runs one job (what a workflow job does). `--list` prints the names. |

**Parallelism is capped by the tool itself** (docs/BUILD.md "Resources"): xtask sets `CARGO_BUILD_JOBS=4`, `NEXTEST_TEST_THREADS=4` and `RUST_TEST_THREADS=4` in the environment of every cargo,
nextest and npm child, in `cargo xtask ci` and in `cargo xtask test-budget` alike (a lower inherited value stays); vitest has `maxWorkers: 4` in `ts/vitest.shared.ts`;
mutants runs 2 workers (1 in the fakes). A gate needs no extra flag. The code is `xtask/src/caps.rs`, with its test.

The diff jobs (`frozen`, `ledger`, `mutants`) compare with `origin/main`; `BOTSTER_CI_BASE_REF` names another base ref. Run `git fetch origin` first.
A job whose tool is absent prints `SKIPPED <job>: <reason>` and the summary says the run is not the full gate; the gate is complete only with no SKIPPED line.
The local gate needs `npm` (job `ts`), the `nightly` toolchain and `cargo-bolero` (job `fuzz`), and `cargo-mutants` (job `mutants`). On CI (`CI` set) a missing cargo tool is installed.


### Property tests, fuzzing and mutation tests

The tools and their reasons are in [`docs/tooling.md`](docs/tooling.md). Install the two that are not in the default tier once:
`cargo install cargo-bolero --locked`, `cargo install cargo-mutants --version 27.1.0 --locked`, and `rustup toolchain install nightly --profile minimal` (for fuzzing only).

| Tool | Command | When it runs |
|---|---|---|
| **proptest** with **test-strategy** | `cargo test -p botster-route-codec props` | Every push, in the default tier: `cases = 64` and a checked-in failure file, `crates/<crate>/proptest-regressions/props.txt`. A failing case is written there; commit the file and the case replays. Raise the count for a one-off search: `PROPTEST_CASES=20000 cargo test -p botster-route-codec props`. |
| **bolero** | `cargo test -p botster-route-codec props::bolero_route_frame_decoder` (a property test), and `cargo +nightly bolero test -p botster-route-codec props::bolero_route_frame_decoder -T 60s` (the fuzzer) | The property test every push. The fuzzer in the landing job `fuzz`, the one job that uses the nightly toolchain. A crash input goes to `crates/<crate>/src/__fuzz__/<test>/crashes/`; check it in, and the property test replays it. The corpus next to it is ignored. |
| **cargo-mutants** | `git diff origin/main...HEAD > mutants.diff && cargo mutants --in-diff mutants.diff` (the changed code); `cargo mutants -p botster-route-codec` (one crate) | The landing job `mutants` runs `--in-diff` and fails on a surviving mutant. The workflow `mutants-weekly` runs the whole fakes and the codecs. |

A generated value in a property test is validated against the checked-in schema (`jsonschema`), so the test is not its own oracle.

**A surviving mutant is a review finding.** Fix it with a test that fails for the mutant, from a clause. If the mutant cannot change behavior (an equivalent
mutant), the pair records it in `.cargo/mutants.toml`: an entry in `exclude_re` (a regex over the name that `cargo mutants --list` prints), with a comment above
the entry that names the function, why no test can tell the mutant from the original, and the pair. The reviewer judges the reason. An entry never serves to make a job pass.

`crates/botster-test-support` holds the helpers of the test tooling: the process census that `cargo xtask test-budget` uses (a zombie counts as a leftover;
a child that calls `setsid` is still found), a TERM, grace, KILL, reap and drain driver for slow-tier tests, and a short canonical temp root for sockets.
Its real-process controls run in the slow tier (`cargo xtask test-budget --slow`).

### Real-only conformance ids

Botster's terminal semantics are libghostty. A fake in this repo carries script data and never emulates, parses or encodes terminal bytes (steward rules R-5 and R-7, `docs/BUILD.md` architecture rules). So some ids prove nothing against a fake. The lists below come from the transcripts. `cargo xtask real-only --check` fails in CI when they are out of date.

<!-- real-only:begin (generated by `cargo xtask real-only`; do not edit) -->

Ids marked `fake: structural`. A fake only checks their stimuli. They run in full against a real implementation, where libghostty is the oracle.

| Id | Ruling |
|---|---|
| `conf::a10_1_wrong_instance_is_never_signalled` | R-12 |
| `conf::a10_1_wrong_token_is_never_signalled` | R-12 |
| `conf::a10_2_corrupted_row_is_lost_registry_corrupt` | R-12 |
| `conf::a11_1_frames_after_a_failed_handshake_change_nothing` | R-12 |
| `conf::a12_1_held_report_released_proceeds_normally` | R-12 |
| `conf::a12_1b_partial_cleanup_claims_no_paths` | R-12 |
| `conf::a2_1_adopt_completes_ok_with_the_state_it_reached` | R-12 |
| `conf::a2_1_attach_state_rules` | R-12 |
| `conf::a2_1_env_is_exact` | R-12 |
| `conf::a2_1_every_begin_operation_has_a_row_and_the_documented_ok_value` | R-12 |
| `conf::a2_1_no_deadline_code_exists` | R-12 |
| `conf::a2_1_not_admitted_state_is_wrong_state_sync_for_every_row` | R-12 |
| `conf::a2_1_read_screen_history_flag_and_text` | R-12 |
| `conf::a2_1_state_change_in_flight_is_wrong_state_async` | R-12 |
| `conf::a2_1_sync_errors_allocate_no_op_for_every_row` | R-12 |
| `conf::a2_2_cancel_is_ok_cancelled_never_err` | R-12 |
| `conf::a2_2_cancel_race_reports_the_real_outcome` | R-12 |
| `conf::a2_2_missing_op_is_bad_frame_not_refused` | R-12 |
| `conf::a2_2_never_sent_write_is_not_written_zero` | R-12 |
| `conf::a2_2_refused_is_a_sequencing_error_route_input_only` | R-12 |
| `conf::a2_2_sent_unacknowledged_write_is_unknown_on_link_loss` | R-12 |
| `conf::a2_2_unsupported_what_values` | R-12 |
| `conf::a2_2_write_input_never_completes_with_err_except_internal` | R-12 |
| `conf::a2_2_write_to_ended_is_ok_not_written_session_ended` | R-12 |
| `conf::a2_3_close_first_reason_wins_with_new_reasons` | R-12 |
| `conf::a2_3_every_route_close_reason_maps_to_one_wire_reason` | R-12 |
| `conf::a2_3_lost_session_closes_routes_session_lost` | R-12 |
| `conf::a2_3_unknown_reason_maps_to_transport_lost` | R-12 |
| `conf::a2_3_write_failed_closes_with_write_failed` | R-12 |
| `conf::a2_4_bell_payload` | R-12 |
| `conf::a2_4_events_lost_kinds_set` | R-12 |
| `conf::a2_4_every_payload_carries_instance_and_at` | R-12 |
| `conf::a2_4_notification_osc9_and_osc777` | R-12 |
| `conf::a2_4_notification_oversize_is_truncated_with_flag` | R-12 |
| `conf::a2_4_other_osc_forms_stay_in_output` | R-12 |
| `conf::a2_4_prompt_mark_each_mark_and_exit_code` | R-12 |
| `conf::a2_5_peer_identity_and_required_bounds` | R-12 |
| `conf::a2_5_recv_after_exit_until_remove` | R-12 |
| `conf::a2_5_send_before_epoch_commit_is_not_connected` | R-12 |
| `conf::a2_5_send_to_exited_is_service_ended` | R-12 |
| `conf::a2_5_service_recv_empty_is_none` | R-12 |
| `conf::a2_5_service_send_error_values` | R-12 |
| `conf::a2_5_spawn_failure_codes` | R-12 |
| `conf::a2_5_startup_runs_to_epoch_one_commit` | R-12 |
| `conf::a2_7_pump_report_fields` | R-12 |
| `conf::a2_8_core_sets_no_term_in_the_session_env` | R-12 |
| `conf::a2_8_terminal_identity_names_the_entry` | R-12 |
| `conf::a2_8_terminfo_source_compiles_with_tic` | R-12 |
| `conf::a2_9_extension_too_long_to_keep_is_dropped_with_the_name_cut` | R-12 |
| `conf::a2_9_leading_dot_is_not_an_extension` | R-12 |
| `conf::a2_9_long_name_is_never_refused` | R-12 |
| `conf::a2_9_long_stem_is_cut_and_extension_kept` | R-12 |
| `conf::a2_9_suffix_room_is_reserved_so_the_name_stays_unique_and_within_the_limit` | R-12 |
| `conf::a3_1_a_notification_is_never_counted_in_two_discards` | R-12 |
| `conf::a3_1_all_dropped_fail_the_policy_sends_no_frame` | R-12 |
| `conf::a3_1_bell_osc9_osc777_are_counted_and_osc52_is_not` | R-12 |
| `conf::a3_1_body_is_shortened_before_title_at_char_boundaries_and_truncated_is_set` | R-12 |
| `conf::a3_1_count_counts_only_passing_and_last_is_the_last_passing` | R-12 |
| `conf::a3_1_empty_pattern_list_matches_nothing` | R-12 |
| `conf::a3_1_empty_pattern_matches_every_osc9_and_osc777_but_not_a_bell` | R-12 |
| `conf::a3_1_empty_title_or_body_is_omitted_never_an_empty_string` | R-12 |
| `conf::a3_1_feature_negotiated_in_attached_features` | R-12 |
| `conf::a3_1_host_notification_events_are_unchanged` | R-12 |
| `conf::a3_1_last_fits_the_route_max_frame_bytes_including_json_escapes` | R-12 |
| `conf::a3_1_no_feature_no_frame_and_no_guarantee` | R-12 |
| `conf::a3_1_no_policy_is_refused_for_size_or_content` | R-12 |
| `conf::a3_1_output_dropped_is_retained_and_survives_the_resync` | R-12 |
| `conf::a3_1_output_dropped_waits_in_the_route_queue_under_route_queue_bytes` | R-12 |
| `conf::a3_1_overflow_resync_reports_dropped_notifications` | R-12 |
| `conf::a3_1_policy_all_counts_a_bell` | R-12 |
| `conf::a3_1_policy_is_applied_at_the_time_of_the_discard` | R-12 |
| `conf::a3_1_policy_matches_the_full_text_before_shortening` | R-12 |
| `conf::a3_1_policy_matching_never_matches_a_bell` | R-12 |
| `conf::a3_1_policy_matching_osc777_title_newline_body` | R-12 |
| `conf::a3_1_policy_matching_substring_on_osc9_message` | R-12 |
| `conf::a3_1_policy_none_sends_no_frame` | R-12 |
| `conf::a3_1_policy_survives_host_restart` | R-12 |
| `conf::a3_1_set_notification_policy_errors_and_states` | R-12 |
| `conf::a3_1_set_policy_in_created_is_stored_in_the_row_and_reaches_the_worker_at_start` | R-12 |
| `conf::a3_1_stall_resync_reports_dropped_notifications_once_before_the_resync` | R-12 |
| `conf::a3_1_started_frame_is_completed_and_its_notifications_are_not_counted` | R-12 |
| `conf::a3_1_unshortened_last_has_no_truncated_field` | R-12 |
| `conf::a3_2_close_from_stalled_runs_dp12` | R-7 |
| `conf::a3_2_resume_writes_no_focus_in` | R-12 |
| `conf::a3_2_stall_changes_no_focus_value_and_posts_no_focus_changed` | R-12 |
| `conf::a3_2_stall_writes_no_focus_out` | R-12 |
| `conf::a3_3_features_list_output_dropped_and_notification_policy` | R-12 |
| `conf::a3_3_set_notification_policy_has_its_table_row` | R-12 |
| `conf::a4_1_service_report_after_exit_until_remove` | R-12 |
| `conf::a4_1_service_report_equals_spawn_completion` | R-12 |
| `conf::a4_1_service_report_survives_adoption` | R-12 |
| `conf::a4_1_unknown_or_removed_service` | R-12 |
| `conf::a5_1_deterministic_under_script_and_seed` | R-12 |
| `conf::a5_1_edges_scripted` | R-12 |
| `conf::a5_1_not_in_facade_or_ffi` | R-12 |
| `conf::a5_1_testkit_is_real_core_one_worker_code_path` | R-12 |
| `conf::a5_2_deferred_completion_keeps_slot_until_polled` | R-12 |
| `conf::a5_2_due_deadline_processed_in_its_pump` | R-12 |
| `conf::a5_2_each_section_11_variation_has_a_source` | R-12 |
| `conf::a5_2_reads_match_atomic_transitions` | R-12 |
| `conf::a5_3_async_failure_only_through_edges_and_real_completion` | R-12 |
| `conf::a5_3_every_code_reachable_or_listed_real_only` | R-12 |
| `conf::a5_3_only_sync_column_codes_scripted` | R-12 |
| `conf::a5_3_sync_refusal_has_no_op_event_slot_or_state` | R-12 |
| `conf::a5_4_hub_tests_on_testkit_under_seeds` | R-12 |
| `conf::a5_4_suite_runs_real_and_testkit` | R-12 |
| `conf::a6_1_lane_and_epoch_outcomes_match_real_lanes` | R-12 |
| `conf::a6_1_peer_identity_is_real_process_only` | R-12 |
| `conf::a6_1_scripted_service_over_in_memory_lanes_uses_the_real_preamble_and_epoch_checks` | R-12 |
| `conf::a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone` | R-12 |
| `conf::a6_2_deferred_set_is_exactly_the_enumerated_ids_with_start_conditions` | R-12 |
| `conf::a6_2_first_protocol_adoptable_set_is_one` | R-12 |
| `conf::a6_2_out_of_set_hello_is_lost_worker_version` | R-12 |
| `conf::a6_3_delete_failure_does_not_stop_teardown_and_lists_exact_paths` | R-12 |
| `conf::a6_3_lost_result_reports_outcome_unknown` | R-12 |
| `conf::a6_3_unauthenticated_result_is_not_trusted` | R-12 |
| `conf::a6_3_unreachable_worker_reports_outcome_unknown` | R-12 |
| `conf::a6_3_uploads_deleted_at_remove_report_deleted` | R-12 |
| `conf::a6_3_worker_lost_during_cleanup_reports_outcome_unknown` | R-12 |
| `conf::a7_1_current_lock_is_not_counted` | R-1.3 |
| `conf::a7_1_frame_cap_above_route_frame_max_is_invalid_input` | R-12 |
| `conf::a7_1_overlong_route_tag_or_owner_is_invalid_input` | R-12 |
| `conf::a7_1_query_deadline_above_max_is_invalid_input` | R-12 |
| `conf::a7_1_query_deadline_absent_or_zero_is_invalid_input` | R-12 |
| `conf::a7_1_query_deadline_positive_below_1ms_is_invalid_input` | R-12 |
| `conf::a7_1_reduction_keeps_entries_admitted_under_the_larger_limit` | R-1.3 |
| `conf::a7_1_reduction_then_update_and_rollback` | R-1.3 |
| `conf::a7_1_refused_attach_reserves_nothing` | R-12 |
| `conf::a7_1_rollback_drops_the_replaced_lock` | R-1.3 |
| `conf::a7_1_rollback_past_the_history_is_no_history` | R-1.3 |
| `conf::a7_1_screen_cap_below_snapshot_plus_one_is_invalid_input` | R-12 |
| `conf::a7_1_screen_cap_equal_to_snapshot_plus_one_attaches` | R-12 |
| `conf::a8_1_capture_reserves_max_snapshot_bytes_at_begin` | R-12 |
| `conf::a8_1_failed_capture_releases_its_reservation` | R-12 |
| `conf::a8_1_k_captures_admitted_then_capture_limit` | R-12 |
| `conf::a8_1_polled_success_counts_total_bytes_and_polled_failure_counts_nothing` | R-12 |
| `conf::a8_1_refused_then_retry_after_a_failed_completion_is_polled` | R-12 |
| `conf::a8_1_refused_then_retry_after_a_successful_completion_is_polled` | R-12 |
| `conf::a8_1_reservation_becomes_total_bytes_on_completion` | R-12 |
| `conf::a8_1_unpolled_success_and_failure_still_count_max` | R-12 |
| `conf::a8_2_baseline_inside_an_uncarriable_sequence_closes_route_snapshot_too_large` | R-12 |
| `conf::a8_2_capture_inside_an_uncarriable_sequence_is_snapshot_too_large` | R-12 |
| `conf::a8_2_offered_snapshots_still_satisfy_the_resume_invariant` | R-12 |
| `conf::a8_2_resync_inside_an_uncarriable_sequence_closes_route_snapshot_too_large` | R-12 |
| `conf::a9_3_ineligible_payload_is_never_compressed` | R-12 |
| `conf::a9_3_scripted_compression_choice_is_the_workers_only_decision` | R-12 |
| `conf::ad_1_running_adopts_running` | R-12 |
| `conf::ad_1_starting_with_identity_adopts` | R-12 |
| `conf::ad_1_starting_without_identity_is_lost` | R-12 |
| `conf::ad_1_stopping_resumes_stop` | R-12 |
| `conf::ad_2_lost_reasons` | R-12 |
| `conf::ad_2_lost_row_keeps_id_and_allows_retry` | R-12 |
| `conf::ad_3_adopted_equals_started` | R-12 |
| `conf::ad_4_absent_worker_protocol_is_unknown_never_compatible` | R-12 |
| `conf::ad_4_compatibility_is_not_a_guarantee_of_adoption_auth_and_reachability_can_still_fail` | R-12 |
| `conf::ad_4_unknown_version_is_lost` | R-12 |
| `conf::ad_6_reused_pid_is_never_killed` | R-12 |
| `conf::ad_6_token_and_instance_checked` | R-12 |
| `conf::ad_7_crash_between_steps_leaves_no_unregistered_payload` | R-12 |
| `conf::ad_7_payload_launches_after_durable_identity` | R-12 |
| `conf::ad_7_uncertain_registry_write_is_reported` | R-12 |
| `conf::am_2_next_write_waits_for_completion_or_partial_end` | R-74 |
| `conf::am_2_no_interleave_across_paste_markers` | R-5 |
| `conf::am_3_exactly_one_completion` | R-12 |
| `conf::am_4_capacity_returns_on_poll` | R-12 |
| `conf::as_1_asset_is_immutable` | R-1.3 |
| `conf::as_1_lookup_by_name_finds_staged_and_published` | R-1.3 |
| `conf::as_1_page_is_one_self_contained_asset` | R-1.3 |
| `conf::as_1_publish_returns_an_opaque_asset_id` | R-1.3 |
| `conf::as_1_publish_under_an_existing_name_repoints_it` | R-1.3 |
| `conf::as_1_quota_bounded` | R-1.3 |
| `conf::as_1_staged_assets_loaded_without_execution` | R-1.3 |
| `conf::as_2_https_app_embeds_loopback_assets_origin` | R-1.3 |
| `conf::as_2_name_resolves_to_newest_asset_id_at_open` | R-1.3 |
| `conf::as_2_no_token_is_404` | R-1.3 |
| `conf::as_2_non_loopback_http_assets_origin_refused` | R-1.3 |
| `conf::as_2_private_network_access_header_for_the_app_origin_only` | R-1.3 |
| `conf::as_2_renewal_by_asking_again` | R-1.3 |
| `conf::as_2_served_only_from_the_assets_origin` | R-1.3 |
| `conf::as_2_token_expires` | R-1.3 |
| `conf::as_2_token_is_not_an_app_credential` | R-1.3 |
| `conf::as_2_token_minted_only_for_an_authorized_client_request` | R-1.3 |
| `conf::as_2_token_scoped_to_one_asset` | R-1.3 |
| `conf::as_3_content_type_from_the_allowed_set` | R-1.3 |
| `conf::as_3_csp_sandbox_nosniff_no_store_no_cookie` | R-1.3 |
| `conf::as_3_frame_has_no_direct_hub_call_but_client_mediated_actions_exist` | R-1.3 |
| `conf::as_3_inline_script_and_style_run_and_nothing_loads_externally` | R-1.3 |
| `conf::as_3_navigation_is_not_claimed_to_be_confined` | R-1.3 |
| `conf::as_3_referrer_policy_no_referrer` | R-1.3 |
| `conf::as_4_assets_deleted_on_disable_uninstall_quarantine` | R-1.3 |
| `conf::as_4_no_cross_plugin_access` | R-1.3 |
| `conf::as_4_reload_replaces_at_handover` | R-1.3 |
| `conf::as_4_runtime_assets_survive_crash_reload_and_hub_restart` | R-1.3 |
| `conf::bd_3_client_uses_only_welcome_limits` | R-1.4 |
| `conf::ca_13_sanitized_reads` | R-1.3 |
| `conf::ca_14_context_delivered_as_defined_by_client_mc1c` | R-1.3 |
| `conf::ca_15_lost_reply_keeps_session` | R-1.3 |
| `conf::ca_16_expired_receipt_is_not_found` | R-1.3 |
| `conf::ca_16_operation_record_is_discriminated` | R-1.3 |
| `conf::ca_16_safe_write_get_after_hub_restart_is_unknown` | R-1.3 |
| `conf::ca_16_unknown_kind_waits_for_terminal` | R-1.4 |
| `conf::ca_17_cancel_pending_write` | R-1.3 |
| `conf::ca_18_client_input_refused_not_writable` | R-1.3 |
| `conf::ca_18_noninteractive_has_no_client_input_route` | R-1.3 |
| `conf::ca_18_plugin_can_write_to_noninteractive` | R-1.3 |
| `conf::ca_19_env_secrets_not_logged` | R-1.3 |
| `conf::ca_19_needs_its_own_scope` | R-1.3 |
| `conf::ca_19_runs_exact_argv_no_shell` | R-1.3 |
| `conf::ca_20_adoption_restores_port_reservations_first` | R-1.3 |
| `conf::ca_20_legacy_port_forward_marker_maps_to_allocate_port` | R-1.3 |
| `conf::ca_20_no_two_live_sessions_share_a_port` | R-1.3 |
| `conf::ca_20_port_allocated_before_start_and_in_env` | R-1.3 |
| `conf::ca_20_port_env_rejects_reserved_and_invalid_names` | R-1.3 |
| `conf::ca_20_port_released_at_run_end` | R-1.3 |
| `conf::ca_21_delete_checks_occupancy_atomically_with_reservations` | R-1.3 |
| `conf::ca_21_delete_own_vs_any` | R-1.3 |
| `conf::ca_21_owner_is_the_creator` | R-1.3 |
| `conf::ca_21_reuse_does_not_transfer_ownership` | R-1.3 |
| `conf::ca_22_empty_patterns_match_nothing` | R-1.3 |
| `conf::ca_22_matching_tests_notification_text_and_bell_never_matches` | R-1.3 |
| `conf::ca_22_missing_policy_is_all` | R-1.3 |
| `conf::ca_22_no_plugin_code_on_the_notice_path` | R-1.3 |
| `conf::ca_22_notifications_declared_and_validated` | R-1.3 |
| `conf::ca_22_patterns_only_with_matching` | R-1.3 |
| `conf::ca_9_sink_never_blocks` | R-1.3 |
| `conf::dp_10_t1_keystroke_to_pty_latency_budget` | R-12 |
| `conf::dp_10_t1_pty_to_client_latency_budget` | R-12 |
| `conf::dp_10_t1_throughput_and_byte_identity` | R-12 |
| `conf::dp_11_hop_counts_match_the_stated_topologies` | R-12 |
| `conf::dp_12_a_route_whose_latest_focus_is_false_causes_nothing` | R-12 |
| `conf::dp_12_focus_changed_is_reported_once_at_adoption_with_the_current_value` | R-12 |
| `conf::dp_12_focus_out_only_when_focus_reporting_is_on` | R-12 |
| `conf::dp_12_focus_report_capability_is_advertised_per_worker` | R-12 |
| `conf::dp_12_focus_report_is_independent_of_focus_reporting` | R-12 |
| `conf::dp_12_focus_state_change_posts_focus_changed` | R-12 |
| `conf::dp_12_last_focused_route_closing_writes_focus_out` | R-7 |
| `conf::dp_12_no_focus_out_while_another_route_is_focused` | R-7 |
| `conf::dp_12_no_route_ever_is_authoritatively_false` | R-12 |
| `conf::dp_12_terminal_state_focused_matches_the_event` | R-12 |
| `conf::dp_12_unreachable_or_n_minus_1_worker_leaves_focused_unknown` | R-12 |
| `conf::dp_1_host_process_never_sees_route_bytes` | R-12 |
| `conf::dp_2_failed_handoff_closes_route_handoff_failed` | R-12 |
| `conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy` | R-12 |
| `conf::dp_3_data_frames_are_raw` | R-12 |
| `conf::dp_3_frame_limit_checked_before_allocation` | R-12 |
| `conf::dp_3_output_deflate_frames_share_no_state` | R-12 |
| `conf::dp_3_output_deflate_inflated_payload_is_at_most_the_frame_bound_minus_the_type_byte` | R-12 |
| `conf::dp_3_output_deflate_inflates_to_the_exact_pty_bytes_in_order` | R-12 |
| `conf::dp_3_route_codec_matches_client_appendix_a4_a5` | R-12 |
| `conf::dp_3_route_limits_reject_a_screen_cap_below_snapshot_plus_one` | R-12 |
| `conf::dp_3_screen_is_one_frame_within_max_screen_frame_bytes` | R-12 |
| `conf::dp_3_unknown_type_closes_route` | R-12 |
| `conf::dp_4_worker_encodes_route_frames_with_write_time_modes` | R-7 |
| `conf::dp_5_abort_before_commit_writes_nothing_and_sends_no_frame` | R-12 |
| `conf::dp_5_bytes_and_text_reach_the_pty_exactly` | R-12 |
| `conf::dp_5_confirmed_paste_resend_is_a_new_op_with_require_bracketed_false` | R-12 |
| `conf::dp_5_every_not_written_reason_maps_to_a_refusal_or_no_frame` | R-12 |
| `conf::dp_5_exception_frames_wait_in_the_route_queue_and_full_queue_stops_input_reads` | R-12 |
| `conf::dp_5_full_input_queue_stops_reading_the_route_and_never_drops` | R-12 |
| `conf::dp_5_input_before_live_is_refused_not_ready` | R-12 |
| `conf::dp_5_input_done_answers_only_want_result_paste_and_file_commit` | R-12 |
| `conf::dp_5_input_done_only_for_want_result_true` | R-12 |
| `conf::dp_5_invalid_utf8_text_is_refused_unsupported_not_route_closing` | R-12 |
| `conf::dp_5_lane_full_is_not_a_route_reason` | R-12 |
| `conf::dp_5_mode_precondition_zero_effect_reported_on_the_route` | R-12 |
| `conf::dp_5_modes_coalesce_to_one_latest_unstarted_frame` | R-12 |
| `conf::dp_5_no_input_has_a_success_acknowledgement` | R-12 |
| `conf::dp_5_not_reported_sends_no_frame` | R-12 |
| `conf::dp_5_op_is_optional_except_on_a_paste` | R-12 |
| `conf::dp_5_paste_is_assembled_and_written_contiguously_at_commit` | R-12 |
| `conf::dp_5_refusals_survive_a_resync` | R-12 |
| `conf::dp_5_route_op_namespace_is_separate_from_host_opids` | R-12 |
| `conf::dp_5_session_ended_is_a_reason_with_written_bytes_after_a_partial_paste` | R-12 |
| `conf::dp_5_silent_not_reported_is_omitted_not_written` | R-12 |
| `conf::dp_5_unstarted_terminal_query_frame_retires_when_the_query_resolves` | R-12 |
| `conf::dp_5b_abort_or_route_end_leaves_nothing` | R-12 |
| `conf::dp_5b_commit_is_answered_with_input_done_and_path` | R-12 |
| `conf::dp_5b_creation_failure_writes_no_path_and_leaves_no_file` | R-12 |
| `conf::dp_5b_files_are_deleted_at_remove` | R-12 |
| `conf::dp_5b_name_is_sanitized_unique_and_inside_the_directory` | R-12 |
| `conf::dp_5b_other_input_flows_during_an_upload` | R-12 |
| `conf::dp_5b_over_limit_leaves_nothing` | R-12 |
| `conf::dp_5b_oversize_encoded_path_is_refused_before_the_file_is_created` | R-12 |
| `conf::dp_5b_path_is_followed_by_one_space_in_one_paste` | R-12 |
| `conf::dp_5b_path_is_pasted_unmodified_with_no_quoting` | R-12 |
| `conf::dp_5b_path_is_written_in_receive_order` | R-12 |
| `conf::dp_5b_path_paste_failure_carries_path_and_written_bytes` | R-12 |
| `conf::dp_5b_path_write_failure_reports_written_bytes_and_keeps_the_file` | R-12 |
| `conf::dp_5b_second_file_begin_is_refused` | R-12 |
| `conf::dp_5b_written_bytes_and_the_precreation_bound_include_the_space` | R-12 |
| `conf::dp_5b_written_bytes_are_encoded_pty_bytes_with_markers` | R-12 |
| `conf::dp_6_close_falls_back_zero_routes_keeps_size` | R-12 |
| `conf::dp_6_host_resize_and_policy_latest_wins` | R-12 |
| `conf::dp_6_latest_most_recent_activity_wins` | R-12 |
| `conf::dp_6_resize_has_no_answer_size_arrives_in_modes` | R-12 |
| `conf::dp_8_adopted_route_keeps_route_tag_owner_and_client_tags` | R-12 |
| `conf::dp_8_adoption_fences_the_previous_host` | R-12 |
| `conf::dp_8_old_epoch_commands_and_descriptors_are_refused` | R-12 |
| `conf::dp_8_route_adopted_event_per_live_route` | R-12 |
| `conf::dp_8_routes_survive_host_restart_and_bytes_keep_flowing` | R-12 |
| `conf::ds_4_major_mismatch_falls_back` | R-1.4 |
| `conf::e1_1_worker_found_by_host_supplied_path` | R-12 |
| `conf::e2_1_osc_0_and_2_give_title_changed` | R-12 |
| `conf::e2_1_osc_1_gives_no_title_changed_and_stays_in_output` | R-12 |
| `conf::e2_2_other_modes_are_tracked_model_modes` | R-12 |
| `conf::e2_2_unrecognized_mode_number_changes_no_field` | R-12 |
| `conf::e2_3_mode_change_posts_latest_flags` | R-12 |
| `conf::e2_3_one_step_net_changed_posts_final_flags` | R-12 |
| `conf::e2_3_one_step_net_unchanged_posts_nothing` | R-12 |
| `conf::e2_3_write_start_uses_current_model_modes` | R-12 |
| `conf::e3_1_carried_steps_run_first_in_due_order` | R-12 |
| `conf::e3_1_carried_transition_into_a_full_mandatory_queue_is_parked_and_woken_on_room` | R-12 |
| `conf::e3_1_due_effect_without_event_runs_in_its_pump_with_no_budget` | R-12 |
| `conf::e3_1_due_step_beyond_budget_is_carried_with_more` | R-12 |
| `conf::e3_1_eventless_effect_overtakes_a_carried_or_parked_step` | R-12 |
| `conf::e3_1_runnable_silent_bypasses_a_transition_parked_on_queue_room` | R-12 |
| `conf::e3_1_two_silences_due_together_with_budget_one` | R-12 |
| `conf::e4_1_client_reply_reserves_its_frame_size` | R-12 |
| `conf::e4_1_fallback_reserves_its_reply_length` | R-12 |
| `conf::e4_1_reply_one_byte_over_the_free_bound_is_parked` | R-12 |
| `conf::e6_1_reply_from_another_route_is_wrong_route_and_the_query_stays_open` | R-12 |
| `conf::en_6_unknown_optional_ignored` | R-1.4 |
| `conf::ev_10_core_ingest_independent_of_plugins` | R-1.3 |
| `conf::ev_10_slow_subscriber_does_not_stall_others` | R-1.3 |
| `conf::ev_10_slow_subscriber_does_not_stall_the_hub` | R-1.3 |
| `conf::ev_11_high_emit_needs_permission` | R-1.3 |
| `conf::ev_11_no_producer_rate_gate` | R-1.3 |
| `conf::ev_12_descriptor_is_read_only_existing_surface` | R-1.3 |
| `conf::ev_12_events_hub_without_session_read_is_unavailable` | R-1.3 |
| `conf::ev_12_gap_names_recovery_state_or_unavailable` | R-1.3 |
| `conf::ev_12_high_overflow_records_ledger_entry` | R-1.3 |
| `conf::ev_12_marker_needs_no_queue_room` | R-1.3 |
| `conf::ev_12_no_silent_high_loss` | R-1.3 |
| `conf::ev_12_other_plugin_high_event_gap_is_unavailable` | R-1.3 |
| `conf::ev_12_partial_gap_has_no_descriptor` | R-1.3 |
| `conf::ev_12_published_state_source_makes_recovery_state_only_with_the_grant` | R-1.3 |
| `conf::ev_12_resync_after_gap` | R-1.3 |
| `conf::ev_12_state_gap_carries_actionable_state_read` | R-1.3 |
| `conf::ev_12_state_source_creates_no_cross_plugin_api` | R-1.3 |
| `conf::ev_13_aggregated_gap_is_recovery_partial` | R-1.3 |
| `conf::ev_13_ledger_aggregates_exactly_when_full` | R-1.3 |
| `conf::ev_13_ledger_is_bounded` | R-1.3 |
| `conf::ev_13_mixed_unit_aggregation_keeps_events_and_bytes_separate` | R-1.3 |
| `conf::ev_14_exit_is_high` | R-1.3 |
| `conf::ev_14_modes_event_keyed_by_session` | R-1.3 |
| `conf::ev_14_one_core_feed_per_session` | R-1.3 |
| `conf::ev_14_tap_default_own_sessions_any_needs_approval` | R-1.3 |
| `conf::ev_14_tap_is_output_only` | R-1.3 |
| `conf::ev_14_tap_loss_marker_counts_bytes` | R-1.3 |
| `conf::ev_14_tap_needs_its_own_scope` | R-1.3 |
| `conf::ev_14_terminal_metadata_events` | R-1.3 |
| `conf::ev_14_title_cwd_coalesce` | R-1.3 |
| `conf::ev_15_event_invoke_shape` | R-1.3 |
| `conf::ev_15_failed_event_handler_not_retried` | R-1.3 |
| `conf::ev_15_gap_bypasses_filters` | R-1.3 |
| `conf::ev_15_gap_retained_when_handler_fails` | R-1.3 |
| `conf::ev_15_gap_retained_when_invoke_queue_full` | R-1.3 |
| `conf::ev_15_gap_uses_registered_handler` | R-1.3 |
| `conf::ev_15_loss_during_gap_handling_is_a_new_obligation` | R-1.3 |
| `conf::ev_15_no_gap_handler_refused_at_load` | R-1.3 |
| `conf::ev_15_retry_keeps_gap_id_and_summary` | R-1.3 |
| `conf::ev_1_emit_declared_only` | R-1.3 |
| `conf::ev_1_metadata_reaches_host` | R-12 |
| `conf::ev_1_refusal_is_returned_not_raised` | R-1.3 |
| `conf::ev_1_replaced_keyed_event_moves_to_latest_position` | R-12 |
| `conf::ev_1_stamped_fields_supplied_by_plugin_refused` | R-1.3 |
| `conf::ev_1_subject_and_key_optional_and_bounded` | R-1.3 |
| `conf::ev_2_drop_oldest_with_marker` | R-12 |
| `conf::ev_2_each_subscription_names_a_registered_handler` | R-1.3 |
| `conf::ev_2_gap_markers_never_name_private_events` | R-1.3 |
| `conf::ev_2_grant_check_and_audience_check_are_distinct` | R-1.3 |
| `conf::ev_2_mandatory_and_keyed_never_dropped` | R-12 |
| `conf::ev_2_min_priority_order_and_default` | R-1.3 |
| `conf::ev_2_one_marker_per_session` | R-12 |
| `conf::ev_2_pattern_grammar` | R-1.3 |
| `conf::ev_2_pattern_wider_than_grant_refused_at_load` | R-1.3 |
| `conf::ev_2_producer_reload_private_name_never_matches_wildcard` | R-1.3 |
| `conf::ev_2_retired_instance_never_creates_a_loss_marker` | R-12 |
| `conf::ev_2_source_permission_checked` | R-1.3 |
| `conf::ev_2_subjects_filter_narrows_session_events_only` | R-1.3 |
| `conf::ev_2_subscriber_cannot_block_producer_registration` | R-1.3 |
| `conf::ev_2_subscriber_identity_recorded` | R-1.3 |
| `conf::ev_3_clipboard_write_surfaced` | R-12 |
| `conf::ev_3_oversize_write_is_reported_not_dropped` | R-12 |
| `conf::ev_3_status_maps_to_kind` | R-1.3 |
| `conf::ev_4_exit_has_signal` | R-12 |
| `conf::ev_5_emitter_not_blocked` | R-1.3 |
| `conf::ev_5_full_queue_defers_transition_not_output` | R-12 |
| `conf::ev_5_keyed_events_bounded_by_keys` | R-12 |
| `conf::ev_5_no_mandatory_event_lost_or_replaced` | R-12 |
| `conf::ev_5_poll_wakes_pending_work` | R-12 |
| `conf::ev_5_released_retires_keyed_droppable_and_loss_events` | R-12 |
| `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded` | R-12 |
| `conf::ev_5_route_closed_retires_route_keyed_events` | R-12 |
| `conf::ev_5_service_released_retires_service_keyed_events` | R-12 |
| `conf::ev_5_slot_held_until_polled` | R-12 |
| `conf::ev_5_slow_subscriber_does_not_stall_others` | R-1.3 |
| `conf::ev_5_stop_kills_while_queue_full` | R-12 |
| `conf::ev_6_gap_forces_baseline` | R-1.3 |
| `conf::ev_6_keyed_event_carries_latest` | R-12 |
| `conf::ev_6_loss_leaves_marker_then_state_is_readable` | R-12 |
| `conf::ev_6_session_family_baseline_then_deltas` | R-1.3 |
| `conf::ev_7_consumer_before_producer` | R-1.3 |
| `conf::ev_7_each_terminal_feature_reaches_the_host` | R-12 |
| `conf::ev_7_event_and_output_both_carry_the_sequence` | R-12 |
| `conf::ev_7_every_mode_change_posts_modes_changed` | R-12 |
| `conf::ev_7_hyperlink_in_output_and_snapshot` | R-12 |
| `conf::ev_7_no_feature_needs_opt_in` | R-12 |
| `conf::ev_7_producer_reload_keeps_subscription` | R-1.3 |
| `conf::ev_8_a_sent_or_started_query_is_not_affected_by_a_resync` | R-12 |
| `conf::ev_8_admission_reserves_full_bytes` | R-12 |
| `conf::ev_8_attach_requires_a_positive_query_deadline` | R-12 |
| `conf::ev_8_client_answers_first_shadow_suppressed` | R-12 |
| `conf::ev_8_client_decline_makes_shadow_answer_at_once` | R-12 |
| `conf::ev_8_client_reply_after_a_resync_is_rejected_query_expired` | R-12 |
| `conf::ev_8_client_silent_shadow_answers_at_deadline` | R-12 |
| `conf::ev_8_core_has_no_other_query_timer` | R-12 |
| `conf::ev_8_decline_does_not_consume_the_query_id_before_fallback_admission` | R-12 |
| `conf::ev_8_encoded_reply_is_bounded_and_once` | R-12 |
| `conf::ev_8_every_query_stays_in_output` | R-12 |
| `conf::ev_8_exactly_one_reply_under_every_interleaving` | R-12 |
| `conf::ev_8_fallback_started_means_admitted_refused_fallback_stays_pending` | R-12 |
| `conf::ev_8_full_query_capacity_backpressures_parser_in_order` | R-12 |
| `conf::ev_8_hub_events_sanitized` | R-1.3 |
| `conf::ev_8_late_client_reply_is_refused` | R-12 |
| `conf::ev_8_no_route_shadow_answers_at_once` | R-12 |
| `conf::ev_8_open_query_survives_host_restart` | R-12 |
| `conf::ev_8_oversize_query_answered_by_shadow_with_marker` | R-12 |
| `conf::ev_8_query_goes_to_latest_route_with_host_deadline` | R-12 |
| `conf::ev_8_refused_fallback_admission_stays_on_the_fallback_path_only` | R-12 |
| `conf::ev_8_replies_never_split_a_paste_and_keep_query_order` | R-12 |
| `conf::ev_8_reply_after_decline_is_query_expired_then_already_replied` | R-12 |
| `conf::ev_8_reply_does_not_move_input_rev` | R-12 |
| `conf::ev_8_resync_before_answer_ends_the_opportunity_and_uses_the_shadow` | R-12 |
| `conf::ev_8_resync_never_lets_a_client_answer_from_stale_state` | R-12 |
| `conf::ev_8_route_close_falls_back_to_shadow` | R-12 |
| `conf::ev_8_set_color_profile_changes_later_shadow_answers` | R-12 |
| `conf::ev_8_shadow_answers_colors_from_the_host_supplied_profile_with_no_client` | R-12 |
| `conf::ev_8_shadow_cannot_answer_and_client_silent_means_no_reply` | R-12 |
| `conf::ev_8_shadow_reply_is_computed_at_the_query_point` | R-12 |
| `conf::ev_8_unstarted_query_frame_is_retired_by_a_resync` | R-12 |
| `conf::ev_8_worker_never_writes_before_client_opportunity_ends` | R-12 |
| `conf::ev_9_events_hub_alone_never_grants_session_notification` | R-1.3 |
| `conf::ev_9_every_session_event_carries_instance_id` | R-12 |
| `conf::ev_9_hub_events_delivered` | R-1.3 |
| `conf::ev_9_hub_wildcard_does_not_match_session_notification_without_events_terminal` | R-1.3 |
| `conf::ev_9_plugin_to_plugin_event` | R-1.3 |
| `conf::ev_9_policy_applied_once_by_the_hub_and_no_duplicate_for_attached_clients` | R-1.3 |
| `conf::ev_9_restarted_source_has_new_instance` | R-1.3 |
| `conf::ev_9_session_notification_filtered_by_the_session_type_policy` | R-1.3 |
| `conf::ev_9_session_notification_needs_events_terminal_for_plugins` | R-1.3 |
| `conf::ev_9_session_notification_reaches_clients` | R-1.3 |
| `conf::ev_9_source_and_instance_on_every_event` | R-1.3 |
| `conf::ev_9_target_scoped_instance_receives_only_its_target_notifications` | R-1.3 |
| `conf::fs_1_no_absolute_path_reaches_plugin` | R-1.3 |
| `conf::fs_1_root_bound_by_operator` | R-1.3 |
| `conf::fs_2_symlink_replaced_after_validation_never_escapes` | R-1.3 |
| `conf::fs_3_rearm_delivers_changed_once` | R-1.3 |
| `conf::fs_3_unload_cancels_watches` | R-1.3 |
| `conf::fs_3_watch_is_level_triggered_like_st6` | R-1.3 |
| `conf::in_2_link_loss_is_unknown_not_zero` | R-74 |
| `conf::in_2_partial_is_certain` | R-74 |
| `conf::in_6_cancel_reports_progress` | R-74 |
| `conf::in_6_refused_full_wakes_on_room` | R-74 |
| `conf::in_7_lost_worker_ends_pending` | R-12 |
| `conf::in_9_application_cursor_and_keypad` | R-5 |
| `conf::in_9_key_encoded_by_worker_modes_at_write` | R-5 |
| `conf::in_9_key_shifted_and_alternate_keys_omission_rule` | R-5 |
| `conf::in_9_kitty_all_32_flag_combinations_pinned` | R-5 |
| `conf::in_9_kitty_each_flag` | R-5 |
| `conf::in_9_kitty_flag_16_needs_flag_8` | R-5 |
| `conf::in_9_kitty_modifier_keys_need_flag_8` | R-5 |
| `conf::in_9_kitty_text_with_control_codepoint_omitted` | R-5 |
| `conf::in_9_kitty_unknown_flag_bits_ignored` | R-5 |
| `conf::in_9_legacy_keys_table` | R-5 |
| `conf::in_9_legacy_release_is_not_reported` | R-5 |
| `conf::in_9_modes_read_at_write_start` | R-74 |
| `conf::in_9_mouse_each_encoding_and_limits` | R-5 |
| `conf::in_9_mouse_each_tracking_mode` | R-5 |
| `conf::in_9_non_us_layout_uses_supplied_text` | R-5 |
| `conf::lb_10_descriptors_per_kind` | R-1.4 |
| `conf::lb_10_fs_watch_handler_id_stays_mandatory` | R-1.4 |
| `conf::lb_10_handler_kind_strings_and_payloads` | R-1.4 |
| `conf::lb_10_register_builds_ready_and_refuses_after_ready` | R-1.4 |
| `conf::lb_10_timer_and_watch_handler_id_dispatch` | R-1.4 |
| `conf::lb_11_http_and_ws_are_async_operations` | R-1.4 |
| `conf::lb_11_http_method_set_and_header_pairs` | R-1.4 |
| `conf::lb_12_observation_event_grants` | R-1.4 |
| `conf::lb_12_session_calls_and_scopes` | R-1.4 |
| `conf::lb_12_wrong_state_has_code_and_reason` | R-1.4 |
| `conf::lb_13_operation_record_shape` | R-1.4 |
| `conf::lb_13_unknown_kind_or_state_waits_for_terminal` | R-1.4 |
| `conf::lb_14_process_and_plugin_call_async_and_typed` | R-1.4 |
| `conf::lb_15_files_assets_web_relay_shapes` | R-1.4 |
| `conf::lb_16_other_call_shapes` | R-1.4 |
| `conf::lb_16_write_result_vs_error_mapping` | R-1.4 |
| `conf::lb_16b_metadata_replaces_namespace_and_null_removes` | R-1.4 |
| `conf::lb_16b_update_and_spawn_arguments_match_the_hp_rows` | R-1.4 |
| `conf::lb_17_dollar_schema_accepted_and_ignored` | R-1.4 |
| `conf::lb_17_manifest_keys_closed` | R-1.4 |
| `conf::lb_17b_all_lua_files_under_the_root_are_staged` | R-1.4 |
| `conf::lb_17b_entry_relative_to_the_package_root` | R-1.4 |
| `conf::lb_17b_require_name_to_file_rule` | R-1.4 |
| `conf::lb_18_definition_and_bindings` | R-1.4 |
| `conf::lb_18_hello_and_ready_fields` | R-1.4 |
| `conf::lb_19_accessory_is_role_accessory` | R-1.4 |
| `conf::lb_19_agent_definition_fields` | R-1.4 |
| `conf::lb_19_script_file_relative_to_the_package_root` | R-1.4 |
| `conf::lb_1_binding_covers_every_call` | R-1.4 |
| `conf::lb_20_batch_can_create` | R-1.4 |
| `conf::lb_20_delete_needs_expected_revision` | R-1.4 |
| `conf::lb_21_config_events_shapes` | R-1.4 |
| `conf::lb_21_route_ended_is_client_only` | R-1.3 |
| `conf::lb_2_unknown_argument_key_is_invalid_request` | R-1.4 |
| `conf::lb_3_helpers_return_envelopes_never_raise` | R-1.4 |
| `conf::lb_4_handler_arguments` | R-1.4 |
| `conf::lb_5_caller_and_reference_spelling` | R-1.4 |
| `conf::lb_5_operator_credential_id_is_optional` | R-1.4 |
| `conf::lb_5_plugin_caller_variant_is_preserved` | R-1.4 |
| `conf::lb_6_events_on_is_load_time_only` | R-1.4 |
| `conf::lb_6_session_family_frames` | R-1.4 |
| `conf::lb_7_operation_notification_shape` | R-1.4 |
| `conf::lb_8_bytes_round_trip_base64` | R-1.4 |
| `conf::lb_9_empty_array_encodes_as_array` | R-1.4 |
| `conf::lb_9_json_null_sentinel` | R-1.4 |
| `conf::lba1_1_empty_array_is_a_read_only_value` | R-1.4 |
| `conf::lba1_1_json_array_returns_the_array_not_an_envelope` | R-1.4 |
| `conf::lba1_2_conversion_values_are_bare` | R-1.4 |
| `conf::lba1_3_entity_provider_kind_is_written_entity_kind` | R-1.4 |
| `conf::lba2_1_handler_field_in_lua_fails_the_load` | R-1.4 |
| `conf::lba2_1_prompt_and_resource_entries` | R-1.4 |
| `conf::lba2_1_tool_entry_is_name_description_input_schema_call` | R-1.4 |
| `conf::lc_12_stop_all_keeps_final_model_workers_until_remove` | R-12 |
| `conf::lc_3_remove_created` | R-12 |
| `conf::lc_7_remove_created` | R-12 |
| `conf::lc_7_remove_order_and_completion` | R-12 |
| `conf::lc_9_worker_protocol_absent_for_an_unreachable_worker_whose_hello_was_never_read` | R-12 |
| `conf::lh_1_local_helpers_send_nothing` | R-1.4 |
| `conf::lh_4_globals_equal_published_set` | R-1.4 |
| `conf::mp_10_fake_mcp_server_is_scripted_and_offline` | R-1.3 |
| `conf::mp_10_real_run_list_call_cancel_unload` | R-1.3 |
| `conf::mp_11_checked_by_the_net_allowlist` | R-1.3 |
| `conf::mp_11_optional_get_stream_for_notifications` | R-1.3 |
| `conf::mp_11_session_id_and_version_headers` | R-1.3 |
| `conf::mp_11_streamable_http_post_json_and_sse` | R-1.3 |
| `conf::mp_12_descriptor_count_and_bytes_bounded` | R-1.3 |
| `conf::mp_12_endless_pages_do_not_grow_hub_memory` | R-1.3 |
| `conf::mp_12_init_and_list_deadlines` | R-1.3 |
| `conf::mp_12_message_bytes_bounded` | R-1.3 |
| `conf::mp_12_page_count_bounded` | R-1.3 |
| `conf::mp_13_annotations_never_authorize` | R-1.3 |
| `conf::mp_13_descriptor_fields_carried_unchanged` | R-1.3 |
| `conf::mp_13_structured_content_passes_through` | R-1.3 |
| `conf::mp_13_uncarriable_descriptor_omitted` | R-1.3 |
| `conf::mp_14_old_generation_response_discarded` | R-1.3 |
| `conf::mp_14_refresh_swaps_the_whole_list_atomically` | R-1.3 |
| `conf::mp_14_removed_tool_not_dispatched_after_swap` | R-1.3 |
| `conf::mp_15_dispatched_call_not_replayed_after_restart` | R-1.3 |
| `conf::mp_15_unsent_call_is_safe_to_retry` | R-1.3 |
| `conf::mp_1_bad_combination_fails_load` | R-1.3 |
| `conf::mp_1_declaration_fields` | R-1.3 |
| `conf::mp_1_stdio_needs_its_own_scope` | R-1.3 |
| `conf::mp_2_negotiates_the_mc0_revision` | R-1.3 |
| `conf::mp_2_no_acceptable_revision_is_unavailable` | R-1.3 |
| `conf::mp_2_tools_only` | R-1.3 |
| `conf::mp_3_child_dies_with_the_bridge_group` | R-1.3 |
| `conf::mp_3_child_has_no_core_credential_or_env` | R-1.3 |
| `conf::mp_3_http_server_checked_against_net_allowlist` | R-1.3 |
| `conf::mp_3_nothing_shared_across_plugins` | R-1.3 |
| `conf::mp_3_stdio_runs_behind_a_bridge_service` | R-1.3 |
| `conf::mp_3_stdio_started_per_plugin_with_scrubbed_env` | R-1.3 |
| `conf::mp_4_allow_tools_narrows` | R-1.3 |
| `conf::mp_4_collision_fails_that_server_only` | R-1.3 |
| `conf::mp_4_double_underscore_in_key_server_or_direct_tool_refused` | R-1.3 |
| `conf::mp_4_external_tool_name_preserved_exactly` | R-1.3 |
| `conf::mp_4_full_name_set_validated_before_publication` | R-1.3 |
| `conf::mp_4_later_loader_fails_on_collision` | R-1.3 |
| `conf::mp_4_list_changed_refreshes` | R-1.3 |
| `conf::mp_4_namespaced_tool_names` | R-1.3 |
| `conf::mp_4_routes_by_stored_mapping_never_by_splitting` | R-1.3 |
| `conf::mp_4_underscore_ambiguity_collides_by_full_name` | R-1.3 |
| `conf::mp_5_call_authorized_with_verified_caller` | R-1.3 |
| `conf::mp_5_caller_in_meta_without_credentials` | R-1.3 |
| `conf::mp_5_caller_supplied_meta_overwritten` | R-1.3 |
| `conf::mp_5_hub_forwards_no_credential_or_session_authority` | R-1.3 |
| `conf::mp_5_plugin_process_not_in_call_path` | R-1.3 |
| `conf::mp_5_session_caller_not_widened` | R-1.3 |
| `conf::mp_6_concurrency_bounded` | R-1.3 |
| `conf::mp_6_deadline_cancels_with_notification` | R-1.3 |
| `conf::mp_6_oversize_result_is_response_too_large` | R-1.3 |
| `conf::mp_6_server_error_passes_through` | R-1.3 |
| `conf::mp_6_timeout_after_dispatch_is_effect_unknown` | R-1.3 |
| `conf::mp_7_reload_handover_closes_old_servers` | R-1.3 |
| `conf::mp_7_servers_start_at_activated_only` | R-1.3 |
| `conf::mp_7_unload_stops_servers_and_closes_connections` | R-1.3 |
| `conf::mp_8_failing_server_marked_unavailable` | R-1.3 |
| `conf::mp_8_restart_is_bounded` | R-1.3 |
| `conf::mp_8_slow_server_does_not_delay_others` | R-1.3 |
| `conf::mp_9_descriptor_containing_secret_omitted` | R-1.3 |
| `conf::mp_9_secret_never_in_hub_output` | R-1.3 |
| `conf::mp_9_stderr_tail_bounded_and_redacted` | R-1.3 |
| `conf::mt_1_name_with_double_underscore_fails_load` | R-1.3 |
| `conf::mt_1_register_tool_prompt_resource` | R-1.3 |
| `conf::mt_2_name_collision_fails_load` | R-1.3 |
| `conf::mt_3_list_reflects_loaded` | R-1.3 |
| `conf::mt_4_caller_is_verified` | R-1.3 |
| `conf::mt_4_schema_checked_before_invoke` | R-1.3 |
| `conf::mt_5_error_result_shape` | R-1.3 |
| `conf::mt_5_oversize_result_is_response_too_large_not_truncated` | R-1.3 |
| `conf::mt_6_session_argument_is_a_reference` | R-1.3 |
| `conf::mt_7_mcp_proxy_feature_advertised` | R-1.3 |
| `conf::nt_10_fake_io_is_scripted_and_offline` | R-1.3 |
| `conf::nt_10_telegram_shaped_long_poll_and_send` | R-1.3 |
| `conf::nt_10_unload_mid_request_cancels_and_leaks_nothing` | R-1.3 |
| `conf::nt_1_no_descriptor_or_ip_reaches_plugin` | R-1.3 |
| `conf::nt_1_plugin_has_no_sockets` | R-1.3 |
| `conf::nt_2_allow_grammar_and_malformed_entry_fails_load` | R-1.3 |
| `conf::nt_2_host_and_authority_override_refused` | R-1.3 |
| `conf::nt_2_origin_outside_allowlist_denied` | R-1.3 |
| `conf::nt_2_private_ranges_refused_without_net_private` | R-1.3 |
| `conf::nt_2_rebinding_checked_at_connect` | R-1.3 |
| `conf::nt_2_redirect_rechecked` | R-1.3 |
| `conf::nt_2_userinfo_refused` | R-1.3 |
| `conf::nt_2_ws_scheme_maps_to_allow_origin` | R-1.3 |
| `conf::nt_3_bodies_bounded` | R-1.3 |
| `conf::nt_3_cancel_ends_request` | R-1.3 |
| `conf::nt_3_does_not_hold_handler_slot` | R-1.3 |
| `conf::nt_3_request_is_an_async_operation` | R-1.3 |
| `conf::nt_3_timeout_bounded` | R-1.3 |
| `conf::nt_4_close_is_typed_high_priority` | R-1.3 |
| `conf::nt_4_messages_are_events_with_gap_marker` | R-1.3 |
| `conf::nt_4_open_is_an_async_operation` | R-1.3 |
| `conf::nt_4_send_backpressured_when_queue_full` | R-1.3 |
| `conf::nt_4_ws_events_delivered_only_to_owner` | R-1.3 |
| `conf::nt_4_ws_id_bound_to_plugin_and_instance` | R-1.3 |
| `conf::nt_4_ws_overflow_gap_recovery_unavailable` | R-1.3 |
| `conf::nt_5_cross_origin_redirect_drops_credentials_and_body` | R-1.3 |
| `conf::nt_5_echoed_secret_in_response_not_in_hub_output` | R-1.3 |
| `conf::nt_5_final_origin_rechecked_after_substitution` | R-1.3 |
| `conf::nt_5_get_own_declared_secret_only` | R-1.3 |
| `conf::nt_5_placeholder_not_allowed_in_host_scheme_port` | R-1.3 |
| `conf::nt_5_placeholder_substituted_by_hub` | R-1.3 |
| `conf::nt_5_put_only_plugin_writable_secret` | R-1.3 |
| `conf::nt_5_receipt_metadata_keeps_placeholders` | R-1.3 |
| `conf::nt_5_value_never_in_errors` | R-1.3 |
| `conf::nt_5_value_never_logged_or_in_events` | R-1.3 |
| `conf::nt_6_failing_endpoint_does_not_delay_hub` | R-1.3 |
| `conf::nt_6_no_shared_cookies_or_pool` | R-1.3 |
| `conf::nt_6_per_plugin_bounds` | R-1.3 |
| `conf::nt_6_slow_endpoint_does_not_delay_other_plugin` | R-1.3 |
| `conf::nt_7_body_bounded_and_queue_full_503` | R-1.3 |
| `conf::nt_7_caller_is_system` | R-1.3 |
| `conf::nt_7_handler_kind_needs_net_inbound` | R-1.3 |
| `conf::nt_7_handler_result_schema_and_timeout` | R-1.3 |
| `conf::nt_7_loopback_only` | R-1.3 |
| `conf::nt_7_routes_only_own_path` | R-1.3 |
| `conf::nt_7_unauthenticated_request_never_invokes` | R-1.3 |
| `conf::nt_7_webhook_auth_stripped_from_headers` | R-1.3 |
| `conf::nt_8_bootstrap_has_no_network` | R-1.3 |
| `conf::nt_8_crash_cancels_and_closes` | R-1.3 |
| `conf::nt_8_reload_old_completion_not_delivered_to_new` | R-1.3 |
| `conf::nt_8_unload_cancels_and_closes_everything` | R-1.3 |
| `conf::nt_9_each_error_is_distinguished` | R-1.3 |
| `conf::nt_9_effect_none_before_send` | R-1.3 |
| `conf::nt_9_error_leaks_no_secret_or_address` | R-1.3 |
| `conf::nt_9_timeout_after_send_is_unknown` | R-1.3 |
| `conf::or_2_session_order` | R-12 |
| `conf::ou_10_no_authorization_in_core` | R-12 |
| `conf::ou_11_transport_ownership_is_defined` | R-12 |
| `conf::ou_12_output_bytes_are_unmodified_and_in_order` | R-12 |
| `conf::ou_1_attach_hands_transport_to_worker` | R-12 |
| `conf::ou_1_attach_options_carry_no_paste_policy` | R-12 |
| `conf::ou_1_attached_features_are_the_intersection` | R-12 |
| `conf::ou_1_no_common_terminal_format_is_unsupported_before_reservation` | R-12 |
| `conf::ou_1_terminal_format_is_negotiated_against_the_target_worker` | R-12 |
| `conf::ou_1_view_only_route_ignores_input` | R-12 |
| `conf::ou_2_close_once_first_reason_wins` | R-12 |
| `conf::ou_2_route_state_machine` | R-12 |
| `conf::ou_2_stall_and_close_are_reported_to_the_host` | R-12 |
| `conf::ou_2_stall_close_after` | R-12 |
| `conf::ou_2b_every_core_close_reason_maps_to_one_wire_reason` | R-12 |
| `conf::ou_2b_failed_reasons_are_reported_to_the_host_and_route_ended` | R-12 |
| `conf::ou_2b_handoff_failure_is_reported_as_handoff_failed` | R-12 |
| `conf::ou_2b_healthy_reasons_send_route_closed_last` | R-12 |
| `conf::ou_3_other_readers_continue` | R-12 |
| `conf::ou_3_progress_is_bytes_accepted_by_the_kernel` | R-12 |
| `conf::ou_3_progressing_reader_lossless` | R-12 |
| `conf::ou_3_stalled_reader_resyncs` | R-12 |
| `conf::ou_4_partial_frame_is_completed_before_resync_or_close` | R-12 |
| `conf::ou_5_peer_fault_closes_only_that_route` | R-12 |
| `conf::ou_6_slow_route_does_not_delay_others` | R-12 |
| `conf::ou_6_spurious_readiness_is_harmless` | R-12 |
| `conf::ou_7_exit_after_last_frame_then_close` | R-12 |
| `conf::ou_7_exit_after_tail_queued_then_close` | R-12 |
| `conf::ou_7_progressing_slow_route_delays_exit_lossless` | R-12 |
| `conf::ou_7_stalled_route_does_not_hold_exit` | R-12 |
| `conf::ou_8_route_output_frame_kinds` | R-12 |
| `conf::ou_9_adopted_session_attaches_by_baseline` | R-12 |
| `conf::ou_9_attach_is_sync_baseline_in_pump` | R-12 |
| `conf::ou_9_baseline_then_live_no_gap` | R-12 |
| `conf::ou_9_resync_completes_a_partial_frame` | R-12 |
| `conf::ou_9_resync_drops_unstarted_frames` | R-12 |
| `conf::pc_1_addresses_an_interface_not_a_plugin` | R-1.3 |
| `conf::pc_1_call_is_an_async_operation_with_a_result` | R-1.3 |
| `conf::pc_1_callee_must_opt_in_per_tool` | R-1.3 |
| `conf::pc_1_cancel_after_dispatch_sends_cancel_effect_unknown` | R-1.3 |
| `conf::pc_1_cancel_before_dispatch_effect_none` | R-1.3 |
| `conf::pc_1_needs_a_grant_per_owner` | R-1.3 |
| `conf::pc_1_never_returning_tool_times_out_and_releases_edge_keeps_terminal_receipt` | R-1.3 |
| `conf::pc_1_old_instance_completion_never_reaches_new_instance` | R-1.3 |
| `conf::pc_2_arguments_cannot_forge_the_caller` | R-1.3 |
| `conf::pc_2_callee_sees_a_verified_plugin_caller` | R-1.3 |
| `conf::pc_2_identity_from_authenticated_instance` | R-1.3 |
| `conf::pc_3_cycle_refused_over_pending_edges` | R-1.3 |
| `conf::pc_3_depth_bounded` | R-1.3 |
| `conf::pc_3_edge_retired_at_terminal` | R-1.3 |
| `conf::pc_3_no_reentry` | R-1.3 |
| `conf::pc_3_queued_dispatch_one_handler_per_plugin` | R-1.3 |
| `conf::pc_3_trace_inherited_inside_handler` | R-1.3 |
| `conf::pc_4_first_binding_stored_with_the_receipt` | R-1.3 |
| `conf::pc_4_key_namespace_is_stable_across_provider_handover` | R-1.3 |
| `conf::pc_4_pending_record_fsynced_before_dispatch` | R-1.3 |
| `conf::pc_4_retry_after_compatible_provider_update_joins_the_receipt` | R-1.3 |
| `conf::pc_4_same_key_different_args_conflicts` | R-1.3 |
| `conf::pc_4_same_key_same_args_same_operation` | R-1.3 |
| `conf::pc_4_survives_plugin_reload_and_crash` | R-1.3 |
| `conf::pc_4_two_interfaces_do_not_alias` | R-1.3 |
| `conf::pc_4_uncertain_keyed_call_never_rerun` | R-1.3 |
| `conf::pc_4_unknown_result_retained_through_window` | R-1.3 |
| `conf::pc_4_window_bounded` | R-1.3 |
| `conf::pc_5_callee_crash_ends_pending_calls_effect_unknown_after_dispatch` | R-1.3 |
| `conf::pc_5_caller_unload_cancels_pending_calls` | R-1.3 |
| `conf::pc_5_undispatched_call_effect_none` | R-1.3 |
| `conf::pm_10_operations_exist` | R-1.3 |
| `conf::pm_10_remove_retains_the_store_until_purge` | R-1.3 |
| `conf::pm_10_status_values` | R-1.3 |
| `conf::pm_11_definitions_and_accessories_share_the_model` | R-1.3 |
| `conf::pm_11_definitions_are_data_and_hash_verified` | R-1.3 |
| `conf::pm_12_target_discovery_uses_the_same_statuses_and_operations` | R-1.3 |
| `conf::pm_12_target_instance_has_its_own_lock_entry` | R-1.3 |
| `conf::pm_13_agent_with_the_scope_can` | R-1.3 |
| `conf::pm_13_agent_without_the_scope_cannot` | R-1.3 |
| `conf::pm_13_client_verbs_are_the_hub_client_contract_names` | R-1.3 |
| `conf::pm_13_plugin_cannot_manage_packages` | R-1.3 |
| `conf::pm_14_fake_git_and_catalog_are_scripted_and_offline` | R-1.3 |
| `conf::pm_14_real_run_install_update_preview_pin_rollback_remove` | R-1.3 |
| `conf::pm_1_catalog_approval_does_not_approve_its_listed_sources` | R-1.3 |
| `conf::pm_1_git_ref_branch_tag_or_commit` | R-1.3 |
| `conf::pm_1_local_path_never_reaches_a_plugin` | R-1.3 |
| `conf::pm_1_local_source_is_live_with_dev_reload` | R-1.3 |
| `conf::pm_1_package_has_one_source` | R-1.3 |
| `conf::pm_1_source_kinds` | R-1.3 |
| `conf::pm_1_source_url_approved_by_the_install_that_names_it` | R-1.3 |
| `conf::pm_2_any_number_of_catalogs` | R-1.3 |
| `conf::pm_2_catalog_content_never_runs` | R-1.3 |
| `conf::pm_2_catalog_entry_shape` | R-1.3 |
| `conf::pm_2_catalog_fetch_follows_nt2_rules` | R-1.3 |
| `conf::pm_2_refresh_only_when_asked` | R-1.3 |
| `conf::pm_3_content_hash_ignores_timestamps_owners_and_empty_dirs` | R-1.3 |
| `conf::pm_3_content_hash_is_path_sorted_with_type_mode_and_digest` | R-1.3 |
| `conf::pm_3_install_reproduces_from_the_lock` | R-1.3 |
| `conf::pm_3_lock_history_bounded` | R-1.3 |
| `conf::pm_3_lock_is_durable` | R-1.3 |
| `conf::pm_3_lock_record_fields` | R-1.3 |
| `conf::pm_3_nothing_updates_without_an_operation` | R-1.3 |
| `conf::pm_4_every_fetch_applies_nt2_including_catalog_selected_urls` | R-1.3 |
| `conf::pm_4_fetch_uses_the_isolated_io_service` | R-1.3 |
| `conf::pm_4_install_runs_no_plugin_code_or_script` | R-1.3 |
| `conf::pm_4_size_bounded` | R-1.3 |
| `conf::pm_4_unpack_refuses_traversal_symlink_and_special_files` | R-1.3 |
| `conf::pm_5_failed_step_leaves_nothing_loaded` | R-1.3 |
| `conf::pm_5_install_order` | R-1.3 |
| `conf::pm_5_staged_until_grants_approved` | R-1.3 |
| `conf::pm_6_changed_hash_without_update_refused` | R-1.3 |
| `conf::pm_6_hash_verified_before_every_load` | R-1.3 |
| `conf::pm_6_local_source_is_live_not_enforced` | R-1.3 |
| `conf::pm_7_approval_resumes_the_staged_commit_and_hash_not_the_moved_branch` | R-1.3 |
| `conf::pm_7_commit_is_one_atomic_lock_write` | R-1.3 |
| `conf::pm_7_crash_after_commit_leaves_new_active` | R-1.3 |
| `conf::pm_7_crash_before_commit_leaves_old_active` | R-1.3 |
| `conf::pm_7_failure_after_activation_is_an_ordinary_crash` | R-1.3 |
| `conf::pm_7_failure_before_handover_keeps_the_old_version` | R-1.3 |
| `conf::pm_7_pending_approval_never_replaces_the_active_lock` | R-1.3 |
| `conf::pm_7_preview_shows_commits_hashes_files_manifest_and_grants` | R-1.3 |
| `conf::pm_7_preview_upgrade_is_a_separate_read_and_changes_nothing` | R-1.3 |
| `conf::pm_7_update_is_atomic_at_handover` | R-1.3 |
| `conf::pm_8_new_grants_need_approve_grants` | R-1.3 |
| `conf::pm_8_removed_grants_apply_at_handover_not_at_preview_or_staging` | R-1.3 |
| `conf::pm_8_update_with_new_grants_keeps_old_version_until_approved` | R-1.3 |
| `conf::pm_9_pin_blocks_update` | R-1.3 |
| `conf::pm_9_rollback_never_restores_revoked_authority` | R-1.3 |
| `conf::pm_9_rollback_restores_the_locked_commit_and_verifies_hash` | R-1.3 |
| `conf::pm_9_rollback_revalidates_the_manifest_against_current_grants` | R-1.3 |
| `conf::pm_9_rollback_steps_through_the_history` | R-1.3 |
| `conf::pm_9_unpin_releases` | R-1.3 |
| `conf::px_1_environment_scrubbed` | R-1.3 |
| `conf::px_1_runs_argv_without_a_shell` | R-1.3 |
| `conf::px_2_command_exit_versus_bridge_exit` | R-1.3 |
| `conf::px_2_command_not_found_is_typed` | R-1.3 |
| `conf::px_2_tails_bounded_and_truncated_flag` | R-1.3 |
| `conf::px_2_tails_redact_own_secrets_and_are_never_logged` | R-1.3 |
| `conf::px_2_terminal_record_shape` | R-1.3 |
| `conf::px_3_cancel_unload_and_crash_kill_the_group` | R-1.3 |
| `conf::px_3_reload_handover_kills_old_instance_commands` | R-1.3 |
| `conf::px_3_timeout_after_start_is_effect_unknown` | R-1.3 |
| `conf::px_3_timeout_kills_the_group` | R-1.3 |
| `conf::px_4_needs_its_own_scope` | R-1.3 |
| `conf::px_4_no_containment_claim` | R-1.3 |
| `conf::st_1_declaration_admitted` | R-1.3 |
| `conf::st_1_equal_rev_means_unchanged` | R-12 |
| `conf::st_1_index_change_rebuilds` | R-1.3 |
| `conf::st_1_new_instance_never_equal` | R-12 |
| `conf::st_1_reads_carry_model_rev` | R-12 |
| `conf::st_1_resize_changes_rev_without_output` | R-12 |
| `conf::st_2_screen_history_flag` | R-12 |
| `conf::st_3_coordinates_are_zero_based_cells` | R-12 |
| `conf::st_3_cursor_fields` | R-12 |
| `conf::st_3_trimming_rules` | R-12 |
| `conf::st_4_terminal_state` | R-12 |
| `conf::st_5_cursor_invalid_after_rebuild` | R-1.3 |
| `conf::st_5_pages_are_current_reads` | R-1.3 |
| `conf::st_5_reads_after_exit` | R-12 |
| `conf::st_6_at_most_one_pending_per_watch` | R-1.3 |
| `conf::st_6_capture_bound` | R-12 |
| `conf::st_6_capture_is_frozen_at_model_rev` | R-12 |
| `conf::st_6_full_queue_does_not_drop_the_obligation` | R-1.3 |
| `conf::st_6_notification_after_every_commit` | R-1.3 |
| `conf::st_6_pages_owned_last_flag_and_range` | R-12 |
| `conf::st_6_rearm_delivers_changed_once` | R-1.3 |
| `conf::st_6_snapshot_versioned` | R-12 |
| `conf::st_6b_baseline_restores_palette_cursor_shape_hyperlinks_modes_and_kitty_flags` | R-12 |
| `conf::st_6b_cut_inside_an_sgr_sequence_applies_the_suffix_not_prints_it` | R-12 |
| `conf::st_6b_graphics_only_when_the_feature_is_advertised` | R-12 |
| `conf::st_6b_model_after_baseline_plus_output_equals_the_sessions_model` | R-12 |
| `conf::st_6b_resume_invariant_holds_at_every_byte_offset_of_a_corpus_with_partial_escape_sequences` | R-12 |
| `conf::st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut` | R-12 |
| `conf::st_7_client_tail_is_exact_and_bounded` | R-12 |
| `conf::st_7_completion_appends_a_status_update_with_a_new_cursor` | R-12 |
| `conf::st_7_completion_raises_the_keyed_activity_edge` | R-12 |
| `conf::st_7_cursor_from_another_instance_is_a_gap` | R-12 |
| `conf::st_7_event_keeps_the_original_semantic_payload` | R-12 |
| `conf::st_7_events_bounded_by_records_and_bytes` | R-12 |
| `conf::st_7_events_distinguish_paste_release_and_no_write` | R-12 |
| `conf::st_7_facts_after_link_loss` | R-12 |
| `conf::st_7_facts_are_read_at_one_ordering_point` | R-12 |
| `conf::st_7_gap_is_reported_when_retention_removed_a_record` | R-12 |
| `conf::st_7_incomplete_history_is_flagged` | R-12 |
| `conf::st_7_incremental_read_returns_records_after_the_cursor` | R-12 |
| `conf::st_7_incremental_reader_sees_completion_of_a_pending_record` | R-12 |
| `conf::st_7_limits_that_cannot_hold_the_protected_records_are_invalid_config` | R-12 |
| `conf::st_7_new_client_rev_is_never_visible_without_its_record` | R-12 |
| `conf::st_7_newest_record_is_never_dropped_even_when_oversize` | R-12 |
| `conf::st_7_one_cursor_orders_client_and_host_records` | R-12 |
| `conf::st_7_projection_snapshot_and_diff` | R-1.3 |
| `conf::st_7_record_is_created_at_receipt_with_pending_state` | R-12 |
| `conf::st_7_tail_complete_flag` | R-12 |
| `conf::st_7_tail_holds_only_written_bytes_pending_is_flagged` | R-12 |
| `conf::st_8_documents_only_count` | R-1.3 |
| `conf::st_8_quota_counts_index_bytes` | R-1.3 |
| `conf::st_9_batch_atomic_across_crash` | R-1.3 |
| `conf::st_9_delete_durable_across_kill9` | R-1.3 |
| `conf::st_9_durable_across_restart` | R-1.3 |
| `conf::st_9_fsync_before_ack` | R-1.3 |
| `conf::st_9_survives_kill9` | R-1.3 |
| `conf::sv_10_lost_service_keeps_id` | R-12 |
| `conf::sv_1_exact_env_plus_core_vars` | R-12 |
| `conf::sv_1_inherits_only_listed_descriptors` | R-12 |
| `conf::sv_1_no_product_prefix` | R-12 |
| `conf::sv_2_authenticated_lane_faults_end_epoch` | R-12 |
| `conf::sv_2_bad_preamble_closes_only_that_connection` | R-12 |
| `conf::sv_2_config_must_fit_wire_and_one_frame` | R-12 |
| `conf::sv_2_core_sends_nothing_unsolicited` | R-12 |
| `conf::sv_2_duplicate_lane_ignored` | R-12 |
| `conf::sv_2_len_below_two_is_invalid` | R-12 |
| `conf::sv_2_preamble_authenticates` | R-12 |
| `conf::sv_2_unauthenticated_connection_does_not_end_epoch` | R-12 |
| `conf::sv_3_inbound_lossless_in_order` | R-12 |
| `conf::sv_3_lanes_independent` | R-12 |
| `conf::sv_3_readable_is_level_until_drained` | R-12 |
| `conf::sv_3_send_backpressure_no_ownership` | R-12 |
| `conf::sv_3_writable_after_refusal_with_queue_not_full` | R-12 |
| `conf::sv_3_writable_wakes_when_free_bytes_fit_refused_frame` | R-12 |
| `conf::sv_4_no_silent_skip` | R-12 |
| `conf::sv_4_supported_bounds_applied_and_reported` | R-12 |
| `conf::sv_4_unsupported_optional_bound_does_not_refuse_launch` | R-12 |
| `conf::sv_5_no_unobservable_limit_causes` | R-12 |
| `conf::sv_6_epoch_commits_only_after_all_lanes` | R-12 |
| `conf::sv_6_equal_epoch_valid_within_staged_group` | R-12 |
| `conf::sv_6_lower_epoch_rejected` | R-12 |
| `conf::sv_6_no_mixing_of_generations` | R-12 |
| `conf::sv_6_one_lane_fault_ends_all_lanes_then_higher_epoch` | R-12 |
| `conf::sv_6_partial_frame_never_resumed` | R-12 |
| `conf::sv_6_same_process_after_adoption` | R-12 |
| `conf::sv_6_unsent_dropped_received_kept_with_old_epoch_tag` | R-12 |
| `conf::sv_7_exited_service_stays_exited` | R-12 |
| `conf::sv_8_adoption_cancels_grace` | R-12 |
| `conf::sv_8_guardian_loss_is_reported_honestly` | R-12 |
| `conf::sv_8_pdeathsig_where_enforced` | R-12 |
| `conf::sv_9_final_frames_readable_until_remove` | R-12 |
| `conf::sv_9_log_tail_bounded` | R-12 |
| `conf::sv_9_sigterm_then_group_kill` | R-12 |
| `conf::sv_9_start_rollback` | R-12 |
| `conf::tm_1_clocks_are_injected` | R-12 |
| `conf::tm_1_deadlines_use_monotonic` | R-12 |
| `conf::tm_3_due_deadline_fires_in_pump` | R-12 |
| `conf::tm_3_next_deadline_is_host_armed` | R-12 |
| `conf::tm_3_wake_handle_ignores_deadlines` | R-12 |
| `conf::tm_4_silent_fires_once_at_threshold` | R-12 |
| `conf::tm_5_deadlines_in_order` | R-12 |
| `conf::tm_6_begin_and_attach_signal_wake` | R-12 |
| `conf::tm_6_create_without_worker_input_wakes` | R-12 |
| `conf::tm_6_more_means_runnable_not_blocked` | R-12 |
| `conf::tm_6_parked_work_does_not_spin` | R-12 |
| `conf::tm_6_poll_room_unparks_and_wakes` | R-12 |
| `conf::tp_1_at_most_one_unacknowledged_tap_frame_on_the_control_link` | R-12 |
| `conf::tp_1_control_frames_not_delayed_by_tap` | R-12 |
| `conf::tp_1_full_tap_drops_oldest_with_loss_marker_and_never_slows_the_pty` | R-12 |
| `conf::tp_1_tap_bytes_equal_pty_bytes` | R-12 |
| `conf::tp_1_tap_chunk_carries_instance_and_dropped_count` | R-12 |
| `conf::tp_1_tap_is_off_by_default` | R-12 |
| `conf::tp_1_tap_never_back_pressures_a_route_and_overhead_is_measured` | R-12 |
| `conf::ts_10_last_focused_route_end_writes_focus_out` | R-5 |
| `conf::ts_4_state_equivalence` | R-5 |
| `conf::ts_4d_modes_effective_size` | R-5 |
| `conf::ts_5b_unreported_mode_input_is_silent` | R-5 |
| `conf::wp_1_needs_its_own_scope` | R-1.3 |
| `conf::wp_1_null_clears_the_origin_and_pairing_is_refused_no_web_origin` | R-1.3 |
| `conf::wp_1_origin_grants_no_trust` | R-1.3 |
| `conf::wp_1_served_origin_resolves_to_plugin_origin` | R-1.3 |
| `conf::wp_1_set_origin_changes_pairing_urls` | R-1.3 |
| `conf::wp_1_there_is_no_hub_default_origin` | R-1.3 |
| `conf::wp_2_attempt_retired_on_completion_expiry_and_plugin_lifecycle` | R-1.3 |
| `conf::wp_2_attempt_starts_with_relay_open_and_is_bounded` | R-1.3 |
| `conf::wp_2_envelopes_opaque_and_bounded` | R-1.3 |
| `conf::wp_2_external_origin_plugin_carries_the_signal` | R-1.3 |
| `conf::wp_2_guessed_relay_id_is_not_found` | R-1.3 |
| `conf::wp_2_needs_its_own_scope_and_handler_admission` | R-1.3 |
| `conf::wp_2_plugin_never_chooses_or_reads_the_challenge` | R-1.3 |
| `conf::wp_2_relay_id_bound_to_plugin_and_attempt` | R-1.3 |
| `conf::wp_2_relay_id_maps_to_the_pr4_connection` | R-1.3 |
| `conf::wp_2_served_origin_hub_answers_post_signal_as_a_relay_attempt` | R-1.3 |
| `conf::wp_3_acceptance_decided_by_pr4_to_pr9_only` | R-1.3 |
| `conf::wp_3_plugin_cannot_read_route_bytes_secrets_or_keys` | R-1.3 |
| `conf::wp_3_relay_failure_is_denial_only` | R-1.3 |
| `conf::wp_4_deliver_backpressured` | R-1.3 |
| `conf::wp_4_outbound_queue_bounded_with_counted_gap` | R-1.3 |
| `conf::wp_4_relay_plugin_requires_gap_handler` | R-1.3 |
| `conf::wp_4b_uses_only_net_and_ws` | R-1.3 |
| `conf::wp_5_binds_where_the_operator_configures` | R-1.3 |
| `conf::wp_5_each_plugin_has_its_own_origin` | R-1.3 |
| `conf::wp_5_fixed_content_types_and_nosniff` | R-1.3 |
| `conf::wp_5_no_handler_invoked_per_request` | R-1.3 |
| `conf::wp_5_path_cannot_leave_the_root` | R-1.3 |
| `conf::wp_5_serves_get_and_head_only_from_the_root` | R-1.3 |
| `conf::wp_6_client_fetches_from_the_web_origin_route` | R-1.3 |
| `conf::wp_6_ice_carries_no_trust` | R-1.3 |
| `conf::wp_6_ice_servers_stored_durably_and_bounded` | R-1.3 |
| `conf::wp_6_turn_credential_never_logged` | R-1.3 |

<!-- real-only:end -->

### Tests: tiers and budgets

Install the runner once: `cargo install cargo-nextest --locked`. Use `cargo xtask test-budget` where you would use `cargo test`. Phase 1 repos copy `xtask/src/test_budget.rs`, `xtask/src/timers.rs`, `.config/nextest.toml` and the CI steps (BUILD.md, "Testing: fast and lean").

| Tier | Run with | Holds | Budget |
|---|---|---|---|
| default | `cargo xtask test-budget` | In-process tests only: transcripts against fakes, unit tests, drift tests. No child process, socket on disk or daemon. | 2 s a test and 60 s in all. A leftover process fails the run. |
| slow | `cargo xtask test-budget --slow [--deadline 10m]` | The few real-process tests, each citing the clause it proves. | No per-test limit; one deadline for the whole tier. |

The command prints per-test times (slowest first) and writes `target/test-times.json`. On a pull request, CI passes the last `main` file with `--compare <file>`, and the command prints every test that at least doubled. A doubling is a review finding, not a failure.

**Slow tests.** A crate with slow tests declares a `slow` cargo feature and puts them in an integration-test target named `slow` (or `slow_*`) that needs the feature:

```toml
[features]
slow = []

[[test]]
name = "slow"
required-features = ["slow"]
```

`cargo test` and the default tier skip that target. The slow tier runs only the `slow*` targets of the crates that have the feature.

**Timers.** A test waits on the real event, never on a timer. The one allowed timer is a deadline timer, and its line or the line before it says why:

```rust
// timer: deadline — the driver must outlast the step deadline (design 6.1)
std::thread::sleep(d);
```

`cargo xtask timers` fails on `sleep(`, `setTimeout(` and `setInterval(` in test code (files under `tests/`, `*.test.*`, and items with `#[cfg(test)]` or `#[test]` in a `src` file) without that marker. It also fails on any timer in the production code of a fake crate: a fake takes an injected clock.

## Running the Core suite

`botster-core-conformance` holds the Core transcripts (`conformance/core/`) and the runner. A Phase 1 Core repo builds a `CoreHarness` (configured `open`, named data dirs, worker builds, controls, the probe binary, route attach) and calls `conformance_tests!(|seed| Box::new(RealCoreHarness::new()), CoreSchemas)` once. The same suite runs against `FakeCore` (feature `fake`, `FakeCoreHarness`), which is Core section 11's faithfulness proof (steward ruling R-1.5). Notes, the control list and the placement decisions are in `docs/fake-core-notes.md`.

The Core testkit's scripted program backend reads probe scripts (Core A5-1) through the `botster-probe-script` crate: the types, the parser, the validator and the schema `schemas/core/probe-script.schema.json`. The crate has no fake dependency and no interpreter.

### Plugin fakes and the HP runner

`botster-plugin-fakes` holds the plugin-side FakeHub and FakePlugin. `botster-plugin-conformance` holds the HP runner and the transcripts of `conformance/hp/`. See `docs/plugin-fakes.md` for the steps, the controls, the fixtures and the prior art.

```rust
// A Hub repo (subject plugin_hub). `make` is Fn(u64) -> Box<dyn HpSubject>.
botster_plugin_conformance::hub_conformance_tests!(make_hub);
// A plugin-host repo (subject plugin_host). The transcripts name fixture plugins in conformance/hp/fixtures/.
botster_plugin_conformance::host_conformance_tests!(make_host);
```

The `fake` feature has the adapters (`fake::hub`, `fake::host`). The ids `conf::lh_1_local_helpers_send_nothing`, `conf::lh_4_globals_equal_published_set` and `conf::tr_2_hostile_plugin_contained` are Lua-host ids and stay in `conformance/pending.txt` for the `lb-suite` feature.

## Coming next (build)

- Contract crates (`botster-hub-contract`, `botster-plugin-contract`) and the generated schemas.
- Fakes (FakeCore, FakeHub, FakePlugin), written from the clauses.
- Conformance transcripts and runners (Rust, plus TypeScript for Web).
