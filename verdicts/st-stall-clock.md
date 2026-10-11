# ST stall clock: design review

VERDICT: NOT CLEAN (3 open)

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

This finding does not decide Q3's rule for a worker without a host.
It concerns the report that the design says will restore clocks after adoption.

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

## Pending contract decisions

Q1, Q2 and Q3 remain with the steward through the lead. This verdict does not approve any proposed answer.
Q1 must settle E3-1's atomic step and event rule before the reservation design can receive CLEAN.
Q2 must settle the progress fence definition before the contracts transcript change can receive CLEAN.
Q3 must settle the clock rule during loss of the host and adoption.

## Validation

The reviewer verified the artifact hash and read the design against the named contracts, rulings, BUILD.md and plan.
The reviewer inspected report ordering, host deadline scheduling and the current adoption report at the reviewed base.
The reviewer ran no build or test. All three findings remain open.
