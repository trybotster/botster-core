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
