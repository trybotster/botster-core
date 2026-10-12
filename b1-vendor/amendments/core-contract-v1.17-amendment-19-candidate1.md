# Core contract v1.17, Amendment 19 (candidate 1): a route's `max_frame_bytes` must carry every fixed-shape frame of that route

The base is the Core contract of `frozen/current` (manifest final37 staging), with Amendments 2 to 18 and Errata 1 to 4, which are unchanged except where A19-3 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-09. Found by the Stage 1 Core lead (P3 on Core PR #206, `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches`).

**Why.**
- A9-1 states the floor of `max_frame_bytes` (`core-contract-v1.17-amendment-9-candidate1.md`, line 13):
  > **A route's `max_frame_bytes` must carry that route's own `attached` frame.** The worker knows the frame's exact size at attach: the selected features, the terminal format and the limits.
- After `attached`, the worker sends `baseline_begin`. Its body carries the full `ModeFlags` (route codec revision 37, line 260):
  > | `0x02` | `baseline_begin` | JSON `{ rows, cols, modes: ModeFlags }` |
- The codec bounds that frame by `max_frame_bytes` (route codec revision 37, line 223):
  > On a route, `screen` is bound by `route_limits.max_screen_frame_bytes`, `history` by `route_limits.max_history_page_bytes`, and every other frame by `route_limits.max_frame_bytes`;
- DP-3 states the same bound (`core-contract-v1.17.md`, line 164):
  > every frame except the screen and a history page is at most `max_frame_bytes`;
- The codec crate agrees: `bound_of` (`crates/botster-route-codec/src/wire.rs`) gives `max_screen` to `screen`, `max_history` to `history`, and `max_frame` to every other type byte.
- The Core lead measured the frames at default attach options on a 24 by 80 session (testkit Core, contracts-v0.1.21, before Amendment 17 removed `max_chunk_bytes` from `attached`). Each size is the type byte plus the body: `attached` 344 bytes; `baseline_begin` 747 bytes; `modes` 747 bytes; `baseline_end{not_requested}` 28; `live` 1; `resync` 21 or 22; `route_closed{attach_failed}` 61 or 62. `screen` (1,158) has its own bound.
- So a route with `max_frame_bytes` equal to its `attached` frame attaches, and then its `baseline_begin` decodes as `frame_too_large`. A9-1's test `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches` cannot pass on an implementation that keeps the codec bound.
- The `attached` frame is not the largest frame of fixed shape. `baseline_begin` and `modes` carry `ModeFlags`, and the size of `ModeFlags` changes while the route is open. So the floor must use the widest value of each frame, not the value at attach.

**Terms.**
- A **floor frame** of a route is a worker-to-client frame type that agrees with all three conditions:
  1. the worker can send it on that route;
  2. the codec bounds it by `route_limits.max_frame_bytes` (every type except `screen` and `history`, codec A.1);
  3. the worker does not split or shorten it to fit the bound.
- The **worst case** of a floor frame is its encoded size (the type byte plus the body, with every JSON escape, as the worker encodes it) when each field has its widest value on that route. A19-1 gives the widest values.
- The **floor** of a route is the largest worst case of its floor frames.

| # | Clause | Test |
|---|---|---|
| A19-1 | **A route's `max_frame_bytes` must be at least that route's floor.** It replaces A9-1's floor (the `attached` frame only).<br>- **The floor frames** are `attached`, `baseline_begin`, `modes`, `baseline_end`, `live`, `resync`, `route_closed`, `input_refused` and `input_done` without `path`, and `terminal_query` on a route that can be asked a query (`answers_queries` is true, EV-8). A later worker-to-client frame type that agrees with the three conditions of the term is a floor frame too.<br>- **Not floor frames:** `screen` and `history` (their own bounds, DP-3); `output` and `output_deflate` (the worker sets the payload length of each frame, DP-3); `output_dropped` (the worker shortens it to fit, Amendment 3, A3-1 item 4); `input_refused` and `input_done` with `path` (A19-2 checks each file before the worker creates it).<br>- **The widest value of each field:**<br>&nbsp;&nbsp;- a value that attach fixes (`features`, `terminal_format`, the applied limits): its exact value;<br>&nbsp;&nbsp;- `rows` and `cols`: the largest size that Amendment 2 allows (A2-1, `Size`);<br>&nbsp;&nbsp;- `ModeFlags`: each bool as `false`; `kitty_flags` at the largest value of the codec's bitset (A.6); each enum at its longest name that the worker sends; `other_modes` with every mode that the terminal model tracks and that has no normative field (E2-2), each as `false`. The tracked modes are the modes of the terminal model of the target worker's protocol version. Core knows that set for every worker protocol version that it supports, because it must know it to adopt such a worker (AD-4). No hello or handshake field is added;<br>&nbsp;&nbsp;- a name from a set (a reason, a `what` name, a query `kind`): its longest name that the worker sends;<br>&nbsp;&nbsp;- **`what` in `unsupported{what}`** is a name from the worker's own set: the names that Amendment 2 lists for `NotWritten(Unsupported{what})` (Amendment 2, the definitions of the IN-2 reasons, `core-contract-v1.17-amendment-2-candidate4.md`, line 80), and the codec's `utf8` (TS-5b). It never contains client text. So its longest name is known at attach;<br>&nbsp;&nbsp;- a part that a clause bounds by a value known at attach: that bound. For `terminal_query`, `request_bytes_base64` is the base64 of `max_query_bytes` bytes (EV-8), and `answer_within_ms` is the route's `query_deadline` in milliseconds;<br>&nbsp;&nbsp;- any other integer: the largest value that its clause allows, or the codec's integer range (A.1) when no clause gives one;<br>&nbsp;&nbsp;- an optional field: present.<br>- **The refusal does not change.** A smaller value is refused synchronously with `InvalidInput{field: route_limits.max_frame_bytes}`, before any route is reserved, as A9-1 and A7-1 state. Core computes the floor for the target worker at the call, as A9-1 computes the `attached` frame. A value equal to the floor is accepted.<br>- **A floor above the ceiling.** When the floor is larger than `CoreLimits.max_route_frame_bytes`, no value is valid. Every attach with those options is refused with the same sync `InvalidInput{field: route_limits.max_frame_bytes}`. `open` does not refuse that configuration. The reason: such a route could not decode its own frames. For example, a `CoreLimits.max_query_bytes` near `max_route_frame_bytes` (9B) gives this result on a route with `answers_queries: true`.<br>- The floor comes from the codec schema and from the route's own values, so this adds **no number**. Amendment 3 computes the worst-case minimum `output_dropped` frame the same way (A3-1 item 4). | `conf::a9_1_frame_cap_below_the_routes_attached_frame_is_invalid_input` (kept), `conf::a19_1_frame_cap_equal_to_the_floor_attaches_and_carries_its_baseline`, `conf::a19_1_frame_cap_one_byte_below_the_floor_is_invalid_input`, `conf::a19_1_every_floor_frame_at_its_widest_value_fits_the_floor`, `conf::a19_1_terminal_query_is_in_the_floor_only_when_the_route_answers_queries` |
| A19-2 | **The frame that carries a file's path must fit `max_frame_bytes` before the file exists.** It extends DP-5b's check before the file is created (A19-3 item 5).<br>- At `file_commit`, **before** it creates the file, the worker encodes **every frame that can carry this file's path, at its largest**: the `input_done` frame, and the `input_refused` frame with `path` (DP-5b) with its longest `reason` and every optional field present. Each encoding is the type byte plus the JSON, with every JSON escape of `path`.<br>- If the larger of these frames is larger than the route's `max_frame_bytes`, the worker refuses the commit with `input_refused{too_large}`, in the form without `path`. It creates no file, and it pastes nothing.<br>- The forms of `input_done` and `input_refused` with `path` stay outside the floor (A19-1). Their size depends on each file's name, so Core cannot compute it at attach. | `conf::a19_2_path_frame_over_max_frame_is_refused_before_the_file_is_created` |

**The tests.**
- `conf::a19_1_frame_cap_equal_to_the_floor_attaches_and_carries_its_baseline`: attach with `max_frame_bytes` equal to the floor. The route receives `attached`, `baseline_begin`, `screen`, `baseline_end` and `live`, and then a `modes` frame after a mode change. No frame is `frame_too_large`.
- `conf::a19_1_frame_cap_one_byte_below_the_floor_is_invalid_input`: attach with the floor minus one. The result is sync `InvalidInput{field: route_limits.max_frame_bytes}`, and no route is reserved.
- `conf::a19_1_every_floor_frame_at_its_widest_value_fits_the_floor`: encode each floor frame with the widest values of A19-1 through the codec. Each encoded frame is at most the floor that Core computed for the route. It includes `ModeFlags` with every tracked mode in `other_modes`, and `rows` and `cols` at their largest value.
- `conf::a19_1_terminal_query_is_in_the_floor_only_when_the_route_answers_queries`: with the same options, the floor with `answers_queries: false` does not include `terminal_query`. The floor with `answers_queries: true` is at least the worst-case `terminal_query` frame.
- `conf::a19_2_path_frame_over_max_frame_is_refused_before_the_file_is_created`: attach with a `file_directory` and a `max_frame_bytes` such that the path paste fits `max_paste_bytes` and the route's frame bound, but the `input_done` frame with the path does not fit `max_frame_bytes`. Upload a file and commit it. The route receives `input_refused{too_large}` without `path`. No file exists in the directory, and the PTY receives no paste.

**A9-2 still holds, with no change.** A9-2's reason is that the `attached` frame alone is larger than the worst-case minimum `output_dropped` frame. The `attached` frame is a floor frame, so the floor is at least the `attached` frame. So under A19-1, every route that attaches can carry the minimum frame, and A3-1 item 4's guard never removes the feature. `conf::a9_2_every_attached_route_can_carry_the_minimum_output_dropped_frame` is unchanged.

**Rejected.**
1. **Exempt `baseline_begin` (and `modes`) from `max_frame_bytes`.** This removes the client's bound on these frames. The client then cannot check the frame length before it allocates (DP-3).
2. **A cap on the size of `ModeFlags`.** This adds a number with no authority. The terminal model already gives the widest value (E2-2).

**Ids.**
- **Withdrawn**, with authority A19-1: `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches`. It is replaced by `conf::a19_1_frame_cap_equal_to_the_floor_attaches_and_carries_its_baseline`.
- **Kept**: `conf::a9_1_frame_cap_below_the_routes_attached_frame_is_invalid_input`. A value below the `attached` frame is below the floor, so the test stays true.
- **New**: `conf::a19_1_frame_cap_equal_to_the_floor_attaches_and_carries_its_baseline`, `conf::a19_1_frame_cap_one_byte_below_the_floor_is_invalid_input`, `conf::a19_1_every_floor_frame_at_its_widest_value_fits_the_floor`, `conf::a19_1_terminal_query_is_in_the_floor_only_when_the_route_answers_queries`, `conf::a19_2_path_frame_over_max_frame_is_refused_before_the_file_is_created`.
- **Unchanged**: `conf::a9_2_every_attached_route_can_carry_the_minimum_output_dropped_frame`, `conf::a7_1_frame_cap_above_route_frame_max_is_invalid_input`.

## A19-3. Text replaced

Each quotation below is exact, with no ellipsis.

1. **A9-1, the clause.**
   - Old (`frozen/current/core-contract-v1.17-amendment-9-candidate1.md`, line 13):
     > **A route's `max_frame_bytes` must carry that route's own `attached` frame.** The worker knows the frame's exact size at attach: the selected features, the terminal format and the limits.
   - New: "A route's `max_frame_bytes` must be at least that route's floor (A19-1). Core computes the floor at attach from the codec schema and the route's own values."
   - The rest of A9-1 (the sync refusal, "as A7-1 refuses an option out of range" and "adds **no number**") is unchanged. The A9-1 test column loses `conf::a9_1_frame_cap_equal_to_the_attached_frame_attaches` and gains the four A19-1 ids.

2. **A9-4, item 1 (OU-1).**
   - Old (`frozen/current/core-contract-v1.17-amendment-9-candidate1.md`, line 36):
     > "and is at least the size of the route's own `attached` frame (A9-1)"
   - New: OU-1 gains "and is at least the route's floor (A19-1)" in its place.
   - So the OU-1 text, Old (`frozen/current/core-contract-v1.17.md`, line 142):
     > `max_frame_bytes` defaults to and is at most `CoreLimits.max_route_frame_bytes`
   - reads: "`max_frame_bytes` defaults to and is at most `CoreLimits.max_route_frame_bytes`, and is at least the route's floor (A19-1)".

3. **A9-4, item 2 (A7-1's list).**
   - Old (`frozen/current/core-contract-v1.17-amendment-9-candidate1.md`, line 37):
     > **A7-1's list of rules** gains A9-1's lower bound.
   - New: "A7-1's list of rules gains the rule `route_limits.max_frame_bytes` below the route's floor (A19-1)." A7-1's own text (`core-contract-v1.17-amendment-7-candidate4.md`, line 29) names no `attached`-frame floor, so it has no other change.

4. **A9-2, the guard (no change of text).**
   - Old (`frozen/current/core-contract-v1.17-amendment-9-candidate1.md`, line 20):
     > So under A9-1, every route that attaches can carry the minimum frame.
   - New: this sentence and A9-2's clause read "A9-1" as A9-1 replaced by A19-1. The reason stays true, because the `attached` frame is a floor frame.

5. **DP-5b, the check before the file is created.**
   - Old (`frozen/current/core-contract-v1.17.md`, line 167):
     > **The encoded path paste is validated before the final file is created:** at `file_begin`, and again before the file is created at the commit, Core computes the **worst-case encoded size of the path paste** (the directory, the separator, the longest sanitized and uniquified name, **the trailing space**, and the bracketed-paste markers) and, if it exceeds `max_paste_bytes` or the route's frame bound, **refuses with `too_large` and creates no file**.
   - New: the sentence stands, and A19-2 adds a second check at the commit. Before the file is created, the worker also encodes every frame that can carry the path, at its largest (`input_done`, and `input_refused` with `path`). If the larger is larger than the route's `max_frame_bytes`, the worker refuses the commit with `input_refused{too_large}`, without `path`. It creates no file, and it pastes nothing (A19-2). The DP-5b test column gains `conf::a19_2_path_frame_over_max_frame_is_refused_before_the_file_is_created`.

The route codec does not state the `attached`-frame floor, so A19 asks for no codec revision. The Hub→client contract is unchanged.

Every other clause is unchanged.
