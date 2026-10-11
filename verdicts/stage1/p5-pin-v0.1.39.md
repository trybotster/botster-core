# Core #282 — contracts .39 pin

## Round 1 — source and evidence review, 2026-10-11

Verdict: source CLEAN (0 open) at `2866d5928f66954cf57e0a4959db2e587a543080`.
Base: `20c4e98df6b47a45f3e2284a976e1c8a1341457e`.
Tree: `588eadc84b92b6d2940850da76a7f9e02d54af8a`.
Risk: HIGH. Package CLEAN: `631116bf363d4435b159dd65eccfc808f5b26499`, verdicts/p5-pin-v0.1.39.md, complete Round 1.
Sol reads that complete record, all eleven own changed files, the corrected full body, relevant source context, and retained evidence.
This is source and evidence acceptance. The first gate remains red. This verdict does not approve a merge with a red gate.

### Pin and host adaptation

The seven manifest dependencies move in one step from contracts .36 to .39.
Old commit: `96f48d6d3d7a0e75e752d78433eff92efad0e77e`.
New commit: `ddce507e2c08e740f899e97dcfec566f3d2275b8`.
The .39 annotated tag object is `de6f5d0912da2eae2209e883f9ddea497a835e05`.
The lock changes nine source lines and adds the existing rustix dependency edge for botster-core-conformance.
No dependency version or Ghostty pin changes. The body now distinguishes the two commits and the tag object.
The body lists all nine intervening Contracts PRs and their changes, including R-66 through R-70 and R-43(5).
Earlier Contracts verdicts remain separate. This verdict accepts no later Contracts ruling or .40 pin.

RealEdges::wake_descriptor returns an owned duplicate of its current mio Registry descriptor.
That Registry also supplies the existing wake descriptor. The accessor creates no separate wake source or unsafe code.
The caller owns the duplicate. The method has a public doc comment and API snapshot entry.
The lead approved this public Core API addition for the real harness. It is not a machine action used only by tests.
The unit proof checks a different descriptor number, initial non-readiness, signal readiness, and clearing after consume and drain.

The tapped real harness stores the duplicate in the current handle entry.
CrashHandle::is_live checks the weak handle and current crash slot. wake_fd returns None after crash or drop.
The plain harness has no tapped entry. A reopened name uses its new handle entry.
The real proof checks Create readiness, clearing after pumps, and None after crash or drop.
The duplicate has no strong ownership of Core's data-directory claim. The existing weak tap remains weak.

RealRoute::readiness returns the current stream descriptor when reads are open and ungated.
The expired-deadline path permits one nonblocking read attempt. An empty attempt returns Empty.
The new route proof checks readiness after worker output, expired empty read, gating, ungating, and a closed end.
Sol reads the new proof and the surrounding read paths. No fake input or separate scheduler is added.

### Gate copy and exact source composition

The complete job-common.sh copy equals the pinned Contracts file byte for byte.
SHA-256: `ab191b18e47e04d05b7f3946bfe4c02c1e645a27117fb2766711693f184ecaaf`.
Core's wrapper retains its settings values and all four artifact paths.
The shared executable body changes its artifact loop to read the settings array. Cleanup, locking, and disk checks remain.
The wrapper sources its adjacent copy. The lists check compares complete file contents and rejects read errors or byte differences.
The ledger write path refreshes the copy. Sol does not execute the owner's appended-byte check.

The A5-4 source head is `dd30a41ee8b87e9f1b3e7ce883c6d6a2f0d834c4`, based on `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
The final head imports #276 through the exact base above. Three base paths change; only real.rs overlaps the eleven own paths.
Sol reads the complete four-line overlap: worker_file_name capability arm, two comments, and capability test entry.
All ten other own blobs equal the A5-4 source head. Both non-overlapping imported blobs equal the base.
Sol checks all 338 tree entries. No unexpected entry differs from the base plus those own blobs and the reviewed overlap.
The exact parents are dd30a41e and 20c4e98d. This source composition accepts no later import.

### Pending removals and repeated evidence

The own real-pending diff removes thirteen identifiers and adds none:

- conf::dp_7_revoked_reason_reaches_client_and_host
- conf::ev_5_poll_wakes_pending_work
- conf::ev_5_route_closed_retires_route_keyed_events
- conf::ou_2b_handoff_failure_is_reported_as_handoff_failed
- conf::ou_2b_healthy_reasons_send_route_closed_last
- conf::ou_7_exit_after_last_frame_then_close
- conf::ou_7_exit_after_tail_queued_then_close
- conf::st_6b_cut_inside_an_sgr_sequence_applies_the_suffix_not_prints_it
- conf::st_6b_model_after_baseline_plus_output_equals_the_sessions_model
- conf::st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut
- conf::tm_3_wake_handle_ignores_deadlines
- conf::tm_6_create_without_worker_input_wakes
- conf::tm_6_poll_room_unparks_and_wakes

Sol parses all 140 N-run result records. Each removed identifier passes five of five on testkit and real tiers.
The fourteenth candidate, a2_2_missing_op, passes five of five on testkit and fails five of five on plain real Core.
It remains real-pending. Its wrapped-composition PASS does not replace its plain-Core failure at step 18.
tm_6_begin_and_attach remains real-pending: its pump posts one event at step 21 where the transcript expects zero.
The testkit pending file has no own change. The body preserves dp_3's later route-input gap and both held identifiers.

N-run head: `bf17702ef303b794f9c93ef0dfc869b6cf142f6b`. The Linux node uses kernel 6.12.
Log: `shared/core-stage1/evidence/p5-r68/pin-nrun-linux-bf17702e.log`.
SHA-256: `eb5292f8f948788edaadd2a4a6033577da744b96ff6bc206b28ec6c940ccbb4a`.
The N-run precedes the oracle and worker-file-name imports. The pin, wake accessor, crash helper, and route adapter stay identical.
Sol claims no final-head repeated run. All thirteen removals PASS in the exact-head slow step and stage-2 baseline.
The Mac run did not start because of its disk floor, according to the owner. Sol does not independently verify that failed job.
The lead requested Linux evidence. No exact-head Mac execution is claimed.
The N-run header now says fourteen candidates and thirteen removals. Its correction changes a comment only.

### A5-4 tables and corrected detail ownership

The final comparison uses .36 at 84b1e9be and .39 at dd30a41e. Both jobs use the Linux 6.8 node.
Sol reads both scripts and raw result records. Their filtered pipelines do not preserve the underlying Cargo status.
Sol uses raw outcomes, not wrapper success, to determine PASS and failure.
The combined log exactly equals the two original logs concatenated in order.
Sol independently parses every cell in both corrected tables. No cell differs from the raw record and its owned failure details.
The raw must run has 690 unique identifiers in each tier and tree.
Each table displays 598 identifiers. All 92 omitted identifiers are ignored; none passes.
Real omits 70 no-transcript, 18 withdrawn, two deferred, one testkit-proven, and one real-only identifier.
Testkit omits the same first 90 and two real-only identifiers.

Real: must-pass PASS 177 to 191; pending-real PASS three to one; nineteen changed rows.
Testkit: must-pass PASS 212 to 212; no changed row.
The real table retains ST-6 PASS to FAIL at step 17, seed 1.
The earlier .36 pair fails ST-6 at step 17, seed 5, while its .39 side passes.
AD-3 and IN-9 also have failures in earlier .36 records. These records support an existing intermittent failure, not a causal exclusion proof.
P5 owns the separate ST-6 structural diagnosis. This verdict neither removes nor closes that work.

Package R282-E1 closes after the parser resets detail ownership at each result and run boundary.
Pending a2_2_cancel_race details no longer attach to the preceding must-pass failure.
Sol reads the corrected parser and independently verifies every complete displayed cell, including OU-9 and ST-6 details.
Parser SHA-256: `47a7d678efbbd4a564124e577e82673fc0a7adf9ec09fc92c305d7b1ea5854d1`.
Real table SHA-256: `64ea8ad0c9f4cdbc2bdac45c51640fd06dd927e34465e5ceec24dce23bc22358`.
Testkit table SHA-256: `423f227e789248f09d41fb8842387e88fe248b7f1b8298bdf2ad8aa1b20e65c9`.
Before raw SHA-256: `78052972ca7f8864eafa84cb0d4e329ec1493c088469e2a64b84b95ea9c3f34d`.
After raw SHA-256: `d94ae24c8bd367c48d37486a76e284e14145a41391d994678ab4672cc041985a`.
Combined SHA-256: `b077d2b1e1c0d1243c941a86748ae3a1479d911464ccba613d3efde320a79e18`.
Artifact root: `~/botster-sessions/shared/core-stage1/evidence/p5-r68/`.

### First red gate and IN-9 attribution

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-pin-v0.1.39-2866d592-pool-20261011-105652-38426.log`.
SHA-256: `7da530c7ccf070fb5cac3063c0991f09576d1969c7076379df0e9884367a5ba7`.
Sol reads its exact-head and base headers, step summaries, suite summaries, all twelve stage-1 records, and baseline failure.
Eight steps PASS. Mutants FAIL. Fuzz does not run. Total step time is 396.2 seconds.
Default: 1739 PASS, 478 skipped, 12.609 seconds. Slow: 572 PASS, 2016 skipped, 42.146 seconds.
The three new wake and readiness proofs PASS. Minimum: testkit 60/69; real 53/68; real accepted 54/69.
Stage 1: twelve mutants, zero caught, ten missed, two unviable, zero timeout, 77.2 seconds.
The unmutated stage-2 baseline has 571 PASS, one failure, 2704 skipped, 48.643 seconds; baseline duration 64.8 seconds.
IN-9 fails at seed 0, step 8, step_timeout, 10.226 seconds.
No stage-2 mutant result or final catcher record exists. No mutation acceptance follows from this gate.
Job exit is 1 after 412 seconds; queue one second, run 411 seconds. Wrapper exit is 1 after 412 seconds.
The body reports a manual wrong-Registry reversal. Sol claims no separate raw-log verification of that local control.

Sol reads the plain-v1 IN-9 failure at b6fd4b98, .36: seed 7, step 8, step_timeout.
Log: `gates/botster-core-detached-b6fd4b98-b6fd4b98-pool-20261011-102704-34139.log`.
The filtered A5-4 wrapper exits zero despite that suite failure. This review preserves the raw failure.
The complete IN-9 transcript is byte-identical at .36 and .39.
SHA-256: `c09bcc3c142bcb49f94604f9099c511c76f307d25ab5acd87d291fb3662db969`.
Step 4 binds the model revision. Step 5 writes output. Step 6 makes one plain pump.
Step 7 submits guarded WriteInput. Step 8 waits only for NotWritten(Stale).
There is no waiting output fence. Sol reads the pump driver and unchanged worker output and guard paths.
Core can check the guard before the worker consumes output. A successful write cannot satisfy the stale-only wait.
This causal path is a source-based inference. The failure logs do not trace actual Written completion.
Sol confirms that #282 leaves the diagnosed missing fence unchanged. The new descriptors do not add a transcript fence.
The lead assigns the Contracts fence repair to P5 under R-77. That repair is outside this head.

### Remaining work and evidence limits

The package reviewer closes R282-E1 and four LOW record corrections. Sol reads all corrected records; zero source findings remain.
Old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` supplies no corresponding contracts pin or gate wrapper.
Its four as_fd calls are plugin IPC socket sends. Sol reuses no old code here.

The lead requires no separate re-gate of 2866d592. After both source CLEANs, P5 sends FINAL-READY and waits for SLOT.
P5 then imports current v1 once, supplies the applicable complete check, and runs one final exact-head gate.
Any failed check requires its reviewer deltas. The final mutation and fuzz results, body, and package completion remain required.
If that final gate fails only on the same IN-9 signature, the lead's plan 23al one-re-gate path applies.
This is a narrow retry condition, not permission to repeat until green.
Sol accepts no future import, gate, pin, removal, or merge readiness.
Sol ran no build, test, gate, measurement, mutation, reversal, or executable base check.
