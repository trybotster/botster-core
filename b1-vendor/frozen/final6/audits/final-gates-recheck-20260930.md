# Final gates: delta re-check against the fixed files (2026-09-30)

Auditor: read-only contracts auditor (Claude Opus 5.5). I changed no existing file. I ran no build and no test. I sent no message. This is the only file I wrote.

This is a **delta** re-check. It covers only the items that the final gate records found open. It is not a fresh audit.

## 0. Method

### 0.1 Files and aliases

All paths are under `~/botster-sessions/shared/`. A citation is `ALIAS:line`.

| Alias | File | sha256 (computed today) |
|---|---|---|
| C | `core-contract-v1.17.md` | `8ee7ca05300156312363a00bf3bafae99674d0f09fc349866f71bb2f3a6d3486` |
| HC | `hub-client-contract-v1-candidate-parity43.md` | `d5da3d2efde74a2599a718d7f6531758d7a34a6d14af97423d3524b27bd0b95f` |
| HP | `hub-plugin-contract-v1.1.joint-J4c.md` | `13535bbf734fb5062d9e1e69f0e477fbe330eb7c752073d38dffa272d12d5415` |
| LB | `hub-plugin-lua-binding-v1.1-r4.md` | `9b63a7ce82ed03985f503a3838e88974fafd94382f1d99aafa0c3d0a05655ae8` |
| RC | `route-codec-v1-extract-rev26.md` (unchanged) | `bf59b2b605e21b7e59b8d191cebdac5244241414d5d8f5c1372a86ece52a7abf` |
| UI10 | `ui-contract-v1-frozen-20260930.md` (unchanged) | `2063e8977c36c0e3cacdd0526de131f537b8bc63f4d39ec6db76e3300e5a7661` |
| UI11 | `ui-contract-v1.1-amendment-candidate4.md` (unchanged) | `02cd8476b3791630f32f21af8a48558b30985f902b008bd7bf9d7dbec1035ae7` |
| UI12 | `ui-contract-v1.2-embed-frozen-20260930.md` (unchanged) | `d18867f5d157e8a438becd58c8d987d2b815a0057de36b0bce495ba692f1ebd1` |
| JT | `audits/gate4-suite-traceability-20260930.md`, section "Final journey text" (line 394) | — |

### 0.2 How I found the changes

- I made a line and word diff of each fixed file against the file that the gate records read: C v1.16 → v1.17, HC parity40 → parity43, HP J3 → J4c (and J4b → J4c), LB A4r77-r2 → r4.
- For each open item, I read the fixing line in the fixed file. I quote it.
- For gate 4, I ran a script over every clause table. It finds rows with no `conf::` or `bench::` id and test ids that are in two clauses of one file. I read the LB bullets by hand.

### 0.3 Status words

- **CLOSED**: the fixed text removes the defect. The fixing line is quoted.
- **STILL OPEN**: the defect is still there. The wrong line is quoted.
- **ACCEPTED-OPEN**: a known, deliberate exception (the codec items, C-33, gate 3 #16).
- **NEW**: a defect that a fix made, or that I saw at the same seam during this pass.
- Severity is as in the gate records: **high**, **medium**, **low**.

---

## 1. Hashes and citation form

### 1.1 J4c pins r4

- HP:7: "the Lua binding `hub-plugin-lua-binding-v1.1-r4.md` (kit writer, sha256 9b63a7ce82ed03985f503a3838e88974fafd94382f1d99aafa0c3d0a05655ae8)".
- Computed sha256 of LB: `9b63a7ce82ed03985f503a3838e88974fafd94382f1d99aafa0c3d0a05655ae8`. **Match.**
- The J4b → J4c diff has only three changes: the title, this pin (HP:7) and one amendment-log line (HP:11). So J4c is J4b plus the r4 pin.

### 1.2 Every header pin matches its file

| Pin in | Pinned file | Pinned hash | Match |
|---|---|---|---|
| C:1, C:164 DP-3 | RC | `bf59b2b6…e52a7abf` (full) | yes |
| HC:4 | RC | `bf59b2b6…e52a7abf` (full) | yes |
| HC:4 (history) | codec rev 14 | `cd80cddf…4654` | not re-checked (history, not a pin) |
| HC:6 | UI10, UI11, UI12 | full hashes | yes (all three) |
| HC:6 | `ui-contract-v1.2-embed-candidate13.md` | `06dc745d…9305f` | yes |
| HP:7 | RC, UI10, UI11, UI12, LB r4 | full hashes | yes (all five) |
| LB:4 (supersedes list) | r3, r2 | `e7cd154a…074b`, `c24e677c…4655` | yes (both) |
| LB:103 (finding F-16) | HP J3, HP J4 | `92958d02…2c06`, `469c01f3…4e` | yes (both) |
| UI10:3 | `ui-contract-v1-draft3-candidate7.md` | `1893208b…912a56` (full) | yes |
| UI11:4-5 | UI11 candidates 1 to 3; UI10 | `f774e173…2ad5`, `b700628a…3fed`, `92e1bcba…ff8e`; `2063e897…` (full) | yes (all four) |
| UI12:4-6 | candidate13; UI10 | `06dc745d…` (full); `2063e897…7661` | yes |

**Result: every header pin matches its file. PASS.**

### 1.3 Core, HC and HP cite each other by contract name only

| Contract | Result | Evidence |
|---|---|---|
| C | PASS | C:1: "The Hub→client contract and the Hub→plugin contract are cited BY CONTRACT NAME AND CLAUSE ID ONLY". A search for `hub-client-contract*.md`, `hub-plugin-contract*.md` and revision ids finds nothing. |
| HC | PASS | HC:37: "`HP` means the Hub→plugin contract, and `Core` means the Core contract. `contracts-v1-joint-manifest.md` maps each name to its file and hash." The only revision id left is HC:9 "EV (matched to HP A4r29)", in the header's closed-items history list (allowed). |
| HP | PASS | HP:7 names both by contract name. HP:15 now reads "and the Core contract section 10 (services)". HP:468 now reads "every field of the client `session` row, Hub→client contract section 6". HP:9 and HP:11 name old files and revisions in the change list and the amendment log (allowed). No revision id appears after HP:11. |

### 1.4 Process item (not a gate item)

- **The manifest is stale.** `contracts-v1-joint-manifest.md` still lists `core-contract-v1.16.md`, `hub-client-contract-v1-candidate-parity40.md`, `hub-plugin-contract-v1.1.joint-J3.md` and `hub-plugin-lua-binding-v1.1-A4r77-r2.md`. C:1, HC:37 and HP:7 each say that this manifest maps each contract name to its file. So today every by-name citation resolves to the file before the fixes.
- JT:396 also says "against the final contracts (manifest final2)".
- *Action:* publish manifest final3 with the four fixed files and their hashes (section 0.1), then ask each Sol for the joint ACK.

---

## 2. Gate 2: cross-contract consistency

### 2.1 Section 1.1 of the gate 2/5 record (the one OPEN of the 26)

| # | Status | Evidence |
|---|---|---|
| 9 | **STILL OPEN (low)** | Not in the J3 → J4c diff. HP:478 SA-2 still: "and with kind `wrong_state` the codes `not_ended`, `not_restartable` …" (`wrong_state` is not an HP kind). HP:523 still: "(for example `restart_not_ended`, …)". *Fix:* SA-2 says "`failed{detail.code: wrong_state, reason}`"; HP:523 uses `not_ended`. |

### 2.2 Section 1.2 of the gate 2/5 record (the 12 OPEN items)

| # | Status | Evidence |
|---|---|---|
| 10 DP-4 paste policy | **STILL OPEN (low)** | Not in the v1.16 → v1.17 diff. C:165 DP-4 still: "applies the route's paste policy (OU-1)". OU-1 has no paste policy: C:142 test `conf::ou_1_attach_options_carry_no_paste_policy`. *Fix:* "applies the paste's `require_bracketed` (IN-8)". |
| 11 `max_query_text_bytes` | CLOSED | C:261: "at most `CoreLimits.max_query_text_bytes` bytes (default 1,024), without C0, DEL or C1 characters." The words "reported in `RouteLimits`" are gone. |
| 12 `attach_failed` reasons | **ACCEPTED-OPEN** | Codec item. RC is unchanged. Deferred to the first codec revision after v1 (the reason set is non-exhaustive). |
| 13 `hub/route.ended` | CLOSED | HP:383 EV-9: "**`hub/route.ended`** (a client terminal route ended; priority `high`; audience clients)". Agrees with HC:326 TS-9 (`priority: "high"`). |
| 14 `terminal/modes` | CLOSED | HC:391 EV-6: "(`title`, `cwd`, `bell`, `notification`, `prompt_mark`, `modes`, …". |
| 15 gap counter name | CLOSED | HP:782 App A: "`event_gap` (`{gap_id, owner, name, subject?, count, lost_bytes?, …`". LB:15 LB-6 matches: "`count`, `lost_bytes?` … J4b dropped `lost_events`". |
| 16 caller residue | CLOSED | HC:344 SF-0: "`operator` with its `credential_id` when present". HP:820 App B: "`caller{kind, credential_id?, hub_id?, …`". |
| 17 MT-5 client code | CLOSED | HP:401 MT-5: "a client maps it by its own rules". (Residue in LB, see N-8.) |
| 18 dangling `(T2)` | **STILL OPEN (low)** | Not in the diff. HP:63: "Lane 0 is never starved by another lane (T2)." HP:73 MD-3: "a full lane is `Backpressured` at `send` (T2)." T2 does not exist. *Fix:* cite Core SV-3. |
| 19 session row schema | CLOSED | HP:468 SS-1: "**A row is the Hub-client contract's `session` resource** (every field of the client `session` row, Hub→client contract section 6)". HP:792 App A: "every field of the client `session` row". |
| 21 route fields | CLOSED | HC:738 A.3: "core fields `id`, `session`, `binding`, …". Matches the HC `route` row (`binding`). |
| 22 operation kinds, codes | CLOSED | HC:214: "`spawn` (a restart is a `spawn` operation, HP CA-14) uses `starting`, `installed`, `failed { reason }`, `cancelled`; `safe_write` uses … `failed { reason }`, `unknown { reason }`". HC:518 MC-1c: "refuses the spawn with `internal { reason: credential_unavailable }`" (`internal` is in the closed list, HC error table). |

### 2.3 Section 1.3 of the gate 2/5 record: medium M1 to M6

| # | Status | Evidence |
|---|---|---|
| M1 OU-8 frame set | CLOSED | C:150 OU-8: "**also `output_deflate` (only when negotiated, DP-3) and any frame type that DP-3's extension rule admits** (an ignorable type in `0x40` to `0x7F`, or a type whose feature the client selected)." (Wording residue: N-5.) |
| M2 oversize plugin result | CLOSED | HC:617: "`response_too_large` before dispatch \| `none` \| `response_too_large` with `effect: none`". HC:618: "`response_too_large` after dispatch (the handler ran; HP IV-2) \| `unknown` \| `response_too_large` with `effect: unknown` (never safe to repeat)". Agrees with HP:170 IV-2. |
| M3 default origin | CLOSED | HP:323 WP-1: "**`web.set_origin{origin: null}` clears the origin: the Hub has no default origin, so with none set the Hub refuses `issue_pairing` with `wrong_state{no_web_origin}` (client PR-0).**" Tests `conf::wp_1_null_clears_the_origin_and_pairing_is_refused_no_web_origin`, `conf::wp_1_there_is_no_hub_default_origin`. Agrees with HC:450 PR-0. LB:25 LB-15 follows ("`origin = nil` CLEARS the origin"). |
| M4 signal endpoint | CLOSED | HP:324 WP-2: "**On a served origin (WP-1, WP-5) the Hub itself serves `POST <origin>/hubs/<hub_id>/signal`**, bounded by `max_relay_envelope_bytes` and **handled as a relay attempt** …; **on an external origin the plugin carries it**". Tests `conf::wp_2_served_origin_hub_answers_post_signal_as_a_relay_attempt`, `conf::wp_2_external_origin_plugin_carries_the_signal`. (Two seam residues: N-1, N-2.) |
| M5 context-file owner | CLOSED | HC:518 MC-1c: "**This clause owns its schema** (HP CA-14 delivers it unchanged): a UTF-8 JSON object `{ prompt?, context: { <context_key>: string }, session: SessionRef, session_type_id?, session_type_dir?, target_id?, worktree_id?, worktree_path?, branch?, repo? }`". HP:256 CA-14: "the Hub delivers the session context to the program exactly as client MC-1c defines it … this contract restates none of its content". No circle now. |
| M6 file-name citations | CLOSED | See 1.3 (HC:37, HP:15, HP:468). |

### 2.4 Section 1.3 of the gate 2/5 record: low L1 to L19

| # | Status | Evidence |
|---|---|---|
| L1 | CLOSED | C:164 DP-3: "`0x81 input` (JSON, the shapes of A.5: … `paste_abort`, `file_begin`, `file_commit`, `file_abort`, `reply_terminal_query`)". |
| L2 | CLOSED | C:166 DP-5: "plus `write_failed` (a file could not be written on the host, DP-5b) and the query-reply reasons of EV-8 (`already_replied`, `query_expired`, `unknown_query`, `invalid_input`, `too_large`)". |
| L3 | CLOSED | C:77 IN-2 `NotWritten` now lists "`PayloadTooLarge` (**route-only**: a host write over the limit is refused synchronously at `begin`, but a route input is admitted by the worker, so it is reported as this reason)". Agrees with C:334 and C:380 (host `begin` refusal). |
| L4 | CLOSED | C:142 OU-1: "`RouteLimits{…, max_query_reply_bytes, max_file_bytes, query_deadline, …}`". |
| L5 | **ACCEPTED-OPEN** | Codec item (refusal at `file_begin` not stated in RC). Deferred to the first codec revision after v1. |
| L6 | CLOSED | HP:383 EV-9: "**`hub/notice`** (the notice event of VW-9: audience clients, and **never a reaction source**)". |
| L7 | CLOSED | HP:260 CA-22: "which goes only to clients that are not attached to that session (client SF-5c, SF-5d)". |
| L8 | **STILL OPEN (low; HP half only)** | HC half is fixed. HC:917: "the request is refused **before any operation starts**, with `conflict { reason: requirement_conflict \| ambiguous_provider \| requirement_missing, … }` and `effect: none`, … a later execution failure ends it `failed { reason }`". HP half is not in the diff. HP:925 PD-6 still: "**no provider in range** is `requirement_conflict{interface, range, found}`; … else `ambiguous_provider{interface, candidates}`" (no HP kind). *Fix:* PD-6 writes `failed{detail.code: requirement_conflict, …}` and `failed{detail.code: ambiguous_provider, …}`, as it already does for `requirement_missing`. |
| L9 | CLOSED | HC:212 `view.component`: "`status_badge`, and `embed` under the feature `ui_embed`, ui-contract 1.2". (Seam residue: N-6.) |
| L10 | CLOSED | HP:455 VW-10: "no `allow-same-origin` for the app, no top navigation, no popups);" ("by default" is gone). |
| L11 | CLOSED | HP:338 AS-2: "(client A.9 `open_asset`; ui-contract UI-EM-7)". |
| L12 | **STILL OPEN (low; ui owner)** | UI12 is unchanged. UI12:37: "An asset is deleted with its plugin on unload (NT-8 style)." HP:340 AS-4 governs (delete on disable, uninstall or quarantine). Not on the list of deliberate exceptions. *Fix:* next ui revision says "deleted on disable, uninstall or quarantine (HP AS-4)". |
| L13 | CLOSED | HC:885 A.9: "`{ session_type_id, target_id?, worktree_id?, prompt?, context?: { <context_key>: string } …`". Agrees with HP:771 App A. (Summary residue at HC:226: N-7.) |
| L14 | CLOSED | HP:771 App A: "`environment?: {name: string}` (only names in the session type's `allowed_environment_overrides`, bounded by `max_env_bytes`)". LB:28 LB-16b matches. (HP:900 LN-2 residue: see gate 3 PAR-3.) |
| L15 | CLOSED | HC:885: "a `target_id` that differs is `invalid_arguments`". |
| L16 | CLOSED | HC:613: "refused before dispatch: `unavailable`, `failed` (other reasons), or a kind this table does not name". `instruction_budget` is gone. |
| L17 | CLOSED | HC:894: "`{ text, cursor?: { row, col, visible }, model_rev, history_unavailable? }` (as HP SA-1 returns …)". |
| L18 | CLOSED | HP:323 WP-1: "pairing pins the Hub's keys (client PR-3)". HC:453 PR-3 is the pin clause. |
| L19 | CLOSED | VW-1: HP:449 "the **admitted entity family of the same package** (admitted under UI-AD-1)". VW-6: HP:454 "the part set is the UI contract's part set (the UI Nodes vocabulary the clients render)". The substance of UI10:330 and UI10:332 is applied. (Cosmetic: HP:454 has a stray "**:" after "placeholder."; the "six data parts and seven structural parts" count is not stated. No behavior change.) |

### 2.5 New at the same seams (this pass)

No new item is medium or high.

| # | Severity | Seam | Quotes | Fix |
|---|---|---|---|---|
| N-1 | low | HC PR-4 against HP WP-2 (M4 fix) | HC:459 PR-4: "The web plugin carries them between that origin and the Hub through the untrusted relay of HP WP-2 to WP-4". HP:324 WP-2: "On a served origin … the Hub itself serves `POST <origin>/hubs/<hub_id>/signal`". The client posts to the same URL in both cases, so no client test changes. | HC PR-4 adds: "on a served origin the Hub answers that endpoint itself (HP WP-2)". |
| N-2 | low | HP WP-5 against WP-2 and WP-6 (M4 fix) | HP:327 WP-5: "The Hub serves **`GET` and `HEAD` only** from that root **directly** (no handler runs per request)". HP:324 WP-2 adds a Hub `POST …/signal` on the same served origin; HP:328 WP-6 already adds `…/ice`. WP-5 names no exception. | WP-5 adds: "except the Hub's own `/hubs/<hub_id>/ice` and `/hubs/<hub_id>/signal` endpoints (WP-2, WP-6), which a static route cannot shadow". |
| N-3 | low | J3 against WP-2 (M4 fix) | JT:400: "signalling goes to `<web origin>/hubs/<hub_id>/signal` through the plugin relay". That is true only on an external origin (HP:324). | J3 names the origin kind, or runs both legs (served: the Hub answers; external: the plugin relays). |
| N-4 | low | J7 against HC error codes | JT:404: "A caller without the client permission is refused `capability_denied`". `capability_denied` is the HP name (HP:442 EN-9). A client sees `permission_denied` (HC error table; HC:607 maps `capability_denied` to `permission_denied`). | J7 asserts client `permission_denied`. |
| N-5 | low | C OU-8 wording (M1 fix) | C:150: "Nothing else is written to a route: **also `output_deflate` …**". The sentence says "nothing else" and then adds more. The meaning is clear. | "Also written: `output_deflate` … . Nothing else is written to a route." |
| N-6 | low | HP VW-1 against HC:212 (L9 fix) | HP:449 VW-1 lists "the data components `list`, `table`, `detail`, `form`, `actions`, `status_badge`" with no `embed`. HC:212 and HP:455 VW-10 now have `embed`. | VW-1 adds "and `embed` under `ui_embed` (VW-10)". |
| N-7 | low | HC section 6 summary against A.9 (L13 fix) | HC:226: "`create(session, { session_type_id, target_id?, context? })`". HC:885 A.9 also has `worktree_id?` and `prompt?`. A.9 governs. | HC:226 lists the same fields as A.9, or points to A.9. |
| N-8 | low | LB finding text against HP MT-5 (#17 fix) | LB:85 F-6: "(its own kind; a client maps it to its `result_too_large` code)". HP:401 now: "a client maps it by its own rules". F-6 is in a findings section. | Next LB revision drops the parenthesis. |
| N-9 | low | HP:7 wording against LB r4 | HP:7: "**Pinned by hash** (files that never cite this contract): … the Lua binding `hub-plugin-lua-binding-v1.1-r4.md`". LB:5: "Against: **the Hub→plugin contract, by name**". LB now cites HP (by name, so there is no hash cycle). | HP:7 says "(files that never pin this contract by hash)". |
| N-10 | low (no gate effect) | HP CA-14 tests placement (PAR-3 fix) | HP:261 is the CA-20 (port) row. It holds `conf::ca_14_spawn_with_worktree_id` and `conf::ca_14_ensure_worktree_request_branch_base_ref_reuse`. HP:256 CA-14 does not. Each id is in one row only, so it is the "prefix names another clause" case of gate 4. | Move both ids to the CA-14 row. |

### 2.6 Gate 2 result

- No high or medium item is still open. M1 to M6 are CLOSED.
- Still open (all low): #9, #10, #18, L8 (HP half), L12 (ui).
- **Gate 2: PASS.**

---

## 3. Gate 3: requirements coverage (the 16 PARTIAL rows)

| # | Status | Evidence |
|---|---|---|
| #16 | **ACCEPTED-OPEN** | Hub-internal ownership. A plan item, not a contract clause. |
| #19 | CLOSED | It closes with #54 (below). |
| #54 | CLOSED | HC:204 `session` verbs: "**update** (`{ label?, task? }` as a merge patch; permission `session.update`; #54)". HC:814 A.7: "`session` \| `update` (`label`, `task`) \| committed". HC:887 A.9: "`session` \| `update` \| a merge patch of `{ label?, task? }`". HC:222 KR-1 test `conf::kr_1_session_update_label_and_task`. |
| #97 | CLOSED | HC:222 KR-1: "**The rules stated in the kind rows above are normative, and each has a named test** (gate 3)", with `conf::kr_1_session_lifecycle_class_mapping` (A.2c). |
| #101 | CLOSED | HC:222 KR-1: `conf::kr_1_plugin_log_load_id_and_markers`. |
| PAR-3 | **STILL OPEN (PARTIAL)** | Parts a, b and d are fixed: (a) HP:261 `conf::ca_14_spawn_with_worktree_id`, `conf::ca_14_ensure_worktree_request_branch_base_ref_reuse`; (b) HP:211 "`worktrees.{list,show,delete}` … `worktree_delete[:any]` for delete (CA-21)"; (d) HC:222 `conf::kr_1_session_create_in_existing_worktree`. **Part c is not fixed.** HP:900 LN-2 still: "`botster.session_types.spawn` \| `session_type_spawn` \| `{session_type_id, target_id?, cwd?, environment?, context?}`". It has no `worktree_id?` and no `prompt?`, and its `environment?` has no bound. HP:771 App A and HP:256 CA-14 have all three. *Fix:* the LN-2 row copies the App A argument list. |
| PAR-16 | CLOSED | HP:262 CA-18 test `conf::ca_18_presentation_default_plugin_hidden`. HC:222 KR-1 test `conf::kr_1_session_presentation_hides_from_default_list_not_from_list`. |
| PAR-19 | CLOSED | One source for `title`. HC:204: "`title?` (**only** the terminal's latest title, from the Core model; nothing else writes it; G-21)". HP:477 SA-1: "`sessions.update{session, label?, task?, metadata?}` … writes the session's `label` and `task` (… the client `title` is the terminal title only and no plugin writes it)". HP:741 and HP:870 also use `task?`. LB:28 LB-16b matches. Tests: HC:222 `conf::kr_1_session_title_is_terminal_only`, `conf::kr_1_session_owner_worktree_branch_fields`. |
| PAR-24 | CLOSED | HP:37: "plugin-supplied UI code other than the sandboxed `embed` view (VW-10, AS-1 to AS-4) (entities plus declarative views otherwise)". |
| PAR-26 | CLOSED | HC:76: "**The v1 shipped set** includes the first-party messaging and workspaces plugins (zero plugin regressions: the workspaces plugin places sessions for other plugins through HP PC-1 to PC-5)." The mechanism is tested by PC-1 to PC-5. *Note:* no named fixture places a session for another plugin. The gate 3 fix proposed one. I recommend it for the build plan. It is not a PARTIAL cause, because the stated defect ("no clause puts a first-party workspaces plugin in the v1 shipped set") is removed. |
| PAR-32 | CLOSED | HC:204: "`task?` (a short text that a client, an agent or a plugin sets with `update`, HP SA-1; legacy `task`, parity CG-4)". HC:222 KR-1 test `conf::kr_1_session_activity_idle_after_parameter`. |
| PAR-33 | CLOSED | HC:222 KR-1 test `conf::kr_1_worktree_active_sessions`. |
| PAR-36 | CLOSED | HC:222 KR-1 test `conf::kr_1_hub_state_starting_recovering_ready`. |
| PAR-37 | CLOSED | HC:222 KR-1 tests `conf::kr_1_session_type_script_runs_as_command`, `conf::kr_1_spawn_target_browse_directories_only_operator`. |
| UD-4 | **STILL OPEN (PARTIAL)** | It closes with PAR-3, PAR-16, PAR-19, PAR-24 and PAR-26. Only PAR-3 is still open. |
| UD-11 | CLOSED | It closes with PAR-24 (CLOSED). |

**New contradiction check at these seams:**
- `session.update` (HC:204) against HP SA-1 (HP:477) and LB-16b (LB:28): the field sets agree (`label`, `task`; `metadata` is plugin-only and read-only to clients). No contradiction.
- `title` has one writer (the Core model). No clause in HC, HP or LB lets anyone else write it.
- PAR-3 part c (HP:900) is now also an intra-HP mismatch with the L14 fix (HP:771 bounds `environment`; HP:900 does not).

**Gate 3: FAIL.** 2 rows are still PARTIAL: PAR-3 (HP:900 LN-2 row) and UD-4 (which depends on PAR-3). One fix to HP:900 closes both.

---

## 4. Gate 4: test ids and journeys

### 4.1 Clauses without a test id, and the duplicate

| Item | Status | Evidence |
|---|---|---|
| HC BD-3 | CLOSED | HC:193 BD-3: "A client never assumes one: it uses only the values the Hub sent, … \| `conf::bd_3_client_uses_only_welcome_limits` (FakeHub sends unusual limits and the client obeys them)". |
| LB-2 | CLOSED | LB:11: "*Test: `conf::lb_2_unknown_argument_key_is_invalid_request`.*" |
| LB-5 | CLOSED | LB:14: "*Test: `conf::lb_5_caller_and_reference_spelling`.*" |
| LB-10 | CLOSED | LB:19: "*Test: `conf::lb_10_register_builds_ready_and_refuses_after_ready`.*" |
| LB-11 | CLOSED | LB:20: "*Test: `conf::lb_11_http_and_ws_are_async_operations`.*" |
| LB-12 | CLOSED | LB:21: "*Test: `conf::lb_12_session_calls_and_scopes`.*" |
| LB-13 | CLOSED | LB:22: "*Test: `conf::lb_13_operation_record_shape`.*" |
| LB-14 | CLOSED | LB:24: "*Test: `conf::lb_14_process_and_plugin_call_async_and_typed`.*" |
| LB-15 | CLOSED | LB:25: `conf::lb_15_files_assets_web_relay_shapes`. |
| LB-16 | CLOSED | LB:26: "*Test: `conf::lb_16_other_call_shapes`.*" |
| LB-17 | CLOSED | LB:29: "*Test: `conf::lb_17_manifest_keys_closed`.*" |
| LB-18 | CLOSED | LB:27: `conf::lb_18_definition_and_bindings`. |
| Core duplicate id | CLOSED | C:164 DP-3 no longer lists `conf::dp_5_confirmed_paste_resend_is_a_new_op_with_require_bracketed_false`. It is only in C:166 DP-5. My script finds 0 duplicate ids in C, RC, HC and HP. |
| WP-1/PR-0 conflict | CLOSED | See M3 (HP:323, HC:450). |

**Script result on the fixed files** (rows with no `conf::` or `bench::` id):
- C: `LC-8` only (a removed row). No finding (as before).
- RC: none.
- HC: `HS-6`, `PR-9` only (judged acceptable by the gate 4 record). BD-3 now has a test. KR-1 (new) has 12 tests.
- HP: `ST-10` only (acceptable, by absence).
- UI10, UI11, UI12: unchanged; none.

**NEW, STILL OPEN (gate 4 blocker): LB-16b has no test id.**
- LB:28: "- **LB-16b Session update and spawn arguments (J4b).** `botster.sessions.update{ session, label?, task?, metadata? }` (scope `session_update[:any]`); … `environment` is `{ [name] = string }` with only names in the session type's `allowed_environment_overrides`, bounded by `max_env_bytes` (else `invalid_request`)." The bullet ends with no "*Test:*".
- LB:91 section F says: "Every clause has one test. … and the eleven ids named in LB-2, LB-5 and LB-10 to LB-18." LB-16b is a twelfth behavioral clause. It changes what a Lua plugin may pass, so it is behavioral by the gate 4 rule. HP `conf::sa_1_update_own_metadata_only` tests the Hub side only.
- *Fix:* add "*Test: `conf::lb_16b_session_update_and_spawn_argument_shapes`.*" to LB:28, and make section F say "twelve ids".

### 4.2 Journeys

| Journey | Status | Evidence |
|---|---|---|
| J1 | CLOSED | JT:398 is the proposed text: "Setup for `webrtc`: install and enable the web plugin fixture, approve its `web_origin` and `relay_signaling` grants, and pair (J3). Type and see the echo; no success frame arrives for keys or text. A paste with `want_result: true` gets `input_done`; a paste over `max_paste_bytes` gets `input_refused{too_large}`." Agrees with C:166 DP-5. |
| J3 | CLOSED (low residue N-3) | JT:400: "With no web origin, `issue_pairing` is refused `wrong_state{no_web_origin}`." Agrees with the fixed HP:323 WP-1 and HC:450 PR-0. The relay wording fits an external origin only (N-3). |
| J6 | CLOSED | JT:403: "Lost event copies are not replayed; a consumer of the crashed producer keeps its subscription; a session-family subscriber gets a fresh baseline". The words "subscribers see gaps" are gone. The package verbs are in: "Install from a `local` source, `approve_grants`, enable … `invoke(package, reload)`". `plugin_log` "`restarted` marker" agrees with HC:213 `{ restarted: load_id }`. |
| J7 | CLOSED (low residue N-4) | JT:404: "Views of the workspaces plugin and the HP VW-7 messaging inbox fixture … an agent that holds it sees the same projection." Agrees with HP:442 EN-9. The refusal code name is the HP one (N-4). |

**Gate 4: FAIL.** One behavioral clause names no test: LB-16b (LB:28). Everything the gate 4 record found open is CLOSED.

---

## 5. Gate 5: stability

| # | Status | Evidence |
|---|---|---|
| C-33 | **ACCEPTED-OPEN** | C:164 DP-3 still: "**A change to any of it needs a new extract and hash agreed by both pairs.**" RC still holds Hub-only text. Deferred to the first codec revision after v1. |
| P-33 | CLOSED | HP:15: "and the Core contract section 10 (services)". HP:468: "Hub→client contract section 6". |
| G1 | CLOSED | HC:363: "**Unknown focus counts as not focused:** a Core focus state of `unknown` (an unreachable or older worker) sets `attention` and allows push, so a notification is never lost to an unknown state." |
| G2 | CLOSED | HC:882 `swap`: "**Compatibility is not adoption** (Core AD-4): a compatible worker that then fails authentication or reachability (Core AD-6, AD-2) appears as a `lost` session after the swap, attributed to adoption, not to the swap check." |
| G3 | CLOSED | HC:360: "**Exactly one surface per client** (HP EV-9, CA-22)". HC:518: "(HP CA-14 delivers it unchanged)". No HP revision id is left in HC normative text (HC:9 is header history). |
| G4 | CLOSED | With M1 (C:150). |
| G5 | CLOSED | HC:545: "**Echo not confirmed by `echo_deadline_ms` (an HP parameter)**". HC:546: "an echo that arrives within `cleanup_window_ms`". |

Citation rule (1.3): PASS. Hash pins (1.2): PASS.

**Gate 5: PASS.** (Process item 1.4, the stale manifest, must be fixed before the joint ACK. It is not a gate 5 clause defect.)

---

## 6. Summary tables

### Gate 2

| Group | Items | CLOSED | STILL OPEN | ACCEPTED-OPEN |
|---|---|---|---|---|
| 1.1 (#9) | 1 | 0 | 1 (#9, low) | 0 |
| 1.2 (12 OPEN) | 12 | 9 | 2 (#10, #18; low) | 1 (#12) |
| M1 to M6 | 6 | 6 | 0 | 0 |
| L1 to L19 | 19 | 16 | 2 (L8 HP half, L12 ui; low) | 1 (L5) |
| **Total** | **38** | **31** | **5 (all low)** | **2** |
| New (section 2.5) | 10 | — | 10 (all low) | — |

**Verdict: PASS** (no medium or high item is open).

### Gate 3

| Group | Items | CLOSED | STILL OPEN | ACCEPTED-OPEN |
|---|---|---|---|---|
| PARTIAL rows | 16 | 13 | 2 (PAR-3, UD-4) | 1 (#16) |

**Verdict: FAIL** (2 PARTIAL rows remain; one fix to HP:900 closes both).

### Gate 4

| Group | Items | CLOSED | STILL OPEN | ACCEPTED-OPEN |
|---|---|---|---|---|
| Clauses with no test id (BD-3, 11 LB) | 12 | 12 | 0 | 0 |
| Core duplicate test id | 1 | 1 | 0 | 0 |
| WP-1/PR-0 conflict | 1 | 1 | 0 | 0 |
| Journeys J1, J3, J6, J7 | 4 | 4 | 0 | 0 |
| **Total** | **18** | **18** | **0** | **0** |
| New | 1 | — | 1 (LB-16b has no test id) | — |

**Verdict: FAIL** (1 clause with no test id: LB-16b).

### Gate 5

| Group | Items | CLOSED | STILL OPEN | ACCEPTED-OPEN |
|---|---|---|---|---|
| C-33, P-33 | 2 | 1 | 0 | 1 (C-33) |
| G1 to G5 | 5 | 5 | 0 | 0 |
| **Total** | **7** | **6** | **0** | **1** |
| Citation rule; hash pins | 2 checks | PASS; PASS | — | — |

**Verdict: PASS.**

### What closes the package

1. HP:900 LN-2 `botster.session_types.spawn` takes App A's arguments: `worktree_id?`, `prompt?`, and the bounded `environment?` (closes PAR-3 and UD-4; gate 3 then passes).
2. LB:28 LB-16b names a test id, and LB:91 counts it (gate 4 then passes).
3. Publish manifest final3 with the fixed files (1.4).
4. Optional, low: #9, #10, #18, L8 (HP PD-6), L12 (ui), N-1 to N-10.
