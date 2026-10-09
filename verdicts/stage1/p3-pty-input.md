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
