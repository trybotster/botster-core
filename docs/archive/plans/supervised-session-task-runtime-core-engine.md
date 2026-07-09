# Design Supervised Session Task Runtime For Core Engine

Ticket: `ticket_1780189402_252733`
Run: `run_1780189444_842476`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Design supervised session task runtime for core engine`, run `run_1780189444_842476`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts, reviews, findings, open questions, or prior answers.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster overlay notes loaded:
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `botster data plane bypasses the hub through session and client actors`
  - `sessionioworker is the production read path for session pty output`
  - `botster plugin runtime uses supervisor plus per plugin workers`
  - `botster client subscriptions should not hydrate global state`
  - `synced state types are allowed while pushed event variants are forbidden`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
- General vault context loaded:
  - `[[identity]]`
  - `[[goals]]`
- Repo context inspected:
  - `README.md`: core owns reusable transport-neutral mechanisms and typed contracts; hub/hosts own runtime policy, concrete adapters, auth, persistence, transport, and async supervision.
  - `crates/botster-core/src/runtime/mod.rs`: existing `SessionRuntime`, `SessionRuntimeInput`, `SessionRuntimeOutput`, and typed `SessionRuntimeError`.
  - `crates/botster-core/src/engine/session_worker.rs`: existing request/event worker state machine over `SessionWorkerRuntime`, including writer routing, resize, snapshots, initial-output ordering, shutdown, and output activity.
  - `crates/botster-core/src/engine/subscription_multiplexer.rs`: existing multi-client subscription fanout and request routing.
  - `crates/botster-core/src/engine/multiplexer.rs`: existing host-facing facade that coordinates sessions, workers, subscriptions, activity/lifecycle, notifications, and plugins, but does not yet own a reusable runtime-drain/read-loop protocol.
  - `crates/botster-core/src/contract/actor.rs`: existing lifecycle, backpressure, queue failure, and session I/O request/event contracts.
  - `crates/botster-core-test-support/src/fake/session_worker.rs` and `crates/botster-core-test-support/src/fake/mod.rs`: existing fake worker runtime, fake mailbox, and fake session runtime.
  - Existing tests for session runtime, session worker, subscription multiplexer, and assembled multiplexer API.
- Prior plan artifacts inspected:
  - `docs/archive/plans/core-session-worker-engine.md`
  - `docs/archive/plans/core-subscription-multiplexer-routing-engine.md`
  - `docs/archive/plans/core-session-model-activity-engine.md`
  - `docs/archive/plans/assemble-core-multiplexer-engine-api.md`

Project Pipelines checklist: run-level checklist `checklist_1780189516_894073` created for vault workflow evidence after an initial plugin timeout; evidence should name notes read, convention conflicts, verification commands, and capture decision.

Plan review context loaded after sequence 2 returned to Plan:

- Review `review_1780189975_416525` returned `changes_required`.
- Open findings loaded:
  - `finding_1780189975_125719`: missing data-plane actor boundary note.
  - `finding_1780189976_988841`: supervisor ownership not reconciled with SessionIoWorker/ClientWorker.
  - `finding_1780189976_436762`: per-client backpressure boundary under-specified.
  - `finding_1780189976_264798`: missing plugin-supervisor and client-subscription notes; naming collision.
  - `finding_1780189976_379886`: optional synced-state-vs-pushed-event guard.
- Resolution in this revision: loaded the missing notes, added the architecture-fit boundary below, scoped backpressure to session-side failures, disambiguated the new runtime name from the plugin supervisor, and added the optional actor/transport guard.

## Architecture Fit

The new core runtime must compose the established data-plane actor model. In product terms, `SessionIoWorker` remains the session-side owner of session I/O: the read loop, write queue, snapshot/scrollback source, and session-output basis for fanout. `ClientWorker` remains the per-client owner of subscription state, transport encoding, outbound queueing, and slow-client backpressure. The hub or host remains coordination policy, not byte relay.

In `botster-core`, the proposed engine is therefore a scheduling-neutral coordinator over the existing core equivalents of those actors: `SessionRuntime` plus `SessionWorkerEngine` for the session-side read/write path, and `MultiplexerEngine` / `SubscriptionMultiplexer` for client fanout. It must not introduce a second byte-relay layer or bypass the worker/multiplexer path. The public reader tick exists only to define what happens when the session-side read owner is polled; hosts still decide where that read owner is scheduled.

To avoid collision with the existing plugin runtime supervisor, implementation should prefer a name that includes the session runtime context, such as `SessionRuntimeSupervisor` or `ManagedSessionRuntime`. This is distinct from the plugin supervisor that owns per-plugin Lua workers.

## Scope

Add the core supervision layer that turns the existing runtime, session worker, and multiplexer primitives into managed live-session mechanics without choosing a concrete executor or transport.

In scope:

- Add a small reusable managed session runtime under `crates/botster-core/src/engine/`, likely `session_runtime_supervisor.rs` or `managed_session_runtime.rs`.
- Model scheduling-independent supervision semantics:
  - reader-loop ownership as an explicit `drain` / `poll` / `tick` method that hosts can call from any executor;
  - runtime output conversion from `SessionRuntimeOutput` into `SessionWorkerRuntimeEvent`;
  - routing of worker-emitted `SessionIoEvent` through the existing `MultiplexerEngine` / `SubscriptionMultiplexer` path;
  - writer request routing from client ingress through the multiplexer and worker into a host runtime input adapter;
  - resize forwarding through the same writer route before snapshots;
  - process-exit ordering that flushes terminal output before lifecycle exit;
  - lifecycle transitions for starting, running, stopping, exited, and failed;
  - clean shutdown that closes the worker/runtime exactly once and makes repeated shutdown idempotent or typed;
  - typed supervisor errors for host-visible runtime failures and invalid lifecycle transitions;
  - session-side backpressure/failure observations using existing `MailboxSendFailure`, `BackpressureSummary`, `QueueSource::SessionIo`, and route types where possible.
- Preserve the ClientWorker boundary: per-client slow-transport backpressure stays with client actors/adapters and must not be centralized in the managed session runtime.
- Preserve the subscribe/hydration boundary: `SubscribeSession` in tests establishes transport/session fanout only; it must not imply route registry, entity, UI tree, or global snapshot hydration.
- Keep the execution model synchronous and replaceable. Hosts should be able to wrap the supervisor in Tokio, threads, a TUI loop, or tests without core depending on any of them.
- Add or extend fake test support so tests drive a fake runtime through the public supervised path rather than manually injecting worker events only.
- Export only the minimal public types needed by embedders.
- Update `README.md` only if the new public supervisor contract needs a narrow ownership-boundary row or paragraph.

Non-scope:

- No concrete PTY implementation, process spawning implementation, thread pool, Tokio task, channel runtime, Unix socket, WebRTC, Rails, TUI, React, Lua plugin, MCP, provider, or product daemon wiring.
- No Project Pipelines workflow behavior or UI behavior.
- No auth, persistence, retention, reconnect policy, admitted-target selection, executable discovery, environment inheritance, or restart strategy.
- No broad rewrite of `SessionWorkerEngine`, `SubscriptionMultiplexer`, `MultiplexerEngine`, or runtime contracts unless the implementation proves a narrow gap.
- No new dependency is expected.
- No compatibility branch, version suffix, old trybotster import, or duplicate parallel API.

Botster layers touched:

- Rust `botster-core` engine layer: primary surface.
- Rust `botster-core` runtime contract layer: only if the supervisor needs a tiny typed error or adapter shape that cannot live in the engine module.
- Rust `botster-core-test-support`: fake runtime/supervisor helpers for conformance tests.
- Docs: this plan and possibly a narrow README ownership-boundary update.

Worktree/target assumption: implementation agents operate in this assigned pipeline worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts: this document is the repo-visible Plan artifact. Gate evidence should cite it plus checklist evidence.

## Assumptions And Unknowns

Assumptions:

- This ticket builds on already-present `SessionRuntime`, `SessionWorkerEngine`, `SubscriptionMultiplexer`, and `MultiplexerEngine`; it should not restate those lower-level tickets.
- The missing reusable behavior is the managed live-session task runtime: a deterministic bridge that coordinates session read/drain sequencing, write routing, lifecycle transitions, and shutdown/error surfacing through the existing SessionIoWorker/ClientWorker-shaped core paths.
- "Reader loop ownership" can be represented without spawning a thread by exposing a host-called drain/tick method. Core owns what happens when the reader is run; hosts own when and where it is scheduled.
- `SessionRuntime::drain_output` is the existing runtime primitive to adapt for reader events. If it is insufficient, add the smallest contract change needed rather than introducing a concrete channel or executor.
- `SessionWorkerRuntime` may need an adapter implementation backed by `SessionRuntimeInput` so writer requests can be proven to hit the runtime, not just a fake command recorder.
- Supervisor errors should wrap or classify `SessionRuntimeError` and add only supervisor-specific cases such as unknown session, already closed, duplicate shutdown, or lifecycle violation.
- Shutdown "closes the worker exactly once" means runtime shutdown is invoked once for a supervised session even if callers repeat shutdown, process exit arrives concurrently in test order, or a later request tries to write.
- Process exit flushing must be tested through the supervisor path, not only the existing `SessionWorkerEngine` unit tests.
- The production entry point changed by this ticket can be a public core API and tests using fake runtime adapters. Host daemon wiring is intentionally out of scope.
- Actor/transport enum changes, if any, must not add pushed terminal-mode/color event variants. Typed synced-state structs remain allowed in session protocol surfaces.

Unknowns for implementation:

- Exact naming is open. Prefer names that avoid plugin-supervisor ambiguity, such as `ManagedSessionRuntime`, `ManagedSessionRuntimeOutcome`, and `ManagedSessionRuntimeError`.
- Whether the supervisor should be a standalone per-session engine or an extension of `MultiplexerEngine`. Prefer the smallest shape that proves reader events reach multiplexer fanout and writer requests route to runtime.
- Whether `SessionRuntimeInput::Shutdown` needs a reason field to preserve the existing `SessionIoRequest::Shutdown` reason. Avoid changing it unless the test cannot prove shutdown semantics without it.
- Whether session-side backpressure belongs as a managed-runtime fake mailbox helper or as delegated observations from existing session queues. Prefer reusing existing typed queue contracts; do not absorb per-client transport backpressure.
- Whether README docs are necessary. Add them only if the public API boundary is otherwise unclear.

No human question is blocking the plan. The ticket intent is clear and can be satisfied without waiving scope.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/session_runtime_supervisor.rs` or `crates/botster-core/src/engine/managed_session_runtime.rs`
  - New scheduling-neutral supervision state machine and public outcome/error types.
  - Runtime drain/read-loop tick method.
  - Writer routing adapter into runtime input.
  - Lifecycle, shutdown, and ordered event handling.
- `crates/botster-core/src/engine/multiplexer.rs`
  - Possible narrow integration method so supervised runtime output can route through the existing multiplexer fanout and activity/lifecycle path.
  - Possible direct session request/runtime adapter hook if writer routing should be proven through the facade.
- `crates/botster-core/src/engine/mod.rs`
  - Export the supervisor module and public types.
- `crates/botster-core/src/lib.rs`
  - Re-export host-facing supervisor types.
- `crates/botster-core-test-support/src/fake/mod.rs`
  - Extend fake runtime support for supervised-session tests if needed.
- `crates/botster-core-test-support/src/fake/session_worker.rs`
  - Possible helper to bridge `SessionWorkerRuntime` calls into `FakeSessionRuntime` inputs, or a new fake supervisor runtime module.
- `crates/botster-core/tests/managed_session_runtime_test.rs`
  - New acceptance tests that prove the live supervised path end to end.
- `README.md`
  - Optional narrow update to document the supervisor boundary.
- `docs/archive/plans/supervised-session-task-runtime-core-engine.md`
  - This plan artifact.

Possible but avoid unless compiler/API shape requires:

- `crates/botster-core/src/runtime/mod.rs`
  - Only for typed supervisor/runtime error alignment or a tiny input variant change.
- `crates/botster-core/src/engine/session_worker.rs`
  - Only for narrow accessors or outcome details needed by the supervisor.
- `crates/botster-core/src/contract/actor.rs`
  - Only if current typed failure/backpressure/lifecycle contracts cannot represent required host-visible surfaces.

Not expected:

- `Cargo.toml` dependency changes.
- `crates/botster-core-dev`.
- Any hub, CLI, browser, TUI, Rails, Lua plugin, MCP, provider, cloud, Project Pipelines plugin, or old trybotster files.

## Implementation Shape

Suggested minimal shape:

- A public supervisor type that owns or references:
  - target `SessionId`;
  - existing `SessionRuntime` or an adapter around it;
  - existing `SessionWorkerEngine`;
  - existing `MultiplexerEngine` / `SubscriptionMultiplexer` path for fanout;
  - lifecycle/shutdown state;
  - optional session-side failure/backpressure observations.
- Public operations:
  - `handle_session_request(request, now_seconds)` or equivalent writer entry point;
  - `drain_runtime_once(now_seconds)` / `tick_reader(now_seconds)` that calls the runtime drain primitive, converts output to `SessionWorkerRuntimeEvent`, and returns ordered core outcomes;
  - `shutdown(reason, now_seconds)` that routes shutdown through the worker/runtime once and transitions lifecycle;
  - optional `is_closed` / lifecycle accessors for tests and embedders.
- A bridge from `SessionRuntimeOutput::PtyOutput` to `SessionWorkerRuntimeEvent::TerminalBytes`.
- A bridge from `SessionRuntimeOutput::ProcessExited` to `SessionWorkerRuntimeEvent::ProcessExited`.
- A bridge from writer requests to `SessionRuntimeInput::{PtyInput, Resize, Shutdown}` so tests can assert writes reach the runtime.
- Typed `ManagedSessionRuntimeError` or equivalent with stable variants and `thiserror` display:
  - runtime failure wrapping `SessionRuntimeError`;
  - unknown or mismatched session if needed;
  - closed session write/shutdown behavior if surfaced as an error rather than observation.
- Outcomes should reuse existing `MultiplexerEngineOutcome` or map directly to its fields where possible. Do not invent a second vocabulary for client egress, session events, lifecycle, or backpressure.

Behavior details to pin:

- Reader output drained before process exit is emitted as `TerminalBytes` before `ProcessExited`.
- Runtime drain errors become typed supervisor errors and lifecycle failure observations where appropriate.
- Client input routed through the supervised path reaches `SessionRuntimeInput::PtyInput`.
- Resize routed through the supervised path reaches `SessionRuntimeInput::Resize` before snapshot or initial snapshot handling.
- Subscribe in this path establishes the route used for fanout; it must not hydrate global route registries, plugin entities, UI trees, or surface state.
- Shutdown emits pending output before shutdown/lifecycle transition and calls runtime shutdown exactly once.
- Later writes after shutdown do not reach the runtime; they produce a typed closed error or observation.
- Reader tick after shutdown/exited is a no-op or typed lifecycle observation, not a second close.

## Risks

- Building a real async executor in core would violate the replaceable host boundary.
- Adding supervision as a parallel byte-relay facade beside `SessionWorkerEngine` / `MultiplexerEngine` could duplicate SessionIoWorker/ClientWorker semantics. Prefer composing existing public engines.
- Tests that inject `SessionWorkerRuntimeEvent` directly would not prove the new reader-loop path. Acceptance tests must drive `SessionRuntime::drain_output` or the final equivalent public runtime boundary.
- Tests that only inspect fake worker commands would not prove writes route to the runtime. Acceptance tests must assert `SessionRuntimeInput` values on the fake runtime or equivalent.
- Shutdown can double-close if both explicit shutdown and process exit paths call the runtime cleanup. Tests need the exact-count assertion.
- Process exit can race ahead of buffered output if the drain loop preserves runtime batch order incorrectly.
- Typed errors can become vague if they are strings or raw JSON. Use stable enum variants and existing `SessionRuntimeErrorKind` where possible.
- Concrete transport/product vocabulary in public contracts would violate the Botster core boundary.
- Centralizing per-client slow-transport backpressure in this runtime would violate ClientWorker ownership; keep client-side pressure at the client actor/adapter layer.
- Adding pushed terminal-mode/color event variants while touching actor/transport contracts would violate the synced-state-vs-pushed-event rule.
- Broad changes to existing lower-level engines could destabilize already-proven contracts. Keep edits surgical.

## Acceptance Checks / Tests

Run:

- `cargo fmt`
- `cargo test -p botster-core supervised_session`
- `cargo test -p botster-core session_worker`
- `cargo test -p botster-core multiplexer_engine`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Add targeted tests in `crates/botster-core/tests/managed_session_runtime_test.rs`:

1. `reader_events_reach_subscription_multiplexer`
   - Spawn/register a supervised session through the public core path.
   - Subscribe a client using `TransportIngress::SubscribeSession`.
   - Assert subscribe establishes only the transport/fanout route, with no route registry, entity, UI tree, or global snapshot hydration.
   - Queue fake runtime output and run one reader tick.
   - Assert resulting `TransportEgress::TerminalOutput` is emitted for the subscribed client.

2. `writer_requests_route_to_session_runtime`
   - Send `TransportIngress::TerminalInput` or direct `SessionIoRequest::PtyInput` through the supervised path.
   - Assert fake runtime receives `SessionRuntimeInput::PtyInput` with exact bytes.

3. `resize_forwarding_reaches_runtime_before_snapshot`
   - Route resize through the supervised path, then request a snapshot or initial snapshot.
   - Assert fake runtime records `SessionRuntimeInput::Resize` before the snapshot-related action.

4. `shutdown_closes_worker_and_runtime_exactly_once`
   - Call shutdown twice and/or send a write after shutdown.
   - Assert only one runtime shutdown input/command is recorded.
   - Assert later writes are rejected or observed as closed without reaching runtime.

5. `process_exit_flushes_ordered_output_before_lifecycle_exit`
   - Queue fake runtime output followed by process exit in one reader drain.
   - Assert terminal output reaches the multiplexer before process-exit client/lifecycle output.
   - Assert the recorded session lifecycle becomes exited after output handling.

6. `runtime_drain_error_returns_typed_supervisor_error_for_hosts`
   - Configure fake runtime drain to return a typed `SessionRuntimeError`.
   - Assert the public managed runtime returns a typed `ManagedSessionRuntimeError` or equivalent, not a string/JSON error.

7. `supervisor_surfaces_session_side_queue_full_and_closed_failures`
   - Use bounded fake queues/mailboxes where applicable.
   - Assert `MailboxSendFailureReason::QueueFull` and `QueueClosed` preserve `QueueSource::SessionIo` and typed route context.
   - Assert per-client slow-transport backpressure remains outside this runtime and is not centralized here.

8. `supervisor_contract_excludes_concrete_transport_and_product_policy`
   - Source/type guard for the new supervisor module banning concrete WebRTC, browser, TUI, ActionCable, Rails, Project Pipelines, auth, retention, reconnect, cloud, and product config vocabulary.

9. `supervisor_does_not_add_pushed_terminal_mode_event_variants`
   - If actor or transport enums are touched, assert the change adds no pushed terminal-mode/color event variants. Typed synced-state structs remain allowed in session protocol surfaces.

Existing tests expected to remain green:

- `crates/botster-core/tests/session_runtime_contract_test.rs`
- `crates/botster-core/tests/session_worker_engine_test.rs`
- `crates/botster-core/tests/subscription_multiplexer_engine_test.rs`
- `crates/botster-core/tests/multiplexer_engine_api_test.rs`

Runtime/user path proof:

- This ticket is intentionally core-engine work, not host daemon wiring.
- The changed production entry point must be an exported `botster_core` supervisor/facade API.
- Acceptance evidence must show tests driving real public runtime output through the managed session runtime into the existing worker/multiplexer path and real public writer requests into the fake runtime. Code existence alone is not sufficient.

## Vault Checklist Evidence

- Vault/project notes constraining the plan: `planner-playbook`, `botster-planner-playbook`, `botster-architecture`, `cli-patterns`, `spa-patterns`, `botster data plane bypasses the hub through session and client actors`, `sessionioworker is the production read path for session pty output`, `botster plugin runtime uses supervisor plus per plugin workers`, `botster client subscriptions should not hydrate global state`, `synced state types are allowed while pushed event variants are forbidden`, Project Pipelines orchestration/workbench/UI-contract notes, explicit target/worktree orchestration notes, and identity/goals context.
- Convention conflicts: none after revision. The plan keeps reusable, scheduling-neutral session runtime coordination in Rust `botster-core`, composes the SessionIoWorker/ClientWorker-shaped data-plane boundaries, avoids plugin-supervisor ambiguity, and leaves host scheduling, concrete transports, product policy, global hydration, per-client backpressure policy, and UI outside core.
- Verification evidence so far: planning inspection only; no implementation tests were run. Planned verification commands are listed above.
- Durable knowledge capture: no new vault note required before implementation. Capture later if the implementation settles a durable convention for supervised session runtime naming, shutdown semantics, or host-owned scheduling boundaries.

## Vault Gaps Worth Capturing

- Capture if the final API establishes a stable Botster convention that core owns host-called reader ticks while hubs own executor scheduling.
- Capture if "shutdown exactly once" semantics gain a durable rule for explicit shutdown versus process-exit ordering.
- Capture if the implementation introduces a reusable typed supervisor error vocabulary that other core engines should follow.
