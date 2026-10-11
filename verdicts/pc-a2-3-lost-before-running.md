# PC package review: a2_3 before Running, PR #277

VERDICT: CLEAN (0 open).

Latest review head: `2c78bb6c8424d37089062437731980ec0d34eab0`.
Round 1 below covers the original source head. Round 2 covers the imported base.

- READY: `msg_plugin-w_1791737722_0fd975`.
- Head: `7d7d429f06335c18e8a21e6c63085be4c5465eb0`.
- Own-diff base, PR base, and gate header base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
- Head parent: `f46a1fff431f30851d642447ebc32b3e5ef5425d`.
- Tier: HIGH. The change touches the start flow and a pending list.
- Contracts pin: `contracts-v0.1.36`, unchanged.
- Binding plan: 23bo, `~/botster-sessions/pins/stage1-plan.5c247504.md`.
- Plan SHA-256: `0c4aed22c9810d623b2b75e0da9073aff4d0d3d742db3db315f659d39d9caa7c`.

The reviewer read the PC brief first and followed its reading order.
The reviewer read BUILD.md, the applicable contracts and rulings, R-72, the design note, and the complete eight-file own diff.
The reviewer checked the current PR body, the pinned a2_3 transcript, the startup paths, adoption guards, and route cleanup.
The reviewer applies the previously read orchestrate-delivery skill to R-72's change in the classification premise.
This record uses the existing PC handoff. No new agent or workflow is required.

## Source conclusion

R-72 requires Core to classify what failed. It must not classify a worker death by the order of two edge reports.
The new `StartPhase::LinkLost` records the link end of a worker that this host spawned before Running.
Core keeps the session Starting and preserves the existing startup deadline. The helper posts no event.
`flow_waiting` excludes this phase from runnable session work. `run_start` performs no step in this phase.
The worker exit report still enters the existing `on_process_exited` path.
That path completes the start with `StartFailed(WorkerFailed)` and `Lost(WorkerGone)`.

If the worker stays alive until the deadline, `startup_expired` selects the existing link-failure outcome.
It gives `StartFailed(WorkerFailed)` and `Exited{code: None, signal: None, cause: Other}`.
It clears the deadline through `fail_start`. It does not signal the worker or use `StartupTimeout`.
The PR body states this reason before and after. Only this case's completion time changes.
The general startup-timeout path keeps its existing signal and `StartupTimeout` outcome.

`on_link_closed` uses the wait for `AwaitHello`, `SendLaunch`, and `AwaitLaunched`.
Its preceding adoption arm retains `Lost(WorkerUnreachable)` for adopted starts.
An adopted start enters `SendLaunch` with its existing link. A link end in that phase enters the adoption arm first.
Running link loss stays outside this change. The helper's debug assertion checks the spawned-start invariant and existing deadline.

`SendLaunch` also uses the wait when `send_msg` finds no link.
That function returns false only when the session has no link.
This covers an early hello followed by link EOF during the identity-row write.
The row result then advances to `SendLaunch`, with the spawned identity and original startup deadline.
Core sends no launch on that missing link. This is the same failure boundary, within the approved spawned-start scope.

Routes attached during the wait remain in the session and its pending handoffs.
The unchanged failed-start path posts the end state before it starts route cleanup.
It captures all attached routes in `bound_at_end` and uses the existing close path.
Lost closes routes once with `SessionLost`. Exited closes the held routes with `SessionEnded{exit}`.
The first-reason rule and route transport cleanup stay unchanged.

The new report-order test checks both EOF-first and exit-first cases with an attached route.
It checks no event at EOF, Starting during the wait, one Lost state, and one SessionLost close after that state.
The deadline test checks the unchanged reason and Exited value, with no signal.
The identity-row test checks the additional missing-link path. The existing pressure test preserves an already chosen Exited failure.
The reviewer traced each stated revert to the corresponding failing assertion. The reviewer executed no revert or test.

Only `conf::a2_3_lost_session_closes_routes_session_lost` leaves `core-real-pending.txt`.
The testkit already runs this ID must-pass. The real tier now does so on the supplied exact-head PASS.
No contract, transcript, timeout value, pin, worker, unsafe code, or mutation profile changes.
The Prior art note rejects the old immediate-EOF behavior under R-50 and R-72.
The reviewer read that old implementation and its named test at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The PR reuses no old mechanism and adds no hand-written substitute for a component.

The body lists six other classification places. The reviewer checked their cited source paths.
The body distinguishes races, a case that is not a race today, and the related adoption-connect case.
This PR changes none of those places. Plan 23bl leaves their later triage to the lead after #277 merges.
This CLEAN closes no separate adoption, Running-worker, stop, or route-input gap.

## Evidence

The reviewer read PC's retained macOS logs in `shared/core-stage1/evidence/pc-a2-3/`.
The fixed a2_3 trial passes six times, in 1.71 to 2.06 seconds.
The immediate-failure control fails three times at seed 0, step 29, with SessionEnded before the required Lost state.

The reviewer read the A5-4 script, all four tables, and the result lines and summaries in all four raw logs.
Each table and raw log contains 690 unique transcript results. Each raw result agrees with its table.
Testkit before and after: 209 PASS, zero FAIL, 481 ignored. The tables are byte-identical.
Real before: 179 PASS, one FAIL, 510 ignored. Real after: 180 PASS, zero FAIL, 510 ignored.
The only status change is a2_3 from FAIL to PASS. No PASS becomes non-PASS.
The comparison uses the same lists and worker, with the base engine as PC states in the PR body.

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-a2-3-lost-before-running-7d7d429f-pool-20261011-094729-72318.log`.
SHA-256: `d034fd8d28a3c205f3ad496eb0c25318ee8641d0c670b40f1712d08ab7334548`.
The header names the exact head and base above. All ten steps PASS in 316.6 seconds.
The Linux job exits 0 after 433 seconds. The wrapper exits 0 after 434 seconds.
Default: 1727 PASS, 481 skipped, 12.342 seconds. Slow: 554 PASS, 2021 skipped, 42.168 seconds.
Testkit a2_3 passes at line 557 in 0.231 seconds. Real a2_3 passes at line 4042 in 0.207 seconds.
All four relevant engine tests pass. The default test budget passes.
The minimum counts are testkit 56/69, real-passing 47/68, and real-accepted 48/69.
This branch does not yet contain #276's additional E1-1 proof.

Stage 1 tests six mutants and catches all six. Stage 2 does not run because stage 1 misses none.
The reviewer matched all six raw mutant identities with their six named catcher records.
The result has zero missed, zero timeout, zero unviable, and zero FLAKY outcomes.
Fuzz reports no changed crate with a decoder harness. The mutation profile is unchanged.
The reviewer ran no build, test, gate, mutation, reproduction, or executable merge-check job.

## Verdict scope

Package verdict: CLEAN (0 open) on the exact head above. No package finding remains open.
Sol's same-head integration verdict remains required.
After both CLEANs, PC must merge current v1 once, post the base-merge check, and run one final exact-head gate.
The body notes a newer v1 and an overlapping pending-list path. This verdict approves no later source delta or conflict resolution.
The original program-controls group remains incomplete. Wait for the doorbell. Never poll.

## Sol source CLEAN receipt

Receipt: `msg_plugin-w_1791741176_90ec42`.
Sol verdict: `2587b8498e51f36e034c8655ae2013be35e08ff1`, `verdicts/stage1/pc-a2-3-lost-before-running.md`, Round 1.
The reviewer read the complete verdict and verified that `origin/stage1/integration-review-sol` contains its commit.
Sol reports source CLEAN with zero open findings on `7d7d429f06335c18e8a21e6c63085be4c5465eb0`.
Both source CLEANs cover that exact head and base `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Sol retains the evidence limits: the compact logs do not independently prove the scratch checkout's provenance.
The separate engine reversals are PC's body reports. Neither reviewer executed or independently reproduced those reversals.
The same-head integration-review condition is closed. The current-v1 import, applicable base check, and final exact-head gate remain due.
Neither verdict approves a later source delta or gate. No execution job ran here. Wait for the doorbell. Never poll.


## Round 2: imported-base source and final gate

- Superseding READY receipt: `msg_plugin-w_1791741344_7d121d`.
- Final gate receipt: `msg_plugin-w_1791741557_e14e9d`.
- Reviewed head: `2c78bb6c8424d37089062437731980ec0d34eab0`.
- Own-diff base, PR base, and gate header base: `20c4e98df6b47a45f3e2284a976e1c8a1341457e`.
- Parents: `79277df141aa6c5b18f150ed47015bd733b5efc1` and `20c4e98df6b47a45f3e2284a976e1c8a1341457e`.
- Tree: `fed65ce895b71a25f22d1c9ac82d3344e9952580`.
- Binding plan: 23bu, `~/botster-sessions/pins/stage1-plan.133ea0b2.md`.
- Plan SHA-256: `f8f6e1fe17a4e7e2bda32b9b98f9326c8ea912ee009770684df2a66e6aace815`.

The reviewer read the PC brief first for this delta. The earlier BUILD.md and contract reads remain applicable.
The reviewer read pair-common, the 23bt-to-23bu plan changes, the base-check report, and the lead's handoff.
Plan 23bu records the final-gate slot and the lead's second-import decision for this PR.
Its contracts `.40` work does not change this PR's contracts pin or R-72 scope.

### Actual import history

The first import joined original head `7d7d429f06335c18e8a21e6c63085be4c5465eb0` with v1 `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
It produced `79277df141aa6c5b18f150ed47015bd733b5efc1`.
PR #276 then moved v1 to `20c4e98df6b47a45f3e2284a976e1c8a1341457e`.
The lead ordered PC to stop the stale-base gate and import that tip.
The second import produced the reviewed head above. The history contains two imports.
The stopped `79277df1` job supplies no completed gate evidence.
The lead's 03:57 and 04:05 handoff entries record the exception and assign #277 the final-gate slot.

### Failed base check and independent source comparison

Report: `~/botster-sessions/shared/core-stage1/evidence/pc-a2-3/base-merge-check-2c78bb6c.out`.
SHA-256: `fcf69ab5cd1353ea0fde08595497f07ef002dc3e3fa598812e74e85031ec086a`.
The report passes ancestry and checks (1), (2), and (3). It passes (4) for `core-pending.txt`.
It fails (4) for `core-real-pending.txt`: the base does not only remove ID lines.
This FAIL requires both source delta reviews. It does not permit automatic carry-over of the original CLEANs.

The reviewer independently compares the two own diffs with the check's canonical `git diff-tree` options.
Outside the two line-set files, the diffs are byte-identical: 15231 bytes.
SHA-256: `f330332690061aad54448cce4aabee47a9d401e0830ee34efc6b80a7b17f1819`.
The eight own paths are unchanged. All seven source and test blobs equal the original reviewed blobs.
None of the 19 changed base paths overlaps an own path outside the two line-set files.
Every changed base path outside the own diff equals its imported-base blob in the reviewed head.
`core-pending.txt` equals the imported base exactly.
`core-real-pending.txt` equals the imported base minus only `conf::a2_3_lost_session_closes_routes_session_lost`.

The base adds `tm_6_begin_and_attach_signal_wake`, `ou_9_attach_is_sync_baseline_in_pump`, and `st_5_reads_after_exit`.
It also adds their comments and a blank line. The imported head preserves those additions.
These additions explain why the base is not a pure removal under check (4).
The reviewer reads the current start, link-end, and worker-exit paths again.
The unchanged R-72 logic preserves the original worker-exit classification, startup deadline, adoption guard, and route-close order.
No own change alters a contract, transcript, timeout value, pin, worker, mutation profile, or unsafe code.
Source conclusion: CLEAN on the exact imported head. This is an explicit delta review after the check's FAIL.
The Round 1 A5-4 provenance and engine-reversal evidence limits remain in force.

### Final gate evidence

Log: `~/botster-sessions/gates/botster-core-stage1-pc-a2-3-lost-before-running-2c78bb6c-pool-20261011-105545-34013.log`.
SHA-256: `00ac6621cdd278fa7836f913156836da5423592010e4693ae257231a2c44c904`.
The header names the reviewed head and imported base above. All ten steps PASS in 136.3 seconds.
The Linux job and wrapper exit 0 after 144 seconds on msa1.
Default: 1740 PASS, 478 skipped, 11.580 seconds. Slow: 558 PASS, 2027 skipped, 41.432 seconds.
Testkit a2_3 passes at line 281 in 0.209 seconds. Real a2_3 passes at line 3783 in 0.161 seconds.
All four relevant engine tests pass. Counts: testkit 60/69; real-passing 49/68; real-accepted 50/69.
The imported #276 named E1-1 proof and both LC-2 proofs also pass in this same gate.

Stage 1 catches all six mutants in 10.8 seconds. Stage 2 does not run because stage 1 misses none.
The reviewer matches all six raw mutant identities with the six named catcher records.
There are zero missed, timeout, unviable, or FLAKY outcomes.
The mutant identities equal those from the original gate. Two named catcher records differ between gates.
The fuzz step is PASS. It reports no changed crate with a decoder harness.
The reviewer runs no build, test, gate, mutation, reproduction, or executable base-check job.

The log reports three passing IDs that remain real-pending: `dp_7_revoked_reason_reaches_client_and_host`,
`ou_2b_handoff_failure_is_reported_as_handoff_failed`, and `ou_7_exit_after_tail_queued_then_close`.
The lead's 04:15 and 04:17 handoff entries assign their removal and repeated evidence to P5's pin PR.
This PR removes none of those IDs. This verdict does not close that separate pending-list work.

### LOW body finding and final conditions

LOW PC-277-BODY-1 is closed. The final-gate body says the same mutants were caught by the same tests.
The final gate names different catchers for `on_link_closed` and `flow_waiting -> true`.
The former is caught by `tests::adoption::a_lost_row_with_no_valid_token_is_registry_corrupt` in the final gate.
The latter is caught by `tests::admission::a_policy_that_needs_a_missing_feature_is_unsupported` in the final gate.
Required change: remove the same-tests claim or state that two named catchers differ.
Correction request: `msg_plugin-w_1791741667_241f45`.
Correction receipt: `msg_plugin-w_1791741692_a9715e`.
The reviewer verified the corrected body through the GitHub API on the unchanged reviewed head.
The body now names the two different catchers and retains the other four unchanged catchers.
No source change or new gate is required.

The reviewer checked the current PR body through the GitHub API.
The corrected body records the original CLEANs, both imports, stopped job, failed check, and final gate.
Package verdict: CLEAN (0 open) on `2c78bb6c8424d37089062437731980ec0d34eab0`.
Sol's same-head delta CLEAN remains required. PC must add both delta verdict receipts to the body before merge.
The final gate base must still equal the v1 tip at merge, under Q7 and plan 23bu.
The original program-controls group remains incomplete. Wait for the doorbell. Never poll.


## Sol Round 2 CLEAN receipt

Receipt: `msg_plugin-w_1791741951_bbd121`.
Sol verdict: `64b9e757bc39334ce2c672fdbde5d262e5e031be`, `verdicts/stage1/pc-a2-3-lost-before-running.md`, Round 2.
The reviewer read the complete verdict and verified that `origin/stage1/integration-review-sol` contains its commit.
Both explicit delta CLEANs cover `2c78bb6c8424d37089062437731980ec0d34eab0` and base `20c4e98df6b47a45f3e2284a976e1c8a1341457e`.
The failed base-check condition now has both required source reviews. The same-head integration-review condition is closed.
Sol preserves the Round 1 evidence limits and confirms the final gate and corrected catcher records.
GitHub still reports PR #277 OPEN on that head. The body includes the package delta verdict and still says Sol requested.
PC must add Sol's delta verdict before merge. The gate base must still equal the v1 tip at merge.
No execution job ran here. Wait for the doorbell. Never poll.


## Final body and merge receipt

PC receipt: `msg_plugin-w_1791742011_98c533`.
The reviewer checks the live PR body through the GitHub API again.
The body now includes both same-head delta verdicts, the failed check, both imports, and the green final gate.
The LOW catcher claim remains corrected. All final review conditions close.

GitHub reports PR #277 MERGED at `9f7085f703457b0669785697dbeb14ac687a8942`.
The merge parents are gate base `20c4e98df6b47a45f3e2284a976e1c8a1341457e` and gated head `2c78bb6c8424d37089062437731980ec0d34eab0`.
The reviewer verifies that the merge and gated head share tree `fed65ce895b71a25f22d1c9ac82d3344e9952580`.
The tree diff is empty. The first merge parent equals the final gate base, which satisfies the recorded Q7 condition.
Package verdict remains CLEAN (0 open). This record closes #277 only.
The original program-controls group remains incomplete. The six other R-72 classification places remain with the lead for triage.
No execution job ran here. Wait for the doorbell. Never poll.
