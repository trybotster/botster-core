# contracts #84 — p3/oraw-scope (R-62 follow-up, scope ruling: Q3's OPOST rule and the print rule cover every real-tier O_raw read), reviewer 017f

HIGH (BUILD.md "Risk tiers" rule 1): the PR changes the fence lint, which decides pass or fail. 016c reclassified it from STANDARD; the body now says HIGH. Package reviewer: 016c.

## Round 1: NOT CLEAN on b58c200d071fbb84b9e7e4e5ce6955ebfbed3ac5 (1 LOW)

One commit, `e61cb2e` on main `c973323f`, then two merges of main: `1191d54` (main `adb5e163`) and `b58c200d` (main `0be439d4f3b31a6abdffd91adc397b809b75af3c`). Main is now `12ebec28f9e00c6b84d3af817ab1e5e4df96461d`; `git diff 0be439d4 12ebec28` changes only `docs/steward-rulings.md`, a rulings-only move.

Checked, no finding:
- **The merges (Q7).** Each tree equals `git merge-tree --write-tree` of its parents: `1191d54` `f927e2b1…`, `b58c200d` `648bf605…`. Against main `c973323f`, the PR's own diff is still only `fence_lint.rs` (+43 −7 with the self-test) and one sentence in `docs/core-testkit-controls.md`.
- **The ruling** (main, R-62 follow-up "the scope of Q3's rule is every real-tier O_raw read", option (b)): Q3's lint rule and the print rule cover `oracle_resume`, `oracle_screen`, `oracle_modes` and `oracle_state`. The lint's match is exactly these four, for both the `pty_output` check and the print check, and the finding text names the read.
- **The four are the real tier's O_raw reads today.** Core v1 `crates/botster-core-testkit/src/real/oracle.rs`: `oracle_screen` reads O_raw, and `oracle_modes` and `oracle_state` read "the same O_raw". `oracle_encode` and `oracle_query_reply` use only the stored row's size, so they are not O_raw reads, and the self-test checks that `oracle_encode` is not flagged. The other oracle controls in transcripts (`oracle_cursor`, `oracle_notification`, `oracle_restore`, `oracle_graphics`) have no real-tier form in `real/`; `oracle_hyperlinks` decodes the capture or the route's baseline, not O_raw. So none of them is an O_raw read now. (Note for the steward: if one of them gets an O_raw real form later, the rule must name it too.)
- **The self-test** `every_o_raw_read_takes_the_output_and_print_rules`: for each of the three new reads, a `pty_output` with `0a` and a print with `0d0a` give R-60 at step 0, and a print with `0d1b44` gives none.
- **No transcript is flagged**: the gate's lint passes with no exception line added.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 174 s on msa1, base main `0be439d4`; the new test passes: `~/botster-sessions/gates/botster-contracts-p3-oraw-scope-b58c200d-pool-20261011-202625-10392.log`.

### F1 (LOW): the lint comment quotes the controls doc for a sentence that the controls doc does not have

`fence_lint.rs:1815-1817` says: the real-tier `oracle_screen`, `oracle_modes` and `oracle_state` read O_raw "(the controls doc: "Conditions 1 and 3 of `oracle_resume` hold for it")". No file in this repository has that sentence (`git grep` finds only this comment and the ruling). It is in Core v1, `crates/botster-core-testkit/src/real/oracle.rs:4-6`, the module documentation of the real tier's oracle reads. The controls doc's statement is the sentence that this PR adds to condition 4. Please cite the controls doc's condition 4 (this PR) and the R-62 follow-up scope ruling, or Core's `real/oracle.rs`, not a controls-doc quote. The ruling text on main repeats the same attribution; I tell the steward separately, and it does not block this PR.
