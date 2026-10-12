# Hub→client contract v1: Amendment 2 (candidate 6): completeness

Owner: plan auditor. Reviewer: Sol Hub A. Date: 2026-09-30. **This file is the amendment, in the post-freeze amendment path.** The base is the frozen `hub-client-contract-v1-candidate-parity45.md` plus Amendment 1 (`hub-client-contract-v1-amendment-1-candidate5.md`). Both stay unchanged. The joint manifest lists the base and the amendments together. This amendment cites Core, HP (with HP Amendment 2) and the ui-contract by contract name and clause id only.

**Why:** the completeness sweep (`audits/completeness-sweep-20260930.md`) asked two questions of the frozen package. (A) Where does each name, path and setting live? (B) Is every command fully specified? This amendment closes every HC gap that the sweep found: G-1, G-2 (client verbs), G-7 to G-12, G-15, G-16, G-17, G-20, G-23 to G-27, G-30 and G-33 of part A, and H-1 to H-34 of part B1. It also pins the route codec revision 30, which closes R-1 to R-10.

---

## Part 1. Where it lives

### New clauses

| # | Clause | Test |
|---|---|---|
| RT-1 | **App name and roots** (G-7; CLI-RT1 and CLI-RT2 become normative here, and the CLI spec cites this clause). The app name is `botster` in every build, release or development. **The config root** is `BOTSTER_CONFIG_DIR`, else the platform configuration directory plus `botster` (macOS: `~/Library/Application Support/botster`; Linux: `$XDG_CONFIG_HOME/botster`, else `~/.config/botster`). **The data root** is `BOTSTER_DATA_DIR`, else `BOTSTER_CONFIG_DIR`, else `~/.botster`. The Hub, the CLI and every local client derive both roots by this rule. The Hub keeps all its durable state under the data root. | `conf::rt_1_roots_precedence`, `conf::rt_1_test_hub_isolated_by_config_dir` |
| RT-2 | **Data-root layout** (G-8, G-24, G-25, G-27). The Hub creates these, each with mode 0700 (a file 0600), and owns them:<br>- `hub.lock`: the single-instance lock (RT-4);<br>- `store/`: the Hub's durable store (plugin stores, package locks, assets, operation records, credentials, the VAPID private key; its internal format is the Hub's);<br>- `packages/`: installed package files (HP PM-4);<br>- `logs/hub.log`: the Hub's own log (`diagnostics` names its path);<br>- `terminfo/`: the compiled terminfo entry (MC-1c);<br>- `sessions/<session_id>/`: one directory per session row. It holds `context.json` (the MC-1c context file, written once per run, mode 0600) and `uploads/` (the TS-5f file directory that the Hub passes to Core as `file_directory`). The directory is deleted with the session row.<br>**Access:** the Hub owns the data root, and a client uses the verbs, not these files, **with exactly these authorized exceptions**:<br>- the session's own program reads its `sessions/<session_id>/context.json` through `BOTSTER_CONTEXT_FILE` (MC-1c), and `botster context` reads it the same way;<br>- the session worker writes `sessions/<session_id>/uploads/` (TS-5f; the Hub passes the directory to Core);<br>- the session's program may read its own `uploads/` files, whose paths it received;<br>- **the CLI's own files are never under the data root's Hub entries:** CLI logs go to `<config root>/logs/cli/` and CLI state to `<config root>/cli/`. When the config root and data root are the same directory, the Hub's entries listed above and the CLI's `logs/cli/` and `cli/` do not overlap. The CLI holds no Hub credential file (a local client authenticates by socket peer uid, TB-L3).<br>No other party writes under the Hub's entries. | `conf::rt_2_layout_created_with_modes`, `conf::rt_2_session_dir_deleted_with_row`, `conf::rt_2_authorized_access_exceptions_only`, `conf::rt_2_cli_files_disjoint_from_hub_entries` |
| RT-3 | **Install layout** (G-12). An install is one directory that holds `botster`, `botster-hub`, `botster-tui`, `botster-worker`, `botster-plugin-host`, `botster-mcp-bridge` and `botster-process-bridge`. **Each binary finds the others only as siblings in its own directory**; there is no `PATH` fallback. `hub update` replaces the set as one unit. MC-1c's "the directory of its own `botster` CLI" is this install directory. | `conf::rt_3_siblings_only_no_path_fallback` |
| RT-4 | **Single-instance lock** (G-8). The lock is an exclusive advisory lock on `<data root>/hub.lock`. **Its scope is the data root:** two Hubs with different data roots may run. A Hub takes the lock before it binds its socket, and it refuses to start when the lock is held (HS-0 `failed lock_held`). It releases the lock only after it removed its socket (HS-9 `lock_released: true`). | `conf::rt_4_second_hub_same_root_refused`, `conf::rt_4_lock_before_socket` |
| RT-5 | **Runtime directory and socket safety** (G-9). The Hub creates `/tmp/botster-<uid>` (when `XDG_RUNTIME_DIR` is unset) and `<runtime dir>/botster` with mode 0700. If either exists and is not owned by the Hub's uid, or has a wider mode, the Hub refuses to start (HS-0 `failed socket_unavailable`). **A client checks, before it connects, that the socket and its directory are owned by its own uid**, and refuses otherwise. Another local user therefore cannot serve a fake Hub. | `conf::rt_5_foreign_runtime_dir_refused`, `conf::rt_5_client_checks_socket_owner` |
| CF-1 | **The Hub configuration file** (G-1, G-10, G-26). The Hub reads its parameters from `<config root>/hub.toml` (TOML 1.0, UTF-8). The file is optional; a missing key takes its documented default. The operator creates and edits it, and the Hub never writes it. **The Hub refuses a file that another user can write.** Each parameter is a key under a table: the HC keys are listed in CF-2, and the HP keys in HP Amendment 2 section 3 (each with its table and its live or restart status). **An unknown key, a value of the wrong type or a value out of range** makes `botster-hub up` write `failed config_invalid <key>` (or `failed config_unknown_key <key>`) to the ready descriptor (HS-0), and exit nonzero. The Hub reads the file at start. **`invoke(hub, id, reload_config)`** (committed) re-reads it. It applies the changed **live** keys, does not apply a changed **restart-only** key, and emits `hub/config.changed { keys, restart_required: [key] }` (HP A2-52). A file that fails validation at reload is refused with `invalid_arguments { reason: config_invalid, detail: { key } }`, and nothing changes. `invoke(hub, diagnostics)` reports the effective values. **`BOTSTER_OFFLINE=1`** in the Hub's environment sets `[hub] offline = true` for that run. | `conf::cf_1_unknown_key_refuses_start`, `conf::cf_1_reload_applies_live_reports_restart_required`, `conf::cf_1_world_writable_file_refused`, `conf::cf_1_offline_env` |
| CF-2 | **HC parameters in `hub.toml`.** Each row is the key path, type, default and when it applies. Section 15 of HP lists the HP keys. | `conf::cf_2_every_hc_parameter_has_a_key` |

**CF-2 table**

| Key | Type | Default | Applies |
|---|---|---|---|
| `[hub] offline` | bool | `false` | restart. When true, no update check runs, and `check_update` answers `failed { reason: offline }`. |
| `[hub] log_level` | `debug \| info \| warn \| error` | `info` | live |
| `[limits] max_frame_bytes`, `max_chunk_bytes`, `max_outstanding`, `max_watches` (at least 32), `route_connect_seconds`, `idempotency_window_seconds`, `cursor_idle_seconds`, `operation_retention_seconds` | integers | the Hub's defaults, reported in `welcome.limits` (HS-3) | restart |
| `[routes] max_paste_bytes`, `max_file_bytes`, `query_client_deadline_ms`, `stall_deadline_ms`, `stall_close_after_ms` | integers | the Hub's defaults, reported per route in `route_limits` (TS-1) | live, for new routes |
| `[sessions] activity_idle_ms` | integer | `3000` | live |
| `[sessions] worktree_base` | absolute path | `<data root>/worktrees` | live, for new worktrees |
| `[sessions] term` | string | Core's emulator entry name (Core names it) | restart |
| `[agents] scopes` | list of permission names and `*` patterns | every client permission (MC-1b) | live |
| `[audit] retention_days` | integer | `30` | live |
| `[audit] max_bytes` | integer | `67108864` (64 MiB) | live |
| `[network] trusted_hosts` | list of `{ path, sha256 }` | empty | restart (HP TR-1) |
| `[update] check_on_start` | bool | `true` | restart |

### Edits to existing clauses (part 1)

| Clause or row | The edit |
|---|---|
| HS-0 | The ready line is UTF-8 and ends with `LF`. In `failed <code> <message>`, `code` is one of `lock_held`, `config_invalid`, `config_unknown_key`, `socket_path_too_long`, `socket_unavailable`, `adoption_failed`, `internal` (non-exhaustive) (G-23). **The ready descriptor goes only to the process that started the Hub.** |
| TB-L1 | `<app>` is `botster` (RT-1). The directory creation and owner checks are RT-5. |
| MC-1c | **`TERM`** is the value of `[sessions] term` (default: Core's emulator entry name), and **`TERMINFO`** is `<data root>/terminfo`, which the Hub populates from the entry Core ships (G-11). **The context file** is `<data root>/sessions/<session_id>/context.json` (RT-2; G-25). **`session_type_dir`** (G-15): for a session type that a package of kind `agent_definition` provides, it is that package's root (HP A2-39 to A2-51); for a session type created through `create(session_type)`, it is **absent**, and the context file omits it. **`PATH`** gets the install directory (RT-3) first. |
| Section 6, `session_type` row, `source` | G-16: a `{ repo: { target_id } }` session type is an `agent_definition` package discovered in the target (HP CP-8, PK-8, A2-39 to A2-51). The file in the repository is its definition. `create(session_type, { source: { repo } })` is refused with `unsupported { reason: repo_source_is_discovered }`; a client edits the file in the repository and then calls `rescan_packages`. |
| SF-5b, `push_public_key` | G-20: it is the base64url encoding (no padding) of the uncompressed P-256 public point (65 bytes). The private key is in the Hub's durable store (RT-2). A new key pair invalidates every `push_subscription`. |
| PR-4, the ICE endpoint | G-30: `GET <web origin>/hubs/<hub_id>/ice` answers `application/json` `{ servers: [{ urls: [string], username?, credential? }] }`, as HP WP-6 stores it. |
| PR-3 | G-33: a client that is not a browser keeps its client key in `<config root>/credentials/<hub_id>.json`, mode 0600. Only that client reads the file. |
| Section 6, `navigation` row, and A.9 `resolve_route` | G-17 (ui-contract UI-SL-9): the Web URL of a navigation target is `<web origin>/hubs/<hub_id>/v/<package key>/<view>?<slot>=<row key>`. **`route_path`** is the part after `/hubs/<hub_id>`: `/v/<package key>/<view>`, with an optional query `?<slot>=<row key>`, each part percent-encoded. The paths `/signal` and `/ice` are reserved. `resolve_route { route_path }` answers `{ target: { kind: view, id, select? } }`, or `not_found` for an unknown path. |
| Section 6, `catalog` row | The row's fields are exactly `id`, `url`, `refreshed_at?` and `entries` (CLI OI-9). |

### New client verbs for operator settings (G-2; HP A2-51 uses the same names)

| Kind | Action | Arguments | Result | Effect | Specific failures |
|---|---|---|---|---|---|
| `package` | `enable` (extended) | `{ fs_roots?: { <root_id>: <absolute path> } }` | `{ operation }` | accepted (not durable) | `invalid_arguments { reason: fs_root_unknown \| path_not_absolute \| path_not_found }`; `wrong_state { reason: fs_root_unbound }` when a declared root has no binding |
| `package` | `update` (configuration) | the `configuration` merge patch may set `webhook_secret`, a secret field (SF-3: never returned; shown as `set: bool`) | the package | committed | `invalid_arguments` (schema) |
| `spawn_target` | `rescan_packages` | `{}` | `{ operation }`; the operation ends `succeeded { found: [package key], removed: [package key] }` (the instances the rescan found, and those it removed) or `failed { reason }` (HP A2-51(d)) | accepted (not durable) | `wrong_state { reason: not_a_directory }` |
| `session_type` | `create` / `update` | adds `agent_scopes?: [permission]` | the session type | committed | `invalid_arguments { reason: scope_widens }` when it names a scope outside `[agents] scopes` |
| `session` | `create` | adds `agent_scopes?: [permission]` | the receipt | accepted (durable) | `invalid_arguments { reason: scope_widens }` |
| `hub` | `reload_config` | `{}` | `{ applied: [key], restart_required: [key] }` | committed | `invalid_arguments { reason: config_invalid, detail: { key } }` |

`agent_scopes` only narrows: the effective scope set of an agent is the intersection of `[agents] scopes`, its session type's `agent_scopes` and its session's `agent_scopes` (MC-1b). The TR-1 trusted hosts are the `[network] trusted_hosts` key (CF-2).

---

## Part 2. Every command fully specified

### New clauses

| # | Clause | Test |
|---|---|---|
| A.1b | **Tagged values and error data** (H-1; codec revision 30 A.1 uses the same rule). In JSON, an enum value with no data is a string (`"running"`). An enum value with data is an object with exactly one key, the value name, whose value is the data object (`{ "failed": { "reason": "fetch_failed" } }`, `{ "degraded": { "interface": "x" } }`). **`Error` gains `detail?: object`.** Every datum that this contract names next to an error code (for example `sessions`, `unknown`, `candidates`, `consumers`, `interface`, `range`, `found`, `name`, `key`, `code`) is a member of `detail`. `reason` stays a string. | `conf::a_1b_tagged_value_round_trip`, `conf::a_1b_error_detail_members` |
| A.7b | **Operations** (H-2, H-17, H-29). The `{ operation }` result of every accepted action is **the full `operation` row at admission**. Session create keeps its own receipt `{ operation_id, session, state }`. Each kind, its states, its `failed.reason` set (non-exhaustive), its `progress` and its `result`:<br>- generic kinds use `queued`, `running`, `succeeded`, `failed { reason }`, `cancelled`; `progress` is absent or `{ message, fraction? }` (`fraction` in 0..1);<br>- `spawn` (create and restart): `starting`, `installed`, `failed { reason: exec_failed \| cwd_missing \| worktree_failed \| worker_failed \| session_type_unavailable }`, `cancelled`; `result` is the session ref;<br>- `safe_write`: as SW-2 (HP);<br>- `hub.update`: `result { version }`; `failed { reason: download_failed \| verify_failed \| install_failed }`;<br>- `hub.shutdown`, `hub.restart`: **ended by `close { hub_shutdown, lock_released }` (HS-9)**; a client never waits for their terminal state;<br>- `hub.swap`: ended as `hub.shutdown`, after the HS-9 close;<br>- `hub.upgrade_packages`, `hub.rollback_packages`: `result { packages: [{ package, state: succeeded \| failed, reason? }] }`;<br>- `session.stop`, `worktree.create` (`failed { reason: git_failed }`), `worktree.delete`, `package.install` (`failed` with the HP PM-4, PD-6, PK-3 and PKC-1 reasons, plus `fetch_failed`, `url_refused`, `too_large`, `hash_mismatch`), `package.remove`, `package.enable`, `package.disable`, `package.reload`, `package.entrypoint`, `package.upgrade` (`result { package, version, resolved_commit }`; `failed { reason: fetch_failed \| hash_mismatch \| activation_failed }`), `package.rollback` (`result { package, ref, resolved_commit }`; `failed { reason: hash_mismatch }`), `catalog.refresh` (`failed { reason: fetch_failed \| url_refused \| malformed \| too_large }`), `spawn_target.rescan_packages` (`succeeded { found, removed }`). | `conf::a_7b_operation_row_at_admission`, `conf::a_7b_rescan_operation_result_found_and_removed`, `conf::a_7b_every_kind_states_closed`, `conf::a_7b_shutdown_ended_by_close` |
| A.9b | **Specific failures of each action** (H-10; the table below). Every request may also fail with the **common failures**: `invalid_arguments` (schema), `permission_denied`, `unsupported`, `not_found` (the target), `remote_hub_unsupported`, `too_many_requests`, `busy`, `response_too_large`, `hub_shutdown`, `cancelled`, `internal`, and, for a plugin kind or a contributed action, `stale_descriptor` and `plugin_failed`. **A missing resource named in the arguments** (for example `session_type_id`, `target_id`, `worktree_id`, `catalog_id`, `entry_id`, `entrypoint_id`, `route_id`, a `ref`) is `invalid_arguments` with a `field_errors` entry whose `code` is `not_found` and whose `pointer` names the argument (H-11). A refusal before dispatch has `effect: none` (section 11). | `conf::a_9b_<kind>_<action>_<reason>` (one per row and reason) |
| H-18 | **Hub-wide operations do not overlap.** While `swap`, `update` or a bulk package operation (`upgrade_packages`, `rollback_packages`) runs, a second one of these, and `shutdown` and `restart`, are refused with `wrong_state { reason: swap_in_progress \| update_in_progress \| package_operation_in_progress }` and `effect: none`. A package action on a package that the running bulk operation covers is refused the same way. | `conf::h_18_second_hub_operation_refused` |

**A.9b table.** "Covered" rows cite the clause that already states the failure.

| Kind | Action | Specific failures (code { reason }) | Effect |
|---|---|---|---|
| `hub` | `check_update` | none: a failed check is the result `state: failed { reason }` (H-3) | read |
| `hub` | `update` | `wrong_state { up_to_date \| offline \| dev_build \| update_in_progress \| swap_in_progress }`; operation failures in A.7b | accepted, durable |
| `hub` | `shutdown`, `restart` | `wrong_state { swap_in_progress \| update_in_progress \| package_operation_in_progress }` | accepted, not durable |
| `hub` | `reload_config` | `invalid_arguments { config_invalid }` (CF-1) | committed |
| `hub` | `diagnostics`, `preview_package_upgrades` | common only | read |
| `hub` | `swap_preflight` | `invalid_arguments { unknown_worker_protocol }` (H-26) | read |
| `hub` | `swap` | covered: `wrong_state { unadoptable_sessions }` with `detail { sessions, unknown }`; `wrong_state { swap_in_progress \| update_in_progress \| package_operation_in_progress }`; `invalid_arguments { unknown_worker_protocol }` | accepted, not durable |
| `hub` | `issue_pairing` | covered: `wrong_state { no_web_origin }`; `invalid_arguments { ttl_out_of_range }` for a `ttl_seconds` that is not a positive integer. The Hub caps a larger value at its own maximum and reports the applied value as `expires_at` | immediate |
| `hub` | `upgrade_packages` | covered: `conflict { requirement_conflict \| ambiguous_provider \| requirement_missing }`; `conflict { breaking_not_accepted }` with `detail { consumers }`; `wrong_state { package_operation_in_progress }` | accepted, durable |
| `hub` | `rollback_packages` | `wrong_state { no_history \| package_operation_in_progress }`; per-package outcomes in `result` (A.7b) | accepted, durable |
| `session` | `create` (type) | covered: `wrong_state { swap_in_progress }`, `internal { credential_unavailable }`, `invalid_arguments { target_mismatch }`; `wrong_state { session_type_unavailable }` (`resolve.available` is false); `invalid_arguments { scope_widens }` | accepted, durable |
| `session` | `create` (command) | covered: `permission_denied` (`session.create_command`); `invalid_arguments { argv_empty \| cwd_not_allowed }`; `wrong_state { swap_in_progress }` | accepted, durable |
| `session` | `update` | covered: `conflict` (`if_rev`), `invalid_arguments` (identity fields). Allowed in every state (H-30) | committed |
| `session` | `delete` | covered: `wrong_state` for `starting`, `running`, `stopping` (A.2c) | committed |
| `session` | `attach` | covered: `unsupported { binding_auth_unavailable \| terminal_format }`; before the reservation (H-4): `wrong_state { session_not_running }` (`ended`, `lost`), `not_found { route }` (a `replace_route` that is not a live route of this caller on this session), `unsupported { binding }`, `busy`. `attach_failed` arrives on the route only when the session ends or the screen fails after the reservation | immediate |
| `session` | `attach_offer` | H-5: `permission_denied { bad_signature }`; `not_found { route }` (unknown, or another caller's); `wrong_state { route_connected \| route_expired \| not_webrtc_route }`; `busy`. Each leaves the reservation as it was, except `route_expired` | immediate |
| `session` | `detach` | `not_found { route }` (unknown, or another caller's) | immediate |
| `session` | `stop` | `wrong_state { not_running }` for `ended` and `lost`; on `stopping`, answers the existing operation | accepted, not durable |
| `session` | `restart` | covered: `wrong_state { not_ended \| not_restartable \| record_unavailable \| environment_not_retained \| not_ready \| swap_in_progress }` | accepted, durable |
| `session` | `read_screen` | `wrong_state { no_screen_model }` only when the session has no model. A running session, or an ended one whose model is still kept (until it is removed), is read normally (frozen A.9; HP SA-1, A2-30) | read |
| `session` | `clear_attention` | none: idempotent, `ok` with no attention set | committed |
| `session` | `safe_write` | by the section 11 extension (H-9): `wrong_state { session_ended }`, `invalid_arguments { text_too_long \| text_empty }`, `busy`; outcomes by operation (HP SW-2) | accepted, not durable |
| `session` | `write` | by the section 11 extension (H-9): `wrong_state { session_ended }`, `busy` (lane full), `invalid_arguments { bytes_too_long \| bytes_empty \| bytes_invalid }` | immediate |
| `session_type` | `create`, `update` | `conflict` (an id that exists, create); `invalid_arguments { port_env_invalid \| scope_widens }`; `wrong_state { not_editable }` (update); `unsupported { repo_source_is_discovered }` | committed |
| `session_type` | `delete` | `wrong_state { not_editable }`. Current sessions keep running | committed |
| `session_type` | `resolve`, `definition` | common only (a missing `target_id` follows H-11) | read |
| `spawn_target` | `create`, `update` | `invalid_arguments { path_not_absolute \| path_not_found \| not_a_directory }`; `conflict { path_exists }` | committed |
| `spawn_target` | `delete` | `wrong_state { in_use }` (worktrees or current sessions) | committed |
| `spawn_target` | `validate` | none: problems are in the result; `problems[].code` ∈ `path_missing`, `not_a_directory`, `not_git`, `unreadable` (non-exhaustive) | read |
| `spawn_target` | `browse` | `invalid_arguments { path_not_absolute }`; `not_found { path }`; `wrong_state { not_a_directory \| unreadable }` | read |
| `spawn_target` | `rescan_packages` | `wrong_state { not_a_directory }` | accepted, not durable |
| `worktree` | `create` | `wrong_state { not_git }`; `conflict { branch_checked_out }`; operation `failed { git_failed }` | accepted, durable |
| `worktree` | `delete` | covered: `wrong_state { dirty }` (unless `force`), `wrong_state { occupied }` | accepted, durable |
| `package` | `create` (install) | covered: `invalid_arguments { path_invalid }`, operation `failed` (A.7b); `conflict { already_installed }` | accepted, durable |
| `package` | `delete` (remove) | none: the consumers that require its interfaces become `degraded { interface }` (HP PD-8) | accepted, durable |
| `package` | `update` (configuration) | covered: `invalid_arguments` (schema, `field_errors`) | committed |
| `package` | `enable` | `wrong_state { staged \| quarantined \| errored \| fs_root_unbound }`; `invalid_arguments { fs_root_unknown \| path_not_absolute \| path_not_found }`. Already enabled: `ok`, with no change | accepted, not durable |
| `package` | `disable` | none: already disabled is `ok` | accepted, not durable |
| `package` | `reload` | `wrong_state { disabled }` | accepted, not durable |
| `package` | `start_entrypoint`, `stop_entrypoint`, `restart_entrypoint` | `wrong_state { disabled }`; an unknown `entrypoint_id` follows H-11 | accepted, not durable |
| `package` | `preview_upgrade` | `wrong_state { local_source }`; `wrong_state { fetch_failed }` (retryable) | read |
| `package` | `upgrade` | H-6: `conflict { breaking_not_accepted \| ambiguous_provider \| requirement_missing \| requirement_conflict }` with `detail { consumers \| candidates }`, `effect: none`; `wrong_state { pinned \| local_source \| up_to_date \| package_operation_in_progress }`; operation failures in A.7b | accepted, durable |
| `package` | `pin` | `wrong_state { local_source }`; an unknown `ref` follows H-11 | committed |
| `package` | `unpin` | none: not pinned is `ok` | committed |
| `package` | `rollback` | `wrong_state { no_history \| local_source }`; operation failures in A.7b; it ends `staged` when the rolled-back version needs a new approval (HP PM-9) | accepted, durable |
| `package` | `approve_grants` | `conflict { grants_mismatch }` (the set is not exactly the pending set); `wrong_state { nothing_pending }` | committed |
| `package` | `open_asset` | covered: `not_found` for a foreign asset; also `not_found` for a disabled, quarantined or removed package, and for a staged name that the new version dropped (HP AS-4); `wrong_state { assets_origin_unconfigured }`; `invalid_arguments` when both or neither of `asset_id` and `name` are given | immediate |
| `catalog` | `create` | `invalid_arguments { url_not_https }`; `conflict` (the URL is already added). It does not fetch: the first `refresh` does | committed |
| `catalog` | `delete` | none: installed packages keep their lock | committed |
| `catalog` | `refresh` | operation failures in A.7b | accepted, not durable |
| `quarantine` | `resolve` | none: `release` while the cause still holds answers `ok`, and the package is quarantined again | committed |
| `navigation` | `resolve_route` | `not_found` (an unknown `route_path`) | read |
| `client_credential` | `delete` | none. For the caller's own credential, the Hub sends `res { ok: {} }`, then closes with `close { auth_failed }` (H-28) | committed |
| `route` | `delete` | `not_found` for an unknown route or another caller's route | immediate |
| `push_subscription` | `create` | `invalid_arguments { endpoint_not_https }`; `conflict` (the endpoint is already registered) | committed |
| `push_subscription` | `delete` | `not_found` for another credential's subscription | committed |
| any plugin kind | declared actions | the declared `reasons`; plugin failures by the section 11 mapping | as declared |

### Edits to existing clauses (part 2)

| Clause or row | The edit |
|---|---|
| Hub row `update`, and A.9 `check_update` (H-3) | `update` is `{ state: idle \| checking \| available \| downloading \| installing \| failed { reason }, available?: { version } }`. `check_update` answers `{ state: up_to_date \| ahead \| available \| failed { reason: check_failed \| offline \| dev_build }, available?: { version } }`. `ahead` means that the running version is newer than the latest release. `check_update` is a read; it never fails because of the network. **After the Hub restarts itself** (`update`, `restart`), a client sees the outcome by reconnecting: it repeats TB-L1 discovery and the handshake until `welcome` arrives. The contract sets no total deadline (a client may stop on a user interrupt). Each attempt is bounded by the existing liveness probe (CLI-RT4, the legacy 500 ms bound). Then the client calls `get(operation, id)`. The update operation is durable (A.7b). HS-0's ready descriptor goes only to the parent process, and a client is not the parent of a Hub that restarted itself. |
| Section 6, `worktree` and `spawn_target` rows (H-7, H-34) | `worktree.state` ∈ `creating`, `ready`, `deleting`, `failed`, `missing` (the directory is gone). `spawn_target.kind` ∈ `directory`, `git` (non-exhaustive); it is also a create input. The client row names are normative for clients; HP's row names differ by design (`path` is HP `root`, `state` is HP `status`), and the Hub maps them. |
| MC-2 (H-8) | `whoami {}` answers `{ caller: SessionRef, scopes: [permission] }`. The `delete` tool is `delete { ref, args?, idempotency_key?, operation_id? }`, where `args` is as the kind declares in A.9. |
| VB-5 (H-12) | `delete(kind, target, args?)`: `args` as the kind declares in A.9 (`package { purge? }`, `worktree { force? }`). |
| DS-1 (H-13) | `reasons: [{ code, message }]`, where `code` is a section 11 code or a kind `reason`. |
| Section 8 (H-14) | `watch(event, …)` answers `{ stream_id }`. |
| A.7 and A.9 (H-15) | `push_subscription create { endpoint, keys: { p256dh, auth } }` answers the row without the endpoint (committed). `push_subscription delete` (committed). |
| Section 6, `spawn_target` row (H-16) | invoke `validate`, `browse` (operator permission `spawn_target.browse`), `rescan_packages`. |
| Section 11 (H-9) | The plugin-failure mapping table also maps the refusals of the Hub primitives that a client reaches through `session safe_write` and `write`: `failed { wrong_state: r }` becomes `wrong_state { reason: r }`; `invalid_request { r }` becomes `invalid_arguments { reason: r }`; `backpressured` becomes `busy`; each with `effect: none`. |
| Section 6, `quarantine` row (H-19) | `reason` ∈ `crash_loop`, `hash_mismatch`, `admission_failed`, `bootstrap_failed` (non-exhaustive). |
| Section 6, `session` row (H-20) | `failure_reason` ∈ `spawn_failed`, `exec_failed`, `cwd_missing`, `worker_lost` (non-exhaustive). |
| A.9 `session_type create` (H-21) | `command` is a string: an executable path, or a name found on the session's `PATH`. `args?` is `[string]`. `interaction?` ∈ `interactive` (default), `noninteractive` (HP CA-18). `lifecycle?` ∈ `restartable` (default), `one_shot`; it sets the row's `restartable`. `role?` is a string label. `traits?` is `[string]`. |
| A.9 `swap`, `swap_preflight` (H-26) | `target_worker_protocol` is a u16. |
| A.9 `read_screen`, `resolve`, `validate` (H-31) | `history_unavailable?` is a history `unavailable` reason (A.2b). `diagnostics?` is `[{ code, message }]`. `problems[].code` is as the A.9b `validate` row. |
| VB-1 (H-32) | `limit` is a positive integer. When it is absent or larger than the Hub's page maximum, the Hub uses its page maximum and returns that many items or fewer; a non-positive `limit` is `invalid_arguments`. No new number is introduced. |
| Section 6, `plugin_log` row (H-33) | `level` ∈ `debug`, `info`, `warn`, `error` (HP `log.write`). |
| TS-1, TS-5b, TS-5d, TS-5f, TS-8, A.1, A.2b, A.4, A.5, A.6 and the P9 binding | As route codec revision 30 (`route-codec-v1-extract-rev30.md`, which this amendment pins by hash in the joint manifest): `attach` gains `writable?`; the reply from a wrong route is `wrong_route`; the paste and file sequencing reasons; the `route_closed` exit location; a resync baseline without history; the `attach_failed` reasons `snapshot_too_large` and `bad_peer`; tagged values (R-1 to R-10). |

### New conformance tests (part 2)

`conf::h_3_check_update_states`, `conf::h_3_update_outcome_after_self_restart`, `conf::h_7_worktree_state_values`, `conf::h_8_whoami_result`, `conf::h_8_mcp_delete_args`, `conf::h_9_safe_write_refusal_mapping`, `conf::h_11_missing_argument_ref_is_field_error`, `conf::h_14_event_watch_answers_stream_id`, `conf::h_15_push_subscription_create_delete`, `conf::h_28_own_credential_delete_res_then_close`, `conf::h_32_list_limit_bounds`.
