# Integration review — #297, R-88(B) op_posted

## Round 1 — complete initial source and evidence review

PR: https://github.com/trybotster/botster-core/pull/297.
Risk: HIGH, rules 3 and 5.
Exact head: `2bb06bc335bb076136dbc76b466b4b22c5b34ff2`.
Base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
Tree: `4698444dfd27c0f0cf7c5d0f4ab158c1e55c16f7`.
Parents: `88eacd8da0c41267d779d4267aeb7c592ba85bd6` and the base above.
READY receipt: `msg_plugin-w_1791769214_6114d8`.

Sol reads the complete seven-file delta, with 280 additions and 64 deletions.
This is the first #297 integration review. No earlier #297 CLEAN carries to this head.
The actual merge base equals the named base. The imported #292 tree equals its accepted final tree.
All conformance files, Cargo.toml, Cargo.lock, .cargo, and mutants.toml remain equal to base.
The PR changes no pin, pending identifier, exclusion, or production test branch.
Sol reads R-88(B) at contracts-v0.1.42, `ec7075a4a3fee067323f24f02576777ccd1abea7`.
This control must report whether Core posted the terminal event, before or after its poll.
It must be a pure read. The future .42 fences and Core pin are separate work.

### R297-SI-1 — MEDIUM — retained record accepts a dropped or crashed handle

The testkit op_posted handler at process_controls.rs:188 reads `record_of(handle)` only.
TestkitHarness::drop_handle at harness.rs:254 removes only the entry in `handles`.
Its documentation states that a control naming the gone handle is Bad.
The record remains in `records`, and the control dispatcher does not validate the handle first.
Thus op_posted returns a cached boolean after the handle is dropped.
For an earlier posted operation it can return `{"posted":true}`.

The real op_posted handler at real.rs:585 also reads only `records.get(handle)`.
Real drop_handle at real.rs:1206 removes only `handles`.
The complete host_crash path at real.rs:880–923 validates the tap and quiet state,
then removes the handle entry and crashes Core. It retains the record.
The new handler can therefore accept a dropped handle or a successfully crashed handle.
The existing real tap lookup checks the current handle and upgrades its weak tap;
the new record read bypasses that check. Neither dispatcher supplies the missing validation.
This is a static source finding. Sol performs no runtime reproduction.

Required change: validate the current handle before reading its record in both tiers.
Reject dropped handles in both tiers and crashed handles in the real tier.
Keep the control a pure read and retain the correct before-poll and after-poll answers for an open handle.
Add lifecycle regressions after an earlier successful completion, including real host_crash.
Do not retain Core in the record to make this check.

Sol sends the finding to P5 and its reviewer in `msg_plugin-w_1791769718_fce69b`
and `msg_plugin-w_1791769718_6eb834`.
Package finding R297-F1 in `msg_plugin-w_1791769515_b12c05` has the same defect and closure conditions.
The package's first message named the wrong testkit line; its formal record corrects it to 188.
The passing evidence below does not exercise this lifecycle case and does not close the finding.

### Interface and lifetime assessment for open handles

Sol reads HostEngine admission, the complete commit path, terminal posting, polling, and step updates.
next_op starts at 1. Admission checks arguments, payload size, pending limit, and capacity before it mints an id.
It then increments next_op and commits the admitted operation, with no later synchronous rejection or id hole.
Every admitted variant inserts PendingOp.
complete sets Step::Done only when it posts Completed. If another event was posted in that step,
it schedules NextComplete first. Queue::post_completed pushes the reserved completion without a room refusal.
The only ops.remove occurs when poll_events takes Completed. set_step cannot overwrite Done.
Thus a nonzero id below next_op that is absent from unposted_ops has a posted terminal event.
The record retains this fact after a poll. Unknown ids and still-unposted ids remain false.

CoreRecord combines the existing parked-work set with next_op and the unposted-op set.
RefusalLayer captures the initial diagnostics and records after every applicable forwarded state-changing call.
A scripted synchronous refusal does not reach Core and mints no id.
Both open paths give the layer and harness the same fresh record.
The handlers read it without a Core call, pump, poll, or clock change.
The change preserves the parked-work row mapping and the real capture, driver, and crash-slot composition.

Both tiers use the same scenario. It verifies false before the worker completion,
pumps until the terminal event is posted, verifies true before polling,
then confirms that the next poll still returns that operation's Completed.
It also checks true after polling and false for an unminted id.
Fast record tests cover missing next_op, zero, the issued boundary, waiting, and post-poll history.
The diagnostics test checks the pending set before completion and the queued terminal event after posting.
No additional source finding is open.

### Completed exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-r88-op-posted-2bb06bc3-pool-20261011-183117-63636.log`.
SHA-256: `2dbfeed5bcb1ab28b16417234f3c403aa317941ba50a9ac60f0e982e5ab993d1`.
Its header names the exact head and base. It runs on msa1, Linux 6.12.111+deb13, with four CPUs.
All ten steps PASS in 497.6 seconds. The job exits zero after 506 seconds,
with one queued second and 505 run seconds. The wrapper exits zero after 507 seconds.
Default: 1853 PASS, 435 skipped, 15.830 seconds.
Slow: 608 PASS, 2044 skipped, 43.591 seconds.
The new diagnostics test passes in 0.003 seconds.
The testkit op_posted self-test passes in 0.004 seconds; the real self-test passes in 0.051 seconds.
The existing parked-work tests also pass.

Both conformance summaries have 256 PASS and zero failures, in 6.61 and 5.56 seconds.
Strict checks 324 of 342 pending identifiers, with eighteen real-only identifiers not run and three held.
Minimum coverage is testkit-passing 65/69, real-passing 58/68, and real-accepted 59/69.
All four named real-process proofs PASS.
Held-full checks two budget identifiers at 32 seeds, with 64 expected ignored results and zero failures,
in 19.31 seconds. This control-only PR supplies no status removal.

Sol independently parses all 37 unique first-stage outcomes:
22 caught, nine missed, six unviable, zero timeout, in 140.6 seconds.
The unmutated slow baseline has 608 PASS, eleven slow cases, 2756 skipped, and 44.890 seconds.
Its complete interval is 63.3 seconds.
Stage 2 examines exactly the nine first-stage missed identities:
seven caught, two unviable, zero missed, zero timeout, in 142.4 seconds.
The combined result is 29 caught and eight unviable, with no missed or timeout outcome.
All 29 unique raw catches equal the 29 unique citations.
Each selected catcher has exactly one normal PASS before mutation;
each second-stage catcher also has unmutated baseline PASS.
The new predicate comparisons have named record or scenario catches.
The complete log has no FLAKY token. Fuzz reports no changed crate with a decoder harness.
The stopped 88eacd gate is not accepted evidence. No gate result carries to a repair head.

### Local evidence and six future-fence transcript trials

Sol reads the complete script and completed log, without executing them.
Script: `~/botster-sessions/shared/core-stage1/evidence/p5-r87/r88-core-pr-evidence.sh`.
Script SHA-256: `01655005ca73980678e25a660d7762bfcb5db2c6af4f003e4a69535d64837c35`.
Log: `~/botster-sessions/shared/core-stage1/evidence/p5-r87/r88-core-pr-evidence-2bb06bc3.log`.
Log SHA-256: `a018d5d898661ba2eb18b29fcd9f534dacc20883884702f7ffcaaaea0b19b6d8`.
The log records this exact clean Core head, macOS 27.0.1, and rustc 1.97.0.
It records a target clean of 24016 files and 7.0 GiB, zero remaining target entries,
and the same Core worktree as the only builder during the run.
Fmt, lists, taint, and clippy show success. Host reports 300 PASS and testkit reports 422 PASS.
The filtered control self-test run reports twenty PASS, zero failures,
including op_posted and parked_work in both tiers. The candidate prebuild names three binaries.

The script uses the .41 contracts crates at `2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87`.
It copies only six transcripts from .42 target `ec7075a4a3fee067323f24f02576777ccd1abea7`.
Sol independently verifies all six copied blob hashes against that target:
am_4 capacity, ev_5 full queue, ev_5 poll, ev_5 stop, tm_6 more input, and tm_6 poll.
Every named testkit trial PASS uses seeds 0–31; every named real trial PASS uses seed 0.
The script temporarily removes pending entries for those local trials,
then restores the pending files, Cargo.lock, and contracts conformance files.
The final log reports both worktrees clean.
The filtered command pipelines supply no per-command numeric exits or aggregate exit proof;
Sol accepts only the retained named PASS records and completed summaries.
This proves those six copied .42 transcripts with .41 crates.
It is not a full .42 pin run, .42 API composition acceptance, or approval for a pending removal.
It does not supply the missing dropped-handle or crashed-handle regression.

### Complete body and package record

Sol reads the complete current PR body at this exact head and base.
Raw body SHA-256: `f2c14722f2266ae0f5a431ece8e4f10935672d5839710f26bd1416eb8e1c0aa4`.
It states HIGH risk, prior art, the record mechanism, completed gate counts,
the #292 import history, and the focused .42 transcript scope with .41 crates.
No body finding is open. The stopped earlier gate supplies no proof.

Sol reads complete package Round 1 at `9a28d3bdf6cb92221d76d7b79e293512998692c0`,
verdicts/p5-r88-op-posted.md.
Its NOT CLEAN covers the same exact head, base, tree, body, and completed evidence.
Package receipts `msg_plugin-w_1791769742_a7354d` and `msg_plugin-w_1791769763_f55cb6`
confirm the same sole MEDIUM finding and matching closure conditions.

R297-SI-1 remains open. P5 must send repair READY on the new exact head.
The repair needs the applicable static and mutation evidence, both reviews,
and its completed exact-head merging gate before acceptance.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job.
Never poll. Wait for the doorbell.

VERDICT: NOT CLEAN (1 open) at 2bb06bc335bb076136dbc76b466b4b22c5b34ff2 against base 20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5

## Round 2 — lifecycle repair and completed exact-head acceptance

Exact head: `3693a2e370f9ab820bc6f05f159e079e9650a6b9`.
Base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`.
Tree: `c83156285a094638acf2097912ce31c62f94c53c`.
Parent: reviewed Round 1 head `2bb06bc335bb076136dbc76b466b4b22c5b34ff2`.
Repair READY: `msg_plugin-w_1791770369_d8b7ba`.
Sol reads the complete three-file repair: 73 additions and 22 deletions.
The complete base-to-head delta is seven files, 342 additions, and 75 deletions.
The actual merge base equals the named base. All conformance blobs remain equal to base.
The engine, predicate, shared scenario, contracts pin, and exclusions have no repair delta.
Their Round 1 source conclusions remain applicable.

### R297-SI-1 closes

Testkit drop_handle removes both the handle and its record.
Real Handle now owns CoreRecord; the independent real records map is removed.
The constructor gives Handle and RefusalLayer the same fresh record.
The pure `record(&self, handle)` lookup reads only through the current handle map.
Both successful host_crash and drop_handle remove that handle and thus remove the harness's access to its record.
The record contains no Core, driver, or tap reference. The crash path still drops those resources.
Neither control adds a pump, poll, clock change, edge call, or record mutation.
parked_work also uses the same record helper and retains its session validation.

The testkit lifecycle test first checks posted true for the completed Create operation.
After the drop it requires Bad for break_control, op_posted, and parked_work.
The record test also requires record_of to return None after drop_handle.
The real drop regression keeps a separate RefusalLayer and record clone alive across drop_handle.
It proves that a retained record still cannot be read through the harness after the handle is removed.
That test uses injected diagnostics for its posted-before-drop setup.
The real host-crash regression uses actual Core and workers, checks posted true before crash,
then requires Bad after the successful crash.
The unchanged record scenarios still check true before the completion's poll and true after that poll.
All these tests have normal PASS in the repaired-head gate below.
R297-SI-1 closes. Package R297-F1 has the same closure conditions and also closes.

### Complete reversal evidence and cache condition

Sol reads both full scripts and logs without executing them.
Testkit log `~/botster-sessions/shared/core-stage1/evidence/p5-r87/r297-f1-revert.log`:
SHA-256 `ebfa555f07aeb91c1666165f2b6094974f55bef51a9eefe8efd8341885eb4483`.
Its script SHA-256 is `68b9596b090ca3295d093762e9605f53837a5e7882ef50ebe7b8675ed3f384ac`.
It starts at the clean repair head. Removing only records.remove makes both testkit regressions fail.
Restoring the file gives clean status and two passing regressions.

Real log `~/botster-sessions/shared/core-stage1/evidence/p5-r87/r297-f1-revert-real.log`:
SHA-256 `0edc0c26d1f62b0535c46895aa7cf821bd9aa5bc8a5b9ef55d4f4c4df08c558a`.
Its script SHA-256 is `1d52e2ab6b5e6c3fb4b60feb3f257a63b5191cd7938bc2dd02bc02e94c3daf50`.
The script uses old real.rs with only the new host-crash assertions.
The run reaches and fails the Bad-after-crash assertion after passing posted-before-crash.
Restoring the repair gives clean status and one passing test.
The script runs this thread-counting test alone.
Both logs retain filtered outcomes and summaries; neither provides each command's numeric exit.

The scripts and logs do not themselves retain a new clean or a complete writer history.
Sol requests the B13 condition in `msg_plugin-w_1791770498_2b2366`.
P5 confirms it in `msg_plugin-w_1791770506_bbb179`:
the same Core worktree was the only targets/p5 builder from the earlier recorded clean at 01:31:21 UTC
through both reversals and the repair checks. The earlier #292 SLOT writer used it before that clean.
Pool gates build on their nodes. This owner statement supplies the same-worktree cache condition.

Sol also reads the complete r297-b13-provenance.sh and r297-b13-provenance.log.
Script SHA-256: `f392f0524a928b9ff24023a60f16f3b845bd9e838396a846a865d34b5c1c40f7`.
Log SHA-256: `f435ddd8c80255193e8f7d3a88ed113139717a60144895be98ffb2e9b5aa6abd`.
The snapshot has 30601 files. Dep-info names this Core worktree, the .41 contracts worktree, and the target directory.
It finds zero directories born before the clean.
The 88 files with earlier birth times reside in Ghostty output directories born after the clean.
P5 attributes those times to source copies; Sol confirms that the build script uses fs::copy.
The snapshot supports the owner statement but does not independently prove every earlier writer.
P5's separate local fmt, clippy, taint, 422 testkit unit passes, and real scenario/drop passes remain owner reports.
The retained reversal records and the exact-head Linux gate supply the independently read repair evidence.
No new execution is requested or performed by Sol.

### Completed repaired-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-r88-op-posted-3693a2e3-pool-20261011-185114-64984.log`.
SHA-256: `0003d9a2b47bd53dfacd82e56275c3e1943cb135001c777eafa1b19f348db3bd`.
The header and lists step name this exact head and base. All ten steps PASS in 450.9 seconds.
On msa1 the job exits zero after 461 seconds, with zero queued seconds.
The wrapper exits zero after 462 seconds.
Default: 1853 PASS, 435 skipped, 13.886 seconds.
Slow: 608 PASS, 2044 skipped, 42.110 seconds.
Both testkit lifecycle regressions PASS; real drop and host-crash regressions PASS.
Both op_posted and parked_work scenarios PASS in both tiers.
Coverage checks 1076 slow-only tests and all four named real proofs PASS.

Sol independently parses all 39 unique first-stage outcomes:
23 caught, ten missed, six unviable, zero timeout, in 102.1 seconds.
Stage 2 examines exactly the ten first-stage missed identities:
eight caught, two unviable, zero missed, zero timeout, in 154.5 seconds.
Its unmutated baseline has 608 PASS, eleven slow cases, 2756 skipped, and 44.463 seconds.
The baseline interval is 55.2 seconds.
All 31 unique raw catches equal all 31 unique citations.
Each selected catcher has exactly one normal PASS; each second-stage catcher also has baseline PASS.
The testkit drop-handler mutant is caught by the record regression.
The real record-helper mutant is caught by the host-crash regression.
The combined result is 31 caught and eight unviable, with no remaining miss or timeout.
The complete log has no FLAKY token. Fuzz reports no changed crate with a decoder harness.

Both conformance summaries have 256 PASS and zero failures, in 5.84 and 5.12 seconds.
Strict runs 324 of 342 pending identifiers, with eighteen real-only not run and three held.
The held-full check has 64 expected ignored trials, zero failures, and 19.27 seconds.
The status files retain 691 ledger identifiers, 412 testkit-pending, and 44 real-pending.
Sol independently verifies that all 32 reported passing ignored real identifiers remain pending.
No status removal or new contracts pin is accepted.
The six .42 transcript trials from Round 1 remain evidence for 2bb06bc3 with .41 crates.
They are not a repaired-head execution claim. This completed gate supplies the repair-head proof.

### Body corrections and matching final package verdict

Sol reads the complete repair body, raw SHA-256 `49df106f34e7f5ba400f9941d00a6d6f8862574ea7ec01ab3fc3b53ef89d0071`.
It still states old diff counts as current and uses present wording for the first SLOT review.
Package receipt `msg_plugin-w_1791770413_67ce6c` requests LOW R297-F2 and the B13 clarification.
Sol independently verifies those issues and agrees with the corrections in `msg_plugin-w_1791770498_2b2366`.

P5 supplies the corrections in `msg_plugin-w_1791770506_bbb179` at unchanged head and base.
Sol reads the complete body delta. Corrected raw SHA-256:
`7aa7f8bfa507f5ccead70b01eea02e031ea1d5babd231d3f10d2d24d0cde5ae0`.
The body names +342/-75 for the current head, dates +280/-64 to the first reviewed head,
and makes the first SLOT review statement historical.
It adds the writer condition and snapshot limits while retaining the separate earlier and repaired gate scopes.
The body precision and local provenance conditions close. No body finding remains open.

Sol reads complete package Round 2 at `f2c2402dcca43ab0b885db38ab74df4978aed5d3`,
verdicts/p5-r88-op-posted.md.
Its final CLEAN covers the same exact head, base, tree, complete repair, corrected body, and completed gate.
It closes both R297-F1 and LOW R297-F2 and preserves the evidence limits above.
Package receipt `msg_plugin-w_1791770672_95016c` confirms zero open findings.

Both reviews now have zero open source, evidence, or body findings on this exact head.
This completes Sol's final acceptance. The lead's Q7 check remains required before merge.
Any later source, base import, pin, pending removal, or different merge tree needs its applicable review and evidence.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job. Never poll.

VERDICT: CLEAN (0 open) at 3693a2e370f9ab820bc6f05f159e079e9650a6b9 against base 20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5
