# Types-only terminal protocol and compatibility contract

Ticket: `ticket_1786661004_962658`
Run: `run_1786661055_398881`
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery` / `botster_stack_plan`
Base: `main` at `033cd01`

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Repository playbook: [[botster-core-playbook]]
- Resolved from `list_spawn_targets` via ticket `target_id`. Not inferred from the ambient session directory.

## Playbooks and notes loaded

Role and overlay:

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-core-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[spa-patterns]]
- [[project pipeline orchestration belongs in a device-level botster plugin]]
- [[project pipelines needs an operator workbench not more primitives]]
- [[project pipelines ui contract belongs in the plugin readme]]
- [[botster orchestration should spawn agents with explicit target ids]]
- [[botster orchestration prompts must bind agents to explicit worktrees]]

Not loaded:

- [[project-pipelines-playbook]] — Project Pipelines package/plugin paths are out of scope.
- [[botster runtime teardown lenses]] — this ticket is types-only; it is not a WebRTC, SessionIo, ClientWorker teardown, multi-peer, or live-runtime ticket.

Targeted atomic notes:

- [[public protocol versions host control and Core terminal planes independently]]
- [[proposed each protocol plane owns its compatibility descriptors]]
- [[transport ownership north star for modular Botster is proposed]]
- [[Core owns the incremental attach phase machine]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[Core reports terminal mechanism capabilities and Hub admits their use]]
- [[botster core contract surface needs consumer proof]]
- [[botster packages should enforce core hub cli plugin provider boundaries]]
- [[generated typescript dtos must encode serde field optionality]]
- [[generated dto drift tests need symmetric field and type checks]]
- [[additive daemon capabilities do not raise the default client requirement]]
- [[ready then history is a compatibility feature not an Attach field]]
- [[ready then history is advertised as optional daemon support]]
- [[compatibility fixtures advertise every required optional feature]]
- [[live terminal output base64 envelopes carry renderable bytes]]
- [[incremental GHOSTSNP attach streams READY history pages and FINISH]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[daemon event shape changes bump conformance fixture revision not protocol version]]
- [[hub test support npm releases need external consumer smoke]]
- [[Hub test support capability cutovers use a new unpublished package version]]
- [[proposed Hub terminal tests enforce content blind adapters]]
- [[proposed Core publishes the transport adapter conformance harness]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[plan steps need reviewable plan artifacts]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

`[[proposed Core publishes the transport adapter conformance harness]]` and the adapter half of [[transport ownership north star for modular Botster is proposed]] were loaded only to keep them out of this ticket. Adapter contract work belongs to sibling ticket `ticket_1786661004_133253`.

## Context loaded

- Pipeline ticket, project, run, and empty artifacts/reviews/questions/checklists via `project_pipelines_current_context`.
- Project `project_1786660949_205223` (`Botster Terminal Transport North Star`) and sibling tickets. This ticket is the protocol-plane first step. No registered ticket dependencies.
- Target-repo README, workspace `Cargo.toml`, CI (`.github/workflows/ci.yml`), `docs/README.md`, `docs/plans/README.md`, and living architecture docs.
- Current Core terminal/runtime types: `TransportIngress` / `TransportEgress`, `TerminalAttachState`, `WorkerSnapshotPhase`, session-process protocol in `contract/session_protocol.rs`.
- Current Hub-owned combined protocol in `botster-hub-client`: `PROTOCOL = botster-hub-daemon-v1`, `PROTOCOL_VERSION = 7`, `CONFORMANCE_FIXTURE_REVISION = 38`, mixed `DaemonRequest` / `DaemonEvent`, TypeScript emitter, and `@trybotster/hub-test-support` 0.1.33 including GHOSTSNP goldens.
- Repo placement: landed plans go under `docs/archive/plans/`. `docs/plans/` is a retired stub.

## Botster layers touched

- New Core-owned types-only protocol crate, generated TypeScript, fixtures, and package versioning.
- Living architecture/README for the new crate.
- No Lua plugin, Hub runtime, Hub host-control protocol, TUI, SPA, Rails relay, MCP, Ghostty adapter, SessionIo/ClientWorker, or Project Pipelines product layer.

## Scope

Create a standalone, Core-owned, types-only terminal protocol plane in this workspace.

1. Add workspace crate `botster-terminal-protocol` (version `0.1.0`).
2. Own the public terminal wire types:
   - Requests: `Attach`, `Detach`, `Input`, `Resize`.
   - Events: `Snapshot` including first-class `SnapshotPhase` (`ready`, `history`, `finish`), `AttachState`, `TerminalOutput`, `ProcessExited`.
3. Own terminal protocol identity and compatibility:
   - Independent protocol name, not `botster-hub-daemon-v1`.
   - `protocol_version` and `conformance_fixture_revision`.
   - Feature tokens and advertised-vs-required descriptors.
   - `snapshot_delivery=ready_then_history` is advertised optional support and is not in the default client requirement. Attach stays `{ session_id, subscription_id }`.
4. Make adapter-facing frame bodies opaque outside the crate:
   - Public `TerminalFrame` serializes and deserializes.
   - No public Snapshot-body or attach-phase accessors on that type.
   - Snapshot history bytes stay an opaque validated base64 envelope.
   - Live `TerminalOutput` uses the same envelope field names but a distinct type whose decoded bytes are renderable PTY output.
5. Generate TypeScript from the Rust serde source. Commit the artifact. Drift-check field sets, mapped types, and per-field optionality.
6. Own terminal conformance assets that belong to Core:
   - Copy Hub-owned GHOSTSNP late-attach goldens and terminal-event fixtures into this crate.
   - Do not edit Hub in this run.
7. Package versioning: Cargo crate version plus a committed Node package coordinate such as `@trybotster/terminal-protocol` at `0.1.0`, kept unpublished in this ticket.
8. Prove crate isolation: no `botster-core` runtime dependency and no Hub crate dependency.
9. Document the living contract under `docs/architecture/` and list the crate in the root README workspace table.

## Non-scope

- Hub host-control protocol: hello, admission, spawn, packages, grants, status, entity frames, Drain, ReadScreen, ReadModeFlags, CaptureSnapshot, WebRTC delivery chunks.
- Deleting or republishing `@trybotster/hub-test-support`. That is Hub successor work (`ticket_1786661010_198387` and related Hub tickets).
- Transport adapter trait, WouldBlock/Full/Closed, or adapter conformance harness (`ticket_1786661004_133253`).
- ClientWorker push path, teardown, or subscription inventory (`ticket_1786661004_845807`).
- Wiring `botster-core`, `botster-core-daemon`, or Ghostty production paths to the new crate.
- Web, TUI, or TUI Kit consumption (later tickets).
- Publishing to crates.io or npm.
- Changing `TransportEgress` / session-process SPH1 frames in this ticket.
- Porting Hub `Scrollback` into the new plane. Incremental attach uses Snapshot phases.
- Runtime-teardown proofs, live Hub pins, or authentic Ghostty production attach in this ticket.

## Repository ownership boundaries and cross-repo dependencies

Core owns this crate, terminal feature tokens, terminal compatibility descriptors, generated terminal TypeScript, and terminal fixtures.

Hub still owns the host-control protocol and current combined `botster-hub-client` surface until later Hub tickets compose the two planes and drop terminal authority.

Web and TUI will pin this crate or its generated package by version, not a Hub Git revision. That consumption is not this run.

Ghostty remains the snapshot-byte authority. This crate owns protocol envelopes and frozen goldens; it does not decode GHOSTSNP records.

Cross-repo:

- No blocking prerequisite. This run can complete in `botster-core` alone.
- Do not broaden this run into Hub, Web, TUI, or TUI Kit.
- Successor, not prerequisite: Hub must later stop owning terminal feature tokens, terminal TypeScript, and GHOSTSNP goldens. Register any Hub follow-up against `tgt_7e208a0c76a44980a83b63af976b1f22`, never this target.
- Successor: Web `tgt_40abcf71ccf049f4ac0c99953a799869` and TUI `tgt_c3d470bab78549df920a41e8fb0e58d8` consume the published coordinate.

## Assumptions and unknowns

Assumptions:

- The project north star plus this ticket authorizes the types-only protocol plane. It does not ratify adapter push or Hub cold-cut in this run.
- Crate name is `botster-terminal-protocol`. Protocol identity is a new string such as `botster-terminal-v1`, starting at protocol version `1` and conformance revision `1`.
- `SnapshotPhase` is a first-class field on the new plane's Snapshot type. Current Hub `DaemonEvent::Snapshot` has no phase field; phases live inside GHOSTSNP. The new plane makes phase visible to clients without GHOSTSNP decode, while Hub adapters see only `TerminalFrame`.
- Public API split: `wire::TerminalFrame` is opaque serialize-only. A `client` module holds semantic request/event types for Web/TUI and TypeScript generation. Hub-shaped consumers must depend only on `wire` plus compatibility descriptors.
- `ProcessExited` is the new-plane name. Current Hub/Core event name is `ProcessExit`. Mapping is a later Hub cutover concern.
- `AttachState` vocabulary: `attaching`, `attached`, `snapshot_history_incomplete`, `attach_failed`. `detached` stays Core-internal unless a public wire need appears.
- Default required terminal features are the baseline attach/detach/input/resize/output set. Additive tokens stay advertised-only.
- External consumer smoke in this repo is a path-isolated Cargo consumer plus an `npm pack` / tarball or equivalent installed-artifact check. CI has no Node toolchain, so the Node pack check must be Rust-driven or skip-cleanly documented; the committed generated TS plus Cargo consumer are the required CI proof.
- Direct merge to `main`. No PR.

Unknowns:

- Exact default-required token names (`terminal_streaming` + `resize` vs one baseline token). Implementer should mirror current Hub baseline names that are terminal-owned and omit host tokens.
- Whether later TUI Rust should use `client` types or only generated artifacts. This ticket exposes `client` so TUI is not blocked.
- Whether workspace crate versions stay independently at `0.1.0` or later share a workspace version. Do not add release tooling here.

## Affected surfaces and files

Expected new:

- `crates/botster-terminal-protocol/Cargo.toml`
- `crates/botster-terminal-protocol/src/lib.rs`
- `crates/botster-terminal-protocol/src/{wire,client,compat,typescript}.rs` or equivalent small modules
- `crates/botster-terminal-protocol/generated/terminal-protocol.ts`
- `crates/botster-terminal-protocol/fixtures/ghostsnp/*` copied from Hub test-support goldens
- `crates/botster-terminal-protocol/fixtures/*.json` terminal compatibility / event-order fixtures
- `crates/botster-terminal-protocol/tests/*` isolation, opacity, compatibility, drift, consumer-shaped proof
- `crates/botster-terminal-protocol/examples/generate_typescript.rs` if that matches Hub prior art
- `packages/terminal-protocol/**` versioned Node package mirror of generated TS, metadata, and fixtures
- `docs/architecture/terminal-protocol.md`

Expected edits:

- `Cargo.toml` workspace members
- `README.md` workspace table and start-here note that clients pin this crate for terminal types
- `Cargo.lock`

Do not touch:

- `crates/botster-core/src/engine/**`
- `crates/botster-core-daemon/**`
- `crates/botster-terminal-ghostty/**`
- Hub, Web, TUI, TUI Kit trees

## Risks

- Over-scoping into adapter/runtime tickets.
- Copying Hub host-control types into the new crate and recreating a combined protocol.
- Public semantic accessors that let Hub inspect Snapshot bodies or attach phases.
- Putting `snapshot_delivery=ready_then_history` in the default requirement and breaking older unchanged clients.
- Generating TypeScript with required fields that serde omits.
- Treating crate-local tests as enough consumer proof.
- Editing Hub to "finish the move" and violating repository ownership.
- Placing the plan under retired `docs/plans/`.
- Dual-use of history-bearing GHOSTSNP goldens as no-history fixtures.

## Acceptance checks and tests

Production entry point for this ticket is the new crate's public API and committed artifacts. Later runtime tickets will emit these frames. This ticket is intentionally scaffold-for-consumers, not a live attach path change.

Repository / CI (must pass on the worktree):

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --doc --workspace
```

If the worktree path contains `:`, set colon-free `CARGO_TARGET_DIR` first. This worktree path does not.

Crate-specific proof:

- `cargo tree -p botster-terminal-protocol` shows no `botster-core`, `botster-core-daemon`, `botster-hub`, or `botster-hub-client`.
- Default compatibility requirement accepts a descriptor that omits `snapshot_delivery=ready_then_history`.
- `for_ready_then_history_attach()` rejects that descriptor and accepts a current advertised descriptor.
- Feature-dependent fixtures advertise every optional token they require.
- Snapshot and TerminalOutput share envelope field names but are distinct types. Live output rejects legacy `{ "data": "..." }`. Snapshot decoded bytes are not a render API.
- `wire::TerminalFrame` has no public `phase()`, `history()`, `state()`, or Snapshot-body getter. A compile-fail or isolated consumer test proves a Hub-shaped crate cannot inspect those.
- Generated TS drift test asserts bidirectional field sets, mapped types, and per-field optionality.
- Committed generated artifact matches the emitter.
- Package metadata version equals crate `0.1.0`.
- Isolated external Cargo consumer depends on the crate by path/version and compiles against public types without Hub.
- Copied GHOSTSNP goldens keep distinct SHAs for history vs blank and are not regenerated at build time.

Downstream-shaped proof required by [[botster-core-playbook]] / [[botster core contract surface needs consumer proof]]:

- The isolated Cargo consumer plus generated-TS token smoke are the in-repo stand-ins. Live Web/TUI attach is later tickets.
- Do not claim Hub or browser production proof from this run.

Not required here:

- Authentic Ghostty encode/decode beyond frozen golden bytes.
- Live Hub pin, Unix/WebRTC adapter harness, or teardown matrix.

## Vault gaps

- Capture after implement if useful: Core terminal protocol identity is independent of `botster-hub-daemon-v1`, including the exact protocol string and starting versions.
- Capture if the `wire` vs `client` module split is the durable opacity rule, or if a second crate is later required.
- Do not capture adapter-harness or ClientWorker-push decisions from this ticket.

## Worktree and target assumptions

- Implement and later steps use spawn target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` and the pipeline-assigned worktree, not the ambient session directory and not Hub/Web/TUI checkouts.
- Merge directly to `main`. Do not create a PR.
- Restore tracked `.gitignore` from HEAD if a later step wipes it. Never truncate it.

## Runtime-teardown class

`teardown_class_applies`: false.

This ticket does not change peer lifecycle, SessionIo/ClientWorker teardown, multi-peer ownership, CPU/battery/FD spin, or terminal-state vs live-runtime divergence.

## Product decision ledger

- Default: new independent protocol plane; copy Core-owned fixtures; keep Hub untouched.
- Non-goals: adapter contract, runtime push, Hub deletion of old assets, client cutover.
- Follow-up-ok: Hub/Web/TUI successor tickets already exist on this project.
- Ask-human threshold: only if Implement cannot keep the crate types-only without editing Hub or inventing host-control types.

## Pipeline gates and artifacts

- Plan artifact: this file.
- Implement must leave a report artifact and command evidence for the acceptance commands above.
- Review/Verify must reject URI-only Plan evidence and crate-local tests without consumer-shaped proof.

## Required docs

- `docs/architecture/terminal-protocol.md` — living contract.
- Root README workspace table.
- Crate-level rustdoc start-here: types and compatibility only; no runtime.

## Implementation sketch

1. Add the crate and workspace member with `serde`, `serde_json`, `base64`, `thiserror`. No runtime crates.
2. Define compatibility constants and advertised/required split first, with the two-direction tests.
3. Define client request/event types from the current terminal wire semantics, plus Snapshot phase.
4. Define `TerminalFrame` as the only adapter-facing public body: encode/decode bytes, no semantic getters.
5. Port the Hub TypeScript emitter pattern into a crate-local generator. Commit `generated/terminal-protocol.ts`.
6. Copy GHOSTSNP goldens and terminal fixtures. Add SHA distinctness tests.
7. Add `packages/terminal-protocol` metadata that mirrors crate version and generated artifact.
8. Add isolated consumer test and architecture doc.
9. Run the CI command set. Do not create a PR.
