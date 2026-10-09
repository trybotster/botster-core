# Integration review: #168 (P3 M2a, branch stage1/p3-m2a-v1)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head 1c3154cf

Reviewed head: `1c3154cfe695282d2c8acab4d45d5bc3b769ffed`. Base: v1 `d1d18f4aeba717d2dec2708024c77f55e3ee35ef` (merged
at `50276544`). Scope: `botster-worker-core` (input.rs, worker.rs), `botster-worker` (main.rs, io_decisions.rs),
`botster-core-testkit` (program.rs, worker.rs, worker_controls.rs, harness, controls, sim), `.cargo/mutants.toml`.

### Checked, no finding

- **One PTY write at a time.** The machine sets `in_flight` before it emits `Action::PtyWrite`, and it emits no other
  write until `Input::PtyWritten`. So the real driver's `self.pty_write = Some(bytes)` cannot drop bytes.
- **Edge parity, real driver vs testkit edge.** A missing payload gives `Err(EIO)` on both. A write that the PTY does not
  take gives `Ok(0)` plus a wait for write readiness on both, and both then give `PtyWritable`. A failed write gives its
  errno, or EIO when there is none. The real driver retries an interrupted write in the next turn. The pure decisions are
  `io_decisions::pty_write` and `pty_write_interest`, which are tested in the default tier.
- **Readiness.** `ReadyState` adds `pending_write && !write_blocked`. The exhaustive test now covers all 256 bit
  states. A PTY event that is only writable no longer sets `pty_readable` (`control_ready`).
- **Hold on real-process tests.** No new real-process test code is in the diff. The real-PTY regression is parked
  (`parked/m2a-in_6_real_pty_cancel-regression.patch`), and the lead's DO NOT MERGE ruling stands until P6 rebuilds it
  on botster-test-process and it passes in the gate.
- **Testkit wiring.** `worker_controls` registers one function per control, through `registered_controls`. The program
  edge and the process edge are keyed by `(data_dir, InstanceId)`, so two directories that mint the same instance stay
  apart (F9, tested). `program.rs` is the in-process scripted program, with no real process.
- **Conformance accounting.** All 28 ids in `tests/worker_transcripts.rs` are in `conformance/core-pending.txt`.

### N1 MEDIUM — whole-function exclusions hide new M2a decisions in `Driver::run` and `Driver::perform`

`.cargo/mutants.toml:59` excludes every mutant `in Driver::run`, and `:63` excludes every mutant `in Driver::perform`.
Their proofs (for example `pty_events_resume_reads_after_would_block`) are from before M2a and write no input. M2a adds
these decisions inside those functions:

- `Driver::run`: `if event.is_writable() && self.pty_wants_write` (the `&&` to `||` mutant), and the new
  `control_ready(...)` call for the PTY token.
- `Driver::perform`: the arm `Action::PtyWrite(bytes) => self.pty_write = Some(bytes)` (the delete-match-arm mutant).
  Without this arm, no host write reaches the real PTY.

`mutants.toml` itself says: "Recheck each glue entry when its body or callers change. New pure decisions require
default-tier tests." This is the same class as #167 E4: a whole-function exclusion hides a decision that changed. Fix
one of these two ways:

- Move the decision into `io_decisions` as a pure, tested function (as M2a did for `pty_write` and
  `pty_write_interest`).
- Name a test that catches the mutant in the entry's comment. If that test is the parked regression, the entry states it
  and carries the same DO NOT MERGE rule as the `write_pty_once` entry.

Mechanical check: list every boolean operator and every match arm that M2a added inside each excluded `Driver::*`
function. Account for each one in the PR body.

### N2 LOW — `botster-route-codec` is a production dependency of `botster-worker-core`, but only its tests use it

`crates/botster-worker-core/Cargo.toml` adds `botster-route-codec` under `[dependencies]`. The only use is
`src/worker/tests.rs:6` (`HexBytes`). This adds a package edge to the production worker for test code. Move it to
`[dev-dependencies]`, or use the `HexBytes` that the contract prelude already gives.

### State: v1 moved

v1 is now `ee7dd16c73a6991b6ef9b3a85d84fd93e57a3230` (#164 merged). A trial merge with `1c3154cf` conflicts in
`crates/botster-core-testkit/src/core/tests.rs`, `src/worker.rs` and `src/worker/tests.rs`. A CLEAN needs a head that
contains current v1. This reviewer then checks the merge delta.

### Observation (no action asked)

When the real driver's PTY has left the poll (`deregister_pty`) while the leader lives, a write that gets `Ok(0)` turns
no write interest on. The transaction then waits until the payload exits or the host cancels it. Both are bounded, so
this is not a finding. P3 can say so in the module documentation.

VERDICT: NOT CLEAN (2 open: N1 MEDIUM, N2 LOW; v1 merge needed)
