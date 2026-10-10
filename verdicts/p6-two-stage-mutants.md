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


## Round 2 — 2026-10-10

Exact head: `3890e23093b77c3620237e7c9349c9ca0bf2c1a1`.
Review and full gate base: `c19ad938bd87c60331bd002c0117102d8756b1ae`.
Previous reviewed head: `8cce58801c3e5ca63b7550d5b81aca2cc9c5325e`.
Risk tier remains HIGH.

No package finding remains. MS-R1-1, MS-R1-2, and MS-R1-3 close.
The reviewer read the complete replacement delta in `ci.rs`, `test_budget.rs`, and the mutation reason.
The reviewer also read the updated PR body, current plan pin, exact-head gate, and supplied merge check.
Plan revision 23y changes no requirement of this PR. It assigns the strict real-tier exception to a separate PR after #218.

**MS-R1-1 closes.** The stage-2 baseline now calls `test_budget::bounded_success` with `SLOW_DEADLINE` and a dedicated PID file.
That helper sets the PID-file environment and delegates to the existing `bounded_report` from #220.
The reused path creates the run's process group, tracks its descendants, bounds the wait, and checks output closure.
It performs cleanup before an error can return. A deadline failure or survivor fails the baseline and prevents stage 2.
The path waits only for the child that its own command starts. It does not reap production children.
The baseline retains its workspace, slow features, filter, seed environment, and profile without test termination.

The new real-process helper proof checks successful and failed exits, a passed deadline, and a survivor.
The gate-level proof passes a real blocked command and a command with a survivor through `two_stage_decision`.
Its stage-2 closure fails if called. Both cases return the required baseline failure before stage 2 starts.
The proof uses the shared `Blocker`, the existing slow deadline, and `Duration::ZERO` for an already-passed deadline.
It adds no sleep or polling loop. It checks public pass and failure behavior, not private state.
The existing two-stage fixture now uses the same bounded baseline path as the gate.

**MS-R1-2 closes.** `two_stage_decision` sends each completed stage report to `out` before it checks the next stage.
`mutants_job` prints each report through that callback. A later error no longer discards earlier reports.
The proof checks retained reports for a stage-2 miss, a stage-1 timeout, and failed starts of stage 2 and its baseline.
The failed-baseline proof also checks both the stage-1 report and the failed-baseline report.
These assertions observe the reports that the gate caller receives.

**MS-R1-3 closes.** The redundant private deadline assertion is removed.
The exact argument proof still requires public `--timeout 600` behavior and the unchanged workspace baseline arguments.

The mutation reason now names the bounded helper and its two real-process proofs, plus the report-retention proof.
The helper has no mutation exclusion. The PR adds no broad exclusion or dependency.
The Prior art record retains the existing decision, fixture, command, and cargo-mutants reuse.
The replacement explicitly reuses #220's bounded execution and cleanup path.

The accepted prebuilt-worker limit remains stated. The reviewer does not claim that stage 2 rebuilds `target/candidate`.
The whole-file `real.rs` evidence, fixture assertion, and artifact-retention script remain unchanged from round 1.
The `real.rs` blob is identical to the evidence head. Its exclusion removal remains supported.
The body now gives the measured caught Test phase range as 1.6–100.8 seconds.
The retained red-on-revert proof remains applicable. The fixture keeps its slow-only catch and both-stage miss assertions.

The reviewer read the exact-head Linux gate:
`~/botster-sessions/gates/botster-core-stage1-p6-two-stage-mutants-3890e230-pool-20261010-133139-77510.log`.
All ten jobs pass, exit 0. Default: 1485 passing tests. Slow: 384 passing tests. Conformance: 193 passing.
The new report-retention and both real-process failure proofs pass.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
The real report names 87 pending-real ids, with zero newly passing.

This gate runs stage 2 on the real workspace.
Stage 1 reports 25 mutants: 14 caught, two missed, zero timeout, and nine unviable, in 146.9 seconds.
The two misses replace `bounded_success` with `Ok(true)` and `Ok(false)`.
The bounded unmutated baseline passes in 121.4 seconds. Stage 2 catches both mutants in 169.3 seconds.
The final verdict reports 25 mutants: 16 caught, including two by stage 2, zero missed, zero timeout, and nine unviable.
The repeated command reports identical counts: stage 1 takes 94.8 seconds, baseline 81.8 seconds, and stage 2 148.5 seconds.

The reviewer read the separate merge-check log:
`~/botster-sessions/gates/botster-core-stage1-p6-two-stage-mutants-3890e230-pool-20261010-134832-14635.log`.
It checks `9069dcf376b5fef28481d5fe0a600e0a083efd61` to the exact head against the recorded base.
All four conditions and ancestry pass. The PR diff outside line-set files is byte-identical.
Its SHA256 is `031bc0512775dff5841560bd300a15f02dc526fa7d3528183ba0db94f0569dad`.
The current head incorporates the required newer base. This verdict certifies that exact head.

The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: CLEAN
