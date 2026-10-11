# P5 R-68 route wait review

## Round 1 — 2026-10-11 — final READY

- PR: https://github.com/trybotster/botster-contracts/pull/47.
- Exact head: `f303ccdb9e02c6763d31cd8f7503095084630adc`.
- Tree: `c5e63ca03ffbbb2ca0c803602a34139a0458a883`.
- Base: `5ff1b52be672bc107e80cd0d911110034745eda4`.
- Parents: `2f1749ee1e89ae8e422c7dcf602e1b6efcdfc190` and `5ff1b52be672bc107e80cd0d911110034745eda4`.
- Source head before the base merge: `2f1749ee1e89ae8e422c7dcf602e1b6efcdfc190`.
- Original base: `56e5b744588b98ccbbb892d6b0a63522317284f0`.
- READY receipt: `msg_plugin-w_1791735364_165aa1`.
- Tier: HIGH. BUILD.md rules 1 and 3 apply. The route driver decides step results in a shared crate.
- Plan: revision 23bd, `stage1-plan.03c56e26.md`, SHA256 `885c9816bc027b8779db1b06488b5de05a9ad34a3cf395b2b3b6a401b6a3202e`.
- Authority: the R-68 draft, all three follow-ups, and the steward's correction of the crate-symbols requirement.

The four source findings are closed. One required evidence item is missing. This is the first formal verdict from reviewer 0167. Earlier messages were source inspections before READY.

### Closed source findings

1. **R68-F1 — MEDIUM — partial bytes did not reset the count.** The route wait now resets its count on route bytes. The change covers incomplete frames. It also resets the count when a pump reports progress. The final gate records `bytes_of_a_frame_that_is_not_complete_reset_the_no_spin_count` as PASS in 0.305 s. The pump progress proof passes in 0.605 s.
2. **R68-F2 — MEDIUM — direct frame operations bypassed the wait.** `CoreRoutes::read` now makes the wait pass. `CoreDriver` installs the wait for order constructs and codec route steps. The driver clears the wait after those operations. Frame validation, bindings, and output checks remain in `RouteDriver`. The final gate records `route_expect_on_an_idle_route_pumps_its_handle_and_waits_on_both_descriptors_until_the_step_deadline` as PASS in 0.306 s.
3. **R68-F3 — MEDIUM — the final readable wake was counted before its reads.** The driver now counts a readable wake only after the next pass makes no progress. The final gate records `the_end_of_the_stream_after_the_last_readable_wake_before_the_bound_closes_the_route_with_no_line` as PASS in 0.010 s.
4. **R68-F4 — MEDIUM — a Blocked read bypassed the pump.** Both reads now use `idle`, which matches `Empty` or `Blocked`. The path without descriptors preserves the second read when the pump makes no progress. The final gate records `a_blocked_read_is_idle_the_pass_pumps_the_handle_and_waits_on_the_descriptors` as PASS in 0.306 s. The proof covers both descriptors and no readiness descriptor.

The source uses the route's own handle and generation. The route wait calls only `pump(now)`. It does not poll events or change the clock. The driver polls the route descriptor and the wake descriptor only when both exist. The wait recomputes the remaining time after EINTR. A poll error ends the step at its limit. The driver releases the shared-state borrow before each pump. The driver clears the wait after each step or pass. It does not read or drain the wake descriptor. The refusals operation keeps its separate read path.

The no-spin proof passes in 0.022 s. The proof with no readable descriptor passes in 0.304 s. The read deadline proof passes in 0.306 s. The refusals pump proof passes in 0.011 s. These results come from the supplied final gate. I did not execute the proofs.

The crate-symbols question is closed by the steward's clarification. Runner traits are not contract text. No table change is required. The final gate records a successful crate-symbols check on the exact final head.

### Base merge and supplied gates

I read the PR body and its base-merge comment. I also checked the merge with read-only Git and Python operations.

- The six own paths and six base paths do not overlap.
- The own diff is identical before and after the merge.
- The diff SHA256 with full blob identifiers is `5d4d5078eda3c85455b2c3123e888927dc167686564e48f63477884a713718f7`. This matches the PR comment.
- The entire final tree equals the new base tree plus the own delta. I checked all 2315 tree entries.
- `git diff --check` passed on the source head. I ran no executable merge-check job.

The source-head gate is `gates/botster-contracts-p5-r68-route-pump-2f1749ee-pool-20261011-083201-66254.log`.
Its SHA256 is `879d66107ea5203321b06bc01dd962cc3b731f5654cccff77ea7b7342902953a`.
Its header names the source head and CI base `9d19b997fd044ca76c0844f8a32fa79d930e5ec4`.
All ten steps pass. The default suite has 2085 passes and zero skips in 7.007 s.
The slow suite has 33 passes and zero skips in 0.505 s.
The mutation summary has 65 tested, 53 caught, and 12 unviable mutants. It reports no missed mutant or timeout.
The job and wrapper exit with 0 after 640 s.

The final gate is `gates/botster-contracts-p5-r68-route-pump-f303ccdb-pool-20261011-085500-41595.log`.
Its SHA256 is `b364ad8067a18e0ed399bfdb13411e480a1fd3960037d19a09ce7c7ce2c0ade2`.
Its head and CI base match this verdict.
All ten steps pass, with a total of 624.6 s.
The default suite has 2085 passes and zero skips in 8.776 s.
The slow suite has 33 passes and zero skips in 0.505 s.
The mutation summary has 65 tested, 53 caught, and 12 unviable mutants. It reports no missed mutant or timeout.
The baseline takes 63 s to build and 3 s to test. The mutation timeout is 20 s.
The job and wrapper exit with 0 after 633 s on msa1.
Crate-symbols checks 55 tables and 19267 symbols. No base file lacks a table. The 51 UI gaps remain.
I checked the headers, test records, check summaries, and final summaries. I did not inspect raw mutant identities or individual catcher logs.

The clock test change adds a fake-tier check before the existing real-clock check. The existing real-clock assertion remains. No timeout or mutation exclusion changes. The final gate records the test as PASS in 0.327 s.

### Supplied Core evidence

I read `nrun-job.sh`, `nrun-table.py`, `a54-job.sh`, and `a54-table.py` in `shared/core-stage1/evidence/p5-r68`.
I checked the raw logs and both full tables. I ran only read-only artifact parsing.

The before Core head is `f04f7c553f43c449b77c07dd8cc4122cb9555803`.
The after Core head is `516c1dbb4a0095caf5cb1b77ca6ddd884f4c8dff`.
The scratch delta changes the contracts pin, the route readiness adapter, the wake descriptor adapter, and descriptor tests.
I read the full six-file scratch delta. The scratch branch is not a product merge candidate in this review.

Each N-run log contains 100 result records: ten IDs, two tiers, and five indexed records per group.
Each group has indexes 1 through 5, with no duplicate index.
All ten testkit groups pass 5 of 5 in all four jobs.
The real-tier A20-1 ID is reported as testkit-proven and does not run on the real tier.

On Linux, the last-frame `ou_7` ID and the healthy-close `ou_2b` ID change from 0 of 5 to 5 of 5.
The missing-op `a2_2` ID changes from 0 of 5 to 5 of 5 on both operating systems.
The other real-tier passing groups stay at 5 of 5.
The lost-session ID still fails at step 29. The write-failed ID still lacks `fail_writes`.
No passing group becomes non-passing.
The Linux before and after jobs use different kernels, as the body states.

The A5-4 logs use the same Linux node and kernel. Their concatenation exactly equals `a54-linux-both.log`.
Both supplied tables exactly match the supplied parser on those raw logs.
The real table has 599 IDs. It has 179 must-pass passes before and after, and five then six pending-real passes.
Only the missing-op `a2_2` ID changes, from INCONCLUSIVE at step 18 to PASS.
The testkit table has 599 IDs, 209 must-pass passes before and after, and no change.
No table row changes from PASS to non-PASS.

I also checked all 690 raw must-run records per tier and tree. Their ID sets and statuses remain equal.
The tables omit 70 IDs without transcripts, 18 withdrawn IDs, two deferred IDs, and one tier-specific exclusion.
The real table excludes the A20-1 testkit-proven ID. The testkit table excludes the real-only reused-PID ID.
No omitted record passes. These exclusions remain unchanged.

Artifact SHA256 values:

| Artifact | SHA256 |
|---|---|
| `nrun-linux-before-f04f7c55.log` | `6af56b5484d41d2abba4de30b87bfc18e5dfbffd3329c1996f7ad2dd6af18be5` |
| `nrun-linux-after-516c1dbb.log` | `c6704c92be07ae7243d10ac9b54fe105637b3fe0fd277ddcb40b5908d2c0ae0f` |
| `nrun-mac-before-f04f7c55.log` | `a7495b92d797d7f7dd5d6b2cd5824bb85f09bb8abc7d254d40d9680fb47eba55` |
| `nrun-mac-after-516c1dbb.log` | `db98eb242843eae71a886bb2dfd101b60b500813b3ef3e0c5fc6afb9c4e88357` |
| `a54-linux-before-f04f7c55.log` | `51f620a08da5d7b281a8f6c502a47a8dc5a7d2510bab9c6b848135b0ec3285a3` |
| `a54-linux-after-516c1dbb.log` | `fa34943b1adc3c55f412ce9a2d92799e6b3f9279b682be7c424be6e20893ef85` |
| `a54-linux-both.log` | `39140253f8476eeed25ba6b316a02c27e7055a636fca49ff3988a364a2bb4771` |
| `a54-real-table.md` | `7fe647ddd080410521b482c268f6a79f635b56c970f1d0be48ad80ebe380b58f` |
| `a54-testkit-table.md` | `3fcf8bd04c7b7501d3ddf8537b9c3a625a2e007dd4a33feda6c0ffa6e4f77832` |

### Open finding

Reviewer 016c's local verdict is `.review-verdicts/r68-route-pump-f303ccdb.md` in the contracts reviewer worktree.
Its SHA256 is `9ce02db055387618d9cc544523b9a22673ba2931d48b9f77b44c89d683cb7aa9`.
I read the entire verdict. It names this exact head and reports NOT CLEAN for the same missing proof.
Reviewer 016c has no verdict commit because that reviewer's instructions prohibit commits.

**R68-E1 — MEDIUM — the required deterministic real-tier late-RouteReceived proof is missing.**

R-68 Evidence item 1 and plan 23bd require a deterministic real-tier case in which RouteReceived arrives after the last pump.
The proof must reach the step limit without the fix and reach EOF with the fix.

`nrun-job.sh` repeats existing transcripts without a control of the acknowledgement order.
The scratch delta adds descriptor tests, but no test forces that order.
The PR body states that the old driver passes the two `ou_*` cases in some runs.
The Linux N-run and full-suite logs also show different old-driver results for those cases.
Thus, these logs show the timing-dependent failure and satisfy the N-run requirement. They do not supply the separately required deterministic proof.

Required change: provide a real-tier proof that forces the late acknowledgement order, with red-on-revert evidence.
Name the exact Core scratch head and contracts head. Update the PR body with the proof and its log.
This finding matches reviewer 016c's evidence finding.

I changed no product code. I ran no tests, builds, gates, or mutation jobs.

VERDICT: NOT CLEAN
