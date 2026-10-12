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

## Corrected READY inspection — matching complete package verdict remains due

Head: `6e6ebba7bc214ed9c55d3cecec70f8af1cdf2307`.
Source base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Tree: `d5271969e76e68e6b795101f793c759cebd3be56`.
READY: `msg_plugin-w_1791832396_87becd`.
The reviewer reads the complete corrected body, completed gate, local record, and pre-gate taint record.
The initial whole-change source audit and complete one-line test repair remain applicable.
Corrected own full-index SHA-256: `f3abcd282262b7f0db354b0ce5721832abc8ba80fda4babde976c7984313204d`.
One-line repair full-index SHA-256: `baa906de3eca318bcc4efabe9261d11e3791a9e3d3c33340037b19d9870fff28`.
The production part of `lists.rs` is byte-identical to the initial READY head.
The `.44` pin, lists, other readers, and existing eligibility checks remain unchanged.

R323-SI-1 / GS-RP-R1-1 closes in source and body.
The corrected test explicitly supplies an empty fix field and requires the `fix` error.
The body names that case as new since this repair.
Current body SHA-256: `938e3e7a0d964600495f39d64b36d9547af23cc92df7655d2a996603ff5d44b9`.
The body withdraws the initial unretained clippy and isolated-test claims.
It retains the original local failures and marks the local timeout as `local, unclassified`.
No timing finding applies under pair-common lines 113–117.
The four original reversals retain their original head, failure signatures, and restoration limits.
No new source, body, or evidence finding is identified.

### Corrected exact-head gate

Log: `~/botster-sessions/gates/botster-core-stage1-st-real-pending-reentry-6e6ebba7-pool-20261012-120242-14715.log`.
Verified SHA-256: `26cea9856ac0e7ca735b5851050c36668c46d882fa4f050ca9f7272cbb396c19`.
The log names this exact head and source base, four CPUs, and MS-A1.
All ten steps PASS in 200.6 seconds.
The job and wrapper exit zero after 537 seconds, with 326 seconds queued and 211 seconds running.
Default tests pass 1959, with 427 skipped, in 11.410 seconds.
Normal slow tests pass 633, with 2060 skipped, in 42.638 seconds.
Both strict summaries report 264 passed, zero failed, and 427 ignored.
Held-full reports 96 ignored results and zero failures in 22.09 seconds.
All conformance files remain unchanged. No additional removal follows from passing pending-real trials.

Stage 1 tests 32 mutants: thirty caught, two unviable, zero missed, and zero timeout, in 54.7 seconds.
The whole mutation step takes 57.5 seconds.
All thirty unique raw catches match the thirty citations. Every citation has a normal PASS record.
Stage 2 does not run because Stage 1 misses none.
Both unviable replacements lack a `Default` implementation and supply no behavioral catch.
The log has no runtime FAIL, FLAKY, RETRY, TIMEOUT, or INCONCLUSIVE result.
Both locally timed-out base-merge tests have current gate PASS records.
Fuzz selects no changed byte-decoder harness.

### Corrected local records

`st-reentry-local-6e6ebba7.log` SHA-256: `b71185bff5dd347f41c8bea758c360292e4e054b38fbb9eb87c03f99fc9b5523`.
The complete record identifies this head and an owner statement about the prior clean and sole target writer.
Fmt, clippy with Core/testkit/xtask and slow features, and lists have numeric exits zero.
All 323 xtask tests pass in 6.182 seconds, with exit zero.
The later two-test run has one PASS and one timeout at 2.003 seconds, with exit 100.
The timeout is `an_unreviewed_change_after_the_merge_still_fails`.
`the_real_tier_pending_list_is_a_line_set_file` passes in 1.855 seconds.
The body accurately retains this local, unclassified result. The pool gate decides the timing outcome.

`st-reentry-ci-taint-6e6ebba7.log` SHA-256: `c0a31b3612029a425ad815170b9d05c9e489a2f539b6be538dc241f6a06df878`.
The complete record identifies this head and the actual CI taint step.
The step passes in 29.9 seconds and exits zero.
These local records do not replace the exact-head pool gate or establish an independent target-writer history.

The reviewer reads complete package initial inspection `1f0f77ddee0a8e5ef30b6294abf293fa6d0f3d8f` and matching receipt `17fdaa0e46e6ecb937786d446ed9617c6ac346d1`.
Their source repair conclusion and initial evidence limits agree with the prior integration inspection.
The complete package verdict on this corrected READY remains due. This inspection completes no CLEAN round.
The current-base final slot, applicable import review, final evidence, final receipts, and Q7 remain required.
This review accepts no future re-entry, serving control, pin, removal, or merge.
The reviewer runs no execution job. Wait for the package verdict doorbell. Never poll.

## Round 1 — corrected complete READY CLEAN

Head: `6e6ebba7bc214ed9c55d3cecec70f8af1cdf2307`.
Source and gate base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Tree: `d5271969e76e68e6b795101f793c759cebd3be56`.
The complete corrected inspection at `4b8de4aea59ecc7f6d5bcffa9a74c4008703a47a` remains applicable.
No source or body changes after that inspection.

The reviewer reads the complete applicable package section, `Core #323 — corrected complete package verdict`, at `f0a5525aefc42ee0ae38232fc7d63833c53cd280`.
Package file: `verdicts/gs-gate-speed.md`.
Receipt: `msg_plugin-w_1791832558_8068c8`.
The package verdict is CLEAN on the same exact head, source base, gate base, and tree.
Its complete source review, body corrections, local classification, gate audit, and original evidence limits agree with this inspection.

R323-SI-1 / GS-RP-R1-1 LOW is closed.
The explicit empty-fix test, accurate body, withdrawal of unretained claims, and completed corrected gate satisfy the remaining conditions.
No integration source, body, or evidence finding remains open.
All thirty raw catches match citations and normal PASS. Both unviable replacements supply no behavioral catch.
The original local failures and reversals retain their original scopes and limits.
The local Mac timeouts remain unclassified under the binding rule; no timing finding is opened.

The lead's third entry path is implemented without changing the other readers or eligibility restrictions.
The parser checks authority syntax and field presence. Each future re-entry still requires review of its actual authority, diagnosed cause, and fix owner.
This rule supplies no passing proof for a transcript. No identifier enters or leaves any list in this PR.
#298's entry, R-109's fence, its serving Core control, and later removal remain separate work.

Both corrected-head reviews are CLEAN.
The current-base final slot, applicable import review, final evidence, matching final receipts, and Q7 remain due.
This verdict accepts no later source, pin, list change, retry, or merge.
The reviewer runs no build, test, gate, mutation, reversal, reproduction, measurement, or executable base check.

VERDICT: CLEAN (0 open; corrected initial READY, final delivery checks remain due)
