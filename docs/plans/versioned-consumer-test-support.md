# Versioned Consumer Test Support Plan

Ticket: `ticket_1780075248_215152`
Run: `run_1780077466_843858`

## Context Loaded

- Pipeline context: ticket `ticket_1780075248_215152`, run `run_1780077466_843858`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts/findings/questions, dependency `ticket_1780014900_202484` closed.
- Orchestrator correction received by Botster inbox: this run is main-rooted. Ignore dependency-populated `base_run_id`/`base_ticket_id` for PR stacking; target `main`. Current repo layout is the workspace on `main` at `e29bea1`.
- Required playbooks: `planner-playbook`, `botster-planner-playbook`.
- Required/supporting vault notes: `plan steps need reviewable plan artifacts`, `botster-architecture`, `cli-patterns`, `spa-patterns`, `project pipeline orchestration belongs in a device-level botster plugin`, `project pipelines needs an operator workbench not more primitives`, `project pipelines ui contract belongs in the plugin readme`, `botster orchestration should spawn agents with explicit target ids`, `botster orchestration prompts must bind agents to explicit worktrees`.
- Repo context: `Cargo.toml`, `README.md`, `crates/botster-core/Cargo.toml`, `crates/botster-core-test-support/Cargo.toml`, `crates/botster-core-test-support/src/lib.rs`, `crates/botster-core-test-support/src/fixtures/regression/regression_shapes.rs`, `crates/botster-core-test-support/src/assertions/mod.rs`, `crates/botster-core-test-support/src/fake/mod.rs`, and `crates/botster-core/tests/regression_shape_fixtures_test.rs`.
- Prior dependency context: `docs/plans/contract-test-fixtures-regression-shapes.md` established preserve/translate/drop regression fixtures and a separate `botster-core-test-support` crate.

## Scope

- Finish the public dev/test-only support surface in `crates/botster-core-test-support` so downstream crates can import helpers pinned to their `botster-core` release.
- Keep the existing separate crate shape unless implementation discovers a hard architectural blocker. It is already a workspace member and `botster-core` already depends on it only as a dev-dependency.
- Add small, public conformance assertions that exercise public `botster_core` types and fixture output, not private implementation details.
- Add small fake transport/session helpers only where they make consumer tests realistic without pulling in hub policy or runtime actors.
- Add at least one core test and one downstream-style example/integration test that use the same public helper from `botster-core-test-support`.
- Document that `botster-core-test-support` is version-coupled to the matching `botster-core` release and belongs in downstream `dev-dependencies`.
- Keep all fixtures/builders/assertions generic to core contracts: identifiers, transport ingress/egress, session protocol frames, entity frames, actor/client stream data, and plugin worker contract data.

## Non-Scope

- No production dependency from `botster-core` to `botster-core-test-support`.
- No hub policy, CLI startup, renderer assumptions, auth, provider marketplace behavior, Project Pipelines product behavior, or product-specific workflows in the support crate.
- No copied old `trybotster` runtime tests or reliance on reference paths existing in this repo.
- No broad refactor of current core contract types unless an existing public type cannot support a required assertion.
- No optional configuration layer, feature matrix, or compatibility shims beyond normal Cargo dev-dependency use.
- No PII or environment-specific path content in source/docs.

## Assumptions And Unknowns

- Assumption: "versioned" means the support crate is published/released with the same version as `botster-core`, not that it implements runtime negotiation across versions.
- Assumption: Cargo dev-dependency isolation satisfies "helpers are not required by production builds"; implementation should prove this through dependency direction and tests rather than adding feature flags.
- Assumption: fake transports/sessions should be minimal in-memory helpers over public contract frames, not session-worker or hub simulators.
- Assumption: conformance assertions can be ordinary Rust functions that panic/assert, because downstream tests can call them directly.
- Unknown: whether the workspace should keep both crates at `0.1.0` manually or centralize workspace package version metadata later. This ticket should document coupling without introducing release tooling.
- Unknown: the exact fake helper names; implementer should choose names that match existing module layout and avoid speculative abstraction.

## Affected Surfaces And Files

Expected:

- `crates/botster-core-test-support/src/assertions/mod.rs`
- `crates/botster-core-test-support/src/fake/mod.rs`
- `crates/botster-core-test-support/src/lib.rs`
- `crates/botster-core-test-support/Cargo.toml`
- `crates/botster-core/tests/regression_shape_fixtures_test.rs` or a new core integration test using the shared helper
- a new downstream-style test/example, likely under `crates/botster-core-test-support/tests/`
- `README.md`

Possibly touched:

- `crates/botster-core-test-support/src/fixtures/regression/mod.rs`
- `crates/botster-core-test-support/src/fixtures/regression/regression_shapes.rs`
- `docs/plans/versioned-consumer-test-support.md` if Plan Review requests refinement

## Botster Layers Touched

- Rust core contract workspace.
- Docs.
- No Lua plugin, Rust hub, session/client worker runtime, TUI, React SPA, Rails relay, MCP, provider, auth, or Project Pipelines product layer should change.

## Worktree And Target Assumptions

- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1780075248_215152`.
- Spawn target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Branch/PR base: `main`, not a stacked dependency branch, per orchestrator correction.

## Risks

- Support helpers can accidentally become product policy. Mitigation: every public helper should return or assert public `botster_core` contract data only.
- Fakes can grow into a runtime simulator. Mitigation: keep fakes in-memory and frame-oriented; do not model hub lifecycle, worker scheduling, provider auth, or renderer behavior.
- The same-helper acceptance can be satisfied superficially. Mitigation: use one named public assertion/helper from both a `botster-core` test and a consumer-style test in the support crate.
- Version coupling can be under-documented. Mitigation: README and crate rustdoc should both state matching-release/dev-dependency use.
- Production builds might pull test support accidentally if dependency direction changes. Mitigation: verify `botster-core-test-support` appears only under `botster-core` dev-dependencies and workspace membership, not production dependencies.
- Clippy may reject assertion/fake code because `unwrap_used` is denied as warnings under `-D warnings`. Mitigation: avoid `unwrap` in library code; tests may use `expect` with clear messages where current repo style already does.

## Acceptance Checks And Tests

- `cargo fmt`
- `cargo test -p botster-core-test-support`
- `cargo test -p botster-core regression_shape`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Review checks:
  - `botster-core-test-support` is a dev/test-only support crate and is not required by production builds.
  - README/rustdoc explicitly says helpers are version-coupled to the matching `botster-core` release.
  - At least one `botster-core` test and one downstream-style test use the same public support helper.
  - Conformance assertions use public `botster_core` types, not private modules or old `trybotster` internals.
  - Fake transports/sessions stay generic and in-memory.
  - No source/docs include PII or local absolute paths other than this plan's pipeline worktree context.

## Pipeline Gates And Artifacts

- Plan artifact: this file.
- Plan gate: submit the above context, scope, assumptions/unknowns, affected files, risks, acceptance checks, and vault gaps.
- Plan Review should verify that the planned support surface remains dev-only and does not smuggle hub/plugin/product policy into core.

## Vault Gaps Worth Capturing

- Capture if implementation establishes a reusable pattern for extracted Rust crates exposing paired `*-test-support` crates.
- Capture if Cargo version-coupling conventions for Botster workspace crates need a durable release-process note.
- Capture if fake transport/session helpers reveal a stable core conformance pattern that should be reused by future downstream Botster crates.
