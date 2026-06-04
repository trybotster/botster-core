# Durable Session Worker Protocol And Restart Contract

Ticket: `Define durable session worker protocol and restart contract`
Run: `run_1780533201_805922`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket
  `ticket_1780532685_767820`, run `run_1780533201_805922`, current step
  `botster_plan`, current run step `run_step_1780533201_416120`, gate
  `botster_plan_gate`.
- No prior artifacts, reviews, findings, dependencies, open questions, or prior
  answers are present in the current pipeline context.
- Ticket target: `trybotster/botster-core` on `main`, target id
  `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Required playbooks loaded:
  - [[planner-playbook]]
  - [[botster-planner-playbook]]
- Required Botster planning overlay notes loaded:
  - [[botster-architecture]]
  - [[cli-patterns]]
  - [[spa-patterns]]
  - [[project pipeline orchestration belongs in a device-level botster plugin]]
  - [[project pipelines needs an operator workbench not more primitives]]
  - [[project pipelines ui contract belongs in the plugin readme]]
  - [[botster orchestration should spawn agents with explicit target ids]]
  - [[botster orchestration prompts must bind agents to explicit worktrees]]
- General vault context loaded:
  - [[identity]]
  - [[goals]]
- Repo context loaded:
  - `README.md`: core is policy-free reusable mechanism/contract surface;
    hub owns runtime policy, lifecycle, routing, recovery, and extension
    supervision; CLI owns operator commands and process startup.
  - `docs/architecture/engine-command-surface.md`: `BotsterEngine` is the
    typed local control facade; product CLI UX/config/auth/reconnect policy is
    out of core.
  - `docs/plans/session-process-wire-protocol.md`: existing frame-level
    session-process protocol extraction covers handshake bytes, frame constants,
    PTY input/output, resize, snapshot, ping/pong, shutdown, mode/screen, and
    opaque snapshots, but not durable worker topology or restart contract.
  - `docs/plans/core-session-worker-engine.md`: existing `SessionWorkerEngine`
    consumes `SessionIoRequest` and emits `SessionIoEvent` through host-supplied
    runtime adapters, with snapshot-before-live-output and activity semantics.
  - `docs/plans/bound-pty-output-queues-and-surface-backpressure.md`: current
    local PTY reader path already has typed session-I/O backpressure and
    production-path proof requirements.
  - `docs/plans/core-notification-session-inbox-primitives.md`: existing
    notification/inbox primitives are separate from PTY input.
  - `crates/botster-core/src/contract/session_protocol.rs`: current wire
    protocol is frame-level and includes ping/pong but no higher-level
    spawn/adopt/health/readiness/write-delivery/restart state contract.
  - `crates/botster-core/src/contract/actor.rs`: existing typed queue,
    backpressure, session I/O request/event, client control, lifecycle, and
    plugin-worker vocabulary.
  - `crates/botster-core/src/contract/session.rs`: stable session ids,
    metadata cap, lifecycle/activity state, but no durable worker identity or
    restart-survival model.
  - `crates/botster-core/src/engine/session_worker.rs` and
    `crates/botster-core/tests/session_worker_engine_test.rs`: existing engine
    behavior and tests for input, resize, snapshots, shutdown, output ordering,
    backpressure failures, and host-policy exclusion.
  - `crates/botster-core/src/engine/managed_session_runtime.rs` and
    `crates/botster-core/src/engine/botster.rs`: existing managed/default
    runtime path that proves `DefaultBotsterEngine` routes real local runtime
    output through session workers and subscription fanout.
  - `crates/botster-core-test-support/src/fixtures/regression/regression_shapes.rs`
    and `crates/botster-core/tests/regression_shape_fixtures_test.rs`: existing
    public regression-shape fixture pattern for preserving/ translating old
    runtime evidence without importing product policy.
- Project Pipelines checklist discipline:
  - `project_pipelines_checklist_instructions` loaded.
  - Creating the run-level vault checklist failed with a Project Pipelines
    plugin-worker timeout. Per the loaded Project Pipelines checklist fallback
    guidance, preserve checklist evidence in this plan and gate evidence rather
    than retrying repeatedly.
  - Vault notes constraining the plan are listed above.
  - Convention conflicts: none. The plan keeps policy-free contracts in core,
    leaves hub/CLI/plugin/product behavior outside core, uses typed contracts
    instead of CLI-output parsing, and avoids speculative runtime wiring.
  - Verification evidence required from implementation is listed below.
  - Durable capture decision is listed under Vault Gaps.

## Scope

Define the production contract for Botster's durable local session-worker model
at the public core contract and documentation layer.

In scope:

- Add a public architecture document that explains the durable topology:
  hub as policy/control plane, core daemon as local multiplexer/supervisor/router,
  and independent session worker processes as PTY/child-process owners.
- Define typed core contracts for daemon-to-session-worker communication above
  the existing byte-frame layer:
  - worker identity and version/compatibility metadata;
  - spawn and adopt requests/results;
  - attach and detach intent;
  - PTY input and resize commands;
  - output frames and snapshot/screen-state handoff descriptors;
  - heartbeat, health, shutdown, failure, stale-worker detection, and restart
    survival summaries.
- Define a policy-free daemon control API contract that hub and embedders can
  call directly through typed local IPC/library shapes, not CLI-output parsing.
- Define the daemon CLI role as a thin operator/dev/debug wrapper over the same
  typed control API. The docs should cover start, status, session list,
  attach/stream, shutdown, and health where practical, without implementing
  product CLI UX in this slice.
- Define session write primitives as two separate surfaces:
  - raw PTY input bytes;
  - host/plugin-authorized session annotations or notifications intended to
    appear in the session stream/screen.
- Define guarded session-write semantics:
  - readiness evidence supplied by core from terminal/session state;
  - host-supplied write policy inputs;
  - queue/defer/reject behavior;
  - explicit delivery state transitions.
- Cover readiness examples required by the ticket: agent waiting for an answer,
  cursor visibility or invisibility, and terminal/session state indicating an
  injected write would interrupt an active prompt or unsafe moment.
- Define delivery states at least:
  - accepted;
  - queued/deferred;
  - rejected;
  - written/injected;
  - delivered/acknowledged where provable.
- Define durability semantics explicitly:
  - survives hub restart;
  - survives core daemon restart where worker adoption succeeds;
  - dies when the session worker dies;
  - stale-worker detection and failure reporting.
- Define queue/backpressure and slow-consumer behavior in the contract, using
  existing typed pressure vocabulary where it fits.
- Add contract tests and/or regression fixtures covering protocol compatibility,
  identity, heartbeat/health, attach/detach, bounded output behavior,
  readiness-gated session-write routing, deferred/rejected writes, and delivery
  state transitions.
- Update README or architecture docs only enough to make the new public
  contract discoverable and align the ownership-boundary table with the durable
  worker contract.

Non-scope:

- No real daemon process implementation, Unix socket server, worker supervisor,
  process spawning/adoption loop, fd passing, thread/task orchestration, or
  persistent registry implementation.
- No product hub policy, auth, marketplace, Rails, cloud/WebRTC, Project
  Pipelines, provider config, device config locations, or UI behavior.
- No concrete product CLI parser or operator UX beyond documenting the thin
  wrapper role over typed control API contracts.
- No parsing of CLI output as an API.
- No terminal backend implementation, Ghostty snapshot restore implementation,
  restty/browser renderer behavior, TUI rendering, or ActionCable/Rails relay.
- No broad refactor of `SessionWorkerEngine`, `DefaultBotsterEngine`,
  `ManagedSessionRuntime`, or existing session-process frame constants unless a
  small compiler-proven gap blocks the public contract.
- No compatibility branches or version-suffixed duplicate APIs. Use additive
  typed contracts and explicit version metadata.
- No PII in docs, fixtures, logs, or tests. Examples must use synthetic ids and
  generic command/session labels.

Botster layers touched:

- Rust core contract layer: primary.
- Rust docs/architecture and plan artifacts: primary.
- Rust test-support fixtures and core contract tests: primary.
- Rust core engine docs/facade references: possible small discoverability
  update.
- Hub, plugin, TUI, React SPA, Rails relay, MCP, provider, and product CLI:
  intentionally not implemented.

Worktree/target assumptions:

- Implementer works only in the pipeline-assigned worktree for target
  `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The run targets `main`; no stacked dependency behavior is present in the
  current context.

Pipeline gates/artifacts:

- Plan artifact: this file.
- Plan gate evidence should cite this file and summarize loaded vault/repo
  context.
- Advancement target: `botster_plan_review`.

## Assumptions And Unknowns

Assumptions:

- This ticket is contract-definition work, not the full implementation of
  durable multiprocess session supervision.
- The existing `session_protocol` byte frame layer should be preserved and
  referenced. The new contract should sit above it rather than replacing frame
  constants that already exist.
- The existing `SessionWorkerEngine` and `SessionIoRequest`/`SessionIoEvent`
  path remains the current in-process engine proof; durable worker contracts
  should align with it instead of forking a second vocabulary for PTY input,
  resize, snapshots, output, shutdown, and process exit.
- `BotsterEngine`/`DefaultBotsterEngine` remain the daemon control API
  inspiration for typed library contracts. The durable daemon control contract
  should reuse or wrap those concepts rather than inventing a CLI-shaped API.
- Version/compatibility metadata belongs in typed public contracts; negotiation
  or rollout policy belongs to hosts.
- Readiness evidence is an observation contract, not core product policy. Core
  can report cursor visibility, screen/prompt markers, mode flags, last
  input/output/activity, pending snapshots, and worker health; hosts provide
  policy deciding whether a write should be sent, queued, deferred, or rejected.
- Delivery must not overclaim. A request being accepted by the daemon is not the
  same as written to the PTY, rendered on screen, or acknowledged by a semantic
  agent prompt.
- Existing `BackpressureSummary`, `DeliveryLag`, `MailboxSendFailure`, and
  queue source vocabulary should be reused before adding new pressure types.
- Contract fixtures should follow the existing regression-shape fixture pattern:
  preserve or translate runtime evidence into public `botster_core` data without
  importing old hub policy or implementation.

Unknowns for implementation:

- Final module naming. Prefer a focused contract module such as
  `contract::durable_session` or `contract::session_worker_protocol` rather than
  overloading `engine::session_worker`.
- Whether daemon control API types belong beside `engine::command` or in a new
  contract module. Prefer the smallest location that keeps the public API
  discoverable and avoids a second router.
- Whether guarded session-write contracts should extend the existing
  notification/session inbox vocabulary or live as a session-worker write
  contract. The ticket requires writes intended to appear in a session
  stream/screen, so they should not be confused with generic notification inbox
  delivery.
- Whether `QueueSource` needs a new `SessionWorker`/`CoreDaemon` source. Prefer
  existing `SessionIo` or `TransportAdapter` labels unless tests show the
  pressure route is ambiguous.
- Exact readiness evidence fields. Keep them narrow and testable; do not
  encode product-specific agent states as core enum variants.
- Whether README should include a short durable worker section or only link to
  the architecture doc. Prefer one concise README pointer plus the detailed
  architecture document.

No human question is blocking planning. The ticket is broad but has one
coherent interpretation in this repo: define typed core contracts, docs, and
fixtures for the durable worker north star without implementing the daemon or
product host wiring yet.

## Affected Surfaces / Files

Expected changes:

- `docs/architecture/durable-session-worker-protocol.md`
  - New public contract document covering topology, daemon control API, daemon
    CLI wrapper role, worker protocol, guarded writes, readiness evidence,
    delivery transitions, restart survival matrix, stale-worker detection,
    queue/backpressure, and explicit exclusions.
- `docs/plans/durable-session-worker-protocol-restart-contract.md`
  - This plan artifact.
- `crates/botster-core/src/contract/session_worker_protocol.rs` or
  `crates/botster-core/src/contract/durable_session.rs`
  - New typed public contracts for worker identity, protocol version metadata,
    spawn/adopt, attach/detach, heartbeat/health, shutdown/failure, restart
    survival, readiness evidence, guarded write requests, and delivery state.
- `crates/botster-core/src/contract/mod.rs`
  - Export the new contract module.
- `crates/botster-core/src/lib.rs`
  - Re-export public contract types that downstream embedders need.
- `crates/botster-core/tests/session_worker_protocol_contract_test.rs`
  - Contract tests for serde compatibility, identity/version metadata,
    spawn/adopt, heartbeat/health, attach/detach, restart survival semantics,
    readiness-gated writes, deferred/rejected/written/delivered states, and
    pressure/slow-consumer shapes.
- `crates/botster-core-test-support/src/fixtures/regression/regression_shapes.rs`
  - Add reusable fixture builders for stale-worker adoption, guarded session
    writes, and durable restart survival if that keeps tests readable.
- `crates/botster-core/tests/regression_shape_fixtures_test.rs`
  - Add tests proving new fixture shapes round-trip and remain policy-free.
- `README.md`
  - Likely add one concise pointer to the durable worker contract and update the
    ownership-boundary table only if the new public contract changes what core
    currently proves.

Possible but avoid unless needed:

- `crates/botster-core/src/contract/session.rs`
  - Only for small identifier newtypes if they are clearly shared session
    metadata rather than worker-protocol-only values.
- `crates/botster-core/src/engine/command.rs`
  - Only if daemon control command kinds can be represented as additive
    typed command vocabulary without adding runtime behavior.
- `crates/botster-core/src/contract/actor.rs`
  - Only if new pressure/delivery variants cannot reuse existing typed
    `BackpressureSummary`, `DeliveryLag`, or mailbox failure types.

Likely unchanged:

- `crates/botster-core/src/engine/session_worker.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/runtime/local_process.rs`
- `crates/botster-core/src/contract/session_protocol.rs`
- `crates/botster-core-dev`
- `crates/botster-terminal-ghostty`
- Any hub, Rails, React, TUI, Lua plugin, MCP, provider, or product CLI code.

## Proposed Contract Shape

Prefer a new public contract module with explicit, serializable shapes. Names
below are suggested, not mandatory:

- `DurableSessionProtocolVersion`
  - current version, minimum compatible version, implementation label, feature
    flags/capabilities, and opaque host-owned compatibility metadata if needed.
- `SessionWorkerIdentity`
  - worker id, session id, process identity, born-at timestamp supplied by host,
    protocol version metadata, and restart/adoption generation.
- `SessionWorkerSpawnRequest` / `SessionWorkerSpawned`
  - explicit session id, working directory/executable/environment references
    where policy-free and PII-safe, initial PTY size, host metadata, and worker
    identity result.
- `SessionWorkerAdoptRequest` / `SessionWorkerAdopted`
  - worker identity, expected session id, protocol version metadata,
    snapshot/screen handoff availability, last heartbeat, and adoption verdict.
- `SessionWorkerAttachRequest` / `SessionWorkerDetached`
  - client/subscription/session ids, requested size, snapshot strategy, and
    attach state without naming concrete transports.
- `SessionWorkerHeartbeat` / `SessionWorkerHealth`
  - liveness timestamp, lifecycle state, last output timestamp, queue pressure,
    snapshot availability, child process state, and stale reason when unhealthy.
- `SessionWorkerShutdownRequest` / `SessionWorkerFailure`
  - typed reason, graceful/forced requested mode if needed, process exit payload,
    stale/lost worker classification, and final durability boundary.
- `DurableRestartSemantics`
  - survival matrix for hub restart, core daemon restart with successful
    adoption, failed adoption, worker death, and stale worker cleanup.
- `SessionReadinessEvidence`
  - cursor visibility, terminal mode flags, last input/output/activity,
    plain-screen/prompt marker summaries where available, pending snapshot state,
    worker health, and host semantic hints. Keep examples generic.
- `GuardedSessionWriteRequest`
  - target session id, request id, write primitive (`PtyInput` vs
    session-visible annotation/notification), readiness evidence snapshot or
    reference, host policy verdict input, queue/defer deadline, and payload.
- `GuardedSessionWriteState`
  - accepted, queued/deferred, rejected, written/injected, delivered/acknowledged
    where provable. Include reason fields for rejected/deferred states.
- `SessionWorkerQueueLimits`
  - bounded output capacity, pressure summary, slow-consumer behavior, and
    ordering/drop/coalescing semantics. Prefer references to existing pressure
    types over new fields.
- `DaemonControlCommand` / `DaemonControlOutcome`
  - typed start/status/list/attach-stream/shutdown/health/session-inspection
    vocabulary for embedders and the thin CLI wrapper. Do not parse CLI output.

The implementation should keep docs and tests honest about what is scaffold
versus currently wired production behavior. For this ticket, the changed user
path is public contract consumption and compile/test fixtures, not a running
daemon restart.

## Risks

- Implementing a real daemon/worker supervisor in this ticket would sprawl past
  contract definition and likely violate the core/hub/CLI boundary.
- A docs-only change without typed contract tests would not satisfy the
  acceptance requirement for compatibility, identity, health, attach/detach,
  readiness-gated writes, and delivery transitions.
- A typed contract that is not exported from `botster-core` would be invisible
  to the future hub implementation.
- Duplicating existing `session_protocol` frame constants or
  `SessionIoRequest`/`SessionIoEvent` vocabulary could create conflicting
  protocol layers.
- Encoding product policy in readiness or guarded write states would make core
  decide when it is safe to interrupt an agent, which belongs to hosts/plugins.
- Overclaiming delivery would be misleading. Tests must distinguish accepted
  from written and written from delivered/acknowledged.
- Accidentally adding PII in examples is easy for session contracts. Use
  synthetic ids, generic labels, and no paths, titles, prompt text, usernames,
  or terminal transcripts.
- Adding raw `serde_json::Value` fields broadly would violate the typed contract
  posture. Any `BoundaryJson` use must be owner/reason classified.
- Introducing queue configurability or policy knobs too early would create API
  surface before the real daemon exists. Keep queue contracts descriptive and
  typed, not broadly configurable.
- README could overstate that core currently provides durable daemon restart.
  Wording must say the ticket defines the contract and fixtures; production
  multiprocess wiring is later host/daemon work.

## Acceptance Checks / Tests

Required focused tests:

- `cargo test -p botster-core --test session_worker_protocol_contract_test`
  - `worker_protocol_version_metadata_round_trips`
  - `worker_identity_binds_session_process_and_generation`
  - `spawn_and_adopt_contracts_preserve_session_identity`
  - `attach_and_detach_contracts_use_client_subscription_identity`
  - `heartbeat_and_health_distinguish_alive_unhealthy_and_stale`
  - `restart_semantics_matrix_names_hub_core_and_worker_survival`
  - `guarded_pty_input_accepts_but_does_not_claim_delivery`
  - `guarded_annotation_write_can_defer_reject_write_and_acknowledge`
  - `readiness_evidence_covers_answer_wait_cursor_and_prompt_safety`
  - `bounded_output_contract_preserves_pressure_and_slow_consumer_semantics`
  - `daemon_control_api_is_typed_not_cli_output`
  - `session_worker_protocol_examples_exclude_pii`

Required regression fixture checks if fixtures are added:

- `cargo test -p botster-core --test regression_shape_fixtures_test`
  - Prove stale-worker/adoption, restart-survival, and guarded-write shapes
    round-trip through public `botster_core` types and contain no product
    policy or PII.

Required existing checks to keep nearby surfaces stable:

- `cargo test -p botster-core --test session_protocol_test`
- `cargo test -p botster-core --test session_worker_engine_test`
- `cargo test -p botster-core --test managed_session_runtime_test`
- `cargo test -p botster-core --test botster_engine_api_test`
- `cargo test -p botster-core --test boundary_test`
- `cargo test -p botster-core --test actor_contract_test`
- `cargo test -p botster-core --test notification_inbox_test`

Required repo-level checks:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test --doc --workspace`

Runtime/user-path proof:

- This ticket is intentionally contract/scaffold work.
- Evidence must identify the changed public path as exported `botster_core`
  contract types, public docs, and tests/fixtures that instantiate and
  serialize those types through the real crate.
- Evidence that docs text exists is not enough; tests must prove the public
  typed contracts for identity/versioning, health, attach/detach,
  readiness-gated writes, delivery transitions, and restart semantics.
- Do not claim production hub restart, daemon restart, or worker adoption
  behavior changed until a later ticket wires a real daemon/session-worker
  runtime path.

## Vault Gaps Worth Capturing

No durable vault capture is required before implementation. Existing notes
already constrain core versus hub/CLI policy, terminal data-plane ownership,
session-worker engine boundaries, backpressure, plan artifacts, and Project
Pipelines checklist fallback.

Capture after implementation if the final API settles any of these durable
rules:

- Botster durable session-worker contracts live above the byte-frame
  `session_protocol` layer and must reuse `SessionIoRequest`/`SessionIoEvent`
  semantics where possible.
- Guarded session writes must distinguish accepted, queued/deferred, rejected,
  written/injected, and delivered/acknowledged, and must not treat acceptance as
  delivery.
- Core owns readiness evidence and deterministic scheduling mechanics while
  hosts/plugins own semantic write policy.
- Hub restart, core daemon restart, and worker death have separate survival
  semantics in Botster's durable session model.
