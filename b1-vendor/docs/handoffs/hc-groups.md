# Handoff: hc-groups (Stage 0, trybotster/botster-contracts)

## Goal and clause list
- Land HC and codec Hub/binding transcripts: 240 ids (210 HC, 30 codec). List: `~/botster-sessions/shared/foundation/ids-hc-groups.txt` (id, contract, clause, file, section). Pin: `~/botster-sessions/pins/foundation-hc-groups.aaca3cdf.md`.
- Form: `subject: hub` transcripts are shared and structural (they run on the real Hub in Stage 2 through `HubSubject`, with the documented controls only). `subject: client` transcripts assert only the client's response. Every expectation has `because` = one citation (or a list) whose clause id contains a digit. No order is asserted where the contract leaves it open. The in-memory transport never proves a binding (HC A5-1). Do not build a server-role player (`ts-player` owns it).
- The 30 ids whose ledger contract is `codec` keep `contract: "codec"` in their transcript, and the files stay in `conformance/hc/`.
- Delivery: 2 or 3 READYs by section, each with an exact-head green `botster-gate` and the READY main SHA.
- Roles: reviewer label "core-p4c — reviewer" (`sess-1790940826-00ba-ec77c7d0bfb011bdf41468f85c08b54c`), lead `sess-1790906371-009d-f607653bc23d9de48c8a47464787a6ba`, ts-player implementer `sess-1790955504-00c9-738dd8e9ea14b39a33a3a3310f54a7b0`.

## Branch and head (all pushed)
- `foundation/hc-groups` = `69cbac3f6fc327806d78d6c99baaa58cce36f07b`. Batch 1 (61 ids at first READY, now 55 after review) plus review rounds 1 and 2 plus the R-29 crate commit. It contains main `0082564`. origin/main was `c68a61a` at the pause (a docs-only change in `verdicts/`).
- `foundation/hc-groups-wip-b2` = `4a05bdeb457fec2298fb179f4f05569faa497414`. Batch 2 (EV, SF, SF-5d, MC, TB-M, TB-L1, TB-L3, TB-0 unknown binding, PR without pairing frames, mc_6), built on the OLD batch 1 `decda9c`. Its gate passed on that head only.
- `foundation/hc-groups-wip-b3` = `18b0d0f5a638ecc863f839a7dfd244b792415531`. Batch 2 plus batch 3 (BP, TB-L2, TB-L4, TR-1, TS, QR, SF-5f). Not gated.
- Nothing from hc-groups is on main yet. The earlier features (core-p4c, attach-refused, hp-plugins) are merged.
- Merge the wip branches AFTER fixing batch 1 (see Gotchas), then re-run the generators, then merge `origin/main`.

## Done
- Batch 1 READY sent for `decda9c` (gate PASS, log `~/botster-sessions/gates/botster-contracts-foundation-hc-groups-decda9c2-linux-20261002-091826-23274.log`). Verdict: REJECT with 25 findings (below). All 25 are fixed on `69cbac3` and NOT yet re-reviewed or re-gated (the round-3 gate was cancelled at the pause).
- Batch 1 ids on `69cbac3` (55): A5 (`a5_1_in_memory_transport_is_not_a_binding` only), RT (10 minus `rt_2_authorized_access_exceptions_only`, `rt_5_client_checks_socket_owner`), CF (6 incl. `cf_2`), A.1b (2), A.7b (`a_7b_operation_row_at_admission`, `a_7b_shutdown_ended_by_close`), H-* (14), PKC (5), hub_swap (5), KR-1 (11).
- Docs: `docs/hc-groups-notes.md` (choices, controls, pending table), `docs/hub-subject-controls.md` (every new control has a row citing its clauses; a test checks both ways).
- Only code change on the branch: `crates/botster-hub-contract` comments and README (R-29 marks dropped, commit `69cbac3`, codegen for Rust schemas and TS done) and `crates/botster-hub-conformance/tests/hc_structural.rs` (two exact counts became lower bounds, "deferred < 30" became a ratio).

## R-29 status (steward ruling on main, `docs/steward-rulings.md`)
- R-29 confirms every H2/H3/H4 assumption. The crate's "pending steward ruling" marks are DROPPED on `69cbac3` (own commit, schemas and TS regenerated).
- Confirmed layouts: envelope frame name is member `type`; watch sub-frames and `batch_items.changes` are tagged in `frame` (`snapshot_begin`, `snapshot_items`, `snapshot_end`, `upsert`, `patch`, `remove`, `batch_begin`, `batch_items`, `batch_end`, `watch_closed`); `event_gap` is `{type: event, frame: gap, stream_id, owner, name, subject?, count, lost_bytes?, recovery, state_read?, aggregated?}`; pairing and signal frames are `{frame: <snake_case name>}`; `if_rev` is in `req.args` beside `patch`; every integer wall-clock time is Unix MILLISECONDS except `expires_at` (PR-7b and `issue_pairing`: seconds); a member named `*_ms` is a duration; `rescan_packages` ends `succeeded` with `row.result {found: [package key], removed: [package key]}`; `rev` is an opaque string member of every row from get/update/list and of every `snapshot_items` and `upsert.value` row.
- Check first: the crate's row types and the generated kind schemas have NO `rev` member (rows have `additionalProperties: true`, so a transcript can carry it, but the fixture rows in `conformance/hc/fixtures/base.json` and the `rows.rs` types do not). Decide with the lead whether R-29 needs `rev` added to the row types.
- The lead's instruction (not started): land the held ids asserting the confirmed members: `wt_1` to `wt_9`, `bd_2_large_collection_ok`, `ds_3_kind_watch_on_plugin_load`, `vb_4_identity_immutable`, `vb_4_merge_patch_rev_equality`, `ev_4_*` (4), `sf_5_notice_gap_counted`, `pr_*` (the 17 that need pairing frames; they also need a real WebRTC peer, so they are slow-tier), `tr_4_responses_between_baseline_chunks`, `a_7b_rescan_operation_result_found_and_removed`, and time assertions (`ev_1_event_fields`, SF-5 times: assert Unix milliseconds, for example `{"$between": [1000000000000, 99999999999999]}`; `expires_at` in seconds).

## Left (pending, with the reason)
- Layout-dependent, now unblocked by R-29 (see above): wt_1..wt_9, bd_2, ds_3, vb_4 x2, ev_4 x4, sf_5_notice_gap_counted, tr_4, a_7b_rescan, pr_* (17 ids).
- Suite-level statements about the runner and player (coordinate with the runner owner and ts-player; the reviewer wants a structural statement or self-test): `a5_1_real_hub_in_process_runs_the_suite`, `a5_2_one_player_rust_and_wasm`, `a5_2_player_serves_real_bindings`, `a5_2_player_serves_transcripts_without_hub_logic`, `a5_2_unexpected_client_frame_fails`, `a5_3_client_only_transcripts_cite_a_clause_and_assert_only_the_client`, `a5_3_shared_transcripts_pass_on_the_real_hub`, `a5_4_*` (2; need the permute-only delivery decorator and the producer-alternative script format).
- Held on reviewer findings: `a_7b_every_kind_states_closed` (all operation kinds), `kr_1_plugin_log_load_id_and_markers` (needs gap markers through a real retention or rate-limit parameter that no pinned clause names; `restarted` alone is partial), `rt_2_authorized_access_exceptions_only` (needs program-side and worker-side actors, TS-5f), `rt_5_client_checks_socket_owner` (client-only, needs a ClientSubject control).
- Client-only ids that need new `ClientSubject` observations (and the conforming toy in `tests/client_player.rs`): `ev_6_client_does_not_double_act`, `sf_4_exclude`, `sf_4_options_watched`, `sf_5_client_never_evaluates_reactions`, `sf_5c_matching_filter_applied_by_client`, `sf_5f_two_routes_one_client_acts_once`, `qr_client_suppresses_emulator_auto_replies`.
- No observation exists: `sf_5_session_caller_gets_no_notices`, `sf_5_notice_keeps_source_audience`.
- Messaging plugin tool schema is in no pinned clause: `mc_3_lease_redelivery`, `mc_3_post_success_implies_durable`, `mc_3_unacked_survives_kill9`.
- Need a scripted terminal program (HP `sw_*` vocabulary, those ids are still pending): `mc_4_*` except `mc_4_raw_write_needs_stronger_permission`.
- Real WebRTC peer: `bp_4_forged_route_offer_refused`, `bp_4_route_peer_fingerprint_bound`, `tb_w2_route_on_worker_peer`, `tb_w4_progress_and_close`, `tr_2_control_on_hub_peer_routes_on_worker_peers`, `tr_5_signalling_carries_no_secret`.
- Other: `tb_0_any_client_kind_any_binding`, `tb_0_transcripts_run_unchanged_on_every_binding` (statements), `tb_l2_concurrent_same_token_one_wins` (two overlapping handshakes), `tb_l2_no_terminal_frame_read_by_hub` (token inside hex bytes), `qr_lost_completion_no_second_answer`, `qr_reply_is_not_latest_activity`, `en_3b_cancelled_means_no_effect` (no conditional construct), `pr_7b_vectors` (no pinned vector).

## Last review verdict and OPEN findings
REJECT `decda9c` (25 findings, all received; reviewer said these were all of them for batch 1: "I have reviewed all 61 new cases"). Status on `69cbac3`:
1. cf_1 reload: response and event as `expect_set` — fixed.
2. cf_1 live key not proven — fixed (activity of a session: 1000 ms of quiet is `active` under the old 3000 ms bound and `idle` after the reload; `keys` must each name a changed key). `diagnostics` is opaque, never parse it.
3. rt_1 isolated: `fs_prepare` entries have actor `setup`; assert only `setup` actors under home — fixed.
4. sessions.term: omitted with a notes entry (lead direction) — fixed.
5. a5_4 x2: moved to pending — fixed.
6. h_7: new controls `hold_git`/`release_git` (post-admission hold), `hold_update` for h_18 — fixed.
7. Wait for terminal states: `settle` before row assertions — fixed in h_7, h_8; check any new async case.
8. kr_1 title: `fake: structural`, ruling `R-5` — fixed.
9. a_7b_every_kind: pending — fixed. 10. plugin_log: pending — fixed.
11. rt_3: now triggers `process.run` through `plugin_host_call` on a Hub with and without the process bridge, and checks `process_launches` — fixed.
12. Path mapping: documented "temporary root" paragraph in `docs/hub-subject-controls.md` — fixed.
13. rt_2_authorized: pending — fixed. 14. rt_2_cli_files: CLI-owned entries (`actor: cli`), absence check removed — fixed.
15, 16. A5 smoke cases: removed to pending — fixed. 17. `min:1` removed — fixed.
18. pkc fetch_log: `count` bound and unchanged; no exact counts elsewhere — fixed. 19. pkc distinct rows: `$one_of` of the two orders — fixed. 20. h_32: `max` only plus `next_cursor` — fixed.
21. h_9: `[sessions] max_write_bytes`/`max_raw_write_bytes` = 1024, `text_too_long`, `bytes_too_long` — fixed.
22. Added the semantic clauses (H-3, CF-1, KR-1) to A.1b expectations — fixed. Check every NEW expectation for this rule: A.1b fixes only the representation.
23. h_3 update outcome: `restart` without `build` uses the identity the install edge holds after `update` — fixed (doc row of `restart` says so).
24. `restart.env` replaces the whole environment (`{}` is empty); absent keeps the earlier value except `build` — documented; h_3 passes `env: {}` explicitly — fixed.
25. cf_2: each key with a stated default set to it, ready required; keys without a stated default are left out (notes) — fixed.
- Expect the reviewer to find more in batches 2 and 3. Risky areas to re-check before they READY: transcripts that use handwritten OSC/DA1 bytes (`sf_5d_*`, `sf_5f_*`, `qr_*`) probably need `fake: structural` + R-5 like `kr_1_session_title_is_terminal_only` (hclib `H(..., fake="R-5")`); `hold_completion` is used only where the held request does not need to answer; async cases need `settle` or a terminal operation state; negative checks need a positive case in the same transcript.

## Steward rulings and amendments in play
R-29 (above, landed). Earlier rulings used: R-19, R-24, R-26. HC Amendment 6 candidate 1 (install result names the package; detached worktree head) is on main; it may change PKC and worktree transcripts (read `frozen/current/hub-client-contract-v1-amendment-6-candidate1.md` before touching them). No question is waiting.

## Gotchas
- Generators: `~/botster-sessions/shared/foundation/scratch-hc-groups/` (`hclib.py`, `gen_*.py`, `docrows.py`, `fixdoc.py`, `pend.py`, `clause.py`, `notes/b2.md`, `notes/b3.md`). The JSON in the repo is the source of truth; env `HCOUT` and `HCR` point the scripts at another worktree (shell state does not persist between Bash calls, so set them in the same command). `hclib.H` asserts `because` form. `pend.py` recomputes `conformance/pending.txt` as origin/main's pending minus the ids that have a transcript. `docrows.py` adds only the rows of controls that a transcript uses; `fixdoc.py` unions each control's clauses.
- `because` must be one citation `<Contract> <id>` where the id has a digit. `TB-M`, `QR` have no digit: cite `HC MC-2`, `Codec A.4`, `Core EV-8`.
- Request ids must be u64 decimal strings. A bound value cannot sit inside `$contains` or `bytes_hex`. Bind map keys carry no `$`. A control argument must not be named `op`.
- The close frame reason is the one-key object `{"hub_shutdown": {"lock_released": true}}`.
- The ledger (`cargo xtask ledger --check --base origin/main`) fails when pending.txt and the transcripts disagree; after each merge of main re-run `pend.py`, `cargo xtask replacement-map`, `cargo xtask real-only`, `cargo fmt --all`, and the tests (`cargo test -p botster-hub-conformance`).
- Gate: `~/botster-sessions/shared/tools/botster-gate <worktree>` (queue waits can be 20+ minutes; run it in the background and unpiped). Mutants only mutate changed crate sources, so test-only changes do not need mutant tests.
- Scratch worktrees `/private/tmp/claude-501/hcb2` and `hcb3` are disposable; the pushed branches hold the content.
- New controls this feature added are all in `docs/hub-subject-controls.md` and in `notes/b*.md`.

## Exact next step for a fresh implementer
1. `git fetch origin`; check out `foundation/hc-groups` (`69cbac3`), merge `origin/main`, re-run the derived-file commands, run the tests.
2. Ask the lead/reviewer whether R-29 adds `rev` to the row types; if so, change `rows.rs`, regenerate (`cargo xtask codegen`, `npm run codegen -w @trybotster/hub-contract` in `ts/`) and the fixtures.
3. Run `botster-gate` on that head, send the reviewer READY (batch 1, round 3) with the READY main SHA and the gate log.
4. Then merge `foundation/hc-groups-wip-b2` (resolve `docs/hub-subject-controls.md`, `docs/hc-groups-notes.md`, `conformance/pending.txt` by regenerating), fix the risk items above, add the R-29 ids, gate, READY batch 2. Then batch 3 the same way.
