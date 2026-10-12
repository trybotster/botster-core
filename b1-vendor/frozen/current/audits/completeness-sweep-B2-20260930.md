# Completeness sweep B2: every command fully specified (2026-09-30)

Auditor: read-only contracts auditor (Claude). No file was edited except this one.

## Scope

- HP: `hub-plugin-contract-v1.1.joint-J4e.md` (928 lines) and `hub-plugin-contract-v1.1.amendment-1-candidate5.md` (44 lines).
- Lua: `hub-plugin-lua-binding-v1.1-r6.md` (103 lines).
- Core: `core-contract-v1.17.md` (514 lines).

Question (B): for each call, handler kind, manifest action, event and Core operation, does the contract state:
1. the inputs, with types and required or optional;
2. the output, with every enum value defined;
3. the failures specific to the call;
4. the effect, the commit point and the durability.

## Method

- I read the four files in full.
- I compared the Appendix A call set (HP lines 722-780) with the LN-2 Lua table (HP lines 851-909) by script. Both hold the same 57 calls. No call is missing from either table.
- For each Appendix A row I checked the four points against the clause that the row cites.
- For Core I listed every `begin(...)` operation, every sync call and every event, and I looked for its result type, its error codes and its enum values.

## Summary

| Owner | High | Medium | Low | Total |
|---|---|---|---|---|
| 0016 (HP and Lua) | 9 | 22 | 9 | 40 |
| 0019 (Core) | 5 | 11 | 5 | 21 |
| All | 14 | 33 | 14 | 61 |

Severity:
- **high**: an implementer must guess a behavior that is visible across the contract boundary.
- **medium**: the text is ambiguous or two clauses disagree.
- **low**: cosmetic, or a stale table.

What is good:
- Appendix A has a row for every call, and every row has an effect and a commit point.
- LN-2 and Appendix A agree name for name.
- Core IN-2 (`InputResult`), SZ-3 (resize), IN-6 (`CancelResult`), AD-2 (`Lost` reasons), the route `input_refused` reasons (DP-5) and SV-5 (service exit causes) are complete.

---

## A. HP and Lua gaps (owner 0016)

### High

**B2-1. `session_types.spawn` has an empty failure list.** (high, 0016)
- Evidence: HP:771 lists `invalid_request;` and nothing else. HP:772 (`ensure_worktree_and_spawn`) says "as `spawn`". Other clauses name failures that the row omits: `invalid_request{unknown_context_key}` (CA-14, HP:256), `quota_exceeded{no_session_port}` (CA-20, HP:261), the target boundary of CP-8 (HP:144), a disabled target (CA-13, HP:254), `backpressured` for the receipt reservation (CA-16, HP:264).
- Amendment: "`session_types.spawn` failures: `not_found` (target, session type or worktree); `invalid_request{unknown_context_key, environment_not_allowed, env_too_long, prompt_too_long}`; `failed{detail.code: target_disabled}`; `capability_denied` (a target outside a target-scoped package's own target, CP-8); `quota_exceeded{no_session_port}`; `backpressured` (no receipt capacity, CA-16). Effect `none` for every refusal."

**B2-2. The `wrong_state` sub-reason has no defined field.** (high, 0016)
- Evidence: the texts mix three shapes. HP:779 `failed(wrong_state: occupied)`. HP:479 `failed{wrong_state: session_ended}`. HP:478 `failed{detail.code: wrong_state}` "with the reasons `not_ended` ...". LB-12 (Lua:21) says "a reason of `not_ended` ...". No clause says whether `occupied` is the `detail.code` or a second field. HP:523 also gives `not_ended` as an example `detail.code`.
- Amendment (section 14): "A state refusal is `failed{detail.code: wrong_state, detail.reason: <reason>}`. `detail.reason` is a closed set per call: `sessions.stop`, `sessions.delete`, `sessions.read_screen`: `starting|running|stopping|ended|lost|no_screen_model`; `sessions.safe_write`, `sessions.write`: `session_ended|not_running`; `sessions.restart`: `not_ended|not_restartable|record_unavailable|environment_not_retained|not_ready`; `worktrees.delete`: `occupied`. A host that does not know a reason acts on `detail.code`."

**B2-3. `sessions.write`: result state and error overlap.** (high, 0016)
- Evidence: HP:765 and HP:894 give the result `{state: written|partial|cancelled|failed|unknown, ...}`. RW-2 (HP:490) maps `NotWritten(Refused|...)` to `failed{detail.code: not_written, reason}` and `Failed` to `failed{detail.code: os_write_failed}` "with the exact counts". One cannot tell if `failed` is `ok: true` with `state: failed`, or `ok: false` with kind `failed`. The values of `reason` are not given. An error has no field for byte counts. Appendix A lists neither `not_written` nor `os_write_failed`.
- Amendment (RW-2): "`Written`, `Partial`, `Cancelled`, `Unknown` and `Failed` return `ok: true` with `state` = `written|partial|cancelled|unknown|failed` and the counts. For `failed`, `reason: os_write_failed`. `NotWritten(LaneFull)` is `backpressured`. `NotWritten(SessionEnded|Stopping)` is `failed{detail.code: wrong_state, detail.reason: session_ended}`. Every other `NotWritten(r)` is `failed{detail.code: not_written, detail.reason: refused|unsupported|mode_precondition_failed|stale|not_reported}`, effect `none`. `reason` in a result is one of `os_write_failed`, `cancelled`, `link_lost`." Add `not_written` to the Appendix A row.

**B2-4. The operation record states are not closed per kind.** (high, 0016)
- Evidence:
  - CA-16 (HP:264): an id from an earlier Hub run answers `{kind: "safe_write", state: "unknown", reason: "hub_restarted"}` for every kind. A spawn, http or plugin_call id then changes its kind.
  - PC-4 (HP:314) ends a `plugin_call` as `unknown{hub_restarted}`. PC-1 (HP:311) ends it `cancelled` or `timed_out`. PC-5 (HP:315) ends it `failed{plugin_crashed}`. CA-16 allows only `done` or `failed` for `plugin_call`.
  - `process` (PX-2, HP:295) fails "with `reason`", but `plugin_call` and `http` fail with `error`. The record carries both `reason?` and `error?` (HP:773) and no rule says which kind uses which.
  - `spawn` `failed{reason}` (CA-14, CA-15) has no value set.
  - `process` and `plugin_call` have no non-terminal state after `queued` (for example `running`).
- Amendment (CA-16): "The state table per kind is closed. `spawn`: `starting`, `installed`, `failed`, `cancelled`, `unknown`. `safe_write`: as SW-2. `http`: `queued`, `done`, `failed`, `cancelled`, `unknown`. `ws`: `queued`, `open`, `failed`, `cancelled`, `unknown`. `process`: `queued`, `running`, `done`, `failed`, `cancelled`, `unknown`. `plugin_call`: `queued`, `dispatched`, `done`, `failed`, `cancelled`, `timed_out`, `unknown`. A terminal failure carries `error` (a section 14 error). `reason` is used only by `safe_write` and `spawn`. A record lost at a Hub restart answers `{kind: <the kind encoded in the id>, state: "unknown", reason: "hub_restarted", terminal: true}`. `spawn` `failed.reason` is one of `start_failed`, `worker_lost`, `worktree_failed`, `session_type_invalid`, `target_disabled`."

**B2-5. Handler kinds are not a closed, defined set.** (high, 0016)
- Evidence:
  - `Ready.handlers[{id, kind, descriptor}]` (HP:818) does not define the `kind` strings. LB-10 (Lua:19) says `tool` and `prompt`. VC-5 (HP:158) and HP:782 say `mcp_tool` and `mcp_prompt`.
  - The origins `command` and `hook` (HP:158, HP:782, LB-10) are never defined. No payload, no result, no grant.
  - `session_action` (HP:158) has no handler kind. VW-8 (HP:457) uses `ui_action`. CP-2 (HP:137) speaks of "session handlers".
  - Missing payload or result schemas: `entity_provider` (snapshot shape and paging, EN-2/EN-3, HP:438-439); `ui_action` (view id, action id, entity id, input, `idempotency_key`, `operation_id`, VW-3/VW-8); `mcp_prompt` result; MCP resource read (MT-1, HP:397); `relay` handler result (WP-2, HP:324); `timer` and `store_watch` handler selection (see B2-27).
- Amendment (new Appendix A table "Handler kinds"): "One row per kind: `mcp_tool`, `mcp_prompt`, `mcp_resource`, `ui_action`, `event`, `event_gap`, `entity_provider`, `timer`, `store_watch`, `operation`, `webhook`, `relay`, `fs_watch`. Each row gives the wire `kind` string, the `descriptor` schema, the grant (CP-2), the `Invoke.payload` schema and the `Result.value` schema. `command`, `hook` and `session_action` are removed from VC-5 and LB-10 (or each gets a row)." LB-10 uses the same strings.

**B2-6. Hub event payloads are not defined.** (high, 0016)
- Evidence: EV-9 (HP:383) names `hub/session.created|starting|running|stopping|ended|lost|removed|attached|detached`, `hub/session.activity|modes|size|silent`, `hub/route.ended`, `hub/plugin.loaded|unloaded|failed`, `hub/config.changed` and "spawn operations". No payload is given for any of them. The spawn operation events have no names. EV-14 (HP:388) names `terminal/title|cwd|bell|notification|prompt_mark|modes|exit` with no payload (only `terminal/output` has one). Defined payloads: `hub/worktree.*` (EV-8), `hub/session_notification`, `hub/plugin.dependency`, `hub/notice` (VW-9), `net/ws.*` (NT-4).
- Amendment (EV-9, new table): "One row per Hub and terminal event name: payload schema, subject, priority, audience and grant. Minimum: `hub/session.<state>{session, state, exit?{code, signal, cause}, reason?}`; `hub/session.attached|detached{session, client_kind}`; `hub/session.activity{session, source: output|client, at}`; `hub/session.modes{session, modes}` (SO-1 shape); `hub/session.size{session, rows, cols}`; `hub/session.silent{session, since}`; `hub/route.ended{session, reason}` (the OU-2b wire reasons); `hub/plugin.loaded|unloaded|failed{plugin_key, version, reason?}`; `hub/config.changed{plugin_key, names}`; `terminal/title{text}`, `terminal/cwd{cwd}`, `terminal/bell{}`, `terminal/notification{source: osc9|osc777, title?, body}`, `terminal/prompt_mark{mark: a|b|c|d, exit_code?}`, `terminal/modes{modes}`, `terminal/exit{code?, signal?, cause}`. Spawn operations are delivered only by CA-16; delete "spawn operations" from EV-9."

**B2-7. `entity.publish`: the `patch` format and the status values are not defined.** (high, 0016)
- Evidence: HP:780 and HP:909 take `type: upsert|patch|remove` and `patch?`. No clause gives the patch format (JSON Merge Patch, JSON Patch or another). EN-4 (HP:440) lists `accepted`, `pending_gap`, `resync_scheduled` and does not say what the last two mean or what the plugin must do. The failure row has no `detail.code`.
- Amendment (EN-4): "`patch` is a JSON Merge Patch (RFC 7396) on the current record. A patch on an absent id is `not_found`. A patched record that fails the family schema is `invalid_request{detail.code: entity_schema}`. `pending_gap`: the change is admitted, and a subscriber that missed it gets a reset and a fresh baseline (EN-3); the plugin does nothing. `resync_scheduled`: the Hub will ask the provider for a new snapshot; the plugin does nothing. `quota_exceeded{detail.code: entity_bytes}`."

**B2-8. Lua cannot express JSON `null`.** (high, 0016, Lua binding)
- Evidence: LB-15 (Lua:25) says `web.set_origin{origin = nil}` clears the origin. LB-16 (Lua:26) says `secrets.put{name, value | nil}`. In Lua, `{origin = nil}` is the empty table `{}`. WF-4 (HP:86) says absent and `null` differ. A missing required `origin` is `invalid_request`. ST-5 (HP:234) orders `null` apart from a missing field, so a stored document can hold `null`. LB-9 (Lua:18) defines no null value for encode or decode.
- Amendment (LB-9): "`botster.json.null` is a unique sentinel. It encodes as JSON `null`, and `json.decode` and every host-call result return it for `null`. Arguments: `web.set_origin{origin = botster.json.null}` clears; `secrets.put{name, value = botster.json.null}` deletes. A Lua `nil` is an absent key." Fix LB-15 and LB-16 to match.

**B2-9. `store.batch` marks `expected_revision` required on every op, so a batch cannot create.** (high, 0016)
- Evidence: HP:731 and HP:860 give `ops: [{op: put|delete, collection, id, doc?, expected_revision}]` with no `?`. ST-2 (HP:231) creates only with `expected_revision` absent. ST-3 (HP:232) says "each with its expected revision". Lua row 8 (Lua:44) copies the required field. The workspaces and messaging plugins create documents in batches (Lua:69, Lua:72).
- Amendment (Appendix A and LN-2): "`ops: [{op: put|delete, collection, id, doc?, expected_revision?}]`. A `put` with no `expected_revision` creates and fails `conflict{revision_conflict, op_index}` if the id exists. A `delete` needs `expected_revision`; without it the batch is `invalid_request{detail.code: expected_revision_required, op_index}`."

### Medium

**B2-10. The `plugins.call` failure row is incomplete.** (medium, 0016)
- Evidence: HP:757 omits `conflict{idempotency_key_reuse}` (PC-4, HP:314). The depth bound `plugin_call_depth` (PC-3, HP:313) has no code. The terminal failures (`plugin_crashed`, `timed_out`, `cancelled`) are not on the row.
- Amendment: "Admission failures add `conflict(idempotency_key_reuse)` and `conflict(call_depth_exceeded)`. Terminal record errors: `failed(plugin_crashed)`, `timed_out`, `cancelled`, `failed(handler_error)`, `response_too_large`."

**B2-11. The kind and code of a fenced write are not stated, and `wrong_context` is missing from the common failures.** (medium, 0016)
- Evidence: LD-3 and LD-9 (HP:121, HP:127) say an old-instance write after the fence is "refused `effect: none`" with no kind or code. CA-3 (HP:188) `failed{wrong_context}` applies to every call but is not in the Appendix A common list (HP:720).
- Amendment: "A write refused at the reload fence is `unavailable{detail.code: instance_fenced}`, effect `none`. Add `failed(wrong_context)` and `unavailable(instance_fenced)` to the common failures of Appendix A."

**B2-12. Calls that admit an operation do not list `backpressured` or the port quota.** (medium, 0016)
- Evidence: CA-16 (HP:264) refuses `backpressured` when no receipt can be reserved. The rows for `sessions.create_command` (HP:743), `sessions.restart` (HP:768), `ensure_worktree_and_spawn` (HP:772) and `sessions.safe_write` (HP:764) do not list it. `create_command` and `restart` also omit `quota_exceeded{no_session_port}` (CA-20).
- Amendment: "Every row whose result carries `operation_id` lists `backpressured(no_receipt_capacity)`. Every row that starts a session run lists `quota_exceeded(no_session_port)`."

**B2-13. `sessions.stop` has a `stop_timeout` that cannot happen.** (medium, 0016)
- Evidence: SA-5 (HP:493) says a stop that does not stop in time is `failed{stop_timeout}`. SA-1 (HP:477) and HP:739 answer `{decision: "stopping"}` as soon as Core accepts, and the end is observed later.
- Amendment: delete `failed{stop_timeout}` from SA-5. "A stop that does not end the session is observed on the session family; `sessions.stop` never reports it."

**B2-14. The value shapes of `equals` and `range` are not defined.** (medium, 0016)
- Evidence: HP:728 and ST-4 (HP:233) say `equals` "binds a prefix of the index fields" and `range` is `{from?, to?}`. They do not say if `equals` is an array in field order or an object keyed by field name. `store.watch{equals?}` (HP:732) has the same gap. `store.query` does not list `failed(incompatible_document)` (ST-11, HP:241) for a document in the page.
- Amendment (ST-4): "`equals` is an array of values for the first N index fields, in declaration order. `range.from` and `range.to` are single values of field N+1. A value of the wrong declared type is `invalid_request{detail.code: index_value_type}`. A page that meets a document outside `accepts` fails `failed{incompatible_document, id}`."

**B2-15. Collection reads have a bound but no cursor.** (medium, 0016)
- Evidence: CA-13 (HP:254) bounds `spawn_targets.list`, `worktrees.list` and `session_types.list` by `page_items`. Their rows (HP:769, HP:775, HP:777) take no `after` and return no `next_after`.
- Amendment: "Each of these reads takes `after?` and returns `next_after?`, as `sessions.list` (SS-1)."

**B2-16. Several row and result enums are not defined.** (medium, 0016)
- Evidence: `session_types.show` returns `{definition}` (HP:770) with no schema. `fs.list` entry `kind` and `fs.stat` `kind` (HP:751-752) have no value set. The worktree row `status` and the spawn target row `kind` (HP:794-795) have no value set. `config.get` `diagnostics` (HP:726, CF-1) has no item schema.
- Amendment: "`definition` is the Hub-client `session_type` resource with its `context_keys`, `allowed_environment_overrides`, `interaction` and `presentation`. `fs` `kind`: `file|dir|symlink|other`. Worktree `status`: `creating|ready|occupied|deleting|failed`. Spawn target `kind`: `git|directory`. `diagnostics[]`: `{name, code, message}`."

**B2-17. `http.request` argument and result shapes are partly open.** (medium, 0016)
- Evidence: HP:744 and NT-3 (HP:279) do not give the `method` set or the `headers` shape (an object or a list of pairs; repeated headers). The row omits the NT-2 codes `net_userinfo` and `net_header_forbidden` and a refused scheme. The response `headers` shape is not given.
- Amendment (NT-3): "`method` is one of `GET|HEAD|POST|PUT|PATCH|DELETE|OPTIONS`. `headers` is a list of `[name, value]` pairs, in both directions, and keeps repeats and order. Admission failures add `invalid_request(net_userinfo, net_header_forbidden, net_scheme)`."

**B2-18. `process.run`: `truncated` has two shapes.** (medium, 0016)
- Evidence: HP:750 and LB-14 (Lua:24) give `truncated` as one value. PX-2 (HP:295) gives `truncated: {stdout, stderr}`. PX-2 fails "with `reason`" (`command_not_found`, `timed_out`, `cwd_not_allowed`), but `detail.code: process_bridge_failed` uses `error`.
- Amendment: "`truncated` is `{stdout: bool, stderr: bool}` everywhere. A failed `process` record carries `error`: `failed(command_not_found)`, `failed(process_bridge_failed)`, `invalid_request(cwd_not_allowed)`, `timed_out`." (This joins B2-4.)

**B2-19. `relay.deliver` has an undefined `rejected` status and a doubtful effect.** (medium, 0016)
- Evidence: HP:761 returns `{status: accepted|rejected}` and says effect `none`. No clause says when `rejected` happens or how it differs from an error. The envelope reaches a client connection, so `none` is wrong after `accepted`. The `relay` handler's `Result` (WP-2, HP:324) has no schema.
- Amendment: "`accepted`: the envelope was queued to the attempt, effect `committed`. `rejected`: the attempt has ended (completed or expired); effect `none`. The `relay` handler returns `{}`; a failure drops the envelope and counts it."

**B2-20. The allowed asset content types are not listed.** (medium, 0016)
- Evidence: HP:762 refuses `content_type_not_allowed`. AS-3 (HP:339) says "a fixed allowed set" and does not list it.
- Amendment (AS-1): "Allowed `content_type`: `text/html`, `text/plain`, `text/css`, `application/javascript`, `application/json`, `image/png`, `image/jpeg`, `image/gif`, `image/webp`, `image/svg+xml`. Any other is `invalid_request{content_type_not_allowed}`."

**B2-21. The `log.write` result lacks `dropped_before`.** (medium, 0016)
- Evidence: CA-9 (HP:222) says "the next accepted record reports `dropped_before`". HP:724 and HP:853 give the result `{seq}` only.
- Amendment: "`log.write` result: `{seq, dropped_before?}`."

**B2-22. `sessions.update` does not say how `metadata` is written.** (medium, 0016)
- Evidence: HP:741 and SA-1 (HP:477) take `metadata?` and write "only the plugin's own metadata namespace". They do not say if the value replaces `metadata[<plugin_key>]` or merges into it, or what `null` does. The commit point is "the write returns", with no durability.
- Amendment: "`metadata` is a JSON object that replaces `metadata[<plugin_key>]` whole; `null` removes the namespace. The write is durable (ST-9) before the answer."

**B2-23. The `safe_write` `failed` and `unknown` reasons are open.** (medium, 0016)
- Evidence: SW-2 (HP:480) lists `abandoned` and `cancelled` reasons. `failed{reason}` has none. `unknown{reason}` names `hub_restarted` and then "a write whose host step could not be confirmed" with no code. SW-8 (HP:486) completes `unknown` with no reason.
- Amendment: "`failed.reason`: `core_refused`, `session_removed`. `unknown.reason`: `hub_restarted`, `link_lost`, `cancel_after_text`."

**B2-24. Observation events name two different grants.** (medium, 0016)
- Evidence: SO-3 (HP:496) gives `session_observe[:any]` for `hub/session.activity|modes|size|silent` and `terminal/*`. EV-2 (HP:376) and EV-14 (HP:388) give `events_terminal`. LB-12 (Lua:21) says "`session_observe[:own|:any]` or `events_terminal`". MN-2 (HP:841) puts `session_observe` on surface `session_actions`, not `events`.
- Amendment (SO-3): "A subscription to `hub/session.activity|modes|size|silent` needs `events_hub` plus `session_observe[:own|:any]`. A subscription to `terminal/*` (except `terminal/output`) needs `events_terminal[:own|:any]`. Nothing else admits them."

**B2-25. CP-6 still defines `events.describe`.** (medium, 0016)
- Evidence: CP-6 (HP:141) says a public event "can be discovered (`events.describe{owner,name}`)". HP:784 and LB row 23 (Lua:59) say `events.describe` is dropped. HP:199 says the same.
- Amendment: delete the `events.describe` sentence from CP-6. "A public event is discovered from the producer's manifest; no call exists."

**B2-26. Appendix B frames lack fields that clauses use.** (medium, 0016)
- Evidence: HS-1 (HP:103) puts `bindings` in `Hello`. EV-11 (HP:385) puts `priorities` in `Hello`. FS-3 (HP:305) and LB-15 (Lua:25) put `fs_watches` in `Ready`. VW-8 (HP:457) says `Ready` carries the `action_id` to `handler_id` map. Appendix B (HP:817-818) has none of them.
- Amendment (Appendix B): "`Hello` adds `bindings[{interface, interface_version, provider?, provider_version?, interface_hash?}]` and `priorities[{name, rank}]`. `Ready` adds `fs_watches[{root, path, handler_id}]` and `session_actions[{action_id, handler_id}]`."

**B2-27. Timers and store watches do not name their handler.** (medium, 0016)
- Evidence: `timers.arm{timer_id, delay_ms}` (HP:735) and `store.watch{collection, index?, equals?}` (HP:732) take no `handler_id`. `fs.watch` does (FS-3, HP:305). With several `timer` or `store_watch` handlers, no rule picks one.
- Amendment: "A plugin registers at most one `timer` handler and one `store_watch` handler; a second is a load failure `invalid_request{duplicate_handler_kind}`." (Or add `handler_id` to both calls and to `Ready.timers` and `Ready.watches`.)

**B2-28. API and feature load failures have three spellings and a kind outside the closed set.** (medium, 0016)
- Evidence: HS-4 (HP:106) `missing_feature{names}`. Section 18 (HP:639) `unsupported_plugin_api`. PD-2 (HP:920) `unsupported{detail.code: api_unsupported | feature_missing | ui_contract_unsupported}`. LB-18 (Lua:27) uses the PD-2 codes. `unsupported` is not in the section 14 kinds table (HP:509-521).
- Amendment: "The load failure is `failed{detail.code: api_unsupported | feature_missing{names} | ui_contract_unsupported}`. HS-4 and section 18 use these codes. No kind `unsupported` exists."

**B2-29. The `hub/session_family` frames have no schema.** (medium, 0016)
- Evidence: EV-6 (HP:380) names `snapshot_begin`, `snapshot_chunk`, `snapshot_end` and deltas. No field is given. LB-6 (Lua:15) and the workspaces plugin depend on it.
- Amendment (EV-6): "`snapshot_begin{generation}`, `snapshot_chunk{generation, sessions: [session]}`, `snapshot_end{generation}`, `delta{generation, op: upsert|remove, session?, session_id}`. A row is the client `session` resource."

**B2-30. Package operations: names disagree and failures are open.** (medium, 0016)
- Evidence: PM-10 (HP:359) names `install`, `update`, `pin`, `unpin`, `rollback`, `enable`, `disable`, `remove`, `status`, `refresh_catalog`. Section 9.14 (HP:346) names the client verbs `create`, `upgrade`, `preview_upgrade`, `approve_grants`, `delete`, `refresh` and has no verb for `enable`, `disable` or `status`. No code is given for: a fetch failure, a package over `package_bytes_max`, an unpack refusal (traversal, symlink, special file), a catalog over `catalog_bytes_max` or with a bad `catalog_version`, `approve_grants` with a set that differs from the pending set, `rollback` with no older entry, `pin` to a ref that does not resolve. Amendment 1 adds only `path_*` and `manifest_*` codes (Amd:13-14).
- Amendment (PM-10): "A table maps each operation to its client verb and lists its failures: fetch `failed{net_dns|net_connect|net_tls|net_denied}`; `quota_exceeded{package_too_large}`; `failed{unpack_refused, path}`; `quota_exceeded{catalog_too_large}`; `invalid_request{catalog_version_unsupported}`; `conflict{grants_mismatch}`; `failed{wrong_state: no_history}`; `not_found{ref}`."

**B2-31. Observe reads and the screen read refuse an ended session differently.** (medium, 0016)
- Evidence: `sessions.read_screen` (HP:742) is `failed(wrong_state)` with no screen model. `sessions.terminal_state` and `read_cursor` (HP:766-767) list `unavailable` with no condition. Core ST-5 keeps reads valid after exit.
- Amendment: "The three reads succeed for a session with a model (running or ended, until removed). With no model they are `failed{wrong_state, reason: no_screen_model}`. `unavailable` means only that the Hub is not ready (SS-3)."

### Low

**B2-32. The section 9.1 call table is stale.** (low, 0016) HP:192-214 omit `process.run`, `fs.*` calls, `plugins.*`, `web.*`, `relay.*`, `assets.*`, `secrets.put` and `sessions.create_command`. Amendment: "Section 9.1 is informative; Appendix A is the list."

**B2-33. Spawn admission answers lack `kind`.** (low, 0016) HP:743, 768, 771-772 return `{operation_id, session, state, terminal}` with no `kind: "spawn"`. SW-1 (HP:479) answers `{operation_id, state}` with no `kind` or `terminal`, unlike HP:764. Amendment: "Every admission answer carries `kind`."

**B2-34. `store.watch` and `fs.watch` state their effect differently.** (low, 0016) HP:732 "none (registers)"; HP:754 "commit: the watch is recorded". Amendment: use "commit: the watch is registered (not durable)" in both.

**B2-35. LN-2 forbids names that LN-1 and LB-9 add.** (low, 0016) LN-2 (HP:849) says "no other `botster.*` name exists". LN-1 (HP:848) adds host-local names. LB-9 (Lua:18) adds `json.array` and `json.empty_array`, which LN-1 omits. Amendment: "LN-2 covers host calls. Host-local names are LN-1 plus the binding's LB-8 and LB-9 names."

**B2-36. `operations.get` has a rate limit with no parameter.** (low, 0016) CA-16 (HP:264) refuses `backpressured` "over the ordinary rate". Section 15 names no parameter. Amendment: add `operations_get_rate` to section 15.

**B2-37. `timers.arm` `delay_ms` has no bound.** (low, 0016) HP:735, CA-12. Amendment: add `max_timer_delay_ms`; over it is `invalid_request{delay_out_of_range}`.

**B2-38. `manifest_malformed{path}` reuses the word `path`.** (low, 0016) Amd:14 uses `path` for a JSON pointer; Amd:13 uses `path` for a source directory. Amendment: rename the field `pointer`.

**B2-39. Amendment 1 cites a limit name that HP does not define.** (low, 0016) Amd:13 and Amd:22 bound `path` by `max_frame_bytes`. HP section 15 names `frame_bytes`. Amendment: cite `frame_bytes`, or name a client limit.

**B2-40. `hub.identity` lists `unavailable` with no condition.** (low, 0016) HP:725. Amendment: "`unavailable` only before the Hub has a `hub_id` (first start)."

---

## B. Core gaps (owner 0019)

### High

**B2-41. There is no per-operation outcome table.** (high, 0019)
- Evidence: section 2 (Core:35) says every operation completes as `Completed{op, result}`. The event table (Core:212) says "An operation finished". The error table (Core:316-349) is global. For most operations the success value, the error codes and the state preconditions are not stated: `Create`, `Start`, `Stop`, `Signal`, `Remove`, `StopAll`, `UpdateMetadata`, `SetSizePolicy`, `SetColorProfile`, `SetOutputTap`, `ReadScreen`, `ReadCursor`, the mode read, `Detach`, `AdoptAll`, `Adopt`, `EndEpoch`, `StopService`, `RemoveService`. Defined ones: `WriteInput` (IN-2), `Resize` (SZ-3), `CaptureSnapshot` (ST-6), `ReadFacts` (ST-7), `AttachWebRtc` success (DP-2), `SpawnService` success (SV-4).
- Amendment (new section 9.4): "One row per `begin` operation: arguments with types; the `Ok` value; the sync error codes; the async error codes; the session or service states in which it is admitted. Example: `Start(id)`: `Ok(())` after `Running`; sync `UnknownSession`, `WrongState` (not `Created`), `PendingLimit`; async `StartFailed{reason}`, `RegistryFailed`. `Adopt(id)`: `Ok(state)`; sync `UnknownSession`, `WrongState` (not `Lost(WorkerUnreachable|WorkerVersion)`); async `Lost(reason)` as the result."

**B2-42. `WriteInput` can fail two ways and no rule picks one.** (high, 0019)
- Evidence: IN-2 (Core:77) gives `Ok(InputResult{outcome})` with `NotWritten(SessionEnded)`, `Cancelled` and `Unknown`. The error table also has `SessionEnded` (Core:327), `Cancelled` "before any effect" (Core:341) and `WorkerLinkFailed` (Core:331). A write to an ended session (IN-7, Core:82) could be `Err(SessionEnded)` or `Ok(NotWritten(SessionEnded))`. HP RW-2 (HP:490) maps only `InputResult`.
- Amendment (IN-2): "`WriteInput` completes `Ok(InputResult)` for every admitted write. It never completes with `Err(SessionEnded)`, `Err(Cancelled)` or `Err(WorkerLinkFailed)`; those cases are `NotWritten(SessionEnded)`, `Cancelled` and `Unknown`. Its only async error is `Internal`."

**B2-43. The `RouteClosed` reason enum is not closed and has unnamed values.** (high, 0019)
- Evidence: OU-2 (Core:143) lists `Detached`, `SessionEnded`, `SessionRemoved`, `PeerClosed`, `BadFrame`, `SnapshotTooLarge`, `StallTimeout`, `HandoffFailed`. OU-2b (Core:144) adds `Replaced`, `Revoked`, `BadPeer`, `NotConnected`. OU-2b also maps "a failed write to the transport" to `write_failed` and "a worker exit or `Lost` session" to `session_lost` with no Core reason name. `BadFrame` carries a code that OU-2 does not show.
- Amendment (OU-2): "`RouteCloseReason` (non-exhaustive) = `Detached`, `Replaced`, `Revoked`, `SessionEnded{exit}`, `SessionRemoved`, `SessionLost`, `PeerClosed`, `WriteFailed`, `BadFrame{code}`, `BadPeer`, `NotConnected`, `SnapshotTooLarge`, `StallTimeout`, `HandoffFailed`. OU-2b maps each one; an unknown reason maps to `transport_lost`."

**B2-44. The payloads of `Bell`, `PromptMark` and `Notification` are not defined.** (high, 0019)
- Evidence: the event table (Core:207, Core:223) names the three events as "Discrete terminal events (BEL, OSC 133, OSC 9/777)" with no fields. HP `hub/session_notification` (HP:383) needs `source: bell|osc9|osc777`, `title?`, `body?`. HP `terminal/prompt_mark` (HP:388) needs the mark.
- Amendment (6.2): "`Bell{id, instance, at}`. `PromptMark{id, instance, mark: PromptStart|CommandStart|CommandExecuted|CommandFinished, exit_code?, at}` (OSC 133 A, B, C, D). `Notification{id, instance, source: Osc9|Osc777, title?, body, at}`; `body` and `title` are bounded by `clipboard_bytes` or a new `notification_bytes`, and an oversize text is truncated with `truncated: true`."

**B2-45. `service_send` and `service_recv` have no complete signatures.** (high, 0019)
- Evidence: SV-3 (Core:436) says `service_send` "refuses at once with `Backpressured`". `Backpressured` is not a `CoreError` code (Core:320-349). No code exists for an unknown service, a lane out of range, a send before the epoch commits, or a send to an `Exited` service. `service_recv` does not say what it returns when the queue is empty.
- Amendment (SV-3): "`service_send(id, lane, frame) -> Result<(), SendError>`, `SendError` = `Backpressured`, `UnknownService`, `NoSuchLane`, `NotConnected` (no committed epoch), `ServiceEnded`, `Oversize`. `service_recv(id, lane) -> Result<Option<Frame>, RecvError>`, `None` when empty; `RecvError` = `UnknownService`, `NoSuchLane`. Add `UnknownService` to 9.3."

### Medium

**B2-46. Service states and lane-end reasons are not closed.** (medium, 0019) SV-5 (Core:438) lists `Started`, `Reconnected`, `Exited`. `Lost(GuardianLost)` with `payload_may_remain` (SV-8, Core:441) and `Released` (Core:201) are not in SV-5. `LaneEnded.reason` (Core:435, Core:438) lists `BadLane`, `Oversize`, `Truncated`, `PeerClosed`, not `EndEpoch`, a higher epoch, a host stop or a service exit. Amendment: "`ServiceState` = `Started`, `Reconnected{epoch}`, `Exited{...}`, `Lost{reason, payload_may_remain}`, `Released`. `LaneEndReason` = `BadLane`, `Oversize`, `Truncated`, `PeerClosed`, `EpochEnded`, `Superseded`, `ServiceExited`, `ServiceRemoved`."

**B2-47. `SpawnService` failures are open.** (medium, 0019) SV-1 (Core:434) says "a typed refusal (SV-4)", and SV-4 gives only `BoundUnavailable`. No code for an `exec` failure, a duplicate `service_id`, or a bad lane config (`InvalidConfig` is listed as `open` only, Core:345). Amendment: "async `StartFailed{reason: exec_failed{errno}|startup_timeout}`; sync `IdInUse`, `ServiceLimit`, `InvalidConfig`, `BoundUnavailable`."

**B2-48. `ServiceLimits` omits fields that clauses use.** (medium, 0019) SV-2 (Core:435) checks `ServiceLimits.peer_identity`. SV-4 (Core:437) speaks of a bound "the host marked `required`". The struct in SV-4 has neither. Amendment: "`ServiceLimits` adds `peer_identity: Uid` (default the host uid) and, per rlimit, `{value, required: bool}`."

**B2-49. `StartFailed.reason` and session `Exit.cause` have no value sets.** (medium, 0019) Core:45 `Exit{code, signal, cause}`; LC-4 (Core:58) `StartFailed{reason}`; Core:330. Service causes are listed (SV-5) but session causes are not. HP CA-15 needs "an explicit failure reason". Amendment: "`StartFailReason` = `ExecFailed{errno}`, `CwdMissing`, `StartupTimeout`, `WorkerFailed`, `RegistryFailed`. Session `ExitCause` = `Normal`, `Signal`, `HostStop`, `Killed`, `WorkerLost`, `Other`."

**B2-50. `NotWritten(Refused)` has no meaning, and `Unsupported{what}` is open.** (medium, 0019) IN-2 (Core:77) lists `Refused` with no condition. 5.1A (Core:109, Core:116) gives `what` values `pixel_position` and `produced_text` only. Amendment: "`Refused`: the worker refused the write for a reason that is not a mode, a guard or a size (for example a second file upload, DP-5b). `what` (non-exhaustive) = `pixel_position`, `produced_text`, `named_key`, `wheel_direction`, `coordinate`, `release_event`."

**B2-51. `cancel(op)` does not say which operations it accepts.** (medium, 0019) IN-6 (Core:81) is in the input section and describes input completions. It does not say what `cancel` does for `Start`, `Stop`, `Resize`, `CaptureSnapshot` or `ReadFacts`. Amendment: "`cancel` applies to `WriteInput` only. For any other op it returns `TooLate` if completed, else `Refused(NotCancellable)`."

**B2-52. The `features()` names are incomplete and disagree.** (medium, 0019) 9.3 (Core:353) lists `silence` and OS bounds. 9B (Core:422) says `silence` is the only optional feature. Other clauses use `snapshot_graphics` (Core:194), `focus_report` (Core:218), `size_policy_other` (Core:168), `payload_dies_with_guardian` (Core:441), `service_preamble_versions` (Core:435), `route_transport:stream|webrtc` (Core:491). Amendment: "One table lists every feature name, whether it is per Core or per worker, and whether it is optional."

**B2-53. `attach` state rules and `AttachWebRtc` failures are not stated.** (medium, 0019) OU-1 (Core:142) lists `UnknownSession`, `RouteLimit`, `InvalidInput`, `Unsupported`. It does not say if a `Created`, `Starting`, `Exited` or `Lost` session accepts a route. DP-2 (Core:163) gives `AttachWebRtc` success only; a bad offer or an offer without candidates has no code. Amendment: "`attach` is admitted for `Starting`, `Running` and `Exited` (final model, ST-5); else `WrongState`. `AttachWebRtc` async errors: `InvalidInput{bad_offer}`, `InvalidInput{offer_without_candidates}`, `StartFailed{webrtc}`."

**B2-54. `SpawnRequest` and the color profile have no field types.** (medium, 0019) Core:45 gives `SpawnRequest { argv, env, cwd, size, labels, color_profile? }` with no types. It does not say if `env` is exact or added to an inherited set, the type of `labels`, or the failure for a missing `cwd`. `SetColorProfile` (Core:242) has no profile schema. Amendment: "`env` is exact (as SV-1). `labels: Map<String, String>`. A missing `cwd` is `StartFailed{CwdMissing}`. `ColorProfile { palette: [Rgb; 256]?, foreground: Rgb, background: Rgb, cursor: Rgb? }`."

**B2-55. `ReadScreen` has no argument or result schema.** (medium, 0019) ST-2 (Core:189) says "plain text" and `history_unavailable`. It does not say if the text is the visible screen, the scrollback or both, nor give the result fields. HP SA-1 relies on it. Amendment: "`ReadScreen{session, history: bool}` completes `Screen{text, rows, cols, history_unavailable, model_rev}`; `text` is the visible screen, plus scrollback when `history` is true."

**B2-56. `DeadlineExpired` has no owning operations.** (medium, 0019) Core:340 says "An operation passed its deadline" and names none. Amendment: list the operations that can complete `DeadlineExpired` (for example `Start` after `startup`, `AttachWebRtc` after `connect_deadline`), or remove the code.

### Low

**B2-57. The `EventsLost.kinds` set is not listed.** (low, 0019) Core:208, Core:238 (`Query`), Core:240 (`Tap`). Amendment: "`kinds` ⊆ {`Bell`, `PromptMark`, `Notification`, `ClipboardWrite`, `Query`, `Tap`}, non-exhaustive."

**B2-58. `InputResult.detail` and `PumpReport` have no type.** (low, 0019) Core:77, Core:37. Amendment: "`detail` is a human string; `PumpReport{more: bool, events_posted: u32}`."

**B2-59. `Size` has no range.** (low, 0019) Core:45. Amendment: "`rows` and `cols` are 1 to 65,535; zero is `InvalidInput`."

**B2-60. `set_silence_threshold` has no signature.** (low, 0019) TM-4 (Core:287). Amendment: "`set_silence_threshold(session, Option<Duration>) -> Result<(), CoreError>` (sync; `UnknownSession`, `Unsupported`)."

**B2-61. `ServiceLimits.startup` has no stated end point.** (low, 0019) SV-4 (Core:437), SV-5 `StartupTimeout`. Amendment: "`startup` runs from `exec` until every lane of epoch 1 has authenticated."

---

## C. Checks that passed

- Appendix A and LN-2 hold the same 57 calls (script check). No Amendment 1 call is added; Amendment 1 changes only the manifest file and sources (Amd:36).
- Every Appendix A row has an effect and a commit point. ST-9 fsync is stated for `store.put`, `store.delete`, `store.batch`, `web.*`, `assets.publish`, and `plugins.call` keyed records (PC-4).
- `plugins.bindings`, `hub/plugin.dependency`, `hub/worktree.*`, `hub/session_notification`, `hub/notice`, `net/ws.*`, `event_gap` and the `event` invoke record have full payloads.
- Core `InputResult`, `CancelResult`, `Resize` outcomes, `Lost` reasons, `FocusChanged` (true, false, unknown), session states, route `input_refused` reasons, query reply validation, `ReadFacts` and service exit causes are complete.
