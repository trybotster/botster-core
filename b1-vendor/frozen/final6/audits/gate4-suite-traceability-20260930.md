# Gate 4: cutover acceptance suite ↔ contracts traceability (2026-09-30)

Read-only audit. Nothing was built or run.
- **Suite:** `tools/cutover-e2e/` (README, `run.sh`, `lib.sh`, `legs/a,b,c`, `scenarios.py`, and the `tui-*-check.sh` scripts), plus the TUI live tests those scripts call (`tui-app-split/crates/botster-tui/tests/live_tui.rs`).
- **Contracts:** Core `core-contract-v1-draft.md` (draft 40, prefix `C:`), Hub→plugin `hub-plugin-contract-v1.md` (FROZEN v1.1 A4r5, prefix `HP:`), Hub→client `hub-client-contract-v1-draft.md` (freeze held for T3, SW and BP, prefix `HC:`).
- **Results used:**
  - `evidence/cutover-e2e/scenarios-v3-135607` (the last full leg-a run);
  - `scenarios-ids-135922` (the latest partial run, 9 scenarios);
  - `20260930-130306` (leg b: TUI T-S1 to T-S3);
  - `20260930-134407` (leg c).
  - Candidate `hub-6b4813a9`.

## Summary

**29 checks mapped** (20 leg-a scenarios, 7 leg-b checks including the missing Web driver, 2 leg-c proofs).
- **20 need a change.** 15 of them contradict a clause, rely on a removed mechanism, or accept behavior the contract forbids; the other 5 exercise no clause or assert too loosely.
- **3 are unimplemented placeholders:** tui-large-paste, tui-slow-reader and Web.
- **Of the 103 user-visible clause rows in table B, 74 have no scenario, and 13 more are covered only partly.** Of the 16 that are covered, only 9 are covered cleanly.

Status today:
- **Leg a:** 11 PASS, 1 FAIL (s10), 4 XFAIL (s05, s08, s09, s18), 2 SKIP (s08b, s13). s09b PASSes in the partial run, and s01 and s03 PASS in the full run.
- **Leg b:** T-S1 to T-S3 PASS. T-S4 has no recorded run. Large-paste, slow-reader and Web SKIP.
- **Leg c:** test and clippy FAIL.
- `run.sh` defaults to legs `a,b,c,d`, but there is no `legs/d-*.sh`, so a default run always records a SKIP.

**Systemic findings**

1. **Native Hub messaging.** s04, s06, s07, s10, s11, s12 and s16 call the native `post_message`, `receive_messages` and `ack_message` MCP tools. s09 and s09b call the native `notify_session`. HC:MC-2 removes all of these: messaging is the messaging plugin's (HC:MC-3, HP:9.6).
   - s10 is the contract's named red case for HC:MC-3, and it must be re-pointed at the plugin tools.
2. **Caller-chosen session ids.**
   - `Hub.spawn_sink` falls back to `sessions spawn --session-id` when the Hub does not mint an id (the partial run notes "used caller-chosen ids").
   - Every TUI live test spawns with `DaemonRequest::Spawn { session_id: "live-tui-…" }`.
   - Both contradict HC:§6 (Hub-minted ids) and HP:SS-5 (`conf::ss_5_caller_cannot_choose_id`).
3. **Hub-terminated routes and the old close event.**
   - The TUI tests read the Hub's `live_attach_occupancy` (subscription_id, generation, stream_epoch) as their oracle. HC:P3 and HC:§6 "Removed from the client wire" remove it, and HC:TS-3 puts no generation or epoch on the wire.
   - The TUI's detach path waits on `DaemonEvent::TerminalSubscriptionClosed` on the control connection (`hub_io.rs:933`, `:1678`). HC:TS-8 replaces it with `route_closed` on the route.
   - T-S2 asserts "a new generation" where HC:TS-11 says "a new route".
   - All of these pass against today's Hub-terminated route, so none of them proves HC:BP-1.
4. **The doorbell scenarios contradict HP:SW-4.**
   - s08 rings a **raw, non-echoing** sink and expects the text to arrive. SW-4 says a session that does not echo the probe never receives the text.
   - s08 and s09b assert "no `zx`". SW-4 says the probe does reach a non-echoing session, and the completion reports `probe_written: true, residue: possible`.
5. **s05 contradicts HP:SA-4.** It restarts a freeform `sessions spawn -- sh -c …` session. SA-4 says "a plain spawn never has a record", so restart must be refused `wrong_state{not_restartable}` (HC:§6 restart).

---

## Table A: suite → clauses

Status is the latest recorded result. "Verdict": **OK** (maps cleanly); **CHANGE** (contradicts a clause or relies on a removed mechanism); **LOOSE** (it exercises clauses but asserts less than, or other than, they define); **NO-CLAUSE** (the behavior is not defined by any clause); **STUB** (not implemented).

| Check | What it checks | Clauses exercised | Status | Verdict and reason |
|---|---|---|---|---|
| s01 | A Hub starts from the candidate, reports running, and its environment has no `BOTSTER_*` | none (harness hygiene; closest are HC:HS-3 and HC:HS-7) | PASS | **NO-CLAUSE.** Keep it as a harness precondition, not as contract evidence. |
| s02 | An id-less spawn is listed `running` | HC:§6 session create (freeform), HC:VB-1, HP:SS-5 | PASS | **CHANGE.** The shared `spawn_sink` falls back to `--session-id` (HP:SS-5, HC:§6). It does not check the accepted receipt `{operation_id, session, state: starting}` (HC:§6 create, A.7). |
| s03 | `sessions shutdown` → the state leaves running → the payload pid is gone | C:LC-5, HC:§6 `stop`, HC:A.2c | PASS | **LOOSE.** It asserts `!= running`, not `ended` with `exit{code, signal}` (HC:A.2c, C:EV-4). OK otherwise. |
| s04 | kill -9 the Hub, restart, both sessions are `running`, and their own tokens message each other | C:LC-12, C:AD-1, C:AD-3, HP:VC-3 (`vc_3_token_survives_hub_restart_same_run`), HP:SS-5 (`adoption_keeps_id`), HC:MC-3 | PASS | **CHANGE.** Native `post_message` and `receive_messages` (HC:MC-2); `hub.sessions.list` is not an HC:MC-2 name (`list_sessions`). |
| s05 | An ended session restarts under the same id | HC:§6 restart, HP:SA-2, HP:SA-4, HP:SS-5 (`restart_keeps_id`), HP:VC-3 (new token) | XFAIL | **CHANGE.** It restarts a freeform spawn, which HP:SA-4 makes non-restartable. Use a session-type spawn, and assert the new token and the old token refused (HP:VC-3). |
| s06 | Messaging tools are listed; a forged token and no token get 401 | HC:MC-1, HP:VC-1, HP:VC-3 | PASS | **CHANGE.** It requires `post_message`, `receive_messages`, `ack_message` and `notify_session` as Hub tools, which HC:MC-2 removes. It should assert the MC-2 native list. |
| s07 | A posts to B; B receives with A's identity; ack removes it | HC:MC-3 (lease and ack), HP:VC-1, HC:MC-1 | PASS | **CHANGE.** Native message tools (HC:MC-2). Move to the messaging plugin's tools. |
| s08 | A ring to an idle, unattached session: exact text once, no probe residue, and it waits while the cursor is hidden | HC:MC-4, HP:SW-3 (`waits_for_visible_cursor`), HP:SW-4, HP:SW-7 | XFAIL | **CHANGE.** The sink is `raw -echo` (non-echoing), so SW-4 forbids the text it expects. The ring is triggered by native `post_message`. |
| s08b | A human typing during a ring keeps their input; the ring is abandoned | HP:SW-4 (`human_edge_abandons_never_erases_human_input`), C:IN-10, HP:RW-3 | SKIP | **STUB.** Needs a route client that types. |
| s09 | `notify_session` types the text into an echoing session | HP:SW-1, HP:SW-2, HP:SW-4 (`echoing_sink_receives_text`) | XFAIL | **CHANGE.** Native `notify_session`; the ring is the messaging plugin's (HC:MC-2). The echo sink is correct. |
| s09b | A non-echoing session gets neither the text nor `zx` | HP:SW-4 (`non_echoing_sink_receives_nothing`), HC:MC-4 | PASS (vacuous: nothing is typed today) | **CHANGE.** It asserts "no `zx`", but SW-4 says the probe does reach the session. It should assert text absent, `probe_written: true`, `residue: possible`, and `abandoned{no_echo}`. Native `notify_session`. |
| s10 | An unacked message survives kill -9 between receive and ack | HC:MC-3 (`unacked_survives_kill9`), HP:ST-9, HP:VC-3 | **FAIL** | **CHANGE.** It is the right red case (named by HC:MC-3) on the wrong layer. It must run through the messaging plugin over HP:ST-9. |
| s11 | whoami names the caller; a session cannot read another's inbox | HC:MC-1, HC:MC-2 (`whoami`), HP:VC-4, HC:SF-0 (plugin policy) | PASS | **CHANGE.** The inbox half uses native `receive_messages`. Isolation is now plugin policy over the verified caller (HP:VC-4, HP:VW-7). |
| s12 | A flooding session: status stays fast, messaging still works, Hub RSS < 1 GB | C:DP-1 (host off the byte path; implied), HP:EV-10 (Hub not stalled) | PASS | **NO-CLAUSE / CHANGE.** No clause states control-plane latency or a Hub memory bound under output flood. Uses native `post_message`. |
| s13 | A slow reader does not stall other clients | HC:TS-7, C:OU-3(c), C:OU-6 | SKIP | **STUB.** Duplicates tui-slow-reader. |
| s14 | A plugin package installs, enables, and its tool answers over MCP | HP:LD-1, HP:HS-1, HP:HS-2, HP:CP-1, HP:CP-2, HP:MT-1, HP:MT-3, HP:MT-4, HC:§6 package create (local path) | PASS | **LOOSE.** It matches the tool by substring. HC:MC-2 fixes `<package>__<tool>`. |
| s15 | A failing plugin tool reports an error; the Hub and other calls work | HP:MT-5, HP:ER-5, HC:MC-5, HP:LM-4 | PASS | **LOOSE.** It accepts a JSON-RPC `error` or `isError`. HC:MC-5 and HP:§14 require `isError: true`, with the Error object (with `effect`) in `structuredContent`, for a handler failure. |
| s16 | After `down` and `start`, sessions are running and their tokens message each other | C:LC-12, C:AD-1, HP:VC-3, HC:MC-3 | PASS | **CHANGE.** Native message tools (HC:MC-2). |
| s17 | Two Hubs with separate data dirs do not interfere | none (C:LC-2 is about the **same** dir) | PASS | **NO-CLAUSE.** Harmless. The clause that should be tested is the same-dir refusal (ADD item 26). |
| s18 | Id-less spawns get distinct 128-bit ids, never reused | HC:§6 ids, HP:SS-5 (`recreate_gets_new_id_never_reused`) | XFAIL | **OK.** It is the spec. The 32-hex format is stricter than the clause ("128 random bits"); acceptable. |
| b: T-S1 | Spawn, attach the TUI over the route socket, type, see the echo | HC:TS-1, HC:TS-2, HC:TS-4, HC:TS-4b, HC:TS-5, HC:TS-5b, HC:BP-2 (partial) | PASS | **CHANGE.** Spawns with a caller-chosen `session_id` (HP:SS-5). Its oracle is `live_attach_occupancy` / generation / stream_epoch (HC:P3, §6 removed, TS-3). It passes on a Hub-terminated route, so it does not show BP-1. |
| b: T-S2 | Detach, then re-attach "with a new generation"; the echo is visible | HC:TS-10, HC:TS-11, HC:TS-8, HC:TS-4 | PASS | **CHANGE.** Same id and oracle issues as T-S1. It waits on `TerminalSubscriptionClosed` on the control connection instead of `route_closed{detached}` on the route (HC:TS-8). It should assert a new `route_id`, not a generation (TS-11). |
| b: T-S3 | Unsafe multi-line paste asks consent; the block arrives once; the route stays attached | HC:TS-5d, HC:TS-5e, HC:TS-5c (`confirmed_paste_resend_is_new_op`), C:IN-8 | PASS | **CHANGE.** Caller-chosen id and occupancy oracle, as T-S1. The paste semantics map cleanly. |
| b: T-S4 | After an outer PTY resize, the PTY size equals the pane; input still echoes | HC:TS-6 (single route), HC:TS-4d, C:DP-6, C:SZ-1 | not run yet | **CHANGE.** Caller-chosen id. One route only, so Latest across routes is untested (ADD item 20). |
| b: large-paste | A paste > 64 frames arrives complete and once | HC:TS-5d, HC:BP-3, HC:TR-3 | SKIP | **STUB.** |
| b: slow-reader | A client that stops reading does not stall another | HC:TS-7, C:OU-3, C:OU-6 | SKIP | **STUB.** |
| b: web | Web driver | (would cover HC:TR-*, PR-*, BP-4) | SKIP (no driver) | **STUB.** |
| c: test | `./test.sh --locked` exits 0 | none (build gate) | FAIL | **NO-CLAUSE.** Correct as a process gate. |
| c: clippy | Strict clippy exits 0 | none (build gate) | FAIL | **NO-CLAUSE.** Correct as a process gate. |

---

## Table B: user-visible clauses → coverage

A user-visible clause is one whose behavior a user, a client or an agent can observe end to end. Coverage: a scenario id, **partial** (the scenario touches it but does not assert the clause), or **NONE**.

### Core (observable through the Hub)

| Clause | Behavior | Covered by |
|---|---|---|
| C:P9 / C:EV-7 / C:OU-12 | Terminal transparency: OSC 52, OSC 8, OSC 9/777, BEL, title, cwd, OSC 133 and modes pass unfiltered | NONE |
| C:EV-8 | Queries answered by the most recently active client first, then the shadow terminal; exactly one reply | NONE |
| C:EV-4 | Exit carries code **and** signal | NONE (s03 does not check) |
| C:LC-2 | A second Hub on the same data dir is refused, and the first is unaffected | NONE |
| C:LC-4 | A start failure is typed; the session ends with a failure reason | NONE |
| C:LC-5 | Stop ends the payload; the worker keeps the final model | s03 (partial: payload only) |
| C:ST-5 | Screen reads succeed after exit, including after adoption | NONE |
| C:LC-12 | A Hub exit or crash never ends sessions | s04, s16 |
| C:AD-1 / C:AD-3 | Adopted sessions behave as started ones | s04, s16 |
| C:AD-2 | A lost worker gives a typed `lost` / `ended{worker_lost}` | NONE (TUI T-S12 exists outside the suite) |
| C:AD-4 | After a Core upgrade, N−1 workers are adopted | NONE |
| C:DP-1 / C:DP-8 | Bytes keep flowing with the host dead; routes survive a host restart | NONE |
| C:DP-6 | Latest size policy | T-S4 (partial: one route) |
| C:DP-9 | A flooding route cannot starve another route's input | NONE |
| C:OU-3 / C:OU-6 | A progressing reader is lossless; a slow route delays no one | NONE (s13 and slow-reader are stubs) |
| C:OU-5 | A peer fault closes only that route | NONE |
| C:OU-7 | The final PTY tail is delivered before exit | NONE |
| C:IN-8 | The paste precondition refuses with exact zero | T-S3 |
| C:IN-9 | Semantic keys and mouse are encoded by the worker from live modes | NONE (TUI T-S9 exists outside the suite) |
| C:IN-10 | A guarded write never lands after human input | NONE (s08b stub) |
| C:TM-4 | Silence edge (`hub/session.silent`) | NONE |
| C:SV-8 | A plugin with no Hub is killed after the orphan grace | NONE |

### Hub→client

| Clause | Behavior | Covered by |
|---|---|---|
| HC:HS-2 / HC:HS-3 / HC:HS-4 | Version and feature selection at connect | NONE |
| HC:HS-7 | The Unix socket is 0600 and the peer uid is checked | NONE |
| HC:HS-8 / HC:HS-9 | Typed close; **a control disconnect keeps routes open** | NONE |
| HC:HS-5 | A new incarnation drops cursors and keeps routes | NONE |
| HC:EN-3 / HC:EN-3b | One response per request; `hub_shutdown` means the effect is unknown | NONE |
| HC:EN-7 | A `committed` write survives kill -9 | NONE (s04 covers sessions only, and not via `committed`) |
| HC:EN-7b | `operation_id` resolves after kill -9 | NONE |
| HC:EN-8 | An idempotent retry returns the first result | NONE |
| HC:WT-1 to HC:WT-4 | Watch baseline, deltas, atomic batches, resync (lists update without polling) | NONE (T-S1 uses the session row incidentally) |
| HC:DS-3 / HC:DS-6 | Screens appear on plugin load; a stale descriptor dispatches nothing | NONE |
| HC:§6 ids / HP:SS-5 | Hub-minted 128-bit ids, never reused, not caller-chosen | s18 (XFAIL) |
| HC:§6 create | Spawn is accepted with a durable operation, and the row is visible at `starting` | NONE (s02 checks only `running`) |
| HC:§6 restart / HP:SA-2 / HP:SA-4 | Restart keeps the id, only when restartable, with typed refusals | s05 (contradicts SA-4) |
| HC:§6 delete / HC:A.2c | Delete only `ended` or `lost` | NONE |
| HC:§6 package create | Local-path package admission | s14 |
| HC:BP-1 | **The direct byte path: with the Hub SIGSTOPped, typing still echoes** | NONE |
| HC:BP-2 | Output bytes are identical to the PTY stream | NONE |
| HC:BP-4 | WebRTC route bound to the client fingerprint; a forged offer is refused | NONE |
| HC:BP-5 | Detach and revocation close the route at the worker | T-S2 (partial: detach only, via the old event) |
| HC:BP-6 | Latency and throughput relative to a reference relay | NONE (bench) |
| HC:BP-7 / HC:TS-11 | Routes survive a Hub restart; `list(route)` and `delete(route)` after reconnect | NONE |
| HC:TS-1 | Multi-attach and `replace_route` | NONE |
| HC:TS-2 / HC:TS-4 / HC:TS-4b | Connect once; the baseline is a state cut; `live` is the input-ready point | T-S1, T-S2 (partial) |
| HC:TS-4b (not_ready) | Input before `live` is `rejected{not_ready}` | NONE |
| HC:TS-4c | Resync on the same route | NONE |
| HC:TS-5 / HC:TS-5b | Per-op results in receive order, with exact units | T-S1 (partial) |
| HC:TS-5c | No blind resend; a confirmed paste is a new op | T-S3 |
| HC:TS-5d | Paste: one op, contiguous, `too_large` with zero effect | T-S3 (small only); large-paste stub |
| HC:TS-5e | **The unsafe-paste rule** (client classifies, `require_bracketed`, consent, resend) | T-S3 |
| HC:TS-6 | **Latest size** across several routes, with close fallback | T-S4 (partial: one route) |
| HC:TS-7 | A stalled reader gets a resync, then `stalled` | NONE (stubs) |
| HC:TS-8 | A healthy close drains the final tail, and `route_closed` is the last frame | NONE (T-S2 relies on `TerminalSubscriptionClosed`) |
| HC:TS-9 | `route_ended` event on transport loss | NONE |
| HC:TS-10 | Detach ends only that route | T-S2 |
| HC:P9 (output) | Transparency to clients, with one actor per side effect | NONE |
| HC:P9 (queries) | **Client-first queries** with shadow fallback; exactly once | NONE |
| HC:SF-0 to HC:SF-4 | **Plugin views**: bound kinds, actions, field errors, secrets, options | NONE |
| HC:SF-5 | Notices from event streams | NONE |
| HC:EV-1 to HC:EV-4 | **Client event streams**, priority drop, `event_gap` | NONE |
| HC:TR-5 / HC:PR-0 to HC:PR-7b | **Pairing** by URL fragment; untrusted signalling; DTLS bound | NONE |
| HC:PR-8 | Revocation closes live connections and routes | NONE |
| HC:MC-0 | MCP revision negotiation, Origin, protocol header | NONE |
| HC:MC-1 | A bearer token proves the caller | s06, s11 |
| HC:MC-1b | A plugin grant bounds an agent's call | NONE |
| HC:MC-2 | Native tool list (no message tools) | s06 (contradicts) |
| HC:MC-3 | **Messaging at-least-once across kill -9**, via the plugin | s10 (FAIL, wrong layer), s07 (wrong layer) |
| HC:MC-4 | **The safe_write doorbell** | s08, s09, s09b (all need change), s08b (stub) |
| HC:MC-5 | Two error layers | s15 (loose) |
| HC:MC-6 | Audit never logs the token or bodies | NONE |

### Hub→plugin

| Clause | Behavior | Covered by |
|---|---|---|
| HP:LD-1 | Load; a failed load leaves nothing and keeps diagnostics | s14 (success path only) |
| HP:LD-3 | A failed reload keeps the old instance serving | NONE |
| HP:LD-4 / HP:LD-6 | Unload drains and removes tools | NONE |
| HP:MT-2 | A tool-name collision fails the load | NONE |
| HP:MT-3 | tools/list reflects loaded plugins | s14 (partial) |
| HP:MT-4 | A schema-invalid call is refused before plugin code runs | NONE |
| HP:MT-5 / HP:ER-5 | Error shape and `effect` | s15 (loose) |
| HP:IV-3 / HP:IV-4 | Deadline → `timed_out`; a full queue → `backpressured` | NONE |
| HP:CP-4 / HP:CP-7 | Own-session scope versus `:any`; the caller never widens a grant | NONE |
| HP:VC-1 / HP:VC-2 | Verified caller; forged arguments are data | s07 (partial) |
| HP:VC-3 | The token survives a Hub restart; a restart issues a new token | s04, s16 (survival only) |
| HP:ST-9 | fsync before ack; survives kill -9 | s10 (indirect, wrong layer) |
| HP:CA-12 | Timers re-armed after a restart | NONE |
| HP:CA-14 / HP:CA-15 / HP:CA-16 | Plugin spawn: reserved ref, row visible, completion notification | NONE |
| HP:SW-1 / HP:SW-2 | `queued` at once; one completion with receipt fields | NONE |
| HP:SW-3 | Gate: hidden cursor, alt screen, client quiet, output settle, margin | s08 (hidden cursor only) |
| HP:SW-4 | Probe, echo, erase, residue, late cleanup | s09, s09b, s08 (need change) |
| HP:SW-5 | Plugin crash cancels; a Hub restart gives `unknown{hub_restarted}` | NONE |
| HP:SW-6 | A newer write replaces the pending one | NONE |
| HP:SW-7 | Sanitizing; `submit` adds CR | NONE |
| HP:SW-8 | Cancel before the probe is clean; after it, residue is reported | NONE |
| HP:RW-1 to HP:RW-3 | Raw write needs `session_write`; bytes unmodified; one arbitration | NONE |
| HP:SA-1 | Plugin stop, delete, update, semantic `read_screen` | NONE |
| HP:SO-1 to HP:SO-3 | Session observe reads and events | NONE |
| HP:EV-9 / HP:EV-14 | Hub and terminal events reach a plugin; opt-in output tap | NONE |
| HP:EV-11 to HP:EV-13 / HP:EV-15 | Priority, exact loss markers, gap handler | NONE |
| HP:CR-2 / HP:CR-3 | Crash → `plugin_crashed` in flight; restart once, then quarantine | NONE |
| HP:CR-5 | A plugin is re-adopted after a Hub kill -9 | NONE |
| HP:CR-6 | Orphan grace | NONE |
| HP:CR-8 | One plugin's crash loop does not starve another | NONE |
| HP:TR-1 / HP:TR-2 | Host allowlist; a hostile plugin is contained | NONE |
| HP:VW-1 to HP:VW-7 / HP:EN-9 | **Declarative views; the messaging inbox fixture**; operator-only shared projection | NONE |

Counts: **103 clause rows.** Covered: 16 (9 cleanly; 7 only by scenarios that contradict the clause, assert it loosely, or run on the wrong layer). Partial: 13. NONE: **74**.

---

## Items to ADD or CHANGE (for the foundation writer)

The CHANGE items come first, because they unblock the rest.

**CHANGE**

1. **Harness: minted ids only.** Delete the `--session-id` fallback in `Hub.spawn_sink`. When no `session_id=` is returned, fail with a pointer to s18. Proves HC:§6 ids and HP:SS-5 (`ss_5_caller_cannot_choose_id`) for every scenario.
2. **Harness: a messaging-plugin fixture.** Install the real messaging plugin package in each Hub, and give the harness an `Mcp.msg_*` wrapper over its `<package>__<tool>` names. Proves HC:MC-2 and HC:MC-3, HP:9.6 and HP:ST-9.
3. **s04, s07, s10, s11, s12, s16:** move from the native `post_message`, `receive_messages` and `ack_message` to the plugin tools (item 2).
   - s10 stays the red case: an unacked message is redelivered after kill -9 (HC:MC-3 `unacked_survives_kill9`, HP:ST-9 `survives_kill9`).
   - s11's isolation half proves HP:VC-4 and HP:VW-7 (`plugin_policy_refuses_other_session_data`).
   - s04 also renames `hub.sessions.list` to `list_sessions` (HC:MC-2).
4. **s06:** assert the native tool set is exactly HC:MC-2's (`whoami`, `list_sessions`, `read_screen`, `spawn_session`, `stop_session`, plus plugin `<package>__*`), with no Hub message or `notify` tool. Keep the 401 checks (HC:MC-1, HP:VC-3).
5. **s05:** spawn through a session type, so the session is restartable. After restart, assert: the same id; a new token; the old token 401 (HP:SA-2, HP:SA-4, HP:VC-3 `old_token_refused_after_restart`, HC:§6 restart). Add a negative: restart of a freeform spawn gives `wrong_state{not_restartable}` (HP:SA-4).
6. **s08:** use an **echoing** sink and ring through the messaging plugin (which calls `sessions.safe_write`). Assert:
   - the text arrives exactly once;
   - no `zx` remains;
   - nothing is typed while the cursor is hidden;
   - the receipt shows `residue: none`.

   Proves HC:MC-4, HP:SW-3 (`waits_for_visible_cursor`), HP:SW-4 (`probe_echo_erase`) and HP:SW-7.
7. **s09:** ring through the messaging plugin (or a fixture plugin granted `session_safe_write:any`) into an echoing sink. Assert `delivered` and the text typed (HP:SW-1, HP:SW-2, HP:SW-4 `echoing_sink_receives_text`).
8. **s09b:** use a `raw -echo` sink, so the probe is observable. Assert:
   - the text never arrives;
   - `zx` **may** be present;
   - the receipt is `abandoned{no_echo}`, `probe_written: true`, `residue: possible`, `text_written_bytes: 0`.

   Proves HP:SW-4 (`non_echoing_sink_receives_nothing`, `never_echoing_session_reports_residue_possible`) and HP:SW-2.
9. **s08b:** implement it with a route client that types while a ring is pending. Assert `abandoned{human_input}`, the human bytes intact, and no erase of human input (HP:SW-4 `human_edge_abandons_never_erases_human_input`, C:IN-10, HP:RW-3).
10. **s14:** make these assertions:
    - the exact tool name is `<package>__<tool>` (HC:MC-2);
    - admission is via `create(package, {source: {local_path}})` (HC:§6);
    - after disable, the tool leaves tools/list (HP:MT-3, HP:LD-6).
11. **s15:** require `result.isError: true` with a section-11 `Error` (including `effect`) in `structuredContent`. A JSON-RPC `error` is a failure here (HC:MC-5, HP:MT-5, HP:ER-5).
12. **s03:** assert `state == ended` and `exit{code, signal}` (HC:A.2c, C:EV-4), not `!= running`.
13. **TUI live tests T-S1 to T-S4:**
    - spawn without a `session_id` and read the minted id (HP:SS-5);
    - drop the `live_attach_occupancy` / subscription / generation / stream_epoch oracle (HC:P3);
    - on detach, expect `route_closed{detached}` on the route, not `TerminalSubscriptionClosed` (HC:TS-8);
    - T-S2 asserts a **new `route_id`** (HC:TS-11).
14. **s13 and tui-slow-reader:** merge them into one two-route check. Route X stops reading; route Y keeps echoing; X gets a resync, then `stalled` (HC:TS-7, C:OU-3, C:OU-6).
15. **run.sh:** either add `legs/d-*.sh`, or remove `d` from the default `--legs`. Today a default run always records a SKIP.

**ADD**

16. **Direct byte path with the Hub stopped:** attach, `SIGSTOP` the Hub pid, type, see the echo, then `SIGCONT` (HC:BP-1 `bp_1_hub_stopped_bytes_still_flow`, C:DP-1, C:TH-3). Unix first; WebRTC with the Web leg.
17. **Routes survive a Hub restart:**
    - with a route attached, close the control connection, then kill -9 the Hub: bytes keep flowing;
    - restart and reconnect as the same caller: `list(route)` shows the route;
    - `delete(route)` gives `route_closed{detached}`.

    Proves HC:BP-7, HC:HS-9 (`control_disconnect_keeps_route_open`), HC:HS-5 and C:DP-8.
18. **Terminal transparency:** the program emits OSC 52 write, OSC 8, OSC 9, OSC 777, BEL, OSC 0/2, OSC 7, OSC 133, and mode changes. The concatenated client `output` equals the PTY bytes (HC:P9, HC:BP-2 `output_bytes_identical_to_pty`, C:EV-7, C:OU-12).
19. **Client-first queries:**
    - DA1 and CPR with an attached client that answers: the program gets the client's reply;
    - with no route: the shadow answers;
    - the client declines: the shadow answers;
    - a late answer is `rejected{already_replied}`;
    - one reply only.

    Proves HC:P9 queries (`qr_*`) and C:EV-8.
20. **Latest size across routes:** two routes with different sizes; the most recently active wins; closing it falls back to the other; a resize answers `applied` or `superseded` (HC:TS-6, C:DP-6, HC:TS-4d).
21. **Multi-attach and replace:** two routes on one session; `replace_route` ends the old one `replaced`; detaching one leaves the other (HC:TS-1, HC:TS-10).
22. **Final tail on exit:** the session prints N bytes and exits. The client receives all of them, then `route_closed{session_ended{exit}}` as the last frame (HC:TS-8, C:OU-7, C:EV-4).
23. **Transport loss:** kill the client's socket. The owner's control connection gets `event{route_ended{transport_lost}}` (HC:TS-9); re-attach gives a new route (HC:TS-11).
24. **Large paste** (replaces the tui-large-paste stub):
    - a paste over 64 frames arrives once and contiguously;
    - a paste over `max_paste_bytes` is `rejected{too_large}` with zero effect.

    Proves HC:TS-5d, HC:BP-3 and HC:TR-3.
25. **Input before `live`** is `rejected{not_ready}`, `effect: none` (HC:TS-4b).
26. **Same data dir:** a second `start` on the same data dir is refused, and the first Hub's sessions are unaffected (C:LC-2, C:AD-5).
27. **Durable writes and retry:**
    - `create(session)` with an `operation_id`, then kill -9 right after the `res`;
    - after restart, `get(operation)` resolves it (and answers `busy`, never `not_found`, during recovery);
    - a repeat with the same idempotency key returns the first result.

    Proves HC:EN-7, HC:EN-7b and HC:EN-8.
28. **Watches:** a scripted client watches `session`. Spawn and stop give upsert and patch; a forced overflow gives a resync baseline; a plugin load shows its kinds (HC:WT-1 to HC:WT-4, HC:DS-3).
29. **Pairing (Web leg):**
    - `issue_pairing`, then pair, then reconnect with `auth_response`;
    - a restart invalidates an unused link;
    - revoking a credential closes its live control connection and routes;
    - signalling never carries the secret.

    Proves HC:PR-0 to HC:PR-8 and HC:TR-5.
30. **Web route driver (web-*-check.sh):** attach over the worker-terminated WebRTC route, echo, paste, and refuse a forged route offer (HC:BP-4, HC:TR-2, HC:TR-3).
31. **Doorbell lifecycle:**
    - a plugin crash cancels its pending write (`cancelled{plugin_unloaded}`);
    - a newer write replaces a pending one;
    - a cancel before the probe is clean, and a cancel after it reports residue;
    - after a Hub kill -9, `operations.get` gives `unknown{hub_restarted}`.

    Proves HP:SW-5, HP:SW-6, HP:SW-8 and HP:CA-16.
32. **The other doorbell gates:** alternate screen, a client typing within the quiet window, output not settled, and the near-margin cursor each delay the ring (HP:SW-3).
33. **Raw write:** a plugin without `session_write` is refused. With it, bytes arrive unmodified and unguarded (HP:RW-1 to HP:RW-3, HC:MC-4 `raw_write_needs_stronger_permission`).
34. **Plugin lifecycle:**
    - an entrypoint error fails the load, leaves nothing registered, and keeps its logs (HP:LD-1);
    - a failed reload keeps the old instance serving (HP:LD-3);
    - a crash gives in-flight `plugin_crashed`, one restart, then quarantine on a second crash (HP:CR-2, HP:CR-3);
    - after a Hub kill -9 the plugin is re-adopted and its tool still answers (HP:CR-5);
    - one plugin's crash loop does not affect another (HP:CR-8).
35. **Plugin call rules:**
    - a tool-name collision fails the second load (HP:MT-2);
    - a schema-invalid call runs no plugin code (HP:MT-4);
    - a slow handler gives `timed_out` (HP:IV-3);
    - an agent calling a plugin tool on a session outside the plugin's scope is refused (HP:CP-7, HC:MC-1b).
36. **Events:**
    - `hub/session.*` and `terminal/title`, `terminal/bell` and `terminal/exit` reach a subscribed plugin (HP:EV-9, HP:EV-14);
    - a client event stream gets the same events, with `event_gap` under overflow (HC:EV-1, HC:EV-2);
    - a package notice reaches a client (HC:SF-5).
37. **Plugin views: the messaging inbox fixture of HP:VW-7.**
    - A post through another plugin appears in the view.
    - An ack through a UI action removes the row.
    - An unrelated delta reaches every operator watcher.
    - A session caller cannot subscribe.
    - A stale descriptor dispatches nothing.

    Proves HP:VW-1 to HP:VW-7, HP:EN-9, HC:SF-0 to HC:SF-2 and HC:DS-6.
38. **Plugin spawn:** `session_types.spawn` returns `{operation_id, session, state: starting}`; the row is visible at once; the completion notification arrives; a failed start keeps the row until delete (HP:CA-14 to HP:CA-16).
39. **Session actions:**
    - plugin `stop`, `delete` (ended only), `update` (own metadata namespace only) and semantic `read_screen` (HP:SA-1);
    - client `delete` of a running session is `wrong_state` (HC:A.2c);
    - `read_screen` after exit succeeds (C:ST-5).
40. **Observe:** `terminal_state` and `read_cursor` shapes; `hub/session.silent` fires once per idle period (HP:SO-1 to HP:SO-3, C:TM-4).
41. **MCP:**
    - an unsupported MCP revision is answered with the Hub's revision (HC:MC-0);
    - a bad `Origin` is refused (HC:MC-0);
    - the audit log has no token or argument bodies (HC:MC-6).
42. **Lost worker:** kill a session worker. The session shows `ended{worker_lost}` or `lost`, with typed reasons; its routes end; re-attach is refused with `attach_failed` (C:AD-2, HC:A.2c, HC:TS-11). This brings TUI T-S12 into the suite.
43. **Semantic keys:** arrows in application cursor mode, page keys and a kitty-flag key reach the PTY as C:5.1A defines (C:IN-9, HC:A.5). This brings TUI T-S9 into the suite.
44. **A hostile plugin and the host allowlist:** an unlisted host is refused `host_not_trusted`; a fixture plugin's escape attempts all fail with no effect (HP:TR-1, HP:TR-2).
45. **Later:** adopting N−1 workers across a Core upgrade (C:AD-4), and the BP-6 benchmarks (HC:BP-6, C:DP-10). Neither blocks the cutover gate unless the orchestrator says so.
46. **Contract gap to raise, not a scenario:** s12 shows a real user expectation, that the Hub stays responsive and memory-bounded while a session floods output. No clause states it. Propose one to the Core and Hub pairs (control-plane latency and a Hub RSS bound under output flood), or drop the RSS assertion.

---

## Addendum: recount and trim (orchestrator scope correction, 2026-09-30)

**The rule.** A user-visible clause is **covered** when either of these runs against the **real** implementation:
- a conformance test (Core, Hub→client or Hub→plugin), or
- an acceptance scenario.

**cutover-e2e keeps only end-to-end user journeys that no single conformance suite can prove.** Scenarios that contradict the contracts are fixed or deleted. The suite does not grow to one scenario per clause.

### Recount

- **Coverage by conformance.** Every user-visible clause in the three contracts names at least one `conf::` test id; the exceptions are the scope statements (for example PR-9, TB-0's note and the implementation notes). Once each conformance suite runs on the real implementation, **those clauses count as covered**. At that point gate 4 is decided by three checks:
  - (a) each suite runs on the real side, and the Hub→client suite runs on **every** binding (TB-0);
  - (b) the seven journeys below exist and pass;
  - (c) no scenario contradicts a contract.
- **Today:** the conformance suites do not exist yet (design only), and 20 scenarios contradict the contracts. **Gate 4 stays FAIL.** This is expected before implementation, and the pass criterion is now crisp.

### The trimmed foundation-writer list

**A. Fix or delete contradicting scenarios** (items 1–15 of the list above):

| Item | Action | Reason |
|---|---|---|
| 1 | keep | the harness uses minted ids only |
| 2, 3 | keep | a messaging-plugin fixture; s04, s07, s10, s11, s12 and s16 move to the plugin tools. s10 (an unacked message across `kill -9`) stays: it is the known red case |
| 4 | keep | s06 asserts the native tool set of HC:MC-2 (now including `restart_session`, `update_session` and `delete_session`) |
| 5, 11, 12 | fix | s05, s15 and s03 are cheap corrections. Delete any of them instead if its conformance test covers it |
| 6–9 | keep | these are the doorbell journey (below) |
| 10 | move | s14 moves to conformance |
| 13 | keep | the TUI T-S tests use the new route names |
| 14 | move | s13 and slow-reader move to conformance (HC:TS-7) |
| 15 | keep | run.sh: no default leg `d` without scripts |

**B. The seven journeys** (the only new acceptance scenarios):

| # | Journey | From the list above | Proves (examples) |
|---|---|---|---|
| J1 | Attach and type **on each binding**: `local-stream` from the TUI, and `webrtc` from the Web client or a TUI over `webrtc` | 30, 13 | HC:TB-0, BP-1, BP-4, TS-4, TS-5 |
| J2 | **Hub restart with bytes flowing**: `SIGSTOP` the Hub and see the echo; restart the Hub; the route survives; `list(route)` and `delete(route)` work as the same caller | 16, 17 | HC:BP-1, BP-7, HS-5, HS-9 |
| J3 | **Pairing**: issue a link, pair a browser, revoke it, and see a forged answer fail | 29 | HC:PR-0 to PR-9 |
| J4 | **The messaging doorbell to an agent mid-turn**: an echoing sink and a non-echoing sink, the human race, and the late-echo cleanup | 6–9, 31 | HC:MC-3, MC-4; HP:SW-1 to SW-4 |
| J5 | **Telegram**: long poll plus send through the plugin connection primitives | new | the plugin connection primitives (user requirement) |
| J6 | **Plugin crash and reload**: the Hub stays up, subscribers see gaps, state recovers | 34 (a subset) | HP lifecycle and EV-12/13 |
| J7 | **The workspaces UI**: views over the workspaces plugin kinds in Web and TUI | 37, adjusted | HC section 7b; the ui-contract view vocabulary |

**C. Moved to conformance** (not the foundation writer's job; each belongs to the named suite): items 10, 14, 18–28, 32, 33, 35, 36, 38–44. **Later:** item 45. **Raised as a contract gap:** item 46, "the Hub stays responsive and memory-bounded while a session floods output". It goes to the Core and Hub→plugin pairs as a candidate clause, not as a scenario.


## Final journey text (2026-09-30, after final gate 4; supersedes rows J1 to J7 above)

These are the seven cutover-e2e journeys against the final contracts (the joint manifest final3). A journey's text is normative for the foundation writer.

- **J1 Attach and type on each binding.** `local-stream` from the TUI; `webrtc` from the Web client and from a TUI over `webrtc`. Setup for `webrtc`: install and enable the web plugin fixture, approve its `web_origin` and `relay_signaling` grants, and pair (J3). Type and see the echo; no success frame arrives for keys or text. A paste with `want_result: true` gets `input_done`; a paste over `max_paste_bytes` gets `input_refused{too_large}`. A forged route offer is refused. Detach gives `route_closed{detached}`; a re-attach gives a new `route_id`. Proves HC TB-0, BP-1, BP-4, TR-2, TR-3, TS-4, TS-5, TS-5b, TS-5d, TS-8, TS-11; HP WP-2 to WP-4.
- **J2:** unchanged; optional legs: `hub restart`, and `swap_preflight` then `swap` with a route attached, waiting with `--ready-fd` (HS-0).
- **J3 Pairing through the web plugin.** With no web origin, `issue_pairing` is refused `wrong_state{no_web_origin}`. Enable the web plugin fixture, set the origin (`web.set_origin`) and the ICE servers (`web.set_ice_servers`). Issue a link: its URL uses that origin. Pair a browser: signalling goes to `<web origin>/hubs/<hub_id>/signal` (through the plugin relay on an external origin; answered by the Hub itself on a served origin), and the plugin never sees a secret, a key, a signature or route bytes. Reconnect with `auth_response`. A Hub restart invalidates an unused link. `delete(client_credential)` closes the live connections and routes of that credential. A forged answer fails authentication. Proves HC PR-0 to PR-8, TR-5; HP WP-1 to WP-4, WP-6.
- **J4:** unchanged; add SW-5, SW-6, SW-8 and CA-16 to Proves; agents reach the messaging plugin through `<plugin_key>__<tool>` tools, never a Hub message tool.
- **J5 Telegram.** Install the plugin, approve its `net` allowlist and its secret, enable it. It long-polls `getUpdates` with `{{secret:bot_token}}` as an async `http.request` (answer `queued`, completion on `operations.on`), then sends a message. Unload it during a pending poll: the request is cancelled, nothing reaches the dead instance, and the secret appears in no log, event or receipt. Proves HP NT-2, NT-3, NT-5, NT-8, NT-9, NT-10, CA-16, PM-8.
- **J6 Plugin crash and reload.** Install from a `local` source, `approve_grants`, enable. Crash the plugin: in-flight calls fail `plugin_crashed`; its pending safe writes end `cancelled{plugin_unloaded}`; its network work is cancelled; it restarts once, and a second crash in `restart_window` quarantines it (a `quarantine` row; `invoke(quarantine, resolve)`). The Hub stays up and another plugin is not affected. Lost event copies are not replayed; a consumer of the crashed producer keeps its subscription; a session-family subscriber gets a fresh baseline; state comes back from the plugin store. `invoke(package, reload)` with a failing candidate keeps the old instance serving. After a Hub kill -9 the plugin is re-adopted and its tool answers. `plugin_log` shows a `restarted` marker. Proves HP LD-1, LD-3, LD-5, CR-2, CR-3, CR-5, CR-7, CR-8, EV-4, EV-6, EV-7, SW-5, NT-8, PM-5, PM-8.
- **J7 Plugin views in Web and TUI.** Views of the workspaces plugin and the HP VW-7 messaging inbox fixture. A post appears in the view; an ack through a UI action removes the row; an unrelated delta reaches every watcher (one shared projection). A caller without the client permission is refused `permission_denied`; an agent that holds it sees the same projection. A stale descriptor dispatches nothing. A navigation with a `select` preset opens the target view with the slot set, and the Web URL restores it. An `embed` part renders sandboxed in Web and as an "open in browser" line in the TUI. Proves HP VW-1 to VW-7, EN-9; HC SF-0 to SF-2, DS-6; ui-contract UI-SL-5, UI-SL-6, UI-EM-3, UI-EM-6.

**Scenario items that must change** (for the foundation writer's list A; they contradict the final contracts):

| Item | Change it to |
|---|---|
| 4 and the s04 note | the MC-2 verb tools (`describe`, `list`, `get`, `create`, `update`, `delete`, `invoke`, `whoami`); there is no `list_sessions` tool |
| 10 | `create(package, {source: {kind: "local", path}})`, then `approve_grants` (HP PM-8) |
| 19 | `input_refused{reason: already_replied}` or `query_expired` |
| 20 | no resize answer; the size arrives in `modes` |
| 24 | `input_refused{op, reason: too_large}` |
| 25 and the row at line 142 | `input_refused{reason: not_ready}` |

**Coverage note:** no journey covers notification bubbling (HC SF-5d) or the Hub swap. The conformance ids exist, so this is not a gate failure. J2's optional swap leg covers the swap.
