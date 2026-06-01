# Ghostty Shadow-Terminal Architecture Plan

Ticket: Define Ghostty shadow-terminal architecture in botster-core
Run: run_1780282398_598491

## Context Loaded

- Pipeline context: ticket `ticket_1780282384_121710`, run
  `run_1780282398_598491`, step `botster_plan`, gate
  `botster_plan_gate`; prior plan artifact
  `artifact_1780282660_302741`; Plan Review
  `review_1780282952_974661`; open findings
  `finding_1780282952_914077`, `finding_1780282952_538385`,
  `finding_1780282952_961300`, and `finding_1780282952_602818`; no open
  questions or question answers.
- Worktree: pipeline-provided ticket worktree.
- Target: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Required playbooks loaded:
  - [[planner-playbook]]
  - [[botster-planner-playbook]]
- Additional vault/project constraints loaded:
  - [[identity]]
  - [[goals]]
  - [[botster-architecture]]
  - [[cli-patterns]]
  - [[spa-patterns]]
  - [[project pipeline orchestration belongs in a device-level botster plugin]]
  - [[project pipelines needs an operator workbench not more primitives]]
  - [[project pipelines ui contract belongs in the plugin readme]]
  - [[botster orchestration should spawn agents with explicit target ids]]
  - [[botster orchestration prompts must bind agents to explicit worktrees]]
  - [[pty sessions use vt100 shadow screen]]
  - [[non-broker sessions and local shadow screen are wrong sources for pty state]]
  - [[libghostty-vt embedder callback architecture and constraints]]
  - [[botster terminal clients share one sessionio data plane subscription path]]
  - [[botster bundles xterm-ghostty terminfo at build time via tic compilation]]
  - [[ghostty scrollbar state is the source of truth for tui terminal scroll]]
  - [[backpressure recovery uses broker snapshots not empty shadow screen]]
- Repo context inspected:
  - `Cargo.toml`
  - `README.md`
  - `docs/plans/terminal-screen-snapshot-boundary.md`
  - `crates/botster-core/Cargo.toml`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/contract/terminal_screen.rs`
  - `crates/botster-core/src/engine/terminal_screen.rs`
  - `crates/botster-core/src/engine/mod.rs`
  - `crates/botster-core-test-support/src/fake/terminal_screen.rs`
  - `crates/botster-core/tests/terminal_screen_contract_test.rs`
  - `crates/botster-core-test-support/tests/downstream_conformance_test.rs`

## Scope

Define Ghostty as the blessed core-side shadow-terminal backend path while
keeping the `botster-core` crate itself a reusable backend-neutral contract and
engine crate. This ticket should make the architecture explicit through docs
and a compileable workspace/module skeleton, not a full libghostty integration.

In scope:

- Add a docs architecture note, likely
  `docs/ghostty-shadow-terminal-architecture.md`, that reconciles this ticket
  with `docs/plans/terminal-screen-snapshot-boundary.md`: `botster-core` keeps
  the neutral `TerminalScreenRuntime` seam, while the server/core-side Botster
  runtime should use Ghostty as the authoritative implementation for tmux-like
  screen and snapshot truth.
- Update the terminal screen/snapshot documentation only where needed to remove
  flat Ghostty/restty equivalence. The README should keep `botster-core`'s
  neutrality and ownership-boundary claims intact while drawing the
  authoritative-backend-vs-client-renderer distinction.
- Reframe restty as a web/client rendering path only. It may consume terminal
  state and streams, but it must not be named as an authoritative core shadow
  terminal or parser backend.
- Add a workspace crate skeleton, likely
  `crates/botster-terminal-ghostty`, that depends on `botster-core` and exposes
  a small documented adapter boundary for a future Ghostty-backed
  `TerminalScreenRuntime`.
- Put the strong "Ghostty is the authoritative core-side backend; restty is
  client/out-of-core rendering" statement in the new
  `botster-terminal-ghostty` crate docs and the docs architecture note, not as a
  rewritten `botster-core` ownership policy.
- Keep the new Ghostty crate free of real libghostty build, submodule, Zig, FFI,
  or terminfo work in this slice unless implementation discovers a no-op module
  cannot compile without a tiny local shape.
- Add focused tests or doc tests that prove the public skeleton compiles against
  the existing `TerminalScreenRuntime` seam.
- Add a per-crate `[lints]` table to the new crate mirroring `botster-core`,
  unless implementation instead moves the repo to `[workspace.lints]` and
  opts crates into it.
- Update the workspace `Cargo.toml` and any README/docs table needed to make the
  new crate discoverable.

Non-scope:

- No real Ghostty parser, FFI bindings, build.rs, Zig setup, vendored source, or
  submodule initialization.
- No restty integration, browser terminal renderer changes, React/Catalyst UI,
  TUI rendering, Rails relay, WebRTC, or hub policy changes.
- No new terminal emulator in `botster-core`; the existing
  `TerminalScreenEngine`/`TerminalScreenRuntime` boundary remains the reusable
  core seam.
- No behavior-returning `GhosttyTerminalRuntime` placeholder that pretends to be
  a runtime and only returns "unavailable". Prefer docs plus marker/config
  types or a doc-tested adapter signature that references the seam without
  exposing unwired behavior.
- No replacement of existing session, actor, transport, or session protocol
  snapshot carriers.
- No broad extraction cleanup, compatibility layers, feature-flag matrix, or
  optional backend selection framework.
- No PII or machine-specific paths in committed docs beyond the pipeline plan
  metadata already used by existing plan artifacts.

Botster layers touched: Rust core docs/contracts, Rust workspace/crate layout,
and compile/test scaffolding. Hub, plugin, session/client worker, TUI, React
SPA, Rails relay, MCP, and Project Pipelines runtime layers are referenced only
as consumers or workflow evidence.

## Assumptions And Unknowns

Assumptions:

- `botster-core` should stay backend-neutral at the public trait level, but the
  repo should name Ghostty as the blessed concrete adapter path for
  authoritative server/core-side terminal state.
- A separate crate is the right boundary for concrete Ghostty integration. It
  keeps `botster-core` reusable while giving future libghostty build and FFI
  work a clear home.
- The current `TerminalScreenEngine`, `TerminalScreenRuntime`,
  `TerminalSnapshotPayload`, and `TerminalScreenState` are still the seam a
  Ghostty adapter should implement.
- The existing terminal-screen boundary tests already prove renderer-neutral
  opaque snapshots and SessionWorker integration; this ticket should extend the
  architecture story rather than replace that work.
- "Core owns authoritative terminal screen/snapshot state" means core-side
  runtime/session infrastructure owns the truth used for tmux-like detach,
  reattach, snapshot, and recovery behavior. It does not mean browser restty or
  local renderer instances are authoritative.
- restty may still be valid for the web/client rendering path, but not as a
  dependency or authority for core shadow-terminal infrastructure.
- Compileable skeleton is acceptable because the ticket asks to define
  architecture and module structure, not to ship a working Ghostty backend, but
  the skeleton must not include dead behavior that appears production-ready.

Unknowns for the implementer to resolve narrowly:

- Exact crate name: prefer `botster-terminal-ghostty` unless local naming
  conventions or Cargo metadata suggest a better fit.
- Whether the skeleton should expose documentation-only marker/config types or
  a doc-tested adapter function/type signature. Default to marker/config types;
  do not add a behavior-returning placeholder runtime unless the implementer can
  prove it is not dead or misleading.
- Whether a common terminal crate is justified now. Default answer should be no:
  keep common contracts in `botster-core` until concrete duplication appears.
- Whether tests should be ordinary Rust tests in the new crate, source-scanning
  tests in `botster-core`, or both. Choose the smallest test that proves the
  architectural contract without brittle prose matching.

No human question is needed before implementation. The ticket's intent is
specific enough, and the plan does not waive any acceptance point.

## Affected Surfaces / Files

Expected:

- `Cargo.toml`
  - Add the new Ghostty adapter crate to workspace members.
- `crates/botster-terminal-ghostty/Cargo.toml`
  - New package metadata and dependency on `botster-core`.
  - Per-crate lint configuration matching `botster-core`, unless the
    implementation deliberately centralizes lints in `[workspace.lints]`.
- `crates/botster-terminal-ghostty/src/lib.rs`
  - Compileable documented module skeleton for the future Ghostty-backed
    `TerminalScreenRuntime` adapter. Prefer marker/config types and doc-tested
    seam examples over an unwired runtime implementation.
  - Explicit docs that Ghostty is the blessed authoritative shadow-terminal
    backend path for core-side terminal truth.
  - Explicit docs that restty is client/out-of-core rendering and must not be
    used as core shadow-terminal infrastructure.
- `crates/botster-terminal-ghostty/tests/architecture_contract_test.rs`
  - Focused tests proving exported skeleton shape and guarding the restty
    boundary if a source guard is useful.
- `README.md`
  - Update workspace layout and terminal boundary sections to include the
    Ghostty adapter crate and restty/client-renderer distinction without
    rewriting `botster-core` as a concrete-backend owner.
- `docs/ghostty-shadow-terminal-architecture.md`
  - Architecture note that cross-links the prior terminal-screen boundary plan
    and states the final placement: `botster-core` owns the seam, the sibling
    Ghostty crate owns the concrete core-side backend path, and restty remains
    client/out-of-core.
- `docs/plans/ghostty-shadow-terminal-architecture.md`
  - This plan artifact.

Possibly touched only if needed:

- `crates/botster-core/src/contract/terminal_screen.rs`
  - Documentation-only wording only if needed to clarify that this remains a
    backend-neutral seam. Do not make this file bless or depend on Ghostty.
- `crates/botster-core/src/engine/terminal_screen.rs`
  - Documentation-only wording only if needed to keep the engine docs aligned
    with the neutral-seam architecture.
- `crates/botster-core/tests/terminal_screen_contract_test.rs`
  - A small source guard if the better place to ban restty-as-authority wording
    is the existing terminal boundary suite.

Not expected:

- Any real Ghostty, restty, Zig, WASM, Vite, React, TUI, hub, Rails, MCP, or
  plugin implementation files.
- Any new third-party dependency.
- Any session protocol frame, actor contract, transport frame, entity, UI,
  crypto, identity, package, or notification contract changes.

## Risks

- Adding Ghostty directly to `botster-core` would make the reusable core crate
  depend on a concrete backend and likely on native build policy.
- Leaving README wording that lists Ghostty and restty as equivalent host
  choices would fail the ticket's architectural reframe.
- A skeleton that exposes fake behavior as if it were production-ready would
  mislead consumers. Avoid behavior-returning unavailable runtimes; keep the
  skeleton to docs, marker/config types, and doc-tested seam references unless
  real behavior exists.
- Adding a "common terminal" crate before duplication exists would create
  speculative abstraction. Keep common contracts in `botster-core` for now.
- Over-broad source guards can make docs brittle. Treat `rg` output as a manual
  review aid unless implementation adds a robust doctest or crate-level
  architecture test.
- Compile checks can fail from missing docs or clippy lints only if the new
  crate has its own `[lints]` table or the workspace centralizes lints. Existing
  lints are currently per-crate, not inherited from the workspace.
- Runtime-path proof is intentionally scaffold-level. The plan must document
  that no production Ghostty backend is wired yet and prove only the public
  crate/module path that a future adapter will use.

## Acceptance Checks / Tests

Implementation acceptance:

- New Ghostty crate docs and the docs architecture note state that Ghostty is
  the blessed core-side shadow-terminal backend path for authoritative
  screen/snapshot state.
- README is minimally reframed so it no longer lists Ghostty and restty as
  equivalent backend choices. It must preserve `botster-core`'s neutral trait
  seam and ownership-boundary language.
- New Ghostty crate docs and the docs architecture note state that restty is
  client-side/out-of-core rendering and not core shadow-terminal
  infrastructure.
- A compileable `botster-terminal-ghostty` crate exists in the workspace and is
  discoverable from README workspace layout.
- The new crate depends on `botster-core` and references the existing
  `TerminalScreenRuntime` seam instead of introducing a parallel terminal
  abstraction.
- The new crate has per-crate lint configuration matching `botster-core`, or
  lints are centralized with an explicit workspace-level change.
- The skeleton does not expose a behavior-returning unavailable runtime. Any
  compileable API is docs/marker/config oriented or doc-tested as an adapter
  shape.
- No real Ghostty backend dependency, build.rs, FFI, submodule, or WASM policy
  is added. No restty client-renderer policy is added.
- No PII is introduced.

Suggested tests/checks:

- `cargo test -p botster-terminal-ghostty`
  - Proves the new crate skeleton compiles and any architecture guard tests pass.
- `cargo test -p botster-core terminal_screen`
  - Rechecks the existing terminal screen seam after any doc or contract wording
    changes.
- `cargo test --workspace`
  - Proves the workspace member addition did not break existing crates.
- `cargo clippy --workspace --all-targets -- -D warnings`
  - Required after adding per-crate lints to the new crate, or after moving
    lints to `[workspace.lints]`.
- `cargo fmt --all -- --check`
  - Verifies formatting after adding the crate and docs-adjacent tests.
- `rg -n "restty|Ghostty|ghostty|TerminalScreenRuntime|botster-terminal-ghostty" README.md docs crates`
  - Manual review aid only. This is not the architectural guarantee; the review
    should inspect the docs and any doctest/architecture test directly.

Runtime/user-path proof:

- This ticket is intentionally scaffold-level. The production entry path remains
  the existing `SessionWorkerRuntime` and `TerminalScreenRuntime` seam. The new
  proof is that a workspace crate intended for the Ghostty adapter compiles
  against that seam and is documented as the future authoritative backend path.
  A later implementation ticket must wire real Ghostty parsing into that crate
  and then prove session attach/snapshot/recovery behavior through the
  SessionIo/ClientWorker data path.

## Vault Gaps Worth Capturing

- Capture after implementation if the repo settles the concrete crate boundary:
  "Ghostty terminal integration belongs in botster-terminal-ghostty, while
  botster-core owns only the trait/contract seam."
- Capture after implementation if the restty distinction becomes a durable rule:
  "restty is a client renderer path, not an authoritative shadow-terminal
  backend."
- No convention conflict found in planning. The plan follows existing Botster
  boundaries: core contracts stay reusable, product/client rendering stays out
  of core, SessionIo/ClientWorker remains the data plane, and Project Pipelines
  evidence is recorded through gate artifacts and checklist evidence.
