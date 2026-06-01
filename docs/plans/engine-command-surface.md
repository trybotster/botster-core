# Botster Core Engine Command Surface

Ticket: `ticket_1780348077_351705`
Run: `run_1780348098_553098`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Define botster-core engine command surface`, run `run_1780348098_553098`, current step `botster_plan`, run step `run_step_1780348099_287934`, gate `botster_plan_gate`, no prior artifacts, reviews, findings, questions, or answers.
- Project Pipelines checklist instructions loaded. Creating the run-level vault checklist initially timed out inside the plugin worker, then the checklist appeared in run context and was completed as `checklist_1780348142_250597`.
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
- Plan artifact convention loaded:
  - [[plan steps need reviewable plan artifacts]]
- General vault context loaded:
  - [[identity]]
  - [[goals]]
- Repo context inspected:
  - `README.md`
  - `Cargo.toml`
  - `crates/botster-core/Cargo.toml`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/engine/mod.rs`
  - `crates/botster-core/src/engine/botster.rs`
  - `crates/botster-core/src/engine/multiplexer.rs`
  - `crates/botster-core/src/engine/managed_session_runtime.rs`
  - `crates/botster-core/src/runtime/mod.rs`
  - `crates/botster-core/src/contract/session.rs`
  - `crates/botster-core/src/contract/transport.rs`
  - `crates/botster-core/src/contract/terminal_screen.rs`
  - `crates/botster-core/src/contract/notification.rs`
  - `crates/botster-core/src/contract/actor.rs`
  - `crates/botster-core/tests/botster_engine_api_test.rs`
  - `crates/botster-core/tests/multiplexer_engine_api_test.rs`
  - `crates/botster-core-dev/src/lib.rs`
  - `crates/botster-core-dev/tests/engine_smoke_test.rs`
- Prior plan artifacts inspected:
  - `docs/plans/assemble-core-multiplexer-engine-api.md`
  - `docs/plans/ergonomic-embeddable-botster-engine-api.md`
  - `docs/plans/default-pty-runtime-multiplexer-engine-integration.md`
  - `docs/plans/minimal-real-embedder-example.md`
  - `docs/plans/core-session-model-activity-engine.md`
  - `docs/plans/terminal-screen-snapshot-boundary.md`

## Scope

Define the policy-free command boundary that embedders, hub, tests, and future plugin/provider layers should target instead of reaching into lower-level engine pieces directly.

In scope:

- Audit the existing public engine/session/screen APIs and document the command surface they already imply.
- Add a concise architecture note, preferably in `README.md` or a focused `docs/architecture/engine-command-surface.md`, that names command/request/result/event ownership and the explicit exclusions.
- Add a small `botster-core` module skeleton if it makes the command vocabulary compile-checkable and discoverable. Prefer `crates/botster-core/src/engine/command.rs` or similarly narrow naming, exported through `engine/mod.rs` and `lib.rs`.
- Cover only commands supported by current core mechanisms:
  - spawn local session from explicit host-resolved `SessionSpawnRequest`;
  - attach and detach clients;
  - send terminal input;
  - resize sessions;
  - list sessions;
  - inspect session lifecycle and activity;
  - read screen state;
  - capture and replay snapshots;
  - shut down sessions;
  - post/drain notifications only through the existing notification inbox model.
- Define or document request/result/event shapes and error vocabulary by mapping to current types where possible: `SessionSpawnRequest`, `CoreSession`, `TransportIngress`, `TransportEgress`, `SessionIoRequest`, `SessionIoEvent`, `TerminalScreenState`, `TerminalSnapshotPayload`, `NotificationItem`, `BotsterEngineOutput`, `MultiplexerEngineOutcome`, `BotsterEngineError`, `MultiplexerEngineError`, and `ManagedSessionRuntimeError`.
- Keep the command surface synchronous at core level. Hosts own scheduling, executors, queues, transport delivery, and async supervision.
- Add compile/doc tests or a focused integration test that proves the public command surface is usable through crate-root exports and routes to the existing runtime path.
- Keep all examples synthetic and scrubbed of private paths, usernames, prompt text, terminal contents beyond test strings, credentials, or hostnames.

Non-scope:

- No product CLI UX, config discovery, auth, cloud/WebRTC/signaling, marketplace/update policy, Rails relay, TUI, restty, hub policy, Project Pipelines product policy, GitHub/Cloudflare provider behavior, or UI rendering.
- No new runtime scheduler, Tokio dependency, channel topology, daemon lifecycle, persistence, reconnect, retention, spawn target admission, command discovery, default shell selection, environment inheritance policy, or notification presentation policy.
- No broad rewrite of `BotsterEngine`, `DefaultBotsterEngine`, `ManagedSessionRuntime`, `MultiplexerEngine`, `SessionWorkerEngine`, `SubscriptionMultiplexer`, terminal screen contracts, notification inbox, or runtime traits.
- No fabricated command support. If a command is not currently supported by core, document it as out of surface rather than adding placeholder behavior.
- No compatibility branch, version-suffixed alternate API, speculative provider abstraction, or broad cleanup unrelated to the command boundary.

Botster layers touched:

- Rust `botster-core` engine facade and contract documentation: primary.
- Rust `botster-core` public exports: only if a command skeleton is added.
- Rust tests/rustdoc: proof that the public command surface is consumable.
- Docs: concise architecture note and this plan artifact.
- No plugin, Lua core, TUI, React SPA, Rails relay, MCP, provider, or Project Pipelines runtime changes.

Worktree/target assumptions:

- Implementers work in the pipeline-provided ticket worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The production path in this repo is the exported Rust library API and the `botster-core-dev` real embedder harness, not full hub product wiring.
- Pipeline gate evidence should cite this plan artifact and any retried checklist evidence.

## Assumptions And Unknowns

Assumptions:

- The existing `BotsterEngine`, `DefaultBotsterEngine`, `ManagedSessionRuntime`, and `MultiplexerEngine` are the runtime foundation. This ticket should define the command boundary over them, not reimplement them.
- A concise module skeleton is allowed if it gives embedders a stable vocabulary. The smallest acceptable implementation may be docs plus tests if current types already express every shape without a useful wrapper.
- `spawn local session` means an explicit host-resolved spawn request through `DefaultBotsterEngine`/`LocalProcessRuntime`; core still does not discover commands, cwd, target, config, or environment policy.
- `list sessions` can initially mean a public read-only session inventory from existing recorded `CoreSession` state. If current public API only exposes `session(id)` and `session_ids()` on lower-level `MultiplexerEngine`, the command surface may need a narrow facade method on `BotsterEngine` and `DefaultBotsterEngine`.
- `inspect lifecycle/activity` should return or document `CoreSession.lifecycle`, `CoreSession.activity`, and `classify_activity`; it should not add product status projection.
- `read screen state` and `capture/replay snapshot` should map to existing terminal screen and session-worker carriers. The default managed local runtime supports a narrow terminal-state path; unsupported branches must return typed errors rather than fake fidelity.
- Notifications are in scope only because core already has `NotificationInbox` plus `BotsterEngine::post_notification` and `drain_notifications`.
- No human question is blocking planning because the ticket explicitly excludes product and transport policy.

Unknowns for implementation:

- Exact shape: a pure docs architecture note may be enough, but a `command` module with typed enums/structs may be worth adding if it avoids forcing consumers to infer command vocabulary from `BotsterEngine` method names.
- If a module skeleton is added, whether it should be one umbrella enum such as `EngineCommand` or smaller request/result/event structs. Prefer smaller named structs when variants would only wrap existing rich types.
- Whether command results should alias existing outcomes or define stable facade result types. Prefer aliases or wrapper structs that preserve typed outcomes without duplicating event routing behavior.
- Whether `BotsterEngine` needs `list_sessions()` and `screen/snapshot` convenience methods to close command-surface gaps. Add only narrow methods that delegate to existing engine paths.
- Whether architecture docs belong in `README.md`, `docs/architecture/engine-command-surface.md`, or both. Prefer one concise architecture doc plus a README pointer if README would otherwise become too large.

## Affected Surfaces / Files

Expected:

- `docs/architecture/engine-command-surface.md` or `README.md`
  - Defines command/request/result/event ownership, error model, sync/async expectations, supported commands, and explicit exclusions.
- `docs/plans/engine-command-surface.md`
  - This plan artifact.
- `crates/botster-core/src/engine/botster.rs`
  - Possible narrow convenience methods for gaps such as listing sessions, screen read, snapshot capture/replay, or command-shaped helpers if existing public methods are insufficient.
- `crates/botster-core/src/engine/multiplexer.rs`
  - Possible narrow accessors only if the facade cannot list/inspect sessions through existing public state.
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Possible narrow command helpers only if default local runtime screen/snapshot commands are currently reachable only through lower-level internals.
- `crates/botster-core/src/engine/mod.rs`
  - Export any new command module or types.
- `crates/botster-core/src/lib.rs`
  - Re-export any new command module/types from the crate root.
- `crates/botster-core/tests/engine_command_surface_test.rs` or `botster_engine_api_test.rs`
  - Compile/integration proof that the command surface can drive the public runtime path and maps to existing outcomes.

Possible but avoid unless implementation proves necessary:

- `crates/botster-core/src/engine/command.rs`
  - Small command vocabulary skeleton with request/result/event docs and type aliases.
- `crates/botster-core/src/contract/session.rs`, `transport.rs`, `terminal_screen.rs`, `notification.rs`, or `actor.rs`
  - Only doc comments or tiny public conversions if required by the command surface.
- `crates/botster-core-dev/src/lib.rs`
  - Optional smoke harness update only if it cheaply proves the new command facade, not just the existing default engine path.

Not expected:

- Cargo dependency changes.
- `botster-terminal-ghostty` changes.
- Hub, CLI product, browser, TUI, Rails, Lua plugin, MCP, provider, or Project Pipelines files.

## Implementation Shape

Suggested minimal shape:

1. Add a concise architecture note that includes a table:
   - command;
   - request type;
   - result type;
   - event/notification output;
   - owner of policy;
   - current public entry point.
2. Start by mapping commands to existing API:
   - `spawn_session`: `SessionSpawnRequest` plus `CoreSessionMetadata` into `BotsterSpawnOutcome`.
   - `attach_client` / `detach_client`: `ClientId`, `SessionId`, `SubscriptionId` into `BotsterEngineOutput`.
   - `send_input`: bytes into existing `write_bytes`.
   - `resize`: rows/cols into existing `resize`.
   - `list_sessions`: recorded `CoreSession` inventory from engine state.
   - `inspect_session`: recorded `CoreSession` plus `classify_activity`.
   - `read_screen`: existing `SessionIoRequest::GetScreen`/`ScreenReady` or terminal screen engine path.
   - `capture_snapshot`: existing `RequestSnapshot`, `SnapshotReady`, and `TerminalSnapshotPayload` conversion.
   - `replay_snapshot`: existing `PrepareSnapshot`/`PreparedSnapshotReady` and terminal screen replay path where supported.
   - `shutdown`: existing `shutdown_session`.
   - `notifications`: existing `NotificationItem`, `post_notification`, `drain_notifications`.
3. If method gaps remain, add narrow public facade methods that delegate to existing lower-level engines. Do not add a second router.
4. If a `command` module is useful, keep it skeletal:
   - type aliases for existing request/result/error/event shapes where those are already stable;
   - small enums only for command names or high-level event categories;
   - rustdoc that states sync core, async host, and policy exclusions.
5. Prove the runtime path:
   - tests instantiate crate-root public types;
   - at least one command path uses `DefaultBotsterEngine` to spawn a synthetic local command and observes client egress through subscription fanout, or explicitly documents why the ticket is docs/skeleton-only;
   - docs/rustdoc compile against exported types.

Design constraints:

- Core commands are mechanisms, not product actions.
- Stable Botster controls stay typed. Do not use `BoundaryJson` for attach, input, resize, snapshot, lifecycle, or notification controls.
- Host policy stays outside the command boundary. Request structs should require already-resolved ids, commands, cwd, env, timestamps, and subscription ids.
- Error model should reuse existing typed errors and document unsupported commands explicitly.
- Sync/async expectation: core APIs return deterministic values synchronously; hosts may wrap them in async actors and queues.

## Risks

- Adding a command enum that duplicates `BotsterEngine` routing could create a second drift-prone API. Mitigation: commands should be aliases/docs or thin delegates over current methods.
- Defining product-like commands could move hub, CLI, provider, or Project Pipelines policy into core. Mitigation: document policy owners and keep request inputs explicit.
- Claiming support for screen/snapshot/replay beyond current core mechanics would overpromise. Mitigation: tests or docs must distinguish supported, unsupported, and host-adapter-dependent commands.
- `list_sessions` can become historical browsing policy if broadened. Mitigation: list only currently recorded core sessions; historical retention belongs to hosts.
- Notification support can imply UI presentation or push delivery. Mitigation: core only queues/drains typed items; host/client presentation is out of scope.
- `BoundaryJson` could be tempting for command payload flexibility. Mitigation: stable command controls must remain typed; raw JSON remains plugin/relay-owned only.
- A docs-only change could fail the acceptance if it does not identify concrete command shapes and entry points. Mitigation: architecture doc must map every requested command to types or explicit unsupported status.
- PII can leak through local paths in examples or plan docs. Mitigation: use synthetic ids and neutral worktree references only.

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

Targeted acceptance assertions:

1. A repo-visible architecture note identifies command/request/result/event shapes for every requested command or explicitly says unsupported when core does not support it.
2. The command surface names ownership boundaries: core mechanisms versus host/hub/CLI/provider/product policy.
3. The error model reuses or documents typed errors and unsupported command behavior.
4. Sync/async expectations are explicit: core methods are synchronous and deterministic; host runtimes own scheduling.
5. Public exports are crate-root consumable if new command types are added.
6. Tests prove at least one real runtime command path reaches the existing production entry point, preferably `DefaultBotsterEngine` spawning a synthetic local process and delivering output via subscribed `TransportEgress`.
7. Tests or rustdoc prove command-shaped screen/snapshot/list/inspect APIs where added.
8. No examples or docs include personal paths, usernames, credentials, prompts, or private terminal contents.

Runtime/user path proof:

- The changed production path is the exported `botster_core` library API used by embedders and the `botster-core-dev` real embedder harness when touched.
- Evidence that types exist is not enough. Implementation evidence must show the command boundary delegates into existing engine/runtime methods, or document that the ticket intentionally delivered a docs/skeleton boundary only because existing methods already implement runtime behavior.

## Vault Checklist Evidence

- Vault/project notes constraining the plan: [[planner-playbook]], [[botster-planner-playbook]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[plan steps need reviewable plan artifacts]], [[identity]], and [[goals]].
- Convention conflicts: none. The plan keeps workflow/product policy outside `botster-core`, preserves typed core controls, and uses a repo-visible plan artifact because this repository has `docs/plans/`.
- Verification evidence so far: planning inspection only; no implementation verification commands were run. Planned verification commands are listed above.
- Durable knowledge capture: no new vault note is required before implementation. Capture after implementation if the final command vocabulary becomes a durable convention for `BotsterEngine` versus `MultiplexerEngine` ownership, or if screen/snapshot command support exposes a reusable unsupported-command rule.

## Vault Gaps Worth Capturing

- Capture a Botster architecture note if this ticket settles the command vocabulary as the preferred boundary for hub, plugin/provider, and embedder callers.
- Capture if the final shape establishes that `BotsterEngine` is the command facade while `MultiplexerEngine` remains the lower-level assembled primitive.
- Capture if snapshot/screen support needs a durable rule distinguishing supported core commands from host-adapter-dependent commands.
