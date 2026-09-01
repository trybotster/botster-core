# Core: expose a supported thread-safe wake pump host seam

Ticket: `ticket_1788220245_689733`
Run: `run_1788221267_887498`
Target repository: `botster-core` (`trybotster/botster-core`)
Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Base revision: `873df1c` ("Use process exit for sibling pump proof")
Consumer ticket: `ticket_1787894427_525056` (botster-hub cold cut, `tgt_7e208a0c76a44980a83b63af976b1f22`)

Revision 4, after Plan Review `review_1788222362_799486` (five findings),
`review_1788223098_185674` (two findings), and `review_1788223541_423684` (one
blocker) returned `changes_required`. All findings and their resolutions are
recorded in "Plan Review resolutions" below.

## Context loaded

Repository playbook:

- [[botster-core-playbook]]

Role playbooks:

- [[planner-playbook]]
- [[botster-planner-playbook]]

Required Botster planner context:

- [[botster-architecture]]
- [[cli-patterns]]

Class overlay (runtime-teardown class applies):

- [[botster runtime teardown lenses]]

Atomic notes:

- [[core terminal progress is wake driven and targeted]]
- [[core ingress wake sources are transport neutral]]
- [[session wake coalescing belongs in a lifecycle registry not each handle]]
- [[session ingress wakes retire on observed exit not shutdown acceptance]]
- [[core waking terminal adapters shipped at revision ec589ee]]
- [[hub daemon runtime stays on one owner thread while socket handlers submit requests]]
- [[Hub keeps CoreDaemon single owned without a concurrent worker]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
- [[Core ClientWorker bind requires a live attach generation]]
- [[a pre attach declaration must be consumed on every attach return]]
- [[host ShutdownSession classification must call the exact-session Core query]]
- [[count before publish or a concurrent counter cannot be exact]]
- [[a concurrency counter needs a quiesce oracle not a during race sampler]]
- [[an overflow reconcile walk must reuse the readiness filter it backstops]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[botster core contract surface needs consumer proof]]
- [[exhaustive match arms do not prove production reachability]]
- [[dead code allowances identify scaffold only entry points]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]

Repository context read:

- `crates/botster-core-daemon/src/daemon.rs` (`CoreDaemon`, `wait_wakes`,
  `pump_woken`, `session_registry_state`, `shutdown`, `shutdown_session`,
  `ensure_running`, `spawn`, `attach`, the bind pair, `input`, `resize`,
  `detach`)
- `crates/botster-core-daemon/src/lib.rs` (public re-export surface)
- `crates/botster-core/src/contract/terminal_wake.rs` (`TerminalWakeSource`,
  `WakeInner`, `recv_nodes`, `assemble_batch`)
- `crates/botster-core/src/engine/managed_session_runtime.rs` (the two
  production `Rc` uses at lines 95 and 2247)
- `crates/botster-core-daemon/tests/terminal_wake_test.rs` (production-shaped
  worker PTY wake proofs)
- `crates/botster-core-test-support/tests/consumers/` (isolated hub-shaped
  consumer crates)
- `docs/architecture/core-daemon.md`, `docs/architecture/terminal-adapter.md`,
  `docs/README.md`, `docs/plans/README.md`
- `.github/workflows/ci.yml` (repository gate commands)
- Consumer read-only: `botster-hub` `src/runtime.rs`
  (`SharedCoreDaemon = Mutex<CoreDaemon>`), `src/subscription/closed_events.rs`
  (`session_close_event_decision`)

## Resolved architecture decision

`question_1788221610_604436` asked which ownership model this seam must use. The
answer selected **option B** and added binding constraints:

- Exactly one thread owns and mutates `CoreDaemon`.
- Core exposes the narrow pump-side seam only. Core must not gain a Hub-specific
  control request API.
- No `Arc<Mutex<CoreDaemon>>`, no `unsafe impl Send`, no shared mutable daemon
  access, and no second pump path.
- Core provides `wait_wakes`, `pump_woken`, `session_registry_state`, an
  interruptible wait mechanism, and ordered stop support.
- Host requests must interrupt the wait without polling.
- `CoreDaemon` remains intentionally `!Send`, because that property is what
  enforces single-thread ownership.

Hub owns the host thread, the bounded request queue, scheduling policy, request
attribution, and cancellation. Hub relocates `CoreDaemon::new` onto its
data-plane thread; that thread owns the daemon from construction through
shutdown. Those items belong to `ticket_1787894427_525056`, not to this ticket.

## Plan Review resolutions

`review_1788222362_799486` raised five findings. Each is resolved here.

1. `finding_1788222362_190183` (blocker, product). Correct. Revision 1 proposed a
   `WakePumpHost` that owned `CoreDaemon` by value and exposed only four methods
   plus a consuming `into_daemon`. That makes `spawn`, `attach`, the bind pair,
   `input`, `resize`, `detach`, and lifecycle calls unreachable while the pump
   host is alive, so the Hub loop could not process its own bounded requests.
   [[Hub keeps CoreDaemon single owned without a concurrent worker]] rejects the
   same shape for a second reason: an ownership wrapper must not imply a
   synchronization boundary that does not exist. **Resolution: Core adds no
   owning wrapper type.** The data-plane thread holds the `CoreDaemon` value
   directly and calls every method, pump-side and control-side, through a plain
   `&mut`. The new seam is the interrupt, the control handle, the interruptible
   wait, and the stop-ordering rule. See "Owner-thread call path" below.
2. `finding_1788222362_626273` (high, product). Correct. The late-message matrix
   now carries every ownership-creating Core request surface, with owner tag,
   post-stop rejection, and sweep, and it names which side owns each rule.
3. `finding_1788222362_840081` (high, product). Correct. Acceptance check 20 in
   revision 1 stated an impossible order. `CoreDaemon::shutdown` runs inside the
   thread that is later joined, so the join cannot precede it. The check now
   states the exact ordered sequence and keeps the invariant that actually
   matters: the pump loop has ended before Core shutdown begins.
4. `finding_1788222362_562668` (low, process). The gate call carried all five
   fields and the engine stored them, but the `step.completed` event recorded
   empty evidence. This resubmission passes the same five fields to both
   `project_pipelines_submit_gate` and `project_pipelines_request_step_advance`,
   and reuses checklist `checklist_1788221615_290908` rather than creating a
   duplicate.
6. `finding_1788223099_667515` (high, product). Correct, and it identified a
   real data-loss defect, not a wording problem. Revision 2 said `wait_pump`
   returns immediately once stop is observed. Because `recv_nodes` drains the
   bounded channel before any stop check could run, a stop that raced arriving
   wakes could return `Stopped` after those wakes were already removed from the
   channel. `CoreDaemon::shutdown` cannot recover a batch that no longer exists
   in the channel, so those terminal bytes would be lost at teardown.
   **Resolution: drained wakes always win.** `wait_pump` returns `Wakes` whenever
   the drain yielded anything and keeps stop pending; only an empty drain returns
   `Stopped`, and once stop is pending the drain never blocks. The rule, its
   termination argument, and its bound are stated in Scope and in
   `teardown_bounds`. New acceptance checks 19a and 19b prove it, with 19a
   racing `request_stop()` against both an adapter writable wake and a session
   ingress wake and requiring a red result when the rule is reverted.
7. `finding_1788223099_862048` (high, product). Correct. Revision 2 asserted
   base behavior that the code does not have. Verified at `873df1c`:
   `CoreDaemon::attach` (daemon.rs:979) returns from `ensure_running`,
   `ensure_session_mutable`, or `ensure_control_plane_live` before
   `engine.attach_client`, so those arms do not consume a pre-attach
   declaration; and `bind_terminal_adapter` (daemon.rs:1031) and
   `bind_waking_terminal_adapter` (daemon.rs:1059) run `ensure_running` and
   `ensure_session` before the `control_plane_failed` arm, so only that arm
   calls `adapter.close()`. Open same-target ticket `ticket_1788112223_631570`
   owns both residual gaps and names these exact line numbers.
   **Resolution: describe the base behavior exactly, cite the owner ticket, and
   narrow the claim.** The matrix rows now state the verified behavior and
   attribute the policy decision to `ticket_1788112223_631570`. Acceptance check
   25 now asserts only the no-Core-state-allocation rule this ticket owns and no
   longer asserts an explicit close on an early reject.
   `ticket_1788112223_631570` is **not** registered as a blocking dependency:
   this seam does not depend on either gap being fixed, and adding the
   dependency would stall this run on an unrelated policy decision. If a
   reviewer disagrees, that is a scope call for the human, not a silent choice.
   [[a pre attach declaration must be consumed on every attach return]] is now
   loaded and recorded; it states the rule at the `ClientWorker::record_attach`
   level, which is why the daemon-level guard arms remain a separate gap.
8. `finding_1788223541_912045` (blocker, product). Correct, and it is a defect
   that revision 3 introduced while fixing finding 6. Revision 3 argued that the
   post-stop phase was "bounded by what the channel already holds". That
   reasoning is wrong: a bounded channel bounds instantaneous occupancy, not
   total post-stop work. Adapter and session producers stay live until
   `CoreDaemon::shutdown` runs, so they can refill the channel between
   `wait_pump` calls and during `recv_nodes`' uncapped `try_recv` loop
   (`terminal_wake.rs:485`, verified). Under a chatty session every post-stop
   drain would find new nodes, the loop would stay in `Wakes` forever, and the
   owner thread would never reach `Stopped`, Core shutdown, thread exit, or
   `join`. That is an unbounded control-plane hang, which
   [[botster runtime teardown lenses]] rejects outright.
   **Resolution: the stop collision wins exactly once.** The first `wait_pump`
   call that observes a pending stop performs one non-blocking drain capped at
   `WAKE_QUEUE_CAPACITY` nodes and returns `Wakes` if that drain yielded
   anything; every later call returns `Stopped` immediately without touching the
   channel. Later wakes stay queued for `CoreDaemon::shutdown`'s existing
   bounded final drain. The bound is now exact and producer-independent: at most
   one extra pump iteration after stop, handling at most `WAKE_QUEUE_CAPACITY`
   nodes. The node cap is not optional, because `recv_nodes` loops `try_recv`
   until empty and a sustained producer could keep that single loop running.
   Acceptance check 19b is replaced by a sustained-producer termination test
   that keeps a live PTY producer and a writable-reporting adapter running for
   the whole test and asserts the exact post-stop iteration count. Checks 19a
   and 19b are now a matched pair: 19a fails if stop discards the collision
   batch, 19b fails if stop drains until empty, and only the bounded
   collision-wins-once rule passes both.
5. `finding_1788222362_207757` (medium, product). Correct.
   [[botster-architecture]] and [[cli-patterns]] are Must Load entries in
   [[botster-planner-playbook]] and revision 1 omitted them. Both are now loaded
   and recorded. [[botster-architecture]] supplied
   [[Hub keeps CoreDaemon single owned without a concurrent worker]], which is
   the convention that independently confirms finding 1.

## Owner-thread call path

The data-plane thread owns one `CoreDaemon` value. Every call, pump-side and
control-side, uses a plain `&mut` on that value. There is no wrapper, no shared
mutable access, no `unsafe`, and no second pump path.

```rust
// Hub-owned, on the Hub data-plane thread. Core owns none of this policy.
let mut daemon = CoreDaemon::new(config);         // non-Send value never crosses a thread
let control = daemon.wake_pump_control();         // Clone + Send + Sync, no daemon access
// `control` is the only value handed to the Hub owner thread.

loop {
    match daemon.wait_pump(idle_timeout) {        // new: interruptible wait
        WakePumpWait::Stopped => break,
        WakePumpWait::Interrupted => {}
        WakePumpWait::Wakes(batch) => {
            daemon.pump_woken(&batch, now)?;      // existing, unchanged
        }
    }
    // Hub-owned bounded request drain, same thread, same &mut, Hub policy only.
    hub_requests.drain_bounded(&mut daemon);      // spawn, attach, bind, input,
                                                  // resize, detach, lifecycle,
                                                  // session_registry_state
}
hub_requests.finish_accepted_bounded(&mut daemon);
daemon.shutdown(None, now)?;                      // on this thread, after the loop
// thread exits; the Hub owner thread joins it.
```

`CoreDaemon` stays `!Send`, so the compiler alone proves the Hub owner thread
cannot own or call it. `WakePumpControl` is the only Send value that crosses,
and it exposes no daemon access, so it cannot become a second control path.

## Scope

Core adds the narrow pump-side seam and the neutral interrupt primitive it
needs. Core adds no type that owns `CoreDaemon`.

1. `botster-core`, `crates/botster-core/src/contract/terminal_wake.rs`:
   - Add a coalesced, transport-neutral interrupt to `TerminalWakeSource`.
   - Add `TerminalWakeInterrupt`, a `Clone + Send + Sync` handle with
     `interrupt()`. It sets one level-triggered pending flag and, when that flag
     transitions from false to true, publishes at most one `WakeNode::Interrupt`
     so a blocked `recv_timeout` returns.
   - Add an interruptible wait entry point that reports why the wait ended.
     `TerminalWakeSource::wait_wakes` keeps its current signature and behavior,
     and it does not consume the interrupt flag.
   - The interrupt must not name a route or a session, must not fabricate an
     adapter route or ingress session, must not clear the overflow flag, and
     must not reorder or drop queued wakes.

2. `botster-core-daemon`, `crates/botster-core-daemon/src/daemon.rs` plus a new
   `crates/botster-core-daemon/src/wake_pump.rs` for the seam types only:
   - `CoreDaemon::wake_pump_control(&mut self) -> WakePumpControl`. Issuing a
     control marks this daemon as pump-hosted.
   - `WakePumpControl`: `Clone + Send + Sync`, with `interrupt()`,
     `request_stop()`, and `stop_requested()`. It holds the wake interrupt and a
     shared stop flag. It holds no daemon access of any kind.
   - `CoreDaemon::wait_pump(&self, timeout) -> WakePumpWait`, with
     `#[non_exhaustive]` variants `Wakes(TerminalWakeBatch)`, `Interrupted`, and
     `Stopped`.
   - **The stop collision wins exactly once.** Two properties must hold at the
     same time, and satisfying only one of them is a defect:
     - *Lossless:* `wait_pump` must never discard work it has already removed
       from the bounded channel, because `CoreDaemon::shutdown` cannot recover a
       batch that no longer exists there.
     - *Bounded:* the post-stop phase must terminate in a stated finite bound
       regardless of producer behavior. A bounded channel bounds instantaneous
       occupancy, not total post-stop work: adapter and session producers stay
       live until `CoreDaemon::shutdown` runs, so they can refill the channel
       between calls and during the `try_recv` loop.

     The rule that satisfies both:
     1. **First** `wait_pump` call that observes a pending stop performs one
        final non-blocking drain, capped at `WAKE_QUEUE_CAPACITY` nodes. The cap
        is required, because `recv_nodes` currently loops `try_recv` until empty
        and a sustained producer could keep that loop running.
     2. If that capped drain yields any adapter route or ingress session, return
        `Wakes(batch)`. The collision batch wins once, so nothing already drained
        is lost.
     3. **Every later** `wait_pump` call with stop pending returns `Stopped`
        immediately and does not touch the channel.
     4. Wakes published after that point stay queued. They are not lost: they are
        handed to `CoreDaemon::shutdown`, which already owns a bounded final
        drain under its two-second watchdog.

     The resulting bound is exact and producer-independent: at most one extra
     pump iteration after stop, handling at most `WAKE_QUEUE_CAPACITY` nodes.

     The interrupt keeps the ordinary order: drain, return `Wakes` if the drain
     yielded anything and leave the interrupt flag pending, otherwise return
     `Interrupted`, otherwise the timeout result. `Stopped` outranks
     `Interrupted`.
   - `CoreDaemon::shutdown` fails closed with a typed error when a control was
     issued and `request_stop()` was never observed. When no control was ever
     issued, `shutdown` keeps its existing runtime behavior for single-thread
     embedders. The exhaustive-error source compatibility caveat is recorded
     under Assumptions and unknowns.
   - `pump_woken`, `session_registry_state`, and every control-plane method keep
     their current signatures and stay reachable through `&mut CoreDaemon`.
   - Re-export the new types from `crates/botster-core-daemon/src/lib.rs`.

3. Documentation: update `docs/architecture/core-daemon.md` and
   `docs/architecture/terminal-adapter.md` for the interruptible wait, the
   single-owner-thread rule, the single-waiter rule, and the stop-then-shutdown
   order. Update the root `README.md` host-surface pointer if it names the
   wake-driven host loop.

4. Tests: production-shaped worker PTY proofs in
   `crates/botster-core-daemon/tests/terminal_wake_test.rs`, plus one isolated
   hub-shaped consumer crate that builds the daemon inside a spawned thread and
   drives the real loop, including a bounded request drain against the same
   `&mut CoreDaemon`.

## Non-scope

- No type that owns `CoreDaemon`, and no ownership wrapper that implies a
  synchronization boundary that does not exist.
- No change to `CoreDaemon`'s `!Send` property, and no change to the two
  production `Rc` uses in `managed_session_runtime.rs`.
- No `unsafe` anywhere, in Core or in Hub.
- No Core-side thread, executor, request queue, scheduler, admission policy,
  cancellation policy, or control-request API. Core creates no operating-system
  thread for this seam, preserving the contract already stated in
  `docs/architecture/core-daemon.md`.
- No change to `pump_woken` targeting, `TerminalWakeBatch` shape, adapter
  contracts, worker protocol version 3, generation fencing, duplex input,
  resize persistence, lifecycle commit, or output delivery.
- No polling path, no correctness timer, and no global route or session scan.
- No Hub changes in this run. Hub thread ownership, `CoreDaemon` construction on
  that thread, request admission and its stop, queue bounds, attribution,
  cancellation, and the transport cold cut stay in `ticket_1787894427_525056`.
- No client (web, TUI) work.

## Repository ownership boundaries and cross-repository dependencies

- Core owns the seam types, the interrupt primitive, wake semantics, targeted
  pumping, registry close classification, per-call fail-closed rules
  (`ensure_running`, `ensure_session`, generation fencing, bind rejection), and
  the stop-before-shutdown rule.
- Hub owns the data-plane thread, `CoreDaemon` construction on that thread, the
  bounded request queue, request admission and the stop of that admission,
  attribution, cancellation, fairness between control requests and terminal
  wakes, and route policy.
- `ticket_1787894427_525056` (botster-hub) already carries a registered
  dependency on this ticket (`dependency_1788220253_452317`). No new dependency
  ticket is required, and this run must not broaden into Hub.
- Deliverable for the consumer: publish the exact merged Core revision for the
  Hub pin after merge, and record it in the run and in the vault note that
  currently pins revision `ec589ee`.

## Runtime-teardown class answers

`teardown_class_applies`: **yes**. The seam governs `SessionIo`/`ClientWorker`
teardown ordering, adapter close progress, late wake and late request admission,
and the boundary between a live pump loop and `CoreDaemon::shutdown`.

`teardown_isolation`: the ownership set that dies with one failed route is the
single subscription: its `RouteWakeState`, its bound adapter, and its
`ClientWorker` route entry. `pump_woken` already partitions the batch per
`SessionId`, so one failing session cannot abort the remaining named sessions in
the same batch. The seam adds no shared mutable state across routes, so it
introduces no new sibling coupling. One failed route must not stop the loop.

`teardown_bounds`: `wait_pump(timeout)` is bounded by the caller's timeout. The
post-stop phase has an exact, producer-independent bound: the first call that
observes a pending stop performs one non-blocking drain capped at
`WAKE_QUEUE_CAPACITY` nodes and returns `Wakes` if that drain yielded anything;
every later call returns `Stopped` immediately without touching the channel. So
the loop runs at most one extra iteration after stop, no matter how fast live
producers publish, and no already-drained batch is discarded. Wakes published
after that point remain queued for `CoreDaemon::shutdown`, which owns the
bounded final drain under its two-second watchdog. `pump_woken` keeps its
existing per-session bounds. `CoreDaemon::shutdown` keeps its existing two-second per-session hang
watchdog and its typed `ShutdownFailed` error. The seam adds no unbounded wait.
If the channel is full when `interrupt()` fires, the publish is dropped on
purpose: queued nodes already guarantee that the next wait cannot block, so
liveness holds without growing the bounded queue.

`late_message_matrix`: every surface that creates or ends durable ownership,
including the Core request surfaces the Hub loop drives on the owner thread.

| Surface | Owner tag | Rejection after terminal failure or stop | Residual sweep | Rule owner |
|---|---|---|---|---|
| Adapter writable wake | `Arc<RouteWakeState>` (session, subscription) | `assemble_batch` skips a retired state and clears `queued` | `retire_route` marks retired before removal | Core |
| Adapter closed wake | same `RouteWakeState` | same retired filter; classification through `session_registry_state` | one idempotent teardown per route | Core |
| Session ingress wake | `Arc<SessionWakeState>` per live `SessionId` | retired sessions are skipped and `queued` is cleared | `forget_session` retires state and recovery data together, after teardown commits | Core |
| Overflow reconcile walk | live registries only | reuses the same retired and queued filters as the fast path | bounded walk, no global scan | Core |
| `spawn` | `SessionId` plus the registry row | `ensure_running` fails closed after `shutdown`; Hub stops admission at `request_stop` | registry row and engine session are created together or not at all | Core rejection, Hub admission |
| `attach` | client, session, subscription, generation | `ensure_running`, `ensure_session_mutable`, `ensure_control_plane_live` | unmatched egress is retained, not leaked to a foreign route | Core |
| `expect_terminal_adapter` | client, session, subscription | **Base behavior, verified at `873df1c`:** `attach` returns from `ensure_running`, `ensure_session_mutable`, or `ensure_control_plane_live` *before* `engine.attach_client`, so those three arms do **not** consume the declaration. Only an attach that reaches the engine consumes or rejects it. | `cancel_expected_terminal_adapter` is the caller's only retirement path for a declaration left by an early-rejected attach | Core; the residual gap is owned by `ticket_1788112223_631570` |
| `bind_terminal_adapter` and `bind_waking_terminal_adapter` | client, session, subscription, generation | **Base behavior, verified at `873df1c`:** `ensure_running` and `ensure_session` run *before* the `control_plane_failed` arm, so only that arm calls `adapter.close()`. The two earlier reject arms drop the boxed adapter without an explicit `close()`. A stale generation cannot bind. | every rejection arm allocates no wake state, which is the property this ticket owns and tests | Core; the close-on-every-arm decision is owned by `ticket_1788112223_631570` |
| `input`, `mode_gated_input`, `resize` | live subscription id and generation | control-queue-full retries in order; other failures fail closed | capacity-parked input retries only on a matching session ingress wake | Core |
| `detach` and `detach_terminal_subscription` | session, subscription, generation | a stale generation cannot detach a live replacement owner | one idempotent teardown per route | Core |
| `shutdown_session` and `remove_session` | `SessionId` | `remove_session` rejects live and stopping sessions | ingress wake retires only after the lifecycle transition commits | Core |
| Interrupt | no route or session identity | ignored once `stop_requested()` is true; the wait returns `Stopped` | pending flag is consumed by the next wait; it names nothing, so it sweeps nothing | Core |
| Stop request | the control handle | `shutdown` fails closed when stop was never requested; the first stop-observing wait returns its capped collision batch so nothing drained is discarded, and every later wait returns `Stopped` without touching the channel | stop is monotonic and cannot be cleared; wakes published after the collision batch stay queued for `CoreDaemon::shutdown`'s bounded final drain | Core |
| Hub bounded request queue | Hub request id and attribution | Hub stops admission at `request_stop`; accepted work finishes before shutdown | Hub cancels or drains its own queue | Hub (`ticket_1787894427_525056`) |

The interrupt and the stop request deliberately create no durable ownership.
That is what keeps them out of the ownership matrix rather than adding rows that
need sweeping. Requests raced against stop are handled in two layers: Hub stops
admission, and Core fails every call closed through `ensure_running` once
`shutdown` completes.

`production_path_proof`: worker or PTY input, or adapter writable or closed
transition → `TerminalWakeSink`/`SessionWakeHandle` publish → data-plane thread
blocked in `CoreDaemon::wait_pump` → `WakePumpWait::Wakes` → `pump_woken` →
engine facade input apply and targeted egress → bound adapter `try_write`. The
control-request path is: Hub owner thread submits a bounded request and calls
`WakePumpControl::interrupt()` → the blocked wait returns `Interrupted` → the
Hub drain runs the request against the same `&mut CoreDaemon`. The stop path is:
Hub stops admission → `WakePumpControl::request_stop()` → the first stop-observing
wait returns its capped collision batch as `Wakes` when the drain yielded
anything, and the loop pumps it → the next wait returns `Stopped` without
touching the channel → the loop ends → the Hub finishes bounded accepted work →
`CoreDaemon::shutdown` runs on the owner thread and drains any wakes published
after the collision batch under its two-second watchdog → the thread exits → the
Hub owner thread joins it. Oracles run against a real worker PTY in
`crates/botster-core-daemon/tests/terminal_wake_test.rs` and drive the seam from
a genuinely spawned thread, not from a helper call on the test thread.

`ownership_identity`: unchanged. Routes stay identified by session,
subscription, and generation. Sessions stay identified by `SessionId` with one
registry-owned wake state per live session. `WakePumpControl` carries no route,
session, or generation identity, so it cannot resurrect, replace, or delete a
row owned by a different live owner. A stale `WakePumpControl` retained after
the host thread ends can only set flags that no live loop reads.

`sibling_fail_closed_policy`: on successful stop and shutdown, no sibling
behavior changes, because the whole daemon is stopping. Within a live loop, one
failing session's `pump_woken` error must not stop the loop or the sibling
sessions named in the same batch, and one failing Hub request must not stop the
pump loop. On ultimate shutdown failure, `CoreDaemon` already returns
`ShutdownFailed` after its bounded watchdog; the seam surfaces that error to the
host without swallowing it and without leaving the host blocked. Late wakes fail
closed through the existing retired filters.

## Assumptions and unknowns

Assumptions (stated, not silently taken):

1. Hub relocates `CoreDaemon::new` onto its data-plane thread and drives every
   Core call from that thread. Confirmed by the answer to
   `question_1788221610_604436`.
2. `TerminalWakeBatch` keeps its current shape. The wait reason travels in the
   new `WakePumpWait` enum instead, so no public struct becomes breaking. This
   respects [[botster core public enums are breaking until non exhaustive is decided]];
   `WakePumpWait` ships `#[non_exhaustive]` from the start.
3. Every existing public `CoreDaemon` method keeps its signature, and the seam
   is additive at runtime. A host that never calls `wake_pump_control` sees no
   behavior change, including in `shutdown`. Source compatibility is not fully
   additive: `CoreDaemonError` remains exhaustive and gains the `WakePump`
   variant. A downstream exhaustive match must add an arm. This ticket does not
   add `#[non_exhaustive]`; that broader compatibility decision remains
   separate. Hub ticket `ticket_1787894427_525056` owns the Hub match update.
4. Only one waiter drains the wake channel at a time. `recv_nodes` takes the
   receiver mutex, so a second waiter blocks rather than steals; the plan
   documents the single-waiter rule, and `CoreDaemon: !Send` makes a second
   daemon-side waiter unconstructible.

Unknowns to resolve during Implement:

1. Whether `WAKE_QUEUE_CAPACITY` accounting tests
   (`public_occupancy_is_exact_after_quiesce`, `live_allocation_bound`) need an
   explicit statement that a coalesced interrupt occupies at most one slot.
   Resolve by running those tests, not by assuming.
2. Whether `CoreDaemon::shutdown`'s internal `wait_wakes` loop should also
   consume the interrupt flag. Current plan: it must not, so a host interrupt
   cannot spin the shutdown watchdog loop. Prove with a test that interrupts
   during shutdown.
3. Whether the root `README.md` names the host wake loop and therefore needs the
   seam pointer.

## Affected surfaces and files

- `crates/botster-core/src/contract/terminal_wake.rs` — interrupt state,
  `WakeNode::Interrupt`, `TerminalWakeInterrupt`, interruptible wait.
- `crates/botster-core/src/lib.rs` — re-export `TerminalWakeInterrupt` and the
  wait outcome type.
- `crates/botster-core-daemon/src/wake_pump.rs` — new module: `WakePumpControl`,
  `WakePumpWait`, `WakePumpError`.
- `crates/botster-core-daemon/src/daemon.rs` — `wake_pump_control`, `wait_pump`,
  and the stop-before-shutdown rule inside `shutdown`.
- `crates/botster-core-daemon/src/lib.rs` — module declaration and re-exports.
- `crates/botster-core-daemon/tests/terminal_wake_test.rs` — seam proofs.
- `crates/botster-core-test-support/tests/consumers/hub-data-plane-shaped/` — new
  isolated consumer crate (non-workspace member, same pattern as
  `hub-lifecycle-shaped` and `hub-adapter-shaped`).
- `crates/botster-core-test-support/tests/` — the driver test that builds and
  runs that consumer crate.
- `docs/architecture/core-daemon.md`, `docs/architecture/terminal-adapter.md`,
  and `README.md` if it names the host wake loop.

## Risks

1. **Interrupt starves or perturbs terminal wakes.** Mitigation: the interrupt
   never clears the overflow flag, never consumes a route or session state, and
   is assembled after node drain, so a wait that finds real wakes reports
   `Wakes` and leaves the pending interrupt flag set for the next wait.
2. **Lost wakeup.** An interrupt raised between the node drain and the blocking
   `recv_timeout` must not be missed. Mitigation: the flag is level-triggered
   and checked after the drain and immediately before blocking; the coalesced
   node publish also breaks an already-blocked `recv_timeout`.
3. **Bounded queue pressure from interrupts.** Mitigation: coalescing keeps at
   most one interrupt node in flight, and a full channel makes the publish
   unnecessary because the wait cannot block.
3a. **Post-stop non-termination under live producers.** The most dangerous
   failure mode in this seam, and the one revision 3 got wrong. Producers stay
   live until `CoreDaemon::shutdown` runs, so any rule that drains until the
   channel is empty can loop forever and hang the owner thread before Core
   shutdown. Mitigation: the stop collision wins exactly once, with a
   `WAKE_QUEUE_CAPACITY` node cap on that single drain and an immediate
   `Stopped` on every later call. Proved by acceptance check 19b under a
   sustained producer, paired with 19a so neither property can be satisfied
   alone.
4. **Occupancy exactness regression.** The repository already requires exact
   accounting ([[count before publish or a concurrent counter cannot be exact]],
   [[a concurrency counter needs a quiesce oracle not a during race sampler]]).
   Mitigation: count before publish, refund every failed send, and re-run the
   existing quiesce oracles.
5. **Stop rule breaks existing embedders.** A daemon that never issues a control
   must keep today's `shutdown` behavior exactly. Mitigation: the fail-closed
   rule is conditional on a control having been issued, and a test covers the
   no-control path.
6. **Scaffold-only seam.** A seam with no production consumer in this repository
   risks being dead code ([[dead code allowances identify scaffold only entry points]],
   [[exhaustive match arms do not prove production reachability]]). Mitigation:
   the hub-shaped consumer crate drives the real loop on a spawned thread,
   including a bounded request drain, and the plan records that the production
   host wiring lands in `ticket_1787894427_525056`. This ticket is intentionally
   a contract plus downstream-shaped proof, not a Hub wiring ticket.
7. **Uninitialized Ghostty submodule blocks every gate.** Verified in this
   worktree: `crates/botster-terminal-ghostty/vendor/ghostty` is empty, and
   `cargo check` fails in the `botster-terminal-ghostty` build script.
   Mitigation: Implement runs
   `git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty`
   and confirms Zig 0.16.0 through `mise` before any gate.
8. **Two waiters on one wake source.** A host that keeps a cloned
   `TerminalWakeSource` and waits on another thread would contend for the
   receiver mutex. Mitigation: document the single-waiter rule on `wait_pump`
   and on `TerminalWakeSource::wait_wakes`; add a test proving the pump loop and
   `CoreDaemon::shutdown` never overlap on one thread.

## Acceptance checks and tests

### Repository gates (CI-owned; see `.github/workflows/ci.yml`)

Run from a colon-free worktree. This worktree path contains no `:`, so
`CARGO_TARGET_DIR` stays at the repository default.

1. `git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty`
2. `cargo fmt --all -- --check`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace`
5. `cargo test -p botster-core --no-default-features --lib`
6. `cargo test --doc --workspace`
7. `cargo doc --workspace --no-deps`
8. `cargo test -p botster-core --test local_process_runtime_test`
9. Isolated consumer crate: direct `cargo test` in
   `crates/botster-core-test-support/tests/consumers/hub-data-plane-shaped`,
   or the wrapper test that starts it. A workspace filter does not run a
   non-member crate.

### Contract proofs (compile time)

10. `WakePumpControl` is `Send + Sync + Clone`, proved by a static assertion.
11. `CoreDaemon` is still not `Send`, proved by a `compile_fail` doctest,
    matching the existing `compile_fail` precedent in
    `crates/botster-core/src/lib.rs`.
12. `WakePumpControl` exposes no daemon accessor. Proved by the consumer crate,
    which holds a control handle on the spawning thread and cannot reach the
    daemon from there.

### Production-path proofs (real worker PTY, spawned thread)

13. **Owner thread stays idle.** Terminal output reaches a bound waking adapter
    while the spawning thread performs no Core call at all. Red on revert of the
    seam wiring.
14. **Interrupt without polling.** The spawning thread calls
    `WakePumpControl::interrupt()`; a wait blocked on a long timeout returns
    `Interrupted` well before that timeout elapses. The test asserts elapsed time
    below the timeout, not merely a returned value.
15. **Interrupt reaches the control-request path.** After an interrupt returns
    the wait, the owner-thread loop runs a Core control call against the same
    `&mut CoreDaemon` and that call succeeds while terminal routes stay bound.
    This proves the seam does not make control calls unreachable.
16. **Interrupt loses no wake.** An interrupt raised concurrently with real
    wakes returns `Wakes` with the exact expected routes, or returns
    `Interrupted` and the very next wait returns those exact routes. No route is
    dropped and no unnamed route appears.
17. **Level-triggered interrupt.** An interrupt raised before the wait starts
    makes the next wait return without blocking.
18. **Interrupt names nothing.** An interrupt with no pending wake yields an
    empty batch and pumps zero routes. No global scan and no polling path.
19. **Stop ordering.** `request_stop()` makes a blocked wait on a quiet channel
    return `Stopped`, and every later wait on a quiet channel returns `Stopped`
    without blocking.
19a. **Stop never discards a drained wake.** `request_stop()` races both a bound
    adapter writable wake and a session ingress wake. The test proves the wait
    returns `Wakes` naming both, that `pump_woken` delivers both through the
    production path, and only then that the next wait returns `Stopped` and
    shutdown runs. Reverting the collision-wins-once rule must make this test
    red. Checks 19a and 19b are a matched pair and must be run together: 19a
    fails if the stop discards the collision batch, and 19b fails if the stop
    drains until empty. Only the bounded collision-wins-once rule passes both.
19b. **Post-stop termination under a sustained producer.** A live session
    produces PTY output continuously and a bound adapter reports writable
    transitions continuously, so the wake channel is refilled between calls and
    during the `try_recv` loop. After `request_stop()`, the pump loop must reach
    `Stopped`, `CoreDaemon::shutdown`, thread exit, and `join` within the stated
    bound: at most one `Wakes` result after stop, then `Stopped`. The producer
    keeps running for the whole test, so a rule that drains until empty fails
    this test by never terminating. The test asserts the exact iteration count,
    not merely that the loop eventually ends.
20. **Exact shutdown sequence.** The test drives, in order: stop Hub-shaped
    admission, `request_stop()` and the interrupt it raises, the pump loop ends,
    bounded accepted work finishes against the same `&mut CoreDaemon`,
    `CoreDaemon::shutdown` runs on that owner thread and returns `Ok`, the thread
    exits, and only then the spawning thread's `join` returns. The invariant
    asserted is that the pump loop has ended before Core shutdown begins, so no
    second waiter can steal the wakes that `shutdown_session` needs for its
    bounded final drain.
21. **Shutdown fails closed without stop.** `CoreDaemon::shutdown` after
    `wake_pump_control()` but before any `request_stop()` returns the typed error
    and does not tear down sessions.
22. **No control means no behavior change.** A daemon that never calls
    `wake_pump_control()` shuts down exactly as it does today.
23. **Interrupt during shutdown does not spin.** An interrupt raised while
    `CoreDaemon::shutdown` runs its bounded drain does not shorten, spin, or
    abort the watchdog loop, and shutdown still completes.
24. **Late request after shutdown fails closed.** A Core call issued after
    `shutdown` completes returns the `ensure_running` error and creates no
    session, route, or wake state.
25. **Bind after stop allocates no Core state.** A waking bind attempted after
    `request_stop()` and shutdown allocates no wake registry entry and leaves the
    registry length unchanged. This check deliberately asserts only the
    no-allocation rule that this ticket owns. It does **not** assert that the
    adapter is explicitly closed, because the verified base behavior at `873df1c`
    is that `ensure_running` and `ensure_session` reject before the arm that
    calls `adapter.close()`, so an early-rejected adapter is dropped without an
    explicit close. Changing that is owned by `ticket_1788112223_631570` and is
    out of scope here.
26. **Sibling isolation.** A slow or failing route does not stop pumping of a
    sibling route named in the same batch, and does not end the loop. Extends the
    existing `pump_woken_worker_resize_isolates_the_named_sibling` pattern.
27. **Late wakes fail closed.** A wake published after `retire_route` or
    `forget_session` pumps nothing, resurrects no session, and re-registers no
    recovery entry, when delivered through the seam.
28. **Close classification on the owner thread.** `session_registry_state`
    returns exact `Found`, `Absent`, and `Err` classification, matching the shape
    Hub's `session_close_event_decision` consumes.
29. **Preserved behavior.** The existing `terminal_wake_test.rs` suite stays
    green: targeted duplex input, mode-gated input, worker resize persistence
    and acknowledgment, generation fencing, occupancy exactness, overflow
    recovery, and worker protocol version 3.

### Downstream-shaped proof (charter requirement)

30. A new isolated consumer crate `hub-data-plane-shaped` constructs
    `CoreDaemon` inside `std::thread::spawn`, takes one `WakePumpControl` out to
    the spawning thread, and runs the real loop: `wait_pump`, `pump_woken`, a
    bounded Hub-shaped request drain that calls `spawn`, `attach`, a waking
    bind, `input`, `resize`, `detach`, and `session_registry_state` against the
    same `&mut CoreDaemon`, then `CoreDaemon::shutdown` on that thread, then
    exit and join. It must compile with no `unsafe`, no `Arc<Mutex<CoreDaemon>>`,
    no ownership wrapper around `CoreDaemon`, and no `CoreDaemon` value on the
    spawning thread. The driver test asserts those absences in the consumer
    source, matching the existing `lifecycle_journal_consumer_test.rs` pattern.

### Publication

31. After merge, publish the exact merged Core revision for the Hub pin, record
    it in the run, and update the vault note that currently pins revision
    `ec589ee`.

## Vault gaps worth capturing

1. A new note recording that Core exposes the wake pump seam as an interrupt, a
   control handle, and an interruptible wait rather than as a type that owns
   `CoreDaemon`, because an owning wrapper would make the host's other Core
   calls unreachable and would imply a synchronization boundary that does not
   exist. This joins [[Hub keeps CoreDaemon single owned without a concurrent worker]]
   to the data-plane thread case, and it extends
   [[hub daemon runtime stays on one owner thread while socket handlers submit requests]]:
   the Core owner thread becomes the data-plane thread, and the Hub owner loop
   submits bounded requests to it.
2. A note recording that a coalesced interrupt on a bounded wake channel can
   safely drop its publish when the channel is full, because queued nodes
   already guarantee a non-blocking wait. This liveness argument is easy to get
   wrong on review.
3. A note recording the strongest lesson of this plan: a stop signal on a
   draining bounded channel must win exactly once, not repeatedly. Returning
   `Stopped` before the drained batch loses terminal bytes that shutdown cannot
   recover; draining until empty never terminates, because a bounded channel
   bounds instantaneous occupancy while live producers keep refilling it. The
   only rule satisfying both is a single capped collision batch followed by an
   unconditional `Stopped`, with the remainder handed to the component that
   already owns a bounded final drain. Two plan revisions failed this in
   opposite directions, so the note should carry the matched-pair test shape
   (lossless test plus sustained-producer termination test) that makes each
   wrong rule red.
4. Update the pinned-revision note after merge, replacing or extending
   [[core waking terminal adapters shipped at revision ec589ee]] with the
   revision that carries this seam.
