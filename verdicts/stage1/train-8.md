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
