# contracts #85 — p3/r94-e4-1-ev-8-held-fill (R-94 and its follow-up: the e4_1/ev_8 held fill, and the refined R-93 lint), reviewer 017f

HIGH (BUILD.md "Risk tiers" rule 1): the PR changes `fence_lint/order.rs`, which decides pass or fail. Package reviewer: 016c.

## Round 1: CLEAN on 4f8ab836d9a181707033df39b35ba4131dadb452

Three commits: `165efab` (the 9 transcripts), `b78efba` (merge of main `3756a34cc2fae51c24c5efaae78f90a69d250a45`, #79) and `4f8ab836` (`order.rs` rules 1-3 with self-tests, and the exception lines). Main is still `3756a34c`.

- **The merge `b78efba` (Q7, a shared path).** `git merge-tree --write-tree 165efab 3756a34c` has one conflict, in `held_write.rs`'s list. The merge's tree differs from merge-tree only in that file. I checked the resolution as a set: the merged list (28 ids) is exactly the union of both sides, drops no id of the merge base, and is sorted. `core-fence-exceptions.txt` auto-merged.
- **The 9 transcripts against R-94's conditions.** For every fill write (14 windows in 9 ids), checked by script:
  - the step before is `pty_blocked {on, kernel_bytes: 1}`, and the fill is `repeat ["61", 40960]`;
  - the step after is `await_control pty_queued` with exactly `bytes $between [1, 40960]`, so the fence comes directly after the fill and before the query's `pty_output` and the reply's `route_send` (R-93's follow-up condition);
  - every bound is `add [$room, 40960]` (Q2), and no `pty_queued` value is used in a bound or an expectation;
  - the final input-log checks use F plus the reply's length (`$whole`: `add [$L, 40960]`, or `add [40960, 7]` in the parked windows);
  - `requires.controls` adds `pty_queued`; the ids were already `fake: structural` (R-12) and stay so, as `tm_6_begin` kept R-12 on #77.
- **The 3 parked-reply windows** (`e4_1_client_reply…` s86, `e4_1_reply_one_byte_over…` s55, `ev_8_admission…` s54) check `contains [[$a, $b], $in_ct]` with `$a` = fill + `62` + reply and `$b` = fill + reply + `62` (the driver's `contains` is array membership, `driver.rs:2437`), plus `hex_prefix $fill` and `hex_len_eq` F + 7. That is the R-94 follow-up's form: exact in content, each part once, either order.
- **`held_write.rs`** lists all 9 ids, so the R-74 form test checks them.
- **The exception lines.** All 14 e4_1/ev_8 R-74 lines go (13 `debt:` lines and `ev_8_fallback_started` s27's `exception:` line, as the follow-up says), and all 10 P3-owned R-93 lines go. No R-74 or R-93 line remains for these ids, and no `exception:` line is added (the follow-up: "the lint can prove these windows"). Five R-56 `unfenced capture` lines go, because each capture now follows an `await_pty` at its full count. The R-60 and RC's R-80 lines move by the inserted steps.
- **The lint (rules 1-3).** The rule logic is 016c's scope. At integration level: the red log (`shared/core-stage1/evidence/p3-r94/red-lint-4f8ab836.log`) shows the old lint code with the new self-tests fails 5 tests, including `every_core_transcript_keeps_the_fence_and_line_discipline_rules`, and the new code passes 76 of 76.
- **Core evidence** (Core v1 `bfef8865`, `.41` tree with each revision's transcripts, cleaned target, macOS): every id fails or is INCONCLUSIVE on both tiers at the same step before (main `3756a34c`) and after (`4f8ab836`), moved only by the inserted steps (step 18 for the four ids that fail before the fill; 27 → 29 for the five whose first window starts at step 25). In those five, the new `pty_queued` fence at s26 passes on both tiers before the failing query step. The two `queue_overflow` ids are UNSUPPORTED_CONTROL before and after. So these are Core gaps: the ids stay in `core-pending` until their Core PRs (R-94's owner line).
- **Gate PASS on the exact head**, all ten steps, exit 0 after 410 s on msa1, base main `3756a34c` (current main): `~/botster-sessions/gates/botster-contracts-p3-r94-e4-1-ev-8-held-fill-4f8ab836-pool-20261011-211145-14813.log`.

No finding. CLEAN on `4f8ab836d9a181707033df39b35ba4131dadb452`. If main moves before the merge by more than rulings, I re-check the delta per Q7.
