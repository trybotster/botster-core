# Harden Daemon And Worker Hot Paths For Many PTYs

Ticket: `Harden daemon and worker hot paths for many PTYs`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `ticket_1780532711_421539`, run `run_1780542138_321150`, current step `botster_plan`, current run step `run_step_1780542138_915867`, gate `botster_plan_gate`.
- Ticket intent: harden the daemon/worker architecture for tens to hundreds of PTYs so slow clients, slow plugins, and high-output sessions do not stall unrelated sessions or the daemon control path.
- Dependency loaded from context: `ticket_1780532711_470736` / `Add core daemon supervisor and persistent session registry` is closed.
- Reviews, findings, open questions, prior answers, and artifacts loaded from context: none.
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
- Additional vault constraints loaded:
  - [[identity]]
  - [[goals]]
  - [[plan steps need reviewable plan artifacts]]
  - [[sessionioworker is the production read path for session pty output]]
  - [[botster hub event storms must be rejected before queues grow unbounded]]
  - [[botster hub events use bounded priority lanes instead of unbounded queue fuses]]
  - [[plugin hardening needs lifecycle resource and observability layers]]
  - [[project pipelines checklist worker timeouts require artifact evidence fallback]]
- Project Pipelines checklist instructions loaded. Creating the run-level vault checklist timed out with `plugin worker invoke timeout`, so this artifact and gate evidence carry the checklist fallback provenance required by [[project pipelines checklist worker timeouts require artifact evidence fallback]].
- Repo context loaded:
  - `crates/botster-core/src/runtime/local_process.rs`: local PTY reader queues are already bounded with `DEFAULT_PTY_READER_CHUNK_CAPACITY`, explicit pressure accounting, and `SessionRuntimeOutput::Backpressure`.
  - `crates/botster-core/src/engine/managed_session_runtime.rs`: `ManagedSessionRuntime::drain_runtime_once` and `drain_runtime_all_once` route runtime output, typed backpressure observations, pending worker runtime events, and fair one-drain-per-session ticks.
  - `crates/botster-core/src/engine/session_worker.rs`: `SessionWorkerEngine` preserves initial snapshot ordering, live output, process exit, shutdown, and last-output activity.
  - `crates/botster-core/src/engine/subscription_multiplexer.rs`: per-client subscription routing reports delivery lag, queue-full, and queue-closed without transport writes in the core path.
  - `crates/botster-core/src/engine/plugin_worker.rs`: per-plugin capacity, timeout, cancellation, reload/unload, and backpressure mechanics already exist.
  - `crates/botster-core/src/engine/multiplexer.rs` and `crates/botster-core/src/engine/botster.rs`: `MultiplexerEngine`, `BotsterEngine`, and `DefaultBotsterEngine` are the public facade paths for spawn, attach, input, resize, snapshot, plugin, and drain behavior.
  - `crates/botster-core-test-support/src/conformance/mod.rs`: many-PTY and adversarial hot-path harnesses already exercise real `DefaultBotsterEngine` local PTY sessions, a noisy session, and timed control commands.
  - Existing tests cover three-session fair drain, subscription backpressure isolation, plugin timeout/capacity, default-engine route pressure, 20/50/100 many-PTY loads, and adversarial public-command timing. The scale gap is mostly combined adverse-path coverage and explicit daemon-control responsiveness under slow subscriber/plugin simulations.

## Scope

In scope:

- Extend existing load and adversarial harnesses instead of inventing a new runtime architecture.
- Prove the production facade path changes or remains correct through `DefaultBotsterEngine`, `ManagedSessionRuntime`, `SubscriptionMultiplexer`, and `PluginWorkerEngine`.
- Add deterministic tests that cover at least tens of sessions directly and keep the existing opt-in 100-session shape usable without rewriting the harness.
- Add or tighten combined simulations for:
  - one noisy/high-output PTY while quiet sessions complete,
  - slow output subscribers represented by client-worker/transport queue lag or queue-full reports,
  - slow or saturated plugin worker invocations,
  - public daemon/control commands such as list, inspect, attach/detach, resize, input, screen, snapshot, and shutdown while output load is active.
- Document the actual queue/backpressure behavior under test: local PTY reader pressure, client delivery lag/failure semantics, plugin worker capacity and timeout semantics, and fair drain behavior.
- Add metrics/test hooks only where they produce deterministic test evidence through existing typed observations.
- Keep all fixtures/log strings synthetic and PII-free.

Non-scope:

- No browser, TUI, Rails relay, ActionCable, WebRTC, Project Pipelines UI, or product workflow policy changes.
- No new queue framework, executor abstraction, daemon supervisor redesign, or optional configurability unless a test cannot be deterministic without a narrow helper.
- No broad refactor of `SessionWorkerEngine`, `SubscriptionMultiplexer`, `PluginWorkerEngine`, `ManagedSessionRuntime`, or `DefaultBotsterEngine`.
- No lossy terminal-output policy changes. Existing bounded reader behavior should remain typed pressure plus retained-byte ordering unless implementation finds a concrete bug.
- No moving byte-bearing terminal data through hub/product policy. Session/client worker boundaries remain the architectural authority.

Botster layers touched:

- Rust core local PTY/session runtime and default facade tests.
- Rust core subscription/client worker routing tests.
- Rust core plugin worker tests.
- Rust core test-support/conformance harness.
- Docs/plan artifact only unless the implementation settles a public contract that requires README updates.

Worktree and target assumptions:

- Downstream agents should work only in the assigned pipeline worktree.
- Pipeline target id from context: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The run targets `main`; do not treat closed dependency context as license for stacked or stale-base assumptions.

Pipeline gates/artifacts:

- Plan artifact: `docs/archive/plans/harden-daemon-and-worker-hot-paths-for-many-ptys.md`.
- Plan gate evidence should cite this artifact, loaded vault notes, checklist timeout fallback, and exact repo context inspected.
- Advancement target: `botster_plan_review`.

## Assumptions And Unknowns

Assumptions:

- "Daemon control path" maps to the public core/default-engine command surface in this repository, because the target repo is `botster-core` and concrete hub daemon UI/policy is outside this crate.
- The closed supervisor/session-registry dependency means this ticket should harden current hot paths rather than rebuild session ownership.
- Existing bounded reader and many-PTY work are part of the baseline and should not be duplicated.
- Slow subscriber behavior can be proven in core by `SubscriptionMultiplexer::report_delivery_lag` and `report_delivery_failure`, because actual transport writes are caller-owned.
- Slow plugin behavior can be proven in core by plugin worker timeout, cancellation, and per-plugin capacity pressure, then composed with session load at the harness/report boundary where practical.
- 20-session CI default and 50-session local load are sufficient direct "tens" evidence. The existing ignored 100-session test is the opt-in scalability check.

Unknowns for implementation:

- Whether a single combined conformance test should include plugin pressure during the real local PTY load, or whether the smallest reliable shape is a conformance report that composes real PTY load with focused plugin-worker and subscription-multiplexer pressure proofs.
- Whether the current adversarial hot-path report should fail when no reader backpressure is observed, or keep reporting that no reader pressure occurred under the chosen bounded load. The ticket requires queue/backpressure behavior documented and tested, not necessarily pressure in every default run.
- Whether additional test hooks are needed to force small capacities deterministically for local PTY pressure in conformance, or whether existing `LocalProcessRuntimeOptions` coverage is enough.
- Whether "fair routing" should be asserted by drain-attempt counts only, or also by bounded phase budgets while noisy output remains active.

No human question is blocking planning. The ticket has one coherent interpretation in this repo: strengthen evidence around the existing worker/data-plane hardening paths for many sessions.

## Affected Surfaces / Files

Expected:

- `crates/botster-core-test-support/src/conformance/mod.rs`
  - Tighten `run_many_pty_load` and/or `run_adversarial_hot_path_load` reports so they carry explicit success criteria for fair drain, hot-path phase timings, quiet session completion during noisy output, typed queue/backpressure observations, and the current slow-client/plugin proof boundary.
  - If combined plugin/subscriber simulations are added, keep them deterministic and synthetic.
- `crates/botster-core-test-support/tests/downstream_conformance_test.rs`
  - Add or tighten tests for 20-session default, 50-session environment path, ignored 100-session path, adversarial hot-path command budgets, and any new combined report fields.
- `crates/botster-core/tests/managed_session_runtime_test.rs`
  - Extend fair-drain tests from three sessions to a larger deterministic fake count if the conformance harness cannot cover a needed invariant cheaply.
  - Add a fake slow-subscriber or backpressure observation test only if the public managed-runtime path lacks direct evidence.
- `crates/botster-core/tests/subscription_multiplexer_engine_test.rs`
  - Strengthen slow-client tests so queue-full or queue-closed for one client/session cannot block unrelated session fanout and cannot revive stale routes.
- `crates/botster-core/tests/plugin_worker_engine_test.rs`
  - Add multi-plugin or many-invocation pressure tests only if current timeout/capacity tests do not prove unrelated plugin isolation strongly enough.
- `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - Add public facade composition evidence if behavior must be visible through `BotsterEngine` rather than only lower-level engines.
- `README.md` or public docs
  - Update only if the conformance report contract changes or the queue/backpressure semantics need a user-facing command.

Possible but not expected:

- `crates/botster-core/src/runtime/local_process.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/subscription_multiplexer.rs`
- `crates/botster-core/src/engine/plugin_worker.rs`

These should change only if tests expose an actual hot-path bug or missing typed observation.

## Risks

- Replanning already-implemented bounded-reader work would create churn without addressing the ticket's remaining acceptance gap.
- A fake-only many-session test would not prove the runtime path changed. Real `DefaultBotsterEngine` PTY coverage must remain part of acceptance.
- Combining real PTYs, noisy output, slow plugin sleeps, and slow subscriber simulations in one test can become flaky. Prefer deterministic focused tests plus one public conformance report that names how they compose.
- Treating `DefaultBotsterEngine`'s synchronous facade as a concrete hub daemon could blur repo boundaries. Keep hub/product policy out of `botster-core`.
- Queue pressure can be under-observed in normal fast CI runs. Tests that require pressure should force capacity/load deterministically or assert documented no-pressure semantics separately.
- Slow-client pressure must not unsubscribe or mutate unrelated routes unless the reported reason is `QueueClosed` for that exact active route.
- Plugin timeout tests can leave background threads alive if the fake runtime ignores cancellation. Existing tests use cancellation-aware fakes; new tests should follow that pattern.
- Large PTY counts can hit machine file-descriptor limits. Keep 100-session checks ignored/opt-in unless the CI environment is proven to support them.
- Plan artifacts and test output must avoid local usernames, absolute worktree paths, or real session content.

## Acceptance Checks / Tests

Required focused checks:

- `BOTSTER_ENV=test cargo test -p botster-core-test-support many_pty_load_default --features local-runtime -- --nocapture`
  - Must cover at least 20 real local PTY sessions through `DefaultBotsterEngine`.
- `BOTSTER_ENV=test BOTSTER_CORE_LOAD_SESSIONS=50 cargo test -p botster-core-test-support many_pty_load_default --features local-runtime -- --nocapture`
  - Must prove the same harness scales to a local 50-session run without code changes.
- `BOTSTER_ENV=test cargo test -p botster-core-test-support many_pty_load_adversarial_noisy_reports_reader_backpressure --features local-runtime -- --nocapture`
  - Must prove noisy-session queue/backpressure behavior and quiet-session completion semantics, or be updated to the renamed equivalent.
- `BOTSTER_ENV=test cargo test -p botster-core-test-support adversarial_hot_path_commands_remain_bounded_under_noisy_load --features local-runtime -- --nocapture`
  - Must prove list, inspect, attach/detach, resize, input, screen, snapshot, and shutdown command phases remain within documented budgets while noisy output is active.
- `BOTSTER_ENV=test cargo test -p botster-core-test-support many_pty_load_100 --features local-runtime -- --ignored --nocapture`
  - Opt-in local/CI check for the 100-session path. If skipped for resource limits, record the exact reason.
- `cargo test -p botster-core --test managed_session_runtime_test`
  - Required for fake fair-drain, backpressure, and production managed-runtime routing changes.
- `cargo test -p botster-core --test subscription_multiplexer_engine_test`
  - Required for slow-subscriber/client-worker/transport pressure changes.
- `cargo test -p botster-core --test plugin_worker_engine_test`
  - Required for slow-plugin or plugin-capacity changes.
- `cargo test -p botster-core --test multiplexer_engine_api_test`
  - Required if public facade composition changes.
- `cargo test -p botster-core-test-support`
  - Required if conformance helpers or report fields change.

Required repo-level checks:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`

Runtime/user-path proof:

- Evidence must name the actual path under test:
  - real local sessions via `DefaultBotsterEngine`,
  - output draining via `ManagedSessionRuntime::drain_runtime_all_once` or `drain_runtime_once`,
  - session event fanout via `SubscriptionMultiplexer`,
  - plugin pressure via `PluginWorkerEngine`,
  - hot-path command ingress via `DefaultEngineCommand`.
- Evidence that helper code exists is insufficient. The implementation report must include command output summaries that show sessions completed, exits observed, hot-path phase timings stayed bounded, and pressure/lag/failure observations were typed and route-scoped.

## Vault Gaps Worth Capturing

- Capture after implementation if the final evidence shape becomes a durable convention: `many PTY hot-path hardening composes real default-engine load with focused worker pressure proofs`.
- Capture after implementation if a new rule is settled for when conformance reports should require observed reader pressure versus documenting that no pressure occurred in a passing run.
- Capture after implementation if plugin pressure is composed into the many-PTY harness, because that would clarify the boundary between real daemon/default-engine load evidence and focused plugin-worker isolation tests.

## Project Pipelines Checklist Fallback

- Vault notes read: listed in Context Loaded.
- Convention conflicts: none. The plan keeps terminal bytes in session/client data-plane boundaries, uses existing typed backpressure and plugin-worker primitives, avoids product policy, and creates a repo-visible plan artifact.
- Verification evidence for Plan: local repo inspection only; no implementation tests were run in the Plan step.
- Durable knowledge capture: no vault note captured before implementation. Capture candidates are listed under Vault Gaps.
- Checklist persistence: attempted run-level vault checklist creation, but the Project Pipelines plugin worker timed out. This artifact is the durable fallback evidence.
