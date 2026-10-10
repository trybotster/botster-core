# P6 RealCoreHarness package review

## PR #220 — Round 1 — 2026-10-10

Head: `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
Tree: `14579ebda35d1814c3e621ac23bc790c27f120e9`.
Parent: `dfdcd05f12bc49d4cc321b5198aa390c4779eeb9`.
PR and gate base: `159cc4003c8910ba6ef28402c360ba4d2dd820a4`.
The head contains the base. `git diff --check` finds no whitespace error.

Risk tier: HIGH, under BUILD.md rules 1, 3, and 5.
This PR changes gate decisions, shared crates, and mutation exclusions.
The reviewer read all twenty changed files, the PR body, the current plan, and the supplied evidence.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### Open findings

**RH-F1 — MEDIUM — OPEN: a worker name selects the probe binary.**

At `crates/botster-core-testkit/src/real.rs:165-178`, `wrapper` selects the binary from `file_name == PROBE`.
`worker_named("botster-conformance-probe")` offers that name as a current worker.
`open` passes the name through `worker_wrapper`, so the wrapper executes the probe instead of the worker.
Worker and probe wrappers also share the path `<root>/w/<grace>/<file_name>`.
The tagged `CoreHarness::worker_named` contract permits an arbitrary worker file name, under Core E1-1.
Required change: select the binary by an explicit role, independent of the file name.
Keep worker and probe wrapper paths distinct when their file names match.
Add a focused proof for this permitted worker name. P6 accepted this finding.

**RH-F2 — HIGH — OPEN: pending-real execution bypasses gate protections.**

At `xtask/src/ci.rs:202-225`, `slow_job` starts another real-process run after `test_budget::command` returns.
The new `Command::output` call has no slow-tier deadline, process-group cleanup, PID tracking, or leftover failure check.
The existing path provides those protections in `test_budget::run_nextest` and `clean_up`.
`Limits::real()` bounds transcript wait steps. It does not bound arbitrary blocking code or detect a leaked process.
`real_report_ran` checks only the exit status and two report prefixes.
Required change: execute pending-real through the existing bounded execution and cleanup path.
Retain non-failing outcomes for pending transcripts.
Prove that a hung execution and a leftover process fail the gate.
Use the existing deadline value. This is the same finding as Astra R1-1.

**RH-F3 — MEDIUM — OPEN: the descriptor proof can block and does not verify reuse.**

At `crates/botster-core-testkit/tests/slow_edge_tap.rs:41`, `worker.read` blocks if the tested close fails.
At line 52, `new_near.read` blocks if a stale read consumed its byte.
Neither read has a deadline or non-blocking mode.
The new socket pair only “may” reuse the old descriptor number; the test does not verify that precondition.
Plan 23l requires the closed-link proof to cover a reused descriptor number.
Required change: make these reads non-blocking or bounded.
Make the descriptor reuse precondition explicit and verify it.
This finding does not require a product injection seam. It is the same finding as Astra R1-2.

**RH-F4 — LOW — OPEN: the edge-tap descriptions overstate each pump's read.**

`edge_tap.rs:7-9` and the new DESIGN.md section say each pump drains links until `WouldBlock`.
They also say that every held item reaches the engine on the next pump.
The production reader can stop for its budget or held input, as the accepted plan states.
Required change: correct the descriptions. Held items remain non-quiet until the driver consumes them.
The implemented `Tap::quiet` behavior and the tagged driver's `!report.more` condition are correct.
This is the same finding as Astra R1-3.

**RH-F5 — HIGH — OPEN: final slow-mutation evidence lacks inspectable failure details.**

The PR adds an interim exclusion for every mutant in `real.rs`.
The lead permits that exclusion only with separate slow-feature evidence, pending plan 23n's gate change.
The full run at `619b92d2` has nine misses.
The focused run at the reviewed head reports fourteen catches and two unviable outcomes, including the nine repaired survivors.
That passing job retained no outcomes or per-mutant logs.
The reviewer cannot verify each reported final catch's behavioral failure from the aggregate lines.
The initial mutation baseline also omitted Core conformance, although each mutant's test run included it.
Required change: supply detailed evidence for the nine repaired survivors and the wrapper functions changed by RH-F1.
Identify the intended baseline tests and their passing outcomes.
Save `outcomes.json` and each mutant's log. A focused replacement job is sufficient; no full rerun is requested.
The reviewer has accepted the recovered initial outcomes described below.

### Source and list checks

The tapped harness composes `HostDriver<EdgeTap<RealEdges>>` through `botster_core::open_parts`.
The plain harness calls production `Core::open`.
Every `HostEdges` method forwards its inputs and results, including descriptor sends and scheduler choices.
Take-ahead holds at most one accept, one exit, and one read chunk per link when nothing is held.
Held bytes, ends, accepts, and exits reach the driver in order and keep the tap non-quiet.
`connect_worker` runs only when the driver calls it; the tap does not use it as a probe.
The wrapper decodes registry rows with production `Row::decode` and retains them across handles.
The harness keeps weak tap references, so the production data-directory lock ends with its handle.
Guard wrappers use the shared `botster-test-process` anchor, with a guard for each configured stop grace.
Candidate verification now checks the anchor bytes in addition to worker and probe bytes.
The clock is injected, but progress is not injected, under R-46.
The runner uses `Limits::real()` and retains the contract-derived A20 testkit-only exception.
Active real trials must pass on the tapped composition and the plain composition, unless the plain harness lacks a required control.
Pending, deferred, withdrawn, unselected, and pending-real trials never count as actual passes.
The `--ignored` minimum report explicitly assumes the preceding nextest trials passed in the same gate job.
RH-F2 concerns the execution protections around that report run.

The Core ledger, pending list, deferred list, and copied contracts status files equal the base bytes.
The canonical minimum file equals the approved plan file at `bae71c81` byte for byte.
Its SHA256 is `b279f6250a6ef4394a6a7551367b0cd25862aa7dc1006f9fe8bf883def690abb`; it contains 69 IDs.
The new list checks reject duplicate, empty, unknown, or withdrawn minimum IDs.
They reject pending, deferred, withdrawn, slow-only, and A20 testkit-only IDs in the real-pending list.
After initialization, a real-pending addition must be a new ledger ID or leave Core's pending list in the same PR.

The reviewer parsed the initialization logs by ID.
`b01225dd` has 99 real passes and 83 failures.
`dfdcd05f` reports seven adoption IDs that now pass and must leave the real-pending list.
`97a8015a` fails exactly eleven newly active #217 IDs, which enter the list.
The final list equals those 83 failures, minus seven passes, plus eleven new failures: 87 IDs.
Each current entry has a raw unsupported-control failure in the initialization evidence.
The group counts are 21 route, 32 program, 22 adoption/worker, four wake, three oracle, two handoff, two writes, and one route-fill.
The body assigns each group an owner and cause.
All 193 active testkit IDs have exact-head PASS lines.
All 106 active real IDs outside the real-pending list have PASS lines, with no extra real pass.
Minimum counts are testkit 50 / 69, real-passing 29 / 68, and real-accepted 29 / 69.
The A20 member remains Core-pending, so it adds no accepted pass.

### Supplied gate and mutation evidence

Exact-head gate:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-6c1624e9-pool-20261010-000244-43539.log`.
All ten stages PASS; the job and gate exit zero on msa1 after 650 seconds.
Default: 1455 passed, 497 skipped. Slow: 373 passed, 1921 skipped.
Conformance: 193 passed, zero failed; 407 pending with transcripts and 70 without transcripts.
Two IDs remain deferred and 18 withdrawn.
The pending-real report lists 87 IDs, with zero newly passing IDs.
Both standard mutation steps report 119 tested, 89 caught, 30 unviable, zero missed, and zero timeout.
The extra outer `NEXTEST_PROFILE=slow` command does not establish slow-feature mutation coverage.

Initial slow-feature run:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-619b92d2-pool-20261009-213134-49005.log`.
Recovered artifacts:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-619b92d2-mutants.out/mutants.out/`.
The reviewer parsed all 53 mutant outcomes and read the individual failure details.
Results: 23 caught, nine missed, 21 unviable, zero mutation timeout.
Twenty-two catches have Core conformance failure, unsupported-control, or inconclusive results.
The guard's deleted negation instead causes a missing-entry panic in the mutated guard decision.
All 21 unviable outcomes are compile failures: missing trait implementations, invalid constructors, private methods, or a trait-call error.
The nine misses have successful build and test phases.
The baseline runs ten testkit slow tests and omits Core conformance; each mutant test run includes both packages.

Focused slow-feature run:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-6c1624e9-pool-20261010-001413-72738.log`.
It reports sixteen tested, fourteen caught, two unviable, zero missed, and zero timeout.
Its job and gate exit zero on msa1 after 5976 seconds.
The reviewer read the new survivor tests, but RH-F5 requires the missing failure details and replacement wrapper evidence.

All five findings remain open. P6 and Astra received them directly.
Astra independently records the same three execution, proof, and documentation findings, plus RH-F1.
Integration Round 1 verdict: `37e9a9f1ae0a3def9c95c745853f6467689b4984`, `verdicts/stage1/p6-real-core.md`.
The reviewer sends no NOT CLEAN report to the lead and awaits a replacement READY.

VERDICT: NOT CLEAN
