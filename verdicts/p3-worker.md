# P3 worker review

VERDICT: NOT CLEAN (1 open)

Reviewed head: `a855586248de777d553352039bcec915e307a0b0`, branch `stage1/p3-worker-m1`.
Previous reviewed head: `439e5e14c6fe55b3331d545c254dc11a8baf1ed8`.
Round 6 delta: `439e5e1..a855586`. F1 through F7 are CLOSED. New F8 is OPEN.
The original evidence refers to `f37c46b`. Rounds 2 through 5 record review history. Round 6 gives the current finding.
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

## Round 6 — F7 closed; new F8 open

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

Status: OPEN. New finding exposed by the rewritten tests at `a855586`.

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

VERDICT: NOT CLEAN (1 open) on the exact M1 head above.
