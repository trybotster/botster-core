# Completeness sweep A: "where does it live" (2026-09-30)

Auditor: read-only contracts auditor (subagent). No file was edited. This file is the only output.

## Scope

Package (joint manifest `contracts-v1-joint-manifest-final4.md`):
- Core: `core-contract-v1.17.md`
- Route codec: `route-codec-v1-extract-rev26.md`
- HC: `hub-client-contract-v1-candidate-parity45.md` plus `hub-client-contract-v1-amendment-1-candidate5.md`
- HP: `hub-plugin-contract-v1.1.joint-J4e.md` plus `hub-plugin-contract-v1.1.amendment-1-candidate5.md`
- Lua binding: `hub-plugin-lua-binding-v1.1-r6.md`
- UI: `ui-contract-v1-frozen-20260930.md`, `ui-contract-v1.1-amendment-frozen-20260930.md`, `ui-contract-v1.2-embed-successor-candidate2.md`
- CLI: `cli-spec-v1-candidate26.md` (the newest candidate; no candidate 27 exists)

Question: which names, paths, variables, URLs, config keys and formats must a second party agree on? Which ones does no contract define completely?

Abbreviations for files in citations: HC = HC base, HCA1 = HC Amendment 1, HP = HP J4e, HPA1 = HP Amendment 1, LB = Lua binding, CLI = CLI spec, UI11 = ui-contract 1.1, UI12 = ui-contract 1.2. Citations are `file:line`.

## Summary

| Severity | Count |
|---|---|
| High (two parties cannot interoperate) | 6 |
| Medium (ambiguous) | 16 |
| Low (cosmetic or one-party) | 11 |
| **Total** | **33** |

## Gaps

### High

**G-1. HIGH. Owner: HC (with HP section 15 cross-reference). The Hub configuration file does not exist in any contract.**
- Evidence: the contracts name many "Hub parameters" that an operator sets, but no file, location, format or key syntax.
  - HP:537 "`PluginLimits` is chosen by the Hub per plugin (defaults in the Hub; operator-overridable) ... the defaults are in the Hub's configuration, not in this document".
  - HP:553 "the default lives in the Hub configuration"; HP:567 `session_port_range`; HP:570 `target_plugin_dir`; HP:571 `net_listen`, `assets_listen` "operator-configured addresses"; HP:294 `process_path`.
  - HC:204 `activity_idle_ms`; HC:519 `agent_scopes` "which the operator may restrict per session type or per session"; HC:567 `audit_retention_days`, `audit_max_bytes`; HC:518 terminfo entry "(a Hub parameter)".
  - CLI:154 drops the legacy `worktree_base` key as "the Hub's", but no Hub contract names it.
  - Absence: no `hub.toml`, `config.toml`, `hub.json` or "configuration file" in any file of the package (grep, all 11 files).
- Why high: an operator and a packager cannot set `assets_listen` (needed for every embed, AS-2), `net_listen`, `session_port_range` or `agent_scopes`. A test harness cannot set the LM-5 "test values".
- Smallest amendment (HC, new clause CF-0): "**Hub configuration file.** The Hub reads its parameters from `<config root>/hub.toml` (TOML 1.0; `<config root>` as CLI-RT2). The file is optional. Each key is a parameter name of HC or HP section 15, at the top level, with the type and range that the owning clause states. A missing key takes the documented default. An unknown key or a value out of range makes `botster-hub up` write `failed config_invalid <key>` to the ready descriptor (HS-0) and exit nonzero. The operator creates and edits the file; the Hub never writes it. The Hub reads it at start only. The Hub refuses a file that is writable by another user. `invoke(hub, diagnostics)` reports the effective values." Plus one table in HC that lists every parameter with its default, or a pointer to such a table in the Hub's published docs.

**G-2. HIGH. Owner: HC (verbs) and HP (clauses that need them). Operator settings that have no verb and no file.**
- Evidence:
  - FS-1, HP:303 "the operator binds each `id` to a real directory at enable", but HC:920 `enable` takes `{}`.
  - NT-7, HP:283 "a per-plugin webhook secret (a bearer token or an HMAC that the operator sets)": no verb, no file.
  - TR-1, HP:618 "the operator's trusted-host list by path and digest": no verb, no file.
  - CP-8, HP:144 "on an operator rescan": no `rescan` action on `spawn_target` or `hub` (HC:206, HC:901 to HC:903).
  - MC-1b, HC:519: the operator restricts `agent_scopes` "per session type or per session", but the `session_type` create fields (HC:898) and the `session` create fields (HC:885) have no scope field.
- Why high: a client cannot perform these acts, so a plugin that needs an fs root, a webhook or a non-shipped host cannot be enabled.
- Smallest amendment (HC section 6 and A.9): add `invoke(package, enable, { fs_roots?: { <id>: <absolute path> } })`; add `webhook_secret` as a secret field of the package `configuration` (SF-3 rules); add `invoke(spawn_target, rescan)` (committed); add `agent_scopes?: [string]` to `session_type` create/update and `session` create; state that the trusted-host list is the `trusted_hosts` table of `hub.toml` (G-1) with entries `{ path, sha256 }`.

**G-3. HIGH. Owner: HP. `target_plugin_dir` has no value.**
- Evidence: HP:570 "`target_plugin_dir` | a relative directory name inside the target"; HP:144 "in the plugin directory that `target_plugin_dir` names (a Hub parameter)"; HPA1:19 (PK-8) uses it. No default anywhere. The task brief guesses `<target>/.botster/plugins/*`; no contract says so.
- Why high: a repository author must put target-scoped packages, agent definitions and accessories in a directory that every Hub reads. With a per-Hub value, the same repository works on one machine and not on another.
- Smallest amendment (HP CP-8): "`target_plugin_dir` is `.botster/plugins` by default. The default is contract: a change is MAJOR. An operator override applies to that Hub only, and a repository that ships packages uses the default."

**G-4. HIGH. Owner: HP (Amendment 1 follow-up). The content of `botster-agent.json` and `botster-accessory.json` is not defined, and "accessory" is not defined.**
- Evidence: HPA1:17 (PK-6) names the files and two schema artifacts, `agent-definition.schema.json` and `accessory.schema.json`; HPA1:35 adds them to Appendix B. Appendix B (HP:801 to HP:811) lists the artifact names of the base only, with a content line for each; the two new artifacts have no content line. PM-11 (HP:360) says only "They are data (session types and accessories)". No clause states the fields of an agent definition, how it maps to a `session_type` (HC:898 fields), where its script and companion files live, or what an accessory is.
- Why high: an agent-definition author and a catalog publisher cannot write a valid file. The 10 measured local agent definitions (CLI:16, `initialization` script plus `notes.md`) have no v1 form.
- Smallest amendment (HP, PK-6b): "`botster-agent.json` is one JSON object with exactly the fields of the client `session_type` create (HC A.9) except `source`, plus `script_file?` (a path relative to the package root whose content is the `script`). The package root is the session type's `session_type_dir` (HC MC-1c). `botster-accessory.json` is `<define>` or accessories are removed from v1." Plus the Appendix B content lines for both artifacts.

**G-5. HIGH. Owner: HP. Inbound webhook authentication has no wire format.**
- Evidence: NT-7, HP:283: "the Hub verifies a per-plugin webhook secret (a bearer token or an HMAC that the operator sets)" and "strips ... the `Authorization` header and the signature header". The signature header name, the HMAC algorithm, the signed bytes and the encoding are not named. The listener address is `net_listen` (HP:571) with no default port.
- Why high: an external sender (for example a GitHub webhook, which signs with `X-Hub-Signature-256: sha256=<hex>`) cannot produce a request that the Hub accepts.
- Smallest amendment (HP NT-7): "A request is authenticated by `Authorization: Bearer <secret>` or by `X-Botster-Signature-256: sha256=<lowercase hex HMAC-SHA256 of the raw body under the secret>`. The operator chooses the mode per plugin in the package configuration. The webhook URL is `http://<net_listen>/plugins/<plugin_key>/<path>`; `net_listen` defaults to `127.0.0.1:<port>` with `<port>` fixed in the Hub parameter table."

**G-6. HIGH. Owner: HP. A plugin's static origin (host and port) is not allocated by any rule, and its stability is not stated.**
- Evidence: WP-5, HP:327: "Each plugin is served from its own origin (a distinct host and port)" and "The listener binds where the operator configures it (`net_listen`, loopback by default)". WP-1, HP:323: `{served: true}` means "the plugin's own served origin". PR-0, HC:450: the pairing URL is `<web origin>/hubs/<hub_id>#<bundle>`. No clause says how the port is chosen, whether it is stable across a Hub restart, reload or upgrade, or where a client or an operator reads it (the `package` row, HC:208, has no origin field).
- Why high: browser storage and credentials are bound to the origin. If the port changes at restart, every paired browser loses its credential, earlier pairing links break, and an operator's tunnel or reverse proxy points at a dead port.
- Smallest amendment (HP WP-5): "The Hub allocates each plugin's static origin as `http://127.0.0.1:<port>` from the Hub parameter `static_port_range`, records it durably per package key (ST-9), and keeps it across restart, reload and upgrade until the package is removed. The `package` row shows it as `static_origin?` (operators only)."

### Medium

**G-7. MEDIUM. Owner: HC. The Hub's roots and the app name are defined only in the CLI spec.**
- Evidence: CLI:66 (CLI-RT1) app name `botster`; CLI:67 (CLI-RT2) config root and data root ("The data root is `BOTSTER_DATA_DIR`, else `BOTSTER_CONFIG_DIR`, else `~/.<app>`") and "named so that `botster config` and the Hub agree on it". HC TB-L1 (HC:420) uses `<app>` and `$BOTSTER_CONFIG_DIR` without defining `<app>` or the data root. HC:203 reports `data_root` but no HC clause says how the Hub derives it. The joint manifest calls the CLI spec "informative for the freeze".
- Why medium: test-Hub isolation through `BOTSTER_CONFIG_DIR` (HC:420 "this isolates a test Hub") works only if the Hub also derives its data root from it. No normative contract binds the Hub to that.
- Smallest amendment (HC, TB-L0): "The app name is `botster` in every build. The Hub's config root and data root follow CLI-RT2 exactly (`BOTSTER_CONFIG_DIR`, else the platform configuration directory plus `botster`; data root `BOTSTER_DATA_DIR`, else `BOTSTER_CONFIG_DIR`, else `~/.botster`). The Hub keeps all durable state under the data root."

**G-8. MEDIUM. Owner: HC. The single-instance lock has no stated scope or location.**
- Evidence: HS-0, HC:85: "A second instance is refused with a message naming the running Hub". CLI:77 "another instance holds its lock". DEP-11 (CLI:274) "released its singleton lock and removed its socket". HC:881 "it may include internals such as the lock file". The key of the lock (per data root, per socket path, per user, per machine) is not named.
- Why medium: with `BOTSTER_HUB_SOCKET` and `BOTSTER_CONFIG_DIR` overrides, a harness and an operator cannot know whether two Hubs on one data root, or on one socket, are refused.
- Smallest amendment (HC HS-0): "The single-instance lock is an exclusive advisory lock on `<data root>/hub.lock`. Its scope is the data root. A Hub refuses to start when the lock is held, and it binds its socket only after it holds the lock."

**G-9. MEDIUM. Owner: HC. The fallback runtime directory has no creation and ownership rule.**
- Evidence: TB-L1, HC:420: "`<runtime dir>` is `$XDG_RUNTIME_DIR` if set, else `/tmp/botster-<uid>`. The Hub creates the directory with mode 0700". It does not say which directory gets 0700 (`/tmp/botster-<uid>` or `<runtime dir>/botster`), what happens when `/tmp/botster-<uid>` already exists and belongs to another user, or whether a client checks the owner of the socket. TB-L3 (HC:431) checks only the client's uid.
- Why medium: on macOS `XDG_RUNTIME_DIR` is normally unset, so every Unix client uses `/tmp`. Another local user can create `/tmp/botster-<uid>` first and serve a fake Hub to the CLI.
- Smallest amendment (HC TB-L1): "The Hub creates `/tmp/botster-<uid>` and `<runtime dir>/botster` with mode 0700. If either exists and is not owned by the Hub's uid, or has a wider mode, the Hub refuses to start. A client refuses a socket whose directory is not owned by its own uid."

**G-10. MEDIUM. Owner: HC (with CLI). `BOTSTER_OFFLINE` is set by the CLI for the Hub but no Hub contract reads it.**
- Evidence: CLI:79 (CLI-ST4): "sets `BOTSTER_OFFLINE=1` in the environment of the Hub ... Its meaning ... belongs to the Hub and the cloud plugin". Absence: no `BOTSTER_OFFLINE` in HC or HP. HC P5 keeps cloud out of the Hub.
- Smallest amendment: either HC: "`botster-hub up` honors `BOTSTER_OFFLINE=1`: `check_update` answers `state: offline` and no update check runs at boot. Plugins see it as the read-only Hub fact `offline: bool`." or CLI: drop CLI-ST4's variable and keep `--offline` as a no-op.

**G-11. MEDIUM. Owner: HC (value) and Core (entry). The `TERM` value and the `TERMINFO` directory are not named.**
- Evidence: MC-1c, HC:518: "It sets `TERM` and `TERMINFO` to the terminfo entry of Core's emulator (a Hub parameter)". Core names no `TERM` value; HP:256 restates "with `TERM`, `TERMINFO`". No install path for the compiled entry.
- Why medium: an agent program, an `ssh` hop and a packager (who must ship the compiled entry) all depend on the exact name.
- Smallest amendment (HC MC-1c): "`TERM` is `<name>` (Core's published emulator entry, Core section <n>). `TERMINFO` is the install's terminfo directory (G-12). A Hub parameter may override both."

**G-12. MEDIUM. Owner: HC (packager-facing). The install layout and the binary names of the worker, the plugin hosts and the bridges are not named.**
- Evidence: Named: `botster`, `botster-hub up --ready-fd` (HC:85), `botster-tui` (CLI:83). The lookup rule "in the directory of the `botster` binary, else on `PATH`" covers the Hub and the TUI only (CLI:77, CLI:83). Unnamed: the session worker (Core:496 "passes its path"), the Lua plugin host (TR-1, HP:618 "shipped with the Hub"), `mcp-bridge` (HP:413) and `process-bridge` (HP:294). HC:203 `components` names them only as roles. HC:846 `update` "updates the whole install". MC-1c (HC:518) puts "the directory of its own `botster` CLI" on `PATH` but does not say how the Hub finds that CLI.
- Why medium: a packager (Homebrew, a tarball) cannot lay out an install that the Hub and `hub update` both accept.
- Smallest amendment (HC, IN-1): "An install is one directory `<bin>` that holds `botster`, `botster-hub`, `botster-tui`, `botster-worker`, `botster-lua-host`, `botster-mcp-bridge` and `botster-process-bridge`, and `<bin>/../share/botster/terminfo/`. Each binary finds the others in its own directory only. `hub update` replaces the set as one unit."

**G-13. MEDIUM. Owner: HP. The schema artifacts have no published location or `$id`, and a manifest cannot carry `$schema`.**
- Evidence: HP:803 "Each artifact is generated from the contract crate (`botster-plugin-contract`), checked in". No repository path, URL or `$id`. HPA1:15 (PK-4) "The Hub validates with this artifact". MN-1 (HP:840) is a closed key set, so an editor's `"$schema": "..."` line fails with `manifest_unknown_key`. By contrast HC:869 names `botster-hub-contract/schemas/v1/<kind>.json`.
- Why medium: plugin authors and catalog publishers need a stable schema URL for editor validation and CI.
- Smallest amendment (HP Appendix B): "Each artifact is published at `botster-plugin-contract/schemas/v<botster_plugin_api>/<artifact>` and carries `$id: https://<published host>/schemas/plugin/v1/<artifact>`. MN-1 and PK-6 accept and ignore an optional top-level `$schema` string."

**G-14. MEDIUM. Owner: HP. The catalog document has no schema artifact, no version value and no media type.**
- Evidence: PM-2, HP:351: "a JSON document `{catalog_version, entries: [{id, kind, name, description, source: {git url, ref?}}]}`". HPA1:30 adds `path`. The value of `catalog_version`, the uniqueness rule for `id`, the bounds of `name` and `description`, the URL scheme (HP:350 says `https` for git sources only) and the `Content-Type` are not stated. Appendix B (HP:801 to HP:811) has no `catalog.schema.json`.
- Smallest amendment (HP PM-2): "A catalog is served over `https` as `application/json`. `catalog_version` is the integer `1`. `id` is unique in the document and matches `[a-z0-9-]+`. The schema artifact `catalog.schema.json` (Appendix B) validates it; an unknown key is ignored, a document that fails the schema is `failed{detail.code: catalog_invalid}`."

**G-15. MEDIUM. Owner: HC (MC-1c) and HP (PK-6). `session_type_dir` has no stated value for a packaged agent definition or for a client-created session type.**
- Evidence: MC-1c, HC:518: "`session_type_dir` is the directory of the session type's definition files (legacy scripts call it `session_dir`)". CLI:114 depends on it for `botster context file notes.md`. HP has no definition (HP mentions it 0 times in a clause; DEP-4, CLI:268, records only 0016's acceptance). A device session type made by `create(session_type, { script })` (HC:898) stores a script body; no clause says which directory it gets, or where a user puts companion files.
- Smallest amendment (HC MC-1c): "For a session type from a package (HP PK-6), `session_type_dir` is the package root. For a session type created through `create(session_type)`, it is `<data root>/session_types/<id>/`, which the Hub creates with mode 0700; the operator may place companion files there."

**G-16. MEDIUM. Owner: HC. A repository-sourced session type has no stated storage location.**
- Evidence: HC:898 `source: "device" | { repo: { target_id } }`. No clause says whether a `repo` session type is written into the repository (and at which path) or kept by the Hub. Legacy repository agents lived in the repository so that teammates shared them.
- Smallest amendment (HC section 6, `session_type`): "A `repo` session type is a package of kind `agent_definition` discovered in `<target>/<target_plugin_dir>/<id>/botster-agent.json` (HP CP-8, PK-8). `create` with `source: { repo }` is refused `unsupported`; the file in the repository is the definition." (Or the opposite rule, stated.)

**G-17. MEDIUM. Owner: UI (URL form) and HC (`route_path`). The Web app URL space is not fixed.**
- Evidence: UI11:25 (UI-SL-6): "A Web client maps a target to the URL `…/<package>/<view>?<slot>=<row key>`": the prefix is an ellipsis. HC:924 `resolve_route` takes `{ route_path }` with no stated syntax. SF-5 notices carry `url?` (HC:355) that a push service worker opens. Pairing uses `<web origin>/hubs/<hub_id>` (HC:450); `/signal` and `/ice` are reserved below it (HC:459).
- Why medium: a plugin that builds a notice URL, a push handler and a second Web client cannot produce or parse a deep link.
- Smallest amendment (UI-SL-6 and HC A.9): "The Web URL of a navigation target is `<web origin>/hubs/<hub_id>/v/<package key>/<view>?<slot>=<row key>`. `route_path` is the part after `/hubs/<hub_id>`. The paths `/signal` and `/ice` are reserved."

**G-18. MEDIUM. Owner: HP. A package cannot serve its own files: a static route root is an operator-bound directory, so the Web app bundle has no defined home.**
- Evidence: WP-5, HP:327: "`static_routes: [{route, root, index?}]` ... with an `fs:<root>` grant for the `root` (FS-1)". FS-1, HP:303: "the operator binds each `id` to a real directory at enable". HC:450 says the web plugin may serve the Web app itself. So the Web app files must sit in a directory the operator binds by hand (and G-2 shows no verb for that binding).
- Smallest amendment (HP WP-5): "A static route `root` may be `package:<relative dir>`, a directory inside the package root. It needs no `fs` grant and no operator binding, and the PK-2 and PM-4 containment rules apply."

**G-19. MEDIUM. Owner: HP and Lua binding. The `require` name-to-file rule and the staged file set are not defined.**
- Evidence: MN-1, HP:840: `entrypoints` (`{service: {lua: <staged file>}}`). HP:622: "`require` serves only files staged at load". LB:76 "sibling modules staged at load". No clause says which files are staged (all `*.lua` under the package root? only siblings of the entry?), how `require("a.b")` maps to a path, or that the entry path is relative to the package root.
- Smallest amendment (LB, LB-17b): "The entry is a path relative to the package root. Every `*.lua` file under the package root is staged. `require("a.b")` loads `<package root>/a/b.lua`, else `<package root>/a/b/init.lua`; any other name is an error."

**G-20. MEDIUM. Owner: HC. The encoding of `push_public_key` is not stated.**
- Evidence: SF-5b, HC:355: "The `hub` row carries `push_public_key` (the Hub's VAPID key, RFC 8292)". HC:203 lists the field with no type. The Web client passes it to the browser as `applicationServerKey`.
- Smallest amendment (HC SF-5b): "`push_public_key` is the base64url (no padding) encoding of the uncompressed P-256 public point (65 bytes). The Hub keeps the private key under the data root; a new key invalidates every `push_subscription`."

**G-21. MEDIUM. Owner: CLI. The CLI log directory contradicts itself.**
- Evidence: CLI:57 (CLI-G3): "`<log dir>` is `$BOTSTER_CONFIG_DIR/logs` if that variable is set and `/tmp` if not". CLI:67 (CLI-RT2): "The CLI **writes** only under the config root (its logs, CLI-G3)". CLI:154 prints `log_dir`. Without the variable, the logs go to `/tmp`, which is not the config root.
- Smallest amendment (CLI-G3): "`<log dir>` is `<config root>/logs` (CLI-RT2)." Or change CLI-RT2 to say "under the config root or `/tmp`".

**G-22. MEDIUM. Owner: HP. A discovered target package has two different roots.**
- Evidence: PM-12, HP:361: "a `local` source that is that target's directory". HPA1:19 (PK-8): "every immediate subdirectory [of `target_plugin_dir`] that holds a package manifest file is a package root". The content hash (PM-3) and containment (PM-4) depend on which root is meant.
- Smallest amendment (HPA1 edit to PM-12): "The `local` source of a discovered instance is its package root of PK-8 (`<target>/<target_plugin_dir>/<subdir>`), not the target directory."

### Low

**G-23. LOW. Owner: HC.** The `failed <code>` codes of the ready line are not listed, and the line encoding (UTF-8, `LF`) is not stated. Evidence: HC:85 "`failed <code> <message>` with a typed `code`". Amendment: "The line is UTF-8 and ends with `LF`. `code` is one of `lock_held`, `config_invalid`, `socket_path_too_long`, `socket_unavailable`, `adoption_failed`, `internal`."

**G-24. LOW. Owner: HC.** The Hub's own log location is not named. Operators and bug reports need it. Evidence: absence in HC; CLI:57 covers the CLI only; HC:881 `diagnostics` is opaque. Amendment: "The Hub writes its log to `<data root>/logs/hub.log`; `diagnostics` names the path."

**G-25. LOW. Owner: HC.** The context file's directory and lifecycle are not stated (created where, deleted when, rewritten on restart or not). Evidence: HC:518 states only "a file readable only by the session's user" and "complete before the program starts". Amendment: "The file is `<data root>/sessions/<session_id>/context.json`, mode 0600, written once per run, kept until the session row is deleted."

**G-26. LOW. Owner: HC (with CLI).** The worktree base directory has no parameter name (legacy `worktree_base`, CLI:154), and the branch name `botster-issue-<N>` that CLI-WT2 (CLI:149) searches for is produced by no contract. Amendment: name a Hub parameter `worktree_base` (default `<data root>/worktrees`) in the G-1 table; mark the branch convention as the issue plugin's.

**G-27. LOW. Owner: HC.** The directory that Core's file input writes into (`file_directory`, Core:142, Core:167) is chosen by the Hub, but no HC clause names it. A user looking for an uploaded file has no rule. Amendment: "The Hub passes `<worktree or cwd>/.botster/uploads` (or a named parameter) as `file_directory`."

**G-28. LOW. Owner: HP.** `interfaces/<name>@<version>.json` (HP:921, LB:27) does not say "relative to the package root", and does not restate the allowed characters of `<name>`. Amendment: "at `<package root>/interfaces/<name>@<M.m.p>.json`".

**G-29. LOW. Owner: HP.** "Lockfile" (PM-3, HP:352; CLI:158 "a lockfile") names a durable Hub record, not a file. Users who know `lazy-lock.json` will look for one. Amendment: "The lock is Hub state, not an interchange file. v1 defines no exported lock file."

**G-30. LOW. Owner: HC.** The response body of `GET <web origin>/hubs/<hub_id>/ice` (HC:459, HP:328) is not given. Amendment: "The body is `{ servers: [{ urls, username?, credential? }] }`, as WP-6 stores it."

**G-31. LOW. Owner: CLI.** The CLI spec cites HC parity43 (CLI:15) while the package freezes parity45. Amendment: update the citation.

**G-32. LOW. Owner: UI.** The vector files live in "the crate's `vectors/`" (UI 1.0, `ui-contract-v1-frozen-20260930.md:238`) but the crate is not named. Amendment: name the crate.

**G-33. LOW. Owner: HC.** A remote `webrtc` client that is not a browser (a TUI or CLI on another machine, HC:489) has no named credential store. Only that client reads it, so this is one-party. Amendment (optional): "A non-browser client keeps its client key in `<config root>/credentials/<hub_id>.json`, mode 0600."

## Items that are complete

| Item | Where | Note |
|---|---|---|
| Plugin manifest file `botster-plugin.json`, package root, `path`, one manifest per root, validation order | HPA1:12 to HPA1:20; HCA1:11 to HCA1:25 | Name, location, size, codes, lifecycle are all stated. The two other file names are G-4. |
| Hub socket discovery, modes, path limit | HC:420 (TB-L1), HC:431 (TB-L3); CLI:68 | Order, env names, fallbacks, 0700 and 0600, 103 bytes. Ownership is G-9. |
| Session environment | HC:518 (MC-1c); HP:261 (CA-20) | Exact `BOTSTER_*` set, `port_env` validation, context file schema. TERM is G-11. |
| Ready descriptor | HC:85 (HS-0); CLI:77 | `--ready-fd <N>`, one line, EOF rule. Codes are G-23. |
| Pairing URL, signalling and ICE paths | HC:450, HC:452, HC:459; HP:323 to HP:328 | `<web origin>/hubs/<hub_id>#<bundle>`, `/signal`, `/ice`. |
| Asset URL shape and headers | HP:338 (AS-2), HP:339 (AS-3) | `/<plugin-key>/<asset_id>?t=<token>` on `assets_listen`. The listen value is G-1. |
| Webhook path | HP:283 (NT-7) | `POST /plugins/<plugin_key>/<path>`. Auth is G-5. |
| Core service environment and lane sockets | Core:434 (SV-1), Core:435 (SV-2) | `CORE_SERVICE_*`, `CORE_SERVICE_ENDPOINT/lane-<n>`, preamble. |
| HC kind schemas | HC:869 | `botster-hub-contract/schemas/v1/<kind>.json`, part of the tag. |
| CLI variables | CLI:57, CLI:67, CLI:68, CLI:148 | `BOTSTER_HUB_SOCKET`, `BOTSTER_CONFIG_DIR`, `BOTSTER_DATA_DIR`, `BOTSTER_LOG_FILE`, `BOTSTER_REPO`, `RUST_LOG`, `BOTSTER_ENV=test`. |
| Dev checkouts file | CLI:184 (CLI-DV4) | `<config root>/dev-checkouts.toml`, keys `hub`, `worker`, `plugin_hosts`, `mcp_bridge`, absolute paths. |
| MCP for agents | HC:517, HC:518; CLI:124 to CLI:129 | `BOTSTER_MCP_URL`, `BOTSTER_MCP_TOKEN`, bearer header, `botster mcp-serve` proxy. |
| Package keys and lock content hash | HP:144 (CP-8), HP:352 (PM-3) | `<name>@<target_id>`; path-sorted hash format. |
| Hub-internal stores (plugin store, lock records, pairing secret, client credentials, VAPID private key, Core registry) | HP ST-9; HC:450; Core:170, Core:275 | Correctly unnamed: no second party reads them. G-20 asks only for the public key encoding. |

## Method

- Read in full: CLI spec candidate 26, HP Amendment 1, HC Amendment 1, the joint manifest.
- Read by clause: HC sections 6, 9, 10, A.9, HS-0, TB-L1 to TB-L3, PR-0 to PR-4, MC-1 to MC-6, SF-3, SF-5b; HP CP-8, CP-9, CA-13, CA-14, CA-20, NT-5, NT-7, FS-1 to FS-3, WP-1 to WP-6, AS-1 to AS-4, PM-1 to PM-14, section 15, TR-1, MP-1, MP-3, Appendix B, MN-1; Core SV-1, SV-2, DP-5b; UI-SL-6, UI-EM rules.
- Grep over all 11 files for `BOTSTER_*`, `CORE_*`, file extensions, `config`, `Hub parameter`, `data_root`, `TERM`, `TERMINFO`, `VAPID`, `/hubs/`, `lock`, `log`, `.botster`, `session_type_dir`, `require`, `accessory`. Absence claims above rest on these whole-file greps.
