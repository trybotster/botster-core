# Core daemon

`botster-core-daemon` is the production supervisor over policy-free Core
primitives. It owns the durable session registry, session supervision and
adoption, the retention of ended-session terminal history, and a typed host
API. Session workers own PTYs and the only terminal parser. Hubs own auth,
product policy, copy, cloud, and UI decisions.

`CoreDaemon` is not transferable between threads. A host that needs a data
plane thread constructs and keeps the daemon on that thread and drives it
through `wait_wakes` / `wait_pump` and `pump_woken`.

## Engines

| Config | Engine | Terminal authority |
| --- | --- | --- |
| `worker_path` set | `WorkerBackedBotsterEngine` | Worker Ghostty per session; no parent shadow |
| `worker_path` unset | `DefaultBotsterEngine` with the Ghostty backend | In-process Ghostty per session |

Restart-durable adoption requires the worker path.

## Pending operations

Every call that touches a worker, the filesystem, or a slow path is a
pending operation. Nothing on the shared pump waits.

```rust
let id = daemon.begin(CoreOperation::ReadScreen(request))?;
// later, after pump_woken:
for completion in daemon.take_completions() { /* match on id */ }
```

`CoreOperation` variants and their completions:

| Operation | Completes when | Completion payload |
| --- | --- | --- |
| `Spawn` | Worker handshake finished on the launch thread; registry row saved | `CoreSession` |
| `Adopt` | Immediately (socket connect and handshake are bounded) | `CoreSession` |
| `ShutdownSession` | Session exited, or the shutdown deadline passed | `()` |
| `RemoveSession` | Immediately | `bool` removed |
| `ReleaseEndedSession` | Immediately | `bool` released |
| `ReadScreen` | Worker `FRAME_SCREEN` reply for the probe id, or retained state | `ScreenReadback` |
| `ReadModeFlags` | Worker `FRAME_MODE_FLAGS` reply, or retained state | `ModeFlagsReadback` |
| `CaptureSnapshot` | Worker capture finished, or retained state | `SnapshotCapture` |
| `Resize` | Registry size matches the acknowledged worker size | `()` |
| `CancelInput` | Immediately; the route receives the worker `INPUT_RESULT` | `bool` was in flight |

Limits enforced by `begin`: 4 pending spawns per daemon, 8 pending readbacks
per session, 4 open captures per owner. Exceeding one returns
`CoreDaemonError::PendingLimit`.

Deadlines: readbacks and captures use `worker_reply_timeout` (default 5 s);
shutdown uses 2 s. An expired deadline completes with `DeadlineExpired`. A
worker link failure completes with `WorkerLinkFailed`. `cancel(id)` completes
the operation with `Cancelled`; work already sent to the worker is not
undone.

On the local engine, readbacks, captures, spawns, and resizes complete
before `begin` returns.

## Snapshot captures

`CaptureSnapshot` returns a `SnapshotCapture` with a `capture_id`, the total
bytes, the 256 KiB page size, the page count, the size, and the Ghostty
colors frozen with the snapshot. `read_snapshot_page(capture_id, page)`
returns a `SnapshotPage` view over the shared buffer; nothing is copied.
`release_capture` frees it; `release_owner_captures` frees every capture of
one owner on disconnect. Captures expire after 60 s without a page read.

A live capture is a worker snapshot barrier queued with route attach
captures for that session, so it never interleaves with an attach capture.

## Retention of ended sessions

When a session exits the daemon keeps its final terminal state as a
`RetainedTerminal`: screen text, GHOSTSNP bytes, mode bits, size, colors,
exit time, and the accounted byte size. Worker-backed sessions take it from
`FRAME_FINAL_STATE`; local sessions capture it from the in-process
backend.

`RetentionPolicy { max_object_bytes, max_total_bytes, max_sessions }` is
host-supplied (`with_retention_policy`). The default is 16 MiB per object,
256 MiB total, 256 sessions.

- An object larger than `max_object_bytes` is refused and the session
  reads back as `history_unavailable: oversize`.
- When totals exceed the policy the oldest retained sessions are evicted
  and read back as `evicted`.
- A session that ended before this daemon incarnation reads back as
  `restart`; retention is in-memory only.
- A worker that ended without a final state reads back as
  `capture_failed`.

`retention_accounting()` reports totals, evictions, and oversize refusals.
`remove_session` forgets the retained object.

`release_ended_session` restarts an ended session in place. Its
preconditions match `remove_session`: the registry row is `Exited` or
`Stale`, and the engine session is absent or terminal; otherwise it returns
`false`. It forgets the engine side (the session, the retained object,
commit bookkeeping, pending drains) and releases the admission entry the id
holds, including an explicit reservation from an earlier `ReserveSession`,
through the engine that issued it. It keeps the registry row and the
session's routed-envelope targets, and journals nothing. `true` means the id
can be reserved now; `false` after the preconditions held means the entry is
retained while the previous launch's cleanup is unconfirmed, and a later call
retries. The host then reserves and spawns the same id as usual: the row goes
from ended to `Running` in one `Upsert`, with no `Removed`. Attach and
admission generations are engine-wide, so the new run's are fresh. A failed
spawn leaves the row ended. The sequence repeats for every later run.

## Attach, bind, and pump

`attach(client, session, subscription, now)` records the route and starts a
worker capture. For a route declared with `expect_terminal_adapter`, the
attach response carries no terminal frames; the host binds a waking adapter
with `bind_waking_terminal_adapter` and the next targeted pump delivers
`ATTACH_STATE attached`, `MODES`, the snapshot, and live output as scheme 2
frames. Unbound routes receive the harness frames through `drain`.

`pump_woken(batch, now)` drains only the sessions and routes named by the
wake batch, reconciles worker replies for pending operations, persists
acknowledged sizes, commits lifecycle observations, and retains final
terminal state for sessions that exited. It never scans unnamed sessions.

Resync after a route overflow is engine-internal: the route receives
`ROUTE_RESYNC` and a fresh capture; the daemon observes nothing new.

## Lifecycle control plane

Registry rows, the ordered lifecycle journal, `lifecycle_baseline_page`,
`lifecycle_changes_page`, `observe_lifecycle_slice`,
`observe_session_lifecycle`, and `session_registry_state` are unchanged in
shape: they carry no terminal bytes and are independent of registry size
where documented.

## Registry format

Records are one JSON file per session under the data directory with a
fixed-length digest filename. Files in any other format are rejected with
`UnsupportedFormat` without migration or scans; there is no legacy filename
probe.

## Host-facing helpers still synchronous

`spawn`, `adopt_session`, `remove_session`, `input`, `resize`, `drain`,
`detach`, and `shutdown` remain for the operator CLI and simple embedders.
Production hosts on the shared pump use `begin` for everything that can
block on a worker.
