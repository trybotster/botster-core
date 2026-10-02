# P1 lifecycle review

VERDICT: NOT CLEAN (16 open)

Reviewed head: `fb75dec1b6a00270c89f63ce6b67357892e060ff` on `stage1/p1-lifecycle`.
Initial code checkpoint: `6a8017621f024cbf6c07a3f9b9c50deae15fb936`.
I also reviewed the delta through `eb39516`, `3ca7680`, and `fb75dec`.

Base: `7c0ae16` (P0 and the merged P6 testkit). The review covers P1's dependency move and code commit.
Authority: plan pin `555bc433`, BUILD.md, and the P1 brief.
The initial checkpoint pins manifest final14 through `contracts-v0.1.2`.
The latest head pins manifest final16 through `contracts-v0.1.4`, following the lead's correction reported by the implementer.
This is a logic review. I did not run the gate or a test suite.

The design note contains the required Prior art note. The engine uses injected inputs and actions.
I accept the JSON construction of `RemoveReport`: the pinned type has no public constructor.
I exclude the stated P5 adoption, P4c WebRTC, P4a descriptor handoff, and P7 service implementations from this checkpoint.
Those exclusions do not establish package or Stage 1 acceptance. The pending list must retain unproved ids.
The latest tag fixes the six unit-valued Remove transcripts and supplies Amendments 7 and 8.
Those ids remain pending until both harnesses prove them.
The delta adds A8-1 capture reservations. I found no additional defect in that reservation change.
The delta does not close F1 through F15. F16 also applies under Amendment 7.

All findings are OPEN. Each finding must close before CLEAN.
Line numbers below refer to the initial code checkpoint. The cited logic remains in the latest head.

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
