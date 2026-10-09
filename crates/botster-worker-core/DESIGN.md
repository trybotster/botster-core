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

Today the host engine registers the route (`attach`, OU-1) and emits `Action::HandoffRoute{link, route, transport,
options}` after the launch (`flush_handoffs`). No component sends `HostMsg::AttachRoute`, and the testkit edge drops `route`
and `options`. P4a changes this as follows.

**Sender (host).** The descriptor travels with the first byte of its `AttachRoute` frame, in the link's ordered outbound
path (the `SCM_RIGHTS` model):

1. On `HandoffRoute`, the driver encodes `HostMsg::AttachRoute{route, options}` into the link's outbound buffer like any
   frame. It records a mark: the offset of the frame's first byte, the route, and the endpoint. The mark owns the endpoint.
2. The writer sends bytes up to the mark with `link_send`. A partly written earlier frame is therefore always complete
   first.
3. At the mark, the writer calls a new edge (it replaces `handoff_route`):
   `link_send_descriptor(link, bytes, endpoint) -> Result<usize, (StreamEndpoint, DescriptorSendError)>`. It has three
   outcomes:
   - `Ok(n)`, `n >= 1`: the link took the descriptor and the first `n` bytes of the frame. The edge owns the endpoint
     from here (real: the host closes its copy after `sendmsg`). The mark is consumed, and the rest of the frame goes by
     `link_send` like any bytes. The descriptor is therefore sent once and the frame bytes once;
   - `Err((endpoint, Blocked))`: `WouldBlock` or `Interrupted`, no byte and no descriptor taken. The edge gives the
     endpoint back; the mark keeps it and the frame stays unstarted. The writer tries the mark again at the next write
     readiness of the link;
   - `Err((endpoint, Failed))`: any other error, nothing taken. The driver closes the endpoint and drops the frame (it
     was not started, so the framing stays intact).
4. `Ok` feeds `Input::HandoffSent{route}` to the engine. `Failed` feeds the route to `failed_handoffs` (`HandoffFailed`,
   as today). `Blocked` feeds nothing.
5. The marks are link-scoped. When the link closes, each mark is dropped, its endpoint is closed, and its route is a failed
   handoff, unless the session's loss closed the route first (the first reason wins, OU-2).

**Receiver (worker).** A binding delivers each descriptor before the link bytes that it rides with:

- testkit: a new `LinkEnd::send_with_descriptor(bytes, descriptor)` tags the descriptor with the stream offset of its
  first byte. A `recv` never returns bytes across a tagged offset, and the descriptor becomes readable at that offset. The
  worker driver reads it and feeds `Input::Descriptor` before the `LinkBytes` that start at the offset;
- real (later, real-only): `recvmsg` returns the descriptor with the segment that starts at its byte.

The machine keeps a FIFO of unbound descriptors for the current link. An `AttachRoute` binds the oldest one. An
`AttachRoute` with no unbound descriptor is a host fault: the worker closes the link, as for a bad frame. At `LinkClosed`
and at `AdoptLink` (DP-8), the machine emits `CloseDescriptor` for each unbound descriptor of the old link.

### Worker-core inputs and actions (sans-IO)

The worker holds no transport object. The driver keeps each received descriptor in a table and gives the machine an id.

| New `Input` | New `Action` |
|---|---|
| `Descriptor(DescriptorId)`: the link delivered one, before the bytes that it rides with | `BindRoute{descriptor, route}`: the transport of `descriptor` is `route`'s from now |
| `RouteBytes{route, bytes}`: bytes that the route delivered, in order | `CloseDescriptor(DescriptorId)`: close an unbound descriptor |
| `RouteWritten{route, result}`: the answer to `RouteWrite`. `Ok(n)` is the bytes that the kernel accepted (the progress point of OU-3a), and `Ok(0)` waits for `RouteWritable`. `Err(errno)` is a terminal write error: the route closes `WriteFailed`. The driver never reports `WouldBlock` (it is `Ok(0)`) or `Interrupted` (the driver writes again) as `Err` | `RouteWrite{route, bytes}`: one write; at most one is out per route |
| `RouteWritable{route}` | `RouteRead{route, on}`: read interest (off while the admission point is full, DP-5) |
| `RouteClosedByPeer{route}`: a read returned `Ok(0)`, a reset or another terminal read error (OU-5): the route closes `PeerClosed`. `WouldBlock` waits for read readiness and `Interrupted` reads again; neither is reported | `RouteClose{route}`: close the transport; the driver reports nothing more of `route` |
| | `PtyReadBudget(n)`: the driver reads at most `n` PTY bytes in total until the next budget; `0` stops every PTY read, a drain too |

**Source backpressure (OU-3d, OU-7).** The worker sets the PTY read budget after each step:

- the budget is the smallest free payload space of the `Open` routes (free queue bytes less the frame overhead of the
  frames that the bytes need), at most the driver's read chunk;
- a `Stalled` route does not count (OU-3b: Core stops holding the PTY for it); with no `Open` route the budget is the read
  chunk;
- a drain (`DrainPty`) obeys the budget, so the exit tail is lossless for a progressing route. A route that makes no
  progress for `reader_progress_deadline` becomes `Stalled`, stops counting, and the drain continues (OU-7: it is
  skipped).
- The worker computes the budget again when a route's write progresses (`RouteWritten`), when a route becomes `Stalled`,
  and when a route closes. Each of these can release a budget of `0`.

### The route machine (OU-2; new `src/worker/route.rs`)

- States: `Open`, `Stalled`, `Closed`. One `Route` per route id holds the state, the negotiated format and features, the
  options, a bounded output queue (`route_queue_bytes`) with the written offset of its first frame, a codec `StreamReader`
  bounded by the route limits, the last progress instant, and the latest `focus` input (DP-12).
- `Open → Stalled`: frames wait and no byte is accepted for `reader_progress_deadline`. The droppable frames that are not
  started are dropped (OU-3b), each affected query is retired (EV-8(i), below), the kept frames stay, and the worker
  sends `RouteStalled`.
- `Stalled → Open`: a write is accepted again; the worker runs the resync sequence below (reason per the extract) and
  sends `RouteResumed`.
- `→ Closed` at the first close reason of OU-2. The first reason is kept; a later reason is ignored. The worker emits
  `RouteClose` once and sends `WorkerMsg::RouteClosed{route, reason, route_tag}` once, with the first reason, after
  `RouteClose`. Two paths:
  - **healthy close** (`Detached`, `Replaced`, `Revoked`, `SessionEnded`, `SessionRemoved`, `SnapshotTooLarge`,
    `BadFrame`): the worker completes a partly written frame (OU-4), writes `route_closed` with the wire reason of OU-2b
    as the last frame, and emits `RouteClose` when the kernel accepted it. What happens to the queue before
    `route_closed` depends on the reason:
    - `SessionEnded` and `SessionRemoved` (OU-7): the reason is fixed when `Exited` is posted. The worker keeps the
      whole queue and delivers it while the route progresses. `route_closed` follows the last queued frame;
    - `Detached`, `Replaced`, `Revoked`, `SnapshotTooLarge` and `BadFrame`: the worker drops the droppable frames that are
      not started and keeps the kept frames in order (a choice where the contract is silent: no client reads output after
      a detach).

    The stall rules stay in force while a healthy close delivers. A stall alone never closes a route before
    `stall_close_after` (OU-2):
    - no byte accepted for `reader_progress_deadline`: the route becomes `Stalled` as above (droppable frames dropped,
      affected queries retired, `RouteStalled`);
    - a write is accepted again: the resync sequence, then the rest of the queue, then `route_closed`;
    - `Stalled` for `stall_close_after`: the worker emits `RouteClose` with no more frames. The report keeps the first
      reason (for example `SessionEnded`), not `StallTimeout`;
    - a terminal transport error: a failed close as below. The report keeps the first reason;
  - **failed close** (the failed-route reasons of OU-2b; the route has no `route_closed`, and the host reports
    `route_ended`): the worker emits `RouteClose` at once and writes nothing more. The reasons are distinct:
    - `PeerClosed` (read `Ok(0)`, a reset or a read error; wire `transport_lost`);
    - `WriteFailed` (a write returned an error; wire `write_failed`);
    - `StallTimeout` (`Stalled` for `stall_close_after` with no close reason before it; wire `stalled`);
    - `HandoffFailed` is the host's (the handoff above), and `SessionLost` is the host's when the worker is lost (wire
      `session_lost`). The worker never sends these two.

### The output queue and the resync sequence (OU-4, OU-9, DP-5)

Each queued frame has a class:

- **droppable**: `output` and the baseline frames (`baseline_begin`, `screen`, `history`, `baseline_end`, `live`);
- **kept**: `input_refused`, `input_done` and `route_closed`. A stall or a resync never drops them;
- **bounded**: `modes` and `terminal_query`, as DP-5 defines their bound.

The affected query (EV-8(i)). A stall discard (OU-3b) or a resync can drop the `output` prefix that a queued
`terminal_query` frame needs: the frame is then not started, and its prefix is gone from the route. For each such
affected query the worker, in the same step:

1. retires the unstarted `terminal_query` frame (it is not sent after the new baseline);
2. ends the route's answer opportunity, and admits the saved shadow fallback (the parse-point answer of EV-8(d)) in the one
   admission point;
3. answers a later client reply for that `query_id` with `input_refused{query_expired}` until the fallback is admitted, and
   with `input_refused{already_replied}` after it.

If the admission is refused (a full lane), the query stays pending on the fallback path only and is retried when room
frees; the client path never opens again. A query whose frame was already sent, or was started and then finished (OU-4),
is not affected and keeps its opportunity.

The resync sequence (at `Stalled → Open`, OU-9):

1. The worker completes a frame that it started (OU-4). The frame boundary after it is the resync point.
2. The worker drops the droppable frames that are not started, and retires each affected query as above. The kept
   frames and the other bounded frames stay in order.
3. At the resync point the worker writes `resync{reason}`, then a new baseline sequence from a new point `R`
   (`baseline_begin`, `screen`, `history`, `baseline_end`) and `live`, then the output after that `R` (with the `unfed`
   suffix as in "Frames" below).
4. A snapshot over the limits closes the route `SnapshotTooLarge` (healthy close) and writes no partial baseline.

### Frames (OU-8, OU-9, DP-3)

- On `BindRoute`, in one machine step, the worker takes the baseline at the point `R`. `R` is the model's consumed
  boundary: the bytes that the model applied. The frames are `attached{features, terminal_format, limits}`,
  `baseline_begin{rows, cols, modes}`, `screen`, `history`, `baseline_end{history}`, `live`. `features` is the
  intersection with `options.route_features`. `terminal_format` is negotiated against this worker's formats (OU-1).
- The model can hold a suffix of earlier output that it has not applied (`unfed`, for example a lone ESC). That suffix is
  after `R`. The worker writes it as the first `output` frame after `live`, then the later PTY output. Each byte after `R`
  is therefore on the route exactly once (the cut that #200's resume oracle checks).
- Two limits, both checked before any baseline frame is queued:
  - the snapshot: an encoded snapshot over `max_snapshot_bytes` cannot be offered (OU-9). The route closes
    `SnapshotTooLarge` with `route_closed{attach_failed{snapshot_too_large}}` and no baseline frame. (Whether `attached`
    comes first follows the extract's TS-1; `a8_2_baseline_inside_an_uncarriable_sequence_closes_route_snapshot_too_large`
    accepts both.)
  - the frame: `max_screen_frame_bytes` is a per-route limit. The route keeps the applied value that `attached.limits`
    reports; it is at least `max_snapshot_bytes + 1` (the snapshot and the type byte). With no route request it is
    exactly `max_snapshot_bytes + 1` (`dp_3_screen_is_one_frame_within_max_screen_frame_bytes`: 65537 for 65536). The
    check is `snapshot_payload + 1 <= applied max_screen_frame_bytes`, computed before the frame is built
    (`dp_3_frame_limit_checked_before_allocation`).
- PTY output goes to each `Open` route unchanged and in order (OU-12). The worker splits it into `output` frames whose size
  is within the route's `max_frame_bytes`, as the codec measures a frame (`bound_of`).
- Client frames (`ToWorker`) go to the one admission point that host input already uses (AM-2, DP-4, DP-9). Input is
  fire-and-forget: only `input_refused` and the `input_done` of DP-5 are written back. `Observation::ClientInput{route,
  input_rev}` reports the admission to the host.
- Exit (OU-7): the PTY tail is queued on every `Open` route before `Exited`; a `Stalled` route is skipped.

### Testkit

- `TestkitHarness::attach_stream` makes a `stream_pair`. One end goes to `Core::attach` as `RouteTransport::Stream`; the
  other end is the `RouteClient` (`write`, `read` until the deadline, `control`).
- The testkit worker binding reads tagged descriptors from its link end and binds the `StreamEnd` as the route edge. It
  reads at most the PTY read budget.
- Controls map to the existing `EndControl` hooks where one fits: `route_gate` (gate), `route_accept` (accept_at_most),
  `fail_handoff` (fail_next_handoff), `client_close` (close), `input_blocked` (the admission point is full). New:
  `route_stream_holders`, `alloc_window`/`alloc_peak`, `hold_handoff`/`release_handoff`, `route_stream_written`,
  `oracle_screen`, `oracle_screen_payload`, and a route-transport term in `edges_quiet`.

### Real-only (not in the P4a testkit PRs)

- `link_send_descriptor` with `SCM_RIGHTS` in `botster-core/src/real.rs` (today `handoff_route` returns `Err`), and the
  host closing its copy of the stream (`dp_2_stream_handoff_transfers_ownership_and_closes_host_copy`).
- The `botster-worker` binary: `recvmsg` descriptors on the `mio` loop, the PTY read budget, and the `RouteTransport` edge
  over the socket.
- The testkit minimum count and the real minimum count are reported separately in each PR.

### Proofs that the P4a PRs add (worker-core and testkit tests)

- Handoff: a `Blocked` send at the mark, then a later send (one descriptor, the frame bytes once); a short write at the
  mark (`Ok(n)` less than the frame); both receive kinds ready in the same step; an earlier control frame partly written
  at the mark; repeated handoffs on one link; a failed descriptor send (the frame is not sent, `HandoffFailed`); a link
  closed with marks pending; `AdoptLink` with unbound descriptors.
- Backpressure: a progressing slow reader loses no byte; two routes with different `max_frame_bytes`; an exit tail larger
  than the free queue space.
- Baseline cut: attach and resync while the model holds an `unfed` suffix, including a lone ESC.
- Close: a partly written frame, then a write failure (`WriteFailed`, no `route_closed`); a route that is not writable at
  the `stall_close_after` deadline (`StallTimeout`).
- Limits: a snapshot over `max_snapshot_bytes` is refused although the frame limit would take it.
- Resync: a partly written `output` frame, and kept `input_refused`, `input_done` frames, across a resync.

### Prior art (BUILD.md rule 0)

Reused, not rewritten:

- `botster-route-codec` (contracts v0.1.20): every frame type, `StreamReader`, `bound_of`, `stream_wrap` and the deflate
  helpers.
- `botster-core-edges::RouteTransport` (read, write, close) as the worker's route edge.
- The testkit's `stream_pair` and `EndControl` (gate, accept_at_most, fail_next_write, read_at_most, fail_next_handoff,
  reset) for the route controls.
- The worker's one admission point (AM-2) for route input, and the `CaptureSnapshot` encoder for the baseline.
- The consumed cut and the `unfed` suffix of #200 (`ModelLog`, `model_snapshot`).

Rejected:

- `origin/delivery/core-route-stall-resync-*` (2026-09): the old daemon architecture (638 files), not in v1. Its edges and
  its host-side relay do not fit the sans-IO worker. No code is ported.
- The legacy mechanisms that the contract's "reusable from" column names (C1 source backpressure, `ROUTE_RESYNC`, the
  Hub's `ProcessExit` ordering): P4a takes their rules (stop reading the PTY, the resync fence, exit after the tail) and
  ports no code, because they are in the old architecture.
- A host-terminated relay: it puts the host on the data path (DP-1, DP-11).
- WebRTC: withdrawn by A17.
- A descriptor id in `AttachRoute`: an id alone fixes neither the order nor the cleanup. The ordered outbound path above
  fixes both with no link message change.

### Changes outside worker-core (each is HIGH)

- `botster-core-host`: the outbound marks, `link_send_descriptor` in place of `handoff_route`, and `Input::HandoffSent`.
- `botster-core-testkit`: `send_with_descriptor`, the tagged `recv`, the worker binding, and the PTY read budget.
- No link message change: `HostMsg::AttachRoute{route, options}` exists.
