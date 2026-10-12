# Contracts #73 (`p3/r88-lint`): integration review (R-88 lint and R-80 list growth, P3)

Reviewer: contracts integration reviewer (017f). Implementer: P3 (`sess-1791517264-0164-de59be35e09a9d1858798a29a4f8e2b1`). Package reviewer: 016c. Ruling: R-88 "The sweep rulings" (pending: item 5 of `~/botster-sessions/shared/steward/drafts/pending-rulings-edits.md`; it governs now). Tier: HIGH.

## Checks for the READY head

- The lint finds the racy-read shapes of sections A-E: parked work on a full queue (A), consumption or posted-op reads (B), state reads after a worker report (C), `ev_8` output before `queue_overflow` (D), and `scheduler_delay` counts (E).
- The lint counts a `pump` with an `events_posted` or `more` expectation as a report-derived read (the "reads outside the scanner" rule).
- Each racy id is either fenced in its owner's PR, or listed in the shrink-only exceptions with its sweep entry. No id is un-pended on an unfenced pass.
- Known false fences are not accepted as fences: `pump_until Stopping` (Core sets `Stopping` at `begin`), a count of `pump`s, and a `pump_until Completed` that polls the event that a later step asserts.
- Self-tests: each shape is flagged in its racy form and cleared in its fenced form, and each test is red on revert.
- Out of this PR's scope: the R-80 list growth (`uncarriable_sequence` as an output; `attach_route` and `queue_overflow` as model reads) belongs to the R-80 lint owner's PR (P5). The `op_posted` control belongs to P5's PR.
- Note: R-88 adds `parked_work` and title-sentinel fences to the three `e3_1` ids. This supersedes my R-87 sibling note ("no fence needed"). The owner proves by 32 per-seed runs which session's work is parked.

## Early design answers (P3's `r88-lint-design.md`, sha256 `ac494f77eb8cb2b6aa058c0cc027c5c93b51a2f2e952f07f3dd10701b55223a1`)

- (a) The added pairs (`Signal`, `Remove` and `Detach` as causes; `call cancel` as a read) go past R-88's text, so the steward rules on them, not the reviewers. Detach and cancel look sound: tm_6_begin s22 already shows the cancel race, and R-88 (B) rules its held-write fence. For `Signal` and `Remove`, P3 names the report that each one waits for.
- (b) An ordered event `expect` is a fence when its matcher names the event (no `$any`). `ignore` and `forbid` never count.
- (c) A route-frame `expect_quiet_until` is not an R-88 read: no report that Core consumes decides it.
- (1) The output class reuses R-80's `output_fences`, with the last-bytes rule for derived events and the `$one_of` intersection. There is no second definition.
- (2) A fence control counts only when its expectation proves the report is there: `parked: true`, `held: true`, `count >= 1`, or `ready`/`posted: true`.
- (3) If the sweep judged a read "fenced" but the lint still flags it, that is a lint gap, not an exception. A "safe" read is listed only when its reason cannot be encoded.
- Commit 1 (R-80 list growth) is P5's in R-88 "unless the lead reassigns". The body states the lead's reassignment.

Checks for the READY head: all of the above, plus the self-tests in the design (one case per class, red on today's am_4 s24).

### Correction (R-88 on main `c0c2abc09f208fea39896972373da53cb90dd830`)

I based two of my early answers on the draft, not on main. On main:
- **Added pairs.** Line 39 says: "P3 may add a pair that the sweep did not name; the reviewers check each addition against a transcript." So no steward ruling is needed. For each added pair (`Signal`, `Remove`, `Detach`, `call cancel`), I check the exact cause, the report and the read in a transcript, with the clause.
- **Commit 1 owner.** Line 49 assigns the R-80 list growth to P3. My "P5 unless the lead reassigns" answer is withdrawn.

016c's design findings to P3 (recorded, not repeated):
- do not list missed valid fences;
- keep the report's identity by route, op and handle: a `route_reports` of r1 does not clear a `route_send` on r2 of the same session;
- a held-state precondition is not report consumption;
- fence matchers must prove their predicates, and report-made states are explicit;
- the `control.op` selector collides with the argument name of `op_posted`.

## Design revision 3 (sha256 `b93f7e935ba8384d22350626ea98f6dcdcaeeeb85a942089d3677b08204b4938`): no objection

My points are in the design:
- R-80's `output_fences` is reused (line 82);
- only expectations that prove the report count (line 83);
- the cause identity is handle, session, class, op and route (line 130);
- the four fence forms that the lint now learns;
- one transcript and clause for each added pair (lines 146-150).

The result on main `c0c2abc0` is 58 matches in 26 ids. There is one new racy read for the lead: `in_6_refused_full_wakes_on_room` s9.

The note on the order with #69:
- If #69 lands first, the lint clears `ev_5_route_closed` s28/s29 by #69's index-25 `route_reports {$r2}` `activity_before_close: true` fence, then by index 27's `closed: true`, with no exception and with a self-test of that exact form.
- If the lint lands first, s28/s29 are listed, and #69's merge removes them (shrink-only).

I check this on the READY head.

### 016c's prototype findings on revision 3 (package scope; recorded, not repeated; I also check them at READY)

- MEDIUM: `pty_queued` suspends every input cause with no proving expectation and no held-write context (prototype line 249).
- MEDIUM: `Completed` and `op_posted` clear a cause by the text of the op variable, with no handle and no binding generation (lines 153 and 258). 016c asks for negative tests: `bytes: 0`, an unrelated later op, and a `$w` reused across handles.
- Distinctness: `activity_before_close` stays true once set. So an earlier `route_reports` with `activity_before_close: true` does not fence a later `route_send` on the same route. A fence for the later send needs a distinct proof. (In #69, r2 has one send before its fence, so #69 is not affected.)

## Round 1: NOT CLEAN (2 LOW) on 39cc39ddfe775e2eabdd88bab3f680d0ed9e654a

There are two commits on main `17f8a6e94cf3fff61c4eb2aff4a02f1e5dfd4b3e`: `7e83ea5` (the R-80 list growth, the one-byte rule, the `dp_3` sentinel) and `39cc39d` (R-88, `tests/fence_lint/report.rs`). Four files change: `core-fence-exceptions.txt`, `dp_3_screen…json`, `fence_lint.rs` and `fence_lint/report.rs` (+1987/-44).

Gate: `~/botster-sessions/gates/botster-contracts-p3-r88-lint-39cc39dd-pool-20261011-162459-27871.log` (sha256 `62989cfff5a121041da2c9890920a6f4d8a5c71edb65d682e2d2cfb92d13e349`, recomputed). The header names head `39cc39dd...` and base `17f8a6e9`. msa1. Ten steps PASS. Exit 0.

### My design points: in the code

- R-80's `output_fences` and `output_fence_covers` are reused, with the last-bytes test and the `$one_of` intersection (`report.rs` `output_fence`, line 495).
- A control fences only with a proving expectation: `parked: true`, `held: true`, `count >= 1`, `bytes >= 1`, `posted: true`, `activity_before_close: true` or `closed: true` (`control_fence`, line 567).
- An ordered event `expect` or `expect_set` goes through the same wait function. `ignore` and `forbid` never fence.
- A route-frame `expect_quiet_until` is no read, and the module doc gives the reason.
- Identity: `Completed` matches the current binding on the op's handle (`self.current(v)`, `.filter(|op| op.handle == h)`). **Correction (016c): `op_posted` (lines 628-635) has no handle filter.** See 016c's first MEDIUM below. `route_reports` `activity_before_close` clears only the first client send on its route (`first_on_route`). `pty_queued` counts only in R-74's form (a blocked PTY, a filler write, a lower bound of at least 1).
- The ownership of both commits (main R-88 "Owners") is cited.

### Findings

- **F1 LOW (the debt notes prescribe a fence that #71 proved wrong).** The debt notes of the three `e3_1` ids (lines 337-350) tell the owner to add `await_control parked_work {s1} {parked: true}`. #71's per-seed probe (`e31-whose-parked-probe.log`) shows that for `carried_transition`, an s1 fence is INCONCLUSIVE in 32 of 32 seeds, and s2's work is parked in 32 of 32. Main's R-88 says "The parked session is not assumed". For `eventless_effect` and `runnable_silent`, main's R-88 (line 39) says the fence is added and proved in the PR that un-pends each id. Required:
  - `carried_transition`: the note names `{s2}`, or the line goes when #71 merges (see the merge order below);
  - the other two: the note says that the session is proved per seed at the un-pend, not s1.
- **F2 LOW (`parked_work` clears more than it proves).** `control_fence` clears every armed `Life` cause of S on the handle when `parked_work {S}` is true (line 579). `parked: true` proves that at least one report of S waits for room. The worker link is ordered, so the parked frame is the first unconsumed report. A later life cause of S (for example a `Remove` after a `Stop`) may have no report written yet. After the poll that frees room, a read that depends on that later cause races, but the lint has cleared it. Required, one of:
  - (a) clear only the earliest armed `Life` cause of S, with a self-test of two pending life causes;
  - (b) show that no corpus id has two armed life causes of one session at a `parked_work` fence, and add a self-test that pins the chosen behavior, with the reason in the module doc.

### Merge order with #71 and #69 (coordination; no finding against this head)

The corpus test requires the exception list to equal the findings. #73 lists R-88 debt for ids that #71 fences (`ev_5_full_queue`, `ev_5_stop_kills`, `tm_6_more_means`, `tm_6_parked_work`, the three `e3_1`) and for `ev_5_route_closed` s28/s29, which #69 fences.
- Whichever PR merges second must merge main and reconcile the list against the lint's findings on the merged tree.
- Not every line will go. In `e3_1_carried_transition`, the ruled fence is after the last `more: false` pump, so the pump reads before it stay matches unless the lint judges them insensitive. They then need `exception:` lines with reasons.
- I check the reconciliation, as a delta review, on the merged head of whichever PR is second.

### Notes (no finding)

- `event_fence` has a branch for `Activity` with `source` the string `"Client"`. `ActivitySource::Client` carries a `RouteId` (`botster-core-contract/src/event.rs` line 90), so no transcript can match that branch. It is dead code, not a gap.

### 016c's round 1 findings on the same head (package scope; recorded, not repeated)

016c has an exact-module proof (`.review-r88-proof/proof.rs`). Each case expects the later read to stay flagged, and each gets `[]`.
1. MEDIUM: `op_posted {for_op}` clears the op of another handle. Line 632 has no handle filter. **My round 1 said that `op_posted` filters the handle. That was wrong, and I corrected it above.**
2. MEDIUM: `await_get` of `Running` clears an unfenced `lose_worker` cause (line 428). A non-terminal report state does not prove the loss report.
3. MEDIUM: a 2-byte write with `kernel_bytes: 1` counts as a real held filler (line 371). R-74's form needs a write larger than any kernel buffer.
4. MEDIUM: a `Completed` whose outcome is `$one_of [NotWritten SessionEnded, Written]` clears the session through a string search of the result (lines 450-452). Only a result that requires `NotWritten SessionEnded` proves the end.

I check all four, with their regression cases, on the next head, together with my F1 and F2.
5. MEDIUM (an extension of item 1): `RouteClosed`, `RouteStalled` and `RouteResumed` clear route causes by the text of the route variable, with no handle filter (lines 486-489). 016c's fifth proof reuses `r1` on h2 and waits for h2's `RouteClosed`. That clears h's `Detach` cause: the result is `[]`, not `[5]`. **My round 1 saw this code and assumed that route names are unique per transcript. That was wrong.**
6. LOW (016c F5): the debt owners for `a2_3_close_first_reason_wins_with_new_reasons` and `tm_6_begin_and_attach_signal_wake` differ from R-88's explicit PC assignment.

## Round 2: NOT CLEAN on 2b4cad9269dcd42f3b434279c108afa390cf1fd7 (016c's MEDIUM; F1 partly open)

Delta from `39cc39dd`: `2dbf8495` and `6a639013` (lint fixes), `5b14c815` (merge of main `28e05487`, #71 and #74; tree `e54e1ef6` equals `git merge-tree`), `2b4cad92` (exceptions reconciled, am_4 test). Gate PASS on `2b4cad92`, base main `28e05487` (P3's log `botster-contracts-p3-r88-lint-2b4cad92-pool-20261011-170302-7028.log`).

Closed:
- **F2 (as asked):** `parked_work {parked: true}` now clears only the earliest armed life cause of its session. Test `a_parked_frame_clears_only_the_earliest_armed_life_cause` (two causes, both fenced, an unarmed cause). But see 016c's MEDIUM below: one observation per fence is still not enforced.
- **My correction (`op_posted` has no handle filter):** `.filter(|op| op.handle == h)`; test `an_op_posted_fence_is_on_its_own_handle`.
- **016c's route-handle MEDIUM:** each `attach_route` gets a binding `r#n` per handle (`State::attach`, `route_on`). `Client` activity, `RouteClosed`/`RouteStalled`/`RouteResumed` and `route_reports` clear only causes on the fence's handle and current binding. Test `a_route_fence_names_its_handle_and_its_binding`.
- **016c's other round-1 MEDIUMs and its owner LOW:** a non-terminal state proves only a `Start` (`a_state_that_is_not_terminal_proves_only_a_start`); `ends_session` needs every alternative (`a_completed_ends_the_session_only_when_every_alternative_does`); the held write needs `HELD_WRITE_FILL_BYTES`; the owners move to PC for `a2_3_close_first` and `tm_6_begin` (R-88 C/B as reassigned). 016c reports its F1-F5 closed.
- **am_4 s25 `exception:` line: confirmed.** #74's s11 `op_posted {for_op: $w2}` proves that `$w2`'s `Completed` is posted before `$w1` (s15) and `$w3` (s20) begin, so `poll_events {max: 1}` reads `$w2` first in every order. The lint cannot encode queue order. The renamed test checks the s11 fence and is red without it (`[14]`). This matches my #74 review.
- The e3_1 lines of #71's title fences (step 17) and the fenced reads of ev_5/tm_6 go. The file only shrinks: 13 lines added, 30 removed, and each added line replaces a removed or renumbered one.

Open:
- **016c's MEDIUM (report.rs ~657), confirmed.** Each `parked_work {parked: true}` clears the earliest remaining armed life cause. Two waits with no poll and no room freed between them can observe the same parked frame, so the second wait clears a second cause that it does not prove. 016c's exact-head case returns `[]` where `[6]` is due. A repeated observation must not clear another cause until something can change the parked set (a poll, or another step that frees room). This extends my F2: I asked for "earliest" and did not ask for one cause per parked frame.
- **F1 (LOW), partly open.** P3's READY says "e3_1_carried's lines go, because #71's `parked_work {s2}` fences them". Only steps 27 and 32 go. Steps 28-31 stay, with the old debt text: "an R-57 title sentinel on s17's output and await_title s1 before s18, then await_control parked_work {s1} {parked: true}". The title fence has landed (#71, s18), #71 proved s2 parked in 32 of 32 and s1 INCONCLUSIVE, and the s32 fence comes after these reads. So these four lines prescribe the fence that F1 asked to remove. Please give them the same form as the eventless/runnable_silent lines (the session that per-seed runs prove parked, a fence before the read, at the un-pend), or an `exception:` with a reason, and correct the READY text.

Still to do after the next head: the second merger reconciles with #75 if it lands first.

## Round 3: CLEAN on 522e5485cee9aa574c9bfb94ef52edba6c3ad23a

One commit on `2b4cad92`.
- **016c's MEDIUM: closed.** `State::parked_seen` holds each (handle, session) whose parked frame a `parked_work` fence has used. A second read of it returns without clearing, until a step on that handle takes events from the queue (`TAKES_EVENTS`: the polls, `drain`, `collect_events`, `pump_until`, `pump_collect`, and the event forms of `expect`, `expect_eventually` and `expect_set`, but not a route's `on` form). Red cases: two reads give `[6]`, and a read of another session gives `[6]`. With a `drain` between the two reads, the result is `[]`.
- **F1: closed.** e3_1_carried s28-s31 stay as debt for P5, and their text now states the facts: the s18 title fence and #71's `parked_work {s2}` at s32 are in (s2 proved in 32 of 32 seeds, s1 INCONCLUSIVE), but these reads come before s32, so each needs its own `exception:` reason or an earlier fence at the un-pend. P3's round-2 READY claim is corrected in the PR body.
- Gate PASS on the exact head, all ten steps, exit 0 after 180 s on msa1, base main `8ce40a0d`: `~/botster-sessions/gates/botster-contracts-p3-r88-lint-522e5485-pool-20261011-172108-78178.log`, sha256 `d92f402b26db70c6041840619268b0ace9aef8729f45b9bdb5497042435b2750` (I computed it).
- Main is now `13ad6720`. `8ce40a0d..13ad6720` changes only `docs/steward-rulings.md`, which is the rulings-only exception.

No open finding. CLEAN on `522e5485`.

Merge order notes:
- #66 changes `conformance/core-fence-exceptions.txt` and `driver_steps.rs` (`driver_steps.rs` is not in #73). If #66 lands first, #73's merge shares the exceptions file and needs a Q7 delta review.
- #75 changes `fence_lint.rs` (the program-printed sentinel and the R-80 follow-up on main `c6287e0`). The second of #73 and #75 to merge reconciles `fence_lint.rs`, and I review that delta.

### After the CLEAN: 016c's round-3 MEDIUM (package scope), and main moved

- 016c (round 3 on `522e5485`): `TAKES_EVENTS` resets `parked_seen` for every step of those names, including one that takes only a class K event (for example `expect_eventually TitleChanged`). Such an event can come before the full mandatory queue, so it frees no mandatory room, and the same frame stays parked. 016c's case returns `[]` where `[7]` is due. This is lint-internal (package scope). Per my rule I do not withdraw the CLEAN. I re-check the delta on the next head, and 016c asks the steward before round 4 (round limit).
- Main is `939e1acd` (#66), which shares `core-fence-exceptions.txt`. #69 (next to merge) owns the two `ev_5_route_closed` debt lines (steps 28, 29), so whichever of #69 and #73 merges second removes them.

### Round limit: the steward permits round 4 (no split), via 016c

A `parked_work` witness ends only after a step takes a mandatory event of that session, or observes that parked work's own event or completion. A read of class K events only must not reset it. 016c's red case must give `[7]`. Then P3 merges main `939e1acd` (the shared exceptions file, Q7 delta review, including #69's two `ev_5_route_closed` lines if #69 has landed), and ONE gate runs on the exact head. A new round-4 finding needs another decision before round 5. I review the round-4 delta against this rule.
