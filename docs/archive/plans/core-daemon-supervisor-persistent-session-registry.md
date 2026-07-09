# Add Core Daemon Supervisor And Persistent Session Registry

Ticket: `ticket_1780532711_470736`
Run: `run_1780535237_548066`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Add core daemon supervisor and persistent session registry`, run `run_1780535237_548066`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts, reviews, findings, questions, or answers.
- Dependency context loaded from pipeline: closed dependency `ticket_1780532685_767820`, `Define durable session worker protocol and restart contract`.
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
  - [[plan steps need reviewable plan artifacts]]
- General vault context loaded:
  - [[identity]]
  - [[goals]]
- Repo context inspected:
  - `Cargo.toml`: workspace currently has `botster-core`, `botster-core-dev`, `botster-core-test-support`, and `botster-terminal-ghostty`.
  - `crates/botster-core/src/lib.rs`: public exports already include engine facade, managed runtime, local runtime, session protocol, terminal screen, notifications, and subscription routing.
  - `crates/botster-core/src/engine/botster.rs`: `DefaultBotsterEngine` is the current policy-free local PTY-backed public facade.
  - `crates/botster-core/src/engine/managed_session_runtime.rs`: scheduling-neutral live-session bridge already routes runtime drains and client writes through session worker and multiplexer paths.
  - `crates/botster-core/src/engine/multiplexer.rs`: assembled core facade already owns in-memory sessions, handles, workers, subscriptions, notifications, plugins, timers, lifecycle/activity observations, and backpressure observations.
  - `crates/botster-core/src/engine/session_worker.rs`: session worker state machine already owns typed input, resize, snapshots, initial snapshot barrier, mode/screen requests, shutdown, and runtime events.
  - `crates/botster-core/src/contract/notification.rs`: current notification status vocabulary is queued, delivered, expired, dropped, acknowledged; this is not enough for guarded write delivery states.
  - `crates/botster-core/src/contract/session_protocol.rs`: session-to-daemon protocol contracts and terminal/session evidence payloads already exist.
  - `crates/botster-core/src/runtime/local_process.rs`: current local process runtime has a shared in-memory process registry and bounded output queues, but no durable daemon registry.
  - `crates/botster-core-dev/src/lib.rs`, `src/main.rs`, and `tests/engine_smoke_test.rs`: existing dev-only smoke harness proves the real default engine path without becoming product CLI policy.
  - Existing plan artifacts inspected: `core-session-worker-engine.md`, `session-process-wire-protocol.md`, `core-notification-session-inbox-primitives.md`, `supervised-session-task-runtime-core-engine.md`, and `default-pty-runtime-multiplexer-engine-integration.md`.
- Project Pipelines checklist instructions loaded. Creating the run-level vault workflow checklist timed out in the plugin worker; per [[project pipelines checklist worker timeouts require artifact evidence fallback]], checklist evidence is preserved in this plan and should also be included in gate evidence.

## Scope

Add the production core daemon layer that supervises durable session workers and exposes a typed daemon control API plus a thin CLI over that same API.

In scope:

- Add a new workspace crate/binary for the daemon and CLI, preferably `crates/botster-core-daemon`, instead of growing `botster-core-dev` into production operator tooling.
- Embed existing `botster-core` primitives rather than reimplementing session routing:
  - `DefaultBotsterEngine` / `ManagedSessionRuntime` for local PTY-backed session mechanics;
  - `MultiplexerEngine` and `SubscriptionMultiplexer` for attach/detach/input/resize/output fanout;
  - `SessionWorkerEngine` and session protocol contracts for worker I/O;
  - notification and terminal screen contracts for guarded write evidence.
- Define a typed daemon API that covers the ticket verbs:
  - spawn;
  - list;
  - attach;
  - detach;
  - input;
  - resize;
  - drain and subscribe output;
  - guarded session notification/write injection;
  - health;
  - adoption;
  - shutdown.
- Add a persistent registry model and filesystem-backed registry store for non-sensitive metadata needed to discover and adopt live session workers after daemon restart.
- Persist only worker/session metadata required for adoption: session id, worker protocol endpoint or process identity, spawn/runtime metadata needed for discovery, lifecycle state, terminal size, timestamps, and safe non-PII labels. Do not persist PTY bytes, terminal scrollback, credentials, auth state, or product workflow data.
- Add adoption mechanics that load registry records, inspect liveness through typed session-worker protocol evidence, and rehydrate daemon registry state enough for a follow-up restart/recover ticket to adopt live workers.
- Add explicit guarded write delivery state vocabulary separate from ordinary notification inbox status:
  - accepted;
  - queued or deferred;
  - rejected;
  - written or injected;
  - delivered or acknowledged only when the daemon has explicit evidence.
- Gate guarded writes on core-owned terminal/session readiness evidence such as cursor visibility, prompt/waiting-for-answer hints, terminal snapshot/screen state, mode flags, and safe-write indicators already exposed through session protocol or terminal screen contracts. If an evidence source is not currently available, represent it as absent and defer or reject rather than guessing.
- Route host/plugin-authorized notifications through the typed daemon API while keeping auth, policy, copy, product meaning, and routing authorization outside core.
- Use bounded/non-blocking queues for hot paths. Slow attach subscribers or notification drains must not stall daemon supervision or PTY worker reads.
- Add integration tests that drive the real daemon API path, not only individual contract structs.
- Add CLI smoke coverage that proves the daemon can be started, inspected, and used at a basic tmux-like level without hub.
- Add docs explaining daemon versus worker ownership, typed API versus CLI, readiness-gated writes, delivery semantics, registry durability, and non-policy boundaries.

Non-scope:

- No hub auth, marketplace, Rails, cloud/WebRTC, ActionCable, Project Pipelines workflow policy, provider policy, plugin authorization decisions, browser/TUI rendering, or admitted target policy.
- No product-specific command discovery, default shell selection, config hierarchy, workspace admission, retention policy, or notification copy decisions.
- No CLI-output parsing as the normal hub/embedder API. The CLI is for operator/dev/debug use over the typed daemon API.
- No broad rewrite of `botster-core` engine primitives unless the daemon exposes a narrow missing hook.
- No terminal backend migration to Ghostty inside `botster-core`; concrete Ghostty mechanics stay in `botster-terminal-ghostty`.
- No raw JSON escape hatches for stable daemon controls such as attach, input, resize, notification/write delivery, adoption, or shutdown.
- No PII in registry fixtures, logs, CLI output assertions, docs examples, or tests.

Botster layers touched:

- Rust core daemon: primary new production surface.
- Rust `botster-core` engine/runtime/contract surfaces: narrow additions only where the daemon needs typed public hooks.
- Rust `botster-core-test-support`: fake daemon registry/session worker helpers if useful for integration tests.
- Rust CLI/operator surface: thin CLI in the daemon crate.
- Docs: plan artifact plus daemon ownership/API docs.
- No plugin, Lua core, MCP, TUI, React SPA, Rails relay, cloud/provider, or Project Pipelines runtime behavior changes.

Worktree/target assumptions:

- Implementation agents operate in the assigned Project Pipelines worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.
- This plan avoids absolute local worktree paths in repo-visible artifacts.

Pipeline gates/artifacts:

- This file is the Plan artifact.
- Gate evidence should cite this file and record the checklist timeout fallback.
- Advancement target is `botster_plan_review`.

## Assumptions And Unknowns

Assumptions:

- A production daemon belongs in a new production crate/binary because `botster-core-dev` is explicitly dev-only and should not become the operator CLI.
- The daemon API should be Rust-typed first. The CLI should call the same API internally and render human/debug output second.
- A simple filesystem-backed registry is sufficient for this ticket. It should use standard library filesystem operations and serde JSON already present in the workspace rather than adding a database dependency.
- Registry durability for this ticket means enough persisted state for a subsequent adoption/restart ticket to discover and adopt live workers, not full crash-proof terminal scrollback persistence.
- Core daemon ownership stops at reusable mechanics: worker supervision, routing, readiness evidence, bounded queues, registry metadata, and typed delivery states.
- Hub/plugin authorization can be represented as a trusted caller boundary or opaque caller label in tests, but authorization decisions stay outside the daemon.
- Guarded writes should fail closed: absent readiness evidence yields deferred or rejected, never delivered.
- The implementation can use synthetic local shell commands in tests with generic ids and no private paths.

Unknowns for implementation:

- Exact crate name. Prefer `botster-core-daemon` because it keeps production daemon/CLI distinct from `botster-core-dev`.
- Exact transport for the typed control API. Prefer an in-process Rust API first, with a thin CLI command runner over it. Add IPC only if needed to prove daemon start/inspect/use smoke coverage.
- Exact registry file layout. Prefer one daemon registry root with one JSON record per session to make partial recovery and fixture inspection simple.
- Exact adoption depth. The ticket acceptance says durable enough for a subsequent adoption ticket; implementers should add the public adoption operation and registry/liveness probe seams now, but full cross-process recovery can remain bounded if tests prove persisted records are sufficient for the follow-up.
- Exact readiness evidence model. Prefer a small typed evidence struct over product-specific heuristics; map current snapshot, screen, mode flags, prompt marks, cursor visibility, and safe-write indicators where available.
- Whether existing `NotificationDeliveryStatus` should be extended or a new guarded-write state enum should be added. Prefer a separate enum to avoid corrupting existing inbox semantics.

No human question is blocking this plan. The ticket is broad, but it has one coherent meaning: a production core daemon supervisor over existing core engine/session-worker mechanics, not hub/product policy.

## Affected Surfaces / Files

Expected:

- `Cargo.toml`
  - Add the new daemon crate to workspace members.
- `crates/botster-core-daemon/Cargo.toml`
  - Production daemon crate metadata, depending on `botster-core`.
- `crates/botster-core-daemon/src/lib.rs`
  - Public daemon API exports.
- `crates/botster-core-daemon/src/main.rs`
  - Thin CLI entry point over the daemon API.
- `crates/botster-core-daemon/src/daemon.rs`
  - Supervisor state, command dispatch, health, shutdown, and bounded route coordination.
- `crates/botster-core-daemon/src/api.rs`
  - Typed request/result/event/error enums and structs.
- `crates/botster-core-daemon/src/registry.rs`
  - Durable registry records and filesystem-backed load/save/remove/adoption scans.
- `crates/botster-core-daemon/src/guarded_write.rs`
  - Readiness evidence and delivery state transitions for guarded notifications/writes.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  - Spawn/list/attach/drain/input/resize/guarded-write/shutdown tests through the daemon API.
- `crates/botster-core-daemon/tests/cli_smoke_test.rs`
  - Basic start/inspect/use CLI smoke over the daemon path.
- `docs/core-daemon.md` or `docs/architecture/core-daemon.md`
  - Daemon versus worker ownership, typed API versus CLI, registry durability, readiness gates, and delivery semantics.
- `docs/archive/plans/core-daemon-supervisor-persistent-session-registry.md`
  - This plan artifact.

Possible but keep narrow:

- `crates/botster-core/src/engine/botster.rs`
  - Add small public helper methods if the daemon cannot access needed existing behavior through `DefaultBotsterEngine`.
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Add narrow hooks for readiness/screen/snapshot evidence only if existing public facade methods are insufficient.
- `crates/botster-core/src/contract/notification.rs`
  - Add guarded-write or delivery-state contract only if it should be shared beyond the daemon crate. Prefer the daemon crate if the vocabulary is daemon-specific.
- `crates/botster-core/src/contract/session_protocol.rs`
  - Add safe-write/readiness payloads only if they are worker protocol facts rather than daemon policy.
- `crates/botster-core-test-support/src/fake/*`
  - Add fake registry or readiness helpers if tests would otherwise duplicate too much setup.
- `README.md`
  - Add one boundary paragraph or table row only if the new daemon production surface is otherwise undiscoverable.

Not expected:

- Existing dev-only `botster-core-dev` promotion to production daemon.
- `botster-terminal-ghostty` changes unless a test explicitly needs the adapter.
- Hub, Lua plugin, MCP, Rails, React SPA, TUI, provider, marketplace, or cloud files.
- New broad dependencies for async runtimes, databases, logging frameworks, or CLI frameworks unless implementation proves the standard library path is not viable.

## Implementation Shape

Suggested minimal shape:

- Create `botster-core-daemon` as a production crate with:
  - `CoreDaemon` or `BotsterCoreDaemon` supervisor type;
  - `DaemonCommand` / `DaemonRequest`;
  - `DaemonResponse` / `DaemonEvent`;
  - `DaemonError`;
  - `DaemonHandle` or in-process API object for embedders;
  - `DaemonRegistry` with JSON records under a caller-supplied data directory.
- The daemon owns one `DefaultBotsterEngine` initially. It should delegate spawn/list/attach/detach/input/resize/drain/shutdown to the existing engine facade and only add supervision, registry, queue, readiness, adoption, and API concerns around it.
- Registry writes should happen on spawn and lifecycle changes, and registry removal or terminal final state should happen on clean shutdown according to the chosen adoption contract.
- Queue pressure should use bounded route buffers with typed rejected/deferred outcomes rather than blocking daemon commands indefinitely.
- Guarded write handling should:
  - accept the caller request into a typed state machine;
  - evaluate readiness evidence from core-owned session/terminal state;
  - reject invalid targets or unsafe policy-free conditions;
  - defer when evidence is insufficient but retryable;
  - inject/write only through the same input/session worker path as ordinary input;
  - mark delivered/acknowledged only when a worker/session/client event proves it.
- CLI commands should be intentionally thin:
  - start daemon with explicit data directory;
  - health/status;
  - spawn a command;
  - list sessions;
  - attach/drain output or run a one-shot basic input/drain scenario;
  - shutdown.
- CLI tests should assert structured behavior and scrubbed output, not parse a product protocol.

Runtime path proof:

- The integration test must instantiate the daemon API, spawn a worker-backed local session, attach a client, drain output through the daemon, send input through the daemon, resize through the daemon, execute guarded writes through the readiness gate, and shut down through the daemon.
- The CLI smoke must execute the daemon binary path enough to prove operator/dev use does not require hub.
- Evidence that structs compile is not enough.

## Risks

- The ticket is large enough to tempt broad architecture. Keep the crate and API narrow: daemon supervision and registry over existing core mechanics only.
- Building a second session router inside the daemon would duplicate `MultiplexerEngine` and likely break terminal data-plane guarantees.
- Marking guarded notifications delivered on API acceptance would violate the ticket. Delivery states must reflect evidence, not command receipt.
- Persisting too much registry state risks PII or product policy leakage. Registry fixtures need synthetic ids and path-neutral data.
- A CLI-first implementation could force hub/embedders to parse text. The typed API must be the normal path.
- Adoption can become full recovery policy. This ticket should provide the durable registry and adoption operation/seams, while leaving product retention/restart policy out.
- Unbounded subscriber queues can stall daemon hot paths or grow memory under slow consumers. Tests should cover pressure or at least explicit bounded capacity behavior.
- IPC or process-management tests can be timing-sensitive. Keep CLI smoke bounded and deterministic with synthetic commands.
- Adding external crates for async, CLI parsing, logging, or persistence would conflict with the minimal-dependency convention unless there is a concrete compiler/runtime need.

## Acceptance Checks / Tests

Required targeted tests:

1. `daemon_spawns_worker_backed_session_and_lists_registry`
   - Start daemon API with a temporary data dir.
   - Spawn a synthetic local session.
   - Assert list returns the session and registry file exists with non-PII metadata.

2. `daemon_attach_drain_input_and_resize_use_engine_path`
   - Attach a client through daemon API.
   - Drain known startup output.
   - Send input and observe expected output through daemon drain.
   - Resize and assert screen/snapshot or daemon state observes the new size.

3. `guarded_write_accepts_ready_session_and_injects_through_input_path`
   - Arrange readiness evidence.
   - Submit guarded write.
   - Assert states progress through accepted and written/injected, and delivered/acknowledged only if explicit proof is generated.

4. `guarded_write_defers_when_session_not_ready`
   - Arrange absent/unsafe readiness evidence.
   - Assert queued/deferred state and no input injection.

5. `guarded_write_rejects_invalid_or_unsafe_target`
   - Unknown session or explicit unsafe state returns rejected and does not write.

6. `daemon_shutdown_updates_lifecycle_and_registry`
   - Shutdown one session and then daemon.
   - Assert lifecycle/health reflect shutdown and registry state matches the adoption contract.

7. `registry_records_are_durable_enough_for_adoption_scan`
   - Write registry records, construct a new daemon instance against the same data dir, and assert adoption scan reports adoptable/live or stale records using typed states.

8. `slow_subscriber_does_not_block_daemon_hot_path`
   - Saturate a bounded output/notification route and assert the daemon reports lag/deferred/rejected pressure without blocking unrelated health or shutdown.

9. `daemon_cli_smoke_starts_inspects_and_uses_session`
   - Run the daemon binary or CLI harness with explicit data dir.
   - Prove start/status/spawn/list/input-or-drain/shutdown at a basic tmux-like level.

Preservation tests:

- Existing `botster-core` tests for `botster_engine`, `managed_session_runtime`, `multiplexer_engine`, `session_worker_engine`, `subscription_multiplexer`, `notification_inbox`, `local_process_runtime`, and `session_protocol` remain green.
- Existing `botster-core-dev` smoke harness remains dev-only and still passes.

Verification commands:

- `cargo fmt`
- `cargo test -p botster-core-daemon`
- `cargo test -p botster-core daemon` if shared core tests are added
- `cargo test -p botster-core botster_engine`
- `cargo test -p botster-core managed_session_runtime`
- `cargo test -p botster-core local_process_runtime`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Documentation acceptance:

- Daemon docs clearly explain:
  - daemon owns registry, supervision/adoption, routing, bounded queues, readiness-gated write mechanics, and typed delivery states;
  - session workers own PTYs and terminal/session evidence;
  - hub/hosts own auth, policy, copy, spawn target admission, product semantics, cloud/WebRTC/Rails, and UI presentation;
  - typed API is the embedder/hub path and CLI is operator/dev/debug.

## Vault Checklist Evidence

- Vault/project notes constrained the plan: [[planner-playbook]], [[botster-planner-playbook]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], and [[plan steps need reviewable plan artifacts]].
- Convention conflicts: none. The plan follows core/hub policy separation, minimal dependencies, repo-visible plan artifact discipline, path-neutral artifact wording, typed controls over raw JSON, bounded hot paths, and Project Pipelines checklist fallback guidance.
- Verification evidence for this Plan step:
  - `git status --short` before planning showed a clean worktree.
  - Repo context was inspected with `rg --files`, targeted `rg`, and `sed` reads.
  - No compile/test command was run because this step only creates the Plan artifact; implementers should run the acceptance commands above.
- Durable knowledge capture: none needed before implementation. Existing vault notes cover the relevant daemon/core/hub boundaries and Project Pipelines workflow discipline.

## Vault Gaps Worth Capturing

No durable vault capture is required before implementation.

Capture after implementation only if the final API settles a reusable rule not already covered by existing notes:

- The durable registry boundary between core daemon metadata and product/hub persistence policy.
- The exact guarded-write delivery state model and what evidence is sufficient to mark delivered or acknowledged.
- The production split between `botster-core-daemon` and the existing dev-only `botster-core-dev` harness.
