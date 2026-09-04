# Control-plane lifecycle journal

Living contract for the Core control-plane lifecycle journal. Hosts
advance session lifecycle without a terminal client or Hub terminal
Drain. Historical plan material lives in
`docs/archive/plans/bound-observe-lifecycle-and-baseline.md`.

Hub ticket `ticket_1786663582_169720` consumes this surface. Hub
projection, host retention, and plugin policy stay in Hub.

## Observe

`CoreDaemon::observe_lifecycle_slice(now_seconds, resume, budget)` is
the production progress tick.

- `resume = None` mints a pass id over the ordered live-session index.
  The pass records a generation watermark and a final `SessionId`.
- `resume = Some(cursor)` continues only when `cursor.pass_id` and
  `cursor.last_visited` both match that open snapshot. Otherwise the
  result is `resync_required = ObservePassUnavailable`, `complete =
  false`, and no suffix.
- Later slices walk only the unvisited ordered suffix. They do not list
  or sort the full live set. Generation tags exclude sessions that
  appear after mint. Those sessions wait for a new pass.
- `max_sessions`, `max_encoded_result_bytes`, and `max_elapsed` each
  stop before remaining pass ids are visited. Elapsed starts at API
  entry and includes pass setup. `max_elapsed` is a host-tick yield
  bound, not session policy. A setup-only yield returns
  `last_visited = None`. The caller resumes with that exact cursor. Do
  not use `now_seconds` as the elapsed clock.
- Byte admission uses a reserved 256-`x` public error before each
  visit. `observe_session` mutates the journal and pending drain and
  cannot be rolled back. Public slice messages are sanitized to
  `A-Za-z0-9` space `. : _ / + - ?` and truncated to 256 bytes.
- Core checks the exact encoded size of every successful slice. This
  check includes empty, zero-item, and elapsed-yield slices. An
  undersized resumed call keeps the pass open for a later retry.
- Per-session drain errors stay on the typed internal outcome and on
  the sanitized slice DTO. A later id in the same slice still runs.
- The walk does not call `drain_runtime_all_once`. Incidental terminal
  egress stays on the pending-drain path. The slice returns no terminal
  bytes, phases, snapshots, attach state, or `ProcessExited` frames.
- Observe never `try_write`s a bound adapter. When it queues frames onto
  a bound Ready owner, it emits one coalesced session ingress wake. The
  host delivers those frames through `wait_wakes` and `pump_woken`.
- Observe still commits registry state and journal rows immediately. A
  session with no live owner that still holds undelivered frames keeps
  its session wake until a later commit after delivery, hard-stop, or
  owner removal.

`observe_lifecycle(now_seconds)` is the unbounded compatibility
wrapper: new pass, unbounded item/byte/elapsed budgets, typed
`CoreDaemonError` values. It does not reconstruct those errors from
slice strings.

## Exact-session observe

`CoreDaemon::observe_session_lifecycle(session_id, now_seconds)` is
the host exact-session query. It returns
`Result<SessionLifecycleLookup, CoreDaemonError>`.
`SessionLifecycleLookup` is `#[non_exhaustive]` with `Found` and
`Absent`.

- The call visits one `SessionId`. It does not walk
  `observe_lifecycle`, `lifecycle_baseline`, or `load_all`.
- When the engine or the live observe index has the id, the call
  runs `observe_session` first. That drain can reconcile parked
  `ProcessExited` on this session only.
- `Ok(Found)` returns the reconciled registry-backed row.
- `Ok(Absent)` means both the registry and the engine lack the
  session after that observe attempt. Absence is not
  `UnknownSession`.
- Drain failure, registry I/O, malformed JSON, and shutdown return
  `Err`. Hosts must not map `Err` or `Absent` to Active.
- The result has no terminal bytes, phases, snapshots, attach
  state, or `ProcessExited` frames. Incidental terminal egress
  stays on the pending-drain path.
- The query does not `try_write` a bound adapter. When it queues
  frames onto a bound Ready owner, it emits one coalesced session
  ingress wake.

Hosts classify one session through this query. They must not
classify shutdown with `CoreDaemon::drain`, `lifecycle_baseline`,
or capped pagination. The owner-loop walk remains
`observe_lifecycle_slice` plus `lifecycle_baseline_page`.

## Baseline

`lifecycle_baseline_page(snapshot, after, budget)` pages a frozen
registry snapshot. `LifecycleBaselineBudget` supplies `max_rows`,
`max_bytes`, and `max_elapsed`. Elapsed starts at API entry.

- `snapshot = None` mints at the current journal watermark and walks
  the registry directory under the call budget. It does not
  `load_all()` or sort the remaining name set. One freeze is cached. A
  new mint replaces it.
- Later pages at that snapshot continue the same directory iterator
  and then walk only the next frozen suffix. They do not re-read a
  mutated registry. Observe between pages does not change already
  decided freeze rows.
- Setup-only and index-in-progress yields keep the freeze identity,
  return no rows, set `next = None`, and have `complete = false`.
- `after` is inclusive of `next` after the index is complete.
  `complete` is true only on the page that includes the last frozen
  row, or on an empty sealed snapshot. An incomplete page is not
  finished ended evidence.
- A dropped or foreign freeze returns `SnapshotUnavailable` or
  `SourceChanged` with `complete = false` and no rows.
- A complete page drops the freeze.

`lifecycle_baseline()` remains the unbounded one-shot reader. Hub
Stage A must not call it.

## Wake and journal pages

`take_journal_advanced_wake` and `lifecycle_changes_page` are unchanged
except that a paged baseline snapshot identity is the journal watermark
at mint. Safe consume order is take, page until caught up or resync,
take, re-page if woke.

Resync outcomes are control results and are not required to satisfy a
byte budget. `SessionLifecyclePageError` stays `#[non_exhaustive]` with
`BudgetTooSmall`.

## Production path

1. One `observe_lifecycle_slice` per host owner turn, until `complete`
   or a budget yield. The caller stores the resume cursor.
2. `take_journal_advanced_wake`.
3. `lifecycle_baseline_page` until `complete` when installing or
   resyncing a projection.
4. `lifecycle_changes_page` after the snapshot watermark.
5. Take again; re-page if woke.
6. Exact-session host classification calls
   `observe_session_lifecycle` for one `SessionId`. That call is
   not a page walk.

Zero-client natural exit uses observe plus a bounded journal page. No
`CoreDaemon::drain`. No terminal client. A parked process exit
becomes `Found` with `Exited` on the first exact-session query after
the child exits.

## Runtime-teardown lenses

This surface is runtime-teardown class. Shipped answers:

- Isolation: one session observe updates that session's journal row and
  may hard-stop only that session's terminal subscriptions. A retained
  drain error does not stop later snapshot ids.
  `observe_session_lifecycle` visits only the requested `SessionId`.
- Bounds: one slice is one non-blocking host tick. No `block_on(close)`
  and no `drain_runtime_all_once`. An exact-session query calls
  `drain_runtime_once` for that id only. ClientWorker hard-stop stays
  synchronous close+drop on the tick that observes `ProcessExited` for
  a visited bound subscription.
- Late messages: stale or forged observe cursors resync with
  `complete = false`. Unknown baseline snapshots resync. Spawn, Attach,
  Bind, Drain, Input, `remove_session`, and `lifecycle_changes_page`
  are unchanged.
- Production proof: sliced observe plus `lifecycle_changes_page` shows
  `Exited` with zero attaches and no `CoreDaemon::drain`.
- Ownership: observe identity is `(pass_id, optional last_visited)` plus
  the generation watermark and final key. Journal identity is
  `(source_id, sequence)`.
  Terminal identity remains `(session_id, subscription_id, generation)`.
- Siblings: a retained observe error does not fail the daemon or
  unvisited siblings. An incomplete slice is not sibling sacrifice.
