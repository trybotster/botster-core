# hp-plugins notes: HP transcripts for plugin calls, assets, MCP tools, MCP proxies and package management

Branch `foundation/hp-plugins`. The list is `~/botster-sessions/shared/foundation/ids-hp-plugins.txt` (193 ids). The pin splits it in two READY batches: batch 1 (PC, AS, MT: 69 ids) and batch 2 (PM, MP: 124 ids). No id of the list had a transcript on main when batch 1 started. Both players (FakeHub, FakePlugin) are unchanged: 0 lines of fake growth.

## Batch 1: PC (31), AS (28), MT (10)

All 69 are `fake: structural`, ruling R-1.3: every outcome is a decision of the real Hub (the wait-for graph, the idempotency table, the asset store and its origin, the MCP lists). Each has full steps for the real Hub (stimuli, observers and an assertion that cites a clause sentence) and its full run happens against the real Hub in Stage 2. The frames and the host calls follow Appendix A (`calls_and_scripts_follow_appendix_a`). The proof kinds are those of `replacement-map.json`; the slow ids (`pc_4_pending_record_fsynced_before_dispatch`: fsync, `as_2_https_app_embeds_loopback_assets_origin` and `as_2_private_network_access_header_for_the_app_origin_only`: a real browser) name the real control that proves them.

### Prior art (BUILD.md rule 0)

- Vault: `botster-v1-plugin-contracts` and the plugin call lane note of hp-calls-a; nothing in the vault states a behavior of plugin calls, assets or MCP lists beyond the contract. No old code was read (the contract is the source), so no `Stolen-From` commit.
- Repo: the 145 transcripts of hp-calls-a. Reused: the Hello, Ready, Activated and invoke preamble, the `second` plugin pattern (`load_plugin`), the reload pattern of `ld_9_*` (candidate `cand`), the restart pattern of `ca_16_notification_survives_plugin_restart_via_get`, `await_operation`, `settle` as the barrier of a negative assertion.
- Hand-rolled with reason: one generator (`~/botster-sessions/shared/foundation/scratch-hp-plugins/`: `lib.py`, `gen_pc*.py`, `gen_as*.py`, `gen_mt.py`; scratch, not in the repo). It only avoids 69 copies of the preamble. The JSON in `conformance/hp/` is the source of truth.

### New real-Hub controls (`HUB_CONTROLS`, table in the doc comment)

`plugin_identity`, `rebind`, `edge_sync_before_dispatch` (PC); `asset_get`, `client_open_asset`, `plugin_assets`, `package_executions`, `configure_assets_origin`, `app_api_get`, `asset_preflight`, `browser_embed`, `browser_render`, `embed_action`, `plugin_lifecycle`, `package_update` (AS); `mcp_list`, `mcp_call` (admits a call and returns a ticket), `mcp_response` (waits on the real response of the ticket), `hub_feature` (MT). A real Hub implements each with injected parts (design 6.3), never by reading its internal tables or with a test hook. `edge_sync_before_dispatch` reads the injected storage edge and lane edge: a completed sync of the keyed pending record before the first frame that dispatches the call (more syncs are allowed; a missing or late one is false). The graph of PC-3 and the trace are observed by behavior: a refused cycle, a call that would close a cycle after the edge is retired, and the depth bound over a chain of handlers. `browser_embed` and `browser_render` need a real browser or renderer (acceptance fixtures that AS-2 and AS-3 name). `reload` takes the optional `plugin` and `as` that FakeHub already reads, and `service_connected` takes the optional `plugin`.

### Choices that a reader may question

- **Negative assertions** (a call never dispatched, a tool that never runs twice) use `expect_quiet_until` with the `settle` barrier of the connection, after a positive case in the same transcript, so the observer would change.
- **Values that the contract leaves open are not pinned**: the `origin` of the callee's `Invoke` (VC-5 is informative), the failure kind of an exceeded `plugin_call_depth` (typed `string`, `ok: false`, effect `none`), the status of a refused method on the assets origin (`$between` 400 and 499), the `401` or `403` of a token used as an app credential, the shape of the `timed_out` record (state `failed` or `timed_out`).
- **`pc_1_cancel_before_dispatch_effect_none`** makes the call queue (not dispatch) by keeping b's one active handler busy (IV-4). The operation record has `effect: none`; the `operations.cancel` call itself is `committed` (Appendix A).
- **`pc_1_cancel_after_dispatch_...`** also checks that the callee's earlier committed `store.put` survives (a later `store.get` returns it): cancel is not rollback.
- **`pc_4_*`** keep the first binding, the dedupe identity and the window apart: `two_interfaces_do_not_alias` (two providers, same key and arguments, two operations), `first_binding_stored_with_the_receipt` (`rebind`, the retry joins the receipt and never reaches the new provider), `uncertain_keyed_call_never_rerun` and `pending_record_fsynced_before_dispatch` (a Hub restart ends the call `unknown{hub_restarted}` and the key returns that record), `window_bounded` and `unknown_result_retained_through_window` (the window in `plugin_call_idempotency_window`).
- **`as_3_navigation_is_not_claimed_to_be_confined`** asserts nothing about navigation: it only requires the exact header of AS-3, which has no `navigate-to` directive. No AS-3 id asserts that a frame cannot navigate.
- **`as_4_*`**: disable, uninstall and quarantine are three separate acts in one transcript (`plugin_lifecycle`), each on its own plugin; crash, reload and Hub restart keep the runtime assets in another transcript. `as_4_reload_replaces_at_handover` checks the staged set before and after the handover and that the runtime asset stays.
- **MT**: `mcp_list` is the MCP client's view of the lists; `mcp_call` admits a `tools/call` and `mcp_response` waits for the real response after the handler's `Result`. `mt_2_name_collision_fails_load` collides a direct tool of the plugin `demo` (`_echo`) with the plugin `demo_` (`echo`): both are exposed as `demo___echo`, the later loader fails with `conflict{name}`, publishes nothing, and the earlier plugin keeps its tool. The collisions of native and proxied names are in the MP-4 transcripts of batch 2.
- **Not asserted (gaps recorded)**: the number of the Hub-owned depth (only the bound is observed); the failure kind of an exceeded depth.

### Review round 1 of batch 1 (REJECT 2907597f) and the fixes

1. Bind keys carry no `$`; the nested bind of the renewal transcript is gone.
2. `mcp_call` returns a ticket at admission; `mcp_response` waits for the real MCP response after the handler's `Result`.
3. `call_graph`, `call_trace` and `hold_dispatch` are removed. Graph retirement and trace inheritance are observed by calls. The sync-before-dispatch order is observed by `edge_order` at the injected storage and lane edges.
4. PC-4 retries compare the operation identity and read the retained record; no retry pins `queued` or `terminal: false`.
5. PC-4 no-replay checks finish the first handler (or reconnect the provider after a Hub restart) before the quiet check, so the provider is eligible.
6. The timeout transcript requires `state: failed` and `error.kind: timed_out`; the old-instance transcript requires a terminal record.
7. `setup.plugins.<key>.requires` states the locked interface, provider and tool membership; the depth transcripts have a positive depth and a valid chain (b requires z.echo of c).
8. `setup.plugins.<key>.views` declares the embed view `view` (permission `embed.view`, action `refresh`); `client_open_asset` issues the client verb for a viewer with permissions.
9. The AS-3 pages have the styled element, a script that submits the form, an explicit Hub URL and a clicked link; the results count the attempted requests. The real renderer runs belong in the slow tier (lead-approved: `conformance/replacement-map.json` now gives `slow:browser-embed` to `as_3_inline_script_and_style_run_and_nothing_loads_externally`, `as_3_referrer_policy_no_referrer` and `as_3_frame_has_no_direct_hub_call_but_client_mediated_actions_exist`).
10. `as_1_quota_bounded` has one case for each bound.
11. `mt_2` is the cross-plugin delimiter collision; the prompt of MT-1 is matched by its description.
12. Control invocations (not plugin calls) assert no `caller`.

### Review round 2 of batch 1 (REJECT 8ecc595e) and the fixes

1. `pc_1_old_instance_completion_never_reaches_new_instance` reads the retained record while the new instance still runs `h1`, then ends `h1` and checks quiet for the operation handler, so a wrongly delivered completion would be dispatched.
2. `edge_order` is replaced by `edge_sync_before_dispatch` (no sync count is pinned).
3. AS-3 browser results use `{"$not": 0}` for the attempted and navigation requests: no upper bound.
4. `pc_4_survives_plugin_reload_and_crash` crashes and reconnects b, retries the key (retained identity, quiet while b is eligible), then reloads b, reloads demo and crashes demo.

## Batch 2: PM (62) and MP (62)

All 124 are `fake: structural`, ruling R-1.3, and run in full against the real Hub in Stage 2. HP Amendment 3 is accepted and in the manifest (`frozen/current/MANIFEST.md`, final22), so `pm_14_fake_git_and_catalog_are_scripted_and_offline` and `mp_10_fake_mcp_server_is_scripted_and_offline` are structural transcripts against the real Hub with its injected I/O edges (A3-1). The plugin-side FakeHub and FakePlugin are unchanged: 0 lines of fake growth. The two real-run ids (`pm_14_real_run_...`, `mp_10_real_run_...`) use the slow controls `real_git_source`, `real_catalog` and `real_mcp_servers`.

### What the transcripts are built from

- **Client verbs.** PM-1 to PM-14 drive the Hub through `client_verb` and `client_read`: the verbs of the Hub-client contract that the 9.14 text names (`create`, `delete` and the `invoke` actions of the `package` and `catalog` kinds, and the hub actions). The caller is the operator, an agent with a scope, or a plugin (PM-13).
- **Injected edges (A3-1).** `script_git`, `script_catalog`, `script_resolve`, `script_redirect` (the git source, the catalog and DNS, offline), `script_mcp` (the MCP server for both transports), the storage edge (`storage_hold`, `storage_writes`) and the process edge (`bridge_census`, `bridge_starts`, `execution_count`). Every observer reads an edge boundary; none reads an internal table.
- **Barriers.** `await_package`, `await_catalog`, `await_mcp_server` and `await_mcp_list` wait on the real event, never on a timer or a poll. Time moves only with `advance_clock`.
- **The content hash of PM-3** is computed in the generator from the clause's own algorithm (path order, `file` or `link`, mode `100644` or `100755`, the digest of the content or of the link target). The same bytes give the same hash with other timestamps, owners and empty directories.
- **New controls** are in the table of the doc comment of `HUB_CONTROLS` (36 rows). `admit_manifest` gains the result field `approvals` (MP-1: `mcp_proxy_stdio`, "run local MCP server commands").

### Choices that a reader may question

- **Statuses and verbs that the contract names loosely.** `pin`, `unpin` and `approve_grants` are committed, `upgrade` and `rollback` are accepted, `preview_upgrade` is a read (the 9.14 intro). `enable` and `disable` name no class, so only the absence of an error is asserted. The `lock_history_max` count in `pm_3_lock_history_bounded` is read as the number of earlier entries that a rollback can reach (two rollbacks work, the third is refused).
- **Values left open.** The `kind` of a refused second source (PM-1), of a refused depth, of a refused unsupported method and the exact fields of the preview's `changed_files`, `manifest_diff` and `grant_diff` are typed, not pinned. The restart backoff of MP-8 is not pinned: the transcript passes 30 s inside the 60 s window and asserts the start count 1 plus `max`.
- **MP-11.** The optional `GET` stream is never required: the server answers `405` and the server stays available.
- **MP-13 `mp_13_uncarriable_descriptor_omitted`** (steward ruling R-24): the scripted edge sends one descriptor whose `inputSchema` repeats a key and one whose `inputSchema` holds an integer outside plus or minus (2^53 - 1) (`raw_input_schema`). Both are omitted and reported in the diagnostics, and an ordinary tool beside them still lists and answers a call.
- **MP-5 `mp_5_hub_forwards_no_credential_or_session_authority`** asserts no `authorization` or `cookie` header on the call and that the declared secret header is present; it does not claim OS isolation (the clause says the contract does not).
- **MT-2 native names.** A native Hub name has no `__` and a proxied or direct name always has one, so a proxied name cannot equal a native name. The collisions that can happen are between delimiter-ambiguous names, and they are in `mp_4_underscore_ambiguity_collides_by_full_name`, `mp_4_later_loader_fails_on_collision`, `mp_4_full_name_set_validated_before_publication` and `mp_4_collision_fails_that_server_only` (proxied against proxied, and a server that lists one name twice).

### The four ids added by the lead (HP A3-1)

- `nt_10_fake_io_is_scripted_and_offline`, `nt_10_telegram_shaped_long_poll_and_send` and `nt_10_unload_mid_request_cancels_and_leaks_nothing` are structural transcripts against the real Hub with its injected I/O edge, scripted by the control `net_world` of hp-events (`net_seen`, `net_state`, `net_release`, `hub_output` read what the edge saw). The replacement map said `player:hp-frame` for the first; with A3-1 it is `hub-inproc+edge` (network), changed here and regenerated in `docs/fake-replacement-map.md`.
- `a3_1_plugin_side_fakehub_scripts_host_results_only` runs against the plugin-side FakeHub (not structural): the FakeHub answers `http.request` with the scripted result whatever the URL (a private address and a host outside the allowlist get no `net_denied` from it), and a package-installing call is the FakeHub's own `unknown_call`. So it adds no network, package or MCP semantics.
- `every_cited_clause_exists` now also reads the HP Amendment 3 file, which holds the clause A3-1.

### Test speed

Each generated HP test is its own process (nextest) and `run_test` parsed the whole transcript directory to find its own file, so the default tier grew with the square of the suite and passed the 60 s budget with the 128 new tests. `run_test` now parses only the file of its id (falling back to the directory scan): the `fakes` binary went from 74 s of CPU to 6 s.

### A7 (the lock history): `pm_3_lock_history_bounded` and five `a7_1_*` ids

HP Amendment 7 (candidate 3) is in the manifest. The six transcripts read the lock record at the storage edge (`lock_record_stored`: the current lock, and the earlier ones newest first with the limit each was admitted under) and use `set_limit` for the live reduction. They follow the clause's examples: with `lock_history_max` 2, versions 1 to 4 give [v3, v2], and rollbacks reach v3, v2 and then `failed{wrong_state, no_history}` with effect none; the current lock is not counted; a rollback drops the lock it replaced and never pushes; a reduction from 3 to 1 trims nothing at the change, an update to v5 gives [v4, v3, v2], an update to v6 gives [v5, v3], and rollbacks reach v5, v3 and then `no_history`.

### Review round 1 of batch 2 (REJECT fca76677) and the fixes

Findings 1 to 26 of the reviewer. `docs/hp-plugins-notes.md` lists what changed:
- PM-7: the preview uses the client fields `old_hash` and `new_hash` and looks for the actual changed file, the manifest and the grant inside `text` (an unchanged file must not appear); the three crash and atomic-write transcripts finish the candidate bootstrap, wait for the armed storage hold (`await_storage_hold`) and scope the writes to the upgrade (`storage_writes since`); one transcript proves that the new directory was synced before the single synced lock write (`storage_dir_synced_before_lock_write`).
- PM-4: the size bound lies between the exact size of the small and the large package; the install transcript uses an admitted package and models the git hook and the submodule at the source boundary; the NT-2 transcript binds the operation ids and checks install, upgrade, rollback and catalog refresh (private address, redirect).
- PM-3: the hash vector has its entrypoint. `json` files of `script_git` have a stated byte rule (compact, key order as given).
- PM-5 to PM-14: the bootstrap is part of enable, so `await_package settled` comes after Hello, Ready and Activated; no handover has a running invocation (the old request ends before the candidate's Ready); `pm_10_remove` binds a request variable for each invocation; the real run has the second install, the old version running during the approval, the dev reload of the local source and the store retained by the remove.
- MP: `mcp_server_log ordered` replaces exact whole logs; collisions hold the later server's list until the earlier is published; the child env is compared as a map and the secret is declared; restart bounds are `starts` between 2 and 3 behind `await_restart_exhausted`; the endless-page transcript asserts a bounded request count and no request after the refusal; the descriptor and aggregate bounds use `mcp_proxy_tools_max` (per server and per plugin); the init and list deadlines wait for the dispatched request (`await_mcp_message`); `mcp_5` compares the serialized request bytes with the session token; `mp_1` compares approvals as a map; the http transcript has a private-address server; the stderr tail has `stderr_bytes` and a byte bound.
- NT-10: the long-poll test waits in a pending state, sends in the completion handler's context, and the unload test observes the cancellation at the I/O edge (`net_cancelled`) and no delivery to the dead instance.
- (MP-12 descriptor bytes: closed by steward R-26, see round 2.)

### Review round 2 of batch 2 (REJECT 8714ea86) and the fixes

1. The leftover tails of `nt_10_telegram_shaped_long_poll_and_send` and `pm_1_local_path_never_reaches_a_plugin` (a second copy of the old steps, which wrote the file again) are removed; no generator writes a transcript twice.
2. A bound value that a negative check looks for is a control argument (`lane_transcript contains`, `mcp_server_log wire_contains`), because `$contains` does not resolve a bound variable.
3. The second slow call of `mp_10_real_run` waits for a `tools/call` since a mark (`await_mcp_message since`).
4. `pm_4` NT-2: the package is updated successfully first (it has a history), and the failed rollback must not be the `wrong_state` refusal of an empty history.
5. MP-12 (steward R-26, no amendment): `mcp_proxy_tools_max` bounds one server's descriptors and one descriptor's bytes use `mcp_proxy_message_bytes`. The transcript has one server that exceeds the count over two pages, two servers within it that both list, and an oversized descriptor; the lists of the two failing servers are discarded whole. The earlier note on a missing descriptor-byte parameter is closed.
6. `await_restart_exhausted` is removed (it polled the scheduler). `mp_8_restart_is_bounded` jumps the injected clock over the declared window (60 s) and asserts that the starts inside it are at most the first start plus `max`.
7. `mp_4_collision_fails_that_server_only` has no early whole-list read.
8. `pm_14` real run: the rollback is followed by a lock assertion, and the local dev reload completes (Hello, Ready, Activated, Shutdown of the old instance).
9. `mp_3_child_has_no_core_credential_or_env` asserts the exact set of env names as well as the values.

### Review round 3 of batch 2 (REJECT dae29afd) and the fixes

1. `pm_4` NT-2: before the rollback the transcript deletes the local copies of the earlier versions (`package_cache_wipe`), so the rollback has to fetch the locked commit. Every refusal of that transcript now asserts the code `net_denied` (not a generic failure).
2. `mp_8_restart_is_bounded` counts the starts in the half-open window of 60000 ms that begins at the first start that the process edge saw (`bridge_starts window_ms`), with the upper bound of the first start plus `max`.
