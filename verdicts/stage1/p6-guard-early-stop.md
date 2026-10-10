# PR #224 — Payload guard before registration

## Round 1 — 2026-10-10

Reviewed head: `984d80e294a676c2c2d641f277a4eded3dc02af1`.
PR and gate base: `26843c74b460688c8e7219eb48a60420b930061e`.
Tier: HIGH. The change affects a shared test guard and its cleanup verdict.
Scope: the complete changed file, readiness helpers, PR body, gate, reversal, and package verdict.

No integration finding remains.

### Source review

The guard sends readiness only after the member and ready helper both register.
When registration ends without a member, the guard now returns its existing unregistered outcome.
The ready helper receives EOF and exits 1. The shell prefix then exits before it runs the payload command.
The shell and helper can already exist; this outcome does not prove that no process started.

Accept errors, read errors, unexpected tags, and invalid groups still fail the guard.
A registered member still receives cleanup and must report success or leave a group that becomes empty within the existing limit.
Production retains responsibility for reaping the payload.
The change adds no process spawn, sleep, or polling loop.

The invalid-group proof retains its error-message and absent-readiness assertions.
The new proof registers only the ready helper, releases the guard, and checks successful guard drop and EOF without readiness.
Its socket read uses the existing cleanup limit.
The observed P5 log contains the ready-only guard failure in the LC-5 stop test.
The proposed signal ordering remains an inference, as the PR body states.

### Evidence

Gate: `botster-core-stage1-p6-guard-early-stop-984d80e2-pool-20261010-100041-1057.log`.
The header names the reviewed head and base. All ten stages pass in 333.5 seconds.
Default tests: 1,479 passed. Slow tests: 381 passed.
The new proof, invalid-group proof, and cleanup-failure proof pass in all three binaries that include the guard.
Conformance remains 193 testkit passes, 407 pending trials, 70 entries without transcripts, two deferred trials, and 18 withdrawn trials.
Minimum counts remain testkit 50/69, real-passing 29/68, and real-accepted 29/69.
The report retains 87 pending-real ids with zero passes.
Both mutation stages report no mutants because the diff changes excluded test code.
The pool job exits zero after 343 seconds on MS-A1; the wrapper exits zero after 344 seconds.

Reversal: `botster-core-stage1-p6-guard-revert-49378f76-pool-20261010-100646-11714.log`.
Scratch head: `49378f76e48c79b193398e0ecbfaaa20e90e14f3`.
Its only source difference restores the old ready-only error branch.
The new proof fails with the expected guard error. The invalid-group proof passes.
Nextest exits 100. The probe shell prints that status and exits zero; its wrapper status is not a passing test result.

The package verdict is CLEAN at `bf689f7e80fd54fae51bb15a52e43e14c8f1c97c`, in `verdicts/p6-guard-early-stop.md`.
I read that verdict and confirmed its agreement with the source and retained evidence.
The remote implementation head and base match. The base is an ancestor, and `git diff --check` passes.
I changed no product code and ran no builds, tests, gates, or reversal jobs.

VERDICT: CLEAN
