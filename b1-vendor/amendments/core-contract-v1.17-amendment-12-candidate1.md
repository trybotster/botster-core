# Core contract v1.17, Amendment 12 — candidate 1 (holding a worker report at the control-link boundary)

The base is the Core contract of `frozen/current` (manifest final20). It is unchanged except where A12-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the core-p4b pair, through the Stage 0 lead (`conf::a6_3_worker_lost_during_cleanup_reports_outcome_unknown`, `conf::a6_3_lost_result_reports_outcome_unknown`).

**Why.** Core A6-3 distinguishes two cases:
- a worker lost **during** the cleanup;
- a cleanup **result** that is lost.

Both must give `NotDeleted{reason: outcome_unknown}`. To prove them, a test must reach the point where the cleanup has started and its result exists, but Core has not read it. A6-1's withheld control link only withholds a connection at connect time. No testkit ability holds one report on an established link.

| # | Clause | Test |
|---|---|---|
| A12-1 | **The process edge can hold the worker's next report to Core at the control-link boundary.** The worker's own code produces the report and writes it. The edge holds it before Core reads it. Then the test does exactly one of three things:<br>- **release** it: Core reads it, and everything proceeds as with no hold;<br>- **end the worker** while the report is held: the worker is lost during the cleanup;<br>- **break the control link** with the report unread: the result is lost, as when a real link fails after the worker wrote it.<br>The edge **never drops a report while the link stays up**, because a reliable stream cannot lose one message and keep the connection. Core's real code decides every outcome. A read-only count of held reports at the boundary is an observer under steward ruling R-14.3. | `conf::a12_1_held_report_released_proceeds_normally`, `conf::a6_3_worker_lost_during_cleanup_reports_outcome_unknown` (hold, then end the worker), `conf::a6_3_lost_result_reports_outcome_unknown` (hold, then break the link) |

## A12-2. Text replaced

1. **Core A5-1's edge list:** the process edge gains A12-1.

Every other clause is unchanged.
