# Minimal Botster Core Dev Command Runner

Ticket: `ticket_1780348077_336776`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: run `run_1780353551_585887`, current step `botster_plan`, run step `run_step_1780353552_661441`, gate `botster_plan_gate`.
- Ticket: `Add minimal botster-core dev command runner`.
- Dependency loaded from context: `ticket_1780348077_294552` is closed and supplied the typed engine command API.
- Target from pipeline context: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Current worktree: the pipeline-provided ticket worktree.
- Required playbooks loaded: `planner-playbook`, `botster-planner-playbook`.
- Required vault/project notes loaded: `identity`, `goals`, `botster-architecture`, `cli-patterns`, `spa-patterns`, `project pipeline orchestration belongs in a device-level botster plugin`, `project pipelines needs an operator workbench not more primitives`, `project pipelines ui contract belongs in the plugin readme`, `botster orchestration should spawn agents with explicit target ids`, `botster orchestration prompts must bind agents to explicit worktrees`.
- Project Pipelines checklist instructions loaded. Creating the run-level vault checklist failed with SQLite `database is locked`; checklist evidence is preserved in this plan and should also be copied into gate evidence.
- Repo context loaded:
  - `Cargo.toml`: workspace includes `crates/botster-core-dev`.
  - `README.md`: core is not the product app, hub, or CLI; the `botster-core-dev` description is stale because it says the harness fakes the session/client/plugin path.
  - `docs/architecture/engine-command-surface.md`: `BotsterEngine` is the canonical policy-free command facade and `DefaultBotsterEngine` is the local PTY-backed facade.
  - `docs/plans/minimal-real-embedder-example.md`: prior related plan for a real embedder smoke path.
  - `crates/botster-core-dev/src/lib.rs`: current harness already uses `DefaultEngineCommand` over `DefaultBotsterEngine` for spawn, attach, drain, input, resize, activity, and shutdown.
  - `crates/botster-core-dev/src/main.rs`: binary is a thin caller for `run_engine_smoke()`.
  - `crates/botster-core-dev/tests/engine_smoke_test.rs`: Unix smoke test asserts real local command behavior.
  - `crates/botster-core/src/engine/botster.rs` and `crates/botster-core/src/engine/command.rs`: public default command API includes read-screen and capture-snapshot commands.
  - `crates/botster-core/tests/botster_engine_api_test.rs` and `crates/botster-core/tests/managed_session_runtime_test.rs`: examples for read-screen and snapshot event assertions.

## Scope

Finish the minimal non-product dev/example command runner in `botster-core-dev` so `cargo run -p botster-core-dev` proves the public typed command API end to end against a real explicit local command.

In scope:

- Keep `crates/botster-core-dev` as the command-runner home; do not add a new product CLI crate.
- Preserve the shared harness path used by both the binary and smoke test.
- Extend the report and smoke test to prove read-screen and capture-snapshot commands where the default local runtime supports them.
- Keep the explicit shell command deterministic, local, and host-supplied by the harness; no command discovery or default shell selection.
- Refresh stale README wording so `botster-core-dev` is described as a dev-only real local engine smoke runner, not a fake-only harness.
- Keep output scrubbed of user paths, hostnames, credentials, or other PII.

Non-scope:

- No product CLI UX, install flow, auth, config discovery, hub daemon, Rails relay, WebRTC/signaling, cloud/provider behavior, marketplace policy, TUI, React/browser, Project Pipelines runtime behavior, or plugin workflow changes.
- No broad refactor of `BotsterEngine`, `DefaultBotsterEngine`, `ManagedSessionRuntime`, `MultiplexerEngine`, runtime traits, or terminal-screen internals.
- No new command-line parser, flags, config files, persistent state, or command discovery.
- No Ghostty adapter work; the ticket targets the public core local runtime path.

Botster layers touched:

- Rust core dev harness: primary.
- Rust public core facade: exercised but not expected to change.
- Rust tests: focused dev-harness smoke coverage.
- Docs: README wording and this plan artifact.

Worktree and target assumptions:

- Implementer must work only in this pipeline-assigned worktree and preserve target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- This run is stacked on closed dependency `run_1780350440_426368` / `ticket_1780348077_294552`; do not retarget or rebase silently.

Pipeline gates/artifacts:

- Plan artifact: `docs/plans/minimal-botster-core-dev-command-runner.md`.
- Gate evidence should cite this artifact and state the checklist write lock fallback.

## Assumptions And Unknowns

Assumptions:

- The existing `botster-core-dev` harness is the intended minimal runner location.
- The current public `DefaultEngineCommand::{ReadScreen,CaptureSnapshot}` commands are sufficient; implementation should first use those APIs rather than change core.
- Unix-only PTY behavior is acceptable for the real local smoke test because existing local runtime tests are Unix-shaped; non-Unix behavior may remain a clear skip path.
- `sh -c ...` remains acceptable as the explicit local command for this dev harness.

Unknowns for implementation:

- Exact screen/snapshot payload text may vary with PTY timing. Assertions should prove that the commands are executed and return the expected event kind and a non-empty or relevant payload after output has drained, without relying on exact chunking.
- If the local runtime returns a blank screen/snapshot before its shadow state catches up, the implementer should add one focused drain/read retry in the harness, not broaden runtime internals.

No human question is blocking planning. The ticket intent is specific enough: a dev/example runner, not product CLI behavior.

## Affected Surfaces / Files

Expected changes:

- `crates/botster-core-dev/src/lib.rs`
  - Add report fields for screen and snapshot evidence.
  - Execute `DefaultEngineCommand::ReadScreen` and `DefaultEngineCommand::CaptureSnapshot` through the same public typed command path.
  - Extract screen/snapshot event evidence from returned `session_events`.
- `crates/botster-core-dev/tests/engine_smoke_test.rs`
  - Assert screen and snapshot evidence is present and comes from supported command outcomes.
- `README.md`
  - Update the workspace-layout line for `crates/botster-core-dev` so it no longer claims the harness is fake-only.

Likely unchanged:

- `crates/botster-core-dev/src/main.rs`
  - Should remain a thin report printer unless new report fields require no structural change.

Possible but not expected:

- `crates/botster-core/src/engine/botster.rs` or `crates/botster-core/src/engine/command.rs`
  - Only if the existing public typed commands are broken; prefer a focused bug fix with tests over API churn.

## Risks

- Stopping at the existing harness would miss the ticket's explicit screen/snapshot acceptance.
- Exact PTY output and screen timing can be flaky; tests should use substring/event-kind checks with bounded waiting.
- A long-running shell loop can leak if shutdown is skipped on error paths. Keep the harness linear and call shutdown after successful spawn where practical.
- Adding args/config/discovery would accidentally turn the dev harness into product CLI surface.
- Stale README wording can cause future agents to preserve fake-only behavior despite the real runner intent.
- Checklist persistence is currently locked, so workflow evidence must be preserved in the plan and gate.

## Acceptance Checks / Tests

Required checks:

- `cargo fmt --all -- --check`
- `cargo test -p botster-core-dev`
- `cargo run -p botster-core-dev`
- `cargo test -p botster-core --test botster_engine_api_test`
- `cargo clippy --workspace --all-targets -- -D warnings`

Targeted success criteria:

1. `cargo run -p botster-core-dev` executes the same shared harness as the smoke test.
2. The runner spawns an explicit local command through `DefaultEngineCommand::SpawnSession`.
3. The runner attaches a client through `DefaultEngineCommand::AttachClient`.
4. The runner observes startup output through subscribed client egress.
5. The runner sends input through `DefaultEngineCommand::SendInput` and observes echoed output.
6. The runner resizes through `DefaultEngineCommand::Resize`.
7. The runner executes `DefaultEngineCommand::ReadScreen` and records screen evidence.
8. The runner executes `DefaultEngineCommand::CaptureSnapshot` and records snapshot evidence.
9. The runner shuts down through `DefaultEngineCommand::Shutdown`.
10. Source, docs, printed output, and assertions contain no absolute home paths, personal names, credentials, or host-specific paths.

Runtime path proof:

- This ticket is intentionally a dev-harness/example runtime path.
- The changed user path is `cargo run -p botster-core-dev`; it must exercise the public typed default command API against a real local process.
- Evidence that the command API exists is insufficient unless the dev runner actually executes it.

## Vault Gaps Worth Capturing

- Capture a durable vault note after implementation if `botster-core-dev` becomes the standing convention for real public-command API smoke coverage.
- No new vault note is required from planning alone; the discovered checklist write lock is already covered by the existing `project pipelines checklist worker timeouts require artifact evidence fallback` and SQLite lock notes.
