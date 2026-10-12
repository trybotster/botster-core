# #322 FIFO release integration review

## Round 1 — source and READY evidence CLEAN

Head: `25fbc9ec7bae580d28e3b29f7f43f0d6e0ff78b2`.
Tree: `f72531499b9521e57e15810e4e4f0b732911e523`.
Parent and merge base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Gate comparison base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
READY receipt: `msg_plugin-w_1791832823_287b9b`.
The lead assigns package reviewer 0167 and Sol despite the LOW risk tier.

The reviewer reads complete package Round 1 at `c88d002dbf65a5a3d9e97d615a7a389fa03c8b4f`.
File: `verdicts/p5-fifo-release.md`. Receipt: `msg_plugin-w_1791833586_7ea8bb`.
It is CLEAN on this exact head and records closure of its one LOW body finding.

### Source and package interfaces

The complete parent-to-head diff changes only `crates/botster-worker/tests/common/session.rs`, with nine additions and six deletions.
Full-index diff SHA-256: `88188f1b09554a7c52bb50c333ce1921881fdb119d945f5cd94e51e46a3aaa19`.
The Git whitespace check passes.
The reviewer reads the complete changed test, `Blocker`, and the nearby descriptor test.

`Blocker` holds nonblocking FIFO read and write descriptors. Its `send` checks that the complete line was written.
Its `release` closes both descriptors. The former release could close the last FIFO ends before the payload opened its reader.
The new external `head -n 1` reads the count through a newline without requiring EOF.
The test retains the FIFO while the count waits for that reader.
The test then observes the second write result and the payload's `done` marker.
The marker follows the count-limited PTY read and closure of the received file.
The test checks the complete received prefix and final byte before it releases the FIFO.
Thus the marker and byte check prove consumption before release; the write result alone is not the consumption fence.

The test retains its cancellation counts, partial-prefix condition, next-write result, input bytes, and bounded reads.
The second FIFO still holds the payload for Kill, exit-signal, Remove, and guard cleanup checks.
The guard still owns cleanup. No wait loop, deadline extension, shared fixture change, or production behavior enters this patch.
The worker binary and `slow_session` both compile the common test. Their inclusion paths remain unchanged.
The nearby descriptor test uses the same retained-FIFO pattern with an `alive` marker.
The owner's complete sweep table and prior-art statement remain owner reports.
The package reviewer inspects additional named sweep sites; this integration review does not claim a second complete repository inventory.
No scheduling reproduction campaign or reversal run is requested or supplied.

No host, testkit, sys edge, public API, dependency pin, or conformance list changes.
The parent still uses its older valid Launch signature. A future base import must retain that base's signature change and this FIFO fix together.
The review accepts no future resolution before its delta review.

### Completed READY gate

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-fifo-release-25fbc9ec-pool-20261012-121218-56646.log`.
Verified SHA-256: `a0b23cd755b5f7f1739365ff16fd2f4e76b85b1b90886d03f80e125e2df05389`.
The log names the exact head and comparison base above, msa1, and both concurrency caps at four.
The comparison base is not the PR parent. Its merge base with this head is the parent stated above.
This gate supplies READY evidence, not final current-base or train acceptance.

All ten CI steps PASS. CI total is 366.2 seconds.
The job and wrapper exit zero after 450 seconds: one second queued and 449 seconds running.
Default has 1957 passes, 427 skips, and 10.542 seconds. Slow has 633 passes, 2058 skips, and 41.867 seconds.
The reviewer independently parses every reported PASS ordinal and test identity in both sections.
Each section has its complete ordinal set and unique test identities.
The log contains no FLAKY, failure, timeout, panic, SIGSEGV, or SIGABRT token.
The changed test passes in `slow_session` at 0.013 seconds and in `slow_driver` at 0.012 seconds.
Both strict summaries have 264 passes, zero failures, and 427 ignored.
The branch's tier environment supplies seeds 0–31 to the strict/default checks and seed zero to the normal slow tier.
Held-full runs three budget identifiers at 32 seeds each: 96 ignored, zero failures, and 22.25 seconds.

The log retains core pending 403 and real-pending 34. It reports 27 passing ignored real identifiers.
Those identifiers remain real-pending; the log's removal suggestions authorize no removal in this review.
No conformance status changes in this patch.
Mutation selection finds no mutant and starts no run. Fuzz selection finds no changed decoder harness.
Neither step establishes a new mutation catch or fuzz execution.
The supplementary local Mac observation establishes no strict, slow, or timing gate acceptance.

### Body finding and disposition

R322-F1, LOW, required the body to replace `Gate: pending` with the completed exact-head gate and its scope.
Package notice: `msg_plugin-w_1791833164_63f259`.
The reviewer reads the complete corrected body on the unchanged head.
Body SHA-256: `8289aea29a56970846ce59204773c5416d060baf02af994fe575e1932beed390`.
It states the gate path, hash, passes, times, empty mutation/fuzz selections, and comparison-base distinction.
It states READY-only scope and labels the local Mac result as supplementary observation, not gate evidence.
R322-F1 is CLOSED. The body change requires no product change or new run.

Zero integration findings remain, including LOW. The package and integration reviews accept source and completed READY evidence.
The applicable current-base or train review, completed merging gate, final receipt, and Q7 remain due.
This verdict accepts no future head, final gate, retry, or merge.
The reviewer changes no product code and runs no build, test, gate, mutation, reversal, reproduction, measurement,
or executable base check.

VERDICT: CLEAN (0 open; source and READY evidence only)

## Round 2 — train-7b source delta CLEAN

Train head: `007efbde6a59af403d0937c38361c137a8961a48`.
Train tree: `6dbc71d0ded09ba4a27bd30e501b0fd61315fa13`.
Train base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Frozen #322 member: `25fbc9ec7bae580d28e3b29f7f43f0d6e0ff78b2`.
Lead request: `msg_plugin-w_1791836260_58aa9f`.
The lead assigns Sol the #322 integration delta only. Sol-C owns combined review and the one final train receipt.

The reviewer reads complete matching package Round 2 at `9e1880e3c1d423ef28396ed0c836c0ce9c56db2d`.
File: `verdicts/p5-fifo-release.md`. Receipt: `msg_plugin-w_1791836483_bc474e`.
It is CLEAN on this exact head, base, and tree. The initial source review at `41b5329f125fe16a57b31d69abbd587ee7749620` remains applicable.

The ordered merges are #316, #323, #302, and #322:

| Member | Frozen head | Merge commit |
| --- | --- | --- |
| #316 | `f1ac08b998f15f062f123a996e2688344c33c3ba` | `e9485b94bfc2476be07cb1642ccec4a5437b9952` |
| #323 | `6e6ebba7bc214ed9c55d3cecec70f8af1cdf2307` | `cd9af56fd6b0d238ad606aae9fec3ab9b431ebbf` |
| #302 | `953aa2ba300df85bff1df7a8be15f5214b6ca67b` | `abdae8913136fe68f51121d95cf40b15149ced99` |
| #322 | `25fbc9ec7bae580d28e3b29f7f43f0d6e0ff78b2` | `007efbde6a59af403d0937c38361c137a8961a48` |

The reviewer reads each merge's exact parents. The final merge joins the #302 merge and frozen #322 member.
The pushed train head and remote v1 base match the review identities.
Complete train diff SHA-256: `7ecd274c9c350c483b5010dd5ab48e2c6025d174e85a7742881b5fdbb0e23029`.
It has eight paths, 278 additions, and 39 deletions.
The #322 insertion changes exactly its one session test file, with nine additions and six deletions.
Insertion full-index SHA-256: `e042bf8fc2b199fcca7e099b960e21252d367b399b7a0afa8be7eb7f539b80a8`.
Every added and removed line equals the accepted member patch, excluding hunk positions and object indices.
The Git whitespace check passes.

The reviewer reads the complete member-to-train shared-file difference and the final fixed test.
The full fixed cancel test equals the accepted member test exactly.
It retains the newline count reader, FIFO ends, Done result, done marker, byte equality, release, Kill, and Remove order.
The marker and byte check still prove consumption before release.
The only other shared-file change is train 6's real PTY resize test.
That test owns separate temporary state and retains its existing bounded FIFO and session waits.
It changes no cancel fixture, fixed test, or group cleanup.
All member-to-train shared-file changes exactly equal the imported base's shared-file changes.
No hidden change enters the merge.

The train pins `.46` at `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1` through #316.
The reviewer reads the complete core-contract event delta against `.44`.
It changes only RouteAdopted tag/owner types and their tests within that package.
The route-codec package has no change between these pins.
No cancellation result, byte-count, input, or cleanup interface used by this test changes.
#316's pin and exception decisions retain their assigned reviewers' scope.

The reviewer independently checks all nine conformance text sets after each merge.
#302 has two merge-base candidates; all nine formulas pass against both, for 45 comparisons in total.
Every final set after #322 equals the preceding merge. #322 changes no identifier or status.
Final counts are pending 388, real-pending 38, held four, ledger 692, minimum 69, real-only 26, real-proofs seven,
deferred three, and withdrawn 27.
The added A22 pending and ledger identifier belongs to #316. No removed identifier returns through #322.

R322-F1 retains its closure. Zero #322 composition findings remain, including LOW.
The lead reports id-set checks PASS, test-target compilation PASS, and strict conformance 281/0.
These are operator reports. No pre-check log or completed train gate is supplied in this round.
The original READY gate retains its original member head and pin scope.
Combined review, completed exact-train-head gate, Sol-C's one final receipt, and merge verification remain due.
This delta accepts no gate, future head, retry, status change, or merge.
The reviewer changes no product code and runs no execution job.

VERDICT: CLEAN (0 open; #322 train-7b source delta only)
