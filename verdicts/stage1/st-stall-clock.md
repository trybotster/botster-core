# PR #298 integration review

## Round 1

Exact head: `0f9079c44e028644c6d1724c9dae9cd8b03d0459`. Reviewed and gate base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`. Lead assignment: `msg_plugin-w_1791771329_97e289`. Branch: `stage1/st-stall-clock`. Risk: HIGH.

Sol-B reviewed the host/worker clock protocol, queue reservation and driver boundary, progress report order, both testkit control paths, conformance changes, design note, package verdict, and complete PR body.

The host owns both deadlines on the injected clock. The worker owns route transitions and accepted-byte progress. The messages carry the worker's progress mark. No worker stall timer or production test branch is added. Terminal snapshots still come from the existing model and codecs.

The clock accepts the three new clock reports only after RouteReceived and from the route's session. The worker sends the receipt before reports for its bound route. The declared DP-8 adoption limit remains: surviving routes are not restored in this PR. The pre-existing stale RouteClosed limit is retained for DP-8. This review approves no DP-8 implementation.

The testkit and real tap share RouteReports. Both route_accept paths set the progress base before transport acceptance. The progress answer requires a later consumed mark. This proof retains the ruling's scope: no accepted bytes between the previous fence and route_accept. It does not replace the required report-order guarantee.

Package verdict: `2dab92da9aea9798256916956de2ad9a08eda7c1`, `verdicts/st-stall-clock.md`. Sol-B read the exact-head findings and independently checked the relevant boundary paths below.

### ST-I298-1 — HIGH — OPEN: the reservation blocks its release report

Matches ST298-R1. With one empty mandatory slot, the due close reserves it before the driver reads an already queued RouteResumed. The acceptance check permits RouteStalled and RouteClosed to use a reservation, but rejects RouteResumed. The FIFO held-frame path then blocks the report that must release the reservation. No queued event exists for a poll to remove.

An earlier mandatory frame can also block the due answer behind it on the same link. Accepting only RouteResumed does not resolve that case.

Required change: implement the steward's R-90 follow-up at contracts `1798a65d`, relayed by the lead in `msg_plugin-w_1791771819_28471d`. Only an earlier mandatory frame from the answering link, waiting only for room, can take over the slot. The answer then becomes an ordinary parked report. Preserve the mandatory bound, FIFO, event budget, and host-visible state at event post.

Require a driver proof of the empty-queue deadlock, takeover, parked answer, and post after a poll. Require a proof with several routes due together. Record the rule in DESIGN.md.

### ST-I298-2 — HIGH — OPEN: failed resume leaves incompatible clock states

Matches ST298-R2. On baseline failure, the worker clears its stalled state and starts or retains a healthy close without reporting resume. If writes then block, the host retains the old close clock. Its due message reserves a slot, but the worker declines it because the stalled state is gone. No answer releases that slot.

Required change: define and implement the report and clock state after baseline failure. Preserve the first close reason and typed close. Prove one accepted byte, failed baseline, then blocked writes across the old close deadline. Cover both a new SnapshotTooLarge close and an existing healthy close.

### ST-I298-3 — MEDIUM — OPEN: a new deadline retains old parking state

Matches ST298-R3. route_progress restarts the waiting deadline without clearing parked. While the queue remains full, deadlines_at hides that new deadline although its due step never ran.

Required change: clear obsolete parking state when progress invalidates the old step. Check the other transitions. Prove that next_deadline exposes the new deadline after progress while room remains full. Check its budget and room behavior when it becomes due.

### ST-I298-4 — MEDIUM — OPEN: Output bypasses the progress flush

Matches ST298-R4. report flushes Output before route progress. LinkWritten also flushes Output before route_progress_written. report_output sends directly. A later Output report can therefore reach the host before its pending progress report, contrary to the R-90 order rule.

Required change: apply the order rule to Output flush paths. Prove link order with earlier unwritten Output and progress reports, followed by new output without a title. Preserve the coalescing bound.

### ST-I298-5 — MEDIUM — OPEN: exact-head Mac evidence is missing

Matches ST298-E1. Plan section 8, revision 23ai, requires focused Mac evidence for these route writes, link reads, and descriptor receipts. The earlier #295 evidence does not cover the new worker resync or reservation protocol.

Required closure: supply the completed focused Mac log on the reviewed implementation head. Preserve all failures and their dispositions. Obtain package CLEAN on that same head after the source repairs.

### Evidence and accounting

Linux gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-0f9079c4-pool-20261011-185413-82688.log`. The header names the exact head and base. All ten steps PASS. Default: 1,897 passed. Slow: 609 passed. Mutants: 230 total, 211 caught, 0 missed, 0 timeout, 19 unviable. Three catches come from stage 2. The job exits 0 after 919 seconds; the wrapper exits 0 after 920 seconds. These results do not close the source findings.

Six stall IDs leave testkit pending and enter real pending because the real transcripts lack consumption fences. The two additional e3 removals belong to inherited #295. Three lead-approved budget holds remain pending and run through held-full. The gate records five budget IDs at 32 seeds, one trial per seed. Sol-B found no additional accounting defect in this scope. No further ID removal is approved.

The A3-1 output_dropped and EV-8 query exclusions remain explicit and pending. The body states the adoption limit, prior art, earlier red gates, and required fence work. The later current-base merge and final gate remain due after source and evidence acceptance.

Sol-B ran no local build, test, gate, mutation job, reversal, or reproduction campaign.

VERDICT: NOT CLEAN (5 open)

## Diagnostic receipt — one re-gate attribution

Exact head: `1d044f8bb53cb7ee1e0df302d24bfaef1fd83ce6`. This receipt addresses plan 23al condition 2. It is not a round-2 source verdict or CLEAN.

The red gate `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-195551-10795.log` fails slow on `conf::in_9_modes_read_at_write_start`, seed 0, step 8, `limit=step_timeout`. The job and wrapper exit 1.

PC's retained reproduction is `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/pc-r78/in9-repro-f6025aed.log`. It starts from v1 `f6025aed4cdf7da836e835e0040fc128b7e8f32f` and applies a scratch delay of 300 ms to mode output. It reproduces the exact ID, seed, step, and symptom. The change moves timing only, adds no product behavior, and changes no expected result. This is controlled reproduction evidence, not an unmodified-v1 run. The wrapper's exit 0 does not make its failed test green.

Sol-B read both logs, the pinned .41 transcript, the R-88 diagnosis, and the source delta. The transcript attaches no route. It starts a focus write under pty_blocked without the required held-write precondition. Its ModesChanged wait can discard an earlier completion before the later completion wait.

The host input transaction, worker input/model, real program control, and PTY backend paths are unchanged. The new progress flush has no route to report. The changed code supplies no missing held-write fence and does not change the diagnosed cause. Sol-B confirms non-interference for this exact failure.

Package attribution receipt: `d0e5f664dfaabd195534b06d1612c33173963bff`. Sol-B read it. The lead explicitly accepts this controlled reproduction under 23bl(A) and authorizes ONE re-gate in `msg_plugin-w_1791774104_a9ca69`. The repair owner is PC; the fence is assigned to contracts #79 and its next pin.

The one re-gate must pass every step on this exact head. This receipt authorizes no further retry or pending-list change. The five findings remain open until the round-2 READY and completed evidence. Sol-B ran no local build, test, or diagnostic job.

VERDICT: NOT CLEAN (5 open)

## Round 2

Exact head: `1d044f8bb53cb7ee1e0df302d24bfaef1fd83ce6`. Source base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`. Gate comparison base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`. No new base merge belongs to this round.

Sol-B read the complete source repair, new driver/host/worker proofs, exact-head Mac evidence, authorized re-gate, updated body, and package verdict `9d59fbec865c5f9d09b86467dd269e31d76213e2`.

The original five findings are CLOSED:

- ST-I298-1: asked state and slot ownership are separate. Same-link takeover preserves the asked step. The driver proofs cover the empty-queue resume, parked answer after a predecessor, and several due routes. Bounds, FIFO, and the steward's rule remain explicit.
- ST-I298-2: a failed baseline retains the original stalled mark. The due close can end blocked writes with the first reason. Worker proofs cover SnapshotTooLarge and an existing healthy close. The host proof covers the typed close and slot release.
- ST-I298-3: a replacement waiting step, stall, or resume clears obsolete parking. The new proof exposes the future deadline while room remains full and checks later parking.
- ST-I298-4: report_output flushes route progress first. The new proof covers pending Output and progress under link backpressure.
- ST-I298-5: completed exact-head Mac evidence supplies the changed checks. Its broader failures and dispositions remain recorded below.

### ST-I298-6 — HIGH — OPEN: clock transitions precede their parked events

Matches package ST298-R5. Sol-B independently checked inbound.rs, stall.rs, and run_parked. inbound applies route_stalled before route_event. That call releases the slot and arms close_at at the report-consumption pump. route_event can then append the stall event behind existing ParkedRoute work. run_parked posts that event later without applying its clock transition at the post.

With the last slot reserved and an older parked close waiting, consuming the stall answer releases room for the predecessor. The predecessor fills the queue. RouteStalled remains unposted, but next_deadline exposes its close clock. A delayed next poll can therefore permit CloseDue before RouteStalled posts. The resume path also changes its clock before a deferred event.

Required change: apply each host-visible stall/resume clock transition atomically with its event post. Preserve the report's mark while it waits. Use the posting pump's time for stall_close_after. Keep event order, mandatory bounds, and event budget.

Require a driver proof with an older parked event, the last slot reserved, and a delayed second poll. No close deadline can appear before RouteStalled posts. The full close interval must start at that post. Cover a deferred resume as well. Sol-B sent this finding to ST and 0181.

### ST-I298-7 — LOW — OPEN: the body overstates two evidence facts

The body calls the re-gate's base 20b2525c. The retained gate header names comparison base bfef8865; 20b2525c remains the source base. The body also calls the AM-2 failure a Mac-only defect. The logs establish Mac failures and sampled Linux passes, while R-93 diagnoses a missing cross-source arrival fence.

Required closure: distinguish the source and gate comparison bases. Describe the observed platform results without claiming that the defect is exclusive to Mac. Preserve the failures and R-93 disposition. Sol-B requested these corrections from ST.

### Completed evidence and limits

The one authorized re-gate is `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-200112-36429.log`. All ten steps PASS. Default: 1,907 passed. Slow: 609 passed. Held-full runs five budget IDs at 32 seeds. Mutants: 247 total, 227 caught, 0 missed, 0 timeout, 20 unviable. Three catches come from stage 2. The job exits 0 after 1,133 seconds; the wrapper exits 0 after 1,134 seconds on gaming. The earlier red in_9 gate remains recorded in the diagnostic receipt above. No further retry is approved.

Mac log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-1d044f8b-pool-20261011-195909-27591.log`. The five changed packages pass 1,046 of 1,046 tests. The broader real run uses strict seeds 0-31, passes 240 of 242, and exits 100. This is not a green broader run.

- ev_5_route_closed_retires_route_keyed_events: seed 13, step 28, activity_before_close false after close. This is the retained R-66 missing input-consumption fence. The repair remains contracts #69 through its pin.
- am_2_one_admission_point_orders_all_sources: seed 1, step 14, BAC instead of ABC. The isolated base log `/Users/jasonconigliari/botster-sessions/gates/st-am2-isolated-base-20b2525c.log` reproduces the same ID, step, and bytes at seed 5. It records a target clean before fresh compilation from the separate base worktree. R-93 diagnoses the missing route-frame arrival fence before the host write. The lead assigns PC the route_frames_taken control and transcript fence. The admission queue is unchanged by #298. Its route timing can change arrival timing, but supplies no missing fence.

The package verdict retains both broader failures and closes its original evidence finding. No conformance file changes in this repair delta. The earlier accounting remains in scope. No additional pending removal is approved.

Sol-B ran no local build, test, gate, mutation job, reversal, or reproduction campaign.

VERDICT: NOT CLEAN (2 open)

## Round 3

Exact head: `76b369c7a39bec37a2bdb3c31110c887f60d4edb`. Source base remains `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`. Gate comparison base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`. No new base merge belongs to this round.

Sol-B read all five commits and the complete six-file delta from `1d044f8b`. The delta changes host source, proofs, and DESIGN.md. It changes no worker, link protocol, testkit, process, PTY, socket, conformance file, or contracts pin.

ST-I298-6 is CLOSED under the lead's approved rule: add at post, remove at consume. The lead records this decision in shared/core-stage1/handoffs/core-lead.md. The steward records no objection. This decision supersedes Round 2's requirement to defer the resume's deadline removal until its event post.

ParkedRoute::Stalled retains the worker's mark. post_route_progress arms the close deadline only after post_mandatory succeeds. route_stall_posted uses the posting pump's time and the full stall_close_after interval. The parked runner removes the current event before that call. A later parked event for the same route suppresses the deadline, because Core already consumed that route's resume or close. Posting an older stall event therefore cannot restore its ended close step.

RouteResumed removes the close deadline at consumption and releases an outstanding close reservation. Its event retains the existing parked order. Waiting until the event posts can let a due close take the only slot. The resumed worker can then decline that close without an answer, which prevents the resume event from posting. The lead-approved rule avoids this cycle. DESIGN.md states both parts of the rule and their reasons.

The new driver proof uses an older parked close, the last reserved slot, and a late poll. It observes no close deadline before RouteStalled posts. It checks the full close interval from that post and the retained mark in RouteCloseDue. Two resume proofs cover takeover of a due close's slot and consumption before any close step runs. The engine proof covers a stall with its resume parked behind it. All four proofs pass in the completed gate. Event order, mandatory bounds, and the existing event-budget rules remain intact.

ST-I298-7 is CLOSED. Sol-B read the complete updated body. It distinguishes the source base from the comparison bases for both gates. It attributes AM-2 to the general R-93 missing arrival fence. It states the Mac failures and sampled Linux passes without claiming a Mac-only defect. Earlier failures remain recorded.

Completed gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-stall-clock-76b369c7-pool-20261011-205842-74210.log`. Its lists step names source merge base `20b2525c`, distinct from comparison base `fcc09461`. All ten steps PASS in 826.8 seconds. Default: 1,911 passed. Slow: 609 passed. Mutants: 260 total, 239 caught, 0 missed, 0 timeout, 21 unviable. Stage 2 tests the five stage-1 misses, catches three, and reports two unviable. Its unmutated baseline passes. No FLAKY result appears. The job and wrapper exit 0 after 834 seconds on gaming. The stopped `726f2547` gate supplies no completed result.

Reversal excerpts: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/handoffs/st-298-r3-red-on-revert-excerpts.md`. Reported SHA256: `786db4b3addbaccfff85dcfd7a0fbd374e9392a945f18843a29cc653ff2371dd`. Sol-B read the complete record. It describes the old-source reversal, the disabled stall-post guard, and the rejected at-post resume variant. It records their filtered failures and restoration.

ST copied these excerpts from its session transcript after the runs. No original log file exists. The record reports the same worktree and target for these checks. Sol-B retains these facts as ST's owner report. Sol-B does not claim independent verification of the original output, cleanup, or artifact provenance. The reviewed source and completed pool gate independently support the repaired behavior. Sol-B requested no rerun.

The original five findings remain CLOSED. Round 2's Mac evidence retains its exact earlier-head scope and its broader failures. Its selected packages passed 1,046 of 1,046; the broader real run passed 240 of 242 and exited 100. The R-66 and R-93 dispositions remain applicable. Round 3 changes no OS-dependent transport or worker path. It claims no new Mac run or repaired transcript fence.

Package CLEAN: `7c422c3c1e0e5f4a61b310f863160b1c67821a7e`, `verdicts/st-stall-clock.md`, on this exact head. Sol-B read the complete Round 3 section. Both reviews close the remaining clock finding and retain the evidence limits. No package or integration finding remains.

This CLEAN covers this exact source head and its supplied initial evidence. The current-base import, applicable base review, final exact-head gate, and final body remain due. The adoption and other pending limits remain recorded in the earlier rounds. This verdict approves no additional status removal or later contracts pin.

Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open)
