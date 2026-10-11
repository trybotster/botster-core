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
