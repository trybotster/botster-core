# PR #219 — Failed-start route closure integration review

## Round 1 — 2026-10-10

Reviewed head: `b506a09b192a101c03cedd046cae17cf4f265bdb`.
Base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`.
Tier: HIGH. The PR changes shared packages and descriptor ownership through `HostEdges`.
Scope: the complete nineteen-file PR diff, R-50, the merge, tests, and supplied evidence.

The implementer initially requested a delta review from `46caa1b8`.
Neither reviewer had a prior verdict for that revision. The implementer corrected the request to a first full review.
This review does not assume an earlier acceptance of unchanged source.

### R1-1 — HIGH — Driver-held handoffs can close before the failed-start state with the wrong reason

The new engine path closes routes after the failed-start state when those routes remain registered.
The driver can close a route first when it still holds the stream in an outbound descriptor mark.

The following sequence is admitted:

1. Attach a route while the session is Starting.
2. Receive `Launched`, which moves the pending stream into a `HandoffRoute` action.
3. Keep descriptor transmission blocked, so the driver retains the stream in a mark.
4. Defer `PostRunning` until the next pump through the existing scheduler.
5. Report `ProcessExited` before that state step runs.

`inbound.rs:697-714` closes the worker link and changes the start flow to `PostFailed` with `Lost(WorkerGone)`.
The state event still requires a later `Work::Session` step.
`driver.rs:476-484` removes the link and queues `HandoffFailed` for each retained mark.
`perform_counted` feeds those failures before the pump enters its ready-work loop (`driver.rs:230-237`, `driver.rs:740-744`).
`inbound.rs:50-51` then closes the route with `HandoffFailed`.
With queue room available, the host posts that close before `SessionState{Lost}`.
The route is already removed when the new failed-start Finish path runs.

This sequence reports a blocked descriptor send, followed by worker loss. It requires no independent `DescriptorSendError::Failed` result.
R-50 requires the failed-start state first, followed by exactly one close with the reason of that state.
The expected reason here is `SessionLost`.

Preserve the driver-held stream and route until the failed-start state determines their close.
Add a driver-level regression test with a retained descriptor mark and loss before `PostRunning`.
Check event order, close reason, exactly one close, and stream release.
The new engine-only loss test does not model the driver's descriptor marks or its `failed_handoffs` queue.

Status: OPEN. I sent the finding to the implementer and the sequence to the package reviewer.
This finding limits the PR's R-50 completeness claim; it does not claim that the PR introduced the driver behavior.

### Other source checks

- Failed starts with engine-held streams enter the existing Stop Finish phase after posting their state.
- The flow retains queued operations and closes routes before completing waiting Stop operations.
- The existing capacity check includes closes from Finish and retains streams until the close can post.
- `close_route` removes an engine-held stream from pending handoffs before issuing `CloseRoute`.
- The shared codec helpers preserve the healthy-frame and failed-cause mapping used by the worker and testkit.
- The worker continues to apply frame bounds before queueing a close frame.
- The real edge supports both `UnixStream` and `OwnedFd`, writes without blocking, and releases the stream.
- The default tests cover the held stream, short write, gate, empty frame, frame bounds, and queued Remove.
- The slow test covers both real descriptor forms, end of stream, and a partial write without waiting for a reader.
- No pending list changes. The mapping transcript remains pending for its separate `uncarriable_sequence` limitation.

### Merge checks

The merge parents are `46caa1b8a738712aff358e3faaeee291ed53d710` and the stated base `d174ef48`.
The combined merge diff has no conflict resolution changes.
I compared the complete old and new PR diffs after removing index lines and hunk coordinates. They are identical.
The three shared paths contain the previously reviewed #221 additions alongside this PR's unchanged changes.
The base is an ancestor. `git diff --check` passes. Remote head and base match.
I read the complete PR description and the comment containing the failed base-merge check.

### Supplied evidence

Gate log: `botster-core-stage1-p4a-failed-start-routes-b506a09b-pool-20261010-045613-28848.log`.
The header names the exact head and base. All ten stages pass.
Default tests: 1,451 passed. Slow tests: 260 passed.
Both conformance reports show 193 passing trials, 407 pending trials, 70 entries without transcripts, two deferred trials, and 18 withdrawn trials.
Both mutation stages report 57 mutants: 53 caught, four unviable, no misses, and no timeouts.
The second mutation stage sets the slow environment variable but does not enable the `slow-tests` feature.
The five new host loss tests, driver forwarding test, and both stream-close tests have PASS results.
Full gate time is 552.5 seconds. The repeated mutation stage takes 124.3 seconds.
The pool job exits zero after 753 seconds: one queued second and 752 execution seconds.

The separate slow mutation log at `40ea69f79da8ebeddd16f5d4e07de734fe7e51e6` reports three caught mutants and no survivors.
Its command disables configuration, enables `slow`, uses the slow profile, and requests immediate fail-fast.
It catches the excluded whole-body replacement of `RealEdges::close_route_stream`.
The real edge source and its slow test are unchanged between that revision and the reviewed head.
This supports the narrow exclusion; the repeated default mutation stage does not supply that evidence.

I read the saved R-50 transcript results using contracts head `affab39`.
The Lost case and the isolated Exited case pass at `46caa1b8` and fail at the missing-close assertion on base `159cc400`.
The Exited result is a proof-only case, not a pass of the complete mapping transcript.
Those results do not cover the driver-held sequence in R1-1.
I ran no builds, tests, gates, or mutants.

### Verdict

NOT CLEAN: R1-1 remains open at HIGH severity.
The package verdict for this head is pending.

## Round 2 — 2026-10-10

Reviewed head: `5d55a0a8306fb91ce0ddb6487c64ca204808608c`.
Base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`, unchanged.
I read the complete four-file replacement diff, the updated PR body, and the new driver test.
Remote head and base match. The ancestry check and `git diff --check` pass.

### R1-1 — CLOSED for the reported two-event sequence

The driver distinguishes a retained mark lost with its link from a failed descriptor send.
The engine leaves `HandoffLost` to the Start or Stop flow instead of immediately posting `HandoffFailed`.
The new driver test holds the descriptor, defers Running, and delivers both process exit and link close in each order.
It checks Lost before exactly one SessionLost close and verifies endpoint release.
The supplied gate runs that test successfully.
Genuine descriptor-send failures retain their previous path.

### R2-1 — HIGH — Link loss alone discards a close obligation during late startup

The new `HandoffLost` branch ignores every route whose session has a Start flow (`inbound.rs:53-63`).
That condition does not establish that the flow will end the session or close the route.

The following sequence uses the new test's setup:

1. Attach during Starting.
2. Receive Launched and retain the descriptor mark because transmission is blocked.
3. Defer PostRunning.
4. Close the control link while the worker remains alive.
5. Release the deferral and pump until Start completes.

`on_link_closed` clears the worker link, but its Start guards exclude PostRunning and Finish (`inbound.rs:617-675`).
It does not set a startup failure or pending end for either phase.
The new `HandoffLost` handler discards the notification because the flow is still Start.
PostRunning then posts Running. `finish_start` completes successfully with no pending end and no requested Stop.
The route remains registered even though its stream was dropped with the driver's mark.
No worker can report that route's close, and the host has retained no close obligation.

The same gap exists if the link closes while a successful Start is in Finish.
The new test always supplies ProcessExited after LinkClosed, which hides the missing close when only LinkClosed arrives.
The existing running-session test reaches Flow::Idle before link loss, so it also misses this boundary.

Defer the close only when an end flow will perform it, or retain an explicit close obligation until that decision is known.
Add driver tests for link loss alone in PostRunning and successful Finish.
Require exactly one close without depending on a later process exit.
Preserve the two-event ordering test and genuine descriptor-send failures.

Status: OPEN. I sent R2-1 to the implementer and asked the package reviewer to check the same sequence.
This is a regression in the replacement's new deferral condition.

### Evidence and verdict

Gate: `botster-core-stage1-p4a-failed-start-routes-5d55a0a8-pool-20261010-051705-82976.log`.
The header names the exact head and base. All ten stages pass.
Default tests: 1,452 passed. Slow tests: 260 passed.
Both conformance reports show 193 passing trials, 407 pending trials, 70 entries without transcripts, two deferred trials, and 18 withdrawn trials.
Both mutation stages report 64 mutants: 60 caught, four unviable, no misses, and no timeouts.
The second stage still sets only the slow environment variable; it does not enable the slow test feature.
Full gate time is 428.7 seconds. The repeated mutation stage takes 134.3 seconds.
The pool job exits zero after 575 seconds: one queued second and 574 execution seconds.
The real edge source remains identical to the separate slow mutation evidence from Round 1.
I read package Round 144 at `9adde0deba393a0809f4646c049e1df0c47e6d57`; it confirms the earlier finding.
I ran no builds, tests, gates, or mutants.

NOT CLEAN: R2-1 remains open at HIGH severity.
The package verdict for this replacement head is pending.

## Round 3 — 2026-10-10

Reviewed head: `5c1b8d9040e2f3ec4faf3e77b3d6ae2ab39e8878`.
Base: `d174ef48a218b74beb4bac9ec555c6c39d2650a4`, unchanged.
I read the complete six-file replacement diff, the updated PR body, and all three new driver tests.
Remote head and base match. The ancestry check and `git diff --check` pass.

### R2-1 — CLOSED

The Start flow now retains each lost handoff in `Session::lost_handoffs`.
A successful start with no pending end closes those routes as HandoffFailed.
A failed start or pending end leaves route closure to the end flow.
`close_route` removes an obligation when that route closes.
The link-only driver tests cover PostRunning and successful Finish with the worker alive.
They verify one close, the expected reason, endpoint release, and no retained route.
The Stop-flow proof checks state-before-close ordering. The earlier two-event proof remains intact and passes.

### R3-1 — HIGH — Closing all retained handoffs in one startup step exceeds the event budget

`finish_start` calls `complete(f.op, result)` and then iterates over every retained handoff (`flows.rs:307-322`).
For a successful start with no pending end, each iteration calls `route_close`.
With mandatory queue room available, each call immediately posts a RouteClosed event.

The completion posts first (`engine.rs:321-333`). Its `step_mark` guard only detects an event posted before that completion.
The later route closes do not check that guard or the pump budget.
The driver calls `Budget::account` only after the whole `Input::Run` returns.

With `pump_events = 1` and one retained handoff, the Finish step can therefore post Completed and RouteClosed in one pump.
With multiple retained handoffs, the same step posts their closes as well.
Mandatory queue capacity is separate from the per-pump event limit; sufficient queue room permits this violation.

The new after-Running proof already configures `pump_events = 1`, but it does not inspect each PumpReport.
The test's eventual close assertions pass even when a pump exceeds its limit.

Process each close through separately budgeted work and retain the remaining obligations until their steps run.
Preserve state order, the first close reason, and behavior when the mandatory queue is full.
Add a driver proof with multiple retained handoffs and `pump_events = 1`.
Check every pump's `events_posted`, eventual completion, exactly one close per route, and no retained routes.

Status: OPEN. Both reviewers independently confirmed the same mechanism.
I sent the finding to the implementer. This finding concerns the new loop, not the closed link-only gap.

### Evidence and verdict

Gate: `botster-core-stage1-p4a-failed-start-routes-5c1b8d90-pool-20261010-083157-19297.log`.
The header names the exact head and base. All ten stages pass.
Default tests: 1,455 passed. Slow tests: 260 passed.
Both conformance reports show 193 passing trials, 407 pending trials, 70 entries without transcripts, two deferred trials, and 18 withdrawn trials.
Both mutation stages report 68 mutants: 63 caught, five unviable, no misses, and no timeouts.
The second stage remains an environment-only repeat of default mutation coverage.
All three new driver tests and the earlier loss-order test have PASS results.
Full gate time is 410.0 seconds. The repeated mutation stage takes 145.2 seconds.
The job exits zero after 563 seconds: one queued second and 562 execution seconds.
The real edge source remains unchanged from the separate slow mutation evidence.
The package's Round 145 verdict at `2c1fd9ea95290301fbad38f417f1ddf010786669` records the prior finding and unchanged scope.
I ran no builds, tests, gates, or mutants.

NOT CLEAN: R3-1 remains open at HIGH severity. R1-1 and R2-1 remain closed at their recorded scopes.
The package artifact for this head is pending.
This is the third NOT CLEAN integration round for #219.
The round-limit rule in contracts `56bd0a5:docs/BUILD.md:119` requires a lead decision before round four.
The lead must split the PR, settle the disputed point, or replan the root cause. A fourth review does not start by default.
