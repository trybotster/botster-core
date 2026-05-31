# Add Default Local PTY Process Runtime

Ticket: `ticket_1780189402_540507`
Run: `run_1780189443_396097`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Add default local PTY process runtime to botster-core`, current step `botster_plan`, gate `botster_plan_gate`.
- Prior Plan Review loaded: `review_1780190794_938873` returned changes required. This revision addresses the open findings about runtime-path proof, the separate `SessionRuntime` and `SessionWorkerRuntime` contracts, `ResizePayload` naming, and shutdown cleanup.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster/vault context loaded:
  - `identity`
  - `goals`
  - `botster-architecture`
  - `cli-patterns`
  - `sessionioworker is the production read path for session pty output`
  - `portable_pty MasterPty is private bypass with ioctl and raw fd`
  - `pty master fd close sends sighup but ignores it needs killpg`
  - `botster runtime now uses broker-authoritative pty lifecycle with unified session registry`
  - `pty-spawn-prepends-binary-directory-to-path`
- Repo context inspected:
  - `README.md`: `botster-core` owns reusable mechanisms and policy-free contracts; hosts own executable choice, auth, persistence, concrete transport delivery, and product workflows.
  - `crates/botster-core/src/runtime/mod.rs`: current `SessionRuntime` trait and `SessionSpawnRequest` contract are host-provided only.
  - `crates/botster-core/src/engine/session_worker.rs`: `SessionWorkerRuntime` is a separate engine I/O adapter trait for write, resize, snapshot, mode/screen helpers, send-file preparation, color profile, and shutdown.
  - `crates/botster-core/src/engine/multiplexer.rs`: public facade uses `SessionRuntime` only for `spawn_session`; terminal I/O and session events route through the per-session `SessionWorkerRuntime` and explicit `handle_runtime_event`.
  - `crates/botster-core/tests/session_runtime_contract_test.rs`: current tests prove the trait with `FakeSessionRuntime`, not a real local process runtime.
  - `crates/botster-core-test-support/src/fake/session_worker.rs`: fake worker/runtime patterns for targeted acceptance tests.
  - Prior plan artifacts: `core-session-worker-engine.md` and `assemble-core-multiplexer-engine-api.md`.
- Dependency check:
  - `portable-pty` latest release verified as `0.9.0` on docs.rs/crates.io search. It is maintained enough for evaluation and matches existing Botster vault notes about its API boundaries.

## Scope

Add a reusable default local PTY/process runtime inside `botster-core` so embedders can get a working local session engine without implementing PTY management themselves.

In scope:

- Add a concrete public local runtime adapter, likely `LocalProcessRuntime`, under `crates/botster-core/src/runtime/`.
- Implement both existing local-session contracts:
  - `SessionRuntime` for explicit spawn, direct input, direct resize/shutdown, and direct output draining.
  - `SessionWorkerRuntime` for the session worker engine's input, resize, snapshot, helper, and shutdown calls.
- Prefer a cloneable/shared-state adapter shape so embedders can use the same default runtime as both `MultiplexerEngine`'s `SessionRuntime` and its per-session `SessionWorkerRuntime` without implementing PTY management.
- Spawn the requested executable with the exact `SessionSpawnRequest` command, arguments, working directory, explicit environment variables, and optional `ResizePayload`.
- Use a maintained Rust PTY/process primitive when it fits; `portable-pty 0.9.0` is the leading candidate.
- Manage runtime-owned session state: child identity, output reader, input writer, resize handle, exit reporting, and shutdown cleanup.
- Drain available output as `SessionRuntimeOutput::PtyOutput` and child exit as `SessionRuntimeOutput::ProcessExited`.
- Deliver `SessionRuntimeInput::PtyInput`, `Resize`, and `Shutdown` through the concrete runtime.
- Map `SessionWorkerRuntime::write_input`, `resize`, `snapshot`, `request_initial_snapshot`, `mode_flags`, `screen`, `set_color_profile`, and `shutdown` onto the same local PTY/process state where the selected primitive supports the operation. For helper requests that cannot be meaningfully supported by a raw PTY primitive yet, return deterministic policy-free placeholder data or typed failure events already modeled by the contract, and document the limitation in tests.
- Return typed `SessionRuntimeErrorKind::SpawnFailed`, `SessionNotFound`, `InputFailed`, or `OutputFailed` for failure paths.
- Add platform-gated tests where needed to prove real local spawn, output drain, input write where applicable, resize contract where supported, spawn failure reporting, shutdown cleanup, and no private user paths or PII in docs/fixtures.
- Re-export the new runtime type from `runtime/mod.rs` and `lib.rs` if it is intended as public embedder API.
- Update README narrowly to state that core now includes an optional/default local runtime adapter while hosts still own policy and request construction.

Non-scope:

- No default command, shell selection, PATH mutation, product config discovery, target admission, auth, cloud, Rails, WebRTC, TUI, marketplace, Project Pipelines, or Lua plugin behavior.
- No hub recovery, broker persistence, session manifest policy, reconnect policy, retention policy, or process-freezing behavior.
- No broad rewrite of `SessionRuntime`, `SessionWorkerEngine`, `MultiplexerEngine`, or test-support fakes. Small accessors/helpers are allowed only if needed to wire the default adapter into existing public contracts.
- No compatibility branch or version-suffixed duplicate API.
- No wholesale port of old TryBotster hub/session process code.

Botster layers touched:

- Rust `botster-core` runtime layer: primary surface.
- Rust `botster-core` session worker adapter surface: implement the existing `SessionWorkerRuntime` contract for the local runtime; do not change session worker policy.
- Rust `botster-core` public exports and tests.
- Rust `botster-core` docs/README only for the public boundary.
- No plugin, SPA, TUI, Rails relay, MCP, provider, or product workflow layer changes.

Worktree/target assumption: implementers work in the pipeline-provided `botster-core` worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts: this file is the Plan artifact. Gate evidence should cite this file and the run checklist.

## Assumptions And Unknowns

Assumptions:

- The ticket intentionally changes core from "trait only" to "trait plus one default local implementation" for both the spawn/runtime trait and the session worker runtime adapter.
- A concrete PTY/process primitive in core does not violate the core boundary if it remains policy-free and only executes explicit spawn requests.
- The existing synchronous `SessionRuntime` trait is sufficient for direct trait-level use if the concrete runtime owns background reader/exit plumbing internally and exposes available data through `drain_output`.
- The existing `SessionWorkerRuntime` trait remains the engine data-plane adapter. A default local runtime must implement it too, or embedders still need to write PTY management for the assembled engine path.
- `MultiplexerEngine<LocalProcessRuntime, LocalProcessRuntime>` can honestly prove spawn-path compatibility and worker-command compatibility, but not automatic output fanout unless the test explicitly drains local runtime output and feeds converted `SessionWorkerRuntimeEvent` values into `handle_runtime_event`.
- `portable-pty` should be evaluated first because it provides a cross-platform PTY API and is the latest known maintained fit for this layer.
- Tests may need platform gates. Unix can prove PTY echo/input and resize more directly; Windows behavior should be included if the selected primitive supports it cleanly.
- Spawn requests should use synthetic paths and commands only, such as `sh`, `printf`, `cat`, or Rust test binaries where platform appropriate. Do not include private home paths.
- Environment handling remains set-vars only, matching `SpawnEnvironment`; ambient inheritance policy is still host-owned.

Unknowns for implementation:

- Whether `portable-pty` exposes a stable enough child exit/status API for nonblocking `drain_output`, or whether exit reporting needs a small runtime-owned waiter thread.
- Whether resize is fully portable through the selected primitive on every test platform; if not, gate the resize proof to supported platforms and document unsupported behavior as a typed error or no-op only if the primitive requires it. The contract type is `ResizePayload`; when using `portable-pty`, map `rows`/`cols` to `portable_pty::PtySize` and set pixel dimensions to zero unless the crate requires better values.
- Whether process identity should set `pid`, `runtime_id`, or both. Prefer OS PID when the primitive exposes it, with a synthetic runtime id only when useful for runtime bookkeeping.
- Shutdown cleanup must be concrete, not "drop the PTY and hope." Plan for direct child termination on all platforms supported by the primitive, plus Unix process-group termination where the selected primitive exposes or permits it without fragile hand-rolled PTY setup. If process-group cleanup cannot be implemented cleanly in this ticket, document it as residual risk and still assert the directly spawned child is gone in tests.
- Whether the runtime module should be named `local_process`, `local_pty`, or `process`. Prefer names that describe the mechanism without implying product policy.

No human question is blocking. The ticket intent is clear and can be satisfied without waiving scope.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/Cargo.toml`
  - Add the selected maintained runtime dependency, likely `portable-pty = "0.9.0"`.
  - Add small platform support dependencies only if required by the selected primitive and tests.
- `crates/botster-core/src/runtime/mod.rs`
  - Export the new concrete runtime module/type.
- `crates/botster-core/src/runtime/local_process.rs` or `local_pty.rs`
  - Implement `LocalProcessRuntime`, shared per-session state, `SessionRuntime`, `SessionWorkerRuntime`, spawn, input, resize, snapshots/helpers where supported, drain, exit, and shutdown.
- `crates/botster-core/src/lib.rs`
  - Re-export public concrete runtime type and any narrow config/error helpers if needed.
- `crates/botster-core/tests/local_process_runtime_test.rs`
  - Real local runtime acceptance tests for direct `SessionRuntime` and `SessionWorkerRuntime` behavior.
- `crates/botster-core/tests/session_runtime_contract_test.rs`
  - Possible small source guard update so the concrete runtime remains free of product/private-path terms.
- `README.md`
  - Narrow boundary update: core has a default local runtime adapter, while hosts still construct explicit spawn requests and own policy.
- `docs/plans/default-local-pty-process-runtime.md`
  - This plan artifact.

Possible but avoid unless necessary:

- `crates/botster-core/src/engine/multiplexer.rs`
  - Only if a tiny accessor/helper is necessary. Tests may instantiate `MultiplexerEngine<LocalProcessRuntime, LocalProcessRuntime>` without changing this file.
- `crates/botster-core-test-support`
  - Only if reusable conformance helpers are needed. Real runtime tests can probably live directly in `botster-core/tests`.

Not expected:

- `crates/botster-core-dev`.
- `crates/botster-core/src/engine/session_worker.rs`, except for compiler-required narrow helper accessors. The local runtime should adapt to the existing trait.
- `crates/botster-core/src/contract/*`.
- Any hub, CLI, browser, TUI, Rails, Lua plugin, MCP, Project Pipelines, provider, or old TryBotster files.

## Implementation Shape

Suggested minimal API:

- `LocalProcessRuntime::new() -> Self`
- `impl Default for LocalProcessRuntime`
- `impl Clone for LocalProcessRuntime` if shared state is used so the same adapter can be installed as the session runtime and per-session worker runtime.
- `impl SessionRuntime for LocalProcessRuntime`
- `impl SessionWorkerRuntime for LocalProcessRuntime`
- Internal `LocalSession` state keyed by `SessionId`:
  - PTY writer/master handle needed for input and resize
  - reader thread or nonblocking reader channel for output bytes
  - child/waiter state for process exit
  - shutdown flag and last known exit
  - latest terminal size for snapshot/helper responses when the raw PTY primitive cannot provide a richer parser-backed snapshot

Behavior details:

- `spawn_session(request)`:
  - Validate only mechanics required to spawn; do not decide command policy.
  - Build the command from `request.executable` and `request.arguments`.
  - Set cwd from `request.working_directory.path`.
  - Apply each explicit env variable from `request.environment.variables`.
  - Use `request.initial_pty_size` when provided, otherwise select the primitive's default terminal size.
  - Start a reader path that records available PTY output for `drain_output`.
  - Return `SessionRuntimeHandle` with the request/session ids and available process identity.
- `send_input(PtyInput)` writes exact bytes.
- `send_input(Resize)` applies rows/cols through the PTY primitive.
- `send_input(Shutdown)` terminates the runtime-owned child/session and records a process exit if available.
- `drain_output(session_id)` returns all currently buffered output plus any newly observed exit event, then removes drained items.
- `SessionWorkerRuntime::write_input` writes exact bytes to the same PTY input path.
- `SessionWorkerRuntime::resize` applies rows/cols to the same PTY resize path and records the current `ResizePayload`.
- `SessionWorkerRuntime::snapshot` and `request_initial_snapshot` should return the best contract-compliant local snapshot available without adding a terminal parser. A raw PTY runtime may return currently buffered bytes or an empty/synthetic snapshot with the requested rows/cols, but tests must document what is proven.
- Provide a small conversion helper only if useful, such as `LocalProcessRuntime::drain_worker_events(session_id, last_output_at) -> Result<Vec<SessionWorkerRuntimeEvent>, SessionRuntimeError>`, so embedders can poll the local runtime and feed `MultiplexerEngine::handle_runtime_event`. This helper must stay mechanical and policy-free.
- Unknown sessions return `SessionRuntimeErrorKind::SessionNotFound`.
- Spawn failures include the failed executable name but must not include private absolute paths unless they came from the explicit request and tests use synthetic values.

## Risks

- Adding executable discovery, default shells, PATH mutation, config lookup, or target admission would turn a reusable runtime into product policy.
- A concrete runtime that is only exported but not driven by tests would fail the runtime-path proof requirement.
- Blocking reads in `drain_output` can hang hosts or tests. Output collection should be backgrounded or nonblocking.
- Shutdown can leak children if it relies only on dropping PTY handles. The implementation should include direct child cleanup and consider process-group cleanup where supported.
- `portable-pty` may not expose every needed operation uniformly across platforms. Keep the adapter narrow and gate tests rather than hand-rolling fragile OS PTY code.
- Exit reporting can race with final output. Tests should assert output can be drained before or with process exit for simple commands.
- A `MultiplexerEngine` test can overclaim if it skips the real local runtime output path. Engine proof must distinguish spawn/worker-command wiring from explicit polling and `handle_runtime_event` delivery.
- Snapshot helper behavior can be mistaken for a real terminal parser. The default local runtime should not claim parser-backed snapshots unless it actually owns one.
- Public docs can overclaim production hub wiring. The README should describe the core adapter, not claim Botster hub integration.
- Adding dependencies without version verification would violate local dependency discipline. The latest known `portable-pty` version has been checked.

## Acceptance Checks / Tests

Required targeted tests:

1. `local_process_runtime_spawns_simple_command_and_drains_output`
   - Spawn a synthetic command that writes a known marker.
   - Poll/drain with a bounded timeout.
   - Assert `PtyOutput` contains the marker and `ProcessExited` is eventually reported.

2. `local_process_runtime_writes_input_to_pty`
   - Spawn an echoing command such as `cat` or a platform-appropriate test helper.
   - Send `SessionRuntimeInput::PtyInput`.
   - Assert drained output includes the sent bytes where applicable.

3. `local_process_runtime_resizes_pty_when_supported`
   - Spawn a command that reports terminal size or assert the primitive resize call succeeds through the runtime.
   - Use the real contract name `ResizePayload`; assert mapping to the selected primitive's rows/cols shape. For `portable-pty`, rows/cols map to `PtySize` with zero pixel dimensions.
   - Gate by platform/primitive capability if necessary.

4. `local_process_runtime_reports_spawn_failure`
   - Spawn a definitely missing synthetic executable.
   - Assert `SessionRuntimeErrorKind::SpawnFailed`.

5. `local_process_runtime_reports_session_not_found`
   - Send input and drain output for an unknown session.
   - Assert `SessionRuntimeErrorKind::SessionNotFound`.

6. `local_process_runtime_shutdown_cleans_up_child`
   - Spawn a long-running command, send `Shutdown`, and assert the directly spawned child exits within a bounded timeout.
   - On Unix, assert process-group cleanup when implemented; if not implemented, explicitly document process-group cleanup as residual risk and still prove no direct child leak.

7. `local_process_runtime_satisfies_session_worker_runtime_contract`
   - Instantiate `SessionWorkerEngine<LocalProcessRuntime>` or call the `SessionWorkerRuntime` trait through the public engine.
   - Prove `PtyInput`, `Resize`, `GetSnapshot` or `GetInitialSnapshot`, and `Shutdown` route to the real local runtime adapter, not a fake worker.

8. `local_process_runtime_can_be_used_through_public_session_runtime_trait`
   - Box it as `Box<dyn SessionRuntime>` and prove direct public trait use.
   - If a `MultiplexerEngine<LocalProcessRuntime, LocalProcessRuntime>` test is added, state exactly what it proves: spawn goes through `SessionRuntime`, client input/resize requests go through the local `SessionWorkerRuntime`, and output fanout requires explicitly draining local runtime output and feeding `SessionWorkerRuntimeEvent` into the engine.

9. `local_process_runtime_output_can_be_fed_into_multiplexer_when_polled`
   - Optional if the helper shape is small: spawn through `MultiplexerEngine<LocalProcessRuntime, LocalProcessRuntime>`, subscribe a client, drain local runtime output into `SessionWorkerRuntimeEvent`, call `handle_runtime_event`, and assert client egress.
   - This test must not imply the engine automatically calls `SessionRuntime::drain_output`; polling remains host-owned.

10. `runtime_docs_and_tests_do_not_embed_private_paths_or_pii`
   - Source/fixture guard for private home paths, user names, or product-only terms in the new runtime tests/docs.

Commands:

- `cargo fmt`
- `cargo test -p botster-core local_process_runtime`
- `cargo test -p botster-core session_runtime`
- `cargo test -p botster-core`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Runtime/user path proof:

- Implementation must show the production-facing public core path changes from trait-only to real embeddable local runtime.
- Evidence should include tests importing `botster_core::LocalProcessRuntime` or its final public name and driving `SessionRuntime::spawn_session`, `send_input`, and `drain_output`.
- Evidence must also show the same default runtime, or a sibling public adapter backed by the same local state, satisfying `SessionWorkerRuntime` through `SessionWorkerEngine`.
- If a `MultiplexerEngine` test is practical, it must be framed precisely:
  - It can prove `spawn_session` uses the local `SessionRuntime`.
  - It can prove client input/resize requests use the local `SessionWorkerRuntime`.
  - It can prove output fanout only if the test explicitly drains local runtime output and passes converted `SessionWorkerRuntimeEvent` values to `handle_runtime_event`.
  - It must not claim that `MultiplexerEngine` automatically calls `SessionRuntime::send_input` or `drain_output`; the repo currently does not have that path.

## Vault Gaps Worth Capturing

Potential capture after implementation:

- A durable Botster note for the selected default local runtime primitive and any API gotchas discovered during implementation.
- A durable convention for when `botster-core` may include concrete mechanism adapters despite generally avoiding host policy.
- A process cleanup note if the implementation settles cross-platform direct-child or process-group shutdown behavior in core.

No pre-implementation capture is required. Existing vault notes already constrain the main boundary: reusable core mechanism is allowed, product policy is not.

## Vault Checklist Evidence

- Vault/project notes constrained the plan: `planner-playbook`, `botster-planner-playbook`, `identity`, `goals`, `botster-architecture`, `cli-patterns`, `sessionioworker is the production read path for session pty output`, `portable_pty MasterPty is private bypass with ioctl and raw fd`, `pty master fd close sends sighup but ignores it needs killpg`, `botster runtime now uses broker-authoritative pty lifecycle with unified session registry`, and `pty-spawn-prepends-binary-directory-to-path`.
- Convention conflicts: none. The plan adds a concrete mechanism adapter but keeps product/runtime policy outside core and requires explicit spawn requests.
- Verification evidence so far: planning inspection only; no implementation tests were run in the Plan step. Planned verification commands are listed above.
- Durable knowledge capture: no capture before implementation. Capture after implementation if the selected primitive or cleanup behavior creates a reusable Botster convention.
