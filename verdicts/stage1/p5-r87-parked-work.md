# #292 R-87 integration review

## Round 1 — source, fixes, and evidence, 2026-10-11

READY head: `6c231801e78ea268dd2894d669a41aa3c1392615`.
READY tree: `8edac0b57c7913ed5576dce56c280f71ac861aca`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Scoped corrected head: `bda64e1592e33aa5eedfc8f39d2b114bd0a06b4f`.
Corrected tree: `4b77363def4624dd2d3584d77fb4821464933756`.
Risk HIGH. Plan 23cm governs. Sol reviews all nine changed paths and complete source deltas through bda64e15.
Sol reads corrected R-87 at contracts `7c4004a0ba6c9734fbe8d2072f68addde2d58596`.
The control must report both engine work and held frames that wait only for mandatory room.
The read must exclude runnable work and work that waits for another condition.
Sol reads full package Round 1 at `6b1ec1d1c01db123c9e92de43fb2dca53e888f8e`, `verdicts/p5-r87-parked-work.md`.

### Import and production trace

The lead-approved 98fe6fde merge imports base 5e953398 into the branch cut at f68f26a7.
Sol independently checks nine own paths and seven imported paths: no overlap, and no own blob changes in that import.
These static Git checks supply no executable base-check result or final-slot acceptance.
The extracted ready_at body retains every original decision; ready supplies has_room as before.
The extracted accepts_at body retains the original input conditions; can_accept supplies has_room as before.
Those extractions preserve production ready and acceptance outcomes.

The driver adds a diagnostic set from engine work and its own held frames.
The engine compares ready_at(true) with ready_at(false), then maps the difference to sessions.
It excludes offered work under both conditions. It also compares held-frame acceptance with and without room.
Observed frames that wait for Start or Adopt remain excluded. A queue with room returns an empty set.
The diagnostic performs no pump, poll, signal, or read-interest change.
The Work::Op mapping has the missing AdoptRow case below.

RefusalLayer records diagnostics at construction and after forwarded calls that can change Core state.
Both harnesses retain a separate shared record for each handle. Both controls validate the handle and session row.
The control reads the record without calling Core. Scripted refusals leave Core and its record unchanged.
The plain real harness still refuses test controls. The tapped real harness registers parked_work.
The scenario checks a running session, parked Stop, unrelated session, freed room, and absent row on both tiers.
The engine tests cover session work, DetachLocal op work, parked route events, and held frames, with negative cases.
They do not cover an AdoptAll row step with no pending-operation session.

### R292-SI-1 — MEDIUM, open: AdoptRow work has no session in the diagnostic

Location: engine.rs `parked_on_room`, Work::Op arm.
The new mapping uses only PendingOp.session. AdoptAll admission stores that field as None in admit.rs.
inbound.rs `adopt_rows` supplies the row queue and sets Next::AdoptRow.
run.rs `step_needs_room` requires room for that step. ready_at(true) offers it; ready_at(false) excludes it.
The diagnostic therefore identifies the parked op but inserts no SessionId.
run.rs `adopt_next_row` reads the front row key and calls adopt_row for that session.
An already-shown session of this handle still needs that step to post its recorded state under LC-11.
That session can wait only for mandatory room while parked_work falsely answers false.
This defect exists on both reviewed heads. The current tests cover DetachLocal in the same arm and miss this different mapping.

Required change: map an active AdoptRow step to its front row's SessionId.
Add a regression with an already-shown row and a full mandatory queue; verify the diagnostic before and after room is freed.
Retain row_waits_for_its_session and other non-room exclusions. Add no timer, limit, or private transcript assertion.
Sent to P5 and the package reviewer: messages 1791761326_707c73 and 1791761326_9cd4bd.

### Package findings and scoped fixes

Sol independently confirms R292-F1: real.rs attaches worker_alive's AD-7/R-46 comment to parked_work.
d0493f3e moves those lines directly above worker_alive. The complete delta closes that LOW source finding.
Sol independently confirms R292-F2: the scenario's Create setup uses an unbounded `while core.pump(now()).more {}`.
bda64e15 checks an explicit deadline inside that loop using the existing 30-second BOUND.
The loop still pumps without polling. The complete delta closes that MEDIUM source finding without changing BOUND.
The body corrects its HIGH citations to rules 3 and 5, closing the package's LOW risk-citation finding.
The completed 6c231801 gate does not cover these later source fixes.

### Completed READY-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-r87-parked-work-6c231801-pool-20261011-160553-28432.log`.
SHA-256: `1c56adf3097448147ea666f5fd969b1f6efe90e8ecf6e225b9d473531e1863fb`.
Sol reads exact head/base headers, tier results, mutation outcomes and citations, named scenario results, and completed exits.
All ten steps PASS in 425.8 seconds. The msa1 job exits zero after 713 seconds: 277 queued, 436 running.
The wrapper exits zero after 714 seconds.
Default: 1795 PASS, 460 skipped, 18.090 seconds. Slow: 585 PASS, 2022 skipped, 45.374 seconds.
The named parked_work scenario passes in both normal tiers.
The unmutated slow baseline has 585 PASS, ten slow cases, and 2732 skipped, in 43.080 seconds; its interval is 58.5 seconds.
Stage 1: 48 mutants, 30 caught, eight missed, zero timeout, ten unviable, 111.6 seconds.
Stage 2: eight mutants, six caught, zero missed, zero timeout, two unviable, 114.9 seconds.
Sol independently matches all eight second-stage identities to all eight first-stage misses.
Sol independently matches all 36 unique raw catches to all 36 unique citations. Every catcher has a normal PASS before mutation.
The total is 36 caught, including six from stage 2, and 12 unviable. Unviable results supply no catch.
The gate contains no FLAKY line. Fuzz reports no changed crate with a decoder harness.
The earlier RED gates remain recorded. The current result supplies no later-head gate or future-pin acceptance.

### Evidence provenance and body findings

Sol reads the full dedicated-target Mac log and script, plus the provenance file and its static comparison script.
Mac log SHA-256: `87c4da324f9bddce3b223ac922e50141a49c964690612b7bfcd37098bdc68204`.
Script SHA-256: `e549980f24edf1c6b51258d029373ec9e0750ac959076428ecbb3bb5f4df8ba6`.
Provenance record SHA-256: `57641464e1f3b57125c0a7726c93ee6e51a2e1fe3fc3bcb04c537cf3b3c34719`.
P5's provenance script is read only; Sol does not execute it. P5 supplies receipt 1791761305_0b45a3.
The Mac record names clean Core 9e5f165a, contracts .39 with two changed transcripts, and target directory targets/p5-r87.
The script and owner state that this directory was new and private to that script.
The record reports PASS for each fenced identifier on testkit seeds 0–31 and real seed 0 after prebuild-worker.
Its two transcript blob identifiers match the recorded contracts #68 blobs.
Sol independently compares 9e5f165a with dc9b230b: five implementation blobs match; only the shared scenario and its slow test are added.
Later tests and imported base changes limit this record to unchanged implementation, not an exact-current-head future-pin run.
Filtered pipeline results retain named test outcomes; no recorded aggregate exit is inferred beyond those outcomes.

The first per-seed record lacks a proved uncommitted tree and target/clean state.
The two earlier reversal records lack retained target/cache provenance.
P5 marks all three historical and unvalidated. The updated body makes that correction explicit.
The isolated pool gate supplies the current mutation result. Sol relies on it rather than unvalidated local reversals.
The later local mutant-fix clean and sole-writer condition are owner-reported. Sol grants no extra proof from that local record.

At this read, GitHub names bda64e15 against 5e953398.
Raw body SHA-256: `b2baf88991375b3eb91ed586606c78e22c3bdcda0a52d6c21202fff6d2377555`.

R292-SI-2 — MEDIUM, open: the revised body adds r88-groupA-tiers-mac.log under Validated records without its cache condition.
Sol reads the complete log and script. Both name targets/p5, with no retained clean or single-worktree record.
The later 11261-file clean before mutant-fix tests does not establish the earlier group-A cache state.
The group-A log retains eight PASS and six failed results across seven identifiers and both tiers.
Its two testkit failures and four unsupported-control failures remain actual failed results.
Required change: qualify this record as historical/unvalidated unless retained evidence establishes the B13 cache condition.
Retain its failures and its earlier-source/patched-transcript scope. No new run is requested.

R292-SI-3 — LOW, open: the body says 9e5f165a and this PR have the same harness.rs blob.
Sol independently compares them: the current harness.rs adds 25 test lines from 76bf7467.
Required change: limit the literal five-blob claim to 9e5f165a versus dc9b230b, then state the later test-only differences.
This precision issue does not establish changed control semantics.
Both body findings are sent to P5 and the package reviewer in messages 1791761408_67cf4f and 1791761408_6962bb.

Core remains pinned to .39. This PR changes no pending list or transcript and grants no unpending decision.
The lead keeps am_4 live under option (b) until its separate R-88 fence.
The tag carrying #68, required pin review, exact final slot, final gate, and later acceptance remain separate obligations.
Sol changes no product code and runs no build, test, gate, mutation, reversal, measurement, diagnostic, or executable base check.
Never poll. Wait for the doorbell.

VERDICT: NOT CLEAN (3 open: R292-SI-1 MEDIUM source, R292-SI-2 MEDIUM evidence body, R292-SI-3 LOW body) at bda64e1592e33aa5eedfc8f39d2b114bd0a06b4f against base 5e953398925351c12e463a7b0a946b79d9841edc

## Round 2 — AdoptRow repair and body corrections

Exact head: `2e03346020a63cdc9c069a638170e95cb1416177`.
Base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Tree: `344d20332dc8b81fbc782f77cf038b8d6623a558`.
Parent: bda64e15. Owner receipt: `msg_plugin-w_1791761470_b5c4eb`.
Sol reads the complete two-file source delta and the complete reported body delta.

R292-SI-1 closes at source level.
The Work::Op arm retains p.session when present. Otherwise, it decodes the front row's ROW_PREFIX key into SessionId.
Only work offered with room and refused without room enters that arm, as before.
Of the room-bound op steps, AdoptRow has no session; DetachLocal retains its explicit session.
ready_at and row_waits_for_its_session remain unchanged. The diagnostic does not count a row waiting for its Create to be shown.
The new regression creates and shows s1, fills the mandatory queue, then starts AdoptAll.
It checks the diagnostic set equals {s1}, then becomes empty after a poll frees room.
This setup exercises the missing producer-to-session mapping without changing production scheduling or a transcript assertion.

The retained reversal log shows this regression failing against the prior mapping: actual {}, expected {SessionId("s1")}.
SHA-256: `43482ccfb9c21371dbcdfb850fd854b0bbb6a57f1dbb6a7ec60f9946fadfb715`.
It records the bda64e15 source label but no complete command, target/cache condition, or clean header.
Sol records its observed failure and owner attribution. Sol grants no independent validated local execution claim from that filtered log.
The source fix and regression are fully reviewed; their completed exact-head gate remains due in the lead's final slot.

R292-SI-2 and R292-SI-3 close.
The body moves group-A evidence into the historical/unvalidated list and states its absent cache condition.
It links the retained raw log, which preserves all six failures. It claims no execution proof from that log.
The literal five-blob comparison now names 9e5f165a and dc9b230b.
The body separately names later tests, comments, deadline check, and AdoptRow mapping.
It limits the completed 6c231801 gate to its actual source and leaves later changes for the final gate.
The sentence about the final gate is a future obligation; no completed final gate is supplied or accepted here.
GitHub confirms this exact head/base. Raw body SHA-256: `01e48a55251600625701c80963676f3a28ec90627bef074769250e86335c5285`.

Sol reads complete package Round 2 at `8bc8779a0b1fb9384966436ba4fab17678502ad7`.
It independently confirms the original AdoptRow finding and remaining body conditions on bda64e15.
Its formal repaired-head review remains due. No package receipt on the earlier head automatically accepts this repair.
No integration source or body finding remains open on 2e033460.
Matching package acceptance and exact-head/final-slot execution remain due; this round grants no final merge approval.
Sol runs no execution job. Never poll.

VERDICT: NOT CLEAN (1 open acceptance condition: matching repaired-head package review and completed final-slot gate) at 2e03346020a63cdc9c069a638170e95cb1416177 against base 5e953398925351c12e463a7b0a946b79d9841edc; all three integration findings are closed

## Round 3 — matching source acceptance

Exact head, base, and tree remain those in Round 2.
Sol reads complete package Round 3 at `f79e822f5e3407dbcd4f1140b96b5a5f3c833740`.
Its source CLEAN covers the same repair, regression, corrected body, and deferred final gate.
The package relays P5 receipt 1791761553_ea226d for the local regression runs.
P5 states that the reversal and 299-test lib run follow a targets/p5 clean of 11261 files, 3.6 GiB.
Only the same Core worktree path writes that target until both runs complete; branch changes occur within that path.
No candidate worker participates. The clean output remains in the session transcript, not a retained file.
This owner statement establishes B13's condition, with that provenance limit. Sol claims no independent clean verification.
The retained named regression failure and reviewed source fix agree. The earlier unvalidated records remain unvalidated.

Matching source acceptance is complete. All source and body findings are closed, including LOW.
This source CLEAN permits the lead's FINAL-READY sequence. It does not grant final-slot or merge acceptance.
The lead must assign SLOT, import current v1, obtain the applicable check and delta reviews, and complete one exact-final-head gate.
That gate and its mutation result must cover the later fixes. The 6c231801 gate supplies no carryover for those changes.
Sol runs no execution job. Never poll.

VERDICT: CLEAN (0 open source/body findings) at 2e03346020a63cdc9c069a638170e95cb1416177 against base 5e953398925351c12e463a7b0a946b79d9841edc; final-slot acceptance remains due

## Round 4 — final-slot source import

Exact head: `1ed69c709da7f75a7f90895ae36675f3e8702d84`.
Reviewed base: `1b984f06f11378e1a424db75dfe99b1e2d15c567`.
Tree: `8d370b057c8b25dcb05a8d31621b6db46e3066fc`.
Sol verifies both merge parents: previously reviewed 2e033460 and imported v1 1b984f06.
P5 supplies the SLOT delta in `msg_plugin-w_1791767479_6dc6ea`.
The earlier complete source review covers the unchanged own implementation and tests.

### Canonical result and independent source comparison

Sol reads the complete retained canonical report and its source-comparison supplement.
Path: `~/botster-sessions/shared/core-stage1/evidence/p5-r87/base-merge-check-292.txt`.
SHA-256: `00a9e3818f9cd4380f28c650aa9a074fc341cc02659e52763240c942c0e97ad8`.
Ancestry and condition 1 PASS. Conditions 2 and 3 FAIL because real.rs changes in both branches.
Both pending-file condition-4 checks PASS. The report correctly requires both reviewers' explicit delta reviews.
Sol runs no executable base check and does not change this retained FAIL into PASS.

Sol independently compares every complete own patch before and after the import.
All nine patches match byte for byte after removal of only blob-index and hunk-location metadata.
This includes the entire real.rs patch, not only its added lines.
Every imported non-own blob matches the imported base.
All nine conformance status files match the imported base byte for byte, including both pending files and the held file.
Thus this import changes no identifier, class, owner, pending status, or named real proof through #292's own diff.
The ledger retains 691 identifiers; pending and real-pending retain 412 and 44 respectively.
The previously repaired AdoptRow mapping and its complete regression remain unchanged.

### real.rs composition

Sol reads the complete real.rs import delta and the current open, wrapper, control, and diagnostic-recording context.
The imported route controls, start and handoff holds, oracle controls, and wake test retain their base implementation.
The own parked_work control uses a distinct dispatch name and still validates the session through its recorded row.
Its answer comes from the per-handle ParkedWork record, without calling the Core or making progress.
The tapped open path still creates the same driver, capture layer, and crash slot.
It then wraps that slot with RefusalLayer and shares a fresh ParkedWork record with the harness.
The imported wake descriptor remains on Handle. The wrapper still delegates wake_handle and next_deadline to its inner Core.
The layer records diagnostics after each applicable Core call and returns that call's original result.
The imported route controls use the same weak tap and separate request path. They do not replace the parked-work record.
The plain harness still serves no parked_work control.
No source defect arises from this composition. Sol establishes zero new source findings on this head.

### Local static evidence

Log: `~/botster-sessions/shared/core-stage1/evidence/p5-r87/r292-slot-checks.log`.
SHA-256: `d94a86856c15e3eda3e67d63df745f9c6e4fdb16f3da991aed543e6b90687243`.
Sol first reads the completed static prefix and later reads the completed unit-test section after P5 reports its completion.
Sol reads no running unit result or final-gate output.
The log records exact head and parents, clean worktree, and cargo clean removing 2139 files and 839.4 MiB.
It records zero target entries after the clean and names the sole SLOT worktree writer through the run.
Fmt and lists report success. Taint PASS takes 22.6 seconds. Clippy reports zero errors.
The completed unit summaries include 299 host tests and 421 testkit tests, with zero failures.
The record ends with a timestamp and clean worktree status.
These results supply their stated local scope. They do not replace the completed exact-head final gate.

### R292-SI-4 LOW closes: current body scope

The original slot-body snapshot still names no SLOT import and says that Core pins .39.
Its SHA-256 is `01e48a55251600625701c80963676f3a28ec90627bef074769250e86335c5285`.
Sol sends the correction as R292-SI-4 LOW. The package reviewer independently reports matching R292-D1.
P5 corrects it in `msg_plugin-w_1791767592_e5ed6b`.
Sol reads the complete reported body delta and confirms exact GitHub head 1ed69c70 and base 1b984f06.
Current body SHA-256: `a5d3abb56ffe51d6f68f010e2bfbe24a007ce1aad0d770fdce816e750ddd8eef`.
The body adds both reviewed and current identities, the overlap, each canonical condition, source preservation, and local clean scope.
It limits the old gate and local records to their exact earlier heads and marks the final gate RUNNING.
It states the current .41 pin and retains .39 only for the earlier source scope. R292-SI-4 closes.

Sol independently verifies that both target .41 transcripts contain no parked_work fence.
Sol reads their complete .41-to-.42 fence deltas at tag target ec7075a4a3fee067323f24f02576777ccd1abea7.
Both .42 transcripts add the required parked_work waiting fence; am_4 adds its separate op_posted waiting fence.
This narrow comparison supports the body's tag distinction. It accepts no .42 Core pin, transcript run, or pending removal.
This PR changes no contract pin or pending list. The later op_posted Core implementation remains separate.
The earlier Mac, mutation, reversal, and group-A records retain all previously recorded source and cache limits.

The matching complete package delta verdict and completed exact-final-head gate remain due.
The final body, both final evidence receipts, and lead's Q7 check before merge also remain due.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job. Never poll.

VERDICT: CLEAN (0 open source delta or body findings; package delta and final evidence remain due) at 1ed69c709da7f75a7f90895ae36675f3e8702d84 against base 1b984f06f11378e1a424db75dfe99b1e2d15c567

### Round 4 supplement — R292-SI-4 LOW reopens for metadata precision

Package receipt `msg_plugin-w_1791767787_6b89f6` identifies one remaining precision issue in the corrected body.
The body says that condition 3 differs only at the real.rs index line.
That is the canonical check's first difference. Ten hunk headers also change their line offsets.
Sol independently verifies all ten changed headers and unchanged content after removal of both metadata types.
The source CLEAN remains valid. The statement about the only raw difference is false.
Sol's earlier body check missed that distinction; the earlier zero-body-finding conclusion is superseded here.
R292-SI-4 reopens only for this LOW precision issue, matching package R292-D1.
Required correction: name the index line as the first difference and name the remaining changed hunk offsets.
Retain the unchanged patch-content comparison and the canonical FAIL. No source change or new execution is required.
The current body correction, complete package delta verdict, and final evidence remain due.
Sol runs no execution job. Never poll.

VERDICT: NOT CLEAN (1 LOW body issue; 0 source delta findings) at 1ed69c709da7f75a7f90895ae36675f3e8702d84 against base 1b984f06f11378e1a424db75dfe99b1e2d15c567

### Round 4 supplement — metadata correction and matching package verdict

P5 supplies the corrected sentence in `msg_plugin-w_1791767932_2232fd`.
Sol reads the complete body delta and verifies the unchanged exact head and base through GitHub.
Current body SHA-256: `5438951fc87ea26eb07506994f6d9c3dc50034bb1d51952ac03eeaffa4015816`.
The body now names the first index difference and the remaining changed hunk offsets.
It retains unchanged content after removal of both metadata types and the canonical FAIL. R292-SI-4 closes.

Sol reads complete package Round 4 at `33cdd3c281f4d78579d82023a9d156dc0ddcea04`, verdicts/p5-r87-parked-work.md.
Its source delta CLEAN covers the exact head, base, tree, nine preserved patches, all status files, and corrected body.
Package receipt `msg_plugin-w_1791768016_ad5bef` also confirms the complete current body and closed metadata condition.
Both source delta reviews now have zero open source or body findings on 1ed69c70.
The completed exact-head final gate, mutation result, final body receipts, and Q7 before merge remain due.
Sol runs no execution job. Never poll.

VERDICT: CLEAN (0 open source delta or body findings; final evidence remains due) at 1ed69c709da7f75a7f90895ae36675f3e8702d84 against base 1b984f06f11378e1a424db75dfe99b1e2d15c567
