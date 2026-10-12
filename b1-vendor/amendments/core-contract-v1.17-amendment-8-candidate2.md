# Core contract v1.17, Amendment 8 — candidate 2 (the capture byte bound, and a snapshot that cannot be formed)

The base is the Core contract of `frozen/current` (manifest final14, with Core Amendment 7 accepted for final15). It is unchanged except where A8-3 names a replacement. Other contracts are cited by contract name and clause id only.

**Supersedes candidate 1** (`f0a0e3d9…5768`, commit `16fcdad`), which the Amendment reviewer rejected (verdict `b6dedbb`):
1. A8-1 states every way capacity returns, including a completed or failed capture, when the host polls its `Completed` (AM-4). It also replaces 9.3's `CaptureLimit` resume text.
2. A8-2 states the ST-6b qualification and how the every-byte-offset test treats a refusal, and it adds a resync test.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-01.
- A8-1 was found by the core-p1 pair, through the Stage 0 lead (`conf::st_6_capture_bound`).
- A8-2 was found by the Stage 1 Core lead's libghostty audit (`botster-core@765f43a`).

## A8-1. The Core-wide capture byte bound is a reservation

**Why.** 9B: `snapshot_retained_bytes` is "Bytes held by open captures, Core-wide. Over it: `CaptureLimit`". `CaptureLimit` is synchronous (9.3, A2-1). At `begin`, a capture's size is not known yet. A sync check against the bytes already held could admit a capture that then raises the total above the bound, so the bound would not hold. No clause fixes a capture's encoded size, so two captures of one state may differ in size.

| # | Clause | Test |
|---|---|---|
| A8-1 | **`CaptureSnapshot` reserves `max_snapshot_bytes` at `begin`.**<br>- **Held bytes** are the sum, over all open captures, of `Capture.total_bytes` for a completed capture and `max_snapshot_bytes` for a capture still in progress.<br>- `begin(CaptureSnapshot)` is refused **synchronously with `CaptureLimit`** when held bytes plus `max_snapshot_bytes` would exceed `snapshot_retained_bytes`. So the held bytes never exceed the bound.<br>- **Capacity returns in three ways**, and a refused host retries after any of them:<br>&nbsp;&nbsp;1. **the host polls a capture's `Completed`:** a successful capture's reservation becomes its `total_bytes`, and a failed capture's reservation is freed. As AM-4 and EV-5(a) say for every capacity code, this takes effect when the `Completed` is polled, not when the capture finishes;<br>&nbsp;&nbsp;2. **`release` or `release_owner`** frees the capture's bytes at once;<br>&nbsp;&nbsp;3. **expiry**, processed in `pump` (ST-6).<br>- **Example:** with `snapshot_retained_bytes = k × max_snapshot_bytes`, k captures in progress are admitted, and the next `begin` is `CaptureLimit`. No test assumes a capture's size.<br>- **No equal-size promise:** two captures of one session at one `model_rev` may have different `total_bytes`. The format may carry per-capture data. | `conf::a8_1_capture_reserves_max_snapshot_bytes_at_begin`, `conf::a8_1_reservation_becomes_total_bytes_on_completion`, `conf::a8_1_failed_capture_releases_its_reservation`, `conf::a8_1_k_captures_admitted_then_capture_limit`, `conf::a8_1_refused_then_retry_after_a_successful_completion_is_polled`, `conf::a8_1_refused_then_retry_after_a_failed_completion_is_polled` |

## A8-2. A snapshot that cannot be formed

**Why.** ST-6b allows two ways to meet the resume invariant: the snapshot carries the parser's pending state, or the cut is taken only at a parser ground state. Suppose a program writes an escape string that does not end (an endless OSC or DCS). Then the terminal model (libghostty, the only oracle, steward rule R-7) may be unable to carry the pending state within its continuation limit, and no ground state comes. ST-6b forbids a partial snapshot. AM-3 requires exactly one completion. Core has no generic deadline outcome (A2-1). `cancel` applies only to writes.

| # | Clause | Test |
|---|---|---|
| A8-2 | **A snapshot that cannot be formed within its bounds is `SnapshotTooLarge`**, the outcome that already exists for an oversize snapshot. A snapshot cannot be formed within its bounds when either:<br>- it would exceed `max_snapshot_bytes`; or<br>- the model is inside a sequence whose pending state exceeds what the snapshot format can carry, so neither way of ST-6b is available at the cut.<br>The outcomes:<br>- **`CaptureSnapshot`** completes `SnapshotTooLarge` (async, as 9.3 and OU-9 already say), with exactly one completion (AM-3), and frees its reservation (A8-1);<br>- **a route baseline or a resync** closes the route **at once** with reason `SnapshotTooLarge`, typed, with no partial baseline (OU-9). The client may attach again later (HC TS-11).<br>**No waiting:** Core does not wait for the sequence to end, and it adds no deadline and no limit.<br>**The ST-6b qualification:** the resume invariant holds for **every snapshot that is offered**, at every cut point. A snapshot is **not offered** only at a cut where the model holds pending sequence state that is larger than the format's stated continuation limit (below) and no ground state is available. There the outcome is `SnapshotTooLarge` as above.<br>**The every-byte-offset test** (`conf::st_6b_resume_invariant_holds_at_every_byte_offset_of_a_corpus_with_partial_escape_sequences`) checks, at each offset:<br>&nbsp;&nbsp;- either a snapshot is offered and the resume invariant holds exactly;<br>&nbsp;&nbsp;- or the outcome is `SnapshotTooLarge`, and the pending state at that offset is larger than the stated continuation limit.<br>&nbsp;&nbsp;A refusal at an offset within the limit is a failure. The corpus includes partial sequences both within and beyond the limit.<br>**The exclusion is stated** (ST-6b, "never silent"): the `snapshot_format` description names the format's continuation limit, the largest pending sequence state it carries. That limit is a property of the format and of the pinned model, not a `CoreLimits` value. | `conf::a8_2_capture_inside_an_uncarriable_sequence_is_snapshot_too_large`, `conf::a8_2_baseline_inside_an_uncarriable_sequence_closes_route_snapshot_too_large`, `conf::a8_2_resync_inside_an_uncarriable_sequence_closes_route_snapshot_too_large`, `conf::a8_2_offered_snapshots_still_satisfy_the_resume_invariant` |

## A8-3. Text replaced

1. **9B, `snapshot_retained_bytes`:** "Over it: `CaptureLimit`" reads with A8-1 (a reservation at `begin`).
2. **ST-6, "Open captures per owner and in total are bounded (9B)"** reads with A8-1.
2a. **9.3, `CaptureLimit`, the resume text** "a capture is released or expires (`pump`)" becomes "the host polls a capture's `Completed`, or a capture is released, or a capture expires (`pump`) (A8-1)".
2b. **ST-6b, the resume invariant:** it holds for every snapshot that is offered. A snapshot is not offered only under A8-2's qualification.
3. **9.3, `SnapshotTooLarge`:** "A capture or a route baseline exceeds `max_snapshot_bytes`" becomes "A capture or a route baseline cannot be formed within its bounds (A8-2): it would exceed `max_snapshot_bytes`, or the pending sequence state exceeds what the snapshot format carries".
4. **OU-9, "A snapshot larger than `max_snapshot_bytes` cannot be offered"** reads with A8-2 and includes the uncarriable pending state.
5. **`conf::st_6_capture_bound`** asserts A8-1's rule, with no measured capture size.

Every other clause is unchanged.
