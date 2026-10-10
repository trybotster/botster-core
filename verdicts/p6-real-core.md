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
