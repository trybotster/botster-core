# PR #225 — Two-stage mutation verdict

## Round 1 — 2026-10-10

Reviewed head: `8cce58801c3e5ca63b7550d5b81aca2cc9c5325e`.
PR and gate base: `150a2069428173c97892160b73e94993233f413a`.
Tier: HIGH. The change affects gate decisions, mutation exclusions, and the remote gate script.
Scope: all eight changed files, their execution paths, plan revisions 23n/23r, the gate, and retained mutation evidence.

The package reviewer identified the three findings below after my initial assessment reported no source finding.
I then confirmed each finding in source. These are package-origin findings, not independent discoveries.
No integration CLEAN artifact was published.

### R1-1 — HIGH — The stage-2 baseline bypasses bounded execution and process cleanup

Location: `xtask/src/ci.rs:466`, particularly the bare `cmd.status()` at line 471.
Package finding: MS-R1-1.

The new baseline runs the slow tests under the `mutants` profile, which has no test termination deadline.
The call supplies no outer deadline, process tracker, or survivor check.
A baseline test that hangs can block the gate indefinitely.
A baseline that exits successfully but leaves a process can pass without the existing cleanup verdict.

The ordinary slow tier uses `test_budget::run_bounded`, a process group, PID tracking, and cleanup checks.
PR #220 also put the separate pending-real run under those controls through `bounded_report`.
The new baseline bypasses those controls. Its later mutation command's `--timeout 600` does not bound the baseline.
Reuse the bounded execution path with the existing slow deadline.
Retain failure and cleanup for a timeout, a surviving process, and an open output pipe.
Status: OPEN.

### R1-2 — MEDIUM — A later failure loses completed-stage reports

Location: `xtask/src/ci.rs:495` and `two_stage_decision` at lines 663–698.
Package finding: MS-R1-2.

The decision accumulates stage reports in a vector. The caller prints that vector only after the decision succeeds.
A baseline failure returns its own error and loses the completed stage-1 report.
A stage-2 failure loses the completed stage-1 and baseline reports.
These failures therefore omit the per-stage times required by plan revision 23r.
Preserve each completed report when a later operation fails.
Add a proof that checks the retained reports on a failure path.
Status: OPEN.

### R1-3 — LOW — The argument proof also asserts a private constant

Location: `stage_2_tests_exactly_the_missed_mutants_with_the_slow_tier` in `xtask/src/ci.rs`.
Package finding: MS-R1-3.

The test already checks the public command arguments, including `--timeout 600`.
It also asserts `test_budget::SLOW_DEADLINE.as_secs() == 600` directly.
The second assertion tests private implementation state and adds no observable behavior check.
Remove that assertion. Retain the command argument proof.
Status: OPEN.

### Remaining source review

Stage 1 retains its diff selection, exclusions, default seeds, timeout multiplier, and non-terminating mutation profile.
Stage 2 selects each missed name with an escaped, anchored filter and uses the same diff and exclusions.
It builds the workspace with the slow features and uses the slow test filter and seed set.
Its mutation command runs in place, uses the existing slow deadline, and stops tests at the first failure.
The decision rejects incomplete outcome counts, failed baselines, misses in both stages, and timeouts.
The remote script retains the stage-2 mutation artifacts on failure.

The configuration removes only the interim whole-file `real.rs` exclusion.
The configuration states the prebuilt-worker limitation, as plan 23n permits.
Tests that execute `target/candidate` do not see a worker-code mutant in that binary.
An uncaught viable mutant still fails the two-stage verdict.

### Supplied evidence

Gate: `botster-core-stage1-p6-two-stage-mutants-8cce5880-pool-20261010-103617-70695.log`.
All ten stages pass in 270.2 seconds. Default: 1,482 passed. Slow: 382 passed.
The real fixture proof passes in 7.635 seconds; it checks a slow-only catch and a miss in both stages.
Both outer mutation runs report 25 mutants: 16 caught, nine unviable, zero missed, and zero timeouts.
Stage 1 takes 97.7 seconds, then 100.3 seconds in the repeated command. Neither outer run needs stage 2.
The job takes 803 seconds, including 423 seconds queued and 380 seconds running. The wrapper takes 804 seconds.
The passing gate does not close the three findings.

The separate `a9e0783d` evidence log runs the slow baseline and then the whole-file mutation command for `real.rs`.
The baseline passes 379 tests in 67.755 seconds.
The outcomes JSON confirms 52 mutants: 31 caught, 21 unviable, zero missed, and zero timeouts.
The retained failure lines identify test failures for the caught mutants and compilation errors for the unviable mutants.
The command uses the workspace, all six slow features, the slow filter, and immediate fail-fast behavior.
That job takes 1,705 seconds, including one second queued. The tested source differs from this head only by the reviewed #224 guard merge.

The `7b7690de` reversal restores the earlier per-package stage-2 arguments.
Its fixture fails because stage 2 reports zero mutants instead of four. Nextest exits 100; the probe shell prints that status and exits zero.
I verified the reversal diff and read its retained log.

The base is an ancestor of the reviewed head. The merge has no combined diff, and `git diff --check` passes.
Current `v1` has advanced to `7caf3457a04bd37f5d4e844db1900525eea7c8ec`.
The lead's reported instruction requires the later base merge check and a new gate after CLEAN; this verdict provides no merge approval.
The package artifact is pending. I reported the missed findings to the lead under plan revision 23e.
I changed no product code and ran no builds, tests, gates, mutations, or reversal jobs.

VERDICT: NOT CLEAN (1 HIGH, 1 MEDIUM, 1 LOW open).
