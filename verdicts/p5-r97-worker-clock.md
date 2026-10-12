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

### Open evidence questions

I requested two clarifications in `msg_plugin-w_1791778097_cf56cc`; Sol received `msg_plugin-w_1791778097_8ab765`.
Neither question has a disposition at publication of this round.

R-97 section 4 requires `ad_7_crash_between_steps` sections 2 and 3 to remain passing with the testkit's real-clock default.
Both full comparison records instead reject that id as real-only, with its real proof absent, before transcript execution.
Those records do not exercise the required sections. P5 must state the applicable proof or obtain the required disposition.

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
Their provenance question and the missing AD-7 execution remain open.

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

R303-SI-1 and R303-F1 remain open. The two evidence questions also remain unresolved.
P5 must send a source repair READY with the required evidence and dispositions.
The current green gate does not close these findings or carry to a source repair.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge check.

VERDICT: NOT CLEAN
