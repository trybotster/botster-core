# Core: apply targeted duplex input during pump_woken

Ticket: `ticket_1788128130_441301`
Run: `run_1788128178_478344`
Revision: 3 (revised after Plan Review `review_1788129045_539289` and `review_1788129843_885975`)
Target repository: `botster-core` (`trybotster/botster-core`)
Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Base: `main` at `3672c66` ("Rearm wake after obligation read errors")

## Problem

Revision `3672c667` made the wake-driven data plane targeted, but it left the Core-owned
input transition incomplete.

`ManagedSessionRuntime::pump_woken` (`crates/botster-core/src/engine/managed_session_runtime.rs:1305`)
runs only Stage A for named adapter routes:

- `self.client_worker.intake_woken(batch)` reads adapter bytes and pushes decoded
  `TerminalInputCommand` values into `owner.input_queue`.
- No caller then runs Stage B (`ClientWorker::take_terminal_input`) or an apply stage.

`CoreDaemon::pump_woken` (`crates/botster-core-daemon/src/daemon.rs:1092`) calls only
`DaemonEngine::pump_woken`. The session-scoped `DaemonEngine::apply_terminal_input`
(`daemon.rs:3627`) runs only from `CoreDaemon::drain` (`daemon.rs:1341`). A host that drives
the runtime with `wait_wakes` + `pump_woken` never reaches the apply stage, so an accepted
`TerminalInputFrame` stays in `input_queue` forever and never reaches the PTY.

That is the exact Hub failure named in the ticket: the Hub test receives initial output,
sends a valid binary `TerminalInputFrame`, observes successful targeted pumps, and never
sees the PTY echo.

## Wake classes (corrected after Plan Review)

`TerminalWakeBatch` carries two different wake classes, and they are not interchangeable.
[[core terminal progress is wake driven and targeted]] and the wake-driven data-plane
capture define them:

- `adapter_routes`: one exact route (`session_id`, `subscription_id`) has
  adapter work — readable client bytes, returned write capacity, or closure. Client input
  exists only on this class.
  A batch route entry carries no generation. `bind_route` keeps the generation inside
  `RouteWakeState`, so generation identity comes from the live owner, never from the batch.
- `ingress_sessions`: one session has new worker or PTY output. This class carries no client
  input.

Therefore **client input intake and input apply are scoped to `adapter_routes` only**.
Session ingress continues to drive runtime-output drain and adapter egress pumping, exactly
as `3672c667` shipped. Revision 1 of this plan merged the two classes for Stage B; that was
wrong, and this revision removes it.

## Playbooks and notes loaded

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-architecture]]
- [[botster-core-playbook]] (repository charter for the resolved target)
- [[botster runtime teardown lenses]] (runtime-teardown class applies; see below)
- Botster architecture capture: `ops/archive/inbox/2026-08-27-botster-wake-driven-data-plane-and-hub-decomposition.md`
- [[core owns duplex terminal transport while Hub stays content blind]]
- [[core terminal progress is wake driven and targeted]]
- [[terminal adapters emit coalesced writable and closed wakes]]
- [[session wake coalescing belongs in a lifecycle registry not each handle]]
- [[an overflow reconcile walk must reuse the readiness filter it backstops]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core queues concurrent attaches and serializes snapshot encoding]]
- [[core holds declared attach frames until the bound adapter drains]]
- [[a global owner walk needs a global per session gated hold]]
- [[a batch dequeue loop must update its exclusion set as it grants work]]
- [[every TerminalInputResult must stamp the live subscription id]]
- [[adapter accepted writes are not consumer flushed writes]]
- [[botster core contract surface needs consumer proof]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster-core CI runs a contract only test lane because workspace feature unification hides breaks]]
- [[spawned Hub tests can reach only four of fourteen Core test builders]]
- [[bound attach suppression skip is unproven without a red oracle]]
- [[vault example paths are not repository placement conventions]]

## Context loaded

- `crates/botster-core/src/engine/managed_session_runtime.rs`
  (`pump_woken:1305`, `apply_terminal_input:263` and `:535`, `handle_client_ingress:715`,
  `flush_runtime_inputs:813`-shaped global walk, `flush_runtime_inputs_for_session`,
  `apply_client_worker_with`, `prepend_inputs:1875`, `drain_inputs:1871`)
- `crates/botster-core/src/engine/client_worker.rs`
  (`intake_woken:744`, `intake_terminal_input_keys`, `take_terminal_input`, `pump_woken:690`,
  `sessions_awaiting_gated`, `enqueue_input_result:922`)
- `crates/botster-core/src/engine/multiplexer.rs` (`handle_client_ingress:366`, session-scoped)
- `crates/botster-core/src/engine/botster.rs`
  (`apply_terminal_input:687` local, `apply_terminal_input:1424` worker with the
  incremental-attach gate)
- `crates/botster-core/src/contract/terminal_wake.rs`
  (`TerminalWakeSink::wake:123`, `notify_session:353`, `bind_route:379`, `retire_route:409`,
  route registry keyed by `(SessionId, SubscriptionId)`)
- `crates/botster-core/src/runtime/worker_process.rs`
  (`send_input:1522`, `admit_encoded:1840` with `control queue full` and
  `control plane sealed`)
- `crates/botster-core-daemon/src/daemon.rs` (`CoreDaemon::pump_woken:1092`, `drain:1333`,
  `DaemonEngine:469`)
- `crates/botster-core-daemon/tests/terminal_wake_test.rs`,
  `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `botster-hub` `Cargo.toml`, `crates/botster-hub-test-support/Cargo.toml`,
  `crates/botster-hub-client/Cargo.toml`, `Cargo.lock` (read only, for the pin procedure)
- `docs/README.md`, `docs/plans/README.md`, `.github/workflows/ci.yml`

## Defects the plan must not reproduce (verified in the current base)

1. `ManagedSessionRuntime::handle_client_ingress:758` calls `self.flush_runtime_inputs()`,
   which walks every engine session, and then `apply_client_worker_with`, which calls the
   global `ClientWorker::pump()`. Reusing that helper inside the pump would add both a global
   scan and a second global pump. The woken path must not call it.
2. `flush_runtime_inputs_for_session` drains all inputs, sends `input.clone()`, and on failure
   prepends only the *remaining* iterator. The failed input is dropped. A transient
   `control queue full` therefore loses an accepted frame today.
3. `WorkerBackedBotsterEngine::apply_terminal_input:1424` defers to
   `prepare_terminal_input(session_id)` while `incremental_attaches` holds the session. A new
   apply path that skips this gate would push input and resize past the attach barrier.

## Runtime-teardown class answers

`teardown_class_applies`: **yes**. The change touches ClientWorker route ownership, per-route
generation fencing, hard-stop teardown on decode or apply failure, and terminal-state versus
live-runtime divergence between a queued frame and the PTY.

`teardown_isolation`: The ownership set is one `OwnerKey { session_id, subscription_id }` with
its `generation`. An apply failure hard-stops only that owner through `owner_apply_teardown`
and `detach_live`, and emits one `UnsubscribeSession` ingress. Sibling routes on the same
session and all other sessions keep running.

`teardown_bounds`: The apply stays bounded and non-blocking. Stage B keeps
`APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK` per owner per tick, `INPUT_QUEUE_CAPACITY` bounds
the queue, and the gated path parks the owner with the existing
`DEFAULT_MODE_GATED_INPUT_TIMEOUT` deadline instead of blocking. Retry follows one coalesced capacity
wake from the control writer, never a self-rearm, so a persistently full queue produces no
repeated pump cycle. No sleep, wait, or `block_on` enters the pump.

`late_message_matrix`:

| Command | Owner tag | Rejection after terminal failure | Residual sweep |
|---|---|---|---|
| `Input` | route key + owner `generation` + `client_id` | a full ordinary control queue parks the owner before dequeue and loses nothing; sealed or failed control state returns `owner_apply_teardown` for that route only | teardown routes through `unsubscribe_owner_teardowns` in the same `pump_woken`; the parked entry clears on dequeue, teardown, or generation replacement |
| `ModeGatedInput` | route key + `generation` + `awaiting_gated` park state | session held by `sessions_holding_gated` plus `sessions_awaiting_gated`; a full ordinary control queue parks the owner with the frame still queued and never hard-stops; `control plane sealed` hard-stops the owner; `already in flight` returns `SessionNotWritable` | `cancel_mode_gated_pty_input` on teardown; `clear_awaiting_gated` on Ready or TimedOut; the parked entry clears on dequeue, teardown, or generation replacement |
| `Resize` | route key + `generation` + `client_id` | same parking and failure rules as `Input` | same tick teardown routing and the same parked-entry clearing |
| Retired or replaced route | route removed by `retire_route`, or the live owner now holds a newer generation | `live.get_mut` misses, or the parked generation no longer matches, so Stage B skips the key | the stale parked entry is dropped, never retried; no queue grows |
| Session-ingress wake | carries no client input | Stage B never runs for this class | not applicable |

`production_path_proof`: `TerminalWakeSource::wait_wakes` -> host loop ->
`CoreDaemon::pump_woken` -> `DaemonEngine::pump_woken` (Stage A `intake_woken`) ->
`DaemonEngine::apply_woken_terminal_input` -> route-scoped Stage B under the capacity predicate ->
`MultiplexerEngine::handle_client_ingress` or `submit_mode_gated_pty_input` ->
`flush_runtime_inputs_for_session` -> PTY write. Proof drives `CoreDaemon::pump_woken`
itself, never a direct `apply_terminal_input` call and never a ClientWorker helper call. The
retry path is: control writer pops -> session capacity wake -> `wait_wakes` -> the same
`CoreDaemon::pump_woken` entry -> parked owner whose generation still matches.

`ownership_identity`: Each delivery carries `client_id`, `session_id`, `subscription_id`, and
`generation` from the live owner, so a reused subscription id cannot inherit a stale delivery.
Stage B reads only the current `live` map. Each parked entry carries the owner generation and is
dropped when the live owner's generation differs, so a replacement owner can never inherit a
parked retry. `TerminalInputResult` keeps `with_subscription`.

`sibling_fail_closed_policy`: On success siblings keep working. On owner apply failure only
the failing owner hard-stops. On session control-plane failure the existing `teardown_session`
sweep removes every route of that one session; other sessions survive.

## Backpressure design (revision 3)

Revision 2 planned to retain a failed input inside `SessionRuntimeWorkerAdapter.inputs` and to
rearm the route immediately. Plan Review showed three defects in that design, and the code
confirms all three:

- The retained input had no retry path that runs without a new Stage B delivery, and the worker
  input queue carries no route identity.
- `ControlQueue::pop` (`control_queue.rs:150`) frees an ordinary slot and emits no wake, and
  `run_control_writer` (`worker_process.rs:2738`) is its only consumer. An immediate self-rearm
  after a full result is therefore a polling loop, not a capacity signal.
- The `ModeGatedInput` arm still hard-stopped on a full control queue after Stage B had already
  removed the frame, which loses an accepted frame.

Revision 3 replaces that design with three mechanisms.

**A. Do not dequeue what the control plane cannot accept.** `ControlQueue::admit`
(`control_queue.rs:110`) rejects an ordinary frame at
`WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS`. Add a bounded
`ControlQueue::can_admit_ordinary()` under the same lock, and expose it on the worker runtime
as a per-session query. Woken Stage B asks before it dequeues. Without capacity it does not
pop the command, does not call the engine, and does not enqueue an `input_result`. The accepted
frame stays at its original position in the owner's `input_queue`, which already carries route
identity and generation. This is lossless and duplicate-free for `Input`, `ModeGatedInput`, and
`Resize` by the same rule. Only the Core host thread admits ordinary frames, so a passing check
cannot be invalidated before the admit; the writer thread only frees capacity. Sealed or failed
control state keeps the existing hard-stop.

**B. Retry on a real capacity transition.** Extend the control writer so that popping an
ordinary frame away from the full boundary notifies that session's wake once. The worker
runtime already holds a `SessionWakeHandle` per session and already uses `notify_session_wake`
(`worker_process.rs:2384`) from its reader thread, so this reuses the ratified coalesced-wake
pattern and adds no public API, no timer, and no poll.

**C. Select retried routes by exact identity, not by session expansion.** ClientWorker records
each parked owner key with its generation. Woken Stage B selects
`adapter_route_keys(batch)` plus parked owners whose session the batch names and whose
generation still matches the live owner. A stale generation entry is dropped, never retried.
This does not expand an ingress wake to arbitrary sibling owners; it selects only routes that
Core itself parked while holding an accepted frame. Revision 2's public
`TerminalWakeSource::rearm_route` is removed, so no generation-blind public rearm exists.

## Scope

Core-only change in `botster-core` and `botster-core-daemon`.

1. **Route-scoped Stage B.** Add `ClientWorker::take_terminal_input_keys(keys, sessions_holding_gated, admit)`.
   Refactor `take_terminal_input` to build `rotated_live_keys()` and delegate, so both paths
   share one dequeue body, including the rule that every gated grant inserts its session into
   the held set before the next dequeue. `admit` is the per-session capacity predicate from
   mechanism A; the existing global caller passes an always-admit predicate so `CoreDaemon::drain`
   behavior does not change.
2. **One shared adapter-route filter.** Add `ClientWorker::adapter_route_keys(batch)` returning
   the deduplicated `batch.adapter_routes` key list. `intake_woken` and the woken apply both use
   it. `ClientWorker::pump_woken` keeps its existing two-class egress behavior unchanged.
3. **Parked-owner index.** Add a bounded `ClientWorker` record of owners parked for capacity,
   keyed by `OwnerKey` with the owner `generation`. Parking sets it, a successful dequeue clears
   it, and teardown or generation replacement removes it.
4. **Capacity predicate.** Add `ControlQueue::can_admit_ordinary()` and a
   `WorkerProcessRuntime` per-session query over it. `LocalProcessRuntime` writes straight to the
   PTY through `registry.write_input` and has no control queue, so its predicate always admits
   and its behavior does not change.
5. **Capacity wake.** Notify the session wake handle once when the control writer pops an
   ordinary frame away from the full boundary, coalesced through the existing wake state.
6. **Targeted ingress applier.** Add a private
   `ManagedSessionRuntime::apply_targeted_client_ingress(client_id, ingress, now)` that keeps
   `reject_unsupported_ingress` and the `ensure_terminal_backend_ok` check, calls the
   session-scoped `self.engine.handle_client_ingress` (`multiplexer.rs:366`), and then calls
   `flush_runtime_inputs_for_session` for that one session. It never calls
   `flush_runtime_inputs()` and never calls `apply_client_worker_with`.
7. **Shared delivery body.** Parameterize the existing `apply_one_delivery` over its ingress
   applier. `CoreDaemon::drain` keeps the current global applier. The woken path passes the
   targeted applier.
8. **Retention-order fix (defense in depth).** `flush_runtime_inputs_for_session` drops the
   failed input today: it sends `input.clone()` and prepends only the remaining iterator. Change
   it to return the failed input and every later input to the same queue in original order.
   Mechanism A means the woken path does not rely on this, but the loss defect is real and the
   ticket requires no loss.
9. **Woken apply, worker runtime.** Add
   `ManagedSessionRuntime<WorkerProcessRuntime, T>::apply_woken_terminal_input(batch, last_output_at)`.
   For each session named by `batch.adapter_routes` or holding a matching parked owner: consume
   control-writer failure, poll the gated result or timeout, build the held set from
   `session_runtime().sessions_holding_gated()` plus `client_worker.sessions_awaiting_gated()`,
   dequeue with the route-scoped Stage B under the capacity predicate, apply each delivery
   through the shared body with the targeted applier, and retain teardowns.
10. **Incremental-attach deferral.** Expose the woken apply on `BotsterEngine` for both runtimes.
    The `WorkerBackedBotsterEngine` arm keeps the existing gate: a woken session inside
    `incremental_attaches` runs `prepare_terminal_input` only, its routes park in the same index,
    and attach completion plus the next wake applies the deferred input.
11. **Local runtime arm.** Add the same method to
    `ManagedSessionRuntime<LocalProcessRuntime, T>` without the gated stage.
12. **Daemon wiring.** Add the `DaemonEngine` dispatch arm and call
    `apply_woken_terminal_input(&sub_batch, now_seconds)` from `CoreDaemon::pump_woken` after the
    per-session `engine.pump_woken` succeeds and before the lifecycle commit. An apply error
    follows the same `first_error` plus `record_terminal_commit_failure` handling.
13. Update the `docs/architecture/` terminal wake document: input intake and apply are
    adapter-route scoped, and parked routes retry on a control-queue capacity wake.

## Non-scope

- No change to `botster-hub`, `botster-web`, `botster-tui`, WebRTC code, or Unix transport code.
- No replay buffer and no second terminal route.
- No global scan, no polling path, no host-side input queue, no second `ClientWorker::pump`,
  no JSON input route, and no compatibility fallback.
- No public `rearm_route` and no generation-blind wake entry point.
- No behavior change to `CoreDaemon::drain`, to `apply_terminal_input`, or to the lifecycle
  commit and output delivery behavior from `3672c667`. Step 8 changes a shared helper; it
  removes a loss defect, adds no new drop site, and the existing drain tests must stay green
  unedited.
- No new wire protocol variant and no change to `TerminalWakeBatch`.

## Repository ownership boundaries and cross-repository dependencies

- Core owns duplex terminal input, mode gating, resize, ordering, bounded queues, generation
  fencing, wake classes, and targeted pumping.
- Hub stays content blind. Hub only supplies adapters and calls `wait_wakes` plus `pump_woken`.
  This run changes no Hub file and adds no Hub obligation.
- No cross-repository prerequisite exists, so this run registers no dependency ticket.
- Consequence, not prerequisite: `botster-hub` main pins Core `7eafa470a18025895995bbedc20d34b58106a03b`,
  which is 37 commits behind this base. Advancing that pin after merge is follow-up work owned
  by the `botster-hub` target, not by this run.
- Revision 3 adds no public Core contract. The capacity predicate, the parked-owner index, and
  the capacity wake are all internal. The behavior of the public `pump_woken` entry still changes,
  so the downstream-shaped proof below stays required, per
  [[botster core contract surface needs consumer proof]].

## Assumptions and unknowns

1. Assumption: intake must precede apply on one tick. Applying first would need a second wake
   that the PTY cannot produce before the input lands, which would deadlock the reproduction.
2. Assumption: input arrives only on `adapter_routes`, per the wake-class contract above. If
   Implement finds a production path that delivers client input under an ingress-only wake,
   that is a blocking question for a human, not a silent filter widening.
3. Assumption: the `input_result` frame and the PTY echo leave on the next wake. Applying input
   makes the child produce output, which raises the existing session ingress wake, so no extra
   rearm is needed for the success path. Only the capacity-parked path needs the new wake in
   mechanism B.
7. Assumption: adding `can_admit_ordinary`, the parked-owner index, and the control-writer
   capacity wake is inside this ticket's intent. The ticket requires preserving accepted frames
   across backpressure without loss and forbids a polling path. The current code offers no
   capacity query and no capacity signal, so the only alternative is the present fail-closed
   hard-stop, which the ticket explicitly rejects. If the owner prefers fail-closed teardown
   instead, that is a human decision and this plan changes.
4. Assumption: `docs/archive/plans/` is the plan home, from `docs/README.md` and
   `docs/plans/README.md`.
5. Unknown: whether any current test depends on `pump_woken` leaving input unapplied. The
   existing `pump_woken` tests are the oracle; Implement must not relax one to pass.
6. Unknown: whether the step 5 retention fix changes any existing drain-path assertion. If a
   current test asserts the dropped-input behavior, Implement must stop and report it rather
   than edit the assertion.

## Affected surfaces and files

- `crates/botster-core/src/engine/client_worker.rs` (route-scoped Stage B, shared adapter-route filter)
- `crates/botster-core/src/engine/managed_session_runtime.rs` (targeted ingress applier,
  parameterized delivery body, two `apply_woken_terminal_input` impls, retention fix)
- `crates/botster-core/src/engine/botster.rs` (facade methods, incremental-attach gate)
- `crates/botster-core/src/runtime/control_queue.rs` (`can_admit_ordinary`)
- `crates/botster-core/src/runtime/worker_process.rs` (per-session capacity query, control-writer
  capacity wake)
- `crates/botster-core-daemon/src/daemon.rs` (`DaemonEngine` arm, `CoreDaemon::pump_woken`)
- `crates/botster-core/tests/client_worker_engine_test.rs`
- `crates/botster-core/tests/managed_session_runtime_test.rs`
- `crates/botster-core/tests/terminal_wake_*` or the contract test that owns wake-source laws
- `crates/botster-core-daemon/tests/terminal_wake_test.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `docs/architecture/` terminal wake or adapter document
- `docs/archive/plans/pump-woken-targeted-duplex-input-apply.md` (this plan)

## Risks

1. **Wake-class drift.** Widening the input filter to ingress sessions would apply sibling
   input. One shared `adapter_route_keys` helper plus red ablation A2 guards this.
2. **Hidden global work.** The managed `handle_client_ingress` hides a global flush and a
   global pump. The targeted applier plus the no-unnamed-session oracle guards this.
3. **Retention regression.** The step 5 fix touches a helper shared with `drain`. Existing
   drain tests must stay green unedited, and a duplicate-byte assertion must accompany the
   retention assertion.
4. **Attach bypass.** Losing the `incremental_attaches` gate would push input past the attach
   barrier. A deferral test drives attach and asserts no PTY write until completion.
5. **Retry spin.** A self-rearm on a full queue would poll. Retry now follows only the control
   writer's capacity transition, coalesced through the existing session wake state. A queue held
   full must produce bounded work and no repeated pump cycle.
5b. **Stale retry.** A parked entry from a replaced owner must never move a new owner's input.
   Each entry carries its generation and is dropped on mismatch.
6. **Double apply.** Stage B pops the command, so a popped command cannot be re-dequeued. The
   mixed pump-then-drain test must still assert exactly one PTY write.
7. **Feature unification.** The local-runtime arm is feature gated, so the contract-only lane
   must run.
8. **Downstream reach.** Only some Core builders are reachable from a Hub daemon child, so
   downstream proof uses the Hub test named in the ticket at an actual candidate pin.

## Acceptance checks and tests

Behavior, all driven through `CoreDaemon::pump_woken`:

1. Plain input reaches the PTY exactly once.
2. Mode-gated input uses the authoritative Core mode state, submits once, parks the owner, and
   completes through the existing gated result path, reaching the PTY exactly once.
3. Resize reaches the named session exactly once.
4. A wake naming route A never applies input queued on sibling route B, and never applies input
   for a session absent from the batch.
5. A batch carrying only `ingress_sessions` applies no client input for an unparked route, and
   still drains runtime output and pumps adapter egress as `3672c667` shipped. It applies input
   only for a route Core itself parked, and only when that route's generation still matches.
6. A full ordinary control queue keeps the accepted frame queued in original order, keeps the
   owner alive, emits no `input_result`, produces no duplicate byte, and the capacity wake
   completes delivery. Prove this separately for `Input`, for `ModeGatedInput`, and for `Resize`.
   Use the existing `ControlQueue` `hold_pops` hook to hold the bound.
7. `control plane sealed` still hard-stops the owner and leaves siblings live.
8. While an incremental attach is active, input and resize stay deferred, and both apply after
   the attach completes, still exactly once.
9. A control queue held full parks the exact owner, keeps the frame queued, performs bounded work,
   and produces no repeated pump cycle. When the writer frees a slot, one capacity wake makes the
   parked owner deliver the same bytes once.
9b. A parked owner whose subscription is replaced by a newer generation never applies its parked
   input to the replacement owner.
10. The pump stays bounded: per-owner per-tick budget, input queue capacity, and no global scan.
    `pump_woken_does_not_try_read_unrelated_adapter` (`terminal_wake_test.rs:335`) stays green.
11. Existing lifecycle commit and output delivery tests from `3672c667` stay green with no edit.

Oracles:

- **No-unnamed-session oracle**: instrument the woken apply path so a visit to a session that
  the batch did not name fails the test. This is the positive control for finding
  "existing apply helper performs a global runtime-input scan".
- **Exact-bytes oracle**: assert the PTY received the exact frame payload once, not merely that
  some output appeared.

Red ablations, each must fail before the fix and pass after:

- A1: remove the apply call from `CoreDaemon::pump_woken`; the plain-input PTY test turns red.
- A2: widen the Stage B key set to include `ingress_sessions` owners; the sibling isolation
  test and the ingress-only test turn red.
- A3: make the capacity predicate always admit; the transient-full tests for `Input`,
  `ModeGatedInput`, and `Resize` turn red.
- A4: remove the `incremental_attaches` gate from the woken apply; the deferral test turns red.
- A5: drop the generation from the parked-owner entry; the stale-retry test turns red.
- A6: remove the control-writer capacity wake; the parked frame stalls and the retry test turns red.
- A7: restore the old `flush_runtime_inputs_for_session` failure arm; the retention-order test
  turns red.

Core gates, per [[botster-core uses CI-owned Cargo commands because it has no test script]]:

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
cargo test -p botster-core --no-default-features --lib
BOTSTER_ENV=test cargo test --doc --workspace
cargo test -p botster-core --test local_process_runtime_test
```

### Downstream candidate-pin procedure (required)

`botster-hub` consumes Core as a pinned git revision, so `--locked` proves nothing until every
pin names the candidate. Current Hub main pins `7eafa470a18025895995bbedc20d34b58106a03b` in
nine places across three manifests:

- `Cargo.toml`: `botster-core`, `botster-core-daemon`, `botster-terminal-protocol`,
  `botster-core-test-support`, `botster-terminal-ghostty`
- `crates/botster-hub-test-support/Cargo.toml`: `botster-core`, `botster-terminal-protocol`,
  `botster-terminal-ghostty`
- `crates/botster-hub-client/Cargo.toml`: `botster-terminal-protocol`

Procedure:

1. Push the candidate Core branch and record its SHA.
2. Use a clean, colon-free `botster-hub` proof checkout. Record the Hub SHA and confirm
   `git status` is clean before the pin edit.
3. Replace all nine `rev = "7eafa470..."` values with the candidate Core SHA.
4. Refresh `Cargo.lock` without `--locked`, then assert the lock contains no remaining
   `7eafa470` source line and that every `botster-core*`, `botster-terminal-protocol*`, and
   `botster-terminal-ghostty` source line names the candidate SHA.
5. Run the reproduction with the default target layout and `CARGO_TARGET_DIR` unset:

```bash
RUSTUP_TOOLCHAIN=1.97.0 ./test.sh --locked -p botster-hub --test hub_daemon_lifecycle_test \
  unix_adapter_unbound_scoped_drain_delivers_terminal_output -- --exact --nocapture
```

6. Record: Hub SHA, candidate Core SHA, the nine edited pins, the lock source lines, the clean
   pre-edit state, the exact command, the selected-test count (must be 1, per
   [[exact Rust test ablations require a one test baseline]]), and the observed PTY echo.
7. The pin edit is proof-only. Do not merge it to Hub. Advancing the Hub pin is separate
   follow-up work owned by the `botster-hub` target.

## Vault gaps worth capturing

1. "A targeted wake pump must complete every Core-owned stage, not only intake" — the general
   rule behind this defect.
2. "Adapter-route wakes and session-ingress wakes select different work" — client input follows
   routes; runtime output follows sessions. Revision 1 of this plan conflated them.
3. "A retained runtime input must return to its queue with the failed item first" — the
   `flush_runtime_inputs_for_session` loss defect.
3b. "Do not dequeue what the control plane cannot accept" — lossless backpressure comes from a
   capacity check before the dequeue, not from retention after a failed send.
3c. "A retry needs a capacity transition, not a self-rearm" — a consumer that frees capacity
   without a wake turns any self-rearm into a polling loop.
4. "Downstream locked proof requires an explicit candidate-pin procedure" — a git-pinned
   consumer proves nothing under `--locked` until every manifest pin names the candidate.
