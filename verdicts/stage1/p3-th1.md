# P3 TH-1 integration review — PR #202

## Round 1 — 2026-10-09

Reviewed head: `2dc7dacfd90468bc55474ff020ca58312a66cd42`.
Base: `fe0e6d6d5f9df40838df957b7d8f5d0564712a6a` (`v1`, including #201).
HIGH is correct under rule 3 because the change crosses the facade test target and shared testkit.
Rule 5 also applies to the new public testkit builder.
The reviewer read the complete three-file diff, the facade type, test-target configuration, pinned transcript, runner, and supplied gate.
The reviewer changed no product code and ran no tests, builds, mutants, or gates.

### Source and proof

`tests/conformance.rs` uses compile-time assertions for `botster_core::Core: Send` and `botster_core::Core: !Sync`.
These assertions name the public facade type, which contains the real host driver and its existing `Cell` marker.
The assertions are unconditional in the conformance test target.
The existing `static_assertions` development dependency supplies both assertions; this PR adds no dependency.

The runner passes `CORE_IS_SEND_NOT_SYNC` to `TestkitHarness::with_core_type` for each seed.
The true constant records a property that the compiler checks before the runner can execute.
`TestkitHarness` retains `None` by default and preserves both supplied boolean values.
The harness test checks `None`, `Some(true)`, and `Some(false)`.
The testkit does not need a dependency on the facade to report the runner's checked result.

At contracts commit `03891658`, the TH-1 transcript contains one `type_check: send_not_sync` step.
The pinned driver accepts `Some(true)`, fails `Some(false)`, and reports unsupported for `None`.
The replacement map assigns TH-1 to `static` proof.
The concrete assertions, runner wiring, and passing transcript satisfy that proof requirement.
This PR changes no production type or behavior and makes no runtime concurrency claim.

### Accounting and evidence

The pending diff removes only `conf::th_1_core_is_send_not_sync` and adds no id.
The approved minimum list contains 70 ids. Comparing that list with the base and head gives 34/70 and 35/70.
The exact-head gate contains an individual PASS record for TH-1.
No separate real-process test or real-harness progress is claimed.

The head contains the stated base. The PR description names the reviewed head.
`git diff --check` reports no error.
The change adds no contract pin, mutation exclusion, process fixture, or timeout change.

Supplied log:
`~/botster-sessions/shared/core-stage1/evidence/p3-th1/gate-2dc7dacf.out`.
The log names the exact head and base. It ran on Linux node `msa1`, allocation `fd07aa89`.
All ten CI checks passed. The default tier passed 1152 tests. The slow tier passed 249 tests.
The conformance report records 104 passed and zero failed.
Both mutation runs report four mutants: three caught, one unviable, zero missed, and zero timeouts.
The separate mutation run uses `NEXTEST_PROFILE=slow`.
The gate exited zero after 189 seconds.

The reviewer also read `~/botster-sessions/shared/core-stage1/evidence/p3-th1/red-th1-false-2dc7dacf.out`.
P3 identifies this as a focused run with the constant changed to false on the reviewed head, followed by source restoration.
The log shows successful compilation and one transcript failure at seed 0, step 0, with the expected type-check mismatch.
It therefore reaches the intended assertion rather than failing during setup. P3 reports exit 101.
The reviewer did not execute that experiment.

P3 corrected the PR description's tier-count labels at the reviewer's request. The reviewed head did not change.

The reviewer read package round 131 at `692572aaf6e60832d0d3705b1d6aa19ffe9a9363`, file `verdicts/p3-worker.md`.
That verdict is CLEAN on this exact head. No integration or package finding remains.

VERDICT: CLEAN (0 open) at 2dc7dacfd90468bc55474ff020ca58312a66cd42
