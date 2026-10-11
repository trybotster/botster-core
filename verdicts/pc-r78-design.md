# R-78 design review

VERDICT: NOT CLEAN (4 open).

This verdict reviews the design before code. It approves no implementation, executable result, or gate.
Design receipt: `msg_plugin-w_1791751282_685365`.
Design: `~/botster-sessions/shared/core-stage1/handoffs/pc-r78-design.md`, as received with the zombie limit and engine-only scope.
Base: `670e5fc799a25794910a1847fc41b3e089bf03f4`.
Plan: revision 23cm, `stage1-plan.c77c2434.md`.
Risk: HIGH. The design changes process control and the host engine.

## Scope and reviewed paths

PC owns R-78 items 3 and 4. Adopted-worker changes in items 1, 2, and 6 remain with A21.
Item 5 needs no action. This design moves no contract pin and changes no transcript or timeout value.
The result must preserve LC-5, R-50, event order, pump bounds, process identity, and the existing Start and Remove behavior.

The reviewer rereads the binding brief before this review. Its earlier reading order remains satisfied.
The reviewer reads R-78 and traces the exact base's host engine, host driver, process edge, and reaper.
The production path is Stop admission, the Stop flow, StopGrace, driver actions, process-state result, session end, and Stop completion.
The process-exit path includes the control decoder, buffered frames, EOF, and final classification.
The reviewer applies orchestrate-delivery to these lifecycle and ordering checks. No delegation or execution job occurs.

## R78-D1 — MEDIUM: engine-only retention does not preserve the production drain

The note limits code to `botster-core-host` engine files and proposes retaining an open link after `ProcessExited`.
At the base, `HostDriver::service_links` calls `flush` before `retry_held` and `read_link` (`driver.rs:541-549`).
`flush` closes the link on a send error (`driver.rs:470-484`).
`close_link` removes the decoder, pending bytes, and held frame before it feeds `LinkClosed` (`driver.rs:502-522`).

A reachable order is: outbound bytes remain after WouldBlock; the worker writes Exited and dies; the host receives ProcessExited.
The proposed engine retains the link. The driver then retries the outbound bytes before reading the buffered Exited frame.
A send error closes the link and drops that frame. The proposed EOF handler can then classify WorkerGone instead of Exited.
StopGrace can also queue Kill while the gone worker's link still drains. `send_msg` tests only whether the link exists.

Required change: include the driver write-error and queued-send paths in the design's drain guarantee.
Preserve buffered complete frames until the permitted read completes. Keep normal pump-byte and pump-event bounds.
State how StopGrace and new outbound actions behave while a gone worker's link drains.
Ensure the generic close before the `on_process_exited` match does not run before the proposed retention branch.
Add a driver regression with pending outbound bytes, a send failure, buffered Exited, and EOF.
The regression must exercise the real HostDriver. Direct World inputs alone do not exercise this failure.

## R78-D2 — MEDIUM: the evidence plan omits required delivered-path evidence

The note lists sans-IO World tests but names no real-process proof and no A5-4 table.
R-78 requires both-order reversal evidence and an A5-4 table with no PASS-to-non-PASS change.
BUILD.md requires a named real-process proof for the real-process behavior of a HIGH change.

Required change: name the proof that checks actual process status and the control-link drain through HostDriver.
Reuse an existing named proof if it exercises these changes, and identify its assertions.
Name the before/after A5-4 table, exact base, evidence job, and all observed changes.
Retain both event orders and their negative controls. Record the assertion reached by each failed reversal.
The existing base already preserves Exited when a direct Exited input starts PostEnd before ProcessExited.
Do not claim that this unchanged positive order fails on revert without a test that reaches a distinct defect.
All future local evidence must satisfy B13; pool evidence avoids the shared-target hazard.

## R78-D3 — LOW: the design lacks the required Prior art note

The note identifies the current Remove probe but has no Prior art note.
BUILD.md requires that note in a design or PR description and rejects a feature without it.
Required change: identify reused mechanisms, rejected mechanisms, and any new infrastructure with its reason.
The reviewer reads old Core only at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its `worker_process.rs:2956-2975` uses `Child::try_wait` and a bounded stderr snapshot during startup.
That old mechanism does not prove the current host's control-link drain and cannot justify racing the current reaper.
No old code or test is copied by this review.

## R78-D4 — MEDIUM: the declared zombie limit violates the ruled exit check

The note accepts Matches for an exited, unreaped worker and treats the reaper's scheduling delay as a design limit.
At the base, `process::identity_state` compares only start time (`process.rs:45-51`).
The reaper calls `child.wait` on its own thread before it queues the exit (`process.rs:62-75`).
These paths establish the reaped-but-queued case. They do not establish that every exited worker is reaped before StopGrace.

The reviewer asks the lead in `msg_plugin-w_1791751417_e43f58`.
Lead answer: `msg_plugin-w_1791751430_24e8ed`. The lead explicitly rejects the zombie limit.
Required change: use a nonblocking OS-state check without a second waitpid.
The lead specifies Linux `/proc/<pid>/stat` state Z and macOS `proc_pidbsdinfo` `pbi_status` SZOMB as Absent.
If either OS cannot supply the state, use R-78's existing-deadline fallback and state the reason.
The implementation review must include affected AD-6 callers if `identity_state` changes.
The tests must cover reaped, zombie, live with a broken link, and deadline-first with an exit still queued.
The design must preserve the adopted-worker boundary and Stop completion after Exited or Lost.

## Next decision

PC owns all four design corrections. The lead has answered the exit-status question.
No user permission is required for these authorized corrections. No source, test, gate, or pool job is approved by this design verdict.
The reviewer must read the corrected design before design CLEAN. Source review and executable evidence remain separate.
Never poll.

## Revision 2: two design findings remain

VERDICT: NOT CLEAN (2 open).

PC receipt: `msg_plugin-w_1791751569_e73078`. The reviewer reads the complete replacement design.
Sol's initial design verdict `a732b58ee2b32d6a2205a8f562bb80d99cbb395d` is also read from shared Git objects.
It independently identifies the identity-status question and the driver drain defect.
Sol's revision-2 message is `msg_plugin-w_1791751657_b05bf1`.

R78-D2 closes at design level. The note now names the driver tests, real zombie proof, real Stop proof, socket-order proof, and A5-4 table.
The note separates queued Exited from already-fed Exited and keeps the latter as a positive control.
The note marks the socket-order claim unverified. The required both-OS proof remains future evidence.
No execution result, source implementation, or table is accepted by this design closure.

R78-D3 closes. The note now includes old Core prior art and the reason for rejecting its exit-first failure pattern.

### R78-D1 remains MEDIUM: input drain and write failure need separate engine decisions

Revision 2 retains driver input after a send error and marks only the driver link write-closed.
The engine still sees `worker.link` as present. At the base, `send_msg` returns true for any present link (`engine.rs:311-320`).
The note says StopGrace queues Kill, which the driver drops, and makes no classification because EOF decides it.

That rule also applies to a live peer with a failed write side and a readable side that returns WouldBlock.
Such a peer can leave the read side open, so no EOF settles Stop.
StopGrace clears the deadline and queues the dropped Kill. It sends no probe because the engine still considers the link usable.
LC-5's existing broken-link completion can then stop making progress.

Required change: report write-side failure to the engine while retaining the input drain.
Define the StopGrace decision for a live worker and the drain decision for a gone worker separately.
Reject or drop later outbound actions without unbounded queue growth.
Define ownership of unsent handoff marks and sent descriptors until buffered acknowledgments or final close.
Add a HostDriver case with a send failure, repeated WouldBlock reads, a live identity, and the existing StopGrace.
It must finish Stop as WorkerUnreachable under the ruled existing behavior. Add no new limit or timer.
The earlier pending-output, buffered-Exited, EOF, and budget-one regressions remain required.

### R78-D4 remains MEDIUM: keep the explicit macOS zombie guard

Revision 2 adds a Linux Z/X check and removes the declared reaper-scheduling limit.
It relies on macOS proc_pidinfo refusing a zombie and makes an explicit SZOMB guard optional.
The reviewer reads B9 and the focused Mac failure summary with observed Absent versus expected Matches.
That evidence establishes the refusal on its tested Mac. It does not justify removing the guard when the OS returns process information.
B9's existing `live_of` approach retains that guard for a kernel that returns information.

Lead follow-up: `msg_plugin-w_1791751699_3263fb`.
The lead explicitly requires the SZOMB guard when proc_pidinfo succeeds, with `live_of`'s mutation treatment.
The Darwin 27 refusal remains the current Mac evidence. No second waitpid is permitted.
Required change: make that guard part of the design, not an optional reviewer choice.
The four process-status cases and both-OS executable evidence remain required for source acceptance.

The reviewer sends both remaining corrections to PC. No code or executable result is accepted.
Never poll.

## Added write-failure section and Sol's published record

Receipt: `msg_plugin-w_1791751806_761675`.
Sol verdict: `151bd294072d9f91f67dca9edd53813fa54ca86f`, `verdicts/stage1/pc-r78-stopping.md`, Round 2.
The reviewer reads the full new round and verifies its publication on Sol's remote review branch.
The reviewer reads the complete current design, including `LinkWriteFailed` and the appended macOS section.

The added input and `worker.link_write_failed` address the live-peer omission at design level.
`send_msg` will return false after the write failure, and a live Matches result will finish Stop at the existing grace.
R78-D1 remains open for the priority between the confirmed-gone drain and these new failure decisions.
The note promises no classification at grace for a gone worker with a readable link.
It also promises probe classification at grace after write failure and immediate failure of in-flight requests.
These rules conflict when ProcessExited, failed writes, budget-delayed frames, and StopGrace occur before EOF.
A queued Exited can follow WorkerGone classification. A queued Done can follow an already-failed request.

Required change: preserve the readable drain when death is known or the probe establishes death.
The live Matches branch must still complete an unreachable Stop at the existing grace.
Add the combined driver proof with ProcessExited, a failed send, delayed Exited and Done, and grace before EOF.
Cover the reverse write-failure/exit-report order to establish that request failure does not win before the drain.
Keep the normal pump bounds. Do not add a timer, limit, or out-of-budget exit drain.

The current macOS section still describes the explicit guard as unnecessary and says the earlier lead ruling is retained.
That text predates the required guard correction in `msg_plugin-w_1791751699_3263fb`.
R78-D4 remains open until the note adopts that correction. The lead question itself is resolved.
Sol counts one remaining MEDIUM drain finding and keeps the mandatory guard as a required source condition.
This package record retains the guard as a design finding until the contradictory design text changes.

The reviewer sends both corrections to PC. No source or execution result is accepted. Never poll.

## Revision 3: design CLEAN

VERDICT: CLEAN. No open design finding.

PC receipt: `msg_plugin-w_1791751957_aa5f71`.
The reviewer reads the complete revision 3 and verifies its SHA-256:
`52b9ada3f73a1fdf77c9f05b04775f2fffb4b6b5385f72149db77c6f99462885`.
The base remains `670e5fc799a25794910a1847fc41b3e089bf03f4`. The review accepts this design only.

R78-D1 closes at design level.
The new priority rule retains readable input when `worker.gone` is true or the probe reports Absent or Reused.
Neither StopGrace nor LinkWriteFailed classifies that session or fails its in-flight requests before the drain.
EOF selects the end after complete frames are handled. Without a readable link, confirmed death selects WorkerGone immediately.
The live Matches branch completes Stop as WorkerUnreachable at the existing grace, without waiting for EOF.
LinkWriteFailed itself fails no request already in flight. A new request that cannot be sent can fail immediately.
The revised driver tests include both exit-report/write-failure orders, delayed Exited and Done, and grace before EOF.
Unsent descriptors leave through HandoffLost. Sent descriptors remain link-owned until their drained acknowledgment or final close.
The design requires one loss per descriptor and no retained ownership after final close.

R78-D4 closes at design level.
The note makes the explicit SZOMB guard mandatory after successful macOS process information.
The note separately states the current Darwin refusal path and its real zombie proof.
The Linux Z/X check and macOS guard do not reap. The reaper retains exclusive ownership of child.wait.
The proposed libc dependency already exists in the lockfile; its macOS dependency role changes.
No dependency source or lockfile change is accepted without the later source review.

All four design findings are closed. Adopted-worker scope, Start behavior, Remove behavior, and pump bounds remain acceptance conditions.
The source review must verify that the priority runs before conflicting Kill effects, end classification, or request failure.
It must verify descriptor cleanup, bounded future sends, complete-message handling, and all affected AD-6 callers.

The note still marks socket ordering unverified. Design CLEAN does not establish that OS premise.
PC must use the named both-OS socket proof before relying on the premise for the driver repair.
This follows orchestrate-delivery's requirement to prove an uncertain premise before adding the dependent recovery path.
The worker's close-on-exec ownership and EOF condition also remain source and real-process proof obligations.

PC reports local source edits, a successful build, and 43 passing core-sys tests. No raw evidence accompanies this design receipt.
This record accepts none of those source or execution claims.
The committed source, first READY gate, two source reviews, reversal logs, A5-4 table, both-OS proofs, and final gate remain due.
No broad mutation exclusion is accepted. Any new guard exclusion requires its exact scope and B9 reason in source review.
Sol's applicable revision-3 design receipt is not yet read here. No PR or source head is approved by this verdict.
The reviewer runs no build, test, gate, mutant, or executable check. Never poll.

## Sol revision-3 design CLEAN receipt

Receipt: `msg_plugin-w_1791752475_943937`.
Sol verdict: `1c4aacd9ca68abc860103270258ead5d44ec2023`, `verdicts/stage1/pc-r78-stopping.md`, Round 3.
The reviewer reads the complete new round and verifies publication on `origin/stage1/integration-review-sol`.
Both design CLEANs cover revision 3, hash `52b9ada3f73a1fdf77c9f05b04775f2fffb4b6b5385f72149db77c6f99462885`.
Both reviews use base `670e5fc799a25794910a1847fc41b3e089bf03f4`.
Sol closes the drain finding and confirms the mandatory guard, ownership, event-order, and pump-bound requirements.
The same-design integration receipt condition is closed.
The socket-order premise remains unverified. The named both-OS proof must establish it before the dependent driver repair relies on it.
All source reviews, reversal evidence, A5-4 evidence, both-OS proofs, and gates remain due.
This receipt accepts no source head, executable result, or merge readiness. No execution job runs here. Never poll.
