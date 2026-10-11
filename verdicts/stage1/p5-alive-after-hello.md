# Integration review: #249, payload_alive worker evidence

VERDICT: CLEAN (0 open)

## Round 1: NOT CLEAN (1 open)

- Head: `c0694c7e295c9132d4b607fa8caee53e41a0466c`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Risk: HIGH. The PR changes the shared testkit crate.
- READY: `msg_plugin-w_1791706704_5b1752`.
- Package source finding: AH-F1, message `msg_plugin-w_1791706879_cf80d6`.
- The complete package artifact is requested and remains pending at this record.

Sol reviewed all five changed paths and the relevant process and worker source.
Sol ran no build, test, gate, mutation, reversal, or executable merge-check job.

## AH-F1 MEDIUM: the hold test still counts a wrap child as a payload

Status: OPEN.

The changed test is `the_start_controls_hold_the_launch_and_fail_next_refuses_before_core` in `real.rs`.
Its wait first checks `launch_held`, then reads `live_children` for the recorded worker without checking `heard`.
The predicate returns true when a child exists. The next assertion reports `the Launch passed the hold`.
The recorded PID can still run the guard's wrap stage with its live intermediate child.
That child is not a payload. The production fix in this PR adds `heard` precisely to exclude that case.

`pump_until_at` evaluates the predicate after a quiet pump. Quiet does not prove exec or hello receipt.
The wrapper starts and reaps its intermediate before it execs the worker. Exec keeps the PID and start time.
Thus the test can fail before hello even when no Launch passes the hold.
Sol and the package reviewer independently identified this source path.
P5 confirms that no condition excludes the window in `msg_plugin-w_1791706919_57bc5f`.

Required change: infer a payload from children only after the tap has worker-exec evidence.
Keep the separate observation that Launch reaches its hold. Keep the no-child assertion after that observation.
P5 reports a local correction, `03094697`, and a running focused job.
Sol has not received that correction for review or its completed evidence. Sol will not poll the job.

## Source checks

`Heard` records instances for each data directory and outlives each tap.
An accepted link records its instance only after the tap decodes hello.
A successful `connect_worker` records the instance because the worker binary binds that endpoint.
The worker binary starts its payload only after Launch. Its wrapper reaps its intermediate before exec.
For an unknown payload, `payload_alive` now returns false before that worker evidence exists.
Known payload checks remain unchanged. After worker evidence, child listing and worker identity checks remain.
The directory record preserves the existing reopen case when the new tap has not read a Launched report.

`launch_held` reads whether the armed hold has stopped Launch at the existing edge boundary.
The PR changes no production Core behavior, timeout, transcript, pending list, minimum list, or mutation exclusion.
It does not implement the contracts `start_held` control or the nine R-59 fences.
AD-7 remains in its list. Its unfenced pass does not count toward the real minimum.
The counted minimum remains 35/68, plus one unfenced pass, despite historical reports of 36/68 and accepted 37/69.

Prior art: Sol searched OLD CORE `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` for child inference and start holds.
The old source has no corresponding harness mechanism to reuse.
The PR reuses hello decoding, the directory sharing pattern for Rows, Hold, and the existing test-process helpers.
The new directory record is needed because `link_of` ends with the link and tap.
The PR's Prior art note explains that choice. No alternate process launcher or custom process listing is added.

## Retained evidence and limits

Pre-review gate head: `3406ad795b646cc6eea714b5d21a65db3f93483f`, base `475a442fa58cd2555c4121ce705ea3cf75470692`.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-3406ad79-pool-20261011-004335-49218.log`.
All ten steps pass. Default tests pass 1607 cases. Slow tests pass 428 cases.
The strict testkit report passes 206 tests and retains minimum 55/69. The pending-real report has 77 IDs and zero passes.
The summary reports 1877.9 seconds. The job and wrapper exit 0 after 1987 seconds.

Stage 1 tests fifteen mutants: eight caught, six missed, one unviable, and no timeout.
Stage 2 tests those six survivors: four caught, two unviable, and no missed mutant or timeout.
The final result is twelve caught and three unviable.
Sol checked each identity across both stages. The heard negation and both launch_held constant mutants are caught in stage 2.
The two added unviable mutants return default harness or Core values; their builds fail.
This evidence does not exclude the schedule in AH-F1.

The current import has parents `3406ad79` and `5c480db4`.
Its tree, `ad55bef8d84e46f0dffde1e050f0c5b7fe1e3418`, equals Sol's independent git merge-tree result.
Sol compared both complete own diffs: 24831 bytes, SHA-256 `b7cc0dab62c2d722ca4d5f90061dd4a7913a4439ed11adabd6a10b760fb762b4`.
They are byte-identical. All four retained base-merge-check conditions pass.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-c0694c7e-pool-20261011-011736-69133.log`.

Mac evidence uses `f07c3d1cdbd00b0b312ab503930c9aa52cdacfd4`.
Log: `~/botster-sessions/shared/core-stage1/gate-logs/hello-mac-f07c3d1c.log`.
The run passes 39 of 40 tests. The changed child, hold, reopen, and hello tests pass.
The existing zombie test expects identity Matches and receives Absent. The test command exits 100.
The PR identifies that failure as catalog B9 and separate work. This review does not resolve it or call the Mac run wholly green.
Later own source adds only the armed-hold negative assertion. Later base source includes the reviewed real route and sha2 profile.

The new child test uses a shell with one live cat child and proves the child runs with a returned FIFO byte.
Its public `payload_alive` result is false. A retained selected reversal removes only the heard guard.
Reversal head: `8146eb6c2ed0d875657ced9a7ed443355357d093`.
Log: `~/botster-sessions/gates/botster-core-scratch-p5-hello-revert-c0694c7e-8146eb6c-pool-20261011-011840-78263.log`.
The test then fails with alive true versus false. Its test command exits 100; the wrapper exits 0 after printing that result.
Sol read the reversal source, run header, complete failure, and both exit records.
Scratch debug runs report 30/30 for the hold test and AD-7. Their debug output changes timing and does not establish the failure rate.

The final gate on the corrected reviewed head remains required after both CLEANs.
This verdict grants no merge approval. It does not close the separate findings or final gates for #233, #240, or #243.

## Round 2: source CLEAN

- Head: `0309469729459788da3a369bcab3443debc7a359`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Parent: `c0694c7e295c9132d4b607fa8caee53e41a0466c`.
- Package CLEAN: `62a26786932cffb661b52c324cb685ed8b195629`, read in full, including both rounds.
- Open findings: zero, including LOW.

AH-F1 closes. Sol read the complete one-file correction and checked the source for the record lookup.
The predicate checks launch_held first. It checks heard before it reads children.
The test then asserts heard and no child after the wait. It retains the initial negative hold observation and the release proof.
The record lookup locks Rows and does not lock Tap again. The added Tap lock creates no recursive lock in this path.
The correction changes no production control, OS process operation, timeout, pending list, or transcript.
All imported source and the prior review checks remain applicable.

Focused log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-03094697-pool-20261011-012153-14347.log`.
Sol read the complete log. Its header names the reviewed head and base.
Prebuild passes. Testkit clippy with slow and all targets passes with `-D warnings`.
The hold and shell regression tests run together twenty times; the result is pass=20, fail=0.
The job exits 0 after forty seconds. The wrapper exits 0 after forty-one seconds.
The explicit tally establishes the test result. The wrapper does not enforce a zero failure count.

P5 also supplies a selected reversal at `872b8aaf1724ffdfb2a6a70d4f371c67e3d5733d`.
Its only source change lets every nonempty buffer pass through sendable, including Launch.
Log: `~/botster-sessions/gates/botster-core-scratch-p5-hold-leak-03094697-872b8aaf-pool-20261011-012416-33205.log`.
Sol read the source diff, exact header, five failures, tally, and exit records.
All five runs fail with one child and `the Launch passed the hold`. The result is pass=0, fail=5.
The job and wrapper exit 0 after printing that tally. Their exit codes do not indicate passing tests.
This reversal shows that the corrected proof detects a leaking hold.

Sol checked the published replacement and retained evidence. No reviewer execution job ran.
The final gate on this head remains required after both CLEANs under the lead's pool rule.
This source CLEAN closes no separate finding or final gate for #233, #240, #243, or P6's short data root.
R-59 still leaves AD-7 unfenced and outside the counted real minimum.


## Round 3: completed final gate and the 23al application

VERDICT: CLEAN at `0309469729459788da3a369bcab3443debc7a359` (0 source findings).

Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
The implementation has not changed since Round 2.
Sol read the complete package cause addenda through `21af7a8e8326d354c2aa68895a136fd1603f0b29`.
The new Heard record does not change forwarded bytes, descriptor receipt, close decisions, or the completion fence.
EV-5 runs the changed observer but does not read Heard or arm a hold. Extra observer work can change scheduling.
The addenda claim no equal link-close time or equal failure rate. Sol accepts that source limit.

### Exception record

Plan 23al is CLEAN at 1555a98b196c259dfe5f5d56a87c473ba49d49dd in integration commit 60df36a3.
Sol independently verified the published pin's SHA-256 a3965b731b7b00fbe56e28581d298f38fe4d1b2836ff322f33d4c988cc5183ab.
The rule permits one re-gate for a diagnosed v1 cause outside the PR change, with written confirmation and approval.
The first final gate has one EV-5 PendingLimit failure. Its mutation and fuzz steps do not run.
Sol read that complete failure and final summary, as recorded in the plan verdict.

The lead states the application precisely in `msg_plugin-w_1791708303_99f70a` and the lead handoff.
The failure identity is the retry Detach's failed success expectation at the same transcript step.
Both traced samples have four pending operations and zero queued events before the recovery poll.
The lead uses P3's diagnosis and R-63/R-64 to connect the descriptor trigger, pending operations, and unfenced recovery.
The link-close time can select different admission errors: PendingLimit while the slots remain held, or UnknownRoute after r5 closes.
The plain-v1 sample reproduces UnknownRoute. No supplied plain-v1 sample reproduces PendingLimit itself.
That distinction remains explicit. The matching diagnostic state alone would not establish a shared cause without the source and trace diagnosis.

Sol accepts the lead's stated failure classification for this application, with the diagnosis and error distinction retained.
This does not assert that the returned errors are identical or that every failure with those counts has this cause.
The source confirmation establishes that #249 leaves the diagnosed descriptor and recovery mechanisms unchanged.
The lead records #240 as the descriptor-trigger removal, PC as the R-63 fence owner, and R-64/A21 as the missing-completion owner.
The exception does not close EV-5 or approve a new route-close reason. Core must retain the steward's A21 limit.
The running re-gate began before the corrected rule's acceptance. The lead counted it only after the rule CLEAN and source confirmation.
Sol grants no second further re-gate and no permission to omit a gate step.

### Green final evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-03094697-pool-20261011-013626-2040.log`.
Sol read the exact-head/base header, EV-5 passes, all 15 stage-1 identities, all six stage-2 identities, and the complete final summary.
All ten steps pass. EV-5 passes on the testkit and real tiers and in the slow mutation baseline.
Stage 1 has eight caught mutants, six misses, and one unviable mutant. It reports no timeout.
Stage 2 tests exactly those six misses. It catches four and reports two unviable mutants, with no miss or timeout.
The caught slow mutants are both launch_held constant returns, payload_alive's default return, and its deleted negative Heard guard.
The final result has 12 caught mutants and three unviable mutants. No mutant remains missed or timed out.
The fuzz step finds no changed decoder harness. The CI summary is 295.9 seconds.
The job exits 0 after 303 seconds; the wrapper exits 0 after 304 seconds.

The lead merged #249 into b7c97b42b8e559053153f8a8202ee32dbde0a93a.
Sol checked both parents and the tree. The merge tree equals the reviewed and gated 03094697 tree, 97bc42fce1cd3dc8d2cd84cb9b4fd789c49a4555.
This completion record closes no gate or source finding for #233, #240, or #243.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.
