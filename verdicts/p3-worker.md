# P3 worker review

Current restack verdict: NOT CLEAN (F13 open after the mutation gate).
Reviewed head: `2e9811ca0b1c8a790698999efc24c538b5e755d7`, branch `stage1/p3-m1-v1`.
Round 24 reviews worker binding tests. F13 remains open for 118 entries.
The CLEAN below applies only to the old M2a head that it names.

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
