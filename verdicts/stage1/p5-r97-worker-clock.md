# #303 — R-97 worker clock

## Round 1 — first READY

Head: `cd0a468873fc9c318b5002a640d5c6b753007a35`.
Tree: `2f460ae0ebbabafddcc35598e24556b4e31b841b`.
Parent: `3cc95f90154530282319fc0ec28bcc1ed602438a`.
Stacked base: `403bbf689cc9f745a9552cc518374a6a8f2cdb32`, #301.
Gate reference: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
The gate reference is not an ancestor. Its merge base with the head is `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
READY receipt: `msg_plugin-w_1791777811_e372d9`.
Package verdict: `500ea989ea562b09db8b2919410e095f9b1e77a4`, complete Round 1 in `verdicts/p5-r97-worker-clock.md`.

I read the seven-file own diff, affected source paths, R-97, lead rulings, body, supplied local records, and both completed gates.
The own diff has 418 additions and 65 deletions. Its full-index SHA256 is `733530352fbefca054f099469fa2450e1c72636e9d1bc536b70cc9bbc5bcb5ad`.
The body snapshot SHA256 is `6b13074d2056979a6f2949e58499bec46d706adab8af7b60943e62e056453fee`.
The package reviewer independently confirms both findings below.

### R303-SI-1 — MEDIUM — OPEN — run_ready uses stale worker time

`Workers::run_ready` at worker.rs:529 runs the Sim without advancing it to `WorkerClock::now`.
`TestkitRoute::read` at route_client.rs:280 and `input_blocked` at :330 use this path without a Core pump.
`Sim::step` supplies its stored time to `MachineNode::handle`.
`Worker::on_candidate` arms the candidate deadline from that time. `expire_adoption` checks that time before it processes input.

After the worker clock moves, `run_ready` can arm a fresh deadline from an old instant.
The next synchronized worker run can then expire the fresh candidate immediately.
This path can also process input before it applies a deadline already due on the worker clock.
Thus the new clock does not govern every worker execution path.

Synchronize `run_ready` with the worker clock.
Preserve the existing host time and PTY step budget on this path.
Add a manual-clock behavior regression that observes candidate peer bytes or process exit after the worker clock moves.
This is a source finding. I performed no runtime reproduction.
Finding receipts: `msg_plugin-w_1791778130_82c3dd` and `msg_plugin-w_1791778130_bb0513`.
Package confirmation: `msg_plugin-w_1791778312_89bd63`.

### R303-F1 — MEDIUM — OPEN — earlier FLAKY outcome has no disposition

The earlier gate at parent `3cc95f90154530282319fc0ec28bcc1ed602438a` records one real-tier FLAKY outcome.
`conf::ev_5_route_closed_retires_route_keyed_events` failed, then passed on its retry.
The outcome occurred under the wait guard mutation that replaces `due < left` with true.
The gate retains that mutation as MISSED and requires a structural fix under BUILD Testing rule 9.
The body reports the four surviving mutations but omits this FLAKY outcome.

Retain the outcome in the body.
Supply the raw failure signature, diagnosis, owner, and structural fix or applicable accepted disposition.
If P5 uses an existing disposition, show that it applies and that this change does not interfere with it.
The later green gate does not erase the earlier outcome. I requested no new execution job.
Package finding receipt: `msg_plugin-w_1791778269_d973a7`.
Independent Sol request: `msg_plugin-w_1791778251_4388ad`.

### Evidence questions — unresolved

The full raw comparison files lack exact-head and seed headers.
`r97-compare.sh` writes extracted id files, rather than these raw files. Its retained log extracts zero ids on both runs.
P5 must identify the raw-record producer, head, seed command, and clean or same-worktree condition.
The filenames and empty comparator result do not establish this provenance.

R-97 section 4 requires AD-7 sections 2 and 3 to remain passing with the testkit's real-clock default of 500 ms.
Both raw comparison files instead reject `conf::ad_7_crash_between_steps_leaves_no_unregistered_payload` before transcript execution.
They report a real-only guard and no named real proof.
P5 must identify the applicable proof or obtain the required disposition.
I do not claim that those raw files prove AD-7's required sections.
The package reviewer asks the same questions in `msg_plugin-w_1791778097_8ab765`.
These questions remain acceptance conditions in addition to the two numbered findings.

### Other source conclusions

The harness supplies the real worker clock. Manual clock construction is limited to unit tests.
No public control advances the worker clock.
The regular worker run advances its Sim from the worker clock; the Core pump supplies injected time only to the host.
Core's deadline query contains only host deadlines.
The wake wrapper uses the worker clock for its wait end and remaining time.
It waits on the host wake handle, runs worker work after a deadline wait, and forwards the host descriptor.
Process exit signals the host wake.
The tests cover clock isolation, excluded worker deadlines, wait boundaries, host signals, no deadline, and descriptor forwarding.
These conclusions do not close `run_ready`.

The lead permits this orphan/candidate clock PR before the separate stop-grace PR.
The stop-grace change, identity hold, and guardian-exit scope remain separate.
The pending sets equal #301: 409 testkit pending ids, 44 real pending ids, and three held ids.
The pending-file delta changes comments only. No pin, production machine, or mutation exclusion changes.

### Completed gate audit

Current gate: `~/botster-sessions/gates/botster-core-stage1-p5-r97-worker-clock-cd0a4688-pool-20261011-205652-67001.log`.
SHA256: `9038587cebd52f753477862458799bd89b6d1ec058fb553455eadd5993494a29`.
All ten steps PASS in 302.7 seconds. Job and wrapper exit zero after 314 seconds on msa1.
The job has one queued second and 313 run seconds.
Default: 1875 PASS, 432 skipped, 17.134 seconds. Slow: 611 PASS, 2054 skipped, 44.127 seconds.
The eight new clock/wait tests have normal PASS records.
Held-full runs two budget ids at 32 seeds: 64 expected ignored results, zero failures, 19.28 seconds.
All 32 reported passing ignored real ids remain in the unchanged real-pending list.

I independently match all 48 unique mutation identities: 34 caught, 14 unviable, zero missed, zero timeout.
All 34 raw catches equal the citations. Every cited test has normal PASS.
Stage 2 does not run because stage 1 misses no mutation. The log has no FLAKY token.
The mutation outcome duration is 146.2 seconds; the whole mutation step is 148.9 seconds.
The gate diff includes #301 and this PR because it compares against the v1 merge base.
It supplies stacked first-READY evidence, not final integration acceptance.

Earlier red gate: `~/botster-sessions/gates/botster-core-stage1-p5-r97-worker-clock-3cc95f90-pool-20261011-202832-31148.log`.
SHA256: `9b556ae7b586dc254fe5938d2ba2df27ff0e0bbbd10be4544e2ce33753149ef3`.
I independently match its 48 unique initial identities: 30 caught, 14 unviable, four missed.
The four stage-2 identities equal the initial four misses. All four remain missed.
All 30 raw catches match citations and normal PASS. Stage 2 includes the FLAKY outcome above.
The default step passes 1871 tests; the slow step and stage-2 baseline pass 611 tests each.
Eight CI steps PASS; mutants FAIL; fuzz does not run. Total: 840.6 seconds.
Job and wrapper exit 1 after 979 seconds. This is not green evidence.

### Local evidence and its limits

The raw before/after files each have 691 outcomes: 262 PASS, 341 FAIL, and 88 ignored.
I independently compare every outcome cell. All cells match.
Their first diagnostic signatures match except for the startup id: seed 0, step 7 before and step 24 after.
Before raw SHA256: `77436cf5eb7afa4948e2ddbf3ab6fb199a25d7aec57b28c58face76b79e1a1d8`.
After raw SHA256: `9304bedfa688e7cb589c535cd4807dd04fdece1b4f830ae4ed8d7a2e91592122`.
P5 attributes them to #301 and the intermediate clock head, not this final head.
The provenance question remains. These outcomes do not establish an exact-head or full R-97 proof.

The complete manual-reversal script/log records four individual mutations, intended failing wait tests, and restored clean status.
Log SHA256: `15426a8b283d45e2d27d2e66743f02f542c45ee35aa35edac9f43297aa921bf7`.
The script sets Cargo build jobs to four; it does not set the nextest thread cap.
The filtered log supplies no numeric test exit for each reversal.
The current pool gate independently catches all four mutations.

The complete repair-check script/log names the clean exact head, both parallelism caps, and the toolchain override removal.
Log SHA256: `7d9e6c6d365a63d12845afdd2330cff00c97757ba3830602a140a327fee32a2a`.
It records fmt/clippy completion, 305 host, 435 testkit, and 192 worker-core passes.
It records 259 strict conformance passes with seeds 0–31, taint PASS, and clean final status.
Its filtered pipelines do not retain numeric exits for every command.
The scripts cite the 03:47:25 UTC target clean and one subsequent worktree writer.
`b13-switches.log` records that clean of 17016 files and 4.6 GiB.
It also records an earlier stale compile failure. That failure supplies no accepted evidence.
The producer statement and clean record do not establish every earlier target writer.

### Disposition

R303-SI-1 and R303-F1 remain open. Both evidence questions remain unresolved.
P5 must send repair READY with applicable evidence and dispositions.
The present gate cannot carry to a source repair.
After #301 merges, this PR still needs the final slot, current-base import/checks, applicable delta reviews, exact-final-head gate, and Q7.
I ran no build, test, gate, mutation, reversal, measurement, or executable base check.

VERDICT: NOT CLEAN (2 open)

## Round 1 supplement — steward correction to AD-7 condition

Reviewed source remains `cd0a468873fc9c318b5002a640d5c6b753007a35`, stacked base `403bbf689cc9f745a9552cc518374a6a8f2cdb32`.
Lead receipt: `msg_plugin-w_1791779127_e7d8f0`.
The lead names contracts main `ad19a75ae6444f0786ca9ea2a4f5adb7b731d527` and closes the AD-7 acceptance condition.
I read the corrected R-97 item 4 at that exact contracts commit.

AD-7 is real-only and has never run on the testkit, which has no `wait_worker_exit` control.
The AD-7 portion of item 4 now applies only on the real tier.
#303 changes no real-tier implementation for this id; the id stays real-only.
A testkit form needs a separate control and remains separate work.
The two required unit tests remain: injected-time isolation and exclusion of worker deadlines from `next_deadline()`.
The earlier source/gate review covers both tests on the reviewed head.

I read the complete lead-named `evidence/p5-hold/r303-compare.log`.
Its SHA256 is `5de5beb0c7774c32c391da3e5a4d2787b4d797ddb43d3adbfe1e8cfe1240fe81`.
It records a clean of 6320 files and 1.9 GiB, then one worktree writer.
Its compared heads are #301 `403bbf689cc9f745a9552cc518374a6a8f2cdb32` and repair `955ca290737453265d7eb4622c5e998abbe0c84f`.
Each forced testkit AD-7 trial uses seeds 0–31 and ends at `UNSUPPORTED_CONTROL op=wait_worker_exit`.
This supports the corrected testkit scope. It does not report a real-tier AD-7 PASS.
The lead explicitly accepts this disposition. The AD-7 evidence condition closes by that ruling and disposition.

This supplement accepts no source or gate at the repair head, which is not yet reviewed here.
R303-SI-1 and R303-F1 remain open on the reviewed head.
The original raw comparison provenance condition also remains unresolved in this review.
Repair READY and its applicable source, evidence, body, and package review remain due.
I ran no execution job.

VERDICT: NOT CLEAN (2 open)
