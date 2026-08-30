# Core: apply targeted duplex input during pump_woken

Ticket: `ticket_1788128130_441301`
Run: `run_1788128178_478344`
Revision: 2 (revised after Plan Review `review_1788129045_539289`)
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

- `adapter_routes`: one exact route (`session_id`, `subscription_id`, `generation`) has
  adapter work — readable client bytes, returned write capacity, or closure. Client input
  exists only on this class.
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
`DEFAULT_MODE_GATED_INPUT_TIMEOUT` deadline instead of blocking. The exact-route rearm is one
registry lookup plus one lock-free coalesce arm. No sleep, wait, or `block_on` enters the pump.

`late_message_matrix`:

| Command | Owner tag | Rejection after terminal failure | Residual sweep |
|---|---|---|---|
| `Input` | route key + owner `generation` + `client_id` | sealed or failed control state returns `owner_apply_teardown` for that route only | teardown routes through `unsubscribe_owner_teardowns` in the same `pump_woken` |
| `ModeGatedInput` | route key + `generation` + `awaiting_gated` park state | session held by `sessions_holding_gated` plus `sessions_awaiting_gated`; `control plane sealed` or full control queue hard-stops the owner; `already in flight` returns `SessionNotWritable` | `cancel_mode_gated_pty_input` on teardown; `clear_awaiting_gated` on Ready or TimedOut |
| `Resize` | route key + `generation` + `client_id` | same as `Input` | same tick teardown routing |
| Retired route wake | route already removed by `retire_route` | `live.get_mut` misses, so Stage B skips the key | no queue growth; the owner is already dropped |
| Session-ingress wake | carries no client input | Stage B never runs for this class | not applicable |

`production_path_proof`: `TerminalWakeSource::wait_wakes` -> host loop ->
`CoreDaemon::pump_woken` -> `DaemonEngine::pump_woken` (Stage A `intake_woken`) ->
`DaemonEngine::apply_woken_terminal_input` -> route-scoped Stage B ->
`MultiplexerEngine::handle_client_ingress` or `submit_mode_gated_pty_input` ->
`flush_runtime_inputs_for_session` -> PTY write. Proof drives `CoreDaemon::pump_woken`
itself, never a direct `apply_terminal_input` call and never a ClientWorker helper call.

`ownership_identity`: Each delivery carries `client_id`, `session_id`, `subscription_id`, and
`generation` from the live owner, so a reused subscription id cannot inherit a stale delivery.
Stage B reads only the current `live` map. `TerminalInputResult` keeps `with_subscription`.

`sibling_fail_closed_policy`: On success siblings keep working. On owner apply failure only
the failing owner hard-stops. On session control-plane failure the existing `teardown_session`
sweep removes every route of that one session; other sessions survive.

## Scope

Core-only change in `botster-core` and `botster-core-daemon`.

1. **Route-scoped Stage B.** Add `ClientWorker::take_terminal_input_keys(keys, sessions_holding_gated)`.
   Refactor `take_terminal_input` to build `rotated_live_keys()` and delegate, so both paths
   share one dequeue body, including the rule that every gated grant inserts its session into
   the held set before the next dequeue.
2. **One shared adapter-route filter.** Add `ClientWorker::adapter_route_keys(batch)` that
   returns the deduplicated `batch.adapter_routes` key list. `intake_woken` and the new apply
   both use it, so Stage A and Stage B can never select different routes. `ClientWorker::pump_woken`
   keeps its existing two-class behavior (adapter routes plus waking bound owners of
   `ingress_sessions`) unchanged, because egress is not input.
3. **Targeted ingress applier.** Add a private
   `ManagedSessionRuntime::apply_targeted_client_ingress(client_id, ingress, now)` that keeps
   `reject_unsupported_ingress` and the `ensure_terminal_backend_ok` check, calls the
   session-scoped `self.engine.handle_client_ingress` (`multiplexer.rs:366`), and then calls
   `flush_runtime_inputs_for_session` for that one session. It never calls
   `flush_runtime_inputs()` and never calls `apply_client_worker_with`.
4. **Shared delivery body.** Parameterize the existing `apply_one_delivery` over its ingress
   applier. `CoreDaemon::drain` keeps the current global applier, so its behavior does not
   change. The woken path passes the targeted applier from step 3.
5. **Backpressure retention fix.** Change `flush_runtime_inputs_for_session` so a failed
   `send_input` returns the failed input *and* every later input to the same worker queue in
   the original order, with no clone-and-drop. Classify the failure: `control queue full` is
   transient and keeps the owner alive with the input retained; `control plane sealed` and any
   other `InputFailed` keep the existing failure path.
6. **Woken apply, worker runtime.** Add
   `ManagedSessionRuntime<WorkerProcessRuntime, T>::apply_woken_terminal_input(batch, last_output_at)`.
   For each session named by `batch.adapter_routes` only: consume control-writer failure,
   poll the gated result or timeout, build the held set from
   `session_runtime().sessions_holding_gated()` plus `client_worker.sessions_awaiting_gated()`,
   dequeue with `take_terminal_input_keys(adapter_route_keys(batch), held)`, apply each
   delivery through the shared body with the targeted applier, and retain teardowns.
7. **Incremental-attach deferral.** Expose the woken apply on `BotsterEngine` for both
   runtimes. The `WorkerBackedBotsterEngine` arm keeps the existing gate: when
   `incremental_attaches` contains a woken session, that session runs
   `prepare_terminal_input` only, Stage B stays deferred for its routes, and the plan rearms
   those routes (step 9) so the deferred input applies after the attach completes.
8. **Local runtime arm.** Add the same method to
   `ManagedSessionRuntime<LocalProcessRuntime, T>` without the gated stage, applying through
   the existing local delivery body.
9. **Exact-route rearm.** Add `TerminalWakeSource::rearm_route(session_id, subscription_id)`:
   one lookup in the existing route registry, then the same lock-free coalesce arm that
   `TerminalWakeSink::wake` performs, returning without effect for a retired or absent route.
   Core calls it for one exact route only when that route still holds retained input or a
   queued egress frame after the apply. This replaces revision 1's `notify_session`, which
   would have crossed wake classes. It adds no scan, no timer, and no second pump.
10. **Daemon wiring.** Add the `DaemonEngine` dispatch arm and call
    `apply_woken_terminal_input(&sub_batch, now_seconds)` from `CoreDaemon::pump_woken` after
    the per-session `engine.pump_woken` succeeds and before the lifecycle commit. An apply
    error follows the same `first_error` plus `record_terminal_commit_failure` handling the
    pump error already uses, so per-session isolation is unchanged.
11. Update the `docs/architecture/` terminal wake document where it lists the pump stages, and
    state that input intake and apply are adapter-route scoped.

## Non-scope

- No change to `botster-hub`, `botster-web`, `botster-tui`, WebRTC code, or Unix transport code.
- No replay buffer and no second terminal route.
- No global scan, no polling path, no host-side input queue, no second `ClientWorker::pump`,
  no JSON input route, and no compatibility fallback.
- No behavior change to `CoreDaemon::drain`, to `apply_terminal_input`, or to the lifecycle
  commit and output delivery behavior from `3672c667`. The step 5 retention fix changes a
  shared helper; it removes a loss defect and adds no new drop site, and the existing drain
  tests must stay green unedited.
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
- `TerminalWakeSource::rearm_route` is a public Core contract addition, so it needs the
  downstream-shaped proof below, per [[botster core contract surface needs consumer proof]].

## Assumptions and unknowns

1. Assumption: intake must precede apply on one tick. Applying first would need a second wake
   that the PTY cannot produce before the input lands, which would deadlock the reproduction.
2. Assumption: input arrives only on `adapter_routes`, per the wake-class contract above. If
   Implement finds a production path that delivers client input under an ingress-only wake,
   that is a blocking question for a human, not a silent filter widening.
3. Assumption: the `input_result` frame and the PTY echo may leave on a later wake.
   `enqueue_input_result` arms no wake, so step 9 rearms the exact route. If Implement shows
   the echo wake already arrives for every case, step 9 narrows to the retained-input case
   rather than remaining as dead code.
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
- `crates/botster-core/src/contract/terminal_wake.rs` (`rearm_route`)
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
5. **Rearm storm.** An unconditional rearm could spin. The rearm fires only when the exact
   route still holds retained input or a queued frame, and the coalesce gate already collapses
   repeats. A no-work tick must arm nothing.
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
5. A batch carrying only `ingress_sessions` applies no client input, and still drains runtime
   output and pumps adapter egress as `3672c667` shipped.
6. Transient `control queue full` retains the exact accepted bytes in original order, keeps the
   owner alive, produces no duplicate byte, and the next wake completes delivery.
7. `control plane sealed` still hard-stops the owner and leaves siblings live.
8. While an incremental attach is active, input and resize stay deferred, and both apply after
   the attach completes, still exactly once.
9. The exact-route rearm arms only the route that retains work, and a no-work tick arms nothing.
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
- A3: restore the old `flush_runtime_inputs_for_session` failure arm; the retention test turns red.
- A4: remove the `incremental_attaches` gate from the woken apply; the deferral test turns red.

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
4. "Downstream locked proof requires an explicit candidate-pin procedure" — a git-pinned
   consumer proves nothing under `--locked` until every manifest pin names the candidate.
