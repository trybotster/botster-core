# Classify BoundaryJson Escape Hatches

## Context Loaded

- Pipeline context: `ticket_1780014883_660674`, `run_1780030158_985003`, current step `botster_plan`, gate `botster_plan_gate`.
- Dependency context: `ticket_1780014863_508751` is closed and already introduced the actor contract scaffold in `src/actor.rs`, `src/boundary.rs`, `src/transport.rs`, and `tests/actor_contract_test.rs`.
- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1780014883_660674`.
- Target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Required playbooks loaded:
  - `/Users/jasonconigliari/knowledge/notes/planner-playbook.md`
  - `/Users/jasonconigliari/knowledge/notes/botster-planner-playbook.md`
- Additional vault constraints loaded:
  - `/Users/jasonconigliari/knowledge/self/identity.md`
  - `/Users/jasonconigliari/knowledge/self/goals.md`
  - `/Users/jasonconigliari/knowledge/notes/botster-architecture.md`
  - `/Users/jasonconigliari/knowledge/notes/cli-patterns.md`
  - `/Users/jasonconigliari/knowledge/notes/spa-patterns.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipeline orchestration belongs in a device-level botster plugin.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipelines needs an operator workbench not more primitives.md`
  - `/Users/jasonconigliari/knowledge/notes/project pipelines ui contract belongs in the plugin readme.md`
  - `/Users/jasonconigliari/knowledge/notes/botster orchestration should spawn agents with explicit target ids.md`
  - `/Users/jasonconigliari/knowledge/notes/botster orchestration prompts must bind agents to explicit worktrees.md`
- Repo context inspected:
  - `src/boundary.rs`: `BoundaryJson(pub serde_json::Value)` exists with docs limiting use to Lua/plugin/relay payloads.
  - `src/actor.rs`: `TransportSignal`, `PluginWorkerMessage::Invoke`, and `PluginWorkerEvent::Completed` use `BoundaryJson`.
  - `src/transport.rs`: `TransportIngress::BoundaryPayload` and `TransportEgress::BoundaryPayload` use `BoundaryJson`; typed variants already exist for subscribe, input, resize, snapshot, focus, ping/pong, process exit, and attach state.
  - `tests/actor_contract_test.rs`: current `boundary_json_is_reserved_for_lua_plugin_or_relay_payloads` only proves presence and absence in one stable control debug string; it does not classify all escape hatches or require owner/reason metadata.
  - `README.md`: ownership boundary already states core owns reusable contracts and excludes runtime policy/product workflows.

## Scope

Implement the narrow classification pass for `BoundaryJson` after the actor-contract scaffold.

In scope:

- Add an explicit inventory of every allowed `BoundaryJson` escape hatch in current public core contracts.
- Require tests to record owner and reason metadata for each allowed escape hatch.
- Tighten architecture guardrails so stable Botster-owned controls do not silently use `BoundaryJson` or raw `serde_json::Value`.
- Cover ticket-named stable controls: terminal attach state, ping/pong, focus, kitty/mode/process exit, snapshot/subscribe/input/resize, and plugin/relay payload exceptions.
- Update repo documentation only where it makes the escape hatch classification discoverable from the public crate boundary.

Non-scope:

- No new runtime actor loops, mailbox implementations, relay implementation, Lua execution, WebRTC adapters, Rails relay code, or plugin policy.
- No broad redesign of actor contracts already introduced by the dependency ticket.
- No new generic JSON escape hatches or optional configurability.
- No copying old trybotster implementation from `/Users/jasonconigliari/Rails/trybotster`; those paths remain evidence only.
- No terminal mode/color contract expansion unless needed to express the ticket's explicit TODO exclusion.

## Assumptions And Unknowns

Assumptions:

- This ticket is a contract/test/documentation guardrail slice in `botster-core`; the production runtime path is downstream compile-time consumption of exported contracts, not an executable path in this crate.
- Current `BoundaryJson` uses are intentional only for relay-owned and plugin-owned payloads.
- `serde_json::Value` in `entity.rs` and `session_protocol.rs` is outside this ticket unless it represents actor control payloads; those modules have separate public contract reasons for flexible structured records and recovery identity metadata.
- Existing dependencies are sufficient.

Unknowns for implementation:

- Whether to represent owner/reason metadata as a small test-only fixture table or as public rustdoc/README classification text plus tests that assert the text stays in sync. Prefer the smallest reviewable shape.
- Whether terminal mode/kitty should be documented as an explicit TODO exclusion in this ticket or covered by existing `session_protocol::ModeFlags`. The dependency plan says pushed terminal mode-change events were intentionally excluded; this ticket should preserve that exclusion explicitly if not adding a typed control.

## Affected Surfaces/Files

Expected:

- `tests/actor_contract_test.rs`: add classification fixtures and stronger assertions over all `BoundaryJson`-using public actor/transport variants.
- `src/boundary.rs`: possibly refine `BoundaryJson` rustdoc to name owner/reason classification expectations.
- `src/actor.rs`: possibly refine comments on `TransportSignal`, `PluginWorkerMessage::Invoke`, and `PluginWorkerEvent::Completed`; avoid changing variant shapes unless tests reveal an unclassified use.
- `src/transport.rs`: possibly refine comments on `BoundaryPayload` variants.
- `README.md`: likely add a concise `BoundaryJson` subsection under ownership or migration guidance.
- `docs/archive/plans/boundary-json-escape-hatches.md`: this plan artifact.

Not expected:

- `Cargo.toml`
- `src/session.rs`, `src/client.rs`, or `src/session_protocol.rs` unless documenting the terminal mode/process-exit exclusion requires a targeted comment.
- Any old trybotster files.

Botster layers touched:

- Rust core contract crate only: actor contracts, boundary documentation, transport-neutral frame contracts, and tests.
- No plugin, Lua core, Rust hub runtime, TUI, React SPA, Rails relay, MCP, or Project Pipelines runtime changes.

Worktree/target assumptions:

- All work happens in the assigned worktree above for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The old trybotster evidence path is reference material only, not an implementation source.

## Risks

- A test that only checks debug strings can pass while a new stable control variant later embeds `BoundaryJson`; the guardrail must inspect the source or enumerate allowed escape hatches explicitly.
- Over-banning `serde_json::Value` could falsely fail entity/UI/session-protocol contracts whose flexible payloads are not actor-control escape hatches.
- Adding owner/reason fields to public contract types would broaden API churn; prefer classification evidence in tests/docs unless runtime consumers need the metadata.
- Treating terminal mode/kitty as raw JSON would violate the ticket; if not typed in actor controls, it must be explicitly excluded with the reason that mode flags are session-protocol state/probe data, not a pushed Botster-owned actor control in this slice.
- Documentation without tests would not prevent future ambiguity.

## Acceptance Checks/Tests

Required implementation checks:

- `cargo test boundary_json_is_reserved_for_lua_plugin_or_relay_payloads`
- `cargo test boundary_json_escape_hatches_are_classified_with_owner_and_reason`
- `cargo test stable_botster_controls_do_not_use_boundary_json`
- `cargo test`

Expected assertion shape:

- The allowed `BoundaryJson` inventory names every current actor/transport escape hatch on the base inspected during planning:
  - `TransportSignal.payload`: owner `relay`, reason encrypted or relay-owned signaling envelope.
  - `TransportIngress::BoundaryPayload.payload`: owner `relay/plugin`, reason adapter-owned ingress payload.
  - `TransportEgress::BoundaryPayload.payload`: owner `relay/plugin`, reason adapter-owned egress payload.
  - `PluginWorkerMessage::Invoke.payload`: owner `plugin`, reason plugin handler input schema is owned by the plugin.
  - `PluginWorkerEvent::Completed.payload`: owner `plugin`, reason plugin handler response schema is owned by the plugin.
- Stable Botster-owned controls are represented by typed variants/fields and are absent from the `BoundaryJson` inventory: attach state, ping, pong, focus/focus changed, request snapshot, subscribe/unsubscribe, terminal input, resize, terminal output, scrollback, process exit, client health/state, session lifecycle, and backpressure.
- Terminal mode/kitty is either typed through an existing contract (`ModeFlags`) or explicitly excluded from actor-control `BoundaryJson` usage with a TODO reason tied to session protocol state/probing, not raw JSON.
- Source-level guard prevents unclassified new `BoundaryJson` use in `src/actor.rs` and `src/transport.rs`.

Runtime/user path evidence:

- This crate is intentionally contract-only. The changed path to prove is public API consumption: downstream hub/client/session/plugin code imports typed core variants for stable controls and has only the documented plugin/relay escape hatches available.
- Evidence should include exported public types, serde round-trip tests where relevant, and tests that fail when a new unclassified actor/transport `BoundaryJson` occurrence appears.

Pipeline gates/artifacts:

- Attach this plan as the Plan gate artifact.
- Implementation gate should include command output for the targeted tests and full `cargo test`.
- Review should check that every changed line traces to the ticket, the loaded boundary conventions, or tests needed to enforce them.

## Vault Gaps Worth Capturing

Capture a durable vault note after implementation only if the final shape establishes a reusable convention beyond this repo plan, most likely:

- "BoundaryJson escape hatches require owner/reason classification" as a Botster core-contract guardrail.
- Or a clarified rule for terminal mode/kitty representation in `botster-core` if implementation has to settle that ambiguity.

No capture is needed before implementation because the existing vault notes already constrain the main architecture boundaries: core versus hub policy, session/client actor ownership, transport-neutral data plane contracts, plugin-owned payloads, and Project Pipelines artifact discipline.
