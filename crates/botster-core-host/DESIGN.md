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
| `AdoptAll` recovers each decodable row by its recorded state and, for a live worker, by the worker's report. | P1 posted every non-`Created` row as `Lost(Other)`. P5 replaced that placeholder ("Adoption (P5)" below; steward ruling R-35): Core never posts `Lost(Other)`. |
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
- **The startup limit** reaches the worker with a second new launch argument, `--startup-ms` (milliseconds, a `u64`). The
  worker needs it for its AD-7 self-exit and for the candidate deadline (part 7). `WorkerLaunch::parse` refuses a
  missing or malformed value, with a parse test (integration D2).
- **Removal of the endpoint** (integration D3; SV-9 names endpoints among what a remove releases):
  - the worker unlinks its endpoint when it ends (`Action::Exit`, and on `Terminate`);
  - the host unlinks `<data_dir>/w/<InstanceId>` after `Remove` has verified the worker's end (LC-7), if the path is still
    there (a worker that was killed could not unlink it). A failed unlink is recorded in `diagnostics()` and does not fail
    the `Remove`.
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
- **Rule (integration D4):** the five hello fields (`magic`, `protocol`, `instance`, `proof`, `host_epoch`) and the proof
  rule (the domain, the role byte, the order of the hashed fields) **never change between protocol numbers**. Otherwise a
  worker of protocol N - 1 could not answer a host of protocol N, and it would be `Lost(WorkerUnreachable)` instead of
  adopted or `Lost(WorkerVersion)` (AD-4). The `botster-core-link` docs state the rule, and a test pins the encoded hello
  and the proof of a fixed input.
- Owner: `botster-core-link` (the proof) and both machines. Cross-package: the integration reviewer reviews it.

### 3. The adopt handshake (AD-6, DP-8, AD-4, A10, A11)

1. The host connects to the endpoint (a new host edge, `connect_worker`; see 6).
2. The host sends `Hello{protocol: T, instance, proof: host(token, instance, E), host_epoch: E}`, where E is the new
   host's epoch.
3. The worker checks the instance, the host proof, and that E is **at least the highest epoch it has seen** (DP-8: the
   worker obeys the highest epoch). A lower E is refused. On a failure it closes **this connection only**: its current
   link, its payload and its highest epoch do not change (A11).
   - **An equal E is valid** (integration D1): a host that reached step 4 and then timed out or lost the link has the row
     `Lost(WorkerUnreachable)`, and its `Adopt(id)` retry (AD-2) comes with the same E. An equal E replaces the current link
     after the proof passes, with the fence of step 4. Only one host has epoch E (LC-2: the data-dir lock), so the fence
     still holds.
   - Test: the host abandons the handshake after step 4 (the worker accepted, and its answer was lost or late), then
     `Adopt(id)` on the same handle, with the same epoch, adopts (P5-F20). A lower epoch after it is still refused.
4. On success the worker records E, closes its old link (the fence: DP-8 "adoption fences the previous host"), and answers
   `Hello{protocol: P, instance, proof: worker(token, instance, E), host_epoch: E}`, then its adoption report (4).
   **The fence retires every request of the old host** (P3's review): request numbers are per link, so an old `Done{req}`
   on the new link would complete the new host's request of the same number.
   - Queued writes of the old host are dropped with no report: no host is there to hear it.
   - The PTY write in progress runs to its end (AM-2: its bytes are contiguous), and it reports nothing.
   - Bytes queued for the old link are cleared. They are never sent on the new link.
   - Test: a write in flight at the fence, and the new host's first request has the same number as the old one.
5. The host checks the instance and the worker proof. A failure: Core never signals that process and decodes no later
   frame of the link (A10-1, A11-1; tmux's `PEER_BAD`). The row is `Lost(WorkerGone)`.
   - The text: AD-6 "A process that does not match is never signalled: an unrelated process that reuses the pid is
     `WorkerGone`." A10-1 "the row's outcome is what AD-6 and AD-2 give. For example, a non-matching process at the
     recorded pid is `WorkerGone`." The four A10-1 and A11-1 transcripts expect `Lost(WorkerGone)`.
   - The residual case: a live worker behind an endpoint that another process answered becomes `WorkerGone`, and Core
     does not end it at `Remove`. AD-6 forbids a signal to a process that does not match, so Core could not end it in
     any case.
   - This reverses the first design (`Lost(WorkerUnreachable)`, "indeterminate"), which #176's review accepted. That
     reading was ours, not the contract's (lead ruling on #176a-2, 2026-10-09).
6. The host checks P against **the adoptable set that Core exposes** (`adoptable_worker_protocols()`: {T, T - 1}, with
   T - 1 only when it is at least 1; at T = 1 the set is {1}, A6-2). P in the set adopts; any other P is
   `Lost(WorkerVersion)` (AD-4, A6-2). One function decides both the exposed set and this check, so they cannot differ
   (P5-F21). The two A6-2 deferrals stay as they are. P is recorded on the session (LC-9), also on `Lost(WorkerVersion)`.
7. **Deadline:** if the connect, the hello or the report does not complete within `CoreLimits.startup`, the row is
   `Lost(WorkerUnreachable)`, and the worker protocol stays absent when no hello was read (LC-9;
   `conf::a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone`). This is the startup deadline of the start,
   reused: one reachability deadline. (Proposed; recorded here for review.)

### 4. The adoption report (AD-1, AD-3, ST-5, DP-12)

After its hello the worker sends one `Adopted` message with its live state, never values remembered from the spawn (the
old daemon's lesson; vault: evidence comes from protocol primitives, not defaults):
- the payload, one of five states (P5-F22):
  - `NotLaunched`: no `Launch` came (a crash between AD-7 steps 3 and 4);
  - `Spawning`: a `Launch` came and its spawn has not answered yet;
  - `Running`, with the payload identity (A52 below);
  - `Exited`, with its code and signal;
  - `LaunchFailed`: the spawn failed;
- the terminal state that the host serves (size, modes, title, cwd, the reads of ST-5) and the current focus (DP-12: one
  `FocusChanged` at adoption, with the current value);
- its features (AD-4: `worker_features` of an N - 1 worker);
- the input revisions (host and client) and the model revision (IN-10, ST-1), so that the new host can form a guard that
  the worker accepts (P3's review);
- its routes (DP-8 `RouteAdopted`): P4a; until then the worker reports none, and the route ids stay pending with that reason.

The report never carries `last_output_at` (Core Amendment 18, contracts-v0.1.21). The worker has no clock to stamp it
(TM-1), and A18-1 makes it the host's own observation: "After adoption, it is `None` until the adopting host observes
output." The silence of an adopted session follows A18-2: its idle start is the latest output that this host observed, or,
with none, the adoption point, which is "the `pump` in which Core posts that session's `SessionState` for its adoption".
`mark_adoption_point` records that pump's monotonic time and unix time as the idle start of a session with no output
under this host. Every path that posts an adoption's state calls it: `post_adoption` (`Running`, `Exited`), the start
flow of an adopted `Starting` row (`NotLaunched`, `Spawning`, also on an `Adopt(id)` retry) and the stop flow of an
adopted `Stopping` row. No clause limits the idle start to `Running` (review #207 A18-F2). A `Lost` adoption leaves no
worker and gets none. A retried adoption's point replaces an earlier adoption point, never an observed output's.
`Silent` is then due a threshold after it, with `since` that pump's unix time. E3-1 is unchanged.

### 5. AdoptAll per row (AD-1, AD-2, AD-6)

Each row is decoded and checked before any connect (vault: "validate before the first change of state"). Then:

| Row | Result |
|---|---|
| does not decode (A10-2) | `Lost(RegistryCorrupt)` (done) |
| `Created` | `Created` (done) |
| `Starting` with no worker identity | `Lost(StartInterrupted)` |
| any other, identity `Absent` or `Reused` (A9 `ProbeIdentity`) | `Lost(WorkerGone)`. Core never signals it (AD-6 "reused pid is never killed"). |
| any other, identity `Matches` | the handshake of 3, then by the report: `Running`, or `Exited` when the payload ended; a `Stopping` row is adopted `Stopping`, the stop is sent again, and `stop_grace` counts from the adoption. A report with no running payload follows the next table. |

**A worker with no running payload** (P5-F22). AD-7 makes this state possible: a crash after the identity is durable and
before the `Launch` leaves an authenticated worker with no payload. Core never invents `Running`, and it never sends a
second `Launch` for a launch that already happened.

| Row | Report | Result |
|---|---|---|
| `Starting` | `NotLaunched` | The adopting host does AD-7 step 4: the identity is durable, so it sends the `Launch` built from the row. `Launched` gives `Running`, and the row's one `SessionState` is posted then. `LaunchFailed` gives **the same outcome as a failed launch of an ordinary `Start`** (LC-4, the same path and cause; no special case). `AdoptAll` completes after this answer (LC-11), bounded by the worker's `startup`. |
| `Starting` | `Spawning` | No second `Launch`. The host waits for the worker's `Launched` or `LaunchFailed`, then applies the row above. |
| `Starting` | `LaunchFailed` | The ordinary failed-launch outcome, as above. |
| `Stopping` | `NotLaunched` or `LaunchFailed` | `Exited{cause: HostStop}`, with no code and no signal: nothing runs, and the stop needs nothing. No `Launch` is sent. |
| `Stopping` | `Spawning` | The stop is sent; the worker applies it to the spawn's result (it does this today for an early stop). |
| `Running` or `Exited` | `NotLaunched` or `Spawning` | `Lost(RegistryCorrupt)`: the worker is authenticated (AD-6), so the durable row is the wrong record, which is a corrupt registry record although its bytes decode (R-35 correction, contracts `main` `c3ed727`). Defensive: it cannot happen under AD-7 (those rows are written after `Launched`), and it needs no transcript. |

- **Steward ruling R-35** (contracts `main` `f969f5e`; no amendment) settles this table. The code cites R-35.
- **`Adopt(id)` re-reads the worker** (steward ruling R-36, contracts `main` `c62085f`, with the follow-up `d18b6de`; no
  amendment). This replaces the round 2 rule "a retry keeps the intent" (P5-F22).
  - Every end is written to the row, `Lost(WorkerUnreachable)` and `Lost(WorkerVersion)` too. A `Lost` row keeps the
    worker's identity and token, **not** the earlier state.
  - `Adopt(id)` is always admitted for a `Lost(WorkerUnreachable | WorkerVersion)` session, whatever path ended it (A2-1).
    `WrongState` is only for a session in another state.
  - The handshake of 3 runs again. The report alone decides the result (AD-3), and no `Launch` and no stop is sent:

    | Report | Result |
    |---|---|
    | `Running` | `Running` |
    | `Exited` | `Exited`, with the cause from the report alone (no earlier `Stop` counts: it completed, LC-5) |
    | `NotLaunched` | `Lost(StartInterrupted)`: a start that never reached its payload (AD-2). No `Launch` on a `Lost` row. |
    | `Spawning` | the host waits for the spawn's answer with no `Launch`, bounded by `startup` (TM-3), as R-35 (b); then `Running` or the row below |
    | `LaunchFailed` | the outcome of a failed ordinary start (LC-4) |

    A probe, connect, hello or deadline failure is `Lost` with the reason that applies now (AD-2). A session with no
    recorded worker identity (a hello can end a start before the spawn answers) is `Lost(StartInterrupted)`, with no probe
    and no connect (AD-1; review P5-F26).
  - There is no `Stopping` outcome. A host that still wants the payload ended calls `Stop` after the adoption.
  - **Exactly one payload spawn:** the first adoption of a `Starting` row sends at most one `Launch` (R-35 (a)), and a
    retry sends none. The worker also accepts at most one `Launch` in its life.
  - A row that records `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` is posted as recorded by `AdoptAll`, with no
    handshake; `Adopt(id)` may retry it. Such a row with a worker identity and no valid token cannot be authenticated
    (AD-6), so it is `Lost(RegistryCorrupt)`, as a row of another state (review P5-F26).
  - Tests: an adoption that loses its link after the `Launch`, then a retry with each report (`Running` with no second
    `Launch`; `NotLaunched` gives `Lost(StartInterrupted)`; `Spawning` waits; `LaunchFailed`); a stop that meets a broken
    link, then a retry with `Running`, `Exited` and `NotLaunched`, on the same handle and on a new one.
- **Core never posts `Lost(Other)`** (R-35 correction: AD-2 names `Other` only as the value a host maps an unknown reason
  to). Every `Lost` that Core posts carries a listed AD-2 reason. P1's placeholder `Lost(Other)` for every decodable row
  violates AD-1, and this per-state adoption replaces it. The PR lists every Core-side construction of `Lost(Other)` in the
  workspace, with its fix (the pattern rule).
- The worker's orphan deadline covers every crash point: the worker gets `startup` at its launch (`--startup-ms`, part 1),
  before any `Launch`.
- Tests (R-35), through the process edge's script point between AD-7 steps 3 and 4, as proofs of
  `conf::ad_1_starting_with_identity_adopts` and `conf::ad_7_crash_between_steps_leaves_no_unregistered_payload` (no new
  id): a `Starting` row with `NotLaunched` adopts `Running` with exactly one `Launch`; a `Stopping` row with no payload is
  `Exited{cause: HostStop}` with no `Launch`; a spawn in flight adopts with no second `Launch`.

- Each row posts one `SessionState` (LC-11, EV-5); `Completed{AdoptAll}` follows the last one. Rows whose handshakes are
  in flight do not block each other; a row posts when its handshake ends.
- `Adopt(id)`: the handshake for one `Lost(WorkerUnreachable | WorkerVersion)` session, with the R-36 table above.
- AD-5 is LC-2: a live host holds the data-dir lock, so a second host cannot open.
- ID-2: the session keeps its `InstanceId` from the row; operations of the old host do not survive (ID-2).

### 6. Edges and the testkit (cross-package: P6)

- Host edge `connect_worker(endpoint) -> Option<LinkId>`: real, a non-blocking `mio` connect, registered like an accepted
  link; testkit, an in-memory endpoint of the `Sim`.
- The `Sim` needs: worker endpoints by path, kept across a host drop (the workers stay already, plan 4.1); `connect_worker`;
  and one process table for all hosts of a harness, so that an identity probe of the new host sees the old host's workers.
  The table is in memory only; real processes stay under `botster-test-process` (P6's request).
- `connect_worker` is a method of `HostEdges` (the driver's edge trait), not of the harness trait: the testkit implements
  it on its edges, and `RealEdges` implements it in `botster-core`. `RealCoreHarness` needs no change for it (agreed with
  P6: P5 codes the Sim changes).

### 7. The worker side (cross-package: P3's machine and binary)

- The worker machine gets candidate links: a connection on the endpoint is a candidate until its hello passes 3.3. Only a
  passed candidate replaces the current link. Inputs and actions grow by a link id.
- **A candidate is unauthenticated input, so it is bounded** (P3's review):
  - at most one candidate at a time: a connection that comes while one is pending is closed at once;
  - a candidate whose hello has not passed by the `startup` deadline is closed;
  - until its hello passes, a candidate's frame bound is one `Hello`.
  These are sans-IO decisions of `botster-worker-core`, with default-tier tests; the binary only accepts and passes the
  connection on.
- **What part 2 builds in the machine (`botster-worker-core`):**
  - Only a candidate has an id (`CandidateId`, named by the driver): `Input::Candidate`, `CandidateBytes`,
    `CandidateClosed`; `Action::CandidateClose`. The control link stays the one link of the existing inputs and actions.
    A passed candidate gives `Action::AdoptLink(id)`: the driver closes the control link, drops its unwritten bytes, and
    makes the candidate the control link, whose `LinkWritten` counts from zero. So the drivers' existing link code does not
    change. (This replaces "inputs and actions grow by a link id" above.)
  - The candidate's frame bound is the size of the largest hello of this instance (D4 fixes the hello fields).
  - A worker that is removing, waiting to end, terminating or ended takes no candidate.
  - Bytes after the hello in the same read go to the new link.
  - The `LaunchFailed` report keeps the spawn's reason; `terminal` is present only for a payload that ran.
  - The self-exit deadline runs while the worker has no payload and its link is not ready (AD-7,
    `conf::ad_7_crash_between_steps_leaves_no_unregistered_payload`). The worker's first input is the write of its own
    hello (`LinkWritten`), which needs nothing from the host, so a host that connects and never speaks does not keep the
    worker alive.
  - Every input first applies the adoption deadlines that are due at its instant (the candidate's deadline and the
    self-exit; P5-F27). A hello at the candidate's deadline does not pass, and a host at the self-exit deadline is too
    late, whatever order the driver gives the inputs. The stop grace stays a `Timer` deadline.
  - `WorkerConfig.startup` defaults to `CoreLimits.startup`.
- **What part 2 builds in the drivers (#176):**
  - The host launches the worker with `--endpoint` (`<data_dir>/w/<InstanceId>`) and `--startup-ms`
    (`CoreLimits.startup`).
  - The worker binary binds the endpoint (`mio`) before its first hello and removes it at every end of its driver. It
    accepts one connection a turn as a candidate and reads one chunk of each candidate a turn. `AdoptLink` drops the old
    link and the inputs that the driver queued for it (`io_decisions::fence`). An `AdoptLink` of a candidate that the
    driver no longer holds fails closed: the old link is dropped, and the machine hears `LinkClosed` of the new link.
  - The testkit gives candidates through the run's endpoints (`SimEdges::connect_worker`, `Workers::connect_endpoint`).
    A killed worker leaves its endpoint, and no worker listens on it; `Remove` removes it.
  - The decisions are pure functions with default-tier tests. **HOLD until #171:** the real-process proofs (a real host
    adopts a real worker through its endpoint; the endpoint file is removed at the exit and at `Remove`; the self-exit
    of a real worker at `--startup-ms`). They kill the 13 mutants of the real I/O that the default tier cannot reach.

### 8. Decisions recorded here

- **A52, `PayloadId.start_time`:** `Option<u64>`, `None` when the start time is unknown, and an unknown start time never
  matches an identity. A sentinel 0 is a value that looks valid (lead asked P5 to decide this).
- **The reachability deadline** is `startup` (3.7).
- **A missing endpoint is never repaired** (1). Core does not bind in its place, and the worker does not bind its endpoint
  again (P3's choice: a rebind races the cleaner and hides AD-2's `Lost`).
- **Peer uid:** optional defense in depth. The proof is the authority (AD-6), and the `0700` directory is the fence; the
  workspace's `rustix` reads the peer uid only on Linux.
- **Ownership** (agreed with P3): P5 codes the worker parts; P3 reviews the `botster-worker-core` and `botster-worker`
  parts. The worker-machine work is based on #168's `worker.rs` (the input state that the fence retires), or starts after
  #168 merges.

### 9. What the first code PR builds (host side, sans-IO)

- **The proof roles** (part 2): `host_proof` and `token_proof` (the worker's role) in `botster-core-link`, in both
  handshakes.
- **The report** (part 4): `WorkerMsg::Adopted` with `AdoptReport` and the five `AdoptedPayload` states. The input
  revisions and the routes come later with the worker side (P3's #168, P4a).
- **The row path** (part 5): `session_of_row` checks each decoded row before any probe or connect. A row whose state is
  `Lost(Other)` is a corrupt record (Core never writes it). A row that names a live worker gets `Flow::Adopt`: the
  identity probe, the connect, the host's hello, the worker's hello, the report, and then the table of part 5. R-35 (a)
  and (b) hand the row to the start's own flow (`StartFlow.adopted`), and a `Stopping` row with a payload hands it to the
  stop's own flow, so the outcomes are the ordinary ones.
- **No session before its state.** An adopting row is in the admission state `Adopting`: `get`, `list` and every
  operation see no session until the row's one `SessionState` is posted (`UnknownSession`). `StopAll` does not target it.
- **The retry** follows R-36 (part 5). `AdoptFlow.recorded` is the row's state for `AdoptAll` and `None` for `Adopt(id)`.
  Every end is written, with the worker's identity; an adoption also writes a state that it posts when the row records
  another one (a retried `Lost` row that is `Running` again records `Running`). A lost link during an adopted start is
  `Lost(WorkerUnreachable)` (the worker may have accepted the `Launch`), not the ordinary start's `Exited`. A retry that
  ends in the state that the session shows posts no second event; the `Adopt` result is the record.
- **An end during the post** (review P5-F25): an end that the worker reports while the adoption waits to post `Running`
  is applied after `Running` (OR-2).
- **An `Exited` row** is adopted with the exit that the row recorded (the report confirms that the payload ended).
- **`connect_worker(instance)`**, not `connect_worker(endpoint)`: the edge owns the endpoint path, because the edge also
  passes `--endpoint` at the spawn. The default of the `HostEdges` method answers `None`: until the worker binds its
  endpoint, every adoption of a live worker ends `Lost(WorkerUnreachable)` (AD-2: indeterminate, `Adopt(id)` may retry).
  `RealEdges` and the testkit `Sim` implement the method with the worker endpoint, after `botster-test-process` (#171),
  because the endpoint's proof is real-process.
- **Not in this PR:** the `FocusChanged` of DP-12 at adoption, `RouteAdopted` (P4a), the worker side (part 7), the launch
  arguments (part 1), and A52.

### Prior art (BUILD.md rule 0; written 2026-10-09)

Sources, read on 2026-10-09:
- tmux `master`: `tmux.c` (`make_label`), `server.c` (`server_accept`, `server_create_socket`, `server_signal`), `server-acl.c`,
  `proc.c` (`peer_check_version`), `client.c`, and `tmux(1)`.
- shpool `master`: `libshpool/src/daemon/server.rs` (`handle_conn`, `select_shell_desc`, `spawn_subshell`),
  `libshpool/src/daemon/peer.rs` (`check`), and the README.
- zellij `main`: `zellij-utils/src/consts.rs`, and the session-resurrection page of the documentation.
- abduco: the README.
- Old botster-core at `72b2e33` (read with `git show` only): `crates/botster-core/src/runtime/worker_process.rs`
  (`adopt_reserved_inner`), the old daemon crate's `src/daemon.rs` (`adoption_scan`), and the test
  `adoption_of_live_process_with_reaped_socket_fails_without_rebinding`.
- Vault notes:
  - "botster hub socket liveness requires a protocol handshake";
  - "adoption restart evidence must come from real protocol primitives not defaults";
  - "botster hub socket cleanup must preserve connectable sockets and repair missing socket paths";
  - "persisted core session metadata is revalidated against the current size cap".

**The shape.**
- tmux, zellij and shpool: one long-lived server owns every PTY, and a client reconnects to it. When the server dies,
  every session dies. zellij's "resurrection" does not keep processes: it saves the layout and the pane commands, and it
  runs them again behind a "Press ENTER to run" banner.
- abduco: one server process for each session, closer to Botster.
- Botster: each worker owns one session and outlives the host (LC-12). So the worker is the server and the new host is the
  client: one client reattaches to many per-session servers.

| Point | tmux | shpool | zellij / abduco | Old botster-core | Botster (AD) and verdict |
|---|---|---|---|---|---|
| Where the endpoint is, and who may connect | Directory `$TMUX_TMPDIR/tmux-<uid>`, made `0700`. `make_label` refuses an existing one unless `lstat` shows a directory that the uid owns with no group or other bits ("directory %s has unsafe permissions"). The socket is bound under a umask. The server checks the peer uid against an ACL; the owner and root are always allowed. | The daemon reads the peer credentials of every connection (`SO_PEERCRED` on Linux; `getpeereid` and `LOCAL_PEERPID` on macOS). Another uid is refused: "shpool prohibits connections across users". A client binary that differs from the daemon's only gives a warning. | zellij: `<runtime or tmp dir>/zellij-<uid>/contract_version_1/`. abduco: `$HOME/.abduco` or `/tmp/abduco/$USER`, owner only. | The worker's socket path is stored in the registry row, and the host connects to it. | AD-6: an endpoint that only the host's uid can read and connect to. **REUSE (idea):** tmux's directory check (owned by the uid, no group or other bits, `lstat`, not a symlink) before a bind or a connect, and shpool's peer-uid check on the connected stream, as a second fence. Checked: the workspace's `rustix` 1.1.5 has `socket_peercred` (`SO_PEERCRED`) only under `cfg(linux_kernel)`, and no `getpeereid`; the workspace forbids `unsafe`. So the directory check is the fence on both OSes: a `0700` directory that the host's uid owns stops every other non-root uid at `connect`, because `connect` needs search permission on the directory. The Linux peer-uid check is optional defense in depth; adding it is a design choice for the P5 PR, not a requirement of AD-6. |
| How the other end is checked | No identity check beyond the uid. Every message header carries `PROTOCOL_VERSION`; on a mismatch `peer_check_version` answers `MSG_VERSION`, marks the peer `PEER_BAD`, and dispatches no later message. | The daemon sends its version first; the client only warns on a mismatch. | zellij: compatibility by directory (`contract_version_N`), with no check on the link. | Hello and welcome. Only the exact protocol is accepted. The welcome must name the same session. No secret: any process of the same uid that listens on the path can answer. | AD-6 and DP-8: a token proof bound to the instance and the epoch, which is stronger than all four. **REJECT** an exact-version-only rule: AD-4 adopts protocols N and N−1. **REUSE (idea):** tmux's `PEER_BAD`: after a failed check, no later frame of that link is decoded or acted on. This is A11. Vault: an endpoint is live only after the handshake answers, never because a path exists or a connect succeeds. |
| What state the new client gets | The server keeps each grid and redraws the whole screen for the new client. | An in-memory render of the terminal redraws the screen; the restore mode keeps a number of output lines (default 500). | zellij: the server renders the full screen for each client. Resurrection runs the commands again (rejected below). abduco: the README names no screen state. | The welcome repeated the spawn-time modes, so after adoption the host asked the worker for the live modes ("The adopted welcome repeats spawn-time modes. Probe for live ones."). | AD-1: the worker owns the libghostty terminal, so it is the "server that keeps the screen". **REUSE the lesson:** the adoption report gives the live state (payload state, modes, size, identity); never values remembered from the spawn, and never a positive default (vault). **REJECT** zellij's resurrection: Core never starts a replacement for a session that it could not adopt (`Lost`). Old test: a missing endpoint must not create a replacement worker. |
| The old client is still attached | Several clients can attach. `attach -d` detaches the others; `-x` also sends SIGHUP to the client's parent. | One client per session: an attach to a busy session gets `Busy`, unless `-f` or `shpool detach` forces it. | zellij: several clients. abduco: several; only the newest non-read-only client resizes. | — | One controlling host. A live old host holds the data-dir lock (LC-2), so a second host cannot open. For an old host that died, the worker accepts only an epoch above every epoch it has seen and closes the old link (DP-8). This is tmux's `-d` with the epoch as the rule, and shpool's one controller per session. **REUSE (idea).** |

**Other lessons.**
- A socket path can disappear while its listener lives (a `/tmp` cleaner on macOS; vault). The old daemon then failed
  adoption without binding a replacement and without killing the worker (old test above). tmux re-creates its socket on
  `SIGUSR1`. **Botster:** a worker endpoint that cannot be connected to gives `Lost(WorkerUnreachable)`; Core never
  binds or spawns in its place. **Decided (P3's choice, part 8):** the worker does not bind its endpoint again: a
  rebind races the cleaner and hides AD-2's `Lost`.
- Validate everything that can refuse an adoption before the first change of state (vault: "persisted core session metadata
  is revalidated"): a row is decoded and checked before the host connects; a failed check changes nothing.

**Hand-rolled, with the reason.** The worker endpoint, the connect and the handshake use `std`/`mio` Unix sockets and the
existing link codec (`botster-core-link`). No library offers a per-session reattach with a token proof; tmux, shpool and
zellij are programs, not libraries. The directory check uses `rustix` (already in the workspace).

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
