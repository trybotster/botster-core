# Handoff: feature hp-manifest (paused by the user, 2026-10-02)

## Goal and clause list
HP (Hub→plugin) transcripts in `conformance/hp/`, one per ledger id, for HP Amendment 2 (sections 1 to 3: calls, package files and operator settings, Hub parameters), the Lua names (25.2), the manifest (25.1), capabilities and permissions (section 6: CP), crash and adoption (CR-6), hosts (LH, TR-2), the package root of HP Amendment 1 (PK-1 to PK-9), the plugin definition (25.3: PD-1 to PD-9), HP Amendment 6 (`a6_1_*`, `a2_7_status_values_mean_no_plugin_action`) and HP Amendment 8 (`a8_1_*`, `a8_2_*`, `a8_3_*`, `pd_4_*`, `pd_6_breaking_*`).
- Pin: `~/botster-sessions/pins/foundation-hp-manifest.6ceb6ebb.md`. Brief: `~/botster-sessions/shared/foundation/brief-hp-manifest.md`. Id list: `~/botster-sessions/shared/foundation/ids-hp-manifest.txt` (240 ids), plus the 5 `a6_1_*` ids and the 14 `a8_*` ids that the lead added later.
- Reviewer label "hp-manifest — reviewer" (session `sess-1790949908-00c4-b52fc62a36b8c2705068629efd7e3dc3`). Lead: `sess-1790906371-009d-f607653bc23d9de48c8a47464787a6ba`.
- Rules: `role-implementer.md`, `pair-common.md`, `docs/BUILD.md`, the HP players stay unchanged (R-1.3, R-12), every expectation cites a clause sentence, no open order or value pinned, `botster-gate` green on the exact head before every READY.

## Branch and head
`foundation/hp-manifest`, head `c7d0925ac3ee9e5bb220385a250571c2232bcac1`, pushed. Worktree `~/botster-sessions/trybotster-botster-contracts-foundation-hp-manifest` (clean). Nothing of this feature is on main yet.
- Last REVIEWED head: `f86c468daf7685c9a9c6b3c722b6a5c3edb8d85f` (REJECT, round 2); its gate was green (`~/botster-sessions/gates/botster-contracts-foundation-hp-manifest-f86c468d-linux-20261002-090253-66700.log`).
- `c7d0925` has NO gate and NO review: it holds the round-2 fixes, a merge of origin/main (a209d40-era, with lb-cli-suite code: `$set`, driver, Lua runner) and the 17 new A8 transcripts. FakeHub and FakePlugin: 0 lines changed.

## Done
- 3 READY batches are on the branch (not landed): READY 1 (A2 sections 1-3, A6, LN, `a2_8/34/47`, the repair of landed `wp_2_attempt_retired_on_completion_expiry_and_plugin_lifecycle`), READY 2 (MN, CP, CR-6, TR-2, PK), READY 3 (PD, `a2_29`).
- R-22 `wp_2` fix: DONE in the branch (deliver to a completed or expired attempt asserts `{status: rejected}`, effect none, cites `HP A2-18`; `not_found` kept for another plugin's or an earlier instance's id). It is in no landed commit yet; it lands with this branch. The new `a2_18_relay_deliver_accepted_rejected_effects` is the clause test.
- A6 (steward candidate 2, in manifest final27): `a6_1_no_gap_gives_accepted`, `a6_1_store_projected_gap_gives_pending_gap`, `a6_1_computed_gap_gives_resync_scheduled`, `a6_1_gap_caused_by_this_publish_counts`, `a6_1_accepted_is_admission_state_not_a_delivery_promise`, `a2_7_status_values_mean_no_plugin_action`: written (structural; stimuli `family_gap`, `fill_watch_queue`, `watch_events`).
- A8 (manifest final31, a209d40; candidate 2): written in `c7d0925`, NEVER reviewed: `pd_4_consumer_fails_closed_on_unknown_required_field`, `pd_4_minor_only_adds`, `pd_6_breaking_upgrade_shown_in_the_preview_and_refused_unless_accepted`, `a8_1_*` (5), `a8_2_*` (6), `a8_3_*` (3). The pd_6 transcripts `pd_6_accept_breaking_argument_names_consumer_and_interface` and `pd_6_bulk_upgrade_is_one_resolution_all_or_nothing` were changed to the A8-3 refusal `conflict{breaking_not_accepted, consumers}`.
- 226 + new ids have transcripts; the generator is `~/botster-sessions/shared/foundation/scratch-hp-manifest/` (`run_all.sh` regenerates the JSON in `conformance/hp/` after you extend the `gen_*.py` list; `lib.py` normalizes every transcript; the JSON is the source of truth if the folder is lost).
- Notes: `docs/hp-manifest-notes.md` (prior art, controls, choices, round-1 and round-2 fix lists, gaps). New matcher `$subsequence` (vectors + design 5.2 note). New controls are in the `HUB_CONTROLS` table.

## Left (ids still pending, per id)
- `a2_6_every_event_has_a_payload_row`: no real stimulus on main for `hub/session.attached|detached|activity|modes|size|silent` and `hub/route.ended`. Lead ruled: keep pending; do NOT use `raise_hub_event` (the transcript would supply the payload). Needs the hp-surfaces controls (`session_program`, `program_step`) and a `HUB_CONTROLS` entry for a client attach and a route end (coordinate by label with "hp-surfaces — implementer" so there is one control per meaning).
- `lh_1_local_helpers_send_nothing`, `lh_4_globals_equal_published_set`: the lb-cli-suite branch holds a transcript for each (`conformance/lb/` there). Check main: if they landed, drop them from this list; else the lead decides which feature lands them.
- `cp_8_boundary_covers_reads_mutations_spawn_subscriptions_actions` (removed after review finding 9): needs authorized and refused cross-target cases for a mutation (`sessions.stop` on another target with `:any`, effect observed), worktree reads, a subscription and a contributed action. The contributed action needs hp-surfaces' `client_action` / `session_row_actions` controls (not on main when paused).
- `cp_9_package_and_secret_tools_are_scoped_not_operator_only` (removed after review finding 8): no control sets a secret as an agent caller.
- `pd_6_preview_returns_the_ambiguous_requirements` (removed after review finding 7): no clause gives the entries of the `ambiguous` list. A8-3 covers `breaking` only. Needs a ruling (QUESTION to the lead) or a new amendment.
- Everything in the list that has a transcript but was rejected in round 2 is "fixed, not re-reviewed" (see the findings below).

## Last review verdict and OPEN findings
Round 2, REJECT of `f86c468d` (the reviewer's message, all 13 findings). Status in `c7d0925`:
1. MAIN DRIFT (lb-cli-suite code): FIXED in the merge. A newer main (7+ commits) may exist; merge again before the gate.
2. cp_5 and pd_2 optional-feature (mcp grant, Hello grants, Ready handlers agree; do not use `setup.grants`): FIXED (every generated manifest declares `mcp`; package-flow transcripts keep the manifest's grants; `Hello.grants` compared with `$set`).
3. cp_1 and cp_3 `timers.arm` named `h1`: FIXED (a `tm` timer handler is registered and named).
4. pd_7 optional edge (the demo's Ready lacked its declared export `a` and its capabilities): FIXED (tools `a`, manifest capabilities).
5. and 6. `$subsequence` and `each(one_of)` for unordered lists: FIXED with `$set` (main's matcher): `Hello.bindings`, `resolved_requires`, device-package sessions, package-root files, rescan `found`, ambiguous `candidates`. `$subsequence` stays for the ordered HTTP headers (A2-17) and the PD-7 load order, and for single-element membership.
7., 8., 9. removed and pending (see Left).
10. `pd_3b_*` reused one interface identity: FIXED (a distinct interface name per case).
11. `pd_9_*` ready cases: FIXED (quiet check `on: b`, `settle conn: b`; the cases lack a declared export; no extra-tool case).
12. size refusals: FIXED (`$one_of` of the two complete alternatives `invalid_request{manifest_malformed}` or `quota_exceeded{package_too_large}`).
13. `pd_7_providers_load_before_consumers`: FIXED (`load_order` read before `hub_restart`).
Still to do: the reviewer asked to "audit the repeated generated patterns"; a pass was done for `$one_of`/`$array` exactness, request liveness, grants per handler kind and descriptors (scripts in the scratch folder: `liveness.py`), but the A8 transcripts and the final head have had no review.

## Steward rulings / amendments in play
- R-22 (relay.deliver: rejected after completion or expiry; not_found for another plugin's or an earlier instance's id). R-27 (Lua refusals fix the envelope, not where it is checked: transcripts assert the returned envelope, never the presence or absence of a HostCall for a refusal; `tr_2` follows it). R-6.1 (four required manifest keys).
- HP Amendments 3-8 are binding (A3: scripted services are the real Hub's edges; A4: `write_pending_bytes`; A5: entity records `{id, entity}`; A6: publish statuses; A7: lock history; A8: interface fail-closed rules and breaking changes; `frozen/current/hub-plugin-contract-v1.1.amendment-8-candidate2.md`). Waiting: nothing from the steward at pause time.

## Gotchas
- Always `cd` into the worktree; shell `cd` persists between calls.
- Run focused tests with `botsterq run --label hp-manifest --deadline <d> -- cargo test -p botster-plugin-conformance --test fakes`; the full gate is `~/botster-sessions/shared/tools/botster-gate <worktree>` run in the background with its output in a file (it takes 8 to 20 minutes; it exits 65 on uncommitted tracked changes; untracked files do not matter). A gate that waits for a slot holds the branch lock; stop a gate you started with the TaskStop of the background task (never kill a pid).
- After a merge: `conformance/pending.txt` and `README.md` (real-only list) conflict often. Take main's file, then remove ids that have a transcript (`cargo xtask ledger --check --base origin/main` names them) and run `cargo xtask real-only`. Untracked local transcripts change the ledger and real-only checks: keep the worktree clean when you check.
- `every_cited_clause_exists` only knows `HP ` citations and the amendment files listed in `tests/fakes.rs`; add each new amendment file there.
- `admit_manifest` approvals are a map `{scope: name}`; `client_verb` and `client_read` are hp-plugins' controls (landed); `watch_events`, `family_gap`, `hold_fsync`, `release_fsync`, `operation_persisted` are hp-surfaces' lines that this branch copies (drop the duplicates if hp-surfaces lands first).
- The mutants job runs `cargo mutants --in-diff`: arithmetic on loop counters in new Rust code can create a timeout mutant (fixed once in `$subsequence`).
- default tier budget: conformance-speed landed; the gate on `f86c468` was green.

## Exact next step for a fresh implementer
1. `cd ~/botster-sessions/trybotster-botster-contracts-foundation-hp-manifest && git fetch origin && git merge origin/main` (resolve `pending.txt`, `README.md` as above), run the focused tests, `cargo xtask ledger --check --base origin/main`, `real-only --check`, `fmt`, `clippy`.
2. Push, run `botster-gate` on the exact head (background, log in the scratch folder), and send the reviewer "READY FOR REVIEW foundation/hp-manifest <sha>" with: the gate log path, a "Prior art" note (see `docs/hp-manifest-notes.md`), the list of round-2 fixes (above), and that the 17 A8 transcripts are new and unreviewed.
3. Fix every finding of the next review (LOW included) and re-gate. After CLEAN the lead lands the branch (`DONE`).
4. Then resolve the pending ids: ask the lead for rulings on `pd_6_preview_returns_the_ambiguous_requirements` and `cp_9_...secret...`; when hp-surfaces lands, write `cp_8_boundary...` and `a2_6...`.
