# PR #315: E2 report holds

## Round 1: initial source and evidence review

Exact head: `cac85963de8f02a9751f94714248d957c082a497`.
Tree: `f048d9c0690b8f1e50b0f2e74c5f01aa71873a68`.
Source parent: `080987829a2e2245a7ea97606020400392c974d6`.
Gate comparison base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Lead assignment: `msg_plugin-w_1791823646_39ca4b`.
Risk: HIGH. Sol-B owns integration; 016a owns the package review.

### Scope and source audit

Sol-B reads the complete 17-file delta, full PR body, and design note, including sections 2, 4, and 10.
The later E2 ruling in section 10 narrows the earlier scope in section 4.
It includes four report controls, mixed_holds, and the client-input report order proof.
It leaves fs_directory and fs_list to uploads work. No conformance ID leaves pending.

One ReportHolds component serves SimEdges and EdgeTap at the host link receive boundary.
It identifies instances from hello frames and selects worker reports with Core's frame and message decoders.
An arm selects the next worker message or the next Launched/Exited message for its delay point.
The holder preserves complete earlier frames, the held frame, later bytes, and the order of link ends.
A frame already partly delivered cannot become held. Without an arm, reads request no more than the driver's buffer.
Mixed control kinds are refused per instance. Duplicate worker holds and invalid releases are Bad.
The delay counter starts at one when the frame becomes held. PumpBound advances it once at each pump start.
The release control resolves the session row before taking the holder lock. It signals the wake after unlocking.

Both adapters record spawned identities and defer reaped exits while that instance has a hold and an open link.
An explicit break discards held bytes and later bytes. Closing the driver link removes its holder record.
The real adapter retains tapped-only dispatch. The plain real harness does not list or serve the controls.
Production interfaces and implementations receive no test branch or report-holder code.

Sol-B reads the unit tests, control-registry tests, three real control tests, and host PumpBound proof.
They cover release, break, delay, frame order, partial frames, errors, mixed holds, and exit ownership.
The real tests use the existing candidate worker and harness guards.
The findings below identify behavior that those passing tests do not establish.

The worker order proof observes LinkSend and PtyWrite actions for two client frames in one read.
Sol-B reads receive_route, client_received, apply_route, and the report enqueue path.
Each client-input report enters the ordered link actions before that frame reaches PTY admission.
The new test and design bullet retain the existing worker implementation.
This establishes the declared worker order. It removes no remaining transcript or E3 debt.

The only mutation exclusion change moves LINK_CAPACITY's arithmetic location from line 34 to line 35.
The source remains `const LINK_CAPACITY: usize = 64 * 1024;`.
No pin, conformance file, process allowance, or additional exclusion changes.
Sol-B verifies the named old-Core search at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
It returns the two unrelated lines stated in the body.
Sol-B reads GatedRuntime. Its invocation gate supplies no worker-frame order, report count, or break behavior to reuse.

### Findings

**B315-I1 — MEDIUM — a held cleanup report cannot make the real edges quiet. OPEN.**

Evidence: `edge_tap.rs`, Tap::quiet and LinkTap::holds_anything; `report_holds.rs`, recv and gives; RealEdges::link_recv.
After RemoveResult becomes held and the worker closes, recv retains EOF behind the hold and returns WouldBlock.
The holder has no free bytes or free exit, so gives returns false.
Tap::quiet then probes the inner socket again. EOF remains readable, so the tap stores an end and returns false.
The next pump transfers that end behind the same hold. The next quiet probe reads EOF again.
The held report and its deferred exit therefore prevent quiet from becoming true.
Section 10 states that held bytes and exits are not reports for edges_quiet.

Required change: make the quiet probe respect the report holder's retained end and held delivery state.
Add a controlled tap or real-edge proof for a held cleanup report with EOF and a deferred exit.
The held state must become quiet before release. Release must expose delivered work as non-quiet.
Sol-B establishes this finding by source inspection and runs no reproduction.

**B315-I2 — MEDIUM — the real completion helper does not enforce its deadline while more stays true. OPEN.**

Evidence: `real.rs`, slow_controls::completion.
The helper states that it completes by the cleanup deadline.
It checks expiration only inside `if !report.more`.
If Core keeps more true without completing the requested operation, the loop never checks that deadline.

Required change: check the cleanup deadline on every iteration, for runnable and idle paths.
Retain the real wake wait when no work remains.

**B315-I3 — LOW — the design note contradicts the link-end ordering. OPEN.**

Evidence: design note section 10 says EOF or read error ends the holds of the link.
The implementation and unit test keep that end behind a held frame until release.
An explicit break instead discards the held bytes.

Required change: make the design note state the intended distinction between an ordered link end and an explicit break.

### Completed gate audit

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-e2-report-holds-cac85963-pool-20261012-092918-39330.log`.
SHA256: `ece78344976d64b33d4b020dd8aac40194fc6c52f9974c8fa1726ecc7f451304`.
Sol-B reads the identity, base accounting, changed-test passes, raw mutation results, citations, summaries, and exits.
The log correctly distinguishes comparison base 838e4bb0 from merge base 08098782.
Those bases differ in the four accepted #310 xtask files. The current PR has not imported #310.
The body states this distinction. Later import and train review remain due.

All ten steps PASS in 1,018.8 seconds. Default passes 1,959 tests; slow passes 622 tests.
Strict conformance passes 261 tests without failure. Pending remains 406; real-pending remains 43.
The new host, worker-order, unit, testkit-control, and real-control tests PASS.
Stage 1 tests 155 mutants: 131 caught, 11 missed, 13 unviable, and zero timeout.
The unmutated slow baseline passes all 622 tests. Stage 2 catches those 11 exact misses.
The final result has 142 caught, 13 unviable, zero missed, and zero timeout.
Sol-B independently matches all 142 raw catch identities to their citations. Every catcher has a normal PASS.
No FLAKY or INCONCLUSIVE record appears. Fuzz reports no changed decoder-harness crate.
The job and wrapper exit 0 after 1,028 seconds on msa1, with one second queued and 1,027 seconds running.
This Linux gate does not establish the missing quiet case or accept the source findings.
The earlier focused mutation job retains its stated separate scope; this review relies on the full gate's mutation results.

The same-head package verdict remains due at publication of this round.
Sol-B sends these three findings to P6 and 016a. No CLEAN is sent to the lead.
Every finding must close on a reviewed exact head before CLEAN.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: NOT CLEAN (3 open)

## Package receipt and document closure

Sol-B reads the complete same-head package verdict `2ab95fe9e9becc641db1710ed44f11f740f14b86`.
It confirms B315-I1 and B315-I2 on `cac85963de8f02a9751f94714248d957c082a497`.
Both reviews agree on the retained GREEN gate and its limits.

Sol-B reads the corrected shared design note, section 10.
It now states that EOF and read errors wait behind the held frame until release.
It separately states that an explicit break discards held and later bytes.
B315-I3 is CLOSED by this document correction.
The source findings B315-I1 and B315-I2 remain OPEN on the reviewed head.
The note describes the intended quiet repair; that statement supplies no accepted replacement source.
P6 reports local fixes and a running gate. Sol-B accepts no unfinished result and waits for completed READY.
Sol-B ran no execution job.

VERDICT: NOT CLEAN (2 open)

## Round 2: report-hold repair and completed evidence

Exact head: `29dc0efe0f817f729f62582d09a28a239c49272b`.
Tree: `e48333e160ae048a5b7e1b0f59e0b7246d41022b`.
Parent: reviewed input `cac85963de8f02a9751f94714248d957c082a497`.
Source merge base remains `080987829a2e2245a7ea97606020400392c974d6`.
Gate comparison base remains `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Sol-B reads the complete five-file delta, relevant fixture and adapter context, full body, and corrected design section 10.
No base import occurs. Round 1's unchanged source, contract, process ownership, and Prior-art checks retain their scope.

**B315-I1 is CLOSED.** Tap::quiet skips an inner link read when ReportHolds::withholds identifies a held frame.
The probe can no longer take EOF again behind that hold.
ReportHolds::gives includes an end with no hold before it. Released ends remain deliverable work.
Other links retain the bounded quiet probe. The existing process-exit path remains unchanged.
The tap acquires the holder lock in the existing tap-to-holder order. No reverse acquisition appears in the delta.

The new controlled tap test uses an inner closed state that returns EOF on every read.
It checks WouldBlock and quiet twice before release, with zero wake signals.
After release, it checks non-quiet, report delivery, non-quiet for the end, and EOF delivery.
It checks quiet after the driver closes the link.
The component end test separately checks withholds during the hold and gives for the released end.
The existing exit tests separately cover deferred exits and release.
The new tap test isolates the repeated-EOF defect; it does not claim a new composed real-process exit test.
Both changed tests PASS. The new tap test catches the withholds-to-false and gives-to-false mutants.

**B315-I2 is CLOSED.** The completion helper checks expiration after each pump that does not complete the requested operation.
The check applies when more is true and when more is false. The idle path retains its public wake wait.
No additional sleep, reap call, or derived wait appears.

**B315-I3 remains CLOSED.** The corrected design distinguishes an ordered EOF or read error from an explicit break.
The implementation, unit test, and design now agree on the held-end order.

Completed log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-e2-report-holds-29dc0efe-pool-20261012-095714-12815.log`.
SHA256: `2ccaf806491ddf58159b70ab4183dd4c7c6b1cf02276701275a161117346a46b`.
Sol-B reads its exact identity, base accounting, affected tests, raw mutation outcomes, citations, summaries, and exits.
The log retains the correct comparison and merge bases. All ten steps PASS in 873.8 seconds.
Default passes 1,960 tests; slow passes 622 tests. Strict conformance passes 261 with zero failure.
The new quiet proof, changed component proof, and all three real report-control proofs PASS.
Pending remains 406; real-pending remains 43. The repair changes no conformance file, pin, or mutation exclusion.
This review accepts no status removal suggested by pending-test output.

Stage 1 tests 159 mutants: 136 caught, ten missed, 13 unviable, and zero timeout.
The unmutated slow baseline passes 622 tests. Stage 2 catches the ten exact stage 1 misses.
Sol-B independently compares those missed and caught identity sets; they match.
The final result has 146 caught, 13 unviable, zero missed, and zero timeout.
All 146 unique raw catches match citations. Every catcher has a normal PASS.
No FLAKY or INCONCLUSIVE record appears. Unviable results supply no catch evidence.
Fuzz reports no changed decoder-harness crate.
The job and wrapper exit 0 after 882 seconds on msa1, with one second queued and 881 seconds running.

Earlier local-head log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-e2-report-holds-8d605032-pool-20261012-095336-3367.log`.
SHA256: `495460070fbecbb5d291e8b066c2eab39c0a33744ae718bcf83fe71a03db5b34`.
Sol-B reads the failure, byte assertion, summary, and exits. The 64-byte test read cannot contain the complete hello.
Default reports one failure. Slow, mutants, and fuzz do not run.
The job and wrapper exit 1 after 174 seconds. That gate remains failed.
Sol-B compares the local head with this READY head. Only four test read sizes change from 64 to 4096.
The expected frames, quiet assertions, and source repair remain unchanged. This structural fixture correction accounts for the failure.
It supplies no same-source retry claim.

Sol-B reads exact-head package verdict `cc1c7595740500c1b7c4f873ba4516782e0cff95`, its complete Round 2.
Both reviews close all three findings and agree on the completed gate and evidence scope.
The full body states the repair, corrected design, failed local gate, source and comparison bases, and completed current gate.
No source, body, or supplied-evidence finding remains on this exact head.
The Linux evidence covers the new controls through their current real adapter. This receipt claims no new Mac execution.
The current-base import or train, its applicable delta, final exact-head gate, and lead merge verification remain due.
This receipt accepts no later head, pin, status removal, or retry.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; exact repaired source and completed initial evidence)

## Round 3: #315 source delta on train 5

Exact head: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Tree: `42e00e9808db9faec524e2404f67302bee0d66c5`.
Base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Frozen #315 member: `29dc0efe0f817f729f62582d09a28a239c49272b`.
Lead assignment: `msg_plugin-w_1791828964_e1c322`.
The train merges #306 first, then #315. Sol-B owns both deltas and the combined review.

Sol-B independently compares all 18 own added/deleted patches against the landing merge. Every comparison matches.
Thirteen whole files match the accepted member. Five testkit files also retain the imported base's changes.
Sol-B reads their complete deltas and final control, receive, close, scheduler, quiet, and completion paths.
The final library retains queue_controls, report_holds, and #306's approved root macro.
Both control registrations and tapped real dispatch paths remain intact.
Control-room state remains separate from report holds, scheduler holds, and start holds.
The host's room check runs before link reads and replaces none of them.
PumpBound remains the first scheduler choice, once per pump. The report delay counts the same pump boundary.
The holder passes hello bytes before it withholds a report. RouteReports records only bytes returned to the driver.
The new room lookup therefore preserves the instance and consumed-report boundary.
The existing reports-to-faults and tap-to-holder lock orders remain. No new reverse acquisition appears.
Spawned identity association, already-reaped exit retention, explicit break, quiet repair, free-end accounting, and completion deadline checks remain intact.
The base's removed Writable observation needs no special ReportHolds branch; other decoded reports retain their generic classification.

#306's macro invokes the same root callee with caller-side compile-time arguments.
It changes no control state, process ownership, scheduler policy, or contract assertion.
All 28 new #315 tests remain in their existing library binaries and module paths.
The three real tests match real::slow_controls. The slow root filter excludes none of them.
The separate integration-target feature repairs do not remove these unit modules.
The complete test coverage mapping is in `verdicts/stage1/train-5.md` in this commit.

No conformance set, pin, or known-flaky policy changes relative to the train base.
The sole exclusion coordinate change still names LINK_CAPACITY at final core.rs line 35, column 33.
All three earlier findings remain closed under their reviewed source and document scope.
Sol-B reads exact-head package delta `f79ddfa7be40715be30942e326eb175693b028f5`, its complete Round 3.
It agrees with these source and interface checks and reports zero findings.
No #315 source delta finding remains on this head.
The original gate retains its original head and comparison base. The lead's pre-checks remain operator reports only.
The exact train gate, final body and receipts, and lead merge verification remain due.
This receipt accepts no later head, pin, status removal, or completed merge.
Sol-B ran no execution job.

VERDICT: CLEAN (0 open; #315 train 5 source delta only, final train evidence pending)

## Round 4: final #315 receipt on train 5

Exact head: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Tree: `42e00e9808db9faec524e2404f67302bee0d66c5`.
Gate and merge base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Frozen #315 member: `29dc0efe0f817f729f62582d09a28a239c49272b`.
Lead final request: `msg_plugin-w_1791831674_e19b66`.
Round 3's source and interface acceptance remains unchanged. All three earlier findings remain closed.

Completed gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-5-2683de7c-pool-20261012-112942-61568.log`.
SHA256: `bcb49fb7a9644f33b0a5f1b2dbc0e16dda3fb6cd84f1a4f291bfc2427375d536`.
Sol-B verifies the hash and reads exact identities, affected tests, coverage, accounting, raw mutations, citations, summaries, and exits.
All ten steps PASS in 1,747.7 seconds. Default passes 1,998 tests; ordinary slow passes 663 tests.
All 28 new #315 tests have normal PASS records in their expected tiers: 25 default and three real slow_controls tests.
The PumpBound, repeated-EOF, worker order, registry, and component proofs PASS.
The real break proof passes in 0.049 seconds. The real release and delay proofs each pass in 0.052 seconds.
Root presence and slow-only coverage pass. The coverage inventory includes all new #307 tests as recorded in the combined receipt.

Stage 1 tests 176 mutants: 147 caught, 14 missed, 15 unviable, and zero timeout.
The unmutated slow baseline passes 673 tests. Stage 2 catches the 14 exact misses.
The final result has 161 caught, 15 unviable, zero missed, and zero timeout.
All 161 unique raw catches match citations and normally passing catchers.
The non-checker source scope has 159 mutation identities: 146 caught and 13 unviable.
The repeated-EOF test catches withholds-to-false and gives-to-false. Their normal PASS records are present.
No runtime FLAKY or INCONCLUSIVE result appears. The stored EV-2 diagnosis is not a trial result.
EV-2 receives no mutation credit and passes ordinary slow and the unmutated baseline.
All four selected decoder fuzz harnesses complete. The job and wrapper exit 0 after 1,854 seconds on msa1.
This gate makes no new Mac execution claim.

Sol-B reads final package receipt `46e1a72a5ef550165fcd04c7f7ad257818bd4769`, its complete Round 4.
It agrees with these checks and reports zero findings.
Sol-B reads the full updated #315 body. It records this exact train, base, member order, gate log/hash, and completed results.
The original gate, local failed gate, buffer repair, and earlier evidence limits remain recorded with their original scopes.
The existing CONTROLS warning is recorded as a P6 follow-up. It changes no frozen source or accepted gate result.
The complete combined final record is Round 2 in `verdicts/stage1/train-5.md` in this commit.
No #315 source, final-body, or supplied-evidence finding remains on this exact head.
The lead must still verify the final merged tree and remote publication.
This receipt accepts no later head, pin, status removal, specific retry, or completed merge.
Sol-B ran no execution job.

VERDICT: CLEAN (0 open; final #315 receipt on the completed exact-head train 5 gate)
