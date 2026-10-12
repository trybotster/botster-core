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
