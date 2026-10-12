# P6 review: E2 report holds — PR #315

## Round 1 — 2026-10-12

Exact head: `cac85963de8f02a9751f94714248d957c082a497`.
Tree: `f048d9c0690b8f1e50b0f2e74c5f01aa71873a68`.
Source parent and merge base: `080987829a2e2245a7ea97606020400392c974d6`.
Gate comparison base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Risk: HIGH. The delta changes multiple packages and `.cargo/mutants.toml`.

I read the complete 17-file delta, the component, the adapters, and all new tests.
I read the complete PR body through the CLI after GitHub MCP truncates it.
Body SHA256: `6b0a80561aecd17518cfadc51f4558cb0e74b85d81af30cd78ee4f9538a4ab59`.
I read design-note sections 2, 4, and 10. Section 10 limits E2 to report controls and the worker report-order proof.
Uploads work owns `fs_directory` and `fs_list`. This PR removes no pending identifier.

### Source and contract review

One ReportHolds component serves the simulated edge and the real edge tap after their existing reads ahead.
Core's frame and message decoders identify the instance and target report.
An arm selects the next worker message, or the next Launched/Exited message for its delay point.
The component preserves earlier frames, the held frame, later bytes, and their order.
A frame whose first byte already reached the driver cannot become held.
With no arm, each read asks for no more bytes than the driver requests.
Mixed hold kinds are Refused. Duplicate worker holds, invalid releases, and invalid control arguments are Bad.
The session lookup occurs before the holder lock. Release signals the wake after the holder lock ends.

PumpBound advances delays once per pump. The new host test observes public scheduler calls and requires PumpBound first and once.
The adapters retain exits that the production process edge already reaped. They add no reap call or process ownership.
An explicit break discards held bytes and later bytes. Closing the driver's link removes that link's holder record.
The real controls remain available only through the tapped harness.
The new real tests use the existing worker candidate, harness guards, and cleanup deadline.
Finding B315-I2 identifies a failure to enforce that deadline.

The component tests cover byte order, partial frames, release, break, errors, mixed holds, and exit association.
The registry tests assert completion results and public session behavior.
The real tests assert delayed Running state and Deleted or OutcomeUnknown uploads results.
These tests retain public behavior assertions. They do not inspect private production state.
No existing contract assertion is removed by this delta.

The worker test observes LinkSend and PtyWrite actions for two complete client frames.
I read receive_route, client_received, apply_route, and the report enqueue path.
Each client-input report enters the ordered link actions before its frame reaches PTY admission.
The new design rule and test preserve the existing worker implementation.
They establish the declared order; they remove no transcript or E3 debt.

The only mutation exclusion change moves LINK_CAPACITY's arithmetic location from line 34 to line 35.
Its source remains `const LINK_CAPACITY: usize = 64 * 1024;`.
No contracts pin, status file, process allowance, or additional exclusion changes.

I verify the Prior-art search at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
It finds the two unrelated lines stated in the body.
I read GatedRuntime at that exact commit. It gates plugin invocations inside one process.
It provides no worker-frame order, report count, or link-break behavior to reuse. The rejection is supported.

### Findings

**B315-I1 — MEDIUM — the real quiet probe repeatedly exposes a held EOF. OPEN.**

Evidence: `edge_tap.rs:307`, Tap::quiet; `report_holds.rs:338`, recv; RealEdges::link_recv.
After a cleanup report becomes held, recv keeps the link's EOF behind that report and returns WouldBlock.
ReportHolds::gives is false when the holder has only held bytes and a held exit.
Tap::quiet then reads the inner socket directly. RealEdges reads the stream again and returns EOF again.
The tap stores that end and reports non-quiet. The next pump transfers the end behind the same hold.
The next quiet probe repeats the inner EOF read. The held state cannot become quiet through this path.
Section 10 requires held bytes, ends, and exits to supply no report to edges_quiet.

Required change: make quiet observations respect the holder's delivery state and retained end.
Add a controlled tap or real-edge assertion for a held cleanup report with EOF and a deferred exit.
The held state must become quiet before release. Release must expose deliverable work as non-quiet.
I confirm this finding by source inspection. I run no reproduction.

**B315-I2 — MEDIUM — the real completion helper bypasses its deadline when more stays true. OPEN.**

Evidence: `real.rs:1867`, slow_controls::completion.
The helper promises completion by the cleanup deadline. It checks expiration only inside `if !report.more`.
If Core keeps more true without completing the requested operation, the loop never checks the deadline.
Required change: check the deadline on every iteration. Retain the wake wait when no runnable work remains.

**B315-I3 — LOW — the design note contradicts link-end ordering. DOCUMENT CORRECTION READ.**

Integration's initial review finds that section 10 says EOF or read error ends a hold.
The implementation and its test keep the end behind the held frame until release.
I read the corrected shared section 10 during this round. It now states that distinction and the explicit-break behavior.
The document correction closes that contradiction. It does not close B315-I1 or B315-I2 on this exact head.
P6's earlier notice identifies Scheduler::bound(PumpBound) as the actual pump-start observation.
P6 reports local source fixes and a running gate in `msg_plugin-w_1791824028_951edc`.
That report supplies no reviewed replacement head or completed evidence in this round.

### Retained evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p6-e2-report-holds-cac85963-pool-20261012-092918-39330.log`.
SHA256: `ece78344976d64b33d4b020dd8aac40194fc6c52f9974c8fa1726ecc7f451304`.
I read the identity, base accounting, changed-test results, mutation records, citations, summaries, and exits.
The comparison base differs from the source parent in four accepted #310 xtask files. A later import needs its own review.
All ten steps PASS in 1018.8 seconds.
Default passes 1959 tests, with 431 skipped, in 11.541 seconds.
Slow passes 622 tests, with 2090 skipped, in 39.891 seconds.
Strict conformance passes 261 tests. Pending remains 406; real-pending remains 43.
The new host, worker, component, registry, and real tests PASS.

Stage 1 tests 155 mutants: 131 caught, 11 missed, 13 unviable, and zero timeout.
The unmutated slow baseline passes 622 tests. Stage 2 catches the 11 misses.
The final result has 142 caught, 13 unviable, zero missed, and zero timeout.
I independently compare all 142 unique raw catch identities with their citations. The sets match exactly.
Every cited catcher has a normal PASS. The complete log contains no FLAKY or INCONCLUSIVE record.
Fuzz reports no changed crate with a decoder harness.
The job and wrapper exit zero after 1028 seconds: one second queued and 1027 seconds running.
This gate does not establish the missing quiet case or close the source findings.
I rely on this full gate's mutation results. I do not claim a review of the earlier focused mutation log.

I read integration verdict `328a0436c951f2c8832c7039c3b1bc238795b8a1`, Round 1, in full.
Our source findings and retained-gate conclusions agree. I separately read the later shared document correction.
Two source findings remain at this exact head. I send the findings directly to P6 and integration.
No CLEAN is sent to the lead.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-12: report-hold repairs

Exact head: `29dc0efe0f817f729f62582d09a28a239c49272b`.
Tree: `e48333e160ae048a5b7e1b0f59e0b7246d41022b`.
Parent and reviewed input: `cac85963de8f02a9751f94714248d957c082a497`.
Source merge base: `080987829a2e2245a7ea97606020400392c974d6`.
Gate comparison base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Risk remains HIGH. READY: `msg_plugin-w_1791825160_2c59ae`.

I read the complete five-file repair delta and the relevant adapter, fixture, holder, and completion paths.
The head descends directly from the reviewed input. No base import occurs.
I read the complete updated PR body after GitHub MCP truncates it.
Body SHA256: `b75f8956ced08659500fcbf776c93b2e11923847da1f01134a2895fe00b3e76f`.
GitHub names this exact head and the gate comparison base. The PR remains OPEN when read.
Round 1's unchanged source, contract assertions, process ownership, status accounting, and Prior-art review carry to this head.

### Finding closure

**B315-I1 — CLOSED.** Tap::quiet now checks ReportHolds::withholds before it probes a link.
It skips the inner read when that link has a held frame. It cannot read the held EOF again through that probe.
ReportHolds::gives now includes an end with no hold before it. A released end therefore remains deliverable work.
Quiet retains the existing bounded probe on other links and the existing process-exit path.
The tap takes its holder lock in the existing tap-to-holder order. The changed path adds no reverse lock acquisition.

The new controlled tap test uses an inner link whose closed state returns EOF on every read.
It asserts WouldBlock and quiet twice while the frame and EOF remain held. It requires zero wake signals in that state.
After release, it asserts non-quiet, receives the report, asserts non-quiet for the end, and receives EOF.
After the driver closes the link, it asserts quiet.
These assertions observe edge behavior. The closed-state setup belongs to the test fixture, not private production state.
The component end test separately requires withholds during the hold and gives for the free end after release.
The earlier component exit tests still cover retained exits and release. The new tap test isolates the repeated-EOF defect.
Both tests pass normally. The withholds-to-false mutant is caught by the new tap test.

**B315-I2 — CLOSED.** The completion helper checks its cleanup deadline after every pump that does not complete the requested operation.
The check runs for both more=true and more=false. The idle path retains its wait on the public wake handle.
The helper adds no sleep, additional reap, or unbounded derived wait.

**B315-I3 — CLOSED.** I read the corrected shared design-note section 10 again.
It states that EOF and read errors wait behind the active hold. It distinguishes an explicit break and the driver's link close.

### Completed gate and earlier failure

Log: `~/botster-sessions/gates/botster-core-stage1-p6-e2-report-holds-29dc0efe-pool-20261012-095714-12815.log`.
SHA256: `2ccaf806491ddf58159b70ab4183dd4c7c6b1cf02276701275a161117346a46b`.
I read its head/base header, conformance records, changed-test results, mutation records, citations, tier summaries, and exits.
All ten steps PASS in 873.8 seconds.
Default passes 1960 tests, with 431 skipped, in 11.317 seconds.
Slow passes 622 tests, with 2091 skipped, in 39.544 seconds.
The new quiet-state test, changed component test, and all three real report-control tests PASS.
Strict conformance passes 261 with zero failures. Pending remains 406; real-pending remains 43.
No status removal is accepted from the job's pending-test suggestions.

Stage 1 tests 159 mutants: 136 caught, 10 missed, 13 unviable, and zero timeout.
The unmutated slow baseline passes 622 tests. Stage 2 catches those exact 10 misses.
I independently compare the stage-1 missed identities with the stage-2 caught identities. The sets match exactly.
The final result has 146 caught, 13 unviable, zero missed, and zero timeout.
All 146 unique raw catches match their citations. Every cited catcher has a normal PASS.
The complete log contains no FLAKY or INCONCLUSIVE record. No catch is credited to an unviable mutant.
Fuzz reports no changed crate with a decoder harness.
The job and wrapper exit zero after 882 seconds: one second queued and 881 seconds running.

The body retains the earlier failed gate at local head `8d605032`.
Log: `~/botster-sessions/gates/botster-core-stage1-p6-e2-report-holds-8d605032-pool-20261012-095336-3367.log`.
SHA256: `495460070fbecbb5d291e8b066c2eab39c0a33744ae718bcf83fe71a03db5b34`.
I read the named failure, expected/actual byte output, tier summary, and final exits.
The new test asks for 64 bytes and incorrectly expects the complete hello frame, which exceeds that buffer.
The default tier reports one failure; later steps do not run. The job and wrapper exit one.
I inspect that local-head delta against the READY head. It changes only four test read sizes from 64 to 4096.
The expected frames and quiet-state assertions remain unchanged. The source repair is unchanged.
The structural fixture correction accounts for the failure. This is not a same-source retry credited as a catch.

No source or evidence finding remains for this exact head. The package source and retained Linux gate are CLEAN.
This verdict accepts no later head, base import, train composition, contracts pin, or status removal.
The integration verdict, later slot or train review, and lead merge verification retain their separate requirements.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN
