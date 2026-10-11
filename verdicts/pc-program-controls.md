# PC package delta review: PR #233

VERDICT: CLEAN

- Current head: `a596c945111ae5df7db757e53ef945d238873c75`.
- Current base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723` (`v1`).
- Source-reviewed head: `4d05d270d6c6cebcdae247050cd702b6a157c639`.
- Source-reviewed base: `554d43e890b391cefc9da205c71db6856accb21d`.
- Previous reviewed head: `8cfdd6715c6a69c419d9819636e185bfff74bb24`.
- Risk tier: HIGH. This delta belongs to the shared testkit PR.
- Open findings: 0.
- Binding plan: revision 23al, `stage1-plan.1555a98b.md`.
- Plan SHA-256: `a3965b731b7b00fbe56e28581d298f38fe4d1b2836ff322f33d4c988cc5183ab`.
- Contracts pin: contracts-v0.1.29, `65f76ab9829d7a0fda2835c10abc7ffae2c35215`, inherited from #245.

The final gate failed after package and integration CLEAN on the previous head.
This verdict supersedes package CLEAN commit `3ebf5746ef1876ce00e86e2bee5bd7a86a318621` for the current PR.
SI-1 through SI-5 are closed. The previous review below remains the source record for unchanged code.

## Completed gate and two pending-ID removals

Request: `msg_plugin-w_1791710640_8d164a`.
This verdict supersedes NOT CLEAN commit `dfead09daa9a2393549cf0ebfe79223a8caaa140`.
The reviewer read the complete two-line removal diff and its commit record.
The local branch and its tracking ref name the current head.
The GitHub API reads failed. The reviewer instead checked both remote branch refs with `git ls-remote`.
The program-controls branch names a596c945. The remote v1 branch names the stated base, 8dab35d1.

The only delta from 6dd3caef is `conformance/core-real-pending.txt`.
The file equals the parent file with exactly these two lines removed:

- `conf::ev_2_one_marker_per_session`.
- `conf::in_5_lane_bounds_refuse_at_begin`.

The delta changes no code, transcript, timeout, pin, exclusion, or other status file.
The completed gate on the parent supplies both real PASS records, under contracts-v0.1.29's R-56 and R-57 fences.
These results meet the lead's required Linux after-pin condition. The removals are valid within this PR.
The PC group now has 20 pending IDs. All six steward-held IDs and the four remaining minimum PC IDs stay pending.
No additional source finding exists. The group is not complete.

Gate log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-6dd3caef-pool-20261011-020628-1436.log`.
Gate head: `6dd3caef6ac68eb77bd872e54c9c5ff7ebe003fb`.
Gate base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
The reviewer read the header, suite summaries, pending-real report, mutation records, final summary, and exit result.
All ten steps pass. The job exits 0 after 1015 seconds on Linux node msa1.
Default tests: 1614 pass, 484 skip. Slow tests: 461 pass, 1981 skip.
The pending-real report has 61 IDs and exactly two passes, EV-2 and IN-5, at lines 4461 and 4462.
The gate reports minimum real-passing 39 / 68 and real-accepted 40 / 69.
That raw tally does not override R-59: the unfenced AD-7 pass does not count toward the accepted minimum progress.
The fuzz step passes because the diff changes no crate with a decoder harness. It runs no decoder fuzz campaign.

### SI-5 closure

The unchanged closed-peer proof passes on Linux in 0.002 seconds.
The focused Mac normal run and selected guard reversal remain accepted for the unchanged program module.
The reviewer read all 79 stage-2 mutation outcomes and checked the identity multisets from both stages.
Both stages select the same 79 unique mutant identities. Stage 1 misses all 79; stage 2 catches 59 and has 20 unviable.
No mutant survives stage 2. No mutant times out.
The previously missed guard-to-true mutant at `real/program.rs:269:20` is explicitly caught.
The final mutation summary is 59 caught, zero missed, zero timeout, and 20 unviable.
This closes SI-5. All five findings are closed.

The completed green gate covers 6dd3caef, before the two removals. It is not CI evidence on a596c945.
Integration CLEAN and the final Linux gate on the current exact head remain required before merge.
The implementer will run that gate after both reviewers report CLEAN.
The reviewer runs no build, test, gate, mutation, reversal, or executable merge-check job.
This verdict grants no merge approval by itself. Wait for the final gate doorbell. Never poll.

## Earlier disjoint base merge

Message: `msg_plugin-w_1791709587_77d7ec`.
The current head only merges the base's #252 change to `xtask/src/ci.rs`.
The reviewer read the completed base-merge check and confirmed the PR head and base.
Log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-4d05d270-6dd3caef-20261011.log`.
Every condition passes. The own diff outside line-set files is unchanged: 48483 bytes.
SHA-256: `74399e1ff9c3d0bdb8ae1bb3fc1606758efa09e3d7666243aae1790547aa38c3`.
The reviewer independently confirmed that diff and the new base's ancestry.
The base leaves both pending files unchanged. The head retains the source-reviewed pending files.
No source delta round is needed under the disjoint-base rule. This carries NOT CLEAN, not CLEAN.
SI-5 remains open. The completed final gate on the exact current head must close it.
The implementer reports cancellation of the stale-base gate and a new gate running on the current head.
The reviewer did not read or poll either running job.

## Earlier base-merge delta

Delta message: `msg_plugin-w_1791708757_0cc48c`.
The reviewer confirmed the PR head and base. The reviewer read the complete source delta and the combined conflict diff.
The first merge is `3a71241d53c2613a978490d2df4cabea55b27487`, with base `b7c97b42b8e559053153f8a8202ee32dbde0a93a`.
The second merge is the reviewed head, with base `554d43e890b391cefc9da205c71db6856accb21d`.

The base-merge check fails, so neither review carries automatically.
Log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-8cfdd671-4d05d270-20261011.log`.
The reviewer read the whole completed check log.
Conditions 1, 2, and 3 fail on the shared paths. The real-pending line-set check also rejects the changed group comment.
The reviewer checked both resolutions directly:

- `core-real-pending.txt` equals the current base file, with exactly the previous 16 PC IDs removed.
  Both sides add no ID. The base removes three other IDs. The merge preserves those removals and all base comments.
- `real.rs` keeps both `program::CONTROLS` and `route::CONTROLS` in the tapped control set.
  Its tests check both sets. The removed negative assertion for `pty_output` conflicts with the new program control.
  The negative assertion for `pty_fail_after` stays. The plain harness still serves no controls.
- The merged `open` keeps the base's shared `heard` record and the program observer's wake registration.
  The PR preserves the base's route attachment implementation and payload identity checks.

The program module, testkit manifest, and process-check exception are byte-identical to the previous reviewed head.
The current `Cargo.toml` and `Cargo.lock` equal the new base. This PR adds no pin change of its own.
The core-pending, held, deferred, ledger, and real-only files equal the new base.
The own diff still has five files. The reviewer found no additional source issue.
The six steward-held PC IDs remain pending. EV-2 and IN-5 also remain pending at this head.
Their new fences come from #245. Each removal still needs its real PASS result and a review of the resulting commit.
R-59's unfenced AD-7 pass does not count toward the real minimum.

The implementer reports 351 passing testkit library tests on 3a71241d. No retained log was supplied for that focused run.
The implementer reports a final gate running on 4d05d270. The reviewer did not read or poll that running job.
SI-5 remains the only open finding. The reviewer waits for the completed gate evidence.

## SI-5 MEDIUM — The socket error guard lacks a reliable mutation proof

Evidence: the final Linux gate missed one mutant in stage 2.
The mutant replaces the WouldBlock or Interrupted match guard in `send_line` with true, at real/program.rs:269.
That change retries permanent socket errors until the cleanup deadline instead of returning the socket error.
The previous tests did not reliably reject that behavior.
Plan section 8 treats a missed mutant as a review finding.

Gate head: `ec317eaedbfa29467ceb68cb61b0919b717f3a3f`.
Gate base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-ec317eae-pool-20261010-231846-63449.log`.
The reviewer read the header, missed-mutant record, mutation result, and final summary.
Stage 2 reports 58 caught, 1 missed, 0 timeout, and 20 unviable among 79 mutants.
Eight earlier steps passed. Mutation failed. Fuzz did not run. The gate exited with code 1.

Required change: supply a behavior test that rejects retries of a permanent socket error.
Supply a passing normal run and evidence that the missed mutant fails that test.
The final Linux gate must pass on the corrected head. Plan 23ai also requires focused Mac evidence for this socket test.

Status: CLOSED by the completed 6dd3caef gate and mutation evidence above. The source correction is unchanged.
The delta adds one 17-line test, `a_request_write_to_a_closed_probe_fails_with_its_error_not_at_the_bound`.
It creates a socket pair, makes the harness end non-blocking, and drops the peer.
It requires BrokenPipe or ConnectionReset from `send_line`. TimedOut fails that assertion.
The test uses the existing cleanup deadline. It changes no production code, transcript, timeout value, list, pin, or mutation exclusion.
The reviewer found no additional source issue in this delta.

The reviewer read the selected reversal record.
Log: `~/botster-sessions/gates/botster-core-pc-send-line-guard-reversal-20261011-001126.log`.
With the missed mutant, the test fails with TimedOut after 10 seconds.
The first reversal record contains no passing normal run. Later retained evidence supplies both Mac results on 8cfdd671:

- Normal run: `~/botster-sessions/gates/botster-core-pc-send-line-test-mac-8cfdd671-20261011-001400.log`.
  All 16 program tests pass; exit 0.
- Reversal: `~/botster-sessions/gates/botster-core-pc-send-line-guard-reversal-8cfdd671-20261011-001419.log`.
  The guard replaced with true gives TimedOut. The added test fails after 10 seconds; exit 101.

The reviewer read both completed records. The unchanged program module preserves that focused Mac proof.
The corrected Linux gate on 8cfdd671 stopped at an AD-7 failure, before mutation and fuzz.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-8cfdd671-pool-20261011-001231-18048.log`.
It does not close SI-5. The new base contains #249's payload identity correction.

The reviewer ran no build, test, gate, mutation, reversal, or executable merge-check job.
The verdict grants no merge approval. The earlier head retained 22 pending PC IDs; the current head retains 20.
Next: wait for the final Linux gate on a596c945. Never poll.

---

# Earlier PC package review: PR #233

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
