# P3 reviewer handoff — CURRENT, amended pause boundary 2026-10-04

The lead amended the pause in `msg_plugin-w_1791178396_bd0c50`.
This reviewer finished PR #165 and remains available for PR #163's merge delta.
M2a and the A32/A33 follow-up remain paused.

- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-review-p3`.
- Branch: `stage1/review-p3`.
- Pushed verdict HEAD: `ea4415fe0551c8023f3545bc3dc25209bf962e9e`.
- Verdict file: `verdicts/p3-worker.md`. Last round overall: 86.
- Tracked tree: clean. The reviewer ran no tests, builds, measurements, mutants, or gates.
- Every earlier finding, closure, and verdict round remains preserved.

## Last verdict per PR

PR #165, branch `stage1/p3-guard-macos`: CLEAN in round 86.
Exact reviewed head: `c03bcfb181d21cdf805b752a790359f63b9ef7b9`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Verdict commit: `ea4415fe0551c8023f3545bc3dc25209bf962e9e`.
All #165 findings F34–F43, including LOW, are closed within this PR's scope.
F43 now preserves observation errors, accepts only an actual exit before reap, and reports every fixture ownership/cleanup failure.
The EPERM exception is accepted because only a subsequent empty listing permits success; retained members still fail cleanup.
The completed focused Mac proof passes 158 + 13 tests and exits 0 after 12 seconds.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-c03bcfb1-mac-20261004-223014-89077.log`.
Integration verdict `b2f7db0` reports zero findings at this head and required this package CLEAN.
This focused proof is not a full landing gate or Linux evidence. The implementer and lead own those remaining landing steps.

PR #163, branch `stage1/p3-audit-fixes`: NOT CLEAN in round 80.
Exact reviewed head: `40b63dceb6e3f3d7be69a1488ac77eccc121a071`.
Its source/filter and actual collection are accepted. F28 native mutation evidence and F33 required landing execution remain open.
F39 remains open for its later merge delta: its outer cleanup waits must allow the inner cleanup and completion/report interval.

## PENDING reviews

- ACTIVE when submitted: #163's v1 merge/restack delta, every conflict resolution, F39, and its exact-head completed evidence for F28/F33.
- PAUSED: M2a restack at `a7f4a386593457e3b30f03b56938092de9b060a3`; no package restack verdict exists.
- PAUSED: assigned A32/A33 driver test/mutation follow-up when submitted.
- Future M2b, GHOSTSNP paging, final A13/A14, and pending conformance duties remain unchanged.

The earlier pause handoff below records the full PR histories, contacts, bindings, and earlier evidence.
Its c03bcfb1 pending item and F43-open state are superseded by this round 86 CLEAN.
Never poll. End the turn and receive once after a doorbell. Do not run gates or spawn agents.

---

## Previous pause handoff — preserved as historical state at round 85

# P3 reviewer handoff — CURRENT, paused 2026-10-04

The lead ordered PAUSE for the user token budget in message `msg_plugin-w_1791178222_5bfea3`.
The reviewer finished the active review, pushed its verdict, and stopped.
Do not start new review work until the user or lead resumes the session.

## Workspace and verdict

- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-review-p3`.
- Branch: `stage1/review-p3`.
- Pushed verdict HEAD: `df09cc8d568ae7b0614f928e124ac0956937377b`.
- Tracked tree: clean. The spawner's .gitignore change was restored at takeover. Never commit .gitignore.
- Verdict file: `verdicts/p3-worker.md`. It contains all original findings, closures, and rounds, plus rounds 74–85.
- No other verdict file was edited by this reviewer.
- The reviewer ran no tests, builds, measurements, mutants, or gates.
- Every verdict was sent to the current implementer and integration reviewer.
- No CLEAN terminal event was sent to the lead during this audit review.

## Last verdict per PR

### PR #163 — stage1/p3-audit-fixes

Last round: 80, verdict commit `6f2693e4527451d2f499cba608629d68f7a65e37`.
Exact reviewed head: `40b63dceb6e3f3d7be69a1488ac77eccc121a071`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Verdict: NOT CLEAN; two evidence findings remain open on this source head.

- F28 MEDIUM: updated pending_output proof needs native Mac Ok(0)/Ok(1) mutation evidence.
  The updated baseline passed at ccba0504. The old round 67 mutation proof tested the removed assertion and does not close this requirement.
  Normal mutants excludes the pending_output functions and uses the default tier, so a full gate alone does not renew this proof.
- F33 MEDIUM: the corrected landing filter and actual collection are accepted; completed required test execution is pending.
  Filter: `binary(/^slow/) | test(/(^|::)slow_/)`.
  The collection log at 40b63dce lists nested storage::slow_tests and real::slow_tests, root slow_tests, worker slow_driver, and the required failed-watch test.
  Required execution: `slow_edges::a_failed_exit_watch_ends_the_worker_with_a_failure` through the corrected landing selection.
- F39 LOW remains a later merge duty: when #163 incorporates #165, its outer bounded waits must allow inner cleanup and completion/report time.
  This duty does not belong to the absent merge delta on the last reviewed #163 source head.

All earlier #163 source findings retain their recorded closures.
The A52 error path is accepted: errno and invariant context reach main, without an invented exit status.
Its real-driver test uses a blocked FIFO member and preserves the independent guard during production cleanup observation.
The earlier Mac full gate at ccba0504 is red, not a passing landing result.
It passed the native PTY count baseline but omitted the worker unit slow tests through the old filter.
Its shared guard and A10 failures are recorded in round 79.
The lead later moved the shared guard correction from P5 to P3; P5 #162 keeps A10.
The separate A32/A33 driver-test and mutation follow-up is not closed by #163.

### PR #165 — stage1/p3-guard-macos

Last round: 85, verdict commit `df09cc8d568ae7b0614f928e124ac0956937377b`.
Exact reviewed head: `4c06846d039b9209cbd2d8d57144bbad807b6d98`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Verdict: NOT CLEAN; one source finding remains open.

F43 HIGH remains open in guard_cleanup.rs's Owned fixture owner:

- ended_within_cleanup sends completion after any non-INTR waitid error.
- Its boolean does not prove an exit before Child::wait.
- It retries Ok(None) without a blocking event.
- Owned Drop returns on try_wait errors and discards kill/reap errors.

The reviewer requires the actual observation result, an observed exit before reap, and visible cleanup errors during normal Drop and panic.
All other #165 findings are closed: F34–F38 and F40–F42 at their recorded heads; F39 is closed in #165 at 08fef89d.
The guard uses group signals while an unreaped reserve holds the group identity.
Both guards use one shared cleanup path.
The module split is accepted: process_guard.rs, guard_cleanup.rs, guard_platform.rs, and payload_guard.rs.
The payload report and registration proofs are accepted.
The driver harness's release-before-production, report-after-production order is accepted.
The driver owner uses an outer allowance of 2 * CLEANUP; the inner cleanup limit remains unchanged.
The reservation proof now checks actual SIGKILL and normal-exit effects with blocked FIFO members.
Its fixture owner remains the sole open source issue at the reviewed head.

Latest accepted focused Mac proof at 4c06846d:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-4c06846d-mac-20261004-222705-81386.log`.
Slow clippy and worker prebuild pass. 158 tests and 13 selected core guard tests pass; nine other core tests are skipped.
The job exits 0 after eight seconds. This is not a full landing gate or Linux proof.
Integration verdict `7dc2559` reports zero integration findings at 4c06846d, conditional on this package's exact-head CLEAN.

## PENDING reviews and evidence

1. PR #165's NEW, UNREVIEWED head `c03bcfb181d21cdf805b752a790359f63b9ef7b9`.
   The implementer submitted it after the lead's PAUSE message. The reviewer recorded the submission and did not inspect its delta or log.
   Review `4c06846d..c03bcfb1` for F43.
   The implementer claims observation errors, Ok(None), and cleanup errors are now explicit.
   It also changes group-kill EPERM handling: like ESRCH, EPERM permits the next listing, with a retained member failing at the deadline.
   Review that new exception and its decision proof; do not assume the prior ESRCH ruling covers it.
   Claimed focused Mac proof: 158 + 13 tests pass.
   UNREAD log: `~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-c03bcfb1-mac-20261004-223014-89077.log`.
   Submission message: `msg_plugin-w_1791178241_fa35b8`.
   The integration reviewer reports zero integration findings at `b2f7db0` on c03bcfb1, conditional on this package CLEAN.
   Its message refers to an amended pause, but this reviewer has received no amended lead order. The source and evidence review remains pending.
2. PR #163: completed native mutation outcomes for F28 and corrected landing execution for F33 remain owed.
   No completed correction-head evidence for these requirements has arrived.
3. PR #163's later merge/restack after the guard and P5 fixes: review the actual delta and every conflict resolution, including F39.
   The lead's recorded order is guard #165, P5 #162, #163, M2a #142, then #161, with the required reviews and one permitted gate per step.
4. M2a restack at `a7f4a386593457e3b30f03b56938092de9b060a3`: no package restack verdict exists.
   The old M2a CLEAN applies only to `98960e434b0991ebb9d7e65c952f1c6ea116b83a`.
   Preserve the a2_8 pending rule and the actual required harness proofs.
   Review the submitted exact head and evidence after the lead resumes its place in the merge order.
5. The separate A32/A33 driver mutation/test follow-up remains assigned work. Review its exact head when submitted.

M2b remains future work: terminal model, paging, snapshots, ReadFacts/input records, tap, disable_history, final A13/A14 duties, and both conformance harness proofs.
Complete the GHOSTSNP Worker paging duty in its paging PR. Do not close pending conformance ids from an earlier package CLEAN.

## Authority and workflow

Keep the binding documents and rules in the previous handoff below.
The current contracts pin remains contracts-v0.1.13 / `a8db5c9a0f43fc440564989fd38a55121fdda38b`.
Read the current lead handoff and the actual lead state log:
`~/botster-sessions/botster-v1-orchestrator-state.md`.
The shared botster-resume-state file is not that log.
The last resource ruling permitted one Mac job at a time and held Linux work; the PAUSE now stops this review session.
Read completed evidence only. The reviewer runs no gates.
Never poll. End the turn and call receive_messages once after a doorbell.
Do not spawn agents. Preserve all pushed history and every earlier finding and closure.

Current contacts:

- Implementer: `sess-1791168825-010a-e873347e8fdb315566b5545e5c2d418b`.
- Integration reviewer: `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9`.
- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
- This reviewer: `sess-1791169299-010e-6e279534401cc89d86c447f2fa43f4bc`.

Send verdicts to the implementer and integration reviewer.
Send the lead only QUESTION, a BLOCKED condition needing a lead decision, or a terminal event.
The lead explicitly requested the current PAUSED report with verdict HEAD and pending list.

---

## Previous handoff, preserved as historical state at round 73

# P3 reviewer handoff — 2026-10-04

## Workspace and verdict

- Worktree: /Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-review-p3.
- Branch: stage1/review-p3. The worktree is clean.
- Verdict file: verdicts/p3-worker.md. Last round: 73.
- Pushed verdict commit: 22124c35f2bb2d785f92182a90c6cfe9149d6fb4.
- Last package verdict: CLEAN for P3 M1 on da2b0494bbda711e5a67cb180ddf05c607784635 (PR136).
- All findings F1–F24, including LOW findings, are CLOSED. Preserve every finding, closure, and round.
- The verdict header records the current exact head. Earlier CLEANs apply only to their named milestone and head.

## Current boundary

The lead requests replacement of this reviewer at the idle point. The current verdict is already pushed and sent.
No review, tool execution, gate, or edit is active.
The implementer handoff reports M1 merged as 01fd38968b9e7605becc7e2b5088628aff52865a.
It reports integration CLEAN c9a1177bf3797b57dba0449b25e02a68becfb9f0 and a passing ten-step Linux gate.
Gate log: ~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-da2b0494-linux-20261004-190335-66474.log.
That handoff reports 328 mutations: 261 caught, 67 unviable, zero misses/timeouts.
This reviewer has not independently read that final landing log. The package CLEAN preceded the required landing gate.

Next branch: stage1/p3-m2a-v1, pushed a7f4a386593457e3b30f03b56938092de9b060a3.
No M2a restack review or verdict exists. No M2a PR exists according to the implementer handoff.
No M2a exact-head review request is pending in this reviewer's inbox.
The implementer plans Linux proof of in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write, then M2a mutation accounting.
Do not run that job as reviewer. Wait for the implementer's submitted head and evidence.
M2a changes the testkit's harness.rs, program.rs, and worker.rs, so it also needs integration review.

## Review scope and authority

Read pair-common.md, brief-p3-restack.md, handoffs/p3-worker.md, and brief-p3-worker.md in ~/botster-sessions/shared/core-stage1/.
Read ~/botster-sessions/pins/stage1-plan.bdda2359.md and ~/Projects/botster-contracts/docs/BUILD.md.
The contract pin is contracts-v0.1.13, dependency commit a8db5c9a0f43fc440564989fd38a55121fdda38b.
A13 is final. R-28 and erratum 2 apply.
Review each restacked PR's delta from its earlier reviewed version. Name every conflict resolution.
Old M1 CLEAN: review commit 30bb483, old implementation head 46b16945ead49715949d5983bb41a673c081f8ad.
Old M2a CLEAN: review commit 5eb1f130439e64c098212aeb9d4b8f6590aa7a91, old implementation head 98960e434b0991ebb9d7e65c952f1c6ea116b83a.
Neither old-base CLEAN clears a new restack.
CLEAN requires an exact head and closure of every finding, including LOW.
M1 package CLEAN covers P3 scope; integration CLEAN is separate.
The reviewer runs no tests, builds, or gates. Read source and completed evidence only.

## Binding review rules

- libghostty owns terminal semantics. Derive expected terminal values from it. Do not hand-write expected terminal bytes.
- Keep one production Worker/Driver path. Dependency injection is permitted. Production test branches are not.
- A test guard owns and kills the process group on Drop, panic, and test-parent death.
- The guard must not reap the payload that production reaps. Retain ownership through signals. Never signal a released cached group identifier.
- Children must not busy-spin. They must exit when the test parent is gone. Use event waits and marked deadlines.
- Tests must prove a clause or a real bug. Do not add tests solely to kill a mutant.
- Every mutation exclusion needs one function or constant, a reason, and concrete proving test names where applicable.
- Actual assertion failure can close a mutant. An external mutation timeout cannot.
- Verify raw outcomes and actual failure logs. Match relocated mutants by source role, not operator text alone.
- Preserve conformance ids as pending until required testkit and real-harness proof exists. M2b and real-process conformance remain open.

The lead's pure-tuning ruling requires four conditions: no clause fixes the value; retention/order/complete-frame proof at default, 1088, and the legal minimum; all-build enforcement of the lower bound; an exact exclusion with reason and proving test names.
The Driver and testkit positive-bound entries satisfy that rule at M1. Recheck changed callers and source.
Worker::report_exit's OR-to-AND equivalence is accepted only for M1's reachable call states. Recheck it when M2 adds callers or reconnect behavior.
Payload::reap, wait_unreaped OR/XOR, and set_nonblocking OR/XOR have exact written premises. Preserve and recheck those premises.
Rounds 68–71 review OS adapter exclusions and concrete test names. Round 73 accepts only Payload Debug::fmt as printing glue.

## Evidence and remaining terminal work

All 167 original M1 mutation survivors have recorded dispositions and retained name artifacts under verdicts/.
Later Linux query/cleanup and native Mac helper mutations are accounted for in rounds 64–67.
Useful raw evidence:
- /private/tmp/p3-payload-corrected-linux-evidence (30 tested, 25 caught, five unviable).
- /private/tmp/p3-payload-mac-evidence (native helper Ok(0)/Ok(1), both caught).
- /private/tmp/p3-command-line-evidence (all three execute mutations caught).
- /private/tmp/p3-driver-pty-evidence and /private/tmp/p3-driver-flush-evidence.
Native log: ~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-63b5c1db-mac-20261004-181240-42534.log.
The native helper baseline passes 47/47. Both mutants fail pending > 1 in 0.027 seconds, Failure(100).

M2b must complete snapshots, ReadFacts/input records, tap, disable_history, and all 11 A13 ids.
Complete GHOSTSNP.md's Worker paging section with R-30/A8-2 code references in the paging PR.
Use contiguous Page { index, bytes, last } values for native encoded bytes. Add no snapshot framing.
P6 live observer controls and every-cut fit proof wait for that implementation.

## Messaging and workflow

- Lead: sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673.
- Current implementer: sess-1791168825-010a-e873347e8fdb315566b5545e5c2d418b.
- Current integration reviewer: sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9.
- Retired sessions …00ff and …0101 must never receive messages. The shared implementer handoff still contains stale contacts.
- Send verdicts to the current implementer and integration reviewer.
- Send the lead only QUESTION, BLOCKED, or terminal CLEAN with exact implementation head and verdict commit.
- The lead clarified that BLOCKED is only for a decision or obstruction the lead must resolve. Routine finding corrections stay in the reviewer/implementer loop.
- Never poll. End the turn while waiting. After a doorbell, call receive_messages once.
- post_message takes {session_uuid, payload}. Use the botster MCP tools through functions.exec.
- Git fetch/add/commit/push require sandbox escalation because the worktree metadata lies outside the writable root. Those authorized review operations have succeeded.
- Never rewrite pushed history. Do not spawn agents.

## Exact reviewed implementation heads

The complete chronological findings and head-specific scope are in verdicts/p3-worker.md.
The following unique exact reviewed heads are extracted from that file:
- da2b0494bbda711e5a67cb180ddf05c607784635
- 98960e434b0991ebb9d7e65c952f1c6ea116b83a
- 46b16945ead49715949d5983bb41a673c081f8ad
- 5a41a33fd65468dcddb9fe025e8f645743693e00
- 049751aed4b97fc7b817c0201119c0e363a73935
- ed92707f51f80ddd31f3b3c9fb88f4151e94cdbb
- 4de71bbf5dcb5d95ff3b9ad12a4b797a7166ae61
- 0044bf7346ca486bf09b5265dbfc0c75f93dd978
- 78b88fa99546059dc5d8500af890432edd855537
- fdd2b8e73927d592b75713c55a86c6ea8c60c037
- 91f8a4260a5e0ffb0721872400ad03da4868498e
- cc34474de63030b74031f5a21d6608bf27a4a843
- 2e9811ca0b1c8a790698999efc24c538b5e755d7
- 91b58e2c8a8f7d198484e8019f08c50aae0e6dd4
- 34bad40b4ee3d97eab1866b991c4e2ba5251a7dd
- c1af2211cedce075be7af5f63fa1cca51f0a6647
- 4960d73873d8301575e312f8fbc596059300c813
- 5172a53553636dc94d9c86c98b0969499b1955dd
- 3d13dda8fb2baad575495501a083d0244eb726ce
- ebed1022a1f3804712342f8288902f42ef1db159
- a532a7292a3a419cb9903b1425b22ef331f05be2
- a32542a887ca013afea3c787d557caa010e663fd
- ec8cee11fea200ef6db9dd2d80f5200d5dca2180
- 4b1d39cb294dfc4308dae1ddb0f2ce4f6274170a
- 0e66b74f97d2f6b7665fcff1310f83bfd0bca4b2
- 39a55c9e8fd8ecc6d4678ba952a363d99fa04139
- e4593e86feab06da3711bef9d3ac06bb7e20be38
- 7efe4553d290cd6f76befc0d50ed7b5206b0ea0e
- 7b54136568a88f1bbbf599de37002eb8daed363c
- e9efad5e788754bfc3c545b3bdfbfc5333238130
- 1c45103cd0d63ddef9cccb5c6934442ff0e4ea29
- e475c2841368c6b71bfaf0497b7d06b4d3e0fab9
- c731d03e0fbe2ad73d8f152ec67525c09c66f833
- 3fd4144515e8e903d6417adf56d12b7f038d8b70
- 63b5c1db6cec74b27c7b10d34cbbb37e99e60986
- 3f9fe56ed466bf9f3ed7fde827a80168b69c3046
- 533fd3a44d6d14a26702ad3796290ee2fafebefd
- 9eb9a59fcdd798d86cf8a7b96fc2c108e19282dc
- b77038acb506f804262f3d9c99923aeceefb2683
