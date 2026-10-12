# Core contract v1.17, Amendment 7 — candidate 2 (the refusal code for an attach option out of its range)

The base is the Core contract of `frozen/current` (manifest final14), which is unchanged except where A7-2 names a replacement. Other contracts are cited by contract name and clause id only.

**Supersedes candidate 1** (`73339786…db60`, commit `78e5aae`; never reviewed). The only change: the `query_deadline` rule cites OU-1 and EV-8, because no clause EV-8b exists (OU-1's own "(EV-8b)" reference has no target).

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-01. Found by the core-p4a-dp pair, through the Stage 0 lead (`conf::dp_3_route_limits_reject_a_screen_cap_below_snapshot_plus_one`).

**Why.** OU-1 states rules for `attach` options, such as:
- "`max_screen_frame_bytes` … **must be at least `max_snapshot_bytes + 1`**";
- `max_frame_bytes` "is at most `CoreLimits.max_route_frame_bytes`";
- `connect_deadline` "positive and at most `CoreLimits.max_connect_deadline`";
- `query_deadline` "**required and positive** when `answers_queries` is true";
- `route_tag` and `owner` are "each at most `CoreLimits.max_route_tag_bytes`".

But no clause gives the code for breaking one of these rules:
- OU-1's sync list for `attach` names `InvalidInput` only "for a transport that is not a connected stream";
- A2-1 names "`InvalidInput` for an `options` field" only in the `AttachWebRtc` row;
- section 9.3 defines `InvalidInput` for malformed semantic input only.

| # | Clause | Test |
|---|---|---|
| A7-1 | **An attach option out of its stated range is refused synchronously with `InvalidInput{field}`**, before any route is reserved. No route, no event and no route id result (AM-4).<br>- It applies to `attach` and to `AttachWebRtc` alike.<br>- `field` names the option, for example `route_limits.max_screen_frame_bytes`.<br>- **The rules it enforces** are the ones OU-1 states that Core can check at the call:<br>&nbsp;&nbsp;- `route_limits.max_screen_frame_bytes` below `max_snapshot_bytes + 1`;<br>&nbsp;&nbsp;- `route_limits.max_frame_bytes` above `CoreLimits.max_route_frame_bytes`;<br>&nbsp;&nbsp;- `connect_deadline` that is zero, or above `CoreLimits.max_connect_deadline`;<br>&nbsp;&nbsp;- `query_deadline` absent or zero while `answers_queries` is true (OU-1; the query deadline of EV-8);<br>&nbsp;&nbsp;- `route_tag` or `owner` longer than `CoreLimits.max_route_tag_bytes`.<br>- **A value equal to its bound is accepted.** For example, `max_screen_frame_bytes = max_snapshot_bytes + 1` attaches, and `max_snapshot_bytes` is refused.<br>- **Not refused: a bound known only from the transport.** `route_limits.max_chunk_bytes` above the WebRTC route's SCTP maximum message size is not an error. The route's effective chunk size is the smaller of the two, and the `RouteLimits` that `attach` returns report the effective value (steward ruling R-9.2; HC TR-3: "chosen at or below the SCTP maximum message size"). On a stream route, `max_chunk_bytes` does not apply. | `conf::a7_1_screen_cap_below_snapshot_plus_one_is_invalid_input`, `conf::a7_1_screen_cap_equal_to_snapshot_plus_one_attaches`, `conf::a7_1_frame_cap_above_route_frame_max_is_invalid_input`, `conf::a7_1_connect_deadline_out_of_range_is_invalid_input`, `conf::a7_1_missing_query_deadline_with_answers_queries_is_invalid_input`, `conf::a7_1_overlong_route_tag_or_owner_is_invalid_input`, `conf::a7_1_refused_attach_reserves_nothing`, `conf::a7_1_chunk_cap_above_sctp_max_is_reduced_and_reported` |

## A7-2. Text replaced

1. **OU-1, the sync list:** "(`UnknownSession`, `RouteLimit`, `InvalidInput` for a transport that is not a connected stream)" becomes "(`UnknownSession`, `RouteLimit`, `InvalidInput` for a transport that is not a connected stream or for an option out of its range, A7-1)".
2. **A2-1, the `AttachWebRtc` row:** "plus `InvalidInput` for an `options` field" reads with A7-1.
3. **Section 9.3, `InvalidInput`:** its meaning also covers an `attach` option out of its stated range (A7-1). It is still sync, with no `OpId`.
4. **`conf::dp_3_route_limits_reject_a_screen_cap_below_snapshot_plus_one`** asserts the A7-1 outcome: sync `InvalidInput{field: route_limits.max_screen_frame_bytes}`.

Every other clause is unchanged.
