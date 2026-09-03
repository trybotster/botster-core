# Delete the Core polling adapter path after the Hub wake cutover

Ticket: `ticket_1787894967_973951`
Run: `run_1788280094_679374`
Plan visit: 2 (renewed after Plan Review `review_1788327888_688420`)

## Target repository

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Base ref: `main` at `48a4370` (`Enforce declared paste assembly bounds`), verified
  with `git fetch origin --prune`
- Previous plan base `e5a927c` is stale. This plan is rebased onto `48a4370`.
- Merge policy: direct to `main`

## Playbooks and notes loaded

Repository playbook:

- [[botster-core-playbook]]

Role playbooks:

- [[planner-playbook]]
- [[botster-planner-playbook]]

Required Botster context (added this visit):

- [[botster-architecture]]
- [[cli-patterns]]

Workflow policy (added this visit, because this Plan visit creates artifacts,
questions, dependencies, and gate evidence):

- [[project-pipelines-playbook]]

Class overlay (runtime-teardown class applies):

- [[botster runtime teardown lenses]]

Targeted atomic notes:

- [[core terminal progress is wake driven and targeted]]
- [[core waking terminal adapters shipped at revision ec589ee]]
- [[core ingress wake sources are transport neutral]]
- [[core owns bounded atomic terminal input transactions across clients]]
- [[core owns duplex terminal transport while Hub stays content blind]]
- [[terminal adapters emit coalesced writable and closed wakes]]
- [[session wake coalescing belongs in a lifecycle registry not each handle]]
- [[session ingress wakes retire on observed exit not shutdown acceptance]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core ClientWorker bind requires a live attach generation]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
- [[core holds declared attach frames until the bound adapter drains]]
- [[capacity parked terminal inputs retry only on matching session ingress wakes]]
- [[core one slot adapters preserve resize input and echo wake obligations]]
- [[control queue full retries terminal input in order while other failures fail closed]]
- [[every TerminalInputResult must stamp the live subscription id]]
- [[removing a delivery gate requires replacement ordering proof]]
- [[botster core contract surface needs consumer proof]]
- [[conformance harnesses need an implementation ablation at the contract oracle]]
- [[dead code allowances identify scaffold only entry points]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster core worktrees need the ghostty submodule initialized]]
- [[core terminal wake test binaries must run from the repository working directory]]
- [[Core token requirement changes update every documentation surface]]

### How the added notes constrain this plan

- [[botster-architecture]] fixes the layer: this is Core runtime and transport
  substrate, not Hub policy and not client presentation. It also names
  [[core owns bounded atomic terminal input transactions across clients]] as
  current Core truth, which is the contract the refreshed base added.
- [[cli-patterns]] keeps the change inside the Rust runtime and PTY layer, with
  deterministic Rust integration proof rather than wall-clock observation.
- [[project-pipelines-playbook]] governs the workflow surface this visit used:
  bind the run to the explicit target id, register cross-repository
  prerequisites against the dependency repository target, and submit complete
  gate evidence rather than a URI alone.

## Context loaded

- Ticket source capture: `ops/archive/inbox/2026-08-27-botster-wake-driven-data-plane-and-hub-decomposition.md`
- Core source at `48a4370`: `crates/botster-core/src/engine/client_worker.rs`,
  `crates/botster-core/src/engine/managed_session_runtime.rs`,
  `crates/botster-core/src/engine/botster.rs`,
  `crates/botster-core/src/contract/terminal_wake.rs`,
  `crates/botster-core-daemon/src/daemon.rs`
- Core test support: `crates/botster-core-test-support/src/conformance/`,
  `crates/botster-core-test-support/tests/consumers/`
- Core CI: `.github/workflows/ci.yml`
- Downstream `botster-hub` at `origin/main` `bb1a330`, which now pins Core
  `48a4370` uniformly across `botster-core`, `botster-core-daemon`,
  `botster-terminal-protocol`, `botster-core-test-support`, and
  `botster-terminal-ghostty`. Both Hub dependencies have closed, so the Hub
  baseline for this plan is `bb1a330` on Core `48a4370`, not the earlier
  `db2c43c` on `e5a927c`.
- Plan Review `review_1788327888_688420` and its five findings
- Sibling tickets `ticket_1788313897_932611` (Hub paste pin) and
  `ticket_1788112223_631570` (Core residual bind rejection gaps)
- Human answer `question_1788280396_580838` (coordinated two-ticket cold cut)

## What the refreshed base changed

Four merged commits (`8d9cb1c`, `58d328d`, `e065f75`, `48a4370`) added bounded
atomic terminal paste transactions across 44 files. The parts that touch this
deletion:

1. `SubscriptionOwner` gained `paste` and `paste_in_flight` assembly state with
   a deadline.
2. `ClientWorker::expire_pastes_keys` retires expired assemblies and enqueues a
   `Timeout` rejection. It is called from three places:
   `pump()` (global keys), `pump_woken()` (route keys), and
   `intake_terminal_input_keys()` (whatever keys its caller passed).
3. `ClientWorker::next_paste_deadline` and `ClientWorker::expired_paste_routes`
   expose the earliest deadline and the exact expired routes.
4. `ManagedSessionRuntime::clamp_paste_wait` shortens a host wait so no assembly
   sleeps past its deadline, and `expired_paste_wake_batch` synthesizes a
   targeted wake batch for expired assemblies.
5. `ManagedSessionRuntime::wait_wakes` and `CoreDaemon::wait_pump`
   (`daemon.rs:1147`, `daemon.rs:1171`) already apply the clamp and the
   synthesized batch.

Conclusion of the re-audit: **paste timeout already lives on the targeted wake
path.** Deleting `pump()` and the global `intake_terminal_input()` removes two
redundant global expiry drivers. The remaining drivers are `pump_woken`,
`intake_woken`, and the clamp plus synthesized batch, which together cover every
production wait. The deletion must preserve all three, and the plan adds
explicit acceptance for that.

Also confirmed during the re-audit: `TerminalWakeKind::Writable` is documented as
"capacity returned, **or the adapter otherwise has work Core should pump**"
(`crates/botster-core/src/contract/terminal_wake.rs:48`). Adapter-originated
input, including paste frames, therefore reaches Core through the existing
wake kinds. This deletion needs no new wake variant, which keeps
[[botster core public enums are breaking until non exhaustive is decided]]
satisfied.

## Current state that this plan removes

Line numbers are at `48a4370`.

1. `ClientWorker::bind_terminal_adapter` (`client_worker.rs:318`) binds a plain
   `TerminalAdapter` and sets `owner.waking = false`.
2. `ClientWorker::pump()` (`client_worker.rs:702`) scans every live route once
   per host tick, including its global paste expiry.
3. `ClientWorker::intake_terminal_input()` (`client_worker.rs:767`) scans every
   live route and calls `try_read` on each bound adapter.
4. `ManagedSessionRuntime::pump_bound_adapters()`
   (`managed_session_runtime.rs:1860`) and the private `apply_client_worker`
   call `ClientWorker::pump()`.
5. `ManagedSessionRuntime::drain_runtime_once` pumps bound adapters, while
   `drain_runtime_once_without_pump` (`managed_session_runtime.rs:1703`) does
   not. The private `drain_runtime_once_with(pump: bool)` carries that split.
6. Both `ManagedSessionRuntime::apply_terminal_input` implementations
   (`managed_session_runtime.rs:320` worker-backed, `:870` local) call the
   global `intake_terminal_input`.
7. `WorkerBackedBotsterEngine::drain_runtime_once_with` calls
   `pump_bound_adapters` at four points and switches drains on `pump_bound`.
8. The polling bind is published at four more layers:
   `DefaultBotsterEngine::bind_terminal_adapter` (`botster.rs:513`),
   `WorkerBackedBotsterEngine::bind_terminal_adapter` (`botster.rs:1175`),
   `ManagedSessionRuntime::bind_terminal_adapter`
   (`managed_session_runtime.rs:1429`), and
   `CoreDaemon::bind_terminal_adapter` (`daemon.rs:1048`, plus the engine enum
   arm at `daemon.rs:3620`).
9. `CoreDaemon::drain` calls `apply_terminal_input` before
   `drain_runtime_once`, so a plain `drain` intakes and pumps bound adapters.
   `CoreDaemon::drain_subscription` (`daemon.rs:1526`) delegates to `drain` and
   inherits that behavior.
10. `CoreDaemon::observe_session` calls `drain_runtime_once`, so bounded
    lifecycle observation also pumps bound adapters today.

## Scope

In scope:

1. Delete the polling bind path end to end at all five layers listed in item 8
   and item 1 above. `bind_waking_terminal_adapter` becomes the only bind.
2. Delete drain-time polling of bound adapters. Remove `ClientWorker::pump`,
   `ClientWorker::intake_terminal_input`,
   `ManagedSessionRuntime::pump_bound_adapters`, the `pump` and `pump_bound`
   switches, and the `drain_runtime_once_without_pump` twin. One
   `drain_runtime_once` remains, and it never touches a bound adapter.
3. Preserve every paste-transaction driver that survives on the wake path:
   `pump_woken`, `intake_woken`, `clamp_paste_wait`, `expired_paste_wake_batch`,
   `next_paste_deadline`, and `expired_paste_routes`. Deleting the two global
   expiry drivers must not weaken timeout, atomicity, one-result, or ordering
   behavior.
4. Remove the global adapter intake from both `apply_terminal_input`
   implementations. `CoreDaemon::drain` and `drain_subscription` keep unbound
   route egress, lifecycle observations, and backpressure, and stop driving
   bound adapters. These remain live production surfaces: Hub serves socket
   clients through `DaemonRequest::drain_subscription`.
5. Keep lifecycle observation transport neutral and bounded. `observe_session`,
   `observe_lifecycle_slice`, `observe_lifecycle`, and
   `observe_session_lifecycle` keep their current bounds and stop pumping bound
   adapters as a consequence of the single non-pumping `drain_runtime_once`.
6. Remove state that the deletion makes dead, including the `owner.waking` flag
   and any branch that only separated polling owners from waking owners. Keep
   `WakingAdapterHolder` only if it still carries behavior.
7. Keep `TerminalAdapter` public. `WakingTerminalAdapter` is its supertrait, and
   the published `assert_terminal_adapter_conformance` harness still proves the
   base trait laws.
8. Migrate every Core test that proves an ownership, capability, pressure, hold,
   paste, teardown, or input law to `bind_waking_terminal_adapter` plus wake
   driven progress. Delete only tests whose sole subject is the removed polling
   progress.
9. Migrate the isolated consumer crate
   `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped` to the
   waking bind and `wait_wakes` / `pump_woken`.
10. Add a source guard test that fails when the polling bind or drain-time
    adapter pumping returns to Core source.
11. Update the documentation surfaces that describe the removed path:
    `README.md`, `docs/architecture/terminal-adapter.md`,
    `docs/architecture/client-worker-terminal-egress.md`,
    `docs/architecture/engine-command-surface.md`, and the rustdoc on every
    changed public item.
12. Publish the exact merged Core revision for the final Hub integration.

Out of scope:

1. Any change inside `botster-hub`, including its Core pin. Hub owns those.
2. The two residual bind rejection gaps owned by `ticket_1788112223_631570`.
   See "Sibling scope" below.
3. Any change to the paste transaction contract itself: frame kinds, bounds,
   assembly, commit, abort, rejection taxonomy, and result shape stay as
   `48a4370` shipped them.
4. Any change to the wake contract shapes: `TerminalWakeKind`,
   `TerminalWakeSource`, `TerminalWakeSink`, `SessionWakeHandle`,
   `TerminalWakeBatch`, `TerminalWakeRoute`, `wait_wakes`, `wait_pump`,
   `wake_pump_control`, and `pump_woken`.
5. Any new configurability, feature flag, or migration window. This is a cold
   cut.
6. Lifecycle paging, baselines, journals, registry persistence, and attach phase
   machinery beyond the removal of adapter pumping.
7. `botster-tui`, `botster-web`, and `botster-workspaces`. Verified: no
   `bind_terminal_adapter` call exists in the `botster-tui` checkout.

## Repository ownership boundaries and cross-repository dependencies

- Core owns the terminal subscription state machine, the bind ladder, the wake
  contract, the paste transaction, and the targeted pump. Core owns this
  deletion.
- Hub owns admission, its data-plane driver, its pins, and its tests. Core must
  not edit Hub for this ticket.

Dependencies:

| Ticket | Target | Status | Why |
|---|---|---|---|
| `ticket_1787894427_525056` | botster-hub | closed | Hub cold-cut wake-driven duplex terminal transports |
| `ticket_1787603671_590198` | botster-hub | closed | superseded Hub Unix duplex work |
| `ticket_1788280452_111197` | botster-hub `tgt_7e208a0c76a44980a83b63af976b1f22` | closed, merged at `db2c43c` | moved Hub bound-adapter test progress onto the wake driver |
| `ticket_1788313897_932611` | botster-hub `tgt_7e208a0c76a44980a83b63af976b1f22` | closed | Hub ingress validates every terminal input frame header against its pinned protocol crate, so Hub rejected paste frame kinds 4..7 until it pinned the merged Core revision. Registered as `dependency_1788328056_742915`; Hub `bb1a330` now pins Core `48a4370`, so the downstream proof can run. |

Sibling scope (same repository, not a dependency):
`ticket_1788112223_631570` owns two residual gaps at `CoreDaemon`, including the
bind rejection arms that drop an adapter without an explicit `close()`. This
plan does not fix that gap and does not claim it is fixed. This deletion does
remove `CoreDaemon::bind_terminal_adapter`, which is one of the two functions
that ticket names, so its planner must re-scope onto
`bind_waking_terminal_adapter` alone. The Implementer must not silently absorb
that fix.

Downstream proof duty stays with Core: build and run the full locked Hub suite
against the Core deletion candidate before Core merges.

## Runtime-teardown class answers

`teardown_class_applies`: yes. The change removes a bind path and a pump path
for live terminal subscriptions, so subscription ownership, hard stop, paste
assembly retirement, and sibling isolation all move onto the wake path alone.

`teardown_isolation`: the ownership set is one `OwnerKey`
(`session_id`, `subscription_id`) plus its `TerminalSubscriptionGeneration`,
bound adapter, capability set, snapshot phase state, capacity-parked entry,
paste assembly state (`paste`, `paste_in_flight`), and wake route. One route's
hard stop must close and drop only that adapter, retire only that assembly, and
unsubscribe only that multiplexer route. Healthy siblings on the same session
keep their adapters, queued input, in-flight assemblies, and wake routes. The
deletion must not widen any teardown to a session-wide or global sweep.

`teardown_bounds`: Core close stays synchronous, non-blocking, and on the host
tick. No new wait, join, or timeout enters Core. Two existing bounds must
survive the deletion: the 512 rejected `Writable` pump bound that ends a blocked
route, and the paste assembly deadline, which is enforced by
`clamp_paste_wait` plus `expired_paste_wake_batch` rather than by any global
scan. Removing `pump()` must not remove either bound.

`late_message_matrix`:

| Ownership-creating message | Owner tag | Rejection after terminal failure | Residual sweep |
|---|---|---|---|
| `attach` | client id, session id, subscription id, new generation | control-plane failure and unknown session return typed errors | pending drain drop plus expected-adapter cancel |
| `expect_terminal_adapter` (pre-attach declaration) | client id, session id, subscription id | a declaration for a different client is not consumed | every attach return consumes or rejects the declaration, except the residual case owned by `ticket_1788112223_631570` |
| `bind_waking_terminal_adapter` | live generation plus client id | `ControlPlaneFailed` and the whole `ClientWorker` ladder (`BindBeforeAttach`, `UnknownSubscription`, `StaleGeneration`, `AlreadyBound`) call `adapter.close()` then drop, and allocate no wake state. **The two earlier `CoreDaemon` guards, `ensure_running()?` and `ensure_session()?`, return before any explicit close, so the boxed adapter is dropped without `close()`.** That drop-only policy is current unchanged behavior owned by `ticket_1788112223_631570`; this deletion neither fixes nor worsens it. | a rejected bind retires any route it allocated |
| `TerminalInput`, `Resize`, `ModeGatedInput` | subscription id stamped on every `TerminalInputResult` | control-queue-full retries in order; other failures fail closed and hard-stop the owner | capacity-parked entries retain only on a matching session ingress wake and a live generation |
| `PasteBegin`, `PasteChunk`, `PasteCommit`, `PasteAbort` | operation id plus mode generation and revision on the live owner | admission, mode, duplicate, stale-generation, over-count, and incomplete failures deliver zero PTY bytes and emit exactly one result | deadline expiry retires the assembly and enqueues one `Timeout` rejection, driven by `pump_woken`, `intake_woken`, and the clamped wait |
| adapter `Writable` wake (capacity **or** readable work) | `TerminalWakeRoute` with session, subscription, generation | a stale generation route pumps nothing | wake route retires on teardown |
| adapter `Closed` wake | same route identity | one idempotent teardown | route retire plus `UnsubscribeSession` |
| session ingress wake | session id | retires on observed exit or runtime removal, not on shutdown acceptance | overflow reconcile walk reuses the readiness filter |

The removed polling bind carried no admission rule the waking bind lacks. The
deletion removes matrix rows and adds none.

`production_path_proof`: the production path after this change is worker or PTY
ingress, or an adapter writable or closed transition, or a clamped paste
deadline, then `CoreDaemon::wait_wakes` or `CoreDaemon::wait_pump`, then
`CoreDaemon::pump_woken`, then targeted per-route intake, apply, and egress.
Proof runs against a real worker PTY in
`crates/botster-core-daemon/tests/terminal_wake_test.rs`. The ablation is the
deletion itself: after the change, a test that only calls `drain`,
`drain_subscription`, `observe_lifecycle_slice`, `observe_session_lifecycle`, or
a snapshot readback must observe no bound-adapter progress. The existing
`readback_does_not_advance_bound_adapter` test extends to cover `drain`,
`drain_subscription`, and bounded lifecycle observation.

`ownership_identity`: every durable row keeps session id, subscription id, and
generation, and every `TerminalInputResult` keeps its live subscription id
stamp. A paste assembly is additionally keyed by operation id and its mode
generation and revision. A reused subscription id receives a fresh generation
and a fresh wake gate, and cannot inherit a previous assembly. A retained sink
clone cannot restore a forgotten `SessionId`.

`sibling_fail_closed_policy`: on a successful close, siblings keep working. A
route that exhausts the rejected-writable bound hard-stops alone and preserves
siblings. A paste timeout or rejection stops one operation on one owner and
leaves sibling owners and sibling sessions untouched. Control-plane failure
fails that session's owners closed and leaves other sessions untouched. No new
sibling-sacrifice arm enters Core with this change.

## Assumptions and unknowns

Assumptions:

1. Hub prerequisite `ticket_1788280452_111197` merged at `db2c43c` and satisfies
   its own acceptance. Any residual Hub reliance on Core drain for bound-adapter
   progress will surface in the required full locked Hub suite run.
2. Hub dependency `ticket_1788313897_932611` has closed. Hub `bb1a330` pins Core
   `48a4370` for every Core-family crate, so the downstream proof can start from
   that Hub revision.
3. `CoreDaemon::drain` and `drain_subscription` stay public for unbound routes,
   because Hub serves socket clients through
   `DaemonRequest::drain_subscription`.
4. `TerminalAdapter` stays public because `WakingTerminalAdapter` requires it
   and the published harness proves its laws.
5. `botster-terminal-protocol`, `botster-terminal-protocol-client`, and the npm
   packages need no change, because the wire protocol does not describe the bind
   path.
6. Removing `CoreDaemon::bind_terminal_adapter` is a breaking change to a
   published Core surface, taken deliberately through the coordinated cold cut
   agreed in `question_1788280396_580838`.

Unknowns for the Implementer to resolve:

1. The exact migrate-versus-delete split across the polling bind call sites in
   `crates/botster-core/tests/client_worker_engine_test.rs`. The default is
   migrate; deletion needs a stated reason per test.
2. Whether `WakingAdapterHolder` still earns its indirection once `owner.waking`
   is gone.
3. Whether the local-process `apply_terminal_input` has any remaining caller
   after the global intake is removed, or becomes dead and must go.
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

1. Paste regression. `pump()` and the global intake each drive
   `expire_pastes_keys`. Removing both leaves the clamped wait and the targeted
   pumps as the only expiry drivers. A missed clamp path would let an assembly
   outlive its deadline. Mitigation: explicit acceptance on the paste timeout,
   atomic delivery, one-result, and stale-mode behaviors over the wake path.
2. Hidden production dependence. `CoreDaemon::drain` currently advances bound
   adapters as a side effect. Mitigation: real-worker wake tests plus the full
   locked Hub suite.
3. Test-law loss. Migrating the polling bind call sites can silently drop an
   oracle. Mitigation: migrate rather than rewrite, and keep every assertion.
4. Hard-stop bound regression. The 512 rejected-writable bound and the
   process-exit-then-close order live on the pump path. Mitigation: keep those
   tests and run them on the `pump_woken` path.
5. Lifecycle regression. Making `drain_runtime_once` non-pumping changes what
   `observe_session` does. Mitigation: assert bounded observation still advances
   lifecycle and still does not advance bound adapters.
6. Public API break. Mitigation: verified `botster-tui`, `botster-web`, and
   `botster-workspaces` do not call the removed bind, and Hub already binds
   waking.
7. Ordering proof gap. [[removing a delivery gate requires replacement ordering proof]] applies: removing drain-time pumping needs same-batch ordering proof
   for snapshot, attached, live output, and input echo.
8. Cross-ticket collision. `ticket_1788112223_631570` edits the same
   `CoreDaemon` bind functions. Mitigation: the ownership table above, and no
   silent absorption of that fix.
9. Stale downstream proof. Hub cannot compile against a Core revision carrying
   paste frames until `ticket_1788313897_932611` lands. Mitigation: that ticket
   is now a registered dependency, not a caveat.

## Acceptance checks and tests

Core gates, per [[botster-core uses CI-owned Cargo commands because it has no test script]]:

1. Initialize the ghostty submodule in the fresh worktree first, per
   [[botster core worktrees need the ghostty submodule initialized]].
2. `cargo fmt --all -- --check`
3. `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings`
4. `BOTSTER_ENV=test cargo test --workspace`
5. `cargo test -p botster-core --no-default-features --lib` (contract-only lane)
6. `BOTSTER_ENV=test cargo test --doc --workspace`
7. `cargo doc --workspace --no-deps`
8. `script/terminal-protocol-node-smoke.sh`
9. Run direct wake binaries from the repository working directory, per
   [[core terminal wake test binaries must run from the repository working directory]].

Core behavior proofs:

10. Real-worker proof that PTY bytes reach a bound waking adapter through only
    `wait_wakes` / `wait_pump` and `pump_woken`.
11. Extended negative proof: `drain`, `drain_subscription`,
    `observe_lifecycle_slice`, `observe_session_lifecycle`, and both snapshot
    readbacks advance no bound adapter.
12. Bounded lifecycle proof: bounded observation still advances lifecycle rows
    and still respects item, byte, and elapsed budgets.
13. Ordering proof on the wake path: snapshot phases, `Attached`, residual
    producer output, then queued input echo, in one batch order.
14. Hard-stop proof: 512 rejected `Writable` pumps hard-stop the blocked route
    and preserve siblings.
15. Process-exit proof: process exit is delivered before close, and the session
    survives.
16. Capability, hold, declaration, and teardown laws from
    `client_worker_engine_test.rs` pass under the waking bind.

Paste transaction preservation (required by the refreshed base):

17. Paste timeout: an assembly that passes its deadline is retired with exactly
    one `Timeout` rejection, driven only by the clamped wait plus
    `expired_paste_wake_batch` and `pump_woken`, with no global scan.
18. Clamp reachability: both production waits, `ManagedSessionRuntime::wait_wakes`
    and `CoreDaemon::wait_pump`, still clamp to `next_paste_deadline`.
19. Atomic delivery: a validated multi-frame paste delivers the bracketed opener,
    content, and closer as one ordered operation; a failed operation delivers
    zero PTY bytes.
20. One result: each paste operation emits exactly one `TerminalInputResult`,
    stamped with the live subscription id.
21. Partial-write hard-stop: an operating-system write failure after delivery
    begins stops only the affected owner.
22. Stale-mode recovery: a stale mode generation or revision rejects the
    operation and leaves the owner usable.
23. Sibling survival: a paste timeout, rejection, or hard stop on one owner
    leaves sibling owners and sibling sessions making progress.

Guards and consumers:

24. Source guard: a test fails when Core source reintroduces
    `fn bind_terminal_adapter`, `ClientWorker::pump(`,
    `intake_terminal_input(`, `pump_bound_adapters(`, or
    `drain_runtime_once_without_pump(`. The guard must go red when the deleted
    text is pasted back.
25. The isolated `hub-adapter-shaped` consumer passes using the waking bind and
    the published harness, and still constructs no published Core driver.
    A workspace test does not run this nested non-member crate, so run it one of
    these two ways, per [[botster-core-playbook]]:

    - The wrapper that starts it, from the repository working directory:

      ```bash
      BOTSTER_ENV=test cargo test -p botster-core-test-support         --test terminal_adapter_conformance_test         isolated_hub_shaped_consumer_runs_harness_against_its_own_adapter
      ```

      The wrapper shells out with `cargo test --quiet --offline`,
      `current_dir` set to the consumer directory, and `CARGO_TARGET_DIR` set to
      that consumer's own `target/`, which keeps its build isolated.

    - Or the direct command, which must reproduce the wrapper's isolation:

      ```bash
      cd crates/botster-core-test-support/tests/consumers/hub-adapter-shaped
      CARGO_TARGET_DIR="$PWD/target" cargo test --quiet --offline
      ```

      Do not run the direct form with the workspace `CARGO_TARGET_DIR`, and do
      not convert it to a workspace member to make a filter reach it.
26. Compile-fail proof that a `TerminalAdapter` that is not a
    `WakingTerminalAdapter` cannot bind.

Downstream proof, required before Core merge:

27. `botster-hub` at its `main` (`bb1a330` or later), with
    every Core-family revision (`botster-core`, `botster-core-daemon`,
    `botster-terminal-protocol`, `botster-core-test-support`,
    `botster-terminal-ghostty`) overridden to the same Core deletion candidate,
    compiles. Do not mix Core pins.
28. The full locked Hub suite passes at default concurrency against that
    candidate, including `tests/hub_daemon_lifecycle/webrtc_terminal_adapter.rs`
    and `tests/hub_daemon_lifecycle/unix_terminal_adapter.rs`. Use a colon-free
    worktree and do not set `CARGO_TARGET_DIR`, per
    [[Hub official gates must not set CARGO TARGET DIR]].
29. The Hub baseline is re-measured against the refreshed Core base. The earlier
    1363-test pass against Core `e5a927c` is not accepted as evidence.
30. Publish the exact merged Core revision hash in the Verify evidence for the
    final Hub integration.

## Vault gaps worth capturing

1. A successor note to [[core waking terminal adapters shipped at revision ec589ee]]
   recording the merged deletion revision and the removal of
   `CoreDaemon::bind_terminal_adapter`.
2. A note that Core drain, `drain_subscription`, and bounded lifecycle
   observation never advance a bound terminal adapter, so hosts must not use
   readback for terminal progress.
3. A note that paste assembly deadlines are enforced by a clamped host wait plus
   a synthesized targeted wake batch, not by a global scan, so no host may
   reintroduce a scan to expire assemblies.
4. A note that a Core cold cut of a published surface needs a downstream test
   migration ticket first, because downstream test seams outlive downstream
   production usage. This ticket pair is the worked example.
