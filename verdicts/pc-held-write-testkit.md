# PC package review: R-74 held writes, testkit part, PR #283

VERDICT: CLEAN (0 open), source review on `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`.

Round 1 below retains the two findings on the original head. Round 2 closes both by source review.
The exact-head gate on the fixed head remains pending.

- READY receipt: `msg_plugin-w_1791743029_b2452a`.
- Head: `646ebe7321cc5a5de46f15d9aa6b61028ffec1e5`.
- Parent, own-diff base, PR base, and gate header base: `9f7085f703457b0669785697dbeb14ac687a8942`.
- Tier: HIGH, under BUILD.md rule 3. The change touches the shared testkit crate.
- Contracts pin: `contracts-v0.1.36`, unchanged in all seven crates.
- Binding plan: 23bx, `~/botster-sessions/pins/stage1-plan.5224c5dc.md`.
- Plan SHA-256: `64977fbc8080379f6b684eb1c2dd0d5aa307b574f26a1193a911451c62c5038c`.

## Scope and contract basis

The reviewer reads the PC brief first and follows its reading order.
The reviewer reads the resume brief, pair-common, BUILD.md, applicable steward rulings, R-74, and the group-9 design note.
The previous #220 and P3/P6 harness reads remain applicable. This review changes no real-tier control.
The reviewer reads the complete three-file own diff, the current PR body, and the program and control paths.
The reviewer reads the pinned A5 candidate 4 text and the worker's PTY write and readiness paths.
The reviewer reads the plan changes from 23bu through 23bx and the lead's split approval at 04:56.

R-74 allows `kernel_bytes` only on Core's testkit program edge. FakeCore stays frozen.
The new testkit queue is accepted input that the program has not read. `pty_queued` reports its size without changing it.
Release delivers the queue in order. The real tier ignores the requested kernel capacity.
The lead approves PART A now, with PART B in PC's `.40` pin PR after #282 and the contracts tag.
PART B must add the real control and remove the six IDs only with its required evidence.
No pending ID, pin, transcript, timeout value, worker code, production hook, unsafe code, or mutation profile changes here.
This review grants no real-tier completion or group-completion credit.

## Findings

### HW-TK-1 — MEDIUM — the chunk cap can hide future queue work

Evidence: `crates/botster-core-testkit/src/program.rs:495-503`, especially the `!controls.blocked` guard at line 497.
R-74 permits writes while the program is blocked if `kernel_room` remains.
`is_writable` uses that rule, but the next-step marker still assumes every blocked program has no room.

Static case:

1. Set `pty_chunk` to 1 and `kernel_bytes` to 3 on a blocked program.
2. Write `abcd`. The edge accepts 1 byte into the queue.
3. The remaining write gets `WouldBlock` because `input_left` is 0. The kernel queue still has room for 2 bytes.
4. The guard leaves `step_refused` false because `blocked` is true.
5. `is_writable` is false until a new step restores the chunk cap.

`Workers::has_ready` at `worker.rs:481-487` includes `ProgramControl::waits_for_next_step`.
`TestkitCore::pump` at `worker.rs:1671-1677` uses that result to retain `more` and signal the wake.
The current marker can therefore report no future work while a later pump can accept more bytes into the queue.
This breaks the existing `pty_chunk` progress rule when the new form acts with it.
The worker's unchanged `on_pty_written` retries a partial write, then waits for readiness after the zero-progress result.
The defect is at the injected program edge, not in that worker logic.

Required change: mark next-step work when the chunk cap alone prevents progress and the blocked kernel queue still has room.
Retain no-next-step behavior when the kernel queue is full or the `pty_accept` limit is spent.
Add a behavior test for the combined controls. Show that later pumps fill the queue without a release or unrelated wake.
Keep the accepted queue unread until release. No test or reproduction runs in this reviewer worktree.

### HW-TK-2 — LOW — the held-write check discards the events it must inspect

Evidence: `crates/botster-core-testkit/src/pty_controls/tests.rs:534-541`.
The test calls `settle` before it checks that no `Completed` for the write arrived.
`settle` polls and discards events after every pump at lines 97-104.
The later assertion cannot inspect any matching completion that those polls already consumed.
The final post-release count and byte checks do not make this earlier absence assertion observe the held interval.
BUILD.md Testing rule 3 requires a test to prove its cited behavior. R-74's held-write fence requires the op to remain pending.

Required change: retain or check events from every pump in the held interval, through the final idle pump.
Assert that none completes this write before release. Keep the post-release completion and ordered-byte checks.
The change may use a focused helper or a local pump loop. It must not discard the events under the assertion.

Both findings were sent to PC in `msg_plugin-w_1791743210_dce502` before publication.

## Other source checks

The queue limit composes with `pty_accept`, `pty_fail_after`, and the per-step cap by taking the minimum available room.
The write decrements each applicable limit by its accepted count. It counts queued bytes as written without adding them to the input log.
`set_blocked(false)` drains the queue once into the input log and `delivered`.
`advance` moves `delivered` to the script's seen bytes before a later direct write, preserving byte order.
The script cannot match queued bytes before release. A plain block resets the additional room to zero.
These paths have no separate source finding outside HW-TK-1.

The control parser retains `deny_unknown_fields` and the session/handle checks through `program_of`.
It rejects `kernel_bytes` with `on: false` before it changes the edge. Negative and string capacities are parse errors.
`pty_queued` takes only the session and performs no pump, poll, wake, or state change.
The release keeps the existing host wake. The change adds no terminal parsing, mode tracking, or input encoding.
All terminal behavior remains in the real worker and libghostty.

The five added tests cover queue acceptance, unread bytes, release order, script matching, plain blocks, and bad control arguments.
The held-write test needs HW-TK-2's correction. The suite has no case for HW-TK-1's combined controls.
Exact queue sizes in these testkit tests come from the explicitly injected capacity. They are not real kernel-count assertions.

The Prior art note exists. The reviewer reads the pinned old `read_available` function at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its FIONREAD use takes a runtime pipe snapshot. It does not implement this program control, and the PR reuses none of it.
The new queue uses the standard Vec and existing injected edge to model only R-74's stated boundary.

## Supplied gate evidence

Log: `~/botster-sessions/gates/botster-core-stage1-pc-held-write-testkit-646ebe73-pool-20261011-111204-3662.log`.
SHA-256: `6ad44b5a5fa5efd7129a5ac9de1535bb96efe1bd8030eb7f7fbc313cf37b658f`.
The header names the exact head and base above. All ten steps PASS in 554.7 seconds.
The Linux job and wrapper exit 0 after 668 seconds. The job queues for 1 second and runs for 667 seconds.
Default: 1745 PASS, 478 skipped, 18.687 seconds. Slow: 558 PASS, 2032 skipped, 57.292 seconds.
All five new tests pass. The default budget passes. The gate changes no pending-list state.
Minimum counts: testkit 60/69; real-passing 49/68; real-accepted 50/69.

Stage 1 catches all 28 mutants in 151.2 seconds. Stage 2 does not run because stage 1 misses none.
The reviewer matches all 28 raw identities with all 28 named catcher records.
There are zero missed, timeout, unviable, or FLAKY outcomes. Fuzz is PASS with no changed decoder crate.
A green gate does not close either finding. The tests do not cover the missing combined-control schedule.

The log reports five passing IDs that remain real-pending. This PR removes none of them.
Three of these IDs also appeared in #277's gate. The lead assigns pending-list removals to P5's `.39` pin PR.
No removal of those IDs is approved by this verdict.

The body reports six candidate testkit transcript passes before the split.
The retained `evidence/pc-held-write/summary-after.txt` lists those six testkit passes on a scratch candidate based on `b6fd4b98`.
That evidence is not an exact-head `.40` gate or an A5-4 table for this PR.
PART B still requires the final contracts tag, both-tier comparison, Linux gate, and repeated real-tier evidence.

The reviewer runs no build, test, gate, mutation, reproduction, or executable base-check job.

## Next review conditions

PC must close both findings and send a new exact head with the applicable gate evidence.
HIGH requires Sol's same-head integration CLEAN after the package findings close.
After both CLEANs, PC must request the lead's final-gate slot under plan 23bu.
With the slot, PC imports the then-current v1 once, posts the base check, and runs one exact-head final gate.
The gate base must equal the v1 tip at merge. This verdict approves no later import or source delta.
The original program-controls group remains incomplete. Wait for the doorbell. Never poll.


## Round 2: source fixes

- Fix receipt: `msg_plugin-w_1791743341_c41c67`.
- Head: `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`.
- Parent: `646ebe7321cc5a5de46f15d9aa6b61028ffec1e5`.
- Own-diff and PR base: `9f7085f703457b0669785697dbeb14ac687a8942`.
- The head has no v1 import. The contracts pin remains `.36`.

The reviewer reads the brief first again. The binding rules and contracts read in Round 1 remain applicable.
The reviewer reads the complete two-file delta and the corrected helper and test call sites.
The source delta closes both findings. No new source finding remains open.

HW-TK-1 closes. The next-step guard now accepts `!blocked || kernel_room > 0`.
It retains the checks for an unspent accept limit, an active chunk cap, and a spent per-step allowance.
A chunk-only refusal can therefore retain future work while the blocked kernel queue has room.
A full kernel queue retains no next-step marker. A spent accept limit also retains no next-step marker.
The existing edge test adds these combined states. The new control test sets chunk 1, kernel capacity 3, and a five-byte write.
It pumps to idle, then observes three queued bytes and no completion over the retained events.
This case fails at the queue count with the old guard, which lets the host settle before it fills the queue.
Release then gives Written 5 and the complete ordered input log. The test covers the host's scheduling boundary.

HW-TK-2 closes. `settle` now retains the events from each poll, including the poll after the final idle pump.
The held-write test checks those retained events for a matching `Completed` before release.
The chunked-write test uses the same retained-event check. Existing call sites that ignore the return keep their prior drain behavior.
The helper keeps the same pump limit and clock. It adds no sleep, timeout value, production hook, or event re-injection.
The test interval has one write and no program output, so its event count does not exceed the existing poll bound.

PC reports 394 local testkit passes, 212 conformance passes, and two failures with the guard reverted.
The reviewer traces the stated reversal to the new queue count and next-step assertion. The reviewer executes no reversal or test.
No separate raw reversal log was supplied in this receipt. These execution counts remain PC's reports.

Source verdict: CLEAN (0 open) on `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`.
The original gate on `646ebe73` does not cover this later source delta.
PC reports that the new exact-head gate follows its running contracts job. No completed gate receipt for this head exists here.
The new mutation results, exact-head gate, Sol's same-head integration verdict, and updated body remain required.
After both source CLEANs, PC must request the lead's final-gate slot. The current-v1 import and Q7 conditions remain due.
This source CLEAN approves no later head or incomplete gate. No execution job ran here. Never poll.


## Sol Round 1 receipt

Receipt: `msg_plugin-w_1791743437_932a8d`.
Sol verdict: `951091b9b7bec3b686731eaafc98e247b02f87be`, `verdicts/stage1/pc-held-write-testkit.md`, Round 1.
The reviewer reads the complete verdict and verifies its publication on `origin/stage1/integration-review-sol`.
Sol independently confirms HW-TK-1 and HW-TK-2 on original head `646ebe7321cc5a5de46f15d9aa6b61028ffec1e5`.
This receipt adds no separate source finding. Sol leaves the correction unread in that Round 1 verdict.
Package Round 2 source CLEAN remains on `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`.
Sol's review of that fixed head and its exact-head gate remain pending here.
No execution job runs here. Wait for the doorbell. Never poll.


## Sol Round 2 source CLEAN receipt

Receipt: `msg_plugin-w_1791743918_7be331`.
Sol verdict: `d9bdb2db873eb6f11f6eae54c44f2848a5c98cf1`, `verdicts/stage1/pc-held-write-testkit.md`, Round 2.
The reviewer reads the complete verdict and verifies publication on `origin/stage1/integration-review-sol`.
Both source CLEANs cover `bd4c6f16c9c1435252f929663a33ea8b28b4beb5` and base `9f7085f703457b0669785697dbeb14ac687a8942`.
Sol independently closes both findings and accepts the corrected guard, event retention, and host-pump test.
The same-head integration source-review condition is closed. Both reviewers retain the reported-reversal evidence limits.
No completed gate receipt for the fixed head exists here. The body must include both source verdicts and completed evidence.
The lead's final slot, current-v1 import, applicable base check, final exact-head gate, and Q7 remain required.
This receipt approves no later import, running gate, `.40` pin, or real-tier result. No execution job runs here. Never poll.


## Fixed-head gate: RED on the advanced base's list comparison

PC receipt: `msg_plugin-w_1791743932_aa5b95`.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-held-write-testkit-bd4c6f16-pool-20261011-113530-95097.log`.
SHA-256: `4208ebc4b8ac166e5e7686c6f87daa02940f462b9f9b96f4acd27090116fbd49`.
Head: `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`.
Gate header base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
The PR and own-diff base remain `9f7085f703457b0669785697dbeb14ac687a8942`.

The reviewer reads the complete failed-step output and CI summary.
Fmt, clippy, and taint PASS. Lists FAIL on four entries that the later base removed from `core-pending.txt`:

- `conf::dp_3_unknown_type_closes_route`;
- `conf::ou_10_no_authorization_in_core`;
- `conf::ou_11_transport_ownership_is_defined`;
- `conf::ou_1_attach_hands_transport_to_worker`.

The reviewer independently compares the old PR base with the gate base. The base removes exactly those four entries from that file.
The reviewer verifies that this PR has an empty own diff under `conformance/`.
The failed comparison sees the PR's retained old-base entries as additions against the later gate base.
The R-74 source change does not alter this cause. Both source CLEANs remain on the fixed head.

Public-api, prebuild-worker, test-budget, slow, mutants, and fuzz are NOT RUN.
The gate fails in 75.6 seconds of CI steps. The Linux job and wrapper exit 1 after 160 seconds on msa1.
This RED is not a completed gate and supplies no exact-head mutation evidence. The original 28 catches cover only `646ebe73`.

PC asks the lead how to apply the current-v1 import and final-gate slot. The reviewer does not authorize an import or another same-head gate.
Under plan 23an(b), an approved base import that removes this diagnosed cause is a source repair.
The new head still needs its applicable source review or proven carry, base check, and one exact-head green gate.
The red log and its cause must remain in the body. The reviewer approves no unseen import, check, or gate.
No execution job runs here. Wait for the lead's answer and the doorbell. Never poll.


## Sol red-gate audit and lead decision

Receipt: `msg_plugin-w_1791744194_c01d54`.
Sol record: `3b907b883df1ae34b1406279290b07e1113de496`, `verdicts/stage1/pc-held-write-testkit.md`, Round 3.
The reviewer reads the new round and verifies its publication on `origin/stage1/integration-review-sol`.
Sol independently confirms the four removed entries, unchanged own pending file, and six NOT RUN steps.
Both source CLEANs remain on `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`. No new finding opens.

The reviewer reads the lead's 07:38 decision in the shared handoff.
PC sends FINAL-READY now. The lead gives #283 the final-gate slot after #282.
The approved final import and gate must supply the missing mutation result. No separate same-head retry is required.
The applicable import review or proven carry, base check, final green gate, and body remain due.

The reviewer reads the 23bx-to-23by plan delta.
Current pin: `~/botster-sessions/pins/stage1-plan.42684041.md`.
SHA-256: `86e0b9ef3e16b3b485a52508ec4da86affcecbc85deb88f22318160582c5fdcb`.
That delta does not change R-74's testkit queue or the final-gate slot rule.
No execution job runs here. Wait for the final-slot evidence and the doorbell. Never poll.

## B13: replacement local evidence

PC receipts: `msg_plugin-w_1791746051_50c162` and `msg_plugin-w_1791746172_38a937`.
The reviewer reads the shared-target rule in `pair-common.md` and the live PR body.
The PR remains OPEN on `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`.
Its base remains `9f7085f703457b0669785697dbeb14ac687a8942`.

B13 makes local evidence invalid if another worktree built into the shared target directory after its last clean.
PC replaces the earlier local unit, conformance, and guard-revert results.
The reviewer reads the replacement logs, summaries, failed assertions, revert patch, head record, and README.
Directory: `~/botster-sessions/shared/core-stage1/evidence/pc-held-write-testkit/`.

| File | Result | SHA-256 |
| --- | --- | --- |
| `clean-lib-bd4c6f16.log` | 394 passed, 0 failed | `5288f1fec2233ebcbfea9cb5a1140c519778909343afc99405d81dd3468a8878` |
| `clean-conformance-bd4c6f16.log` | 212 passed, 0 failed; minimum 60/69 | `054e17a32cec4751ce85335582325aee681dcdbcade0f2c504e96a5e26994017` |
| `revert-hw-tk-1.log` | 392 passed, 2 failed | `3b0eee6937b9b5a05f700fa49b17fb992f4cf52dd24c69c0da6965f12999fe50` |
| `restored-bd4c6f16.log` | 394 passed, 0 failed | `a35c0b9258708863d90655da279816beabcaa2e2ed6b8127493b3e6b10f243ef` |
| `revert-hw-tk-1.patch` | Only the corrected HW-TK-1 guard is reverted | `f9a0e6bdfe38f1d957068fd2ac9497be1cf6fc52c800c3fc761b4ca6d21b5ae2` |

The revert fails `only_a_spent_chunk_waits_for_the_next_step` and `a_chunked_write_fills_the_kernel_queue_before_the_host_settles`.
The pump test observes one queued byte instead of three. This agrees with the source finding.
The revert and restoration logs each show compilation of this PR's testkit crate.
PC reports a separate clean that removed 16.3 GiB before these runs.
PC reports that no other worktree built into the directory after that clean.
PC also reports that all dep-info files name this worktree. The README records these reports.
No clean log exists. The reviewer does not independently verify the clean, writer history, or dep-info inventory.
The reviewer runs no executable check. These are reviewed PC execution logs, not independent reviewer execution.

The replacement evidence closes no new source finding. Both source CLEANs remain on the fixed head.
The original pool gate is unaffected by B13. It still covers only `646ebe73`.
The fixed-head gate stays RED. Its six later steps, including mutants, remain NOT RUN.
The final-slot import, applicable review/check, final green gate, and body remain due.

The reviewer asks PC about the pre-split six-transcript testkit result from contracts #52.
Question: `msg_plugin-w_1791746248_58e78a`. PC answer: `msg_plugin-w_1791746265_3ec208`.
PC confirms that the earlier run did not satisfy B13 and marks it invalid in the body.
The reviewer verifies that correction in the live body. The head remains unchanged.
The six-transcript result is no longer claimed here. PART B must supply valid evidence for both tiers.
This evidence question is closed. No new source finding opens.
No final merge approval is given. Wait for the doorbell. Never poll.

## Final-slot import: source CLEAN carries

VERDICT: CLEAN. No open source finding.

PC receipt: `msg_plugin-w_1791750654_adad67`.
Reviewed head: `bd4c6f16c9c1435252f929663a33ea8b28b4beb5`.
Imported base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
New head: `4daf627b54739f847dd2448bf660a6a45c1b714a`.
Parents: the reviewed head, then the imported base.
Tree: `fcfb3382e8c6c8633d152eb3bd7f2d6a151440d4`.
The reviewer verifies the published head and base in the live PR.
PC reports that the lead gives #283 the slot before #282, which waits on a permission prompt.
The live body records that decision. The package has no dependency on #282 for this testkit-only change.

The reviewer reads `base-merge-check-4daf627b.out` in the shared evidence directory.
SHA-256: `c1759610f3bd56aa59dbb847a6786e9bba88318612ee534046b33237da924222`.
The output reports PASS for all four checks, including a merge without conflict.
The reviewer independently compares the Git objects and canonical patches through read-only commands.

- The canonical own patch is byte-identical before and after import: 19815 bytes.
- Its SHA-256 is `5caf01f561face85debe54c9da930443c977eace09e66d77474d839274f1ed00`.
- The 11 paths changed on the base do not intersect the three own paths.
- All three reviewed source blobs are unchanged.
- All 11 changed base blobs are preserved exactly in the new head.
- Both pending files were unchanged by the original own diff and now equal the imported base's files.

The package source CLEAN carries to the new head. No source delta round is required by this PASS.
The reviewer does not run the base-check executable, a build, a test, or a gate.

The reviewer reads the 23by-to-23cm plan delta from the pin named in PC's handoff.
Pin: `~/botster-sessions/pins/stage1-plan.c77c2434.md`.
SHA-256: `c77c243490be26d6e4ee9f5b9491fd8b62d6dc9827dceebccfd864e2d60abf02`.
The delta retains R-74's queue behavior and records B13, later contract work, and PC's later diagnosis work.
The `.40` pin and real-tier changes remain separate from this PR.

PC reports that the one final gate is running on the new head. No completed final-gate log is read here.
The gate must supply the fixed guard's mutation result. The body must record the completed evidence and applicable review receipts.
Sol's applicable import receipt, the final green gate, and Q7 remain due here.
No final merge approval is given. Wait for the doorbell. Never poll.

## Final gate and merge receipt

VERDICT: CLEAN. No open source finding.

PC receipt: `msg_plugin-w_1791751041_4ae590`.
Final head: `4daf627b54739f847dd2448bf660a6a45c1b714a`.
Gate base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-held-write-testkit-4daf627b-pool-20261011-133054-46680.log`.
SHA-256: `4031f5cf739a27eb403a0c8407919855e86b2ca6c4ac5bd7e954bf3770de37cb`.

The reviewer reads the exact-head header, all ten summaries, both exits, test summaries, raw mutant identities, and named catchers.
All ten steps PASS. The CI steps total 343.6 seconds.
The Linux job exits 0 after 353 seconds on msa1. The wrapper exits 0 after 354 seconds.
Default tests: 1754 passed, 474 skipped, 11.127 seconds.
Slow tests: 559 passed, 2037 skipped, 40.256 seconds.
Testkit conformance: 216 passed, zero failures, minimum 61/69.
Real minimum: 49/68 passing, 50/69 accepted. Six pending-real trials pass; this PR removes no pending entry.

All 33 raw caught identities match the 33 named catchers. No mutant is missed, timed out, or unviable.
Stage 1 catches all mutants in 115.9 seconds. Stage 2 does not run because Stage 1 misses none.
All five mutants of the corrected `program.rs:498` guard are caught by `only_a_spent_chunk_waits_for_the_next_step`.
They replace `||` with `&&`, delete `!`, or replace `>` with `==`, `<`, or `>=`.
The host-pump test `a_chunked_write_fills_the_kernel_queue_before_the_host_settles` passes at line 1202.
It also catches the `pty_queued` replacement mutant. The log contains no FLAKY line.
Fuzz PASS reports that the diff changes no crate with a decoder harness. It does not claim a changed decoder trial.
The final gate supplies the mutation evidence that the fixed-head red gate could not supply.
The earlier red gate remains RED in this record.

The reviewer reads the final live body. It names the gate, base, mutation results, package import carry, and Sol's import carry by message.
The body retains the red gate and the invalid pre-split evidence qualification.
Sol's import message is reported by the body but has not reached this reviewer.
The reviewer asks Sol for that receipt in `msg_plugin-w_1791751076_1d3a98`.
The source and final-gate conclusions above do not claim that this reviewer read Sol's missing message.

The reviewer verifies that GitHub reports PR #283 MERGED at `2026-10-11T20:37:42Z`.
Merge commit: `670e5fc799a25794910a1847fc41b3e089bf03f4`.
Its parents are the exact gate base, then the exact gated head.
Its tree is `fcfb3382e8c6c8633d152eb3bd7f2d6a151440d4`, the gated head's tree.
The reviewer verifies an empty diff between the gated head and merge commit.
The lead merged the exact gated tree from its recorded base. Q7's base and tree condition is verified.

The package's source review and final-gate audit are complete. The Sol message receipt remains due for the cross-review record.
The `.40` pin, six held-write transcripts on both tiers, and later PC work remain outside this merged PR.
The reviewer runs no build, test, gate, mutant, or executable base check. Never poll.

