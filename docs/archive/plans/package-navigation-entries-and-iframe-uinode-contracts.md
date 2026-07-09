# Define Package Navigation Entries And Iframe UiNode Contracts

Ticket: `ticket_1783371357_714397`
Run: `run_1783371472_299407`
Step: Plan

## Context Loaded

- Pipeline context: ticket, run, current Plan step, gate prompt, events, checklist, open questions, prior answers, findings, artifacts, reviews, and dependencies. There are no prior artifacts, findings, reviews, questions, or answers for this run.
- Required playbooks: [[planner-playbook]] and [[botster-planner-playbook]].
- Required Botster overlay notes: [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], and [[botster orchestration prompts must bind agents to explicit worktrees]].
- Relevant existing contract direction loaded from vault maps: core owns reusable package manifests, `UiNode` shapes, and policy-free contracts; hub owns package admission, registry normalization, trust and placement policy; clients render validated contracts; plugin UI surfaces emit hub/core-valid `UiNode` trees and plugin-owned dynamic state through entity frames.
- Repo context inspected:
  - `crates/botster-core/src/package/manifest.rs`: `PackageManifest` already carries optional `surfaces`.
  - `crates/botster-core/src/package/surface.rs`: `PackageSurfaceDescriptor` has stable ids, `kind`, title, description, icon, legacy `order`, category, and supports. `kind: "app"` already exists and must remain the surface/route kind.
  - `crates/botster-core/tests/package_surface_contract_test.rs`: serde compatibility, surface round-trip, kind inventory, and example manifest tests exist.
  - `docs/examples/package-surfaces.json` and `README.md`: current docs describe surface descriptors and still present `order/category` as hints.
  - `crates/botster-core/src/contract/ui.rs`: `UiNodeKind`, raw `props`, recursive validation, capability validation, and schema tables already define the renderer-neutral UI contract.
  - `crates/botster-core/tests/ui_contract_test.rs`: validation tests already pin unknown prop rejection, primitive inventory, required props/slots, capability downgrades, and public wire shapes.
  - `crates/botster-core/src/package/mod.rs`, `crates/botster-core/src/contract/mod.rs`, and `crates/botster-core/src/lib.rs`: public re-export surfaces.

## Scope

Implement portable contract additions in `botster-core` only.

In scope:

- Extend package manifest contracts with optional navigation descriptors that let plugins declare navigation intent.
- Keep navigation entries stable and portable: stable id, label, optional icon token, and a target that points at a package surface/route, preferably an existing `kind: "app"` surface.
- Preserve `PackageSurfaceKind::App` as the route/surface kind. Navigation is metadata about where a host may present a link, not a replacement for surface kind.
- Ensure new navigation contract does not include authoritative plugin order, priority, pinning, hiding, shell placement, layout, sidebar, or local-navigation replacement fields.
- If `PackageSurfaceDescriptor.order` remains for serde compatibility, document it as legacy/non-authoritative and make new navigation tests prove navigation does not depend on it.
- Add a renderer-neutral `Iframe` or `Webview` `UiNodeKind` for sandboxed custom HTML surfaces such as vault graph output.
- Validate iframe/webview props for required `src` and `title`, sandbox policy, and explicit bridge/action allowance metadata where appropriate.
- Reject inline/raw HTML properties on iframe nodes and continue rejecting unknown props generally.
- Add serialization/validation tests for safe iframe nodes, missing `src`, missing `title`, and raw/inline HTML attempts.
- Update docs and examples to state host/client responsibilities for navigation and the iframe/webview security boundary.

Non-scope:

- No hub admitted navigation registry, user ordering preferences, pinning, hiding, client shell placement, or package policy implementation.
- No concrete React, TUI, webview, browser iframe renderer, Lua plugin, hub route registry, Rails relay, MCP, or Project Pipelines runtime changes.
- No route layout, padding, local navigation, dashboard ownership, sidebar replacement, or page shell primitives.
- No raw HTML injection into the parent Botster UI.
- No broad rewrite of package surfaces, UiNode validation, or public API shape beyond the narrow additive contracts.
- No new dependencies unless `serde`/`serde_json` cannot express the contract.

## Botster Layers Touched

- Rust core contract crate: package manifest contracts, UI node contract, public exports, tests, docs, and examples.
- Docs: README plus example manifest and this plan artifact.
- No plugin, Lua core, Rust hub policy/runtime, session/client worker, TUI, React SPA, Rails relay, MCP, or Project Pipelines runtime code.

## Proposed Contract Shape

Package navigation:

- Add a focused `package::navigation` module or extend `package::surface` if the types are only meaningful beside surfaces. Prefer a dedicated module if it keeps `surface` from mixing route/surface shape with host navigation intent.
- Add `PackageNavigationEntry` with:
  - `id: String`
  - `label: String`
  - `icon: Option<String>`
  - `target: PackageNavigationTarget`
  - optional non-authoritative descriptive metadata only if needed, such as `description`.
- Add `PackageNavigationTarget` with a narrow surface target, for example `{ kind: "surface", surface_id: "workbench" }`. If route target support is needed, make it explicit and stable rather than a raw browser URL.
- Add `navigation: Vec<PackageNavigationEntry>` to `PackageManifest` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- Add validation helper only if existing package-surface tests already use explicit validation functions for similar contract pieces. Otherwise, keep tests at serde/shape level and validate obvious invariants through test helper assertions.
- Do not add `order`, `priority`, `pinned`, `hidden`, `placement`, `sidebar`, `layout`, or `local_navigation` to the new navigation type.

Iframe/webview UI node:

- Add `UiNodeKind::Iframe` unless implementation finds repo vocabulary already favors `Webview`. If choosing `Webview`, tests and docs must explain that it is still a sandboxed iframe/webview primitive and not raw HTML injection.
- Allowed props should be narrow:
  - `src`: required string or bind, host-readable URL/path descriptor.
  - `title`: required nonblank string or bind for accessibility.
  - `sandbox`: optional array or typed policy value. Prefer an allowlist vocabulary over a raw browser sandbox string if the existing raw `props` validation can still keep tests simple.
  - `allow`: optional explicit capability metadata for iframe permissions, not implicit all-access.
  - `bridge`: optional explicit bridge/action allowance metadata if plugins need host-mediated messages. Keep this declarative and host-owned.
- Forbidden by omission and tests: `html`, `raw_html`, `inner_html`, `srcdoc`, `dangerouslySetInnerHTML`, `className`, `style`, `layout`, `padding`, `sidebar`, and local-navigation props.
- Add validation for nonblank `src` and `title`. Existing required prop checks only check key presence, so implement title/src value validation explicitly if blank strings would otherwise pass.
- Consider adding an iframe capability field/fallback only if current `UiCapabilitySet` pattern requires it. Do not make capability negotiation broader than the ticket.

## Assumptions And Unknowns

Assumptions:

- This is intentionally scaffold/contract-only in `botster-core`. The production path changed by this ticket is public manifest/UI contract serialization and validation consumed by hub/client/plugin code later.
- `PackageSurfaceDescriptor.order` remains for compatibility but should be documented as legacy/non-authoritative. Removing it would be a broader breaking migration than the ticket asks for.
- Navigation targets should reference package surface ids rather than own raw URLs, because `kind: "app"` remains the route/surface kind and hub/client policy owns routing.
- Iframe/webview `src` should name a sandboxed resource URL/path that a host has admitted or generated; core validates shape, not network policy, origin policy, or runtime access.
- Bridge/action permissions are metadata only. Hub/client implementations decide whether to admit and wire them.

Unknowns for implementation:

- Exact type names: `PackageNavigationEntry` vs `PackageNavigationItem`, and `Iframe` vs `Webview`. Prefer names that match existing repo vocabulary and pin wire names in tests.
- Whether navigation should live as `manifest.navigation` or nested under surfaces. The ticket says package manifest contracts with optional navigation entries/items, so prefer top-level `navigation` unless code review finds an existing manifest pattern that strongly favors nesting.
- Whether `sandbox` should be a typed enum list or a string array. Prefer typed enum list if it stays compact; otherwise validate a string array with documented allowed values.
- Whether bridge/action allowance belongs as `bridge` metadata on the iframe node or as separate `allow` entries. Keep it explicit either way.

No human question is blocking this plan. The ticket's architecture direction resolves the main ambiguity: plugin navigation is intent only, while host/client policy owns admitted registry and presentation preferences.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/package/manifest.rs`: add optional `navigation` to `PackageManifest`.
- `crates/botster-core/src/package/navigation.rs` or `crates/botster-core/src/package/surface.rs`: new navigation entry/target types and rustdoc.
- `crates/botster-core/src/package/mod.rs`: export navigation types.
- `crates/botster-core/src/contract/ui.rs`: add iframe/webview node kind, schema, prop validation, and optional capability metadata.
- `crates/botster-core/src/contract/mod.rs` and `crates/botster-core/src/lib.rs`: export any new public UI/navigation types if needed.
- `crates/botster-core/tests/package_surface_contract_test.rs` or a new `package_navigation_contract_test.rs`: navigation serde, compatibility, and no-ordering-authority tests.
- `crates/botster-core/tests/ui_contract_test.rs`: iframe/webview serialization and validation tests.
- `README.md`: update package surfaces/navigation and UI contract documentation.
- `docs/examples/package-surfaces.json` or a new `docs/examples/package-navigation.json`: example manifest with navigation entries.
- `docs/archive/plans/package-navigation-entries-and-iframe-uinode-contracts.md`: this plan artifact.

Not expected:

- `Cargo.toml`, runtime, engine, daemon, identity, terminal, entity, or capability runtime modules.
- Browser/TUI renderer code, Lua plugin packages, hub package registry code, route registry implementation, or Project Pipelines plugin code.

## Risks

- Adding plugin-defined `order`, `priority`, placement, pinning, or hiding to the new navigation type would contradict the ticket's authority split.
- Treating navigation as a new surface kind would break the explicit `kind: "app"` direction.
- Leaving docs that present `order` as host-owned ordering authority could confuse downstream implementers even if the new type is correct.
- Accepting `srcdoc`, `html`, or raw inline HTML props would violate the sandbox boundary.
- Using raw browser iframe options without contract validation could make TUI or non-browser clients impossible to reason about.
- A doc-only implementation would not prove the production path. Tests must deserialize/serialize manifests and validate real `UiNode` instances through public contract APIs.
- Over-validating URL or sandbox policy in core could move host/client policy into a policy-free crate.

## Acceptance Checks / Tests

Required targeted tests:

- Package manifest serde:
  - Manifest without `navigation` still deserializes and serializes without the field.
  - Navigation entries round-trip through `PackageManifest`.
  - Navigation target references a stable surface id while the target surface remains `kind: "app"`.
  - Navigation entries do not carry `order`, `priority`, `pinned`, `hidden`, `placement`, or shell/layout authority.
  - Existing surface `order` remains serde-compatible but docs/tests mark it legacy/non-authoritative.
  - Example manifest with navigation deserializes.
- UiNode validation:
  - `iframe`/`webview` node serializes/deserializes with safe `src`, `title`, sandbox, and bridge/action allowance metadata.
  - Missing `src` fails validation.
  - Missing or blank `title` fails validation.
  - Inline/raw HTML props such as `html`, `raw_html`, `srcdoc`, or `dangerouslySetInnerHTML` fail as unknown/invalid props.
  - Route layout/sidebar/padding props fail as unknown props on iframe/webview nodes.
  - Primitive inventory test includes the new node kind.
- Docs:
  - README states plugin declares navigation intent; hub admits/normalizes; clients render; hub/user/client preferences own ordering/pinning/hiding.
  - README states generated HTML such as a vault graph must render through iframe/webview, not raw parent UI injection.
  - README states core does not define route layout, padding, local navigation, sidebar replacement, or shell placement primitives.

Suggested commands:

- `cargo fmt --all -- --check`
- `cargo test -p botster-core --test package_surface_contract_test`
- `cargo test -p botster-core --test ui_contract_test`
- `cargo test -p botster-core`
- If public exports or rustdoc change materially: `RUSTDOCFLAGS="-D warnings" cargo doc -p botster-core --no-deps`
- If touched code broadens beyond the expected files: `cargo clippy -p botster-core --all-targets -- -D warnings`

Runtime/user-path evidence:

- This ticket is intentionally core-contract scaffold. The production entry point to prove is public package manifest and `UiNode` consumption: downstream hosts deserialize `PackageManifest`, inspect optional navigation entries, and validate `UiNode` trees before clients render them.
- Implementer evidence should therefore cite public integration tests using `botster_core::{PackageManifest, UiNode, UiNodeKind}` or `botster_core::package`/`botster_core::ui` exports, not private helper-only tests.

## Pipeline Gates And Artifacts

- Plan gate artifact: this file plus gate evidence summarizing context, scope, assumptions, affected surfaces, risks, acceptance checks, and vault gaps.
- Plan Review should specifically check that navigation has no plugin ordering authority, `kind: "app"` remains the route/surface kind, iframe/webview cannot carry raw HTML, and no route layout/sidebar primitives were introduced.
- Implement should update this plan or ask a human question if implementation discovers a need for host policy, renderer implementation, route layout primitives, or plugin-defined ordering.

## Vault Gaps Worth Capturing

No vault capture is required before implementation. Existing notes cover the main durable architecture: core contract ownership, hub/client policy ownership, plugin surface validation, staged extensibility, and no raw UI injection.

Capture after implementation only if the final contract settles a durable rule not already explicit in the vault:

- The precise canonical name and wire shape for package navigation entries.
- The canonical iframe/webview sandbox and bridge metadata vocabulary.
- The legacy status of `PackageSurfaceDescriptor.order` as non-authoritative compatibility metadata.
