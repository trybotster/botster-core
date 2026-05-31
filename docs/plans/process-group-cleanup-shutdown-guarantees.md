# Implement Process-Group Cleanup And Shutdown Guarantees

Ticket: `ticket_1780189420_549797`
Run: `run_1780196234_596866`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: current step `botster_plan`, gate `botster_plan_gate`, no artifacts, no open questions, and Plan Review findings from `review_1780201389_361608`.
- Plan Review returned `changes_required` because the first plan targeted an unwired seam: `MultiplexerEngine` calls `SessionRuntime::spawn_session`, but shutdown currently flows through `SessionWorkerRuntime::shutdown`.
- Botster message inbox loaded: orchestrator corrected the run to be main-rooted. Treat stale dependency/base metadata in the context payload as non-authoritative; do not stack this work on the dependency ticket.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Botster overlay notes loaded:
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
- Ticket-specific vault notes loaded:
  - `pty master fd close sends sighup but ignores it needs killpg`
  - `botster runtime now uses broker-authoritative pty lifecycle with unified session registry`
  - `process-terminating defaults must expose an injectable callback seam for tests`
  - `botster pipeline reviewers must bypass rtk summaries for cargo gate evidence`
- General context loaded:
  - `identity`
  - `goals`
- Repo context inspected:
  - `README.md`
  - workspace `Cargo.toml`
  - `crates/botster-core/Cargo.toml`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/runtime/mod.rs`
  - `crates/botster-core/src/engine/mod.rs`
  - `crates/botster-core/src/engine/multiplexer.rs`
  - `crates/botster-core/src/engine/session_worker.rs`
  - `crates/botster-core-test-support/src/fake/mod.rs`
  - `crates/botster-core-test-support/src/fake/session_worker.rs`
  - `crates/botster-core/tests/session_runtime_contract_test.rs`
  - `crates/botster-core/tests/session_worker_engine_test.rs`
  - `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - `crates/botster-core-dev/src/lib.rs`
  - `crates/botster-core-dev/tests/engine_smoke_test.rs`
- Prior repo plan artifacts inspected:
  - `docs/plans/core-session-worker-engine.md`
  - `docs/plans/ergonomic-embeddable-botster-engine-api.md`
  - `docs/plans/core-session-model-activity-engine.md`
- Checklist discipline:
  - `project_pipelines_checklist_instructions` loaded.
  - Initial checklist/gate writes were blocked by a Project Pipelines SQLite write lock.
  - A run-level vault checklist was later created by Plan Review: `checklist_1780201330_990291`.
  - Revised gate evidence must record that the seam decision is now explicit and that convention conflicts remain none.

## Plan Review Resolution

This revised plan chooses Plan Review option (a): wire the engine shutdown/lifecycle path to the new local process cleanup mechanic.

The implementation must not leave cleanup only behind direct calls to `SessionRuntime::send_input(Shutdown)` or `SessionRuntime::drain_output`, because no current engine path calls those methods. The production core path to prove is:

`BotsterEngine::shutdown_session` or `MultiplexerEngine::shutdown_session`
-> `SessionIoRequest::Shutdown`
-> `SessionWorkerEngine::handle_request`
-> `SessionWorkerRuntime::shutdown`
-> local process-group cleanup
-> `SessionIoEvent::ProcessExited` and lifecycle update where status is available.

The direct `SessionRuntime` path may still exist for embedders, but it is not enough for this ticket. The implementer must bridge spawn ownership and shutdown ownership so the process identity created during `SessionRuntime::spawn_session` is available to the `SessionWorkerRuntime::shutdown` implementation used by the engine.

This ticket remains core-mechanic work, not CLI product integration. It fixes the reusable embeddable core path. The existing orphan symptom described in `pty master fd close sends sighup but ignores it needs killpg` lives in the Botster CLI/session process layer; the CLI is fixed only when it adopts this core local process runtime or the same cleanup primitive in a follow-up. Suggested follow-up title: "Adopt botster-core local process-group cleanup in the Botster CLI session process."

## Scope

Implement reusable local process shutdown mechanics in `botster-core` for sessions backed by a child process, including process-group cleanup on platforms that support it.

In scope:

- Add a concrete local process-backed runtime under `crates/botster-core/src/runtime/` that provides both:
  - the spawn-side `SessionRuntime` implementation used by `MultiplexerEngine::spawn_session`;
  - a paired `SessionWorkerRuntime` implementation used by `SessionWorkerEngine` for terminal I/O and shutdown.
- Share child process identity between those two trait implementations through a narrow shared registry or handle. This is the core architectural correction from Plan Review.
- Spawn local child sessions from the existing explicit `SessionSpawnRequest` without adding executable discovery, target admission, retention, restart, or product workflow policy.
- On Unix, spawn the child into its own process group using stable Rust process APIs where possible, and terminate the process group on shutdown.
- Implement shutdown as a two-stage cleanup:
  - graceful path: request termination for the child process group and collect process exit status;
  - forced path: after a bounded grace window, force-kill the process group and collect process exit status.
- Make shutdown idempotent. Repeated explicit shutdowns, handle drops, and already-exited children must not surface false failures or leave a second cleanup path racing the first.
- Surface failures through typed runtime errors, adding focused `SessionRuntimeErrorKind` variants if the existing kinds are too vague for shutdown, status collection, or cleanup timeout/failure.
- Preserve process-exit status capture through the engine path. If shutdown observes a status synchronously, `SessionWorkerEngine` must emit `SessionIoEvent::ProcessExited` before or alongside `SessionIoEvent::Shutdown`; if status is observed later, a public runtime event/drain path must feed `SessionWorkerRuntimeEvent::ProcessExited` into `MultiplexerEngine::handle_runtime_event`.
- Ensure dropping the runtime or session handle attempts cleanup for still-live child processes.
- Add platform-gated tests that prove no representative child command is orphaned after shutdown/drop on supported Unix platforms.
- Keep testability explicit: any destructive or time-sensitive shutdown mechanics must expose a small injectable seam, such as a grace duration or sleeper/clock hook, so tests can exercise the forced path without killing the test runner or sleeping for production-length intervals.
- Update docs only where the new public runtime surface needs discoverability in the existing core ownership boundary.

Non-scope:

- No hub user-facing restart, retention, recovery, reconnect, prompt, or workspace policy.
- No Botster CLI product startup, device config, admitted target management, Lua plugin behavior, TUI, React SPA, Rails relay, MCP, or Project Pipelines product changes.
- No broad replacement of `SessionWorkerEngine`, `MultiplexerEngine`, or `BotsterEngine`.
- No full terminal emulator or browser/TUI terminal rendering behavior.
- No speculative runtime abstraction beyond the local process cleanup required by this ticket.
- No compatibility branches, legacy names, or version-suffixed duplicate APIs.
- No PII-bearing test fixtures such as real user paths, usernames, prompts, terminal transcripts, or environment dumps.

Botster layers touched:

- Rust `botster-core` runtime layer: primary surface.
- Rust `botster-core` engine/session-worker layer: required surface, because the engine shutdown path is `SessionWorkerRuntime::shutdown`.
- Rust `botster-core-test-support` only if reusable conformance helpers are needed.
- Rust `botster-core-dev` smoke harness only if a tiny change is needed to prove the public runtime path is reachable.

Worktree/target assumptions:

- Implementers must work in this assigned pipeline worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The run is main-rooted. Do not branch or PR as a stacked change from `ticket_1780189402_540507`.

Pipeline gates/artifacts:

- This file is the repo-visible Plan artifact.
- Plan gate evidence should cite this file and the loaded vault/repo context.
- Checklist creation is currently blocked by the Project Pipelines SQLite lock; retry before later gates and attach the same evidence there once writes succeed.

## Assumptions And Unknowns

Assumptions:

- `botster-core` now owns reusable local process cleanup mechanics because the ticket explicitly says core defines and implements local cleanup mechanics.
- Hub/product crates still own restart, retention, recovery, UX messaging, admission, persistence, and retry policy.
- The existing `SessionRuntime` trait is the correct public entry point for local process spawn, but it is not the currently wired engine shutdown seam.
- The currently wired engine shutdown seam is `SessionWorkerRuntime::shutdown`. The plan must implement cleanup there through a paired worker runtime.
- Local process cleanup can be implemented without bringing Tokio into `botster-core`.
- A small platform-specific Unix implementation is acceptable when paired with `#[cfg(unix)]` tests and clear unsupported-platform behavior.
- Unsupported platforms should fail explicitly with typed runtime errors or expose a non-process-group fallback only if it can still satisfy the current platform's guarantees. Do not pretend process-group cleanup is available where it is not.
- If a new dependency is required for signals or process-group syscalls, the implementer must verify the latest stable crate version before adding it. Prefer the smallest dependency surface that expresses the OS primitive cleanly.
- Test commands should use synthetic commands such as `sh -c 'sleep ... &'` or a test helper binary, never real agent commands or user data.
- Shutdown through the engine may block synchronously for the configured grace window. The production default must be small and bounded, and tests must use a short injected grace value.

Unknowns for implementation:

- Exact public names: prefer direct paired names such as `LocalProcessSessionRuntime` and `LocalProcessSessionWorkerRuntime`, plus a shared registry/handle if needed.
- Exact bridging API. Prefer a small constructor/factory that lets hosts pass the spawn-side runtime to `BotsterEngine` or `MultiplexerEngine` and pass a worker runtime derived from the same shared registry at `spawn_session` time.
- Whether `SessionWorkerRuntime::shutdown` should return a typed shutdown result/status, or whether the paired worker runtime should enqueue a `SessionWorkerRuntimeEvent::ProcessExited` for later draining. The engine-routed proof must decide this; do not leave status available only through `SessionRuntime::drain_output`.
- Exact typed error inventory. Likely additions include `ShutdownFailed`, `CleanupFailed`, or `ExitStatusFailed`; add only variants with tests and clear call sites.
- Whether process status should capture signal values on Unix after forced kill. Prefer filling `ProcessExitedPayload.signal` where the platform exposes it.
- Whether a minimal local runtime can omit PTY I/O initially. If so, document that this ticket proves local process cleanup and status capture, while PTY byte plumbing remains host-adapter work.

No human question is blocking this plan. The ticket intent is clear enough: add core-owned local cleanup mechanics while keeping hub policy out.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/runtime/mod.rs`
  - Export paired local process runtime types and any shutdown option/status/error types that are public.
  - Add focused `SessionRuntimeErrorKind` variants if needed.
- `crates/botster-core/src/runtime/local_process.rs` or equivalent
  - Concrete spawn runtime, paired worker runtime, shared process registry, shutdown state, process-group cleanup, drop cleanup, and platform gates.
- `crates/botster-core/src/engine/session_worker.rs`
  - Required if the trait needs to return or surface shutdown status/errors from `SessionWorkerRuntime::shutdown`.
- `crates/botster-core/src/engine/multiplexer.rs`
  - Required if `ProcessExited` or typed shutdown errors need to be routed from worker shutdown into lifecycle state during `shutdown_session`.
- `crates/botster-core/src/engine/botster.rs`
  - Required if the ergonomic facade needs a helper or test path that proves shutdown uses the paired local process worker runtime.
- `crates/botster-core/src/lib.rs`
  - Re-export the public local process runtime types if they are intended for embedders.
- `crates/botster-core/tests/session_runtime_contract_test.rs`
  - Extend contract tests for shutdown idempotence, typed errors, and exit-status serialization if these remain fake-runtime-level contracts.
- `crates/botster-core/tests/local_process_runtime_test.rs`
  - New platform-gated integration tests for real local child process cleanup and process status capture.
- `crates/botster-core/Cargo.toml`
  - Only if a minimal OS primitive crate is required; verify the latest stable version first.
- `README.md`
  - Optional narrow update if a public local process runtime becomes part of the embeddable core API.
- `docs/plans/process-group-cleanup-shutdown-guarantees.md`
  - This plan artifact.

Possible but avoid unless necessary:

- `crates/botster-core-dev/src/lib.rs`
- `crates/botster-core-dev/tests/engine_smoke_test.rs`
- `crates/botster-core-test-support/src/fake/mod.rs`

Not expected:

- SPA, TUI, Rails, Lua plugin, MCP, Project Pipelines plugin, or old trybotster product files.

## Implementation Shape

Suggested minimal shape:

- Add a local process runtime pair:
  - `LocalProcessSessionRuntime` implements `SessionRuntime` for spawn and any direct embedder input/output methods.
  - `LocalProcessSessionWorkerRuntime` implements `SessionWorkerRuntime` for the currently wired engine request path.
  - Both share one process registry, keyed by `SessionId`.
- Maintain a per-session registry with:
  - `SessionId`
  - child process handle
  - process id
  - Unix process-group id when available
  - shutdown state such as running, terminating, exited
  - queued `SessionRuntimeOutput` values
- `LocalProcessSessionRuntime::worker_runtime()` or equivalent:
  - returns a worker runtime handle backed by the same registry;
  - is the object passed as `worker_runtime` to `MultiplexerEngine::spawn_session` or `BotsterEngine::spawn_session`.
- `spawn_session`:
  - uses `SessionSpawnRequest` exactly as supplied;
  - sets explicit args, cwd, env vars, and optional PTY size only where the local process runtime supports them;
  - on Unix, creates a separate process group for the child;
  - returns `SessionRuntimeHandle` with OS pid and a stable runtime id.
- `SessionWorkerRuntime::shutdown(session_id, reason)` on the paired worker runtime:
  - no-ops successfully for already-exited/already-shutdown sessions;
  - sends graceful termination to the process group where available;
  - waits or polls for exit within a small bounded grace window;
  - sends forced termination to the process group if the child is still alive;
  - records or returns `ProcessExited` with exit code or signal;
  - returns typed errors for OS failures that genuinely prevent cleanup or status collection.
- `send_input(SessionRuntimeInput::Shutdown { session_id })`:
  - may delegate to the same shared cleanup primitive for direct embedders;
  - must not be the only cleanup path, because the engine does not currently call it.
- `drain_output`:
  - returns queued `ProcessExited` events and any supported output events;
  - returns `SessionNotFound` for unknown sessions, matching existing contract behavior.
- `Drop`:
  - attempts best-effort cleanup of live sessions without panicking.
  - must not hide explicit shutdown errors during normal API calls.

Runtime/user path proof:

- The changed production core entry point is `BotsterEngine::shutdown_session` or `MultiplexerEngine::shutdown_session` using a paired local process worker runtime.
- Evidence that helper functions exist is not enough. Tests must spawn a representative local child through the public engine path, request shutdown through the engine facade or multiplexer, and verify the process is not left alive.
- Direct tests of `LocalProcessSessionRuntime` are still useful for edge cases, but they do not satisfy the runtime-path proof by themselves.

## Risks

- Killing only the session leader can leave background grandchildren alive. Mitigation: process-group creation plus group termination tests.
- Closing stdio or a future PTY master may only send `SIGHUP`, which long-running tools can ignore. Mitigation: explicit graceful and forced process-group cleanup.
- A broad supervisor abstraction could absorb hub restart/retention policy into core. Mitigation: keep this to local process mechanics and typed errors.
- Implementing only `SessionRuntime::send_input(Shutdown)` would be unwired from the current engine shutdown path. Mitigation: paired `SessionWorkerRuntime` implementation plus mandatory engine proof test.
- Process-exit status can be captured in `SessionRuntimeOutput` but never observed by engine lifecycle if it only appears behind `drain_output`. Mitigation: require `ProcessExited` routing through `SessionWorkerEngine`/`MultiplexerEngine`.
- Forced cleanup tests can become flaky if they rely on long sleeps or wall-clock races. Mitigation: short bounded grace values and polling with clear platform gates.
- Synchronous shutdown blocks the engine caller for the grace window. Mitigation: small production grace default, injected short test grace, and polling/yielding instead of one long sleep.
- Drop-based cleanup can mask errors. Mitigation: explicit shutdown path returns typed errors; `Drop` is best-effort and panic-free.
- Process-exit status can be lost if cleanup removes registry state before queuing `ProcessExited`. Mitigation: test exit status after natural, graceful, and forced termination.
- Platform APIs differ. Mitigation: `#[cfg(unix)]` process-group tests and explicit unsupported-platform behavior.
- Adding an OS crate without verifying current stable versions violates dependency convention. Mitigation: verify before changing `Cargo.toml`.
- Real process tests can leak if they fail mid-assertion. Mitigation: use guards that kill process groups in test teardown.
- PII can leak through cwd/env snapshots in test failure output. Mitigation: synthetic temp dirs and bounded synthetic env vars only.

## Acceptance Checks / Tests

Required targeted tests:

1. `local_process_runtime_spawns_and_captures_process_exit_status`
   - Spawn a short-lived synthetic command through the public local runtime.
   - Drain output and assert `SessionRuntimeOutput::ProcessExited` includes the expected exit code.

2. `local_process_runtime_graceful_shutdown_records_exit`
   - Spawn a command that exits on graceful termination.
   - Send `SessionRuntimeInput::Shutdown`.
   - Assert shutdown succeeds, exit is recorded, and the process is gone.

3. `local_process_runtime_forced_shutdown_kills_ignoring_child_group`
   - Unix-gated.
   - Spawn a representative command that ignores graceful termination and starts a child in the same process group.
   - Send shutdown with a short test grace window.
   - Assert forced cleanup occurs, status is surfaced, and neither parent nor child remains alive.

4. `local_process_runtime_shutdown_is_idempotent`
   - Call shutdown repeatedly for the same session.
   - Assert no duplicate failure and no duplicate process cleanup race.

5. `local_process_runtime_drop_cleans_live_child_group`
   - Unix-gated.
   - Spawn a long-running representative child.
   - Drop the runtime or session owner without explicit shutdown.
   - Assert the child process group is gone.

6. `local_process_runtime_unknown_session_shutdown_returns_typed_error`
   - Send shutdown for an unknown session.
   - Assert `SessionRuntimeErrorKind::SessionNotFound`.

7. `session_runtime_error_kinds_pin_shutdown_variants`
   - If new error kinds are added, pin serde names in the existing runtime error kind serialization test.

8. `botster_engine_shutdown_uses_runtime_cleanup_path`
   - Mandatory.
   - Spawn a session through `BotsterEngine` or `MultiplexerEngine` using the local process runtime pair.
   - Shut it down through the public engine API.
   - Assert the paired `SessionWorkerRuntime::shutdown` path performed process-group cleanup.
   - Assert lifecycle observes `ProcessExited` or the final status path selected by the implementation.
   - Assert no parent or representative child remains alive.

9. `multiplexer_shutdown_status_updates_session_lifecycle`
   - Mandatory if the implementation changes `SessionWorkerRuntime::shutdown` or worker event routing.
   - Drive shutdown through `MultiplexerEngine::shutdown_session`.
   - Assert exit status reaches `SessionLifecycleState::Exited` or a documented stopping/exited sequence, not only a raw runtime queue.

Required commands:

- `cargo fmt`
- `cargo test -p botster-core session_runtime`
- `cargo test -p botster-core local_process`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Gate evidence should include raw cargo output and exit status, not summarized RTK prose.

## Vault Checklist Evidence

- Vault/project notes constrained the plan: `planner-playbook`, `botster-planner-playbook`, `botster-architecture`, `cli-patterns`, `spa-patterns`, Project Pipelines orchestration/workbench/UI-contract notes, explicit target/worktree orchestration notes, `pty master fd close sends sighup but ignores it needs killpg`, `botster runtime now uses broker-authoritative pty lifecycle with unified session registry`, `process-terminating defaults must expose an injectable callback seam for tests`, and `botster pipeline reviewers must bypass rtk summaries for cargo gate evidence`.
- Convention conflicts: none. Plan Review identified an unresolved architecture seam, and this revision resolves it by wiring cleanup through `SessionWorkerRuntime::shutdown` while preserving hub ownership of user-facing lifecycle policy.
- Verification evidence so far: planning inspection only; no implementation verification commands were run. Planned commands are listed above.
- Durable knowledge capture: capture after implementation if the final API establishes a reusable convention for local process runtime cleanup, the `SessionRuntime` vs `SessionWorkerRuntime` split, process-group platform behavior, or shutdown error vocabulary.

## Vault Gaps Worth Capturing

- Capture a Botster architecture note if the final implementation establishes `LocalProcessSessionRuntime` or equivalent as the durable core-owned local process primitive.
- Capture the `SessionRuntime` vs `SessionWorkerRuntime` ownership split once the implementation proves the bridge. The distinction is non-obvious and caused the first plan review failure.
- Capture a testing convention if the forced cleanup path settles a reliable short-grace process-group test pattern.
- Capture an error-vocabulary note if new `SessionRuntimeErrorKind` shutdown variants become the reusable standard for runtime/supervisor failures.
