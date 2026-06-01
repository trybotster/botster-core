# Terminal Backend Shadow State Conformance Contract Plan

Ticket: Add terminal backend conformance contract for shadow state
Run: run_1780282400_744091

## Context Loaded

- Pipeline context: ticket `ticket_1780282384_356500`, current step `botster_plan`, gate `botster_plan_gate`; Plan Review returned changes required with five findings covering module placement, existing coverage delta, restore ambiguity, barrier snapshot terminology, and negative checks.
- Required playbooks: [[planner-playbook]], [[botster-planner-playbook]].
- Additional vault/project constraints: [[identity]], [[goals]], [[botster-architecture]], [[cli-patterns]], [[spa-patterns]], [[project pipeline orchestration belongs in a device-level botster plugin]], [[project pipelines needs an operator workbench not more primitives]], [[project pipelines ui contract belongs in the plugin readme]], [[botster orchestration should spawn agents with explicit target ids]], [[botster orchestration prompts must bind agents to explicit worktrees]], [[plan steps need reviewable plan artifacts]].
- Repo context inspected: `Cargo.toml`, `crates/botster-core/Cargo.toml`, `crates/botster-core-test-support/Cargo.toml`, `crates/botster-core/src/contract/terminal_screen.rs`, `crates/botster-core/src/engine/terminal_screen.rs`, `crates/botster-core-test-support/src/assertions/mod.rs`, `crates/botster-core-test-support/src/fake/terminal_screen.rs`, `crates/botster-core-test-support/src/conformance/mod.rs`, `crates/botster-core-test-support/src/lib.rs`, `crates/botster-core/tests/terminal_screen_contract_test.rs`, `crates/botster-core-test-support/tests/downstream_conformance_test.rs`, and prior plan `docs/plans/terminal-screen-snapshot-boundary.md`.
- Botster layers touched: Rust core contract/engine and Rust test-support only. This is core shadow terminal state, not hub, browser, TUI, Rails, Lua plugin, or rendering work.
- Worktree/target assumptions: this plan is for the pipeline-provided worktree and explicit target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`; downstream agents should keep using the assigned worktree, not ambient checkouts.

## Scope

Define reusable conformance assertions in the always-available `botster-core-test-support::assertions` module for terminal shadow-state backends. The contract should prove that any backend adapter can:

- Maintain screen dimensions across output, resize, snapshot, and restore operations.
- Read current screen state through the public terminal screen state shape.
- Produce opaque snapshots that preserve backend-owned bytes, dimensions, and optional format labels.
- Replay/restore snapshot state through the infallible `TerminalScreenRuntime::replay_snapshot` seam.
- Preserve resize behavior before and after snapshot capture/restore.
- Preserve snapshot-before-live-output semantics by proving held live output is emitted only after the initial snapshot barrier is released.

Existing coverage already proves raw terminal output bytes round-trip through public protocol/transport contracts via `assert_terminal_output_round_trips`, and core tests already prove basic `TerminalScreenEngine<FakeTerminalScreenRuntime>` output and binary snapshot capture/replay. The net-new delta is to move these backend-specific guarantees into reusable assertions, add resize-survives-restore and format-label restore coverage, add current-screen-state conformance, add actor barrier ordering, and add a negative backend test proving at least one assertion rejects non-conforming behavior.

The smallest shape is a set of assertions in `crates/botster-core-test-support/src/assertions/mod.rs` beside `assert_terminal_output_round_trips`. Do not put the new reusable fake-backed assertions in `conformance/mod.rs`, because that module is `local-runtime` feature-gated and built around `ManagedSessionRuntime<LocalProcessRuntime>`.

## Non-Scope

- No Ghostty, libghostty-vt, restty, WASM, or vendored terminal backend implementation.
- No client rendering assertions, browser/TUI behavior, React/Catalyst UI, or terminal cell painting policy.
- No new terminal protocol frame family unless compiler pressure proves an existing public type cannot carry the contract.
- No new runtime dependency in `botster-core` or `botster-core-test-support`.
- No broad refactor of `SessionWorkerEngine`, `ManagedSessionRuntime`, transport routing, or the existing terminal screen boundary.
- No new restore capability flag or unsupported-restore contract in this ticket.
- No PII-bearing fixtures or local path capture in tests or docs.

## Assumptions And Unknowns

- Existing `TerminalScreenRuntime` already names the backend seam. The new ticket should add reusable conformance assertions around that seam rather than introduce a parallel backend trait.
- Existing `TerminalSnapshotPayload` is the right opaque snapshot carrier for core shadow state; snapshots stay byte payloads plus dimensions and optional host-owned format label.
- Restore is mandatory for the current `TerminalScreenRuntime` seam because `replay_snapshot(&mut self, payload) -> ()` is infallible and has no capability signal. This plan treats "when supported" as satisfied by the existing trait: any backend claiming to implement this seam must restore snapshots correctly. Any future optional restore-capability signaling belongs to the real Ghostty/backend integration ticket, not this conformance assertion slice.
- "Current screen state" means the public `TerminalScreenState` currently exposed by core: dimensions, plain text, title, cwd, mode flags, and color profile. It does not imply renderer-specific cell inspection.
- Snapshot-before-live-output semantics are separate from `TerminalScreenEngine` opaque snapshots. They should be tested with the actor-contract `InitialSnapshotBarrier` and `SessionIoEvent` ordering, without wiring it to `TerminalSnapshotPayload`.
- Unknown for implementation: whether the conformance API should expose one function or a small assertion set. Prefer a small assertion set if it preserves the boundary between terminal backend shadow-state assertions and actor barrier ordering.
- No human question is needed; the ticket explicitly says no Ghostty implementation is required and confines the work to core shadow terminal state.

## Affected Surfaces / Files

Expected:

- `crates/botster-core-test-support/src/assertions/mod.rs`
  - Add reusable, always-available terminal backend shadow-state assertions beside `assert_terminal_output_round_trips`.
- `crates/botster-core-test-support/src/fake/terminal_screen.rs`
  - Extend the fake only as needed for the contract: seeded state or observation access. Do not add unsupported-restore capability signaling.
- `crates/botster-core-test-support/tests/downstream_conformance_test.rs`
  - Add downstream-style tests proving consumers can run the new assertions against `FakeTerminalScreenRuntime`.
  - Add at least one deliberately broken test backend and assert the relevant reusable assertion panics or otherwise fails.
- `crates/botster-core/tests/terminal_screen_contract_test.rs`
  - Add or adjust core tests only if the conformance assertions expose a missing invariant in the existing core boundary.

Possibly touched:

- `crates/botster-core/src/contract/terminal_screen.rs`
  - Only if implementation exposes a genuine missing helper in current public types. Do not add restore capability markers for this ticket.
- `docs/plans/terminal-backend-shadow-state-conformance-contract.md`
  - This plan artifact.

Not expected:

- `Cargo.toml` files.
- `crates/botster-core/src/engine/terminal_screen.rs`, except if a missing existing invariant requires a narrow fix.
- `crates/botster-core-test-support/src/conformance/mod.rs`, because it is feature-gated behind `local-runtime` and reserved for managed local PTY runtime helpers.
- Hub, CLI/TUI, browser, Rails, Lua plugin, WebRTC, or Ghostty files.

## Implementation Shape

- Add reusable assertions with names close to the behavior, for example:
  - `assert_terminal_backend_snapshot_round_trips_opaque_state`
  - `assert_terminal_backend_resize_survives_snapshot_restore`
  - `assert_terminal_backend_screen_state_matches_output_and_metadata`
  - `assert_initial_snapshot_precedes_live_output`
- Keep the assertions generic over `TerminalScreenRuntime` where possible, constructing `TerminalScreenEngine<R>` in the helper so downstream backends are tested through the same public engine path as the fake.
- Use binary test bytes containing non-UTF-8 data for output/snapshot assertions.
- Use distinct pre- and post-resize dimensions so tests prove state changed and was not only defaulted.
- For snapshot-before-live-output, drive `InitialSnapshotBarrier` with live output before snapshot release and assert `SessionIoEvent::InitialSnapshotReady` is emitted before held `SessionIoEvent::TerminalBytes`. This assertion is actor-contract ordering, not a `TerminalSnapshotPayload` backend assertion.
- Treat restore as unconditional for `TerminalScreenRuntime` implementers. A backend that drops restore state or loses dimensions must fail the restore conformance assertion.
- Reuse or build on existing `assert_terminal_output_round_trips` for public output-frame round-trip coverage instead of duplicating that protocol/transport assertion under a new name.

Production path proof for this ticket is intentionally scaffold-level: the runtime path is the public `TerminalScreenEngine<TerminalScreenRuntime>` and `InitialSnapshotBarrier`, with the fake backend as the executable proof. Ghostty wiring remains a later implementation.

## Risks

- A conformance helper that only checks struct construction would not prove the runtime path; assertions must drive engine methods.
- Snapshot tests using only UTF-8 strings could miss binary snapshot corruption.
- Folding client rendering concerns into the helper would violate the ticket boundary and make Ghostty/restty decisions prematurely.
- Adding a second backend trait could split the terminal backend contract from the already-exported `TerminalScreenRuntime`.
- Treating restore as optional in this ticket would require a core contract extension that is outside scope; implementation must assert restore unconditionally for `TerminalScreenRuntime`.
- Without a negative backend test, the conformance suite could pass even if an assertion silently checks the wrong behavior.
- Conflating actor initial snapshots with backend opaque snapshots could wire together independent surfaces and obscure the intended runtime path.
- Existing `cargo clippy --all-targets --all-features -- -D warnings` may surface warnings outside the touched files; implementation evidence should attribute any pre-existing failures exactly.

## Acceptance Checks / Tests

Required focused checks:

- Fake backend tests run the reusable conformance assertions from `botster-core-test-support`.
- Existing `assert_terminal_output_round_trips` remains the public protocol/transport output assertion; new backend assertions should not duplicate it unnecessarily.
- Resize conformance proves dimensions update and remain correct through snapshot capture/restore.
- Snapshot conformance proves opaque bytes, size, and format label round-trip through capture and replay.
- Restore conformance proves a fresh or mutated backend restores prior snapshot state unconditionally through `TerminalScreenRuntime::replay_snapshot`.
- Screen-state conformance proves output and metadata are readable from `TerminalScreenState`.
- Initial snapshot barrier conformance proves live output recorded before snapshot release is emitted after the initial snapshot, using `InitialSnapshotBarrier` and `SessionIoEvent` rather than `TerminalSnapshotPayload`.
- A deliberately broken backend test proves at least one reusable assertion fails when a backend drops bytes, loses dimensions, or fails restore.
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

## Plan Review Findings Addressed

- `finding_1780282924_281581`: resolved by moving expected reusable assertion work from feature-gated `conformance/mod.rs` to always-available `assertions/mod.rs`.
- `finding_1780282924_374770`: resolved by stating existing coverage and narrowing the net-new delta to reusable backend assertions, resize-survives-restore, format-label restore, screen-state reads, barrier ordering, and negative checks.
- `finding_1780282924_189635`: resolved by treating restore as mandatory for current `TerminalScreenRuntime` implementers and deferring optional restore-capability signaling to future real-backend work.
- `finding_1780282924_458903`: resolved by separating actor `InitialSnapshotBarrier` ordering from `TerminalScreenEngine` opaque snapshot assertions.
- `finding_1780282924_935035`: resolved by adding a required broken-backend counterexample acceptance check.
