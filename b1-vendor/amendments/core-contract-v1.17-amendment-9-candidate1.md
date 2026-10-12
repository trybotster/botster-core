# Core contract v1.17, Amendment 9 — candidate 1 (a route must carry its own `attached` frame; the testkit makes the compression choice)

The base is the Core contract of `frozen/current` (manifest final16). It is unchanged except where A9-4 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the core-p4a-ou pair, through the Stage 0 lead (`docs/core-p4a-ou-notes.md` on `foundation/core-p4a-ou`).

## A9-1. The smallest `max_frame_bytes` (adds to A7-1)

**Why.** DP-3 bounds every route frame except `screen` and `history` by `route_limits.max_frame_bytes`. That includes the first frame, `attached` (codec A.4: features, terminal format and the route's limits). OU-1 and A7-1 give `max_frame_bytes` an upper bound only. A host can set it so small that the route cannot send its own `attached` frame, and no clause says what happens then.

| # | Clause | Test |
|---|---|---|
| A9-1 | **A route's `max_frame_bytes` must carry that route's own `attached` frame.** The worker knows the frame's exact size at attach: the selected features, the terminal format and the limits. A smaller value is refused **synchronously with `InvalidInput{field: route_limits.max_frame_bytes}`**, before any route is reserved, as A7-1 refuses an option out of range. The bound is computed from the route's own frame, so this adds **no number**. | `conf::a9_1_frame_cap_below_the_routes_attached_frame_is_invalid_input`, `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches` |

## A9-2. The `output_dropped` eligibility case cannot happen

**Why.** Core Amendment 3, A3-1 item 4: "A route whose `max_frame_bytes` cannot carry that minimum frame does not get the feature".
- The minimum `output_dropped` frame is the type byte plus `{"notifications":N,"last":{"source":"osc777","truncated":true}}`. Here `N` has the digits of `route_queue_bytes`, at most 20 for a 64-bit count.
- Every `attached` frame carries the route's limits object (codec A.4), and that object alone is larger than the minimum frame.
- So under A9-1, every route that attaches can carry the minimum frame.

| # | Clause | Test |
|---|---|---|
| A9-2 | A3-1 item 4's eligibility rule **stands as a guard**, but under A9-1 no attached route fails it. Its two tests are **withdrawn**: `conf::a3_1_route_too_small_for_the_worst_case_minimum_frame_gets_no_feature` and `conf::a3_1_minimum_frame_includes_truncated_and_the_widest_count`. A9-2's test replaces them. It checks that, for every route the suite attaches, the route's `attached` frame is at least as large as the worst-case minimum `output_dropped` frame for its `route_queue_bytes`. So the guard never removes the feature. | `conf::a9_2_every_attached_route_can_carry_the_minimum_output_dropped_frame` |

## A9-3. The testkit makes the worker's per-frame compression choice (adds to Core A5-1 and A5-2)

**Why.** DP-3 says that on a route that negotiated `output_deflate`, "the worker chooses per frame" whether to send `0x0E output_deflate` or `0x07 output`. 9B's `output_deflate_min_bytes` adds that the worker "may send a larger payload uncompressed". This is a choice the contract leaves open. A worker that never compresses passes every test that does not make it compress. Then the deflate clauses (A3-1's inflated-size charge and count bound, and DP-3's `output_deflate` cases) are never exercised.

| # | Clause | Test |
|---|---|---|
| A9-3 | **The testkit's scheduler edge (A5-1, A5-2) also makes the per-frame compression choice** that DP-3 leaves to the worker.<br>- For each output payload eligible under DP-3 (the route negotiated `output_deflate`, and the payload is at least `output_deflate_min_bytes`), the test can script the choice to compress or not, or the seed picks it.<br>- **One code path:** the choice is the worker's own decision point. The real binary decides with its own policy. The testkit supplies the decision, and **everything after it is real**: the DEFLATE stream, the inflated-size bound, the queue charge, and A3's accounting.<br>- The edge never makes an ineligible payload compressed, and it never changes the bytes.<br>- So the deflate-specific tests run with compression forced on, and they fail against a worker that compresses wrongly. | `conf::a9_3_scripted_compression_choice_is_the_workers_only_decision`, `conf::a9_3_ineligible_payload_is_never_compressed` |

## A9-4. Text replaced

1. **OU-1, `route_limits.max_frame_bytes`:** "defaults to and is at most `CoreLimits.max_route_frame_bytes`" gains "and is at least the size of the route's own `attached` frame (A9-1)".
2. **A7-1's list of rules** gains A9-1's lower bound.
3. **Core Amendment 3, A3-1 item 4:** its two tests are withdrawn and replaced by A9-2's test. The rule's text stays.
4. **Core A5-1, the scheduler edge,** and **A5-2** include the per-frame compression choice of A9-3. `conf::a3_1_deflate_frame_is_charged_at_its_inflated_size_to_route_queue_bytes`, `conf::a3_1_compressed_repeated_bells_never_exceed_the_count_digits_of_route_queue_bytes` and the `conf::dp_3_output_deflate_*` tests run with compression forced on through A9-3.

Every other clause is unchanged.
