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
