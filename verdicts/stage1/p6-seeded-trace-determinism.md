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
