# P5 FIFO release review — #322

## Round 1 — 2026-10-12 — source and READY evidence

- Head: `25fbc9ec7bae580d28e3b29f7f43f0d6e0ff78b2`.
- Tree: `f72531499b9521e57e15810e4e4f0b732911e523`.
- PR parent and merge base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Gate comparison base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
- READY receipt: `msg_plugin-w_1791832823_0500cd`.
- Risk: LOW. One worker test file changes; no product code or shared fixture API changes.

### Source

The complete parent-to-head diff changes crates/botster-worker/tests/common/session.rs only, with nine additions and six deletions.
Its full-index SHA256 is `88188f1b09554a7c52bb50c333ce1921881fdb119d945f5cd94e51e46a3aaa19`.
The Git whitespace check passes.

I read the complete changed test, Blocker implementation, and the nearby descriptor test that uses the same fence pattern.
Blocker keeps nonblocking read and write descriptors open. Its send writes the count line, and its release closes both descriptors.
The former release could close the final FIFO ends before the payload opened its reader and discard the queued count.
The new head -n 1 reader completes at the newline without waiting for EOF.
The test retains Blocker until the payload consumes the count, the second write completes, the done marker arrives, and the received bytes equal the cancelled prefix followed by b.
The done marker follows the payload's count-limited read and output-file close, so the byte check and subsequent release have a real observation fence.
The second Blocker still holds the payload for the existing Kill, exit-signal, Remove, and guard cleanup checks.
No deadline, expected count, cancellation assertion, input byte, or cleanup requirement changes.
Both slow_session and the worker binary compile this common test.

I read the owner's complete sweep table and independently inspect its named Blocker sites in core-testkit real.rs, slow_process.rs, and driver_edges.rs.
Those examples retain the FIFO until an echoed line or done marker proves the read, or kill the held helper without releasing it.
In particular, real.rs sends running and reads that exact line before its later release.
The owner's repository-wide sweep remains an owner report; this review does not claim a second complete FIFO inventory.
No repeat campaign or red-on-revert run was requested for this scheduling race.

### Completed READY gate

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-fifo-release-25fbc9ec-pool-20261012-121218-56646.log`.
SHA256: `a0b23cd755b5f7f1739365ff16fd2f4e76b85b1b90886d03f80e125e2df05389`.
I read the completed 5918-line retained log and parse its result lists independently.
It names this exact head and comparison base. The comparison base is not an ancestor of this PR head; its merge base is the PR parent above.

All ten CI steps pass, with CI total 366.2 seconds.
The msa1 job and wrapper exit zero after 450 seconds, including one queued second and 449 running seconds.
Default reports 1957 passes, 427 skips, and 10.542 seconds. Slow reports 633 passes, 2058 skips, and 41.867 seconds.
Each reported passing ordinal appears exactly once with the correct denominator. No FLAKY token occurs.
The target passes in slow_session at 0.013 seconds and in the worker binary's slow_driver at 0.012 seconds.
Both strict summaries report 264 passes and zero failures.

The retained summaries keep strict pending at 316 of 333, with seventeen real-only identifiers not run and four held.
Core pending is 403, plus seventy no-transcript entries; real pending is 34, minimum is 69, ledger is 691, deferred is two, and withdrawn is eighteen.
Minimum testkit is 67/69; real passing is 66/68; real accepted is 67/69.
All 27 passing ignored real identifiers remain in the 34-entry real-pending set.
The three held budget identifiers run over 32 seeds each: 96 ignored, zero failures, 22.25 seconds.
No conformance list, status, API, transcript, or pin changes in this PR.

Mutation selection finds no mutant and starts no mutation run.
Fuzz selection finds no changed crate with a decoder harness.
Neither output is accepted as a new mutation or fuzz execution.
The supplementary local Mac result supplies no strict, slow, or timing gate acceptance.

### Body finding and disposition

R322-F1, LOW, required the stale Gate: pending entry to identify the completed exact-head gate and its limits.
I sent the finding to P5 in `msg_plugin-w_1791833164_d51ab2` and to Sol in `msg_plugin-w_1791833164_63f259`.
The owner's correction receipt is `msg_plugin-w_1791833266_79403a`.
I read the complete corrected body on the unchanged head and v1 PR base.
It states the exact log and hash, ten passes, durations, target passes, empty mutation and fuzz selections, comparison-base versus merge-base distinction, and READY-only scope.
It labels the local Mac run as supplementary and not gate evidence. R322-F1 closes before this round is published.

Zero package findings remain on this exact head.
This verdict covers source, the corrected body, and completed READY evidence.
Matching integration review and applicable current-base or train review and acceptance remain required.
This verdict accepts no final train gate, future head, or merge. The combined integration reviewer owns the final train receipt under the current Green rule.
I changed no product code and ran no build, test, gate, mutation, measurement, or executable merge check.

VERDICT: CLEAN

## Round 2 — 2026-10-12 — train-7b source delta

- Train head: `007efbde6a59af403d0937c38361c137a8961a48`.
- Train tree: `6dbc71d0ded09ba4a27bd30e501b0fd61315fa13`.
- Train base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
- Frozen #322 member: `25fbc9ec7bae580d28e3b29f7f43f0d6e0ff78b2`.
- Package initial CLEAN: `c88d002dbf65a5a3d9e97d615a7a389fa03c8b4f`.
- Matching integration initial CLEAN: `41b5329f125fe16a57b31d69abbd587ee7749620`.
- Lead delta request: `msg_plugin-w_1791836260_874ef0`.
- Integration scope assignment: `msg_plugin-w_1791836292_e842fb`.

This round reviews only #322's fit on this train head. Sol supplies this member's integration delta.
Sol-C owns the combined review and single final train receipt. No train gate is reviewed here.

### Composition and patch preservation

I read the ordered merge identities and the complete #322 insertion diff.
The four merges are:

1. #316 `f1ac08b998f15f062f123a996e2688344c33c3ba` at `e9485b94bfc2476be07cb1642ccec4a5437b9952`.
2. #323 `6e6ebba7bc214ed9c55d3cecec70f8af1cdf2307` at `cd9af56fd6b0d238ad606aae9fec3ab9b431ebbf`.
3. #302 `953aa2ba300df85bff1df7a8be15f5214b6ca67b` at `abdae8913136fe68f51121d95cf40b15149ced99`.
4. #322 at the train head, whose parents are that #302 merge and the frozen #322 member.

The #322 merge changes exactly its one own file: crates/botster-worker/tests/common/session.rs.
It adds nine lines and deletes six. Every added and removed line equals the accepted member patch, excluding hunk positions and object indices.
Insertion full-index SHA256: `e042bf8fc2b199fcca7e099b960e21252d367b399b7a0afa8be7eb7f539b80a8`.
The complete train diff has eight paths, 278 additions, and 39 deletions.
Its full-index SHA256 is `7ecd274c9c350c483b5010dd5ab48e2c6025d174e85a7742881b5fdbb0e23029`.
The Git whitespace check passes.

### Shared file and dependency fit

I read the complete shared-file difference from the accepted #322 head and the affected final contexts.
That difference adds only train 6's real PTY resize test. It changes no existing function or fixture in this file.
The added test owns its own temporary root, FIFOs, and session, so it introduces no shared test state into the cancel test.
The full cancel test, including its documentation, is byte-identical to the accepted #322 form.
Its head -n 1 reader, retained Blocker ends, Done result, done marker, received-byte assertion, release, Kill, and Remove stay in that order.
The marker and byte equality still prove count consumption before the final FIFO descriptors close.
GuardedSession launch and group cleanup remain unchanged; the Blocker fixture implementation remains unchanged.
The slow_session binary retains the same common-module inclusion and adds only the existing built_in_test macro check from the base.
The two inclusion paths continue to use the same test.

The train pins contracts .46 at `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1` instead of .44.
I read the core-contract package delta between those exact pins: it changes RouteAdopted tag and owner fields and their tests.
It changes no WriteInput, cancellation outcome, byte count, Screen, or cleanup interface used by this test.
The pin, gate-decision changes, known-flaky dispositions, and other members retain their own reviewers' scope.
This member verdict does not replace those reviews or accept their separate status changes.

### Lists and disposition

I independently compare all nine conformance text sets after each ordered merge.
Each equals the previous train state plus its member additions and minus its member removals from that member's merge base.
All 36 comparisons pass. Every set after #322 equals the preceding merge; #322 changes no identifier.
Final pending has 388 identifiers and ledger has 692. The new A22 pending/ledger identifier belongs to the pin member.
No base removal returns through #322.

Zero #322 source-composition findings remain. R322-F1 and the initial gate retain their previous closure and limits.
The lead reports successful id-set checks, workspace test-target compilation, and strict conformance at 281/0.
Those are operator pre-check reports, not a completed train gate or independent execution evidence.
The matching member integration delta, combined review, completed exact-train-head gate, and final receipt remain due.
Under the current Green rule, Sol-C sends the single final train receipt if this member stays unchanged.
This verdict accepts no final gate, future head, retry, or merge.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge check.

VERDICT: CLEAN
