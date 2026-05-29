# Build Core Subscription Multiplexer And Client Routing Engine

## Context Loaded

- Pipeline context: `ticket_1780075965_242935`, `run_1780092075_837515`, current step `botster_plan`, gate `botster_plan_gate`.
- Orchestrator correction: this run is main-rooted. Dependency-derived base fields in the run metadata are stale; downstream work should target `main` and must not create a stacked PR.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Additional vault constraints loaded:
  - `identity`
  - `goals`
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
  - `botster client subscriptions should not hydrate global state`
  - `plan steps need reviewable plan artifacts`
- Repo context loaded:
  - `README.md`: core owns reusable mechanisms and transport-neutral contracts; hub owns runtime policy, lifecycle, orchestration, adapters, product workflows, and concrete transport policy.
  - `Cargo.toml` and `crates/botster-core/Cargo.toml`: workspace is Rust-only; core already uses serde/thiserror and strict lint settings including `missing_docs` and `unwrap_used`.
  - `crates/botster-core/src/contract/client_stream.rs`: existing single-client stream harness tracks one client's active `SessionId -> SubscriptionId` route and handles duplicate/replacement subscriptions, dropped unsubscribed input, pong request ids, and typed backpressure observations.
  - `crates/botster-core/src/engine/session_worker.rs`: existing session worker engine consumes `SessionIoRequest` and emits `SessionIoEvent` through a host-supplied runtime.
  - `crates/botster-core/src/contract/actor.rs`: existing `SessionIoRequest`, `SessionIoEvent`, `BackpressureRoute`, `BackpressureSummary`, and client/session mailbox contracts.
  - `crates/botster-core/src/contract/transport.rs`: existing typed transport ingress/egress frames with `subscription_id` on subscription-routed egress.
  - `crates/botster-core/tests/client_stream_contract_test.rs`: proves the current single-client harness but not fanout across multiple clients.
  - `crates/botster-core/tests/session_worker_engine_test.rs`: proves session worker behavior but not client multiplexer routing.
  - `docs/plans/client-stream-contract.md` and `docs/plans/core-session-worker-engine.md`: prior dependency plans and acceptance mapping.
- Plan Review context loaded on return to Plan: `review_1780092536_755130` and findings `finding_1780092536_619625`, `finding_1780092536_571596`, and `finding_1780092536_540100`.
- Review resolution: snapshot, initial-snapshot, and scrollback responses are per-client pull responses under the pull-based hydration convention and are moved out of scope for this ticket. Scope and acceptance now align to terminal bytes, process exits, duplicate/replacement subscriptions, dropped input, ping/pong, typed backpressure, and attach state only if explicitly verified.
- Project Pipelines checklist: run-level vault checklist `checklist_1780092124_748593` created after an initial plugin-worker timeout; items should record notes read, convention conflict review, verification evidence, and capture decision.

## Scope

Build the reusable core subscription multiplexer and client routing engine that coordinates multiple clients over the existing single-client stream harness and session worker contracts.

In scope:

- Add a pure, synchronous multiplexer engine under `crates/botster-core/src/engine/`.
- Track multiple `ClientId` values and their per-session `SubscriptionId` routes without concrete transport types.
- Translate client ingress into session I/O requests only when the sending client has an active route for the target session.
- Fan out session-pushed events that are session-wide by contract: terminal bytes, process exits, and attach-state updates.
- Preserve duplicate subscription idempotency and replacement subscription semantics per client and per session.
- Preserve ping/pong request ids through the client route.
- Surface dropped/observed input from unsubscribed clients without emitting `SessionIoRequest`.
- Surface typed backpressure summaries with `ClientId`, `SessionId`, and `SubscriptionId` in `BackpressureRoute`.
- Reuse `ClientStreamHarness`, `TransportIngress`, `TransportEgress`, `SessionIoRequest`, `SessionIoEvent`, and backpressure contracts wherever possible instead of inventing a parallel vocabulary.
- Compose only the non-generation `ClientStreamHarness` methods. Generation/reconnect dedup stays in hub/client reconnect policy and must not enter this core multiplexer.
- Add tests that drive the exported multiplexer engine, not just enum construction.

Non-scope:

- No WebRTC, TUI, Unix socket, ActionCable, Rails, browser, or concrete adapter code.
- No Tokio tasks, channels, process spawning, or runtime supervision.
- No hub lifecycle, permissions, reconnect policy, persistence, UI, plugin workflow, Project Pipelines, or product policy.
- No fanout of snapshot, initial-snapshot, or scrollback responses in this ticket. Those are per-client pull responses, and current core contracts carry no requesting-client identity for safe multiplexer targeting.
- No focus-change fanout unless implementation finds an existing session-wide contract and adds an explicit test. Focus ingress may still be dropped/observed for unsubscribed clients.
- No broad refactor of existing client-stream or session-worker contracts unless a compiler-proven gap blocks the multiplexer.
- No old trybotster path dependency; old paths remain reference evidence only.
- No new dependencies expected.

Botster layer touched: Rust `botster-core` engine and contract-test layer, specifically the session/client data-plane mechanism.

Worktree/target assumption: downstream agents operate in this assigned `botster-core` worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts: this document is the repo-visible Plan artifact. Gate evidence should cite this file plus the loaded vault/repo context.

## Assumptions And Unknowns

Assumptions:

- The existing `ClientStreamHarness` is the right per-client primitive. The new multiplexer should compose it rather than duplicate its single-client routing rules.
- The core runtime path for this ticket is intentionally scaffold-level but executable: tests instantiate the exported multiplexer and drive real `TransportIngress` and `SessionIoEvent` values through it.
- Session workers remain the session-side engine; the multiplexer routes to and from them but does not own PTY execution or snapshot production.
- A client may subscribe to multiple sessions, and multiple clients may subscribe to the same session.
- Duplicate subscription means same client, same session, same subscription id. Replacement means same client and session with a different subscription id.
- Unsubscribing an old replaced subscription id must not detach the newer active route.
- Fanout should use the current subscriber set at event-handling time. Former subscribers must not receive later terminal output or process exits.
- Snapshot, initial-snapshot, and scrollback responses are not safe to broadcast because the current contract carries only `SessionId` and data, not requesting-client identity. Implementers must not route these as session-wide fanout in this ticket.
- The multiplexer deliberately uses the non-generation harness APIs. `ClientStreamHarness` generation-aware methods exist for reconnect paths, but reconnect dedup remains hub/client policy.
- Backpressure is an observable typed route condition, not retry/coalescing policy.

Unknowns for implementation:

- Exact public naming is open. Prefer direct names such as `SubscriptionMultiplexer`, `SubscriptionMultiplexerOutcome`, and `SubscriptionRouteObservation`.
- Whether the multiplexer outcome should reuse `ClientStreamOutcome` per client or define a wrapper such as `client_egress: Vec<(ClientId, TransportEgress)>` plus `session_requests` and observations. Prefer the wrapper if it makes multi-client fanout explicit.
- Whether attach state should be a multiplexer method or represented through an existing event.
- Whether disconnected-client cleanup needs a `remove_client` method in this ticket. Include it only if required to prove current subscribers only; avoid reconnect policy.

No human question is needed before implementation. The ticket intent is clear and all acceptance items can be satisfied within core without waiving scope.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/subscription_multiplexer.rs`
  - New pure multi-client routing engine.
  - Registry of client stream harnesses and current session subscriber indexes.
  - Methods for client ingress, session events, attach state, typed backpressure, and optional client removal.
- `crates/botster-core/src/engine/mod.rs`
  - Export the new engine module and public types.
- `crates/botster-core/src/lib.rs`
  - Re-export public multiplexer types if they are intended as host-facing core API.
- `crates/botster-core/tests/subscription_multiplexer_engine_test.rs`
  - New acceptance tests that drive the multiplexer behavior end to end.
- `crates/botster-core/src/contract/client_stream.rs`
  - Possible small additions only if the multiplexer needs a reusable outcome wrapper or observation already implied by existing semantics.
- `crates/botster-core/src/contract/actor.rs` or `transport.rs`
  - Possible minimal additions only if an acceptance item cannot be represented by existing typed contracts.
- `docs/plans/core-subscription-multiplexer-routing-engine.md`
  - This plan artifact.

Not expected:

- `Cargo.toml` or crate dependency changes.
- `README.md`, unless implementation adds a public boundary detail that should be documented.
- `crates/botster-core-dev`.
- Old trybotster source paths.
- Any hub, browser, TUI, Rails, Lua plugin, or MCP files.

## Implementation Shape

Suggested minimal API:

- `SubscriptionMultiplexer::new()`.
- `handle_ingress(TransportIngress) -> SubscriptionMultiplexerOutcome`.
- `handle_session_event(SessionIoEvent) -> SubscriptionMultiplexerOutcome`.
- `handle_attach_state(session_id, state) -> SubscriptionMultiplexerOutcome`.
- `report_backpressure(client_id, summary) -> SubscriptionMultiplexerOutcome`.
- Optional `remove_client(client_id) -> SubscriptionMultiplexerOutcome` for detach cleanup if tests need it.

Suggested outcome shape:

- `client_egress: Vec<(ClientId, TransportEgress)>`
- `session_requests: Vec<(SessionId, SessionIoRequest)>`
- `client_control_frames: Vec<(ClientId, ClientControlFrame)>`
- `observations: Vec<...>`

Suggested behavior:

- On `SubscribeSession`, create or reuse that client's `ClientStreamHarness`, pass the ingress through it, and update any subscriber index from the harness's active route.
- On duplicate subscribe, preserve idempotency and do not emit duplicate session subscription churn.
- On replacement subscribe, remove the old client/session route from the subscriber index and activate only the new subscription id.
- On `UnsubscribeSession`, detach only when the id matches the current active route.
- On terminal input, resize, snapshot request, send-file, and focus ingress, route only if the client has an active subscription for that session; otherwise emit dropped observations and no `SessionIoRequest`.
- On ping/heartbeat, return pong for the requesting client with the same `RequestId`.
- On session event fanout, send terminal output, process-exit, and attach-state frames to each current subscriber of that session, each carrying that subscriber's active `SubscriptionId`.
- Do not fan out `SnapshotReady`, initial snapshots, or scrollback. Current contracts do not identify the requesting client, so broadcasting them would violate pull-based hydration.
- On backpressure, preserve the typed route context and keep policy decisions out of core.

## Risks

- Duplicating `ClientStreamHarness` logic in a new engine would create two subtly different subscription semantics. Compose the existing harness unless there is a concrete reason not to.
- A multiplexer that only stores route data but does not drive `TransportIngress` and `SessionIoEvent` through behavior tests would fail the runtime-path proof requirement.
- Fanout can accidentally include stale subscribers after replacement or unsubscribe. Tests must assert old subscription ids stop receiving output.
- Snapshot, initial-snapshot, and scrollback responses can be accidentally treated as broadcast events even though they are per-client pull responses without requester identity in the current contract. This ticket must leave them out of multiplexer fanout.
- Backpressure can become vague if represented as strings or raw JSON. Use `BackpressureSummary` and `BackpressureRoute`.
- It is easy to smuggle reconnect, permissions, or concrete transport policy into the engine. Those remain hub/client responsibilities.
- `ClientStreamHarness` has generation-aware reconnect methods; using them here would import reconnect dedup policy into core. The multiplexer should compose the non-generation methods only.
- Adding concrete transport names would violate the core boundary and README ban list.
- Broadly reshaping existing transport egress variants could churn existing tests without adding required behavior.

## Acceptance Checks / Tests

Run:

- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Add targeted tests in `crates/botster-core/tests/subscription_multiplexer_engine_test.rs`:

1. `multiple_clients_can_subscribe_to_one_session`
   - Subscribe two clients to the same session with different subscription ids.
   - Feed terminal output and assert both clients receive one egress frame with their own subscription id.

2. `one_client_can_switch_subscriptions`
   - Subscribe one client to a session with `sub-old`, then resubscribe with `sub-new`.
   - Assert replacement observation.
   - Feed terminal output and process exit; assert only `sub-new` is used.

3. `duplicate_subscriptions_are_idempotent`
   - Subscribe same client/session/subscription twice.
   - Assert duplicate observation and no duplicate fanout.

4. `unsubscribed_inputs_do_not_reach_session_worker`
   - Send terminal input, resize, snapshot request, send-file, and focus for an unsubscribed session.
   - Assert no `SessionIoRequest` is emitted and typed dropped observations are present.

5. `current_subscribers_only_receive_terminal_output_and_process_exits`
   - Subscribe two clients, unsubscribe one, then feed terminal output and process exit.
   - Assert only the current subscriber receives egress.

6. `pong_preserves_request_id_for_requesting_client`
   - Send `Ping { request_id }` from a client.
   - Assert `Pong { request_id }` is emitted for that client only.

7. `backpressure_includes_typed_route_context`
   - Report backpressure for a subscribed route.
   - Assert `BackpressureSummary.route` includes `client_id`, `session_id`, and `subscription_id`.

8. `attach_state_fans_out_to_current_subscribers`
   - Subscribe two clients, emit attach state, and assert both current subscribers receive it with their own subscription ids.
   - Unsubscribe one client and assert later attach state reaches only the current subscriber.

9. `snapshot_initial_snapshot_and_scrollback_are_not_broadcast`
   - Feed `SessionIoEvent::SnapshotReady` and any exposed scrollback/initial-snapshot path through the multiplexer with multiple subscribed clients.
   - Assert no broadcast egress is emitted and a typed unsupported/non-scope observation is produced if the implementation exposes such observation.
   - This test guards the pull-based hydration convention and the lack of requesting-client identity in current contracts.

10. `engine_contract_excludes_concrete_transport_types`
   - Source/type guard that the new engine module does not mention WebRTC, browser, TUI, Unix socket, ActionCable, Rails, permissions, reconnect policy, or persistence.

Existing tests expected to remain green:

- `crates/botster-core/tests/client_stream_contract_test.rs`
- `crates/botster-core/tests/session_worker_engine_test.rs`
- `crates/botster-core/tests/actor_contract_test.rs`
- full workspace `cargo test`

Runtime/user path proof:

- This ticket is core-engine work, not host wiring.
- The production-facing path changed when downstream host crates can import and call the exported multiplexer instead of each concrete transport reimplementing subscription fanout and input routing.
- Tests must instantiate the exported multiplexer and drive real `TransportIngress` and `SessionIoEvent` values through it. Evidence that code or enum variants exist is not enough.

## Vault Gaps Worth Capturing

No durable vault gap must be captured before implementation. Existing notes already cover:

- terminal clients sharing one SessionIo/ClientWorker data-plane path
- client workers owning transport-neutral stream state
- session/client actors owning byte-bearing data-plane behavior outside hub policy
- backpressure using typed route context
- plan steps needing repo-visible plan artifacts

Capture later only if implementation settles a durable new rule for multi-client subscription replacement semantics, client removal/detach cleanup, or the public multiplexer API shape that should guide future Botster core work.
