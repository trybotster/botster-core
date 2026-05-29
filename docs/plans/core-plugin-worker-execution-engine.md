# Core Plugin Worker Execution Engine

Ticket: `ticket_1780075966_133421`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: run `run_1780077476_802225`, current step `botster_plan`, gate `botster_plan_gate`, ticket `Build core plugin worker execution engine`.
- Orchestrator correction received through Botster inbox: this run is main-rooted. Ignore the auto-populated `base_run_id` / `base_ticket_id` as stacking context, target `main`, and plan against the current workspace layout at main commit `e29bea1`.
- Current worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1780075966_133421`.
- Target id from pipeline context: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
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
- Repo context loaded:
  - `crates/botster-core/src/contract/actor.rs`
  - `crates/botster-core/src/engine/mod.rs`
  - `crates/botster-core/src/runtime/mod.rs`
  - `crates/botster-core/src/package/{capability,manifest,mod}.rs`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/tests/actor_contract_test.rs`
  - `crates/botster-core/tests/boundary_test.rs`
  - `crates/botster-core-test-support/src/fake/mod.rs`
  - `Cargo.toml`
  - prior plans `docs/plans/plugin-worker-boundary-contract.md` and `docs/plans/contract-test-fixtures-regression-shapes.md`

## Scope

Build the reusable plugin worker execution engine in `botster-core`, moving beyond the existing boundary structs without embedding Lua, WASM, hub product policy, Rails/cloud/auth behavior, or marketplace/update policy.

In scope:

- Add an abstract plugin runtime interface in `crates/botster-core/src/runtime/mod.rs` so Lua, WASM, or host runtimes can plug in behind core-owned execution mechanics.
- Add a core engine module, likely `crates/botster-core/src/engine/plugin_worker.rs`, that owns:
  - plugin identity keyed worker state
  - handler registry by `PluginHandlerRef`
  - descriptor ownership registry by `PluginDescriptorRef`
  - invocation dispatch through the abstract runtime
  - timeout attribution using the existing `PluginInvocationFailureKind::TimedOut`
  - failure attribution using existing request id, handler ref, plugin key, and reason fields
  - reload/unload cleanup scoped to one `PluginKey`
  - backpressure isolation through per-plugin bounded queues or equivalent per-plugin capacity accounting
  - capability checks against declared `PackageManifest.capabilities`
- Extend existing contract structs only where the engine requires stable public inputs. The likely addition is declared required capabilities on handlers/descriptors or load specs so the engine can reject undeclared capability use without guessing from handler kind.
- Add fake runtime support in `crates/botster-core-test-support` or integration tests to prove the actual engine path, not only serde shape existence.
- Export new engine/runtime APIs from `crates/botster-core/src/lib.rs` when they are public core contracts.
- Add a narrow README note only if the new engine surface needs discoverability in the ownership boundary.

Non-scope:

- No product plugins, Project Pipelines workflow policy, GitHub/Cloudflare/provider logic, Rails relay behavior, auth, marketplace/update policy, or user-specific paths.
- No Lua VM, `mlua`, WASM interpreter, Tokio runtime, browser UI, TUI UI, or hub orchestration implementation inside `botster-core`.
- No old trybotster source import or compatibility shim. Old trybotster paths are reference evidence only and should not be required to exist.
- No broad actor/transport/entity refactor unrelated to plugin worker execution.
- No speculative configuration surface beyond what tests need to prove handler dispatch, timeouts, failures, cleanup, backpressure, and capability rejection.

## Botster Layers Touched

- Core: primary surface. The engine and runtime traits belong here because they are reusable mechanism.
- Plugin runtime: abstract trait only. Concrete Lua/WASM/host interpreters remain outside core.
- Hub: future consumer only. Hub policy, plugin installation policy, and product descriptor routing remain out of this ticket.
- Tests: fake runtime and integration tests prove the engine execution path.
- Docs: this plan artifact, and possibly a narrow README ownership note.

## Assumptions And Unknowns

Assumptions:

- The previous boundary-contract ticket already supplied most message, handler, descriptor, cleanup, timeout, and backpressure structs; this ticket should reuse those instead of renaming them.
- The implementation should be `std` plus current dependencies unless a test proves a missing primitive. No new async runtime dependency should be introduced for this core crate.
- Timeout handling can be proven with an engine-owned dispatch boundary around a fake runtime. The fake runtime may block or delay, but the core API should report timeout as a `PluginInvocationFailure` attributed to the original request and handler.
- Capability enforcement should compare requested handler/descriptor capabilities against `PackageManifest.capabilities`. If current structs cannot express requested capabilities cleanly, add the smallest typed field rather than infer product policy from descriptor names.
- Backpressure isolation means plugin A reaching capacity must reject or report pressure for plugin A without blocking, corrupting, or clearing plugin B.

Unknowns for implementation:

- Whether engine APIs should expose a long-lived worker handle per plugin or a synchronous `PluginWorkerEngine` facade with internal per-plugin workers. Prefer the smallest API that lets tests prove production-like dispatch and cleanup.
- Whether fake runtime helpers belong in `botster-core-test-support` for reuse or directly in `crates/botster-core/tests/plugin_worker_engine_test.rs`. Prefer test-support only if more than one test file needs them.
- Whether handler capabilities should live on `PluginOwnedDescriptor`, `PluginHandlerRef`, or a separate `PluginHandlerRegistration`. Prefer a registration type if it avoids overloading descriptor bodies.

No human question is blocking the plan. The ticket is specific enough to proceed without waiving requirements.

## Affected Surfaces And Files

Expected changes:

- `crates/botster-core/src/runtime/mod.rs`
  - Define abstract plugin runtime trait(s) and runtime errors/results.
- `crates/botster-core/src/engine/mod.rs`
  - Export the plugin worker engine module.
- `crates/botster-core/src/engine/plugin_worker.rs`
  - Implement registry, dispatch, timeout/failure attribution, cleanup, backpressure, and capability checks.
- `crates/botster-core/src/contract/actor.rs`
  - Narrow additions only if the engine needs handler/descriptor capability declarations or clearer registration inputs.
- `crates/botster-core/src/lib.rs`
  - Public exports for new core engine/runtime types.
- `crates/botster-core/tests/plugin_worker_engine_test.rs`
  - Fake runtime tests for the acceptance criteria.
- `crates/botster-core-test-support/src/fake/mod.rs`
  - Optional fake runtime helpers if sharing is useful.
- `README.md`
  - Optional narrow update for the new core mechanism.

Not expected:

- `crates/botster-core/src/contract/transport.rs`
- `crates/botster-core/src/contract/entity.rs`
- `crates/botster-core/src/identity/*`
- `crates/botster-core-dev/*`
- Product plugin, Rails, browser, or TUI paths.

## Implementation Plan

1. Define the runtime abstraction.
   - Add trait(s) for loading, invoking, reloading/unloading, and stopping a plugin runtime.
   - Keep interpreter-specific values behind `BoundaryJson` or existing core structs.
   - Return typed `PluginInvocationSuccess` / `PluginInvocationFailure` values instead of runtime-specific errors.

2. Define engine-owned registration inputs.
   - Use `PackageManifest` as the declared capability source.
   - Register handlers/descriptors with stable `PluginHandlerRef` and `PluginDescriptorRef`.
   - Add explicit required capability metadata if existing structs do not provide it.

3. Implement plugin-scoped state.
   - Maintain per-plugin handler and descriptor registries.
   - Keep descriptor ownership separate from executable handler dispatch.
   - Use per-plugin queue/capacity state so pressure is attributed to one plugin key.

4. Implement invocation dispatch.
   - Look up the handler by ref.
   - Check the handler's required capability against the owning manifest.
   - Dispatch to the owning runtime.
   - Convert missing handler, capability rejection, runtime failure, queue pressure, stopped worker, and timeout into attributed failures.

5. Implement timeout handling.
   - Use the smallest core-owned concurrency boundary that can enforce an invocation deadline without adding Tokio.
   - Ensure timeout failures include `request_id`, `handler`, `plugin_key`, and `timeout_ms`.

6. Implement reload/unload cleanup.
   - Reload replaces only one plugin's runtime, handler records, descriptor records, and resources.
   - Unload removes only the owner plugin's descriptors/resources and stops that runtime.
   - Cleanup results should reuse `PluginCleanupResult` and carry removed refs for reviewable evidence.

7. Add fake runtime tests.
   - Fake runtime should support success, failure, delay/timeout, and per-plugin state.
   - Tests must call the engine API directly so the actual runtime path changed is proven.

## Risks

- Over-porting old trybotster runtime internals would put hub/plugin policy in `botster-core`.
- A pure registry implementation without runtime invocation would fail the ticket's "execution engine" intent.
- Timeout tests can become flaky if they depend on tight wall-clock thresholds. Use generous bounds and deterministic fake runtime controls where possible.
- Capability checks can become product policy if inferred from plugin names or descriptor ids. They must be based on declared package metadata.
- Backpressure tests that use one global queue would miss the isolation requirement. Pressure must be keyed by plugin identity.
- Reload cleanup can corrupt neighboring plugins if registries are filtered by descriptor kind or package name instead of `PluginKey`.

## Acceptance Checks And Tests

Required tests:

- `handler_invocation_dispatches_to_registered_runtime`
  - Register a fake runtime and handler.
  - Invoke through the engine.
  - Assert the fake runtime receives the expected request and the engine returns the handler's success payload.

- `invocation_timeout_is_attributed_to_request_handler_and_plugin`
  - Configure fake runtime delay beyond `timeout_ms`.
  - Assert the engine returns or emits `PluginInvocationFailureKind::TimedOut` with `request_id`, `handler`, `plugin_key`, and `timeout_ms`.

- `runtime_failure_is_attributed_without_corrupting_other_plugins`
  - Register two plugins.
  - Make plugin A fail one handler.
  - Assert plugin B can still invoke successfully and plugin A failure carries plugin A identity.

- `reload_cleanup_replaces_one_plugin_descriptors_only`
  - Register descriptors for plugin A and plugin B.
  - Reload plugin A.
  - Assert old A descriptors/resources are cleaned, new A descriptors are registered, and B records remain unchanged.

- `unload_cleanup_removes_only_owner_plugin`
  - Unload plugin A.
  - Assert cleanup result refs all carry plugin A and plugin B remains registered/invokable.

- `capability_rejection_uses_declared_package_metadata`
  - Register a handler/descriptor requiring a capability missing from `PackageManifest.capabilities`.
  - Assert invocation or registration is rejected with an attributed failure.

- `backpressure_is_isolated_by_plugin_identity`
  - Saturate plugin A's queue/capacity.
  - Assert pressure is reported for plugin A and plugin B can still accept/invoke work.

Commands:

- `cargo fmt`
- `cargo test -p botster-core plugin_worker`
- `cargo test -p botster-core`
- `cargo clippy -p botster-core --all-targets --all-features -- -D warnings`

Runtime path evidence:

- This ticket is not scaffold-only. The Implement step must prove the engine API itself is used by tests: register fake runtimes, invoke handlers through the engine, observe timeout/failure/reload/unload/backpressure/capability behavior from the engine, and not merely instantiate contract structs.
- Production host integration remains future work. The core runtime path changed when downstream hub/plugin-runtime crates can call the exported engine and runtime trait rather than assembling handler maps and dispatch semantics ad hoc.

## Pipeline Gates And Artifacts

- Plan artifact: `docs/plans/core-plugin-worker-execution-engine.md`.
- Gate evidence should include the orchestrator correction that the run is main-rooted and should not stack on the dependency ticket.
- Plan Review should check that every proposed changed file traces to core plugin worker execution, required tests, or narrow documentation.

## Vault Gaps Worth Capturing

No durable vault capture is required from planning alone. Existing notes already cover:

- core owns reusable mechanism while hub/plugins own policy
- supervisor plus per-plugin workers
- typed handler refs instead of closures
- descriptor registries in the parent hub with executable behavior in workers
- plugin tests proving worker boundaries rather than hub leakage
- project-pipeline orchestration staying plugin-owned

Capture later only if implementation discovers a reusable convention for handler capability declaration placement or for deterministic timeout testing in core Rust engine code.
