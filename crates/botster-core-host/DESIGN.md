# botster-core-host: P1 design note

Package P1 (session registry and lifecycle) of Stage 1. Plan pin `555bc433`, contracts `contracts-v0.1.2` (manifest final14).

## Shape

- `HostEngine` is a sans-IO machine (plan 2.1). It reads no clock, draws no random number, starts no thread and touches no
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
| `AdoptAll` keeps `Created` rows and posts every other row as `Lost(Other)`. | AD-1 recovery of a live worker is the adoption package's (P5). `Other` says that Core cannot tell. |
| `AttachWebRtc`, `Adopt` and the service rows are refused with `Unsupported` or `UnknownService`. | They belong to P4c, P5 and P7. |
| A stop sends its request and starts its grace before the state event is posted. `Remove` closes each bound route (with its `RouteClosed`) first; steps 2 to 5 wait for that, then run under pressure and only `Released` waits for room. | EV-5c, and steward ruling R-15: LC-7's order holds under a full queue. |
| The worker reports the payload's identity at launch, and a stop without a link signals the payload's group, never the worker's. After the kill the session is `Lost(WorkerUnreachable)`. | LC-5 and LC-6: the payload group ends, the worker keeps its final model. Core cannot learn the exit of a payload whose worker it cannot reach, and the kill cannot be refused. |
| `Signal` is a request to the worker and completes on its confirmation. | A2-1: `Ok` is "after the signal was sent"; a link that fails before the confirmation gives `WorkerLinkFailed`. |
| `Remove` waits for the worker process to end (a worker that does not end within `stop_grace` after its teardown is killed) before it deletes the row and frees the id. | LC-7 step 3 before steps 4 and 5; A6-3: a stray worker is ended. |
| Ops of an instance that is gone complete `SessionEnded` (a write: `NotWritten(SessionEnded)`); the ops admitted after a failed `Create` complete with its `RegistryFailed`. | AM-3: no op stays attached to a session that is gone; ID-1: no op reaches a later instance. |
| A `StopAll` leaves a target whose stop row cannot be written and completes. | LC-12 and A2-1: `StopAll` has no async error; a target that cannot be stopped is left. |
| Setters of a `Created` session are steps that run in a pump, and a start waits for the setters admitted before it. | OR-1: no progress in `begin`; AM-1: begin order. |
| The input lane of a session is released when the host polls the write's `Completed`. | AM-4, EV-5a. |
| `cancel` keeps the exact set of op ids of removed instances as ranges, for the life of the handle. | ID-1 and IN-6 give no window. |
| A frame that needs mandatory room is held unread on its link with the read interest off; the pump bounds events and bytes per link, a due deadline runs first, and sessions are visited round-robin. | EV-5b, plan 2.4, 2.5 rule 7, 9B. |
| `RemoveReport` is built through its JSON form. | The type is `#[non_exhaustive]` and `contracts-v0.1.2` gives it no constructor. A constructor in the next tag removes the workaround. |
| The registry key of a session is `session/<id>`; the real `Storage` names the file by the SHA-256 of the key. | Any id is a valid key, and the file name stays short. |

## Prior art (BUILD.md rule 0)

- **Reused:** nothing from the old code was copied (no `Stolen-From` commit). The old exit watch (`process_exit.rs`) uses
  `libc` and `unsafe`; this workspace forbids unsafe code, so the real `Process` edge reaps its children with `Child::try_wait`
  and wakes on the link's EOF. The kqueue and pidfd watch stays P3's steal.
- **Rejected:** `registry.rs` (no fsync, no lock, no token), `session_protocol.rs` (the old control frames). Lessons kept: one
  file per row; the length is checked before any allocation.
- **Libraries added:** `atomic-write-file` (atomic replace: it syncs the file, renames it and syncs the directory in `commit`, and its error does not say which step
  failed, so every commit error is `Uncertain`; our own directory sync follows for the case that the library skips it), `getrandom` (the OS CSPRNG), `mio` (the wake object; `polling`'s registration is `unsafe`), `libproc` (start
  time of a process on macOS), `sha2` (the token proof and the row file name).
- **Hand-rolled, with reasons:** the event queue and the engine (they are the contract); the token proof (a bound hash; a
  keyed MAC would need a second round trip on a local socket that only the host's uid can reach).

## Round 2 decisions

- **StopAll row (R-16).** The Stopping row of a `StopAll` target is best effort. When the write fails, the stop goes on and `StopAll` completes (LC-12). A plain `Stop` keeps `RegistryFailed`.
- **No payload-group signal (AD-6, lead ruling on F7).** The host signals only processes that it identifies by pid and start time: the worker. It never signals a bare payload group, because the group id can be reused by an unrelated group after the original group ends. With a broken link (LC-5), the host sends `Term` to the verified worker, and `Kill` after `stop_grace`. `Launched` still carries the payload identity, and the row records it.
- **Interface note for P3 (worker side).** On `SIGTERM` the worker must end its payload group and keep the final model. The worker is the parent of the payload. It keeps the payload leader unreaped (`waitid` with `WNOWAIT`) until the group kill completes, so the group id cannot be reused (POSIX). The host-side `Kill` to the worker after `stop_grace` is the last resort and can leave the payload group alive.
- **Ops that end with their instance (AM-3, IN-7).** Writes that were sent and not acknowledged complete `Unknown`. Writes never sent complete `NotWritten(SessionEnded)`. Resize, size policy and signal complete `SessionEnded`. Detach completes `Ok`. Start, Remove, metadata and notification policy complete with the failed Create, or `RegistryFailed`. Other ops complete `WorkerLinkFailed`.
- **Remove grace.** A kill is not an observed exit. Remove waits for the exit input and repeats the kill each `stop_grace`.
- **Pump bounds.** Held frames, process exits and completions count against `pump_events`. A read takes at most what is left of `pump_bytes`.
