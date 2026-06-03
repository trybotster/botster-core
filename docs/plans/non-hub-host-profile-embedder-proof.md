# Non-Hub Host Profile Embedder Proof

Ticket: `ticket_1780447077_166056`

## Context Loaded

- Pipeline context loaded with `project_pipelines_get_ticket` and `project_pipelines_current_context`: ticket `ticket_1780447077_166056`, run `run_1780455774_850143`, current step `botster_plan`, run step `run_step_1780455774_971357`, gate `botster_plan_gate`, target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Pipeline state: no prior artifacts, reviews, findings, questions, or question answers for this run. Dependency `ticket_1780447077_921369` / "Add core plugin package lifecycle command surface" is closed and merged through PR #68. Branch starts at `origin/main` commit `41e0110`.
- Worktree: the assigned Project Pipelines ticket worktree on branch `project-pipelines/ticket_1780447077_166056`. Ticket implementation target is the `botster-core` repository; this run's assigned worktree is the active editing surface.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster/vault notes loaded:
  - `identity`
  - `goals`
  - `botster-architecture`
  - `cli-patterns`
  - `spa-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
  - `botster plugin runtime uses supervisor plus per plugin workers`
- Project Pipeline checklist attempt: `project_pipelines_create_vault_checklist` failed with a Project Pipelines SQLite database lock. Per `project pipelines checklist worker timeouts require artifact evidence fallback` / SQLite lock guidance, checklist evidence is preserved in this plan and should be repeated in gate evidence.
- Repo context loaded:
  - `crates/botster-core-dev/src/lib.rs`: existing dev-only real embedder harness already drives `DefaultBotsterEngine` and `DefaultEngineCommand` through real local session spawn, attach, output drain, input, resize, read screen, capture snapshot, activity classification, and shutdown.
  - `crates/botster-core-dev/tests/engine_smoke_test.rs`: existing smoke test calls the same shared harness path as the binary and asserts the real no-hub local runtime behavior.
  - `crates/botster-core/src/package/host_profile.rs` and `crates/botster-core/tests/host_profile_contract_test.rs`: current host-profile admission helper and contract tests for provider-only profile metadata, enablement, source, bootstrap, required capabilities, and compatibility.
  - `crates/botster-core/src/engine/command.rs` and `crates/botster-core/src/engine/botster.rs`: public policy-free `BotsterEngine` command facade, `DefaultBotsterEngine`, and plugin lifecycle command variants. `DefaultEngineCommand` intentionally omits plugin lifecycle; generic `BotsterEngine<R, W>` owns plugin load/invoke and can be evaluated with `LocalProcessRuntime`.
  - `crates/botster-core/tests/botster_engine_api_test.rs`: existing plugin load/reload/unload/invoke and capability rejection tests through `BotsterEngine`.
  - `crates/botster-core-test-support/src/fake/plugin_worker.rs`: reusable fake `PluginRuntime` for deterministic plugin invocation.
  - `docs/architecture/first-party-host-profile-primitives.md`: current architecture note documents host-profile admission as scaffold-only in core, `DefaultBotsterEngine` as policy-free local runtime, and host/hub ownership of registry/startup policy.
  - `docs/architecture/engine-command-surface.md`: command surface documents `DefaultEngineCommand` intentionally omits plugin lifecycle today; plugin lifecycle is available through generic `BotsterEngine`.
  - `README.md`: already frames core as reusable mechanisms and `botster-core-dev` as a dev-only real embedder harness.

## Scope

Implement the smallest no-hub embedder proof that composes the already-available pieces into one reviewable host-profile scenario.

In scope:

- Extend `crates/botster-core-dev` so `run_engine_smoke()` or a closely named sibling produces a single `EngineSmokeReport` proving:
  - a minimal non-hub trusted host profile package is constructed and admitted with `admit_host_profile`;
  - the admitted profile's `required_capabilities` are load-bearing: the exact admitted capability is used as the plugin handler requirement in the allow case, and the same handler requirement is withheld from a second ordinary plugin manifest to prove typed denial;
  - the host profile documents/proves the inputs a custom host must supply before entering core: host Botster version, enablement decision, source provenance, bootstrap entrypoint, required provider names, required capabilities, explicit spawn request fields, client/subscription ids, logical clocks, and plugin worker registration/runtime;
  - the implementation first evaluates a single generic `BotsterEngine<LocalProcessRuntime, W>` path for both real local session management and plugin load/invoke. Prefer that if feasible because it best satisfies the "one embeddable tmux-like engine without hub" ticket claim;
  - if the single generic local-runtime engine path is not wired today, the harness must still compose the existing no-hub local session path and plugin-worker path from one shared host proof, and both report/docs must explicitly state that current core exposes real local sessions through `DefaultBotsterEngine` while plugin lifecycle is exposed through generic `BotsterEngine` so the proof does not overclaim one unified engine surface;
  - an ordinary plugin is loaded and invoked through worker-mediated core plugin mechanics without hub involvement;
  - capability boundaries reject a plugin handler whose required capability is absent from the plugin manifest, and allow a handler whose required capability is present because that capability came from the admitted host profile.
- Prefer extending the existing `botster-core-dev` harness/test rather than adding a new crate or broad new abstraction.
- Add a targeted test in `crates/botster-core-dev/tests/engine_smoke_test.rs` or a new focused test file that calls the same shared dev-harness path used by the binary.
- Update README or the existing architecture note only as needed to document what a custom host profile must provide and to state that the proof is no-hub/dev-harness evidence, not hub package-manager wiring.

Botster layers touched:

- Rust core dev harness: primary changed layer.
- Rust `botster-core` public contracts: exercised; avoid changing unless a compile gap blocks the proof.
- Rust test-support fakes: may become a dev-dependency for `botster-core-dev` if using `FakePluginRuntime` avoids duplicating fake plugin code.
- Docs: README and/or `docs/architecture/first-party-host-profile-primitives.md` if the custom-host requirements need clearer durable prose.

## Non-Scope

- No `botster-hub`, CLI product daemon, TUI, React/browser, Rails relay, cloud/WebRTC provider, marketplace UX, package install registry, lockfile, persistence, auth, or legacy monolith migration.
- No real Lua plugin loading unless the current core public mechanics already expose it cheaply. The ticket says "load/invoke a simple ordinary plugin through core plugin mechanics if available"; `PluginRuntime` plus `PluginWorkerRegistration` is the available policy-free core mechanism.
- No global capability ledger or startup/config lifecycle primitive. Those are documented follow-ups, not this proof.
- No wiring host-profile admission into `BotsterEngine` or `PluginWorkerEngine`; that would collapse the current core-vs-host boundary. The host proof should call admission before loading runtime/plugin paths.
- No new product policy, optional configurability, command-line parser, install UX, or broad API refactor.
- No local user paths, secrets, tokens, or host-specific identifiers in reports, tests, or docs.

## Assumptions And Unknowns

Assumptions:

- The ticket's "non-hub host profile" means a dev/test host that is not `botster-hub`, not a new production host-profile registry.
- `botster-core-dev` depends on `botster-core` with default features, so `local-runtime` is enabled for the real session proof. The Unix smoke test must continue asserting `ran_real_embedder == true` so a feature-off or non-real proof cannot pass on the target platform.
- The existing `DefaultBotsterEngine` real local PTY proof is acceptable only as a fallback if a single generic `BotsterEngine<LocalProcessRuntime, W>` proof is not currently feasible. The implementer must evaluate the single-engine path first.
- Plugin execution should be framed as worker-mediated through `PluginWorkerRegistration` and `PluginRuntime`, matching the per-plugin worker boundary; no hub Lua/shared closure execution belongs in this proof.
- The admitted host profile's required capability is not just metadata: implementation must derive the plugin grant/deny capability from the admitted profile output and use it in plugin handler registration/assertions.
- Using `botster-core-test-support` as a dev-dependency of `botster-core-dev` is acceptable if it keeps fakes out of production core and avoids copy-paste.
- Documentation can be concise because `docs/architecture/first-party-host-profile-primitives.md` already explains scaffold boundaries and custom host responsibilities.

Unknowns for implementation:

- Whether the cleanest report shape is to extend `EngineSmokeReport` with host-profile/plugin fields or split into a nested `NonHubHostProfileSmokeReport`. Prefer one report if it stays readable; split only if it prevents a bulky struct.
- Whether `BotsterEngine<LocalProcessRuntime, W>` can currently provide the same real output drain/screen/snapshot evidence as `DefaultBotsterEngine`. If yes, use it for a unified proof. If no, the final report must name the two public engine surfaces and avoid claiming one engine did both.
- Whether `botster-core-dev` should use test-support fakes as a normal dependency because the binary also runs the shared harness. If that feels wrong, define a tiny local fake `PluginRuntime` in `botster-core-dev`; do not move test-only fakes into production core.
- Whether the current `BotsterEngine` generic runtime setup has convenient fake session runtime exports in test-support for the plugin-only path. The implementer should inspect before adding local fakes.

No human question is blocking this plan. The ticket intent is specific enough to proceed without waiving any requested acceptance item.

## Affected Surfaces / Files

Expected changes:

- `crates/botster-core-dev/src/lib.rs`
  - Extend the shared dev harness with host-profile admission evidence, load-bearing capability grant/deny evidence, ordinary worker-mediated plugin load/invoke evidence, single-vs-two engine-surface evidence, and custom-host requirements in the report.
- `crates/botster-core-dev/tests/engine_smoke_test.rs`
  - Assert the new report proves no-hub host profile admission, session runtime, plugin invocation, and capability boundary behavior.
- `crates/botster-core-dev/Cargo.toml`
  - Add `botster-core-test-support` only if needed for deterministic fake plugin/runtime helpers.
- `README.md` or `docs/architecture/first-party-host-profile-primitives.md`
  - Add concise custom-host requirement documentation only if the report/test does not make it clear enough.

Possible but not expected:

- `crates/botster-core-test-support/src/fake/*`
  - Only for a small reusable fake helper that is immediately consumed and clearly belongs to downstream conformance support.
- `crates/botster-core/src/*`
  - Avoid unless current public APIs cannot express the proof despite existing tests suggesting they can.

Not expected:

- `botster-hub`, Rails, TUI, React SPA, CLI product, Lua plugin template, provider package manager, or marketplace files.

## Risks

- Accidentally claiming `DefaultBotsterEngine` supports plugin lifecycle would be inaccurate; plugin lifecycle currently belongs to generic `BotsterEngine`, while `DefaultEngineCommand` omits plugin commands.
- Running the session proof and plugin proof in separate helper code could look like "code exists" rather than an embedder path. Mitigation: evaluate one `BotsterEngine<LocalProcessRuntime, W>` path first; otherwise one dev-harness report must narrate the two current public surfaces and explicitly avoid overclaiming unification.
- Wiring admission into core engines would exceed scope and violate the architecture note that host/hub owns registry/startup policy.
- A fake plugin invocation without a denied-capability case would fail the capability-boundary acceptance.
- A capability denial that merely duplicates `botster_engine_api_test.rs` without using the admitted host profile's capability would fail the ticket's composition intent.
- Extending the dev binary into a real CLI would violate the dev-only boundary.
- Adding local absolute paths or environment-derived identifiers would violate no-PII acceptance.
- Real PTY tests can be platform-sensitive. Existing harness already gates real runtime on Unix; keep that behavior and make non-Unix behavior explicit if plugin/admission proof can still run.
- `local-runtime` is feature-gated. The dev crate currently enables default features; tests must assert real embedder execution on Unix so a feature regression cannot silently degrade the proof.

## Acceptance Checks / Tests

Required checks after implementation:

- `cargo test -p botster-core-dev`
- `cargo run -p botster-core-dev`
- `BOTSTER_ENV=test cargo test -p botster-core host_profile`
- `BOTSTER_ENV=test cargo test -p botster-core botster_engine`
- `BOTSTER_ENV=test cargo test -p botster-core-test-support`
- `cargo clippy --all-targets --all-features -- -D warnings`
- For implement/verify gates, run cargo/clippy/test through raw passthrough when an RTK layer is active, and cite raw exit status plus test counts/diagnostics rather than summarized prose.

Targeted assertions:

1. The dev harness/test admits a provider manifest with non-hub profile id such as `minimal-test-host`, source provenance, bootstrap entrypoint, required provider, required capability, and compatible Botster requirements.
2. The exact capability returned from the admitted profile is used as the ordinary plugin handler's required capability.
3. With that capability present in the ordinary plugin manifest, the worker-mediated plugin handler runs and returns a completed `PluginInvocationOutcome`.
4. With that same capability withheld from a second ordinary plugin manifest, the plugin handler is not called and core returns a typed plugin invocation failure. This assertion must add host-profile composition value beyond the existing standalone capability-rejection test.
5. The implementer evaluates a single generic `BotsterEngine<LocalProcessRuntime, W>` real-session-plus-plugin proof first and records the result. If it is feasible, use it.
6. If the proof must use two public surfaces, the report/docs explicitly say so: real local session management is currently through `DefaultBotsterEngine`, while plugin lifecycle is through generic `BotsterEngine`; do not claim one unified engine did both.
7. The harness proves the real local no-hub session path still spawns and manages a session through the selected local-runtime engine surface.
8. The harness proves output/events flow through subscribed client egress, not just by checking that types compile.
9. On Unix/default-feature test platforms, the harness asserts `ran_real_embedder == true`.
10. The report or docs list what a custom host profile must provide before entering core.
11. The binary output and test fixtures contain no PII, `/Users/` paths, tokens, credentials, or host-specific identifiers.

Runtime path proof:

- The changed user/runtime path is `cargo run -p botster-core-dev` and the test that calls the same shared harness. The proof is intentionally dev-harness evidence, not production hub wiring.
- Evidence that contract types exist is not sufficient. The final gate should cite the concrete output/test assertions showing the no-hub host proof called admission, session commands, plugin load/invoke, and denied-capability behavior.
- The final gate should also state whether one generic local-runtime `BotsterEngine` performed both real session and plugin work, or whether the proof intentionally documents current separate engine surfaces.

## Vault Checklist Evidence

- Vault/project notes constrained the plan: listed in Context Loaded.
- Convention conflicts: none. The plan preserves Botster's core/hub boundary, keeps workflow policy out of core, avoids new product abstractions, and uses existing Rust public facades/fakes.
- Verification evidence planned: commands listed in Acceptance Checks, with targeted assertions for runtime path proof rather than compile-only evidence and raw cargo/clippy evidence where RTK is present.
- Durable knowledge captured: not yet. Capture a new vault note only if implementation discovers a reusable pattern not already covered by `botster-core-dev` harness docs or host-profile architecture notes.

## Vault Gaps Worth Capturing

No mandatory vault gap from planning alone.

Potential capture after implementation:

- If the final implementation establishes a repeatable pattern for combining `botster-core-dev` real-runtime smoke harnesses with generic `BotsterEngine` plugin proofs, capture a short note about "dev harness reports should prove the runtime path and adjacent public facades from one shared binary/test path."
