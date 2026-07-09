# Non-Blocking WebSocket Capability Runtime

## Context Loaded

- Pipeline context: `ticket_1780417025_597962`, run `run_1780421304_863446`, step `botster_plan`, gate `botster_plan_gate`, target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, implementation worktree recorded by Project Pipelines.
- Ticket dependency: `ticket_1780417024_852482` is closed and merged through PR #58 at `1cb26b0`. This branch was created before that merge and is currently behind `origin/main`; implementation must first bring `origin/main` into the ticket branch.
- Required playbooks: `planner-playbook`, `botster-planner-playbook`.
- Required Botster overlays/notes: `botster-architecture`, `cli-patterns`, `spa-patterns`, `project pipeline orchestration belongs in a device-level botster plugin`, `project pipelines needs an operator workbench not more primitives`, `project pipelines ui contract belongs in the plugin readme`, `botster orchestration should spawn agents with explicit target ids`, `botster orchestration prompts must bind agents to explicit worktrees`.
- Repo evidence inspected from `origin/main`: `crates/botster-core/src/runtime/capability.rs`, `crates/botster-core/src/runtime/mod.rs`, `crates/botster-core/src/engine/plugin_worker.rs`, `crates/botster-core/src/contract/actor.rs`, `crates/botster-core/tests/plugin_capability_runtime_test.rs`, `crates/botster-core/tests/plugin_worker_engine_test.rs`, and `docs/architecture/non-blocking-plugin-capability-runtime.md`.
- Checklist evidence: `project_pipelines_create_vault_checklist` failed with a Project Pipelines SQLite write lock, so vault checklist evidence is preserved in this plan and in gate/artifact evidence.

## Scope

Implement the WebSocket family of the accepted capability runtime as the smallest core primitive that can be tested without introducing hub, Rails, WebRTC, TUI, or browser dependencies.

The implementation should:

- Start from `origin/main` so `botster_core::runtime::capability` exists.
- Extend the WebSocket capability contract with the fields needed for bounded runtime behavior: connection id/resource identity, connect/send/receive/close actions, typed lifecycle events, typed message events, typed close events, and typed error events.
- Add a small policy-free core WebSocket runtime or harness under `crates/botster-core/src/runtime/` that implements `PluginCapabilityRuntime` for WebSocket operations through bounded queues.
- Use `CapabilitySurface::Network` with scope `websocket` for connect/send/close admission.
- Track each connection as `PluginResourceKind::NetworkConnection` and record/release resources through existing `PluginWorkerEngine::record_resource`, unload, and reload cleanup paths.
- Model bounded outbound send queues and bounded inbound event/receive queues with typed `BackpressureSummary` values using `QueueSource::PluginWorker` and `BackpressureRoute.plugin_key`.
- Make cancellation, timeout, close, runtime stop, queue-full, unknown operation, and unknown resource outcomes explicit through `CapabilityRuntimeErrorKind` and `CapabilityRuntimeEvent`.
- Keep reconnect as a host-profile decision. Core exposes no reconnect surface in this ticket; host profiles reconnect by issuing a fresh `Connect` request and own retry loops, origin policy, credentials, and product behavior.
- Update `docs/architecture/non-blocking-plugin-capability-runtime.md` to state that WebSocket now has a tested core runtime primitive/harness while concrete network adapters remain host-profile-owned.

## Non-Scope

- No real network client dependency unless the implementer proves it is necessary; prefer a transport-adapter trait and deterministic in-memory fake for this ticket.
- No hub, Rails, ActionCable, WebRTC, TUI, SPA, or Project Pipelines product-policy code.
- No Lua plugin API wiring, package manifest grant defaults, origin allowlist policy, credential lookup, or reconnect policy implementation.
- No broad rewrite of `PluginWorkerEngine`, session/client workers, PTY paths, or queue source taxonomy.
- No speculative configuration surface beyond queue capacities needed by the WebSocket runtime tests.

## Assumptions And Unknowns

- Assumption: the merged scaffold from PR #58 is the accepted architecture baseline. The first implementation step is a non-destructive branch update to include `origin/main`.
- Assumption: "WebSocket capability runtime" means a core, policy-free runtime boundary and deterministic harness, not a concrete internet WebSocket client in this crate.
- Assumption: slow remote/client isolation can be proven deterministically by saturating bounded WebSocket queues and asserting synchronous `Backpressured` return from `submit`, not by timing unrelated work.
- Unknown: whether the host adapter should be named WebSocket-specific or generalized as a network-connection adapter. Implementer should choose the smallest name that keeps public contracts clear and does not collide with future non-WebSocket network resources.
- Decision: inbound WebSocket receive is events-only through `CapabilityRuntimeEvent::WebSocketMessage`; this ticket does not add a `Receive` request action.

## Affected Surfaces And Files

Likely changed:

- `crates/botster-core/src/runtime/capability.rs`: WebSocket request/event/error shapes and any helper methods.
- `crates/botster-core/src/runtime/mod.rs`: exports for any new WebSocket runtime types.
- `crates/botster-core/src/lib.rs`: crate-root exports for public WebSocket runtime types.
- `crates/botster-core/tests/plugin_capability_runtime_test.rs`: contract tests for capability gating, serde, lifecycle events, close/error typing, and pressure route.
- `crates/botster-core/tests/plugin_worker_engine_test.rs`: unload/reload cleanup proof for `NetworkConnection` resources owned by one plugin.
- `docs/architecture/non-blocking-plugin-capability-runtime.md`: runtime-path and policy-boundary update.

Possible if the smallest implementation needs them:

- `crates/botster-core/src/runtime/websocket.rs` or a submodule inside `runtime/capability.rs`: bounded in-memory WebSocket runtime/harness.
- `crates/botster-core/src/contract/actor.rs`: only if existing `PluginResourceKind::NetworkConnection` is insufficient. Avoid adding a WebSocket-specific resource kind unless tests prove the generic kind is ambiguous.

## Risks

- Reimplementing product/network policy in core. Allowed origins, credentials, reconnect strategy, concrete network backend, and default grants belong to host profiles.
- Type-only tests. Acceptance requires proving bounded queues, lifecycle/error events, cleanup, and isolation behavior, not merely serde or enum existence.
- Stale base branch. Implementing before merging `origin/main` would recreate or diverge from the accepted scaffold.
- Queue semantics drift. WebSocket send/receive pressure must be typed and bounded; do not hide pressure in strings or unbounded Vecs.
- Hot-path regression. The tests must prove PTY/session/client/plugin-worker unrelated work is not blocked by slow WebSocket remote/client behavior.
- Public API churn. New exported enum variants or public structs should be justified by acceptance and covered by tests because downstream exhaustive matches can break.

## Acceptance Checks And Tests

Required verification after implementation:

- `cargo fmt --all -- --check`
- `BOTSTER_ENV=test cargo test -p botster-core --test plugin_capability_runtime_test`
- `BOTSTER_ENV=test cargo test -p botster-core --test plugin_worker_engine_test`
- `BOTSTER_ENV=test cargo test -p botster-core`
- `BOTSTER_ENV=test cargo clippy -p botster-core --all-targets --all-features -- -D warnings`
- `BOTSTER_ENV=test cargo doc -p botster-core --no-deps --all-features`

Required behavior evidence:

- Connection capability gating rejects missing `Capability { surface: Network, scope: Some("websocket") }`.
- Connect returns a `CapabilityRuntimeHandle` with a `NetworkConnection` resource and emits typed open/lifecycle events.
- Send uses a bounded queue; saturation returns `Backpressured` or emits `CapabilityRuntimeEvent::Backpressure` with plugin route, capacity, and depth.
- Inbound receive/event delivery uses a bounded queue; slow plugin callback/client consumption cannot grow memory unbounded and reports typed pressure.
- Slow or blocked WebSocket remote behavior cannot block unrelated plugin invocations or session/client hot paths. At minimum, a regression should invoke unrelated `PluginWorkerEngine` work or session/client harness work while a WebSocket operation is saturated.
- Close releases the resource and emits typed closed/released events. Unknown or double-close returns `ResourceNotFound` or a typed no-live-resource error.
- Timeout and cancellation produce typed `TimedOut`/`Cancelled` errors or events and stop pending operation/resource state.
- Plugin unload/reload cleanup removes only the owning plugin's WebSocket resources and does not remove another plugin's connection.
- Docs compile and the architecture doc explicitly preserves hub/profile ownership of reconnect policy, origins, credentials, quotas, and concrete backend selection.
- PII scan over committed diff has no real local paths, secrets, tokens, or personal data. Synthetic fixtures must be clearly fake.

## Vault Gaps Worth Capturing

- No new architecture convention is needed yet; existing notes cover core-vs-profile policy, plugin-worker boundaries, bounded queues, and checklist timeout fallback.
- If implementation discovers a reusable testing pattern for non-network WebSocket isolation without a real socket backend, capture it as a Botster runtime test-harness note.
- The Project Pipelines checklist SQLite write lock recurred here; it is already a known workflow infrastructure issue, and this plan follows the artifact/gate evidence fallback.
