# Core Session Model And Activity Engine Plan

Ticket: `ticket_1780075964_158547`
Run: `run_1780077467_970052`

## Context Loaded

- Pipeline context: current step `botster_plan`, run `run_1780077467_970052`, ticket `ticket_1780075964_158547`, gate `botster_plan_gate`, no prior artifacts, reviews, findings, questions, or answers.
- Orchestrator correction: this run is main-rooted. Ignore dependency-populated `base_run_id`/`base_ticket_id` for stacking, target `main`, and plan against the workspace layout introduced at commit `e29bea1`.
- Closed dependency: `ticket_1780014900_202484` (`Build contract test fixtures from current regression shapes`).
- Required playbooks loaded: `planner-playbook` and `botster-planner-playbook`.
- Required Botster notes loaded from the planner overlay: `botster-architecture`, `cli-patterns`, `spa-patterns`, `project pipeline orchestration belongs in a device-level botster plugin`, `project pipelines needs an operator workbench not more primitives`, `project pipelines ui contract belongs in the plugin readme`, `botster orchestration should spawn agents with explicit target ids`, and `botster orchestration prompts must bind agents to explicit worktrees`.
- Ticket-specific vault note loaded: `session output activity derives from sessionio last output not client attachment`.
- General identity/goals context loaded from `/Users/jasonconigliari/knowledge/self/identity.md` and `/Users/jasonconigliari/knowledge/self/goals.md`.
- Existing repo context inspected: `README.md`, workspace `Cargo.toml`, `crates/botster-core/Cargo.toml`, `crates/botster-core/src/lib.rs`, `crates/botster-core/src/contract/mod.rs`, `crates/botster-core/src/contract/session.rs`, `crates/botster-core/src/contract/session_protocol.rs`, `crates/botster-core/src/contract/actor.rs`, `crates/botster-core/src/contract/client_stream.rs`, `crates/botster-core/src/engine/mod.rs`, `crates/botster-core/src/runtime/mod.rs`, and current session/regression tests.
- Prior plan artifacts inspected: `docs/plans/session-process-wire-protocol.md` and `docs/plans/contract-test-fixtures-regression-shapes.md`.

## Scope

Implement a small reusable core session model and activity reducer in `botster-core`.

In scope:

- Add public core session vocabulary for session kind/type without product-only assumptions. The shape should support known Botster uses such as terminal/agent/plugin-owned sessions while retaining an extension/custom escape hatch for embedders.
- Add serializable lifecycle/activity state to the public core session model. Reuse or align with existing `SessionLifecycleState` rather than creating conflicting lifecycle vocabularies.
- Track activity timestamps and byte accounting for input and output:
  - last input timestamp and input byte count;
  - last output timestamp and output byte count;
  - optional last declared activity timestamp for non-byte signals that core can classify deterministically without knowing host policy.
- Add a pure reducer/state-machine API for input, output, process/lifecycle, and declared activity events.
- Add deterministic active/idle classification from injected `now` and threshold values, based on time since the most recent input/output/declared activity signal.
- Ensure public session state serializes/deserializes with stable serde shapes and no PII-bearing fields.
- Update regression fixtures/tests so the current last-byte-active behavior is represented by reusable core logic, likely by adding a fixture for output-driven activity into `botster-core-test-support`.
- Add focused unit/integration tests that exercise the public entry point, not only private construction.
- Export the new public types from `crates/botster-core/src/contract/mod.rs` and `crates/botster-core/src/lib.rs`.

Botster layers touched:

- `botster-core` contract layer.
- `botster-core` engine layer if the reducer belongs under `engine/session_activity.rs`.
- `botster-core-test-support` only if a reusable regression fixture is needed.
- No hub, CLI, TUI, SPA, Rails relay, Lua plugin, MCP, or docs surface beyond this plan unless a README discoverability line proves necessary.

## Non-Scope

- No concrete timer scheduling, polling loops, async tasks, Tokio channels, persistence, auth policy, UI labels, workspace routing, recovery, process spawning, or product workflow interpretation.
- No port from `/Users/jasonconigliari/Rails/trybotster`; that path is reference evidence only and should not be required to exist in this repo.
- No Project Pipelines product policy or UI changes.
- No hard-coded agent-only or product-only session kind assumptions.
- No new dependencies unless implementation proves the standard library plus existing serde stack cannot express the model.
- No broad refactor of existing actor, transport, or client-stream contracts.

## Assumptions And Unknowns

- Assumption: existing `SessionMetadata.last_output_at` is protocol handshake evidence and can remain as-is; the reusable activity model should live in `session`/`engine` instead of overloading the wire protocol module.
- Assumption: `SessionLifecycleState` in `contract::actor` is reusable enough to re-export or embed in the session model; if it is too hub-control-specific, the implementer should add a narrowly named session lifecycle type and prove why it is not conflicting.
- Assumption: timestamps can be represented as integer milliseconds or seconds without a host clock trait, because classification takes injected `now` as data. The implementer should choose the existing crate style if one appears.
- Assumption: "active/idle" means recent activity within `threshold`, not process-running status. A running process with no recent input/output/declared activity should classify idle.
- Assumption: input bytes should count as activity because the ticket explicitly names last-input accounting, even if the old last-byte-active behavior was output-centered.
- Unknown: whether `SessionKind` should be a serde-tagged enum or a newtype/string plus known constructors. Prefer the smallest shape that avoids product-only assumptions and remains stable for embedders.
- Unknown: whether process events should include exit code/signal details or only update lifecycle. Reuse `ProcessExitedPayload` if exit details are needed by tests.
- Unknown: whether README should mention the new session activity surface. Add docs only if rustdoc/export names are not enough for discoverability.

## Affected Surfaces And Files

Expected:

- `crates/botster-core/src/contract/session.rs`: session kind/type vocabulary, serializable session state, activity fields, event/reducer-facing public types if they are contract data.
- `crates/botster-core/src/engine/mod.rs`: export a new engine module if reducer logic is separated from pure data.
- `crates/botster-core/src/engine/session_activity.rs` or equivalent: pure reducer/classifier if the implementation separates contracts from behavior.
- `crates/botster-core/src/contract/mod.rs`: public re-exports.
- `crates/botster-core/src/lib.rs`: crate-level public re-exports.
- `crates/botster-core/tests/session_activity_test.rs`: reducer, serialization, and classification acceptance tests.

Possible:

- `crates/botster-core-test-support/src/fixtures/regression/regression_shapes.rs`: add last-output/last-byte-active fixture data if useful for proving regression translation.
- `crates/botster-core/tests/regression_shape_fixtures_test.rs`: assert fixture translation into the new activity engine.
- `README.md`: optional one-line ownership-boundary update only if needed.

## Risks

- Core could absorb host policy by adding timers, scheduling, persistence, or UI status interpretation. Mitigation: reducer accepts events plus injected `now`/threshold and returns pure state/classification only.
- Session kind could become product-specific if it only models "agent". Mitigation: include generic terminal/process/plugin/custom vocabulary or use an extensible value type.
- Lifecycle vocabulary could split from existing `SessionLifecycleState`. Mitigation: reuse or deliberately align, with tests proving serde shape.
- Activity classification could accidentally tie active/idle to lifecycle running/exited. Mitigation: tests must show running-but-stale is idle and recent activity is active.
- Timestamp unit ambiguity could create off-by-one behavior at the threshold boundary. Mitigation: tests cover below, at, and above threshold.
- Adding public reducer APIs without wiring tests could become dead surface. Mitigation: tests call the public exported API exactly as embedders would.
- PII leakage could enter session labels/cwd/title fields. Mitigation: this ticket should not add human labels, cwd, title, prompts, or terminal content to the core activity state.

## Acceptance Checks And Tests

Planned targeted tests:

- `session_kind_serializes_without_product_only_assumptions`: known and custom session kinds round-trip through public serde types.
- `session_activity_updates_from_output_bytes`: output event updates `last_output_at`, output byte count, and latest activity.
- `session_activity_updates_from_input_bytes`: input event updates `last_input_at`, input byte count, and latest activity.
- `session_activity_updates_from_process_events`: lifecycle/process events update lifecycle without requiring byte activity unless explicitly declared.
- `declared_activity_signal_updates_latest_activity`: non-byte activity can refresh classification through a declared signal.
- `active_idle_classification_uses_injected_clock_and_threshold`: deterministic classification at fresh, boundary, and stale times.
- `running_session_without_recent_activity_is_idle`: lifecycle running does not force active.
- `session_activity_state_round_trips_public_json`: lifecycle and activity states serialize through public core types.
- `last_output_regression_shape_translates_to_core_activity`: current last-byte-active behavior is represented through the reusable activity reducer/fixture.

Commands:

- `cargo fmt`
- `cargo test -p botster-core session_activity`
- `cargo test -p botster-core regression_shape`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Runtime/user path proof:

- This ticket is core-mechanism work. The production entry point is the public `botster_core` API consumed by hub/embedding hosts later, not a concrete hub timer loop in this repo.
- Implementation evidence must show the exported public types/reducer are used by tests through `botster_core`, and any regression fixture translates last-output behavior into those public types.
- If the implementer intentionally leaves host runtime wiring out, the implementation report should state that scheduling/persistence/UI interpretation are out of scope by ticket design.

## Pipeline Gates And Artifacts

- Plan artifact: `docs/plans/core-session-model-activity-engine.md`.
- Plan gate evidence should reference this artifact plus pipeline context, loaded vault notes, assumptions, affected files, risks, and tests.
- Implementation gate should include committed work evidence, exact cargo/fmt/clippy outputs, and proof that the public API path changed.
- Review/verify should reject unwired private-only code, host-policy creep, product-only session kind assumptions, missing serde tests, or active/idle behavior tied to process lifecycle instead of activity timestamps.

## Vault Checklist Evidence

- Vault/project notes constrained the plan: `planner-playbook`, `botster-planner-playbook`, `botster-architecture`, `cli-patterns`, `spa-patterns`, Project Pipelines orchestration/workbench/UI-contract notes, explicit target/worktree orchestration notes, `plan steps need reviewable plan artifacts`, `session output activity derives from sessionio last output not client attachment`, and identity/goals context.
- Convention conflicts: none. The plan follows the Botster core boundary by keeping reusable state/reducer logic in `botster-core` and leaving host policy to hub/embedding hosts.
- Verification evidence so far: planning inspection only; no implementation commands were run beyond repository reads. Planned verification commands are listed above.
- Durable knowledge capture: no new vault note required before implementation. Capture later if the implementation establishes a reusable convention for core-owned pure reducers versus host-owned schedulers, or if timestamp/session-kind vocabulary needs a durable Botster architecture note.

## Vault Gaps Worth Capturing

- Capture if the final API establishes a durable rule that `botster-core` owns pure session activity reducers while hubs own timer scheduling and status projection.
- Capture if session kind vocabulary settles into a reusable extensibility pattern for core contracts.
- Capture if last-byte-active behavior has subtleties from the old runtime that are not already covered by existing `session output activity derives from sessionio last output not client attachment` context.
