# Plan: preserve live output across repeated ProcessExited writer-barrier rounds

- Ticket: `ticket_1787034922_646556`
- Run: `run_1787034943_217745`
- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Base: `d981bb0` (Core main with `c23b833` ProcessExited-on-payload and `dbb4cbc` writer barrier)
- Revision 2: addresses Plan Review `review_1787036342_918375`
  (`finding_1787036342_571063`, `finding_1787036342_394153`, `finding_1787036342_342091`).

## Problem

Hub test `external_hub_webrtc_shutdown_after_live_exit_is_idempotent_cleanup` runs five
spawn → attach → write → natural-exit → shutdown rounds on one daemon. At Core `fd66efd`
the test passes. At Core `d981bb0` round 0 passes and later rounds panic at
`webrtc_proofs.rs:581` with empty live bytes: the producer wrote 4 bytes and exited, and
the already-attached WebRTC subscriber received nothing. Plan Review independently
reproduced the baseline failure (round 1, empty live bytes) at Hub `57cbeb2` / Core
`d981bb03`.

The Hub test function is byte-identical to Hub `origin/main`. Hub ReadScreen and WebRTC
forwarding did not change. The regression is Core-side, introduced by the combination of:

1. `c23b833`: `WorkerProcessRuntime::drain_output` now returns the `ProcessExited`
   payload as soon as the worker frame arrives, re-pumps once, and removes the runtime
   session in the same call (`crates/botster-core/src/runtime/worker_process.rs:1350-1375`).
   The old reader-EOF + reap + zero-status gate is gone (intentionally; do not restore it).
2. `dbb4cbc`: the worker writer thread treats `FRAME_PROCESS_EXITED` as a terminal
   barrier and stops after writing it
   (`crates/botster-core-daemon/src/bin/botster-session-worker.rs`, `write_egress_lanes`).

## Production delivery path (bound adapter, not DrainSubscription)

The failing Hub route is a **Core-bound WebRTC TerminalAdapter**, not host-pulled
`DrainSubscription`. Per `CoreDaemon::bind_terminal_adapter`
(`crates/botster-core-daemon/src/daemon.rs:913-937`): after bind, this route's terminal
frames leave **only** through the adapter; `drain` / `drain_subscription` do not also
return them. The bound-adapter egress owner is the synchronous ClientWorker
(`crates/botster-core/src/engine/client_worker.rs`):

- Every engine drain and session request (`drain_runtime_once`,
  `drain_runtime_all_once`, `handle_session_request` — which `read_screen` uses —
  `shutdown_session`, `pump_bound_adapters`) runs `apply_client_worker`
  (`managed_session_runtime.rs:968-998`): `ingest_bound_terminal_frames` strips
  bound-route terminal frames from client egress into per-owner queues, then `pump()`
  writes queued frames to adapters.
- `pump_one` (`client_worker.rs:488-568`) writes the queue head with `try_write`,
  tracks in-flight completion through `pressure()`, and — once the ProcessExit frame is
  delivered (`process_exit_delivered`) — performs `hard_stop`: remove the owner, clear
  the queue, `close()` and drop the adapter on the same host tick, per
  [[Core subscription hard-stop is synchronous close and drop on the host tick]].
- `ingest_bound_terminal_frames` (`client_worker.rs:309-396`) has three edge behaviors
  on this path: frames for an attached-but-not-yet-bound owner are retained back into
  client egress, an **unbound** ProcessExit hard-stops the route immediately
  (`client_worker.rs:343-345, 389-393`), and frames arriving after
  `process_exit_enqueued` are dropped (`client_worker.rs:349-351`).

The Hub test drives this path with repeated `ReadScreen` requests (every 30 ms) while
reading the adapter handle. It never calls `DrainSubscription` in the failing window.

## Diagnosis (evidence-backed hypothesis chain)

Single-round worker frame order is correct: `drain_output` pushes pending PTY bytes into
the output batch before the `ProcessExited` event, and the worker's single reader thread
stores `process_exited` only after every earlier frame's send returned. The W1/W2 proofs
cover that ordering.

What changed observably: at `fd66efd` the delivery gate delayed `ProcessExited` (and the
`Exited` lifecycle transition) until worker reap, so live bytes always reached the bound
adapter in **earlier host ticks** than the ProcessExit frame. At `d981bb0` a warm round
completes write + exit fast enough that the bytes and `ProcessExited` arrive in **one**
runtime batch, so one `apply_client_worker` pass enqueues `[bytes, process_exit]`
together and one `pump_one` tick can write both and then hard-stop. Cold round 0 still
separates them; warm rounds collapse the window. The observed flake pattern (4/4 red in
review, 1 pass then 3/4 red in Implement, round-1 failure in Plan Review's baseline)
matches a timing race, not a per-round counter.

Candidate mechanisms, ordered by likelihood; the Implementer must confirm one with a
differential trace and rule the others out with evidence (ablation or trace, per
[[ablate dont argue to prove a branch is unexercised]]):

- **C1 — same-tick close abandons accepted writes.** In `pump_one`, when
  `try_write(bytes)` and `try_write(process_exit)` both return `Ok` with `pressure() ==
  Ready` in one tick, the loop pops both, sets `process_exit_delivered`, and
  `hard_stop` closes and drops the adapter on that same stack. `Ready` means the
  adapter accepted the write, not that the transport flushed it; Hub's own adapter code
  comments that host close abandons an in-flight frame. Closing on the tick that
  accepted the final writes can destroy the only flush path for those bytes. At
  `fd66efd`, seconds of reap delay guaranteed the bytes were flushed ticks earlier.
  This matches the reviewer's failure shape: "the bound adapter still closes before its
  accepted write completes."
- **C2 — attach-before-bind exit hard-stop.** Between Core attach (owner created,
  `adapter: None`) and Hub's bind, an ingested ProcessExit for the unbound route
  hard-stops the owner immediately (`client_worker.rs:343-345`), and the round's
  TerminalOutput frames are retained into client egress → parked in daemon
  `pending_drain` → unreachable on a bound-adapter route (bound routes never consume
  `drain_subscription`). A warm round shortens spawn→exit enough to land exit inside
  the attach/bind window.
- **C3 — retained-frame parking after owner removal.** After hard-stop removes the
  owner, later frames for that route are retained into client egress and parked in
  `pending_drain` (`daemon.rs` retention paths, including `shutdown_session`
  `daemon.rs:1689` and `read_screen` `daemon.rs:1142`); on a bound-adapter route no
  consumer ever takes them. This is the retention-without-a-reachable-flush class
  ([[retention without a reachable flush is data loss]]) across process generations —
  real, but secondary unless the trace shows the bytes actually took this branch.
- **C4 — parent stall/overflow drop.** `send_worker_event`
  (`worker_process.rs:2138-2160`) drops live PTY bytes to overflow when no stall owner
  is present and the channel is full. Low probability (default capacity, 4-byte
  payload) but cheap to rule out.

## Scope

In scope (Core only):

- Make pre-exit bytes accepted for a bound TerminalAdapter reach that adapter's
  consumer before the route closes, across repeated spawn/attach/exit/shutdown rounds
  on one daemon, with `ProcessExited` still delivered on payload arrival.
- A new Core bound-adapter multi-round regression test that is red at `d981bb0` and
  green after the fix.
- The smallest ordering fix in the traced surface: ClientWorker `pump_one` /
  `hard_stop` close ordering, `ingest_bound_terminal_frames` exit/window semantics, or
  the daemon retention paths — chosen by the differential trace.
- If the fix adds a delivery-completion clause to the adapter close ordering, update
  the Core-owned `TerminalAdapter` contract docs and the published conformance harness
  so trusted adapters are held to the proven behavior.

Out of scope (do not do):

- Any Hub edit. The ticket forbids weakening the Hub live-byte oracle; Hub repins after
  merge.
- Restoring the worker reap / exit-status delivery gate (`c23b833` contract holds).
- Removing or weakening the `FRAME_PROCESS_EXITED` writer barrier, the W1/W2 proofs, or
  the sibling-stays-live-during-reap proof (`dbb4cbc` contract holds).
- Blocking or threaded adapter close (human answer `question_1786670811_244393`
  requires non-blocking close; a bounded number of additional pump ticks before close
  is acceptable, a closer thread or blocking wait is not).
- New public API surface, speculative configurability, or adjacent refactors.

## Contract to preserve (from the ticket)

1. Bytes written before natural exit must still reach an already-attached subscriber
   after `ProcessExited` delivery and the `FRAME_PROCESS_EXITED` writer barrier.
2. Repeated spawn/attach/exit/shutdown rounds on one daemon must isolate: a later
   session must not observe empty live output while its producer wrote and exited.
3. Keep ProcessExited-on-payload delivery and the W1/W2 proofs. Do not restore the
   reap/exit-status gate.

## Ownership boundaries and cross-repo dependencies

- Core owns: worker frame order, `ProcessExited` delivery, the writer barrier, the
  ClientWorker bound-adapter egress (queues, pump, hard-stop close ordering), the
  `TerminalAdapter` contract and conformance harness, `pending_drain` retention/flush,
  and the subscription ownership lifecycle. All fix code lands in Core.
- Hub owns: host classification, adapter implementation forwarding, request cadence,
  and the repin. Hub main is pinned to Core `fd66efd`; after this ticket merges, Hub
  (parent Hub ticket `ticket_1786977409_499180`, finding `finding_1787034725_966366`)
  repins forward and reruns every live proof. Rolling Hub back to `fd66efd` is
  forbidden (reopens `finding_1787032735_978877`).
- If the trace proves the Hub WebRTC adapter violates an existing Core conformance
  requirement (rather than Core closing too early), do not silently edit Hub: record
  the finding, strengthen the Core conformance harness to expose it, and raise the Hub
  follow-up through the parent Hub ticket. The default position remains the ticket's:
  the fix is Core-side.
- No new dependency tickets: the Hub consumer work already exists as the parent run.
  This run uses a Hub worktree read-only plus a Cargo `[patch]` in a disposable copy
  for downstream proof.

## Assumptions and unknowns

- Assumption: the Hub WebRTC adapter's `pressure()` can report `Ready` when a write is
  accepted into a transport buffer that `close()` then abandons. The differential
  trace must confirm which side of the accept/flush line the lost bytes sit on.
- Assumption: Hub completes `bind_terminal_adapter` during Attach handling, so the C2
  window is short; the trace must confirm whether warm rounds ever land exit inside it.
- Unknown: the winning fix shape. Candidates, smallest first: (a) `pump_one` defers
  `hard_stop` after `process_exit_delivered` until the adapter's in-flight/accepted
  writes complete, bounded by the existing 512-tick `WRITE_ATTEMPT_BUDGET` so a stuck
  adapter still fails only that route; (b) `ingest_bound_terminal_frames` treats an
  exit-in-window (C2) by delivering queued bytes before the unbound-exit hard stop;
  (c) a retention-path fix (C3) that keeps bound-route frames reachable. The chosen
  fix must not delay `ProcessExited` payload delivery to the host control plane and
  must keep close non-blocking and same-tick-initiated once delivery completes.
- Unknown: exact per-round determinism of the flake. The regression test must force the
  fatal interleaving deterministically (bytes + exit in one ingest batch; adapter that
  distinguishes accepted from flushed), not rely on warm-timing luck.

## Runtime-teardown lenses ([[botster runtime teardown lenses]])

- `teardown_class_applies`: yes. The ticket is ProcessExited delivery, worker teardown
  and background reap, ClientWorker/adapter teardown ordering, and terminal-state vs
  live-runtime divergence (lifecycle says exited while live bytes are undelivered)
  across process generations on one daemon.
- `teardown_isolation`: the ownership set that dies at `ProcessExited` is one session's
  worker child (background reap), its parent session entry (channels, stall state,
  pending_output), its runtime handle, and — through ClientWorker hard-stop — the bound
  subscription owner (queue cleared, adapter closed and dropped). The daemon keeps the
  registry row, `retained_terminal`, and `pending_drain` entries keyed by that session
  id. One session's exit must not disturb sibling sessions' egress (existing
  sibling-live proof stays green) and must not disturb the next generation's session
  (this ticket's new proof).
- `teardown_bounds`: background reap is bounded (`WORKER_REAP_GRACE` 2 s then kill);
  `shutdown_session` is bounded (2 s deadline, then retain + typed error); the worker
  writer thread terminates at the exit frame; adapter close is non-blocking and a
  hanging close is an adapter defect; adapter write stalls fail the single route after
  the 512-tick write budget. The fix must not add unbounded waits — no
  wait-for-reader-EOF, no wait-for-reap, no unbounded wait-for-flush; any
  delivery-completion deferral of close is bounded by the existing write budget.
- `late_message_matrix` (ownership-creating or state-touching messages after exit):
  - `Attach` after exit: owner is (session, subscription, generation) per
    [[Core terminal subscription ownership is session, subscription, and generation]];
    generation assigned at attach; rejected for exited sessions; reused
    subscription_id gets an incremented generation so a delayed Closed/detach for
    generation N cannot delete generation N+1.
  - `BindTerminalAdapter` after exit or teardown: bind creates durable adapter
    ownership on the live owner row and requires the current live attach generation
    per [[Core ClientWorker bind requires a live attach generation]] — a bind before
    attach returns `BindBeforeAttach`, a stale generation returns `StaleGeneration`,
    and neither detach nor adapter-Closed handling can recreate ownership after
    teardown. Residual cleanup: a bind presented to a missing, mismatched, or
    already-bound owner closes and drops the presented adapter on that stack
    (`client_worker.rs:185-231`). Post-exit, the owner row is already hard-stopped, so
    late binds fail closed without recreating the route. The regression test must
    cover the attach→bind window against an exit racing in (C2).
  - `PtyInput`/`Resize` after removal: rejected with `SessionNotFound`-class errors;
    no ownership created.
  - Terminal frames after `process_exit_enqueued` on a bound route: dropped by
    design (`client_worker.rs:349-351`, writer-barrier semantics); frames for an
    unbound or removed owner are retained into client egress and then parked in
    `pending_drain` — the fix must ensure bound-route bytes never take that
    unreachable branch while the route is live (C2/C3).
  - `Drain`/`DrainSubscription` after exit and removal: stay admissible for unbound
    routes; must flush `pending_drain` first; tolerated `SessionNotFound` covers the
    runtime side. Bound routes never receive terminal frames here by contract.
  - `ReadScreen`/`ReadModeFlags` after exit: served from `retained_terminal`; their
    internal drains run `apply_client_worker`, so they are production pump ticks —
    the regression test uses them as the driving tick.
  - `ShutdownSession` after exit: idempotent exact-session classification
    (`Found`/`Absent`); its retained `shutdown_drain` must obey the same
    reachable-flush rule for unbound routes.
  - `RemoveSession`: terminal-only; deliberately drops `pending_drain` as explicit
    host retention policy; the live-byte assertion completes before shutdown/removal.
- `production_path_proof`: child exits → worker runtime drain emits `ProcessExited` →
  writer barrier flushes queued metadata then the exit frame and stops → parent stdout
  reader stores the payload → `drain_output` emits bytes then `ProcessExited` and
  removes the session → `route_runtime_outputs` → `apply_client_worker`:
  `ingest_bound_terminal_frames` enqueues `[bytes, process_exit]` on the bound owner →
  `pump_one` writes bytes, then process_exit, then hard-stop close → Hub WebRTC
  adapter → subscriber. Oracles: (1) new Core bound-adapter multi-round daemon test —
  spawn, attach, bind a test `TerminalAdapter`, release a finite producer, drive
  `ReadScreen` as the production tick, assert the exact bytes reach the adapter sink
  **before** the process_exit frame and **before** `close()` in every round — red at
  `d981bb0` (recorded), green with the fix, red again on fix revert; (2) live Hub
  proof: Hub worktree copy at `57cbeb2` with Cargo `[patch]` to this Core worktree
  running `./test.sh --locked --test hub_daemon_lifecycle_test
  external_hub_webrtc_shutdown_after_live_exit_is_idempotent_cleanup -- --exact` at
  least 4 consecutive times, all green.
- `ownership_identity`: session ids are unique per round; subscription identity is
  (session, subscription, generation); bind stores one immutable negotiated capability
  set on the accepted generation; `pending_drain` entries are keyed by session id;
  stall owners rebuild from live subscription inventory. Stale entries or delayed
  Closed events from round N must never match round N+1 ids, and the fix must not
  introduce any cross-generation scalar state.
- `sibling_fail_closed_policy`: on successful exit delivery, sibling sessions, sibling
  subscriptions, and later generations keep full egress (tested, including the
  existing sibling-live-during-reap proof). A stuck adapter fails only its own route
  (512-tick budget). On a flush-path error, retain the data and propagate the error —
  never consume-and-drop ([[retention without a reachable flush is data loss]]). No
  silent sibling sacrifice exists on this path.

## Affected surfaces/files

- `crates/botster-core/src/engine/client_worker.rs` — `pump_one` exit-delivery vs
  hard-stop close ordering, `ingest_bound_terminal_frames` exit/window semantics
  (primary fix surface for C1/C2).
- `crates/botster-core/src/engine/managed_session_runtime.rs` — `apply_client_worker`
  call sites only if the traced fix needs an extra bounded pump before teardown.
- `crates/botster-core-daemon/src/daemon.rs` — retention paths only if C3 is traced as
  load-bearing.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — new bound-adapter
  multi-round regression test (reuse the existing test-adapter and test-only worker
  hooks: `test_hold_before_exit_ms`, `test_exit_code`; extend the test adapter to
  distinguish accepted from flushed writes and to record write/close ordering).
- `crates/botster-core/tests/client_worker_engine_test.rs` — unit-level lock on the
  pump ordering (bytes before exit before close; no close with accepted-undelivered
  frames within budget).
- `crates/botster-core-test-support/tests/terminal_adapter_conformance_test.rs` and the
  published conformance harness — only if the fix adds a delivery-completion clause to
  the close ordering contract.
- `docs/architecture/client-worker-terminal-egress.md` / `terminal-adapter.md` —
  contract wording if the close-ordering obligation changes.
- `crates/botster-core/tests/local_session_worker_process_test.rs` — only for C4.

## Implementation steps

1. **Reproduce in Core first, on the bound-adapter path.** Add a daemon integration
   test that models the Hub sequence on one `CoreDaemon` across ≥3 rounds: spawn a
   worker-backed session, attach, `bind_terminal_adapter` with a test adapter whose
   sink records (frame, accepted-vs-flushed, close ordering), release a finite
   producer that writes known bytes and exits, drive repeated `ReadScreen` calls as
   the production tick, and assert per round that the exact bytes are observable at
   the adapter sink before the process_exit frame and before close. Force the fatal
   interleaving (bytes + exit in one ingest batch) via the existing worker test hooks.
   Confirm red at `d981bb0` and record the failure.
2. **Differential trace.** Instrument the failing round to name the exact loss branch:
   C1 same-tick close after accepted writes, C2 attach/bind-window unbound-exit hard
   stop, C3 retained-frame parking, or C4 channel drop. Record the answer in the
   implementation artifact and rule the others out with ablation or trace evidence.
3. **Fix.** Apply the smallest change on the traced branch, preserving:
   ProcessExited-on-payload, writer barrier, non-blocking bounded close, hard-stop on
   the host tick once delivery completes, lifecycle-without-Drain progress, and
   subscription-scoped drain retention semantics for unbound routes.
4. **Prove.** Run the acceptance checks below, including the live Hub downstream proof
   and a red-on-revert control for the new test. Keep an unbound `DrainSubscription`
   multi-round test only if the trace proves a separate retained-egress defect (C3).

## Acceptance checks/tests

1. New bound-adapter multi-round Core regression test: red at `d981bb0` (recorded),
   green with fix, red on fix revert.
2. Existing proofs stay green: `shutdown_delivers_process_exited_during_worker_hold_before_exit`,
   `shutdown_delivers_process_exited_when_worker_exits_nonzero`,
   `observe_then_drain_still_delivers_terminal_process_exited`,
   `observe_session_lifecycle_reconciles_parked_process_exited`, the writer-barrier
   worker tests, the sibling-live-during-reap proof, the ClientWorker engine tests,
   and the terminal adapter conformance suite.
3. Repository gates ([[botster-core uses CI-owned Cargo commands because it has no test
   script]]), after initializing the required Ghostty submodule:
   - `BOTSTER_ENV=test cargo test --workspace`
   - `cargo fmt --all -- --check`
   - `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings`
   - `BOTSTER_ENV=test cargo test --doc --workspace`
4. Downstream live proof (charter-required, hub-shaped): in a disposable copy of the
   Hub worktree at `57cbeb2` (do not modify the real Hub checkout), add a Cargo
   `[patch]` pointing the five Core git dependencies at this Core worktree, then run
   `./test.sh --locked --test hub_daemon_lifecycle_test
   external_hub_webrtc_shutdown_after_live_exit_is_idempotent_cleanup -- --exact`
   at least 4 consecutive times, all green. Record the runs. This is live proof, not
   soft residual.
5. If the close-ordering contract gains a delivery-completion clause: conformance
   harness proves it, and the Core fake adapter plus published harness still prove
   close is prompt and idempotent.
6. No Hub source changes anywhere in evidence; the live-byte oracle is untouched.

## Risks

- The fix could reintroduce the class `c23b833` fixed (registry rows stuck `Running`,
  shutdown waiting on reap). Mitigation: keep the existing hold/exit-code tests green;
  no new waits on the ProcessExited payload delivery path.
- Deferring close until accepted writes complete could hang a route on a broken
  adapter. Mitigation: bound the deferral with the existing 512-tick write budget so a
  stuck adapter still fails only that subscription.
- A close-ordering contract change could break other trusted adapters (TUI, Unix mux).
  Mitigation: run the full conformance suite; keep close non-blocking and same-tick
  initiated once delivery completes.
- Fixing the C2 window could recreate ownership after teardown. Mitigation: keep
  [[Core ClientWorker bind requires a live attach generation]] rejections intact;
  never buffer frames for an owner that no longer exists.
- The regression test could be timing-flaky like the Hub test. Mitigation: force the
  interleaving through explicit call order and worker test hooks (daemon calls are
  synchronous on one thread), not sleeps.
- Workspace-load flake in `plugin worker unload deadline` tests is diagnostic guidance,
  not a waiver — rerun, do not excuse.

## Vault gaps worth capturing

- The reachable-flush obligation extends to bound adapters: "accepted by the adapter"
  is not "delivered to the consumer", and a same-tick close after the final accepted
  write can destroy the only flush path. Candidate note alongside [[retention without
  a reachable flush is data loss]] and [[Core subscription hard-stop is synchronous
  close and drop on the host tick]].
- The `fd66efd → d981bb0` lesson: removing a delivery gate can silently remove an
  ordering guarantee (bytes flushed ticks before exit observation) that downstream
  transports depended on. Candidate note: "delivery gates can hide ordering contracts;
  removing one needs an explicit replacement ordering proof."
