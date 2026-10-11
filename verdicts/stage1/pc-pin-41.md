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
