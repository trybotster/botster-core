# fake-core: feature notes

Feature `fake-core` (design section 12, lane A, order 3). Crates: `botster-fake-core`, `botster-core-conformance`, `botster-conformance-probe`. Transcripts: `conformance/core/` (132 ids: Core sections 4, 5.1, 5.1A, 5.2, 9.1, 9.2, 9.3, 9A and 9B).

## Prior art (BUILD.md rule 0)

- Vault: "a non completing shared fake adapter is a blind no progress oracle" (every negative assertion here uses an observer that changes: a `poll_events` that finds nothing after a pump, a PTY log, a pending-slot limit; see the ablations in the review message). The R-5 notes (FakeCore scripts observations, validates input clauses without encoding) fix the scope.
- botster-core @ 72b2e3354ffc: `crates/botster-core-test-support/src/fake/*` and `terminal_adapter/*` model the screen and the old actor vocabulary (R-5 forbids the first, and neither follows begin, pump and one queue). `crates/botster-core-daemon` tests use the old daemon. Nothing was stolen, so there is no `Stolen-From` commit.
- Reused inside this repo: `botster-fake-route` (the route endpoint and its controls) and the route step driver of `botster-hub-conformance` (`route_*` steps), `botster-conformance` (engine, seeds, macro generator).
- Hand-rolled with a reason: the Core step driver and FakeCore itself (no maintained library does spec-first clause transcripts with seeded interleaving of a begin, pump and event-queue API).

## Placement and judgment calls

1. **The one admission point lives in `botster-fake-route`** (`host.rs`). Core AM-2 puts host writes and client frames in one serialization point in the worker, and the route endpoint is that worker. The host lane is generic: it takes a closure that decides at the start of the write, so every Core rule (guards, mode checks, scripted outcomes) is in `botster-fake-core/src/input.rs`.
2. **The probe binary depends on `botster-fake-core`** for the script type, so one script drives FakeCore and real Core (design 6.2) and the two cannot drift.
3. **`conformance_tests!(make, schemas)`** takes `Fn(seed) -> Box<dyn StepDriver>` (use `driver_for(Box::new(harness))`), so a fake gets its seed.
4. **`er_deadline_expired` is `withdrawn`**: A2-1 removes `DeadlineExpired`.
5. **Not modeled yet** (later suite groups; `begin` gives `Unsupported`): `AttachWebRtc`, `Adopt`, `SetColorProfile`, `SetOutputTap`, `SetNotificationPolicy`, `EndEpoch`; class D and L events; EV-5b mandatory-queue pressure; service lanes and epochs.
6. A start that cannot run (a `cwd` that does not exist, A2-1) is a real filesystem check.
7. Structural ids (R-5): the encoding ids (`in_9_legacy_keys_table`, kitty, mouse, application modes, non-US text, the worker-modes-at-write id) and `am_2_no_interleave_across_paste_markers`. Their steps use the control `oracle_encode`, which only a harness with the pinned libghostty implements (Core repo). The README lists them.

## Test controls (design 6.3)

FakeCore: `fail_next`, `lose_worker`, `break_control`, `pty_chunk`, `pty_blocked`, `pty_input`, `pty_output`, `withhold_feature`, `unavailable_bound`, `control_queue`, `revisions`. Route controls come from `botster-fake-route`. A real harness builds each from injected parts. `oracle_encode` is real-only.

## Local note

`include_dir!` does not rebuild when a transcript file changes in place: touch `crates/botster-core-conformance/src/lib.rs` (CI builds fresh).
