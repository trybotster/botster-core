# Hub→plugin contract v1.1, Amendment 6 (candidate 2): when `entity.publish` answers `pending_gap` or `resync_scheduled`

The base is the Hub→plugin contract of `frozen/current` (manifest final26): v1.1 joint J4e, with Amendments 1 to 5, which are unchanged except where A6-2 names a replacement. Other contracts are cited by contract name and clause id only.

**Supersedes candidate 1** (`2f15ce7f…3b90`, commit `b4cfcba`), which the Amendment reviewer rejected (verdict `37cc49f`). The status is now decided at one observation point, the publish's admission, and it counts a gap that this publish itself causes. `accepted` describes the state at admission, not future delivery. A test case covers a publish that overflows a queue.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the hp-manifest pair, through the Stage 0 lead (`conf::a2_7_status_values_mean_no_plugin_action`).

**Why.** A2-7 defines what the two statuses mean:
- `pending_gap`: "a subscriber that missed it gets a reset and a fresh baseline (EN-3), the plugin does nothing";
- `resync_scheduled`: "the Hub will ask the provider for a new snapshot, the plugin does nothing".

No clause says which state produces which. EN-3 separates a **store-projected** family, whose snapshot is one committed state of a collection, from a **computed** family, whose snapshot comes from the provider handler.

| # | Clause | Test |
|---|---|---|
| A6-1 | **`entity.publish` answers by the family's projection and its subscribers' state at one observation point: the publish's admission, after the Hub has offered this change to every current subscriber of the family.** A subscriber **has a gap pending** at that point when it has missed a change (overflow, refusal or handler failure, EN-3) and its reset and fresh baseline have not been delivered yet. **This includes a gap that this publish causes**, for example when queuing this change overflows a subscriber's queue. For an admitted publish:<br>- **`accepted`:** at the observation point, no subscriber has a gap pending, and every current subscriber has the change queued in order (EN-3(c)). It describes the state at admission and **promises no future delivery**: a later overflow, refusal or handler failure still ends in an EN-3 reset and baseline.<br>- **`pending_gap`:** at least one subscriber has a gap pending at the observation point. The family is **store-projected** (EN-3(b)), so the Hub builds that baseline itself from the committed collection, and the provider is not invoked.<br>- **`resync_scheduled`:** at least one subscriber has a gap pending, and the family is **computed** (EN-3(a)). The Hub will invoke the provider for a new snapshot to rebuild that subscriber's baseline.<br>In every case the change is admitted, and the plugin does nothing (A2-7).<br>**Stimuli for tests** (the real Hub's injected scheduler holds subscriber delivery, BUILD.md, sans-IO):<br>- **an earlier gap:** a small subscriber queue that overflows on an earlier change, then this publish;<br>- **this publish causes the gap:** a subscriber queue exactly full, so that queuing this change overflows it.<br>In both cases a store-projected family gives `pending_gap`, and a computed family gives `resync_scheduled`. With room in every queue and no gap pending, the answer is `accepted`. | `conf::a2_7_status_values_mean_no_plugin_action`, `conf::a6_1_store_projected_gap_gives_pending_gap`, `conf::a6_1_computed_gap_gives_resync_scheduled`, `conf::a6_1_no_gap_gives_accepted`, `conf::a6_1_gap_caused_by_this_publish_counts`, `conf::a6_1_accepted_is_admission_state_not_a_delivery_promise` |

## A6-2. Text replaced

1. **A2-7, the `pending_gap` and `resync_scheduled` meanings** read with A6-1.

Every other clause is unchanged.
