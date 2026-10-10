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
