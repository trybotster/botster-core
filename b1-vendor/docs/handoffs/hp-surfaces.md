# Handoff: feature hp-surfaces (paused by the user, 2026-10-02)

## Goal and clause list
HP (Hub→plugin) transcripts for sections 12 and 13 in `conformance/hp/`, one per ledger id, `because` on every expectation. Pin: `~/botster-sessions/pins/foundation-hp-surfaces.f1aacf6e.md`. Id list: `~/botster-sessions/shared/foundation/ids-hp-surfaces.txt` (189 ids), plus the ids the lead added: the five `a4_1_*` (HP A4-1) and the ten `a5_1_*` (HP A5-1) in `scratch-hp-surfaces/extra_ids.txt` (a5 ones there; see the generator). Not mine: `a2_7_status_values_mean_no_plugin_action` and `a6_1_*` (hp-manifest pair). Reviewer label: "hp-surfaces — reviewer". Lead session: sess-1790906371-009d-f607653bc23d9de48c8a47464787a6ba.

## Branch and head
`foundation/hp-surfaces`, head `7d3f1bd886a0963acb560fd56c2a9c9b7e4cf73e`, pushed (it contains main of 2026-10-02 final28 with the speed fix). Worktree: `~/botster-sessions/trybotster-botster-contracts-foundation-hp-surfaces`. Clean.

## Done
- 188 transcripts written (of 204 ids), out of `conformance/pending.txt`. Generator (the source of the JSON): `~/botster-sessions/shared/foundation/scratch-hp-surfaces/` (`lib.py`, `l13.py`, `g12a..h.py`, `g13a..e.py`, `genall.py` regenerates every transcript, applies `pending_back.txt` and rewrites `pending.txt`). The JSON is the source of truth; edit the generator and run `python3 genall.py`, or edit the JSON by hand.
- Controls added to `HUB_CONTROLS` (doc table in `crates/botster-plugin-conformance/src/lib.rs`): 14 + 17 new ones; the hp-events controls `client_invoke`, `raise_hub_event`, `subscriptions` are reused.
- New test `client_transcripts_of_hp_are_structurally_valid` and `pub check_client` in `botster-ui-conformance`; four `vw_10_*` client-subject ids.
- `botster-plugin-contract`: `EntityRecord {id, entity}` and regenerated Appendix B (HP A5-1), own commit 73508e7.
- Notes: `docs/hp-surfaces-notes.md` (Prior art, classification, pending ids with reasons).
- FakeHub/FakePlugin unchanged (0 fake lines).
- Last fully green gate: 652bf796 (log `~/botster-sessions/gates/botster-contracts-foundation-hp-surfaces-652bf796-linux-20261002-070632-85852.log`). The gate on 7d3f1bd was started and cancelled by the pause (not finished).

## Left (16 ids pending, reasons in docs/hp-surfaces-notes.md)
- Client has to show data, no client-step observation: `vw_8_link_uses_the_ui_safe_url_rule`, `vw_9_client_evaluates_no_pointer`, `vw_9_client_dedupes_by_notice_id_only`, `vw_9_tag_replaces_earlier_notice`, `vw_9_two_untagged_notices_are_never_suppressed`, `vw_9_tag_namespaced_by_plugin_and_reaction`, `vw_9_delivery_is_an_upper_bound` (need client steps `shows_notice`, `row_action_link`).
- Plugin duty (needs a plugin_host fixture FakePlugin does not run): `en_5_publish_after_durable_write`, `vw_7_plugin_policy_refuses_other_session_data`, `ss_5_deletion_during_plugin_crash_reconciles_from_baseline`.
- Needs a player that answers event Invokes one by one (and R-21.1 gives no chunk count): `ss_5_ended_and_removed_are_observable`.
- Client authority observation missing: `vw_10_client_mediated_declared_action_is_authorized_by_the_client`.
- A5-1 client validation of a flat row: `a5_1_flat_row_validates_with_additional_properties_false`, `a5_1_composed_closed_schema_validates_the_row_without_id`.
- Opaque ids cannot be bound, tied and ordered by the frozen matchers (reviewer REJECTED two workarounds): `ss_1_list_page_and_cursor`, `ss_4_filter_before_cursor`. A lead decision on a new matcher (design 5.2 + engine vectors) would unblock them.

## Last review verdict and OPEN findings
Round 5 (head 0779d1d): REJECT. Open then: (1) ss_1/ss_4 repeat-read workaround (closed by keeping both pending), (2) exact-head green gate outstanding. Closed: so_3_overflow (EV-13 marker position), paired completions, per-session fences, byte contiguity, request ids, notes counts. Ask the reviewer for its current list; it may add findings on any not-yet-reviewed delta (the merge only).

## Steward rulings and amendments in play
R-1.3, R-1.4, R-21; HP A2, A3, A4 (write_pending_bytes is a Hub-wide total; refusal `backpressured{write_pending_full}`), A5 (record `{id, entity}`; the declared schema validates entity only; `id` is a core field). A6 (publish statuses): hp-manifest pair, not mine; if its ids bring behavior that changes `en_4_*`, check.

## Gotchas
- `S.write()` in `lib.py` builds the plugin's own package from its grants (adds the `mcp` surface, strips the invalid `storage` surface, adds session types and storage collections).
- `settle` leaves a `{"settled": true}` item that the next `expect` sees; use `settle_quiet`.
- Matchers bind at first use; `$one_of` with variables only compares. `anyorder()` makes open-order arrays.
- Merging main conflicts in `README.md`, `conformance/pending.txt` and `tests/fakes.rs` every time: take main's README and pending (`git show origin/main:<file>`), run `python3 genall.py`, `cargo xtask ledger`, `cargo xtask real-only`, keep both sides of the `include_str!` lists in `every_cited_clause_exists`.
- Gate: `~/botster-sessions/shared/tools/botster-gate <worktree>`; run it in the background and wait on the log with Monitor; commit first (65 = uncommitted tracked changes). The default tier budget failure was fixed on main (conformance-speed).
- `ledger` needs each transcript's `clause` equal to the ledger row's clause (rw_1_client_and_agent_callers... is SW-1).

## Exact next step for a fresh implementer
1. `cd` to the worktree, `git fetch origin && git merge origin/main` (resolve as above), regenerate with `python3 ~/botster-sessions/shared/foundation/scratch-hp-surfaces/genall.py`, run `cargo test -p botster-plugin-conformance`.
2. Commit, push, run `botster-gate` on the exact head; on a green log send the reviewer "READY FOR REVIEW foundation/hp-surfaces <sha>" with the log path, the merged origin/main sha and the Prior art note.
3. Fix every finding; the lead lands it on reviewer CLEAN.
