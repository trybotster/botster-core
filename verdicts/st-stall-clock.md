# ST stall clock: design review

VERDICT: NOT CLEAN (1 open)

## Revision 2 review

Reviewed artifact: `shared/core-stage1/handoffs/st-stall-clock-design.md`, revision 2.
Artifact SHA-256: `42f1222eb18c204323f82305ea4daa50a4a9bf656f346a5ced3caeb11a54bf65`.
Reviewed Core base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Review scope: design only. The reviewer ran no build or test.

- ST-D1: CLOSED for design. Section 2 flushes held progress before every later report.
  The revised bound allows one additional progress report per route before each other report.
  The specified proof covers the earlier unwritten `waiting: false` report under link backpressure.
  The implementation review will check both flush paths and the bound proof.
- ST-D3: CLOSED. Section 7b identifies old code, current reuse, rejected mechanisms, library alternatives and vault decisions.
  It states that no old code is copied. The reviewer checked the cited old write-attempt bound and close reason.
- ST-D2: OPEN pending Q4's scope decision. The proposed snapshot now identifies state, waiting, mark and applied limits.
  Its restoration rule distinguishes adoption from a new stall transition and specifies the required adoption proof.
  However, section 3 offers two delivery paths and does not select one.

The reviewer confirmed that DP-8 route adoption is absent at the reviewed base (`botster-core-host/DESIGN.md:167`).
The current adoption report has no route records. Thus, the proposed snapshot depends on work outside the existing adoption path.
The snapshot design resolves the original data gap, but the design cannot claim restoration until the delivery path is selected.

Required change for ST-D2: record the lead's Q4 decision in the design note.
If this PR builds DP-8 route adoption, name its files, identity mapping, clauses and proofs.
DP-8 and ID-2 require a new host `RouteId` for each surviving route.
If a later PR builds adoption, name that dependency and state this PR's adoption limit explicitly.
Keep the restoration design and proof requirement with that dependency. Do not claim that this PR restores adopted route clocks.

Section 7a correctly separates mandatory queue room from the later pump's event budget.
It also states all four Q1 conditions and the approved Q2 and Q3 rules.
The applied per-route `stall_deadline` correction follows OU-1.
No new logic finding is added in this round.

The sections below retain the first review and its evidence. Their original status is superseded by this revision 2 review.

## First review record

Reviewed artifact: `shared/core-stage1/handoffs/st-stall-clock-design.md`.
Artifact SHA-256: `ac4ac6e239efd9f9b9c8654252b95cf5e12b42e6382033c4e019991538c05353`.
Reviewed Core base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Review scope: design only. No implementation head exists for this review.

The planned Core PR has the correct HIGH tier. The design assigns the stall clock to Core and route transitions to the worker.
This split follows TM-1, TM-3, OU-2 and R-90. The query clock remains outside this change, as EV-8(b) requires.

## ST-D1 — MEDIUM — Coalescing does not preserve the proposed title fence

Evidence: sections 2 and 5 of the design note.
Section 2 suppresses a second progress report while the first report is unwritten.
Section 5 assumes that the title report follows the progress report that arms the current deadline.

These rules do not establish that order. Consider this sequence:

1. The worker queues a progress report with `waiting: false`. The link has not written that report.
2. New output makes frames pending on a gated route. Coalescing holds the new `waiting: true` report.
3. The worker processes the title sentinel and queues its title report.
4. Core consumes the old progress report and the title report before it consumes the held progress report.

The title fence can complete while Core has no reader deadline for the pending frames.
An `advance_clock` can then precede deadline arming. R-88 and R-90 require a fence that proves Core consumed the relevant report.

The cited precedent has an extra rule. At the reviewed base, `worker.rs:418-425` flushes `output_unsent` before another report.
Its comments at lines 289-295 also permit an extra output report before another report to preserve order.
The proposed strict bound of one unwritten progress report omits that rule.

Required change: define how progress coalescing preserves report order before each report that can satisfy the title fence.
Reconcile that rule with the stated queue bound. Specify a proof with link backpressure and an earlier unwritten progress report.
The proof must show that the title fence cannot complete before Core consumes the current waiting report.

## ST-D2 — MEDIUM — Adoption cannot restore an existing stall from the proposed report

Evidence: sections 2 and 3 of the design note.
The worker sends `RouteProgress` for each live route after `Adopted`.
However, `waiting` is true only for an `Open` route with pending frames.
An existing `Stalled` route therefore reports `waiting: false`.

The proposed snapshot cannot distinguish that route from an `Open` route with an empty queue.
Core arms the close deadline only when it posts `RouteStalled`. The adoption path specifies no replay of that transition or stall state.
Thus, the stated adoption path cannot restore the close deadline for a route that was already stalled.

The steward approved Q3: no stall deadline runs while no host is connected.
This finding concerns the report that restores clocks after adoption, including a route that stalled before the host disappeared.

Required change: specify the adoption snapshot for both `Open` and `Stalled` routes.
Identify how Core receives the state, mark and applicable route limits needed to restore each deadline.
Apply the steward's Q3 answer to deadline arming. Keep state restoration distinct from posting a new transition event.
Specify an adoption proof with a route that stalled before the host restart.

## ST-D3 — MEDIUM — The required Prior art note is missing

Evidence: the design note has no Prior art section.
The `stop_grace` and `output_unsent` references identify current mechanisms but do not complete the required prior-art review.
BUILD.md rule 0 requires a note that identifies existing work, reuse, rejected work and reasons for custom infrastructure.
The reviewer must reject a feature without that note.

Required change: add the Prior art note before design approval.
Record the relevant vault decisions, old code at the authorized SHA, and framework or library alternatives.
State what the design reuses and rejects. Give the reason for each new custom mechanism.
If the implementation copies old code, identify the required `Stolen-From` and `For-Clause` commit trailers.

## Contract decisions received before the verdict was sent

The reviewer read the R-90 follow-up at contracts commit `43647054722f3133cfd30b67a909b8fc427dcb3e`.
The steward approved Q1 with four conditions:

1. No host-visible route fact changes before the event posts.
2. The post consumes event budget in the pump that posts it. Without budget, Core carries the post with `more: true`.
3. Core releases the reservation when it consumes the newer report after the worker declines the transition.
4. Due steps follow TM-5 and E3-1. Stall events across sessions follow link consumption order, with no asserted cross-session order.

The revised design must state these four conditions. The reservation alone does not satisfy the rule for the later post's event budget.
Q2 is approved in contract terms: `progressed` proves consumption of transport progress that occurred after the latest `route_accept`.
The contract definition does not expose the internal mark. The title fence remains conditional on report order, so ST-D1 still applies.
Q3 is approved: the host's absence suspends both stall deadlines. The new host arms deadlines after it consumes progress reports.
That answer does not supply the missing state in ST-D2's adoption snapshot.

The lead also assigned a separate host-driver PR for the TM-6 `more_input` defect before the stall-clock PR.
The pair reviewer and Sol review both Core PRs. This design verdict does not review or approve that separate driver change.

## Validation

The reviewer verified the artifact hash and read the design against the named contracts, rulings, BUILD.md and plan.
The reviewer inspected report ordering, host deadline scheduling and the current adoption report at the reviewed base.
The reviewer ran no build or test. All three findings remain open.
