# P3 worker review

Current restack verdict: NOT CLEAN (F13, F18, and F19 open; F14, F15, F16, F17, and F20 closed in source).
Reviewed head: `7b54136568a88f1bbbf599de37002eb8daed363c`, branch `stage1/p3-m1-v1`.
Round 53 verifies corrected-source Mac failures. F13 retains 60 entries; F18/F19 require further correction.
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
