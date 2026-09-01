# Delete the Core polling adapter path after the Hub wake cutover

Ticket: `ticket_1787894967_973951`
Run: `run_1788280094_679374`

## Target repository

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Base ref: `main` at `e5a927c`
- Merge policy: direct to `main`

## Playbooks and notes loaded

Repository playbook:

- [[botster-core-playbook]]

Role playbooks:

- [[planner-playbook]]
- [[botster-planner-playbook]]

Class overlay (runtime-teardown class applies):

- [[botster runtime teardown lenses]]

Targeted atomic notes:

- [[core terminal progress is wake driven and targeted]]
- [[core waking terminal adapters shipped at revision ec589ee]]
- [[core ingress wake sources are transport neutral]]
- [[terminal adapters emit coalesced writable and closed wakes]]
- [[session wake coalescing belongs in a lifecycle registry not each handle]]
- [[session ingress wakes retire on observed exit not shutdown acceptance]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core ClientWorker bind requires a live attach generation]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
- [[core holds declared attach frames until the bound adapter drains]]
- [[capacity parked terminal inputs retry only on matching session ingress wakes]]
- [[core one slot adapters preserve resize input and echo wake obligations]]
- [[removing a delivery gate requires replacement ordering proof]]
- [[botster core contract surface needs consumer proof]]
- [[conformance harnesses need an implementation ablation at the contract oracle]]
- [[dead code allowances identify scaffold only entry points]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster core worktrees need the ghostty submodule initialized]]
- [[core terminal wake test binaries must run from the repository working directory]]
- [[Core token requirement changes update every documentation surface]]

## Context loaded

- Ticket source capture: `ops/archive/inbox/2026-08-27-botster-wake-driven-data-plane-and-hub-decomposition.md`
- Core source: `crates/botster-core/src/engine/client_worker.rs`,
  `crates/botster-core/src/engine/managed_session_runtime.rs`,
  `crates/botster-core/src/engine/botster.rs`,
  `crates/botster-core-daemon/src/daemon.rs`
- Core test support: `crates/botster-core-test-support/src/conformance/`,
  `crates/botster-core-test-support/tests/consumers/`
- Core CI: `.github/workflows/ci.yml`
- Downstream: `botster-hub` `main`, which pins Core `e5a927c` for `botster-core`,
  `botster-core-daemon`, `botster-terminal-protocol`, `botster-core-test-support`,
  and `botster-terminal-ghostty`
- Human answer `question_1788280396_580838`, which selected a coordinated
  two-ticket cold cut

## Current state that this plan removes

1. `ClientWorker::bind_terminal_adapter` binds a plain `TerminalAdapter` and sets
   `owner.waking = false`.
2. `ClientWorker::pump()` scans every live route once per host tick.
3. `ClientWorker::intake_terminal_input()` scans every live route and calls
   `try_read` on each bound adapter.
4. `ManagedSessionRuntime::pump_bound_adapters()` and the private
   `apply_client_worker` call `ClientWorker::pump()`.
5. `ManagedSessionRuntime::drain_runtime_once` pumps bound adapters, while
   `drain_runtime_once_without_pump` does not. The private
   `drain_runtime_once_with(pump: bool)` carries that split.
6. `ManagedSessionRuntime::apply_terminal_input` (both the worker-backed and the
   local-process implementations) calls the global `intake_terminal_input`.
7. `WorkerBackedBotsterEngine::drain_runtime_once_with` calls
   `pump_bound_adapters` at four points and switches drains on `pump_bound`.
8. `DefaultBotsterEngine::bind_terminal_adapter`,
   `WorkerBackedBotsterEngine::bind_terminal_adapter`,
   `ManagedSessionRuntime::bind_terminal_adapter`, and
   `CoreDaemon::bind_terminal_adapter` publish that polling bind.
9. `CoreDaemon::drain` calls `apply_terminal_input` before
   `drain_runtime_once`, so a plain `drain` intakes and pumps bound adapters.
10. `CoreDaemon::observe_session` calls `drain_runtime_once`, so bounded
    lifecycle observation also pumps bound adapters today.

## Scope

In scope:

1. Delete the polling bind path end to end: `CoreDaemon::bind_terminal_adapter`,
   the engine enum arms in `daemon.rs`, `DefaultBotsterEngine::bind_terminal_adapter`,
   `WorkerBackedBotsterEngine::bind_terminal_adapter`,
   `ManagedSessionRuntime::bind_terminal_adapter`, and
   `ClientWorker::bind_terminal_adapter`. `bind_waking_terminal_adapter` becomes
   the only bind.
2. Delete drain-time polling of bound adapters. Remove `ClientWorker::pump`,
   `ClientWorker::intake_terminal_input`,
   `ManagedSessionRuntime::pump_bound_adapters`, the `pump` and `pump_bound`
   switches, and the `drain_runtime_once_without_pump` twin. One
   `drain_runtime_once` remains, and it never touches a bound adapter.
3. Remove the global adapter intake from both `apply_terminal_input`
   implementations. `CoreDaemon::drain` keeps unbound route egress, lifecycle
   observations, and backpressure, and stops driving bound adapters.
4. Keep lifecycle observation transport neutral and bounded.
   `observe_session`, `observe_lifecycle_slice`, `observe_lifecycle`, and
   `observe_session_lifecycle` keep their current bounds and stop pumping bound
   adapters as a side effect of the single non-pumping `drain_runtime_once`.
5. Remove state that the deletion makes dead, including the `owner.waking`
   flag and any branch that only existed to separate polling owners from waking
   owners. Keep `WakingAdapterHolder` only if it still carries behavior.
6. Keep `TerminalAdapter` public. `WakingTerminalAdapter` is its supertrait, and
   the published `assert_terminal_adapter_conformance` harness still proves the
   base trait laws.
7. Migrate every Core test that proves an ownership, capability, pressure,
   hold, teardown, or input law to `bind_waking_terminal_adapter` plus wake
   driven progress. Delete only tests whose sole subject is the removed polling
   progress.
8. Migrate the isolated consumer crate
   `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped` to the
   waking bind and `wait_wakes` / `pump_woken`.
9. Add a source guard test that fails when the polling bind or drain-time
   adapter pumping returns to Core source.
10. Update the documentation surfaces that describe the removed path:
    `README.md`, `docs/architecture/terminal-adapter.md`,
    `docs/architecture/client-worker-terminal-egress.md`,
    `docs/architecture/engine-command-surface.md`, and the rustdoc on every
    changed public item.
11. Publish the exact Core revision on `main` after merge, and record it for the
    final Hub integration.

Out of scope:

1. Any change inside `botster-hub`. The Hub test migration is prerequisite
   ticket `ticket_1788280452_111197`.
2. Any Hub pin bump. Hub owns its `Cargo.toml` revisions.
3. Any change to the wake contract itself: `TerminalWakeKind`,
   `TerminalWakeSource`, `TerminalWakeSink`, `SessionWakeHandle`,
   `TerminalWakeBatch`, `TerminalWakeRoute`, `wait_wakes`, `wait_pump`,
   `wake_pump_control`, and `pump_woken` keep their current shapes.
4. Any new configurability, feature flag, or migration switch. This is a cold
   cut, not a deprecation window.
5. Any change to lifecycle paging, baselines, journals, registry persistence, or
   attach phase machinery beyond the removal of adapter pumping.
6. `botster-tui`, `botster-web`, and `botster-workspaces`. Verified: no
   `bind_terminal_adapter` call exists in the `botster-tui` checkout.

## Repository ownership boundaries and cross-repository dependencies

- Core owns the terminal subscription state machine, the bind ladder, the wake
  contract, and the targeted pump. Core owns this deletion.
- Hub owns its admission policy, its data-plane driver, its pins, and its own
  tests. Core must not edit Hub for this ticket.
- Prerequisite dependency registered: `ticket_1788280452_111197`
  ("Hub: move bound-adapter test progress off Core drain onto wake pumps",
  target `tgt_7e208a0c76a44980a83b63af976b1f22`). Core implementation starts
  only after that ticket closes.
- Existing closed dependencies: `ticket_1787894427_525056` (Hub cold-cut
  wake-driven duplex terminal transports) and `ticket_1787603671_590198`
  (superseded Hub Unix duplex work).
- Downstream proof duty stays with Core: build and run the full locked Hub
  suite against the Core deletion candidate before Core merges.

## Runtime-teardown class answers

`teardown_class_applies`: yes. The change removes a bind path and a pump path
for live terminal subscriptions, so subscription ownership, hard stop, and
sibling isolation all move onto the wake path alone.

`teardown_isolation`: the ownership set is one `OwnerKey`
(`session_id`, `subscription_id`) plus its `TerminalSubscriptionGeneration`,
its bound adapter, its capability set, its snapshot phase state, its
capacity-parked entry, and its wake route. One route's hard stop must close and
drop only that adapter and unsubscribe only that multiplexer route. Healthy
siblings on the same session keep their adapters, their queued input, and their
wake routes. The deletion must not widen any teardown to a session-wide or
global sweep.

`teardown_bounds`: Core close stays synchronous, non-blocking, and on the host
tick. No new wait, join, or timeout enters Core. The existing rejected-writable
hard-stop bound (512 rejected `Writable` pumps) stays the bound that ends a
blocked route. Removing `pump()` must not remove that bound; it must survive on
the `pump_woken` path.

`late_message_matrix`:

| Ownership-creating message | Owner tag | Rejection after terminal failure | Residual sweep |
|---|---|---|---|
| `attach` | client id, session id, subscription id, new generation | control-plane failure and unknown session return typed errors | pending drain drop plus expected-adapter cancel |
| `expect_terminal_adapter` (pre-attach declaration) | client id, session id, subscription id | a declaration for a different client is not consumed | every attach return consumes or rejects the declaration |
| `bind_waking_terminal_adapter` | live generation plus client id | `BindBeforeAttach`, `UnknownSubscription`, `StaleGeneration`, `AlreadyBound`, `ControlPlaneFailed`; the presented adapter is closed and dropped, and no wake route is allocated | rejected bind retires any route it allocated |
| `TerminalInput`, `Resize`, `ModeGatedInput` from a bound adapter | subscription id stamped on every `TerminalInputResult` | control-queue-full retries in order; other failures fail closed and hard-stop the owner | capacity-parked entries retain only on a matching session ingress wake and a live generation |
| adapter `Writable` wake | `TerminalWakeRoute` with session, subscription, generation | a stale generation route pumps nothing | wake route retires on teardown |
| adapter `Closed` wake | same route identity | one idempotent teardown | route retire plus `UnsubscribeSession` |
| session ingress wake | session id | retires on observed exit or runtime removal, not on shutdown acceptance | overflow reconcile walk reuses the readiness filter |

The removed polling bind carried no distinct admission rule that the waking bind
lacks. The deletion therefore removes rows from this matrix but adds none.

`production_path_proof`: the production path after this change is
worker or PTY ingress, or adapter writable or closed transition, then
`CoreDaemon::wait_wakes` or `CoreDaemon::wait_pump`, then
`CoreDaemon::pump_woken`, then targeted per-route intake, apply, and egress.
Proof runs against a real worker PTY in
`crates/botster-core-daemon/tests/terminal_wake_test.rs`, and the ablation is
the deletion itself: after the change, a test that only calls `drain`,
`drain_subscription`, `observe_lifecycle_slice`, `observe_session_lifecycle`,
or a snapshot readback must observe no bound-adapter progress. The existing
`readback_does_not_advance_bound_adapter` test extends to cover `drain` and
bounded lifecycle observation.

`ownership_identity`: every durable row keeps the shipped identity of session
id, subscription id, and generation, and every `TerminalInputResult` keeps its
live subscription id stamp. A reused subscription id receives a fresh
generation and a fresh wake gate. A retained sink clone cannot restore a
forgotten `SessionId`.

`sibling_fail_closed_policy`: on a successful close, siblings keep working. On a
route that exhausts the rejected-writable bound, Core hard-stops that route
only and preserves siblings. Control-plane failure for a session fails that
session's owners closed and leaves other sessions untouched. No new
sibling-sacrifice arm enters Core with this change.

## Assumptions and unknowns

Assumptions:

1. Hub prerequisite `ticket_1788280452_111197` merges first, and Core
   implementation starts only after it closes. This follows human answer
   `question_1788280396_580838`.
2. `CoreDaemon::drain` and `drain_subscription` stay public for unbound routes.
   The ticket removes bound-adapter polling, not the drain API.
3. `TerminalAdapter` stays public because `WakingTerminalAdapter` requires it
   and the published conformance harness proves its laws.
4. The `botster-terminal-protocol` and `botster-terminal-protocol-client`
   crates and the npm packages need no change, because the wire protocol does
   not describe the bind path.
5. Removing `CoreDaemon::bind_terminal_adapter` is a breaking change to a
   published Core surface, and the coordinated two-ticket cold cut is the agreed
   way to take it.

Unknowns for the Implementer to resolve:

1. The exact split in `crates/botster-core/tests/client_worker_engine_test.rs`
   between laws to migrate and polling-only tests to delete. The default is
   migrate; deletion needs a stated reason per test.
2. Whether `WakingAdapterHolder` still earns its indirection once `owner.waking`
   is gone.
3. Whether the local-process `apply_terminal_input` has any remaining caller
   after the global intake is removed, or whether it becomes dead and must go.
4. The final home of the source guard test: the existing
   `crates/botster-core-test-support/tests/terminal_adapter_conformance_test.rs`
   or a new dedicated guard test.

## Affected surfaces and files

Core library:

- `crates/botster-core/src/engine/client_worker.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/src/engine/botster/takeover_fail_closed_tests.rs`
- `crates/botster-core/src/contract/terminal_subscription.rs` (rustdoc that
  names the removed bind)

Core daemon:

- `crates/botster-core-daemon/src/daemon.rs`

Core tests:

- `crates/botster-core/tests/client_worker_engine_test.rs`
- `crates/botster-core/tests/managed_session_runtime_test.rs`
- `crates/botster-core/tests/terminal_adapter_contract_test.rs`
- `crates/botster-core/tests/botster_engine_api_test.rs`
- `crates/botster-core/tests/multiplexer_engine_api_test.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `crates/botster-core-daemon/tests/terminal_wake_test.rs`

Core test support:

- `crates/botster-core-test-support/src/conformance/mod.rs`
- `crates/botster-core-test-support/tests/terminal_adapter_conformance_test.rs`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/src/lib.rs`

Documentation:

- `README.md`
- `docs/architecture/terminal-adapter.md`
- `docs/architecture/client-worker-terminal-egress.md`
- `docs/architecture/engine-command-surface.md`
- This plan under `docs/archive/plans/`

## Risks

1. Hidden production dependence. `CoreDaemon::drain` currently advances bound
   adapters as a side effect. If any Core-internal path relied on that, live
   output can stall. Mitigation: the real-worker wake tests plus the Hub suite.
2. Test-law loss. Migrating about 31 polling bind call sites can silently drop
   an oracle. Mitigation: migrate rather than rewrite, and keep every existing
   assertion.
3. Hard-stop bound regression. The 512 rejected-writable bound and the
   process-exit-then-close order live on the pump path. Mitigation: keep those
   tests and run them on the `pump_woken` path.
4. Lifecycle regression. Making `drain_runtime_once` non-pumping changes what
   `observe_session` does. Mitigation: assert bounded observation still
   advances lifecycle and still does not advance bound adapters.
5. Public API break. Downstream crates outside this project could call the
   removed bind. Mitigation: verified `botster-tui`, `botster-web`, and
   `botster-workspaces` do not call it, and Hub already binds waking.
6. Ordering proof gap. `removing a delivery gate requires replacement ordering
   proof` applies: removing drain-time pumping needs same-batch ordering proof
   that the wake path preserves snapshot, attached, live output, and input echo
   order.
7. Conformance consumer drift. The isolated `hub-adapter-shaped` consumer runs a
   nested `cargo test`. Its migration must keep it isolated and must not import
   published Core drivers.

## Acceptance checks and tests

Core gates, per [[botster-core uses CI-owned Cargo commands because it has no test script]]:

1. `git submodule update --init --recursive crates/botster-terminal-ghostty/vendor/ghostty`
   in the fresh worktree first.
2. `cargo fmt --all -- --check`
3. `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings`
4. `BOTSTER_ENV=test cargo test --workspace`
5. `cargo test -p botster-core --no-default-features --lib` (contract-only lane)
6. `BOTSTER_ENV=test cargo test --doc --workspace`
7. `cargo doc --workspace --no-deps`
8. Direct wake binaries run from the repository working directory.

Core behavior proofs:

9. Real-worker proof in `terminal_wake_test.rs` that PTY bytes reach a bound
   waking adapter through only `wait_wakes` / `wait_pump` and `pump_woken`.
10. Extended negative proof: `drain`, `drain_subscription`,
    `observe_lifecycle_slice`, `observe_session_lifecycle`, and both snapshot
    readbacks advance no bound adapter.
11. Bounded lifecycle proof: bounded observation still advances lifecycle rows
    and still respects item, byte, and elapsed budgets.
12. Ordering proof on the wake path for the removed drain pump: snapshot
    phases, `Attached`, residual producer output, then queued input echo, in one
    batch order.
13. Hard-stop proof on the wake path: 512 rejected `Writable` pumps hard-stop
    the blocked route and preserve siblings.
14. Process-exit proof: process exit is delivered before close, and the session
    survives.
15. Capability, hold, declaration, and teardown laws from
    `client_worker_engine_test.rs` pass under the waking bind.
16. Source guard: a test fails when Core source reintroduces
    `fn bind_terminal_adapter`, `ClientWorker::pump(`, `intake_terminal_input(`,
    `pump_bound_adapters(`, or `drain_runtime_once_without_pump(`. The guard
    must fail red when the deleted text is pasted back.
17. Isolated `hub-adapter-shaped` consumer passes using the waking bind and the
    published harness, and still constructs no published Core driver.
18. Compile-fail proof that a `TerminalAdapter` that is not a
    `WakingTerminalAdapter` cannot bind.

Downstream proof (required before Core merge):

19. `botster-hub` at its post-prerequisite `main`, with its Core git revisions
    overridden to the Core deletion candidate, compiles.
20. The full locked Hub suite passes against that candidate, including
    `tests/hub_daemon_lifecycle/webrtc_terminal_adapter.rs` and
    `tests/hub_daemon_lifecycle/unix_terminal_adapter.rs`. Use a colon-free
    worktree and do not set `CARGO_TARGET_DIR` for that Hub gate.
21. Publish the exact merged Core revision hash in the Verify evidence for the
    final Hub integration.

## Vault gaps worth capturing

1. A successor note to [[core waking terminal adapters shipped at revision ec589ee]]
   that records the merged deletion revision and states that
   `CoreDaemon::bind_terminal_adapter` no longer exists.
2. A note that Core drain and bounded lifecycle observation never advance a
   bound terminal adapter, so hosts must not use readback for terminal
   progress.
3. A note that a Core cold cut of a published surface needs a downstream test
   migration ticket first, because downstream test seams outlive downstream
   production usage. This ticket pair is the worked example.
