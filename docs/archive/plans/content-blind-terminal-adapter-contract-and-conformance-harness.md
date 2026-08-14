# Content-blind terminal adapter contract and conformance harness

Ticket: `ticket_1786661004_133253`
Run: `run_1786666001_600822`
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery` / `botster_stack_plan`
Base: pipeline worktree on `botster-core` `main`
Depends on closed: `ticket_1786661004_962658` (types-only terminal protocol)
Revision: addresses Plan Review `review_1786667060_162655` findings
`finding_1786667060_490660` and `finding_1786667060_829786`.

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
- [[botster pipeline needs continuous product owner between agent steps]]
- [[plan steps need reviewable plan artifacts]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

Not loaded:

- [[project-pipelines-playbook]] — Project Pipelines package/plugin paths are out of scope.
- [[botster runtime teardown lenses]] — this ticket defines the adapter write/close/pressure contract and a transport-neutral harness. It does not change WebRTC/peer lifecycle, SessionIo/ClientWorker teardown, multi-peer ownership, CPU/battery/FD spin, or terminal-state vs live-runtime divergence. Those belong to sibling ticket `ticket_1786661004_845807` and later Hub adapter tickets.
- [[botster-hub-playbook]] / [[botster-web-playbook]] / [[botster-tui-playbook]] / [[botster-tui-kit-playbook]] / [[botster-terminal-ghostty-playbook]] — not the target repository charter. Cross-repo seams are named below without substituting those charters.

Targeted atomic notes:

- [[transport ownership north star for modular Botster is proposed]]
- [[proposed Core transport adapters use bounded writes without policy queues]]
- [[proposed Core publishes the transport adapter conformance harness]]
- [[proposed Hub terminal tests enforce content blind adapters]]
- [[proposed ClientWorker owns terminal queues and terminal frames never retry]]
- [[proposed dead sink handling triggers one Core detach without a Hub round trip]]
- [[proposed Hub admission binds adapters with negotiated subscription capabilities]]
- [[proposed transport lifecycle lets control connections outlive terminal subscriptions]]
- [[proposed terminal plane prefers a dedicated stream per subscription]]
- [[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
- [[public protocol versions host control and Core terminal planes independently]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[botster packages should enforce core hub cli plugin provider boundaries]]
- [[botster data plane bypasses the hub through session and client actors]]
- [[botster durable terminal egress is owned by sessionio and clientworker actors]]
- [[incremental attach snapshot frames require lossless streaming backpressure]]
- [[local process pty reader queues must be bounded and pressure must be typed]]
- [[conformance harnesses gate on deterministic invariants not timing]]
- [[hub test support npm releases need external consumer smoke]]

## Context loaded

- Pipeline ticket, project `project_1786660949_205223` (`Botster Terminal Transport North Star`), run, closed parent `ticket_1786661004_962658`, and sibling tickets. This ticket is the Core adapter-contract second step. Registered dependency on the types-only protocol ticket is closed.
- Target-repo README, workspace `Cargo.toml`, crate manifests, `docs/README.md`, `docs/plans/README.md`, local verification commands in the root README.
- Current Core types: `contract::transport::{TransportIngress, TransportEgress}` are semantic frame enums, publicly re-exported. They are not adapter traits.
- Current protocol plane: `botster-terminal-protocol` 0.1.0 exports opaque `TerminalFrame` with `from_bytes` / `to_bytes` and no phase, state, history, payload, or Snapshot-body accessors.
- Current test support: `botster-core-test-support` `conformance` is PTY/`local-runtime` gated. `FakeSessionTransport` records the old semantic `TransportIngress` / `TransportEgress` frames. No content-blind adapter trait or adapter harness exists.
- Parent plan `docs/archive/plans/types-only-terminal-protocol-and-compatibility-contract.md` explicitly reserved adapter contract work for this ticket.
- Repo placement: reviewable plans go under `docs/archive/plans/`. `docs/plans/` is a retired stub. Living design after merge belongs in `docs/architecture/`.
- Worktree hygiene: tracked `.gitignore` has content. Worktree path has no `:`. `CARGO_TARGET_DIR` override is not required for this Plan visit.
- This ticket is not a consumer of Hub session-type eligibility work.
- Plan Review `review_1786667060_162655` returned `changes_required`. Open product findings: close during an active write (`finding_1786667060_490660`) and isolated consumer must implement an adapter (`finding_1786667060_829786`). Process finding `finding_1786667060_303419` is the duplicate vault checklist; reuse `checklist_1786666367_554958` and do not create another.

## Botster layers touched

- `botster-core` public contract module for the adapter trait and typed write/pressure/close results.
- `botster-core-test-support` transport-neutral harness plus Fake, Unix-shaped, and WebRTC-shaped test adapters.
- Living architecture/README pointers.
- Isolated Hub-shaped consumer that implements its own `TerminalAdapter` and `TerminalAdapterHarnessDriver` and runs the published harness.
- No Lua plugin, Hub runtime, Hub host-control protocol, TUI, SPA, Rails relay, MCP, Ghostty adapter, SessionIo/ClientWorker production push, real Unix socket, real WebRTC DataChannel, or Project Pipelines product layer.

## Product decision ledger

These values are decisions, not Implement choices.

| Decision | Value | Disposition |
| --- | --- | --- |
| Ticket class | Contract + harness scaffold for later ClientWorker and Hub adapters | Binding |
| Production entry point this ticket | Public `TerminalAdapter` API and the published test-support harness | Binding. Intentionally not ClientWorker push. Document that in architecture. |
| Trait names | `TerminalAdapter`, `TerminalAdapterWriteError`, `TerminalAdapterPressure` | Binding. Do not reuse `TransportIngress` / `TransportEgress`. |
| Write payload | `&botster_terminal_protocol::TerminalFrame` | Binding |
| Write results | typed `WouldBlock`, `Full`, `Closed` | Binding |
| Adapter capacity | exactly one transport-internal active write | Binding. Not a configurable queue. |
| Pressure | pollable `pressure()` on the trait. No async runtime, waker, or channel requirement | Binding |
| Retry | adapter must not retain or later emit a rejected frame | Binding |
| Close while a write is active | local `close()` and transport-side close abandon the in-flight frame; it is not delivered | Binding. Same rule for both close paths. |
| Isolated consumer | must implement its own adapter and harness driver; must not only call a published Core driver | Binding |
| Harness driver bound | `assert_terminal_adapter_conformance` requires `D: Default` so both close-during-active-write paths run on independent adapters | Binding. Implement detail required by the both-close-path law |
| Ingress adapter | out of scope | Non-goal |
| Real Unix/WebRTC | out of scope. Test adapters are transport-shaped in-memory drivers | Non-goal |
| ClientWorker bind/push/teardown | sibling `ticket_1786661004_845807` | Non-goal |
| Hub production adapters | Hub tickets `ticket_1786661008_634435` and `ticket_1786661008_247079` | Cross-repo follow-up |
| Public enums | not `#[non_exhaustive]` at `0.1.0`; adding a variant is breaking | Binding, same as the protocol plane |
| Follow-up-ok | vault ratification of the proposed north-star notes after this slice lands | Follow-up |
| Ask human only if | Implement would rename existing `TransportEgress` enums, put the trait in `botster-terminal-protocol`, add a second queue, or implement real sockets/DataChannels in Core | Threshold |

## Scope

Define the Core-owned content-blind terminal egress adapter contract and publish a transport-neutral conformance harness in Core test support.

### 1. Public contract in `botster-core`

Add `crates/botster-core/src/contract/terminal_adapter.rs` and export it through `contract` and the existing compatibility re-export style. Do not add it to `prelude`. The start-here path remains spawn → attach → drain → input → shutdown. This trait is an advanced host/adapter seam.

Add a path dependency:

```toml
botster-terminal-protocol = { path = "../botster-terminal-protocol", version = "0.1.0" }
```

Pinned public API:

- `TerminalAdapter`
- `TerminalAdapterWriteError`
- `TerminalAdapterPressure`

Pinned trait:

```rust
pub trait TerminalAdapter {
    fn try_write(
        &mut self,
        frame: &botster_terminal_protocol::TerminalFrame,
    ) -> Result<(), TerminalAdapterWriteError>;

    fn close(&mut self);

    fn pressure(&self) -> TerminalAdapterPressure;
}
```

Pinned enums, not `#[non_exhaustive]`:

```rust
pub enum TerminalAdapterWriteError {
    WouldBlock,
    Full,
    Closed,
}

pub enum TerminalAdapterPressure {
    Ready,
    WouldBlock,
    Full,
    Closed,
}
```

Laws:

| Result / signal | Meaning | Adapter may retain the frame? |
| --- | --- | --- |
| `Ok(())` | The frame occupies the single active-write slot until the transport finishes that write | Yes, that one in-flight frame only |
| `WouldBlock` | Transport is not ready even though the write slot is empty | No |
| `Full` | The one active-write slot is occupied | No |
| `Closed` | Adapter is closed. Further writes stay `Closed` | No |

Additional laws:

- `close()` is idempotent. After `close()`, `try_write` returns `Closed` and `pressure()` is `Closed`.
- Transport-side death (test hook or later production close) has the same `Closed` effect as local `close()`.
- Close while a write is active: if `try_write` has returned `Ok(())` and `complete_active_write` has not run, local `close()` and `force_closed()` both abandon that in-flight frame. `delivered_frame_bytes` does not grow. `pressure()` becomes `Closed`. Later `try_write` returns `Closed`. `complete_active_write` after close is a no-op and must not deliver the abandoned frame. Implementations must not flush-then-close, deliver after `Closed`, or keep the slot `Full` after close.
- `Ok(())` means the slot is occupied, not that the client received the frame. Close is transport death. Terminal frames do not retry, so the abandoned frame is lost and later recovery is a fresh attach on the sibling ClientWorker ticket.
- The one in-flight slot is transport state, not a second policy queue. The adapter must not enqueue additional frames behind that slot.
- The adapter must not retry a rejected write, reorder accepted frames, or inspect `TerminalFrame` bodies. Serialization via `TerminalFrame::to_bytes()` is allowed. Decoding Snapshot bodies or matching READY / PAGE / FINISH is forbidden.
- Framing, encryption, and ciphertext chunking may happen inside the one active write. Chunks of frame N must not interleave with frame N+1.
- Subscription queues, attach state, slow-client policy, and snapshot resynchronization stay in Core ClientWorker. They are not adapter methods.

Do not reuse or overload `TransportIngress` / `TransportEgress`. Those remain the current semantic drain-path frame enums. The proposed vault notes used those names for adapter traits; this repo already spent them. New names are the ratification of that slice.

Do not add `TransportIngress` adapter methods, capability binding, subscription inventory, or ClientWorker push in this ticket.

### 2. Conformance harness in `botster-core-test-support`

Add an always-on module `crates/botster-core-test-support/src/terminal_adapter/` that does not require `local-runtime` or `ghostty-terminal`.

Do not put adapter laws in `conformance/mod.rs`. That module is PTY-gated and proves a different contract.

Harness shape:

```rust
pub trait TerminalAdapterHarnessDriver {
    type Adapter: botster_core::contract::terminal_adapter::TerminalAdapter;
    fn adapter(&mut self) -> &mut Self::Adapter;
    fn force_would_block(&mut self);
    fn clear_would_block(&mut self);
    fn complete_active_write(&mut self);
    fn force_closed(&mut self);
    fn delivered_frame_bytes(&self) -> &[Vec<u8>];
}

pub fn assert_terminal_adapter_conformance<D>(driver: &mut D)
where
    D: TerminalAdapterHarnessDriver + Default;
```

The harness must prove deterministic invariants, not timing:

1. Bounds: after `Ok(())`, the next `try_write` is `Full` until `complete_active_write`. No second queued frame appears in `delivered_frame_bytes`.
2. Ordering: accepted frames are delivered in write-accept order after their active writes complete.
3. Typed rejection: `force_would_block` yields `WouldBlock` with an empty slot; occupied slot yields `Full`; after close, `Closed`.
4. Close propagation: local `close()` and `force_closed()` both make later writes and pressure `Closed`. Close is idempotent.
5. Close during an active write: after `Ok(())` while the slot is `Full`, both local `close()` and `force_closed()` leave `delivered_frame_bytes` unchanged, set `pressure()` to `Closed`, reject the next `try_write` with `Closed`, and leave `complete_active_write` as a no-op. Previously completed frames remain. The abandoned in-flight frame does not appear later.
6. No adapter retry: a rejected `try_write` does not later appear in `delivered_frame_bytes` unless the caller writes that same frame again after the adapter returns `Ready`.
7. Content-blind write: the harness writes opaque `TerminalFrame` values constructed from the protocol crate. It does not ask adapters to report phase or Snapshot body. WebRTC-shaped chunking, if simulated, is internal to one active write and still delivers one complete frame byte blob per accepted write.

Publish three drivers that implement the same driver trait and pass the same harness function:

| Driver | What it simulates | What it must not do |
| --- | --- | --- |
| `FakeTerminalAdapter` | In-memory one-slot sink | Policy queue, retry, attach state |
| `UnixShapedTerminalAdapter` | Ordered byte pipe with one in-flight write | Real `UnixStream`, listen/accept, host auth |
| `WebRtcShapedTerminalAdapter` | One in-flight write that may split ciphertext into chunks | Real DataChannel, DTLS, SCTP, or Hub crypto |

These are Core-owned test adapters so the harness is transport-neutral before Hub implements production adapters. They are not production Unix or WebRTC.

Add `botster-terminal-protocol` as a `botster-core-test-support` dependency for fixture frames only. Do not depend on `botster-terminal-protocol-client`.

### 3. Downstream-shaped consumer proof

Crate-local trait tests are not enough ([[botster core contract surface needs consumer proof]]).

Add `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/` as an isolated Cargo package that:

- depends on path `botster-core` with `default-features = false`
- depends on path `botster-core-test-support` with `default-features = false`
- depends on path `botster-terminal-protocol`
- does not depend on `botster-terminal-protocol-client`, `botster-hub`, or `botster-hub-test-support`
- implements its own minimal `TerminalAdapter` and `TerminalAdapterHarnessDriver` in the consumer crate
- constructs opaque frames and calls `assert_terminal_adapter_conformance` against that external implementation
- must not satisfy the consumer proof by only constructing a published Core driver (`FakeTerminalAdapter`, `UnixShapedTerminalAdapter`, or `WebRtcShapedTerminalAdapter`)

A workspace test must `cargo test` or `cargo check` that consumer with its own `CARGO_TARGET_DIR`, matching the protocol crate's hub-shaped consumer pattern. The consumer package must actually run the harness against its own adapter, not only compile imports.

Also add `crates/botster-core-test-support/tests/terminal_adapter_conformance_test.rs` that runs the harness against all three published Core drivers, including `--no-default-features`. Those three runs stay. They do not replace the external implementation.

Add `crates/botster-core/tests/terminal_adapter_contract_test.rs` for public-type, docs, and no-default-features compilation of the trait. That test must not become the only proof.

### 4. Docs

Implement writes living truth to `docs/architecture/terminal-adapter.md`:

- adapter vs existing `TransportEgress` enum
- write/close/pressure laws
- one-slot rule
- close-during-active-write abandon rule
- harness ownership
- explicit scaffold note: ClientWorker does not push through this trait until `ticket_1786661004_845807`; Hub Unix/WebRTC adapters are later Hub tickets

Update root README workspace table / test-support sentence and `docs/architecture/terminal-protocol.md` with a one-line pointer. Do not put the adapter contract into the types-only protocol crate.

Do not write to `docs/plans/`.

## Non-scope

- ClientWorker production push, subscription queues, slow-client policy, detach generation, or inventory.
- Binding adapters to subscriptions or negotiated capabilities.
- Real Unix listener/socket or real WebRTC DataChannel.
- Hub admission, grants, encryption, framing production code, or Hub test-support ownership of the harness.
- Deleting or renaming existing `TransportIngress` / `TransportEgress` enums.
- Changing `botster-terminal-protocol` public accessors.
- Ghostty, GHOSTSNP decoding, attach phase machine, or snapshot goldens.
- Ingress adapter trait.
- Async runtime, configurability of slot count, retries, or dual production paths.
- PRs. Merge directly into `main`.

## Repository ownership boundaries and cross-repo dependencies

| Surface | Owner | This ticket |
| --- | --- | --- |
| Adapter trait, write/close/pressure laws | `botster-core` | Yes |
| Opaque `TerminalFrame` | `botster-terminal-protocol` in this repo | Consume only |
| Semantic Snapshot / phase types | `botster-terminal-protocol-client` | Must not depend |
| Adapter conformance harness and test adapters | `botster-core-test-support` | Yes |
| Hub-test-support / Hub production adapters | `botster-hub` | No. Later Hub tickets import this harness |
| ClientWorker push and teardown | `botster-core` sibling ticket `ticket_1786661004_845807` | No |
| Web / TUI protocol consumption | those repos | No |

Do not silently broaden this run into Hub. Do not add a new ticket dependency. Downstream Hub tickets already exist and should consume this contract after it merges.

Parent protocol ticket is closed. This ticket may depend on `botster-terminal-protocol` 0.1.0 in-tree.

## Assumptions and unknowns

Assumptions:

- The project north star is the product instruction for this run. The vault notes remain `decision_state: proposed`. This ticket ratifies the adapter-and-harness slice in code and architecture docs. It does not rewrite every proposed vault note.
- "Unix-shaped" and "WebRTC-shaped" mean in-memory drivers with those transport constraints, not OS or browser transports. Real transports would violate Hub ownership.
- `try_write(&TerminalFrame)` is the Core → adapter seam. Adapters may serialize to bytes. They must not gain body accessors.
- Existing semantic `TransportEgress` remains until later cold-cut tickets. This ticket adds a parallel contract; it does not migrate the drain path.
- Isolated consumer proof requires an external adapter implementation plus the three Core driver runs. Live Hub Unix/WebRTC proof is intentionally later.

Unknowns that are not blocking:

- Exact Hub encryption/chunk sizes. WebRTC-shaped driver may use a fixed test chunk size. Production chunking stays in Hub.
- Whether ClientWorker will hold `&mut dyn TerminalAdapter` or a generic. Sibling ticket chooses that. This ticket keeps the trait object-safe (`&TerminalFrame`, no generic write method).

## Affected surfaces / files

Create:

- `crates/botster-core/src/contract/terminal_adapter.rs`
- `crates/botster-core/tests/terminal_adapter_contract_test.rs`
- `crates/botster-core-test-support/src/terminal_adapter/mod.rs` and driver modules
- `crates/botster-core-test-support/tests/terminal_adapter_conformance_test.rs`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/` (`Cargo.toml`, `src/lib.rs` or `main.rs`)
- `docs/architecture/terminal-adapter.md`

Edit:

- `crates/botster-core/Cargo.toml` — add `botster-terminal-protocol`
- `crates/botster-core/src/contract/mod.rs`
- `crates/botster-core/src/lib.rs` — module export only; do not add to `prelude`
- `crates/botster-core-test-support/Cargo.toml` — add `botster-terminal-protocol`
- `crates/botster-core-test-support/src/lib.rs` — `pub mod terminal_adapter` without feature gate
- `README.md`
- `docs/architecture/terminal-protocol.md` — pointer only

Do not edit:

- `crates/botster-core/src/contract/transport.rs` frame enums, except a doc cross-link if needed
- `FakeSessionTransport` / PTY `conformance`
- `botster-terminal-protocol` public API
- Hub, Web, TUI, TUI Kit trees
- `docs/plans/` stub

## Risks

- Name collision with existing `TransportEgress`. Mitigated by pinned new names.
- Implementer expanding into ClientWorker or real transports. Mitigated by non-scope and ledger.
- Harness landing behind `local-runtime`, making Hub `--no-default-features` import fail. Mitigated by always-on module and `--no-default-features` tests.
- Putting the harness in a Hub crate later. This ticket must keep it in `botster-core-test-support`.
- Treating one in-flight write as a multi-frame buffer. Harness must fail that.
- Close during an active write flushing or delivering after `Closed`. Harness must fail that.
- Timing assertions. Forbidden; use driver hooks.
- `botster-core` gaining a protocol-crate dependency. Intended and one-way. Protocol crate must still not depend on `botster-core`.
- Public enum break later. Document the 0.1.0 exhaustive-match rule.

## Acceptance checks / tests

Ticket acceptance mapped:

| Ticket acceptance | Proof |
| --- | --- |
| Fake, Unix-shaped, and WebRTC-shaped test adapters run the same harness | `assert_terminal_adapter_conformance` called on all three drivers in `terminal_adapter_conformance_test.rs` |
| Harness proves bounds, ordering, typed rejection, close propagation, and no adapter retry | Explicit assertions listed in Scope §2, including close-during-active-write for both close paths |
| Hub test support does not own this harness | No Hub files change. Isolated consumer depends on Core test support only and implements its own adapter |
| Merge directly into main | No PR |

Charter / convention proof:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test --doc --workspace`
- `cargo doc --workspace --no-deps`
- `cargo test -p botster-core --no-default-features --lib`
- `cargo test -p botster-core --test terminal_adapter_contract_test`
- `cargo test -p botster-core-test-support --test terminal_adapter_conformance_test`
- `cargo test -p botster-core-test-support --no-default-features --test terminal_adapter_conformance_test`
- Isolated Hub-shaped consumer runs `assert_terminal_adapter_conformance` against an adapter implemented in that package
- Architecture doc states this ticket is scaffold-for-consumers: production ClientWorker push and Hub adapters are later tickets
- `teardown_class_applies`: false. Runtime-teardown lenses not required.

Downstream proof required by [[botster-core-playbook]] is the isolated Hub-shaped consumer's own adapter implementation plus the three published Core drivers. Live Hub Unix/WebRTC adapter proof is owned by later Hub tickets and must import this same harness.

## Implementation sequence

1. Add `terminal_adapter` contract module and `botster-core` → `botster-terminal-protocol` dependency.
2. Export the module. Keep it out of `prelude`.
3. Add test-support module, three drivers, and harness.
4. Add crate tests, the three Core driver runs, and the isolated consumer that implements its own adapter and driver.
5. Write `docs/architecture/terminal-adapter.md` and the README / terminal-protocol pointers.
6. Run the acceptance commands above.
7. Merge directly to `main`. Do not open a PR.

## Plan Review resolutions

| Finding | Resolution |
| --- | --- |
| `finding_1786667060_490660` close during an active write | Close abandons the in-flight frame on both local and transport-side close. Harness asserts `delivered_frame_bytes`, `pressure()`, later `try_write`, and no-op `complete_active_write`. |
| `finding_1786667060_829786` isolated consumer | Consumer crate must implement its own `TerminalAdapter` and `TerminalAdapterHarnessDriver` and run the harness. Published Core drivers remain additional proof, not a substitute. |
| `finding_1786667060_303419` duplicate checklist | Process-only. Reuse `checklist_1786666367_554958`. Do not create another checklist. Record the timeout-retry skip reason in gate evidence. |

## Vault gaps worth capturing

- Proposed notes name adapter traits `TransportIngress` / `TransportEgress`, but those identifiers already name semantic frame enums. Capture the collision so later agents do not follow the proposed names. Inbox this Plan visit: `existing TransportEgress enums are semantic frames not adapter traits`.
- After merge, the adapter-and-harness slice of [[proposed Core transport adapters use bounded writes without policy queues]] and [[proposed Core publishes the transport adapter conformance harness]] can be ratified. Do not rewrite those notes in this ticket.
- Full north-star ratification still waits on ClientWorker push, Hub adapters, and the cold cut.

## Runtime-teardown class

`teardown_class_applies`: false.

Not loaded: [[botster runtime teardown lenses]].

Close signaling exists on the adapter so later teardown can observe `Closed`. This ticket does not implement detach, peer lifecycle, or SessionIo/ClientWorker teardown. Those answers belong on `ticket_1786661004_845807` and the Hub adapter tickets.
