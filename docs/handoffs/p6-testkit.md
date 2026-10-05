# Handoff: P6 testkit, Stage 1 Core

## CURRENT — PAUSED (2026-10-04, user order, token budget)

Nothing is unpushed. Both worktrees are clean (tracked files). No gate or heavy job of P6 is running.

### Sessions
- Implementer: the Opus session that wrote this section (worktree `~/botster-sessions/trybotster-botster-core-stage1-p6-oracle`).
- Package reviewer: `sess-1791169299-010f-cc0146e26529bfbb3589ccc55e082532` (Sol). The `...00fd` reviewer is retired.
- Integration reviewer: `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9` (Opus). It reviews the RealCoreHarness PR.
- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.

### Branches and exact pushed heads
1. **PR #161** `stage1/contracts-v0.1.14` (worktree `~/botster-sessions/trybotster-botster-core-stage1-p6-pin`), head
   **`dea90ed4ee502432a70e19ec93b26c4a81558773`**, base v1 `144b023`. Contracts pin a8db5c9 -> **contracts-v0.1.17**
   (`1725abf`, manifest final35, Core A14-A16, R-30..R-34). Commits: 5a34cf8 (v0.1.14), b27fdac (v0.1.15), 257d2ed (v0.1.16),
   dea90ed (v0.1.17). Core ids 654 -> 675; pending gained a14_* (10, P3), a16_1_* (7, P7), a15_1_* (4: P5 host, P5 limits, P3
   worker, P4a route). No contract crate changed between a8db5c9 and 1725abf.
   - Review: **CLEAN on dea90ed**, verdict `d1a3869e68f90c0ab69a951141304776e0818eb8`, `verdicts/p6-contracts-v0.1.14.md` on
     `stage1/review-p6`. Zero open findings.
   - Gate: the only gate (head 5a34cf8, log `gates/botster-core-stage1-contracts-v0.1.14-5a34cf84-linux-20261004-201733-75810.log`)
     was RED only on the known races A10 (slow_real_core reaper race, P5) and A31 (slow_payload FIONREAD, P3). No regate (rule 9).
   - Merge order (lead): P3 guard fix, #162, #163, #142, then **#161 last**. Then: merge origin/v1 into the branch, reviewer delta
     CLEAN, gate ONCE on the exact head, READY to the lead.
2. **RealCoreHarness** `stage1/p6-real-harness` (worktree `~/botster-sessions/trybotster-botster-core-stage1-p6-oracle`), head
   **`276427d0c6515adcfc796c34662bf107228721e6`** (WIP commits on `d54ff94`, base v1 `144b023`). No PR, no review yet.
   - Content: `anchor.rs` (protocol), `src/bin/botster-test-anchor.rs` (rewritten wrapper/intermediate/anchor), `real/guard.rs`
     (`AnchorGuard`, `SETTLE` deadlines, `await_reports`), `real/harness.rs` (`RealCoreHarness` over `Core::open`), `Candidate`
     with the anchor, xtask `prebuild-worker` builds the anchor, `tests/suite` shared runner + `tests/slow_conformance.rs`
     (botster-core `slow`), `tests/slow_real_harness.rs` + `tests/common` + fixture bin `tests/bin/fixture.rs`
     (`botster-test-fixture`). Design and prior art: `crates/botster-core-testkit/DESIGN.md` "RealCoreHarness".
   - Proof so far (Linux focused run of 209ceb5): prebuild OK, clippy -D warnings clean, 200 testkit unit tests pass;
     slow_real_harness 3/6 pass (refusal on group move, owner death before registration, descriptors). The Core lifecycle test
     passed exit code, Stop TERM, Lost(WorkerGone) and Remove, then failed on the anchor count. Fixed in 276427d, UNVERIFIED:
     anchors counted before the payload wrapper connected (now `await_anchors`), and pidfd_open EINVAL on a releasing pid.
     slow_conformance not yet reached.
   - Focused command (needs `--profile slow`): `botster-gate --on linux <worktree> -- env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4
     sh -c 'cargo xtask prebuild-worker && cargo clippy -p botster-core-testkit -p botster-core -p xtask --all-targets --features
     botster-core-testkit/slow,botster-core/slow -- -D warnings && cargo nextest run --profile slow -p botster-core-testkit
     --features slow --test slow_real_harness --no-fail-fast && cargo nextest run --profile slow -p botster-core --features slow
     --test slow_conformance'`.
   - Still to do before READY: green focused run; merge v1 (after #161 lands, the pin); per-function mutation exclusions for
     slow-only code with a slow-tier mutation run (precedent in `.cargo/mutants.toml`); PR with the Prior art note; READY to the
     package AND integration reviewers; the one Mac cleanup run (lead approval); full Linux gate after both CLEAN.
   - Known gap: ruling item 15 SIGUSR1 delivery needs a stop over a broken control link, i.e. `break_control` on the real harness.
     Report it in READY as pending with that control.

### Rulings in force
- RealCoreHarness anchor design (lead ruling 2026-10-04; full list below under "Binding anchor ruling"): public launch inputs
  only; the wrapper execs the real binary; a double-forked anchor holds the group and the guard fd; on guard EOF: TERM, the
  existing stop_grace, verify by pid+start time, KILL the group as the final act; refuse on a group move; no subreaper (init/tini
  and launchd reap, one code path).
- R-30 (exact oracle fit measurement), R-31 (every-cut: one session per corpus item, release each capture, no sampling, one
  beyond-limit string per kind), R-32, R-33, R-34: all in contracts-v0.1.17 (`docs/steward-rulings.md`).
- Merge rule: green gate on the exact head; no rerun-until-green (rule 9).

### Open audit issues (P6)
- **#159** (testkit MEDIUM/LOW: A34, A35, A41, A42, A43, A44, A59-A64, A51): not started. Plan: a separate small PR after
  RealCoreHarness (say so in the RealCoreHarness READY). A63 (R-30 citation) is resolved by #161. A51 is respected by the new
  fixture binary. A44 (registry bound to TestkitHarness) matters for real-harness controls.
- **#154** (A12, every-cut cost): answered by R-31; the every-cut rework to R-31 is not started.

### Exact next step (on resume)
- The wind-down chain is PAUSED; nothing merges now. #161 stays CLEAN at `dea90ed`, last in the merge order on resume.
- On resume, when the lead says #142 has merged: `git merge origin/v1` into `stage1/contracts-v0.1.14` (no force-push), push,
  reviewer delta CLEAN, gate ONCE on the exact head, READY to the lead.
- RealCoreHarness: resume at the pushed head `276427d`. Run the focused command above on Linux (`--profile slow`), fix until
  green, then the remaining RealCoreHarness steps listed above.
- Pending ids: unchanged by P6 work in this phase; no id left `core-pending.txt`. #161 adds the a14_*, a15_1_* and a16_1_* ids
  as pending with their owners (see above). The P6 package ids and oracle-dispatch ids listed below stay pending with their reasons.

Updated at the clean boundary for transfer from Sol to the Opus implementer.
The lead requested this transfer. The outgoing implementer stops after HANDOFF READY.

## Heads and sessions

- Repo: `trybotster/botster-core`.
- Worktree: `~/botster-sessions/trybotster-botster-core-stage1-p6-oracle`.
- Current branch: `stage1/p6-real-harness`.
- Current pushed WIP head: `d54ff94fdfdb9a83996083b02db7ff4486f3d242`.
- Current base: v1 merge `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- No PR, review, compile check, test, or gate exists for the WIP head.
- The oracle-dispatch branch `stage1/p6-oracle-dispatch` has no implementation delta. Do not use it for the current scaffold.
- Earlier oracle reviewed and gated head: `c36120e62a3252c1588bf88fa860e5325939ee19`.
- PR #140 merge in v1: `e8cf15068825888795f3ff2582d98b5c8e9b09e4`. Its tree equals the reviewed head.
- Earlier oracle reviewer verdict: CLEAN, zero open findings, including LOW.
- Verdict commit: `ba01ce494871eb7558279b4a279dd964e8aaa8de`, `verdicts/p6-oracle.md`, branch `stage1/review-p6`.
- Worktree status: clean. All implementation commits are pushed.
- Implementer: `sess-1791136735-00fc-4b09baad3d85f4ce4397761167aced46`.
- Package reviewer: `sess-1791136735-00fd-5d70db1b6f5f889e162f1932f6ef2da4`.
- Integration reviewer: `sess-1791143089-0101-8ce5f4942328f5697c410ea4da89c466`.
- Current P3 implementer: `sess-1791142478-00ff-2e34fc638a8fdb437255a2fa665f81b6`.
- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.

## Current boundary and open work

The worktree is clean. The WIP commit is pushed. No gate or heavy job is running.
No source finding remains open on the merged oracle or registry PRs.
The WIP source has no review verdict. Treat all WIP behavior as unverified.
Do not treat its source reasoning as a cleanup proof.

PR #141 registered controls in their owning modules without changing behavior.
Its pushed head is `7e9b4ec6e7252c3ff7ec36cbff74c24fdae644fe` on `stage1/p6-control-registry`.
Its merge is `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Package CLEAN: `db280708bbdf9ecdef5df04d0dbef2bf07c6c3aa`, `verdicts/p6-control-registry.md`.
Integration CLEAN: `aa1d354ac7dd709b8357acb60419a1d030b93837`, `verdicts/stage1/p6-control-registry.md`.
All ten full Linux gate steps passed.
Gate log: `~/botster-sessions/gates/botster-core-stage1-p6-control-registry-7e9b4ec6-linux-20261004-193412-88388.log`.
Mutants: 11 caught, one unviable, zero missed, zero timeout.
Focused checks: 198 unit tests, the doc test, Clippy, and formatting passed.

P3 M1 merged as `01fd38968b9e7605becc7e2b5088628aff52865a`.
The lead ended the restriction on harness.rs, program.rs, core.rs, and worker.rs.
P3 owns its M2a worker/program changes and the agreed open() call-site changes.
P3 will add workers(), worker_controls.rs, its module declaration, and its registration call.
P6 must coordinate before overlapping those edits. P6 must not change P3's production model.

## Current RealCoreHarness scaffold

Commit `d54ff94` adds only:
- `src/bin/botster-test-anchor.rs`, an unverified helper scaffold;
- the helper binary target and optional slow dependencies in the testkit Cargo.toml;
- two dependency edges in Cargo.lock, with no version change;
- the M2b observation requirements in DESIGN.md.

The helper has wrap, intermediate, and anchor stages.
Safe Command::spawn calls implement the two forks. No unsafe fork call is present.
The wrap stage connects to a guard socket and passes that connection as inherited stdin and stderr to the helper stages.
The anchor uses guard fd 0. Its stdout carries one ready acknowledgement to the wrapper.
The intermediate exits immediately. The wrapper waits and reaps only the intermediate.
The anchor reports its pid, start time, group, leader pid, and leader start time.
The wrapper then execs the real binary, preserving its pid and native exit path.
The detached anchor ignores TERM and blocks on guard EOF.
On EOF, it verifies membership, sends group TERM, waits the supplied grace, verifies again, and sends group KILL as its final action.
The helper uses the existing configured stop_grace value. The harness must supply that value.

This source is NOT runnable integration yet.
The guard socket owner, RealCoreHarness, generated wrappers, candidate manifest entry, and prebuild step are not implemented.
The Linux subreaper and verified anchor reaping are not implemented.
The helper has not been compiled. Dependency resolution has not been checked.
The old helper fixture tests have not been copied.
The next implementer must inspect descriptor isolation, startup failure, parent death before registration, and group-move refusal.
The next implementer must prove lifecycle equivalence and cleanup through actual production entry points.

## Binding anchor ruling

The lead assigned this boundary to P6 through public launch inputs.
Do not add a Process injection seam or a test branch to Core's private production process code.

1. Prebuild botster-test-anchor through cargo xtask prebuild-worker. Put it in the same sha256 manifest.
2. The process Core or the worker spawns must exec the real binary and preserve its identity, exit status, and signals.
3. Before exec, the wrapper double-forks the anchor. The wrapper reaps only the intermediate child.
4. The detached grandchild holds the inherited process group and guard fd.
5. The anchor reports its pid, start time, and group before the real body starts.
6. The worker wrapper executes the verified prebuilt botster-worker through OpenConfig.worker_path.
7. The payload wrapper executes the verified prebuilt probe inside the payload's own session and group.
8. Guard EOF means test Drop or test death. The anchor ignores TERM for itself.
9. Cleanup sends group TERM, waits the EXISTING configured grace, verifies recorded identities and group membership, then sends group KILL.
10. KILL is the final action and also ends the anchor. The anchor reaps nothing.
11. Core and the worker retain exclusive reaping of their production-owned children.
12. Linux may use PR_SET_CHILD_SUBREAPER and reap only verified descendants. macOS relies on anchors and init reaping.
13. If the real binary moves to another group after exec, the anchor must report that move and refuse to kill.
14. Tests must prove panic cleanup and killed-test cleanup on Linux and macOS. The lead requires one Mac run.
15. Tests must prove that Core observes the real worker's pid, exit path, SIGUSR1 delivery, WorkerGone, and Remove completion.
16. If an id requires exact argv or paths that a wrapper changes, keep that id pending and send the lead a QUESTION.

The accepted initial proposal to reap a real child before final KILL was superseded by this detached-anchor ruling.
The anchor never busy-spins. No process is discovered by name or pattern. No unverified process is signalled.

## Live oracle deferral and argument adapters

The lead explicitly deferred live oracle dispatch to P3 M2b.
Current Worker discards Input::PtyOutput at worker.rs:669 and returns placeholder terminal state at lines 448-463.
M2b owns the actual libghostty model, completed model steps, and CaptureSnapshot.
P6 makes no production change in botster-worker-core.

The lead directs P6 to finish RealCoreHarness now and then add independent oracle argument adapters behind the registry.
Those adapters claim no terminal controls.
A control that needs the absent model returns typed ControlError::Refused with:
`not yet available: needs the worker model (P3 M2b)`.
It returns no fake terminal result. No pending id leaves the list in this phase.

The exact M2b observation requirements were sent to P3, which acknowledged them.
They are recorded in DESIGN.md under Required M2b observation boundary.
They cover data-directory plus InstanceId identity, actual Size and History, exact consumed chunks after completed model steps,
accepted resize/configuration ordering, actual native model access, capture revisions, and ordered post-capture output.
An oracle read does not pump the subject. A PTY read does not prove model consumption.
Observation errors fail the testkit run. Every-cut uses fresh actual sessions, real program writes, the fence, and actual pages.

## Merged prerequisites

- P1 lifecycle PR #135: `1d25d093301072bd0c13a122c68d8f1b1ca0815c`. Core::open is available in v1.
- P1 terminal identity PR #137: `393f403047beb1583ab3a711afbe2c37255c7e64`.
- P2 public oracle APIs PR #138: merge `38bbe580013d02175636395a28a4c359b20c32b2`, reviewed tree `a088d673d759984d9b10823d17d713b45c006ce9`.
- P2 GHOSTSNP spec PR #139: merge `a3b8af5be21389423439fb3c09d6a81d924c987d`, reviewed tree `d8b84543ab444f2cd04799eb9de6c9c7a7734d0c`.
- Both P2 PRs passed separate reviews and full Linux gates before merge.

## Delivered P6 APIs

- `oracle.rs`: OracleHandle retains a separate native terminal. The driver feeds only consumed output and accepted configuration changes.
- The handle supplies state, modes, screen, cursor, notification, key, mouse, focus, and paste observations or encodings.
- `oracle::oracle_query_reply` creates a fresh native shadow with the session size, optional prefix, and optional color profile.
- `snapshot_controls.rs`: CapturePages receives actual capture or baseline bytes and source configuration.
- Public controls: oracle_restore, oracle_resume, oracle_graphics, oracle_hyperlinks, and snapshot_unsupported_version.
- Restore fields compare separate state groups. Resume compares native snapshots, including pending parser input and saved state.
- `every_cut.rs`: CutSession requires a fresh real Core session for every offset, a fence, actual capture completion, and every page read.
- Offered captures include output after their snapshot revision and before the suffix. The helper replays those chunks first.
- The helper checks retention independently of the outcome. It reads semantic failure and retention again after capture.
- Failed retention within the independently measured limit remains a mismatch even when final replay succeeds.
- Unknown or non-fitting cuts set inconclusive. An independently established oversized offer adds a mismatch.
- `statement_runs.rs`: run_deterministic calls run_script_events with fresh harnesses. error_codes_reachable requires actual probes against refusal::ROWS.
- `DESIGN.md` states the dispatch obligations for all helpers.
- No harness.rs, program.rs, core.rs, or worker.rs was edited in this phase.

## R-30 and paging

Read R-30 in botster-contracts `docs/steward-rulings.md`, main commit `eb4aba0`.
The ruling replaces the defective pending-state size-bound condition in the older control text.
Fit uses an independent native encoding with the same format, version, size, history settings, and zero image limit.
The diagnostic terminal retains at least the input length.
The measurement must add worker framing and maximal per-capture fields from the published format spec.
It must never read framing values from the subject's capture.

`crates/botster-terminal-ghostty/GHOSTSNP.md` names CONTINUATION_LIMIT as 1,048,576 bytes.
The native format adds no per-capture identifier or timestamp.
The host counts only page.bytes and keeps pages unchanged.
Worker paging is explicitly pending P3 M2b. P3 must complete that spec section with its paging code.
Until then, production dispatch must use UnknownFraming. The every-cut fit remains inconclusive.
Synthetic framing and native session adapters in unit tests do not prove conformance IDs.

The revised graphics ruling requires reads of the native storage limit on the actual model and restored instance.
Both constructors set the limit to zero before input, and libghostty enforces it.
P2 tests verify image lookup after image stimuli on both screens.
The graphics control needs no image parser, guessed IDs, or stimulus record.

## Process cleanup

The slow process-group tests replace sleep children with children that block on parent-owned pipes.
Each outer test retains OwnedGroup through assertions and panic paths.
A parent-exit fixture starts its child inside the outer owned group and exits without Rust cleanup.
The outer test observes child EOF before it drops the group guard.
This proves that the child ends when its parent exits, even when parent cleanup does not run.
The P1 stream test remains present. All eight group tests passed.

## Proof

Full Linux gate on exact CLEAN head: all ten steps passed.
Gate log: `~/botster-sessions/gates/botster-core-stage1-p6-oracle-c36120e6-linux-20261004-171917-7957.log`.
Mutation result: 127 tested, 119 caught, eight unviable, zero missed, zero timeout.
Focused proof: 174 tests passed; clippy with -D warnings and fmt passed.
The slow process-group source passed eight tests and remained unchanged through the final review fixes.
The full gate also passed its slow tier and process checks.
No timeout or transcript changed. No P6 mutation exclusion was added.
No old botster-core source or tests were copied.

## Pending IDs and reasons

No ID left core-pending.txt in this phase.
Unit helpers cannot prove protected harness dispatch or real capture integration.

### P6 package IDs

Reason: P3 M1 open is now available, but complete edge controls and statement dispatch remain pending.
Real-process and suite proofs require RealCoreHarness and verified guarded use of the worker binary.
The statements now have helper APIs, but their real probes and harness dispatch remain pending.

- `conf::a5_1_deterministic_under_script_and_seed`
- `conf::a5_1_edges_scripted`
- `conf::a5_1_not_in_facade_or_ffi`
- `conf::a5_1_testkit_is_real_core_one_worker_code_path`
- `conf::a5_2_deferred_completion_keeps_slot_until_polled`
- `conf::a5_2_due_deadline_processed_in_its_pump`
- `conf::a5_2_each_section_11_variation_has_a_source`
- `conf::a5_2_reads_match_atomic_transitions`
- `conf::a5_3_async_failure_only_through_edges_and_real_completion`
- `conf::a5_3_every_code_reachable_or_listed_real_only`
- `conf::a5_3_only_sync_column_codes_scripted`
- `conf::a5_3_sync_refusal_has_no_op_event_slot_or_state`
- `conf::a5_4_hub_tests_on_testkit_under_seeds`
- `conf::a5_4_suite_runs_real_and_testkit`
- `conf::or_3_unspecified_orders_are_randomized`

### IDs that use the delivered oracle or statement controls

Reason: live terminal dispatch waits for the P3 M2b model and observation boundary.
The transcript must also pass through both harnesses before its ID can leave pending.
Every-cut has an additional reason: fit evidence awaits P3 M2b worker paging under R-30.
The list below comes from the pinned contracts-v0.1.13 transcripts and the current pending file.

- `conf::a2_1_read_screen_history_flag_and_text`
- `conf::a2_2_unsupported_what_values`
- `conf::a2_4_notification_oversize_is_truncated_with_flag`
- `conf::a3_2_close_from_stalled_runs_dp12`
- `conf::a5_1_deterministic_under_script_and_seed`
- `conf::a5_3_every_code_reachable_or_listed_real_only`
- `conf::a8_2_offered_snapshots_still_satisfy_the_resume_invariant`
- `conf::dp_12_last_focused_route_closing_writes_focus_out`
- `conf::dp_12_no_focus_out_while_another_route_is_focused`
- `conf::dp_4_worker_encodes_route_frames_with_write_time_modes`
- `conf::dp_5_unstarted_terminal_query_frame_retires_when_the_query_resolves`
- `conf::dp_5b_oversize_encoded_path_is_refused_before_the_file_is_created`
- `conf::dp_5b_path_is_followed_by_one_space_in_one_paste`
- `conf::dp_5b_written_bytes_and_the_precreation_bound_include_the_space`
- `conf::dp_5b_written_bytes_are_encoded_pty_bytes_with_markers`
- `conf::e2_2_other_modes_are_tracked_model_modes`
- `conf::e2_2_unrecognized_mode_number_changes_no_field`
- `conf::e2_3_mode_change_posts_latest_flags`
- `conf::e2_3_one_step_net_changed_posts_final_flags`
- `conf::e2_3_one_step_net_unchanged_posts_nothing`
- `conf::e2_3_write_start_uses_current_model_modes`
- `conf::e4_1_client_reply_reserves_its_frame_size`
- `conf::e4_1_fallback_reserves_its_reply_length`
- `conf::e4_1_reply_one_byte_over_the_free_bound_is_parked`
- `conf::ev_1_replaced_keyed_event_moves_to_latest_position`
- `conf::ev_2_mandatory_and_keyed_never_dropped`
- `conf::ev_5_keyed_events_bounded_by_keys`
- `conf::ev_6_keyed_event_carries_latest`
- `conf::ev_6_loss_leaves_marker_then_state_is_readable`
- `conf::ev_7_each_terminal_feature_reaches_the_host`
- `conf::ev_7_every_mode_change_posts_modes_changed`
- `conf::ev_7_hyperlink_in_output_and_snapshot`
- `conf::ev_8_admission_reserves_full_bytes`
- `conf::ev_8_client_answers_first_shadow_suppressed`
- `conf::ev_8_client_decline_makes_shadow_answer_at_once`
- `conf::ev_8_client_reply_after_a_resync_is_rejected_query_expired`
- `conf::ev_8_client_silent_shadow_answers_at_deadline`
- `conf::ev_8_decline_does_not_consume_the_query_id_before_fallback_admission`
- `conf::ev_8_exactly_one_reply_under_every_interleaving`
- `conf::ev_8_fallback_started_means_admitted_refused_fallback_stays_pending`
- `conf::ev_8_late_client_reply_is_refused`
- `conf::ev_8_no_route_shadow_answers_at_once`
- `conf::ev_8_oversize_query_answered_by_shadow_with_marker`
- `conf::ev_8_refused_fallback_admission_stays_on_the_fallback_path_only`
- `conf::ev_8_reply_after_decline_is_query_expired_then_already_replied`
- `conf::ev_8_reply_does_not_move_input_rev`
- `conf::ev_8_resync_before_answer_ends_the_opportunity_and_uses_the_shadow`
- `conf::ev_8_resync_never_lets_a_client_answer_from_stale_state`
- `conf::ev_8_route_close_falls_back_to_shadow`
- `conf::ev_8_set_color_profile_changes_later_shadow_answers`
- `conf::ev_8_shadow_answers_colors_from_the_host_supplied_profile_with_no_client`
- `conf::ev_8_shadow_cannot_answer_and_client_silent_means_no_reply`
- `conf::ev_8_shadow_reply_is_computed_at_the_query_point`
- `conf::ev_8_unstarted_query_frame_is_retired_by_a_resync`
- `conf::ev_8_worker_never_writes_before_client_opportunity_ends`
- `conf::in_9_application_cursor_and_keypad`
- `conf::in_9_key_encoded_by_worker_modes_at_write`
- `conf::in_9_key_shifted_and_alternate_keys_omission_rule`
- `conf::in_9_kitty_all_32_flag_combinations_pinned`
- `conf::in_9_kitty_each_flag`
- `conf::in_9_kitty_flag_16_needs_flag_8`
- `conf::in_9_kitty_modifier_keys_need_flag_8`
- `conf::in_9_kitty_text_with_control_codepoint_omitted`
- `conf::in_9_kitty_unknown_flag_bits_ignored`
- `conf::in_9_legacy_keys_table`
- `conf::in_9_legacy_release_is_not_reported`
- `conf::in_9_mouse_each_encoding_and_limits`
- `conf::in_9_mouse_each_tracking_mode`
- `conf::in_9_non_us_layout_uses_supplied_text`
- `conf::st_3_coordinates_are_zero_based_cells`
- `conf::st_3_cursor_fields`
- `conf::st_3_trimming_rules`
- `conf::st_4_terminal_state`
- `conf::st_5_reads_after_exit`
- `conf::st_6_snapshot_versioned`
- `conf::st_6b_baseline_restores_palette_cursor_shape_hyperlinks_modes_and_kitty_flags`
- `conf::st_6b_cut_inside_an_sgr_sequence_applies_the_suffix_not_prints_it`
- `conf::st_6b_graphics_only_when_the_feature_is_advertised`
- `conf::st_6b_model_after_baseline_plus_output_equals_the_sessions_model`
- `conf::st_6b_resume_invariant_holds_at_every_byte_offset_of_a_corpus_with_partial_escape_sequences`
- `conf::st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut`

The remaining Scope 2 controls from brief-p6-testkit-scope2.md remain pending for the P1/P3 injected handles and dispatch.
These include identity, start holds, storage faults, spawn faults, scheduler controls, quiet fences, and wake controls.
The candidate manifest reader, OwnedGroup, refusal layer, and earlier edge handles already exist.

## Exact next step

1. Read the WIP helper and the anchor ruling above before editing.
2. Finish the harness guard and the helper's prebuild/manifest integration on stage1/p6-real-harness.
3. Implement RealCoreHarness over actual Core::open and guarded verified worker/probe wrappers.
4. Add the slow conformance harness without replacing the default testkit suite.
5. Prove normal production reaping, panic cleanup, parent death, and group-move refusal on Linux.
6. Run the required single Mac cleanup proof through the gate tool when the implementation is ready.
7. Send the exact head to both package and integration reviewers. This PR touches cross-package test infrastructure.
8. Run the full Linux gate only after both exact-head reviews are CLEAN.
9. Add the independent argument adapters in the next stacked PR. Claim no live terminal controls.
10. Wait for P3 M2b before live oracle dispatch and actual capture integration.

No pending id leaves the list in the currently authorized integration phase.

Run gates and heavy jobs through botster-gate on Linux, one at a time.
Never change a timeout, transcript, or expected terminal byte.
Keep git prompt-free. Never force-push or use git branch -D.
Never kill a process by name or pattern.
Never poll. End the turn while waiting. Call receive_messages once after a doorbell.
Report to the lead only QUESTION, BLOCKED, or a terminal event.

The prior .gitignore spawner restriction was revised by the lead during this phase.
If the spawner removes ignored artifact lines again, the lead permits restoring .gitignore before a gate.
Never commit that spawner deletion. Botster-gate runs the committed head; use no detached gate-worktree workaround.
