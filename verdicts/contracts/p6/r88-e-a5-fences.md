# contracts #75 — p6/r88-e-a5-fences (R-88 group (E): a5_1/a5_2 fences, s79 ruling (B)), reviewer 017f

HIGH (rule 1: the fence lint decides pass or fail; the ruling also calls the control change HIGH). Package reviewer: 016c.

## Round 1: NOT CLEAN on 9cac43b15d0f84d566b3135bc6d227b904201ba2 (1 MEDIUM, 1 LOW)

I reviewed `526c9aaf49d1c340a20dcdc8b70b0313d39c0c59` (one commit on main `3c68880b011fdb60f7bca8124eef350e2d4d4708`), then the delta to `9cac43b1` (the steward's condition on the lint, `fence_lint.rs` only). The ruling text is R-88 (E) and its s79 item on main.

Checked, no finding:
- **s21/s27 `held_reports` fences** after the `begin` of `Start` and `Stop` of s2, as the ruling asks. `requires.controls` adds `held_reports` (sorted).
- **s78 `await_title s12`** before the s80 `drain` (the old s76), and **s79 `route_reports {s12, $r12a}`**. s12's last print is `62626262` + `1b5d323b` `r88-a5-2-s12` `1b5c`. s76/s77 `bytes_hex` grow by exactly those bytes, verbatim. This is ruling (B).
- **a5_1 s41** `route_reports {s6, $r6}` after `route_expect_pty` and before the `drain`; `requires.controls` adds `route_reports`.
- **The exception line** moves from step 72 to 74: two steps are inserted before it, and s74 is the same first `route_send` to s12.
- **The lint (`9cac43b1`).** `program_last_print` takes only the last `print`/`print_after_input` item. That item must end with a title sequence (`ends_with_a_title_sequence`) that sets the title. The sentinel's step is the session's last host input before the wait, as the steward set it. The test has a green case and four red cases: a launch after the input, another session's program, another title, output after the title in the same print, and a title in an earlier print.
- **The `held_reports` row** counts both holds, and the `scheduler_delay` row names the holder on each tier, the count start, the fence and the `mixed_holds` refusal (`ControlError::Refused` exists, `harness.rs:54`).
- **P6's design-note question to me (s95/s114, now s99/s118).** A `Create`'s record is `Created` before and after its completion, so the `get` of `s14a`/`s14b` gives `Created` whether or not `await_get` pumped the held `Create` forward. Those steps need no change.

### F1 (MEDIUM): s98's `await_get` removes the one-pump order, but s101 still asserts it

- `scheduler_deadline before: true` (s94) is meant to post the deadline's event before the other events of that pump. P6's Core evidence (`run.rs` 276, `inbound.rs` 277-279) says that the stop-grace deadline pump only sends `Kill`. `Exited{Killed}` is posted only when the worker's exit report arrives, in a later pump or later in the pump.
- `Completed{$coa}` is ready in the s97 pump (`scheduler_ready`, s93).
- s101 is `expect_eventually SessionState s13a Exited{Killed}`, with `ignore` [`Activity`, s13a `Stopping`, s14a `Created`]. `expect_eventually` fails on any other item (`docs/foundation-design.md:186`). `Completed{$coa}` is not ignored. So s101 passes only if `Exited{Killed}` is posted before `Completed{$coa}`, which is the one-pump order.
- s98 now waits with `await_get`, which pumps and never polls. So the transcript no longer claims the order at s98, but s101 still requires it. By P6's own evidence, nothing makes it true. s101 fails, or passes by timing.
- The (b) half has the reverse problem. s120 (`expect_eventually Completed{$cob}`, `ignore` without s13b `Exited`) passes because the exit comes later, whatever `scheduler_deadline before: false` does. So the variation does not show that `scheduler_deadline` is the source.

The ruling says to use `await_get` "unless a clause requires the one-pump order. In that case the owner names the clause and uses a held-report fence." s101/s102 together with `scheduler_deadline` (A5-2, TM-3) are such a requirement. So either:
- (i) name the clause and use a held-report fence (the ruling's alternative); or
- (ii) get a steward ruling that changes what s101/s102 and s120/s121 assert about the deadline's place. For example, the event that `scheduler_deadline` places for this deadline could be the kill (`signals_sent`), not `Exited`.

Both ids are core-pending, so no run shows this yet. It is a transcript defect that the un-pend would hit.

### F2 (LOW): the `scheduler_delay` row does not state which pump Core first reads the report in

s23 (`get s2` is `Created` or `Starting`) needs the report still held after s22's `pump`. With `pumps: 2`, that holds only if the hold pump inside s21's `await_control` is pump 1, s22 is pump 2, and Core reads the report no earlier than pump 3. The row says "only after `pumps` pumps" and "the count starts at the pump in which the edge holds the report". The design note says that P6 would state this count in the row. Please state it directly: the hold pump is counted as the first, and Core reads the report in pump `pumps + 1` at the earliest. The same applies to s29.

### Next

- P6's planned merge of main `28e05487` (#74) is not disjoint: both sides change `docs/core-testkit-controls.md` (#74 adds the `op_posted` row). `git merge-tree` gives no conflict. I review the delta (Q7), then the ONE gate on the merged head.

## Round 2: NOT CLEAN on 910d7f2e455a8dbec437fd15fc89c363fe47876a (016c's MEDIUM open; F3 LOW new)

Delta from `9cac43b1`: `929798d6` (F2, the `scheduler_delay` row) and `910d7f2e` (F1, a5_2). The steward ruled F1 on main `15ca1f717906a188bbf01f69f14689ba1ecaa237`: a clause requires the order (`scheduler_deadline before`), so the fence is a held-report fence, and the order assertions stay. If Core cannot make the counted pump deterministic on the real tier, the id gets a `debt:` line, and the assertion is not weakened.

- **F1 closed.** For each half (s13a `before: true`, s13b `before: false`): `scheduler_hold $coa` (s93/s118, was `scheduler_ready`), then `scheduler_delay {s13x, exited, pumps: 1}`, then the deadline pump (s98/s123, the kill). Then `await_control held_reports {count: 1}` (s99/s124, the hold pump = pump 1), `get` `Stopping`, `scheduler_release` + `scheduler_ready $coa`, and one counted `pump` (s103/s128). Then `get` `Exited{Killed}` and `get` `Created`, and the order assertions s107/s108 and s132/s133, unchanged. The Create cannot complete during `await_control`, because it is held. Both the exit report and `Completed{$coa}` come in the counted pump, so `before` decides their order. This matches the ruling.
- **F2 closed** as I asked: the row says that the hold pump is pump 1 and that Core reads the report in pump `pumps + 1` at the earliest.

### F3 (LOW, new): the rows do not state two facts that s103-s108 now need

1. s104 (`get` `Exited{Killed}` right after the counted pump s103) needs Core to read the exit report IN pump `pumps + 1`, not only "at the earliest" (my F2 wording, which was enough for s23/s29 but is not enough for s104/s129). Please make the `scheduler_delay` row say that the edge gives the report at the start of pump `pumps + 1`, and that Core reads it in that pump.
2. s107 needs `Exited{Killed}` to come before `Completed{$coa}` in s103. But `scheduler_deadline`'s row places only "a deadline that is due in a pump", and no deadline is due in s103 (P6's own note). The row must say that the exit caused by a deadline's kill is that deadline's event, and that `before` places it in the pump in which Core reads the exit report. Without this, a harness that follows the rows can give either order. P6 sent this note to the Core PR, but the contract for both harnesses is this row.

### 016c's MEDIUM (fence_lint.rs ~1660), confirmed; my partial miss

The lint dates a program-printed sentinel at the session's last host input before the wait. That input need not trigger the print. With a title printed after input `31`, a newer `pty_output` without a title, and an unrelated later input `32`, a second `await_title` of the old title gets the date of input `32`. So it counts as a fence after the newer output, although no new title was printed. 016c ran the exact module, which returns `[]`. In round 1 I saw that the last input is the latest possible date, and not a safe one for this check, but I accepted it as the steward's definition. The fix must tie the sentinel to the input that triggers its print (`match_hex`) and to the output after it, and keep the ruled a5_2 green.

Still to do after the fixes: the merge of main `28e05487` (shares `docs/core-testkit-controls.md` with #74) and its Q7 review, then ONE gate on the merged head.

### Steward correction after round 2 (main 8ce40a0d): F3 part 2 withdrawn; the F1 form changes

The steward withdrew the F1 application that `910d7f2e` follows. Core E3-1 item 1 says that the stop-grace kill posts no event, and that `Exited` follows "under the normal rules". So **F3 part 2 is withdrawn**: `scheduler_deadline` must not treat the kill's exit as the deadline's event, because that contradicts E3-1. My F1 premise stays (s101 asserted an order that nothing sourced), but the steward's new form replaces the held-report construction:
1. a5_2's `scheduler_deadline` variations use a silence deadline: `Silent` against the other op's `Completed` in one pump.
2. The stop-grace steps use `await_get Exited Killed`, with no order claim.
3. The held-report construction leaves these steps.

**F3 part 1 stays** wherever s22/s27 use `scheduler_delay`: the row states the pump in which Core reads the held report. 016c's lint MEDIUM stays open. I review the next head against this form.

## Round 3: no open finding on 60c87f1e60d72d8c506ac620141508c69a1ec6ce (final CLEAN after the main merge, Q7 and the gate)

Delta from `910d7f2e`: `4d1772cb` (lint), `2c683294` (a5_2, the steward's new form), `60c87f1e` (F3 part 1).

- **F1, in the steward's new form (main `8ce40a0d`, lines 69-71): closed.** For each half: `set_silence_threshold 1000` on s13x after its one output byte is collected (s88/s114), `scheduler_ready` on the Create `$co*`, `scheduler_deadline {before}`, `advance_clock 1000`, `next_deadline` 0, and one `pump` (s97/s123). `Silent` is the deadline's own event (TM-4, E3-1 item 2), and the Create's `Completed` is ready in the same pump, so `before` decides their order. s100 (`Silent` first; it ignores only `Activity` and the s14a `Created` state) and s126 (`Completed` first; the same ignores) each hold only in the order that `before` sets. The stop-grace steps (s103-s109, s129-s135) use `await_get Exited{Killed}` with no order claim. The held-report construction is gone from them. `requires.features` is `["silence"]`.
- **F3 part 1: closed.** The `scheduler_delay` row says that the edge gives the report to Core at the start of pump `pumps + 1`, and that Core reads it in that pump.
- **016c's lint MEDIUM: closed on my side** (016c checks its own finding). `Program` follows each session's prints through the transcript. A `print` runs at the session's `Start`. A `print_after_input` runs at the host input that completes its `match_hex` in the probe's input log (even offsets only). An input of unknown bytes (a key, a non-`bytes` frame) places no later print. The sentinel is the last print, only after it has run and only if it ends with the title. A `pty_output` of the session after that step is not covered. The red cases include 016c's case (an old title, newer output, an unrelated input), an input that does not complete the match, and a key input. The green cases include a `print` at the start and a match split across two inputs. `input_hex` reads `WriteInput` bytes and `route_send` `bytes` frames, so a5_2's s74/s75 place s12's sentinel at s75.

Next: P6 merges main `8ce40a0d`. That merge is not disjoint: #74 changed `docs/core-testkit-controls.md`, which this PR also changes. So I review the merge delta (Q7), then the ONE gate on the merged head, and then the final CLEAN.

### 016c's round-2 MEDIUM on 60c87f1e (fence_lint.rs ~672), confirmed; I missed it, and my round-3 "no open finding" is withdrawn

`sentinel()` checks `ends_with_a_title_sequence(last) && sets_title(last, title)`. `sets_title` matches the requested title at any even offset (line 715), and `ends_with_a_title_sequence` accepts any title at the end. So a last print of title `t`, then `6162`, then title `u` passes `await_title t`, which can return before `6162` and `u` are processed. Required: the requested title's own sequence ends the last print. Round 3 is NOT CLEAN on `60c87f1e` until this closes.

### Round limit: the steward DECIDED (round 4, no split)

After three NOT CLEAN rounds (BUILD round limit), the steward decided as 016c and I recommended. #75 gets round 4 and is not split. The requirement (R-80 condition 3): the requested title's sequence must be the LAST bytes of the program's final print. Any title or other bytes after it make it no fence. P6 adds the red case. Then come the main merge, my Q7 delta review (`docs/core-testkit-controls.md` is shared with #74), and ONE exact-head gate.

The lead's decision (the same day) agrees: settle the point, no split, no replan; P6 fixes, merges main once, runs ONE gate, then round 4. If the ruling is a general rule, it also goes into P3's R-88 lint (#73), so that it cannot recur. #73 and #75 both change `fence_lint.rs`, so the second of them to merge carries the rule in both places. I check that at the second merge's delta review.

## Round 4: no finding on f0e356ff2507d6539ef529f169611741c1aef4be (final CLEAN after one more main merge)

- **Merge `0c9d3565` (main `8ce40a0d`), Q7 delta review: no finding.** Its tree `d6030574` equals `git merge-tree`. From merge base `3c68880b`, every one of the PR's five files has the same sorted +/- lines in `3c68880b..60c87f1e` and in `8ce40a0d..0c9d3565`. Every one of main's six files has the same lines in `3c68880b..8ce40a0d` and in `60c87f1e..0c9d3565`. That includes the shared `docs/core-testkit-controls.md`: the PR's two row edits and #74's `op_posted` row are all kept.
- **016c's sentinel-position MEDIUM: closed.** `sentinel()` uses `ends_with_title(last, title)`: the last bytes of the final print must be the requested title's own sequence (`OSC 0`/`OSC 2` + the title + `BEL`/`ST`). The hex has an even length, so the suffix is byte-aligned. The red case is title `t`, then `6162`, then title `u` in one print. This is the steward's R-80 condition 3 (main `c6287e0`). `f0e356ff` changes only `is_multiple_of` for clippy.
- Gate PASS on `f0e356ff`, all ten steps, exit 0 after 174 s on msa1, base main `c6287e00` (rulings-only over `8ce40a0d`): `~/botster-sessions/gates/botster-contracts-p6-r88-e-a5-fences-f0e356ff-pool-20261011-172249-85946.log`.

**But main has moved again, to `939e1acd2c16611da0e4b44c640921bcd40d3527` (#66 merged).** #66 changes `conformance/core-fence-exceptions.txt` (it removes two `dp_5` lines), which #75 also changes (the a5_2 R-60 line moves from 72 to 74). The merge is not disjoint, so main cannot fast-forward to `f0e356ff`. `git merge-tree` shows no conflict (tree `947c7831935e3567fa05812682f527461b6b9192`). P6 merges main once, I review that delta (Q7), and the ONE gate runs on the merged head. Then comes the final CLEAN.

### 8adbe93741e6c5528955271cb2284309c45e50ca (merge of main 939e1acd): Q7 delta review, no finding (final CLEAN waits for its gate)

Parents `f0e356ff` and main `939e1acd`. The tree `947c7831935e3567fa05812682f527461b6b9192` equals `git merge-tree`. From merge base `8ce40a0d`: the PR's five files have the same sorted +/- lines in `8ce40a0d..f0e356ff` and in `939e1acd..8adbe937`. #66's seven files have the same lines in `8ce40a0d..939e1acd` and in `f0e356ff..8adbe937`. In the shared `core-fence-exceptions.txt`, #66's removal of the `dp_5` lines and the PR's a5_2 step 74 line are both kept. Gate: running on `8adbe937`.

## Final: CLEAN on 8adbe93741e6c5528955271cb2284309c45e50ca

- Gate PASS on the exact head, all ten steps, exit 0 after 182 s on msa1, base main `939e1acd` (current main): `~/botster-sessions/gates/botster-contracts-p6-r88-e-a5-fences-8adbe937-pool-20261011-172748-2973.log`.
- The pushed PR head is `8adbe937`. The Q7 delta review of its merge has no finding (above).
- Every finding is closed: F1 (in the steward's silence form), F2, F3 part 1 (part 2 withdrawn per E3-1), 016c's stale-title MEDIUM and its sentinel-position MEDIUM (R-80 condition 3).
- Merge-order notes for the next mergers: #73 changes `fence_lint.rs` and `core-fence-exceptions.txt`, so #73's merge after this one gets a Q7 delta review. #73's a5_2 debt lines (steps 22, 27, 94, 113) and a5_1 step 44 line refer to the old step numbers and the old fences, so #73 must reconcile them with this PR.

### After the merge: a miss of mine, found by #73's lint (P3, 69301401)

a5_2 has two client sends on `r12a` (s74, s75). The s79 fence `route_reports {s12, $r12a} {activity_before_close: true}` is sticky after the first send, so it proves only s74's `Activity`. s75's report can still post after the s80 `drain`, so s83's `pump {events_posted: 0}` can race. I did not check the send count against the fence. #73 keeps s83 as a `debt:` line for P6 ("s75's send needs its own fence, or a ruling that s74's Activity covers it"). The id is core-pending, so no run is affected now. I told the steward.
