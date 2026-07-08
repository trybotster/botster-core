# Add UINode Application Primitives And Semantic Interaction Contracts

Ticket: `ticket_1783529011_837869`
Run: `run_1783529040_816681`
Step: Plan

## Context Loaded

- Pipeline context: ticket, active run, current Plan step, gate prompt, recent events, dependencies, artifacts, findings, questions, prior answers, reviews, and checklist state. There are no prior artifacts, findings, reviews, questions, answers, or dependencies for this run.
- Required playbooks: [[planner-playbook]] and [[botster-planner-playbook]].
- Required Botster vault context: [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], and [[botster orchestration prompts must bind agents to explicit worktrees]].
- Relevant prior plan artifacts: `docs/plans/ui-node-v1-primitives-forms-field-schemas.md`, `docs/plans/ui-action-validation-envelopes.md`, and `docs/plans/ui-capability-bindings-conformance-fixtures.md`.
- Repo context inspected:
  - `crates/botster-core/src/contract/ui.rs` already owns renderer-neutral `UiNode`, `UiNodeKind`, `UiCapabilitySet`, action request/result envelopes, field schemas, bindings, validation, and current primitive schema tables.
  - `crates/botster-core/tests/ui_contract_test.rs` already covers v1 primitive inventory, forms, action envelopes, capability downgrade checks, binding grammar, renderer-specific prop rejection, and public import paths.
  - `crates/botster-core-test-support/src/ui_conformance.rs` already exposes reusable renderer conformance fixtures consumed by downstream-style tests in `crates/botster-core-test-support/tests/downstream_conformance_test.rs`.
  - `crates/botster-core/src/contract/mod.rs` and `crates/botster-core/src/lib.rs` re-export UI contract types through public paths.
  - `README.md` documents that core owns portable UI vocabulary and validation shape only, while clients and hosts own concrete rendering, placement, and policy.

## Scope

Add the next practical application/dashboard primitives to the `botster-core` UI contract so plugin apps can describe operator dashboards without web/TUI-specific hardcoding.

In scope:

- Add semantic node kinds:
  - `metric`
  - `metric_grid`
  - `toolbar` or `action_bar`, with a preference for `toolbar` unless implementation finds an existing action-bar vocabulary in the repo.
  - `status_badge`
  - `section`
- Enhance existing node kinds:
  - `table`: typed columns/rows, stable row ids, primitive or child-node cells, empty state, row activation/action, and optional selection semantics.
  - `list`/`list_item`: selection and row/item action semantics aligned with table where the existing list slots allow it.
  - `panel`: semantic `density`, `variant`, and named slots such as `header`, `toolbar`, `body`, `footer`, `empty`, and `actions`.
  - `empty_state`: optional primary and secondary actions in addition to current title/description/icon/action support.
- Keep all new properties semantic and renderer-neutral: no Ionic, Catalyst, CSS, ratatui, Restty, DOM event names, layout class names, or client shell placement fields.
- Add validation tests for every new node kind and enhanced prop/slot contract.
- Add a composite conformance fixture using `metric_grid` + `table` + `toolbar` + `empty_state` + `status_badge` + `section`/`panel`.
- Update public root/module re-exports for any new typed helper structs/enums introduced for table columns/rows, metric trend, density, variant, selection, or row activation.
- Update docs to explain intended rendering semantics and explicitly defer `kanban`, `timeline`, `graph`, and `data_grid`.

Non-scope:

- No browser, TUI, React/Ionic, Catalyst, ratatui, Lua plugin, hub, daemon, MCP, Rails relay, or Project Pipelines product workflow implementation.
- No `data_grid` unless plain `table` cannot express a ticket-required contract. Current evidence says `table` can carry this slice.
- No high-level domain views: `kanban`, `timeline`, and `graph` are explicitly deferred.
- No legacy duplicate primitive names, version-suffixed aliases, or compatibility wrappers.
- No broad refactor of the raw `UiNode.props` model into a fully typed AST.
- No new dependencies unless existing `serde`, `serde_json`, and current validation helpers are proven insufficient.

## Proposed Contract Shape

Prefer additive changes in `crates/botster-core/src/contract/ui.rs` that keep the current raw node shape and validate structured props where semantics need stronger shape.

- `UiNodeKind::Metric`
  - Props: `label`, `value`, optional `caption`, `tone`, `status`, `trend`, `delta`, `action`, `ref`.
  - `label` and `value` are required. `trend` should be a small typed enum or typed object if direction plus value is needed.
- `UiNodeKind::MetricGrid`
  - Props: optional `density`, `variant`, `compact`.
  - Children should be metric nodes or semantic children; validation should reject renderer layout props.
- `UiNodeKind::Toolbar`
  - Props: optional `label`, `density`, `variant`.
  - Slots should allow `commands`, `filters`, `search`, and `actions`; keep it a semantic action/filter/search container, not panel styling.
- `UiNodeKind::StatusBadge`
  - Props: `label`, `status` and/or `tone`, optional `hover_label`, optional `action`.
  - It differs from generic `badge` by carrying compact state semantics.
- `UiNodeKind::Section`
  - Props: `title`, optional `description`, `density`, `variant`.
  - Slots: `header`, `toolbar`, `body`, `footer`, `empty`, `actions`, with `title` or `header` enough to establish the group.
- `Panel`
  - Keep existing `title`/`tone`; add `density`, `variant`.
  - Add named slots `header`, `toolbar`, `body`, `footer`, `empty`, and `actions`.
- `EmptyState`
  - Keep `title`, `description`, `icon`, `action`; add `primary_action` and `secondary_action` or action slots if that fits the existing validation style better. Prefer typed action props if only one or two actions are needed.
- `Table`
  - Replace or extend simple `columns: ["name"]` with validation that accepts typed columns while preserving the existing simple string-array shape if tests show it is current public contract.
  - Rows should have stable ids, cell maps keyed by column id, and optional row action/activation.
  - Cells may be primitive JSON values or child `UiNode` values. Use typed deserialization for the table prop instead of ad hoc string parsing.
  - Selection semantics should be explicit: none/single/multiple plus selected row ids if owner-controlled selection is present.
- `List`/`ListItem`
  - Add list-level selection semantics and item-level action/activation fields matching table naming where practical.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/contract/ui.rs`
  - `UiNodeKind` additions.
  - New typed helper structs/enums for metric trend/status, density/variant if needed, table columns/rows/cells, and selection semantics.
  - `schema_for`, prop validation, slot validation, stable-id rules, action requirements, and capability checks where needed.
- `crates/botster-core/src/contract/mod.rs`
  - Public re-exports for new helper types.
- `crates/botster-core/src/lib.rs`
  - Root re-exports for new helper types.
- `crates/botster-core/tests/ui_contract_test.rs`
  - Contract tests for new node kinds, enhanced table/list/panel/empty-state behavior, invalid prop/kind failures, and public import paths.
- `crates/botster-core-test-support/src/ui_conformance.rs`
  - New composite app-screen fixture exercising the required primitive mix.
- `crates/botster-core-test-support/tests/downstream_conformance_test.rs`
  - Assertion that the conformance fixture set includes the new application/dashboard fixture.
- `README.md`
  - Short UI contract paragraph documenting semantics and explicit deferrals.

Optional:

- A focused doc under `docs/architecture/` only if README becomes too dense. Prefer README for discoverability.

Not expected:

- `Cargo.toml`.
- Runtime engines, daemon APIs, package manifests, entity store internals, terminal/session code, or plugin worker execution.
- Browser/TUI/client renderer files; this checkout is the core contract crate.

Botster layers touched:

- Rust core contract crate and public test-support fixtures only.
- No plugin, Lua core, Rust hub runtime, session/client worker, TUI, React SPA, Rails relay, MCP, or Project Pipelines runtime changes.

Worktree/target assumptions:

- Run target id is `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Implementer should work only in this assigned run worktree.

## Assumptions And Unknowns

Assumptions:

- This is intentionally contract/scaffold work in `botster-core`. The production path to prove is public downstream consumption of `botster_core::ui` and reusable `botster_core_test_support::ui_conformance` fixtures.
- The existing raw `UiNode.props` map is intentional. Add typed validation for complex props, but do not replace the whole node model.
- `table` can express the needed workhorse data contract; `data_grid` remains deferred.
- Existing `Badge` stays generic; new `StatusBadge` carries compact status semantics because the ticket explicitly names it.
- `toolbar` is the preferred action-container primitive name because it is a common cross-client concept and avoids panel-style ambiguity. Use `action_bar` only if implementation finds current repo vocabulary requiring that name.
- Named slots already exist in the model and are appropriate for `panel` and `section`.
- Unknown props and unknown node kinds should continue failing clearly.
- The current form action/result semantics already cover field errors, form errors, pending/submitting-adjacent owner result states, and validation outcomes. This ticket should add tests only if a gap remains, not duplicate prior action-envelope work.

Unknowns for implementation:

- Exact table column/row/cell helper names. Prefer simple names like `UiTableColumn`, `UiTableRow`, `UiTableCell`, and `UiSelectionMode` if the code benefits from typed structs.
- Whether table cells as child nodes should be encoded directly in `rows[].cells` or through row slots/children. Prefer the shape that round-trips cleanly through serde and validates recursively without custom renderer assumptions.
- Whether `density`/`variant` should be shared enums across panel/section/toolbar/metric_grid or string tokens. Prefer shared enums if they prevent invalid vocabulary without overfitting.
- Whether `status` and `tone` should both be accepted on `status_badge` and `metric`. If both exist, tests must make the semantics clear.

No blocking human question is needed. The only plausible naming ambiguity is `toolbar` vs `action_bar`; the ticket allows either, and the plan chooses `toolbar` as the smallest conventional primitive unless repo evidence contradicts it.

## Risks

- Adding renderer-specific prop names would violate the core boundary and make downstream TUI/browser parity harder.
- Over-designing `table` into `data_grid` would ignore the ticket's explicit deferral.
- Keeping only a simple `columns` array and child text rows would fail the row id, cell, empty state, row action, and selection requirements.
- Adding duplicate aliases such as both `toolbar` and `action_bar` would create legacy vocabulary immediately.
- Weak conformance fixtures could prove type existence without proving a real composite app screen can be represented.
- Changing existing simple table/list/panel behavior without compatibility tests could break downstream consumers pinned to the current contract.
- Treating action/result pending or validation as client-owned policy would conflict with the existing owner-authored action envelope boundary.

## Acceptance Checks / Tests

Required targeted tests:

- `application_primitive_inventory_includes_dashboard_nodes`
- `metric_and_metric_grid_round_trip_semantic_values`
- `toolbar_declares_commands_filters_search_and_actions_without_renderer_props`
- `status_badge_carries_status_without_reusing_renderer_style`
- `section_and_panel_named_slots_validate`
- `empty_state_accepts_primary_and_secondary_actions`
- `table_round_trips_columns_rows_stable_ids_and_node_cells`
- `table_rejects_rows_without_stable_ids`
- `table_selection_and_row_activation_are_semantic`
- `list_selection_and_item_actions_match_table_semantics`
- `renderer_specific_application_props_are_rejected`
- `deferred_high_level_views_are_rejected_as_unknown_node_kinds`
- `public_api_import_path_exposes_application_ui_contract_types`
- `ui_renderer_conformance_includes_application_dashboard_fixture`

Required command evidence:

- `cargo fmt --all -- --check`
- `cargo test -p botster-core --test ui_contract_test`
- `cargo test -p botster-core-test-support downstream_conformance`
- `cargo test -p botster-core-test-support --no-default-features`
- `cargo test -p botster-core --no-default-features --lib`
- `cargo clippy -p botster-core -p botster-core-test-support --all-targets --all-features -- -D warnings`

Broader checks if shared exports or docs produce unexpected drift:

- `cargo test -p botster-core`
- `cargo test --workspace`
- `cargo doc --workspace --no-deps`

Runtime/user-path evidence:

- This ticket is intentionally core-contract only.
- The changed runtime/user path is downstream public API consumption: a plugin or host can emit a dashboard-shaped `UiNode` tree, `botster-core` can deserialize and validate it, invalid stale/client-specific output fails clearly, and downstream renderer tests can use the shared conformance fixture to prove support.
- Implementation evidence must therefore include public import/serde/validation tests and a composite conformance fixture, not only enum additions.

Pipeline gates/artifacts:

- Plan gate should attach this document and checklist evidence.
- Implement gate should attach exact command output summaries and explain any unrelated failures precisely.
- Review should verify renderer neutrality, no deferred primitives were added, no duplicate aliases exist, public exports are complete, and the composite fixture actually exercises the production contract path.

## Vault Gaps Worth Capturing

Capture after implementation only if the final code settles durable conventions not already in the vault:

- Shared `density`/`variant` vocabulary for cross-client UI primitives.
- Stable table row/cell wire shape for plugin-authored dashboards.
- When a generic `badge` should become a semantic `status_badge`.
- Whether `toolbar` becomes the canonical action/filter/search container name over `action_bar`.

No immediate vault capture is needed before implementation. Existing notes already cover the main constraints: core owns reusable contracts, plugin workflow policy stays out of core, cross-client UI primitives must remain semantic, and Project Pipelines needs richer operator surfaces without adding product policy to core.
