# Define UiCapabilitySet, Bindings, And Renderer Conformance Fixtures

## Context Loaded
- Pipeline context for run `run_1780943875_789780`, step `botster_plan`, ticket `ticket_1780939862_542875`.
- Dependencies are closed: `Define UiNode v1 primitives, forms, and field schemas in botster-core` and `Add typed UI action and validation round-trip envelopes to botster-core`.
- Vault/playbook context: [[planner-playbook]], [[botster-planner-playbook]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], and [[botster orchestration prompts must bind agents to explicit worktrees]].
- Repo context inspected: `crates/botster-core/src/contract/ui.rs`, `crates/botster-core/src/contract/mod.rs`, `crates/botster-core/tests/ui_contract_test.rs`, `crates/botster-core-test-support/src/lib.rs`, `crates/botster-core-test-support/src/conformance/mod.rs`, and `crates/botster-core-test-support/tests/downstream_conformance_test.rs`.

## Scope
- Add renderer-neutral capability negotiation types to `botster_core::ui`, centered on a `UiCapabilitySet` that describes what a renderer can support.
- Cover the ticket's capability vocabulary with typed, serializable contract fields: viewport classes, pointer, keyboard, hover, clipboard, context menu, dialog presentation, table, terminal selection, QR/connection-code, and rich color capability.
- Keep the capability set as renderer capability metadata and validation input, not a rendering engine or UI policy layer.
- Tighten binding grammar where needed for reusable renderer conformance: `ui.bind`, `bind_list`, `bind_if`, item-relative paths, exact top-level `where` filters, and empty-template behavior.
- Add explicit contract tests in `crates/botster-core/tests/ui_contract_test.rs` for capability wire shape, downgrade validation, binding grammar, controlled versus renderer-local presentation-state expectations, responsive fallbacks, and action metadata.
- Add reusable UI renderer conformance fixtures/helpers to an ungated `botster-core-test-support` surface so downstream TUI/web renderers can import or copy stable fixtures and assertions with `default-features = false`.
- Add downstream-style tests proving the conformance helpers exercise public `botster_core` imports and fixture data.
- Update public re-exports so consumers can import the new UI capability and fixture-related contract types through `botster_core` and `botster_core::ui`.

## Non-Scope
- No concrete TUI, browser, React, Catalyst, ratatui, Restty, Ghostty, Lua plugin, Rails relay, MCP, or Project Pipelines UI implementation.
- No new renderer registry, runtime renderer adapter, browser hydration logic, or terminal backend logic.
- No new dependencies unless a compile-time test-only dependency is already unavoidable; prefer existing `serde` and `serde_json`.
- No broad primitive inventory expansion beyond fields needed to express capability downgrade and conformance fixtures.
- No product workflow policy in `botster-core`; Project Pipelines-specific UI policy stays plugin-owned.

## Assumptions And Unknowns
- Assumption: this ticket is scaffold/core-contract work. The production path to prove is public API consumption by downstream renderers, not an end-user-rendered screen in this checkout.
- Assumption: `UiCapabilitySet` should be additive and optional for consumers; existing valid `UiNode` trees should not require a capability set to validate.
- Assumption: capability downgrade behavior means a reusable helper can assert that a renderer either supports a fixture directly or declares deterministic fallback handling, without core choosing the visual fallback.
- Assumption: controlled state remains owner-authored when `value`, `checked`, or `selected` is present; renderer-local state is allowed only where stable node ids let renderers preserve it.
- Unknown: exact names for table, QR, terminal-selection, and rich-color capability fields. Implementer should pick concise renderer-neutral names and pin their JSON wire shape in tests.
- Unknown: whether the helper should be called a "renderer harness" or "renderer conformance fixture" in public API. Prefer the latter to avoid implying a live renderer runtime.
- No blocking human question: the ticket explicitly forbids a concrete renderer and accepts reusable fixtures/harness mechanics.

## Affected Surfaces And Files
- `crates/botster-core/src/contract/ui.rs`: new `UiCapabilitySet` and supporting enums/structs; capability-aware validation helpers if needed; binding grammar clarifications.
- `crates/botster-core/src/contract/mod.rs`: public re-exports for new UI capability contract types.
- `crates/botster-core/tests/ui_contract_test.rs`: contract tests for capability serialization, downgrade behavior, bindings, responsive fallbacks, action metadata, and controlled/local state.
- `crates/botster-core-test-support/src/ui_conformance.rs` or equivalent ungated support-crate module: reusable UI renderer conformance fixtures/assertions. Do not place these helpers under the existing local-runtime-gated `conformance` module.
- `crates/botster-core-test-support/src/lib.rs`: expose the UI conformance support without `#[cfg(feature = "local-runtime")]`; keep the existing PTY/runtime conformance module gated.
- `crates/botster-core-test-support/tests/downstream_conformance_test.rs`: downstream-style proof that consumers can use the fixtures/assertions.
- Optional: a focused `crates/botster-core-test-support/tests/ui_renderer_conformance_test.rs` if the existing downstream test would become too broad.

## Risks
- Overfitting capability names to web or TUI behavior would violate the renderer-neutral core boundary.
- Encoding fallback policy in core would create a hidden concrete renderer; core should validate and fixture-test declarations only.
- Making capability checks mandatory on existing `UiNode::validate()` would be breaking churn outside the ticket's intent.
- Putting pure UI conformance helpers under `botster-core-test-support::conformance` would hide them from `default-features = false` consumers because that module is currently gated behind `local-runtime` and pulls PTY support.
- Binding grammar can drift from existing browser/TUI raw wire handling if typed helpers are narrower than accepted JSON; fixtures should pin both typed and JSON paths.
- Adding too many primitives or presentation fields would conflict with the narrow v1 form and semantic primitive conventions.
- Checklist persistence initially timed out during planning, then the run checklist became readable and was updated. Keep checklist evidence and gate/artifact evidence aligned.

## Acceptance Checks And Tests
- `cargo test -p botster-core ui_contract`
- `cargo test -p botster-core-test-support downstream_conformance`
- `cargo test -p botster-core-test-support --no-default-features`
- Add and run a focused support-crate test if split out: `cargo test -p botster-core-test-support ui_renderer_conformance`
- `cargo clippy -p botster-core -p botster-core-test-support --all-targets --all-features -- -D warnings`
- Review public rustdoc/import path manually or by compile tests: new contract types import from both `botster_core::ui::{...}` and `botster_core::{...}`; UI conformance helpers import from `botster_core_test_support::{...}` with `default-features = false`.
- Verification evidence should identify the production path as public downstream consumption of `botster_core::ui` plus reusable `botster_core_test_support` conformance helpers. No runtime renderer path is expected in this ticket.

## Pipeline Gates And Artifacts
- Plan gate evidence should attach this document plus the loaded vault/repo context, assumptions, affected surfaces, risks, and acceptance checks.
- Plan Review should check that the plan stays additive, renderer-neutral, and fixture-oriented.
- Implement should update this plan only if implementation meaningfully diverges from the accepted scope.

## Vault Gaps Worth Capturing
- Capture if implementation reveals a durable naming convention for `UiCapabilitySet` fields across TUI and web renderers.
- Capture if capability downgrade semantics need a sharper distinction between "unsupported", "renderer-local fallback", and "owner-authored fallback".
- Capture if binding fixture grammar exposes drift between typed Rust structs and the raw UI wire grammar downstream renderers already accept.
