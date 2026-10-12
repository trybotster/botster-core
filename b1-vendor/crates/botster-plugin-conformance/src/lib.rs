//! The Hub→plugin conformance runner (design 6.5; ledger prefix `hp`). Hub-side scripts (`plugin_hub`) play the plugin against a Hub endpoint. Host-side scripts
//! (`plugin_host`) play the Hub against a plugin host with a fixture plugin. A frame is `{"lane", "type", "payload"}`; see [`driver`] for the steps.
//!
//! **One `#[test]` per id (design 6.1).** A Hub repo calls `hub_conformance_tests!(make)` and a plugin-host repo calls `host_conformance_tests!(make)`, where `make` is
//! `Fn(u64) -> Box<dyn HpSubject>`. The `fake` feature holds the adapters that wrap the fakes of `botster-plugin-fakes`.
//!
//! **Test controls.** The controls of a Hub subject are in `HUB_CONTROLS` and the controls of a host subject are in `HOST_CONTROLS`; each transcript lists the ones it
//! uses in `requires.controls`. A real Hub or host implements them with injected parts, never with a test branch in production code (design 6.3).

// Failure values carry whole JSON documents for the report. They occur on a cold path.
#![allow(clippy::result_large_err)]

pub mod driver;
pub mod expand;
#[cfg(feature = "fake")]
pub mod fake;
pub mod schemas;
pub mod subject;

pub use driver::HpDriver;
pub use schemas::HpSchemas;
pub use subject::{Backpressured, HpSubject, Unsupported};

use botster_conformance::{
    load_dir, run_transcript, Limits, SchemaResolver, SeedSet, Selection, StepDriver,
};
use botster_plugin_contract::prelude::Lane;
use include_dir::{include_dir, Dir};

/// The HP transcripts and their fixtures (`conformance/hp`), embedded.
pub static HP_TRANSCRIPTS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/hp");

/// The controls that a Hub subject implements for the transcripts (design 6.3). A control that a transcript names and a subject lacks is `unsupported_control`.
///
/// The argument and result shape of each real-Hub control from `terminal_notification` on, and the clauses that need it. A control returns the JSON that the transcript matches with `expect`.
///
/// | Control | Arguments | Result | Clauses |
/// |---|---|---|---|
/// | `terminal_notification` | `{session, kind: bell\|osc9\|osc777, text?, title?, body?}` | `{}` | HP CA-22 |
/// | `session_notifications` | `{}` | `{raised: [{session, source, title?, body?}]}`: the `hub/session_notification` events raised since the last read | HP CA-22 |
/// | `session_context` | `{session}` | `{env: {name: string}, env_ports: {name: integer\|null}, context: {key: value}}` as the session program reads them; `env_ports` is each environment value parsed as a decimal integer (null when it is not one) | HP CA-14, CA-20 |
/// | `spawned_command` | `{operation_id}` or `{session}` | `{argv: [string], via_shell: bool, env: {name: value}, env_names: [string], uid, hub_uid}` as the started program sees them | HP CA-19, PX-1 |
/// | `hub_log` | `{}` | `{text}`: every Hub log line written so far | HP CA-19, PX-2 |
/// | `process_group_alive` | `{operation_id}` | `{alive: bool}` | HP PX-3 |
/// | `entity_snapshot` | `{family}` | `{entities: [{id, fields: {..}}]}` | HP ST-7 |
/// | `entity_diffs` | `{family}` | `{diffs: [{id, type, fields: {..}}]}` since the last read | HP ST-7 |
/// | `client_input` | `{session, text}` | `{refused?: code}`: a client types into the session | HP CA-18 |
/// | `client_route` | `{session}` | `{available: bool}`: whether the Hub gives a client an input route | HP CA-18 |
/// | `commit_burst` | `{kind: store\|fs, collection?, root?, path?, count}` | `{}`: `count` changes by another writer | HP ST-6, FS-3 |
/// | `admit_manifest` | `{file: "botster-plugin.json"\|"botster-accessory.json", content}` | `{error?: {kind, detail}, approvals?: {scope: name}}`: the package admission of a complete package file (`approvals` maps each scope that the operator must approve to its name; the order is not fixed); `content` is a complete file (the fixtures/frames.json templates `manifest` and `accessory`, with only the field under test changed) | HP CA-20, CA-22, MN-1 |
/// | `await_session` | `{session, state}` | `{}` once the session row has the state | HP CA-20 |
/// | `await_operation` | `{operation_id, state?, terminal?}` | `{}` once the operation record has the state, or is terminal: the barrier that replaces polling | HP PX-3, CA-17 |
/// | `stall_log_sink` | `{}` | `{}`: the Hub log sink stops accepting | HP CA-9 |
/// | `fs_race` | `{root, path, outside_path}` | `{}`: the next open of `path` finds a symlink to `outside_path` | HP FS-2 |
/// | `kill_process_bridge` | `{operation_id}` | `{}`: the process bridge of the command dies | HP PX-2 |
/// | `hold_spawn` | `{}` | `{}`: the next spawn stays `starting` and holds its reservations before the payload starts, until `release_spawn` | HP CA-21 |
/// | `release_spawn` | `{}` | `{}`: the held spawn goes on | HP CA-21 |
/// | `plugin_identity` | `{plugin}` | `{plugin_key, instance}`: the identity that the Hub authenticated for the loaded instance of the plugin (the name of a plugin of the transcript: `demo` or a loaded key) | HP PC-2 |
/// | `rebind` | `{interface, provider}` | `{}`: the Hub changes the resolved provider of the interface for the requiring plugins (a binding change) | HP PC-4 |
/// | `asset_get` | `{url}` or `{origin?: assets\|app\|static, plugin_key, asset_id, token?, method?: GET\|HEAD\|..., headers?}` | `{status, headers: {name: value}, body_b64?}`: a request to the assets origin (default), the app origin or the plugin's static origin; header names are lower case | HP AS-1 to AS-4 |
/// | `client_open_asset` | `{package, key, asset_id?\|name?, viewer: {permissions: [..]}}` | `{url, token, expires_at}`, or `{error: {kind}}`: the client verb `invoke(package, key, open_asset, ..)` for the embed view `key` that the package declares (client A.9, UI-EM-7), issued by a viewer with those permissions; the Hub mints the token in answer, so a transcript never mints one | HP AS-2 |
/// | `plugin_assets` | `{plugin}` | `{assets: [{name, asset_id, content_type, source: staged\|published}]}`: the Hub's asset store of the plugin | HP AS-1, AS-4 |
/// | `package_executions` | `{plugin}` | `{started}`: the processes or plugin code that the Hub started while it staged the package | HP AS-1 |
/// | `configure_assets_origin` | `{listen}` | `{error?: {kind, detail?}}`: the Hub parameter `assets_listen` is set; an origin that the clause refuses is an error at configuration | HP AS-2 |
/// | `app_api_get` | `{path, bearer?, query?: {name: value}}` | `{status}`: a request to the app's own API with the credential that the transcript presents | HP AS-2 |
/// | `asset_preflight` | `{url, origin}` | `{status, headers}`: a CORS and Private Network Access preflight from the origin | HP AS-2 |
/// | `browser_embed` | `{app_origin, url}` | `{loaded, mixed_content_blocked, preflight_accepted}`: a real browser embeds the asset url in a page of the app origin (acceptance fixture of AS-2) | HP AS-2 |
/// | `browser_render` | `{app_origin, url, click?, probe?, hub_origin?}` | `{inline_script_ran, inline_style_applied, attempted_external_requests, external_requests, form_submissions, navigation_requests, hub_requests, credentials_in_frame: [..], referer_sent}`: a real renderer loads the asset page in a sandboxed frame inside a page of the app origin, clicks the element `click` if named, reads the computed style of the element `probe` if named, and reports the requests that the page attempted and the ones that left the frame (slow tier: a real browser) | HP AS-3 |
/// | `embed_action` | `{package, key, action, viewer: {permissions: [..]}}` | `{performed, authorized_by?}`: the client-mediated action of an embed frame (UI-EM-5), authorized by the viewer's own permission | HP AS-3 |
/// | `plugin_lifecycle` | `{plugin, action: disable\|uninstall\|quarantine}` | `{}`: the plugin is unloaded by that act (LD-6, LD-7) | HP AS-4 |
/// | `package_update` | `{plugin, staged_assets?: [{name, path, content_type}], interface_hash?}` | `{}`: the package files of the plugin change on disk (and the interface hash that it provides, if named); the next reload takes them | HP AS-4, PC-4 |
/// | `edge_sync_before_dispatch` | `{record, operation_id, dispatch_to}` | `{synced_before_first_dispatch}`: observed at the injected storage edge and lane edge, one counter across them. True when a sync of the named record of that operation completed before the first frame that carries the call to the plugin `dispatch_to` was written; further syncs of the record are allowed, and a missing sync or one that completes after that frame gives false | HP PC-4 |
/// | `client_verb` | `{kind: package\|catalog\|hub, verb: create\|delete\|invoke, id?, args?, as?: {kind: operator} \| {kind: agent, scopes: [..]} \| {kind: plugin, plugin_key}}` | `{accepted: {operation_id}}` (accepted durable), `{committed: true}`, `{result, text}` (a read: `text` holds the JSON text of each top-level field of `result`, so a transcript can look for a name inside a field whose shape the contract leaves open) or `{error: {kind, detail?}}`: a verb of the Hub-client contract for the named caller (HP PM-13); for `package` an `invoke` carries `args.action` (`preview_upgrade`, `upgrade`, `pin`, `unpin`, `rollback`, `approve_grants`, `enable`, `disable`, `status`) and for `catalog` `refresh` | HP PM-1 to PM-14, MP-1 |
/// | `client_read` | `{kind: package\|catalog\|operation\|session_type, id?, package?, latest?}` | the row, or `{rows: [..]}` without `id`, or `{error: {kind: not_found}}`: a read of the client contract. A package row carries `kind`, `status`, `source`, `lock` (PM-3), `pinned_ref` and `pending_grants`; an `operation` read with `package` and `latest` gives the latest durable operation record of that package (`state`, `error`) | HP PM-1 to PM-14 |
/// | `await_package` | `{package, status?, settled?}` | `{}` once the package row has the status, or once the operation on it has ended (`settled`): the barrier that replaces polling | HP PM-1 to PM-14 |
/// | `net_cancelled` | `{origin}` | `{cancelled}`: the requests to the origin that the I/O edge saw the Hub cancel (the connection or the stream closed by the Hub) | HP NT-10 |
/// | `await_catalog` | `{catalog, settled}` | `{}` once the add or the refresh of the catalog has ended | HP PM-1 to PM-14 |
/// | `script_git` | `{url, refs: {<ref>: {commit, files: {path: {text\|b64\|json\|link\|special, mode?, mtime?, uid?}}, empty_dirs?, submodules?, hooks?}}}` | `{}`: the injected git source edge serves these refs (a ref is a branch, a tag or a commit); no real network (HP A3-1). A `json` file is serialized compact (`separators (",", ":")`, the key order given, UTF-8): the same byte rule as the content hash of PM-3. `hooks` names the hooks that the repository would run if the Hub ran any | HP PM-1 to PM-14 |
/// | `script_catalog` | `{url, document, etag?}` | `{}`: the injected catalog edge serves the document (with the ETag, and a 304 on a conditional request) | HP PM-2, PM-4 |
/// | `script_resolve` | `{host, addresses}` | `{}`: the injected DNS edge resolves the host to these addresses | HP PM-2, PM-4, MP-3, MP-11 |
/// | `script_redirect` | `{url, to}` | `{}`: the injected I/O edge answers a request to the url with a redirect | HP PM-2, PM-4 |
/// | `edge_requests` | `{edge: git\|catalog\|connections, since?}` | `{requests: [{url?, commit?, ref?, headers?, via, address?, host?, operation?}], mark}`: what the injected edge received since the mark; `via` names the component that opened the connection (`io_service`), `mark` is the count so far, and `connections` lists every connection with its resolved address | HP PM-1 to PM-4, PM-7, MP-3, MP-11 |
/// | `local_dir` | `{name, files}` | `{path}`: a directory of the operator in the test's temporary root | HP PM-1, PM-6 |
/// | `local_dir_write` | `{path, file, text}` | `{}`: the operator changes a file of a local directory | HP PM-1, PM-6 |
/// | `package_file_write` | `{package, file, text}` | `{}`: a file of the installed package changes on disk | HP PM-6, PM-11, PM-14 |
/// | `package_files_wipe` | `{package}` | `{}`: the installed files of the package are deleted | HP PM-3 |
/// | `package_cache_wipe` | `{package}` | `{}`: the Hub's local copies of the package's earlier versions (its cache and the files of the lock history) are deleted, so a rollback has to fetch the locked commit | HP PM-4, PM-9 |
/// | `package_files` | `{package}` | `{version, files: [..]}`: the active package directory as the Hub would load it (the version of its manifest) | HP PM-7 |
/// | `package_escape_probe` | `{package, path}` | `{exists}`: whether the path, taken relative to the package directory, exists after the install attempt | HP PM-4 |
/// | `execution_count` | `{}` | `{started}`: the processes or plugin code that the injected process edge saw the Hub start since the last read | HP PM-2, PM-4, PM-5, PM-11 |
/// | `storage_hold` | `{record, point: before_write\|after_write}` | `{}`: the injected storage edge parks the next write of the record before it reaches the medium, or after it is synced and before it is acknowledged; a following `hub_restart` is the kill | HP PM-7 |
/// | `storage_writes` | `{record, since?}` | `{writes: [{synced}], mark}`: the writes and syncs that the storage edge saw for the record since the mark (the count so far); `since` limits the list to a stage of the run | HP PM-7 |
/// | `await_storage_hold` | `{record, point}` | `{}` once the storage edge has parked the write that `storage_hold` armed (before it reached the medium, or after its sync): the barrier before a kill | HP PM-7 |
/// | `storage_dir_synced_before_lock_write` | `{package, since}` | `{synced_before_lock_write}`: true when the storage edge saw a sync of the new package directory and its files before the write of the lock record since the mark | HP PM-7 |
/// | `await_client_operation` | `{operation_id, settled}` | `{}` once the durable operation of that id has ended | HP PM-4 |
/// | `lock_record_stored` | `{package}` | `{current: {version}, history: [{version, limit}]}`: the lock record as the storage edge holds it, the current lock and the earlier ones newest first, each with the limit it was admitted under | HP PM-3, A7-1 |
/// | `set_limit` | `{limit, value}` | `{}`: the operator changes the Hub parameter live (A2-52) | HP A7-1 |
/// | `lane_transcript` | `{plugin, contains?}` | `{text, contains}`: every byte the Hub wrote to the plugin's lanes, read at the lane edge, decoded as text; `contains` says whether that text holds the given string (a value that a transcript has bound, such as a path or a token) | HP PM-1, MP-5, MP-9 |
/// | `target_dir` | `{target_id, files}` | `{path}`: a target directory that holds a package (CP-8) | HP PM-12 |
/// | `discover_targets` | `{}` | `{}`: the Hub scans the targets for packages | HP PM-12 |
/// | `real_network_attempts` | `{}` | `{count}`: the connections that the Hub tried to open outside the injected edges | HP PM-14, MP-10 |
/// | `real_git_source` | `{name, files_v1, files_v2}` | `{url, commit_v1, commit_v2}`: a real git repository with two commits (slow tier: real git source) | HP PM-14 |
/// | `real_catalog` | `{name, entries}` | `{url}`: a real catalog served over loopback (slow tier) | HP PM-14 |
/// | `script_mcp` | `{plugin, server, transport: stdio\|http, tools?, pages?, protocol_version?, capabilities?, session_id?, set_cookie?, url?, hang_on?, exits_on_start?, endless_pages?, stderr?, server_requests?, redirect_to?, get_stream?}` | `{}`: the injected edge of the MCP server of that plugin (a fake process for stdio, the scripted I/O service for http); a tool has `behavior` (`echo`, `hang`, `oversize`, `is_error`, `structured`, `sse_answer`, `change_list`) | HP MP-2 to MP-15 |
/// | `await_mcp_server` | `{plugin, server, state: available\|unavailable, settled?}` | `{}` once the server has the state: the barrier that replaces polling | HP MP-2 to MP-15 |
/// | `await_mcp_list` | `{plugin, server, generation}` | `{}` once the server's list is at that generation | HP MP-4, MP-14 |
/// | `mcp_server_log` | `{plugin, server, since?, ordered?: [matcher], wire_contains?}` | `{in_order, wire, wire_contains, messages: [{dir: in\|out, method?, id?, params?, result?, headers?}], calls: [..], cancelled: [..], list_requests, calls_all_instances: [..], mark}`: the JSON-RPC messages that the injected edge received (`in`) and sent (`out`); `calls` are the `tools/call` requests, `calls_all_instances` those of every instance of the server; `ordered` asks whether those messages, in that order and with other messages between them allowed, appear in the log (`in_order`); `wire` is the serialized text of every message that the edge received, and `wire_contains` (asked with the argument `wire_contains`) says whether that text holds the given string | HP MP-2 to MP-15 |
/// | `mcp_server_state` | `{plugin, server}` | `{state, error?: {kind, detail?}}`: the status of the server that the Hub shows the operator | HP MP-2 to MP-15 |
/// | `await_mcp_message` | `{plugin, server, method, tool?, since?}` | `{}` once the injected edge has received that JSON-RPC method (for `tools/call`, for that tool) since the mark `since` (the whole run without it) | HP MP-10, MP-12 |
/// | `await_mcp_stderr` | `{plugin, server, bytes}` | `{}` once the edge has written that many bytes to the child's stderr | HP MP-9 |
/// | `mcp_session_token` | `{session}` | `{token}`: the MCP session token of the session, as the operator's own client reads it | HP MP-5 |
/// | `mcp_server_event` | `{plugin, server, event: exit\|fail\|restart\|list_changed\|release_page\|answer_slow\|old_instance_answer, ..}` | `{}`: a stimulus at the injected edge (the child exits, the connection fails, a notification, a held page is released, a held slow call is answered, a late answer of a previous instance) | HP MP-4 to MP-15 |
/// | `bridge_census` | `{}` | `{bridges: [{plugin, server, started_by, first_party, group, child_parent, child_same_group, child_env_names, child_env, child_has_core_socket, started_after}], children, open_http_connections, distinct_process_groups}`: the injected process edge's census of the `mcp-bridge` services and their children | HP MP-3, MP-7, MP-10 |
/// | `bridge_starts` | `{plugin, server, window_ms?}` | `{starts}`: how many times the process edge started the server's bridge; with `window_ms`, only the starts in the half-open window of that length that begins at the first start the edge saw | HP MP-8 |
/// | `child_alive` | `{plugin, server}` | `{alive}`: whether the real stdio child of the server still runs (slow tier: real processes) | HP MP-3 |
/// | `set_secret` | `{plugin, name, value}` | `{}`: the operator sets the value of a secret that the plugin's manifest declares (NT-5) | HP MP-5, MP-9 |
/// | `mcp_cancel` | `{ticket}` | `{}`: the MCP client cancels the call of the ticket | HP MP-10 |
/// | `real_mcp_servers` | `{plugin, stdio, http}` | `{}`: one real stdio MCP server and one real http MCP server for the plugin's declared servers (slow tier: real MCP server) | HP MP-10 |
/// | `mcp_list` | `{kind: tools\|prompts\|resources, plugin?}` | `{items: [{name, ..}]}`: the MCP list that the Hub shows a client (`tools/list`, `prompts/list`, `resources/list`), optionally only the items of one plugin; a tool is shown as `<plugin_key>__<name>` | HP MT-1, MT-3 |
/// | `mcp_call` | `{tool, arguments, client?: {token_of_session}}` | `{ticket}`: an MCP `tools/call` by a client (the MCP session token of a session, or the operator) is admitted; the control returns at once and never waits for the handler | HP MT-4, MT-5, MT-6 |
/// | `mcp_response` | `{ticket}` | `{is_error, error?: {kind, detail?}, content?}`: waits on the real MCP response to that call (a marked deadline timer) and returns it | HP MT-4, MT-5, MT-6 |
/// | `hub_feature` | `{name}` | `{in_hello}`: whether the feature name is in the `features` of the `Hello` that the Hub sent | HP MT-7 |
/// | `watches` | `{}` | `{store: n, fs: n}`: the live watches of the plugin | HP ST-6, FS-3 |
///
/// The controls of feature hp-events (HP section 10; the stimuli put Hub, terminal and client state in motion and the observers read a boundary that the plugin does not see). FakeHub implements only `emit_events` (and the existing `status`, `published`, `diagnostics` shapes below); a transcript that uses any other one is `fake: structural`.
///
/// | Control | Arguments | Result | Clauses |
/// |---|---|---|---|
/// | `emit_events` | `{emitters: [{owner, name, count, priority?: high\|normal\|low, key?, handler?}]}` | `{}`: the Hub's ingress receives `count` copies of each emitter's event; each copy's payload is `{seq, emitter}` (`seq` counts the copies of that emitter in this call, `emitter` is its index in the list; an emitter with `count` 0 only holds an index) | HP EV-4, EV-5, EV-10 to EV-13 |
/// | `raise_hub_event` | `{name, subject?, payload}` | `{}`: the Hub raises one Hub-owned event (owner `hub`) as the state change of its own would | HP EV-5, EV-9, EV-10, EV-12, EV-15 |
/// | `terminal_feed` | `{session, kind: title\|cwd\|bell\|notification\|prompt_mark\|modes\|exit\|output, ...fields of the A2-6 payload of the kind (title: text; cwd: cwd; notification: source, title?, body; prompt_mark: mark, exit_code?; modes: modes; exit: code?, signal?, cause; output: bytes_b64)}` | `{}`: Core's per-session metadata feed (Core EV-1, EV-7) or its raw output yields one item | HP EV-2, EV-9, EV-10, EV-13, EV-14, EV-15 |
/// | `hold_ingress` | `{}` | `{}`: the Hub's injected scheduler stops draining the event ingress queue (`ingress_queue` copies stay in it) | HP EV-3 |
/// | `release_ingress` | `{}` | `{}`: the scheduler drains the ingress queue again | HP EV-3 |
/// | `core_feed_unread` | `{session}` | `{unread: n}`: the items of Core's feed of the session that the Hub has not ingested yet | HP EV-10 |
/// | `core_feeds` | `{}` | `{feeds: [{session}]}`: one entry per Core metadata feed that the Hub holds | HP EV-14 |
/// | `subscriptions` | `{plugin?}` | `[{plugin, generation, handler_id, pattern: {owner, name}}]`: the subscriptions with the verified subscriber identity | HP EV-2 |
/// | `session_change` | `{session, label?}` | `{}`: a client changes the session row (the Hub emits the delta of `hub/session_family`) | HP EV-6 |
/// | `hub_worktree` | `{verb: create\|delete, target_id, branch, fail?: bool}` | `{}`: a client creates or deletes a worktree through the Hub; `fail` makes the operation fail | HP EV-8 |
/// | `client_events` | `{client}` | `{events: [{owner, name, subject?, payload}]}`: the Hub events that the Hub sent to the connected client since the last read | HP EV-9 |
/// | `diagnostics` | `{plugin?}` | adds `load_error: {kind, detail?}` to its result: the typed reason of a failed load | HP EV-2, EV-9, EV-11, EV-14, EV-15 |
///
/// The controls of feature hp-events for the outside world and the web plugin (HP sections 9.8 and 9.12). The Hub's I/O service reaches the world through an injected edge; `net_world` scripts that edge and the observers read what the edge saw, so a transcript never trusts the Hub's own account of its I/O. FakeHub implements none of them (`fake: structural`).
///
/// | Control | Arguments | Result | Clauses |
/// |---|---|---|---|
/// | `net_world` | `{endpoints: [{origin, addresses: [ip], then_addresses?: [ip], answers?: [{status, headers?: [[name, value]], body_b64?, hold?: bool, echo_header?}], fail?: net_dns\|net_connect\|net_tls, ws?: {accept: bool, stall_reads?: bool}}]}` | `{}`: the scripted outside world; `addresses` is what the name resolves to at the first resolution, `then_addresses` at every later one; `answers` are consumed per request (the last repeats); `hold` keeps the answer until `net_release`; `echo_header` answers with the value of that request header as the body | HP NT-1 to NT-9, WP-4b |
/// | `net_seen` | `{origin?, path_and_query?, path_prefix?}` | `{requests: [{origin, method, path_and_query, headers: [[name, value]], header_map: {lowercase name: value}, body_b64, plugin, connection}], connects: [{origin, address}], plugin_sockets: n}`: what the edge saw since the start, `requests` limited to those that match the filter (`connection` identifies the pooled connection; `header_map` holds the last value of each header name) | HP NT-1 to NT-9 |
/// | `net_close_idle` | `{origin}` | `{}`: the edge closes every idle pooled connection to the origin, so the next request connects and resolves the name again | HP NT-2 |
/// | `net_release` | `{origin}` | `{}`: the held answers of the origin are sent | HP NT-3, NT-6, NT-8 |
/// | `net_push` | `{origin, text?, data_b64?}` | `{}`: the WebSocket peer of the origin sends one message | HP NT-4 |
/// | `net_close` | `{origin, code?, reason?, drop?: bool}` | `{}`: the WebSocket peer closes (or the connection drops with `drop`) | HP NT-4 |
/// | `net_state` | `{plugin?}` | `{open_requests: n, open_sockets: n}`: the requests and sockets the I/O service holds for the plugin | HP NT-3, NT-8 |
/// | `hub_output` | `{}` | `{log, events, receipts, errors, call_results}`: five texts, everything the Hub wrote to its log, the events it produced (JSON), the metadata of its operation receipts (JSON), the error messages and details it returned (JSON) and the results of the host calls it answered (JSON) since the start | HP NT-5, NT-9, WP-6 |
/// | `webhook_post` | `{plugin, path, headers: [[name, value]], body_b64}` | `{request}`: a local client posts to `POST /plugins/<plugin>/<path>` on the Hub's listener; it returns at once | HP NT-7, NT-8 |
/// | `webhook_answer` | `{request}` | `{status, headers, body_b64}`: the HTTP answer, once it is sent | HP NT-7, NT-8 |
/// | `listener` | `{}` | `{bound: [address]}`: the addresses the Hub's listener is bound to | HP NT-7, WP-5 |
/// | `client_invoke` | `{kind, id?, verb, args?}` | `{ok, value?, error?}`: an authenticated operator client calls `invoke(kind, id, verb, args)` over the client protocol; for `issue_pairing`, `value.url_origin` is the `scheme://host[:port]` part of `value.url` | HP WP-1 |
/// | `peer_connect` | `{origin?, relay_id?, authenticated: bool, delivered?: bool}` | `{accepted: bool}`: a client peer connects; `authenticated` is whether it completes the PR-4 to PR-9 checks | HP WP-1, WP-3, WP-6 |
/// | `peer_signal` | `{relay_id, envelope_b64}` | `{connection_id}`: the client's PR-4 message for the attempt reaches the Hub, which hands it to the plugin | HP WP-2, WP-3, WP-4 |
/// | `peer_signal_seen` | `{relay_id}` | `{inbound: [{envelope_b64}]}`: the envelopes that the Hub took from the plugin for the attempt | HP WP-2 |
/// | `relay_outcome` | `{relay_id, outcome: completed\|failed}` | `{}`: the PR-4 exchange of the attempt ends | HP WP-2 |
/// | `relay_attempts` | `{}` | `{attempts: [{relay_id, connection_id, plugin?}]}`: the live attempts (`plugin` absent for an attempt the Hub itself opened on a served origin) | HP WP-2 |
/// | `hold_relay_ingress` | `{}` | `{}`: the Hub's inward relay path stops taking envelopes | HP WP-4 |
/// | `release_relay_ingress` | `{}` | `{}`: the inward relay path goes on | HP WP-4 |
/// | `web_request` | `{plugin?, origin?, method, path?, route?: ice\|signal, headers?, body_b64?}` | `{status, headers: {lowercase name: value}, body_b64?}`: an HTTP request to the served origin of the plugin (or to `origin`); `route` names the Hub's own `/hubs/<hub_id>/ice` or `/signal` | HP WP-1, WP-2, WP-5, WP-6 |
/// | `web_origins` | `{}` | `{origins: {plugin key: origin}}`: the served origin of each plugin | HP WP-1, WP-5 |
/// | `peer_config` | `{}` | `{hub: {ice_servers}, worker: {ice_servers}}`: the ICE configuration of the Hub's and the workers' own peer connections | HP WP-6 |
/// | `await_operation` | adds `plugin?` | | the operation of that plugin (HP NT-3, NT-4) |
pub const HUB_CONTROLS: &[&str] = &[
    "status",
    "completions",
    "invoke",
    "cancel",
    "counters",
    "received",
    "published",
    "hold_lane",
    "release_lane",
    "settle",
    "expire_drain",
    "reload",
    "unload",
    "service_connected",
    "service_exited",
    "service_lost",
    "hub_restart",
    "load_plugin",
    "open_subscription",
    "hold_commits",
    "release_commits",
    "diagnostics",
    "descriptors",
    "emit_events",
    // Real-Hub observers and stimuli of the host call transcripts (feature hp-calls-a, steward ruling R-1.3). FakeHub does not implement them: a transcript that
    // uses one is `fake: structural`, and a real Hub adapter maps each with injected parts. The shapes are in the table of the doc comment of `HUB_CONTROLS`.
    "terminal_notification",
    "session_notifications",
    "session_context",
    "spawned_command",
    "hub_log",
    "process_group_alive",
    "entity_snapshot",
    "entity_diffs",
    "client_input",
    "client_route",
    "commit_burst",
    "admit_manifest",
    "await_session",
    "await_operation",
    "stall_log_sink",
    "fs_race",
    "watches",
    "kill_process_bridge",
    "hold_spawn",
    "release_spawn",
    // Feature hp-events (HP section 10): see the second table of the doc comment above.
    "raise_hub_event",
    "hold_ingress",
    "release_ingress",
    "terminal_feed",
    "core_feed_unread",
    "core_feeds",
    "subscriptions",
    "session_change",
    "hub_worktree",
    "client_events",
    // Feature hp-events (HP sections 9.8 and 9.12): the third table of the doc comment above.
    "net_world",
    "net_seen",
    "net_release",
    "net_push",
    "net_close",
    "net_close_idle",
    "net_state",
    "hub_output",
    "webhook_post",
    "webhook_answer",
    "listener",
    "client_invoke",
    "peer_connect",
    "peer_signal",
    "peer_signal_seen",
    "relay_outcome",
    "relay_attempts",
    "hold_relay_ingress",
    "release_relay_ingress",
    "web_request",
    "web_origins",
    "peer_config",
    // hp-plugins (feature hp-plugins, steward ruling R-1.3): real-Hub observers and stimuli of the plugin-call, asset, tool and package transcripts.
    "plugin_identity",
    "rebind",
    "asset_get",
    "client_open_asset",
    "plugin_assets",
    "package_executions",
    "configure_assets_origin",
    "app_api_get",
    "asset_preflight",
    "browser_embed",
    "browser_render",
    "embed_action",
    "plugin_lifecycle",
    "package_update",
    "mcp_list",
    "mcp_call",
    "mcp_response",
    "edge_sync_before_dispatch",
    // hp-plugins batch 2 (packages, catalogs, MCP proxies): the client verbs of the package and catalog kinds, the injected edges (git source, catalog, DNS, MCP servers, storage, process), and their observers
    "await_catalog",
    "await_mcp_list",
    "await_mcp_server",
    "await_package",
    "bridge_census",
    "bridge_starts",
    "child_alive",
    "client_read",
    "client_verb",
    "discover_targets",
    "edge_requests",
    "execution_count",
    "lane_transcript",
    "local_dir",
    "local_dir_write",
    "mcp_cancel",
    "mcp_server_event",
    "mcp_server_log",
    "mcp_server_state",
    "package_escape_probe",
    "package_file_write",
    "package_files",
    "package_files_wipe",
    "package_cache_wipe",
    "real_catalog",
    "real_git_source",
    "real_mcp_servers",
    "real_network_attempts",
    "script_catalog",
    "script_git",
    "script_mcp",
    "script_redirect",
    "script_resolve",
    "set_secret",
    "storage_hold",
    "storage_writes",
    "await_storage_hold",
    "storage_dir_synced_before_lock_write",
    "await_client_operation",
    "lock_record_stored",
    "set_limit",
    "await_mcp_message",
    "await_mcp_stderr",
    "mcp_session_token",
    "net_cancelled",
    "target_dir",
    "hub_feature",
];

/// The controls that a host subject implements for the transcripts (design 6.3).
pub const HOST_CONTROLS: &[&str] = &[
    "settle",
    "status",
    "counters",
    "received",
    "hold_lane",
    "release_lane",
    "reconnect",
    "restart_process",
    "memory",
];

pub(crate) fn lane_name(l: Lane) -> &'static str {
    match l {
        Lane::Control => "control",
        Lane::Invoke => "invoke",
        Lane::Call => "call",
        Lane::Bulk => "bulk",
    }
}

pub(crate) fn lane_byte(name: &str) -> Option<u8> {
    Lane::ALL
        .into_iter()
        .find(|l| lane_name(*l) == name)
        .map(Lane::byte)
}

/// Runs one transcript by id for a generated `#[test]`. Panics with the report line unless it passed.
///
/// The seed set and the selection come from `BOTSTER_SEEDS`, `BOTSTER_ONLY`, `BOTSTER_CLAUSE` and `BOTSTER_SEED`.
pub fn run_test(
    crate_name: &str,
    dir: &'static Dir<'static>,
    id: &str,
    make: &dyn Fn(u64) -> Box<dyn HpSubject>,
    schemas: &dyn SchemaResolver,
) {
    run_test_with(crate_name, dir, dir, id, make, schemas);
}

/// [`run_test`] for a suite whose transcripts live in `dir` and whose fixtures and frame templates (`fixtures/frames.json`) live in `templates`
/// (the Lua binding suite: its transcripts are in `conformance/lb`, and it shares the HP templates).
pub fn run_test_with(
    crate_name: &str,
    dir: &'static Dir<'static>,
    templates: &'static Dir<'static>,
    id: &str,
    make: &dyn Fn(u64) -> Box<dyn HpSubject>,
    _schemas: &dyn SchemaResolver,
) {
    // Only this id's file is parsed: a test runs in its own process, and a parse of the whole directory would cost every test all of them.
    let mut t = botster_conformance::load_one(dir, id)
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_else(|| panic!("no transcript with id {id}"));
    let selection = Selection::from_env();
    if !selection.selects(&t) {
        return;
    }
    let templates_json: serde_json::Value = templates
        .get_file("fixtures/frames.json")
        .and_then(|f| f.contents_utf8())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    let expand = |v: &serde_json::Value| {
        expand::expand(v, &templates_json).unwrap_or_else(|e| panic!("{id}: {e}"))
    };
    t.steps = t.steps.iter().map(expand).collect();
    t.setup = expand(&t.setup);
    let seeds = selection.seeds(&SeedSet::from_env());
    // The frame schemas compile once per process: every transcript of a run shares them (design 9, test speed).
    static SCHEMAS: std::sync::OnceLock<HpSchemas> = std::sync::OnceLock::new();
    let schemas = SCHEMAS.get_or_init(HpSchemas::new);
    let factory =
        |seed: u64| -> Box<dyn StepDriver> { Box::new(HpDriver::new(make(seed), templates)) };
    // A suite statement (design 5.5): `run_suite{subject, filter?}` runs every transcript of that subject and passes only if each one passes.
    if let Some(spec) = t.steps.iter().find_map(|s| s.get("run_suite")) {
        let subject = spec["subject"].as_str().unwrap_or("").to_string();
        let filter = spec
            .get("filter")
            .and_then(|f| f.as_str())
            .unwrap_or("")
            .to_string();
        // One seed of the run: each inner transcript already runs under every seed as its own test, so the statement adds the claim and not the cost.
        let one = SeedSet {
            seeds: seeds.seeds.iter().take(1).copied().collect(),
        };
        let mut failures = Vec::new();
        let mut ran = 0;
        for inner in load_dir(dir).unwrap_or_else(|e| panic!("{e}")) {
            let is_subject = serde_json::to_value(inner.subject)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .as_deref()
                == Some(subject.as_str());
            // A suite statement never runs another suite statement, so it cannot recurse.
            let nested = inner.steps.iter().any(|s| s.get("run_suite").is_some());
            if !is_subject || nested || !inner.id.starts_with(&filter) {
                continue;
            }
            let mut inner = inner;
            inner.steps = inner.steps.iter().map(expand).collect();
            inner.setup = expand(&inner.setup);
            ran += 1;
            let outcome = run_transcript(&inner, &factory, &one, schemas, &Limits::default());
            if !outcome.is_pass() {
                failures.push(botster_conformance::report::describe(&inner.id, &outcome));
            }
        }
        assert!(
            ran > 0,
            "{id}: run_suite found no transcript of subject '{subject}'"
        );
        assert!(
            failures.is_empty(),
            "{id}: {} of {ran} transcripts of subject '{subject}' did not pass\n{}",
            failures.len(),
            failures.join("\n")
        );
        return;
    }
    let outcome = run_transcript(&t, &factory, &seeds, schemas, &Limits::default());
    if !outcome.is_pass() {
        panic!(
            "{}\n{}",
            botster_conformance::report::describe(id, &outcome),
            botster_conformance::report::reproduce(
                crate_name,
                &botster_conformance::report::test_name(id),
                &outcome
            )
        );
    }
}

include!(concat!(env!("OUT_DIR"), "/hp_tests.rs"));
