# ST stall clock: design review

VERDICT: CLEAN

## Core #295: evidence review

Reviewed head: `dcc53d8f94d64e94a5b0028c0bd1e86369456773`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
ST295-E1 is CLOSED. There is no new code commit and no new source finding.

Mac evidence: `~/botster-sessions/gates/botster-core-stage1-st-more-input-dcc53d8f-pool-20261011-162605-32994.log`.
The header names the reviewed head. The job runs on macOS and explicitly sets `BOTSTER_PENDING_STRICT=1 BOTSTER_SEEDS=0-31` for real conformance.
All 351 selected host and link tests passed.
Both removed pending IDs passed in the full-seed real run (lines 570-571).
The named `slow_real_core` tests passed, including process exit, link hello and wake checks.

The broader real run passed 230 of 232 tests and exited 100. This review does not call that run green.
The two failures remain recorded:

- `conf::ev_5_route_closed_retires_route_keyed_events`, seed 1, step 28:
  `activity_before_close` was false after the close report was consumed.
  The pinned transcript sends r2 input, uses only `await_quiet`, and then sends Detach.
  It has no fence that proves the worker read r2's input before Detach.
  R-66's follow-up diagnoses this exact precondition gap and requires an activity fence before Detach.
  #295 preserves delivery order for complete frames on a link. It adds no synchronization between route input and Detach.
  The cited P6 log at `9231c06d` has an earlier retirement failure, but its expected value differs from this failure.
  That log alone is not an exact-signature reproduction. The pinned transcript and R-66 support the attribution here.
  The corrected evidence names the controlled v1 reproduction log `botster-core-detached-f68f26a7-f68f26a7-pool-20261011-150206-64641.log`.
  It contains the matching step-28 `{activity_before_close: false, closed: true}` failure at seed 0.
  Its header identifies a scratch snapshot, `9c44c653e9d71c32f276842b870a4c6a8a8dc203`, based on v1 `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
  The reviewer inspected its route_io diff: it delays r2's read input and adds tracing. It is not a clean-v1 gate.
  Sol-B reports that the controlled reproduction was accepted. The verdict retains the scratch qualification.
- `conf::st_6_capture_is_frozen_at_model_rev`, seed 5, step 17:
  the revision inequality compared 1 with 1.
  The pinned transcript sends the second output at step 13 and uses only `await_quiet` before ReadScreen.
  R-80 diagnoses this model-read fence gap. #295 changes neither the worker's output handling nor that transcript.
  The RC Linux log at `46c8c548` contains the same step-17 inequality failure at seed 0 (lines 5753-5755).

These diagnoses identify missing transcript fences outside #295's repair.
The changed driver can change timing, but it supplies neither missing fence and does not change the required worker report order.
The focused Mac checks and full-seed proofs required by ST295-E1 passed on the reviewed head.
The separate full Linux gate remains green on that head.
The implementer relays the lead's disposition: contracts #69 fixes the ev_5 fence; R-80 fixes the st_6 fence.
Both fixes reach Core through RC's .42 pin. Neither live ID is re-pended.

The reviewer ran no local build or test. This CLEAN covers #295's exact head and retains both broader-run failures as known limits.
The normal final-gate and base-merge rules still apply before the lead merges the PR.
The earlier implementation verdict below remains as review history; this evidence review supersedes its open status.

## Core #295: first implementation review

Reviewed head: `dcc53d8f94d64e94a5b0028c0bd1e86369456773`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
PR: <https://github.com/trybotster/botster-core/pull/295>.
Risk tier: HIGH. The driver change crosses into the shared link crate.
This review covers the separate TM-6 driver fix. The stall-clock design CLEAN below remains valid.

The reviewer found no new logic defect in the reviewed diff.
The process exit slot preserves a taken exit when the event budget is exhausted.
Link reads distinguish partial input from complete frames within the existing byte allowance.
The driver holds complete frames and link ends when the event budget is exhausted.
The held-frame path checks engine acceptance before the event budget, so a frame parked on queue room does not set `more_input`.
The decoder's new `ready()` query does not consume a frame or bypass its length check.

The read loop advances through a delivered frame or a read after `Ok(None)`.
The revision removes the loop that caused the first gate's mutation timeout.
The unchanged byte-allowance case remains conservative, as the PR body explicitly states.

The PR body includes Prior art and records coordination with PC's #291.
The reviewer checked the specified order: #295 lands first; #291 uses `end_link` and counts its pending drain as runnable input.
The review approves no unreviewed #291 delta.

### ST295-E1 — MEDIUM — Required Mac evidence is not yet supplied

The READY says the focused Mac job is running. The PR body supplies no completed Mac log.
Plan section 8, revision 23ai, requires focused Mac evidence for portable code whose behavior the OS decides, including sockets.
The changed driver reads link sockets and takes process exit reports.

Required change: supply the completed focused Mac log for this exact head and link it in the PR.
The evidence must pass the selected driver and link checks and the named real-process checks.
Also supply the promised real-tier results for both removed pending IDs with `BOTSTER_SEEDS=0-31`.
The Linux gate's slow tier uses seed 0; its green result alone does not prove those real-tier IDs at seeds 0-31.
The announced Mac job includes that full-seed evidence and can close this requirement when it passes.

### Evidence reviewed

Linux gate: `~/botster-sessions/gates/botster-core-stage1-st-more-input-dcc53d8f-pool-20261011-161450-67786.log`.
The header names the reviewed head and base. Every gate step passed. The final status is exit 0 on msa1.
Mutation result: 19 tested, 14 caught, 0 missed, 0 timeout and 5 unviable.
Stage 2 did not run because stage 1 missed no mutant.

Both removed IDs pass in the gate's testkit tier and real tier:

- `conf::e3_1_due_step_beyond_budget_is_carried_with_more`
- `conf::e3_1_two_silences_due_together_with_budget_one`

The testkit tier uses seeds 0-31. The real tier in this gate uses seed 0.
The list changes remove only those two pending entries. They change no transcript or timeout value.
The new driver tests check event delivery, session state, transport closure, `more` and wake behavior.
The reviewer read the earlier red gate diagnosis and the exact-head repair in the source and PR body.
The reviewer ran no build or test locally.

There is one open evidence finding. A new code commit requires delta review.
An evidence-only reply can close ST295-E1 on this head without another code review round.

## Revision 3 review

Reviewed artifact: `shared/core-stage1/handoffs/st-stall-clock-design.md`, revision 3.
Artifact SHA-256: `2e6d6323dd5743a62962c4e9a9a1148449dcb30f1bf7659e758d56bdd1ba7a01`.
Reviewed Core base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Review scope: design only. This verdict approves no implementation head or gate evidence.

ST-D2 is CLOSED. Section 3 records the lead's Q4 decision, option (b).
The stall-clock PR documents the adoption snapshot and restoration rule in both DESIGN.md files. It builds no adoption code.
The note states the adoption limit: a surviving route has no stall clock until DP-8 route adoption lands.
Both DESIGN.md files and the PR body must retain that limit.

The note names ST's next PR as the DP-8 dependency. That PR owns route reports, the new RouteId mapping, stable tags and restoration.
It also owns the specified adoption proof and all five listed `dp_8_*` ids.
The delivery order is the driver `more_input` PR, the stall-clock PR, then the DP-8 route adoption PR.

ST-D1 and ST-D3 remain CLOSED. All three design findings are closed. No new finding is added.
The reviewer verified the revision 3 hash and checked the adoption change against the lead's explicit decision.
The reviewer ran no build or test.

The earlier sections retain review history. This revision 3 verdict supersedes their status.

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
