# Add Routed Envelope Primitive For Multiplexer Coordination

## Context Loaded

- Pipeline context: ticket `ticket_1780628392_974373`, run `run_1780628533_475398`, current step `botster_plan`, gate `botster_plan_gate`.
- No prior artifacts, reviews, findings, blocking dependencies, questions, or answers were present when planning started.
- Required playbooks loaded:
  - [[planner-playbook]]
  - [[botster-planner-playbook]]
- Additional vault constraints loaded:
  - [[identity]]
  - [[goals]]
  - [[botster-architecture]]
  - [[cli-patterns]]
  - [[spa-patterns]]
  - [[project pipeline orchestration belongs in a device-level botster plugin]]
  - [[project pipelines needs an operator workbench not more primitives]]
  - [[project pipelines ui contract belongs in the plugin readme]]
  - [[botster orchestration should spawn agents with explicit target ids]]
  - [[botster orchestration prompts must bind agents to explicit worktrees]]
  - [[plan agents must author vault context as wikilinks not home paths]]
- Repo context loaded:
  - `README.md`: `botster-core` owns reusable local execution mechanics, typed contracts, and policy-free engine facades; hub/products own auth, persistence policy, adapters, UI, and workflow behavior.
  - `crates/botster-core/src/contract/mod.rs` and `src/lib.rs`: public contract/export surface already exposes client, session, transport, notification, actor, subscription multiplexer, and engine primitives.
  - `crates/botster-core/src/contract/session.rs`: current stable ids include `SessionId`, `SubscriptionId`, and `RequestId`; there is no generic `EndpointId`, `EnvelopeId`, cursor, or topic/session/plugin/client route contract.
  - `crates/botster-core/src/contract/transport.rs`: current transport frames cover terminal/session ingress and egress plus opaque boundary payloads, not a generic routed envelope.
  - `crates/botster-core/src/contract/notification.rs` and `tests/notification_inbox_test.rs`: notification inbox already has ids, target scoping, content metadata, delivery status, expiry, drain, drop, and acknowledge behavior, but it is notification-specific.
  - `crates/botster-core/src/engine/subscription_multiplexer.rs` and `tests/subscription_multiplexer_engine_test.rs`: subscription fanout, slow route observations, queue failure handling, and session/client backpressure already exist for terminal session streams.
  - `crates/botster-core/src/engine/multiplexer.rs`: assembled `MultiplexerEngine` already wires subscription routing and notifications into the host-facing core engine path.
  - `crates/botster-core/src/engine/managed_session_runtime.rs` and `src/engine/botster.rs`: production facade methods route client ingress through `ManagedSessionRuntime` and `MultiplexerEngine`; runtime-path proof should use these facades when possible.
  - `crates/botster-core-test-support/src/conformance/mod.rs`: downstream conformance helpers already wrap public APIs and can be extended for envelope conformance.
  - `docs/architecture/first-party-host-profile-primitives.md`: current audit explicitly treats notifications, entity frames, actor backpressure, and subscription routing as reusable host-profile primitives while keeping presentation, routing policy, acknowledgement policy, filtering, and workflow meaning host-owned.
- Project Pipelines checklist: run checklist `checklist_1780628579_483243` was created after an initial plugin-worker timeout; evidence should be kept both in checklist items and this plan/gate artifact.

## Scope

Add or harden a transport-neutral routed-envelope primitive in `botster-core` that composes the existing session subscription and notification mechanics without creating a messaging product API.

In scope:

- Add stable contract types for generic routed envelopes:
  - endpoint identity for core-recognized address families such as client, session, subscription, plugin, stream, and topic;
  - envelope identity;
  - route/target metadata;
  - payload metadata with typed core fields and optional owner-classified `BoundaryJson` only for extension-owned payload schemas;
  - cursor/order metadata;
  - delivery and acknowledgement state where implemented;
  - bounded queue/backpressure observations.
- Keep the payload policy-free. Core can classify routes, ordering, delivery state, cursor position, and queue pressure; it must not know product meanings such as questions, chat, Project Pipelines gates, or `post_message`.
- Reuse existing primitives where they fit:
  - `ClientId`, `SessionId`, `SubscriptionId`, `RequestId`, `PluginKey`;
  - `BackpressureRoute`, `BackpressureSummary`, `QueueSource`, `MailboxSendFailure`;
  - `SubscriptionMultiplexer` for session stream fanout;
  - `NotificationInbox` delivery status/drain mechanics if it can be generalized without breaking existing notification API;
  - `MultiplexerEngine` and `ManagedSessionRuntime` for host-facing runtime proof.
- Add a small pure engine/harness for routed envelopes only if no existing engine can represent generic envelope routing.
- Wire the primitive through at least one real core entry point, preferably `MultiplexerEngine`, so the changed behavior is reachable through the public facade. If implementation intentionally stops at contract/conformance fixtures, it must document why the ticket is scaffold-only.
- Add public docs explaining the primitive as a multiplexer routed-envelope contract, explicitly not a built-in messaging product.
- Add conformance helpers or fixtures in `botster-core-test-support` that a hub/plugin can use to prove native tools and plugin tools route through the same primitive.

Non-scope:

- No Project Pipelines product terms, workflow questions, chat semantics, `post_message`/`receive_messages` product API, notification badge policy, or operator workbench behavior in core.
- No hub auth, package admission policy, client authorization, persistence, retention jobs, or durable database schema.
- No concrete transport wiring for WebRTC, ActionCable, Rails, browser, TUI, Unix sockets, cloud, or daemon protocol changes unless a compiler-visible public contract already requires a neutral field addition.
- No broad rewrite of `TransportIngress`, `TransportEgress`, `SubscriptionMultiplexer`, or `NotificationInbox` unless a targeted change is necessary to expose the generic primitive.
- No new dependency expected; prefer existing `serde`, std collections, and current core types.
- No PII in tests, docs, fixtures, or example envelope payloads.

Botster layers touched:

- Rust `botster-core` contract and engine layers.
- Rust `botster-core-test-support` conformance layer.
- Public docs under `docs/architecture/` or `README.md`.

Worktree/target assumption:

- Agents work in the assigned pipeline worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts:

- This file is the Plan artifact.
- Gate evidence should cite this plan and the checklist evidence for loaded vault notes, convention review, verification commands, and capture decision.

## Assumptions And Unknowns

Assumptions:

- The ticket is asking for a generic multiplexer envelope layer, not a notification-only extension and not a Project Pipelines API.
- Existing notification and subscription primitives are partial building blocks. The implementation should harden/generalize them rather than create a second unrelated message queue.
- `EndpointId` and `EnvelopeId` should be distinct newtypes unless existing ids exactly express the same meaning.
- Route targeting should be explicit and typed. A generic route enum is preferable to raw strings for core-owned target families.
- Ordering/cursor support can be deterministic in-memory metadata, such as monotonic sequence/cursor newtypes. Durable persistence and replay policy stay host-owned.
- Bounded queue behavior can be represented by pure in-memory capacity checks or by existing typed pressure/failure observations. Core should report pressure; hosts decide retry/drop/dead-letter policy.
- Delivery/ack state should be implemented only to the extent core can define policy-free transitions such as queued, delivered, acknowledged, expired, dropped, backpressured, or failed.
- A conformance helper can use synthetic payloads and opaque extension data as long as stable route/delivery fields are typed.

Unknowns for implementation:

- Whether `NotificationInbox` should be refactored to wrap a generic `RoutedEnvelopeInbox`, or whether the new primitive should live beside it and notification remains a specialization. Prefer the smaller change that keeps current public notification tests stable.
- Whether stream/topic/session/plugin/client targeting should be one enum or separate route structs. Prefer one typed route enum unless ergonomics or serialization clarity says otherwise.
- Whether `MultiplexerEngine` should expose `post_envelope`, `drain_envelopes`, and `acknowledge_envelope`, or whether a nested `RoutedEnvelopeInbox` accessor is cleaner. Prefer explicit methods if they prove the public runtime path.
- Whether the existing `TransportIngress::BoundaryPayload` route id is enough for adapter-owned opaque routing. It likely is not enough for the ticket because it lacks endpoint ids, delivery state, cursors, subscription fanout, and queue behavior.
- Whether slow consumer isolation should be proven in the generic envelope engine with per-endpoint queues or by composing existing `SubscriptionMultiplexer` backpressure behavior. The implementer should choose based on the smallest contract that satisfies tests.

No human question is required before implementation. The ticket has a broad but coherent meaning: add a generic core multiplexer envelope primitive while keeping product semantics above core.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/contract/routed_envelope.rs`
  - New stable contract types for endpoint ids, envelope ids, route/target metadata, cursors/order, delivery/ack state, payload metadata, and bounded queue settings/results.
- `crates/botster-core/src/contract/mod.rs`
  - Export the new contract module and public types.
- `crates/botster-core/src/lib.rs`
  - Re-export host-facing routed-envelope types.
- `crates/botster-core/src/engine/routed_envelope.rs`
  - Pure policy-free envelope routing/inbox/fanout engine if the contract needs behavior beyond structs.
- `crates/botster-core/src/engine/mod.rs`
  - Export the new engine module and types.
- `crates/botster-core/src/engine/multiplexer.rs`
  - Wire generic routed-envelope operations into the assembled public core engine if implementation adds behavior methods.
- `crates/botster-core/tests/routed_envelope_test.rs` or `routed_envelope_engine_test.rs`
  - Contract and behavior tests for routing, fanout, cursoring, delivery/ack, queue bounds, and slow consumer isolation.
- `crates/botster-core-test-support/src/conformance/mod.rs` or a focused `routed_envelope` conformance module.
  - Hub-facing helper/example showing a host profile can build coordination tools on top without core knowing workflow semantics.
- `docs/architecture/routed-envelope-primitive.md` or `README.md`
  - Public docs explaining the primitive as a multiplexer coordination contract, not a messaging product.

Possible but avoid unless needed:

- `crates/botster-core/src/contract/notification.rs`
  - Only refactor if notification can reuse generic envelope internals without changing notification's public semantics.
- `crates/botster-core/src/engine/subscription_multiplexer.rs`
  - Only touch if generic envelope fanout can reuse its route/pressure observations with a tiny adapter.
- `crates/botster-core/src/contract/transport.rs`
  - Only add generic envelope transport frames if required by a host-facing entry point; do not overload `BoundaryPayload` with stable core controls.
- `docs/architecture/first-party-host-profile-primitives.md`
  - Update only if the new primitive changes the host-profile primitive audit.

Not expected:

- `Cargo.toml` dependency changes.
- Hub, Rails, WebRTC, browser, TUI, Lua plugin, MCP, or Project Pipelines plugin files.
- Concrete persistence or database files.

## Implementation Shape

Suggested minimal contract:

- `EndpointId(String)` and `EnvelopeId(String)` newtypes.
- `EnvelopeCursor(u64)` or a similarly deterministic monotonic cursor.
- `EnvelopeTarget` enum with typed variants such as `Client(ClientId)`, `Session(SessionId)`, `Subscription { session_id, subscription_id }`, `Plugin(PluginKey)`, `Stream(String)`, and `Topic(String)`.
- `EnvelopePayload` or `RoutedEnvelopePayload` with typed metadata fields and `extension: Option<BoundaryJson>` for owner-classified schemas.
- `RoutedEnvelope` containing id, source endpoint, targets, payload metadata, cursor/order fields, created timestamp or logical sequence, and delivery metadata.
- `EnvelopeDeliveryStatus` and `EnvelopeDeliveryState` covering queued, delivered, acknowledged, expired/dropped/failed/backpressured only where behavior is implemented.
- `RoutedEnvelopeQueueConfig` reusing or mirroring `BoundedQueueConfig`.
- `RoutedEnvelopeObservation` for fanout, cursor advancement, delivery failure, acknowledgement, queue pressure, and slow endpoint isolation.

Suggested minimal behavior:

- A pure in-memory `RoutedEnvelopeRouter` or `RoutedEnvelopeInbox` with explicit capacity per endpoint/target.
- `publish(envelope) -> outcome` that assigns or validates ordering metadata, enqueues per target, and returns typed observations.
- `drain(target, cursor, limit) -> outcome` that returns deliverable envelopes in deterministic order and advances/returns cursor metadata.
- `acknowledge(target, envelope_id) -> outcome` that records acknowledgement state if the envelope was delivered to that target.
- `subscribe(topic_or_stream, endpoint) -> outcome` and `unsubscribe(...) -> outcome` only if topic/session fanout cannot be represented by direct targets.
- Slow consumer isolation: one target at capacity should produce pressure/failure for that target without blocking enqueue/drain for other targets.

Production-path proof:

- Prefer adding `MultiplexerEngine` methods such as `publish_envelope`, `drain_envelopes`, and `acknowledge_envelope` that delegate to the pure primitive. That makes the behavior reachable through the assembled host-facing core engine.
- If the implementer decides only public contract/conformance scaffolding is appropriate, the implementation artifact must explicitly say so and tests must prove public exported types plus conformance helpers. Do not imply hub runtime behavior changed in that case.

## Risks

- Creating a product-shaped message API would violate the core boundary. Names and docs should say envelope, endpoint, route, cursor, delivery, and queue, not chat, question, inbox message, Project Pipelines, or workflow.
- A parallel notification queue could split semantics from the existing `NotificationInbox`. Either reuse it as a specialization or keep the relationship documented and tested.
- A route enum that falls back to raw strings for core-owned targets would undercut endpoint identity. Use typed ids for client/session/subscription/plugin families.
- Overusing `BoundaryJson` would hide core-owned delivery and routing controls. Stable fields requested by the ticket should be typed.
- Queue/backpressure behavior can accidentally become retry or retention policy. Core should report bounded queue pressure and delivery state; hosts decide what to do next.
- Cursoring can become durable replay policy if overbuilt. Keep it deterministic and in-memory unless the ticket explicitly grows persistence.
- Fanout tests can pass by inspecting stored state only. Acceptance needs to drive the public engine path or document scaffold-only scope.
- Refactoring notification/subscription internals could break existing public tests. Keep compatibility unless replacing the old path is necessary and intentional.
- Adding docs with local vault paths would leak runtime-local context. Use note titles/wiki links only.

## Acceptance Checks / Tests

Run after implementation:

- `cargo fmt --all -- --check`
- `cargo test -p botster-core routed_envelope`
- `cargo test -p botster-core notification_inbox`
- `cargo test -p botster-core subscription_multiplexer`
- `cargo test -p botster-core-test-support routed_envelope`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Add targeted tests:

1. `routes_envelope_to_explicit_client_endpoint`
   - Publish a synthetic envelope to one client endpoint and drain that endpoint.
   - Assert envelope id, target, payload metadata, and cursor/order fields survive.

2. `routes_envelope_to_session_and_subscription_targets`
   - Publish to typed session/subscription targets.
   - Assert only matching targets drain the envelope.

3. `topic_or_stream_subscription_fans_out_to_current_subscribers`
   - Subscribe two endpoints, publish once, and assert both receive one envelope with independent delivery state.
   - Unsubscribe one and assert later publish reaches only the current subscriber.

4. `delivery_ack_state_is_per_target`
   - Deliver the same envelope to two targets.
   - Acknowledge one target and assert the other target remains delivered/unacknowledged.

5. `cursoring_resumes_after_last_seen_envelope`
   - Publish ordered envelopes, drain with a cursor/limit, then resume from the returned cursor.
   - Assert no duplicate or skipped envelopes.

6. `bounded_queue_reports_pressure_without_blocking_other_targets`
   - Fill one endpoint queue to capacity.
   - Publish to that endpoint and another endpoint.
   - Assert pressure/failure is reported only for the full endpoint and the other endpoint still receives the envelope.

7. `slow_consumer_isolation_preserves_fast_consumer_delivery`
   - Keep one target undrained while another drains promptly.
   - Assert fast target receives later envelopes even while slow target has pressure observations.

8. `notification_or_subscription_primitives_do_not_regress`
   - Keep existing notification inbox and subscription multiplexer tests green.
   - If refactored, add explicit tests proving old public semantics still hold.

9. `boundary_json_is_limited_to_extension_payloads`
   - Assert route, endpoint, delivery, cursor, queue, and ack fields are typed.
   - Any `BoundaryJson` field must be classified as extension/plugin/adapter-owned payload schema.

10. `hub_facing_conformance_helper_builds_semantic_tool_payload_above_core`
    - In test support, create a synthetic host/plugin coordination payload using the generic envelope primitive.
    - Assert core routes and tracks delivery without inspecting product semantics.

Docs acceptance:

- Public docs must state that this is a multiplexer routed-envelope primitive.
- Public docs must explicitly say it is not a built-in messaging product and does not encode chat, Project Pipelines, workflow questions, notifications UI, auth, persistence, or concrete transport policy.
- Examples must use synthetic ids and no PII.

## Vault Gaps Worth Capturing

No vault capture is required before implementation. Existing notes already cover the relevant boundaries: core versus product policy, Botster as a multiplexer, session/client data-plane ownership, Project Pipelines as plugin-owned workflow policy, `BoundaryJson` classification, and plan artifact path neutrality.

Capture later if implementation discovers a durable rule not already present, especially:

- the naming/relationship between `NotificationInbox` and a generic routed-envelope primitive;
- a reusable rule for cursor/order metadata in policy-free core queues;
- a concrete gotcha around slow consumer isolation in generic envelope fanout.
