# Hub→client contract v1: Amendment 3 (candidate 9): notifications in discarded output

Owner: plan auditor. Reviewer: Sol Hub A. Date: 2026-09-30. **This file is the amendment, in the post-freeze amendment path.** The base is the frozen Hub→client contract plus Amendments 1 and 2. The joint manifest names each file and hash, and the base and earlier amendments are unchanged. This amendment cites Core and HP by contract name and clause id only.

**Candidate 6 replaces candidates 1 to 5** (the last was `2cf3a8d6…9273`). Candidate 5 added the rule for several routes to one session. Candidate 6 adds the feature negotiation and makes the worker the policy actor for discarded output (Sol Hub A).
- Following the orchestrator's design, it drops the stall-eligibility approach and keeps SF-5d unchanged: the attached client stays the one actor for its route.
- Following the user's scope correction, it states **only what crosses the boundary**: the guarantee and the frame. How the worker finds the notifications is implementation, so it appears only as a non-normative note.

**The gap:**
1. A stalled or overflowing route has its unstarted output discarded and gets a resync (TS-7, TS-4c).
2. A bell, OSC 9 or OSC 777 in that output, even one parsed while the route was open, never reached the client.
3. That client is attached, so SF-5d sends it no `hub/session_notification`.
4. So the notification was lost.

## Edits to existing clauses

| Clause | The edit |
|---|---|
| Route codec pin | This amendment pins **route codec revision 36** (the joint manifest gives its hash). Revision 33 adds the TS-4c guarantee, the frame `0x0F output_dropped` and its route feature `output_dropped`. |
| SF-5d | **Unchanged.** "Attached" keeps its frozen meaning. |

## New clause

| # | Clause | Test |
|---|---|---|
| SF-5f | **A resync never loses a notification.**<br>- **Negotiation:** a client opts in with the route feature `output_dropped` (TS-1 `route_features`). A first-party client (Web, TUI) always offers it. Without it, no frame is sent and the guarantee does not apply to that route. The worker offers the feature only on a route whose `max_frame_bytes` can carry the **worst-case minimum** `output_dropped` frame (a count with the digits of `route_queue_bytes`, source `osc777`, `truncated: true`; codec A.4). On a smaller route the feature is absent from `attached.features`. An empty `title` or `body` is omitted, so every shortened, marked report fits.
- **Guarantee:** on a route with the feature, a notification (bell, OSC 9, OSC 777) in output that the route never received because of a resync, **and that passes the session's notification policy**, is still reported to **that route's client**, **exactly once**, **before** the resync baseline.<br>- **Frame:** the report is one `output_dropped { notifications: count, last: { source: bell \| osc9 \| osc777, title?, body?, truncated? } }` frame (codec A.4); `last` is required, and the frame always fits the route's `max_frame_bytes` (the worker shortens `last` and sets `truncated`). It is an exception frame: retained, ordered, never discarded (codec TS-7). It is sent only when `count` is at least 1.<br>- **The client acts on it** as it would have acted on the raw sequences: one notification when `count` is 1, or a summary when `count` is above 1. **The worker is the policy actor for discarded output:** the Hub supplies the session type's notification policy (SF-5c) to the worker with the session. `count` counts only the notifications that pass it, and `last` is the last one that passes, so the client does not apply the policy again to this frame.<br>- **One surface per client, across its routes:** a client that holds **more than one** route to the same session acts on notifications (raw sequences and `output_dropped`) from **exactly one** of those routes, chosen by the client; it ignores them on the others. So a notice delivered raw on route A and reported again by route B's `output_dropped` reaches that client once (SF-5d's one-surface rule, per client).
- **No other path exists:** a stalled route gets no Hub event (SF-5d unchanged), so a notice is never both sent during a stall and reported at discard.
- **OSC 52 is never reported:** a clipboard write in output the client never received is not replayed.<br>*(Non-normative: one way to meet the guarantee is to scan the discarded bytes with Core's parser. Started frames are always completed, so they need no report.)* | `conf::sf_5f_lost_notification_reported_once_before_resync` (OSC 9 parsed while the route was open, then its output discarded by a stall: the client gets exactly one `output_dropped` with count 1, before `resync`), `conf::sf_5f_count_and_last_for_many`, `conf::sf_5f_received_output_never_reported_again`, `conf::sf_5f_no_frame_without_lost_notification`, `conf::sf_5f_osc52_never_reported`, `conf::sf_5f_output_dropped_is_retained_exception_frame`, `conf::sf_5f_two_routes_one_client_acts_once`, `conf::sf_5f_matching_policy_counts_only_passing` (the discarded notices are OSC 9 "error" then "done", and the pattern is "error": count 1, last "error"), `conf::sf_5f_none_policy_sends_no_frame`, `conf::sf_5f_mixed_sources_bell_never_matches`, `conf::sf_5f_no_frame_without_feature`, `conf::sf_5f_route_too_small_gets_no_feature`, `conf::sf_5f_skeleton_fits_but_marked_report_would_not_gets_no_feature` (max_frame_bytes 60: the unmarked skeleton is 45 bytes but the marked report is 62, so no feature is offered) (route A open delivers raw OSC 9, route B stalled reports it in output_dropped: the client shows one notification) |
