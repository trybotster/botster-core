# P1 lifecycle review

VERDICT: CLEAN

Reviewed head: `2016886ffda434bee0e87242d85e36ecc43ec212` on `stage1/p1-lifecycle`.
Round 7 head: `0ac2b84d38a8e3a3a1667eec172985cd0284dc90`.
Round 6 head: `40117af8398c292e8763d04beca875466bd276ef`.
Round 5 head: `154f0109e8809536a30340cda89198fa34259db5`.
Round 4 head: `3936f014be834e24505354e99fa4932ed989be29`.
Round 3 head: `eda5711bc9252dbf402e8d8b391bcf8e8e80ce07`.
Round 2 head: `1f0146e831b03fb3d1edd247240d97b3c9503552`.
Round 1 head: `fb75dec1b6a00270c89f63ce6b67357892e060ff`.
Initial code checkpoint: `6a8017621f024cbf6c07a3f9b9c50deae15fb936`.
I also reviewed the delta through `eb39516`, `3ca7680`, and `fb75dec`.

Round 1 base: `7c0ae16` (P0 and the merged P6 testkit).
Round 2 base: `ccb04eb` (the merged CI infrastructure).
The review covers P1's dependency move, code, and fixes.
Authority: plan pin `555bc433`, BUILD.md, and the P1 brief.
The initial checkpoint pins manifest final14 through `contracts-v0.1.2`.
Round 2 pins manifest final16 through `contracts-v0.1.4`.
The latest head pins manifest final19 through `contracts-v0.1.6` at `caa029cfcc9c0bfd1f59a05d35d7c110ea99f3e0`.
Its separate pin commit is `ee574ea`.
This is a logic review. I did not run the gate or a test suite.

The design note contains the required Prior art note. The engine uses injected inputs and actions.
I accept the JSON construction of `RemoveReport`: the pinned type has no public constructor.
I exclude the stated P5 adoption, P4c WebRTC, P4a descriptor handoff, and P7 service implementations from this checkpoint.
Those exclusions do not establish package or Stage 1 acceptance. The pending list must retain unproved ids.
The Round 1 tag correction fixes the six unit-valued Remove transcripts and supplies Amendments 7 and 8.
Those ids remain pending until both harnesses prove them.
The delta adds A8-1 capture reservations. I found no additional defect in that reservation change.
The Round 1 pin delta did not close F1 through F15. F16 also applies under Amendment 7.

Current open findings: none.
Closed findings: F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F15, F16, F17, F18, F19, F20.
F14 has an authorized scope deferral. It is not satisfied as a TI-1 requirement.
This CLEAN verdict applies to the reviewed P1 host-side checkpoint only.
P3 supplies the EndPayload worker handler and the worker-dependent end-to-end proofs.
F14 and the other unproved conformance ids remain pending with their owners and scope reasons.
The implementer must run the gate once on this exact committed head before reporting DONE to the lead.
A later commit requires a delta review.

## Round 8: harness-count assertion delta

I reviewed the complete delta from `0ac2b84` to `2016886`.
It changes one assertion in `xtask/src/ci.rs` from one link harness to two.
The HARNESSES table contains link_decoder and msg_decoder for botster-core-link.
The new expected count therefore matches the existing selection logic.
The unchanged assertion still rejects harness selection for botster-core-sys.
This delta changes no runtime or gate selection logic and opens no finding.
VERDICT: CLEAN remains valid on the exact head above.
The implementer reports that the previous gate failed at this assertion.
The new head needs its own gate result. I did not run tests or the gate.

## Round 7: public API snapshot delta

I reviewed the complete delta from `40117af` to `0ac2b84`.
It changes only `api/botster-core.txt`.
The snapshot now records the existing Core::open and CoreApi implementation, plus the resulting auto-trait changes.
I checked the added signatures against the facade source.
The snapshot retains Core's Send implementation and negative Sync implementation, as TH-1 requires.
This delta changes no runtime code and opens no finding.
VERDICT: CLEAN remains valid on the exact head above.
I did not run a build, tests, or the gate.

## Round 6: final closure evidence

I reviewed the complete delta from `154f010` to `40117af`.
This remains a logic review. I did not run tests or the gate.

| Finding | Status | Evidence |
|---|---|---|
| F7 | CLOSED | DESIGN.md's choices table and P3 interface note now use EndPayload consistently. The forbidden bare-group signal, worker Term/Kill fallback, and live-payload permission are removed. |
| F20 | CLOSED | The shell creates its ready file after installing the trap. The test checks that explicit indication under a 10-second deadline before sending EndPayload. It retains Reaper cleanup and the exact exit assertion. |

The Round 5 code closure for F3 remains valid because this delta changes no driver logic.
The EndPayload runtime interface remains the identity-verified SIGUSR1 request to the worker process alone.
All checkpoint findings, including LOW findings, are closed or have the lead's explicit scope deferral.

## Round 5 evidence (historical)

The Round 5 statuses below describe `154f010` only.
The final closure table above contains the current statuses.

## Round 5: closure evidence and remaining defects

I reviewed the delta from `3936f01` to `154f010`.
This remains a logic review. I did not run tests or the gate.
The references in this section use `154f010`.

| Finding | Status | Evidence |
|---|---|---|
| F3 | CLOSED | The driver queues failed handoffs and publishes their results within the event budget. It runs due Silent steps before process and link input, outside scheduler selection. |
| F7 | OPEN, LOW | The code and botster-core-link interface follow the authorized EndPayload signal path. DESIGN.md still gives conflicting instructions for P3. |
| F20 | OPEN, LOW | The new real signal test uses a fixed sleep as proof that the worker handler is ready. |

F7's runtime defect is closed for P1's host-side checkpoint.
EndPayload maps to SIGUSR1 for the verified worker process alone, rather than its group.
Stop, link failure during Stop, and stop grace all use that request.
The interface defines payload teardown, worker survival, idempotence, and exit reports after recovery or adoption.
P3 still supplies the real worker handler, and the required end-to-end ids remain pending for P3.
This closure does not establish LC-5 or ST-5 end-to-end conformance.

### F7 remaining — LOW: The design note still gives forbidden worker-control instructions

Evidence: `crates/botster-core-host/DESIGN.md:37`, `:65`, and `:66`.

The choices table still says that a broken-link Stop signals the bare payload group.
The Round 2 decisions still instruct the host to send Term and then Kill to the worker.
The P3 interface note still specifies SIGTERM and permits a surviving payload after the host kills the worker.
The new EndPayload paragraph supplies the correct interface, but it does not remove those contradictory instructions.
The previous review explicitly required removal of the statement that permits this live payload.

Required change: Make DESIGN.md state the current worker-control interface consistently.
Remove the bare-group signal, worker Term/Kill fallback, and live-payload permission.
Keep the approved SIGUSR1 request, worker survival, and P3 ownership clear.
Authority: the lead's F7 authorization, LC-5, ST-5, and AD-6.

### F20 — LOW: The signal test assumes handler readiness from elapsed time

Evidence: `crates/botster-core-sys/tests/slow_process.rs:114`.

The test sleeps for 300 ms, then sends SIGUSR1 twice.
Elapsed time does not prove that the shell installed its trap.
A delayed shell can receive SIGUSR1 before the trap is installed and exit from the default signal action.
The test then fails despite a correct signal edge.
The annotation describes a settle timer, not a deadline timer.
The binding pair-common rule allows marked deadline timers only and prohibits sleeps used to wait for progress.

Required change: Wait for an explicit handler-ready indication before sending EndPayload.
Use a deadline to bound that wait.
Retain the test's process cleanup and exact exit assertion.
Authority: pair-common's test rule and BUILD.md's fast and lean proof rules.

## Round 4 evidence (historical)

The Round 4 statuses below describe `3936f01` only.
The Round 5 table above contains the current statuses.

## Round 4: closure evidence and remaining defects

I reviewed the delta from `eda5711` to `3936f01`, including the separate contract pin commit.
Erratum 3 candidate 3 is now published in the pinned contract set.
The seven E3-1 ids remain pending conformance proof.
This remains a logic review. I did not run tests or the gate.
The references in this section use `3936f01`.

| Finding | Status | Evidence |
|---|---|---|
| F3 | OPEN | Metadata and local Detach now post one event per input. At the bound, the eligible work excludes normal work. Action results and carried-step priority still bypass the intended limits. |
| F7 | OPEN | Core no longer signals an unproved payload group. Its broken-link grace now kills the worker group, which violates payload termination and final-model worker survival. |
| F17 | CLOSED | `created_path` preserves the notification policy's admitted result path. Admission and Unknown share `held_bytes`, which multiplies the key repeat and wheel notches. |

### F3 remaining — HIGH: Action batches exceed the bound, and newer input bypasses carried deadlines

Evidence: `botster-core-host/src/driver.rs:164`, `:170`, `:559`, and `:575`; `src/inbound.rs:523`.

`perform_counted` drains the entire action queue before accounting for the events that the results post.
`step_mark` limits completions within one input, but each action result is a separate input with a fresh mark.

For example, configure `pump_events = 1` and enough mandatory room.
Register two routes on a Running session before the next pump.
Both handoffs enter the action queue.
If both handoffs fail, each result posts RouteClosed, and `perform` posts both in the same pump.
The pump therefore exceeds its event bound despite the new one-event completion guard.

The driver also feeds newer process and link inputs before it considers Work::Silent.
After a pump carries a Silent step, a Bell frame in the next pump can consume the whole budget first.
The carried Silent then carries again.
Repeated newer input can keep that carried step from running.
E3-1 requires runnable carried steps before newer work.

Listing Silent before ordinary work is also insufficient for the shared scheduler.
ReadyWork still includes operations alongside Silent, and the seeded scheduler can select an operation first.
The carried-step test covers only an ordinary operation under the production selection policy.
It does not cover newer link input or a different legal scheduler choice.

Required change: Budget the publication caused by each action result.
Retain deferred publication work without delaying event-less due effects.
Select runnable carried steps before newer input and normal work under every scheduler policy.
Prove two failed handoffs with `pump_events = 1`.
Prove a carried Silent followed by a newer Bell and by a scheduler choice that would prefer an operation.
Authority: 9B, A2-7, A5-2, E3-1 items 1, 3, and 5, and TM-6.

### F7 remaining — HIGH: Broken-link grace kills the worker and can leave the payload alive

Evidence: `botster-core-host/src/run.rs:211` and DESIGN.md's P3 interface note.

When the link is absent, `kill_payload` sends GroupSignal::Kill to the verified worker group.
It then records Lost(WorkerUnreachable) as though the payload group was killed.
SIGKILL cannot invoke a worker signal handler.
The P3 handler therefore cannot translate this signal into the required payload-group kill.
The design note explicitly permits the payload group to survive this fallback.

LC-5 requires the payload to end and the final-model worker to survive until Remove.
Identity verification makes this signal safe from pid reuse; it does not make the worker the correct kill target.
The current test requires a worker Kill and preserves the original F7 defect.

Required change: Provide an identity-safe control path that asks the worker to kill its payload group without killing the worker.
Retain the unreaped payload leader through group control, or prove an equivalent mechanism.
P1 must provide a usable host-side request for P3 to implement.
Remove the design statement that permits a live payload after Stop completes.
The lead authorized a distinct catchable worker-control signal in message `msg_plugin-w_1790934505_87cdd0`.
P1 names SIGUSR1, or another catchable signal, and sends it only to the identity-verified worker.
Its meaning is a graceful payload request followed by payload-group kill after stop_grace, while the worker keeps serving the final model.
The worker retains the unreaped leader during group control.
The host never sends SIGKILL to the worker on this path.
P1 records the signal, meaning, idempotence, and reports after link recovery or adoption in its PR and botster-core-link documentation.
P3 implements the handler. P1 retains the ids that need that handler as pending, with the P3 reason.
A worker that does not respond is a Lost case, not a reason to kill the worker.
F7 remains open while that interface lacks a valid grace-kill request.
Authority: LC-5, LC-6, ST-5, and AD-6.

## Round 3 evidence (historical)

The Round 3 statuses below describe `eda5711` only.
The Round 4 table above contains the current statuses.

## Round 3: closure evidence and remaining defects

I reviewed the delta from `1f0146e` to `eda5711`.
This remains a logic review. I did not run tests or the gate.
The references in this section use `eda5711`.

| Finding | Status | Evidence |
|---|---|---|
| F2 | CLOSED | The consumption check now parks Exited, Launched, and LaunchFailed frames when mandatory room is absent. The driver test covers a parked Exited frame. |
| F3 | OPEN | Held inputs and process exits check the budget. Retirement and route waiters complete in separate steps. Receive slices use the remaining allowance. Other paths still post multiple events per step. |
| F7 | OPEN | Launched now requires payload identity. The new group signal path controls descendants after leader exit, but it does not prove group identity when the leader is absent. |
| F8 | CLOSED | Remove grace requests a kill and continues waiting for ProcessExited. The id remains reserved during that wait. |
| F10 | CLOSED | StopAll targets continue after row failure under R-16. Plain Stop retains RegistryFailed. Concurrent Stop waiters fail while StopAll still stops its target. |
| F13 | CLOSED | Cancel checks retired ranges before the operation table and its Done case. |
| F17 | OPEN | Retirement now selects a result by operation kind and uses Unknown for sent writes. Notification-policy state and semantic-input bounds remain incorrect. |
| F19 | CLOSED | Frame consumption uses an iterative loop with one read buffer. The driver test supplies 50,000 frames. |

### F3 remaining — HIGH: A pump still posts more than its event bound

Evidence: `botster-core-host/src/inbound.rs:81`, `src/run.rs:351`, and `src/driver.rs:579`.

With `pump_events = 1`, a successful UpdateMetadata row result posts Completed and MetadataChanged in one input.
`perform_counted` accounts for both only after the input returns.
The queue counts both posts, including keyed replacements.
Thus the pump posts two events despite its bound of one.

DetachLocal has the same problem.
It calls `close_route`, which posts RouteClosed, then calls `complete` in the same step.
A bound route with a failed worker link can reach this path and post two events.
The change to route waiters does not split this direct completion.

The exhausted-budget check also inspects `ready[0]` before the scheduler selects the work.
If a deadline occupies index zero, the scheduler can select normal operation work and run it after the event budget is exhausted.

Required change: Split every multi-event path into separate publication steps.
Preserve Completed-before-MetadataChanged and RouteClosed-before-Completed ordering.
After budget exhaustion, restrict the eligible work itself to permitted deadline processing.
Prove UpdateMetadata and local Detach through HostDriver with `pump_events = 1`.
Authority: 9B, A2-7, A5-2, LC-9, and DP-7.

Deadline subcase: with two due silence deadlines and `pump_events = 1`, this head posts both Silent events in one pump.
The lead confirmed acceptance of erratum 3 candidate 3 in message `msg_plugin-w_1790931045_566b58`.
It belongs to manifest final19, with publication pending at the time of that message.
Candidate 3 supersedes rejected candidate 2 (`9f20fd5`) and rejected candidate 1 (`5cdb45e`).
The lead directs P1 to implement and review against candidate 3.
An event-less due effect runs in its due pump regardless of the event budget.
An atomic effect-and-event step runs only with budget; otherwise the entire step carries to the next pump.
Carried steps set more and signal the wake.
Runnable carried steps run before newer work in due-time order.
If a carried mandatory event has no queue room, its entire step parks with its state unchanged.
Polling room makes that step runnable and signals the wake.
The event budget and mandatory queue room are separate limits.
Due order applies only among runnable steps.
A parked step never blocks a later runnable step, including Silent.
Among runnable steps, carried steps come first; each group keeps due order.
When room returns, a parked step takes its place by due time among runnable steps.
An event-less due effect can overtake an earlier carried or parked event step.
Silent posts in the first pump with budget, with unchanged since, once per idle period.
The seven `e3_1` ids require implementation and conformance proof:

- `e3_1_due_effect_without_event_runs_in_its_pump_with_no_budget`
- `e3_1_due_step_beyond_budget_is_carried_with_more`
- `e3_1_carried_steps_run_first_in_due_order`
- `e3_1_two_silences_due_together_with_budget_one`
- `e3_1_carried_transition_into_a_full_mandatory_queue_is_parked_and_woken_on_room`
- `e3_1_eventless_effect_overtakes_a_carried_or_parked_step`
- `e3_1_runnable_silent_bypasses_a_transition_parked_on_queue_room`

### F7 remaining — HIGH: An absent leader does not prove the saved group identity

Evidence: `botster-core-sys/src/process.rs:124` and `:45`.

`signal_payload_group` rejects Reused but sends a group signal for Absent.
The saved group can have ended before this check.
Its pid can then name an unrelated group leader that exits and is reaped while that group's descendants remain.
The probe again returns Absent, and Core signals the unrelated group.
The rule that a current group retains its id does not prove continuity from the saved identity.

The lead confirmed the AD-6 requirement in message `msg_plugin-w_1790930323_896b04`.
It applies to every signal Core sends, including this payload fallback.
No contract ruling is needed to reject the unproved group signal.

Required change: Prove the identity throughout the control interval.
The lead permits a worker parent to retain the unreaped payload leader until its group kill completes.
For a broken link, the host can signal the verified worker, whose handler controls that retained payload group.
The worker must remain alive to serve the final model, as LC-5 requires.
The implementer must choose and prove the mechanism.
Ask the lead if identity proof and LC-5 cannot both hold for a required case.
Authority: AD-6, LC-5, and LC-6.

### F17 remaining — HIGH: Retirement still gives a forbidden result and understates write uncertainty

Evidence: `botster-core-host/src/run.rs:547`, `:559`, and `src/flows.rs:442`.

`ended_result` always groups SetNotificationPolicy with the registry operations.
Admit SetNotificationPolicy on an Exited session whose worker supports that feature, then admit Remove before the policy finishes.
Retirement returns RegistryFailed even though the worker-state row permits only WorkerLinkFailed.
Amendment 3 permits RegistryFailed only in Created.
Select this result from the admitted operation path, not only from its operation kind.

Both `payload_len_of` and `payload_len` return 64 for every semantic input.
For example, configure `max_key_repeat >= 100` and `max_input_bytes >= 6400`.
A sent Backspace Key press with `repeat = 100` can write 100 encoded bytes before the link fails.
The resulting Unknown reports `max_payload_bytes = 64`.
IN-9 counts encoded bytes for semantic input, so 64 is not a valid upper bound for that write.
The same defect applies to repeated wheel notches.

Required change: Preserve the documented result path for notification policy in each admitted state.
Use a valid conservative bound for semantic writes, including repeat and notches, on retirement and link failure.
Reuse the admission bound instead of maintaining two inconsistent payload-length helpers.
Prove a sent repeated key that exceeds 64 encoded bytes before link loss.
Authority: Amendment 3 A2-1, IN-2, IN-7, and IN-9.

## Round 2 evidence (historical)

The Round 2 statuses below describe `1f0146e` only.
The Round 3 table above contains the current statuses.

## Round 2: closure evidence and remaining defects

The implementer rebased onto `ccb04eb` and sent fix commit `76110a4` plus comment commit `8bbee48`.
The final delta `1f0146e` applies ruling R-15 and closes F18.
I reviewed those changes against the rebased P1 code and the Round 1 findings.
The following references use `8bbee48`, except the final F18 closure at `1f0146e`.
I did not run tests or the gate.

| Finding | Status | Evidence |
|---|---|---|
| F1 | CLOSED | Stop sends its request and starts grace before publishing Stopping. Remove follows ruling R-15 and continues the permitted effects under pressure. |
| F2 | OPEN | Held route frames and read-interest restoration fix route-event growth. Worker exit frames still bypass the consumption check. |
| F3 | OPEN | Accounting and separate Stop completions improve the budget. Other inputs and retirement steps still exceed it. |
| F4 | CLOSED | `poll_events` releases each input reservation and checks the instance. |
| F5 | CLOSED | Failed Create retires all associated pending operations. F17 records invalid retirement results. |
| F6 | CLOSED | Remove retires associated operations, clears inflight requests, and guards later steps by instance. F17 records invalid retirement results. |
| F7 | OPEN | The reported payload identity separates payload and worker groups. Missing identity and leader absence still leave a payload group uncontrolled. |
| F8 | OPEN | Normal removal waits for exit, but grace expiry still declares worker exit without observing it. |
| F9 | CLOSED | Signal travels as a numbered worker operation and waits for Done. Link failure resolves its inflight request. |
| F10 | OPEN | StopAll no longer hangs in the reviewed failure case, but now completes with a Running target. |
| F11 | CLOSED | Atomic commit errors map to Uncertain, and the Prior art note describes directory synchronization correctly. |
| F12 | CLOSED | Created setters apply in pump. Start waits for the admitted setters. |
| F13 | OPEN | Exact ranges fix truncated history and unminted zero. Unpolled operations still bypass the retired-instance check. |
| F14 | SCOPE DEFERRED | The lead approved a placeholder for this checkpoint, subject to comments and pending ids. See the ownership note below. |
| F15 | CLOSED | HostDriver now calls the Session choice point for session work. |
| F16 | CLOSED | Attach rejects durations below 1 ms before reserving a route. |
| F18 | CLOSED | Remove starts with CloseRoutes. Each route remains bound until its RouteClosed posts. SendRemove then releases captures and starts teardown. |

### F2 remaining — HIGH: Exit frames do not park with the state transition

Evidence: `crates/botster-core-host/src/engine.rs:224`.
`can_accept` checks only RouteClosed, RouteStalled, and RouteResumed.
With mandatory room absent, WorkerMsg::Exited still enters the engine and changes the flow.
The driver continues consuming later frames instead of retaining the exit frame and removing read interest.

EV-5b and plan 2.5 rule 7 explicitly require the worker exit to stay unread when its state event cannot fit.
Extend the consumption check to the inputs that cause a parked state transition.
Prove the exit-frame case through HostDriver.

### F3 remaining — HIGH: Some inputs and steps still exceed the pump bounds

Evidence: `src/driver.rs:355`, `:369`, `:544`, `src/flows.rs:406`, and `:464` in botster-core-host.

- `retry_held` delivers every acceptable held input without checking the remaining event budget.
- `poll_process_exit` drains every exit without checking that budget. One exit can also complete many inflight operations.
- `fail_inflight` and `retire_session_ops` still call `complete` repeatedly in one step.
- `close_route` also completes every waiting route operation after posting RouteClosed, without an event-budget check.
- `read_link` tests its remaining byte allowance but always passes a 16 KiB buffer to `link_recv`.
  With a configured byte allowance below 16 KiB, one read can exceed that allowance.
- When fewer than 16 KiB remain but the allowance is nonzero, `may_read` refuses another read.
  That branch does not set more_input. The driver can report no runnable input even though it stopped because of its byte bound.

For example, `pump_events = 1` plus two pending reads and one link failure posts two completions in one pump.
The new single-frame Bell test does not cover this path.

Preserve the current accounting fixes.
Split every multi-completion path into budgeted publication steps.
Limit the actual receive slice to the remaining byte allowance.
Distinguish parked work from input deferred only by a pump bound.
Authority: 9B, A2-7, TM-6, and plan 2.4.

### F7 remaining — HIGH: Broken-link Stop can claim an end without controlling the payload group

Evidence: `crates/botster-core-link/src/msg.rs:157`, `botster-core-host/src/inbound.rs:218`,
`botster-core-host/src/run.rs:189`, and `botster-core-sys/src/process.rs:120`.

Core accepts Launched with `payload: None` and reaches Running.
After link loss, neither the graceful request nor the grace kill sends an OS signal for that session.
`kill_payload` nevertheless sets the end to Lost(WorkerUnreachable).

When a payload identity exists, the real edge still requires that the group leader's pid and start time match.
If the worker reaps that leader after link loss, surviving descendants can retain the payload group.
The identity check then returns Absent and skips the group kill.
Leader absence does not prove group absence.

Require the identity needed for broken-link control before accepting a real launched payload.
Provide a group-control path that accounts for surviving descendants after leader exit without signaling an unrelated group.
Do not treat missing control identity as proof of payload termination.
Authority: LC-5, LC-6, and AD-6.

### F8 remaining — HIGH: Grace expiry still frees an id before observed worker exit

Evidence: `crates/botster-core-host/src/flows.rs:586`.

`remove_grace_expired` queues SignalGroup(Kill), sets `worker_gone = true`, and advances removal.
The process edge has not returned an exit result.
Its signal method has no result and can skip a signal or ignore an OS error.
A sent signal is not an observed process exit.

The new `remove_waits_for_the_worker_to_end` test explicitly expects completion after the deadline without supplying ProcessExited.
That expectation preserves the defect.

Keep removal waiting after the kill request.
Advance steps 4 and 5 only after an exit or a verified absence result for the worker identity.
Authority: LC-7 and A6-3.

### F10 remaining — HIGH: StopAll completes with a Running target

Evidence: `crates/botster-core-host/src/flows.rs:731` and `src/run.rs:502`.

On a stop-row failure, Core restores Running and removes that session from every StopAll target set.
StopAll then completes Ok.
LC-12 requires completion after every original target reaches Exited, Lost, or Created.
Its explicit leave-as-is states are Created, Exited, and Lost; this target is Running.
The A2-1 table's shorthand refers to LC-12 and does not remove that completion condition.

The new test `stop_all_leaves_a_target_whose_stop_row_failed` asserts the forbidden Running result.
Preserve the original target obligation and meet LC-12's completion condition.
Ask the lead for a contract ruling if the failure path needs an outcome that the contract does not define.

The lead supplied ruling R-16 in message `msg_plugin-w_1790928340_fa7c8c` after this review.
For StopAll targets, the Stopping row write is best effort, and the stop proceeds to LC-12 completion.
Plain Stop keeps RegistryFailed.
F10 remains open at the reviewed head because that implementation restores Running and drops the target.
Close F10 when the implementation follows R-16.

### F13 remaining — MEDIUM: Unpolled retired operations return TooLate

Evidence: `crates/botster-core-host/src/admit.rs:798`.

The retired range check runs only when the operation is absent from `self.ops`.
An old operation whose Completed remains unpolled still exists as Step::Done.
After Remove and recreation of the id, cancel returns TooLate for that operation.
ID-1 requires UnknownOp for the removed incarnation, with no polling exception.

Check retired-instance identity before the live operation table's Done case.
Keep the Completed event and its pending slot until polling, as EV-5a requires.

### F14 ownership note

The lead confirmed the deferral in message `msg_plugin-w_1790927459_d216e0`.
P2 supplies `botster_terminal_ghostty::terminal_identity()` after its fork patch 5 lands.
P1 still owns all three TI-1 ids and must wire that function in a follow-up PR.
The three ids remain in core-pending.txt.
The real facade and testkit now label the source as a TI-1 placeholder, and no inspected test asserts an empty source.
This closes the checkpoint scope question only. It does not establish TI-1 conformance.

### F17 — HIGH: Retirement results bypass the operation table and input certainty rules

Evidence: `crates/botster-core-host/src/flows.rs:484` and `src/run.rs:511`.

Failed Create gives its RegistryFailed result to every associated non-input operation.
An admitted Resize or SetColorProfile can therefore receive RegistryFailed, which its A2-1 row does not allow.
During Remove, `ended_result` gives every non-input operation SessionEnded.
ReadModeFlags, ReadScreen, CaptureSnapshot, UpdateMetadata, and Detach do not have that asynchronous error in their rows.
The new read-removal test asserts SessionEnded for ReadModeFlags.

Retirement also reports NotWritten(SessionEnded) for every input operation.
It does not check whether the request was sent and remains unacknowledged.
That path can claim certain zero for a write whose actual progress Core does not know.

Required change: Resolve each operation through its documented result path.
Preserve exact or unknown write progress according to IN-2 and IN-7.
Ask for a contract ruling when no documented result covers an admitted operation after a failed Create or Remove.
Authority: A2-1, A2-2, AM-3, IN-2, and IN-7.

### F18 — CLOSED: Remove releases captures before it closes bound routes

Original evidence at `8bbee48`: `crates/botster-core-host/src/flows.rs:504` and `:548`.

SendRemove releases all captures before CloseRoutes runs.
A previously readable capture becomes UnknownCapture while the session still has bound routes in the host table.
This occurs even when mandatory room is available.
LC-7 orders route closure, capture release, then upload cleanup and worker end.

Required change: Preserve the order of teardown effects.
Ruling R-15 in contract commit `ee44b0c` also requires this order under mandatory pressure.
The lead relayed it in message `msg_plugin-w_1790928065_a6d4e9`.
Each route closure and its RouteClosed event form one atomic step.
When the queue has no room, the route stays Open in the worker and host.
Steps 2 through 5 wait until every bound route's RouteClosed event is posted.
With no bound routes, or after those events, steps 2 through 5 continue under pressure.
SessionState{Released} then waits for room, followed by Completed{Remove}.

Prove the pressure case with one bound route and one open capture.
The capture must stay readable until the queue frees and RouteClosed posts.

Closure: `1f0146e` starts Remove in CloseRoutes and moves to SendRemove only after every route closes.
The existing room check parks CloseRoutes while any bound route remains and the queue is full.
`close_route` posts RouteClosed before it removes the binding.
The new test retains the capture and blocks teardown until the route event posts.

### F19 — MEDIUM: Frame consumption recurses once per decoded frame

Evidence: `crates/botster-core-host/src/driver.rs:369` and `:409`.

After processing pending bytes, `read_link` calls itself instead of continuing its loop.
Each call creates another 16 KiB read buffer.
Frames that post no event, such as Pages for an unknown request, do not consume the event budget.
A legal-sized input batch can therefore cause thousands of nested calls before the byte bound stops reception.
Rust does not guarantee tail-call elimination. The driver can exhaust its stack, and nested calls retain their read buffers until return.

Required change: Consume frames with an iterative loop and one read buffer per active link read.
Keep the existing byte and event bounds.

## Round 1 evidence (historical)

The sections below preserve the original findings.
Their current status appears in the Round 2 table above.
Their line numbers refer to the initial code checkpoint.

## F1 — HIGH: Mandatory pressure blocks Stop effects

Evidence: `crates/botster-core-host/src/flows.rs:313` and `src/run.rs:76`.

Fill `mandatory_events` before admitting `Stop` on a Running session.
The stop writes its row, then waits in `PostStopping` because the queue has no room.
`SendStop` follows that state event. Core sends neither the graceful request nor a kill, and creates no stop deadline.
`StopAll` uses the same flow. Remove also waits in `CloseRoutes` before it sends the worker teardown request.

EV-5c explicitly requires Stop, StopAll, and Remove effects to continue under mandatory pressure.
The current test at `src/tests/queue_pressure.rs:49` empties the queue before Stop; it does not cover this condition.

Required change: Separate destructive effects and their deadlines from event publication.
Keep the visible state transition parked until its event fits.
Prove the behavior with a queue that is already full before admission.

## F2 — HIGH: Link reads bypass backpressure and can grow parked queues without a bound

Evidence: `crates/botster-core-host/src/driver.rs:292`, `:314`, and `src/inbound.rs:265`.

`read_link` drains each link until WouldBlock and delivers every frame immediately.
The driver has no read-interest method or engine capacity check.
When mandatory room is absent, repeated `RouteStalled` and `RouteResumed` messages append to `parked_events` without a bound.
Route-close messages similarly append to `parked_closes`.
The single-frame decoder bound does not bound these queues or the work of one pump.

EV-5b requires a blocked transition to remain unread on its link.
Plan 2.5 rules 7 and 8 require bounded reception, removal of read interest, and restoration after a poll frees room.

Required change: Add consumption checks and read-interest control to the shared driver and both edges.
Keep an unconsumable frame in the bounded receive buffer.
Restore read interest and signal the wake when polling frees room.

## F3 — HIGH: Pump bounds and due deadlines are not enforced

Evidence: `crates/botster-core-host/src/driver.rs:390`, `:405`, `:438`, and `src/run.rs:96`.

The driver drains process exits and links before it starts budget accounting.
Link delivery can post any number of observations or completions.
The driver checks `posted >= bound` only between engine steps and checks no `pump_bytes` budget.
An engine step can also complete multiple Stop waiters and exceed `pump_events`.
If only link delivery posts events and no engine work runs, the report returns zero events and leaves the counter for a later pump.

Due deadlines appear after operations and sessions in `ready()`.
The production scheduler picks index zero for `ReadyWork`.
With `pump_events = 1`, one ready completion can exhaust the budget before an already-due capture expiry, silence deadline, or kill runs.
That capture remains readable after the pump that must expire it.

Authority: 9B, A2-7, TM-3, TM-4, TM-5, ST-6, and plan 2.4.

Required change: Account for all work and event publication within the pump bounds.
Report events from the current pump only.
Process each due deadline in that pump, with deadline ordering preserved.

## F4 — HIGH: Input capacity returns before Completed is polled

Evidence: `crates/botster-core-host/src/engine.rs:241` and `src/admit.rs:427`.

`complete` immediately decrements `input_ops` and `input_bytes`.
With `input_ops_per_session = 1`, Core admits a second write while the first write's Completed event remains unpolled.
The global pending slot is retained, but the session lane reservation is released.

AM-4 requires LaneFull capacity to return after an operation of that session completes and is polled.

Required change: Retain the session lane reservation until polling removes its Completed event.
Preserve the instance association when releasing that reservation.

## F5 — HIGH: Failed Create leaves already-admitted operations without completions

Evidence: `crates/botster-core-host/src/admit.rs:531` and `src/flows.rs:551`.

Admit Create and then Start before a pump, as AM-1 permits.
Inject a failure into the Create row write.
`flow_row` removes the session and completes only Create.
Start remains in `Await(Flow)` forever because its queued flow was in the removed session.
Create followed by Remove has the same problem.
An admitted metadata operation can instead index the missing session and panic.

Authority: AM-1, AM-3, ER-0, and A2-1.

Required change: Resolve every operation associated with the failed incarnation through its documented result path.
Do not leave waiters, queued flows, or ready operations attached to a removed session.

## F6 — HIGH: Remove can orphan operations, leak captures, or let old operations reach a new instance

Evidence: `crates/botster-core-host/src/flows.rs:454`, `:494`, `src/run.rs:224`, `:307`, and `src/inbound.rs:317`.

The Remove flow releases captures once, then removes the session without resolving all of its operations.
An Exited session can have an admitted read or capture while Remove runs.
A deferred ready operation indexes `self.sessions[&session_id]` after removal and panics.
An operation waiting for a worker reply can remain pending after release removes its link mapping.
A capture completion after Remove's capture-release step can create a capture that survives session release.

If the host recreates the same SessionId first, ready operations use that id without checking `PendingOp.instance`.
They can act on the replacement session.

Authority: AM-3, ID-1, LC-7, and ST-6.

Required change: Complete or retire all operations of the old instance during teardown.
Check the instance before every later operation step or reply.
Prevent late capture creation and release all captures before freeing the id.

## F7 — HIGH: Stop with a broken link signals the worker group instead of the payload group

Evidence: `crates/botster-core-host/src/flows.rs:334`, `src/run.rs:167`, `src/inbound.rs:537`, and `crates/botster-core-sys/src/process.rs:62`.

`WorkerHandle.identity` names the worker process created by `spawn_worker`.
The real process edge places that worker in its own group.
When the control link is absent, Stop and the grace kill signal this identity's group.
Core has no payload identity in this path.
It can kill the worker and lose the final model while leaving a separately grouped payload alive.

LC-5 requires Stop to end the payload group and retain the worker.
LC-6 requires signals to reach the payload group.

Required change: Provide a real edge that controls the payload group after control-link loss.
Keep worker termination separate from payload termination.
Prove both payload death and worker survival for the broken-link case.

## F8 — HIGH: Remove frees the id before worker termination

Evidence: `crates/botster-core-host/src/flows.rs:460`, `:475`, `:516`, and `:673`.

An authenticated RemoveResult immediately advances to row deletion.
Core does not wait for worker exit before Released and Completed.
If an Exited session has a live worker but no link, Core reports outcome_unknown and deletes the row without ending the worker.
Link loss during cleanup has the same problem.

LC-7 step 3 ends the worker before durable-row deletion and id release.
A6-3 requires teardown to continue and a stray worker to be ended even when cleanup has an unknown outcome.

Required change: Complete worker termination before steps 4 and 5.
Keep the authenticated cleanup result separate from the worker-exit result.
End a known live worker when its cleanup link fails.

## F9 — HIGH: Signal completes before the signal is sent

Evidence: `crates/botster-core-host/src/run.rs:255`, `src/engine.rs:231`, and `src/driver.rs:253`.

`send_msg` only queues an action and returns true when a link id exists.
`Next::Signal` then posts Ok immediately.
The driver can later encounter WouldBlock or BrokenPipe while sending that action.
No request acknowledgment associates the signal with its operation.
Core therefore reports success when no signal reached the worker or payload.

A2-1 defines Signal's Ok value as completion after the signal was sent.
Its asynchronous errors include SessionEnded and WorkerLinkFailed.

Required change: Track Signal until the edge or authenticated worker confirms the required effect.
Complete through the documented error path when the effect cannot occur.

## F10 — HIGH: A stop-row failure leaves StopAll pending forever

Evidence: `crates/botster-core-host/src/run.rs:389`, `:428`, and `src/flows.rs:603`.

StopAll starts a stop flow without a Stop waiter.
If that flow's registry write fails, `flow_row` restores Running and ends the flow.
The StopAll operation still waits for the target to become Exited or Lost.
No remaining flow, deadline, or operation can make that happen.

Authority: AM-3, LC-12, and the A2-1 StopAll row.

Required change: Handle target failure inside StopAll without leaving an unreachable completion condition.
Use the contract's target treatment and result path.
If the contract does not fix the recovery decision, send a QUESTION through the lead instead of inventing a result.

## F11 — HIGH: Atomic commit errors can incorrectly claim a certain failed write

Evidence: `crates/botster-core-sys/src/storage.rs:113`.

`file.commit().map_err(failed)` maps every atomic-write-file commit error to `StorageError::Failed`.
The pinned `atomic-write-file 0.3.1` performs `renameat`, then `fsync(dir)` in `imp/unix/mod.rs::rename_temporary_file`.
Its commit can therefore fail after replacement has occurred.
The extra directory sync in FileStorage is never reached on that error.
Core reports `RegistryFailed{uncertain: false}` even though the row may have changed.

Authority: AD-7 and the brief's typed failed-versus-uncertain storage results.
The Prior art note also incorrectly says this crate does not sync the directory.

Required change: Classify a commit error as uncertain whenever replacement may have occurred.
Use the library's actual commit semantics.
Correct the Prior art note and test the failure after replacement.

## F12 — MEDIUM: Created-session setters make visible progress in begin

Evidence: `crates/botster-core-host/src/admit.rs:641`, `:660`, and `:691`.

Resize changes `Session.size` and `request.size` during admission.
`get` immediately exposes the new size before a pump.
SetSizePolicy and SetColorProfile also change the stored request during admission.
A later Create completion can report the size of a Resize that has not run.

OR-1 and TM-2 reserve progress for pump.
AM-1 allows admission bookkeeping at begin; it does not move the visible cached state there.

Required change: Store admitted setters as pending work.
Apply their visible effects in pump and preserve required session ordering.

## F13 — MEDIUM: cancel uses incomplete identity history and accepts an unminted id as TooLate

Evidence: `crates/botster-core-host/src/admit.rs:477`, `:761`, `src/engine.rs:15`, and `src/flows.rs:524`.

Before any operation is admitted, `cancel(OpId(0))` returns TooLate because zero is less than `next_op`.
Core never minted that id.
For a long-lived session, `ops_seen` discards ids after 4096 operations.
After Remove, those old ids return TooLate instead of UnknownOp.
The global retired set also discards ids after 65536 entries.

ID-1 requires UnknownOp for an operation of a removed incarnation.
IN-6 requires UnknownOp for an id that this handle never minted.
The contract gives these rules no history-window exception.

Required change: Preserve exact identity semantics for the full handle lifetime.
Do not infer that an arbitrary smaller integer was a valid operation of a live instance.

## F14 — MEDIUM: The real facade returns an empty terminfo source

Evidence: `crates/botster-core/src/lib.rs:88`.

Core::open installs `terminfo_source: String::new()` in EngineConfig.
`terminal_identity()` returns that empty value.
TI-1 requires the source text of the pinned emulator's xterm-ghostty entry to ship with the library.
P1 owns the TI-1 ids. This placeholder is not one of the design note's stated package exclusions.

Required change: Wire the source of the pinned emulator into the real facade.
Keep the term and source from the same pin.
Coordinate the source artifact with P2 if necessary.

## F15 — MEDIUM: The host never uses the production session round-robin policy

Evidence: `crates/botster-core-host/src/driver.rs:419`, `src/run.rs:96`, and `crates/botster-core-edges/src/scheduler.rs::Production::pick`.

The driver always requests `ChoicePoint::ReadyWork`.
Production returns zero for that choice.
`ready()` lists sessions in BTreeMap order, so Core repeatedly advances the first session until it blocks or finishes.
The driver's host path never requests `ChoicePoint::Session`, which is the only choice that advances the production session cursor.
Under a small pump budget, later sessions can wait behind all work from the first session.

Plan 2.4 requires round-robin session visits with the pump bounds.

Required change: Connect host session selection to the production round-robin policy.
Keep control input priority and the seeded scheduler's allowed choices.
Prove visits across multiple ready sessions through HostDriver, not only through the isolated scheduler.

## F16 — MEDIUM: attach accepts a positive query deadline below 1 ms

Evidence: `crates/botster-core-host/src/admit.rs::applied_route_limits`.

The query-deadline check rejects only zero and values above the maximum.
With answers_queries enabled, `Duration::from_nanos(1)` passes and reserves a route.
EV-8(b) and A7-1 require a minimum of 1 ms.

Required change: Refuse every duration below 1 ms before reserving the route.
Accept exactly 1 ms and exactly the configured maximum.
