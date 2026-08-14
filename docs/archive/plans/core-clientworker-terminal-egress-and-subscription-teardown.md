# Core ClientWorker terminal egress and subscription teardown

Ticket: `ticket_1786661004_845807`
Run: `run_1786669014_213203`
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery` / `botster_stack_plan`
Base: pipeline worktree on `botster-core` `main` at `e4d87a0`
Depends on closed: `ticket_1786661004_133253` (content-blind terminal adapter contract)

This ticket is **runtime-teardown class**. [[botster runtime teardown lenses]] applies. Required lens answers are in this document and must appear in Plan gate evidence.

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
- [[botster runtime teardown lenses]]
- [[botster-runtime-reviewer-playbook]]
- [[botster-runtime-verifier-playbook]]
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
- [[pipeline artifacts should cite vault notes by wikilink not home path]]
- [[pipeline artifacts should use path neutral worktree references]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[prefer framework and library components over custom solutions]]

Not loaded:

- [[project-pipelines-playbook]] — Project Pipelines package/plugin paths are out of scope.
- [[botster-hub-playbook]] / [[botster-hub-client-playbook]] / [[botster-web-playbook]] / [[botster-tui-playbook]] / [[botster-tui-kit-playbook]] / [[botster-terminal-ghostty-playbook]] — not the target repository charter. Cross-repo seams are named below without substituting those charters.

Targeted atomic notes:

- [[transport ownership north star for modular Botster is proposed]]
- [[proposed ClientWorker owns terminal queues and terminal frames never retry]]
- [[proposed dead sink handling triggers one Core detach without a Hub round trip]]
- [[proposed ProcessExited closes terminal subscriptions but not the host session]]
- [[proposed Hub audits route ownership against Core subscription inventory]]
- [[proposed Core transport adapters use bounded writes without policy queues]]
- [[proposed Hub admission binds adapters with negotiated subscription capabilities]]
- [[proposed transport lifecycle lets control connections outlive terminal subscriptions]]
- [[proposed Core publishes the transport adapter conformance harness]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[Core owns the incremental attach phase machine]]
- [[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
- [[public protocol versions host control and Core terminal planes independently]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[botster packages should enforce core hub cli plugin provider boundaries]]
- [[botster data plane bypasses the hub through session and client actors]]
- [[botster durable terminal egress is owned by sessionio and clientworker actors]]
- [[client workers own transport neutral stream state not hub orchestration]]
- [[incremental attach snapshot frames require lossless streaming backpressure]]
- [[incremental GHOSTSNP attach streams READY history pages and FINISH]]
- [[attach routes use subscription scoped Core drains]]
- [[worker backed attach snapshots fence PTY output at the worker]]
- [[session wide drains cannot deliver subscription owned initial state]]
- [[post READY history failure omits FINISH and still attaches]]
- [[worker snapshot barrier cancels when the parent path closes]]
- [[session shutdown during attach does not produce attach failed]]
- [[pre READY attach failure creates no attach ownership]]
- [[conformance harnesses gate on deterministic invariants not timing]]
- [[terminal webrtc failure records do not prove peer runtime teardown]]
- [[webrtc peer cleanup removes every per peer owner together]]
- [[file descriptor exhaustion from stale webrtc connections]]
- [[graceful-termination-requires-explicit-cleanup-hooks]]
- [[terminal adapter traits must not reuse TransportIngress or TransportEgress]]

## Context loaded

- Pipeline ticket, project `project_1786660949_205223` (`Botster Terminal Transport North Star`), run, closed parent `ticket_1786661004_133253`, and sibling Hub/Web/TUI tickets. This ticket is the Core ClientWorker push and teardown step. Registered dependency on the adapter-contract ticket is closed.
- Target-repo README, workspace crates, `docs/README.md`, `docs/plans/README.md`, `docs/architecture/terminal-adapter.md`, `docs/reports/content-blind-terminal-adapter-contract-implement.md`, parent plan `docs/archive/plans/content-blind-terminal-adapter-contract-and-conformance-harness.md`, CI `.github/workflows/ci.yml`, and local verification commands in the root README.
- Current production start-here remains spawn → attach → drain → input → shutdown. `CoreDaemon::attach`, `drain`, and `drain_subscription` still return semantic `TransportEgress` for Hub and embedders.
- Current adapter seam exists and is scaffold-only: `contract::terminal_adapter::TerminalAdapter` plus `botster-core-test-support::terminal_adapter` Fake / Unix-shaped / WebRTC-shaped drivers. Parent architecture doc states ClientWorker does not push yet.
- Current `ClientWorkerMessage` and `ClientStreamHarness` are contract/harness types. There is no production ClientWorker that owns a bound adapter, a subscription generation, or a control-plane inventory.
- Current detach is `detach_client(client_id, session_id, subscription_id)` with no generation. `ClientStreamGeneration` already exists on the stream harness.
- Current incremental attach phase machine lives in `WorkerBackedBotsterEngine` and `ManagedSessionRuntime`. READY / PAGE / FINISH order and input/resize barriers already exist on the drain path.
- `botster-terminal-protocol-client::TerminalEvent::to_frame` is the semantic-to-opaque encoder. Hub must not depend on that crate. Core may.
- Repo placement: reviewable plans go under `docs/archive/plans/`. `docs/plans/` is a retired stub. Living design after merge belongs in `docs/architecture/`.
- Worktree hygiene: tracked `.gitignore` has content. Worktree path has no `:`. `CARGO_TARGET_DIR` override is not required for this Plan visit.
- This ticket is not a consumer of Hub session-type eligibility work. No parent pin, hub-test-support 0.1.26, or conf 33 requirement applies.
- Project north-star notes remain `decision_state: proposed` in the vault. This project and ticket are the implementation authorization for this Core slice. Do not wait for a separate ratification ticket.

## Botster layers touched

- `botster-core` production ClientWorker, bind API, inventory, detach generation, and adapter pump on the existing host tick.
- `botster-core-daemon` public bind, inventory, and generation-aware detach surfaces.
- `botster-core-test-support` Fake adapter proof plus an isolated Hub-shaped consumer that binds through public APIs.
- Living architecture and README pointers for the bound-adapter path.
- No Lua plugin, Hub runtime, real Unix socket, real WebRTC DataChannel, TUI, SPA, Ghostty crate change, or Project Pipelines product layer.

## Product decision ledger

These values are decisions, not Implement choices.

| Decision | Value | Disposition |
| --- | --- | --- |
| Ticket class | Runtime-teardown. ClientWorker owns bound-adapter egress, queues, slow-client policy, and subscription teardown | Binding |
| Production entry this ticket | `CoreDaemon` bind + existing host tick (`drain` / `drain_runtime_once`) pumps ClientWorker `try_write`. Fake adapter observes frames | Binding |
| Actor shape | Synchronous ClientWorker inside the engine. No new OS thread or async runtime | Binding |
| Drain coexistence | Unbound subscriptions keep today's `TransportEgress` drain path. Bound subscriptions do not also emit those terminal frames on drain | Binding. Dual terminal delivery is forbidden. Dual *unbound* drain remains until Hub cold-cut `ticket_1786661010_198387` |
| Start-here | spawn → attach → drain → input → shutdown stays the unbound embedder path. Bind is an advanced host/adapter seam, not prelude | Binding |
| Bind API | Additive `bind_terminal_adapter`. Do not change `attach_client` / `CoreDaemon::attach` required arguments | Binding |
| Adapter type | `Box<dyn TerminalAdapter + Send>` | Binding |
| Frame encoding | Core encodes with `botster-terminal-protocol-client` `to_frame`. Do not invent a second JSON encoder | Binding |
| Queue owner | Only ClientWorker. Adapter still has one in-flight slot and no policy queue | Binding |
| Retry | Terminal frames never retry. Recovery is detach + fresh attach | Binding |
| Lost snapshot | Any lost READY / PAGE / FINISH / other snapshot frame fails that subscription and requires a fresh attach | Binding |
| Slow-client | Queue overflow or persistent adapter `WouldBlock`/`Full` past the ClientWorker bound fails that subscription. No silent drop of snapshot frames | Binding |
| Detach identity | Idempotent by `subscription_id` + `generation`. Existing `detach` without generation detaches the live generation if present | Binding |
| Adapter `Closed` | One effective Core detach for the bound generation. No Hub Detach round-trip. No host session shutdown | Binding |
| ProcessExited | Remaining output, then `process_exit`, then close that subscription and adapter. Repeat per live subscription on the session. Host session stays | Binding |
| Inventory | Control-plane records only: client, session, subscription, generation, adapter-bound. No terminal state, phases, or bytes | Binding |
| Host policy | Out of Core. No worktree, spawn-template, or plugin lifecycle policy | Binding |
| Public enums | New inventory/detach-result enums are exhaustive at `0.1.0`. Adding a variant is breaking. Prefer additive methods over changing existing enums | Binding |
| Downstream proof this ticket | Isolated Hub-shaped consumer binds a content-blind adapter through public Core/CoreDaemon APIs and observes opaque frames. No live Hub, Web, or TUI in this run | Binding |
| Worker-backed attach | Existing incremental attach tests stay green. If ClientWorker sits on the worker-backed output path, add a worker PTY + authentic Ghostty bound-adapter proof | Binding |
| Hub adapters | Unix `ticket_1786661008_634435` and WebRTC `ticket_1786661008_247079` | Cross-repo follow-up |
| Hub drain removal | `ticket_1786661010_198387` | Cross-repo follow-up |
| Follow-up-ok | Vault note that bound-adapter push is current, not proposed, after this slice merges | Follow-up |
| Ask human only if | Implement would remove drain, double-deliver bound frames, put attach phases in inventory, add a second queue in the adapter, implement real sockets/DataChannels, or shut down the host session on adapter close / ProcessExited | Threshold |

## Runtime-teardown lens answers

| Field | Content |
| --- | --- |
| `teardown_class_applies` | Yes. The ticket owns ClientWorker / SessionIo subscription teardown, multi-subscription isolation, adapter `Closed` vs live-runtime divergence, and ProcessExited close order. |
| `teardown_isolation` | One failed subscription owns its queue, bound adapter, attach barrier interest, and inventory row. Sibling subscriptions on the same session stay live. ProcessExited is session-wide only in that every live subscription receives its own remaining-output + `process_exit` + close. Host session, registry, and worker process stay. |
| `teardown_bounds` | ClientWorker is synchronous. `try_write` and `close` are non-blocking adapter calls. Core must not `block_on` adapter close or wait for an in-flight write. Close abandons the in-flight slot. A hanging adapter `close()` is a Hub adapter defect; Core still marks the subscription torn down after the single `close()` call and must not spin a retry loop. |
| `late_message_matrix` | See table below. |
| `production_path_proof` | Production path: host tick → `CoreDaemon::drain` / `drain_runtime_once` → ClientWorker pump → `TerminalAdapter::try_write` / `close`. Proof oracles: Fake adapter `delivered_frame_bytes` and `pressure() == Closed`, inventory absence, sibling still delivering, session still listed. A map-remove or JSON file is not enough. Tests must drive attach/bind/detach/close/exit through the public facade, not only a helper that skips the pump. |
| `ownership_identity` | Durable owner key is `(session_id, subscription_id, generation)`. Client id is associated metadata. Reused `subscription_id` requires a new generation. Delayed `Closed` or Detach for generation N must not delete generation N+1. |
| `sibling_fail_closed_policy` | Successful close: siblings keep working. Ultimate close failure (adapter `close()` returns after Core already marked torn down, or close hangs): Core still treats the subscription as gone, does not take down siblings, and a test must prove sibling isolation. Session ProcessExited closes every subscription on that session by design; that is not sibling sacrifice. |

### Late-message admission matrix

Every message that creates or can recreate durable terminal-subscription ownership is listed. Ingress that only mutates an existing owner is included so late delivery cannot resurrect a closed generation.

| Message | Grant / owner tag | After terminal failure | Sweep if it races close |
| --- | --- | --- | --- |
| `attach` / `AttachClient` | Host-chosen `client_id` + `session_id` + `subscription_id`; Core assigns or records `generation` | Same id + stale generation rejected. Same id + new generation is a new owner. Pre-READY failure creates no attach ownership | Cancel snapshot barrier; do not leave a bind-pending row |
| `bind_terminal_adapter` | `subscription_id` + `generation` | Reject unknown, stale generation, already-bound, or closed | If bind wins the race against detach, one detach still closes that adapter. If detach wins, bind returns a typed error and the adapter is closed by the caller or by Core reject-and-close |
| Existing `detach` without generation | Live generation for that `subscription_id` if present | Idempotent no-op when already gone | None |
| Generation-aware detach | `subscription_id` + `generation` | Idempotent no-op on mismatch or already gone | None |
| Adapter `Closed` (local `close` or transport death) | Bound adapter identity = `subscription_id` + `generation` | One effective detach. Second `Closed` is a no-op | Close is the sweep. Adapter must not outlive the terminated subscription |
| Terminal input | Live `client_id` + session + subscription + generation | Drop / fail closed. Do not create a subscription | Drop |
| Resize | Live client + session + subscription + generation | Drop / fail closed | Drop |
| SessionIo output / snapshot pages / live bytes | Live subscription + generation | Drop. Do not enqueue onto a closed or new generation | Drop |
| Snapshot / read-screen / replay requests | Live subscription + generation | Drop if no live owner | Drop |
| Lost snapshot / queue overflow | Live subscription + generation | Fail that subscription. Require fresh attach. No frame replay | Same as detach |
| `ProcessExited` from the session runtime | Session | Deliver remaining output then `process_exit` then close each live subscription and adapter | Close remaining adapters. Emit a separate control-plane session lifecycle event. Do not shut down host policy |
| Session shutdown | Session | Distinct from `attach_failed`. Cancel barriers. Close each subscription and adapter | Same as ProcessExited close-out without claiming attach failure |
| Hub-forwarded Detach | Same as detach | Same | Same |
| SubscribeEntities / UnsubscribeEntities / peer Request | Not a Core terminal-subscription owner in this repository | Out of scope | Out of scope |

A plan that guarded only Detach and not Attach, Bind, or adapter `Closed` would be incomplete.

## Scope

Make Core ClientWorker the sole terminal egress and teardown authority for **adapter-bound** subscriptions. Keep host session policy out of Core.

### 1. Production ClientWorker

Add a synchronous ClientWorker in `crates/botster-core/src/engine/` (new module). Wire it from `ManagedSessionRuntime` / `DefaultBotsterEngine` / `WorkerBackedBotsterEngine` so the existing host tick pumps it.

Responsibilities:

- Own the only subscription egress queue and slow-client policy.
- Encode SessionIo / attach-phase outcomes to `TerminalFrame` via `botster-terminal-protocol-client`.
- `try_write` through the bound adapter. On `WouldBlock` or `Full`, keep the frame at the head of the ClientWorker queue. Do not ask the adapter to retry.
- On adapter `Closed`, run one idempotent detach for that generation.
- Expose inventory rows without terminal state.

Do not turn `ClientStreamHarness` into the production owner. Keep that harness as the existing drain-path contract test. Do not reuse `TransportIngress` / `TransportEgress` as adapter trait names.

### 2. Bind, generation, and inventory APIs

Additive public methods on the engine facade and `CoreDaemon`:

- `bind_terminal_adapter(client_id, session_id, subscription_id, generation, adapter)`
- `list_terminal_subscriptions()` → inventory records
- generation-aware detach, plus keep current `detach` as “detach live generation if present”

Bind rules:

- Bind may precede attach (adapter waits for the subscription) or follow attach before the first pump of that route.
- After bind, that route’s terminal frames leave only through the adapter.
- `drain` / `drain_subscription` must not also return those terminal frames.
- Control-plane observations, backpressure, and session lifecycle may still appear on drain.
- Double bind for the same live generation is a typed error.

Inventory record fields:

- `client_id`
- `session_id`
- `subscription_id`
- `generation`
- `adapter_bound`

Forbidden inventory fields: READY/PAGE/FINISH, attach phase, snapshot bytes, queue contents, decoder state.

Do not add these APIs to `prelude`.

### 3. Teardown

One function tears down a `(subscription_id, generation)`:

1. Ignore if missing or generation mismatches.
2. Cancel any snapshot barrier owned by that subscription.
3. On ProcessExited path only: pump remaining queued output, then write `process_exit`.
4. `adapter.close()` once if bound.
5. Drop the queue. Abandoned in-flight adapter writes stay abandoned.
6. Remove the inventory row.
7. Emit control-plane detached / lifecycle facts. Do not emit host shutdown policy.

Callers of that function:

- explicit detach
- adapter `Closed`
- lost snapshot / slow-client failure
- ProcessExited (once per live subscription)
- session shutdown (not `attach_failed`)

### 4. Attach phases and barriers stay in Core

Reuse the existing incremental attach phase machine. Do not build a second READY/FINISH/`Attached` machine.

Bound-adapter proof must cover:

- READY, PAGE, FINISH order
- live output after attach
- input and resize barriers
- cancellation
- post-READY history failure still attaches without FINISH
- lost snapshot page fails the subscription

### 5. Docs

After implementation, update living docs:

- new `docs/architecture/` note for ClientWorker bind/push/teardown/inventory
- `docs/architecture/terminal-adapter.md` (remove “ClientWorker does not push yet”)
- `docs/architecture/engine-command-surface.md` if commands are added
- root README: start-here stays drain; bind is the advanced adapter path

Do not write to retired `docs/plans/`.

## Non-scope

- Removing `CoreDaemon::drain` / `drain_subscription` or the unbound start-here path
- Hub Unix or WebRTC production adapters
- Hub cold-cut of terminal drains
- Real sockets, DataChannels, encryption, or chunking
- Ingress adapter trait
- Host session spawn, worktree, retention, or plugin lifecycle policy
- Changing `TransportIngress` / `TransportEgress` enum names
- Web, TUI, or TUI Kit consumption
- Ratifying the whole north-star vault set beyond this Core slice
- Project Pipelines package/plugin work

## Repository ownership boundaries and cross-repo dependencies

Core owns:

- ClientWorker queues, slow-client policy, attach phases, detach, adapter pump, inventory

Hub owns, and this run must not implement:

- adapter admission, grants, Unix/WebRTC instances, framing, encryption
- route ownership records and reconciliation against inventory
- host session cleanup after ProcessExited

Registered dependency: closed Core adapter ticket `ticket_1786661004_133253` on the same target.

Do not broaden this run to Hub. Do not add a Hub ticket dependency that would block this Core merge; Hub tickets consume this API later.

```
this ticket (Core ClientWorker)
  depends on: ticket_1786661004_133253 (closed)
  consumed later by:
    ticket_1786661008_634435 (Hub Unix adapters)
    ticket_1786661008_247079 (Hub WebRTC adapters)
    ticket_1786661010_198387 (Hub cold-cut)
    ticket_1786661010_115885 (integration proof)
```

## Assumptions and unknowns

Assumptions:

- The project is sufficient authorization to implement proposed north-star notes for this Core slice.
- Hub will keep draining unbound terminal frames until its bind and cold-cut tickets. Core must not break that path in this merge.
- `botster-terminal-protocol-client` as a Core library dependency is allowed because Core owns both crates and Hub still must not take that dependency.
- A host tick already exists (`drain`). Pumping adapters there is the production entry, not a second event loop.
- `Send` on the adapter trait object is enough for Hub to construct an adapter and move it onto the Core owner thread.

Unknowns Implement must resolve locally, not by asking unless they hit the ask-human threshold:

- Exact typed error names for stale generation, already-bound, and unknown subscription
- Whether generation is assigned by Core on attach or supplied by the host on bind
- How tightly to bound the ClientWorker queue versus `QueueSource::ClientWorker` capacity 512

Recommended default for the generation unknown: Core assigns a monotonic generation on attach; bind must present that generation from inventory. Host-supplied generation is accepted only when it matches the live row.

## Affected surfaces / files

Expected touch list. Implement may add tests beside these; do not wander into Ghostty vendor or Hub.

- `crates/botster-core/src/engine/client_worker.rs` (new)
- `crates/botster-core/src/engine/mod.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/command.rs`
- `crates/botster-core/src/contract/actor.rs` (generation on detach / inventory types if they belong on the contract)
- `crates/botster-core/src/lib.rs`
- `crates/botster-core/Cargo.toml` (depend on `botster-terminal-protocol-client` if encoding lives here)
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/api.rs`
- `crates/botster-core-daemon/src/lib.rs`
- `crates/botster-core/tests/` new ClientWorker / inventory / teardown tests
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` or a focused sibling
- `crates/botster-core-test-support/tests/consumers/` isolated bind/push consumer
- `docs/architecture/terminal-adapter.md`
- `docs/architecture/` new ClientWorker note
- `docs/architecture/engine-command-surface.md`
- `README.md`

Keep existing adapter harness laws unchanged unless a ClientWorker test needs a driver hook that does not weaken adapter rules.

## Risks

- Double delivery: bound route still appears on `drain_subscription`. Tests must assert drain is empty of that route’s terminal frames after bind.
- Dual queues: adapter slot plus ClientWorker queue is correct; a third buffer is not.
- Breaking Hub: changing `attach` / `detach` signatures or removing drain.
- Inventory becoming a second attach state machine.
- Lost snapshot silently skipped instead of failing the subscription.
- ProcessExited closing the host session or skipping sibling subscriptions.
- Stale generation detach deleting a reused subscription.
- Encoding drift if Core hand-builds JSON instead of `to_frame`.
- Worker fence regression if output routing changes without authentic Ghostty proof.
- Treating Fake adapter map contents as teardown proof without `Closed` pressure and inventory absence.
- Unbounded pump loop on `WouldBlock`.

## Acceptance checks / tests

Ticket acceptance, all against the Core Fake adapter on the production facade (`CoreDaemon` or `DefaultBotsterEngine` / worker-backed engine + public bind/pump):

1. READY, PAGE, FINISH, live output, input and resize barriers, cancellation, slow-client failure, and ProcessExited order.
2. A lost snapshot frame fails that subscription and requires a fresh attach. No terminal-frame replay helper exists.
3. Concurrent subscriptions stay isolated. One `Closed` or slow-client failure does not stop a sibling.
4. Inventory reports identity and generation without terminal state duplication.
5. Detach is idempotent by subscription id and generation.
6. Adapter `Closed` causes one effective detach. Host session remains.
7. ProcessExited order: remaining output, then `process_exit`, then adapter `Closed` / inventory removal. Control-plane session lifecycle is separate.
8. Late Attach / Bind / Detach / input / output / `Closed` follow the matrix above.
9. Bound-route terminal frames do not also appear on `drain` / `drain_subscription`.
10. Isolated Hub-shaped consumer binds a content-blind adapter through public APIs and does not decode Snapshot bodies.

Charter / lens additions:

11. Existing adapter conformance harness stays green.
12. Existing unbound drain / incremental attach / daemon integration tests stay green.
13. If worker-backed output routing changes: real worker PTY + authentic Ghostty bound-adapter proof with pre-boundary and post-boundary markers.
14. Production-path oracles: delivered fake-adapter bytes, `pressure() == Closed`, inventory absence, sibling still live. Not a terminal JSON file.
15. Deterministic assertions. No timing-budget pass/fail. See [[conformance harnesses gate on deterministic invariants not timing]].

Repository gates after implementation:

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
```

Focused development tests may run first. Do not invent a repository test wrapper.

Merge directly into `main`. Do not create a PR.

## Vault gaps worth capturing

After merge, capture if still true:

- Bound-adapter ClientWorker push is current Core behavior, not only a proposed north-star note.
- Drain remains the unbound / transitional host path until Hub cold-cut.
- Subscription ownership identity is `(subscription_id, generation)`.

Do not capture Hub adapter implementation details from this Core slice.

## Implement notes

- Prefer the smallest wiring that reuses the existing attach phase machine and adapter trait.
- Keep one Plan → Implement path. Do not open a second pipeline for teardown variety.
- Review must load [[botster-runtime-reviewer-playbook]].
- Verify must load [[botster-runtime-verifier-playbook]] and prove the live pump path.
