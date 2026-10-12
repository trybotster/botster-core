# Plugin parity audit: v1 contracts against every legacy monorepo plugin (2026-09-30)

Auditor: plugin parity auditor (Opus), session of 2026-09-30. Read-only: no code edit, no build, no test run, no commit.
Requirement (user, 2026-09-30): "We need to accommodate the features of every existing plugin in the monorepo. I want zero regressions."

## 0. Scope, sources and method

**Plugins audited** (the live dev Hub `list_plugins` shows 13 loaded instances):

| Tag | Plugin | Source audited | Status on the live Hub |
|---|---|---|---|
| PP | project-pipelines 1.1.0 | `~/.botster-dev/plugins/project-pipelines/` (20 Lua files; catalog copy byte-identical) | loaded |
| OR | orchestrator 2.4.0 | `~/.botster-dev/plugins/orchestrator/init.lua` (catalog 2.3.0 is a strict subset; `~/.botster` 2.3.0 has two extra argument aliases) | loaded |
| MS | messaging 1.2.0 | `~/.botster-dev/plugins/messaging/init.lua` (catalog 1.1.0 adds `notify_session`) | loaded |
| TG | telegram 1.1.1 | `~/.botster-dev/plugins/telegram/init.lua` (no other copy) | loaded |
| GH | github 3.1.1 | `~/.botster-dev/plugins/github/` (4 files; catalog differs only in `mcp_proxy.lua`; `~/.botster` 3.0.0 is older) | loaded |
| BB | botster-bugs 1.0.0 | `~/.botster-dev/plugins/botster-bugs/` | loaded |
| KI | knowledge-inbox-pipeline | `~/.botster-dev/plugins/knowledge-inbox-pipeline/init.lua` (dev only) | loaded |
| CF | cloudflare-hosted-preview | `~/.botster-dev/plugins/cloudflare-hosted-preview/init.lua` (catalog identical; `trybotster/.botster` copy older, not loaded by the dev Hub) | loaded |
| RW | rails-worktree-lifecycle, repo-scoped | `~/Rails/{trybotster,hyperflex,trybackster,jupiter,hearth}/.botster-dev/plugins/rails-worktree-lifecycle/init.lua` (4 distinct variants) | 5 instances loaded |
| HB | Hub-builtin MCP tools (not a plugin; agents use them) | `~/Rails/trybotster/cli/lua/hub/mcp_defaults.lua:888-995` | always |
| WL | worktree-lifecycle (catalog template) | `catalog/templates/plugins/worktree-lifecycle/`, `~/.botster/plugins/worktree-lifecycle/` | not loaded |
| DS | demo-surface (catalog template) | `catalog/templates/plugins/demo-surface/` | not loaded |

**Contracts read** (tags used in the parity columns):
- `HP:` = Hub→plugin, `hub-plugin-contract-v1.md` (FROZEN v1.1; the amendment log shows **A4r27** at read time; the file is still being amended).
- `HC:` = Hub→client, `hub-client-contract-v1-frozen-20260930.md`.
- `UI:` = `ui-contract-v1-draft.md` (draft 2, not frozen).
- Core is not cited: no plugin feature maps to Core directly.

**Evidence files.** Five read-only inventory passes wrote the full per-plugin feature inventories and host-API call-site lists, with file:line for every item:
- `audits/parity-work/project-pipelines.md` (PP: 62 tools, 12 routes, 21 tables, every call site)
- `audits/parity-work/orchestrator-messaging-telegram.md` (OR, MS, TG, HB)
- `audits/parity-work/github-bugs-knowledge.md` (GH, BB, KI)
- `audits/parity-work/cloudflare-worktree-demo.md` (CF, RW, WL, DS, repo-scoped loading)
- `audits/parity-work/legacy-host-api.md` (the whole legacy plugin host API, for reference)

This file condenses those inventories into a feature list per plugin (section 2). Each feature has a citation and a parity verdict. Sections 3 to 5 add the gap list, the user decisions and the gate-3 requirement rows. Citations are `path:line` relative to the plugin root unless a path is given. "HUB/" means `~/Rails/trybotster/cli/`.

**Verdicts.**
- COVERED: the plugin can be rewritten on v1 with the same user-visible behavior. Different helper names are acceptable.
- PARTIAL: part of the behavior is possible. The note says what is missing.
- GAP: no v1 clause makes the behavior possible.
- USER: a ruling excludes the behavior, or the choice is a product decision. Section 4 lists it for the user.

## 1. Summary

- **Features mapped: 116.** COVERED 51, PARTIAL 36, GAP 19, USER 10. Several PARTIAL rows become COVERED when one gap closes: 11 PARTIAL rows depend on G-1 or G-3.
- **22 open gaps.** They are numbered G-1 to G-27; five numbers were folded, see the note under section 3.
  - Owners:
    - 9 for the plugin pair only: G-1, G-2, G-3, G-4, G-6, G-7, G-8, G-25, G-26.
    - 1 for the UI owner only: G-16.
    - 2 for the client contract only: G-21, G-27.
    - 10 that span contracts: G-5, G-11, G-12, G-13, G-14, G-15, G-18, G-19, G-20, G-24.
  - **8 user decisions (U-1 to U-8).**
- **The six gaps with the widest reach:**
  1. **G-1 plugin-to-plugin calls.** Messaging is a plugin and the Hub has no inbox. So project-pipelines (4 durable `Hub:post` sites), botster-bugs, github (PR events with delivery feedback) and every spawner that places a session in a workspace have no way to reach another plugin, except at-most-once events.
  2. **G-2 command execution with captured output.** `run_command_gate` runs pipeline command gates and git steps. All rails-worktree-lifecycle work is shell. `sessions.create_command` makes a visible session with no output capture and no timeout.
  3. **G-3 worktree delete and a specified worktree request.** Plugins can list worktrees but cannot delete one. The `ensure_worktree_and_spawn` worktree request has no schema: branch, `base_ref`, reuse of an existing worktree.
  4. **G-5 repo-scoped plugins.** Five rails-worktree-lifecycle instances load from spawn-target repos. v1 has no target-scoped package concept and no rule for one package name in several repos.
  5. **G-6 filesystem read and watch.** knowledge-inbox-pipeline watches and reads the vault. `fs:<root>` is "designed, not built" in HP section 9.1.
  6. **G-13 deep links and G-15 home widgets and nav badges.** project-pipelines has 12 parameterized routes, push URLs to ticket pages, 2 home widgets and a nav badge.
- **Hosted preview (CF).** The CA-20, VW-8 and `session_update:any` amendments (A4r25 to A4r27) close most of it. Still open: G-19 (the `PORT` variable name), G-20 (dynamic action label, status and URL) and **U-1**: the plugin runs a public, unauthenticated Cloudflare quick tunnel to a session's dev-server port.

## 2. Per-plugin feature inventory and parity map

Every row: feature, citation, the v1 clauses that make it possible, the verdict, and a note. The full argument schemas and every call site are in the evidence files (section 0).

### 2.1 project-pipelines (PP)

**Host API used** (full call sites: `parity-work/project-pipelines.md` section 2):
- Globals:
  - `plugin.db` (`db:eval` ×19, `db:execute`, model `insert/update/where/delete`)
  - `mcp.tool` ×62 and `mcp.prompt` ×2 (`project_pipelines/mcp.lua:111,143`)
  - `events.on/off` (`init.lua:55-89`), `hooks.on` (`init.lua:98,105`), `timer.every/cancel` (`init.lua:117-123`)
  - `log.*`, `json.*`, `config.data_dir`
  - `spawn_targets.get/list` (`project_pipelines/repo.lua:117`, `project_pipelines/web/ui.lua:252`)
  - `fs.list_dir/exists` (`project_pipelines/worktree_cleanup.lua:99,234`)
  - `push.send` (`project_pipelines/notification_policy.lua:112`), `hub.run_command_gate` (`project_pipelines/engine.lua:101`)
  - `ui.*` DSL (24 functions), `os.time/getenv/clock`
- Hub modules:
  - `lib.hub`: `create_agent` ×4, `create_accessory`, `delete_agent` ×2, `post` ×4, `notify` ×7, `run_command_gate`, `list_owned_sessions`, `delete_worktree`, `entity_*`
  - `lib.agent` (`Agent.get` ×17, `session:info()`), `lib.config_resolver`, `lib.entity_broadcast`, `lib.surfaces`, `lib.dashboard`, `lib.action`, `lib.notifications`, `lib.workspace_store`, `lib.tree_snapshot`

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| PP-1 | 62 MCP tools (tickets, projects, pipelines, steps, gates, runs, checklists, questions, reviews, findings, artifacts, PR links, dependencies) with JSON schemas | `project_pipelines/mcp.lua:162-1268` | HP:MT-1, MT-2, MT-4, MT-5; HC:MC-2 (exposed as `project-pipelines__<tool>`) | COVERED | Tool names gain the package prefix. Prompts that name tools (`project_pipelines/engine.lua:1122,2046`) must be rewritten. |
| PP-2 | 2 MCP prompts (agent role, review role) | `project_pipelines/mcp.lua:144,154` | HP:MT-1 | COVERED | |
| PP-3 | Caller identity from `context.session_uuid` (submit_gate stamps, current_context inference, receive_question_answers filter) | `project_pipelines/mcp.lua:729,995,1135,1185` | HP:VC-1, VC-6, IV-1 | COVERED | |
| PP-4 | Durable domain state: sqlite, 21 tables, 36 indexes, schema v12, 4 migrations, transactions | `project_pipelines/db.lua:13-364`, `project_pipelines/repo.lua:215-219` | HP:ST-1 to ST-11 | COVERED | This is a rewrite from SQL to document collections. Transactions become `store.batch` with expected revisions. Unique indexes (pr_links provider+repo+number, dependency pairs) become id design, because ST has no unique constraint. |
| PP-5 | Ticket search (LIKE over title, description and id) | `project_pipelines/repo.lua:499-533` | HP:ST-10 | COVERED | ST-10 states the cost: the plugin scans in its own code. |
| PP-6 | Event log (78 kinds) that carries state (membership, merge agent, advisors) | `project_pipelines/repo.lua:467-483,1726-1918` | HP:ST-1, ST-4 | COVERED | |
| PP-7 | Load-time data rewrites (legacy prune, stack-delivery prompt refresh) | `init.lua:43-44`, `project_pipelines/stack_delivery.lua:152-188` | HP:HS-7 (reads only), LD-10, `Ready.timers` | PARTIAL | Writes are refused in bootstrap. The plugin must defer them to the first timer or request (a timer descriptor in `Ready` fires as a request). That is possible, but ordering is no longer "at load". |
| PP-8 | 30 s reconcile timer plus a full reconcile at load | `init.lua:113-126`, `project_pipelines/engine.lua:3177-3204` | HP:CA-12, `Ready.timers`, SS-1, CA-16 | COVERED | Re-arm on each fire. The reconcile uses `sessions.list{owner}` and operation records. |
| PP-9 | Spawn step agents, question advisor, merge/PR steward, manual assist, each with branch `project-pipelines/<ticket>`, `from_worktree` reuse, `base_ref`, `base_target_path`, workspace name, label, prompt, metadata | `project_pipelines/engine.lua:634-706,999,1149,1278,31-49,81-92` | HP:CA-14, CA-15, `session_types.ensure_worktree_and_spawn`, SA-1 (`update` for label, metadata) | PARTIAL | The worktree request has no schema (G-3). Workspace placement needs another plugin (G-1). The prompt maps to `context` (unspecified shape; see G-3). |
| PP-10 | Manual accessory spawn (terminal) in the ticket worktree | `project_pipelines/engine.lua:1240` | HP:`session_types.spawn` (accessory session type), CA-14 | PARTIAL | Needs a worktree reference as the spawn location (G-3). |
| PP-11 | Correlate spawns by request id through the `agent_created` and `agent_lifecycle` hooks; failed lifecycle blocks the run | `init.lua:98-110`, `project_pipelines/engine.lua:2839-3023` | HP:CA-14 (reserved `SessionRef` at accept), CA-16 (operation notifications), EV-6 | COVERED | Better than today: the reference comes back at accept. |
| PP-12 | Delete ticket sessions, with or without the worktree, on close | `project_pipelines/engine.lua:1669,1345` | HP:SA-1 (`stop` then `delete`), EV-6 | PARTIAL | A running session needs stop, then a wait for `ended`, then delete. Worktree deletion: GAP (G-3). |
| PP-13 | Sweep and delete ticket worktrees (`cleanup_worktrees`, skip occupied paths) | `project_pipelines/worktree_cleanup.lua:300-371,247-266` | HP:`worktrees.list/show` (read only) | GAP | G-3 (delete) and G-21 (the session row has no worktree reference, so "occupied" cannot be computed). |
| PP-14 | Read the on-disk workspace store (`<data_dir>/workspaces/*/sessions`) | `project_pipelines/worktree_cleanup.lua:96-137` | HP:`worktrees.list` | COVERED once G-3/G-21 land | Hub internals are replaced by worktree rows. |
| PP-15 | Command steps and command gates: user-defined shell command, cwd = target path, 600 s timeout, exit status, stdout/stderr tails, async completion event `command_gate_completed` | `project_pipelines/engine.lua:1742-1803,2724-2830`; HUB/`src/hub/server_comms/event_lua.rs:661` | HP:CA-19 `sessions.create_command`, EV-14 `terminal/exit` and the output tap | PARTIAL | G-2. `create_command` has no `target_id` for cwd, no timeout, and no captured output (the tap is lossy and low priority with no replay; `read_screen` is only the last screen). It creates a visible session per gate (G-18). |
| PP-16 | Git plumbing through commands: `.gitignore` restore, `git branch/status/pull/diff` state machine after merge | `project_pipelines/engine.lua:110-143,1528-1624,2740-2789` | as PP-15 | PARTIAL | G-2. |
| PP-17 | Durable task messages into agent inboxes (`Hub:post`, msg_id dedupe, `wait=true`) | `project_pipelines/engine.lua:528,909,2097,2145` | HP:section 9.6 (no Hub inbox), SW-1 | GAP | Messaging is a plugin. PP cannot call it (G-1). `safe_write` alone loses the inbox, dedupe and durability. |
| PP-18 | Attention lines into sessions with an MCP action hint (`Hub:notify`) | `project_pipelines/engine.lua:584,927,1051,1384,1431,2110,2159` | HP:SW-1 to SW-10 (`session_safe_write:any`) | COVERED | The plugin renders the hint text itself. |
| PP-19 | Consume GitHub PR events (`pr_merged`, `pr_review_submitted`, `pr_comment`) from another plugin | `init.lua:69-89`, `project_pipelines/github_integration.lua:34-120` | HP:EV-1, EV-2 (`events_plugin:github`), EV-4, EV-7 | PARTIAL | Events are at-most-once with no delivery count. Today the producer acks the cloud only when a subscriber received the event (GH-6). Durable hand-off needs G-1. |
| PP-20 | Notification policy claim: suppress terminal notifications of owned sessions unless they mention permission or approval | `project_pipelines/notification_policy.lua:90-98,73-88` | none (HC:EV-6: clients derive terminal notices from route output) | GAP | G-12. |
| PP-21 | Web push notifications (phase change, question), with URL and tag | `project_pipelines/notification_policy.lua:108-150` | none | GAP | G-11 (and G-13 for the URL target). |
| PP-22 | Web surface `pipelines`: nav entry (workspace, order 30), 12 routes with parameters (`/tickets/:ticket_id`, `/runs/:run_id`, `/projects/:project_id`, `/pipelines/:pipeline_id/edit`, …), sidebar surface | `project_pipelines/web/surface.lua:260-291` | HP:VW-1 to VW-5, EN-1 to EN-9; HC:section 7b, `navigation` kind; UI:sections 3-9 | PARTIAL | Screens map to views (UI appendix A covers home and settings). Parameterized routes and deep links: G-13. Sidebar: a second view plus navigation. |
| PP-23 | Home workbench panels (needs attention, questions, running, PRs and merge, projects, standalone tickets, pipeline definitions) | `project_pipelines/web/screens/home.lua:175-306` | UI:`list`, `table`, `detail` with `display: metric`, plugin-published kinds (UI Q4) | COVERED | Computed rows (attention_item) are plugin-published kinds. |
| PP-24 | Ticket page: questions with answer textarea, dependencies add and remove, merge result, PR links, pipeline start buttons, spawn-session dialogs, timeline, runs, agent terminals | `project_pipelines/web/screens/ticket.lua:42-811` | UI:`detail`, `list`, `form`, `options_source`, UI-AC-4 dialogs, UI-SL-3 `open` | PARTIAL | Everything except: open a ticket page by id (G-13), open a PR URL (G-16), and embedded terminals (PP-26). |
| PP-25 | Pipeline editor: per-field edits on pipeline, step (4 transition selects) and gate, with field errors | `project_pipelines/web/screens/pipelines.lua:111-365`, `project_pipelines/web/actions.lua:180-216` | UI:UI-FM-1 to UI-FM-7, UI-RC-3 (`options_source.where` with row), SF-2 | COVERED | Edit-on-change becomes form submit. No capability is lost. |
| PP-26 | Fullscreen session terminal inside the plugin (`/sessions/:uuid`, `/tickets/:id/sessions/:uuid`) and `ui.session_row` | `project_pipelines/web/surface.lua:26-29`, `project_pipelines/web/screens/ticket.lua:175,813-819` | UI:UI-SL-3 `open` (the client's own session screen), UI-RF-1 | COVERED | Different presentation (the client's own terminal screen, not an embedded one). The "back to ticket" return needs G-13. |
| PP-27 | 2 home-page dashboard widgets (needs attention, active runs) | `project_pipelines/web/surface.lua:204,231` | none | GAP | G-15. |
| PP-28 | Nav badge when any question is open | `project_pipelines/web/surface.lua:265` | none (the `navigation` row has no badge) | GAP | G-15. |
| PP-29 | 19 entity kinds; 4 in the default snapshot, 15 by detail query (per ticket or run) | `project_pipelines/entities.lua:1549-1562`, `project_pipelines/entity_contract.lua:12-32` | HP:EN-1 to EN-5, EN-9, ST-7; HC:WT-1, WT-2 (watch filters) | PARTIAL | v1 has one projection per family (EN-9: no parameterized watch). Every row of large families (`event`, `run_step`) must be projected. The Hub filters at watch time. Risk R-1. |
| PP-30 | Visibility filters (closed tickets hidden) | `project_pipelines/entities.lua:34-98` | HP:ST-7 allowlist, HC:WT-2 filter, UI `where` | COVERED | |
| PP-31 | Per-subscriber drafts and feedback, presentation state | `project_pipelines/web/actions.lua:14-75`, `project_pipelines/web/screens/ticket.lua:649-740` | UI:P5, UI-RZ-2d, UI-FM-7 | COVERED | |
| PP-32 | Session info reads (`worktree_path`, `branch_name`, `workspace`, `cwd`, `notification`, `label`) for badges and routing | `project_pipelines/engine.lua:81-92`, `project_pipelines/web/ui.lua:316-402` | HP:SS-1, SS-2, EV-14 (`terminal/notification`, `terminal/cwd`) | PARTIAL | G-21 (no worktree, branch or title on the row). "Needs attention" must be kept by the plugin from `terminal/notification` events. |
| PP-33 | Agent choices per target (`ConfigResolver.list_agents`) | `project_pipelines/mcp.lua:116-138` | HP:`session_types.list{target_id}` | COVERED | |
| PP-34 | Spawn-target lookups (target path for cwd) | `project_pipelines/repo.lua:117` | HP:CA-13 (sanitized rows, relative paths) | PARTIAL | A cwd is expressed as a target or worktree reference, which G-2 must accept. |
| PP-35 | Hot reload keeps sqlite, re-registers everything, tears down old timers and subscriptions | `init.lua:8-31,56-123` | HP:LD-3, LD-5, LD-6 | COVERED | |
| PP-36 | `botster.url.open` for PR URLs | `project_pipelines/web/screens/home.lua:115`, `project_pipelines/web/screens/ticket.lua:541,583` | none | GAP | G-16. |

### 2.2 orchestrator (OR)

**Host API used** (`parity-work/orchestrator-messaging-telegram.md` 4.1):
- `hub.state`, `hub.hooks` (`hooks.on("hub_rpc_request")` :612)
- `lib.agent` (`list`, `get`, `all_info`)
- `lib.hub`: `get`, `is_local`, `call_safely`, `cleanup_dead`, `agent_list`, `create_agent`, `create_accessory`, `update_session`, `list_workspaces`, `rename_workspace`, `move_agent_workspace`, `delete_agent`, `get_pty_snapshot`, `send_message`, `command`, `post`, `receive_messages`
- `hub.hub_id`, `mcp.tool` ×11, `hub_discovery.list`, `spawn_targets.list/get/inspect`, `socket.send`, `timer.every/cancel`, `log.*`

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| OR-1 | `whoami` (session, hub, repo, branch, worktree, agent, workspace, status, label, task) | `orchestrator/init.lua:84-115` | HC:MC-2 native `whoami`; HP:VC-1 | PARTIAL | Native `whoami` exists. Repo, branch, worktree and workspace are not on the session row (G-21). |
| OR-2 | `list_hubs` and a `hub_id` argument on every tool (other local Hubs through `hub_discovery`) | `orchestrator/init.lua:117-160` | HP:VC-8 refuses a foreign Hub; federation is out of scope | USER | U-2. |
| OR-3 | `list_spawn_targets` with git facts (is_git_repo, repo_name, current_branch, has_botster_dir) | `orchestrator/init.lua:162-189` | HP:`spawn_targets.list`, CA-13 | PARTIAL | G-24 (no git identity on the row). |
| OR-4 | `create_agent{issue_or_branch, prompt, agent_name, workspace_*, target_id, target_name, label}` | `orchestrator/init.lua:191-295` | HP:`ensure_worktree_and_spawn`, CA-14, SA-1; HC:MC-2 `spawn_session` | PARTIAL | G-3 (worktree request) and G-1 (workspace). The accepted-then-installed shape replaces the synchronous payload (acceptable). |
| OR-5 | `create_accessory{accessory_name, branch, from_worktree, …}` | `orchestrator/init.lua:297-397` | HP:`session_types.spawn` | PARTIAL | G-3 (spawn inside an existing worktree). |
| OR-6 | `list_workspaces`, `rename_workspace`, `move_agent_workspace` (Hub-owned workspace store today) | `orchestrator/init.lua:399-480` | HP:entities, views (the workspaces plugin, UI section 14) | PARTIAL | Workspaces move into a plugin (botster-workspaces). Its tools are COVERED. Other plugins that place sessions in workspaces need G-1. **Porting note:** v1 needs a workspaces plugin in the shipped set. |
| OR-7 | `update_session{label, task}` on any session | `orchestrator/init.lua:482-537` | HP:SA-1 (`session_update:any`), HC:MC-2 native `update_session` | PARTIAL | Label COVERED. `task` has no row field (G-21). |
| OR-8 | `delete_agent{delete_worktree}` on any session, running or not | `orchestrator/init.lua:539-572` | HP:SA-1 `stop` + `delete`; HC:MC-2 `stop_session`, `delete_session` | PARTIAL | Worktree delete: G-3. |
| OR-9 | `get_pty_snapshot` (visible screen text) | `orchestrator/init.lua:574-604` | HP:SA-1 `read_screen` (`session_screen:any`); HC:MC-2 `read_screen` | COVERED | |
| OR-10 | Label resolution (`agent_label` → session) | `orchestrator/init.lua:50-78` | HP:SS-1 `label` filter | COVERED | |
| OR-11 | Hub-to-hub RPC answer (`hub_rpc_request` over the Unix socket: send_message, hub_command, post, receive, snapshot, agent list) | `orchestrator/init.lua:612-765` | none | USER | U-2. The RPC is unauthenticated today (`handlers/socket.lua:96-103`). Do not carry it forward as is. |
| OR-12 | 60 s cleanup of dead remote Hub connections | `orchestrator/init.lua:771-773` | – | USER | Part of U-2. |
| OR-13 | Old aliases `agent_id` (move, update, delete, snapshot) and `profile` (create) | `~/.botster/plugins/orchestrator/init.lua:349,402,446,481,208` | HP:MT-1 | COVERED | Plugin choice. Already removed from the dev copy. |

### 2.3 messaging (MS)

**Host API used:**
- `lib.hub`: `call_safely`, `get`, `post`, `receive_messages`
- `lib.agent`: `list`, `get`
- `mcp.tool` ×2, `json.encode`, `log.info`
- The catalog copy also calls `hub:notify`.

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| MS-1 | `post_message{payload, session_uuid or agent_label, msg_type, reply_to, expires_in}`, sender from the caller, msg_id dedupe, 3600 s TTL | `messaging/init.lua:27-101`; HUB/`lua/lib/hub.lua:597-644` | HP:VC-1, ST-9, SS-1 (label), section 9.6; HC:MC-3 | COVERED | v1 is stronger: durable, at-least-once. |
| MS-2 | Doorbell typed into the target terminal (probe `zx`, human-activity wait, bracketed paste, Enter) | HUB/`src/agent/message_delivery.rs:1-360` | HP:SW-1 to SW-10 | COVERED | |
| MS-3 | `receive_messages` (caller-only, destructive drain) | `messaging/init.lua:103-128` | HP:VC-1, ST-9; HC:MC-3 (lease and ack) | COVERED | v1 changes it to lease and ack (improvement). |
| MS-4 | Refuse posts to accessories and missing sessions | HUB/`lua/lib/hub.lua:605-612` | HP:SS-2, session row `session_type`/`role` | COVERED | |
| MS-5 | Cross-hub post and receive (`hub_id`) | `messaging/init.lua:90-99` | HP:VC-8 | USER | U-2. |
| MS-6 | `notify_session` (catalog copy only; PP prompts still name it) | catalog `messaging/init.lua:100-167`; PP `project_pipelines/engine.lua:1128,2046` | HP:SW-1 | COVERED | Plugin-owner decision (section 4.2 P-1): restore it or fix the PP prompts. |
| MS-7 | Used as a library by other plugins (PP, BB post into inboxes) | PP `project_pipelines/engine.lua:528`; BB `botster-bugs/init.lua:235` | none | GAP | G-1. |

### 2.4 telegram (TG)

**Host API used:**
- `lib.hub` `send_message`, `lib.session` `list/get`, `session:info`, `hub.state`
- `secrets.get` (:29-30), `http.request` ×6, `json.*`
- `timer.every/cancel` (:291-297), `mcp.tool` (:340), `hooks.on` (`after_agent_create` :358, `after_agent_close` :363), `log.*`

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| TG-1 | Long poll `getUpdates` every 5 s with offset and an in-flight guard | `telegram/init.lua:297-334` | HP:NT-3 (async, never holds the slot), CA-12, NT-10 (the Telegram-shaped run) | COVERED | The offset should move to the store (improvement). |
| TG-2 | Bot API sends: `sendMessage` (with an inline keyboard), `sendChatAction`, `answerCallbackQuery`, `editMessageText` | `telegram/init.lua:46-80,163-175` | HP:NT-2, NT-3, NT-5 (`{{secret:bot_token}}`) | COVERED | |
| TG-3 | Secrets `telegram/bot_token`, `telegram/chat_id`; register nothing when missing | `telegram/init.lua:29-36` | HP:NT-5, HS-7 (bootstrap reads include `config.get`, not `secrets.get`) | PARTIAL | Declare `chat_id` as config. `secrets.get` is not a bootstrap read, so "register no tool when unset" becomes "the tool answers not configured". Small behavior change: accept it, or add `secrets.get` to HS-7 (G-25). |
| TG-4 | `telegram_send{text}` MCP tool | `telegram/init.lua:340-352` | HP:MT-1 | COVERED | |
| TG-5 | `/agent` picker, sticky routing, single-session auto-route, pending message queue | `telegram/init.lua:147-284` | HP:SS-1 (labels), ST-* (state) | COVERED | Do not carry forward the defects in `parity-work/orchestrator-messaging-telegram.md` section 6 (accessory routing, unchecked callback chat). |
| TG-6 | Deliver Telegram text by typing into the agent terminal | `telegram/init.lua:187-190` | HP:SW-1 (`session_safe_write:any`) | COVERED | |
| TG-7 | "Agent started/closed" notices for every session | `telegram/init.lua:358-371` | HP:EV-9 `hub/session.created`, `hub/session.ended` (`events_hub`, `session_read:any`) | COVERED | |

### 2.5 github (GH)

**Host API used:**
- `hub.state`, `lib.agent` (`get`, `get_meta`, `find_by_workspace`), `lib.hub` (`send_message`, `delete_agent`, `create_agent`), `hub.hooks` (`pty_notification`)
- `rawget(_G, "spawn_targets")` and `rawget(_G, "plugin_worker_parent_hub")`
- `hub.detect_repo`, `hub.api_token`, `hub.server_id`, `config.server_url`
- `secrets.get/set`, `http.post` (sync), `http.request`, `mcp.proxy`, `timer.every/cancel`, `events.emit`
- `action_cable.connect/subscribe/perform/close`

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| GH-1 | 14 `github_*` MCP tools proxied from the remote MCP server `mcp.trybotster.com`, tool list refreshed every 600 s, argument normalization (`repository` → `repo`) | `github/mcp_proxy.lua:105-147` | HP:MT-7 (`mcp_proxy` not in 1.0), section 22 | USER | U-3. v1 fallback without a contract change: declare the 14 tools statically and forward JSON-RPC with `http.request` (NT-3). That loses the dynamic tool list, and still needs G-7 and G-8. |
| GH-2 | Mint the MCP token: `POST {server_url}/integrations/github/mcp_tokens` with `Bearer hub.api_token()`; store it with `secrets.set`; clear and re-mint on 401 | `github/mcp_proxy.lua:16-103` | HP:NT-5 (operator-only secrets) | GAP | G-7 (plugin-writable secret) and G-8 (Hub cloud credential). |
| GH-3 | Inbound GitHub events over ActionCable (`Github::EventsChannel` per repo), ack by `perform("ack")`, server replays unacked | `github/event_routing.lua:267-363` | HP:NT-4 (outbound WebSocket; ActionCable is JSON over WebSocket) | PARTIAL | The transport is possible. The authenticated cloud identity is G-8/U-4. |
| GH-4 | Route the Hub repo plus every enabled spawn-target repo, recomputed every 30 s | `github/init.lua:69-147` | HP:`spawn_targets.list`, CA-12 | PARTIAL | G-24 (repo identity of a target). |
| GH-5 | Emit `pr_merged`, `pr_review_submitted`, `pr_comment` to other plugins | `github/event_routing.lua:63-255` | HP:EV-1 (public, `plugins` audience), EV-9 | COVERED | |
| GH-6 | Ack a PR event to the cloud only if a subscriber received it | `github/event_routing.lua:276-281` | HP:EV-4 (at-most-once, `accepted` only) | GAP | G-1 (a call with a result gives delivery feedback). |
| GH-7 | Mention: type a notice into the agent of workspace `repo#N` (with 👀), else spawn a new agent on branch N with metadata | `github/event_routing.lua:23-61,291-310` | HP:SW-1, CA-14, SA-1 (own metadata namespace), SS-1 | PARTIAL | Lookup by workspace needs G-1, or the plugin keeps its own index in its metadata namespace (possible). Spawn needs G-3 (branch) and G-24 (target by repo). |
| GH-8 | `agent_cleanup`: delete agents of `repo#N`, keep worktrees | `github/event_routing.lua:283-290` | HP:SA-1 stop + delete (`:any`) | COVERED | |
| GH-9 | Terminal notification of an issue-linked agent → `POST /api/hubs/{server_id}/notifications` (`question_asked`) | `github/notifications.lua:14-56` | HP:EV-14 `terminal/notification`, NT-3 | PARTIAL | The trigger is COVERED. The cloud call needs G-8. |
| GH-10 | Reload closes the ActionCable connection and cancels timers | `github/init.lua:162-171` | HP:NT-8, LD-6 | COVERED | |

### 2.6 botster-bugs (BB)

**Host API used:** `lib.agent` (`get`, `info`), `lib.hub` (`list_owned_sessions`, `post`, `notify`, `create_agent`), `spawn_targets.list/get/inspect`, `mcp.tool`, `log.info`, `os.time`.

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| BB-1 | `file_botster_bug{title, description, severity, reproduction_steps, expected, actual, evidence, source, target_id, target_name}` | `botster-bugs/init.lua:259-332` | HP:MT-1, VC-1 | COVERED | |
| BB-2 | Find the live orchestrator among owned sessions (`metadata.role`) | `botster-bugs/init.lua:110-123` | HP:SS-1 `owner`, SA-1 own metadata | COVERED | |
| BB-3 | Post the report as a durable task into the orchestrator inbox and ring it | `botster-bugs/init.lua:223-257` | none for the post; HP:SW-1 for the ring | GAP | G-1 (messaging is a plugin). |
| BB-4 | Spawn a codex orchestrator on branch `botster-bugs-main` from `base_ref=main` in workspace "Botster Bugs" | `botster-bugs/init.lua:304-323` | HP:`ensure_worktree_and_spawn`, CA-14 | PARTIAL | G-3 (branch, base_ref) and G-1 (workspace). |
| BB-5 | Resolve the target by id, by name, or the one named `trybotster`, with repo name | `botster-bugs/init.lua:59-108` | HP:`spawn_targets.list/validate` | PARTIAL | G-24 (repo name). |

### 2.7 knowledge-inbox-pipeline (KI)

**Host API used:**
- Modules: `hub.hooks`, `hub.state`, `lib.hub` (`update_session`, `list_agents`, `create_agent`, `send_message`, `create_accessory`), `lib.session` (`get`, `find_by_meta`, `close`), `lib.target_context`, `lib.action`, `lib.plugin_assets`, `lib.surfaces`
- Host internals: `handlers.connections`, `ctx.client.send_ui_tree_snapshots`
- Filesystem and watch: `fs.exists/is_dir/listdir/stat`, `watch.directory/unwatch`
- `timer.after/every/cancel`, `events.on/off`, `hooks.on/off`, `ui.*` (including `iframe`, `session_list`, `session_terminal`)
- `mcp.tool` ×3, `hub.resolve_command_path`, `hub.server_id`, `spawn_targets.list`

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| KI-1 | 3 MCP tools (status, inbox dispatch, health dispatch) | `knowledge-inbox-pipeline/init.lua:961-1022` | HP:MT-1 | COVERED | |
| KI-2 | Watch `~/knowledge/inbox` (create, modify, rename) with a 1 s debounce | `knowledge-inbox-pipeline/init.lua:914-938` | none (`fs:<root>` "designed, not built", HP section 9.1) | GAP | G-6. |
| KI-3 | Read the vault: inbox listing, today's health report, graph HTML stat | `knowledge-inbox-pipeline/init.lua:66-121,261-283` | none | GAP | G-6. Hard-coded `/Users/jasonconigliari/knowledge` becomes config (HP:CF-1) plus an fs root. |
| KI-4 | Spawn codex workers (Knowledge Worker, Vault Health Worker, Chat) on target `knowledge`, branch `main` | `knowledge-inbox-pipeline/init.lua:313-396` | HP:CA-14, `session_types.spawn` | PARTIAL | Branch or "main checkout" location: G-3. Workspace "Vault": G-1. |
| KI-5 | Ping existing workers by typing instructions | `knowledge-inbox-pipeline/init.lua:344-375` | HP:SW-1 | COVERED | |
| KI-6 | Claim pre-existing sessions by label and rewrite their owner metadata | `knowledge-inbox-pipeline/init.lua:153-192` | HP:SA-1 `session_update:any` (own namespace), CP-4 | PARTIAL | The Hub-recorded owner (the spawning plugin) cannot change. Acting on claimed sessions needs `:any` scopes. |
| KI-7 | Timers: 60 s ping loop, hourly health, 1 s graph completion poll, cooldowns | `knowledge-inbox-pipeline/init.lua:454-620,943-959` | HP:CA-12 | COVERED | |
| KI-8 | Run `bash ops/scripts/graph/visualize.sh` as a hidden accessory with auto-close; detect exit and output mtime | `knowledge-inbox-pipeline/init.lua:492-555,880-911` | HP:CA-19, EV-14 `terminal/exit` | PARTIAL | G-2 (a process, not a visible session) or G-18 (hidden session). The output mtime check needs G-6. |
| KI-9 | Surface "Vault": overview panel with status lines and 4 action buttons; nav order 25; sidebar | `knowledge-inbox-pipeline/init.lua:622-753,822-872` | HP:VW-1 to VW-5; UI parts | COVERED | Status becomes a plugin-published singleton kind. |
| KI-10 | Full-screen knowledge graph: `plugin_assets.expose_file(graph.html)` in a sandboxed `iframe` | `knowledge-inbox-pipeline/init.lua:755-811` | none (HP section 1: no plugin-supplied UI code) | USER | U-5. |
| KI-11 | Sidebar session list filtered to this plugin's sessions | `knowledge-inbox-pipeline/init.lua:744-750` | UI:UI-BD-2 (Hub `session` kind as a source) | PARTIAL | G-14 (no owner field on the client session row). |
| KI-12 | Session terminal route | `knowledge-inbox-pipeline/init.lua:813-819` | UI:UI-SL-3 `open` | COVERED | |
| KI-13 | React to `agent_created` and `agent_lifecycle` for its own spawns | `knowledge-inbox-pipeline/init.lua:1034-1067` | HP:CA-16, EV-6 | COVERED | |

### 2.8 cloudflare-hosted-preview (CF)

**Host API used:**
- `lib.hub`: `update_session`, `list_owned_sessions`, `create_accessory`, `prepare_plugin_command`
- `lib.session`: `list`, `get`, `update`, `close`, `get_meta`
- `lib.session_actions` `register/unregister`, `lib.target_context`, `hub.state`
- `http.request`, `json.decode`, `timer.after`, `hooks.on/off` (`pty_output`, `agent_created`), `events.on/off` (`plugin_command_prepared`, `process_exited`), `os.getenv`

**What it exposes (answering the brief):**
- **It tunnels.** It runs `cloudflared tunnel --url http://127.0.0.1:<port> --no-autoupdate --config <empty file>` (`cloudflare-hosted-preview/init.lua:498-510`).
- **What is exposed:** exactly one local HTTP origin, the parent session's forwarded port. The Hub allocates it in 46000-61999 and exports it as `PORT` (HUB/`lua/lib/session.lua:160-217,572-579`). That port is normally the session's dev server. The Hub, terminals and other ports are not exposed.
- **Access control: none.**
  - The public hostname is a random `*.trycloudflare.com` scraped from cloudflared output (`:154-165`).
  - There is no Cloudflare Access, no account, no API token and no DNS record.
  - The only other outbound call is a public DNS-over-HTTPS readiness check to `cloudflare-dns.com` (`:336-343`).
  - Anyone who has the URL reaches the dev server.
- It does **not** forward terminals or relay signalling.

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| CF-1 | Per-session allocated port, readable by the plugin, exported to the session | HUB/`lua/lib/session.lua:572-579` | HP:CA-20 (A4r25, A4r27) | PARTIAL | CA-20 exports `BOTSTER_SESSION_PORT`. Today's variable is `PORT`, and dev servers read it: G-19. |
| CF-2 | Session action "Enable Cloudflare preview" on every session with a port, with dynamic label (Enable, Starting, Retry, Disable), status, url, error and icon | `cloudflare-hosted-preview/init.lua:928-965` | HP:VW-8 (A4r25) | PARTIAL | VW-8 has a static label, and `applies_to` cannot test "has a port". Dynamic label, status and URL are missing: G-20. |
| CF-3 | Resolve the `cloudflared` binary on PATH (typed `command_missing` with an install URL) and write the config file | `cloudflare-hosted-preview/init.lua:609-676` | HP:CA-19 | PARTIAL | CA-19 does not say that `argv[0]` is looked up on PATH, and has no typed "command not found" reason. G-2 covers both. The config file can be written by the command itself (`sh -c`). |
| CF-4 | Run cloudflared as a plugin-owned long-lived process | `cloudflare-hosted-preview/init.lua:546-606` | HP:CA-19, CA-15 | PARTIAL | This is a visible session in v1 (G-18). |
| CF-5 | Scrape the tunnel URL from the process output | `cloudflare-hosted-preview/init.lua:729-777` | HP:EV-14 `terminal/output` (`session_output:read`, own sessions by default) | COVERED | |
| CF-6 | DNS readiness polling every 1 s before showing the URL; 20 s URL discovery timeout | `cloudflare-hosted-preview/init.lua:310-417` | HP:NT-3, CA-12 | COVERED | |
| CF-7 | Mirror the state into the parent session (`plugin_state.cloudflare_hosted_preview`) | `cloudflare-hosted-preview/init.lua:194-257` | HP:SA-1 `session_update:any` (A4r25); HC session row `metadata` | PARTIAL | Clients do not render plugin metadata on the session row: G-20. |
| CF-8 | Exit → error state; one connector per parent; reconcile and re-adopt on load | `cloudflare-hosted-preview/init.lua:781-904` | HP:EV-14 `terminal/exit`, SS-1 `owner`, CA-15 | COVERED | |
| CF-9 | Expose a local dev server to the public internet with no authentication | `cloudflare-hosted-preview/init.lua:498-510` | HP section 22 (tunnels deferred by the user) | USER | U-1. |

### 2.9 rails-worktree-lifecycle (RW; repo-scoped; variants RT = trybotster, RH = hyperflex and trybackster, RJ = jupiter, RHe = hearth)

**Host API used:**
- `hub.hooks` `on` (`worktree_created`, `after_agent_create`, `worktree_deleted`)
- `plugin.db` (RT, RH), `worktree.repo_root`
- `fs.exists/read/write/mkdir/copy`, `os.execute`, `io.popen`, `os.time`, `log.*`
- Call sites: `parity-work/cloudflare-worktree-demo.md` section 2.2.

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| RW-1 | Loaded from `<repo>/.botster-dev/plugins/` for each enabled spawn target; one instance per repo, keyed `repo:<root>:<name>` | HUB/`lua/hub/loader.lua:194-203`, HUB/`lua/hub/init.lua:202-280` | HC `package` kind (`create` from a local path); HP:CP-1, MN-1 `name` | GAP | G-5 (no target-scoped package, no duplicate names). |
| RW-2 | Every instance fires for every repo's worktrees (a defect: competing `.env` prefixes, possible cross-worktree `dropdb`) | HUB/`lua/hub/hooks.lua:191-222`; live metrics: 63 runs each | – | USER | U-6: keep or fix. Recommendation: fix (a per-target event filter under G-5). |
| RW-3 | Trigger on `worktree_created` and again on every `after_agent_create` into a worktree | RT:369-381 | HP:EV-8 (`worktree_created`), EV-9 `hub/session.created` | PARTIAL | G-4 (payload must carry the worktree reference and the target). G-21 (the session row has no worktree, so "agent created in a worktree" cannot be recognized). |
| RW-4 | Copy `.gitignore`, `.ruby-lsp/.gitignore` and the tailwind previews dir from the source checkout into the worktree | RT:155-206 | none for fs writes; G-2 would run `cp` | GAP | G-2 (or G-6 with write). |
| RW-5 | Write the `RAILS_WORKTREE_DATABASE_PREFIX` block into `.env` and `mise.local.toml`; `mise trust` | RT:208-295 | as RW-4 | GAP | G-2. |
| RW-6 | Clone or install `node_modules` (RT, RH); Ghostty submodule init (RT) | RT:93-153 | as RW-4 | GAP | G-2. |
| RW-7 | Symlink `<repo>/reference` into the worktree (RHe) | RHe:20-46 | as RW-4 | GAP | G-2. |
| RW-8 | On `worktree_deleted`: drop 8 databases plus `_test-N` (`psql`, `dropdb --force`) | RT:330-367; RJ:293-332 | HP:EV-8 `worktree_deleted`; G-2 | GAP | G-2 and G-4 (the deleted event needs the worktree id and branch). |
| RW-9 | Ledger of worktree → prefix (RT, RH `plugin.db`) | RT:10-21,234-240 | HP:ST-* | COVERED | |
| RW-10 | Handlers block for about 7.4 s with no veto of creation | live `max_exec_ms` 7380-7483 | HP:IV-3, IV-4 (async operations do not hold the slot under G-2) | COVERED once G-2 lands | |

### 2.10 Hub-builtin MCP tools (HB; agents call them today)

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| HB-1 | `list_plugins`, `reload_plugin`, `enable_plugin`, `disable_plugin`, `get_plugin_logs` for agent sessions | HUB/`lua/hub/mcp_defaults.lua:914-995` | HC `package` kind and `plugin_log` kind (operator verbs); HC:MC-2 lists no package tools | USER | U-7: may an agent (session caller) manage packages? |
| HB-2 | `secrets_set{namespace, key, value}` for agent sessions | HUB/`lua/hub/mcp_defaults.lua:888` | HP:NT-5 (operator only); HC:SF-3 | USER | U-7. |
| HB-3 | `reload_layout` (web layout override files) | HUB/`lua/hub/mcp_defaults.lua:939` | HP:EN-6, VW-2 (no layout code) | COVERED | This is removed by design (cold cut to declarative views). No user plugin depends on it. |

### 2.11 Catalog templates (WL, DS)

| # | Feature | Citation | v1 clauses | Verdict | Note |
|---|---|---|---|---|---|
| WL-1 | Observers on `worktree_created` and `worktree_deleted` (examples commented out) | `worktree-lifecycle/init.lua:51-63` | HP:EV-8 | COVERED | The template must be rewritten on G-2 and G-4 to stay useful as an example. |
| DS-1 | Web surface with one route and a nav entry | `demo-surface/init.lua:9-20` | HP:VW-1 to VW-5 | COVERED | |
| DS-2 | TUI component `botster.ui.register_component` linked by `-- @tui` | `demo-surface/tui/status.lua:8-13` | none | COVERED (no regression) | Legacy never renders it: `render_component` has no caller (`parity-work/legacy-host-api.md` 5.11). |

## 3. Gap list (numbered; smallest contract addition; owner)

Rulings respected:
- The Hub offers low-level primitives, and plugins own the logic.
- Messaging is a plugin.
- The connection primitives are in v1.
- Connectors and session forwarding are deferred.
- Plugins are isolated service processes with no sockets or subprocesses of their own (HP:TR-2).

Each gap names the features it closes.

| # | Gap | Features | Smallest addition | Contract (owner) |
|---|---|---|---|---|
| **G-1** | **Plugin-to-plugin request with a result.** Events are at-most-once and give no result (HP:EV-4). There is no cross-plugin read (EV-12). So a plugin cannot post into the messaging plugin's inbox, place a session in a workspace, or hand off a PR event with delivery feedback. | PP-9, PP-17, PP-19, MS-7, GH-6, GH-7, BB-3, BB-4, KI-4, OR-4, OR-6 | A caller kind **`plugin {plugin_key, instance}`** (VC-1 extension, fail-closed for old hosts). A host call **`plugins.call{plugin, tool, args, idempotency_key?}`**: an async operation, kind `plugin_call` (CA-16), whose terminal record carries the callee's `Result`. The callee opts in per tool (`callable_by: ["plugins"]` in the tool descriptor). The caller needs the grant `plugin_call:<owner>`. It is never re-entrant (CA-4 holds: the callee runs as its own Invoke). An idempotency key gives exactly-once for durable posts. | HP (plugin pair) |
| **G-2** | **Command execution with captured output and a timeout, not a session.** `run_command_gate` is async, has no shell, runs in its own process group, and returns the exit status with 8 KiB output tails. PP gates and git steps, all RW shell work, the KI graph script and CF binary resolution use it (or blocking `os.execute`). | PP-15, PP-16, RW-4 to RW-8, KI-8, CF-3 | **`process.run{argv, cwd: {target_id \| worktree_id, relative?}, env?, timeout_ms, stdout_tail_bytes, stderr_tail_bytes}`**: an async operation, kind `process` (CA-16), with terminal record `{exit_code?, signal?, stdout_tail_b64, stderr_tail_b64, truncated, duration_ms, reason?: command_not_found \| timed_out \| cwd_not_allowed}`. `argv[0]` is resolved on the Hub's PATH. The process group is killed on timeout, cancel, unload and crash (NT-8 style). Scope **`process_run`**, named "run arbitrary commands" at approval. Bounds: `max_argv_bytes`, `max_env_bytes`, `process_concurrent`. The same `cwd` reference should also be added to `sessions.create_command`, which has no target today. | HP (plugin pair) |
| **G-3** | **Worktree mutation and a specified worktree request.** Plugins can list and show worktrees only. The `ensure_worktree_and_spawn` "worktree request" has no schema. | PP-9, PP-10, PP-12, PP-13, OR-4, OR-5, OR-8, BB-4, GH-7, KI-4 | (a) Specify the worktree request: `worktree: {branch, base_ref?, reuse: worktree_id?}`, plus `session_types.spawn{..., worktree_id?}` to start a session inside an existing worktree (today's `from_worktree`). Also specify the spawn `context` shape (the prompt). (b) **`worktrees.delete{worktree_id, delete_branch?: false}`**: scope `worktree_delete` (worktrees created by its own spawns) and `worktree_delete:any`; refused `failed{wrong_state: occupied}` while a current session uses it. | HP (plugin pair); HC `worktree` kind already has operator delete |
| **G-4** | **Worktree events usable by a plugin.** EV-8 names `worktree_created` and `worktree_deleted` but gives no payload. RW needs the worktree reference, target, branch and creating session. The delete event must carry the last known identity, because it fires after removal. | RW-3, RW-8, WL-1 | Payload schema: `{worktree_id, target_id, branch, session?: SessionRef}` for created. The same last-known fields for deleted. The events become subject-scoped by target (for G-5). | HP (plugin pair) |
| **G-5** | **Target-scoped (repo-scoped) packages.** Today the Hub discovers `<target>/.botster[-dev]/plugins/*` for each enabled spawn target and loads one instance per target. | RW-1, RW-2 | Manifest `scope: "device" \| "target"`. A target-scoped package is admitted per spawn target with key `<name>@<target_id>`. Its `worktree_*` and session events are filtered to that target. Whether the Hub auto-discovers the directory, or the operator admits each one with `create(package, {source: {local_path}, target_id})`, is a user choice (U-6). | HP (plugin pair) + HC `package` row `target_id?` (plan auditor) |
| **G-6** | **Filesystem read and watch under an operator-approved root.** HP section 9.1 lists `fs:<root>` as "designed, not built; behind a feature name". No clause exists. | KI-2, KI-3, KI-8 | Make the `fs` feature a v1 clause set. The manifest declares `fs_roots: [{name, path_config}]`, and the operator approves each. Calls: `fs.list{root, path}`, `fs.stat`, `fs.read{max_bytes}`. `fs.watch{root, path}` gives level-triggered `Invoke{origin: fs_watch}` like ST-6. No write in v1: writes go through G-2. | HP (plugin pair) |
| **G-7** | **Plugin-writable secret** for a credential the plugin mints and rotates. | GH-2 | Manifest `secrets: [{name, plugin_writable: true}]` plus `secrets.put{name, value \| null}`. The value never appears in logs, receipts or `config.get` (NT-5 rules). | HP (plugin pair) |
| **G-8** | **Hub cloud identity for a plugin** (`hub.api_token`, `hub.server_id`, `config.server_url`), needed for the trybotster.com GitHub integration. | GH-2, GH-3, GH-9 | Only if U-4 says yes: a placeholder `{{hub_credential:cloud}}` substituted by the I/O service under NT-5 rules, and only for the Hub's configured cloud origin. `hub.identity` returns `server_id` and `cloud_origin`. Otherwise the operator stores a cloud token in the plugin's secrets by hand. | HP (plugin pair), conditional on U-4 |
| **G-11** | **Web push delivery of a plugin notice** (kind, title, body, url, tag). | PP-21 | Extend the notice reactions (HC:SF-5) with `delivery: ["in_app", "push"]`, `url_pointer` (a deep link under G-13) and `tag_pointer`. The Hub sends the web push for a declared notice. | HC (plan auditor) + HP event declaration field |
| **G-12** | **Plugin policy over terminal notifications of its own sessions** (suppress unless the text matches). | PP-20 | A declarative filter on a plugin-declared session type: `notifications: {mode: "all" \| "none" \| "matching", patterns: [literal substrings]}`. Clients apply it to OSC 9/777 and bell. Nothing is re-entrant and no plugin code runs on the notice path. | HC (plan auditor) + HP session type field |
| **G-13** | **Deep links: open a view with a preset selection.** PP routes carry ids (ticket, run, project, pipeline), and push notices and cross-page buttons target them. | PP-22, PP-24, PP-26, PP-36 (return path), G-11 | UI: an activation `{navigate: {view, select: {<slot>: Operand}}}`. HC `navigation` target `{kind: view, id, select?: {slot: row key}}`. A client URL form `…/<package>/<view>?<slot>=<row key>` (Web). The selection stays client-local after open (P5). | UI (0017) + HC `navigation` (plan auditor) |
| **G-14** | **Session rows filtered by owning plugin in a view.** | KI-11 | The client `session` row gains `owner?` (the spawning plugin key; HP already records it for SS-1). The UI allows it in a `where`. | HC (plan auditor) + UI allowlist (UI Q5) |
| **G-15** | **Home-page widgets and nav badges.** | PP-27, PP-28 | UI and HC: a view may declare `placement: "home"` with `size: small \| medium \| wide \| full` and `order`. The Hub `workspace_panel` home renders home-placed views. The `navigation` row gains `badge?: {source: {kind, id, field}, display: dot \| count, tone}`, bound to a plugin entity field (no code). | UI (0017) + HC (plan auditor) |
| **G-16** | **External URL link** (open PR, preview URL). | PP-36, CF-2, CF-7 | `Field.display: "link"` (value must be an `https`/`http` URL; the client opens it outside the app after showing the host). Or an activation `{url: Operand}`. | UI (0017) |
| **G-18** | **Plugin-internal session presentation.** Helper sessions (cloudflared, graph generator, and command sessions if G-2 is not taken) show in every session list, where today they are `visibility=plugin/hidden`. | CF-4, KI-8, PP-15 | A spawn argument `presentation: "default" \| "plugin" \| "hidden"`, carried on the client row. Clients hide `plugin` and `hidden` from the default session list. A `plugin` session shows only through that plugin's views. | HP (plugin pair) + HC row (plan auditor) |
| **G-19** | **`PORT` variable name.** CA-20 exports only `BOTSTER_SESSION_PORT`. Dev servers (Rails `bin/dev`) read `PORT`, which the Hub exports today. | CF-1 | The session type declares `port_env: ["PORT"]`. The Hub also sets those names. | HP CA-20 (plugin pair) + HC MC-1c |
| **G-20** | **Dynamic per-session action state and visible plugin state on the session row** (label, status, error, URL; gated on "has a port"). | CF-2, CF-7 | VW-8 gains `label_pointer`, `status_pointer` and `url_pointer` into `metadata[<plugin>]` (the plugin writes them with `sessions.update`). `applies_to` gains `has_port: bool`. UI and HC say how clients render a contributed action's status and link. | HP VW-8 (plugin pair) + UI (0017) + HC (plan auditor) |
| **G-21** | **Session row fields plugins read:** worktree reference and branch, title (OSC title or task), owner. Today these are `worktree_path`, `branch_name`, `display_name`, `task`, `owner_plugin` from `session:info()`. | PP-13, PP-32, OR-1, OR-7, RW-3 | The client `session` row gains `worktree_id?`, `branch?`, `title?` (set by `sessions.update` or the terminal title) and `owner?` (G-14). Carried to plugins by SS-1. | HC (plan auditor) |
| **G-24** | **Spawn-target git identity** (repo `owner/name`, current and default branch). | OR-3, GH-4, GH-7, BB-5 | The `spawn_target` row gains `git?: {repo, current_branch?, default_branch?}` (Hub-derived, sanitized, no absolute paths). | HP CA-13 (plugin pair) + HC `spawn_target` (plan auditor) |
| **G-25** | **`secrets.get` in bootstrap**, so a plugin registers nothing when its credentials are unset (Telegram today). | TG-3 | Add `secrets.get` to the HS-7 bootstrap reads. Or accept the change to "tool answers not configured". | HP (plugin pair); minor |
| **G-26** | **HP section 1 still says HTTP and secrets are out of scope for 1.0** ("HTTP, filesystem roots and secrets capabilities … behind feature names, section 9.9"). This contradicts section 9.8 (in v1). | editorial | Change section 1 to say that only filesystem roots are behind a feature name, or that they are in v1 under G-6. | HP (plugin pair); editorial |
| **G-27** | **Cross-contract spawn-location naming.** HC `session` create takes `target_id` and no worktree. HP has `worktree_id` only in `ensure_worktree_and_spawn` results. | G-3 | HC `create(session, {…, worktree_id?})` mirrors G-3(a). Operators need it too: the ticket page "spawn in ticket worktree". | HC (plan auditor) |

G-9, G-10, G-17, G-22 and G-23 were folded into user decisions or other gaps:
- G-9 is the MCP proxy: U-3.
- G-10 is multi-hub: U-2.
- G-17 is the iframe: U-5.
- G-22 is agent package management: U-7.
- G-23 is event delivery feedback: G-1.

The numbers are kept so that messages already sent stay valid.

**Risk (not a gap).**
- **R-1: large plugin families.** EN-9 shares one projection per family. PP served 15 families by per-ticket detail query. In v1 every `event`, `run_step`, `gate_result` and `finding` row must be projected, within ST-8 quotas. The client filters at watch time (HC:WT-2).
- **Mitigation:**
  - Use ST-7 store projection, so no plugin memory is involved.
  - Prune the event log.
  - Parameterized watches stay post-v1 unless the PP port proves a quota breach.

## 4. Decisions

### 4.0 Rulings received 2026-09-30 (from the orchestrator via the plan auditor)

- **U-1:** tunnel allowed as a plugin-run command.
- **U-2:** one Hub per machine; multi-local-Hub removed.
- **U-4:** the cloud integration is a separate plugin product.
- **U-5:** sandboxed iframe plugin HTML allowed (UI owner).
- **U-6:** repo-scoped packages auto-discovered per spawn target; the Hub-wide firing is fixed.
- **U-7:** agents are clients with operator-restrictable scopes; MCP is a client binding over the same verbs.
- **Also ruled:** the pairing and signalling Web origin is provided by a plugin, for signalling only, and it is untrusted.
- **U-3:** the MCP proxy is IN v1 (HP MP-1..15); GH-1 is COVERED.
- **U-8:** plugin-owned, noted.
- **Effects on this audit:**
  - OR-2, OR-11, OR-12 and MS-5 are **dropped by ruling**.
  - CF-9 is COVERED under G-2 and CA-19.
  - KI-10 becomes a UI item.
  - HB-1 and HB-2 are COVERED under "agents are clients".
  - G-5 has its design.
  - G-8 is withdrawn from the core contracts (the cloud plugin uses G-7 and NT-*).

### 4.1 User decisions (to the orchestrator; not decided here)

| # | Decision | Why it is the user's | Features |
|---|---|---|---|
| U-1 | **Public dev-server tunnels.** cloudflare-hosted-preview exposes a session's dev-server port to the internet through an unauthenticated Cloudflare quick tunnel (`*.trycloudflare.com`, no Access). It does not forward terminals or relay signalling. On v1 it is buildable from generic primitives: CA-19, CA-20, the EV-14 tap, NT-3, VW-8. Does the "tunnels and forwarding deferred" ruling (HP section 22) exclude it, or allow it as a plugin-run command? | The user deferred "plugins that forward a session (for example a tunnel)". This is a tunnel of a session's port. | CF-9, and the CF plugin as a whole |
| U-2 | **Several local Hubs.** Today `list_hubs`, a `hub_id` argument on every orchestrator and messaging tool, cross-hub posts and an unauthenticated hub-to-hub socket RPC exist. v1 refuses a foreign Hub (HP:VC-8) and has no federation. Drop them in v1, or add local multi-Hub? | Federation is out of v1 scope by design. | OR-2, OR-11, OR-12, MS-5 |
| U-3 | **Remote MCP proxy** (`mcp_servers`, HP:MT-7, already flagged in HP section 22). The alternative is the github plugin forwarding 14 static tools over `http.request`: no contract change, but the dynamic tool list is lost. | Already on the sign-off list. | GH-1 |
| U-4 | **Is the trybotster.com cloud integration in v1?** It covers GitHub events over ActionCable, MCP token minting and question notifications, and needs the Hub's cloud credential (G-8). | Cloud federation is optional per the north star. It is not in the three contracts. | GH-2, GH-3, GH-9 |
| U-5 | **Plugin HTML in an iframe** (the knowledge graph page). HP section 1 excludes plugin-supplied UI code. The alternative is a link that opens the file outside Botster (needs G-16). | It is a product and trust decision. | KI-10 |
| U-6 | **Repo-scoped plugins:** auto-discover per spawn target (today) or operator admission per target, and fix the Hub-wide firing (recommended) or keep it. | Today's behavior is a defect that users may rely on. | RW-1, RW-2 |
| U-7 | **Agent authority over packages and secrets.** Today any agent can list, reload, enable or disable plugins, read plugin logs, and set any secret (`secrets_set`). v1 gives these to operators only. The ruling of 2026-09-28 covers sessions, not packages. | Authority decision. | HB-1, HB-2 |
| U-8 | **`notify_session`** in the messaging plugin (removed in dev, still in catalog and named in PP prompts). This is plugin-owned, listed only so nobody decides it silently. The messaging owner may decide it. | Plugin behavior choice. | MS-6 |

### 4.2 Plugin-owner notes (no contract change; for the rebuild)

- **P-1:** Restore `notify_session` or rewrite the PP prompts (U-8).
- **P-2:** Do not carry forward the legacy defects listed in the evidence files:
  - messaging: dropped `from_label`, wrong `from.hub_id`, unauthenticated RPC drain;
  - telegram: `resp` nil index, unchecked callback chat, accessory routing;
  - RW: unescaped `LIKE` in `dropdb` selection, 40-character prefix collision, `.gitignore` overwrite;
  - CF: parent-close cleanup is not registered, and the readiness loop has no deadline;
  - botster-bugs: duplicate orchestrators, target-order mismatch;
  - KI: `force` is ineffective, and status reads write metadata.
- **P-3:** Tool names gain the `<package>__` prefix (HC:MC-2). Every prompt that names a tool must be updated (PP, BB, TG footer).
- **P-4:** A workspaces plugin must be in the shipped v1 set, because workspaces are Hub-owned in the monorepo.

## 5. Requirements for gate 3 (new ids PAR-1 to PAR-27; same columns as `gate3-requirements-coverage-20260930.md`)

| # | Requirement | Source | Satisfying clauses | Status | Note |
|---|---|---|---|---|---|
| PAR-1 | A plugin can call another plugin's opted-in tool and get its result. It can do this durably with an idempotency key, under a verified plugin caller. | user zero-regression bar; PP-17, BB-3, GH-6, OR-6 | none | GAP | G-1 |
| PAR-2 | A plugin can run a command (argv, cwd as a target or worktree reference, env, timeout) and get the exit status and bounded output tails asynchronously, without creating a session. | PP-15, RW-4..8, KI-8, CF-3 | plugin:CA-19 (session only, no output, no timeout) | PARTIAL | G-2 |
| PAR-3 | A plugin can spawn into a new or existing worktree with a named branch and `base_ref`, and can delete a worktree it created (or any, with `:any`). | PP-9, PP-13, OR-4, OR-8, BB-4 | plugin:`ensure_worktree_and_spawn` (unspecified request), `worktrees.list/show` | PARTIAL | G-3, G-27 |
| PAR-4 | Worktree created and deleted events carry worktree id, target, branch and creating session. | RW-3, RW-8 | plugin:EV-8 (names only) | PARTIAL | G-4 |
| PAR-5 | Target-scoped plugin packages: one instance per spawn target, whose events are limited to that target. | RW-1, RW-2 | none | GAP (design ruled) | G-5. Ruled 2026-09-30 (U-6): auto-discovered per spawn target; the Hub-wide firing is fixed. |
| PAR-6 | Read-only filesystem access and a directory watch under an operator-approved root. | KI-2, KI-3 | plugin:§9.1 "fs:<root> designed, not built" | GAP | G-6 |
| PAR-7 | A plugin can store and rotate its own credential as a secret. | GH-2 | plugin:NT-5 (operator-only) | GAP | G-7 |
| PAR-8 | The cloud GitHub integration (events, MCP token, notifications) works, or is explicitly out of v1. | GH-2, GH-3, GH-9 | none | RULED | U-4 (2026-09-30): the cloud integration is a plugin and a separate product; the architecture names nothing cloud-specific. G-8 is withdrawn from the core contracts; that plugin needs only G-7 and NT-*. |
| PAR-9 | The GitHub MCP tools stay available to agents. | GH-1 | plugin:MP-1..15 (MCP proxy, folded into HP) | RULED | U-3: the user said YES; the MCP proxy is in v1. |
| PAR-10 | Web push for plugin notices, with a deep-link URL. | PP-21 | client:SF-5 (in-app notices only) | GAP | G-11 |
| PAR-11 | A plugin sets a notification policy for terminal notices of its own sessions. | PP-20 | none | GAP | G-12 |
| PAR-12 | A view can be opened with a preset selection (deep link), from another view, a notice or a URL. | PP-22, PP-24 | plugin:VW-5; UI:UI-SL-3 | GAP | G-13 |
| PAR-13 | A view can list the sessions a given plugin owns. | KI-11 | UI:UI-BD-2 | PARTIAL | G-14 |
| PAR-14 | Plugins contribute home-page widgets and data-bound nav badges. | PP-27, PP-28 | none | GAP | G-15 |
| PAR-15 | Views can show an external URL as a link that the client opens. | PP-36, CF-2 | none | GAP | G-16 |
| PAR-16 | Plugin helper sessions can be kept out of the default session list. | CF-4, KI-8 | none | GAP | G-18 |
| PAR-17 | A session's allocated port is also exported under names the session type declares (for example `PORT`). | CF-1 | plugin:CA-20; client:MC-1c | PARTIAL | G-19 |
| PAR-18 | A plugin-contributed session action shows per-session label, status, error and link, and can be limited to sessions with a port. | CF-2, CF-7 | plugin:VW-8, SA-1 | PARTIAL | G-20 |
| PAR-19 | The session row gives worktree, branch, title and owner to plugins and clients. | PP-32, OR-1, OR-7, RW-3 | plugin:SS-1; client:§6 `session` | PARTIAL | G-21 |
| PAR-20 | The spawn-target row gives the git repo identity and branches. | OR-3, GH-4, BB-5 | plugin:CA-13 | PARTIAL | G-24 |
| PAR-21 | A plugin can decide at load whether a secret is set. | TG-3 | plugin:HS-7 | PARTIAL | G-25 |
| PAR-22 | Several local Hubs: kept or explicitly dropped. | OR-2, OR-11, MS-5 | plugin:VC-8 | RULED | U-2 (2026-09-30): one Hub per machine. Multi-local-Hub behavior (list_hubs, hub_id routing, hub-to-hub RPC) is removed. hub_id stays in references for future cross-machine federation. |
| PAR-23 | Public dev-server tunnels: allowed or excluded. | CF-9 | plugin:§22 | RULED | U-1 (2026-09-30): allowed as a plugin-run command (CA-19 or G-2, CA-20, EV-14 tap, NT-3). |
| PAR-24 | Plugin HTML (an iframe) in a view: allowed or excluded. | KI-10 | plugin:§1 | RULED | U-5 (2026-09-30): allowed, sandboxed to the plugin's own origin. It goes to the UI contract (0017), with the asset serving in HP. |
| PAR-25 | Agent authority over packages and secrets: kept or operator-only. | HB-1, HB-2 | client:MC-2, SF-3; plugin:NT-5 | RULED | U-7 (2026-09-30, "agents are clients"): an agent is a client caller with an operator-restrictable scope set, and MCP is a client binding over the same verbs and kinds. Package operations are available to an agent that is granted them. `secrets_set` maps to the operator-scoped secret field (SF-3) under that grant. |
| PAR-26 | A workspaces plugin exists in the shipped set and can place a session at spawn time for another plugin. | OR-6, PP-9, KI-4, BB-4 | UI:§14; plugin:entities | PARTIAL | P-4, G-1 |
| PAR-27 | Every durable plugin feature listed in section 2 as COVERED keeps a conformance fixture. At minimum: messaging inbox (VW-7), Telegram long poll (NT-10), store batch (ST-3), spawn reconcile (CA-16). | this audit | plugin:§19 | COVERED | For the suite (gate 4). |

## 6. Messages sent

- Plugin gaps went to the plugin writer (0016): G-1 to G-7, G-18 to G-20, G-24 to G-26, plus the hosted-preview status.
- UI gaps went to the ui-contract owner (0017): G-13 to G-16 and G-20.
- Client gaps went to the plan auditor (003c): G-11 to G-15, G-18 to G-21, G-24, G-27 and the PAR rows.
- The user decisions U-1 to U-8 went to the orchestrator with the completion notice.
