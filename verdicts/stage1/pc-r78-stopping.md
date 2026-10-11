# PC R-78 items 3 and 4 — design review

## Round 1, 2026-10-11

Design: `~/botster-sessions/shared/core-stage1/handoffs/pc-r78-design.md`.
Design SHA-256: `bb1b8fab049aedeaf7fdcaed08fd87f04d2e5e1d7625566daa4539085af2a314`.
Reviewed base: `670e5fc799a25794910a1847fc41b3e089bf03f4`.
Plan 23cm remains governing. Scope is R-78 items 3 and 4, HIGH, PC after #283.
No source head or PR exists for this design receipt. No gate result is accepted.
Sol reads the complete design, published R-78 and R-72, and the plan's R-78 term.
Sol reads the affected engine flow, deadline, inbound, driver, process edge, and real identity implementation.
The proposed stored spawned flag distinguishes this host's children from adopted workers without changing the durable row.
The proposed identity action already exists. The driver answers it synchronously through the injected edge.
The proposed exit-queue order case preserves the pump bound: take_exits stops when the event budget is exhausted.
Sol accepts no exit drain outside that budget. Adopted-worker changes remain A21's separate scope.

### R78-SI-1 — QUESTION: identity check is not an exit-status check

R-78 item 3 requires WorkerGone if the worker has exited and WorkerUnreachable if it lives.
The proposed IdentityState check observes the pid start time, not the worker's exit status.
At process.rs:45–50, a matching start time returns Matches. The real edge calls that function directly.
The design states that an exited, unreaped worker can still return Matches.
The reaper calls child.wait(), then queues the exit. Core's pump does not control the reaper's scheduling.
A full stop_grace since link loss does not establish when the worker exits within that interval.
Thus the documented unreaped interval can include the classification instant.
The current proposal can then post WorkerUnreachable for an exited worker.
R-78's preferred-form explanation says a dead worker's exit is known by the grace point, but this check does not enforce that premise.
The lead must resolve whether this identity-only check and its documented interval satisfy the ruling.
If they do not, PC must obtain an applicable non-blocking exit check or the ruling's named existing-deadline form.
Sol does not choose a new limit, timer, OS process policy, or contract exception.

### R78-SI-2 — MEDIUM, open: engine-only scope does not preserve the input drain after a failed write

R-78 item 4 requires Core to handle every complete control message before it classifies the worker's exit.
The design retains an open link in the engine after ProcessExited, but changes no driver path.
HostDriver::service_links at driver.rs:544–548 calls flush before retry_held and read_link.
In flush, a link_send error calls close_link_because and returns.
close_link removes LinkState, including pending bytes and a held input, closes the edge, and feeds LinkClosed.
read_link then finds no link. A complete Exited frame in pending, held, or readable socket data can remain unread.
This is a source-derived failure condition. Sol runs no reproduction and claims no measured occurrence.
The engine-only change cannot close this condition. It can classify WorkerGone before it handles that frame.
Expand the design to retain and drain input after a failed outbound write, within the existing pump budget.
Define how the driver reports loss of the write side while the engine waits for the input drain.
Add a driver-level case with ProcessExited, a queued Exited frame, and an outbound write failure.
Keep the applicable complete-message and protocol-error rules. Do not add a drain outside the budget.
Clarify whether the first item-4 test queues the Exited frame or feeds it to the engine before ProcessExited.
An already-fed Exited calls begin_end_flow today. It does not demonstrate the claimed old WorkerGone result by itself.
The two-order proof must identify the edge delivery order and the action that the reversal removes.

The remaining design choices have no separate finding at this stage.
The full R-78 evidence remains due: red-on-revert proofs for both orders of items 3 and 4 and the A5-4 table without regression.
The source implementation, package review, integration source review, final gate, and body remain unreviewed.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.

VERDICT: NOT CLEAN (1 MEDIUM design finding; R78-SI-1 requires a lead decision) against base 670e5fc799a25794910a1847fc41b3e089bf03f4
