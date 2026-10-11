# PC package review: PR #233

VERDICT: CLEAN

- Reviewed head: `fce17f78b96009cb560fb3896a6f957840ec2c43`.
- Reviewed base: `11a5246a669de9e8ae6b7ca1ed7763192a099781` (`v1`).
- Risk tier: HIGH. BUILD.md rule 3 applies because the testkit is a shared crate.
- Open findings: 0.
- Scope: this PR's five changed files, its merge resolutions, and its 16 real-pending removals.

The binding plan is revision 23ah at `stage1-plan.247f8e96.md`.
The reviewer verified SHA-256 `b9c1871ce3ad595502a3716fc7490c9b4b9fd2f4ca5caa96b73e7b12b90c1daa`.
The contracts pin remains `contracts-v0.1.27`, commit `b161db87ca82873b4f6b21d8433a69f086abb7cf`.
The reviewer used the PC brief, BUILD.md, P3's real-form mapping, and the pinned transcripts.

## Logic

The harness sends the five program controls through the production probe's control socket.
The production worker owns the PTY. The harness writes no terminal bytes and changes no terminal mode.
The probe applies each request before its acknowledgement. Output acknowledgement does not wait for the terminal write to finish.
The harness returns the probe's cumulative input reports in order.
The blocked, accept, and chunk requests use the real forms that P3 specified.
The harness rejects empty or invalid output bytes and a zero chunk cap.

Both real harnesses supply their own probe socket. Only the tapped harness serves the controls.
The merged dispatch retains P5's controls and adds all five program controls.
The control module changes no Core clock. Only transcript `advance_clock` steps move the injected clock, as R-46 requires.

The input observer signals host wakes after input reports. It adds no Core input and changes no `edges_quiet` condition.
The PR body states P3's observer-wake exception and its `no_spurious_wakes` limit.
The temporary accept exception identifies the non-blocking listener, bounded poll, and P6's replacement obligation.
The reviewer checked its implementation against that stated exception.

## Earlier source findings

The reviewer checked all three integration source findings and their corrections on this head.

- SI-1 HIGH is closed. Request writes use a non-blocking socket outside the state lock. Each write checks the shared deadline first.
  The acknowledgement wait uses that deadline. The request-start fence precedes the report-progress assertion.
  A peer that does not read requires Bad from the write deadline path.
- SI-2 MEDIUM is closed. The source removes detached exit watchers. The accept thread owns and joins the report readers.
  Harness cleanup shuts down their sockets and joins the accept thread.
- SI-3 MEDIUM is closed. Probe state uses the complete worker `ProcessIdentity`, including start time.
  The existing-connection proof rejects a different start time for the same PID.

The retained SI-1 reversal fails with the state lock held during the request write.
The same test passes on the unmodified head.
Log: `~/botster-sessions/gates/botster-core-pc-si1-lock-held-reversal-686300e2-20261010-184710.log`.
The reviewer read that log and the reversal diff. The reviewer ran no reversal job.

The later mutation corrections retain partial report lines across read deadlines.
Socket shutdown ends those reads during cleanup. The accept-error decision preserves Wait, Next, and Stop.
The reviewer checked the new proofs for those decisions, the default blocked value, and all five controls.

## Lists and merges

The group has 38 IDs after #222 and #227. This PR removes 16 IDs and leaves 22 pending.
The three minimum removals are `ad_3_adopted_equals_started` and both `tm_3` IDs.
The reviewer read their pinned transcripts. The `ad_3` output and input fences remain intact.
Both `tm_3` transcripts retain their explicit clock steps and exact deadline assertions.
The four minimum route IDs remain pending.

All six IDs that need steward approval remain pending.
`ev_5_full_queue_defers_transition_not_output` and the five IDs from #227 remain pending.
The other status lists and the minimum list match the current base.
The contracts pin, lockfile, and mutation configuration also match the current base.

The reviewer read the conflicting merge changes. The resolved source retains P5's control dispatch and the program-control dispatch.
The resolved list includes the five new #227 IDs and preserves the PR's 16 removals.
The program module is byte-identical between gated head `7a4ab70b` and reviewed head `fce17f78`.
Its blob is `2a83304cc10f4e2463bc41d90abeced1d62fb434`.
The reviewer does not treat the failed base-merge check as an automatic approval.

Two IDs pass on Linux and remain pending under the lead's ruling (A), message `msg_plugin-w_1791698173_6bb0a3`:
`ev_2_one_marker_per_session` and `in_5_lane_bounds_refuse_at_begin`.
The lead says a race that passes on an idle node is not proof.
The input-log fence requires steward review. The event-marker cause on macOS requires separate analysis.
The PR states these conditions and does not remove either ID.

## Gate evidence and merge requirement

The lead's pool rule is message `msg_plugin-w_1791697703_733a2c`.
It permits one pre-review gate, review of the base-merge changes, and one final gate after both CLEAN reviews.
The implementer supplied both lead messages verbatim in `msg_plugin-w_1791698260_ea0cc8`.

The pre-review gate passed on `7a4ab70bf08ff28344e9e47cad09d923ff3ae1d4`, base `15791ea336c5f3d0fbd9483a1e190ec01743351e`.
The job moved from Gaming to Linux and completed a full second run.
All CI steps passed. Each of the 16 removed IDs has real-tier PASS evidence in that log.
The final mutation result is 59 caught, 0 missed, 0 timeout, and 20 unviable.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-7a4ab70b-pool-20261010-201116-66449.log`.

**The final gate on the CLEAN head remains required before merge.**
The older gate does not establish that `fce17f78` passed CI.
The integration reviewer must also report CLEAN on this head.
A later code change requires a delta review.

The reviewer ran no tests, build, gate, or mutation job. The reviewer checked logic and the supplied evidence.
This verdict clears PR #233. It does not clear the 22 pending IDs or complete the PC group.

---

# Earlier review: PR #231

VERDICT: CLEAN

- PR: `trybotster/botster-core#231`.
- Reviewed head: `4521237e8200768a36ab4c8a27f268f3914a88ab`.
- Reviewed base: `6c2bccdad9ab6cfac860e63c5eb15d6f8ec5ac6c` (`v1`).
- Risk tier: HIGH. BUILD.md rule 4 applies because the PR moves the contracts pin.
- Open findings: 0.

The review covers the contracts pin move only. The program controls still need their implementation review.

The binding sources are `brief-program-controls.md`, contracts `docs/BUILD.md`, and plan revision 23ab at `stage1-plan.77739f96.md`.
The verified plan SHA-256 is `969711aafa5e79cd617b3a6b9777ac6042c2116c038b9fc8a20b5ca6572dc682`.

The seven manifest dependencies name `contracts-v0.1.27`.
The tag resolves to `b161db87ca82873b4f6b21d8433a69f086abb7cf`, as plan section 0 requires.
All nine contracts sources in `Cargo.lock` use that tag and commit.
The lockfile has no other package change. The manifest has no other value change.

The pending, real-pending, held, deferred, ledger, and minimum ID lists are unchanged.
The PR changes only comments in `core-pending.txt` and `status.rs`.
The six IDs that need steward approval remain pending.

The reviewer checked the dependency changes to `CoreHarness::probe_env`, operation normalization, and the probe's default path.
The harness keeps the empty default environment. The Core harnesses do not enable the control socket on this head.
Only `ad_3_adopted_equals_started` changes among the original 32 PC transcripts.
Its new ActivityOutput fence precedes the terminal-state check. The ID remains real-pending.
The contracts manifest and ID lists do not change between the two tags.

The existing full Linux gate passed on the reviewed head and base.
The gate ran `env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci` through the pool.
Every CI step passed. The mutation step found no mutant in the diff.
The gate exited with code 0 after 816 seconds.
Gate log: `~/botster-sessions/gates/botster-core-stage1-pc-pin-v0.1.27-4521237e-pool-20261010-145612-73744.log`.

The reviewer ran no tests. The reviewer checked logic, parsed the manifests, compared the lists, and read the supplied gate evidence.
The integration reviewer must also report CLEAN before the lead merges this HIGH PR.
