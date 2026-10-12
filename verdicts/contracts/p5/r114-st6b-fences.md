# contracts #97 — p5/r114-st6b-fences (R-114 (1): a sync read before each `st_6b` `oracle_resume`), reviewer 017f

HIGH. Implementer: P5. Package reviewer: 016c. Base: main `67e36681`. One commit, `1c88e3b2`. It changes only three transcripts. No lint is added: the steward's R-115 froze the xtask checks, and checklist item 5 replaces the R-88 lint pair.

## Round 1: NOT CLEAN on 1c88e3b22986374c5a59554c30ea7a20d8a627e1 (1 finding: no gate)

- **The change.** `st_6b_cut_inside_an_sgr_sequence_applies_the_suffix_not_prints_it`, `st_6b_model_after_baseline_plus_output_equals_the_sessions_model` and `st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut` each get two steps between the last output fence (`await_title` and the `pump_collect` of its `TitleChanged`) and `oracle_resume`:
  - `begin ReadScreen {session: s1, history: false}`, bound to `$r_fence`;
  - `pump_until Completed {op: $r_fence, result: {ok: {screen: {…types…}}}}`, citing Core ST-6b and A2-1.
  The `screen` fields are those of `read.rs` `Screen` (text, rows, cols, history_unavailable, model_rev), in the same form as `a2_1_every_begin_operation_has_a_row_and_the_documented_ok_value` steps 16-17. The matcher allows extra keys.
- **The `oracle_resume` users.** `git grep oracle_resume` also lists `a8_2_offered_snapshots_still_satisfy_the_resume_invariant` and `st_6b_resume_invariant_holds_at_every_byte_offset…`. Both use `oracle_resume_every_cut`, which has no session and no live output. So these three are the only `oracle_resume` users, as the body says.

### The reviewer checklist (R-115), for the three changed transcripts

1. `route_accept` → `advance_clock`: no route in these transcripts. Not applicable.
2. `pty_output` on a gated route: no route. Not applicable.
3. Silence: no step asserts a silence event or its absence after the `advance_clock` at step 12. Not applicable.
4. Capture expiry: `CaptureSnapshot` (step 9) completes, and its `Completed` is consumed (step 10), before the `advance_clock {unix: +1}` at step 12. `capture_ttl` is 60 s by default (`limits.rs:146`), so the 1 s advance does not reach it. Met.
5. Output on S → `oracle_resume` on S: the new `ReadScreen` is pumped through its `Completed` after the last output's model fence and before `oracle_resume`. **Met.** Each fence title ends its output and occurs nowhere earlier in the session (`r62-st6b-sgr-2`, `T2`, `r62-st6b-dec-2`), so `await_title` fences the whole window under R-113. The Core side (the `DESIGN.md` rule that a read's reply follows the output reports of every applied byte) is in Core PR (2), which is not open yet. See the open item below.
6. `route_frames_taken`: none. Not applicable.
7. `fork_child` title: none. Not applicable.

### F1 (LOW): no pool gate on the exact head

The READY names only local checks ("conformance crates green, the 4 xtask --check runs exit 0"). I found no log for `1c88e3b2` in `~/botster-sessions/gates/`. A HIGH contracts PR needs a full pool gate on its exact head before CLEAN. Run `botster-gate --on linux` on `1c88e3b2` and name the log in the body. No change to the head is needed.

### Open item (not a finding)

- **Core PR (2).** R-114 makes this fence valid only with Core's `DESIGN.md` rule and its test. The body will name the Core PR when it opens. Per R-114, I check that the Core pin that takes these transcripts also carries that rule and its test.
- **Main** is `11b2a5a8`. `git diff 67e36681 11b2a5a8` changes only `docs/steward-rulings.md` (R-115), a rulings-only move.

## Final: CLEAN on 1c88e3b22986374c5a59554c30ea7a20d8a627e1 (F1 closed; the head is unchanged)

- **F1: closed.** The head has a pool gate: `~/botster-sessions/gates/botster-contracts-p5-r114-st6b-fences-1c88e3b2-pool-20261012-123542-17516.log`. My `shasum -a 256` of it is `d8a42c2379e1de2cb7bb17c7b4cd9712e90b43a6d83c5b55d38df565633f144d`, the value that P5 sent. All ten steps PASS, exit 0 after 438 s on msa1, base main `216ea74e`. The six `st_6b` fake checks pass, including the three changed ids.
- **Main** is `216ea74e`. `git diff 67e36681 216ea74e` changes only `docs/steward-rulings.md` (R-115 and its correction), a rulings-only move. No merge and no new gate is needed.
- **The Core side.** The body now names Core PR trybotster/botster-core#325 ("worker-core: a read's reply follows the observations of the output before it (R-114)", OPEN at `aed0c8e1`), with the `DESIGN.md` rule and the worker-core test `a_reads_reply_follows_the_observations_of_the_output_before_it`. Per R-114, the Core pin that takes these transcripts must also carry #325. I check that at the pin. It is not a condition of this contracts PR.

My finding F1 is closed. CLEAN on `1c88e3b22986374c5a59554c30ea7a20d8a627e1`.
