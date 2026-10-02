# botster-worker-core: P3 design note

Package P3 (session worker, PTY and terminal) of Stage 1. Plan pin `c43693ff` (and `2b03dc1b`, no change to P3's scope). This
note covers milestone M1 (plan 6.2): the hello, the payload launch after the durable identity, and the exit with code and signal.
It is stacked on P1's head `2016886` (the control-link messages of `botster-core-link`).

## Shape

- `Worker` (`src/worker.rs`) is a sans-IO machine (plan 2.1): `handle(now, Input)`, `poll_action()`, `next_deadline()`. It reads
  no clock, starts no thread and does no I/O.
- Two drivers run the same machine (plan 2.1, A5-1):
  - the `botster-worker` binary, role `session`: one `mio` loop over the control socket, the PTY master, `SIGUSR1`
    (`signal-hook-mio`) and a `mio::Waker` for the exit watch; the control link is served first in each turn (plan 2.4);
  - the testkit (`botster-core-testkit/src/worker.rs`): the `Sim` steps the machine on the test thread, with the scripted
    program as the `Program` edge and an in-memory link. `TestkitCore::pump` runs the workers' ready work, then the host's pump.
- The real `Program` edge is `botster-core-sys/src/payload.rs`.

## The protocol of M1 (P1's wire)

| Host → worker | Worker |
|---|---|
| hello (`Hello` with the token proof, AD-6) | the worker sent its own hello first; a host hello with another proof, instance or epoch closes the link |
| `Launch` (AD-7 step 4) | `SpawnPayload`; then `Launched{features, terminal, formats, payload}` or `LaunchFailed{CwdMissing \| ExecFailed{errno}}` |
| `Stop` (LC-5) | `SIGTERM` to the payload group |
| `Kill` (LC-5, at `stop_grace`) | `SIGKILL` to the payload group |
| `Op{Signal}` (LC-6) | the signal to the group, then `Done{Ok(Unit)}`; no live group: `SessionEnded` |
| `Remove` (LC-7 step 3) | `SIGKILL` to the group if it is unreaped, reap, `RemoveResult{Deleted}`, close, end |
| `SIGUSR1` (`GroupSignal::EndPayload`, LC-5, P1 F7) | `SIGTERM`, then `SIGKILL` at `stop_grace` (`LaunchSpec.stop_grace_ms`); a repeat changes nothing |

The payload's exit is `Exited{code}` or `Exited{signal}` (EV-4), sent after one drain of the PTY that follows the exit, so the
output written before the exit is read first (the final model holds it, ST-5).

## Choices where the contract is silent (each is reviewable)

| Choice | Reason |
|---|---|
| The leader is reaped only after a `SIGKILL` to its group, once the leader has ended (`Kill`, a `Signal(Kill)`, the grace of `EndPayload`, or `Remove`). A leader that ends by itself stays a zombie until one of them. | Lead ruling on P1 F7: the group id cannot be reused while the worker may still signal it. |
| The graceful request is `SIGTERM` to the group. | LC-5 says "a graceful request"; the old runtime's lesson: closing the master only sends `SIGHUP`, which agents ignore. |
| An operation that M1 does not serve is answered `Done{Err(Internal)}` naming the operation. | Every request gets its `Done`; the reads, input and setters come with M2. `Internal` is the honest code for a worker that cannot serve it yet. |
| `Launched.terminal` is a placeholder (the launch size, default flags, revisions 0) and `formats` is empty. | The terminal model is M2 (libghostty through P2). No terminal-state id leaves `core-pending.txt` in M1. |
| A host message that the worker cannot decode is ignored; a frame above the bound closes the link. | Plan section 3: additive messages keep N−1 compatible; a bad length ends a stream. |
| A closed link leaves the payload running, and the exit is kept unreported. | DP-8. Reconnection and the report after adoption are P5's (AD-1). |
| `LaunchSpec.stop_grace_ms` (additive, serde default from the Core limits table) carries `CoreLimits.stop_grace`. | The worker times the grace of `EndPayload` itself; agreed with P1. |
| `Remove` while the spawn is out waits for the spawn's answer, then ends the payload. | LC-7: no payload outlives its session. |

Not in M1: the worker's own exit when no host attaches within `startup` (AD-7, P5's id `ad_7_crash_between_steps_...`); the
worker needs `startup` on its command line for it.

## Prior art (BUILD.md rule 0)

- **Vault:** "botster workers own authoritative terminal modes and semantic input encoding" (M2 follows it); "the old PTY crate's
  master is private" (plan 7.1); "O_CLOEXEC is process-scoped and does not affect SCM_RIGHTS fd transfer" (DP-2, later).
- **Old botster-core at `72b2e33`:**
  - `crates/botster-core/src/runtime/process_exit.rs` (kqueue `EVFILT_PROC` and `waitid(WNOWAIT)` on macOS, pidfd on Linux):
    **REJECTED as a steal**, kept as a lesson. Its kqueue path needs `unsafe` (`libc::kevent`, and `rustix::event::kqueue::kevent`
    is `unsafe fn`), and the workspace forbids `unsafe` code. Lead decision: one waiter thread per payload blocks in the safe
    `rustix::process::waitid(P_PID, WEXITED | WNOWAIT)` and wakes the loop through `mio::Waker`. Lessons kept: watch while the
    child is unreaped; report the exit without reaping. Its subtle cases (`NOTE_EXIT` before the child is reapable, `ESRCH` at
    registration, the `SIGCHLD` recheck) disappear, because `waitid` returns only when the child is reapable. One code path on
    macOS and Linux.
  - `crates/botster-core/src/runtime/local_process.rs` (on the old PTY crate that plan 7.1 rejects): **REJECTED** (plan 7.1). Lesson kept: `killpg`
    `SIGTERM`, then `SIGKILL`.
  - the old session-worker binary of the old daemon crate (plan 7.1, its path is in the PR's Prior-art note): **mechanism REJECTED** (not sans-IO), lessons kept: control
    first in each turn (the binary's loop); the remaining lessons (reply slots, the snapshot gate) are M2's.
  - Nothing was stolen: there is no `Stolen-From` commit.
- **Libraries added:**
  - `pty-process` 0.5.3: opens the PTY with `rustix`, starts the payload as a session leader with the PTY as its controlling
    terminal (`setsid`, `TIOCSCTTY`), and exposes the master descriptor. It is not the PTY crate that plan 7.1 rejects: the master is ours. Its
    `pre_exec` `unsafe` is inside the library, so our crates keep `unsafe_code = "forbid"`.
  - `signal-hook` 0.3 and `signal-hook-mio` 0.2: the `SIGUSR1` handler as a readiness source of the `mio` loop.
  - `mio` (already in the workspace, P1), `rustix` (`event` feature added, for the slow tests' readiness waits).
- **Hand-rolled, with reasons:** the `Worker` machine (it is the contract); the testkit bindings (they connect the machine to
  P6's in-memory edges).
