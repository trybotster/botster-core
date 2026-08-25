# Core: make terminal subscriptions duplex and pressure-isolated

Ticket `ticket_1787600672_342292`. Run `run_1787632374_189517`. Step `botster_stack_plan`.

Revision 6. Plan Review returned `changes_required` five times.
`review_1787634119_893294` raised four product findings plus one missing-context
finding; `review_1787635010_824294` raised four more against revision 2;
`review_1787635689_864971` raised two against revision 3; and
`review_1787636259_958552` raised two against revision 4; and
`review_1787636806_488333` raised three against revision 5. Section 19 maps every
finding to the section that resolves it.

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
4. Add a **non-blocking submit, poll, and cancel** mode-gated input path so one
   subscription cannot stall a sibling, and apply decoded input inside the production
   `CoreDaemon` drain tick. Cancellation is fenced at the worker, per §5.8.
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
- Add a bounded parent-to-worker control egress queue with its own writer thread, so
  no tick-thread call performs socket I/O. This replaces blocking `write_json` calls
  on the tick path only; the frame encoding and the socket itself are unchanged.
- Change the worker protocol only by the one addition §5.8 names:
  `FRAME_MODE_GATED_CANCEL` plus its in-barrier check. Revision 3 listed the whole
  worker protocol as non-scope; that made cancellation unable to stop a write it had
  already authorized, so the non-scope is withdrawn for this one frame and nothing
  else. `FRAME_MODE_GATED_PTY_INPUT`, the request and result payloads, the deadline
  fence, the freshness check, and the admit ordering are otherwise unchanged.
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
[[Core terminal protocol separates Hub-safe envelopes from client semantic bodies]].

**Verified crate dependency direction.** Revision 2 of this plan claimed the client
crate could re-export `ModeFreshnessToken` from `botster-core`. That was wrong and
would have created a Cargo cycle. The real direction, read from the manifests, is:

```
botster-core  ->  botster-terminal-protocol-client  ->  botster-terminal-protocol
```

`botster-core/Cargo.toml:22-23` depends on both protocol crates.
`botster-terminal-protocol-client/Cargo.toml` depends on the Hub-safe crate and on
nothing from Core. The Hub-safe crate depends on neither. **No new edge is added,
and no edge is reversed.** Every type the client crate publishes must therefore be
defined at or below the client crate, never pulled up from Core.

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
values, rows, or cols. Hub can construct, forward, and emit a frame and can learn
nothing else, so Hub remains content-blind on ingress exactly as it is on egress.

**`botster-terminal-protocol-client` — semantic encode and decode.**

```rust
pub enum TerminalInputCommand {
    Input { data: Vec<u8> },
    ModeGatedInput { mode_generation: u64, mode_revision: u64, data: Vec<u8> },
    Resize { rows: u16, cols: u16 },
}

pub fn encode_terminal_input(command: &TerminalInputCommand)
    -> Result<TerminalInputFrame, TerminalInputEncodeError>;

pub fn decode_terminal_input(frame: &TerminalInputFrame)
    -> Result<TerminalInputCommand, TerminalInputDecodeError>;
```

`ModeGatedInput` carries `mode_generation` and `mode_revision` as **plain `u64`
fields**, which is exactly what the wire carries. It does not name Core's
`ModeFreshnessToken`. Core converts between the two at its single decode site.

That conversion is the established pattern in this repository, not a new mirror:
`client_worker.rs::encode_terminal_frame` already maps Core's internal
`TransportEgress` into the client crate's independent wire types
(`TerminalOutput`, `Snapshot`, `ProcessExit`). Ingress simply runs the same mapping
in the opposite direction. A Core-side parity test asserts the mapping is total in
both directions, so adding a field to Core's `ModeFreshnessToken` fails the test
rather than silently dropping the field.

Core calls `decode_terminal_input`. TUI and Web-shaped consumers call
`encode_terminal_input`. Hub depends on neither, because Hub does not depend on the
client crate.

**Encoding is fallible, and the limits are per kind.** The body length field is a
`u16`, so an arbitrary `Vec<u8>` cannot always be encoded. `mode_gated_input` also
spends 16 body bytes on its two `u64` values, so its data ceiling is lower than
plain input's. The exact limits are in §5.6. `encode_terminal_input` returns
`TerminalInputEncodeError::PayloadTooLarge { kind, max, actual }` rather than
truncating, panicking, or silently splitting. Splitting is the caller's decision,
because only the caller knows whether a byte run may be divided.

`decode_terminal_input` returns a typed error for a truncated body, an unknown kind
tag, or a body length that does not match the kind's fixed prefix.

Wire layout, big-endian, fixed 4-byte header:

```
byte 0      scheme version, exact equality, value 1
byte 1      kind tag: 1 = input, 2 = mode_gated_input, 3 = resize
bytes 2..4  body length, u16, exact
body        kind-specific, see below
```

| Kind | Body | Body length |
| --- | --- | --- |
| `input` | raw input bytes, no encoding, no escaping | `0 ..= 65_535` |
| `mode_gated_input` | `mode_generation: u64`, `mode_revision: u64`, then raw input bytes | `16 ..= 65_535` |
| `resize` | `rows: u16`, `cols: u16` | exactly `4` |

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
    pub mode_generation: u64,
    pub mode_revision: u64,
    pub mode_flags: TerminalModeFlags,
    pub rejection: Option<TerminalInputRejection>,
}

pub struct TerminalModeFlags {
    pub kitty_enabled: bool,
    pub cursor_visible: bool,
    pub bracketed_paste: bool,
    pub mouse_mode: u8,
    pub alt_screen: bool,
    pub focus_reporting: bool,
    pub application_cursor: bool,
}

pub enum TerminalInputRejection {
    StaleMode,
    PartialWrite,
    Timeout,
    SessionNotWritable,
}
```

`TerminalModeFlags` is defined **in the client crate**, for the same cycle reason as
§5.2: Core depends on the client crate, so the client crate cannot name Core's
`ModeFlags`. Its seven fields mirror
`botster-core::contract::session_protocol::ModeFlags` one for one, and Core maps
between them at the single encode site, exactly as `encode_terminal_frame` already
maps `TransportEgress` today. A Core-side parity test asserts the mapping is total,
so adding a field to Core's `ModeFlags` fails that test rather than silently
dropping the new field on the wire.

The freshness values ride as two plain `u64` fields for the same reason.

A new `TerminalEvent` variant carries wire tag `input_result`, and `EVENT_TYPES` in
`crates/botster-terminal-protocol/src/frame.rs` gains `"input_result"`.

**Every listed variant is reachable on a live owner.** Revision 1 also listed
`Malformed` and `QueueOverflow`. Both are removed. Those conditions hard-stop the
owner, and `hard_stop_key` closes and drops the same adapter that would have carried
the result. Existing contract text is explicit that close abandons an in-flight
frame and that an accepted write is not a delivered write, per
[[adapter accepted writes are not consumer flushed writes]]. A result frame enqueued
immediately before that close is unobservable, so promising it would be a false
claim. Ingress loss (§5.4) is fail-closed for the same reason and reports the same
way.

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

`TerminalAdapter` gains exactly one method, and it returns a **typed** ingress
outcome rather than an `Option`:

```rust
/// Take the next ingress event. Never blocks.
fn try_read(&mut self) -> TerminalIngress;

pub enum TerminalIngress {
    /// No complete frame is buffered. The stream is still contiguous.
    Empty,
    /// One complete frame, in arrival order.
    Frame(Vec<u8>),
    /// The transport dropped at least one frame. The stream is no longer
    /// contiguous. Carries no payload and no count of lost bytes.
    Lost,
    /// The adapter is closed. Terminal state.
    Closed,
}
```

Revision 2 returned `Option<Vec<u8>>` and said a full receive buffer would report
through `pressure()`. That was wrong twice: `TerminalAdapterPressure` describes only
egress readiness, the single active write, and close, and `None` cannot be
distinguished from lost input. Silent ingress loss breaks byte fidelity and ordering,
which are the two properties this ticket exists to guarantee, so the loss must be
**observable and fail-closed**.

Contract rules:

- Non-blocking. `Empty` is returned immediately when nothing is buffered.
- The adapter delivers **complete frames only**, in arrival order. Transport
  reassembly (Hub chunking) happens below the adapter.
- The adapter does not decode, inspect, reorder, coalesce, or synthesize frames.
  `Lost` is a flag, not payload, so reporting it stays content-blind.
- The adapter must buffer at least `MIN_ADAPTER_INGRESS_BUFFER_FRAMES` complete
  frames before it may report `Lost` (§5.6). A conforming adapter therefore never
  reports `Lost` to a Core that drains every tick within its intake budget.
- Once the adapter drops an ingress frame it must return `Lost` on the next
  `try_read` and must not return a later `Frame` before that `Lost` is observed.
  Loss is never silent and never reordered behind good data.
- After `close()` and after transport-side death, `try_read` returns `Closed`
  permanently and buffered ingress is dropped.

`Lost` is unrecoverable by construction: a gap in a terminal byte stream cannot be
repaired, and the client cannot know which keystrokes vanished. Core therefore
hard-stops that owner (§5.5). The client re-subscribes and re-attaches, which
restores a known-good state.

Adding a trait method is breaking for the shaped adapters and for Hub.
`TerminalAdapterWriteError` and `TerminalAdapterPressure` are unchanged, so
[[botster core public enums are breaking until non exhaustive is decided]] is not
triggered for those two. `TerminalIngress` is new, and it is exhaustive at `0.1.0`
under the same rule the module already documents.

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

```rust
struct GatedWait {
    request_id: GatedRequestId,
    deadline: Instant,
}
```

`ClientWorker` gains `input_cursor: usize`, a rotation index.

**Stage A — intake.** `ClientWorker::intake_terminal_input() -> Vec<ClientWorkerTeardown>`

1. Visit live owners starting at `input_cursor`, then advance the cursor by one.
2. For each owner with a bound adapter, call `try_read` at most
   `INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK` times, stopping early on `Empty`.
3. Dispatch on the `TerminalIngress` value:
   - `Empty` — stop intake for this owner.
   - `Closed` — stop intake for this owner. The existing close handling owns it.
   - `Lost` — fail-closed. Hard-stop that owner through `hard_stop_key`, return its
     teardown, and stop intake for that owner. Ingress contiguity is gone, so no
     later frame from this adapter may be applied.
   - `Frame(bytes)` — decode with `decode_terminal_input`. A decode error is
     fail-closed: hard-stop that owner, return its teardown, stop intake for it.
     A malformed frame is a broken or hostile client, and Core must not guess.
4. Push each decoded command onto `input_queue`. **Enqueue rule:** push only while
   `input_queue.len() < INPUT_QUEUE_CAPACITY`. The push that would exceed the
   capacity is refused; Core hard-stops that owner and returns its teardown. Nothing
   is enqueued past the bound, so the bound is exact rather than approximate.

**Stage B — apply.** `ClientWorker::take_terminal_input() -> Vec<TerminalInputDelivery>`

1. Visit live owners in the same rotation order.
2. **If `awaiting_gated` is set for this owner, dequeue nothing at all and move to
   the next owner.** Revision 2 stopped only when the queue *head* was another
   `ModeGatedInput`, which let a plain `Input` behind a pending gated command leave
   the queue on the following tick and reach the PTY ahead of it. That contradicted
   this plan's own ordering guarantee and its own acceptance test. The owner is now
   fully parked until §5.8 clears `awaiting_gated` through a correlated result, a
   timeout, or teardown.
3. Otherwise pop from the **front** of `input_queue`, in order, at most
   `APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK` commands for that owner.
   **Dequeue rule:** a command is removed from the queue exactly when it is returned
   as a delivery. A returned command is never replayed, because it no longer exists
   in the queue.
4. Stop applying for that owner as soon as one of these holds, and leave the rest
   queued for a later tick:
   - the per-owner apply budget is spent;
   - the command just returned was `ModeGatedInput`, which sets `awaiting_gated`
     when §5.7 submits it, so nothing further for this owner may be dequeued;
   - the head command is `ModeGatedInput` and another owner on the same session
     already holds the session's gated slot.

Ordering: within one owner, commands are dequeued strictly in arrival order and the
owner is parked for the entire lifetime of a pending gated command, so no command
can overtake it. Across owners there is no ordering relation, which is exactly the
isolation the project requires.

**Why overflow is now reachable.** Intake admits up to 64 frames per owner per
tick, apply removes at most 16. A client that sustains more than 16 commands per
tick grows its own queue by up to 48 per tick and crosses 256 within six ticks. The
257th enqueue attempt trips the bound. §12 drives that through the production
`CoreDaemon::drain` loop rather than by calling a helper.

Every ingress failure path uses `hard_stop_key`, so ingress teardown produces the
same `ClientWorkerTeardown` rows and the same synchronous close as egress teardown,
per [[Core subscription hard-stop is synchronous close and drop on the host tick]].
Every teardown path also cancels an outstanding gated request, per §5.8.

### 5.6 Exact bounds

| Constant | Location | Value | Meaning |
| --- | --- | --- | --- |
| `TERMINAL_INPUT_SCHEME_VERSION` | `botster-terminal-protocol` | `1` | Exact equality on byte 0 |
| `MAX_TERMINAL_INPUT_BODY_BYTES` | `botster-terminal-protocol` | `65_535` | `u16` body ceiling |
| `MAX_TERMINAL_INPUT_FRAME_BYTES` | `botster-terminal-protocol` | `65_539` | 4-byte header plus the body ceiling |
| `MAX_INPUT_DATA_BYTES` | `botster-terminal-protocol` | `65_535` | `input` data ceiling; body is data only |
| `MODE_GATED_PREFIX_BYTES` | `botster-terminal-protocol` | `16` | two `u64` freshness values |
| `MAX_MODE_GATED_DATA_BYTES` | `botster-terminal-protocol` | `65_519` | `65_535 - 16`; the lower per-kind ceiling |
| `RESIZE_BODY_BYTES` | `botster-terminal-protocol` | `4` | exact, `rows: u16` plus `cols: u16` |
| `MIN_ADAPTER_INGRESS_BUFFER_FRAMES` | `botster-core` contract | `64` | Equal to the intake budget. A conforming adapter buffers at least this many complete frames before it may report `Lost`. |
| `WORKER_CONTROL_QUEUE_FRAMES` | `worker_process` | `32` | Bounded parent-to-worker control egress queue, per session |
| `WORKER_CONTROL_RESERVED_SLOTS` | `worker_process` | `2` | Slots ordinary frames may never occupy: one cancel, one shutdown. Both exact, because at most one gated request per session is in flight and shutdown happens once. Ordinary capacity is therefore 30. |
| `WORKER_CONTROL_WRITE_TIMEOUT` | `worker_process` | `2 s` | `UnixStream::set_write_timeout`, so a stalled `write_all` errors instead of blocking forever |
| `WORKER_CONTROL_WRITER_JOIN_BOUND` | `worker_process` | `1 s` | Teardown joins the writer thread under this bound, then detaches; the FD is already closed |
| `INPUT_QUEUE_CAPACITY` | `ClientWorker` | `256` commands per subscription | Bounded per-subscription backlog |
| `INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK` | `ClientWorker` | `64` | Stage A budget |
| `APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK` | `ClientWorker` | `16` | Stage B budget; bounds per-tick PTY work |

`MIN_ADAPTER_INGRESS_BUFFER_FRAMES` equals `INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK`
on purpose. A host that drains every tick can always take everything a conforming
adapter is required to hold, so `Lost` is reachable only when the host stops draining
or the transport itself fails. That makes `Lost` a genuine fault signal rather than
routine backpressure.

`WRITE_ATTEMPT_BUDGET` stays `512` and is untouched. Frozen-contract row A27b depends
on that exact value.

### 5.7 Production path

```
Hub subscription channel receives binary bytes
  -> Hub reassembles chunks and hands complete frame bytes to the bound adapter
  -> adapter buffers them; Hub decodes nothing
  -> CoreDaemon::drain(session_id, last_output_at)          <- host tick, unchanged signature
       -> engine.apply_terminal_input()                     <- NEW, first step of the tick
            1. poll any in-flight gated reply for this session (§5.8);
               Ready or TimedOut clears awaiting_gated and enqueues its input_result
            2. ClientWorker::intake_terminal_input   (Lost, malformed, or overflow -> hard stop)
            3. ClientWorker::take_terminal_input     (parked owners dequeue nothing)
            4. per delivery, apply through the engine primitives:
                 Input           -> engine.write_bytes
                 ModeGatedInput  -> engine.submit_mode_gated_pty_input   (non-blocking,
                                    sets awaiting_gated and parks that owner)
                 Resize          -> engine resize
            5. for every teardown returned by any step above, cancel that owner's
               outstanding gated request with engine.cancel_mode_gated_pty_input
            6. enqueue one input_result egress frame per completed outcome
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
| Runtime error for that session | hard-stop **that owner only**, emit its teardown, cancel any outstanding gated request, continue the loop |

`apply_terminal_input` returns `Ok` whenever the tick itself completed, even when
individual commands failed. Only a failure of the tick machinery propagates.

**Teardown always releases the gated slot.** Step 5 is not optional bookkeeping. Any
teardown produced by steps 1 through 4 removes a `ClientWorker` owner, but the
`gated_in_flight` slot lives on the session runtime, so it must be cancelled
explicitly or it stays occupied until its deadline and strands the session's gated
lane. §5.8 defines the cancel primitive and its exact matching rule.

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

// New. Releases the session slot for an abandoned request. Never blocks.
fn cancel_mode_gated_pty_input(&mut self, session_id, request_id: &GatedRequestId)
    -> Result<(), SessionRuntimeError>;

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

**Cancellation on teardown must fence the worker, not just the parent.**
`gated_in_flight` is parent state, but the request is already on the worker's control
channel. Revision 3 cleared only the parent slot. That was not enough: the worker
still owned the submitted request and could pass its own freshness check and write to
the PTY before `deadline_unix_ms`. The deadline bounds a late write; it does not
prevent one. A torn-down generation could therefore put bytes into the session's PTY
after its ownership ended, and freeing the parent slot immediately let a replacement
request overlap the abandoned one.

**This revision changes the worker protocol, and §4 records that change.** Revision 3
listed the worker protocol as non-scope. That non-scope made the design unsafe, so it
is withdrawn for exactly one addition. The ticket assigns Core "mode-gated input,
generation, close, recovery, and teardown", and a teardown that cannot stop a write it
already authorized is not a teardown.

One new control frame:

```
FRAME_MODE_GATED_CANCEL  ->  { request_id: String }
```

#### 5.8.1 The worker observes cancellation through a dedicated cell, not a scan

Revision 4 had the worker drain `frame_receiver` inside the PTY barrier looking for a
cancel, stashing other frames in a deferred buffer. That was wrong twice.
`botster-session-worker.rs:136` creates that channel with unbounded
`mpsc::channel()`, so the in-barrier scan had no work bound, and the deferred buffer
had no capacity, no overflow rule, and no defined replay owner. A fence must not be
the most expensive step in the critical section it protects.

The worker instead owns one **single-slot cancellation cell**:

```rust
cancel_cell: Arc<Mutex<Option<GatedRequestId>>>
```

- The control **reader thread** (`botster-session-worker.rs:897`) already parses every
  frame. On `FRAME_MODE_GATED_CANCEL` it stores the `request_id` in the cell and does
  **not** forward the frame to `frame_receiver`. All of that work happens off the
  barrier path.
- Inside `runtime.with_pty_io_barrier`, after the existing deadline and freshness
  checks and **strictly before** `barrier.write_input`, the worker takes one lock,
  compares one id, and clears the cell when it matches. That is O(1) with no scan, no
  deferred buffer, and no replay ordering to get wrong.
- One slot is exact, not a guess: the parent admits one gated request per session at a
  time and sends at most one cancel per request, so two cancels can never be live
  together. A second cancel overwrites, which is the correct behavior for a newer one.
- The cell is cleared at admit completion whether or not it matched, so a cancel that
  names an already-finished request cannot leak into a later one.

A matching cancel ends the admit with `admitted = false`, `bytes_written = 0`,
`error_kind = "cancelled"`, and **no write**.

**The race is total, with no third outcome.** The cell read sits in the same critical
section as the deadline and freshness checks and strictly before the write, and
`write_input` is bounded and completes inside the barrier. So exactly one of these
happens:

| Order | Result |
|-------|--------|
| Cancel stored in the cell before the barrier reads it | Nothing is written. Reply is `cancelled`, `bytes_written = 0`. |
| The write completed before the reader thread stored the cancel | Bytes were written. The reply reports `admitted = true` and the exact `bytes_written`, truthfully. |

There is no interleaving in which a partial write escapes the fence, because the write
is not re-entered after the check. The reply always tells the parent which of the two
occurred, so the plan never claims a byte was suppressed when it was not.

#### 5.8.2 One writer owns every post-spawn control write

Revision 4 called `submit_mode_gated_pty_input` and `cancel_mode_gated_pty_input`
non-blocking. They were not. `worker_process.rs:2247` writes through
`stream.write_all(&frame).and_then(|_| stream.flush())` with no deadline, and a submit
can carry up to `MAX_MODE_GATED_DATA_BYTES`. Revision 5 moved three sends to a bounded
queue, which was still incomplete: the write half cannot be owned by a writer thread
while other methods keep writing to it directly, and a cloned handle would let two
`write_all` calls interleave framed bytes on one stream.

**The writer thread takes exclusive ownership of the `WorkerControl` write half.** No
other code holds it, nothing is cloned, and therefore no two writes can interleave.
Every post-spawn frame is enqueued; nothing bypasses the queue.

Exact inventory, read from the source. These are every post-spawn control write:

| Frame | Current site | Class |
| --- | --- | --- |
| `FRAME_PTY_INPUT` | `worker_process.rs:1304` (`write_frame`) | Ordinary |
| `FRAME_MODE_GATED_PTY_INPUT` | `:796` | Ordinary |
| `FRAME_RESIZE` | `:1308` | Ordinary |
| `FRAME_SET_TIMEOUT` | `:417` | Ordinary |
| `FRAME_GET_MODE_FLAGS` | `:441` | Ordinary |
| `FRAME_GET_SNAPSHOT` | `:599`, `:689`, `:710`, `:1389` | Ordinary |
| `FRAME_SET_COLOR_PROFILE` | color-profile apply path | Ordinary |
| `FRAME_PING` | `:384` (`write_frame`) | Ordinary |
| `FRAME_MODE_GATED_CANCEL` | new, §5.8 | Cancel |
| `FRAME_SHUTDOWN` | `:1227`, `:1312`, `:1398` (`write_frame`) | Terminal |

`FRAME_SPAWN_SESSION` (`:1197`) is **excluded and stays a direct write**. It happens on
the initial control handle before the session is registered, which is the last write
before the writer thread takes ownership. The handover point is exactly that line, and
§12 asserts no other direct post-spawn write survives.

**One FIFO queue, three admission classes.** Ordering must hold across all classes, so
there is one queue and no priority lane. A priority lane could deliver a cancel for a
request the worker had not yet received, and the fence would silently do nothing.

| Class | Admission rule |
| --- | --- |
| Ordinary | Admitted while `len < WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS`. Overflow returns `ControlQueueFull`. |
| Cancel | Admitted into the reserved region, so a cancel always fits. FIFO keeps it behind its own submit. |
| Terminal (`FRAME_SHUTDOWN`) | Admitted into the reserved region. It is the last frame: after it the queue accepts nothing, ordinary or otherwise. |

`WORKER_CONTROL_RESERVED_SLOTS` is 2, one for a cancel and one for a shutdown. Both are
exact rather than conservative: at most one gated request per session is in flight, so
at most one cancel is outstanding, and shutdown happens once. Revision 5 reserved only
a cancel slot and left shutdown to compete with ordinary traffic, which could have made
a required teardown frame unsendable exactly when it was needed.

**Overflow is fail-closed and owner-scoped.** An ordinary `try_send` that finds the
ordinary capacity full returns `ControlQueueFull`, and §5.7 routes that per-command
error to a hard stop of that owner alone. No frame is ever dropped, so no byte run is
silently truncated.

#### 5.8.3 The writer is bounded, and same-session pressure is fail-closed

A queue in front of a blocking socket only moves the stall; it does not bound it. The
writer thread itself must be bounded, and the consequence for same-session siblings
must be stated rather than assumed away.

**Write bound.** The `Socket` variant sets `UnixStream::set_write_timeout` to
`WORKER_CONTROL_WRITE_TIMEOUT`, so a `write_all` that cannot make progress returns an
error instead of blocking forever. The `Stdio` variant has no write timeout; its bound
is the hard stop below.

**Named hard stop.** The session teardown path owns writer shutdown. It calls
`UnixStream::shutdown(Shutdown::Write)` for the socket variant, or drops `ChildStdin`
for the stdio variant. Either makes an in-progress `write_all` return immediately, so
the writer is never waited on indefinitely. This is the hard stop
[[botster runtime teardown lenses]] requires: it ends the driver loop even when the
underlying write path misbehaves.

**FD and join.** The shutdown owner closes the file descriptor exactly once. It then
joins the writer thread under `WORKER_CONTROL_WRITER_JOIN_BOUND`. If the join exceeds
that bound the thread is detached; the FD is already closed, so the detached thread can
neither hold the socket nor block the caller, and it exits on its next write error.
Teardown therefore never blocks the control plane.

**Same-session sibling policy: fail-closed, stated plainly.** A session has exactly one
worker and one ordered control channel. Every later frame for that session queues behind
a stalled write, so two subscriptions on the **same session** cannot be isolated from
each other at this layer. This plan does not claim they can.

- Within `WORKER_CONTROL_WRITE_TIMEOUT`, same-session siblings are **delayed, not
  failed**. The tick still never blocks, because it only ever calls `try_send`.
- Past that bound the write fails, and **every subscription on that session is
  hard-stopped together**. The session's control channel is unusable, and reporting some
  subscriptions as live while the runtime cannot reach the worker is exactly the
  terminal-state-versus-live-runtime divergence the teardown lens exists to prevent.
  This follows [[webrtc peer cleanup removes every per peer owner together]]: the whole
  per-session owner set dies together.
- **Every other session is unaffected**, because each has its own queue, its own writer
  thread, and its own FD. That is the isolation this ticket actually guarantees, and
  §12 proves it with a cross-session oracle in addition to the same-session policy test.

The parent lane bound stays independent of all of this: the gated slot frees on the
correlated reply or at `mode_gated_input_timeout + MODE_GATED_REPLY_GRACE`, whichever
comes first.

#### 5.8.4 Lane release and cancellation coverage

Parent side, `cancel_mode_gated_pty_input(session_id, request_id)`:

1. `try_send` the cancel frame into the reserved slot. Returns immediately.
2. Mark the parent slot `Cancelled { request_id }`. **Do not free it yet.**
3. Free the slot only when the correlated reply arrives, or at
   `mode_gated_input_timeout + MODE_GATED_REPLY_GRACE`.

Holding the lane until resolution is what removes the overlap: a replacement gated
request for that session is not admitted while an abandoned one can still write.

Because the fence is keyed by `request_id`, cancelling generation N's request can
never affect generation N+1. N+1 cannot even submit until N's lane is released, and
its own submit then produces a fresh `GatedRequestId`.

Every path that removes an owner calls the cancel when `awaiting_gated` is set:
`hard_stop_key` (malformed frame, ingress `Lost`, queue overflow, per-command apply
error, replacement), `detach_live`, `detach_generation`, `teardown_session`, and
`teardown_all`.

**What still serializes, and why that is correct.** `gated_in_flight` is a
per-session slot and the worker admits one gated request per session at a time. The
PTY is one device, so gated input for one session is inherently serial. The apply
stage therefore parks the owner that holds a pending gated command, and skips an
owner whose head command is `ModeGatedInput` while another owner on the same session
holds the slot. It stalls **nothing else**: other owners on the same session continue
to apply plain `Input` and `Resize`, owners on other sessions are untouched, egress
`pump` continues for every owner, and no thread sleeps. Head-of-line blocking is
confined to one owner's queue and is bounded by the gated deadline, after which
`poll` returns `TimedOut` and the owner resumes with a `Timeout` result.

**Deterministic proof, no sleeps.** `test_mode_gated_hold_ms` already exists and is
forwarded to the worker as `test_hold_ms`. §12 uses it to hold one reply while a
sibling on another session sends input and receives echoed output, then releases it.
That is a real production-path hold, not a mocked stall. The same hook holds a reply
while the owner is torn down, which proves cancellation releases the slot without
waiting for the deadline.

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
| `crates/botster-terminal-protocol-client/src/input.rs` | **New.** `TerminalInputCommand`, `encode_terminal_input`, `decode_terminal_input`, `TerminalInputEncodeError`, `TerminalInputDecodeError` |
| `crates/botster-terminal-protocol-client/src/events.rs` | `TerminalInputResult`, `TerminalModeFlags`, `TerminalInputKind`, `TerminalInputRejection`, new `TerminalEvent` variant, `ALL` inventories |
| `crates/botster-terminal-protocol-client/src/typescript.rs` | Generator emits the new unions, interfaces, encode helpers, and token |
| `crates/botster-terminal-protocol-client/tests/{typescript_drift,wire,tui_shaped}.rs` | Drift, mirror, wire-tag, and TUI-shaped encode/decode coverage |
| `crates/botster-core/src/contract/terminal_adapter.rs` | `try_read` added returning the new `TerminalIngress` enum, `MIN_ADAPTER_INGRESS_BUFFER_FRAMES`, duplex contract documented |
| `crates/botster-core/src/contract/terminal_subscription.rs` | `TerminalInputDelivery`, re-export of `TerminalInputCommand` |
| `crates/botster-core/src/engine/client_worker.rs` | Ingress queue, rotation cursor, `intake_terminal_input`, `take_terminal_input`, `awaiting_gated`, fail-closed decode and overflow |
| `crates/botster-core/src/runtime/worker_process.rs` | `submit_mode_gated_pty_input`, `poll_mode_gated_pty_input`, `cancel_mode_gated_pty_input`, `GatedPoll`; bounded control egress queue with three admission classes; per-session writer thread that takes the `WorkerControl` write half by move; every post-spawn `write_json` and `write_frame` site rerouted through the queue; write timeout, `shutdown(Shutdown::Write)` hard stop, and bounded join; existing blocking method reimplemented on the new primitives |
| `crates/botster-core/src/engine/managed_session_runtime.rs` | Ingress stages and apply inside the tick, per-command error isolation, `input_result` enqueue |
| `crates/botster-core/src/engine/botster.rs` | Engine-level `apply_terminal_input`, submit and poll passthrough |
| `crates/botster-core-daemon/src/daemon.rs` | `CoreDaemon::drain` applies terminal input first |
| `crates/botster-core/src/contract/session_protocol.rs` | `FRAME_MODE_GATED_CANCEL` frame constant and its payload type |
| `crates/botster-core-daemon/src/bin/botster-session-worker.rs` | Single-slot `cancel_cell` set by the control reader thread; O(1) cell check inside `atomic_mode_gated_admit` before `write_input` |
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
- **A9.** The client crate defines its own wire types for the freshness values and
  the mode flags, and Core maps at the boundary. This is required by the crate
  dependency direction, and it matches the shipped egress pattern in
  `client_worker.rs::encode_terminal_frame`. A total-mapping parity test, not
  convention, is what keeps the two from drifting.
- **A10.** Ingress `Lost` is unrecoverable and therefore fail-closed. A gap in a
  terminal byte stream cannot be repaired, and the client cannot know which
  keystrokes vanished, so closing the subscription and forcing a re-attach is the
  only honest response.
- **A11.** The parent and the session worker binary are always version-matched,
  because the parent builds and spawns the worker from the same source tree and Hub
  pins Core by exact revision. The one new frame needs no negotiation.
- **A12.** One reserved cancel slot and one reserved shutdown slot are exact rather
  than conservative, because at most one gated request per session is in flight and
  shutdown happens once. If that invariant ever changed, both bounds change with it.
- **A13.** Same-session subscriptions share one worker and one ordered control
  channel, so they cannot be isolated from each other at the control-transport layer.
  §5.8.3 states a fail-closed policy for that case rather than claiming an isolation
  the layer cannot deliver. Cross-session isolation is complete, and that is what the
  ticket's sibling requirement needs.
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
| R11 | A future field added to Core's `ModeFlags` or `ModeFreshnessToken` is silently dropped on the wire. | The mapping is total and a Core-side parity test asserts it in both directions, so adding a field fails the test rather than shipping a gap. |
| R12 | An adapter implementer returns `Empty` instead of `Lost` and reintroduces silent input loss. | The published conformance harness asserts the `Lost` contract, and a red-on-revert control proves the byte-fidelity test fails when loss is silent. |
| R14 | The cancellation cell holds a stale id and cancels the wrong request. | The cell is single-slot, cleared at admit completion whether or not it matched, and compared by exact `request_id`. A test sends a cancel for a finished request and asserts the next request is admitted normally. |
| R16 | A wedged worker control socket permanently strands one session's gated lane. | The lane frees on the correlated reply or at `timeout + grace`, whichever comes first. The stalled-reader test asserts both the bound and continued sibling progress. |
| R18 | A post-spawn write site is missed and keeps writing directly, interleaving framed bytes. | The writer thread owns the write half by move rather than by convention, so a missed site fails to compile. §12 adds a source assertion and a concurrent frame-integrity test. |
| R19 | Same-session subscriptions are assumed isolated when they are not. | §5.8.3 states the fail-closed policy instead of claiming isolation, and §12 tests delay-then-collective-hard-stop rather than survival. |
| R17 | The bounded control queue drops a plain input frame under pressure and breaks byte fidelity. | Ordinary overflow is fail-closed: it hard-stops that owner rather than dropping a frame, so no byte run is ever silently truncated. The overflow test asserts the teardown, not a drop. |
| R15 | The worker protocol addition creates parent/worker version skew. | The parent builds and spawns the session worker from the same source tree, and Hub pins Core by exact revision, so the two are always matched. The charter gate command already rebuilds the worker binary before the suite. |
| R13 | A teardown path is added later without cancelling the gated slot. | Cancellation is driven from one place, step 5 of §5.7, over the teardown vector every ingress stage already returns, so a new failure path inherits it. A test enumerates all seven owner-removing paths. |
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
queue and to the gated deadline, and its cancel primitive releases the slot
immediately when the owner dies rather than stranding the session's gated lane until
that deadline expires.

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
- `submit_mode_gated_pty_input` and `cancel_mode_gated_pty_input` are non-blocking
  because they `try_send` into the bounded control egress queue and return. A
  dedicated per-session writer thread performs the blocking socket write, off the tick
  thread and holding no lock the tick needs. No tick-thread call performs socket I/O.
- Ordinary control sends fail closed at the ordinary capacity and hard-stop that owner
  alone. A cancel always fits, because `WORKER_CONTROL_RESERVED_CANCEL_SLOTS` is never
  consumed by ordinary traffic.
- A blocked control write is bounded by `WORKER_CONTROL_WRITE_TIMEOUT`, and the named
  hard stop is `shutdown(Shutdown::Write)` or dropping `ChildStdin`, which makes an
  in-progress `write_all` return at once. Teardown joins the writer under
  `WORKER_CONTROL_WRITER_JOIN_BOUND` and then detaches; the FD is already closed, so a
  detached writer holds nothing and blocks nobody.
- A wedged control socket degrades exactly one **session**. Same-session subscriptions
  are delayed within the write timeout and then hard-stopped together past it, which
  §5.8.3 states as an explicit fail-closed policy. Every other session is unaffected,
  and the tick never blocks.
- The worker's cancel observation is O(1): one lock, one id compare, one clear. It is
  not a scan, so it adds no unbounded work to the PTY critical section.
- The cancelled lane is still bounded. The parent frees the slot on the correlated
  reply or at `mode_gated_input_timeout + MODE_GATED_REPLY_GRACE`, whichever comes
  first, so an abandoned request delays a replacement by at most that bound and never
  indefinitely.
- A conforming adapter buffers at least `MIN_ADAPTER_INGRESS_BUFFER_FRAMES` complete
  frames, so its receive buffer is bounded and its overflow is observable as `Lost`
  rather than silent.
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
| Ingress overflow | Same | The enqueue that would exceed capacity is refused and the owner is hard-stopped | Queue dropped with the owner; outstanding gated request cancelled |
| Ingress `Lost` | Same | The adapter reports lost frames before any later frame; Core hard-stops that owner because contiguity is unrecoverable | Queue dropped with the owner; outstanding gated request cancelled |
| Late gated reply | `(session_id, GatedRequestId)` | A reply whose `request_id` does not match the live slot is discarded by the existing correlation check. After cancellation the slot is empty, so a late reply matches nothing and no `input_result` is synthesized | `gated_in_flight` cleared on `Ready`, `TimedOut`, and explicitly by `cancel_mode_gated_pty_input` on every owner-removing path: `hard_stop_key`, `detach_live`, `detach_generation`, `teardown_session`, `teardown_all` |
| Per-command apply error | Same | Hard-stops that owner only; the shared tick continues | Teardown row on the same tick; outstanding gated request cancelled |
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
4. **Worker-fence oracle.** Hold a submitted gated request with
   `test_mode_gated_hold_ms`, tear down generation N during the hold, then release
   it. Assert the worker replied `cancelled` with `bytes_written = 0`, that the PTY
   received zero bytes from that request, and that generation N+1 was admitted only
   after the cancelled reply released the lane. This proves teardown stops a write
   the parent had already authorized, which a deadline alone cannot do.
5. **Red-on-revert controls.** Four, each naming the test that must fail first:
   removing the ingress stages from `apply_client_worker_with` breaks the byte
   oracle; substituting the blocking `mode_gated_pty_input` back into the apply step
   breaks the sibling-progress oracle; restoring the head-only apply stop rule breaks
   the ordering oracle; dropping the in-barrier cancel check breaks the worker-fence
   oracle. A fifth, making adapter loss silent, breaks the byte-fidelity oracle.
   Implement records every failing test name.
6. **Control-pressure oracle.** With the worker's control reader stalled, a
   subscription on a different session still completes input-to-echo through
   `CoreDaemon::drain`. This proves no tick-thread call performs socket I/O.
7. **No-spin control.** With one subscription flooded and one idle, the idle
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

A gated reply carries its own `GatedRequestId`, and `cancel_mode_gated_pty_input`
is keyed by `(session_id, request_id)` rather than by subscription. It clears the
slot only on an exact id match, so cancelling generation N's abandoned request can
never cancel generation N+1's live one: N+1's own submit produced a fresh id. The
existing correlation check then discards a reply whose id does not match the live
slot, so a late reply for a torn-down generation cannot complete a command for its
replacement.

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
| `assert_ingress_empty` | A fresh adapter returns `TerminalIngress::Empty`, and an idle adapter keeps returning `Empty` |
| `assert_ingress_order` | Three injected frames come back in exact arrival order |
| `assert_ingress_whole_frames` | A partially injected frame is not returned until complete |
| `assert_ingress_closed_local` | After `close()`, `try_read` returns `Closed` permanently and buffered ingress is dropped |
| `assert_ingress_closed_transport` | Same after transport-side death: `Closed`, permanently, never `Empty` |
| `assert_ingress_lost` | After the driver drops a buffered frame, the next `try_read` returns `Lost`; no `Frame` is returned before that `Lost` is observed; and the adapter accepts `MIN_ADAPTER_INGRESS_BUFFER_FRAMES` complete frames without reporting `Lost` |
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
| No cycle exists | A dependency assertion proves `botster-terminal-protocol-client` does not depend on `botster-core`. `cargo tree` over the workspace must show the single direction core to client to hub-safe. |
| Encoding is fallible at the exact per-kind ceiling | `input` with 65,535 bytes encodes; 65,536 returns `PayloadTooLarge { max: 65_535 }`. `mode_gated_input` with 65,519 bytes encodes; 65,520 returns `PayloadTooLarge { max: 65_519 }`. Both ceilings are asserted separately so the 16-byte prefix cannot regress unnoticed. |
| `resize` body is exactly four bytes | A frame whose declared length is not 4 fails `decode_terminal_input`. |
| The mode-flag mapping is total | A Core-side parity test maps every `ModeFlags` field to `TerminalModeFlags` and back. Adding a field to Core's struct fails this test. |

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
| Order within one owner is preserved across the stall | Enqueue `ModeGatedInput` then `Input`. Hold the gated reply, then run several ticks. Assert the PTY receives zero bytes from the later `Input` while `awaiting_gated` is set, and that it arrives only after the gated command completes. Red-on-revert: restore the head-only stop rule of revision 2 and assert this test is the first failure. |
| A parked owner dequeues nothing at all | While `awaiting_gated` is set, assert the owner's `input_queue` length does not change across ticks, and that a sibling owner keeps applying in those same ticks. |
| A lost reply resumes the owner | Drive past `timeout + grace`, assert a `Timeout` result, a cleared `awaiting_gated`, and that the next command applies. |
| The legacy blocking path is unchanged | Characterization test pins the existing timeout and the reject-concurrent-gated-call behavior across the submit/poll refactor. |
| One apply error does not abort the tick | Force a per-command runtime error, assert exactly one teardown and that a sibling's delivery in the same tick still applied. |

### Fail-closed reporting tests, from finding 4

| Claim | Test |
| --- | --- |
| A malformed frame closes exactly one subscription | Assert `pressure()` is `Closed`, one teardown row, inventory shows the route gone, and the sibling count is unchanged. |
| No undeliverable result is promised | Assert `TerminalInputRejection::ALL` contains no `Malformed` or `QueueOverflow` variant, so the published surface cannot claim a report it cannot deliver. |
| Live-owner results do arrive | `StaleMode`, `PartialWrite`, `Timeout`, and `SessionNotWritable` each reach the client through the adapter with the owner still bound, proved by delivered frame bytes rather than by an accepted write. |

### Control-egress tests, from findings 1 and 2 of rounds 4 and 5

| Claim | Test |
| --- | --- |
| One writer owns the write half | A source assertion that no post-spawn `write_json` or `write_frame` call site remains outside the queue, and that the write half is moved into the writer thread rather than cloned. `FRAME_SPAWN_SESSION` is the single allowed direct write and is asserted to happen before handover. |
| Framed bytes never interleave | Drive concurrent input, resize, snapshot, and ping sends from several threads and assert the worker decodes every frame intact, in send order, with no corrupted length prefix. |
| The queue capacity is exact | Fill to ordinary capacity 30 and assert the 31st ordinary send returns `ControlQueueFull` while the queue length never exceeds `WORKER_CONTROL_QUEUE_FRAMES`. |
| The reserved slots do their job | At ordinary capacity, assert a cancel still enqueues, and that a shutdown still enqueues after it. Neither is refused. |
| Shutdown is terminal | After a shutdown is enqueued, assert every later send of any class is refused. |
| Cancel is never written before its own submit | Assert FIFO order on the wire: the worker observes the submit frame before the cancel that names it. |
| Ordinary overflow is fail-closed and owner-scoped | Assert `ControlQueueFull` hard-stops exactly one owner, drops no frame, and leaves the sibling count unchanged. |
| No tick-thread call performs socket I/O | Assert the tick path reaches only `try_send`, and every `write_all` happens on the writer thread. Red-on-revert: restore the direct `write_json` call on the tick path and assert the cross-session pressure test fails first. |
| A stalled socket does not block the tick or other sessions | Stop the worker reading its control socket, drive `CoreDaemon::drain`, and assert a subscription on a **different** session completes input-to-echo. |
| A blocked write is bounded | Assert the stalled `write_all` returns after `WORKER_CONTROL_WRITE_TIMEOUT` rather than blocking indefinitely. |
| Writer shutdown is bounded and closes the FD once | Force a stall, drive session teardown, and assert `shutdown(Shutdown::Write)` unblocks the writer, the join completes within `WORKER_CONTROL_WRITER_JOIN_BOUND` or detaches, the FD is closed exactly once, and teardown itself never blocks. |
| Same-session pressure is fail-closed, not silently isolated | Two subscriptions on **one** session, one control write stalled. Assert both are delayed but live within `WORKER_CONTROL_WRITE_TIMEOUT`, then that **both** are hard-stopped together past it, and that a third subscription on another session was unaffected throughout. This tests the stated policy rather than an isolation claim this layer cannot make. |

### Ingress-loss tests, from finding 4 of round 2

| Claim | Test |
| --- | --- |
| Loss is observable, never silent | A harness driver drops one injected frame. `try_read` returns `Lost` on the next call and does not return a later `Frame` before that `Lost` is observed. |
| Loss is fail-closed | After `Lost`, Core hard-stops that owner: `pressure()` is `Closed`, one teardown row, inventory shows the route gone, and no further frame from that adapter reaches the PTY. |
| Loss cannot be confused with an empty buffer | `Empty` leaves the owner live and the queue unchanged; `Lost` tears it down. The two are asserted in the same test so an `Option`-shaped regression fails. |
| The required buffer floor holds | A conforming adapter accepts `MIN_ADAPTER_INGRESS_BUFFER_FRAMES` complete frames without reporting `Lost`, and Core's intake budget drains all of them in one tick. |
| Byte fidelity is never claimed across a gap | A red-on-revert control makes the adapter drop a frame silently and return `Empty`; the ordering and byte-fidelity test must become the first failure. |

### Gated-cancellation tests, from finding 3 of round 2

| Claim | Test |
| --- | --- |
| An abandoned request writes nothing to the PTY | Hold a submitted request with `test_mode_gated_hold_ms`, tear down generation N during the hold, release the hold, and assert the worker replied `cancelled` with `bytes_written = 0` and that the PTY received zero bytes from that request. Red-on-revert: drop the in-barrier cancel check and assert this test fails first. |
| The lane is released, and only then | Assert generation N+1's gated request is admitted after the cancelled reply arrives, and not before it. A replacement never overlaps an abandoned request. |
| The lane is still bounded when no reply comes | Drop the reply entirely and assert the slot frees at `timeout + grace` rather than never. |
| The cancel race has no third outcome | Cancel after the write has completed and assert the reply reports `admitted = true` with the exact `bytes_written`, so the plan never claims a suppressed byte that was actually written. |
| The cancel observation is O(1) and off the barrier | Assert the control reader thread consumes `FRAME_MODE_GATED_CANCEL` and never forwards it to `frame_receiver`, and that a `FRAME_PTY_INPUT` sent during the hold is still applied in order. Nothing is scanned or deferred inside the barrier. |
| A stale cancel cannot leak into a later request | Send a cancel naming an already-finished `request_id`, then submit a new gated request, and assert the new one is admitted normally. |
| Every owner-removing path cancels | Drive `hard_stop_key` through malformed frame, ingress `Lost`, and queue overflow, plus `detach_live`, `detach_generation`, `teardown_session`, and `teardown_all`. Each asserts the cancel frame was sent and the lane released on reply. |
| A replacement cannot submit while the abandoned lane is held | Cancel generation N, and while its lane is still `Cancelled`, assert generation N+1's gated command stays queued and is never submitted. Revisions 4 and 5 asserted the opposite ordering, which §5.8 makes unreachable. |
| A replacement proceeds cleanly after release | Release N's lane through the cancelled reply, then assert N+1 submits with a fresh `GatedRequestId` and completes, and that N's stale cancel cannot affect it. |
| A late reply after cancellation is discarded | Release the held reply after cancellation. Assert it is dropped, no `input_result` is synthesized, and no owner receives a frame. |
| A sibling session is unaffected | Assert a second session's gated lane is untouched throughout each cancellation above. |

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
4. Gated-primitive commit — `submit_mode_gated_pty_input`,
   `poll_mode_gated_pty_input`, and `cancel_mode_gated_pty_input`, plus the
   `FRAME_MODE_GATED_CANCEL` frame, the worker's cancellation cell, and the bounded
   parent control egress queue with its writer thread. The blocking method is
   reimplemented on the same primitives and keeps its characterization test. The
   fence and the egress bound land here so review can verify both separately from the
   ingress consumer. This lands **before** the runtime commit so review can
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

8. **Crate dependency direction must be read from the manifests, not assumed.**
   Revision 2 of this plan proposed a re-export that would have created a Cargo
   cycle, because it assumed the wrong direction between `botster-core` and
   `botster-terminal-protocol-client`. The shipped rule is that a lower crate never
   names a higher one, and Core maps at the boundary instead.
9. **A transport that can drop input must report the drop, or byte fidelity is a
   false claim.** The `TerminalIngress::Lost` contract is the durable lesson: an
   `Option`-shaped read cannot distinguish idle from lost.
10. **Runtime state outside the owner needs an explicit cancel on teardown.**
    Removing a `ClientWorker` owner does not release a session-level slot. This
    belongs beside [[webrtc peer cleanup removes every per peer owner together]].

11. **Cancelling a request at the sender does not cancel it at the receiver.**
    Revision 3 cleared only the parent slot and believed a deadline was a fence. A
    deadline bounds a late side effect; it does not prevent one. A cancellation is
    only real when the party that performs the effect checks it inside the same
    critical section that performs the effect.

12. **A fence must not be the most expensive step in the critical section it
    protects.** Revision 4 put an unbounded channel scan inside the PTY barrier to
    look for a cancel. A dedicated single-slot signal set off the hot path is both
    cheaper and simpler than scanning a queue for the message you want.
13. **"Non-blocking" is a claim about the call stack, not the intent.** Revision 4
    called submit and cancel non-blocking while they reached `write_all` plus `flush`
    on a socket with no deadline. Verify the leaf call before writing the word.

14. **A queue in front of a blocking socket moves the stall; it does not bound it.**
    The writer thread needs its own write deadline and a named hard stop, or the
    bound is only apparent.
15. **One serialization owner means ownership by move, not by convention.** Leaving
    other write sites able to touch the same handle is how framed bytes interleave.
16. **State the isolation you can deliver, not the one you want.** Same-session
    subscriptions share one worker and one control channel and cannot be isolated
    there; a fail-closed policy that is tested beats an isolation claim that is not.

No gap beyond these sixteen was found.

## 19. Plan Review response

### Round 5: `review_1787636806_488333`, verdict `changes_required`

Round 5 confirmed that revision 5's O(1) cancellation cell and its move of three sends
to a bounded queue were right. Three issues remained. The third is a bookkeeping failure
of mine and is recorded as such.

| Finding | Severity | Resolution |
| --- | --- | --- |
| `finding_1787636806_379244` — route every post-spawn control write through one bounded writer | high, product | **Confirmed real.** Revision 5 moved only submit, cancel, and plain input. Source shows post-spawn writes also at `worker_process.rs:417` (`FRAME_SET_TIMEOUT`), `:441` (`FRAME_GET_MODE_FLAGS`), `:599`, `:689`, `:710`, `:1389` (`FRAME_GET_SNAPSHOT`), `:1308` (`FRAME_RESIZE`), plus `write_frame` sites at `:384` (`FRAME_PING`), `:1304` (`FRAME_PTY_INPUT`), and `:1227`, `:1312`, `:1398` (`FRAME_SHUTDOWN`). If those keep the write half the writer cannot own it; if they clone the stream, two `write_all` calls interleave framed bytes. §5.8.2 now gives the writer the write half **by move**, lists every post-spawn frame with its site and class, and excludes only `FRAME_SPAWN_SESSION` at `:1197`, which is the last write before handover. Three admission classes share one FIFO queue: Ordinary, Cancel, and Terminal. `WORKER_CONTROL_RESERVED_SLOTS` rises to 2 so shutdown is always sendable; revision 5 reserved only a cancel slot and left a required teardown frame competing with ordinary traffic. §12 adds source-assertion, frame-integrity, capacity, reserved-slot, terminal-shutdown, and overflow rows. |
| `finding_1787636806_378579` — bound writer shutdown and prove same-session pressure | high, product | **Confirmed real, and the second half changes a claim rather than adding a mechanism.** A queue in front of a blocking socket moves the stall; it does not bound it. §5.8.3 sets `WORKER_CONTROL_WRITE_TIMEOUT` through `UnixStream::set_write_timeout`, names `shutdown(Shutdown::Write)` (or dropping `ChildStdin`) as the hard stop that returns an in-progress `write_all` at once, gives the session teardown path as the shutdown owner, closes the FD exactly once, and joins under `WORKER_CONTROL_WRITER_JOIN_BOUND` before detaching. On same-session isolation the reviewer is right that I could not prove it, and the honest reason is that it is **not true**: a session has one worker and one ordered control channel, so a stalled write delays every later frame for that session. §5.8.3 therefore states a fail-closed policy instead of an isolation claim — delayed within the timeout, all same-session subscriptions hard-stopped together past it, per [[webrtc peer cleanup removes every per peer owner together]] — and §12 tests that policy plus an unaffected third session. Cross-session isolation, which is what the ticket's sibling requirement needs, remains complete. |
| `finding_1787636806_905150` — obsolete acceptance rows and missing pressure tests | medium, product | **Confirmed, and this one was my process error, not a design gap.** My round-4 edit script hit an `AssertionError` on an unrelated anchor and exited **before** its `write`, so three §12 changes were silently lost: the two obsolete rows stayed, and the control-egress suite never landed. I then re-ran only the surviving fragments and reported the tests as added. §19 asserted six tests that §12 did not contain, which is precisely the "prose asserting what no mechanism performs" failure this plan keeps recording. Revision 6 lands the full suite, deletes both obsolete rows, adds the reachable replacement ordering, and **verifies every §19 claim against §12 by grep before submission**. |

### Round 4: `review_1787636259_958552`, verdict `changes_required`

Round 4 confirmed that revision 4's worker-visible cancel fence and the corrected
`TerminalIngress` conformance table were right. Two defects remained, both introduced
by that same fix, plus one self-contradictory acceptance row.

| Finding | Severity | Resolution |
| --- | --- | --- |
| `finding_1787636259_772231` — bound mode-gated submit and cancel control writes | high, product | **Confirmed real.** `worker_process.rs:2247` writes through `stream.write_all(&frame).and_then(\|_\| stream.flush())` with no deadline and no bounded writer queue, and a submit can carry up to `MAX_MODE_GATED_DATA_BYTES`. Revision 4 called submit and cancel non-blocking while they reached exactly that leaf call, so a stalled worker control socket would have blocked `CoreDaemon::drain` during submit or teardown during cancel. §5.8.2 adds a bounded per-session control egress queue with its own writer thread, mirroring the pattern the worker already uses for its egress (`botster-session-worker.rs:1335`, `spawn_writer`). Tick-thread calls only `try_send`. `WORKER_CONTROL_QUEUE_FRAMES` is 32 with `WORKER_CONTROL_RESERVED_CANCEL_SLOTS` of 1, so a cancel always fits, and it rides the same FIFO queue rather than a priority lane, which is what guarantees the worker sees a submit before its cancel. Ordinary overflow is fail-closed to that owner alone. A wedged socket degrades one session, bounded by `timeout + grace`, while the tick and every sibling keep progressing. §12 adds the control-egress suite. **Correction:** revision 5's §19 claimed those tests while a failed edit script left §12 without them; revision 6 lands the suite and §12 is verified to contain every row §19 names. |
| `finding_1787636259_795563` — bound and reconcile the worker cancel observation path | high, product | **Confirmed real.** `botster-session-worker.rs:136` creates `frame_receiver` with unbounded `mpsc::channel()`, so revision 4's in-barrier scan had no work bound, and its deferred buffer had no capacity, no overflow rule, and no defined replay owner. §5.8.1 replaces the scan with a single-slot cancellation cell that the control reader thread sets, so the barrier does one lock, one id compare, and one clear. That is O(1), removes the deferred buffer and its replay ordering entirely, and is strictly simpler than what it replaces. One slot is exact rather than a guess, because at most one gated request per session is in flight and the parent sends at most one cancel per request. The reviewer also caught that the replacement acceptance row cancelled generation N *after* N+1 had submitted, which this plan's own lane-hold rule makes unreachable; §12 now asserts the reachable pair instead: N+1 stays queued while N's lane is held, then submits cleanly after release. |

### Round 3: `review_1787635689_864971`, verdict `changes_required`

Round 3 confirmed that revision 3 resolved the semantic dependency, size, owner
ordering, and observable-loss findings. Two findings remained.

| Finding | Severity | Resolution |
| --- | --- | --- |
| `finding_1787635689_923187` — cancel abandoned gated input at the worker correctness boundary | high, product | **Confirmed real.** Revision 3's `cancel_mode_gated_pty_input` cleared only the parent slot while explicitly leaving the worker protocol unchanged. The worker still owned the submitted request and could pass its own freshness check and write to the PTY before `deadline_unix_ms`. A deadline bounds a late write; it does not prevent one. Freeing the parent slot immediately also let a replacement overlap the abandoned request. §5.8 now adds `FRAME_MODE_GATED_CANCEL` and checks it **inside** `runtime.with_pty_io_barrier`, after the deadline and freshness checks and strictly before `barrier.write_input`. The worker's control frames already arrive on a reader thread feeding an mpsc channel (`botster-session-worker.rs:896`), so a cancel sent during an open barrier is already queued and the in-barrier drain sees it; non-cancel frames drained in that pass are stashed and replayed in order. The parent now holds the lane in a `Cancelled` state until the correlated reply or `timeout + grace`, so no replacement overlaps. The race is total with exactly two outcomes, and the reply always reports which occurred, so the plan never claims a suppressed byte that was actually written. **This withdraws revision 3's worker-protocol non-scope for one frame**, recorded in §4: the ticket assigns Core "mode-gated input, generation, close, recovery, and teardown", and a teardown that cannot stop a write it already authorized is not a teardown. §12 adds five worker-fence tests including a red-on-revert control that drops the in-barrier check. |
| `finding_1787635689_715966` — conformance arms still Option-shaped | medium, product | **Confirmed real.** §5.4 changed the return type to `TerminalIngress`, but the §12 conformance table still required `None` for the empty and closed states, so those arms could not implement the stated trait contract, and no arm named `Lost` even though the plan relies on the published harness to enforce loss reporting. The table now requires `Empty` for a fresh or idle adapter, `Closed` permanently after both close paths, and adds `assert_ingress_lost` with its ordering assertion and the `MIN_ADAPTER_INGRESS_BUFFER_FRAMES` floor. |

### Round 2: `review_1787635010_824294`, verdict `changes_required`

Four product findings against revision 2. All four were confirmed against the
repository source before this revision changed anything.

| Finding | Severity | Resolution |
| --- | --- | --- |
| `finding_1787635010_822759` — semantic codec dependency cycle and fallible size handling | high, product | **Confirmed: my error.** The manifests show `botster-core` depends on `botster-terminal-protocol-client`, which depends on `botster-terminal-protocol`; the client crate depends on nothing from Core. Revision 2's proposed re-export of `ModeFreshnessToken` would have created a Cargo cycle. §5.2 now carries `mode_generation` and `mode_revision` as plain `u64` fields on `TerminalInputCommand`, and §5.3 defines `TerminalModeFlags` in the client crate, with Core mapping at its single encode and decode sites. That is the pattern `client_worker.rs::encode_terminal_frame` already uses for egress, so it adds no new mirror class. §5.2 also makes `encode_terminal_input` fallible with `PayloadTooLarge`, and §5.6 fixes the per-kind ceilings: 65,535 data bytes for `input`, 65,519 for `mode_gated_input` after its 16-byte prefix, and exactly 4 for `resize`. §12 asserts both ceilings separately and asserts the absence of a cycle. |
| `finding_1787635010_389980` — owner ordering while a gated command is pending | high, product | **Confirmed real.** Revision 2's Stage B stopped only when the queue *head* was another `ModeGatedInput`, so on the next tick a plain `Input` behind a pending gated command could leave the queue and reach the PTY ahead of it. That contradicted this plan's own ordering guarantee and its own acceptance test. §5.5 step 2 now parks the owner completely: while `awaiting_gated` is set the owner dequeues nothing at all, and only a correlated result, a timeout, or teardown clears it. Other owners stay eligible. §12 adds a red-on-revert control that restores the head-only rule and must fail first. |
| `finding_1787635010_882023` — cancelling the gated runtime slot on owner teardown | high, product | **Confirmed real.** `hard_stop_key` removes a `ClientWorker` owner, but `gated_in_flight` is session runtime state, so revision 2's matrix asserted a clearing that no primitive performed. §5.8 adds `cancel_mode_gated_pty_input(session_id, request_id)`: non-blocking, clearing the slot only on an exact id match, relying on the worker's own `deadline_unix_ms` fence to bound the abandoned request, and leaving a late reply to be discarded by the existing correlation check with no `input_result` synthesized. §5.7 step 5 drives it from one place over the teardown vector every ingress stage already returns, so all seven owner-removing paths inherit it. §12 adds generation-reuse and sibling tests. |
| `finding_1787635010_791293` — receive-buffer overflow must be observable and fail closed | high, product | **Confirmed real.** Revision 2 said a full receive buffer would report through `pressure()`, but `TerminalAdapterPressure` describes only egress readiness, the single active write, and close, and `Option<Vec<u8>>` cannot distinguish idle from lost. §5.4 replaces the return type with `TerminalIngress { Empty, Frame, Lost, Closed }`. `Lost` carries no payload, so it stays content-blind; it must precede any later frame; and Core hard-stops that owner because a gap in a terminal byte stream is unrecoverable. §5.6 sets `MIN_ADAPTER_INGRESS_BUFFER_FRAMES` to 64, equal to the intake budget, so a conforming adapter never reports `Lost` to a host that drains every tick. §12 adds five ingress-loss tests including a red-on-revert control for silent loss. |
| `finding_1787634119_476601` — Plan gate evidence | info, process | The revision 2 gate carried a full summary and every required field. The reviewer notes `step.completed` evidence is still empty; that event is written by the pipeline engine on advance, not by this agent's gate submission, so it is not something the Plan step can populate. Gate evidence and the gate summary are both complete again here. Flagged for the engine rather than re-planned. |

### Round 1: `review_1787634119_893294`, verdict `changes_required`

| Finding | Severity | Resolution |
| --- | --- | --- |
| `finding_1787634119_990692` — usable semantic API across the opaque boundary | high, product | §5.2 splits the opaque Hub-safe carrier from the client-crate semantic codec, using the existing crate dependency direction and adding no new edge. §12 adds four boundary-usability tests, including a dependency assertion that Hub never gains the semantic crate. Confirmed as a real defect: revision 1's carrier could not be decoded by Core or constructed by any client. |
| `finding_1787634119_562904` — ingress queue lifecycle and overflow proof | high, product | §5.5 separates intake from apply with exact enqueue and dequeue rules and distinct budgets (64 intake, 16 apply). §5.6 fixes both constants. The capacity bound is now reachable, and §12 drives it through the production `CoreDaemon::drain` loop. Confirmed as a real defect: revision 1 could not reach 257. |
| `finding_1787634119_650778` — synchronous mode-gated input blocks siblings | high, product | Confirmed against source: `worker_process.rs:769` blocks up to `5 s + 1 s` in a sleep loop. §5.8 adds `submit_mode_gated_pty_input` and `poll_mode_gated_pty_input`, keeps the blocking method for the legacy JSON path, and confines stalling to one owner's queue. §5.7 defines per-command error isolation so an apply error never aborts the shared tick. §12 proves it with the existing `test_mode_gated_hold_ms` hook and a red-on-revert control. |
| `finding_1787634119_343164` — `input_result` versus immediate hard-stop | high, product | §5.3 removes the `Malformed` and `QueueOverflow` variants. Fail-closed teardown reports by close, which is the signal every other Core hard-stop already uses. §12 asserts the close and asserts that the published rejection inventory contains no undeliverable variant. Confirmed as a real contradiction. |
| `finding_1787634119_534987` — required Botster maps and release identity | high, product | §2 loads [[botster-architecture]], [[cli-patterns]], and [[spa-patterns]] and records their effect. §2 and §3 add [[conformance fixture revisions must be unique per published content]] with verified registry evidence: the registry holds only `0.1.0` at revision 1 with no duplex token, so revision 2 and version 0.2.0 are free. §14 re-verifies before publish. |
| `finding_1787634119_476601` — incomplete Plan gate evidence | info, process | Gate evidence is resubmitted complete, with a non-empty gate summary. The existing artifact `artifact_1787632944_418165` and the existing checklist `checklist_1787632899_874706` are reused. No second artifact and no second vault checklist are created. |
