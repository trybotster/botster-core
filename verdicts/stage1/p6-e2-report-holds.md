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
