# P7 guardian machine

The `Guardian` in `botster-guardian-core` owns the service lifetime state.
It performs no I/O.
The real driver and the testkit driver use this same machine through `Machine::handle` only.
The guardian holds no service lane connection.
The host owns lane authentication, epochs, queues, and public events.

## Modules

- `guardian.rs`: the lifecycle machine.
- `link.rs`: the hello, host authentication, and the written-byte count of one connection.
- `log.rs`: the bounded log ring.
- `wire.rs`: commands, reports, and the raw log frame.

## State

The state is typed, so impossible combinations cannot exist.

- `Service`: `Unlaunched`, `Spawning`, `Live(Leader)`, `Reaping`, or `Gone`.
  `Gone` never launches again (SV-7).
- `Spawning` keeps what arrives before the exec result: an exit, and a stop or kill request.
- A `Leader` has two independent tracks:
  - the exit: `Running`, `Exited(status)` while `DrainLogs` is outstanding, then `Drained(status)`;
  - the signals: `Idle`, `Census`, `Term`, `Grace`, `Killing`, then `Killed`.
- `Ending`: `Remove`, `Flush`, `Now`, or `Done`. It says how the guardian ends once the service is gone.

## Driver contract

The driver answers each process action with exactly one input:

| Action | Input |
|---|---|
| `SpawnService` | `Spawned` |
| `Enumerate` | `Descendants` |
| `TermService` | `TermSent` |
| `KillTree` | `TreeKilled` |
| `DrainLogs` | `LogsDrained` |
| `ReapService` | `Reaped` |

A result that answers no outstanding action changes nothing.

The driver must keep the leader unreaped until `ReapService`.
The driver must verify each descendant identity before it signals that descendant.
`KillTree` covers the service group, the known descendants, and a current enumeration.
The driver checks for live descendants after the kill.
`TermSent` and `TreeKilled` report whether the signal reached a leader that had not started to exit.
`DrainLogs` reads the output and cause bytes present at the exit.
A descendant that holds a pipe must not prolong the drain.

## Signals and exit causes

Stop sends `Enumerate`, then `TermService`.
The stop grace runs from the time of the `TermSent` result.
So census and delivery delays cannot shorten the grace (finding F1).

A kill request waits for an outstanding census or SIGTERM result, and then sends `KillTree`.
So SIGKILL never overtakes SIGTERM, and `KillTree` always carries the census.

The leader's exit also starts `KillTree`, so the group's survivors die with the leader (SV-9).

The exit cause follows SV-5:

1. The cause of the guardian's last signal that reached a live leader.
   SIGTERM gives `HostStop`.
   A kill gives `StartupTimeout`, `OrphanGrace`, or `Killed`.
2. Else the first cooperative cause byte gives `ChildReported`.
3. Else `Signal` or `Normal`, from the leader's status.

The guardian reports `Exited` and sends `ReapService` only when the exit is drained and the tree kill is complete.

## Deadlines

- Startup runs from exec until the host sends `EpochCommitted` (A2-5).
- Orphan grace runs from the start until a host authenticates, and again from each link loss (SV-8).
  A failed hello does not extend it.
  Only authentication cancels it.
- The stop grace runs from the `TermSent` result.

## Logs

`service_log_tail` is a sync read on the host (SV-9).
So the guardian pushes its output to the host; the host does not request it.

- The guardian keeps the newest `log_bytes` bytes (`CoreLimits.service_log_bytes`).
- Log bytes travel raw in `LOG_FRAME` frames as `[u64 LE offset][bytes]` (plan 3: bulk data is raw).
  The offset counts every byte that the service wrote before the chunk.
- One batch is in flight at a time.
  A new batch waits until the driver reports the previous batch written.
  So the queued log never exceeds the ring.
- After each authentication, the guardian resends its whole ring.
- A host that sees an offset other than the end of what it holds keeps only what follows.
  With the same bound, the host's ring then equals the guardian's ring.

## Ending

- `Remove` kills the tree, reports `Removed` once the service is gone, and ends after the driver wrote that report or the link closed.
- `Terminate` and orphan expiry kill the tree and end without a host.
- A dying guardian accepts no new connection.
- The guardian retains its payload identity, spawn report, exit, and log across host loss, and sends them after each authentication.

## Prior art

The prior-art pass used old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` through `git show` only.
It read `runtime/plugin_process/launch.rs` and `runtime/plugin_process/supervisor.rs` under the old `crates/botster-core/src` tree.
It also read the pinned Stage 1 plan, the earlier rebuild plan, and the vault's process-group cleanup note.

This change reuses the existing `botster-core-link` frame decoder, hello codec, and token proof.
It uses the pinned contract types and the workspace versions of serde and bolero.
It adds one crate: `botster-guardian-core`.
It adds no external dependency version.

This change copies no old source.
The old supervisor uses a thread, locks, and a real clock.
Those mechanisms cannot run in the required sans-IO machine.
The new machine keeps the old cleanup constraint: no signal after the leader is reaped.
The later real-edge change will assess reuse of the descriptor and rlimit code.

The guardian lifetime state is new contract logic.
A runtime or process library cannot own the contract's injected inputs and actions.
The log ring is a `VecDeque` with an offset; no ring crate is needed for that.
Both decoders (`Command`, `LogChunk`) have registered bolero harnesses.

## Verification scope

The focused tests cite the guardian's clauses and derive each expected value from the configuration, the spec, or the injected input.
They check authentication, exec results, startup, orphan grace, the stop sequence and its grace, deferred kills, exit causes, the log stream and its bound, and the ending order.
These tests prove the machine transitions.
They do not prove an OS signal, descriptor inheritance, or a real parent-death guarantee.
The later real-process tests must prove those conditions.
No conformance id leaves `conformance/core-pending.txt` in this change.
