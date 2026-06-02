# Capability-Scoped File Watcher Runtime Plan

## Context Loaded

- Pipeline context: `ticket_1780417025_690228`, active run `run_1780421305_522856`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts/findings/questions/answers on this run.
- Dependency context: `ticket_1780417024_852482` is closed; PR #58 merged as `1cb26b0` and added the scaffold-only capability runtime contract in `crates/botster-core/src/runtime/capability.rs`.
- Local repo state: current worktree branch is still at pre-PR #58 `0ebb5e0`; `git fetch origin main` shows `FETCH_HEAD` at `1cb26b0`. The worktree also has local setup churn. Implement should reconcile onto post-PR #58 `main` before editing and must not stage local setup files.
- Vault/playbooks loaded: [[planner-playbook]], [[botster-planner-playbook]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[plugin file watch callbacks run in plugin worker vms]], [[plugin worker watchers can block tokio runtime shutdown]], [[botster plugin runtime uses supervisor plus per plugin workers]], [[plugin auto-detection requires dual watchers for directory and file events]] (historical/outdated watcher context), [[plugin-scoping-is-unrestricted-by-default]], [[workspace_include requires directory-level pruning not git ls-files alone]], plus [[identity]] and [[goals]].
- Repo surfaces inspected: current checkout plus `FETCH_HEAD:crates/botster-core/src/runtime/capability.rs`, `FETCH_HEAD:crates/botster-core/src/contract/actor.rs`, `FETCH_HEAD:docs/architecture/non-blocking-plugin-capability-runtime.md`, existing `PluginWorkerEngine`, `PluginResourceKind::Watch`, `CapabilitySurface::Filesystem`, `QueueSource::PluginWorker`, and fake plugin runtime test support.

## Scope

Implement the file-watcher member of the post-PR #58 capability runtime, keeping it mechanism-level and host/profile-policy-free.

1. Add a core-owned `FileWatchRuntime` or equivalently named runtime in `botster-core` that implements `PluginCapabilityRuntime` for `CapabilityOperation::Watch` requests.
2. Add an injectable `FileWatchEventSource` trait and fake source support so tests can register watches, emit file events, and assert cleanup without depending on OS watcher behavior.
3. Validate watch registration in core before accepting work:
   - operation must be `CapabilityOperation::Watch`;
   - package/grant set must include `Capability { surface: Filesystem, scope: Some(scope_id) }`;
   - `ScopedRelativePath::is_scoped_relative()` must pass;
   - callback handler, when present, must belong to the same `PluginKey`.
4. Own bounded watch-event delivery in core:
   - bounded registration/event queues;
   - typed `CapabilityRuntimeErrorKind::Backpressured` on submit when full;
   - typed `CapabilityRuntimeEvent::Backpressure` with `QueueSource::PluginWorker` and `BackpressureRoute.plugin_key`;
   - no blocking wait for queue space.
5. Own debounce/coalescing semantics for file changes:
   - coalesce noisy backend events by plugin/resource/path/change-family or a documented equivalent key;
   - emit after the debounce window expires;
   - preserve plugin identity, operation id/resource ref, scoped relative path, and `WatchChangeKind`;
   - surface backend overflow as `WatchChangeKind::Overflow` without treating it as a successful path-specific change.
6. Own cleanup on unregister/unload/reload:
   - unregister one watch resource;
   - `cleanup_plugin(plugin_key)` releases only that plugin's watch resources and returns `PluginCleanupResult`;
   - cleanup emits or can drain `CapabilityRuntimeEvent::CleanupCompleted`;
   - release paths call the injected source so real host adapters can stop OS watcher tasks before runtime shutdown.
7. Re-export only the public types hosts/tests need from `runtime/mod.rs` and crate root, following the existing `PluginCapabilityRuntime` export style.

## Non-Scope

- No concrete `notify`/FSEvents/kqueue/polling watcher dependency in `botster-core`.
- No hub/profile policy for which directories to watch, default grants, root resolution, symlink handling, gitignore traversal, credentials, retries, or quotas beyond explicit capacity/debounce config.
- No Lua primitive, MCP tool, Project Pipelines policy, SPA/TUI UI, or Botster hub integration.
- No broad rewrite of `PluginWorkerEngine`, `CapabilityOperation`, or package manifest admission.
- No auto-detection/hot-reload policy; the historical dual-watcher note is context only, not a requirement to revive plugin source watching in core.

## Assumptions And Unknowns

- Assumption: this ticket expects a real core runtime over an injectable source, not another scaffold-only type document. Acceptance asks for fake watcher support and runtime behavior tests.
- Assumption: concrete OS watcher adapters remain host/profile-owned because Botster core is currently dependency-light and policy-free.
- Assumption: filesystem capability scope strings from PR #58 are the right grant vocabulary; do not introduce a parallel path-grant type unless the existing `Capability { surface, scope }` proves insufficient.
- Assumption: `QueueSource::PluginWorker` remains the pressure source for watch runtime mailboxes, matching the dependency architecture.
- Unknown: exact debounce default. Implement should choose a small explicit default (for example 25-50ms) and make tests deterministic through injected timestamps rather than sleeping.
- Unknown: whether event coalescing should merge create+modify+remove into the last kind or preserve an overflow/remove priority. The implementation must document the chosen deterministic rule and test it.
- Worktree assumption: implementation should start from post-PR #58 `main` (`1cb26b0` or newer). If local `.gitignore` churn still blocks a fast-forward and cannot be safely classified as generated setup state, ask before editing.

## Affected Surfaces / Files

- `crates/botster-core/src/runtime/capability.rs`: existing request/event/error/trait types; add only small helper methods if needed.
- `crates/botster-core/src/runtime/file_watch.rs` or a focused submodule under `runtime/capability/`: core watch runtime, config, event-source trait, registration state, debounce/coalescing, cleanup.
- `crates/botster-core/src/runtime/mod.rs` and `crates/botster-core/src/lib.rs`: narrow re-exports for the new runtime/testable public contracts.
- `crates/botster-core-test-support/src/fake/...`: reusable fake watcher event source if the fake is useful outside one test file; otherwise keep the fake private to the focused test.
- `crates/botster-core/tests/plugin_file_watch_runtime_test.rs`: focused runtime tests for grants, path validation, debounce/coalescing, bounded noisy delivery, cleanup, fake source support, and hot-path isolation.
- Possibly `crates/botster-core/tests/plugin_capability_runtime_test.rs`: only if existing scaffold tests need one assertion updated for new runtime helpers.
- `docs/architecture/non-blocking-plugin-capability-runtime.md`: short follow-up section saying the watch family now has a core runtime over host-provided event sources, while OS adapters remain host/profile-owned.

## Risks

- Accidentally moving directory/root policy into core. Core should validate scope ids and scoped relative path shape, not resolve host paths or decide allowed directories.
- Shipping an unwired public type. Every new public type must be used by the runtime, test support, or documented host adapter boundary.
- Type-existence tests can pass while runtime behavior is not proven. Tests must instantiate the runtime and drive accepted/denied/noisy/cleanup paths.
- Debounce tests can become flaky if they sleep on wall time. Prefer explicit timestamps or an injected clock.
- Bounded delivery can accidentally block if implemented with blocking sends. Use try-send/non-blocking semantics or bounded in-memory accounting that rejects immediately.
- Cleanup can leak event-source registrations, which maps directly to the watcher shutdown gotcha in the vault. Tests must assert source-side unregister/cleanup calls.
- Public enum churn can break downstream exhaustive matches. Avoid new enum variants unless necessary; current PR #58 already added the needed watch resource/capability scaffolding.

## Acceptance Checks / Tests

- `cargo fmt --all -- --check`
- `BOTSTER_ENV=test cargo test -p botster-core --test plugin_file_watch_runtime_test`
- `BOTSTER_ENV=test cargo test -p botster-core --test plugin_capability_runtime_test --test plugin_worker_engine_test`
- `BOTSTER_ENV=test cargo test -p botster-core`
- `BOTSTER_ENV=test cargo clippy -p botster-core --all-targets --all-features -- -D warnings`
- `BOTSTER_ENV=test cargo doc -p botster-core --no-deps --all-features`
- PII/local-path scan over committed diff before review; fake absolute paths are acceptable only when clearly synthetic test fixtures.

Required focused assertions:

- Allowed registration returns a `CapabilityRuntimeHandle` and source registration for matching `Filesystem` scope grant.
- Missing/wrong scope grant is rejected before source registration.
- Absolute, backslash-absolute, empty, and `..` scoped paths are rejected before source registration.
- Callback handler for another plugin is rejected.
- Multiple noisy events within the debounce window drain to the documented coalesced event after the window.
- Event queue saturation produces typed backpressure with `QueueSource::PluginWorker` and the owning `PluginKey`, without unbounded growth.
- `unregister` releases one watch; `cleanup_plugin` releases all and only the target plugin's watches and returns `PluginCleanupResult` with `PluginResourceKind::Watch`.
- Saturating the file-watch runtime does not block an unrelated `PluginWorkerEngine` invocation and does not produce `SessionIo` or `ClientWorker` pressure.

## Vault Gaps Worth Capturing

- No new vault capture needed at plan time. Existing notes already cover worker-owned file-watch callbacks, watcher shutdown cleanup, per-plugin worker boundaries, and core-vs-host policy split.
- Capture later only if implementation establishes a reusable `FileWatchRuntime` pattern that should constrain future host-profile adapters beyond this ticket.

## Advancement Target

Submit `botster_plan_gate` with this plan and advance to `botster_plan_review`.
