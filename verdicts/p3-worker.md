# P3 worker review

Current verdict: NOT CLEAN for PR #168 M2a at `7550018fff91ed948dbdfb7eea14caec48b27d6c`.
Round 101 closes F53 and completes the merge delta review. Only the required real-PTY proof HOLD remains open.
PR #167 Part A remains CLEAN at `41e997446090afb7ac45ae9823734dfeea7eb40b`. F49 through F52 are CLOSED.
PR #163 Part B retains F28, F33 execution, and F39 for its later merge delta.
PR #165 remains CLEAN at `29b37890efeffa4410d7dfc34f5c2344bfad1ba4` and landed through #162.
F45 through F48 are CLOSED. F39 is CLOSED within #165.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.
Each cross-package PR also requires the integration reviewer's exact-head CLEAN and the implementer's landing gate.
M1's round 73 CLEAN and M2a's earlier exact-head CLEAN remain preserved.
M2a at `a7f4a386593457e3b30f03b56938092de9b060a3` has no restack verdict.

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


## Round 79 — Completed Mac gate and missing driver collection

Reviewed head: `ccba0504b8f0e18d274132e0b88b80b2e364becd`, branch `stage1/p3-audit-fixes`.
There is no new source delta.
The reviewer read the completed log:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-ccba0504-mac-20261004-212806-3319.log`.
The log names this exact head, base `144b0234fb632bcbb5176b17c2fe55f3239405df`, and native Mac execution.
The job exits 1 after 773 seconds. This is not a passing landing gate.

### Completed evidence

Format, default clippy, taint, lists, public API, and worker prebuild pass.
The default tier runs 758 tests and all 758 pass. Its nextest report also names 654 skipped entries.
The default nextest run takes 3.181 seconds.
The slow tier runs 85 tests: 83 pass, two fail, and zero selected tests are skipped.
The slow tier selects seven binaries and skips seven binaries.
Mutation and fuzz do not run because the slow step fails.

All selected slow_payload tests pass natively.
The corrected PTY count test passes in 0.030 seconds.
The blocked-cleanup regression passes in 0.075 seconds.
The expected-panic cleanup regression passes in 0.030 seconds.
The selected slow_session and slow_cli tests also pass.

Two tests fail:

- slow_real_core::a_worker_is_not_left_when_the_cleanup_of_a_test_fails fails its cleanup assertion in 0.201 seconds.
- slow_process::process_guard::parent_dies_before_fifo_reader fails its FIFO EOF deadline in 10.048 seconds.

The second failure reports "all descendants closed the pipe: Timeout".
It is a bounded failure, so this result does not reopen F26's unbounded-cleanup finding.
The reviewer also read the completed P5 and P7 Mac logs at heads `1a96eee7` and `6e8d5b34`.
Both use base `144b023` and record the same slow_process failure in the earlier shared guard.
The lead assigns its root-cause correction to P5 PR #162 in the state log dated 2026-10-04.
The lead requires no rerun, no timeout change, a root-cause correction, and a deterministic proof.
Audit A10's slow_real_core correction is also P5 work.
These failures remain landing blockers; this review does not dismiss them as harmless or authorize a rerun.

### F28 — MEDIUM — OPEN: native baseline passes; mutation evidence pending

The native count test now has a completed passing baseline on this exact head.
The gate does not renew the native mutation proof from round 67.
The normal mutation configuration excludes pending_output and the normal mutation step uses the default tier.
In this completed job, that step does not run at all.
The updated real-PTY test still requires the applicable native Ok(0) and Ok(1) mutation evidence.
F28 remains OPEN until that completed evidence arrives and the reviewer checks the actual outcomes.
The implementer reports that the focused native job waits for capacity.
The reviewer requests no parallel jobs and runs no job.

### F33 — MEDIUM — The landing slow selection omits this PR's required driver proof

Status: OPEN.
Evidence: `xtask/src/test_budget.rs:169-189`;
`crates/botster-worker/src/main.rs`, slow_driver, driver_observer, and slow_edges test modules;
the completed Mac log's selected binaries and test names.

The slow selection enables each package's slow feature, then filters binaries with `binary(/^slow/)`.
The worker's feature-gated driver tests live in the botster-worker unit binary.
That binary name does not match the filter.
The default tier does not enable the slow feature, so it also omits those tests.
The new a_failed_exit_watch_ends_the_worker_with_a_failure therefore runs in neither landing tier.
The observer copy of the session tests and the other real-driver edge tests are also absent.
The completed gate log contains no execution of the required F25 proof.
The implementer corrected its initial claim that those tests passed in this gate.

Required change: Register the feature-gated worker driver tests in the landing slow selection.
Retain the existing slow integration tests and their independent process ownership.
Use the production driver path. Do not add a production test branch or a duplicate test implementation.
Show completed collection and execution of the new failed-watch test on the corrected reviewed head.
A focused run can supply interim evidence, but it does not correct the landing selection.
The broader A32/A33 test rewrite remains separate assigned work.
Do not defer collection of this PR's required proof to that later rewrite.

Authority: BUILD.md testing rules 2 and 3, the lead's A52 actual-path proof condition, and the exact-head landing requirements.

### Verdict

F1 through F27 and F29 through F32 retain their recorded closures.
F28 remains OPEN for native mutation evidence. F33 is OPEN for the landing collection correction above.
Round 78's statement that all source findings were closed is superseded by F33.
The gate is red, and its two failing shared tests remain assigned to the lead's stated owners.
All earlier findings, closures, and verdict rounds remain preserved.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (2 open findings) on `ccba0504b8f0e18d274132e0b88b80b2e364becd`.


## Round 80 — Slow selection at every module depth

Reviewed head: `40b63dceb6e3f3d7be69a1488ac77eccc121a071`, PR #163, branch `stage1/p3-audit-fixes`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `ccba050..40b63dc`, three commits, only `xtask/src/test_budget.rs`.
The reviewer read the complete delta and the completed native Mac collection log:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-40b63dce-mac-20261004-215438-84612.log`.

### F33 — MEDIUM — OPEN: source and collection corrected; execution pending

The intermediate `1a13fd1` filter selected only root slow modules in binary targets.
The intermediate `0110caa` filter removed the target restriction but still matched only the start of a test path.
It omitted nested library modules such as storage::slow_tests and real::slow_tests.
The reviewer reported this omission to both reviewers. The integration reviewer reopened I7 at verdict commit `fa34b7d`.

The current filter is `binary(/^slow/) | test(/(^|::)slow_/)`.
It matches a slow module at the start of the path or after a module separator.
The completed collection uses the slow packages, features, and this exact filter on the reviewed head.
It names the nested storage::slow_tests and real::slow_tests tests, the root slow_tests test, and the worker slow_driver tests.
It also names slow_edges::a_failed_exit_watch_ends_the_worker_with_a_failure.
The collection log reports 130 selected entries and exits 0 after five seconds.
This is actual collection evidence. The existing literal argv assertion supplies no collection proof.

The source requirement and collection requirement are satisfied.
F33 remains OPEN only for completed execution of the required failed-watch proof in the corrected landing selection.
The focused #165 log uses a different branch and does not contain this #163 test.
Its passing results do not close F33.
The implementer plans the #163 Mac gate after the guard PR and the required merge review.

### F28 — MEDIUM — OPEN: native mutation evidence remains pending

Round 79's native baseline remains recorded.
This filter change supplies no native Ok(0) or Ok(1) mutation outcome for pending_output.
F28 remains OPEN under its recorded requirements.

### Verdict

F28 and F33 are evidence findings on this source head.
They do not block permitted verification under the lead's gate-evidence closure rule.
Any source change requires a new delta review before execution.
The previous red gate and its assigned corrections remain recorded.
All earlier findings and closures remain preserved.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (2 open evidence findings) on `40b63dceb6e3f3d7be69a1488ac77eccc121a071`.


## Round 81 — Shared guard correction, PR #165

Reviewed head: `36702023b8a36876ad226ac61adb63723511f28e`, branch `stage1/p3-guard-macos`.
Base and merge base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
The lead's later ruling assigns the shared Mac guard correction to P3. It supersedes round 79's P5 assignment.
The lead requires the root-cause correction, a deterministic proof, and no timeout increase.
The reviewer read all eight changed files, the intermediate guard heads, and the PR description.
The review includes the `81ccd17..3670202` correction in process_guard.rs and payload_guard.rs.
This PR changes shared test infrastructure. It changes no production terminal or worker semantics.

### Accepted corrections and completed evidence

The anchor starts /usr/bin/true in the guarded group and keeps that child unreaped.
The anchor moves to its own group, then sends group signals while the reservation remains unreaped.
The anchor reaps only its reservation. It does not reap the production worker or payload.
Both guard types call the same end_group implementation.
The PR's Prior art section names libproc, kqueue, libc, and the rejected alternatives.

The current rounds use list, kill, then wait.
This order corrects the intermediate kill-before-list defect reported as integration G3.
The ForkRace decision model now ends members through the injected kill effect.
Its wait requires each listed member to have ended, so the old order fails this model.
This is a meaningful decision proof; the older helper call-count proof was insufficient.

The deadline now returns Failure::Left with the listed members.
A top-level listing error and a wait-setup error now return Failure::Error.
GroupGuard checks the anchor status and reports failure, including during an existing panic.
These changes correct part of integration G1 and G4. The remaining paths appear below.

The reviewer read the completed native Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-36702023-mac-20261004-215726-95373.log`.
The log names this exact head and base, slow clippy, worker prebuild, and two focused nextest commands.
It exits 0 after 11 seconds. The first command passes 132 tests; the second passes nine guard tests and skips nine other tests.
The previously failing slow_process::process_guard::parent_dies_before_fifo_reader passes in 0.044 seconds.
The same guard regression also passes in the selected worker and core test binaries.
This is a focused Mac proof, not a full landing gate or Linux evidence.

### F34 — HIGH — CLOSED at 81ccd17: signals no longer use an unowned PID

The reviewer reported this finding on `c5ba07e` before the formal verdict.
The initial implementation listed numeric member PIDs and then sent each PID a signal.
A member could exit, be reaped, and have its PID reused before the signal.
The anchor owned the group identity; it did not own each listed process identity.
The intermediate `e0514aa` corrected Linux with pidfds but retained a Mac check-before-kill window.
The reviewer did not accept that Mac window as ownership proof.

At `81ccd17`, every signal targets the group while the unreaped reservation keeps its identity.
The current head retains this correction. No member receives a signal by a bare PID.
This closes the unsafe signal source path.
F38 records the remaining test requirement for the new reservation behavior.

### F35 — MEDIUM — OPEN: member inspection and signal errors remain hidden

Evidence: `crates/botster-core-sys/tests/common/process_guard.rs:147-150,297-315,323-353` at the reviewed head.

The Mac listing propagates pids_by_type errors but discards every pidinfo error with .ok().
The Linux listing propagates directory errors but discards every process stat read error.
Both paths treat an inspection failure as proof that the member has ended.
A permission error or another inspection error can therefore remove a live member from the list.
If all inspections fail, end_members reports success without a group signal.
The comments claim that these failures mean the process ended; the code does not establish that condition.
The group signal also discards every kill_process_group error.

Required change: Omit a member only when the edge proves disappearance or another accepted non-live state.
Report other inspection and signal errors through the cleanup result.
Keep the reservation owned through every signal and the final cleanup decision.
Prove the inspection-error outcome through injected edges used by the actual cleanup path.
Do not add sleeps, active retry loops, or a separate test implementation.

Authority: group ownership, reported cleanup failures, and BUILD.md testing rules 3 and 5.

### F36 — MEDIUM — OPEN: the Mac wait can still cause an active retry loop

Evidence: `process_guard.rs:247-262` and `end_members:209-220` at the reviewed head.

The Mac wait discards the complete Watcher::poll result and returns Ok(()).
The local kqueue 1.2.1 source returns EventData::Error for a failed syscall and None for a timeout.
The guard ignores both outcomes and every event's identity and data.
A repeatable wait error can therefore return immediately while the member remains listed.
The next round repeats listing, signaling, and waiting without a blocking event until the deadline.

The watch ESRCH path also returns Ok(()) without proving that live_members will stop listing that member.
A member in exit can remain non-zombie in the listing while the watch cannot attach.
The integration reviewer records this remaining case as G5 at verdict commits `2722ce0` and `6b3bb0d`.

Required change: Inspect the wait outcome and preserve errors.
Distinguish an exit event, a deadline, and an observation failure.
Do not repeatedly treat an unwatchable listed member as completed progress.
Prove the error and ESRCH paths with the real cleanup decision and injected observation edges.
The proof must check the outcome and absence of active retries, not a fixed helper call count.

Authority: BUILD.md rule 5 and the user's prohibition on busy-spinning children.

### F37 — MEDIUM — OPEN: the payload guard still hides its cleanup failure

Evidence: `payload_guard.rs:48-68,79-83,121-126` at the reviewed head.

The payload anchor now returns a failure status and writes its report to stderr.
The shell prefix redirects that stderr to /dev/null.
PayloadGuard::drop waits only for its request thread. It does not observe the anchor's cleanup outcome.
Thus a returned inspection, wait, or deadline failure does not reach the test through this guard.
The PR discloses this limitation and cites test EOF checks.
Those checks can show a retained writer, but they do not report every guard error or the remaining members.

Required change: Make the payload guard's cleanup failure observable to its owner.
Preserve production ownership of payload reaping and the existing PTY-drain ordering.
Prove that a cleanup failure reaches the test with its cause during normal Drop and panic cleanup.
Use the shared cleanup path and injected edges. Do not add a second cleanup implementation.

Authority: independent test ownership, reported cleanup failures, and the user's actual-behavior test requirement.

### F38 — MEDIUM — OPEN: the new ownership and failure proofs remain incomplete

Evidence: `process_guard.rs:375-423` and the unchanged real guard fixtures at the reviewed head.

The new ForkRace model corrects the old order proof.
The native regression proves successful cleanup for the observed fork race.
The real early-leader-exit test checks the anchor's group before cleanup starts.
It does not check the reservation after the anchor leaves that group, through the later group signals.
The deadline and error tests call end_members directly.
They do not prove the guard's failure report or the native inspection and wait adapters.
The ForkRace test also asserts the literal ended list [1, 2] instead of deriving the expected members from its setup.

Required change: Prove ownership across the anchor's group change and all later group signals.
Use owned processes for the native ownership fact and injected edges for controlled cleanup failures.
Check the guard's visible cleanup result and the actual process effects.
Derive expected member values from the test setup.
Retain the completed native regression and the meaningful ForkRace decision proof.
Do not add tests that only inspect helper counters or exist only to kill a mutant.

Authority: the user's test-quality requirements and BUILD.md testing rule 3.

### F39 — LOW — OPEN in #163's later merge scope: equal cleanup deadlines

The integration reviewer assigns G2 to the #163 merge delta, because the guard PR lands first.
PR #163 bounds its outer anchor wait at ten seconds.
PR #165 permits the anchor's inner cleanup to use the same ten seconds.
The outer deadline can expire before the anchor returns its legitimate failure report.

Required change: Resolve the outer allowance when #163 incorporates the guard.
Derive the allowance from the shared cleanup bound with enough time for the report and owner wait.
Review the actual merge resolution before its gate.
This requirement does not increase the inner cleanup timeout and does not block #165 for an absent merge delta.

### Verdict

PR #165 is NOT CLEAN for F35, F36, F37, and F38 on this exact head.
F34 is CLOSED at its recorded correction head.
F39 remains OPEN for the later #163 merge delta.
PR #163 separately retains F28 and F33 from round 80.
The integration reviewer's G1, G3, and G4 closures do not close this reviewer's remaining source paths.
All earlier findings, closures, and verdict rounds remain preserved.
M2a, M2b, mutation, and conformance duties remain unchanged.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (4 open findings in PR #165) on `36702023b8a36876ad226ac61adb63723511f28e`.


## Round 82 — Guard reports and two-phase cleanup

Reviewed head: `97b8e94767e862b7349d8542306f91c9bed45bb0`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
The reviewer read the full five-file delta `3670202..acd05de` and the one-file delta `acd05de..97b8e94`.
The review includes the changed guard owners, not only the shared cleanup helper.

### Completed evidence

The reviewer read the completed focused Mac logs for `acd05de6` and `97b8e947`.
Their paths are:

- `~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-acd05de6-mac-20261004-220807-29615.log`
- `~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-97b8e947-mac-20261004-220951-33326.log`

The first passes slow clippy, worker prebuild, 142 tests, and 11 selected core guard tests. It exits 0 after eight seconds.
The second passes slow clippy, worker prebuild, 147 tests, and 12 selected core guard tests. It exits 0 after nine seconds.
Each core command skips nine other tests through its explicit guard filter.
Both logs name base `144b0234fb632bcbb5176b17c2fe55f3239405df`.
The real parent-death regression and the new real GroupGuard failure test pass in all six selected binaries that contain the guard.
At the current head, the slow_process parent-death regression passes in 0.046 seconds.
These are completed focused native Mac proofs. Neither log supplies a full landing gate or Linux execution.
Neither branch contains PR #163's required failed-watch test, so F33 remains unchanged.

### F35 — MEDIUM — OPEN: the principal error paths are corrected

The Mac path no longer treats every refused pidinfo call as disappearance.
It uses kqueue registration to distinguish an exiting or absent process from an observation failure.
Other inspection and registration errors now fail the listing.
The Linux path now permits only NotFound or ESRCH read failures to omit a process.
Other stat read errors and missing fields fail the listing.
The group signal now returns its error to the cleanup decision.
These corrections satisfy the principal source requirements of F35.

One unreadable-field path remains at `process_guard.rs:451` on `acd05de6`, unchanged in the current head.
The Linux path still uses `pgrp.parse::<i32>().ok()` and treats a failed parse as a non-member.
It therefore omits that process without establishing group membership or disappearance.
Required change: Return the unreadable-stat error when a required field cannot be parsed.
Do not classify an observation failure as a proved non-member.
The reserved-group ESRCH exception is recorded separately as F42 below.

### F36 — MEDIUM — CLOSED at acd05de6

The Mac adapter now distinguishes an error event, a deadline, and an observed event.
It propagates EventData::Error rather than discarding the result.
Watch ESRCH returns Waited::Gone.
The shared decision retains members called Gone in the preceding round.
A member called Gone again while still listed returns Failure::Left instead of repeating indefinitely.
The controlled decision test proves this retained-member outcome.
The Linux adapter also distinguishes its timeout from exit readiness and preserves other errors.
The native regression and corrected error decision tests pass in the completed focused Mac proof.
The current head retains these corrections.
This closes F36's discarded-error and repeated-Gone retry paths.

### F37 — MEDIUM — OPEN: the payload report path still accepts failures as success

The payload anchor now sends `ok` or `fail: <why>` through its guard socket.
The request thread reads that report, and PayloadGuard::drop rejects an explicit failure report.
GuardedPayload and OwnedWorker call release before production cleanup and read the report afterward.
These changes correct the former stderr-to-/dev/null report path.

Remaining evidence: `payload_guard.rs:39-63,103-116` at the current head.
The request thread discards report read errors.
PayloadGuard::drop uses thread.join().unwrap_or_default(), so a thread panic becomes an empty report.
It also accepts every result that lacks the `fail: ` prefix, including malformed reports.
The comment says an empty report means production killed the member.
A read error or thread panic also produces that result, so the code has not established that condition.

Required change: Preserve transport and thread failures in the visible cleanup result.
Validate the report instead of treating every non-failure string as success.
Keep the allowed production-killed case distinct from an observation failure.
Prove the failure through PayloadGuard and its owner during normal Drop and panic cleanup.
The passing GroupGuard failure test does not exercise this separate socket/report path.
Preserve production reaping and the required PTY close ordering.

### F38 — MEDIUM — OPEN: the native guard proof improves; a helper-count assertion remains

The anchor now checks its reservation with waitid NOWAIT|NOHANG before every group signal.
The check fails if the reservation is no longer its unreaped child.
The reservation inherits the group when it forks; the anchor moves out afterward and reaps the reservation after its final decision.
The successful native guard tests execute those checks and verify real process cleanup.
Together with the source ownership rule, this satisfies the recorded reservation proof requirement.

The new a_cleanup_that_cannot_finish_fails_through_the_guard uses a real GroupGuard with a zero cleanup allowance.
The member blocks on a FIFO; the anchor reports members left; guard Drop panics with that report.
The test then observes pipe EOF and reaps only its own fixture child.
This is an actual guard failure proof, not a helper-only assertion.
The named first and joiner values also replace the literal expected member list in the ForkRace proof.

However, the deadline decision test now counts kill calls and asserts kills == rounds + 1.
Its injected expiry depends on a helper check count, and its wait always returns Deadline.
This tests the round helper's internal call sequence, contrary to the user's requirement and BUILD.md rule 3.
Required change: Remove the helper-count assertion and its supporting counter setup.
Use a meaningful deadline outcome and derive expectations from the test's state or real effects.
The completed real guard failure proof can cover the same clause if the redundant helper test is removed.
Keep the meaningful ForkRace behavior proof.
F37 separately owns the missing PayloadGuard report proof; it need not be duplicated under F38.

### F40 — HIGH — The driver harness no longer requests independent cleanup before production reaping

Status: OPEN.
Evidence: `crates/botster-worker/tests/common/driver_edges.rs:11-18,380-396` at the reviewed head.

The delta moves Driver before PayloadGuard in Harness field order.
Harness has no Drop implementation and never calls release before its Driver drops.
The production payload destructor therefore runs before the guard receives its cleanup request.
If production does not end the group, or blocks in its reaper, the test never drops the later guard field.
The independent guard cannot perform the cleanup that the user requires it to own.
This reverses the earlier cleanup order and creates a new failure path.
The implementer's message says the harness releases first; the source does not do that.

The real-loop test also drops its guard before it receives the driver result.
The new guard Drop waits for the report, which can require production to close the PTY master first.
This violates the new release-before-production, Drop-after-production contract and can hold the test before its marked retirement deadline.

Required change: Request independent cleanup before any production wait can block.
Retain the guard until production closes the PTY and finishes its bounded cleanup.
Apply that order to ordinary Harness Drop, panic cleanup, and the test that moves Driver into a thread.
Keep the worker and payload reapers in production.
Prove that the independent guard still ends the group when production cleanup fails, without a busy child or an unbounded owner wait.

Authority: the user's real-process ownership requirement and BUILD.md testing rules 3 and 5.

### F41 — LOW — The shared guard module combines too many responsibilities

Status: OPEN.
At `acd05de6`, process_guard.rs has 795 lines. The current head adds another decision test.
The file combines socket registration, guard ownership, platform process inspection, exit waits, cleanup decisions, models, and real-process fixtures.
The newest error paths and owner changes require reading these distinct responsibilities together.
This is the oversized module that the user explicitly prohibited.

Required change: Split these responsibilities into small shared modules.
Keep one cleanup decision path for both guards.
Keep platform adapters separate from ownership and the test fixtures.
Retain the existing behavior proofs and helper process registration after the move.
A file move must not introduce a second cleanup implementation or new production test branches.

Authority: the user's module-quality requirement and one-code-path rule.

### F42 — LOW — CLOSED at 97b8e947: a reserved group can have no signalable member

The integration reviewer opened G6 at verdict commit `bb23fd6` on `acd05de6`.
A member can exit after the listing; a BSD-style group signal can then return ESRCH when only zombies remain.
Treating that case as an unconditional failure can reject successful cleanup.
The current correction lets a signal ESRCH reach the next listing after the reservation check succeeds.
Other signal failures still return Failure::Error.
The controlled decision proof checks the subsequent empty listing and successful outcome.
The reservation check converts its own failure to a distinct error, so it cannot enter the signal ESRCH exception.
The integration reviewer closes G6 at verdict commit `4c25d96` and reports zero integration findings on this head.
That integration CLEAN remains conditional on this package's exact-head CLEAN.

### Verdict

PR #165 is NOT CLEAN for F35, F37, F38, F40, and F41 on this exact head.
F36 is CLOSED at `acd05de6`; F42 is CLOSED at the current head. F34 retains its recorded closure.
F39 remains OPEN only for the later #163 merge delta.
PR #163 separately retains F28 and F33 under round 80.
All earlier findings, closures, and verdict rounds remain preserved.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (5 open findings in PR #165) on `97b8e94767e862b7349d8542306f91c9bed45bb0`.


## Round 83 — Shared modules and independent cleanup requests

Reviewed head: `f5f9d0f64252d41803dcc1b53ca9b586bb90f5cb`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `97b8e947..f5f9d0f6`, six files.
The reviewer read the complete new cleanup and platform modules, the guard owner changes, and the retained real fixtures.

### Completed evidence

The reviewer read the completed focused native Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-f5f9d0f6-mac-20261004-221717-52020.log`.
The log names this exact head, base `144b023`, slow clippy, worker prebuild, and the focused nextest commands.
It exits 0 after 17 seconds.
The first command passes 155 tests with zero skips.
The second command passes 13 selected core guard tests and skips nine other tests.
The parent-death regression passes in every selected binary that contains it.
The new PayloadGuard failure test passes in slow_payload and both session fixture locations.
The existing GroupGuard failure test and the moved cleanup decision tests also pass.
This is a focused native Mac proof, not a full landing gate or Linux execution.
The integration reviewer reports zero integration findings at verdict commit `113f3f8`, conditional on this package's exact-head CLEAN.

### F35 — MEDIUM — CLOSED at f5f9d0f6

The Linux adapter now returns an unreadable-group error when the required pgrp field does not parse.
It no longer treats that observation failure as a proved non-member.
The earlier inspection and signal corrections remain in the shared platform and cleanup modules.

The Mac listing also handles libproc's reported empty-list error through a group existence probe.
Only ESRCH permits an empty result; otherwise it preserves the listing error.
The probe uses signal zero and does not end a process.
The native completed cleanup proof exercises the moved Mac path.
This closes F35's remaining source requirement.

### F37 — MEDIUM — OPEN: report errors are corrected; registration errors still become no ownership

The report path now preserves read errors and thread panics.
Drop validates `ok` and `fail: <why>` and rejects a malformed report.
A registered group with no report requires the shared observation rounds to establish an empty group.
Those observation rounds send no terminating signal because the test process holds no group reservation.
The new real PayloadGuard failure test observes a failure report, the owner's panic, pipe EOF, and fixture-child completion.
These changes satisfy the prior report-path source and normal-Drop proof requirements.

Remaining evidence: `payload_guard.rs`, serve, its registration loop and group parsing.
The registration loop still returns Outcome::unregistered() on accept errors, tag read errors, or an invalid tag.
It does this even if an anchor connection was accepted earlier.
The group line also parses through .ok(), and an invalid line becomes group: None.
The function then sends both readiness acknowledgements anyway.
If that registered stream later ends without a report, Drop accepts group: None as owning nothing.
An observation or protocol failure can therefore still become successful cleanup.

Required change: Preserve registration failures after the member connects.
Reject an invalid registered group before sending readiness.
Distinguish cancellation before registration from an established connection whose ownership could not be read.
Do not infer no ownership from a failed read or invalid protocol value.
Prove these outcomes through the shared registration/report path and retain the real cleanup failure proof.

### F38 — MEDIUM — OPEN: the new reservation test checks results without process effects

The deadline test no longer asserts the number of helper kill calls.
It checks the reported members left. This corrects the round 82 helper-count finding.
The meaningful ForkRace proof and both real guard failure proofs remain accepted.

The new guard_cleanup::a_kill_goes_out_only_while_the_reserve_holds_the_group starts /usr/bin/true.
That program can exit before the first signal, and the test accepts either success or ESRCH.
The test then reaps that process and checks the reservation error text.
It does not observe a held member ending because of the signal.
It also does not observe another owned member remaining alive when the reservation has been released.
Its comments claim those process effects, but its assertions only check helper results.
A helper that returned the accepted result without sending a signal could pass the first assertion.

Required change: Prove the claimed behavior with controlled owned processes and actual effects.
A held member must end through the guarded signal.
A released reservation must prevent a signal from ending another owned member.
Use real events and bounded waits, with no busy child and no production reaping by a test guard.
Alternatively, remove a redundant test and name the existing behavior proofs that cover its complete claim.
The earlier native ownership proof remains recorded; this finding does not discard it.

### F40 — HIGH — CLOSED at f5f9d0f6: independent cleanup starts before production waits

Harness now holds a Release value before Bounded<Driver> in its field order.
Release Drop sends the independent cleanup request before production's destructor can block.
The guard remains after the driver and reads its report after production closes the PTY.
The Bounded owner moves production Drop to a helper thread and uses a marked completion deadline.
These ownership rules also apply when the test panics.

The real-loop test takes Driver out of the owner, releases before Remove, and drops the guard after the driver's completed result.
It no longer joins the report path before production can close the PTY.
The real PayloadGuard failure proof ends its fixture member through the independent guard, without a production cleanup signal.
The native driver edge and real-loop proofs also pass on this exact head.
This closes F40's missing cleanup request and reversed report order.
F39 separately records the new outer deadline allowance.

### F41 — LOW — CLOSED at f5f9d0f6: the shared guard has separate responsibilities

process_guard.rs retains registration, ownership, its anchor, and the real guard fixtures.
guard_cleanup.rs contains the one cleanup decision path, reservation checks, outcomes, and decision proofs.
guard_platform.rs contains platform listing and exit-wait adapters.
PayloadGuard calls the same cleanup module.
The shared module paths work in the selected core-sys, core, and worker binaries.
The native log records the retained helper registration and real process proofs after the move.
No production test branch or duplicate cleanup implementation appears.
This satisfies the module-quality requirement.

### F39 — LOW — OPEN: the new outer driver wait has the same bound as inner cleanup

Round 82 assigned the equal outer/inner deadline issue only to #163's later merge delta.
This head introduces Bounded<Driver> in #165 itself.
Its outer completion wait uses CLEANUP, while the independent anchor can use the same CLEANUP to finish its group cleanup.
The release starts that inner cleanup immediately before the outer production wait begins.
The outer wait can therefore report failure before legitimate inner cleanup and production completion finish.
This is the same deadline composition problem on a new source path.
F39 now belongs to the current #165 scope as well as the later #163 merge delta.

Required change: Derive the outer allowance from the inner cleanup bound and the completion/report allowance.
Retain the existing inner cleanup timeout.
Apply the same rule when #163 incorporates the guard and its bounded owner waits.
Do not close this finding from passing fast-path timings; the full permitted inner deadline must fit.

### Verdict

PR #165 is NOT CLEAN for F37, F38, and F39 on this exact head.
F35, F40, and F41 are CLOSED at this head.
F34, F36, and F42 retain their recorded closures.
PR #163 separately retains F28 and F33; its later merge also retains the F39 duty.
All earlier findings, closures, and verdict rounds remain preserved.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (3 open findings in PR #165) on `f5f9d0f64252d41803dcc1b53ca9b586bb90f5cb`.


## Round 84 — Registration errors and real reservation effects

Reviewed head: `08fef89d22657db2b558e63d8da8435c9398765a`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `f5f9d0f6..08fef89d`, two commits, three files.
The reviewer read the complete registration, reservation-test, and outer-deadline changes.
The integration reviewer reports zero integration findings at verdict commit `ffcc8f5`, conditional on this package's exact-head CLEAN.

### Completed evidence

The completed focused native Mac log is:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-08fef89d-mac-20261004-222304-71264.log`.
The log names this exact head and base, slow clippy, worker prebuild, and the focused nextest commands.
It exits 0 after nine seconds.
The first command passes 155 tests with zero skips.
The second passes 13 selected core guard tests and skips nine other tests.
The actual reservation signal/no-signal test passes in all six selected binaries that contain it.
The parent-death regression, both guard failure proofs, and driver edge tests retain their passing results.
This is a completed focused Mac proof, not a full landing gate or Linux proof.

### F37 — MEDIUM — OPEN for the controlled registration proof

The registration path now retains accept and read failures.
It rejects unknown or repeated tags and an invalid registered group before sending readiness.
A ready helper without a member returns a failure.
A member without a ready helper receives the cleanup request and retains its report path.
A connection that ends before its tag can cancel registration; the guard preserves an already registered member in that case.
These source corrections close the remaining silent-registration-error paths from round 83.
The earlier report validation and real PayloadGuard cleanup-failure proof remain accepted.

The required controlled registration-error proof has not arrived.
This delta changes the registration code but adds no test of its rejected registration outcomes through the owner/report path.
The completed log contains the existing cleanup-limit failure proof, which exercises a different path.
Required evidence: Prove that a rejected registration reaches the guard owner as a failure and does not acknowledge readiness.
Use the real registration/report path with controlled inputs.
Retain a proving cancellation case so a launch that never registers does not become an incorrect failure.
Use observable behavior rather than helper counters or internal literal comparisons.
F37 remains OPEN for this evidence requirement only.
F43 below is a separate source finding, so this head is not yet eligible for evidence-only closure.

### F38 — MEDIUM — CLOSED at 08fef89d: the reservation test observes process effects

The reservation test now creates members blocked in a FIFO open.
With the reservation held, reserved_kill ends the group and the member's exit status identifies SIGKILL.
After the reservation is reaped, reserved_kill rejects the signal.
The remaining member then completes its FIFO read and exits normally.
Thus the test observes both effects that the previous /usr/bin/true proof only claimed.
It derives the expected signal from Signal::KILL rather than a numeric signal literal.
The blocking FIFO writer has a marked deadline, which corrects the intermediate nonblocking-open startup race.
The completed native log records both cases passing.
This closes F38's required behavior proof.
F43 separately records the test's missing cleanup ownership and process-wait bounds.

### F39 — LOW — CLOSED in #165 at 08fef89d; OPEN for the later #163 merge delta

Bounded<Driver> now derives its outer completion allowance as 2 * CLEANUP.
The code reserves one CLEANUP interval for the independent anchor and another for production completion and its report.
The inner cleanup limit remains unchanged.
This closes F39 on the current #165 source path.
PR #163 still must apply the same composition rule when it incorporates the guard and resolves its bounded owner waits.
The recorded later merge duty remains OPEN in that separate scope.

### F43 — HIGH — The new real reservation test can leave children or wait forever

Status: OPEN.
Evidence: `crates/botster-core-sys/tests/common/guard_cleanup.rs:275-337` at the reviewed head.

The test holds reserve, held, and alive as raw std::process::Child values.
No guard kills their group on panic or early return. Child Drop does not end the process.
If reserved_kill returns an unexpected error or an assertion fails, blocked FIFO members can remain after the test.
The waits for held, both reserves, and alive are also unbounded.
If the tested signal does not occur, held.wait() can hold the job indefinitely before any marked deadline.
The bounded FIFO open does not bound these process waits or provide cleanup ownership.
This is a new resource-ownership defect in the rewritten proof, despite its passing baseline.

Required change: Give every fixture group independent cleanup ownership on every exit path.
Preserve the group identity until its final signal; never signal an identity after releasing its owner.
Bound the observed process completions with real exit events and marked deadlines.
Reap only the fixture children that this test owns.
Keep the actual held-signal and released-no-signal assertions.
Do not replace the proof with helper return checks, sleeps, or busy children.

Authority: the user's real-process guard requirement, no released-ID signals, and BUILD.md testing rules 3 and 5.

### Verdict

PR #165 is NOT CLEAN for F37's evidence and F43's source defect on this exact head.
F38 and F39 are CLOSED at this head within #165's scope.
F34, F35, F36, F40, F41, and F42 retain their recorded closures.
PR #163 separately retains F28, F33, and its later F39 merge duty.
All earlier findings, closures, and verdict rounds remain preserved.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (2 open findings in PR #165) on `08fef89d22657db2b558e63d8da8435c9398765a`.


## Round 85 — Registration proof and fixture child owner

Reviewed head: `4c06846d039b9209cbd2d8d57144bbad807b6d98`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `08fef89d..4c06846d`, three commits, two files.
The reviewer read the complete child-owner and registration-proof changes.
The integration reviewer reports zero integration findings at verdict commit `7dc2559`, conditional on this package's exact-head CLEAN.

### Completed evidence

The reviewer inspected the completed focused native Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-4c06846d-mac-20261004-222705-81386.log`.
It names this exact head and base, slow clippy, worker prebuild, and the focused nextest commands.
It exits 0 after eight seconds.
The first command passes 158 tests with zero skips.
The second passes 13 selected core guard tests and skips nine other tests.
The new registration test passes in all three selected binaries that include PayloadGuard.
The reservation effect test passes in all six selected binaries that include the shared cleanup module.
The cancelled failed-spawn case and the real parent-death regression also pass.
This is a completed focused Mac proof, not a full landing gate or Linux proof.

### F37 — MEDIUM — CLOSED at 4c06846d

The new test connects an untrusted registrant to the real PayloadGuard socket.
One case registers an invalid group; another case supplies a ready helper without a member.
Both cases release and drop the actual guard, observe its failure report, and read EOF without a readiness byte.
The read has a marked deadline configured while the stream is open.
The test therefore checks the owner's failure and rejected readiness, not the registration helper's internals.
The existing failed-spawn test covers release before any registration and completes quietly.
The earlier report-validation and real cleanup-failure proofs remain accepted.
Together with round 84's source correction, this closes F37.

### F43 — HIGH — OPEN: child ownership is added, but observation errors still bypass the bound

Evidence: `crates/botster-core-sys/tests/common/guard_cleanup.rs:273-320` at the reviewed head.

The test now wraps each fixture child in Owned.
Its normal completion path first observes the child with waitid NOWAIT on a helper thread and a marked deadline.
Owned Drop kills only its unreaped fixture child, then observes and reaps that child.
The test still observes the required held-signal and released-no-signal effects.
These changes correct the absence of an owner and bound the successful observation path.

Remaining source defects:

- ended_within_cleanup exits its loop on every non-INTR waitid error and sends the same completion value as an exit result.
- Owned::status then calls Child::wait without distinguishing an observed exit from an observation failure.
- Owned::drop returns on every try_wait error, so it can abandon a child without a cleanup report.
- Drop discards kill and reap errors.
- The observation loop also retries Ok(None) immediately, rather than requiring a blocking exit result or reporting an invalid outcome.

Thus an error is still treated as proof of completion, and the later wait has no established exit precondition.
The new boolean loses the errno and the distinction needed to enforce the completion bound.
The successful native baseline does not exercise or close those source paths.

Required change: Preserve the observation result and its error.
Require an actual exit result before the synchronous reap.
Retry only an interrupted blocking wait; do not spin on a successful None.
Handle try_wait, kill, and reap errors explicitly and report cleanup failure during both normal Drop and an existing panic.
Keep ownership until the final cleanup decision and signal only an owned identity.
Retain the actual process-effect proof and the existing deadlines.
Do not invent an exit result from a failed observation.

### Verdict

PR #165 is NOT CLEAN for F43 on this exact head.
F37 is CLOSED at this head.
F34, F35, F36, F38, F39, F40, F41, and F42 retain their recorded closures within #165's scope.
PR #163 separately retains F28, F33, and its later F39 merge duty.
All earlier findings, closures, and verdict rounds remain preserved.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (1 open source finding in PR #165) on `4c06846d039b9209cbd2d8d57144bbad807b6d98`.


## Round 86 — Shared guard CLEAN on the amended pause boundary

Reviewed head: `c03bcfb181d21cdf805b752a790359f63b9ef7b9`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `4c06846d..c03bcfb1`, two commits, only guard_cleanup.rs.
The lead's direct amended pause order authorizes this near-done review and the later #163 merge delta.
M2a and the A32/A33 follow-up remain paused.
The reviewer read the complete delta and the completed focused native Mac log.

### F43 — HIGH — CLOSED at c03bcfb1

ended_within_cleanup now returns the observation error or a bounded completion result.
Only an interrupted blocking wait retries.
A successful wait with no status returns an error rather than repeating without a blocking event.
Only an actual waitid exit result produces the successful completion value.
Owned::status therefore reaps only after an observed exit.

Owned Drop now distinguishes an already reaped child, a live owned child, and a failed ownership check.
It reports failed checks, kills, observations, and reaps.
During an existing panic, it prints the cleanup failure instead of causing a second panic.
It signals only its unreaped fixture child and retains the marked completion deadline.
The actual held-SIGKILL and released-normal-exit assertions remain unchanged.
This closes the remaining observation-error and silent-cleanup paths in F43.

### New EPERM rule — accepted

The shared cleanup decision now permits a group signal EPERM to reach the subsequent observation and listing.
It applies the same final-state rule already used for ESRCH.
A refused signal alone does not produce success.
Success still requires a listing with no live member.
A member that cannot end remains a reported failure at the cleanup deadline or through a failed observation.
The existing repeated-Gone rule prevents an unwatchable retained member from driving an indefinite active retry loop.
The reservation failure remains a distinct error and cannot enter either signal-errno exception.
The group identity stays reserved through every signal.

The revised decision proof supplies ESRCH and EPERM separately, then observes a subsequent empty listing and successful cleanup.
The retained-member and deadline proofs continue to check failure outcomes through the same decision path.
The real cleanup and reservation-effect proofs remain accepted.
The reviewer accepts this final-state decision without inferring successful cleanup from the signal errno alone.

### Completed native evidence

The completed focused Mac log is:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-c03bcfb1-mac-20261004-223014-89077.log`.
It names this exact head and base, slow clippy, worker prebuild, and the focused nextest commands.
It exits 0 after 12 seconds.
The first command passes 158 tests with zero skips in 2.019 seconds.
The second passes 13 selected core guard tests and skips nine other tests in 0.104 seconds.
The reservation effect proof passes in all six selected binaries that contain it.
The actual registration rejection and payload cleanup failure proofs pass in each selected PayloadGuard location.
The parent-death regression passes in slow_process in 0.069 seconds and also passes in the other selected guard binaries.

This is the required focused native Mac proof for the guard correction.
It is not a full landing gate or Linux proof.
The reviewer ran no tests, builds, measurements, mutants, or gates.
The integration reviewer reports zero integration findings at verdict commit `b2f7db0` on this exact head.
Its integration CLEAN was conditional on this package's exact-head CLEAN, which this round supplies.
The implementer and lead still own the required landing and merge steps.

### Verdict and scope

F34 through F43 are CLOSED within PR #165 at their recorded correction heads and scopes.
F39's #165 requirement is closed; its separate later #163 merge requirement remains open.
PR #165 has no open package finding, including LOW, on this exact head.
PR #163 separately retains F28, F33, and its later F39 merge duty.
This CLEAN does not clear #163, the unreviewed M2a restack, M2b, the A32/A33 follow-up, or pending conformance ids.
All earlier findings, closures, and verdict rounds remain preserved.

VERDICT: CLEAN for PR #165 on `c03bcfb181d21cdf805b752a790359f63b9ef7b9`.


## Round 87 — Remaining waits and sleep fixtures in the shared guard

Reviewed head: `71195e7209cc0cbee03bedb52eda3f2b83db2585`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `c03bcfb1..71195e72`, two commits, two files.
The implementer submitted this correction for P5-F4 under the lead's near-done review scope.
The reviewer read the complete delta and the remaining early-exit and panic fixtures.
The integration reviewer records C3 MEDIUM and C4 LOW at verdict commit `cef28d1` on this exact head.
The reviewer accepts those findings in this package scope.
The provisional assessment sent before reading that integration message is corrected by this verdict.

### F44 — MEDIUM — CLOSED at 71195e72: the parent-death fixture bounds its first read and parent reap

This finding records the transferred P5-F4 requirement.
The previous parent-death regression read its readiness line directly on the test thread.
Its Parent Drop also killed the child and then called an unbounded wait.
Those paths could hold the test instead of producing a bounded failure.

The current regression uses first_line with the marked CLEANUP deadline.
It replaces Parent with the existing cleanup::Owned owner.
That owner observes the exit within the deadline before it reaps, and reports each ownership or cleanup error.
It reaps only the fixture parent. The independent anchor still ends the worker group after that parent's death.
The EOF wait now uses the same CLEANUP value.
No production reaper or worker path changes.

The two new zero-limit tests use actual cat processes blocked on FIFOs.
They exercise a read that cannot complete and a child wait that cannot complete.
Each observes the corresponding visible failure through the same bounded function used by the fixtures.
The Owned wrapper provides the retained cleanup path for the blocked fixture child.
The proof uses no sleep, busy child, helper call counter, or production test branch.
This closes P5-F4's two named paths, recorded here as F44.

### F45 — MEDIUM — The early-leader-exit proof still has two unbounded waits

Status: OPEN. This is integration C3.
Evidence: `crates/botster-core-sys/tests/common/process_guard.rs:345-370` at the reviewed head.

an_early_exit_keeps_the_group_owned_until_cleanup directly calls pipe.read_line before any deadline.
It then calls child.wait on the test thread before the guard drops.
A missing descendant line or a leader that does not exit can therefore hold the test before its bounded EOF observation.
These are the same unbounded-read and unbounded-reap defects corrected in the separate parent-death fixture.
They remain outside that correction's source path.

Required change: Use the bounded first-line path and an observed-exit child owner for these waits.
Preserve the required order: the fixture owner reaps the leader while the independent anchor still owns the group.
Then prove that guard cleanup ends the remaining descendant.
Keep the anchor membership check and the actual EOF effect.
Do not let the guard reap a worker or payload that production owns.

Authority: BUILD.md testing rules 3 and 5, independent group ownership, and integration C3.

### F46 — LOW — Two guard self-tests still use sleep loops

Status: OPEN. This is integration C4.
Evidence: `process_guard.rs:320-340,345-370` at the reviewed head.

a_panic_before_ready_ends_the_child runs a shell loop with /bin/sleep 1.
an_early_exit_keeps_the_group_owned_until_cleanup starts the same sleep loop in its background descendant.
These unchanged fixtures still violate BUILD.md's no-sleeps-or-polling rule.
A timed loop supplies no required process or terminal behavior for either ownership proof.

Required change: Replace these hold bodies with blocked event-driven fixtures, such as a FIFO open with no writer.
Preserve panic cleanup before readiness and ownership after the leader has been reaped.
Keep the real EOF and exit observations and their marked bounds.
The current #165 closure must resolve this LOW finding; a historical package CLEAN does not waive it.

Authority: BUILD.md testing rule 5, the user's event-wait requirement, and integration C4.

### Completed evidence

The reviewer read the completed focused native Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-71195e72-mac-20261004-224513-28395.log`.
It names this exact head and base, slow clippy, worker prebuild, and the focused nextest commands.
It exits 0 after nine seconds.
The first command passes 168 tests with zero skips in 1.185 seconds.
The second passes 15 selected core guard tests and skips nine other tests in 0.087 seconds.
Both new timeout cases and the actual parent-death regression pass in all six selected binaries that contain them.
The other accepted registration, cleanup failure, reservation, and driver proofs retain passing results.
This successful baseline does not close F45's missing wait bounds or F46's prohibited fixture bodies.
It is not a full landing gate or Linux proof.

### Verdict

PR #165 is NOT CLEAN for F45 and F46 on this exact head.
F44 is CLOSED at this head. F34 through F43 retain their recorded closures within #165's scope.
Round 86's CLEAN remains preserved for its named earlier head; it does not clear this new head or these later findings.
PR #163 separately retains F28, F33, and its later F39 merge duty.
M2a and A32/A33 remain paused under the lead's amended order.
All earlier findings, closures, and verdict rounds remain preserved.
The reviewer ran no tests, builds, measurements, mutants, or gates.

VERDICT: NOT CLEAN (2 open findings in PR #165) on `71195e7209cc0cbee03bedb52eda3f2b83db2585`.


## Round 88 — Correction delta and whole guard review after resume

Reviewed head: `88faedde845c53b62690a4648045988e63d67dd3`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `71195e72..88faedde`, one commit, two files.
The reviewer also read the whole guard change from the base, including all eleven changed files and the PR description.
Authority: BUILD.md testing rules 3, 5, and 10; the lead's independent-anchor and exclusive-production-reaping rulings.

### F45 — MEDIUM — CLOSED at 88faedde: the early-leader waits have bounds

The early-leader test now reads its descendant line through first_line with the marked CLEANUP deadline.
It wraps the fixture leader in cleanup::Owned and calls status before dropping the independent GroupGuard.
status requires an observed exit within CLEANUP before reaping the fixture leader.
The test then checks the anchor's membership in the original group and drops the guard.
Its bounded EOF observation preserves the actual descendant-cleanup proof.
Owned retains fixture cleanup on assertion failure and panic.
This closes F45, which is integration C3.
The guard still does not reap a worker or payload owned by production.

### F46 — LOW — CLOSED at 88faedde: both members wait on a FIFO

The panic-before-ready fixture now runs /bin/cat against a FIFO with no writer.
The early-leader fixture starts the same blocked member in the background, reports its PID, and exits the leader.
Both members wait on an event without a sleep loop.
Both tests retain the actual pipe EOF observations and marked bounds.
The panic fixture also obtains its child's status through Owned's bounded exit observation.
This closes F46, which is integration C4.
The GroupGuard cleanup-failure fixture also replaces its raw child and direct wait with Owned and status.

### F47 — MEDIUM — The PayloadGuard failure fixture still has an unbounded reap

Status: OPEN.
Evidence: `crates/botster-core-sys/tests/common/payload_guard.rs:366-389` at the reviewed head.

The whole PR adds a_payload_cleanup_that_cannot_finish_fails_through_the_guard.
That test retains its fixture shell as a raw Child and calls payload.wait directly on the test thread.
The zero-limit guard deliberately reports members left after its final signal.
That report does not establish an observed child exit before the reap.
The EOF observation does not supply that precondition either.
The script redirects the exec'd cat's stdout to /dev/null, so the pipe can close while cat remains blocked.
A missing child exit can therefore hold the test without its own completion deadline.
An earlier assertion failure also skips the raw Child's reap.
The parallel GroupGuard fixture now uses Owned, but this separate PayloadGuard fixture does not.

Required change: Give this fixture child an owner on every exit path.
Use the existing bounded exit-observation path before its final reap.
Preserve the actual PayloadGuard failure report and independent group cleanup.
Reap only this fixture child; preserve production's exclusive reaping in the production-backed tests.
Retain the existing cleanup limit and event-driven FIFO member.

Authority: BUILD.md testing rules 5 and 10 and the same bounded-observation requirement that closes F45.

### F39 — LOW — OPEN in #165 for the real-loop retirement wait

Evidence: `crates/botster-worker/tests/common/driver_edges.rs:437-455` at the reviewed head.

Round 84 closed the Bounded<Driver> path by deriving its outer allowance as 2 * CLEANUP.
The whole PR also changes pty_events_resume_reads_after_would_block to request independent cleanup before Remove.
That test takes Driver out of Bounded and runs Driver on a separate thread.
Its result wait still uses Duration::from_secs(10), equal to the independent member's CLEANUP interval.
It therefore omits the completion allowance that the other owner path now supplies.
The outer wait can expire before cleanup and the driver result complete.
This separate path was not covered by round 84's closure.

Required change: Derive this outer retirement allowance from CLEANUP and include cleanup completion and result delivery.
Apply the accepted composition rule used by Bounded<Driver>.
Keep the inner CLEANUP limit unchanged and retain release-before-production, report-after-production order.
F39 also remains OPEN for #163's separate later merge delta.

### F48 — LOW — The PR description still claims that payload cleanup reports are unseen

Status: OPEN.
Evidence: PR #165's description, read at the exact reviewed head.

The Fix section says the payload member's report is unseen because stderr goes to /dev/null.
The current member sends its report through the socket.
PayloadGuard::drop reads that report and rejects failure, transport, and malformed-report outcomes.
The description therefore states the earlier limitation after F37's source correction removed it.
Its focused proof also names only the older 3670202 head.

Required change: Update the description to explain the current socket report and two-phase cleanup order.
Record the completed focused proof for the current head with its log path.
Preserve the Prior art note and identify the combined landing gate as a separate required step.

### Completed evidence

The reviewer read the completed focused native Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-88faedde-pool-20261008-204257-69794.log`.
The log names this exact head and base `9ea0c9c22d0d0595a166becbba7f9e8872247c22`.
Slow clippy with -D warnings and worker prebuild pass.
The first nextest command passes 168 tests with zero skips in 1.379 seconds.
The core guard selection passes 15 tests and skips nine other tests in 0.107 seconds.
The corrected early-leader and panic tests pass in all six selected binaries that contain them.
The parent-death regression, reservation-effect proof, report-failure proofs, and driver tests also pass.
The job exits 0 after 20 seconds on Mac.
This is focused Mac evidence; it is not a full landing gate or Linux execution.
The passing baseline does not close the remaining source findings.

### Verdict and scope

F45 and F46 are CLOSED at this head.
PR #165 is NOT CLEAN for F47 MEDIUM, F39 LOW in the real-loop path, and F48 LOW.
All earlier closures retain their recorded heads and scopes; round 84's F39 closure still covers Bounded<Driver>.
PR #163 separately retains F28, F33, and its later F39 merge duty.
This review does not clear M2a, M2b, the A32/A33 follow-up, or pending conformance IDs.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings and verdict rounds remain preserved.

VERDICT: NOT CLEAN



## Round 89 — Shared guard CLEAN after the whole guard corrections

Reviewed head: `9eaea51ccb47230f4ee19e14056c17dfb607a826`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `88faedde..9eaea51c`, two commits, three files.
The implementer submitted this head in place of the intermediate 7e20c5fa READY.
The reviewer read both complete commit deltas, the completed exact-head Mac log, and the corrected PR description.
Round 88's whole guard review supplies the unchanged source review.

### F47 — MEDIUM — CLOSED at 7e20c5fa, retained at 9eaea51c

The PayloadGuard failure fixture now wraps its shell in the existing cleanup::Owned owner.
Its final status call requires an observed exit within CLEANUP before reaping that fixture child.
Owned retains cleanup and reaping on assertion failure and panic.
The independent payload member still owns group cleanup; this change reaps no production-owned payload.
Both cleanup-failure fixtures remove cat's stdout redirection to /dev/null.
The blocked member now retains the pipe until its end, so the bounded EOF observation proves the intended process effect.
The shared FIFO helper remains event-driven and introduces no sleep or polling.
The completed focused logs at both correction heads show the actual failure proofs passing.

### F39 — LOW — CLOSED within #165 at 9eaea51c

The real-loop retirement wait now derives its outer allowance as 2 * CLEANUP.
The reason matches Bounded<Driver>: the allowance includes independent cleanup, production completion, and result delivery.
The inner CLEANUP limit is unchanged.
The test retains cleanup release before Remove and guard report observation after the driver result.
The other waits in this file observe writes or exit after a group signal; they do not include the guard's cleanup interval.
This closes the additional real-loop path from round 88.
F39 remains OPEN for #163's later merge delta and its bounded guard owner waits.

### F48 — LOW — CLOSED for the corrected PR description at 9eaea51c

The description now explains the payload member's socket report and the two-phase cleanup order.
It states the no-report exception accurately: a registered group must be proved empty within the cleanup limit.
It identifies first_line, eof, and Owned as the named bounded fixture observations.
It separately identifies GroupGuard's direct registration join and anchor wait as the #163 follow-up scope.
The reviewer checked that #163's current source supplies those bounded wrappers; their merge allowance remains F39's duty.
The description names the current exact-head focused Mac proof and labels earlier evidence as history.
It also records the combined #162 landing gate and preserves the Prior art note.
No source commit followed the description correction.

### Completed evidence

Exact-head log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-9eaea51c-pool-20261008-204745-82739.log`.
The log names head `9eaea51ccb47230f4ee19e14056c17dfb607a826` and base `9ea0c9c22d0d0595a166becbba7f9e8872247c22`.
Slow clippy with -D warnings and worker prebuild pass.
The first nextest command passes 168 tests with zero skips in 1.475 seconds.
The core guard selection passes 15 tests and skips nine other tests in 0.155 seconds.
The corrected PayloadGuard failure proof passes in all three selected binaries that contain it.
The real-loop retirement proof passes in 0.157 seconds.
The parent-death, early-leader, panic, registration, reservation, and report-failure proofs retain passing results.
The job exits 0 after 11 seconds on Mac.
The reviewer also read the completed intermediate 7e20c5fa evidence: 168 + 15 pass, exit 0 after 16 seconds.
These are focused Mac proofs. The combined #162 landing gate still owes Linux compilation and execution.

### Verdict and scope

PR #165 has no open package finding, including LOW, at this exact head.
F34 through F48 retain their closures within #165 at their recorded correction heads and scopes.
F39's separate later #163 merge duty remains OPEN.
PR #163 also retains F28 native mutation evidence and F33 required landing execution.
This CLEAN does not clear #163, M2a, M2b, the A32/A33 follow-up, or pending conformance IDs.
The P5 and integration reviews remain separate required reviews.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved.

VERDICT: CLEAN


## Round 90 — C7 cleanup allowance delta

Reviewed head: `47ae53a79b95e5499b8548c456a2fbaceacba992`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `9eaea51c..47ae53a7`, one commit, one file, four changed lines.
The lead explicitly requested this delta review after round 89's exact-head CLEAN.
The integration reviewer identifies C7 LOW as another F39 composition path.

### F39 — LOW — C7 path CLOSED at 47ae53a7

The panic-cleanup test in slow_payload.rs waits for GuardedPayload's complete destructor and the panic outcome.
Its destructor releases the independent guard, runs production cleanup, and then reads the guard report.
The earlier outer wait used ten seconds, equal to the guard's inner CLEANUP interval.
The correction now derives that outer allowance as 2 * process_guard::cleanup::CLEANUP.
The marked deadline and comment state that composition explicitly.
The inner limit, cleanup order, panic assertion, error report, and final thread join remain unchanged.
This closes C7 within the package's F39 scope.
F39 remains OPEN only for #163's separate later merge delta.

### Completed evidence and description

The reviewer checked the completed exact-head focused Mac log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-47ae53a7-pool-20261008-204957-86837.log`.
It names this exact head and base `9ea0c9c22d0d0595a166becbba7f9e8872247c22`.
Slow clippy with -D warnings and worker prebuild pass.
The first nextest command passes 168 tests with zero skips in 1.314 seconds.
The corrected panic-cleanup test passes in 0.035 seconds.
The core guard selection passes 15 tests and skips nine other tests in 0.099 seconds.
The job exits 0 after ten seconds on Mac.
The description names all three derived outer allowances and this exact-head log.
It preserves the corrected report exception, scoped wait claims, and Prior art note.
The integration reviewer reports CLEAN at this head in verdict commit `2b7420f683d80b20ce7d9c5190acfd7799f01252`.
The combined #162 Linux landing gate remains required.

### Verdict and scope

PR #165 has no open package finding, including LOW, at this exact head.
Round 89's source closures and scope limits remain valid after this delta.
PR #163 separately retains F28, F33, and its later F39 merge duty.
This CLEAN does not clear #163, M2a, M2b, the A32/A33 follow-up, or pending conformance IDs.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved.

VERDICT: CLEAN


## Round 91 — Deadline marker placement

Reviewed head: `29b37890efeffa4410d7dfc34f5c2344bfad1ba4`, PR #165, branch `stage1/p3-guard-macos`.
Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `47ae53a7..29b37890`, one commit, one file, one comment moved.
The implementer reports that the combined #162 Linux gate failed the timer scan.

The deadline marker now sits directly above writer.recv_timeout(CLEANUP) inside the method chain in guard_cleanup.rs.
The reviewer checked the timer scanner's rule: the marker must be on the call's line or directly above it.
The new placement satisfies that rule after formatting.
The delta changes no statement, timeout value, expected value, process ownership, or cleanup order.
Round 90's source closures remain valid.

The reviewer read the completed exact-head Linux log:
`~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-29b37890-pool-20261008-210034-20788.log`.
The log names this exact head and base `9ea0c9c22d0d0595a166becbba7f9e8872247c22`.
Formatting, taint, the timer scan of 111 Rust files, and lists pass.
The job exits 0 after 18 seconds on the Linux node msa1.
This is static-check evidence, not a full landing gate or a real-process test result.
The combined #162 landing gate remains required on its new reviewed head.

PR #165 has no open package finding, including LOW, at this exact head.
PR #163 separately retains F28, F33, and its later F39 merge duty.
This CLEAN does not clear #163, M2a, M2b, the A32/A33 follow-up, or pending conformance IDs.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved.

VERDICT: CLEAN


## Round 92 — Audit fixes Part A after the split

Reviewed head: `c95b4d7646fb5388ce6421bcc4baa708c73685e9`, PR #167, branch `stage1/p3-audit-fixes-a`.
Base: `a22811b61cd52aa503dc57e66b47c3d3bbd4746d`.
Prior audit head: `40b63dceb6e3f3d7be69a1488ac77eccc121a071`, PR #163.
The lead split the audit fixes after assigning all new real-process test code to P6.
Part A carries A3, A30, A53, and the F33 slow-tier selection.
The reviewer checked the whole retained change and the split dependencies against BUILD.md and the lead's rulings.

### Retained source and split scope

The host, binding, testkit, worker-core, and xtask changes match the earlier reviewed audit source.
The new head changes only the Drain module documentation from Part A's initial head `d823a40e`.
Cargo.lock retains the dependencies from the merged guard change and adds the host's workspace binding dependency.
Payload retains v1's exit watch. Its sole Part A change makes reap consume self through drop(self).
A53's single report path remains valid: launch requires the host hello, and the first drain reports the exit once.
The reap no-op exclusion names one function and states the concrete equivalent-body reason.
The other changed exclusion entries remove obsolete exceptions; they add no broad exclusion.

A3 uses libghostty for semantic write sizes. The host computes the size once and retains it for IN-5 and IN-7.
The text-report states run first, with the reviewed early refusal bound and the explicit cost note.
The associated-text bound follow-up remains separate, as the lead ruled.
A30 gives the testkit a bounded drain and sends PtyDrained only for an asked drain.
The shared Drain decision includes one flushing read and one count after that read.
The real driver still has the open A31 count-only defect. The module documentation and PR description state this correctly.
The approved split does not close A31 or prove parity with the real driver.
No guard file or real-process fixture changes in Part A.
F28 native mutation evidence, F33 failed-watch execution, and F39's merge duty remain in Part B.

### F49 — LOW — OPEN: the test still claims parity with the real driver

Evidence: `crates/botster-core-testkit/src/worker/tests.rs:77-80` at the reviewed head.
The comment says, "The drain is the real driver's (`Drain`)".
The test name is `the_edge_drains_as_the_real_driver_does`.
The real driver does not use Drain at this head, and the new module documentation explicitly records that difference.
The test proves the bounded testkit drain, including its flushing read and its later-byte bound.
It does not prove behavior that the real driver currently has.
Rename the test for the bounded testkit drain and correct its comment.
Describe the testkit readiness path directly in worker.rs rather than claiming parity through its "As the real driver" comment.
This is the remaining local form of integration D1's inaccurate parity claim.
The reviewer sent F49 directly to the P3 implementer.

### Completed evidence

The reviewer read the completed focused Linux log at `d823a40ea57f5622dfcab8e6198837dca13a3557`:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-d823a40e-pool-20261008-211218-39117.log`.
It names base `a22811b61cd52aa503dc57e66b47c3d3bbd4746d`.
Formatting, clippy, taint, lists, public-api, worker prebuild, and test-budget pass.
The default tier passes 757 tests and skips 654 tests; its wall time is 1.2 seconds.
The slow-feature clippy command passes with -D warnings. The job exits 0 after 200 seconds on msa1.
The reviewer also read the exact-head static log:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-c95b4d76-pool-20261008-211852-47753.log`.
Formatting, taint, timers for 112 Rust files, and lists pass. The job exits 0 after nine seconds.
The latest delta changes only a module comment, so the earlier executable evidence remains applicable to this review.
These focused jobs are not the landing gate and do not supply Part B's required evidence.

### Verdict and limits

PR #167 is NOT CLEAN for F49 LOW at this exact head.
No other package finding is open within Part A's submitted scope.
Part B, M2a, M2b, the A32/A33 follow-up, and pending conformance IDs remain outside this verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved.

VERDICT: NOT CLEAN


## Round 93 — Part A drain claims corrected

Reviewed head: `4592ba9c4e5657ef0d5856b6b55af40f2471c9bb`, PR #167, branch `stage1/p3-audit-fixes-a`.
Source base: `a22811b61cd52aa503dc57e66b47c3d3bbd4746d`.
Reviewed delta: `c95b4d76..4592ba9c`, one commit, two files, comments and one test name.
Round 92 records the whole Part A source review and the approved split scope.

### F49 — LOW — CLOSED at 4592ba9c

The test name is now `the_edge_drain_is_bounded_by_the_asked_count`.
Its comment describes the bounded testkit drain and its one flushing read, without claiming parity with the real driver.
The drain field comment and the readiness comment also describe the testkit path directly.
The remaining real-driver comparisons concern link setup and queued link writes. Neither comparison claims drain parity.
The delta changes no statement, assertion, expected value, mutation exclusion, or process ownership.
The Drain module documentation still states that A31 remains open in the real driver.

### Evidence and description

The reviewer read the completed exact-head Linux log:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-4592ba9c-pool-20261008-212212-55802.log`.
Its job base is `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`, distinct from the source base above.
Formatting, taint, the timer scan of 112 Rust files, lists, and clippy pass.
The renamed test passes. The nextest selection runs one test and skips 198 tests.
The job exits 0 after 16 seconds on msa1.
The earlier focused evidence at d823a40e remains applicable because the later deltas change only comments and one test name.
These jobs are focused evidence, not the landing gate.

The reviewer verified the current PR description and exact head through GitHub.
The description names the current log, its job base, and the earlier logs as history.
It distinguishes the source base from each job base and states F49's corrected scope.
It preserves the open A31 statement and the Part B obligations.

### Verdict and limits

PR #167 Part A has no open package finding, including LOW, at this exact head.
F33's selection source is included; its required failed-watch execution remains in Part B.
Part B retains F28 native mutation evidence, F33 execution, and F39's later merge duty.
A31 remains open after Part A. The lead keeps #161 parked until Part B.
This CLEAN does not clear Part B, M2a, M2b, the A32/A33 follow-up, or pending conformance IDs.
The cross-package integration review and the implementer's landing gate remain required.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 94 — Part A merge with v1

Reviewed head: `b70855356aa243996edabf8bfb3c9f6d2c2f5db6`, PR #167, branch `stage1/p3-audit-fixes-a`.
Previous CLEAN head: `4592ba9c4e5657ef0d5856b6b55af40f2471c9bb`.
Merged v1 head and current PR base: `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
The commit has exactly those two parents. The lead ordered Part A to land first.

The delta adds v1's guardian-core change from #142: 14 files and 2310 added lines.
All 19 Part A source and documentation files are byte-identical to the previous CLEAN head.
The two shared files are .cargo/mutants.toml and Cargo.lock.
Their changes add only the guardian's two exclusions and its package record. Part A's entries and dependency remain intact.
The two new exclusions name one guardian function and one guardian constant; neither exclusion matches a P3 function.
All 12 other upstream files are byte-identical to the merged v1 head.
The upstream xtask change adds two guardian fuzz harnesses in ci.rs; it does not alter the test-budget selection.
Guardian-core has no slow feature, slow_* module, or slow_* test target.
F33's slow-tier filter therefore selects no additional guardian tests.
Guardian-core does not depend on the host, payload edge, binding, worker-core, or testkit.
The merge adds no call into Part A's changed functions.
This review checks the merge's effect on P3. It does not replace #142's package or integration review.

The reviewer read the completed exact-head static log:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-b7085535-pool-20261008-213359-76310.log`.
The log names this exact head and job base `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Formatting, taint, the timer scan of 120 Rust files, and lists pass.
The job exits 0 after nine seconds on msa1.
These static checks are not the landing gate or new executable proof.
The reviewer verified the updated PR description. It names the merge head and log and labels earlier evidence as history.

PR #167 Part A has no open package finding, including LOW, at this exact head.
The prior source closures remain valid. A31 stays open in the real driver after Part A.
Part B retains F28 native mutation evidence, F33 required failed-watch execution, and F39's later merge duty.
This CLEAN does not clear Part B, M2a, M2b, the A32/A33 follow-up, or pending conformance IDs.
The cross-package integration review and the implementer's landing gate remain required at the submitted head.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 95 — Part A mutation fixes

Reviewed head: `723bc8d5c937d905a918a8063deedaaf5326b8c7`, PR #167, branch `stage1/p3-audit-fixes-a`.
Base: `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Previous CLEAN head: `b70855356aa243996edabf8bfb3c9f6d2c2f5db6`.
The initial READY named `7be3184c09dc4c77e46414cddcd15428ccd3aa6d`.
The later READY adds only exclusion comments for integration E1; the regexes and executable source are unchanged.

### Landing failure and new tests

The reviewer read the completed landing log:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-b7085535-pool-20261008-213644-81473.log`.
The default tier passes 784 tests; the slow tier passes 197 tests.
The mutation step tests 112 mutants: 82 caught, 17 missed, and 13 unviable.
The missed mutants affect Part A's Focus bound, key-state enumeration, key search stop, mouse notch normalization, and drain steps.
The job exits 1 after 181 seconds. The fuzz step does not run.
The implementer changes the source before another gate; the implementer does not rerun this red head.

The new drain test checks exact progress for short reads, full reads, and zero reads in both counted phases.
The native-encoder tests check refusal below a key's worst case and one-notch mouse bounds.
The host test now checks the observable Unknown bound for a Focus write as well as bytes and repeated keys.
These checks address the missed decisions without inventing terminal bytes or starting real processes.
The new private key-state test has the separate F50 defect below.

### Exclusion arguments

The payload_size entry excludes only `/` to `*` in one named function.
An admitted key has the same complete maximum under either search threshold.
A refused key remains over the final payload bound under either threshold.
Valid CoreLimits cap max_paste_bytes at 64 MiB and max_key_repeat at 4096, so the multiplied threshold cannot overflow u64.
The argument check refuses a zero repeat before payload_size divides by it.
The threshold can change search cost and the length quoted in the error detail.
Core 9.3 makes the code contractual and the detail human text; both paths retain PayloadTooLarge.
The reviewer accepts this exclusion within contract-valid configuration.

The every_key_state entry excludes only `!=` to `==` in one named function.
It complements the six boolean key modes and permutes the same 64 combinations.
It preserves every kitty-flag combination and the text-first order within each run of 32.
The full maximum remains the same, and an early result remains above the supplied limit.
A different early length can change only the host refusal's detail under Core 9.3.
The reviewer accepts the equivalence argument. Its proving-test reference must change when F50 is corrected.
The integration E1 comments make the code/detail distinction explicit and change no exclusion pattern.

### F50 — MEDIUM — OPEN: the new test checks a private helper's internals

Evidence: `crates/botster-terminal-ghostty/src/encode.rs:819-843` at the reviewed head.
`the_key_states_are_every_combination_once_with_the_text_states_first` calls private EncoderState::every_key_state.
It checks internal tuple count, uniqueness, bit range, and index layout.
BUILD.md testing rule 3 says, "none tests a helper's internals."
Core 5.1A requires the safe worst-case bound over native encoder modes.
It does not require this private representation or exactly one visit per tuple.
Replace this test with native-encoder behavior coverage through longest_key_sequence and the independent explicit-mode path.
If a missed state-transform mutant cannot change the bound, document a narrow equivalence argument instead.
Update the exclusion's proving-test reference and the PR description after this correction.
The reviewer sent F50 directly to the P3 implementer and copied the integration reviewer.

### Completed focused evidence

The reviewer read the completed focused log at `7be3184c09dc4c77e46414cddcd15428ccd3aa6d`:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-7be3184c-pool-20261008-214331-91507.log`.
Formatting, clippy, taint, lists, test-budget, and mutants pass.
The default tier passes 788 tests and skips 654 tests; its wall time is 1.1 seconds.
The mutation step tests 110 mutants: 97 caught, 13 unviable, zero missed, and zero timeouts.
The job exits 0 after 174 seconds on msa1.

The exact-head static log is:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-723bc8d5-pool-20261008-214836-98013.log`.
Formatting, taint, timers for 120 Rust files, and lists pass. The job exits 0 after nine seconds.
Both logs name base `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
The later delta changes comments only, so the 7be3184c executable evidence remains applicable to this review.
These focused jobs do not supply a green landing gate or remove F50's test-design defect.
The reviewer requested that the description name the superseding head and its static log.

### Verdict and limits

PR #167 Part A is NOT CLEAN for F50 MEDIUM at this exact head.
The prior source closures and F49 closure remain valid. No other package finding is open within Part A's submitted scope.
A31 remains open in the real driver after Part A.
Part B retains F28 native evidence, F33 failed-watch execution, and F39's later merge duty.
M2a, M2b, the A32/A33 follow-up, and pending conformance IDs remain outside this verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 96 — F50 correction and native Mac equivalence counterexample

Reviewed head: `f1cee83415af0962b83ccadc45b29e006120e443`, PR #167, branch `stage1/p3-audit-fixes-a`.
Base: `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Reviewed delta: `723bc8d5..f1cee834`, one commit, three files.
The delta removes the private helper test, adds a native-encoder behavior test, and adds two exclusion patterns.
The implementer asks whether the 1960-input exploration log proves the mode-bit exclusion.

### F50 — MEDIUM — CLOSED at f1cee834

The helper-internal key-state test is removed from encode.rs.
The new `bits_above_the_five_kitty_flags_change_no_key_bytes` test calls the public explicit-mode encoder.
It compares native results with and without high flag bits across the independent mode enumeration.
The input cases include associated text, a function key, and a keypad key with NumLock.
It checks native encoding behavior rather than private tuple count or layout.
The removed test's clauses remain covered by the independent worst-case tests and the recorded source review.
The separate invalid exclusion is F51 below.

### F51 — MEDIUM — OPEN: the mode-bit exclusion loses a required Mac mode

Evidence: `.cargo/mutants.toml`, the new pattern for `EncoderState::every_key_state` at column 40.
The pattern excludes `&` to `|` and `&` to `^` in the mode-bit closure.
The comment claims that every key reaches its worst case with all six modes on.
The finite Linux input survey does not prove that universal claim, and the claim is false on macOS.

The reviewer inspected the pinned libghostty source at `3f8eb6810bb673aa782b047de21783ac81fb1121`.
The relevant source is src/input/key_encode.zig: legacy, legacyAltPrefix, and kitty's associated-text decision.
The binding's KeyEncoder::configure sets OptionAsAlt to true on every platform.
Use this valid input: Key::Char("a"), modifiers [Alt], text containing 64 copies of "a", Press, and no alternate keys.
With kitty disabled and alt_esc_prefix false, native legacy encoding writes the complete 64-byte text.
With all six modes on, the Mac legacyAltPrefix branch uses the unshifted codepoint instead of the supplied multi-byte text.
That branch writes the escape prefix and the single base character.
The kitty branch cannot supply the omitted long result: Alt prevents associated text when OptionAsAlt is true.
Its sequence for this input remains shorter than 64 bytes, for every kitty-flag combination.

The `|` mutant sets every boolean mode true.
The `^` mutant can make a mode false only when the index equals that mode's single-bit mask.
At those indices, the low kitty bits are zero before the text-bit inversion, so kitty flag 16 is enabled.
Thus both mutants retain only the all-on mode combination for the disabled-kitty legacy path.
Both mutants omit the Mac mode that writes all 64 text bytes, so their computed worst-case bound is too small.
This can change the host's retained-byte accounting and Unknown bound.
The unchanged production enumeration includes the required mode; this finding concerns the exclusion and missing regression proof.

Remove the column-40 exclusion.
Add a behavior test that compares the public bound with the independent native encoder for this valid long-text case.
Keep the native library as the oracle; do not invent terminal sequences in the test.
Provide completed Mac evidence that catches both excluded mutants.
Correct the exclusion argument and the PR description's universal claim.
The reviewer sent F51 directly to the implementer and copied the integration reviewer.

### Other exclusion arguments

The retained `!=` to `==` exclusion still permutes the same six-mode combinations and preserves the full maximum.
The new kitty-flag `&` to `^` exclusion at column 36 is equivalent at the current pin.
The reviewer inspected src/terminal/c/key_encode.zig: setoptTyped truncates the supplied flags to u5 before storing them.
The mutant permutes the same five-bit combinations; the C API discards all higher bits.
The new native behavior test supports that argument.
The prior payload_size threshold exclusion remains accepted within contract-valid limits.

### Completed evidence and its limits

The reviewer read the scratch exploration log:
`~/botster-sessions/gates/botster-core-scratch-p3-f50-explore-f3611957-pool-20261008-215141-2770.log`.
The reviewer also read the scratch source at `f3611957b7f17e75053cda9a3c7e223a02c7c1e4`, without running it.
The release-mode Linux command reports 1960 inputs, zero shorter all-on maxima, and zero differing high-bit results.
The survey uses eight modifier sets and five character scalars. It supplies no explicit associated text or alternate keys.
It does not cover the Mac branch or the long-text counterexample above.
The scratch job exits 0 after 54 seconds. It is exploration, not proof of universal equivalence.

The exact-head focused log is:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-f1cee834-pool-20261008-215400-6319.log`.
It names this exact head and base `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Formatting, clippy, taint, lists, test-budget, and mutants pass.
The default tier passes 788 tests and skips 654 tests; its wall time is 1.1 seconds.
The mutation step tests 107 mutants: 94 caught, 13 unviable, zero missed, and zero timeouts.
The job exits 0 after 160 seconds on msa1.
The disputed mutants are excluded, and Linux does not compile the relevant Mac branch.
This green focused job therefore does not close F51 or supply a green landing gate.
The PR description names the current head and evidence but repeats F51's incorrect all-on claim.

### Verdict and limits

PR #167 Part A is NOT CLEAN for F51 MEDIUM at this exact head.
F50 and F49 are CLOSED. No other package finding is open within Part A's submitted scope.
A31 remains open in the real driver after Part A.
Part B retains F28 native evidence, F33 failed-watch execution, and F39's later merge duty.
M2a, M2b, the A32/A33 follow-up, and pending conformance IDs remain outside this verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 97 — F51 regression and unenforced platform scope

Reviewed head: `3d5237b2b1650424f6f23a27c841407014a165aa`, PR #167, branch `stage1/p3-audit-fixes-a`.
Base: `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Reviewed delta: `f1cee834..3d5237b2`, two commits, two files.
The delta adds F51's native behavior test and revises the exclusion comments.
The final head restores the same column-40 exclusion pattern.

### F51 — MEDIUM — Regression and Mac catches accepted; platform scope remains OPEN

The new test is `the_key_bound_of_an_alt_key_with_long_text_covers_the_states_with_modes_off`.
It uses the valid Char("a"), Alt, 64-byte text input from round 96.
It compares the public key bound with the maximum native result over the independent explicit-mode enumeration.
It requires the bound to cover the supplied text. It invents no terminal sequence and tests no private helper layout.
The test is valid behavior coverage for F51.
The Mac focused run catches both column-40 mutants at this exact head.
The Linux focused run reports both mutants MISSED, consistent with the platform difference in legacyAltPrefix.
The Mac regression and required catches close F51's behavior/proof portion.

The final .cargo/mutants.toml entry remains:
`crates/botster-terminal-ghostty/src/encode\.rs:\d+:40: replace & with [|^] in EncoderState::every_key_state`.
The comment says, "This entry is for the Linux gate only."
The configuration has no platform condition, and xtask::mutants_job supplies no platform-specific configuration.
Cargo-mutants therefore excludes both mutants during a configured Mac gate as well as a configured Linux gate.
A comment does not enforce the proposed Linux-only scope.
The entry still excludes mutations known to change Mac behavior under the default configuration.

Enforce the exclusion's platform scope mechanically so a Mac gate includes both mutants.
Keep the named Mac proof required when this function or its native dependency changes.
Give the Linux equivalence a pinned-source argument that covers all valid inputs.
The Linux Alt branch explains the new long-text case; it does not alone prove the maximum under the other five key modes.
A finite survey cannot establish the universal equivalence claim.
Update the configuration comments and PR description to match the enforced scope and its proof.
The reviewer sent these remaining F51 requirements directly to the implementer.
The other three accepted equivalence arguments remain unchanged.

### Completed evidence

The reviewer read the exact-head Mac log:
`~/.local/state/jobq/logs/jobq-botster-core-3d5237b2-20261008221247-efb1.log`.
Its command uses cargo mutants --no-config, selects every_key_state, and runs with two build jobs.
Both column-40 mutants are caught. The full diagnostic result is 19 tested: 14 caught, two missed, two unviable, one timeout.
The two misses are the accepted !=/== permutation and kitty-flag XOR equivalences.
The timeout is `695:36: replace + with *`, which expands the state search to 2^30 states under cargo test.
The diagnostic job exits 3 after 109 seconds, so it is not a green mutation job.
The per-mutant catches supply the specific F51 Mac proof; they do not supply a green landing gate.

The reviewer read the exact-head Linux log:
`~/.local/state/jobq/logs/jobq-botster-core-3d5237b2-20261008221445-9d9c.log`.
It uses the same --no-config selection and four build jobs.
It reports 19 tested: 12 caught, four missed, two unviable, one timeout.
The four misses include both column-40 mutants and the two accepted equivalences above.
The same expanded-search mutant times out. The job exits 3 after 77 seconds.
Both diagnostic commands use cargo test; the configured gate uses nextest with the default two-second per-test deadline.
The earlier configured f1cee834 job reports no timeouts. The diagnostics do not prove a new production timeout defect.

The exact-head static log is:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-3d5237b2-pool-20261008-221607-45188.log`.
Formatting, taint, timers for 120 Rust files, lists, and clippy pass. It exits 0 after ten seconds on msa1.
The static log names this exact head and base `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
The PR description names the current head, both native diagnostic results, and their known timeout.
It still claims a Linux-only configuration scope that the code does not enforce.

### Verdict and limits

PR #167 Part A is NOT CLEAN for the remaining F51 MEDIUM requirements at this exact head.
F50 and F49 remain CLOSED. No other package finding is open within Part A's submitted scope.
A31 remains open in the real driver after Part A.
Part B retains F28 native evidence, F33 failed-watch execution, and F39's later merge duty.
M2a, M2b, the A32/A33 follow-up, and pending conformance IDs remain outside this verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 98 — F51 platform scope and the mutation-result exclusion

Reviewed head: `9ddbdf897e33aa4fa2c742c20670588bfdabddad`, PR #167, branch `stage1/p3-audit-fixes-a`.
Base: `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Reviewed delta: `3d5237b2..9ddbdf89`, three commits, three files.
The reviewer checked the changes against the previous Part A review, BUILD.md, and the lead's anchor rulings.
The reviewed guard remains the version that landed through #162. This delta changes no process fixture.

### F51 — CLOSED — The platform condition preserves the Mac proof

The global configuration no longer excludes the column-40 mode-bit mutations.
`platform_exclusions(std::env::consts::OS)` returns no additional exclusion on macOS.
On other platforms, it returns the exact-function column-40 pattern.
`mutants_job` adds that pattern before the `--` separator, as a cargo-mutants option.
The named default test proves that Mac selection adds none and Linux selection adds the recorded exception.
The pure selection function remains mutation-tested.
The configured Mac run at `aeda1caca46b6fdd5215ea977615dbf53dd5718f` catches both column-40 mutations.
The final head changes only the comment that names this proof. The binding and regression test are identical.

After the implementer's question, the reviewer accepted a native platform coverage exception with this named Mac proof.
The reviewer withdraws round 97's universal Linux equivalence-proof requirement.
This exception records the native Mac branch, rather than a universal equivalence claim.
The comment requires a new focused Mac proof when the enumeration, key encoding, or Ghostty pin changes.
The native long-text regression from round 97 remains valid behavior coverage.
F51 is CLOSED at this exact head. F49 and F50 remain CLOSED.

### Expanded search — Compile-time bound accepted

`KEY_STATES` computes the same 64 mode combinations times 32 kitty-flag combinations.
The all-build constant assertion requires exactly 2048 states.
The iterator uses this constant without changing the admitted states or their order.
Arithmetic mutations that expand the search fail constant evaluation instead of reaching the test timeout.
The current diagnostic removes the nextest termination deadline and reports no mutant timeout.
This change resolves the expanded-search diagnostic from round 97.

### F52 — MEDIUM — The new exclusion hides the mutation-result decision

The new `.cargo/mutants.toml` entry excludes all mutations in `xtask::mutants_job`.
Its concrete process-glue reason applies to starting git and cargo-mutants, writing the diff, and printing the summary.
The function also decides whether a failed mutation command rejects the CI step at `xtask/src/ci.rs:336-337`.
That decision is not in either cited pure function, `parse_outcomes` or `platform_exclusions`.
The cited tests do not prove that missed mutants, timeouts, or baseline failures reject the step.
A `mutants_job -> Ok(())` mutation skips the required mutation step and returns success.
Running the successful unmutated step cannot prove rejection of a failed step.
The claim that its decisions are pure functions with unit tests is inaccurate at this head.

Keep the exact-function exception for process glue.
Move the mutation-result decision to an in-process function, or use an existing tested status function.
Use that function from `mutants_job`.
Default tests must prove that a successful result passes and failed results reject the step.
Keep this decision mutation-tested. Update the exception comment and PR description.
This requires no process fixture or nested mutation run.
Authority: BUILD.md's required mutation step and the existing xtask policy that excludes process glue alone.
The reviewer sent F52 directly to the implementer and integration reviewer.

### Completed evidence

The configured Mac proof log is:
`~/.local/state/jobq/logs/jobq-botster-core-aeda1cac-20261008222501-7480.log`.
It names exact head `aeda1caca46b6fdd5215ea977615dbf53dd5718f` and base `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Its command selects `every_key_state` with the real configuration, without `--no-config`.
The log reports 14 tested: 12 caught, two unviable, zero missed or timed out.
Both column-40 mutants are caught. The job exits 0 after 87 seconds.
The final head changes only its proof comment, so this native proof applies to the current binding and test.

The exact-head Linux diagnostic log is:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-9ddbdf89-pool-20261008-222643-62154.log`.
It names the current head and base. Formatting, taint, timers for 120 Rust files, lists, and clippy pass.
The mutation command uses `NEXTEST_PROFILE=slow`, which removes nextest's per-test termination deadline for this diagnostic.
It reports 111 tested: 95 caught, 16 unviable, zero missed or timed out.
The mutation step passes in 148.5 seconds. The job exits 0 after 158 seconds.
The PR description records the platform exception, Mac proof, compile-time bound, and current diagnostic evidence.
It also records the new process-glue exception, whose decision coverage F52 challenges.
These focused jobs do not replace the required landing gate after both exact-head reviews are CLEAN.

### Verdict and limits

PR #167 Part A is NOT CLEAN for F52 MEDIUM at this exact head.
F51, F50, and F49 are CLOSED. No other package finding is open within Part A's submitted scope.
A31 remains open in the real driver after Part A.
Part B retains F28 native evidence, F33 failed-watch execution, and F39's later merge duty.
M2a, M2b, the A32/A33 follow-up, and pending conformance IDs remain outside this verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 99 — F52 mutation-result decision

Reviewed head: `41e997446090afb7ac45ae9823734dfeea7eb40b`, PR #167, branch `stage1/p3-audit-fixes-a`.
Base: `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Reviewed delta: `9ddbdf89..41e99744`, one commit, two files.
The reviewer checked the delta against round 98, BUILD.md, and the lead's anchor rulings.
The delta changes xtask's result decision, its default test, and its exclusion entry.
It changes no product code, guard, or real-process fixture.
The binding and native regression files match the configured Mac proof at `aeda1cac` exactly.

### F52 — CLOSED — Failed mutation runs reject the step through the tested decision

`mutation_verdict(code: Option<i32>)` passes only `Some(0)`.
Every other code and `None` return an error. `None` represents a child that ended by a signal.
`mutants_job` calls this same function with `status.code()` and propagates its error.
The default test `only_a_mutation_run_with_every_mutant_caught_passes` proves success and failure results.
It checks success at zero and rejection at codes 1, 2, 3, and 4, plus a signal result.
This test proves the required CI result decision. It does not assert a private data layout.
The function remains mutation-tested.
The error text reports the code and identifies missed mutants, timeouts, and baseline failures.

The exclusion now matches only `replace mutants_job -> Result<()> with Ok(())`.
Every other generated mutation in `mutants_job` remains eligible.
The whole-body replacement removes the process operations and the result summary together.
Default tests cannot start a nested mutation run.
The concrete exception names the tested result, summary, and platform decisions.
The completed job also shows the required summary and real process execution.
The reviewer accepts this exact-function, exact-replacement exception for process glue.
F52 is CLOSED at this head.

### Completed evidence

The exact-head Linux log is:
`~/botster-sessions/gates/botster-core-stage1-p3-audit-fixes-a-41e99744-pool-20261008-223521-78736.log`.
It names this head and base `0b0eecc06d0cd4c39af5e33e59c0b3f643aba6d4`.
Formatting, taint, lists, and clippy pass.
The mutation diagnostic uses `NEXTEST_PROFILE=slow`, without nextest's per-test termination deadline.
It reports 113 mutants: 97 caught, 16 unviable, zero missed or timed out.
The mutation step passes in 155.3 seconds. The job exits 0 after 165 seconds.
The reviewer read the full PR description. It names the exact head and matches the submitted code and evidence.
The earlier configured Mac proof remains applicable because the binding and native regression files are unchanged.
Round 98 records that proof: 14 tested, 12 caught, two unviable, including both column-40 catches.
The required full landing gate remains the implementer's next step after both exact-head reviews are CLEAN.

### Verdict and limits

PR #167 Part A is CLEAN at this exact head. F49 through F52 are CLOSED.
No package finding remains open within Part A's submitted scope.
The previous whole-change reviews and their unchanged code remain applicable.
A31 remains open in the real driver after Part A under the lead's approved split.
Part B retains F28 native evidence, F33 failed-watch execution, and F39's later merge duty.
The guard remains the reviewed version that landed through #162. Part A changes no fixture wait.
M2a, M2b, the A32/A33 follow-up, and pending conformance IDs remain outside this verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 100 — M2a restack, input admission, and required real-PTY proof

Reviewed head: `1c3154cfe695282d2c8acab4d45d5bc3b769ffed`, PR #168, branch `stage1/p3-m2a-v1`.
Reviewed base: `d1d18f4aeba717d2dec2708024c77f55e3ee35ef`, v1 after #167.
The submitted range has eleven commits and changes eighteen files.
The reviewer checked the full M2a change, its restack, and the completed gate evidence.
Authority: BUILD.md, the pinned Stage 1 plan and contract, and the lead's M2a ruling of 2026-10-08.
The PR is a draft. Its current GitHub base has advanced to `ee7dd16c73a6991b6ef9b3a85d84fd93e57a3230`.
This verdict covers only the named submitted head. A later v1 merge requires a delta review.

### Logic review — Complete

The worker retains one active host transaction and a FIFO for queued host writes.
It checks both guards when the transaction starts, before advancing the host input revision.
It retains transaction ownership across short writes and waits for the in-flight count before honoring a cancel.
It reports exact counts for completion, cancellation, failure, and the payload's end.
Queued cancels complete with zero counts. Later transactions start after the active transaction ends.
The current input module matches round 11's reviewed module except the detail-only refusal table removal described below.
The new tests cover the kill refusal, client revision guard, and writable input while a write is in flight.
The terminal model and route input remain later milestones, as the M2a scope declares.

The real driver retains a pending `PtyWrite` across turns.
The loop processes control work before one PTY write attempt.
An interrupted attempt retains the bytes for the next turn. A blocked attempt waits for write readiness.
`io_decisions::pty_write` determines retry, blocked, and error results in the default tier.
`io_decisions::pty_write_interest` determines whether poll registration must change.
The driver uses these same decisions for the real OS operations.
This preserves F10's turn bound and permits control, signal, exit, and timer progress between fragments.
The two new per-function OS glue exclusions name their required real-PTY proof.
The reviewer accepts their classification. Their required proof remains the HOLD below.

The testkit uses the same Worker machine with the scripted program edge.
Its `PtyWrite` and `PtyWritable` inputs participate in the seeded schedule.
Its program and process controls use the handle's data directory and instance, preserving F9's namespace correction.
The control registry receives the worker module's registrations through the established module interface.
The restack retains v1's `Drain` in the testkit and `unread` in the program edge.
The real driver retains v1's count-only drain under the approved A31 split.
No new process fixture appears in this branch.
The transcript proof list keeps its IDs pending until the required real harness proof exists.

### Two removals — Accepted within the submitted scope

The M2b payload refusal still returns `Internal`.
Its new human-readable detail omits the table of payload names.
Core 9.3 makes the code contractual and the detail human-readable, so this removal changes no required result.
M2b must still implement the declared semantic payload kinds through libghostty.

The testkit link's removed `WouldBlock` arm covered descriptor-only readiness.
`End::readiness` treats a queued descriptor as readable, but a byte read of an empty open queue returns `WouldBlock`.
Ordinary M2a byte readiness comes from queued bytes, reset, or peer closure; those paths retain their existing results.
The old arm did not consume the descriptor, so it did not implement descriptor handoff.
The PR records this as the existing P4a edge gap. M2a adds no descriptor path or route proof.
The reviewer accepts removal of this incomplete path within M2a's stated scope.
This acceptance does not close P4a's descriptor handoff duty.

### F53 — LOW — Spent chunk and accept limits request an idle pump

Evidence: `crates/botster-core-testkit/src/program.rs:466-469` and `src/worker.rs:216-220,680-685`.
`ScriptedProgram::write` sets `step_refused` when the chunk cap is spent, even when the accept limit is also spent.
Example: set `pty_chunk` to 2, set `pty_accept` to 2, and begin a write of `abc`.
The first attempt takes `ab`. The retry finds both limits at zero and returns `WouldBlock`.
It sets `step_refused` to true. `Workers::has_ready` then requests another pump and signals the wake.
The next step restores the chunk cap but leaves `accept == Some(0)`, so the write cannot resume.
The requested pump has no write progress to perform.
This repeats F11's extra idle pump pattern.
The new test covers a blocked write with a spent chunk and an accept refusal without a chunk.
It does not cover both limits spent together.

Request a next step only when that step can remove the refusal that prevents write progress.
Cover the combined chunk and accept limits with behavior or readiness assertions.
Retain readiness for a positive spent chunk when no other condition prevents progress.
Authority: TM-6, plan 2.5's readiness rule, and F11's preserved closure requirement.
The reviewer sent F53 directly to the implementer and integration reviewer.
This correction needs no real-process test.

### HOLD — Required real-PTY regression

The lead confirmed that CLEAN means merge-ready. A development-only CLEAN is not permitted.
The exact required HOLD is:
"the real-PTY regression in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write, rebuilt on botster-test-process, must pass the gate".
This is the lead's ruling of 2026-10-08, confirmed directly during this round.
The existing patch is parked outside this branch. The completed slow tier does not contain that regression.
The gate therefore does not prove the new real PTY write path's cancellation, counts, and resumption.
The reviewer will review the rebuilt test delta and its completed evidence before CLEAN.
P6 owns the new real-process test code and its mechanical check.
The logic review is complete. F53 and this HOLD are the only open M2a items in this verdict.

### Completed evidence and prior art

The exact-head full Linux gate log is:
`~/botster-sessions/gates/botster-core-stage1-p3-m2a-v1-1c3154cf-pool-20261008-230738-28177.log`.
It names this head and reviewed base `d1d18f4a`.
Formatting, clippy, taint, timers for 122 Rust files, lists, public API, and worker prebuild pass.
The default tier passes 820 tests. The slow tier passes 197 tests.
Mutation results: 151 tested, 131 caught, 20 unviable, zero missed or timed out.
The fuzz step passes because the diff changes no crate with a decoder harness.
The full gate exits 0 after 292 seconds. Its summary reports 286.1 seconds of CI steps.

The separate no-termination diagnostic log is:
`~/botster-sessions/gates/botster-core-stage1-p3-m2a-v1-1923f9d9-pool-20261008-230219-19641.log`.
It records tracked changes as probe commit `70fb7e878518afb6c06f067e8e63ca8c9e748e3d`.
The reviewer verified that this probe and the submitted head have identical tree `0cd8ac7762932a40d10244b9ee0d34dd027ecf47`.
The diagnostic uses `NEXTEST_PROFILE=slow` and reports the same 151 results, with zero missed or timed out.
It exits 0 after 299 seconds. It is diagnostic evidence, not a merge gate.
The evidence does not close F53 or the missing real-PTY proof.

The reviewer read the full PR description and its prior-art record.
It records tmux, zellij, wezterm, shpool, abduco, dtach, and the old Core mechanism.
It rejects their input mechanisms for the stated topology, count, scheduling, and libghostty requirements.
It records no reused code and explains why the contract-specific admission decision is implemented here.
The new dependency is the existing workspace route codec, which supplies `HexBytes`.
The PR names the required real-PTY test and explicitly forbids merge before its rebuilt proof passes.

### Verdict and limits

PR #168 M2a is NOT CLEAN at this exact head for F53 LOW and the required real-PTY proof HOLD.
The logic review is complete. No other M2a logic finding remains open.
PR #167 Part A's round 99 CLEAN remains preserved at its exact head.
Part B retains its previous findings and A31 scope. This verdict does not clear those duties.
M2b, P4a, the A32/A33 follow-up, and pending conformance IDs remain outside this verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 101 — F53 closure, N1/N2 changes, and the v1 merge

Reviewed head: `7550018fff91ed948dbdfb7eea14caec48b27d6c`, PR #168, branch `stage1/p3-m2a-v1`.
Base: `ee7dd16c73a6991b6ef9b3a85d84fd93e57a3230`, v1 after #164.
Reviewed delta: merge `33a2bc877e01ff19d100a030c5c58f1b87b7cb60`, then fix `7550018f`, after round 100's head `1c3154cf`.
Authority: plan revision 22, pin `~/botster-sessions/pins/stage1-plan.71a623ef.md`, BUILD.md, and the lead's M2a ruling.
The reviewer verified the plan SHA256: `71a623ef93f487754e400dc186429216357e39fe251933675cc7f851843d519d`.
The corrected plan retains F51's platform coverage exception and requires no nextest termination during mutation evidence.
It keeps the real-process proof requirement and P6's test infrastructure ownership.

### F53 — CLOSED — An exhausted accept limit requests no next step

The next-step condition now also requires `controls.accept != Some(0)`.
A spent chunk can request another step only when the program is not blocked and its accept limit permits progress.
The combined-limit regression sets both limits to 2 and writes `abc`.
The first attempt takes two bytes. The retry returns `WouldBlock` and requests no next step.
The existing cases retain readiness for a positive spent chunk and suppress readiness for a blocked write.
The test asserts write and readiness behavior. It does not assert a private data layout.
The exact-head default tier selects this test and passes it.
The reviewer closes F53 at this head. F11's preserved readiness requirement remains in force.

### N1/N2 changes — Accepted in the package review

`io_decisions::pty_writable` contains the writable-event decision that M2a added inside excluded `Driver::run`.
The real driver calls this same function before clearing write interest and delivering `PtyWritable`.
Its default test checks all four combinations of write wait and event writability.
The pure decision remains mutation-tested. The exact-head default tier selects and passes its test.
The exclusion comments distinguish this decision from the real PTY event and write operations.
They name the parked regression for the PTY arm, the action dispatch, and the write-interest operation.
The bitwise XOR of disjoint READABLE and WRITABLE flags remains equivalent.
The bitwise AND removes interest and remains part of the required real-process proof.
The cited regression still does not exist on the branch. That evidence remains the single HOLD below.

The worker-core route-codec dependency moves to `dev-dependencies`.
The dependency supplies test construction only and adds no production dependency.
The reviewer accepts the N1 source correction and N2 dependency correction.
The integration reviewer retains responsibility for its own N1/N2 verdict.
No new package logic finding exists in these changes.

### Merge delta — Both process namespaces and upstream changes are preserved

The reviewer inspected the merge conflict resolutions and the final diff against both parents.
The old and new M2a diffs each change the same eighteen paths.
Six paths are shared with #164: the mutation configuration, lock file, testkit Core tests, harness, worker, and worker tests.
The merge registers each spawned worker in both v1's run process table and M2a's directory/instance map.
The tables retain their separate roles: identity probes and signals use the run table; controls use the session map.
The Core test resolution uses v1's `HostDriver::open` and M2a's handle/data-directory constructor arguments.
The reopen fixture supplies the same `reopen` directory to its Core, spawner, and identity probe.
The worker exit fixture retains v1's registered process table and M2a's directory field.
The final fix restores the #164 exclusion entries and comments that the intermediate merge had omitted.
The final configuration differs from v1 only by M2a's entries and proof comments.
No upstream-only file differs from v1 at the submitted head.
Eight M2a files remain byte-identical to round 100, including the admission module, machine, controls, and machine tests.
The reviewed guard and process fixtures receive no M2a change.
The merge has shared paths and conflicts, so it does not qualify for the plan's base-only script exception.
This round supplies the package delta review.

### Completed evidence

The exact-head Linux log is:
`~/botster-sessions/gates/botster-core-stage1-p3-m2a-v1-7550018f-pool-20261008-231933-52884.log`.
It names this head and base `ee7dd16c`.
The command runs the full `cargo xtask ci`, then the in-diff mutation step with `NEXTEST_PROFILE=slow`.
All ten CI steps pass, including formatting, taint/timers for 124 Rust files, lists, API, and prebuild.
The default tier passes 833 tests. The slow tier passes 206 tests.
The full gate's mutation step reports 156 tested: 136 caught, 20 unviable, zero missed or timed out.
The separate no-termination mutation step reports the same results at the same exact head.
The full CI summary reports 307.3 seconds. The no-termination step takes 273.8 seconds.
The combined job exits 0 after 594 seconds.
The fuzz step has no changed decoder harness.
The reviewer read the full updated PR description. It matches the code, merge resolutions, and completed evidence.
The PR retains its explicit merge prohibition until the rebuilt real-PTY regression passes.

### HOLD and verdict

The logic review is complete. No package logic finding remains open within M2a.
F53 is CLOSED. Exactly one item remains open:
HOLD: "the real-PTY regression in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write, rebuilt on botster-test-process, must pass the gate".
This is the lead's ruling of 2026-10-08. CLEAN means merge-ready; no development-only CLEAN is permitted.
The HOLD includes the missing selected proof for the Driver exclusions under plan section 8.
The current 206 slow tests do not contain the parked regression and cannot close this HOLD.
The reviewer will review the rebuilt regression delta and completed gate evidence before CLEAN.

PR #168 M2a is NOT CLEAN at this exact head for this single HOLD.
Part B retains its previous findings and A31 duty. M2b, P4a, and pending conformance IDs remain outside this verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN

## Round 102 — Guarded signal calls and their mechanical enforcement

Reviewed head: `9be8102708a33c1a4c79693e09797ab6942c7c3c`, PR #177, branch `stage1/p3-group-signal-guard`.
Base: `0b684a19470c5b647e64653fd285d4cd43544278`, v1 after #174.
Authority: BUILD.md, plan revision 22 at pin `~/botster-sessions/pins/stage1-plan.71a623ef.md`, and the lead's signal ruling of 2026-10-09.
The pin SHA256 remains `71a623ef93f487754e400dc186429216357e39fe251933675cc7f851843d519d`.
The lead requires one guarded group-signal function, raw-call bans, and the A10-2 corrupted-row behavior test.
The lead also requires each pattern fix to include the mechanical check that finds all its instances.
The reviewer inspected the full nineteen-file diff, the final source, the updated PR description, and completed gate evidence.

### Guard behavior — Accepted

`signal::target` refuses 0, 1, values above `i32::MAX`, and the supplied own identifier.
Both OS wrappers call this function before a raw signal call.
The group wrapper obtains its own group from `getpgrp`. The process wrapper obtains its own process identifier.
The wrappers retain OS errors. Conversion to `io::Error` retains the OS errno and reports a refusal as `InvalidInput`.
The reviewer checked the locked Rustix 1.1.5 source: group identifier 1 becomes `kill(-1, signal)`.
The new positive-range check prevents that operation and prevents signed-conversion errors.

All current raw `kill_process_group` and `kill_process` calls outside the guard move to these wrappers.
`Children::signal_group` retains its process identity check before dispatch.
Term and Kill use the group wrapper. EndPayload uses the process wrapper.
Payload signaling retains the live-child condition and signal conversion.
OwnedGroup cleanup and the existing real-process helpers use the same guard.
The changes add no child wait, sleep, process fixture, or new timer value.
The changes preserve the existing group anchor, reserve child, cleanup bound, and production reaping rules.
No separate guard logic finding exists at this head.

The default tests cover invalid targets, own targets, allowed targets, errno preservation, and refusal conversion.
The wrapper test uses SIGCONT for own targets, so a failed refusal does not terminate the test process.
The corrupted-row test records pid 1 and supplies a matching identity probe.
It exercises AdoptAll, Stop, and Remove through Core and records refused signal requests.
It verifies that the actual worker still has a matching identity.
The reviewer accepts these behavior tests together with the OS-wrapper tests.

### F54 — MEDIUM — OPEN — Existing lint allowances and slow-only code bypass the raw-signal ban

The six Clippy configuration files ban both raw signal methods.
`xtask/src/signal_bans.rs` verifies only that each configuration contains both entries.
It does not verify the effective lint scope.

`crates/botster-core/tests/slow_real_core.rs:8` retains file-wide `allow(clippy::disallowed_methods)`.
Its changed worker signal call previously used raw `rustix::process::kill_process`.
Restoring that raw call remains exempt from the ban, even if Clippy compiles the slow feature.
Other existing item and file allowances have the same scope problem.
The configuration test remains green because no configuration entry changes.

`xtask/src/ci.rs:82` runs Clippy for the workspace and all targets, but it does not enable the slow feature.
`slow_real_core.rs` has `cfg(feature = "slow")` at its file root.
The slow test step enables that feature through nextest, which does not run Clippy.
Thus a raw call in slow-only code can also avoid the ban without any lint allowance.
The completed green gate does not establish enforcement for this code.

Required closure: enforce the raw-call ban across the required feature scopes and existing lint allowances.
Only the two guarded OS calls may receive the necessary exception.
Provide a safe red-on-revert proof that a raw call outside the guard makes the check fail.
The proof must not execute the raw call.
This finding concerns mechanical enforcement, not a current raw-call survivor in product code.

### Completed evidence and verdict

The exact-head Linux log is:
`~/botster-sessions/gates/botster-core-stage1-p3-group-signal-guard-9be81027-pool-20261009-033702-65783.log`.
It names this head and base `0b684a19`.
It runs full `cargo xtask ci`, then the in-diff mutation step with `NEXTEST_PROFILE=slow`.
All ten CI steps pass. The timer check scans 125 Rust files.
The default tier passes 853 tests. The slow tier passes 218 tests.
Both mutation runs report eight tested: six caught, two unviable, zero missed, and zero timed out.
The full CI summary reports 28.9 seconds. The combined job exits 0 after 41 seconds on msa1.
The gate selects and passes the new guard, corrupted-row, and configuration tests.
These results do not close F54 because the gate does not check the missing lint scopes.

PR #177 is NOT CLEAN at this exact head for F54.
The reviewer sent F54 directly to the P3 implementer.
PR #168 retains its single real-PTY HOLD from round 101. Part B retains its previous open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN

## Round 103 — Integration findings on the guarded signal change

Reviewed head: `9be8102708a33c1a4c79693e09797ab6942c7c3c`, PR #177.
Base: `0b684a19470c5b647e64653fd285d4cd43544278`.
Authority and completed gate evidence remain those of round 102. No new head or gate exists in this round.
The integration reviewer sent G1 and S1 after round 102.
Its verdict commit is `a567bb2b31d731b0829519152813215942177ff2`.
Its note accepts F54 at `69b7a74a75357e4b3a442dccf73ef4a13fe31486`.
The package reviewer inspected both source paths and confirms the findings below.
The integration identifiers remain in use. No duplicate package identifiers are assigned.

### G1 — MEDIUM — OPEN — The guard refuses the anchor's final cleanup signal

`guard_cleanup::end_group` starts in the group that it must end.
It tries to spawn a reserve child, then move the anchor to another group.
If either operation fails, its error branch reports the failure and sends a final KILL to the original group.
When that branch runs, the anchor still belongs to the original group.
The migrated `signal_group` call refuses this target as `Own` and sends no signal.
The branch ignores the refusal and returns its report. The anchor exits, but other members can remain alive.
The normal path still moves the anchor before bounded cleanup rounds.
Thus round 102's statement that this change preserves cleanup behavior was incorrect for the reserve-error path.

The correction must retain the own-target refusal for identifiers from records.
It must also preserve the intentional final signal to the anchor's own group.
The integration reviewer proposes a separate operation that takes no recorded target and signals the current group.
The correction must have behavior evidence for the failure path and retain the anchor's reaping and cleanup rules.
The package reviewer sent its confirmation directly to the P3 implementer.

### S1 — LOW — OPEN — The gate's shell signal calls bypass the guard

`xtask/src/test_budget.rs:140` invokes the external `kill` command.
The timeout path passes the negative identifier of the group that the xtask created.
The cleanup path passes process identifiers from the process list, then that negative group identifier.
These calls do not use the new target refusal.
The Rustix method bans do not check an external command.
This is an existing call path within the signal pattern that the lead requires the change to address.
The package reviewer sent this confirmation directly to the P3 implementer.

### Verdict

F54 remains OPEN. G1 and S1 remain OPEN under their integration identifiers.
The completed green gate from round 102 does not exercise the reserve-error path or enforce the missing signal checks.
PR #177 is NOT CLEAN at this exact head.
PR #168 retains its single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN

## Round 104 — Signal scan, own-group operation, and the #171 merge

Reviewed head: `f16eed6fb50da9581060c06acc6f2dded1d61ca0`, PR #177.
Base: `fc23cd9747e64e24f99565543b45ae5f7400a71c`, v1 after #171.
Reviewed delta: changes after `9be81027`, merge `75482fa1`, and the final thirty-one-file diff against the submitted base.
Authority: BUILD.md, plan r22 pin `~/botster-sessions/pins/stage1-plan.71a623ef.md`, and the lead's signal and anchor rulings.
The pin SHA256 remains `71a623ef93f487754e400dc186429216357e39fe251933675cc7f851843d519d`.
The lead's current handoff states that #171 landed, but the HOLD on new real-process test code remains until P6 PR B lands.
The reviewer read the full updated PR description through gh after the MCP response truncated it.

### F54 — CLOSED — Raw-method allowances and disabled features no longer hide calls

The new `cargo xtask signals` step scans token trees from tracked Rust files.
It rejects all three raw Rustix signal identifiers outside `core-sys/src/signal.rs`, including imports and renamed imports.
The scan descends into token groups, including macro groups, without applying cfg or lint attributes.
A lexer failure is a violation. Comments and string contents do not count as raw method identifiers.
The taint job runs this scan after its existing checks.
Clippy also enables all features. Each of the seven Clippy configuration files bans all three raw methods.
The default tier selects and passes the source fixtures for allowances, cfg, imports, and renamed imports.
The step verdict has a selected test that fails when a source fixture adds one violation.
The implementer also reports a manual safe revert of slow_real_core's old raw call, followed by a failing scan and source restoration.
These changes close F54's original raw-method scope gaps.
The new shell-command scan has a separate finding, F56 below.

### S1 — CLOSED — Current gate cleanup calls use the guard

`test_budget::kill` no longer invokes the external kill command.
Its pure parser maps a negative identifier to a group and a positive identifier to a process.
The dispatch calls `signal_group` or `signal_process` with KILL.
Each wrapper performs the existing invalid-target and own-target refusal.
The selected parser test covers process targets, group targets, and invalid text.
Only the whole OS dispatch body receives a mutation exclusion. The parser and refusal decisions remain tested.
The reviewer closes S1 for the current call path. The new ban's shell coverage remains subject to F56.

### G1 — OPEN for proof — The own-group source correction is accepted

`signal_own_group` takes no recorded target. It checks the current group, then calls `kill_current_process_group`.
The pure own-group condition refuses 0 and 1. The recorded-target wrappers retain their own-target refusal.
The old cleanup error path and the new test-process cleanup error path use the own-group operation.
The new anchor's TERM operation also uses it while the anchor holds membership.
The bounded KILL rounds use `signal_group` after the anchor leaves the group and while its reserve holds the identifier.
The merge preserves identity verification, group verification, cleanup deadlines, and exact reserve reaping.
The source correction restores the intended distinction between deliberate own-group cleanup and recorded-target signaling.

The new SIGURG test proves delivery to the current process, subject to F55 below.
It does not exercise the reserve-error branch that caused G1.
The PR explicitly defers a child-in-a-new-group test and the reserve-failure proof until P6 PR B lands.
The reviewer accepts the source correction but cannot close G1's required behavior proof at this head.
The existing green tests do not establish that this failure path ends its members.
The integration reviewer closes G1 with a carry to PR B in verdict `3f8a9c3aa2ec98ee48f57b8960019fda5ef6e145`.
The package reviewer will ask the lead to resolve this proof requirement. G1 remains open here pending that ruling.

### F55 — MEDIUM — OPEN — The signal-delivery test polls instead of waiting on an event

`signal.rs:169` adds `a_signal_to_our_own_group_reaches_this_process`.
It polls an AtomicBool up to 1,000,000 times and calls `thread::yield_now` between checks.
BUILD.md testing rule 5 prohibits polling and requires waiting on the real event.
An iteration limit does not satisfy that rule or provide a deadline for signal delivery.
Replace the polling loop with a completion wait and an allowed marked deadline.
The reviewer sent F55 directly to the P3 implementer.

### F56 — MEDIUM — OPEN — The command scan misses renamed imports and escaped program names

`signals::kill_program` matches only an identifier whose text is `Command`.
`use std::process::Command as Proc; Proc::new("kill")` starts the banned program but fails this match.
`signals::string_text` retains string escapes instead of decoding them.
`Command::new("ki\x6cl")` also starts kill, but the scan compares the source spelling with the decoded program name.
Thus both valid Rust forms produce no violation at this head.
The Clippy method bans do not apply to external program names.
Plan section 8 requires syntax-aware mechanical checks that handle aliases and use renames.
The new check does not fully enforce the shell-signal pattern that it claims to ban.
Check renamed Command imports and decoded string literals. Add safe source fixtures that execute no program.
The reviewer sent F56 directly to the P3 implementer.

### L1 — LOW — OPEN — The signal ban does not cover the allowed unsafe scope

The integration reviewer reports L1 at this head. The package reviewer confirms its scope.
The raw-call token list and Clippy entries omit `libc::kill` and `libc::killpg`.
The new test-process anchor has an existing allowed unsafe scope in `close_inherited`.
The unsafe-exception check verifies that scope's name and attribute; it does not ban signal calls inside its body.
A raw libc signal call in that allowed body bypasses both checks.
The terminal binding also has the existing whole-crate unsafe exception.
Thus the PR's claim that workspace unsafe forbidding already blocks libc signal calls is incomplete.
Extend the mechanical signal ban to these raw libc operations, including imports and renamed imports.
Retain the integration identifier L1. The source currently contains no libc signal call.

### Merge and mutation exclusions

Merge `75482fa1` shares the mutation configuration, lock file, xtask manifest, CI module, and command table with #171.
The final manifest retains #171's parser dependencies and adds core-sys for guarded gate cleanup.
The final taint job retains the unsafe-exception check and adds the signal scan after it.
The final command table retains #171's commands and adds signals.
The mutation configuration retains #171's entries and adds only the documented signal-related changes against v1.
The new test-process crate uses core-sys for all three former raw group signals.
Its old platform target helper and test are removed; the shared refusal tests cover their group-1 property.
No other platform wait, identity, or cleanup decision changes in that crate.
The reviewer inspected this merge delta rather than applying the base-only merge exception.

The new Clippy whole-body exclusion covers process launch glue only.
The updated taint whole-body exclusion names the separately tested signal decisions.
The whole-body kill exclusion names the selected parser and refusal tests.
No signal-scan verdict or other pure gate decision receives a new exclusion.

### Completed evidence and verdict

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-group-signal-guard-f16eed6f-pool-20261009-040121-96622.log`.
It names head `f16eed6f` and base `fc23cd97`.
All ten full CI steps pass. The signal scan reads 153 Rust files; the timer check reads 137.
The default tier passes 918 tests in 1.399 seconds. The slow tier passes 241 tests in 10.121 seconds.
The full and separate no-termination mutation runs each report 44 tested: 41 caught, three unviable, zero missed, zero timed out.
The separate run uses `NEXTEST_PROFILE=slow`.
The CI summary reports 151.3 seconds. The separate mutation summary reports 102.6 seconds.
The combined job exits 0 after 279 seconds on msa1.
The new parser, own-group, scan, and verdict tests run in the default tier and pass.
The intermediate gate failures have corresponding source changes; this READY cites the completed final-head gate.
The green result does not close F55, F56, L1, or G1's deferred proof.

PR #177 is NOT CLEAN at this exact head. F54 and S1 are CLOSED. F55, F56, L1, and G1's proof remain OPEN.
PR #168 retains its separate single real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN

## Round 105 — Lead ruling on G1's required proof

Reviewed head: `f16eed6fb50da9581060c06acc6f2dded1d61ca0`, PR #177.
Base: `fc23cd9747e64e24f99565543b45ae5f7400a71c`.
No new source head or gate exists in this round. Round 104's source review and evidence remain in force.

The lead answered the package question on 2026-10-09:
G1's reserve-failure proof does not carry to P6 PR B. It must pass before PR #177 is CLEAN.
The new-test HOLD does not block this proof because the crate owner's exception permits botster-test-process's own tests.
The proof belongs in that crate's own test suite, within PR #177.
P3 writes it with the crate's fixtures. P6, the crate owner, checks it.
If the behavior reduces to a decision, a pure decision test plus the existing I/O shell is acceptable.
The lead requires the integration reviewer to reopen G1 until the proof exists.
This ruling resolves the review difference recorded in round 104.
It does not change the accepted own-group source correction or the required guard and anchor invariants.

G1's proof remains OPEN. F55, F56, and L1 remain OPEN.
F54 and S1 remain CLOSED at this head.
PR #177 is NOT CLEAN. PR #168 retains its separate real-PTY HOLD. Part B retains its earlier duties.
The reviewer sent the ruling to the P3 implementer and integration reviewer.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN

## Round 106 — Reserve-failure proof, event wait, and remaining alias coverage

Reviewed head: `05c70358eab8f1a8e1ca77280a68b135dc2f990f`, PR #177.
Base: `fc23cd9747e64e24f99565543b45ae5f7400a71c`.
Reviewed delta: the sixteen-file change after `f16eed6f`, plus the final guard and scan source.
Authority: BUILD.md, plan r22 pin `~/botster-sessions/pins/stage1-plan.71a623ef.md`, and the lead's G1 ruling recorded in round 105.
The reviewer read the complete updated PR description and exact-head gate log.
No new base merge exists in this delta. Round 104's merge review remains in force.

### G1 — CLOSED — The required failure-path proof runs and passes

`end_group` now calls `end_group_reserved` with the unchanged reserve operation.
The latter accepts the reserve operation as an injected edge in the dev-only test-process crate.
Its error path calls `signal_own_group(KILL)` before returning.
The new slow test starts a helper in its own group and keeps the helper's stdin open.
The helper starts an owned cat process that inherits that group and stdin.
The helper calls the same cleanup function with a reserve operation that returns an error.
The test waits for EOF on the pipe held by both processes, before outer cleanup can run.
It then requires the helper's exit signal to be KILL.
These observations establish that the final signal ends the helper and its member.
The helper's owner cannot run its normal member cleanup after KILL.
Without the final signal, the helper returns, its member owner cleans up, and its normal exit fails the signal assertion.
The implementer reports this safe red-on-revert result: left None, right Some(9).
The exact-head slow gate selects and passes `an_unreserved_cleanup_ends_every_member_by_its_last_kill`.

The fixture uses the crate's existing OwnedChild, bounded pipe reads, and cleanup deadline.
It adds no raw child wait, sleep, polling loop, or test behavior branch to product code.
The existing reserve reaping, identity checks, group checks, and cleanup bounds remain intact.
P6's shared owner handoff records its OK on this proof at `05c70358`.
The implementer's READY and PR description report the same owner check.
The package reviewer also requested direct confirmation from P6.
This existing written owner check and the completed proof satisfy the lead's own-crate exception.
The reviewer closes G1 at this head.

### F55 — CLOSED — The signal test waits on the real completion

The SIGURG handler now writes a byte to an anonymous socket pair through signal-hook.
The test blocks in `read_exact` for that byte and unregisters the handler before checking the result.
A marked deadline bounds the socket read with the existing ten-second cleanup value.
The old AtomicBool polling loop and yield calls are removed.
The exact-head default tier selects and passes this test.
The reviewer closes F55 under BUILD.md testing rule 5.

### F56 — OPEN — Composed aliases still bypass the command scan

The scan now uses `syn::LitStr::value` to decode each literal.
The escaped program names from round 104 are recognized, including hexadecimal and Unicode escapes.
The scan also recognizes a direct Command import rename and a type alias whose right side names Command.
The selected fixtures cover these corrected forms and pass.

It does not combine these forms:
`use std::process::Command as Proc; type Shell = Proc; Shell::new("kill")`.
`command_names` records Proc from the import, but checks each type right side only for the literal identifier Command.
It therefore omits Shell. The call produces no command violation.
The command starts the same banned program as the direct call.
Resolve alias chains for the checked operations, including combinations of import renames and type aliases.
Add safe source fixtures that execute no program.
F56 remains OPEN. The reviewer sent this remaining case directly to P3.

### L1 — OPEN — A renamed libc module bypasses the token scan

All seven Clippy configuration files now ban `libc::kill` and `libc::killpg`.
The token scan recognizes killpg and direct libc::kill paths or import lists.
Its selected source fixture covers those forms and passes.

`use libc as sys; unsafe { sys::kill(-1, libc::SIGKILL); }` produces no token violation.
`libc_kill` requires the literal module name libc, while the general raw identifier list omits kill.
In a permitted unsafe scope with a disallowed_methods allowance, Clippy does not close this gap.
The mechanical check must resolve the renamed module and reject this raw signal operation.
L1 remains OPEN under its integration identifier. The reviewer sent this case directly to P3.
The current source contains no such raw call; this finding concerns the required pattern enforcement.

### Documentation and completed evidence

The PR's new review section describes the source changes and the completed proof.
Its opening ban summary still claims six configuration files, two guarded calls, and complete libc protection from unsafe forbidding.
The reviewer requested a summary correction to seven files, five banned methods, three guarded calls, and the actual unsafe exceptions.
The old G1 proof deferral must be marked as superseded by the lead's ruling and the new proof.
These corrections accompany the remaining scan findings.

The new mutation exclusions cover the extracted reserve and end_group_reserved OS glue.
Their comments name the selected group cleanup tests and the new failure-path proof.
The scan and its verdict decisions remain mutation-tested.
The new syn dependency supplies literal parsing. The lock file already contains syn 2.0.119 and adds only its xtask use.

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-group-signal-guard-05c70358-pool-20261009-042155-67173.log`.
It names this head and base `fc23cd97`.
All ten full CI steps pass. The signal scan reads 153 Rust files; the timer check reads 137.
The default tier passes 919 tests. The slow tier passes 243 tests in 10.127 seconds.
Both mutation runs report 69 tested: 66 caught, three unviable, zero missed, and zero timed out.
The separate mutation run uses `NEXTEST_PROFILE=slow`.
Full CI reports 167.4 seconds. The separate mutation run reports 148.8 seconds.
The combined job exits 0 after 328 seconds on msa1.
The gate selects the new G1 proof, event-wait test, decoded-literal fixture, and direct-alias fixture.
The implementer also reports a local signal-scan mutation run with all 56 mutants caught.
The green gate does not cover the composed aliases or renamed libc module above.

PR #177 is NOT CLEAN at this exact head for F56 and L1.
G1 and F55 are CLOSED. F54 and S1 remain CLOSED.
PR #168 retains its separate single real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN

## Round 107 — Final signal guard review under the lead's enforcement ruling

Reviewed head: `0e0becfbe04a4236206fa56402b38d343c062581`, PR #177, branch `stage1/p3-group-signal-guard`.
Base: `fc23cd9747e64e24f99565543b45ae5f7400a71c`.
Reviewed delta: the two-file change after `05c70358`, with the whole guard review and #171 merge review from rounds 102–106.
Authority: BUILD.md, plan r22 pin `~/botster-sessions/pins/stage1-plan.71a623ef.md`, and the lead's signal, G1, and enforcement rulings.
The pin SHA256 remains `71a623ef93f487754e400dc186429216357e39fe251933675cc7f851843d519d`.

### Enforcement ruling and finding closures

The lead's ruling of 2026-10-09 makes Clippy disallowed-methods the name-resolving authority for signal and clock bans.
P6 owns the syntax-aware ban on allowance attributes outside specified sites, including expect and blanket allowances.
P3 owns the follow-up that moves the seventeen clock allowances to one helper per crate, with integration review.
Each permitted allowance must apply only to its intended operation.
The follow-up proof must reject a raw signal call inside an otherwise permitted clock-helper scope.
The lead assigns these changes to follow-up work and forbids further alias, glob, or re-export findings against this token scan.
This explicit ruling governs this package verdict. The follow-ups remain required work for their owners.

All seven Clippy configuration files ban the three raw Rustix signal methods and both raw libc signal methods.
The Clippy gate compiles all targets and all features with warnings denied.
The default configuration test checks all five ban entries in every configuration file.
The current token scan also recognizes the composed aliases and renamed libc module from round 106.
It accumulates names until no new alias remains. Each iteration adds only names found in the finite source token tree.
Its selected safe fixtures cover the prior F56 and L1 examples, import chains, type-alias chains, and libc globs.
The fixtures execute no signal or external program.
F56 and integration L1 are CLOSED in this package review at the exact head, under the lead's ruling.
No further naming finding is raised against the token scan.
F54 and S1 remain CLOSED.

### Guard, own-group operation, and completed behavior proof

The recorded-target wrappers retain the refusal of 0, 1, values above the pid range, and the caller's own group or process.
They retain OS errors and preserve errno in conversion to io::Error.
Production callers retain their identity and live-child conditions before signaling.
The separate own-group operation takes no recorded target and refuses current group 0 or 1.
Intentional anchor TERM and reserve-error KILL use it. Reserved cleanup rounds signal from outside the group.
The changes preserve start-time checks, group checks, the reserve, bounded cleanup, and exact reserve reaping.
No change transfers production reaping to the anchor.

G1 remains CLOSED. The final slow gate selects and passes the required reserve-failure proof.
The test waits for EOF before outer cleanup and requires the helper's KILL exit signal.
The final delta adds an assertion that the helper's group equals its own process identifier before the deliberate group kill.
This assertion strengthens the already reviewed group isolation and adds no new process fixture or timed loop.
P6 directly confirmed its owner check of the proof and helper at `05c70358` under the lead's own-crate exception.
The final delta preserves that proof and changes only the group assertion within its helper.
The reported red-on-revert result remains recorded in round 106.

F55 remains CLOSED. The SIGURG test waits for the handler's byte on an anonymous socket pair with a marked deadline.
The final default gate selects and passes it. No polling or sleep returns in this delta.
The corrupted-row Core test and the pure target, own-group, and error tests retain their reviewed behavior.
No new package logic finding exists in the final delta or the whole guard change.

### Description and exact-head evidence

The reviewer read the complete final PR description.
It now states seven configuration files, five banned methods, three guarded calls, and the actual unsafe exception.
It marks the old G1 proof deferral as superseded and names the lead's enforcement follow-ups and their owners.
Its gate section names the submitted exact head and completed log.
The description matches the final source, evidence, scope, and lead rulings.

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-group-signal-guard-0e0becfb-pool-20261009-044148-62086.log`.
It names this head and base `fc23cd97`.
All ten full CI steps pass. Signals scan 153 Rust files; timers scan 137.
The default tier passes 919 tests in 1.602 seconds. The slow tier passes 243 tests in 10.130 seconds.
Both mutation runs report 76 tested: 73 caught, three unviable, zero missed, and zero timed out.
The separate mutation run uses `NEXTEST_PROFILE=slow` and has no nextest termination.
Full CI reports 198.1 seconds. The separate mutation step reports 176.3 seconds.
The combined job exits 0 after 381 seconds on msa1.
The final gate selects the G1 proof, signal event test, alias-chain fixture, and existing guard and configuration tests.
The reviewer read this evidence and ran no gate.

### Verdict and scope

PR #177 is CLEAN at `0e0becfbe04a4236206fa56402b38d343c062581` for the P3 package review.
F54, F55, F56, G1, S1, and L1 are CLOSED at the named scope and authority.
The lead's enforcement follow-ups remain assigned work. They do not block #177 under its explicit ruling.
This verdict covers this exact source head and its completed gate, not a later source or base merge.
The integration reviewer controls its own verdict.
PR #168 retains its separate single real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN

## Round 108 — Narrow clock and build allowances

Reviewed head: `770cc9f786222c1dd2a13f6b456e7c9f1fbfea25`, PR #178, branch `stage1/p3-clock-helpers`.
Base: `da43a1b1f85fd0b62da2e69767fc6ea0ba8cd594`, v1 after #177.
The reviewer inspected the full fourteen-file diff at `25e1dbe3` and the final W1 correction at this replacement head.
Authority: BUILD.md, plan r22 pin `~/botster-sessions/pins/stage1-plan.71a623ef.md`, and the lead's #177 enforcement ruling.
The pin SHA256 remains `71a623ef93f487754e400dc186429216357e39fe251933675cc7f851843d519d`.
The lead assigned this clock-helper follow-up to P3 and requested this package review.

### Source and allowance scope

The changed test clock sites call helpers that contain only `std::time::Instant::now`.
Each call still reads the real clock at its original position and passes that value to the same injected clock or pump.
The helpers introduce no cached time, shared state, lock, extra timer, or loop.
The host and worker helpers remain in test-only modules.
The Core unit-test helper requires both test and slow, matching its two slow-test callers.
The integration-test helper and shared guard helper serve their separately compiled test targets.
The guardian's existing one-call helper already has the required shape and is unchanged.
Guard deadlines and platform wait arguments retain the same time calculations and marked deadlines.

Both Core slow-test files lose their file-wide disallowed_methods allowances.
The testkit loses seven unnecessary allowances. Its own Clippy configuration permits real clocks and bans raw signal methods.
Removing these allowances restores the signal ban across those test bodies without changing their clock calls.
The Ghostty build script loses its file-wide allowance.
Its five wrappers each contain only the intended environment or filesystem call.
They preserve the original arguments, return values, fallback behavior, and error handling at every call site.
No build option, output path, package check, or network condition changes.

The reviewer inspected all remaining allowance sites at this exact head.
They are six clock helpers, five build wrappers, and the three unchanged guarded signal expressions from #177.
Each current allowance covers only its intended operation.
The PR's source table matches this inventory.
No broad file allowance, whole-test allowance, or unnecessary testkit allowance remains in this set.
No new real-process fixture, raw wait, sleep, polling loop, machine clock read, or gate decision is added.
The reviewer has no package behavior or allowance-scope finding.

### Integration W1 and assigned enforcement follow-up

The replacement delta adds the slow feature condition to Core's unit-test helper and documents its slow-only use.
This resolves integration W1: the prior default test build compiled an unused helper and emitted a dead_code warning.
The final helper's conditions match both lib.rs and real.rs callers.
The final default build no longer emits that warning.
The integration reviewer reports CLEAN at this head in verdict `99dca333211faa163d3c7491c388dfbc413fea7a`.
The package reviewer accepts this correction and keeps the integration identifier.

P6's attribute ban and its red-on-revert fixture remain assigned follow-up work under the lead's ruling.
They must reject a raw signal call inside an otherwise permitted clock-helper scope.
This PR supplies the narrow source sites that the check will pin to their intended method.
The token scan remains in place. Its planned reduction correctly waits for that attribute ban.
This package verdict does not claim that the pending check has landed.

### Completed evidence

The reviewer read the complete final PR description, including its corrected head, log, and W1 history.
Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-clock-helpers-770cc9f7-pool-20261009-050449-83939.log`.
It names this head and base `da43a1b1`.
All ten full CI steps pass, including Clippy with all features, prebuild, and both test tiers.
Signals scan 153 Rust files. Timers scan 137.
The default tier passes 919 tests in 1.560 seconds. The slow tier passes 243 tests in 10.119 seconds.
Both mutation commands explicitly report `INFO No mutants to filter` before the older ambiguous no-outcomes message.
Every changed logic line belongs to test-only code, an integration test file, or build.rs.
The reviewer checked cargo-mutants 27.1.0's primary source: it skips cfg(test) nodes and does not discover build-script targets.
Thus the no-mutant result is consistent with the diff and the actual tool output.
The reviewer does not infer success from the no-outcomes message alone.
The separate mutation command uses `NEXTEST_PROFILE=slow`.
Fuzz reports no changed crate with a decoder harness.
Full CI reports 25.0 seconds. The separate mutation step reports 0.3 seconds.
The combined job exits 0 after 31 seconds on msa1.

### Verdict and scope

PR #178 is CLEAN at `770cc9f786222c1dd2a13f6b456e7c9f1fbfea25` for the P3 package review.
No package finding remains open for this change. Integration W1 is corrected at this head.
The required P6 attribute check remains outside this PR under the lead's explicit assignment.
This verdict covers this exact head and completed gate, not a later source or base merge.
PR #177 remains CLEAN at its named scope and is merged in v1 as the submitted base.
PR #168 retains its separate single real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN

## Round 109 — HIGH-path list coverage

Reviewed head: `b1992a0f349466deef5b3aef75ce4b9595a637c2`, PR #184, branch `stage1/p3-high-tier-paths`.
Branch parent: `b520f8324d6a55be7b3b521e8c6a5b9a066039f1`.
Completed gate base: `aaac0c0d1f44172ca5d5dd5dd6986c9787be4aa6`.
The merge base of the reviewed head and the completed gate base is the branch parent above.
The PR changes only `ci/high-tier-paths.txt`: 36 added lines.
Authority: BUILD.md at contracts main `56bd0a5347a537d25bbee65a67854e0e317a9b9a`, Risk tiers and Round limit.
This head starts after the lead's new review rules. Those rules apply to this round.
The reviewer checked the stated tier first. HIGH is correct under rule 1 because the PR changes a HIGH-path list.
HIGH requires package and integration reviews, and every finding must close.

### F57 — MEDIUM — The list omits current process-control and adoption behavior

The list names OS adapters and several machines, but it excludes other files that own rule-5 behavior.
The PR description explicitly excludes the decision files because listed files contain the target checks, proof checks, and OS calls.
Those checks do not protect the ordering, timing, or absence of process actions.
A PR confined to an omitted file can receive STANDARD even when it changes the following behavior:

- `crates/botster-core-host/src/run.rs:275`: `kill_payload` selects the stop-grace signal and worker identity.
- `crates/botster-core-host/src/run.rs:667`: row recovery starts adoption after a restart.
- `crates/botster-core-host/src/run.rs:739`: `session_of_row` restores the token and identity and decides whether to adopt.
- `crates/botster-core-host/src/flows.rs:597`: Remove repeats identity checks and waits for the worker to end.
- `crates/botster-core-host/src/flows.rs:618`: `flow_remove_probed` permits a kill only when the recorded identity matches.
- `crates/botster-core-host/src/driver.rs:274`: the driver dispatches worker spawn, identity checks, and signals to the real edges.
- `crates/botster-core-host/src/admit.rs:285`: adoption admission limits which Lost sessions can retry.
- `crates/botster-core-host/src/admit.rs:751`: an admitted adoption starts the adoption flow.
- `crates/botster-core-host/src/engine.rs:463`: the engine defines the protocols that adoption accepts.
- `crates/botster-core-host/src/engine.rs:608`: the engine selects the worker identity for process actions.
- `crates/botster-guardian-core/src/guardian.rs:433`: the guardian permits reaping only after cleanup settles.
- `crates/botster-guardian-core/src/guardian.rs:534`: the guardian chooses the kill target and known descendants.
- `crates/botster-guardian-core/src/guardian.rs:543`: the guardian orders the descendant enumeration before TERM or KILL.

The specific rule-5 areas take priority over the generic STANDARD example for sans-IO logic.
The list already applies this principle to the pure PTY decisions in io_decisions.rs and drain.rs.
Add the six files above to cover their current risky behavior.
This finding does not require every transitive helper or codec to be listed.

The description must also correct its fallback claim for future fd handoff.
BUILD.md applies the area-based fallback only when a repo has no HIGH-path list.
After this list exists, new rule-5 code must extend the list. The lead or reviewer may also raise a tier.
The reviewer accepts no current fd-handoff entry because the source delegates descriptor passing to future P4a work.
The reviewer sent F57 and its complete scope directly to P3.

### Accepted scope and completed evidence

The reviewer accepts the whole worker.rs and inbound.rs entries because each file contains a proof check.
The reviewer accepts io_decisions.rs and drain.rs for PTY behavior, and session.rs for restart decoding.
The listed process adapters, durable storage files, entropy source, launch parser, and proof files belong to their stated areas.
All 19 entries match tracked files at this exact head.
Rules 1 through 4 independently cover gate code, unsafe and FFI code, cross-package changes, shared crates, workspace configuration, and pins.
A mechanical stale-entry check is outside this PR. BUILD.md does not require that new check for this list.
The PR changes no runtime behavior and adds no real-process test.

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-high-tier-paths-b1992a0f-pool-20261009-083957-49200.log`.
The log names the reviewed head and the gate base above.
All ten CI steps pass. The default tier passes 920 tests in 1.668 seconds.
The slow tier passes 243 tests in 10.141 seconds.
Signals scan 153 Rust files. Timers scan 137 Rust files.
Both mutation commands explicitly report `INFO Diff changes no Rust source files`.
The reviewer accepts that result because the PR changes only the text list.
The separate mutation command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 164.0 seconds. Separate mutants take 0.4 seconds.
The combined job exits 0 after 229 seconds on msa1.
The gate passes but does not close the source-coverage finding.

### Verdict and scope

PR #184 is NOT CLEAN at this exact head for F57.
This is the first NOT CLEAN round for #184. The round-limit notice is not due.
The integration reviewer controls its own verdict.
Round 108 CLEAN for #178 remains valid at its named head and scope.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN

## Round 110 — HIGH-path list and its gate check

Reviewed head: `b9c3ea2a8ffbdc6317241de248af24fd053bc720`, PR #184, branch `stage1/p3-high-tier-paths`.
Branch base: `b520f8324d6a55be7b3b521e8c6a5b9a066039f1`.
Completed gate base: `aaac0c0d1f44172ca5d5dd5dd6986c9787be4aa6`.
The merge base of the reviewed head and completed gate base remains the branch base above.
The reviewer inspected the full five-file PR change, the delta from b1992a0f, and the final outcome correction from b40e9bc3.
Authority: BUILD.md at contracts main `56bd0a5347a537d25bbee65a67854e0e317a9b9a`, Risk tiers and Round limit.
The reviewer checked the stated tier first. HIGH is correct under rule 1.
The PR changes the HIGH-path list, CI wiring, and the mutation profile. Each is gate-decision code.
HIGH requires package and integration reviews, and every finding must close.

### F57 is CLOSED

The list now includes core-host driver.rs, run.rs, flows.rs, admit.rs, and engine.rs, plus guardian-core guardian.rs.
Each added entry names the current process-control or adoption behavior that makes its file HIGH.
The flows.rs reason also names the session token draw.
The header states that process-control and adoption decisions belong on the list, including decisions about timing and targets.
The header gives rule-5 areas priority over the generic sans-IO example.
All 25 entries match tracked files at this exact head.
The reviewer retains round 109's accepted coverage for the other entries.

The complete PR description corrects the fd-handoff fallback claim.
It states that the fallback applies only to a repo without a list.
It requires new rule-5 code to extend this list and notes that the lead or reviewer may raise a tier.
The reviewer accepts the absence of a current fd-handoff entry because current code delegates descriptor passing to future P4a.
The description preserves the broad entries for machines with proof checks and the pure PTY decisions.
No package finding remains on the list or its description.

### Gate check and mutation scope

The new high-tier command validates the list against tracked paths from git ls-files.
The command propagates file-read, git, and validation errors.
verdict skips blank and comment lines, splits the entry from its reason, and reports every problem with its line number.
The check requires a nonempty reason, an exact path or trailing directory wildcard, and at least one tracked match.
The directory match requires the slash boundary, so a sibling with the same prefix does not match.
The pure outcome rejects a nonempty problem list and returns the pass line only for an empty problem list.
The final correction moves the pass/fail decision out of command and into this tested function.
The command contains only I/O and error propagation.

Eight named tests cover valid entries, stale entries, directory boundaries, missing reasons, invalid globs, all error lines, and step outcomes.
The repository test checks the actual list against the walked tree because the mutation copy has no git metadata.
The completed default test log selects and passes all eight tests.
The PR reports a safe failing check with lock_moved.rs replacing lock.rs in the list, followed by a pass after restoration.
The reviewer read that reported proof and the completed gate. The reviewer did not run the command or modify the list.

The standalone command is registered in the same table used by command selection.
The taint step calls high_tier after the existing four checks and propagates its error.
The existing taint-job exclusion gains an accurate note for the fifth check.
The only new exclusion matches the whole-body command replacement with Ok(()).
It does not exclude verdict, matches, outcome, or their individual decisions.
The exclusion names its I/O limitation and the selected tests that prove the decisions.
The reviewer accepts this narrow I/O exclusion under the lead's existing ruling for command glue.
No real-process fixture, raw wait, sleep, polling loop, timer value, or runtime product behavior changes.

### Completed evidence

The reviewer read the complete PR description and the final exact-head log.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-high-tier-paths-b9c3ea2a-pool-20261009-085207-67495.log`.
It names this reviewed head and completed gate base.
All ten CI steps pass. The taint step prints the high-tier pass line.
The default tier passes 928 tests in 1.641 seconds. The slow tier passes 243 tests in 10.128 seconds.
Signals scan 154 Rust files. Timers scan 137 Rust files.
Both mutation runs test 13 mutants and catch all 13, with zero missed, timed out, or unviable.
The earlier b40e9bc3 survivor was the negated pass/fail decision in command.
The final head moves that decision into outcome, whose failure and success test is selected and passes.
The final completed mutation evidence supersedes the earlier failure.
The separate mutation command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 105.5 seconds. Separate mutants take 81.2 seconds.
The combined job exits 0 after 195 seconds on msa1.

### Verdict and scope

PR #184 is CLEAN at `b9c3ea2a8ffbdc6317241de248af24fd053bc720` for the P3 package review.
F57 is closed. No package finding remains open for this change.
The integration reviewer controls its own verdict and must review this new head.
Its prior CLEAN at b1992a0f was withdrawn in verdict a406fdab922d508aa9a6a5a33076151b84c90c04.
This package CLEAN covers only this exact head, branch base, and completed evidence. It covers no later source or base merge.
#184 had one NOT CLEAN round before this CLEAN. The round-limit notice is not due.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN

## Round 111 — Kernel answers in the pidfd guard test

Reviewed head: `1bfc6fd39fa077bec92b7fe021d5d7c9f6798618`, PR #186, branch `stage1/p3-kernel-errno`.
Base: `aaac0c0d1f44172ca5d5dd5dd6986c9787be4aa6`.
The reviewer inspected the complete one-file change, its guard callers, the full PR description, and both completed Linux logs.
Authority: BUILD.md at contracts main 56bd0a5347a537d25bbee65a67854e0e317a9b9a and the lead's process-control-test ruling.
The reviewer checked the tier first. HIGH is correct under that ruling.
HIGH requires package and integration reviews, and every finding must close, including LOW findings.

### F58 — LOW — The documentation excludes a possible PID-reuse answer

The new gone_at_open comment says a listed member is never a non-leader thread because /proc lists thread-group IDs.
The PR description makes the stronger claim that ENOENT cannot reach production.
The directory listing proves the kind of PID at listing time. It does not pin that numeric PID until pidfd_open.
The guard records only the PID. end_members lists members, kills the group, and then calls await_end for each recorded PID.
A parent can reap a listed member. The kernel can then reuse that PID for a non-leader thread before the wait opens its pidfd.
The existing await_end documentation explicitly permits PID reuse during this interval.
The reserve holds the process-group ID. It does not hold every member PID.

Correct the new comment and PR description to preserve this distinction.
ENOENT is not proof that the recorded member is gone. The wait propagates it as an error, including possible PID reuse.
Keep gone_at_open unchanged. This finding requests no runtime change or new fixture.
The reviewer sent F58 directly to P3 and sent its scope to the integration reviewer.

### Accepted logic and kernel evidence

The test holds a non-leader thread on a channel, so its two pidfd_open calls refer to the same live thread.
It accepts exactly the two documented kernel refusals.
For EINVAL, it requires await_end to return Gone.
For ENOENT, it requires await_end to fail with the exact ENOENT errno.
Every other result fails the test. The channel and thread join remain unchanged.
The test adds no sleep, polling loop, process fixture, timer, or allowance.
The guard predicate and the runtime wait code are unchanged.

The reviewer read the kernel author's [2025 patch](https://lkml.iu.edu/hypermail/linux/kernel/2504.1/07385.html).
That patch distinguishes a released task, which returns ESRCH, from a non-leader thread, which returns ENOENT without PIDFD_THREAD.
The reviewer checked [Linux proc enumeration](https://raw.githubusercontent.com/torvalds/linux/v6.12/fs/proc/base.c).
next_tgid selects PIDTYPE_TGID for numeric entries in proc_pid_readdir.
That source supports the listing-time property. It does not prove that the numeric PID cannot be reused before a later open.

The reviewer read the PR's errno audit and the six listed real-kernel sites.
The other listed assertions cover descriptor exhaustion, permissions, missing parents, absent signal targets, absent children, and socket reset.
The reviewer inspected those sites and the constant-errno tests in botster-test-process.
No additional kernel-answer finding arose from that review.

### Completed evidence

Full gate log: `~/botster-sessions/gates/botster-core-stage1-p3-kernel-errno-1bfc6fd3-pool-20261009-091001-91169.log`.
It names this exact head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 922 tests in 1.848 seconds.
The slow tier passes 243 tests in 10.131 seconds, including the renamed test in all seven binaries.
This kernel run covers the EINVAL answer.
Both mutation commands explicitly report INFO No mutants to filter.
The diff changes only test code and comments. The reviewer accepts that result from the diff and actual tool output.
The separate command uses NEXTEST_PROFILE=slow. Signals scan 153 Rust files. Timers scan 137 Rust files.
Full CI takes 165.3 seconds. Separate mutants take 0.4 seconds. The combined job exits 0 after 232 seconds on msa1.

Focused run log: `~/botster-sessions/gates/botster-core-stage1-p3-kernel-errno-1bfc6fd3-pool-20261009-091500-99554.log`.
It names the same exact head and base and runs on gaming with kernel 6.18.40.1-microsoft-standard-WSL2.
The focused slow_process test passes: one passed, zero failed, and 23 filtered out.
The earlier 5099dca7 log from gaming shows this same test's old expectation failing with errno 2 instead of errno 22.
The focused pass, kernel identity, earlier observed ENOENT, and inspected branch logic support the ENOENT proof.
This focused run is not a second full gate. The reviewer accepts both kernel proofs.

### Verdict and scope

PR #186 is NOT CLEAN at this exact head for F58 LOW only.
The reviewer accepts the test logic and completed evidence. No package runtime finding exists.
This is the first NOT CLEAN round for #186. The round-limit notice is not due.
The integration reviewer controls its own verdict.
#184 retains round 110 CLEAN at its exact head and scope.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN

## Round 112 — Kernel test documentation and final base merge

Reviewed head: `1a07efc3c54bca0e8e89db1fab21eb746fc0eca7`, PR #186, branch `stage1/p3-kernel-errno`.
Base: `465978d67ff620cedc21d50a20fc193fb8397e9a`.
Merge parents: `0f559acd2f0ae714774a4bd0fa42e8043b4f137a` and the base above.
The reviewer inspected the complete F58 correction, the two-file base change, the final PR diff, and completed evidence.
Authority: BUILD.md at contracts main 56bd0a5347a537d25bbee65a67854e0e317a9b9a and the lead's process-control-test ruling.
The reviewer checked the tier first. HIGH remains correct under that ruling.
HIGH requires package and integration reviews, and every finding must close.

### F58 is CLOSED

The corrected gone_at_open comment explains that /proc lists thread-group IDs but the guard retains only numeric PIDs.
It states that a parent can reap a listed member and a non-leader thread can reuse its PID before pidfd_open.
It keeps ENOENT as an error instead of treating it as proof that the member is gone.
The complete PR description gives the same explanation and removes the claim that ENOENT cannot reach the guard.
The predicate and executable guard code remain unchanged.
The documentation correction at 0f559acd changes only the comment after the previously reviewed code head 1bfc6fd3.
No package finding remains on the test or documentation.
Round 111's accepted test logic, errno audit, and kernel-source review remain in force.

### Base merge and gate rulings

The gate at 0f559acd failed only at lists because its pending file restored 19 IDs removed from the newer v1 base.
It stopped after that step and did not count as a full gate.
P3 then merged v1 465978d6. The base change affects two paths outside this PR's guard-platform path.
conformance/core-pending.txt removes 19 IDs. worker_transcripts.rs removes three duplicate runs and updates its explanation.
The conformance harness now covers those three removed runs under the lead's revision 23a rule.
The reviewer read merged #185's full description and its passing TestkitHarness proof for the 19 removed IDs.
The reviewed base change introduces no guard or pidfd interaction.

The final description includes the full base-merge-check report.
It proves forward ancestry, no merge conflict, disjoint base paths, and an identical PR diff.
The reviewer independently compared the two PR diffs as read-only git output.
The canonical diff is byte-identical: 4550 bytes, SHA256 `46fe8b56194a9dd543de8fe98cf1007cc79a473141d22e9295c623cd36dabf61`.
The reviewer inspected the base change because the preceding head had no CLEAN verdict.
This review does not rely on the after-CLEAN exception to omit a needed delta review.

The lead ruled that comment-only heads have no gate exception. The final head has its own completed full gate.
The lead also ruled that the kernel proofs at 1bfc6fd3 carry forward because the executable code is unchanged.
The lead corrected its record: round 111 remains NOT CLEAN, and only its accepted logic and proofs carry forward.
This round provides the package CLEAN for the final head.

### Completed evidence

Final log: `~/botster-sessions/gates/botster-core-stage1-p3-kernel-errno-1a07efc3-pool-20261009-092248-21180.log`.
It names this exact head and base. The Linux run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 941 tests in 2.146 seconds.
The slow tier passes 243 tests in 10.115 seconds, including the renamed pidfd test in all seven binaries.
Signals scan 153 Rust files. Timers scan 137 Rust files.
Both mutation commands explicitly report INFO No mutants to filter.
The PR changes only test code and comments, which supports that result.
The separate command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 26.4 seconds. Separate mutants take 0.3 seconds. The gate exits 0 after 33 seconds on msa1.

The EINVAL proof at 1bfc6fd3 used the full gate on kernel 6.12.111+deb13-amd64.
The ENOENT proof at 1bfc6fd3 used the focused slow_process test on gaming, kernel 6.18.40.1-microsoft-standard-WSL2.
Both completed logs and their scopes remain recorded in round 111 and the final PR description.
The new final gate also selects and passes the EINVAL test. The earlier ENOENT proof carries forward under the lead's ruling.
The reviewer read the complete corrected final description, including the head, base, gate, merge-check output, and proof history.

### Verdict and scope

PR #186 is CLEAN at `1a07efc3c54bca0e8e89db1fab21eb746fc0eca7` for the P3 package review.
F58 is closed. No package finding remains open for this change.
The integration reviewer controls its own verdict and must cover this final head.
This package CLEAN covers only this exact head, base, and completed evidence. It covers no later source or base merge.
#186 had one NOT CLEAN round before this CLEAN. The round-limit notice is not due.
#184 retains round 110 CLEAN at its exact head and scope.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN

## Round 113 — EV-4 leaves the pending list

Reviewed head: `489285047b18f1d736a35e2a4adbda0f29f7903c`, PR #189, branch `stage1/p3-flip-ev4`.
Base: `a14e9dc2b61a5426485f9c7f0f829c900e03bdc7`.
Merge parents: `5409483b9a7283da6346adfc794f0cd0f01beec7` and the base above.
The reviewer inspected the full two-file diff, the merge conflict resolution, the conformance harness, the pinned transcript, and completed evidence.
Authority: BUILD.md Risk tiers, the lead's pending-removal ruling, and plan revision 23a at 48c14ab8dd40341295b004b1dfc679eb27207a52.
The contracts source is contracts-v0.1.19, as the reviewed workspace specifies.

### F59 — HIGH — Wrong STANDARD tier; CLOSED in this round

The reviewer checked the stated tier first and found STANDARD incorrect.
The PR changes worker_transcripts.rs in the shared testkit crate, so BUILD.md rule 3 makes the PR HIGH.
The lead's exception applies only to a PR that removes IDs from core-pending.txt and changes nothing else.
The reviewer sent F59 directly to P3 and notified the integration reviewer.
P3 accepted F59 and corrected the complete description to HIGH under rule 3 at the same source head.
P3 requested integration review. The final description names both required reviews.
The reviewer read and accepted the corrected description before recording this verdict.
F59 is closed. The correction changed no source code or gate input.

### Source and proof coverage

Only conf::ev_4_exit_has_signal leaves core-pending.txt.
The corresponding duplicate run leaves worker_transcripts.rs, and its comments remove EV-4.
The conformance harness now runs that ID as an active trial through TestkitHarness.
The harness uses the same pinned transcript, driver_for, schemas, and run_transcript function as the former duplicate run.
The default CI environment supplies seeds 0-31.
The pinned transcript checks an exit signal of 15 with no exit code and an exit code of 3 with no signal.
The replacement map classifies this ID as core-testkit+edge, with the process edge, rather than slow.
Under plan 23a, its passing TestkitHarness trial permits the ID to leave pending.
This does not claim that RealCoreHarness has run the ID or that a real signal proof is replaced.

The merge conflict resolution preserves both parents' removals.
The final IDS contains only a2_8_terminal_identity_names_the_entry and lc_3_remove_created.
Both remaining IDs are still pending. The clause line retains TI-1, LC-3, and LC-7.
The resolution preserves v1's revision 23a explanation and the real-only exception.
No extra ID leaves pending, and no required duplicate run remains after the active trial replaces it.
The diff changes no runtime machine, process adapter, conformance runner, seed set, or mutation exclusion.
No new real-process fixture, timer, sleep, polling loop, or gate-decision function is added.
No package source finding remains.

### Completed evidence

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-flip-ev4-48928504-pool-20261009-093335-59628.log`.
It names this reviewed head and base and runs on msa1, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The active conformance report gives 20 passed and zero failed.
The default tier selects conf::ev_4_exit_has_signal and passes it in 0.166 seconds.
The default tier passes 942 tests in 2.265 seconds. The slow tier passes 243 tests in 10.117 seconds.
Signals scan 153 Rust files. Timers scan 137 Rust files.
Both mutation commands explicitly report INFO No mutants to filter.
The changed Rust code is test-only. The reviewer accepts that result from the diff and actual tool output.
The separate command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 161.7 seconds. Separate mutants take 0.4 seconds. The gate exits 0 after 228 seconds on msa1.
The corrected complete PR description names the exact head, gate, tier, conflict resolution, and replacement-map classification.

### Verdict and scope

PR #189 is CLEAN at `489285047b18f1d736a35e2a4adbda0f29f7903c` for the P3 package review.
F59 is closed. No package finding remains open for this change.
HIGH requires the integration review. The integration reviewer controls its own verdict.
This package CLEAN covers only this exact head, base, and completed evidence. It covers no later source or base merge.
#189 has no recorded NOT CLEAN round. The round-limit notice is not due.
#186 retains round 112 CLEAN at its exact head and scope.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 114 — EV-4 review after the #187 base merge

Reviewed head: `a5d64425bc57f0115598265794c49c7d7f82ce2c`, PR #189, branch `stage1/p3-flip-ev4`.
Base: `8bc21dd516f20f26002f99683b63c6a11adf0d43`.
Merge parents: `489285047b18f1d736a35e2a4adbda0f29f7903c` and the base above.
Authority: BUILD.md Risk tiers, the lead's pending-removal ruling, and plan revision 23a at 48c14ab8dd40341295b004b1dfc679eb27207a52.
The reviewer checked the tier first. HIGH remains correct under rule 3 because the PR changes the shared testkit crate.
F59 remains closed. The complete final description retains HIGH and requests both reviews.

### Merge review and source coverage

The base-merge-check reports FAIL. Both changes touch conformance/core-pending.txt, and the canonical diff has different blob indexes.
The no-conflict check passes. The reviewer does not apply the exemption for a base merge with a passing check.
The reviewer inspected all three imported paths, the final two-file PR diff, the completed gate, and #187's complete description.
The merge preserves EV-4's removal and #187's AM-3, LC-7, and OR-2 removals.
The PR's own pending change still removes only conf::ev_4_exit_has_signal.
The worker_transcripts.rs blob is unchanged from round 113.
Its two remaining IDs, clause line, revision 23a explanation, and real-only exception remain intact.
The EV-4 transcript, harness, driver, schemas, and default seed set remain unchanged.
Round 113's source coverage and replacement-map classification therefore remain applicable.

The imported host change retains the link after process exit during Remove's AwaitTeardown phase.
The host can then read an already-written RemoveResult before LinkClosed.
When the result is absent, link closure after worker exit permits teardown to continue with OutcomeUnknown.
When link closure occurs first, the host waits for worker exit unless the removal grace expires.
A worker exit before the host asks for teardown still completes removal without a RemoveResult.
The new tests cover results of Deleted and Unknown after process exit, and exit before the teardown request.
The extended test verifies that link closure alone does not complete removal while the worker remains alive.
These conditions apply to Remove. They do not change the ordinary process-exit path used by EV-4.
No package interaction finding remains.

The reviewer read #187's completed HIGH evidence and its accepted TestkitHarness scope.
That change varies the order of ProcessExited, link messages, and LinkClosed in the World and TestkitHarness tests.
It changes no OS adapter, socket read, reaper, or worker path.
The lead's hold on new process fixture code remains in force until P6 PRB lands.
The imported AM-3, LC-7, and OR-2 IDs are not real-only IDs under revision 23a.
#187 reports all ten CI steps passing, 946 default tests, 243 slow tests, and five caught mutants with no survivors.
This package review checks the imported source and its interaction with #189. It does not replace #187's recorded reviews.

### Completed evidence

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-flip-ev4-a5d64425-pool-20261009-094159-89893.log`.
The log names the reviewed head and base. The Linux run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 947 tests in 2.352 seconds.
The slow tier passes 243 tests in 10.126 seconds.
The active conformance report gives 23 passed and zero failed.
The report retains 589 pending IDs, 59 IDs without transcripts, two deferred IDs, and two withdrawn IDs.
The pending total is 648. The run list contains 23 IDs.
EV-4 passes in 0.167 seconds. AM-3 passes in 0.157 seconds.
LC-7 passes in 0.149 seconds. OR-2 passes in 0.141 seconds.
Both new host tests pass. Signals scan 153 Rust files. Timers scan 137 Rust files.
Both mutation commands explicitly report INFO No mutants to filter.
The PR's own changed Rust code remains test-only. Those mutation commands cover the PR diff from the new base.
The imported host change has separate mutation evidence in #187; this result does not exempt that source from mutation testing.
The separate command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 28.8 seconds. Separate mutants take 0.4 seconds. The gate exits 0 after 37 seconds on msa1.
The complete final description names the head, base, HIGH tier, gate, and full failing base-merge-check output.

### Verdict and scope

PR #189 is CLEAN at `a5d64425bc57f0115598265794c49c7d7f82ce2c` for the P3 package review.
F59 remains closed. No package finding remains open for this change.
The integration reviewer controls its own verdict and must review this final head because the base-merge-check fails.
This package CLEAN covers only this exact head, base, and completed evidence. It covers no later source or base merge.
#189 has no recorded NOT CLEAN round. The round-limit notice is not due.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 115 — EV-4 review after the contracts-v0.1.20 base merge

Reviewed head: `f2f667f90614532b2ec2403dbf01e88d3193f42a`, PR #189, branch `stage1/p3-flip-ev4`.
Base: `dd07eabd3fb5801ff8438f32d08924a7f440ddfa`.
Merge parents: `a5d64425bc57f0115598265794c49c7d7f82ce2c` and the base above.
Authority: BUILD.md Risk tiers, the lead's pending-removal ruling, and revision 23a at 48c14ab8dd40341295b004b1dfc679eb27207a52.
The reviewer checked the tier first. HIGH remains correct under rule 3 for the shared testkit change.
F59 remains closed. The complete final description states HIGH and requests both reviews.

### Merge review and contracts coverage

The base-merge-check reports FAIL because both changes touch core-pending.txt and the canonical diff has different blob indexes.
The merge has no conflict. The reviewer does not apply the exemption for a base merge with a passing check.
The proposed set rule is not part of this head. The reviewer applies the current merge rule.
The reviewer inspected all three imported paths, the contracts tag delta, and the final two-file PR diff.
The reviewer also read both complete PR descriptions, the lead's #188 merge record, and completed evidence.

The base imports #188's pin update from contracts-v0.1.19 to contracts-v0.1.20 at 03891658e793e5400ba46b5bc003b5d9f952f5e2.
All six workspace dependencies use the new tag. All nine contracts package source entries in Cargo.lock use the new commit.
The other imported changes update two A15 reason comments. They change no pending ID.
The contracts delta changes no crate API, runner implementation, frozen contract, ledger, or replacement-map entry.
Its only crate change adds test vocabulary for five controls.
The tag adds two transcripts: conf::dp_3_frame_limit_checked_before_allocation and conf::ou_9_attach_is_sync_baseline_in_pump.
Both remain pending in Core, as the lead requires until P4a's controls exist.
R-37 and the control documentation define their allocation and handoff observers. This merge adds no observer implementation.
The amendment candidates in the tag do not change the frozen contracts.
BUILD.md in the tag includes the risk rules already in force for this review.

The EV-4 transcript is byte-identical across the two contracts tags.
Its replacement-map entry remains core-testkit+edge with the process edge and the same real-signal note.
Revision 23a therefore still permits the passing TestkitHarness trial to remove EV-4 from pending.
The final PR diff still removes only EV-4 and its duplicate worker transcript run.
The worker_transcripts.rs blob remains unchanged from rounds 113 and 114.
All prior pending removals remain preserved. The runtime, harness, driver, schemas, and seed set remain unchanged.
No new process fixture or timer is added. No package interaction finding remains.
The reviewer accepts this merge without replacing #188's recorded P5 and integration reviews.
The lead records P5 CLEAN 88884ebd and integration CLEAN 8e44ba28 for #188 at b7980e6d27d2fb4ad25b2575cac1326cdff22014.
The reviewer also read #188's passing ten-step gate summary at that head.

### Completed evidence

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-flip-ev4-f2f667f9-pool-20261009-094813-6570.log`.
The log names the reviewed head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 947 tests in 2.536 seconds.
The slow tier passes 243 tests in 10.135 seconds. EV-4 passes in 0.186 seconds.
Conformance reports 23 passed and zero failed, 591 pending, and 57 without transcripts.
The pending total remains 648. Two IDs remain deferred, and two remain withdrawn.
The two new transcripts account for the shift from IDs without transcripts to pending IDs with transcripts.
Signals scan 153 Rust files. Timers scan 137 Rust files.
Both mutation commands explicitly report INFO No mutants to filter for the PR's own test-only Rust diff.
The separate command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 57.3 seconds. Separate mutants take 0.4 seconds. The gate exits 0 after 70 seconds on msa1.
The complete final PR description names the head, base, HIGH tier, gate, history, and full failing merge-check output.

### Verdict and scope

PR #189 is CLEAN at `f2f667f90614532b2ec2403dbf01e88d3193f42a` for the P3 package review.
F59 remains closed. No package finding remains open for this change.
The integration reviewer controls its own verdict and must cover this final head because the base-merge-check fails.
This package CLEAN covers only this exact head, base, and completed evidence. It covers no later source or base merge.
#189 has no recorded NOT CLEAN round. The round-limit notice is not due.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 116 — Pending line-set rule in base-merge-check

Reviewed head: `5ee3da3ee11e3675978ea51c3c31679962a84431`, PR #190, branch `stage1/p3-pending-set-merge`.
Base: `3000ae14bf8b05efe410c22ac66d5915cf36ff45`.
Merge parents: `83df54b0488187eb2cd6ee6ad3e4d3faee07cab3` and the base above.
The reviewer checked the tier first. HIGH is correct under BUILD.md rule 1 because base-merge-check decides whether review is required.
The lead approved the set rule instead of the package-file split. P6 approved P3's work in this P6-owned module.
The lead's scope is an explicit path list and pure, disjoint whole-line removals. Anything else must fail.
The reviewer read the full implementation, the changed tests, mutation settings, the complete description, and completed evidence.

### F60 — HIGH — The set rule ignores file mode and type; OPEN

Locations at the reviewed head: xtask/src/base_merge.rs:381-393, 395-404, and 418-427.
Condition (3) excludes each SET_FILES path from the complete patch.
Condition (4) reads that path's blob text with cat-file and stores only Option<String>.
The code does not retain or compare the tree entry's mode or type.
The name-only path list and set-file overlap exception do not restore that information.

A concrete source counterexample starts from a passing merge of two disjoint pending-ID removals.
Keep the correct merged text. Add a commit that changes only core-pending.txt's mode from 100644 to 100755.
The new head still descends from the reviewed head. The new merge base still descends from the old merge base.
The merge-tree check still compares the reviewed head and new merge base, so the extra commit does not change its result.
Condition (2) excludes the pending path. Condition (3) excludes the pending patch, including the mode change.
Condition (4) sees the same four text values and passes.
The command therefore reports PASS for an unreviewed change outside the approved whole-line-removal rule.
A blob's mode can also identify a symlink; text equality alone does not prove a regular pending file.

Required correction: retain each set-file entry's mode and type at all four commits.
Permit the set rule only for regular files with unchanged metadata, or the existing all-absent case.
A mode or type change on either side or at the final head must fail.
Add a real-repository regression that changes only the pending entry's mode after a valid merge and requires FAIL.
Cover mode and type changes on the two sides as well.
The reviewer sent F60 directly to P3 and the integration reviewer.
The reviewer established this counterexample from source inspection and ran no test or gate.

### Other source coverage

SET_FILES contains only conformance/core-pending.txt.
The pure text decision rejects duplicate ID lines in the old file, additions, edits, reordered lines, and removed non-ID lines.
It requires disjoint removed sets and compares the new text with the exact old-order union of removals.
Partial file absence fails. Absence at all four commits passes.
The command collects text for every configured set path before it calls judge.
The original ancestry, forward-base, conflict, and canonical-patch conditions remain in the decision.
The command removes all four GIT_*_PATHSPECS variables before it invokes git.
The new real-repository tests cover a valid two-flip merge, an overlapping removal, and extra text changes after a merge.
The hostile GIT_LITERAL_PATHSPECS case requires the check to reject an extra change to another path.
The description reports red-on-revert runs for the set-path list and environment removal.
No new mutation exclusion is added. The existing command-only whole-body exclusion leaves the decision functions exposed.
The change adds no production process behavior or new real-process fixture.
No other package finding remains after this source review.

### Completed evidence and its limit

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-pending-set-merge-5ee3da3e-pool-20261009-095726-42960.log`.
The log names this head and base. It runs on msa1, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 955 tests in 2.607 seconds.
The slow tier passes 243 tests in 10.130 seconds. The new set-rule tests run and pass in the default tier.
Both mutation commands report 32 caught, zero missed, zero timeout, and zero unviable.
The separate command uses NEXTEST_PROFILE=slow.
Full CI takes 233.5 seconds. Separate mutants take 118.8 seconds. The gate exits 0 after 360 seconds.
The passing gate and caught mutants do not cover F60's omitted file metadata.
The complete final description states the correct tier, head, base, gate, prior failed gate, and reported revert checks.

### Verdict and scope

PR #190 is NOT CLEAN at `5ee3da3ee11e3675978ea51c3c31679962a84431` for the P3 package review.
F60 is open. This is #190's first recorded NOT CLEAN round; the round-limit notice is not due.
The integration reviewer controls its own verdict.
#189 retains round 115 CLEAN at f2f667f90614532b2ec2403dbf01e88d3193f42a; the lead records its merge as 3000ae14.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 117 — Real-proof references for active real-only Core IDs

Reviewed head: `d34a629d9078aadcc63012adfe2bf47222c2a921`, PR #192, branch `stage1/p3-real-proofs`.
Base: `3000ae14bf8b05efe410c22ac66d5915cf36ff45`.
Merge parents: `15a362ae21c9a8fde682582af95483fe426341fb` and the base above.
The reviewer checked the tier first. HIGH is correct under BUILD.md rule 1 for the lists decision.
Rule 3 also applies to the workspace mutation settings. HIGH requires both reviews and closure of every finding.
Authority: BUILD.md and revision 23a's real-only exception, with the lead's requirement for named real proofs of non-pending slow:* IDs.
The reviewer read all five changed files, their full diff, the lists command context, the named real-test source, and completed evidence.
The reviewer also read the complete PR description and its interim-model agreement with P6.

### F61 — HIGH — The resolver accepts helpers and invented test paths; OPEN

Location at the reviewed head: xtask/src/real_proofs.rs:143-160, with the tier decision at 79-80.
The resolver drops the module prefix and searches for fn <leaf>( in every tracked Rust file under the binary's crate tests/ directory.
The resolver does not establish that the function has a test attribute or belongs to the named binary's module tree.
It does not check cfg predicates or ignore attributes.
The tier decision then uses the supplied binary and test strings, not a resolved test's actual identity.

The existing tree provides a concrete false-pass entry:
- id = conf::lc_2_data_dir_is_exclusive
- binary = slow_real_core
- test = config

crates/botster-core/tests/slow_real_core.rs defines config as a plain helper without a test attribute.
The source needle matches, the binary name satisfies the slow filter model, and the ID has slow:data-dir-lock in the replacement map.
The entry satisfies verdict although no nextest test named config exists.
An entry naming no_such_module::a_second_open_is_refused_until_the_first_is_dropped also passes by its leaf alone.
A fabricated slow_ module prefix can similarly classify a default-tier leaf as slow.
A function in an unrelated test file, a disabled test, an ignored test, or matching text in a comment can also satisfy the lookup.
The tests currently cover a missing leaf and another crate; they do not cover these false-pass cases.

Required correction: resolve an actual reachable test by its exact binary and complete nextest path.
Establish that the test is selected and not ignored in a configuration used by the slow tier.
The interim supported target set may stay narrow, but unsupported cases must fail rather than return a text-based pass.
Use P6's shared resolver when available, or provide a sound interim resolver for the supported cases.
Add regressions for a helper, wrong module, unrelated test file, disabled or ignored test, and fabricated slow_ prefix.
The reviewer sent F61 directly to P3 and integration. This finding follows from source inspection; the reviewer ran no test or gate.

### F62 — LOW — Probe evidence names the wrong head; OPEN

Location: the complete PR description's Proof section and real-tree table.
The section states that its probes ran at this head, but the unchanged row reports pending 652 and to run 19.
The exact-head gate reports pending 648 and to run 23.
The edited rows also retain the older counts of pending 651 and to run 20.
These rows cannot describe the unchanged tree at the stated final head.

Required correction: identify the actual probe head and explain the carry-forward scope, or supply completed probes at the final head.
Do not label earlier output as exact-head output.
The reviewer sent F62 directly to P3 and integration. HIGH requires this LOW finding to close before CLEAN.

### Other source coverage

The proof file starts empty. No active Core ID has a slow:* entry in the pinned replacement map at this head.
The parser rejects unknown keys, wrong TOML shapes, and missing string fields.
The verdict rejects duplicate IDs, IDs outside the Core ledger, and entries without a slow:* classification.
An entry for a pending real-only ID is permitted so that its real test can land first.
The running set excludes pending, deferred, and withdrawn IDs from the Core ledger.
The lists command reads the pinned replacement map, the proof file, and tracked Rust sources before it reports problems.
The new mutation exclusion matches only the command's whole-body Ok(()) replacement and names the tested decision functions.
The decisions remain mutation-tested. No production process behavior or new real-process fixture is added.
The P6 agreement plans to replace the interim resolver with mutants_cited::tests and tiers_of when PR B and this change have both landed.
That future replacement does not close F61 at the current head.
No other package finding remains after this source review.

### Completed evidence and its limit

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-real-proofs-d34a629d-pool-20261009-100347-51753.log`.
The log names this head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 958 tests in 2.497 seconds.
The slow tier passes 243 tests in 10.129 seconds. All eleven new real_proofs tests run and pass.
The lists report gives 675 ledger IDs, 648 pending, two deferred, two withdrawn, and 23 to run.
Both mutation commands report 31 tested: 30 caught, zero missed, zero timeout, and one unviable.
The separate command uses NEXTEST_PROFILE=slow.
Full CI takes 148.7 seconds. Separate mutants take 109.8 seconds. The gate exits 0 after 267 seconds.
These passing checks do not cover F61's false-positive definition lookup.
The complete final description names the correct gate head and base. Its earlier probe claims require F62's correction.

### Verdict and scope

PR #192 is NOT CLEAN at `d34a629d9078aadcc63012adfe2bf47222c2a921` for the P3 package review.
F61 and F62 are open. This is #192's first recorded NOT CLEAN round; the round-limit notice is not due.
The integration reviewer controls its own verdict.
Under the lead's report ruling, these normal findings go only to P3 and integration. No lead decision is needed to continue.
#190 retains round 116 NOT CLEAN with F60 open. P3 reports a fix at 5a0b07ac with its gate running; that fix is not reviewed here.
#189 retains round 115 CLEAN at f2f667f90614532b2ec2403dbf01e88d3193f42a and merged base 3000ae14.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 118 — Set-file metadata and one-sided merge rules

Reviewed head: `5a0b07ac0575743977c5d45ae78ae0a6a55ec101`, PR #190, branch `stage1/p3-pending-set-merge`.
Base: `3000ae14bf8b05efe410c22ac66d5915cf36ff45`.
Parent: `5ee3da3ee11e3675978ea51c3c31679962a84431`.
The reviewer checked the tier first. HIGH remains correct under BUILD.md rule 1 for gate-decision code.
Authority remains BUILD.md and the lead's approved set rule, with P6's agreement for this P6-owned module.
The reviewer inspected the complete correction and the full two-file change against the base.
The reviewer read the full corrected description and completed exact-head evidence.

### F60 — HIGH — CLOSED

The check now reads the tree entry's mode and type with its text at all four commits.
Blob retains the mode/type string from ls-tree. A non-blob entry remains present and reaches the decision with its metadata.
judge_set requires 100644 blob at the reviewed merge base, reviewed head, new merge base, and new head.
The metadata check runs before either the one-sided branches or the set-text decision.
An executable file, symlink, gitlink, or tree fails at any position. Partial absence still fails.
The all-absent case remains valid.

The default test a_set_file_that_is_not_a_regular_file_at_any_commit_fails covers all four rejected kinds at all four positions.
The real-repository regression a_real_mode_change_of_the_set_file_after_the_merge_fails starts from a valid two-flip merge.
It adds an executable-mode change without changing the pending text and requires FAIL (4) at the new head.
The exact-head gate selects and passes both tests.
The description reports that removing the metadata check makes both tests fail.
The new input and decision reject round 116's false-pass counterexample. F60 is closed.

### Whole-change and integration R1 coverage

The correction also restores the original byte rule when only one side changes the set file.
If the reviewed head leaves the file unchanged, the new head must equal the base's file.
If the base leaves it unchanged, the new head must equal the reviewed file.
Equality covers metadata and text. The regular-file check still applies first.
A comment edit or an already-reviewed addition on the only changed side can therefore carry forward unchanged.
An additional final-head edit fails. Changes on both sides still require the strict set rule.
The one-sided unit test covers both branches and incorrect results.
The real-repository test covers a PR that leaves pending unchanged while the base edits its comment.
The reviewer accepts this correction for the package scope. Integration controls R1's closure and its own verdict.

SET_FILES still contains only conformance/core-pending.txt.
The two-sided text rule still rejects duplicate IDs, additions, edits, reordered lines, removed non-ID lines, overlap, and incorrect merged text.
Every configured path still reaches condition (4).
The ancestry, forward-base, conflict, overlap outside SET_FILES, and canonical-patch decisions remain in force.
The command still removes all four GIT_*_PATHSPECS variables.
The hostile-pathspec and extra-final-change regressions remain selected.
No mutation exclusion changes. The decision functions remain exposed to mutation testing.
The change adds no production process behavior or new real-process fixture.
No package finding remains.

### Completed evidence

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-pending-set-merge-5a0b07ac-pool-20261009-100828-95489.log`.
The log names this head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 959 tests in 2.408 seconds.
The slow tier passes 243 tests in 10.117 seconds.
The new metadata, mode-only, and one-sided tests run and pass in the default tier.
Signals scan 153 Rust files. Timers scan 137 Rust files.
The lists report gives 675 ledger IDs, 648 pending, two deferred, two withdrawn, and 23 to run.
Both mutation commands report 39 caught, zero missed, zero timeout, and zero unviable.
The separate command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 154.6 seconds. Separate mutants take 133.5 seconds. The gate exits 0 after 296 seconds.
The complete final description names the correct tier, head, base, source corrections, tests, revert checks, and gate history.

### Verdict and scope

PR #190 is CLEAN at `5a0b07ac0575743977c5d45ae78ae0a6a55ec101` for the P3 package review.
F60 is closed. No package finding remains open for this change.
The integration reviewer controls its own verdict and its R1 finding.
This package CLEAN covers only this exact head, base, and completed evidence. It covers no later source or base merge.
#190 had one recorded NOT CLEAN round before this CLEAN. The round-limit notice is not due.
#192 retains round 117 NOT CLEAN at d34a629d9078aadcc63012adfe2bf47222c2a921 with F61 and F62 open.
P3 accepted those findings and plans to use P6's resolver after #181 lands, with new regressions and corrected probe evidence.
#189 retains round 115 CLEAN at its named head and merged base 3000ae14.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 119 — Testkit PTY program controls

Reviewed head: `46642d85de52c98ce8fe1cfbd0bf7afa27847c4d`, PR #195, branch `stage1/p3-pty-controls`.
Base: `1dd1657a2c53f4da6953f7a349d7fa9d59b97ec6`.
Parent: `3b43213afd7268d755b89eb4f122da2e598f80a2`.
The reviewer checked the tier first. HIGH is correct under BUILD.md rule 3 because this changes the shared testkit.
The lead assigned the work. The description records P5 and P6 agreement on the design.
Authority: BUILD.md, plan 2.1, the pinned program-control documentation, and Core A5-1, A5-2, A5-3, ST-1, OU-7, and TM-6.
The reviewer read the full six-file diff, control parsing, program read/write behavior, worker lifecycle, and existing block test.
The reviewer also read the complete description, probe scope, and completed exact-head gate.

### F63 — MEDIUM — program_edge reverses the process lock order; OPEN

Location at the reviewed head: crates/botster-core-testkit/src/worker.rs:234-242.
program_edge locks the ProcessCell and keeps its MutexGuard alive while it locks the owner's Processes table to clone the wake.
Existing process end takes these locks in reverse order.
Processes::end at lines 102-108 locks the cell while the caller holds the owner table.
WorkerEdges performs Action::Exit through this path at line 616. WorkerSpawner also uses it for GroupSignal::Kill at line 346.
ProcessTable::holds_reports at lines 88-96 likewise holds the table while locking its cells.

CoreApi is Send under Core TH-1. A caller can move its Core to a pumping thread while retaining the harness on another thread.
A program control on the harness can therefore run concurrently with worker exit during a pump.
If the control holds the cell and exit holds the owner, exit waits for the cell while the control waits for the owner.
Both threads stop. The current single-thread control tests do not exercise this lock cycle.

Required correction: release the cell guard after validation and cloning ProgramControl, before reading the owner's wake.
break_link already limits the cell guard to a separate scope before it accesses the owner.
Add a meaningful regression for a program-edge control concurrent with process end, without an unbounded wait.
The reviewer sent F63 directly to P3 and integration.
This finding follows from source inspection and the Send contract. The reviewer ran no test or gate.

### Other source coverage and proof limits

The registry dispatches both new controls through the existing parse and session_row helpers.
The handlers reject unknown handles or sessions, absent workers, extra keys, and invalid or empty output hex.
ProgramControl::write appends plain output rather than an atomic piece. The existing program reader keeps the seeded read sizes.
The worker receives Input::PtyOutput through its existing readiness and read path.
The new program handle is set after a successful payload spawn and cleared at reap or worker end.
The controls reject an ended worker or absent payload.

pty_output signals the owning host's wake. pty_blocked defaults to on and signals the wake only on release.
The block uses the existing ProgramControl::set_blocked, which also clears a pty_accept limit on release.
The existing the_handle_blocks_a_program_that_moved test proves WouldBlock and subsequent acceptance at the program edge.
The new wake tests arm no_spurious_wakes before they require a clear idle wake, as #194 requires.
The output test uses eight seeds and proves that five injected bytes become unread and are then read by the worker.
Its unread observation uses the documented program-edge counter, not private worker state.
The description reports revert checks for the write and both wake signals.

The description states both current proof limits.
The testkit has no input-write action, so a harness test cannot observe a blocked write; no transcript counts as proof of pty_blocked.
The M1 worker reports no output to the host and serves no terminal read, so the output test proves only consumption by the worker.
The transcript probes still fail at output Activity or stop at route controls. No pending ID leaves the list in this PR.
A reported pass with the separate output-activity branch is not proof for this reviewed head.
The change adds no real-process fixture or production process code. No other package finding remains.

### Completed evidence and its limit

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-pty-controls-46642d85-pool-20261009-105935-10918.log`.
The log names this head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 979 tests in 2.809 seconds.
The slow tier passes 243 tests in 10.132 seconds. All four new pty_controls tests run and pass.
Signals scan 159 Rust files. Timers scan 143 Rust files.
The lists report gives 675 ledger IDs, 645 pending, two deferred, two withdrawn, and 26 to run.
Both mutation commands report 15 tested: ten caught, zero missed, zero timeout, and five unviable.
The separate command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 240.0 seconds. Separate mutants take 82.1 seconds. The gate exits 0 after 387 seconds.
These passing checks do not cover F63's lock-order cycle.
The complete final description names the correct tier, head, base, gate, tests, and limits.

### Verdict and scope

PR #195 is NOT CLEAN at `46642d85de52c98ce8fe1cfbd0bf7afa27847c4d` for the P3 package review.
F63 is open. This is #195's first recorded NOT CLEAN round; the round-limit notice is not due.
The integration reviewer controls its own verdict. No lead decision is needed to continue.
#190 retains round 118 CLEAN at 5a0b07ac0575743977c5d45ae78ae0a6a55ec101; the lead records its merge as 67fd748a.
#192 retains round 117 NOT CLEAN with F61 and F62 open, awaiting a new exact-head READY after the shared-resolver correction.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 120 — Testkit PTY controls, lock-order correction and merged start controls

Reviewed head: `ab430951006a5aeee2f054b363679940356d9bbb`, PR #195, branch `stage1/p3-pty-controls`.
Base: `1f157c29b3d58e4097d1a0969847cadc2475ec90`.
Parent: `491966a647082dd3cca252e2f985d8393e805c01`.
The reviewer checked the tier first. HIGH remains correct under BUILD.md rule 3 for the shared testkit.
Authority: BUILD.md, including testing rule 5, plan 2.1, the pinned program-control documentation, and the lead's anchor rulings.
The reviewer inspected the correction, the full seven-file PR change against the new base, and the imported #196 start-control change.
The reviewer read the complete final description, reported revert checks, and completed exact-head gate.

### F63 — MEDIUM — source correction accepted; regression proof remains OPEN

The source correction in worker.rs:298-309 is accepted.
program_edge validates the cell and clones ProgramControl inside a separate scope.
The cell guard ends before the owner Processes table is locked to clone its wake.
The previous owner/cell lock cycle is removed.

The new regression, a_program_edge_control_concurrent_with_the_process_end_does_not_deadlock,
uses thread::sleep(Duration::from_millis(50)) at worker/tests.rs:417 to set the thread order.
The preceding deadline marker says this lets the control reach the cell before process end.
That sleep is synchronization by elapsed time, not a deadline on an event.
BUILD.md testing rule 5 forbids sleeps or polling and requires tests to wait on the real event.
A deadline marker does not make this ordering method comply.

The test also permits the control to run only after process end on a slow host, at lines 441-446.
Under that schedule, the old program_edge returns the accepted has-ended error without taking the owner lock.
Both threads finish, so the regression passes with the old lock order.
The reported three failing revert runs show the usual schedule, but do not remove this false-pass schedule.

Required correction: replace the ordering sleep with event coordination or deterministic concurrency exploration
that establishes the relevant lock order. Keep a real completion deadline to bound failure.
Show that the old lock order fails without relying on the OS scheduling the control within 50 ms.
The reviewer sent this remaining proof requirement directly to P3 and integration.
F63 remains open for the regression proof only. No new finding ID is assigned.

### Other source coverage and merged-base scope

The parser, registered dispatch, normal-byte injection, seeded program reads, and host wake paths remain accepted.
The block control still uses the existing ProgramControl block and release behavior.
The existing program-edge test remains the block/acceptance proof; the harness has no input-write action.
The output test proves worker consumption only because the M1 worker does not report output or serve a terminal read.
The description retains these limits. No pending ID leaves the list through #195.

The new output refusal checks payload_alive and returns Bad after payload exit, before injecting bytes.
The eight-seed regression first confirms that program_edge still exists, then requires refusal.
This covers the interval after payload exit and before worker reap.
The merged worker fields retain both program and payload_alive and clear them together at reap or worker end.
No other package finding remains.

The new base imports #196 at its reviewed head 64a6deb5bec9cc52c08aae3376726003ead1920b.
The lead records P5 verdict 3274099a and integration verdict ec24a408 for that HIGH shared-testkit change.
The reviewer inspected the start-key controls, worker lifecycle changes, registry dispatch, and their union with the PTY controls.
The start key includes both data directory and instance. Held starts apply before payload spawn.
The five pending-ID removals belong to the merged base, not this PTY-control change.
The base's completed gate and prior verdicts do not close F63 at the current PR head.
No production process code or real-process fixture is added by #195.

### Completed evidence and its limit

Exact-head log: `~/botster-sessions/gates/botster-core-stage1-p3-pty-controls-ab430951-pool-20261009-112122-4292.log`.
The log names this head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 993 tests in 2.738 seconds.
The slow tier passes 243 tests in 10.118 seconds.
The new output-refusal test passes in 0.010 seconds. The concurrency regression passes in 0.053 seconds.
Signals scan 161 Rust files. Timers scan 145 Rust files.
The lists report gives 675 ledger IDs, 640 pending, two deferred, two withdrawn, and 31 to run.
The conformance run passes all 31 active IDs; 583 pending and 57 without transcripts remain.
Both mutation commands report 18 tested: 13 caught, zero missed, zero timeout, and five unviable.
The separate command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 114.1 seconds. Separate mutants take 113.2 seconds. The gate exits 0 after 236 seconds.
The passing gate does not correct the regression's ordering sleep or its false-pass schedule.

### Verdict and scope

PR #195 is NOT CLEAN at `ab430951006a5aeee2f054b363679940356d9bbb` for the P3 package review.
F63 remains open for the regression proof; its source correction is accepted.
This is #195's second recorded NOT CLEAN round. The round-limit notice is not yet due.
The integration reviewer controls its own verdict. No lead decision is needed to continue.
Normal NOT CLEAN findings go only to P3 and integration under the lead's current reporting rule.
#190 retains round 118 CLEAN and is merged at 67fd748a.
#192 retains round 117 NOT CLEAN with F61 and F62 open, awaiting a new exact-head READY after the shared-resolver correction.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties, including F39 for the merge change.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 121 — Testkit PTY controls, event-based lock-order proof

Reviewed head: `b42f47a64326d391aace2d1bca92a06ca0515853`, PR #195, branch `stage1/p3-pty-controls`.
Base: `1f157c29b3d58e4097d1a0969847cadc2475ec90`.
Parent: `ab430951006a5aeee2f054b363679940356d9bbb`.
The reviewer checked the tier first. HIGH remains correct under BUILD.md rule 3 for the shared testkit.
Authority: BUILD.md, including testing rules 3, 5 and 8, plan 2.1, the pinned program-control documentation, and the lead's anchor rulings.
The reviewer inspected the entire two-file correction and confirmed that no other source or base changed since round 120.
The seven-file PR scope and imported #196 start controls retain the full source coverage recorded in rounds 119 and 120.
The reviewer read the complete final description, reported revert proof, and completed exact-head gate.

### F63 — MEDIUM — CLOSED

program_edge delegates to a private program_edge_between with an empty callback.
The helper validates the cell and clones ProgramControl within the separate cell-guard scope.
It calls the supplied callback after that guard ends and before it locks the owner to clone the wake.
The normal control therefore retains the accepted lock-order correction.
This callback is dependency injection within the testkit fake, not a test branch in production process code.
It changes no production worker, core process edge, environment selection, or public control.

The regression now waits on events instead of a sleep.
The control reads the cell, signals at_owner, and waits for go before taking the owner lock.
Only after at_owner does the test start process end. That thread takes the owner lock, signals owner_held,
and calls Processes::end, which needs the cell. The test waits for owner_held before sending go.
All three recv_timeout sites have marked completion deadlines. No delay chooses the thread order.
The control must return Ok: it reads the cell before process end, so the previous accepted after-end branch is gone.
The test proves the actual F63 lock cycle rather than only a helper result.

With the old cell guard held across the callback and owner lock, process end holds the owner and waits for the cell.
After go, the control holds the cell and waits for the owner. This cycle exists on every schedule that reaches those events.
With the corrected guard scope, process end can acquire the cell and release the owner; both operations finish.
The completion deadline turns a regression into a bounded failure.
The description reports five old-order revert failures at the deadline and a passing corrected test.
The exact-head gate runs the corrected regression in 0.003 seconds.
The source and event order close both the original lock defect and round 120's regression-proof gap.

### Retained whole-change coverage and limits

The registered controls use strict argument parsing and the existing session-row lookup.
They reject invalid handles, sessions, arguments, absent workers or payloads, and invalid output hex.
pty_output checks payload_alive and refuses output after payload exit, including before reap.
Its eight-seed test confirms the program edge still exists in that interval and requires refusal.
Plain output uses the existing seeded program reader and readiness path; the host wake is signalled.
The worker sets the program handle on payload spawn and clears program and payload_alive together on reap or worker end.
pty_blocked uses the existing block edge; release clears the block and acceptance limit and wakes the host.
The wake tests disable seeded spurious wakes before checking an idle wake.
No new lock cycle is introduced by the correction.

The existing program test remains the proof of blocked writes and later acceptance.
The harness still has no input-write action, so no transcript counts as proof of pty_blocked.
The M1 worker still reports no output or terminal read; output proof stops at worker consumption.
The description states both limits and keeps the separate output-activity branch's reported passes outside this head's proof.
No pending ID leaves through #195. The five removals and start controls remain part of merged #196 at the unchanged base.
No production process code, real-process fixture, pin, or mutation exclusion changes in this PR.
No package finding remains.

### Completed exact-head evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p3-pty-controls-b42f47a6-pool-20261009-112955-39079.log`.
The log names this head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 993 tests in 3.427 seconds.
The slow tier passes 243 tests in 10.165 seconds.
All five pty_controls tests and the event-based F63 regression run and pass.
Signals scan 161 Rust files. Timers scan 145 Rust files.
The lists report gives 675 ledger IDs, 640 pending, two deferred, two withdrawn, and 31 to run.
The conformance run passes all 31 active IDs, with 583 pending and 57 without transcripts.
Both mutation commands report 20 tested: 13 caught, zero missed, zero timeout, and seven unviable.
The separate mutation command uses NEXTEST_PROFILE=slow. Fuzz reports no changed crate with a decoder harness.
Full CI takes 138.1 seconds. Separate mutants take 99.9 seconds. The gate exits 0 after 246 seconds.
The complete final description names the correct tier, head, base, proof, limits, and gate.

### Verdict and scope

PR #195 is CLEAN at `b42f47a64326d391aace2d1bca92a06ca0515853` for the P3 package review.
F63 is closed. This covers only the exact head and base above, not a later source change or base merge.
#195 had two NOT CLEAN rounds; no third NOT CLEAN or round-limit notice is due.
The integration reviewer controls its own verdict.
#190 retains round 118 CLEAN and is merged at 67fd748a.
#192 retains round 117 NOT CLEAN with F61 and F62 open, awaiting a new exact-head READY after the shared-resolver correction.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties, including F39 for the merge change.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 122 — Worker PTY output observations and two TM-3 removals

Reviewed head: `cdf0f4d20e39069dd75747d523cdb36ca6b29c06`, PR #197, branch `stage1/p3-output-activity`.
Base: `c06f5b982d05dd65b00661a6cb9bb8b3aeea5514`.
Parent: `3c243a044e762cbdf5a93d4858ea9298b16e09be`.
The reviewer checked the tier first. HIGH is correct: the lead raised it for worker PTY behavior and the HIGH-path scope.
Authority: BUILD.md, plan revisions 23a through 23d at 9b7bc5a0 (pin stage1-plan.e3c60694.md),
Core ST-1, IN-10, ST-5, TM-3 and the Activity event row, plus the lead's current rulings.
The reviewer read the full five-file change, its comment-only correction, worker report and lifecycle paths,
real and testkit driver ordering, host observation handling, both pinned transcripts, replacement-map rows,
the complete final PR description, and completed gate evidence at both heads.

### Source and behavioral proof

Each nonempty Input::PtyOutput advances the worker's model_rev independently of link state.
An empty read changes nothing. The launch terminal state uses the same counter.
The tokens remain opaque to the host; the test's chosen numeric values verify this worker's implementation only.
The host's existing Output observation path updates the terminal revision and output time,
resets the silence state, and posts Activity keyed by instance and source on the injected clock.
The change adds no terminal parser or claimed libghostty terminal semantics.
The terminal-state and CaptureSnapshot duties of M2 remain pending.

While the last Output report is unwritten, further reads keep only the latest revision in the machine.
LinkWritten releases that pending revision only after the cumulative total covers the previous Output frame.
Before another report, report() sends the pending Output first, so Exited and RemoveResult follow prior output.
The existing staged close still waits for all bytes queued before it; Terminate retains its separate immediate close path.
A closed link sends no new output reports. The driver continues to read the PTY.
Real and testkit drivers deliver the spawn answer before they offer the payload's output.
The existing drain path reads the exit tail before PtyDrained and Exited.

The corrected queue claim is accepted. Reads alone queue at most one unwritten Output report.
Flushing before another report can add another Output before the first is written.
The bound is therefore one Output from reads, plus at most one for each other report, not a global one-report queue limit.
The integration reviewer raised the earlier overstatement; the final comment and description state the actual bound.
The existing queue growth from other reports is outside this PR's output-only bound.

The machine tests prove empty/nonempty reads, distinct revisions, partial LinkWritten totals,
coalescing across three further reads, release at the write boundary, pending Output before Exited,
and no send after link closure. The exit-drain test now requires the output tail observation.
The description reports separate failing revert checks for coalescing and flush-before-other-report.

The real-worker lifecycle helper skips Output observations rather than assuming a kernel-dependent report count.
The real driver test drains the host socket until EOF so Remove's staged close has a reader.
The socket clone shares its file description; the description states that making it blocking also affects h.peer.
Subsequent operations on h.peer are writes. The existing real-driver retirement deadline and process guards remain.
These changes consume the host protocol and add no new process launch, group ownership, wait, or cleanup fixture.
The named real-worker exit and PTY readiness tests run and pass at this head.
No new real-process proof is claimed for the two TM-3 IDs, and no real-harness count is increased.

### Pending removals and progress scope

Only conf::tm_3_due_deadline_fires_in_pump and conf::tm_3_wake_handle_ignores_deadlines leave core-pending.txt.
At the reviewed replacement-map commit 2701038, both use core-testkit+edge with the clock edge, not slow-only proof.
Both pinned transcripts first require Activity{Output} after pty_output and then assert the deadline or wake behavior.
The exact-head conformance run passes both under the gate's seed set.
These removals comply with the lead's revision 23a rule and add regression protection now.
Only the first ID belongs to the minimum set: minimum testkit progress is 28/70, up from 27/70; real progress remains 0/70.
conf::tm_3_next_deadline_is_host_armed stays pending for its M2 CaptureSnapshot step.

### Completed exact-head evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p3-output-activity-cdf0f4d2-pool-20261009-114608-35231.log`.
The log names the exact head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 998 tests in 3.053 seconds.
The slow tier passes 243 tests in 10.121 seconds.
The three new machine tests, both newly active TM-3 IDs, both real-worker exit tests,
and pty_events_resume_reads_after_would_block run and pass; the real PTY readiness test takes 0.010 seconds.
Signals scan 161 Rust files. Timers scan 145 Rust files.
The lists report gives 675 ledger IDs, 638 pending, two deferred, two withdrawn, and 33 to run.
All 33 active IDs pass; 581 pending and 57 without transcripts remain.
Both mutation commands report 13 tested: 12 caught, zero missed, zero timeout, and one unviable.
The separate command uses NEXTEST_PROFILE=slow. No mutation exclusion changes.
Fuzz reports no changed crate with a decoder harness.
Full CI takes 42.4 seconds. Separate mutants take 13.4 seconds. The gate exits 0 after 62 seconds.
The final description names the correct tier, final head, base, gate, proof, corrected bound, and progress limits.

### Verdict and scope

PR #197 is CLEAN at `cdf0f4d20e39069dd75747d523cdb36ca6b29c06` for the P3 package review.
No package finding remains. This covers only this exact head and base, not a later source change or base merge.
No NOT CLEAN round is recorded for #197. The integration reviewer controls its own verdict.
#195 retains round 121 CLEAN and is merged at c06f5b98; F63 remains closed.
#192 retains round 117 NOT CLEAN with F61 and F62 open, awaiting a new exact-head READY after the shared-resolver correction.
#168 retains its separate single planned real-PTY HOLD. Part B retains its earlier open duties, including F39 for the merge change.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 123 — PR A, PTY input admission and real write path

Reviewed head: `fc39fee6be5024df41a0cc937417401100c6303b`, PR #198, branch `stage1/p3-pty-input`.
Base: `7aec2bb917f48994d1705301d2383873a219c031`.
Parent: `2a19f9d250e779666919c86f159e63376d009f4c`.
The reviewer checked the tier first. HIGH is correct under BUILD.md rule 3 for shared testkit and workspace configuration,
and for the lead-raised PTY write scope under rule 5.
Authority: BUILD.md, plan 23a through 23d at 9b7bc5a0 (pin stage1-plan.e3c60694.md),
Core AM-2, IN-2, IN-5, IN-6, IN-7 and IN-10, and the lead's explicit M2a real-PTY proof requirement.
The reviewer read all sixteen changed files, the imported legacy test guards, complete PR description,
completed exact-head gate, thirteen pinned transcript cases, and their reviewed replacement-map rows.
The reviewer compared the input admission module with round 101's accepted M2a head 7550018f.

### F64 — HIGH — Required real-PTY proof still uses legacy process guards; OPEN

The new in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write test is in worker/tests/common/session.rs.
At lines 15-19 that module imports the legacy payload_guard and process_guard modules from botster-core-sys/tests/common.
Session::launch uses their PayloadGuard::new and prefix; OwnedWorker retains their ownership and cleanup paths.
The reviewer read those imported files: they implement the legacy group guards, current-test-binary helpers,
and cleanup locally. They do not delegate this ownership to botster-test-process.
botster-worker/Cargo.toml has no botster-test-process dev dependency at this head.

The test does run and pass in both slow binaries. That proves this legacy fixture ran, not that it was rebuilt on the shared crate.
The description and new mutation-exclusion reason incorrectly state that it is under botster-test-process ownership.
The lead's explicit requirement, preserved in round 101, is that this named real-PTY regression be rebuilt on
botster-test-process and pass the gate before the PTY write path merges.
Plan section 6.1 also assigns real-process infrastructure to P6 and prohibits new proof code on legacy infrastructure
before the shared crate and check land. The lead's handoff still records #181 awaiting its final correction gate.
The required ownership part of the proof is therefore not satisfied by this passing gate.

Required correction: rebuild this named regression using the shared crate's ownership and capabilities.
Ask P6 for any missing shared capability; do not copy process guards or cleanup code into this package.
Update the description and exclusion reasons to name the actual fixture, then provide the exact-head selected proof.
This finding retains the existing M2a merge requirement; it is not a new deadline or a request to change product behavior.

### F65 — MEDIUM — New I/O-shell reasons omit the required proof-citation form; OPEN

The two new exclusions for Driver::write_pty_once and Driver::set_pty_write_interest are at .cargo/mutants.toml:127-139.
Their reason text names io_decisions::pty_write, io_decisions::pty_write_interest and the real-PTY proof only in free text.
Plan 23c/23d requires an I/O-shell exclusion to cite at least one selected test in the decision (proof_a, proof_b) form.
A proof named only in free text fails that rule. The current reasons do not use the required form.
The passing pre-#181 gate does not waive the binding plan rule.

Required correction: give each new entry the real decision/proof citation in the required form.
Retain mutation coverage for the pure decision functions and cite only tests selected by a gate tier.
The pure write, interest and writable-event tests do run at this head; the slow proof must also satisfy F64.
The reviewer sent F64 and F65 directly to P3 and integration. No lead decision is needed to continue.

### Whole-change source coverage

The input admission module differs from the accepted M2a module only by removal of its duplicate model_rev
and comparison with the worker's counter from #197. The port keeps start-time guards and FIFO host writes.
Bytes and UTF-8 Text are supported; Paste, Key, Mouse, Focus, route admission and query replies remain later work.
One active transaction owns the PTY input across short writes. One PtyWrite is out at a time.
Cancellation of a queued write completes with exact zero; an active write waits for an outstanding count.
A full PTY waits for PtyWritable rather than retrying without readiness. Counts precede the completion decision.
An OS error gives Failed with prior counts. Payload end gives the certain Partial or zero result once its count is known.
Queued writes perform the guards at their start; host input revision advances after those checks.
The worker uses one model revision for output and the terminal guard.

The real driver keeps one pending write, performs one write per turn after serving the control link,
retains interrupted writes, and reports no progress as Ok(0) with write interest.
The writable-event decision and poll-interest decision use the pure io_decisions functions.
The default tests cover successful, empty, blocked, interrupted, missing-PTY and errno results,
all four writable-event combinations, and registered/unregistered interest transitions.
The new narrow glue exclusions retain the prior M2a classification, subject to F64 and F65.
No gate-decision function is excluded by this PR.

The testkit routes PtyWrite and PtyWritable as separate scheduler inputs.
It uses the existing run process table and cell program handle; it does not restore M2a's separate worker namespace.
The run resets each program's chunk allowance at the pump step, and has_ready retains work for a spent chunk.
The F53 rule survives: an exhausted accept limit or blocked program does not request another chunk step.
Controls use strict parsing and session-row/program lookup. pty_input observes accepted bytes;
pty_chunk, pty_accept and pty_fail_after operate on the program edge, not the worker machine.
Tests cover chunk progress across pumps, block and release, failed-write counts, invalid controls,
combined exhausted limits and input-log order. No additional package source finding is recorded.

The named real regression waits on FIFO markers and link reports, checks a nonzero cancelled prefix,
releases the program to read the prefix and the following byte, and checks the exact received bytes.
Its O_CLOEXEC change prevents inherited FIFO ends from holding the release open.
It ends the payload through Kill and consumes Exited before Remove.
These behaviors are useful, but their fixture ownership remains F64; they do not close the shared-crate requirement.

### Pending removals and completed evidence

The thirteen removals match the description. At replacement-map 2701038, each is core-testkit or core-testkit+perturb.
None is real-only or belongs to the minimum set. The minimum count remains testkit 28/70, real 0/70.
The source supports the claimed host-write, cancel, guard and lane-bound transcript paths.
The route-contiguity minimum ID stays pending for P4a; CaptureSnapshot and terminal-model duties remain pending.
The reported additional AM-4 pass remains pending and outside this PR's thirteen removals.

Log: `~/botster-sessions/gates/botster-core-stage1-p3-pty-input-fc39fee6-pool-20261009-122909-62645.log`.
The log names this head and base. The run uses msa1 with kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 1040 tests in 3.658 seconds.
The slow tier passes 245 tests in 10.119 seconds.
The named real-PTY regression runs in the integration binary in 0.010 seconds and driver binary in 0.012 seconds.
The new testkit behavior tests and pure driver decision tests run and pass.
Signals scan 162 Rust files. Timers scan 146 Rust files.
The lists report gives 675 ledger IDs, 625 pending, two deferred, two withdrawn, and 46 to run.
All 46 active IDs pass; 568 pending and 57 without transcripts remain.
Both mutation commands report 118 tested: 108 caught, zero missed, zero timeout, and ten unviable.
The separate mutation command uses NEXTEST_PROFILE=slow. Fuzz has no changed decoder harness.
Full CI takes 223.0 seconds. Separate mutants take 212.9 seconds. The gate exits 0 after 444 seconds.
The passing results do not close F64's ownership requirement or F65's citation requirement.

### Verdict and scope

PR #198 is NOT CLEAN at `fc39fee6be5024df41a0cc937417401100c6303b` for the P3 package review.
F64 HIGH and F65 MEDIUM are open. This is #198's first recorded NOT CLEAN round; no round-limit notice is due.
Normal findings go to P3 and integration, not to the lead as BLOCKED. Integration controls its own verdict.
#197 retains round 122 CLEAN and is merged at 7aec2bb9. #195 retains round 121 CLEAN with F63 closed.
#192 retains round 117 NOT CLEAN with F61 and F62 open, awaiting a new exact-head READY after the shared-resolver correction.
#168's named real-PTY merge requirement remains unsatisfied by this legacy-fixture proof.
Part B retains its earlier open duties, including F39 for the merge change.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings, closures, and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 124 — PR #198 PTY input fixture correction — 2026-10-09

Reviewed head: `8a762ac00a354b032432a5a26963d93c91e0284c`.
Base: `7aec2bb917f48994d1705301d2383873a219c031`.
Parent: `02062f1bf34bcbece5affcefdb6e8282deed935d`.
Previous reviewed head: `fc39fee6be5024df41a0cc937417401100c6303b`, round 123.
The tier remains HIGH under BUILD.md rule 3 and the lead's PTY scope ruling.
Authority: BUILD.md rule 5 and testing rule 10; the lead's anchor rulings; plan 23b-23e.
The reviewer read all five changed files, the shared process owners, worker lifecycle paths, description, and completed gate.
Round 123 covers the whole input admission change. This correction changes no product code, testkit behavior, counters, or pending IDs.

### F64 — HIGH — Shared-crate migration; CLOSED

The named real-PTY regression now uses GuardedSession, with Guard, OwnedChild, Blocker, Bounded, and Deadline from botster-test-process.
The workspace and worker dev dependency now name that crate.
Guard wraps the payload and verifies the anchor's leader against the payload report before launch returns.
OwnedChild owns and reaps the worker; group mode also cleans the observer group on Drop.
The test uses Blocker commands for release and hold. It does not use the former sleep to hold the payload.
Bounded reads the ready and done markers. Close-on-exec prevents inherited FIFO ends from keeping release open.
The regression checks the cancelled prefix, the resumed following byte, and the exact received bytes.
It sends Kill, consumes Exited, and then sends Remove. The worker must exit with code 0 within the shared bound.
GuardedSession releases the payload anchor before its fields drop; the worker ends before Guard reads the cleanup result.
Production keeps payload reaping. The fixture copies no process guard or cleanup implementation.
Both named slow-tier tests pass at this exact head. The description now names the actual shared owners.
These changes close the migration finding. F66 below records a separate missing parent-death path in the new fixture.

The lead's handoff explicitly accepts the temporary bounded-accept loop at this head.
The listener is nonblocking. A real poll waits for readiness within Deadline::cleanup; EINTR retries use the remaining time.
This loop is not a sleep or busy poll. The accepted carry remains required:
The second of #181 and #198 to land adds one temporary allow entry.
P6's next crate PR adds bounded accept, removes that entry, and moves this regression to that function.
The reviewer does not treat this accepted carry as a finding or BLOCKED state.

### F65 — MEDIUM — Structured I/O-shell citations; CLOSED

Both new reasons now use the required decision (proof_a, proof_b) form.
Driver::write_pty_once cites io_decisions::pty_write with the pure result test and the named real-PTY regression.
Driver::set_pty_write_interest cites io_decisions::pty_write_interest with the pure interest test and the same real regression.
The exact-head gate selects the pure tests and both slow regression binaries. The pure decisions remain mutation-tested.
No gate-decision function is excluded. Plan 23c/23d's citation requirement is satisfied.

### F66 — HIGH — New fixture leaves the worker or observer alive after test-parent death; OPEN

This is integration R2-1, independently confirmed by the package reviewer.
At crates/botster-worker/tests/common/session.rs:618-628, GuardedSession starts the observer directly with OwnedChild::spawn_group.
The other branch starts the worker directly with OwnedChild::spawn.
OwnedChild::start calls Command::spawn and stores the child. OwnedChild::Drop performs its cleanup.
OwnedChild does not start an anchor or another owner that survives test-parent death.
GuardedSession's Guard wraps only the payload; that anchor owns the payload group, not the worker or observer group.
Test-parent death skips Rust Drop. The control socket closes, but DP-8 intentionally preserves the worker after LinkClosed.
The worker's payload-exit and drain paths report the exit; they do not request worker exit without Remove or Terminate.
Thus the payload anchor does not supply worker or observer cleanup when the test parent dies.
BUILD.md testing rule 10 requires cleanup on every exit path. The lead's ruling explicitly includes test-parent death.
The existing parent_death_ends_the_driver_observer proof starts the legacy Session fixture, not GuardedSession.
Its passing result does not prove cleanup for the new fixture.

Required correction: use shared-crate ownership that survives test-parent death for the new worker and observer paths.
Ask P6 for a missing shared capability. Do not copy local guard or cleanup infrastructure.
Keep the payload anchor, identity checks, group reservation, exact reaping, and current deadline rules.
Add a bounded parent-death proof that exercises the new fixture and observes cleanup of its worker or observer.
Provide the completed exact-head gate. The reviewer sent F66 directly to P3 and integration.
No lead decision is needed to continue this correction.

### Completed evidence and whole-change scope

Log: `~/botster-sessions/gates/botster-core-stage1-p3-pty-input-8a762ac0-pool-20261009-125251-39706.log`.
The log names this exact head and base. It runs on gaming, kernel 6.18.40.1-microsoft-standard-WSL2.
All ten CI steps pass. The default tier passes 1040 tests in 5.046 seconds.
The slow tier passes 245 tests in 10.244 seconds.
The named real-PTY regression passes in the integration binary in 0.020 seconds and driver binary in 0.022 seconds.
The new testkit behavior tests and pure driver decision tests pass.
Signals scan 162 Rust files. Timers scan 146 Rust files.
The ledger has 675 IDs: 625 pending, two deferred, two withdrawn, and 46 active. All 46 active IDs pass.
The report retains 568 pending and 57 IDs without transcripts.
Both mutation runs test 118 mutants: 108 caught, ten unviable, zero missed, and zero timeouts.
The separate mutation command uses NEXTEST_PROFILE=slow. The log sets the mutation timeout to 20 seconds.
Fuzz has no changed decoder harness. Full CI takes 264.4 seconds; separate mutants take 226.9 seconds.
The gate exits 0 after 501 seconds. These results do not prove the missing parent-death path.

Round 123's product source coverage remains applicable because the correction changes no product code.
The thirteen pending removals remain supported; the minimum counts remain testkit 28/70 and real 0/70.
Part B and the later terminal and route duties remain outside this PR's scope.
Integration records NOT CLEAN at this head in verdict commit `893501ccf485b6a664705fa8d0c08aee5f61d21e`.

### Verdict and retained duties

PR #198 is NOT CLEAN at `8a762ac00a354b032432a5a26963d93c91e0284c` for the P3 package review.
F64 and F65 are CLOSED. F66 HIGH is OPEN. This is #198's second recorded NOT CLEAN round.
No round-limit notice is due. Normal findings go to P3 and integration; they are not BLOCKED reports to the lead.
The named real-PTY proof has moved to the shared crate and passes, but its parent-death ownership needs correction before merge.
This verdict does not change #168's original head verdict or authorize merging that original PR.
#197 retains round 122 CLEAN and its merge at 7aec2bb9. #195 retains round 121 CLEAN with F63 closed.
#192 retains round 117 NOT CLEAN with F61 and F62 open. Part B retains F39 and its other recorded duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings and verdict rounds remain preserved at their named heads and scopes.

VERDICT: NOT CLEAN


## Round 125 — PR #198 worker anchor and parent-death proof — 2026-10-09

Reviewed head: `546f8084f6bbcbfe9ca2ba9ef323dc74f68bec15`.
Base: `7aec2bb917f48994d1705301d2383873a219c031`.
Parent and previous reviewed head: `8a762ac00a354b032432a5a26963d93c91e0284c`, round 124.
The head contains the current v1 base. The tier remains HIGH under BUILD.md rule 3 and the lead's PTY scope ruling.
Authority: BUILD.md rule 5 and testing rule 10; the lead's anchor rulings; plan 23b-23e.
The reviewer read the complete one-file correction, shared Guard/OwnedChild/read/wait helpers, full description, and completed gate.
Rounds 123 and 124 cover the whole product change and shared-crate migration.
This correction changes no product code, testkit behavior, mutation exclusions, counters, or pending IDs.

### F66 — HIGH — Worker and observer ownership after test-parent death; CLOSED

GuardedSession now starts both possible worker programs through the shared Guard wrapper.
The real-worker branch retains the worker arguments and launch environment.
The observer branch retains the test selector and control environment, so it still runs the changed Driver under mutation.
OwnedChild::spawn_group makes the worker or observer the leader of a separate group.
Guard::anchors(1) verifies the worker's leader pid before the payload launch.
Guard::anchors(2) verifies the payload's leader pid after Launched.
The worker anchor reports first because the fixture waits for it before starting the payload.
The payload retains its separate session and anchor. Production alone reaps the payload.
DP-8 remains unchanged: link loss preserves the worker, while the test fixture's anchor ends it when the test dies.

Both anchors use the shared identity checks, group reservation, cleanup rounds, and exact reserve reaping.
OwnedChild retains worker ownership and exact worker reaping.
On Drop, the fixture releases both anchors before ending and reaping the worker, then reads the guard outcomes.
A construction failure also leaves the started worker with its shared owner and guard.
Test-parent death closes the guard connections without requiring Rust Drop.
The worker anchor and payload anchor then clean their respective groups.
No local process guard or cleanup implementation is added.

The new named proof is a_guarded_session_ends_when_its_test_parent_dies.
It starts guarded_parent in the same test binary through OwnedChild and keeps the parent's stdin open.
The helper starts GuardedSession, reports the worker and payload group IDs, and waits on a bounded stdin read.
The helper also ends after stdin EOF or the shared cleanup bound, so the outer test does not leave an unbounded helper.
The test reads the report through shared first_line, kills the parent, and reads its exit status through the shared owner.
It then uses rounds::await_group_end for each reported group within Deadline::cleanup.
These observations send no signal to either subject group. Thus the proof observes anchor cleanup after parent death.
Both integration and driver-observer binaries run and pass this proof at the exact reviewed head.
This closes package F66 and confirms the correction for integration R2-1. Integration controls its own verdict.

P3 reports that removing the worker wrapper makes both tests fail and leaves the worker or observer alive.
The reviewer did not read that red-check patch or log and does not count it as independently verified evidence.
The source and completed exact-head positive proof support this package closure.
Integration requested the reported red-check evidence and owns that verification.

### Retained whole-change coverage and accepted carry

F64's shared-crate migration and F65's structured citations remain CLOSED.
The required in_6_real_pty_cancel_keeps_counts_and_resumes_the_next_write still runs in both binaries.
It checks the certain cancelled prefix, the next transaction's completion, and the exact received bytes.
Rounds 123 and 124 retain their admission, driver, testkit, control, and transcript coverage at this unchanged product source.
The thirteen pending removals remain supported. Minimum counts remain testkit 28/70 and real 0/70.
Route contiguity, terminal-model work, CaptureSnapshot, and later route duties remain outside this PR's scope.

The lead's accepted bounded-accept carry remains required.
The second of #181 and #198 to land adds one temporary allow entry for this launch site's deadline poll.
P6's next crate PR adds bounded accept, removes that entry, and moves the regression to the shared function.
The listener stays nonblocking, and the interim poll waits on a real event within the existing shared deadline.
This accepted carry is not a finding or BLOCKED state.

### Completed exact-head evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p3-pty-input-546f8084-pool-20261009-130637-72222.log`.
The log names this exact head and base. It runs on msa1, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 1040 tests in 3.765 seconds.
The slow tier passes 249 tests in 10.127 seconds.
The parent-death proof passes in the integration binary in 0.007 seconds and driver binary in 0.008 seconds.
The real-PTY cancellation proof passes in the integration binary in 0.013 seconds and driver binary in 0.014 seconds.
Signals scan 162 Rust files. Timers scan 146 Rust files.
The ledger retains 675 IDs: 625 pending, two deferred, two withdrawn, and 46 active. All 46 active IDs pass.
Both mutation runs test 118 mutants: 108 caught, ten unviable, zero missed, and zero timeouts.
The separate mutation command uses NEXTEST_PROFILE=slow. Both logs set the mutation timeout to 20 seconds.
Fuzz has no changed decoder harness. Full CI takes 235.7 seconds; separate mutants take 232.9 seconds.
The gate exits 0 after 476 seconds. The reviewer read the completed results and ran no gate.

### Verdict and scope

PR #198 is CLEAN at `546f8084f6bbcbfe9ca2ba9ef323dc74f68bec15` for the P3 package review.
F64, F65, and F66 are CLOSED. No package finding remains on this PR.
The ported PTY path now has its required named shared-crate real proof and parent-death cleanup proof at this exact head.
This verdict does not change #168's original head verdict or authorize merging that original PR.
Integration still requires its own exact-head verdict. The accepted bounded-accept carry remains required at merge.
#197 retains round 122 CLEAN and its merge at 7aec2bb9. #195 retains round 121 CLEAN with F63 closed.
#192 retains round 117 NOT CLEAN with F61 and F62 open. Part B retains F39 and its other recorded duties.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings and verdict rounds remain preserved at their named heads and scopes.

VERDICT: CLEAN


## Round 126 — PR #199 terminal model and CaptureSnapshot — 2026-10-09

Reviewed head: `f50bf6beb486007c16e49ca13a3c5d39058da15c`.
Base: `58d6663204b50ce6c42d467e4fd6715ab46145dd`, v1 after #198.
Parent: `bcfb4afc80eeaa0ef3ea369a91dea6279948533f`.
The tier is correctly HIGH under BUILD.md rule 3 for the shared crates and rule 5 for the worker paths.
Authority: BUILD.md, plan 23b-23e, Core ST/IN/EV clauses, Amendment 13 candidate 5, and the pinned libghostty binding.
The reviewer read all twelve changed files, the full description, relevant binding and host paths, transcript expectations, map rows, and completed evidence.
The model port was also compared with bef6177d. No tests, builds, gates, measurements, or mutants ran in this review.

### F67 — MEDIUM — Cursor reads insert spaces for wide-character continuation cells; OPEN

At crates/botster-worker-core/src/worker/model.rs:258-267, read_cursor maps every empty cell string to a space.
The binding's row_cells does not use an empty string for an ordinary empty cell.
reads::cell_text returns a space for a cell without text. It returns an empty string for SPACER_TAIL or SPACER_HEAD.
The binding's row_cells documentation and wide-character test confirm this distinction.
For program output a日b, the binding returns the cells a, 日, an empty continuation string, and b.
The worker adds a space for that continuation string in both row_text and text_before_cursor.
The worker thus changes libghostty's text instead of applying only ST-3's trailing-space rule.
The current worker read test uses ASCII and does not cover this case.

Required correction: preserve the binding's empty continuation text when assembling both cursor fields.
Keep the cursor coordinates as terminal-cell coordinates. Apply only ST-3's trailing U+0020 trim to row_text.
Add a wide-character worker proof that derives both expected fields from an independent libghostty terminal's row_cells.
Do not hand-write the expected terminal text. The reviewer sent this finding directly to P3 and integration.
Integration confirms the source trace and records it as R1-2, credited to the package reviewer.

### F68 — HIGH — Model event drain discards OSC 5522 acknowledgements; OPEN

This is integration R1-1, independently confirmed by the package reviewer.
At model.rs:141-219, after_step drains the model's events and queues only drained.pty_writes.
The binding stores each OSC 5522 acknowledgement in Drained::clipboard_acks, separate from pty_writes.
The binding's drain takes those entries and resets ack_bytes. The worker never consumes clipboard_acks.
Thus this head can surface ClipboardWrite while discarding the acknowledgement that the program must receive.
Amendment A13-1b assigns that acknowledgement to the worker through the one admission point.
Each entry must be one contiguous transaction, in the order of the writes it answers.
The acknowledgement must survive input pressure and event loss. It advances no input revision and completes no host operation.
The description cites Amendment 13 but contains no explicit lead exclusion of this duty.
The existing query-shadow reply proof does not cover the separate acknowledgement field.

Required correction: admit each clipboard acknowledgement through the existing reply path as its own transaction.
Preserve order and exact bytes. Do not merge all acknowledgements into one transaction or send them outside admission.
Add an independent libghostty oracle proof and an input-pressure proof for acknowledgement delivery and contiguity.
The proof must also check that the acknowledgement advances no input revision and completes no host operation.
The reviewer sent F68 directly to P3 and integration. No lead decision is needed to continue the correction.

### Whole-change source coverage

LaunchSpec carries CoreLimits with a serde default. The host copies its configured limits into each launch.
The host test checks the exact configured limits and stop grace. The link test checks round-trip and absent-field defaults.
The worker constructs the libghostty model before starting the payload and applies the clipboard limit.
A model construction failure reports LaunchFailed with WorkerFailed.
The model retains output while Spawning and feeds it after Launched, preserving the report order.
Launched carries the fresh modes and the binding's GHOSTSNP format. No output has set title or cwd at that point.
The worker advances its existing model_rev for each completed model step and retains #197's Output coalescing.
An incomplete string terminator waits for more bytes instead of advancing a step with zero consumption.
The model drains events after each step and compares final mode flags with the last posted flags.
Title, cwd, bell, prompt marks, notification truncation, clipboard contents, and lost-kind reports use the binding's events.
Notification truncation now uses floor_char_boundary; the earlier mutation timeout loop is absent.
Unknown clipboard locations produce a loss report rather than an invented location letter; the body asks the lead about that case.
The acknowledgement handling remains F68.

ReadScreen and ReadModeFlags use the model and carry the worker's model_rev.
ReadCursor uses the binding's cursor and row cells, subject to F67.
CaptureSnapshot obtains the model's GHOSTSNP bytes at this serialized operation point.
It sends one owned page, index 0 and last true, before Done with page_count, total_bytes, and model_rev.
ST-6 specifies no page size. The unchanged host mints CaptureId and owns pages, limits, release, and expiry.
The worker refuses a snapshot over max_snapshot_bytes with SnapshotTooLarge and sends no page.
The tests cover the exact bound, the over-bound case, the page bytes against an independent oracle, and report order.

The admission queue now holds host writes and model replies in arrival order.
Replies use no request ID, complete no host operation, and advance no host or client input revision.
The host's guards still run once at transaction start. The model encodes Paste, Key, Mouse, and Focus at that start.
Paste counts include markers only in PTY bytes. A required absent paste mode gives the certain zero result.
Key repeat encodes one event and repeats its bytes within one transaction. Existing host validation supplies repeat and payload bounds.
The tests compare key, mouse, paste markers, and query-shadow bytes with an independent terminal.
The reply path drops replies after payload exit. One active transaction still owns the PTY across short writes.
Route offers, pending-query duties, resize, cross-instance tokens, ReadFacts, setters, and oracle_resume remain later scope.
No new testkit control or process fixture is introduced. The real session helper changes only its LaunchSpec literal.

### Pending removals and completed evidence

The diff removes exactly 42 IDs and adds none to core-pending.txt.
At the pinned contracts commit 0389165, all 42 map to core-testkit, core-testkit+edge, or core-testkit+perturb.
None is real-only or names a slow proof. All 42 have exact-head gate PASS records.
The two minimum removals are or_1_no_progress_outside_pump and tm_3_next_deadline_is_host_armed.
The 70-ID minimum list gives testkit 28/70 at the base and 30/70 at this head. Real remains 0/70.
The transcript paths use the real Worker with existing pty_output/pty_input controls, model events, reads, captures, and semantic input.
The cross-owner AM-4 removal follows the lead's ruling recorded in the description; it already passes at the base.
The baseline evidence reports 47 passed and 41 failed. Its 41 failing IDs are exactly the other 41 removals.
The all-pending probe reports 132 passed and 482 failed; every baseline failure above is among its passes.
The exact-head gate runs the selected 88 active IDs and reports 88 passed, zero failed.
The probe's other pending failures are not part of the selected merge gate or these removals.

Evidence directory: `~/botster-sessions/shared/core-stage1/evidence/p3-pr-b/`.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-capture-snapshot-f50bf6be-pool-20261009-135627-42103.log`.
The log names this exact head and base. It runs on msa1, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 1100 tests in 5.506 seconds.
The slow tier passes 249 tests in 10.117 seconds.
The existing parent-death proof passes in both binaries, as does the real-PTY cancellation proof.
Signals scan 163 Rust files. Timers scan 147 Rust files.
The ledger has 675 IDs: 583 pending, two deferred, two withdrawn, and 88 active.
The conformance report retains 526 pending and 57 without transcripts.
Both mutation runs test 66 mutants: 58 caught, eight unviable, zero missed, and zero timeouts.
The separate run uses NEXTEST_PROFILE=slow and records a 20-second mutation timeout.
The changed link decoder makes fuzz applicable; the completed fuzz step passes.
Full CI takes 253.0 seconds; separate mutants take 76.9 seconds. The gate exits 0 after 337 seconds.
Passing evidence does not close F67 or F68.

### Verdict and retained scope

PR #199 is NOT CLEAN at `f50bf6beb486007c16e49ca13a3c5d39058da15c` for the P3 package review.
F67 MEDIUM and F68 HIGH are OPEN. This is #199's first recorded NOT CLEAN round; no round-limit notice is due.
Integration records the same two findings in verdict commit `99760ea72eab3584df6ff30e1d404bd2014fa180`.
Normal findings go to P3 and integration. They are not BLOCKED reports to the lead.

F39 remains OPEN for #163's actual guard merge change and its registration and anchor-owner wait bounds.
#199's terminal-model PR B is a different scope from that earlier guard/reaping Part B.
This PR changes no guard wait, registration wait, or anchor ownership. It neither closes F39 nor adds a new F39 defect.
#198 retains round 125 CLEAN and is merged at 58d66632. F64, F65, and F66 remain closed at their named scopes.
#192 retains round 117 NOT CLEAN with F61/F62 open. Other retained #163 duties remain open.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings and verdict rounds remain preserved at their exact heads and scopes.

VERDICT: NOT CLEAN


## Round 127 — PR #199 cursor and acknowledgement correction — 2026-10-09

Reviewed head: `f2a8237c249ac6f9aa1169dda4db19b7c75bc5c6`.
Base: `58d6663204b50ce6c42d467e4fd6715ab46145dd`.
Parent and previous reviewed head: `f50bf6beb486007c16e49ca13a3c5d39058da15c`, round 126.
The head contains the current v1 base. The tier remains HIGH; the correction also changes the terminal binding's callback decision.
Authority: BUILD.md, plan 23b-23e, ST-3, A13-1b, and steward ruling R-41 at contracts main c2a04f8.
The reviewer read the complete four-file correction, the full description, R-41, the host loss-marker path, and completed gate.
Round 126 covers the whole change. This correction changes no launch interface, snapshot implementation, pending list, or mutation exclusion.

### F67 — MEDIUM — Wide-character cursor text; CLOSED

read_cursor now joins the binding's cells without replacing empty continuation strings.
It takes the prefix by terminal-cell count and trims only trailing U+0020 from row_text.
The new a_wide_character_adds_no_text_for_its_second_cell proof feeds a日b to the worker and an independent libghostty terminal.
It derives the expected row and prefix from the oracle's row_cells and checks the cursor's cell coordinate.
The proof also checks that its input reaches a wide continuation cell.
The exact-head default gate selects and passes this proof. No expected terminal text is hand-written in the new proof.

### F68 — HIGH — OSC 5522 acknowledgement delivery; CLOSED

After each model step, after_step now queues each clipboard_acks entry through enqueue_reply as its own transaction.
The existing reply path preserves arrival order, short-write contiguity, and the absence of a host request or input revision.
The new osc_5522_acknowledgements_are_written_in_order_through_the_admission_point proof obtains three acknowledgements from an independent terminal.
A host write owns the PTY before the acknowledgements arrive. A later host write waits behind all three.
The proof gives the first acknowledgement a partial write and checks that its remaining bytes precede the next acknowledgement.
It checks the acknowledgement order, exact oracle bytes, absence of Done and HostInput, and the later host revision's single increment.
The exact-head default gate selects and passes this proof. The source still drops replies after payload exit.
P3 reports red checks against the previous model.rs; the reviewer did not run those checks.
The source and selected positive proofs support both closures.

### F69 — LOW — Unknown-location loss path accepts a nonempty selection; OPEN

R-41 outcome (1) requires no ClipboardWrite for an unknown location, plus EventsLost with ClipboardWrite in its kinds.
The binding now correctly gives IO_ERROR for ClipboardLocation::Other, regardless of size.
However, clipboard_selection at model.rs:76-84 returns a nonempty selection before examining the location.
clipboard_selection(Some("s0"), ClipboardLocation::Other(9)) therefore returns Some("s0").
after_step then posts ClipboardWrite and skips the loss branch, contrary to the new ruling and description.
The current selection proof covers Other only with no selection. It does not cover this bypass.
The existing internal Observation::Lost is the correct transport for the loss branch: the unchanged host calls post_lost and emits EventsLost.
No new event or enum variant is required.

Required correction: reject Other before accepting any selection string.
Preserve the nonempty selection's precedence for the three known locations.
Extend the existing pure proof to cover Other with nonempty, empty, and absent selection strings.
Keep the binding's IO_ERROR result and each acknowledgement's admission transaction.
The severity is LOW because the pinned model cannot produce Other; R-41 explicitly defines this defensive path.
BUILD.md requires all findings to close on a HIGH PR, including LOW. This finding therefore prevents CLEAN.
The reviewer sent F69 directly to P3 and integration. No lead decision or new conformance ID is needed.

### Completed evidence and retained coverage

Log: `~/botster-sessions/gates/botster-core-stage1-p3-capture-snapshot-f2a8237c-pool-20261009-140746-68630.log`.
The log names this exact head and base. It runs on msa1, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 1103 tests in 5.404 seconds.
The slow tier passes 249 tests in 10.115 seconds.
The two correction proofs and a_write_is_io_error_over_the_bound_or_at_an_unknown_location run and pass in the default tier.
The existing real-PTY and test-parent-death proofs remain selected in both binaries.
Signals scan 163 Rust files. Timers scan 147 Rust files.
The ledger retains 675 IDs: 583 pending, two deferred, two withdrawn, and 88 active. All 88 active IDs pass.
The report retains 526 pending and 57 without transcripts.
Both mutation runs test 70 mutants: 62 caught, eight unviable, zero missed, and zero timeouts.
Both logs set the mutation timeout to 20 seconds. The separate command uses NEXTEST_PROFILE=slow.
The fuzz step passes. Full CI takes 255.7 seconds; separate mutants take 80.0 seconds. The gate exits 0 after 343 seconds.
The unchanged 42 removals retain round 126's transcript, baseline, map, and source coverage.
Minimum counts remain testkit 30/70 at this head and real 0/70.
Passing evidence does not cover or close F69's nonempty-selection branch.

### Verdict and scope

PR #199 is NOT CLEAN at `f2a8237c249ac6f9aa1169dda4db19b7c75bc5c6` for the P3 package review.
F67 and F68 are CLOSED. F69 LOW is OPEN. This is #199's second recorded NOT CLEAN round.
No round-limit notice is due. Normal findings go to P3 and integration, not to the lead as BLOCKED.
F39 remains open for #163's guard merge change. This correction changes no guard wait or registration wait.
#198 retains round 125 CLEAN and its merge at 58d66632. #192 retains F61/F62 and its last exact-head verdict.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings and verdict rounds remain preserved at their exact heads and scopes.

VERDICT: NOT CLEAN


## Round 128 — PR #199 unknown-location correction — 2026-10-09

Reviewed head: `086bd814b72f8bc3730bbb9ed6c5bdd5aaf395a3`.
Base: `58d6663204b50ce6c42d467e4fd6715ab46145dd`.
Parent and previous reviewed head: `f2a8237c249ac6f9aa1169dda4db19b7c75bc5c6`, round 127.
The head contains the current v1 base. The tier remains HIGH under BUILD.md rules 3 and 5.
Authority: BUILD.md, plan 23b-23e, and R-41 at contracts main c2a04f8.
The reviewer read the complete one-file correction, full description, exact refs, and completed gate.
Rounds 126-127 cover the whole change, the cursor correction, and acknowledgement delivery.
This correction changes only clipboard_selection and its existing pure proof.

### F69 — LOW — Unknown-location selection bypass; CLOSED

clipboard_selection now checks the location first. Other returns None before the function examines any selection string.
The three known locations retain their defined letters and the precedence of a nonempty program selection.
The proof checks each known location with absent, empty, and nonempty selection strings.
It also checks Other with those three forms. The nonempty-string bypass is absent.
The default gate selects and passes the extended proof at this exact head.
The unchanged after_step loss branch then sends Observation::Lost for an unknown location.
The unchanged host post_lost turns that observation into EventsLost with ClipboardWrite in its kinds.
The binding still answers Other with IO_ERROR. Each acknowledgement still enters its own reply transaction.
This satisfies R-41 without a new event, variant, conformance ID, or process fixture.
The description now states that the location is checked before selection handling.
P3 reports a red check against the previous function. The reviewer did not run that check.

### Retained whole-change coverage and completed evidence

F67 and F68 remain CLOSED. The wide-character cursor proof and ordered acknowledgement proof remain selected and pass.
The binding's unknown-location and size decision proof also passes.
No new package finding is recorded. The earlier launch, model, read, capture, admission, and host coverage remains applicable.
The unchanged 42 removals retain their source, transcript, replacement-map, and baseline evidence from round 126.
The selected gate again reports 88 active IDs passed, zero failed.
The minimum counts remain testkit 30/70 at this head and real 0/70. The base has testkit 28/70.
Later route, resize, cross-instance token, facts, setter, and oracle_resume duties remain outside this PR's scope.

Log: `~/botster-sessions/gates/botster-core-stage1-p3-capture-snapshot-086bd814-pool-20261009-141545-4327.log`.
The log names this exact head and base. It runs on msa1, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 1103 tests in 5.409 seconds.
The slow tier passes 249 tests in 10.129 seconds. Existing real-PTY and test-parent-death proofs remain selected.
Signals scan 163 Rust files. Timers scan 147 Rust files.
The ledger retains 675 IDs: 583 pending, two deferred, two withdrawn, and 88 active.
The report retains 526 pending and 57 without transcripts.
Both mutation runs test 70 mutants: 62 caught, eight unviable, zero missed, and zero timeouts.
The separate command uses NEXTEST_PROFILE=slow and records a 20-second mutation timeout.
The fuzz step passes. Full CI takes 261.0 seconds; separate mutants take 81.4 seconds.
The gate exits 0 after 349 seconds. The reviewer read completed evidence and ran no gate.

### Verdict and retained scope

PR #199 is CLEAN at `086bd814b72f8bc3730bbb9ed6c5bdd5aaf395a3` for the P3 package review.
F67, F68, and F69 are CLOSED. No package finding remains on this PR.
Integration still requires its own exact-head verdict. No third NOT CLEAN round or round-limit notice is needed.
F39 remains OPEN for #163's guard merge change and its registration and anchor-owner wait bounds.
This PR changes no guard wait or registration wait. Its terminal-model PR B does not close that earlier guard/reaping Part B duty.
#198 retains round 125 CLEAN and its merge at 58d66632. #192 retains F61/F62 and its last exact-head verdict.
The accepted bounded-accept carry remains with #181, which lands second after #198.
The reviewer changed no product code and ran no tests, builds, measurements, mutants, or gates.
All earlier findings and verdict rounds remain preserved at their exact heads and scopes.

VERDICT: CLEAN


## Round 129 — PR #200 oracle_resume dispatch — 2026-10-09

Reviewed head: `61ab4501df234c3da74e6e2c24c1de3b12db2c8e`.
Base: `a6555ebaf221042ca7b777ca2f2425e4a63dd960`.
Parent: `f581deb3857e23a6a640f6adba300d1c44928458`.
The head contains the current v1 base. The tier is HIGH under BUILD.md rules 3 and 5.
The change crosses the testkit and worker packages and adds a public worker getter.
Authority: BUILD.md, plan 23b-23f, pinned contracts v0.1.20 at 03891658, and the lead's getter ruling (a).
Plan pin: stage1-plan.baa0d2a6.md, sha256 baa0d2a6c96d156ba50a17693948fe844a3d64adb4de9fc9776a44ad756a0818.
The reviewer read the complete nine-file change, full description, helper, simulation dispatch, refusal layer, three pinned transcripts, replacement map, and completed evidence.

### F70 — HIGH — The resume comparison does not observe the current worker model; OPEN

This is integration finding R1-1. Integration independently confirms the finding, and P3 accepts it.
In crates/botster-core-testkit/src/resume_controls.rs:149-156, the control constructs its comparison target with replay(size, log.output).
The helper then compares that target's native snapshot with Core's capture pages after the logged suffix.
The control never reads the current worker model. Worker::model_rev supplies a cut label, not that model's state.

A worker can produce a valid capture and then stop applying later output.
The edge still records those output bytes. Both comparison states still apply the logged suffix and can agree.
The control can therefore return equal=true while the actual worker model remains at the capture state.
The proof pages_of_another_state_are_not_equal changes the capture bytes. It does not cover this case.

The pinned docs/core-testkit-controls.md requires oracle_resume to compare with the session's model.
The existing snapshot_controls::oracle_resume helper also takes the actual model as its comparison target.
The lead's ruling (a) authorizes the read-only model_rev getter. It does not authorize replacing the subject with a replay.
This is an observation gap, not a demand for new product behavior or a production test hook.

Required correction: use the current worker model as the comparison target through the lead-approved observation interface.
Keep the actual Core capture pages and the exact output suffix as the independent restored side.
Add a negative proof with an unchanged valid capture and suffix, but a different live subject state.
The proof must return equal=false for that state. Preserve the positive partial-sequence and distinct-cut proofs.
P3 has already asked the lead about the observation interface. No duplicate reviewer escalation is needed.

### Source and conformance scope

The model_rev getter returns the existing field and changes no worker behavior.
Its proof checks unchanged revision for an unfed ESC, revision advance after a step, and agreement with ReadCursor.
The testkit records output at the program edge and records revision positions in Binding::ready.
The simulation calls ready before choosing an input. The capture log records pages when Completed passes through poll_events.
The refusal layer continues to wrap TestkitCore. The new control uses the existing control registry and parser.
The replay retains the unconsumed suffix. Its loop reduces the remaining input after every nonterminal step.
The change introduces no real-process fixture, sleep, production reaping change, guard change, or mutation exclusion.
The existing shared real-process proofs remain selected in the slow tier.
No additional package finding is recorded at this head.

The pending list removes exactly these three IDs:
- conf::st_6b_model_after_baseline_plus_output_equals_the_sessions_model
- conf::st_6b_cut_inside_an_sgr_sequence_applies_the_suffix_not_prints_it
- conf::st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut

The replacement map at 0389165 assigns all three to core-testkit. None requires a new real-only proof for removal.
Each transcript takes an actual capture, writes a later suffix, and asks oracle_resume for equal=true.
F70 invalidates the comparison that supports all three removals. A passing report does not close F70.
The reported minimum rises from testkit 30/70 to 31/70; the reviewer does not accept that increase until F70 closes.
The real minimum remains 0/70. oracle_restore, oracle_graphics, and oracle_resume_every_cut remain pending.
The all-pending probe reports 135 passed and 479 failed, compared with the base's 132 passed and 482 failed.
These three IDs are the reported gains. No existing passing ID is reported lost.

### Completed exact-head evidence

Log: ~/botster-sessions/gates/botster-core-stage1-p3-oracle-resume-61ab4501-pool-20261009-145718-80866.log.
Probe: ~/botster-sessions/shared/core-stage1/evidence/p3-pr-c/probe-pr-c.out.
The gate names this exact head and base. It runs on msa1, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 1112 tests in 6.037 seconds.
The slow tier passes 249 tests in 10.116 seconds.
All five new resume control proofs and the worker getter proof run and pass in the default tier.
Signals scan 165 Rust files. Timers scan 149 Rust files.
The ledger retains 675 IDs: 580 pending, two deferred, two withdrawn, and 91 active. All 91 active IDs pass.
The report retains 523 pending and 57 without transcripts.
Both mutation runs test 35 mutants: 23 caught, 12 unviable, zero missed, and zero timeouts.
Both runs record a 20-second mutation timeout. The separate run uses NEXTEST_PROFILE=slow.
The fuzz step passes because the diff changes no crate with a decoder harness.
Full CI takes 152.8 seconds. Separate mutants take 118.3 seconds. The gate exits 0 after 279 seconds.
The reviewer reads completed evidence only. Passing evidence does not establish the missing live-model comparison.

### Verdict and retained scope

PR #200 is NOT CLEAN at `61ab4501df234c3da74e6e2c24c1de3b12db2c8e` for the P3 package review.
F70 HIGH is OPEN. This is #200's first recorded NOT CLEAN round.
The reviewer sent F70 directly to P3 and integration. No round-limit notice is due.
The ordinary correction remains in that loop. It is not a BLOCKED report to the lead.
#199 retains round 128 CLEAN and its merge at a6555eba. F67, F68, and F69 remain CLOSED.
F39 remains OPEN for #163's guard merge change and its registration and anchor-owner wait bounds.
#198 retains round 125 CLEAN and its merge at 58d66632. #192 retains F61/F62 and its last exact-head verdict.
The accepted bounded-accept carry remains with #181, which lands second after #198.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier findings and exact-head verdicts remain preserved.

VERDICT: NOT CLEAN


## Round 130 — PR #200 live-model correction — 2026-10-09

Reviewed head: `63b3b280fb1b21625f18297a15bff41d66a3f65e`.
Base: `a6555ebaf221042ca7b777ca2f2425e4a63dd960`.
Parent and previous reviewed head: `61ab4501df234c3da74e6e2c24c1de3b12db2c8e`, round 129.
The head contains the current v1 base. The tier remains HIGH under BUILD.md rules 3 and 5.
The reviewer read the complete six-file correction, full description, exact refs, shared-machine dispatch, lock use, and completed gate.
Round 129 covers the whole original change, three transcripts, replacement map, pending list, and baseline evidence.
Authority: BUILD.md, pinned contracts v0.1.20 at 03891658, plan 23b-23g, and lead ruling (1).
The reviewer read the new plan 23g delta. It changes no conclusion on this PR, which changes no mutation exclusion.
Plan pin: stage1-plan.0467b050.md, sha256 0467b0501dd088adebc3a6100533faf4e6270ce208cd2a1b73572c2da98dffb6.

### F70 — HIGH — Missing live-model comparison; CLOSED

Lead ruling (1), message msg_plugin-w_1791583451_c908fe, requires the actual live model as the comparison target.
The ruling authorizes Worker::model_snapshot with CaptureSnapshot's encoder and permits replay only to find the suffix.
It also requires a negative proof for a worker that stops stepping after the capture.
P3 supplied the exact ruling text and its original proposed interface. The shared implementer handoff also records both.

Worker::model_snapshot reads the existing model and calls model.term.snapshot(), the encoder that CaptureSnapshot uses.
It returns None before launch. It returns the encoder's owned bytes or error after launch, without the capture size bound.
It changes no model state, queue, revision, clock, action, or production path.
The description names both public worker getters and states the facade-only public API check's scope.

The testkit now gives the simulation and process cell the same SharedWorker.
Its Machine implementation forwards handle, poll_action, and next_deadline to the existing Worker.
The normal simulation remains the only writer. The control reads the live machine between pumps.
The lookup clones the shared machine and releases the process table and cell locks before it locks that machine.
Binding::ready releases the machine lock after reading model_rev and before it locks the process cell.
MachineNode releases each machine call's lock before it performs the returned edge action.
Construction still drains the initial hello actions before adding the node. No new scheduler choice or synchronization point is added.
No worker owns a process cell. The new shared reference therefore adds no ownership cycle.

oracle_resume retains Core's actual capture pages. It restores them in a separate native terminal and applies the exact logged suffix.
The replay now returns only a consumed-byte count. It cannot supply the comparison target.
The control obtains the live worker snapshot and compares the two encoded snapshots.
An unknown capture, unknown revision, missing worker model, or encoder error cannot produce equal=true.

The proof a_live_model_that_diverged_is_not_equal keeps the capture and logged suffix unchanged and changes the actual worker model.
It first checks equal=true for the original state and then checks equal=false after the live state changes.
The proof a_worker_that_stopped_stepping_is_not_equal keeps a valid capture and records a suffix that the worker never applied.
It checks equal=false for that unchanged live state. This is F70's concrete failure case.
Both helpers exist only in testkit cfg(test) code. The production Worker has no test mutation hook.
The worker proof checks None before launch, equality with an independent native terminal, and equality with CaptureSnapshot's page.
All three new proofs run and pass in the exact-head default tier.
P3 reports that both negative proofs fail against the previous control. The reviewer did not run those red checks.
The correction satisfies F70 and the lead's ruling (1). No new package finding is recorded.

### Retained scope and completed evidence

The original partial-SGR, distinct-cut, wrong-page, unknown-revision, first-revision-position, and unconsumed-suffix proofs remain selected and pass.
The getter revision proof also remains selected and passes.
The three pending removals retain round 129's transcript and replacement-map review.
The live-model comparison now supports their required ST-6b assertion. None is real-only.
The selected gate reports 91 active IDs passed and zero failed.
The reported all-pending probe remains 135 passed/479 failed, against base 132/482, with exactly these three gains and no reported loss.
The minimum is testkit 31/70 at this head and real 0/70. The base has testkit 30/70.
oracle_restore, oracle_graphics, and oracle_resume_every_cut remain pending and outside this PR's scope.
The change adds no real-process fixture, sleep, guard change, production reaping change, or mutation exclusion.
The existing real-PTY cancellation and parent-death proofs remain selected and pass in both worker binaries.

Log: ~/botster-sessions/gates/botster-core-stage1-p3-oracle-resume-63b3b280-pool-20261009-151955-30889.log.
The gate names this exact head and base. It runs on msa1, allocation 9438fb70, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 1115 tests in 5.847 seconds.
The slow tier passes 249 tests in 10.114 seconds.
Signals scan 165 Rust files. Timers scan 149 Rust files.
The ledger retains 675 IDs: 580 pending, two deferred, two withdrawn, and 91 active.
The report retains 523 pending and 57 without transcripts.
Both mutation runs test 49 mutants: 36 caught, 13 unviable, zero missed, and zero timeouts.
Both runs record a 20-second mutation timeout. The separate run uses NEXTEST_PROFILE=slow.
The fuzz step passes because the diff changes no crate with a decoder harness.
Full CI takes 180.6 seconds. Separate mutants take 142.9 seconds. The gate exits 0 after 332 seconds.
The reviewer read completed evidence and ran no gate.

### Verdict and retained scope

PR #200 is CLEAN at `63b3b280fb1b21625f18297a15bff41d66a3f65e` for the P3 package review.
F70 HIGH is CLOSED. No package finding remains on this PR. Integration requires its own exact-head verdict.
No third NOT CLEAN round or round-limit notice is needed.
#199 retains round 128 CLEAN and its merge at a6555eba. F67, F68, and F69 remain CLOSED.
F39 remains OPEN for #163's guard merge change and its registration and anchor-owner wait bounds.
This terminal-model PR does not close that earlier guard/reaping Part B duty. It changes no guard or registration wait.
#198 retains round 125 CLEAN and its merge at 58d66632. #192 retains F61/F62 and its last exact-head verdict.
The accepted bounded-accept carry remains with #181, which lands second after #198.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier findings and exact-head verdicts remain preserved.

VERDICT: CLEAN


## Round 131 — PR #202 static Core TH-1 proof — 2026-10-09

Reviewed head: `2dc7dacfd90468bc55474ff020ca58312a66cd42`.
Base: `fe0e6d6d5f9df40838df957b7d8f5d0564712a6a`.
Parents: `8411effde214bfd11a8dd7c065be277aa771251b` and the reviewed base.
The head contains the current v1 base. The tier is HIGH under BUILD.md rules 3 and 5.
It changes the facade test crate and shared testkit, including a public testkit builder.
Authority: BUILD.md, pinned contracts v0.1.20 at 03891658, and accepted plan 23a-23h.
Current plan pin: stage1-plan.ea5e6dd4.md, sha256 ea5e6dd4de72db642773b9612b677d31ddb249f9a81b33e392851d7392b3efb3.
The reviewer read the complete three-file change, full corrected body, exact refs, pinned transcript and map row, driver handling, and completed evidence.

### Static proof and runner path

The conformance test crate asserts botster_core::Core: Send and asserts that the same type does not implement Sync.
Both assertions run at compile time, outside any test function or conditional branch.
A change that removes Send or adds Sync prevents this suite from compiling.
CORE_IS_SEND_NOT_SYNC records true only in the suite whose two assertions compiled.
The actual harness_factory gives that result to each seed's TestkitHarness through with_core_type.
The suite already declares static_assertions as a dev dependency. This PR adds no dependency or feature.
The existing facade Core has PhantomData<Cell<()>> for its non-Sync property. This PR changes no facade behavior.

The harness defaults to None and stores the supplied result as Some(bool).
Its existing CoreHarness method returns that value. The unit proof covers None, Some(true), and Some(false).
The pinned driver passes type_check: send_not_sync only for Some(true).
It fails Some(false) with the expected type mismatch and refuses to treat None as a pass.
The compile assertion observes the actual facade type. The testkit supplies no guessed type property by default.
The builder passes static evidence across the testkit's dependency boundary. It adds no production test hook.
No source finding is recorded.

The reviewer also read the saved negative proof at evidence/p3-th1/red-th1-false-2dc7dacf.out.
P3 changed the constant to false for that focused check. The suite compiled and ran one transcript.
It failed conf::th_1_core_is_send_not_sync at seed 0, step 0 with the expected type-check mismatch.
The artifact reports zero passed and one failed. The reviewer ran no check.
The PR initially called aggregate tier test totals conformance counts. P3 corrected that wording at the unchanged head.
The reviewer verified the corrected body. No description correction remains open.

### Pending removal and completed evidence

The pending list removes only conf::th_1_core_is_send_not_sync.
The transcript contains one type_check: send_not_sync step. The replacement map at 0389165 classifies its proof as static.
Plan 23a permits this static ID to leave pending when the required proof passes. It requires no process fixture.
The reviewer independently counted the minimum list against both pending lists: base 34/70, head 35/70.
The gate runs and passes this exact ID. It reports 104 active conformance IDs passed and zero failed.
The real minimum remains 0/70. The aggregate slow-tier total is not a real-harness conformance count.

Gate: ~/botster-sessions/gates/botster-core-stage1-p3-th1-send-not-sync-2dc7dacf-pool-20261009-154114-17192.log.
Shared copy: ~/botster-sessions/shared/core-stage1/evidence/p3-th1/gate-2dc7dacf.out.
The gate names the exact head and base. It runs on msa1, kernel 6.12.111+deb13-amd64.
All ten CI steps pass. The default tier passes 1152 tests in 6.823 seconds.
The slow tier passes 249 tests in 10.121 seconds.
The extended harness proof and conf::th_1_core_is_send_not_sync run and pass in the default tier.
Signals scan 165 Rust files. Timers scan 149 Rust files.
The ledger retains 675 IDs: 567 pending, two deferred, two withdrawn, and 104 active.
The report retains 510 pending and 57 without transcripts.
Both mutation runs test four mutants: three caught, one unviable, zero missed, and zero timeouts.
Both runs record a 20-second timeout. The separate run uses NEXTEST_PROFILE=slow.
The fuzz step passes because the diff changes no crate with a decoder harness.
Full CI takes 108.9 seconds. Separate mutants take 70.9 seconds. The gate exits 0 after 189 seconds.
The source adds no real-process fixture, sleep, production reaping change, guard change, or mutation exclusion.

### Verdict and retained scope

PR #202 is CLEAN at `2dc7dacfd90468bc55474ff020ca58312a66cd42` for the package review assigned to the P3 reviewer.
No package finding remains. Integration requires its own exact-head verdict because the tier is HIGH.
No NOT CLEAN round or round-limit notice is needed for this PR.
The lead's accepted 23h assignment sends this pair to P4a next. P3's non-minimum queue stays parked.
The reviewer read the P4a clause list and inventory. P3 will supply the P4a design note before its first PR.
#200 retains round 130 CLEAN, integration 80edcf86, and its merge at 3fa51cd2. F70 remains CLOSED.
F39 remains OPEN for #163's guard merge change and its registration and anchor-owner wait bounds.
#192 retains F61/F62. Earlier guard, bounded-accept, and process-proof carry items retain their recorded scope.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier findings and exact-head verdicts remain preserved.

VERDICT: CLEAN


## Round 132 — PR #203 P4a stream-route design — 2026-10-09

Reviewed head: `96fa437426b05c2743f0bb2c30731e30d0323f55`.
Base: `cc2e86ee356c7adcfc9ab95dd50494ae98429a45`.
Parents: `fc1d85d40d1b437d0ea5a22ec2620bd63cf44112` and the base.
The head contains the current v1 base. Its one-file design delta is unchanged by the base merge.
Tier: STANDARD under BUILD.md's documentation example. The PR changes no code, pin, configuration, or production behavior.
The reviewer asked P3 to add that tier and rule to the description, which currently omits them.
Authority: BUILD.md, accepted plan 23h, pinned contracts v0.1.20 at 03891658, and the lead's stream-only A17 direction.
The reviewer read the full description and 80-line design addition, host handoff and writer paths, link framing/messages,
testkit descriptor queues, worker model suffix handling, codec frames, and OU-2b/3/4/7/9 and DP-2/3 clauses.
Integration independently reported the capacity, splitting, failed-close, descriptor-order, and consumed-cut concerns.
No integration verdict is required by STANDARD. Integration supplies independent design feedback at the lead's request.

### F71 — MEDIUM — Bounded output lacks source backpressure and frame splitting; OPEN

DESIGN.md:100-129 proposes a bounded queue but gives no PTY capacity/read gate.
OU-3d requires source backpressure while a progressing reader is behind. OU-7 requires a lossless PTY tail for those readers.
A full queue cannot preserve output merely by declaring that queue bounded.
Line 124 also makes each PtyOutput one output frame per route. A read can exceed that route's max_frame_bytes minus the type byte.
Required: define bounded splitting without changing bytes or order. Define the PTY read/capacity gate and retained-input bound.
Preserve progress for other routes, release source backpressure for stalled routes, and keep the exit-tail queue fence.

### F72 — MEDIUM — Every close waits for writes, including failed routes; OPEN

DESIGN.md:104 and 114-115 requires completing a partial frame and route_closed before closing every transport.
A failed transport cannot complete those writes. An expired stall must not wait indefinitely for them.
OU-2b permits failed routes to lack route_closed and requires the corresponding host failure report.
The proposed RouteClosedByPeer also combines read EOF and every I/O error without a distinct write-failure result.
Required: define healthy close/drain and immediate failed close/report paths. Preserve the first reason and one close/report.
Keep transport_lost and write_failed distinct. Keep OU-4's frame boundary for writes that can complete.

### F73 — MEDIUM — Descriptor pairing lacks an enforced input and write order; OPEN

DESIGN.md:85-92 concludes that sending a descriptor first makes it present when AttachRoute is decoded.
The testkit stores descriptors and link bytes in separate queues. The simulation can choose ready inputs in different orders.
Queue insertion order alone does not guarantee Descriptor reaches the machine before the corresponding LinkBytes.
The real proposal also puts ancillary bytes before AttachRoute without defining how they share the existing outbound writer.
An independent byte send can cross an earlier partial control frame. The existing link decoder accepts only framed bytes.
Required: define descriptor-before-matching-message delivery at the driver boundary and one ordered outbound mechanism.
Define handoff-success feedback to the host engine and cleanup for failed, closed, or replaced links.
FIFO pairing is acceptable with these invariants. A descriptor ID is not required solely to solve this order problem.
Keep AttachRoute in the host's normal output order. The edge must not independently encode and send that host message.
The real transfer still requires its named real-process proof before production handoff code can merge.

### F74 — MEDIUM — The baseline cut loses pre-bind bytes retained outside the native model; OPEN

DESIGN.md:119-125 takes the baseline at model point R and forwards only later PtyOutput reads.
The existing worker can retain an unfed ESC outside the native model after a PtyOutput read completes no step.
The snapshot does not contain that byte. A later read can complete the sequence.
Forwarding only that later read omits the ESC on the client and violates OU-9's no-gap rule.
Required: define R as the consumed model cut and include its retained suffix exactly once in route output after the baseline.
Preserve byte order and no duplication across multiple reads, query stops, and captures inside partial sequences.

### F75 — MEDIUM — Baseline refusal checks only the screen frame bound; OPEN

DESIGN.md:122-123 closes SnapshotTooLarge only for a screen above max_screen_frame_bytes.
OU-9 separately forbids any offered native snapshot above max_snapshot_bytes, with no partial baseline.
A permitted max_screen_frame_bytes can exceed max_snapshot_bytes plus one. Its frame bound therefore cannot replace the native snapshot bound.
Required: define both bounds, with the type byte included only in the frame bound.
Complete snapshot fit checks before emitting a partial baseline. Preserve history page bounds and the bounded baseline queue.

### F76 — MEDIUM — The P4a feature lacks its required prior-art note; OPEN

The existing prior-art section covers P3 M1. The new P4a section contains no P4a prior-art note.
BUILD.md requires the feature note or PR body to name existing work, reused work, and rejected mechanisms with reasons.
P3's separate inventory identifies the existing codec/edges and the old daemon route code, but neither description incorporates that evidence.
Required: add a P4a prior-art note. Name the reused codec and edge tools and the rejected old route mechanisms and reasons.
Record the reason for any new hand-written infrastructure. This does not authorize copying old mechanisms or tests.

### F77 — MEDIUM — The resync sequence and discard classes are undefined; OPEN

DESIGN.md:111-113 describes resync as a queued item with baseline content.
The pinned codec's Resync holds a reason. OU-9 requires resync{reason}, a fresh baseline sequence, and then live.
Required: define that sequence at the next frame boundary after completing any started frame.
Discard only the permitted unstarted output and baseline frames. Retain input_refused, input_done, and route_closed.
Preserve the bounded modes and terminal_query rules. A stalled route's recovery must not break framing or lose mandatory results.

### Evidence, questions, and verdict

The reviewer sent F71-F77 directly to P3 and integration at this exact head.
The answer to question 1 is conditional FIFO pairing with F73's explicit invariants; no new wire ID is inherently necessary.
The answer to question 2 is host-engine success feedback followed by the normal ordered AttachRoute path.
The doc changes no product code and removes no pending ID. Minimum counts remain testkit 35/70 and real 0/70.
P3 first reported only a local fmt check under the lead's original no-gate instruction.
P3 then reported the lead's correction: a full gate is required for this documentation PR and is running at 96fa4374.
No completed gate evidence is available for this head at this round. The deterministic design findings already prevent CLEAN.

PR #203 is NOT CLEAN at `96fa437426b05c2743f0bb2c30731e30d0323f55` for the package review assigned to the P3/P4a reviewer.
F71-F77 MEDIUM are OPEN. This is #203's first recorded NOT CLEAN round. No round-limit notice is due.
These corrections remain in the implementer/reviewer loop. They are not a BLOCKED report to the lead.
#202 retains round 131 CLEAN and its merge at cc2e86ee. #200 retains round 130 CLEAN and F70 CLOSED.
P3's non-minimum queue stays parked under accepted 23h. Earlier F39, F61/F62, and scoped carry items remain preserved.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier exact-head verdicts remain preserved.

VERDICT: NOT CLEAN


## Round 133 — PR #203 P4a stream-route design corrections — 2026-10-09

Reviewed head: `c1d26778ded49180a310acf4f75e409a36161fe7`.
Base: `cc2e86ee356c7adcfc9ab95dd50494ae98429a45`.
Parent: `bcbd1d5bff0d6a343fc2cc4e5fc80253db80b39e`.
The head contains the current v1 base. Its complete delta adds 230 lines to DESIGN.md only.
Tier: STANDARD under BUILD.md's documentation example. The corrected PR body names that tier and rule.
The PR changes no code, pin, configuration, or production behavior.
Authority: BUILD.md, accepted plan 23h, contracts v0.1.20 at 03891658, and the lead's stream-only A17 direction.
The reviewer read the complete design, all corrections since round 132, full PR body, and completed exact-head gate.
The reviewer checked OU-1/2b/3/4/7/9, DP-2/3/5, and EV-8(i) against the design.
Integration supplies independent feedback at the lead's request. STANDARD does not require an integration verdict.

### F71 — MEDIUM — CLOSED as a design correction

The design adds a cumulative PtyReadBudget. Zero stops reads and drains.
The budget uses free payload capacity of Open routes and excludes Stalled routes.
The worker recomputes it after progress, stalls, and closes. Output splitting uses each route's frame bound.
These commitments address bounded source backpressure, lossless progress, and frame splitting.
The exit drain obeys the same budget. A stalled route stops holding the drain.

### F72 — MEDIUM — CLOSED as a design correction

Healthy close completes a started frame and writes route_closed last before RouteClose.
SessionEnded and SessionRemoved retain the whole queued tail for a progressing route.
The design fixes the session-end reason at Exited and preserves the first reason through later failure or timeout.
A healthy close retains the stall rules. A stall alone does not close before stall_close_after.
Resume completes the started frame, resyncs, delivers the remaining queue, and writes route_closed last.
An expired stall closes without more frames. StallTimeout applies only when no earlier close reason exists.
Failed transport closes immediately. The machine receives distinct terminal write and read errors.
The driver filters WouldBlock and retries Interrupted. These temporary conditions do not feed WriteFailed or PeerClosed.
The worker emits one RouteClose and one host report with the first reason.

### F73 — MEDIUM — CLOSED as a design correction

A marked AttachRoute frame shares the host's ordered outbound writer with every earlier control frame.
The descriptor travels with the frame's first bytes. Positive progress consumes the mark and transfers ownership.
Later writes send only the remaining bytes and never send the descriptor again.
Blocked returns the endpoint to the mark for retry without progress. Failed returns it for closure and drops the unstarted frame.
Success feeds HandoffSent. Permanent failure feeds HandoffFailed. Link closure cleans pending marks and endpoints.
The tagged testkit receive delivers Descriptor before matching LinkBytes. FIFO pairs descriptors in that enforced order.
LinkClosed and AdoptLink close old unbound descriptors. FIFO needs no new wire ID with these order and cleanup rules.
The design adds proof cases for blocked retry, short writes, simultaneous readiness, earlier partial frames, and cleanup.
The real handoff remains outside the P4a testkit PRs and requires its named real-process proof before production code merges.

### F74 — MEDIUM — CLOSED as a design correction

R is the consumed model boundary. The retained unfed suffix follows live before later PTY output, exactly once.
Attach and resync use the same suffix rule. This closes the pre-bind ESC gap.
The design names the retained-suffix proof, including a lone ESC.

### F75 — MEDIUM — CLOSED as a design correction

The worker checks native max_snapshot_bytes before it queues any baseline frame.
It preserves the applied per-route max_screen_frame_bytes, including larger valid host choices.
The default and minimum are max_snapshot_bytes plus one. The frame check uses actual snapshot payload size plus one.
The applied frame limit appears in attached.limits. The native payload bound remains a separate check.
A refusal emits no partial baseline. The size check occurs before frame allocation.
The full PR body now states the corrected applied-limit rule.

### F76 — MEDIUM — CLOSED as a design correction

The P4a prior-art note names reused codec, edge, testkit, admission, snapshot, and consumed-cut tools.
It names rejected old daemon mechanisms with reasons and excludes a host relay and WebRTC.
It gives the reason for FIFO without a new descriptor field. The route machine follows the worker's sans-IO design.
This meets BUILD.md's feature prior-art note requirement.

### F77 — MEDIUM — CLOSED as a design correction

The design defines resync{reason}, a fresh baseline, then live after a started frame completes.
It separates droppable output/baseline frames from kept input_refused/input_done/route_closed frames.
Modes and terminal_query retain their DP-5 bounds.
At stall discard and resync, an affected query loses its route opportunity and its unstarted terminal_query frame retires.
The worker uses the saved parse-point shadow fallback. A full lane leaves the query pending on the fallback path only.
Late client replies are query_expired before fallback admission and already_replied after it.
Sent queries, or started queries that finish, keep their opportunity across resync.
This explicit exception replaces the former blanket retention of all bounded frames.

### Completed evidence and verdict

Gate: ~/botster-sessions/gates/botster-core-stage1-p4a-design-c1d26778-pool-20261009-160005-69360.log.
The saved gate names this exact head and base. It ran on msa1, slot 0, kernel 6.12.111+deb13-amd64.
All ten full CI steps PASS. Default: 1152 tests in 6.301 s. Slow: 249 tests in 10.119 s.
Active conformance: 104 passed, zero failed. Ledger: 675 IDs, 567 pending, two deferred, two withdrawn.
Signals scan: 165 Rust files. Timers scan: 149 Rust files.
Both mutant steps PASS and report no Rust source change. They create no mutant outcomes for this documentation delta.
Full CI: 36.8 s. Separate slow-profile mutants: 0.4 s. Gate exit: zero after 44 s.
No pending ID is removed. Minimum counts stay testkit 35/70 and real 0/70.
The documentation gate does not prove future implementation. Each P4a implementation PR needs its own review and required proofs.

During this round, the reviewer sent remaining findings at b23c663b and 30738078 directly to P3 and integration.
The implementer supplied further READY heads before the reviewer committed a verdict.
Those intermediate heads have no separate package verdict. This round records only the final exact head above.
Integration's published 047c0551 remains NOT CLEAN at 30738078 for its earlier F72/F77 scope.
That older integration verdict does not describe the corrected c1d26778 head.

PR #203 is CLEAN at `c1d26778ded49180a310acf4f75e409a36161fe7` for the assigned package design review.
F71-F77 are CLOSED as design corrections. No package finding remains at this head.
#203 has one earlier recorded package NOT CLEAN round. No package round-limit notice is due.
#202 retains round 131 CLEAN and its merge at cc2e86ee. #200 retains round 130 CLEAN and F70 CLOSED.
P3's non-minimum queue stays parked under accepted 23h. Earlier F39, F61/F62, and scoped carry items remain preserved.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier exact-head verdicts remain preserved.

VERDICT: CLEAN


## Round 134 — PR #206 P4a PR1 route machine and stream handoff — 2026-10-09

Reviewed head: `1484a9ea50d6553d27db1524d8f6393ffb4477fb`.
Base: `9b255cff007552f3952563852104f00a14ddf842`.
Parent: `ed6ed38b86c0c8f2eedb21660a42c5fe42e0ac8c`.
Tier: HIGH under BUILD.md rules 1, 3, and 5: mutation configuration, shared crates/workspace dependencies, and handoff/real paths.
The PR body states HIGH and names its shared-crate scope. The head contains the current v1 base.
The delta changes 33 files, with 2453 insertions and 160 deletions.
Authority: BUILD.md, accepted plan 23k at 2cec1a39, contracts v0.1.21 at 60a4169, and the lead's P4a split and limits ruling.
Plan pin: stage1-plan.73969100.md, sha256 73969100d06b36134bf849a42e1511b6ab706ad5899e30cc77a542ee920f952b.
The minimum list now contains 69 IDs. The earlier 70/71 counts are historical, not the current denominator.
The reviewer read the complete PR body, changed source and proofs, design corrections, pinned transcripts/map, and completed gate.
The reviewer checked the host writer and marks, descriptor queues, worker baseline/output paths, testkit binding/client, and real stubs.

### Accepted scope and authority

The lead's option (b) adds AppliedRouteLimits to AttachRoute. The host computes those limits once and sends the returned values.
The worker announces the supplied values. The design and PR body name the link-message correction.
The ordered host writer places each descriptor on its frame's first bytes and completes earlier partial frames first.
Blocked returns endpoint ownership to the mark. Failed closes the endpoint and removes the unstarted frame.
The testkit tags descriptor offsets and feeds Descriptor before matching LinkBytes.
The worker pairs descriptors in FIFO order and cleans unbound descriptors on link closure/adoption.
The baseline uses the consumed model cut and forwards the unfed suffix before later output.
These mechanisms have useful source proofs. The findings below prevent acceptance of the complete implementation at this head.

The body identifies history, HandoffSent, route input, and real handoff as later work in the approved P4a sequence.
The real edge still refuses every descriptor send. The packaged worker receives no route descriptor and ignores the new actions.
This PR implements no successful real fd transfer. Its named real-process transfer proof remains required before that production behavior merges.
The real-edge mutation exclusion follows only the function rename and retains its existing named slow failure proof.
The prior-art note still identifies reused codec/edges and rejected old mechanisms with reasons.

### F78 — MEDIUM — Emitted frames do not enforce their applied bounds before allocation; OPEN

worker/route.rs:61-64 encodes and queues every control frame without checking its applied max_frame_bytes.
The applied-limits test uses max_frame_bytes = 9, but its Client decodes with u64::MAX for every bound.
It therefore accepts attached and baseline_begin frames larger than the limit that the worker announces.
DP-3 bounds every frame except screen/history by the route's max_frame_bytes and requires checking before allocation.
route.rs:79-82 also forces one payload byte when max_frame_bytes = 1, so output has length two and exceeds that legal cap.
Required: enforce the applicable bound for each emitted frame. Do not force a positive output payload when the cap cannot hold it.
Use actual applied bounds in boundary proofs, including small control caps and a cap of one.

The screen allocation also precedes its check: route.rs:189-195 calls Terminal::snapshot before inspecting either size bound.
Terminal::snapshot queries the native size, reserves/resizes that full size, and only then returns its Vec.
Checking the returned length is too late for the pre-allocation rule.
Required: use the native size query to check the applicable native/frame bound before buffer reservation.
Keep the library's encoder and format. This does not authorize a hand-written snapshot or changed transcript expectation.

### F79 — MEDIUM — Baseline insertion exceeds the route queue bound; OPEN

route.rs:150-154 queues every baseline frame before send_budget. Route::push increments queued without a bound check.
baseline builds all frames and adds the retained unfed suffix at once.
A valid route_queue_bytes equal to max_snapshot_bytes can hold an exactly fitting native snapshot.
This code also queues attached, baseline controls, stream framing, and any retained suffix, so queued exceeds route_queue_bytes.
The later zero PTY budget does not undo that already excessive queue.
Core 9B bounds frames queued per route. OU-9 guarantees that a permitted snapshot can fit the queue.
Required: define and enforce bounded baseline formation/delivery, including its byte accounting and retained suffix.
Do not reject a permitted native snapshot merely because surrounding frame overhead shares this eager insertion.
Add a proof for the tight valid queue bound and a retained suffix, with queue usage checked throughout delivery.

### F80 — MEDIUM — The testkit client loses a partial-write suffix; OPEN

route_client.rs:43-49 keeps the suffix only in a borrowed local slice and returns on WouldBlock or any other error.
The TestkitRoute object retains no unsent bytes. A full stream therefore loses the suffix.
Its test writes abcdef into capacity four, accepts abcd, then writes and accepts g. The missing ef is never retried.
The void RouteClient::write returns without an incomplete-write result, so a transcript can silently lose input.
The comment and PR body describe retained backpressure, but the code drops the bytes.
Required: retain and retry the suffix in order, or fail explicitly while the consumer remains unsupported.
Never return as a completed write after losing bytes. Test complete ordered delivery after partial acceptance.
This finding does not require implementing the deferred route-input consumer in this PR.

### F81 — MEDIUM — Interrupted route writes become terminal failures; OPEN

testkit worker.rs:1335-1341 handles WouldBlock, but maps every other error to a terminal RouteWritten Err.
EndControl can inject Interrupted. That input then closes the route WriteFailed instead of retrying.
The new Input contract and accepted design explicitly require the driver to retry Interrupted.
Required: retry the same outstanding bytes on Interrupted without advancing progress or emitting a terminal failure.
Add an edge proof that injects Interrupted, then succeeds, with every byte delivered once.

### F82 — MEDIUM — Write failure overwrites an earlier close reason; OPEN

route.rs:276-280 always calls end_route(WriteFailed) for a terminal write error.
A baseline refusal already sets closing to SnapshotTooLarge or BadPeer before delivering route_closed.
If that write fails, the host receives WriteFailed instead of the stored first reason.
OU-2 requires the first reason and exactly one close/report. The accepted design states the same rule.
Required: preserve an existing closing reason when a subsequent write fails, while closing the failed transport immediately.
Add a proof for failure while a healthy close is queued or partly written, with one report of its first reason.

### Completed evidence and verdict

Gate: ~/botster-sessions/gates/botster-core-stage1-p4a-route-machine-1484a9ea-pool-20261009-170209-10406.log.
The log names this exact head and base. It ran on msa1, slot 1, kernel 6.12.111+deb13-amd64.
All ten full CI steps PASS. Default: 1303 tests in 8.227 s. Slow: 254 tests in 21.887 s.
Active facade conformance: 116 passed, zero failed. Ledger: 679 IDs, 559 pending, two deferred, two withdrawn.
Signals scan: 182 Rust files. Timers scan: 180 Rust files.
Both mutation runs test 147 mutants: 135 caught, 12 unviable, zero missed, zero timeouts.
Separate slow-profile mutation timeout: 20 s. Full CI: 493.2 s. Separate mutants: 271.5 s. Gate exit zero after 775 s.
The gate supplies completed evidence but does not resolve the source findings.

The two removed pending IDs are ou_9_baseline_then_live_no_gap and dp_3_screen_is_one_frame_within_max_screen_frame_bytes.
Both replacement-map rows at 60a4169 permit core-testkit proofs. The reviewer read both pinned transcripts.
The reviewer independently counted testkit minimum 40/69 at base and 42/69 at head. The corrected PR count is accepted.
The real minimum count does not increase from these testkit removals. No successful real handoff is claimed.
The terminal-format ID stays pending. The pinned runner merges attach options without binding substitution, as the body reports.
That runner issue does not excuse F78-F82 or authorize a transcript change in Core.

The reviewer sent F78-F82 directly to P3 and integration with this exact head.
Integration independently confirms F80-F82 and is checking F78/F79 before its published verdict.
PR #206 is NOT CLEAN at `1484a9ea50d6553d27db1524d8f6393ffb4477fb`.
F78-F82 MEDIUM are OPEN. This is #206's first recorded package NOT CLEAN round. No round-limit notice is due.
These findings stay in the implementer/reviewer loop. They are not a BLOCKED report to the lead.
#203 retains round 133 CLEAN at c1d26778 and merge cd97009e. F71-F77 remain CLOSED as design corrections.
The new implementation findings do not rewrite that exact-head design verdict.
P3's non-minimum queue stays parked. Earlier F39, F61/F62, and scoped carry items remain preserved.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier exact-head verdicts remain preserved.

VERDICT: NOT CLEAN


## Round 135 — PR #184 HIGH-path list union with #181 — 2026-10-09

Reviewed head: `03df8a1f31ab079adb18d4f51999b8421911baa1`.
Base: `b453b449a959f7e10b94733b4425b4903d6a4890`.
Parents: `655f17546d823d7432ce4f3ea239884438c58f78` and the base.
Tier: HIGH under BUILD.md rules 1 and 3: the HIGH-path list, gate code, and workspace mutation configuration.
The full corrected PR body states HIGH and requires package and integration reviews.
Authority: BUILD.md, accepted plan 23k, the plan 23g mutation-review rule, and the lead's #181 union order.
The reviewer read round 110, the complete final five-file change, the #181 union resolution, body, and exact-head gate.
The reviewer inspected high_tier, its tests, command registration, taint_job wiring, list coverage, and exclusion citations.
The final delta changes five files with 261 insertions and eight deletions.
The head contains the current base. None of #184's five files changes between 655f1754 and this head.
The reviewer compared the two complete own diffs against their respective bases: they are byte-identical.
This is an explicit package delta review, not a claim that the old CLEAN alone covers the base merge.

### F57 remains CLOSED; source union is accepted

The 25 rule-5 entries preserve round 110's accepted source coverage, including its six process/adoption additions.
The list now adds .cargo/mutants.toml as the plan 23g exception. The header states that exception explicitly.
All 26 entries match paths in this exact head. Reasons preserve the rule-5 priority over generic sans-IO examples.
The list does not infer an I/O classification or weaken the independent rules 1 through 4.
Future rule-5 code must extend the list when it adds a new file. The existing whole-file entries retain their accepted scope.

The command still reads the list and git's tracked paths, then propagates the tested verdict/outcome result.
Validation still rejects stale paths, missing reasons, invalid glob forms, and directory-prefix siblings.
The union registers high_tier alongside #181's checks and runs it after gate_decisions in taint_job.
The module/import/command lists preserve both branches. No existing check is removed.
The repository-list proof additionally walks .cargo because the general tree walk skips hidden directories.
This makes its test input include the new listed mutation file even in a mutation copy without git metadata.

The taint_job exclusion reason now names high_tier and its tested decisions.
The new command exclusion covers only the whole-body replacement with Ok(()).
Its reason cites verdict with its seven named proofs and outcome with its step-result proof.
The strict decision/proof form meets #181's citation rule. No verdict, matches, or outcome decision is excluded.
The reviewer retains round 110's accepted I/O-shell scope. The union changes no runtime product behavior or real-process fixture.

### F83 — LOW — CLOSED in this round

The initial 655f1754 body still named 25 entries, excluded .cargo from the list, and presented b9c3ea2a's gate as current.
The reviewer sent that body finding to P3 and integration before a verdict commit.
The final body states 26 entries, the plan 23g exception, the #181 union, strict citations, and the .cargo test walk.
It names the reviewed 03df8a1f head, current base, and completed gate.
It keeps b9c3ea2a and b40e9bc3 as historical evidence. That correction changes no source or gate input.
F83 is CLOSED. No package finding remains at this head.

### Completed evidence and verdict

Gate: ~/botster-sessions/gates/botster-core-stage1-p3-high-tier-paths-03df8a1f-pool-20261009-172559-13907.log.
The log names the exact reviewed head and base. It ran on Gaming, slot 0, kernel 6.18.40.1-microsoft-standard-WSL2.
All ten full CI steps PASS. Default: 1285 tests in 10.668 s. Slow: 254 tests in 22.928 s.
Active conformance: 121 passed, zero failed. Ledger: 679 IDs, 554 pending, two deferred, two withdrawn.
Signals scan: 178 Rust files. Timers scan: 176 Rust files.
The log reports 166 checked citation names and 1104 gate mutants against 189 regex exclusions and two glob exclusions.
The high-tier step prints its pass result. All eight high_tier tests are selected and PASS.
Both mutation runs test and catch all 13 mutants, with zero missed, timeouts, or unviable.
The separate run uses the slow profile and a 20 s mutation timeout.
Full CI: 339.7 s. Separate mutants: 98.3 s. Gate exit zero after 707 s, including 192 s queue time and 515 s run time.
This PR changes no pending ID and produces no testkit or real minimum gain.

PR #184 is CLEAN at `03df8a1f31ab079adb18d4f51999b8421911baa1` for the assigned package union review.
F57 remains CLOSED. F83 is CLOSED. No package finding remains.
Integration must publish its own same-head union verdict under HIGH.
#184 has one earlier recorded package NOT CLEAN round. No package round-limit notice is due.
The intermediate 655f1754 review has no separate committed package verdict.
Round 110 CLEAN at b9c3ea2a remains preserved at its historical head and scope.
#206 retains round 134 NOT CLEAN at 1484a9ea, verdict ba5c9ebc. F78-F82 remain OPEN.
P3's non-minimum queue stays parked. Earlier F39, F61/F62, and scoped carry items remain preserved.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier exact-head verdicts remain preserved.

VERDICT: CLEAN


## Round 136 — PR #206 P4a route machine corrections and v0.1.22 union — 2026-10-09

Reviewed head: `959a716cf9b918c1a7fa8845a7ca8ed15c3f87b4`.
Base: `1627732f5f651f5946e5d60b64dedcbf6c4cb683`.
Parents: `b0d50044e52b0f4c000add8a30344cdb5f6fa0b3` and the base.
Tier: HIGH. The change crosses host, link, testkit, worker, and terminal crates and changes the mutation configuration.
Authority: BUILD.md risk rules and real-process rules; accepted plan 23n at f545597b; contracts v0.1.22 at af5771c.
The reviewer read R-44 at dede41d7797f20d1fb0ee7d4720e4d7a09cc37e3.
The lead supplied R-45 during this round and instructed the reviewers to act before its documentation lands.

The last package verdict for #206 was round 134 at 1484a9ea, NOT CLEAN, commit ba5c9ebc.
No package verdict exists here for b0d50044. This round reviews all corrections from 1484a9ea through the reviewed head.
The reviewer read the full final own change and the final merge corrections, including the public API snapshot.
The own change covers 36 files with 2808 insertions and 170 deletions.
The public API snapshot follows the exported RealEdges method rename; it adds no unrelated API.
The five final A17 removals preserve v0.1.22 and its stream-only contract. The hub conformance dependency also uses v0.1.22.
The ordered host writer, applied limits sent once by the host, descriptor ownership, and model cut retain their accepted design.
The prior-art note preserves reused codec/library mechanisms and rejected old mechanisms with reasons.
The real edge still refuses descriptor sends. The packaged worker still implements no successful route descriptor transfer.
The named real-process proof remains required before successful production transfer merges.
No test-process guard, anchor, wait, timeout, or cleanup rule changes in this correction.

### F78 — MEDIUM — CLOSED

Terminal::snapshot_at_most checks the native size before buffer reservation. CaptureSnapshot and route baseline use this method.
The method retains the native encoder, format, and continuation errors. Its boundary proof compares the native snapshot at its exact size.
The worker checks each encoded baseline frame against bound_of before queuing it. Screen retains its separate frame bound.
The route test client now decodes with the actual applied bounds.
A route below its attach-frame bound sends no frame and reports HandoffFailed once, as R-44 requires.
A route with no common terminal format follows that same path.
A SnapshotTooLarge close sends its typed frame only when the frame fits. Otherwise the transport closes without a frame.
The output payload no longer forces one byte. A successful baseline establishes a frame bound that permits output.
The exact-largest-frame proof succeeds at the measured bound and fails one byte below it.
These corrections close the emitted-frame and native-allocation finding.

### F80, F81, and F82 — MEDIUM — CLOSED

F80: TestkitRoute retains unsent client bytes and sends them before later bytes.
Its capacity-four proof now delivers abcd, then efg. A later read also flushes retained bytes.
A failed stream clears retained bytes and accepts no further client writes.
The stream implementation returns WouldBlock rather than Ok(0) for a nonempty write with no room.

F81: the worker route-write binding and TestkitRoute retry Interrupted. WouldBlock returns without an error loop.
The binding proof injects Interrupted and observes the complete write. The client proof observes unchanged ordered bytes.

F82: on_route_written retains route.closing when a later terminal write fails.
The proof starts a SnapshotTooLarge close, injects a write error, closes the transport, and reports SnapshotTooLarge once.
A route without an earlier close reason still reports WriteFailed.

### F79 — MEDIUM — OPEN under R-45; the old hard-cap remedy is superseded

OU-9 guarantees that a snapshot at max_snapshot_bytes fits when route_queue_bytes equals that limit.
Core 9B explicitly describes a route over route_queue_bytes. The prior finding's unconditional hard-cap remedy overstates that text.
The lead obtained the steward's R-45 clarification during this review:
- One baseline sequence, baseline_begin through live, including its screen frame and stream prefixes, is exempt from the threshold test.
- The threshold test counts only queued frames behind that sequence, including output after R and the held suffix as output.
- Baseline-only overage is not a not-progressing condition and starts no extra stall clock.
- The reader_progress_deadline rule remains unchanged. A transport that accepts bytes is progressing while the baseline drains.
- A tight-queue proof must deliver the baseline and live without resync. The ruling adds no ledger ID.

At this head, route.queued and free_payload count every queued byte without a separate baseline term.
The body and DESIGN.md permit the baseline controls, framing, and held suffix as a combined overage.
The held suffix is not part of R-45's exemption. The current test delivers the sequence but asserts only a zero PTY budget and final drain.
It does not establish the new threshold distinction or the required no-resync behavior.
P3 is correcting the source and proof under R-45. F79 remains OPEN for that new head.
This finding does not require a hard cap or rejection of a fitting snapshot.

### F84 — LOW — OPEN: stale runner statement and minimum count in the body

The body says v0.1.22 still sends literal $fmt in attach_route options.
The pinned af5771c includes ff5ac74, which substitutes the full attach_route specification before attach.
An unbound variable becomes a bad step. The unchanged merge inside attach does not negate that earlier substitution.
The package reviewer independently counted 69 minimum IDs: 41 pass at base and 43 pass at this head.
The body still states 40 to 42. Correct both statements without changing the transcript or claiming the pending format ID passed.

### F85 — MEDIUM — OPEN: manual slow-feature mutation evidence is not named

Plan 23n states that an env-only NEXTEST_PROFILE=slow xtask mutation run repeats the default mutation stage.
Until P6's gate fix lands, an exclusion based on slow mutation evidence must name its applicable manual log in the PR body.
The required run includes --no-config --in-place --features slow and -- --profile slow with the gate's fail-fast setting.
This PR renames the excluded RealEdges::handoff_route method to link_send_descriptor and updates its named slow failure proof.
The body names only the env-only second xtask run. That run does not provide slow-feature mutation evidence.
P3 must supply applicable manual evidence. Existing evidence needs explicit source, signature, and proof applicability.
This request is scoped to the exclusion. It does not request a broad gate rerun or reviewer execution.

### Completed evidence and verdict

Gate: ~/botster-sessions/gates/botster-core-stage1-p4a-route-machine-959a716c-pool-20261009-182804-79349.log.
The log names the exact reviewed head and base. It ran on msa1 and exited zero after 875 seconds.
Queue time was one second. Run time was 874 seconds. All ten full CI steps PASS.
Default: 1327 tests passed in 9.666 seconds. Slow: 254 tests passed in 22.941 seconds.
Active facade conformance: 123 passed, zero failed. The default facade run reports 559 ignored.
Signals scan: 183 Rust files. Timers scan: 181 Rust files. Citation check: 167 names.
Gate decision check: 1104 xtask mutants, 189 regex exclusions, and two glob exclusions.
Each mutation run reports 170 tested: 157 caught, 13 unviable, zero missed, and zero timeouts.
Full CI took 555.5 seconds. The separate mutation job took 300.4 seconds with a 20-second mutation timeout.
The two runs do not establish slow-feature mutation coverage under plan 23n.

The two removed pending IDs remain ou_9_baseline_then_live_no_gap and dp_3_screen_is_one_frame_within_max_screen_frame_bytes.
The minimum testkit count is 41/69 at base and 43/69 at head. The real minimum count gains nothing from these removals.
No successful real handoff is claimed. P4a must add its route transport term to edges_quiet in PR2.
History, HandoffSent, route input, and successful real transfer retain their named later scopes.

The reviewer sent findings directly to P3 and integration. The reviewer sent only QUESTION to the lead for F79.
The lead answered with R-45. Integration round 2 is NOT CLEAN at the same head, verdict 083087bab9a18763e5895395164e7e466c1e3e9c.
PR #206 is NOT CLEAN at `959a716cf9b918c1a7fa8845a7ca8ed15c3f87b4`.
F78/F80/F81/F82 are CLOSED. F79/F84/F85 remain OPEN. This is #206's second recorded package NOT CLEAN round.
No round-limit notice is due. Ordinary findings remain in the implementer/reviewer loop.
#184 is merged at 5348befa. Its round 135 CLEAN and F57/F83 closures remain preserved.
#203 retains its exact-head design CLEAN. F71-F77 remain CLOSED as design findings.
F39 for #163 and F61/F62 for #192 retain their prior scopes. P3's non-minimum queue stays parked.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier exact-head verdicts remain preserved.

VERDICT: NOT CLEAN


## Round 137 — PR #206 R-45 accounting and completed slow evidence — 2026-10-09

Reviewed head: `cb5b8822425a15b41337a7468a8b43a493a14501`.
Base: `1627732f5f651f5946e5d60b64dedcbf6c4cb683`.
Parent: `959a716cf9b918c1a7fa8845a7ca8ed15c3f87b4`.
Tier: HIGH. The full change crosses packages, includes the terminal FFI binding, and changes the mutation configuration.
Authority: BUILD.md; accepted plan 23n at f545597b; contracts v0.1.22 at af5771c; R-44 at dede41d.
R-45 is now published at contracts `7f7a4679cc29d2dc361c8b9a9da204bf2ecd1cd2`, docs/steward-rulings.md.
The reviewer read that fixed source and the lead's gate-evidence ruling of this round.

The reviewer read all five changed files since round 136, the corrected full PR body, and both completed evidence logs.
The correction contains 159 insertions and 41 deletions. The full PR contains 36 files, 2927 insertions, and 171 deletions.
The complete own change retains the source and union review from rounds 134 and 136.
No new base merge occurs in this round. The head contains the named current base.
The final change retains the applied limits, ordered descriptor handoff, model cut, native snapshot encoder, and failure stub scopes.
No prior-art choice, product process rule, guard, anchor, wait, timeout, or cleanup rule changes in this correction.
F78/F80/F81/F82 remain CLOSED with their round 136 proofs and scopes.

### F79 — MEDIUM — CLOSED under R-45

Route::baseline tracks the unwritten bytes through live at the front of the queue.
At attach, the worker sets that count before it adds the output after R.
free_payload charges queued minus baseline. The held suffix becomes output and remains in the charged bytes.
Every route write starts at the front. Each successful partial write reduces queued and baseline by the same accepted byte count.
baseline saturates at zero. Later output writes reduce only queued. The invariant baseline <= queued remains valid.
The queue budget therefore charges only frames behind the baseline while it drains.
The attached frame precedes baseline_begin and is not a frame behind the sequence. The design states this explicitly.
The exemption does not change the frame bounds or the native snapshot limit.

The worker proof uses route_queue_bytes = max_snapshot_bytes = the measured native snapshot size and retains an ESC suffix.
The proof observes the emitted PTY budget after attach. It then adds exactly the admitted output and observes the reduced budget.
The proof delivers the complete ordered baseline, live, held suffix, and later output. Its exact frame list excludes resync.
The old accounting would return a zero budget at attach and fail the changed proof.
The proof's budget helper repeats the arithmetic, but its inputs are measured snapshot bytes and actual queued output frames.
The action observations verify that the baseline is exempt and the held suffix and later output are charged.

The Core test opens with both limits at the measured snapshot size and attaches through the public API.
It reads attached, baseline_begin, an exactly fitting screen, baseline_end, and live, then Empty.
It observes no resync or route close. This is R-45's named tight-queue proof with no new ledger ID.
The body correctly states that this Core test alone is not red on revert of the exemption.
The worker proof supplies that distinction. Both tests PASS in the completed default gate.
This PR has no stall clock. Baseline occupancy starts no extra stall clock.
Later stall and resync work must preserve R-45 and the unchanged reader_progress_deadline.
The prior hard-cap remedy remains superseded. F79 is CLOSED.

### F84 — LOW — CLOSED

The corrected body states that v0.1.22 substitutes attach_route bindings before attach, as ff5ac74 implements.
The unchanged OU-1 transcript now passes and leaves core-pending.txt in this commit.
The reviewer read its pinned transcript and replacement-map row at af5771c. The row assigns a core-testkit proof.
The gate selects and passes ou_1_terminal_format_is_negotiated_against_the_target_worker.
The reviewer independently counted 69 minimum IDs: 41 pass at base and 44 pass at head.
The body reports that count and all three removed pending IDs. Active facade conformance is 124 passed, zero failed.
The three IDs are ou_9_baseline_then_live_no_gap, dp_3_screen_is_one_frame_within_max_screen_frame_bytes, and the OU-1 format ID.
The body claims no real minimum gain. No transcript or expectation changes in Core. F84 is CLOSED.

### F85 — MEDIUM — CLOSED

Manual log: ~/botster-sessions/gates/botster-core-stage1-p4a-route-machine-cb5b8822-pool-20261009-190622-24485.log.
The header names this exact head and base. The separate pool job exits zero after 15 seconds on msa1.
The command includes --no-config, --in-place, --features slow, --profile slow, and --max-fail 1:immediate.
Its scoped file and regex select RealEdges::link_send_descriptor. It uses the gate's slow selection and prebuilds the worker.
The unmutated baseline selects 51 slow tests and passes all 51.
Both method replacements, Ok(0) and Ok(1), are caught. The log shows no missed mutant or mutation timeout.
Both mutants fail real::slow_tests::the_edges_draw_random_bytes_and_refuse_the_handoff at the typed Failed assertion, real.rs:661.
The nextest failure summaries are ordinary assertion failures, not tests terminated at a deadline.
The body names this log, command, baseline, mutants, and proof. This supplies plan 23n's manual slow-feature evidence.
The source still implements only the real handoff refusal. Successful production transfer remains later work with a required named real-process proof.
F85 is CLOSED. The env-only repeated xtask mutation job is not treated as slow-feature mutation evidence.

### Completed gate evidence and lead ruling

Full log: ~/botster-sessions/gates/botster-core-stage1-p4a-route-machine-cb5b8822-pool-20261009-185035-66116.log.
The header names the exact head and base. The job ran on msa1 with zero queue time.
All ten full CI steps PASS. Default: 1329 tests passed in 8.549 seconds. Slow: 254 passed in 21.940 seconds.
Both facade runs report 124 passed, zero failed, and 558 ignored.
Full CI takes 539.9 seconds. The separate xtask mutation job takes 333.7 seconds.
Each mutation job reports 179 tested: 166 caught, 13 unviable, zero missed, zero timeouts.
The repeated mutation job uses a 20-second mutation timeout.

The pool command exits one after 885 seconds because an appended manual command combines incompatible --in-place and --jobs flags.
The argument parser refuses that appended command before it builds or tests anything.
Both CI commands precede it with || exit $? and complete with exit zero.
P3 reruns the manual command alone on the same head, producing the zero-exit log above.
The body reports this status accurately. It does not describe the full pool command as exit zero.

The lead reads the log and explicitly accepts these completed same-head results as gate evidence for #206 at cb5b8822.
The lead states that the gate commands were green and that only the appended manual command failed argument parsing.
The lead also requires all future manual evidence runs to use separate pool jobs, so the gate exit reflects only the gate.
This review relies on that explicit ruling. It does not infer a general exception for failed CI steps or skipped gates.

### Verdict and retained scopes

PR #206 is CLEAN at `cb5b8822425a15b41337a7468a8b43a493a14501` for the assigned package review.
F78-F82, F84, and F85 are CLOSED. No package finding remains at this head.
Integration must publish its own same-head HIGH verdict before merge.
#206 has two earlier recorded package NOT CLEAN rounds. No fourth-round decision or round-limit notice is due.
The package reviewer sends CLEAN with this exact head and verdict commit to the lead after push.

The real minimum count gains nothing from the three testkit removals. Successful real transfer is not implemented or claimed.
P4a must add its route transport term to edges_quiet in PR2. History, HandoffSent, route input, stall/resync, and real transfer retain later scopes.
#184 retains round 135 CLEAN and its 5348befa merge. F57/F83 remain CLOSED.
#203 retains round 133 design CLEAN and its cd97009e merge. F71-F77 remain CLOSED as design findings.
F39 for #163 and F61/F62 for #192 retain their prior scopes. P3's non-minimum queue stays parked.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier exact-head verdicts remain preserved.

VERDICT: CLEAN


## Round 138 — PR #217 P4a route lifecycle — 2026-10-09

Reviewed head: `8ae2643d8a2ddb1fffceda129c3775b3abfa0f88`.
Base: `e99012939c8df4f85da5ae5af7144534b129cb70`.
Parent: `50b53cce611b3eb166b579058d73974f850227b7`.
Tier: HIGH under BUILD.md rule 3. The change crosses worker, host, and testkit packages and adds shared controls.
Authority: BUILD.md; accepted plan 23q at bae71c81; contracts v0.1.24 at d79aed5; R-44 through R-48 and the lead's PR2 order.
The reviewer read the updated lead and implementer handoffs, full PR body, source change, tests, replacement-map rows, and completed gate.
The reviewer inspected closure, route input, admission, host completion, edge interests, controls, cause recording, and quiet-state composition.
The change covers 20 files with 1636 insertions and 78 deletions.
The approved PR2 scope includes basic bytes/text input. PR3 retains its later input controls and minimum work.
That scope does not authorize loss, unbounded retention, or missing failure reports in the paths implemented here.

### Accepted parts and retained limits

The worker queues a healthy close after existing frames and keeps a reason against a later Detach or write error.
Session exit queues the session_ended frame after the tail. Attach after exit queues baseline, live, then the ended frame.
The host's SessionLost path can close routes without a worker, with the host's typed cause and event order.
The handoff hold returns Blocked with the endpoint intact and preserves the ordered writer's mark.
The release clears the hold and signals the host wake. Another link is not held.
The failure hook takes no byte and returns the endpoint with Failed once. The driver owns its release.
Route controls act at the stream edges. The body states their real-tier forms and unresolved real-tier gaps explicitly.
R-47 route_fill and the portable fail_writes form remain tracked real-tier work; they are not acceptance exemptions.
If the real harness lands first, this PR must add its failing real IDs to core-real-pending with reason and owner.
The PR that lands second must adapt. No real passing count is inferred from the testkit flips.

The new read loop retries Interrupted and maps EOF or a terminal read error to one RouteEnded input.
A WouldBlock read returns an empty input and does not end the route.
The binding selects route reads only when readable matches registered read interest.
Sim::has_ready reaches WorkerEdges::ready, so edges_quiet now includes ready route reads and writes through that composition.
Its readiness term does not fix the missing queue allowances described in F88.
The pinned codec uses fixed refusal field names. The reviewer has no separate oversized-refusal-frame finding at this head.
The retained design prior-art note identifies reused codec, transport, stream controls, and admission mechanisms.
No process guard, anchor, wait, deadline, or cleanup rule changes. No production test hook is added.
Successful real transfer and the production route driver remain later scopes with required named real-process proofs.

### F86 — MEDIUM — OPEN: a peer close replaces the first route reason

worker/route.rs:527-528 calls end_route(PeerClosed) for every RouteEnded.
A prior Detach, BadFrame, or SnapshotTooLarge close can already set route.closing while its last frame waits.
A client EOF or read error at that point removes the route and reports PeerClosed instead of the first reason.
on_route_written already preserves route.closing for the corresponding write fault. The new read-fault path does not.
OU-2 requires the first reason and one transport close/report.
Required: preserve an existing closing reason while closing the failed transport immediately.
Prove a held healthy close followed by RouteEnded, with exactly one transport close and one report of the original reason.
Integration R1-1 independently identifies the same path, with HIGH integration severity.

### F87 — MEDIUM — OPEN: refused complete input can bypass receipt accounting

worker/route.rs:508-511 sends a refusal directly for Decoded::Unsupported or Decoded::Invalid.
Only on_client_frame calls client_received and reports Observation::ClientInput.
The codec returns those decoded variants for complete input with unknown enums, invalid input fields, or invalid UTF-8 text.
Those frames therefore leave client_rev unchanged and produce no client activity report, despite a known route and complete receipt.
IN-4 requires every complete client input frame to advance input_rev{client} on receipt and carry its route.
This also affects host input guards that compare the client revision.
Required: account complete client input in a common receipt path before application or refusal.
Keep ignored extension frames distinct from client input. Do not advance twice for a valid frame.
Prove unknown-enum and invalid-input refusal, correct route/revision activity, no PTY write, and an unaffected open route.
Integration independently confirms this finding as R1-4.

### F88 — MEDIUM — OPEN: the new route-read path has no queue allowance

Pending::Route stores each accepted input in the shared input queue without route byte accounting.
The binding registers read interest from !route.ended and reads up to 64 KiB each time.
It never receives a route_input_queue_bytes allowance from the machine.
When the PTY is blocked, continued bytes/text frames therefore accumulate without the contractual per-route input bound.

refuse unconditionally pushes another input_refused frame into the route's outgoing queue.
When the client floods refused input and does not read, those frames accumulate past route_queue_bytes.
The PTY read budget does not stop the route input reader. DP-5 explicitly requires that read stop when one more refusal cannot fit.
R-45 exempts the one baseline sequence; it does not exempt refusals or other frames behind it.

on_route_read also pushes bytes into StreamReader before it checks route.closing.
A gated closing route therefore retains every later client read without decoding or removing it.
The edge keeps reading because ended remains false until EOF or a read error.
This is unbounded retention even when the input application path has already stopped.

Required: implement byte allowances for the implemented route input and outgoing exception queues.
Stop transport reads when no allowance remains. Retain bytes already taken in order without exceeding the documented bound.
Handle closing-route bytes before decoder insertion, and stop or safely discard further input once the close is committed.
Prove blocked PTY input, refusal flooding against a blocked client, partial frames at the allowance edge, and a gated closing route.
Preserve R-45 accounting and the first close reason. Do not replace a full queue with byte loss or a new timeout.
Integration R1-2 confirms the closing-route retention path and includes the other queue paths after package feedback.

### F89 — MEDIUM — OPEN: the host retires ended routes before their actual close

flows.rs StopPhase::Finish now calls close_route(SessionEnded) after the session's state, one route per step.
Worker::report_exit posts Exited when the tail is queued, then queues each route's session_ended close.
end_route suppresses the later SessionEnded report, so the host has no completion boundary for actual route delivery.
With route_gate active, the host therefore retires a route while its tail, close frame, and transport still exist.
run.rs close_route also completes a waiting Detach. That completion can occur before the worker closes the transport.
It can replace the reason of an earlier worker Detach close with the host's SessionEnded reason.

OU-7 distinguishes queued output at Exited from each route's later delivery and close.
DP-7 completes Detach only after the route closes. The event-order unit tests do not hold a real testkit tail across this boundary.
Required: retain Exited-session routes until worker close completion or a typed failure/stall ends them.
Preserve the host's exit cause in the SessionEnded result and preserve an earlier route reason.
Use an explicit completion boundary between worker and host. Do not infer completion from Exited.
Prove a held tail and a pending Detach across payload exit, with no premature route retirement or Detach completion.
Keep SessionLost separate: that path has no worker left to deliver a completion.
The reviewer independently confirms integration R1-3 from report_exit, StopPhase::Finish, end_route, and close_route.

### F90 — MEDIUM — OPEN: route input loses ownership and lifecycle outcomes

Pending::Route contains only Vec<u8>. on_client_frame discards route/op identity when it calls enqueue_route_input.
try_start makes Active with req:None. finish_active reports only a host req, so a route write has no outcome destination.
A PTY error or payload end after a known partial bytes/text write therefore emits no input_refused with its route/op/written_bytes.
try_start also silently discards queued route bytes when the payload no longer runs.
DP-5 requires failure and session-ended exceptions when input is not applied as sent; successful bytes/text writes still remain silent.

input_fence clears the entire queue on host adoption. It cannot distinguish old-host requests from continuing route input.
A route transaction queued behind an active host write is therefore lost at adoption.
DP-8 keeps route transports and byte flow independent of the host's lifetime.

Required: retain route/op ownership through queue, active write, and completion.
Report the DP-5 exception with the correct known written_bytes for partial failure or session end.
Preserve continuing route transactions across the old-host request fence and maintain one ordered PTY admission point.
Prove partial bytes/text failure, queued input at payload end, and queued route input across adoption.
The reviewer independently reads the ownership, finish_active, and input_fence paths and confirms integration R1-5.

### Completed evidence and pending removals

Gate: ~/botster-sessions/gates/botster-core-stage1-p4a-route-lifecycle-8ae2643d-pool-20261009-205549-59478.log.
The header names the exact head and base. The job runs on msa1 with zero queue time.
All ten full CI steps PASS. Default: 1437 tests passed in 12.452 seconds. Slow: 256 passed in 21.854 seconds.
Both facade reports show 205 passed, zero failed, and 485 ignored.
Each mutation run reports 128 tested: 113 caught, 15 unviable, zero missed and zero timeouts.
Full CI takes 365.6 seconds. The repeated mutation job takes 255.4 seconds.
The job exits zero after 635 seconds. The botster-gate wrapper reports zero after 636 seconds.
The env-only second mutation job remains repeated default coverage. This PR adds no slow-based mutation exclusion.
The first gate's missed mutants and timeout are historical. The final gate reports none.
The tests now fail on an empty read while the controlled stream holds bytes, so that earlier mutant cannot cause an endless test loop.

The pending list removes exactly 23 IDs. All 23 replacement-map rows at d79aed5 permit core-testkit, edge, or perturb proofs.
None is slow:* or a real-only removal. The reviewer reads the held-tail transcript and its queued-versus-delivered boundary.
The reviewer independently counts minimum 46/69 at base and 54/69 at head, matching the body.
The passing transcripts and mutation result do not close F86-F90. They do not prove the missing fault and retention paths.

### Verdict and retained scopes

PR #217 is NOT CLEAN at `8ae2643d8a2ddb1fffceda129c3775b3abfa0f88`.
F86-F90 MEDIUM are OPEN. The reviewer sends all findings directly to P3 and integration.
This is #217's first recorded package NOT CLEAN round. No round-limit notice is due.
These findings remain in the implementer/reviewer loop. They are not a BLOCKED report to the lead.
The lead receives no ordinary NOT CLEAN findings under the reporting rule.

#206 is merged at 6cc7a722. Round 137 CLEAN at cb5b8822 and its F78-F82/F84/F85 closures remain preserved.
#215 is merged at 9103dca1 under P5's STANDARD package review. #216 pin v0.1.24 is merged at e9901293.
Accepted plan 23q is pinned at ~/botster-sessions/pins/stage1-plan.bae71c81.md.
Its counts are testkit-passing /69, real-passing /68, and real-accepted /69, with A20's allocator observation rule.
The real harness PR will add conformance/minimum-core.txt as the canonical list. Its initialization and later mutations retain their assigned reviews.
F39 for #163 and F61/F62 for #192 retain prior scopes. P3's non-minimum queue stays parked.
The reviewer changed no product code and ran no tests, builds, gates, measurements, or mutants.
All earlier exact-head verdicts remain preserved.

VERDICT: NOT CLEAN


## Round 139 — PR #217 replacement route lifecycle — 2026-10-09

Reviewed head: `557c414f3383dc357186972380291737a959f401`.
PR base: `e99012939c8df4f85da5ae5af7144534b129cb70`.
Parent: `c811fdd36dd392310a03172b031610f6705e9de0`.
Prior reviewed head: `8ae2643d8a2ddb1fffceda129c3775b3abfa0f88`, round 138.
Tier: HIGH under BUILD rule 3 because the PR changes the shared testkit and its controls.
All findings, including LOW findings, must close before this HIGH PR is CLEAN.
The reviewer reads the correction and the remaining change against BUILD, plan 23q, contracts v0.1.24, and the lead rulings.

### Scope and earlier findings

The replacement follows the lead's option A. It removes all route decode, admission, refusal, and read binding code.
worker/input.rs equals the PR base. The worker no longer accepts RouteRead or RouteEnded.
The testkit uses the PR1 read-interest behavior. The pending list restores the 12 input-dependent IDs from round 138.
Compared with the PR base, the pending list removes 11 IDs and adds none.
The body carries F86/F87/F88/F90 and the corresponding integration findings into the complete PR3 input unit.
F86/F87/F88/F90 close only within this PR2 scope. Their PR3 requirements remain open.
PR3 must cover receipt revision, route/op identity, every refusal, the adoption fence, both allowances, and read pause/resume.

F89's original premature-close fault is corrected at the normal completion boundary.
The worker reports SessionEnded for each route only after its queue is delivered.
The host records that route's completion and projects the host's exit cause when Finish posts the close.
The worker preserves an earlier detach reason across payload exit. The host completes the pending Detach at the reported close.
The host tests check no close before the report, independent completion of two routes, and a pending Detach across exit.
The worker tests check no report before delivery and one report for each delivered route.
This closes the original F89 source fault. F91 and F92 below identify faults in the replacement completion path.
The existing SessionLost path remains separate from normal worker delivery.

### F91 — HIGH — OPEN: some worker-loss paths never release ended routes

inbound.rs:677-716 handles ProcessExited. It calls close_worker_link when the worker still has a link.
flows.rs close_worker_link takes worker.link and removes self.links. It does not record RouteEnd::Lost.
The ProcessExited flow match leaves an existing Stop PostEnd or Finish unchanged.
If Exited already occurred and a bound route has no close report, next_ended_route cannot select that route.
finish_waits then keeps Finish waiting. A later LinkClosed finds no self.links entry and returns without recording route loss.
The route remains bound, and a Remove queued behind the end flow cannot run.

A second order has the same fault. Exited during Start stores pending_end.
If LinkClosed arrives before finish_start creates the Stop flow, the new loss block does not record the routes.
That block checks only Flow::Stop with an end. finish_start later begins Exited Finish with no live link and no route completion.
The body promises SessionLost when the link is lost before delivery. Neither order meets that promise or OU-7's close boundary.

Required: record route loss for every applicable worker/link teardown, including loss before the end flow exists.
Preserve an earlier Delivered record and an earlier route close reason.
Prove Exited with a held route, then ProcessExited before LinkClosed, and completion of the queued Remove.
Also prove Exited during Start, then LinkClosed before finish_start, with one SessionLost close.
The reviewer independently confirms integration R2-1 from the source. Integration grades this finding HIGH.

### F92 — MEDIUM — OPEN: Finish advertises work when the event queue is full

run.rs:98 includes only PostStopping and PostEnd in flow_needs_room for Flow::Stop.
The new Finish phase also posts a mandatory RouteClosed event through close_route.
When a route has a Delivered or Lost record, finish_waits returns false.
If the event queue is full, ready still offers Work::Session because Finish does not require event capacity.
StopPhase::Finish calls close_route, which returns false without changing the route or flow.
The next ready call offers the same work again. This violates the no-busy-work requirement.

Required: make the Finish route-close step wait for mandatory event capacity.
Do not block Finish work that needs no event capacity after the routes are closed.
Prove a full event queue with an eligible route close, no ready work, and polling that permits exactly one close.
The reviewer independently confirms integration R2-2 from ready, finish_waits, StopPhase::Finish, and close_route.

### F93 — LOW — OPEN: the PR description still names the removed scope

The PR title still says route input and 23 flips. This head removes route input and has 11 flips.
The body and flows.rs comment also say the launch handed every route to the worker.
flush_handoffs queues HandoffRoute actions. The driver can retain the endpoint at a blocked descriptor send.
A queued action does not prove descriptor delivery. The body already describes this hold correctly in its control table.
A released handoff can reach the exited worker and complete through its attach-after-exit path.
The reviewer does not claim a separate product fault for that held path.

Required: update the title for the final scope and count. State the difference between a queued handoff and descriptor delivery.
Keep the completion claim consistent with the driver's blocked-send behavior.

### Completed evidence

Gate: ~/botster-sessions/gates/botster-core-stage1-p4a-route-lifecycle-557c414f-pool-20261009-214620-89447.log.
The header names the exact head and base. The job runs on msa1 with two seconds of queue time.
All ten full CI steps PASS. Default: 1419 tests pass in 14.338 seconds. Slow: 256 pass in 23.319 seconds.
Both facade reports show 193 passed, zero failed, and 497 ignored.
Both mutation reports show 110 tested: 95 caught, 15 unviable, zero missed, and zero timeouts.
Full CI takes 345.7 seconds. The repeated mutation job takes 218.2 seconds.
The job and wrapper exit zero after 575 seconds. The job reports 573 seconds of run time.
The env-only second mutation job remains repeated default coverage. This PR adds no slow-based mutation exclusion.
The earlier c811fdd3 gate has six misses. The final gate is complete, but its result does not close F91 or F92.

All 11 removed IDs have PASS lines in the completed gate. They retain their permitted replacement-map proof classes.
None is a slow-based removal or a real-only removal. The reviewer counts minimum 46/69 at base and 50/69 at this head.
The body records the real forms of the controls and the gaps for route_fill and portable fail_writes.
These gaps remain named real-tier work when the real harness lands. They are not exemptions from real acceptance.
The shared testkit changes add no production test hook or real-process guard change.
The earlier guard, anchor, wait, deadline, and cleanup rulings remain unchanged.

### Verdict and retained scopes

PR #217 is NOT CLEAN at `557c414f3383dc357186972380291737a959f401`.
F91 HIGH, F92 MEDIUM, and F93 LOW are OPEN. The reviewer sends them directly to P3 and integration.
Integration round 2 is NOT CLEAN at the same head, verdict `d2b9ddbbe10a79b14a08d68575731597cbc1a27d`.
This is #217's second package NOT CLEAN round. No third-round notice is due yet.
These findings stay in the implementer/reviewer loop. They do not meet the BLOCKED reporting condition.
The lead receives no ordinary NOT CLEAN report under the reporting rule.
Integration reports that v1 has advanced to dc7fb11d915b76d45fdae84b1edaa208a7f895eb.
The next replacement must include the current integration base and its required exact-head evidence.

Round 137 CLEAN for #206 and its F78-F82/F84/F85 closures remain preserved. #206 is merged at 6cc7a722.
#215 is merged at 9103dca1. #216 is merged at e9901293 with contracts v0.1.24 at d79aed5.
Accepted plan 23q remains bae71c81. F39 for #163 and F61/F62 for #192 retain their prior scopes.
P3's non-minimum queue stays parked. The reviewer changes no product code and runs no tests, builds, gates, measurements, or mutants.
All earlier exact-head verdicts remain preserved.

VERDICT: NOT CLEAN
