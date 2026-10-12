# PB B1 exit-watch integration review

## Round 1: source and completed initial gate

Exact head: `0e25e27da6606a27cf2ff94df5d139c80f27b4d5`.
Tree: `f71a1fe3893b4afb977fc17469d92e935a0588d2`.
Source and gate base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Lead assignment: `msg_plugin-w_1791836382_d0b936`.
The base is an ancestor of this head. The source diff check passes.
The complete full-index patch SHA256 is `007a10d11dac8068af01250ec185916b4a8503fa5dc9d58939c4a42ce3e85274`.
The patch changes eleven files, with 362 additions and 27 deletions.

Sol-B reads the complete source delta, affected ownership and cleanup paths, and complete PR body.
Sol-B reads R-106 (a)-(c) and the stderr follow-up at contracts `3d590705`.
The follow-up accepts stderr as the worker's own log, including when Core directs stderr to /dev/null.
The errno must not enter host or client detail text. The rule requires no durable log sink.

### Package boundaries and ordering

The sys watch returns Result<ExitStatus, ExitWatchFailed> to its single callback.
The wait still uses WEXITED and WNOWAIT. The watch leaves the leader unreaped.
EINTR retries. Every other errno, including ECHILD, returns ExitWatchFailed with the raw errno.
Only an observed waitid status constructs a payload ExitStatus.
The watch thread sends the result and wakes the driver. It signals no process or group.

The driver maps the error to Input::ExitWatchFailed without creating a payload exit status.
The machine defers a failure during Spawning until Spawned establishes its owned live payload.
It applies that failure after Launched and returns before try_start.
After an observed exit or an ended worker, the handler does nothing.
Otherwise, it queues SIGKILL unless a group kill was already queued.
Worker::signal acts only in PayloadState::Live, which retains the unreaped leader.
SignalPayload reaches the driver's owned Payload and its recorded process group.
Payload::signal_group refuses a payload without its child. No foreign group identity enters this path.

The handler queues no Exited, other end message, or ReapPayload.
It closes a pending candidate and ends through Action::ExitFailure.
The real driver's perform returns an error for that action.
The settle loop propagates the error before any later action or input runs.
Driver::run fails. The existing command-line path returns ExitCode::FAILURE and writes the error to stderr.
No errno enters the control link or a detail field.
Payload drop sends its existing group kill while it retains the child, closes the PTY, and waits for the leader.
The machine suppresses a duplicate kill action; the existing payload drop can send its cleanup kill again.

The testkit injects an edge input into the same production worker machine.
The injection belongs only to testkit tests; it introduces no production test branch or conformance control.
After the edge delivers the failure, watch_failed suppresses further simulated payload exits.
ExitFailure records worker status Code(1) and uses the existing ended cleanup.
That cleanup closes the worker link, endpoint, candidate connections, and route streams, and drops the simulated payload.
The host receives the worker exit and link end through its existing classification path.
The host's production code does not change.

The testkit proof keeps time fixed and uses a payload that ignores SIGTERM.
A pending Stop waits before the injection. After the injection, its result is End.
The only subsequent states are Stopping and Lost(WorkerGone); no Exited appears.
Both worker_alive and payload_alive report false.
This proof isolates the failure from Core's stop-grace deadline.
Machine tests separately check the exact kill and exit actions, graceful stop, prior kill, spawn, observed exit, and duplicate failure.
Sys tests inject EIO and ECHILD after EINTR and require an error without an exit status.
The zero-start-time test checks the existing fail-closed identity behavior; it changes no production identity code.

Sol-B independently searches the production payload and worker paths for ExitStatus constructions.
The only payload constructions are payload.rs:338 and :341, from the waitid result.
Worker-core occurrences are pattern matches, including route exit handling.
The lead limits this evidence to the payload path.
The host worker-reaper sites process.rs:90 and :106 remain PB-W1 for B3; this review does not close them.

### Design and package receipt

Design: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/handoffs/pb-b1-design.md`.
Current design SHA256: `6990ade90135444d2e3bf38a359f61dd369072f20fbe00a35b6184fd5afb13d7`.
Sol-B reads the complete design and prior-art note, also present in the PR body.
The note names old Core `72b2e335`, reused ownership paths, rejected approaches, the new error type, and no dependency change.
The package reviewer checked those old references. No copied old source or tests are claimed.
Package finding PB1-1 LOW remains closed by that note.

Sol-B reads the complete package source receipt `1ab2adf1f17566136deb5d32bf4d37959548eecb`.
Sol-B requests its completed gate receipt because the first version still says the gate is running.
Sol-B then reads the complete updated package at `7dc06d4b7e84f6c668f2cd06aa23cd51d0b8f0ec`.
It records the completed exact-head gate and retains CLEAN. Source and evidence results agree.

### Execution evidence read

Completed pool gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-pb-b1-exit-watch-0e25e27d-pool-20261012-130618-58413.log`.
SHA256: `0de859daa3399bde2db85985186568a5df9694cb91947c0460f2667fb715d15f`.
The log names this exact head and base. All ten steps PASS; their total is 557.2 seconds.
The job and wrapper exit 0 on msa1 after 757 seconds: 191 queued and 566 running.
The default run passes 2087 tests, with 411 skipped, in 14.915 seconds.
The ordinary slow run passes 677 tests, with 2081 skipped, in 45.303 seconds.
All ten new tests pass in the default run. The changed real-driver watch-result assertion also passes in the ordinary slow run.
Strict conformance passes 281 with zero failures. No conformance identifier changes in this patch.
The fuzz step reports no changed crate with a decoder harness; it passes with no harness run.

Mutation stage 1 records 21 identities: 17 caught, one missed, three unviable, and zero timeouts.
The unmutated stage 2 baseline passes 687 tests, with 2911 skipped, in 43.376 seconds.
Stage 2 catches the sole missed Driver::run mutant with slow_driver::a_client_close_ends_its_real_route.
Final results are 18 caught, zero missed, three unviable, and zero timeouts.
Sol-B independently matches all 18 raw catch identities to their citations.
Every cited test passes in the ordinary default or slow run, before mutation execution.
No runtime FLAKY or INCONCLUSIVE result appears.
The inherited EV-2 entry appears only as recorded diagnosis and mutation exclusion; it earns no catch credit.
This review grants no retry authorization.

The earlier d8358ee9 gate remains RED: clippy rejects the discarded Result in driver_edges.rs.
Its SHA256 is `fd1123961209d4ee7264d093b34d371df04d07e23b9be17f98ae5c4d3da322c2`.
All steps after clippy are NOT RUN. The corrected expression explicitly requires a successful watch result.
The production handler remains unchanged between that head and this head.

Sol-B reads the complete focused reversal log at d8358ee9.
Path: `/Users/jasonconigliari/botster-sessions/gates/botster-core-pb-b1-reversal-d8358ee9-20261012-200131.log`.
SHA256: `6a3faf1095d678f21a8a1cd133d7b6132eed7caa46fac3bf4f40a68f5560fed0`.
The scratch handler returns immediately. Five tests fail with assertion evidence, including the testkit Stop proof.
The two suppression tests pass as expected. This is focused reversal evidence, not a full gate.
The log's shared-target cleanup and single-worktree statements remain owner reports, without independent execution verification.
The Linux gate supplies no new macOS execution claim.

### Disposition

No source, interface, body, or supplied-evidence finding remains on this exact head.
No conformance list, contracts pin, dependency, timeout, budget, profile, or mutation exclusion changes in this patch.
This CLEAN accepts the source and completed initial exact-head gate.
Current-base delivery, any assigned train delta, its final receipt, and lead merge checks remain due.
This receipt accepts no later head, completed merge, PB-W1 repair, or additional retry.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; exact PR head and completed initial evidence)

## Round 2: train 8 source delta

Exact train head: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
Tree: `3f6c95e20e26a54eca52908502a4bade7aec1cad`.
Train base: `007efbde6a59af403d0937c38361c137a8961a48`.
Frozen member: `0e25e27da6606a27cf2ff94df5d139c80f27b4d5`.
Lead request: `msg_plugin-w_1791837736_3600b2`.

The train merges #325, #327, #324, and #329 in that order.
Sol-B independently compares the applied #327 patch with the reviewed member's own patch.
All eleven path sets and added/deleted line sets match, excluding object indices and hunk positions.
Ten final source blobs equal the frozen member byte for byte.
The remaining worker/tests.rs composes #325's separate read-order test and the exit_watch module declaration.
Neither addition changes a shared fixture. The failure tests and their exact action assertions remain intact.

The sys watch, worker failure handler, real driver, and testkit failure injection and cleanup remain exact.
The input-log cache changes no failure state or payload ownership.
The held-full selection applies only to conformance transcripts; it does not select or suppress B1's unit tests.
The real resume fence changes no exit-watch path.
The .46 pin changes the train's contract inputs, but adds no new B1 message, Lost reason, errno field, or Stop result.
R-106's kill-before-reap and stderr-only error path remain intact.
PB-W1 remains outside this review and scheduled for B3.

Sol-B reads the complete train 8 package section at `8aad5e51a43050d09d9dea7be10f6fdaa599b7cf`.
Its exact-head source CLEAN agrees with this review. All prior closures remain applicable.
The source review finds zero open issues, including LOW.
The lead's ID-set, compile, and strict-conformance pre-checks remain operator reports.
This source delta accepts no completed train gate, later head, merge, or retry.
Sol-B owns the combined review and single final receipt after the completed exact-head gate.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; #327 train 8 source delta only)
