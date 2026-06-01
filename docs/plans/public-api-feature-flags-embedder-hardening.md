# Harden Public API And Feature Flags For Embedders

Ticket: `ticket_1780245510_834657`
Run: `run_1780245516_948273`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Harden botster-core public API and feature flags for embedders`, run `run_1780245516_948273`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts, reviews, findings, questions, or answers.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster overlay notes loaded:
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
- General vault context loaded:
  - `identity`
  - `goals`
- Checklist discipline:
  - `project_pipelines_checklist_instructions` loaded.
  - Run-level vault checklist creation was attempted twice with `project_pipelines_create_vault_checklist`; both calls reported plugin worker timeouts, but later `project_pipelines_list_checklists` showed both records were created.
  - Latest checklist `checklist_1780245612_491876` was updated with vault context, convention conflict, verification, and capture evidence.
- Repo context inspected:
  - `Cargo.toml`
  - `README.md`
  - `crates/botster-core/Cargo.toml`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/runtime/mod.rs`
  - `crates/botster-core/src/engine/botster.rs`
  - `crates/botster-core/tests/botster_engine_api_test.rs`
  - `crates/botster-core/tests/local_process_runtime_test.rs`
  - `crates/botster-core-dev/Cargo.toml`
  - `crates/botster-core-dev/src/lib.rs`
  - `crates/botster-core-test-support/Cargo.toml`
- Prior plan artifacts inspected:
  - `docs/plans/default-local-pty-process-runtime.md`
  - `docs/plans/ergonomic-embeddable-botster-engine-api.md`
  - `docs/plans/assemble-core-multiplexer-engine-api.md`

## Scope

Audit and harden the `botster-core` embedder-facing public surface now that the default local PTY-backed engine path exists.

In scope:

- Review crate-root exports in `crates/botster-core/src/lib.rs` and module exports in `contract`, `engine`, `runtime`, `identity`, and `package`.
- Classify public exports as intentionally stable embedder API, lower-level advanced API, or accidental/internal exposure.
- Reduce accidental exposure only when doing so does not require a compatibility branch or speculative API redesign.
- Add or adjust Cargo feature flags only where they create a real embedder boundary:
  - core contract/API surface without local PTY/process runtime dependencies
  - default local PTY/process runtime path when explicitly enabled or kept as default
  - test/dev support staying outside production dependency trees
- Keep `DefaultBotsterEngine`, `LocalProcessRuntime`, and related local process exports documented if they remain public.
- Update README and rustdoc comments so embedders can tell which entry point to use:
  - `BotsterEngine` with custom host adapters
  - `DefaultBotsterEngine` for explicit local PTY-backed sessions
  - lower-level contracts for advanced hosts
  - `botster-core-test-support` as dev-only downstream test support
- Add compile-checked public API proof where useful, preferably rustdoc examples or integration tests importing only `botster_core` crate-root exports.
- Verify production `botster-core` does not depend on `botster-core-test-support`.
- Preserve the current no-PII posture in docs, examples, and tests.

Non-scope:

- No hub policy, CLI startup, auth, Rails/cloud, WebRTC, marketplace/install/update, Project Pipelines product behavior, old monolith migration, or compatibility shims.
- No broad rewrite of engine, runtime, session worker, subscription, plugin worker, identity, package, UI, or entity contracts.
- No new runtime dependency unless feature gating requires moving an existing dependency behind a feature.
- No default command selection, shell discovery, target admission, config discovery, persistence, reconnect, retention, process-tree policy, or UI rendering behavior in core.
- No version-suffixed duplicate APIs or long-lived dual code paths.
- No moving `botster-core-test-support` into production `botster-core` dependencies.

Botster layers touched:

- Rust `botster-core` public API facade and module exports.
- Rust `botster-core` Cargo feature/dependency boundary.
- Rust `botster-core` docs, rustdoc, and tests.
- Possibly `botster-core-dev` only if feature changes require its dev smoke harness to enable the local runtime feature.
- No plugin, Lua core, Rust hub, TUI, React SPA, Rails relay, MCP, provider, marketplace, or workflow-product changes.

Worktree/target assumption: implementers work in this assigned botster-core ticket worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts: this file is the Plan artifact. Gate evidence should cite this file, the loaded vault notes, checklist `checklist_1780245612_491876`, the duplicate checklist creation timeout behavior, and the baseline Cargo probes.

## Assumptions And Unknowns

Assumptions:

- The ticket does not require making `botster-core` dependency-free. It requires separating local PTY/process runtime dependencies from core contracts where that reduces real embedder friction.
- The current production dependency tree is allowed to include `portable-pty` only if the default local runtime remains part of the default feature set. If a contract-only embedder should avoid that dependency, a feature gate is the smallest useful boundary.
- `botster-core-test-support` is correctly dev-only today: `crates/botster-core/Cargo.toml` lists it under `[dev-dependencies]`, and `cargo tree -p botster-core -e=no-dev --offline` does not include it.
- The public production entry points changed by this ticket are exported Rust APIs and docs. There is no product hub wiring in this repo, so runtime/user-path proof comes from integration tests, doctests, and the dev harness.
- Feature flags should be additive and simple. Prefer one `local-runtime` feature over a matrix of narrow flags unless the code proves a second boundary is necessary.
- `serde`, `serde_json`, crypto/identity, contract, package, and engine types remain core contracts unless the export audit proves an accidental leak.
- No human question is currently blocking. The ticket says feature flags should be adjusted "only where they reduce real embedder friction", so the implementer can decide based on the export/dependency audit and should document the decision.

Unknowns for implementation:

- Whether to keep local runtime enabled by default or make it opt-in. Prefer preserving default behavior if current tests/docs treat `DefaultBotsterEngine` as the normal path, while adding `default = ["local-runtime"]` and allowing `--no-default-features` contract-only builds if feasible.
- Whether `DefaultBotsterEngine` can be hidden behind the same `local-runtime` feature without disrupting docs and tests. If hidden, rustdoc examples and public exports need matching `#[cfg(feature = "local-runtime")]` treatment.
- Whether `BotsterEngine` currently depends on `LocalProcessRuntime` only because `DefaultBotsterEngine` lives in the same module. If so, the smallest change may be module-level conditional imports and exports rather than splitting files.
- Whether doctests compile against `botster_core_test_support` from `botster-core` rustdoc. If docs need dev-only fakes, ensure the test command covers doctests or move examples to no-run snippets that still compile in the intended context.
- Whether crate-root re-export volume is acceptable as intentional ergonomic API or should be trimmed. The audit should prefer documenting intentional broad exports over churn that makes embedders import deep modules unnecessarily.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/Cargo.toml`
  - Add a small `[features]` section if the audit supports it.
  - Move `portable-pty` behind `local-runtime` if contract-only builds should avoid PTY/process dependencies.
  - Keep `botster-core-test-support` under `[dev-dependencies]` only.
- `crates/botster-core/src/lib.rs`
  - Reorganize or annotate crate-root exports so intentional public API is clear.
  - Gate local runtime exports if a feature is added.
- `crates/botster-core/src/runtime/mod.rs`
  - Gate `local_process` module and local runtime public exports if a feature is added.
  - Keep `SessionRuntime` and spawn/input/output contracts available for contract-only builds.
- `crates/botster-core/src/engine/botster.rs`
  - Gate `DefaultBotsterEngine` if it depends on local runtime.
  - Keep custom-adapter `BotsterEngine` available without local runtime if feasible.
- `crates/botster-core/src/engine/mod.rs`
  - Gate or document default-engine exports consistently with `botster.rs`.
- `README.md`
  - Document feature flags, embedder entry points, and production/dev dependency boundaries.
- `crates/botster-core/tests/*`
  - Update public API/export tests or add one focused test for crate-root importability.
  - Gate local runtime/default engine tests when feature flags require it.
- `crates/botster-core-dev/Cargo.toml`
  - Enable the local runtime feature only if `botster-core-dev` needs it after feature changes.
- `docs/plans/public-api-feature-flags-embedder-hardening.md`
  - This plan artifact.

Possible but avoid unless required:

- `crates/botster-core-test-support/*`
  - Only if test helper imports need feature-aware compilation. Do not make production core depend on it.
- Module-level rustdoc in `contract`, `engine`, `runtime`, `identity`, or `package`.
  - Add only where missing docs obscure embedder intent.

Not expected:

- Any hub, CLI product, browser, TUI, Rails, Lua plugin, MCP, Project Pipelines, provider, marketplace, or old TryBotster files.
- New third-party dependencies.
- Source-level runtime behavior changes outside conditional compilation needed for feature boundaries.

## Implementation Shape

Suggested sequence:

1. Audit public exports:
   - Read crate-root `pub use` inventory and module `pub use` lists.
   - Mark each export as contract, engine facade, default local runtime, identity/crypto, package, UI/entity, or test-only accidental.
   - Prefer adding short rustdoc/module docs for intentional broad exports over removing stable-looking names.

2. Add the smallest feature boundary if the audit justifies it:
   - `default = ["local-runtime"]`
   - `local-runtime = ["dep:portable-pty"]`
   - `portable-pty = { version = "0.9.0", optional = true }`
   - Gate `local_process`, `LocalProcessRuntime`, `LocalProcessRuntimeOptions`, `LocalProcessWorkerRuntime`, and `DefaultBotsterEngine` behind `#[cfg(feature = "local-runtime")]`.
   - Keep `BotsterEngine`, contracts, identity, package, and custom runtime traits available in `--no-default-features` builds.

3. Update docs:
   - README feature section with examples:
     - default local runtime enabled for embedders who want the policy-free PTY path
     - `default-features = false` for contract/facade-only embedders
     - `botster-core-test-support` under dev-dependencies only
   - Rustdoc for crate root or modules stating public API groups and host responsibilities.

4. Add tests/proofs:
   - Contract-only build: `cargo test -p botster-core --no-default-features --lib` and at least one integration/doc test that does not require local runtime if practical.
   - Default/all features build: existing local runtime and default engine tests stay green.
   - Production dependency proof: `cargo tree -p botster-core -e=no-dev --offline` must not include `botster-core-test-support`; if feature gating is added, also inspect `--no-default-features`.
   - Clippy with all targets/all features.

## Risks

- Gating too much can make the ergonomic API disappear for contract-only embedders. Keep `BotsterEngine` custom-adapter path available unless the compiler proves it is coupled to local runtime.
- Gating too little can leave `portable-pty` and its process dependencies in the contract-only path, which is the main embedder friction this ticket is likely targeting.
- Trimming crate-root exports aggressively can create avoidable churn for embedders. Document intentional exports unless an export is clearly accidental or test-only.
- Public docs can overclaim product behavior. Keep README language explicit that core does not own hub policy, CLI startup, auth, Rails/cloud, WebRTC, marketplace, or workflow behavior.
- Feature-gated rustdoc examples can silently rot if not covered by doctests or integration tests.
- `botster-core-dev` and integration tests may need explicit feature selection after moving local runtime behind a feature.
- `cargo clippy --all-targets --all-features -- -D warnings` can surface pre-existing warnings. Implementer must attribute failures to touched vs. untouched code instead of waiving broadly.
- The checklist MCP creation timeout created duplicate records despite returning errors. Use the latest updated checklist as evidence and avoid mutating the duplicate unless the pipeline owner requests cleanup.

## Acceptance Checks / Tests

Baseline evidence already gathered during planning:

- `cargo test -p botster-core --no-default-features --lib` passed. Current repo has no `[features]` section, so this proves the current library path only.
- `cargo test -p botster-core --all-features --lib` passed. Current repo has no `[features]` section, so this is equivalent to the current default library path.
- `cargo tree -p botster-core -e=no-dev --offline` passed and showed no `botster-core-test-support` production dependency.
- Initial `cargo tree -p botster-core --no-dev-dependencies` without `--offline` failed because the sandbox could not resolve `index.crates.io`; the offline rerun supplied the usable evidence.

Required implementation verification:

- `cargo fmt`
- `cargo test -p botster-core --no-default-features --lib`
- `cargo test -p botster-core --no-default-features --tests` or a narrower no-default integration/doc test if some existing integration tests intentionally require local runtime.
- `cargo test -p botster-core --all-features`
- `cargo test -p botster-core-dev --all-features` if `botster-core-dev` feature selection changes.
- `cargo test`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo tree -p botster-core -e=no-dev --offline`
- If feature gating is added: `cargo tree -p botster-core --no-default-features -e=no-dev --offline` and confirm `portable-pty` is absent from the contract-only tree.
- If docs/rustdoc examples are changed: `cargo test -p botster-core --doc --all-features`; add `--no-default-features` if examples are intended to compile in contract-only mode.

Functional acceptance:

- Public exports are reviewed and either intentionally documented or narrowed.
- README names the embedder entry points and feature flags accurately.
- Contract-only embedder build is possible if local runtime feature gating is implemented.
- Default local PTY-backed runtime path still compiles and is tested when its feature is enabled.
- Production `botster-core` does not require `botster-core-test-support`.
- No PII, private paths, product-specific policy, or old monolith migration behavior appears in docs or fixtures.

## Vault Gaps Worth Capturing

- Consider a durable note after implementation if the feature shape lands: `botster-core local process runtime should be feature-gated from contract-only embeds`.
- Consider a note if the export audit establishes a durable rule: `botster-core crate root exports are the stable embedder facade`.
- No convention conflict found. The plan follows existing Botster notes: core owns reusable mechanisms and contracts, hub/CLI/plugins own policy, and docs should prove public runtime paths rather than merely exposing code.
