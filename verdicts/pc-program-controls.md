# PC package review: PR #233

VERDICT: CLEAN

- Reviewed head: `ec317eaedbfa29467ceb68cb61b0919b717f3a3f`.
- Reviewed base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17` (`v1`).
- Risk tier: HIGH. BUILD.md rule 3 applies because the testkit is a shared crate.
- Open findings: 0.
- Scope: this PR's five changed files, its merge resolutions, and its 16 real-pending removals.

The current binding plan is revision 23ai at `stage1-plan.c7cbc474.md`.
The reviewer verified SHA-256 `ea4b778d22a7421c0da60a224e7624ffe50e30799dfc919087b6e9787df9bd6b`.
The source review used revision 23ah before the lead issued revision 23ai.
The contracts pin remains `contracts-v0.1.27`, commit `b161db87ca82873b4f6b21d8433a69f086abb7cf`.
The reviewer used the PC brief, BUILD.md, P3's real-form mapping, and the pinned transcripts.

## Supplement to Round 1

This verdict supersedes package NOT CLEAN commit `b5ca3ab9ed81a5ba6f26965d3b374f7c44543070`.
The first package CLEAN, `deec5b5bf779b55d759e18b5264fe3ea0e6d1905`, missed the required Prior art detail.
The integration reviewer identified SI-4. All four findings are now closed.
The source review covered `fce17f78b96009cb560fb3896a6f957840ec2c43` before the base-only merge described below.

### SI-4 LOW — The Prior art note omitted required comparisons and reasons

BUILD.md rule 0 and the pair-common brief require a recorded prior-art comparison and reasons for custom infrastructure.
The original note named the testkit controls and contracts #30 only.
It omitted the pinned old Core comparison and the library choices.
Every LOW finding blocks CLEAN on this HIGH PR.

Status: CLOSED. The reviewer read the corrected PR body after message `msg_plugin-w_1791699014_d68628`.
The note compares old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
It records the rejected worker routing channel, shell cat payloads, and old bounded-wait helper.
It names the reused probe protocol, process helpers, serde_json, rustix poll, and testkit control semantics.
It explains the synchronous socket hub, shared state, request writer, report reader, and temporary accept loop.
The reviewer checked the pinned old-code references. No old test was ported.

The corrected note states that the real tier uses exported HostEdges and the payload.
It does not claim that plan 23l forbids all composition through an exported interface.
The request writer shares one Deadline across the complete line and acknowledgement wait.
The note compares that need with per-call write timeouts and the available botster_test_process helpers.
The note removes the unsupported claim that no library can bound a UnixStream write.
This correction changes the PR body only.

### Focused Mac evidence

Plan revision 23ai requires focused Mac tests when the operating system decides the portable code's behavior.
The reviewer read the retained log on `fce17f78b96009cb560fb3896a6f957840ec2c43`, macOS 27.0.1 arm64.
Log: `~/botster-sessions/gates/botster-core-pc-program-controls-mac-fce17f78-20261010-230102.log`.
The PR comment includes this log and its results.

All 15 `real::program::slow_tests` passed. The test of both harnesses' probe environments also passed.
The broader `real::` run had 30 passes and one failure, with exit code 101.
The failure was `real::slow_controls::a_recorded_process_that_ended_and_is_not_reaped_does_not_live`.
That unchanged P5 test expected Matches for a zombie, but Mac reported Absent.
The reviewer compared that test with the base. The implementer reports that the lead assigned the failure to P5.
This evidence clears the focused PC requirement. It does not establish a green result for the broader Mac run.

Real conformance had 141 passes, zero failures, and exit code 0.
The report counted 39 / 68 minimum real-passing IDs and 40 / 69 real-accepted IDs.
The pending-real report listed 64 IDs and zero passes. It supports no additional pending-ID removal.

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

The implementer then merged `v1` at `7e8e07d6221603e8cb602bb25daed4cb6dd04c17` (#241).
The resulting head is `ec317eaedbfa29467ceb68cb61b0919b717f3a3f`.
The reviewer read the retained base-merge check and the two base-path changes.
All four base-merge conditions pass. The two changed base paths do not overlap the PR's own diff.
The own diff is byte-identical: 50,444 bytes, SHA-256 `d96c1406b582875cac7ecd22ce9d105d18b64a74fa31ce72a4fe602f283cab0d`.
The reviewer independently compared those diff bytes. No reviewer delta round is required.
Log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-fce17f78-ec317eae-20261010.log`.
The PR comment records this check. The final gate must use the new head.

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
The older gate does not establish that `ec317eae` passed CI.
The integration reviewer must also report CLEAN on this head.
A later code change requires a delta review.

The reviewer ran no tests, build, gate, or mutation job. The reviewer checked logic and the supplied evidence.
This package verdict clears the reviewed logic. It does not authorize merge before the required final gate and integration CLEAN.
The verdict does not clear the 22 pending IDs or complete the PC group.

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
