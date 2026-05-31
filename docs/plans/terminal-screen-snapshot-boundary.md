# Terminal Screen And Snapshot Boundary Plan

Ticket: Prepare terminal screen and snapshot integration boundary
Run: run_1780189447_449024

## Context Loaded

- Pipeline context: ticket `ticket_1780189421_191212`, run `run_1780189447_449024`, current step `botster_plan`, gate `botster_plan_gate`, no prior artifacts/findings/questions/answers.
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
  - [[synced state types are allowed while pushed event variants are forbidden]]
  - [[session io owns paste and snapshot helpers before mailbox wiring]]
- Repo context inspected:
  - `README.md`
  - `Cargo.toml`
  - `docs/plans/session-process-wire-protocol.md`
  - `docs/plans/client-stream-contract.md`
  - `crates/botster-core/src/lib.rs`
  - `crates/botster-core/src/contract/session_protocol.rs`
  - `crates/botster-core/src/contract/actor.rs`
  - `crates/botster-core/src/contract/transport.rs`
  - `crates/botster-core/src/contract/client_stream.rs`
  - `crates/botster-core/src/engine/session_worker.rs`
  - `crates/botster-core-test-support/src/fake/session_worker.rs`
  - `crates/botster-core/tests/session_protocol_test.rs`
  - `crates/botster-core/tests/session_worker_engine_test.rs`
  - `crates/botster-core/tests/client_stream_contract_test.rs`

## Scope

Define a narrow reusable terminal screen/snapshot boundary in `botster-core` and prove it with tests plus a fake/lightweight implementation path. The boundary should be host-embeddable and renderer-neutral: it can normalize PTY output, maintain minimal screen state, capture/replay opaque snapshot payloads, and surface screen-state hooks without deciding how the hub, browser, TUI, Ghostty, or restty render terminals.

In scope:

- Add a terminal screen contract module, likely `crates/botster-core/src/contract/terminal_screen.rs`.
- Add a small pure engine module, likely `crates/botster-core/src/engine/terminal_screen.rs`, that drives the public contract with a host-supplied runtime trait or in-memory fake.
- Preserve opaque snapshot round-trip behavior by wrapping bytes with typed metadata only; core must never parse or reinterpret Ghostty/restty snapshot internals.
- Define terminal output normalization as stable event/input shapes, not a full emulator. Examples: raw PTY bytes in, normalized output chunks/events out, dimensions, and synced screen state where existing session protocol state already names title/cwd/mode.
- Define screen state hooks that a host parser can call after output, resize, snapshot capture, and snapshot replay. Title, cwd, and mode/color state must be modeled as synced screen-state fields, not pushed change events, per `synced state types are allowed while pushed event variants are forbidden`.
- Add `botster-core-test-support` fake helpers for consumers to prove conformance without Ghostty/restty.
- Add focused tests that exercise the exported engine/contract path, not just type construction.
- Update README boundary text or a repo doc only enough to state why Ghostty/restty are optional host/future dependencies rather than core dependencies.

Non-scope:

- No broad terminal emulator rewrite.
- No Ghostty/restty vendoring, build scripts, submodule setup, WASM, or native parser dependency in this ticket.
- No browser, hub, TUI, React/Catalyst, WebRTC, Rails, ActionCable, or rendering policy.
- No direct snapshot helper that bypasses session/client-worker ownership, such as `snapshot_and_subscribe`.
- No compatibility branch for old VT snapshot formats. Opaque payloads round-trip; schema interpretation remains outside core.
- No product-specific terminal workflow policy or Project Pipelines UI work.

Botster layers touched: Rust core contracts, Rust core pure engine, test-support fakes, and docs. Hub/client/plugin layers are referenced as consumers only.

## Assumptions And Unknowns

Assumptions:

- `botster-core` is intentionally a reusable contract/engine crate; production integration into the full TryBotster hub/CLI is a later consumer ticket.
- Existing `SessionIoRequest::{GetSnapshot, PrepareSnapshot, GetScreen}` and `SessionIoEvent::{SnapshotReady, PreparedSnapshotReady, ScreenReady}` remain valid and should be reused rather than replaced.
- Existing session protocol frames (`FRAME_SNAPSHOT`, `FRAME_GET_SCREEN`, `FRAME_SCREEN`, mode/title/cwd frames) preserve the wire protocol compatibility surface.
- Snapshot payloads are opaque bytes. Compatibility means byte-identical capture/replay through public contracts, not semantic equality of parsed terminal cells.
- Ghostty/restty may be dependency choices for concrete host implementations because they already own terminal fidelity in TryBotster, but this core boundary should accept them through trait adapters or opaque payloads.
- Minimal screen state can be represented by dimensions, plain-screen text, and synced optional title/cwd/mode/color fields in the fake path; the ticket does not require cursor, scrollback, hyperlinks, or graphics fidelity in core.
- `SnapshotReady` remains the actor/mailbox carrier because it includes `request_id` and `session_id` correlation. Add a new snapshot value type only if implementation needs a reusable correlation-free value for parser/runtime adapters; otherwise reuse `SnapshotReady`.
- The integration seam is the existing `SessionWorkerRuntime` implementation boundary. A host session-runtime adapter can own `TerminalScreenEngine` internally, call it from `write_input`, `resize`, `snapshot`, `prepare_snapshot`, `mode_flags`, and `screen`, then return existing `SessionIoEvent`/`SnapshotReady`/`ScreenReady` shapes through `SessionWorkerEngine`.

Unknowns for implementation to resolve narrowly:

- Exact naming: prefer `terminal_screen` and types like `TerminalOutputChunk`, `TerminalScreenState`, `TerminalScreenHook`, `TerminalScreenEngine`, and `TerminalScreenRuntime` if they fit local style.
- Whether `ScreenReady` should remain plain text only or be accompanied by a new renderer-neutral `TerminalScreenState` struct. If title/cwd/mode/color are represented, they belong on this state struct as synced state, not on hook event variants.
- Whether a correlation-free snapshot value type is justified. If added, it must be explicitly documented as a reusable value converted into existing `SnapshotReady`/`PreparedSnapshotRequest` carriers; if not, reuse existing structs.
- Whether snapshot metadata should carry `rows`/`cols` only or also `format`/`version`. If a format field is added, keep it an opaque label such as `TerminalSnapshotFormat(String)` and avoid enum cases that bake Ghostty/restty policy into core.
- Whether to place capture/replay tests under `terminal_screen_contract_test.rs` or extend `session_worker_engine_test.rs`. Prefer a new focused test file.

No human question is needed before implementation; the ticket explicitly asks for a boundary and a minimal fake/lightweight path, not a production terminal backend.

## Affected Surfaces / Files

Expected:

- `crates/botster-core/src/contract/terminal_screen.rs`
  - Public terminal screen/snapshot contracts.
  - Opaque snapshot payload type that preserves bytes and minimal metadata.
  - Normalized output and hook/event types.
- `crates/botster-core/src/contract/mod.rs`
  - Export the new contract module.
- `crates/botster-core/src/engine/terminal_screen.rs`
  - Pure embeddable engine over a host-supplied runtime/parser trait.
  - Minimal fake-friendly capture/replay and hook dispatch behavior.
- `crates/botster-core/src/engine/mod.rs`
  - Export the new engine module.
- `crates/botster-core/src/lib.rs`
  - Re-export public contract and engine types.
- `crates/botster-core-test-support/src/fake/terminal_screen.rs`
  - Fake parser/runtime for downstream conformance tests.
- `crates/botster-core-test-support/src/fake/mod.rs`
  - Export the fake helper.
- `crates/botster-core/tests/terminal_screen_contract_test.rs`
  - Contract tests for normalization, hooks, capture, replay, and opaque compatibility.
- `crates/botster-core-test-support/tests/downstream_conformance_test.rs`
  - Extend the existing downstream conformance test with a small consumer-facing fake proof if it stays focused.
- `README.md`
  - Add a concise boundary note naming Ghostty/restty as concrete host/future dependencies, not core dependencies.
- `docs/plans/terminal-screen-snapshot-boundary.md`
  - This plan artifact.

Possibly touched only if compiler/tests prove it necessary:

- `crates/botster-core/src/contract/actor.rs`
  - Reuse existing snapshot/screen structs where possible. Add fields only if the new contract needs typed payload metadata and the change is clearly compatible.
- `crates/botster-core/src/contract/session_protocol.rs`
  - Avoid changing frame constants. Add tests only if the new boundary needs to pin existing opaque frame behavior more directly.

Not expected:

- `Cargo.toml`; no new dependency should be needed.
- `crates/botster-core/src/contract/transport.rs`; existing snapshot and terminal output frames already carry opaque bytes through subscriptions.
- Hub, browser, TUI, Rails, or plugin files.

## Implementation Shape

Suggested public contract:

- `TerminalOutputChunk { bytes: Vec<u8> }` or equivalent normalized output input shape. Keep bytes opaque enough that core does not become a parser.
- `TerminalScreenSize { rows: u16, cols: u16 }`.
- Prefer existing `SnapshotReady`, `PreparedSnapshotRequest`, and `PreparedSnapshotReady` for actor/session correlation. Add `TerminalSnapshotPayload { bytes, rows, cols, format }` only as a correlation-free reusable value if the engine needs it; `format` is a host-owned label, not a core enum of parser backends.
- `TerminalScreenState { size, plain_text, title, cwd, mode_flags }` if state assertions need more than plain screen text. These fields are synced parser state, matching `synced state types are allowed while pushed event variants are forbidden`; do not model title/cwd/mode/color as pushed hook variants.
- `TerminalScreenHook` variants only for renderer-neutral lifecycle observations such as output normalized, resized, snapshot captured, snapshot replayed, and screen read. Do not add `ModeChanged`, title-changed, cwd-changed, or color-delta hook/event variants.
- `TerminalScreenRuntime` trait with methods similar to `write_output`, `resize`, `capture_snapshot`, `replay_snapshot`, `read_screen`, and hook callbacks.
- `TerminalScreenEngine<R>` that calls the runtime trait synchronously and returns a deterministic outcome containing hooks, optional snapshot payload, optional screen text/state, and observations.

The fake implementation can be intentionally small: append UTF-8-lossy printable text for screen reads, preserve all raw bytes in normalized chunks, store rows/cols, and replay snapshots by restoring bytes/metadata without parsing them. This is enough to prove the boundary while leaving full terminal fidelity to Ghostty/restty adapters.

Named call-site seam:

- `SessionWorkerEngine` already calls host-provided `SessionWorkerRuntime::snapshot`, `prepare_snapshot`, `mode_flags`, and `screen`.
- The new terminal screen engine belongs inside that host-provided runtime adapter, not beside the data plane as an alternate router. A concrete TryBotster session runtime can back those trait methods with Ghostty/restty later, while `botster-core-test-support` backs them with the fake implementation.
- The session worker continues emitting existing `SessionIoEvent` variants. This keeps actor/mailbox contracts stable and avoids a second snapshot delivery path.

Runtime path proof for this ticket:

- The production-facing path is scaffold-level by design: downstream hosts will call the exported `TerminalScreenEngine`/trait boundary from session runtime adapters.
- Tests must instantiate the exported engine and fake runtime, drive output/resize/capture/replay/screen-read through it, and assert emitted hooks plus byte-identical snapshot round trips.
- Evidence that structs exist is not enough; the tested path must use the same public engine API a host adapter would call.

## Risks

- Pulling Ghostty/restty into `botster-core` would bake a concrete backend and build policy into the reusable contract crate.
- Modeling a full terminal emulator would violate the ticket's "no broad rewrite" constraint and duplicate parser ownership.
- Treating snapshots as parsed core data would break the opaque compatibility contract and force version policy into core.
- Adding a second snapshot value without documenting its relationship to `SnapshotReady` could create two drifting opaque-snapshot representations. Prefer existing carriers unless a reusable correlation-free value is clearly needed and tested for conversion.
- Adding direct snapshot helpers could bypass the established SessionIo/ClientWorker data-plane boundary.
- A fake that only stores strings could accidentally drop arbitrary binary snapshot bytes. Tests must include non-UTF-8 bytes.
- Hook names can become renderer policy or forbidden pushed terminal-mode events. Keep hooks about engine lifecycle only, and keep title/cwd/mode/color as synced state fields.
- Docs could overpromise current Ghostty/restty wiring. State dependency status precisely: concrete host adapters may use them; core does not depend on them in this slice.

## Acceptance Checks / Tests

Targeted tests to add:

1. `normalizes_output_without_losing_raw_bytes`
   - Feed binary PTY bytes through the engine.
   - Assert normalized output preserves the exact byte sequence and emits an output hook.

2. `capture_and_replay_round_trips_opaque_snapshot_bytes`
   - Capture a snapshot containing non-UTF-8 bytes and dimensions.
   - Replay it into a fresh fake runtime.
   - Assert bytes, rows, cols, and optional format label round-trip unchanged.

3. `screen_state_syncs_title_cwd_and_mode_without_pushed_change_events`
   - Drive output plus metadata/mode/color state through the engine or fake runtime.
   - Assert state is readable from `TerminalScreenState` or existing synced state structs.
   - Assert no title/cwd/mode/color pushed hook variants are emitted.

4. `plain_screen_read_uses_fake_runtime_without_terminal_backend_dependency`
   - Feed simple output, request screen text/state, and assert the fake path works without Ghostty/restty.

5. `snapshot_payload_compatibility_matches_existing_session_protocol_shape`
   - Convert or pair the new payload with existing `SnapshotReady`/`PreparedSnapshotRequest`/`PreparedSnapshotReady`.
   - Assert opaque bytes survive through the existing session protocol-oriented structs.

6. `terminal_screen_boundary_does_not_expose_renderer_policy`
   - Add a concrete source-scanning guard test, following `actor_contract_test.rs::terminal_mode_is_not_a_pushed_actor_contract`.
   - Scan `src/contract/terminal_screen.rs` and `src/engine/terminal_screen.rs`.
   - Reject `ModeChanged`, `terminal-mode-delta`, title/cwd/color delta hook names, `BoundaryJson`, and browser/TUI/hub/rendering terms in the new core modules.

7. `terminal_screen_runtime_seam_feeds_existing_session_worker_contracts`
   - Prove the fake terminal screen runtime can satisfy or compose with `SessionWorkerRuntime::snapshot`, `prepare_snapshot`, `mode_flags`, and `screen`.
   - Assert existing `SnapshotReady`, `PreparedSnapshotReady`, and `ScreenReady` remain the session-worker output carriers.

Verification commands:

- `cargo test terminal_screen`
- `cargo test snapshot_frame_round_trips_opaque_bytes_without_parsing`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`

Review checks:

- No new dependencies in `Cargo.toml`.
- No Ghostty/restty imports or build steps in core.
- No hub/browser/TUI/Rails/plugin product policy in the diff.
- Public non-test types and methods satisfy the crate's `missing_docs = "warn"` lint profile.
- New engine and test code avoid `unwrap`; workspace clippy enforces `unwrap_used = "warn"` and verification runs with `-D warnings`.
- Opaque snapshot byte equality is covered with binary data, not only strings.

## Vault Gaps Worth Capturing

Likely capture after implementation, if the final API confirms it:

- `botster-core` owns the terminal screen/snapshot boundary as opaque payload, synced screen-state, and lifecycle hook contracts; Ghostty/restty remain concrete host adapter dependencies, not core dependencies.

No convention conflict was found while planning. Existing vault notes already cover the broader constraints: session/client actors own the terminal data plane, direct snapshot helpers are translated rather than preserved, and core must avoid product/rendering policy.
