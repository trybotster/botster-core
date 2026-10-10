# P6 review: two-stage mutation verdict

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/225.
Exact head: `8cce58801c3e5ca63b7550d5b81aca2cc9c5325e`.
Review and gate base: `150a2069428173c97892160b73e94993233f413a`.
Risk tier: HIGH. The PR changes the gate decision, mutation exclusions, and retained gate artifacts.

Three package findings remain. The reviewer sent them directly to P6 and integration.

- **MS-R1-1 — HIGH: the new slow baseline has no bounded execution or cleanup verdict.**
  In `xtask/src/ci.rs:466–472`, `mutants_job` runs the stage-2 baseline with bare `Command::status()`.
  Its `mutants` profile never terminates a test. This call has no deadline derived from `SLOW_DEADLINE`.
  It also bypasses the PID tracker, cleanup, and survivor verdict that the existing bounded execution path provides.
  A hung unmutated baseline can wait until the outer pool deadline. The call supplies no separate survivor check.
  Use the existing bounded execution and cleanup path for this new baseline. Keep its workspace, features, filter, and profile.
  Add public behavior proofs that a hung baseline and a baseline survivor fail before stage 2 starts.
  Keep production responsible for reaping its own children.

- **MS-R1-2 — MEDIUM: a later failure discards completed stage reports.**
  `two_stage_decision` stores reports in a `Vec<String>`. `mutants_job` prints that vector only after the decision succeeds.
  A failed baseline returns only its baseline context. A stage-2 miss or timeout returns only its stage-2 context.
  These paths discard the earlier stage times. Plan revision 23r requires the gate to print each stage's time.
  Retain completed stage reports when a later stage fails. Add a behavior proof for the failure output.

- **MS-R1-3 — LOW: the command proof also asserts private state.**
  `stage_2_tests_exactly_the_missed_mutants_with_the_slow_tier` asserts `test_budget::SLOW_DEADLINE.as_secs() == 600` at line 1375.
  The adjacent exact argument assertion already checks the public `--timeout 600` behavior.
  Remove the redundant private constant assertion, or replace it with a public behavior assertion.

The reviewer read all eight changed files and the PR body at this exact head.
The Prior art note records reuse of the existing closure-based decision, slow fixture proofs, and manual slow-run command form.
It also records use of cargo-mutants' existing exact-name filter, timeout, in-place run, and outcomes format.
The reviewer checked cargo-mutants 27.1 source. Its baseline selects the mutated packages explicitly, despite `--test-workspace true`.
That source supports the separate workspace baseline. MS-R1-1 concerns how the PR executes that baseline.

Stage 2 uses escaped, anchored `-F` patterns for only the stage-1 misses. It uses the same diff and exclusions.
The workspace features come from the existing slow-package selection. The per-mutant timeout comes from `SLOW_DEADLINE`.
Stage 2 runs serially in place. Both mutation stages use the profile without test termination.
The decision rejects a timeout in either stage, a failed baseline, absent outcomes, and incomplete outcome counts.
The new real fixture uses the shared process group guard and a wait derived from the existing slow deadline.
It adds no sleep or polling loop and does not reap production children.
Its assertions check the mutation command's pass and failure behavior.

The reviewer accepts the stated prebuilt-worker limit under revision 23n.
A test that executes `target/candidate` does not see a worker mutant because the gate builds that candidate once.
A test binary that contains the changed code can see the mutant. A miss in both stages fails the gate.
The PR states this limit in `.cargo/mutants.toml`; it does not claim that stage 2 rebuilds the candidate.

The reviewer read the exact-head gate log:
`~/botster-sessions/gates/botster-core-stage1-p6-two-stage-mutants-8cce5880-pool-20261010-103617-70695.log`.
All ten jobs pass, exit 0. Default: 1482 passing tests. Slow: 382 passing tests. Conformance: 193 passing.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Stage 1 reports 25 mutants: 16 caught, zero missed, zero timeout, and nine unviable, in 97.7 seconds.
The repeated mutation command reports the same counts in 100.3 seconds. Neither command runs stage 2.
The new real fixture passes in 7.635 seconds. It exercises both stages and the workspace baseline.

The reviewer read the separate whole-file exclusion-removal log:
`~/botster-sessions/gates/botster-core-stage1-p6-two-stage-mutants-a9e0783d-pool-20261010-100726-13058.log`.
The unmutated slow baseline passes 379 tests in 67.755 seconds.
The whole-file run reports 52 mutants in about 27 minutes: 31 caught, 21 unviable, zero missed, and zero timeout.
The reviewer checked all 52 outcome rows against the retained raw failure markers.
Each caught mutant builds successfully and fails a named test with exit 100. Each unviable mutant fails compilation with exit 101.
The `real.rs` blob is identical at the evidence head `a9e0783db11df47e0d9c805ee9556f68128bda93` and the reviewed head.
This evidence supports removal of the interim whole-file exclusion.
Caught Test phase durations range from 1.603 to 100.823 seconds. The reviewer asked P6 to correct the body's 2–76 second range.
The whole-file job is separate evidence, not a full gate.

The reviewer read the existing red-on-revert log:
`~/botster-sessions/gates/botster-core-stage1-p6-23n-revert-7b7690de-pool-20261010-095551-91182.log`.
Probe head: `7b7690de164f411352b01108d7d6690ab8a85f7c`.
Git objects show that this probe restores the per-package stage-2 arguments and retains the new fixture proof.
The fixture fails because stage 2 reports zero mutants when it must report four. Nextest exits 100.
The probe shell reports that result and exits 0. This is an expected failing proof, not a failed full gate.
The retained fixture also removes its slow assertion in a second case and checks that misses in both stages fail.

The lead permits review on the recorded base before the implementer merges the newer `v1` and runs the required gate.
This verdict does not certify that later merge head.
The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.

VERDICT: NOT CLEAN
