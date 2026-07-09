# Define Ergonomic Embeddable BotsterEngine API

Ticket: `ticket_1780189402_297736`
Run: `run_1780189445_706818`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Define ergonomic embeddable BotsterEngine API`, run `run_1780189445_706818`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts, reviews, findings, questions, or answers.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster overlay notes loaded:
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
- General vault context loaded:
  - `identity`
  - `goals`
- Checklist discipline:
  - Run-level vault checklist `checklist_1780189506_395630` created after an initial plugin worker timeout; checklist item 1 records loaded vault notes.
- Repo context inspected:
  - `README.md`
  - `Cargo.toml`
  - `crates/botster-core/Cargo.toml`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/runtime/mod.rs`
  - `crates/botster-core/src/engine/mod.rs`
  - `crates/botster-core/src/engine/multiplexer.rs`
  - `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - `crates/botster-core-test-support/src/fake/mod.rs`
  - `crates/botster-core-test-support/src/fake/session_worker.rs`
  - `crates/botster-core-test-support/src/fake/plugin_worker.rs`
  - `crates/botster-core-dev/src/lib.rs`
  - `crates/botster-core-dev/tests/engine_smoke_test.rs`
- Prior plan artifacts inspected:
  - `docs/archive/plans/assemble-core-multiplexer-engine-api.md`
  - `docs/archive/plans/core-session-worker-engine.md`
  - `docs/archive/plans/core-subscription-multiplexer-routing-engine.md`

## Scope

Add a small ergonomic public API on top of the existing `MultiplexerEngine` so a Rust consumer can embed `botster-core` as a tmux-like library without constructing transport ingress frames by hand for ordinary operations.

In scope:

- Add a public `BotsterEngine` facade in `botster-core` that composes the existing `MultiplexerEngine` rather than replacing it.
- Provide consumer-facing methods for the ticket workflow:
  - create an engine from host adapters
  - spawn a session from explicit host-resolved spawn inputs
  - attach and detach clients
  - write PTY bytes
  - resize a session
  - receive or drain client output frames
  - post and drain notifications
  - register/invoke plugin handlers
  - classify session activity
  - shut down a session
- Add lightweight request/result types only where they remove real caller friction over current low-level `TransportIngress` and broad outcome plumbing.
- Keep `MultiplexerEngine` available as the lower-level assembled core primitive; `BotsterEngine` should delegate to it.
- Add fake/default adapter support only where needed to let consumer-style tests build a complete lifecycle without product hub wiring.
- Add one integration test that imports only public `botster_core` and `botster_core_test_support` APIs and drives the full lifecycle from spawn through shutdown.
- Update `botster-core-dev` smoke harness to use `BotsterEngine` if that is a small change; this gives an executable dev path through the new public API.
- Update README docs to separate the embeddable core API from the Botster hub/product API and keep host responsibilities explicit.

Non-scope:

- No concrete local PTY implementation, Tokio worker loop, hub daemon integration, socket server, WebRTC, TUI, React SPA, Rails relay, MCP, Project Pipelines product behavior, or plugin marketplace behavior.
- No replacement of the lower-level `MultiplexerEngine`, `SessionWorkerEngine`, `SubscriptionMultiplexer`, `NotificationInbox`, or `PluginWorkerEngine`.
- No broad refactor of existing contracts or test support beyond changes needed for the ergonomic facade.
- No new dependency unless the compiler proves the current crate surface cannot express the API.
- No automatic executable discovery, cwd admission, environment inheritance, auth, persistence, reconnect, retention, or notification presentation policy in core.
- No PII-bearing examples, metadata, or test fixtures.

Botster layers touched:

- Rust `botster-core` engine layer: primary surface.
- Rust `botster-core-test-support`: fake adapters/helpers only as needed for consumer-style tests.
- Rust `botster-core-dev`: optional smoke harness update if it stays small.
- Docs: README core/hub API boundary and this plan.

Worktree/target assumption: implementers must work in this assigned botster-core ticket worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts: this file is the repo-visible Plan artifact. Gate evidence should cite this file plus vault checklist evidence.

## Assumptions And Unknowns

Assumptions:

- The prior `MultiplexerEngine` work is the foundation. This ticket should improve consumer ergonomics, not rebuild the multiplexer.
- `BotsterEngine` is acceptable as the public ergonomic type name because the ticket names it directly.
- The production entry point changed by this ticket is the exported `botster_core::BotsterEngine` API. There is no hub product wiring in this repo, so runtime proof comes from public integration tests and, if feasible, the dev smoke harness.
- Core can own method-level orchestration over reusable primitives. Hosts still own concrete local execution, adapter threads, auth, persistence, and product policy.
- Spawn input must remain explicit. Core must not infer executable, working directory, environment, target admission, or user identity.
- Receiving output can be modeled as returned/drained typed client frames from the facade; it should not imply a concrete transport.
- Fake/default adapters may exist in `botster-core-test-support` for tests, but production core must not depend on test support.
- Existing `BoundaryJson` remains limited to plugin-owned payloads and other documented escape hatches.
- No human question is blocking this plan; the ticket can be satisfied without waiving scope.

Unknowns for implementation:

- Exact module path is open. Prefer `crates/botster-core/src/engine/botster.rs` or `botster_engine.rs` if that is clearer than extending `multiplexer.rs`.
- Whether `BotsterEngine` should be generic over runtime adapter types like `MultiplexerEngine<R, W>`, or wrap boxed trait objects for a simpler consumer type. Prefer the smaller shape that keeps tests readable and avoids allocation unless ergonomics clearly suffer.
- Whether output frames should be returned from each operation only, or accumulated in per-client buffers with `drain_client_output(client_id)`. Prefer operation return values unless the consumer-style test shows attach/write/output observation is awkward.
- Whether spawn should accept `SessionSpawnRequest` directly or a narrower `BotsterSessionSpec` builder. Prefer direct `SessionSpawnRequest` plus convenience helpers so policy stays host-resolved.
- Whether plugin registration should pass through existing `PluginWorkerRegistration` or gain a convenience wrapper. Prefer pass-through first; the ergonomic win is session/client lifecycle methods.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/botster.rs` or equivalent
  - New `BotsterEngine` facade, public result/outcome types, and method-level wrappers over `MultiplexerEngine`.
- `crates/botster-core/src/engine/mod.rs`
  - Export the new facade and any public ergonomic types.
- `crates/botster-core/src/lib.rs`
  - Re-export `BotsterEngine` from the crate root.
- `crates/botster-core/tests/botster_engine_api_test.rs`
  - Consumer-style integration test using crate-root public APIs through the full lifecycle.
- `crates/botster-core-test-support/src/fake/mod.rs`
  - Possible helper exports if the new test needs a reusable fake engine fixture.
- `crates/botster-core-dev/src/lib.rs`
  - Optional: move the smoke harness from direct primitive coordination to `BotsterEngine`.
- `crates/botster-core-dev/tests/engine_smoke_test.rs`
  - Expected to remain green if the harness is updated.
- `README.md`
  - Add a narrow section distinguishing embeddable core API from hub product API and host responsibilities.
- `docs/archive/plans/ergonomic-embeddable-botster-engine-api.md`
  - This plan artifact.

Possible but avoid unless compiler/API shape requires:

- `crates/botster-core/src/engine/multiplexer.rs`
  - Small accessors or outcome helpers only if needed by `BotsterEngine`.
- `crates/botster-core/src/runtime/mod.rs`
  - Small adapter convenience types only if they reduce caller friction without adding policy.
- `crates/botster-core-test-support/src/fake/session_worker.rs` or `plugin_worker.rs`
  - Test helper additions only.

Not expected:

- Workspace dependency changes.
- Contract enum rewrites.
- Hub, CLI product, browser, TUI, Rails, Lua plugin, MCP, or Project Pipelines files.

## Implementation Shape

Suggested minimal public API:

- `BotsterEngine::new(session_runtime)` or `BotsterEngine::with_plugin_config(session_runtime, plugin_config)`.
- `spawn_session(request, metadata, worker_runtime) -> Result<BotsterSpawnResult, BotsterEngineError>`.
- `attach_client(client_id, session_id, subscription_id, now_seconds) -> Result<BotsterEngineOutput, BotsterEngineError>`.
- `detach_client(client_id, session_id, subscription_id, now_seconds) -> Result<BotsterEngineOutput, BotsterEngineError>`.
- `write_bytes(client_id, session_id, data, now_seconds) -> Result<BotsterEngineOutput, BotsterEngineError>`.
- `resize(client_id, session_id, rows, cols, now_seconds) -> Result<BotsterEngineOutput, BotsterEngineError>`.
- `handle_runtime_event(event) -> Result<BotsterEngineOutput, BotsterEngineError>` or narrower helpers such as `receive_output(session_id, data, at)`.
- `post_notification(item) -> NotificationId`.
- `drain_notifications(target, now) -> Vec<NotificationItem>`.
- `load_plugin(registration)` and `invoke_plugin(request) -> PluginInvocationResult`.
- `classify_activity(session_id, now_seconds, active_threshold_seconds) -> Result<SessionActivityStatus, BotsterEngineError>`.
- `shutdown_session(session_id, reason, now_seconds) -> Result<BotsterEngineOutput, BotsterEngineError>`.

Suggested constraints:

- Delegate core mechanics to `MultiplexerEngine`; do not duplicate subscription, session worker, plugin worker, notification, or activity rules.
- Keep outcomes typed. `BotsterEngineOutput` can wrap or flatten `MultiplexerEngineOutcome`, but should make client frames easy to inspect.
- Keep the API synchronous and deterministic. Async supervision belongs to host crates.
- Keep method names library-like and operation-oriented. Callers should not need to know `TransportIngress::SubscribeSession` for the common attach path.
- Preserve lower-level APIs for advanced embedders.
- Examples and tests use synthetic ids such as `session-api-1`, `client-a`, and `fake-shell`.

## Risks

- The biggest risk is turning `botster-core` into a product hub. Mitigation: `BotsterEngine` delegates reusable mechanics only and leaves process supervision, admission, persistence, auth, reconnect, and presentation outside core.
- A wrapper that merely renames `MultiplexerEngine` without reducing caller friction would miss the ergonomic intent. The acceptance test should read like a consumer lifecycle, not like transport-frame construction.
- A wrapper that hides too much host policy would violate the core boundary. Spawn and runtime adapters must stay explicit.
- Returning output only as broad internal outcomes can make the API hard to embed. Tests should assert client output through the ergonomic return/drain surface.
- Duplicating multiplexer state or plugin invocation behavior can create drift. The implementation should compose existing engines.
- Adding fake/default adapters to production core can accidentally make fake behavior look like supported runtime behavior. Keep reusable fakes in test support unless a production no-op adapter is clearly documented as test/demo-only.
- Updating docs can overclaim hub integration. README should say this is the core API; the Botster hub product remains a separate host.

## Acceptance Checks / Tests

Required targeted test:

- `botster_engine_consumer_lifecycle_uses_public_api`
  - Import `BotsterEngine` and supporting public types from `botster_core`.
  - Use fake adapters from `botster_core_test_support`.
  - Create an engine.
  - Spawn a session from explicit `SessionSpawnRequest`.
  - Attach at least one client, ideally two clients.
  - Write bytes through the client-facing method and assert the fake session worker/runtime sees them.
  - Resize through the client-facing method and assert the fake worker records rows/cols.
  - Feed runtime output and assert subscribed clients receive typed output frames through the ergonomic output surface.
  - Post and drain a notification.
  - Load and invoke a plugin handler.
  - Classify activity after output.
  - Detach a client and prove it stops receiving later output.
  - Shut down the session and assert lifecycle/shutdown state is observable.
  - Assert post-shutdown late output does not refresh activity or fan out.

Additional focused tests if behavior splits:

- `botster_engine_returns_typed_error_for_unknown_session`.
- `botster_engine_keeps_spawn_policy_explicit`.
- `botster_engine_exports_are_crate_root_consumable`.

Existing tests expected to remain green:

- `crates/botster-core/tests/multiplexer_engine_api_test.rs`
- `crates/botster-core/tests/session_worker_engine_test.rs`
- `crates/botster-core/tests/subscription_multiplexer_engine_test.rs`
- `crates/botster-core/tests/notification_inbox_test.rs`
- `crates/botster-core/tests/plugin_worker_engine_test.rs`
- `crates/botster-core-dev/tests/engine_smoke_test.rs`

Verification commands:

- `cargo fmt`
- `cargo test -p botster-core botster_engine`
- `cargo test -p botster-core`
- `cargo test -p botster-core-dev`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Runtime/user path proof:

- Public integration tests must instantiate `botster_core::BotsterEngine`, not private modules.
- If `botster-core-dev` is updated, `dev_harness_exercises_public_engine_path` proves the dev executable path also uses the new facade.
- If `botster-core-dev` is not updated, implementation handoff must explicitly document why this ticket is core API scaffold-only and cite the public integration test as the production library entry-point proof.

## Vault Gaps Worth Capturing

No durable vault gap must be captured before implementation.

Potential capture after implementation:

- If `BotsterEngine` settles the public naming and lifecycle vocabulary for embedders, capture a Botster note documenting `MultiplexerEngine` as the lower-level assembled primitive and `BotsterEngine` as the ergonomic crate-root facade.
- If output delivery chooses per-client buffering instead of operation return values, capture the rationale because it will constrain future hub and downstream embedder APIs.
