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
| The registry key of a session is `session/<id>`. The real `Storage` keeps a row at `rows/<kind>/<components>.row`: the id in lowercase base32 with no padding, cut into components of at most 200 characters (all but the last are directories). Every step uses `openat`/`mkdirat` on directory descriptors. A path that does not decode is foreign: counted in `diagnostics()`, left untouched, never a row. | Lead ruling on audit A1: every file that Core writes names its own key, so a damaged row is still `Lost(RegistryCorrupt)` under its id (AD-1, AD-2, A10-2), and neither `NAME_MAX` nor `PATH_MAX` limits an id (`max_session_id_bytes` has no upper bound). A confinement that checks each file operation against its whole path (AppArmor) refuses paths above about 8 KiB: there, `Create` of an id whose path is longer fails with `RegistryFailed` and leaves nothing behind (a failed write removes the directories it made). Lead ruling on #164 (2026-10-08); a ceiling of `max_session_id_bytes` is an open steward question. |
| `open` creates only the data directory (a non-recursive `mkdir`); a missing parent fails `open` with `RegistryFailed`. Every `open` syncs the data directory and its parent, and no other ancestor; a parent that cannot be opened for the sync fails `open` with `RegistryFailed`. Both requirements are in the rustdoc of `DataDir::open`. | Lead ruling on integration finding K4. AD-7: the entry of the data directory, and the entry of `rows` in it, are durable before `open` succeeds, also on a retry after a failed open, because the syncs run on every open. The other ancestors are the host's to provide and to make durable, so an ancestor that the host may only pass through (`0711`, `0100`) never fails `open`, and `open` never claims a durability that it does not have. |
| A write to a worker link uses `write(2)`, which raises `SIGPIPE` when the worker is gone. Core sets neither `SO_NOSIGPIPE` nor `MSG_NOSIGNAL` yet. | The Rust runtime sets `SIGPIPE` to ignored when a Rust program starts, so in that state the write returns `EPIPE` and the link closes. A host that restores the default disposition (a Rust host can do so, and a C embedder through the C ABI of section 13 can start with it) is killed by `SIGPIPE` when it writes to a gone worker. The fix (`SO_NOSIGPIPE` on macOS, `MSG_NOSIGNAL` on Linux) and its real-process proof are audit A49, which waits in the follow-up PR for the `botster-test-process` crate (plan r22). |
| `diagnostics()` keeps the reasons of the last 16 link closes, the count of failed final-row writes, and the edges' accept failures. | LC-10 allows one opaque value; a failure that a completion cannot carry is visible there (audit A28, A48). |

## Adoption (P5): design for review (2026-10-09)

Draft. No code yet. The ids are the 55 of `docs/stage1-clauses/p5-adoption.txt` (branch `stage1/plan`), at
`contracts-v0.1.13`. An id leaves `conformance/core-pending.txt` only when it passes on both harnesses (plan 5), so the
ids stay pending until the real harness of P6 (`botster-test-process`, #171) can run them.

### Shape: the worker is the server, the new host is the client

- Each worker outlives its host (LC-12) and owns one session. During its whole life it listens on its own **worker
  endpoint**. A new host that adopts the session connects to that endpoint.
- The direction follows from the epoch rule. The hello proof binds the host epoch (DP-8). A worker cannot prove itself to
  a host whose epoch it has never seen, so the host must speak first. The contract names the endpoint: "worker or guardian
  endpoints are readable and connectable only by the host's uid" (AD-6).
- The old daemon used the same direction (`adopt_reserved_inner` at `72b2e33`: the host connects to a reconnectable control
  socket that the worker listens on). See the Prior art note below.

### 1. The worker endpoint (filesystem; P5, with the worker driver of P3)

- **Path:** `<data_dir>/w/<InstanceId>`. `open` creates `w` with mode `0700` (like the registry directory) and refuses it
  when it is not a directory that the host's uid owns with no group or other bits (AD-6 "`open` refuses an unsafe
  directory"; tmux's `make_label` check, with `lstat`).
- **Length:** a Unix socket path holds about 104 bytes. `open` already refuses a `data_dir` whose control socket cannot be
  bound (A47). It also refuses a `data_dir` whose longest endpoint cannot be bound. The longest `InstanceId` is
  `"<u64>-<u64>"`, 41 bytes.
- **Launch:** the host passes the path to the worker with a new launch argument, `--endpoint`. The worker binds it before
  its first hello, so a host can adopt it from the moment its row records its identity (AD-7 step 3).
- **A missing endpoint** (removed by a cleaner while the worker lives): the connect fails, and the row is
  `Lost(WorkerUnreachable)`. Core never binds or spawns in its place (AD-2: "a `Lost` session is never restarted in place").
  `Adopt(id)` may be retried.

### 2. The proof gets a direction (AD-6; changes the start handshake too)

- Today one function gives both proofs: the worker's hello carries `token_proof(token, instance, epoch)`, and the host
  answers with the same value. In an adoption the host speaks first. An impostor at the worker endpoint could then send
  the host's own proof back, and the host would accept it.
- **Change:** the proof hashes a role byte after the domain: `host` or `worker`. Each side checks the role of the other.
  Neither side can then replay what it received. The start handshake uses the same two roles, so there is one rule for
  both handshakes.
- No released worker exists (protocol 1 is not released), so the change breaks no adoptable worker (AD-4).
- Owner: `botster-core-link` (the proof) and both machines. Cross-package: the integration reviewer reviews it.

### 3. The adopt handshake (AD-6, DP-8, AD-4, A10, A11)

1. The host connects to the endpoint (a new host edge, `connect_worker`; see 6).
2. The host sends `Hello{protocol: T, instance, proof: host(token, instance, E), host_epoch: E}`, where E is the new
   host's epoch.
3. The worker checks the instance, the host proof, and that E is **above every epoch it has seen**. On a failure it closes
   **this connection only**: its current link, its payload and its highest epoch do not change (A11).
4. On success the worker records E, closes its old link (the fence: DP-8 "adoption fences the previous host"), and answers
   `Hello{protocol: P, instance, proof: worker(token, instance, E), host_epoch: E}`, then its adoption report (4).
5. The host checks the instance and the worker proof. A failure: Core never signals that process and decodes no later
   frame of the link (A10-1, A11-1; tmux's `PEER_BAD`). The row is `Lost(WorkerUnreachable)`, because a live worker may
   still be at the identity (AD-2: indeterminate; `Adopt(id)` may be retried).
6. The host checks P: P = T or P = T - 1 adopts; any other P is `Lost(WorkerVersion)` (AD-4, A6-2). P is recorded on the
   session (LC-9), also on `Lost(WorkerVersion)`.
7. **Deadline:** if the connect, the hello or the report does not complete within `CoreLimits.startup`, the row is
   `Lost(WorkerUnreachable)`, and the worker protocol stays absent when no hello was read (LC-9;
   `conf::a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone`). This is the startup deadline of the start,
   reused: one reachability deadline. (Proposed; recorded here for review.)

### 4. The adoption report (AD-1, AD-3, ST-5, DP-12)

After its hello the worker sends one `Adopted` message with its live state, never values remembered from the spawn (the
old daemon's lesson; vault: evidence comes from protocol primitives, not defaults):
- the payload: running, or exited with its code and signal; its identity (A52 below);
- the terminal state that the host serves (size, modes, title, cwd, the reads of ST-5) and the current focus (DP-12: one
  `FocusChanged` at adoption, with the current value);
- its features (AD-4: `worker_features` of an N - 1 worker);
- its routes (DP-8 `RouteAdopted`): P4a; until then the worker reports none, and the route ids stay pending with that reason.

### 5. AdoptAll per row (AD-1, AD-2, AD-6)

Each row is decoded and checked before any connect (vault: "validate before the first change of state"). Then:

| Row | Result |
|---|---|
| does not decode (A10-2) | `Lost(RegistryCorrupt)` (done) |
| `Created` | `Created` (done) |
| `Starting` with no worker identity | `Lost(StartInterrupted)` |
| any other, identity `Absent` or `Reused` (A9 `ProbeIdentity`) | `Lost(WorkerGone)`. Core never signals it (AD-6 "reused pid is never killed"). |
| any other, identity `Matches` | the handshake of 3, then by the report: `Running`, or `Exited` when the payload ended; a `Stopping` row is adopted `Stopping`, the stop is sent again, and `stop_grace` counts from the adoption |

- Each row posts one `SessionState` (LC-11, EV-5); `Completed{AdoptAll}` follows the last one. Rows whose handshakes are
  in flight do not block each other; a row posts when its handshake ends.
- `Adopt(id)`: the same row path for one `Lost(WorkerUnreachable | WorkerVersion)` session (AD-2 retry).
- AD-5 is LC-2: a live host holds the data-dir lock, so a second host cannot open.
- ID-2: the session keeps its `InstanceId` from the row; operations of the old host do not survive (ID-2).

### 6. Edges and the testkit (cross-package: P6)

- Host edge `connect_worker(endpoint) -> Option<LinkId>`: real, a non-blocking `mio` connect, registered like an accepted
  link; testkit, an in-memory endpoint of the `Sim`.
- The `Sim` needs: worker endpoints by path, kept across a host drop (the workers stay already, plan 4.1); `connect_worker`;
  and one process table for all hosts of a harness, so that an identity probe of the new host sees the old host's workers.

### 7. The worker side (cross-package: P3's machine and binary)

- The worker machine gets candidate links: a connection on the endpoint is a candidate until its hello passes 3.3. Only a
  passed candidate replaces the current link. Inputs and actions grow by a link id.
- The worker binary binds and polls the endpoint (`mio`), and passes accepted connections to the machine.
- A worker that has no payload and no host for `startup` exits by itself (AD-7,
  `conf::ad_7_crash_between_steps_leaves_no_unregistered_payload`). The worker does not have this rule yet (no `startup`
  deadline in `botster-worker-core` at `a0f78fe4`). It is P5's id, so P5 adds it to the worker, with P3's agreement.

### 8. Decisions recorded here

- **A52, `PayloadId.start_time`:** `Option<u64>`, `None` when the start time is unknown, and an unknown start time never
  matches an identity. A sentinel 0 is a value that looks valid (lead asked P5 to decide this).
- **The reachability deadline** is `startup` (3.7).
- **A missing endpoint is never repaired by Core** (1). Whether the worker binds its endpoint again when the path disappears
  is a worker choice for P3; the contract does not ask for it.

### Prior art

See the adoption Prior art note in `handoffs/p5-adoption.md` (moved here with the first P5 code PR): tmux, shpool, zellij,
abduco and the old daemon. In short: REUSE tmux's directory check and its "a failed check ends the peer" rule (A11);
REUSE the old daemon's lesson that the adoption state is the live state; REJECT exact-version adoption (AD-4) and zellij's
resurrection (Core never starts a replacement); the handshake and the endpoint are hand-rolled over `std`/`mio` sockets and
`botster-core-link`, because tmux, shpool and zellij are programs, not libraries.

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
- **Write bound.** `begin` computes a write's size once (`payload_size`): the payload bytes, or for `Key`, `Mouse` and `Focus` the worst case over every mode from libghostty's encoders, once for each repeat or notch (5.1A). The pending op keeps it as `held_bytes`. `PayloadTooLarge`, the lane bound (IN-5) and `Unknown` (IN-7) all use that one value.
- **SetNotificationPolicy retirement.** The registry path (`RegistryFailed`) is only for an op admitted in `Created` (`created_path`). Other states end `WorkerLinkFailed`.
- **Worker-control signal (F7, lead ruling).** The broken-link stop sends `GroupSignal::EndPayload` (`SIGUSR1`) to the verified worker process alone, at the stop and at `stop_grace`. The host never sends `SIGKILL` to the worker on this path, because a kill cannot run a handler and can leave the payload alive. P3 implements the handler; the interface (meaning, idempotence, report after link return or adoption) is in the `botster-core-link` crate docs. Conformance ids that need the worker side stay pending for P3.
- **Carried `Silent` first; failed handoffs.** The driver runs a due `Silent` before it reads any link input in a pump. A failed route handoff is queued and fed to the engine one per step, inside the budget.
