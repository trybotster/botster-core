# Typed Engine Command API

Ticket: `ticket_1780348077_294552`
Run: `run_1780350440_426368`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Implement typed engine command API over public core engine`, run `run_1780350440_426368`, current step `botster_plan`, run step `run_step_1780350440_368211`, gate `botster_plan_gate`, dependency on closed ticket `ticket_1780348077_351705`.
- Base dependency context loaded from `run_1780348098_553098`: PR #44 merged at `c7569a944da86aef6c6fde4740e2e1ab256514a1`, defining the command surface vocabulary, architecture note, facade helpers, and tests.
- Project Pipelines checklist discipline attempted before planning. Creating the run vault checklist failed with a plugin SQLite `database is locked` error; gate evidence should record the attempted checklist creation and any successful retry.
- Required playbooks loaded:
  - [[planner-playbook]]
  - [[botster-planner-playbook]]
- Required Botster overlay notes loaded:
  - [[botster-architecture]]
  - [[cli-patterns]]
  - [[spa-patterns]]
  - [[project pipeline orchestration belongs in a device-level botster plugin]]
  - [[project pipelines needs an operator workbench not more primitives]]
  - [[project pipelines ui contract belongs in the plugin readme]]
  - [[botster orchestration should spawn agents with explicit target ids]]
  - [[botster orchestration prompts must bind agents to explicit worktrees]]
- Additional constraining notes loaded:
  - [[plan steps need reviewable plan artifacts]]
  - [[botster engine command surface uses botsterengine as facade]]
  - [[identity]]
  - [[goals]]
- Repo context inspected:
  - `README.md`
  - `docs/architecture/engine-command-surface.md`
  - `docs/plans/engine-command-surface.md`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/engine/mod.rs`
  - `crates/botster-core/src/engine/command.rs`
  - `crates/botster-core/src/engine/botster.rs`
  - `crates/botster-core/src/engine/managed_session_runtime.rs`
  - `crates/botster-core/src/engine/multiplexer.rs`
  - `crates/botster-core/tests/botster_engine_api_test.rs`
  - `crates/botster-core/tests/managed_session_runtime_test.rs`
  - `crates/botster-core-dev/src/lib.rs`

## Scope

Implement a typed, policy-free command API over the public `BotsterEngine` facade and `DefaultBotsterEngine` local-runtime facade, using the command vocabulary defined by the dependency ticket.

In scope:

- Add concrete command request/result/error/event types in `crates/botster-core/src/engine/command.rs`.
- Keep command requests explicit and host-resolved: executable, args, cwd, env, pty size, session ids, client ids, subscription ids, request ids, timestamps, thresholds, and shutdown reasons are supplied by callers.
- Provide a typed dispatch path over `BotsterEngine<R, W>` for commands supported by custom host adapters:
  - spawn session;
  - attach client;
  - detach client;
  - send input;
  - resize;
  - list sessions;
  - inspect session lifecycle/activity;
  - read screen;
  - capture snapshot;
  - replay/prepare snapshot where the adapter supports it;
  - shutdown;
  - post and drain notifications.
- Provide the matching typed dispatch path over `DefaultBotsterEngine` where `local-runtime` is enabled.
- Reuse existing public facade methods (`spawn_session`, `attach_client`, `write_bytes`, `resize`, `list_sessions`, `inspect_session`, `read_screen`, `capture_snapshot`, `replay_snapshot`, `shutdown_session`, `post_notification`, `drain_notifications`) rather than reaching into private lower-level internals.
- Preserve typed errors with useful context by wrapping or aliasing existing `BotsterEngineError`, `DefaultBotsterEngineError`, and explicit unsupported-command cases. Avoid string-only or JSON error payloads.
- Add a drift guard tying `EngineCommandKind` to the new typed request dispatch so future vocabulary changes force code/test updates.
- Update `docs/architecture/engine-command-surface.md` and public rustdoc to describe the typed command API, not only the prior vocabulary aliases.
- Add focused fake-backed tests for every command shape and default-local tests where practical.
- Extend the dev smoke harness only if it remains small and proves the public typed command path without duplicating product wiring.

Non-scope:

- No hub, Rails, WebRTC, TUI, React SPA, restty, provider, cloud, auth, config discovery, marketplace/update policy, Project Pipelines runtime, or product CLI UX changes.
- No new async executor, scheduler, mailbox, queue topology, persistence, reconnect policy, default shell discovery, spawn target admission, or historical session browsing.
- No second engine implementation. The typed API must delegate to the public facades and existing runtime/session-worker/multiplexer paths.
- No broad refactors of `BotsterEngine`, `DefaultBotsterEngine`, `ManagedSessionRuntime`, `MultiplexerEngine`, terminal screen contracts, notification inbox, or runtime traits beyond small helpers required by typed command dispatch.
- No `BoundaryJson` escape hatch for stable engine commands.
- No fabricated support. If the default local runtime cannot replay snapshots, the typed command API should surface the existing typed unsupported error.
- No committed local absolute paths, usernames, terminal prompts, credentials, hostnames, or private terminal contents.

Botster layers touched:

- Rust `botster-core` public engine facade and command module: primary.
- Rust `botster-core` tests and doctests: primary.
- Rust `botster-core-dev` smoke harness: possible if used for real local-runtime proof.
- Docs: command surface architecture note and this plan.
- No plugin, Lua core, TUI, React SPA, Rails relay, MCP, provider, or Project Pipelines runtime changes.

Worktree/target assumptions:

- Implementers work in the pipeline-provided worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The base dependency is already merged into this worktree at `c7569a9`.
- The production runtime path for this repo is the exported `botster_core` public API plus the `botster-core-dev` local embedder harness, not the full TryBotster hub product.

## Assumptions And Unknowns

Assumptions:

- `BotsterEngine` is the canonical command facade; this is fixed by [[botster engine command surface uses botsterengine as facade]].
- `DefaultBotsterEngine` should expose the same typed command API when `local-runtime` is enabled, with `DefaultBotsterEngineError` preserving local runtime failures.
- `MultiplexerEngine` remains an implementation primitive. New public command APIs should not require normal embedders to assemble or call it directly.
- The existing command aliases in `engine::command` are insufficient for this ticket because they do not provide a typed request/result/error/event API or a command dispatch path.
- It is acceptable for spawn over generic `BotsterEngine<R, W>` to require the caller to supply the worker runtime as part of the typed spawn request, because custom adapters are host-owned.
- Notification commands can use command-specific request structs even though the result is currently `NotificationId` or drained `Vec<NotificationItem>`.
- Replay snapshot support is adapter-dependent. Fake-backed tests can prove dispatch; default-local tests should prove typed unsupported behavior unless the real local runtime already supports replay.

Unknowns for implementation:

- Exact API shape: prefer a compact `EngineCommand<W>` enum plus `execute` method over scattered request functions if it keeps all variants exhaustively checked without adding routing policy. If generic worker runtime on spawn makes one umbrella enum awkward, use per-command request structs plus an exhaustive `EngineCommandKind::from_request`/dispatch guard.
- Whether `EngineCommandResult` should remain the current `BotsterEngineOutput` alias or become a real enum covering spawn, output, list, inspect, notification post/drain, and unsupported results. Prefer a real enum if that is necessary to satisfy "typed request/result/error/event API."
- Whether `EngineCommandError` should be a facade-specific enum with `Core(BotsterEngineError)` and `Unsupported` variants, or a type alias plus command-level unsupported result. Prefer preserving source errors and session/request ids where available.
- Whether to extend `botster-core-dev::EngineSmokeReport` for typed list/inspect/screen/snapshot commands. Do it only if the change remains focused; fake-backed command tests plus default-local unsupported tests may be enough for the ticket.
- Whether command API docs belong only in `engine::command` rustdoc plus `docs/architecture/engine-command-surface.md`, or also in the README. Prefer a short README pointer if needed, not duplicated tables.

No human question is blocking planning. The ticket clearly asks for implementation of the API defined by the prior command-surface plan, and the dependency run supplies the required boundary decision.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/command.rs`
  - Add typed command request/result/error/event API.
  - Preserve existing public aliases where useful, but stop relying on aliases as the whole implementation.
  - Add exhaustive command-kind mapping or dispatch guard.
- `crates/botster-core/src/engine/botster.rs`
  - Add narrow `execute_command` / `run_command` methods on `BotsterEngine<R, W>` and `DefaultBotsterEngine`, or equivalent public methods that live close to the facade.
  - Keep methods as delegates to existing public facade operations.
- `crates/botster-core/src/engine/mod.rs`
  - Export new command request/result/error/event types.
- `crates/botster-core/src/lib.rs`
  - Re-export new command API at crate root and under `engine_command`.
- `crates/botster-core/tests/botster_engine_api_test.rs` or new `crates/botster-core/tests/engine_command_api_test.rs`
  - Cover typed commands over fake runtime adapters and crate-root imports.
- `crates/botster-core/tests/managed_session_runtime_test.rs` or existing local-runtime tests
  - Cover default-local unsupported snapshot replay if that behavior is exposed through the typed command API.
- `crates/botster-core-dev/src/lib.rs`
  - Optional smoke harness update if used to prove typed command execution over `DefaultBotsterEngine`.
- `docs/architecture/engine-command-surface.md`
  - Update from vocabulary note to typed command API contract.
- `docs/plans/typed-engine-command-api.md`
  - This plan artifact.

Possible but avoid unless implementation proves necessary:

- `README.md`
  - Short pointer or export list update only.
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Only tiny helper delegates if the typed default-local path cannot otherwise preserve errors cleanly.
- `crates/botster-core/src/engine/multiplexer.rs`
  - Avoid direct changes unless a missing public facade helper blocks typed dispatch.

Not expected:

- `Cargo.toml` or dependency changes.
- `botster-terminal-ghostty` changes.
- Hub/CLI product, browser, TUI, Rails, Lua plugin, MCP, provider, or Project Pipelines files.

## Implementation Shape

Suggested minimal shape:

1. Model typed commands in `engine::command`.
   - `EngineCommand<W>` or a family of request structs such as `EngineSpawnSessionCommand<W>`, `EngineAttachClientCommand`, `EngineSendInputCommand`, etc.
   - `EngineCommandResult` as a concrete enum if one command entry point returns heterogeneous result shapes.
   - `EngineCommandError<E>` or facade-specific error wrappers that preserve the original typed error and command context.
   - `EngineCommandEvent` should stay tied to existing `SessionIoEvent` / `BotsterEngineOutput` events rather than inventing a new pushed event stream.
2. Add an execution API on the facade.
   - For generic `BotsterEngine<R, W>`, dispatch typed commands by calling existing public facade methods.
   - For `DefaultBotsterEngine`, dispatch typed default-local commands by calling its public methods.
   - Keep the execution methods synchronous and deterministic.
3. Add drift guards.
   - Exhaustive `match` over every `EngineCommandKind` in tests or implementation.
   - Tests should fail or stop compiling when a command kind is added without dispatch/result coverage.
4. Prove runtime/user path.
   - Fake-backed tests cover every typed command variant and assert resulting typed outputs/errors.
   - Default-local test or `botster-core-dev` smoke covers at least spawn/attach/input/resize/list/inspect/shutdown through the typed command API, and covers screen/snapshot/replay as supported or explicitly unsupported.
   - The test filter `engine_command` must execute a non-zero test count.
5. Update docs.
   - `docs/architecture/engine-command-surface.md` should state the real typed API names and explain supported versus adapter-dependent behavior.
   - Rustdoc examples should compile against crate-root exports.

## Risks

- A command enum can become a second engine router. Mitigation: dispatch methods must be thin calls into existing `BotsterEngine` / `DefaultBotsterEngine` public methods, with no duplicated session state logic.
- Heterogeneous command results can become too abstract. Mitigation: use a small explicit result enum and preserve existing rich types instead of reducing everything to JSON or strings.
- Generic spawn may overfit to fake tests if worker runtime ownership is unclear. Mitigation: make worker runtime an explicit host-supplied part of the spawn command for generic engines.
- Default-local support can overpromise snapshot replay. Mitigation: test typed unsupported errors and document adapter-dependent support.
- Command-kind drift was already found in the base ticket review. Mitigation: add compile-time or exhaustive mapping guards in this ticket.
- Adding product-like command policy would violate Botster architecture. Mitigation: keep all ids, commands, cwd, env, timestamps, thresholds, and reasons caller-provided.
- `BoundaryJson` is tempting for ergonomic payloads. Mitigation: stable engine controls remain typed; raw JSON stays plugin/relay-owned.
- Filtered cargo tests can pass vacuously. Mitigation: verification must report non-zero test counts for `engine_command` filters.
- Project Pipelines SQLite locks can drop checklist/gate writes. Mitigation: preserve evidence in this repo artifact and retry plugin writes once after the plan is complete.
- Plan docs can leak local paths. Mitigation: use neutral worktree references and avoid absolute local paths in committed artifacts.

## Acceptance Checks / Tests

Required checks:

- `cargo fmt --all -- --check`
- `cargo test -p botster-core engine_command`
- `cargo test -p botster-core botster_engine`
- `cargo test -p botster-core managed_session_runtime`
- `cargo test -p botster-core --doc`
- `cargo test -p botster-core-dev`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `RUSTDOCFLAGS='-D rustdoc::broken_intra_doc_links' cargo doc -p botster-core --no-deps`
- `RUSTDOCFLAGS='-D rustdoc::broken_intra_doc_links' cargo doc -p botster-core --no-deps --no-default-features`

Targeted acceptance assertions:

1. Public crate-root exports include the typed command request/result/error/event API.
2. Typed command requests cover spawn, attach, detach, input, resize, list, inspect, screen read, snapshot capture, snapshot replay where supported, shutdown, post notification, and drain notifications.
3. Typed command results preserve existing rich outputs: spawn outcome, `BotsterEngineOutput`, session inventory, inspection, notification id, drained notifications, and typed unsupported/error outcomes.
4. Typed errors preserve useful context without PII and wrap existing facade/default-local errors instead of flattening them to strings.
5. Commands execute through `BotsterEngine` / `DefaultBotsterEngine` public facade methods, not through duplicated internal state mutation.
6. At least one real local-runtime path uses the typed command API over `DefaultBotsterEngine` to spawn a synthetic command and observe subscribed client egress.
7. Fake-backed tests exercise every command variant, including screen/snapshot/replay success paths.
8. Default-local tests prove adapter-dependent unsupported behavior for replay or any unsupported screen/snapshot command.
9. `EngineCommandKind` has a drift guard tied to typed command dispatch/result coverage.
10. No committed docs/tests/examples contain personal paths, usernames, credentials, prompts, hostnames, or private terminal contents.

Runtime/user path proof:

- The changed runtime path is the exported `botster_core` typed command API consumed through `BotsterEngine` and `DefaultBotsterEngine`.
- Evidence that types compile is not enough. Tests must show typed commands delegate into existing facade methods and produce the same runtime outcomes as direct method calls, including the `DefaultBotsterEngine` local PTY path where practical.

## Vault Checklist Evidence

- Vault/project notes constraining the plan: [[planner-playbook]], [[botster-planner-playbook]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[plan steps need reviewable plan artifacts]], [[botster engine command surface uses botsterengine as facade]], [[identity]], and [[goals]].
- Convention conflicts: none. The plan keeps policy outside core, uses `BotsterEngine` as the command facade, preserves typed controls, and commits a repo-visible plan artifact.
- Verification evidence so far: planning inspection only; no implementation verification commands were run. Planned verification commands are listed above.
- Checklist status: attempted run checklist creation failed with Project Pipelines SQLite `database is locked`; retry before gate submission if the plugin database clears.
- Durable knowledge capture: no new vault note is required before implementation. Capture after implementation if this ticket establishes a durable rule for typed command enum drift guards or `DefaultBotsterEngine` typed-command smoke coverage.

## Vault Gaps Worth Capturing

- Capture a Botster architecture note if the implementation settles a reusable pattern for typed command enums over public facade methods without creating a second engine router.
- Capture a testing convention if command vocabulary enums must always include an exhaustive drift guard tied to public facade dispatch.
- Capture a dev-harness convention if `botster-core-dev` becomes the standing place for typed command API real-runtime smoke coverage.
