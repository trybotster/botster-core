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

## Round 2

- Exact head: `cc491863b4143d37a3e9586eb1848db0bb9c3a5f`.
- Gated parent: `c542a9fd93be950cb77522257b1b4e3741920a52`.
- Imported source base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
- Gate comparison reference: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
- Delta reviewed: the program repair in `9757243c34ed1fb8380391099060f3db7571441b`, the base import, and the final comment correction. The PR still changes only the three testkit files. It changes no conformance list or pin.

B290-1 is CLOSED. The SIGTERM tests observe `signal(15)` and `poll_exit` before the ignore step, after that step, and after an earlier Hold. The validation fixture includes Hold so automatic exit cannot hide the signal behavior. The removed `ignores_sigterm()` getter has no runtime caller. The flag remains in the signal implementation.

The `window_size()` getter and unused stored size are removed. No script step reads that size, so the removal changes no observable program behavior. The remaining getters supply registered controls, readiness, or the bounded drain.

The coverage statement is corrected. The named host resize test proves forwarding and order only. It supplies no new worker PTY/model resize proof. The separate worker size-operation gap remains open outside this PR. The final head changes one comment line from the gated parent and changes no implementation, macro, or attribute.

Package CLEAN: `f99ed57c0522ec52fd0613a4395e4d7c53fe4c4d`, `verdicts/pc-f283-observable.md`, round 2. Sol-B read the complete verdict. No package finding remains open.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-pc-f283-1-observable-c542a9fd-pool-20261011-170645-20432.log`. It covers the parent, not the comment-only head. All ten steps PASS. All eight mutants are caught, with no missed mutant or timeout. The gate supports the unchanged implementation and tests. It is not the final-head merge gate.

Supporting reversals: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/pc-f283/reversals-9757243c.log`. R5 and R6 each fail three tests. R7 fails two tests. The restored source passes 31 tests. These local records do not establish a complete clean history. The pool gate independently passes the behavior tests.

No integration source or evidence finding remains. The final-slot import, required base check, applicable delta review, and exact-final-head gate remain due. This CLEAN accepts only the named head's source and recorded evidence. Sol-B ran no local build, test, gate, or reversal.

VERDICT: CLEAN (0 open)
