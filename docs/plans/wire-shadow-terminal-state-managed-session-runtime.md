# Wire Shadow Terminal State Into Managed Session Runtime

Ticket: `ticket_1780282384_868731`
Run: `run_1780286324_228030`

## Context Loaded

- Pipeline context loaded with `project_pipelines_current_context`: ticket `Wire shadow-terminal state into core managed session path`, run `run_1780286324_228030`, current step `botster_plan`, gate `botster_plan_gate`.
- Plan review return loaded: `review_1780287674_939890` returned `changes_required` with six open findings. This revision resolves them by tightening initial-snapshot mechanics, request-kind scope, existing-test rewrites, source-vocabulary constraints, and acceptance proof coverage.
- Inbox correction loaded with `receive_messages`: treat this run as main-rooted with `base_ref=main`, `base_run_id=null`, and `base_ticket_id=null`; do not create a stacked PR or branch from dependency work.
- Dependencies loaded from context and treated as already closed:
  - `Define Ghostty shadow-terminal architecture in botster-core`
  - `Add terminal backend conformance contract for shadow state`
- Required playbooks loaded:
  - `planner-playbook`
  - `botster-planner-playbook`
- Required Botster/vault context loaded:
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
- Repo context inspected:
  - `crates/botster-core/src/engine/managed_session_runtime.rs`
  - `crates/botster-core/src/engine/session_worker.rs`
  - `crates/botster-core/src/engine/multiplexer.rs`
  - `crates/botster-core/src/engine/terminal_screen.rs`
  - `crates/botster-core/src/contract/terminal_screen.rs`
  - `crates/botster-core/src/runtime/local_process.rs`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/tests/managed_session_runtime_test.rs`
  - `crates/botster-core/tests/terminal_screen_contract_test.rs`
  - `crates/botster-core/tests/session_worker_engine_test.rs`
  - `crates/botster-core-test-support/src/fake/terminal_screen.rs`
  - `crates/botster-core-test-support/src/fake/session_worker.rs`
  - Prior plans: `terminal-screen-snapshot-boundary.md`, `supervised-session-task-runtime-core-engine.md`, and `default-local-pty-process-runtime.md`
- Project Pipelines checklist: `checklist_1780287329_625820` was created after an initial lock/timeout and all four vault workflow items were marked done. Plan Review also created `checklist_1780287610_671168`.

## Scope

Wire the existing terminal screen boundary into the core managed session runtime so PTY output has one core-owned shadow terminal state in the managed path.

In scope:

- Make `ManagedSessionRuntime` own or construct a terminal screen backend per managed session.
- Feed every drained `SessionRuntimeOutput::PtyOutput` into that backend before emitting the existing `TerminalBytes` fanout event.
- Keep live output fanout unchanged: subscribers still receive the exact PTY bytes through the existing `SessionWorkerEngine` and `SubscriptionMultiplexer` path.
- Serve snapshot, initial snapshot, and plain screen requests from the core-owned shadow terminal backend instead of rejecting them as unsupported.
- Keep `GetModeFlags` and `SetColorProfile` explicitly unsupported in this ticket unless implementation also adds managed-runtime round-trip tests proving those paths read/write real shadow state. Do not move a request kind from explicit error to silent default data.
- Preserve reconnect and snapshot-before-live-output semantics by continuing to use `SessionWorkerEngine`'s initial snapshot barrier and pending-output flush behavior.
- Add or promote a minimal core-owned terminal screen runtime if needed, using opaque bytes and plain text only. It can be fake-quality while Ghostty bindings are under audit, but it must live on the actual managed session path.
- Keep Ghostty as the future concrete backend direction and keep restty out of core.
- Add focused tests proving the public runtime path changed, not just that terminal screen types exist.
- Update docs only if needed to state the runtime ownership boundary precisely.

Non-scope:

- No Ghostty bindings, libghostty-vt build work, submodule initialization, or backend audit work.
- No restty dependency, client renderer coupling, browser/TUI/Rails/WebRTC/plugin work, or Project Pipelines UI work.
- No full terminal emulator, scrollback/grid fidelity, hyperlinks, graphics, or renderer-specific policy in `botster-core`.
- No replacement of `SessionWorkerEngine`, `SubscriptionMultiplexer`, `TransportIngress`, or existing session protocol frames.
- No new product workflow policy, target admission, reconnect policy, persistence, auth, or cloud behavior.
- No mode/color profile support unless it is backed by real shadow state and round-trip tests in the managed runtime path.
- No stacked branch/PR behavior; this run is main-rooted.

Botster layers touched:

- Rust `botster-core` managed runtime and terminal screen engine: primary.
- Rust `botster-core` tests and possibly `botster-core-test-support` fakes.
- Docs plan artifact and optional README note only.
- No plugin, Lua core, Rust hub, TUI, React SPA, Rails relay, or MCP surface.

Worktree/target assumption: implementation agents work in this assigned Project Pipelines worktree for target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, targeting `main`.

Pipeline gates/artifacts: this file is the repo-visible Plan artifact. Gate evidence should cite this file, the inbox correction, and checklist write-lock status.

## Assumptions And Unknowns

Assumptions:

- The intended production entry point is `ManagedSessionRuntime::{drain_runtime_once, handle_client_ingress, handle_session_request}`, not a standalone terminal screen demo.
- `SessionWorkerEngine` remains the ordering owner for initial snapshots. The managed runtime should provide authoritative state to the worker, not bypass the worker with a second snapshot path.
- `TerminalScreenEngine` and `TerminalScreenRuntime` are the accepted seam from the prior dependency ticket.
- Snapshot payload bytes stay opaque. Any minimal backend can preserve bytes and dimensions without claiming Ghostty-level fidelity.
- A small in-core minimal backend is acceptable if the existing fake backend lives only in `botster-core-test-support` and cannot be used by `botster-core` itself.
- Existing `SnapshotReady`, `InitialSnapshotReady`, `ScreenReady`, `ModeFlagsReady`, and `PreparedSnapshotReady` carriers should be reused.
- `LocalProcessWorkerRuntime` can remain a separate lower-level adapter unless implementation proves it must share the same backend; this ticket is specifically the managed session runtime path.
- No client renderer dependency is allowed in core. Browser restty may render later, but cannot own the authoritative state.

Unknowns for implementation:

- Exact generic/API shape: either parameterize `ManagedSessionRuntime<R, T>` over a terminal backend factory or keep its public type stable and use an internal minimal backend by default.
- Whether the minimal backend belongs in `engine/terminal_screen.rs` as a public `PlainTerminalScreenRuntime` or as a private adapter in `managed_session_runtime.rs`. Prefer the smallest public surface that tests and embedders need.
- Whether the minimal backend needs any public constructor/factory shape beyond the default managed runtime constructor.
- Whether `PrepareSnapshot` should continue to pass through the request bytes unchanged or round-trip through the backend. Acceptance is about screen/snapshot requests reading core-owned state; prepared payload storage can remain pass-through unless tests show it is part of snapshot read semantics.

No human question is blocking. The ticket allows a fake/minimal backend and explicitly forbids client renderer/restty dependency.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/engine/managed_session_runtime.rs`
  - Replace unsupported snapshot/screen/initial-snapshot paths with terminal-backend-backed behavior.
  - Feed drained PTY output into the terminal backend before fanout.
  - Preserve runtime input flushing for writes, resize, and shutdown.
  - Keep `GetModeFlags` and `SetColorProfile` rejected unless backed and tested in this same ticket.
- `crates/botster-core/src/engine/terminal_screen.rs`
  - Possible minimal backend type if the managed runtime needs an in-core default.
- `crates/botster-core/src/contract/terminal_screen.rs`
  - Avoid changes unless the minimal backend exposes a real missing contract.
- `crates/botster-core/src/engine/mod.rs` and `crates/botster-core/src/lib.rs`
  - Export a new minimal backend only if it is public API.
- `crates/botster-core/tests/managed_session_runtime_test.rs`
  - Main acceptance tests for PTY output updating shadow state, snapshot/screen reads, live fanout preservation, and reconnect/initial snapshot ordering.
  - Rewrite the existing `supervised_session_resize_forwarding_reaches_runtime_before_snapshot` assertion that currently expects `RequestSnapshot` to return `UnsupportedSessionRequest`. The resize-before-snapshot intent moves to the new resize/shadow snapshot test.
  - Extend the existing `supervised_session_contract_excludes_concrete_transport_and_product_policy` source guard instead of adding a duplicate guard test.
- `crates/botster-core/tests/terminal_screen_contract_test.rs`
  - Extend only if a new backend belongs with terminal screen contract coverage.
- `crates/botster-core-test-support/src/fake/terminal_screen.rs`
  - Touch only if test support needs a factory or conformance helper for the managed path.
- `docs/plans/wire-shadow-terminal-state-managed-session-runtime.md`
  - This plan artifact.

Possible but avoid unless implementation proves it necessary:

- `crates/botster-core/src/runtime/local_process.rs`
  - Only if the default local worker adapter must share the new backend immediately. The ticket target is the managed runtime path, so this should not be first choice.
- `README.md`
  - Narrow boundary wording only.

Not expected:

- `Cargo.toml`; no new dependency should be needed.
- Hub, browser, TUI, Rails, Lua plugin, MCP, provider, cloud, or old TryBotster files.

## Implementation Shape

Suggested minimal path:

1. Introduce an in-core minimal terminal screen runtime if needed.
   - Store `TerminalScreenSize`, raw bytes, plain text, mode flags, optional color profile, title, cwd, and optional opaque format label.
   - `write_output` appends exact PTY bytes and updates the plain text with UTF-8-lossy text.
   - `capture_snapshot` returns byte-identical raw state plus current dimensions.
   - `screen_state` returns the plain text and synced state.

2. Update `SessionRuntimeWorkerAdapter`.
   - Add a `TerminalScreenEngine<...>` field beside current pending `SessionRuntimeInput` state.
   - `write_input` remains runtime input only; do not feed user input into shadow output state.
   - `resize` updates both runtime input and shadow terminal dimensions synchronously.
   - Add a method such as `record_output(&mut self, data: &[u8])` used by `ManagedSessionRuntime::drain_runtime_once` before routing `TerminalBytes`.
   - `snapshot` and `screen` read from the terminal backend.
   - `request_initial_snapshot` must capture the backend snapshot and record a pending `SessionWorkerRuntimeEvent::InitialSnapshotReady`. It cannot return the event directly because `SessionWorkerRuntime::request_initial_snapshot` returns `()`.
   - `GetModeFlags` and `SetColorProfile` remain unsupported unless this adapter stores real mode/color state and the managed-runtime tests prove round trips for those request kinds.

3. Update `ManagedSessionRuntime::drain_runtime_once`.
   - On `PtyOutput`, first mutate the session's worker adapter shadow backend, then call `handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes { ... })`.
   - On `ProcessExited`, preserve current ordering.
   - Drain any pending `InitialSnapshotReady` event recorded by the worker adapter and route it through `engine.handle_runtime_event`, mirroring the existing PTY output path. This preserves `SessionWorkerEngine` as the owner of the initial-snapshot barrier and live-output release ordering.

4. Remove overly broad unsupported rejections.
   - `TransportIngress::RequestSnapshot` should be allowed.
   - `SessionIoRequest::{SubscribeTerminal, GetSnapshot, GetInitialSnapshot, GetScreen}` should be allowed.
   - `GetModeFlags` and `SetColorProfile` should stay unsupported unless implemented with explicit managed-runtime round-trip tests.
   - Keep unsupported only for genuinely unavailable managed-runtime operations, such as send-file if it remains out of scope.

5. Preserve all existing public client egress contracts.
   - `TerminalOutput` egress still carries original PTY bytes.
   - `SnapshotReady`, `InitialSnapshotReady`, and `ScreenReady` continue to flow through `SessionIoEvent` and multiplexer outcomes. `ModeFlagsReady` remains out of this ticket unless explicitly backed and tested.

## Risks

- Feeding shadow state after fanout would make immediate snapshot reads stale. Tests should assert output is visible to snapshots/screens after a drain.
- Delivering initial snapshots outside `SessionWorkerEngine` could break the snapshot-before-live-output barrier. Keep barrier ownership in the worker.
- Treating terminal input as terminal output would pollute screen state. Only PTY output updates shadow state.
- A fake/minimal backend can overpromise fidelity. Docs and type names should make clear this is a minimal opaque/plain backend while Ghostty remains the concrete backend direction.
- Adding restty, browser, TUI, or renderer vocabulary to core would violate architecture constraints.
- Generic type churn in `ManagedSessionRuntime` could create avoidable public API breakage. Prefer stable constructors or default type parameters if a generic backend is exposed.
- Snapshot byte semantics can drift if there are two representations. Reuse `TerminalSnapshotPayload` conversion helpers and existing actor carriers.
- Existing source guard tests reject some product/renderer terms. New names and docs in core should remain transport-neutral.
- `crates/botster-core/tests/managed_session_runtime_test.rs::supervised_session_contract_excludes_concrete_transport_and_product_policy` already forbids literal source terms including `reconnect`, `browser`, `TUI`, `Rails`, and `cloud` in `managed_session_runtime.rs`. Implement the behavior through snapshot/barrier mechanics without adding those words to that source file. Extend this existing guard for `restty` and concrete backend names instead of adding a duplicate guard.
- Existing tests lock in the old unsupported snapshot behavior. The implementation must intentionally rewrite/delete unsupported-error assertions for any request kind that becomes supported.
- Project Pipelines SQLite writes may remain locked. Gate evidence must preserve the plan/checklist attempt even if checklist persistence is unavailable.

## Acceptance Checks / Tests

Add targeted tests in `crates/botster-core/tests/managed_session_runtime_test.rs`:

1. `supervised_session_pty_output_updates_shadow_terminal_snapshot`
   - Spawn managed fake runtime, emit PTY output, drain once, request snapshot, and assert the snapshot bytes come from the shadow terminal state.

2. `supervised_session_screen_read_uses_core_shadow_terminal_state`
   - Emit PTY output, drain once, request `GetScreen`, and assert `ScreenReady.text` includes the output.

3. `supervised_session_live_output_fanout_still_emits_original_bytes`
   - Preserve the existing fanout assertion and also assert snapshot state was updated from the same output.

4. `supervised_session_request_snapshot_is_no_longer_unsupported`
   - Route `TransportIngress::RequestSnapshot` through `handle_client_ingress` and assert a `SnapshotReady` session event or client egress appears instead of `UnsupportedSessionRequest`.

5. `supervised_session_initial_snapshot_precedes_live_output_from_shadow_state`
   - Subscribe/request initial snapshot, emit live output before the initial snapshot is delivered, and assert `InitialSnapshotReady` precedes `TerminalOutput`.

6. `supervised_session_initial_snapshot_after_prior_output_reflects_shadow_state`
   - Emit PTY output, drain once so shadow state accumulates it, then issue `SubscribeTerminal` or `GetInitialSnapshot`.
   - Assert `InitialSnapshotReady.snapshot` reflects the previously accumulated core-owned state.
   - Also assert any live output arriving while the barrier is held is flushed after the initial snapshot.

7. `supervised_session_resize_updates_runtime_and_shadow_before_snapshot`
   - Resize, request snapshot, assert runtime input includes resize and snapshot dimensions match the resized shadow state.
   - Replace the old unsupported-error half of `supervised_session_resize_forwarding_reaches_runtime_before_snapshot`; do not leave contradictory assertions behind.

8. `supervised_session_mode_and_color_paths_stay_explicitly_unsupported_or_are_backed`
   - If `GetModeFlags` and `SetColorProfile` stay out of scope, assert they still return `UnsupportedSessionRequest` rather than silent default data.
   - If implementation chooses to support either request kind, replace this with managed-runtime round-trip tests proving that path reads/writes real shadow state.

9. Existing source guard extension
   - Extend `supervised_session_contract_excludes_concrete_transport_and_product_policy` to reject `restty` and concrete backend imports/names. Do not add a second redundant source guard.

Verification commands:

- `cargo fmt`
- `cargo test -p botster-core managed_session_runtime`
- `cargo test -p botster-core terminal_screen`
- `cargo test -p botster-core session_worker`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Runtime-path proof required for implementation gate:

- Show that `ManagedSessionRuntime::drain_runtime_once` mutates the shadow backend before routing terminal bytes.
- Show that `handle_client_ingress(RequestSnapshot)` or `handle_session_request(GetScreen/GetSnapshot/GetInitialSnapshot)` returns state produced by that backend.
- Show that the initial snapshot path records a pending `InitialSnapshotReady` and routes it through `engine.handle_runtime_event`, so `SessionWorkerEngine` owns the barrier release.
- Show that `GetModeFlags` and `SetColorProfile` either remain explicit unsupported errors or have managed-runtime round-trip proof; no silent defaults.
- Show live `TerminalOutput` egress remains byte-identical to the original PTY output.

## Vault Gaps Worth Capturing

Capture after implementation if confirmed:

- `ManagedSessionRuntime` owns the authoritative core shadow terminal for the managed session path; PTY output must update that backend before fanout, and snapshots/screens must read from it.

Potential update to an existing note:

- Extend `sessionioworker is the production read path for session pty output` or the terminal screen boundary note to name the managed-runtime shadow-state ordering rule.

No convention conflict was found while planning. The plan follows the loaded Botster constraints: session/client actor data plane remains authoritative, restty stays renderer-only, Ghostty remains future backend direction outside this slice, and Project Pipelines state remains plugin-owned.
