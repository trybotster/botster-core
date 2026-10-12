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

## Round 3 — 2026-10-12: #315 source delta on train 5

Exact train head: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Tree: `42e00e9808db9faec524e2404f67302bee0d66c5`.
Train base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Reviewed #315 input: `29dc0efe0f817f729f62582d09a28a239c49272b`.
Original source merge base: `080987829a2e2245a7ea97606020400392c974d6`.
Original gate comparison base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Lead request: `msg_plugin-w_1791828964_3f25ee`.
Scope: #315's source delta and composition on this exact train head. Risk remains HIGH.

### Merge and patch preservation

I inspect both merge objects and their exact parents.
First merge `1034c71e1e3b42a916455e5fc5ee8ee67388f7ff` joins the train base and #306 input `0a09168702048cf34ef3fa30dffc07131b620f0a`.
The final merge joins that first merge and the reviewed #315 input above.
I compare each file's added and deleted lines in #315's original patch with its landing patch after the first merge.
All 18 comparisons match exactly. No #315 line is lost, changed, or duplicated in the landing patch.
Thirteen complete #315 files are byte-identical between the reviewed input and the train.
Five files also contain the new base's changes: controls.rs, core.rs, edge_tap.rs, lib.rs, and real.rs in botster-core-testkit.
The report_holds component and both test modules remain byte-identical.
Component SHA256: `101a1c8863109bb4005dbf25c598f76dabdb5b9a002cfc253f3bbce61e8da24a`.
The worker implementation subtree and report-order test remain unchanged from the reviewed #315 input.

I read the complete base deltas in the five shared files and the relevant host, message, and route-report changes.
I read the final receive, close, scheduler, quiet, control registration, dispatch, and completion paths.
The report-hold registration and the control-queue registration both remain in the registry.
The real harness retains both controls and their tapped-only dispatch.
Per-host report-holder state, spawned identity association, and retention of already reaped exits remain intact.
The shared files add no additional reap or ownership of a production child.
The quiet repair, free-end accounting, and completion deadline check remain intact.
All public contract assertions and the finding closures from Round 2 carry to this exact source.

### Composition with the base

#307 adds the control-room check before each link read. It does not replace or bypass that read.
Its event path runs only after a cancel was refused for lack of room.
The driver still asks PumpBound once, as its first scheduler choice, before it handles exits or services links.
The report-delay counter therefore retains its reviewed pump-start observation.
The report-control tests do not refuse a cancel or set control_queue. Their control-room path has no waiting event.

On the simulated edge, RouteReports identifies the instance from hello bytes that reached the driver.
The holder passes the hello before a held worker report. Its reads still record only the bytes returned to the driver.
The added instance lookup does not count the holder's later bytes as consumed reports.
The control-room read takes the reports lock before the faults lock. Report reads take the reports lock before the holder lock.
The queue control resolves its session row before it takes the faults lock. The report control resolves its row before its holder lock.
I find no new reverse lock acquisition in these intersecting paths.
On the real edge, both controls use the tap lock. The report holder retains the existing tap-to-holder order.

The base removes Observation::Writable and posts SessionWritable from the host's control-room check instead.
ReportHolds still classifies Launched, Exited, and all other successfully decoded worker messages.
It does not match the removed observation variant. The worker report-order path is unchanged.
The .44 contracts pin adds no source change to the reviewed worker implementation or report-control assertions.
No #315 conformance identifier leaves pending on this train.

### Composition with #306

#306 and #315 share only botster-core-testkit/src/lib.rs as changed files.
I read the complete shared-file delta and the new build-root macro definition.
The final file retains queue_controls, adds report_holds, and uses #306's root build check.
The macro calls the same assert_built_here helper with caller-side package and manifest values.
It changes no report-control state, scheduler, process ownership, or contract assertion.
The host and worker unit roots also retain that macro and the new #315 test modules.

I read the relevant Cargo test declarations and the slow-filter change.
The new report component, registry, tap, host, and worker tests remain unit tests in their existing test binaries.
The real report-control tests remain in real::slow_controls and match the existing slow_ filter term.
The additional exact-name filter term selects build-root tests. It excludes no #315 test.
The required-features declarations apply to separate integration binaries, not the new unit-test modules.
This composition review does not replace #306's package review or the combined integration review.

The complete conformance and ci trees remain identical to the train base.
The existing LINK_CAPACITY exclusion still names core.rs line 35, column 33, which matches the final source.
Its positive capacity and arithmetic expression remain unchanged. No additional mutation exclusion or process allowance appears.

No #315 source finding remains on this exact train head.
This CLEAN covers source and composition only. The original retained gate still belongs to the original #315 head and bases.
The lead reports compile and strict-conformance pre-checks. I do not accept those reports as a completed train gate.
The exact train-5 gate, final #315 body and receipts, and lead merged-tree/push verification remain due.
This verdict accepts no later head, pin, status removal, or completed merge.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN

## Round 4 — 2026-10-12: final #315 receipt on train 5

Exact train head: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Tree: `42e00e9808db9faec524e2404f67302bee0d66c5`.
Gate base and merge base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Frozen #315 member: `29dc0efe0f817f729f62582d09a28a239c49272b`.
Lead request: `msg_plugin-w_1791831674_84d558`.
Scope: #315's final package receipt on this exact gated train head.

Round 3's source and composition remain unchanged. I confirm the exact tree again.
The original patch, public behavior assertions, worker report order, process ownership, and finding closures retain their reviewed scope.
I read integration commit `961f2be2a8c07f19ec09de7dfc9f8883b2bcbf2d`: #315 Round 3 and combined train-5 Round 1, in full.
Those records agree with the package source verdict and preserve their source-only limits.

### Completed final gate

Log: `~/botster-sessions/gates/botster-core-stage1-train-5-2683de7c-pool-20261012-112942-61568.log`.
SHA256: `bcb49fb7a9644f33b0a5f1b2dbc0e16dda3fb6cd84f1a4f291bfc2427375d536`.
I independently confirm the hash. I read the identity, accounting, affected tests, coverage records, mutation results, citations, summaries, fuzz completions, and exits.
The header names the exact train head and base above. Lists reports that base as the merge base too.
All ten steps PASS in 1747.7 seconds.
Default passes 1998 tests, with 427 skipped, in 11.863 seconds.
Slow passes 663 tests, with 2078 skipped, in 42.153 seconds.

I inventory the 28 new #315 tests from the original patch and match each complete name to its normal-tier PASS.
All 25 default tests and all three real slow_controls tests pass in their assigned tiers.
The real break test passes in 0.049 seconds. Release and delay each pass in 0.052 seconds.
The PumpBound, repeated-EOF, and worker report-order tests each pass in 0.003 seconds.
The host, testkit, and worker-core build-root tests pass in default. The testkit root also passes in ordinary slow.
Ordinary slow selects packages with a slow feature; it does not select host or worker-core test binaries.
The static name check inspects 251 Rust files. Slow coverage checks 1098 slow-only tests and completes without a coverage or root-presence violation.
These records preserve #306's accepted coverage limits. They establish no stronger stale-library guarantee.

Strict conformance passes 265 with zero failures. Pending remains 403; real-pending remains 34.
Held-full runs three budget identifiers at 32 seeds: 96 expected ignored trials, with zero failures.
The real minimum reports 66/68 passing and 67/69 accepted under its stated accounting.
No pending-test suggestion supplies an additional status removal in this receipt.

Stage 1 tests 176 mutants: 147 caught, 14 missed, 15 unviable, and zero timeout.
The unmutated slow baseline passes 673 tests in 43.788 seconds. Its stage takes 63.5 seconds.
Stage 2 catches the exact 14 stage-1 misses. I independently compare those identity sets; they match.
The final result has 161 caught, 15 unviable, zero missed, and zero timeout.
All 161 unique raw catch identities match their citations. Every cited catcher has a normal PASS.
The gives-to-false and withholds-to-false mutants name the normally passing repeated-EOF test.
No catch is credited to an unviable mutant.

No FLAKY or runtime INCONCLUSIVE result appears. The single INCONCLUSIVE string belongs to the stored known-flake diagnosis.
The listed EV-2 test passes ordinary slow in 0.103 seconds and the unmutated baseline in 0.115 seconds.
The mutation filter explicitly excludes its exact pair. No credited catch names EV-2.
Those ordinary passes remove no known-flaky entry and establish no structural repair.

Four fuzz harnesses complete their 60-second limits in 61 seconds each:

- link_decoder: 6230000 cases.
- msg_decoder: 3615174 cases.
- guardian_command_decoder: 7088305 cases.
- guardian_log_decoder: 28432924 cases.

No sanitizer failure or written crash artifact appears in the retained log.
The job and wrapper exit zero after 1854 seconds: one second queued and 1853 seconds running.
This receipt claims no new Mac execution. The source adds no OS-specific process ownership or reap path.

### Final evidence body and disposition

I read the complete initial body after GitHub MCP truncates it. I ask P6 to add the final train record.
P6 confirms the update in `msg_plugin-w_1791831938_6a856b`.
I read the complete updated delta against the previously reviewed body. The frozen member head remains unchanged.
Updated body SHA256: `1d475c73319e90e97d9a18983ececb93864888f34f6070646475ee1fb657a15c`.
Its final train section names the exact head, tree, base, ordered member merges, completed results, log, and hash.
The source and gate records support its test counts, conformance counts, mutation summary, and EV-2 credit limit.
The initial gate and earlier failed local gate retain their original scopes and outcomes.
The body records the existing CONTROLS dead-code warning in builds without the real tier as a P6 follow-up.
The real tier uses that constant. The completed gate passes; this prose record changes no frozen source or process rule.

No #315 source, final-body, or supplied-evidence finding remains on this exact train head.
This is #315's final package CLEAN receipt naming the completed train-5 gate.
It does not replace #306's package review or the combined integration final review.
The final integration receipts and lead verification of the merged tree and remote push remain separate requirements.
This receipt accepts no later head, contracts pin, status removal, additional re-gate, or completed merge.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN
