# Build Core Session Worker Engine

## Context Loaded

- Pipeline context: `ticket_1780075965_592896`, `run_1780082761_929787`, current step `botster_plan`, gate `botster_plan_gate`.
- Orchestrator correction: this run targets `main`; stale dependency-derived base fields in pipeline context must not make this a stacked PR.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Additional vault constraints loaded:
  - `identity`
  - `goals`
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
  - `plan steps need reviewable plan artifacts`
- Repo context loaded:
  - `README.md`: core owns reusable mechanisms and transport-neutral contracts; hub owns runtime policy, lifecycle, orchestration, adapters, and product workflows.
  - `crates/botster-core/src/contract/actor.rs`: existing `SessionIoRequest`, `SessionIoEvent`, bounded queue/backpressure, initial snapshot barrier, coalescing policy, and process-exit contracts.
  - `crates/botster-core/src/contract/client_stream.rs`: existing synchronous client stream harness that routes transport ingress and session events.
  - `crates/botster-core/src/engine/mod.rs`: currently only the engine module shell.
  - `crates/botster-core/src/runtime/mod.rs`: currently only the host-runtime interface shell.
  - `crates/botster-core-test-support/src/fake/mod.rs`: fake-runtime support shell.
  - `crates/botster-core/tests/session_io_mailbox_test.rs`: current mailbox shape tests and helper-only fake harness.
  - `crates/botster-core/tests/client_stream_contract_test.rs`: current client stream behavior tests.
  - `docs/archive/plans/actor-contract-types.md` and `docs/archive/plans/client-stream-contract.md`: prior dependency plans.
- Reference evidence inspected from old trybotster only as evidence, not as source to copy:
  - session I/O worker contract and runtime concepts: input, resize, snapshots, initial snapshot barrier, shutdown, exit propagation, backpressure, and `last_output_at`.
  - reference path lookup found one missing old file, confirming old paths are non-authoritative and must not become dependencies.

## Scope

Build the first reusable `botster-core` session worker engine that consumes existing `SessionIoRequest` values and emits `SessionIoEvent` values plus activity/backpressure observations through public core contracts.

In scope:

- Add a small pure/synchronous session worker engine under `crates/botster-core/src/engine/`.
- Define host-supplied runtime traits or command/event structs needed by the engine to write PTY input, resize, request snapshots, shut down, and observe process output/exit without depending on Tokio, hub handles, Unix sockets, WebRTC, TUI, Rails, or Lua.
- Route `PtyInput`, `Resize`, `GetSnapshot`, `GetInitialSnapshot`, `Shutdown`, and already-modeled helper requests through the engine.
- Preserve snapshot-before-live-output ordering for initial attach by using or extending the existing `InitialSnapshotBarrier`.
- Flush pending live output before process-exit and shutdown events.
- Report mailbox send failures for queue-full and queue-closed cases with existing `MailboxSendFailure` and `BackpressureSummary`/route context.
- Track session output activity timestamp updates from live output events, independent of client attachment.
- Add fake runtime/test support that proves behavior through the engine path rather than through ad hoc test-only harnesses.
- Export the new engine types from `crates/botster-core/src/lib.rs` if they are part of the public reusable contract.

Non-scope:

- No concrete process spawning, Unix socket protocol implementation, Tokio worker loops, mpsc channels, thread supervision, or real PTY ownership.
- No hub policy for restart, retention, recovery, auth, admitted targets, UI meaning, cloud sync, or product configuration.
- No WebRTC, browser, TUI, ActionCable, Rails, Project Pipelines, Lua plugin, or MCP implementation changes.
- No broad refactor of existing actor/client-stream contracts beyond changes directly needed for the engine.
- No compatibility branches or version-suffixed duplicate APIs.
- No copying old trybotster runtime code into core.

Botster layer touched: Rust `botster-core` engine and test-support layers, specifically the session/client data-plane mechanism. No plugin, SPA, TUI, Rails relay, or MCP layer changes are planned.

Worktree/target assumption: downstream agents operate in the assigned `botster-core` pipeline worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts: this repo-visible plan document is the Plan artifact. Gate evidence should cite this file plus the loaded vault/repo context.

## Assumptions And Unknowns

Assumptions:

- `botster-core` should own deterministic engine mechanics, not executable runtime policy.
- A synchronous fake runtime is enough to prove reusable behavior for this ticket; production host crates can adapt it to async/task boundaries later.
- Existing dependencies should be sufficient. Do not add Tokio or channel crates to `botster-core`.
- Existing request/event contracts are mostly sufficient; prefer small additions to engine observation/result types over reshaping public actor enums.
- Activity timestamp can be represented with a host-supplied monotonic or millisecond clock abstraction, avoiding `Instant` serialization and wall-clock policy in public wire shapes.
- Queue-full and queue-closed acceptance can be proven with a bounded in-memory engine mailbox or fake sender owned by test support, using existing failure reason types.
- Snapshot-before-live-output must be proven through actual engine request/event flow, not only the existing standalone `InitialSnapshotBarrier` unit test.

Unknowns for implementation:

- Exact naming is open. Prefer direct names such as `SessionWorkerEngine`, `SessionWorkerRuntime`, `SessionWorkerOutcome`, and `SessionActivity`.
- Whether fake runtime belongs in `botster-core-test-support/src/fake/session_worker.rs` or in the core test module. Prefer test-support if downstream consumers will reuse conformance helpers.
- Whether `GetInitialSnapshot` should become a first-class engine method or remain a `SessionIoRequest` branch that produces `InitialSnapshotReady` plus buffered output.
- Whether send-file and prepared snapshot helper behavior should remain helper-level in this ticket or be routed through the same engine result type. Keep it only if needed to avoid regressing existing mailbox tests.

No human question is needed before implementation. The ticket intent is clear: move from contract shapes to deterministic reusable session worker behavior while keeping concrete host policy outside core.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/mod.rs`
  - Export the new engine module.
- `crates/botster-core/src/engine/session_worker.rs`
  - New pure engine state, runtime trait/adapter boundary, request handling, output buffering, activity timestamp, shutdown, and backpressure observation.
- `crates/botster-core/src/lib.rs`
  - Public exports for reusable engine types if needed by host crates.
- `crates/botster-core-test-support/src/fake/mod.rs`
  - Export fake session runtime helpers.
- `crates/botster-core-test-support/src/fake/session_worker.rs`
  - Fake runtime/mailbox helpers for conformance tests.
- `crates/botster-core/tests/session_worker_engine_test.rs`
  - New behavior tests that drive the engine.
- `crates/botster-core/tests/session_io_mailbox_test.rs`
  - Possible small cleanup to replace the local ad hoc harness with shared fake support where it directly overlaps.
- `docs/archive/plans/core-session-worker-engine.md`
  - This plan artifact.

Possible but not expected:

- `crates/botster-core/src/contract/actor.rs`
  - Only if a minimal public activity or engine observation type clearly belongs beside existing actor contracts.
- `crates/botster-core/Cargo.toml`
  - Avoid unless a compiler-proven need appears; no new dependency is expected.

Not expected:

- `README.md`, unless implementation adds a new public engine guarantee that needs top-level boundary documentation.
- `crates/botster-core-dev`.
- Any concrete host/runtime repo or old trybotster source.

## Implementation Shape

Suggested minimal shape:

- `SessionWorkerEngine::new(session_id, runtime, clock_or_now_fn)` or equivalent.
- `handle_request(SessionIoRequest) -> SessionWorkerOutcome`.
- `handle_runtime_event(...) -> SessionWorkerOutcome` for live output, snapshot response, process exit, EOF/desync, and runtime errors.
- `SessionWorkerOutcome` with:
  - `events: Vec<SessionIoEvent>`
  - `runtime_commands: Vec<...>` or calls recorded through a fake runtime
  - `backpressure: Vec<BackpressureSummary>` or `MailboxSendFailure` values where relevant
  - `activity: Option<...>` or a readable `last_output_at` value
- A bounded fake mailbox helper that distinguishes `QueueFull` and `QueueClosed`.

Behavior details:

- `PtyInput` records a runtime write command with the exact byte payload.
- `Resize` records runtime resize with rows/cols and ensures a following snapshot observes the resized dimensions in fake runtime tests.
- `GetSnapshot` records or produces a snapshot response correlated by `RequestId`.
- `GetInitialSnapshot` gates live output until `InitialSnapshotReady`, then releases buffered live output in order.
- Live output updates `last_output_at` and emits `TerminalBytes` only after the initial snapshot barrier allows it.
- Process exit flushes pending output before `ProcessExited`.
- Shutdown flushes pending output before `Shutdown`, marks the engine closed, and prevents later routing.
- Closed/full mailbox send attempts return typed failures and do not silently drop requests.

## Risks

- Building real async process supervision would violate the core/hub boundary.
- A fake-only harness that bypasses the exported engine would repeat the current shape-test gap.
- Activity timestamps can accidentally become host policy if they use uncontrolled wall-clock behavior in tests.
- Snapshot ordering can regress if the implementation emits live output directly from runtime events before the initial snapshot barrier is satisfied.
- Process-exit propagation can lose trailing output unless ordered events force a flush.
- Backpressure can become vague if represented only as strings instead of preserving typed route/source/reason context.
- Adding concrete terms like WebRTC, browser, TUI, ActionCable, Rails, or Lua to engine contracts would break the reusable core boundary.
- Over-generalizing the runtime abstraction before there is a host integration would create speculative API surface.

## Acceptance Checks / Tests

Run:

- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Add targeted tests in `crates/botster-core/tests/session_worker_engine_test.rs`:

1. `session_worker_routes_input_writes`
   - Drive `SessionIoRequest::PtyInput` through the engine.
   - Assert fake runtime receives exactly the input bytes and no unrelated event is emitted.

2. `session_worker_routes_resize_before_snapshot`
   - Drive `Resize`, then `GetSnapshot`.
   - Assert fake runtime records the resize and the emitted `SnapshotReady` carries the resized rows/cols.

3. `initial_snapshot_precedes_live_output_through_engine`
   - Subscribe/request initial snapshot, feed live output before snapshot readiness, then resolve the snapshot.
   - Assert emitted events are `InitialSnapshotReady` followed by buffered `TerminalBytes` in order.

4. `process_exit_flushes_pending_output_before_exit_event`
   - Buffer/coalesce output, then feed process exit.
   - Assert `TerminalBytes` precedes `ProcessExited`.

5. `shutdown_flushes_and_closes_engine`
   - Buffer output, send shutdown, assert output precedes `Shutdown`.
   - Assert later input/output is rejected or ignored with a typed closed observation.

6. `mailbox_failures_report_queue_full_and_closed`
   - Use fake bounded mailbox capacity zero and a closed mailbox.
   - Assert `MailboxSendFailureReason::QueueFull` and `QueueClosed` with `QueueSource::SessionIo`.

7. `live_output_updates_activity_timestamp`
   - Feed live output with a deterministic fake clock.
   - Assert engine activity/last-output timestamp changes, independent of client subscription state.

8. `engine_contract_excludes_concrete_host_policy`
   - Source-level or type-name guard that the new engine module does not mention WebRTC, browser, TUI, ActionCable, Rails, auth, retention, restart strategy, cloud sync, or product config.

Existing tests expected to remain green:

- `crates/botster-core/tests/session_io_mailbox_test.rs`
- `crates/botster-core/tests/client_stream_contract_test.rs`
- `crates/botster-core/tests/actor_contract_test.rs`
- all current workspace tests under `cargo test`

Runtime/user path proof:

- This ticket is intentionally core-engine scaffold, not host wiring.
- The changed executable path is the exported engine and fake-runtime test path: tests must instantiate the public engine, feed real `SessionIoRequest` values, and observe real `SessionIoEvent`, activity, and failure outputs from that engine.
- Evidence that contract enums exist is not enough; acceptance requires behavior through `SessionWorkerEngine` or its final chosen public equivalent.

## Vault Gaps Worth Capturing

No durable vault gap must be captured before implementation.

Potential capture after implementation:

- A stable rule for how `botster-core` represents session activity timestamps, if the implementation settles a reusable clock/activity vocabulary.
- A stable rule for session worker engine API shape, if it becomes the template for future core worker engines.
