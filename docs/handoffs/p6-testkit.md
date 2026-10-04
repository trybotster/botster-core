# Handoff: P6 testkit (Stage 1 Core), implementer

Written at the lead's PAUSE. A new agent can resume from this file.

## Where things are
- Repo trybotster/botster-core, branch `stage1/p6-testkit`, worktree `~/botster-sessions/trybotster-botster-core-stage1-p6-testkit`.
- Head of the branch: `744da574039d30d49047b6a661d6849887de53b9`. Everything on the branch is already in `origin/v1` (PR #129 merged as
  `67124ec`; `v1` is now past that). **Nothing is unpushed.** The worktree has one pre-existing uncommitted change, `.gitignore`
  (it deletes the `mutants.out` and bolero ignore lines). The lead said: never reset it, never commit it. `botster-gate` refuses a dirty
  tree, so I gate a clean detached worktree: `git worktree add --detach ~/botster-sessions/p6-gate-15f26f3 <sha>` (already exists; do
  `git checkout --detach <sha>` in it, then `~/botster-sessions/shared/tools/botster-gate .`).
- Reviewer: `sess-1790912516-00a5-b0c58a4f635433a12164a49d5e37602d` (verdicts in `verdicts/p6-testkit.md` on `stage1/review-p6`).
  Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`. P1: `sess-1790919734-00aa-d8597316795619ab4c83427e523a0f96`.
  P3: `sess-1790940814-00b7-10ebbedc6ec6f9fe6ea13a273ee6925d`.
- Read first: `shared/core-stage1/pair-common.md`, `brief-p6-testkit-scope2.md`, plan pin `~/botster-sessions/pins/stage1-plan.2b03dc1b.md`
  (4.1 to 4.2c, 5, 8), BUILD.md, and `docs/core-testkit-controls.md` of botster-contracts at the pinned tag. Design note of the crate:
  `crates/botster-core-testkit/DESIGN.md`.

## What landed
- **M0b (PR #126):** crate `botster-core-testkit`: seeded scheduler (one ChaCha8 stream, a named function per A5-2 item), seeded
  `Entropy` (stream 1 of the seed), `SimWake`, in-memory `Link`/`RouteTransport` with readiness flags and interests (plan 2.5 rule 8),
  scripted program edge over `botster-probe-script`, the `Sim` (`Node`, `Binding`, `MachineNode`), `TestkitHarness` wired into
  `tests/conformance.rs`.
- **Scope 2 step 1 (PR #128):** pin to contracts-v0.1.7; harness reads the contracts' `deferred.txt`/`withdrawn.txt` (verbatim copies
  `conformance/contracts-*.txt`, checked by `cargo xtask lists`; parser `botster_core_testkit::status`); five report counts and listed
  not-applicable cases; xtask resolves the base commit once (`fsutil::base`); nightly verify-only.
- **Partial step (PR #129, `67124ec`):** pin to contracts-v0.1.9 (ledger 643; the 3 E4 ids are pending); `RefusalScript`/`RefusalLayer`
  (`refusal.rs`) and the `fail_next` control; `EndControl` (route-stream controls) in `net.rs`; `ProgramControl` (program controls) and
  the piece-queue `Output` in `program.rs`; scheduler `Overrides`; `statements.rs` (`check_crates`); `candidate.rs` (manifest/sha check)
  and `process_group.rs` (`OwnedGroup`) with slow-tier tests; `.cargo/mutants.toml` exclusion of process_group glue.

## What is left of scope 2, and what each item waits for
- **Waits for P1 and P3 (their stacked testkit PR, branches `stage1/p1-testkit-wiring`, `stage1/p3-worker-m1-stack`,
  `stage1/p3-worker-m2a`):** they add `core.rs` (SimEdges, Faults, Directories, TestkitCore wiring), `worker.rs` (WorkerSpawner,
  TestkitCore, Workers), `harness.rs` open/dispatch of the `pty_*`, `process_end_worker`, `lose_worker`, `break_control` controls, and
  small `program.rs` additions (`input_chunk`, `new_step`, `write_plain`, `is_writable`). I reviewed that part (findings F1 to F8, P3
  took F1 to F5). **Do not edit `harness.rs`, `program.rs`, `core.rs`, `worker.rs` until that PR lands**, then rebase.
- **After that PR lands, mine to build** (all as edge state behind handles; ask the lead if a control seems to need a production-crate
  change): `hold_start`/`release_start`, `hold_start_at`, `spawn_record`, `withhold_control_link` (also lets `lose_worker
  reason: worker_unreachable` work), `storage_fail_write`, `process_refuse_spawn` (Faults has `next_write`, `next_spawn`),
  `registry_row`, `corrupt_registry_row`, `impostor_worker`, `signals_received`, `announce_protocol`, `host_uid`, `process_identity`,
  `scheduler_hold/release/order/ready/delay` (need operation or session identity from P1's driver), `edges_quiet` (the `await_quiet`
  fence, never polls), `no_spurious_wakes`/`wake_spurious` (merge P1's `SimHostWake` into `SimWake`: add the Condvar wait, wire
  `maybe_wake_spuriously`), `payload_alive`, A12-1a held report and A12-1b cleanup script point, A9-3 compression choice,
  `control_link_stats` (R-14.3: count at the link boundary only, no sync point), `LinkEnd::has_bytes_or_eof()` (P3 F3; descriptors
  must not look like bytes), `service_*` controls and the in-memory service lane (A6-1; waits for P7 and the SV-2 preamble).
- **Waits for P2 (`botster-terminal-ghostty`):** `oracle_*`, `snapshot_unsupported_version`, `oracle_resume_every_cut`.
- **RealCoreHarness integration (plan 4.2):** the type, `slow` feature in `tests/conformance.rs`, controls built from injected parts of
  the real `Core`. Scaffolding exists (`Candidate`, `OwnedGroup`); it needs P1's `Core::open` and P3's `botster-worker` binary
  (`cargo xtask prebuild-worker` writes `target/candidate/manifest.json`).
- **The statements `run_suite`, `error_codes_reachable`, `run_deterministic`** (`CoreHarness::statement`; `run_deterministic` can use
  `botster_core_conformance::run_script_events`; `error_codes_reachable` should use `refusal::ROWS`).
- **My 15 ids** (A5-1 to A5-4, OR-3): they stay in `conformance/core-pending.txt` until P1's `open` works in the testkit (every
  transcript opens a default handle first) and the controls they require exist (`docs/core-testkit-controls.md`). Remove an id from
  pending only in the PR that makes it pass; `cargo xtask lists` checks. a5_x ids that need a real-process proof stay pending with the
  reason until RealCoreHarness runs.
- Re-measure R4 (the budget example `examples/budget.rs`) once ids run; the first numbers measure only the failing-open path.
- The driver defect P3 raised (`conf::a2_2_cancel_race_reports_the_real_outcome` and `await_pty` with the injected clock) is FIXED
  in `contracts-v0.1.13` (`a8db5c9`): the driver checks `pty_input` again after each pump before it declares Core idle. P6 is still
  on v0.1.9: move the pin to v0.1.13 in its own commit, then verify that the a2_2 cancel-race id and the `await_pty` paths run
  through the conformance harness with the new driver.

## Decisions in force
- **RefusalScript (plan 4.2a):** a sync refusal is returned before the call reaches Core; `ROWS` is the per-call sync column, each row
  cites its clause, with a test per row against `botster-core-contract` types and a check that every `Op` variant has a row. A code
  outside the column is `ScriptError::NotInSyncColumn` (the harness maps it to `ControlError::Refused({call, code, reason:
  "not_in_sync_column"})`). Entries carry an absolute call number; two entries on one call number conflict; overflow is an error.
  **R-19:** on every scripted attach refusal the layer returns the caller's transport (`AttachRefused{error, transport}`, contracts
  v0.1.9). A call with no row cannot be scripted. Async failures are never scripted: only an edge produces them.
- **Edges, not branches:** controls are edge state behind handles that outlive the move into a worker (`EndControl`, `ProgramControl`,
  scheduler `Overrides`). No test branch in a production crate (BUILD.md testing rule 8). A fixed choice draws nothing from the stream.
- **A5-2 sources:** route WRITE size is not a choice point (A5-2 names only read sizes); the session visit stays round-robin.
- **Mutation policy:** `cargo xtask ci` runs `cargo mutants --in-diff`; a missed mutant is a review finding: kill it with a test or
  state why it is equivalent; `.cargo/mutants.toml` `exclude_re` entries are per function with an accurate reason, never to make a job
  pass. `process_group.rs` glue is excluded per function (the slow tier proves it); a pure decision added there stays mutated.
- **Flow:** commit in small steps, push (never force), send the reviewer the exact head, gate once on the exact CLEAN head with
  `botster-gate` on a clean worktree, DONE to the lead with head, verdict commit and gate log. Report to the lead only QUESTION,
  BLOCKED or DONE. Never poll: end the turn and wait for the doorbell. One heavy botsterq job at a time.
- Prior art: no old botster-core code was copied (no `Stolen-From`); `candidate.rs` reuses only the idea of the old `real_worker.rs`.
  Hand-rolled with reasons: the `Sim` (turmoil/madsim need tokio), the in-memory duplex (readiness flags, descriptors by value, scheduler
  read sizes), the probe interpreter (the contracts leave it to the testkit), the status-file parser (the contracts' parser is inside
  their xtask).

## Reviewer state

Last reviewed head: `744da574039d30d49047b6a661d6849887de53b9`.
Last verdict commit: `2793fd65d609409271f922e9b0c587e2eb27134e` on `stage1/review-p6`.
Verdict: CLEAN. Open findings: none.

The review covers the refusal layer, edge building blocks, statement check, candidate reader, and process group guard.
It does not approve full Scope 2 completion. RealCoreHarness type and integration, harness control dispatch, quiet fences, identity-dependent controls, terminal oracle controls, and conformance proofs remain outstanding.
The reviewer ran no tests or gate. The implementer must provide a green gate on the exact reviewed head.
The reviewer preserved the existing dirty `.gitignore` and did not stage it.
The verdict branch is pushed. The reviewer is paused and will stay idle until the lead retires it.
