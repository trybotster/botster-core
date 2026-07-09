# Add Default Local PTY Process Runtime

Ticket: `ticket_1780189402_540507`
Run: `run_1780189443_396097`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Add default local PTY process runtime to botster-core`, current step `botster_plan`, gate `botster_plan_gate`, prior artifacts, reviews, findings, and the human answer to `question_1780191167_662553`.
- Current return reason loaded: GitHub review on PR #26 requested changes with body "Please fix merge conflict." `gh pr view 26` reports head `project-pipelines/ticket_1780189402_540507`, base `main`, mergeable `CONFLICTING`, review decision `CHANGES_REQUESTED`.
- Human scope decision loaded: choose option A for this ticket. Implement the default local PTY/process runtime at the `SessionRuntime` boundary only. Do not expand this ticket into a full `SessionWorkerRuntime` bridge or terminal-emulator grid.
- Latest Plan Review loaded: `review_1780191548_622010` returned changes required because the prior revision chose option B. This revision re-scopes to option A.
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
  - `crates/botster-core/src/runtime/mod.rs`: `SessionRuntime` is the spawn/send-input/drain-output boundary this ticket targets.
  - `crates/botster-core/src/engine/session_worker.rs`: `SessionWorkerRuntime` is a separate session worker data-plane adapter and is now explicitly out of scope for this ticket.
  - `crates/botster-core/src/engine/multiplexer.rs`: `MultiplexerEngine` calls `SessionRuntime::spawn_session` only; it does not call `send_input` or `drain_output`.
  - `crates/botster-core/tests/session_runtime_contract_test.rs`: current tests prove the trait with `FakeSessionRuntime`, not a real local process runtime.
  - Prior plan artifacts: `core-session-worker-engine.md`, `assemble-core-multiplexer-engine-api.md`, and prior revisions of this plan.
- Dependency check:
  - `portable-pty` latest release verified as `0.9.0` on docs.rs/crates.io search. It remains the leading maintained, policy-free Rust PTY/process primitive to evaluate.

## Scope

Add a reusable default local PTY/process runtime inside `botster-core` at the `SessionRuntime` boundary so embedders can spawn and drive a local PTY session directly without implementing PTY management.

In scope:

- Add a concrete public runtime adapter, likely `LocalProcessRuntime`, under `crates/botster-core/src/runtime/`.
- Implement the existing `SessionRuntime` trait only:
  - `spawn_session`
  - `send_input`
  - `drain_output`
- Spawn the requested executable with the exact `SessionSpawnRequest` command, arguments, working directory, explicit environment variables, and optional `ResizePayload`.
- Use or evaluate a maintained Rust PTY/process primitive when it fits; `portable-pty 0.9.0` is the leading candidate.
- Manage runtime-owned session state: child identity, output reader, input writer, resize handle, exit reporting, and shutdown cleanup.
- Drain available output as `SessionRuntimeOutput::PtyOutput` and child exit as `SessionRuntimeOutput::ProcessExited`.
- Deliver `SessionRuntimeInput::PtyInput`, `Resize`, and `Shutdown` through the concrete runtime.
- Return typed `SessionRuntimeErrorKind::SpawnFailed`, `SessionNotFound`, `InputFailed`, or `OutputFailed` for failure paths.
- Add platform-gated tests where needed to prove real local spawn, output read, input write where applicable, resize contract where supported, spawn failure reporting, shutdown cleanup, direct public trait use, and no private user paths or PII in docs/fixtures.
- Re-export the new runtime type from `runtime/mod.rs` and `lib.rs` if it is intended as public embedder API.
- Update README narrowly to state that core now includes a policy-free local `SessionRuntime` implementation while hosts still construct explicit spawn requests and own product policy.
- Small adapter seams/types are allowed only if they avoid painting the future `SessionWorkerRuntime`/`MultiplexerEngine` bridge into a corner. They must not implement the bridge in this ticket.

Non-scope:

- No `impl SessionWorkerRuntime for LocalProcessRuntime`.
- No `SessionWorkerEngine<LocalProcessRuntime>` behavior tests.
- No `MultiplexerEngine` end-to-end terminal fanout test.
- No terminal-emulator grid, parser-backed snapshot, `screen`, `mode_flags`, or snapshot helper behavior.
- No Ghostty backend dependency or trybotster-owned terminal parser fork.
  Terminal backend work belongs to the separate terminal screen/snapshot/parser
  path. No restty client-renderer dependency; restty remains client rendering
  only.
- No default command, shell selection, PATH mutation, product config discovery, target admission, auth, cloud, Rails, WebRTC, TUI, marketplace, Project Pipelines, or Lua plugin behavior.
- No hub recovery, broker persistence, session manifest policy, reconnect policy, retention policy, or process-freezing behavior.
- No broad rewrite of `SessionRuntime`, `SessionWorkerEngine`, `MultiplexerEngine`, or test-support fakes.
- No compatibility branch or version-suffixed duplicate API.
- No wholesale port of old TryBotster hub/session process code.

Botster layers touched:

- Rust `botster-core` runtime layer: primary surface.
- Rust `botster-core` public exports and tests.
- Rust `botster-core` docs/README only for the public `SessionRuntime` boundary.
- No session worker, plugin, SPA, TUI, Rails relay, MCP, provider, or product workflow layer changes.

Worktree/target assumption: implementers work in the pipeline-provided `botster-core` worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts: this file is the revised Plan artifact. Gate evidence should cite this file, the human answer, and the run checklist.

## PR Conflict Return Plan

The feature plan above remains the approved implementation scope. The current returned Plan step is a narrow PR maintenance pass:

- Update the existing PR #26 branch `project-pipelines/ticket_1780189402_540507`; do not create a new branch, run, or PR.
- Bring the branch current with `main` using the repo's normal Git workflow.
- Resolve merge conflicts surgically while preserving the approved SessionRuntime-only scope.
- Do not reintroduce `SessionWorkerRuntime`, `SessionWorkerEngine` behavior
  tests, `MultiplexerEngine` fanout tests, Ghostty backend work,
  terminal-grid/parser work, shell/default command policy, PATH mutation, or
  product configuration discovery. Do not add restty client-renderer work.
- Carry the open low-severity review finding `finding_1780193017_482511`: either resolve it by ensuring final PTY bytes cannot be dropped when exit wins the race against reader-channel drain, or document it exactly as residual risk in implementation gate evidence if left unchanged.
- Push the updated existing branch so PR #26 becomes mergeable again.
- Reply or otherwise satisfy the GitHub review by updating the PR branch; no separate PR is needed.

Conflict-pass verification:

- `git status --short --branch` must show a clean branch after conflict resolution.
- `gh pr view 26 --json mergeable,reviewDecision,statusCheckRollup` should no longer report `CONFLICTING`.
- Re-run the approved verification for this ticket after conflict resolution:
  - `cargo fmt`
  - `cargo test -p botster-core local_process_runtime`
  - `cargo test -p botster-core session_runtime`
  - `cargo test -p botster-core`
  - `cargo test`
  - `cargo clippy --all-targets --all-features -- -D warnings`

## Assumptions And Unknowns

Assumptions:

- The human answer to `question_1780191167_662553` is authoritative for this ticket.
- The ticket acceptance criteria map to `SessionRuntime`: spawn, output read, input write, resize, spawn failure, typed errors, and no PII.
- The future `SessionWorkerRuntime` bridge, supervised session task runtime, ergonomic `BotsterEngine` API, and default-runtime integration are separate tickets.
- A concrete PTY/process primitive in core does not violate the core boundary if it remains policy-free and only executes explicit spawn requests.
- The existing synchronous `SessionRuntime` trait is sufficient if the concrete runtime owns background reader/exit plumbing internally and exposes currently available data through `drain_output`.
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

No human question remains blocking. The prior ambiguity was answered with option A.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/Cargo.toml`
  - Add the selected maintained runtime dependency, likely `portable-pty = "0.9.0"`.
  - Add small platform support dependencies only if required by the selected primitive and tests.
- `crates/botster-core/src/runtime/mod.rs`
  - Export the new concrete runtime module/type.
- `crates/botster-core/src/runtime/local_process.rs` or `local_pty.rs`
  - Implement `LocalProcessRuntime`, per-session state, `SessionRuntime`, spawn, input, resize, drain, exit, and shutdown.
- `crates/botster-core/src/lib.rs`
  - Re-export public concrete runtime type and any narrow config/error helpers if needed.
- `crates/botster-core/tests/local_process_runtime_test.rs`
  - Real local runtime acceptance tests for direct `SessionRuntime` behavior.
- `crates/botster-core/tests/session_runtime_contract_test.rs`
  - Possible small source guard update so the concrete runtime remains free of product/private-path terms.
- `README.md`
  - Narrow boundary update: core has a default local `SessionRuntime` adapter, while hosts still construct explicit spawn requests and own policy.
- `docs/archive/plans/default-local-pty-process-runtime.md`
  - This revised plan artifact.

Possible but avoid unless necessary:

- `crates/botster-core-test-support`
  - Only if a tiny reusable conformance helper is genuinely useful. Real runtime tests can probably live directly in `botster-core/tests`.

Not expected:

- `crates/botster-core-dev`.
- `crates/botster-core/src/engine/session_worker.rs`.
- `crates/botster-core/src/engine/multiplexer.rs`.
- `crates/botster-core/src/contract/*`.
- Any Ghostty backend, restty client renderer, hub, CLI, browser, TUI, Rails, Lua plugin, MCP, Project Pipelines, provider, or old TryBotster files.

## Implementation Shape

Suggested minimal API:

- `LocalProcessRuntime::new() -> Self`
- `impl Default for LocalProcessRuntime`
- `impl SessionRuntime for LocalProcessRuntime`
- Internal `LocalSession` state keyed by `SessionId`:
  - PTY writer/master handle needed for input and resize
  - reader thread or nonblocking reader channel for output bytes
  - child/waiter state for process exit
  - shutdown flag and last known exit

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
- Unknown sessions return `SessionRuntimeErrorKind::SessionNotFound`.
- Spawn failures include the failed executable name but must not include private absolute paths unless they came from the explicit request and tests use synthetic values.

## Risks

- Adding executable discovery, default shells, PATH mutation, config lookup, or target admission would turn a reusable runtime into product policy.
- Accidentally retaining the prior B-scope `SessionWorkerRuntime` bridge would violate the human decision and broaden this ticket.
- A concrete runtime that is only exported but not driven by tests would fail the runtime-path proof requirement.
- Blocking reads in `drain_output` can hang hosts or tests. Output collection should be backgrounded or nonblocking.
- Shutdown can leak children if it relies only on dropping PTY handles. The implementation should include direct child cleanup and consider process-group cleanup where supported.
- `portable-pty` may not expose every needed operation uniformly across platforms. Keep the adapter narrow and gate tests rather than hand-rolling fragile OS PTY code.
- Exit reporting can race with final output. Tests should assert output can be drained before or with process exit for simple commands.
- Public docs can overclaim `MultiplexerEngine` or session-worker integration. The README should describe the `SessionRuntime` adapter only.
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

7. `local_process_runtime_can_be_used_through_public_session_runtime_trait`
   - Box it as `Box<dyn SessionRuntime>` or otherwise call it through the public trait.
   - Prove direct public trait use, not private helper methods.

8. `runtime_docs_and_tests_do_not_embed_private_paths_or_pii`
   - Source/fixture guard for private home paths, user names, or product-only terms in the new runtime tests/docs.

Explicitly dropped from this ticket:

- Any test proving `SessionWorkerEngine<LocalProcessRuntime>` routes PTY input/resize/snapshot/shutdown.
- Any `MultiplexerEngine` fanout test that requires `SessionWorkerRuntime` or `handle_runtime_event`.
- Any Ghostty backend or terminal-grid snapshot/parser test.
- Any restty client-renderer test.

Commands:

- `cargo fmt`
- `cargo test -p botster-core local_process_runtime`
- `cargo test -p botster-core session_runtime`
- `cargo test -p botster-core`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Runtime/user path proof:

- Implementation must show the production-facing public core path changes from trait-only to a real embeddable local `SessionRuntime`.
- Evidence should include tests importing `botster_core::LocalProcessRuntime` or its final public name and driving `SessionRuntime::spawn_session`, `send_input`, and `drain_output` against a real local child.
- Evidence must not claim `MultiplexerEngine` end-to-end terminal I/O changed in this ticket. That integration is intentionally deferred by the human answer.

## Vault Gaps Worth Capturing

Potential capture after implementation:

- A durable Botster note for the selected default local runtime primitive and any API gotchas discovered during implementation.
- A durable convention for when `botster-core` may include concrete mechanism adapters despite generally avoiding host policy.
- A process cleanup note if the implementation settles cross-platform direct-child or process-group shutdown behavior in core.

No pre-implementation capture is required. Existing vault notes already constrain the main boundary: reusable core mechanism is allowed, product policy is not.

## Vault Checklist Evidence

- Vault/project notes constrained the plan: `planner-playbook`, `botster-planner-playbook`, `identity`, `goals`, `botster-architecture`, `cli-patterns`, `sessionioworker is the production read path for session pty output`, `portable_pty MasterPty is private bypass with ioctl and raw fd`, `pty master fd close sends sighup but ignores it needs killpg`, `botster runtime now uses broker-authoritative pty lifecycle with unified session registry`, and `pty-spawn-prepends-binary-directory-to-path`.
- Human decision constrained the plan: option A from `question_1780191167_662553`, `SessionRuntime` only for this ticket.
- Convention conflicts: none after re-scope. The plan adds a concrete mechanism adapter but keeps product/runtime policy outside core and requires explicit spawn requests.
- Verification evidence so far: planning inspection only; no implementation tests were run in the Plan step. Planned verification commands are listed above.
- Durable knowledge capture: no capture before implementation. Capture after implementation if the selected primitive or cleanup behavior creates a reusable Botster convention.
