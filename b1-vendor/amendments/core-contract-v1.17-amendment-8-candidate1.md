# Core contract v1.17, Amendment 8 — candidate 1 (the capture byte bound, and a snapshot that cannot be formed)

The base is the Core contract of `frozen/current` (manifest final14, with Core Amendment 7 accepted for final15). It is unchanged except where A8-3 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-01.
- A8-1 was found by the core-p1 pair, through the Stage 0 lead (`conf::st_6_capture_bound`).
- A8-2 was found by the Stage 1 Core lead's libghostty audit (`botster-core@765f43a`).

## A8-1. The Core-wide capture byte bound is a reservation

**Why.** 9B: `snapshot_retained_bytes` is "Bytes held by open captures, Core-wide. Over it: `CaptureLimit`". `CaptureLimit` is synchronous (9.3, A2-1). At `begin`, a capture's size is not known yet. A sync check against the bytes already held could admit a capture that then raises the total above the bound, so the bound would not hold. No clause fixes a capture's encoded size, so two captures of one state may differ in size.

| # | Clause | Test |
|---|---|---|
| A8-1 | **`CaptureSnapshot` reserves `max_snapshot_bytes` at `begin`.**<br>- **Held bytes** are the sum, over all open captures, of `Capture.total_bytes` for a completed capture and `max_snapshot_bytes` for a capture still in progress.<br>- `begin(CaptureSnapshot)` is refused **synchronously with `CaptureLimit`** when held bytes plus `max_snapshot_bytes` would exceed `snapshot_retained_bytes`. So the held bytes never exceed the bound.<br>- When a capture completes, its reservation becomes its `total_bytes`. A capture that fails releases its reservation. Release and expiry (ST-6) free the capture's bytes. The resume condition stays: a capture is released or expires (9.3).<br>- **Example:** with `snapshot_retained_bytes = k × max_snapshot_bytes`, k captures in progress are admitted, and the next `begin` is `CaptureLimit`. No test assumes a capture's size.<br>- **No equal-size promise:** two captures of one session at one `model_rev` may have different `total_bytes`. The format may carry per-capture data. | `conf::a8_1_capture_reserves_max_snapshot_bytes_at_begin`, `conf::a8_1_reservation_becomes_total_bytes_on_completion`, `conf::a8_1_failed_capture_releases_its_reservation`, `conf::a8_1_k_captures_admitted_then_capture_limit` |

## A8-2. A snapshot that cannot be formed

**Why.** ST-6b allows two ways to meet the resume invariant: the snapshot carries the parser's pending state, or the cut is taken only at a parser ground state. Suppose a program writes an escape string that does not end (an endless OSC or DCS). Then the terminal model (libghostty, the only oracle, steward rule R-7) may be unable to carry the pending state within its continuation limit, and no ground state comes. ST-6b forbids a partial snapshot. AM-3 requires exactly one completion. Core has no generic deadline outcome (A2-1). `cancel` applies only to writes.

| # | Clause | Test |
|---|---|---|
| A8-2 | **A snapshot that cannot be formed within its bounds is `SnapshotTooLarge`**, the outcome that already exists for an oversize snapshot. A snapshot cannot be formed within its bounds when either:<br>- it would exceed `max_snapshot_bytes`; or<br>- the model is inside a sequence whose pending state exceeds what the snapshot format can carry, so neither way of ST-6b is available at the cut.<br>The outcomes:<br>- **`CaptureSnapshot`** completes `SnapshotTooLarge` (async, as 9.3 and OU-9 already say), with exactly one completion (AM-3), and frees its reservation (A8-1);<br>- **a route baseline or a resync** closes the route **at once** with reason `SnapshotTooLarge`, typed, with no partial baseline (OU-9). The client may attach again later (HC TS-11).<br>**No waiting:** Core does not wait for the sequence to end, and it adds no deadline and no limit. The resume invariant of ST-6b is unchanged: every snapshot that **is** offered satisfies it.<br>**The exclusion is stated** (ST-6b, "never silent"): the `snapshot_format` description names the format's continuation limit, the largest pending sequence state it carries. That limit is a property of the format and of the pinned model, not a `CoreLimits` value. | `conf::a8_2_capture_inside_an_uncarriable_sequence_is_snapshot_too_large`, `conf::a8_2_baseline_inside_an_uncarriable_sequence_closes_route_snapshot_too_large`, `conf::a8_2_offered_snapshots_still_satisfy_the_resume_invariant` |

## A8-3. Text replaced

1. **9B, `snapshot_retained_bytes`:** "Over it: `CaptureLimit`" reads with A8-1 (a reservation at `begin`).
2. **ST-6, "Open captures per owner and in total are bounded (9B)"** reads with A8-1.
3. **9.3, `SnapshotTooLarge`:** "A capture or a route baseline exceeds `max_snapshot_bytes`" becomes "A capture or a route baseline cannot be formed within its bounds (A8-2): it would exceed `max_snapshot_bytes`, or the pending sequence state exceeds what the snapshot format carries".
4. **OU-9, "A snapshot larger than `max_snapshot_bytes` cannot be offered"** reads with A8-2 and includes the uncarriable pending state.
5. **`conf::st_6_capture_bound`** asserts A8-1's rule, with no measured capture size.

Every other clause is unchanged.
