# Core #289 — contracts pin .39 to .41

## Round 1 — source and incomplete pin evidence, 2026-10-11

Head: `4e26982b4c8ed836e43baa30f18fbf8134db90d5`.
Base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Original source base: `23550811fb84c3c2cb066019716987575d7aecb2`.
Tree: `cf24c8f7398af76441ea3bdb275f3600f0545bc3`.
Risk tier: HIGH. The PR changes a contracts pin and the shared testkit.

### P289-SI-1 — MEDIUM, open: the required A5-4 comparison is absent

The lead's PART B ruling requires an A5-4 table against .39 on both tiers, with no unexplained PASS-to-non-PASS change.
The body supplies gates and pending counts but no complete before/after table or raw comparison records.
PC confirms that no table exists in `msg_plugin-w_1791755816_72dea7`.
Supply the complete table and retained raw records. Name the exact source, pin, tier, and relevant build provenance for each run.
Explain every changed result. Preserve the original failures and limits.
The current green gate does not establish this comparison.
PC reports planned serial pool jobs: before on this base at .39; after on this exact head at .41.
Sol accepts no future result from those planned jobs.

### P289-SI-2 — MEDIUM, open: the required repeated real proofs are absent

The lead's PART B ruling requires N>=5 real proofs in addition to the Linux gate for the held-write scope.
The body identifies only one-run pending results. PC confirms that no repeated-run record exists yet.
Supply the required retained real records for all six R-74 held-write identifiers.
State the source, pin, platform, seed selection, trial count, result, and build provenance.
Preserve inconclusive and failed trials. A normal PASS or caught mutant supplies no five-run proof.
PC plans five runs per tier for the four removals and every `pty_queued` identifier.
The planned two event-fence removals add coverage; this review imposes no new N>=5 requirement for those two identifiers.
Sol requests no duplicate execution and accepts no future result in this verdict.

### Source boundaries and pin contents

Sol reads the complete seven-file own delta, affected control context, complete body, and retained gate evidence.
The lead explicitly permits one early import after the stale-base lists failure.
Merge `d82ec32f6e22584f9f040b170355de5c20afb4f0` imports the current base into the previous pin source.
Only the two pending paths overlap the previous own seven-file delta and the base's ten-file delta.
Every tree entry outside those overlaps matches the previous own source or the new base, as applicable.
The only subsequent change modifies both pending files. Both earlier xtask fixes and #284's source remain intact.

All seven workspace contracts dependencies move together from .39 to .41.
All nine lock entries move to `2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87`, the exact .41 tag target.
All 223 locked packages retain their other fields. The lock changes no package version or dependency list.
The unchanged `ci/remote/job-common.sh` equals the .41 file byte for byte. Contracts BUILD.md is unchanged.
The body lists the skipped .40 contents and the .41 contents. It excludes #60 and #63 from this tag.

Sol checks the relevant contracts production changes and transcript changes against .39.
R-74 adds the probe's `Queued` request and report. The probe reads `FIONREAD` and sends that report instead of Ack.
The report changes no probe gate and triggers no stdin read.
The real Core control waits for that response, stores the reported count, and returns it at the program boundary.
The response increments the acknowledgement counter and wakes the condition wait. It posts no host event.
The real `kernel_bytes` argument is ignored, as specified; `on: false` with that argument is Bad.
The existing bounded request and response wait remains applicable. No production test branch or terminal encoder is added.
The new test reads request and response behavior; it checks successive reported counts of seven and zero.
The served-controls test now includes `pty_queued`, closing the first gate's missed arm deletion.

The six held-write transcripts use F=40960, `kernel_bytes: 1`, and the `pty_queued` range fence.
The real count remains a kernel observation, not an exact capacity assertion. FakeCore remains structural for those forms.
R-74 adds release and matcher-bound input fences where applicable. The probe fails terminal setup with exit 71.
R-73 resolves relation parts from the transcript's sent frames, observed refusals, and the byte log; it ignores segment attribution.
The updated Core test verifies the correct op, a wrong op, and a without-op failure through the transcript runner.
The .41 R-83 changes add route input fences; existing R-60 exceptions still prevent the proposed four real-tier promotions.
The pin also includes the R-28 oracle flags correction, R-71 hold wording and transcript correction, and erratum 6.
The R-71 no-deadline transcript raises startup to 30000 and advances the injected clock beyond it.
The AD-7 transcript removes the plain hold section; its other sections remain. This review claims no coverage from that removed section.
The new E6-1 identifier remains pending with its observed step-18 query failure and RC owner.
The pin adds no test-only worker state or Core behavior to make that new clause pass.

### Conformance accounting

Sol independently compares the .41 ledger with `core-ledger-ids.txt`: all 691 Core identifiers agree exactly.
Against the reviewed base, testkit pending removes seven identifiers and adds E6-1.
Three earlier removals are the two AM-2 identifiers and TH-3.
Four later route removals enter real pending: A2-2 refused, DP-3 raw frames, DP-5 abort, and DP-5 contiguous paste.
Each of those four has one .41 fence exception. No real PASS counts as its removal.
Real pending removes exactly four identifiers: EV-2 drop-oldest, EV-5 released, IN-2 link-loss, and IN-7 lost-worker.
Each has normal real PASS in this gate and no .41 fence exception. Repeated proof remains due under P289-SI-2.
The mutation exclusions are unchanged.
TM-6 stays pending. The body preserves its previous failure, unchanged transcript, unknown cause, and GS owner.
The ignored trial's PASS supplies no diagnosis or pending removal.
F-283-1 remains PC's separate follow-up. This pin claims no fix for those imported assertions.

### Completed gate and retained failures

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-pin-41-4e26982b-pool-20261011-144733-21583.log`.
SHA-256: `426854307dda1a64cf844de2018c61daa3dcaad3cd6c303a3b71af3790e1169e`.
The exact header names this head and base. All ten steps PASS in 404.4 seconds.
The job queues for one second and runs for 419 seconds. The job and wrapper exit zero after 420 seconds on msa1.
Default: 1786 PASS, 461 skipped, 13.201 seconds. Slow: 586 PASS, 2017 skipped, 42.252 seconds.
Minimum: testkit 64/69, real-passing 56/68, real-accepted 57/69. All four named real proofs pass.
All seven testkit removals have normal PASS in this gate. Twenty-six identifiers remain real-pending; 15 ignored trials pass.

Mutation stage 1 has eight mutants: zero caught, eight missed, zero timeout, zero unviable, 17.5 seconds.
The stage-2 baseline passes all 586 tests in 41.816 seconds; the baseline interval is 54.4 seconds.
Stage 2 tests those exact eight missed identities and catches all eight in 199.6 seconds.
Sol independently compares all eight raw catches, stage-1 misses, and citation identities; the sets agree without duplicates.
Every cited test has normal PASS. The complete log contains no FLAKY token.
The deleted real `pty_queued` arm is caught by IN-2 link-loss. The constant-one getter is caught by the new response test.
Fuzz selects no changed decoder crate. Sol claims no fuzz execution or Mac execution from this gate.

The retained ea67844e gate has eight PASS steps, mutants FAIL, and fuzz NOT RUN.
Stage 1 misses eight mutants; stage 2 catches seven and misses the `pty_queued` arm deletion.
The new served-controls test and current exact-head mutation result close that miss.
The retained bd5ea95b gate has fmt, clippy, and taint PASS, lists FAIL, and six later steps NOT RUN.
Its lists failure names the seven identifiers removed by #284 from the then-current base.
The lead-approved import preserves those removals; its new source and passing gate remain distinct from the failed gate.
The structural merge-base fix belongs to GS. This PR adds no such fix.

### Review status and limits

Body SHA-256 at this review: `9d952551931c87c5a66678ba829af8503123d0596e0396c99eb13cc96ef1d3db`.
GitHub MCP fails; the GitHub CLI fallback confirms this unchanged head and current base.
The uncited local checks and reversal assertion remain owner reports. Sol accepts no independent local execution proof from them.
Sol reads complete package verdict `42f295473daa793617c78aca4da7d454eaa703af` in `verdicts/pc-pin-41.md`.
PIN41-1 and PIN41-2 match these two evidence findings on this exact head and base. The package establishes no new logic defect.
Sol establishes no new integration source defect. Both evidence findings must close before source acceptance is complete.
After both CLEANs, the final SLOT import, applicable check and delta review, exact-head final gate, and final body remain due.
This receipt accepts no later head, pending removal, evidence job, or final merge.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check script. Never poll.

VERDICT: NOT CLEAN (2 open: P289-SI-1 and P289-SI-2) at 4e26982b4c8ed836e43baa30f18fbf8134db90d5

## Round 2 — completed evidence and list restoration, 2026-10-11

Exact head: `f45794a454761c20fa2cfb9c0bd1b0f9fabecfbc`.
Parent: `4e26982b4c8ed836e43baa30f18fbf8134db90d5`.
Reviewed base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Tree: `6ed07fea1e12fba1d4efd83112cc95b825abea00`.
Open integration findings: 0. P289-SI-1 and P289-SI-2 close as evidence conditions.

Sol reads the complete new-head delta. It restores only IN-7 to its original real-pending group.
The implementation, pin, tests, ledger, and other entries remain unchanged.
This restoration occurs within the unmerged PR. It does not restore an identifier already removed from the merged base.
Final own counts: seven testkit removals, one E6-1 addition, three real removals, and four real additions.
IN-7 stays pending. This receipt accepts no five-PASS claim for IN-7.

### A5-4 evidence closure

Evidence directory: `~/botster-sessions/shared/core-stage1/evidence/pc-pin-41/`.
Sol reads both complete job scripts, both current table scripts, all changed table cells, and every raw result through static parsing.
The before job uses base f68f26a7 and .39. The after job uses parent 4e26982b and .41.
Both jobs run on Linux msa1, kernel `6.12.111+deb13-amd64`.
PC confirms that `BOTSTER_SEEDS` is unset. The scripts supply no override; the reviewed library default selects seeds 0–7.
This receipt claims no seeds 0–31 coverage or Mac execution from these jobs.

| Artifact | SHA-256 |
| --- | --- |
| `a54-linux-before-f68f26a7.log` | `7159ba5eedff88e920b4959c44058d39ca60e9be9e769f702501ea41806bed32` |
| `a54-nrun-linux-after-4e26982b.log` | `e4358f490347669aeee40f4aa0229ecaac8dddda7c221b0d343a81f8b7344788` |
| `a54-real-v039-v041.md` (v2) | `d4ebfc8cd1d550addc2545dbc82ae7ab27374c52b4abce77bcb9aa9a204221bd` |
| `a54-testkit-v039-v041.md` (v2) | `cef1931879478b9c405310db42ff6375aa73924e0ea07d77f0c58b4675f8da5d` |
| `nrun-real-testkit-4e26982b.md` | `85c8154436669c6f688c2ddf9447d99e6c2f4ba58f949cc63875297b2a773596` |

The first tables omit 92 real rows and 93 testkit rows with other markers.
PC preserves the first parser as v1. The corrected v2 tables require no new execution.
Each v2 table contains all 691 unique .41 Core ledger identifiers.
Sol independently matches every identifier, result class, and PASS cell with the retained raw records.
Sol reads all changed failure details. The combined log equals the exact concatenation of the two raw logs.
Testkit: 223 must-pass PASS before, 230 after, eight changed rows, no PASS-to-non-PASS.
Real: 198 must-pass PASS before, 204 after, 21 changed rows; pending-real PASS increases from four to fourteen.
E6-1 is absent before and pending after. The corrected marker rows change no executed result.
The scripts filter output through pipelines. Wrapper zero after 79 and 67 seconds does not turn a test failure into PASS.

The real after run retains one must-pass FAIL: EV-5 route-closed, seed 7, raw step 28.
It receives `activity_before_close: false, closed: true`, instead of both true. No new pending entry is added for that failure.
The transcript is byte-identical across .39 and .41. It calls no held-write control.
The worker, route transport, and `route_reports` behavior on this path remain unchanged.
Those source facts support non-interference; they do not independently prove the cause of the original seed-7 failure.

Package receipt `110de71a0946a57d6494f749ced02e4cf13fcb8c` records lead disposition `msg_plugin-w_1791756534_fb4088`.
The lead accepts this retained failure for #289 using the separate .39 controlled reproduction already accepted by both reviewers.
Sol's complete controlled review is Round 6 of `rc-testkit-controls.md`, commit `acd9d7f5809f83e6c5da91be89ddc0e722675d05`.
That controlled job fails at seed 0, step 28, with the same values, on plain v1 plus a timing-only scratch diagnostic.
Its log SHA-256 is `b0fa78606981a6fe0621558c35d473d7c0aac59452fa402790443904caedcc8c`.
Its scratch diff SHA-256 is `027d7d7aad29ad7cd084dd9081f76001d837a3d07671debae1ac858e339f5397`.
The test exits 101 before delayed-input release. The shell's later zero is not test PASS.
This receipt claims no uninstrumented .39 failure, original after-run trace, or later release behavior.
EV-5 record: `~/botster-sessions/shared/core-stage1/handoffs/flaky-ev-5-route-closed-retires-route-keyed-events-4e26982b.md`.
The corrected record retains that attribution limit and RC's separate witness.
Record SHA-256: `90a86251a825a4fc25094108c5cdc4df7eb1a3b15c808f9e36dd09a0f0126ebd`.
Repair remains RC's contracts #69, R-66 activity fence, into .42. This pin does not fix that window.
P289-SI-1 closes with the complete comparison and these explicit failure limits.

### Repeated proof and IN-7 retention

Sol independently parses all 80 unique records: eight identifiers, two tiers, and five numbered runs per pair.
The records include all six required R-74 held-write identifiers and two additional event-fence identifiers.
Every testkit pair passes five of five. Seven real pairs pass five of five.
IN-7 real passes four of five. Run 4 is INCONCLUSIVE at seed 3, raw step 7, with `step_timeout`.
The complete failed trial remains in the raw log and table. No successful replacement run is claimed.
The three remaining real removals pass five of five on both tiers and have no .41 fence exception.
The other held-write identifiers retain their pending and fence-exception limits despite the observed passes.

Rule-9 record: `~/botster-sessions/shared/core-stage1/handoffs/flaky-in-7-lost-worker-ends-pending-4e26982b.md`.
SHA-256: `0f92c5759c624a1a9a5bc85b947ffa42e9567a7e21920f8d897bc8b957d21f29`.
Sol reads the complete record. It reports the stop at `await_control pty_queued`, in the R-74 held-write path.
Diagnosis remains open, owner PC, after #289 and R-78. The lead requires source and retained-log diagnosis without a reproduction campaign.
P289-SI-2 closes as an evidence condition with IN-7 restored to pending. IN-7 has no real acceptance claim.

### Other evidence and final-slot limits

Sol reads all 216 lines of `reversal-pty-queued-arm-4e26982b.log` and its full scratch diff.
SHA-256: `99ef8572cac4dec45e003ed5056aae80c27866128b89e90bbe3e49c9418c5bfd`.
Deleting the served arm produces `Err(Unsupported)` and test exit 101. Restoration produces PASS and exit zero.
Both runs compile the named PR worktree. The clean and shared-target writer history remain owner reports.
The independently reviewed parent gate's mutation catch covers that arm without relying on the local clean report.

Sol reads the complete `static-f45794a4.log`.
SHA-256: `12c4e2e1031e127b53ec690ad4a16250350ac914daf518692ecad5b4c638b55c`.
Taint and lists report success; lists names base f68f26a7 and 27 real-pending identifiers.
PC reports fmt success. The log contains no exact-head header; execution identity remains PC's report.
PC supplies the exact lead direction in `msg_plugin-w_1791756340_5a5262`: no separate full gate now for this list-only head.
The final SLOT gate must cover the final head. The parent green gate remains evidence for the unchanged implementation.
That parent gate is not a full gate on f45794a4.

Final body SHA-256: `c70e93fae81bf832f3e26cb5ffa91f0319f5025b84b8c5961dd6403be2e76dfa`.
GitHub MCP fails. The GitHub CLI fallback verifies the exact new head, current base, and complete body delta.
The body retains both failures, seed scope, IN-7 owner, original red gates, table correction, and the final-gate requirement.
Sol reads the complete Round 2 package section at `110de71a0946a57d6494f749ced02e4cf13fcb8c`, `verdicts/pc-pin-41.md`.
Its source CLEAN matches this head, base, tree, and evidence limits. Both package evidence findings close.
F-283-1 remains PC's separate follow-up. Existing TM-6 and other pending limits remain unchanged.
The final SLOT import, applicable check and delta reviews, exact-head final green gate, final body, and Q7 remain due.
This source receipt accepts no final merge, later head, or later pending removal.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job. Never poll.

VERDICT: CLEAN (0 open; source and evidence only) at f45794a454761c20fa2cfb9c0bd1b0f9fabecfbc against base f68f26a7b12738f2b19ab39fe76a93f3d1a72f12; final SLOT acceptance remains due.

## Round 3 — final-slot import and four additional testkit removals

Current head: `69d414995408a21a32b73d01608a798202057462`.
Reviewed base: `b0c0a419af27259006b5e06d4e5da3b3d00083e6`.
Tree: `12f279eac5f55917f829b8ea3f8bce7165f1cd1f`.
The SLOT merge is `0abf666c001e7dcd55e3bc86f0a95e168c183f26`.
Sol verifies both merge parents: reviewed f45794a4 and imported v1 b0c0a419.
Their common ancestor is f68f26a7. Two later commits change only the two pending files.
PC supplies the merge in `msg_plugin-w_1791765546_613c36` and the current head in `msg_plugin-w_1791765735_15ea3c`.

### Canonical check and production composition

Sol reads the complete retained base-merge-check log for 0abf666c.
Path: `~/botster-sessions/shared/core-stage1/evidence/pc-pin-41/base-merge-check-0abf666c.log`.
SHA-256: `d8b9d74d2c862a58d39d63c997f8e021781a1830628fd49484cdfde8ad196041`.
The ancestry checks PASS. Conditions 1, 2, 3, and both pending-file condition-4 checks FAIL.
The conflicts concern real/program.rs and both pending files. The retained FAIL requires both reviewers' explicit delta reviews.
Sol runs no executable base check and does not change that retained result to PASS.

Sol reads the complete current own delta and the overlapping program-control context.
The imported Programs object contains a Requests wrapper for the same shared hub.
Programs::queued remains in Programs. It calls Programs::request, which delegates to Requests::request.
The getter reads the reported count from self.requests.hub after the request returns.
Requests uses the same hub for probe identity, sent and acknowledged counts, closure state, and bounded waits.
The Queued report stores its count, increments acknowledgement, and wakes the condition wait without posting a host event.
The imported route_fill invokes the shared output request synchronously. It adds no background request writer on this path.
The successive-response test still checks seven and zero through the actual request/report boundary.
The served-control, parser, ignored kernel_bytes rule, and Bad-on-release rule retain their reviewed behavior.
No additional source defect arises from this overlap.

Sol independently compares the four unaffected own patches after removing only blob-index and hunk-location metadata.
Cargo.toml, Cargo.lock, the ledger, and route_pty_log.rs retain their complete reviewed patches.
All added and deleted program.rs lines also match the reviewed lines after the hub-access relocation.
That comparison treats line order separately because the getter moves within the impl; Sol reads its exact new position.
Every imported non-own blob matches b0c0a419. Both later commits preserve all source blobs.
The pin remains .41 at 2fcd7739708fe6de1390ff9bd9f90da5a8bc6d87.
All 223 lock packages retain every field except the nine contracts source strings that the pin changes.
No additional package, dependency, mutation exclusion, bound, or deadline changes.

### Three-way identifier sets and later list delta

Sol independently parses the common ancestor, both parents, and merged files.
Every merged set equals both parents' additions minus both parents' removals, with no missing, extra, or restored identifier.

| File | Common ancestor | Reviewed parent | v1 parent | SLOT merge | Current head |
| --- | ---: | ---: | ---: | ---: | ---: |
| core-pending.txt | 444 | 438 | 422 | 416 | 412 |
| core-real-pending.txt | 26 | 27 | 39 | 40 | 44 |
| core-ledger-ids.txt | 690 | 691 | 690 | 691 | 691 |
| core-real-only.txt | 26 | 26 | 26 | 26 | 26 |

The pending conflict removes both parents' separately removed DP-5 identifiers.
The real-pending conflict retains both added blocks and the reviewed IN-7 restoration.
The held file matches imported v1 byte for byte and retains both budget entries and the unproven entry.
No pending-file comment or blank-line resolution changes identifier disposition.

Commit 8ab0f6d50fcb3c11656d747e794f197094ba9600 removes four testkit pending lines.
Commit 69d41499 adds exactly those four identifiers to real-pending with their reason and P3 fence owner.
Sol reads both complete commits and independently checks equality of the removed and added sets.
The four identifiers are:

- dp_5_confirmed_paste_resend_is_a_new_op_with_require_bracketed_false.
- dp_5_input_done_only_for_want_result_true.
- dp_5_mode_precondition_zero_effect_reported_on_the_route.
- in_9_kitty_unknown_flag_bits_ignored.

Sol checks their exact .41 fence exceptions. The three DP-5 identifiers have one R-60 program_ready exception each.
The kitty identifier has one R-60 exception and three R-56 capture exceptions at steps 13, 19, and 25.
None was already real-pending or real-only at 0abf666c.
Their new real-pending block preserves the rule that a real PASS cannot promote an identifier with a pinned fence exception.
This is a move from testkit pending to real-pending, not real-tier acceptance.
The final own counts are eleven testkit removals and one E6-1 addition, plus three real removals and eight real additions.
E6-1 remains pending. IN-7 remains real-pending with its retained failed trial and open diagnosis.
The final gate must supply normal testkit PASS results for all eleven own removals.

### Retained local records and their limits

The complete static log has SHA-256 `e7233aafe11be1970ed0f04655253a1b91796bfba0d14513db2186577d0029ce`.
It records exact head 0abf666c, a clean worktree, and cargo clean removing 3.3 GiB.
Clippy and all ten xtask checks report exit zero. PC reports fmt success; the log has no separate fmt result line.

The original local strict log has SHA-256 `318514f72df99f04b96d75f275c5fdf97a8f0cc561aae74abb338a920cb25340`.
Its header names 0abf666c, the prior clean, strict mode, and seeds 0–31.
The conformance binary reports 252 passed, four failed, and 435 ignored, then exits 101.
All four failures say that a pending identifier passes and must leave testkit pending.
These are strict-accounting failures, not four failing contract outcomes.
PC's earlier combination-only explanation is not proved: a pending entry in both parents supplies no plain-v1 execution result.
PC preserves that limit in the corrected body. Sol claims no plain-v1 result for those four identifiers.

The removal rerun is `local-check-0abf666c-unpend4.log`, SHA-256 `9dbf93967357c5770e6f2d6ab5e1d0ba79819b0a0c74a946ef0a8a32509c72a4`.
Its header names 0abf666c plus the uncommitted four removals, the same worktree and prior clean, and --no-fail-fast.
Each of the four identifiers has a normal conformance ok result. That binary reports 256 passed and zero failed in 2.95 seconds.
The retained summaries total 1178 passed and zero failed; the shell exits zero.
This is the scratch removal state before the real-pending block, not an exact-final-head gate.

The lists/taint log has SHA-256 `c53e6f0068e8a9d8c88c8959ea0304abe714db616b85345af1877a4620db2868`.
Its header names 8ab0f6d5 plus the four-identifier real-pending block. PC attributes that tree to 69d41499.
Lists reports 691 ledger identifiers, 412 pending identifiers, and 44 real-pending identifiers; lists and taint exit zero.
These local records retain their stated source and clean scope. They do not replace the final gate.
PC reports that the 8ab0f6d5 gate stopped before execution and that the one final gate runs on 69d41499.
Sol reads no running output and claims no completed final gate.

### Body correction and package scope

P289-SI-3 LOW arose because the body still named f45794a4 as HEAD and omitted the final-slot import.
It closes after Sol reads the complete reported body delta and confirms exact GitHub head 69d41499 and base b0c0a419.
Current body SHA-256: `97ea97bf34f8f82aa2b66924619cf25e87748534644bdd556b4eef4d3cdb319d`.
The final-slot section names both current identities, all three conflicts, both later commits, and the correct pending counts.
It ties old gates, A5-4, and N-run records to their old heads and marks the final gate RUNNING.

Sol reads complete package Round 3 at `b4d6f23fbbefc6c8cc9ad108043cd9ebf9b1a95d`, verdicts/pc-pin-41.md.
It agrees on the 0abf666c source composition and set resolution.
It accepts neither the later list delta nor final evidence. The exact-current-head package supplement remains due.
The previous A5-4 and N-run evidence retains its earlier scope and recorded EV-5 and IN-7 limits.
The completed exact-final-head gate, final body, package evidence acceptance, and final merge checks remain due.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job. Never poll.

VERDICT: CLEAN (0 open source delta findings; current package supplement and final evidence remain due) at 69d414995408a21a32b73d01608a798202057462 against base b0c0a419af27259006b5e06d4e5da3b3d00083e6

### Current-head package receipt

Sol reads complete package Round 4 at `fc643a29d9bf552855dbbaa14f6f5356b1372e2b`.
It covers exact head 69d41499 and base b0c0a419 with zero source findings.
Its four matching transfers, pinned exceptions, P3 owner, local strict results, and evidence limits agree with this review.
Both source delta reviews now cover the current head.
The package receipt condition closes. The completed final gate, final body, package evidence acceptance, and final merge checks remain due.
No final merge or plain-v1 execution result is accepted. Sol runs no execution job. Never poll.
