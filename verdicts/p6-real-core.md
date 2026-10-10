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

## PR #220 — Round 2 — 2026-10-10

Head: `9317693d8b1250a31a6a14e7c1125f18f3f7d0a5`.
Tree: `e425a63ee5ae5574213f79112780bea4fb030658`.
Parents: `3ee0f1185511304777a1742f9b2ab665d7037819` and `d174ef48a218b74beb4bac9ec555c6c39d2650a4`.
PR and gate base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`.
The head contains the base. `git diff --check` finds no whitespace error.

Risk tier remains HIGH, under BUILD.md rules 1, 3, and 5.
The reviewer read the complete Round 2 changes, the corrected body, the merge changes, and the supplied evidence.
The reviewer read plan revision 23t at `pins/stage1-plan.23c132e0.md`.
Its verified SHA256 is `9f28e812b5969fc1e932ba690b3af99be5915dd1e96c9b09f6cebf943f084fd7`.
Revision 23s controls the new real-only progress counts. This head retains contracts-v0.1.24; the v0.1.25 move is a separate PR.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### Findings closed

**RH-F1 — MEDIUM — CLOSED.**

`wrapper` selects the candidate binary with the explicit `Role` enum.
Worker and probe wrappers use separate role directories, even when their file names and stop grace match.
The focused proof checks the distinct paths and each wrapper's selected binary.
The exact-head slow gate passes `a_worker_named_like_the_probe_still_runs_the_worker`.

**RH-F2 — HIGH — CLOSED.**

`slow_job` calls `test_budget::pending_real` with the existing `SLOW_DEADLINE`.
The pending-real command uses the extracted `run_bounded` path from `run_nextest`.
That path owns a process group, tracks descendants, kills the group at its deadline, and returns cleanup data.
`bounded_report` reports and kills leftovers before it reads the captured report.
The report read uses the remaining deadline. `run_failures` rejects leftovers, an expired deadline, and an open report pipe.
The command retains the existing seed environment and applies the cargo resource limits.
Pending transcript outcomes remain non-failing; the binary's status and required report still control the gate verdict.
The reviewer read the extracted process path and its cleanup order.
The exact-head gate passes the decision proof and all three real-process proofs for status, deadline, and leftover behavior.

**RH-F3 — MEDIUM — CLOSED.**

Every socket read in the broken-link proof is non-blocking.
The proof identifies the descriptor number that Core closes and checks that exactly one descriptor closes.
It retains spare socket pairs until a new socket takes that number.
The proof checks that a stale link neither reads nor writes the new socket.
The exact-head slow gate passes `a_broken_link_never_reaches_a_descriptor_that_reuses_its_number`.

**RH-F4 — LOW — CLOSED.**

The module documentation and DESIGN.md now state that a read can stop at its budget or held input.
They state that an item can wait for more than one pump.
They correctly require both tap quiet and a clear `report.more` before the runner stops pumping.

**RH-F5 — HIGH — CLOSED.**

The final focused run uses fixed parent `3ee0f1185511304777a1742f9b2ab665d7037819`.
`real.rs` is byte-identical in the final merge.
The job first runs the intended baseline with both packages, slow features, the slow profile, and the slow filter.
That baseline passes all 175 selected tests.
The retained log contains the complete `outcomes.json` and the failure sections for all eighteen mutants.
The reviewer parsed every outcome and read each failure section.
Results: sixteen caught, two unviable, zero missed, and zero timeout.
Each catch has a named test failure in a test that passes without the mutation.
The two unviable mutants require absent `Default` implementations for `DataDirRef` and `WorkerRef`.
The wrapper replacements cause the must-pass adoption trial to fail as inconclusive; they do not pass as pending outcomes.
The nine earlier survivors are caught by their focused proofs or the must-pass broken-control trial.
The cargo-mutants baseline still selects only testkit; each mutant test phase selects both packages.
The separate passing baseline resolves that selection difference.
The recovered initial 53 outcomes and their failure checks from Round 1 remain accepted.

### Additional changes and merge checks

The new `core-real-only.txt` contains exactly the 26 Core ledger rows whose pinned replacement-map proof starts with `slow:`.
The reviewer compared its bytes with contracts-v0.1.24's authoritative map.
`ledger-ids` writes or checks the file. `lists` also checks the file against the pin.
The pure `real_only_text` proof covers ordering, the ledger filter, proof selection, and missing map data.
The runner counts a non-pending real-only minimum ID through its named real-process proof on both tiers, under plan 23s.
It prints each counted proof source. All four current real-only minimum IDs remain pending, so this change adds no pass.
The A20 testkit-only member also remains pending and adds no accepted pass.

The reviewer read every new or changed mutation exclusion.
The five new gate shell exclusions cover whole-body replacements only and cite tested pure decisions.
The decision functions remain subject to mutation testing.
The shared bounded execution also has passing slow proofs that exercise its process behavior.
The existing whole-module `real.rs` exclusion remains the explicit interim allowance until the separate plan 23n gate PR.
The supplied manual slow-feature evidence supports that allowance; the repeated default mutation step is not slow-feature evidence.

The eight merge patches equal the corresponding v1 imports after removal of blob headers and hunk positions.
The merge introduces no additional package change or conflict resolution.
Core ledger, pending, deferred, and copied contracts status files equal the base bytes.
The canonical minimum file and the real-pending ID set remain unchanged from Round 1.
Comments correctly assign program controls to the fourth pair and state the accepted Linux-only write-failure form.

### Supplied evidence

Exact-head gate:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-9317693d-pool-20261010-083223-20264.log`.
All ten stages PASS. The job exits zero after 747 seconds; the gate exits zero after 748 seconds on msa1.
Default: 1464 passed, 497 skipped. Slow: 377 passed, 1929 skipped.
The reviewer compared exact PASS sets with Round 1: all 193 testkit IDs and 106 real IDs remain passing, with no set change.
The report retains 87 real-pending IDs, with zero newly passing IDs.
Minimum counts remain testkit 50 / 69, real-passing 29 / 68, and real-accepted 29 / 69.
Both default mutation steps report 125 tested, 95 caught, 30 unviable, zero missed, and zero timeout.

Focused slow-feature evidence:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-3ee0f118-pool-20261010-040958-20767.log`.
The intended baseline passes 175 tests. The mutation baseline passes seventeen testkit tests.
All eighteen mutant outcomes have retained command arguments and failure details in that log.
The job and gate exit zero after 7018 seconds on msa1.
Raw files also remain in the branch volume at `target/mutants-3ee0f118/mutants.out`.

All five package findings are closed. No new finding remains open, including LOW.
The reviewer reports package CLEAN to the lead, P6, and Astra with this verdict commit.

VERDICT: CLEAN

### Round 2 correction — 2026-10-10

This correction withdraws the preceding Round 2 CLEAN at the same exact head.
Head remains `9317693d8b1250a31a6a14e7c1125f18f3f7d0a5`; tree and base remain those stated above.
The reviewer missed a gate decision inside a newly excluded function.
P6's package reviewer reported RH-R2-1 HIGH. Astra independently confirmed it and informed this reviewer.
This reviewer then read the exact-head comparison and independently confirmed the finding.

**RH-F6 / RH-R2-1 — HIGH — OPEN: the new exclusion covers an untested comparison verdict.**

At `xtask/src/lists.rs:718-719`, `ledger_ids_command` rejects a missing or different checked-in copy.
The new whole-body mutation exclusion covers this comparison and its failure result.
The cited `ledger_text` and `real_only_text` tests prove generated text.
Neither cited decision proves rejection of a missing or different copy.
The exclusion therefore violates the plan's rule that gate decisions remain subject to mutation testing.
Required change: extract the comparison decision into a pure function and test matching, missing, and different copies.
Cite that decision and its proof in the shell exclusion. Keep the decision subject to mutation testing.

The five original package findings remain closed. The other Round 2 checks and evidence remain valid.
The earlier statement that every new exclusion cites its tested decision was incorrect for `ledger_ids_command`.
The reviewer sent the finding directly to P6 and Astra.
The reviewer will report BLOCKED to the lead to withdraw the earlier package CLEAN, with the correction commit.
No replacement READY has arrived. This correction is part of Round 2, not a new review round.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: NOT CLEAN

## PR #220 — Round 3 — 2026-10-10

Head: `8e73278755d66ab02b09141ad57c4bd0f41272b7`.
Tree: `bed25a7846737e17ffae1ca2af2f1c2c04b74a00`.
Parent: `9317693d8b1250a31a6a14e7c1125f18f3f7d0a5`.
PR and gate base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`.
The head contains the base. `git diff --check` finds no whitespace error.

Risk tier remains HIGH, under BUILD.md rules 1, 3, and 5.
The complete Round 3 change consists of one commit in two files: `lists.rs` and `.cargo/mutants.toml`.
The reviewer read the complete change, the reused decision and its proof, the updated body, and the exact-head gate evidence.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

**RH-F6 / RH-R2-1 — HIGH — CLOSED.**

`ledger_ids_command` reads each checked-in copy with a contextual error on a failed read.
A missing file therefore fails before the comparison. The function no longer converts read errors to optional comparison inputs.
It passes every `(path, copy, pinned text)` tuple to the existing pure `copy_problems` decision.
That decision returns a named problem for each different copy.
The shell reports those problems with `anyhow::ensure!` and prints success only when no problem remains.
The existing proof checks matching copies, matching empty copies, and a different copy with its correct file name.
The exact-head default gate passes `a_copy_that_is_not_the_pinned_file_is_a_problem`.
The corrected exclusion cites `copy_problems` and that proof in the required form.
The exclusion covers only the whole-body replacement of `ledger_ids_command`.
No exclusion covers `copy_problems` or its comparison operator.

The five original findings remain closed. All other corrected Round 2 checks and evidence carry to this head.
`real.rs` is byte-identical to the reviewed parent, so the accepted manual slow-mutation evidence remains applicable.
No merge or list change occurs in Round 3.
Plan 23u's separate pin-move exception does not apply: this PR retains contracts-v0.1.24 and adds no pending ID.

Supplied exact-head log:
`~/botster-sessions/gates/botster-core-stage1-p6-real-core-8e732787-pool-20261010-084907-54482.log`.
The recorded head and base match the values above. All ten stages PASS.
Default: 1464 passed, 497 skipped. Slow: 377 passed, 1929 skipped.
The reviewer compared the PASS sets with Round 2: all 193 testkit IDs and 106 real IDs remain passing.
The report retains 87 real-pending IDs, with zero newly passing IDs.
Minimum counts remain testkit 50 / 69, real-passing 29 / 68, and real-accepted 29 / 69.
Both default mutation steps report 125 tested, 95 caught, 30 unviable, zero missed, and zero timeout.
The job exits zero after 761 seconds; the gate exits zero after 762 seconds on msa1.
The second default mutation command still does not establish slow-feature coverage; the accepted manual evidence supplies it.

All six package findings are closed. No new finding remains open, including LOW.
The reviewer reports package CLEAN to the lead, P6, and Astra with this verdict commit.

VERDICT: CLEAN
