# P3 worker review

Current verdict: NOT CLEAN for PR #163. F28 is OPEN only for completed native evidence. F25 is CLOSED at the head below. All other findings retain their recorded closures. All findings F1 through F24 remain CLOSED at their recorded heads and scopes.
Reviewed head: `ccba0504b8f0e18d274132e0b88b80b2e364becd`, branch `stage1/p3-audit-fixes`.
Round 78 closes F25. All source findings are closed. The HOLD still prevents the verification required for F28.
The cross-package PR also requires the integration reviewer's exact-head CLEAN and the implementer's landing gate.
Round 73's CLEAN remains preserved for M1 at `da2b0494bbda711e5a67cb180ddf05c607784635`.
M2a at `a7f4a386593457e3b30f03b56938092de9b060a3` has no restack verdict. The earlier M2a CLEAN below applies only to its named old head.

VERDICT: CLEAN

Reviewed head: `98960e434b0991ebb9d7e65c952f1c6ea116b83a`, branch `stage1/p3-worker-m2a`.
Previous reviewed head: `5a41a33fd65468dcddb9fe025e8f645743693e00`.
Round 11 closes F9 and F10. All eleven findings are CLOSED.
This CLEAN verdict covers M2a only. M2's terminal model, route admission, and the same-suite real-process proof remain later work.
The original evidence refers to `f37c46b`. Rounds 2 through 10 record review history. Round 11 records the latest delta verdict.
Base: `2016886`. Scope: M1, including the Worker machine, real driver, payload edge, and testkit driver.
This verdict covers both review units in the implementer's message.

Current authority: plan pin `2b03dc1b`, pair-common.md, brief-p3-worker.md, BUILD.md, and contracts-v0.1.9 (manifest final22).
The lead explicitly authorized contracts-v0.1.9, rulings R-19/R-20, and the stack on P1 `6db7924` on 2026-10-02.
The original review used plan pin `c43693ff` and contracts-v0.1.7 (manifest final21).
The reviewer inspected logic only. The reviewer did not run tests or a gate.
The implementer reported 276 default tests and 12 slow tests passing. That evidence does not close the findings below.

## F1 — HIGH — The testkit can consume the exit before the spawn result

Status: CLOSED at `37c96f1`.
The binding gates payload inputs behind Spawned. The machine also retains an early exit and applies it after Launched.

Evidence: `crates/botster-core-testkit/src/worker.rs:332-346,393` and `crates/botster-worker-core/src/worker.rs:489-494`.

After `SpawnPayload`, the testkit holds a `Spawned` result and a scripted program.
`ready()` polls the program's exit and offers `Ready::Exited` alongside `Ready::Spawned`.
The seeded scheduler can select `Exited` first.
`Worker::on_exited()` rejects that input because the machine is still `Spawning`, rather than `Live`.
The binding has already removed the exit from `self.exit`.
`ScriptedProgram::poll_exit()` reports each exit once, so the binding cannot recover it.
The later spawn result reports `Launched`, but the host never receives the payload's exit.

An immediate `signal_self(15)` or `exit(3)` in the EV-4 transcript reaches this case.
The real driver queues `Spawned` before it handles the exit watch, so the two drivers differ.

Required change: Preserve the dependency between the spawn result and payload inputs.
Deliver the successful spawn result before an exit that depends on that spawn, or retain the exit until the machine can accept it.
Check both immediate-exit cases under the scheduler's allowed orders.

Authority: EV-4, A5-1, A5-2, BUILD.md's one-code-path rule, and plan 2.1/4.1.

## F2 — HIGH — The worker loses a stop signal during spawn

Status: CLOSED at `37c96f1`.
The machine retains EndPayload, Stop, and Kill during spawn. It applies these requests on success and discards them on failure.

Evidence: `crates/botster-worker-core/src/worker.rs:355-381,533-552` and `crates/botster-worker/src/main.rs:146-163,178-187,199-202`.

`on_end_payload()` returns when the machine is `Spawning`.
The real driver can queue a launch and `EndPayload` in the same readiness turn.
The machine handles the launch and the driver starts the child.
The driver appends `Spawned` behind the already queued `EndPayload` input.
The machine then discards `EndPayload` and subsequently accepts `Spawned`.
No graceful request or grace deadline remains, so the payload continues running.
The testkit can produce the same order because `EndPayload` and `Spawned` can both be ready.

Required change: Retain an end request during `Spawning`.
Apply the request when the spawn succeeds, with the required graceful request and group kill.
Handle spawn failure without sending a signal to an unproven group.

Authority: LC-5, the brief's worker-control signal requirement, and the lead's P1 F7 decision.

## F3 — HIGH — A continuous PTY stream can prevent control and timer progress

Status: CLOSED at `37c96f1`.
The real driver reads one chunk per descriptor per turn and settles inputs between batches.
It retains readiness and uses a bounded exit drain. Continuous output no longer holds the read loop indefinitely.

Evidence: `crates/botster-worker/src/main.rs:146-177,264-311`.

`read_pty()` reads until `WouldBlock` and appends every chunk to the unbounded `inputs` queue.
The driver does not run the machine during that read loop.
A payload that continuously supplies bytes can prevent the loop from returning and can grow the queue without a bound.
Even a control request already read in the same turn waits until the PTY loop returns.
The driver also cannot handle `SIGUSR1` or a due grace timer during that loop.
`read_control()` uses the same unbounded collection pattern.

Required change: Bound the work and retained bytes of each driver turn.
Run the machine between bounded batches and preserve pending readiness when a batch reaches its bound.
Keep control requests and due stop deadlines runnable under continuous PTY output.

Authority: LC-5, EV-5(c), BUILD.md's deterministic scheduler requirement, and plan 2.4/2.5.

## F4 — HIGH — Closing a link either blocks the real worker or drops testkit reports

Status: CLOSED at `ca45f66`.
The drivers report cumulative bytes written. The machine compares those bytes with every queued send before it releases close.
An earlier write report cannot release a close over later bytes. Both drivers preserve nonblocking report delivery.

Evidence: `crates/botster-worker/src/main.rs:346-376` and `crates/botster-core-testkit/src/worker.rs:407-425`.

The real driver clears `O_NONBLOCK` and calls `write_all()` before closing the link.
A connected host that stops reading can block this call indefinitely.
`LinkClose` can result from an oversized frame while a payload still runs.
In that case, the blocked worker cannot serve its PTY, worker-control signal, or grace timer.
Removing the socket from the readiness loop does not remove those other responsibilities.

The testkit instead calls `flush()` once and closes its endpoint even when `outbound` still holds bytes.
`Action::Exit` clears those remaining bytes through `ended()`.
If the bounded link is full, a queued `RemoveResult` or preceding report can disappear on a healthy link.
The real and testkit drivers therefore have different report-delivery behavior under backpressure.
The testkit also leaves write interest false and performs sends inside its readiness checks.

Required change: Stage close and exit until preceding reports have drained, using nonblocking writes and explicit write readiness.
Represent the same pending close behavior in both drivers.
Keep the worker's other inputs runnable while a live host does not read.
Treat a failed transport separately from a transport that temporarily accepts no bytes.

Authority: A5-1/A5-4, LC-5, LC-7, A6-3's complete cleanup result, and plan 2.5/3.

## F5 — MEDIUM — A failure after child spawn bypasses payload cleanup

Status: CLOSED at `37c96f1`.
The edge sets nonblocking mode before it spawns the child. Payload owns the child immediately after successful spawn.

Evidence: `crates/botster-core-sys/src/payload.rs:94-111`.

The edge spawns the child before it sets the PTY master to nonblocking mode.
If `set_nonblocking()` fails, `?` returns before the edge constructs `Payload`.
Dropping a `std::process::Child` neither kills nor reaps that child.
The cleanup in `Payload::drop()` therefore cannot run on this path.
Core receives a spawn failure even though a payload can remain running or unreaped.

Required change: Set the master flags before spawning the child, or establish the cleanup owner immediately after successful spawn.
Every subsequent failure must kill the proven payload group and reap its leader in the required order.

Authority: LC-4, the brief's F7 cleanup requirement, and BUILD.md testing rule 10.

## F6 — MEDIUM — The slow tests do not clean up every failure path

Status: CLOSED at `ca45f66`.
The tests have a registry-based cleanup fallback after Core drops, and OwnedWorker requests cleanup through the worker.
That request covers payload launch before the test reads Launched. F7 separately tracks the remaining identity race in the fallback.

Evidence: `crates/botster-worker/tests/slow_session.rs:179-195,260-270`.

`Sessions::drop()` attempts only `Remove`.
Core refuses `Remove` synchronously while a session is `Starting`, `Running`, or `Stopping`.
A panic before the payload exits therefore skips cleanup and leaves the worker and payload running.
This includes a failed Start wait or a failed assertion after Start succeeds.

`OwnedWorker::drop()` kills only the worker process.
The payload belongs to a separate session and process group, so killing the worker does not guarantee payload cleanup.
That guard can leave the payload behind if the control-signal test fails after launch.

Required change: Give each test a cleanup owner that covers its worker and proven payload group on every exit path.
Stop active sessions before Remove, and retain a cleanup path when the Core operation fails.
Preserve the rule against signalling an unproven id.

Authority: BUILD.md testing rule 10, pair-common.md's process ownership rule, and plan 5/10 R12.

## Scope limits and accepted structure

- The Worker machine contains no clock reads, threads, filesystem calls, or test behavior branches.
- The hello checks the token proof, instance, and host epoch before accepting Launch.
- Launch remains the sole action that starts the payload. P1 owns the preceding durable identity write.
- The normal signal/reap path retains the payload leader until the group-kill action precedes reaping. No signal follows `Reaped`.
- DESIGN.md records prior art, rejected mechanisms, added libraries, and the reported lead decision for the waiter thread.
- The marked terminal placeholder, input encoding, admission, terminal events, reads, snapshots, tap, and resize remain M2 scope.
- The startup timeout and adoption remain later work as stated in the handoff. This verdict does not accept their contract ids.
- The contracts pin move and P1's reported R-20 change remain outside this exact diff. A later head requires a delta review.

All open findings must close before CLEAN. No LOW finding is exempt from closure.

## Round 2 — Review history

This review inspected the complete delta at `37c96f1`. The reviewer ran no tests or gate.
The implementer reported 281 default tests and 12 slow tests passing on macOS.

### F4 — HIGH — A stale flush report can release a later unsent report

Round 2 status: OPEN. Closed in round 3, as recorded above.

Evidence: `crates/botster-worker/src/main.rs:198-206,212-215,380-382` and `crates/botster-worker-core/src/worker.rs:619-623`.

The blocking close is removed, and the testkit now stages close correctly.
The real driver still reports flush completion through a queued boolean with no association to the bytes it covers.
Consider this order:

1. The machine emits two reports and stages a close in one input, such as pending read replies followed by RemoveResult.
2. The driver sends the first report completely and queues `LinkFlushed{drained: true}`.
3. The driver processes the next LinkSend before it handles that queued flush input.
4. The socket accepts no more bytes, so the later report remains in `outbound`.
5. The machine handles the first flush input and clears `unflushed` for all reports.
6. The machine emits LinkClose and Exit. The driver discards the later report in `drop_link()`.

`settle()` performs every queued action before it handles the next input, so this order is permitted by the actual driver.
The close protocol therefore still loses a report under backpressure.

Required change: Make a flush completion identify the sends that it covers, or deliver the completion only for the current queue state.
An earlier completion must not authorize closing over bytes queued after that completion.
Check multiple sends in one machine step with the socket becoming full between those sends.

Authority: LC-7, A6-3, A5-1/A5-4, and plan 2.5/3.

### F6 — MEDIUM — Cleanup still depends entirely on successful Core progress

Round 2 status: OPEN. Closed in round 3, as recorded above.

Evidence: `crates/botster-worker/tests/slow_session.rs:180-207,358-413`.

Stopping before Remove fixes the normal cleanup of a running session.
The guard still skips refused operations and catches a cleanup timeout without a process-owner fallback.
If Stop fails to complete, Remove can remain WrongState and the guard leaves the worker running.
If Remove fails to complete, dropping Core preserves the worker by LC-12.
These are precisely the failure paths that a lifecycle test must clean up.

OwnedWorker also holds no payload identity until the test reads Launched.
A failure after the child starts but before the test records that report leaves `payload: None`.
The guard then kills only the worker, which does not guarantee cleanup of the separate payload group.

Required change: Keep a cleanup path that does not depend only on the Core behavior under test.
Cover a failed or timed-out Stop, a failed or timed-out Remove, and launch before the test records Launched.
Use proven process ownership or a request to the owned worker. Do not signal a cached payload id without current proof.

Authority: BUILD.md testing rule 10, pair-common.md's process ownership rule, and plan 5/10 R12.

### F7 — HIGH — The new test guard can signal a reused payload group id

Round 2 status: OPEN. New finding in `37c96f1`. The direct payload signal is removed in round 3; F7 remains open below.

Evidence: `crates/botster-worker/tests/slow_session.rs:279-290,418-435` and `crates/botster-worker-core/src/worker.rs:565-569`.

OwnedWorker caches the payload pid from Launched.
Its drop sends SIGKILL to that cached group whenever the worker process remains alive.
Worker liveness does not prove that the payload leader remains unreaped.
After the SIGUSR1 grace, the worker kills and reaps the payload leader while the worker itself continues running.
The group id can then be reused.
A panic after the exit report, or during the Remove exchange, makes the guard signal that potentially unrelated group.
Even receiving the exit report is not a safe boundary because sending the report and reaping the leader can precede the test's read.

Required change: Remove the assumption that a live worker reserves every payload id it ever reported.
Request cleanup through the owned worker, which knows whether its leader remains unreaped, or use an ownership mechanism that prevents reuse.
Do not retain the direct killpg path with only `worker.try_wait()` as proof.

Authority: the lead's P1 F7 decision, the explicit rule against signalling an unproven id, and pair-common.md's process ownership rule.

## Round 3 — Review history

This review inspected the complete delta at `ca45f66`. The reviewer ran no tests or gate.
The implementer reported clean clippy checks, 284 default tests, and 13 slow tests passing on macOS.

### F7 — HIGH — The cleanup waiter releases the id before the last possible signal

Round 3 status: OPEN. Closed in round 4, as recorded below.

Evidence: `crates/botster-worker/tests/slow_session.rs:112-116,125-138`.

The direct signal to a cached payload group is removed.
SIGTERM now asks the worker to kill only its held payload group, reap the leader, and end.
That change fixes the original payload-id path.
The shared `end_child_worker()` helper introduces the same reservation error for the worker id:

1. The helper starts a thread that calls blocking `waitpid()`.
2. The main thread's `recv_timeout()` reaches its deadline without receiving the thread's result.
3. The worker exits and the waiter reaps it, releasing the worker id for reuse.
4. The main thread sends SIGKILL to that id.

Steps 2 through 4 can also occur when the waiter reaps before the deadline but pauses before it sends the channel result.
A missing channel result does not prove that the child remains unreaped.
The helper can therefore signal an unrelated process after the worker id is reused.

RowReaper's non-matching-identity path also calls `waitpid(pid, NOHANG)` on that unproven id.
Another test in the same test process can own a child that reuses that id.
In that case waitpid can reap the other test's child; ECHILD is not guaranteed.

Required change: Keep the worker unreaped until every possible signal has completed.
Use an exit observation that retains the child, such as `waitid(WNOWAIT)`, and let the cleanup owner reap after the signal decision.
Do not use receipt of a channel message as proof that the id remains reserved.
Do not reap a non-matching id unless a separate child-ownership record proves it belongs to this cleanup owner.

Authority: the explicit rule against signalling an unproven id, the lead's P1 F7 reservation principle, and BUILD.md testing rule 10.

## Round 4 — Closure on the previous base

F7 status: CLOSED at `306b143a8c75d8cd8024a8f8125c264163648c34`.

The exit observer now uses `waitid(P_PID, WEXITED | WNOWAIT)` and does not reap the child.
The cleanup owner completes its SIGKILL decision, joins the observer, and then reaps with waitpid.
The worker id therefore stays reserved through the last possible signal, including a timeout before the observer's channel result arrives.
RowReaper no longer signals or waits for a non-matching identity.
The cleanup guard no longer signals a cached payload group id.

The reviewer inspected the complete delta, which changes only the slow test cleanup file.
The reviewer ran no tests or gate. The implementer reported clean clippy checks and 13 slow tests passing on macOS.

Round 4 verdict: CLEAN on `306b143a8c75d8cd8024a8f8125c264163648c34`. Every finding was closed on that head.
A rebase, contracts pin move, or any other later commit requires a delta review before this verdict applies to that head.

## Round 5 — Review history

The range-diff `2016886..306b143` against `6db7924..dbc4957` shows unchanged P3 patches except dependency and module context.
The added commit changes TestkitCore's attach return type to AttachRefused and adds the EV-4 and LC-5 transcript selections.
Attach delegates directly to the host driver, so the wrapper preserves R-19's returned transport on refusal.
All five selected transcripts remain pending for the real-process harness, as the plan requires.
R-20 and the host's fixed-timing change are compatible with the worker's existing stop path.

The reviewer inspected the rebase, the new commit, and the changed base behavior that affects P3's cleanup assumptions.
The reviewer ran no tests or gate.
The implementer reported clean clippy checks, 291 default tests, five transcripts on 32 seeds, and 14 slow tests passing on macOS.

### F7 — HIGH — The base reaper can release the worker id during fallback cleanup

Round 5 status: REOPENED at `439e5e14c6fe55b3331d545c254dc11a8baf1ed8`. Closed in round 6 below.

Evidence: `crates/botster-core-sys/src/process.rs:119-134` and `crates/botster-worker/tests/slow_session.rs:81-83,114-115,130-151`.

P1's new Children::spawn transfers Child to an independent thread that calls `child.wait()` and reaps the worker.
That thread owns cloned state and continues after Core drops.
RowReaper still assumes that dropping Core leaves nobody else able to reap the worker.
That assumption no longer holds.

The reaper can reap the worker after RowReaper verifies its start time but before end_child_worker sends SIGTERM.
The id can then be reused before that signal.
The same reaper can release the id between the cleanup timeout and SIGKILL.
The cleanup observer's WNOWAIT cannot reserve a child that another thread reaps.
The final cleanup waitpid can also act on a reused id after the independent reaper finishes.

OwnedWorker's separately owned Child path has no independent P1 reaper and remains compatible with the round 4 fix.

Required change: Coordinate fallback cleanup with the child owner that can reap the worker.
That owner must keep the worker unreaped through every possible signal and perform the final reap itself, or transfer exclusive ownership before cleanup.
Remove RowReaper's unchecked assumption about ownership after Core drops.
A second start-time check does not remove the check-to-signal race.
Preserve cleanup of active sessions and the prohibition against signalling or reaping an unproven id.

Authority: AD-6, the explicit rule against signalling an unproven id, the lead's P1 F7 reservation principle, and BUILD.md testing rule 10.

Round 5 verdict: NOT CLEAN (1 open) on `439e5e14c6fe55b3331d545c254dc11a8baf1ed8`.

## Round 6 — Review history

F7 status: CLOSED at `a855586248de777d553352039bcec915e307a0b0`.

The slow tests now start each real worker directly and retain its Child in OwnedWorker.
The tests no longer use the independent P1 reaper or RowReaper.
The WNOWAIT observer and final reap therefore have exclusive ownership of the worker throughout cleanup.
The tests never signal a cached payload id.

The reviewer inspected the complete delta, including the rewritten slow tests and removed development dependencies.
The named tests now prove the worker and its real payload edge through the control link.
They do not prove real Core's host lifecycle path; the same-suite real-process proof remains pending under plan 4.2.
The reviewer ran no tests or gate. The implementer reported clean clippy checks and 14 slow tests passing on macOS.

### F8 — MEDIUM — The test link discards a second frame in the same socket read

Round 6 status: OPEN. New finding exposed by the rewritten tests at `a855586`. Closed in round 7 below.

Evidence: `crates/botster-worker/tests/slow_session.rs:137-162` and `crates/botster-core-link/src/frame.rs:137-149,184-193`.

Link::frame reads a socket chunk and pushes it into FrameDecoder.
FrameDecoder stops consuming bytes when its first frame is complete.
If that chunk also contains another frame, the next push returns zero because the first frame has not been taken yet.
The helper breaks the loop and drops the unconsumed remainder of the socket chunk.
The next frame() call retrieves the first frame, but the second frame is already lost.

The new immediate-exit tests can receive Launched and Exited in one socket read.
The Signal test can receive Done and Exited in one socket read.
Both are valid stream chunkings, so passing runs do not establish that the helper preserves the worker's reports.
A coalesced read can make these tests wait for a report they discarded themselves.

Required change: Retain every unread byte or decode and queue every complete frame from each socket read.
Keep partial bytes for the next read as well.
Check the helper with two complete frames in one chunk and a complete frame followed by part of the next frame.
Do not depend on the kernel delivering one worker report per read.

Authority: EV-4, LC-6, BUILD.md's structural test rules, and the plan's real-process proof requirement.

Round 6 verdict: NOT CLEAN (1 open) on `a855586248de777d553352039bcec915e307a0b0`.

## Round 7 — Closure of F8

F8 status: CLOSED at `0ab2a7dc1d0f49a89931432dba51b956ca0fdb3b`.

Link now retains the bytes that FrameDecoder has not consumed.
Each frame() call pushes those bytes, removes only the consumed prefix, and returns the next complete frame.
The helper reads more bytes only when the decoder needs them.
This preserves complete frames and partial frames across socket reads.
The added regression test sends two complete frames and a partial third frame, then completes the third frame in another write.

The reviewer inspected the complete delta, which changes only the slow test helper and its regression test.
The reviewer ran no tests or gate. The implementer reported clean clippy checks and 15 slow tests passing on macOS.

Round 7 verdict: CLEAN on `0ab2a7dc1d0f49a89931432dba51b956ca0fdb3b`. All eight findings are closed.
Any later commit, including a rebase, requires a delta review before this verdict applies to that head.

## Round 8 — Documentation delta

The corrected review head is `1911ed09654260e7a57ae0a5b2a251ff470ad330`. The implementer withdrew the earlier stated head `32cbde5`.
The delta changes only `crates/botster-worker-core/DESIGN.md`.
It replaces banned old mechanism names with references to the plan's prior-art rows.
The note retains the prior-art sources, rejection reasons, library choices, and the statement that no code was stolen.
The old binary's exact path remains referenced through the PR's Prior-art note.
The delta changes no code, dependency, or test behavior. No new finding exists.

The reviewer inspected the complete delta and ran no tests or gate.
The implementer reported that the previous head's Linux gate passed fmt and clippy, then failed the taint check.
The implementer reported local taint, lists, and public-api checks passing on this fix head. This verdict does not establish a green gate.

VERDICT: CLEAN on the exact M1 head above. All eight findings remain closed.
Any later commit, including a rebase, requires a delta review before this verdict applies to that head.

## Round 9 — M1 stack and transferred testkit wiring

Reviewed head: `46b16945ead49715949d5983bb41a673c081f8ad`, on P1 base `823a1f1` and v1 base `67124ec`.
The review covers all four units in the implementer's message:

- `befe0ff`: the transferred P1 testkit wiring, including the host edges, directory storage, wake object, and harness construction.
- `7c69586..eb9b3d1`: the replay of the eleven previously reviewed M1 commits.
- `f60e7d9`: the restored candidate module and refusal controls.
- `46b1694`: the lock file correction for v1's rustix and schemars dependencies.

The transferred wiring matches P1's `95a5854` for the testkit source and manifest files.
The final harness constructs the real HostDriver with in-memory edges and the real Worker with the scripted program edge.
The host and workers share the scheduler of the run. The TestkitCore wrapper delegates the Core API to the host driver.
The directory lock ends when the host edges drop. The registry remains in the harness after that drop.
The marked terminal identity placeholder and unsupported controls remain later work. This review does not claim adoption or M2 conformance.

The range-diff compares `6db7924..1911ed0` with `befe0ff..eb9b3d1`.
Nine commits match exactly. The other two differ only in patch context from v1's dependencies and testkit interfaces.
The v1 program control handle retains the existing blocked-write behavior. The replay preserves all eight finding closures.

The final harness restores the refusal map, fail_next control, and RefusalLayer around each opened Core.
The existing v1 refusal layer retains the caller's transport on an attach refusal, as R-19 requires.
The candidate module remains public. The lock correction restores declared dependencies without changing their versions.
No new finding exists.

The reviewer inspected logic only and ran no tests or gate.
The implementer reported clean clippy, taint, lists, ledger, and public-api checks.
The implementer reported 442 default tests and 15 slow tests passing on macOS.
This verdict does not establish a green Linux gate or the same-suite real-process proof.

VERDICT: CLEAN on the exact M1 head above. All eight findings remain closed.
Any later commit, including a rebase, requires a delta review before this verdict applies to that head.

## Round 10 — M2a host input and testkit controls

Reviewed head: `5a41a33fd65468dcddb9fe025e8f645743693e00` on P1 `3512c68`.
The range-diff shows all fourteen M1 stack commits unchanged: `823a1f1..46b1694` equals `3512c68..3cb011c`.
The reviewer inspected `af4abdd`, `db1c1d4`, the proof list at `6621709`, the test-only delta at `0cbb064`, and the testkit fixes at `5a41a33`.
The admission state retains transaction ownership across short writes. Guards run at transaction start.
Cancellation waits for an outstanding write count. The completion reports exact counts.
Bytes and Text are implemented. The other payload kinds explicitly wait for the libghostty model.
The pending conformance list remains unchanged. The final 26 transcript ids are testkit proof only.

### F9 — HIGH — Controls can target another handle's worker

Status: OPEN.

Evidence at the reviewed head:
`crates/botster-core-testkit/src/worker.rs:102-112,137-169,239-246,403`;
`crates/botster-core-testkit/src/core.rs:339-369`;
`crates/botster-core-host/src/engine.rs:221-224`.

Workers shares its programs and worker_processes maps across every handle. Both maps use only InstanceId as the key.
HostEngine mints InstanceId from the directory's host epoch and a handle-local counter.
Two new directories each start at epoch 1. Their first sessions each receive InstanceId("1-1").
The second worker replaces the first worker's entry in both shared maps.
The sessions map includes the handle, but its lookup returns the colliding InstanceId and then reads the shared map.

Open handles A and B on different new directories. Create and start one held payload in each handle.
After B starts, pty_blocked for A changes B's payload. pty_input for A returns B's input log.
process_end_worker for A ends B's worker. break_control for A breaks B's link.
These controls no longer inject a fault into the requested session's edge.

Required change: Include the data-directory or host namespace in the shared worker and program keys.
Keep that namespace stable for the lifetime and restart behavior that the harness supports.
Do not rely on InstanceId being unique across different directories.
Check program controls and process controls with two live handles on different directories, each with its first session.

Authority: A5-1, A5-3, ID-1, and plan 4.1's shared Sim with the real machines.

### F10 — HIGH — The real write path has no turn bound

Status: OPEN.

Evidence at the reviewed head:
`crates/botster-worker/src/main.rs:135-174,212-222,239-255`;
`crates/botster-worker-core/src/worker/input.rs:233-287`.

settle drains actions and inputs until both queues are empty.
Each successful short PTY write queues PtyWritten. Handling that count immediately emits the next PtyWrite.
This cycle continues inside settle until the entire active transaction, and queued transactions, finish or block.
The cycle has no write-count or byte budget for the turn.
The Interrupted retry loop inside PtyWrite also has no turn bound.

While a large transaction receives repeated positive short writes, the driver does not return to read_control or poll.
A newly arrived Cancel, Stop, SIGUSR1, or payload-exit notification therefore waits for the write cycle.
A stop grace deadline that becomes due inside that cycle also waits for the outer loop's timer check.
IN-5's retained-byte limit bounds storage. It does not bound driver work before the next control or timer check.
The testkit can select other ready inputs between write attempts, so the real driver lacks the same progress opportunity.

Required change: Bound PTY write work in each real driver turn.
Retain transaction ownership and pending write work across turns.
Return to control, signal, exit, and timer handling between bounded write batches.
Bound Interrupted retries as well. Preserve exact counts and contiguous input when a turn yields.
Check repeated short writes with a pending cancel and a due stop deadline.

Authority: LC-5, IN-6, the closure requirement of F3, and plan 2.4's control-first worker scheduling.

### F11 — LOW — The testkit reports runnable work after it finishes that work

Status: CLOSED at `5a41a33` during this review.
The wrapper now uses remaining readiness and does not use the completed-input count.
The evidence below describes the superseded head `0cbb064`.

Evidence at `0cbb064`: `crates/botster-core-testkit/src/worker.rs:657-669`.

TestkitCore::pump forces more = true whenever Workers::run handled any input.
It does this even when the host driver reports more = false and Workers::has_ready returns false.
For example, consume the last injected PTY output on an otherwise idle held payload.
The current machine advances model_rev and emits no action for that output.
Both drivers have no runnable work after the pump, but the wrapper reports more and sets the wake flag.
The caller must perform an extra empty pump to clear that flag.

Required change: Report more from remaining runnable work, rather than from work completed in this pump.
If an effect needs a later pump, represent that effect as pending work and include it in the readiness check.
Check that a pump which consumes the last input returns more = false when no runnable work remains.

Authority: TM-6, A5-1, and plan 2.5's rule against wake loops for work that is not runnable.

The reviewer inspected logic only and ran no tests or gate.
The implementer reported clean clippy, 420 default tests, 15 slow tests, and 27 transcript ids passing on all 32 seeds.
The implementer also supplied test-only pty_chunk cases and P6 review fixes. The reviewer inspected both deltas.
The P6 fixes remove the completed-work flag, preserve a link on WouldBlock, and distinguish lose_worker reasons.
Sim::with_seed delegates to with_scheduler. No new finding exists in those changes.
The cancel-race transcript left the proof list and remains pending. The implementer raised its driver behavior with the lead.
The implementer reported 200 testkit and worker-core tests, and 26 transcript ids passing on all 32 seeds at `5a41a33`.
Reported tests do not close F9 or F10.

VERDICT: NOT CLEAN (2 open findings) on the exact M2a head above.
All findings, including LOW findings, require closure before CLEAN.

## Round 11 — Closure of F9 and F10

Reviewed head: `98960e434b0991ebb9d7e65c952f1c6ea116b83a`.
Delta: one commit after `5a41a33`, in the testkit harness, testkit worker driver, and real worker driver.

F9 status: CLOSED at this head.
WorkerKey includes the data directory and InstanceId. Both shared maps use WorkerKey.
The spawner carries the data directory into the worker and program entries.
TestkitCore carries the same data directory into each session lookup recorded from SessionState.
Program controls and process controls use that complete key.
Separate directories no longer share entries when both mint InstanceId("1-1").
The regression test starts the first session in each of two directories and checks that injected output reaches only the requested payload.
The reviewer checked both control lookup paths in the code.

F10 status: CLOSED at this head.
The real driver retains PtyWrite as pending work. settle no longer performs the PTY write.
The outer loop reads control input and checks the stop deadline before it calls write_pty_once.
write_pty_once performs at most one write attempt in the turn.
An Interrupted result retains the bytes for the next turn and does not retry inside the function.
A pending write keeps the poll timeout at zero. A blocked write waits for write readiness.
The machine retains transaction ownership and counts each write result before it issues the next fragment.
Control, signal, exit, and timer inputs can progress between fragments.

The reviewer inspected the complete delta and ran no tests or gate.
The implementer reported clean clippy, 201 default tests, and 6 worker slow tests passing.
No new finding exists. F1 through F11 are CLOSED.

VERDICT: CLEAN on the exact M2a head above.
The terminal model and the remaining M2 features remain later work. The same-suite real-process proof remains pending.
Any later commit, including a rebase, requires a delta review before this verdict applies to that head.

## Round 12 — M1 restack onto merged P1

Reviewed head: `049751aed4b97fc7b817c0201119c0e363a73935`.
Base: `1d25d093301072bd0c13a122c68d8f1b1ca0815c`.
Previous M1 head: `46b16945ead49715949d5983bb41a673c081f8ad`, CLEAN at verdict commit `30bb483`.
Authority: plan pin `bdda2359`, the restack brief, pair-common.md, BUILD.md, and contracts-v0.1.13, including R-28 and erratum 2.

The implementer reports no conflicts and no conflict resolutions.
The range-diff compares `823a1f1..46b16945` with `1d25d09..925f627`.
Twelve patches match exactly. Two differ only in Cargo.lock context.
The reviewed Worker machine, real driver, payload edge, and testkit wiring remain unchanged from M1.
The candidate module and RefusalLayer remain present. Earlier M1 finding closures remain intact.
The merged P1 code retains its Resize forwarding order and its check for an in-flight Resize before the same-size shortcut.
M1 does not implement worker Resize handling. The `sz_*` conformance proof remains M2 work.

The new commits add a test guard inside the payload session and a worker-SIGKILL plus panic test.
The guard signals its own current group. It does not signal a cached group id or reap the production payload.
The helper blocks on sockets and contains no busy-spinning child loop.
However, the guard can die before it performs cleanup, as F12 records below.

This PR adds no mutation exclusion. `.cargo/mutants.toml` matches the merged base.
The inherited entries identify individual functions and give reasons. This delta does not establish a mutation result.
No terminal expectation or transcript changes in this delta. The terminal model, R-28 input behavior, and A13 remain later M2 work.

### F12 — HIGH — SIGTERM can remove the independent payload guard

Status: OPEN at the reviewed head.

Evidence: `crates/botster-core-sys/tests/common/payload_guard.rs:92-109`;
`crates/botster-worker/tests/slow_session.rs:290-303,356-369`;
`crates/botster-core-sys/tests/slow_payload.rs:174-181`.

The guard member handles SIGHUP only. SIGTERM retains its default action, which ends the member.
The prefix starts the member before the test script installs its TERM trap.
The member therefore does not inherit the payload's later decision to ignore TERM.

The existing Stop and worker-control-signal tests send SIGTERM to the whole payload group while the payload ignores TERM.
That signal ends the guard member while the payload remains alive.
If production cleanup then fails, a panic drops a guard whose member has already died.
Closing the member's socket cannot make the dead member kill the surviving group.
The test again depends on production cleanup. The new broken-cleanup test does not cover this path because it sends no payload SIGTERM first.

Required change: Keep independent cleanup available after a graceful group signal that the payload can ignore.
Preserve the protection against group-id reuse. Do not reap a payload that production reaps.
Add failure-path proof with the graceful group signal before worker cleanup becomes unavailable and before the test panics.
The proof must show that the test guard ends the surviving payload group.

Authority: the restack brief's independent group guard requirement, its broken-cleanup test requirement, and BUILD.md testing rule 10.

The reviewer inspected logic only. The reviewer ran no tests or gate.
The implementer reports that the focused Linux job on `88f7bfe` failed before tests because its command omitted the required parallelism variables.
No test result exists for that job. The implementer plans a corrected focused job on the review head.
This verdict covers P3's package scope. M1 also requires the integration reviewer's verdict.

VERDICT: NOT CLEAN (1 open finding) on the exact M1 head above.
Every finding, including LOW findings, must close before CLEAN.

## Round 13 — Closure of F12

Reviewed head: `ed92707f51f80ddd31f3b3c9fb88f4151e94cdbb`, PR #136.
Previous reviewed head: `049751aed4b97fc7b817c0201119c0e363a73935`.
The complete delta changes the guard's signal handlers, the broken-cleanup test, and the handoff.

F12 status: CLOSED at this head.
The member registers handlers for both SIGHUP and SIGTERM before it connects and registers with the guard.
The shell waits for registration before it runs the payload script.
A graceful SIGTERM therefore does not remove the member that performs independent cleanup.
The member still signals its own current group on socket EOF. The guard does not reap the production payload.

The broken-cleanup test installs HUP and TERM traps in the payload and waits for the payload's FIFO readiness message.
The test sends `Op::Signal(Term)` and requires its successful `Done` report.
The test then kills the worker with SIGKILL and waits for the worker's exit.
The FIFO remains held before cleanup. A caught panic drops the guard, and FIFO EOF proves that the payload released the FIFO.
The test retains its marked deadline. No child busy-spin or cached group signal is added.

The handoff distinguishes the failed first focused job from the corrected job on the previous head.
The implementer reports 37 worker-core units, five M1 transcript ids, five slow_payload entries, and nine slow_session entries passing at `049751a`.
That evidence predates this fix. The implementer later reports five slow_payload entries and nine slow_session entries passing on this exact head.
The reported entries include the graceful SIGTERM, worker SIGKILL, and panic case.
Reported log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-ed92707f-linux-20261004-124906-8719.log`.
The reviewer inspected logic only and ran no tests or gate. This verdict does not establish test success or a green gate.

No new finding exists. F1 through F12 are CLOSED at their recorded scope and heads.
This CLEAN covers P3's package scope for restacked M1 only. The integration reviewer must also clear this exact head.
M2a, the terminal model, the remaining M2 features, and the same-suite real-process proof remain later work.

VERDICT: CLEAN on `ed92707f51f80ddd31f3b3c9fb88f4151e94cdbb`.
Any later commit requires a delta review before this verdict applies to that head.

## Round 14 — Current v1 and the pinned terminal identity

Reviewed head: `4de71bbf5dcb5d95ff3b9ad12a4b797a7166ae61`, PR #136.
Previous reviewed head: `ed92707f51f80ddd31f3b3c9fb88f4151e94cdbb`.
The review covers merge `1a49733`, identity change `9a6099d`, and pending-list comments at `4de71bb`.

The implementer reports no conflicts and no conflict resolutions in the merge of `v1` at `393f403047beb1583ab3a711afbe2c37255c7e64`.
That v1 head is an ancestor of the reviewed head.
The merged change wires real Core's terminal identity to `botster_terminal_ghostty::terminal_identity()`.
The follow-up uses that same function when TestkitCore constructs EngineConfig.
Both drivers pass the returned term and terminfo source without substitutes.
The binding reads those values from libghostty. The delta adds no terminal parser or hand-written terminal expectation.
This follows TI-1 and BUILD.md's requirement to use the real terminal component.

The new binding dependency appears in both the testkit manifest and Cargo.lock.
The testkit proof list adds `conf::a2_8_terminal_identity_names_the_entry`.
All three `a2_8` ids remain pending. Each comment names the remaining proof or control.
The lead's reported instruction requires both harnesses to pass before an id leaves pending.
This verdict does not establish the identity transcript's success or the remaining TI-1 proof.

The Worker machine, real worker driver, payload edge, and F12 guard fix do not change.
The delta adds no mutation exclusion or test branch.
No new P3 finding exists. Earlier findings remain closed at their recorded scope and heads.
The integration reviewer owns its I1 and I2 closure decisions. This source delta includes both requested changes.

The reviewer inspected the complete delta and ran no tests or gate.
The implementer reports a focused identity and lifecycle job running on code head `9a6099d`.
Only pending-list comments change after that code head. No result was supplied with the review request.
This CLEAN covers P3's package scope for restacked M1 only.
The integration reviewer must also clear this exact head. M2a and M2b remain later reviews.

VERDICT: CLEAN on `4de71bbf5dcb5d95ff3b9ad12a4b797a7166ae61`.
Any later commit requires a delta review before this verdict applies to that head.

## Round 15 — TI-1 proof documentation

Reviewed head: `0044bf7346ca486bf09b5265dbfc0c75f93dd978`, PR #136.
Previous reviewed head: `4de71bbf5dcb5d95ff3b9ad12a4b797a7166ae61`.
The complete delta changes one pending-list comment and the handoff only.

The identity comment now states that the testkit passes and that RealCoreHarness proof remains pending.
The handoff records the implementer's reported Linux exit 0 on code head `9a6099d4125bdf2a23db0290c506507fc0f3509f`.
The identity transcript and five M1 lifecycle transcripts passed in that reported job.
Reported log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-9a6099d4-linux-20261004-125359-14517.log`.
Only documentation changes after that code head. All three `a2_8` ids remain pending.
The environment and tic comments retain their separate remaining controls.

The delta changes no code, dependency, test behavior, transcript, or mutation exclusion.
No new finding exists. Every earlier finding and closure remains preserved.
The reviewer inspected the complete delta and ran no tests or gate.
This CLEAN covers P3's package scope for restacked M1 only. Integration must clear the same exact head.
M2a and M2b remain later reviews. This verdict does not establish a full green gate or real-harness TI-1 proof.

VERDICT: CLEAN on `0044bf7346ca486bf09b5265dbfc0c75f93dd978`.
Any later commit requires a delta review before this verdict applies to that head.

## Round 16 — Deadline comment correction

Reviewed head: `78b88fa99546059dc5d8500af890432edd855537`, PR #136.
Previous reviewed head: `0044bf7346ca486bf09b5265dbfc0c75f93dd978`.
The complete delta moves the deadline comment immediately before `recv_timeout` and updates the handoff.
The executable code and ten-second timeout remain unchanged.
The marked deadline retains its reason: the payload must release the FIFO after test cleanup.
No new finding exists. All earlier findings and closures remain preserved.

The implementer reports that the full Linux gate on the previous head exited 1 at the timer check.
Formatting and clippy passed. Tests, mutation, and fuzz did not run in that gate.
Reported log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-0044bf73-linux-20261004-130018-20279.log`.
The handoff records that failure and does not claim later gate steps passed.
The correction addresses the reported comment placement defect in source.
The reviewer inspected logic only and ran no tests or gate.
This verdict does not establish a successful timer check or a green gate on the new head.

VERDICT: CLEAN on `78b88fa99546059dc5d8500af890432edd855537`, P3 package scope for M1 only.
Integration must clear this same exact head. Any later commit requires a delta review.

## Round 17 — Mutation gate findings

Reviewed head: `78b88fa99546059dc5d8500af890432edd855537`, PR #136.
The implementer reports a failed full Linux gate and has not requested a new source review.
The reviewer read the gate summary, outcomes.json, and missed.txt. The reviewer ran no gate.

### F13 — MEDIUM — The mutation gate leaves 167 mutants unclosed

Status: OPEN on the reviewed head. This finding supersedes round 16's CLEAN.
F1 through F12 retain their recorded closures.

Evidence: gate log `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-78b88fa9-linux-20261004-130525-23750.log`.
Artifact root: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004130525-23750/target-mutants.out/`.
The complete missed list is preserved in `verdicts/p3-worker-mutants-78b88fa.txt`.
Every entry in that list requires closure under F13.

The result contains 376 mutants: 139 caught, 167 missed, 0 timeouts, and 70 unviable.
The missed mutants are distributed as follows:

- `botster-worker-core/src/worker.rs`: 5.
- `botster-worker/src/main.rs`: 65.
- `botster-core-sys/src/payload.rs`: 26.
- `botster-core-testkit/src/core.rs`: 28.
- `botster-core-testkit/src/harness.rs`: 1.
- `botster-core-testkit/src/worker.rs`: 42.

The gate reports PASS for formatting, clippy, taint, lists, public-api, worker prebuild, default test budget, and slow tests.
Mutation reports FAIL. Fuzz reports NOT RUN because mutation failed.
Passing earlier stages does not close a missed mutant.

Required change: Close every missed mutant with a test or a reviewed equivalent-mutant argument.
Each proposed exclusion must cover one function and give its reason.
A claim that real-process code needs slow-tier proof must name the proof and its result.
Retain the independent group guards and production reaper ownership when adding that proof.
Keep F13 open until every listed miss has a recorded disposition and the required mutation evidence supports closure.

Authority: BUILD.md's mutation rule, plan section 8, and the restack brief's per-file mutation and exclusion rules.
The implementer plans worker-core coverage first, then testkit coverage and slow-tier mutation proof for real-process functions.
No proposed fix or exclusion has received review in this round.

VERDICT: NOT CLEAN (F13 open; 167 mutant entries require closure) on `78b88fa99546059dc5d8500af890432edd855537`.

## Round 18 — Partial worker-core mutation disposition

Reviewed head: `fdd2b8e73927d592b75713c55a86c6ea8c60c037`.
Previous reviewed head: `78b88fa99546059dc5d8500af890432edd855537`.
The complete delta adds four worker-core tests, one exact-mutant exclusion, and a handoff update.
The implementer explicitly requests partial review, not CLEAN.

The exclusion for `replace || with && in Worker::report_exit` is ACCEPTED as equivalent in M1.
It names one function and one mutation. Its comment gives the complete reachability argument.
The first accepted hello precedes Launch, so `report_exit` returns because no exit exists.
The only later caller is the first drain after an exit. That caller sets `exit_drained` before calling `report_exit`.
At that call, `exit_reported` is false and `!exit_drained` is false. Both expressions therefore continue to the same report.
Later drains do not call `report_exit`, and M1 has no reconnect path.
This disposition closes that one entry under F13. Recheck the argument when caller or reconnect behavior changes.

The four tests cover the other four worker-core misses in source:

- `Worker::on_frame`, HOST_MSG guard replaced with true: a HELLO frame containing a host Kill message must produce no action.
- `Worker::on_op`, kill equality changed to inequality: explicit Signal(Kill) after the exit drain must emit one reap.
- `Worker::on_drained`, `&&` changed to `||`: a drain before the exit must not permit a reap before the required exit drain.
- `Worker::on_end_payload`, `||` changed to `&&`: EndPayload after group kill must produce neither another signal nor a grace deadline.

The tests observe machine actions and reports. They add no test branch or terminal expectation.
Each test reaches the behavior changed by its named mutant. The reviewer accepts the test design.
The implementer says focused mutation verification is starting. No result accompanies this review request.
These four entries remain pending mutation evidence. The remaining 162 entries have no disposition in this delta.

The reviewer inspected logic only and ran no tests or gate.
F13 remains OPEN: one accepted equivalent entry, four entries awaiting mutation evidence, and 162 other entries awaiting closure.
All earlier findings retain their closures. This round does not grant CLEAN.

VERDICT: NOT CLEAN (F13 open) on `fdd2b8e73927d592b75713c55a86c6ea8c60c037`.

## Round 19 — Worker-core mutation evidence

Evidence head: `fdd2b8e73927d592b75713c55a86c6ea8c60c037`.
The reviewer read the focused Linux mutation log. The reviewer ran no tests or gate.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-fdd2b8e7-linux-20261004-132453-37330.log`.
The header names this exact head and the file `crates/botster-worker-core/src/worker.rs`.
The job reports exit 0: 110 mutants tested, 103 caught, and 7 unviable.
The complete counts leave zero missed mutants and zero timeouts.
This is focused mutation evidence, not a full green gate.

The four worker-core entries reviewed in round 18 are now CLOSED under F13.
The exact `Worker::report_exit` equivalent entry remains CLOSED by the accepted argument in round 18.
All five original worker-core misses therefore have recorded closure.
F13 remains OPEN for the other 162 entries in the preserved list.
The implementer is continuing the testkit work. No new source delta accompanies this evidence.

VERDICT: NOT CLEAN (F13 open; 162 mutant entries remain) on `fdd2b8e73927d592b75713c55a86c6ea8c60c037`.

## Round 20 — Proposed host-edge mutation coverage

Reviewed head: `91f8a4260a5e0ffb0721872400ad03da4868498e`.
Previous reviewed head: `fdd2b8e73927d592b75713c55a86c6ea8c60c037`.
The complete delta adds host-edge tests, extends the harness worker_named check, and updates the handoff.
The only production-file change declares the test module under `cfg(test)`.
The delta adds no behavior branch or mutation exclusion.

The tests exercise row persistence and injected failures, seeded edge forwarding, wake behavior, link data and descriptors, process events, and directory reopen.
The facade test checks configuration forwarding and typed failures through the host driver.
The wake test uses a marked deadline and checks that an unset wake waits for the requested timeout.
No child process or terminal expectation is added.
No new source finding exists in this delta.

The reviewer accepts these test designs but does not infer that every missed mutant is caught.
In particular, the new spawn test sends one byte per link.
That test does not distinguish the `spawn_worker` mutant that changes `64 * 1024` to `64 + 1024`.
That entry still needs mutation evidence or an equivalent-mutant argument, as F13 already requires.
The implementer says compilation and focused mutation verification remain pending.

The reviewer inspected logic only and ran no tests or gate.
No additional F13 entry closes in this round. The five worker-core closures remain preserved.
The other 162 entries remain open, including the 29 original core.rs and harness.rs misses pending evidence.
WorkerEdges and real-process findings remain later work.

VERDICT: NOT CLEAN (F13 open; 162 mutant entries remain) on `91f8a4260a5e0ffb0721872400ad03da4868498e`.

## Round 21 — AttachOptions fixture correction

Reviewed head: `cc34474de63030b74031f5a21d6608bf27a4a843`.
The complete delta from `91f8a42` replaces the unavailable `AttachOptions::default()` with the contract JSON reader.
The fixture supplies an explicit `file_directory`. The handoff records the failed baseline compilation.
The failed job supplies no mutation evidence. The corrected job remains pending.
No new source finding exists. No mutation entry closes in this round.
The reviewer inspected logic only and ran no tests or gate.

VERDICT: NOT CLEAN (F13 open; 162 mutant entries remain) on `cc34474de63030b74031f5a21d6608bf27a4a843`.

## Round 22 — Verified testkit mutation closures

Evidence head: `cc34474de63030b74031f5a21d6608bf27a4a843`.
The reviewer read the focused Linux log and the actual focused outcomes. The reviewer ran no tests or gate.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-cc34474d-linux-20261004-133509-42847.log`.
The job reports 177 tested: 94 caught, 27 missed, 56 unviable, and 0 timeouts.
This is a failed focused mutation job, not a full green gate.

The initial failure collector copied stale `target/mutants.out` from the earlier full gate.
The implementer retrieved the existing focused output from `target/p3-testkit-mutants/mutants.out` without a rerun.
The actual outcomes and lists are at `/private/tmp/p3-testkit-evidence/`.
Their counts match the focused log. The reviewer matched every original testkit entry by its exact mutant name.

The actual outcomes mark 44 original misses as `CaughtMutant`:

- `core.rs`: 27.
- `harness.rs`: 1.
- `worker.rs`: 16.

Those 44 entries are CLOSED under F13.
Their exact names are preserved in `verdicts/p3-worker-mutants-cc34474-caught.txt`.
No original testkit entry became unviable. The other 27 original testkit entries remain `MissedMutant`.
The capacity arithmetic miss and the release/release_owner misses remain open.

The five worker-core closures remain preserved.
F13 remains OPEN for 118 entries: 27 testkit, 26 payload edge, and 65 real worker driver entries.
No new source delta accompanies this evidence. Proposed binding tests and equivalence arguments require later review.

VERDICT: NOT CLEAN (F13 open; 118 mutant entries remain) on `cc34474de63030b74031f5a21d6608bf27a4a843`.

## Round 23 — Proposed capacity and read-bound equivalence arguments

Source examined: `cc34474de63030b74031f5a21d6608bf27a4a843`.
The implementer proposes exact exclusions for the capacity and READ_CHUNK arithmetic mutants.
Both mutations change 65536 bytes to 1088 bytes.
The proposed arguments establish that partial progress can preserve bytes. They do not establish equivalent behavior.
A smaller capacity changes when the link becomes full and writable.
A smaller read bound splits a control frame or a batch of control frames across more Binding::take inputs.
The scheduler can select other ready inputs between those reads. Its choices and the event timing can therefore differ for the same seed.
A5-2 permits varied partial progress. Permission for variation does not prove that this mutation cannot change behavior.

The reviewer does not accept either equivalence argument as supplied.
Both original entries remain OPEN under F13. No proposed exclusion has received acceptance in this round.
The implementer must supply evidence or an argument that accounts for readiness, scheduling, and observable reports.
The reviewer ran no tests or gate. F13 remains open for 118 entries.

## Round 24 — Proposed worker binding coverage

Reviewed head: `2e9811ca0b1c8a790698999efc24c538b5e755d7`.
The complete delta from `cc34474` adds worker binding tests, declares their test module, and updates the handoff.
The tests cover partial link writes and cumulative counts, peer EOF, spawn-before-output ordering, drains, process signals, and exit notification.
They also check unique process identities and the shared worker machine's grace deadline.
The tests observe edge inputs, reports, and readiness. The delta adds no production behavior branch or mutation exclusion.
No terminal expectation is added. No new source finding exists.

The reviewer accepts the test designs. The implementer's focused mutation job is pending.
The release/release_owner entries and both arithmetic entries remain open.
No entry closes in this source-only round. The five worker-core and 44 testkit closures remain preserved.
The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13 open; 118 mutant entries remain) on `2e9811ca0b1c8a790698999efc24c538b5e755d7`.

## Round 25 — Verified worker binding mutation closures

Evidence head: `2e9811ca0b1c8a790698999efc24c538b5e755d7`.
The reviewer read the focused log and outcomes.json. The header names this exact head and `botster-core-testkit/src/worker.rs`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-2e9811ca-linux-20261004-134928-47783.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004134928-47783/target-mutants.out/`.
The job reports 100 tested: 60 caught, 4 missed, 36 unviable, and 0 timeouts. The baseline passed.
This is a failed focused mutation job, not a full green gate.

The reviewer matched the original worker.rs misses by exact mutant name.
Thirty-eight are now `CaughtMutant`; sixteen already closed in round 22.
The other 22 are now CLOSED under F13.
Their names are preserved in `verdicts/p3-worker-mutants-2e9811c-caught.txt`.
No original worker.rs entry became unviable.

Four worker.rs entries remain `MissedMutant`: READ_CHUNK arithmetic, the write-interest negation in ready, release, and release_owner.
The core.rs capacity arithmetic entry also remains open.
Earlier closures remain preserved. F13 remains OPEN for 96 entries: 5 testkit, 26 payload edge, and 65 real worker driver entries.
The reviewer ran no tests or gate. No new source delta accompanies this evidence.

VERDICT: NOT CLEAN (F13 open; 96 mutant entries remain) on `2e9811ca0b1c8a790698999efc24c538b5e755d7`.

## Round 26 — Remaining testkit coverage

Reviewed head: `91b58e2c8a8f7d198484e8019f08c50aae0e6dd4`.
The complete delta from `2e9811c` extends link tests, checks write interest, adds live-capture release tests, and updates the handoff.
The facade release test injects worker reports through the host edge.
Libghostty supplies the terminal modes, title, cwd, and every snapshot byte.
The fixture assigns revisions for its injected reports. It does not claim worker revision or snapshot conformance.
The release assertions check capture retirement through the real host driver.
The reviewer accepts the release and write-interest test designs. Their mutation evidence remains pending.
The delta adds no production behavior branch or mutation exclusion.

### F14 — LOW — The new bulk tests require implementation-specific single-call progress

Status: OPEN at this head.

Evidence: `core/tests.rs`, `each_spawn_has_its_own_link_and_process_events_reach_the_spawner`;
`worker/tests.rs`, `a_large_control_frame_is_one_ready_input`.

The spawn test requires all 8192 bytes to pass through one send and one receive.
The new read test requires an 8192-byte control payload to arrive in one Binding::take input.
A5-2 permits partial progress and varied chunk sizes. Plan 2.5 does not require one-call progress for this payload size.
These assertions check the current capacity and READ_CHUNK choices rather than a clause or a real bug.
A smaller positive bound can split valid frames while preserving their bytes and decoded messages.
The test description cites plan 2.5, but that section does not fix the asserted one-input behavior.

Required change: Replace these assertions with clause-based proof of byte retention, ordering, and complete frame delivery under partial progress.
Do not add an arbitrary minimum chunk or capacity requirement solely to catch a tuning mutant.
Keep both arithmetic entries under F13 until their disposition satisfies the mutation rules and the test rules.
The rejected equivalence arguments in round 23 remain rejected as supplied.

Authority: BUILD.md testing rule 3, its rule against testing helper internals, and A5-2.

The reviewer inspected logic only and ran no tests or gate.
The focused mutation job is pending. No F13 entry closes in this round.
The five worker-core and 66 testkit closures remain preserved.

VERDICT: NOT CLEAN (F13 open with 96 entries; F14 open) on `91b58e2c8a8f7d198484e8019f08c50aae0e6dd4`.

## Round 27 — Lead ruling on pure tuning constants

The lead supplied a review rule for the capacity and READ_CHUNK arithmetic entries.
The lead accepts contract equivalence for pure tuning constants only when each entry satisfies all four conditions:

1. The entry cites that no clause fixes the value.
2. A test proves retention, order, and complete frames at the default, at 1088, and at the smallest legal value.
3. The code enforces any lower bound implied by the contract. A mutant below that bound must be killed.
4. Each entry names one function or constant and includes the argument and the names of its proving tests.

This rule permits a conditional disposition beyond the behavior-identity argument reviewed in round 23.
The supplied round 23 arguments alone still do not satisfy this rule.
F14's rejection of one-call 8192-byte assertions remains in force, as the lead explicitly confirms.
No source delta, qualifying test evidence, or proposed exclusion accompanies the ruling.
Both arithmetic entries remain OPEN under F13. F13 retains 96 open entries, and F14 remains OPEN.
The reviewer will apply all four conditions to the proposed correction and exclusions.
The reviewer ran no tests or gate.

## Round 28 — Release and write-interest mutation evidence

Evidence head: `91b58e2c8a8f7d198484e8019f08c50aae0e6dd4`.
The reviewer read the focused log. Its header names this exact head and the selected mutation expressions.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-91b58e2c-linux-20261004-140021-52074.log`.
The job reports exit 0: seven selected mutants caught, zero missed, and zero timeouts.
The selection includes the release, release_owner, and ready write-interest mutations.

Those three original entries are now CLOSED under F13.
The two arithmetic entries remain OPEN because their current tests are subject to F14.
The implementer explicitly makes no arithmetic closure claim from this result.
The earlier five worker-core and 66 testkit closures remain preserved.
F13 retains 93 open entries: two testkit arithmetic entries, 26 payload edge entries, and 65 real worker driver entries.
F14 remains OPEN. The implementer is preparing the correction under the lead's round 27 rule.
The reviewer ran no tests or gate. This focused result does not establish a full green gate.

VERDICT: NOT CLEAN (F13: 93 entries; F14 open) on `91b58e2c8a8f7d198484e8019f08c50aae0e6dd4`.

## Round 29 — F14 correction and proposed tuning exclusions

Reviewed head: `34bad40b4ee3d97eab1866b991c4e2ba5251a7dd`.
The reviewer inspected the complete delta from `91b58e2` and ran no tests or gate.
The delta injects internal positive capacity and read bounds through the existing edge code.
The default values remain 65536. No test behavior branch or separate worker code path is added.

F14 status: CLOSED in source at this head.
The bulk spawn test no longer requires an 8192-byte single write.
The new control-read test no longer requires a complete frame in one input.
The replacement tests check byte retention, ordering, and complete frames at 65536, 1088, and 1.
The program test checks output retention and order at the same values.
The shared Worker/TestkitCore test checks create, start, signal, and remove completions through the same edge path at each bound.
The reviewer accepts these test designs. Their focused mutation job remains pending.

The two proposed exclusions remain UNACCEPTED under F13 pending changes and evidence.
Their comments cite the absence of a fixed contract value, explain partial progress, and name the proving tests.
However, both regular expressions match every unnamed `replace * with + in` mutant in their entire source file.
The comments name LINK_CAPACITY and READ_CHUNK, but the patterns do not identify those declarations.
Required change: Restrict each pattern to its one verified constant declaration or give that constant a uniquely matchable scope.
A later unnamed arithmetic expression must not inherit the exclusion.

The lower-bound checks use `debug_assert!` in `SimEdges::spawn_worker` and `Workers::with_read_chunk`.
Release builds omit these checks. The supplied zero-bound tests establish only the debug-build rejection.
Required change: Enforce the positive lower bound in every build, as condition 3 of the lead's ruling requires.
Keep the zero-bound proof and provide the focused result for the corrected code.

The lead's condition 2 also requires passing proof at all three bounds. Test source alone does not establish that result.
Both arithmetic entries remain OPEN under F13 until all four conditions have verified support.
The earlier mutation closures remain preserved. F13 retains 93 open entries: two testkit arithmetic entries and 91 real-process entries.

VERDICT: NOT CLEAN (F13 open; F14 closed) on `34bad40b4ee3d97eab1866b991c4e2ba5251a7dd`.

## Round 30 — Narrow exclusions and all-build lower bounds

Reviewed head: `c1af2211cedce075be7af5f63fa1cca51f0a6647`.
The complete delta from `34bad40` narrows both constant exclusions, replaces debug assertions, and updates the handoff.
The LINK_CAPACITY pattern names only its multiplication at core.rs:33:33.
The READ_CHUNK pattern names only its multiplication at worker.rs:38:30.
Each entry therefore covers one verified constant declaration and retains its argument and named tests.
Both lower-bound checks now use `assert!`, which enforces the positive range in all builds.

The source changes resolve both defects identified in round 29.
The reviewer conditionally accepts the two exclusion designs under the lead's four-condition rule.
Passing proof at the default, 1088, and 1 remains pending on this corrected head.
The job on `34bad40` cannot establish the corrected head's result.
Both arithmetic entries remain OPEN under F13 until the required evidence arrives.
F14 remains CLOSED. All earlier mutation closures remain preserved.
The reviewer inspected logic only and ran no tests or gate.

VERDICT: NOT CLEAN (F13 open; 93 mutant entries remain) on `c1af2211cedce075be7af5f63fa1cca51f0a6647`.

## Round 31 — Constant mutant-name correction

Reviewed head: `4960d73873d8301575e312f8fbc596059300c813`.
The complete delta from `c1af221` removes the trailing `in` from both exact-location exclusion patterns and updates the handoff.
Constant mutant names omit that suffix. The patterns retain their exact file, line, column, operator, and replacement.
Each pattern still covers only its verified constant declaration.
No executable code or proving test changes in this delta.
The reviewer accepts the pattern correction. The exclusions remain conditional on corrected-head evidence under round 30.
The handoff reports 178 tested at the older `34bad40` head: 120 caught, 1 missed, 57 unviable, and 0 timeouts.
That result does not prove the all-build checks or corrected patterns at this head.
No additional original entry closes in this round. F13 retains 93 open entries; F14 remains closed.
The reviewer inspected logic only and ran no tests or gate.

VERDICT: NOT CLEAN (F13 open; 93 mutant entries remain) on `4960d73873d8301575e312f8fbc596059300c813`.

## Round 32 — Corrected-head testkit proof

Evidence head: `4960d73873d8301575e312f8fbc596059300c813`.
The reviewer read the focused Linux log. Its header names this exact head and all three changed testkit files.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-4960d738-linux-20261004-141936-76354.log`.
The baseline passes. The job reports exit 0: 176 tested, 119 caught, 57 unviable, zero missed, and zero timeouts.
The baseline includes the reviewed parameterized tests and both zero-bound tests.
The focused run therefore supplies corrected-head evidence for the source review in rounds 29 through 31.

Both tuning exclusions are now ACCEPTED under the lead's four-condition rule.
No clause fixes the two positive values, and the comments cite partial progress and decoder retention.
The named tests prove retention, ordering, and complete frames at 65536, 1088, and 1.
The shared Worker/TestkitCore test proves complete lifecycle operations through the same edge code at those bounds.
All-build assertions enforce the positive lower bound. The named zero-bound tests prove rejection.
The exact-location patterns cover only LINK_CAPACITY and READ_CHUNK, one declaration per entry.
The two original arithmetic entries are CLOSED under F13 by this accepted contract-equivalence argument and proof.
Recheck the exclusion locations and arguments when those declarations, callers, or contract requirements change.

All 71 original testkit entries now have recorded closure: 69 caught and 2 accepted tuning exclusions.
The five worker-core entries remain closed: four caught and one accepted equivalent entry.
F13 remains OPEN for 91 original real-process entries: 26 payload edge entries and 65 real worker driver entries.
F14 remains CLOSED. No new source delta accompanies this evidence.
The reviewer ran no tests or gate. This focused result does not establish a full green gate.

VERDICT: NOT CLEAN (F13 open; 91 mutant entries remain) on `4960d73873d8301575e312f8fbc596059300c813`.

## Round 33 — Proposed real payload coverage

Reviewed head: `5172a53553636dc94d9c86c98b0969499b1955dd`.
The complete delta from `4960d73` adds error mapping tests, extends real PTY tests, adds Drop/reap proof, and updates the handoff.
The independent payload guard remains. The delta adds no real-process exclusion or production behavior branch.
The new tests cover nonblocking flags, payload identity, pending output, and input delivery.
The source additions also expose two test failure paths below.
Focused slow-tier mutation evidence is pending. No F13 entry closes in this round.

### F15 — MEDIUM — The readiness reader still loops on EOF

Status: OPEN at this head.
Evidence: `crates/botster-core-sys/tests/slow_payload.rs`, the changed read loop in `a_group_signal_ends_the_leader_and_its_group`.

The loop waits until its collected bytes contain `up`.
Its `Ok(n)` arm accepts `n == 0`, appends no bytes, and repeats the loop.
A payload that exits before the readiness text, or a mutant that returns EOF, makes the loop repeat without a readiness wait.
The new retained-byte bound does not detect that path because the byte count stays unchanged.
The test cannot fail or drop its independent guard through this loop.

Required change: Fail on EOF before the expected readiness text. Keep the independent guard active through that failure.
The test must reach a finite failure when no further byte can arrive.
Authority: BUILD.md testing rules 5 and 10, and the restack brief's cleanup-on-panic requirement.

### F16 — LOW — The reap assertion can wait on a reused child PID

Status: OPEN at this head.
Evidence: `crates/botster-core-sys/tests/slow_payload.rs`, `dropping_the_payload_reaps_its_leader`.

The test caches the leader PID, drops the production owner, and calls blocking `waitid(EXITED | NOWAIT)` on that PID.
Production has already reaped the leader on the expected path, so its PID is no longer reserved.
Another concurrently running test can own a child that receives that PID.
The query does not reap that child, but it can wait for that child's exit or report its status instead of ECHILD.
The cached PID alone does not prove that the query still addresses this test's payload.

Required change: Use a reap observation tied to this test's child ownership, without reaping the production payload.
The check must not block on or inspect another test's child after PID reuse.
An isolated helper with no other child owner is one possible approach.
Preserve the independent group guard and production-only reaping.
Authority: the real-process ownership rule, BUILD.md's independent-test rules, and the PID reservation principle retained in F7.

The reviewer inspected logic only and ran no tests or gate.
All previous closures remain preserved. F13 retains 91 open entries.

VERDICT: NOT CLEAN (F13: 91 entries; F15 and F16 open) on `5172a53553636dc94d9c86c98b0969499b1955dd`.

## Round 34 — Payload mutation evidence and compiler correction

Evidence head: `b6b1660bb2c63828c75e51b0ec95cab8f585b9a3`.
The complete source delta from `5172a53` changes Errno::ACCES to Errno::ACCESS in the test and records the failed baseline.
No new source finding exists in that correction. F15 and F16 remain open.
The reviewer read the corrected-head slow-profile mutation log and actual outcomes.json.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-b6b1660b-linux-20261004-143410-99127.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004143410-99127/target-mutants.out/`.

The baseline passes. The job reports 30 tested: 21 caught, 4 missed, 4 unviable, and 1 timeout.
All 26 original payload entries match these outcomes: 21 caught, 4 missed, and 1 timeout.
Twenty caught entries are now CLOSED under F13.
Their names are preserved in `verdicts/p3-worker-mutants-b6b1660-caught.txt`.
The caught `Payload::drop` entry remains pending corrected proof because its reap assertion is subject to F16.

The `Payload::read -> Ok(0)` entry timed out, which confirms F15's failure path.
The other four misses remain open: Payload::reap no-op, wait_unreaped bitwise XOR, wait_unreaped fallback sign, and set_nonblocking bitwise XOR.
The mutation timeout is not accepted as a caught result.
No equivalence argument or real-process exclusion has received review in this round.

F13 remains OPEN for 71 entries: six payload entries and 65 real worker driver entries.
F15 and F16 remain OPEN. All earlier closures remain preserved.
The reviewer inspected logic and existing evidence only. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 71 entries; F15 and F16 open) on `b6b1660bb2c63828c75e51b0ec95cab8f585b9a3`.


## Round 35 — Payload equivalence proposals and observer delta

Reviewed head: `3d13dda8fb2baad575495501a083d0244eb726ce`.
The complete delta from `b6b1660` adds premature-EOF failure, an isolated reaping observer, and the handoff update.
No production path or mutation exclusion changes. The reviewer ran no tests or gate.

F15 is CLOSED in source: the readiness loop now panics on EOF before readiness.
Unwinding drops the independent payload guard.
F16 is CLOSED in source: the observer runs only its exact helper with one test thread and owns no other direct child.
The query uses NOWAIT and NOHANG. It cannot block or reap, and a reused PID cannot identify another child of this observer.
The outer Observer retains its Child until wait completes and retires the handle before Drop can signal it.
Corrected mutation evidence remains pending. The held Payload::drop entry and timed-out Payload::read entry remain open under F13.

The wait_unreaped OR-to-XOR proposal has a valid argument: EXITED and NOWAIT are disjoint flags in pinned rustix 1.1.5.
The set_nonblocking OR-to-XOR proposal has a valid current-caller argument.
Its sole caller passes a fresh PTY from pinned pty-process 0.5.3 blocking::open.
The dependency opens RDWR|NOCTTY, sets descriptor CLOEXEC, and does not set status NONBLOCK on that path.
No other code receives the master before set_nonblocking. OR and XOR therefore set the same clear bit.
Each exclusion still requires one exact-function mutation entry, its reason, and a caller/dependency recheck condition.
No F13 entry closes before the reviewer inspects those entries.

The Payload::reap no-op proposal remains pending.
The production machine requires killed && exit_drained before ReapPayload.
However, a_group_signal_ends_the_leader_and_its_group sends TERM before reap, rather than KILL.
The proposed claim that every caller has already killed the group is not a complete proof.
A revised argument must cover every current caller and the additional SIGKILL while the leader remains reserved.
Signal delivery alone does not establish that every group member has already exited.
The wait_unreaped fallback-sign entry remains open without an accepted argument.

### F17 — MEDIUM — The observer does not detect parent death

Status: OPEN at this head.
Evidence: slow_payload.rs, dropping_the_payload_reaps_its_leader and Observer.

The outer test starts another test executable and waits for it.
Observer::drop kills that child when the outer test unwinds, but parent process death does not run Drop.
The observer has no parent-lifetime connection or parent-death mechanism.
If a mutation leaves the observer blocked, termination of the outer test process can leave the observer alive.
The payload guard detects observer death; it does not cause observer death when the outer parent dies.

Required change: Give the observer an independent parent-lifetime mechanism that ends it when the outer test process dies.
Preserve the isolated child ownership proof and production-only payload reaping.
Preserve the independent payload group guard through observer termination.
Authority: the user's real-process rule that children exit when their parent is gone.

F13 retains 71 entries. F17 remains open. All earlier findings and closures remain preserved.

VERDICT: NOT CLEAN (F13: 71 entries; F17 open) on `3d13dda8fb2baad575495501a083d0244eb726ce`.


## Round 36 — Observer ownership and three payload exclusions

Reviewed head: `ebed1022a1f3804712342f8288902f42ef1db159`.
The complete delta from `3d13dda` changes observer startup, one reap caller, the exit-wait loop, three exclusions, and the handoff.
No conflict resolution accompanies this delta. The reviewer inspected all four changed files and the existing GroupGuard implementation.
The reviewer ran no tests or gate. The pending job at `3d13dda` cannot prove this later source delta.

F17 is CLOSED in source.
The outer test creates GroupGuard before it starts the observer shell in a new process group.
The shell waits for the independent anchor to join that group before exec starts the observer.
Parent death closes the anchor's control socket. The anchor then kills its owned observer group.
Observer death closes the separate payload-guard connection, so the payload anchor ends its own payload group.
GroupGuard reaps only its own anchor. Observer reaps only its observer child. Production alone reaps the payload.
The observer has no other direct child during the retired-payload query; the outer test owns the observer anchor.
The registration helper exits and the shell waits for it before exec starts the observer.
The NOWAIT|NOHANG query and the isolated ownership proof remain intact. F16 remains CLOSED in source.

The three new exclusions are ACCEPTED under F13.
Each pattern identifies one exact mutation in one named function and matches one original missed entry.
Their original names are preserved in `verdicts/p3-worker-mutants-ebed102-equivalent.txt`.

- Payload::reap no-op: each current caller observes the leader's exit and sends group SIGKILL before consuming Payload.
  The corrected TERM test now follows that order. Production requires killed && exit_drained before ReapPayload.
  A no-op runs Drop, which sends a redundant SIGKILL while the same leader remains unreaped, then waits for that leader.
  The reserved group id prevents a signal to a reused group. The reported leader exit cannot change.
  The argument permits the first SIGKILL to remain pending. Recheck callers and Drop when either changes.
- wait_unreaped OR-to-XOR: EXITED and NOWAIT are disjoint flags, so both operations produce the same options.
  The pattern covers only that operation in wait_unreaped. Recheck the options and pinned rustix when either changes.
- set_nonblocking OR-to-XOR: its sole current caller passes a fresh blocking PTY from pinned pty-process 0.5.3.
  The dependency's blocking open path leaves NONBLOCK clear. Both operations therefore set the same bit.
  Recheck the caller and the pinned PTY dependency when either changes.

The exit-watch change extracts the existing decision loop into wait_unreaped_with with an injected wait operation.
Production supplies the same waitid(P_PID, EXITED|NOWAIT) operation. The default test supplies EINTR followed by ECHILD.
Production and the test use one loop, with no production test branch.
The test checks the retry and negative fallback code. No exclusion covers that fallback.
Mutation evidence must identify the relocated fallback-sign entry in wait_unreaped_with before its original entry can close.

F13 retains 68 entries: 65 real driver entries and three payload entries.
The payload entries are Payload::drop, Payload::read -> Ok(0), and the relocated fallback sign.
All other findings remain CLOSED. All earlier findings and closures remain preserved.

VERDICT: NOT CLEAN (F13: 68 entries open) on `ebed1022a1f3804712342f8288902f42ef1db159`.


## Round 37 — Corrected EOF and reaping mutation evidence

Current reviewed head remains `ebed1022a1f3804712342f8288902f42ef1db159`.
Evidence head: `3d13dda8fb2baad575495501a083d0244eb726ce`.
The reviewer read the exact-head Linux log and actual outcomes.json.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-3d13dda8-linux-20261004-144102-7257.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004144102-7257/target-mutants.out/`.

The baseline passes. The job reports 30 tested: 22 caught, four missed, four unviable, and zero timeouts.
The original Payload::read -> Ok(0) and Payload::drop no-op entries are both CaughtMutant in the actual outcomes.
These two original entries are now CLOSED under F13.
Their names are preserved in `verdicts/p3-worker-mutants-3d13dda-caught.txt`.
The EOF test now fails rather than loops. The isolated reaping assertion has the child-ownership proof accepted in round 35.
The later observer startup delta preserves that assertion and improves independent cleanup.
This earlier run does not prove the later observer guard or the relocated fallback-sign test.

The four missed entries are the three equivalence entries accepted in round 36 and the fallback sign.
The fallback-sign entry remains open until evidence covers its relocation to wait_unreaped_with.
F13 retains 66 original entries: 65 real worker driver entries and one payload fallback-sign entry.
Current-head payload evidence remains pending. No full green gate is established by this focused result.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries open) on `ebed1022a1f3804712342f8288902f42ef1db159`.


## Round 38 — Mac baseline failure before mutation

Reviewed and evidence head: `ebed1022a1f3804712342f8288902f42ef1db159`.
The reviewer read the exact-head Mac log and actual baseline.log in the gate worktree.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-ebed1022-mac-20261004-150213-23937.log`.
Baseline: `~/botster-sessions/gate-trees/botster-core-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa/target/mutants.out/log/baseline.log`.
The job exits 124 after 2701 seconds at its 45-minute deadline. No mutant runs.
The baseline reports 59 passing tests and one test terminated after 2686.817 seconds.
The failing test is the_pty_counts_output_and_delivers_input_to_the_program.
Its stderr reports the pending_output > 1 assertion failure at slow_payload.rs:229.
The test then remains alive until the job sends SIGTERM.
The observer test passes in this baseline, but a failed baseline supplies no mutation closure.

### F18 — MEDIUM — The pending-output test fails on Mac

Status: OPEN at this head.
Evidence: slow_payload.rs, the_pty_counts_output_and_delivers_input_to_the_program, and the exact-head Mac baseline.

The test polls the PTY, checks only that poll returns a positive count, and requires pending_output > 1 immediately.
A positive poll count does not prove that the program has completed its setup and queued the expected output.
The Mac baseline reaches the assertion and fails it.
The evidence does not yet distinguish a test readiness error from an incorrect production pending-output query.
Production uses pending_output as the bound for its drain after exit, so the distinction matters to EV-4.

Required change: Establish the failure's cause and correct the readiness proof or production query as needed.
Prove that the query counts queued program output on Mac with a bounded readiness sequence.
Check the relevant readiness event. Keep the input-delivery and output-retention assertions.
Do not weaken the assertion to hide an incorrect production drain bound.
Authority: EV-4, A2-1, BUILD.md's deterministic test rule, and the real-process review scope.

### F19 — HIGH — Panic cleanup blocks while the payload waits for input

Status: OPEN at this head.
Evidence: the same Mac baseline and GuardedPayload/ PayloadGuard cleanup.

The input test panics before it writes the input line.
The payload program can still wait in read at that point.
GuardedPayload::drop must end its group before production can block in its reaper.
Instead, the test remains alive for 2686.817 seconds until the external deadline sends SIGTERM.
The log does not identify the blocking cleanup operation or prove the anchor's group membership on Mac.
A passing observer test does not prove this payload-panic path.

Required change: Make cleanup end the owned payload group on this panic path on Mac.
Prove independent anchor membership and finite cleanup while the payload waits for input.
Keep the guard independent of mutated payload functions. The guard must not reap the production payload.
Retain parent-death cleanup and prevent signals to a group whose ownership has ended.
Authority: the user's real-process ownership rules and BUILD.md's cleanup-on-failure requirement.

F13 retains 66 original entries. F18 and F19 remain OPEN.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries; F18 and F19 open) on `ebed1022a1f3804712342f8288902f42ef1db159`.


## Round 39 — Proposed Mac guard and readiness correction

Reviewed head: `a532a7292a3a419cb9903b1425b22ef331f05be2`.
The complete delta from `ebed102` changes the payload anchor, the pending-output test, and the handoff.
The shell now supplies its live group id while it waits for registration.
The anchor calls setpgid before it registers, so the shell cannot start its body before the anchor joins the payload group.
Cleanup still signals only the anchor's current group. The delta does not add a cached-id signal after ownership ends.
This improves the explicit group-ownership proof. It does not establish the cause of the prior blocked cleanup.
F19 remains OPEN pending Mac proof of finite panic cleanup while the payload waits for input.

F18 remains OPEN, including the new test loop.
The loop repeatedly queries pending_output and polls without checking revents or consuming bytes.
If pending_output stays at zero or one while the descriptor remains ready, poll can return immediately on every iteration.
That path is a busy loop until the deadline. A deadline bounds time but does not establish a blocking readiness sequence.
The prior Mac failure's cause remains unverified.
Use bounded synchronization to prove queued program output and retain the production count check.
Do not hide an incorrect EV-4 drain bound behind a weaker test.

The delta changes no production path or mutation exclusion.
F13 retains 66 original entries. F18 and F19 remain OPEN.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries; F18 and F19 open) on `a532a7292a3a419cb9903b1425b22ef331f05be2`.


## Round 40 — Current-v1 binding merge delta

Reviewed head: `a32542a887ca013afea3c787d557caa010e663fd`.
Previous reviewed head: `a532a7292a3a419cb9903b1425b22ef331f05be2`.
Merge commit: `d25355563b00aa16aec44fa39cfda63a60c9b6d3`.
Its parents are the previous P3 head and merged v1 `38bbe580013d02175636395a28a4c359b20c32b2` (PR #138).
The implementer reports no conflicts. No conflict resolution is named or present in the changed P3 scope.

The merge imports binding inspection, restoration, hyperlink/parser reads, cell/color/cursor reads, and the image-limit APIs.
All nine imported binding and audit files match the merged v1 blobs exactly.
The other changes are the combined mutation configuration and the P3 handoff update.
No P3 worker, payload, machine, or testkit source changes.
Current M1 callers do not adopt the new inspection or restoration APIs.
The existing terminal-identity call and binding-backed testkit proof remain unchanged.
The P2 implementation's contract verdict belongs to the P2 and integration reviewers; this review covers its P3 merge effects.

The mutation configuration preserves all reviewed P3 entries unchanged.
The only added entry identifies the Render::drop no-op in inspection.rs, one named function and one mutation.
Its reason states that removing the native free leaks memory without changing a public read.
The destructor contains only ghostty_render_state_free. The entry follows the existing binding destructor-exclusion form.
No broad P3 exclusion, handwritten terminal expectation, new worker path, or production test branch accompanies the merge.

The handoff correctly retains F13, F18, and F19 and identifies the pending earlier-head Mac baseline.
That earlier job cannot establish a full green gate on this merged head.
No new P3-scope finding exists in this merge delta.
F13 retains 66 original entries. F18 and F19 remain OPEN.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries; F18 and F19 open) on `a32542a887ca013afea3c787d557caa010e663fd`.


## Round 41 — FIFO synchronization and bounded panic cleanup

Reviewed head: `ec8cee11fea200ef6db9dd2d80f5200d5dca2180`.
The complete delta from `a32542a` changes slow_payload.rs and its handoff.
No production path, exclusion, dependency, or conflict resolution changes.

The input test now uses payload_waiting_for_input instead of repeated PTY queries and polls.
The program writes ready to the PTY, runs external /bin/echo to write queued to a FIFO, then reads its input.
The test opens the FIFO before spawn, waits once within a deadline, checks IN, and consumes the marker.
It then checks the unchanged production pending_output query before it sends the input line.
The input-delivery and complete-output assertions remain.
This removes the busy loop and separates incomplete program output from a production query failure.
The guard remains active through setup, marker wait, query failure, and panic.
F18 remains OPEN pending Mac evidence for the queued-output count.
A further count failure after this marker requires investigation of the production query rather than another readiness workaround.

The new a_panic_ends_the_payload_while_it_waits_for_input test uses the same marker synchronization.
A separate thread catches a panic while it owns the guarded payload.
The main test bounds completion of the independent guard and production reaper with recv_timeout, then joins the thread.
The guard does not reap the payload. Production still owns that reap.
Parent death retains the independent anchor cleanup from the prior source correction.
The new test exercises the failure path from the Mac baseline with a finite completion check.
F19 remains OPEN pending Mac evidence for this exact corrected path.

No new source finding exists in this delta.
The active Mac baseline at `a532a729` does not cover this later synchronization or panic test.
F13 retains 66 original entries. F18 and F19 remain OPEN pending evidence.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries; F18 and F19 open) on `ec8cee11fea200ef6db9dd2d80f5200d5dca2180`.


## Round 42 — Second Mac failure and handoff-only delta

Reviewed head: `4b1d39cb294dfc4308dae1ddb0f2ce4f6274170a`.
The complete delta from `ec8cee1` changes only docs/handoffs/p3-worker.md.
The source, exclusions, and dependency pins remain unchanged.
The handoff records the Mac resource hold and the later M2b paging/specification obligation under R-30 and A8-2.
That later obligation adds no M1 implementation or closure in this round.

Evidence head: `a532a7292a3a419cb9903b1425b22ef331f05be2`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-a532a729-mac-20261004-154924-48598.log`.
The reviewer read the exact-head plain nextest log.
The Mac job exits 124 after 2700 seconds. It runs 14 tests: 13 pass and the input test remains alive until external SIGTERM.
The test reports no program output at its ten-second deadline, then terminates after 2688.563 seconds.
Explicit anchor group membership alone therefore does not fix the blocked cleanup observed under F19.
The evidence does not identify the blocking cleanup operation or establish the pending-output failure's cause.
The later FIFO synchronization and bounded panic test are absent from this run.

F18 and F19 remain OPEN pending corrected-source Mac evidence and any required source correction.
A Linux result can support the source review but cannot establish correction of these observed Mac failures.
The recorded Mac hold limits when the implementer can collect that evidence; it does not close either finding.
F13 retains 66 original entries. No mutation closure follows from this plain baseline.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries; F18 and F19 open) on `4b1d39cb294dfc4308dae1ddb0f2ce4f6274170a`.


## Round 43 — Corrected-source Linux payload baseline

Reviewed and evidence head: `4b1d39cb294dfc4308dae1ddb0f2ce4f6274170a`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-4b1d39cb-linux-20261004-163542-77986.log`.
The reviewer read the exact-head plain nextest log with base v1 `38bbe580`.
The Linux job exits zero after ten seconds. All 15 slow_payload tests pass with zero skipped in 0.022 seconds.
The queued-output/query/input test passes in 0.006 seconds.
The panic-while-waiting-for-input test passes in 0.008 seconds.
This supplies Linux evidence for the corrected FIFO synchronization and bounded cleanup source from round 41.
It does not establish correction of the observed Mac failures under F18 and F19.
No mutant runs in this plain baseline, so no F13 entry closes.

F13 retains 66 original entries. F18 and F19 remain OPEN for corrected-source Mac evidence.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries; F18 and F19 open) on `4b1d39cb294dfc4308dae1ddb0f2ce4f6274170a`.


## Round 44 — Failure diagnostics and payload mutation summary

Reviewed head: `0e66b74f97d2f6b7665fcff1310f83bfd0bca4b2`.
The complete delta from `4b1d39c` adds test failure diagnostics and the handoff evidence.
No production path, exclusion, or dependency changes.
The query test prints its pending count only on failure, before its assertion and guard Drop.
The panic test starts independent cleanup in its own thread before the main test can start a process-state diagnostic.
The diagnostic reads ps output and filters by the test and recorded payload ids. It never signals or reaps those processes.
The label identifies the payload PID as recorded before cleanup; the diagnostic does not reserve that PID after production reap.
The code test deadlines remain ten seconds. No new source finding exists in this delta.

Mutation evidence head: `4b1d39cb294dfc4308dae1ddb0f2ce4f6274170a`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-4b1d39cb-linux-20261004-163631-78503.log`.
The reviewer read the exact-head focused Linux mutation log.
The baseline passes. The summary reports 28 tested: 23 caught, five unviable, zero missed, and zero timeouts.
The successful job has no locally collected raw outcomes available to the reviewer.
The reviewer requested retrieval of the existing outcomes.json and caught.txt without a rerun.
The relocated fallback-sign entry remains open until its exact outcome is verified.
The summary alone does not map that original entry to a caught result.

F13 retains 66 original entries pending the raw fallback evidence.
F18 and F19 remain OPEN for corrected-source Mac evidence. The Mac hold still applies.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries; F18 and F19 open) on `0e66b74f97d2f6b7665fcff1310f83bfd0bca4b2`.


## Round 45 — GHOSTSNP documentation merge and diagnostic compilation

Reviewed head: `39a55c9e8fd8ecc6d4678ba952a363d99fa04139`.
Previous reviewed head: `0e66b74f97d2f6b7665fcff1310f83bfd0bca4b2`.
Merge commit: `e8094417d6d2d12600945313bf3ac839c8ebd397`.
Its parents are the previous P3 head and merged v1 `a3b8af5be21389423439fb3c09d6a81d924c987d` (PR #139).
The implementer reports no conflicts. No P3 conflict resolution accompanies the delta.

The complete delta imports GHOSTSNP.md, changes only snapshot rustdoc, and updates the P3 handoff.
The imported specification and snapshot.rs blobs match merged v1 exactly.
No production behavior, P3 source, exclusion, dependency, or conformance list changes.
The specification leaves Worker paging pending until the P3 M2b CaptureSnapshot implementation.
The M2b PR must complete that section with citations to its paging code.
Host metadata contributes zero counted page bytes, but that fact does not establish the worker's framing overhead.
The every-cut fit check remains inconclusive until the Worker paging section defines that overhead.
The merge introduces no new P3-scope finding and supplies no M1 finding closure.

Compilation evidence head: `0e66b74f97d2f6b7665fcff1310f83bfd0bca4b2`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-0e66b74f-linux-20261004-163914-80877.log`.
The reviewer read the exact-head Linux nextest log.
The diagnostics compile, and all 15 slow_payload tests pass with zero skipped in 0.020 seconds.
The job exits zero after ten seconds. This proves Linux compilation and baseline behavior only.
It does not close F18 or F19's Mac evidence requirements.

F13 retains 66 original entries pending raw fallback-sign evidence.
F18 and F19 remain OPEN for corrected-source Mac evidence.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 66 entries; F18 and F19 open) on `39a55c9e8fd8ecc6d4678ba952a363d99fa04139`.


## Round 46 — Raw fallback-sign outcome

Current reviewed head remains `39a55c9e8fd8ecc6d4678ba952a363d99fa04139`.
Evidence head: `4b1d39cb294dfc4308dae1ddb0f2ce4f6274170a`.
The implementer retrieved existing artifacts without rerunning the job, from its target volume through a read-only mount.
The reviewer read `/private/tmp/p3-payload-evidence/outcomes.json` and caught.txt.
The raw counts match the exact-head log: 28 tested, 23 caught, five unviable, zero missed, and zero timeouts.
The baseline result is Success.
The artifact times, 23:36:37 through 23:37:23 UTC, match the focused job from round 44.

The raw outcome identifies `payload.rs:233:47: delete - in wait_unreaped_with` as CaughtMutant.
The reviewer checked that location against the evidence head: it is the unchanged negative fallback moved into the shared injected-wait loop.
This closes the original `payload.rs:224:47: delete - in wait_unreaped` entry under F13.
Its original name is preserved in `verdicts/p3-worker-mutants-4b1d39c-caught.txt`.
The default test supplies EINTR followed by ECHILD through the same loop that production uses.
No exclusion hides the fallback error path.

All 26 original payload entries now have closure: 23 caught and three accepted exact-function equivalences.
F13 retains 65 original real worker driver entries.
F18 and F19 remain OPEN for corrected-source Mac evidence. This Linux result does not close either Mac finding.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 65 entries; F18 and F19 open) on `39a55c9e8fd8ecc6d4678ba952a363d99fa04139`.


## Round 47 — Shared fixture for the mutated real driver

Reviewed head: `e4593e86feab06da3711bef9d3ac06bb7e20be38`.
The complete delta from `39a55c9` adds a shared session fixture, imports it into slow binary unit tests, and updates the handoff.
The integration test still launches the prebuilt candidate binary.
The unit fixture launches an exact test observer in current_exe, which cargo-mutants rebuilds with the changed Driver.
The observer supplies the fixture's instance, epoch, token, and control path to Driver::start, then calls Driver::run.
The same production functions install readiness and signal handling in both modes.
No production test branch, second Worker machine, behavior change, or exclusion accompanies the delta.

The reviewer compared the complete old fixture with the new shared file.
Apart from documentation, its guard module path, and executable selection, the session tests remain unchanged.
Both modes use the same control codec, payload guard, OwnedWorker cleanup, and operation assertions.
The helper module paths retain the exact names needed by the payload anchor and registration helper.
The fixture move can supply driver mutation evidence, but supplies no closure before that evidence exists.

### F20 — MEDIUM — The driver observer has no independent parent-death guard

Status: OPEN at this head.
Evidence: tests/common/session.rs, Session::launch and OwnedWorker; main.rs, driver_observer.

The unit fixture starts another test executable that runs the mutated Driver.
OwnedWorker::drop ends that observer on unwind, but outer test-process death does not run Drop.
PayloadGuard owns only the payload group, not the observer process.
The observer has no independent parent-lifetime mechanism or owned observer group.
A mutant that disables control reads or driver cleanup can leave the observer alive after its test parent dies.
The cleanup rule must not depend on the production function under mutation.
This is the same parent-death requirement retained in F17 for the payload reaping observer.

Required change: Give the driver observer an independent parent-lifetime guard that ends its owned group when the outer test dies.
Preserve retained worker ownership through its last signal and reap.
Preserve the independent payload group guard. The test guard must not reap the production payload.
Authority: the user's real-process rule that children exit when their parent is gone and guard cleanup remains independent of production.

F13 retains 65 original driver entries. F18 and F19 remain OPEN for corrected-source Mac evidence.
F20 remains OPEN. All earlier findings and closures remain preserved.
The reviewer inspected logic only and ran no tests or gate.

VERDICT: NOT CLEAN (F13: 65 entries; F18, F19, and F20 open) on `e4593e86feab06da3711bef9d3ac06bb7e20be38`.


## Round 48 — Shared driver fixture baseline

Reviewed and evidence head: `e4593e86feab06da3711bef9d3ac06bb7e20be38`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e4593e86-linux-20261004-164407-87814.log`.
The reviewer read the exact-head Linux binary-unit nextest log with --bin botster-worker and the slow profile.
All ten tests pass with zero skipped in 0.214 seconds. The job exits zero after ten seconds.
This supplies compilation and baseline evidence for the shared Driver observer fixture.
It does not exercise parent death while a mutation disables production cleanup, so F20 remains OPEN.
No mutant runs in this baseline. F13 retains 65 original driver entries.
F18 and F19 remain OPEN for corrected-source Mac evidence.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 65 entries; F18, F19, and F20 open) on `e4593e86feab06da3711bef9d3ac06bb7e20be38`.


## Round 49 — Independent lifetime guard for the driver observer

Reviewed head: `7efe4553d290cd6f76befc0d50ed7b5206b0ea0e`.
The complete delta from `e4593e8` changes the shared session fixture and its handoff.
No production Driver code, worker path, exclusion, dependency, or conflict resolution changes.
The integration candidate launch remains unchanged.

F20 is CLOSED in source.
The unit fixture creates GroupGuard before it spawns the observer shell in its own process group.
The shell waits for registration before exec starts the Driver observer.
The independent anchor joins the observer group before registration completes.
Parent death closes the anchor's control socket, so the anchor kills the observer group without running mutated Driver cleanup.
The payload keeps its separate PayloadGuard and production-only reaping.
OwnedWorker drops both guards before it queries or retires its retained worker Child.
Its last worker signal still occurs only while that child remains unreaped.

The new parent-death test starts a helper parent that launches a real Driver observer and payload.
It waits for that parent to report observer readiness, kills only the retained parent Child, and reaps that parent.
It then requires EOF on the inherited observer pipe within ten seconds.
The test never signals the recorded observer or payload PID.
The inherited pipe closes only when the observer and its inheriting descendants end.
The existing payload anchor control connection provides independent cleanup of the separately created payload group.
The parent-death test and correction still need exact-head compilation and runtime evidence.
Earlier-head mutation results cannot prove this new guard or regression test.

F13 retains 65 original driver entries pending mutation evidence.
F18 and F19 remain OPEN for corrected-source Mac evidence.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 65 entries; F18 and F19 open) on `7efe4553d290cd6f76befc0d50ed7b5206b0ea0e`.


## Round 50 — First direct-driver mutation evidence

Current reviewed head remains `7efe4553d290cd6f76befc0d50ed7b5206b0ea0e`.
Evidence head: `e4593e86feab06da3711bef9d3ac06bb7e20be38`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e4593e86-linux-20261004-164448-88453.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004164448-88453/target-mutants.out/`.
The reviewer read the exact-head log and actual outcomes.json.

The baseline result is Success. The job exits three after 481 seconds.
It tests 69 mutants: five caught, 41 missed, four unviable, and 19 timeouts.
All 65 original driver entries match exact names in these outcomes: five caught, 41 missed, and 19 timeouts.
The five caught entries are now CLOSED under F13.
Their names are preserved in `verdicts/p3-worker-mutants-e4593e8-caught.txt`.
They cover the zero read-chunk arithmetic, Driver::run no-op, two WouldBlock handling changes, and the flush counter subtraction.
The shared session assertions exercise the rebuilt Driver executable rather than an unchanged candidate.
The later observer guard preserves those assertions and improves independent cleanup.
This earlier run does not prove that later guard or its parent-death test.

The mutation tool sets a 20-second test budget, equal to the fixture's socket-read deadline.
A bounded fixture failure can therefore reach the external mutation budget before it reports its assertion failure.
The evidence does not classify any timeout as caught, and the reviewer accepts no timeout as closure.
The implementer must provide caught evidence or an accepted exact-function argument for each remaining entry.
Increasing an external mutation budget must not weaken the code test deadlines or conceal blocked cleanup.
No driver exclusion is proposed in this round.

F13 retains 60 original driver entries: 41 missed and 19 timed out.
F18 and F19 remain OPEN for corrected-source Mac evidence.
F20 remains CLOSED in source, with corrected-head evidence pending.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 60 entries; F18 and F19 open) on `7efe4553d290cd6f76befc0d50ed7b5206b0ea0e`.


## Round 51 — Exact-head driver observer cleanup evidence

Reviewed and evidence head: `7efe4553d290cd6f76befc0d50ed7b5206b0ea0e`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-7efe4553-linux-20261004-165400-93593.log`.
The reviewer read the exact-head Linux binary-unit nextest log with the slow profile.
All 18 tests pass with zero skipped in 0.215 seconds. The job exits zero after ten seconds.
The parent_death_ends_the_driver_observer regression passes in 0.007 seconds.
This supplies compilation and runtime evidence for the F20 correction accepted in round 49.
F20 remains CLOSED, with exact-head regression evidence now verified.

The implementer proposes pure readiness, deadline, and I/O error decisions used by the existing Driver.
That proposal is not yet a source delta and supplies no mutation closure.
Any extracted decision must remain on the actual Driver path and preserve one Worker machine and independent process cleanup.
The reviewer will inspect the exact source and relocation mapping when the implementer submits that head.

No mutant runs in this plain baseline. F13 retains 60 original driver entries: 41 missed and 19 timed out.
F18 and F19 remain OPEN for corrected-source Mac evidence.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 60 entries; F18 and F19 open) on `7efe4553d290cd6f76befc0d50ed7b5206b0ea0e`.


## Round 52 — Pure decisions on the existing real-driver path

Reviewed head: `7b54136568a88f1bbbf599de37002eb8daed363c`.
The complete delta from `7efe455` adds io_decisions.rs, changes actual Driver calls, and updates the handoff.
No exclusion, dependency, conflict resolution, second Worker machine, or production test branch accompanies the delta.
The reviewer compared every changed Driver decision with its prior expression and error arm.
No new source finding exists in this refactor.

The existing Driver calls the extracted decisions at their original operation points.
The original mutation responsibilities move as follows:

- The due-deadline comparison moves to due, retaining the inclusive <= boundary and absent-deadline behavior.
- Busy readiness and timeout selection move to ReadyState::timeout, retaining control/PTY readiness, drain presence, and queued input conditions.
- Poll interruption handling moves to poll_interrupted, retaining retry on Interrupted and propagation of every other poll error.
- Control read/write readiness accumulation moves to control_ready and writable_ready, retaining prior readiness and every event source.
- Interrupted/WouldBlock classification from read_control, read_pty_chunk, and flush moves to failure, retaining Retry, Blocked, and Closed effects.
- The control-read, PTY-read, initial flush, and continued-write gates move to read_control, read_pty, flush, and keep_writing.

The Driver still performs the same reads, writes, readiness clearing, drain updates, link-loss actions, and progress reporting.
The pure default tests cover all readiness combinations, exact deadline boundaries, and distinct interrupted/blocked/error cases.
The actual Driver uses those tested functions; the tests do not select another production path.
Direct adapter effects, caller wiring, and the remaining unchanged operations still require evidence.
A caught relocated decision must map back to its original entry before that entry closes.
Removal of an old mutation location by itself supplies no closure.

The handoff reports the lead's release of the Mac hold for one queued payload diagnostic.
That job uses the corrected payload tests with a twenty-minute whole-job deadline and unchanged ten-second code test deadlines.
No result exists in this round. F18 and F19 remain OPEN pending that evidence and any required correction.
F13 retains 60 original driver entries. F20 remains CLOSED with regression evidence from round 51.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 60 entries; F18 and F19 open) on `7b54136568a88f1bbbf599de37002eb8daed363c`.


## Round 53 — Corrected-source Mac diagnostic still fails

Reviewed and evidence head: `7b54136568a88f1bbbf599de37002eb8daed363c`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-7b541365-mac-20261004-165836-99257.log`.
The reviewer read the exact-head Mac nextest log for the corrected slow_payload tests.
The job exits 124 after 1201 seconds. Fifteen tests run: 13 pass, two fail, and zero are skipped.
The panic-cleanup test fails after 10.127 seconds.
The queued-output/query/input test remains alive until external SIGTERM after 1197.209 seconds.
No mutation result follows from this diagnostic.

F18 remains OPEN as a production-query finding under the corrected synchronization proof.
The queued-output FIFO marker arrives before the test calls the unchanged production pending_output query.
The diagnostic prints pending_output=0 after that marker, then the query assertion fails.
The previous incomplete-output readiness explanation no longer resolves the failure.
Production uses this query as its exit-drain bound, so its Mac semantics require investigation and correction.
Keep the queued-output and complete-output proof. Do not weaken it or bypass the query for tests.

F19 remains OPEN despite the explicit group join and independent guard.
The panic test times out while guard cleanup and production reaping run in a separate thread.
Its process-state snapshot reports test PID 99480 and recorded payload PID 99486.
It reports leader PID 99486, PPID 99480, PGID 99486, state ?Es.
It reports anchor PID 99490, PPID 99486, PGID 99486, state ?E.
Thus the snapshot shows the anchor in the leader's group during this failure.
The query test also blocks during panic cleanup until the external job deadline terminates it.
The log does not identify the blocking cleanup operation or explain the reported process states.
Investigate those operations and native PTY behavior before another correction claims finite Mac cleanup.
Retain independent group ownership, parent-death cleanup, and production-only payload reaping.

The corrected-source evidence now exists and fails. F18 and F19 require further correction, not only another proof run.
F13 retains 60 original driver entries. F20 remains CLOSED with verified parent-death evidence.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 60 entries; F18 and F19 open) on `7b54136568a88f1bbbf599de37002eb8daed363c`.


## Round 54 — Mac PTY query and cleanup correction

Reviewed head: `e9efad5e788754bfc3c545b3bdfbfc5333238130`.
The complete delta from `7b541365` changes payload ownership/query code, Driver drain errors, direct tests, two manifests, Cargo.lock, and the handoff.
The reviewer inspected all nine changed files and the local Apple and published kqueue 1.2.1 sources.
No conflict resolution, handwritten terminal parser, unsafe project code, second Worker path, or exclusion accompanies the delta.

The Apple tty source routes FIONREAD through ttnread, which counts canonical/raw input.
The master PTY EVFILT_READ path supplies its readable output count from t_outq.c_cc.
The new Mac pending_output uses a fresh kqueue Watcher on the retained master fd and samples one zero-wait read event.
It consumes no PTY bytes. Watcher::drop does not close descriptors registered through add_fd.
The library maps a failed kevent poll to an Error event; the project propagates that error.
Registration errors also propagate. A successful poll with no event returns zero.
Driver::perform now propagates a pending-output query error instead of treating it as a zero-byte drain.
The repeated-count test retains the queued-output proof and checks that the query consumes no bytes.
F18 remains OPEN pending corrected-source Mac evidence.

The Apple exit source calls ttywait before terminal revocation, which supports the proposed drain dependency.
Payload::drop now sends group SIGKILL, closes its owned master, then waits for its own leader.
Payload::reap also closes the master before waiting.
The independent guard requests anchor cleanup and returns instead of waiting for anchor EOF while production still holds the master.
The anchor retains group membership through its final signal to its current group.
Production alone reaps the payload. No test guard takes that reap.
This source removes the proposed master-close dependency from guard completion.
F19 remains OPEN pending corrected-source Mac proof of finite panic cleanup.

The reviewer rechecked all three existing payload equivalences after the ownership change.
Payload::reap no-op remains ACCEPTED: current callers observe exit and send group SIGKILL first.
The no-op consumes self into Drop, which retains the leader through an extra SIGKILL, closes the master, then waits.
The already reported exit cannot change, and the group id remains reserved through the signal.
The wait-option OR-to-XOR and fresh-blocking-descriptor OR-to-XOR arguments remain unchanged and ACCEPTED.
No original payload closure is revoked. New query and cleanup mutations still require evidence.

Direct slow Driver tests now cover control EOF, link loss, retained partial writes, totals, interest, PTY readiness, drain completion, and deregistration.
They call the existing Driver methods and keep independent payload cleanup ahead of production reap.
The payload is never removed into an unguarded local owner.
One failure path in the partial-write test remains below.

### F21 — MEDIUM — The partial-write test can busy-loop without progress

Status: OPEN at this head.
Evidence: tests/common/driver_edges.rs, partial_writes_retain_bytes_and_track_interest_and_totals.

The loop accepts a peer read that returns WouldBlock without waiting for readiness.
It then forces control_writable=true, calls flush, and repeats while the received byte count is incomplete.
If neither operation progresses, the loop repeats immediately until its deadline.
A mutation that prevents writes reaches that path. The deadline bounds elapsed time but does not prevent a busy loop.

Required change: Wait on descriptor readiness when the loop makes no progress.
Retain the finite deadline, complete-byte ordering, written totals, and write-interest assertions.
Use the existing Driver path for writes. Do not add a test branch to production.
Authority: BUILD.md's no-polling test rule and the bounded real-process test requirements.

The reviewer verified the exact-head Linux baseline log:
`~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e9efad5e-linux-20261004-172920-12855.log`.
All 44 selected slow_payload and binary tests pass, zero skipped, in 0.234 seconds. The job exits zero after ten seconds.
The explicit filter excludes three other binaries. This is not a full gate or Mac proof.
F13 retains 60 original driver entries. F18, F19, and F21 remain OPEN.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 60 entries; F18, F19, and F21 open) on `e9efad5e788754bfc3c545b3bdfbfc5333238130`.


## Round 55 — Corrected Mac query and panic cleanup pass

Reviewed and evidence head: `e9efad5e788754bfc3c545b3bdfbfc5333238130`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e9efad5e-mac-20261004-173007-13507.log`.
The reviewer read the exact-head corrected Mac slow_payload nextest log.
All 15 tests pass, zero skipped, in 0.089 seconds. The job exits zero after four seconds.
The Mac-only kqueue path compiles and runs in this proof.

F18 is CLOSED on this exact head.
The FIFO-synchronized queued-output query/input test passes in 0.027 seconds.
It checks a positive queued count, repeats the query without consuming output, delivers input, and retains complete program output.
The query now uses the Mac master-read filter instead of the input-queue ioctl.
The source and this runtime proof establish the correction of the observed Mac query failure.

F19 is CLOSED on this exact head.
The panic-while-waiting-for-input test passes in 0.034 seconds within its unchanged ten-second deadline.
The independent guard returns after its cleanup request, then production closes the master before waiting for its leader.
The isolated payload reaping test passes in 0.040 seconds.
These results establish finite cleanup and production-only reaping on the observed Mac failure path.
The independent anchors retain group ownership and parent-death cleanup.
The three payload equivalence arguments remain ACCEPTED after the source recheck in round 54.

This plain baseline supplies no original driver mutation closure.
F13 retains 60 original driver entries. F21 remains OPEN in the partial-write test.
New query/cleanup code still requires mutation accounting in the applicable gate.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: 60 entries; F21 open) on `e9efad5e788754bfc3c545b3bdfbfc5333238130`.


## Round 56 — Relocated decisions and direct driver mutation evidence

Evidence head: `e9efad5e788754bfc3c545b3bdfbfc5333238130`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e9efad5e-linux-20261004-173244-15398.log`.
Artifacts: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004173244-15398/target-mutants.out/`.
The reviewer read the exact-head log and actual outcomes.json.
The baseline result is Success. The raw job tests 73 mutants: 63 caught, five unviable, three missed, and two timeouts.
The helper file has 42 caught mutations and one unviable mutation, with no miss or timeout.

The reviewer mapped original entries by source responsibility, not only operator text.
Thirty-nine original entries moved to the decision functions reviewed in round 52.
Their source responsibilities and exhaustive default assertions match the caught helper mutations:

- Original due comparison at 149 maps to due and its inclusive boundary test.
- Original busy conditions at 159–162 map to ReadyState::timeout and all 64 state combinations.
- Original poll error guards at 172 map to poll_interrupted and success/interrupted/error cases.
- Original accumulated read/write flags at 178–180 map to control_ready and writable_ready, with every event combination.
- Original PTY/control gates at 321/358 map to read_pty/read_control and every readiness/drain combination.
- Original I/O classifications at 335–336, 365–366, and 386–387 map to failure and interrupted/blocked/closed cases.
- Original flush entry gate at 374 maps to flush; loop conditions at 378 map to keep_writing and every readiness/byte case.

The actual Driver uses each tested decision and retains the reviewed effects for its result.
Two of these 39 original entries already closed in round 50. The other 37 now close under F13.
The unchanged adapter decisions have 21 caught original entries, matched by location role after line shifts.
Three already closed in round 50. The other 18 now close under F13.
The mapping distinguishes the written-total comparison from the later interest comparison, which share operator text.
It also distinguishes initial flush, continued write, and interest negations.
The 55 newly closed original names are preserved in `verdicts/p3-worker-mutants-e9efad5-closed.txt`.
Removal of a location alone is not the basis for any closure.

F13 retains five original driver entries:

- READ_CHUNK multiplication changed to addition: missed.
- main no-op: missed.
- Driver::run PTY event arm deletion: missed.
- Driver::flush written += changed to *=: timeout.
- Driver::flush written != changed to ==: timeout.

The two timeouts remain open despite direct accounting assertions. No timeout counts as caught.
New query/cleanup mutations still require accounting in the applicable gate.
F21 remains open; these outcomes do not establish a correct no-progress wait in that test.
The reviewer ran no tests or gate.

## Round 57 — P6 merge, manifest correction, and remaining partial-write loop

Reviewed head: `1c45103cd0d63ddef9cccb5c6934442ff0e4ea29`.
The delta from `e9efad5` includes readiness change ff1ff05, merge 2a88d7a, its handoff, and the manifest correction.
Merge commit `2a88d7a0eb04454bc9d96a0ae55b0fae6dd41d2f` imports v1 `e8cf15068825888795f3ff2582d98b5c8e9b09e4` (PR #140).
Both named conflict resolutions are reviewed:

- Cargo.lock keeps both P3 worker-core and P6 sha2 dependencies. The resulting lockfile equals the prior P3 lockfile.
- conformance/core-pending.txt keeps every pending id, adds the P6 snapshot comments, and updates TI-1's testkit-wiring comment.

The six imported P6 implementation/design/test blobs match merged v1 exactly.
The combined lib.rs keeps P6 exports plus the existing P3 core/worker modules and re-exports.
Existing P3 core, worker, harness, and candidate/refusal implementations remain unchanged.
The new oracle helpers add no M1 producer, terminal parser, or alternate Worker path.
Their every-cut fit evidence still awaits M2b paging.
No new mutation exclusion accompanies this merge.

The automatic manifest merge introduced the duplicate terminal dependency reported as integration I3 HIGH on ae039b52.
Correction 1c45103 removes the duplicate path entry and preserves the existing workspace binding declaration.
The reviewer parsed the corrected workspace, testkit, sys, and worker manifests successfully.
The corrected testkit manifest equals its prior P3 version. Integration I3's source defect is corrected at this head.
The failed ae039b52 job stopped before tests and supplies no test or mutation evidence.

F21 remains OPEN after ff1ff05.
The loop polls peer IN and queued control OUT with the remaining deadline, then uses reported writability for flush.
A flush no-op can leave the empty socket writable and outbound bytes queued.
OUT stays level-ready, so poll returns immediately, the peer read returns WouldBlock, and flush makes no progress.
The loop still repeats until its deadline without blocking.
Required change: Fail promptly when reported readiness produces no read/write progress, or wait on a condition that cannot remain ready without progress.
Retain the deadline, complete bytes, totals, interest assertions, and existing production flush path.

F13 retains five original driver entries. F21 remains OPEN.
F18/F19 closures and all earlier findings remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: five entries; F21 open) on `1c45103cd0d63ddef9cccb5c6934442ff0e4ea29`.


## Round 58 — Direct assertions catch both former flush timeouts

Reviewed and evidence head: `1c45103cd0d63ddef9cccb5c6934442ff0e4ea29`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-1c45103c-linux-20261004-175018-23825.log`.
Raw evidence: `/private/tmp/p3-driver-flush-evidence/`, retrieved from the completed volume without a rerun.
The reviewer read the exact-head log, actual outcomes.json, and both relevant mutation failure logs.

The baseline passes with six selected direct entries. Twenty-three entries lie outside the explicit filter.
The mutation job uses a 120-second external budget, unchanged code deadlines, and nextest fail-fast.
The raw result tests nine mutations: seven caught, two unviable, zero missed, and zero timeouts.
The written += to *= mutation at main.rs:390:34 fails written > 0 at driver_edges.rs:175 in 0.007 seconds.
The written != to == mutation at main.rs:402:25 fails inputs.is_empty at driver_edges.rs:170 in 0.007 seconds.
Both raw outcomes are CaughtMutant. These are actual bounded assertion failures, not external tool timeouts.

Both original flush entries are now CLOSED under F13.
The reviewer maps the changed line numbers by their written-total roles in the reviewed unchanged flush implementation.
Their original names are preserved in `verdicts/p3-worker-mutants-1c45103-caught.txt`.
The failed assertions occur before the partial-write completion loop, so they do not depend on F21's no-progress path.
F13 retains three original misses: READ_CHUNK addition, main no-op, and Driver::run PTY arm deletion.
New query/cleanup code still requires mutation accounting in the applicable gate.

F21 remains OPEN pending review of the proposed progress assertion on a pushed exact head.
The implementer reports that each ready turn will require increased received bytes or written totals.
That proposal supplies no source closure in this round.
All earlier findings and closures remain preserved. The reviewer ran no tests or gate.

VERDICT: NOT CLEAN (F13: three original entries; F21 open) on `1c45103cd0d63ddef9cccb5c6934442ff0e4ea29`.

## Round 59 — Progress assertion and authenticated PTY fixture retirement

Reviewed head: `e475c2841368c6b71bfaf0497b7d06b4d3e0fab9`.
The delta from 1c45103 changes the direct Driver fixture and its handoff only.
No production code, mutation exclusion, or merge conflict changes.

F21 is CLOSED.
Each partial-write turn now requires increased received bytes or changed written totals.
A ready socket with a no-op flush therefore fails an assertion instead of repeating without progress.
The fixture retains its readiness poll, deadline, complete-byte comparison, totals, interest checks, and production flush call.
The reviewer read the actual 7c2503e0 baseline log:
`~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-7c2503e0-linux-20261004-175451-26664.log`.
The partial-write test passes in 0.007 seconds. Its source remains unchanged at the reviewed head.

The new PTY fixture starts a guarded real payload with ready, go, and done FIFOs.
The fixture calls production read_pty_chunk before releasing output and checks that WouldBlock clears readiness.
The real Driver::run must resume reads before the program can complete four MiB of PTY output.
The done FIFO checks program progress. It supplies no terminal-byte expectation or terminal-semantics proof.
The fixture uses blocking readiness waits with deadlines and a finite output program.
The independent guard still kills the payload group without reaping the production payload.
Harness field order replaces its custom Drop and releases the guard before the production reaper.
The fixture can move Driver into its real loop thread without adding another worker path.

The earlier 7c2503e0 baseline runs seven selected tests: six pass and the PTY test fails retirement at 10.017 seconds.
The failure occurs at the retirement channel after the done FIFO marker. No mutants run.
That fixture closed the control link, which does not retire the worker under DP-8.
Correction e475c284 sends authenticated Hello and Remove through the existing frame codec.
The correction retains the FIFO proof, independent guard, and bounded retirement channel.
Corrected PTY execution and mutation evidence remain pending. The earlier failure closes no original mutation entry.

F13 retains three original entries: READ_CHUNK addition, main no-op, and Driver::run PTY arm deletion.
New query/cleanup code still requires mutation accounting in the applicable gate.
All earlier findings and closures remain preserved. The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F13: three original entries) on `e475c2841368c6b71bfaf0497b7d06b4d3e0fab9`.

## Round 60 — PTY readiness assertion catches the original arm deletion

Reviewed and evidence head: `e475c2841368c6b71bfaf0497b7d06b4d3e0fab9`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-e475c284-linux-20261004-175646-27658.log`.
Raw evidence: `/private/tmp/p3-driver-pty-evidence/`, copied from the completed volume without a rerun.
The reviewer read the exact-head log, outcomes.json, baseline log, and PTY mutation failure log.

The baseline passes all seven selected direct entries. Twenty-three entries lie outside the filter.
The corrected PTY fixture passes in 0.015 seconds. The partial-write test passes in 0.008 seconds.
The mutation job tests ten entries: eight caught, two unviable, zero missed, and zero timeouts.
Its external mutation budget is 120 seconds. The fixture keeps its ten-second progress deadline.
Deleting the PTY arm at main.rs:184:21 fails the done-marker poll assertion at driver_edges.rs:287 in 10.019 seconds.
The actual test result is Failure(100), and the raw outcome is CaughtMutant.
This is a fixture assertion failure after a bounded readiness wait. It is not an external tool timeout.

The original main.rs:182:21 PTY arm deletion is CLOSED under F13.
The reviewed source maps that original entry to the unchanged PTY readiness assignment at main.rs:184:21.
Its original name is preserved in `verdicts/p3-worker-mutants-e475c28-caught.txt`.
The seven caught flush entries repeat existing closures and add no further original-entry closure.
F13 retains two original entries: READ_CHUNK addition and main no-op.
New query/cleanup code still requires mutation accounting in the applicable gate.
F21 remains CLOSED. All earlier findings and closures remain preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F13: two original entries) on `e475c2841368c6b71bfaf0497b7d06b4d3e0fab9`.

## Round 61 — Command-line decisions and unguarded process tests

Reviewed head: `c731d03e0fbe2ad73d8f152ec67525c09c66f833`.
The delta from e475c284 extracts command-line decisions, shares the candidate path helper, and adds two process tests.
No merge conflict or mutation exclusion changes.

command_line::execute still uses WorkerLaunch::parse and calls the injected Driver only after successful parsing.
It preserves usage status 2, success status 0, driver failure status 1, and error text.
Main supplies the same Driver::start followed by Driver::run, then prints the error with the same prefix.
The default tests check refusal without Driver calls, exact launch identity, success, driver failure, and missing-token refusal.
This extraction creates no second Worker path or production test branch.
The shared candidate helper preserves the prior prebuilt-binary path algorithm.
Compilation, boundary execution, and default mutation evidence remain pending.
Main's proposed process-glue classification supplies no original mutation closure before that evidence and classification review.

### F22 MEDIUM — New command-line process tests lack independent group cleanup

Status: OPEN on c731d03.
Location: `crates/botster-worker/tests/slow_cli.rs`, both Command::output calls.
Each test starts a real worker with Command::output and relies on that call to wait for exit.
Neither test establishes an independent process-group guard for panic or test-parent death.
If the worker remains alive, the parent blocks in output without independent cleanup or a fixture deadline.
Expected immediate refusal does not replace ownership of the real process.
pair-common.md requires every real-process test to own and clean up its process group.
The resumed review instructions also require guard cleanup when the test parent is gone.

Required change: Start each worker through an independent process-group guard with parent-death cleanup.
Keep the worker Child owned by the test, and let the test reap that worker.
The guard must not reap the worker or signal a cached group after the worker is released.
Bound the wait without polling or busy-spinning children.
Retain the prebuilt candidate, status, stdout, and stderr assertions.

F13 retains READ_CHUNK addition and main no-op. New query/cleanup mutations still need accounting.
F21 remains CLOSED. All earlier findings and closures remain preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F13: two original entries; F22 open) on `c731d03e0fbe2ad73d8f152ec67525c09c66f833`.

## Round 62 — Guarded CLI tests and positive read bounds

Reviewed head: `3fd4144515e8e903d6417adf56d12b7f038d8b70`.
The reviewer inspected the complete delta from c731d03. No merge conflict or mutation exclusion changes.

F22 is CLOSED in source.
The CLI fixture creates the existing GroupGuard before spawning a shell in a new process group.
The shell completes group registration before exec of the prebuilt candidate.
A test thread owns the worker Child and calls wait_with_output. The guard reaps only its own anchor.
The anchor retains group membership through the worker reap, preventing reuse of the group identifier before cleanup.
The guard's socket closes on Drop or parent death. The anchor then kills the group it still owns.
A ten-second channel deadline bounds the worker wait without polling.
On deadline failure, guard cleanup starts before the fixture raises its assertion failure.
The fixture retains the exact launch environment and output/status assertions.
Corrected guarded execution evidence remains pending.

READ_CHUNK now has type NonZeroUsize and retains default 65536.
Driver::start delegates to start_with_read_bound. Both control and PTY reads use that field through the same code.
The type enforces a positive bound in every build. No production test branch or second worker path appears.
The new direct test checks control-byte order, complete decoded frames, and program-output retention at 65536, 1088, and 1.
Each read must produce positive progress within its bound. A missing input fails promptly instead of polling.
The test checks WouldBlock readiness clearing after consuming each finite stream.
The program-output literal is data from printf, not an expected terminal encoding.
Passing execution at all three bounds remains pending. No constant exclusion is accepted in this round.

The reviewer read the c731d03 command-line evidence log and raw outcomes:
`~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-c731d03e-linux-20261004-180200-33062.log` and `/private/tmp/p3-command-line-evidence/`.
The default baseline passes seven tests, zero skipped. All three generated execute mutations are CaughtMutant.
Their actual logs fail launch-status or Driver-call assertions with Failure(100), without external timeouts.
The command_line.rs implementation and default tests remain unchanged at the reviewed head.
The earlier unguarded boundary tests pass, but do not verify the corrected guard fixture.
The explicit candidate.rs path also corrects the mutation scanner's reported missing module referent.

Main still contains the real Driver call, argument/environment reads, error printing, and status return.
The execute evidence verifies the extracted decisions. It does not catch a no-op main in the prebuilt candidate.
Main's proposed exact-function process-glue classification remains pending corrected boundary evidence and an explicit exclusion reason.
F13 retains two original entries: READ_CHUNK addition and main no-op.
New query/cleanup mutations still need accounting in the applicable gate.
F21 remains CLOSED. All earlier findings and closures remain preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F13: two original entries) on `3fd4144515e8e903d6417adf56d12b7f038d8b70`.

## Round 63 — Guarded baseline and proposed exact exclusions

Reviewed and evidence head: `3fd4144515e8e903d6417adf56d12b7f038d8b70`.
The reviewer read the exact-head Linux log:
`~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-3fd41445-linux-20261004-180554-37672.log`.
The job prebuilds the candidate at that head, then runs the worker binary tests and slow_cli.
All 41 selected tests pass, zero skipped, in 0.229 seconds. The job exits zero.
Both guarded prebuilt CLI status tests pass in 0.005 seconds.
The guard's panic, parent-death, and retained-group tests pass in this selection.
The three-bound retention test passes in 0.012 seconds. PTY readiness and retirement pass in 0.014 seconds.
F22's corrected guarded execution is now verified. F22 remains CLOSED.

The reviewer conditionally accepts the proposed READ_CHUNK tuning classification under the lead's round 27 rule.
No contract clause fixes a control or PTY read to 65536 bytes or requires a frame in one read.
The same Driver code now proves retention, ordering, and complete control frames at 65536, 1088, and the legal minimum 1.
NonZeroUsize enforces the positive lower bound in every build.
The proposed exclusion must name only this constant's verified arithmetic expression.
Its reason must cite the absence of a fixed contract value, positive bounds, and control_and_pty_reads_retain_bytes_at_each_positive_bound.
A file-wide unnamed arithmetic pattern would remain unacceptable.

The reviewer conditionally accepts main as process glue at this exact head.
Main reads process arguments and the token environment, supplies the real Driver call, prints errors, and returns the selected status.
WorkerLaunch::parse still owns parsing. command_line::execute owns refusal and status decisions and remains mutation-tested.
The raw execute evidence catches all three generated replacements through actual assertions.
The exact-head prebuilt tests verify usage status 2 and connection-failure status 1.
The unchanged real Driver session path also passes successful retirement with status 0 in this baseline.
An exclusion must name main alone and record this division and these proving tests.
This classification does not apply to Driver, command_line::execute, or another function in main.rs.

Neither exclusion exists yet. Both original entries remain OPEN under F13 until their exact entries and reasons are reviewed.
New query/cleanup code still requires mutation accounting in the applicable gate.
All earlier findings and closures remain preserved. The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F13: two original entries) on `3fd4144515e8e903d6417adf56d12b7f038d8b70`.

## Round 64 — Linux query and cleanup mutation accounting

Reviewed and evidence head: `3fd4144515e8e903d6417adf56d12b7f038d8b70`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-3fd41445-linux-20261004-180653-38263.log`.
Raw evidence: `~/botster-sessions/gates/artifacts-trybotster_botster_core_stage1_p3_m1_v1-bcb322fa-20261004180653-38263/target-mutants.out/`.
The reviewer read the exact-head log and raw outcomes, plus the query-helper and Drop failure logs.

The Linux baseline passes. The job tests 32 mutations: 24 caught, five unviable, three missed, zero timeouts.
Both replacements of the new non-Mac pending_output helper at payload.rs:224:5 are CaughtMutant.
Ok(0) and Ok(1) each fail the pending > 1 assertion in the real PTY test in 0.005 seconds.
The current Payload::pending_output replacements are also caught.
The current Payload::drop replacement at payload.rs:210:9 is caught by the isolated production-reap observer in 0.007 seconds.
Its failure log contains actual waitid and observer-success assertion failures, with Failure(100).
These results account for the Linux query helper and confirm the existing Drop closure after the master-close change.
The three exact-function payload equivalence arguments remain as reviewed in rounds 54 and 55.

Two misses replace the Mac-only pending_output helper at payload.rs:230:5 with Ok(0) or Ok(1).
The Linux build does not compile that helper, so these results establish no native helper coverage.
Both native entries remain pending Mac mutation evidence. Omitting inactive entries from a focused Linux command grants no config exclusion.
The third miss deletes the fallback minus at wait_unreaped_with:279:47.
The focused command selects slow_payload and omits the library test that caught this unchanged fallback in round 46.
The earlier original-entry closure remains preserved. This selection does not repeat its proof at the current head.
The implementer proposes a library-inclusive Linux selection and native Mac evidence. Those results remain pending.

F13 retains the two original driver entries pending exact exclusions, plus the native query-helper accounting described above.
F21 and F22 remain CLOSED. All earlier findings and closures remain preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN on `3fd4144515e8e903d6417adf56d12b7f038d8b70`.

## Round 65 — Exact main and READ_CHUNK exclusions

Reviewed head: `63b5c1db6cec74b27c7b10d34cbbb37e99e60986`.
The complete delta from 3fd41445 changes only .cargo/mutants.toml and the handoff.
Production and test source remain identical to the passing baseline reviewed in round 63.
No merge conflict resolution accompanies this delta.

The READ_CHUNK exclusion is ACCEPTED under the lead's four-condition tuning rule.
Its pattern names only main.rs:47:55 and the multiplication-to-addition mutation.
The reviewer verified that column 55 is the arithmetic expression in this constant declaration.
Its reason cites partial reads, all-build positivity, and the named retention test at 65536, 1088, and 1.
The exact-head source and supplied passing proof satisfy the conditions reviewed in rounds 62 and 63.

The main exclusion is ACCEPTED as exact-function process glue.
Its pattern matches main replacements and mutations in main, and does not cover Driver or command_line::execute.
Its reason records parsing and status decisions outside main, caught execute mutations, guarded CLI boundary proof, and successful session retirement.
The reviewer parsed the TOML and checked both patterns against matching and unrelated example mutation names.
Both entries have one constant or function, a reason, and named proving tests.
Both entries require review if their source roles change. No other exclusion changes.

The original READ_CHUNK addition and main no-op entries are now CLOSED under F13.
All 167 original entries now retain a caught or accepted-equivalence/process-glue disposition in this verdict history.
This count does not establish native coverage for the later Mac query helper.
F13 remains OPEN for that new helper's two generated replacements until native mutation evidence arrives.
The proposed library-inclusive Linux fallback check also remains pending; its earlier original closure is preserved.
F21 and F22 remain CLOSED. All earlier findings and closures remain preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F13: native query-helper accounting pending) on `63b5c1db6cec74b27c7b10d34cbbb37e99e60986`.

## Round 66 — Library-inclusive Linux payload proof

Reviewed and evidence head: `63b5c1db6cec74b27c7b10d34cbbb37e99e60986`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-63b5c1db-linux-20261004-181046-41087.log`.
Raw evidence: `/private/tmp/p3-payload-corrected-linux-evidence/`, copied from the completed volume without a rerun.
The reviewer read the exact-head log, outcomes.json, baseline log, and fallback mutation failure log.

The corrected command selects library tests and slow_payload. Its temporary filter omits only the inactive Mac helper at payload.rs:230:5.
The baseline passes 47 tests, zero skipped, in 0.062 seconds.
The job tests 30 mutations: 25 caught, five unviable, zero missed, and zero timeouts. The job exits zero.
The fallback-minus deletion at wait_unreaped_with:279:47 is CaughtMutant.
The library test an_interrupted_watch_retries_and_a_failed_watch_reports_unknown_exit fails its status assertion in 0.003 seconds, with Failure(100).
This repeats the preserved original fallback closure at the current head and resolves the omitted-selection evidence gap.
Both non-Mac helper replacements, both Payload::pending_output replacements, and the Payload::drop replacement remain caught in the raw outcomes.
The Linux query/cleanup accounting reviewed in round 64 is therefore verified at this exact head.

The temporary Linux filter grants no config exclusion or native coverage.
F13 remains OPEN only for the later Mac query helper's two generated replacements.
The lead-authorized native job is pending. The reviewer does not request or run a gate.
All 167 original-entry dispositions and all earlier findings and closures remain preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F13: native query-helper accounting pending) on `63b5c1db6cec74b27c7b10d34cbbb37e99e60986`.


## Round 67 — Native helper accounting and exact-head package CLEAN

Reviewed and evidence head: `63b5c1db6cec74b27c7b10d34cbbb37e99e60986`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-63b5c1db-mac-20261004-181240-42534.log`.
Raw evidence: `/private/tmp/p3-payload-mac-evidence/`, copied from completed output without a rerun.
The reviewer read the exact-head log, outcomes.json, baseline log, and both mutation failure logs.

The native job uses --in-diff and selects only the Mac pending_output helper at payload.rs:230..253.
It runs the existing library and slow_payload tests with unchanged code deadlines and a 120-second mutation budget.
The baseline passes 47 tests, zero skipped, in 0.464 seconds.
Both generated replacements, Ok(0) and Ok(1) at payload.rs:230:5, are CaughtMutant.
Each fails the real PTY test's pending > 1 assertion in 0.027 seconds, with Failure(100).
The job has zero missed, zero timeouts, and zero unviable entries. It exits zero after 16 seconds.
These actual native assertion failures close the later Mac helper accounting. No new exclusion applies.

F13 is CLOSED. All 167 original mutation entries retain their recorded dispositions.
Later Linux and native query/cleanup accounting is complete for the reviewed generated entries.
All findings F1 through F22, including LOW findings, are CLOSED at this exact head.
Every earlier finding and closure remains in this document.
The named restack and merge resolutions remain as reviewed in the preceding rounds.
The contracts authority remains contracts-v0.1.13, final A13, R-28, and erratum 2, with binding BUILD.md and the restack plan.

This CLEAN covers P3's M1 package scope and its delta from the old reviewed M1.
The cross-package PR still requires the separate integration reviewer's exact-head CLEAN.
The implementer then runs the plan's gate on the head cleared by both reviewers.
M2a and M2b restack review remain separate milestones. The existing real-harness conformance pending list remains explicit.
The reviewer ran no tests, builds, or gates.

VERDICT: CLEAN (P3 M1 package scope) on `63b5c1db6cec74b27c7b10d34cbbb37e99e60986`.


## Round 68 — Per-function OS adapter exclusions

Reviewed head: `3f9fe56ed466bf9f3ed7fde827a80168b69c3046`.
The complete delta from 63b5c1db changes only .cargo/mutants.toml and the handoff.
Production and test source remain identical to the exact-head proof reviewed through round 67.
No merge conflict resolution accompanies this delta.

The reviewer accepts all 24 new per-function OS adapter entries.
The landing mutation command in xtask::ci selects the default tier without slow features.
That tier does not create the real sockets, PTYs, or children required to execute these adapters.
The entries preserve the existing slow and native mutation proof instead of claiming that their mutations are equivalent.
Each entry names one function, its real-process proof, and the recorded evidence set.
The native/non-native pending_output implementations share one helper name with mutually exclusive target conditions; both native bodies have recorded proof.

The accepted Driver entries are start, start_with_read_bound, run, settle, perform, spawn, deregister_pty, read_pty_chunk, read_control, flush, link_lost, and drop_link.
Their reasons cover real descriptor construction, event dispatch, action execution, byte progress, readiness, shutdown, and production cleanup.
The existing lifecycle, direct descriptor, PTY readiness, and partial-write tests supply their recorded slow proof.
The constructor extraction retains the same body and passes the three-bound baseline.
The dispatcher functions settle and perform forward the shared machine's inputs/actions to these real edges; their protocol decisions remain in Worker.

The accepted Payload entries are spawn, pid, master, read, pending_output, write, watch_exit, signal_group, Drop::drop, and the helpers pending_output, wait_unreaped, and set_nonblocking.
Their reasons cover real descriptor/process ownership, reads/writes, native output queries, unreaped exit watches, group signals, master closure, and production reaping.
The library-inclusive Linux and native Mac mutation evidence remains as reviewed in rounds 66 and 67.
Payload::reap retains its earlier exact no-op equivalence entry and receives no broad new exclusion.

The reviewer parsed the TOML and checked every new pattern's function scope.
The patterns do not match command_line::execute, io_decisions, errno_of, exec_failure, wait_unreaped_with, or Payload::reap.
Worker-core remains mutation-tested. The main and READ_CHUNK entries remain unchanged from round 65.
No file or crate exclusion is added. A changed function body or caller requires review of its recorded reason.
No new finding arises from this configuration delta.

All findings F1 through F22 remain CLOSED. Every earlier finding, closure, and original mutation disposition remains preserved.
This CLEAN covers P3's M1 package delta. The cross-package PR requires the integration reviewer's exact-head CLEAN and the implementer's landing gate.
The reviewer ran no tests, builds, or gates.

VERDICT: CLEAN (P3 M1 package scope) on `3f9fe56ed466bf9f3ed7fde827a80168b69c3046`.


## Round 69 — Integration I4 and incomplete exclusion proof names

Reviewed head: `3f9fe56ed466bf9f3ed7fde827a80168b69c3046`.
The integration reviewer reported I4 LOW after the package issued round 68's CLEAN.
The finding concerns this package's new exclusion comments, so the package records it as F23.
Round 68's CLEAN is superseded at this head. Its pattern-scope and completed mutation-evidence conclusions remain preserved.

### F23 LOW — Several new exclusion reasons lack concrete test names

Status: OPEN. Corresponds to integration I4.
Location: the new Driver and Payload entries in .cargo/mutants.toml.
Several comments name slow_driver or slow_payload suites, lifecycle/exit tests, or group/descriptor checks instead of concrete test functions.
Examples include Driver::start, Driver::settle, Driver::spawn, Payload::spawn, Payload::pid, Payload::master, and Payload::watch_exit.
The integration reviewer cites the plan's requirement for a named slow test where applicable.
The suite descriptions identify the intended evidence but do not satisfy that concrete-name requirement.
The package's round 68 statement that every entry has a named proof was too broad for these comments.

Required change: Give the concrete proving test function names in each affected exclusion reason.
Keep each function-specific pattern, completed evidence reference, and pure-decision mutation coverage unchanged.
This finding requires no new test or gate. The reviewer will inspect the corrected exact head.

F1 through F22 remain CLOSED. All earlier findings, closures, and evidence remain preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F23 LOW open) on `3f9fe56ed466bf9f3ed7fde827a80168b69c3046`.


## Round 70 — Concrete test names and removed equivalence reasons

Reviewed head: `533fd3a44d6d14a26702ad3796290ee2fafebefd`.
The complete delta changes only exclusion comments and the handoff.
The reviewer parsed both TOML versions and verified that all exclusion patterns remain identical.
Production and test source remain unchanged.

F23 is CLOSED. The affected OS adapter comments now name concrete proving test functions.
The reviewer checked the named payload and Driver test definitions against the recorded proof.

### F24 LOW — The comment correction removes existing equivalence arguments

Status: OPEN.
Location: the existing wait_unreaped OR-to-XOR and set_nonblocking OR-to-XOR exclusion comments in .cargo/mutants.toml.
The correction replaces the wait_unreaped argument about disjoint EXITED and NOWAIT bits with test names.
The remaining comment requests a dependency recheck but no longer states why OR and XOR are equivalent.
The correction also removes set_nonblocking's statement that its sole caller supplies a fresh blocking PTY from pinned pty-process 0.5.3.
The remaining description of Pty::open no longer states that caller premise explicitly.
Concrete test names supplement an equivalence argument; they do not replace its premises.

Required change: Restore the disjoint-option-bits argument for wait_unreaped beside the new test names.
Restore set_nonblocking's sole-caller, fresh-blocking-PTY premise and pinned dependency beside its new test name.
Keep the patterns and all new concrete proof names unchanged.
This correction requires no new execution proof. The accepted source equivalence arguments remain preserved in earlier rounds.

F1 through F23 are CLOSED. Every earlier finding and closure remains preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (F24 LOW open) on `533fd3a44d6d14a26702ad3796290ee2fafebefd`.


## Round 71 — Restored equivalence reasons and exact-head CLEAN

Reviewed head: `9eb9a59fcdd798d86cf8a7b96fc2c108e19282dc`.
The complete delta from 533fd3a adds two exclusion-comment lines and the handoff record.
The reviewer parsed both TOML versions and verified identical configuration values.
Production, test source, and exclusion patterns remain unchanged. No merge conflict resolution accompanies the delta.

F24 is CLOSED. This also resolves the corresponding integration I5 source issue.
The wait_unreaped entry again states that EXITED and NOWAIT are disjoint bits, making OR and XOR equal.
The set_nonblocking entry again states that its sole caller supplies a fresh blocking PTY from pinned pty-process 0.5.3.
The comments retain the concrete test names, NONBLOCK-clear premise, OR/XOR argument, and caller/dependency recheck conditions.
F23's concrete-name closure remains intact. No new finding arises from the delta.

All findings F1 through F24, including LOW findings, are CLOSED at this exact head.
All original mutation dispositions and later Linux/native helper proof remain preserved.
The reviewed production and test source remain identical to the completed proof recorded through round 67.
This CLEAN covers P3's M1 package delta and all named resolutions recorded in this history.
The cross-package PR requires the integration reviewer's exact-head CLEAN and the implementer's landing gate under the restack plan.
M2a and M2b review remain separate milestones. The explicit real-harness pending list remains preserved.
The reviewer ran no tests, builds, or gates.

VERDICT: CLEAN (P3 M1 package scope) on `9eb9a59fcdd798d86cf8a7b96fc2c108e19282dc`.


## Round 72 — Working-harness failure-propagation test correction

Reviewed head: `b77038acb506f804262f3d9c99923aeceefb2683`.
The complete delta from 9eb9a59 changes one P6 test input/comment and the handoff.
Production, mutation patterns, and contract pins remain unchanged. No merge conflict resolution accompanies this delta.

The reviewer read the actual failed landing log at 9eb9a59:
`~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-9eb9a59f-linux-20261004-182418-50922.log`.
Default tests run 750 entries: 749 pass and one fails, with 654 entries outside the tier.
The failed assertion expects run_deterministic with open:{} to return an error.
The working TestkitHarness accepts that valid default-worker open, so this input no longer exercises failure propagation.
The failed gate supplies no later slow, mutation, or fuzz evidence.

The correction supplies open:{worker:null} and cites Core LC-1 MissingWorkerPath.
The reviewer checked the conformance driver's open mapping and the TestkitHarness OpenConfig mapping.
The driver maps explicit null to no worker. The harness passes no worker_path to Core.
LC-1 requires MissingWorkerPath. The script does not expect that error, so the runner returns failure through run_deterministic.
The assertion therefore retains a real failure-propagation check through the working harness.
It does not replace the harness, invent a refusal, change production behavior, or weaken the asserted error result.

The reviewer read the focused exact-head regression log:
`~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-b77038ac-linux-20261004-182718-52204.log`.
The corrected test passes in 0.004 seconds. One test runs, with 195 outside the filter. The job exits zero after ten seconds.
This verifies the correction and supplies no full landing-gate claim.
No new finding remains in this package delta.

All findings F1 through F24 remain CLOSED. All earlier findings, closures, and mutation evidence remain preserved.
This CLEAN covers P3's M1 package scope. The cross-package test correction also requires the integration reviewer's exact-head CLEAN.
The implementer must then run the restack plan's landing gate at the head cleared by both reviewers.
The reviewer ran no tests, builds, or gates.

VERDICT: CLEAN (P3 M1 package scope) on `b77038acb506f804262f3d9c99923aeceefb2683`.


## Round 73 — Payload diagnostic formatter exclusion

Reviewed head: `da2b0494bbda711e5a67cb180ddf05c607784635`.
The complete delta from b77038ac adds one exclusion with its reason and the handoff facts.
Production and test source remain unchanged. No merge conflict resolution accompanies this delta.

The reviewer read the actual landing log at b77038ac:
`~/botster-sessions/gates/botster-core-stage1-p3-m1-v1-b77038ac-linux-20261004-183142-56625.log`.
Format, clippy, taint, lists, public-api, candidate prebuild, default tests, and slow tests pass.
Mutation tests run 329 entries: 261 caught, 67 unviable, one missed, and zero timeouts.
The sole miss replaces Payload's Debug::fmt at payload.rs:72:9 with an empty successful formatting result.
The default mutation tier does not execute the real-payload formatting test. Fuzz does not run after that failed step.
This log supplies no full landing-gate success claim.

The new exclusion is ACCEPTED as one printing-glue function.
The reviewer checked the formatter body: it prints Payload, the stored pid, child.is_none() as reaped, and a non-exhaustive marker.
It does not call Payload::pid, change runtime state, or control a runtime decision.
The reason names the_payload_runs_on_the_pty_with_the_exact_environment, which checks the label and actual pid.
The reviewer read the earlier actual mutation failure log in /private/tmp/p3-payload-corrected-linux-evidence.
The same formatter replacement fails description.contains("Payload") in 0.003 seconds, with Failure(100).
That completed library-inclusive slow mutation proof remains applicable because formatter and test source are unchanged.

The reviewer parsed the TOML and checked the new pattern against the formatter mutation and unrelated function names.
The entry covers only <impl std::fmt::Debug for Payload>::fmt and preserves all earlier exclusion scopes and pure-decision coverage.
The corrected reason at this exact head accurately records the direct stored-field access.
No new finding remains in this delta. All findings F1 through F24 remain CLOSED.
Every earlier finding, closure, and mutation disposition remains preserved.
This CLEAN covers P3's M1 package scope. The cross-package PR still requires integration exact-head CLEAN and the implementer's new landing gate.
The reviewer ran no tests, builds, or gates.

VERDICT: CLEAN (P3 M1 package scope) on `da2b0494bbda711e5a67cb180ddf05c607784635`.


## Round 74 — PR #163: fixes for the merged-code audit

Reviewed head: `0403470b8a08bdcbb6d89aab2a98ca3242c8e37b`.
Branch: `stage1/p3-audit-fixes`. PR: https://github.com/trybotster/botster-core/pull/163.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
The reviewer read all 24 changed files and the PR's Prior art note.
The reviewer read the relevant findings in audit `02acc4b:audits/v1-pr134-141.md`.
Seven commits follow the base. No merge or conflict resolution appears in this delta.
This review precedes M2a under the lead's audit-fix order.

### Accepted source changes and scope

A3: The host removes the fixed 64-byte estimate.
The binding asks libghostty's key encoder for lengths across the six key options and 32 kitty flag combinations.
The mouse bound uses the native encoder across the tracking modes and formats.
The focus bound uses the native focus encoder.
The host checks arguments before division by the validated repeat count.
The host computes the reservation once and stores it for lane accounting and an unacknowledged write's `Unknown` bound.
The tests derive sizes from the binding and exercise admission and link failure through the host machine.
The separate binding tests compare length probes with completed encoding calls through explicit modes.
The new code writes no terminal bytes and copies no terminal encoding table.
Bytes, Text, and Paste retain their payload-byte accounting. No contract value changes.

A8: Both session launch paths now use the existing GroupGuard prefix and a separate worker process group.
The guard's anchor retains group membership. The guard does not reap the production payload.
The separate PayloadGuard retains its existing ownership and cleanup path.
The new launch path addresses the unowned prebuilt-worker source defect.
F26 below keeps the complete cleanup-deadline claim open.

A30: The testkit measures unread program output when DrainPty arrives.
It limits reads to that count and emits PtyDrained only for a requested drain.
The changed test exercises output before, during, and after that drain.
The scripted program has no kernel flip buffer, so its unread count includes its available output directly.

A31: The real driver reads the initial count, makes one flushing read, and measures the remaining count once after that read.
The new Drain decision limits subsequent reads to that second count.
Interrupted reads retain the drain for the next turn. Empty, blocked, and ended reads finish it.
The driver retains its bounded turns and control-first order.
This matches the lead's recorded A31 ruling in the state log.
The scripted-reader tests check omitted output and a writer that continues after the drain bound.
No race campaign or new tuning constant appears.
F28 below concerns the changed native-count proof, not this accepted drain algorithm.

A52: The injected wait decision now panics with the errno instead of producing Code(-1).
The lead explicitly accepted this invariant-failure policy and the documented start_time 0 sentinel.
The live-process identity test checks that the sentinel does not match the current process.
The link type remains unchanged. F25 below concerns the false claim about the panic's effect on the worker.

A53: Payload::reap now consumes self through drop(self).
Its no-op mutation remains equivalent because an empty consuming body also drops self at its end.
The updated exclusion states that exact reason.
The worker removes the unreachable hello-time exit report and exit_reported.
The first requested drain after an exit reports that exit once through on_drained.
The obsolete report_exit and Focus-arm exclusions are removed.
No new mutation exclusion appears in this PR.

The lead deferred A36 to M2b and A32/A33 to `stage1/p3-driver-mutation`.
This review does not close those audit findings or grant a new blanket driver exclusion.
The earlier per-function mutation reasons require fresh review when their bodies, callers, or proving tests change.
M2b, the snapshot paging documentation, A13, and both-harness conformance proof remain separate work.
The conformance pending list does not change.

### F25 — LOW — An exit-watch panic does not end the worker as documented

Status: OPEN.
Evidence: `crates/botster-core-sys/src/payload.rs:173-179,276-280`;
`crates/botster-worker/src/main.rs:85-89,201-204,281-285`.

The new comment says that a failed wait ends the worker and makes the host observe a lost worker.
Production runs wait_unreaped in a detached payload-exit thread.
The workspace does not select panic=abort. A panic unwinds that thread and skips on_exit.
The callback therefore sends no status and wakes no driver.
The driver retains its original channel sender, so the thread's exit also cannot disconnect that channel.
The worker's main thread continues. The comment describes an effect that this path does not provide.
The unit test catches a panic from a direct helper call. It supplies no evidence of worker termination.

Required change: Make the documented failure effect accurate under the lead's accepted panic policy.
Remove the worker-termination claim if thread failure alone is the intended effect.
If worker termination remains the intended effect, propagate the failure to the driver and prove that effect through the real path.
Do not invent an exit status or change the public link protocol to correct this comment.

Authority: the lead's A52 ruling and the requirement for accurate failure reporting and comments.

### F26 — HIGH — A11 still leaves test cleanup outside a deadline

Status: OPEN. This keeps audit A11's complete cleanup claim open.
Evidence: `crates/botster-worker/tests/common/session.rs:124-142`;
`crates/botster-core-sys/tests/slow_payload.rs:51-56,370-388,416-430`;
`crates/botster-core-sys/tests/common/process_guard.rs:90-104`;
`crates/botster-core-sys/tests/common/payload_guard.rs:82-96`.

The session cleanup waits with a deadline for TERM, sends KILL on timeout, then calls blocking waitpid without a deadline.
A process that still cannot become waitable therefore holds the test in the same final-wait failure that A11 identifies.
Sending KILL does not establish that the final wait has completed.
The slow payload Observer::wait and Observer::drop also still call Child::wait without a deadline.
The reap-observer test reaches those calls directly.

The new finish_within deadline starts only after GuardedPayload::drop drops its PayloadGuard.
That guard joins its registration thread without a deadline.
The session fixture also drops both guards before its bounded worker observation.
GroupGuard::drop joins its registration thread and waits for its anchor without a deadline.
A stuck registration or anchor therefore bypasses the newly added timeout helpers.
The PR's statement that every fixture wait has a deadline is not yet true.

Required change: Bound the complete test cleanup path, including guard completion and the final worker or observer wait.
Preserve group ownership until the last signal. Preserve production ownership of the payload reap.
Keep production Payload::drop unchanged, as the audit and lead require.
Prove a bounded failure when a child or cleanup step does not complete.
Use event waits and marked deadlines. Do not add polling or a global lock.

Authority: BUILD.md testing rules 5 and 10, audit A11, and the user's real-process ownership requirement.

### F27 — MEDIUM — Cleanup timeout can become a successful expected-panic test

Status: OPEN.
Evidence: `crates/botster-core-sys/tests/slow_payload.rs:63-76,345-365`.

finish_within logs a timeout but returns normally when the caller is already unwinding.
The expected-panic test catches that original panic and sends only outcome.is_err().
That boolean is true whether production cleanup completed or its new helper timed out.
The outer and inner waits both use ten seconds. Their start order depends on thread scheduling.
If the cleanup wait starts first, its timeout can return before the outer deadline expires.
The test then accepts the expected panic while the detached thread still waits in production cleanup.
The outer deadline therefore does not reliably close this false-success path.

Required change: Report cleanup completion separately from the expected panic.
Require the test to observe successful cleanup before it passes.
Preserve a failure result during unwinding without causing an uncontrolled second panic.
Do not rely on the relative start times of two equal deadline timers.
Use the existing cleanup regression to prove this real failure behavior.

Authority: BUILD.md testing rules 3, 9, and 10; audit A11's bounded-failure requirement.

### F28 — MEDIUM — The native PTY count test no longer proves a changing count

Status: OPEN.
Evidence: `crates/botster-core-sys/tests/slow_payload.rs:313-340`;
rounds 65-67 of this verdict and their native pending_output proof.

Waiting for master readability addresses the fixture's known Linux race.
The replacement assertion accepts every count from one to the number of queued bytes.
The next assertion compares two calls to pending_output without changing the queue.
The test then reads the bytes but never checks the count after a read.
A helper that always returns Ok(1) satisfies both count assertions and does not affect the later writes or reads.
The test therefore proves neither the actual readable count nor its change when the queue is consumed.
This is a source comparison, not a claim that the reviewer ran a mutant.

The earlier native evidence rejected Ok(1) through pending > 1 in this same test.
That assertion is removed, so the earlier failure log no longer proves the current test's coverage.
The function exclusion still names this test and the retained native evidence.

Required change: Check the count against actual byte consumption in the existing real-PTY test.
Include the quiet queue after consumption, before the program writes more output.
Derive expected values from the program bytes and reads. Keep the master-readiness wait.
Refresh the applicable native proof when execution is permitted.
Do not add a separate test whose only purpose is to reject a constant-return mutant.

Authority: BUILD.md testing rule 3, the user's behavior-test requirement, and the native mutation-evidence ruling.

### Completed evidence and verdict

The reviewer read the completed log at `1060fca6560337625f7cf309d3ba27c32adf419b`:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-1060fca6-linux-20261004-203837-38920.log`.
The selected format, default clippy, taint, prebuild, and default test steps pass.
The default report counts 758 tests. Its recorded wall time is 2.6 seconds.
The selected worker tests count 52 passes, including helper entries, with zero skips.
slow_payload then fails compilation at the temporary borrowed descriptor.
The job ends with exit 101. It is not a passing landing gate.

The reviewer also read the completed `fb36ff89` log.
Its slow-feature clippy check fails on the duplicate module, redundant conversion, and explicit drop of the PollFd array.
Commit 0403470 fixes those three source issues. The reviewer inspected that complete delta.
No successful corrected-head slow_payload, slow-feature clippy, or mutation result is supplied.
The full HOLD prevents new heavy jobs. The reviewer requests no job during that HOLD.
Completed results from an earlier head do not clear the four source findings above.

All earlier findings, closures, and mutation dispositions remain preserved.
F1 through F24 remain CLOSED at their recorded heads and scopes.
F25 through F28 are OPEN on this submitted head.
The reviewer ran no tests, builds, or gates.

VERDICT: NOT CLEAN (4 open findings) on `0403470b8a08bdcbb6d89aab2a98ca3242c8e37b`.


## Round 75 — Integration findings on the same audit-fix head

Reviewed head: `0403470b8a08bdcbb6d89aab2a98ca3242c8e37b`, branch `stage1/p3-audit-fixes`.
There is no new implementation delta.
The reviewer read the integration verdict at `872a3e650d5a0c410949ddfa5f3232a3af486314`:
`verdicts/stage1/p3-audit-fixes.md` on `stage1/integration-review`.
The reviewer checked I1 through I5 against the submitted source.
F29 through F32 below record I2 through I5 as package findings.

The integration reviewer withdrew I6 after the package reviewer challenged it.
Audit A53 explicitly permits `Payload::reap` as `drop(self)`.
That body is correct. Its empty-body mutant also drops the consumed value and is exactly equivalent.
The exclusion states that reason. No change to this wrapper is required.
Round 74's A53 acceptance remains in effect.

Round 74's A3 acceptance covers ownership of the computation and correctness of the enumerated bound.
It does not clear the synchronous cost in F29.
F30 supersedes round 74's acceptance of the testkit drain's parity with the real driver.
The lack of a kernel flip buffer does not remove the observable difference for output written after the initial count.
The real driver's accepted A31 algorithm remains in effect.

### F25 — MEDIUM — A failed exit watch leaves the worker without an exit result

Status: OPEN. This supersedes round 74's LOW severity and comment-only correction option.
The source evidence remains the same. Integration finding I1 identifies the same failure.

A panic in the detached watcher skips the exit callback.
The driver retains its original sender, so watcher failure cannot disconnect the receiver.
The driver receives no exit status and no wake. A Stop that depends on the exit can remain pending.
Changing the comment alone would document this failure without correcting it.
The lead's accepted invariant panic must produce an end that the host can observe.

Required change: Preserve the lead's panic policy and propagate watcher failure to the driver.
The driver must end the payload group and end the worker with a failure that the host can observe.
Use the production cleanup path. Do not abort the process and skip group cleanup.
Do not invent a payload exit status or change the public link protocol.
Correct the comment after the behavior is correct.
Prove the driver behavior through its injected edges, including cleanup and the observable failure.
A direct helper test that only catches the panic does not prove this behavior.

Authority: the lead's A52 ruling, EV-4, AM-3, and the requirement for visible failure and exactly one completion.

### F29 — MEDIUM — Key admission repeats large text work on the host thread

Status: OPEN. This records integration finding I2.
Evidence: `crates/botster-terminal-ghostty/src/encode.rs:679-705,742-770`;
`crates/botster-core-host/src/admit.rs:371-399,445-456`.
Native evidence: Ghostty pin `3f8eb6810bb673aa782b047de21783ac81fb1121`,
`src/terminal/c/key_encode.zig:136-168` and the text loop in `src/input/key_encode.zig`, `KittySequence::encode`.
The reviewer read that exact native pin through the existing local Ghostty checkout.

A key that fits in its worst form makes every one of the 2048 state probes.
Each probe receives the complete event, including its text.
The native size probe runs the encoder with a discarding writer after the fixed writer reports insufficient space.
The associated-text form visits the text's codepoints and formats each printable codepoint.
Many state combinations repeat that same text work.
The host runs these probes synchronously in begin, before it checks pending operation capacity.
Large admissible text therefore causes repeated full-text work on the host thread.
The existing early stop does not help an admissible key.
The reviewer made no runtime measurement and does not claim a measured delay.

Required change: Remove the repeated large-text work while preserving the exact libghostty-derived maximum.
Do not add a hand-written encoder, terminal-byte table, or assumed text expansion factor.
Ordering states for early refusal alone does not correct the admissible-key case.
When execution is permitted, record the cost for worst admissible keys at the default 1 MiB and maximum 64 MiB limits.
If an exact bound needs a new policy or tuning constant, ask the lead a QUESTION before selecting it.
The current HOLD still forbids new heavy jobs. This finding requests no job during the HOLD.

Authority: BUILD.md's fast and lean requirement, plan 9B, and the requirement that libghostty owns terminal encoding semantics.

### F30 — MEDIUM — The testkit omits the real driver's flushing read

Status: OPEN. This records integration finding I3 and keeps audit A30 open.
Evidence: `crates/botster-core-testkit/src/worker.rs:380-394,425-442,468-470`;
`crates/botster-worker/src/io_decisions.rs:31-89`.

The testkit records only the initial unread count when DrainPty arrives.
It emits PtyDrained after that count is consumed, even if more output has since become available.
The real driver then performs one flushing read and reads the count measured after that read.
Output that arrives after the initial count can therefore reach the real worker before Exited.
The same output can remain unread when the testkit emits PtyDrained.
This is an observable edge difference. The scripted program's accurate initial count does not remove it.
The testkit comment says that it follows the real driver, but its decision differs.

Required change: Put the drain decision where both drivers can use it and use that decision in both paths.
Keep counts and reads in their injected edges.
Prove the behavior for output before the initial count, after that count, and after the final bound.
Use the same decision through the real and scripted edges. Do not copy its branches into a test helper.
A different testkit contract requires an explicit lead ruling with the reason that the difference cannot be observed.

Authority: audit A30, EV-4, ST-5, BUILD.md's edge-parity requirement, and the user's one-path requirement.

### F31 — LOW — The drain decision delegates its empty-read rule to copied branches

Status: OPEN. This records integration finding I4.
Evidence: `crates/botster-worker/src/io_decisions.rs`, `Drain::after_read`;
`crates/botster-worker/src/main.rs`, `read_pty_chunk`; the test helper `drain_with`.

The after_read documentation says that an empty read completes the drain.
For Counted(3), after_read(0) instead returns Counted(3).
For Flushed(3), it instead returns Flushed(3).
The production caller handles zero with a separate Done branch.
The test helper copies that branch, so its test bypasses the decision for the documented empty-read behavior.
The current callers complete the drain, but the shared decision does not own this rule.

Required change: Make after_read return Done for an empty read.
Pass every successful read result through that production decision.
Remove the copied zero-read branches from the production caller and test helper.
Keep the behavior proof in the existing drain test.

Authority: accurate documentation and the user's one-path and behavior-test requirements.

### F32 — LOW — A test still claims the repeated-key check that the PR removed

Status: OPEN. This records integration finding I5.
Evidence: `crates/botster-core-host/src/tests/queue_pressure.rs:758` onward,
`retirement_keeps_the_result_path_and_bounds_a_repeated_key`.

The test name and documentation still claim that the test checks a repeated key's Unknown bound.
The PR removes that assertion. The test now checks only the retirement result path.
The max_key_repeat = 100 setup serves the removed assertion.
The lifecycle test a_write_in_flight_when_the_link_fails_is_unknown now checks a repeated key's Unknown bound.

Required change: Rename the retirement test and correct its documentation to describe its remaining behavior.
Remove the unused repeat-limit setup.
Name the lifecycle test as the repeated-key proof where the documentation describes that coverage.

Authority: accurate test claims and the user's behavior-test requirement.

### Verdict

F1 through F24 remain CLOSED at their recorded heads and scopes.
F25 remains OPEN at MEDIUM severity with the behavior correction above.
F26 through F28 remain OPEN with all round 74 requirements preserved.
F29 through F32 are OPEN. LOW findings also require closure before CLEAN.
The integration verdict remains NOT CLEAN with five open findings on this same head.
The earlier completed evidence and all round 74 gate limits remain unchanged.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (8 open findings) on `0403470b8a08bdcbb6d89aab2a98ca3242c8e37b`.


## Round 76 — Audit-fix correction delta

Reviewed head: `82b4269bcb0d1412b94f38c818df5169617570c6`, branch `stage1/p3-audit-fixes`.
Reviewed delta: `0403470..82b4269`, commits `6770c0c0649f5a744b5f192cf031b45642e7661f`
and `82b4269bcb0d1412b94f38c818df5169617570c6`, 14 files.
The reviewer read every changed file. This delta contains no merge or conflict resolution.
The integration reviewer records I1 through I5 closed and I6 withdrawn in verdict commit `149a3ac`.
Its CLEAN still depends on this package's CLEAN on the same head.

### F25 — MEDIUM — OPEN: invariant context and actual driver proof remain missing

The new watch returns io::Result<ExitStatus> instead of panicking.
The callback sends that result and wakes the driver.
Driver::run applies the result with `?`, so a wait error returns from run.
Run consumes the driver. Returning drops its Payload, which ends the group through the existing production Drop.
The command-line path converts the error to a failure exit and prints its text.
This corrects the detached-thread hang by source inspection. No payload exit status is invented.

The reviewer asked the lead whether this error path can replace the explicit A52 panic ruling.
The lead authorized it in message `msg_plugin-w_1791173754_d79a4c` and superseded the panic wording.
The lead specified three conditions:

1. The error carries the errno and names the invariant, "the Payload holds the unreaped leader".
2. No fallback invents an ExitStatus.
3. The behavior proof shows the error reaches main, the payload group ends, and the worker exit is non-zero.

The current Err(errno.into()) carries the errno but does not name the invariant.
The new exit_input test checks only Result::map and its input values.
It never drives the exit channel, Driver::run, Payload::drop, link closure, or the worker's failure exit.
It therefore does not prove the required behavior through the production path.

Required change: Add the invariant context while preserving the errno.
Prove the complete failure path with the production driver and its injected exit result.
Check group cleanup and the worker's non-zero exit. Check the failure that reaches main.
Replace the map-only proof with that behavior proof. Do not add a production test branch.
The lead explicitly requires NOT CLEAN until this proof exists.

Evidence: `crates/botster-core-sys/src/payload.rs`, `wait_unreaped_with`;
`crates/botster-worker/src/main.rs`, the EXIT arm in Driver::run;
`crates/botster-worker/src/io_decisions.rs:109-120`;
`crates/botster-worker/src/command_line.rs`, execute.

### F26 — HIGH — OPEN: remaining fixture waits and the bounded-failure proof

The changed GroupGuard registration join and anchor wait now have marked deadlines.
PayloadGuard's registration join also has a marked deadline.
GuardedPayload now puts guard cleanup inside its outer deadline.
The worker cleanup observes the exit with WNOWAIT after the KILL fallback before it reaps.
Observer::wait and Observer::drop also observe the exit with a deadline before their final reap.
With exclusive reaping, an observed terminal exit makes those final reaps ready.
These changes correct the specific waits in round 74's initial source evidence.
Production Payload::drop remains unchanged, as required.

The complete fixture claim is still not proved.
`process_guard::Parent::drop` sends KILL then calls Child::wait without a deadline.
The parent_dies_before_fifo_reader regression reaches that Drop directly.
The same guard module's regressions still use direct child waits without deadlines.
The PR also adds no regression that proves bounded failure when cleanup cannot complete.
The existing expected-panic regression proves successful cleanup during an unwind.
It does not exercise a cleanup operation that fails to complete.

Required change: Bound the remaining fixture waits and preserve ownership through the final signal.
Prove that the complete cleanup path reports failure within its deadline when an owned cleanup operation does not complete.
Keep event waits and marked deadlines. Keep production ownership of the payload reap.
Do not add polling, a global lock, or a test-side payload reaper.
All round 74 ownership and deadline requirements remain in effect.

Evidence: `crates/botster-core-sys/tests/common/process_guard.rs:167-175` and its real-process regressions.

### F27 — MEDIUM — CLOSED at 82b4269

GuardedPayload::drop now reports whether its complete cleanup finished.
The expected-panic test catches the unwind directly and separately requires cleanup_report to return true.
A cleanup timeout returns false during unwinding, so the original panic cannot make that test pass.
A panic in the cleanup thread also prevents its completion message and produces failure.
The test no longer depends on the relative start times of equal outer and inner deadline timers.
F26 separately retains the missing proof of bounded cleanup failure.

### F28 — MEDIUM — OPEN: source corrected, native evidence pending

The real-PTY test keeps the master-readiness wait.
It measures the queue, reads until WouldBlock, and compares that count with the number of bytes consumed.
It checks that the consumed bytes equal the program's output.
It then checks an empty queue before the program receives input and writes again.
These assertions derive their values from the program bytes and actual reads.
A constant Ok(1) no longer satisfies the count and empty-queue checks.
The subsequent input read also excludes the output already consumed.
This corrects round 74's source defect.

The implementer reports that neither submitted correction head has compiled under the full HOLD.
The required native proof is not supplied. The earlier proof does not establish this changed test's behavior.
F28 stays OPEN only for the applicable completed proof, including the native Mac path.
The reviewer requests no execution during the HOLD.
After all other findings close, the lead's gate-evidence closure rule permits the required verification on the reviewed head.
A passing result then permits closure and CLEAN on that same head.
A source change after failure requires a new delta review before the next execution.

### F29 — MEDIUM — CLOSED at 82b4269 under the lead's option (b) ruling

The state-log ruling dated 2026-10-04 selects early-stop state ordering in PR #163 now.
It keeps exact enumeration while the steward seeks a bound for KeyInput.text.
It assigns measurements and the authorized bound to a later small PR after the HOLD and a final ruling.
The later A15 entry states that its proposed numbers are rejected pending user authority.
No pair may select or implement those numbers now.

The new enumeration makes kitty flags the low five index bits and inverts flag 16.
Its first 16 states enable associated-text reporting.
The six Boolean modes and all 32 kitty flag sets still occur exactly once across the 2048 states.
The native probes still compute the exact bound and retain the early refusal.
The documentation states the remaining admitted-key cost and marks the text bound pending steward.
This meets the authorized PR scope. It does not prove that the admitted-key cost has been removed.
The later bound and measurements remain explicitly pending under the ruling.

### F30 — MEDIUM — CLOSED at 82b4269

Drain now lives in botster-worker-core and both drivers use it.
The scripted edge supplies its unread count and advances the same decision after each read.
It reads the initial count, performs the flushing read, and respects the count measured after that read.
The changed edge test writes output after the initial count and after the final measurement.
It checks that the first output is drained and the later output remains queued.
The expected counts come from the program's writes.
The shared decision tests retain the omitted-output and continuing-writer cases.
No hand-written terminal semantics or expected terminal bytes are added.

### F31 — LOW — CLOSED at 82b4269

Drain::after_read now completes the drain for a zero-byte read.
The real driver and the decision test helper pass all successful read results through that method.
Their separate zero-byte branches are removed.
The documentation and production decision now agree.

### F32 — LOW — CLOSED at 82b4269

The retirement test has a new name that describes its remaining result-path check.
Its documentation no longer claims the removed repeated-key assertion.
It names the lifecycle test that checks the key's Unknown bound.
The unused max_key_repeat setup is removed.

### Verdict

F1 through F24 remain CLOSED at their recorded heads and scopes.
F27 and F29 through F32 are CLOSED at this head with the scope above.
F25 and F26 still require source or behavior-proof changes.
F28 has corrected source and requires completed native evidence.
All earlier findings, closures, and verdict rounds remain preserved.
The conformance pending list remains unchanged. M2a and M2b remain separate work.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (3 open findings) on `82b4269bcb0d1412b94f38c818df5169617570c6`.


## Round 77 — Watch failure proof and cleanup deadlines

Reviewed head: `800606ac2a99c42a7e25886a5aa1ccc2b88070d1`, branch `stage1/p3-audit-fixes`.
Reviewed delta: `82b4269..800606a`, one commit, eight files.
The reviewer read the complete delta. It contains no merge or conflict resolution.
The implementer reports no compilation or execution at this head under the full HOLD.
The integration reviewer reports zero integration findings in verdict commit `ca088f6`.
Its CLEAN remains conditional on this package's CLEAN on the same head.

### F25 — MEDIUM — OPEN: replace the new sleep with an event wait

ExitWatchFailed retains the errno as a typed value and as its error source.
Its display text names the invariant, "the Payload holds the unreaped leader".
The injected wait test checks the typed error and the invariant text.
The watcher returns this error through the existing exit channel and waker.
The driver now applies `watched?` inline. The map-only helper and its test are removed.
No fallback invents an ExitStatus.

The new a_failed_exit_watch_ends_the_worker_with_a_failure drives the real Driver::run.
It passes that run through command_line::execute, which main calls.
It checks the failure exit code and the error text, including the errno and invariant.
It observes FIFO EOF while the test guard remains alive.
That observation checks the production payload cleanup and the background group member's end.
It also reads the host socket to EOF after the driver returns.
The test keeps the existing marked deadlines and independent group ownership.
This is the actual-path proof that round 76 requires, by source inspection.
The error type and production path meet the lead's refined A52 ruling.

The new script uses `sleep 30` for the background member.
BUILD.md testing rule 5 forbids sleeps and requires tests to wait on real events.
The fixture needs a member that keeps the FIFO writer alive, but it does not need a timed sleep.

Required change: Use an event-blocking background member that retains the FIFO writer.
Keep the readiness event, the production cleanup observation, the independent guard, and the existing deadline waits.
No other source correction to this failure path is required by this delta review.

Evidence: `crates/botster-worker/tests/common/driver_edges.rs:450-456`.
Authority: BUILD.md testing rule 5 and the user's real-process fixture requirements.

### F26 — HIGH — CLOSED at 800606a

The guard's bounded helper now returns the completed wait result.
Parent::drop retains its Child through KILL and moves the final wait into that bounded helper.
The direct waits in blocked_parent, a_panic_before_ready_ends_the_child,
and an_early_exit_keeps_the_group_owned_until_cleanup also use the helper.
The helper uses the existing ten-second marked cleanup deadline.
No test-side reap of a production payload is added.

The new a_cleanup_that_does_not_finish_fails_the_test drives finish_within with a blocked channel wait.
The test sets the deadline to zero and catches the timeout failure.
It requires that the timeout fails the test, then releases the blocked wait.
This proves the bounded failure without a sleep, polling, or an unreleased test operation.
The existing unwind regression still requires the separate successful-cleanup report.
Together with round 76's corrected guard, worker, and observer waits, this closes F26's requirements.
Production Payload::drop remains unchanged.

### F28 — MEDIUM — OPEN: native evidence remains pending

This delta does not change the PTY count test.
Round 76's corrected source remains accepted.
The required completed proof, including the native Mac path, remains pending.
The HOLD still prevents execution. The reviewer requests no job during the HOLD.
After F25 closes, F28 alone does not block the permitted verification under the lead's gate-evidence closure rule.
CLEAN requires its successful result on the same reviewed head.

### Verdict

F1 through F24 remain CLOSED at their recorded heads and scopes.
F26 is CLOSED at this head. F27 and F29 through F32 retain their round 76 closures.
F25 requires the fixture correction above. F28 requires completed native evidence.
All earlier findings, closures, and verdict rounds remain preserved.
The conformance pending list remains unchanged. M2a and M2b remain separate work.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (2 open findings) on `800606ac2a99c42a7e25886a5aa1ccc2b88070d1`.


## Round 78 — Event wait in the failure fixture

Reviewed head: `ccba0504b8f0e18d274132e0b88b80b2e364becd`, branch `stage1/p3-audit-fixes`.
Reviewed delta: `800606a..ccba050`, one commit, one test file.
The reviewer read the complete delta. It contains no production change, merge, or conflict resolution.
The integration verdict at `8505658` records zero integration findings on this head.
Its CLEAN remains conditional on this package's CLEAN on the same head.

### F25 — MEDIUM — CLOSED at ccba050

The failure fixture creates a second FIFO that no process opens for writing.
The external /bin/cat member blocks when it opens that FIFO.
It retains the inherited writer for the first FIFO until production ends the group.
The fixture uses no sleep or busy loop.
The readiness event and marked cleanup deadlines remain unchanged.

The accepted production error path and actual-driver proof from round 77 remain unchanged.
The proof checks the errno, invariant text, failure exit code, production group cleanup, and host link EOF.
The independent guard remains alive during the production cleanup observation.
This closes the remaining F25 requirement.

### F28 — MEDIUM — OPEN: completed native evidence only

The source correction from round 76 remains accepted.
The implementer supplies no completed correction-head proof under the full HOLD.
The required native evidence, including the Mac path, is still pending.
The reviewer requests no execution during the HOLD.

All source findings are now closed on this exact head.
Under the lead's gate-evidence closure rule, F28 alone does not block the required verification after execution is permitted.
A successful result can close F28 and permit CLEAN on this same head.
Any source change requires a new delta review before the next execution.
The reviewer will not give CLEAN while this evidence finding remains open.

The implementer also disclosed eight unchanged older payload scripts with timed hold bodies.
The implementer assigned their replacement to the existing A32/A33 driver-test follow-up.
This delta does not change those scripts or close that assigned audit work.
The separate M2a, M2b, mutation, and conformance duties remain unchanged.

### Verdict

F1 through F27 and F29 through F32 retain their closures, with F25 closed at this head.
F28 remains OPEN only for the completed native evidence above.
All earlier findings, closures, and verdict rounds remain preserved.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (1 open evidence finding) on `ccba0504b8f0e18d274132e0b88b80b2e364becd`.
