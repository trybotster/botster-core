# Core: make terminal subscriptions duplex and pressure-isolated

Ticket `ticket_1787600672_342292`. Run `run_1787632374_189517`. Step `botster_stack_plan`.

## 1. Target

| Field | Value |
| --- | --- |
| Target repository | `botster-core` (`trybotster/botster-core`) |
| `target_id` | `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` |
| Repository path | the authoritative `botster-core` spawn target |
| Pipeline worktree | the pipeline-provided ticket worktree for this run |
| Base commit | `7eafa47` (`Gate IncrementalAttach uses behind local-runtime.`) |
| Project | `project_1787600579_585482` — Botster Isolated Subscription Data Plane |
| Merge policy | direct |

`list_spawn_targets` maps `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` to `trybotster/botster-core`.
Routing came from the run `target_id`, not from the process working directory.

## 2. Playbooks and notes loaded

### Playbooks

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-core-playbook]] — repository ownership charter
- [[botster runtime teardown lenses]] — runtime-teardown class applies, see §11

### Explicitly not loaded

- [[botster-hub-playbook]], [[botster-web-playbook]], [[botster-tui-playbook]],
  [[botster-hub-client-playbook]], [[botster-workspaces-playbook]],
  [[botster-tui-kit-playbook]], [[botster-terminal-ghostty-playbook]] — those
  repositories own the consumer tickets listed in §6.
- [[project-pipelines-playbook]] — this ticket changes no Project Pipelines
  package or plugin path.

### Targeted atomic notes

- [[core owns duplex terminal transport while Hub stays content blind]]
- [[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
- [[botster terminal v1 starts at protocol 1 and conformance revision 1]]
- [[daemon event shape changes bump conformance fixture revision not protocol version]]
- [[terminal adapter traits must not reuse TransportIngress or TransportEgress]]
- [[Core terminal subscription ownership is session, subscription, and generation]]
- [[Core ClientWorker bind requires a live attach generation]]
- [[Core subscription hard-stop is synchronous close and drop on the host tick]]
- [[Core bind stores an immutable negotiated terminal capability set]]
- [[public protocol versions host control and Core terminal planes independently]]
- [[terminal wire enums and TypeScript unions share one variant inventory]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[botster core public enums are breaking until non exhaustive is decided]]
- [[botster core contract surface needs consumer proof]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster-core CI runs a contract only test lane because workspace feature unification hides breaks]]
- [[Core types-only npm releases use human public publish and clean install proof]]
- [[spawned Hub tests can reach only four of fourteen Core test builders]]
- [[adapter accepted writes are not consumer flushed writes]]
- [[vault example paths are not repository placement conventions]]
- [[frozen repository plan contracts outrank vault convention notes]]

## 3. Context loaded

### Frozen project contract

The parent ticket `ticket_1787600670_129312` is closed. Its executable contract is
`docs/plans/freeze-subscription-ownership-and-capture-the-regression-baseline.md`
in `botster-hub` at plan commit `dfbf934`. That document locks the Core revision at
`7eafa47`, which is this ticket's base commit. The sections this plan consumes:

- §5.1 — Core today publishes an **egress-only** `TerminalAdapter`
  (`try_write`, `close`, `pressure`). Terminal input cannot reach Core through it.
- §8.5 — subscription channels carry **binary** DataChannel messages with 12 KiB
  chunking. Hub frames and chunks opaque bytes; Hub does not parse them.
- §10 — Core is the terminal duplex and subscription authority. Core publishes
  `@trybotster/terminal-protocol` with the required `transport=duplex_binary`
  feature token.
- §14 — acceptance rows **A1**, **A2**, **A3** (Core half), and **A24** (Core
  half) are owned by this ticket. Every other row is owned elsewhere.
- §13 — deletion of `DaemonRequest::SendInput`, `ModeGatedInput`, and `Resize` is
  owned by `ticket_1787600679_990088`, not by this ticket.

### Sibling ticket check

Open siblings at plan time:

| Ticket | Repository | Relation |
| --- | --- | --- |
| `ticket_1787600674_500120` | botster-hub | Consumes this contract. Binds the duplex adapter on WebRTC. |
| `ticket_1787603671_590198` | botster-hub | Consumes this contract on the Unix transport. |
| `ticket_1787600682_233928` | botster-hub | Entity and event channels. No Core seam. |
| `ticket_1787600676_914408` | botster-web | Consumes the npm package. |
| `ticket_1787603674_865638` | botster-tui | Consumes the Rust client crate. |
| `ticket_1787600679_990088` | botster-hub | Cold cut. Deletes the old JSON terminal route. |
| `ticket_1787600691_401181` | botster-hub | Boundary audit. |
| `ticket_1787603669_760394`, `ticket_1787600689_646958`, `ticket_1787600684_892051` | botster-web | Baseline, Restty, entity channels. No Core seam. |

No sibling ticket edits `botster-core`. There is no same-repository concurrency risk.

### Repository source read

| Path | Current state |
| --- | --- |
| `crates/botster-core/src/contract/terminal_adapter.rs` | `TerminalAdapter` with `try_write`, `close`, `pressure`. Egress only. Enums exhaustive at `0.1.0`. |
| `crates/botster-core/src/engine/client_worker.rs` | `ClientWorker` owns `(session_id, subscription_id)` → `SubscriptionOwner { client_id, generation, adapter, capabilities, queue, … }`. `WRITE_ATTEMPT_BUDGET = 512`. `pump()` is the per-tick egress pump. |
| `crates/botster-core/src/engine/managed_session_runtime.rs:975` | `apply_client_worker_with` is the single tick site that calls `ingest_bound_terminal_frames` then `pump`. |
| `crates/botster-core-daemon/src/daemon.rs:1088` | `CoreDaemon::drain` is the per-session tick that hosts call. |
| `crates/botster-core-daemon/src/daemon.rs:1240` | `CoreDaemon::mode_gated_input` is today a host-called JSON-RPC-shaped method. |
| `crates/botster-terminal-protocol` | Hub-safe crate. Opaque `TerminalFrame`, `Attach`, `Detach`, `SendInput`, `Resize`, compatibility, `PUBLIC_API_ALLOWLIST`. |
| `crates/botster-terminal-protocol-client` | Semantic crate. `Snapshot`, `AttachState`, `TerminalOutput`, `ProcessExit`, TypeScript generator. |
| `packages/terminal-protocol` | `@trybotster/terminal-protocol` `0.1.0`, `metadata.json`, mirrored `terminal-protocol.ts`. |
| `crates/botster-core-test-support/src/terminal_adapter/` | Transport-neutral conformance harness plus Unix-shaped and WebRTC-shaped drivers. |
| `.github/workflows/ci.yml` | The CI-owned gate commands recorded in §14. |
| `docs/README.md`, `docs/plans/README.md` | `docs/plans/` is a retired stub. Landed plans live in `docs/archive/plans/`. Living truth lives in `docs/architecture/`. |

Worktree hygiene: tracked `.gitignore` is 63 bytes and matches HEAD. The worktree
path contains no `:`. No `CARGO_TARGET_DIR` override is required.

## 4. Scope

In scope:

1. Extend the terminal protocol with an **opaque binary ingress frame family**:
   input bytes, mode-gated input bytes, and resize.
2. Extend `TerminalAdapter` from egress-only to **duplex**: add a non-blocking
   ingress poll alongside `try_write`, `close`, and `pressure`.
3. Extend `ClientWorker` to own a **bounded, per-subscription ingress queue**,
   round-robin ingress drain with a per-tick budget, decode, and ownership tagging.
4. Apply decoded input inside the production `CoreDaemon` drain tick so terminal
   input reaches the PTY without a Hub JSON RPC round trip.
5. Reject stale-mode input deterministically and report the outcome to the client
   through a new Core egress event.
6. Extend the published conformance harness with duplex arms.
7. Add the required `transport=duplex_binary` feature token, bump the conformance
   fixture revision, regenerate the TypeScript artifact, and prepare the
   `@trybotster/terminal-protocol` release.
8. Update `docs/architecture/terminal-adapter.md` and
   `docs/architecture/terminal-protocol.md` to the final contract.

Non-scope:

- Do not redesign Ghostty terminal semantics. Ghostty behavior is unchanged.
- Do not change the `snapshot_delivery=ready_then_history` split, attach phases,
  `GHOSTSNP` encoding, or snapshot pagination.
- Do not delete `CoreDaemon::mode_gated_input`, `CoreDaemon::input`, or the
  existing resize API in this ticket. Hub, Web, and TUI still call them until
  their own tickets land. The cold cut is `ticket_1787600679_990088`.
- Do not add DataChannel creation, labels, encryption, chunking, or the §9 limit
  table. Those are Hub-owned and belong to `ticket_1787600674_500120`.
- Do not change `TransportIngress` or `TransportEgress`. Those semantic enums stay
  until the cold-cut ticket removes the old drain path, per
  [[terminal adapter traits must not reuse TransportIngress or TransportEgress]].
- Do not add a second active terminal path and do not add a version suffix. The
  feature token is the only compatibility gate.
- Do not publish to npm from an agent session. §14 records the human publish step.
- Do not perform Web or wall-clock timing measurements.

## 5. Design

### 5.1 Ingress frame encoding is compact binary

The ticket says "Define mode-gated input and resize as Core binary input frames"
and requires the `transport=duplex_binary` token. This plan therefore defines a
new **compact binary** ingress encoding rather than reusing the JSON
`TerminalFrame` shape.

Three independent reasons, recorded so Plan Review can check the decision:

1. **Byte fidelity.** The existing `SendInput.data` is a Rust `String`. It cannot
   carry a non-UTF-8 paste or arbitrary control bytes. The ticket requires byte
   fidelity, so the input payload must be a length-prefixed raw byte run.
2. **Hot path cost.** Keystroke input is the hottest frame in the system. JSON
   with base64 inflates every keystroke and adds a parse on the ingress side.
3. **Opacity.** A fixed binary header keeps the payload a byte run that Hub has no
   reason and no accessor to parse.

Egress keeps the current JSON `TerminalFrame` encoding. Hub already carries
`TerminalFrame::to_bytes()` output as an opaque byte string, and §8.5 of the frozen
contract sends those bytes as binary DataChannel messages. Changing the egress
encoding is not required by this ticket and would break every attach consumer at
once. `transport=duplex_binary` names the duplex opaque-byte transport, not a
re-encoding of egress.

### 5.2 New Hub-safe types

Added to `crates/botster-terminal-protocol`:

- `TerminalInputFrame` — opaque binary ingress frame. `from_bytes(&[u8])`,
  `to_bytes() -> Vec<u8>`. No public accessor for session, payload, or token, so
  Hub stays content-blind in both directions.
- `TerminalInputFrameError` — typed parse failure: short header, unknown scheme
  version, unknown kind tag, declared length mismatch, or oversize payload.
- `FEATURE_TRANSPORT_DUPLEX_BINARY = "transport=duplex_binary"`.
- `MAX_TERMINAL_INPUT_FRAME_BYTES` — exact ceiling, see §5.6.

Wire layout, big-endian, fixed 4-byte header:

```
byte 0      scheme version, exact equality, value 1
byte 1      kind tag: 1 = input, 2 = mode_gated_input, 3 = resize
bytes 2..4  body length, u16, exact
body        kind-specific, see below
```

| Kind | Body |
| --- | --- |
| `input` | raw input bytes, no encoding, no escaping |
| `mode_gated_input` | `mode_generation: u64`, `mode_revision: u64`, then raw input bytes |
| `resize` | `rows: u16`, `cols: u16` |

Session identity is **not** on the wire. The frame arrives on one bound
subscription adapter, so the owner triple
`(session_id, subscription_id, generation)` is already known from the bind. This
removes a spoofing surface: a client cannot address another session through its
own subscription channel.

`Attach`, `Detach`, `SendInput`, and `Resize` request types stay exported
unchanged for the transitional period. The cold-cut ticket removes the ones that
become dead.

`PUBLIC_API_ALLOWLIST` gains the new names. The allowlist test enforces that.

### 5.3 New client-facing egress event

Added to `crates/botster-terminal-protocol-client`:

- `TerminalInputResult { subscription_id, kind, admitted, bytes_written, mode_flags, mode_freshness, rejection }`
- `TerminalInputRejection` enum: `StaleMode`, `PartialWrite`, `Malformed`,
  `QueueOverflow`, `SessionNotWritable`.
- A new `TerminalEvent` variant with wire tag `input_result`.

`EVENT_TYPES` in `crates/botster-terminal-protocol/src/frame.rs` gains
`"input_result"` so the opaque frame accepts it.

This is the deterministic client-visible answer for stale-mode rejection. Input
does **not** block on it: the client keeps sending, and the result frame arrives on
the same ordered egress stream. That satisfies "terminal input must not require a
Hub JSON RPC response before the next input" while still giving the client a
deterministic rejection signal.

Per [[terminal wire enums and TypeScript unions share one variant inventory]] the
new enums expose an `ALL` inventory and the TypeScript generator derives the union
from it.

### 5.4 Duplex adapter contract

`TerminalAdapter` gains exactly one method:

```rust
/// Take the next received ingress frame bytes, if any. Never blocks.
fn try_read(&mut self) -> Option<Vec<u8>>;
```

Contract rules, mirrored from the existing egress rules:

- Non-blocking. Returns `None` when no complete frame has arrived.
- The adapter delivers **complete frames only**, in arrival order. Transport
  reassembly (Hub chunking) happens below the adapter.
- The adapter does not decode, inspect, reorder, coalesce, or synthesize frames.
- After `close()` and after transport-side death, `try_read` returns `None`
  permanently and any buffered ingress is dropped.
- The adapter holds a **bounded** receive buffer that is transport state, not a
  policy queue. Overflow is a transport-side drop that the adapter reports through
  its existing pressure surface; it never grows without bound.

Adding a trait method is a breaking change for the two in-repo shaped adapters and
for Hub. Both `TerminalAdapterWriteError` and `TerminalAdapterPressure` stay
unchanged, so [[botster core public enums are breaking until non exhaustive is decided]]
is not triggered.

### 5.5 ClientWorker duplex ownership

`SubscriptionOwner` gains:

```rust
input_queue: VecDeque<TerminalInputCommand>,
```

New public type in `crates/botster-core/src/contract/terminal_subscription.rs`:

```rust
pub struct TerminalInputDelivery {
    pub client_id: ClientId,
    pub session_id: SessionId,
    pub subscription_id: SubscriptionId,
    pub generation: TerminalSubscriptionGeneration,
    pub command: TerminalInputCommand,
}

pub enum TerminalInputCommand {
    Input { data: Vec<u8> },
    ModeGatedInput { expected: ModeFreshnessToken, data: Vec<u8> },
    Resize { rows: u16, cols: u16 },
}
```

New method:

```rust
pub fn drain_terminal_input(&mut self)
    -> (Vec<TerminalInputDelivery>, Vec<ClientWorkerTeardown>)
```

Behavior:

1. Iterate live owners in a **rotating start order** so no single subscription can
   starve a sibling. The rotation cursor is a field on `ClientWorker`.
2. For each owner with a bound adapter, call `try_read` at most
   `INPUT_FRAMES_PER_SUBSCRIPTION_PER_TICK` times.
3. Decode each frame. A decode error is fail-closed: hard-stop that owner through
   the existing `hard_stop_key` path and return its `ClientWorkerTeardown`. A
   malformed frame is a broken or hostile client, and Core must not guess.
4. Push decoded commands onto `input_queue` up to `INPUT_QUEUE_CAPACITY`. On
   overflow, hard-stop that owner and return its teardown. Overflow closes exactly
   one subscription and touches no sibling.
5. Return the flattened deliveries in per-subscription arrival order.

Ordering guarantee: within one owner triple, decoded commands preserve exact
arrival order. Across owners there is no ordering relation, which is precisely the
isolation the project requires.

Every ingress path uses `hard_stop_key`, so ingress teardown produces the same
`ClientWorkerTeardown` rows and the same synchronous close as egress teardown, per
[[Core subscription hard-stop is synchronous close and drop on the host tick]].

### 5.6 Exact bounds

| Constant | Location | Value | Meaning |
| --- | --- | --- | --- |
| `TERMINAL_INPUT_SCHEME_VERSION` | `botster-terminal-protocol` | `1` | Exact equality on byte 0 |
| `MAX_TERMINAL_INPUT_FRAME_BYTES` | `botster-terminal-protocol` | `65_540` (4-byte header plus `u16::MAX` body) | Structural ceiling of the length field |
| `INPUT_QUEUE_CAPACITY` | `ClientWorker` | `256` commands per subscription | Bounded per-subscription ingress backlog |
| `INPUT_FRAMES_PER_SUBSCRIPTION_PER_TICK` | `ClientWorker` | `64` | Per-tick drain budget that bounds one tick and prevents sibling starvation |

`WRITE_ATTEMPT_BUDGET` stays `512` and is untouched. The frozen contract's A27b row
depends on that exact value.

### 5.7 Production path

The new input path, end to end:

```
Hub subscription channel receives binary bytes
  -> Hub reassembles chunks and hands complete frame bytes to the bound adapter
  -> adapter buffers them; Hub decodes nothing
  -> CoreDaemon::drain(session_id, last_output_at)          <- host tick, unchanged signature
       -> engine.apply_terminal_input()                     <- NEW, first step of the tick
            -> ManagedSessionRuntime::apply_client_worker_with
                 -> ClientWorker::drain_terminal_input
            -> per delivery, apply through the engine primitives:
                 Input           -> engine.write_bytes
                 ModeGatedInput  -> engine.mode_gated_pty_input   (worker atomic admit)
                 Resize          -> engine resize
            -> enqueue one input_result egress frame per delivery
       -> existing engine.drain_runtime_once
       -> existing ingest_bound_terminal_frames + pump
```

Two properties this ordering buys:

- Input applied at the **top** of the tick reaches the PTY before the same tick
  drains output, so a keystroke and its echo can complete in one tick.
- The `input_result` frame is enqueued before `pump`, so it leaves on the same
  tick through the same ordered adapter.

Re-entrancy rule: the apply step calls the **engine** primitives
(`write_bytes`, `mode_gated_pty_input`, resize) directly. It must not call the
public `CoreDaemon::mode_gated_input` wrapper, because that wrapper performs its
own pre-admit `drain_runtime_for_readback` and would re-enter `drain`. This is
recorded as risk R3.

### 5.8 Stale-mode rejection is deterministic

`ModeGatedInput` carries `(mode_generation, mode_revision)`. Core forwards it to
the existing worker atomic admit barrier
(`ManagedSessionRuntime::mode_gated_pty_input` →
`worker_process.rs:769`). The worker is the correctness boundary today and stays
so. The three outcomes map exactly:

| Worker result | `input_result` frame |
| --- | --- |
| `admitted = true`, `bytes_written = len` | `admitted = true`, no rejection |
| `admitted = false`, `bytes_written = 0` | `admitted = false`, `rejection = StaleMode` |
| `admitted = false`, `error_kind = "partial_write"`, `bytes_written > 0` | `admitted = false`, `rejection = PartialWrite`, exact `bytes_written` |

Every frame carries the post-apply `mode_flags` and `mode_freshness`, so a
rejected client re-arms from the same frame and does not need a second round trip.

### 5.9 Compatibility gate

- `FEATURE_TRANSPORT_DUPLEX_BINARY` is added to `current_feature_list()` **and** to
  `default_required_feature_list()`. It is therefore required by
  `TerminalCompatibilityRequirement::current()`.
- `CONFORMANCE_FIXTURE_REVISION` goes `1 -> 2`, because the terminal plane gains
  new request and event shapes. `PROTOCOL_VERSION` stays `1`, and `PROTOCOL` stays
  `botster-terminal-v1`, per
  [[daemon event shape changes bump conformance fixture revision not protocol version]].
- `DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION` stays `1`. The ticket names the
  feature token as **the** compatibility gate. A second redundant floor would make
  the failure diagnostic ambiguous. Recorded as assumption A4.
- No version suffix and no parallel protocol name is introduced.

## 6. Repository ownership boundaries and cross-repository dependencies

### Core owns, in this ticket

Terminal attach, snapshot delivery, input bytes, output bytes, mode-gated input,
resize, per-subscription ordering, bounded ingress and egress queues, pressure,
generation, close, recovery, and teardown. The published contract, the conformance
harness, the Rust protocol crates, and the npm package.

### Core does not own, and this ticket does not touch

| Responsibility | Owner |
| --- | --- |
| DataChannel creation, labels, generation binding, AES-GCM per-channel keys, 12 KiB chunking, the §9 limit table | botster-hub, `ticket_1787600674_500120` |
| Unix subscription binding and Unix framing | botster-hub, `ticket_1787603671_590198` |
| Entity and package-event channels | botster-hub, `ticket_1787600682_233928` |
| Deleting `DaemonRequest::SendInput`, `ModeGatedInput`, `Resize` | botster-hub, `ticket_1787600679_990088` |
| Browser binary input, Restty decode, `terminalInputQueue` removal | botster-web, `ticket_1787600676_914408` |
| TUI binary input | botster-tui, `ticket_1787603674_865638` |
| Ghostty terminal semantics | botster-terminal-ghostty, unchanged |

### Dependencies

- **Upstream, already satisfied.** `ticket_1787600670_129312` is closed. Its plan
  is the frozen contract this plan consumes. The existing dependency row
  `dependency_1787600703_379028` records it.
- **Downstream, no new dependency row required.** Every consumer ticket already
  exists on its own repository target and already states that it consumes the
  merged Core contract. This ticket registers no new dependency, because Core is
  the top of this chain and blocks on nothing else.
- **Human step inside this ticket.** The npm publish requires an operator with npm
  credentials, per
  [[Core types-only npm releases use human public publish and clean install proof]].
  The Verify step raises `project_pipelines_ask_human` for it. It is not a
  cross-repository dependency.

## 7. Affected surfaces and files

| Path | Change |
| --- | --- |
| `crates/botster-terminal-protocol/src/lib.rs` | New exports, `FEATURE_TRANSPORT_DUPLEX_BINARY`, revision bump, allowlist entries |
| `crates/botster-terminal-protocol/src/input_frame.rs` | **New.** `TerminalInputFrame`, `TerminalInputFrameError`, encode and decode |
| `crates/botster-terminal-protocol/src/compatibility.rs` | Token added to advertised and default-required lists |
| `crates/botster-terminal-protocol/src/frame.rs` | `EVENT_TYPES` gains `"input_result"` |
| `crates/botster-terminal-protocol/tests/public_api.rs` | Allowlist coverage for the new names |
| `crates/botster-terminal-protocol/tests/compatibility.rs` | Wrong-token ablation, A3 Core half |
| `crates/botster-terminal-protocol/tests/hub_shaped.rs` | Hub-shaped consumer proof of opacity in both directions |
| `crates/botster-terminal-protocol-client/src/events.rs` | `TerminalInputResult`, `TerminalInputRejection`, new `TerminalEvent` variant, `ALL` inventories |
| `crates/botster-terminal-protocol-client/src/typescript.rs` | Generator emits the new union, interface, and token |
| `crates/botster-terminal-protocol-client/tests/typescript_drift.rs` | Drift and mirror coverage for the new shapes |
| `crates/botster-terminal-protocol-client/tests/wire.rs` | Wire-tag coverage for `input_result` |
| `crates/botster-terminal-protocol-client/tests/tui_shaped.rs` | TUI-shaped consumer proof |
| `crates/botster-core/src/contract/terminal_adapter.rs` | `try_read` added, duplex contract documented |
| `crates/botster-core/src/contract/terminal_subscription.rs` | `TerminalInputCommand`, `TerminalInputDelivery` |
| `crates/botster-core/src/engine/client_worker.rs` | Ingress queue, rotation cursor, `drain_terminal_input`, fail-closed decode and overflow |
| `crates/botster-core/src/engine/managed_session_runtime.rs` | Ingress drain and apply inside the tick, `input_result` enqueue |
| `crates/botster-core/src/engine/botster.rs` | Engine-level apply entry point |
| `crates/botster-core-daemon/src/daemon.rs` | `CoreDaemon::drain` applies terminal input first |
| `crates/botster-core-test-support/src/terminal_adapter/mod.rs` | Duplex conformance arms |
| `crates/botster-core-test-support/src/terminal_adapter/{fake,unix_shaped,webrtc_shaped,core}.rs` | Drivers gain ingress injection hooks and `try_read` |
| `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/src/lib.rs` | Duplex adapter consumer proof |
| `packages/terminal-protocol/{package.json,metadata.json,terminal-protocol.ts,index.d.ts,index.js,README.md}` | Version bump, token, revision, regenerated mirror |
| `script/terminal-protocol-node-smoke.sh` | Smoke asserts the token and revision |
| `docs/architecture/terminal-adapter.md` | Duplex contract becomes living truth |
| `docs/architecture/terminal-protocol.md` | Input frame family, token, revision 2 |
| `docs/archive/plans/core-duplex-pressure-isolated-terminal-subscriptions.md` | This plan |
| `docs/reports/core-duplex-pressure-isolated-terminal-subscriptions-implement.md` | Implement report |

## 8. Assumptions

- **A1.** Egress keeps the JSON `TerminalFrame` encoding. `transport=duplex_binary`
  names the duplex opaque-byte transport, and §8.5 of the frozen contract already
  carries egress bytes as binary DataChannel messages. Re-encoding egress is not
  required by this ticket and would break every attach consumer at once.
- **A2.** Ingress frames carry no session id. The bound adapter already fixes the
  owner triple, and omitting the id removes a cross-session spoofing surface.
- **A3.** The worker atomic admit barrier stays the correctness boundary for
  mode-gated input. This ticket changes how the request arrives, not how it is
  admitted.
- **A4.** `DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION` stays `1` and the feature
  token is the sole compatibility gate, because the ticket names it as the gate.
- **A5.** Adding a `TerminalAdapter` trait method is an accepted breaking change
  for Hub. `ticket_1787600674_500120` already plans a `WebRtcTerminalAdapter`
  rewrite, and §13 of the frozen contract classifies it as rewrite.
- **A6.** `docs/archive/plans/` is the plan destination and `docs/architecture/` is
  the destination for durable contract text, from `docs/README.md` and from the
  fifteen most recent plan commits.
- **A7.** The npm publish is performed by a credentialed operator, not by an agent
  session.

## 9. Unknowns

- **U1.** The exact `bufferedAmount` reporting that Hub will expose through
  `pressure()` on the new per-channel scheme is Hub-owned. Core's contract is
  unchanged: Core reads `pressure()` and owns the policy. No Core work depends on
  the answer.
- **U2.** Whether Web will want a batched multi-keystroke input frame. Not
  required by this ticket. The current layout permits it later as a new kind tag
  under the same scheme version, so it is additive.
- **U3.** The published npm version number. `0.2.0` is the intent, since the
  package gains a required token and new shapes without a breaking removal. The
  Implement step confirms it against the registry before the human publish.

## 10. Risks

| # | Risk | Mitigation |
| --- | --- | --- |
| R1 | A flooding subscription starves siblings inside one tick. | `INPUT_FRAMES_PER_SUBSCRIPTION_PER_TICK = 64` plus a rotating start cursor. Proved by the sibling-isolation test in §12. |
| R2 | Unbounded ingress backlog grows Core memory. | `INPUT_QUEUE_CAPACITY = 256`, fail-closed hard-stop on overflow, and a structural `MAX_TERMINAL_INPUT_FRAME_BYTES` ceiling. |
| R3 | Applying input inside `drain` re-enters `drain` through `CoreDaemon::mode_gated_input`'s pre-admit readback. | Apply through engine primitives only. A test drives a mode-gated frame through `CoreDaemon::drain` and asserts a single non-re-entrant tick. |
| R4 | New code breaks the no-default-feature contract lane. | `ClientWorker` and the protocol crates stay feature-free. Only the PTY apply step sits behind `local-runtime`. The CI contract lane in §14 runs before the gate is claimed. |
| R5 | The new required token breaks in-repo consumer-shaped crates and Hub at once. | The token is the intended cold-cut gate. The in-repo shaped consumers are updated in the same commit. Hub adoption is `ticket_1787600674_500120`, which already pins Core by exact revision. |
| R6 | The TypeScript mirror drifts from the Rust inventory. | The existing `typescript_drift.rs` tests already fail on drift. New enums expose `ALL` so the generator derives the union, per [[terminal wire enums and TypeScript unions share one variant inventory]]. |
| R7 | A Core-only test is mistaken for production-shaped proof. | [[spawned Hub tests can reach only four of fourteen Core test builders]]. This plan claims Core production-path proof through `CoreDaemon::drain` with a real worker PTY, and defers live Hub proof to `ticket_1787600674_500120`. Stated explicitly in §12. |
| R8 | A malformed-frame hard-stop turns a transient decoder bug into a user-visible detach. | Fail-closed is deliberate: Core must not guess at terminal input. The `input_result` frame carries `Malformed` before the close, so the client sees the reason. |

## 11. Runtime-teardown lens answers

### `teardown_class_applies`

**Yes.** This ticket changes `ClientWorker` subscription ownership, adds an
ingress path that can create and destroy durable per-subscription state, and adds
new hard-stop triggers. It is squarely SessionIo/ClientWorker teardown work under
[[botster runtime teardown lenses]].

### `teardown_isolation`

The ownership set that dies with one failed subscription is exactly one
`SubscriptionOwner` for one `(session_id, subscription_id)` key: its egress queue,
its new ingress queue, its bound adapter, its capability set, and its snapshot-phase
entry. Nothing else.

A failure cannot take down a healthy sibling. `drain_terminal_input` collects
teardowns into a vector and continues iterating the remaining owners. Both the
decode-error path and the overflow path call `hard_stop_key` for one key. Isolation
is chosen over any shared resource: there is no shared ingress buffer, no shared
decoder state, and no shared cursor beyond the rotation index, which is a `usize`
that no failure can corrupt.

### `teardown_bounds`

- `try_read` is contractually non-blocking, exactly like `try_write`. A blocking
  `try_read` is an illegal adapter and fails the published conformance harness.
- The ingress drain per tick is bounded by
  `INPUT_FRAMES_PER_SUBSCRIPTION_PER_TICK` × live subscriptions. It cannot spin.
- The ingress queue is bounded by `INPUT_QUEUE_CAPACITY`. Overflow is fail-closed,
  not a wait.
- `close()` stays synchronous and non-blocking and now also drops buffered ingress.
  Core still calls `close()` on the host tick and still spawns no closer thread.
- The hard stop that ends the path is the existing `hard_stop_key` →
  `ClientWorkerTeardown` → `TransportIngress::UnsubscribeSession` sequence in
  `apply_client_worker_with`. It is unchanged and now also serves ingress failures.

### `late_message_matrix`

Every ingress surface that can create or destroy durable ownership:

| Message | Owner tag | Rejection after terminal failure | Residual sweep |
| --- | --- | --- | --- |
| `Input` frame | Owner triple from the bind; the frame carries no id | Adapter is dropped with the owner, so `try_read` is never called for a dead owner | Buffered bytes die with the adapter on `hard_stop_key` |
| `ModeGatedInput` frame | Same | Same, plus a stale `(mode_generation, mode_revision)` is rejected by the worker barrier even if the owner is live | Same |
| `Resize` frame | Same | Same | Same |
| Malformed frame | Same | Decode failure hard-stops that owner only | Teardown row emitted on the same tick |
| Ingress overflow | Same | Push refused, owner hard-stopped | Queue dropped with the owner |
| `bind_terminal_adapter` | `(client_id, session_id, subscription_id, generation)` | Unchanged. A stale generation closes and drops the adapter and returns `StaleGeneration`, per [[Core ClientWorker bind requires a live attach generation]] | Unchanged |
| `record_attach` replacement | Same | Unchanged. Replacement hard-stops the previous owner and starts generation N+1 | The previous owner's **ingress** queue is now also dropped by that same hard stop |
| `detach_generation` | Same | Unchanged. Generation N detach does not delete generation N+1 | Ingress queue dropped with the owner |
| `teardown_session` / `teardown_all` | Session or global | Unchanged | Ingress queues dropped with their owners |

The matrix guards every ownership-creating ingress surface, not only the input
frame. That is the completeness rule the lens note requires.

### `production_path_proof`

Exact production path, named end to end:

```
CoreDaemon::drain(session_id, last_output_at)
  -> engine.apply_terminal_input(...)
  -> ManagedSessionRuntime::apply_client_worker_with
  -> ClientWorker::drain_terminal_input
  -> adapter.try_read
  -> TerminalInputFrame::from_bytes
  -> engine.write_bytes | engine.mode_gated_pty_input | engine resize
  -> real worker PTY
```

Live oracles, not helper calls:

1. **Byte oracle.** A real `botster-session-worker` PTY runs `cat`. Input frames
   enter through a bound adapter. The echoed bytes come back through the same
   adapter as `terminal_output`, byte-exact and in order.
2. **Teardown oracle.** After a malformed frame or an overflow,
   `CoreDaemon::list_terminal_subscriptions` and
   `CoreDaemon::terminal_subscription_generation` both report the route gone, and
   the adapter's `pressure()` is `Closed`. Inventory absence is the live oracle,
   not a log line.
3. **Red-on-revert control.** Removing the ingress drain call from
   `apply_client_worker_with` must make the byte oracle the first failure. The
   Implement step runs that ablation and records the failing test name.
4. **No-spin control.** With one subscription flooded and one idle, the idle
   subscription's tick count stays bounded by the drain budget. This is a
   deterministic counter assertion, not a wall-clock sample.

This ticket does **not** claim live Hub proof. Per
[[spawned Hub tests can reach only four of fourteen Core test builders]], a Core
builder is not production-shaped Hub proof. Live Hub proof is A23 and belongs to
`ticket_1787600674_500120`.

### `ownership_identity`

Every durable ingress row is keyed by the existing owner triple
`(session_id, subscription_id, generation)`, per
[[Core terminal subscription ownership is session, subscription, and generation]].
The ingress queue lives **inside** `SubscriptionOwner`, so it is created and
destroyed with the generation and cannot outlive it.

Reused-id policy is unchanged and now covers ingress: when Core reuses a
`subscription_id` after teardown it increments the generation, and a delayed close
or detach for generation N never deletes generation N+1. Because the ingress queue
is a field of the owner rather than a side table keyed by `subscription_id`, a
delayed teardown of generation N structurally cannot reach generation N+1's
ingress. There is no separate sweep to get wrong.

Owner sweeps cover both queue orders. "Closed first": the owner is gone, so
`drain_terminal_input` never visits it and its buffered bytes died with the
adapter. "Message first": the frame is decoded and applied under the live
generation, then the close removes the owner on the same or a later tick.

### `sibling_fail_closed_policy`

- **On successful close.** Siblings keep working. Egress, ingress, and the rotation
  cursor are per-owner or index-only. A sibling-isolation test asserts a second
  subscription still delivers input and receives output after the first is torn
  down.
- **On ultimate failure.** Core has no ultimate-close failure mode here, because
  `close()` is contractually non-blocking and Core never waits on it. A misbehaving
  adapter that blocks in `close()` is an illegal adapter and fails the published
  conformance harness before it reaches production. No sibling is sacrificed at the
  Core layer. Bounded peer-close sibling sacrifice remains a Hub-owned policy, per
  [[Hub ultimate WebRTC close failure sacrifices every peer on the dedicated runtime]].
- **Blast radius.** One malformed frame, one overflow, or one decode failure closes
  exactly one subscription. A test asserts the sibling count is unchanged after
  each of the three failures.

## 12. Acceptance checks and tests

### Conformance harness, published

`assert_terminal_adapter_conformance` gains duplex arms. Every arm is deterministic
and driver-hook based, with no sleeps:

| Arm | Claim |
| --- | --- |
| `assert_ingress_ready` | A fresh adapter returns `None` from `try_read` |
| `assert_ingress_order` | Three injected frames come back in exact arrival order |
| `assert_ingress_whole_frames` | A partially injected frame is not returned until complete |
| `assert_ingress_closed_local` | After `close()`, `try_read` is permanently `None` and buffered ingress is dropped |
| `assert_ingress_closed_transport` | Same after transport-side death |
| `assert_ingress_content_blind` | The adapter returns the exact injected bytes and does not decode them |
| `assert_ingress_non_blocking` | `try_read` returns on an empty buffer without a driver hook |

### Frozen-contract acceptance rows owned here

| Row | Test | Location |
| --- | --- | --- |
| **A1** — `TerminalAdapter` carries ingress and egress | Duplex conformance arms pass for the fake, Unix-shaped, WebRTC-shaped, and hub-adapter-shaped consumer adapters | `botster-core-test-support`, consumer crate |
| **A2** — Core rejects stale-mode input deterministically | A stale `(mode_generation, mode_revision)` in a `mode_gated_input` frame yields `admitted = false`, `bytes_written = 0`, `rejection = StaleMode`, and zero PTY bytes | `botster-core-daemon` integration, real worker PTY |
| **A3, Core half** — the token is required | `ensure_compatible` fails with the typed diagnostic when the advertised set omits `transport=duplex_binary`; passes when present | `botster-terminal-protocol/tests/compatibility.rs` |
| **A24, Core half** — consumers can contain the contract | `metadata.json` and `terminal-protocol.ts` carry the token and revision 2; the node smoke resolves the published root export | `typescript_drift.rs`, `script/terminal-protocol-node-smoke.sh` |

### Deterministic conformance tests required by the ticket

| Claim | Test |
| --- | --- |
| Ordering | N input frames on one subscription reach the PTY in exact order |
| Byte fidelity | A non-UTF-8 byte run and a large paste round-trip byte-exact through the PTY echo |
| Stale-mode rejection | A2 above, plus a fresh token immediately after a rejection is admitted |
| Resize | A `resize` frame changes worker geometry and the registry size follows the worker-applied resize |
| Queue bounds | `INPUT_QUEUE_CAPACITY + 1` commands hard-stop exactly one subscription and emit one teardown |
| Pressure | Egress `WouldBlock` and `Full` behavior is unchanged while ingress keeps draining, so backpressure in one direction does not stall the other |
| Reconnect | Detach, re-attach with generation N+1, bind a fresh adapter, and prove input flows on N+1 |
| Stale generation | A frame buffered in a generation-N adapter never reaches the PTY after N+1 exists |
| Teardown | After `teardown_session`, ingress queues are gone and inventory reports zero rows |
| Sibling isolation | One subscription floods to overflow and is torn down; a sibling keeps sending input and receiving output |
| Non-blocking input | N input frames applied across ticks require zero host control responses; the assertion counts control-plane round trips at exactly zero |
| Ready-then-history preserved | Input is accepted after `READY` and before `FINISH`, and the existing attach-phase order is unchanged |
| Ghostty semantics preserved | The existing Ghostty attach, snapshot, and mode suites pass unchanged |

### Downstream-shaped proof, required by the charter

Crate-local tests alone are insufficient for a public contract change.

- `crates/botster-terminal-protocol/tests/hub_shaped.rs` — a Hub-shaped consumer
  forwards ingress and egress bytes with no semantic accessor available in either
  direction.
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/` — an
  out-of-workspace consumer implements the duplex trait and passes the published
  conformance harness. Run with its own direct Cargo command, because a workspace
  filter does not run a nonmember crate.
- `crates/botster-terminal-protocol-client/tests/tui_shaped.rs` — a TUI-shaped
  consumer encodes input frames and decodes `input_result`.
- `script/terminal-protocol-node-smoke.sh` — clean-directory install of the exact
  published coordinate resolves the token and revision.

### Not proved here

- Live Hub, browser, or TUI transport behavior. Those are A4 through A17 and A23,
  owned by the Hub, Web, and TUI tickets.
- Wall-clock latency. `ticket_1787603669_760394` owns the observation format and
  `ticket_1787600679_990088` owns the post-cut comparison.

## 13. Commit shape

1. Plan commit — this document.
2. Protocol commit — the two Rust protocol crates, the token, revision 2, the
   TypeScript regeneration, and the package mirror.
3. Contract commit — `TerminalAdapter::try_read`, the conformance arms, and the
   shaped drivers.
4. Runtime commit — `ClientWorker` ingress ownership and the `CoreDaemon::drain`
   apply step, with its tests.
5. Docs commit — `docs/architecture/` living truth and the Implement report.

No forwarding wrapper is left behind. `mode_gated_input` and `input` remain on
`CoreDaemon` as the transitional host API until the cold-cut ticket removes them;
that retention is required by the frozen contract §4 and is not a wrapper around
the new path.

## 14. Gate commands

`botster-core` has no test wrapper. The CI-owned commands, per
[[botster-core uses CI-owned Cargo commands because it has no test script]] and
[[botster-core CI runs a contract only test lane because workspace feature unification hides breaks]]:

```sh
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
cargo test -p botster-core --no-default-features --lib
BOTSTER_ENV=test cargo test --doc --workspace
cargo doc --workspace --no-deps
script/terminal-protocol-node-smoke.sh
BOTSTER_ENV=test cargo test -p botster-core --test local_process_runtime_test
```

Plus the out-of-workspace consumer crates by their own direct Cargo commands.

Release step, human-owned, after merge:

1. An operator runs `npm publish --access public` from `packages/terminal-protocol`
   in a credentialed shell. No npm token is added to the repository and no publish
   workflow is added.
2. Verification runs `npm view @trybotster/terminal-protocol version` and installs
   the exact published coordinate in a clean directory.

## 15. Vault gaps worth capturing

Capture only after this ticket proves the contract, not at Plan time:

1. **The Core terminal adapter is duplex and Hub stays content-blind in both
   directions.** [[core owns duplex terminal transport while Hub stays content blind]]
   states the decision; the shipped `try_read` contract will be the concrete note.
2. **Core ingress frames use a compact binary header while egress stays JSON.**
   The asymmetry is deliberate and will confuse a future reader without a note.
3. **A malformed or overflowing ingress frame is fail-closed to one subscription.**
   The blast-radius rule belongs beside
   [[Core subscription hard-stop is synchronous close and drop on the host tick]].
4. **Ingress queues live inside `SubscriptionOwner`, so generation reuse needs no
   separate ingress sweep.** This strengthens
   [[Core terminal subscription ownership is session, subscription, and generation]].
5. **A required feature token is the terminal-plane cold-cut gate, and the
   conformance floor stays put.** This refines
   [[daemon event shape changes bump conformance fixture revision not protocol version]].

No gap beyond these five was found.
