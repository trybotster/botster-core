# Implementation report: bind immutable negotiated terminal capabilities

Ticket: `ticket_1786682902_405026`
Run: `run_1786682907_564108`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/bind-immutable-negotiated-terminal-capabilities.md`
Plan artifact: `artifact_1786683335_768743`
Implement checklist: `checklist_1786685324_249973`

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Repository path from `list_spawn_targets`: admitted `botster-core` spawn target (`trybotster/botster-core`)
- Pipeline worktree: Botster-managed ticket worktree for this run
- Merge policy: `direct` (no PR)

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]
- [[project-pipelines-playbook]] — workflow-only. This step records artifacts, gates, and checklists. It does not change Project Pipelines plugin source.

Not loaded:

- [[botster runtime teardown lenses]] — `teardown_class_applies` is no
- [[spa-patterns]] — listed by the implementer overlay; not applicable to this Rust contract
- Hub, Web, TUI, TUI Kit, Ghostty, and Workspaces charters — this run targets `botster-core` only

Targeted notes:

- [[Core ClientWorker bind requires a live attach generation]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[Core reports terminal mechanism capabilities and Hub admits their use]]
- [[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
- [[botster terminal v1 starts at protocol 1 and conformance revision 1]]
- [[public protocol versions host control and Core terminal planes independently]]
- [[Core owns the incremental attach phase machine]]
- [[botster core contract surface needs consumer proof]]
- [[botster core ui and capability contracts must avoid product gravity]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[additive daemon capabilities do not raise the default client requirement]]
- [[ready then history is advertised as optional daemon support]]
- [[proposed Hub admission binds adapters with negotiated subscription capabilities]]
- [[Hub embeds CoreDaemon behind one client admission point]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation steps must persist report artifacts for review]]
- [[implementation artifacts must match actual git state]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

## Botster layers changed

- `botster-terminal-protocol`: Hub-safe `TerminalCapabilitySet`
- `botster-core` bind, inventory, and ClientWorker snapshot encode
- `botster-core-daemon` production bind facade
- Isolated Hub-shaped consumers and living architecture docs

No Lua plugin, Hub Unix/WebRTC adapter, host grants, Ghostty runtime, or Project Pipelines product layer.

## Files changed

Create:

- `crates/botster-terminal-protocol/src/capabilities.rs`
- `docs/archive/plans/bind-immutable-negotiated-terminal-capabilities.md`
- `docs/reports/bind-immutable-negotiated-terminal-capabilities-implement.md`

Edit:

- `crates/botster-terminal-protocol/src/lib.rs`
- `crates/botster-terminal-protocol/tests/compatibility.rs`
- `crates/botster-terminal-protocol/tests/hub_shaped.rs`
- `crates/botster-terminal-protocol/tests/consumers/hub-shaped/src/lib.rs`
- `crates/botster-core/src/contract/terminal_subscription.rs`
- `crates/botster-core/src/contract/mod.rs`
- `crates/botster-core/src/lib.rs`
- `crates/botster-core/src/engine/client_worker.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/tests/client_worker_engine_test.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/lib.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/src/lib.rs`
- `docs/architecture/client-worker-terminal-egress.md`
- `docs/architecture/terminal-protocol.md`
- `docs/architecture/engine-command-surface.md`
- `docs/architecture/terminal-adapter.md`

## Ownership boundaries preserved

Core owns protocol tokens, bind, live subscription state, inventory, and bound-adapter encode. Hub still owns grants, admission, and the intersection that produces the set. The set stores advertised feature tokens only. It does not store host grants or plugin `CapabilitySet`.

## Cross-repo dependencies or separately routed work

- Hub ticket `ticket_1786661008_634435` already depends on this ticket.
- This ticket has no outbound repository dependency.
- This run did not edit `botster-hub`.

## Deviations from plan

None. Empty sets remain valid. Bind gained no new error variant. Omission stays a required Rust argument.

## Tests and downstream proof run

Repository gates:

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
script/terminal-protocol-node-smoke.sh
```

All passed. Workspace tests include `worker_bound_adapter_receives_ready_finish_without_drain_snapshots` through `CoreDaemon::bind_terminal_adapter` and `isolated_hub_shaped_consumer_runs_harness_against_its_own_adapter`.

Focused proofs:

- Bind without a `TerminalCapabilitySet` argument fails to compile (`ClientWorker::bind_terminal_adapter` rustdoc `compile_fail`).
- Empty set constructs and binds. Inventory reports `adapter_bound=true` and `Some` empty.
- Unknown tokens fail at `from_tokens`, not at bind.
- `BindTerminalAdapterError` still has only `BindBeforeAttach`, `UnknownSubscription`, `StaleGeneration`, and `AlreadyBound`.
- Second bind of the live generation returns `AlreadyBound` when the new set differs, including empty versus non-empty.
- Unbound inventory reports `capabilities=None`.
- Empty-set stream encodes live `terminal_output` and does not emit snapshot tags. Skipped snapshots do not return on drain and do not fail the subscription.
- Ready-then-history bind still encodes incremental snapshot plus live output.
- `TerminalCompatibilityRequirement::current()` still requires only `terminal_streaming` and `resize`.
- Isolated Hub-shaped consumer binds empty and optional-token sets through the public Core API, reads the same tokens from inventory, and observes opaque live frames without decoding GHOSTSNP.
- Published adapter harness still passes.

Production entry points used:

- `CoreDaemon::bind_terminal_adapter`
- `DefaultBotsterEngine::bind_terminal_adapter`
- ClientWorker `ingest_bound_terminal_frames` plus host `drain` / `drain_runtime_once`

This ticket is not scaffold-only.

## Unverified behavior or residual risk

- Live Hub Unix admission still belongs to `ticket_1786661008_634435`.
- After `attach_client`, initial snapshots may already have left on drain. The Hub-shaped consumer therefore proves inventory and live output on `DefaultBotsterEngine`. Incremental snapshot encode is proved on the ClientWorker production path with explicit Snapshot ingest.
- Generated TypeScript was not expanded. Drift tests still pass. This type is a Hub-safe Rust bind argument, not a browser inventory DTO.

## Missing vault guidance discovered

None that blocked implementation. Captured the planned convention to the vault inbox:

- `core-bind-stores-an-immutable-negotiated-terminal-capability-set.md`

That inbox note should later link from [[Core ClientWorker bind requires a live attach generation]] and [[Core reports terminal mechanism capabilities and Hub admits their use]].
