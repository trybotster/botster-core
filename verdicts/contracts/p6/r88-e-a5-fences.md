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
