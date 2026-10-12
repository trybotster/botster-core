# P5 package review: Core #292, R-87 parked_work

## Round 1 — 2026-10-11

- Exact head: `6c231801e78ea268dd2894d669a41aa3c1392615`.
- Tree: `8edac0b57c7913ed5576dce56c280f71ac861aca`.
- Base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- READY receipt: `msg_plugin-w_1791760836_ef1299`.
- Tier: HIGH. The shared testkit and cross-package changes require HIGH under BUILD.md rule 3.
- Open findings: three, including two LOW findings. Local cache provenance also awaits the implementer's answer.

### Scope and authority

I read the complete nine-path own diff, the complete production decisions, both control paths, and all new tests.
R-87's corrected text replaces link_held with parked_work. The read reports work that waits only for mandatory-queue room.
Both a held worker frame and an engine transition can satisfy that condition.
Work runnable now, or work waiting for another condition, must not satisfy it.
The control is read-only. The transcript's await_control pumps without polling; the later zero-timeout wake remains strict.
The lead assigns the Core implementation to P5. The lead keeps am_4 live under option (b) until its R-88 fence.
This PR changes no pending list, transcript, timeout value, dependency pin, mutation exclusion, or gate selector.
Core remains at contracts-v0.1.39. The tag that carries contracts #68 supplies the later transcript fences.

The own commits are:

1. `7ce7cfe1a9cba020836e8dc583ffe139c8eeee43`: the host read and diagnostic field.
2. `dc9b230b964a2a0e945cf54d72a02d8d672de0ba`: both harness controls and the record layer.
3. `98fe6fde4ab261fd66f0a5960384285d34f3551c`: the lead-approved base merge.
4. `76bf7467d1fa5872de11a5c2a469b7166c7faa2e`: tests for the missed decisions and timed-out record mutants.
5. `6c231801e78ea268dd2894d669a41aa3c1392615`: a test comment correction.

The merge's earlier base is `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Nine own paths and seven imported paths are disjoint. Every own blob survives that merge unchanged.
`git diff --check` passes. I claim no executable base-check result from those Git reads.
The old worker's room checks concern worker egress. They do not provide this host read or test control.
I read those checks at old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The PR supplies its prior-art note. No old mechanism or tests are copied.

### Production and control logic

The ready_at extraction changes no decision in the existing ready body.
The accepts_at extraction substitutes the explicit room argument for the old has_room call.
Both production entry points pass the current queue-room value.
The queue-room read is pure. The production outcomes remain unchanged by these extractions.

parked_on_room returns an empty set when the queue has room.
Otherwise, it compares offered work with room against offered work without room.
It excludes work offered under both conditions. It maps op work, session work, and parked route work to their sessions.
For held frames, it requires refusal without room and acceptance with room, then maps the link to its session.
Observed frames waiting for a start remain excluded. The ready predicate also retains row, setter, and ticket conditions.
The host diagnostic reads that set without pumping, polling, signalling, or changing read interest.
The four engine tests exercise state work, op work, parked route work, and held frames, with negative and freed-room checks.

RefusalLayer records the host diagnostic after each forwarded state-changing call and at construction.
The harness retains a separate shared record for each handle. The control reads that record without calling Core.
Scripted refusals do not change Core, so they leave the record unchanged.
Read-only Core calls retain their pass-through behavior. The record has no global lock or Core ownership.
Both control handlers validate the handle and session row before reading the record.
Both tiers register the control. The plain real harness continues to refuse test controls.
The scenario checks a running session, a parked Stop, an unrelated session, the freed-room result, and a missing row.
Its new setup loop still lacks the required bound, as F2 explains below.

### R292-F1 — LOW, open: worker_alive has the wrong attached comment

At real.rs:543–561, the parked_work insertion separates the existing AD-7/R-46 comment from worker_alive.
Rust attaches both the alive comment and the parked comment to parked_work.
The comment now describes two different controls on one method. worker_alive loses its comment.
Required change: put the two alive comment lines directly above worker_alive.
The fourth commit fixes the same placement problem for a different test in refusal.rs.
Sent to P5 and Sol: `msg_plugin-w_1791760917_f3f68a` and `msg_plugin-w_1791760917_945fa0`.

### R292-F2 — MEDIUM, open: the scenario pumps without a bound

In refusal.rs, a_stop_is_parked_work_until_a_poll_frees_room executes `while core.pump(now()).more {}`.
That setup loop has no deadline or progress bound. Both tier tests execute it before the bounded pump_until helper.
If Core continues to report more, the loop never reaches the existing 30-second guard and consumes CPU without a bound.
The scenario's BOUND does not cover this loop. The later helper cannot stop it.
Required change: check an explicit deadline using the existing BOUND, or use a bounded helper that pumps without polling.
Preserve BOUND and all transcript values. Add no sleep or unrelated timeout change.
Sent to P5 and Sol: `msg_plugin-w_1791760945_b59bf5` and `msg_plugin-w_1791760945_8b8e62`.

### R292-F3 — LOW, open: the body cites the wrong HIGH rule

The Risk tier paragraph calls all Core production changes HIGH under BUILD.md rule 1.
Rule 1 concerns gate-decision code. Rule 3 correctly classifies the shared testkit and cross-package change as HIGH.
Required change: remove or correct the rule 1 citation. Keep HIGH.
Sent to P5 and Sol: `msg_plugin-w_1791761034_1b4407` and `msg_plugin-w_1791761034_4ea3a7`.

### Gate and mutation evidence

The supplied Linux pool gate names the exact head and base.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-r87-parked-work-6c231801-pool-20261011-160553-28432.log`.
SHA256: `1c56adf3097448147ea666f5fd969b1f6efe90e8ecf6e225b9d473531e1863fb`.
All ten steps pass. The wrapper exits 0 after 714 seconds on msa1.
The job queues for 277 seconds and runs for 436 seconds. Total step time is 425.8 seconds.
The default run has 1795 passes and 460 skips in 18.090 seconds.
The slow run has 585 passes and 2022 skips in 45.374 seconds.
The named scenario passes on both tiers, including the real-process test.

Stage 1 examines 48 mutants: 30 caught, eight missed, zero timeout, and ten unviable, in 111.6 seconds.
The stage-2 baseline passes all 585 tests, with 2732 skips, in 43.080 seconds.
The baseline step takes 58.5 seconds. Stage 2 examines the exact eight misses.
Stage 2 reports six caught, zero missed, zero timeout, and two unviable, in 114.9 seconds.
The combined result is 48 mutants: 36 caught, zero missed, zero timeout, and twelve unviable.
I independently compare all raw catches against all citations: both sets contain the same 36 unique mutant identities.
Every cited test has a normal PASS before mutation. The log contains no FLAKY record.
The new Work::Op and Work::Parked tests catch their deleted arms.
The new record tests catch the stale record and constant-false decisions without waiting for the scenario bound.
Fuzz reports no changed crate with a decoder harness. No fuzz execution is claimed.

The earlier 98fe6fde gate remains RED. Stage 1 reports 25 caught, ten missed, three timeout, and ten unviable.
The timeouts are parked_of returning a default record, ParkedWork::record doing nothing, and ParkedWork::parked returning false.
Stage 2 and fuzz do not run in that earlier gate. The current gate completes their remaining mutation conditions.
The body preserves that red gate and the earlier stale-base lists failure.

### Local evidence limit

The per-seed file contains exactly 32 distinct passing seeds for each fenced ID, with no INCONCLUSIVE record.
The file names an earlier uncommitted implementation and gives no target-directory or clean record.
SHA256: `b1eb1358ff0e328d7f7132da108a9a9eeff0e1b4c0a4a434f2daff534c311720`.
The later Mac-tier log names source `9e5f165a6a3d6389e965a0af082dffb55a086183` and a dedicated p5-r87 target.
The accompanying script states that this target was new and private to that script.
It patches only the two fenced transcripts onto contracts .39 and records their exact matching #68 blobs.
It reports PASS for both IDs on testkit seeds 0–31 and real seed 0.
The host decisions and process-control implementation match the current head byte for byte.
This evidence precedes later tests and base imports. It is not an exact-current-head conformance run with the future pin.

I read the engine reversal record and the local mutant-fix record.
The engine record preserves its initial runnable-work miss and the later test that catches it.
The local record shows prompt assertion failures in the new record tests, while the full scenario still reaches its 30-second bound.
I do not claim that the complete local mutant run takes zero time.
The independent pool result supplies the current-head mutation evidence.

The B13 rule rejects local evidence from a shared target unless a clean or single-worktree condition holds.
I requested cache and source provenance for the earlier per-seed and reversal records.
Question receipt: `msg_plugin-w_1791760963_1e43b1`. The implementer's answer remains pending.
I do not accept those earlier local records as validated execution evidence until that condition is established.
The open source and body findings already prevent CLEAN on this head.

I changed no product code. I ran no build, test, gate, mutation job, or executable merge check.
I used read-only Git, source inspection, and artifact parsing.
The next head requires a delta review. The lead controls the final slot, base import, final gate, and merge.

VERDICT: NOT CLEAN


## Round 2 — source corrections and evidence limits, 2026-10-11

- Exact head: `bda64e1592e33aa5eedfc8f39d2b114bd0a06b4f`.
- Tree: `4b77363def4624dd2d3584d77fb4821464933756`.
- Base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- Previous head: `6c231801e78ea268dd2894d669a41aa3c1392615`.
- Previous verdict: `6b1ec1d1c01db123c9e92de43fb2dca53e888f8e`.
- Fix and provenance receipt: `msg_plugin-w_1791761164_574b97`.
- Tier: HIGH. Open findings: two MEDIUM findings after the subsequent checks below.

### Closed source and body findings

I read the complete two-commit delta. It changes only real.rs and the shared scenario in refusal.rs.
`d0493f3e3e38a6575adf7e56a886ebabe5248741` places the alive comment directly above worker_alive. F1 closes.
The exact head adds an explicit deadline check to the setup loop. The loop reuses the unchanged BOUND.
A pump that continues to report more reaches the assertion when that deadline expires. F2 closes.
The loop adds no poll, sleep, new timeout value, or production change.
The body now cites HIGH rules 3 and 5. The three named host paths occur in the HIGH-path list. F3 closes.
The two controls and the host diagnostic remain unchanged.
The previous-head green gate does not validate this new test code. No exact-head gate is accepted yet.

### Provenance correction

P5 states that the initial per-seed record is superseded.
Its uncommitted source identity rests only on the implementer's statement. It has no target or clean record.
P5 also cannot establish the cache condition for the forms reversal or engine reversal records.
A production source edit does not replace the B13 clean or single-worktree proof.
I withdraw any validation of those three execution records. Their observed log text remains historical evidence only.

The Mac-tier record supersedes the first per-seed record.
It names clean source `9e5f165a6a3d6389e965a0af082dffb55a086183` and a new private target directory.
P5 confirms that this script was the first user of that directory. Its script names the same condition.
The record runs both fenced IDs with testkit seeds 0–31 and real seed 0.
It patches only two #68 transcript blobs onto .39, and records the matching blob identities.
I accept this Mac result with its stated source and transcript scope.
The later isolated pool gate supplies the current implementation's independent mutation result.
The local mutant-fix record follows a clean and has one worktree writer, according to P5's receipt.
I need no local reversal claim to accept the independent pool mutation result.

I read the complete provenance comparison and its script.
I independently verify the diff from 9e5f165a to dc9b230b.
The host driver, engine, and run blobs match exactly, as do harness.rs and process_controls.rs.
The only real.rs addition is the eleven-line slow test. The only refusal.rs addition is the cfg(test) shared scenario.
The two source patch IDs differ because of those added tests. I claim no complete-tree equality from the patch IDs.
Later tests, comments, the deadline check, and the disjoint base import have explicit review.
The historical Mac result is not a full gate on the new head or the future contracts pin.

Mac log SHA256: `87c4da324f9bddce3b223ac922e50141a49c964690612b7bfcd37098bdc68204`.
Provenance record SHA256: `57641464e1f3b57125c0a7726c93ee6e51a2e1fe3fc3bcb04c537cf3b3c34719`.

### R292-E1 — MEDIUM, open: the body accepts unvalidated local records

The live Evidence table still presents the initial per-seed and both reversal logs as accepted execution proof.
That claim conflicts with the implementer's provenance correction.
Required change: label these three records historical and unvalidated, or remove their accepted proof claims.
Use the valid Mac-tier result for the 32-seed and real-tier claim.
Use the isolated pool result for the mutation claim. Link r87-evidence-provenance.txt and preserve each source limit.
No new execution job is required for this body correction.
Sent to P5 and Sol: `msg_plugin-w_1791761262_cecc9a` and `msg_plugin-w_1791761262_1d6254`.

### Subsequent checks on the same head

P5's receipt `msg_plugin-w_1791761305_136adb` corrects the three unvalidated-record labels.
I verify those labels in the published body. The dedicated-target Mac and isolated pool citations remain valid in their stated scope.
E1 remains open for two statements in the corrected Evidence section:

- The newly validated R-88 group-A log uses targets/p5 in both its script and header.
  It is a local Mac run, not a pool snapshot or fresh private target. Its cache condition is not yet established.
  Required change: supply the clean or single-worktree condition, or mark that record unvalidated. No new run is required.
- The body says that 9e5f165a and this PR have five identical implementation blobs.
  The current harness.rs contains 25 added test lines. That complete blob does not match 9e5f165a.
  Required change: name dc9b230b for the exact five-blob comparison, then distinguish the later test additions.

I send these remaining conditions directly to P5 and Sol in `msg_plugin-w_1791761397_272f5f` and `msg_plugin-w_1791761397_ac14f9`.

### R292-SI1 — MEDIUM, open: AdoptAll work has no session in the diagnostic

Sol identifies this finding in receipt `msg_plugin-w_1791761326_9cd4bd`. I independently confirm it from the complete relevant source.
AdoptAll admission stores p.session as None (admit.rs:750–755).
Next::AdoptRow requires mandatory room (run.rs:86–88).
ready_at offers that op with room and omits it without room, unless its row waits for another condition.
parked_on_room maps Work::Op only through p.session (engine.rs:624).
The op therefore contributes no session to the diagnostic, even when its row step waits only for room.

adopt_next_row reads the front row key and passes its SessionId to adopt_row (run.rs:694–700).
For a session already shown by this handle, adopt_row posts that session's state (run.rs:721–727).
With a full queue, this known session has room-blocked work. Its parked_work control nevertheless reports false.
This contradicts corrected R-87's read for all work waiting only for room. The existing DetachLocal test does not exercise AdoptAll.

Required change: map the active AdoptRow key to its session when p.session is None.
Retain ready_at's row_waits_for_its_session exclusion for a row waiting on its Create.
Add the known-row regression with a full queue. Prove that the session is parked, then not parked after a poll frees room.
My confirmation goes directly to P5 and Sol in `msg_plugin-w_1791761397_d50441` and `msg_plugin-w_1791761397_400278`.
The green previous-head gate does not close this logic finding.

I ran no build, test, gate, mutation job, or executable merge check. I changed no product code.
Both the source repair and remaining evidence corrections need verification before CLEAN. The lead retains control of the final slot and merge.

VERDICT: NOT CLEAN


## Round 3 — corrected AdoptAll mapping, 2026-10-11

- Exact head: `2e03346020a63cdc9c069a638170e95cb1416177`.
- Tree: `344d20332dc8b81fbc782f77cf038b8d6623a558`.
- Base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- Previous head: `bda64e1592e33aa5eedfc8f39d2b114bd0a06b4f`.
- Previous verdict: `8bc8779a0b1fb9384966436ba4fab17678502ad7`, Round 2.
- Fix receipt: `msg_plugin-w_1791761470_903cc6`.
- Provenance receipt: `msg_plugin-w_1791761553_ea226d`.
- Tier: HIGH. Open review findings: zero. Result: source CLEAN; final gate remains required.

### R292-SI1 closure

I read the complete one-commit delta. It changes only engine.rs and queue_pressure.rs.
For room-blocked Work::Op, the read first uses p.session.
If that field is None, the read uses the front row's key after stripping ROW_PREFIX.
This is the same row-key interpretation that adopt_next_row and row_waits_for_its_session use.
The filter still starts from ready_at(true) minus ready_at(false).
ready_at and its row_waits_for_its_session exclusion remain unchanged.
A row waiting for its Create therefore stays excluded. The fallback reports the AdoptRow work that waits only for room.
The change affects the diagnostic read only. It changes no admission, scheduler decision, queue operation, or wake action.

The new regression creates and shows s1, fills the queue through s2, then begins AdoptAll.
The test requires the parked set to contain s1 while its row state waits for room.
After a poll frees room, the test requires an empty set.
This scenario proves the missing global-op case. It complements the existing DetachLocal, state, route, and held-frame tests.
I read the supplied reversal log. Without the mapping, the new test fails with an empty set instead of s1.
The outcome matches the independently inspected source defect.
Reversal log: `shared/core-stage1/evidence/p5-r87/r292-si1-revert.log`.
SHA256: `43482ccfb9c21371dbcdfb850fd854b0bbb6a57f1dbb6a7ec60f9946fadfb715`.

P5 reports 299 core-host lib tests passing, clippy with zero errors, and taint PASS after the repair.
P5 states that both the reversal and passing run follow cargo clean of targets/p5: 11261 files, 3.6 GiB.
Until both runs complete, only the same Core worktree path writes that target. Branch changes occur within that path.
Both runs use core-host lib tests only. No prebuilt worker participates.
That owner statement establishes the B13 clean and single-worktree condition for these runs.
The clean output remains in the session transcript, not a retained file. I record that limit and perform no independent execution.
The final isolated gate must validate the exact final source and all in-diff mutants.
R292-SI1 closes for source and named regression scope.

### R292-E1 closure

I verify the complete live body at the exact head.
The body now labels four local records historical and unvalidated: the initial per-seed record, both reversal records, and R-88 group A.
The body says that these records are not proof. It links the provenance record and keeps the R-88 failures visible in that record.
The group-A raw file retains eight PASS outcomes, two behavior failures, and four unsupported-control failures.
I claim no validated group-A result or cause from that unvalidated run.

The exact five-blob comparison now names 9e5f165a and dc9b230b.
The body distinguishes later test additions, comments, the deadline check, and the AdoptRow mapping.
It states that the 6c231801 gate covers the earlier tests and that the final gate must cover the later changes.
The dedicated-target Mac proof retains its earlier source, patched-transcript scope, and seed values.
The pool gate retains its exact earlier head and combined mutation result.
The corrected body does not claim a completed gate on this head. R292-E1 closes.
F1, F2, and F3 remain closed through the unchanged comment, deadline, and risk corrections.

The exact-head full-index own diff SHA256 is `03f7b843f452ab4f09e87b733dc14395c28b4e2f96a62a62abc136f9f2f9493e`.
`git diff --check` passes against the base.
No pending list, transcript, timeout value, dependency pin, mutation exclusion, or gate selector changes.
The am_4 deviation remains under the lead's existing decision.

### Final conditions

This verdict accepts the reviewed source and corrected body. It accepts no later head or merge tree.
The gate on 6c231801 remains previous-head evidence. It does not validate the later production change or test deadline.
P5 confirms the next sequence: both source CLEANs, FINAL-READY, lead SLOT, base import and check, then one final gate.
The exact-final-head gate and mutation result remain required before merge.
Sol's exact-head integration verdict remains separate. The lead controls the slot and merge.

All review findings close. I changed no product code.
I ran no build, test, gate, mutation job, or executable merge check.
I used read-only Git, source inspection, and artifact parsing.

VERDICT: CLEAN


## Round 4 — final-slot base import, 2026-10-11

- Exact head: `1ed69c709da7f75a7f90895ae36675f3e8702d84`.
- Tree: `8d370b057c8b25dcb05a8d31621b6db46e3066fc`.
- Base: `1b984f06f11378e1a424db75dfe99b1e2d15c567`.
- Parents: `2e03346020a63cdc9c069a638170e95cb1416177` and the base.
- Previous head: `2e03346020a63cdc9c069a638170e95cb1416177`.
- Previous base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- Package source CLEAN: `f79e822f5e3407dbcd4f1140b96b5a5f3c833740`.
- Sol source CLEAN: `5bfb2bbee7e98ead84fdd3fea7f2d4c31b72c266`.
- Delta request: `msg_plugin-w_1791767479_7ae87c`.
- Tier: HIGH. Open review findings: zero. Result: source delta CLEAN; final gate remains required.

### Canonical check and independent composition

The lead assigns the slot after #296 and records the current base above.
I read the complete canonical base check in both retained files, including every PASS, FAIL, and warning.
Both ancestry checks pass. Condition 1 reports no merge conflict. Condition 4 passes both pending files.
Condition 2 fails because real.rs overlaps the base's changes. Condition 3 first differs at that file's index line.
This is a retained FAIL. It requires both reviewer delta rounds; no automatic carryover is claimed.

Report: `shared/core-stage1/evidence/p5-r87/base-merge-check-292.txt`.
SHA256: `00a9e3818f9cd4380f28c650aa9a074fc341cc02659e52763240c942c0e97ad8`.
The report includes the canonical output and the implementer's source supplement.

The own delta contains nine paths. The base changes thirty-three paths. Only real.rs overlaps.
Eight other own blobs equal the previous reviewed head exactly.
Both the own and imported added/deleted line sequences of real.rs survive the merge exactly.
The own real.rs line-sequence SHA256 is `4caf4c171381e5b23d17b1d25be45a23550988e337017c8ca0f00c69de0a6e33`.
All nine complete own patches match after removing only index and hunk metadata.
The stable patch ID is `55757a231397995106230f4906106615726e8a5d` on both sides. I compute it independently.
All 342 final tree entries equal the base plus the verified own delta.
All nine conformance text files equal the base exactly. This PR changes no ID or list record.
The final own diff SHA256 is `032cfa0e9a9e24446b540762a0fadc7747a61611c2b5bf39ee2ad6b00ee4d8c3`.
`git diff --check` passes. These static checks supply no executable base-check PASS.

### Source interaction

I read the complete imported real.rs delta and the final own delta in that file.
The ParkedWork record still wraps the same crash slot, capture layer, and host driver.
The inserted field, initialization, control method, dispatch, and real-process scenario keep their reviewed placement and behavior.
The worker_alive comment stays attached to worker_alive. The scenario's deadline and AdoptRow regression survive unchanged.
The imported controls use distinct names. They do not replace parked_work or its record.
The hold controls change tapped edges; the host diagnostic still counts engine work and frames held by the driver for room.
A tap-held start or handoff is not automatically counted as room-blocked host work.
The imported oracle controls use stored rows and a separate oracle terminal. They do not mutate the host's parked-work record.
The route callbacks keep weak tap references. ParkedWork retains session IDs, not Core or its directory claim.
The real wake descriptor and its crash check retain their reviewed behavior.
The imported fake-handle test now supplies its own wake field. That test does not change production descriptor ownership.

I read the imported edge-tap decisions and the oracle-control lookup relevant to these interactions.
The host engine, run logic, record layer, and testkit harness have no base overlap.
The imported mutation exclusions concern xtask process glue. They exclude no new R-87 function.
I read the latest final integration records for #287, #288, and #289.
Those base PRs retain separate reviews and evidence limits. This delta review does not replace their acceptance records.

The base imports contracts-v0.1.41 at `2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87`.
All seven manifest tags and nine lock sources match the base exactly.
The Core contract lib.rs file is unchanged between .39 and .41.
The .41 EV-5 transcript still has its strict wake step and no parked_work fence.
The future .42 pin supplies the R-87 fence. This PR introduces no transcript or pin move of its own.

### Retained local static evidence

Log: `shared/core-stage1/evidence/p5-r87/r292-slot-checks.log`.
SHA256: `d94a86856c15e3eda3e67d63df745f9c6e4fdb16f3da991aed543e6b90687243`.
The log records cargo clean: 2139 files, 839.4 MiB, then zero target entries.
It names the exact head, both parents, the base, a clean status, and one slot worktree writer.
This record establishes the B13 condition for its local checks.
Fmt is clean. Lists and taint pass. The clippy command finishes with no reported errors.
The log reports 299 core-host and 421 testkit unit passes, plus its additional passing test and documentation summaries.
The log ends with a clean status. These local checks do not replace the final pool gate or mutation result.

### R292-D1 — LOW, closed

The initial body omitted the final slot merge and still named .39 as the current pin.
P5 adds the exact head/base, real.rs overlap, canonical FAIL, preserved delta, and linked check records.
P5 limits the earlier green gate to 6c231801 and marks the final gate RUNNING.
P5 correctly states that .41 is now imported and that .39 belongs to historical evidence.
Fix receipts: `msg_plugin-w_1791767592_2a08c7` and `msg_plugin-w_1791767609_c7f931`.
I verify those corrections in the complete published body at the unchanged head.

The corrected body initially said that condition 3 differs only at the index line.
The check reports that first difference. Ten real.rs hunk headers also change their line offsets.
The complete patch content remains unchanged after removing those metadata lines.
I required the body to distinguish the first index difference from the remaining hunk offsets and retain the canonical FAIL.
I sent the condition directly to P5 and Sol in `msg_plugin-w_1791767787_4cb95d` and `msg_plugin-w_1791767787_6b89f6`.

P5 corrects that final condition in `msg_plugin-w_1791767804_613011`.
I read the complete updated body and verify the unchanged exact head. The body now states both metadata differences.
R292-D1 closes. All source and body findings close before this formal delta verdict.

I read complete Sol Round 4 at `dc1f3bf80fb5a05a08b7644bd4c2daaeb47701de`.
Its source delta CLEAN matches this head, base, tree, exact composition, and retained canonical FAIL.
It requires the matching package delta and completed final evidence. This record supplies the package delta.

The exact-head final gate is running by owner report. I read no running gate output and accept no result from it.
The completed final gate and mutation result remain required. Sol's exact-head import verdict remains separate.
I changed no product code. I ran no build, test, gate, mutation job, or executable merge check.
I used read-only Git, source inspection, and artifact parsing.

VERDICT: CLEAN
