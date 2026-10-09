# Integration review: #189 (conf::ev_4_exit_has_signal leaves pending; branch stage1/p3-flip-ev4)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 48928504

Reviewed head: `489285047b18f1d736a35e2a4adbda0f29f7903c` (`5099dca7` the pending removal, `5409483b` the list change,
`48928504` the merge of v1 `a14e9dc2`). The tier is HIGH by rule 3 (the shared testkit's `worker_transcripts.rs`), after the
P3 package reviewer's F59. That is correct, as for #185.

- **The merge.** `git merge-tree --write-tree 5409483b a14e9dc2` conflicts only in `worker_transcripts.rs`, and the real
  merge differs from the trial tree only there. The resolution is the union: v1's header (plan 23a, with the real-only
  exception, from #185) and `IDS` without both #185's three ids and `ev_4`. `IDS` is now `a2_8_terminal_identity_names_the_entry`
  and `lc_3_remove_created`, and the clause line names TI-1, LC-3 and LC-7, which are theirs.
- **The flip.** Against v1, `core-pending.txt` loses only `conf::ev_4_exit_has_signal`. Its replacement-map proof at
  `contracts-v0.1.19` is `core-testkit+edge` (process; "a real signal is a named slow test"), not `slow:*`. So plan 23a lets
  it leave pending on the testkit.
- **The gate log** (`…-p3-flip-ev4-48928504-pool-20261009-093335-59628.log`) names the head and base `a14e9dc2`. The
  conformance binary reports 20 passed and 655 ignored (v1's 19 and `ev_4`). The default tier runs 942 tests and the slow
  tier 243, and all of them pass. Exit 0.
- **The current v1** is `8bc21dd5` (#187 since the merge). `git merge-tree --write-tree origin/v1 48928504` has no conflict.

VERDICT: CLEAN (0 open) at 489285047b18f1d736a35e2a4adbda0f29f7903c

## Round 2 — CLEAN on head a5d64425 (delta: the merge of v1 8bc21dd5)

Reviewed head: `a5d64425bc57f0115598265794c49c7d7f82ce2c`, the merge of v1 `8bc21dd5` (#187) into `48928504`. v1 is still
`8bc21dd5`. Base-merge-check cannot carry the CLEAN, because `core-pending.txt` changed on both sides (#187's three flips
and `ev_4`).

- The merge's tree is the tree of `git merge-tree --write-tree 48928504 8bc21dd5` (`4d790ccc`): no conflict, and no
  change by hand.
- The PR's own diff on the new base (`git diff 8bc21dd5 a5d64425`) is the same as on the old base, apart from the `index`
  and hunk lines.
- The gate log (`…-p3-flip-ev4-a5d64425-pool-20261009-094159-89893.log`) names the head and base `8bc21dd5`. The conformance
  binary reports 23 passed and 652 ignored (v1's 22 and `ev_4`). The default tier runs 947 tests and the slow tier 243, and
  all of them pass. Exit 0.

VERDICT: CLEAN (0 open) at a5d64425bc57f0115598265794c49c7d7f82ce2c
