# Integration review: #182 (the contracts pin to contracts-v0.1.19, `InvalidInput{field}`; branch stage1/p5-pin-v0.1.19)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 217de310

Reviewed head: `217de31035b309dbebec582fc412323f06256785`, one commit on v1 `13d7db0925cd080b5734a7b118c7fb440aa28c28`, which is the current v1 (7 files,
+93 -42; `botster-core-host` and `botster-core-testkit`, plus the pin). The PR's gate log:
`shared/core-stage1/gate-logs/pin-v0.1.19-217de310.log` (full Linux gate, green). This reviewer did not read it. The
contract is read at tag `contracts-v0.1.19` (`636bc1ba`).

- **The steward ruling.** `ErrorCode::InvalidInput` is now `{ field: Option<String> }`, and its JSON is always an object.
  `admit.rs` `invalid` gives `field: None`, and the new `invalid_field` names the field.
- **A7-1's names.** The amendment (its latest text at the tag, candidate 4; the field names are the same in candidates 1 to 4) lists the rules that Core checks at `attach`, and `field`
  names the option. At the head:
  - `route_tag` and `owner` over `max_route_tag_bytes` give `route_tag` and `owner`;
  - `max_frame_bytes` of 0 or over the cap gives `route_limits.max_frame_bytes`;
  - `max_screen_frame_bytes` under `max_snapshot_bytes + 1` gives `route_limits.max_screen_frame_bytes`;
  - `query_deadline` absent, under 1 ms or over the maximum gives `query_deadline`.

  Each spelling matches the amendment's text, and `attach_checks_each_route_limit_at_its_bound` asserts each one.
- **The refusals with no field.** The other `invalid` calls (size, palette, argv, env, cwd, key and mouse arguments, a
  WebRTC transport given to `attach`, a relative `file_directory`) are not in A7-1's list. So `field: None` follows the
  ruling ("where a clause names it"). A15-1's `text` comes in the stacked A15 PR.
- **Other crates.** The testkit's scripted refusal uses the object form (`{"InvalidInput": {}}`), and its sample table has
  the code. The rest of the workspace compiles against the new variant (the gate). The public-API snapshot step passes in
  the gate.
- **The pin and the ledger.** Every contracts crate moves to `contracts-v0.1.19`. The ledger and the status files do not
  change.

Observation (not counted): A7-1 also lists `connect_deadline` (zero, or over `CoreLimits.max_connect_deadline`). Core has
no such check and no such limit yet. The id `conf::a7_1_connect_deadline_out_of_range_is_invalid_input` stays in
`core-pending.txt`, so this PR claims nothing about it. The PR that adds the check must use
`invalid_field("connect_deadline", …)`.

VERDICT: CLEAN (0 open) at 217de31035b309dbebec582fc412323f06256785
