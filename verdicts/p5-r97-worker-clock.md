# P5 review — #303, R-97 worker clock

## Round 1 — 2026-10-11

- PR: <https://github.com/trybotster/botster-core/pull/303>
- Head: `cd0a468873fc9c318b5002a640d5c6b753007a35`
- Tree: `2f460ae0ebbabafddcc35598e24556b4e31b841b`
- PR base: `403bbf689cc9f745a9552cc518374a6a8f2cdb32`, branch `stage1/p5-hold-start`, #301.
- Parent: `3cc95f90154530282319fc0ec28bcc1ed602438a`.
- Gate base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
- READY: `msg_plugin-w_1791777811_e541cc`.
- Risk: HIGH, rule 3, shared testkit.

I read the complete seven-file own diff and the affected worker, route, timer, and wake paths.
The diff has 418 additions and 65 deletions. Its full-index SHA256 is `733530352fbefca054f099469fa2450e1c72636e9d1bc536b70cc9bbc5bcb5ad`.
The PR merge base equals the stacked base. The diff check reports no whitespace error.
No conformance id changes status. The pending-file change only updates the startup-id reason.
I also read the complete current body, R-97, local comparison/reversal/check records, both raw comparison files, and both completed gates.

### R303-SI-1 — MEDIUM — run_ready uses stale worker time

Sol finding receipt `msg_plugin-w_1791778130_bb0513` identifies this defect. I independently confirm it and retain Sol's finding id.
`Workers::run_ready` at worker.rs:527 runs the Sim without advancing it to `WorkerClock::now`.
`TestkitRoute::read` at route_client.rs:280 and `input_blocked` at :330 call this method without a Core pump.
`Sim::step` passes the Sim's stored time to `MachineNode::handle`.
The worker uses that time for `on_candidate` and `expire_adoption`.

After the worker clock moves, this path can arm a fresh candidate deadline from an old instant.
The next clock-synchronized run can then expire that fresh candidate immediately.
The same path can process input before it applies a deadline that is already due on the worker clock.
Thus the new clock does not govern every worker execution path, as R-97 requires.

Synchronize `run_ready` with the worker clock. Preserve no host-clock advance and no new PTY step on that path.
Add a manual-clock behavior regression that observes the candidate peer or process-exit result after the worker clock moves.
No reviewer runtime reproduction was performed.
Confirmation receipts: P5 `msg_plugin-w_1791778312_09427d`; Sol `msg_plugin-w_1791778312_89bd63`.

### R303-F1 — MEDIUM — earlier FLAKY result has no disposition

The first red gate at `3cc95f90154530282319fc0ec28bcc1ed602438a` has four surviving mutants and one stage-2 FLAKY result.
The test is `conf::ev_5_route_closed_retires_route_keyed_events`, under the wait guard mutant `due < left` replaced with true.
It failed and then passed on its retry. The gate correctly retains the mutant as MISSED and requests a structural fix under BUILD Testing rule 9.
The body reports the four misses but omits this FLAKY outcome.

Retain the raw outcome in the body. Supply its failure signature, diagnosis, owner, and structural fix or applicable accepted disposition.
Provide the change's non-interference where an existing disposition is used.
The later green gate does not erase the earlier result. I requested no new run.
Finding receipts: P5 `msg_plugin-w_1791778269_2d86f0`; Sol `msg_plugin-w_1791778269_d973a7`.
Sol independently records the same evidence issue in `msg_plugin-w_1791778251_233612`.

### Evidence questions

I requested two clarifications in `msg_plugin-w_1791778097_cf56cc`; Sol received `msg_plugin-w_1791778097_8ab765`.
Both questions were open at the initial publication of this round. The AD-7 question closes under the correction below.

The earlier R-97 section 4 required `ad_7_crash_between_steps` sections 2 and 3 to remain passing with the testkit's real-clock default.
Both full comparison records instead reject that id as real-only, with its real proof absent, before transcript execution.
Those records do not exercise those sections. I requested the applicable proof or disposition.

The lead supplies the disposition in `msg_plugin-w_1791779127_85efbe`.
I read corrected R-97 item 4 at contracts commit `ad19a75ae6444f0786ca9ea2a4f5adb7b731d527`.
The AD-7 requirement applies only to the real tier. The id remains real-only; a testkit `wait_worker_exit` control is separate work.
The two required unit proofs remain. The reviewed head's completed gate covers both.

I read the complete lead-named `evidence/p5-hold/r303-compare.log`.
Its SHA256 is `5de5beb0c7774c32c391da3e5a4d2787b4d797ddb43d3adbfe1e8cfe1240fe81`.
It records a clean of 6320 files and 1.9 GiB, then one worktree writer.
Its compared heads are #301 `403bbf689cc9f745a9552cc518374a6a8f2cdb32` and repair `955ca290737453265d7eb4622c5e998abbe0c84f`.
Both forced testkit AD-7 trials report unsupported `wait_worker_exit`. They do not report a real-tier AD-7 PASS.
The lead accepts this scope evidence. The AD-7 question closes by the steward correction and lead disposition.
This closure accepts no repair source or gate; the reviewed source remains the head named above.
I read Sol's matching Round 1 supplement at `15f07babf7666cb4e21c767327ea1373c14778e4`.

The raw comparison files have no exact-head or seed headers.
`r97-compare.log` extracts zero ids for each run; `r97-compare.sh` writes only the id files, not the supplied raw files.
P5 must identify how the raw records were produced, including heads, seed command, and the clean/same-worktree condition.
The filenames alone do not establish that provenance. I retain the observed outcomes below without an exact-head execution claim.

### Other source assessment

The harness supplies `WorkerClock::real`. Manual clock construction is limited to unit tests; no control advances it.
Core pumps pass injected time only to the host. The regular worker run advances its Sim from the worker clock.
Core's deadline query excludes worker deadlines. The wait wrapper compares its timeout and worker deadlines on the same clock.
It waits on the host handle, runs worker work after a deadline wait, and returns Woken only from the host handle.
Process exit now signals the host wake. The wrapper forwards the host descriptor.
The manual-clock wait tests cover a deadline inside the wait, a host signal, the timeout boundary, no deadline, and the forwarded descriptor.
The two Core tests cover injected-time isolation and an excluded worker deadline. The real-clock wait tests pass in the completed gate.
These conclusions do not close the separate stale `run_ready` path.

### Completed evidence

Earlier red gate SHA256: `9b556ae7b586dc254fe5938d2ba2df27ff0e0bbbd10be4544e2ce33753149ef3`.
Its path is `gates/botster-core-stage1-p5-r97-worker-clock-3cc95f90-pool-20261011-202832-31148.log` under `~/botster-sessions`.
Stage 1 reports 48 mutants: 30 caught, 4 missed, 14 unviable, no timeout, in 213.0 seconds.
The slow baseline passes. Stage 2 tests those four and misses all four in 210.5 seconds, with the FLAKY outcome above.
The wrapper exits 1 after 979 seconds. This is not accepted green evidence.

Current gate: `~/botster-sessions/gates/botster-core-stage1-p5-r97-worker-clock-cd0a4688-pool-20261011-205652-67001.log`.
SHA256: `9038587cebd52f753477862458799bd89b6d1ec058fb553455eadd5993494a29`.
It names this exact head and gate base. All ten steps PASS in 302.7 seconds.
Job and wrapper exit 0 after 314 seconds on msa1; the job has one queued second and 313 run seconds.
Default: 1875 passes and 432 skips in 17.134 seconds. Slow: 611 passes and 2054 skips in 44.127 seconds.
Coverage checks 1076 slow-only tests and four named real proofs.
All eight new clock/wait tests have normal PASS records.

I independently parse all 48 unique current mutation outcomes: 34 caught, 14 unviable, no miss, and no timeout, in 146.2 seconds.
All 34 raw catches match the 34 citations. Every cited test has a normal PASS. No stage 2 is needed.
The four earlier misses are caught on this head. The current log has no FLAKY token.
All 32 reported passing ignored real ids remain in the unchanged real-pending list.
The strict check runs 321 of 339 pending ids, with 18 real-only ids excluded and 3 held.
Fuzz reports no changed crate with a decoder harness; no new fuzz execution is claimed.

The gate base is not an ancestor of the head. Their merge base is `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Thus the gate's diff includes the #301 stack and this clock change. Its result supplies first-READY stacked evidence, not final integration acceptance.
The lead requires #301 to merge first. Final import, applicable delta/checks, exact-final-head gate, and Q7 remain due.

I independently parse 691 outcomes in each raw comparison file: 262 PASS, 341 FAIL, and 88 ignored.
The outcome maps are identical. Only the startup id's diagnostic record changes, from step 7 to step 24.
The raw files' SHA256 values are `77436cf5eb7afa4948e2ddbf3ab6fb199a25d7aec57b28c58face76b79e1a1d8` and `9304bedfa688e7cb589c535cd4807dd04fdece1b4f830ae4ed8d7a2e91592122`.
Their provenance question remains open. The AD-7 condition closes under the corrected ruling and disposition above.

I read the complete manual-mutant script/log and repair-check script/log.
`r97-mutants.log` SHA256 is `15426a8b283d45e2d27d2e66743f02f542c45ee35aa35edac9f43297aa921bf7`.
It starts on this clean head, applies each of the four missed mutants separately, records failing wait tests, and restores clean status.
`r97-fix-checks.log` SHA256 is `7d9e6c6d365a63d12845afdd2330cff00c97757ba3830602a140a327fee32a2a`.
It names this exact clean head and records fmt/clippy success, 305 host, 435 testkit, and 192 worker-core passes.
It reports 259 strict seed 0–31 conformance passes, taint PASS, and clean final status.
The scripts cite the target clean at 03:47:25 UTC and one worktree as the subsequent writer.
`b13-switches.log` retains that clean of 17016 files and 4.6 GiB. It also records a preceding stale compile failure before the clean.
That preceding failed build is not accepted evidence. Filtered pipelines supply named outcomes and summaries, not numeric exits for each command.

### Disposition

R303-SI-1 and R303-F1 remain open. The original raw comparison provenance question remains unresolved.
P5 must send a source repair READY with the required evidence and dispositions.
The current green gate does not close these findings or carry to a source repair.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge check.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11 — source repair and evidence dispositions

- Head: `955ca290737453265d7eb4622c5e998abbe0c84f`
- Tree: `cf37a37e2f982bb699fd41365cb1252eeebcb59c`
- Stacked base: `403bbf689cc9f745a9552cc518374a6a8f2cdb32`, #301.
- Parent: reviewed Round 1 head `cd0a468873fc9c318b5002a640d5c6b753007a35`.
- Gate base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
- Repair READY: `msg_plugin-w_1791779445_e5eeaf`.

I read the complete two-file repair and its affected execution path.
The repair diff SHA256 is `206d58bc080fe35b410caa2ed9e8eea5e15209e26800043bac37b37362e58dbf`.
The complete own diff has seven files, 450 additions, and 69 deletions.
Its full-index SHA256 is `5aaa67b8094e2e2012aaf12ba1e4c12929d69b7fe03f6efadd2c2b11d7e66b01`.
The PR merge base remains the stacked base. The diff check reports no whitespace error.
No id changes status. The earlier clock, wait, and deadline-query conclusions remain applicable.

### R303-SI-1 closes

`run_ready` now calls `run_at_clock`, so it advances the Sim to the worker clock before handling work.
This also gives fresh candidate inputs the current worker time and applies due adoption deadlines before input handling.
The path adds no host pump, host-clock advance, or program-edge step.
The new manual-clock regression observes the peer: WouldBlock at 99 ms, then EOF when the orphan deadline acts at 100 ms.
It asserts the worker deadline before and after expiry. The behavior proof satisfies the requested regression.
Its normal gate run passes, and its `run_ready` no-op mutant is caught.

I read `r303-si1-reversal.log` and P5's provenance receipt `msg_plugin-w_1791779624_c9afae`.
The log SHA256 is `4fb847372ade7388bc575d7bf994a20fbbea1375b4ce5852ac018dc6208be91d`.
The retained output shows this regression failing at the expiry assertion with the old `run_ready` body.
The marked note, added after the run, supplies head 955, the old cd body, the test command, and the cache condition.
The switch log records a clean of 20100 files and 5.3 GiB at 04:12:12 UTC after the prior worktree writer.
P5 states that only this worktree wrote to the target before the reversal. This supplies the B13 condition as an owner statement.
The filtered failure output contains no numeric command exit or original head header. Those limits remain explicit.
The source regression and completed exact-head gate independently support this closure.

### R303-F1 closes for the initial review

I read the retained stage-2 mutant log. Its SHA256 is `5f2039919e2f02d190bab0b2c75b4608f5c08969042303c57f7474462c1d424c`.
It preserves the real-tier failure at seed 0, zero-based step 28: activity_before_close false, closed true.
The earlier gate's FLAKY result remains a failed-then-passed retry, not a catch.

I independently match that signature to the R-66 follow-up.
At .41 transcript index 25, `await_quiet` does not prove that Core consumed the client-activity report before Detach.
At .42 and .43, that same step awaits route_reports with activity_before_close true and closed false.
Index 28 is the retained expectation that failed. The corrected fence supplies its missing precondition without polling the event.
RC owns this structural repair. The fix is already in the contracts tags and will reach this branch through #299's .43 pin.

I read the real harness composition. It uses production RealEdges, HostDriver, and the real wake.
The real trial constructs neither TestkitCore nor WorkerDeadlineWake. This PR's changed code and the wait mutation do not alter that failure path.
I accept this diagnosis, non-interference, and owned structural disposition for the initial review.
The complete body now retains the raw outcome, diagnosis, owner, fix, and branch-import obligation.
The final import must carry the corrected fence before merge. This CLEAN does not accept a final branch that retains the old fence.

### Comparison provenance and AD-7 scope close

P5 explicitly withdraws the old `r97-raw` files because their producer and command cannot be established.
Those files remain historical, unvalidated observations. The current evidence does not validate them retroactively.

I read the complete replacement script, log, raw files, and their comparison results.
`r303-compare.sh` records the commands and writes each exact head, time, target, and command into its raw output.
Its script SHA256 is `31f36c41f43b49669f0899a30009e7fe5988a998879fef6c2dd232a18369f83a`.
The comparison log SHA256 is `5de5beb0c7774c32c391da3e5a4d2787b4d797ddb43d3adbfe1e8cfe1240fe81`.
It records a clean of 6320 files and 1.9 GiB, then one worktree writer across both runs.
The script removes pending entries locally, runs testkit seeds 0–31 with four test threads, and restores the lists and Cargo.lock.
The final branch and head match the repair; the final status is clean.

Before raw SHA256: `5dbac2495ec22755d27d7f0dc059b15df4f5a84aab3ee4c541c32acdad577084`.
After raw SHA256: `2766155c391f7024766ae82d4cc864fd2e35ec51025786e6d3c5bce2232cbe1d`.
Their headers name #301 head 403 and repair head 955 with the actual commands.
I independently parse all 691 outcomes in each file: 262 passes, 341 failures, and 88 ignored results.
Every outcome cell matches. Only the startup id's diagnostic record changes, from seed 0 step 7 to step 24.
The id stays pending for the remaining query-path failure. No new conformance status is claimed.
The replacement records close the original provenance question.

The AD-7 condition remains closed by corrected R-97 and the lead disposition recorded in Round 1.
Both forced testkit trials report unsupported wait_worker_exit. They establish testkit scope, not real-tier PASS.
The two R-97 unit proof requirements remain and pass on the repaired head.

### Completed repair checks and gate

I read complete `r303-checks.sh` and `r303-checks.log`.
The log SHA256 is `41f74d502f0d20887a0460eec649d56b67890d76bee809d4cfb1e709d195c4f7`.
It names this exact clean head, rustc 1.97.0, and macOS 27.0.1.
Its script cites the preceding comparison clean and one subsequent worktree writer.
The filtered output records fmt/clippy success, 305 host, 436 testkit, and 192 worker-core unit passes.
It records 259 strict seed 0–31 conformance passes, taint PASS, and a clean final status.
These pipelines retain outcomes and summaries, not numeric exits for every command.

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-r97-worker-clock-955ca290-pool-20261011-212423-85992.log`.
SHA256: `b51fd95ae9cecf2333aafcb9232b92f0cdd8321fbf1cc53812cd79f9249407fd`.
The header names the exact repair head and gate base. All ten steps PASS in 335.4 seconds.
The job exits 0 after 347 seconds with no queue wait. The wrapper exits 0 after 348 seconds on msa1.
Default: 1876 passes and 432 skips in 22.925 seconds. Slow: 611 passes and 2055 skips in 47.356 seconds.
Coverage checks 1076 slow-only tests and four named real proofs.
All nine clock/wait tests pass, including the new route-data-plane regression in 0.004 seconds.

I independently parse 49 unique mutation outcomes: 35 caught, 14 unviable, no miss, and no timeout, in 162.1 seconds.
All 35 raw catches equal the 35 citations. Every cited test has a normal PASS.
No stage 2 is needed. The current completed log has no FLAKY token.
The earlier FLAKY result retains its separate diagnosis and disposition above.
Both strict summaries report 259 passes and no failure. The pending check runs 321 of 339 ids, with 18 real-only ids excluded and 3 held.
The held budget check reports 64 ignored trials and no failure in 19.28 seconds.
All 33 reported passing ignored real ids remain in the unchanged 44-entry real-pending list.
Fuzz reports no changed crate with a decoder harness; no new fuzz execution is claimed.

### Disposition

Zero package findings or evidence questions remain for this initial stacked review.
I read the complete corrected body at the unchanged repair head. It preserves all earlier gate and evidence scopes.
This CLEAN covers the source repair, body, evidence dispositions, and completed repair gate.
Gate base fcc is still not an ancestor; its merge base remains bfef. This is not final integration acceptance.
After #301 merges, #303 needs the final slot, current-base import, applicable checks and delta reviews, and a completed exact-final-head gate.
That import must carry #299's corrected R-66 fence. Both final reviews and Q7 remain required.
The separate stop-grace change remains #305; this verdict does not accept it.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge check.

VERDICT: CLEAN

## Round 3 — 2026-10-12 — slot import source delta

- Head: `df461c83ffa8761c691e5ee07cf50f20784029f7`.
- Tree: `d821aaeeb7dbb28623145dc0867fb8314fc91908`.
- Imported base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Parents: accepted repair `955ca290737453265d7eb4622c5e998abbe0c84f` and the imported base above.
- Delta request: `msg_plugin-w_1791829265_35342e`.
- Integration assignment: `msg_plugin-w_1791829329_14a8a5`.
- Initial package CLEAN: `da9040d7345317204d230822b45da6bd15592725`.
- Initial integration CLEAN: `08e9cc63b574bc384602dae5962f7cd84a8bfea6`.

The lead handoff assigns one v1 import before train 6. This round reviews that source delta, not a train gate.
I read the merge object, combined merge diff, complete own delta, affected import contexts, local summary, and corrected body.
The own delta has seven paths, 450 additions, and 68 deletions.
Its full-index SHA256 is `08d9cb91be9716a11c00fcddc38146137d5acb0aea26da67134518ffccaa929f`.
The Git whitespace check reports no error. The accepted head and imported base have merge base `403bbf689cc9f745a9552cc518374a6a8f2cdb32`.
Every path outside the seven own paths equals the imported base.

### Patch preservation and conflict resolutions

Five own patches retain exactly their accepted added and removed lines, excluding diff indices and hunk locations.
These are the pending comments, core tests, harness, start-control tests, and worker tests.
The other two patches differ only as follows:

- The route-client test imports retain v1's Instant import. The imported test node needs that type.
- The worker patch removes v1's revised deadline documentation before adding the accepted host-only documentation.

The route-client constructor still uses WorkerClock::real. Its imported Reader node has no deadline and retains its accepted behavior.
The worker resolution keeps until_deadline, the repaired run_ready, and v1's test-only add_node helper.
run_ready still calls run_at_clock. It moves no host clock and starts no new program step.
The peer-visible 99/100-millisecond regression remains unchanged.
The wake wrapper, timeout comparison, descriptor forwarding, and worker-exit wake retain the accepted implementation.
TestkitCore::next_deadline still returns only the host driver's deadline.
The resolution preserves R-97's separation of host and worker clocks.

The imported #305 change removes the worker's stop-grace timer and retains KillPayload delivery in both drivers.
The merged no-grace worker test uses the manual worker clock and still requires no worker deadline after EndPayload.
The test's clock adaptation matches the accepted #303 patch. Its no-grace assertion comes from the accepted base.
Core's stop grace remains a host deadline and stays visible through the host-only query.
No #305 signal handler, group-kill path, or launch constructor is replaced by this import.
The imported start holds, operation holds, report controls, and other outside-own paths remain those of the accepted base.

### Lists and the required R-66 fence

I independently compare all nine conformance text sets against the common base, accepted member, and imported v1.
Every three-way formula passes. Every final identifier set exactly equals imported v1.
No prior removal returns. This PR removes no identifier; it changes only the existing startup-clock explanation.
The new comment retains the separately owned query-path failure and its pending identifier.

All contracts lock sources point to `.44`, commit `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea`.
I read the exact pinned ev_5_route_closed_retires_route_keyed_events transcript.
At index 25 it waits for route_reports with activity_before_close true and closed false before Detach.
At index 28 it checks activity_before_close true and closed true.
Transcript SHA256: `30cb1e4112929dbd10a0bfa47b6575cb2850b733e5ead2a0f3d4b09b44057448`.
This import satisfies R303-F1's required structural fence import. The earlier FLAKY outcome remains historical and is not converted into a catch.

### Local summary and R303-F2 — LOW, closed

I read complete r303-slot-checks-df461c83.txt and the relevant B13 switch records under evidence/p5-hold.
Summary SHA256: `0c7a36e464fa2f4e02f696515e8bb77514636de980611263e2b053a1f264b25f`.
The summary names this exact head, target, caps, Mac platform, and commands.
The switch record retains the 18:10:02 UTC clean of 57590 files and 14.1 GiB before work in p5-r87-held.
The next recorded switch is to p5-read-facts at 18:21:21 UTC, with another clean.
The owner reports fmt, workspace test compilation, lists, testkit unit tests, and clippy success.
The owner reports 264 strict seed 0–31 conformance passes and zero failures.
These are owner-written terminal results. No raw command output, numeric exit, unit-test count, or independent target-writer history is retained.
They supplement the source review and do not replace the future exact-train-head gate.

R303-F2 required the body to state this summary limit and scope its .41 pin statement to the earlier gate head.
Finding: `msg_plugin-w_1791829479_0991c4`. Owner correction: `msg_plugin-w_1791829562_34e3a0`.
Sol's matching check is `msg_plugin-w_1791829567_135886`.
I read the complete corrected body on the unchanged slot head and v1 PR base.
It now identifies the owner-written summary, absent raw output and exits, historical .41 head, current .44 pin, and corrected fence.
R303-F2 closes before this source-delta round is published. No product change or new run was requested.

### Disposition

Zero package source-delta or body findings remain on this exact slot head.
SI-1, R303-F1, the original comparison provenance question, and corrected AD-7 scope retain their earlier closures and evidence limits.
The local summary and earlier gates supply no final train acceptance.
Matching integration CLEAN, applicable train composition reviews, the completed exact-train-head gate, and final evidence receipts remain required.
The lead owns train 6 and its gate. This verdict accepts no future head, gate, or merge.
I changed no product code and ran no build, test, gate, mutation, measurement, or executable merge check.

VERDICT: CLEAN
