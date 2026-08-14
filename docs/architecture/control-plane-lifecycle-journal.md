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

`observe_lifecycle(now_seconds)` is the unbounded compatibility
wrapper: new pass, unbounded item/byte/elapsed budgets, typed
`CoreDaemonError` values. It does not reconstruct those errors from
slice strings.

## Baseline

`lifecycle_baseline_page(snapshot, after, max_rows, max_bytes)` pages a
frozen registry snapshot.

- `snapshot = None` mints from `load_all()` and the current journal
  watermark. One freeze is cached. A new mint replaces it.
- Later pages at that snapshot return frozen rows. They do not re-read
  a mutated registry. Observe between pages does not change already
  minted rows.
- `after` is inclusive of `next`. `complete` is true only on the page
  that includes the last frozen row, or on an empty snapshot. An
  incomplete page is not finished ended evidence.
- A dropped or foreign freeze returns `SnapshotUnavailable` or
  `SourceChanged` with `complete = false` and no rows.
- A complete page drops the freeze.

`lifecycle_baseline()` remains the unbounded one-shot reader.

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

Zero-client natural exit uses observe plus a bounded journal page. No
`CoreDaemon::drain`. No terminal client.

## Runtime-teardown lenses

This surface is runtime-teardown class. Shipped answers:

- Isolation: one session observe updates that session's journal row and
  may hard-stop only that session's terminal subscriptions. A retained
  drain error does not stop later snapshot ids.
- Bounds: one slice is one non-blocking host tick. No `block_on(close)`
  and no `drain_runtime_all_once`. ClientWorker hard-stop stays
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
