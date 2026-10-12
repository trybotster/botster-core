# Integration review: real-pending re-entry (#323)

## Initial source and evidence inspection — corrected READY remains due

Initial READY head: `62877cbaeff1ffbc63e2aae7f159730cd700e47c`.
Parent and base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Tree: `e36aac3a6d5c4b4d7c6d9ac090bd1683f6d0f867`.
READY: `msg_plugin-w_1791831511_da8cec`.
Risk: HIGH, BUILD rule 1, CI list rule.
Reviewers: Sol integration; 017c package.
The complete matching package verdict remains due. This inspection completes no CLEAN round.

### Written authority and source

Authority receipt: `msg_plugin-w_1791831589_348cf8`.
The reviewer reads the lead's written decision at `shared/core-stage1/handoffs/core-lead.md:1580` and its context.
The lead approves a third real-pending entry path: ruling or lead-decision tag, diagnosed cause, and structural-fix owner.
The lead assigns this HIGH PR to 017c and Sol.
The existing paths remain: removal from core-pending in the same PR, or a new ledger id.
This rule does not alter the separate pin-move exception for core-pending.
R-109 and #298's future re-entry are follow-up work. No identifier enters a list in #323.

The reviewer reads the complete one-file diff and affected command, membership, and reader paths.
The initial diff has 225 additions and four deletions in `xtask/src/lists.rs`.
Full-index SHA-256: `bac493f39a346cb8ea743cb6851ee41b816bf447dd9c20e43291507dd4fca1a8`.
`re_entries` requires a tag directly above a nonempty, non-comment id line.
Every line that starts with `# re-entry` must parse as a tag.
Missing, empty, unknown, or repeated fields fail with the file and line number.
The parser checks the authority's stated syntax. Its fields retain the cause and fix owner from the comment.
The command parses both identifiers and tags from the same file text.
Identifier parsing still rejects duplicate identifiers.
`check_real` retains all ledger, pending, deferred, withdrawn, real-only, and testkit-proven restrictions.
The tag changes only the permitted-entry check against the base.
Initialization and the two existing entry paths remain unchanged.

The tag is a comment, so the conformance suite's `id_list` skips it.
The CI pending-real run still uses that suite. `slow_coverage` still uses `parse_ids`.
`base_merge` condition 4 still permits only removal of whole id lines.
A re-entry addition or a tag edit therefore fails that shortcut and requires a normal delta review.
The four readers, test-budget command, all conformance files, manifest, lockfile, and gate configuration equal the base.
No source defect is identified in the new rule or these integration paths.

### R323-SI-1 / GS-RP-R1-1 — LOW: initial body overstates explicit empty-field tests

Initial body SHA-256: `b86ca8bc233eec27d77a42c94b31af2652d17617f002c6039bcb30b9c47c9aaa`.
The body says the tests cover each field empty.
The initial test table has explicit empty re-entry and cause cases, but no empty fix case.
The shared empty-value branch rejects empty fix in source; this finding concerns the explicit test claim.
Package agreement: `msg_plugin-w_1791831767_5216ff`.
The reviewer requests accurate body wording or identification of the missing case, with no new execution requested.

The owner instead adds the empty-fix case in `6e6ebba7bc214ed9c55d3cecec70f8af1cdf2307`.
Correction receipt: `msg_plugin-w_1791831773_4a05ed`.
The reviewer statically reads this complete one-line test addition.
Its parent is the initial READY head; its tree is `d5271969e76e68e6b795101f793c759cebd3be56`.
The explicit coverage gap is corrected in that later source.
The correction's completed READY, updated body, gate, and matching package review remain due.
This inspection accepts no running gate or later-head execution.

### Completed initial gate

Log: `~/botster-sessions/gates/botster-core-stage1-st-real-pending-reentry-62877cba-pool-20261012-114724-10512.log`.
Verified SHA-256: `29414be170939d87adb7cf03b584f4e2d43501ea6b9226f410ab0592a260e443`.
The log names the exact initial head and base, four CPUs, and MS-A1.
All ten CI steps PASS in 478.3 seconds.
The job and wrapper exit zero after 637 seconds, with 73 seconds queued and 564 seconds running.
Default tests pass 1959, with 427 skipped, in 10.624 seconds.
Normal slow tests pass 633, with 2060 skipped, in 43.982 seconds.
Both strict summaries report 264 passed, zero failed, and 427 ignored.
The held-full record reports 96 ignored results and zero failures in 22.31 seconds.
Conformance counts remain pending 403, real-pending 34, ledger 691, and minimum 69.
The twenty-seven reported passing real-pending trials remain in the unchanged list.

Stage 1 tests 32 mutants: 30 caught, two unviable, zero missed, and zero timeout, in 71.6 seconds.
The reviewer matches all thirty unique raw catch identities to the thirty citations.
Every citation has a normal PASS record. Stage 2 does not run because Stage 1 misses none.
Both unviable replacements require an unavailable `Default` implementation and supply no behavioral catch.
The gate has no runtime FAIL, FLAKY, RETRY, INCONCLUSIVE, or TIMEOUT result.
Fuzz selects no changed byte-decoder harness.
This initial gate supplies no evidence for the later one-line test addition.

### Local records, reversals, and retained limits

The reviewer reads `st-reentry-local.log`.
It names the initial head and reports a target clean at `2026-10-12T18:33:37Z` with an owner statement about one writer.
Fmt and ten xtask checks exit zero.
The local clippy command exits 101 because testkit dead code is unused under that command's feature selection.
The local xtask run reports 321 passed and two timeouts, with exit 100.
The owner identifies the timeout tests as `base_merge::tests::an_unreviewed_change_after_the_merge_still_fails`
and `base_merge::tests::the_real_tier_pending_list_is_a_line_set_file`.
The owner reports `TIMEOUT [2.00x s]`, with no assertion failure retained.

The initial body also claims corrected clippy PASS and two isolated test passes in 1.7 and 1.6 seconds.
The owner states that those results were unretained terminal output and withdraws the claims.
The updated body must preserve that withdrawal and identify the local timeouts as `local, unclassified`.
`pair-common.md:113–117` explicitly requires this classification and prohibits timing findings from local Mac timeouts.
The exact-head pool gate decides the timing result. The reviewer opens no timing finding on these local outcomes.
Later local and gate records remain outside this initial completed-evidence scope.

The reviewer reads the reversal producer script and audits all four retained diff, failure, and summary records.
Each log names the initial head and an owner statement about the prior clean and sole writer.
Each tests forty list tests: 39 PASS, one intended FAIL, and numeric exit 100.
All forty named tests have normal PASS records in the initial gate.
The four edits remove tag acceptance, missing-field validation, authority validation, or next-line validation, respectively.
Their failure signatures match those removed checks.
The script restores the file and prints tracked status after each test run.
The logs retain no numeric restoration exit or restored-suite PASS. Their clean statement refers to the earlier clean.

Reversal SHA-256 values:

- r1: `a5e634951edac64287e6f2438ebe1c70cc830c649e9d4df842a5ec4b3ca595b9`.
- r2: `1f4178099ad3b627b869ca620a37475a6a61fe6b4465c9a1c1e38e318127ec54`.
- r3: `bd92dbfe2055be021e865ec9a0a82ab319825930eb9a37decd9ec50c7c9065a7`.
- r4: `94d51f9205203c897dc2bdb6d97f4b5d18bc80b3ceb2abbe2ba0bed4384185f7`.

The body has its required Prior art note and distinguishes the re-entry rule from its future #298 user.
No future re-entry, serving control, contracts fence, or list removal is accepted here.
Next: receive completed corrected READY, its updated body, and the complete matching package verdict after doorbells.
Applicable final import, exact-head gate, final receipts, and Q7 remain required.
The reviewer runs no build, test, gate, mutation, reversal, reproduction, measurement, or executable base check.
Never poll.
