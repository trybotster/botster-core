# botster-worker-core: P3 design note

Package P3 (session worker, PTY and terminal) of Stage 1. Plan pin `c43693ff` (and `2b03dc1b`, no change to P3's scope). This
note covers milestone M1 (plan 6.2): the hello, the payload launch after the durable identity, and the exit with code and signal.
It is stacked on P1's head `2016886` (the control-link messages of `botster-core-link`).

## Shape

- `Worker` (`src/worker.rs`) is a sans-IO machine (plan 2.1): `handle(now, Input)`, `poll_action()`, `next_deadline()`. It reads
  no clock, starts no thread and does no I/O.
- Two drivers run the same machine (plan 2.1, A5-1):
  - the `botster-worker` binary, role `session`: one `mio` loop over the control socket, the PTY master, `SIGUSR1`
    (`signal-hook-mio`) and a `mio::Waker` for the exit watch; the control link is served first in each turn (plan 2.4);
  - the testkit (`botster-core-testkit/src/worker.rs`): the `Sim` steps the machine on the test thread, with the scripted
    program as the `Program` edge and an in-memory link. `TestkitCore::pump` runs the workers' ready work, then the host's pump.
- The real `Program` edge is `botster-core-sys/src/payload.rs`.

## The protocol of M1 (P1's wire)

| Host → worker | Worker |
|---|---|
| hello (`Hello` with the token proof, AD-6) | the worker sent its own hello first; a host hello with another proof, instance or epoch closes the link |
| `Launch` (AD-7 step 4) | `SpawnPayload`; then `Launched{features, terminal, formats, payload}` or `LaunchFailed{CwdMissing \| ExecFailed{errno}}` |
| `Stop` (LC-5) | `SIGTERM` to the payload group |
| `Kill` (LC-5, at `stop_grace`) | `SIGKILL` to the payload group |
| `Op{Signal}` (LC-6) | the signal to the group, then `Done{Ok(Unit)}`; no live group: `SessionEnded` |
| `Remove` (LC-7 step 3) | `SIGKILL` to the group if it is unreaped, reap, `RemoveResult{Deleted}`, close, end |
| `SIGUSR1` (`GroupSignal::EndPayload`, LC-5, P1 F7) | `SIGTERM`, then `SIGKILL` at `stop_grace` (`LaunchSpec.stop_grace_ms`); a repeat changes nothing |

The payload's exit is `Exited{code}` or `Exited{signal}` (EV-4), sent after one drain of the PTY that follows the exit, so the
output written before the exit is read first (the final model holds it, ST-5).

## Choices where the contract is silent (each is reviewable)

| Choice | Reason |
|---|---|
| The leader is reaped only after a `SIGKILL` to its group, once the leader has ended (`Kill`, a `Signal(Kill)`, the grace of `EndPayload`, or `Remove`). A leader that ends by itself stays a zombie until one of them. | Lead ruling on P1 F7: the group id cannot be reused while the worker may still signal it. |
| The graceful request is `SIGTERM` to the group. | LC-5 says "a graceful request"; the old runtime's lesson: closing the master only sends `SIGHUP`, which agents ignore. |
| An operation that M1 does not serve is answered `Done{Err(Internal)}` naming the operation. | Every request gets its `Done`; the reads, input and setters come with M2. `Internal` is the honest code for a worker that cannot serve it yet. |
| `Launched.terminal` is a placeholder (the launch size, default flags, revisions 0) and `formats` is empty. | The terminal model is M2 (libghostty through P2). No terminal-state id leaves `core-pending.txt` in M1. |
| A host message that the worker cannot decode is ignored; a frame above the bound closes the link. | Plan section 3: additive messages keep N−1 compatible; a bad length ends a stream. |
| A closed link leaves the payload running, and the exit is kept unreported. | DP-8. Reconnection and the report after adoption are P5's (AD-1). |
| `LaunchSpec.stop_grace_ms` (additive, serde default from the Core limits table) carries `CoreLimits.stop_grace`. | The worker times the grace of `EndPayload` itself; agreed with P1. |
| `Remove` while the spawn is out waits for the spawn's answer, then ends the payload. | LC-7: no payload outlives its session. |

Not in M1: the worker's own exit when no host attaches within `startup` (AD-7, P5's id `ad_7_crash_between_steps_...`); the
worker needs `startup` on its command line for it.

## Prior art (BUILD.md rule 0)

- **Vault:** "botster workers own authoritative terminal modes and semantic input encoding" (M2 follows it); "the old PTY crate's
  master is private" (plan 7.1); "O_CLOEXEC is process-scoped and does not affect SCM_RIGHTS fd transfer" (DP-2, later).
- **Old botster-core at `72b2e33`:**
  - `crates/botster-core/src/runtime/process_exit.rs` (kqueue `EVFILT_PROC` and `waitid(WNOWAIT)` on macOS, pidfd on Linux):
    **REJECTED as a steal**, kept as a lesson. Its kqueue path needs `unsafe` (`libc::kevent`, and `rustix::event::kqueue::kevent`
    is `unsafe fn`), and the workspace forbids `unsafe` code. Lead decision: one waiter thread per payload blocks in the safe
    `rustix::process::waitid(P_PID, WEXITED | WNOWAIT)` and wakes the loop through `mio::Waker`. Lessons kept: watch while the
    child is unreaped; report the exit without reaping. Its subtle cases (`NOTE_EXIT` before the child is reapable, `ESRCH` at
    registration, the `SIGCHLD` recheck) disappear, because `waitid` returns only when the child is reapable. One code path on
    macOS and Linux.
  - `crates/botster-core/src/runtime/local_process.rs` (on the old PTY crate that plan 7.1 rejects): **REJECTED** (plan 7.1). Lesson kept: `killpg`
    `SIGTERM`, then `SIGKILL`.
  - the old session-worker binary of the old daemon crate (plan 7.1, its path is in the PR's Prior-art note): **mechanism REJECTED** (not sans-IO), lessons kept: control
    first in each turn (the binary's loop); the remaining lessons (reply slots, the snapshot gate) are M2's.
  - Nothing was stolen: there is no `Stolen-From` commit.
- **Libraries added:**
  - `pty-process` 0.5.3: opens the PTY with `rustix`, starts the payload as a session leader with the PTY as its controlling
    terminal (`setsid`, `TIOCSCTTY`), and exposes the master descriptor. It is not the PTY crate that plan 7.1 rejects: the master is ours. Its
    `pre_exec` `unsafe` is inside the library, so our crates keep `unsafe_code = "forbid"`.
  - `signal-hook` 0.3 and `signal-hook-mio` 0.2: the `SIGUSR1` handler as a readiness source of the `mio` loop.
  - `mio` (already in the workspace, P1), `rustix` (`event` feature added, for the slow tests' readiness waits).
- **Hand-rolled, with reasons:** the `Worker` machine (it is the contract); the testkit bindings (they connect the machine to
  P6's in-memory edges).

## P4a: the stream route (plan 23h)

Scope: OU-1 to OU-12, OU-2b, DP-1 to DP-7, DP-9 and DP-12 on a `RouteTransport::Stream` (A17: no WebRTC). The route
protocol is the codec extract of DP-3 (`botster-route-codec`, contracts v0.1.20); the extract governs every frame below.

### The handoff (DP-2)

- The host engine already registers the route (`attach`, OU-1) and emits `Action::HandoffRoute{link, route, transport,
  options}` after the launch (`flush_handoffs`). Today no component sends `HostMsg::AttachRoute`, and the testkit edge drops
  `route` and `options`.
- Change: the driver calls `edges.handoff_route` first. On `Ok`, the engine sends `HostMsg::AttachRoute{route, options}` on
  the same link. On `Err`, nothing is sent and the route closes `HandoffFailed` (as today).
- The worker pairs each `AttachRoute` with the oldest descriptor that it received on that link and did not bind. The
  descriptor is sent before the message, so it is always there when the message is decoded:
  - testkit: `LinkEnd::send_descriptor` queues it at once;
  - real (later, real-only): `SCM_RIGHTS` rides with link bytes that are sent before the `AttachRoute` frame.
- An `AttachRoute` with no descriptor is a host fault: the worker closes the link (as for a bad frame). A descriptor of a
  link that the worker replaced (`AdoptLink`, DP-8) is dropped unbound.

### Worker-core inputs and actions (sans-IO)

The worker holds no transport object. The driver keeps each received descriptor in a table and gives the machine an id.

| New `Input` | New `Action` |
|---|---|
| `Descriptor(DescriptorId)`: the link delivered one | `BindRoute{descriptor, route}`: the transport of `descriptor` is `route`'s from now |
| `RouteBytes{route, bytes}`: bytes that the route delivered, in order | `RouteWrite{route, bytes}`: one write; at most one is out per route |
| `RouteWritten{route, n}`: the bytes that the kernel accepted (the progress point of OU-3a); `n = 0` waits for `RouteWritable` | `RouteRead{route, on}`: read interest (off while the admission point is full, DP-5) |
| `RouteWritable{route}` | `RouteClose{route}`: close the transport; the driver reports nothing more of `route` |
| `RouteClosedByPeer{route}`: read `Ok(0)` or an I/O error (OU-5) | |

### The route machine (OU-2; new `src/worker/route.rs`)

- States: `Open`, `Stalled`, `Closed`. One `Route` per route id holds the state, the negotiated format and features, the
  options, a bounded output queue (`route_queue_bytes`) with the written offset of its first frame, a codec `StreamReader`
  bounded by the route limits, the last progress instant, and the latest `focus` input (DP-12).
- `Open → Stalled`: frames wait and no byte is accepted for `reader_progress_deadline`; the worker sends `RouteStalled`.
- `Stalled → Open`: a write is accepted again; the worker queues `resync` (same content as the baseline) and sends
  `RouteResumed`.
- `→ Closed`: the reasons of OU-2, mapped to the wire by OU-2b. The worker completes a partly written frame first (OU-4),
  then writes `route_closed` as the last frame, then `RouteClose`, then `WorkerMsg::RouteClosed{route, reason, route_tag}`.

### Frames (OU-8, OU-9, DP-3)

- On `BindRoute`, in one machine step, from the model at one point `R`: `attached{features, terminal_format, limits}`,
  `baseline_begin{rows, cols, modes}`, `screen`, `history`, `baseline_end{history}`, `live`. `features` is the intersection
  with `options.route_features`. `terminal_format` is negotiated against this worker's formats (OU-1).
- A `screen` over `max_screen_frame_bytes` closes the route `SnapshotTooLarge`; the limit is checked before the frame is
  built (DP-3 frame limit).
- Every later `PtyOutput` is one `output` frame per route with the PTY bytes unchanged (OU-12): no gap and no duplicate after
  `R`.
- Client frames (`ToWorker`) go to the one admission point that host input already uses (AM-2, DP-4, DP-9). Input is
  fire-and-forget: only `input_refused` and the `input_done` of DP-5 are written back. `Observation::ClientInput{route,
  input_rev}` reports the admission to the host.
- Exit (OU-7): the PTY tail is queued on every `Open` route before `Exited`; a `Stalled` route is skipped.

### Testkit

- `TestkitHarness::attach_stream` makes a `stream_pair`. One end goes to `Core::attach` as `RouteTransport::Stream`; the
  other end is the `RouteClient` (`write`, `read` until the deadline, `control`).
- The testkit worker binding reads descriptors from its link end and binds the `StreamEnd` as the route edge.
- Controls map to the existing `EndControl` hooks where one fits: `route_gate` (gate), `route_accept` (accept_at_most),
  `fail_handoff` (fail_next_handoff), `client_close` (close), `input_blocked` (the admission point is full). New:
  `route_stream_holders`, `alloc_window`/`alloc_peak`, `hold_handoff`/`release_handoff`, `route_stream_written`,
  `oracle_screen`, `oracle_screen_payload`, and a route-transport term in `edges_quiet`.

### Real-only (not in the P4a testkit PRs)

- `SCM_RIGHTS` in `botster-core/src/real.rs` `handoff_route` (today `Err`), and the host closing its copy of the stream
  (`dp_2_stream_handoff_transfers_ownership_and_closes_host_copy`).
- The `botster-worker` binary: route descriptors on the `mio` loop and the `RouteTransport` edge over the socket.
- The testkit minimum count and the real minimum count are reported separately in each PR.

### Questions for review

1. Pairing by the order of descriptors (above) or a descriptor id in `AttachRoute`? Order needs no wire change; an id needs
   a link message change (`botster-core-link`, HIGH).
2. The engine sends `AttachRoute` after `handoff_route` returns `Ok`. The alternative is that the edge sends both; then the
   frame bypasses the link's outbound buffer and its order with earlier frames.
