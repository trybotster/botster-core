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
