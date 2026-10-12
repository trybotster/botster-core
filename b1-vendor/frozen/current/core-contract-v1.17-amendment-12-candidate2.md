# Core contract v1.17, Amendment 12 — candidate 2 (holding a worker report, and a point inside the cleanup at the control-link boundary)

The base is the Core contract of `frozen/current` (manifest final20). It is unchanged except where A12-2 names a replacement. Other contracts are cited by contract name and clause id only.

**Supersedes candidate 1** (`7dfbbe0a…cdff`, commit `811660c`), which the Amendment reviewer rejected (verdict `351503b`). Holding a result that already exists cannot show a worker lost while its cleanup is incomplete. Candidate 2 keeps the hold for the lost-result case, and adds a separate script point inside the cleanup for the worker-loss case (A12-1b).

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the core-p4b pair, through the Stage 0 lead (`conf::a6_3_worker_lost_during_cleanup_reports_outcome_unknown`, `conf::a6_3_lost_result_reports_outcome_unknown`).

**Why.** Core A6-3 distinguishes two cases:
- a worker lost **during** the cleanup;
- a cleanup **result** that is lost.

Both must give `NotDeleted{reason: outcome_unknown}`. To prove them, a test must reach the point where the cleanup has started and its result exists, but Core has not read it. A6-1's withheld control link only withholds a connection at connect time. No testkit ability holds one report on an established link.

| # | Clause | Test |
|---|---|---|
| A12-1a | **The process edge can hold the worker's next report to Core at the control-link boundary.** The worker's own code produces the report and writes it. The edge holds it before Core reads it. Then the test does exactly one of two things:<br>- **release** it: Core reads it, and everything proceeds as with no hold;<br>- **break the control link** with the report unread: the result is lost, as when a real link fails after the worker wrote it.<br>The edge **never drops a report while the link stays up**, because a reliable stream cannot lose one message and keep the connection. Core's real code decides every outcome. A read-only count of held reports at the boundary is an observer under steward ruling R-14.3. | `conf::a12_1_held_report_released_proceeds_normally`, `conf::a6_3_lost_result_reports_outcome_unknown` (hold, then break the link) |
| A12-1b | **A script point inside the cleanup.** The scheduler edge (A5-1, A5-2), which orders the worker's internal work, can stop the worker's `Remove` cleanup (LC-7 step 3) **after it has started and before it has produced its result**: for example, after it has deleted some of the uploaded files and before the rest. At that point the process edge **ends the worker**. The cleanup is then incomplete: some files may be gone, and others remain. Core's real code decides the outcome: `RemoveReport{uploads: NotDeleted{reason: outcome_unknown}}` with **no paths claimed**, the teardown completes, and the id becomes free (A6-3). The test checks the files on disk to confirm that the cleanup was partial. | `conf::a6_3_worker_lost_during_cleanup_reports_outcome_unknown` (stop inside the cleanup, then end the worker), `conf::a12_1b_partial_cleanup_claims_no_paths` |

## A12-2. Text replaced

1. **Core A5-1's edge list:** the process edge gains A12-1a, and the scheduler edge gains A12-1b's script point inside the cleanup.

Every other clause is unchanged.
