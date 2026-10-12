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
