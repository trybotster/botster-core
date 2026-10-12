# Train 8 integration review

## Round 1: combined source CLEAN

Exact train head: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
Tree: `3f6c95e20e26a54eca52908502a4bade7aec1cad`.
Base: `007efbde6a59af403d0937c38361c137a8961a48`.
Lead request: `msg_plugin-w_1791837736_3600b2`.
Sol-B owns the combined review and the single final train receipt.

The frozen merge order is:

| Member | Frozen head | Merge commit |
| --- | --- | --- |
| #325 | c3e1b41fb04a334eb9c5b864a6d3c68904743954 | ea89cbb8934ac6c5a4f0df70c635e2e364d1b600 |
| #327 | 0e25e27da6606a27cf2ff94df5d139c80f27b4d5 | 3d8b0ae29dc9da81f6114dac92a314d0da9e5c2b |
| #324 | 0572493fc9e695acd57b1916e485fdd900ba7643 | 5f8ad97b4effb96cd54d5500c444bf1b59ec7f5c |
| #329 | b9763b0f851a029a48af3a0f495388d69683f10b | 2d8e8acc52eca692a089af02b4ace508d6694cc3 |

Each merge has the prior train commit and frozen member as its parents.
The lead reports no conflicts. Sol-B verifies the merge objects and reads the complete applied source.
The source diff check passes. The train changes 26 paths, with 986 additions and 200 deletions.
Full-index diff SHA256: `08f5d55a7b100bddd1ee3551cc84157b4f5f88f77be654024e1a0f40bc74cf8a`.

### Exact-head review records

Sol-B reads each complete applicable package and integration delta on this train identity:

| Member | Package receipt | Integration receipt |
| --- | --- | --- |
| #325 | 99bd443eb4ee268fe79dba2209dfa079aed630a3, verdicts/p5-reply-order.md Round 3 | e29ab3b386361c6f1370b8a7b2fc8ca9326a5a6c, verdicts/stage1/p5-reply-order.md Round 3 |
| #327 | 8aad5e51a43050d09d9dea7be10f6fdaa599b7cf, verdicts/pb-b1-exit-watch.md, Train 8 source delta | a5abcf0f22c7e84d7403d6d53c6c36e9173bf002, verdicts/stage1/pb-b1-exit-watch.md Round 2 |
| #324 | 112d3985aea1eb9b25ea5e27fabbac9d82a52bfe, verdicts/gs-gate-speed.md, Train 8 #324 delta | a5abcf0f22c7e84d7403d6d53c6c36e9173bf002, verdicts/stage1/gs-dp5-fill-cost.md Round 2 |
| #329 | 112d3985aea1eb9b25ea5e27fabbac9d82a52bfe, verdicts/gs-gate-speed.md, Train 8 #329 delta | a5abcf0f22c7e84d7403d6d53c6c36e9173bf002, verdicts/stage1/gs-held-full-accounting.md Round 2 |

All four member source deltas are CLEAN. Earlier findings and their closure evidence remain in their member records.
The package reviewers grant no completed train gate acceptance.

### Independent composition and accounting

The #325, #327, and #324 insertions have exactly four, eleven, and five own paths.
Every applied added/deleted line set equals the accepted member patch, excluding hunk positions and object indices.
The #329 insertion has eight paths. Its four stacked #324 code paths are already present.
Seven #329 changed-line sets match directly; its held-header delta matches after subtracting the already-applied #324 metadata.
No additional path or changed line enters those comparisons.

Final member blobs remain exact in three of four #325 paths, ten of eleven #327 paths, three of five #324 paths, and eight of twelve #329 paths.
Sol-B reads all remaining overlaps and their affected final contexts.
The shared worker/tests.rs retains the complete reply-order test and exit_watch module declaration without altering a shared fixture.
The route-client test constructor retains the base's WorkerClock::real adaptation.
The CI source retains #308's step_names and fixture signature beside #329's exact held-full decision.
The lists source retains #323's re-entry parser and wiring beside #329's K calculation.

Sol-B independently checks all nine conformance text sets after each merge and against every merge-base candidate.
The final #329 merge has two candidates, 0572493f and 2683de7c. All 45 formulas pass.
Each result equals the prior parent set minus the member removals plus the member additions against that candidate.
No base removal returns, and no member addition disappears.
Only the three approved budget identifiers leave pending: 388 becomes 385, with no additions.
The held set has four identifiers with unchanged classes. Real-pending remains 38; ledger remains 692; minimum remains 69.
Real-only remains 26. The proof file retains seven exact rows for four identifiers.
Deferred text retains three rows, including its not-applicable case. Withdrawn retains 27 identifiers.
The deferred TOML, lockfile, nextest profiles, known-flaky list, and known-flaky parser equal the train base.
Every mutation-configuration value also equals the train base; only approved explanatory comments change.

### Combined lifecycle and interfaces

#325 establishes and tests the read-reply order without changing production read or output code.
Sol-B independently confirms that report, send_report, on_output, output_advanced, and report_output equal the frozen #325 member.
The complete worker model.rs blob also remains exact.
The resize-aware World fixture adds no resize result when this test feeds only PTY output and ReadScreen.
With instant_link false, it still withholds LinkWritten. The pending Output, later revision, and final Done assertions remain intact.

#327 adds the exit-watch failure input and failure action to the same worker machine.
Its separate tests use their own World instances. It replaces no read or output function.
The failure produces no fabricated payload exit or successful read completion.
The owned unreaped leader still protects the process-group identity through SIGKILL and payload drop.
The driver propagates ExitFailure, writes the errno to stderr, and exits non-zero.
The testkit sends the same machine input, closes the worker's transports, and drops its payload through existing cleanup.
The host's existing drain and classification path supplies Lost(WorkerGone) and completes the pending Stop.
PB-W1 remains outside B1 and scheduled for B3.

#324 caches the same accepted PTY input bytes as lowercase hex under the existing control mutex.
Both append sites and all three response callers remain present.
The real route encodes its byte log before calling the same response helper.
That change affects no exit-watch state, payload ownership, output observation, read completion, or real-resume history.
The new real-resume fences keep distinct final-byte titles and complete ReadScreen for s2, then s1, before oracle reads.
The base's SizeChanged capture exclusion preserves those comparisons and the later resize exclusion proof.

#329 changes only the approved held-full proof and accounting.
Its exact matrix requires every budget identifier at seeds 0-31 and uses the existing two-second per-trial limit.
Failure, missing evidence, or excess time fails the slow step under the lead's approved condition-1 reading.
The runner retains fresh JUnit evidence, process bounds, tracker cleanup, and the existing profile policy.
The ordinary testkit's ignored budget: held-full trials earn no ordinary transcript PASS.
The separate pending: held-full trials supply the proof. Real-tier trials remain must-pass.
Only dp_5_full is in the minimum intersection, so K is one. Re-entry tags cannot supply a held-full proof or change K.
The full combined xtask delta adds no new check outside that approved scope under plan 23co.

### Contracts and execution limits

The train retains the base's .46 pin at `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1`.
Sol-B independently verifies that the three held transcripts and the codec byte module are unchanged from .44 to .46.
Fresh train evidence must still prove the held-full matrix with the current clock and pin.
The #325 receipts separately verify that the three old R-114 st_6b transcripts remain unchanged through .46.
Contracts #97's later ReadScreen/Completed fences occur in .49. This train claims no execution of those future fences.
The requirement for #325 before the pin that takes #97 remains applicable.

The train retains the base's structured IN-8 known-flaky entry, owned by PC and removed by #321.
It does not restore the retired EV-2 entry from older member evidence.
That entry changes no normal-run requirement and supplies no automatic matcher or specific retry authorization.
Earlier member gates, failures, reversals, and local measurements retain their exact heads and scope limits.

The lead reports ID-set PASS, locked workspace compilation with tests PASS, and strict seeds 0-31 at 281 passed and zero failed.
The reported clean time is 2026-10-12T20:37:58Z. These are operator pre-check reports, not a completed train gate.
No source-composition finding remains, including LOW.
Under BUILD Green, Sol-B will send one final train receipt naming the completed gate path and SHA256.
Unchanged members require no separate package final receipt.
The exact-head train gate, final body updates, single final receipt, Q7, equal-tree merge, and remote checks remain due.
This source CLEAN accepts no gate, later head, completed merge, future pin, or additional retry.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; combined source only, final train receipt remains due)

## Round 2: single final train receipt

Exact train head: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
Tree: `3f6c95e20e26a54eca52908502a4bade7aec1cad`.
Base: `007efbde6a59af403d0937c38361c137a8961a48`.
The frozen members and merge order remain those in Round 1.
Lead final-receipt request: `msg_plugin-w_1791839154_6f48a8`.

Completed gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-8-2d8e8acc-pool-20261012-135145-99660.log`.
Computed SHA256: `11fefd182f9023abc4995140fc5ec3b8aadd8c638c489328b3ff57eb290c513d`.
The header names this exact train head and base. The base is an ancestor of the train head.
The source and tree identities remain unchanged since the combined source review.

All ten steps PASS, with a total of 750.9 seconds.
The job exits 0 after 830 seconds on msa1: one queued and 829 running.
The wrapper exits 0 after 831 seconds.
Default passes 2095 tests, with 412 skipped, in 13.303 seconds.
Ordinary slow passes 680 tests, with 2087 skipped, in 44.183 seconds.
All 23 default and 25 ordinary slow binaries pass the exact build-root test.
The static name check scans 254 Rust files; slow coverage checks 1101 slow-only tests.
All seven named real proofs pass in the ordinary slow run.
The read-order proof passes in 0.004 seconds; the real-resume proof passes in 0.099 seconds.
All ten B1 tests pass, including the testkit Lost/Stop proof and six exact-action tests.
The cache regression and held-full decision, naming, accounting, and profile regressions pass.

Strict conformance reports 281 passed, zero failed, and 385 pending identifiers including 70 without transcripts.
The minimum report is testkit-passing 69/69, of which one budget-held.
The real report is real-passing 67/68 and real-accepted 68/69.
This is not completion of the real minimum or Stage 1.
The gate changes no list membership beyond the reviewed source.

The held-full run passes all 96 trials, with zero skipped, in 21.980 seconds.
Sol-B independently checks every identity: each of the three budget identifiers has exactly seeds 0-31 once.
The printed maxima are:

| Identifier | Maximum | Seeds at maximum |
| --- | --- | --- |
| conf::dp_5_full_input_queue_stops_reading_the_route_and_never_drops | 0.280 s | 10 |
| conf::dp_5_unstarted_terminal_query_frame_retires_when_the_query_resolves | 1.257 s | 1 and 23 |
| conf::ev_8_every_query_stays_in_output | 1.255 s | 16 |

Every trial is below the existing two-second limit.
The dp_5_full maximum is also below the lead's 1.5-second condition.
This completes the train's fresh .46 held-full evidence; no older member gate substitutes for it.

Mutation stage 1 tests 70 distinct identities: 63 caught, two missed, five unviable, and zero timeouts.
The unmutated stage 2 baseline passes 690 tests, with 2918 skipped, in 45.890 seconds.
Stage 2 catches exactly the two stage-1 misses:

- SessionInput::log replaced by Ok(Default::default()), caught by conf::am_2_one_admission_point_orders_all_sources.
- Driver::run replaced by Ok(()), caught by slow_driver::a_client_close_ends_its_real_route.

Final results are 65 caught, five unviable, zero missed, and zero timeouts.
Sol-B independently matches all 65 raw catch identities to the 65 citations.
Every cited catcher passes in the ordinary default or slow run before mutation execution.
The raw stage-2 catch set equals the raw stage-1 miss set.
No runtime FLAKY or INCONCLUSIVE result appears.
The IN-8 diagnosis appears in the structured exclusion report; it earns no catch credit.
The fuzz step selects no changed crate with a decoder harness and passes without a harness run.

Sol-B independently reads the complete updated #325, #327, #324, and #329 bodies.
They name this final gate, preserve earlier evidence heads and limits, and retain their frozen member heads.
The #329 body records the tied 1.257-second maximum at seeds 1 and 23.
The #325 body separates its aed0c8e1 evidence from this train and explicitly claims no .49/#97 transcript execution.
All member heads remain frozen. All final body updates are complete.
No new source or evidence finding arises from the completed gate.
The lead's Q7, equal-tree merge, and remote checks remain due.
No later head, completed merge, future pin, or specific retry is accepted.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

This is the ONE final receipt for all four unchanged members and the combined train.
No separate package final receipt is required under BUILD Green.

VERDICT: CLEAN (0 open; final train 8 source and completed exact-head gate)
