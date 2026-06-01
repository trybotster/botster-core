# Integrate Default PTY Runtime With MultiplexerEngine Facade

Ticket: `ticket_1780189420_619547`
Run: `run_1780196235_612958`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Integrate default PTY runtime with MultiplexerEngine facade`, run `run_1780196235_612958`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts, reviews, findings, questions, or answers.
- Botster inbox correction received from `sess-1780010763-0004-e17c2a26675b1870aef8bb2b25e72d0c`: this run is main-rooted. The corrected run row has `base_ref=main`, `base_run_id=null`, and `base_ticket_id=null`. Do not create a stacked PR or branch from a dependency ticket.
- Dependencies loaded from pipeline context:
  - `ticket_1780189402_540507` closed: `Add default local PTY process runtime to botster-core`
  - `ticket_1780189402_252733` closed: `Design supervised session task runtime for core engine`
  - `ticket_1780189402_297736` closed: `Define ergonomic embeddable BotsterEngine API`
- Git context checked:
  - Current branch `project-pipelines/ticket_1780189420_619547` is at `b49046b`, which is behind fetched `origin/main` at `63c0207`.
  - `origin/main` contains the dependency surfaces this ticket needs: `LocalProcessRuntime`, `ManagedSessionRuntime`, `BotsterEngine`, and the default runtime docs/tests.
  - Implementers must merge or rebase onto `origin/main` before coding so they do not reimplement dependency work.
- Required playbooks loaded:
  - [[planner-playbook]]
  - [[botster-planner-playbook]]
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
- Prior plan artifacts inspected:
  - `docs/plans/ergonomic-embeddable-botster-engine-api.md`
  - `docs/plans/default-local-pty-process-runtime.md` from `origin/main`
  - `docs/plans/supervised-session-task-runtime-core-engine.md` from `origin/main`
- Repo context inspected:
  - `crates/botster-core/src/engine/botster.rs`
  - `crates/botster-core/src/engine/multiplexer.rs`
  - `crates/botster-core/src/engine/managed_session_runtime.rs` from `origin/main`
  - `crates/botster-core/src/runtime/local_process.rs` from `origin/main`
  - `crates/botster-core/src/runtime/mod.rs`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - `crates/botster-core/tests/managed_session_runtime_test.rs` from `origin/main`
  - `crates/botster-core/tests/local_process_runtime_test.rs` from `origin/main`
  - `crates/botster-core-test-support/src/fake/mod.rs`
  - `README.md` from `origin/main`
- Project Pipelines checklist instructions loaded. Creating the run-level vault checklist hit a SQLite write lock; checklist evidence is preserved in this plan and should be retried before advancing if the lock clears.

## Scope

Integrate the already-added default local PTY/process runtime and supervised session runtime path into the ergonomic public `BotsterEngine` facade so a consumer can construct a default public engine, spawn a real local session, attach a client, observe fanout, write input, resize, classify activity, and shut down without supplying custom runtime adapters.

In scope:

- Update the ticket worktree to include `origin/main` dependency work before implementation.
- Add a default/public `BotsterEngine` construction path backed by `LocalProcessRuntime` and the existing managed session runtime adapter.
- Preserve the existing generic `BotsterEngine<R, W>` / `MultiplexerEngine<R, W>` alternate-runtime path for fakes, remote providers, and future non-PTY runtimes.
- Route client-facing `BotsterEngine` attach/write/resize/shutdown calls through the same `MultiplexerEngine` and session-worker machinery already used by fake-runtime tests.
- Route real `LocalProcessRuntime::drain_output` results into subscription fanout through the managed runtime path; do not shortcut directly from PTY output to client frames.
- Add an integration test that uses public crate-root APIs and a real local command through the default engine path.
- Keep any unsupported terminal-state behavior explicit. Snapshot/screen/mode-flag support should not be fabricated unless the existing managed runtime already supports it.
- Update README/rustdoc only where needed to document the new default engine construction path and the boundaries around host policy.

Non-scope:

- No hub, Rails, WebRTC, TUI, React SPA, Lua plugin, MCP, provider, Project Pipelines, marketplace, auth, persistence, target admission, reconnect, or notification presentation integration.
- No new product policy such as default shell choice, executable discovery, config lookup, PATH mutation, worktree admission, environment inheritance, or session retention.
- No terminal emulator grid, Ghostty parser/backend integration, full snapshot
  implementation, or mode/screen state fabrication. No restty client-renderer
  integration.
- No broad rewrite of `SessionRuntime`, `SessionWorkerRuntime`, `MultiplexerEngine`, `ManagedSessionRuntime`, or test-support fakes.
- No duplicate version-suffixed facade or compatibility branch. This should be a cold, current API shape.
- No PII in tests, docs, metadata, command strings, or fixtures.

Botster layers touched:

- Rust `botster-core` engine facade: primary surface.
- Rust `botster-core` managed session/runtime integration: narrow routing surface.
- Rust `botster-core` public exports and docs.
- Rust `botster-core` integration tests.
- No plugin, Lua core, TUI, React SPA, Rails relay, MCP, or Project Pipelines runtime behavior changes.

Worktree/target assumptions:

- Work happens in this assigned worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- This is a main-rooted run, not a stacked run. Branch/PR work should be based on `main`/`origin/main`, not on any dependency ticket branch or base run.
- The first implementation action is to merge or rebase `origin/main`, because this branch is currently behind the dependency merge containing the runtime pieces.

Pipeline gates/artifacts:

- This file is the Plan artifact.
- Plan gate evidence should cite this file and note the temporary checklist write lock if it persists.

## Assumptions And Unknowns

Assumptions:

- `LocalProcessRuntime` and `ManagedSessionRuntime` from `origin/main` are the intended building blocks; this ticket should wire them together through the facade, not recreate them.
- The "default engine" should be a public constructor/type alias/convenience on top of `BotsterEngine` rather than a new parallel engine.
- The default path can remain synchronous and poll/drain based. The host still owns scheduling and async supervision.
- A real local integration test can use synthetic commands such as `printf`/`cat`/`sh` without private paths or user data.
- The same public API must still work with `FakeSessionRuntime` and `FakeSessionWorkerRuntime`; existing fake tests should remain valid.
- No human question is blocking unless `origin/main` reveals a materially different API than the fetched shape.

Unknowns for implementation:

- Exact public shape: likely either `BotsterEngine::local()` / `BotsterEngine::default_local()` or a type alias such as `DefaultBotsterEngine = BotsterEngine<LocalProcessRuntime, SessionRuntimeWorkerAdapter>`. Prefer the smallest API that compiles cleanly and reads well in the integration test.
- Whether `SessionRuntimeWorkerAdapter` should become public, stay hidden behind a default constructor, or be exposed only through a type alias. Prefer hiding it unless consumers need to name it.
- Whether the real integration test should use the lower-level managed runtime method names directly or only `BotsterEngine` methods plus a `drain_runtime_once` facade method. Ticket intent points to `BotsterEngine`, so the public facade should own the consumer path.
- How much snapshot-related unsupported behavior to surface. It is acceptable to reject snapshot/screen requests in this ticket if the test proves PTY output fanout through subscriptions.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/botster.rs`
  - Add the default local PTY-backed constructor/type alias and any minimal drain method needed for real runtime output to reach subscribed clients.
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Possible small visibility or helper changes so `BotsterEngine` can reuse the adapter without exposing internals unnecessarily.
- `crates/botster-core/src/engine/mod.rs`
  - Export new default facade type/helper if added.
- `crates/botster-core/src/lib.rs`
  - Re-export the public default engine path from the crate root.
- `crates/botster-core/tests/botster_engine_default_runtime_test.rs` or `botster_engine_api_test.rs`
  - Real local default-runtime integration test through public APIs.
- `README.md`
  - Narrow docs update for the default local facade path and boundary.
- `docs/plans/default-pty-runtime-multiplexer-engine-integration.md`
  - This plan artifact.

Possible but avoid unless needed:

- `crates/botster-core/src/engine/multiplexer.rs`
  - Small accessors already exist on `origin/main`; add only if the facade needs a missing narrow hook.
- `crates/botster-core/src/runtime/local_process.rs`
  - Avoid behavior changes unless integration exposes a real bug.
- `crates/botster-core-test-support/src/fake/mod.rs`
  - Only if fake conformance helpers need to prove alternate-runtime preservation.
- `crates/botster-core-dev/src/lib.rs`
  - Optional smoke harness update if it gives cheap executable-path proof without widening scope.

Not expected:

- Contract enum rewrites.
- Workspace-wide dependency churn beyond what dependency tickets already added.
- Hub, CLI product, browser, TUI, Rails, Lua plugin, MCP, provider, or Project Pipelines files.

## Implementation Shape

Suggested minimal shape:

- Merge/rebase `origin/main` first.
- Add a public default local engine path, for example:
  - `pub type DefaultBotsterEngine = BotsterEngine<LocalProcessRuntime, SessionRuntimeWorkerAdapter>` if the adapter can be public without leaking policy, or
  - `BotsterEngine::local()` returning an opaque/default concrete engine if that keeps the adapter internal.
- Add facade methods only as needed to satisfy the lifecycle:
  - spawn with explicit `SessionSpawnRequest`
  - attach client
  - write bytes
  - resize
  - drain real runtime output once for a session into existing fanout
  - classify activity
  - shutdown
- Under the hood, delegate to `ManagedSessionRuntime<LocalProcessRuntime>` or reuse its `SessionRuntimeWorkerAdapter`; do not duplicate input flushing or output conversion logic.
- Preserve the existing generic facade constructors so tests and embedders can still supply alternate runtimes.
- Keep unsupported terminal-state requests rejected with typed errors rather than inventing snapshots.

Runtime path proof:

- The integration test must instantiate the new default public facade path from `botster_core`, not private modules.
- It must spawn a real local child via `LocalProcessRuntime`, attach a client via the facade, drain PTY output through runtime output conversion, and assert the resulting `TransportEgress::TerminalOutput` is delivered through subscription fanout.
- It must send input and resize through the facade and prove those calls reach the runtime path, not only update in-memory contract structs.
- It must shut down the session and observe lifecycle/activity changes through the facade.

## Risks

- The biggest immediate risk is stale-branch implementation. Coding before merging `origin/main` would recreate already-merged dependency work and likely conflict.
- A related workflow risk is accidentally treating the run as stacked because earlier context exposed dependency metadata. The latest Botster message corrects this: proceed from `main`.
- Exposing `SessionRuntimeWorkerAdapter` as a public type may freeze an internal bridge too early. Prefer a default engine alias/constructor that does not make consumers name internals unless Rust requires it.
- A facade that simply exports `LocalProcessRuntime` without routing through `MultiplexerEngine` would fail the ticket. The test must prove subscription fanout.
- A facade that hides spawn policy by choosing commands, cwd, env, target, or shell would violate the core boundary.
- A real PTY integration test can be timing-sensitive. Use bounded polling with generous timeouts and synthetic commands.
- Shutdown can race final output. Tests should drain/poll in order and avoid assuming every platform reports identical PTY echo behavior unless gated.
- Adding snapshot or screen behavior now would expand scope into terminal-parser work that previous plans intentionally deferred.
- Public docs could overclaim hub/product readiness. Keep wording to embeddable core API.

## Acceptance Checks / Tests

Required targeted test:

- `default_botster_engine_spawns_local_session_and_fans_out_output`
  - Import the default public facade path from `botster_core`.
  - Spawn a synthetic local command with explicit `SessionSpawnRequest`, cwd, env, and optional PTY size.
  - Attach a client with a `SubscriptionId`.
  - Drain runtime output with bounded polling until the known marker reaches `TransportEgress::TerminalOutput`.
  - Send input where applicable through the facade and prove output or runtime delivery.
  - Resize through the facade and assert the call succeeds through the runtime bridge.
  - Classify activity after output.
  - Shut down and assert lifecycle/shutdown behavior is observable.
  - Avoid private paths, usernames, or product-specific strings.

Preservation tests:

- Existing `botster_engine_api_test` still passes with fake runtimes.
- Existing `multiplexer_engine_api_test` still passes with lower-level generic runtime APIs.
- Existing `managed_session_runtime_test` still passes, proving the bridge behavior remains intact.
- Existing `local_process_runtime_test` still passes, proving direct runtime behavior remains intact.

Verification commands:

- `cargo fmt`
- `cargo test -p botster-core default_botster_engine`
- `cargo test -p botster-core botster_engine`
- `cargo test -p botster-core managed_session_runtime`
- `cargo test -p botster-core local_process_runtime`
- `cargo test -p botster-core`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

If `botster-core-dev` is updated:

- `cargo test -p botster-core-dev`

## Vault Gaps Worth Capturing

No pre-implementation capture is required.

Capture after implementation if either decision becomes durable:

- The final public naming for the default local engine path, especially if it establishes `BotsterEngine` as the facade over `ManagedSessionRuntime<LocalProcessRuntime>`.
- The rule for exposing or hiding internal runtime worker adapters in public core API.
- Any PTY timing/shutdown gotcha discovered while proving real local fanout through the facade.

## Vault Checklist Evidence

- Notes read: `planner-playbook`, `botster-planner-playbook`, `identity`, `goals`, `botster-architecture`, `cli-patterns`, `spa-patterns`, `project pipeline orchestration belongs in a device-level botster plugin`, `project pipelines needs an operator workbench not more primitives`, `project pipelines ui contract belongs in the plugin readme`, `botster orchestration should spawn agents with explicit target ids`, `botster orchestration prompts must bind agents to explicit worktrees`.
- Convention conflicts: none. The plan keeps reusable runtime mechanics in core and excludes product policy, hub behavior, and speculative abstractions.
- Verification evidence in Plan step: repo and dependency artifact inspection only; no implementation tests run. `git fetch origin` showed this branch is behind `origin/main` by the default-runtime dependency merge. Botster inbox correction confirmed this run should proceed as main-rooted, not stacked.
- Checklist write status: `project_pipelines_create_vault_checklist` failed once with SQLite `database is locked`. Evidence is recorded here and should be retried before gate advancement if the lock clears.
- Durable knowledge capture: not needed before implementation; capture after implementation only if the public default-engine naming or adapter visibility decision becomes reusable Botster architecture knowledge.
