# PC package review: Core #307, control queue

## Round 1: VERDICT: NOT CLEAN (2 open)

Reviewed head: `614cc30dabd59a7d8d5a770ff0eb74991bb240cb`.
Source base: `e4f495040a80b524a7ac9accba95272d5c2594fa`.
GitHub base and gate comparison ref: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
Branch: `stage1/pc-control-queue`. Risk tier: HIGH, as stated correctly in the body.
Reviewer: `sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7`.
READY receipt: `msg_plugin-w_1791780222_a9016a`.

The reviewer read the binding brief first and followed its reading order.
The review uses BUILD.md, plan 23cm (`stage1-plan.c77c2434.md`), the pinned contracts, and the lead's R-91 assignment.
R-100 and lead receipt `msg_plugin-w_1791780836_98917e` dispose the sizing question.
This review covers all 19 changed files, the complete body, the import resolution, and completed evidence.
No reviewer build, test, gate, mutation, reproduction, or executable base-check job ran.

### CQ-PC-1 — MEDIUM, OPEN: a failed write side can post a false room wake

Locations: `driver.rs:229`, `driver.rs:582`, `driver.rs:649`, `engine.rs:321`, and `inbound.rs:779` in `botster-core-host`.

1. A cancel refused `Full` sets `worker.refused_full`.
2. A later fatal send calls `close_write`. The driver clears `out` and retains the read side.
3. `on_link_write_failed` sets `write_failed`. It retains the refusal mark and the session's link.
4. `service_links` calls `control_room` after the failed flush.
5. The room calculation sees zero unsent bytes. `waits_for_room` accepts the retained mark.
6. Core posts `SessionWritable` for a permanently failed write side. With no pump budget, Core instead reports this as `more`.

IN-6 and SV-3 require room to return. Discarded bytes on a failed write side do not provide room for another control message.
R-78 requires the failed write side to take no further writes while Core retains and reads the read side.
The finding follows from source. The reviewer did not measure this sequence in a reproduction run.
Sol independently records the same finding as R307-SI-1.

Required change: retire the refusal mark when the write side fails and exclude that side from a new `Full` refusal.
Prove that `Full` followed by a fatal send produces no false `SessionWritable` or room-related `more`.
The retained read side must still accept `Done` and complete an in-flight request.

PC reports a repair at `973507ab9e7268165a38cde9d85f4032878f9b64` in receipt `msg_plugin-w_1791780760_5f0fe9`.
That repair is outside this READY head. This review does not accept the repair or close the finding.

### CQ-PC-2 — MEDIUM, OPEN: the sizing estimate does not meet R-91

Locations: `driver.rs:204-219` and DESIGN.md, section "The control queue".

`control_bound` uses `NON_INPUT_FRAME_BYTES = 4096` for each non-input message.
The source and the body explicitly permit larger `AttachRoute` and `UpdateMetadata` messages.
R-91 requires the largest non-input host message frame as the formula's term, with headroom above admitted traffic.
The 4096-byte estimate does not meet that requirement.
The current unit test checks the formula with the same estimate. It does not establish the required frame bound.

Lead receipt `msg_plugin-w_1791780632_f92dc1` rejects an estimate exception.
R-100 at contracts main `9e7eca0293d4fac17e579d4500d08aab9f9b84c2` then permits the link frame bound for this term.
Lead receipt `msg_plugin-w_1791780836_98917e` specifies `header + link_frame_bound` for each non-input message.
The link edge supplies the effective room limit. DESIGN.md must state the bound.

Required change: use the permitted frame bound in the formula and update DESIGN.md and the body.
Keep new `CoreLimits` fields out of v1. No further steward request is needed for this finding.
The oversize-at-call checks from R-100 belong to the separate RC PR, as the lead assigned.

### Source, import, and contract checks

The own diff has 620 insertions and 12 deletions across 19 files. It changes no pin, manifest, or mutation configuration.
The control acts through `HostEdges::link_room` in both tiers. Production logic contains no test-mode branch.
The refusal check preserves `UnknownOp`, `TooLate`, `NotCancellable`, and `AlreadyAdmitted` precedence.
The check occurs before the sent/not-sent branch, as R-91 permits.
The wake uses the instance-keyed event queue. Repeated refusals retain one mark.
The driver counts the wake against the pump budget. A link without a refusal mark supplies no room work.
The driver tests exercise its actual unsent buffer through an injected blocked send and subsequent drain.
These checks do not close CQ-PC-1 or CQ-PC-2.

Merge `4fd8f2eb87b95b2b9cbd01876510a499efd41c13` has parents `7749e5c96f5d374c92b84e7d95b4cc975aaf62a7` and the source base.
The complete conflict resolution retains `next_exit`, `link_ends`, `spawned`, and the control queue fields.
The later `eb37555a` change repairs false `more` for links without a refused cancel after that import.
The independent three-way pending-set check uses merge base `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
It finds 408 expected and 408 actual ids, with no missing or extra id.
The own diff removes only `conf::in_6_cancel_result_kinds` and `conf::in_6_refused_full_wakes_on_room`.
All eight other conformance text blobs match the source base. EV-9 stays pending under R-96. R-93 fairness debt stays pending.
The pinned IN-6 transcripts support the control's argument and event order. This PR does not move the contracts pin.

The removed `Observation::Writable` has no production sender at the source base.
The remaining old uses are the host mapping and its test, which this PR removes.
The prior-art source supports the body: the old daemon returns a Boolean cancel result without this queue or wake.
The old worker's egress bound applies in the opposite direction. This PR copies no code from that implementation.

### Completed evidence audit

Exact-head gate: `~/botster-sessions/gates/botster-core-stage1-pc-control-queue-614cc30d-pool-20261011-213222-45073.log`.
SHA-256: `dedecaad1a1e92ced19f86f88c26c3710f97a976caa5f912ec35223e1537dde8`.
The retained `gate-client-614cc30d.out` is byte-identical to that log.
The gate ran on MS-A1, Linux kernel `6.12.111+deb13-amd64`, with four CPUs.
All ten checks PASS in 626.5 seconds. The job and wrapper exit zero.
Default tests: 1895 PASS, 431 skipped. Slow tests: 615 PASS, 2041 skipped.

The raw mutation records contain 57 distinct first-stage outcomes: 32 caught, 13 missed, and 12 unviable.
Stage 2 catches all 13 first-stage misses, with zero missed, timeout, or unviable outcomes.
All 45 raw catches match their named citations. Every named test has a normal PASS record.
The 13 first-stage miss identities exactly match the 13 second-stage catch identities. The log has no FLAKY record.
The 12 unviable outcomes are build failures, not missed mutants.
Their identities are three `CancelResult` default replacements, four `HostDriver::open` constructor replacements,
four `MutexGuard` constructor replacements, and one `instance_of` default-instance replacement.
The audit does not count these outcomes as behavior proofs.

Fuzz runs execute both changed link decoders: `link_decoder` completes 8090929 runs, and `msg_decoder` completes 4691312 runs.
Each run lasts 61 seconds. The fuzz check PASS is retained.

The earlier `c48446bc` gate remains RED: stage 2 misses seven mutants.
Six miss identities concern the testkit control. The seventh changes the redundant engine room predicate.
The current source adds a behavior test for the control and reuses `waits_for_room` in the engine.
The retained manual record on `614cc30d` reports that each of the six testkit mutations fails the new test.
The completed current gate independently catches all six. No old failure is erased.

The local `614cc30d` record reports host 336, link 52, testkit 427, and worker-core 192 tests passed.
Strict conformance at seeds 0-31 reports 260 passed and zero failed.
The script records a target clean at `2026-10-12T04:24:35Z` and exports both resource caps.
The script filters pipeline output. The retained summaries support the stated counts; the exact-head gate supplies complete pass/fail checks.

The completed `eb37555a` evidence reports both IN-6 ids PASS at strict testkit seeds 0-31 and real seeds 0-7, eight real passes each.
The script records a clean at `2026-10-12T03:37:18Z` before prebuilding the worker.
That seed evidence predates the current mutation-test change and is not an exact-head local run.
The current gate provides normal PASS records for both ids on both tiers.
The retained reversals report failures when the room check or wake is removed.
The `eb37555a` reversals report the three imported TM-6 failures without the mark check and the new deadline failure without `more`.
The earlier local taint failure on `10653a40` remains recorded. The DESIGN.md correction at `7749e5c9` passes its retained rerun.

The reviewed live body SHA-256 is `c312c5822dca598e58fdfe71f9cfe629cfba0078c15b2d56b37589b121338144`.
The body correctly distinguishes the source base from the gate comparison ref and preserves the earlier RED gate.
Its sizing claim must change with CQ-PC-2. No completed gate passes either source finding by itself.

### Next review

PC must send a combined READY head with both repairs and the required completed evidence.
The next review must check the complete repair diff and its composition with the retained read-side behavior.
This verdict accepts no later head and grants no merge approval. Wait for the doorbell. Never poll.

## Round 2: VERDICT: NOT CLEAN (1 open LOW)

Reviewed head: `5139373242b37638cabf1017ab471124c2330adf`.
Tree: `dcbefc16d2986aa6fa2640a24bae856e45dfb7c2`.
Source base remains `e4f495040a80b524a7ac9accba95272d5c2594fa`.
GitHub base remains `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
The gate comparison ref is `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. This round imports no later v1 source.
READY receipt: `msg_plugin-w_1791783700_3cf4ef`.
The reviewer read the brief first and read the current common rules.
The complete repair diff has seven files, 368 insertions, and 27 deletions. All changes are in `botster-core-host`.
The reviewer read every repair, test, and documentation change, the affected failure paths, and the completed evidence.

### CQ-PC-1 — CLOSED on this head

`973507ab9e7268165a38cde9d85f4032878f9b64` clears `refused_full` in `on_link_write_failed`.
The failure feed occurs before `service_links` checks room, so discarded bytes do not supply room work.
`cancel_with_room` excludes `write_failed` from the `Full` branch. It cannot create another refusal mark on that side.
The existing driver and engine send guards still prevent further writes. The repair retains the read side and in-flight requests.
The regression checks `Full`, a fatal send, a spent pump with no false `more`, no `SessionWritable`, and a subsequent `Done` completion.
The second regression checks an admitted cancel on a failed write side while the injected edge reports zero room.
Both tests PASS in the exact-head gate.

PC receipt `msg_plugin-w_1791783933_4e23ae` replaces the older reversal evidence.
The complete `red-on-revert-v2.sh` and `red-on-revert-51393732.log` name this head and an initially clean worktree.
The target clean exits zero at `2026-10-12T05:43:58Z`. The record states that this worktree is the sole writer afterward.
Each one-change reversal shows its source diff and fails only its corresponding regression: 347/348 pass, nextest exit 100.
The restored clean worktree passes 348/348, nextest exit zero.
The script filters nextest output. Its summaries, failing names, source diffs, and numeric exits remain visible.
The older `973507ab` reversal lacks a clean record and numeric exits. It stays historical and is not the accepted replacement proof.

### CQ-PC-2 — CLOSED on this head

`c938caaf367b2a5ae150428257da5b40ff333913` replaces 4096 with `largest_frame(frame_bound)`.
That function returns `HEADER_LEN + frame_bound`. The driver obtains `frame_bound` from `HostEngine::link_frame_bound`.
`HEADER_LEN` is five. `encode_frame` rejects an oversized payload before it appends any bytes.
Every accepted frame therefore fits the term, including message fields without a `CoreLimits` bound.
This is the R-100 option that the lead permits. The room calculation still takes the minimum with `link_room`.
No new `CoreLimits` field or runtime admission rule is added.

The ten new tests cover the host message kinds and the forwarded non-input operations.
They check bounded fields with large default-limit values and check encoder rejection for the four kinds with unbounded strings.
The formula test checks the default frame bound of 65 MiB and four cancel frames.
All ten tests and the formula test PASS in the exact-head gate.
The oversize tests exercise the frame encoder. They do not prove the separate R-100 at-call refusal work assigned to RC.
The runtime sizing repair closes CQ-PC-2. The documentation error below is separate.

### CQ-PC-3 / R307-SI-3 — LOW, OPEN: the stated derivation omits the frame cap

Locations: DESIGN.md, section "The control queue"; the body's bound derivation; and the `small()` comment in `control_term.rs`.
Sol receipt `msg_plugin-w_1791783927_115ab4` identifies this error. The package reviewer confirms it in source.

DESIGN.md and the body state `F = 4 × max(max_paste_bytes, max_snapshot_bytes) + 1 MiB`.
`HostEngine::link_frame_bound` uses saturating arithmetic and clamps the result to `LINK_FRAME_CAP = u32::MAX / 2`.
The stated equation is incorrect when the cap applies. The production term correctly calls the engine method.
The test comment states 1 MiB plus 8 KiB. Both input fields are 1024, so the actual bound is 1 MiB plus 4 KiB.

Required change: state the cap and saturating calculation in DESIGN.md and the body, or name the engine method without an uncapped equation.
Correct the test comment to 1 MiB plus 4 KiB. No runtime sizing change or broader implementation is required.

### Complete gate and body audit

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-pc-control-queue-51393732-pool-20261011-221503-34203.log`.
SHA-256: `e867fc5768a40ed812188bfc8803a829a8f5d92da1932fd9bb1657f7c527b461`.
The retained `gate-client-51393732.out` is byte-identical.
The gate runs on MS-A1, kernel `6.12.111+deb13-amd64`, with four CPUs. All ten checks PASS in 643.8 seconds.
The job and wrapper exit zero after 653 seconds. The job queues for one second and runs for 652 seconds.
Default: 1907 PASS, 431 skipped, 15.839 seconds. Slow: 615 PASS, 2041 skipped, 42.675 seconds.
Both un-pended IN-6 ids PASS in both normal tiers. The conformance lists and pins remain unchanged from Round 1.

All 64 first-stage mutation identities are distinct: 39 caught, 13 missed, and 12 unviable.
The 13 missed identities exactly match the 13 second-stage catches. No mutation remains missed or timed out.
All 52 raw catches match named citations. All cited tests have PASS records before the mutation step in the normal tiers.
The baseline passes 615 tests, with 11 slow and 2802 skipped, in 45.003 seconds.
The 12 unviable identities match Round 1 after source line shifts. They remain build failures, not behavior proofs.
The completed log contains no FLAKY record.
Fuzz completes 5451331 `link_decoder` runs and 3719385 `msg_decoder` runs, each in 61 seconds.

The complete local script and log record a target clean at `2026-10-12T05:06:13Z` and both resource caps.
They report 348 host, 52 link, 427 testkit, and 192 worker-core tests passed.
Strict conformance at seeds 0-31 reports 260 passed and zero failed. Ten xtask checks report exit zero.
The local script filters Cargo pipelines and retains no numeric clean exit. The complete pool gate independently passes these checks.
The prior `c938caaf` record retains two test-only clippy errors. `51393732` changes only those test expressions and passes clippy.
The reviewer read the complete body and its later evidence-only change on the same head.
The updated body correctly withdraws the old reversal record as accepted proof and names its completed replacement.
Its live SHA-256 is `2ce1055c8f104d6f48b0970a15513361b445a5d83c9c679a1ba09e84da4be77a`.
PC's hash `bd1b3c4fb86b23cd3cb7c663db824066a927bc541b225aff3e6557223074e869` differs only by one final newline.
The earlier RED gate, seed limits, local pipeline limits, and original-head findings remain historical.

One LOW documentation finding remains. Both MEDIUM source findings close on this exact head.
No reviewer execution job ran. A corrected-head delta review remains due.
The final import, applicable checks, final gate, both final reviews, and lead Q7 remain due. This verdict approves no merge.

## Round 3: VERDICT: CLEAN (0 open)

Reviewed head: `50a50cff3fcdbbba1976c3194e0b49183726dbc8`.
Parent: `5139373242b37638cabf1017ab471124c2330adf`.
Tree: `90d10894ec8b19da48921f176abae1baaba771e5`.
Source base remains `e4f495040a80b524a7ac9accba95272d5c2594fa`.
GitHub base remains `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
READY receipt: `msg_plugin-w_1791784031_6a062c`.
Body correction receipt: `msg_plugin-w_1791784143_0a62d5`.

The reviewer read the complete parent-to-head diff. It has two files, four insertions, and three deletions.
DESIGN.md now states saturation and the `u32::MAX / 2` cap in the frame-bound derivation.
The test comment now states 1 MiB plus `4 × 1024` bytes and calls this a small bound.
The delta changes no executable expression. All other blobs remain unchanged from the fully reviewed parent.
CQ-PC-3 / R307-SI-3 closes. Both MEDIUM source findings remain closed.

The reviewer read the complete revised body through its audited changes.
The body labels the gate and reversal records as evidence on the gated repair head `51393732`.
It explicitly states that `50a50cff` adds only the text delta and that the final gate will cover the exact imported head.
The body preserves the completed replacement reversal, the earlier RED gate, and the historical evidence limits.
Live body SHA-256: `bb0312d0c9f0475dc0e3a9592cd54fd3069bfebe2093ca5397740b48cde90dd5`.
PC's retained hash `71d67e67c219e45d181174016ee5f195aa8d46d6646c04ed6a3e658e96c86423` differs only by one final newline.

The complete parent gate and its 52-catch audit from Round 2 remain accepted within their parent-head scope.
The complete replacement reversal record remains accepted on the parent head.
There is no exact-`50a50cff` gate. PC reports focused fmt, taint, and ten control-term test passes on this head.
No separate retained record accompanies those focused reports. This review does not treat them as an independently audited run.
The source-only documentation delta needs no repeated runtime proof before the final import.

All three findings close. The complete package source, body, and supplied evidence review is CLEAN on this exact head.
This is the review before the final import. It is not final merge approval.
The matching formal integration verdict, authorized final import, applicable checks, exact-final-head gate, final reviews, and lead Q7 remain due.
No reviewer execution job ran. Wait for the doorbell. Never poll.

### Matching formal integration verdict

Sol receipt `msg_plugin-w_1791784339_07407f` supplies formal CLEAN commit `ea7dcaa013c28e23c5aaf8762148192e37dc3356`.
The reviewer reads its Round 2 verdict and verifies publication on `origin/stage1/integration-review-sol`.
Both formal CLEANs cover `50a50cff3fcdbbba1976c3194e0b49183726dbc8`, source base `e4f495040a80b524a7ac9accba95272d5c2594fa`, and tree `90d10894ec8b19da48921f176abae1baaba771e5`.
Both close the same three findings and retain the parent-head gate and evidence limits.
The matching formal review condition closes. The authorized final import and all final checks remain due.

### Lead inbox notice

Lead receipt `msg_plugin-w_1791787683_b8c46e` states that the lead can send messages but cannot read its inbox.
The published CLEAN verdicts remain available in the verdict files. This notice changes no reviewed head or review result.
The reviewer will resend the important CLEAN receipt when the lead announces that its inbox works again.
No final-slot authorization is inferred from the notice. No new final-import head or completed final gate has been supplied to this reviewer.

Lead receipt `msg_plugin-w_1791818308_100adb` announces inbox recovery and asks for important messages to be resent.
The reviewer resends the matching #307 CLEAN receipt in `msg_plugin-w_1791818343_b236da`.
The resend preserves the reviewed head, parent-gate scope, and pending final steps. It changes no verdict or authorization.

## Round 4 inspection: current-base import; formal verdict pending

Requested head: `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
Tree: `b117637b809c37c63b869e4da2696015dbdf5918`.
Previously accepted head: `50a50cff3fcdbbba1976c3194e0b49183726dbc8`.
Imported base: `080987829a2e2245a7ea97606020400392c974d6`.
Delta request: `msg_plugin-w_1791819576_774597`.
The lead handoff assigns the conflict repair after #307 leaves train 3 and before its train-4 candidacy.
The reviewer read the brief first, current common rules, and BUILD.md's merge-train rules.

Merge `ceb6b22e73034a281fe613338069d441c8fa5e05` has the accepted head and imported base as its parents.
The complete conflict resolution retains `Faults.control_full`, `start_holds`, its methods, and `HeldLink`.
It retains `Tap.control_full` and `op_holds`, including both constructor initializers.
The reviewer read the auto-merged driver, engine, design, link message, control registry, real dispatch, and module changes.
The imported scheduler filters held operation steps. It retains the driver's independent control-room check and budget accounting.
The failed-write mark retirement and failed-side refusal exclusion remain unchanged.
The imported stop-grace interface removes `LaunchSpec.stop_grace_ms` and uses Core's deadline for `KillPayload`.
The only post-merge change removes that obsolete field from the own launch-size test.
The test still supplies an oversized argv to the encoder. Its frame-bound assertion remains intact.

The own changed-line patch against the imported base matches the accepted own patch, except that test-field removal.
All changed non-own blobs match the imported base. The delta changes 75 paths, including 62 crate paths.
The independent three-way pending-set check uses common ancestor `e4f495040a80b524a7ac9accba95272d5c2594fa`.
Ours removes the two IN-6 ids. The base removes `a2_1_attach_state_rules`, `a5_2_reads_match_atomic_transitions`,
`dp_2_stream_handoff_transfers_ownership_and_closes_host_copy`, and `tm_6_parked_work_does_not_spin`.
The expected and actual pending sets each have 404 ids, with no missing or extra id.
Eight other conformance text blobs match the imported base.
The imported manifests, pin to contracts `.43`, and mutation profile match the imported base.

The completed local record names this head and reports a clean marker at `2026-10-12T15:31:06Z`.
It reports fmt success, ten xtask exits zero, and filtered clippy output with the existing Ghostty warning.
It reports 356 host passes plus one ignored test, 54 link passes, 441 testkit passes, and 193 worker-core passes.
Strict conformance at seeds 0-31 reports 263 passed, zero failed, and 428 ignored.
The existing filtered-pipeline and numeric-clean-exit limits remain. These summaries are not a complete gate.

No new source finding is identified. All three prior findings remain closed in the inspected source.
The complete formal delta verdict remains pending. The reviewer has not read the running gate.
The completed gate, current-head body, and implementer's base-merge-check record remain due.
The observed live body still describes the previously accepted head. The reviewer requests its current-head correction.
This inspection accepts no new head or train gate. No reviewer execution job ran. Wait for the doorbell. Never poll.

### Completed records: RED gate; formal verdict remains pending

PC receipt `msg_plugin-w_1791820358_0a2cad` supplies the completed gate, updated body, and base-merge-check record.
The reviewer reads each complete record. No running gate output is read.

The base check names accepted head `50a50cff`, new head `2ed85f93`, and base `08098782`.
Both ancestry checks PASS. Checks (1), (2), and (3) FAIL for the two conflicts, eleven shared paths, and changed own diff.
Check (4) fails for a pending-file comment and passes for the unchanged real-pending file.
The result says that the merge needs delta rounds from both reviewers. The independent id-set check remains valid.
The numeric `exit` line is empty because the implementer's wrapper loses `PIPESTATUS`. The record discloses that defect.
This record supplies no pure-base-merge exception and carries no automatic CLEAN.

Completed gate: `~/botster-sessions/gates/botster-core-stage1-pc-control-queue-2ed85f93-pool-20261012-083921-28034.log`.
SHA-256: `76c3d6ab0c431804b61587cf62bd3b587c1b5c0b91c2f46ff3c5613cd10dd805`.
The retained client copy is byte-identical. The gate names this exact head and base `08098782` on MS-A1, with four CPUs.
Eight checks PASS: fmt, clippy, taint, lists, public-api, prebuild-worker, test-budget, and slow.
Default: 1955 PASS, 429 skipped, 12.335 seconds. Slow: 621 PASS, 2067 skipped, 41.119 seconds.
Stage 1 has 39 caught, 13 missed, and 12 unviable outcomes. All 39 raw catches match citations and normal PASS records.
The missed and unviable identities match the accepted parent gate after line shifts. The log has no FLAKY record.

The unmutated Stage 2 baseline FAILS `conf::in_8_mode_checked_at_write_start_by_worker`.
Its retained failure is INCONCLUSIVE at seed 0, step 8, `step_timeout`.
The baseline has 620 PASS, one failure, and 2844 skipped, in 46.705 seconds.
The same test has PASS results in the normal default and slow tiers. Those passes do not erase the baseline failure.
Stage 2 mutations and fuzz do not run. The current-head audit cannot establish the thirteen second-stage catches.
The gate total is 525.1 seconds. The job exits one after 708 seconds; the wrapper exits one after 709 seconds.
The job queues for 150 seconds and runs for 558 seconds. The completed gate remains RED.

PC attributes the baseline failure to the known IN-8 transcript race and requests a lead decision on the completion path.
The reviewer reads the pinned `.43` transcript. It uses `pty_blocked` and `pty_output`, with no cancel or `control_queue` step.
The source audit identifies no new control-queue cause. This is source reasoning, not a measured plain-v1 reproduction.
The current head's known-flaky list has no entries. An entry in the separate #308 PR does not authorize a re-gate here.
The reviewer infers no exception or rerun approval and performs no reproduction or gate job.

The updated body's live SHA-256 is `1c0e08fd1bc1341eaba98177709e77abeeef354e7a50d3b014a7c9e429faace7`.
Its new import section correctly attributes `start_holds` to #301 and `op_holds` to #300.
It names the current head and retains the RED gate. The earlier gate stays historical.
The reviewer requests one count correction: eleven paths are shared in total, including the two conflicts; nine other paths auto-merge.
The formal verdict remains pending the corrected body and the lead-approved completion path with the required evidence.
No later train head, completed train gate, or merge is accepted. Wait for the doorbell. Never poll.

### Round 4 complete: VERDICT: CLEAN (0 open; before the train)

Exact head remains `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
Imported base remains `080987829a2e2245a7ea97606020400392c974d6`.
Tree remains `b117637b809c37c63b869e4da2696015dbdf5918`.
The previously accepted head is `50a50cff3fcdbbba1976c3194e0b49183726dbc8`.

PC receipt `msg_plugin-w_1791820521_1bc686` supplies the lead's explicit completion decision and corrected body.
The reviewer independently reads the lead's 02:05 PDT handoff entry.
The lead prohibits a re-gate of this head. Train 4 must include #312's `.44` pin before #307.
The train gate supplies the required final gate. This decision does not turn the current RED gate into a PASS.
The earlier `d6f664f7` package record is a source inspection, not a formal CLEAN verdict. This section completes the formal delta review.

#### CQ-PC-4 / R307-SI-4 — LOW, CLOSED: shared-path count and outcome labels

The prior body said eleven other paths auto-merged but listed nine. The canonical check counts eleven shared paths in total.
The body now states two conflicts and nine auto-merges. The count agrees with the retained base check and source review.
It labels the two positive IN-8 outcomes as normal testkit and real-tier results, before the failed unmutated real-tier baseline.
It retains the RED gate and states the lead's no-re-gate decision and train-gate completion path.
The reviewer reads the full revised body through the audited changes.
Live body SHA-256: `b2f9f44bafa07cc27d8483121095ace9a51043c16f965f228e7020674bbfd506`.
No code changes after the inspected head.

The complete source, conflict resolution, interface composition, lists, body, and supplied completed-record review is CLEAN.
All prior source findings remain closed. The LOW body finding closes. No new runtime source finding is identified.
The base-check FAIL correctly routes the overlapping import to this delta review and the matching integration review.
The current gate remains RED. Stage 2 mutations and fuzz remain unexecuted on this head.
The earlier parent gate is not a substitute for the exact train gate.

This CLEAN covers the exact member head before train construction, under the lead's explicit completion decision.
It approves no train head, current RED gate, final mutation result, or merge.
The matching formal integration verdict, ordered `.44` import, applicable train-delta reviews, combined review,
exact-train-head gate, final evidence receipts, and lead Q7 remain due.
If the train does not include the structural fix before #307, this verdict supplies no alternative completion path.
No reviewer execution job ran. Wait for the doorbell. Never poll.

### Round 4 body follow-up: CLEAN at 4fb39c62 is superseded

Sol receipt `msg_plugin-w_1791820664_935ba7` identifies one remaining stale reference in the current-gate paragraph.
The body still places #312 in train 3 and presents #308's known-flaky entry as an active claim.
The lead's current path requires #312 before #307 in train 4. The tested head's known-flaky list is empty.
The reviewer published `4fb39c6210c3170bbeed090a69734bdec7e4cae7` before closing this reference.
That CLEAN is premature and superseded. CQ-PC-4 / R307-SI-4 remains LOW open until the body correction is read.
The reviewer requests the current fix order and removal of the active entry claim, or an explicit historical label.
No source, gate, or rerun change is required. All runtime source findings remain closed.

### Round 4 final body closure: VERDICT: CLEAN (0 open; before the train)

PC receipt `msg_plugin-w_1791820755_33a438` supplies the final correction on unchanged head `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
The reviewer reads the corrected body and compares it with the previously read complete body.
The only new change places #312 before #307 in train 4 and states that this head's known-flaky list is empty.
It labels #308's entry as a separate historical proposal. It retains the current RED gate and the no-re-gate decision.
Live body SHA-256: `05305586c1683a2d42fbdaa1988e3eee625e7e9310bbeea1128d39f60f412587`.
Sol receipt `msg_plugin-w_1791820779_6e5268` independently confirms the same body correction and lead decision.

CQ-PC-4 / R307-SI-4 closes after this complete body read. All other findings remain closed.
This completed source, body, and supplied-record verdict replaces the premature CLEAN at `4fb39c62`.
It covers exact member head `2ed85f93`, imported base `08098782`, and tree `b117637b809c37c63b869e4da2696015dbdf5918`.
The verified lead decision still requires #312's `.44` pin before #307 and final evidence from the exact train gate.
The current gate stays RED. Its thirteen Stage 2 mutants and fuzz remain unexecuted. No final gate or merge is accepted.
The matching formal integration verdict and all ordered-train reviews, checks, gate, final receipts, and Q7 remain due.
No reviewer execution job ran. Wait for the doorbell. Never poll.

### Matching formal member-delta verdict

Sol receipt `msg_plugin-w_1791821012_3510cc` supplies formal CLEAN `db0e256a1d71f1312b7b121c74544e491106ddfc`.
The reviewer reads its complete Round 3 verdict and verifies publication on the integration branch.
It matches package verdict `ad4750f9cc30669c2d431814e75c6a4cbb653d55` on exact head `2ed85f93`, base `08098782`, and tree `b117637b`.
Both close all findings after the final body correction and preserve the current RED gate and canonical FAIL.
The matching formal member-review condition closes. The ordered `.44` import and all final train conditions remain due.

## Round 5: VERDICT: CLEAN (0 open; #307 train delta before the gate)

Train head: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`.
Train base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Tree: `4bb9e8c7f7d6e8b610e4b80ce782499ae3389cde`.
Accepted #307 member: `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
Lead request: `msg_plugin-w_1791821464_8574ec`.
The reviewer reads the binding brief first. The current merge-train rules require this overlapping member-delta review.
This verdict covers #307's source and contract composition on the exact train head before its final gate.

### Order and source preservation

The first merge is #314 at `b3f7f39b72bc2a985634240573b2d08660b162c6`.
The second merge is #312 at `13fc69a0a196f75a6aaa1662c1deac1817e5dd88`.
The third merge is #307 at the reviewed train head.
The second merge includes member `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64` and the `.44` pin before #307, as the lead requires.
The train manifests and lock name contracts commit `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea`.

The reviewer reads the complete eleven-file delta from the accepted member to the train.
Every own changed line against the pre-member train parent matches the accepted own patch against base `08098782`.
All control-queue runtime blobs are byte-identical to the accepted member.
The cancel precedence, failed-write mark retirement, failed-side refusal exclusion, frame-bound formula, and room wake remain unchanged.
Both testkit tiers retain their separate control-room and scheduler-hold state.
The train member merge has no combined-resolution delta. Every changed non-own blob matches its pre-member train parent.

#314 changes the seeded-trace test to same-head determinism under the lead's recorded ruling.
It changes no driver executable expression and no control-queue assertion.
#312 updates the route-log test to find its read step by key after transcript fences move the index.
Its probe adapter adds only the new `Report::Printed` match arm.
The real harness supplies the control-socket variable and does not enable `PRINTS_ENV`.
The reviewer reads the new probe request/report interface and output-order path.
With print reports disabled, prints do not wait for `Recorded`; `Output` still queues bytes and acknowledges the request.
The new report does not create a control-queue event or change the existing program control arguments.
The Core contract crate has no source change between `.43` and `.44`.

### Transcripts and lists

Both own IN-6 transcripts are unchanged between `.43` and `.44`.
The IN-8 transcript adds the R-74 held-write fence: one kernel byte, a 40960-byte fill, and a `pty_queued` wait before the mode-sensitive write.
The required `pty_queued` control is already served. The pin changes no expected IN-6 value.
This source check confirms that the ordered pin contains the named structural fix. It is not a measured successful train run.

The reviewer independently checks all nine conformance sets at each of the three train merges.
Each set equals the three-way additions minus both sides' removals. No missing or extra id occurs.
At the #307 merge, the two IN-6 removals compose with #312's deferred-completion removal.
Final testkit pending is 403: pre-member 405 minus the two own ids.
Final real-pending is 34 and retains all nine removals from #312. #307 adds no real removal.
Held has four ids, real-only 26, real-proof entries seven, ledger 691, and minimum 69.
Eight conformance text blobs equal the pre-member parent. Only the own pending removals differ.
The train introduces no own pin, mutation exclusion, public signature, or gate-decision change from #307.

### Scope of this CLEAN

All four package findings remain closed. No new #307 source, interface, or list finding is identified.
The member PR body remains scoped to its member head and its historical RED gate. The train review uses the lead's exact composition record.
The lead reports host/testkit/link compile pre-checks PASS and strict seeds 0-31 at 264/0.
No separate retained pre-check log is supplied here. This review treats those results as operator reports, not an independently audited gate.

The complete #307 train-delta source review is CLEAN on `03709a0c`.
The matching formal integration member verdict, other member reviews, and combined review remain due before the gate.
The exact train gate must complete the unmutated baseline, Stage 2 mutation proof, and fuzz omitted by the RED member gate.
Final evidence receipts, lead Q7, and an equal-tree merge receipt remain due. No later train head or merge is accepted.
No reviewer build, test, gate, mutation, reproduction, or executable base-check job ran. Wait for the doorbell. Never poll.

### Matching formal train-source verdicts

Sol receipt `msg_plugin-w_1791822093_81313e` supplies commit `584b736e3dc90dc0114e573d9f4ed2372232d0ca`.
The reviewer reads its complete #307 Round 4 verdict and complete train-4 combined Round 1 verdict.
Publication is verified on `origin/stage1/integration-review-sol`.
The formal #307 verdict matches package `76ae3ad37fc67b47cf2fbaea794cb77554b526d1` on the exact train head, base, and tree.
The combined verdict also has zero open source findings. It reports all six exact-head member source receipts read and CLEAN.
The matching #307 formal source-review condition closes. The combined source condition has its published integration verdict.
No completed train gate is accepted. Final member evidence receipts and lead Q7 merge/tree/push verification remain due.

## Round 6: VERDICT: CLEAN (0 open; train-4b base delta before the gate)

Exact train head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
Previously reviewed train: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`, base `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Lead request: `msg_plugin-w_1791825491_f03c1f`.
Scope: #307's delta from the landed #317 base change only. The reviewer reads the brief first.

The reviewer independently compares both complete full-index diffs.
`03709a0c..e37b91d6` is byte-identical to `838e4bb0..9fe97f50`.
Both diffs have SHA-256 `7d1572807eb38d9abd05ebd26e4c76db5cbc27bf48e2674089308f1b85b8dde6`.
They change only `ci/known-flaky.toml` and one test in `xtask/src/known_flaky.rs`.
This train is rebuilt on the new base. It does not descend from the old train commit; preservation follows from the content comparison.
The ordered members remain #314, then #312's `.44` pin, then frozen #307 member `2ed85f93`.

The reviewer reads the complete two-file change and its existing parser, filter, and baseline context.
The single entry names only real-tier `conf::ev_2_mandatory_and_keyed_never_dropped`.
Its diagnosis names the pre-.45 fence gap, the queued acknowledgement, the link-only quiet check, and contracts #89.
The signature fixes the binary, id, step 71, and `step_timeout`; only the seed varies.
The owner is RC and the removing PR is #316. The governing R107-SI-2 permits this entry under the train's `.44` pin.
The corrected ruling makes removal depend on the later fenced pin, not on A31's separate work.

The changed xtask test parses the repository list and requires exactly that one binary/test pair.
All production parser, validation, filter, reporting, and stale-entry logic remains unchanged.
Mutation stages exclude the listed test only when their own test list contains it. It cannot count as a mutant catch.
The normal slow step and unmutated Stage 2 baseline still run the test. The base change adds no unmutated-failure suppression.
The reviewer reads #317's published accepted-source record and its matching integration receipt for this same two-file scope.
Its earlier gate is evidence on #317's head; it is not a train-4b gate.

All #307 source, test, design, interface, and runtime blobs remain unchanged from the reviewed train.
All nine conformance text blobs remain byte-identical. Testkit pending stays 403 and real-pending stays 34.
The earlier ordered-pin and own-patch checks remain valid under this exact content preservation.
No pin, control, clause expectation, timeout, process allowance, or conformance membership changes in this delta.
The configured mutant exclusions in `.cargo/mutants.toml` remain unchanged.
All four package findings remain closed. No new #307 source or base-composition finding is identified.

The #307 base-change delta is CLEAN on exact train-4b head `e37b91d6` before its fresh gate.
The lead's xtask compile PASS is an operator report. It supplies no completed train acceptance evidence.
Old train `03709a0c` stays RED and is not re-gated. This verdict approves no retry or gate result.
Matching formal member and combined source reviews, the fresh exact-head gate, final evidence receipts, and Q7 remain due.
No later head or merge is accepted. No reviewer execution job ran. Wait for the doorbell. Never poll.

### Matching formal train-4b member verdict

Sol receipt `msg_plugin-w_1791825940_05689c` supplies formal CLEAN `12fccdd1383d096d0accb9be752cd44ea2e3dab5`.
The reviewer reads its complete Round 5 and verifies publication on the integration branch.
It matches package `b8e4fa0d20d4c9b917be2667afecc212d25b5fd0` on head `e37b91d6`, base `9fe97f50`, and tree `ad98644e`.
The matching formal #307 source-review condition closes. Zero package source findings remain.
Sol's combined source verdict awaits the complete #312 pair. The fresh exact-head gate, final evidence receipts, and Q7 remain due.

Sol receipt `msg_plugin-w_1791826078_3437ea` supplies completed combined CLEAN `e76f59cb46546c9bd4bed8c306ee2e6d7047d0b1`.
The reviewer reads its complete train-4b Round 2 and verifies publication.
The combined verdict names the same train head, base, and tree. It reports all six complete member source receipts read and CLEAN.
The combined source-review condition now has its published verdict. No combined source finding remains.
The fresh exact-head gate, each final member body/evidence receipt, and Q7 remain due. No gate or merge is accepted.
