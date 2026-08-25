# Core: make terminal subscriptions duplex and pressure-isolated

Ticket `ticket_1787600672_342292`. Run `run_1787632374_189517`. Step `botster_stack_plan`.

Revision 2. Plan Review `review_1787634119_893294` returned `changes_required` with four
product findings and one missing-context finding. Section 19 maps each finding to the
section that resolves it.

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

### Required Botster maps

`botster-planner-playbook` requires these three. Revision 1 of this plan omitted them.

- [[botster-architecture]] — confirms the Core/Hub split this plan applies, and lists
  [[core owns duplex terminal transport while Hub stays content blind]],
  [[botster data plane bypasses the hub through session and client actors]], and
  [[botster subscriptions use dedicated ordered DataChannels]] as current decisions.
- [[cli-patterns]] — mixed-generation index. Read for the release-chain and
  fixture-revision entries. It is not ownership authority; the repository playbook is.
- [[spa-patterns]] — read for the Web consumer of the published package.
  [[restty is a client renderer not authoritative terminal infrastructure]] and
  [[terminal attach size has one client side owner]] confirm that terminal truth stays
  in Core and that the client owns the desired size it sends. Both support carrying
  resize as a Core input frame. Neither changes a Core decision in this plan.

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
- [[conformance fixture revisions must be unique per published content]]
- [[shared conformance fixtures that contradict the core contract teach clients the wrong state machine]]
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
- [[registry integrity compared against a pack of the intended commit retires stale tree publish risk]]
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
| `crates/botster-core/src/engine/client_worker.rs` | `ClientWorker` owns `(session_id, subscription_id)` → `SubscriptionOwner { client_id, generation, adapter, capabilities, queue, … }`. `WRITE_ATTEMPT_BUDGET = 512`. `pump()` is the per-tick egress pump. Already depends on `botster-terminal-protocol-client`. |
| `crates/botster-core/src/engine/managed_session_runtime.rs:975` | `apply_client_worker_with` is the single tick site that calls `ingest_bound_terminal_frames` then `pump`. |
| `crates/botster-core/src/runtime/worker_process.rs:769` | `mode_gated_pty_input` **blocks**: it writes the request, then loops on `pump_session_output` with `thread::sleep(GATED_POLL)` (5 ms) until a matching reply, reader finish, or `parent_wait_deadline`. |
| `crates/botster-core/src/runtime/worker_process.rs:46,51` | `DEFAULT_MODE_GATED_INPUT_TIMEOUT = 5 s`; `MODE_GATED_REPLY_GRACE = 1 s`. Worst-case block is 6 s. |
| `crates/botster-core/src/runtime/worker_process.rs:101,159` | `test_mode_gated_hold_ms` already exists and is forwarded to the worker as `test_hold_ms`. It is a deterministic hold hook for tests. |
| `crates/botster-core/src/runtime/worker_process.rs:784` | `gated_in_flight` is a **per-session** slot. A second concurrent gated call for the same session is rejected, not queued. |
| `crates/botster-core-daemon/src/daemon.rs:1088` | `CoreDaemon::drain` is the per-session tick that hosts call. |
| `crates/botster-core-daemon/src/daemon.rs:1240` | `CoreDaemon::mode_gated_input` is today a host-called JSON-RPC-shaped method that wraps the blocking call. |
| `crates/botster-terminal-protocol` | Hub-safe crate. Opaque `TerminalFrame`, `Attach`, `Detach`, `SendInput`, `Resize`, compatibility, `PUBLIC_API_ALLOWLIST`. |
| `crates/botster-terminal-protocol-client` | Semantic crate. Depends on the Hub-safe crate and re-exports its types. Owns `Snapshot`, `AttachState`, `TerminalOutput`, `ProcessExit`, and the TypeScript generator. |
| `packages/terminal-protocol` | `@trybotster/terminal-protocol` `0.1.0`. |
| `crates/botster-core-test-support/src/terminal_adapter/` | Transport-neutral conformance harness plus Unix-shaped, WebRTC-shaped, and out-of-workspace hub-adapter-shaped consumers. |
| `.github/workflows/ci.yml` | The CI-owned gate commands recorded in §14. |
| `docs/README.md`, `docs/plans/README.md` | `docs/plans/` is a retired stub. Landed plans live in `docs/archive/plans/`. Living truth lives in `docs/architecture/`. |

### Published release identity, verified

Required by [[conformance fixture revisions must be unique per published content]].
Checked against the live registry at plan time:

```
npm view @trybotster/terminal-protocol versions --json   ->  ["0.1.0"]
npm pack @trybotster/terminal-protocol@0.1.0             ->  metadata.json:
    package_version 0.1.0, protocol botster-terminal-v1, protocol_version 1,
    conformance_fixture_revision 1,
    features [terminal_streaming, resize, snapshot_delivery=ready_then_history]
```

Publication history contains exactly one coordinate and exactly one meaning for
revision 1. No published artifact carries `transport=duplex_binary`. Revision **2**
is therefore strictly above every published meaning, and package version **0.2.0**
is unallocated. §14 re-runs this check immediately before the human publish, because
a concurrent release could consume either value between now and then.

Worktree hygiene: tracked `.gitignore` is 63 bytes and matches HEAD. The worktree
path contains no `:`. No `CARGO_TARGET_DIR` override is required.

## 4. Scope

In scope:

1. Add an **opaque binary ingress carrier** to the Hub-safe protocol crate, and the
   **semantic encode and decode API** to the client protocol crate.
2. Extend `TerminalAdapter` from egress-only to **duplex**: add a non-blocking
   ingress poll alongside `try_write`, `close`, and `pressure`.
3. Give `ClientWorker` a **bounded, per-subscription ingress queue** with separate
   intake and apply stages, exact budgets, and fail-closed overflow.
4. Add a **non-blocking submit-and-poll** mode-gated input path so one subscription
   cannot stall a sibling, and apply decoded input inside the production
   `CoreDaemon` drain tick.
5. Report per-command input outcomes to the client through a new Core egress event,
   including deterministic stale-mode rejection.
6. Extend the published conformance harness with duplex arms.
7. Add the required `transport=duplex_binary` feature token, allocate conformance
   fixture revision 2 and package version 0.2.0, regenerate the TypeScript artifact,
   and prepare the `@trybotster/terminal-protocol` release.
8. Update `docs/architecture/terminal-adapter.md` and
   `docs/architecture/terminal-protocol.md` to the final contract.

Non-scope:

- Do not redesign Ghostty terminal semantics. Ghostty behavior is unchanged.
- Do not change the `snapshot_delivery=ready_then_history` split, attach phases,
  `GHOSTSNP` encoding, or snapshot pagination.
- Do not delete `CoreDaemon::mode_gated_input`, `CoreDaemon::input`, or the
  existing resize API in this ticket. Hub, Web, and TUI still call them until
  their own tickets land. The cold cut is `ticket_1787600679_990088`.
- Do not change the worker-side atomic admit protocol. §5.8 decomposes the
  **parent-side wait** only. `FRAME_MODE_GATED_PTY_INPUT`, the request and result
  payloads, and the worker deadline fence are unchanged.
- Do not add DataChannel creation, labels, encryption, chunking, or the §9 limit
  table. Those are Hub-owned and belong to `ticket_1787600674_500120`.
- Do not change `TransportIngress` or `TransportEgress`, per
  [[terminal adapter traits must not reuse TransportIngress or TransportEgress]].
- Do not add a second active terminal path and do not add a version suffix.
- Do not publish to npm from an agent session.
- Do not perform Web or wall-clock timing measurements.

## 5. Design

### 5.1 Ingress frame encoding is compact binary

The ticket says "Define mode-gated input and resize as Core binary input frames"
and requires the `transport=duplex_binary` token. This plan therefore defines a
new **compact binary** ingress encoding rather than reusing the JSON
`TerminalFrame` shape.

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

### 5.2 Crate split: opaque carrier and semantic API

Revision 1 gave `TerminalInputFrame` only `from_bytes` and `to_bytes` and forbade
every accessor. That is unimplementable: Core must decode, and clients must encode.
The fix mirrors the shipped egress arrangement in
[[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
exactly. **No new crate dependency edge is introduced.** The client crate already
depends on the Hub-safe crate, and `botster-core` already depends on the client
crate (`client_worker.rs` imports `botster_terminal_protocol_client`).

**`botster-terminal-protocol` — Hub-safe, opaque carrier.**

```rust
pub struct TerminalInputFrame { /* private Vec<u8> */ }

impl TerminalInputFrame {
    /// Validate the header only. Does not decode the body.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TerminalInputFrameError>;
    /// Emit the exact wire bytes for forwarding.
    pub fn to_bytes(&self) -> Vec<u8>;
    /// Borrow the exact wire bytes for forwarding without a copy.
    pub fn as_bytes(&self) -> &[u8];
}
```

`from_bytes` validates the scheme version, that the kind tag is one of the three
known values, and that the declared body length equals the remaining byte count. It
does **not** interpret the body. There is no accessor for the payload, the freshness
token, rows, or cols. Hub can construct, forward, and emit a frame and can learn
nothing else, so Hub remains content-blind on the ingress direction exactly as it
is on the egress direction.

**`botster-terminal-protocol-client` — semantic encode and decode.**

```rust
pub enum TerminalInputCommand {
    Input { data: Vec<u8> },
    ModeGatedInput { expected: ModeFreshnessToken, data: Vec<u8> },
    Resize { rows: u16, cols: u16 },
}

pub fn encode_terminal_input(command: &TerminalInputCommand) -> TerminalInputFrame;
pub fn decode_terminal_input(frame: &TerminalInputFrame)
    -> Result<TerminalInputCommand, TerminalInputDecodeError>;
```

Core calls `decode_terminal_input`. TUI and Web-shaped consumers call
`encode_terminal_input`. Hub depends on neither function, because Hub does not
depend on the client crate. `ModeFreshnessToken` is re-exported by the client crate
from `botster-core`'s existing public `contract::session_protocol` definition, so
the token has one definition and no mirror.

`encode_terminal_input` is infallible: `Vec<u8>` longer than the `u16` body budget
is split by the caller, and §5.6 fixes that ceiling. `decode_terminal_input` returns
a typed error for a truncated body, an unknown kind tag, or a body length that does
not match the kind's fixed prefix.

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

`Attach`, `Detach`, `SendInput`, and `Resize` request types stay exported unchanged
for the transitional period. The cold-cut ticket removes the ones that become dead.
`PUBLIC_API_ALLOWLIST` gains the new Hub-safe names, and its existing test enforces
that the allowlist and the public surface agree.

### 5.3 Client-facing input result

Added to `crates/botster-terminal-protocol-client`:

```rust
pub struct TerminalInputResult {
    pub subscription_id: String,
    pub kind: TerminalInputKind,        // input | mode_gated_input | resize
    pub admitted: bool,
    pub bytes_written: usize,
    pub mode_flags: ModeFlags,
    pub mode_freshness: ModeFreshnessToken,
    pub rejection: Option<TerminalInputRejection>,
}

pub enum TerminalInputRejection {
    StaleMode,
    PartialWrite,
    Timeout,
    SessionNotWritable,
}
```

A new `TerminalEvent` variant carries wire tag `input_result`, and `EVENT_TYPES` in
`crates/botster-terminal-protocol/src/frame.rs` gains `"input_result"`.

**Every listed variant is reachable on a live owner.** Revision 1 also listed
`Malformed` and `QueueOverflow`. Both are removed. Those two conditions hard-stop
the owner, and `hard_stop_key` closes and drops the same adapter that would have
carried the result. Existing contract text is explicit that close abandons an
in-flight frame and that an accepted write is not a delivered write, per
[[adapter accepted writes are not consumer flushed writes]]. A result frame enqueued
immediately before that close is unobservable, so promising it would be a false
claim.

The fail-closed report is therefore the **close itself**: the adapter reports
`Closed`, and the host observes the `ClientWorkerTeardown` row. That is the same
signal every other Core hard-stop already uses, it needs no new retain state
machine, and it does not weaken any product proof. §12 asserts the close, not a
phantom frame.

Input never blocks on a result. The client keeps sending, and results arrive on the
same ordered egress stream. That satisfies "terminal input must not require a Hub
JSON RPC response before the next input" while still giving a deterministic
stale-mode signal.

Per [[terminal wire enums and TypeScript unions share one variant inventory]], the
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
  policy queue. Overflow is a transport-side drop reported through the existing
  pressure surface; it never grows without bound.

Adding a trait method is breaking for the shaped adapters and for Hub.
`TerminalAdapterWriteError` and `TerminalAdapterPressure` are unchanged, so
[[botster core public enums are breaking until non exhaustive is decided]] is not
triggered.

### 5.5 ClientWorker ingress: two stages, exact lifecycle

Revision 1 conflated intake and apply in one call, which made the capacity bound
unreachable. Intake and apply are now separate stages with separate budgets. Intake
is cheap (a memcpy and a header decode). Apply is expensive (a PTY write or a worker
round trip). Distinct budgets are what makes the queue a real queue.

`SubscriptionOwner` gains:

```rust
input_queue: VecDeque<TerminalInputCommand>,   // capacity INPUT_QUEUE_CAPACITY
awaiting_gated: Option<GatedWait>,             // set while this owner has a gated request in flight
```

`ClientWorker` gains `input_cursor: usize`, a rotation index.

**Stage A — intake.** `ClientWorker::intake_terminal_input() -> Vec<ClientWorkerTeardown>`

1. Visit live owners starting at `input_cursor`, then advance the cursor by one.
2. For each owner with a bound adapter, call `try_read` at most
   `INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK` times.
3. Decode each frame with `decode_terminal_input`. A decode error is fail-closed:
   hard-stop that owner through `hard_stop_key`, return its teardown, and stop
   intake for that owner. A malformed frame is a broken or hostile client, and Core
   must not guess.
4. Push each decoded command onto `input_queue`. **Enqueue rule:** push only while
   `input_queue.len() < INPUT_QUEUE_CAPACITY`. The push that would exceed the
   capacity is refused; Core hard-stops that owner and returns its teardown. Nothing
   is enqueued past the bound, so the bound is exact rather than approximate.

**Stage B — apply.** `ClientWorker::take_terminal_input() -> Vec<TerminalInputDelivery>`

1. Visit live owners in the same rotation order.
2. Pop from the **front** of `input_queue`, in order, at most
   `APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK` commands for that owner.
   **Dequeue rule:** a command is removed from the queue exactly when it is returned
   as a delivery. A returned command is never replayed, because it no longer exists
   in the queue.
3. Stop applying for that owner as soon as one of these holds, and leave the rest
   queued for a later tick:
   - the per-owner apply budget is spent;
   - the head command is `ModeGatedInput` and this owner or another owner on the
     same session already has a gated request in flight (§5.8).

Ordering: within one owner, commands are dequeued strictly in arrival order, so a
plain `Input` is never reordered ahead of a stalled `ModeGatedInput` and byte order
holds. Across owners there is no ordering relation, which is exactly the isolation
the project requires.

**Why overflow is now reachable.** Intake admits up to 64 frames per owner per
tick, apply removes at most 16. A client that sustains more than 16 commands per
tick grows its own queue by up to 48 per tick and crosses 256 within six ticks. The
257th enqueue attempt trips the bound. §12 drives that through the production
`CoreDaemon::drain` loop rather than by calling a helper.

Every ingress failure path uses `hard_stop_key`, so ingress teardown produces the
same `ClientWorkerTeardown` rows and the same synchronous close as egress teardown,
per [[Core subscription hard-stop is synchronous close and drop on the host tick]].

### 5.6 Exact bounds

| Constant | Location | Value | Meaning |
| --- | --- | --- | --- |
| `TERMINAL_INPUT_SCHEME_VERSION` | `botster-terminal-protocol` | `1` | Exact equality on byte 0 |
| `MAX_TERMINAL_INPUT_BODY_BYTES` | `botster-terminal-protocol` | `65_535` | `u16` body ceiling |
| `MAX_TERMINAL_INPUT_FRAME_BYTES` | `botster-terminal-protocol` | `65_539` | 4-byte header plus the body ceiling |
| `INPUT_QUEUE_CAPACITY` | `ClientWorker` | `256` commands per subscription | Bounded per-subscription backlog |
| `INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK` | `ClientWorker` | `64` | Stage A budget |
| `APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK` | `ClientWorker` | `16` | Stage B budget; bounds per-tick PTY work |

`WRITE_ATTEMPT_BUDGET` stays `512` and is untouched. Frozen-contract row A27b depends
on that exact value.

### 5.7 Production path

```
Hub subscription channel receives binary bytes
  -> Hub reassembles chunks and hands complete frame bytes to the bound adapter
  -> adapter buffers them; Hub decodes nothing
  -> CoreDaemon::drain(session_id, last_output_at)          <- host tick, unchanged signature
       -> engine.apply_terminal_input()                     <- NEW, first step of the tick
            1. poll any in-flight gated reply for this session (§5.8), enqueue its input_result
            2. ClientWorker::intake_terminal_input
            3. ClientWorker::take_terminal_input
            4. per delivery, apply through the engine primitives:
                 Input           -> engine.write_bytes
                 ModeGatedInput  -> engine.submit_mode_gated_pty_input   (non-blocking)
                 Resize          -> engine resize
            5. enqueue one input_result egress frame per completed outcome
       -> existing engine.drain_runtime_once
       -> existing ingest_bound_terminal_frames + pump
```

Input applied at the **top** of the tick reaches the PTY before the same tick drains
output, so a keystroke and its echo can complete in one tick. Results are enqueued
before `pump`, so they leave on the same tick through the same ordered adapter.

**Re-entrancy.** The apply step calls engine primitives directly. It must not call
the public `CoreDaemon::mode_gated_input` wrapper, because that wrapper performs its
own pre-admit `drain_runtime_for_readback` and would re-enter `drain`. Recorded as
risk R3.

**Per-command error isolation.** An apply error for one delivery must never abort
the shared drain. Each apply is matched individually:

| Apply outcome | Effect |
| --- | --- |
| Success | `input_result` with `admitted = true` and exact `bytes_written` |
| Worker rejects on a stale token | `input_result` with `rejection = StaleMode` |
| Partial write | `input_result` with `rejection = PartialWrite` and nonzero `bytes_written` |
| Gated wait exceeds its deadline | `input_result` with `rejection = Timeout` |
| Session is not writable | `input_result` with `rejection = SessionNotWritable` |
| Runtime error for that session | hard-stop **that owner only**, emit its teardown, continue the loop |

`apply_terminal_input` returns `Ok` whenever the tick itself completed, even when
individual commands failed. Only a failure of the tick machinery propagates.

### 5.8 Non-blocking mode-gated input

**The defect.** `worker_process::mode_gated_pty_input` writes the request and then
loops on `pump_session_output` with a 5 ms sleep until a matching reply, reader
finish, or `parent_wait_deadline`, which is `mode_gated_input_timeout` (5 s default)
plus `MODE_GATED_REPLY_GRACE` (1 s). A single unresponsive worker therefore blocks
the calling thread for up to **6 seconds**. Calling that inline from the shared drain
tick would let one subscription stall every sibling, which directly violates the
ticket requirement that a slow subscription must not block another subscription.
A frame-count budget does not bound a wall-clock sleep, so revision 1's budget was
not an answer to this.

**The fix: decompose the parent-side wait, leave the worker protocol alone.**

```rust
// New. Writes FRAME_MODE_GATED_PTY_INPUT, claims gated_in_flight, returns at once.
fn submit_mode_gated_pty_input(&mut self, session_id, expected, data)
    -> Result<GatedRequestId, SessionRuntimeError>;

// New. Pumps output once, checks the slot, checks the deadline. Never sleeps.
fn poll_mode_gated_pty_input(&mut self, session_id)
    -> Result<GatedPoll, SessionRuntimeError>;

enum GatedPoll { Idle, Pending, Ready(ModeGatedPtyInputResult), TimedOut }
```

The existing blocking `mode_gated_pty_input` is **kept** and reimplemented as
`submit` followed by the same poll loop it has today. The legacy
`CoreDaemon::mode_gated_input` JSON path therefore keeps byte-identical behavior
until the cold-cut ticket deletes it. This is a decomposition, not a rewrite: the
loop body already is `pump_session_output` plus a slot check plus a deadline check.

The worker side is untouched. `FRAME_MODE_GATED_PTY_INPUT`, the request and result
payloads, the worker deadline fence, and the atomic admit barrier are unchanged, so
the correctness boundary stays exactly where it is today.

**What still serializes, and why that is correct.** `gated_in_flight` is a
per-session slot and the worker admits one gated request per session at a time. The
PTY is one device, so gated input for one session is inherently serial. The apply
stage therefore stalls the owner whose head command is `ModeGatedInput` while that
session has a request in flight, and leaves the command queued. It stalls **nothing
else**: other owners on the same session continue to apply plain `Input` and
`Resize`, owners on other sessions are untouched, egress `pump` continues for every
owner, and no thread sleeps. Head-of-line blocking is confined to one owner's queue
and is bounded by the gated deadline, after which `poll` returns `TimedOut` and the
owner resumes with a `Timeout` result.

**Deterministic proof, no sleeps.** `test_mode_gated_hold_ms` already exists and is
forwarded to the worker as `test_hold_ms`. §12 uses it to hold one reply while a
sibling on another session sends input and receives echoed output, then releases it.
That is a real production-path hold, not a mocked stall.

### 5.9 Compatibility gate and release identity

- `FEATURE_TRANSPORT_DUPLEX_BINARY = "transport=duplex_binary"` is added to
  `current_feature_list()` **and** to `default_required_feature_list()`, so
  `TerminalCompatibilityRequirement::current()` requires it.
- `CONFORMANCE_FIXTURE_REVISION` goes `1 -> 2`. `PROTOCOL_VERSION` stays `1` and
  `PROTOCOL` stays `botster-terminal-v1`, per
  [[daemon event shape changes bump conformance fixture revision not protocol version]].
- Revision 2 is unique across publication history. §3 records the verified evidence:
  the registry holds only `0.1.0`, and that artifact carries revision 1 and three
  features. §14 repeats the check immediately before the human publish.
- Package version goes `0.1.0 -> 0.2.0`: the package gains a required token and new
  shapes without a breaking removal.
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
| `crates/botster-terminal-protocol/src/input_frame.rs` | **New.** `TerminalInputFrame` opaque carrier, header validation, `TerminalInputFrameError` |
| `crates/botster-terminal-protocol/src/compatibility.rs` | Token added to advertised and default-required lists |
| `crates/botster-terminal-protocol/src/frame.rs` | `EVENT_TYPES` gains `"input_result"` |
| `crates/botster-terminal-protocol/tests/public_api.rs` | Allowlist coverage; asserts no semantic accessor exists on the carrier |
| `crates/botster-terminal-protocol/tests/compatibility.rs` | Wrong-token ablation, A3 Core half |
| `crates/botster-terminal-protocol/tests/hub_shaped.rs` | Hub-shaped forward-only proof in both directions |
| `crates/botster-terminal-protocol-client/src/input.rs` | **New.** `TerminalInputCommand`, `encode_terminal_input`, `decode_terminal_input`, `TerminalInputDecodeError` |
| `crates/botster-terminal-protocol-client/src/events.rs` | `TerminalInputResult`, `TerminalInputKind`, `TerminalInputRejection`, new `TerminalEvent` variant, `ALL` inventories |
| `crates/botster-terminal-protocol-client/src/typescript.rs` | Generator emits the new unions, interfaces, encode helpers, and token |
| `crates/botster-terminal-protocol-client/tests/{typescript_drift,wire,tui_shaped}.rs` | Drift, mirror, wire-tag, and TUI-shaped encode/decode coverage |
| `crates/botster-core/src/contract/terminal_adapter.rs` | `try_read` added, duplex contract documented |
| `crates/botster-core/src/contract/terminal_subscription.rs` | `TerminalInputDelivery`, re-export of `TerminalInputCommand` |
| `crates/botster-core/src/engine/client_worker.rs` | Ingress queue, rotation cursor, `intake_terminal_input`, `take_terminal_input`, `awaiting_gated`, fail-closed decode and overflow |
| `crates/botster-core/src/runtime/worker_process.rs` | `submit_mode_gated_pty_input`, `poll_mode_gated_pty_input`, `GatedPoll`; existing blocking method reimplemented on top of them |
| `crates/botster-core/src/engine/managed_session_runtime.rs` | Ingress stages and apply inside the tick, per-command error isolation, `input_result` enqueue |
| `crates/botster-core/src/engine/botster.rs` | Engine-level `apply_terminal_input`, submit and poll passthrough |
| `crates/botster-core-daemon/src/daemon.rs` | `CoreDaemon::drain` applies terminal input first |
| `crates/botster-core-test-support/src/terminal_adapter/mod.rs` | Duplex conformance arms |
| `crates/botster-core-test-support/src/terminal_adapter/{fake,unix_shaped,webrtc_shaped,core}.rs` | Drivers gain ingress injection hooks and `try_read` |
| `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/src/lib.rs` | Duplex adapter consumer proof |
| `packages/terminal-protocol/{package.json,metadata.json,terminal-protocol.ts,index.d.ts,index.js,README.md}` | Version 0.2.0, token, revision 2, regenerated mirror |
| `script/terminal-protocol-node-smoke.sh` | Smoke asserts the token, revision 2, and the encode helpers |
| `docs/architecture/terminal-adapter.md` | Duplex contract becomes living truth |
| `docs/architecture/terminal-protocol.md` | Input frame family, crate split, token, revision 2 |
| `docs/archive/plans/core-duplex-pressure-isolated-terminal-subscriptions.md` | This plan |
| `docs/reports/core-duplex-pressure-isolated-terminal-subscriptions-implement.md` | Implement report |

## 8. Assumptions

- **A1.** Egress keeps the JSON `TerminalFrame` encoding. `transport=duplex_binary`
  names the duplex opaque-byte transport, and §8.5 of the frozen contract already
  carries egress bytes as binary DataChannel messages.
- **A2.** Ingress frames carry no session id. The bound adapter already fixes the
  owner triple, and omitting the id removes a cross-session spoofing surface.
- **A3.** The worker atomic admit barrier stays the correctness boundary for
  mode-gated input. §5.8 changes only the parent-side wait.
- **A4.** `DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION` stays `1` and the feature
  token is the sole compatibility gate, because the ticket names it as the gate.
- **A5.** Adding a `TerminalAdapter` trait method is an accepted breaking change
  for Hub. Frozen contract §13 classifies `WebRtcTerminalAdapter` as rewrite.
- **A6.** `docs/archive/plans/` is the plan destination and `docs/architecture/` is
  the destination for durable contract text.
- **A7.** The npm publish is performed by a credentialed operator, not by an agent
  session.
- **A8.** Per-session serialization of mode-gated input is inherent and acceptable.
  One PTY admits one gated write at a time. The isolation requirement is about
  siblings, and §5.8 keeps every sibling progressing.

## 9. Unknowns

- **U1.** The exact `bufferedAmount` reporting that Hub will expose through
  `pressure()` on the new per-channel scheme is Hub-owned. Core's contract is
  unchanged, and no Core work depends on the answer.
- **U2.** Whether Web will want a batched multi-keystroke input frame. Not required
  here. The layout permits it later as an additive kind tag under the same scheme
  version.
- **U3.** Whether a concurrent release consumes package version `0.2.0` or
  conformance revision `2` before this ticket publishes. §14 re-verifies both
  immediately before the human publish and reallocates upward if needed.

## 10. Risks

| # | Risk | Mitigation |
| --- | --- | --- |
| R1 | A flooding subscription starves siblings inside one tick. | Separate intake (64) and apply (16) budgets plus a rotating cursor. Proved by the sibling-isolation test in §12. |
| R2 | Unbounded ingress backlog grows Core memory. | `INPUT_QUEUE_CAPACITY = 256` with an exact refuse-and-hard-stop enqueue rule, and a structural `MAX_TERMINAL_INPUT_FRAME_BYTES` ceiling. |
| R3 | Applying input inside `drain` re-enters `drain` through `CoreDaemon::mode_gated_input`'s pre-admit readback. | Apply through engine primitives only. A test drives a mode-gated frame through `CoreDaemon::drain` and asserts a single non-re-entrant tick. |
| R4 | New code breaks the no-default-feature contract lane. | `ClientWorker` and the protocol crates stay feature-free. Only the PTY apply step sits behind `local-runtime`. The contract lane runs before the gate is claimed. |
| R5 | The new required token breaks in-repo consumer-shaped crates and Hub at once. | That is the intended cold-cut gate. In-repo shaped consumers are updated in the same commit. Hub adoption is `ticket_1787600674_500120`, which pins Core by exact revision. |
| R6 | The TypeScript mirror drifts from the Rust inventory. | Existing `typescript_drift.rs` tests fail on drift. New enums expose `ALL` so the generator derives the union. |
| R7 | A Core-only test is mistaken for production-shaped proof. | [[spawned Hub tests can reach only four of fourteen Core test builders]]. Core claims production-path proof through `CoreDaemon::drain` with a real worker PTY and defers live Hub proof (A23) to `ticket_1787600674_500120`. |
| R8 | The gated submit/poll split changes legacy JSON behavior. | The blocking method is kept and reimplemented on the same two primitives. A characterization test pins its current timeout and reject-concurrent behavior before and after the split. |
| R9 | A stalled gated owner never resumes because the reply is lost. | `poll_mode_gated_pty_input` enforces the same `timeout + grace` deadline the blocking loop uses and returns `TimedOut`, which clears `awaiting_gated` and emits a `Timeout` result. |
| R10 | Conformance revision 2 or package version 0.2.0 collides with a concurrent release. | §3 records verified registry state; §14 re-verifies immediately before publish and reallocates strictly above published history. |

## 11. Runtime-teardown lens answers

### `teardown_class_applies`

**Yes.** This ticket changes `ClientWorker` subscription ownership, adds an ingress
path that creates and destroys durable per-subscription state, and adds new
hard-stop triggers.

### `teardown_isolation`

The ownership set that dies with one failed subscription is exactly one
`SubscriptionOwner` for one `(session_id, subscription_id)` key: its egress queue,
its ingress queue, its `awaiting_gated` slot, its bound adapter, its capability set,
and its snapshot-phase entry. Nothing else.

A failure cannot take down a healthy sibling. Both ingress stages collect teardowns
into a vector and continue iterating the remaining owners, and §5.7 converts a
per-command apply error into a per-owner hard stop instead of an aborted tick.
Isolation is chosen over any shared resource: there is no shared ingress buffer, no
shared decoder state, and no shared cursor beyond the `usize` rotation index. The
one genuinely shared resource is the per-session `gated_in_flight` slot, which is a
property of the PTY, not of the subscription; §5.8 bounds its effect to one owner's
queue and to the gated deadline.

### `teardown_bounds`

- `try_read` is contractually non-blocking, exactly like `try_write`. A blocking
  `try_read` is an illegal adapter and fails the published conformance harness.
- Per-tick ingress work is bounded by
  `INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK` and
  `APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK` times live subscriptions. It cannot spin.
- **No apply path sleeps or waits.** `submit_mode_gated_pty_input` returns after one
  write, and `poll_mode_gated_pty_input` returns after one output pump. The 6 s
  worst-case block in the existing blocking method is not on this path.
- A gated wait is bounded by `mode_gated_input_timeout + MODE_GATED_REPLY_GRACE`,
  after which `poll` returns `TimedOut` and the owner resumes.
- The ingress queue is bounded by `INPUT_QUEUE_CAPACITY`. Overflow is fail-closed,
  not a wait.
- `close()` stays synchronous and non-blocking and now also drops buffered ingress
  and any `awaiting_gated` slot. Core still calls `close()` on the host tick and
  spawns no closer thread.
- The hard stop that ends the path is the existing `hard_stop_key` →
  `ClientWorkerTeardown` → `TransportIngress::UnsubscribeSession` sequence in
  `apply_client_worker_with`. It is unchanged and now also serves ingress failures.

### `late_message_matrix`

| Message | Owner tag | Rejection after terminal failure | Residual sweep |
| --- | --- | --- | --- |
| `Input` frame | Owner triple from the bind; the frame carries no id | Adapter is dropped with the owner, so `try_read` is never called for a dead owner | Buffered bytes and queued commands die with the owner on `hard_stop_key` |
| `ModeGatedInput` frame | Same | Same, plus a stale `(mode_generation, mode_revision)` is rejected by the worker barrier even if the owner is live | Same; `awaiting_gated` is cleared with the owner |
| `Resize` frame | Same | Same | Same |
| Malformed frame | Same | Decode failure hard-stops that owner only | Teardown row emitted on the same tick; no result frame is promised (§5.3) |
| Ingress overflow | Same | The enqueue that would exceed capacity is refused and the owner is hard-stopped | Queue dropped with the owner |
| Late gated reply | `(session_id, GatedRequestId)` | A reply whose `request_id` does not match the live slot is discarded by the existing correlation check | `gated_in_flight` cleared on `Ready`, `TimedOut`, owner teardown, and session teardown |
| Per-command apply error | Same | Hard-stops that owner only; the shared tick continues | Teardown row on the same tick |
| `bind_terminal_adapter` | `(client_id, session_id, subscription_id, generation)` | Unchanged. A stale generation closes and drops the adapter and returns `StaleGeneration` | Unchanged |
| `record_attach` replacement | Same | Unchanged. Replacement hard-stops the previous owner and starts generation N+1 | The previous owner's ingress queue and `awaiting_gated` slot are dropped by that same hard stop |
| `detach_generation` | Same | Unchanged. Generation N detach does not delete generation N+1 | Ingress queue dropped with the owner |
| `teardown_session` / `teardown_all` | Session or global | Unchanged | Ingress queues, `awaiting_gated` slots, and the session `gated_in_flight` slot dropped with their owners |

The matrix guards every ownership-creating ingress surface, including the gated
reply correlation added by §5.8.

### `production_path_proof`

Exact production path:

```
CoreDaemon::drain(session_id, last_output_at)
  -> engine.apply_terminal_input(...)
  -> ManagedSessionRuntime::apply_client_worker_with
  -> poll_mode_gated_pty_input | ClientWorker::intake_terminal_input | ClientWorker::take_terminal_input
  -> adapter.try_read
  -> decode_terminal_input
  -> engine.write_bytes | engine.submit_mode_gated_pty_input | engine resize
  -> real worker PTY
```

Live oracles, not helper calls:

1. **Byte oracle.** A real `botster-session-worker` PTY runs `cat`. Input frames
   enter through a bound adapter. Echoed bytes return through the same adapter,
   byte-exact and in order.
2. **Teardown oracle.** After a malformed frame or an overflow,
   `CoreDaemon::list_terminal_subscriptions` and
   `CoreDaemon::terminal_subscription_generation` both report the route gone, and
   the adapter's `pressure()` is `Closed`. Inventory absence is the live oracle.
3. **Sibling-progress oracle.** With one session's gated reply held by
   `test_mode_gated_hold_ms`, a sibling subscription on another session completes a
   full input-to-echo round trip through `CoreDaemon::drain` **before** the held
   reply is released. This is the direct disproof of the blocking design.
4. **Red-on-revert controls.** Removing the ingress stages from
   `apply_client_worker_with` must make the byte oracle the first failure.
   Substituting the blocking `mode_gated_pty_input` back into the apply step must
   make the sibling-progress oracle the first failure. Implement records both
   failing test names.
5. **No-spin control.** With one subscription flooded and one idle, the idle
   subscription's applied-command count stays bounded by the apply budget. This is a
   deterministic counter assertion, not a wall-clock sample.

This ticket does **not** claim live Hub proof. Per
[[spawned Hub tests can reach only four of fourteen Core test builders]], a Core
builder is not production-shaped Hub proof. Live Hub proof is A23 and belongs to
`ticket_1787600674_500120`.

### `ownership_identity`

Every durable ingress row is keyed by the existing owner triple
`(session_id, subscription_id, generation)`, per
[[Core terminal subscription ownership is session, subscription, and generation]].
The ingress queue and the `awaiting_gated` slot live **inside** `SubscriptionOwner`,
so they are created and destroyed with the generation and cannot outlive it.

Reused-id policy is unchanged and now covers ingress: when Core reuses a
`subscription_id` after teardown it increments the generation, and a delayed close
or detach for generation N never deletes generation N+1. Because ingress state is a
field of the owner rather than a side table keyed by `subscription_id`, a delayed
teardown of generation N structurally cannot reach generation N+1's ingress. There
is no separate sweep to get wrong.

A gated reply carries its own `GatedRequestId`. The existing correlation check
discards a reply whose id does not match the live slot, so a late reply for a torn
down generation cannot complete a command for its replacement.

Owner sweeps cover both queue orders. "Closed first": the owner is gone, so neither
ingress stage visits it and its buffered bytes died with the adapter. "Message
first": the frame is decoded and applied under the live generation, then the close
removes the owner on the same or a later tick.

### `sibling_fail_closed_policy`

- **On successful close.** Siblings keep working. Egress, ingress, `awaiting_gated`,
  and the rotation cursor are per-owner or index-only. A sibling-isolation test
  asserts a second subscription still delivers input and receives output after the
  first is torn down.
- **On ultimate failure.** Core has no ultimate-close failure mode here, because
  `close()` is contractually non-blocking and Core never waits on it. A misbehaving
  adapter that blocks in `close()` is illegal and fails the published conformance
  harness before it reaches production. No sibling is sacrificed at the Core layer.
  Bounded peer-close sibling sacrifice remains a Hub-owned policy, per
  [[Hub ultimate WebRTC close failure sacrifices every peer on the dedicated runtime]].
- **Blast radius.** One malformed frame, one overflow, one decode failure, or one
  per-command apply error closes exactly one subscription. A test asserts the sibling
  count is unchanged after each of the four failures.

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
| **A2** — Core rejects stale-mode input deterministically | A stale `(mode_generation, mode_revision)` yields `admitted = false`, `bytes_written = 0`, `rejection = StaleMode`, and zero PTY bytes | `botster-core-daemon` integration, real worker PTY |
| **A3, Core half** — the token is required | `ensure_compatible` fails with the typed diagnostic when the advertised set omits `transport=duplex_binary`; passes when present | `botster-terminal-protocol/tests/compatibility.rs` |
| **A24, Core half** — consumers can contain the contract | `metadata.json` and `terminal-protocol.ts` carry the token and revision 2; the node smoke resolves the published root export | `typescript_drift.rs`, `script/terminal-protocol-node-smoke.sh` |

### Boundary-usability tests, from finding 1

| Claim | Test |
| --- | --- |
| Hub can forward and cannot inspect | `hub_shaped.rs` builds a `TerminalInputFrame` from bytes, forwards it, and emits identical bytes. A compile-fail or allowlist assertion proves no payload, token, rows, or cols accessor exists on the Hub-safe carrier. |
| Core can decode every command | Round-trip `encode_terminal_input` then `decode_terminal_input` for all three kinds, including a non-UTF-8 payload and both `u16` extremes. |
| Client-shaped consumers can encode every command | `tui_shaped.rs` encodes all three kinds and decodes an `input_result`. The node smoke does the TypeScript equivalent. |
| Hub does not gain the semantic crate | A dependency assertion proves `botster-terminal-protocol` does not depend on `botster-terminal-protocol-client`. |

### Queue lifecycle tests, from finding 2

| Claim | Test |
| --- | --- |
| A returned command is removed and never replayed | Enqueue three commands, run two ticks, assert exactly three deliveries total and a final queue length of zero. |
| Capacity is reachable through production | Drive `CoreDaemon::drain` in a loop while feeding 64 frames per tick against a 16-command apply budget. Assert the queue reaches exactly 256, that the enqueue which would make 257 is refused, and that exactly one teardown row is emitted. |
| The bound is exact, not approximate | Assert the queue length never exceeds 256 at any observed tick boundary. |
| Intake and apply budgets are both enforced | Assert at most 64 intakes and at most 16 applies per owner per tick. |

### Non-blocking mode-gated tests, from finding 3

| Claim | Test |
| --- | --- |
| A held gated reply does not stall a sibling session | Hold session A's reply with `test_mode_gated_hold_ms`. Assert session B's subscription completes input-to-echo through `CoreDaemon::drain` before the hold releases. Red-on-revert: restore the blocking call and assert this is the first failure. |
| A held gated reply does not stall plain input on the same session | A second subscription on session A applies `Input` while the gated command is pending. |
| Order within one owner is preserved across the stall | A queued `Input` behind a pending `ModeGatedInput` reaches the PTY only after the gated command completes, in exact order. |
| A lost reply resumes the owner | Drive past `timeout + grace`, assert a `Timeout` result, a cleared `awaiting_gated`, and that the next command applies. |
| The legacy blocking path is unchanged | Characterization test pins the existing timeout and the reject-concurrent-gated-call behavior across the submit/poll refactor. |
| One apply error does not abort the tick | Force a per-command runtime error, assert exactly one teardown and that a sibling's delivery in the same tick still applied. |

### Fail-closed reporting tests, from finding 4

| Claim | Test |
| --- | --- |
| A malformed frame closes exactly one subscription | Assert `pressure()` is `Closed`, one teardown row, inventory shows the route gone, and the sibling count is unchanged. |
| No undeliverable result is promised | Assert `TerminalInputRejection::ALL` contains no `Malformed` or `QueueOverflow` variant, so the published surface cannot claim a report it cannot deliver. |
| Live-owner results do arrive | `StaleMode`, `PartialWrite`, `Timeout`, and `SessionNotWritable` each reach the client through the adapter with the owner still bound, proved by delivered frame bytes rather than by an accepted write. |

### Remaining deterministic tests required by the ticket

| Claim | Test |
| --- | --- |
| Ordering | N input frames on one subscription reach the PTY in exact order |
| Byte fidelity | A non-UTF-8 byte run and a large paste round-trip byte-exact through the PTY echo |
| Resize | A `resize` frame changes worker geometry and the registry size follows the worker-applied resize |
| Pressure | Egress `WouldBlock` and `Full` behavior is unchanged while ingress keeps draining |
| Reconnect | Detach, re-attach at generation N+1, bind a fresh adapter, prove input flows on N+1 |
| Stale generation | A frame buffered in a generation-N adapter never reaches the PTY after N+1 exists, and a late generation-N gated reply does not complete an N+1 command |
| Teardown | After `teardown_session`, ingress queues and gated slots are gone and inventory reports zero rows |
| Sibling isolation | One subscription floods to overflow and is torn down; a sibling keeps sending input and receiving output |
| Non-blocking input | N input frames require exactly zero host control round trips |
| Ready-then-history preserved | Input is accepted after `READY` and before `FINISH`, attach-phase order unchanged |
| Ghostty semantics preserved | The existing Ghostty attach, snapshot, and mode suites pass unchanged |

### Downstream-shaped proof, required by the charter

- `crates/botster-terminal-protocol/tests/hub_shaped.rs` — a Hub-shaped consumer
  forwards ingress and egress bytes with no semantic accessor in either direction.
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/` — an
  out-of-workspace consumer implements the duplex trait and passes the published
  conformance harness. Run with its own direct Cargo command, because a workspace
  filter does not run a nonmember crate.
- `crates/botster-terminal-protocol-client/tests/tui_shaped.rs` — a TUI-shaped
  consumer encodes all three input kinds and decodes `input_result`.
- `script/terminal-protocol-node-smoke.sh` — clean-directory install of the exact
  published coordinate resolves the token, revision 2, and the encode helpers.

### Not proved here

- Live Hub, browser, or TUI transport behavior. Those are A4 through A17 and A23,
  owned by the Hub, Web, and TUI tickets.
- Wall-clock latency. `ticket_1787603669_760394` owns the observation format and
  `ticket_1787600679_990088` owns the post-cut comparison.

## 13. Commit shape

1. Plan commit — this document.
2. Protocol commit — the two Rust protocol crates, the carrier and semantic API,
   the token, revision 2, the TypeScript regeneration, and the package mirror.
3. Contract commit — `TerminalAdapter::try_read`, the conformance arms, and the
   shaped drivers.
4. Gated-primitive commit — `submit_mode_gated_pty_input` and
   `poll_mode_gated_pty_input`, with the blocking method reimplemented on them and
   its characterization test. This lands **before** the runtime commit so review can
   verify the decomposition separately from the new consumer.
5. Runtime commit — `ClientWorker` ingress ownership, the two stages, and the
   `CoreDaemon::drain` apply step, with its tests.
6. Docs commit — `docs/architecture/` living truth and the Implement report.

No forwarding wrapper is left behind. `mode_gated_input` and `input` remain on
`CoreDaemon` as the transitional host API until the cold-cut ticket removes them;
that retention is required by frozen contract §4 and is not a wrapper around the
new path.

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

1. **Re-verify release identity first.** Run
   `npm view @trybotster/terminal-protocol versions --json` and
   `npm pack @trybotster/terminal-protocol@<latest>`. Confirm that no published
   artifact carries conformance revision 2 and that version 0.2.0 is unallocated.
   If either is taken, reallocate strictly above published history before publishing,
   per [[conformance fixture revisions must be unique per published content]].
2. An operator runs `npm publish --access public` from `packages/terminal-protocol`
   in a credentialed shell. No npm token is added to the repository and no publish
   workflow is added.
3. Verification runs `npm view @trybotster/terminal-protocol version`, installs the
   exact published coordinate in a clean directory, and asserts the revision, the
   token, and the encode helpers — not only the version string.

## 15. Vault gaps worth capturing

Capture only after this ticket proves the contract, not at Plan time:

1. **The Core terminal adapter is duplex and Hub stays content-blind in both
   directions.** The shipped `try_read` contract plus the carrier/semantic crate
   split will be the concrete note beside
   [[core owns duplex terminal transport while Hub stays content blind]].
2. **Core ingress frames use a compact binary header while egress stays JSON.**
   The asymmetry is deliberate and will confuse a future reader without a note.
3. **An opaque protocol carrier needs a sibling semantic codec, or it is
   unimplementable.** Revision 1 of this plan defined a carrier with no decoder.
   This generalizes
   [[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]]
   into a rule for any future opaque boundary.
4. **A blocking parent-side wait cannot be bounded by a frame-count budget.**
   The mode-gated submit/poll split is the durable lesson: work budgets bound work,
   not wall-clock waits.
5. **A failure report cannot ride the adapter that the same failure closes.**
   Fail-closed teardown reports by close. This belongs beside
   [[Core subscription hard-stop is synchronous close and drop on the host tick]]
   and [[adapter accepted writes are not consumer flushed writes]].
6. **Ingress queues live inside `SubscriptionOwner`, so generation reuse needs no
   separate ingress sweep.** This strengthens
   [[Core terminal subscription ownership is session, subscription, and generation]].
7. **A required feature token is the terminal-plane cold-cut gate, and the
   conformance floor stays put.** This refines
   [[daemon event shape changes bump conformance fixture revision not protocol version]].

No gap beyond these seven was found.

## 19. Plan Review response

`review_1787634119_893294`, verdict `changes_required`.

| Finding | Severity | Resolution |
| --- | --- | --- |
| `finding_1787634119_990692` — usable semantic API across the opaque boundary | high, product | §5.2 splits the opaque Hub-safe carrier from the client-crate semantic codec, using the existing crate dependency direction and adding no new edge. §12 adds four boundary-usability tests, including a dependency assertion that Hub never gains the semantic crate. Confirmed as a real defect: revision 1's carrier could not be decoded by Core or constructed by any client. |
| `finding_1787634119_562904` — ingress queue lifecycle and overflow proof | high, product | §5.5 separates intake from apply with exact enqueue and dequeue rules and distinct budgets (64 intake, 16 apply). §5.6 fixes both constants. The capacity bound is now reachable, and §12 drives it through the production `CoreDaemon::drain` loop. Confirmed as a real defect: revision 1 could not reach 257. |
| `finding_1787634119_650778` — synchronous mode-gated input blocks siblings | high, product | Confirmed against source: `worker_process.rs:769` blocks up to `5 s + 1 s` in a sleep loop. §5.8 adds `submit_mode_gated_pty_input` and `poll_mode_gated_pty_input`, keeps the blocking method for the legacy JSON path, and confines stalling to one owner's queue. §5.7 defines per-command error isolation so an apply error never aborts the shared tick. §12 proves it with the existing `test_mode_gated_hold_ms` hook and a red-on-revert control. |
| `finding_1787634119_343164` — `input_result` versus immediate hard-stop | high, product | §5.3 removes the `Malformed` and `QueueOverflow` variants. Fail-closed teardown reports by close, which is the signal every other Core hard-stop already uses. §12 asserts the close and asserts that the published rejection inventory contains no undeliverable variant. Confirmed as a real contradiction. |
| `finding_1787634119_534987` — required Botster maps and release identity | high, product | §2 loads [[botster-architecture]], [[cli-patterns]], and [[spa-patterns]] and records their effect. §2 and §3 add [[conformance fixture revisions must be unique per published content]] with verified registry evidence: the registry holds only `0.1.0` at revision 1 with no duplex token, so revision 2 and version 0.2.0 are free. §14 re-verifies before publish. |
| `finding_1787634119_476601` — incomplete Plan gate evidence | info, process | Gate evidence is resubmitted complete, with a non-empty gate summary. The existing artifact `artifact_1787632944_418165` and the existing checklist `checklist_1787632899_874706` are reused. No second artifact and no second vault checklist are created. |
