# Define Plugin Worker Boundary Contract

## Context Loaded

- Pipeline context: `ticket_1780014883_957892`, `run_1780030157_678639`, current step `botster_plan`, gate `botster_plan_gate`.
- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1780014883_957892`.
- Target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Dependencies loaded from pipeline context:
  - `ticket_1780014863_508751` closed: Define botster-core actor contract types.
  - `ticket_1780014899_889542` closed: Write core hub client provider boundary README.
- Required playbooks loaded:
  - `/Users/jasonconigliari/knowledge/notes/planner-playbook.md`
  - `/Users/jasonconigliari/knowledge/notes/botster-planner-playbook.md`
- Required Botster overlay notes loaded:
  - `/Users/jasonconigliari/knowledge/notes/botster-architecture.md`
  - `/Users/jasonconigliari/knowledge/notes/cli-patterns.md`
  - `/Users/jasonconigliari/knowledge/notes/spa-patterns.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipeline orchestration belongs in a device-level botster plugin.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipelines needs an operator workbench not more primitives.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipelines ui contract belongs in the plugin readme.md`
  - `/Users/jasonconigliari/knowledge/notes/botster orchestration should spawn agents with explicit target ids.md`
  - `/Users/jasonconigliari/knowledge/notes/botster orchestration prompts must bind agents to explicit worktrees.md`
- Additional ticket-specific vault notes loaded:
  - `/Users/jasonconigliari/knowledge/notes/plugin workers use typed mailbox handler refs not lua closures.md`
  - `/Users/jasonconigliari/knowledge/notes/botster plugin runtime uses supervisor plus per plugin workers.md`
  - `/Users/jasonconigliari/knowledge/notes/plugin supervisor invocation wraps plugin owned action execution.md`
  - `/Users/jasonconigliari/knowledge/notes/plugin hardening needs lifecycle resource and observability layers.md`
  - `/Users/jasonconigliari/knowledge/notes/plugin tests must prove worker boundaries not hub leakage.md`
- Repo context loaded:
  - `src/actor.rs`: existing actor queue, backpressure, hub/client/session, and initial plugin-worker contract types.
  - `src/boundary.rs`: `BoundaryJson` escape hatch for Lua/plugin/relay payloads.
  - `src/transport.rs`: transport-neutral ingress/egress frames.
  - `src/lib.rs`: public exports for current actor contracts.
  - `tests/actor_contract_test.rs`: current actor contract tests, including handler-ref and no-`mlua` checks.
  - `README.md`: core owns reusable contracts, while hub owns policy, lifecycle, routing, recovery, and extension supervision.
- Old trybotster evidence loaded as reference only:
  - `/Users/jasonconigliari/Rails/trybotster/docs/worker-actor-contracts.md`
  - `/Users/jasonconigliari/Rails/trybotster/docs/lua/hot-reload.md`
  - `/Users/jasonconigliari/Rails/trybotster/docs/lua/hook-system.md`
  - `/Users/jasonconigliari/Rails/trybotster/docs/lua/session-actions.md`
  - `/Users/jasonconigliari/Rails/trybotster/cli/src/worker/plugin.rs`

## Scope

Build the plugin-worker boundary contract in `botster-core` as serializable Rust shapes and tests. The work should extend the existing actor-contract slice rather than re-port old runtime code.

In scope:

- `PluginHandlerRef` and `PluginHandlerKind` coverage for every executable plugin-owned handler family named by current Botster notes and old evidence.
- Plugin load/reload/unload specs that describe plugin identity, source metadata, descriptor ownership, and cleanup intent without owning hub policy.
- Invocation request/result shapes with request id, handler ref, timeout, serializable invocation context, payload, success/failure result, and explicit timeout attribution.
- Plugin-worker event shapes for loaded, reloaded, unloaded, invocation completed, invocation failed, invocation timed out, backpressure, cleanup completed, and stopped.
- Descriptor ownership shapes that let the parent hub keep descriptor registries while executable behavior stays addressed by handler refs.
- Test-harness helpers in tests only to prove descriptor replacement and per-plugin backpressure isolation.
- Public exports from `src/lib.rs` for any new contract types.

Non-scope:

- No Lua VM, `mlua`, closure, function pointer, worker thread, Tokio mailbox, supervisor runtime, file watcher, timer runner, MCP runtime, or hub registry implementation.
- No product plugins, Project Pipelines policy, Cloudflare/GitHub/provider policy, browser UI, TUI UI, Rails relay behavior, or hub orchestration policy.
- No old trybotster source import, include, copy, or compatibility shim.
- No broad actor refactor outside the plugin-worker contract surface.

## Botster Layers Touched

- Core: new reusable, transport-neutral plugin-worker contract types.
- Hub: only as named future consumer of descriptor ownership and lifecycle events; no hub implementation in this crate.
- Plugin runtime: only as named future consumer of handler refs and invocation messages; no runtime implementation in this crate.
- Tests/docs: contract tests and this plan artifact.

## Assumptions And Unknowns

Assumptions:

- This ticket is intentionally scaffold-only inside `botster-core`; success is exported, serializable contract shapes plus tests, not a production runtime entry point.
- `serde`, `serde_json`, and existing core identifiers are sufficient; no new dependency should be added.
- `BoundaryJson` remains the right wrapper for plugin-owned payloads and return values because those schemas belong to plugins, not core.
- Descriptor ownership should be typed enough to name owner plugin, descriptor kind, descriptor id, and optional handler ref, but the descriptor body itself can stay `BoundaryJson` when plugin-owned.
- Reload/unload contracts should describe replacement and cleanup boundaries, not implement filesystem watching or registry mutation.

Unknowns for implementation:

- Exact descriptor kind enum names may need to track the current hub descriptor vocabulary when downstream integration begins. For this ticket, prefer broad reusable families such as action, session_action, command, hook, surface_route, asset, timer, mcp, event, http, watch, action_cable, entity_provider, and notification.
- Whether invocation context needs dedicated typed fields for client/session/surface/source in this slice or a narrower initial struct with optional `client_id`, `session_id`, `subscription_id`, `surface_id`, `origin`, and `metadata: BoundaryJson`. Prefer the narrowest shape that still proves serializability and timeout attribution.
- Whether `PluginHandlerKind::Mcp` should be split into `McpTool`, `McpPrompt`, `McpResource`, and `McpProxyAuthError`. The evidence favors explicit handler kinds where attribution matters; implementer should choose explicit variants unless it creates churn with existing tests.

No human question is blocking this plan. The ticket acceptance criteria are specific enough to proceed without waiving scope.

## Affected Surfaces And Files

Expected changes:

- `src/actor.rs`
  - Extend existing plugin contract types.
  - Add invocation context/request/result/failure/timeout types.
  - Add descriptor ownership and reload/unload cleanup shapes.
  - Add plugin-scoped backpressure/failure events that carry plugin identity.
- `src/lib.rs`
  - Export the new public contract types.
- `tests/actor_contract_test.rs`
  - Extend existing plugin-worker tests and add focused harness assertions for the ticket acceptance criteria.
- `README.md`
  - Optional narrow update if a new public contract category needs to appear in the ownership proof table.
- `docs/plans/plugin-worker-boundary-contract.md`
  - This plan artifact.

Not expected:

- `Cargo.toml`
- `src/boundary.rs`, unless documentation for `BoundaryJson` needs a small clarification.
- `src/transport.rs`, `src/session.rs`, `src/client.rs`, except if tests reveal an export/import gap created by this ticket's own changes.

## Implementation Plan

1. Extend plugin identity and handler kind coverage in `src/actor.rs`.
   - Keep `PluginKey`, `PluginHandlerRef`, and stable handler ids.
   - Add missing handler families from current Botster notes where needed, such as watch, action_cable, entity_provider, notification, and MCP sub-kinds if explicit attribution is cleaner.

2. Add serializable invocation contracts.
   - Add `PluginInvocationContext` with typed optional core ids and small source/origin metadata.
   - Add `PluginInvocationRequest` carrying `request_id`, `handler`, `timeout_ms`, `context`, and plugin-owned `BoundaryJson` payload.
   - Add `PluginInvocationResult` or equivalent success/failure enum carrying `request_id`, `handler`, payload, failure reason, and timeout attribution.
   - Update `PluginWorkerMessage::Invoke` to use the request struct rather than open-coded fields if that improves clarity.

3. Add descriptor ownership contracts.
   - Add a descriptor owner record keyed by `plugin_key`.
   - Add descriptor reference/record shapes with descriptor kind, descriptor id, optional handler ref, and plugin-owned descriptor payload.
   - Keep executable handler refs separate from descriptor payloads so handler refs never contain function values.

4. Add reload/unload cleanup contracts.
   - Add reload request/spec shape that names the plugin being replaced and the new load spec.
   - Add unload request/spec shape that names the plugin and cleanup scope.
   - Add cleanup result shape with counts or descriptor/resource refs removed, scoped by plugin key.
   - Events should make it verifiable that reload replaces one plugin's descriptors/resources without touching other plugin identities.

5. Add plugin-scoped pressure and timeout events.
   - Ensure backpressure events carry the affected `plugin_key` through `BackpressureRoute` or a plugin-specific event field.
   - Add timeout/failure event variants where timeout attribution includes `request_id`, `handler`, `plugin_key`, and configured timeout.

6. Export new types from `src/lib.rs`.

7. Add contract tests.
   - Prefer focused tests over broad source snapshots.
   - Use small in-test descriptor maps to prove replacement/removal semantics without adding runtime registry code to core.

## Risks

- Over-porting old trybotster runtime internals would put hub lifecycle policy into `botster-core`.
- Under-typing invocation/failure events would make timeout attribution ambiguous, failing the acceptance criteria.
- Treating `BoundaryJson` as a blanket control-message escape hatch would weaken the existing actor-contract boundary.
- Descriptor ownership tests could accidentally become a fake registry implementation. Keep them as local harness evidence over public types.
- Handler kinds that are too coarse may obscure timeout and failure attribution for MCP/resource subpaths; handler kinds that are too fine may create avoidable churn. Choose explicitness where acceptance evidence depends on it.

## Acceptance Checks And Tests

Run:

- `cargo test`
- If the implementation touches only one test file and quick iteration is needed first: `cargo test --test actor_contract_test`

Required named checks:

- `plugin_handler_refs_never_contain_function_values`
  - Instantiate every handler kind and serialize handler refs.
  - Assert the debug/JSON output contains stable ids and never contains `function`, `closure`, `mlua`, or `Function`.
  - Assert `Cargo.toml` still has no `mlua` dependency.

- `plugin_invocation_context_is_serializable_and_timeout_attributed`
  - Round-trip an invocation request and timeout/failure event.
  - Assert `request_id`, `handler.plugin_key`, `handler.handler_id`, handler kind, `timeout_ms`, and failure kind survive.

- `plugin_reload_replaces_only_one_plugins_descriptors_in_harness`
  - Build a small in-test map with descriptors for two plugin keys.
  - Apply the public reload/cleanup contract for one plugin.
  - Assert only that plugin's descriptor/resource refs are removed/replaced and the other plugin's records remain.

- `plugin_unload_cleanup_is_scoped_to_owner_plugin`
  - Round-trip unload/cleanup result shapes.
  - Assert every removed descriptor/resource ref carries the same owner plugin.

- `plugin_backpressure_is_scoped_to_one_plugin_identity`
  - Instantiate backpressure events for two plugin keys.
  - Assert pressure for plugin A does not require or mutate plugin B in the harness.
  - Assert event routing includes plugin identity explicitly.

- `plugin_worker_messages_cover_load_invoke_reload_unload_shutdown`
  - Round-trip representative `PluginWorkerMessage` and `PluginWorkerEvent` values for the full lifecycle.

Runtime path evidence:

- This is intentionally scaffold-only in `botster-core`.
- The changed path is compile-time/runtime-contract consumption: downstream hub and plugin-worker crates can import one public contract for handler refs, invocation messages, descriptor ownership, reload/unload cleanup, timeout/failure, and backpressure events.
- The proof is exported public types plus serde tests and harness assertions over public contract values.

## Pipeline Gates And Artifacts

- Plan artifact: `docs/plans/plugin-worker-boundary-contract.md`.
- Gate evidence should attach this plan and explicitly state that no production runtime entry point is expected for this crate-level contract ticket.
- Plan-review should check that the implementation scope remains limited to core contract types, tests, and narrow documentation.

## Vault Gaps Worth Capturing

No durable vault capture is needed from planning alone. Existing vault notes already cover:

- handler refs instead of Lua closures
- supervisor plus per-plugin workers
- descriptor registries in the hub and executable behavior in workers
- plugin hardening through lifecycle/resource/observability layers
- tests proving worker boundaries rather than hub leakage

Capture later only if implementation discovers a new reusable rule for descriptor ownership vocabulary or invocation-context fields that should constrain future plugin-worker tickets.
