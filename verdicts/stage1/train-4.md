# Train 4 combined integration review

## Round 1 — combined source before the gate

Exact head: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`.
Tree: `4bb9e8c7f7d6e8b610e4b80ce782499ae3389cde`.
Base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Lead assignment: `msg_plugin-w_1791821464_d6faca`.

### Delivery contract

The required outcome is a reviewed composition of deterministic scheduler traces, the .44 contract pin, and bounded control-queue cancellation.
The scope contains these ordered frozen members:

1. #314: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
2. #312: `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
3. #307: `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.

Sol owns #307's integration delta and the combined review.
Sol-B owns #314's integration delta. Sol-C owns #312's integration delta. Each member retains its package reviewer.
The lead owns train construction, the gate, and Q7 merge/tree/push verification.
The review reuses the binding Stage 1 acceptance plan. It adds no separate acceptance plan or delegation.

The invariants are the accepted member behaviors, public Scheduler observations, separate hold and room state, and exact three-way conformance sets.
The real entry points are the host driver and the real testkit adapter under the pinned contracts.
Decisive evidence must come from the completed gate on this exact train head and each member's final receipts.
The remaining acceptance condition is that missing final evidence. The lead must run the authorized gate after the source reviews.
The reviewers must read the retained gate and final member records before final acceptance.

### Order and complete source audit

I verify all three merge objects and their ordered first-parent chain.
Merge `b3f7f39b72bc2a985634240573b2d08660b162c6` joins the exact base and frozen #314.
Merge `13fc69a0a196f75a6aaa1662c1deac1817e5dd88` adds frozen #312.
The exact final head adds frozen #307.
The member merge bases with their prior train parents are `080987829a2e2245a7ea97606020400392c974d6`.

The combined delta has 26 paths, 1019 added lines, and 97 deleted lines.
Full-index diff SHA256: `aa5a215608ed07d9f894c6265a9eb56d1d7d3c7f908dc9aa25926a7d340cde6b`.
Git diff --check passes. No execution check runs.
I read all new #314 and #312 source changes. I reuse the complete accepted #307 review after independent blob and patch checks.
At each merge, every own added and deleted line matches its frozen member patch. The merges change no extra path.
The accepted #307 source, test, and design blobs remain identical. Its pending blob composes with #312's removal.
The train base differs from the members' base only in four xtask files from accepted #310.

### Shared interfaces

#314 records the public Scheduler choice points, candidate counts or bounds, and answers.
Each run creates a fresh recorder, policy, and Rig. Two complete traces must match and be non-empty.
At least one trace must differ from seed zero. The test no longer claims equality with the historical pre-E1 golden trace.
The lead's ruling permits this same-head determinism test. The earlier E1 migration and mutation records retain their historical scopes.

#307 adds control-room service before the scheduler-hold and read paths. It retains the more_input budget term.
Scheduler holds remain separate from control-full marks and start holds.
The seeded trace workload sets no room override and makes no cancel request. Its new room path therefore does no work.
The new tests add no shared recorder or global random state.
Cancel precedence, failed-write mark retirement, failed-side refusal exclusion, frame-bound sizing, and room wakes remain accepted and unchanged.

#312 pins all seven manifest dependencies and all nine lock sources to contracts-v0.1.44 at `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea`.
The Core contract and route-codec source have no .43-to-.44 delta.
The real adapter adds only the Printed match arm. The harness does not enable PRINTS_ENV.
The probe emits print reports only when that environment variable equals 1.
This train does not add terminal-history service or change the control-queue wake through print reports.
The route-log test selects route_expect_pty by key. Its pinned transcript contains exactly one such step, at index 13.

The accepted #310 resolver reads botster-core-contract's git source from Cargo.lock.
It matches the repository prefix and a hexadecimal checkout revision of at least seven characters against the locked revision.
It rejects zero or multiple matches. The .44 lock therefore selects .44 rather than the earlier checkout.
The default resolver starts no Cargo process. Its slow comparison with Cargo metadata remains required in the final gate.

### Transcript and conformance composition

I read the complete two own IN-6 transcripts at .43 and .44. Both remain byte-identical.
Their SHA256 values are `4254018ed800f7213dee79ce27da39993d238f24ac4157cb6100613ba1181161` and `2c83010626e9bc0e7f6504dbfa0bd0e15e263b0433e8afd2e05d571c1f8947b3`.
The complete .44 IN-8 mode-at-write-start transcript adds the R-74 held-write fence before the mode-sensitive write.
It uses one kernel byte, a 40960-byte fill, and the already served pty_queued wait.
It retains the mode change, unblock, Written outcome, and PTY-byte assertion. It adds no control_queue override.
#312 precedes #307, as the lead requires after the member's RED baseline.
This source evidence confirms the structural fix exists. It does not prove a successful run.

I independently audit all nine conformance text-set formulas after each merge. All 27 checks pass.
Pending counts are 406, 406, 405, and 403 across the base and three merge results.
Real-pending counts are 43, 43, 34, and 34.
#312 removes a5_2_deferred_completion_keeps_slot_until_polled and its obsolete comment, plus its nine accepted real-pending ids.
#307 removes only in_6_cancel_result_kinds and in_6_refused_full_wakes_on_room.
No removed base id returns. Held remains four, ledger 691, real-only 26, real-proof entries seven, and minimum 69.
The train preserves the AM-2 fairness and EV-9 pending debt.
The mutation, exclusion, known-flaky, CI, profile, and process-allowance files retain their base versions.
#309 and #311 are absent. This review accepts no six-times frame bound or later status removal.

### Complete member receipts

I read the complete applicable source sections in these records:

- #314 integration: `5c264269b979df1960afd06ceaeac43cc72ef7f1`, Round 2 in p6-seeded-trace-determinism.md.
- #314 package: `f852df5d103aefa6f97ff1314083506a1845dc76`, Round 2 in p6-seeded-trace-determinism.md.
- #312 integration: `cc05c841f4b02e096d4e72c1043b6b46aa8be9c3`, Round 6 in rc-pin-44.md.
- #312 package: `a401c753c91855cc484db771539e11b395db1ca9`, train-4 section in rc-real-controls.md.
- #307 package: `76ae3ad37fc67b47cf2fbaea794cb77554b526d1`, Round 5 in pc-control-queue.md.
- #307 integration: Round 4 in this commit's verdicts/stage1/pc-control-queue.md.

All six source reviews cover this exact train head, base, and tree. They have zero open findings.
I also read the complete #312 repaired-member package section at `101a3ac854f4cd0c5007e7cfda684b9cf10d0cce`.
The .44 deferred-completion removal retains its strict and focused real evidence scopes and its clean-output provenance limits.
The original #314 and #307 mutation records keep their original heads and scopes. No historical record becomes a train execution result.

### Verdict scope and next evidence

No combined source, interface, or list finding remains.
The lead reports compile PASS and strict seeds 0-31 at 264/0. No separate retained pre-check log is supplied.
I treat those results as operator reports. I accept no completed train gate from them.
The exact #307 member gate remains RED. Its unmutated real Stage 2 baseline failed; thirteen Stage 2 mutants and fuzz did not run.
The empty known-flaky file grants no ordinary retry exception.
The lead's explicit ruling requires .44 before #307 and selects the exact composed train gate as the completion path.
The train contains that ordered fix. The gate must still supply the baseline, mutation recovery proof, and fuzz.

The completed exact-head gate, all final member evidence receipts, and lead Q7 remain due.
The lead must verify the final equal-tree merge and remote heads. This source verdict accepts no merge or later head.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.
Wait for the doorbell. Never poll.

VERDICT: CLEAN (0 open; combined source on exact train 4 head, final train evidence pending)

## Completed RED gate and requested EV-2 member-path check

Lead request: `msg_plugin-w_1791822648_5bb135`.
The head, base, and tree remain the exact Round 1 identities.
The request asks whether a member changes this id's real-tier control, title, queue, or pump path.
It permits no reviewer execution job.

Retained gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-4-03709a0c-pool-20261012-092150-15636.log`.
SHA256: `3329b853b483eeb3f0f1a1bcd96180b53ce43b3daf2b0ee17c4bbf69c1a75fd8`.
I read the header, EV-2 failure, test summaries, CI summary, and final exits.
Seven CI steps pass. Slow fails; mutants and fuzz do not run. CI total is 334.3 seconds.
Default has 1957 passes, 427 skipped, and 10.648 seconds.
Slow has 632 passes, one failure, 2058 skipped, and 46.732 seconds.
The failed trial is conf::ev_2_mandatory_and_keyed_never_dropped, seed 0, step 71, step_timeout.
Nextest reports 10.124 seconds for that trial; its inner test reports 10.08 seconds.
Jobq exits 1 after 418 seconds: queue 2, run 416. The wrapper exits 1 after 419 seconds.
The train gate is RED. It supplies no final mutation or fuzz acceptance.

I read the complete EV-2 transcript at .44 and compare it with .43. Its bytes are unchanged.
Transcript SHA256: `c400c353ed1fca21090c8a3f1c5b782c4088cc3bcb8f3cd43147e2439352a43a`.
The program only holds. The transcript makes no cancel request and uses no control_queue control.
Step 67 sends the TB title. Step 68 awaits quiet. Step 69 reads oracle_state and binds its title.
Step 70 begins Stop. Step 71 collects EventsLost, the bound TitleChanged, Exited, and Completed.
The .44 exception file retains the step-67 no-await_title exception under R-57.
The tested train's known-flaky file has no entries. This check changes no exception or list.

#314 changes a host unit test only. It changes no real-tier executable source for this id.
#307 adds control_room to the host pump, but waits_for_room returns false until a cancel refuses Full.
This transcript cannot set that mark. The added call returns before an edge-room read or budget accounting.
The real control registration does not invoke control_queue here. Its new edge state remains empty.
The accepted title observation and event-queue paths remain unchanged; queue.rs and flow.rs have identical base and train blobs.
The oracle.rs and real/oracle.rs blobs also remain identical.

#312 changes the active probe implementation through its contracts-v0.1.44 pin.
I read the complete .43-to-.44 probe control.rs and main.rs diff.
Before .44, an Output request directly sends bytes to the writer channel, then sends Ack on the socket.
At .44, the same request calls Wire::output, locks WireState, calls Order::output, and applies Queue then Ack effects.
PRINTS_ENV is disabled and this program has no print step. Thus awaiting stays None and Output is not deferred for Recorded.
That preserves the apparent queue-then-ack semantics, but it does not bypass the new Wire path or lock.
Core's write_output and acknowledged output-history update remain unchanged.
The new Printed adapter arm is inactive here. The quiet and oracle reads remain unchanged.

Requested path-check result: MEMBER-CAUSE #312 — the .44 pin changes the active pty_output producer through Wire::output.
This result identifies a changed candidate path under the lead's requested check. It does not diagnose the timeout's runtime cause.
I cannot issue NO-MEMBER-CAUSE as an unchanged-path certificate for this train.
The earlier source CLEAN remains valid: this inspection identifies no demonstrated product defect.

I read the retained P6 B1 comparison around its EV-2 failure.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-p6-b1-evidence-2ea24902-pool-20261011-223903-57841.log`.
SHA256: `cfb090b714b408bf37ee98570e35d200c21b8e9b4022c22aa69ba0421cee5922`.
It contains the same id at seed 25, step 71, step_timeout in its kept comparison, followed by a PASS on the other side.
Its head is `2ea249028b61c8d2cf194b7139271dd5457d7caa`, six commits after `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
None of the three frozen train members is an ancestor of that head.
This proves an earlier retained failure report exists. It does not prove that the changed .44 producer is unrelated to the current failure.

The lead must resolve this changed-path condition before using this check as retry authority.
No retry, new gate, known-flaky entry, matcher change, or final acceptance is authorized by this record.
Final evidence receipts and Q7 remain due. I ran no execution job.

VERDICT: BLOCKED (requested NO-MEMBER-CAUSE certificate; active #312 producer path changed; exact train gate remains RED)

## P6 diagnosis and one-re-gate request — REFUSE on supplied evidence

Lead diagnosis/request: `msg_plugin-w_1791823366_364dc2`.
Exact train head, base, and tree remain unchanged.
I read the lead's complete diagnosis and its matching live handoff entry.
I independently read Tap::quiet, the real oracle read, the acknowledged history update, the Stop admission, and the complete R-57 ruling.
The source supports the proposed race: an Output ack records the intended bytes before the worker must read and apply them.
Tap::quiet checks the host's inbound edges. It does not prove that the worker applied the final TB title.
The oracle replays the acknowledged bytes and therefore can return TB before Core receives its TitleChanged.
The subsequent Stop can race the worker's read and title observation. The transcript has no await_title fence before Stop.
The train changes no worker or terminal implementation source. Its new control-room branch is inactive for this transcript.

I read the complete governing retry rule in pinned plan revision 23cm, section 8, line 511: revisions 23al and 23bl.
The rule explicitly says that the condition concerns the cause, not the changed path.
Thus my preceding MEMBER-CAUSE label answers the lead's original path question only.
It is not proof that #312 causes this failure, and it does not itself defeat a cause-based exception.
For this request, I supersede any inference that a changed Wire path alone blocks revision 23al.
The empty known-flaky file also does not by itself exclude that separate one-re-gate exception.

The exception still requires trace evidence of the diagnosis and the same mechanism on plain v1.
Revision 23bl permits a timing-only controlled reproduction on a v1 tip instead, with the exact id, step, and symptom.
The lead must accept that reproduction in writing. The diagnostic must change timing only, and no tested behavior.
The lead must also record written one-re-gate approval, the cause, its owner, and the fixing PR.

The supplied prior report is the B1 comparison on `2ea249028b61c8d2cf194b7139271dd5457d7caa`, six commits after ae49bbec.
It is not a run on the plain v1 tip. Absence of these three frozen members does not make it that tip.
The retained log also changes to b1-vendor path overrides for the after side.
The comparison reports the same id and step failing before and passing after; it supplies no retained event trace of the missing item.
The current gate similarly records step_timeout, but does not identify which of the four collected events was missing.
No accepted timing-only controlled reproduction on v1 is supplied in this request.
These records support the existence of an earlier race. They do not complete the rule's same-mechanism evidence condition.

Requested reviewer line: REFUSE — the supplied record lacks the required plain-v1 failure or accepted controlled reproduction and the mechanism trace.
This refusal concerns evidence for retry authority. It does not reject the source-supported diagnosis or identify a new member defect.
I cannot certify the full cause attribution for a countable unchanged re-gate from the current records.
The lead can supply retained qualifying evidence for another static review. I request no reviewer execution job.
If the lead chooses a separate explicit exception, the lead must identify and record that exception; this review does not infer one.

The steward questions about draining on Stop or restoring the state fence remain unresolved in the supplied diagnosis.
The source CLEAN remains valid. The exact train gate remains RED; mutants and fuzz remain unexecuted.
No re-gate, train change, list edit, matcher edit, or final acceptance follows from this record.
I ran no build, test, gate, mutation, reproduction, measurement, or executable base check.

VERDICT: BLOCKED (REFUSE the requested one unchanged re-gate on supplied evidence; revision 23al/23bl evidence conditions remain unmet)

## Re-gate withdrawn; R-107 and the new completion path

Lead ruling: `msg_plugin-w_1791823545_354ad3`.
The lead accepts the evidence refusal and withdraws the unchanged re-gate request.
The lead states that `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8` will not be re-gated.
Sol-C independently reports the same withdrawal in `msg_plugin-w_1791823557_e47f37`.
The retained train gate remains RED. The withdrawn request grants no acceptance.

I read complete R-107 at contracts `ed7f654f44bd3bef0f25b8d0521c63cbb7f50c4c`.
R-107 says that bytes written to the PTY before the payload ends belong to the OU-7 exit tail.
The worker must apply that tail to the model and queue its metadata before Exited.
It classifies the missing tail drain as a Core defect. A31, Core #163 part B/B2, owner P3, is the structural fix.
It retains the R-57 exception where Stop or payload exit supplies the causal ordering.
An id that reads the title without an intervening exit still needs an await_title fence.

R-107 permits temporary known-flaky entries for the three named exit shapes until A31 lands.
Its permitted symptom is step_timeout at the step waiting for the tail's metadata, step 71 for the current EV-2 id.
Other failure signatures are not covered. A31's PR removes the entries.
A31 must prove title before Exited in at least twenty runs on the testkit and real Linux/macOS tiers.
Its evidence uses the shortest accepted stop_grace, covers a tail longer than pending_output, and includes the three ids at their failing seeds.

The lead assigns Sol and Sol-C the forthcoming HIGH known-flaky PR from v1.
After that PR lands, the lead plans train 4b on the new v1 with the same frozen members and a fresh gate.
The lead will request the base delta review. No new PR head, base, train, or gate is accepted here.
The new known-flaky source must preserve diagnosis, exact signature, owner, removal PR, and the absence of unrelated gate changes.
This review continues the existing delivery contract under the new ruling. It does not create another acceptance plan.

### QUESTION R107-SI-1 — acknowledged output versus written PTY tail

R-107's case says that the probe acknowledges after the bytes reach the PTY slave.
The pinned .44 source and the source at ed7f654f do not establish that boundary.
Order::output returns Queue(bytes), then Report(Ack).
Wire::apply sends an Output chunk as (bytes, None) to the writer channel, then sends Ack on the socket.
The separate writer thread later calls write_all and flush. Output supplies no completion sender and waits for no write completion.
WireState's lock does not wait for that writer. Only the program print path supplies and waits for a written sender.
Thus an Output ack proves enqueueing, not completed writing to the PTY.
The real oracle's history records acknowledged bytes, so it cannot distinguish a queued-but-unwritten final title from a missing worker drain.

The OU-7 ruling for bytes actually written before exit remains binding.
The current timeout log does not establish that the final title had reached the PTY before Stop.
The lead must resolve this acknowledgement discrepancy with the steward or supply evidence of the actual write boundary.
The resolution must establish which cause the new known-flaky diagnosis and A31 removal proof cover.
This question identifies a source fact; it does not diagnose a second observed failure or change a transcript.
The old source CLEAN stays valid. Future known-flaky and train-4b reviews remain unstarted pending exact-head READY.
I ran no execution job.

STATUS: QUESTION R107-SI-1; old train re-gate withdrawn; new known-flaky PR and train-4b review pending

## R107-SI-1 resolved by the corrected ruling

Lead receipt: `msg_plugin-w_1791823877_c463c0`.
I read the complete corrected R-107 and its follow-up at contracts `fe209c6563fa8943065ef1aaa3e09a7e3154a201`.
The follow-up supersedes the original acknowledgement premise and the original Q2 scope where they differ.
It explicitly says that the retained logs cannot distinguish an unwritten probe chunk from an omitted worker tail drain.
It preserves pty_output's acknowledgement meaning: the bytes are queued in writer order.
It rejects a default acknowledgement that waits for the write, because output backpressure can require a later transcript step to release the writer.
It also rejects a held-output exception that gives one acknowledgement two meanings.
These corrections agree with the independently read source. R107-SI-1 closes.

The new read-only control is pty_output_written {session} -> {bytes}.
It reports the total bytes of that session's pty_output requests whose write_all and flush have returned.
The transcript waits through await_control. The probe adds no wait and holds no reference for the control.
The testkit program edge supplies its immediate written total. The real tier must forward the control to the probe.
The contracts patch includes the control row, probe count, acknowledged-request documentation, and transcript fences.
A Core testkit PR is required if unknown probe controls are not already forwarded.
The lead assigns the contracts patch to P6. No implementation or future pin is accepted by this ruling read.

The EV-2 transcript awaits the cumulative pty_output_written total after the final TB output and before Stop.
Then the actual PTY tail falls under OU-7, and A31's drain requirement remains binding.
The EV-2 R-57 exception remains because the written-output fence and exit order supply its ordering.
The patch must also count every output/exit/check shape in the sweep and add its fence or a debt line that cites the follow-up.
The same PR adds the shape lint and its positive and negative self-tests.

The other two ids have no intervening exit. Their earlier exceptions are revoked.
EV-1 step 10 gets await_title for its existing T3 title, with no new sentinel and no event poll.
EV-2-retired step 121 gets a title sentinel in the same output and await_title on s2.
Only the sentinel's TitleChanged enters later ignore lists. The author must first prove that s2's live instance owns the title.
If the sentinel changes the loss-marker expectations, the author must return to the steward.
Both fences retain R-57's no-launch-or-adoption scope condition.

The current EV-2 known-flaky entry must name both candidate causes and both owners: P6's written-output control/fence and P3's A31.
Its exact covered signature remains the tail-metadata step_timeout, step 71 on the current transcript.
Removal requires both fixes to land and the three ids to pass at their failing seeds.
The entry cannot claim an A31-only diagnosis or an A31-only removal condition.
The other ids get separate entries only if they fail before the patch, with their own read-step signatures and this patch as the fix.
R-107's separate A31 evidence requirements remain binding.

The lead keeps the new HIGH known-flaky PR assigned to Sol and Sol-C, followed by train 4b after that PR lands.
No exact-head READY or train-4b delta request has arrived in this receipt.
The old train gate stays RED and will not be re-gated. Its mutation and fuzz evidence remain absent.
This record closes the ruling question only. It accepts no new list, control, transcript, pin, PR, train, gate, or merge.
I ran no execution job.

STATUS: CLEAN (R107-SI-1 resolved; future known-flaky PR and train-4b review pending)

## R107-SI-2 supersedes the EV-2 completion path

I read complete R107-SI-2 at contracts `a10b85c363b069f8e3f98cdb6597f4451454c98b` and `28275d852946ee187a16d8543b2c260facba14b8` during #317 review.
Contracts #89 already fences EV-2 with await_title TB before Stop. That fence first appears in contracts-v0.1.45.
The ruling withdraws the new written-output fence for EV-2 and its A31-dependent removal condition.
The known-flaky entry now covers the pre-.45 unfenced form and leaves in the pin PR that takes .45 or later.
RC owns that pin, draft Core #316. It must remove the entry and restore the empty-list test together.
A31 gets a separate OU-7 amendment/id. The new written-output control and the two other title fences remain P6 work.
The older R107-SI-1 requirements remain history where this follow-up supersedes them.

P3 sends HIGH #317 READY on `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec` in `msg_plugin-w_1791824755_ebcc9e`.
Sol's complete package verdict is in `verdicts/stage1/p3-known-flaky-ev2.md` in this commit.
It is CLEAN for the exact two-file change, corrected body, and completed 10-PASS pool gate.
Sol-C's matching integration verdict and the lead's merge verification remain required.
The planned train-4b delta and exact composed gate remain due after that PR lands.
No #317 evidence becomes a result on train 03709a0c. Its RED gate and no-re-gate ruling remain intact.
I ran no execution job.

## Train 4b source inspection — matching member receipts pending

Lead assignment and #317 merge receipt: `msg_plugin-w_1791825491_e99ad8`.
Exact train-4b head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
Base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Sol owns the #307 integration delta and combined review. The other reviewer assignments stay unchanged.

I independently verify the three ordered merge objects:

1. `471c0dece49519bcd4f31932eb8bef449d723233`: base 9fe97f50 plus frozen #314 a4eac3c1.
2. `89808877dc1e5664cd4ec59877c4980bc064dbf4`: first merge plus frozen #312 c0f8a09c.
3. `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`: second merge plus frozen #307 2ed85f93.

The independent remote read names that exact train-4b head and exact v1 base.
#317's completed merge receipt is recorded in verdicts/stage1/p3-known-flaky-ev2.md in this commit.
No #316, #309, #311, #315, or A31 implementation is a member of this train.

The complete full-index delta from old train 03709a0c to this train equals the accepted #317 base move exactly.
SHA256: `7d1572807eb38d9abd05ebd26e4c76db5cbc27bf48e2674089308f1b85b8dde6`.
Only ci/known-flaky.toml and xtask/src/known_flaky.rs change between those train heads.
Both blobs equal the accepted new base. I read that complete two-file delta again.
All 26 original train source/test/design/list blobs remain byte-identical to 03709a0c.
The combined diff against the new base equals the previous train's combined diff against its old base exactly.
Its full-index SHA256 remains `aa5a215608ed07d9f894c6265a9eb56d1d7d3c7f908dc9aa25926a7d340cde6b`.
Git diff --check passes. This static check runs no executable base check.

At each merge, the added and deleted own lines match the accepted frozen member.
No extra path changes at any merge. The first two full-index patches equal their original member patches.
The #307 merge full-index patch equals its previous train merge patch, SHA256 `174f783bbb6afd333174de13812476977251389c6ae9ef031765f5e6879926ce`.
Its patch differs from the standalone member's full-index form because the prior #312 merge changes the pending blob.
All twenty own added/deleted line sets still match. All nineteen own non-list blobs equal frozen 2ed85f93.
This difference supplies no new source resolution or restored pending id.

I independently check all nine conformance text-set formulas after each of the three merges. All 27 checks pass.
Pending remains 406 -> 406 -> 405 -> 403. Real-pending remains 43 -> 43 -> 34 -> 34.
The held, ledger, real-only, real-proof, deferred, withdrawn, and minimum sets retain the reviewed train-4 contents.
The .44 pin still precedes #307. All prior runtime interface and transcript source checks remain applicable by exact blob equality.
The seeded trace test retains its fresh state, public Scheduler assertions, and inactive control-room path.
The real adapter retains disabled print reports. The checkout resolver retains the .44 lock selection.

The new known-flaky entry matches only the real EV-2 id, step 71, step_timeout; its seed may vary.
It does not match the host's seeded-trace test, the two IN-6 ids, or another failure signature.
The exact-one-entry assertion remains compatible with the .44 pin and changes no production parser or filter.
Unmutated slow and Stage 2 baseline runs still execute EV-2. Mutation stages omit only its named real-tier trial.
The future gate must show that no skipped EV-2 trial supplies a mutation catch citation.
R107-SI-2's .45-or-later structural fix remains assigned to RC through #316. Its removal and empty-list restoration remain due.

The lead reports cargo check -p xtask --tests --locked PASS. No retained pre-check log is supplied here.
I treat that as an operator report, not the completed composed gate.
The fresh exact-head train gate and all final member evidence receipts remain due.
Old train 03709a0c stays RED and will not be re-gated. The #317 gate is not a substitute for train-4b evidence.

No new source or combined-interface finding appears in this inspection.
The #307 package delta and the #314/#312 matching member receipts remain pending before formal source verdicts.
Requests: `msg_plugin-w_1791825578_61796c`, `msg_plugin-w_1791825578_642cdd`, and `msg_plugin-w_1791825578_6e7ae6`.
Sol-B and Sol-C independently confirm source equality in `msg_plugin-w_1791825570_1ec5cb` and `msg_plugin-w_1791825589_61ca02`.
Their complete same-head package/integration receipts remain required.
This inspection accepts no gate, final mutation result, retry, later source, status removal, or merge.
I ran no execution job.

### Train-4b member receipts read so far

I read complete #307 package Round 6 at `b8e4fa0d20d4c9b917be2667afecc212d25b5fd0` after receipt `msg_plugin-w_1791825792_cd148b`.
It agrees with the complete base delta, source preservation, pin order, filter/baseline scope, and unchanged conformance sets.
Sol's matching #307 integration Round 5 is CLEAN in verdicts/stage1/pc-control-queue.md in this commit.
The #307 pair has zero open source findings on this exact train head.

I read complete #314 integration Round 3 at `2befaf343cedc75dd535a4755be6571863e68567` and package Round 3 at `6ba694457afca3f1e61a7dac0aa708bb203786ce`.
Sol-B receipt: `msg_plugin-w_1791825805_3569e3`.
Both records agree on the exact train head/base/tree, unchanged trace and fixture, and exact known-flaky name filter.
Their 25 earlier synthetic catches contain no newly excluded EV-2 test. Those records keep their original source and head scope.
The package's default-index delta SHA256 is `9635ef61f78726c5a95dfc5d60c2d01324c31ba2355e7b0ebb0e3384038481c3`.
I independently reproduce it. The corresponding full-index hash is the 7d157280 value above.
The #314 pair has zero open source findings on this exact train head.

The complete #312 pair remains pending before the formal combined source verdict.
The fresh gate, final member evidence bodies/receipts, and Q7 remain due.
No running or completed train-4b evidence is accepted by these source receipts. I ran no execution job.

## Round 2 complete — combined train-4b source before the fresh gate

Exact head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
Lead assignment: `msg_plugin-w_1791825491_e99ad8`.
Lead request for the completed combined verdict: `msg_plugin-w_1791825970_0aa174`.
The ordered frozen members remain #314 a4eac3c1, #312 c0f8a09c, and #307 2ed85f93, with full identities above.

The delivery contract, owners, and acceptance plan from Round 1 remain applicable under the corrected R107-SI-2 disposition.
The required new outcome is a reviewed composition on the landed #317 base, followed by one fresh exact-head gate.
The known-flaky list is temporary. The .46 pin must remove its one entry and restore the empty-list assertion together.
A31 and its new OU-7 id remain separate from this train and the EV-2 removal condition.

I read the complete #312 integration Round 7 at `b8337e7eba5b70d753f06cff203e17b60bdbfb65`.
I also read the complete matching package train-4b section at `6fe61678c003a1205e17a1cfd27f0c9e62ccf648`.
Receipt: `msg_plugin-w_1791825962_d18e20`.
Both confirm the exact head/base/tree, accepted #317 base delta, six own blobs, nine conformance blobs, and applicable .44 entry.
Both retain the old RED record and accept source only before the fresh gate.
Neither record has an open source or body finding.

The complete six same-head source receipts are:

- #314 package `6ba694457afca3f1e61a7dac0aa708bb203786ce` and integration `2befaf343cedc75dd535a4755be6571863e68567`.
- #312 package `6fe61678c003a1205e17a1cfd27f0c9e62ccf648` and integration `b8337e7eba5b70d753f06cff203e17b60bdbfb65`.
- #307 package `b8e4fa0d20d4c9b917be2667afecc212d25b5fd0` and integration `12fccdd1383d096d0accb9be752cd44ea2e3dab5`, Round 5 in pc-control-queue.md.

I read every complete applicable section. All six are CLEAN on the exact rebuilt train head.
All prior body and source findings remain closed. The member evidence keeps its original head and scope.

My complete independent source audit above remains applicable without a new source push.
The full-index train delta equals the accepted base delta, SHA256 `7d1572807eb38d9abd05ebd26e4c76db5cbc27bf48e2674089308f1b85b8dde6`.
All 26 prior train blobs remain byte-identical. The combined own diff stays `aa5a215608ed07d9f894c6265a9eb56d1d7d3c7f908dc9aa25926a7d340cde6b`.
The ordered patches retain every accepted own change, with no extra path and no new source resolution.
All 27 conformance set checks pass. Final pending is 403 and real-pending 34; no accepted removal returns.
The scheduler trace, control-room/cancel behavior, hold separation, probe adapter, .44 pin, and checkout resolver retain their accepted interfaces.
The one known-flaky entry cannot match the host trace or IN-6 trials.
It suppresses no unmutated failure and supplies no mutation catch. The later fenced pin retains its explicit removal obligation.
No new combined source, interface, body, or list finding remains.

The lead owns the fresh gate on this exact head. The compile pre-check remains an operator report only.
The gate must supply all required runtime, mutation, and fuzz outcomes, including the unchanged resolver comparison.
The mutation evidence must not credit the omitted EV-2 trial as a catch. The normal slow and unmutated Stage 2 baseline still run it.
Each member's final body and evidence receipt must name the completed exact train gate.
The lead then owns Q7 ancestry, equal-tree merge, and remote verification.

The old train 03709a0c stays RED and will not be re-gated. The #317 green gate is not train-4b evidence.
This CLEAN accepts the combined source only. It authorizes no specific retry and accepts no gate, later head, pin, status removal, or merge.
The remaining work is the fresh gate, all final member evidence receipts, and Q7. I ran no execution job.

VERDICT: CLEAN (0 open; combined source on exact train-4b head, fresh final evidence pending)

## Completed train-4b gate inspection — final member receipts pending

Lead completed-gate request: `msg_plugin-w_1791827181_c344a7`.
The exact train head/base/tree remain the Round 2 identities.
The gate log is `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-4b-e37b91d6-pool-20261012-102808-61022.log`.
I verify SHA256 `97dee8f4d6269dee403c2ff9202709e83d03817bcafd05c04b10cff7725b5d47` and complete its independent outcome/citation audit.
The detailed metrics and retained body finding are in this commit's pc-control-queue.md gate-inspection section.
All ten steps pass; CI total 962.9 seconds, job/wrapper exits zero after 1064 seconds.
Default has 1957 passes. Normal slow and the unmutated Stage 2 baseline each have 633 passes.
The trace determinism test passes in 0.025 seconds. Both own IN-6 transcripts pass in both tiers.
The resolver comparison passes in normal slow and the baseline. Both link fuzz harnesses complete successfully.
All 65 mutant identities are unique. The 39 first-stage catches plus fourteen second-stage catches give 53 catches, with twelve unviable.
The Stage 2 identities equal the Stage 1 misses exactly. All 53 raw catches match their citations and normal PASS.
The excluded EV-2 trial supplies no catch, and passes in both unmutated slow runs.
The thirteen old #307 misses all have new recovery proof; the additional serve mutant comes from #312.
No runtime result is borrowed from an earlier head. The old RED records remain intact.

Sol-B and Sol-C independently report matching gate audits in `msg_plugin-w_1791827246_79467d` and `msg_plugin-w_1791827281_f2a35e`.
Their complete final member/package receipts remain pending.
I read the current complete #314 and #312 final bodies and #307's complete final-body delta.
#307 has LOW R307-SI-5 open for rebuild order and the extra mutant's attribution. Its package final receipt remains pending.
No new combined source or gate finding appears. The formal final combined verdict waits for the correction and all required member final receipts.
Q7 merge/tree/push verification remains due. I ran no execution job.

### Final member records received so far

#307 owner correction `msg_plugin-w_1791827434_69089b` closes LOW R307-SI-5 after my complete corrected-body read.
The correct order is landed #317 base before the three members. The fourteen catches are composed-train evidence, with thirteen #307 misses and one #312 serve mutant.
Exact corrected body hash: `18d06ee4f25951a981e4c5dbd022aca241549f5d2d51e682c2805b670339d666`.
The complete #307 package FINAL remains pending.

I read complete #312 integration Round 8 at `b6a96042e0b4facb73ad895e1fd902c26d687eba` and package FINAL at `ff3fb3392fe3c1ebeba5eb507de0e4bc84355a4e`.
Sol-C receipt: `msg_plugin-w_1791827521_12ac00`.
Both accept the same exact train head, base, tree, gate path/hash, source, body, and mutation outcomes.
Their current body hash `854215e5e3097717ae60f297747db5bfa9413f83fe8ab51fadc0b4bc99f9c0b6` matches my complete-body read.
The body retains historical RED scopes and marks the earlier missing-gate statement as historical.
Both confirm all nine real removals and the a5_2 removal have normal PASS in both tiers.
They preserve earlier Mac evidence limits and make no full-seed real-tier claim: current default seeds are 0-31; normal slow uses seed zero.
They remove no further real-pending id from the twenty-seven passing trials.
Both complete the resolver, known-flaky exclusion, and two fuzz-harness evidence conditions.
Both have zero open findings and retain the future entry-removal/empty-list-restoration condition.

The #312 final pair is complete. The #314 final pair and #307 package FINAL remain pending before combined FINAL.
Q7 remains due. I ran no execution job.

### #314 final pair — complete after replacement receipt

Sol-B final receipt: `msg_plugin-w_1791827666_ca19af`.
I read complete integration Round 4 at `d02a6a40e13d5d1b0d36830bd80d363e478bb32e`.
I read the complete package gate audit and replacement final Round 5 at `efa99fb22d75cb86a0f9cfdde98e011933d095a4`.
The replacement supersedes package `4d21704f3a4ae5f8f9246da979f19932eabb3e92`, which was held for the final body clarification.
Both final reviews name the exact train head/base/tree and the same completed gate/hash.
Both confirm the preserved trace test's normal PASS in 0.025 seconds and zero open source, body, or evidence findings.
Both retain the original synthetic E1 evidence's three-file scope. This gate supplies no new full E1 mutation run.

The LOW B314-I2 clarification separates the pre-.45 EV-2 fence gap and #316 removal from A31's OU-7 obligation.
I independently read the complete body correction at unchanged frozen member head and base.
Body SHA256: `857f7969359223868bff21909ac1ee8fa8b46cd6624a4571f6627be4090c7a3b`.
Raw JSON SHA256: `2e6e90674dafd9dd6665f92a2f8f905ec95712e1478c55deb4a1f0c60cad5826`.
The corrected paragraph preserves the failed old train, the retained logs' causal limits, and the completed new train identities/results.
The final body and both reviews agree with the governing R107-SI-2 disposition.

The #314 and #312 final pairs are complete. The #307 package FINAL remains pending before combined FINAL.
No source or gate finding remains in my audit. Q7 remains due. I ran no execution job.

## Round 3 complete — FINAL combined train-4b receipt

Exact head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
Lead FINAL request: `msg_plugin-w_1791827181_c344a7`.
Completed gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-4b-e37b91d6-pool-20261012-102808-61022.log`.
Independently verified SHA256: `97dee8f4d6269dee403c2ff9202709e83d03817bcafd05c04b10cff7725b5d47`.

I complete the required final records under the existing delivery contract.
The six exact-head final receipts are:

- #314 package replacement `efa99fb22d75cb86a0f9cfdde98e011933d095a4` and integration `d02a6a40e13d5d1b0d36830bd80d363e478bb32e`.
- #312 package `ff3fb3392fe3c1ebeba5eb507de0e4bc84355a4e` and integration `b6a96042e0b4facb73ad895e1fd902c26d687eba`.
- #307 package `4112796926e0e9a6b0cf90bcbcab72294da07bec` and integration Round 6 in this commit's pc-control-queue.md.

I read every complete applicable final section, each current complete member body, and the complete corrections where required.
All six accept the same exact train head, base, tree, completed gate, and preserved frozen members.
The #314 package replacement supersedes its held earlier receipt. B314-I2 closes with the correct R107-SI-2 disposition.
R307-SI-5/CQ-PC-5 closes with the actual rebuild order and composed-train mutant attribution.
All earlier body, source, and evidence findings remain closed. No combined finding remains.

The independent source audit and all 27 conformance set checks remain valid without a source push.
All accepted member interfaces and ordered imports remain preserved. Final pending is 403 and real-pending 34.
The gate checks those sets, the .44 pin, the ledger, and the shared job copy.
It reports 264 passing conformance trials, zero failures, and no unheld pending promotion.
No new removal follows from the twenty-seven passing real-pending trials.
The trace determinism, both IN-6 transcripts, structural IN-8 fix, checkout resolver comparison, and accepted regression tests have normal PASS records.

All ten CI steps PASS on msa1. Default has 1957 passes; normal slow and the unmutated Stage 2 baseline each have 633 passes.
All 65 mutation identities are unique. The 39 Stage 1 catches and fourteen exact Stage 2 recoveries give 53 valid catches.
All raw catch identities match unique citations and normal PASS. There is no final miss, timeout, retry, or FLAKY result.
The twelve unviable replacements establish no behavioral catch.
EV-2 passes normal slow and the unmutated baseline. Its one mutation-stage exclusion supplies no catch credit.
Both decoder fuzz harnesses finish successfully: 6143074 link_decoder runs and 3614151 msg_decoder runs.
CI total is 962.9 seconds. Job and wrapper exit zero after 1064 seconds, with one second queued and 1063 running.

The completed current-head gate satisfies the final runtime, mutation, and fuzz evidence condition.
Historical failed and stopped gates retain their results and scopes. No old evidence is relabeled as a new execution.
The original synthetic E1 proof retains its three-file scope. The current Linux gate makes no new full-seed real-tier or macOS execution claim.
The temporary EV-2 entry must still leave with #316's .45-or-later pin and restored empty-list assertion.
A31 and its separate OU-7 id remain outside this train's completion scope.

The member final evidence and combined review conditions are complete and CLEAN.
The lead must still verify Q7 ancestry, fast-forward merge, gated-tree equality, and remote publication.
This FINAL accepts no later source, future pin, status removal, specific retry, or completed merge.
I ran no build, test, gate, mutation, reversal, reproduction, measurement, or executable base check.

VERDICT: CLEAN (0 open; FINAL combined train-4b at exact gated head, lead merge verification pending)
