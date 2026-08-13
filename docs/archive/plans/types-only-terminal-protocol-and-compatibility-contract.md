# Types-only terminal protocol and compatibility contract

Ticket: `ticket_1786661004_962658`
Run: `run_1786661055_398881`
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery` / `botster_stack_plan`
Base: `main` at `033cd01`
Revision: addresses Plan Review `review_1786661988_371515` findings
`finding_1786661988_522167`, `finding_1786661988_724616`, `finding_1786661988_868062`.

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

- Pipeline ticket, project, run, artifacts, Plan Review `changes_required`, and the three open product findings.
- Project `project_1786660949_205223` (`Botster Terminal Transport North Star`) and sibling tickets. This ticket is the protocol-plane first step. No registered ticket dependencies.
- Target-repo README, workspace `Cargo.toml`, CI (`.github/workflows/ci.yml`), `docs/README.md`, `docs/plans/README.md`.
- Current Core types: `TransportIngress` / `TransportEgress`, `TerminalAttachState`, `WorkerSnapshotPhase`.
- Current Hub-owned combined protocol: `botster-hub-daemon-v1` / protocol 7 / conf 38. Terminal event tags and envelopes from `botster-hub-client` and `late-attach-history-conformance-fixture.json`.
- Repo placement: landed plans go under `docs/archive/plans/`.

## Botster layers touched

- Two Core-owned types-only crates, generated TypeScript, fixtures, Node package, and CI Node consumer smoke.
- Living architecture/README.
- No Lua plugin, Hub runtime, Hub host-control protocol, TUI, SPA, Rails relay, MCP, Ghostty adapter, SessionIo/ClientWorker, or Project Pipelines product layer.

## Scope

Create the Core-owned types-only terminal protocol plane as two workspace crates plus one Node package.

### Crate and package boundary

| Coordinate | Consumers | Public surface |
| --- | --- | --- |
| `botster-terminal-protocol` 0.1.0 | Hub adapters and any content-blind forwarder | Compatibility descriptors, feature tokens, request types Hub may forward, opaque `TerminalFrame` |
| `botster-terminal-protocol-client` 0.1.0 | TUI Rust and the TypeScript generator | Semantic Snapshot / phase / AttachState / TerminalOutput / ProcessExit types |
| `@trybotster/terminal-protocol` 0.1.0 | Web (and any Node consumer) | Generated TypeScript, metadata, terminal fixtures |

Dependency direction:

- `botster-terminal-protocol-client` depends on `botster-terminal-protocol`.
- `@trybotster/terminal-protocol` is generated from the client crate. It is not a Rust public surface.
- Hub must depend only on `botster-terminal-protocol`. Hub must not depend on `botster-terminal-protocol-client` or import generated TypeScript to inspect frames.
- Neither crate depends on `botster-core`, `botster-core-daemon`, `botster-hub`, or `botster-hub-client`.

This split is the enforceable answer to `finding_1786661988_522167`. A public `client` module on the Hub-consumable crate is forbidden.

### Hub-consumable crate public API allowlist

`botster-terminal-protocol` may export only:

- `PROTOCOL`
- `PROTOCOL_VERSION`
- `CONFORMANCE_FIXTURE_REVISION`
- `DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION`
- `FEATURE_TERMINAL_STREAMING`
- `FEATURE_RESIZE`
- `FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY`
- `TerminalCompatibility`
- `TerminalCompatibilityRequirement` with `current()` and `for_ready_then_history_attach()`
- `ensure_compatible`
- `Attach`, `Detach`, `SendInput`, `Resize`
- `TerminalFrame`

`TerminalFrame` serializes and deserializes. It has no public `phase`, `state`, `history`, `payload`, or Snapshot-body accessor. Construction from bytes and emission of bytes are allowed.

The crate must not export `Snapshot`, `SnapshotPhase`, `AttachState`, `TerminalOutput`, `ProcessExit`, or any decoded GHOSTSNP type.

### Client crate public API

`botster-terminal-protocol-client` owns the semantic event types and encodes them into `TerminalFrame`. TUI uses this crate. The TypeScript emitter lives here.

### Pinned public compatibility vocabulary

These values are decisions, not Implement choices. They map to the current producer and the new independent plane.

| Item | Pinned value | Producer mapping |
| --- | --- | --- |
| Protocol name | `botster-terminal-v1` | Sibling of `botster-hub-daemon-v1`. New plane, not a Hub revision. |
| `PROTOCOL_VERSION` | `1` | New plane. Exact equality, same rule as Hub. |
| `CONFORMANCE_FIXTURE_REVISION` | `1` | New fixture set. Floor comparison. |
| `DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION` | `1` | No prior published terminal-plane revision. |
| Default required features | `terminal_streaming`, `resize` | Current Hub default includes these two terminal tokens plus host tokens that stay in Hub. |
| Advertised support | default required plus `snapshot_delivery=ready_then_history` | Current Hub `current_feature_list` advertises that token without requiring it. |
| Operation-specific requirement | `for_ready_then_history_attach()` adds `snapshot_delivery=ready_then_history` | Same split as Hub `DaemonCompatibilityRequirement::for_ready_then_history_attach()`. |
| Attach request | `{ "type": "attach", "session_id", "subscription_id" }` | Current `DaemonRequest::Attach`. No delivery-mode field. |
| Detach request | `{ "type": "detach", "session_id", "subscription_id" }` | Current `DaemonRequest::Detach`. |
| Input request | `{ "type": "send_input", "session_id", "data" }` | Current `DaemonRequest::SendInput`. Ticket name is Input; wire tag stays `send_input`. |
| Resize request | `{ "type": "resize", "session_id", "rows", "cols" }` | Current `DaemonRequest::Resize`. |
| Snapshot event | `{ "type": "snapshot", "session_id", "subscription_id", "payload_base64", "payload_encoding": "base64", "bytes", "phase" }` | Current Snapshot envelope plus first-class `phase`. Current Hub Snapshot has no phase field; phases live in GHOSTSNP. The new plane adds `phase` so clients do not decode GHOSTSNP to learn READY/HISTORY/FINISH. |
| Snapshot phase | `ready`, `history`, `finish` | `WorkerSnapshotPhase` / Ghostty READY, HISTORY, FINISH. |
| TerminalOutput event | `{ "type": "terminal_output", "session_id", "subscription_id", "payload_base64", "payload_encoding": "base64", "bytes" }` | Current live envelope. Distinct type from Snapshot. Rejects `{ "data": "..." }`. |
| Process exit event | `{ "type": "process_exit", "session_id", "subscription_id", "code"? }` | Current `DaemonEvent::ProcessExit` / Core `TransportEgress::ProcessExit`. Ticket/north-star name is ProcessExited; wire tag stays `process_exit`. |
| AttachState event | `{ "type": "attach_state", "session_id", "subscription_id", "state" }` | Current Hub event. |
| AttachState `state` | `attaching`, `attached`, `snapshot_history_incomplete`, `attach_failed` | Core produces attaching / attached / snapshot_history_incomplete. Hub currently maps pre-READY drain failure to `attach_failed`. Public wire keeps `attach_failed`. Do not publish `detached`. |
| Envelope encoding | `payload_encoding` is the literal `base64` | Current `DaemonHistoryEncoding::Base64`. |
| Crate / npm versions | `0.1.0` | Unpublished in this ticket. |

Not in this plane: `sessions`, plugin/package/worktree/entity tokens, `terminal_readback`, `mode_gated_input`, `hub_source_update`, `Drain`, `ReadScreen`, `Scrollback`.

`code` on `process_exit` is optional (`skip_serializing_if = Option::is_none`). Generated TypeScript must mark it optional.

### Fixtures and generation

- Copy Hub GHOSTSNP late-attach goldens into the protocol crate as opaque bytes. Keep history vs blank SHA distinctness. Do not regenerate at build time.
- Client crate owns event-order fixtures that include `phase` and advertise `snapshot_delivery=ready_then_history` when they require it.
- Generate TypeScript from the client crate serde source. Commit `crates/botster-terminal-protocol-client/generated/terminal-protocol.ts`.
- Mirror that artifact into `packages/terminal-protocol` at version `0.1.0`.
- Do not edit Hub in this run.

### Docs

- `docs/architecture/terminal-protocol.md` records the two-crate rule, pinned vocabulary, and consumer direction.
- Root README workspace table lists both crates and states Hub may depend only on `botster-terminal-protocol`.

## Non-scope

- Hub host-control protocol: hello, admission, spawn, packages, grants, status, entity frames, Drain, ReadScreen, ReadModeFlags, CaptureSnapshot, WebRTC delivery chunks.
- Deleting or republishing `@trybotster/hub-test-support`. Hub successor work (`ticket_1786661010_198387`).
- Transport adapter trait or adapter conformance harness (`ticket_1786661004_133253`).
- ClientWorker push, teardown, or subscription inventory (`ticket_1786661004_845807`).
- Wiring `botster-core`, `botster-core-daemon`, or Ghostty production paths to these crates.
- Web, TUI, or TUI Kit consumption beyond in-repo consumer smokes.
- Publishing to crates.io or the npm registry.
- Changing `TransportEgress` / session-process SPH1 frames.
- Porting Hub `Scrollback`.
- Runtime-teardown proofs or live Hub attach.

## Repository ownership boundaries and cross-repo dependencies

Core owns both crates, terminal feature tokens, terminal descriptors, generated terminal TypeScript, and terminal fixtures.

Hub still owns the host-control protocol and the current combined `botster-hub-client` surface until later Hub tickets compose the two planes.

Hub successor tickets must depend only on `botster-terminal-protocol`. Web pins `@trybotster/terminal-protocol`. TUI pins `botster-terminal-protocol-client`. None of those pins is a Hub Git revision.

Ghostty remains the snapshot-byte authority. These crates do not decode GHOSTSNP records.

Cross-repo:

- No blocking prerequisite.
- Do not broaden this run into Hub, Web, TUI, or TUI Kit.
- Successor Hub fixture deletion is `ticket_1786664495_777899` against `tgt_7e208a0c76a44980a83b63af976b1f22`. That Hub ticket depends on this Core ticket.
- Successor Web `tgt_40abcf71ccf049f4ac0c99953a799869` and TUI `tgt_c3d470bab78549df920a41e8fb0e58d8`.

## Assumptions and unknowns

Assumptions:

- The project plus this ticket authorizes the types-only protocol plane only.
- Two crates are required so Rust visibility cannot leak semantic bodies to Hub.
- First-class Snapshot `phase` is an intentional new-plane addition. Successors must send it; current Hub Snapshot JSON is not the new plane.
- Wire tags follow the current producer (`send_input`, `process_exit`) rather than ticket prose names.
- Workspace crate versions stay independently at `0.1.0`. No release tooling.
- Direct merge to `main`. No PR.

Unknowns:

- None that remain for Implement. Vocabulary, crate split, and proof commands are pinned.

## Affected surfaces and files

Expected new:

- `crates/botster-terminal-protocol/**`
- `crates/botster-terminal-protocol-client/**`
- `crates/botster-terminal-protocol-client/generated/terminal-protocol.ts`
- `crates/botster-terminal-protocol/fixtures/ghostsnp/*`
- `crates/botster-terminal-protocol-client/fixtures/*.json`
- Hub-shaped and TUI-shaped isolated consumer tests / trybuild cases
- `packages/terminal-protocol/**`
- `script/terminal-protocol-node-smoke.sh` or equivalent
- `docs/architecture/terminal-protocol.md`

Expected edits:

- `Cargo.toml` workspace members
- `README.md`
- `Cargo.lock`
- `.github/workflows/ci.yml` — install Node and run the Node pack smoke. Do not make the smoke optional.

Do not touch:

- `crates/botster-core/src/engine/**`
- `crates/botster-core-daemon/**`
- `crates/botster-terminal-ghostty/**`
- Hub, Web, TUI, TUI Kit trees

## Risks

- Putting semantic event types on `botster-terminal-protocol` and reopening the opacity finding.
- Exporting `phase` or `state` accessors on `TerminalFrame`.
- Raising the default requirement to include `snapshot_delivery=ready_then_history`.
- Including host tokens in this plane.
- Weak TS drift checks or a skippable Node smoke.
- Editing Hub to delete old assets in this run.
- Dual-use of history-bearing GHOSTSNP goldens as no-history fixtures.
- `#[non_exhaustive]` omitted on public enums, so later variant adds break TUI matches. Accept the break at `0.1.0` and document it; do not add `non_exhaustive` unless Implement finds a construction-literal need.

## Acceptance checks and tests

This ticket is scaffold-for-consumers. Later runtime tickets emit these frames. Production entry points in this run are the two crate public APIs, the committed generated package, and CI.

Repository / CI (must pass; no skips):

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --doc --workspace
cargo doc --workspace --no-deps
```

Plus the Node consumer smoke below. Add Node to `.github/workflows/ci.yml`. If `node` or `npm` is missing locally or in CI, the smoke fails.

If the worktree path contains `:`, set colon-free `CARGO_TARGET_DIR` first. This worktree path does not.

Opacity and isolation:

- `cargo tree -p botster-terminal-protocol` and `cargo tree -p botster-terminal-protocol-client` show no `botster-core`, `botster-core-daemon`, `botster-hub`, or `botster-hub-client`.
- Public-item allowlist test on `botster-terminal-protocol` equals the allowlist above.
- Hub-shaped trybuild / isolated crate depends only on `botster-terminal-protocol` and fails to compile if it names `SnapshotPhase`, `AttachState`, `TerminalOutput` payload fields, `phase`, `history`, or any client-crate path. The test searches the complete public API, not only `TerminalFrame` methods.
- TUI-shaped isolated crate depends on `botster-terminal-protocol-client` and can construct and serialize Snapshot with `phase`, AttachState, TerminalOutput, and process_exit.

Compatibility:

- Default requirement accepts a descriptor that advertises `terminal_streaming` and `resize` and omits `snapshot_delivery=ready_then_history`.
- `for_ready_then_history_attach()` rejects that descriptor and accepts the current advertised descriptor.
- Feature-dependent fixtures advertise every optional token they require.

Wire:

- Snapshot and TerminalOutput share envelope field names and remain distinct types.
- Live output rejects `{ "data": "..." }`.
- Snapshot `phase` is required on the new plane.
- `process_exit.code` omits when `None`; generated TS marks `code?`.

Generated artifacts and Web-shaped smoke:

- Drift test asserts bidirectional field sets, mapped types, and per-field optionality against the committed TypeScript file.
- `packages/terminal-protocol` version is `0.1.0` and matches both crate versions.
- Required Node smoke, no skip:
  1. `npm pack` the committed package.
  2. Install that tarball in a clean temporary directory that is not the workspace.
  3. Import the shipped package from the installed copy.
  4. Compile representative generated TypeScript (at least Attach, Snapshot, `phase`, TerminalOutput, `process_exit`, AttachState).
  5. Assert package metadata version `0.1.0`, protocol `botster-terminal-v1`, protocol version `1`, feature tokens `terminal_streaming`, `resize`, and `snapshot_delivery=ready_then_history`.

GHOSTSNP goldens keep distinct SHAs for history vs blank and are not regenerated at build time.

Downstream-shaped proof:

- Hub-shaped Rust consumer: opaque crate only.
- TUI-shaped Rust consumer: client crate.
- Web-shaped Node consumer: installed tarball.
- Live Web/TUI attach remains successor tickets.

Not required here: authentic Ghostty encode/decode beyond frozen goldens; live Hub pin; adapter harness; teardown matrix.

`teardown_class_applies`: false.

## Vault gaps

- After Implement, capture the two-crate opacity rule if it remains the durable boundary.
- Capture `botster-terminal-v1` / version `1` / conf `1` if those constants ship as planned.
- Do not capture adapter-harness or ClientWorker-push decisions.

## Worktree and target assumptions

- Later steps use `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` and the pipeline worktree.
- Merge directly to `main`. Do not create a PR.
- Restore tracked `.gitignore` from HEAD if a later step wipes it.

## Runtime-teardown class

`teardown_class_applies`: false.

## Product decision ledger

- Default: two-crate opacity split; pinned producer-aligned tags; first-class Snapshot `phase`; required Node pack smoke.
- Non-goals: adapter contract, runtime push, Hub deletion of old assets, client cutover.
- Follow-up-ok: existing Hub/Web/TUI successor tickets.
- Ask-human threshold: only if Implement cannot keep semantic bodies off the Hub-consumable crate.

## Plan Review finding response

1. `finding_1786661988_522167` — replaced the public `client` module with `botster-terminal-protocol` vs `botster-terminal-protocol-client`. Hub-shaped compile test covers the complete public API.
2. `finding_1786661988_724616` — pinned protocol name, versions, feature sets, request/event tags, AttachState vocabulary, and producer mappings above. No remaining Implement vocabulary choice.
3. `finding_1786661988_868062` — required clean `npm pack` install, TypeScript compile, and metadata/token asserts. Added `cargo doc --workspace --no-deps`. Smoke must not skip.

## Pipeline gates and artifacts

- Plan artifact: this file.
- Reused ticket vault checklist `checklist_1786661438_736261`. No second checklist.
- Implement must leave a report artifact and the command evidence above.

## Required docs

- `docs/architecture/terminal-protocol.md`
- Root README workspace table
- Crate rustdocs: Hub-safe crate documents the allowlist; client crate documents that Hub must not depend on it

## Implementation sketch

1. Add `botster-terminal-protocol` with the allowlist only.
2. Add compatibility constants and the two-direction tests.
3. Add `botster-terminal-protocol-client` with semantic events, `phase`, and `TerminalFrame` encode/decode.
4. Add allowlist and trybuild Hub-shaped negative tests against the complete protocol-crate public API.
5. Generate and commit TypeScript. Drift-check optionality.
6. Copy GHOSTSNP goldens. Add SHA tests.
7. Add `packages/terminal-protocol` and the required Node pack smoke. Wire Node into CI.
8. Add architecture doc and README rows.
9. Run the full CI command set including `cargo doc --workspace --no-deps` and the Node smoke. Do not create a PR.
