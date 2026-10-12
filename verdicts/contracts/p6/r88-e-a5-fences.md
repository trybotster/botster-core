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
