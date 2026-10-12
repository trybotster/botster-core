# hc-runner: feature notes

Feature `hc-runner` (design 12c, lane B). It salvages `foundation/fake-hub` at 724892b (the same repo, so no `Stolen-From` trailers; each commit names the source). Crates: `botster-hub-conformance` (the HC runner with the route step driver of the codec suite). Transcripts: `conformance/hc/` (60 ids and the fixtures) and the six Hub ids of `conformance/ui/`. Harness list: `docs/hub-subject-controls.md`.

## Prior art (BUILD.md rule 0)

- Vault: *botster FakeHub binding proof requires real transports before consumer acceptance* (a loopback is plumbing, never a `binding:*`), *botster phase zero fake proofs do not establish real Hub semantic conformance*, *botster conformance runner limits yield inconclusive outcomes* (the driver reports `Pending`, `Idle`, `TimedOut`, and never loops without a bound), *botster TypeScript FakeHub may wrap Rust wasm through its own transport adapter* (now the HC A5-2 player, a later feature).
- `foundation/fake-hub` @ 724892b, `docs/fake-hub-notes.md`, `crates/botster-hub-conformance/src/hc/*`, the 68 HC and 6 UI transcripts. **Reused** in its own commit: the HC step driver (framing, partial reads, route handoff), the schema resolver, the transcripts, the fixtures. **Rejected:** `crates/botster-fake-hub` (the whole Hub core), its `FakeHubSubject` adapter, the `hub_bytes_read` observer (a constant zero proves nothing about BP-1) and the fake-only controls.
- botster-hub @ 9bc6cd7d4963 (`botster-hub-test-support`, `unix_route.rs`, `isolated_hub.rs`): a retired scheme-2 harness that polls with sleeps and keeps global state. **Rejected** whole, as in the fake-hub notes. Nothing is stolen.
- In this repo: `botster-conformance` (engine, matchers, order constructs, `stimulus::prepare`, `testgen`), `botster-hub-contract` (the generated `ServerFrame` and `ClientFrame` schemas), the codec route driver, and `botster-core-conformance`'s `CoreHarness` as the model of a harness trait. Established tools only: `jsonschema`, `include_dir`. Nothing hand-rolled that a maintained tool does.
- Hand-rolled with a reason: the client-subject player (HC A5-2 asks for it; no tool plays one side of a clause transcript).

## What was dropped, and why

The FakeHub (`crates/botster-fake-hub`, 12 files) is gone. HC A5-1 and BUILD.md "Phases" take the Hub's semantics from the REAL Hub, in-process, over its in-memory transport. A hand-written Hub is a second implementation that drifts, and the client-facing FakeHub is stopped for good. The fake route endpoint of the codec suite stays (frozen). `xtask replacement-map` no longer names FakeHub as a fake: a `hub` transcript has no fake proof, and `docs/fake-replacement-map.md` is regenerated.

## The `HubSubject` trait (HC A5-1, A5-4)

`has_feature`, `has_control`, `configure(setup.hub)`, `start`, `connect(binding?, uid?, credential?)`, `write`, `read`, `close`, `advance_clock`, `pump`, `control`. Compared with the fake-hub trait:
- `is_fake` is gone (there is no fake), and `HubRead::Pending` is gone: `read` never waits, and the driver calls `pump`, which runs the Hub's injected, seeded scheduler one step (the source of the HC A5-4 producer variations). `pump` gives `Progress` or `Quiet`.
- `advance_clock` on a Hub that cannot inject its clock is `unsupported_control`, never a pass.
- The loopback is the harness's in-memory transport. It is never a binding (HC A5-1); `tests/hc_structural.rs` checks that no transcript names it as one.
- Each control is a harness obligation with its clauses and its injected edge, in `docs/hub-subject-controls.md` (a test checks the list against the transcripts, both ways). No control reads a Hub field, and a control that observes does so at a recording edge (`dispatch_log`, `watch_count`, `caller_of`), so a negative such as `dispatched: []` can change.
- The macros: `hc_hub_conformance_tests!` (the 57 shared ids, a Hub repo runs them), `hc_client_conformance_tests!` (the 3 client-only ids, a client repo runs them), `ui_hub_conformance_tests!` (the 6 Hub UI ids).

## The client-subject player (HC A5-2, A5-3, R-1.4)

`ClientPlayer<C: ClientSubject>` plays the Hub for a `subject: client` transcript. It holds no Hub logic: `send` (a server frame, checked against `ServerFrame` by the engine first), `send_raw`, `send_wire`, `disconnect`, `client_act` (a stimulus to the client), `client_expect` (a typed observation), `client_requests` (maxima only: BD-3 fixes none as a minimum) and `client_idle` (the client wrote nothing that the script did not match: HC A5-2). Every wait is bounded and ends as `inconclusive` at a deadline or after 10 000 client steps. The R-1.4 faults it executes: transport drop (also mid-frame), a malformed or cut frame, a chunk boundary at any byte, and the failure of an unexpected client frame.

**What moves to `ts-player` (lead ruling, 2026-10-02; this feature claims none of it).** The server-role path that plays the shared (`subject: hub`) transcripts, with the script format for scripted alternatives (HC A5-4); and the fault scripts `watch_queue_loss`, producer alternatives, drop of a committed response, duplicate-key scripts, and route resync and partial write. They need transcripts that carry the alternatives, and they are designed once with the wasm target in view. A transcript declares its kind through its subject: `hub` is shared (a Hub state, run against the real Hub), `client` is client-only (the player sends server input and the transcript asserts only the client's response, and cites its clause). `tests/hc_structural.rs` checks that the steps agree with the kind. `tests/client_player.rs` runs the three client-only ids through the player against a toy client (it obeys `welcome.limits`, falls back for a major mismatch, ignores unknown members) and against one defective toy per transcript (fixed limits, limits of the first welcome kept, major-blind, strict about unknown members, chatty): each transcript must pass on the first and fail on each defect, so the observers can change (design 7.2 rule 7). It is a self-test of the player and proves nothing about a real client; Phase 1 client repos implement `ClientSubject`.

## What is checked in the default tier

No Hub exists in Phase 0, so a `hub` transcript gets a structural check, and its full run is the real Hub in Stage 2. The check (`tests/hc_structural.rs`): every transcript parses and its id is its file name; every valid frame validates against the generated schema (most steps are checked; a step that uses a variable that a run binds is left to the run); the 3 typed negatives fail at their named rule; every expectation cites its clause; the kind agrees with the steps; every control is documented. `tests/hub_driver.rs` checks the driver against a loopback that only records and answers (framing, reads of at most 256 bytes, pumping, controls, route handshake). `tests/hc_schemas.rs` checks the resolver.

## Re-check against the current manifest (final21 and later; HC A4, A5; R-8, R-11, R-12; H2, H3, H4)

I re-read every transcript against the clause text. Findings, all fixed:
1. `en_2_server_frames` advanced the clock 11 s and assumed `route_connect_seconds` is 10; the contract gives no default. The setup now sets it.
2. `en_7b_caller_operation_id_resolves_after_kill9` used the adoption hold to model durable-operation recovery. EN-7b names recovery, not adoption (HS-0 names adoption). It now uses `hold_recovery` and `recovery_done` at the storage edge.
3. `vb_1_pages_from_one_cut` required every page to return the same cursor token. VB-1 says only that a cursor is opaque. Each page now binds its own.
4. `en_6_unknown_optional_ignored` was a Hub-subject transcript, but the replacement map and HC A5-3 name an unknown optional field as client-only. It is now client-only (the player sends a `welcome` and a `res` with an unknown member), and the Hub side of the same EN-6 sentence moved into `en_1_request_shape` (an unknown member in a `req`).
5. Every `hello` listed the format `fake-opaque/1`, which only the frozen fake route endpoint offers. A real worker offers a libghostty payload format; the contract's example is `ghostsnp/2+raw`, so the transcripts list it. `hs_3_terminal_format_selected` no longer sets the Hub's formats (a real Hub takes them from its workers), and `hs_3_features_intersected` no longer sets the Hub's features.
6. `en_8_completed_key_returns_first` asserted that the connection closes after a dropped response. No clause says so; the control cuts it, and the transcript no longer asserts it.
7. `ds_2_schema_valid_can_still_fail_state` and `hs_4_unselected_feature_is_unsupported` now cite the clause that fixes the expected code (VB-5 and A.2c; TS-1).

**H2, H3, H4 stay pending.** No landed transcript uses the watch sub-frame tag, `event_gap`, the pairing frame tags, `if_rev`, `rev`, a timestamp unit, or the rescan `found` and `removed`. The four watch transcripts use only the envelope `type` (`watch`, `event`, EN-2) and `res`. Pending HC ids that depend on H2/H3/H4 (the WT, KR, PR, SF groups) are untouched.

## Ids landed and left pending

Landed (removed from `conformance/pending.txt`): the 60 HC ids of BD, DS, EN, HS, VB (57 `hub` and 3 `client`) and the 6 UI ids (`ui_cf_1_*`, `ui_em_7_asset_open_*`, `ui_em_7_foreign_*`, `ui_em_7_local_socket_*`, `ui_rc_4_*` twice). The replacement map was already right for each; only `xtask/src/replacement_map.rs` changed (no FakeHub row).

Left pending, with the reason:
- `en_3b_cancelled_means_no_effect` (EN-3b makes `cancelled` best effort and the real result allowed, so the no-effect check is conditional on the answer; the engine has no conditional construct, and a forced `cancelled` would invent behavior), `bd_2_large_collection_ok`, `ds_3_kind_watch_on_plugin_load`, `vb_4_identity_immutable`, `vb_4_merge_patch_rev_equality`: BD-2 and DS-3 need the watch baseline layout (H2); VB-4 needs `rev` (H4) and the update args (H2).
- The `a5_*` ids: statements about the suite itself (the real Hub runs it, a player serves it). They need a statement-step vocabulary like Core A5's. `tests/hc_structural.rs` already checks what `a5_3_client_only_transcripts_cite_a_clause_and_assert_only_the_client` states; the id lands with `hc-groups`.
- Sections 6 to 11 and A1 to A3 (SF, MC, PR, EV, KR, WT, QR, RT, TB, PKC, CF, TR): `hc-groups`. The `hub_*` ids and the codec Hub and binding ids: `hc-groups`.

## Judgment calls for the reviewer

- `hs_2_*` configure the Hub's current protocol version (`hub.version`). HS-2 gives the range as `[hub_current - 1, hub_current]` and leaves the number to the build, so the harness builds the Hub with it.
- The route format name `ghostsnp/2+raw` is the contract's example. The transcripts that attach or select a format configure it in `setup.hub.worker_terminal_formats` and in the `hello`, and the harness document says so; no clause requires the name.
- `bd_1_watch_cap` sets `max_watches` to 32, the contract floor, and reads the bound back from `welcome`.
- `hs_5_routes_survive_new_incarnation` and `hs_9_control_disconnect_keeps_route_open` reach the worker with the `worker` control (scripted `pty_output`). No terminal is emulated (R-5, R-7).
- Citations: the amendment rows carry the same clause ids as the base text (for example HS-0 in Amendment 2 fixes `lock_held` and `socket_path_too_long`), so `HC HS-0` cites both. Section 11 has no clause id, so `HC 11` cites its error table where `field_errors` and `busy` are fixed.
- Ablation: the feature adds no fake behavior. Its proof is the structural check, the driver and player self-tests (with a defective toy per client-only transcript), and the mutation job of the gate over the diff.
