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
