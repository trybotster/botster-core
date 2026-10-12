# Hub→plugin contract v1.1, Amendment 7 (candidate 1): what the lock history holds, and how rollback moves through it

The base is the Hub→plugin contract of `frozen/current` (manifest final28): v1.1 joint J4e, with Amendments 1 to 6, which are unchanged except where A7-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the hp-plugins batch 2 reviewer, through the Stage 0 lead (`conf::pm_3_lock_history_bounded`).

**Why.** PM-3: "The lock keeps the last `lock_history_max` entries per package for rollback." A2-29: a rollback with no older entry is `failed{detail.code: wrong_state, detail.reason: no_history}`. No clause says:
- whether the count includes the **current** lock;
- what a rollback does with the lock it replaces.

| # | Clause | Test |
|---|---|---|
| A7-1 | **The lock history holds previous locks only, and rollback steps back through it.**<br>- **The current lock** is the package's lock record (PM-3). It is **not** counted in `lock_history_max`.<br>- **The history** is the list of previous lock records, newest first, at most `lock_history_max` entries.<br>- **An install of a new version, or an update:** the current lock is pushed onto the history, and the new lock becomes current. When the history then holds more than `lock_history_max` entries, the oldest is dropped.<br>- **A rollback:** the newest history entry becomes the current lock, and it is removed from the history. **The lock it replaces is dropped**; a rollback never pushes onto the history, so there is no roll-forward through it. A later update is an explicit operation, as PM-3 requires.<br>- **A rollback with an empty history** is `failed{detail.code: wrong_state, detail.reason: no_history}` (A2-29), with effect `none`.<br>- **Example**, with `lock_history_max = 2`: install v1, then update to v2, v3 and v4 in turn. The current lock is v4, and the history is [v3, v2] (v1 was dropped). The first rollback makes v3 current, with history [v2]. The second makes v2 current, with history []. The third is `no_history`. So two previous versions are reachable.<br>- A live change to `lock_history_max` applies to new admissions only (A2-52). | `conf::pm_3_lock_history_bounded`, `conf::a7_1_current_lock_is_not_counted`, `conf::a7_1_rollback_drops_the_replaced_lock`, `conf::a7_1_rollback_past_the_history_is_no_history` |

## A7-2. Text replaced

1. **PM-3, "The lock keeps the last `lock_history_max` entries per package for rollback"** reads with A7-1.

Every other clause is unchanged.
