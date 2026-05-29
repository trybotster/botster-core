# Define Transport-Neutral Client Stream Contract

## Context Loaded

- Pipeline context: `ticket_1780014864_624778`, `run_1780030155_616228`, current step `botster_plan`, gate `botster_plan_gate`.
- Dependency context: depends on closed `ticket_1780014863_508751` / "Define botster-core actor contract types"; current repo already contains `src/actor.rs`, `src/client.rs`, `src/session.rs`, `src/transport.rs`, and `tests/actor_contract_test.rs`.
- Worktree: `<pipeline-worktree>`.
- Target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Required playbooks loaded:
  - `<vault>/notes/planner-playbook.md`
  - `<vault>/notes/botster-planner-playbook.md`
- Additional vault constraints loaded:
  - `<vault>/self/identity.md`
  - `<vault>/self/goals.md`
  - `<vault>/notes/botster-architecture.md`
  - `<vault>/notes/cli-patterns.md`
  - `<vault>/notes/spa-patterns.md`
  - `<vault>/notes/project pipeline orchestration belongs in a device-level botster plugin.md`
  - `<vault>/notes/project pipelines needs an operator workbench not more primitives.md`
  - `<vault>/notes/project pipelines ui contract belongs in the plugin readme.md`
  - `<vault>/notes/botster orchestration should spawn agents with explicit target ids.md`
  - `<vault>/notes/botster orchestration prompts must bind agents to explicit worktrees.md`
  - `<vault>/notes/plan steps need reviewable plan artifacts.md`
- Repo context loaded:
  - `Cargo.toml`: contract crate with serde/serde_json/thiserror only; no async runtime dependency.
  - `src/lib.rs`: exports public contract modules and actor/transport types.
  - `src/client.rs`: current client identifiers/scopes/liveness state.
  - `src/session.rs`: current session, subscription, and request identifiers.
  - `src/transport.rs`: current transport-neutral ingress/egress frames.
  - `src/actor.rs`: current hub/client/session/plugin mailbox contract types, bounded queue metadata, backpressure summaries, `SessionIoRequest`, and `SessionIoEvent`.
  - `tests/actor_contract_test.rs`: current serde and boundary tests for actor contracts.
  - `docs/plans/actor-contract-types.md`: prior plan for the dependency ticket.
- Old trybotster evidence loaded as reference only:
  - `<trybotster-reference>/cli/src/worker/client.rs`
  - `<trybotster-reference>/cli/src/worker/transport.rs`
  - `<trybotster-reference>/cli/src/socket/framing.rs`
  - `<trybotster-reference>/docs/webrtc-protocol.md`

## Scope

Build a minimal synchronous in-memory client stream contract harness in `botster-core`. The harness should prove the semantics that browser, TUI, and socket adapters need to share without introducing concrete WebRTC, ratatui, socket bridge, Tokio, or hub policy.

In scope:

- Add a public transport-neutral client stream module, likely `src/client_stream.rs`.
- Track per-client session subscriptions by `SessionId` and `SubscriptionId`.
- Accept existing `TransportIngress` and `SessionIoEvent` contract values where they already express the behavior.
- Extend contract types only where the ticket requires missing semantics, especially send-file and generation-gated delivery.
- Produce deterministic outcomes from in-memory handling: transport egress frames, session I/O requests, hub/control observations, and dropped-input/backpressure observations.
- Route terminal bytes, snapshots, scrollback, attach state, focus changes, process exits, and pong frames through active subscriptions.
- Route PTY input, send-file, resize, snapshot request, health, ping/pong, unsubscribe, shutdown, and backpressure without concrete transport policy.
- Add tests that exercise the actual harness path, not only type existence.
- Export the new public types from `src/lib.rs`.

Non-scope:

- No Tokio tasks, channels, async worker loops, handles, or runtime spawning.
- No WebRTC, ratatui, Unix socket, ActionCable, Rails, or browser-specific adapter code.
- No session process protocol changes unless a test needs existing `SessionIoRequest`/`SessionIoEvent` values.
- No plugin workflow, UI, MCP, project-pipeline surface, or operator workbench changes.
- No broad refactors of `actor.rs`, `transport.rs`, or prior contract tests beyond what this ticket needs.
- No copying old trybotster implementation; old paths are evidence only.

Botster layer touched: Rust core contract crate, specifically the session/client worker data-plane contract slice.

Worktree/target assumption: this Plan step and downstream agents operate only in the assigned worktree above and target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.

Pipeline gates/artifacts: this repo-visible plan document plus Project Pipelines gate evidence are the Plan artifacts. Implementation should later attach a change summary and command evidence before review.

## Assumptions And Unknowns

Assumptions:

- `botster-core` is intentionally scaffold/contract-level; the production entry point for now is downstream runtime crates importing and executing these contracts.
- The in-memory harness is the runtime path for this ticket: tests should call the harness methods and assert routed outcomes.
- Existing dependencies are sufficient. Use `std::collections` and existing serde-derived types; do not add Tokio or other runtime crates.
- `TransportIngress` is per-client context in the harness, so existing variants without `client_id` can be interpreted as coming from the harness owner.
- Duplicate subscriptions mean same `SessionId` and same `SubscriptionId`; they should be idempotent and should not produce detach/re-attach churn.
- Changed subscription IDs mean same `SessionId` with a different `SubscriptionId`; replace the route so later egress uses the new subscription identity and old subscription routes are inactive. To make this verifiable, add `subscription_id` to terminal stream `TransportEgress` variants that are routed through a subscription: terminal output, snapshot, scrollback, process exit, attach state, and focus changed.
- "Unsubscribed input is dropped/observed" means PTY input, send-file, resize, and snapshot requests for a session without an active subscription should not produce `SessionIoRequest`; they should produce an observation suitable for tests and downstream diagnostics.
- Backpressure is contract-visible on the client side via `ClientControlFrame::Backpressure(BackpressureSummary)` or a stream observation wrapping `BackpressureRoute`; it should not emit `HubControlMessage` or implement retry/slow-client policy in core.
- Send-file is already modeled past ingress by `SessionIoRequest::SendFile { request_id, data }`, `SessionIoEvent::SendFileFailed { request_id, reason }`, and `SendFileErrorReason`. The only planned contract gap is `TransportIngress::SendFile { request_id, session_id, data }`; filename and temp-file path policy stay downstream.

Unknowns for implementation:

- The exact public naming is open. Prefer the smallest clear vocabulary, for example `ClientStreamHarness`, `ClientStreamOutcome`, `ClientStreamObservation`, and `ClientStreamGeneration`.
- Whether to place generation state in `ClientState::Reconnecting` or a new stream-specific type. Prefer a stream-specific generation wrapper if it avoids changing existing client liveness enums.
- Shutdown should emit `TransportEgress::Close` plus a `Shutdown` observation only. Do not emit `HubControlMessage::Shutdown` from the per-client stream harness.

No human question is needed before implementation; the ticket has a clear contract-scaffold meaning and does not require waiving any acceptance item.

## Affected Surfaces / Files

Expected:

- `src/client_stream.rs`
  - New pure, synchronous stream harness and outcome/observation types.
  - Subscription registry keyed by `SessionId`, with current `SubscriptionId` route.
  - Methods for client ingress, session events, health/generation state, backpressure, and shutdown.
- `src/transport.rs`
  - Add `TransportIngress::SendFile { request_id, session_id, data }`.
  - Add `subscription_id` to terminal stream `TransportEgress` variants that represent subscription-routed delivery: terminal output, snapshot, scrollback, process exit, attach state, and focus changed.
  - Keep names transport-neutral. Do not introduce browser/TUI/socket/WebRTC terms.
- `src/actor.rs`
  - No send-file changes expected; reuse existing `SessionIoRequest::SendFile`, `SessionIoEvent::SendFileFailed`, and `SendFileErrorReason`.
  - Minimal changes only if the stream harness needs a reusable typed client-side backpressure observation beyond existing `BackpressureSummary`/`BackpressureRoute`.
- `src/lib.rs`
  - Export the new stream harness and any new public contract types.
- `tests/client_stream_contract_test.rs`
  - New tests that drive the harness behavior end to end.
- `tests/actor_contract_test.rs` or `tests/session_protocol_test.rs`
  - Touch only if existing boundary/serde assertions need updated exports or new variants.
- `docs/plans/client-stream-contract.md`
  - This plan artifact.

Not expected:

- `Cargo.toml`, unless a compiler-proven gap appears. No new dependency is expected.
- `README.md`, unless implementation creates a public contract detail that needs top-level documentation.
- Old trybotster source paths.
- Any runtime crate outside this worktree.

## Implementation Shape

Suggested minimal API:

- `ClientStreamHarness::new(client_id: ClientId)`.
- `handle_ingress(TransportIngress) -> ClientStreamOutcome`.
- `handle_session_event(SessionIoEvent) -> ClientStreamOutcome`.
- `set_health(ClientConnectionHealth) -> ClientStreamOutcome` or equivalent.
- `with_generation(generation, command)` or `handle_generation(generation, ...)` to prove stale reconnect deliveries are dropped.
- `shutdown(reason) -> ClientStreamOutcome`.

Suggested outcome shape:

- `egress: Vec<TransportEgress>`
- `session_requests: Vec<(SessionId, SessionIoRequest)>`
- `control_frames: Vec<ClientControlFrame>` for client-side health/backpressure reports when an existing control frame is the clearest public contract
- `observations: Vec<ClientStreamObservation>`

Suggested observation variants:

- `Subscribed`, `DuplicateSubscription`, `ReplacedSubscription`, `Unsubscribed`
- `DroppedUnsubscribedInput`, `DroppedUnsubscribedSendFile`, `DroppedUnsubscribedResize`, `DroppedUnsubscribedSnapshot`
- `GenerationStale`
- `Backpressure(BackpressureSummary)` or an equivalent typed observation preserving `BackpressureRoute`
- `Shutdown`

The implementer may choose narrower names, but every public type should remain transport-neutral and derive `Debug`, `Clone`, `PartialEq`, `Eq` where practical.

## Risks

- Building a real worker loop would pull runtime policy into `botster-core` and violate the core contract boundary.
- A harness that only instantiates types but does not route messages would fail the "actual runtime or user path changed" requirement for this scaffold slice.
- Adding a second ingress vocabulary could make transport adapters choose between `TransportIngress` and harness commands. Prefer using existing contracts unless a separate command type is clearly smaller.
- Forgetting `SubscriptionId` in terminal egress would make changed subscription route tests impossible and would weaken socket/browser parity; the plan commits to adding it to subscription-routed egress variants.
- Backpressure can become vague if represented only as strings. Preserve typed queue/session/client/subscription context where possible.
- Generation gating can be overbuilt. The ticket only needs stale delivery rejection semantics; no reconnect policy belongs in core.
- Send-file semantics can accidentally become file-system policy. Core should carry request id plus bytes only; filename, storage, and temp-file path resolution remain downstream.

## Acceptance Checks / Tests

Run:

- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- All new public types, enum variants, fields, and methods must have doc comments consistent with the crate's `missing_docs = "warn"` lint profile. Non-test code must avoid `unwrap` so clippy's `unwrap_used` warning stays clean.

Add targeted tests in `tests/client_stream_contract_test.rs`:

1. `subscribed_clients_receive_terminal_bytes_and_process_exits`
   - Subscribe client to session.
   - Feed `SessionIoEvent::TerminalBytes` and `SessionIoEvent::ProcessExited`.
   - Assert `TransportEgress` includes terminal bytes and process exit with the active `subscription_id`.

2. `unsubscribed_input_send_file_resize_and_snapshot_are_dropped_with_observations`
   - Send input, send-file, resize, and snapshot request for an unsubscribed session.
   - Assert no `SessionIoRequest` is emitted.
   - Assert one observation per dropped operation.

3. `duplicate_subscriptions_are_idempotent`
   - Subscribe same `SessionId`/`SubscriptionId` twice.
   - Assert the second call produces a duplicate/idempotent observation and leaves one active route.
   - Assert terminal output is delivered once.

4. `changed_subscription_ids_replace_old_routes`
   - Subscribe `session-1` with `sub-old`, then subscribe same session with `sub-new`.
   - Assert replacement observation.
   - Feed terminal output/process exit and assert every subscription-routed `TransportEgress` carries `sub-new`, never `sub-old`.
   - Unsubscribe `sub-old` should not detach the new route; unsubscribe `sub-new` should detach.

5. `pong_preserves_request_id`
   - Send `TransportIngress::Ping { request_id }`.
   - Assert `TransportEgress::Pong { request_id }` equals the original request id.

6. `generation_gating_drops_stale_deliveries`
   - Move stream generation forward.
   - Attempt a stale generation-wrapped terminal delivery or ingress.
   - Assert no egress/session request and a stale-generation observation.

7. `shutdown_closes_transport_and_stops_routing`
   - Subscribe, call shutdown, assert close egress/observation.
   - Feed later terminal output/input and assert it is ignored or observed without routing.

8. `backpressure_is_observable_with_route_context`
   - Trigger/report backpressure for a subscribed session.
   - Assert `ClientControlFrame::Backpressure(BackpressureSummary)` or an equivalent typed observation preserves `ClientId`, `SessionId`, and `SubscriptionId` in `BackpressureRoute`.
   - Assert no `HubControlMessage::Backpressure` or `HubControlMessage::Shutdown` is emitted by the per-client harness.

Existing tests should continue to pass:

- `tests/actor_contract_test.rs`
- `tests/session_protocol_test.rs`
- all current `cargo test` coverage.

Runtime path proof for this ticket:

- The changed path is intentionally scaffold-level: downstream transport adapters will call the exported in-memory harness instead of each transport inventing subscription, input, ping, and backpressure semantics.
- Tests must instantiate the exported harness and drive `TransportIngress` plus `SessionIoEvent` through it. Evidence that enum variants exist is not enough.

## Vault Gaps Worth Capturing

No durable vault gap must be captured before implementation. Existing notes already cover:

- client workers owning transport-neutral stream state
- terminal clients sharing one SessionIo/ClientWorker data-plane path
- session/client actors owning byte-bearing data-plane behavior outside hub policy
- transport adapters staying concrete while core contracts remain neutral
- Project Pipelines plan artifact discipline

Capture later only if implementation discovers a stable new rule for subscription replacement semantics, generation gating vocabulary, or send-file payload representation that should guide future Botster contract work.
