# Core: complete worker-backed resize during targeted pump_woken

Ticket: `ticket_1788198279_441580`
Run: `run_1788200376_394138`
Revision: 3 (revised after Plan Review `review_1788201761_105279` finding
`finding_1788201761_141189`, and `review_1788202487_787926` finding
`finding_1788202487_209804`)
Target repository: `botster-core` (`trybotster/botster-core`)
Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Base: `main` at `a781556` ("Prove targeted duplex wake edge cases")

## Problem

Revision `a781556` completed the input hop. `DefaultBotsterEngine::pump_woken`
(`crates/botster-core/src/engine/botster.rs:559`) and
`WorkerBackedBotsterEngine::pump_woken` (`crates/botster-core/src/engine/botster.rs:1217`)
both call `ManagedSessionRuntime::apply_woken_terminal_input` between phase one and
phase three. `apply_one_delivery`
(`crates/botster-core/src/engine/managed_session_runtime.rs:498`) already handles
`TerminalInputCommand::Resize` and routes it through `apply_targeted_client_ingress`
to `TransportIngress::Resize`.

The resize therefore reaches the session worker as one `FRAME_RESIZE`
(`crates/botster-core/src/runtime/worker_process.rs:1545`), but the host-visible
record never changes.

`CoreDaemon::pump_woken` (`crates/botster-core-daemon/src/daemon.rs:1092`) persists a
size only from `DaemonEngine::take_applied_attach_resize`
(`daemon.rs:1164`, `daemon.rs:3701`). That map is populated only by the incremental
attach barrier (`botster.rs:1647`). No producer exists for a resize that arrives as
compact duplex input on a targeted wake tick.

`CoreDaemon::resize` (`daemon.rs:1292`) is the only path that calls
`persist_session_size` (`daemon.rs:1312`), which writes `sessions/<id>.json` and calls
`append_lifecycle_upsert`. A host that drives the runtime with `wait_wakes` +
`pump_woken` never reaches that call.

That is the exact reported failure. The Hub reproduction
`session_entity_subscription_pushes_snapshot_ordered_deltas_and_fresh_reconnect`
requests 31x101 and keeps observing `rows=24`, `cols=80`, because the live PTY and the
persisted record diverge. This is a Core gap. No Hub workaround is in scope.

## Wake classes

`TerminalWakeBatch` carries two classes, as
[[core terminal progress is wake driven and targeted]] defines:

- `adapter_routes`: one exact route has adapter work. Client resize commands exist only
  on this class.
- `ingress_sessions`: one session has new worker or PTY output. This class carries no
  client input, but it does release capacity-parked owners
  ([[capacity parked terminal inputs retry only on matching session ingress wakes]]).

The resize apply, the persistence, and the lifecycle patch stay scoped to the sessions
named by the batch. No unnamed session is loaded, saved, or patched.

## Playbooks and notes loaded

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-core-playbook]] (repository charter for the resolved target)
- [[botster runtime teardown lenses]] (runtime-teardown class applies; see below)
- [[core terminal progress is wake driven and targeted]]
- [[botster engine facades apply terminal input between targeted pump phases]]
- [[core owns duplex terminal transport while Hub stays content blind]]
- [[capacity parked terminal inputs retry only on matching session ingress wakes]]
- [[control queue full retries terminal input in order while other failures fail closed]]
- [[session registry size follows the worker applied resize]]
- [[worker applies the latest attach resize before barrier release]]
- [[core daemon lifecycle metadata is registry backed restart state]]
- [[every TerminalInputResult must stamp the live subscription id]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster-core CI runs a contract only test lane because workspace feature unification hides breaks]]
- [[a regression test must be shown to go red with the fix reverted]]
- [[botster core contract surface needs consumer proof]]
- [[spawned Hub tests can reach only four of fourteen Core test builders]]

## Context loaded

- `crates/botster-core/src/engine/managed_session_runtime.rs` (apply stage, phases).
- `crates/botster-core/src/engine/botster.rs` (both engine facades, attach resize map).
- `crates/botster-core-daemon/src/daemon.rs` (`pump_woken`, `resize`,
  `persist_session_size`, `DaemonEngine`).
- `crates/botster-core/src/runtime/worker_process.rs` (`FRAME_RESIZE`).
- `crates/botster-core-daemon/tests/terminal_wake_test.rs` (targeted wake proofs).
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  (`pump_persist_resize_error_retains_once`).
- `crates/botster-core/tests/local_session_worker_process_test.rs` (`stty size` oracle).
- `.github/workflows/ci.yml` (repository gate commands).

## Scope

1. Record the applied targeted resize in `ManagedSessionRuntime`.
   `apply_one_delivery` inserts `(rows, cols, last_output_at)` for the session when a
   `DeliveryApply::Targeted` resize succeeds. The latest resize in one tick wins.
2. Expose the record. Add `take_applied_terminal_resize(&SessionId)` on
   `ManagedSessionRuntime`, on `DefaultBotsterEngine`, on `WorkerBackedBotsterEngine`,
   and on `DaemonEngine`. This keeps the shipped
   `take_applied_attach_resize` meaning unchanged and adds one separate meaning for
   duplex resizes.
3. Persist and publish in `CoreDaemon::pump_woken`. After phase three for one named
   session, take both applied-resize values, prefer the duplex value, and call one
   persistence step. The existing failure handling stays: on persistence error, record
   obligations, record the commit failure, retain the drain result, and keep the first
   error.
4. Suppress duplicate lifecycle patches. The pump path saves the record and appends the
   lifecycle upsert only when the persisted `rows` or `cols` actually change. Refactor
   `persist_session_size` into a shared inner writer plus a changed-only wrapper. The
   control-plane `CoreDaemon::resize` path keeps its current unconditional write.
5. Tests. Add operation-count proofs at the worker-bound `FRAME_RESIZE` seam, live
   worker-backed proofs of the final PTY size, and three separate red ablations. Add one
   `#[cfg(test)]` `pub(crate)` accessor on `ControlQueue` that returns held
   `(frame_type, payload)` pairs, beside the existing `#[cfg(test)]` `hold_pops`.
6. Documentation. Update `docs/architecture/terminal-adapter.md` with the completed
   resize hop, and land this plan under `docs/archive/plans/`.

## Non-scope

- No change to `CoreDaemon::resize` control-plane semantics.
- No change to the incremental attach queued-resize contract
  ([[worker applies the latest attach resize before barrier release]]).
- No suppression of `FRAME_RESIZE` on repeated identical client requests. Core still
  delivers one `TerminalInputResult` per accepted command.
- No Hub change, no Hub workaround, and no new Hub test seam.
- No new transport crate, no new adapter trait, and no configurability flag.
- No refactor of the phase split or of the wake registry.

## Repository ownership boundaries and cross-repo dependencies

- Core owns terminal resize, ordering, pressure, registry geometry, and the lifecycle
  journal rows for its sessions ([[core owns duplex terminal transport while Hub stays
  content blind]]).
- Hub stays content blind. Hub only binds a content-blind adapter and calls the targeted
  pump. Hub must need no new call and no new policy for this fix.
- Cross-repo dependency: `botster-hub` (`tgt_7e208a0c76a44980a83b63af976b1f22`) must pin
  the merged Core revision to reproduce
  `session_entity_subscription_pushes_snapshot_ordered_deltas_and_fresh_reconnect`
  green. That Hub pin bump is a separate ticket against the Hub target. This run does not
  edit Hub.
- Downstream-shaped proof inside this run uses `botster-core-daemon` integration tests,
  which drive the same `wait_wakes` + `pump_woken` + waking-adapter shape that Hub uses.

## Assumptions and unknowns

- Assumption A1: the targeted resize already reaches the worker PTY, because it shares
  `apply_targeted_client_ingress` with the input hop proven at `a781556`. The first new
  test asserts live `stty size` output. If that test shows the PTY does not resize, the
  apply hop repair is in scope for this ticket and the plan needs one revision.
- Assumption A4, measured and then superseded as an oracle: the kernel raises `SIGWINCH`
  only when `TIOCSWINSZ` changes the stored size. A local Darwin PTY given 31x101,
  31x101, 32x102 produced only `WINCH-1 31 101` and `WINCH-2 32 102`, and Linux
  `tty_do_resize` returns early on an unchanged size. That measurement is why revision 3
  counts admitted `FRAME_RESIZE` frames instead of signals for every exactly-once claim.
- Assumption A5: one admitted `FRAME_RESIZE` frame equals one PTY resize operation,
  because the worker child performs one `ghostty.resize` plus one runtime resize for each
  received frame and applies no same-size suppression
  (`crates/botster-core-daemon/src/bin/botster-session-worker.rs:248`). The Implementer
  must re-read that handler before relying on the count.
- Assumption A2: "repeated identical resize exactly-once behavior without duplicate
  patches" means the registry save and the lifecycle upsert are skipped when the size is
  unchanged, while each accepted command still produces one `FRAME_RESIZE` and one
  `TerminalInputResult`. Silently dropping a client command would break the no-loss
  requirement in the same ticket.
- Assumption A3: an attach-applied resize and a duplex-applied resize cannot both land in
  one tick, because `apply_woken_terminal_input` parks owners of sessions in
  `incremental_attaches`. The implementation still takes both values and persists once,
  so a future overlap cannot double-publish.
- Unknown U1: whether any current test asserts a lifecycle upsert after an unchanged
  `CoreDaemon::resize`. The Implementer must check before adding the changed-only
  wrapper, and must keep the control-plane path unchanged.

## Affected surfaces and files

- `crates/botster-core/src/engine/managed_session_runtime.rs`: applied-resize map,
  record on targeted resize success, `take_applied_terminal_resize`.
- `crates/botster-core/src/engine/botster.rs`: `take_applied_terminal_resize` on both
  public engine facades; clear the map where session state is dropped, next to the
  existing `applied_attach_resizes` cleanup (`botster.rs:990`, `botster.rs:2007`).
- `crates/botster-core-daemon/src/daemon.rs`: `DaemonEngine::take_applied_terminal_resize`,
  the `pump_woken` persistence step, and the changed-only persistence wrapper.
- `crates/botster-core/src/runtime/control_queue.rs`: one `#[cfg(test)]` `pub(crate)`
  accessor for held `(frame_type, payload)` pairs.
- `crates/botster-core/src/engine/botster/takeover_fail_closed_tests.rs`: the
  operation-count tests T4, T4b, T4c, T4d, and the T6 sibling companion.
- `crates/botster-core-daemon/tests/terminal_wake_test.rs`: targeted wake proofs.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`: run the existing
  persistence-failure regression without a source change.
- `docs/architecture/terminal-adapter.md`: the completed resize hop.
- `docs/archive/plans/pump-woken-worker-backed-resize-completion.md`: this plan.

## Risks

- R1: a new public method on both engine facades is a public contract change. It needs
  downstream-shaped proof, per [[botster core contract surface needs consumer proof]].
  Mitigation: daemon integration tests that use the waking-adapter production shape.
- R2: the changed-only wrapper could hide a legitimate `updated_at` refresh. Mitigation:
  the wrapper is used only by the pump path; the control-plane path keeps the
  unconditional write.
- R3: persisting inside `pump_woken` adds one registry read and write on resize ticks.
  Mitigation: the write happens only when the size changes, and only for named sessions.
- R4: a resize parked by control-queue pressure could be applied twice if both the parked
  entry and a new command are drained. Mitigation: the backpressure test asserts one
  `FRAME_RESIZE`-derived size change, one input result, and no duplicate patch.
- R5: registry save failure must not lose retained output. Mitigation: reuse the shipped
  retain-and-first-error arm and keep `pump_persist_resize_error_retains_once` green.
- R7: a signal-based or final-size oracle cannot detect a duplicate same-size resize call,
  because the kernel suppresses `SIGWINCH` for an unchanged size. Mitigation: the
  exactly-once oracle counts admitted `FRAME_RESIZE` frames before the kernel, and the
  `WINCH` trap is demoted to live-PTY corroboration.
- R8: held-pop tests cannot also observe the live PTY, because held frames never reach the
  worker. Mitigation: the plan splits the two oracles across separate tests and requires
  both, so neither the operation count nor the live final-size proof is dropped.
- R6: an unrelated flake could mask a real failure
  ([[botster core bounded waiting queue test flakes under workspace load]]). Mitigation:
  matched base evidence plus a later green workspace gate.

## Runtime-teardown class answers

`teardown_class_applies`: yes. The ticket is a terminal-state versus live-runtime
divergence. The live worker PTY is 31x101 while the persisted record and the published
lifecycle row stay 24x80.

`teardown_isolation`: the ownership set is one `(session_id, subscription_id,
generation)` owner inside `ClientWorker` plus that session's registry row. A resize
failure tears down only that owner through `owner_apply_teardown`. Sibling sessions and
sibling subscriptions on the same tick keep their owners, their PTY size, and their
records. A persistence failure marks a terminal commit failure for that one session and
continues the loop over the other named sessions.

`teardown_bounds`: the resize apply performs no blocking wait. `enqueue_json` for
`FRAME_RESIZE` returns `ControlAdmission::Full` under pressure instead of blocking, and
the owner parks. Registry save is a bounded local file write; on error the daemon records
the failure, retains the drain result, and returns the first error without dropping
output. No unbounded `block_on` is introduced.

`late_message_matrix`:

| Message | Owner tag | Rejection after terminal failure | Residual sweep |
|---|---|---|---|
| Duplex `Resize` | route key `(session_id, subscription_id)` plus the live generation held in `RouteWakeState` | `failed_sessions` skip, `ControlAdmission::Sealed` hard-stops the owner | applied-resize entry cleared with session state; parked entry cleared on owner teardown |
| Duplex `Input` | same route key | same | unchanged from `a781556` |
| Duplex `ModeGatedInput` | same route key plus `request_id` | same | unchanged from `a781556` |
| Attach queued resize | `incremental_attaches[session_id]` client and subscription | cancelled with the snapshot boundary | `applied_attach_resizes.remove` on detach and teardown |
| Detach / hard stop | `(session_id, subscription_id, generation)` | live-generation check | applied-resize map cleared for the session |

`production_path_proof`: the exact path is adapter readable wake ->
`CoreDaemon::wait_wakes` -> `CoreDaemon::pump_woken` -> `DaemonEngine::pump_woken` ->
`pump_woken_phase_one` -> `apply_woken_terminal_input` -> `apply_one_delivery` ->
`TransportIngress::Resize` -> `FRAME_RESIZE` to the worker -> `pump_woken_phase_three` ->
`take_applied_terminal_resize` -> registry save -> `append_lifecycle_upsert`. The live
oracles are the child `stty size` output, the on-disk `sessions/<id>.json`, and the
lifecycle journal rows. No test may assert the fix through a synthetic config seam alone.

`ownership_identity`: the applied-resize entry is keyed by `SessionId` and is taken and
cleared inside the same targeted tick that produced it, so no stale entry can outlive its
owner. The owner that produced it is identified by the live generation, and every emitted
`TerminalInputResult` stamps the live subscription id
([[every TerminalInputResult must stamp the live subscription id]]). A reused
subscription id cannot inherit a previous owner's pending resize because owner teardown
clears the input queue and the parked flag.

`sibling_fail_closed_policy`: on success, siblings keep running and keep their own sizes.
On resize apply failure, only the failing owner is torn down. On persistence failure, only
the failing session records a terminal commit failure and retains its drain result; the
loop continues for the remaining named sessions. No sibling is sacrificed.

## Acceptance checks and tests

### PTY resize oracle

Revision 2 used a `SIGWINCH` count. Plan Review rejected that, correctly. The kernel
raises `SIGWINCH` only when `TIOCSWINSZ` changes the stored size, so two calls that carry
the same rows and columns raise one signal. A duplicate call for one request carries
exactly the same size, so a signal count can never detect it. A final `stty size` read has
the same blind spot.

The exactly-once proof therefore counts resize operations **before** the kernel, at the
worker-bound control frame seam. The chain is one to one at every hop:

1. `apply_one_delivery` applies one `TransportIngress::Resize`
   (`managed_session_runtime.rs:498`).
2. `SessionRuntimeWorkerAdapter::resize` records one `SessionRuntimeInput::Resize`
   (`managed_session_runtime.rs:2430`).
3. `WorkerProcessRuntime::send_input` admits one `FRAME_RESIZE` control frame
   (`worker_process.rs:1545`).
4. The worker child performs one `ghostty.resize` and one PTY resize for each received
   `FRAME_RESIZE` (`crates/botster-core-daemon/src/bin/botster-session-worker.rs:248`).
   The child applies no same-size suppression of its own.

Counting admitted `FRAME_RESIZE` frames therefore counts PTY resize operations, including
duplicate same-size calls that the kernel would hide.

Mechanism. The in-crate tests already reach this seam.
`crates/botster-core/src/engine/botster/takeover_fail_closed_tests.rs:800` takes
`engine.session_runtime().test_control_queue(&session_id)` and calls
`queue.hold_pops(true)`. Held frames stay in `ControlQueue` instead of reaching the writer
thread. Frames are encoded as `[u32 LE length][u8 frame_type][payload]`
(`session_protocol.rs:436`), so the frame type is byte 4 and the payload is the
`ResizePayload` JSON.

This plan adds one `#[cfg(test)]` `pub(crate)` accessor on `ControlQueue` that returns the
held `(frame_type, payload)` pairs. It sits beside the existing `#[cfg(test)]`
`hold_pops`, ships in no production build, and adds no `CoreDaemonConfig` builder, which
keeps [[spawned Hub tests can reach only four of fourteen Core test builders]] satisfied.

Two oracles with separate jobs, because one test cannot hold both:

- Held pops count operations. Frames never reach the worker, so these tests assert counts
  and payloads only.
- Flowing pops prove the live PTY. These tests assert the live final size, which Plan
  Review requires the plan to keep.

The child in the live tests keeps the counting `WINCH` trap:

```sh
c=0
trap 'c=$((c+1)); printf "WINCH-%d %s\n" "$c" "$(stty size)"' WINCH
printf ready
while IFS= read -r _; do printf "SIZE %s\n" "$(stty size)"; done
```

`crates/botster-core-daemon/tests/daemon_integration_test.rs:4013` already uses a `WINCH`
trap in a real worker child. The trap now serves as live-PTY corroboration and sibling
liveness, never as the exactly-once oracle.

New tests. Each names the live oracle it uses.

Operation-count tests, in-crate in
`crates/botster-core/src/engine/botster/takeover_fail_closed_tests.rs`, over
`WorkerBackedBotsterEngine::new(worker_path())` with `hold_pops(true)`:

- T4 exactly one operation for one request. Inject one compact 31x101 resize frame, wake,
  and pump once. Assert the held queue gained exactly one `FRAME_RESIZE` frame whose
  payload decodes to `rows: 31, cols: 101`. Assert exactly one `resize`
  `TerminalInputResult`. Two frames mean Core resized twice.
- T4b exactly one operation per identical repeated request. Inject 31x101, pump, inject
  31x101 again, and pump again. Assert exactly two `FRAME_RESIZE` frames, both decoding to
  31x101. This is the case the `SIGWINCH` oracle could not see, and it proves one PTY
  resize per accepted request with no coalescing and no duplication.
- T4c ordered distinct sizes. Inject 31x101, pump, then 32x102, and pump. Assert exactly
  two `FRAME_RESIZE` frames in order, 31x101 then 32x102.
- T4d no operation without a request. Pump an ingress-only wake for the same session and
  assert the held queue gained zero `FRAME_RESIZE` frames.
- T7 backpressure retention. Fill the ordinary control queue to
  `ControlAdmission::Full`, following the shipped pattern at
  `takeover_fail_closed_tests.rs:800`, so the resize parks. Assert zero `FRAME_RESIZE`
  frames admitted while the owner stays parked, which proves retention without an early
  apply. Release capacity, deliver the matching session ingress wake, and assert exactly
  one `FRAME_RESIZE` frame carrying 31x101. A daemon-level companion then asserts one
  record update and one lifecycle patch. Together these prove retention without loss and
  without duplication.

Live-path tests, in `crates/botster-core-daemon/tests/`, with pops flowing and the real
worker binary:

- T1 live PTY final size, worker-backed. Spawn through `with_worker_path(worker_path())`
  with the counting `WINCH` child. Bind a waking adapter, inject one 31x101 frame, and
  pump. Assert the delivered output reports `31 101`, which proves the named PTY reached
  the requested size through the live path.
- T2 persistence. Same tick as T1. Assert `sessions/<id>.json` through
  `daemon.registry().load(...)` reports `(31, 101)`.
- T3 one lifecycle patch. Assert exactly one lifecycle upsert carries `rows=31` and
  `cols=101` after the pump.
- T5 repeated identical resize, durable side. Inject 31x101 twice across two pumps. Assert
  two `resize` input results, one persisted record at `(31, 101)`, and exactly one
  lifecycle patch. T4b already proved the operation count for this case.
- T6 sibling isolation. Two worker-backed sessions with one bound route each, both running
  the counting `WINCH` child. Resize session A only. Assert A reports `31 101` and the
  record `(31, 101)`. Assert session B reports zero `WINCH-` lines, keeps `(24, 80)` in
  the record and in `stty size`, and receives no lifecycle patch. The in-crate companion
  asserts zero `FRAME_RESIZE` frames on session B's control queue.
- T8 preservation. In one scenario, assert ordinary input still echoes, mode-gated input
  still resolves, lifecycle commit still runs, and output delivery still reaches the
  adapter after the resize tick.
- T9 regression guard. `pump_persist_resize_error_retains_once` must stay green, which
  keeps the attach-resize persistence path and its retain-once behavior intact.

Red ablations. Each is a separate revert with recorded red output.

- A1 resize apply. Remove the `TerminalInputCommand::Resize` targeted apply arm.
  Expected red: T1, T4, T4b, T4c, T6. The operation-count tests go red with zero
  `FRAME_RESIZE` frames, and T1 goes red with no `31 101` report, which proves the
  assertions read the real seam and the live PTY rather than a cached value.
- A2 persistence. Remove only the registry save in the pump persistence step and keep the
  patch append. Expected red: T2, T6.
- A3 patch publication. Remove only the `append_lifecycle_upsert` call in the pump
  persistence step and keep the registry save. Expected red: T3, T5.

Repository gates ([[botster-core uses CI-owned Cargo commands because it has no test
script]] plus `.github/workflows/ci.yml`):

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
cargo test -p botster-core --no-default-features --lib
BOTSTER_ENV=test cargo test --doc --workspace
cargo test -p botster-core --test local_process_runtime_test
```

Downstream proof:

- In this run: the daemon integration tests above drive the Hub-shaped
  `wait_wakes` + `pump_woken` + waking-adapter path, not a helper call.
- After merge, in the Hub repository under a separate ticket: pin the merged Core
  revision and run
  `session_entity_subscription_pushes_snapshot_ordered_deltas_and_fresh_reconnect` with a
  requested 31x101 resize, and require a live green result rather than a soft residual.

Worktree hygiene: the tracked `.gitignore` is 63 bytes and intact. The worktree path
contains no `:`, so no `CARGO_TARGET_DIR` override is required.

## Implementation allocation

Implementation keeps the approved behavior and ownership scope. It places the new
registry, lifecycle-patch, repeated-identical, and sibling proofs in
`terminal_wake_test.rs`. That test target already owns the Hub-shaped `wait_wakes` plus
`pump_woken` path and can run real worker PTYs. `daemon_integration_test.rs` remains
unchanged and supplies the existing `pump_persist_resize_error_retains_once` regression.
No acceptance oracle or runtime-teardown lens was removed.

## Vault gaps worth capturing

- G1: "the targeted wake pump completes resize, persistence, and one lifecycle patch".
  This extends [[session registry size follows the worker applied resize]] from the attach
  barrier to the duplex wake path.
- G2: "registry geometry writes are idempotent for unchanged sizes on the wake path".
  Capture after T5 lands, so a later ticket does not reintroduce duplicate patches.
- G3: "a Core control-plane command path and a Core duplex data path must reach the same
  durable record". This is the general lesson behind the reported divergence.
