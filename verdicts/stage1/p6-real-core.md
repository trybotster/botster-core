# PR #220 — RealCoreHarness integration review

## Round 1 — 2026-10-10

Reviewed head: `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
PR and gate base: `159cc4003c8910ba6ef28402c360ba4d2dd820a4`.
Tier: HIGH. The PR changes shared test infrastructure, gate decisions, and the canonical acceptance list.
Scope: the complete twenty-file PR diff, affected production interfaces, test selection, list initialization, and supplied evidence.

### R1-1 — HIGH — Pending-real execution bypasses the slow execution protections

`xtask/src/ci.rs:slow_job` first calls the bounded `test_budget::command`.
After that call returns, it starts `cargo test --test slow_conformance -- --ignored` through `Command::output`.
This second real-process run is outside the whole slow-tier deadline and the process tracking and cleanup in `run_nextest`.
It also bypasses the leftover-process failure check that `test_budget::command` applies after execution.
`real_report_ran` checks only the exit status and two report prefixes.
A successful exit with leftover processes can therefore pass this new path.
A blocked call can wait indefinitely in `output`.

`Limits::real()` bounds transcript wait steps. It does not bound arbitrary blocking code or establish that no process remains.
The harness's guards provide ownership, but they do not replace the gate's independent failure detection.
Reuse the bounded execution and cleanup path for pending-real trials.
Keep their ordinary transcript failures non-failing, as the plan requires.
Prove that a hung execution and leftover processes fail the gate.
Use the established deadline rather than a new timeout value.

Both package reviewers independently confirmed this finding.
P5 records it as RH-F2. P6 records it as RH-R1-2.
Status: OPEN.

### R1-2 — MEDIUM — The closed-link proof can hang and does not verify descriptor reuse

`crates/botster-core-testkit/tests/slow_edge_tap.rs:39` performs a blocking read for worker EOF.
If the tested close fails, the stream remains open and the test waits instead of reaching an assertion.
At line 52, a blocking read checks that the new socket retained its byte.
If a stale descriptor read consumed that byte, this assertion also waits indefinitely.

The test says that the new socket pair may reuse the old descriptor number.
It neither forces that condition nor checks that reuse occurred.
Plan 23l requires a test of the closed-link behavior across descriptor reuse.
Make the failure checks nonblocking or bounded.
Make the reuse precondition explicit and verify it before claiming that proof.
Preserve the EOF, closed-link read/write, and unaffected new socket assertions.

Both package reviewers independently confirmed the blocking reads.
P5 records the finding as RH-F3. P6 records the blocking-read part as RH-R1-1.
Status: OPEN.

### R1-3 — LOW — New documentation overstates what one pump drains

The module documentation in `edge_tap.rs` and the new section of `DESIGN.md` say that every pump drains each link until WouldBlock.
They conclude that a held item reaches the engine at the next pump.
The accepted plan explicitly states that `read_link` can stop for its budget or retained input.
A held item remains non-quiet until the driver consumes it, which can require later pumps.
Correct these two descriptions to match that bounded reader.

This is a documentation finding. The tap's retained-item check and the conformance driver's `!report.more` check cover their respective conditions.
Status: OPEN.

### P5 RH-F1 — MEDIUM — A worker file name can select the probe binary

The P5 reviewer reported this finding. I confirmed the mechanism in `real.rs`.
`worker_named` permits a current worker with an arbitrary file name.
`worker_wrapper` passes that name to `wrapper`, which chooses the binary by comparing the name with `PROBE`.
Thus `worker_named("botster-conformance-probe")` selects `candidate.probe` when the harness opens Core.
The worker and probe wrappers also share `w/<grace>/<file_name>`, so the roles collide at that path.
Select the executable by its explicit role.
Keep the worker and probe wrapper paths distinct for a colliding name.
Prove that the named worker still runs the worker binary.

Status: OPEN.

### Checked behavior and accounting

The harness uses `open_parts` and `HostDriver::open`, which are the same composition that `Core::open` uses.
The wrapper retains inbound bytes and reports, and forwards them in order.
The harness keeps weak tap references, so it does not retain the data-directory lock after the Core handle ends.
The production `RealEdges::link_close` removes the stream by `LinkId`.
Later reads and writes reach its closed-link result, not a retained raw descriptor.
The probe and worker use the shared process guard through launch wrappers.
The harness injects the transcript clock and reports that progress is not fully injected, as R-46 requires.

The runner shares trial selection between testkit and real tiers.
Pending-real trials remain ignored outcomes and do not count as passing trials.
The real active trial also checks the plain Core composition, with the documented unsupported-control exception.
The canonical minimum list is byte-identical to the approved plan file at `bae71c81`.
Its SHA-256 is `b279f6250a6ef4394a6a7551367b0cd25862aa7dc1006f9fe8bf883def690abb`.
It contains 69 ids. Independent comparison gives 50 testkit-active minimum ids and 29 minimum ids with real PASS lines.
The 87 real-pending ids overlap neither core-pending nor the 106 real PASS ids.

I checked the initialization logs by id:

- `b01225dd`, log ending `20261009-204712-26685.log`: 99 distinct real PASS ids and 83 distinct real FAIL ids.
- `97a8015a`, log ending `20261009-234707-14893.log`: exactly the eleven newly active #217 ids fail.
- `dfdcd05f`, log ending `20261009-235116-22992.log`: the pending-real report names seven adoption ids that now pass.
- The final file is exactly the original 83 failures, plus those eleven ids, minus those seven adoption ids.

### Exact-head gate

Log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-real-core-6c1624e9-pool-20261010-000244-43539.log`.

The header names the reviewed head and base. The base is an ancestor of the head.
The remote head and base matched when checked. `git diff --check` passes.
All ten steps pass in 393.9 seconds.
Default tests: 1,455 passed. Slow tests: 373 passed.
There are 106 distinct real conformance PASS lines, including 29 minimum ids.
The pending-real report says 87 ids and zero passes.
The report gives testkit-passing 50/69, real-passing 29/68, and real-accepted 29/69.
Both mutation steps report 119 mutants: 89 caught, 30 unviable, zero missed, and zero timeouts.
The second step still sets only `NEXTEST_PROFILE=slow`; it does not enable the slow feature.
The pool job exits zero after 650 seconds on msa1.
These green results do not exercise the failure paths in the findings above.

### Manual mutation evidence remains unverified

The interim exclusion of `real.rs` is explicitly permitted until the plan 23n gate change, subject to manual slow mutation evidence.
I read the commands, baseline output, and mutant summaries of the two supplied logs:

- `619b92d2`, log ending `20261009-213134-49005.log`: 53 mutants, 23 caught, nine missed, 21 unviable; exit 2.
- `6c1624e9`, log ending `20261010-001413-72738.log`: 16 focused mutants, 14 caught, two unviable; exit zero.

Both commands enable the slow features, use `--no-config`, and select the slow profile with immediate fail-fast.
The older baseline output names only the testkit package and ten tests, including one `real::slow_tests` test and `slow_edge_tap`.
The summaries show catches lasting hundreds of seconds, including mutants targeted by the new immediate unit assertions.
Those times alone do not establish a fault, but the summaries do not identify the failing assertions or commands.
I requested the existing per-mutant logs and outcomes before accepting these catches as behavioral evidence.
The implementer is recovering saved artifacts and agrees that the summaries alone are insufficient.
I requested no new execution.

The exact-head package verdict artifacts are still pending.
I sent the findings directly to the implementer and package reviewers.
I changed no product code and ran no builds, tests, gates, or mutation tests.

VERDICT: NOT CLEAN (4 open; manual mutation evidence also awaits verification).

### Recovered evidence addendum — same reviewed head

The implementer supplied the saved raw `619b92d2` artifacts after the initial verdict was published.
Artifact root:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-real-core-619b92d2-mutants.out/mutants.out/`.

I checked all 53 entries in `outcomes.json` against their individual logs.
All 23 caught mutants built successfully and ended with test exit 100.
Their commands include both Core and testkit, both slow features, the slow profile, and immediate fail-fast.
The logs identify real conformance failures, unsupported controls, step-limit failures, or the guard lookup panic caused by its mutant.
They do not show a nextest termination being reported as a catch.
All 21 unviable mutants fail compilation with type errors and exit 101.
All nine missed mutants have successful build and test phases.

The baseline differs from the mutant command: it runs only testkit, while each mutant runs Core and testkit.
Thus this saved baseline does not establish a green Core conformance run for the same command.
The focused `6c1624e9` run has no retained raw outcomes.
The implementer will provide evidence on the corrected head, including the nine prior survivors and changed wrapper functions.
The replacement evidence must show the intended unmutated tests and each claimed behavioral catch.
No reviewer execution occurred.
The four source findings remain OPEN, and the manual mutation evidence remains incomplete.

## Round 2 — 2026-10-10

Reviewed head: `9317693d8b1250a31a6a14e7c1125f18f3f7d0a5`.
PR and gate base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`.
Tier: HIGH. I reviewed the complete replacement diff, its merge, and the supplied evidence.

### R2-1 — HIGH — A new mutation exclusion leaves the file comparison decision unproved

The new whole-body exclusion for `ledger_ids_command` cites `ledger_text` and `real_only_text` as its tested decisions.
Those helpers produce expected text. They do not decide whether a missing or different checked-in file must fail the gate.
That comparison remains inside the excluded function at `xtask/src/lists.rs:718`.
Its condition compares `read_to_string(...).ok().as_deref()` with `Some(text.as_str())` and decides whether to return an error.
The cited tests do not exercise this decision.
Thus the new exclusion does not meet the gate's requirement for tested decisions outside an excluded I/O shell.

Reuse the tested copy comparison, or extract and test this comparison decision.
Cite the actual decision and its proof in the exclusion comment.
The proof must cover matching, different, and missing copies.

The P6 reviewer reported this as RH-R2-1. I independently checked the exclusion, function, and cited tests.
Status: OPEN.

### Prior findings closed

- R1-1: `pending_real` uses the shared bounded execution path with the established slow deadline.
  The path owns a process group and tracks processes. `bounded_report` cleans up before reading the captured report.
  It rejects leftover processes, deadline expiry, and output that remains open past the deadline.
  The supplied gate passes the status, deadline, and leftover-process proofs.
- R1-2: the socket proof uses nonblocking reads and verifies which descriptor closes.
  It retains socket pairs until a new socket reuses that descriptor number, then checks stale-link reads and writes.
- R1-3: the documentation now describes bounded reads and retained input that can require later pumps.
- P5 RH-F1: an explicit role selects the binary. Separate role directories prevent worker and probe wrapper collisions.
  The focused proof checks both paths and the binary selected by each wrapper.

The plan 23s count gap also closes.
I independently derived all 26 real-only rows from the pinned contracts replacement map and Core ledger. The bytes match.
The runner counts eligible minimum ids by their named proof on both tiers and prints each proof source.
All four current real-only minimum ids remain pending, so this path adds no current pass.
The minimum file is byte-identical to Round 1. The pending and real-pending id sets are unchanged.

### Manual mutation evidence verified

Log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-real-core-3ee0f118-pool-20261010-040958-20767.log`.

The job explicitly runs the intended unmutated selection first: both packages, both slow features, the slow profile, and immediate fail-fast.
All 175 selected tests pass.
The cargo-mutants baseline selects only seventeen testkit tests; the explicit baseline resolves that selection difference.
I parsed all eighteen mutant outcomes and read each retained failure section.
All sixteen catches build successfully and return test exit 100 with a named failure.
The two unviable mutants return compilation exit 101 because `DataDirRef` and `WorkerRef` lack `Default` implementations.
There are no missed mutants or timeouts. The job exits zero after 7,018 seconds.
The nine earlier survivors and changed wrapper functions are covered by this focused run.
The recovered 23 catches and 21 compilation failures from Round 1 remain valid evidence.
`real.rs` is byte-identical across the final merge.
The manual mutation evidence concern is CLOSED.

### Gate and merge checks

Log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-real-core-9317693d-pool-20261010-083223-20264.log`.

The header names the reviewed head and base. All ten steps pass in 453.4 seconds.
Default tests: 1,464 passed. Slow tests: 377 passed.
The conformance reports retain 193 testkit passes, 106 real passes, and 87 pending-real ids with zero passes.
Minimum counts remain testkit 50/69, real-passing 29/68, and real-accepted 29/69.
Both mutation steps report 125 mutants: 95 caught, 30 unviable, zero missed, and zero timeouts.
The second mutation step still selects only the slow profile; it does not enable the slow feature.
The repeated mutation step takes 281.9 seconds. The pool job exits zero after 747 seconds on msa1.

The base is an ancestor of the head. `git diff --check` passes.
After removal of index and hunk metadata, the complete pre-merge and post-merge PR diffs are identical.
The combined merge diff is empty.
The remote base remains `d174ef48`; the PR branch has advanced to `8e73278755d66ab02b09141ad57c4bd0f41272b7`.
This verdict applies only to the reviewed head. The newer head awaits replacement READY and evidence.

I read P5's exact-head package verdict at `c363d284414ae3e550b2eb646235f320f767502d:verdicts/p6-real-core.md`.
P5 reports CLEAN. That verdict does not identify R2-1; I sent the confirmed finding to P5.
P6 reports the same open finding and is preparing its verdict artifact.
I changed no product code and ran no builds, tests, gates, or mutation tests.

VERDICT: NOT CLEAN (1 HIGH open).

## Round 3 — 2026-10-10

Reviewed head: `8e73278755d66ab02b09141ad57c4bd0f41272b7`.
PR and gate base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`.
Tier: HIGH. I reviewed the complete two-file replacement diff and the updated PR description.

### R2-1 closed

`ledger_ids_command` reads every copy and propagates read errors, including a missing file, with repair instructions.
After successful reads, it pairs each copy with the corresponding path and expected text.
The existing `copy_problems` helper compares the pairs. The command returns an error when that helper reports problems.
The helper's existing proof checks equal text, equal empty text, and one mismatch among matching copies.
The proof verifies that the mismatch identifies the correct file.
The exclusion comment now cites this comparison helper and its proof.
The exclusion still covers only the command's whole-body replacement. The comparison helper remains eligible for mutation testing.
R2-1 is CLOSED. I found no new defect.

### Supplied evidence

Log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-real-core-8e732787-pool-20261010-084907-54482.log`.

The header names the reviewed head and base. All ten steps pass in 478.5 seconds.
Default tests: 1,464 passed, including `a_copy_that_is_not_the_pinned_file_is_a_problem`.
Slow tests: 377 passed. The list command confirms that the checked-in files match the pin.
Conformance: 193 testkit passes; the pending-real report retains 87 ids with zero passes.
Minimum counts remain testkit 50/69, real-passing 29/68, and real-accepted 29/69.
Both mutation steps report 125 mutants: 95 caught, 30 unviable, zero missed, and zero timeouts.
The second step takes 275.6 seconds. It selects the slow profile without enabling the slow feature.
The pool job exits zero after 761 seconds on msa1; the gate wrapper exits zero after 762 seconds.

The manual slow mutation evidence accepted in Round 2 remains applicable: `real.rs` is byte-identical to its tested revision.
The base is an ancestor of the head. `git diff --check` passes.
The remote head and base match the reviewed commits.
I read both Round 2 package artifacts, including P5's withdrawal of its earlier CLEAN verdict.
Both confirm that R2-1 was the sole remaining finding.
I changed no product code and ran no builds, tests, gates, or mutation tests.

### Package verdicts and conclusion

I read both exact-head package verdicts:

- P6: `a68d78a51451c541a3af5944a82d3de9653fe09d:verdicts/p6-real-core.md`, Round 3, CLEAN.
- P5: `4bb1b12cf2f675c2a5dc923b182a16461b834988:verdicts/p6-real-core.md`, Round 3, CLEAN.

Both confirm R2-1's closure and retain the earlier findings' closures and accepted mutation evidence.
All integration findings are closed. No new finding remains open.
This verdict retains the plan's interim manual evidence allowance. It does not replace the separate plan 23n gate change.
The 87 pending-real ids remain unfinished work; this PR does not establish Stage 1 completion.

VERDICT: CLEAN (0 open) at 8e73278755d66ab02b09141ad57c4bd0f41272b7.
