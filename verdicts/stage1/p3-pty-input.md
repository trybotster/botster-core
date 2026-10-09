# Integration review: #198 (P3 PR A: the AM-2 admission point and the PTY write path; branch stage1/p3-pty-input)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head fc39fee6

Reviewed head: `fc39fee6be5024df41a0cc937417401100c6303b`, six commits on v1 `7aec2bb9`, which is the current v1 tip
(16 files, +1548 -40). P3's gate log `gates/botster-core-stage1-p3-pty-input-fc39fee6-pool-20261009-122909-62645.log` names
this head and base `7aec2bb9`. It is a full Linux pool gate: 1040 default and 245 slow tests passed; conformance passed 46,
failed 0; the mutants job had 118 mutants (108 caught, 0 missed, 0 timeout, 10 unviable); exit 0. This reviewer read its
header, its summaries and the lines of the cited real-PTY test. The head contains the v1 tip, and the gate's base is that
tip (the Merge bullet of plan 23d).

The stated tier HIGH is correct: the testkit and `.cargo/mutants.toml` (rule 3), and the PTY write path (rule 5).

- **The pending list.** The change only removes 13 ids. The replacement map gives each one `core-testkit` or
  `core-testkit+perturb`, and none is `slow:*`. So by plan 23a, a TestkitHarness pass is enough. None is a minimum id.
- **`.cargo/mutants.toml`.** Two new whole-function exclusions are added: `Driver::write_pty_once` and
  `Driver::set_pty_write_interest`.
  - Each function makes one OS call. Its decisions are `io_decisions::pty_write` and `io_decisions::pty_write_interest`,
    which are tested in the default tier.
  - The cited proof `in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write` runs in the slow tier of this gate, in
    two binaries (`slow_session` and `bin/botster-worker slow_driver`, log lines 2456 and 2485).
  - The `Driver::run` and `Driver::perform` entries change only their comments.
- **The two drivers of `Action::PtyWrite`.**
  - The real driver writes once per turn before the PTY read. It waits for write readiness after a write that the PTY
    did not take. `deregister_pty` clears the write interest.
  - The testkit edge makes the write its own `Ready::PtyWrite` input, so the scheduler orders it. A write with no PTY
    gives `EIO`, and `WouldBlock` gives `Ok(0)` with `PtyWritable` at the next write readiness.
- **The testkit lock order (shared code).** `Workers::programs` takes `run_processes` and then each cell. `has_ready`
  calls it while the `sim` guard of its left operand is still alive (edition 2021: the temporary lives to the end of the
  `||` expression). So the order is `sim -> run_processes -> cell`, and `sim -> program`. Inside `run_until_idle`, the
  edges take `sim -> owner -> cell`. No path takes a cell, an owner or `run_processes` and then `sim`. So no cycle exists,
  and run_processes -> owner -> cell holds.
- **Cross-package dependency.** `botster-worker-core` adds `botster-route-codec` as a dev-dependency only. `Cargo.lock`
  adds only that edge.
- **The real-PTY regression** (`botster-worker/tests/common/session.rs`).
  - It uses `/bin/cat` and `/bin/echo` for the FIFOs, and the test's FIFO ends are `CLOEXEC`.
  - Each marker wait is one `poll` with a 5 s deadline and the timer marker. The link reads keep the helper's deadline.
  - `exec sleep 30` is the payload itself, and `Kill` ends it before `remove`.
  - The 196 608-byte write is larger than a PTY input buffer while the payload does not read, so the test always gets a
    proper prefix.

Condition (the same kind as #181/#184): #181 changes the citation form of every `.cargo/mutants.toml` reason (plan 23c/23d:
`decision (proof, …)`, and a proof named only in free text fails). The two new entries here cite their proof in free text.
Whichever of #181 and #198 merges second must put these two entries in the strict form. That merge needs this
reviewer's review of the `.cargo/mutants.toml` union and a full gate on its head.

Observation (not counted): `has_ready` can release `sim` before `programs()` runs (`let ready = lock(&self.sim).has_ready();`).
This is not needed for correctness now, but it keeps the `sim` guard out of later lock paths.

VERDICT: CLEAN (0 open) at fc39fee6be5024df41a0cc937417401100c6303b

### Correction after round 1 (same head fc39fee6) — CLEAN WITHDRAWN (the P3 package reviewer's F64, missed here)

The P3 package reviewer's F64 HIGH is real, and it is in this reviewer's scope: shared real-process test code (rule 3),
a fixture shared across packages, and a gate-decision reason in `.cargo/mutants.toml`.
- `crates/botster-worker/tests/common/session.rs:14-18` includes `../../../botster-core-sys/tests/common/payload_guard.rs`
  and `process_guard.rs` through `#[path]`. The new regression `in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write`
  uses `PayloadGuard` and `OwnedWorker` from those legacy fixtures.
- `crates/botster-worker/Cargo.toml` has no `botster-test-process` dev-dependency.
- The new exclusion reason (`.cargo/mutants.toml`, M2a PTY write glue) says that the proof runs "under the payload guard
  of botster-test-process". That is false at this head.
- Plan section 6.1 (revision 22): P6 owns all real-process test code in `botster-test-process`, and no new real-process
  test code is written until it and the xtask check land. A production change whose proof is a real-process test waits for
  that proof. The lead's M2a ruling (2026-10-08) names this regression as the merge proof.

Round 1 checked the deadlines, the FIFO forms and the ownership of the payload in the test body. It did not check which
crate gives the guards. That miss is in this reviewer's scope.

Fix: F64's. Rebuild the regression on `botster-test-process` (P6 for any missing capability), without copying guard or
cleanup code. Correct the exclusion reason and the PR body, and give a gate on the exact new head. F65 (the strict
`decision (proof, …)` form of the two new entries) is the round 1 condition, which the lead already ordered as a delta.

VERDICT: NOT CLEAN at fc39fee6be5024df41a0cc937417401100c6303b (1 open: F64 HIGH, the package reviewer's finding,
confirmed here)

## Round 2 — NOT CLEAN on head 8a762ac0

Reviewer: integration reviewer (Astra), `sess-1791575341-0172-bec01ec06119f11c948f14371195fa92`.

Reviewed head: `8a762ac00a354b032432a5a26963d93c91e0284c`.
The handoff permits a delta review from `fc39fee6be5024df41a0cc937417401100c6303b`.
The complete delta changes five files: the mutation reasons, workspace dependency, lockfile, worker dev-dependency, and session fixture.
The stated HIGH tier is correct under rules 3 and 5.

### Evidence and closures

- The full Linux pool log names the exact head and base `7aec2bb917f48994d1705301d2383873a219c031`.
  The fetched `origin/v1` equals that base, and `git merge-base --is-ancestor` confirms ancestry.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-pty-input-8a762ac0-pool-20261009-125251-39706.log`.
- All ten CI steps pass. The default tier passes 1040 tests; the slow tier passes 245 tests.
  The named real-PTY proof passes in `slow_session` and `bin/botster-worker slow_driver` (lines 2452 and 2480).
  Both mutation runs report 108 caught, 0 missed, 0 timeout, and 10 unviable.
  The second run uses `NEXTEST_PROFILE=slow`. The gate exits 0.
- F65 closes. Each new exclusion uses `decision (proof, ...)` and names its pure decision test and real-PTY proof.
  The log selects both pure tests in the default tier and both instances of the real proof in the slow tier.
- F64's dependency and legacy-guard defects are corrected. This regression now uses `GuardedSession`, `Guard`, `OwnedChild`, `Blocker`, and `Bounded`.
  The worker depends on `botster-test-process` only for tests. The legacy helpers remain for the unchanged tests.
  The PR description and exclusion reasons now describe the actual fixture.
  The replacement fixture has the separate cleanup defect R2-1 below.
- The shared hello/launch extraction preserves the wire exchange and read timeout.
  The new marker reads and child status waits use shared deadlines. The accept loop uses a nonblocking listener and a deadline.
  The successful fixture construction orders cleanup as payload release, worker cleanup, then guard outcome.
- The previous source review carries for unchanged product code, testkit lock order, and the thirteen pending removals.
  The reviewer read the package verdict at the preceding head (`6eaf15d971a37c3b2a867c30d891699ac4eac65b`, round 123).
  The fetched package verdict did not yet contain a round for this replacement head.

### R2-1 HIGH — the new observer/worker has no cleanup when its test parent dies

`crates/botster-worker/tests/common/session.rs:618-628` starts the observer directly with `OwnedChild::spawn_group`.
The other branch starts the worker directly with `OwnedChild::spawn`.
`OwnedChild` cleans up in `Drop`; neither spawn path installs an anchor or another parent-death mechanism.
The fixture's only `Guard` wraps the payload (`:603-608`), whose group does not contain its worker parent.

If the test process is killed after launch, its destructors do not run.
The payload anchor receives EOF and ends the payload group. It does not end the observer/worker.
The observer/worker receives control-link EOF, but `Worker::handle(Input::LinkClosed)` deliberately preserves the worker for DP-8
(`crates/botster-worker-core/src/worker.rs:714-720`). Payload exit also does not request worker exit.
Thus the fixture can leave its observer/worker alive after its test parent dies.

The existing `parent_death_ends_the_driver_observer` test does not cover this path.
Its `observer_parent` helper still calls legacy `Session::launch` (`session.rs:583`), which installs the legacy observer group guard.
The reported panic check exercises `Drop`, so it cannot prove cleanup after test-process death.

Required correction: give both new spawn paths parent-death ownership through `botster-test-process`.
Keep production's DP-8 behavior unchanged. Add a bounded proof that ends the fixture parent after startup and observes the observer/worker end.
Use the shared crate for guard and cleanup capabilities; do not copy the legacy guard.

This is a shared process-ownership finding in the reviewed delta. The reviewer sent it directly to P3 and the P3 reviewer.
The reviewer ran no test, build, or gate.

### Union condition retained

The second of #181 and #198 to merge still needs an integration review of the union and a full gate on that head.
P3 reports that the interim accept needs a `process-check` allow entry until P6 supplies the shared accept helper.
That entry and its scope must be checked in the union; this review does not approve an absent entry.

VERDICT: NOT CLEAN (1 open: R2-1 HIGH) at 8a762ac00a354b032432a5a26963d93c91e0284c

## Round 3 — CLEAN on head 546f8084

Reviewer: integration reviewer (Astra), `sess-1791575341-0172-bec01ec06119f11c948f14371195fa92`.

Reviewed head: `546f8084f6bbcbfe9ca2ba9ef323dc74f68bec15`.
The complete delta from `8a762ac00a354b032432a5a26963d93c91e0284c` changes only the session fixture.
The preceding rounds cover the unchanged product code and shared testkit. The HIGH tier remains correct.

### R2-1 closed

- Both the worker and driver observer now start through `Guard::wrapper` and `OwnedChild::spawn_group`.
  The fixture verifies the worker anchor's leader before launching the payload, then verifies the second anchor's payload leader.
  That sequence makes the two report indices deterministic.
- Test-parent death closes both guard connections. Each anchor ends its group without a test destructor.
  Normal cleanup releases both anchors, ends and reaps the worker, then reads guard outcomes.
  The fixture keeps shared identity checks and group reservation. Production's DP-8 behavior and payload reaping are unchanged.
- `a_guarded_session_ends_when_its_test_parent_dies` waits for the fixture parent's report before killing that parent.
  Shared helpers bound the report read and status wait. The helper's own stdin wait is also bounded.
  `rounds::await_group_end` then observes both groups without signalling them.
- The reviewer read the saved rollback patch, failure log, failing source excerpt, and survivor listing in
  `~/botster-sessions/shared/core-stage1/evidence/p3-198-r2-1/`.
  The patch removes the worker wrapper use and its anchor wait, and adjusts the payload anchor count.
  Both binaries fail at the group-end observation after parent death, not at startup.
  The saved listing identifies the surviving worker and observer, each with PPID 1.
  This verifies that the proof detects R2-1. P3 reports that it then ended both survivors by their exact PIDs.

### Exact-head evidence

- The full Linux pool gate names this head and base `7aec2bb917f48994d1705301d2383873a219c031`.
  Ancestry passes. A final remote-reference check confirms that `v1` and the PR head still equal those commits.
  Log: `~/botster-sessions/gates/botster-core-stage1-p3-pty-input-546f8084-pool-20261009-130637-72222.log`.
- All ten CI steps pass: 1040 default tests and 249 slow tests.
  Both parent-death proofs pass (lines 2448 and 2481), as do both real-PTY cancellation proofs (lines 2453 and 2484).
  Both mutation runs report 108 caught, 0 missed, 0 timeout, and 10 unviable.
  The supplemental run uses `NEXTEST_PROFILE=slow`. The gate exits 0.
- The reviewer read package round 125 at verdict commit `c24571c34aef5ba5d1c0e1b9c0aaeadc7f8bce97`.
  It reports CLEAN on this exact head and closes F64, F65, and F66 (integration R2-1).
- The PR description names the corrected ownership, proof, and exact-head gate. `git diff --check` passes.
  This reviewer ran no test, build, or gate.

The #181/#198 union condition remains: the second PR to merge needs an integration union review and a full gate.
The lead's accepted bounded-accept carry also remains. The second PR adds the temporary allow entry;
P6's next crate PR supplies shared bounded accept, migrates this site, and removes that entry.

VERDICT: CLEAN (0 open) at 546f8084f6bbcbfe9ca2ba9ef323dc74f68bec15
