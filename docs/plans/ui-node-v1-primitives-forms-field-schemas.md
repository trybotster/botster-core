# Define UiNode v1 Primitives, Forms, And Field Schemas

Ticket: `ticket_1780939862_945903`
Run: `run_1780939886_929834`
Step: Plan

## Context Loaded

- Pipeline context: ticket, run, current Plan step, gate prompt, events, open questions, prior answers, findings, artifacts, reviews, and dependencies. There are no prior artifacts, reviews, findings, questions, or answers for this run.
- Required playbooks: [[planner-playbook]] and [[botster-planner-playbook]].
- Botster planning constraints: [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], and [[plan agents must author vault context as wikilinks not home paths]].
- Cross-client UI contract learnings: [[cross-client ui should share semantic primitives and actions with renderer-specific adapters]], [[botster shared form primitive v1 is intentionally narrow and catalyst first]], [[controlled vs uncontrolled widget ownership follows explicit value means Lua owns state]], [[use slots not positional children for compound components with semantic regions]], [[phase one action ids are semantic botster events not DOM event names]], [[phase one web ui composites stay internal while Lua public contract stops at primitives]], and [[web runtime v1 contract uses additive versioning with breaking changes requiring v2]].
- Artifact discipline: [[plan steps need reviewable plan artifacts]].
- Repo context inspected:
  - `crates/botster-core/src/contract/ui.rs` already owns renderer-neutral `UiNode`, `UiNodeKind`, bindings, responsive values, `UiAction`, and recursive validation over a raw `props` map.
  - `crates/botster-core/tests/ui_contract_test.rs` already covers minimal/populated wire shapes, required props and slots, renderer-specific prop rejection, bindings, responsive values, token validation, action pending/result identity, and public import paths.
  - `crates/botster-core/src/contract/mod.rs` and `crates/botster-core/src/lib.rs` re-export the current UI public API.
  - `README.md` documents `botster-core` as the policy-free reusable contract/engine crate.
- Baseline verification: `cargo test -p botster-core ui_contract` passed before planning. The filter currently runs 0 named tests because test names do not include the substring `ui_contract`; use the explicit integration-test target in implementation.

## Scope

Implement the `botster-core` UiNode v1 contract slice for semantic primitives, forms, field metadata, validation hints, stable ids, state ownership, and validation.

In scope:

- Define the public v1 primitive inventory in `UiNodeKind` and tests:
  - Layout: `stack`, `inline`, `panel`, `scroll_area`.
  - Content: `text`, `icon`, `badge`, `status_dot`, `empty_state`.
  - Collections: `list`, `list_item`, `tree`, `tree_item`, `table`.
  - Actions: `button`, `icon_button`, `menu`, `menu_item`, `dialog`.
  - Forms/inputs: `form`, `form_section`, `form_field`, `text_input`, `textarea`, `checkbox`, `select`, `select_option`.
  - Botster-specialized placeholders already in core: `terminal_view`, `connection_code_view`.
- Add first-class `FormSection` and `FormField` semantics without introducing renderer-specific layout, CSS, Ionic, ratatui, Restty, or host policy.
- Add typed field schema contracts for the narrow v1 field kinds: text input, textarea, checkbox, and select.
- Add validation-hint metadata for fields. These are hints for renderers and plugin authors, not core-side business-rule enforcement.
- Cover field metadata: name, label, description/help text, placeholder where relevant, options for select, required flag, default value, disabled/loading/error state, and validation hints.
- Preserve the controlled vs renderer-local ownership rule: explicit `value`, `checked`, or `selected` means the plugin/core author owns state; absence of those props with a stable node id allows renderer-local state initialized from defaults.
- Require stable node ids where the runtime needs identity: action-emitting nodes, controlled fields, renderer-local/uncontrolled fields with defaults, `form`, `form_section`, and `form_field`.
- Keep unknown primitive and unknown prop behavior fail-closed in `botster-core`: serde rejects unknown `type` values and `validate_ui_node` rejects unknown props.
- Add tests that compare the intended v1 contract against the cross-client UI learnings listed above, without importing or copying old trybotster implementation code.
- Update public exports from `contract/mod.rs` and `lib.rs` for new public schema/state types.
- Add concise rustdoc or README documentation only where needed to make the public contract discoverable.

Non-scope:

- No Ionic, ratatui, Restty, CSS, Catalyst, React, TUI widget, browser adapter, or CLI fallback implementation.
- No Project Pipelines product workflow policy or plugin-specific field semantics.
- No custom form validation engine, expression language, regex execution policy, or server-side submission handling.
- No broad migration of old trybotster UI code, builder helpers, Lua APIs, or renderer registries.
- No new dependencies unless implementation proves `serde`/`serde_json`/`thiserror` are insufficient.
- No PII or local path references in docs or tests.

## Proposed API Shape

Keep `UiNode` as the structural contract and add typed helpers for the pieces that need public semantics.

- Add `UiNodeKind::FormSection` and `UiNodeKind::FormField`.
- Add a small field-schema family under `contract::ui`:
  - `UiFieldKind`: `Text`, `Textarea`, `Checkbox`, `Select`.
  - `UiFieldSchema`: field kind, name, label, optional description/help, optional placeholder, required flag, optional default value, optional validation hints, and select options.
  - `UiFieldOption`: string or JSON value plus label and optional disabled state.
  - `UiFieldValidationHints`: optional min/max length, pattern hint, min/max numeric hint, or one-of/options consistency where relevant.
  - `UiFieldState` or equivalent node-level state representation for disabled/loading/error if a typed struct keeps validation cleaner than three independent raw props.
- Prefer serde-compatible structs for schema validation, but keep field values as `serde_json::Value` where type flexibility is part of the renderer-neutral contract.
- `Form` should remain a semantic container with a semantic `action`; it may accept semantic state props such as `disabled`, `loading`, and `error`.
- `FormSection` should group related fields and use explicit semantic slots or children. Accept `title`, optional `description`, and state props only if needed.
- `FormField` should carry the field schema and state. It should use a named slot for a custom control only if the implementation needs to preserve existing input primitives as explicit children; otherwise it can be schema-driven. Prefer the smallest model that still lets renderers map one field to one v1 control.
- Existing input nodes (`text_input`, `textarea`, `checkbox`, `select`, `select_option`) should remain valid primitives and align with `UiFieldSchema` rather than drift into a second field vocabulary.

## State Ownership Rules

Implementation should make these rules testable:

- Controlled text-like and select fields: `value` present means plugin-owned state.
- Controlled checkbox fields: `checked` present means plugin-owned state.
- Renderer-local state: `value`/`checked`/`selected` absent, stable node id present, optional default value present.
- Defaults initialize renderer-local state only; defaults must not be treated as authoritative updates after initial render.
- A field with renderer-local state but no stable id should fail validation because renderers cannot preserve local state across tree refreshes.
- Action pending/result correlation continues to use `UiActionPending.node_id` and `UiActionResult.node_id`; action-emitting nodes therefore need stable ids.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/contract/ui.rs`: primitive enum additions, field schema/state types, schema table updates, recursive validation updates, stable-id checks, and rustdoc.
- `crates/botster-core/src/contract/mod.rs`: re-export new UI contract types.
- `crates/botster-core/src/lib.rs`: root re-export new UI contract types.
- `crates/botster-core/tests/ui_contract_test.rs`: acceptance tests for primitive inventory, forms, field schemas, state ownership, states, defaults, and unknown primitive/prop behavior.
- `README.md`: optional short note if rustdoc alone does not make the v1 UI contract discoverable.
- `docs/plans/ui-node-v1-primitives-forms-field-schemas.md`: this plan artifact.

Not expected:

- `Cargo.toml` unless a new dependency is proven necessary.
- `crates/botster-core/src/engine/*`, runtime, daemon, terminal, package, identity, or entity modules.
- Browser, TUI, Rails, Lua plugin, Ionic, Restty, or Catalyst files. They are downstream renderer/host surfaces and not present in this core checkout.

Botster layers touched:

- Rust core contract crate only: `contract::ui`, public exports, tests, and optional docs.
- No plugin, Lua core, Rust hub runtime, session/client worker, TUI renderer, React SPA, Rails relay, MCP, or Project Pipelines runtime change.

Worktree/target assumptions:

- The pipeline assigned this run to target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The implementation agent should work only in the assigned run worktree and should not use old trybotster paths as source material.

## Assumptions And Unknowns

Assumptions:

- The current `UiNode` raw `props` map is intentional for renderer-neutral wire compatibility; typed structs should validate field schemas rather than replace the node tree with a large typed AST.
- Adding `form_section` and `form_field` is additive within the v1 contract and does not require a v2 bump.
- Unknown primitive rejection is the correct core behavior. Renderer-safe fallback can be a downstream adapter concern after validation failure, not a permissive core parser.
- Disabled/loading/error are semantic state, not renderer style. They may be accepted on form, field, and action nodes if validation keeps the meaning narrow.
- Existing `UiAction.disabled` should not be removed. If node-level disabled is added, tests should document how it coexists with action-level disabled rather than creating ambiguous behavior.
- The old trybotster UI contract is reference evidence only. This ticket should encode the intended contract from current notes and repository shape.

Unknowns for implementation:

- Whether `FormField` should be schema-driven only or require/allow a `control` slot containing one of the narrow v1 input primitives. Prefer schema-driven unless tests reveal a clearer cross-client contract with slots.
- Whether default values belong only in `UiFieldSchema` or also as node props on individual input primitives for backward compatibility with existing `text_input`, `textarea`, `checkbox`, and `select`.
- Whether validation hints should be one struct with optional fields or an enum list. Prefer the shape that produces the clearest JSON and least overfitting.
- Whether stable id should become globally required for all `UiNode`s. Current contract allows id-less static nodes, so prefer requiring ids only where state or action correlation needs them.

No human question is blocking this plan. The plausible ambiguity around unknown primitives is resolved by choosing the existing core posture: fail-closed validation and serde rejection.

## Risks

- Over-expanding form controls would violate the narrow v1 form note and create renderer obligations before TUI/web adapters are ready.
- Letting field schemas become a validation engine would move product policy into core.
- Allowing renderer props such as CSS class names, Ionic component names, ratatui layout hints, or Restty-specific fields would violate the core transport-neutral boundary.
- Adding node-level disabled/loading/error without clear tests could conflict with existing `UiAction.disabled`.
- Requiring ids for every node would be unnecessary API churn; requiring too few ids would break pending action feedback and renderer-local state preservation.
- Accepting unknown props for forward compatibility would weaken the contract tests and make stale plugin output harder to diagnose.
- A doc-only implementation would not prove the production path. Public serde and validation tests must exercise downstream-style imports.

## Acceptance Checks / Tests

Required tests should be named directly against ticket acceptance:

| Requirement | Suggested test |
| --- | --- |
| Public v1 primitive list | `ui_node_v1_primitive_inventory_is_explicit` |
| Form wire shape | `form_and_form_section_round_trip_wire_shape` |
| Field schema wire shape | `form_field_schema_round_trips_for_v1_field_kinds` |
| Field metadata | `field_schema_accepts_metadata_without_renderer_props` |
| Validation hints | `field_schema_validation_hints_are_metadata_not_policy` |
| Default values | `field_defaults_are_representable_for_each_v1_field_kind` |
| Controlled state | `explicit_value_checked_or_selected_marks_field_controlled` |
| Renderer-local state | `renderer_local_fields_require_stable_node_ids` |
| Disabled/loading/error state | `form_field_and_action_state_props_validate` |
| Stable action ids and node ids | `action_emitters_require_stable_node_ids_for_pending_feedback` |
| Unknown primitive rejection | `unknown_ui_node_kind_is_rejected` |
| Unknown prop rejection | keep/extend `renderer_specific_props_are_rejected` |
| Renderer neutrality | `renderer_specific_form_props_are_rejected` |
| Public consumer path | `public_api_import_path_exposes_v1_form_schema_types` |

Required verification commands:

- `cargo fmt --all -- --check`
- `cargo test -p botster-core --test ui_contract_test`
- `cargo test -p botster-core --no-default-features --lib`
- `cargo test -p botster-core`

Full workspace checks from `README.md` are desirable if the implementation touches shared exports broadly:

- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo doc --workspace --no-deps`

Runtime/user-path evidence:

- This ticket is intentionally scaffold/contract-only in `botster-core`.
- The production path to prove is downstream public API consumption: hosts, plugins, browser, TUI, and CLI-ish fallback renderers import `botster_core::ui` or root re-exports, deserialize protocol-shaped `UiNode`s, validate them, and receive deterministic failures for stale or renderer-specific output.
- Evidence should therefore be public integration tests using `botster_core::{UiNode, UiNodeKind, ...}` and `botster_core::ui::{...}` rather than private helper-only tests.

Pipeline gates/artifacts:

- Plan gate should attach this plan and note checklist evidence.
- Implementation gate should include exact command evidence and whether failures are unrelated or introduced by the change.
- Review should verify no renderer policy entered core, no old trybotster code was copied, public exports are complete, and tests cover every acceptance criterion.

## Vault Gaps Worth Capturing

Capture after implementation only if the final shape settles a durable rule not already in the loaded notes. Candidate gaps:

- A precise Botster convention for whether `form_field` is schema-driven or slot-driven.
- A precise Botster convention for how node-level disabled/loading/error coexists with `UiAction.disabled`.
- A precise Botster convention for stable node id requirements by primitive category.

No vault capture is needed before implementation because the current notes already constrain the main architecture: cross-client primitives, narrow v1 forms, controlled state ownership, slot semantics, additive v1 versioning, and core-vs-renderer boundaries.
