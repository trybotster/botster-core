# Minimal Real Embedder Example

Ticket: `ticket_1780245510_145290`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: run `run_1780245518_730680`, current step `botster_plan`, run step `run_step_1780245518_177599`, gate `botster_plan_gate`, ticket `Build a minimal real embedder example for botster-core`.
- Target from pipeline context: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Current worktree: assigned Project Pipelines implementation worktree.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster overlay notes loaded:
  - `identity`
  - `goals`
  - `botster-architecture`
  - `cli-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
- Project Pipelines checklist: run-level vault checklist `checklist_1780245581_327088`.
- Repo context loaded:
  - `README.md`: core is the embeddable tmux-like engine; `botster-core-dev` is dev-only and explicitly not CLI/install/auth/hub/marketplace/product policy.
  - `docs/archive/plans/botster-core-dev-engine-smoke-examples.md`: prior plan added a fake session/client/plugin smoke harness.
  - `crates/botster-core-dev/src/lib.rs`: current smoke harness uses `BotsterEngine<FakeSessionRuntime, FakeSessionWorkerRuntime>`, not a real local command.
  - `crates/botster-core-dev/src/main.rs`: binary prints `run_engine_smoke()` report.
  - `crates/botster-core-dev/tests/engine_smoke_test.rs`: test asserts fake-only report fields.
  - `crates/botster-core/src/engine/botster.rs`: public `DefaultBotsterEngine` exposes spawn, attach, write, resize, drain, classify, and shutdown over `LocalProcessRuntime`.
  - `crates/botster-core/src/engine/managed_session_runtime.rs`: public managed runtime bridges client ingress into runtime input and drains runtime output through subscription fanout.
  - `crates/botster-core/src/runtime/local_process.rs`: `LocalProcessRuntime` runs explicit host-provided commands in PTYs and handles input, resize, output drain, and shutdown.
  - `crates/botster-core-test-support/src/conformance/mod.rs`: downstream conformance harness already proves real local runtime patterns.
  - `crates/botster-core-test-support/tests/downstream_conformance_test.rs` and `crates/botster-core/tests/botster_engine_api_test.rs`: examples of Unix-gated real PTY tests and default engine usage.

## Scope

Build a minimal real embedder example in `botster-core-dev` that exercises the public default local engine path with an explicit local command.

In scope:

- Replace or extend the current fake-only `run_engine_smoke()` path so the dev binary demonstrates a real local command through `DefaultBotsterEngine`.
- Spawn a deterministic explicit shell command such as `sh -c "printf 'ready\n'; while IFS= read -r line; do printf 'echo:%s\n' \"$line\"; done"` with a repo-relative working directory and explicit PTY size.
- Attach a fake client through the public client/session subscription path.
- Drain startup output from the real PTY through `DefaultBotsterEngine::drain_runtime_once`.
- Send input through `DefaultBotsterEngine::write_bytes` and assert echoed output arrives through client egress.
- Resize through `DefaultBotsterEngine::resize`.
- Classify activity through `DefaultBotsterEngine::classify_activity`.
- Shut down through `DefaultBotsterEngine::shutdown_session`.
- Keep a single shared harness function used by both `cargo run -p botster-core-dev` and a smoke test.
- Keep output deterministic and free of user-specific paths, credentials, hostnames, or other PII.
- Update README or the dev harness docs only if current wording no longer clearly distinguishes real embedder example from product CLI.

Non-scope:

- No product CLI, install UX, auth flow, marketplace, hub daemon, Rails relay, React/TUI surface, cloud/provider policy, plugin marketplace, or persistent config.
- No new command discovery or default command selection. The example must pass an explicit command.
- No speculative abstraction over examples or runtime harnesses.
- No broad refactor of `botster-core`, runtime traits, test-support, or prior fake plugin examples.
- No user-specific absolute paths in source, docs, test assertions, or printed output.
- No requirement to exercise real plugin Lua loading; the ticket is about embedders using the tmux-like local engine path.

Botster layers touched:

- Rust core dev harness: primary.
- Rust public default engine/local runtime API: exercised but not expected to change.
- Rust tests: smoke test for the same binary/harness path.
- Docs: README or this plan artifact only if needed.

Worktree/target assumptions:

- Implementer must work only in the assigned worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- This plan targets the current branch/run, not a stacked dependency.

Pipeline gates/artifacts:

- Plan artifact: `docs/archive/plans/minimal-real-embedder-example.md`.
- Gate evidence should cite this artifact and checklist `checklist_1780245581_327088`.

## Assumptions And Unknowns

Assumptions:

- The smallest correct implementation is to adapt `crates/botster-core-dev/src/lib.rs` so `run_engine_smoke()` uses `DefaultBotsterEngine` for the local command path.
- The existing fake plugin invocation can either remain as a separate fake-only detail or be removed from the smoke report if it distracts from the ticket. It is not required by this ticket.
- `sh` is acceptable as the explicit local command for Unix smoke tests because existing repo tests already use shell-based local PTY examples.
- The binary can be Unix-only for the real PTY path if non-Unix hosts return a clear skip/error; tests should use `#[cfg(unix)]` when they require PTY support.
- Existing public APIs are sufficient. If implementation exposes a small ergonomic gap, prefer adapting the dev harness to current APIs before changing core.

Unknowns for implementation:

- Whether to preserve the fake-only plugin fields in `EngineSmokeReport` or split reports into fake engine and real embedder smoke reports. Prefer the simpler report that maps directly to this ticket.
- Whether `cargo run -p botster-core-dev` should run only the real embedder example or print both fake and real smoke sections. Prefer only the real embedder path unless preserving fake plugin coverage is necessary for existing docs.
- Exact output timing from the PTY may vary. Tests should search for expected byte substrings within a deadline rather than assert exact chunking.

No human question is blocking planning. The ticket clearly requires real local runtime behavior and explicitly excludes product surfaces.

## Affected Surfaces / Files

Expected changes:

- `crates/botster-core-dev/src/lib.rs`
  - Use `DefaultBotsterEngine` and an explicit local command in the shared smoke harness.
  - Record a report with session id, client id, observed startup output, routed input, echoed output, resize dimensions or success flag, activity classification, and shutdown observation.
- `crates/botster-core-dev/src/main.rs`
  - Likely remains a thin caller/renderer of the shared report.
- `crates/botster-core-dev/tests/engine_smoke_test.rs`
  - Assert the shared harness drives the real local command path and proves every required scenario step.
- `README.md`
  - Optional small clarification if current docs do not mention that `botster-core-dev` now includes a real local embedder example.

Possible but not expected:

- `crates/botster-core/src/engine/botster.rs`
  - Only if the example exposes a real public API bug or missing re-export.
- `crates/botster-core-test-support/src/conformance/mod.rs`
  - Only if a tiny helper should be shared by downstream examples, not just the dev harness.

Not expected:

- Hub, CLI product, TUI, React/browser, Rails, Lua plugin loader, provider, auth, marketplace, cloud, or device config files.

## Implementation Shape

Suggested minimal shape:

- Keep `main()` unchanged structurally: call `run_engine_smoke()`, print `EngineSmokeReport::lines()`, exit non-zero on error.
- Rework `run_engine_smoke()` to:
  - construct `DefaultBotsterEngine::new()`;
  - build a `SessionSpawnRequest` with deterministic ids, `executable: "sh"`, explicit `arguments`, repo-relative or `"."` working directory, empty environment, and an initial PTY size;
  - call `spawn_session`;
  - call `attach_client`;
  - drain output until `ready` is observed;
  - call `write_bytes` with a deterministic line such as `ping-embedder\n`;
  - drain output until `echo:ping-embedder` is observed;
  - call `resize` with deterministic dimensions;
  - call `classify_activity` and record `SessionActivityStatus::Active`;
  - call `shutdown_session` and record the stopping lifecycle observation.
- Use a small local drain helper with a short deadline and substring matching, modeled on existing `drain_default_until`.
- Keep the report focused on observable embedder behavior rather than internal implementation details.

## Risks

- The existing fake-only smoke harness could satisfy “code exists” while failing the ticket’s real embedder intent. The implementation must prove a real local command was spawned.
- PTY output is chunked nondeterministically, so exact output equality would be flaky.
- A long-running shell loop can leak if shutdown is not called on every successful spawn path. Keep the harness linear and fail with shutdown where practical.
- Real PTY tests are host-dependent. Use Unix gating or clear skip behavior rather than pretending non-Unix hosts can run the same command.
- Accidentally adding argument parsing/config discovery would turn the dev harness into a product CLI.
- Absolute working directories or printed paths could leak local user information.
- Over-refactoring `botster-core` to make the example prettier would exceed the ticket.

## Acceptance Checks / Tests

Required implementation checks:

- `cargo test -p botster-core-dev`
- `cargo run -p botster-core-dev`
- `cargo test -p botster-core --test botster_engine_api_test default_botster_engine_spawns_local_session_and_fans_out_output`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Targeted assertions:

1. The dev binary compiles and runs the same shared function as the smoke test.
2. The report proves a `DefaultBotsterEngine`/local PTY-backed session spawned from an explicit command.
3. The report proves a client attached through the public subscription path.
4. The report proves startup output from the real command reached client egress.
5. The report proves input sent through the client path reached the command and produced echoed output.
6. The report proves resize was called successfully.
7. The report proves activity classification returns `Active` after output.
8. The report proves shutdown requested/stopped the managed session.
9. Source, docs, printed output, and assertions contain no absolute home paths, personal names, credentials, or host-specific paths.

Runtime path proof:

- This ticket is intentionally a dev-harness example, not production host wiring.
- The changed user/runtime path is `cargo run -p botster-core-dev`: it must execute a real local command through public `DefaultBotsterEngine` methods.
- Evidence that `LocalProcessRuntime` and `DefaultBotsterEngine` already exist is insufficient unless the example actually calls them.

## Vault Gaps Worth Capturing

No durable vault gap is required from planning alone.

Potential capture after implementation:

- If the final shape becomes the standing pattern, capture a convention that `botster-core-dev` examples should prefer real `DefaultBotsterEngine` paths for embedder docs, share the exact harness with smoke tests, and keep fake-only examples reserved for plugin/runtime boundary coverage.
