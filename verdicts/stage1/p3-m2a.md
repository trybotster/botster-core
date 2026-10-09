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

### Note after round 1

- The P3 package reviewer opened F53 LOW at `1c3154cf` (`Program::write`: with `pty_chunk` and a spent `pty_accept`, it
  sets `step_refused`, so `Workers::has_ready` asks for one idle pump too many). Owner: the P3 package reviewer.
- Lead clarification: CLEAN means merge-ready. After the logic findings close, M2a stays NOT CLEAN with one HOLD: the
  real-PTY cancel regression, rebuilt on botster-test-process, must pass the gate. This reviewer gives no
  development-only CLEAN on #168.

## Round 2 — NOT CLEAN on head 7550018f (logic findings closed; merge HOLD)

Reviewed head: `7550018fff91ed948dbdfb7eea14caec48b27d6c`. Commits since round 1: merge `33a2bc87` (parents `1c3154cf`,
v1 `ee7dd16c73a6991b6ef9b3a85d84fd93e57a3230`, which is still `origin/v1`), then fix `7550018f`. I diffed the head
against a trial merge of `1c3154cf` and `ee7dd16c` (tree `ff9bd56f`). The difference is exactly the three conflict
resolutions plus the N1, N2 and F53 fixes. This reviewer ran no build, test or gate.

- **v1 merge.**
  - `testkit/src/worker.rs`: the spawn inserts into v1's `run_processes` and into M2a's `worker_processes`, the latter
    with the F9 key.
  - `worker/tests.rs`: the test uses v1's `workers` with `data_dir: "d"`.
  - `core/tests.rs`: v1's `HostDriver::open` with M2a's five-argument `TestkitCore::new`. In the reopen test, the spawner
    and the Core both use the data directory `"reopen"`.
- **N1 CLOSED.**
  - The writable decision is `io_decisions::pty_writable`, a pure function with a four-case test.
  - `Driver::perform`'s `Action::PtyWrite` arm has no delete-arm mutant, because cargo-mutants deletes a match arm only
    when the match has a wildcard arm, and this match has none.
  - The `Driver::run` and `Driver::perform` entries now state what M2a added and name the parked regression for the write
    half, under the DO NOT MERGE rule.
  - In `set_pty_write_interest`, the `|` of READABLE and WRITABLE joins disjoint flags, so `^` gives the same value. The
    `&` mutant is the regression's to catch.
- **N2 CLOSED.** `botster-route-codec` is under `[dev-dependencies]`.
- **F53 (P3 package reviewer) fixed.** A spent `pty_accept` no longer sets `step_refused`. A new test covers the case
  where both limits are spent together.
- **Observation (history only; the tree that lands is correct).** Merge `33a2bc87` did not keep git's clean result for
  `.cargo/mutants.toml`, which did not conflict. It dropped #164's entries (`RealEdges::diagnostics`, the descriptor walk,
  `HostEdges::diagnostics`) and restored the removed `FileStorage::path` and `read_file` entries. `7550018f` restores v1's
  content, but its message does not say so. The head's `mutants.toml` equals v1's plus M2a's entries only. P3: say this in
  the PR body, so that nobody bisects to `33a2bc87` and trusts its mutation config.
- **HOLD (lead ruling, 2026-10-08).** CLEAN means merge-ready. M2a merges only after the real-PTY cancel regression
  (`in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write`) is rebuilt on botster-test-process and passes the gate.
  The `write_pty_once`, `set_pty_write_interest`, `Driver::run` (write half) and `Driver::perform` exclusions cite it as
  their proof.

VERDICT: NOT CLEAN (0 logic findings open; HOLD: the rebuilt real-PTY regression must pass the gate)
