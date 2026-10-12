# Hub→plugin contract v1.1, Amendment 6 (candidate 1): when `entity.publish` answers `pending_gap` or `resync_scheduled`

The base is the Hub→plugin contract of `frozen/current` (manifest final26): v1.1 joint J4e, with Amendments 1 to 5, which are unchanged except where A6-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the hp-manifest pair, through the Stage 0 lead (`conf::a2_7_status_values_mean_no_plugin_action`).

**Why.** A2-7 defines what the two statuses mean:
- `pending_gap`: "a subscriber that missed it gets a reset and a fresh baseline (EN-3), the plugin does nothing";
- `resync_scheduled`: "the Hub will ask the provider for a new snapshot, the plugin does nothing".

No clause says which state produces which. EN-3 separates a **store-projected** family, whose snapshot is one committed state of a collection, from a **computed** family, whose snapshot comes from the provider handler.

| # | Clause | Test |
|---|---|---|
| A6-1 | **`entity.publish` answers by the family's projection and its subscribers' state.** For an admitted publish:<br>- **`accepted`:** every current subscriber of the family will receive the change in order (EN-3(c)).<br>- **`pending_gap`:** at least one subscriber has a gap pending: it has missed a change (overflow, refusal or handler failure, EN-3), and its reset and fresh baseline have not been delivered yet. The family is **store-projected** (EN-3(b)), so the Hub builds that baseline itself from the committed collection, and the provider is not invoked.<br>- **`resync_scheduled`:** at least one subscriber has a gap pending, and the family is **computed** (EN-3(a)). The Hub will invoke the provider for a new snapshot to rebuild that subscriber's baseline.<br>In every case the change is admitted, and the plugin does nothing (A2-7).<br>**Stimulus for a test:** give a subscriber a gap by holding its delivery with the real Hub's injected scheduler (BUILD.md, sans-IO) and a small subscriber queue until it overflows, then publish. A store-projected family gives `pending_gap`; a computed family gives `resync_scheduled`. With no subscriber in a gap, the answer is `accepted`. | `conf::a2_7_status_values_mean_no_plugin_action`, `conf::a6_1_store_projected_gap_gives_pending_gap`, `conf::a6_1_computed_gap_gives_resync_scheduled`, `conf::a6_1_no_gap_gives_accepted` |

## A6-2. Text replaced

1. **A2-7, the `pending_gap` and `resync_scheduled` meanings** read with A6-1.

Every other clause is unchanged.
