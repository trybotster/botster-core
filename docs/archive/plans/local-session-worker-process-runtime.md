# Local Session Worker Process Runtime

Ticket: `ticket_1780532710_740401`
Run: `run_1780535236_279478`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket, run, current Plan step, Plan gate, closed dependency, events, artifacts, findings, questions, and prior answers. No open questions, findings, reviews, or prior answers are present for this run.
- Closed dependency loaded from context: `ticket_1780532685_767820` / `Define durable session worker protocol and restart contract`.
- Required playbooks loaded:
  - [[planner-playbook]]
  - [[botster-planner-playbook]]
- Required vault/project notes loaded:
  - [[identity]]
  - [[goals]]
  - [[botster-architecture]]
  - [[cli-patterns]]
  - [[spa-patterns]]
  - [[project pipeline orchestration belongs in a device-level botster plugin]]
  - [[project pipelines needs an operator workbench not more primitives]]
  - [[project pipelines ui contract belongs in the plugin readme]]
  - [[botster orchestration should spawn agents with explicit target ids]]
  - [[botster orchestration prompts must bind agents to explicit worktrees]]
  - [[plan agents must author vault context as wikilinks not home paths]]
- Repo context inspected:
  - `Cargo.toml`: workspace currently has `botster-core`, `botster-core-dev`, `botster-core-test-support`, and `botster-terminal-ghostty`.
  - `crates/botster-core/src/contract/session_protocol.rs`: existing transport-neutral frame constants, frame codec, handshake helpers, and payload contracts for input, resize, shutdown, ping/pong, process exit, snapshots, terminal metadata, `FRAME_SET_TIMEOUT`, `TimeoutPayload`, and `SessionMetadata.recovery_identity`.
  - `crates/botster-core/src/runtime/local_process.rs`: existing local PTY-backed runtime owns child processes and PTY handles in-process, supports input, resize, bounded reader backpressure, output drain, and process-group cleanup.
  - `crates/botster-core/src/runtime/mod.rs`: current `SessionRuntime` trait is in-process; outputs include PTY bytes, process exit, and typed backpressure.
  - `crates/botster-core/src/engine/session_worker.rs`: current reusable session worker routes typed `SessionIoRequest` values to a `SessionWorkerRuntime` adapter.
  - `crates/botster-core/src/engine/managed_session_runtime.rs`: current production-shaped public core path is `ManagedSessionRuntime` over `LocalProcessRuntime`; it drains local runtime output into worker events and subscription fanout.
  - `crates/botster-core/src/engine/botster.rs`: `DefaultBotsterEngine` exposes the local process facade over `ManagedSessionRuntime<LocalProcessRuntime>`.
  - `crates/botster-core/tests/local_process_runtime_test.rs`: existing real local PTY tests cover output, input, resize, process-group cleanup, idempotent shutdown, bounded reader pressure, and engine shutdown.
  - `crates/botster-core-test-support/src/conformance/mod.rs`: existing many-PTY harness exercises `ManagedSessionRuntime<LocalProcessRuntime>` and public `DefaultBotsterEngine` observations.
  - Existing plan artifacts loaded: `docs/archive/plans/session-process-wire-protocol.md`, `docs/archive/plans/core-session-worker-engine.md`, `docs/archive/plans/process-group-cleanup-shutdown-guarantees.md`, and `docs/archive/plans/bound-pty-output-queues-and-surface-backpressure.md`.
- Plan Review context loaded:
  - `review_1780535764_213649` returned `changes_required`.
  - Open findings resolved in this revision: reconcile with merged `FRAME_SET_TIMEOUT`/`recovery_identity` contract, choose attach/detach semantics, combine bounded-consumer ordering and non-blocking proof, and close the scaffold-only public-path loophole.
- Project Pipelines checklist discipline:
  - Checklist instructions loaded.
  - Run-level vault checklist created: `checklist_1780535284_826591`.
  - This artifact records notes read, convention fit, verification expectations, and capture decision so gate/review evidence remains durable even if checklist tools are slow.

## Scope

Implement the first production-shaped local session worker process runtime for `botster-core`.

In scope:

- Add a small binary/crate structure for a local worker process. Prefer a workspace member such as `crates/botster-session-worker` or a focused binary target whose only job is to host one local session worker process.
- Reuse the existing `botster-core` contracts and mechanics:
  - `session_protocol` frame constants, frame codec, handshake helpers, and typed payloads.
  - `FRAME_SET_TIMEOUT`, `TimeoutPayload`, and `SessionMetadata.recovery_identity` as the merged durable/restart contract. There is no separate merged restart-contract API to implement in this repo beyond these primitives; hub/host code owns restart policy.
  - `LocalProcessRuntime` PTY/process mechanics, bounded reader pressure, resize/input/shutdown behavior, and process-group cleanup.
  - Existing `SessionRuntimeOutput` and `BackpressureSummary` types for output, exit, heartbeat/health evidence, and pressure.
- Define a narrow host-side process adapter in `botster-core` that lets `ManagedSessionRuntime` or `DefaultBotsterEngine` use a worker process instead of owning live PTY handles directly.
- The worker process should own exactly the live PTY, child process, reader thread, writer, and cleanup state for a spawned local session.
- Core/hub side should communicate with the worker through `session_protocol` for input, resize, health, shutdown, and reconnect timeout:
  - `FRAME_PTY_INPUT` / `FRAME_RESIZE` / `FRAME_PING` / `FRAME_PONG` / `FRAME_SHUTDOWN` / `FRAME_SET_TIMEOUT`.
  - The worker welcome/metadata path must expose the birth-time `recovery_identity` from `SessionMetadata`.
- Attach/detach is a parent-side consumer registration decision over worker egress fanout, not new protocol frame constants in this ticket. The worker accepts attach/detach commands through the parent/core process client API; the client updates consumer registration and egress delivery state while the worker PTY owner continues running.
- Worker egress must emit bounded output/event frames and heartbeat/health evidence. Slow or disconnected consumers must not block the PTY owner indefinitely.
- Preserve retained PTY byte ordering. If any drop/coalescing behavior is introduced for events, the retained/dropped semantics must be typed and tested.
- Shutdown must clean up child process groups correctly through the same core local-process cleanup primitive, avoiding orphaned child processes.
- Add a deterministic test or harness that starts a worker for a simple command, observes output, sends input where applicable, resizes, observes health/heartbeat, and shuts down cleanly.
- Add a bounded-consumer regression that proves slow or disconnected consumers do not block the worker indefinitely and that pressure is observable.
- Document the worker process boundary so future hub/core agents know that the worker is the durable local session owner.

Non-scope:

- No Botster CLI/hub product integration beyond the minimal core-facing adapter needed to prove the changed path in this repo.
- No Project Pipelines plugin, Lua plugin, MCP, Rails, WebRTC, browser SPA, or TUI changes.
- No broad rewrite of `SessionWorkerEngine`, `ManagedSessionRuntime`, `DefaultBotsterEngine`, terminal screen state, subscription multiplexer, or Ghostty adapter mechanics.
- No new restart/recovery policy, target admission policy, workspace persistence, reconnect UX, or daemon lifecycle policy. This ticket implements the merged restart primitives (`recovery_identity` and `FRAME_SET_TIMEOUT`) and leaves policy decisions in hub/host code.
- No replacement of `botster-terminal-ghostty`; Ghostty remains a concrete terminal backend crate and should not be pulled into the worker unless the current core test path already requires it.
- No speculative configuration surface beyond the capacities/timeouts needed for deterministic tests.
- No PII in logs, fixtures, docs, test command output, or failure messages. Use synthetic ids, synthetic command strings, and path-neutral docs.

Botster layers touched:

- Rust `botster-core` runtime/session process boundary: primary.
- Rust local worker process crate or binary: primary.
- Rust managed/default engine path: required if it is the host-visible production entry point.
- Rust test-support/conformance harness: likely.
- Docs/plans or README/rustdoc: required for the worker boundary.

Worktree and target assumptions:

- Implementers work only in this pipeline-assigned worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The run targets `main`.
- The dependency is closed and its contract is visible in the current worktree. Do not add parallel protocol/restart primitives for behavior already covered by `SessionMetadata.recovery_identity` and `FRAME_SET_TIMEOUT`.

Pipeline gates/artifacts:

- Plan artifact: `docs/archive/plans/local-session-worker-process-runtime.md`.
- Plan gate evidence should cite this artifact and checklist `checklist_1780535284_826591`.
- Advancement target: `botster_plan_review`.

## Assumptions And Unknowns

Assumptions:

- "Local session worker process" means a separate OS process that owns a session's PTY and child process, not another in-process Rust worker struct.
- The worker should reuse the existing `LocalProcessRuntime` as the PTY/process implementation instead of duplicating spawn, resize, read, bounded queue, and process-group cleanup code.
- The smallest acceptable process topology is one worker process per local session. If the dependency contract explicitly chose a multi-session worker process, the implementer must follow that contract and update this assumption in implementation evidence.
- The core-side adapter can be scheduling-neutral and synchronous for tests, but the runtime path must cross an OS process boundary.
- `session_protocol` is the wire foundation for this ticket. Do not invent a parallel JSON command protocol.
- `SessionMetadata.recovery_identity` is the durable worker identity carried at handshake/welcome time.
- `FRAME_SET_TIMEOUT` / `TimeoutPayload` is the reconnect-timeout/disconnect-grace primitive. It is a core recovery tactic, not hub restart policy.
- Attach/detach in this slice is parent-side consumer registration and output delivery state around the worker egress path. Do not add `FRAME_ATTACH` or `FRAME_DETACH` in this ticket.
- Heartbeat/health evidence should be typed and observable through tests. A ping/pong frame or health command response is enough if it proves the worker event loop is alive.
- Bounded output should use an explicit queue capacity and typed pressure. Blocking the PTY reader forever because a parent process stops reading is not acceptable.

Unknowns for implementation:

- Exact crate/binary naming. Prefer a direct name such as `botster-session-worker` over a vague "daemon" or "broker" name.
- Whether the host-side adapter should implement `SessionRuntime` directly, or whether a small process client wraps the worker and is then composed by `ManagedSessionRuntime`.
- How the worker should delimit startup handshake versus spawn command. Prefer one explicit handshake followed by a typed spawn/config payload if the process is launched before the child.
- Exact bounded egress semantics when the parent disconnects, within the existing contract: use `FRAME_SET_TIMEOUT` to configure reconnect/disconnect grace and typed pressure/overflow accounting for bounded output retention. Do not add a parallel disconnect-grace mechanism.
- Whether tests should spawn the worker binary through `std::process::Command` or through a small library harness that returns a child handle plus protocol client. Acceptance requires a real process boundary either way.

No human question is blocking planning. The ticket intent has one clear reading after repo inspection: move local PTY ownership out of the core/hub process and prove core communicates with that owner through a bounded protocol path.

## Affected Surfaces / Files

Expected:

- `Cargo.toml`
  - Add the worker crate or binary workspace member if a new crate is chosen.
- `crates/botster-session-worker/Cargo.toml` and `crates/botster-session-worker/src/main.rs`, or an equivalent focused binary target
  - Worker process entrypoint.
  - Protocol handshake/read loop.
  - Command handling for spawn/config if needed, input, resize, attach, detach, health, and shutdown.
  - Bounded output/event delivery and heartbeat/health emission.
- `crates/botster-session-worker/src/lib.rs` or focused modules
  - Testable worker runtime loop and protocol adapter if keeping `main.rs` thin.
- `crates/botster-core/src/runtime/`
  - Add a host-side worker process client/adapter that implements or composes with `SessionRuntime`.
  - Keep `LocalProcessRuntime` as the in-worker PTY owner rather than exposing live handles to parent core code.
- `crates/botster-core/src/contract/session_protocol.rs`
  - Expected to stay mostly unchanged for restart/health because `FRAME_SET_TIMEOUT`, `TimeoutPayload`, `FRAME_PING`, `FRAME_PONG`, and `recovery_identity` already exist.
  - Do not add attach/detach frame constants for this ticket; model attach/detach in the parent/core process client.
  - Avoid a second protocol vocabulary.
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Add or update a named worker-backed public constructor/path, for example `ManagedSessionRuntime::with_worker_process`, if this is the chosen public entry point.
- `crates/botster-core/src/engine/botster.rs`
  - Add a clearly named worker-backed public constructor/path, for example `DefaultBotsterEngine::worker_backed`, or deliberately make `DefaultBotsterEngine::new()` worker-backed.
  - If `DefaultBotsterEngine::new()` remains in-process for compatibility, this plan still requires the named worker-backed public constructor and tests; a crate-internal harness is not enough.
- `crates/botster-core-test-support/src/conformance/mod.rs`
  - Add or extend a worker-backed harness if shared downstream evidence is useful.
- Tests:
  - `crates/botster-core/tests/local_session_worker_process_test.rs` or worker-crate integration tests for process-boundary behavior.
  - Existing `local_process_runtime_test.rs` should remain as lower-level PTY/process cleanup coverage.
  - `managed_session_runtime_test.rs` or `botster_engine_api_test.rs` if the public production path changes.
- Docs:
  - `README.md` or a focused architecture doc if needed to document that the local worker process is the durable session owner.
  - This plan artifact.

Likely unchanged:

- `crates/botster-terminal-ghostty`: no concrete Ghostty work is required unless tests need the existing optional terminal backend.
- Browser/TUI/Rails/Lua/Project Pipelines surfaces.
- Existing session worker contract tests except for additions needed by new protocol payloads.

## Implementation Shape

Suggested sequence:

1. Use the merged `session_protocol` contract as authoritative:
   - `SessionMetadata.recovery_identity` is the durable identity carried at worker welcome/metadata time.
   - `FRAME_SET_TIMEOUT` / `TimeoutPayload` is the reconnect-timeout/disconnect-grace primitive.
   - `FRAME_PING` / `FRAME_PONG` is the health/heartbeat primitive.
   - Do not add a second restart/disconnect protocol.
2. Add a focused worker process target with a thin `main` and a testable worker loop.
3. In the worker loop:
   - perform protocol handshake;
   - construct `LocalProcessRuntime` with explicit options;
   - spawn exactly the requested command/session;
   - accept protocol frames for input, resize, health, reconnect timeout, and shutdown;
   - accept attach/detach through the parent/core process client as consumer registration/deregistration over worker egress, without new wire frame constants;
   - drain runtime output on a bounded cadence and send output/event frames;
   - emit typed health or heartbeat evidence;
   - cleanly terminate the local child/process group on shutdown, protocol EOF, or worker drop.
4. Add a parent/core process client:
   - launches the worker binary for a session;
   - owns only worker process IPC handles and session identity, not live PTY handles;
   - maps `SessionRuntimeInput` to protocol frames;
   - maps attach/detach calls to consumer registration state over worker egress;
   - maps worker output frames to `SessionRuntimeOutput`;
   - maps worker health/backpressure frames to typed observations.
5. Wire the changed runtime path into a public entry point:
   - either make `DefaultBotsterEngine::new()` use the worker-backed runtime, or add a named public worker-backed constructor such as `DefaultBotsterEngine::worker_backed` or `ManagedSessionRuntime::with_worker_process`;
   - if `DefaultBotsterEngine::new()` intentionally stays in-process, say so in docs and still wire and test the named public worker-backed constructor.
6. Add tests that prove the worker process boundary, not only type existence.

Runtime/user-path proof:

- The proof path must show a parent core object launching a separate worker process, sending command frames, receiving output/event frames, and shutting down through that protocol.
- Evidence that `LocalProcessRuntime` still works in-process is not sufficient.
- A crate-internal harness is not sufficient. The proof must exercise a named public `botster-core` entry point such as `DefaultBotsterEngine::worker_backed` or `ManagedSessionRuntime::with_worker_process`.
- If `DefaultBotsterEngine::new()` remains in-process intentionally, the implementation must document that compatibility choice and still provide the named worker-backed public constructor.

## Risks

- Leaving `DefaultBotsterEngine` or the tested production path on in-process `LocalProcessRuntime` would miss the ticket's core requirement that core/hub communicate with a separate PTY owner.
- Copying local PTY/process code into the worker would fork cleanup and backpressure behavior. Reuse `LocalProcessRuntime`.
- Inventing a second ad hoc JSON protocol would drift from `session_protocol` and the closed dependency contract.
- Inventing a parallel restart/disconnect-grace mechanism would drift from the merged `FRAME_SET_TIMEOUT` / `TimeoutPayload` contract.
- Adding attach/detach frame constants would over-expand protocol scope when parent-side consumer registration is sufficient for this ticket.
- Bounded output events can deadlock if pressure is reported through the same saturated queue. Keep control/health/pressure observable under saturation.
- Parent disconnect can leave the child process alive if EOF/drop cleanup is not explicit. Tests need disconnect/shutdown cleanup coverage.
- Process tests can leak children on failure. Use guards and short synthetic commands.
- Health/heartbeat can become string-only logs. It must be typed/test-observable and PII-free.
- Adding broad restart/recovery policy to core would violate the core-vs-hub boundary. The worker can expose state; hub/host decides policy.
- New crate dependency churn is possible. Prefer existing dependencies; if a new dependency is unavoidable, verify the current stable version before adding it.
- Absolute vault paths or local usernames in docs would violate plan artifact conventions. Use note titles/wikilinks only.

## Acceptance Checks / Tests

Required targeted checks:

- Worker process harness test:
  - Starts a real worker process for a synthetic command such as `sh -c 'printf ready; read line; printf \"$line\"'`.
  - Observes startup output through protocol frames.
  - Sends input through the parent/core protocol client and observes echoed output where applicable.
  - Sends resize and proves the worker handled it, either by command output (`stty size`) or typed acknowledgement.
  - Sends health or ping and observes typed heartbeat/health evidence.
  - Verifies worker welcome/metadata includes birth-time `recovery_identity`.
  - Sends `FRAME_SET_TIMEOUT` / `TimeoutPayload` and verifies the worker applies the reconnect timeout without creating a separate restart policy.
  - Sends shutdown and asserts process exit/cleanup with no orphan child.
- Attach/detach test:
  - Uses the parent/core worker process client attach/detach API.
  - Detaches a consumer while the worker PTY continues producing bounded synthetic output.
  - Reattaches and observes continued or retained output according to the documented retention policy.
  - Asserts no `FRAME_ATTACH` / `FRAME_DETACH` protocol constants were added for this ticket.
- Bounded consumer test:
  - Forces a tiny worker egress capacity or stops draining parent-side output.
  - In the same regression, asserts the worker event loop/health stays live, the PTY owner keeps making progress, retained bytes preserve order, and typed overflow/drop counts or pressure summaries account for bytes not retained.
- Public path test:
  - Exercises the final named worker-backed public facade, for example `DefaultBotsterEngine::worker_backed` or `ManagedSessionRuntime::with_worker_process`.
  - Proves that spawn, input, resize, output drain, health/backpressure observation, and shutdown cross the worker process boundary.
  - If `DefaultBotsterEngine::new()` stays in-process, asserts the named worker-backed constructor is the tested production-shaped path.
- Regression tests:
  - Existing `cargo test -p botster-core --test local_process_runtime_test` remains green.
  - Existing `cargo test -p botster-core --test managed_session_runtime_test` remains green unless deliberately updated to the worker-backed path.
  - Existing many-PTY conformance tests remain green or gain a worker-backed equivalent.

Expected verification commands:

- `cargo test -p botster-core --test local_process_runtime_test`
- `cargo test -p botster-core --test managed_session_runtime_test`
- `cargo test -p botster-core --test botster_engine_api_test`
- `cargo test -p botster-core --test session_protocol_test`
- `cargo test -p botster-core-test-support`
- Worker crate tests, for example `cargo test -p botster-session-worker` if that crate name is chosen.
- `cargo test --workspace`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`

Verification evidence should explicitly state which test proves the OS process boundary and which public entry point now uses the worker-backed runtime.

## Vault Gaps Worth Capturing

No durable vault note is required before implementation.

Capture after implementation if the final design settles any of these as durable conventions:

- The local session worker process, not hub/core, owns live PTY and child process handles.
- The public core facade that should be used for worker-backed local sessions.
- Worker egress bounded-queue semantics under slow or disconnected consumers.
- Parent disconnect cleanup behavior and whether worker shutdown or retention is the default.
