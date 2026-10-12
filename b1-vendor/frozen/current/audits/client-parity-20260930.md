# Client and Hub-builtin parity audit: v1 contracts against the monorepo's non-plugin features (2026-09-30)

Auditor: plugin parity auditor (Opus). Read-only: no code edit, no build, no test run, no commit. This file continues `plugin-parity-20260930.md`. Gap ids continue at CG-1, user decisions at UC-1, and gate-3 rows at PAR-28.

## 0. Scope, sources and method

**Scope.** Every user-visible feature of the legacy monorepo `~/Rails/trybotster` at `e572e581` that is not a plugin:
- the Web app: `app/frontend`, plus the Rails pages it depends on;
- the TUI: `cli/src/clients/tui`, `cli/lua/ui`;
- behavior built into the Hub: `cli/src/{channel,worker,hub,session,agent,notifications,relay,socket,…}`, `cli/lua/{handlers,lib,hub}`;
- the `botster` CLI.

**Contracts read:**
- `HC:` Hub→client, `hub-client-contract-v1-frozen-20260930.md`. It is the FINAL text. It pins codec rev 14, Core v1.5, HP A4r29 and ui-contract frozen.
- `RC:` route codec, `route-codec-v1-extract-rev17.md` (newest on disk; HC pins rev 14).
- `Core:` `core-contract-v1.5.md`.
- `UI:` `ui-contract-v1-frozen-20260930.md`.
- `HP:` `hub-plugin-contract-v1.md` (A4r29).

**Evidence.** Five read-only inventory passes wrote file:line for every item:

| File | Ids | What |
|---|---|---|
| `audits/client-parity-work/web-terminal.md` | WT-1 to WT-90 | Web terminal and session interaction |
| `audits/client-parity-work/web-shell.md` | WS-1 to WS-90 | Web app shell, settings, cloud pages |
| `audits/client-parity-work/tui.md` | TU-1 to TU-139 | TUI |
| `audits/client-parity-work/hub-builtins.md` | HB-1 to HB-418 | Hub-builtin behavior (sections A to E) |
| `audits/client-parity-work/cli.md` | CL-1 to CL-134 | CLI binary, config, device auth |

The rows below cite those ids. The id carries the file:line in the evidence file. Direct source citations are given where a row depends on one fact.

**Verdicts:** COVERED, PARTIAL, GAP and USER, as in the plugin audit.
- NOT-A-FEATURE marks legacy code that has no user-visible effect today (dead, unreachable, or a defect). It is listed so nobody carries it forward by accident (section 6).
- CLIENT marks behavior that lives entirely in a client renderer. No contract clause is needed, but the TUI and Web rebuilds must keep it (section 5 lists these for the client writers).

**Local usage checks** (this machine, 2026-09-30):
- `~/.botster-dev/agents/` holds 9 agent definitions: claude, codex, cursor, default, fable, grok, omp, opus, sol. They launch their program with `"$(botster context prompt)"` and read `botster context worktree_path|branch_name|session_dir` (`~/.botster-dev/agents/codex/initialization:17-28`, `~/.botster-dev/agents/opus/initialization:16`).
- Repo accessories with a `port_forward` marker exist, for example hyperflex `bin/dev`.
- `~/.botster{,-dev}/lua/user/ui/` is empty. No `improvements/` directory exists, and no plugin declares `-- @tui`. User Lua customization is therefore **unused** here.
- `~/.botster-dev/pastes/` exists (the file paste fallback directory).

## 1. Summary

- **Rows mapped: 98.**
  - COVERED 34.
  - PARTIAL 26.
  - GAP 21.
  - USER 14 (several are now ruled, see section 4.0).
  - NOT-A-FEATURE 3.
  - CLIENT-only requirements are listed separately in section 5.
- **Fifteen open gaps** (CG-1 to CG-20; five numbers folded) and **six user decisions** (UC-1 to UC-6; UC-2 is largely settled by the rulings in section 4.0).
- **Most important:**
  1. **CG-1 file and image paste.** The orchestrator has already sent it to 003c. The CONTENT_FILE flow is `<worktree>/.botster/pastes/<uuid>/botster-paste-<hash>.<ext>`, then the text `"<path> "` is typed into the PTY. No v1 frame exists for it.
  2. **CG-3 session context for the session program.** Every agent definition here runs `"$(botster context prompt)"`. v1 (HC MC-1c) gives a session only `BOTSTER_SESSION_ID`, `BOTSTER_MCP_URL`, `BOTSTER_MCP_TOKEN` and the port variable. HC `session_type.context_keys` has no delivery rule. As written, **no agent starts with its prompt, and scripts lose `worktree_path` and `branch_name`.**
  3. **CG-2 / UC-1 Hub-side attention and web push.** Today the Hub turns bell, OSC 9 and OSC 777 into a per-session attention flag on every client's session list. It sends web push with a deep link and app badge, even when no browser is open, and suppresses notices while someone is focused. HC P9 makes the attached client the only actor, and the Hub "does not also act on Core's semantic events". So a session that nobody is watching can no longer notify anyone. This conflicts with a user ruling (P9), so it goes to the user.
  4. **CG-6 baseline fidelity.** The legacy snapshot restores palette changes, cursor style, OSC 8 links, kitty images and kitty/mouse modes. The v1 TS-4 promises only "screen, modes and size", and Core ST-6 does not list those contents.
  5. **CG-9 / UC-2 cloud scope.** GitHub sign-in, the hub directory with online status, device-code hub registration, hub rename and delete, the TURN configuration, the web-app origin and the signalling relay are all trybotster.com features today. No v1 contract covers them.
  6. **CG-20 CLI surface.** No contract specifies the `botster` binary: `start`, `attach`, `status`, `mcp-serve`, `context`, `update`, `reset`, `get-connection-url`, `json-*`, worktree commands and `debug`. Several agent scripts depend on `botster context`.

## 2. Parity map by area

### 2.1 Terminal byte path, rendering and input (Web WT, TUI TU, Hub HB-A)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-1 | Output passthrough to the client renderer (Restty ghostty-vt in Web, libghostty in the TUI): colors, cursor, wide characters, box drawing, DEC 2026 synchronized output | WT-1..WT-9, WT-19; TU-64 | HC:P9, BP-1, BP-2 | COVERED | |
| C-2 | Kitty graphics images in output | WT-10 | HC:P9, BP-2 (bytes unmodified) | COVERED live | Restoring them on attach: CG-6. |
| C-3 | OSC 8 hyperlinks clickable (Web) | WT-11 | HC:P9 | COVERED live | Restoring them on attach: CG-6. |
| C-4 | OSC 52 clipboard write to the local clipboard; read suppressed | WT-14 | HC:P9 (clipboard read is a query: TS-QR, `clipboard_read`) | COVERED | v1 lets the client answer or decline a clipboard read (a change of policy, per a user ruling). |
| C-5 | Scrollback (10 MB), search, selection, copy, wheel and touch scrolling, overlay scrollbar | WT-12,13,15-18; TU-65,70-72 | HC:TS-4 `history: full`; Core ST-6 | COVERED | Search and selection are client-side (CLIENT). |
| C-6 | Key encoding (legacy xterm, kitty protocol with release), IME, mobile soft keyboard and extra-keys bar | WT-21..26; TU-34,37,84 | HC:TS-5; RC:A.5 (Core 5.1A `key`, `text`, `bytes`) | COVERED | |
| C-7 | Mouse reporting (all modes and encodings), pointer capture | WT-27; TU-77..81 | RC:A.5 `mouse`; A.6 ModeFlags | COVERED | |
| C-8 | Focus in/out: client focus telemetry; `ESC[I`/`ESC[O` written; synthetic focus-out when a focused client disconnects | WT-29,30; TU-85..87; HB-51..53 | RC:A.5 `focus`; Core IN-9 (focus reported only when `focus_reporting`) | PARTIAL | CG-17: nothing writes focus-out when the focused route ends. The legacy write of focus sequences without the mode is a defect (section 6). |
| C-9 | Text paste, bracketed when mode 2004 is on; no confirmation | WT-31,32; TU-74,75 | HC:TS-5d, TS-5e (unsafe-paste confirm is new) | COVERED | v1 adds unsafe-paste confirmation (a user-accepted change). |
| C-10 | **File and image paste and drag-and-drop:** CONTENT_FILE 0x03 and chunked 0x04 over the WebRTC channel. The Hub writes `<worktree>/.botster/pastes/<uuid>/botster-paste-<hash>.<ext>` (falling back to the data dir, then TMPDIR), types `"<path> "` into the PTY, and cleans up on session end. The client enforces a 50 MB limit; the Hub has none. | WT-33..41; HB-1..14; `cli/src/worker/session_io.rs:521-633` | none | GAP | **CG-1** (already sent to 003c by the orchestrator; the details are in section 3). |
| C-11 | Terminal queries answered with the viewing client's colors (`terminal_color_profile`); the active peer's profile wins; headless sessions use the Hub's boot-probed TTY colors | WT-6,7,43; TU-90..93; HB-49,50,54..56 | HC:P9 query arbitration (asked route, then shadow fallback), RC:A.4 `terminal_query`, A.5 `reply_terminal_query` | PARTIAL | Client answers are COVERED (better than today). The shadow fallback with no client attached answers from the worker's default palette, not the user's theme: CG-18. |
| C-12 | Snapshot on attach (gzipped GHOSTSNP) restoring screen, modes, kitty flags, mouse modes, palette, cursor style, links, images; early-output buffer; `request_snapshot` | WT-47..49; TU-66..68; HB-114,115 | HC:TS-4, TS-4c; Core ST-6 | PARTIAL | CG-6 (the contents of the state cut). |
| C-13 | Output gzip over 4 KiB on the WebRTC channel | WT-90; HB-384 | none (RC frames are raw) | PARTIAL | CG-7 (throughput over TURN relay). |
| C-14 | Multiple viewers, all can type; resize last-writer-wins | WT-46,55; HB-116,118,119 | HC:TS-6 `Latest`, TS-5 | COVERED | `Latest` replaces last-writer-wins (ruling). |
| C-15 | Per-client attach state (reconnecting, pending, not_ready, not_found); exited session detection (numeric exit is permanent) | WT-51,52,54; TU-69,99; HB-102,103,106 | HC:TS-8, TS-9, TS-11; session `state`, `exit` | COVERED | v1 fixes the Web "Connecting… forever" on exit (section 6). |
| C-16 | Human typing defers agent message injection; focus sequences exempt | WT-58; HB-53 | HP:SW-3 (client-quiet window), SW-4 | COVERED | |
| C-17 | OSC title shown in the session row (not the tab); cwd tracked but not shown | WT-60,61; TU-95; HB-43,44,97 | HC session row: no `title` or `cwd`; HP:EV-14 | PARTIAL | G-21 (row `title`, already sent) covers it. cwd is not shown today, so it is not needed. |
| C-18 | Input while disconnected | WT-42 (drawn locally only, a defect) | HC:TS-5 (queue until `live`) | COVERED | An improvement. |
| C-19 | Terminal color scheme query `CSI ?996 n` answered from background luma | HB-50 | HC:P9 (`color_scheme` reply) | COVERED | |

### 2.2 Notifications and attention (WT-62..71, TU-96..98, HB-17..42)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-20 | Bell, OSC 9 and OSC 777 detected **by the Hub** for every session, attached or not | HB-17,18,21 | Core EV-7 (the host sees them); HC:P9, EV-6 ("clients get these from the raw route output; the Hub does not also act on Core's semantic events") | USER | **UC-1** / CG-2. |
| C-21 | Default policy: emit only when the text contains "permission" or "phase"; plugin claims can suppress or replace (PP) | HB-22..26; WT-65 | none | GAP | CG-2 (Hub policy); plugin policy is G-12 of the plugin audit. |
| C-22 | Focus-aware suppression: no notice while any client has the session focused | HB-27; WT-65 | none | GAP | CG-2. |
| C-23 | Per-session "needs attention" flag on every client's list (amber border, TUI yellow bar, nav aggregate); cleared by `clear_notification` when a client views the session | HB-28,31; WT-56,59,66; TU-61,98; WS-51,65,89 | none (no row field) | GAP | CG-2. |
| C-24 | Web push from the Hub (VAPID keys and subscriptions stored encrypted on the Hub; Declarative Web Push; deep-link URL to the session or plugin route; `app_badge` count; TTL 24 h) | HB-34..42; WT-67,68 | none | GAP | CG-2. |
| C-25 | Push settings card: enable, subscribe this browser, test, disable, VAPID rotation, stable browser id | WT-69; WS-80,81; HB-36,37 | none | GAP | CG-2. The legacy `vapid_key_req` sends the private key to the browser; do not keep that (section 6). |
| C-26 | Enabling push PATCHes `notifications_enabled` to trybotster.com | HB-38 | none | USER | UC-2 (cloud). |
| C-27 | TUI OS notification (OSC 777 + OSC 9 written to the outer terminal) | TU-96 (never fires today: TU-97) | HC:P9 (the client acts on the raw sequence) | COVERED | It never works today, so this is not a regression. v1 makes it work for attached sessions. |
| C-28 | GitHub issue comment when an issue-linked agent asks a question (cloud) | WS-82; plugin GH-9 | none | USER | UC-2 / plugin U-4. |

### 2.3 Sessions: spawn, definitions, lifecycle (HB-B, HB-C, WS-54..61, TU-108..117)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-29 | Agent and accessory definitions in device `~/.botster[-dev]` and repo `<target>/.botster[-dev]` layers (repo wins); each is an `initialization` script typed as `source <file>`; built-in `terminal` accessory | HB-65..67,70; HB-208 | HC `session_type` kind (source device or repo, `command`, `args`, create/update/delete, `resolve`, `definition`) | PARTIAL | Layered definitions COVERED. A shell-script body needs `command: bash, args: [<script>]`, and the script must be editable: CG-11. |
| C-30 | **Initial prompt and session context delivered to the program:** env `BOTSTER_PROMPT`; `botster context prompt\|worktree_path\|branch_name\|session_dir\|repo\|…` reads the session manifest. **Every local agent definition uses it.** | HB-71,74; CL-60..65, CL-111; `~/.botster-dev/agents/codex/initialization:17-28` | HC:MC-1c (exactly `BOTSTER_SESSION_ID`, `BOTSTER_MCP_URL`, `BOTSTER_MCP_TOKEN`, port); `session_type.context_keys`; `create{context:{prompt}}` | GAP | **CG-3.** |
| C-31 | Session environment: `TERM=xterm-ghostty` plus TERMINFO (the Hub installs the terminfo), COLORTERM passthrough, LANG default, exe dir prepended to PATH | HB-73,74; CL-30 | HC:MC-1c | PARTIAL | CG-3 (TERM and TERMINFO, and PATH to the `botster` CLI). The legacy Hub scrubs no env; v1 scrubbing is intended (G-118 of gate 3). |
| C-32 | Spawn agent: target (admitted, enabled), then main checkout / existing worktree / new worktree (issue number → `botster-issue-N`), agent choice (auto when only one), workspace, prompt | WS-55..58; TU-109..111; HB-77..81,156..166 | HC:`create(session,{session_type_id,target_id,context})`, `worktree.create{target_id, branch}` | PARTIAL | A spawn inside an existing worktree: G-27. `base_ref` and reuse: G-3. Workspace: the workspaces plugin (see C-44). |
| C-33 | Spawn accessory (with `port_forward` → `$PORT` from 46000-61999) | WS-57; TU-112; HB-57,84 | HC session create; HP:CA-20 (A4r25) | PARTIAL | G-19 (`PORT` name, already sent). |
| C-34 | Lifecycle stage text while spawning (creating_worktree, spawning_ptys) | TU-106; HB-82,160 | HC `operation.progress?`, session `state: starting` | COVERED | |
| C-35 | Close: keep or delete the worktree, with `close_actions` (can_delete_worktree, other_active_sessions) | WS-61; TU-30,117; HB-99,101,173,175 | HC: session `stop` + `delete`, `worktree.delete` | PARTIAL | Needs the row's worktree reference and occupancy: G-21 plus CG-5. |
| C-36 | Exited session kept with status "exited"; `auto_close_on_exit` metadata | HB-102 | HC: `ended` kept until delete; `session_type.lifecycle?` | COVERED | |
| C-37 | Restart a session | none today (HB-113) | HC:`restart`; HP:SA-2 | COVERED | New. |
| C-38 | Session survival across a Hub restart (session processes; 120 s reconnect lease; manifest and degraded recovery; orphaned marking; same MCP token re-issued) | HB-86..89,107..111; CL-131 | Core adoption (AD-*), BP-7; HP:VC-3 | COVERED | |
| C-39 | Session row: label > display_name > id; title line (title + task); subtext target · branch · agent; accessory dimming | WS-50; TU-104,105; HB-92,93 | HC `session` core fields | PARTIAL | G-21 (title, branch, owner) plus CG-4 (task). |
| C-40 | Output-activity indicator (green border, TUI `✺`): 3 s window | WT-64; TU-104; HB-98 | none on the client row (HP has `hub/session.activity` for plugins) | GAP | CG-4. |
| C-41 | `update_session` (label, task, metadata, plugin_state, owner_plugin, visibility, surface) | HB-92,193 | HC:`update(session)` labels; HP:SA-1 metadata | PARTIAL | `task` and `visibility`: CG-4 and plugin G-18. |
| C-42 | System and hidden sessions kept out of client lists | HB-64 | none | GAP | Plugin G-18 (already sent). |
| C-43 | Selecting a session in the Web sends `select_agent` | WT-57; WS-47; HB-96 | – | NOT-A-FEATURE | The Hub answer is a no-op ack (HB-96); the TUI does not follow. |

### 2.4 Workspaces, worktrees, spawn targets, templates (HB-C, WS-48..78, TU-101..119)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-44 | Workspaces: grouping in both clients, collapse, rename, move a session, auto-create by name, reopen a closed same-name workspace, closes when the last session closes | WS-48,49,58..60; TU-101,103,115,116; HB-135..148 | UI section 14 (botster-workspaces plugin, a conformance vector); HP entities and views | COVERED | Workspaces become a plugin. The shipped v1 set must include it (plugin audit P-4). Spawning into a workspace from the spawn dialog needs a workspaces-plugin action that spawns, or G-1. |
| C-45 | Worktree creation rules: `<base>/<repo>-<branch>` under `~/botster-sessions`, force-remove of an existing dir, `base_ref` only for a new branch, `base_target_path` override | HB-154..164 | HC:`worktree.create{target_id, branch}` | PARTIAL | G-3 (base_ref, reuse). Path naming is Hub policy. The force-remove is a defect (section 6). |
| C-46 | Worktree deletion runs `git worktree remove --force` plus `git branch -D` | HB-174 | HC:`worktree.delete` | COVERED | Whether the branch is deleted must be stated; plugin G-3(b) proposes `delete_branch?`. |
| C-47 | Worktree list: pull-only today (not pushed on create or delete) | HB-169 | HC `worktree` watch | COVERED | An improvement. |
| C-48 | Spawn target admin: add by absolute path with host directory autocomplete (`fs:browse`), rename, remove, status and git chips | WS-62..64; TU-32,114; HB-133,168,191,211 | HC `spawn_target` kind (create, update, delete, validate) | PARTIAL | Host directory browse: CG-11. Git chips: G-24. |
| C-49 | Template catalog (GitHub contents API): install, uninstall and repair agent, accessory and plugin templates per device or repo scope; quick setup; `plugin:load` after install | WS-68,74,78; HB-178..183 | HC: registry family removed (orchestrator ruling); local package admission only | USER | **UC-3.** The catalog also installs agent and accessory definitions, not only plugins. |
| C-50 | Workspace templates auto-spawn accessories with an agent | HB-149,150 | none | NOT-A-FEATURE (unused) | No workspace template exists on this machine (`~/.botster-dev/workspaces` is the session store). |
| C-51 | Web config editor: device or repo scope; edit, create, rename and delete files under `.botster` (agent and accessory scripts, `port_forward` marker, workspaces, plugin files) through `fs:*`; add or remove agent and accessory dirs | WS-73..77; HB-210 | HC `session_type` create/update/`definition` | PARTIAL | CG-11 (editing a definition's script body; the `port_forward` marker maps to `allocate_port`). |

### 2.5 Connection, pairing, reconnect (WS-1..19, TU-1..15, HB-E)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-52 | Local socket control connection (hello v2) and TUI attach | TU-8; HB-262..266; CL-42..46 | HC:TR-1, TB-L2..L4, HS-1..9 | COVERED | |
| C-53 | TUI reconnect after a Hub restart (10 × 1 s); panels re-subscribe | TU-9,10; CL-48 | HC:HS-5, BP-7 (routes survive), WT-7 | COVERED | Better: terminals keep flowing. |
| C-54 | WebRTC to the Hub with signalling over the cloud ActionCable relay; DataChannel mux | WS-1,3; HB-349,354 | HC:TR-2, TB-W2, PR-4 (signalling at `<origin>/hubs/<hub_id>/signal`) | PARTIAL | Transport COVERED. Who hosts signalling: UC-2. |
| C-55 | ICE/TURN servers from the cloud (`GET /hubs/:id/webrtc`, Metered/coturn, 60 s cache); relay forced for 5 min after direct-path churn; ICE restart ×3 | WS-2,13,14; HB-375..378 | none | GAP | **CG-8.** |
| C-56 | Pairing: v7 bundle in the URL fragment, QR and link (Share dialog), TOFU hub key, Ed25519 browser key in IndexedDB, DTLS-bound challenge, `pair_request`/`auth_response`, one-time secret regenerated after use | WS-5..10; HB-356..372; TU-119,121 | HC:PR-0..PR-9 | COVERED | Cold cut to protocol 1. |
| C-57 | Credential revocation (Hub function only, no UI) | HB-373 | HC:PR-8 `delete(client_credential)` | COVERED | New UI capability. |
| C-58 | Connection overlays and status dots (Direct vs Relay), hub online/offline from the cloud health stream | WS-11,16..18; WT-85,86 | HC: no transport-quality field; cloud online status | PARTIAL | Direct vs Relay is client-derived (CLIENT). Online status: UC-2. |
| C-59 | Hub recovery-state banner (`hub` entity state starting → ready) | WS-19; WT-87; HB-112,267,413; TU-11 | HC `hub` row has no lifecycle state | GAP | CG-10. |
| C-60 | Per-tab browser identity; same-credential offer replaces a stale channel | WS-4; HB-371,374 | HC:PR-8 (caller = credential) | COVERED | |
| C-61 | `dc_ping` health probe, peer grace, send retry | WS-12; WT-50; HB-389 | HC:TB-W4 (binding progress and close) | COVERED | The probe policy is client-side. |

### 2.6 Plugin UI rendering in clients (WS-27..45,65..67; TU-127..130)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-62 | Plugin surfaces rendered from UiNode trees; dynamic routes with `:param`, subpaths, fullscreen layout, 404 vs loading | WS-27,37..45 | HC:7b, SF-0..SF-5; UI: all | PARTIAL | Parameterized routes: plugin G-13 (sent). |
| C-63 | Plugin nav with attention bells | WS-65 | HC `navigation` | PARTIAL | Plugin G-15 (badges). |
| C-64 | Home `workspace_panel` with the built-in `botster.sessions` widget and plugin widgets | WS-33; HB-221,222 | none | GAP | Plugin G-15 (home placement). |
| C-65 | Plugin sidebar mode | WS-32 | HC `navigation` | PARTIAL | Plugin G-15 (placement) covers it if placement includes `sidebar`. |
| C-66 | Plugin iframe (http(s) or plugin asset) with a postMessage bridge | WS-66; WT-84 | none | USER | Plugin U-5. |
| C-67 | Action result `navigate` follow-up | WS-41,42 | UI:UI-AC-1 (`{ok, field_errors?, message?}`) | GAP | Fold into plugin G-13: a result may carry a `navigate` target. |
| C-68 | Plugin-contributed session actions with status badges, URL buttons and error panels with an install link | WT-73..75; WS-52,53 | HP:VW-8 | PARTIAL | Plugin G-20 (sent). |
| C-69 | The TUI renders no plugin surfaces today | TU-118,127..130 | HC:7b | COVERED | New for the TUI. |

### 2.7 Hub management, update, MCP (HB-D, WS-69..72, TU-123)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-70 | Restart Hub (agents preserved) from Web and TUI | WS-71; TU-123,124; HB-202,283,291 | HC `hub` actions: `check_update`, `update`, `shutdown`, `issue_pairing` only | GAP | **CG-10.** |
| C-71 | Dev Rebuild (`cargo build` then exec-restart) | TU-123; HB-292 | none | USER | UC-5 (a developer tool). |
| C-72 | Update check and apply (GitHub releases latest; sha256 when present; `sudo mv` fallback); TUI boot prompt; `update --check` | HB-284..291; CL-23,25,112..117 | HC `hub.check_update`, `update` | COVERED | The update source and channel are Hub policy. |
| C-73 | Hub rename and delete (cloud API) | WS-70,72 | none | USER | UC-2. |
| C-74 | MCP over loopback HTTP, per-session bearer token persisted across restart, loopback Origin check, auth-failure lockout | HB-230..236 | HC:MC-0, MC-1; HP:VC-3 | COVERED | |
| C-75 | Per-session MCP tool scoping: plugin tools limited to the spawn target's and the agent manifest's `plugins` lists; enforced on list and call | HB-245 | none | GAP | **CG-14.** |
| C-76 | Built-in MCP prompts `botster-customize-tui`, `-hub`, `-create-plugin`, `-customize-mcp` | HB-247 | HC:MC-2 (tools only) | PARTIAL | CG-15: they document legacy Lua customization, which v1 removes. A v1 equivalent (plugin authoring guidance) needs a place. |
| C-77 | `botster mcp-serve` stdio→HTTP proxy for agents; outside a session only a `botster_status` tool | HB-238..240; CL-50..59 | HC:MC-1 (HTTP) | PARTIAL | CG-20 (the CLI). |
| C-78 | MCP over the Web DataChannel (tools list and call from the browser) | HB-408 | HC:MC (agents only) | NOT-A-FEATURE | No Web UI calls it (not in the WS inventory). |
| C-79 | Plugin management from clients: install, load, reload (Web settings); enable, disable and logs (MCP only) | WS-68; HB-204,285 | HC `package`, `plugin_log` kinds | COVERED | Agent access is plugin U-7. |
| C-80 | Offline mode (`--offline`: no auth, registration, pairing, push, update check) | HB-270,318; CL-16 | v1 is local-first; cloud is optional | COVERED | |
| C-81 | Preview TCP tunnel: a paired browser opens raw TCP to any `127.0.0.1:<port>` over the DataChannel (`CONTENT_STREAM` 0x02), with no allowlist | HB-59,60,393..396 | none; HP section 22 (forwarding deferred) | USER | **UC-4.** The Web UI that used it was removed (WT-83, commit 70d4261c), so nothing user-visible uses it today. |
| C-82 | Hub user Lua (`user.init`, `improvements/*.lua`), hooks and interceptors (`before_agent_create`, `before_pty_spawn`, `filter_agent_env`, `before_command`, …) | HB-185..189 | none (plugins are isolated services with no interceptors) | USER | **UC-6.** Unused on this machine. |
| C-83 | Plugin runtime primitives used only by plugins (`local_webhooks`, `http`, `websocket`, `timer`, `watch`, `fs`, secrets store) | HB-305..313 | HP section 9.8 and others | COVERED | See the plugin audit. |

### 2.8 CLI (CL, HB-D)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-84 | `start` (embedded TUI, auto-attach when running, `--headless`, `--offline`, singleton flock, device identity creation, boot update check, cwd normalized to `$HOME`) | CL-13..33; TU-1,2,5 | none | GAP | CG-20. |
| C-85 | `attach` (TTY, one live hub discovered from manifests, no keyring, 10 × 1 s reconnect, detach-on-quit) | CL-38..49; TU-3,4,12 | HC:TR-1 (transport only) | GAP | CG-20 (discovery and CLI). |
| C-86 | `status` probe and diagnosis lines | CL-34..37; HB-271 | none | GAP | CG-20. |
| C-87 | `context [key]` / `context file <rel>` from the session manifest | CL-60..65; HB-277 | none | GAP | CG-3 and CG-20 (every agent script uses it). |
| C-88 | `get-connection-url --hub` (reads `connection_url.txt`) | CL-121,122; HB-275 | HC:PR-0 (`issue_pairing`) | PARTIAL | The CLI must call `issue_pairing` (CG-20). |
| C-89 | `reset` (cloud DELETE, keyring, config dir) | CL-118..120; HB-276 | none | GAP | CG-20 and UC-2 (the cloud part). |
| C-90 | `json-get/set/delete`, `list-worktrees`, `delete-worktree N` | CL-126..130; HB-273,274 | HC `worktree` kind (for the worktree commands) | GAP | CG-20. The `json-*` commands are generic file tools (keep as CLIENT). |
| C-91 | `debug memory`, `debug vt-replay` | CL-123..125 | none | PARTIAL | Developer tools. `debug memory` has no v1 wire (HC P3: "diagnostics are one opaque value"); CG-20. |
| C-92 | Device login (device-code flow with trybotster.com, keyring credentials, browser open, env-token handling) | CL-66..88; HB-320..333 | none | USER | UC-2. |
| C-93 | Config and data roots (`~/Library/Application Support/botster{-dev}`, `~/.botster{-dev}`), `config.json` keys, env overrides, `APP_NAME` dev/release coexistence | CL-89..110; HB-303,314..318 | none | PARTIAL | CG-20 (document the v1 roots). Dev/release coexistence matters (the user runs both). |
| C-94 | Logs: per-process log files, rotation, `BOTSTER_LOG_FILE` | CL-6..12; HB-297..301 | HC `plugin_log` (plugins only) | COVERED | Hub logging is an implementation matter, not a contract. |

### 2.9 Cloud features (trybotster.com; WS-20..25,69..72,82..87; HB-E)

| # | Feature | Evidence | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| C-95 | Sign in with GitHub (GitHub App OAuth), account page, docs site | WS-83,85,86 | none | USER | UC-2. |
| C-96 | Hub directory `/hubs.json` with online status, live refresh (`HubListChannel`), switcher, last-used hub, post-approval polling | WS-20..25 | none | USER | UC-2. |
| C-97 | Device-code hub connect (RFC 8628) and registration `POST /hubs`, heartbeat, `alive:false` on shutdown | WS-84; HB-323,335..340 | none | USER | UC-2. |
| C-98 | GitHub webhook commands into the Hub (`create_agent` for mentions, `agent_cleanup`) over HubCommandChannel with sequence acks | HB-229,351..353 | plugin GH-3, GH-7, GH-8 | USER | Plugin U-4 / UC-2. |

The Web client features that stay within the client are summarized in section 5. They include layout, responsive and mobile behavior, the PWA and service worker, the entity stores, the viewport classes and the test readiness markers.

## 3. Gap list (smallest contract addition; owner)

| # | Gap | Rows | Smallest addition | Owner |
|---|---|---|---|---|
| **CG-1** | **File and image paste** (already sent by the orchestrator). | C-10 | Route input `{op, kind: "file_paste_begin", name, total_bytes, mime?}`, then `file_chunk` frames (a new client type, e.g. `0x85` `[u64 op][bytes]`), then `file_paste_commit`/`abort`. The worker writes the file under a Hub-policy directory (default `<session cwd or worktree>/.botster/pastes/<uuid>/botster-paste-<hash>.<ext>`, safe name, extension sanitized). It then types `"<absolute path> "` as one text transaction. `input_result` carries `{path}` (today the browser gets no reply). Bound: `route_limits.max_file_paste_bytes`. Files are removed at session run end, and the tracking survives a Hub restart (today it leaks). Feature name `file_paste`; available on every binding (today it is Web-only). | RC and HC (003c); Core (0019) for the worker file write |
| **CG-2** | **Hub-side attention and push** (depends on UC-1). | C-20..C-25 | (a) The session row gains `attention?: {since, source: bell\|osc9\|osc777, title?, body?}`, set by the Hub from Core EV-7 events under a policy: default match "permission\|phase", plugin session-type filter (plugin G-12), and suppression while any route of the session has focus. It is cleared by `invoke(session, clear_attention)` or by client input on a focused route. (b) A `push_subscription` kind (create, delete, per browser) and `hub` row `push: {vapid_public_key}`, with actions `push_test` and `rotate_vapid`. The Hub sends a web push when attention is set, with a deep link to the session or plugin route (plugin G-13) and `app_badge` = the count of attention rows. (c) HC P9/EV-6 wording: the in-client notice stays the client's; attention state and push are Hub side effects of a different kind. | HC (003c); requires UC-1 |
| **CG-3** | **Session context for the session program.** Needed for the prompt, worktree path, branch, target, repo, session dir, TERM/TERMINFO, and PATH to `botster`. | C-30, C-31, C-87 | HC MC-1c adds `BOTSTER_CONTEXT_FILE`: a 0600 JSON file the Hub writes before start, holding every `context_keys` value plus `prompt`, `worktree_path`, `branch`, `target_id`, `repo`. Alternatively, one env var per declared context key `BOTSTER_CTX_<KEY>`. HC also names `TERM`/`TERMINFO` and puts the `botster` CLI on PATH. HP `session_types.spawn{context}` gets the same schema. CG-20 keeps `botster context <key>` reading it. | HC (003c) + HP (0016) |
| **CG-4** | **Session row fields clients render:** output activity, task. | C-39..C-41 | The row gains `activity: active\|idle` (Hub-derived from Core output timing, the same signal as `hub/session.activity`) and `task?` (written by `update(session)`). Title, branch and owner come from plugin G-21. | HC (003c) |
| **CG-5** | **Worktree occupancy** for the close dialog. | C-35 | The `worktree` row gains `active_sessions: n`, or `sessions: [SessionRef]`. With G-21 `worktree_id` on the session row, a client computes `can_delete_worktree` and `other_active_sessions`. | HC (003c) |
| **CG-6** | **Baseline fidelity.** | C-2, C-3, C-12 | TS-4 state equivalence and Core ST-6 name the restored state: cell attributes, palette changes (OSC 4/10/11/12), cursor shape and blink (DECSCUSR), OSC 8 hyperlinks, kitty graphics placements, kitty keyboard flags, mouse modes, bracketed paste. Or ST-6 lists what is not restored, and the equivalence test excludes exactly that. | Core (0019) + HC TS-4 (003c) |
| **CG-7** | **Output compression** on routes (legacy gzip over 4 KiB matters over TURN). | C-13 | A route feature `output_deflate` negotiated at attach: per-frame deflate of `output` and baseline payloads above a size. Or state that compression is not in v1 (a stated regression). | RC (003c) |
| **CG-8** | **ICE/TURN configuration** for the `webrtc` binding. | C-55 | HC 9.2 states where ICE servers come from: returned by the signalling origin with the signalling session (`GET <origin>/hubs/<hub_id>/ice`, untrusted, like signalling). The Hub and the client get the same list. The relay fallback policy is client policy. | HC (003c); hosting depends on UC-2 |
| **CG-10** | **Hub restart and Hub lifecycle state.** | C-59, C-70 | `invoke(hub, restart)`: accepted operation, sessions preserved (adoption). The `hub` row gains `state: starting\|recovering\|ready`. | HC (003c) |
| **CG-11** | **Operator configuration editing.** | C-29, C-48, C-51 | (a) The `session_type` definition carries an optional `script` body (the Hub stores it as the definition's file and runs `command` with it), editable through `update`. (b) `invoke(spawn_target, browse, {path})`, operator-only: directories under a path, no file contents. (c) The `port_forward` marker becomes `session_type.allocate_port` (HP CA-20). | HC (003c) + HP CA-20 (0016) |
| **CG-14** | **Per-session MCP tool scoping.** | C-75 | `session_type.mcp_packages?: [package]` (and a spawn-target default). The Hub lists and allows only those packages' tools for that session's token; Hub-native tools are always visible. | HC MC (003c) |
| **CG-15** | **Agent guidance prompts.** | C-76 | Hub-native MCP prompts (MC-2 prompts list) for plugin authoring and customization, rewritten for v1. Or drop them explicitly. | HC MC (003c); minor |
| **CG-17** | **Focus-out when the focused route ends.** | C-8 | When a route that last reported `focused: true` ends, and no other route of the session is focused, the worker writes `Focus{false}` (encoded only if `focus_reporting`). | Core (0019) + HC TS-10 (003c); minor |
| **CG-18** | **Shadow answers with the user's theme** when no client is attached. | C-11 | The host supplies a default color profile to the worker (from Hub configuration, or from the last active client). The shadow fallback answers color queries from it. | Core (0019); minor |
| **CG-20** | **CLI surface.** No contract or companion spec defines the `botster` binary. | C-84..C-93 | A short companion spec `botster-cli-v1.md` over HC. Commands: `start`, `attach`, `status`, `mcp-serve`, `context`, `update`, `reset`, `pairing-url`, `worktree list/delete`, `json-*`, `debug`. It also covers config and data roots, dev/release coexistence (`APP_NAME`), hub discovery and the one-hub-per-device rule. | TUI writer (001b), reviewed by Sol Hub A (orchestrator ruling) |

Numbers CG-9, CG-12, CG-13, CG-16 and CG-19 were folded:
- CG-9 is cloud: UC-2.
- CG-12 is the catalog: UC-3.
- CG-13 is workspace templates: unused.
- CG-16 is user Lua: UC-6.
- CG-19 is spawn progress: covered by `operation.progress`.

## 4.0 Rulings received while this audit ran (2026-09-30, orchestrator via the plan auditor)

- **One Hub per machine (U-2).** The CLI's multi-hub discovery and `attach` conflict handling simplify to a single Hub (C-85, CG-20).
- **Cloud is a separate plugin product (U-4).** C-26, C-28, C-73, C-89 (cloud part), C-92, C-95 to C-98 become features of that plugin, not of the v1 contracts. UC-2 is settled except CG-8 (below).
- **The Web origin for pairing URLs and signalling is provided by a plugin** (a configured public host, or the plugin serving the Web app). The signalling is untrusted, and the plugin never sees route bytes or keys. **CG-8 stays open:** HC 9.2 must say that ICE/TURN servers come from that same plugin-provided origin, with the same trust rule.
- **Agents are clients (U-7).** MCP is a client binding over the same verbs, with an operator-restrictable scope set. **CG-14 becomes that scope set:** per-session or per-session-type restriction of which packages' kinds and actions the agent may use. **CG-3 still stands:** the program needs its prompt before it can call anything. C-77 and C-79 are COVERED under this ruling.
- **Sandboxed iframe plugin HTML is allowed (U-5).** C-66 is COVERED once the UI owner adds the part.

## 4.2 Final status (2026-09-30, client candidate parity16 d7d475e3…06f7; HP candidate A4r60; Core draft 69)

- **Ruled and applied:**
  - UC-1 (client SF-5d);
  - CG-1 (path plus a space, TS-5f);
  - CG-3 (`BOTSTER_CONTEXT_FILE` in MC-1c with the session facts; HP A4r60);
  - CG-6 (Core ST-6b, cited by HC TS-4);
  - CG-17 and CG-18 (Core draft 69);
  - UC-2, UC-4 and UC-6 (ruled).
- **Still with the user:** UC-3 (template catalog) and UC-5 (Dev Rebuild).
- **Not re-verified here** (the owners track them in their candidates): CG-4, CG-5, CG-7, CG-8, CG-10, CG-11, CG-14, CG-15, and CG-20 (001b).

## 4.1 Later rulings and owner replies (2026-09-30)

**Orchestrator:**
- CG-20 is owned by the TUI writer (001b), reviewed by Sol Hub A.
- **UC-4: DROP** the raw browser TCP tunnel. It is a forwarding connector, which the user deferred.
- **UC-6: DROP** user Lua inside the Hub. Plugins replace it.
- UC-2 is settled.
- UC-1, UC-3 and UC-5 remain with the user.

**Core writer (0019), in the Core working copy `core-contract-v1-draft.md` draft 69.** It is batched and **not yet published**; the current published candidate is v1.8.
- **CG-1:** a Core worker capability, already in the candidate as DP-5b and codec TS-5f (`file_begin`, `0x85 file_chunk`, `file_commit`/`file_abort`, `max_file_bytes`). The file is written into a host-supplied `file_directory`, with a sanitized, unique, exclusive name, and is deleted at session Remove at the latest.
  - **One open difference for the client pair (003c):** the worker pastes the absolute path as a Core Paste, bracketed when the mode is on, with **no trailing space and no quoting**. Legacy typed `"<path> "` with a trailing space, unbracketed.
  - Keeping the legacy behavior is a TS-5f change in both contracts. The trailing space matters because agents such as Claude Code treat a pasted image path as an attachment when it is followed by further text.
- **CG-6:** closed by the new ST-6b. It names the restored state: cells with attributes including OSC 8, palette and dynamic colors, cursor position, visibility, shape and blink, every ModeFlags field, size and alt screen, title and cwd. Kitty graphics are restored only with the feature `snapshot_graphics`. Exclusions are stated. The HC TS-4 wording is 003c's to align.
- **CG-17:** closed by the new DP-12.
- **CG-18:** closed by `SpawnRequest.color_profile` and `begin(SetColorProfile)`.

## 4. User decisions (for the orchestrator)

| # | Decision | Rows |
|---|---|---|
| **UC-1** | **Attention and push while nobody is watching.** Today the Hub itself sees bell, OSC 9 and OSC 777 on every session. It flags "needs attention" on every client's list, and sends web push with a deep link and app badge to your phone or browser even when no Botster tab is open. HC P9 (your transparency ruling) makes the attached client the only actor, and says the Hub does not act on these events. As written, a session nobody is watching can no longer notify you. Allow the Hub to keep an attention flag and send push (CG-2), in addition to the client's own in-terminal handling? | C-20..C-25 |
| **UC-2** | **(Largely settled: cloud is a separate plugin; the Web origin comes from a plugin. Remaining: CG-8.)** **trybotster.com in v1.** This covers GitHub sign-in, the hub directory with online status, device-code hub registration, hub rename and delete, the TURN servers, the signalling relay and the web-app origin that the pairing URL points to, plus the GitHub integration (plugin U-4). HC PR-4 assumes a signalling endpoint at the pairing URL's origin, but no contract says who runs it. | C-26, C-28, C-54, C-55, C-58, C-73, C-89, C-92, C-95..C-98 |
| **UC-3** | **Template catalog.** Removing the registry family also removes the GitHub-hosted catalog that installs agent definitions (codex, opus, …), accessories and plugins, and its "quick setup". Keep a catalog for definitions, or install them only by local path? | C-49 |
| **UC-4** | **Browser TCP tunnel to any localhost port** (`CONTENT_STREAM`). The Hub still serves it to any paired browser, with no port allowlist. No UI uses it since the preview UI was removed. Drop it in v1 (recommended; it is forwarding, which is deferred)? | C-81 |
| **UC-5** | **Dev Rebuild** (TUI menu: `cargo build` then exec-restart the Hub). Keep it as a developer-only feature? | C-71 |
| **UC-6** | **User Lua inside the Hub** (`user.init`, `improvements/`, interceptors that can veto or rewrite spawns and env). It is unused on this machine, and v1 has no equivalent. Drop it? | C-82 |

The plugin audit's U-1 to U-8 still stand.

## 5. Client-only requirements (no contract change; for the TUI and Web writers)

These have no contract dependency, but a rebuild that drops them is a regression. The ids point to the evidence files.

- **Web terminal:**
  - fonts and theme (WT-2..5);
  - search and selection, including touch (WT-12,13,15);
  - Shift overriding mouse mode (WT-12,17);
  - touch momentum scroll (WT-18);
  - output pacing (WT-19);
  - mobile keyboard layout and extra-keys bar (WT-24..26);
  - OSC 133 click-to-move (WT-28);
  - drop overlay, size limit and toast for file paste (WT-34..36; see CG-1 for the result).
- **Web shell:**
  - routes and deep links `/hubs/:id/sessions/:uuid` (WS-26,28,29);
  - responsive sidebar and mobile slide-over (WS-30);
  - PWA manifest and service worker (WS-35,36,81);
  - viewport classes (WS-44);
  - action lifecycle, pending state and timeout (WS-41,42);
  - entity stores, per hub (fix the global-store defect, WS shell note);
  - test readiness markers `data-cli-status`, `data-hub-snapshot`, `data-surface-ready` (WS-88).
- **TUI:**
  - key tables and modes (TU-16..39);
  - keymap, action and mouse Lua APIs for user customization (TU-40..45; unused here, the TUI writer decides);
  - layout DSL and fallback layout (TU-46..56);
  - multi-panel terminals (TU-57..59);
  - outer-terminal mode mirroring (DECCKM, bracketed paste, kitty, cursor) (TU-82..89);
  - color probe (TU-90);
  - the modal spawn flows (TU-108..117);
  - QR modal (TU-63,119);
  - hot reload in debug builds (TU-131..136).
- **Both clients:**
  - Direct vs Relay indicator (WS-17,18);
  - selection behavior (TU-36);
  - quit vs detach semantics (TU-12, CL-47).

## 6. Legacy behavior not to carry forward (defects and dead code; confirm before treating any as a requirement)

- **Web:**
  - Ctrl+D is intercepted as split-pane on non-Mac (WT-81), and Ctrl+F is shadowed (WT-80).
  - An exited session shows "Connecting…" forever (WT-53).
  - Attach errors unwire the terminal (WT-54).
  - Offline keystrokes are drawn but never sent (WT-42).
  - `transient_event` and `connection:stalled` have no consumer (WT-70,88).
  - The unreachable "session expired" state; the unused `stream_mux.js`, `http_codec.js`, `use_presence.js` and `ProofOfLife.jsx`.
- **TUI:**
  - Stuck on "Rebooting…" after Restart Hub (TU-11).
  - OS notifications never fire (TU-97).
  - Copy-link OSC 52 goes to the Hub's stdout (TU-122).
  - The rename and move inputs ignore typing (TU-29).
  - Collapse calls an undefined function (TU-24).
  - The embedded TUI never loads a snapshot (TU-15).
- **Hub:**
  - Paste files leak across a Hub restart (HB-12).
  - The per-session `notifications` flag is ignored (HB-20).
  - `vapid_key_req` sends the private push key to the browser (HB-37).
  - Focus sequences are written without focus-reporting mode (WT-29).
  - A worktree create force-removes an existing directory (HB-162).
  - `git branch -D` drops unmerged work (HB-174).
  - No env scrubbing (HB-75).
  - `hub.api_token()` ignores `BOTSTER_TOKEN` (HB-416), and `config.server_url()` ignores `config.json` (HB-417).
  - Dormant code: `kill_orphaned_processes`, `FRAME_SET_TIMEOUT`, the `preview` channel, session tee logging, `reliable.rs` (HB-91,134,392).
- **CLI:**
  - `config KEY VALUE` saves nothing (CL-106).
  - `delete-worktree` reports success on a missing path (CL-130).
  - A partial `config.json` silently resets to defaults (CL-99).
  - The first-run save drops Lua keys (CL-102).
  - The Hub Lua path ignores `-dev` (CL-96).
  - `poll_interval`, `agent_timeout` and `max_sessions` are never enforced (CL-104).

## 7. Requirements for gate 3 (PAR-28 to PAR-45)

| # | Requirement | Source | Satisfying clauses | Status | Note |
|---|---|---|---|---|---|
| PAR-28 | Pasting or dropping a file or image into a terminal stores it where the session can read it, types its path, reports the result, and works on every binding. | C-10; user report 2026-09-30 | none | COVERED IN CANDIDATES (Core DP-5b; codec TS-5f types path plus a space, per client parity16) | CG-1 |
| PAR-29 | A session program can read its prompt and context (worktree path, branch, target, repo) at start. Every agent definition depends on this. | C-30, C-87 | client:MC-1c; HC `context_keys` (no delivery) | COVERED IN CANDIDATES: HP A4r60 (spawn prompt? and context?) plus client MC-1c BOTSTER_CONTEXT_FILE with the session facts (session, session_type_id, target_id?, worktree_id?, worktree_path?, branch?, repo?, prompt?, context); the CLI `botster context` reader belongs to 001b (CG-20) | CG-3 |
| PAR-30 | A session nobody is watching can still raise attention on every client and send web push with a deep link. | C-20..25 | client:P9, EV-6 (exclude it) | RULED AND APPLIED (client SF-5d parity16: hub/session_notification to other clients, session.attention when no route is focused, clear_attention, web push to devices with no live connection) | UC-1, CG-2 |
| PAR-31 | Attaching restores the full terminal state: palette, cursor style, hyperlinks, images, keyboard and mouse modes. | C-12 | client:TS-4; core:ST-6 | COVERED IN CANDIDATES (Core ST-6b; HC TS-4 cites it) | CG-6 |
| PAR-32 | Session rows show output activity and task. | C-39, C-40 | client:§6 `session` | GAP | CG-4 (plus G-21) |
| PAR-33 | The close dialog knows whether the worktree can be deleted and who else uses it. | C-35 | client:`worktree` | PARTIAL | CG-5 (plus G-21) |
| PAR-34 | Route output can be compressed, or the loss is stated. | C-13 | codec A.4 | PARTIAL | CG-7 |
| PAR-35 | The `webrtc` binding defines the source of ICE/TURN servers. | C-55 | client:§9.2 | GAP | CG-8 |
| PAR-36 | Clients can restart the Hub with sessions preserved, and see the Hub's lifecycle state. | C-59, C-70 | client:`hub` kind | GAP | CG-10 |
| PAR-37 | Operators can edit a session type's script body and browse host directories when adding a spawn target. | C-29, C-48, C-51 | client:`session_type`, `spawn_target` | PARTIAL | CG-11 |
| PAR-38 | Plugin tools visible to a session can be limited per session type. | C-75 | client:MC | GAP | CG-14 |
| PAR-39 | Hub-native agent prompts: kept (rewritten for v1) or dropped. | C-76 | client:MC-2 | PARTIAL | CG-15 |
| PAR-40 | Focus-out is written when the focused route ends. | C-8 | core:IN-9; client:TS-10 | RESOLVED IN CORE DRAFT 69 (DP-12) | CG-17 |
| PAR-41 | Shadow color answers use the user's theme when no client is attached. | C-11 | core (host default profile) | RESOLVED IN CORE DRAFT 69 | CG-18 |
| PAR-42 | The `botster` CLI surface (start, attach, status, mcp-serve, context, update, reset, pairing URL, worktree, json, debug), config roots and dev/release coexistence are specified. | C-84..93 | none | GAP | CG-20 |
| PAR-43 | Cloud features (sign-in, hub directory, registration, TURN, signalling, web origin) are kept or explicitly dropped. | C-95..98 etc. | none | RULED | UC-2 |
| PAR-44 | The definition template catalog is kept or explicitly dropped. | C-49 | client: registry removed | USER | UC-3 |
| PAR-45 | Browser localhost tunnel, dev rebuild and Hub user Lua are each kept or explicitly dropped. | C-71, C-81, C-82 | – | PARTLY RULED | UC-4 and UC-6 DROPPED; UC-5 with the user |

## 8. Messages sent

- **003c (client and codec):** CG-1 (details for the orchestrator's earlier message), CG-2 to CG-8, CG-10, CG-11, CG-14, CG-15, CG-17 (HC part), and PAR-28 to PAR-45.
- **0019 (Core):** CG-1 (the worker file write), CG-6, CG-17 and CG-18.
- **0016 (plugin):** CG-3 (the HP `session_types.spawn{context}` schema) and CG-11(c).
- **0017 (UI):** C-67 (action result `navigate`, folded into plugin G-13).
- **Orchestrator:** UC-1 to UC-6, and CG-20, which has no owner.
