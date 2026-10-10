# P6 review: payload guard before registration

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/224.
Exact head: `984d80e294a676c2c2d641f277a4eded3dc02af1`.
Base: `26843c74b460688c8e7219eb48a60420b930061e`.
Risk tier: HIGH. The PR changes a shared test guard and its cleanup verdict.

No package finding remains.

The reviewer read the changed guard and its readiness helpers.
`serve` sends readiness only after both the member and the ready helper register.
If registration ends without a member, `serve` returns the existing unregistered outcome and closes the ready stream.
`payload_ready` fails on EOF and exits 1. The prefix's `|| exit 1` prevents the payload command from starting.
The payload shell and ready helper can already exist. The accepted case does not mean that no process started.
The PR body makes this distinction and identifies the signal ordering as an inference.
The observed P5 failure establishes the ready-only guard failure. It does not prove the signal ordering.

The change preserves failures for accept and read errors, unknown or second tags, and invalid groups.
The invalid-group test retains its failure-message and absent-readiness assertions.
The new test observes successful guard drop and EOF with no readiness byte.
It bounds the socket read with the existing cleanup limit.
The LC-5 session test retains its public exit-signal and worker-survival assertions.
The change adds no process spawn, reaping, sleep, or polling loop.
Production keeps responsibility for reaping the payload.
The Prior art note records the F37 rule that changes and the existing outcome and proof forms that the PR reuses.

The reviewer read the exact-head gate log:
`~/botster-sessions/gates/botster-core-stage1-p6-guard-early-stop-984d80e2-pool-20261010-100041-1057.log`.
All ten jobs pass, exit 0. Default: 1479 passing tests. Slow: 381 passing tests. Conformance: 193 passing.
The new test passes in all three binaries that include the guard.
Minimum counts remain testkit 50/69, real 29/68, real-accepted 29/69.
Both mutation commands report no mutant in the diff because the changed file is excluded test code.

The reviewer also read the existing red-on-revert log:
`~/botster-sessions/gates/botster-core-stage1-p6-guard-revert-49378f76-pool-20261010-100646-11714.log`.
Probe head: `49378f76e48c79b193398e0ecbfaaa20e90e14f3`.
Git objects show that the probe restores only the old ready-only verdict and retains the new test.
The new test fails with the expected guard error. The invalid-group test passes.
Nextest exits 100. The probe shell reports that result and exits 0; this is an expected failing test, not a failed full gate.

The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.

VERDICT: CLEAN
