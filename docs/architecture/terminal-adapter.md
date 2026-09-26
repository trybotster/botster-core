# Terminal adapter

`TerminalAdapter` in `botster-core::contract::terminal_adapter` is the
content-blind duplex seam between Core's `ClientWorker` and a host transport.
It writes `RoutedTerminalFrame` envelopes and reads opaque
`TerminalInputFrame` bytes. It never inspects a body beyond its length.

## Contract

```rust
pub trait TerminalAdapter {
    fn try_write(&mut self, frame: &RoutedTerminalFrame) -> Result<(), TerminalAdapterWriteError>;
    fn close(&mut self, reason: TerminalRouteCloseReason);
    fn pressure(&self) -> TerminalAdapterPressure;
    fn try_read(&mut self) -> TerminalIngress;
}
```

- The adapter owns exactly one active write slot. `Ok(())` means the frame
  occupies that slot until the transport finishes it. It does not mean a
  client received the frame.
- The adapter reads `route`, `generation`, and `stream_epoch` from the
  envelope and copies the shared body bytes (`frame.frame.as_bytes()`) to the
  transport. Holding the `Arc` in the slot is allowed; copying the body into
  a private queue is not.
- Rejected writes (`WouldBlock`, `Full`, `Closed`) retain nothing. Core
  retries from its own queue.
- `close(reason)` and `Drop` return without waiting for transport I/O. They
  set `Closed` and abandon the slot. A transport may finish an envelope
  already in progress; it never starts or retries a frame after close.
- `reason` says why Core ended the route. Only the first close carries the
  route's reason; a host logs that one and ignores later closes.
- `try_read` returns whole frames in arrival order, `Empty`, `Lost` when the
  transport dropped at least one frame, or `Closed`. A conforming adapter
  buffers at least 64 complete ingress frames before it may report `Lost`.

`WakingTerminalAdapter` adds `set_wake_sink`. The sink carries `Writable` and
`Closed` wakes to the shared `TerminalWakeSource` so Core pumps only routes
that changed. Production hosts bind only waking adapters.

## What Core sends

Core writes the scheme 2 stream described in
[`terminal-protocol.md`](terminal-protocol.md). For one session event Core
encodes one `TerminalBody` and shares it across every bound route; each route
receives its own envelope with the route id, the fixed attachment generation,
and the epoch captured when the frame was queued. Route-personal frames
(attach state, snapshot pages, input results, resync) are encoded per route.

Per-route egress is bounded to 64 frames or 4 MiB. Overflow recovers that
route through `ROUTE_RESYNC`; other routes are untouched.

## What Core reads

Ingress frames are `TerminalInputFrame` bytes. Core validates the header,
decodes the command, checks strict operation id order, applies per-session
and per-client admission lanes, assembles pastes, and stages the operation
for the worker. Malformed frames and `Lost` hard-stop the route.

## Bind, detach, and teardown

`bind_waking_terminal_adapter(client, session, subscription, generation,
capabilities, adapter)` succeeds only for the live attach generation of an
existing owner that has no adapter. Rejections close the adapter with
`BindRejected`, drop it, and allocate no wake state.

A hard-stop closes the adapter, drops queued frames, releases input lanes,
and reports the in-flight worker operation keys so the host cancels them at
the worker. The client resolves outstanding operations as unknown. The
adapter's close and the `ClientWorkerTeardown` carry the same
`TerminalRouteCloseReason`:

| Reason | Core ended the route because |
|---|---|
| `Replaced` | the same client attached another subscription on the session, or another client took this subscription |
| `Detached` | the client or host detached it |
| `SessionEnded` | the session ended or was forgotten |
| `WorkerLinkFailed` | the session worker's control link failed, was sealed, or stopped acknowledging |
| `AdapterClosed` | the adapter reported `Closed` |
| `TerminalDelivered` | it delivered its final frame (process exit or attach failure) |
| `Stalled` | the reader refused writes for two full attempt budgets |
| `Overflowed` | its egress could not stay inside its bounds |
| `InputFailed` | ingress was lost, an input frame broke the protocol, or a result could not be queued |
| `Failed` | it failed before binding, or a frame could not be encoded |
| `Shutdown` | the client worker ended every route |
| `BindRejected` | Core rejected the bind; the adapter never carried the route |

## Conformance

`botster-core-test-support::terminal_adapter` publishes the adapter laws and
in-memory drivers (`FakeTerminalAdapter`, `UnixShapedTerminalAdapter`,
`WebRtcShapedTerminalAdapter`). Fixture frames are real scheme 2 `OUTPUT`
bodies built with `encode_output`; delivered bytes are the shared body bytes.
