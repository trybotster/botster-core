# ClientWorker terminal egress and ingress

`ClientWorker` in `botster-core::engine::client_worker` owns every bound
terminal route: its binary egress queue, its input admission, and its
teardown. It runs on the host pump thread. There is no ClientWorker OS thread
and no periodic scan; the host advances it from `wait_wakes` and `pump_woken`.

## Route identity

Each live owner is keyed by `(session_id, subscription_id)`. On attach the
worker allocates a generation from one shared monotonic allocator and
validates the subscription id as a scheme 2 `RouteId`. Attach fails
explicitly when the id is not a valid route or the allocator is exhausted;
no owner is created in either case.

The generation is fixed for the life of the attachment. The route also keeps
a `stream_epoch` that starts at 0 and advances only through overflow resync.

## Egress

Session-wide frames are encoded once and shared:

- `push_session_output(session, bytes)` encodes one `OUTPUT` body and
  enqueues the same `Arc` on every receiving route of the session.
- `push_session_modes(session, modes)` shares one `MODES` body and records
  the session's latest modes for Core-originated results.
- `push_session_process_exit(session, code)` rejects unsent input as
  `session_ended`, then shares one `PROCESS_EXIT` body with every route,
  including routes still awaiting a capture.

Route-personal frames go through `push_route_frame`: attach state, snapshot
pages, `HISTORY_UNAVAILABLE`, `INPUT_RESULT`, `ROUTE_RESYNC`.

A receiving route has a bound adapter or a pre-bind hold declared through
`expect_terminal_adapter`. Unbound routes are served by the multiplexer
drain path; `filter_bound_terminal_frames` removes bound-route frames from
that path so no byte is delivered twice.

A route that awaits its capture does not receive live output: those bytes
are inside the snapshot the worker is producing at the same barrier.
`SNAPSHOT_READY` ends the wait.

Each queued frame carries the epoch captured at enqueue. The pump writes
`RoutedTerminalFrame { route, generation, stream_epoch, frame }` to the
adapter one frame at a time and never re-stamps a queued frame.

## Overflow and resync

The per-route queue is bounded to 64 frames or 4 MiB. On overflow:

1. The in-flight head stays; the adapter already holds it.
2. Unsent `OUTPUT`, `MODES`, and snapshot frames are dropped.
3. The route enters `to_epoch = from_epoch + 1` and queues
   `ROUTE_RESYNC { from_epoch, to_epoch }` under `to_epoch`.
4. Unsent `INPUT_RESULT` frames are preserved in order behind the
   transition and re-stamped with `to_epoch`.
5. The route awaits a fresh capture; the engine takes the request through
   `take_resync_requests` and starts a worker capture for that route only.

Epoch exhaustion ends the route with `ATTACH_STATE failed`. A route whose
terminal frame (`PROCESS_EXIT` or `ATTACH_STATE failed`) was delivered
hard-stops on the next pump.

## Ingress (Stage A)

`intake_woken` reads at most 64 frames per named route. Each frame is a
`TerminalInputFrame`; decode failure or `Lost` hard-stops the route.

Admission per command:

- Operation ids must increase strictly from 1 per attachment. A repeated or
  smaller id yields `rejected_protocol`. Paste chunk, commit, and abort repeat
  the active paste id.
- Lanes: 32 operations or 2 MiB retained per session, 128 operations or
  8 MiB per client, counted from admission until the result is delivered. A
  full lane yields `rejected_lane_full`.
- Paste: one assembling paste per route, 1..=1 MiB total, chunk sizes exact,
  5 s assembly timeout. Commit produces one worker operation whose body is
  `[u8 allow_unsafe][data]`. Abort of an assembling or queued paste yields
  `cancelled`; abort of an in-flight paste asks the worker to cancel.

Admitted operations keep their client body bytes; Core never re-encodes
them. The worker encodes key, mouse, focus, and paste bodies against its
terminal modes.

## Stage B and results

`take_one_terminal_input` moves one admitted operation to the in-flight map
under a Core-unique 64-bit key and returns a `StagedTerminalInput`. The
managed runtime sends it as `FRAME_INPUT_OPERATION` and, on a full or sealed
control lane, resolves it at once (`rejected_lane_full`,
`rejected_not_writable`).

`complete_operation(key, result)` releases the lane and queues
`INPUT_RESULT` on the route. Results for routes that were torn down are
dropped; the client already resolved them as unknown on close.
`fail_in_flight_for_session` resolves every in-flight operation with one
outcome when the worker link ends (`outcome_unknown`).

A teardown reports `in_flight_keys` so the host cancels them at the worker.

## Wakes

`take_bound_queue_wake_sessions` returns sessions whose Ready-owner queue
grew since the last take. Hosts turn that into one coalesced session wake so
the next pump writes without a scan. Paste deadlines and pending resize
deadlines clamp the host wait through `next_paste_deadline` and
`expired_paste_routes`.
