# Content-blind terminal adapter

Core owns a content-blind terminal egress adapter contract and a
transport-neutral conformance harness.

ClientWorker now pushes bound-subscription frames through this trait. See
[`client-worker-terminal-egress.md`](client-worker-terminal-egress.md). The
production entry points are `CoreDaemon::bind_waking_terminal_adapter` and
`DefaultBotsterEngine::bind_waking_terminal_adapter`. Bind requires a live
generation and an immutable `TerminalCapabilitySet`. The bind allocates route
wake state only after the rejection ladder. A single-thread host can wait
with `CoreDaemon::wait_wakes`. A host-owned data-plane thread uses
`CoreDaemon::wait_pump` and `WakePumpControl`. Both paths advance only named
routes with `CoreDaemon::pump_woken`. Drain and lifecycle observation do not
advance bound adapters.

Core clamps `wait_wakes` and `wait_pump` to the earliest paste assembly or
pending resize deadline. Expired paste deadlines name subscription routes;
expired resize deadlines name session ingress. Core merges these names into
ordinary wake batches, including batches that already contain sibling traffic.
Hosts pump these batches through the existing wait/pump contract. No timer
thread or content-aware host code is required.

One wake-driven tick has three ordered phases. Core first intakes only the
named adapter routes and drains only named session ingress. It then applies
accepted input only for those exact routes, plus generation-matching owners
that Core previously parked for worker control-queue capacity. Finally it runs
the existing single adapter egress pump. Core can emit a resize `input_result`
for accepted input in that tick. That result does not confirm worker-applied
geometry. A full ordinary worker control queue leaves the command in its
bounded owner queue. The writer's transition away from full emits one coalesced
session wake for retry. A sealed queue is a hard stop rather than a parked retry.

A worker-backed ingress resize remains pending after parent queue admission.
Core records an absolute deadline at acceptance, using `mode_gated_input_timeout`.
The worker sends `FRAME_RESIZE_APPLIED` only after Ghostty and the PTY accept the
resize. The reader queues the acknowledgement and wakes that session.
`CoreDaemon::pump_woken` reconciles available acknowledgements without waiting
for missing acknowledgements in the shared pump. A pending resize therefore
does not impose an acknowledgement wait on later-arriving sibling input.

The daemon persists the latest acknowledged geometry through `SessionRegistry`.
When that geometry changes, the daemon also appends a lifecycle upsert.
An identical later resize still reaches the worker and emits its `input_result`.
It does not rewrite unchanged geometry or append a duplicate lifecycle upsert.
Registry filenames remain private to `SessionRegistry`.

Core reconciles available acknowledgements before checking resize expiry.
An expired pending resize fails only the named live session's control plane
and tears down that session's owners. Core preserves the last confirmed geometry.
For a session already stopping or exited, Core clears pending resize entries
without timeout teardown, preserving pending terminal exit delivery.

Core caps pending ingress resizes at 30 per session. When the cap is full,
the next resize stays in its owner's queue and later input remains behind it.
An acknowledgement wake lets Core retry that parked owner. An explicit
`CoreDaemon::resize` returns `ExplicitResizeBusy` while that session has pending
ingress resizes, before any worker or registry mutation.

The session worker protocol version is 3. This version requires
`FRAME_RESIZE_APPLIED`. Core rejects workers with another protocol version during
spawn and adoption.

The local engine applies a PTY resize synchronously. Its targeted input path
records the applied size immediately after success. Thus, the local engine does
not create worker-only pending acknowledgment state.

`pump_woken` returns a content-free outcome. Core commits lifecycle observations
and retains unconsumed drain content before the method returns. A host does not
inspect or retain terminal bodies from this outcome. Routing `ProcessExited`
does not retire the session ingress wake. `CoreDaemon` retires that wake only
after the lifecycle registry, lifecycle journal, and required final terminal
state all commit. Shutdown acceptance does not retire the wake.

## Adapter vs `TransportEgress`

`contract::transport::{TransportIngress, TransportEgress}` remain the current
semantic drain-path frame enums. They are not adapter traits.

`contract::terminal_adapter::{TerminalAdapter, TerminalAdapterWriteError,
TerminalAdapterPressure, TerminalIngress}` is the write/close/pressure and
ingress seam. It does not overload the transport enum names.

`contract::terminal_wake::{WakingTerminalAdapter, TerminalWakeKind,
TerminalWakeSink, TerminalWakeSource, TerminalWakeInterrupt}` is the
wake-driven seam.
`WakingTerminalAdapter` is a supertrait of `TerminalAdapter`. Wake kinds are
`Writable` and `Closed`. Sinks hold a weak handle so a host-retained clone
cannot pin Core memory after hard-stop. Ingress session handles share one
registry-owned coalesce state per live `SessionId`. The public surface
exposes no `RawFd`.

`TerminalWakeInterrupt` is transport-neutral. It names no route or session.
Repeated interrupts coalesce until an interruptible wait consumes the flag.
Real wakes win over a concurrent interrupt. A full channel can drop the
interrupt node because queued work already prevents the waiter from blocking.
An interrupt does not clear the overflow flag.

The host must use one waiter. `CoreDaemon::wait_pump` drains at most
`WAKE_QUEUE_CAPACITY` channel nodes in one call. After a stop, one real
collision batch can win. Every later pump wait returns `Stopped` without a
channel read. Core shutdown owns the final bounded drain.

`try_read()` returns `TerminalIngress::{Empty, Frame, Lost, Closed}`. After
`close()`, `try_read` stays `Closed` and buffered ingress is dropped. `Lost`
is fail-closed: Core hard-stops that owner and does not decode later frames.
A conforming adapter holds at least `MIN_ADAPTER_INGRESS_BUFFER_FRAMES` (64)
complete frames. `CoreDaemon::pump_woken` consumes ingress only for routes
named by adapter wakes.

The start-here path remains spawn → attach → drain → input → shutdown through
`botster_core::prelude`. This trait is an advanced host/adapter module. It is
not in the prelude.

## Write, close, and pressure laws

`try_write` accepts `&botster_terminal_protocol::TerminalFrame`.

| Result / signal | Meaning | Adapter may retain the frame? |
| --- | --- | --- |
| `Ok(())` | The frame occupies the single active-write slot until the transport finishes that write | Yes, that one in-flight frame only |
| `WouldBlock` | Transport is not ready even though the write slot is empty | No |
| `Full` | The one active-write slot is occupied | No |
| `Closed` | Adapter is closed. Further writes stay `Closed` | No |

Additional laws:

- `close()` is idempotent and non-blocking. After `close()`, `try_write`
  returns `Closed` and `pressure()` is `Closed`. `close()` and `Drop` must
  return without waiting for transport I/O or a lock held by the transport
  writer.
- Transport-side death has the same `Closed` effect as local `close()`.
- The one in-flight slot is transport state, not a second policy queue.
- The adapter must not retry a rejected write, reorder accepted frames, or
  inspect `TerminalFrame` bodies. `TerminalFrame::to_bytes()` is allowed.
- Framing, encryption, and ciphertext chunking may happen inside the one
  active write. Chunks of frame N must not interleave with frame N+1.
- Subscription queues, attach state, slow-client policy, and snapshot
  resynchronization stay in Core ClientWorker.

Public enums are not `#[non_exhaustive]`. Adding a variant at `0.1.0` is a
breaking change.

## One-slot rule

Capacity is exactly one transport-internal active write. It is not a
configurable queue. `Ok(())` occupies the slot. The next `try_write` is
`Full` until that write completes.

## Close during an active write

If `try_write` has returned `Ok(())` and the write has not completed, local
`close()` and transport-side close both abandon that in-flight frame.

- `delivered_frame_bytes` does not grow
- `pressure()` becomes `Closed`
- later `try_write` returns `Closed`
- completing the abandoned write is a no-op

Implementations must not flush-then-close, deliver after `Closed`, or keep
the slot `Full` after close. Terminal frames do not retry. The abandoned
frame is lost. Later recovery is a fresh attach on the sibling ClientWorker
ticket.

## Harness ownership

`botster-core-test-support::terminal_adapter` is always on. It does not
require `local-runtime` or `ghostty-terminal`. Adapter laws do not live in
the PTY `conformance` module.

The harness proves deterministic invariants through driver hooks, not
timing: bounds, ordering, typed rejection, close propagation, close during
an active write on both close paths, no adapter retry, and content-blind
writes.

Published Core test adapters:

| Driver | What it simulates | What it is not |
| --- | --- | --- |
| `FakeTerminalAdapter` | In-memory one-slot sink | Policy queue, retry, attach state |
| `UnixShapedTerminalAdapter` | Ordered byte pipe with one in-flight write | Real `UnixStream` |
| `WebRtcShapedTerminalAdapter` | One in-flight write that may split ciphertext into chunks | Real DataChannel, DTLS, SCTP, or Hub crypto |

Hub test support does not own this harness. Later Hub tickets import it.

## Out of this crate's adapter contract

Not implemented here:

- Real Unix sockets or WebRTC DataChannels
- Changing `TransportIngress` / `TransportEgress` enums
- Changing `botster-terminal-protocol` public accessors

ClientWorker bind, queues, and subscription teardown live in
[`client-worker-terminal-egress.md`](client-worker-terminal-egress.md).
