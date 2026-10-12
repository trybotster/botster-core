# botster-worker-core: P3 design note

Package P3 (session worker, PTY and terminal) of Stage 1. Plan pin `c43693ff` (and `2b03dc1b`, no change to P3's scope). This
note covers milestone M1 (plan 6.2): the hello, the payload launch after the durable identity, and the exit with code and signal.
It is stacked on P1's head `2016886` (the control-link messages of `botster-core-link`).

## Shape

- `Worker` (`src/worker.rs`) is a sans-IO machine (plan 2.1): `handle(now, Input)`, `poll_action()`, `next_deadline()`. It reads
  no clock, starts no thread and does no I/O.
- Two drivers run the same machine (plan 2.1, A5-1):
  - the `botster-worker` binary, role `session`: one `mio` loop over the control socket, the PTY master, `SIGUSR1` and
    `SIGUSR2` (`signal-hook-mio`) and a `mio::Waker` for the exit watch; the control link is served first in each turn (plan 2.4);
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
| `SIGUSR1` (`GroupSignal::EndPayload`, LC-5, P1 F7) | `SIGTERM`, and no timer; a repeat changes nothing |
| `SIGUSR2` (`GroupSignal::KillPayload`, LC-5; steward ruling R-97, follow-up) | `SIGKILL` to the group, now; a repeat changes nothing. Core sends it at the end of its `stop_grace` |

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
| The worker keeps no grace timer: `stop_grace` is the Core library's deadline, and Core sends `KillPayload` at its due `pump`. `LaunchSpec.stop_grace_ms` is removed. | Steward ruling R-97, follow-up (TM-3, E3-1 item 1): a worker timer on the worker's clock could kill before Core's due `pump`. The removal is compatible both ways (`LaunchSpec` takes unknown fields; an older worker's field has a serde default). |
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
  - `signal-hook` 0.3 and `signal-hook-mio` 0.2: the `SIGUSR1` and `SIGUSR2` handlers as a readiness source of the `mio` loop.
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

1. On `HandoffRoute`, the driver encodes `HostMsg::AttachRoute{route, options, limits}` into the link's outbound buffer
   like any frame. It records a mark: the offset of the frame's first byte, the route, and the endpoint. The mark owns the endpoint.
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
4. `Failed` feeds the route to `failed_handoffs` (`HandoffFailed`, as today). `Ok` and `Blocked` feed nothing in PR1.
   `Input::HandoffSent{route}` comes in PR3, with the consumer that needs it.
5. The marks are link-scoped. When the link closes, each mark is dropped, its endpoint is closed, and its route is a failed
   handoff, unless the session's loss closed the route first (the first reason wins, OU-2).

**The limits (correction in PR1).** `HostMsg::AttachRoute` carries `limits: AppliedRouteLimits`. The host computes
the limits once in `attach` (OU-1), returns them in `AttachResult`, and sends the same values in the frame. The worker
announces them in `attached` and enforces them as given; it computes no limit of its own. The first version of this
design said "no link message change". That was wrong: the worker cannot know the host's applied defaults and choices
(for example a `max_frame_bytes` choice) from `options` alone. A test proves that the worker announces and enforces
exactly the limits that the host sent.

**Receiver (worker; plan revision 23ae, PR B).** One link read is one `Input::LinkBytes { bytes, descriptors }`: the read
boundary is in the type, because the kernels place a descriptor segment differently in a read. macOS stops a read before
the segment once it has copied data, and goes on after it; Linux reads on into the segment and stops after it. On both, a
read holds at most one segment, and the read that holds the segment's first byte carries its descriptors.

- testkit: `LinkEnd::send_with_descriptor(bytes, descriptor)` tags the descriptor with the stream offset of its first byte
  and the length of its segment. `LinkEnd::recv_with_descriptors(buf, form)` reads in either kernel form
  (`DescriptorForm`). The testkit worker reads its link in the macOS form; each framing proof sets both forms on its edges;
- real: `fd_pass::recv_with_fds` on the `mio` loop. The driver keeps each descriptor (close-on-exec) in a table under a
  `DescriptorId`, gives the machine the ids with the read, and closes the descriptors of reads that the adoption fence drops.

The machine (`worker/handoff.rs`) attaches a read's descriptors to the one `AttachRoute` frame whose first byte is in that
read. A frame keeps the read of its first byte until it is complete. The bind is in frame order, when each `AttachRoute`
frame is complete. A host fault breaks the link: a read with descriptors holds the first byte of no `AttachRoute` frame or
of more than one, an `AttachRoute` frame starts in a read with no descriptor, its read has another count than one, or
descriptors come before the hello passes. The worker closes the link and each descriptor of that read once:
`CloseDescriptor` for each unbound one, and a route that took one ends `HandoffFailed` (lead ruling Q1). A read leaves
the machine when it is checked. Every close of the link (a fault, a frame above the bound, a frame that closes the link,
`Terminate`, `LinkClosed`, `AdoptLink`; DP-8) closes each read that is left in the same way, because it was not proven;
the routes of checked reads stay (F117). At adoption the close comes after the new link's hello and `Adopted` (F120).

### Worker-core inputs and actions (sans-IO)

The worker holds no transport object. The driver keeps each received descriptor in a table and gives the machine an id.

| New `Input` | New `Action` |
|---|---|
| `LinkBytes{bytes, descriptors}`: one link read and the descriptors that came with it | `BindRoute{descriptor, route}`: the transport of `descriptor` is `route`'s from now |
| `RouteBytes{route, bytes}`: bytes that the route delivered, in order | `CloseDescriptor(DescriptorId)`: close an unbound descriptor |
| `RouteWritten{route, result}`: the answer to `RouteWrite`. `Ok(n)` is the bytes that the kernel accepted (the progress point of OU-3a), and `Ok(0)` waits for `RouteWritable`. `Err(errno)` is a terminal write error: the route closes `WriteFailed`. The driver never reports `WouldBlock` (it is `Ok(0)`) or `Interrupted` (the driver writes again) as `Err` | `RouteWrite{route, bytes}`: one write; at most one is out per route |
| `RouteWritable{route}` | `RouteReadAllowance{route, bytes}`: the client bytes that the driver may still read from the route until the next allowance; `0` stops its reads (DP-5: the route's input bound, `route_input_queue_bytes` and at least one frame, less the bytes it holds; 0 while one more `input_refused` does not fit in `route_queue_bytes`) |
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
- Every frame is checked against its codec bound (`bound_of`: `max_screen_frame_bytes` for `screen`,
  `max_history_page_bytes` for `history`, `max_frame_bytes` for every other frame) before it is queued. The snapshot is
  checked on the library's size query, before its buffer is reserved (`snapshot_at_most`). A route whose `max_frame_bytes`
  cannot carry an attach frame closes `HandoffFailed` with no frame (steward ruling R-44: the worker holds the stream but
  cannot make a working route from the options; OU-2b sends no `route_closed` for it, and the host reports
  `route_ended{handoff_failed}`). An attach with no common terminal format closes the same way; the host refuses it first
  (OU-1). The host's sync floor refusal is Core A19-1 (accepted, staged for final38) and comes after Core pins final38.
  A9-1's floor was the `attached` frame, but `baseline_begin` and `modes` are larger (747 bytes each against 344 for
  `attached`, at default limits). A19 withdraws `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches`.
- The queue threshold (9B, OU-9, OU-3d; steward ruling R-45). Each queued frame is charged its stream-delimited size. The
  baseline goes whole into the route's empty queue at the bind. R-45 exempts the one baseline sequence in delivery (the
  attach frames through `live`, with their stream prefixes; the `screen` frame of `max_snapshot_bytes + 5` bytes
  included) from the `route_queue_bytes` threshold. Only the frames behind the sequence count: the output after `R`, the
  held suffix included. `attached` is ahead of the sequence, not behind it, so it does not count either. The PTY read
  budget is the free payload space under the threshold, so no output is added past it, and a route that is behind stops
  the PTY (OU-3d). A route over the threshold only because of its baseline is not "not progressing" (9B) and has no stall
  clock of its own; OU-3b(b) alone makes it `Stalled` (the stall clock is a later P4a PR). A resync starts a new sequence
  (at most one per route, because a resync drops the unstarted baseline frames) and sets its exempt bytes the same way.
- PTY output goes to each `Open` route unchanged and in order (OU-12). The worker splits it into `output` frames whose size
  is within the route's `max_frame_bytes`, as the codec measures a frame (`bound_of`).
- Client frames (`ToWorker`) go to the one admission point that host input already uses (AM-2, DP-4, DP-9). Input is
  fire-and-forget: only `input_refused` and the `input_done` of DP-5 are written back. `Observation::ClientInput{route,
  input_rev}` reports the admission to the host.
- **Report order (steward ruling on a5_2 s74/s75, R-88 group E).** The worker writes a client frame's `ClientInput` report
  on the control link before it gives any byte of that frame to the PTY. The worker reports each complete frame when it
  receives it, and the frame reaches the PTY only later, at the admission point. The link is FIFO, so every PTY output that
  the frame causes reaches the host after its report. A transcript can fence the report with that output. Proof:
  `worker/tests/routes/report_order.rs`.
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
- The `botster-worker` binary: `recvmsg` descriptors on the `mio` loop (PR B), then the PTY read budget and the route
  transport over the socket (PR C, lead ruling of 2026-10-10): `botster-worker/src/route_io.rs` holds the bound streams
  and their bounded reads and writes; `main.rs` registers each stream with the loop.
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
  fixes both with no new descriptor field.

### The route's paste (RC PR A; plan 23bp; steward ruling R-75)

- **One open paste per route** (Codec TS-5d), in `route.rs` `Route::paste`. A file has a separate slot (R-75; PR C). The
  receipt decides every paste frame (DP-5: the decision is made on receipt, never when the frame is applied):
  - `paste_begin` opens the paste, unless one is open (`refused`) or `total_bytes` is over `max_paste_bytes` (`too_large`,
    R-9). It sends no frame.
  - `paste_chunk` adds its bytes to the open paste of its op. The first chunk that takes the paste over `max_paste_bytes`
    is refused `too_large`, and the paste is discarded.
  - `paste_commit` takes the open paste of its op to the admission point as one route input; `paste_abort` discards it
    and sends no frame.
  - Each sequencing error (R-5: no open paste, or another op) is refused `refused` with zero effect, and the open paste
    stays as it is.
  - A closing route applies no input, so a paste that is open at the close writes nothing.
- **A paste is one client input, received at its commit** (steward ruling R-102). Only the commit advances
  `input_rev{client}`, reports `ClientInput` and makes the input record (`receipt.rs`). `paste_begin`, `paste_chunk` and
  `paste_abort`, applied or refused, do none of these: an aborted paste leaves no trace in the facts. A commit that is
  refused for a sequencing error is one client input too: its record has `kind: paste`, an empty payload and
  `not_written{refused}` at once. A file (DP-5b) follows the same rule at `file_commit`.
- **The open paste is outside the route's input bound.** `max_paste_bytes` bounds it. Its bytes count in the read
  allowance from the commit, when they wait at the admission point.
- **The commit is one transaction** (AM-2, TS-5d: contiguous). A route input carries an `InputPayload`. At its start, the
  admission point encodes it with `model::encode`, the encoder of the host's writes (IN-8: the markers come from
  libghostty, with the mode at the start; `require_bracketed` with the mode off is `mode_precondition_failed` with zero
  effect).
- **The outcome** (TS-5b): `input_done{op, written_bytes}` only for `want_result: true`; `failed` and `session_ended` with
  `written_bytes`. `written_bytes` is the PTY bytes after encoding, markers included (one unit everywhere); for `bytes`
  and `text` it is the same count as the payload's.
- **R-75:** a frame that breaks a 5.1A field rule (`Decoded::Invalid`, the rules of `Input::check`) is refused `refused`,
  never `invalid_input`.
- **Prior art:** the paste state of old Core `client_worker.rs` (`intake_terminal_command`: one assembling paste per route;
  a continuation must match its op; the commit takes the assembled bytes), with the `Stolen-From` and `For-Clause`
  trailers, and the three scenarios of its unit tests. Not taken: its strictly increasing op ids, its 1 MiB constant, its
  5 s assembly timeout, its chunk index and its paste-safety rejection (the contract has none of them). Compared, not
  used: the contracts' `botster-fake-route` paste slot.

### Changes outside worker-core (each is HIGH)

- `botster-core-host`: the outbound marks and `link_send_descriptor` in place of `handoff_route` (PR1), and
  `Input::HandoffSent` (PR3).
- `botster-core-testkit`: `send_with_descriptor`, the tagged `recv`, the worker binding, and the PTY read budget.
- `botster-core-link`: `HostMsg::AttachRoute` gets `limits: AppliedRouteLimits` (PR1, the correction above), and
  `route::terminal_format`, which names a snapshot format on the wire as `{name lowercase}/{version}+raw`.

## ReadFacts (ST-7; P5; steward rulings R-101, R-102, R-103, R-105)

- **The log** (`worker/facts.rs`, one per instance, made at the launch): the input records of client and host input in one
  order, the client tail, and the time of the last client input. Only the worker has them at one point of its ordering
  (AM-2), so `ReadFacts` is answered by the worker at once (`model.rs` `read_facts`).
- **A record at receipt.** A host write's record is made when its `WriteInput` comes (`input.rs` `on_write_input`). A
  client input's record is made in the same call that advances `input_rev{client}` (`route.rs` `client_input`). Its state
  is `pending`, and its `pty_bytes` is null.
- **What a client frame is** (`receipt.rs`, one function for each mapping, no catch-all):
  - one client input: `bytes`, `text`, a key, the mouse, focus, a query reply, and a paste's or a file's commit;
  - not input (no `rev` step, no `Activity`, no record, no `age_ms` reset): a paste's or a file's begin, chunks and abort
    (R-102), a resize (R-105 Q4; it is still route activity for DP-6 `Latest`), and a frame that does not parse, which
    closes the route `BadFrame` (R-103 (1));
  - the record keeps what was sent (R-103 (1)): a key's, the mouse's or focus's 5.1A fields, read from the frame's body
    with the contract's types; a query reply as `kind: bytes` with the decoded bytes of an `encoded` reply, else the JSON of
    its `reply` as received (R-105 Q2); a frame of an unknown kind as
    `kind: bytes` with its body as received. Text that is not UTF-8 is `kind: text` with no payload;
  - a refused input's record ends at once with its IN-2 reason, not the wire reason (R-101 Q2): `not_writable`, `not_ready`
    and `refused` are `Refused`; `too_large` is `PayloadTooLarge`; `unsupported{what}` is `Unsupported` with the variant of
    its cause when one exists (the codec's `key`, an unknown named key, is `named_key`), else `Other` (R-103 (2), R-105
    Q3); a
    refusal at the admission point keeps `SessionEnded` or `Stopping` apart.
- **Completion is an appended status update** with a new cursor (`input.rs` `end_record`); the record is never edited. The
  state comes from the IN-2 outcome (`record_end`): `Partial` and `Failed` are `partial` with the exact PTY bytes, 0
  included (R-101 Q1); `Unknown` is `unknown` with null PTY bytes. A kind that the worker cannot write is
  `NotWritten(Unsupported{what})` in the result and the record, never `Internal` (R-101 Q3). The end of a client input
  reports `Observation::ClientInputEnded{route}`, and the host raises the keyed `Activity{Client(route)}` edge again (EV-6).
- **Inputs that never start:** a host write that the adoption fence drops ends `not_written{refused}` with 0 PTY bytes:
  the fence refuses the old epoch's commands (DP-8), and no cancel was issued (R-103 (3)). A client
  input that waits in a closing route's `received` queue is discarded and ends `not_written` with 0 PTY bytes, never
  `cancelled` (R-105 Q1): its IN-2 reason (`SessionEnded` or `Stopping`) when the session's end closed the route, else
  `refused` (`receipt.rs` `discarded`). The session's end is `SessionEnded`, `SessionRemoved` and `SessionLost`: the worker
  never closes a route `SessionLost`, but a lost session is an ended session for IN-2 (lead, 2026-10-12). An input at the admission point ends at its turn, as before.
- **The cursor** is `ordinal << 29 | seq` (R-101 follow-up): the 24-bit ordinal of `LaunchSpec.cursor_ordinal` (the host's
  durable counter of the data directory) and the sequence number of the entry. An adoption keeps both, so a cursor read
  before it reads incrementally after it. A cursor of another ordinal is a `gap`. Each record that has not ended keeps one
  sequence number for its status update, so an end always has one. When no record can be made, a host write is
  `NotWritten(Refused)` with exact zero, and a client frame is refused `refused` with no `rev` step; `ReadFacts` still
  answers.
- **Retention:** at most `input_events_kept` entries, and at most `input_events_bytes` bytes of the list as JSON, the form
  in which the link carries it. The oldest entry goes first. The newest record and its status update are never dropped: a
  record that would not leave room for its status update is cut to the longest payload that does, then to its minimal
  form, with `payload_truncated`. `payload_truncated` means only that something was cut: a record with no truthful
  payload has none and is not `payload_truncated` (R-103).
- **Time:** `at` and `age_ms` are milliseconds on the worker's own clock, from the worker's start (the first `now` that it is
  given). Before any client input, `age_ms` counts from that start (R-101 Q4, Q5). They are not comparable with the unix
  `at` of events (TM-1).
- **The client tail** keeps the last `input_tail_bytes` PTY bytes that client input wrote, exactly; `tail_complete` turns
  false at the first byte that it drops.
- **Prior art:** old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` has no `ReadFacts`, no input records and no client tail
  (`git grep` for `input_records`, `InputRecord`, `ReadFacts` and `client_tail` finds nothing). No prior art applies.

