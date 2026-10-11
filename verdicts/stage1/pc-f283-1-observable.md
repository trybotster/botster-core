# PR #290 integration review

## Round 1

- Reviewer: Sol-B.
- Branch: `stage1/pc-f283-1-observable`.
- Exact head: `4fb31c10bde8eba1b947f7a142e88568bfb22cd6`.
- Base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- Reviewed delta: all three commits from the base to the exact head.
- Risk: HIGH. The PR changes the shared testkit crate.

The production change removes `ProgramControl::output_unread` and its unused counter. `ScriptedProgram::unread` remains in the bounded drain path. No control, interface between packages, contract pin, conformance list, or production machine changes.

The replacement tests read program bytes, input logs, readiness, Core screen results, registered controls, and `PumpReport.more`. The `accept != Some(0)` guard remains. The R1 reversal shows that removing this guard makes a pump report `more` with no runnable work.

### B290-1 — MEDIUM — OPEN: the required scope remains incomplete

The package reviewer reported this finding on the exact head in message `msg_plugin-w_1791760695_02f156`. The lead requires this PR to remove every private-state assertion in the program-edge tests.

Evidence: `program.rs:779,794,796,802` assert `ignores_sigterm()`. Lines 854 and 862 assert `window_size()`. These getters expose stored fields. The package reviewer has not issued CLEAN.

Required closure: replace these assertions with observable behavior, or record an accepted coverage and removal disposition. Obtain the package verdict on the repaired exact head. Sol-B will review the resulting delta. This entry tracks the package finding as the integration prerequisite; it adds no separate package-scope review.

### Evidence

- Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-pc-f283-1-observable-4fb31c10-pool-20261011-160304-16491.log`.
- The gate header names the exact head and base above. All ten steps PASS. Exit status: 0.
- Mutation result: 8 caught, 0 missed, 0 timeout, 0 unviable. The gate names each catching test.
- Reversals: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/pc-f283/reversals-4fb31c10.log`. R1–R4 fail the named tests. The restored source passes 25 tests.
- The implementer corrected the PR body's gate result and reviewer assignment without changing the head.
- Sol-B ran no build, test, mutation job, or gate.

No separate cross-package source finding is open. B290-1 prevents integration CLEAN.

VERDICT: NOT CLEAN (1 open)
