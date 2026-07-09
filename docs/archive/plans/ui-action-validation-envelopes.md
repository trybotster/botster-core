# Typed UI Action And Validation Envelopes

Ticket: `ticket_1780939862_251954`
Run: `run_1780939887_306674`
Step: Plan

## Context Loaded

- Pipeline context: ticket, run, active Plan step, required gate prompt, empty artifacts/findings/questions/reviews, current run identity, and no prior answers.
- Required playbooks: [[planner-playbook]] and [[botster-planner-playbook]].
- Required Botster overlays: [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], and [[botster orchestration prompts must bind agents to explicit worktrees]].
- Artifact convention: [[plan steps need reviewable plan artifacts]].
- Repo context:
  - `crates/botster-core/src/contract/ui.rs` already owns renderer-neutral UI nodes, bindings, viewport types, and a minimal `UiAction`/`UiActionPending`/`UiActionResult` contract.
  - `crates/botster-core/tests/ui_contract_test.rs` already tests UI node serde, validation, and minimal pending/result correlation.
  - `crates/botster-core/src/contract/mod.rs` and `crates/botster-core/src/lib.rs` re-export UI contract types as the public consumer path.
  - `crates/botster-core/src/contract/actor.rs` already has plugin handler/descriptor kinds for UI actions, but it currently carries invocation payloads through `BoundaryJson`; this ticket should not move plugin execution policy into core.
  - Current dependencies already include `serde`, `serde_json`, and `thiserror`; no JSON Schema engine is present or needed.
- Checklist note: creating the Project Pipelines vault checklist timed out at the plugin-worker boundary. Per [[project pipelines checklist worker timeouts require artifact evidence fallback]], vault/context and verification evidence are preserved here and in gate evidence.

## Scope

Add transport-neutral typed UI action request and result envelopes to `botster-core` so plugin/host/client UI interactions can round-trip submit, reset, validate, and cancel semantics with request/result correlation and owner-authored validation outcomes.

In scope:

- Add action request contracts that carry:
  - request id
  - surface id
  - optional node id
  - action id
  - action kind/intent: submit, reset, validate, cancel
  - optional form values payload
  - optional action payload for non-form metadata
- Add typed result contracts that carry:
  - matching request id, surface id, node id, and action id
  - result state: accepted, rejected, deferred, error
  - field errors, form errors, warnings, normalized values
  - optional UI tree patch or replacement references
  - optional payload/error details where appropriate
- Keep validation authority with the action owner/host/plugin. Clients may use hints for preflight UX only; result envelopes must represent authoritative owner responses.
- Add tests proving submit and validate round trips, field-level errors, normalized values, rejected actions, and request/result correlation.
- Re-export new public contract types through `contract/mod.rs` and `lib.rs`.

Non-scope:

- No JSON Schema engine dependency.
- No renderer-specific assumptions, DOM events, Catalyst/React implementation, TUI rendering behavior, or browser-only naming.
- No hub/plugin worker execution policy changes.
- No Project Pipelines workflow policy or plugin README changes unless implementation discovers a repo-local UI contract drift that must be documented separately.
- No broad refactor of UI node validation, entity frames, actor contracts, transport contracts, or existing action descriptor semantics beyond compatibility needed for the new envelopes.
- No PII-bearing fields or local path metadata in the contract.

Botster layer touched: Rust core contract crate, cross-client UI contract surface.

Worktree/target assumption: downstream steps operate in this pipeline-assigned worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.

## Proposed API Shape

Prefer a narrow additive shape in `crates/botster-core/src/contract/ui.rs`:

- `UiSurfaceId(String)`.
- `UiActionKind` with `Submit`, `Reset`, `Validate`, and `Cancel`.
- `UiFormValues` as a transparent wrapper around `serde_json::Map<String, Value>` or a type alias if the implementation stays clearer.
- `UiActionRequest`:
  - `request_id: UiActionRequestId`
  - `surface_id: UiSurfaceId`
  - `node_id: Option<UiNodeId>`
  - `action_id: UiActionId`
  - `kind: UiActionKind`
  - `values: Option<UiFormValues>`
  - `payload: Option<Value>`
- `UiActionResultState` with `Accepted`, `Rejected`, `Deferred`, and `Error`.
- `UiFieldError { field: String, message: String }` or a map keyed by field name. Prefer the map only if tests need multiple errors per field to stay natural.
- `UiTreeUpdateRef` or similarly small enum for optional UI update references:
  - patch reference
  - replacement reference
  Keep this as an identifier/reference contract, not a tree diff engine.
- `UiActionResult` updated or replaced with fields for result state, errors, warnings, normalized values, optional UI tree update reference, payload, and error detail.

Naming can shift during implementation, but the wire vocabulary should remain action/form/result oriented and renderer neutral. If changing existing `UiActionResult` from `success`/`failure` to the new states is a breaking compile change inside this crate, prefer the cold-turkey migration over carrying parallel versioned names unless a concrete downstream compatibility boundary appears.

## Assumptions And Unknowns

Assumptions:

- The existing `UiActionPending` type is early pending-state scaffolding. It can remain as a client presentation helper if useful, but the new `UiActionRequest` is the authoritative outbound request envelope.
- `surface_id` should be a typed wrapper because plugin surfaces own navigation and action routing.
- `node_id` remains optional because some actions can be surface-level or form-level rather than emitted by one primitive node.
- Form values are JSON values because field primitives include text, textarea, checkbox, and select, and plugin-owned forms may carry plugin-specific value shapes.
- `normalized_values` are returned by the owner and should not imply client-side coercion authority.
- UI tree patch/replacement references are optional response metadata only; core should not implement patch application in this ticket.

Unknowns to resolve during implementation:

- Whether `field_errors` should be `BTreeMap<String, Vec<String>>` or `Vec<UiFieldError>`. Prefer stable deterministic JSON and ergonomic field lookup.
- Whether `reset` requests should carry no values by convention or allow values for reset-to-specific-state flows. Tests should pin the chosen behavior only if the contract enforces it.
- Whether to keep `UiActionStatus` as a deprecated compatibility enum is likely no. Existing tests should be migrated to the new result state unless a compile-time consumer in this repo forces a transition.
- Whether warning entries need typed field scope now. Prefer plain string warnings unless field-scoped warnings are required by acceptance.

No human question is required before implementation; the ticket wording is specific enough and no acceptance item needs a waiver.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/contract/ui.rs`
  - Add typed request/result envelope structs and enums.
  - Update action result state vocabulary.
  - Keep serde field names stable and transport neutral.
- `crates/botster-core/src/contract/mod.rs`
  - Re-export new UI contract types.
- `crates/botster-core/src/lib.rs`
  - Re-export new UI contract types from the crate root.
- `crates/botster-core/tests/ui_contract_test.rs`
  - Replace the old minimal result test with acceptance-focused request/result round-trip tests.

Possible but not expected:

- `README.md` if implementation creates a public contract detail that needs a top-level mention.
- `crates/botster-core/tests/actor_contract_test.rs` only if public escape-hatch or handler-contract tests need adjusted import names.

Not expected:

- `Cargo.toml`.
- `crates/botster-core/src/contract/transport.rs`.
- `crates/botster-core/src/contract/entity.rs`.
- Runtime engine, plugin worker, browser, TUI, Rails, or Project Pipelines plugin files.

## Production / Runtime Path

This ticket is intentionally contract-level in `botster-core`. The production path changed by the implementation should be:

1. A client emits a `UiActionRequest` with `request_id`, `surface_id`, `node_id`, `action_id`, action kind, and optional form values.
2. The hub/plugin owner executes the action through the existing plugin-owned UI action boundary.
3. The owner returns `UiActionResult` with the same request/action/surface identity and authoritative accepted/rejected/deferred/error state.
4. Clients render pending/result feedback and apply owner-returned validation details or optional UI update references without assuming a renderer or owning validation truth.

Because this checkout is the core substrate crate, tests are the runtime-path proof: they must instantiate the exported request/result contracts, serialize them to protocol JSON, deserialize them as a downstream client/host would, and assert identity correlation and validation payloads survive the round trip. Type existence alone is not enough.

## Risks

- Keeping the old `success`/`failure` vocabulary would under-specify rejected/deferred/error outcomes and fail the ticket.
- Making clients authoritative validators would violate the explicit owner/host/plugin validation boundary.
- Adding a JSON Schema dependency would contradict repo direction and the ticket unless a strong existing-repo reason appears.
- Modeling UI patch application in core would overreach; references are enough for this ticket.
- Using DOM event names or renderer fields would break cross-client browser/TUI parity.
- Carrying PII or local paths in requests/results would violate the ticket and artifact hygiene.
- Weak tests could prove serde shape without proving request/result correlation.

## Acceptance Checks / Tests

Add or update targeted tests in `crates/botster-core/tests/ui_contract_test.rs`:

- `ui_action_submit_request_round_trips_form_values`
  - Serialize/deserialize a submit request with `request_id`, `surface_id`, `node_id`, `action_id`, and text/checkbox/select values.
- `ui_action_validate_round_trip_returns_field_errors`
  - Serialize/deserialize a validate request and rejected result with field-level errors and form errors.
- `ui_action_result_returns_normalized_values_and_warnings`
  - Assert normalized owner-returned values and warnings survive result serde.
- `ui_action_rejected_result_preserves_request_correlation`
  - Assert rejected action result preserves request id, action id, surface id, and node id.
- `ui_action_deferred_and_error_states_are_distinct`
  - Assert deferred and error result states are separate wire values and error details serialize only when present.
- `ui_action_result_can_reference_ui_tree_patch_or_replacement`
  - Assert optional patch/replacement references serialize without embedding renderer assumptions.
- `public_api_import_path_exposes_action_envelopes`
  - Import new types through both `botster_core::ui` and crate-root re-exports.

Required verification commands for implementer:

- `cargo fmt`
- `cargo test -p botster-core ui_contract`
- `cargo test -p botster-core`
- `cargo clippy -p botster-core --all-targets --all-features -- -D warnings`

All new public types, enum variants, fields, and methods need doc comments because `missing_docs = "warn"` is configured. Non-test code should avoid `unwrap` because clippy warns on `unwrap_used`.

## Vault Gaps Worth Capturing

- Potential capture after implementation: the exact v1 wire vocabulary for UI action request/result envelopes if it becomes a stable convention across Botster core, browser, TUI, and plugin workers.
- Potential capture after implementation: whether field errors settle on map form or list form, since that affects future plugin authoring and client adapters.
- No new convention conflict was found during planning. The plan follows the loaded Botster boundary notes: core owns reusable typed contracts; plugin/host owners keep validation authority and workflow policy.
