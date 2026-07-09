# Default Ghostty Production Feature Path Plan

Ticket: `ticket_1783552972_584748`
Run: `run_1783624341_891447`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Core: make Ghostty-backed terminal truth the default local production feature path`, run `run_1783624341_891447`, active Plan step `botster_plan`, gate `botster_plan_gate`.
- Dependencies are closed:
  - `Core: expose scrollback, screen, and snapshot through CoreDaemon production API`
  - `Core: fail loud when CoreDaemon durability is claimed without worker path`
- No prior artifacts, reviews, findings, questions, or answers were present in the run context.
- Required playbooks loaded: [[planner-playbook]], [[botster-planner-playbook]].
- Vault/project context loaded: [[botster-architecture]], [[cli-patterns]], [[ghostty shadow terminal integration belongs outside botster core]], [[session-process-owns-vt-parser-hub-rpc-snapshots]], [[coredaemon must expose terminal truth used by the production hub path]], [[botster engine command surface uses botsterengine as facade]], [[coredaemon embedding without worker path creates in process sessions]], [[botster-core local process runtime is feature-gated from contract-only embeds]], [[botster build rs mise exec can override path zig selection]], [[test script required for rust tests not cargo test]], and [[botster cli integration tests require ghostty submodule initialization]].
- Repo context inspected:
  - `Cargo.toml`
  - `README.md`
  - `docs/architecture/ghostty-shadow-terminal-adapter.md`
  - `docs/archive/plans/wire-shadow-terminal-state-managed-session-runtime.md`
  - `docs/archive/plans/terminal-backend-shadow-state-conformance-contract.md`
  - `docs/archive/plans/pin-and-build-trybotster-ghostty-libghostty-vt-adapter.md`
  - `crates/botster-core/Cargo.toml`
  - `crates/botster-core/src/engine/botster.rs`
  - `crates/botster-core/src/engine/managed_session_runtime.rs`
  - `crates/botster-core/src/bin/botster-session-worker.rs`
  - `crates/botster-core/src/runtime/worker_process.rs`
  - `crates/botster-core-daemon/Cargo.toml`
  - `crates/botster-core-daemon/src/daemon.rs`
  - `crates/botster-core-daemon/src/main.rs`
  - `crates/botster-terminal-ghostty/Cargo.toml`
  - `crates/botster-terminal-ghostty/src/lib.rs`
  - `crates/botster-terminal-ghostty/tests/managed_session_runtime_test.rs`
  - `.github/workflows/ci.yml`
  - `.gitmodules`
  - `crates/botster-terminal-ghostty/build_support.rs`
- Project Pipelines checklist creation initially timed out to the caller, but the checklist later appeared in run context and was updated with revised evidence after Plan Review.
- Plan Review returned `changes_required` with five findings. This revision fixes them by deciding the feature matrix, adding CI scope, replacing vacuous opt-out checks, naming the non-generic engine strategy, and reconciling raw Cargo verification with the checkout-specific test-wrapper vault note.
- Second Plan Review returned `changes_required` with two new findings. This revision fixes them by naming existing daemon snapshot-format assertions that must be feature-gated, adding an executed no-default-features daemon test lane, and stating that both GitHub Actions jobs need independent submodule and Zig setup.

Botster layers touched: Rust core facade hooks, Rust daemon host profile, Ghostty adapter crate tests/docs, and possibly README/architecture docs. No Lua plugin, TUI, SPA, Rails relay, MCP, or hub UI work.

Worktree/target assumption: downstream agents should work in the assigned pipeline worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, not an ambient checkout.

## Scope

Make the product/default local production path use the sibling Ghostty terminal adapter when the relevant default product feature is enabled, while keeping `botster-core` contract-only builds free of Ghostty, Zig, and native build requirements.

In scope:

- Preserve the accepted package boundary: `botster-core` owns backend-neutral terminal contracts and facade hooks; `botster-terminal-ghostty` owns Ghostty/libghostty-vt implementation.
- Add the smallest core facade hook needed for host-supplied terminal backends on the default local and worker-backed engine path. The chosen shape is monomorphic facade constructors that accept a terminal backend factory, not generic public facade types.
- Wire `botster-core-daemon` as the first-party production host profile so its default local/product feature path uses `botster-terminal-ghostty` for terminal screen/snapshot state.
- Keep `CoreDaemonConfig::with_worker_path(...)` as the durable production path; the default Ghostty path must be exercised through worker-backed daemon/session runtime, not only an in-process library demo.
- Decide and document the feature matrix as part of the implementation:
  - `botster-core-daemon` gets `default = ["ghostty-terminal"]`;
  - `ghostty-terminal = ["dep:botster-terminal-ghostty", "botster-terminal-ghostty/libghostty-vt"]`;
  - default `botster-core-daemon` and default workspace builds now require Zig `0.15.2` and initialized `crates/botster-terminal-ghostty/vendor/ghostty`;
  - `botster-core-daemon --no-default-features` is the pure-contract/no-native opt-out proof for daemon embedders;
  - `botster-core` still has no Ghostty/Zig dependency, and its existing `default = ["local-runtime"]` feature matrix remains unchanged.
- Update CI so the main Linux workspace path supports the new product default instead of creating a separate default-off lane. The CI strategy is to initialize submodules and install/provide Zig `0.15.2` before the existing workspace `cargo clippy`, `cargo test`, docs, and release build commands.
- Add focused runtime proof that the production host path uses Ghostty-backed terminal state for screen/snapshot fidelity, not just that Ghostty types compile.
- Add focused no-default-feature proof that contract-only embeds still compile without native Ghostty/Zig.
- Preserve executed plain-backend daemon coverage under `--no-default-features`; do not let default-on Ghostty remove the plain backend's snapshot contract tests.
- Update docs to state: Ghostty is the blessed backend, it lives in `botster-terminal-ghostty`, it is enabled on the product/default local production path, and pure contracts can opt out.

Non-scope:

- No moving Ghostty FFI, Zig build scripts, vendored Ghostty source, or native parser implementation into `botster-core`.
- No hub UI, browser/TUI rendering, restty work, Rails relay, Lua plugin, MCP, cloud, or Project Pipelines UI changes.
- No broad feature-flag framework, runtime backend registry, product policy DSL, or optional configurability beyond the feature matrix required by this ticket.
- No rewrite of session-worker framing or `TerminalScreenRuntime`.
- No attempt to make `botster-core` depend on `botster-terminal-ghostty`; that would violate the accepted boundary and create a dependency cycle.

## Assumptions And Unknowns

Assumptions:

- The phrase "DefaultBotsterEngine / ManagedSessionRuntime / worker path can use Ghostty adapter" means core should expose enough facade/factory hooks for the default local engine family to be instantiated with a Ghostty backend, while the actual Ghostty dependency is owned by a host crate.
- `botster-core-daemon` is the right first-party production host profile to make Ghostty default-on, because it depends on `botster-core` and can optionally depend on `botster-terminal-ghostty` without a cycle.
- `botster-core` default features should continue to mean local process/runtime availability, not native Ghostty. The Ghostty default belongs in a product/host crate or adapter feature, not in reusable core.
- The plan intentionally chooses the product-default tradeoff requested by the ticket: default `botster-core-daemon` builds require native Ghostty prerequisites. Keeping default CI Zig-free would contradict the ticket preference and would require a human waiver.
- Cargo workspace feature unification means adding Ghostty to `botster-core-daemon` defaults activates `botster-terminal-ghostty/libghostty-vt` for existing `cargo * --workspace` CI commands. The workflow must satisfy Zig/submodule prerequisites before those commands run.
- The production proof should use `CoreDaemonConfig::with_worker_path(...)` because durability without a worker path is explicitly a footgun.
- Existing `botster-terminal-ghostty` feature-enabled tests already prove managed-session Ghostty behavior against a fake session runtime; this ticket must add or adapt proof for the first-party production path.
- Feature-enabled Ghostty tests may require initialized `crates/botster-terminal-ghostty/vendor/ghostty` and Zig `0.15.2`; the plan should document and gate that rather than forcing pure contract builds to satisfy it.
- This repo has no test wrapper. Verified with `rg --files -g 'test.sh' -g 'script/**'`, which returned no files, and `.github/workflows/ci.yml` invokes Cargo directly. [[test script required for rust tests not cargo test]] is scoped to the Botster CLI checkout, so raw Cargo is the repo-approved idiom here.
- Flipping `botster-core-daemon` defaults to Ghostty must make existing daemon snapshot-format assertions fail loudly until they are feature-gated. That breakage is desired evidence that the production path actually changed, but the plan must preserve plain-backend assertions on the no-default-features lane.

Unknowns for implementation:

- Exact internal mechanics for boxing or erasing the terminal backend factory without making `CoreDaemon` generic. The public strategy is fixed: preserve `DefaultBotsterEngine::new()`, `DefaultBotsterEngine::worker_backed(...)`, `WorkerBackedBotsterEngine::new(...)`, and `CoreDaemon` signatures.
- Exact CI Zig installation command. It must leave a Zig `0.15.2` binary discoverable by `build_support.rs` through `BOTSTER_ZIG`, `ZIG`, mise-managed Zig, `zig` on `PATH`, or `mise exec -- zig`.

No human question is blocking. The dependency direction and contract-only opt-out give a clear implementable interpretation without waiving ticket acceptance.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/botster.rs`
  - Add `DefaultBotsterEngine::with_terminal_backend_factory(...)` and `WorkerBackedBotsterEngine::with_options_and_terminal_backend_factory(...)` or equivalent monomorphic constructors.
  - Preserve existing `DefaultBotsterEngine::new()`, `DefaultBotsterEngine::worker_backed(...)`, `WorkerBackedBotsterEngine::new(...)`, and `WorkerBackedBotsterEngine::with_options(...)` signatures.
- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Reuse the existing generic backend factory; avoid duplicating terminal state machinery.
- `crates/botster-core/src/engine/mod.rs`, `crates/botster-core/src/lib.rs`, and possibly `crates/botster-core/src/prelude.rs`
  - Export only the new facade hook types/functions that embedders need.
- `crates/botster-core-daemon/Cargo.toml`
  - Add `[features] default = ["ghostty-terminal"]`.
  - Add `ghostty-terminal = ["dep:botster-terminal-ghostty", "botster-terminal-ghostty/libghostty-vt"]`.
  - Add optional `botster-terminal-ghostty` dependency.
- `crates/botster-core-daemon/src/daemon.rs`
  - Select the Ghostty-backed engine on the product/default feature path.
  - Retain the plain backend only under `#[cfg(not(feature = "ghostty-terminal"))]` for `--no-default-features`.
  - Keep `DaemonEngine` and `CoreDaemon` public signatures non-generic, likely through private type aliases or boxed terminal backend construction.
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  - Add or adapt a worker-backed Ghostty screen/snapshot fidelity test that proves `CoreDaemon` uses Ghostty terminal truth through the production path.
  - Update the three existing snapshot-format assertions that currently hard-code `plain-opaque-v1`:
    - `worker_backed_capture_snapshot_drains_before_capture_and_preserves_client_egress_once` around line 614.
    - `local_daemon_read_screen_and_capture_snapshot_use_in_process_engine_path` around line 668.
    - `daemon_screen_and_snapshot_negative_paths_return_errors_without_panics` around line 798.
  - Gate each format assertion by feature: under `#[cfg(feature = "ghostty-terminal")]` expect `ghostty-terminal-snapshot-v1`; under `#[cfg(not(feature = "ghostty-terminal"))]` expect `plain-opaque-v1`.
  - Rename or re-comment `local_daemon_read_screen_and_capture_snapshot_use_in_process_engine_path`, because with default Ghostty it still uses the local daemon path but no longer implies the plain in-process terminal backend.
- `crates/botster-terminal-ghostty/tests/managed_session_runtime_test.rs`
  - Keep as adapter-level managed runtime proof; extend only if the new facade hook is best proven here.
- `README.md`, `docs/architecture/core-daemon.md`, and/or `docs/architecture/ghostty-shadow-terminal-adapter.md`
  - Document default feature matrix, command to prove Ghostty fidelity, Zig/submodule requirement, and contract-only opt-out.
- `.github/workflows/ci.yml`
  - Update both jobs, `verify` and `release-verification`; GitHub Actions jobs do not share checkout state or PATH.
  - Add `with: submodules: recursive` to both `actions/checkout@v4` steps.
  - Add a Linux Zig `0.15.2` setup step to both jobs before their respective Cargo commands. The installed binary must be discoverable by the adapter build script, preferably via `BOTSTER_ZIG` or `zig` on `PATH`.
  - Keep the existing main Linux `verify` workspace commands and the `release-verification` `cargo build --workspace --release` command as the Ghostty-enabled product-default CI path after prerequisites are installed.
  - Add a `cargo test -p botster-core-daemon --no-default-features` lane in `verify` so the plain daemon backend is executed, not only compiled.

Possible:

- `crates/botster-core-dev/Cargo.toml` and tests if a dev smoke harness is the cleanest documented command proving fidelity.
- `Cargo.lock` if feature/dependency wiring changes.

Not expected:

- `crates/botster-core/src/bin/botster-session-worker.rs` unless implementation proves the worker process itself must own the Ghostty parser. The current worker sends PTY bytes; parent-side managed runtime owns terminal backend state for daemon readback.
- `crates/botster-core/src/runtime/local_process.rs` or `worker_process.rs` except for necessary type plumbing.
- Any SPA/TUI/Rails/Lua/MCP files.

## Risks

- Adding `botster-terminal-ghostty` as a `botster-core` dependency would create a cycle and violate [[ghostty shadow terminal integration belongs outside botster core]].
- Making native Ghostty part of pure `botster-core` defaults would force Zig/native build requirements on contract-only embedders.
- Cargo workspace feature unification turns `botster-core-daemon`'s default `ghostty-terminal` feature into a native Ghostty requirement for every `cargo * --workspace` command. CI must install Zig `0.15.2` and initialize submodules before existing workspace commands.
- GitHub Actions setup is per job. If submodule checkout and Zig setup are added only to `verify`, `release-verification` still fails on `cargo build --workspace --release`.
- Proving only adapter crate tests would miss the ticket's production-path requirement. At least one acceptance check must drive `CoreDaemon` or an explicitly documented first-party host path.
- A plain-backend fallback under the same default product feature would make the path appear wired while silently not using Ghostty. The plain backend should exist only on `--no-default-features` daemon builds.
- Overwriting existing `plain-opaque-v1` daemon assertions to only expect Ghostty would erase plain-backend coverage. Feature-gate those assertions and run `cargo test -p botster-core-daemon --no-default-features`.
- Feature-gated docs can overpromise CI if Zig/submodule preconditions are not present. The command must state exact features and setup.
- Generic facade changes could cause public API churn through `DaemonEngine` and `CoreDaemon`. The implementation should add monomorphic factory constructors or private aliases and preserve existing facade/CoreDaemon signatures.
- Existing strict lints and rustdoc can fail on feature-gated doc links. Docs mentioning Ghostty-backed facade items must compile under relevant feature sets or use plain code text where needed.
- Native feature tests may fail before test logic if `vendor/ghostty` is uninitialized or Zig resolution uses the wrong toolchain; record clear precondition evidence.

## Acceptance Checks / Tests

Required implementation checks:

- Contract/no-native opt-out on the crate that gains the default:
  - `cargo check -p botster-core-daemon --no-default-features` succeeds on a machine with no Zig and an uninitialized `vendor/ghostty`.
  - `cargo test -p botster-core-daemon --no-default-features` executes the plain daemon backend and keeps `plain-opaque-v1` snapshot-format coverage alive.
  - `cargo tree -p botster-core-daemon --no-default-features -e=no-dev` contains no `botster-terminal-ghostty` edge.
  - Keep `cargo check -p botster-core --no-default-features` and `cargo tree -p botster-core -e=no-dev` as boundary regression guards, not the primary ticket acceptance proof.
- Product/default host path:
  - `cargo test -p botster-core-daemon --test daemon_integration_test -- <ghostty fidelity test name>` runs with default features and proves `CoreDaemon` with `with_worker_path(...)` reads screen/snapshot state from `botster-terminal-ghostty`.
  - The test should emit VT output that the plain byte-preserving backend cannot falsely prove as fidelity, for example colored/styled text where `read_screen` strips escape sequences and `capture_snapshot` returns non-empty opaque Ghostty snapshot bytes with `ghostty-terminal-snapshot-v1`.
  - Existing daemon snapshot tests named above must assert `ghostty-terminal-snapshot-v1` on default features and `plain-opaque-v1` on `--no-default-features`.
- Adapter feature path:
  - `cargo test -p botster-terminal-ghostty --features libghostty-vt` passes when the Ghostty submodule and Zig `0.15.2` are available.
- Default workspace path:
  - `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo test --doc --workspace`, `cargo doc --workspace --no-deps`, and `cargo build --workspace --release` run after submodule initialization and Zig `0.15.2` setup. These are now Ghostty-enabled product-default checks because `botster-core-daemon` default features activate the native adapter.
  - In CI, both `verify` and `release-verification` jobs initialize submodules and set up Zig `0.15.2` independently before their Cargo commands.
- Strict lint/docs:
  - Run the repo's raw Cargo lint/doc gates for changed packages. At minimum: `cargo clippy -p botster-core --all-targets -- -D warnings`, `cargo clippy -p botster-core-daemon --all-targets --all-features -- -D warnings`, `cargo clippy -p botster-core-daemon --no-default-features --all-targets -- -D warnings`, and `cargo test -p botster-core --doc --no-default-features` if docs touch feature-gated core items.
- Source boundary checks:
  - `rg -n "botster-terminal-ghostty|libghostty|zig|build\\.rs|restty" crates/botster-core` must show no new core native/backend dependency beyond intentional docs or source guards.
  - No committed artifacts should include local home paths or PII.

## Vault Gaps Worth Capturing

- Capture a durable note after implementation if `botster-core-daemon` becomes the settled location for product-default Ghostty feature wiring because that is a reusable dependency-cycle pattern: core exposes the seam, host profile owns the concrete backend default.
- Capture a durable note if CI settles a new feature-enabled Ghostty job pattern, especially around Zig `0.15.2`, submodule initialization, and default workspace test policy.
- Capture a durable note if the daemon test pattern settles as feature-gated backend format assertions plus a no-default-features execution lane for fallback backend contracts.
- Capture a correction to [[test script required for rust tests not cargo test]] or a sibling note clarifying that it applies to the Botster CLI checkout, while this `botster-core` workspace uses raw Cargo as shown by `.github/workflows/ci.yml`.

## Checklist Evidence

- Vault notes read: listed in Context Loaded.
- Convention conflicts: none after reconciliation. The plan preserves the core/provider boundary, avoids native dependency sprawl in `botster-core`, keeps product defaults in a host profile, protects contract-only builds with `botster-core-daemon --no-default-features`, and explicitly scopes the raw-Cargo verification convention to this checkout.
- Verification evidence: planning inspection used `project_pipelines_current_context`, `rg`, and targeted `sed` reads over manifests, docs, facades, daemon, worker, Ghostty tests, `.gitmodules`, `build_support.rs`, `.github/workflows/ci.yml`, and the three existing daemon snapshot-format assertions in `daemon_integration_test.rs`. `rg --files -g 'test.sh' -g 'script/**'` returned no files, confirming no local wrapper.
- Durable knowledge capture: no capture yet; candidate notes listed under Vault Gaps.
- Checklist tool evidence: initial checklist creation calls timed out to the caller, then persisted asynchronously. Checklist `checklist_1783624524_522548` was updated after Plan Review with revised vault, convention, verification, and capture evidence.

## Plan Review Findings Addressed

- `finding_1783624847_283418`: resolved by choosing default-on `botster-core-daemon` `ghostty-terminal` feature wiring and stating that default workspace builds now require Zig/submodule prerequisites.
- `finding_1783624847_286274`: resolved by inspecting `.github/workflows/ci.yml`, adding it to affected surfaces, naming workspace feature unification, and requiring CI checkout submodules plus Zig `0.15.2` setup before existing workspace commands.
- `finding_1783624847_886576`: resolved by replacing the primary opt-out proof with `botster-core-daemon --no-default-features` checks and demoting `botster-core` checks to boundary guards.
- `finding_1783624847_470933`: resolved by recording that the test-wrapper note is scoped to the CLI checkout; this repo has no wrapper and uses raw Cargo in CI.
- `finding_1783624847_552812`: resolved by rejecting public facade genericization and choosing monomorphic backend-factory constructors/private aliases that preserve `CoreDaemon` and existing facade signatures.
- `finding_1783625208_645086`: resolved by naming the three existing daemon `plain-opaque-v1` assertion sites, requiring feature-gated format assertions for Ghostty and plain backends, renaming/re-commenting the local daemon path test, and adding `cargo test -p botster-core-daemon --no-default-features`.
- `finding_1783625208_837499`: resolved by requiring submodule checkout and Zig `0.15.2` setup in both GitHub Actions jobs, `verify` and `release-verification`.
