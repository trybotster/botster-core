# P6 review: RealCoreHarness

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/220.
Exact head: `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
Base: `159cc4003c8910ba6ef28402c360ba4d2dd820a4`.
Risk tier: HIGH. The PR changes the gate, shared harness, mutation exclusions, and canonical minimum list.

### Open package findings

- **RH-R1-1 MEDIUM — The socket proof has unbounded reads.**
  `crates/botster-core-testkit/tests/slow_edge_tap.rs` calls blocking `worker.read` and `new_near.read` without a deadline.
  The first call can hang if the close does not produce EOF.
  The second call can hang if a faulty old-link read consumes the new pair's byte.
  Use bounded reads with derived limits from the shared process crate.
  Keep the EOF, preserved-byte, `BrokenPipe`, and absent-peer-byte assertions.
  The reviewer sent this finding directly to P6 and Astra.
  It matches integration R1-2.

- **RH-R1-2 HIGH — The pending-real run bypasses execution protections.**
  `xtask/src/ci.rs::slow_job` uses bare `Command::output` after `test_budget::command` returns.
  The 87 pending-real transcripts therefore run outside the slow deadline, process-group cleanup, process tracking, and survivor check.
  `Limits::real` bounds transcript waits. It does not bound arbitrary blocking code or detect leaked processes.
  `real_report_ran` checks only the exit status and report prefix.
  Reuse the bounded execution and cleanup path with the existing slow deadline.
  Show that a hung run and a surviving process fail the step.
  Keep ordinary pending-real outcomes non-failing.
  The reviewer sent this finding directly to P6 and Astra.
  It matches integration R1-1 and requires the same fix.

- **RH-R1-3 MEDIUM — The minimum report omits named real-process proofs.**
  `tests/suite/mod.rs::minimum_report` counts suite trials only.
  After a `slow:*` minimum id leaves `core-pending.txt`, its named real-process proof must contribute to the minimum counts.
  The four such minimum ids remain pending at this head. The current totals are correct.
  P6 confirmed the future undercount and proposed a fix.
  The lead confirmed the rule in plan revision 23s, commit `8a9727e5d6a089fec14e29069b0cdfd561fb096c`.
  Count each non-pending `slow:*` minimum id in both testkit-passing and real-passing.
  Print each id and its named proof source.
  The reviewer sent the rule and finding directly to P6 and Astra.

### Other review records

The composition uses production `open_parts`, `HostDriver`, and `RealEdges` through the public `HostEdges` boundary.
The wrappers use the shared group guard and verified candidate binaries.
Production starts and reaps the subject processes. The new harness does not reap those processes itself.
The clock methods preserve the R-46 behavior: injected Core time, with real process progress.
The shared suite retains the `Outcome::Passed` assertion for active transcripts and the facade type assertions.

The Prior art note names the reused components and explains the new trait wrapper.
The reviewer verified that the canonical minimum file matches the plan file at `bae71c81` byte for byte.
It contains 69 ids. SHA256: `b279f6250a6ef4394a6a7551367b0cd25862aa7dc1006f9fe8bf883def690abb`.
The real-pending rules reject pending, deferred, withdrawn, real-only, and testkit-proven ids.
The rules permit initialization once and restrict later additions to newly active or new ledger ids.

Astra records two additional source findings: the socket proof does not establish descriptor reuse, and the drain descriptions are incorrect.
The descriptions in `edge_tap.rs` and `DESIGN.md` promise a full drain on every pump.
Plan 23l allows the production reader to stop for its budget or held input.
The pinned driver's `await_quiet` correctly checks both `!report.more` and the tap's quiet result.
Astra also records P5's worker/probe name collision finding.
The wrapper selects the probe by its filename, so a renamed worker with that filename selects the wrong binary.
These findings already went to P6 through the other reviewers.

### Evidence

The reviewer read the exact-head gate log:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-6c1624e9-pool-20261010-000244-43539.log`.
All ten jobs pass, exit 0. Default: 1455 passing tests. Slow: 373 passing tests.
Conformance: 193 passing. Minimum: testkit 50/69, real 29/68, real-accepted 29/69.
Both mutation commands report 119 mutants: 89 caught, zero missed, zero timeout, and 30 unviable.
The repeated command does not add slow-feature mutation coverage.

The reviewer read both manual mutation summary logs and the installed cargo-mutants command source.
The older `619b92d2` run reports 23 caught, nine missed, and 21 unviable mutants.
Its baseline reports only ten testkit tests. It does not establish the intended Core baseline.
The focused exact-head log reports 14 caught and two unviable mutants.
Its raw test failure logs were not supplied in this package review.
Astra reports that the focused raw artifacts are absent and requests replacement evidence.
Replacement evidence must establish the intended baseline and the behavior assertions that catch the mutants.
The interim `real.rs` exclusion remains subject to that evidence and the lead's plan 23n rule.

The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.
The reviewer sent ordinary findings to P6 and Astra. The lead received only the count-rule QUESTION.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/220.
Exact head: `9317693d8b1250a31a6a14e7c1125f18f3f7d0a5`.
Base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`.
The head merges `3ee0f1185511304777a1742f9b2ab665d7037819` with this base.

### Closed findings

- **RH-R1-1 closes.** The socket proof uses single non-blocking reads.
  It verifies that the break closes exactly one descriptor and that the new socket takes that descriptor number.
  It retains the EOF, preserved-byte, `BrokenPipe`, and absent-peer-byte assertions.
  The allocation loop retains each spare pair and must stop or fail before it passes the closed descriptor number.

- **RH-R1-2 closes.** `pending_real` calls `bounded_report` and `run_bounded` with the existing slow deadline.
  The run owns its group and uses the existing process tracker and cleanup function.
  Cleanup runs before the caller reads the captured report.
  The report read also has a deadline. `run_failures` rejects survivors, a run timeout, and output that stays open.
  The new slow tests exercise a successful run, a failed exit, a timeout, and a surviving process.
  The fixture processes block on the shared FIFO fixture. They use no sleep loop.

- **RH-R1-3 closes.** The checked-in real-only file exactly matches the pinned map's 26 Core `slow:*` entries.
  Both list commands check it against the pin.
  The report counts each non-pending real-only minimum id on both tiers and prints its proof source, as plan 23s requires.
  The four such minimum ids remain pending. The current counts remain unchanged.

The corrected drain descriptions match plan 23l.
The wrapper now selects the binary by `Role`, with separate role directories.
The new proof checks that a worker named like the probe still gets a wrapper for the worker binary.
The reviewer found no new package defect in those fixes.

### Open package finding

- **RH-R2-1 HIGH — The new ledger command exclusion still covers a gate decision.**
  `.cargo/mutants.toml` excludes the whole body of `xtask/src/lists.rs::ledger_ids_command`.
  Its reason cites `ledger_text` and `real_only_text` with formatting proofs.
  The excluded body still compares a checked-in copy with the pinned text and selects pass or failure.
  The inline decision is `read_to_string(...).ok().as_deref() != Some(text.as_str())`.
  The cited proofs do not test that comparison verdict.
  Reuse the tested `copy_problems` decision, or extract a tested pure comparison decision.
  Cite that decision and its behavior proof in the exclusion reason. Keep its mutants eligible.
  The reviewer sent this finding directly to P6 and Astra.
  P6 reports a fix at `8e732787`, with its gate still running. This round does not judge that later head.

### Evidence and scope

The reviewer read the exact-head gate log:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-9317693d-pool-20261010-083223-20264.log`.
All ten jobs pass, exit 0. Default: 1464 passing tests. Slow: 377 passing tests.
The three new bounded-run tests pass in the slow tier.
Conformance: 193 passing. The 87 pending-real trials run and report zero newly passing ids.
Minimum: testkit 50/69, real 29/68, real-accepted 29/69.
Both mutation commands report 125 mutants: 95 caught, zero missed, zero timeout, and 30 unviable.
The repeated mutation command does not add slow-feature coverage.

The replacement manual evidence closes the earlier evidence concern.
Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-core-3ee0f118-pool-20261010-040958-20767.log`.
The job first runs the intended baseline with both packages, slow features, the slow profile, and the slow filter.
All 175 baseline tests pass.
The retained outcomes show 18 mutants: 16 caught and two unviable, with zero misses or timeouts.
Each caught mutant builds successfully and ends with Test Failure(100).
The log contains the named test failure for each catch, including the nine earlier survivors and the wrapper functions.
The reviewer also read all 23 older caught logs and all 21 older unviable logs.
Those logs show test failures or compiler errors, respectively.
`real.rs` is identical before and after the merge into this reviewed head.
The interim exclusion still follows the lead's plan 23n rule. This evidence does not replace the required two-stage gate.

The canonical minimum, pending list, slow runner, and edge-tap unit proofs remain unchanged from round 1.
The merge adds the base's route-fill work. The harness merge retains this PR's shared limits parser and the base's route-fill changes.
The shared runner retains the active transcript assertions and facade type assertions.

The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.
No ordinary NOT CLEAN report went to the lead.

VERDICT: NOT CLEAN

## Round 3 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/220.
Exact head: `8e73278755d66ab02b09141ad57c4bd0f41272b7`.
Base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`.

RH-R2-1 closes. The only changes from round 2 are the ledger command and its exclusion reason.
`ledger_ids_command` reads each copy and passes the copy, source, and path to `copy_problems`.
A missing copy fails the read with context. A mismatched copy produces the tested comparison failure.
The existing proof checks both matching copies and a named mismatch.
The exclusion reason cites that decision and proof. No exclusion covers `copy_problems`.
The PR body records this final fix at the reviewed head.

All round-1 findings and the mutation evidence concern remain closed.
The other source files and retained manual evidence are unchanged from round 2.
The interim slow-module exclusion still follows the lead's plan 23n rule.
This CLEAN verdict does not replace the later required two-stage mutation gate.

The reviewer read the exact-head gate log:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-8e732787-pool-20261010-084907-54482.log`.
All ten jobs pass, exit 0. Default: 1464 passing tests. Slow: 377 passing tests. Conformance: 193 passing.
The ledger command reports that its copies match the pin.
Minimum counts remain testkit 50/69, real 29/68, real-accepted 29/69.
The 87 pending-real trials report zero newly passing ids.
Both mutation commands report 125 mutants: 95 caught, zero missed, zero timeout, and 30 unviable.
The repeated mutation command does not add slow-feature coverage.

All package findings close at this exact head.
The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.

VERDICT: CLEAN
