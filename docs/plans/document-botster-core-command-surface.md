# Document Botster Core Command Surface

Ticket: `ticket_1780348078_522307`
Run: `run_1780355821_889618`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket
  `Document botster-core command surface for hub and embedders`, current step
  `botster_plan`, gate `botster_plan_gate`, target
  `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Closed dependency tickets loaded from pipeline context:
  - `Implement typed engine command API over public core engine`
  - `Add minimal botster-core dev command runner`
  - `Extend consumer conformance harness for command API`
- Required playbooks loaded:
  - [[planner-playbook]]
  - [[botster-planner-playbook]]
- Required Botster overlay notes loaded:
  - [[botster-architecture]]
  - [[cli-patterns]]
  - [[spa-patterns]]
  - [[project pipeline orchestration belongs in a device-level botster plugin]]
  - [[project pipelines needs an operator workbench not more primitives]]
  - [[project pipelines ui contract belongs in the plugin readme]]
  - [[botster orchestration should spawn agents with explicit target ids]]
  - [[botster orchestration prompts must bind agents to explicit worktrees]]
- Additional constraining notes loaded:
  - [[plan steps need reviewable plan artifacts]]
  - [[identity]]
  - [[goals]]
- Repo context inspected:
  - `README.md`
  - `docs/architecture/engine-command-surface.md`
  - `docs/architecture/ghostty-shadow-terminal-adapter.md`
  - `docs/plans/typed-engine-command-api.md`
  - `docs/plans/botster-core-dev-engine-smoke-examples.md`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/engine/command.rs`
  - `crates/botster-core/src/engine/botster.rs`
  - `crates/botster-core-dev/src/lib.rs`
  - `crates/botster-core-dev/tests/engine_smoke_test.rs`
  - `crates/botster-core-test-support/src/conformance/mod.rs`
  - `crates/botster-terminal-ghostty/README.md`
  - `Cargo.toml`
  - `crates/botster-core/Cargo.toml`
  - `.github/workflows/ci.yml`
- Project Pipelines checklist discipline attempted. Creating the run vault
  checklist failed with a plugin SQLite `database is locked` error, so this
  plan and the gate evidence preserve the checklist facts as the fallback
  artifact surface.

## Scope

Update the repository documentation so the typed command API is the recommended
embedder surface for hub adapters, the product CLI, tests, and future
plugin/provider layers.

In scope:

- Make `README.md` consistent with the current implementation:
  - core is mechanisms and typed contracts, not product UX;
  - `EngineCommand` / `DefaultEngineCommand` plus `execute_command(...)` are
    the recommended high-level command entry points;
  - `BotsterEngine` is the custom-runtime facade and `DefaultBotsterEngine` is
    the default local PTY-backed facade behind `local-runtime`;
  - `crates/botster-core-dev` now proves a real default local engine path, not
    a fake-only smoke path.
- Update module/rustdoc where needed so `botster_core::engine_command` is
  discoverable through rustdoc and crate-root exports.
- Add concise command API examples that compile in doctests or are backed by
  existing integration/dev harness tests.
- Clarify the core vs hub vs CLI boundary:
  - core owns reusable command mechanisms and typed outcomes;
  - hub owns runtime policy, admission, routing, recovery, supervision, and
    product orchestration;
  - CLI owns operator-facing command parsing, startup, config/auth flows, and
    product UX.
- Document feature and platform caveats:
  - local PTY support is behind default feature `local-runtime`;
  - contract-only embedders can disable default features;
  - local PTY acceptance is Unix-gated;
  - `botster-terminal-ghostty` owns the optional Ghostty shadow-terminal path
    and requires initialized vendored source plus Zig when the
    `libghostty-vt` feature is enabled.
- Remove or correct stale wording, especially any wording that describes
  `botster-core-dev` as fake-only or implies restty is core terminal
  infrastructure.
- Keep docs scrubbed of local absolute paths, usernames, tokens, hostnames,
  private terminal output, or other PII.

Non-scope:

- No new command API behavior unless a docs example exposes a compile error in
  existing public exports.
- No hub, product CLI, Rails relay, WebRTC/signaling, TUI, React SPA, Lua
  plugin, provider, cloud, marketplace/update, auth, config, or Project
  Pipelines runtime changes.
- No new engine router, async executor, queue, persistence, reconnect policy,
  default-shell discovery, spawn-target admission, or supervision abstraction.
- No dependency or feature-flag changes unless rustdoc currently cannot compile
  the documented public surface.
- No broad cleanup of historical plan documents. Past plan artifacts may
  describe earlier states; this ticket should update current reference docs and
  examples.

Botster layers touched:

- Docs: primary layer.
- Rust `botster-core` rustdoc/module docs: likely.
- Rust `botster-core-dev` docs or README references: likely.
- Rust tests/doctests: verification only, with focused doc example changes if
  needed.
- No plugin, Lua core, Rust hub product, TUI, React SPA, Rails relay, MCP, or
  provider implementation changes.

Worktree/target assumptions:

- Work happens in the pipeline-assigned botster-core worktree for target
  `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- The implementation dependencies are already closed and present in this
  worktree.
- This repo's runtime proof is the exported Rust API plus the
  `botster-core-dev` default local engine harness, not the full TryBotster hub.

Pipeline gates/artifacts:

- Plan artifact: `docs/plans/document-botster-core-command-surface.md`.
- Plan gate should cite this file and the checklist fallback caused by the
  Project Pipelines SQLite lock.

## Assumptions And Unknowns

Assumptions:

- The ticket is a documentation pass over an already-implemented typed command
  API. The closed dependencies and current code show `EngineCommand`,
  `DefaultEngineCommand`, `EngineCommandOutcome`, `EngineCommandError`, and
  `execute_command(...)` already exist.
- The recommended embedder surface should be documented as typed commands over
  `BotsterEngine` / `DefaultBotsterEngine`, not as direct
  `MultiplexerEngine` assembly for ordinary hosts.
- `MultiplexerEngine` remains a lower-level primitive for advanced embedders,
  but docs should steer hub/product adapters to the command facade first.
- README is the primary human entry point; `docs/architecture/engine-command-surface.md`
  is the detailed command contract; rustdoc is the compile-checked API surface.
- Feature caveats should name `local-runtime` and `libghostty-vt` without
  turning README into a product install guide.
- Verification commands should be the repo's existing CI commands plus targeted
  typed-command and dev-harness checks.

Unknowns for implementation:

- Whether a README code example should be a doctest in rustdoc instead of a
  fenced README snippet. Prefer rustdoc for compile-checked examples and keep
  README examples short.
- Whether `cargo test --doc --workspace` currently covers all examples after
  feature gating. If a no-default-features example is added, run a targeted
  no-default rustdoc/lib check too.
- Whether stale restty wording exists only in historical plan docs. Do not churn
  old plan artifacts unless current reference docs repeat the stale wording.
- Whether `crates/botster-core-dev` needs only README wording updates or also
  module docs to advertise that it exercises `DefaultEngineCommand`.

No human question is blocking planning. The ticket has one plausible meaning:
document the current typed command surface and boundaries without changing
product runtime behavior.

## Affected Surfaces / Files

Expected:

- `README.md`
  - Correct `botster-core-dev` description from fake-only to real local embedder
    smoke harness.
  - Make the typed command API the recommended high-level path.
  - Keep core vs hub vs CLI distinction concise and explicit.
  - Include verification commands and feature/platform caveats.
- `docs/architecture/engine-command-surface.md`
  - Tighten or add examples around `EngineCommand` and `DefaultEngineCommand`.
  - Clarify that command requests are capabilities/mechanisms, not product UX.
  - Preserve the exclusions for hub/product CLI/config/auth/cloud/WebRTC/
    signaling/marketplace/update/client transport policy.
- `crates/botster-core/src/engine/command.rs`
  - Rustdoc example or wording updates if current module docs lack a concise
    command execution example.
- `crates/botster-core/src/lib.rs`
  - Top-level rustdoc pointer to `engine_command` if discoverability is weak.
- `crates/botster-core-dev/src/lib.rs`
  - Module doc correction if needed to describe the real default local engine
    smoke path.
- `docs/plans/document-botster-core-command-surface.md`
  - This plan artifact.

Possible but avoid unless needed:

- `crates/botster-core/tests/botster_engine_api_test.rs`
  - Only if a new example needs a focused assertion to keep it from drifting.
- `crates/botster-core-dev/tests/engine_smoke_test.rs`
  - Only if docs reference a behavior the existing test does not assert.
- `crates/botster-terminal-ghostty/README.md`
  - Only if the feature/platform caveat is missing or ambiguous.

Not expected:

- `Cargo.toml` dependency or feature changes.
- `botster-core-test-support` implementation changes.
- Hub, CLI product, TUI, React/browser, Rails, Lua plugin, MCP, provider, cloud,
  marketplace, auth, or Project Pipelines files.

## Implementation Shape

1. Update README discoverability.
   - Add a short "Recommended command surface" subsection under the embedder
     path, or revise the existing embedder paragraph.
   - Include a compact snippet that names `DefaultEngineCommand::SpawnSession`
     or direct rustdoc pointer without duplicating the full table.
   - Correct `botster-core-dev` wording to current real local engine behavior.
2. Tighten the architecture note.
   - Ensure `EngineCommand<W>`, `DefaultEngineCommand`,
     `EngineCommandOutcome`, `EngineCommandError`, and
     `ENGINE_COMMAND_KINDS` are named as the typed API.
   - Explicitly say hub/product CLI should translate their product actions into
     host-resolved typed commands rather than putting config/auth/discovery
     policy into core.
3. Add or adjust rustdoc examples.
   - Prefer a no-run or compile-only example for command construction if a real
     PTY would make doctests flaky.
   - Use deterministic ids and scrubbed values.
   - Gate local-runtime-only examples correctly.
4. Verification and stale wording scan.
   - Use `rg` to prove there is no current restty-as-core wording in reference
     docs.
   - Use `rg` to prove fake-only `botster-core-dev` wording is gone from
     current docs.
   - Run targeted cargo checks before broader workspace checks.

## Risks

- Docs can overclaim that core is a product CLI or hub. Mitigation: keep
  command requests explicit and policy-free, and name hub/CLI ownership.
- A README example can drift if it is not compile checked. Mitigation: put
  substantial examples in rustdoc or back them with existing tests.
- `DefaultEngineCommand` is feature-gated. Mitigation: gate examples and run
  doc tests with default features; run a no-default feature check if examples
  mention contract-only embedders.
- Local PTY examples are platform-sensitive. Mitigation: document Unix gating
  and rely on existing Unix-gated tests/dev harness.
- Ghostty docs can imply a default dependency. Mitigation: state that Ghostty
  is optional and owned by `botster-terminal-ghostty`, with
  `libghostty-vt`/Zig/submodule caveats.
- Historical plan docs may contain older wording. Mitigation: update current
  reference docs and avoid broad plan-history rewrites unless stale wording is
  directly user-facing.
- Project Pipelines checklist writes are currently blocked by SQLite locking.
  Mitigation: preserve checklist evidence in this plan and gate submission.
- Plan and docs can leak local paths. Mitigation: use neutral worktree language
  and run a PII scan over changed files.

## Acceptance Checks / Tests

Required checks:

- `cargo fmt --all -- --check`
- `cargo test -p botster-core botster_engine`
- `cargo test -p botster-core engine_command`
- `cargo test -p botster-core-dev`
- `cargo test --doc --workspace`
- `cargo doc --workspace --no-deps`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test -p botster-core --test local_process_runtime_test` on Unix
- If rustdoc examples are intended to compile without default features:
  `cargo test -p botster-core --no-default-features --lib`

Targeted acceptance assertions:

1. README includes a concise core vs hub vs CLI distinction.
2. README or architecture docs recommend the typed command API for embedders,
   hub adapters, tests, and future plugin/provider layers.
3. Docs include command API examples or rustdoc examples using public exports.
4. Verification commands are present and match the repo's CI surface.
5. Local PTY caveats mention `local-runtime`, Unix gating, and explicit
   host-supplied spawn requests.
6. Ghostty caveats mention `botster-terminal-ghostty`, `libghostty-vt`, the
   vendored source initialization requirement, and Zig without moving Ghostty
   into core.
7. No current reference docs imply restty is core authoritative terminal
   infrastructure.
8. `botster-core-dev` docs no longer describe the current smoke harness as
   fake-only.
9. No changed docs/tests/examples contain local absolute paths, usernames,
   credentials, hostnames, or private terminal contents.
10. Runtime/user-path proof points to the existing tests and dev harness that
    execute `DefaultEngineCommand` through `DefaultBotsterEngine`; docs-only
    changes must not claim new production hub wiring.

Runtime/user path proof:

- This ticket intentionally changes docs and examples, not hub production
  wiring.
- The actual runtime path described by the docs is already exercised by:
  - `crates/botster-core/tests/botster_engine_api_test.rs` using
    `EngineCommand` and `DefaultEngineCommand`;
  - `crates/botster-core-dev/src/lib.rs` running a real local command through
    `DefaultBotsterEngine::execute_command`;
  - `crates/botster-core-test-support/src/conformance/mod.rs` proving the
    managed local runtime path for downstream consumers.
- Implementation evidence must cite those tests and run the relevant commands,
  not merely point at type definitions.

## Vault Checklist Evidence

- Vault/project notes constraining the plan: [[planner-playbook]],
  [[botster-planner-playbook]], [[botster-architecture]], [[cli-patterns]],
  [[spa-patterns]], [[project pipeline orchestration belongs in a device-level
  botster plugin]], [[project pipelines needs an operator workbench not more
  primitives]], [[project pipelines ui contract belongs in the plugin readme]],
  [[botster orchestration should spawn agents with explicit target ids]],
  [[botster orchestration prompts must bind agents to explicit worktrees]],
  [[plan steps need reviewable plan artifacts]], [[identity]], and [[goals]].
- Convention conflicts: none. The plan keeps policy outside core, treats
  typed commands as mechanisms, keeps product UX in hub/CLI/plugin/provider
  layers, and creates a repo-visible plan artifact.
- Verification evidence so far: planning inspection only. Planned verification
  commands are listed above.
- Checklist status: attempted run checklist creation failed with Project
  Pipelines SQLite `database is locked`; this plan is the fallback durable
  evidence surface.
- Durable knowledge capture: no new vault note is required before
  implementation. Capture after implementation only if the final docs establish
  a reusable convention for command-surface rustdoc examples or
  `botster-core-dev` as the standing real-embedder documentation proof path.

## Vault Gaps Worth Capturing

- Capture a Botster convention if this ticket settles that current public docs
  should steer hub/product adapters to typed command enums before lower-level
  `MultiplexerEngine` assembly.
- Capture a docs/testing convention if the final examples establish a preferred
  pattern for compile-checked command API snippets without running real PTYs in
  doctests.
- Capture a dev-harness convention if `botster-core-dev` becomes the durable
  proof path for README embedder examples.
