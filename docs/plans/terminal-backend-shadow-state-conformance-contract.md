# Terminal Backend Shadow State Conformance Contract Plan

Ticket: Add terminal backend conformance contract for shadow state
Run: run_1780282400_744091

## Context Loaded

- Pipeline context: ticket `ticket_1780282384_356500`, current step `botster_plan`, gate `botster_plan_gate`; no prior artifacts, findings, questions, or answers.
- Required playbooks: [[planner-playbook]], [[botster-planner-playbook]].
- Additional vault/project constraints: [[identity]], [[goals]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[plan steps need reviewable plan artifacts]].
- Repo context inspected: `Cargo.toml`, `crates/botster-core/Cargo.toml`, `crates/botster-core-test-support/Cargo.toml`, `crates/botster-core/src/contract/terminal_screen.rs`, `crates/botster-core/src/engine/terminal_screen.rs`, `crates/botster-core-test-support/src/fake/terminal_screen.rs`, `crates/botster-core-test-support/src/conformance/mod.rs`, `crates/botster-core-test-support/src/lib.rs`, `crates/botster-core/tests/terminal_screen_contract_test.rs`, `crates/botster-core-test-support/tests/downstream_conformance_test.rs`, and prior plan `docs/plans/terminal-screen-snapshot-boundary.md`.
- Botster layers touched: Rust core contract/engine and Rust test-support only. This is core shadow terminal state, not hub, browser, TUI, Rails, Lua plugin, or rendering work.
- Worktree/target assumptions: this plan is for the pipeline-provided worktree and explicit target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`; downstream agents should keep using the assigned worktree, not ambient checkouts.

## Scope

Define reusable conformance assertions in `botster-core-test-support` for terminal shadow-state backends. The contract should prove that any backend adapter can:

- Feed PTY output bytes without dropping binary data.
- Maintain screen dimensions across output, resize, snapshot, and restore operations.
- Read current screen state through the public terminal screen state shape.
- Produce opaque snapshots that preserve backend-owned bytes and optional format labels.
- Replay/restore snapshot state when the backend supports restore.
- Preserve resize behavior before and after snapshot capture/restore.
- Preserve snapshot-before-live-output semantics by proving held live output is emitted only after the initial snapshot barrier is released.

The smallest shape is a test-support conformance module that drives the already-exported `TerminalScreenRuntime`/`TerminalScreenEngine` contract against a backend supplied by a caller. The fake terminal runtime should run those same conformance assertions so reviewers can see the contract is executable, not only documented.

## Non-Scope

- No Ghostty, libghostty-vt, restty, WASM, or vendored terminal backend implementation.
- No client rendering assertions, browser/TUI behavior, React/Catalyst UI, or terminal cell painting policy.
- No new terminal protocol frame family unless compiler pressure proves an existing public type cannot carry the contract.
- No new runtime dependency in `botster-core` or `botster-core-test-support`.
- No broad refactor of `SessionWorkerEngine`, `ManagedSessionRuntime`, transport routing, or the existing terminal screen boundary.
- No PII-bearing fixtures or local path capture in tests or docs.

## Assumptions And Unknowns

- Existing `TerminalScreenRuntime` already names the backend seam. The new ticket should add reusable conformance assertions around that seam rather than introduce a parallel backend trait.
- Existing `TerminalSnapshotPayload` is the right opaque snapshot carrier for core shadow state; snapshots stay byte payloads plus dimensions and optional host-owned format label.
- Snapshot restore support can be represented as a backend operation that replays a payload. If a future real backend cannot restore, the conformance contract should make unsupported restore explicit rather than silently passing.
- "Current screen state" means the public `TerminalScreenState` currently exposed by core: dimensions, plain text, title, cwd, mode flags, and color profile. It does not imply renderer-specific cell inspection.
- Snapshot-before-live-output semantics can be tested with `InitialSnapshotBarrier` from the actor contract without involving a concrete PTY runtime.
- Unknown for implementation: whether the conformance API should expose one function or a small assertion set. Prefer a small assertion set if it makes unsupported restore behavior and barrier behavior clearer.
- No human question is needed; the ticket explicitly says no Ghostty implementation is required and confines the work to core shadow terminal state.

## Affected Surfaces / Files

Expected:

- `crates/botster-core-test-support/src/conformance/mod.rs`
  - Add reusable terminal backend conformance assertions or re-export a focused submodule from here.
- `crates/botster-core-test-support/src/fake/terminal_screen.rs`
  - Extend the fake only as needed for the contract: seeded state, restore support signaling, or observation access.
- `crates/botster-core-test-support/tests/downstream_conformance_test.rs`
  - Add downstream-style tests proving consumers can run the new assertions against `FakeTerminalScreenRuntime`.
- `crates/botster-core/tests/terminal_screen_contract_test.rs`
  - Add or adjust core tests only if the conformance assertions expose a missing invariant in the existing core boundary.

Possibly touched:

- `crates/botster-core-test-support/src/lib.rs`
  - Re-export a new conformance submodule if implementation splits terminal backend assertions out of `conformance/mod.rs`.
- `crates/botster-core/src/contract/terminal_screen.rs`
  - Only if implementation needs a narrow public unsupported-restore marker or helper type. Prefer avoiding this.
- `docs/plans/terminal-backend-shadow-state-conformance-contract.md`
  - This plan artifact.

Not expected:

- `Cargo.toml` files.
- `crates/botster-core/src/engine/terminal_screen.rs`, except if a missing existing invariant requires a narrow fix.
- Hub, CLI/TUI, browser, Rails, Lua plugin, WebRTC, or Ghostty files.

## Implementation Shape

- Add reusable assertions with names close to the behavior, for example:
  - `assert_terminal_backend_preserves_output_and_screen_state`
  - `assert_terminal_backend_snapshot_round_trips_opaque_state`
  - `assert_terminal_backend_resize_survives_snapshot_restore`
  - `assert_initial_snapshot_precedes_live_output`
- Keep the assertions generic over `TerminalScreenRuntime` where possible, constructing `TerminalScreenEngine<R>` in the helper so downstream backends are tested through the same public engine path as the fake.
- Use binary test bytes containing non-UTF-8 data for output/snapshot assertions.
- Use distinct pre- and post-resize dimensions so tests prove state changed and was not only defaulted.
- For snapshot-before-live-output, drive `InitialSnapshotBarrier` with live output before snapshot release and assert event order is initial snapshot first, then held live output.
- Make unsupported restore explicit if needed. Do not let a backend that drops restore state pass the restore conformance assertion.

Production path proof for this ticket is intentionally scaffold-level: the runtime path is the public `TerminalScreenEngine<TerminalScreenRuntime>` and `InitialSnapshotBarrier`, with the fake backend as the executable proof. Ghostty wiring remains a later implementation.

## Risks

- A conformance helper that only checks struct construction would not prove the runtime path; assertions must drive engine methods.
- Snapshot tests using only UTF-8 strings could miss binary snapshot corruption.
- Folding client rendering concerns into the helper would violate the ticket boundary and make Ghostty/restty decisions prematurely.
- Adding a second backend trait could split the terminal backend contract from the already-exported `TerminalScreenRuntime`.
- If restore support is optional, an overly permissive API could hide backend limitations. The contract should make restore support or non-support visible.
- Existing `cargo clippy --all-targets --all-features -- -D warnings` may surface warnings outside the touched files; implementation evidence should attribute any pre-existing failures exactly.

## Acceptance Checks / Tests

Required focused checks:

- Fake backend tests run the reusable conformance assertions from `botster-core-test-support`.
- Output conformance proves raw PTY bytes are preserved and visible in current screen state.
- Resize conformance proves dimensions update and remain correct through snapshot capture/restore.
- Snapshot conformance proves opaque bytes, size, and format label round-trip through capture and replay.
- Restore conformance proves a fresh or mutated backend can restore prior snapshot state when supported.
- Initial snapshot barrier conformance proves live output recorded before snapshot release is emitted after the initial snapshot.
- Tests confirm the contract remains core shadow terminal state only, not renderer policy or Ghostty implementation.

Verification commands:

- `cargo test -p botster-core-test-support terminal`
- `cargo test -p botster-core terminal_screen`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Vault Gaps Worth Capturing

- No new durable vault capture is required yet. The loaded vault notes already cover the main constraints: terminal data-plane ownership, snapshot-before-live-output ordering, opaque snapshot handling, and Project Pipelines plan artifact discipline.
- Capture later if implementation reveals a reusable distinction between "snapshot-capable" and "snapshot-restore-capable" terminal backends, because that would be a durable terminal backend contract nuance.

## Checklist Evidence

- Vault/project notes loaded: listed in Context Loaded.
- Convention conflicts: none. The plan keeps core reusable, avoids service-style abstractions, avoids Ghostty dependency sprawl, follows Rust core/test-support boundaries, and creates a repo-visible plan artifact.
- Verification evidence planned: listed in Acceptance Checks / Tests.
- Durable knowledge capture: none now; possible future gap listed above.
