# Botster Core Dev Engine Smoke Examples

Ticket: `ticket_1780075967_818496`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: run `run_1780098990_349160`, current step `botster_plan`, gate `botster_plan_gate`, ticket `Add botster-core dev examples for engine smoke testing`.
- Orchestrator correction received through Botster inbox: this run is main-rooted. Dependency-derived `base_run_id` and `base_ticket_id` were cleared in Project Pipelines and must not make this a stacked PR. Target `main`.
- Current worktree: assigned Botster pipeline worktree for this run.
- Target id from pipeline context: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster overlay notes loaded:
  - `identity`
  - `goals`
  - `botster-architecture`
  - `cli-patterns`
  - `project pipeline orchestration belongs in a device-level botster plugin`
  - `project pipelines needs an operator workbench not more primitives`
  - `project pipelines ui contract belongs in the plugin readme`
  - `botster orchestration should spawn agents with explicit target ids`
  - `botster orchestration prompts must bind agents to explicit worktrees`
- Repo context loaded:
  - `README.md`: `botster-core` owns reusable mechanisms and transport-neutral contracts; `crates/botster-core-dev` is explicitly a dev-only smoke harness, not the product CLI.
  - `Cargo.toml`: workspace already includes `crates/botster-core-dev`, `crates/botster-core`, and `crates/botster-core-test-support`.
  - `crates/botster-core-dev/src/main.rs`: currently prints only `botster-core dev harness`.
  - `crates/botster-core-dev/Cargo.toml`: depends only on `botster-core` today.
  - `crates/botster-core/src/engine/subscription_multiplexer.rs`: exported transport-neutral multiplexer for client attach, terminal input, session output, backpressure, and notifications.
  - `crates/botster-core/src/engine/session_worker.rs`: exported session worker engine over a host-provided fakeable runtime.
  - `crates/botster-core/src/engine/plugin_worker.rs`: exported plugin worker engine and handler registration path.
  - `crates/botster-core/src/runtime/mod.rs`: public `SessionRuntime` and `PluginRuntime` traits.
  - `crates/botster-core-test-support/src/fake/mod.rs` and `fake/session_worker.rs`: reusable fake session/runtime helpers already used by tests.
  - Existing tests for the same surfaces: `session_runtime_contract_test.rs`, `session_worker_engine_test.rs`, `subscription_multiplexer_engine_test.rs`, `plugin_worker_engine_test.rs`, and downstream fake conformance tests.
- Project Pipelines checklist: run-level vault checklist `checklist_1780099073_649997` created after an initial plugin-worker timeout; checklist items should record notes read, convention conflict review, verification plan/evidence, and capture decision.

## Scope

Add non-product dev examples that make the embeddable engine tangible through the existing `botster-core-dev` crate and prove the same path with at least one smoke test using fake adapters.

In scope:

- Replace the placeholder `crates/botster-core-dev/src/main.rs` with a small dev-only executable that exercises public `botster-core` engine APIs.
- Keep the executable explicitly framed as a dev harness, not a real Botster CLI.
- Drive one cohesive smoke scenario through public contracts:
  - fake-spawn a session through a fake `SessionRuntime`;
  - attach a fake client through `SubscriptionMultiplexer`;
  - send terminal input through the multiplexer and into the fake/runtime or session worker path;
  - observe terminal output/activity through session output and multiplexer egress;
  - post or route a notification using the typed notification/session event path;
  - invoke a fake plugin handler through `PluginWorkerEngine`;
  - shut down the session through the runtime/worker path.
- Add at least one automated smoke test that calls the same harness function/path as the dev binary instead of duplicating the scenario in test-only code.
- Update `README.md` to state that `crates/botster-core-dev` contains dev harnesses for engine smoke testing and is not the product CLI.
- Add `botster-core-test-support` as a dev-only dependency of `botster-core-dev` if needed for tests/fakes.

Non-scope:

- No real CLI UX, install flow, auth, marketplace, persistent product config, hub daemon management, cloud federation, Rails relay, TUI, React/browser, Lua plugin loading, or provider policy.
- No new production dependency from `botster-core` to `botster-core-test-support`.
- No old `trybotster` path dependency. Old paths are reference evidence only.
- No broad refactor of engine APIs, runtime traits, package boundaries, or existing tests unless the smoke harness exposes a compile-blocking gap.
- No generated PII, local user paths, tokens, credentials, or machine-specific identifiers in printed example output or README prose.

Botster layers touched:

- Rust core dev harness: primary surface.
- Rust `botster-core` public engine/runtime API: exercised, but not expected to be changed.
- Rust test-support fakes: used from dev tests if needed.
- Docs: README only, plus this plan artifact.

Worktree/target assumption:

- Implementer should work only in this assigned `botster-core` pipeline worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts:

- Plan artifact: `docs/plans/botster-core-dev-engine-smoke-examples.md`.
- Gate evidence should cite this file, the main-rooted orchestrator correction, and checklist `checklist_1780099073_649997`.

## Assumptions And Unknowns

Assumptions:

- `crates/botster-core-dev` already exists specifically for this ticket class, so extending it is smaller than adding a new example crate.
- A shared Rust function such as `run_smoke_harness()` can be called by both `main()` and an integration/unit smoke test, proving the same runtime path.
- The harness can use fake adapters and deterministic ids. This is a dev example, so it does not need to spawn a real process or load a real Lua runtime.
- Existing public APIs are sufficient for the requested path. If a small helper is needed, prefer putting it in `botster-core-dev` or test-support, not in production core.
- Notification can be proven through existing typed notification/session event or inbox contracts, not through real OS/browser notification delivery.
- Plugin invocation can be proven with a fake `PluginRuntime` registered into `PluginWorkerEngine`; no interpreter is needed.
- README wording should make the non-product boundary visible without expanding into product docs.

Unknowns for implementation:

- Whether the cleanest shared path is a `lib.rs` in `botster-core-dev` plus thin `main.rs`, or a test-visible module inside `main.rs`. Prefer `lib.rs` if Rust integration tests need to import the path cleanly.
- Whether the existing fake session worker helpers are enough for the input/output/shutdown path, or whether the simpler `FakeSessionRuntime` path should be used for the spawn/input/output portion. Prefer the path that exercises the most public engine behavior without inventing new abstractions.
- Exact printed output format is open. Keep it deterministic and scrubbed of local paths.

No human question is blocking the plan. The ticket is clear: add dev-only examples/smoke harnesses for the core engine and avoid product CLI behavior.

## Affected Surfaces / Files

Expected changes:

- `crates/botster-core-dev/src/main.rs`
  - Thin executable entrypoint that runs the smoke harness and prints deterministic observations.
- `crates/botster-core-dev/src/lib.rs`
  - Likely shared harness function used by both the binary and tests.
- `crates/botster-core-dev/tests/engine_smoke_test.rs`
  - Smoke test calling the same harness path as the binary.
- `crates/botster-core-dev/Cargo.toml`
  - Add dev-only test support dependency if needed; possibly no production dependency changes beyond existing `botster-core`.
- `README.md`
  - Clarify `botster-core-dev` examples are dev harnesses, not product CLI or install UX.

Possible but not expected:

- `crates/botster-core-test-support/src/fake/*`
  - Only if a tiny reusable fake is missing and clearly belongs to downstream conformance support.
- `crates/botster-core/src/*`
  - Avoid unless a public API compile gap blocks the harness.

Not expected:

- Hub, CLI product, TUI, React/browser, Rails, Lua plugin, provider, marketplace, auth, or cloud files.
- Workspace membership changes; the crate is already in `Cargo.toml`.

## Implementation Shape

Suggested minimal shape:

- Add `crates/botster-core-dev/src/lib.rs` exposing:
  - `pub fn run_engine_smoke() -> Result<EngineSmokeReport, EngineSmokeError>` or a similarly small public function.
  - `EngineSmokeReport` with deterministic fields such as spawned session id, client egress count, observed output text, plugin result summary, notification count, and shutdown status.
- Keep `main()` to:
  - call `run_engine_smoke()`;
  - print a concise deterministic report;
  - return a non-zero exit on failure through a simple error display.
- In the harness:
  - create deterministic `SessionId`, `RequestId`, `ClientId`, and `SubscriptionId` values without local path or user data;
  - fake-spawn a session using `FakeSessionRuntime` or a local fake implementing public `SessionRuntime`;
  - subscribe a client through `SubscriptionMultiplexer`;
  - route terminal input and output through public `TransportIngress`, `SessionIoRequest`, `SessionIoEvent`, and `TransportEgress` values;
  - update or observe session activity through the existing activity/session output engine where practical;
  - route a typed notification event or inbox item and include it in the report;
  - register a fake plugin handler and invoke it through `PluginWorkerEngine`;
  - send shutdown through the fake runtime/worker path and include the shutdown observation.
- In the smoke test:
  - call the same `run_engine_smoke()` function;
  - assert the report proves every required scenario step occurred;
  - avoid snapshot-testing overly broad printed text.

## Risks

- The dev crate could accidentally grow into a product CLI. Keep command parsing, config, auth, persistence, daemon management, and install UX out.
- A test that reimplements the scenario instead of calling the harness would fail the "same path as the example" acceptance.
- Using local absolute paths, real user names, tokens, or environment-derived output would violate the no-PII acceptance.
- Adding fakes to production `botster-core` would blur the core/test-support boundary.
- Over-refactoring engine APIs to make the example prettier would exceed the ticket. The harness should adapt to the current public API.
- Only compiling the example without asserting behavior would miss the smoke-testing intent.
- Plugin-worker timeouts or threaded fake behavior can make tests flaky if the fake runtime sleeps. Prefer deterministic immediate fake handler results for this smoke test.

## Acceptance Checks / Tests

Required implementation checks:

- `cargo test -p botster-core-dev`
- `cargo run -p botster-core-dev`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Targeted smoke assertions:

1. The dev harness compiles and runs in test/CI mode.
2. The smoke test calls the same shared harness path as the binary.
3. The report proves a fake session was spawned.
4. The report proves a fake client subscribed/attached through `SubscriptionMultiplexer`.
5. The report proves terminal input was routed to a session-side request/runtime input.
6. The report proves output/activity was observed from the session and delivered to the subscribed client.
7. The report proves a notification was posted or routed through a typed core notification/session path.
8. The report proves a fake plugin handler was invoked through `PluginWorkerEngine`.
9. The report proves shutdown was requested/observed.
10. README explains these are dev harnesses, not the product CLI.
11. Source and output contain no local PII such as `/Users/`, personal names, tokens, or host-specific paths.

Runtime path proof:

- This ticket is intentionally dev-harness scaffold, not production host wiring.
- The path changed is the dev executable and its shared smoke function: `cargo run -p botster-core-dev` and the smoke test must drive public core engine/runtime APIs with fake adapters.
- Evidence that engine code already exists is not enough; the new acceptance evidence must show the dev example uses those APIs end to end.

## Vault Gaps Worth Capturing

No durable vault gap is required from planning alone.

Potential capture after implementation:

- If the final harness establishes a reusable pattern for `botster-core-dev` examples, capture a short convention describing how dev examples should share their runtime path with smoke tests and stay outside product CLI scope.
