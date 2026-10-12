# Core #314: integration review

## Round 1

- Reviewer: Sol-B.
- Lead assignment: `msg_plugin-w_1791820076_b7954f`; #314 goes first in train 4.
- Exact head: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
- Tree: `e6bd7b251e303cee5f2634705d683c2205d1273a`.
- Source and gate base: `080987829a2e2245a7ea97606020400392c974d6`.
- Risk: STANDARD; one test file in core-host.

Sol-B read the complete delta, full test helper, full body, completed gate, and focused mutation evidence.
The source changes no production behavior, API, conformance status, mutation exclusion, contracts pin, or timing allowance.

The seeded policy and two-session start/run/stop workload remain unchanged.
The helper returns every public scheduler observation: choice point, candidate count or bound, and returned answer.
For seeds 0–15, the test compares two complete traces and requires a nonempty trace.
It requires at least one trace to differ from seed zero's trace. It does not require all sixteen traces to be distinct.
The Arc ownership check concerns the recorder fixture's lifetime; the behavioral assertions inspect public scheduler observations.
No process, sleep, wait, reap, or busy loop is added.

The test removes the permanent BEFORE_E1 fixture, digest, and ignored trace printer under the lead's ruling (c).
The module retains the earlier comparison's history and accepted train-2 gate reference.
The earlier E1 before/after proof remains accepted at its original heads and workload scope.
This new test checks determinism on one head. It supplies no new comparison with the pre-E1 implementation.

### Focused mutation preservation

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-p6-trace-evidence-3e7fed26-pool-20261012-083210-88748.log`.
SHA256: `1cf43e25a58450dfe35441d5901887837a7eb382fcb974ce1410a459618a8f28`.
The wrapper names v1 as its base. Its recorded command explicitly changes BOTSTER_CI_BASE_REF for the mutation step.
The effective mutation base is `7c16332a74a600335fbbc1e671214f6e43db81e3`.
The evidence head is `3e7fed2668a0b36c4b69176c2ceb4a14515f94f4`.

Sol-B independently inspected both synthetic commits and their parent chain.
The evidence head's full tree equals this PR's tree.
Its landing diff changes only scheduler.rs, driver.rs, and engine.rs.
Every added and deleted line in those three files matches the accepted E1 source patch at the first train-2 merge.
This evidence covers those production source files. It supplies no new mutation run of every testkit E1 control.

The unmutated baseline passes. Stage 1 reports 26 mutants: 25 caught, one unviable, zero missed, and zero timeout.
Sol-B independently matches all 25 raw catches to citations and normal PASS in the exact-head full gate.
Those identities exactly equal train 2's caught subset for the three selected source files, including line, column, and change.
No credited train-2 catcher for those identities is the removed historical trace test.
The new records establish those same valid catches without the historical comparison.
The unviable begin replacement supplies no catch evidence. Stage 2 does not run because stage 1 misses none.
The mutation run takes 37.5 seconds. The job exits 0 after 101 seconds; the wrapper exits 0 after 102 seconds.
The recorded queue time is one second. No FLAKY record appears.

### Full gate and package receipt

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-seeded-trace-determinism-a4eac3c1-pool-20261012-083442-4144.log`.
SHA256: `232026590f75fbfb4cabad3363ea8f8649ebe6bb96ae779dcef441dff5533834`.
Sol-B read the exact-head/base header, changed-test result, tier summaries, gate decisions, and final exits.
All ten steps PASS in 466.3 seconds. The new determinism test passes in 0.030 seconds.
Default passes 1,935 tests, with 430 skipped. Slow passes 619, with 2,068 skipped.
The source and gate base are equal to the merge base. No conformance identifier changes.
The full gate starts no mutation run because this test-only diff has no mutant.
The separate source-scope job above supplies the mutation preservation evidence.
Fuzz reports no changed decoder-harness crate. No FLAKY record appears.
The job and wrapper exit 0 after 537 seconds, with one second queued and 536 seconds running.

The full body identifies the source-only mutation scope, synthetic evidence branch, equal tree, full gate, and earlier comparison history.
Sol-B verifies the old-Core Prior-art search at 72b2e335. It finds only the archived fake seeded-state note, not a scheduler counterpart.
Sol-B read complete package verdict `fe8837e9d4772705288ba85163ed815cadebd45a`, `verdicts/p6-seeded-trace-determinism.md`, Round 1.
It accepts this exact head, tree, base, source, and evidence with zero findings. Its conclusions agree with this audit.

### Disposition

No source, body, or evidence finding remains on this exact head.
This CLEAN accepts the source and initial gate/evidence. It accepts no later import, train, or contracts pin.
The lead's train-4 procedure, exact delivered-head gate, final receipts, and equal-tree/push verification remain due.
Sol-B changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open)

## Round 2: train 4 source delta

Exact train head: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`.
Tree: `4bb9e8c7f7d6e8b610e4b80ce782499ae3389cde`.
Base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Accepted #314 member: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
Lead assignment: `msg_plugin-w_1791821464_bb4328`. Sol owns the combined review; Sol-B owns this #314 delta.

Sol-B reads the three merge objects and ordered first-parent chain.
Merge `b3f7f39b72bc2a985634240573b2d08660b162c6` joins this base and #314.
Merge `13fc69a0a196f75a6aaa1662c1deac1817e5dd88` adds #312 member `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
The final merge adds #307 member `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
The first merge changes only the seeded trace file. Its added and deleted lines match the accepted #314 patch exactly.
The member, first merge, and final train contain the same file blob. Later members do not change it.
The driver retains the seeded_trace module. The public Scheduler source has no train delta.

Sol-B reads the shared driver fixture changes and relevant control-room, engine, and testkit changes from #307.
Each trace run creates a fresh recorder, seeded policy, and Rig. The existing setup uses no global trace state.
The mock link's new room override defaults to None, which returns unlimited edge room.
The trace workload starts and stops two sessions. It sets no room override and calls no cancel.
The new room-processing path requires a session that waits after a cancel refused Full.
That path therefore does no work in this trace workload.
The trace still records public choice points, candidate counts or bounds, and answers.
The full-trace equality, non-empty trace, and seed-variation assertions remain unchanged.
This composition adds no private production-state assertion to #314.

Sol-B reads exact-head package receipt `f852df5d103aefa6f97ff1314083506a1845dc76`, its complete Round 2.
It agrees with these source checks and reports zero findings.
The original mutation preservation evidence and full gate retain their original head and scope.
Neither record supplies a train 4 execution result. This receipt accepts no running gate or pre-check report as a final gate.
The completed exact-head train gate and final evidence body and receipts remain due.

No #314 source delta finding remains on this exact train head.
This verdict covers the preserved #314 test and its shared fixture composition only.
Sol's combined review and the other members' package reviews remain separate requirements.
This verdict accepts no later train head, pin, status removal, or #306 retry.
The lead owns the final train procedure and merge/tree/push verification.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; #314 train 4 source delta only, final train evidence pending)
