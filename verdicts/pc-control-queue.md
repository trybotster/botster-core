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
