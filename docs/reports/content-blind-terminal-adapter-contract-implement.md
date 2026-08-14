# Implementation report: content-blind terminal adapter contract

Ticket: `ticket_1786661004_133253`
Run: `run_1786666001_600822`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/content-blind-terminal-adapter-contract-and-conformance-harness.md`
Checklist: `checklist_1786667568_576124`

## Target repository and target_id

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Pipeline worktree: Botster-managed ticket worktree for this run
- Merge policy: `direct` (no PR)

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-architecture]]
- [[cli-patterns]]

Not loaded:

- [[project-pipelines-playbook]] — package/plugin paths are out of scope
- [[botster runtime teardown lenses]] — `teardown_class_applies` is false
- [[spa-patterns]] — listed by the implementer overlay; not applicable to this Rust contract

Targeted notes:

- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[terminal adapter traits must not reuse TransportIngress or TransportEgress]]
- [[proposed Core transport adapters use bounded writes without policy queues]]
- [[proposed Core publishes the transport adapter conformance harness]]
- [[conformance harnesses gate on deterministic invariants not timing]]
- [[test script required for rust tests not cargo test]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation steps must persist report artifacts for review]]
- [[implementation artifacts must match actual git state]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

## Botster layers changed

- `botster-core` public contract: `TerminalAdapter`, write/pressure enums
- `botster-core-test-support` always-on adapter harness and three in-memory drivers
- Isolated Hub-shaped consumer crate under test-support
- Living architecture/README pointers

No Lua plugin, Hub runtime, Ghostty, ClientWorker push, real Unix/WebRTC, or Project Pipelines product layer.

## Files changed

Create:

- `crates/botster-core/src/contract/terminal_adapter.rs`
- `crates/botster-core/tests/terminal_adapter_contract_test.rs`
- `crates/botster-core-test-support/src/terminal_adapter/mod.rs`
- `crates/botster-core-test-support/src/terminal_adapter/core.rs`
- `crates/botster-core-test-support/src/terminal_adapter/fake.rs`
- `crates/botster-core-test-support/src/terminal_adapter/unix_shaped.rs`
- `crates/botster-core-test-support/src/terminal_adapter/webrtc_shaped.rs`
- `crates/botster-core-test-support/tests/terminal_adapter_conformance_test.rs`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/Cargo.toml`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/src/lib.rs`
- `docs/architecture/terminal-adapter.md`
- `docs/reports/content-blind-terminal-adapter-contract-implement.md`
- `docs/archive/plans/content-blind-terminal-adapter-contract-and-conformance-harness.md`

Edit:

- `crates/botster-core/Cargo.toml`
- `crates/botster-core/src/contract/mod.rs`
- `crates/botster-core/src/contract/transport.rs` — doc cross-link only
- `crates/botster-core/src/lib.rs` — module re-export only; not prelude
- `crates/botster-core-test-support/Cargo.toml`
- `crates/botster-core-test-support/src/lib.rs`
- `README.md`
- `docs/README.md`
- `docs/architecture/terminal-protocol.md`
- `Cargo.lock`

## Ownership boundaries preserved

- Adapter trait and harness stay in `botster-core` / `botster-core-test-support`
- Opaque frames consumed from `botster-terminal-protocol`; no protocol public-API change
- No `botster-terminal-protocol-client` dependency
- No Hub, Web, TUI, TUI Kit, or Ghostty edits
- Existing `TransportIngress` / `TransportEgress` enums unchanged except a doc pointer
- Trait kept out of `prelude`

## Cross-repo dependencies or separately routed work

- Parent protocol ticket `ticket_1786661004_962658` is closed
- ClientWorker push remains `ticket_1786661004_845807`
- Hub Unix/WebRTC adapters remain later Hub tickets
- No new ticket dependency added

## Deviations from plan

`assert_terminal_adapter_conformance` requires `D: Default` so both
close-during-active-write paths run on independent adapters. The committed
plan now records that bound. No other scope change.

## Tests and downstream proof run

Repository README cargo commands, not the legacy CLI `./test.sh` wrapper.
This workspace has no `cli/test.sh`; the charter and plan name cargo gates.

- `cargo fmt --all -- --check` PASS
- `cargo clippy --workspace --all-targets -- -D warnings` PASS
- `cargo test --workspace` PASS
- `cargo test --doc --workspace` PASS
- `cargo doc --workspace --no-deps` PASS
- `cargo test -p botster-core --no-default-features --lib` PASS
- `cargo test -p botster-core --test terminal_adapter_contract_test` PASS
- `cargo test -p botster-core-test-support --test terminal_adapter_conformance_test` PASS
- `cargo test -p botster-core-test-support --no-default-features --test terminal_adapter_conformance_test` PASS
- Isolated consumer `cargo test --offline` with its own `CARGO_TARGET_DIR` PASS via the workspace conformance test

Downstream proof: the isolated Hub-shaped consumer implements its own
`TerminalAdapter` and `TerminalAdapterHarnessDriver` and runs
`assert_terminal_adapter_conformance`. The three published Core drivers also
pass that harness. Live Hub Unix/WebRTC adapters are later tickets.

Production entry point this ticket: public `TerminalAdapter` API and the
published test-support harness. Intentionally not ClientWorker push.

## Unverified behavior or residual risk

- No production ClientWorker bind/push
- No real Unix socket or WebRTC DataChannel
- Proposed north-star vault notes remain `decision_state: proposed`
- `--no-default-features --lib` still emits pre-existing unused-import /
  dead-code warnings in unrelated engine modules; they are warnings, not
  clippy `-D warnings` failures under default features

## Missing vault guidance discovered

None. The `TransportIngress` / `TransportEgress` name collision is already
captured in [[terminal adapter traits must not reuse TransportIngress or TransportEgress]].
No new inbox capture this visit.

## Constraints applied

- Keep the start-here path on prelude spawn → attach → drain → input → shutdown
- Do not reuse existing transport frame enum names
- Public enums remain exhaustive at `0.1.0`
- Harness asserts deterministic invariants through driver hooks
- Isolated consumer must implement the trait, not only import a Core driver
