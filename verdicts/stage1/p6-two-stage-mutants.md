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

## Successor verification of Round 1 — 2026-10-10

Reviewer: Sol, successor to Astra under rule 23e.
Reviewed head: `8cce58801c3e5ca63b7550d5b81aca2cc9c5325e`.
Reviewed delta: `150a2069428173c97892160b73e94993233f413a...8cce58801c3e5ca63b7550d5b81aca2cc9c5325e`.
This entry verifies the inherited round. It does not start another review round on the unchanged head.

I read all eight changed files and traced the stage runner, decision, argument selection, fixture, and artifact retention.
I compared the new baseline with `test_budget::run_bounded` and `bounded_report` at the reviewed head.
I also read the nextest profile and the metadata selection of slow packages.

All three package-origin findings remain OPEN:

- R1-1 / MS-R1-1, HIGH: the baseline calls bare `cmd.status()` under a profile without a test termination deadline.
  The baseline does not call the existing bounded runner or apply its cleanup verdict.
  The fixture uses `OwnedChild`, so the fixture does not exercise this missing control in `mutants_job`.
  The correction must cover the actual baseline execution path.
- R1-2 / MS-R1-2, MEDIUM: `two_stage_decision` returns an error before its caller prints the accumulated reports.
  Baseline and stage-2 failures therefore lose earlier completed-stage counts and times.
  The correction must preserve those reports on failure, including a failed start.
- R1-3 / MS-R1-3, LOW: the argument test checks `--timeout 600` and separately asserts the private deadline constant.
  Retain the observable argument check and remove the redundant private-state assertion.

The package reviewer confirmed that MS-R1-1, MS-R1-2, and MS-R1-3 remain open on this exact head.
The package reviewer had not supplied the verdict commit when I wrote this entry.
I found no additional integration finding in the reviewed delta.

I read the exact-head gate log and verified its head, base, ten passing steps, and fixture result.
The two outer mutation runs report 25 mutants each: 16 caught, nine unviable, zero missed, and zero timeouts.
Neither outer run executes stage 2.
The fixture result supplies the two-stage behavior proof, but does not close R1-1 or R1-2.

I read the separate `a9e0783d` whole-file evidence log and parsed its retained outcomes JSON.
It reports 52 mutants: 31 caught, 21 unviable, zero missed, and zero timeouts.
Its slow baseline passes 379 tests in 67.755 seconds.
I read the reversal log. The fixture fails with nextest exit 100; the probe shell then exits zero.
The probe shell's exit zero is not a passing gate for that reversal.

The removed exclusion covers only `real.rs`.
The revised shell exclusion cites the new decisions and their proofs.
The artifact script includes the stage-2 output directory.
The stated `target/candidate` limitation remains within the option permitted by plan 23n.
This review grants no merge approval and preserves the required later base check and exact-head gate.

I changed no product code.
I ran no builds, tests, gates, mutation jobs, or reversal jobs.

VERDICT: NOT CLEAN (3 open)

## Round 2 — 2026-10-10

Reviewer: Sol.
Reviewed head: `3890e23093b77c3620237e7c9349c9ca0bf2c1a1`.
PR and gate base: `c19ad938bd87c60331bd002c0117102d8756b1ae`.
Tier: HIGH. Authority: approved plan 23y; its mutation rules retain revisions 23n and 23r.
Scope: all three correction paths, affected execution paths, the nine-file PR delta, supplied evidence, and the base merge.
The unchanged source and evidence checks from Round 1 remain applicable.

### R1-1 / MS-R1-1 — HIGH — CLOSED

`mutants_job` now sends its unmutated stage-2 baseline through `test_budget::bounded_success`.
The caller supplies `SLOW_DEADLINE`, a separate baseline pid file, the slow seeds, and the same slow selection as stage 2.
The new helper prepares the pid file and sets `BOTSTER_TEST_PIDFILE` on the command.
It delegates to the existing `bounded_report` and `run_bounded` path.
That path starts a process group, tracks descendants, kills the group at the deadline, and checks for leftover processes.
It performs cleanup before any later report error can return.
It also fails when the output pipe remains open past the deadline.
The helper returns the command's success only after these checks pass.
The existing capped cargo command preserves the gate's resource limits.

The real-process helper proof checks success, command failure, a blocked command, and a leftover child.
The decision proof runs blocked and leftover baselines through the same helper.
Both cases fail before the supplied stage-2 closure can start.
The fixture now runs its baseline through this helper too.
The exact-head gate selects and passes all three proofs.
No new time value or copied process guard is introduced.

### R1-2 / MS-R1-2 — MEDIUM — CLOSED

`two_stage_decision` now gives each completed-stage line to an output callback immediately.
`mutants_job` prints each callback line immediately.
A later error therefore preserves the earlier completed-stage counts and times.
The new proof checks a stage-2 miss, a stage-1 timeout, a failed stage-2 start, and a failed baseline start.
The existing baseline-failure case now checks its retained lines too.
The gate runs the new proof in 0.006 seconds.
The closure failure paths retain their causes and do not start the next stage.

### R1-3 / MS-R1-3 — LOW — CLOSED

The argument proof no longer asserts the private deadline constant separately.
Its complete command comparison still checks the observable `--timeout 600` argument.
It also retains exact escaped mutant-name filters and the baseline selection check.

### Source and evidence limits

The correction changes only `.cargo/mutants.toml`, `xtask/src/ci.rs`, and `xtask/src/test_budget.rs` before the base merge.
The revised exclusion comment cites the bounded-run and report proofs.
It adds no exclusion for the new helper; stage 2 tests that helper.
The removal of the interim `real.rs` exclusion remains valid.
The `real.rs` bytes equal the source in the previously reviewed whole-file evidence run at `a9e0783d`.
That run's 52 mutants remain supported by the Round 1 evidence review: 31 caught, 21 unviable, zero misses or timeouts.
The slow-only fixture and its reversal remain part of the gate.

The stated `target/candidate` limitation remains within the option permitted by plan 23n.
A test that executes the prebuilt worker does not see a mutation in that worker binary.
A worker mutant that neither stage catches fails the step; this review does not claim per-mutant candidate rebuilds.
I found no additional source finding in the replacement.

### Exact-head gate

Log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-two-stage-mutants-3890e230-pool-20261010-133139-77510.log`.

The header identifies the reviewed head and base. All ten stages pass.
Default: 1,485 passed, 497 skipped. Slow: 384 passed, 1,936 skipped.
Testkit conformance: 193 passed, zero failed.
Pending-real: 87 ids, zero passes.
Minimum counts remain testkit 50/69, real-passing 29/68, and real-accepted 29/69.
The two-stage fixture passes in 7.282 seconds in the regular slow tier.
The blocked-baseline proof passes in 0.011 seconds. The bounded-helper proof passes in 0.022 seconds.

The first mutation command exercises both stages:

- Stage 1: 25 mutants; 14 caught, two missed, zero timeouts, nine unviable; 146.9 seconds.
- Unmutated stage-2 baseline: passed; 121.4 seconds.
- Stage 2: two mutants; two caught, zero missed, zero timeouts, zero unviable; 169.3 seconds.
- Combined verdict: 25 mutants; 16 caught, nine unviable, zero missed or timeouts.

The repeated mutation command reports the same counts.
Its times are stage 1 94.8 seconds, baseline 81.8 seconds, and stage 2 148.5 seconds.
Both baselines pass all 384 selected tests.
The job exits zero after 1,006 seconds; the gate exits zero after 1,007 seconds on msa1.
These results now exercise the outer stage-2 path, unlike the Round 1 gate.

### Base merge and package record

Merge-check log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-two-stage-mutants-3890e230-pool-20261010-134832-14635.log`.

The supplied check compares `9069dcf376b5fef28481d5fe0a600e0a083efd61` with this head and reports PASS for every condition.
It reports disjoint base paths, an unchanged PR diff, and the base's unchanged pending membership.
I independently compared both PR diffs with `git diff --binary`; they are byte-identical.
I independently compared Git's automatic merge tree with the head's tree; both are `861cb847bedbec2c1b949d15de25028cab04b753`.
The current base is the single merge base and is an ancestor of the head.
The remote PR head and base match. `git diff --check` passes.

I read Round 1 package verdict `c4a2ebab5b2dff66c16cb4b2852f5b57ae8069a5`, `verdicts/p6-two-stage-mutants.md`.
Its three findings match the closed findings above.
The package reviewer reports that the replacement source and evidence review closes all three findings, with no additional finding.
The replacement package verdict commit is pending.
I changed no product code and ran no builds, tests, gates, mutation tests, or reversal jobs.

VERDICT: CLEAN (0 open)

### Package verdict receipt — 2026-10-10

I read replacement package verdict `a685c84e5af3f7d7a1a4440018b6bdbdd79303d7`, `verdicts/p6-two-stage-mutants.md` on `stage1/review-p6`.
It reports CLEAN on the same head and closes MS-R1-1, MS-R1-2, and MS-R1-3.
This receipt changes no review scope or conclusion. Round 2 remains CLEAN.
