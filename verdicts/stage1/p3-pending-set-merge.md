# Integration review: #190 (base-merge-check: the line-set rule for core-pending.txt; branch stage1/p3-pending-set-merge)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head 5ee3da3e

Reviewed head: `5ee3da3ee11e3675978ea51c3c31679962a84431` (`83df54b0` the change, `5ee3da3e` the merge of v1 `3000ae14`,
which is the current v1). Two files: `xtask/src/base_merge.rs` and its tests. The stated tier is HIGH by rule 1
(gate-decision code), which is correct.

### Checked, no finding

- **The merge.** Its tree is the tree of `git merge-tree --write-tree 83df54b0 3000ae14`.
- **`removed`** matches the side to `old` in order. It accepts the side only when every unmatched line of `old` is an id
  line and the side has no line left over. So an accepted side is exactly `old` without a set of id lines. `old` has no
  repeated id line (checked first), so the removed set names exact positions. The keys are `old`'s own line slices, so a
  last line with no newline compares the same on both sides. A reorder, an edit, an added line, a comment change or a CRLF
  line fails. All of these fail closed.
- **`set_merge`** refuses an id that both sides remove, and it rebuilds the expected file from `old`'s lines.
- **Conditions 2 and 3.** Condition 2 drops only the `SET_FILES` paths from the overlap. Condition 3 excludes them with
  `:(exclude,literal)` pathspecs in `diff-tree`. The four `GIT_*_PATHSPECS` variables are removed, so the exclusion cannot
  be read literally and empty the diff. The real-repo test `an_unreviewed_change_after_the_merge_still_fails` proves both
  sides, also with `GIT_LITERAL_PATHSPECS` set.
- **Condition 1** still counts a conflict in the set file. Two removals of adjacent lines conflict in git, so such a pair
  still needs a delta round. That is conservative, and this reviewer does not count it.
- **The gate log** (`…-p3-pending-set-merge-5ee3da3e-pool-20261009-095726-42960.log`) names the head and base `3000ae14`.
  The default tier runs 955 tests and the slow tier 243, all pass. Mutants: 32 caught, 0 missed, 0 timeout. Exit 0.

### R1 LOW — condition 4 fails a merge that the old rule passed, when only one side changes the file

`set_line` runs `set_merge` whenever the file exists at the four commits. It does not first check whether both sides
change the file. So condition 4 now also judges a file that only one side changes, and it accepts only pure removals of id
lines there. Two cases that passed before this PR now fail:
- The pull request does not touch `core-pending.txt`, and v1 changes a comment line in it. #188 did this (its two A15
  comment lines). At the head, the base side "is not a pure removal of id lines", so every open pull request fails
  base-merge-check and needs a delta round. Before this PR, condition 2 found no shared path, and condition 3 compared the
  file byte for byte. That passed.
- The pull request adds an id back to the pending list (a HIGH change, plan 23a), and v1 does not touch the file. At the
  head, the pull-request side "is not a pure removal". Before this PR, it passed by conditions 2 and 3.

This is not a false pass, but it turns a merge with no shared change into a delta round. Because of that, the rule costs
rounds that it was written to save. Fix: when one side leaves the file as `old`, require that the new head's file equals
the other side's file (the old byte rule). Use the set rule only when both sides change the file. Add a test for each of
the two cases.

VERDICT: NOT CLEAN at 5ee3da3ee11e3675978ea51c3c31679962a84431 (1 open: R1 LOW)

### Addendum to round 1 (same head 5ee3da3e): the P3 package reviewer's F60, missed here

The P3 package reviewer's F60 HIGH is real, and it is in this reviewer's scope (gate-decision code). Condition 3 excludes
the whole path of a set file, and condition 4 reads only its blob text (`text_at`: `ls-tree --name-only`, then
`cat-file blob`). So nothing compares the tree entry's mode or type. A commit after the merge that changes only the mode of
`core-pending.txt` (100644 to 100755) passes all four conditions: a false PASS. This reviewer checked the text rule and
the pathspec exclusion, but not what the exclusion drops besides the text. Fix: F60's (compare the mode and type of each
set file at the four commits, refuse a change or a non-regular entry, and add a real-repo test for each side and for the
new head).

VERDICT: NOT CLEAN at 5ee3da3ee11e3675978ea51c3c31679962a84431 (2 open: R1 LOW here; F60 HIGH, the package reviewer's
finding, confirmed here)

## Round 2 — CLEAN on head 5a0b07ac

Reviewed head: `5a0b07ac0575743977c5d45ae78ae0a6a55ec101`, one commit on `5ee3da3e`. The base is still v1 `3000ae14`.

- **R1 closed.** `judge_set` compares whole entries (mode and text). If the reviewed head leaves the file as `old`, the new
  head must hold the base's entry. If the base leaves it as `old`, the new head must hold the reviewed entry. Only a
  change on both sides goes to `set_merge`. So the two cases of round 1 (a base comment edit, as in #188, and a pull request
  that adds an id back) pass again by the byte rule. A new head that differs from the one side that changed still fails.
- **F60 closed.** `text_at` reads the entry's mode and type from `git ls-tree` (`100644 blob` from `<mode> <type> <id>\t<path>`),
  and reads the text only for a blob. `judge_set` requires `100644 blob` at all four commits before any other rule. A mode
  change, a symlink (`120000`), a submodule (`160000 commit`) or a directory (`040000 tree`) fails. The real-repo test
  `a_real_mode_change_of_the_set_file_after_the_merge_fails` covers the false pass of round 1.
- **Tests.** `a_one_sided_change_needs_only_the_other_sides_file`, `a_set_file_that_is_not_a_regular_file_at_any_commit_fails`,
  and the real-repo `a_real_merge_where_only_the_base_edits_the_set_file_passes`.
- **The gate log** (`…-p3-pending-set-merge-5a0b07ac-pool-20261009-100828-95489.log`) names the head and base `3000ae14`.
  The default tier runs 959 tests and the slow tier 243, all pass. Mutants: 39 caught, 0 missed, 0 timeout. Exit 0.

VERDICT: CLEAN (0 open) at 5a0b07ac0575743977c5d45ae78ae0a6a55ec101
