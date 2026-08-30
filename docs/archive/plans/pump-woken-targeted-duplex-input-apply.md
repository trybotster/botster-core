# Core: apply targeted duplex input during pump_woken

Ticket: `ticket_1788128130_441301`
Run: `run_1788128178_478344`
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
- No caller then runs Stage B (`ClientWorker::take_terminal_input`) or the apply stage
  (`ManagedSessionRuntime::apply_terminal_input`).

`CoreDaemon::pump_woken` (`crates/botster-core-daemon/src/daemon.rs:1092`) calls only
`DaemonEngine::pump_woken`. The session-scoped `DaemonEngine::apply_terminal_input`
(`daemon.rs:3627`) runs only from `CoreDaemon::drain` (`daemon.rs:1341`). A host that
drives the runtime with `wait_wakes` + `pump_woken` never reaches the apply stage, so an
accepted `TerminalInputFrame` stays in `input_queue` forever and never reaches the PTY.

That is the exact Hub failure:

```
RUSTUP_TOOLCHAIN=1.97.0 ./test.sh --locked -p botster-hub --test hub_daemon_lifecycle_test \
  unix_adapter_unbound_scoped_drain_delivers_terminal_output -- --exact --nocapture
```

The Hub test receives initial output, sends a valid binary `TerminalInputFrame`, observes
successful targeted pumps, and never sees the PTY echo.

## Playbooks and notes loaded

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-core-playbook]] (repository charter for the resolved target)
- [[botster runtime teardown lenses]] (runtime-teardown class applies; see below)
- [[core owns duplex terminal transport while Hub stays content blind]]
- [[core terminal progress is wake driven and targeted]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[a global owner walk needs a global per session gated hold]]
- [[a batch dequeue loop must update its exclusion set as it grants work]]
- [[every TerminalInputResult must stamp the live subscription id]]
- [[adapter accepted writes are not consumer flushed writes]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster-core CI runs a contract only test lane because workspace feature unification hides breaks]]
- [[spawned Hub tests can reach only four of fourteen Core test builders]]
- [[bound attach suppression skip is unproven without a red oracle]]
- [[vault example paths are not repository placement conventions]] (plan destination taken from `docs/README.md`)

## Context loaded

- `crates/botster-core/src/engine/managed_session_runtime.rs` (Stage A/B/apply, `pump_woken`,
  `apply_terminal_input` for `WorkerProcessRuntime` and for `LocalProcessRuntime`,
  `flush_runtime_inputs_for_session`)
- `crates/botster-core/src/engine/client_worker.rs` (`intake_woken`, `take_terminal_input`,
  `pump_woken`, `sessions_awaiting_gated`, `enqueue_input_result`)
- `crates/botster-core/src/contract/terminal_wake.rs` (`TerminalWakeSource`, `notify_session`)
- `crates/botster-core-daemon/src/daemon.rs` (`CoreDaemon::pump_woken`, `CoreDaemon::drain`,
  `DaemonEngine` dispatch)
- `crates/botster-core-daemon/tests/terminal_wake_test.rs`,
  `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `docs/README.md`, `docs/plans/README.md` (plan destination), `.github/workflows/ci.yml`

## Runtime-teardown class answers

`teardown_class_applies`: **yes**. The change touches ClientWorker route ownership,
per-route generation fencing, hard-stop teardown on decode/apply failure, and
terminal-state vs live-runtime divergence (queued frame vs PTY).

`teardown_isolation`: The ownership set is one `OwnerKey { session_id, subscription_id }`
with its `generation`. An apply failure hard-stops only that owner through the existing
`owner_apply_teardown` / `detach_live` path and emits one `UnsubscribeSession` ingress.
Sibling routes on the same session and other sessions keep running. No shared resource is
sacrificed.

`teardown_bounds`: The apply stage stays bounded and non-blocking. Stage B keeps
`APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK` per owner per tick, `INPUT_QUEUE_CAPACITY` bounds
the queue, and the gated path parks the owner with the existing
`DEFAULT_MODE_GATED_INPUT_TIMEOUT` deadline instead of blocking. No new wait, sleep, or
`block_on` enters the pump. Control-writer failure is consumed and converted to a session
teardown, exactly as `apply_terminal_input` already does.

`late_message_matrix`:

| Command | Owner tag | Rejection after terminal failure | Residual sweep |
|---|---|---|---|
| `Input` | route key + owner `generation` + `client_id` | ingress error returns `owner_apply_teardown` for that route only | teardown returned to `unsubscribe_owner_teardowns` in the same `pump_woken` |
| `ModeGatedInput` | route key + `generation`, plus `awaiting_gated` park state | session held by `sessions_holding_gated` + `sessions_awaiting_gated`; sealed/full control queue hard-stops the owner; `already in flight` returns `SessionNotWritable` | `cancel_mode_gated_pty_input` on teardown; `clear_awaiting_gated` on Ready/TimedOut |
| `Resize` | route key + `generation` + `client_id` | ingress error returns `owner_apply_teardown` | same tick teardown routing |
| Retired route wake | `wake_source.retire_route` removed the route | `live.get_mut` miss makes Stage B skip the key | no queue growth; owner already dropped |

`production_path_proof`: The production path is
`TerminalWakeSource::wait_wakes` -> host loop -> `CoreDaemon::pump_woken` ->
`DaemonEngine::pump_woken` (`intake_woken`) -> new `DaemonEngine::apply_woken_terminal_input`
-> `ManagedSessionRuntime` Stage B for named routes -> `handle_client_ingress` /
`submit_mode_gated_pty_input` -> `flush_runtime_inputs_for_session` -> PTY write.
Proof drives `CoreDaemon::pump_woken` itself, never a direct call to
`apply_terminal_input` or to a ClientWorker helper.

`ownership_identity`: Deliveries carry `client_id`, `session_id`, `subscription_id`, and
`generation` from the live owner, so a reused subscription id cannot inherit a stale
delivery. Stage B reads owners under the current `live` map only. `TerminalInputResult`
keeps `with_subscription`, preserving
[[every TerminalInputResult must stamp the live subscription id]].

`sibling_fail_closed_policy`: On success, siblings keep working. On owner apply failure,
only the failing owner hard-stops. On session control-plane failure, the existing
`teardown_session` sweep removes every route of that one session; other sessions survive.

## Scope

Core-only change in `botster-core` and `botster-core-daemon`.

1. `ClientWorker`: add a route-targeted Stage B entry point
   `take_terminal_input_keys(keys, sessions_holding_gated)`. Refactor the existing
   `take_terminal_input` to build `rotated_live_keys()` and delegate, so both paths share one
   dequeue body, including the "insert into the held set after every gated grant" rule from
   [[a batch dequeue loop must update its exclusion set as it grants work]].
2. `ClientWorker`: add `woken_input_keys(batch)` that resolves the same route set that
   `intake_woken` and `pump_woken` use: named adapter routes, plus, for `ingress_sessions`,
   only live owners of that session with `waking && adapter.is_some()`. Reuse one helper so
   the route filter cannot drift between intake, apply, and pump.
3. `ManagedSessionRuntime<WorkerProcessRuntime, T>`: add
   `apply_woken_terminal_input(&mut self, batch, last_output_at)` that, for the named routes
   only, (a) consumes control-writer failure per woken session, (b) polls the gated result or
   timeout per woken session, (c) builds the held set from
   `session_runtime().sessions_holding_gated()` plus `client_worker.sessions_awaiting_gated()`,
   (d) dequeues with `take_terminal_input_keys`, (e) applies each delivery through the existing
   `apply_one_delivery`, (f) flushes runtime inputs for the woken sessions only, and
   (g) retains teardowns in `pending_input_teardowns`.
4. `ManagedSessionRuntime<LocalProcessRuntime, T>`: add the same method without the gated
   stage, applying through the existing `apply_one_local_delivery`.
5. `BotsterEngine` facade: expose `apply_woken_terminal_input` on both runtime impls.
6. `DaemonEngine`: add the matching enum dispatch arm.
7. `CoreDaemon::pump_woken`: after `self.engine.pump_woken(&sub_batch, now_seconds)` succeeds
   for one session, call `self.engine.apply_woken_terminal_input(&sub_batch, now_seconds)`
   before the existing lifecycle commit and output retention run. An apply error follows the
   same `first_error` + `record_terminal_commit_failure` handling the pump error already uses,
   so the loop keeps its per-session isolation.
8. Emit one targeted `wake_source.notify_session(&session_id)` only when at least one delivery
   applied for that session, so the resulting `input_result` frame and PTY echo get a wake on
   the same route set. This reuses the existing bounded session wake; it adds no scan and no
   second pump.
9. Update `docs/architecture/` terminal wake documentation where it states the pump stage list.

## Non-scope

- No change to `botster-hub`, `botster-web`, `botster-tui`, WebRTC code, or Unix transport code.
- No replay buffer and no second terminal route.
- No global scan, no polling loop, no host-side input queue, no second `ClientWorker::pump`,
  no JSON input route, and no compatibility fallback.
- No change to `CoreDaemon::drain`, to `apply_terminal_input`, or to the existing lifecycle
  commit and output delivery behavior introduced by `3672c667`.
- No new public enum variant and no change to the terminal wire protocol.

## Repository ownership boundaries and cross-repository dependencies

- Core owns duplex terminal input, mode gating, resize, ordering, bounded queues, generation
  fencing, and targeted pumping ([[core owns duplex terminal transport while Hub stays content blind]]).
- Hub stays content blind. Hub only supplies adapters and calls `wait_wakes` + `pump_woken`.
  This ticket adds no Hub-side call and no new Hub obligation.
- The fix requires no cross-repository prerequisite. Hub gains the corrected behavior when it
  bumps its Core pin after this merges. No dependency ticket is registered for this run.
- The Hub reproduction command is downstream evidence only. It runs against a Hub checkout
  pointed at this Core branch; this run changes no Hub file.

## Assumptions and unknowns

1. Assumption: the intended tick order is intake (inside `engine.pump_woken`) then apply, so a
   single wake both accepts and applies a frame. Applying before intake would need a second
   wake that the PTY cannot produce, which would deadlock the Hub reproduction.
2. Assumption: the resulting `input_result` frame and the PTY echo may leave on the next wake.
   `ClientWorker::enqueue_input_result` arms no wake today, so step 8 uses the existing
   `TerminalWakeSource::notify_session` to guarantee that next wake. If the wake-loop evidence
   shows the echo already arrives without it, step 8 is dropped rather than kept as dead code.
3. Assumption: `docs/archive/plans/` is the plan destination, from `docs/README.md` and
   `docs/plans/README.md`, not a vault example path.
4. Unknown: whether any current caller depends on `pump_woken` leaving input unapplied. The
   existing `pump_woken` tests are the oracle; the Implement step must not relax one to pass.
5. Unknown: exact `botster-hub` revision used for the downstream reproduction. Verify records
   the Hub SHA and the Core SHA it pins.

## Affected surfaces and files

- `crates/botster-core/src/engine/client_worker.rs` (targeted Stage B, shared woken route helper)
- `crates/botster-core/src/engine/managed_session_runtime.rs` (two `apply_woken_terminal_input`
  impls, targeted flush)
- `crates/botster-core/src/engine/botster.rs` (facade methods on both runtime impls)
- `crates/botster-core-daemon/src/daemon.rs` (`DaemonEngine` arm, `CoreDaemon::pump_woken`)
- `crates/botster-core/tests/client_worker_engine_test.rs` (targeted Stage B unit proof)
- `crates/botster-core/tests/managed_session_runtime_test.rs` (route filter proof)
- `crates/botster-core-daemon/tests/terminal_wake_test.rs` and
  `crates/botster-core-daemon/tests/daemon_integration_test.rs` (production `pump_woken` proof)
- `docs/architecture/` terminal wake or adapter document that lists the pump stages
- `docs/archive/plans/pump-woken-targeted-duplex-input-apply.md` (this plan)

## Risks

1. **Double apply.** If both `pump_woken` and a later `drain` apply, one frame could reach the
   PTY twice. Stage B pops from `input_queue`, so a popped command cannot be re-dequeued;
   the mixed-path test must still assert exactly one PTY write.
2. **Route leakage.** A careless Stage B could dequeue a sibling route on the woken session.
   The shared `woken_input_keys` helper plus a red ablation guards this.
3. **Gated regression.** Dropping the session-wide held set would let two owners submit gated
   input for one session. The held set must keep both
   `sessions_holding_gated()` and `sessions_awaiting_gated()`.
4. **Backpressure loss.** A PTY write that cannot accept every byte must keep the accepted
   frame. The existing `flush_runtime_inputs_for_session` `prepend_inputs` path preserves the
   remainder; the plan adds no new drop site and no re-send of already-sent input.
5. **Lifecycle commit order.** Inserting the apply before the commit could hide a commit
   failure. The apply reuses the same `first_error` handling, and the existing commit tests at
   `daemon_integration_test.rs:315-620` must stay green unchanged.
6. **Feature unification.** The local-runtime impl is feature gated. The contract-only lane
   (`cargo test -p botster-core --no-default-features --lib`) must run, per
   [[botster-core CI runs a contract only test lane because workspace feature unification hides breaks]].
7. **Downstream reach.** Only some Core builders are reachable from a Hub daemon child
   ([[spawned Hub tests can reach only four of fourteen Core test builders]]), so downstream
   proof uses the Hub test named in the ticket, not a Core-only harness.

## Acceptance checks and tests

Behavior:

1. Plain input: a wake carrying one adapter route with a `TerminalInputFrame` reaches the PTY
   exactly once through `CoreDaemon::pump_woken`.
2. Mode-gated input: the gated command uses the authoritative Core mode state, submits once,
   parks the owner, and completes through the existing gated result path.
3. Resize: a resize command reaches the named session exactly once.
4. Route targeting: a wake for route A never applies queued input owned by route B, and never
   applies input for a session absent from the batch.
5. Backpressure: when the runtime input write cannot complete, the accepted frame survives and
   is delivered once, with no duplicate byte on retry.
6. Bounds: the apply respects the per-owner per-tick budget and the input queue capacity, and
   performs no global scan (the unrelated-adapter test at `terminal_wake_test.rs:335` stays green).
7. Lifecycle: the commit and output delivery tests from `3672c667` stay green with no edit.

Red ablations (each must fail before the fix and pass after):

- A1: remove the new apply call from `CoreDaemon::pump_woken`; the plain-input PTY test turns red.
- A2: replace the targeted route filter with every live owner; the sibling-route isolation test
  turns red.

Gates (repository-owned commands, per [[botster-core uses CI-owned Cargo commands because it has no test script]]):

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
cargo test -p botster-core --no-default-features --lib
BOTSTER_ENV=test cargo test --doc --workspace
cargo test -p botster-core --test local_process_runtime_test
```

Downstream-shaped proof (required by the charter for public contract behavior):

```bash
RUSTUP_TOOLCHAIN=1.97.0 ./test.sh --locked -p botster-hub --test hub_daemon_lifecycle_test \
  unix_adapter_unbound_scoped_drain_delivers_terminal_output -- --exact --nocapture
```

Run this in a `botster-hub` checkout pinned to this Core branch. Record the Hub SHA, the Core
SHA, and the observed PTY echo. Per the Hub gate rule, use a colon-free worktree and do not set
`CARGO_TARGET_DIR` for the Hub `./test.sh --locked` run.

## Vault gaps worth capturing

1. "A targeted wake pump must complete every Core-owned stage, not only intake" — the general
   rule behind this defect: a stage split across `pump_woken` and `drain` leaves the wake-driven
   host path incomplete.
2. "Route-targeted and global stage entry points must share one route filter" — prevents drift
   between `intake_woken`, `pump_woken`, and the new apply.
3. Whether `enqueue_input_result` should arm its own route wake, instead of each caller
   notifying the session. Capture the decision after the Implement step measures it.
