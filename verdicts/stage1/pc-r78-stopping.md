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

## Round 2 — lead decision and revised drain design, 2026-10-11

The initial verdict `a732b58ee2b32d6a2205a8f562bb80d99cbb395d` is now pushed after the transient DNS failure.
Lead receipt `msg_plugin-w_1791751563_fad982` rejects the exited-but-unreaped interval.
The required identity check must report an exited, unreaped worker as Absent without a second reap.
Linux reads the OS process state. If an OS cannot supply the check, the ruling's named existing-deadline alternative applies.
Sol reads complete revision 2, SHA-256 `911d80047a8a6b31fa6640325d9d8ef2b138ef9f98772d7b8be02890505c54c4`.
Sol then reads the complete added write-failure section and revised note.
Its SHA-256 is `bc2bbb0d90c05d4c4d87be8e79ee8bdcf97e91f8766a7ccb3f920b005738e002`.
No source PR or executable evidence accompanies this design correction.

### R78-SI-1 — lead question resolved; implementation remains due

Revision 2 proposes a Linux process-state check and a deterministic real zombie proof on both OSes.
It relies on the current macOS refusal path instead of an explicit SZOMB guard when proc_pidinfo succeeds.
Sol reads complete B9, the retained zombie log header, raw failure, summaries, and exits.
Log SHA-256: `882bf51451f220c19a7f618e52d571e599bc37e0ba335dd89c9311cb33d98353`.
At c121ec31, the Mac test fails because the real answer is Absent and its old expected answer is Matches.
That record proves the observed refusal-path result. It does not prove behavior on a kernel that supplies zombie information.
B9's existing live_of approach retains the explicit SZOMB guard for such a kernel.
Sol reads that complete guard and its refusal path in botster-test-process/src/platform/macos.rs at the reviewed base.
The lead corrects the macOS design decision in `msg_plugin-w_1791751692_d4a13d`.
The required design retains the explicit SZOMB guard after successful proc_pidinfo, as live_of does.
The Mac real zombie proof remains the refusal-path evidence.
The lead permits the same mutation exclusion form and reason as the existing guard, with a B9 citation.
This decision resolves the question. The corrected design, exact source guard, narrow exclusion, and evidence remain due for review.
Sol accepts no broad exclusion or future source change through this decision receipt.

### R78-SI-2 — MEDIUM, still open: separate write failure from a confirmed-gone input drain

Revision 2 retains the read side after a send error and drains within the pump budget.
The first version defines only a driver write-closed flag. The engine still treats worker.link Some as send success.
Sol traces that omission through send_msg and kill_payload at the reviewed base.
A live peer with no EOF can receive a dropped Kill at StopGrace, lose its deadline, and never complete Stop.
The added section defines Input::LinkWriteFailed and worker.link_write_failed.
It makes send_msg return false and adds a live-peer/no-EOF proof. That correction addresses the live-peer omission at design level.
The note does not yet define the necessary branch priority for a worker whose death is already known.
It promises no classification at grace during a gone worker's drain, but also probe classification at grace after write failure.
It promises that a queued Done can complete during the drain, but also immediate failure of in-flight requests on write failure.
Those promises conflict when ProcessExited, write failure, delayed frames, and StopGrace occur before EOF.
A queued Exited can still be displaced by WorkerGone, and a queued Done can follow an already-failed request.
Preserve the confirmed-gone drain before classification or request failure when a read link remains.
Apply that priority when worker.gone is already true and when the identity probe establishes death.
Keep the Matches branch that ends an unreachable live worker's Stop at the existing grace.
Add the combined ProcessExited, write failure, budget-delayed Exited/Done, and grace-before-EOF case to the proof plan.

Revision 2 clarifies the first order: Exited is queued, not already fed. The already-fed form is a control case.
It adds the driver budget case, A5-4 table plan, real process proofs, and prior-art comparison.
Sol checks the cited old worker readiness function at 72b2e335. It fails the start at readiness failure and does not supply this drain design.
The claimed socket order is explicitly unverified. Its Linux and Mac real proof remains required before acceptance of that premise.
No new timer, new limit, out-of-budget drain, adopted-worker change, or implementation result is accepted.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.

VERDICT: NOT CLEAN (1 MEDIUM design finding: R78-SI-2) against base 670e5fc799a25794910a1847fc41b3e089bf03f4
