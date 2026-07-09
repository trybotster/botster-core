# Assemble Core Multiplexer Engine API

Ticket: `ticket_1780075966_468839`
Run: `run_1780096018_744313`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Assemble core multiplexer engine API`, run `run_1780096018_744313`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts, reviews, findings, questions, or answers.
- Orchestrator correction received through Botster inbox: this run is main-rooted. The pipeline auto-populated stale `base_run_id` / `base_ticket_id` from a dependency, but Project Pipelines database state was corrected. Target `main`; do not create a stacked PR.
- Worktree: the pipeline-provided ticket worktree for this run.
- Target: the pipeline-provided botster-core spawn target for this run.
- Closed dependencies loaded from pipeline context:
  - `Expose versioned consumer test support for botster-core`
  - `Implement core session model and activity engine`
  - `Define core process spawning and session runtime abstraction`
  - `Build core session worker engine`
  - `Build core subscription multiplexer and client routing engine`
  - `Implement core notification and session inbox primitives`
  - `Build core plugin worker execution engine`
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
- Repo context inspected:
  - `README.md`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/engine/mod.rs`
  - `crates/botster-core/src/engine/session_worker.rs`
  - `crates/botster-core/src/engine/subscription_multiplexer.rs`
  - `crates/botster-core/src/engine/plugin_worker.rs`
  - `crates/botster-core/src/runtime/mod.rs`
  - `crates/botster-core/src/contract/session.rs`
  - `crates/botster-core/src/contract/notification.rs`
  - `crates/botster-core/Cargo.toml`
  - `crates/botster-core-test-support/src/fake/session_worker.rs`
  - Existing acceptance tests for session worker, subscription multiplexer, notification inbox, and plugin worker.
- Prior plan artifacts inspected:
  - `docs/archive/plans/core-session-model-activity-engine.md`
  - `docs/archive/plans/core-session-worker-engine.md`
  - `docs/archive/plans/core-subscription-multiplexer-routing-engine.md`
  - `docs/archive/plans/core-notification-session-inbox-primitives.md`
  - `docs/archive/plans/core-plugin-worker-execution-engine.md`

## Scope

Assemble the existing core pieces into one small host-facing multiplexer engine API that embedders can call instead of manually coordinating session runtime spawning, session worker I/O, client subscription routing, plugin invocation, notification draining, and activity/lifecycle updates.

In scope:

- Add a public engine facade in `botster-core`, likely `MultiplexerEngine`, under `crates/botster-core/src/engine/multiplexer.rs` or similarly direct naming.
- Define a small host adapter trait or input boundary that lets hosts supply:
  - session runtime spawning through existing `SessionRuntime`
  - per-session worker runtime behavior through existing `SessionWorkerRuntime`
  - plugin runtime registration through existing `PluginRuntime`
  - deterministic timestamps for activity and notification checks
- Use existing core pieces rather than duplicating them:
  - `CoreSession`, `SessionActivity`, and activity reducer functions
  - `SessionRuntime` and `SessionSpawnRequest`
  - `SessionWorkerEngine`
  - `SubscriptionMultiplexer`
  - `NotificationInbox`
  - `PluginWorkerEngine`
- Expose host-facing methods for the ticket's required workflow:
  - create or register a session from an explicit spawn request
  - attach clients and route `TransportIngress`
  - accept runtime output/events and fan out through current subscriptions
  - invoke plugin handlers through registered plugin workers
  - post and drain session/client notifications
  - observe lifecycle/activity changes through typed outcomes
  - shut down one session cleanly
- Return one typed outcome/result shape that carries engine observations, client egress, session runtime handles/errors, session I/O events, plugin invocation results, notification deliveries, and updated session state as needed.
- Add fake/test adapters in `botster-core-test-support` only where necessary to drive the full public API in an integration test.
- Add one integration test that drives the assembled public API from session spawn through output fanout, notification delivery, plugin invocation, activity classification, and clean shutdown.
- Update public exports in `crates/botster-core/src/engine/mod.rs` and `crates/botster-core/src/lib.rs`.
- Update `README.md` to document host responsibilities versus core responsibilities for the new API.

Non-scope:

- No concrete Botster hub, CLI startup, product config, auth, persistence, cloud federation, Rails relay, WebRTC, TUI, React UI, Lua VM, MCP workflow policy, Project Pipelines product behavior, or old trybotster source import.
- No new runtime dependency such as Tokio for `botster-core`.
- No broad rewrites of existing session worker, subscription multiplexer, notification inbox, plugin worker, activity, package, identity, or UI contracts.
- No compatibility branch, version-suffixed duplicate API, or speculative optional configurability.
- No product-specific session taxonomy or PII-bearing session metadata.
- No direct host policy for retention, reconnect, permissions, plugin installation, executable discovery, environment inheritance, or notification presentation.

Botster layers touched:

- Rust `botster-core` engine layer: primary surface.
- Rust `botster-core` runtime contract layer: only if a tiny adapter trait is needed to connect existing traits.
- Rust `botster-core-test-support`: fake adapters for the integration path if tests need them.
- Docs: README ownership/API boundary plus this plan.

Worktree/target assumption: implementers must work in the assigned botster-core ticket worktree for this run and target `main`.

Pipeline gates/artifacts: this file is the repo-visible Plan artifact. Gate evidence should cite it plus checklist evidence.

## Assumptions And Unknowns

Assumptions:

- This ticket is the assembly layer over closed dependency tickets. The correct change is a small public facade, not another independent low-level engine.
- Existing modules already satisfy most primitive behavior. The new API should compose them and expose a coherent entry point for embedders.
- The production entry point changed by this ticket is the exported `botster_core` API that future `botster-hub` or third-party hosts can call. This repo should prove that path with an integration test using fake adapters.
- Core may own deterministic state-machine sequencing and typed outcomes. Hosts own adapter implementation, executable choice, auth, persistence, config, retention, reconnect, and product policy.
- Session creation should take explicit host-resolved spawn inputs. Core must not infer executable, cwd, environment, admitted target, or user policy.
- Activity classification should reuse existing `SessionActivity` and reducer functions with injected timestamps.
- Plugin invocation should use `PluginWorkerEngine` and `PluginRuntime`; the assembly API must not add Lua or product plugin semantics.
- Notifications should use `NotificationInbox`; core drains by target and timestamp, while hosts deliver and present.
- No PII is required for tests or docs. Use synthetic ids and generic content.

Unknowns for implementation:

- Exact facade name is open. Prefer `MultiplexerEngine` or `CoreMultiplexer` over product names.
- Whether the facade should own session worker instances directly or accept host-created worker handles. Prefer the smallest ownership model that lets the integration test drive the required end-to-end path without adding async runtime policy.
- Whether session spawning and session worker runtime should share one fake adapter in tests or remain separate adapters matching existing traits. Prefer matching existing traits unless composition becomes awkward.
- Whether outcome types should be one broad `MultiplexerEngineOutcome` or operation-specific outcomes. Prefer operation-specific methods with a shared observation enum only if that keeps docs and tests clearer.
- Whether README needs a new section or a row update in the existing ownership table. Prefer a narrow README section documenting public API responsibilities.

No human question is blocking this plan. The ticket's acceptance criteria can be satisfied without waiving scope.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/multiplexer.rs`
  - New public facade that composes existing core engines and contracts.
  - Operation methods for session creation, client attach/routing, runtime event routing, plugin invocation, notification drain, activity observation, and shutdown.
- `crates/botster-core/src/engine/mod.rs`
  - Export the new facade and outcome/observation types.
- `crates/botster-core/src/lib.rs`
  - Re-export public host-facing API types.
- `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - Integration test that drives the exported public API through the required acceptance path.
- `crates/botster-core-test-support/src/fake/mod.rs`
  - Export fake adapter support if shared helpers are added.
- `crates/botster-core-test-support/src/fake/multiplexer.rs` or extensions to existing fake modules
  - Fake session runtime/worker/plugin/client helpers only as needed for the public API test.
- `crates/botster-core-test-support/src/fake/plugin_worker.rs` or equivalent
  - Shared fake `PluginRuntime` support lifted out of the current plugin-worker test so the facade integration test consumes reusable test-support fakes.
- `README.md`
  - Public API and host/core responsibility documentation.
- `docs/archive/plans/assemble-core-multiplexer-engine-api.md`
  - This plan artifact.

Possible but avoid unless compiler/API shape requires:

- `crates/botster-core/src/runtime/mod.rs`
  - Only for a narrow host adapter trait that composes existing runtime traits.
- `crates/botster-core/src/engine/session_worker.rs`
  - Only for a tiny accessor or constructor needed by the facade.
- `crates/botster-core/src/engine/subscription_multiplexer.rs`
  - Only for a tiny accessor needed to preserve subscriber/session routing through the facade.
- `crates/botster-core/src/contract/actor.rs`, `session.rs`, `notification.rs`, or `transport.rs`
  - Only if the public outcome needs an already-implied typed observation that cannot be expressed with current contracts.

Not expected:

- `Cargo.toml` dependency changes.
- `crates/botster-core-dev`.
- Any hub, CLI, browser, TUI, Rails, Lua plugin, MCP, Project Pipelines, provider, or old trybotster files.

## Implementation Shape

Suggested minimal public API:

- `MultiplexerEngine::new(adapter_or_config)` or `MultiplexerEngine::default()`.
- `spawn_session(request: SessionSpawnRequest, metadata: CoreSessionMetadata, now: u64) -> ...`
  - Calls host `SessionRuntime::spawn_session`.
  - Creates/records `CoreSession`.
  - Emits lifecycle/activity observation and returned `SessionRuntimeHandle`.
- `handle_client_ingress(client_id: ClientId, ingress: TransportIngress) -> ...`
  - Delegates to `SubscriptionMultiplexer`.
  - Returns session requests and client egress/control frames.
- `handle_session_request(request: SessionIoRequest) -> ...` or internal routing from `handle_client_ingress`
  - Delegates to the relevant `SessionWorkerEngine`.
- `handle_runtime_event(event: SessionWorkerRuntimeEvent) -> ...`
  - Delegates to the relevant session worker.
  - Translates `SessionWorkerOutcome` events such as terminal output and process lifecycle into `SubscriptionMultiplexer::handle_session_event` input, producing the client `TransportEgress` fanout that proves the assembly path.
  - Applies activity reducer for terminal output/process lifecycle.
  - Sends session events through `SubscriptionMultiplexer`.
- `invoke_plugin(request: PluginInvocationRequest) -> PluginInvocationResult`
  - Delegates to `PluginWorkerEngine`.
- `post_notification(item: NotificationItem) -> NotificationId`.
- `drain_notifications(target: NotificationTarget, now: NotificationTimestamp) -> Vec<NotificationItem>`.
- `classify_session_activity(session_id, now, threshold) -> Option<SessionActivityStatus>`.
- `shutdown_session(session_id, reason, now) -> ...`
  - Delegates shutdown through the session worker/runtime.
  - Updates lifecycle/activity state.
  - Prevents later fanout for the closed session where appropriate.

Suggested design constraints:

- Keep the facade synchronous and deterministic. Hosts can wrap it in async tasks outside core.
- Store only core state needed to route and observe the workflow: session records, session worker engines, subscription multiplexer, plugin worker engine, and notification inbox.
- Reuse typed errors/results that already exist. Add new typed engine errors only for facade-level conditions such as unknown session or missing worker adapter.
- Do not expose `serde_json::Value` or `BoundaryJson` for stable engine controls.
- Keep fake integration content generic and synthetic.

## Risks

- The main risk is overbuilding a hub inside `botster-core`. Mitigation: the facade coordinates reusable mechanics only and leaves auth, persistence, executable choice, config, reconnect, and product policy outside core.
- Duplicating behavior from `SessionWorkerEngine`, `SubscriptionMultiplexer`, `NotificationInbox`, or `PluginWorkerEngine` would create drift. Mitigation: compose existing public engines directly.
- A facade with only constructors and no integration test would fail the runtime-path proof requirement. The acceptance test must drive the exported API through real core values.
- Adding async workers, channels, or Tokio would cross the boundary from mechanism into runtime policy. Keep core synchronous.
- Treating session metadata as labels, cwd, prompt text, or product identity risks PII. Tests and docs must use synthetic ids and host-owned classification only.
- Plugin invocation can accidentally become Lua/product policy. The API should call `PluginWorkerEngine` with fake `PluginRuntime` only.
- Notification drain can accidentally imply transport delivery. The API should return drained items; hosts decide delivery/presentation.
- Clean shutdown can accidentally imply process supervision or retention. Core should update state and call runtime shutdown; host owns external cleanup.
- README docs can become too broad and claim hub integration that does not exist. Document the core API and host responsibilities precisely.

## Acceptance Checks / Tests

Required targeted test:

- `multiplexer_engine_drives_spawn_attach_output_notification_plugin_activity_and_shutdown`
  - Build the public engine with fake session runtime/worker runtime/plugin runtime/client ids.
  - Spawn a session from `SessionSpawnRequest`.
  - Attach two clients through `TransportIngress::SubscribeSession`.
  - Feed runtime terminal output through the engine.
  - Assert both clients receive fanout through `TransportEgress::TerminalOutput` with their own subscription ids.
  - Post and drain a session notification through the engine.
  - Register/invoke a plugin handler through the engine, assert the shared fake `PluginRuntime` receives the request, and assert the facade returns or surfaces the resulting `PluginInvocationResult` to the host caller.
  - Assert session activity classifies active after output using injected `now`/threshold.
  - Shut down the session cleanly and assert a shutdown/lifecycle observation is emitted.
  - Assert later runtime output does not produce client fanout for the closed session.

Additional focused tests if the implementation splits behavior:

- `engine_returns_typed_error_for_unknown_session_client_ingress`.
- `engine_keeps_host_spawn_policy_out_of_core` with synthetic executable/cwd/env already resolved by the host.
- `engine_docs_and_public_exports_are_consumable` through test imports from `botster_core`, not private module paths.

Existing test suites expected to remain green:

- `crates/botster-core/tests/session_activity_test.rs`
- `crates/botster-core/tests/session_runtime_contract_test.rs`
- `crates/botster-core/tests/session_worker_engine_test.rs`
- `crates/botster-core/tests/subscription_multiplexer_engine_test.rs`
- `crates/botster-core/tests/notification_inbox_test.rs`
- `crates/botster-core/tests/plugin_worker_engine_test.rs`
- full workspace tests.

Commands:

- `cargo fmt`
- `cargo test -p botster-core multiplexer_engine`
- `cargo test -p botster-core`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Runtime/user path proof:

- This ticket is not product host wiring. It intentionally changes the public core runtime path by giving embedders one exported engine API that coordinates the core multiplexer mechanisms.
- Implementation evidence must show tests importing the public `botster_core` API and driving the assembled path with real `SessionSpawnRequest`, `TransportIngress`, `SessionWorkerRuntimeEvent`, `NotificationItem`, and `PluginInvocationRequest` values.
- Evidence that the individual dependency engines still exist is not enough.

## Vault Gaps Worth Capturing

No durable vault capture is required before implementation.

Existing notes already constrain the plan:

- core owns reusable mechanisms while hub/plugins own policy
- Botster is a multiplexer that happens to run agents
- terminal clients share the SessionIo/ClientWorker data-plane path
- client workers own transport-neutral stream state
- plugin worker execution stays behind per-plugin worker boundaries
- notifications are core primitives while delivery remains host-owned
- Project Pipelines remains a device-level plugin, not core workflow policy
- pipeline agents must use explicit target ids and explicit worktrees

Capture later only if implementation settles a reusable convention for public engine facade naming, host adapter shape, or the boundary between core-owned synchronous coordination and host-owned async supervision.

## Vault Checklist Evidence

- Vault/project notes constrained the plan: `planner-playbook`, `botster-planner-playbook`, `botster-architecture`, `cli-patterns`, `spa-patterns`, Project Pipelines orchestration/workbench/UI-contract notes, explicit target/worktree orchestration notes, identity/goals context, and the prior dependency plan artifacts.
- Convention conflicts: none. The plan keeps reusable mechanism in `botster-core` and host/product policy outside core.
- Verification evidence so far: planning inspection only; no build/test commands were run because this step creates the implementation plan. Planned verification commands are listed above.
- Durable knowledge capture: no capture before implementation. Capture after implementation only if the facade API creates a durable convention.
