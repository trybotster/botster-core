# Core: retain attach frames until the bound adapter is Ready

Ticket: `ticket_1787768219_768283`
Run: `run_1787768249_446859`
Parent run: `run_1787678814_340532` (Hub ticket `ticket_1787600674_500120`)
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery` / `botster_stack_plan`
Base: pipeline worktree on `botster-core` at `358ef1a`, clean tracked worktree
Project: `project_1787600579_585482` Botster Isolated Subscription Data Plane

This plan chooses **required Core contract option 2** from the ticket. Core keeps
every initial attach frame inside Core until the host binds an adapter, and Core
delivers those frames through the normal `try_write` pump.

## Revision 2

Revision 2 addresses Plan Review `review_1787769585_709876`:

- `finding_1787769585_497487` (blocker, product): bind-time flush failure lacked a
  production hard-stop path. **The design changed.** Revision 1 flushed the held
  frames inside `bind_terminal_adapter`, which returns only
  `Result<(), BindTerminalAdapterError>` and therefore had no channel for a
  `ClientWorkerTeardown`. Revision 2 moves the flush to the head of
  `ingest_bound_terminal_frames`, which already returns
  `Vec<ClientWorkerTeardown>` and already runs inside
  `ManagedSessionRuntime::apply_client_worker_with`. Every flush failure now
  reaches `unsubscribe_owner_teardowns` and the production `UnsubscribeSession`
  handler through the existing path, with no new teardown channel and no change
  to the bind return type. See "Failure paths and production teardown".
- `finding_1787769585_513131` (high, product): the plan omitted
  [[botster-architecture]] and [[cli-patterns]]. Both are loaded in revision 2,
  and both changed the plan. See "How the required maps constrain this ticket".
- `finding_1787769585_950878` (medium, product): the isolated consumer gate now
  names an exact command. Revision 2 also corrects one factual detail in the
  suggested fix. See "Acceptance checks and tests".
- `finding_1787769585_421412` (low, process): Plan gate evidence is resubmitted
  with the required routing fields against the existing artifact
  `artifact_1787768711_739267` and the existing vault checklist
  `checklist_1787768558_452503`. No duplicate artifact and no duplicate checklist
  are created.

## Target repository and target_id

- Target repository: `botster-core`
- `target_id`: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Resolved from the ticket `target_id` through `list_spawn_targets`
  (`trybotster/botster-core`). The ambient worktree was not used as the routing
  source.
- Non-target repositories in this run: `botster-hub` (`tgt_7e208a0c76a44980a83b63af976b1f22`).
  The ticket forbids editing `botster-hub` here.

## Repository playbook loaded

- [[botster-core-playbook]]

## Other role/surface playbooks and atomic notes loaded

Role playbooks:
- [[planner-playbook]]
- [[botster-planner-playbook]]

Required Botster maps (added in revision 2, required by the planner overlay):
- [[botster-architecture]]
- [[cli-patterns]]

Class overlay (runtime-teardown class applies, see below):
- [[botster runtime teardown lenses]]

Targeted atomic notes:
- [[core owns duplex terminal transport while Hub stays content blind]]
- [[Core ClientWorker bind requires a live attach generation]]
- [[Core bind stores an immutable negotiated terminal capability set]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
- [[adapter accepted writes are not consumer flushed writes]]
- [[bound incremental attach drains live output before FINISH]]
- [[incremental attach drains residual producer output after Attached]]
- [[removing a delivery gate requires replacement ordering proof]]
- [[bound attach suppression skip is unproven without a red oracle]]
- [[attach routes use subscription scoped Core drains]]
- [[incremental GHOSTSNP attach streams READY history pages and FINISH]]
- [[Core owns the incremental attach phase machine]]
- [[Hub embeds CoreDaemon behind one client admission point]]
- [[botster subscriptions use dedicated ordered DataChannels]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster-core CI runs a contract only test lane because workspace feature unification hides breaks]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[a regression test must be shown to go red with the fix reverted]]
- [[vault example paths are not repository placement conventions]]

Reached through [[botster-architecture]] and [[cli-patterns]] in revision 2:
- [[session wide drains cannot deliver subscription owned initial state]]
- [[incremental attach snapshot frames require lossless streaming backpressure]]
- [[botster durable terminal egress is owned by sessionio and clientworker actors]]
- [[client workers own transport neutral stream state not hub orchestration]]
- [[botster data plane bypasses the hub through session and client actors]]
- [[botster dev harnesses must drive real engine types]]
- [[integration tests should use public agent apis not crate-internal test-only helpers]]
- [[narrow ablation at the enforcement point is the cleanest regression negative control]]
- [[test names do not prove their bodies can fail on the named claim]]
- [[PTY integration tests poll for readiness not fixed sleeps]]

[[project-pipelines-playbook]] is **not** loaded. This ticket changes no Project
Pipelines package or plugin path.

## Context loaded

Repository code read at `358ef1a`:
- `crates/botster-core/src/engine/client_worker.rs` (944 lines, full read)
- `crates/botster-core/src/contract/terminal_subscription.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  (`handle_client_ingress`, `attach_snapshot`, `begin_snapshot_attach`,
  `apply_client_worker_with`, `bind_terminal_adapter`, detach paths)
- `crates/botster-core/src/engine/botster.rs`
  (`DefaultBotsterEngine::attach_client`, `WorkerBackedBotsterEngine::attach_client`,
  `bind_terminal_adapter`)
- `crates/botster-core-daemon/src/daemon.rs` (`attach`, `bind_terminal_adapter`)
- `crates/botster-core/tests/client_worker_engine_test.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  (`bind_echo_worker`, `attach_bound_adapter`, `wait_until_bound_attached`)
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/src/lib.rs`
- `docs/architecture/client-worker-terminal-egress.md`
- `docs/architecture/terminal-adapter.md`, `docs/plans/README.md`, `README.md`

Pipeline context read: ticket, run, gates, project, sibling tickets, spawn targets.

### Confirmed defect mechanism

The ticket describes the loss as `attach()` extraction. The code shows a wider
window. `ClientWorker::ingest_bound_terminal_frames` runs on **every** host drain
tick through `ManagedSessionRuntime::apply_client_worker_with`. For a live owner
with `adapter.is_none()` it does three things
(`crates/botster-core/src/engine/client_worker.rs:373-386`):

1. It pushes the terminal frame back into `egress` (`retained`), so the frame
   leaves Core to the host.
2. It drops the pending `next_snapshot_phase` entry for `Snapshot` frames, so the
   READY / HISTORY / FINISH phase is lost.
3. It hard-stops the owner on an unbound `ProcessExit`.

`CoreDaemon::attach` (`crates/botster-core-daemon/src/daemon.rs:895-941`) then
splits those retained frames and returns the route-matching ones in
`AttachedSession.client_egress`. Worker-backed incremental attach emits READY,
HISTORY pages, FINISH, and `AttachState::Attached` across **later** drain ticks,
so the leak is not confined to the `attach` return value. Any fix that only
changes `CoreDaemon::attach` is incomplete.

This is why the Hub ticket has no correct option: Core hands the tail to Hub,
Hub may not queue it, and `bind_terminal_adapter` rejects a pre-attach bind with
`BindBeforeAttach` (`crates/botster-core/src/engine/client_worker.rs:241`).

### How the required maps constrain this ticket

[[botster-architecture]] and [[cli-patterns]] are loaded in revision 2. Four of
their linked notes change or confirm this plan.

1. [[session wide drains cannot deliver subscription owned initial state]] is the
   strongest constraint, and it **ratifies the chosen mechanism**. It states that
   attach must return `Attaching`, `Snapshot`, and `Attached` directly to the
   requesting route, and that where direct return is not possible the host must
   keep independent route-owned queues. A shared queue keyed only by session
   identity is insufficient. The per-owner hold is exactly a route-owned queue
   keyed by `client_id` plus `OwnerKey`. This note also confirms that the ticket's
   rejection of a Hub-owned queue does not mean "no queue"; it means the queue
   must be route-owned and Core-owned.
2. [[incremental attach snapshot frames require lossless streaming backpressure]]
   **changes the overflow policy statement**. Every incremental GHOSTSNP phase is
   protocol-critical. A bounded queue must never silently drop `READY`, a
   `HISTORY` page, or `FINISH`; it must either stall the producer or fail the
   connection explicitly. The hold therefore must not drop a phase to stay under
   capacity. Fail-closed teardown is the permitted behavior, and revision 2 states
   that requirement explicitly as a test, not only as a bound. Implement must also
   confirm that the existing producer-side pacing still applies: worker-backed
   attach already rejects a new attach when `pending` reaches
   `QueueSource::ClientWorker.default_capacity()`
   (`crates/botster-core/src/engine/botster.rs:935-948`), and keeps one snapshot
   frame per host tick.
3. [[botster durable terminal egress is owned by sessionio and clientworker actors]]
   and [[client workers own transport neutral stream state not hub orchestration]]
   confirm the ownership placement. Initial history replay follows the same
   ownership boundary as live bytes, and per-client pressure must affect only that
   client worker and adapter. The hold lives in `SubscriptionOwner`, so pre-bind
   retention cannot hold up the session I/O actor or a sibling route.
4. [[botster dev harnesses must drive real engine types]],
   [[integration tests should use public agent apis not crate-internal test-only helpers]],
   and [[test names do not prove their bodies can fail on the named claim]]
   **raise the acceptance bar**. Proof must drive `CoreDaemon` and the public
   engine API, not `ClientWorker` helpers, and each named claim needs an
   executable body oracle. [[PTY integration tests poll for readiness not fixed sleeps]]
   requires the existing `wait_until_bound_attached` polling shape rather than a
   fixed sleep. [[narrow ablation at the enforcement point is the cleanest regression negative control]]
   fixes the shape of the red-on-revert control: a one-line bypass at the hold
   branch, not a broad revert.

### Why option 1 is rejected

Option 1 ("bind before attach") does not fit the frozen Hub sequence. Hub admits
the subscription, attaches, returns the channel label and generation, and only
then does the browser create the DataChannel. The adapter object does not exist
at attach time. A placeholder adapter bound before the channel opens would report
`Full` or `WouldBlock` on every pump tick and would hit the existing
`WRITE_ATTEMPT_BUDGET` of 512 unsuccessful writes
(`crates/botster-core/src/engine/client_worker.rs:31`) before the browser opens
the channel. Option 1 also requires a reservation generation, which
[[Core ClientWorker bind requires a live attach generation]] rules out.

Option 2 keeps the shipped bind contract unchanged.

## Scope

Core keeps terminal egress for one subscription inside the ClientWorker owner
until the host binds an adapter, when and only when the host declared before
attach that it will bind one.

In scope:

1. Add a pre-attach declaration to `ClientWorker`:
   - `expect_terminal_adapter(client_id, session_id, subscription_id)` records
     that the next attach for that exact identity will bind an adapter.
   - `cancel_expected_terminal_adapter(client_id, session_id, subscription_id)`
     retires an unconsumed declaration.
   - `record_attach` consumes a matching declaration and sets
     `hold_until_bound = true` on the new owner. A declaration for a different
     `client_id` is not consumed and does not apply.
2. Add a bounded per-owner pre-bind hold in `SubscriptionOwner`:
   - `held: VecDeque<(TransportEgress, Option<SnapshotPhase>)>`.
   - Capacity is the existing `QueueSource::ClientWorker.default_capacity()`, the
     same bound the bound-adapter `queue` uses. Overflow hard-stops the owner and
     emits the existing `ClientWorkerTeardown`, matching current bound-queue
     overflow behavior.
3. Change the unbound branch of `ingest_bound_terminal_frames`:
   - When `hold_until_bound` is true, move the frame plus its
     `next_snapshot_phase` value into `held` instead of `retained`.
   - `AttachState { state: Detached }` keeps its current retained-to-host path.
     Core never writes it to an adapter (`encode_terminal_frame` returns
     `Ok(None)` for it), so holding it would strand a control-plane frame.
   - `ProcessExit` for a holding owner is held and marks
     `process_exit_enqueued`. It does **not** hard-stop the owner before bind.
   - When `hold_until_bound` is false, behavior is byte-for-byte unchanged.
4. Flush on the **next ingest tick**, not inside `bind_terminal_adapter`:
   - `bind_terminal_adapter` only installs the adapter and the capability set, as
     it does today. Its signature, its error enum, and its return type are
     unchanged, and it never needs to emit a `ClientWorkerTeardown`.
   - `ingest_bound_terminal_frames` begins with a flush pass over every owner that
     now has an adapter and a non-empty `held`. The pass drains `held` in order
     through the existing `encode_terminal_frame` with the negotiated
     `TerminalCapabilitySet`, pushing each `Some` result onto `owner.queue` and
     preserving `QueuedKind`.
   - The flush runs before that tick's new frames are ingested, so held frames
     always precede live frames in `owner.queue`. `pump` runs after ingest in
     `apply_client_worker_with`, so delivery starts on the same host tick.
   - `Ok(None)` results are dropped, exactly as the bound path drops
     capability-gated `Snapshot` frames today.
   - `Err(())` during the flush hard-stops the owner and returns the teardown in
     the `Vec<ClientWorkerTeardown>` that `ingest_bound_terminal_frames` already
     returns. See "Failure paths and production teardown".
   - `hold_until_bound` becomes false once the flush completes.
5. Clear `held` in `hard_stop` alongside `queue`.
6. Thread the two new methods through the existing passthrough layers:
   `ManagedSessionRuntime`, `DefaultBotsterEngine`, `WorkerBackedBotsterEngine`,
   `BotsterEngine`, and `CoreDaemon`.
7. Document the contract in `docs/architecture/client-worker-terminal-egress.md`
   and note in `CoreDaemon::attach` rustdoc that `AttachedSession.client_egress`
   is empty for a declared adapter route.
8. Tests listed under acceptance checks, including a red-on-revert control.

## Failure paths and production teardown

This section answers `finding_1787769585_497487`.

### The teardown handoff

There is no new teardown channel and no change to the public bind surface.
`ClientWorker::ingest_bound_terminal_frames` already returns
`Vec<ClientWorkerTeardown>`, and `ManagedSessionRuntime::apply_client_worker_with`
already consumes it
(`crates/botster-core/src/engine/managed_session_runtime.rs:1383-1401`):

```
teardowns.extend(self.client_worker.ingest_bound_terminal_frames(&mut outcome.client_egress));
teardowns.extend(self.client_worker.pump());
teardowns.splice(0..0, std::mem::take(&mut self.pending_input_teardowns));
self.unsubscribe_owner_teardowns(outcome, &mut teardowns)
```

`unsubscribe_owner_teardowns` then drives the production
`TransportIngress::UnsubscribeSession` handler for each teardown
(`managed_session_runtime.rs:1403-1428`). Putting the flush at the head of
`ingest_bound_terminal_frames` therefore places every flush failure on the exact
production teardown path with no new plumbing. Revision 1's bind-time flush had
no such path, which is why the design changed.

`CoreDaemon::bind_terminal_adapter` performs no drain
(`crates/botster-core-daemon/src/daemon.rs:947-971`), so the flush lands on the
host's next drain tick. Held frames stay held until then, so nothing is lost.

### The three failure classes

| Class | Reachable in production | Path to idle |
| --- | --- | --- |
| Hold overflow at ingest | Yes. A declared route that attaches and never binds while the worker produces more than `QueueSource::ClientWorker.default_capacity()` frames | `ingest_bound_terminal_frames` returns the teardown on the same tick, `apply_client_worker_with` splices it, `unsubscribe_owner_teardowns` runs `UnsubscribeSession`, `hard_stop` closes and drops the adapter and clears `held` and `queue`, the owner leaves `list_terminal_subscriptions` |
| Adapter closed or failing after bind | Yes. The DataChannel can close between reservation and bind, or during the dump | `pump` observes `Closed` or exhausts `WRITE_ATTEMPT_BUDGET` and hard-stops through the same `apply_client_worker_with` path |
| Encode failure during the flush | Defensive only. `encode_terminal_frame` fails when `TerminalEvent::to_frame` fails, which well-formed frames do not reach | Same tick, same return vector, same `unsubscribe_owner_teardowns` path |

Queue overflow **during** the flush is not a fourth class. `owner.queue` is
provably empty before the first successful bind, because the only producers into
it are `ingest_bound_terminal_frames` for a bound owner and
`enqueue_input_result`, and the latter is reachable only after Stage A intake,
which requires a bound adapter. `held` and `queue` share one capacity constant,
so the flush cannot exceed it. Implement keeps the defensive capacity check and
routes it to the same hard-stop, but the plan does not claim it as a reachable
production path.

### Required production proof for the failure paths

The first two classes are drivable end to end and carry the live proof. Both
tests use `CoreDaemon` with a real worker PTY, per
[[botster dev harnesses must drive real engine types]]:

- **Overflow teardown**: declare, attach, never bind, drive the worker past the
  hold capacity, then drain. Assert exactly one teardown, the owner absent from
  `list_terminal_subscriptions`, the route unsubscribed through the production
  handler, and a sibling subscription on the same session still delivering.
- **Closed adapter at bind**: declare, attach, accumulate a held dump, bind an
  adapter that already reports `Closed`, then drain. Assert the held dump is
  discarded, exactly one teardown flows through the production unsubscribe path,
  the adapter is closed and dropped, and a sibling route is unaffected.

The third class is proved at the `ClientWorker` level with an injected encode
failure, asserting the teardown appears in the value
`ingest_bound_terminal_frames` returns. The plan states plainly that this class
is defensive and that the live proof comes from the first two.

## Non-scope

- No `botster-hub` edit. The Hub consumer ticket `ticket_1787600674_500120`
  consumes this after the ticket closes.
- No Hub-owned queue and no Hub `VecDeque`. Core holds every frame.
- No change to `bind_terminal_adapter` semantics: `BindBeforeAttach`,
  `UnknownSubscription`, `StaleGeneration`, `AlreadyBound`, and
  `ControlPlaneFailed` keep their exact meanings and no variant is added.
  [[botster core public enums are breaking until non exhaustive is decided]]
  makes a new variant breaking, and none is needed.
- No wire-format change. `PROTOCOL_VERSION` stays `1` and no terminal frame type
  changes. No `@trybotster/terminal-protocol` release.
- No change to the unbound JSON drain path used today by Web, TUI, and Unix
  clients. Those consumers have not migrated; sibling tickets
  `ticket_1787600676_914408`, `ticket_1787603671_590198`, and
  `ticket_1787603674_865638` own that migration.
- No new configurability, no timeout policy inside Core for an unbound hold, and
  no second limit table. Host detach and the existing capacity bound are the
  backstops.
- No adjacent cleanup in `client_worker.rs` and no module extraction.
- No wall-clock measurement. The Web baseline ticket owns timing observations.

## Repository ownership boundaries and cross-repo dependencies

- **Core owns** terminal attach, snapshot delivery, ordering, bounded queues,
  pressure, generation, and teardown, per
  [[core owns duplex terminal transport while Hub stays content blind]]. Retaining
  the initial attach frames until the adapter exists is Core-owned mechanism, not
  host policy. This plan moves a responsibility back inside the correct owner.
- **Hub owns** admission, the route reservation, the channel label, the
  content-blind adapter object, and the decision to declare an adapter before
  attach. Hub stays content blind: it never inspects, queues, or reorders a
  terminal frame.
- **Seam**: the new declaration is a host-called Core API. Hub calls
  `expect_terminal_adapter` at admission before `attach`, and
  `cancel_expected_terminal_adapter` when it retires a Reserved route that never
  attached. Both calls are identity-only and carry no terminal payload.
- **Cross-repository dependency direction**: Hub ticket
  `ticket_1787600674_500120` depends on this Core ticket. This ticket depends on
  no other repository. No new dependency ticket is required against
  `botster-hub`; the parent ticket already exists and already tracks the consumer
  work.
- **Non-broadening**: no Hub, Web, or TUI file is touched here.

## Runtime-teardown class

`teardown_class_applies`: **yes**. The change alters ClientWorker subscription
ownership state, an unbound `ProcessExit` teardown trigger, and the frames a
hard-stop discards. Per [[botster runtime teardown lenses]] this is
SessionIo/ClientWorker teardown and terminal-state versus live-runtime
divergence, so every lens is answered below.

`teardown_isolation`: the ownership set is one `OwnerKey`
(`session_id`, `subscription_id`) plus its `generation`, per
[[Core terminal subscription ownership is session, subscription, and generation]].
The hold buffer lives inside that one `SubscriptionOwner`. A hold overflow, a
flush encode failure, or an adapter close removes exactly that owner and emits one
`ClientWorkerTeardown`. Sibling subscriptions on the same session and other
sessions keep their queues, their holds, and their adapters. No shared buffer is
introduced, so no healthy sibling is sacrificed. This also satisfies
[[client workers own transport neutral stream state not hub orchestration]]:
pre-bind pressure on one route cannot hold up the session I/O actor.

`teardown_bounds`: the hold is bounded by
`QueueSource::ClientWorker.default_capacity()`, the same constant the bound queue
uses. Core adds no unbounded wait and no blocking call. Bind stays synchronous
and non-blocking. If the host never binds, the hold stops growing at capacity and
the owner hard-stops fail-closed, which surfaces to the host as the existing
teardown plus `UnsubscribeSession`. The existing `WRITE_ATTEMPT_BUDGET` of 512
unsuccessful writes remains the hard stop for a misbehaving bound adapter, and
the flushed hold uses that same budget because it enters the same `queue`.
Per [[incremental attach snapshot frames require lossless streaming backpressure]],
the hold must never drop `READY`, a `HISTORY` page, or `FINISH` to stay under
capacity. Overflow is an explicit fail-closed teardown, which that note permits;
silent frame loss is not permitted and is covered by a test.

`late_message_matrix`: ownership-creating and ownership-ending messages for a
declared route.

| Message | Owner tag | Reject after terminal failure | Residual sweep |
| --- | --- | --- | --- |
| `expect_terminal_adapter` | `client_id` + `OwnerKey` | Declaration alone creates no owner; it is inert without a matching attach | `cancel_expected_terminal_adapter`, or consumed by the matching `record_attach` |
| `SubscribeSession` (attach) | `record_attach` assigns `generation` | Existing replacement teardown for the same client and session still runs | Existing `teardown_replaced_client_session` |
| `bind_terminal_adapter` | must present the live `generation` | `BindBeforeAttach`, `UnknownSubscription`, `StaleGeneration`, `AlreadyBound`, `ControlPlaneFailed` unchanged; the presented adapter is closed and dropped on the rejecting stack | Held frames stay with the live owner; a rejected bind never mutates `held` |
| Terminal egress frames while holding | routed by `terminal_route` to the `OwnerKey`, and only accepted when `owner.client_id == client_id` | A frame for a foreign client is retained to the host, unchanged | Hold cleared by `hard_stop` |
| `ProcessExit` while holding | held, sets `process_exit_enqueued` | Delivered after bind, then hard-stop on the tick that observes the completed write | `hard_stop` clears both `held` and `queue` |
| `AttachState { Detached }` while holding | retained to host, unchanged | Not applicable | Not applicable |
| `UnsubscribeSession` / detach | generation-aware `detach_generation` | `GenerationMismatch` does not delete a newer owner | `hard_stop` clears `held` |
| Session shutdown / `teardown_session` / `teardown_all` | `OwnerKey` set | Unchanged | `hard_stop` clears `held` |

`ownership_identity`: the declaration and the hold both key on `client_id` plus
`OwnerKey`. `record_attach` consumes a declaration only when the attaching
`client_id` matches, so a reused `subscription_id` taken over by a different
client does not inherit a stale declaration or a stale hold. A stale declaration
that never matches is retired by `cancel_expected_terminal_adapter` or replaced
by a later declaration for the same identity. Generations still increment through
`last_generation`, so a delayed teardown cannot delete a newer generation.

`production_path_proof`: two paths are named, and both are proved live.

Success path: `CoreDaemon::expect_terminal_adapter` → `CoreDaemon::attach` →
`ManagedSessionRuntime::handle_client_ingress` → `record_attach` →
`apply_client_worker_with` → `ingest_bound_terminal_frames` (hold) →
`CoreDaemon::bind_terminal_adapter` (install only) → next drain tick →
`ingest_bound_terminal_frames` (flush) → `ClientWorker::pump` →
`TerminalAdapter::try_write`.

Failure path to idle: hold overflow or flush encode failure inside
`ingest_bound_terminal_frames`, or `pump` observing `Closed`, returns a
`ClientWorkerTeardown` → `apply_client_worker_with` splices it →
`unsubscribe_owner_teardowns` → production `TransportIngress::UnsubscribeSession`
handler → `hard_stop` closes and drops the adapter and clears `held` and `queue`
→ the owner leaves `list_terminal_subscriptions`.

Both paths are proved with a real worker PTY and authentic Ghostty in
`daemon_integration_test.rs`, through `CoreDaemon` and the public engine API, not
through `ClientWorker` helper calls, per
[[botster dev harnesses must drive real engine types]] and
[[integration tests should use public agent apis not crate-internal test-only helpers]].
Readiness uses the existing polling shape, not a fixed sleep, per
[[PTY integration tests poll for readiness not fixed sleeps]]. A red-on-revert
control accompanies the ordering assertion as a one-line bypass at the hold
branch, per [[a regression test must be shown to go red with the fix reverted]],
[[narrow ablation at the enforcement point is the cleanest regression negative control]],
and [[bound attach suppression skip is unproven without a red oracle]].

`sibling_fail_closed_policy`: on a successful bind and flush, siblings are
unaffected. On hold overflow, encode failure, or adapter close, only the failing
owner dies; siblings keep running. There is no ultimate-failure path that
sacrifices siblings, because no resource is shared across owners. Both live
failure-path tests named above assert that a sibling subscription on the same
session keeps delivering while one route hard-stops, and each names the
production entry point it drives.

## Assumptions and unknowns

Assumptions, stated explicitly:

1. Hub can call one identity-only Core method at admission before `attach`. Hub
   already owns the reservation at that moment and already knows it will bind.
2. An empty `AttachedSession.client_egress` is acceptable for a declared route.
   Hub reads the generation from `list_terminal_subscriptions`, which the shipped
   `bind_echo_worker` helper already does.
3. Host detach is the retirement path for a route that attaches but never binds.
   Hub already retires a Reserved route on open timeout. Core therefore needs no
   internal bind timeout, and adding one would be speculative policy.
4. Capability-gated `Snapshot` drop at bind is correct. An adapter that does not
   advertise `FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY` already receives no
   `Snapshot` frames on the bound path, so a held `Snapshot` must not be
   upgraded into a delivery.
5. `SnapshotPhase` values captured at ingest stay valid across the hold. The
   phase is a value copied from `next_snapshot_phase`, not a reference to live
   worker state.

Unknowns to resolve during Implement, none blocking:

1. Whether the pre-attach declaration is better expressed as a distinct
   `CoreDaemon::attach_for_adapter` entry point rather than a separate
   `expect_terminal_adapter` call. This plan chooses the separate call because it
   changes no existing signature and matches Hub's reserve-then-attach order.
   Plan Review may prefer the entry point; the internal mechanism is identical.
2. Whether the conformance harness in `botster-core-test-support` needs a new
   case for the hold. The wire format does not change, so no conformance revision
   bump is expected. Implement confirms this against
   `crates/botster-core-test-support/src/conformance/mod.rs`.
3. Whether `README.md` needs more than the existing pointer to
   `docs/architecture/client-worker-terminal-egress.md`.

Resolved in revision 2, previously open: where the held dump is flushed. Revision
1 flushed inside `bind_terminal_adapter`. Revision 2 flushes at the head of
`ingest_bound_terminal_frames`, because that call already carries the teardown
return path. This is no longer an open choice.

## Affected surfaces and files

- `crates/botster-core/src/engine/client_worker.rs` — declaration map, owner
  `hold_until_bound` and `held`, ingest hold branch, bind-time flush, `hard_stop`
  clear. Primary change.
- `crates/botster-core/src/engine/managed_session_runtime.rs` — passthrough for
  the two new methods; `record_attach` call sites unchanged in shape.
- `crates/botster-core/src/engine/botster.rs` — passthrough on
  `DefaultBotsterEngine`, `WorkerBackedBotsterEngine`, and `BotsterEngine`.
- `crates/botster-core-daemon/src/daemon.rs` — public
  `expect_terminal_adapter` and `cancel_expected_terminal_adapter`; rustdoc on
  `attach` about empty `client_egress` for a declared route.
- `crates/botster-core/src/contract/terminal_subscription.rs` — doc only if the
  hold changes a documented statement. No new enum variant.
- `crates/botster-core/tests/client_worker_engine_test.rs` — unit and
  `DefaultBotsterEngine` proofs.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` — real worker PTY
  and Ghostty ordering proof plus the red-on-revert control.
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/` — one-slot
  Hub-shaped adapter proof that the flushed hold does not stall live writes.
- `docs/architecture/client-worker-terminal-egress.md` — contract text.
- `README.md` — only if unknown 3 resolves to yes.

## Risks

1. **Regressing the unbound JSON path.** Web, TUI, and Unix clients still consume
   retained frames. Mitigation: the hold is opt-in per owner and every existing
   unbound test must keep passing without edits. Any required edit to an existing
   unbound assertion is a signal that the change leaked, and Implement must stop
   and report it.
2. **Ordering loss across the hold-to-queue boundary.**
   [[removing a delivery gate requires replacement ordering proof]] applies: the
   flush must preserve exact arrival order and must not interleave with live
   frames ingested on the same tick. Mitigation: the flush pass runs at the head
   of `ingest_bound_terminal_frames`, strictly before that tick's new frames are
   ingested, and a test asserts a byte-exact ordered sequence spanning the
   boundary.
3. **Snapshot phase loss.** The current unbound branch deliberately drops
   `next_snapshot_phase`. Mitigation: capture the phase into the held entry at
   ingest and assert READY, HISTORY, FINISH phases on the delivered frames.
4. **`ProcessExit` before bind.** Removing the unbound hard-stop for holding
   owners changes a teardown trigger. Mitigation: the hold keeps
   `process_exit_enqueued`, delivery still hard-stops after the completed write
   per [[adapter accepted writes are not consumer flushed writes]], and a test
   covers exit-before-bind end to end.
5. **Unbounded growth if a host declares and never binds.** Mitigation: the
   capacity bound plus fail-closed hard-stop, plus a test that overflows the hold
   and asserts one teardown and unaffected siblings.
6. **Stale declaration applied to a foreign owner.** Mitigation: client-scoped
   consumption plus a test where a different client attaches the same
   `subscription_id`.
7. **Live output starvation during unfinished attach.**
   [[bound incremental attach drains live output before FINISH]] requires the
   bound path to drain live PTY output while attach is unfinished. The hold now
   makes an owner behave like an adapter route before an adapter exists.
   Implement must confirm that the live-output drain condition keys on the bound
   adapter, not on the hold, so the pre-bind window keeps the unbound one
   snapshot frame per host tick limit.
8. **False green from an always-Ready fake adapter.** Mitigation: the ordering
   and no-stall proofs use the one-slot `HubShapedTerminalAdapter` that reports
   `Full` after accepting a frame.
9. **Silent phase loss under hold pressure.**
   [[incremental attach snapshot frames require lossless streaming backpressure]]
   forbids dropping `READY`, a `HISTORY` page, or `FINISH` to stay under
   capacity. Mitigation: overflow fails the owner closed rather than trimming the
   hold, with a test that asserts no surviving owner ever skips a phase.
10. **A flush failure that never reaches the production teardown path.** This was
    the revision 1 defect. Mitigation: the flush lives in
    `ingest_bound_terminal_frames`, whose `Vec<ClientWorkerTeardown>` return value
    `apply_client_worker_with` already routes to `unsubscribe_owner_teardowns`.
    Two live failure-path tests drive it through `CoreDaemon`.

## Acceptance checks and tests

Ticket acceptance mapped to proofs:

- **A1. After attach then bind, at least two initial attach frames reach the
  adapter in order.** `daemon_integration_test.rs`: real worker PTY, authentic
  Ghostty, `expect_terminal_adapter` → `attach` → assert
  `AttachedSession.client_egress` holds no terminal frame for the route → bind
  with `FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY` → pump → assert the
  delivered frame sequence contains `Snapshot` READY, then the HISTORY pages,
  then `Snapshot` FINISH, then `AttachState::Attached`, in that exact order.
- **A2. A regression goes red if those frames are extracted and discarded.**
  Red-on-revert control: with the hold branch reverted to `retained.push`, A1
  fails on both halves, the empty `client_egress` assertion and the delivered
  order assertion. Implement records the observed red output in the implement
  report, per [[a regression test must be shown to go red with the fix reverted]].
- **A3. Live writes after bind are not Full-stalled by a Hub-held dump.**
  `hub-adapter-shaped` consumer proof with the one-slot adapter: after bind, pump
  repeatedly, assert the held dump drains one frame per Ready transition and that
  live `TerminalOutput` produced after bind arrives after the dump with byte
  fidelity and no loss.
- **A4. No Hub `VecDeque` or other Hub-owned attach history.** Structural: no
  `botster-hub` file changes in the diff, and Core returns no terminal frame for
  a declared route through `attach`, `drain`, or `drain_subscription` before
  bind.

Additional required tests:

- Unbound owners with no declaration keep the exact current retained behavior,
  including the `Snapshot` phase drop and the unbound `ProcessExit` hard-stop.
- A declaration for client A is not consumed by an attach from client B.
- `cancel_expected_terminal_adapter` retires an unconsumed declaration; a later
  attach for that identity does not hold.
- **Overflow teardown through the production path** (`CoreDaemon`, real worker
  PTY): declare, attach, never bind, drive the worker past the hold capacity,
  drain. Assert exactly one teardown, the owner absent from
  `list_terminal_subscriptions`, the route unsubscribed through the production
  `UnsubscribeSession` handler, and a sibling subscription on the same session
  still delivering.
- **Closed adapter at bind through the production path** (`CoreDaemon`, real
  worker PTY): declare, attach, accumulate a held dump, bind an adapter that
  already reports `Closed`, drain. Assert the held dump is discarded, exactly one
  teardown flows through the production unsubscribe path, the adapter is closed
  and dropped, and a sibling route is unaffected.
- **Flush encode failure** (`ClientWorker` level, defensive class): an injected
  encode failure returns the teardown in the value
  `ingest_bound_terminal_frames` returns.
- **No silent phase loss**: overflow never drops `READY`, a `HISTORY` page, or
  `FINISH` while keeping the owner alive. The owner fails closed instead, per
  [[incremental attach snapshot frames require lossless streaming backpressure]].
- `AttachState { Detached }` still reaches the host while a route holds.
- `ProcessExit` arriving before bind is delivered after bind, then the owner
  hard-stops on the tick that observes the completed write.
- Detach before bind clears the hold and emits the normal teardown.
- A foreign route drains normally through `drain_subscription` while another
  route holds, preserving
  [[attach routes use subscription scoped Core drains]].
- Bind with capabilities that omit
  `FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY` drops held `Snapshot` frames and
  still delivers `AttachState::Attached`.

Repository gate commands, from
[[botster-core uses CI-owned Cargo commands because it has no test script]] and
[[botster-core CI runs a contract only test lane because workspace feature unification hides breaks]]:

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
BOTSTER_ENV=test cargo test -p botster-core --no-default-features --lib
```

The isolated `hub-adapter-shaped` consumer crate is not a workspace member. Its
exact direct command, matching the driver the repository already ships, is:

```bash
cd crates/botster-core-test-support/tests/consumers/hub-adapter-shaped \
  && CARGO_TARGET_DIR="$PWD/target" cargo test --quiet --offline
```

One correction to Plan Review `finding_1787769585_950878`. The suggested
`--manifest-path` form runs the crate, but it is not the shipped invocation, and
the premise that a workspace filter never reaches this crate is only half true
here. `crates/botster-core-test-support/tests/terminal_adapter_conformance_test.rs:55-75`
contains `isolated_hub_shaped_consumer_runs_harness_against_its_own_adapter`,
which shells out with `cargo test --quiet --offline` in that directory and sets
`CARGO_TARGET_DIR` to the consumer's own `target`. So
`BOTSTER_ENV=test cargo test --workspace` already drives the consumer crate
transitively through that test. The direct command above is still required
evidence for a focused run, and Implement records both: the named workspace test
that drove it, and the direct command output. The general rule in
[[workspace cargo test filters miss isolated downstream-shaped consumer crates]]
still holds for consumer crates that lack such a driver.

Downstream proof required by the charter: the charter requires hub, client,
conformance, or downstream-shaped proof for a public contract change. The ticket
forbids a `botster-hub` edit, so downstream proof is the isolated
`hub-adapter-shaped` consumer plus the `CoreDaemon` real-worker integration test.
Both exercise the surface Hub embeds, per
[[Hub embeds CoreDaemon behind one client admission point]]. Live Hub proof stays
with the Hub consumer ticket `ticket_1787600674_500120` after this ticket closes.

Build requirements: Zig `0.16.0` and the initialized
`crates/botster-terminal-ghostty/vendor/ghostty` submodule, per `README.md`.

## Worktree hygiene

- Tracked `.gitignore` is present and non-empty (63 bytes) at `358ef1a`. No
  restore is needed.
- The worktree path contains no `:`, so `CARGO_TARGET_DIR` stays unset.
- The tracked worktree is clean at plan time.

## Vault gaps worth capturing

1. **The unbound ClientWorker branch leaks terminal frames on every drain tick,
   not only on attach.** Worth an atomic note, because the ticket and the parent
   Hub review both framed the loss as an `attach()` return problem.
2. **Core pre-bind retention is declared before attach and is not a reservation
   generation.** Worth a note next to
   [[Core ClientWorker bind requires a live attach generation]], to stop a future
   agent from re-proposing a pre-attach bind.
3. **Capability-gated `Snapshot` frames are dropped, not upgraded, when a held
   dump flushes.** Worth a note so a later change does not turn the hold into a
   delivery guarantee that the bound path never made.
4. **A Core state transition that can fail must be placed on a call that already
   returns `Vec<ClientWorkerTeardown>`.** `bind_terminal_adapter` returns only a
   typed bind error, so a failure there has no route to
   `unsubscribe_owner_teardowns`. Worth a note, because this is a reusable
   placement rule for ClientWorker work, not a one-off.
5. **The `hub-adapter-shaped` consumer crate is driven by a workspace test.**
   `terminal_adapter_conformance_test.rs` shells out to it, so
   `cargo test --workspace` does reach it. Worth qualifying
   [[workspace cargo test filters miss isolated downstream-shaped consumer crates]]
   so a future agent does not report it as uncovered.

Capture happens after Verify, not during Plan.
