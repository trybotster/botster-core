# Contract Test Fixtures From Regression Shapes

Ticket: `ticket_1780014900_202484`

## Context Loaded

- Pipeline context: run `run_1780074179_815523`, step `botster_plan`, target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, worktree `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1780014900_202484`.
- Orchestrator correction received after Plan Review: treat the run as main-rooted. The pipeline's dependency-populated `base_run_id`/`base_ticket_id` are stale and have been cleared in Project Pipelines; do not create a stacked PR. Implementation target remains `/Users/jasonconigliari/Projects/botster-core`; `/Users/jasonconigliari/Rails/trybotster` is reference evidence only.
- Ticket dependencies are closed: actor contract types, session I/O mailbox semantics, transport-neutral client stream contract, and entity frame semantics.
- Required vault notes loaded: `planner-playbook`, `botster-planner-playbook`, `botster-architecture`, `cli-patterns`, `spa-patterns`, Project Pipelines orchestration/UI notes, explicit target/worktree orchestration notes, and identity/goals context.
- Plan revision context loaded after Plan Review returned changes: `synced state types are allowed while pushed event variants are forbidden` and `botster pipeline reviewers must bypass rtk summaries for cargo gate evidence`.
- Repo context loaded: `README.md`, `Cargo.toml`, `src/session_protocol.rs`, `src/actor.rs`, `src/entity.rs`, `src/transport.rs`, `src/lib.rs`, `tests/session_protocol_test.rs`, and `tests/actor_contract_test.rs`.
- Reference-only old evidence inspected in `/Users/jasonconigliari/Rails/trybotster`, including noisy PTY replay, reconnect generation, queue backpressure, unknown-peer burst, initial snapshot ordering, scoped entity hydration, and plugin-worker timeout/backpressure tests.

## Scope

Create small reusable contract-test fixtures inside `botster-core` that encode the current regression shapes as data builders and focused tests. The fixtures should be useful across future core domains without importing old runtime implementation details.

Primary implementation shape:

- Add a test fixture module, likely `tests/fixtures/regression_shapes.rs` plus `tests/fixtures/mod.rs`, or an equivalent `tests/regression_shape_fixtures_test.rs` if Rust integration-test module ergonomics are simpler.
- Keep fixture builders plain Rust functions returning existing public `botster_core` contract types and byte payloads.
- Add focused tests that prove each fixture is runnable and matches its documented behavior classification.
- Update `README.md` or a short test-module doc comment only if the fixture location would otherwise be hard to discover.

Required fixture behaviors and verdicts:

| Regression shape | Verdict | Contract fixture intent |
| --- | --- | --- |
| Noisy PTY replay | Translate | Preserve noisy terminal bytes as opaque ordered `FRAME_PTY_OUTPUT` payload sequences or `TransportEgress::TerminalOutput`; drop Ghostty/parser fidelity assertions from core. This should follow `synced state types are allowed while pushed event variants are forbidden`: fixtures may use synced-state structs, but must not introduce pushed terminal-mode event variants. |
| Stale reconnect generations | Translate | Translate stale/current reconnect generations into existing core identity fields: old vs current `SubscriptionId` and/or `RequestId`, plus `ClientConnectionHealth::Reconnecting` and `HubControlMessage::AttachClient.subscription_id`. Core should prove typed correlation and subscription identity are representable today, not add generation/epoch fields or assert runtime stale-drop policy. |
| Bounded queue saturation | Preserve | Use `BoundedQueueConfig`, `QueueSource`, and `BackpressureSummary` fixtures to prove bounded capacities and typed pressure context. |
| Unknown-peer bursts | Translate | Represent unknown-peer burst pressure as transport-adapter or boundary/backpressure contract data; drop rate-limit/coalescing algorithm from core. |
| Snapshot-before-live-output | Preserve/translate | Preserve snapshot and live output frame ordering as reusable ordered fixture data; translate barrier activation policy to existing `TerminalAttachState`/snapshot contracts instead of runtime worker behavior. This is another `synced state types are allowed while pushed event variants are forbidden` guard: no pushed terminal-mode event variant should be added while modeling snapshot state. |
| Entity scoped hydration | Translate | Use `EntityFrame` snapshot/upsert/patch/remove fixtures that encode plugin-scoped ids/records; do not add client store scoping policy unless the core entity contract already has scoped frames. |
| Plugin-worker timeout/backpressure | Preserve/translate | Preserve typed plugin worker handler refs, queue capacity, backpressure events, and failure/timeout-shaped payloads where existing public types support them; drop Lua VM execution and blocking timeout mechanics. |

## Non-Scope

- No copied tests from `/Users/jasonconigliari/Rails/trybotster`.
- No Ghostty/vt100 parser, PTY process, WebRTC registry, Tokio mailbox, browser store, plugin Lua VM, or Project Pipelines product logic in `botster-core`.
- No new runtime policy, retry/backoff/coalescing algorithms, or direct snapshot helper APIs such as `snapshot_and_subscribe`.
- No new dependencies unless implementation proves a standard library or existing dependency cannot express the fixture data.
- No broad refactor of current contract types.

## Assumptions And Unknowns

- Assumption: "fixtures" means reusable Rust test helpers plus runnable tests, not production fixture APIs exported by the crate.
- Assumption: fixtures may use public `botster_core` types directly from integration tests; if helper sharing across integration tests is awkward, a single integration-test file with local fixture functions is acceptable.
- Assumption: a shape can be represented as translated fixture data even when the old runtime behavior belongs outside core, as long as the test states the dropped policy explicitly.
- Assumption: stale reconnect generations are expressible without a new core generation field by pairing stale/current `SubscriptionId` or `RequestId` values with existing reconnect and attach control types.
- Assumption: this run targets `main`, not a stacked dependency branch, per orchestrator correction. Implementation and PR setup should ignore stale `base_run_id`/`base_ticket_id` values if they appear in older run events.
- Unknown: whether entity scoped hydration needs a new core `EntityFrame` variant later. This ticket should not add it unless current docs/tests prove a stable cross-client contract shape.

## Affected Surfaces And Files

Expected:

- `tests/fixtures/regression_shapes.rs` or `tests/regression_shape_fixtures_test.rs`
- `tests/fixtures/mod.rs` if using a shared integration-test fixture module
- `README.md` only if discoverability needs a short "contract fixtures" note

Possibly touched if existing contracts cannot express a fixture without stringly JSON:

- `src/entity.rs`
- `src/transport.rs`
- `src/session_protocol.rs`

Reference evidence only:

- `/Users/jasonconigliari/Rails/trybotster/cli/src/ghostty_vt.rs`
- `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/client.rs`
- `/Users/jasonconigliari/Rails/trybotster/cli/src/hub/events.rs`
- `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/session_io_runtime.rs`
- `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/webrtc.rs`
- `/Users/jasonconigliari/Rails/trybotster/app/frontend/test/entity-stores.test.js`
- `/Users/jasonconigliari/Rails/trybotster/app/frontend/test/webrtc-pty-transport.test.js`
- `/Users/jasonconigliari/Rails/trybotster/cli/src/lua/primitives/plugin_worker.rs`

## Risks

- Over-preserving old implementation policy would violate the `botster-core` boundary. Mitigation: every fixture must include a preserve/translate/drop statement and only assert core-owned contracts.
- Making fixtures too generic could create dead abstractions. Mitigation: keep helper builders named after the seven ticket shapes and use them immediately in tests.
- Entity scoped hydration may tempt adding client-store behavior to core. Mitigation: encode only current `EntityFrame` contract data unless a stable core scoped-frame type already exists.
- Stale reconnect generation may tempt adding a generation/epoch field to core. Mitigation: do not add one for this ticket; use stale/current `SubscriptionId` or `RequestId` fixture values with existing reconnect/attach contracts.
- Snapshot ordering can drift into worker scheduling policy. Mitigation: assert ordered contract data and attach-state vocabulary, not async delivery implementation.
- Adding public APIs for tests could expand crate surface. Mitigation: keep fixtures under `tests/` unless a production contract type is needed.

## Acceptance Checks And Tests

- `cargo fmt`
- `rtk proxy -- cargo test`
- `rtk proxy -- cargo clippy --all-targets --all-features -- -D warnings`
- Targeted checks during implementation:
  - `cargo test regression_shape`
  - `cargo test actor_contract`
  - `cargo test session_protocol`
- Review checks:
  - Each fixture is runnable and used by at least one test.
  - Fixture builders are parameterized, return only public `botster_core` types or byte payloads, and have no coupling to test-private runtime state.
  - At least one builder is exercised by two or more distinct contract assertions/domains, such as session-protocol frame ordering and transport/actor message identity.
  - Each requested shape is represented exactly once in a small documented fixture.
  - Each shape states `preserve`, `translate`, or `drop`; there is no defer category.
  - Fixture tests assert public `botster-core` contracts, not old trybotster internals.
  - Production entry-point proof is intentionally scaffold-only: future runtime crates will consume these fixture shapes through public `botster_core` types, while this ticket changes only the test contract surface.

## Vault Gaps Worth Capturing

- Capture if implementation discovers a reusable pattern for "regression shape fixtures" in extracted core crates.
- Capture if scoped entity hydration cannot be represented by current `EntityFrame` without ambiguous JSON conventions.
- Capture if reconnect generation proves to be a stable core contract that is under-documented outside actor tests.
