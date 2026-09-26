# Plugin Process Host (Core premise)

Status: premise for review, 2026-09-26. Writer: Core plugin-process-host session
(branch `delivery/plugin-process-host-20260926`). Contract partner: the Hub
plugin-platform writer (`docs/plans/plugin-platform.md` section 11 in botster-hub).

## 1. User decision and scope

Each plugin runs in its own OS process. The goals are crash containment,
enforceable limits (the OS kills a stuck plugin; memory and CPU caps are not
cooperative), and defense in depth (an OS sandbox per plugin process). The
process boundary also closes the deferred isolation gap: a native stall or a
finalizer that the thread host cannot preempt is preempted by SIGKILL.

Core owns the policy-free mechanism. Hub owns every policy value: the sandbox
profile, every limit, the restart policy, and the capability grants.

## 2. Premise corrections found by the survey

1. **Core has no Lua.** Core has no `mlua` dependency. The Lua VM is Hub code
   behind Core's `PluginRuntime` trait (`LuaPluginRuntime`, botster-hub
   `src/lua_runtime.rs`). Therefore Core cannot ship a finished
   `botster-plugin-worker` binary that hosts Lua. Agreed with the Hub writer:
   - Core ships the **worker library** (`botster_core::plugin_process::worker`):
     handshake, fd and environment hygiene, the sandbox hook call site, the IPC
     loop, and cancellation.
   - Hub builds the **`botster-plugin-worker` binary**: the Core worker library
     plus the Hub Lua runtime. `mlua` stays out of Core.
   - Core ships a **test-only worker binary** with a scripted Rust runtime. Core
     proves the mechanism with real processes and without Lua.
2. **The exit watch is not on Core main yet.** `runtime/process_exit.rs`
   (kqueue `EVFILT_PROC` + SIGCHLD with `waitid(WNOWAIT)` reapability on macOS,
   pidfd on Linux, fix `bff9dc7`) is on `delivery/route-stall-resync-20260925`.
   Main is its ancestor. Implementation slices start after that branch reaches
   main. This premise does not depend on it.
3. **The session-worker spawn is not reusable as-is.** It inherits the full
   parent environment, the parent process group, and the parent cwd. It has no
   rlimits, no `pre_exec`, and no fd sweep. It never drains stderr after
   startup, so a chatty worker can block on a full stderr pipe. It also has an
   unmarked connect retry loop (`connect_spawned_worker_socket`, sleep 10 ms).
   The plugin host adds a new generic launcher. It does not copy the session
   launcher. The session path is out of scope here; the connect poll is reported
   to the event-driven rewrite inventory.
4. **Cancellation cannot wake a waiter.** `PluginCancellationToken` is a bare
   `AtomicBool`. A forwarding runtime would have to poll it, which the
   event-driven rules forbid. Slice 1 adds a cancel-wake to the token.
5. **There is no timer-marker guard in Core.** No test checks `// timer:`
   markers today. All new timers in this work carry markers. The guard itself
   belongs to the event-driven rewrite.

Reusable as-is: the frame codec (`encode_frame`, `FrameDecoder`,
`MAX_FRAME_LEN`, `ProtocolError` in `contract/session_protocol.rs`), the
`ControlQueue` (bounded frames with reserved cancel and terminal slots), and
`process_exit::ExitWatch`.

## 3. Architecture

```
Hub process                                         plugin process (one per plugin)
-----------                                         ------------------------------
PluginWorkerEngine (unchanged admission, queues,    Core worker library
  deadlines, completions, generations)                - fd sweep, env check
    |  PluginRuntime::invoke / stop                   - Bootstrap -> Hub apply_sandbox hook
    v                                                 - Ready
ProcessPluginRuntime (Core)  <== fd 3 socketpair ==>  - IPC loop, serial Invoke FIFO
  - writer thread (ControlQueue)                      - HostCall credits, Log
  - reader thread (FrameDecoder)                    Hub Lua runtime (PluginRuntime impl)
  - exit-watch thread (ExitWatch)
  - HostCall ingress + Log queues -> Hub notifier
```

`ProcessPluginRuntime` implements the existing `PluginRuntime` trait. The engine
keeps all its current mechanics: class-aware admission, queue bounds, the
deadline waiter, completion reservations, and generation scoping. The same Hub
plugin code runs on the thread host (Hub `LuaPluginRuntime` directly) and on the
process host (`ProcessPluginRuntime` forwarding to the child). This is the
host-agnostic requirement.

One process is one plugin generation. A reload spawns a new process under a new
engine generation. A dead process is never reused.

## 4. IPC contract (agreed with the Hub writer)

Transport: one `AF_UNIX SOCK_STREAM` socketpair. The child end is fd 3. Frames
use the existing codec: `[u32 LE len][u8 type][payload]`. The handshake uses a
new magic `BPW1` and its own version byte. Envelopes are JSON (serde), as in the
session protocol. Plugin-API bodies are Hub-owned and opaque to Core. Core
carries them as `BoundaryJson`, never interprets them, and charges their encoded
bytes to the plugin.

Parent to child:

| Frame | Content |
|---|---|
| `Bootstrap` | `sandbox: BoundaryJson` (Hub profile), protocol version |
| `Load` | `sources: BoundaryJson` (the package module set as in-memory text; the sandboxed child has no filesystem), `config: BoundaryJson`, `host_call_credits: {count, bytes}` |
| `Invoke` | `PluginInvocationRequest` |
| `Cancel` | `request_id` |
| `Credit` | returned HostCall credits `{count, bytes}` |
| `Shutdown` | none |

Child to parent:

| Frame | Content |
|---|---|
| `Ready` | pid; sent only after the sandbox hook returned `Ok` |
| `SandboxFailed` / `LoadFailed` | typed reason |
| `Loaded` | `registration: BoundaryJson` (Hub turns it into `PluginWorkerRegistration`) |
| `InvocationResult` | `PluginInvocationResult` |
| `HostCall` | `call_id`, `invocation_request_id`, `max_result_bytes`, `body: BoundaryJson` (fire-and-forget) |
| `Log` | `body: BoundaryJson` |

Deliveries into the plugin (host-call results, event deliveries, timer fires)
are **ordinary `Invoke` frames** of Hub-reserved handler ids. They go through
`PluginWorkerEngine` admission like any other invocation. Core therefore needs
no separate result or event frame, and deliveries reuse admission, deadlines,
cancellation, and generation scoping.

A suspended Lua handler (Hub coroutine) finishes its `Invoke` with a Hub-defined
"suspended" body. A later resume `Invoke` produces the final result. That
routing is Hub logic over the opaque result body.

The child runs `Invoke` frames strictly one at a time, in FIFO order (one Lua
state, one thread). The engine's executor concurrency bounds how many invokes
are in flight to the child. A `Cancel` for an invoke that is still queued in the
child is answered at once with a `Cancelled` result.

## 5. Bounds and backpressure

Every number below comes from the Hub. The process-host config type has no
`Default` implementation, and Core adds no default value.

1. **HostCall credits.** At `Load`, the parent grants `count` credits and a
   `bytes` allowance. The child library refuses a HostCall locally, with a typed
   `Backpressured` error and without suspending the handler, when it has no
   credit. The parent returns credits with a `Credit` frame when the Hub takes
   the call out of the ingress queue. The parent ingress queue therefore cannot
   overflow. A HostCall without credit is a protocol violation: Core kills the
   process (cause `ProtocolViolation`).
2. **Delivery reservations.** When Core accepts a HostCall, the engine reserves
   one delivery slot for that call's result `Invoke`: count 1 plus
   `max_result_bytes`, in the same generation, bounded by the plugin's
   reservation count and byte caps. If the caps cannot fit the reservation, the
   child refuses the call locally, as for a missing credit. The result `Invoke`
   consumes the reservation, and admission cannot refuse it for capacity. The
   Hub guarantees that the result fits the reservation (a larger result becomes
   a typed Hub failure); Core asserts this. Cancel, generation retirement, and
   process exit release the reservation. The count cap also bounds the number of
   suspended handlers, because each suspension holds exactly one reservation.
   This is an engine mechanism, so the thread host uses the same API.
3. **Log queue.** The log queue is per plugin and bounded (items + bytes). When
   the queue is full, Core drops the `Log` frame and increments a counter. Core
   reports the counter with the next accepted log. Logging never blocks the
   child.
4. **stderr.** A drain thread reads the child's stderr to EOF. Core keeps a
   bounded tail for diagnostics and counts the dropped bytes. A full stderr pipe
   can never block the child.
5. **Egress to the Hub.** HostCall and Log ingress use bounded queues with a
   Hub-installed notifier and bounded drains, the same pattern as
   `install_completion_notifier` and `drain_completions`.

## 6. Lifecycle

1. **Spawn** (parent, generic launcher):
   - `env_clear()` plus a Hub-supplied environment allowlist.
   - cwd set to a Hub-supplied directory.
   - `process_group(0)`: the child leads a new process group.
   - stdin and stdout on `/dev/null`; stderr on a pipe; the socketpair child end
     is `dup2`'d to fd 3.
   - Hub-supplied rlimits applied in `pre_exec` with `setrlimit` only
     (async-signal-safe).
2. **Hygiene** (child, first action in `main`, before any plugin byte is read):
   the child closes every fd except 0, 1, 2, and 3 (enumerated from `/dev/fd` on
   macOS and `/proc/self/fd` on Linux). This covers fds that other libraries
   opened without `CLOEXEC`.
3. **Sandbox:** the parent sends `Bootstrap{sandbox}`. The child library calls
   the Hub-supplied `apply_sandbox(&BoundaryJson) -> Result<(), String>` hook.
   Core never interprets the profile (Seatbelt on macOS, Landlock + seccomp on
   Linux are Hub choices).
4. **Ready:** the child sends `Ready`. The parent waits for this frame on the
   reader thread, as an event, bounded by a Hub-supplied startup deadline
   (`// timer: deadline`). Expiry is a typed `SpawnFailed(StartupTimedOut)`.
5. **Load:** the parent sends `Load`. The child sends `Loaded` or `LoadFailed`.
   Plugin code runs for the first time here, after the sandbox is in place.
6. **Run:** `Invoke`, `Cancel`, `HostCall`, `Credit`, and `Log` frames flow.
7. **Stop:** `PluginRuntime::stop` queues `Shutdown` in the reserved terminal
   slot. The child fails its queued invokes with `WorkerStopped` and exits. The
   exit watch reports the exit (see section 7).

## 7. Kill and crash semantics

- **Exit detection** is an event: the exit-watch thread blocks in
  `ExitWatch::wait(None)` and then reaps with `try_wait`. Reader EOF is a second
  event for the same fact. Core never uses a timer to discover an exit.
- **Deadline kill.** The engine's existing deadline cancels the token. The
  cancel-wake (slice 1) wakes the waiting executor thread, which sends
  `Cancel{request_id}`. If no result arrives within the Hub-supplied cancel
  grace (`// timer: deadline — cancel grace; expiry kills the plugin process
  group`), Core runs `killpg(pgid, SIGKILL)`. The grace value is a new number;
  the Hub writer takes it to the user. Core requires it and has no default.
- **Budget kill.** Hub can request a kill at any time through
  `ProcessPluginRuntime::kill(reason)`, for example from its own accounting.
  The cause is `Budget` or `Requested`.
- **On exit:** Core fails every in-flight invoke of that generation with a new
  `PluginInvocationFailureKind`:
  - `WorkerCrashed`, with the exit status or signal.
  - `WorkerKilled`, with a reason: `Deadline`, `Budget`, `ProtocolViolation`,
    `MemoryCap`, or `Requested`.

  Core also drops the queued HostCalls and releases their delivery
  reservations. It emits `PluginProcessExited{plugin_key, generation, cause,
  stderr_tail}` to the Hub. Later invokes for that generation fail at once with
  the same kind until the Hub reloads or unloads the plugin. The Hub owns the
  restart policy.
- **Containment.** A crash cannot corrupt Hub state, because the child holds no
  Hub state. Its registrations are retired by generation. Its callbacks and
  reservations are retired through the engine's generation scoping, which the
  exit event triggers.
- The failure-kind additions are a cold cut, with no compatibility shim.

## 8. Resource limits (mechanism only)

| Limit | Mechanism | Enforced by |
|---|---|---|
| Wall-clock per invoke | engine deadline -> `Cancel` -> grace -> `killpg` | parent |
| CPU time per process | `RLIMIT_CPU` (soft: SIGXCPU; hard: SIGKILL) | kernel, both OSes |
| Open files | `RLIMIT_NOFILE` | kernel |
| Core dumps | `RLIMIT_CORE` | kernel |
| Memory | see below | child |
| Instructions per invoke | Lua hook (existing, Hub) | Hub runtime in the child |

**Memory.** macOS does not enforce `RLIMIT_AS` or `RLIMIT_DATA` for `mmap`.
Parent-side RSS sampling would be a poll. Core therefore offers
`plugin_process::CappedAllocator`, a counting `#[global_allocator]` for the
worker binary. It refuses allocations above a Hub-supplied cap. On refusal, the
child writes a preallocated `MemoryCapExceeded` frame and aborts. The parent
reports `WorkerKilled(MemoryCap)`. This covers all Rust allocations and the
vendored Lua heap, because mlua allocates through the Rust allocator. It does
not cover native C code that calls `malloc` directly. On Linux, `RLIMIT_AS` is
also available as a hard kernel bound. **This is a product decision for the
user:** is the allocator cap (with the C-malloc gap on macOS) acceptable as the
"real" memory cap?

## 9. Hook interface (Core API sketch)

```rust
// Parent side
pub struct PluginProcessConfig {            // no Default
    pub worker_path: PathBuf,
    pub cwd: PathBuf,
    pub env: Vec<(OsString, OsString)>,     // allowlist, applied after env_clear
    pub rlimits: PluginProcessRlimits,      // each Option<u64>
    pub sandbox: BoundaryJson,              // opaque; sent in Bootstrap
    pub startup_deadline: Duration,
    pub cancel_grace: Duration,
    pub host_call_credits: CreditGrant,     // {count, bytes}
    pub delivery_reservations: CreditGrant, // {count, bytes}
    pub log_queue: CreditGrant,             // {count, bytes}
    pub stderr_tail_bytes: usize,
}
impl ProcessPluginRuntime {
    pub fn spawn(plugin_key, config, load: LoadFrame) -> Result<(Self, BoundaryJson /*registration*/), PluginProcessError>;
    pub fn kill(&self, reason: PluginKillReason);
    pub fn install_notifier(&self, notifier: PluginCompletionNotifier);
    pub fn drain_host_calls(&self, max_items, max_bytes) -> HostCallDrain;
    pub fn drain_logs(&self, max_items, max_bytes) -> LogDrain;
    pub fn take_exit(&self) -> Option<PluginProcessExited>;
}
impl PluginRuntime for ProcessPluginRuntime { /* invoke forwards; stop sends Shutdown */ }

// Child side (linked into the Hub binary)
pub struct WorkerHooks {
    pub apply_sandbox: fn(&BoundaryJson) -> Result<(), String>,
    pub load: fn(LoadFrame, HostCallPort) -> Result<(Arc<dyn PluginRuntime>, BoundaryJson), String>,
}
pub fn run_worker(hooks: WorkerHooks) -> !;
```

`HostCallPort` is the child's credit-checked sender for `HostCall` and `Log`.
The Hub Lua API calls it. The same Hub API uses an in-process port on the thread
host.

## 10. Slices (each reviewed, each with real-process tests and ablations)

- **S0.** Rebase onto main after `delivery/route-stall-resync-20260925` lands
  (for `process_exit.rs`).
- **S1. Cancel-wake and launcher.** Add a cancel-wake to
  `PluginCancellationToken` (registered wakers run once on `cancel`). Add the
  generic launcher (env, cwd, pgid, fd 3, rlimits, stderr drain) and the child
  hygiene, `Bootstrap`, sandbox hook, and `Ready` sequence. Add the test worker
  binary and its `prebuild` artifact entry.
  Tests:
  - The child's environment equals the allowlist.
  - The child's open fds are exactly {0,1,2,3}. Ablation: skip the sweep and
    the test fails on a leaked non-CLOEXEC fd.
  - The sandbox hook runs before `Load`. Ablation: reorder and the test fails.
  - A startup deadline expiry is typed.
- **S2. Invoke, crash, kill.** Add `ProcessPluginRuntime` with the
  `Invoke`/`Cancel`/`Shutdown`/`InvocationResult` frames, the exit watch, the
  failure kinds, `PluginProcessExited`, and `killpg` on grace expiry.
  Tests:
  - spawn -> load -> call.
  - A handler that spins natively is killed after its deadline plus grace.
  - A handler that calls `abort()` surfaces as `WorkerCrashed`, and a second
    plugin in the same engine keeps serving.
  - The whole process group dies.
  - Ablations: remove the killpg, remove the exit-watch failure path.
- **S3. HostCall, reservations, Log.** Add the credits, the engine delivery
  reservations (shared with the thread host), the Log queue, and
  protocol-violation kills.
- **S4. Limits and measurement.** Add `CappedAllocator`, `RLIMIT_CPU`, and
  `RLIMIT_NOFILE` tests. Add a benchmark of the round trip for a capability
  call (HostCall -> result Invoke) and an entity publish (HostCall), as a
  `// timer: measurement-window` probe. Report the numbers; no target is
  imposed.
- **S5. Hub integration support.** Pair with the Hub writer on the Hub binary
  and on policy wiring. Core changes only mechanism.

## 11. Open decisions

For the user (through the orchestrator):
1. The cancel grace value (the Hub writer is escalating it).
2. The memory-cap mechanism on macOS (section 8).
3. Every other number in `PluginProcessConfig` is a new limit. The Hub proposes
   them from its existing per-plugin accounting; Core adds none.

For the reviewer:
- Is extending `PluginCancellationToken` with wakers the right seam, compared
  with a per-invoke channel that the engine's deadline waiter signals?
- Is JSON envelope encoding acceptable until S4 measures it?
