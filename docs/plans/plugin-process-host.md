# Plugin Process Host (Core premise)

Status: premise revision 2 for review, 2026-09-26. Writer: Core
plugin-process-host session (branch `delivery/plugin-process-host-20260926`).
Contract partner: the Hub plugin-platform writer
(`docs/plans/plugin-platform.md` in botster-hub, sections 4.1, 5, 11, and 15).
Revision 2 answers reviewer findings R1-R6 (section 12 maps each finding to its
answer).

## 1. User decision and scope

Each plugin runs in its own OS process. The goals are crash containment,
enforceable limits (the OS kills a stuck plugin; memory and CPU caps are not
cooperative), and defense in depth (an OS sandbox per plugin process). The
process boundary also closes the deferred isolation gap: SIGKILL preempts a
native stall or a finalizer that the thread host cannot preempt.

Core owns the policy-free mechanism. Hub owns every policy value: the sandbox
profile, every limit, the restart policy, and the capability grants. Core adds
no numeric default. Every number in this document is Hub-supplied and needs a
user decision before it is chosen.

## 2. Premise corrections found by the survey

1. **Core has no Lua.** Core has no `mlua` dependency. The Lua VM is Hub code
   behind Core's `PluginRuntime` trait (`LuaPluginRuntime`, botster-hub
   `src/lua_runtime.rs`). Therefore Core cannot ship a finished
   `botster-plugin-worker` binary that hosts Lua. The Hub writer and the
   orchestrator agreed on this split:
   - Core ships the **worker library** (`botster_core::plugin_process::worker`):
     the handshake, fd and environment hygiene, the sandbox hook call site, the
     capped allocator, the IPC loop, and cancellation.
   - Hub builds the **`botster-plugin-worker` binary**: the Core worker library
     plus the Hub Lua runtime. `mlua` stays out of Core.
   - Core ships a **test-only worker binary** with a scripted Rust runtime. Core
     proves the mechanism with real processes and without Lua.
2. **The exit watch is not on Core main yet.** `runtime/process_exit.rs`
   (kqueue `EVFILT_PROC` + SIGCHLD with `waitid(WNOWAIT)` reapability on macOS,
   pidfd on Linux, fix `bff9dc7`) is on `delivery/route-stall-resync-20260925`.
   Main is its ancestor. The orchestrator is landing it on main as a standalone
   slice. Until then, implementation happens locally on top of that branch.
3. **The session-worker spawn is not reusable.** It inherits the full parent
   environment, the parent process group, and the parent cwd. It has no rlimits,
   no `pre_exec`, and no fd sweep. It never drains stderr after startup, so a
   chatty worker can block on a full stderr pipe. It also has an unmarked connect
   retry loop (`connect_spawned_worker_socket`, sleep 10 ms). The plugin host
   adds a new generic launcher. The session path is out of scope here, and the
   connect poll goes to the event-driven rewrite inventory.
4. **`ControlQueue` is not reusable.** It hardcodes 32 items, has no byte
   accounting, and lets Cancel traffic occupy both reserved slots, which refuses
   Terminal (reviewer R2). The plugin host defines its own lanes (section 5.2).
5. **Cancellation cannot wake a waiter.** `PluginCancellationToken` is a bare
   `AtomicBool`. A forwarding runtime would have to poll it, which the
   event-driven rules forbid. Slice 1 adds a cancel-wake (section 9.1).
6. **There is no timer-marker guard in Core.** All new timers in this work carry
   `// timer:` markers. The guard itself belongs to the event-driven rewrite.

Reusable as-is: the frame codec (`encode_frame`, `FrameDecoder`, `ProtocolError`
in `contract/session_protocol.rs`), with one addition: a decoder constructor
that takes a caller-supplied maximum frame length. Also reusable:
`process_exit::ExitWatch`.

## 3. Architecture

```
Hub process                                         plugin process (one per plugin generation)
-----------                                         ------------------------------------------
PluginWorkerEngine                                  Core worker library
  - admission, queues, deadlines, completions        - fd sweep (keep 0-4), env as given
  - delivery pool (new, section 5.1)                 - Bootstrap: memory cap, Hub apply_sandbox
    |  PluginRuntime::invoke / stop                  - Ready, Load, Loaded
    v                                                - serial Invoke FIFO (one Lua state)
ProcessPluginRuntime (Core)                          - credit ledger, bounded outbound lanes
  - writer thread    ==== fd 3 socketpair ====>      - fatal-cause pipe writer (fd 4)
  - reader thread    <=== fd 3 socketpair =====     Hub Lua runtime (PluginRuntime impl)
  - supervisor thread (every deadline, section 6)
  - exit-watch thread (the only reaper)  <== fd 4 fatal pipe (1 byte)
  - stderr drain thread
```

`ProcessPluginRuntime` implements the existing `PluginRuntime` trait. The engine
keeps all its current mechanics: class-aware admission, queue bounds, the
deadline waiter, completion reservations, and generation scoping. The same Hub
plugin code runs on the thread host (Hub `LuaPluginRuntime` directly) and on the
process host (`ProcessPluginRuntime` forwarding to the child). This meets the
host-agnostic requirement. The delivery pool is an engine API, so both hosts use
it.

One process is one engine worker generation. A reload spawns a new process under
a new generation. A dead process is never reused.

## 4. IPC contract

Transport: one `AF_UNIX SOCK_STREAM` socketpair. The child end is fd 3. Frames
use the existing codec, `[u32 LE len][u8 type][payload]`, with a Hub-supplied
maximum frame length. The handshake uses the magic `BPW1` and its own version
byte. Envelopes are JSON (serde). Plugin-API bodies are Hub-owned and opaque to
Core. Core carries them as `BoundaryJson`, never interprets them, and charges
their encoded bytes to the plugin.

A second channel, the **fatal-cause pipe**, is a one-way pipe. The child's
write end is fd 4 (`O_NONBLOCK`). It carries at most one cause byte (section
7.3).

Parent to child:

| Frame | Content |
|---|---|
| `Bootstrap` | protocol version; `sandbox: BoundaryJson`; `memory_cap_bytes: Option<u64>` |
| `Load` | `sources: BoundaryJson` (the package module set as in-memory text, because the sandboxed child has no filesystem); `config: BoundaryJson`; the initial ingress, reply, and log grants (section 5.1) |
| `Invoke` | `PluginInvocationRequest` |
| `Cancel` | `request_id` |
| `Credit` | one of `DeliveryPool{slots, request_bytes}` (once, at `attach_delivery`), `Delivery{call_id}`, `Reply{call_id}`, `IngressBytes{bytes}`, `Log{count, bytes}` |
| `Shutdown` | none |

Child to parent:

| Frame | Content |
|---|---|
| `Ready` | pid; sent only after the memory cap is set and the sandbox hook returned `Ok` |
| `BootstrapFailed` / `LoadFailed` | typed reason |
| `Loaded` | `registration: BoundaryJson` (Hub turns it into `PluginWorkerRegistration`) |
| `InvocationResult` | `PluginInvocationResult` |
| `HostCall` | `kind` (`Call` or `Reply`), `call_id` (one namespace for calls and replies: unique among the ids whose credit has not returned), `invocation_request_id` (the invoke running in the child; not a plugin-supplied value), `max_result_bytes` (`Call` only), `body` |
| `Log` | `dropped_since_last: u64`, `body: BoundaryJson` |

**Every delivery into the plugin is an engine-admitted `Invoke`.** Host-call
results, Hub refusals of a host call (for example `capability_denied`), event
deliveries, and timer fires are all `Invoke` frames of Hub-reserved handler ids.
There is no other resume path, so every piece of plugin code runs under an
engine invoke deadline (section 7.1). This answers R6.

A suspended Lua handler (Hub coroutine) finishes its `Invoke` with a Hub-defined
"suspended" body. The later result `Invoke` resumes it. That routing is Hub logic
over the opaque bodies. The child library allows a handler to suspend only on a
`HostCall` that it has already sent with a debited delivery credit. Therefore
every suspended handler holds exactly one delivery-pool unit.

The child runs `Invoke` frames strictly one at a time, in FIFO order (one Lua
state, one thread). A `Cancel` for an invoke that is still queued in the child is
answered at once with a `Cancelled` result.

**Validation (R5).** The parent reader validates every child frame. It never
panics on child input. Each of these is a `ProtocolViolation`, and the parent
then kills the process group (section 7):
- a malformed frame, or a frame above the maximum length;
- an unknown frame type, or a frame that is not valid in the current lifecycle
  phase;
- an `InvocationResult` for a request id that is not in flight, or a second
  result for the same id. An invoke that `stop` failed early stays in the
  in-flight map, marked abandoned, until its result arrives or the process
  exits. Its late result is discarded and is not a violation;
- a `HostCall` or `Log` before `Loaded`;
- a `HostCall` whose `invocation_request_id` is not in flight;
- a `HostCall` with a duplicate open `call_id`, or without enough credit
  (a `Call` before `attach_delivery` has no delivery credit);
- a `Reply` larger than the reply allowance;
- a `Log` without enough credit.

A `Call` that arrives after the pool's generation retired is dropped, not a
violation: the process is being stopped, and the child could not know.

## 5. Bounds, credits, and backpressure

The rule for the child-to-parent direction: **every child frame is
credit-bounded or bounded by an in-flight invoke.** Therefore the parent reader
never blocks on the Hub and never buffers without a bound. The rule for the
parent-to-child direction: **every lane has a bound that is derived from numbers
the Hub already supplies,** and the lanes for Terminal and Cancel can never be
refused.

### 5.1 Delivery pool and credits (R1)

Engine API, used by both hosts (implemented in slice 3a,
`engine/plugin_delivery_pool.rs`):

```rust
pub struct PluginDeliveryQuota {           // every value is Hub policy
    pub call_result_slots: usize,
    pub call_result_request_bytes: usize,
    pub call_result_completion_bytes: usize, // payload allowance per result
    pub ordinary_completion_entries: usize,  // the plugin's ordinary share
    pub ordinary_completion_bytes: usize,
}

impl PluginWorkerEngine {
    /// All or nothing for the plugin's current generation.
    pub fn try_reserve_delivery(&self, plugin_key: &PluginKey, quota: PluginDeliveryQuota)
        -> Result<DeliveryPool, DeliveryRefusal>; // Backpressured | WorkerStopped | RejectedBudget
}

impl DeliveryPool {
    /// Record an accepted host call. Fails only when the caller overdraws
    /// (on the process host this means the child ignored its credits).
    pub fn accept_call(&self, call: CallId, max_result_bytes: usize) -> Result<(), PoolOverdraw>;
    /// Admit the call's single result Invoke. Never refuses for capacity.
    pub fn admit_result(&self, call: CallId, request: PluginInvocationRequest) -> PluginAdmissionResult;
    /// Terminal for a call that will never receive a result.
    pub fn release_call(&self, call: CallId) -> bool;
    /// Called once per returned unit, outside every engine lock.
    pub fn install_unit_returned(&self, notifier: UnitReturnedNotifier);
    pub fn free(&self) -> (usize, usize);
}
```

Mechanism: the reservation creates two completion-store funds, pre-charged
from the engine-wide completion pool (the pool's entries at
`call_result_completion_bytes` plus Core's per-entry metadata, and the
ordinary share), and sets the pool's queue room aside from the plugin's
Background queue. A result admitted from a unit is charged to the pool's
fund and uses the reserved room, so it needs no capacity check. The
plugin's ordinary completions are charged to its share. Retiring the
generation closes both funds: their unused part returns to the engine-wide
pool at once, and each outstanding entry returns as it drains.

- **Reservation.** At load, the pool takes `slots` Background queue slots plus
  `request_bytes`, and `slots` completion-store entries of
  `completion_bytes_per_slot` each, for the plugin's current generation. The
  invariant is: plugin Background capacity = pool + ordinary Background work.
- **Unit.** One accepted call holds one unit: one slot, its declared
  `max_result_bytes` of request bytes, and one completion entry.
- **Exclusive to host calls.** Only the child's host calls debit the pool.
  Hub-originated deliveries (event deliveries, timer fires, watch events) never
  use the pool. They are admitted through ordinary Background capacity, so a
  Hub event and a child host call can never spend the same room.
- **Conservation.** At every instant: child delivery credits + units in flight =
  pool size. "In flight" means sent by the child, queued in the Hub, admitted as
  a result job, executing, or completed but not yet drained. The child debits a
  unit *before* it sends the `HostCall`. The unit returns exactly once, through
  the pool's `call_id` map removal, when one of these happens:
  - the result job's completion is drained;
  - the result job fails or is cancelled;
  - the Hub calls `release_call(call_id)`;
  - the generation retires, in which case all units die with the pool.

  The first three send `Credit{Delivery{call_id}}` to the child. The Hub
  guarantees exactly one terminal (a result or `release_call`) per accepted
  call.
- **Admission of results cannot fail for capacity,** because the unit already
  holds the slot, the bytes, and the completion entry. A result request larger
  than the declared `max_result_bytes` is a Hub bug, and Core asserts it. The Hub
  converts an oversize result into a typed failure that fits.
- **Attach after registration.** The pool exists only for a registered
  engine generation, which is after `Loaded`. The Hub therefore reserves the
  pool after registration and calls `attach_delivery(pool)`, which grants the
  pool's free units to the child with `Credit{DeliveryPool}` and installs the
  pool's unit-returned notifier. Before the attach, the child has no delivery
  credit and refuses its calls locally.
- **Byte measure.** Every byte credit (ingress, reply, log) is charged the
  whole encoded frame (type byte plus payload, envelope included). Both sides
  know that length exactly, and the parent never re-serializes a body.
- **Ingress bytes.** The Hub receives `Call` frames through a bounded ingress
  queue. The child debits `IngressBytes` credit by the frame length before
  sending. The parent returns it when the Hub drains the call. This credit is
  separate from the delivery unit, because the body leaves the queue long
  before the result arrives.
- **Log.** The child debits `Log{1, frame bytes}` credit before sending. When
  it has no credit, it drops the log line locally, counts it, and never
  blocks; the count travels as `dropped_since_last` on the next sent line,
  exactly once. The parent returns log credit when the Hub drains the line.
- **Local refusal.** When the child lacks delivery or ingress credit, the child
  library returns a typed `Backpressured` result to the Lua caller at once. The
  handler does not suspend. No parent refusal frame exists. A call outside a
  running invoke (for example during load) is refused as `NotInInvocation`.
- **Reply (user decision, 2026-09-26).** A suspended request-response chain
  delivers its final result as a `Reply`: a `HostCall` with `kind: Reply`,
  fire-and-forget, with no result `Invoke`. It has its own conserved credit
  class, `reply_credits{count, bytes}`: `count` open replies, each a frame of
  at most `bytes` (the Hub sets 2 credits, sized for a 1 MiB body plus the
  envelope). The child debits one Reply credit before it sends. The Hub's
  only terminal for a Reply is `release_reply(call_id)` on the process, which
  returns the credit as `Credit{Reply{call_id}}` exactly once. A Reply takes
  no delivery-pool unit. A Reply without Reply credit is a protocol
  violation.
- **Reply exhaustion never drops a chain's result.** When no Reply credit is
  free, `HostPort::reply` waits for `Credit{Reply}` with no timer; the running
  invoke's cancellation (its deadline or a `Cancel`) ends the wait with a typed
  `Cancelled`. So the final result is either sent, or its invoke ends with a
  typed failure. `try_reply` refuses at once instead. The Hub holds at most as
  many open chains as Reply credits, so the wait covers only the credit's
  transit.
- **Frame bound versus Reply size.** `max_frame_bytes` must be at least
  `reply_credits.bytes`, or a full-size Reply cannot be framed. It must also
  carry the widest `Credit` frame. `spawn` validates both and returns
  `InvalidConfig` otherwise.
- **Completion markers only.** Every invocation (root, resume, or ordinary)
  completes with a small marker; payloads travel as credited messages. The
  marker bound is the pool's `completion_bytes_per_slot` for results, and the
  Hub's ordinary completion reservation otherwise. Core adds no number.
- **One atomic reservation at load.** The Hub reserves the delivery pool and
  its ordinary completion share together:
  `try_reserve_delivery(plugin_key, specs) -> Result<Reservations, DeliveryRefusal>`
  is all-or-nothing across every spec, so a load either gets its whole quota
  or fails with a typed refusal (the Hub reports `quota_exceeded`).
- There is no chain pool (user decision, 2026-09-26).

### 5.2 Parent-to-child lanes (R2)

The writer thread drains one FIFO. Each frame class has its own admission bound,
and the FIFO capacity is the sum of the class bounds, so one class can never
consume another class's room. All bounds are derived; none is a new number.

| Class | Count bound | Byte bound | Why the bound holds |
|---|---|---|---|
| `Shutdown` | 1 | fixed | sent at most once per process |
| `Cancel` | executor concurrency | fixed per frame | at most one Cancel per in-flight invoke |
| `Invoke` | executor concurrency | engine class byte caps | each executor thread has at most one invoke in flight, and the request was already admitted |
| `Credit` | pool slots + Reply credits + 1 pool grant, plus 1 ingress and 1 log total | fixed per frame | the parent's account keeps every return spent until the writer takes its frame, so a child that ignores its credits cannot spend a queued return, and each unit and reply is at most one pending id; ingress and log credits coalesce into one pending total each |
| `Bootstrap`, `Load` | 1 each | max frame length | startup only |

Frames stay counted until the writer has written them completely
(writer-owned frames are included). Kill never uses this FIFO: `killpg` is a
system call, so kill makes progress under any saturation.

Frames and credits share one push sequence, and the writer sends them in that
order (review H3). Nothing waits behind an item pushed after it, so a stream of
returned credit cannot hold back a queued `Cancel`, `Shutdown`, or `Invoke`,
and a credit returned before an `Invoke` reaches the child first. The account
restores a credit when the writer takes its frame, not after the write: a
compliant child may read and spend it before the parent's write call returns.
So at most one credit frame is outside the account (review H1).

The child side mirrors this (review H2). No plugin-facing call writes the
socket: frames enter one FIFO, and a child writer thread sends them. A host
call or log line enters only after its credit is debited, and an
`InvocationResult` only for an invoke in flight; `Load` carries
`max_in_flight_invokes`. A result stops counting when the writer takes it,
not after its write returns, because the parent may receive it and send the
next `Invoke` first (review H5). So every counted result belongs to an invoke
the parent still holds in flight, and more than `max_in_flight_invokes` of them
is a parent protocol error. The FIFO needs no new number. A Lua log call only enqueues or drops,
so child logging never blocks. After `Shutdown`, the child exits once the FIFO
is written; the shutdown deadline bounds that. The decode buffers on both
sides are bounded by the maximum frame length.

### 5.3 stderr

A drain thread reads the child's stderr to EOF. It keeps a bounded tail for
diagnostics (Hub-supplied size) and counts the dropped bytes. A full stderr pipe
can never block the child.

## 6. Supervision and lifecycle (R3)

**One supervisor thread per process owns every deadline.** It keeps a deadline
book and waits on a condvar until the earliest deadline
(`// timer: deadline — <entry>; expiry kills the plugin process group`). On
expiry it calls `kill_group`. No other thread owns a timer. Executor threads wait
only for events (a result, a stop, or the exit).

**The exit-watch thread is the only reaper.** It starts right after the spawn and
owns the `Child`. It holds its own reference to the shared state, so it finishes
reaping and retirement even after every caller has returned or dropped the
runtime.

Lifecycle:

1. **Spawn** (parent launcher):
   - `env_clear()`, then the Hub-supplied environment allowlist.
   - cwd set to a Hub-supplied directory.
   - `process_group(0)`: the child leads a new process group.
   - stdin and stdout on `/dev/null`; stderr on a pipe; the socketpair child end
     is `dup2`'d to fd 3; the fatal pipe write end is `dup2`'d to fd 4.
   - Hub-supplied rlimits, applied in `pre_exec` with `setrlimit` only
     (async-signal-safe).

   The supervisor arms the **startup deadline**. It covers spawn through
   `Loaded`, so a spinning `Load` is bounded.
2. **Hygiene** (child, the first action in `main`): close every fd except 0-4
   (enumerated from `/dev/fd` on macOS and `/proc/self/fd` on Linux). This
   covers fds that other libraries opened without `CLOEXEC`.
3. **Bootstrap:** set the allocator cap from `memory_cap_bytes`, install the
   fatal panic hook, then call the Hub hook `apply_sandbox(&BoundaryJson)`. Core
   never interprets the profile (Seatbelt, or Landlock + seccomp, are Hub
   choices).
4. **Ready**, then **Load**, then `Loaded` or `LoadFailed`. Plugin code runs for
   the first time in `Load`, after the cap and the sandbox are in place. On
   `Loaded`, the supervisor disarms the startup deadline.
5. **Run:** frames flow as in sections 4 and 5.
6. **Stop:** `PluginRuntime::stop` and `Drop` both do the same three things:
   - fail every waiting invoke at once with `WorkerStopped`, so the engine's
     executor join returns promptly;
   - queue `Shutdown` in its reserved lane;
   - arm the **shutdown deadline**.

   The child fails its queued invokes and exits. If it does not exit by the
   deadline (for example, a blocked writer, a stuck runtime, or a finalizer), the
   supervisor kills the group.

**Every post-spawn failure kills the group.** These are:
- startup deadline expiry;
- `BootstrapFailed` or `LoadFailed`;
- a protocol violation;
- a cancel grace expiry;
- shutdown deadline expiry;
- reader EOF or a read error while the process is alive.

**EOF is not exit.** A closed IPC channel only means the transport is gone. The
parent kills the group and waits for the exit watch. In-flight invokes fail only
when the exit watch reports the reaped exit, with the cause that the parent
recorded (for example `TransportClosed`). A startup failure returns its typed
error to the caller after the kill. Reaping continues on the exit-watch thread.

## 7. Kill, exit, and crash semantics

### 7.1 Deadline kill

The engine's existing deadline cancels the token. The cancel-wake (section 9.1)
wakes the waiting executor, which queues `Cancel{request_id}`. The executor then
registers a **cancel grace** entry with the supervisor. If a result arrives, the
entry is removed. If the grace expires, the supervisor kills the group. The cause
is `WorkerKilled(Deadline)`.

### 7.2 Group kill without pgid reuse

`kill_group` calls `killpg(pgid, SIGKILL)` only while the leader is unreaped. A
mutex that the exit watch holds while it reaps serializes `kill_group` and the
reap. An unreaped leader (alive or a zombie) keeps its pid, so the pgid cannot be
reused while a kill is possible. After the reap, `kill_group` does nothing.

### 7.3 Exit handling (exit watch)

1. `ExitWatch::wait(None)` reports that the leader is reapable. It does not reap
   (fix `bff9dc7`).
2. The exit watch calls `killpg(pgid, SIGKILL)` while the leader is still a
   zombie. This removes any other group member.
3. It reads the fatal-cause pipe without blocking.
4. It reaps with `try_wait`.
5. It classifies the cause from the evidence that the exit leaves (see 7.5).
6. It fails every in-flight invoke of the generation with the new failure kinds:
   - `WorkerCrashed`;
   - `WorkerKilled` with reason `Deadline`, `Budget`, `ProtocolViolation`,
     `MemoryCap`, `StartupDeadline`, `ShutdownDeadline`, `TransportClosed`, or
     `Requested`.
7. It retires the generation. This drops the delivery pool (every unit dies
   with it) and the queued HostCalls and logs. It then emits
   `PluginProcessExited{plugin_key, generation, cause, stderr_tail}` to the Hub.
   Later invokes for that generation fail at once with the same kind until the
   Hub reloads or unloads the plugin. The Hub owns the restart policy.

The steps run exactly once, because only the exit-watch thread runs them.

The failure-kind additions are a cold cut, with no compatibility shim.

### 7.4 Memory cap (R4, R5)

The orchestrator accepted the mechanism: `plugin_process::CappedAllocator`, a
counting `#[global_allocator]` for the worker binary. It covers every Rust
allocation and the vendored Lua heap, because mlua 0.11.6 allocates through
`std::alloc`. Plugins cannot run native C.

- **Cap installation.** The cap arrives in `Bootstrap.memory_cap_bytes`. The
  child library sets it before `Ready`, so it is in force before any plugin byte
  is loaded. `None` means no cap; that is a Hub choice, not a Core default.
- **Failure path.** The failure path is allocation-free and never blocks. When
  an allocation would exceed the cap, the allocator does not return null. A null
  return would let Lua raise a catchable "not enough memory" error. Instead the
  allocator does two things:
  1. It calls `write(4, &[CAUSE_MEMORY_CAP], 1)`. The pipe is `O_NONBLOCK`, and
     the call is one async-signal-safe system call with no lock, no allocation,
     and no use of the IPC socket.
  2. It calls `abort()`.

  The panic hook uses the same path with `CAUSE_PANIC`.
- **Fallback.** If the byte write fails, or a byte was already written, the exit
  watch classifies the exit by the wait status (`WorkerCrashed(SIGABRT)`).
- **Parent side.** The parent kills the group and cleans up through the exit
  watch (section 7.3). The full IPC socket plays no part in this path.

### 7.5 Cause arbitration (R7)

The reader thread, the supervisor, and the exit watch can all observe the same
death in any order. Therefore the cause never depends on which thread records
its intent first. A parent thread that decides to kill records a **kill
reason** (a deadline, a violation, a request, or `TransportClosed`) together
with the fact that it sent SIGKILL. The kill reason is only a reason to kill; it
is not evidence of how the process died. After the reap, the exit watch
classifies the exit from evidence, in this order:

1. **Fatal byte present:** `WorkerKilled(MemoryCap)` or `WorkerCrashed(Panic)`.
   The child wrote the byte before it died, so the byte proves the cause even
   if a parent kill came later.
2. **Wait status is SIGKILL, and a parent thread sent SIGKILL:** the first
   recorded kill reason (for example `WorkerKilled(Deadline)` or
   `WorkerKilled(TransportClosed)`).
3. **After a `Shutdown` was sent, the leader exited with code 0:** `Stopped`,
   which is not a failure.
4. **Anything else:** `WorkerCrashed` with the signal or the exit code. This
   includes SIGABRT, a SIGKILL that no parent thread sent (for example the
   `RLIMIT_CPU` hard limit), SIGXCPU, and an unexpected exit.

Consequences:
- An abort or a memory-cap death that closes the socket first still reports
  its own cause. Its wait status is SIGABRT (rule 4), or its fatal byte is
  present (rule 1). A later parent SIGKILL does not change the termination
  status of a process that is already dying from a fatal signal.
- A child that closes fd 3 and stays alive is killed and reports
  `WorkerKilled(TransportClosed)` (rule 2).
- One residual case remains. A child that calls `exit()` on its own while a
  parent kill races it can report the parent's kill reason. A voluntary exit
  is an anomaly in either classification. The tests cover rules 1, 2, and 4
  under both orders (EOF first, and exit watch first), using a test hook that
  holds the exit watch until the reader has recorded EOF, and the reverse
  hook.

## 8. Resource limits (mechanism only)

| Limit | Mechanism | Enforced by |
|---|---|---|
| Wall-clock per invoke | engine deadline, then `Cancel`, then grace, then `killpg` | parent |
| Startup through `Loaded` | supervisor startup deadline, then `killpg` | parent |
| Shutdown | supervisor shutdown deadline, then `killpg` | parent |
| CPU time per process | `RLIMIT_CPU` (soft: SIGXCPU; hard: SIGKILL) | kernel, both OSes |
| Open files | `RLIMIT_NOFILE` | kernel |
| Core dumps | `RLIMIT_CORE` | kernel |
| Memory | `CappedAllocator`, then fatal byte, then abort; `RLIMIT_AS` also available on Linux | child, then parent classification |
| Instructions per invoke | Lua hook (existing, Hub) | Hub runtime in the child |
| IPC | credits and derived lanes (section 5) | both sides; overdraw is a kill |

## 9. Core API sketch

### 9.1 Cancel-wake

As implemented and accepted (`ab1205c`):

```rust
/// Core-owned receiver: records the cancel and notifies its own waiter.
/// Only a leaf lock of its own; never blocks otherwise; never calls into the
/// engine or a capability runtime.
pub(crate) trait CancelTarget: Send + Sync + 'static {
    fn cancelled(&self);
}

impl PluginCancellationToken {
    /// Notify `target` once on cancel, or at once if already cancelled.
    /// Dropping the subscription removes it.
    pub(crate) fn subscribe(&self, target: Arc<dyn CancelTarget>) -> CancelSubscription;
}
```

Properties:
- Subscription is crate-private. No host code runs inside `cancel`, whatever
  locks the cancelling caller holds.
- Registration and `cancel` serialize on one mutex that also guards the flag,
  so no wake is lost. A late registration observes an earlier cancel.
- Targets run after that mutex is released, each under `catch_unwind`, so a
  panicking target cannot skip other targets or unwind the engine's deadline
  waiter.
- The engine's deadline waiter releases the admission lock and seals the
  `TimedOut` completion before it cancels, so a runtime that returns at once
  on the cancel cannot win the first-commit race.
- A subscription retires when its invoke completes (drop).

### 9.2 Parent and child

```rust
// Parent side
pub struct PluginProcessConfig {            // no Default: every value is Hub-supplied
    pub worker_path: PathBuf,
    pub cwd: PathBuf,
    pub env: Vec<(OsString, OsString)>,     // applied after env_clear
    pub rlimits: PluginProcessRlimits,      // each Option<u64>
    pub sandbox: BoundaryJson,
    pub memory_cap_bytes: Option<u64>,
    pub max_frame_bytes: usize,
    pub startup_deadline: Duration,         // spawn through Loaded
    pub shutdown_deadline: Duration,
    pub cancel_grace: Duration,
    pub ingress_bytes: usize,
    pub reply_credits: PluginReplyCredits,  // {count, bytes}
    pub log_credits: PluginLogCredits,      // {count, bytes}
    pub stderr_tail_bytes: usize,
}
impl PluginProcess {
    pub fn spawn(config, load: &LoadFrame)
        -> Result<(Self, PluginRegistration), PluginProcessError>;
    pub fn kill(&self);
    pub fn attach_delivery(&self, pool: DeliveryPool) -> Result<(), PluginProcessError>;
    pub fn drain_ingress(&self, max_items, max_bytes) -> Vec<PluginIngress>; // returns ingress and log credit
    pub fn release_reply(&self, call_id: CallId) -> bool;
    pub fn install_ingress_notifier(&self, notifier: PluginIngressNotifier);
    pub fn install_exit_notifier(&self, notifier: PluginExitNotifier);
    pub fn exit(&self) -> Option<PluginProcessExited>;
}
impl PluginRuntime for PluginProcess { /* invoke forwards; stop per section 6 */ }

// Child side (linked into the Hub binary)
pub struct WorkerHooks {
    pub apply_sandbox: fn(&BoundaryJson) -> Result<(), String>,
    pub load: fn(LoadFrame, HostPort) -> Result<LoadedPlugin, String>,
}
pub fn run_worker(hooks: WorkerHooks) -> !;

impl HostPort {                              // Clone; used inside a running invoke
    pub fn call(&self, max_result_bytes: usize, body: PluginMessageBody) -> Result<CallId, HostPortRefusal>;
    pub fn reply(&self, body, cancellation: &PluginCancellationToken) -> Result<CallId, HostPortRefusal>;
    pub fn try_reply(&self, body) -> Result<CallId, HostPortRefusal>;
    pub fn log(&self, body) -> bool;         // false: dropped and counted
}
// HostPortRefusal: Backpressured | NotInInvocation | TooLarge | Cancelled
```

`HostPort` is the child's credit-checked sender for `HostCall` and `Log`. The Hub
Lua API calls it. The same Hub API uses an in-process port on the thread host.
That port calls the same `DeliveryPool` API directly. The child clears the
running invoke under the sender's lock before it sends that invoke's result, so
no host call can follow its invoke's result on the wire.

## 10. Slices

Each slice is reviewed and has real-process tests with ablations. The tests run
on macOS (local) and Linux (CI, `ubuntu-latest`).

- **S0.** Develop on top of `delivery/route-stall-resync-20260925` until
  `process_exit.rs` reaches main, then rebase.
- **S1. Cancel-wake, launcher, and startup supervision.**
  - Add the token `subscribe` API.
  - Add the launcher: env, cwd, pgid, fds 3 and 4, rlimits, stderr drain.
  - Add the supervisor and the exit watch as the only reaper.
  - Add the child side: hygiene, then Bootstrap (cap and sandbox hook), Ready,
    Load, and Loaded.
  - Add the test worker binary and its prebuild artifact.

  Tests:
  - The child environment equals the allowlist.
  - The open fds are exactly 0-4 (ablation: skip the sweep; a leaked fd fails
    the test).
  - The sandbox hook and the cap run before `Load` (ablation: reorder).
  - A stuck `Load` is killed at the startup deadline, and its group is reaped
    (ablation: end the deadline at `Ready`).
  - A startup timeout reaps after the caller has returned.
  - A child that closes fd 3 but stays alive is killed and reaped (ablation:
    treat EOF as exit).
  - The token has no lost wake: subscribe, then cancel; cancel, then subscribe.
- **S2. Invoke, cancel, kill, and crash.**
  - Add `Invoke`, `Cancel`, `InvocationResult`, and `Shutdown`.
  - Add the derived lanes, the failure kinds, `PluginProcessExited`, and the
    grace kill.

  Tests:
  - spawn, load, and call.
  - A handler that spins natively is killed at its deadline plus grace
    (ablation: remove the grace kill).
  - `abort()` surfaces as `WorkerCrashed`, while a second plugin keeps serving.
  - A grandchild in the group dies (ablation: signal the pid, not the group).
  - Shutdown with a blocked child writer, and with a stuck finalizer, is killed
    at the shutdown deadline.
  - Lane saturation: `Shutdown` and `Cancel` are admitted while `Invoke` lanes
    are full (ablation: shared lane).
  - Malformed, oversize, unknown-type, unknown-id, and duplicate-id frames are
    each a kill without a Hub panic.
- **S3. Delivery pool, HostCall, ingress, and Log.**
  - Add the engine pool API (shared with the thread host) and the credits.

  Tests:
  - Dequeue without a result, then reservation exhaustion, gives a local
    refusal; `release_call` restores the credit.
  - Cancellation races with result admission.
  - Generation retirement with outstanding units.
  - An overdraw is a kill.
  - Ablation: remove pool enforcement.
- **S4. Limits and measurement.**
  - Add `CappedAllocator`, the fatal pipe, and the `RLIMIT_CPU` and
    `RLIMIT_NOFILE` tests.

  Tests:
  - The cap is hit while the IPC socket is full; the fatal cause is still
    reported and the whole group is killed (ablations: send the cause over IPC;
    return null from the allocator).

  Add a benchmark of the round trip for a capability call (HostCall, then
  result Invoke) and for an entity publish (HostCall), as a
  `// timer: measurement-window` probe. Report the numbers; no target is set.
- **S5. Hub integration support.** Pair with the Hub writer on the Hub binary
  and on policy wiring. Core changes only mechanism.

## 11. Open decisions

For the user, through the orchestrator (the Hub writer escalates them):
- the cancel grace;
- the startup and shutdown deadlines;
- the memory cap value;
- the pool size;
- the ingress and log credits;
- the maximum frame length;
- the stderr tail;
- the rlimits.

Core chooses none of these.

Settled:
- The memory-cap mechanism (orchestrator, 2026-09-26).
- The cancel-token waker seam, under the conditions in section 9.1 (reviewer).
- JSON envelopes, subject to bounded decode and the S4 measurement (reviewer).

## 12. Reviewer findings and answers

| Finding | Answer |
|---|---|
| R1 reservation credits | Standing delivery pool, with conserved units held until the call's terminal and returned exactly once through the `call_id` map (5.1) |
| R2 ControlQueue and accounting | Own lanes with derived per-class bounds and reserved Shutdown and Cancel room; every child frame credit-bounded; bounded decode buffers; kill outside the queue (5.2) |
| R3 lifecycle | One supervisor owns every deadline from spawn through `Loaded`, plus shutdown and grace; every post-spawn failure kills the group; EOF is not exit; the exit watch is the only reaper and does not depend on the caller (6) |
| R4 memory failure path | Fatal byte on a separate `O_NONBLOCK` pipe, then abort; wait-status fallback; parent killpg while the leader is a zombie (7.2-7.4) |
| R5 wire and validation | `memory_cap_bytes` in `Bootstrap`; every invalid child frame is a typed violation and a kill; `call_id` correlation (4, 5.1) |
| R6 unsupervised resumption | `HostCallRefused` removed; every resumption is an engine-admitted `Invoke` under its deadline; suspension requires a debited unit (4) |
| R7 cause arbitration | A kill reason is not exit evidence; classification uses the fatal byte, then SIGKILL plus a parent kill, then a clean exit after Shutdown, then the wait status; tests cover both race orders (7.5) |

Implementation obligations recorded from the revision 2 review:
- Delivery credits conserve request bytes as well as slots.
- A failed or cancelled result unit is not reusable while its completion entry
  is still occupied. The unit returns only when the completion entry is freed.
- Sizes that come from the child are validated (a typed violation) before any
  assertion about a Hub caller bug.
- Stale call and result ids are rejected by generation scope: `call_id` and
  `request_id` checks consult only the live in-flight maps of the current
  generation. Core keeps no unbounded set of historical ids.
