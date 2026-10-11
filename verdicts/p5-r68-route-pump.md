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

## Round 2 — 2026-10-11 — controlled late acknowledgement evidence

- PR: https://github.com/trybotster/botster-contracts/pull/47.
- Exact head: `f303ccdb9e02c6763d31cd8f7503095084630adc`.
- Tree: `c5e63ca03ffbbb2ca0c803602a34139a0458a883`.
- Base: `5ff1b52be672bc107e80cd0d911110034745eda4`.
- Parents: `2f1749ee1e89ae8e422c7dcf602e1b6efcdfc190` and `5ff1b52be672bc107e80cd0d911110034745eda4`.
- Evidence receipt: `msg_plugin-w_1791737368_b336d2`.
- Body correction receipt: `msg_plugin-w_1791737695_aa3851`.
- Tier: HIGH. BUILD.md rules 1 and 3 apply.
- Plan: revision 23bk, `stage1-plan.00635d10.md`, SHA256 `d90aab57ce819b2607339cfe0c92a1d5b60987db52d3b111ce1f223af6354bf6`.
- Authority: the lead approved the scratch seam. The stall rule places #47 in contracts-v0.1.39. The lead requires no source change, base merge, or new gate.

This section supersedes Round 1 on the same head. All four source findings remain closed. R68-E1 now closes.

### Source and proof checks

I read the entire README, both seam patches, job script, four late logs, and published PR body.
Both retained patches exactly equal their committed Git records. Their 131 added lines are identical.
Each commit changes only the scratch harness files `crash.rs` and `real.rs`.

The before Core head is `59d32bba6c6076dbf295cc5c83cc27cd0627efb3`.
Its tree is `85bdfae469b8d111a1f58a01bb3814224492469d`.
Its parent is `f04f7c553f43c449b77c07dd8cc4122cb9555803`.
Cargo.lock resolves its scratch tag to contracts `56e5b744588b98ccbbb892d6b0a63522317284f0`.

The after Core head is `5e0ea617a6b1371fa3007791e37ea4fc64356d55`.
Its tree is `a0d81e38a52bc1167f6f45f67e67b4e391ee7734`.
Its parent is `516c1dbb4a0095caf5cb1b77ca6ddd884f4c8dff`.
Cargo.lock resolves its scratch tag to contracts `2f1749ee1e89ae8e422c7dcf602e1b6efcdfc190`.

The worker source tree is identical on both sides.
The earlier reviewed Core delta supplies the descriptor adapters on the after side.
The driver, harness, clock test, and driver tests at the after contracts pin equal those files at the final head.

The proof and both controls are identical on both sides.
The proof starts a probe and waits for `program_ready`.
It pauses the recorded worker before it attaches r1 and begins Stop.
The controls check the recorded worker's pid and start time. They signal one pid, never a process group.
The harness runs the one-shot resume step after the next Core pump returns and releases the slot lock.
The worker can then acknowledge the route and close its copy.
The old driver pumps once for handoff. It cannot process the later acknowledgement without another pump.
The new driver supplies that pump during the route wait.

The proof checks `expect_closed null` on r1 and ignores all frames.
Thus, a PASS requires EOF (the end of the stream).
Both sides use seed 0 and `Limits::real()`. The ten-second step limit remains unchanged.
The seam adds no sleep. The scratch seam is evidence only.

### Execution evidence

Each command in the four logs matches `late-job.sh` after substitution of the before or after tree value.
Each log names its exact scratch head and reports successful creation of the worker manifest.
Each log has five outcome records and five test-result records, with indexes 1 through 5 and no duplicate index.

| Operating system | Before | After |
|---|---|---|
| Linux 6.12.111+deb13-amd64 | 5 of 5 Inconclusive at step 8, step_timeout, seed 0; 10.01 to 10.02 s | 5 of 5 Passed at EOF; 21 to 24 ms |
| macOS, Darwin 27.0.0 | 5 of 5 Inconclusive at step 8, step_timeout, seed 0; 10.34 to 10.76 s | 5 of 5 Passed at EOF; 357 to 679 ms |

The before test asserts Passed and therefore reports FAILED for the required timeout outcomes.
The after test reports one pass and zero failures in each run.
The script filters output and does not propagate the cargo exit through its pipeline.
A zero wrapper exit alone does not prove PASS. The retained outcome and test-result records supply the proof.
The four wrappers exit with zero after 106, 142, 877, and 213 seconds, respectively.

The published body now states the correct macOS before range.
This closes the LOW body correction. The final head remains unchanged.
The previously reviewed exact-head gate remains green.
The N-run and full A5-4 evidence retains the scope recorded in Round 1. No PASS becomes non-PASS.

Artifact SHA256 values:

| Artifact | SHA256 |
|---|---|
| `late-README.md` | `46d795e0b89cfdfc9e278578614dafbff4411f0270cde87e6aca86b7c6179266` |
| `late-job.sh` | `1556b3b7f943de6450f06bbf2778bf5cbf63d6d67a3acdd9abc6d78791c1bb94` |
| `late-seam-before-59d32bba.patch` | `4e5c9b4f44ee8e9506bcb0ffb138b9d942763d332de34173894c71ef6b9e94b7` |
| `late-seam-after-5e0ea617.patch` | `a5de10020b1e6a99e607a0f06f13dbb35a777f659ad9cbb35e7234dec06c6b7d` |
| `late-linux-before-59d32bba.log` | `534e078596c6295aaad1365fc2d6efa560882e1147993b22ee8bd25d9f881745` |
| `late-linux-after-5e0ea617.log` | `2f1aa6221ec94354ba5c24df389026921a7bf35a665f3058a2761f92b59847e1` |
| `late-mac-before-59d32bba.log` | `607ada74b40f9d93f45b492e22e45f65b15880cbaaf1e7de0da4574b4f17722d` |
| `late-mac-after-5e0ea617.log` | `c7bb7e895fb2371b1d082f3f11f9d4a36b748685fb0730c2c24e4c2d7691a44f` |

### Required reviewer records

Reviewer 016c accepts this exact head and closes R68-E1.
The local record is `r68-route-pump-f303ccdb-accept.md`, SHA256 `6db224a04b29c944db455e97b51da61a850b31441f138b075cc32e5fdfe4dbe7`.
Receipt: `msg_plugin-w_1791737474_ddb3df`.
Reviewer 016c permitted me to preserve the entire record. The companion verdict file records its authorship.
Reviewer 016c did not make this commit.

Sol is CLEAN on this exact head in `85f9206ba5c9265917a4d3a55a5412322f8ba6d6`, Round 2.
Receipt: `msg_plugin-w_1791737633_f6e7c5`.
I read that committed section.

No finding remains open.
This verdict accepts only the named contracts head and supplied evidence.
It does not accept a future tag, Core pin, or pending-ID removal.
I changed no product code. I ran no builds, tests, gates, mutation jobs, or executable merge checks.

VERDICT: CLEAN
