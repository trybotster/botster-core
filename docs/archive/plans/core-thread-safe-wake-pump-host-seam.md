# Core: expose a supported thread-safe wake pump host seam

Ticket: `ticket_1788220245_689733`
Run: `run_1788221267_887498`
Target repository: `botster-core` (`trybotster/botster-core`)
Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Base revision: `873df1c` ("Use process exit for sibling pump proof")
Consumer ticket: `ticket_1787894427_525056` (botster-hub cold cut, `tgt_7e208a0c76a44980a83b63af976b1f22`)

## Context loaded

Repository playbook:

- [[botster-core-playbook]]

Role playbooks:

- [[planner-playbook]]
- [[botster-planner-playbook]]

Class overlay (runtime-teardown class applies):

- [[botster runtime teardown lenses]]

Atomic notes:

- [[core terminal progress is wake driven and targeted]]
- [[core ingress wake sources are transport neutral]]
- [[session wake coalescing belongs in a lifecycle registry not each handle]]
- [[session ingress wakes retire on observed exit not shutdown acceptance]]
- [[core waking terminal adapters shipped at revision ec589ee]]
- [[hub daemon runtime stays on one owner thread while socket handlers submit requests]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
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

- `crates/botster-core-daemon/src/daemon.rs` (`CoreDaemon`, `wait_wakes`, `pump_woken`, `session_registry_state`, `shutdown`, `shutdown_session`)
- `crates/botster-core-daemon/src/lib.rs` (public re-export surface)
- `crates/botster-core/src/contract/terminal_wake.rs` (`TerminalWakeSource`, `WakeInner`, `recv_nodes`, `assemble_batch`)
- `crates/botster-core/src/engine/managed_session_runtime.rs` (the two production `Rc` uses at lines 95 and 2247)
- `crates/botster-core-daemon/tests/terminal_wake_test.rs` (production-shaped worker PTY wake proofs)
- `crates/botster-core-test-support/tests/consumers/` (isolated hub-shaped consumer crates)
- `docs/architecture/core-daemon.md`, `docs/architecture/terminal-adapter.md`, `docs/README.md`, `docs/plans/README.md`
- `.github/workflows/ci.yml` (repository gate commands)
- Consumer read-only: `botster-hub` `src/runtime.rs` (`SharedCoreDaemon = Mutex<CoreDaemon>`), `src/subscription/closed_events.rs` (`session_close_event_decision`)

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

## Scope

Core adds one narrow, supported pump-side seam plus the neutral interrupt
primitive it needs.

1. `botster-core`, `crates/botster-core/src/contract/terminal_wake.rs`:
   - Add a coalesced, transport-neutral interrupt to `TerminalWakeSource`.
   - Add `TerminalWakeInterrupt`, a `Clone + Send + Sync` handle with
     `interrupt()`. It sets one level-triggered pending flag and, when that flag
     transitions from false to true, publishes at most one `WakeNode::Interrupt`
     so a blocked `recv_timeout` returns.
   - Add an interruptible wait entry point that reports why the wait ended.
     `TerminalWakeSource::wait_wakes` keeps its current signature and behavior.
   - The interrupt must not name a route or a session, must not fabricate an
     adapter route or ingress session, must not clear the overflow flag, and
     must not reorder or drop queued wakes.

2. `botster-core-daemon`, new module `crates/botster-core-daemon/src/wake_pump.rs`:
   - `WakePumpHost` owns the `CoreDaemon` by value. It is `!Send` because
     `CoreDaemon` is `!Send`, so the type system alone proves that only the
     constructing thread can own or call it.
   - `WakePumpHost::new(CoreDaemon) -> Self` and `into_daemon(self) -> CoreDaemon`
     (same thread only).
   - `WakePumpHost::wait(timeout) -> WakePumpWait` with variants `Wakes(TerminalWakeBatch)`,
     `Interrupted`, and `Stopped`.
   - `WakePumpHost::pump_woken(&mut self, &TerminalWakeBatch, now_seconds)` and
     `WakePumpHost::session_registry_state(&self, &SessionId)` delegate to the
     daemon without changing their contracts.
   - `WakePumpHost::control(&self) -> WakePumpControl`, a `Clone + Send + Sync`
     handle with `interrupt()`, `request_stop()`, and `stop_requested()`.
     `WakePumpControl` holds no daemon access of any kind.
   - `WakePumpHost::shutdown(&mut self, now_seconds)` runs `CoreDaemon::shutdown`
     on the owning thread and fails closed with a typed error when
     `request_stop()` has not been observed. This makes "stop before Core
     shutdown" a Core-enforced rule instead of a host convention.
   - Re-export the new types from `crates/botster-core-daemon/src/lib.rs`.

3. Documentation: update `docs/architecture/core-daemon.md` and
   `docs/architecture/terminal-adapter.md` for the seam, the interruptible wait,
   the single-owner-thread rule, and the stop-before-shutdown order. Update the
   root `README.md` host-surface pointer if it lists the wake-driven host loop.

4. Tests: production-shaped worker PTY proofs in
   `crates/botster-core-daemon/tests/terminal_wake_test.rs`, plus one isolated
   hub-shaped consumer crate that builds the seam inside a spawned thread and
   drives the real loop.

## Non-scope

- No change to `CoreDaemon`'s `!Send` property, and no change to the two
  production `Rc` uses in `managed_session_runtime.rs`.
- No `unsafe` anywhere, in Core or in Hub.
- No Core-side thread, executor, request queue, scheduler, cancellation policy,
  or control-request API. Core creates no operating-system thread for this seam,
  preserving the contract already stated in `docs/architecture/core-daemon.md`.
- No change to `pump_woken` targeting, `TerminalWakeBatch` shape, adapter
  contracts, worker protocol version 3, generation fencing, duplex input,
  resize persistence, lifecycle commit, or output delivery.
- No polling path, no correctness timer, and no global route or session scan.
- No Hub changes in this run. Hub thread ownership, queue bounds, attribution,
  cancellation, and transport cold cut stay in `ticket_1787894427_525056`.
- No client (web, TUI) work.

## Repository ownership boundaries and cross-repository dependencies

- Core owns the seam type, the interrupt primitive, wake semantics, targeted
  pumping, registry classification, and shutdown ordering enforcement.
- Hub owns the data-plane thread, `CoreDaemon` construction on that thread, the
  bounded request queue, request attribution and cancellation, scheduling
  fairness between control requests and terminal wakes, and route policy.
- `ticket_1787894427_525056` (botster-hub) already carries a registered
  dependency on this ticket (`dependency_1788220253_452317`). No new dependency
  ticket is required, and this run must not broaden into Hub.
- Deliverable for the consumer: publish the exact merged Core revision for the
  Hub pin after merge, and record it in the run and in the vault note that
  currently pins revision `ec589ee`.

## Runtime-teardown class answers

`teardown_class_applies`: **yes**. The seam governs `SessionIo`/`ClientWorker`
teardown ordering, adapter close progress, late wake admission, and the boundary
between a live pump loop and `CoreDaemon::shutdown`.

`teardown_isolation`: the ownership set that dies with one failed route is the
single subscription: its `RouteWakeState`, its bound adapter, and its
`ClientWorker` route entry. `pump_woken` already partitions the batch per
`SessionId`, so one failing session cannot abort the remaining named sessions in
the same batch. The seam adds no shared mutable state across routes, so it
introduces no new sibling coupling. One failed route must not stop the loop.

`teardown_bounds`: `wait(timeout)` is bounded by the caller's timeout and returns
immediately once `request_stop()` is observed, so a stopped loop cannot block on
a quiet wake channel. `pump_woken` keeps its existing per-session bounds.
`WakePumpHost::shutdown` delegates to `CoreDaemon::shutdown`, which keeps its
existing two-second per-session hang watchdog and its typed `ShutdownFailed`
error. The seam adds no unbounded wait. If the channel is full when
`interrupt()` fires, the publish is dropped on purpose: queued nodes already
guarantee that the next wait cannot block, so liveness holds without growing the
bounded queue.

`late_message_matrix`:

| Surface that creates or ends durable ownership | Owner tag | Rejection after terminal failure | Residual sweep |
|---|---|---|---|
| Adapter writable wake | `Arc<RouteWakeState>` (session, subscription) | `assemble_batch` skips a state whose `retired` flag is set and clears `queued` | `retire_route` marks retired before removal |
| Adapter closed wake | same `RouteWakeState` | same retired filter; close classification uses `session_registry_state` | one idempotent teardown per route |
| Session ingress wake | `Arc<SessionWakeState>` per live `SessionId` | retired sessions are skipped and `queued` is cleared | `forget_session` retires state and recovery data together, after teardown commits |
| Overflow reconcile walk | live registries only | reuses the same retired and queued filters as the fast path | bounded walk, no global scan |
| **New:** interrupt | no route or session identity | ignored once `stop_requested()` is true; the wait returns `Stopped` | pending flag is consumed by the next wait; it names nothing, so it sweeps nothing |
| **New:** stop request | the seam itself | `WakePumpHost::shutdown` fails closed when stop was never requested | stop is monotonic and cannot be cleared |

The interrupt deliberately creates no durable ownership. That is the property
that keeps this seam out of the ownership matrix rather than adding a row to it.

`production_path_proof`: worker or PTY input, or adapter writable or closed
transition → `TerminalWakeSink`/`SessionWakeHandle` publish → data-plane thread
blocked in `WakePumpHost::wait` → `WakePumpWait::Wakes` → `WakePumpHost::pump_woken`
→ `CoreDaemon::pump_woken` → engine facade input apply and targeted egress →
bound adapter `try_write`. The stop path is: host `WakePumpControl::request_stop()`
from another thread → blocked `wait` returns `Stopped` → loop drains accepted
bounded work → `WakePumpHost::shutdown` on the owning thread → thread exits →
host `join`. Oracles run against a real worker PTY in
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
sessions named in the same batch. On ultimate shutdown failure, `CoreDaemon`
already returns `ShutdownFailed` after its bounded watchdog; the seam surfaces
that error to the host without swallowing it and without leaving the host
blocked. Late wakes fail closed through the existing retired filters.

## Assumptions and unknowns

Assumptions (stated, not silently taken):

1. Hub relocates `CoreDaemon::new` onto its data-plane thread. Confirmed in the
   answer to `question_1788221610_604436`.
2. `TerminalWakeBatch` keeps its current shape. The wait reason travels in the
   new `WakePumpWait` enum instead, so no public struct becomes breaking. This
   respects [[botster core public enums are breaking until non exhaustive is decided]];
   `WakePumpWait` ships `#[non_exhaustive]` from the start.
3. `CoreDaemon::wait_wakes`, `pump_woken`, `session_registry_state`, and
   `shutdown` stay public and unchanged. The seam is additive, so an embedder
   that drives the daemon on one thread today keeps working.
4. Only one waiter drains the wake channel at a time. `recv_nodes` takes the
   receiver mutex, so a second waiter blocks rather than steals; the seam makes
   the single-owner rule explicit in documentation, and the `!Send` host type
   makes a second daemon-side waiter unconstructible.

Unknowns to resolve during Implement:

1. Whether `WAKE_QUEUE_CAPACITY` accounting tests (`public_occupancy_is_exact_after_quiesce`,
   `live_allocation_bound`) need an explicit statement that a coalesced interrupt
   occupies at most one slot. Resolve by running those tests, not by assuming.
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
- `crates/botster-core-daemon/src/wake_pump.rs` — new module: `WakePumpHost`,
  `WakePumpControl`, `WakePumpWait`, `WakePumpError`.
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
4. **Occupancy exactness regression.** The repository already requires exact
   accounting ([[count before publish or a concurrent counter cannot be exact]],
   [[a concurrency counter needs a quiesce oracle not a during race sampler]]).
   Mitigation: count before publish, refund every failed send, and re-run the
   existing quiesce oracles.
5. **Scaffold-only seam.** A seam with no production consumer in this repository
   risks being dead code ([[dead code allowances identify scaffold only entry points]],
   [[exhaustive match arms do not prove production reachability]]). Mitigation:
   the hub-shaped consumer crate drives the real loop on a spawned thread, and
   the plan records that the production host wiring lands in
   `ticket_1787894427_525056`. This ticket is intentionally a contract plus
   downstream-shaped proof, not a Hub wiring ticket.
6. **Uninitialized Ghostty submodule blocks every gate.** Verified in this
   worktree: `crates/botster-terminal-ghostty/vendor/ghostty` is empty, and
   `cargo check` fails in the `botster-terminal-ghostty` build script.
   Mitigation: Implement runs
   `git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty`
   and confirms Zig 0.16.0 through `mise` before any gate.
7. **Two waiters on one wake source.** A host that keeps a cloned
   `TerminalWakeSource` and waits on another thread would contend for the
   receiver mutex. Mitigation: document the single-waiter rule on the seam and
   on `TerminalWakeSource::wait_wakes`; add a test that proves the seam's own
   wait plus `CoreDaemon::shutdown` never overlap on one thread.

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
11. `WakePumpHost` is not `Send`, proved by a `compile_fail` doctest, matching
    the existing `compile_fail` precedent in `crates/botster-core/src/lib.rs`.
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
15. **Interrupt loses no wake.** An interrupt raised concurrently with real
    wakes returns `Wakes` with the exact expected routes, or returns
    `Interrupted` and the very next wait returns those exact routes. No route is
    dropped and no unnamed route appears.
16. **Level-triggered interrupt.** An interrupt raised before the wait starts
    makes the next wait return without blocking.
17. **Interrupt names nothing.** An interrupt with no pending wake yields an
    empty batch and pumps zero routes. No global scan and no polling path.
18. **Stop ordering.** `request_stop()` makes a blocked wait return `Stopped`;
    every later wait returns `Stopped` without blocking;
    `WakePumpHost::shutdown` succeeds after stop; the thread joins.
19. **Shutdown fails closed without stop.** `WakePumpHost::shutdown` before any
    `request_stop()` returns the typed error and does not run
    `CoreDaemon::shutdown`.
20. **Stop and join precede Core shutdown.** The test proves the pump loop has
    ended before `CoreDaemon::shutdown` begins, so no second waiter can steal the
    wakes that `shutdown_session` needs for its bounded final drain.
21. **Interrupt during shutdown does not spin.** An interrupt raised while
    `CoreDaemon::shutdown` runs its bounded drain does not shorten, spin, or
    abort the watchdog loop, and shutdown still completes.
22. **Sibling isolation.** A slow or failing route does not stop pumping of a
    sibling route named in the same batch, and does not end the loop. Extends the
    existing `pump_woken_worker_resize_isolates_the_named_sibling` pattern.
23. **Late wakes fail closed.** A wake published after `retire_route` or
    `forget_session` pumps nothing, resurrects no session, and re-registers no
    recovery entry, when delivered through the seam.
24. **Close classification through the seam.** `session_registry_state` called
    from the owning thread returns exact `Found`, `Absent`, and `Err`
    classification, matching the shape Hub's `session_close_event_decision`
    consumes.
25. **Preserved behavior.** The existing `terminal_wake_test.rs` suite stays
    green: targeted duplex input, mode-gated input, worker resize persistence
    and acknowledgment, generation fencing, occupancy exactness, overflow
    recovery, and worker protocol version 3.

### Downstream-shaped proof (charter requirement)

26. A new isolated consumer crate `hub-data-plane-shaped` builds the seam inside
    `std::thread::spawn`, runs `wait` / `pump_woken` / `session_registry_state`,
    receives an interrupt and a stop from the spawning thread, calls
    `WakePumpHost::shutdown` on the owning thread, and joins. It must compile
    with no `unsafe`, no `Arc<Mutex<CoreDaemon>>`, and no `CoreDaemon` value on
    the spawning thread. The driver test asserts those absences in the consumer
    source, matching the existing `lifecycle_journal_consumer_test.rs` pattern.

### Publication

27. After merge, publish the exact merged Core revision for the Hub pin, record
    it in the run, and update the vault note that currently pins revision
    `ec589ee`.

## Vault gaps worth capturing

1. A new note recording that the Core wake pump host seam is `!Send` on purpose,
   that `!Send` is the enforcement mechanism for single-thread daemon ownership,
   and that hosts get thread-safe control through a handle that carries no
   daemon access. This supersedes the scope of
   [[hub daemon runtime stays on one owner thread while socket handlers submit requests]]
   for the terminal data plane: the Core owner thread becomes the data-plane
   thread, and the Hub owner loop submits bounded requests to it.
2. A note recording that a coalesced interrupt on a bounded wake channel can
   safely drop its publish when the channel is full, because queued nodes
   already guarantee a non-blocking wait. This liveness argument is easy to get
   wrong on review.
3. Update the pinned-revision note after merge, replacing or extending
   [[core waking terminal adapters shipped at revision ec589ee]] with the
   revision that carries this seam.
