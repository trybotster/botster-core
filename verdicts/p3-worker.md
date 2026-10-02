# P3 worker review

VERDICT: NOT CLEAN (6 open)

Reviewed head: `f37c46b5caee34742b0e40f798ecb9a423706c42`.
Base: `2016886`. Scope: M1, including the Worker machine, real driver, payload edge, and testkit driver.
This verdict covers both review units in the implementer's message.

Authority: plan pin `c43693ff`, pair-common.md, brief-p3-worker.md, BUILD.md, and contracts-v0.1.7 (manifest final21).
The reviewer inspected logic only. The reviewer did not run tests or a gate.
The implementer reported 276 default tests and 12 slow tests passing. That evidence does not close the findings below.

## F1 — HIGH — The testkit can consume the exit before the spawn result

Status: OPEN.

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

Status: OPEN.

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

Status: OPEN.

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

Status: OPEN.

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

Status: OPEN.

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

Status: OPEN.

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

All six findings must close before CLEAN. No LOW finding is exempt from closure.
