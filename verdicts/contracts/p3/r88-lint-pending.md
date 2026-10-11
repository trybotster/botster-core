# Contracts (PR number and branch pending): integration review (R-88 lint, P3)

Reviewer: contracts integration reviewer (017f). Implementer: P3 (`sess-1791517264-0164-de59be35e09a9d1858798a29a4f8e2b1`). Package reviewer: 016c. Ruling: R-88 "The sweep rulings" (pending: item 5 of `~/botster-sessions/shared/steward/drafts/pending-rulings-edits.md`; it governs now). Tier: HIGH.
When the PR's branch is known, this file moves to `verdicts/contracts/<branch>.md`.

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
