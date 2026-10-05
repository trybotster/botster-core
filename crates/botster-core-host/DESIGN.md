# botster-core-host: P1 design note

Package P1 (session registry and lifecycle) of Stage 1. Plan pin `555bc433`, contracts `contracts-v0.1.2` (manifest final14).

## Shape

- `HostEngine` is a sans-IO machine (plan 2.1). Time enters only through `pump(now)` (TM-1): edge results are fed with no new
  time (`HostEngine::input`), and before the first `pump` the engine has no time and no deadline. It reads no clock, draws no random number, starts no thread and touches no
  file. `io.rs` lists its inputs and actions. A registry write, a random value, a spawn, a link message: each is an action with
  a `Ticket`, and the driver answers with the input that carries the ticket.
- `HostDriver` (`driver.rs`) is the one driver of the real `Core` and of the testkit's core. It owns framing and the hello decode
  (`botster-core-link`), the order of a `pump` (the scheduler of plan 2.4), the wake flag (TM-6) and the bounded read of the
  links. It calls no OS function: every effect is a method of `HostEdges`, which has one real implementation (`botster-core`)
  and one testkit implementation (`botster-core-testkit`).
- A **flow** (`flow.rs`) is a machine of the session: create, start (AD-7), stop (LC-5) and remove (LC-7). An operation is a
  waiter on a flow. Every step of a flow changes the state of the session and posts at most one event, so "a state change and
  its event are one atomic step" (EV-5b) and `pump_events` (9B) is a bound that no step overshoots. A step that posts a
  mandatory event is not ready work while the queue has no room: it is parked, `PumpReport.more` stays false, and the poll that
  frees room makes it ready again (EV-5d, TM-6).
- The queue (`queue.rs`) has the four classes of 6.2: M never dropped (`Completed` has a reserved slot; the others share
  `mandatory_events`), K keyed and coalesced to the newer position (EV-1, EV-6), D bounded with drop-oldest and one marker per
  instance (EV-2), L the marker. `Released` retires the unpolled K, L and D events of its instance, with no marker (EV-5).

## Choices where the contract is silent (each is reviewable)

| Choice | Reason |
|---|---|
| `InstanceId` is `"<host epoch>-<counter>"`. | ID-1 asks for a Core-minted id that is unique across restarts. The epoch is strictly increasing (DP-8), so no random value is needed (plan 2.3a says to prefer counters). |
| A start that fails reaches `Exited{cause: Other}` with no code and no signal. | LC-4 says `Exited` or `Lost`. The payload never ran, so no `Lost` reason fits (`StartInterrupted` is for a crash). |
| `ExitCause` is decided by the host: `Killed` after the kill of `stop_grace`, else `HostStop` after a stop or a signal, else `Signal` for a signal, else `Normal`. | The host knows what it asked for (LC-5, LC-6, EV-4); the worker reports only code and signal. |
| A worker that dies while a session runs or stops gives `Lost(WorkerGone)`; a link that closes with the worker alive leaves the state and fails the pending ops. | AD-2 says `WorkerGone` is "no process with this identity". A broken link alone is not a state change (LC-5: the session still ends). |
| `Resize`, `SetSizePolicy` and `SetColorProfile` of a `Created` session change the stored request in memory only. | The A2-1 table gives them no `RegistryFailed`, so no durable write. `SetNotificationPolicy` of a `Created` session is durable (A3-1). |
| A signal number outside 1 to 31 is `Unsupported`. | A2-1 gives `Signal` the sync error `Unsupported` and does not say which values. |
| A host `Key` or `Mouse` write is bounded at `begin` by 64 bytes per event (times `repeat` or `notches`). | IN-9 asks for the worst case over every mode. The encoders are the worker's (P3), which must never produce a longer sequence. |
| `AdoptAll` keeps `Created` rows and posts every other decodable row as `Lost(Other)`. | AD-1 recovery of a live worker is the adoption package's (P5). `Other` says that Core cannot tell. No test asserts this placeholder. |
| A row that Core's decoder rejects (not a row, another `version`, or a row whose `id` is not the id of its key) is `Lost(RegistryCorrupt)` under the id of its key. Its session gets an `InstanceId` minted by this handle and a record of size 0 by 0 with no labels. | AD-1: every row is recovered, and one that cannot be is `Lost(reason)`; AD-2 and A10-2 name `RegistryCorrupt`. EV-9 needs an instance for its `SessionState`, and the row's own one cannot be read. A size of 0 is outside every valid size (A2-1), so nobody takes it for a real one. `Remove` claims no upload of it (`outcome_unknown`, A6-3). |
| A row of a session that this handle holds already (it made the row) keeps the session and its instance, and posts its current state. A row whose session this handle is still creating waits until the create has posted `Created`, then posts its own state, before `AdoptAll` completes. | LC-11: one `SessionState` for every row, posted by `AdoptAll` before its completion. |
| `HostDriver::open` reads the ids of the registry's rows; `Create` refuses them with `IdInUse` until `AdoptAll` turns them into sessions. | ID-1: an id is unique among registry rows; LC-3; AD-2: a `Lost` row keeps the id in use until `Remove`. A read failure makes `open` fail `RegistryFailed`. |
| `AttachWebRtc`, `Adopt` and the service rows are refused with `Unsupported` or `UnknownService`. | They belong to P4c, P5 and P7. |
| A stop sends its request and starts its grace before the state event is posted. `Remove` closes each bound route (with its `RouteClosed`) first; steps 2 to 5 wait for that, then run under pressure and only `Released` waits for room. | EV-5c, and steward ruling R-15: LC-7's order holds under a full queue. |
| The worker reports the payload's identity at launch, and a stop without a link sends `GroupSignal::EndPayload` (`SIGUSR1`) to the verified worker, at the stop and at `stop_grace`. After the stop the session is `Lost(WorkerUnreachable)`. | LC-5 and LC-6: the payload group ends, and the worker keeps its final model. The worker is the parent of the payload and ends its group. The host never signals a bare payload group (AD-6) and never kills the worker on this path. Core cannot learn the exit of a payload whose worker it cannot reach. |
| `Signal` is a request to the worker and completes on its confirmation. | A2-1: `Ok` is "after the signal was sent"; a link that fails before the confirmation gives `WorkerLinkFailed`. |
| `Remove` waits for the worker process to end before it deletes the row and frees the id. A worker that cannot be asked, and a worker that does not end within `stop_grace` after its teardown, has its identity checked (`Action::ProbeIdentity`, AD-6): a matching worker is killed and checked again each `stop_grace`; an absent one, or a pid that another process has now, is gone. | LC-7 step 3 before steps 4 and 5; A6-3: a stray worker is ended; AD-6: a process that does not match is never signalled. A worker that this handle did not spawn has no exit watch, so the check is how the host learns of its end. |
| Ops of an instance that is gone complete `SessionEnded` (a write: `NotWritten(SessionEnded)`); the ops admitted after a failed `Create` complete with its `RegistryFailed`. | AM-3: no op stays attached to a session that is gone; ID-1: no op reaches a later instance. |
| A `StopAll` leaves a target whose stop row cannot be written and completes. | LC-12 and A2-1: `StopAll` has no async error; a target that cannot be stopped is left. |
| Setters of a `Created` session are steps that run in a pump, and a start waits for the setters admitted before it. | OR-1: no progress in `begin`; AM-1: begin order. |
| The input lane of a session is released when the host polls the write's `Completed`. | AM-4, EV-5a. |
| `cancel` keeps the exact set of op ids of removed instances as ranges, for the life of the handle. | ID-1 and IN-6 give no window. |
| A frame that needs mandatory room is held unread on its link with the read interest off; the pump bounds events and bytes per link, a due deadline runs first, and sessions are visited round-robin. | EV-5b, plan 2.4, 2.5 rule 7, 9B. |
| A worker observation is held unread on its link while the session's start is not through (the engine does not accept it), and the driver retries a held frame each time it services the link, before the link's later frames. The host keeps no queue of observations. | OR-2 and EV-5: the events of the model follow `Running` and `Completed{Start}`; ST-4, EV-6: the worker's order is kept; EV-2 and plan 2.5 rule 7: what waits is bounded by one held frame and one read chunk (review findings F21, F22). |
| `RemoveReport` is built through its JSON form. | The type is `#[non_exhaustive]` and `contracts-v0.1.2` gives it no constructor. A constructor in the next tag removes the workaround. |
| The registry key of a session is `session/<id>`. The real `Storage` keeps a row at `rows/<kind>/<components>.row`: the id in lowercase base32 with no padding, cut into components of at most 200 characters (all but the last are directories). Every step uses `openat`/`mkdirat` on directory descriptors. A path that does not decode is foreign: counted in `diagnostics()`, left untouched, never a row. | Lead ruling on audit A1: every file that Core writes names its own key, so a damaged row is still `Lost(RegistryCorrupt)` under its id (AD-1, AD-2, A10-2), and any id fits whatever its length (`max_session_id_bytes` has no upper bound) with no `NAME_MAX` or `PATH_MAX` limit. |
| `open` creates only the data directory (a non-recursive `mkdir`); a missing parent fails `open` with `RegistryFailed`. Every `open` syncs the data directory and its parent, and no other ancestor; a parent that cannot be opened for the sync fails `open` with `RegistryFailed`. Both requirements are in the rustdoc of `DataDir::open`. | Lead ruling on integration finding K4. AD-7: the entry of the data directory, and the entry of `rows` in it, are durable before `open` succeeds, also on a retry after a failed open, because the syncs run on every open. The other ancestors are the host's to provide and to make durable, so an ancestor that the host may only pass through (`0711`, `0100`) never fails `open`, and `open` never claims a durability that it does not have. |
| A write to a worker link uses `write(2)`, which raises `SIGPIPE` when the worker is gone. Core sets neither `SO_NOSIGPIPE` nor `MSG_NOSIGNAL` yet. | Every Rust host ignores `SIGPIPE` (the Rust runtime sets it at start), so the write returns `EPIPE` and the link closes. Only a C embedder (section 13, the C ABI, built later) can have the default disposition. The C ABI package sets `SO_NOSIGPIPE` (macOS) or sends with `MSG_NOSIGNAL` (Linux) and proves it in a C host test (audit A49). No Rust test can tell the two apart without `unsafe` code, which the workspace forbids. |
| `diagnostics()` keeps the reasons of the last 16 link closes, the count of failed final-row writes, and the edges' accept failures. | LC-10 allows one opaque value; a failure that a completion cannot carry is visible there (audit A28, A48). |

## Prior art (BUILD.md rule 0)

- **Reused:** nothing from the old code was copied (no `Stolen-From` commit). The old exit watch (`process_exit.rs`) uses
  `libc` and `unsafe`; this workspace forbids unsafe code, so the real `Process` edge reaps each child on a reaper thread that
  blocks in `Child::wait` and wakes the host through the notifier (`Children::with_notify`). The kqueue and pidfd watch stays
  P3's steal.
- **Rejected:** `registry.rs` (no fsync, no lock, no token), `session_protocol.rs` (the old control frames). Lessons kept: one
  file per row; the length is checked before any allocation.
- **Libraries added:** `atomic-write-file` (atomic replace: it syncs the file, renames it and syncs the directory in `commit`, and its error does not say which step
  failed, so every commit error is `Uncertain`; our own directory sync follows for the case that the library skips it), `getrandom` (the OS CSPRNG), `mio` (the wake object; `polling`'s registration is `unsafe`), `libproc` (start
  time of a process on macOS), `sha2` (the token proof and the row file name).
- **Hand-rolled, with reasons:** the event queue and the engine (they are the contract); the token proof (a bound hash; a
  keyed MAC would need a second round trip on a local socket that only the host's uid can reach).

## Round 2 decisions

- **StopAll row (R-16).** The Stopping row of a `StopAll` target is best effort. When the write fails, the stop goes on and `StopAll` completes (LC-12). A plain `Stop` keeps `RegistryFailed`.
- **No payload-group signal (AD-6, lead ruling on F7).** The host signals only processes that it identifies by pid and start time: the worker. It never signals a bare payload group, because the group id can be reused by an unrelated group after the original group ends. With a broken link (LC-5), the host sends `EndPayload` to the verified worker (see Worker-control signal below). `Launched` still carries the payload identity, and the row records it.
- **Interface note for P3 (worker side).** On `EndPayload` (`SIGUSR1`) the worker ends its payload group and keeps the final model. The worker is the parent of the payload. It keeps the payload leader unreaped (`waitid` with `WNOWAIT`) until the group kill completes, so the group id cannot be reused (POSIX). The worker asks the payload to stop, and kills its group after `stop_grace`.
- **Ops that end with their instance (AM-3, IN-7).** Writes that were sent and not acknowledged complete `Unknown`. Writes never sent complete `NotWritten(SessionEnded)`. Resize, size policy and signal complete `SessionEnded`. Detach completes `Ok`. Start, Remove, metadata and notification policy complete with the failed Create, or `RegistryFailed`. Other ops complete `WorkerLinkFailed`.
- **Remove grace.** A kill is not an observed exit. At each `stop_grace` the host checks the worker's identity: it kills a matching worker again, and takes an absent one as gone.
- **Pump bounds.** Held frames, process exits and completions count against `pump_events`. A read takes at most what is left of `pump_bytes`.

## Round 3 decisions

- **One event per step (9B).** `complete` posts at once only when the current input has posted no event. Otherwise it sets `Next::Complete` and the op completes in a step of its own. `UpdateMetadata` posts `Completed`, then `MetadataChanged` in the next session step (`metadata_pending`). A local `Detach` posts `RouteClosed`, then `Completed` in the next step.
- **Deadlines (erratum 3, E3-1).** `Work::Deadline` is the earliest due effect without an event (capture expiry, the kills, the startup and remove grace). It runs whatever the budget. `Work::Silent` is the earliest due `Silent`, a step atomic with its event. It needs budget, and the driver carries it with `more` otherwise. `ready()` lists `Deadline`, then `Silent`, then other work. A step parked on mandatory room is absent from `ready()`, so it never blocks a runnable step. At the bound the driver lets only `Work::Deadline` run.
- **Write bound.** `held_bytes` is the one bound of encoded bytes for a write: it counts every repeat and notch. Admission accounting and `Unknown` both use it.
- **SetNotificationPolicy retirement.** The registry path (`RegistryFailed`) is only for an op admitted in `Created` (`created_path`). Other states end `WorkerLinkFailed`.
- **Worker-control signal (F7, lead ruling).** The broken-link stop sends `GroupSignal::EndPayload` (`SIGUSR1`) to the verified worker process alone, at the stop and at `stop_grace`. The host never sends `SIGKILL` to the worker on this path, because a kill cannot run a handler and can leave the payload alive. P3 implements the handler; the interface (meaning, idempotence, report after link return or adoption) is in the `botster-core-link` crate docs. Conformance ids that need the worker side stay pending for P3.
- **Carried `Silent` first; failed handoffs.** The driver runs a due `Silent` before it reads any link input in a pump. A failed route handoff is queued and fed to the engine one per step, inside the budget.
