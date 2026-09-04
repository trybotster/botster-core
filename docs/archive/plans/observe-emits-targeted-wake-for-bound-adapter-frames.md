# Observe must not queue bound adapter frames without a targeted wake

Ticket: `ticket_1788523929_630135`
Run: `run_1788523950_844880`
Plan visit: 3 (revised after Plan Review `review_1788525170_869520` and
`review_1788525851_958407`)

## Target repository

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, resolved through the
  admitted spawn-target list, not through the process working directory.
- Base ref: `main` at `72d1c75` (`Strengthen readback no-progress proof`).
  This is the exact Core revision that the ticket names.
- Merge policy: direct to `main`.
- Plan placement: `docs/archive/plans/` follows mainline prior art
  (`delete-the-core-polling-adapter-path.md` was planned and revised there).
  `docs/plans/README.md` marks that directory as a stub.

## Playbooks and notes loaded

Repository charter:

- [[botster-core-playbook]]

Role playbooks:

- [[planner-playbook]]
- [[botster-planner-playbook]]

Runtime task-surface guidance (added in visit 2; the Core charter requires the
runtime overlay for lifecycle, PTY, and transport work):

- [[cli-patterns]]
- [[botster-runtime-reviewer-playbook]]

Class overlay (runtime-teardown class applies; see the lens answers below):

- [[botster runtime teardown lenses]]

Targeted atomic notes:

- [[core terminal progress is wake driven and targeted]]
- [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]]
- [[Hub terminal cold cut consumed Core 72d1c75]]
- [[session ingress wakes retire on observed exit not shutdown acceptance]]
- [[session wake coalescing belongs in a lifecycle registry not each handle]]
- [[core holds declared attach frames until the bound adapter drains]]
- [[attach routes use subscription scoped Core drains]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
- [[a non completing shared fake adapter is a blind no progress oracle]]
- [[shared fake terminal adapter delivered frame bytes is a constant empty oracle]]
- [[botster core contract surface needs consumer proof]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster core worktrees need the ghostty submodule initialized]]
- [[core terminal wake test binaries must run from the repository working directory]]
- [[botster core bounded waiting queue test flakes under workspace load]]

Not loaded: [[project-pipelines-playbook]]. This ticket touches no Project
Pipelines package or plugin path.

## Context loaded

- Ticket text, including the Hub rejection
  (`review_1788523297_801440` / `finding_1788523297_189742`) and the parent
  Hub ticket `ticket_1787600679_990088`.
- Core source at `72d1c75`:
  `crates/botster-core-daemon/src/daemon.rs`,
  `crates/botster-core/src/engine/client_worker.rs`,
  `crates/botster-core/src/engine/managed_session_runtime.rs`,
  `crates/botster-core/src/engine/botster.rs`,
  `crates/botster-core/src/contract/terminal_wake.rs`.
- Core tests: `crates/botster-core-daemon/tests/terminal_wake_test.rs`,
  `crates/botster-core-daemon/tests/daemon_integration_test.rs`, the
  isolated consumer crates under
  `crates/botster-core-test-support/tests/consumers/` and their wrapper
  tests.
- Core CI: `.github/workflows/ci.yml`.
- Core docs: `docs/architecture/client-worker-terminal-egress.md`,
  `docs/architecture/control-plane-lifecycle-journal.md`.
- Hub consumer seam, read only, after `git fetch origin --prune` on
  2026-09-04. Current Hub `origin/main` is `ae6a0b1` and still pins Core
  `48a4370`. The active parent ticket branch for
  `ticket_1787600679_990088` is `6ff08fd` (`Record removal of the global
  adapter pump.`) and pins Core `72d1c75`. On that branch
  `src/daemon/owner_loop.rs` calls `observe_lifecycle_slice` each owner
  turn, `src/daemon/control/sessions.rs` calls
  `observe_session_lifecycle`, and `src/data_plane/driver.rs` is the only
  `pump_woken` caller. Hub source guards already forbid
  `list_terminal_subscriptions` inside the pump.
- Registered dependency: `dependency_1788523941_772682` makes the parent Hub
  ticket depend on this Core ticket. The parent cannot start a run until
  this ticket closes, and it will then pin the merged Core revision.

## Root cause at 72d1c75

Line numbers are at `72d1c75`.

1. `CoreDaemon::observe_session` (`daemon.rs:3167`) calls
   `engine.drain_runtime_once`. `ManagedSessionRuntime::drain_runtime_once`
   (`managed_session_runtime.rs:1444`) routes runtime outputs and then calls
   `apply_client_worker`, which calls
   `ClientWorker::ingest_bound_terminal_frames` (`client_worker.rs:479`).
   For an owner with a bound adapter, ingest pushes each encoded frame,
   including `process_exit`, onto `owner.queue`. Ingest never calls
   `try_write`, and it never notifies a wake.
2. `observe_session` then calls `commit_terminal_lifecycle`
   (`daemon.rs:2217`). When the drain observed `Exited` or `Failed`, the
   commit calls `wake_source().forget_session` (`daemon.rs:2293`). The
   session wake state becomes retired.
3. `TerminalWakeSource::assemble_batch` (`terminal_wake.rs:665`) drops
   every queued ingress node whose session state is retired, and
   `notify_session` (`terminal_wake.rs:451`) returns without sending for a
   retired session. Any real PTY or worker ingress wake that raced the
   observe is therefore discarded. The queued `process_exit` frame has no
   remaining wake that can reach it.
4. `drain_runtime_for_readback` (`daemon.rs:2596`) avoids this because it
   stores the terminal state as an obligation and calls `notify_session`
   before any commit. The later `pump_woken` pumps the route and commits the
   obligation on the same tick. `observe_session` and `CoreDaemon::drain`
   (`daemon.rs:1485`) commit immediately and never notify.

Result: after Hub attaches, binds an adapter, and the process exits, the Hub
owner loop's `observe_lifecycle_slice` steals the exit drain. Core commits
`Exited` to the registry and journal, retires the session wake, and leaves
`process_exit` queued forever on the bound owner. Only an illegal global
adapter pump after every Hub request could deliver it.

## Scope

In scope:

1. **Targeted wake at the ingest boundary.** After a non-pump drain queues
   one or more frames onto a bound `ClientWorker` owner, Core calls
   `TerminalWakeSource::notify_session` for that session before it commits
   lifecycle. The non-pump drains are `observe_session` (which serves
   `observe_lifecycle`, `observe_lifecycle_slice`, and
   `observe_session_lifecycle`), `drain_runtime_for_readback` (which serves
   `read_screen`, `read_mode_flags`, `capture_snapshot`,
   `capture_color_and_snapshot`, and shutdown readback), and
   `CoreDaemon::drain` (which serves `drain_subscription`).
   - The wake condition is a delta, not a standing query: the ingest queued
     at least one new frame onto an owner whose adapter is bound, whose
     write is not in flight, and whose adapter pressure is `Ready`. An owner
     with an in-flight or non-Ready adapter already owns a coalesced
     writable wake obligation from the adapter contract, and a redundant
     ingress pump would consume one unit of `WRITE_ATTEMPT_BUDGET`
     (`client_worker.rs:37`).
   - `ClientWorker` records the affected session ids during
     `ingest_bound_terminal_frames` and `flush_held_after_bind`.
     `ManagedSessionRuntime` exposes a crate-visible take of that set. The
     daemon takes it after each non-pump drain and notifies each session.
     The daemon pump path (`CoreDaemon::pump_woken`, `daemon.rs:1209`) also
     takes and discards the set, so pump-time ingest never self-wakes.
     `WorkerBackedBotsterEngine::pump_woken` (`botster.rs:1181`) calls
     `drain_runtime_once` for deferred incremental-attach sessions; that
     call is inside the pump and must not produce a wake either.
   - The session ingress wake is the Core-owned wake for this case.
     Core does not fabricate adapter wakes. `pump_woken` already pumps every
     bound route of a named ingress session (`client_worker.rs:658`).
   - **Bind-time wake for held frames.** `bind_waking_terminal_adapter` is
     not a pump path, and the `WakingTerminalAdapter` contract does not
     require an initial writable wake. After a successful bind onto an owner
     whose `held` queue is non-empty, Core calls `notify_session` for that
     session while the session wake is still live (item 2 keeps it live).
     The next `pump_woken` for that ingress session runs
     `pump_woken_phase_three` (`managed_session_runtime.rs:1630`), whose
     `ingest_bound_terminal_frames` call runs `flush_held_after_bind` and
     moves `held` into `queue`, and whose `ClientWorker::pump_woken` ingress
     branch then `try_write`s the route. No manual adapter wake is needed.
2. **Defer wake retirement while any live owner still holds undelivered
   frames.** `commit_terminal_lifecycle` calls `forget_session` only when
   the session is terminal and `ClientWorker` has no live owner for that
   session with a non-empty `held` queue, a non-empty `queue`, or an
   in-flight write. This covers bound owners and unbound declared owners
   alike. The same condition runs on every later commit for that session,
   including the commit inside `CoreDaemon::pump_woken` after `process_exit`
   delivery and after a hard stop, so the wake retires on the tick that
   completes routing. If a declared owner never binds, the owner-loop
   `observe_lifecycle_slice` visits the session each turn and re-evaluates
   the rule after `unsubscribe` or hard stop removes the owner, and
   `remove_session` plus `forget_terminal_session`
   (`managed_session_runtime.rs:952`) keep their unconditional forget. The
   session wake state is one registry entry, so the deferral holds no
   thread, socket, or adapter open.
   This is the `ProcessExited routing complete` half of
   [[session ingress wakes retire on observed exit not shutdown acceptance]].
3. **Lifecycle journal advancement stays immediate.** `observe_session`
   still commits registry state and journal rows on the observe call. A
   session with no terminal client and no bound adapter still retires its
   wake on that same call, so `observe_lifecycle_exit_commits_and_retires_once`
   (`daemon_integration_test.rs:1031`) and
   `worker_backed_observe_advances_exit_without_attach_or_drain`
   (`daemon_integration_test.rs:5518`) keep their current assertions.
4. **Negative control.** Extend or sibling
   `readback_does_not_advance_bound_adapter` (`terminal_wake_test.rs:2341`)
   so the oracle is `try_write_count()` on an `auto_complete()` fake, not
   only delivered bytes, and so the session has real queued output and an
   observed exit at the time of the read-only calls. The read-only set is
   `list_sessions`, `list_terminal_subscriptions`,
   `lifecycle_changes_page`, `lifecycle_baseline_page`,
   `session_registry_state`, `observe_lifecycle`, `observe_lifecycle_slice`,
   `observe_session_lifecycle`, and `read_screen`. The assertion is that
   `try_write_count()` does not change across those calls.
5. **Positive control.** A new `terminal_wake_test.rs` test: spawn a
   short-lived session, declare, attach, bind an `auto_complete()` adapter,
   wait for the process to exit, then call `observe_session_lifecycle` and
   one `observe_lifecycle_slice` before any pump. Assert: registry state is
   `Exited`, the journal holds exactly one `Exited` upsert, `try_write_count`
   is unchanged, and `wake_source().session_registry_len()` is still 1.
   Then `wait_wakes` returns an ingress batch that names the session,
   `pump_woken` delivers `process_exit` (`adapter_has_process_exit`), the
   adapter is closed, `session_registry_len()` is 0, and the journal still
   holds exactly one `Exited` upsert. Add the same race in the worker-backed
   configuration, because Hub runs worker-backed sessions.
6. **Ablation proof.** With the `notify_session` call in item 1 removed,
   the positive control must fail on the `wait_wakes` step. With the
   deferral in item 2 removed, the positive control must fail on the
   `session_registry_len() == 1` step or on `process_exit` delivery.
   Implement records both red runs in its evidence.
7. **Downstream-shaped proof.** Add one test to the isolated
   `hub-data-plane-shaped` consumer crate that runs an owner-loop-shaped
   `observe_lifecycle_slice` on the control path while a bound adapter
   route exists, then shows `process_exit` arriving only through
   `wait_pump` plus `pump_woken`. Keep the wrapper's source guards; add
   `list_terminal_subscriptions` to the forbidden list for that consumer.
   The wrapper is `crates/botster-core-test-support/tests/wake_pump_consumer_test.rs`.
8. **Documentation.** Update
   `docs/architecture/client-worker-terminal-egress.md` (the "Readback does
   not pump bound adapters" paragraph gains the wake and retirement rule),
   `docs/architecture/control-plane-lifecycle-journal.md` (observe with a
   bound route emits a session ingress wake and never `try_write`s), and the
   rustdoc on `observe_lifecycle_slice`, `observe_session_lifecycle`,
   `drain`, and `read_screen`.
9. **Publish the merged Core revision** in the Implement evidence so the
   parent Hub ticket can pin it.

Out of scope:

1. Any change inside `botster-hub`. Hub removes its post-request global
   adapter pump under `ticket_1787600679_990088` after it pins the merged
   revision.
2. Restoring `pump_bound_adapters`, `drain_runtime_once_without_pump`, or
   any global route scan. [[Hub terminal cold cut consumed Core 72d1c75]]
   keeps those deleted.
3. New wake kinds, new public enum variants, or changes to
   `TerminalWakeBatch`, `wait_wakes`, `wait_pump`, or `pump_woken`
   signatures.
4. Changes to attach phases, paste transactions, resize acknowledgment, or
   lifecycle paging shapes.
5. Host scanning of `list_terminal_subscriptions`. The ticket forbids this
   as a repair.

## Repository ownership boundaries and cross-repo dependencies

- Core owns the wake emission, the retirement rule, `ClientWorker` queue
  state, and the lifecycle commit. All changes stay in `botster-core` and
  `botster-core-daemon`, plus the Core-owned isolated consumer crates.
- Hub owns the owner loop cadence, the data-plane driver, and its Core pin.
  Hub is the consumer; the parent ticket `ticket_1787600679_990088` already
  exists on the Hub target and will pin the merged revision. No new
  dependency registration is needed. This Core ticket depends on nothing.
- `botster-tui`, `botster-web`, and `botster-workspaces` do not call the
  affected daemon methods.

## Assumptions and unknowns

Assumptions:

1. A session ingress wake is the correct targeted wake. It names exactly
   one session, and `pump_woken` already pumps only that session's bound
   routes. The ticket allows "the targeted adapter or ingress wake".
2. Gating the wake on "not in flight and pressure `Ready`" matches the
   ticket's "Ready bound adapter" wording. If Plan Review prefers an
   unconditional session wake after any bound queue growth, the only cost
   is one redundant pump against a non-Ready adapter, which consumes one
   unit of the 512 write budget.
3. The Hub-side symptom reproduces in Core with a fake `auto_complete()`
   adapter, because the stranding happens before any adapter call.

Unknowns for Implement:

0. Resolved in visit 3: a production adapter is not required to emit a
   writable wake at bind time, so the plan does not rely on one. Scope
   items 1 and 2 keep the session wake live for held frames and emit the
   bind-time `notify_session` while it is live.

1. Whether `CoreDaemon::drain` needs the wake for Hub today. Hub serves
   `drain_subscription` for unbound socket routes only. The plan includes
   `drain` for consistency because it shares the immediate-commit shape.
2. Whether the worker-backed positive control needs a `Ready`-then-exit
   ordering guard when the worker reports `ProcessExited` before the
   incremental attach finishes. Reuse the shape of
   `bound_adapter_receives_live_bytes_when_process_exits_during_incremental_attach`
   (`daemon_integration_test.rs:2969`) if it does.

## Affected surfaces and files

- `crates/botster-core/src/engine/client_worker.rs`: record bound-queue
  growth per session during ingest and held-flush; expose a crate-visible
  take; expose a crate-visible "session has undelivered frames" query
  (held, queued, or in-flight on any live owner) for the retirement rule;
  report from `bind_waking_terminal_adapter` whether the bound owner has
  held frames.
- `crates/botster-core/src/engine/managed_session_runtime.rs`: forward the
  take and the query; clear the delta inside `pump_woken_phase_one` or
  `pump_woken_phase_three`.
- `crates/botster-core/src/engine/botster.rs`: forward the take and query
  through both engine facades; ensure the deferred-session
  `drain_runtime_once` inside `pump_woken` does not leak a wake.
- `crates/botster-core-daemon/src/daemon.rs`: `observe_session`,
  `drain_runtime_for_readback`, `drain`, `commit_terminal_lifecycle`,
  `pump_woken`, `bind_waking_terminal_adapter` (bind-time
  `notify_session` for held frames), and the engine enum forwarding arms.
- `crates/botster-core-daemon/tests/terminal_wake_test.rs`: negative
  control and positive control.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`: existing
  observe and readback exit tests stay green; add the worker-backed race if
  it fits better there.
- `crates/botster-core-test-support/tests/consumers/hub-data-plane-shaped/src/lib.rs`
  and `crates/botster-core-test-support/tests/wake_pump_consumer_test.rs`.
- `docs/architecture/client-worker-terminal-egress.md`,
  `docs/architecture/control-plane-lifecycle-journal.md`.

Botster layers touched: Core runtime (`ClientWorker`, managed runtime, engine
facades) and Core daemon control plane. No plugin, Lua, SPA, TUI, or Hub
layer changes.

## Risks

1. **Spurious wakes from pump-time ingest.** If the delta is not cleared on
   the pump path, every pump would enqueue a second ingress wake. Tests with
   exact wake-batch counts (`one_slot_adapter_preserves_resize_input_and_echo_wake_obligations`,
   `pump_woken_same_wake_resize_then_input_survives_resize_completion`) would
   fail. Mitigation: clear the delta in the pump path and assert exact batch
   shapes in the positive control.
2. **Wake state leak.** If the retirement deferral never re-evaluates, a
   terminal session with a hard-stopped route or an abandoned declared owner
   could keep a live session wake state until `remove_session`. Mitigation:
   the deferral condition is evaluated on every commit for that session,
   including each owner-loop observe turn, and the positive control asserts
   `session_registry_len() == 0` after delivery. Add a hard-stop variant
   (adapter `force_closed` before the pump) and an abandoned-declaration
   variant (`unsubscribe` before bind, then one observe) that assert the
   same.
3. **Budget consumption on non-Ready adapters.** Covered by the `Ready` and
   not-in-flight gate; the negative control on a `block_writes()` fake
   asserts no extra `try_write` and no hard stop.
4. **Journal duplication.** The observe commit and the later pump commit
   must produce exactly one `Exited` upsert. Both controls count the upsert.
5. **Workspace flake.** [[botster core bounded waiting queue test flakes under workspace load]]
   applies; a flake needs matched base evidence and a later green workspace
   gate.

## Runtime-teardown lens answers

- `teardown_class_applies`: yes. The ticket changes when a session wake
  retires relative to `ClientWorker` route delivery, and it repairs a
  terminal-state versus live-runtime divergence (registry `Exited` while
  `process_exit` is undelivered).
- `teardown_isolation`: the ownership set is one session wake state plus
  that session's bound `ClientWorker` owners. Deferring retirement for one
  session touches no sibling session. A sibling route on the same session
  is pumped by the same ingress wake, which is the existing rule.
- `teardown_bounds`: no new waits or threads. Retirement is bounded by the
  next commit for the session: `process_exit` delivery, hard stop after the
  512 write budget or `Closed` pressure, `remove_session`, or shutdown.
  `notify_session` is non-blocking and coalesced.
- `late_message_matrix`: every surface that creates durable Core ownership,
  with its identity tag, its rejection after a terminal state or daemon stop,
  and its sweep when it races the exit or the retirement introduced here.
  Line numbers are at `72d1c75`.

  | Surface | Ownership created | Identity tag | Rejection after terminal or stop | Rollback or sweep, and race order |
  |---|---|---|---|---|
  | `spawn` (`daemon.rs:599`; runtime `local_process.rs:240`, `worker_process.rs:1592`) | registry row, engine session, session wake state | `SessionId` | `ensure_running` rejects after stop; a failed worker `start_writer` forgets the wake it allocated (`worker_process.rs:353`) | `remove_session` forgets the wake unconditionally (`daemon.rs:2046`, `managed_session_runtime.rs:954`). The deferred retirement in this plan is re-evaluated on every later commit for the session, so a session that exits with a bound route and then hard-stops still retires on that commit. Order: exit-first or observe-first both end at the same commit rule. |
  | `adopt_session` (`daemon.rs:1973`; runtime `worker_process.rs:1296`) | engine session and session wake state for a registry row from a previous daemon | `SessionId` plus recovery identity | `ensure_running`; missing worker path; incompatible protocol; oversized metadata rejects without touching the live worker | Same sweep as `spawn`. Adoption creates the wake state before any bound route exists, so the retirement deferral never applies to adoption itself. |
  | `expect_terminal_adapter` (`daemon.rs:960`; `client_worker.rs:224`) | pre-attach declaration in `expected_adapters` | `(ClientId, SessionId, SubscriptionId)` | `ensure_running` rejects after stop. A later `attach` that fails for absent, terminal, or control-plane-failed session cancels the declaration on the error arm (`daemon.rs:1023`). | `cancel_expected_terminal_adapter` (`daemon.rs:973`) retires an unconsumed declaration. A matching attach consumes it; a different client cannot consume it. No wake state is created by a declaration, so this change does not touch it. |
  | `attach` (`daemon.rs:997`) | live `ClientWorker` owner with a new generation; declared owners hold frames in `held` until bind | `(SessionId, SubscriptionId, TerminalSubscriptionGeneration)` plus `ClientId` | `ensure_running`, `ensure_session_mutable`, and `ensure_control_plane_live` reject; each rejection cancels the declaration | `unsubscribe` teardown and hard stop remove the owner. Race with exit: an unbound declared owner that receives `process_exit` into `held` keeps it there. Observe may commit `Exited`, but the held-aware deferral in Scope item 2 keeps the session wake live. The later bind emits `notify_session` (Scope item 1), the next pump flushes `held` and delivers `process_exit`, and that pump's commit retires the wake. An unbound undeclared owner hard-stops on `process_exit` at ingest, which is unchanged. |
  | `bind_waking_terminal_adapter` (`daemon.rs:1064`; `client_worker.rs:337`) | adapter on the live owner plus a route wake sink | live generation for the exact `(session, subscription)` and `ClientId` | Stopped daemon, unknown session, and failed control plane close and drop the adapter before delegation. `ClientWorker` rejects `BindBeforeAttach`, `UnknownSubscription`, `StaleGeneration`, and `AlreadyBound` and closes the adapter. | Hard stop calls `retire_and_hard_stop`, which retires the route sink and removes the owner. A successful bind onto an owner with non-empty `held` emits one `notify_session`. The deferred session-wake retirement counts only live owners, so a rejected or already hard-stopped bind never holds the session wake open. |
  | Non-pump drains (`observe_session`, `drain_runtime_for_readback`, `drain`) | none; these create no durable ownership | n/a | `ensure_running` rejects after stop | New in this plan: they emit one coalesced session ingress wake when they queue frames onto a bound Ready owner, and they leave the session wake live until that owner's frames are delivered or the owner is gone. A late ingress node for a retired session is still dropped in `assemble_batch`. |

  Acceptance checks that cover the matrix rows (existing tests stay green;
  new tests are marked):

  - Failed setup wake rollback:
    `failed_worker_start_does_not_leave_wake_registry_residue`
    (`crates/botster-core/tests/local_session_worker_process_test.rs`).
  - Stopped-daemon rejection: `late_spawn_and_waking_bind_after_shutdown_allocate_no_core_state`,
    `waking_bind_after_shutdown_closes_and_drops_adapter`,
    `waking_bind_for_unknown_session_closes_and_drops_adapter`, and
    `bind_rejection_allocates_nothing` (`terminal_wake_test.rs`).
  - Stale generation bind rejection: `bind_rejection_allocates_nothing` and
    `terminal_subscription_generation_is_exact_membership`
    (`daemon_integration_test.rs:6981`).
  - Late declaration cleanup: `absent_session_attach_cancels_pre_attach_declaration`
    (`daemon_integration_test.rs:190`),
    `terminal_session_attach_cancels_pre_attach_declaration`
    (`daemon_integration_test.rs:236`),
    `declaration_is_not_consumed_by_a_different_client`, and
    `matching_attach_consumes_a_declaration_so_later_attach_does_not_hold`
    (`client_worker_engine_test.rs`).
  - Adoption: `daemon_restart_adopts_live_worker_and_reattaches`
    (`daemon_integration_test.rs:5123`),
    `oversized_persisted_metadata_fails_adoption_without_touching_the_live_worker`,
    and `adoption_rejects_a_worker_from_the_previous_protocol`.
  - Sibling survival: `pump_failure_does_not_block_a_later_sibling`
    (`daemon_integration_test.rs:1148`) and
    `observe_retains_one_session_error_and_still_exits_a_later_sibling`
    (`daemon_integration_test.rs:5700`).
  - New, declared-unbound race (production path, no manual adapter wake):
    declare, attach, let the process exit, call `observe_session_lifecycle`
    before bind. Assert `Exited` is committed, the journal holds one
    `Exited` upsert, and `session_registry_len() == 1`. Bind an
    `auto_complete()` adapter and do not call the fake's `wake`. Assert
    `wait_wakes` returns an ingress batch that names the session, then
    `pump_woken` delivers `process_exit`, the adapter is closed, and
    `session_registry_len() == 0`. Red-on-revert: removing the held-aware
    deferral makes `session_registry_len() == 1` fail after observe;
    removing the bind-time `notify_session` makes `wait_wakes` time out.
  - New, bound race in both queue orders: exit observed by `pump_woken`
    first (existing `bound_adapter_keeps_live_bytes_across_repeated_process_exited_rounds`
    shape) and exit observed by `observe_session_lifecycle` first (the
    positive control in Scope item 5). Both end with one `Exited` upsert and
    `session_registry_len() == 0`.
  - New, hard-stop race: observe consumes the exit, then the adapter reports
    `Closed` before the pump. Assert the route hard-stops, the session wake
    retires on that pump's commit, and the journal holds one `Exited` upsert.
- `production_path_proof`: Hub owner loop `observe_lifecycle_slice` →
  `CoreDaemon::observe_session` → `drain_runtime_once` → ingest queues
  `process_exit` → `notify_session` → Hub data-plane `wait_pump` →
  `CoreDaemon::pump_woken` → `pump_one` `try_write` → `Ready` completion →
  `retire_and_hard_stop` → commit retires the session wake. Oracles:
  `try_write_count`, `adapter_has_process_exit`, adapter closed,
  `session_registry_len`, journal upsert count, plus the two ablation red
  runs and the `hub-data-plane-shaped` consumer test.
- `ownership_identity`: `(session_id, subscription_id, generation)` stays
  the owner key. The retirement query filters on live owners only, so a
  replaced generation cannot hold a stale wake open.
- `sibling_fail_closed_policy`: on delivery success, siblings continue. On
  hard stop of one route, only that route closes; the session wake retires
  when the last undelivered bound route for that session is gone.

## Acceptance checks and tests

Worktree preparation, once, not in the diff:

```bash
git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty
```

The worktree path has no colon, so `CARGO_TARGET_DIR` stays unset.

Repository gates, from the worktree root:

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
cargo test -p botster-core --no-default-features --lib
BOTSTER_ENV=test cargo test --doc --workspace
cargo test -p botster-core-daemon --test terminal_wake_test
cargo test -p botster-core-daemon --test daemon_integration_test
cargo test -p botster-core-test-support --test wake_pump_consumer_test
cargo test -p botster-core-test-support --test lifecycle_journal_consumer_test
```

Product proofs that Review and Verify require:

1. Negative control green: read-only and observe calls leave
   `try_write_count` unchanged on a bound `auto_complete()` adapter with
   queued output and an observed exit.
2. Positive control green in local and worker-backed configurations:
   observe before pump commits `Exited` once, keeps the session wake live,
   and `wait_wakes` plus `pump_woken` alone deliver `process_exit`, close
   the adapter, and retire the wake.
3. Both ablation runs red, with the failing assertion named.
4. Hard-stop variant green: `force_closed` before the pump still retires
   the session wake and leaves one journal upsert. Abandoned-declaration
   variant green: `unsubscribe` before bind, then one observe, retires the
   session wake.
4a. Declared-unbound race green through `wait_wakes` plus `pump_woken`
   only, with both red-on-revert checks named in the matrix section.
5. Non-Ready variant green: a `block_writes()` adapter receives no extra
   `try_write` from observe and is not hard-stopped by it.
6. Existing exact wake-count tests unchanged and green.
7. `hub-data-plane-shaped` consumer test green with
   `list_terminal_subscriptions` in its forbidden source list.
8. Implement evidence names the merged Core revision for the Hub pin.

## Plan Review findings addressed in visit 2

- `finding_1788525170_313374` (high, product): the late-message matrix now
  lists spawn, adoption, declaration and cancellation, attach, bind, and the
  non-pump drains, each with identity, rejection, and sweep, plus the
  acceptance checks that cover them.
- `finding_1788525170_452980` (low, process): [[cli-patterns]] and
  [[botster-runtime-reviewer-playbook]] are loaded and recorded.
- `finding_1788525170_788278` (low, process): the Hub context paragraph now
  separates current `origin/main` from the active parent branch and records
  the registered dependency.
- `finding_1788525170_769671` (low, process): this visit resubmits the gate
  with `plan_uri`, `artifact_id`, `checklist_id`, `target_id`, and
  `target_repository`, and reuses the one Plan vault checklist
  `checklist_1788524372_726191` created in visit 1.

## Plan Review findings addressed in visit 3

- `finding_1788525851_355196` (high, product): the session wake now stays
  live while any live owner, bound or unbound declared, holds undelivered
  frames. A successful bind onto an owner with held frames emits
  `notify_session` while that wake is live. The declared-unbound race check
  uses only `wait_wakes` plus `pump_woken`, with red-on-revert checks for
  both the deferral and the bind-time notification. Matrix, scope, risks,
  files, and acceptance checks are updated.
- `finding_1788525851_610128` (low, process): the gate resubmission keeps
  the artifact and checklist references.

## Vault gaps worth capturing

1. [[session ingress wakes retire on observed exit not shutdown acceptance]]
   needs a follow-up: retirement waits for bound-route delivery, not only
   for the observed exit. Capture after Implement lands.
2. A new gotcha: a non-pump Core drain that commits terminal lifecycle
   before the bound route pumps strands `process_exit`, because
   `assemble_batch` drops retired ingress nodes. Readback avoided this only
   through the obligation and rearm shape.
3. [[Hub terminal cold cut consumed Core 72d1c75]] is marked outdated and
   should record this Core ticket as the repair the Hub cold cut waits on.
