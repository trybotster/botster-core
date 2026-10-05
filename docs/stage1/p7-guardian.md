# P7 guardian machine

The `Guardian` in `botster-guardian-core` owns the service lifetime state.
It performs no I/O.
The real driver and the testkit driver use this same machine.
The guardian holds no service lane connection.
The host owns lane authentication, epochs, queues, and public events.

The driver supplies these inputs:

- Control bytes and the total bytes written on the current connection.
- The result of exec, including the payload identity and `SpawnReport`.
- Captured stdout, stderr, and the cooperative cause byte.
- The leader's exit status, without reaping the leader.
- The results of descendant enumeration, signals, group cleanup, and reaping.
- The monotonic time for each input and deadline.

The driver must keep the leader unreaped until `ReapService`.
The driver must verify each descendant identity before signalling that descendant.
`KillTree` includes the service group, retained descendants, and a current descendant enumeration.
The driver checks for live descendants after the kill.
The driver reports signal delivery and whether the leader already started to exit.
A cleanup signal must not claim an earlier exit.

`DrainLogs` reads the bytes present at the leader's exit.
The driver reads both output streams and the cause descriptor before `LogsDrained`.
A descendant that retains a pipe must not prolong this drain.
The guardian retains the newest `service_log_bytes` bytes.
The guardian returns a large log tail in consecutive bounded control frames.

The driver supplies validated positive orphan grace.
The launch supplies the service's exact startup and stop grace durations.
The startup deadline begins at successful exec.
The Stop path enumerates descendants before SIGTERM.
The Stop deadline begins at the injected time of the SIGTERM result.
Census and signal delivery delays cannot consume the service's stop grace.
The host sends `EpochCommitted` after it commits every lane of epoch 1.
Only authenticated adoption cancels orphan grace.
An unauthenticated connection does not extend orphan grace.

Removal waits for cleanup, reaping, and the written result.
Process termination and orphan expiry finish cleanup without waiting for host reads.
The guardian retains its payload identity, spawn report, exit, and log after a host disconnect.
The guardian never launches the payload twice.

## Prior art

The prior-art pass used old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` through `git show` only.
It read `runtime/plugin_process/launch.rs` and `runtime/plugin_process/supervisor.rs` under the old `crates/botster-core/src` tree.
It also read the pinned Stage 1 plan, the earlier rebuild plan, and the vault's process-group cleanup note.

This change reuses the existing `botster-core-link` frame decoder, hello codec, and token proof.
It uses the pinned contract types and workspace versions of serde and bolero.
It adds one crate: `botster-guardian-core`.
It adds no external dependency version.

This change copies no old source.
The old supervisor uses a thread, locks, and a real clock.
Those mechanisms cannot run in the required sans-IO machine.
The new machine preserves the old cleanup constraint: no signal after the leader is reaped.
The later real-edge change will assess reuse of the descriptor and rlimit code.

The guardian lifetime state is new contract logic.
A runtime or process library cannot own the contract's injected inputs and actions.
The machine uses existing codecs and contract types instead of creating alternate representations.
The command decoder uses serde and has a registered bolero harness.

## Verification scope

The focused tests cite the guardian's clauses.
They check authentication, exec results, startup, orphan grace, observed exit causes, log bounds, and cleanup ordering.
They check delayed census, delayed signal results, and exit before the exec result.
They check removal at the last result byte.
These tests prove the machine transitions.
They do not prove an OS signal, descriptor inheritance, or a real parent-death guarantee.
The later real-process tests must prove those conditions.
No conformance id leaves `conformance/core-pending.txt` in this change.
