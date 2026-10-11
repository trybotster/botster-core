# PR #296 integration review

## Round 1

- Reviewer: Sol-B. Lead assignment: `msg_plugin-w_1791762845_160255`.
- Branch: `stage1/p6-group-guard-order`.
- Exact head: `1a9ba675cd6f51afd88e657649d22ec525d32f44`.
- Reviewed base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
- Reviewed delta: all five changed files, including the shared fixture users.
- Risk: HIGH. The guard is compiled into three packages' tests. The PR also changes a workspace check allowlist.

No cross-package source defect was found. The shell's FIFO line follows the registration acknowledgement. The test therefore cannot kill its group before the anchor joins it. The registration thread returns the group id for cleanup.

When an external SIGKILL ends the anchor without a report, the guard waits for the group's remaining members without signalling. It reaps only its own anchor. Production retains its own child reaping. Other failures still report cleanup failure.

The wait helper moves from the payload guard to the shared cleanup module. Its clock uses `platform::real_now()`, as the existing cleanup path does. Its deadline value does not change. The moved `mkfifo` call uses `OwnedChild` for a bounded wait. The stale allowlist entry is removed.

The other shared users retain the blocking registration prefix before their worker bodies. The Linux gate runs the guard proofs in sys, Core, worker integration tests, and the worker's driver tests.

### GG-I1 — MEDIUM — OPEN: required evidence and package verdict remain pending

The PR body cites Mac passes and a drop-rule reversal on `eedbf877`, without retained log paths. The exact head has later changes to the shared clock call and the FIFO helper.

Required closure: supply the retained Mac and reversal logs. Supply focused Mac evidence on the exact head for the changed guard paths and affected users. Plan section 8, revision 23ai, applies because the cleanup path reads OS group membership and waits for process exits. Obtain the exact-head package CLEAN from 016a.

An evidence-only reply can close this finding. A source change requires delta review.

### Evidence

- Linux gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-group-guard-order-1a9ba675-pool-20261011-164959-15401.log`.
- The header names the reviewed head and base. All ten steps PASS. The final exit status is 0. The log contains no FLAKY result.
- The changed kill test passes. The shared guard's panic, early-exit, and failed-cleanup proofs pass in the affected test targets.
- The mutation step reports no mutant because the diff changes test code. That result supplies no mutation proof of this fix.
- The retained FLAKY record names the original failed attempt and its guard-drop panic. The mutated testkit path is outside the sys test's dependency graph. No reproduction campaign is required or accepted.
- The PR body includes the structural cause, earlier red gates, repairs, other guard users, and Prior art.
- Sol-B requested the package verdict and the missing evidence. Sol-B ran no local build, test, gate, mutation job, or reversal.
- The current `v1` tip has moved. The final-slot import, base check, applicable delta review, and exact-head final gate remain due.

VERDICT: NOT CLEAN (1 open)
