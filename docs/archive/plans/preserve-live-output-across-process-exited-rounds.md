# Plan: preserve live output across repeated ProcessExited writer-barrier rounds

- Ticket: `ticket_1787034922_646556`
- Run: `run_1787034943_217745`
- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Base: `d981bb0` (Core main with `c23b833` ProcessExited-on-payload and `dbb4cbc` writer barrier)

## Problem

Hub test `external_hub_webrtc_shutdown_after_live_exit_is_idempotent_cleanup` runs five
spawn → attach → write → natural-exit → shutdown rounds on one daemon. At Core `fd66efd`
the test passes. At Core `d981bb0` round 0 passes and later rounds panic at
`webrtc_proofs.rs:581` with empty live bytes: the producer wrote 4 bytes and exited, and
the already-attached WebRTC subscriber received nothing.

The Hub test function is byte-identical to Hub `origin/main`. Hub ReadScreen and WebRTC
forwarding did not change. The regression is Core-side, introduced by the combination of:

1. `c23b833`: `WorkerProcessRuntime::drain_output` now returns the `ProcessExited`
   payload as soon as the worker frame arrives, re-pumps once, and removes the runtime
   session in the same call (`crates/botster-core/src/runtime/worker_process.rs:1350-1375`).
   The old reader-EOF + reap + zero-status gate is gone (intentionally; do not restore it).
2. `dbb4cbc`: the worker writer thread treats `FRAME_PROCESS_EXITED` as a terminal
   barrier and stops after writing it
   (`crates/botster-core-daemon/src/bin/botster-session-worker.rs`, `write_egress_lanes`).

## Diagnosis (evidence-backed hypothesis chain)

Single-round frame order is correct: `drain_output` pushes pending PTY bytes into the
output batch before the `ProcessExited` event, and the worker's single reader thread
stores `process_exited` only after every earlier frame's `send_worker_event` returned.
The W1/W2 proofs cover that ordering.

What changed observably: at `fd66efd` the delivery gate delayed `ProcessExited` (and the
`Exited` lifecycle transition) until worker reap, so the live bytes always reached the
attached subscriber in an earlier drain tick than exit observation. At `d981bb0` a warm
round completes write + exit fast enough that the bytes and `ProcessExited` arrive in
**one** runtime batch, and the first Core call that drains that batch decides where the
bytes go:

- The Hub test polls `ReadScreen` every 30 ms. `CoreDaemon::read_screen`
  (`crates/botster-core-daemon/src/daemon.rs:1120-1147`) internally drains the runtime
  and parks client egress into `pending_drain` "for the next explicit drain".
- Hub request turns run `observe_lifecycle_turn`, whose per-session runtime drain can
  also consume the batch and advance lifecycle to `Exited`.
- Once lifecycle reads `Exited`, the Hub host observes exit on the control plane and its
  subscription/maintenance machinery stops or reorders the terminal drain for that
  route. The parked bytes in `pending_drain` then have no reachable flush: nothing on
  the Hub side is obligated to issue another `Drain` for a session it already observed
  as exited, and `CoreDaemon::shutdown_session` (`daemon.rs:1689`) parks its own final
  `shutdown_drain` the same way.

This is the retention-without-a-reachable-flush class ([[retention without a reachable
flush is data loss]]) across process generations: retention is fine only while the later
flush stays reachable in every state the first call leaves behind. Early `ProcessExited`
delivery created a state (exited-and-batch-parked) in which the flush never runs.

Round 0 passes because cold-start latency (worker binary spawn, python startup) lets a
Hub drain tick deliver the bytes before exit lands; warm rounds collapse the window.
The observed flake pattern (4/4 red in review, 1 pass then 3/4 red in Implement) matches
a timing race, not a deterministic per-round counter.

Secondary candidate mechanisms the Implementer must check and either fix or rule out
with evidence (ablation or trace, per [[ablate dont argue to prove a branch is
unexercised]]):

- **S1 — drain reachability after removal**: `CoreDaemon::drain` (`daemon.rs:1047`)
  calls `ensure_session` before `take_pending_drain`. Confirm `engine.session` still
  returns the exited row after `drain_output` removed the runtime session, in every
  state the failing sequence produces. If any state makes `ensure_session` or the
  tolerated-`SessionNotFound` branch unreachable, retained frames leak.
- **S2 — subscription-close ordering**: Core subscription hard-stop closes the route on
  the host tick that carries `ProcessExited`. Retained byte frames for that same
  (session, subscription) must be delivered by `drain_subscription` before or together
  with the close, not filtered out after the route is gone.
- **S3 — wake loss**: request-path internal drains (read_screen, read_mode_flags,
  observe) consume the runtime batch without signaling hosts that subscription egress
  is now pending. If Hub's drain cadence is wake-driven, parked frames wait forever.
- **S4 — parent stall/overflow drop**: `send_worker_event`
  (`worker_process.rs:2138-2160`) drops live PTY bytes to overflow when no stall owner
  is present and the channel is full. Confirm round-N stall ownership is registered
  before the producer writes; a cross-round ownership gap would silently drop bytes.
  This is a lower-probability candidate (default channel capacity, 4-byte payload) but
  it is cheap to rule out.

## Scope

In scope (Core only):

- Make retained pre-exit client egress for an already-attached subscription reach that
  subscriber after `ProcessExited` delivery, across repeated spawn/attach/exit/shutdown
  rounds on one daemon.
- A new Core-shaped multi-round regression test that is red at `d981bb0` and green after
  the fix.
- The smallest ordering/reachability fix in `CoreDaemon`
  (`drain`/`drain_subscription`/`read_screen`/`observe`/`shutdown_session` retention
  paths) and/or `WorkerProcessRuntime::drain_output`, chosen by the differential trace.

Out of scope (do not do):

- Any Hub edit. The ticket forbids weakening the Hub live-byte oracle; Hub repins after
  merge.
- Restoring the worker reap / exit-status delivery gate (`c23b833` contract holds).
- Removing or weakening the `FRAME_PROCESS_EXITED` writer barrier, the W1/W2 proofs, or
  the sibling-stays-live-during-reap proof (`dbb4cbc` contract holds).
- New public API surface, speculative configurability, or adjacent refactors.

## Contract to preserve (from the ticket)

1. Bytes written before natural exit must still reach an already-attached subscriber
   after `ProcessExited` delivery and the `FRAME_PROCESS_EXITED` writer barrier.
2. Repeated spawn/attach/exit/shutdown rounds on one daemon must isolate: a later
   session must not observe empty live output while its producer wrote and exited.
3. Keep ProcessExited-on-payload delivery and the W1/W2 proofs. Do not restore the
   reap/exit-status gate.

## Ownership boundaries and cross-repo dependencies

- Core owns: worker frame order, `ProcessExited` delivery, the writer barrier,
  `pending_drain` retention/flush after `read_screen`, subscription-scoped drains, and
  the ClientWorker subscription lifecycle. All fix code lands in Core.
- Hub owns: host classification, adapter forwarding, drain cadence, and the repin. Hub
  main is pinned to Core `fd66efd`; after this ticket merges, Hub (parent Hub ticket
  `ticket_1786977409_499180`, finding `finding_1787034725_966366`) repins forward and
  reruns every live proof. Rolling Hub back to `fd66efd` is forbidden (reopens
  `finding_1787032735_978877`).
- No new dependency tickets: the Hub consumer work already exists as the parent run.
  This run must not edit Hub; it only uses a Hub worktree read-only + Cargo `[patch]`
  for downstream proof.

## Assumptions and unknowns

- Assumption: `engine.session` keeps returning an `Exited` row after the runtime session
  is removed by `drain_output` (registry-backed), so `drain` stays callable. The
  Implementer verifies this (S1) before relying on it.
- Assumption: the Hub test's 120 × 30 ms `ReadScreen` poll is the only subscriber-side
  driver during the assertion window; Hub's own maintenance drain cadence is
  wake/turn-driven. The differential trace confirms which Core call consumes the fatal
  batch.
- Unknown: whether the winning fix is (a) flush-on-close for subscription-scoped
  retained frames, (b) internal drains fanning egress to attached subscriptions
  eagerly instead of parking, (c) a host-visible pending-egress signal, or (d) ordering
  the lifecycle `Exited` observation after retained egress for attached routes is
  taken. The Implementer picks the smallest one that makes the repro green without
  violating the preserved contracts; (b)/(d) style changes must not delay lifecycle
  truth for hosts with no attached subscriber (keep [[Core control-plane lifecycle
  journal advances without a terminal client or Hub terminal Drain]] intact).
- Unknown: exact per-round determinism of the flake. The regression test must force the
  race deterministically (single batch: bytes + exit consumed by a readback drain
  first), not rely on warm-timing luck.

## Runtime-teardown lenses ([[botster runtime teardown lenses]])

- `teardown_class_applies`: yes. The ticket is ProcessExited delivery, worker teardown
  and background reap, and terminal-state vs live-runtime divergence (lifecycle says
  exited while live bytes are undelivered) across process generations on one daemon.
- `teardown_isolation`: the ownership set that dies at `ProcessExited` is one session's
  worker child (background reap), its parent session entry (channels, stall state,
  pending_output), and its runtime handle. The daemon keeps the registry row,
  `retained_terminal`, and `pending_drain` entries keyed by that session id. One
  session's exit must not disturb sibling sessions' egress (existing sibling-live proof
  stays green) and must not disturb the next generation's session (this ticket's new
  proof).
- `teardown_bounds`: background reap is bounded (`WORKER_REAP_GRACE` 2 s then kill);
  `shutdown_session` is bounded (2 s deadline, then retain + typed error); the worker
  writer thread terminates at the exit frame. The fix must not add unbounded waits —
  in particular it must not reintroduce a wait-for-reader-EOF or wait-for-reap gate on
  the delivery path.
- `late_message_matrix` (ownership-creating or state-touching messages after exit):
  - `Attach` after exit: owner is (session, subscription, generation); rejected because
    bind requires a live attach generation; no residual state.
  - `PtyInput`/`Resize` after removal: rejected with `SessionNotFound`-class errors; no
    ownership created.
  - `Drain`/`DrainSubscription` after exit and removal: MUST stay admissible and must
    flush `pending_drain` first; tolerated `SessionNotFound` covers the runtime side.
    This is the fix surface; the new test locks it.
  - `ReadScreen`/`ReadModeFlags` after exit: served from `retained_terminal`; their
    internal-drain retention must never strand subscription egress.
  - `ShutdownSession` after exit: idempotent exact-session classification
    (`Found`/`Absent`); its retained `shutdown_drain` must obey the same reachable-flush
    rule.
  - `RemoveSession`: terminal-only; deliberately drops `pending_drain` — allowed
    because removal is explicit host retention policy, and the Hub test asserts bytes
    before shutdown/removal.
- `production_path_proof`: child exits → worker runtime drain emits `ProcessExited` →
  writer barrier flushes queued metadata then the exit frame and stops → parent stdout
  reader stores the payload → `drain_output` emits bytes then `ProcessExited` and
  removes the session → engine fanout / daemon retention → `drain_subscription` →
  Hub adapter → WebRTC subscriber. Oracles: (1) new Core multi-round daemon test
  asserting each round's subscriber egress contains the pre-exit bytes, red at
  `d981bb0`, green with the fix, red again on fix revert; (2) live Hub proof: Hub
  worktree with Cargo `[patch]` to this Core worktree running the exact failing test
  repeatedly (≥4 consecutive green runs, since the failure was 3-of-4).
- `ownership_identity`: session ids are unique per round; subscription identity is
  (session, subscription, generation); `pending_drain` entries are keyed by session id;
  stall owners rebuild from live subscription inventory. Stale entries from round N
  must never match round N+1 ids, and the fix must not introduce any cross-generation
  scalar state.
- `sibling_fail_closed_policy`: on successful exit delivery, sibling sessions and later
  generations keep full egress (tested). On a flush-path error, retain the data and
  propagate the error — never consume-and-drop (the invariant from [[retention without
  a reachable flush is data loss]]). No silent sibling sacrifice exists on this path.

## Affected surfaces/files

- `crates/botster-core-daemon/src/daemon.rs` — `drain`, `drain_subscription`,
  `read_screen`/`read_mode_flags` retention, `observe` drain, `shutdown_session`
  retention, `take_pending_drain`/`retain_pending_drain_result` (primary fix surface).
- `crates/botster-core/src/runtime/worker_process.rs` — `drain_output` exit-path
  ordering, only if the trace shows parent-side loss (S4) or removal-order issues.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — new multi-round
  regression test plus any deterministic race hooks (reuse the existing test-only
  config style: `test_hold_before_exit_ms`, `test_exit_code`; add a hook only if the
  race cannot be forced with call ordering alone).
- `crates/botster-core/tests/local_session_worker_process_test.rs` — only if a
  parent-runtime-level assertion is needed for S4.
- `docs/architecture/` — update the drain-contract wording if the fix changes the
  documented retention/flush obligation
  (`docs/architecture/client-worker-terminal-egress.md` or the durable-session-worker
  protocol doc, whichever states the drain contract).

## Implementation steps

1. **Reproduce in Core first.** Add a daemon integration test that models the Hub
   sequence on one `CoreDaemon` across ≥3 rounds: spawn worker-backed session, attach a
   subscription, then force the fatal interleaving — producer writes and exits, the
   first post-exit consumer is a readback (`read_screen`) or observe drain, and only
   afterwards does the subscription drain run. Assert each round's
   `drain_subscription` egress contains the pre-exit bytes. Confirm red at `d981bb0`.
2. **Differential trace.** Instrument or step the failing round to name the exact
   consumer of the fatal batch and the exact state that strands `pending_drain`
   (S1–S4). Record the answer in the implementation artifact.
3. **Fix.** Apply the smallest change that restores reachable flush for attached-route
   egress across generations, preserving: ProcessExited-on-payload, writer barrier,
   bounded teardown, lifecycle-without-Drain progress, and subscription-scoped drain
   retention semantics ([[attach routes use subscription scoped Core drains]]).
4. **Prove.** Run the acceptance checks below, including the live Hub downstream proof
   and a red-on-revert control for the new test.

## Acceptance checks/tests

1. New multi-round Core regression test: red at `d981bb0` (recorded), green with fix,
   red on fix revert.
2. Existing proofs stay green: `shutdown_delivers_process_exited_during_worker_hold_before_exit`,
   `shutdown_delivers_process_exited_when_worker_exits_nonzero`,
   `observe_then_drain_still_delivers_terminal_process_exited`,
   `observe_session_lifecycle_reconciles_parked_process_exited`, the writer-barrier
   worker tests, and the sibling-live-during-reap proof.
3. Repository gates ([[botster-core uses CI-owned Cargo commands because it has no test
   script]]):
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
5. No Hub source changes anywhere in evidence; the live-byte oracle is untouched.

## Risks

- The fix could reintroduce the class `c23b833` fixed (registry rows stuck `Running`,
  shutdown waiting on reap). Mitigation: keep the existing hold/exit-code tests green;
  no new waits on the delivery path.
- Flush-on-close or eager-fanout changes could deliver foreign-route frames or break
  [[attach routes use subscription scoped Core drains]]. Mitigation: keep
  subscription-route filtering; extend the scoped-drain tests if the flush point moves.
- Delaying `Exited` observation (option d) could regress no-client lifecycle progress.
  Mitigation: gate any ordering change on the presence of attached subscriptions and
  keep the lifecycle-without-Drain tests green.
- The regression test could be timing-flaky like the Hub test. Mitigation: force the
  interleaving through explicit call order (daemon calls are synchronous on one
  thread), not sleeps; use existing test-only worker hooks for exit timing.
- Workspace-load flake in `plugin worker unload deadline` tests is diagnostic guidance,
  not a waiver — rerun, do not excuse.

## Vault gaps worth capturing

- The reachable-flush obligation extends across process generations and across the
  lifecycle-observation boundary: "a host that observes `Exited` may never drain
  again" belongs next to [[retention without a reachable flush is data loss]] as its
  cross-generation corollary, with this ticket as the source.
- The `fd66efd → d981bb0` lesson: removing a delivery gate can silently remove an
  ordering guarantee (bytes-before-exit-observation) that downstream cadence depended
  on. Candidate note: "delivery gates can hide ordering contracts; removing one needs
  an explicit replacement ordering proof."
