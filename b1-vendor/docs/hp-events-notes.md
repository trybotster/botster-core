# Feature hp-events: HP transcripts for sections 10, 9.8 and 9.12

Owner: the hp-events pair. Date: 2026-10-02. Branch `foundation/hp-events`. Contract: HP J4e with A1 and A2; steward ruling R-1.3.

The list had 174 ids (events 84, connection primitives 55, web plugin 35; the lead's `ids-hp-events.txt`). 170 have a transcript in `conformance/hp/` and left `pending.txt`. Four stay pending (below).

## Prior art (BUILD.md rule 0)

- **Vault.** "botster hub events use bounded priority lanes instead of unbounded queue fuses" and "botster hub event lanes coalesce repeatable work before rejecting under pressure" (the old Hub's bounded lanes and per-key coalescing; HP EV-11 and EV-13 are the contract form), "botster v1 web plugin owns serving and untrusted signalling" (WP-1 to WP-6), `botster-v1-plugin-contracts`.
- **botster-hub@9bc6cd7d4963ae5cccd642931619b9cac5c8daac.** `src/lua_runtime/events.lua` maps ingress statuses to error kinds (`rejected_undeclared`, `rejected_foreign`, `rejected_audience` to `capability_denied`; `shed_*` to `backpressured`; one more status, `rejected_over_rate`, that HP EV-3 now forbids). It agrees with EV-3 and gave no expected value, so nothing is stolen. `src/transport/http_mcp/*` is an MCP listener with no relation to NT-7. REJECTED as a source: it has no priorities, ledger, gap markers, allowlist, I/O service or relay.
- **frozen/final6/audits/plugin-parity-20260930.md** (TG-1 to TG-7, GH-3, G-12): the Telegram-shaped long poll and send (NT-3, NT-5), the outbound WebSocket (NT-4) and the notification policy (CA-22, EV-9) fed the `given` text, not an expected value.
- **Reused.** The runner, the matchers (no new matcher), the `HUB_CONTROLS` table, the lib of hp-calls-a (`~/botster-sessions/shared/foundation/scratch-hp-calls-a/lib.py`: the generator idea; the new generator is `scratch-hp-events/`, a scratch folder like the old one) and the Appendix A call table (`calls_and_scripts_follow_appendix_a` passes for every call here). Nothing is stolen, so no `Stolen-From` trailer.
- **Hand-rolled with reason.** One generator writes the 170 JSON files (the Hello, Ready, Invoke and peer-plugin preamble repeats in each). The JSON in `conformance/hp/` is the source of truth.

## What a transcript asserts (R-1.3)

- **FakeHub runs 11 ids** (not `structural`): `ev_4_per_source_order`, `ev_4_at_most_once_no_replay`, `ev_5_drop_consumer_copy_with_marker`, `ev_10_per_source_name_order`, `ev_11_high_before_normal_before_low`, `ev_11_starvation_bound_serves_lower_class`, `ev_13_normal_unkeyed_drops_oldest_with_count`, `ev_13_low_drops_oldest_with_count`, `ev_13_marker_at_first_loss_position`, `ev_13_normal_coalesces_by_key`, `ev_13_keyed_coalesce_a1_b2_a3_delivers_b2_then_a3`. The fake already models the event queues from the clause (class order, shedding, markers); the control `emit_events` is the stimulus. The handler h1 runs for the whole transcript, so every copy queues behind it (HP IV-4) and no timing is asserted. 0 lines of fake growth.
- **159 ids are `fake: structural`, ruling R-1.3.** Each has full steps for the real Hub. The stimulus is a plugin call, a Hub or terminal feed (`raise_hub_event`, `terminal_feed`), a client request (`client_invoke`, `peer_connect`, `web_request`, `webhook_post`) or the scripted outside world (`net_world`). The observation is a boundary that the Hub does not write for the test: what the plugin receives (Invoke, HostResult, `event_gap`), what the outside world saw (`net_seen`, `net_state`), the Hub's own outputs (`hub_output`: log, events, receipt metadata, error texts, host call results) and the client side (`client_events`, `web_request`). No expected value comes from a script. The controls are in the second and third tables of the doc comment of `HUB_CONTROLS` (`crates/botster-plugin-conformance/src/lib.rs`), with arguments, results and clauses.
- A transcript never asserts an order that the contract leaves open. Where two deliveries have no promised order, the stimuli are sequenced (one stimulus, one awaited delivery), or the two items sit in different priority classes (EV-11 fixes that order), or the stimulus is chosen so that the order is fixed (`ev_15_gap_retained_when_invoke_queue_full` cancels the queued request). Where the current vocabulary cannot prove the whole set (below), the id stays pending.
- Controls added only with a clause citation: 30, all in the tables of `HUB_CONTROLS`. FakeHub implements none of them. `diagnostics` gains `load_error: {kind, detail?}` (EV-2, EV-11, EV-14, EV-15, WP-4) and `await_operation` gains `plugin?`.
- New fixture `fixtures/host_sockets.json` (NT-1, subject `plugin_host`, structural): a real host runs its step `open_socket`; FakePlugin does not.

## Gaps and open points for the lead and the steward

1. **Pending (4 ids).** `nt_10_fake_io_is_scripted_and_offline`, `nt_10_telegram_shaped_long_poll_and_send`, `nt_10_unload_mid_request_cancels_and_leaks_nothing`: NT-10 says that FakeHub provides the scripted I/O service, and the pin freezes FakeHub. The steward's HP Amendment 3 (candidate 1) makes the scripted I/O service the real Hub's injected edge (`net_world`): the lead hands the three ids over when A3 is in the manifest. `wp_3_served_code_is_trusted_not_claimed_otherwise`: a statement of scope; A3-2 withdraws it.
1. **EV-3 (steward ruling R-21.2).** `ev_3_status_maps_to_kind` has the live cases accepted, `rejected_undeclared`, `rejected_invalid` and `shed_ingress_full` (small `ingress_queue`, the injected scheduler holds the drain: controls `hold_ingress`, `release_ingress`). The whole status table is checked statically by the test `ev_3_status_table_matches_the_clause` (`rejected_foreign` and `rejected_audience` are reserved vocabulary in v1).
2. **EV-6 (R-21.1).** The two EV-6 transcripts assert begin, one or more chunks whose rows are exactly the session set, end, and no delta before the end, with `expect_eventually` and `ack` (no chunk count). The marker of the post-gap case is acknowledged wherever it comes; its own proof is `ev_15_*`.
3. **EV-2 grammar.** "nothing else" has no stated outcome (refused at load or never matching), so the transcript proves the forms that the clause allows and that `wsx` is not under `ws.*`.
4. **NT-3.** The refusal kind of a `timeout_ms` over `net_timeout_max_ms` and of a request body over `net_body_bytes` is not stated. The transcripts assert that nothing stays open after the bound, and that no request reaches the world.
5. **NT-5.** The kind of a placeholder in the host, scheme or port is not stated: `invalid_request` or `capability_denied`.
6. **WP-4 "ingress bound".** `wp_4_deliver_backpressured` reads it as `ingress_queue` and uses the control `hold_relay_ingress` to fill it. If the steward means `relay_queue`, the id changes.
7. **EV-12.** The recovery of `hub/session.*` copies is `state` with a `state_read` of `sessions.list` for a plugin with `session_read`, and `unavailable` without it (the clause example names the session family; the same read is the only authoritative one for the session events).
8. **EV-13 aggregated marker.** `ev_13_ledger_aggregates_exactly_when_full` and `ev_13_aggregated_gap_is_recovery_partial` put the marker first (the position of the first copy lost among the merged names); the name field of an `owner: "*"` entry is not asserted.
9. **Not reachable in the fake.** The ablation "a delivered copy is replayed after a crash" has no effect in FakeHub (it never replays), so `ev_4_at_most_once_no_replay` is judged by its negative step: `expect_quiet_until` an Invoke of the name `first`, after the barrier `settle`. A real Hub that replays fails it.

## Runner change: `ack` (lead-approved)

`expect_set` and `expect_eventually` take an optional `ack` step that the engine hands to the driver for each item as it arrives, with `{"$item": "<JSON pointer>"}` substituted from the item (design 5.3, `crates/botster-conformance/src/order.rs`, vectors in `conformance/engine-vectors/order.json`, including an `ack` that fails and an absent pointer). It asserts no order. Used by the EV-6 transcripts and by `ev_13_mixed_unit_aggregation_keeps_events_and_bytes_separate`. **TS parity needed** (feature ts-player). The hp-surfaces and hp-plugins pairs may reuse it.

## Ablations (design 7.2 rules 6 and 7)

FakeHub grew by 0 lines. The 11 transcripts that run on it still prove their clause: each line is one throwaway break of one fake branch, and the listed transcripts fail at `<id>@<step>` (seed 0):

- Class order reversed (HP EV-11): ev_11_high_before_normal_before_low@8, ev_11_starvation_bound_serves_lower_class@7, ev_10_per_source_name_order@7.
- A full queue drops the newest copy (HP EV-13): ev_13_low_drops_oldest_with_count@7, ev_5_drop_consumer_copy_with_marker@7, ev_13_marker_at_first_loss_position@12, ev_13_normal_unkeyed_drops_oldest_with_count@7.
- A keyed copy does not coalesce with its key (HP EV-13): ev_13_normal_coalesces_by_key@9.
- The marker is not at the first lost copy (HP EV-13): ev_13_normal_unkeyed_drops_oldest_with_count@7, ev_13_marker_at_first_loss_position@14.
- The starvation bound is not applied (HP EV-11): ev_11_starvation_bound_serves_lower_class@11.

## Run

`cargo test -p botster-plugin-conformance --test fakes -- ev_ nt_ wp_` (the structural check validates every valid frame against the generated schemas and checks every call against Appendix A).
