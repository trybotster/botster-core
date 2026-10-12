# Hub→client contract v1 — FINAL (2026-09-30), for Sol Hub A's final ACK

Owners: plan auditor (Claude Opus, drafts) and Sol Hub A (Codex, challenges). Date: 2026-09-30. Status: **joint-package candidate** (fire-and-forget input, file input, one tool-naming rule, the plugin-parity client fields, the user decisions of 2026-09-30). The hash set is `contracts-v1-joint-manifest.md`. Earlier closed items:
- **Route codec:** revision 26, `route-codec-v1-extract-rev26.md` (immutable), sha256 `bf59b2b605e21b7e59b8d191cebdac5244241414d5d8f5c1372a86ece52a7abf` (fire-and-forget input and file input; batched for the joint freeze). Revision 14 (`cd80cddf…4654`) keeps Sol Hub A's PASS until then.
- **Core** and **HP (Hub→plugin):** cited by contract name and clause id only. **Core, HP and this contract never cite each other by hash or file name**: either kind of pin goes stale with every revision and makes a cycle. `contracts-v1-joint-manifest.md` maps each contract name to its file and sha256.
- **ui-contract** (v1.0 with its MINOR amendments 1.1 and 1.2, feature `ui_1_1` and `ui_embed`): cited **by contract name and clause id only**, like Core and HP. `contracts-v1-joint-manifest.md` maps each ui-contract version to its file and hash.
- **Closed items:**
  - SW (ruling B, `model_rev` guard);
  - EV (matched to HP A4r29);
  - BP and T3;
  - TB-0 bindings, with the TB-L2 token handshake;
  - the per-route limits;
  - the gate 2, 3 and 5 client items.
- **May still reopen as an amendment:** the plugin parity audit (the zero-regression requirement) may add gate-3 items.

All other clauses are accepted by the plan auditor (Claude) and Sol Hub A (Codex). The earlier freeze predates the user's reversals of 2026-09-30, and those reversals are applied. This is a design review only: no conformance implementation or product test has run.
Inputs:
- decision 6 (verbs over typed resources with `describe`, features at connect, minimum = current−1, no mechanism on the wire);
- the scope audit §4 and §4.b;
- the TUI and Web handoffs (`handoffs/tui-paused-20260930.md`, `handoffs/web-paused-20260930.md`);
- Hub `crates/botster-hub-client` and `docs/client-protocol.md` at `f2c61ee3`;
- the vault notes *botster hub client state sync is entity frame only* and *botster entity snapshots are authoritative reconnect baselines*;
- the Core contract draft 3, which sets the same form.

**Draft 2 applies Sol Hub A's first constraints:**
- offered and selected versions and features;
- terminal format selection at connect, so clients never see Core-internal versions;
- a Hub incarnation that scopes cursors and routes;
- committed versus accepted writes, and retry after a lost response;
- stale-descriptor refusal;
- a defined snapshot/live cut;
- input and paste retry semantics;
- MCP pinned by reference.

It also applies the two client inventories (TUI at `78a96a4`, Web at `029a952`).

**References.** `HP` means the Hub→plugin contract, and `Core` means the Core contract. `contracts-v1-joint-manifest.md` maps each name to its file and hash. Citations name **clause ids**, never revisions, so an amendment on one side needs no change here unless a cited clause changes meaning (gate 5).

Tags (**non-normative**): **[today]** = the current Hub already does this; **[new]** = to be built. **Tags, the "Replaces today" column and every mention of today's code are non-normative** (user rule, 2026-09-30: "ground up design of north star, then steal code that is worth stealing"). Each clause states ideal behavior, and reuse is decided at build time, per clause, against its conformance tests. The rebuild is from scratch; today's code is a source of proven behavior only.

---

## 1. Scope and principles

- **P1. Everything a client shows is a watched resource.** Clients hold no request-and-poll state. A list a client displays is a `watch`. This removes the Web-local `botster-web.*` projections of apps, packages, spawn targets and status.
- **P2. Few verbs over typed resources.** `describe | list | get | watch | unwatch | create | update | delete | invoke`, plus `cancel` of an outstanding request. Plus one terminal stream per attached route.
- **P3. No mechanism on the wire.** No counters, budgets, credits, generations, epochs, queue ages, reservation TTLs or occupancy oracles. Diagnostics are one opaque value with no compatibility promise.
- **P4. Additive and feature-detected.** Additions are new kinds, new actions, new optional fields or new feature names. The Hub accepts the current and the previous protocol version.
- **P5. References carry the hub** (the federation rule; the user's rule in the decisions file):
  - A session identity is a `SessionRef { hub_id, session_id }`.
  - Any other resource is a `ResourceRef { hub_id, kind, id }`.
  - The one conversion: the session `{hub_id, session_id}` is `{hub_id, kind: "session", id: session_id}`.
  - **One Hub per machine** (user decision, 2026-09-30): it is the end state, and v1 has no multi-local-Hub behavior. `hub_id` stays in references only for a future cross-machine federation, which is not in v1. Only the local `hub_id` is accepted in v1. A foreign one gives `remote_hub_unsupported`.
  - **Nothing cloud-specific** (user decision, 2026-09-30): a cloud integration is a plugin and a separate product. This contract names no cloud service.
- **P6. Stable names.** A client hard-codes only the names in the kind registry (section 6): kinds, actions and core fields. Everything else is rendered generically from `describe`.
- **P7. Terminal payloads are Core's, and envelopes are the Hub's.** The Hub owns the route lifecycle, the framing, the input operations and their results. Core owns the baseline, output and resync payload encodings, and the Hub carries those payloads verbatim.
- **P8. No polling.** Every client wait ends on a frame, an EOF or a transport event.
- **P9. Terminal transparency (user ruling, 2026-09-30).** Using Botster must feel like using the terminal directly.
  - Every terminal feature passes through to clients **by default, with no opt-in**. The `output` payload carries the program's bytes **unmodified**: OSC 52 clipboard writes, OSC 8 hyperlinks, OSC 9 and 777 notifications, the bell, the title, the cwd, OSC 133 prompt marks, every mode, and every other sequence. A client implements each one as a native terminal would.
  - The Hub and Core never filter or rewrite these bytes. **The single owner of each side effect is the client**, which acts on the raw sequence in `output`. There is no separate clipboard or notification frame, so no effect happens twice. A client reports an actual platform refusal (for example, a browser denying clipboard permission); no Botster policy blocks it.
  - **A query that needs a reply is answered exactly once, by the active client first, then by the shadow terminal** (user decision, 2026-09-30). This covers **every** reply-needing query, including model queries (device attributes, DSR and CPR, DECRQM, XTVERSION, color queries, DECRQSS, XTGETTCAP, title and size reports) and external ones (the clipboard read, pixel geometry, window state and position, the color scheme, ENQ). **The session worker arbitrates**, because it owns the routes (section 7.0):
      1. The worker parses the query, saves the **shadow answer at the parse point** (if `shadow_answerable`), and **never auto-answers**.
      2. **The asked route** is the session's most recently active route (under `Latest`, TS-6). If no route has sent input yet, it is the most recently attached route that can send input. A route attached with `answers_queries: false` is never asked. If such a route exists, the worker sends `terminal_query` on that route and waits at most the **host-supplied** `query_client_deadline_ms`. The Hub sets that value as policy (a documented default in the Hub configuration, with a range of 1 ms to the Hub's maximum), and **reports the value applied to each route in `route_limits.query_client_deadline_ms`**. **Core has no timer of its own:** the only deadline is the one the host supplies.
      3. The client answers **from its own real terminal state** (its theme colors, its real window and cell size, its clipboard, its emulator state), or it declines. A client `decline` is **not** a final answer: it triggers the fallback.
      4. **Fallback:** if there is no route to ask, or the client declines, or the deadline passes, the worker writes the saved shadow answer. With no shadow answer, it writes nothing, and the program's own timeout applies.
      5. **Exactly one reply:** the client answer and the fallback atomically consume the same `query_id` **at admission**. "The fallback started" means that the fallback was **admitted**. A fallback refused at admission (for example, the input lane is full) leaves the query pending. A client answer after an admitted fallback is refused with `already_replied`, and the reverse also holds.
    - **Renderer rules for clients (TS-QR):**
      - A client **disables every automatic query reply** of its terminal emulator for bytes that arrive in `output`.
      - It **never** sends reply bytes as `text` or `bytes`.
      - It answers only with `reply_terminal_query`, and only for a `terminal_query` it received **on that route**.
    - **Binding.** The **worker** binds each `query_id` to the route it chose: that route and the session run. A `reply_terminal_query` from any other route is refused with `input_refused { reason: invalid_input }`.
    - **Routing.** A query reply does **not** count as activity for `Latest` (TS-6). The worker never reroutes a query after a reply is admitted. If the completion of an admitted reply is unknown, **no other answer is sent**. If the chosen route ends **before** a reply is admitted, the worker goes straight to the shadow-terminal fallback (step 4).
    - **One actor per side effect.** The **worker** sends `output` bytes straight to the client (BP-1), so an attached client is the one actor for OSC 52 writes, and for notifications and the bell **on that client**. The Hub uses Core's notification events (Core EV-7) **only to inform the other clients** (the client contract's SF-5d: an event, the row's attention and Web push). It never writes to the terminal and never acts on OSC 52.
    - No reply is ever sent twice.
  - The only exception is the accepted unsafe-paste confirmation (TS-5e), which concerns input, not output.
- **The v1 shipped set** includes the first-party messaging and workspaces plugins (zero plugin regressions: the workspaces plugin places sessions for other plugins through HP PC-1 to PC-5).
- **Out of scope:** the UiNode vocabulary (owned by `ui-contract`, with its own semver); the Hub→plugin contract; the Core contract; Web signalling internals beyond section 9.

---

## 2. Connection and handshake

| # | Clause | Test |
|---|---|---|
| HS-0 | **Startup readiness** (CLI DEP-2). `botster-hub up --ready-fd <N>` writes **one line** to the inherited descriptor `N` once the Hub listens on its socket (TB-L1) **and** worker adoption has finished: `ready <socket path>`, or `failed <code> <message>` with a typed `code`. Then it closes the descriptor. **End of file with no line means a failed start.** A second instance is refused with a message naming the running Hub and its binary (`software`), and the Hub exits nonzero. No file is polled. | `conf::hs_0_ready_line_after_listen_and_adoption`, `conf::hs_0_failed_line_typed`, `conf::hs_0_eof_without_line_is_failure`, `conf::hs_0_second_instance_refused_naming_the_running_hub` |
| HS-1 | The first client **envelope** frame is `hello { protocol: "botster-hub", versions: { min, max }, client: { name, version }, features: [..], terminal_formats: [..] }`. `features` is what the client can use. `terminal_formats` lists the route formats the client can decode, in preference order (section 7). Any other first envelope frame closes the connection with `close { handshake_order }`. **A binding may define an authentication phase before the envelope** (TB-0 item 2; for example, the `webrtc` binding's PR-5 to PR-7 in section 9.3). Its frames are allowed only in that phase, and `hello` is accepted only after it completes. | `conf::hs_1_hello_first` |
| HS-2 | **Whose "current".** The Hub supports protocol versions `[hub_current − 1, hub_current]`. The client offers `[min, max]`. **The selected version is the highest version in both ranges.** An empty intersection gets `close { version_unsupported { hub_min, hub_max } }` and no welcome. Both sides then speak the selected version for the whole connection. | `conf::hs_2_selects_highest_common`, `conf::hs_2_empty_intersection_closes` |
| HS-3 | The Hub answers `welcome { hub_id, incarnation, version, features: [..], terminal_format, limits: { max_frame_bytes, max_chunk_bytes, max_outstanding, max_watches, route_connect_seconds, idempotency_window_seconds, cursor_idle_seconds, operation_retention_seconds, max_paste_bytes, max_file_bytes, query_client_deadline_ms } }`. `features` is the **intersection** of what the Hub offers and what the client listed. A feature the client did not list is never used on this connection. `terminal_format` is **advisory**: the first entry of the client's list that the Hub's current workers support. **The format that counts is chosen per route at attach** (TS-1). An adopted older worker may support a different one, so an empty or stale `welcome.terminal_format` never disables a route that its target worker can serve. | `conf::hs_3_features_intersected`, `conf::hs_3_terminal_format_selected` |
| HS-4 | **Features are selection, not a gate.** No feature ever causes a refusal at connect. A request that needs a feature that was not selected fails with `unsupported`. **When features differ between versions,** a feature is defined by its name alone; its meaning never changes. A changed meaning takes a new name. | `conf::hs_4_unselected_feature_is_unsupported` |
| HS-5 | **Incarnation.** `incarnation` is a random id that the Hub picks at each process start. `rev` values, list cursors, watch ids, idempotency keys and **non-durable** operation ids are scoped to one incarnation. **Durable operation ids** (EN-7) and **route ids** (A.3, BP-7) survive a restart. A client that reconnects and sees a new incarnation discards every cursor and watch it held, and re-watches. It keeps `{hub_id, session_id}`, other durable resource ids, and **its open direct routes**, which keep flowing (BP-7). | `conf::hs_5_new_incarnation_invalidates_cursors`, `conf::hs_5_routes_survive_new_incarnation` |
| HS-6 | There is **no conformance revision on the wire.** The suite's revision is a test artifact (section 12). | (by absence) |
| HS-7 | **Authentication is the binding's job** (section 9, TB-0). Each binding authenticates the peer to a caller: `local-stream` by socket permissions and the peer uid (TB-L3); `webrtc` by the paired client key bound to the DTLS session (section 9.3). **Authentication depends on the binding, not on the client kind.** New caller kinds (for example a read-only viewer) are added behind a feature, and the Hub never sends a caller kind that a plugin has not declared it accepts (HP). The caller of a client connection is `operator`, with a `credential_id` when the binding provides one. Agents use MCP (section 10), not this envelope. | `conf::hs_7_each_binding_authenticates_to_operator` |
| HS-8 | **Admission.** Over the connection limit, the Hub sends `close { busy { retry_after_ms } }` and closes. It never leaves a connection half open. | `conf::hs_8_busy_close` |
| HS-9 | **Typed close:** `close { reason }` with `reason` ∈ `handshake_order`, `version_unsupported`, `protocol_error { code }`, `busy`, `hub_shutdown`, `auth_failed`, `route_token_used`, `route_token_invalid`. It is sent before the Hub closes, whenever the Hub can still write. **`hub_shutdown` carries `lock_released: bool`**, which is true only when the Hub sends it **after** it released its single-instance lock and removed its listener socket (it keeps the already-open control connections until that frame). A close without `lock_released: true`, or a bare transport close, proves neither, so a client must not start a replacement Hub on it (CLI DEP-11). Disconnect cleanup: every watch and event stream owned by the connection ends with it. **Direct routes do not end** when the control connection closes. They end only by their own transport closing (TS-9), by an explicit detach or `delete(route)` (TS-10), by revocation (PR-8), or by a session or worker end. Their caller keeps ownership (A.3). | `conf::hs_9_close_reasons`, `conf::hs_9_disconnect_releases_watches`, `conf::hs_9_control_disconnect_keeps_route_open` (close the control connection with a route open: bytes keep flowing; then Hub restart, reconnect as the same caller, `list(route)` shows it, `delete(route)` ends it with `detached`) |

## 3. The envelope

| # | Clause | Test |
|---|---|---|
| EN-1 | Client frames are `hello`, `req { id, verb, kind, target?, args?, idempotency_key?, operation_id?, descriptor_id? }` and `cancel { id }`. `id` is a u64 **encoded as a decimal string** (safe in JavaScript). It is **unique for the whole life of the connection**: an id is never reused, so a late `cancel` can never hit a newer request. It need not increase. `target` is a resource id where the verb needs one. `descriptor_id` is required by DS-6. | `conf::en_1_request_shape`, `conf::en_1_duplicate_outstanding_id_closes` |
| EN-2 | Server frames are `welcome`, `res { id, ok: value } | res { id, err: Error }`, `watch { watch_id, … }` (section 5), `event { stream_id, … }` (section 8) and `close { reason }`. | `conf::en_2_server_frames` |
| EN-3 | **One response per request, while the connection is writable.** Responses may arrive in any order. At a Hub shutdown, the Hub answers each outstanding request with `err: hub_shutdown` before `close`, if it can still write. **`hub_shutdown` and a lost connection do not mean "no effect":** the effect of an outstanding mutation is **unknown** (see EN-8 for how to resolve it). | `conf::en_3_one_response_while_writable`, `conf::en_3_shutdown_is_effect_unknown` |
| EN-3b | **Cancel.** `cancel { id }` is best effort. The request still gets exactly one `res`: its real result, `err: cancelled` (sent **only** when the effect is certainly none), or another error whose `effect` says `unknown` or `partial` (for example, a plugin handler that committed host calls before a drain deadline). | `conf::en_3b_cancel_one_response`, `conf::en_3b_cancelled_means_no_effect` |
| EN-4 | At most `limits.max_outstanding` requests are open at once. Beyond that, the Hub answers `err: too_many_requests` (retryable) and does **not** close the connection. | `conf::en_4_outstanding_bound` |
| EN-5 | A frame over `limits.max_frame_bytes` in either direction is a protocol error. The Hub never sends one: a result that would be too large gets `err: response_too_large`. | `conf::en_5_frame_bound` |
| EN-6 | **Unknown input, handled safely:**
- An unknown **optional** field is ignored.
- An unknown **verb** or **kind** in a request gets `err: unsupported`; the connection stays open.
- An unknown **frame type** or other required discriminant is a protocol error, and it closes the connection with `protocol_error`.
- In responses, a client maps an unknown error code to its `category`, and it shows an unknown enum value as "other".
- An unknown view component is shown as an "unsupported component" placeholder. It is **never** executed. | `conf::en_6_unknown_optional_ignored`, `conf::en_6_unknown_verb_unsupported`, `conf::en_6_unknown_frame_closes` |
| EN-7 | **Effect classes.** Every verb and action declares `effect` in `describe`:
- `read`: no change;
- `immediate`: the change is done when `ok` arrives, but it is not durable (for example `attach`, `detach`, `resize`);
- `committed`: the change is done **and durable** when `ok` arrives. **Durable means:** the change's data, and any directory entry it needs, has been `fsync`ed to the Hub's data directory **before** the `res` is sent. So it survives a `kill -9` of the Hub or a power loss after the `res` (for example `create`, `update` and `delete` of a durable kind);
- `accepted`: the request is **admitted**, not necessarily started. `ok` returns `{ operation }`, and the outcome is the `operation` resource.

Whether an accepted action's operation is **durable** is a per-action descriptor field, `durable_operation: bool`. It is **not** inferred from the resource. A durable operation is **persisted (EN-7 durable) before its effect starts**, and it and its outcome survive a restart. Every other operation is scoped to the incarnation. A.7 lists the value for each fixed action.

**EN-7b. Caller-known operation ids.** An `accepted` request may carry `operation_id`, a string bounded only by `max_frame_bytes`.
- **Scope:** the id is unique **within the `hub_id`**, across all callers, because `get(operation, id)` has no caller namespace.
- **Duplicates:** a second request with the same `operation_id` joins the first only if it matches it exactly (EN-8 comparison). Otherwise it is `conflict { operation_id_in_use }`.
- **After a lost response and a crash,** the client resolves a durable operation with `get(operation, operation_id)`. `not_found` proves that the request was not admitted **only** within `limits.operation_retention_seconds` of the request (a retained, caller-known record), and only **after durable-operation recovery has completed**. While recovery runs after a restart, `get(operation, …)` answers `busy` (retryable), never `not_found`. **Retention never removes a pending operation.**
- `operation_id` takes part in the EN-8 comparison. | `conf::en_7_committed_survives_crash`, `conf::en_7_accepted_returns_operation`, `conf::en_7_durable_operation_survives_restart`, `conf::en_7b_caller_operation_id_resolves_after_kill9` |
| EN-8 | **Retry after a lost response (idempotency).**
- **Key size:** bounded only by `max_frame_bytes`.
- **Scope:** keys are per caller (`operator`, and its `credential_id` when the binding provides one) and per incarnation.
- **Match:** a repeat matches when the key, verb, kind, target, `descriptor_id`, `operation_id` and the **normalized** args (canonicalized with RFC 8785 (JCS)) are equal.
- **Window:** a **pending** key is kept until its request completes, however long that takes. A **completed** key is kept for `limits.idempotency_window_seconds` after completion. A request that arrives after its key expired is a **new** request. An unchanged key does not make it a safe retry.
- **Outcomes:**
  - a repeat that matches a **completed** key returns the first result;
  - a repeat that matches a **pending** key waits and returns the same single result;
  - the same key with a request that does not match is `conflict { reason: idempotency_key_reused }`.
- **Without a key,** a duplicate `create` of an existing id is `conflict`. There is no equal-value exception.
- **After a new incarnation,** keys are forgotten. A client **inspects the current state**. If no durable correlation proves which request produced it, **the outcome remains unknown**; there is no universal mutation ledger.
  - The one durable correlation is `operation_id` (EN-7b).
  - `immediate` effects (for example, whether an `attach` took effect before a crash) may always remain unknown.

| `conf::en_8_completed_key_returns_first`, `conf::en_8_pending_key_kept_until_completion`, `conf::en_8_pending_key_joins`, `conf::en_8_key_reuse_conflict`, `conf::en_8_create_without_key_conflicts` |

---

## 4. `describe`

| # | Clause | Test |
|---|---|---|
| DS-1 | `req { verb: describe }` returns `{ kinds: [KindDescriptor] }`. A `KindDescriptor` is `{ kind, owner: "hub" | <package>, descriptor_id, schema_version: "M.m", schema, core_fields: [..], verbs: { verb: { effect, permission, allowed } }, actions: { name: { args_schema, result_schema, effect, durable_operation, idempotent, permission, allowed, reasons: [..], owner?, descriptor_id?, warning?, label_pointer?, status_pointer?, url_pointer?, applies_to? } }, watchable: bool, durable: bool, projection_allowlist: [field] }`. An action contributed by a plugin to another owner's kind (HP VW-8) carries its own `owner` (the plugin key) and **`descriptor_id`**, which the client sends back on `invoke` (DS-6), and the HP VW-8 fields `warning`, `label_pointer`, `status_pointer`, `url_pointer` and `applies_to` when declared (their rules are in the `session` row, section 6). **`projection_allowlist`** is the set of fields that views and watches of this kind may read. It comes from the owner's single declaration (DS-2: the Hub's declaration for a Hub kind, the plugin's manifest for a plugin kind) and is **never derived from client structs** (ui-contract UI-AD-1 relies on it). Schemas use **JSON Schema draft 2020-12**. | `conf::ds_1_describe_shape` |
| DS-2 | **One declaration.** A kind's schema comes from the same declaration its owner validates with. For Hub kinds, that is the Hub's types. For plugin kinds, it is the schema the plugin declares, in any language. A schema is never written twice. **Schema validity is necessary, not sufficient:** a schema-valid request can still fail `permission_denied`, `wrong_state` or `conflict`. | `conf::ds_2_schema_invalid_is_refused_with_pointers`, `conf::ds_2_schema_valid_can_still_fail_state` |
| DS-3 | `describe` is also a watchable kind, `kind`. A plugin load or unload posts `upsert` or `remove` of its kinds, so screens appear and go away without a reconnect. | `conf::ds_3_kind_watch_on_plugin_load` |
| DS-5 | **Permissions in `describe`.** Each verb and action carries `permission` and `allowed: bool` for this connection's caller. The Hub still enforces every call. `allowed` is a hint, never the enforcement. | `conf::ds_5_allowed_matches_enforcement` |
| DS-6 | **Stale descriptors.** Each `KindDescriptor` has `descriptor_id`, which changes whenever the owning plugin reloads or the kind's schema changes. A mutating request or `invoke` on a plugin kind carries `descriptor_id`. **So does an `invoke` of a plugin-contributed action on a Hub kind** (for example `<package>.<action_id>` on `session`). Each contributed action descriptor has its own `descriptor_id`, which **changes when the contributing plugin reloads**. If the id no longer matches, the Hub answers `err: stale_descriptor` and dispatches **nothing**. A reloaded plugin never receives an action addressed to its previous version. **Disabled actions are listed with `allowed: false` and their `reasons`** (DS-5), never silently omitted, so every client sees the same action set. | `conf::ds_6_stale_descriptor_refused_not_dispatched`, `conf::ds_6_contributed_session_action_stale_after_plugin_reload`, `conf::ds_6_disabled_action_listed_with_reasons` |
| DS-7 | **Plugin kinds** have the same descriptor, verbs, errors and watch semantics as Hub kinds. A plugin kind's errors use the closed codes of section 11, with the plugin's own code in `reason`. | `conf::ds_7_plugin_kind_same_rules` |
| DS-4 | **Hand-built screens** bind to `kind@MAJOR`. If the Hub's MAJOR differs, the client uses its generic screen for that kind and logs a notice. A MINOR change never breaks a hand-built screen, because a client reads only `core_fields`. | `conf::ds_4_major_mismatch_falls_back` (client-side test in each client) |

---

## 5. Verbs and watch semantics

### 5.1 Verbs

| # | Verb | Semantics | Test |
|---|---|---|---|
| VB-1 | `list(kind, filter?, cursor?, limit?)` | One page of an id-ordered listing: `{ items, next_cursor? }`. `filter` is an equality map over core fields, plus the range filters that a kind declares in `describe` (for example `plugin_log.after`). **A listing is one retained cut:** the first call fixes the query and a cut, and every page of that cursor comes from the same cut, so rows are never skipped or duplicated. For a plugin kind, the cut comes from the Hub's projection of that family (HP EN-2). A plugin kind without a projection declares `list: current_read` in `describe`, and then pages are current reads, not one cut. A `cursor` is opaque, bound to its query, and scoped to the incarnation. A cursor unused for `limits.cursor_idle_seconds`, or used with a different query, is `expired_cursor` (retryable: restart the listing). | `conf::vb_1_pages_from_one_cut`, `conf::vb_1_expired_cursor` |
| VB-2 | `get(kind, target)` | One resource with its `rev`, or `not_found`. | `conf::vb_2_get_or_not_found` |
| VB-3 | `create(kind, value)` | Validates the value. **The result follows the declared `effect`** (EN-7): a `committed` create returns the created resource; an `accepted` create returns its receipt. A duplicate id is `conflict` (EN-8). | `conf::vb_3_create_follows_effect` |
| VB-4 | `update(kind, target, patch, if_rev?)` | `patch` is an RFC 7396 merge patch. **Identity fields (`id`, `hub_id`, `kind`) are immutable**: a patch that touches them is `invalid_arguments`. With `if_rev`, a `rev` that is not **equal** is `conflict`. Returns the new resource and its `rev`. | `conf::vb_4_merge_patch_rev_equality`, `conf::vb_4_identity_immutable` |
| VB-5 | `delete(kind, target)` | A state that forbids the delete is `wrong_state`. | `conf::vb_5_delete_wrong_state` |
| VB-6 | `invoke(kind, target?, action, args)` | Runs a declared action with its declared `effect` (EN-7). | `conf::vb_6_invoke_declared_only` |
| VB-7 | A verb that a kind does not declare is `unsupported`, never a protocol error. | | `conf::vb_7_undeclared_verb` |

`rev` is **opaque**. The only comparison is equality. There is no numeric ordering (P3).

### 5.2 Watch

| # | Clause | Test |
|---|---|---|
| WT-1 | `watch(kind, filter?)` answers `res { ok: { watch_id } }`. `res` comes before the watch's first frame. The first frames are **one baseline**: `snapshot_begin { watch_id, reason? }`, zero or more `snapshot_items { watch_id, items }` (id-ordered, each within `max_frame_bytes`), and `snapshot_end { watch_id }`. The baseline equals the source state at **one cut**. Every change after that cut follows as a delta. | `conf::wt_1_baseline_is_one_cut` |
| WT-2 | **Deltas** after `snapshot_end`: `upsert { id, value }`, `patch { id, patch }` and `remove { id }`, in source order. Frames of one watch are ordered by the transport, and there is **no sequence number**. `patch` is sent only for an id the client has from this baseline or a later upsert. **Filters:** a row that starts to match gets `upsert`, and a row that stops matching gets `remove`. | `conf::wt_2_filter_entry_exit`, `conf::wt_2_patch_only_known_rows` |
| WT-3 | **Batches.** One atomic source update (several rows changed together) is sent as `batch_begin`, one or more `batch_items { changes }` (each within `max_frame_bytes`), and `batch_end`. The client applies the whole batch at `batch_end`. **A partial batch or a partial baseline is never applied** to the client's copy: until `batch_end` or `snapshot_end`, the client keeps its previous state. After each applied batch or baseline, the client's copy equals **a valid source state**. | `conf::wt_3_batch_applied_at_end`, `conf::wt_3_partial_never_applied` |
| WT-4 | **Resync.** When the Hub cannot deliver deltas (the per-watch queue is full, or the source lost track), it sends a new baseline: `snapshot_begin { reason }` … `snapshot_end`. The client discards everything since its old baseline. No delta from before the new cut follows `snapshot_begin`. There is never a silent gap. | `conf::wt_4_resync_replaces`, `conf::wt_4_no_stale_delta_after_resync` |
| WT-5 | If the Hub cannot produce a baseline (the owner failed or was unloaded), it sends `watch_closed { watch_id, reason }`, and nothing more follows on that watch. | `conf::wt_5_closed_is_final` |
| WT-6 | `unwatch(watch_id)` answers `ok`. No frame of that watch follows the `res`. | `conf::wt_6_unwatch_then_silence` |
| WT-7 | **No replay.** A reconnect, or a new incarnation, opens a new watch, and its baseline is the sole baseline **[today]**. | `conf::wt_7_reconnect_baseline` |
| WT-8 | **Order:** one watch's frames are ordered. Different watches, event streams, responses and routes have **no promised order among themselves**, and there is **no atomicity across kinds** (a session row and its route may disagree for a while). | `conf::wt_8_cross_order_randomized` (FakeHub seeds) |
| WT-9 | **Writes and watches.** There is **no visibility barrier** and no promise that a watch shows any particular intermediate state: coalescing and resync may skip it. A `committed` `create` or `update` returns the resulting value. A watch **eventually** reflects the source's current state, which includes the write's effect or a later change. A client that needs "my change is visible" checks a **predicate on the current value** (for example `state == stopping`), never a `rev`. | `conf::wt_9_eventual_current_state` |

### 5.3 Bounds

| # | Clause | Test |
|---|---|---|
| BD-1 | Watches and event streams per connection are at most `limits.max_watches`. The Hub guarantees `max_watches` ≥ 32. Beyond the limit, the answer is `busy`. | `conf::bd_1_watch_cap`, `conf::bd_1_floor_32` |
| BD-2 | No collection is limited by the frame size: baselines are chunked (WT-1) and lists are paged (VB-1). Only a **single item** larger than `max_frame_bytes` is refused, with `response_too_large` or `watch_closed { item_too_large }`. | `conf::bd_2_large_collection_ok` |
| BD-3 | Every number is in `welcome.limits`. A client never assumes one: it uses only the values the Hub sent, and a changed value in a new `welcome` changes the client's behavior. | `conf::bd_3_client_uses_only_welcome_limits` (FakeHub sends unusual limits and the client obeys them) |

---

## 6. The kind registry (Hub-owned kinds in v1)

Core fields are listed. A hand-built screen may read only these. Everything else is described by `describe`. Plugin kinds are `<package>.<kind>` and come from plugins (the Hub→plugin contract).

| Kind | Core fields | Verbs and actions | Replaces today (non-normative) |
|---|---|---|---|
| `hub` (one row, id = `hub_id`) | `id`, `software{revision, channel}` (`channel` is `dev` or `release`; `revision` is the commit sha for `dev` and the version for `release`, shown as `dev <sha>` or `release <version>`), `worker_protocol` (the session-worker protocol this Hub speaks, Core AD-4), **`components: { hub, worker, plugin_hosts, mcp_bridge }`**: each is the **installed** identity `{ channel, commit, checkout?, running?: [{ channel?, commit?, count }] }` from the installed binaries' build metadata, which is what new launches use (`checkout` is an absolute path, present only for a `dev` build and shown to operators only). **`running`** lists the identities of live processes of that component that are **not known to equal** the installed one, with their `count`. **This includes unknown identities:** a live process whose build metadata is unknown is always listed (with `channel` and `commit` absent), even though the Hub cannot prove it differs. After a swap, adopted workers and preserved services keep their old build (Core AD-1, AD-4, SV-6), so the running set may be mixed. Each entry comes from the process's own build metadata; `channel` and `commit` are **absent when unknown**, never guessed. `software` is the running Hub's own identity (CLI DEP-13), `pid`, `uptime_seconds`, `data_root` (operators only), and the counts `sessions` and `plugins` (CLI DEP-12; the CLI's `status` prints them), `update{state, available, action}`, `push_public_key` (SF-5b), `web_origin?` (PR-0), `state` (`starting`, `recovering` while it adopts workers, `ready`; parity CG-10) | get, watch; invoke `check_update`, `update` (long-running; the whole install), `shutdown`, `restart`, `issue_pairing` (PR-0), `diagnostics`, `swap_preflight`, `swap`, `preview_package_upgrades`, `upgrade_packages`, `rollback_packages` (HP PM-7, PM-9 for all packages) | `Status`, `Whoami`, `CheckHubUpdate`, `StartHubUpdate`, `GetHubUpdateExecution`, `DaemonShutdown` |
| `session` | `id`, `hub_id`, `label`, `state` (`starting`, `running`, `stopping`, `ended`, `lost`), `lifecycle_class` (`current`, `ended`, `indeterminate`), `exit{code, signal}`, `failure_reason`, `size{rows, cols}`, `session_type{id, source}`, `role`, `traits`, `interaction`, `restartable`, `port?` (the session's allocated port when its type asks for one, HP CA-20; absent otherwise), `owner?` (the key of the plugin that spawned it; absent for an operator or agent spawn; parity G-14), `worktree_id?` and `branch?` (the worktree it runs in, if any; G-21), `title?` (**only** the terminal's latest title, from the Core model; nothing else writes it; G-21), `task?` (a short text that a client, an agent or a plugin sets with `update`, HP SA-1; legacy `task`, parity CG-4), `activity` (`active` or `idle`: idle after the Hub parameter `activity_idle_ms`, default 3000, with no output; parity CG-4), `attention?: { since, source, title?, body? }` (SF-5d), `presentation` (`default`, `plugin` or `hidden`, set at spawn through HP; G-18: clients leave `plugin` and `hidden` sessions out of the default session list, and show a `plugin` session in its owner's views. **Presentation never limits authority or the API:** `list` still returns every session, and a filter selects them), **the Hub's `session` descriptor lists every field of this row in `projection_allowlist`, including `owner`** (ui-contract UI-BD-6), `metadata` (a map of plugin namespace to a JSON object, written by plugins through HP `sessions.update` (SA-1; another plugin's session needs `session_update:any`), read-only to clients) | list, get, watch, **update** (`{ label?, task? }` as a merge patch; permission `session.update`; #54), delete (`ended` or `lost` only, per A.2c; else `wrong_state`); create = spawn (below); invoke `attach` and `detach` (section 7), `stop`, `restart`, `read_screen`, `clear_attention` (SF-5d), **`safe_write`** and **`write`** (the non-route writes, so that every caller, including an agent on `mcp`, can type into a session: HP SW-1 to SW-10 and RW-1 to RW-3 govern them, and the permissions are `session.safe_write` and `session.write`), and **plugin-contributed actions** `<package>.<action_id>`. Those are listed in `describe` for the `session` kind with their args schema, effect and permission (declared through HP VW-8 `session_actions_ui`; a contributed action never widens authority). **A contributed action's per-session state** (parity G-20): its descriptor carries the fields that HP VW-8 declares: `label_pointer?`, `status_pointer?` and `url_pointer?` (JSON pointers confined to that session's `metadata[<package>]`; a `status_pointer` or `url_pointer` that resolves to nothing or to a non-string shows nothing, with no error and no default; an absent, non-string or empty `label_pointer` value gives no override, and the action keeps its required static manifest label (UI-SA-1)), and `applies_to?` (the closed equality filter of HP VW-8 over `session_type_id`, `state`, `interaction`, `role` and the boolean `has_port`). The label override follows ui-contract **UI-SA-1**: only a non-empty string overrides the static declared label; otherwise the static label stays (HP VW-8), the status text beside the action, and the URL as an external link (http or https only; the client shows the host before it opens the link outside the app). A declared `warning` (a static string, HP VW-8; for example, that the action publishes a URL to the internet) is always shown beside the action. An action whose `applies_to` does not match the session is not offered, and dispatched to the plugin's handler with the target `SessionRef` and the verified caller | `ListSessions`, `Spawn`, `SpawnSessionType`, `RemoveSession`, `ShutdownSession`, `Attach`, `ReadScreen`, `ReadModeFlags`, `NotifySession`, `ReadSessionContext`, restart |
| `session_type` | `id`, `source`, `label`, `description`, `editable`, `targets`, `port_env?: [name]` (MC-1c), `notifications?` (SF-5c) | list, get, watch, create, update, delete; invoke `resolve`, `definition` | the 9 session-type requests |
| `spawn_target` | `id`, `label`, `path`, `kind`, `git?: { repo, current_branch?, default_branch? }` (Hub-derived; `repo` is `owner/name` from the remote, never a path or a credential; parity G-24) | list, get, watch, create, update, delete; invoke `validate` | 6 requests |
| `worktree` | `id`, `target_id`, `path`, `branch`, `state`, `active_sessions: [SessionRef]` (current sessions running in it, so a client can warn before delete; parity CG-5) | list, get, watch, create, delete | 4 requests plus the `WorktreeLifecycle` event |
| `package` | `id` (**the package key**: `<name>` for a `device` package, `<name>@<target_id>` for each instance of a target-scoped package, HP CP-8), `name`, `kind` (`plugin`, `agent_definition` or `accessory`, HP PM-11), `state` (`enabled`, `disabled`, `quarantined`, `failed`), `status` (the install lifecycle, HP PM-10: `installed`, `outdated`, `errored`, `live`, `pinned`, `staged` or `degraded { interface }` (a required provider is unavailable or a breaking update was accepted, HP PD-6, PD-8)), `provides?` (`[{ interface: "name@version" }]`, HP PD-3) and `requires?` (`[{ interface, range, optional?, binding?: { interface_version, provider, provider_version, state: available | unavailable | unmet } }]`, HP PD-5, PD-8), `version`, `source` (`{ kind: local, path }`, `{ kind: git, url, ref? }`, or the catalog entry it came from; HP PM-1; a `local` path is shown to operators only), `lock?: { resolved_commit, content_hash, locked_at }` (HP PM-3), `pinned_ref?`, `pending_grants?` (grants that wait for approval, HP PM-8), `configuration`, `entrypoints[]`, `notice_reactions[]`, `target_id?` (a target-scoped package, HP manifest `scope: "target"`; parity G-5). **User decision U-6 (2026-09-30):** the Hub discovers target-scoped packages in each enabled spawn target and runs one instance per target, keyed `<name>@<target_id>`. Its events and session events are limited to that target: the legacy Hub-wide firing is fixed. **Every place this contract writes `<package>` or a plugin key means the package key**, so two instances never share one: the package row `id`, `ResourceRef` ids, plugin kinds `<package>.<kind>`, view and navigation ids, contributed actions `<package>.<action_id>`, `metadata[<package>]`, the event `owner`, the session `owner`, the notice `package`, and the MCP tool prefix (MC-2) | list, get, watch; **create = install** (HP PM-5: `create(package, { source })`); update (configuration); **delete = remove** (`{ purge? }`, HP PM-10: the store is kept unless `purge`); invoke `enable`, `disable`, `reload`, `start_entrypoint`, `stop_entrypoint`, `restart_entrypoint`, **`preview_upgrade`** (read) and **`upgrade`** (HP PM-7 `update {preview}` and `update`, split so each action has one effect; named `upgrade` because `update` is the envelope verb), `pin`, `unpin`, `rollback`, `approve_grants`, `open_asset`. **Plugin management is HP PM-1 to PM-14**; the permission is `package.manage` (scoped, CP-9; a plugin caller never holds it) | 16 package requests |
| `catalog` | `id`, `url`, `refreshed_at?`, `entries: [{ id, kind: plugin | agent_definition | accessory, name, description, source, status }]` (`status` is `available` or the installed package's status; HP PM-2) | list, get, watch, create (`{ url }`: the operator approves the URL, HP PM-2), delete; invoke `refresh` (HP PM-10 `refresh_catalog`). Permission `catalog.manage`. Catalog content is untrusted data and never runs | replaces the registry family and the hosted template catalog |
| `quarantine` | `id`, `package`, `reason`, `since` | list, get, watch; invoke `resolve` | `ResolveQuarantine`, the Status list |
| `navigation` | `id`, `package`, `label`, `icon?`, `order`, `target { kind: view | entrypoint, id, select?: { <slot>: row key } }` (a deep link that opens a view with a preset selection; the selection stays client-local after open; parity G-13), `badge?: { source: { kind, id, field }, display: dot | count, tone }` (bound to a field of a watched row, with no plugin code; parity G-15) | list, watch; invoke `resolve_route` | `ListPackageNavigation`, `ResolvePackageRoute`, `ListApps`, `ResolveAppLaunch` |
| `view` | `id` (`<package>/<view>`), `component` (`list`, `table`, `detail`, `form`, `actions`, `status_badge`, and `embed` under the feature `ui_embed`, ui-contract 1.2), `binds { kind, filter?, fields }`, `actions[]`, `placement?: { area: "home", size: small | medium | wide | full, order }` (a home-page widget; parity G-15) | list, get, watch (a view set changes when a plugin loads or unloads) | `PluginSurfaceRender`, `PluginSurfaceAction`. Views are **declarations**: plugin code does not render or re-render them (plugin-platform §12). Actions are `invoke` on the bound kinds. |
| `plugin_log` | `id` (opaque, ordered within a package), `package`, `load_id` (opaque; it changes at each plugin load, so a client can show "earlier load"), `at`, `level`, `message`, `fields`; plus **marker rows** `{ gap: evicted | rate_limited, count? }` and `{ restarted: load_id }` | list (paged, filter `package`; range filter `after`), watch (filter `package`: the baseline is the retained tail, then appended records and gap rows) | `ReadPluginLogs`. `evicted` means the retention dropped older records. `rate_limited` means the source dropped records before storing them. |
| `operation` | `id`, `kind`, `state`, `progress?`, `result?`, `error?`. **`state` is a per-`kind` enum, equal to the Hub→plugin operation states (HP CA-16 and SW-2)**: `spawn` (a restart is a `spawn` operation, HP CA-14) uses `starting`, `installed`, `failed { reason }`, `cancelled`; `safe_write` uses `queued`, `typing`, `delivered`, `abandoned { reason }`, `replaced`, `cancelled { reason }`, `failed { reason }`, `unknown { reason }`; other kinds use `running`, `succeeded`, `failed`. Every operation also carries `terminal: bool` (whether the state is final). Both `kind` and `state` are non-exhaustive: an unknown value is shown as "other", and `terminal` tells the client whether to keep watching (EN-6). | get, watch |
| `client_credential` | `id` (`credential_id`), `created_at`, `last_seen_at` | list, get, watch, delete (revoke, PR-8) |
| `push_subscription` | `id`, `credential_id`, `created_at` (the endpoint is never returned; SF-5b) | list and delete (own credential only), create | new (parity G-11) |
| `route` | `id`, `session: SessionRef`, `binding` (an open string, TB-0), `since` | list, get, watch, delete (= detach), each limited to the owning caller (A.3) | the route inventory that survives a Hub restart (BP-7) | `GetHubUpdateExecution`, install and update previews |
| `kind` | the `KindDescriptor` of 4 | list, watch | Hello features for plugin families |

| # | Clause | Test |
|---|---|---|
| KR-1 | **The rules stated in the kind rows above are normative, and each has a named test** (gate 3). | `conf::kr_1_session_lifecycle_class_mapping` (A.2c), `conf::kr_1_session_update_label_and_task`, `conf::kr_1_session_title_is_terminal_only`, `conf::kr_1_session_owner_worktree_branch_fields`, `conf::kr_1_session_presentation_hides_from_default_list_not_from_list`, `conf::kr_1_session_activity_idle_after_parameter`, `conf::kr_1_session_create_in_existing_worktree`, `conf::kr_1_worktree_active_sessions`, `conf::kr_1_hub_state_starting_recovering_ready`, `conf::kr_1_plugin_log_load_id_and_markers`, `conf::kr_1_session_type_script_runs_as_command`, `conf::kr_1_spawn_target_browse_directories_only_operator` |

**Session ids are Hub-minted and never reused** (agreed with the Hub→plugin pair, SS-5). A session id is 128 random bits, minted by the Hub, so a deleted and recreated session always has a new id, while a restart keeps the same id. Callers cannot choose a session id: retry safety comes from `idempotency_key` and `operation_id` (EN-8, EN-7b).

**Session create (spawn)** is **accepted, with a durable operation**, the same shape as a plugin spawn (agreed with the Hub→plugin pair): `create(session, { session_type_id, target_id?, worktree_id?, prompt?, context? })` (A.9) returns `{ operation_id, session: SessionRef, state: "starting" }`. The admitted session row and the operation are persisted **before** Core starts the process. The session's outcome (`running`, or `ended` with a `failure_reason`) is on the session watch and on the operation. **Freeform spawn is kept** as a typed variant of the same verb: `create(session, { command: { argv, cwd?, env? }, size?, target_id? })`, beside `create(session, { session_type_id, … })`. It needs permission `session.create_command`: operators have it by default, and plugins get it only as HP defines (if HP offers no freeform spawn call, plugins have none in v1). The existing operator capability is preserved; only the bespoke request goes.

**Session `restart`.** It keeps the same `id`. It is valid only when `restartable` is true and the state is `ended`. Refusals use `wrong_state` with `reason` ∈ `not_ended`, `not_restartable`, `record_unavailable`, `environment_not_retained`, and `not_ready` (retryable).

**Removed from the client wire:**
- `PostMessage`, `ReceiveMessages`, `AckMessage`: **messaging is the messaging plugin's** (user ruling, 2026-09-30). There is no Hub-native message kind. If the plugin wants an inbox in the TUI or Web, it ships declarative views over its own kinds (for example `messaging.message`), as any plugin can (7b).
- `PluginMcpListTools`, `PluginMcpCallTool`: these belong on MCP.
- `CaptureSnapshot`, `ReadSnapshotPage`: snapshots travel on the route (section 7).
- `live_attach_occupancy`, `lifecycle_counters`, `retention` and every counter in Status: gone (P3). A test that needs an oracle uses the Hub's test API, not the wire.
- The conformance revision and its minimum.

---

## 7. The terminal stream

### 7.0 The byte path: client ↔ session worker (user requirement, 2026-09-30; design APPROVED by the user)

The user's words: "passes through bytes unadulterated and ... the bytes go straight from client to PTY when it's connected. No extra hops, minimum latency maximum performance possible."

| # | Clause | Test |
|---|---|---|
| BP-1 | **The route ends at the session worker, not at the Hub, on every binding.** The Hub authorizes an attach (caller, permission, session), hands the route's endpoint to the session's worker in the binding's way (section 9: TB-L2, TB-W2), and gets out of the byte path. **No Hub process touches route bytes.** | `conf::bp_1_hub_stopped_bytes_still_flow` (`SIGSTOP` the Hub process: typing still echoes, and output still arrives on an attached route, on **every** binding) |
| BP-2 | **Unadulterated bytes.** `output` frames carry the PTY's bytes **exactly**, in order. Client input frames reach the PTY as the bytes they specify: raw `bytes` and `text` verbatim, and semantic keys and paste encoded only as P9 and Core 5.1A define. A baseline snapshot is sent **only** at attach and at a resync (TS-4, TS-4c), never as a substitute for output. | `conf::bp_2_output_bytes_identical_to_pty` (a random byte stream written to the PTY equals the concatenated `output` payloads) |
| BP-3 | **Framing** on the route is the thin frame of Appendix A.4, `[u8 type][body]`, delimited by the binding (TS-3). The data frames (`output`, and input `bytes` `0x83`, `text` `0x84` and `paste_chunk` `0x82`) carry raw bytes with no JSON. Control and lifecycle frames (`resize`, `modes`, `input_refused`, `input_done`, `terminal_query`, `route_closed`, and the baseline frames) travel **on the same ordered stream** beside the data. They are small and infrequent. | `conf::bp_3_data_frames_are_raw` |
| BP-5 | *(Non-normative.)* Route frames are produced and consumed by the session's worker. Control frames, authorization, attach, detach and revocation are the Hub's. The Core contract states the split (Core DP-4, EV-8). A client sees only the frames and results defined here. | `conf::bp_5_detach_via_hub_closes_route_at_worker`, `conf::bp_5_revocation_closes_worker_routes` |
| BP-6 | **Performance and its tested topology.** There is no minimum-latency claim without a named topology.
- **One topology per binding** (section 9): the client ↔ the binding's route connection ↔ worker ↔ PTY. For v1 these are `local-stream` on one host, and `webrtc` over loopback.

The benchmark measures the key-to-echo round trip (p50 and p99) and the sustained output throughput. It compares each against a **reference relay**: the same client talking over the same transport to a minimal program that relays directly to a PTY. **The gate is relative:** Botster's added overhead over the reference, as a factor. The factor values are chosen with the orchestrator and set on the first measurement (no invented numbers). | `bench::bp_6_echo_rtt_per_binding`, `bench::bp_6_output_throughput_per_binding` |
| BP-7 | **Worker restart and adoption.** Routes do not survive a worker exit. On a Hub restart (the worker keeps running), **existing direct routes stay connected**, because the Hub is not on their path. The new Hub adopts the worker and its routes' metadata (Core adoption). | `conf::bp_7_routes_survive_hub_restart` |


**Layering (resolved with the Core pair).** The Hub defines the route **lifecycle** and the **input and result envelopes**. Core defines the **payload encodings** of the baseline (GHOSTSNP, Core ST-6), of output and of resync. The **worker** carries those payloads **verbatim** inside the route frames (BP-1). `terminal_format` (HS-3) names the Core payload encoding, for example `ghostsnp/2+raw`. A client never sees a Core version.

| # | Clause | Test |
|---|---|---|
| TS-1 | `invoke(session, id, attach, { history: none | screen | full, replace_route?, answers_queries?, binding?, route_features? })` **reserves a route** and answers `{ route_id, transport, route_limits: { max_frame_bytes, max_screen_frame_bytes, max_history_page_bytes, max_chunk_bytes, max_paste_bytes, max_file_bytes, max_query_reply_bytes, query_client_deadline_ms, stall_deadline_ms, stall_close_after_ms } }`, taken from the worker options selected at attach. `binding` defaults to the binding of the requesting connection. **Before the route is reserved, the Hub refuses the attach with `unsupported { reason }`** in two cases:
- the caller cannot authenticate on the chosen binding (for example, `webrtc` needs a paired credential and the caller has none): `binding_auth_unavailable`. A binding always authenticates the route connection itself (TB-0 item 2), and **no client kind bypasses that**;
- no terminal format from the caller's `hello.terminal_formats` is supported by the **target session's worker**: `terminal_format`.

**Negotiation is per route.** The attach picks the first format in the caller's `hello.terminal_formats` that the target worker supports. `attached.terminal_format` echoes that choice, and `attached.features` is the **intersection** of the attach's `route_features` and the worker's features. **`transport` is `{ binding: <name>, … }`**, where `<name>` is an open, feature-detected string (TB-0) and the other fields are that binding's connect information (section 9). **The three frame bounds** (`max_frame_bytes`, `max_screen_frame_bytes`, `max_history_page_bytes`) bound a frame's size, which counts the type byte plus the body (TS-3). So `max_screen_frame_bytes` = Core's `max_snapshot_bytes` + 1, and `max_history_page_bytes` = the history page payload limit + 1. **`max_chunk_bytes` is not a frame bound:** it bounds one transport chunk, for a binding that chunks. Each route frame fits its own type's limit. **Every limit a route peer can hit is reported per route in `route_limits`**, and it is authoritative over any `welcome.limits` value of the same name (those are Hub-wide hints). The Hub mints `route_id`, unique within the `hub_id`. **A session may have many routes at once** (several panes or clients). `replace_route: <route_id>` ends that route with `replaced` in the same step. Without it, no other route is touched. | `conf::ts_1_multi_attach`, `conf::ts_1_explicit_replace` |
| TS-2 | **Connect.** The client completes its binding's connect step (section 9: TB-L2, TB-W2) within `limits.route_connect_seconds`. A route that is not connected in time ends with `not_connected` (TS-9). | `conf::ts_2_connect_once_in_time` |
| TS-3 | **Framing:** the route carries frames `[u8 type][body]` in both directions. **The binding delimits them** (section 9). A frame's **size** counts the type byte plus the body, and each frame type has its own bound (A.1 `frame_too_large`). The type values and bodies are normative in Appendix A.4. There is no route id, generation or epoch in any frame: the binding's connection **is** the route. | `conf::ts_3_framing` |
| TS-4 | **Baseline.** After connect: `attached`, `baseline_begin { rows, cols, modes }`, one `screen { payload }` frame (at most `route_limits.max_screen_frame_bytes`), zero or more `history { payload }`, and `baseline_end { history: not_requested | complete | unavailable { reason } | incomplete { reason } }`. The `complete` history may be empty. **The baseline is a state cut:** a terminal model that loads the baseline and then applies every later `output` reaches the same **terminal state** as the session's model at the same point (the worker's model): **exactly the state that Core ST-6b lists**: the screen and scrollback cells with their attributes, OSC 8 hyperlinks included; the palette and the dynamic colors; the cursor position, visibility, shape and blink; every `ModeFlags` field; the size and the alternate-screen state; the title and the cwd. Images are restored only when the worker advertises `snapshot_graphics`. Core states every exclusion, and nothing is lost silently (parity CG-6). **The cut is parser-safe, per Core ST-6b's resume invariant:** a model that loads the baseline and then applies every later `output` reaches the session's exact state at every cut point, including a cut inside an escape sequence (for example `ESC [` then `31mX`). Core either carries the pending parser state in the snapshot, or takes the cut at the preceding ground state and sends the complete pending sequence bytes as `output` after the baseline. Either way the prefix is restored, and a client never receives an unmatched sequence suffix. History is compared **only** when it is `complete`. If the screen cannot be produced, attach fails: the route ends with `attach_failed { reason }`. Missing or incomplete history leaves the route live. | `conf::ts_4_state_equivalence` (the model after baseline + output equals the worker's model), `conf::ts_4_history_states` |
| TS-4b | **`live`** follows `baseline_end`. **`live` is the input-ready point** (TS-5). | `conf::ts_4b_live_after_baseline`, `conf::ts_4b_early_input_not_ready` |
| TS-4c | **Resync within a route.** When the worker discards queued output for this route (a reader without progress, TS-7), the worker **first finishes every frame whose bytes or chunks have started**, discards only frames not yet started, and sends `resync` at the next frame boundary, followed by a new `baseline_begin` … `baseline_end`, `live`. The client replaces its screen. The route and its `route_id` stay the same. **If a started frame cannot be finished, the route ends** (TS-9 `write_failed`) and the client re-attaches. **Transport loss is not a resync:** it ends the route (TS-9). | `conf::ts_4c_resync_same_route` |
| TS-4d | **Modes and the effective size** travel on the route: `modes { modes, rows, cols }` in the baseline and at every change. `rows` and `cols` are the size actually applied to the PTY. | `conf::ts_4d_modes_effective_size` |
| TS-5 | **Input is fire-and-forget** (user rule, 2026-09-30: a terminal over ssh has no per-keystroke acknowledgement). The route's ordered, reliable delivery **is** the delivery guarantee, and **there is no success acknowledgement** for `bytes`, `text`, keys, mouse, focus or resize. The two exceptions are a paste that requests completion (TS-5b) and a file (TS-5f). `input { op?, kind: key | text | mouse | focus | resize | paste_begin | paste_chunk | paste_commit | paste_abort | file_begin | file_chunk | file_commit | file_abort, … }`. `op` is optional and chosen by the client, and it is used only to correlate a refusal (TS-5b). A paste's frames, and a file's frames, carry one `op`, which is required for them. **Order is the worker's receive order.** Input received before `live` is refused `not_ready`. Backpressure is the transport's: the worker stops reading the route while its input queue is full, so input is never dropped for lack of room. What a client does with unsent input, or with input handed to a transport that then failed, is client policy (TS-5c). | `conf::ts_5_no_success_ack`, `conf::ts_5_order_is_receive_order`, `conf::ts_5_backpressure_not_drop` |
| TS-5b | **The worker reports only exceptions:** `input_refused { op?, reason }`, sent when an input is **not** applied as sent. `op` echoes the input's `op` when it had one. `reason` is non-exhaustive (an unknown reason shows as "other"):
- `not_ready` (before `live`);
- `not_writable` (a view-only route);
- `too_large`;
- `mode_precondition_failed` (the paste asked for bracketed mode and it was off; the client, which classified the paste, shows its own unsafe-paste reason);
- `unsupported { what }` (an unknown kind or value, or invalid UTF-8 as `what: "utf8"`);
- `refused`;
- `session_ended`;
- `failed` (the write to the PTY failed);
- `write_failed` (a file could not be written on the host, TS-5f).

A refusal means the input had no effect, except `failed` and `session_ended`: those can follow a partial write of **any** input (bytes, text, a paste, or a multi-byte encoded key). They carry `written_bytes`, which is exact when the worker knows it and absent when it does not. **`written_bytes` has one unit everywhere:** the bytes written to the PTY after encoding, including any bracketed-paste markers (Core IN-2). For a file, it counts only the path paste, never the uploaded content. Input that the program's modes do not report (for example, mouse events with tracking off) is not an exception: as on a real terminal, it is simply not sent to the program, with no frame. **A paste may request completion:** `paste_commit { op, want_result: true }` is answered by `input_done { op, written_bytes }` or by `input_refused`. This supports the unsafe-paste confirmation flow and paste progress. Apart from this, only a file is acknowledged (TS-5f). | `conf::ts_5b_refusal_only_on_exception`, `conf::ts_5b_partial_write_reports_written_bytes`, `conf::ts_5b_paste_done_on_request`, `conf::ts_5b_unreported_mode_input_is_silent` |
| TS-5c | **What a client can always know about each input** (user ruling: what to do next is client policy):
- **Never sent on a route:** no effect.
- **Sent, and the route is still live:** the worker processes it in receive order. It is written, or refused with an `input_refused`, or (for input that the modes do not report, A.5) omitted with no frame, as on a real terminal. There is no success receipt: silence proves neither that the write has completed (the input may still be queued behind PTY backpressure) nor that no refusal is in transit.
- **Sent, and the route failed** (TS-9): an `input_done` or `input_refused` for it that already arrived still decides its outcome. Without such a frame, the outcome is unknown, as on ssh.

Each client chooses to drop, keep or resend. *(Non-normative: resending may duplicate input.)* A user-confirmed paste resend is a **new** `op` with `require_bracketed: false`. **There is no size result:** the effective size arrives in `modes { modes, rows, cols }` (TS-4d). | `conf::ts_5c_unsent_has_no_effect`, `conf::ts_5c_confirmed_paste_resend_is_new_op` |
| TS-5d | **Paste** is one operation: `paste_begin`, chunks, then `paste_commit` or `paste_abort`. An abort before the commit writes nothing, with no frame. An abort after the commit has no effect. A paste over `route_limits.max_paste_bytes` is refused with `input_refused { op, reason: too_large }` at the first chunk that exceeds it, with zero effect. A paste not committed when the route ends writes nothing. **Only Core's encoder** adds bracketed-paste markers, at write time, according to the mode at that time. Neither the client nor the Hub adds them. The paste's chunks reach the PTY contiguously: no other input is written between them. | `conf::ts_5d_abort_before_commit`, `conf::ts_5d_contiguous` |
| TS-5f | **File input** (user gap, 2026-09-30: a user pastes or drops a file or an image, and the program gets its path). It is transport-neutral, so every client can send it. One operation:
- `file_begin { op, name, mime?, total_bytes? }`, then `file_chunk` frames (`0x85`), then `file_commit` or `file_abort`. `op` is required and nonzero. One file is open per route at a time; a second `file_begin` before the first ends is refused with `refused`.
- **Bound:** a file over `route_limits.max_file_bytes` is refused with `too_large` at the first chunk that exceeds it, and nothing of it stays on the host.
- **Chunks are not terminal input:** other input keeps flowing while a file uploads.
- **Before it creates the final file**, the worker forms the absolute path and checks that the encoded path paste (markers included) fits `route_limits.max_paste_bytes`. If it does not, the commit is refused with `too_large` and no file is created.
- **At `file_commit`**, the worker writes the file on the session's host, where the session can read it. Then it writes the file's **path followed by one space** as one paste (TS-5d) **at the commit's position in the receive order** (the space lets the user type straight after the path, and agent CLIs detect the path as an attachment; legacy parity CG-1), so the path lands in order with the user's typing. Only Core's encoder adds bracketed-paste markers, when the mode is on.
- **The worker chooses the path.** The directory comes from Hub policy. The file name is unique, derived from `name` reduced to `A-Z a-z 0-9 . _ -` (an empty result becomes `file`), never overwrites a file and never leaves the directory. The worker pastes the absolute path's bytes **unmodified**: neither Core nor the worker quotes or escapes it, because neither knows the program's syntax. *(Non-normative: a Hub directory policy without spaces or shell metacharacters keeps the path usable at a shell prompt.)* `mime` is advisory.
- **The exception to fire-and-forget:** every `file_commit` is answered, with `input_done { op, written_bytes, path }` or with `input_refused { op, reason }`. The reasons include `too_large`, `write_failed`, `not_writable`, `not_ready`, `failed` and `session_ended`.
  - A refusal **before the file is complete** (`too_large`, `write_failed`, `not_writable`, `not_ready`, or `session_ended` during the upload) leaves no file and writes no path.
  - A refusal **while the path is being pasted** (`failed` or `session_ended`) carries `path` (the file stays, under the Hub's cleanup policy) and `written_bytes` of the path paste (exact when known, absent when unknown, TS-5b).
- **An abort, or a route end, before the commit** writes nothing and removes any partial data, with no frame.
- **Hub policy, not wire:** the directory, the naming, the permissions (readable only by the session's user by default) and the cleanup. Cleanup happens at session delete at the latest.
| `conf::ts_5f_path_written_in_receive_order`, `conf::ts_5f_commit_answered_with_path`, `conf::ts_5f_over_limit_leaves_nothing`, `conf::ts_5f_abort_or_route_end_leaves_nothing`, `conf::ts_5f_name_sanitized_and_unique`, `conf::ts_5f_other_input_flows_during_upload` |
| TS-5e | **Unsafe paste is Hub policy** (the definition, accepted by the user on 2026-09-30: bracketed paste is off **and** the payload contains a line break or ESC). **The seam (settled with the Core pair):**
- The **client** classifies the payload by this rule (the route goes straight to the worker, BP-1).
- When the rule needs bracketed mode, the client sends `paste_begin { require_bracketed: true }`, and the worker applies it as Core `Paste { require_bracketed: true }`.
- The worker checks the mode **atomically at the start of the write**. `ModePreconditionFailed` proves zero effect, and the worker reports it on the route as `input_refused { op, reason: mode_precondition_failed }`. The client derives its UI reason from the payload it classified.
- A confirmed resend is a new op with `require_bracketed: false`.
- Core adds the markers and writes contiguously. Core never classifies content.
- **Why the client classifies** (gate 3 #86): clients are authenticated operators, and a confirmed resend is a client choice anyway. So worker-side enforcement would add no protection against a nonconforming client. Core's `paste_require_bracketed_if` option is to be deleted (gate 2 #1). | `conf::ts_5e_precondition_zero_effect`, `conf::ts_5e_confirmed_resend_without_precondition_writes` |
| TS-6 | **Size policy `Latest`.** The Hub **selects** the policy (orchestrator ruling), and the **worker computes and applies** it, because it owns the routes (BP-5):
- **Activity:** a route's last activity is the worker's receive time of its most recent input of any kind (key, text, raw bytes, mouse, focus, paste, resize; matching Core DP-6).
- **Selection:** the effective size is the last reported size of the route with the most recent activity. A route that never reported a size is skipped.
- **Ties:** the worker's receive order is total, so the route whose input the worker received last wins.
- **Close:** when the active route closes, the next most recently active remaining route's last size applies. With no remaining route that has a size, the size stays as it is.
- **Resize input:** a `resize` counts as activity, so the resizing route becomes active. It gets no answer. The applied size (which may come from a more active route) arrives in `modes` (TS-4d).
- The worker resizes the PTY. There is no Hub hop for a resize. | `conf::ts_6_most_recent_activity_wins`, `conf::ts_6_close_falls_back`, `conf::ts_6_tie_by_receive_order` |
| TS-7 | **Backpressure** (matches Core OU-3). A reader that makes progress is lossless: output waits, and the session's output source waits too. A reader with no progress for `route_limits.stall_deadline_ms` (Hub policy, applied by the worker) may have its queued **output** discarded and gets a resync (TS-4c). **Only queued output may be discarded:** pending `input_refused` and `input_done` frames and `route_closed` are retained and still delivered on the continuing route. **Exception frames are bounded without loss:** they wait in the route's existing outgoing queue and count against its existing byte bound (Core `route_queue_bytes`), but a resync never discards them. When that queue cannot hold one more exception frame, the worker stops reading the route's input until it can, as it does for a full input queue. No separate count or ledger exists. So a client that sends input and never reads refusals stalls its own route, and the stall rules above then close it; the worker never drops an exception frame and never grows without bound. **A resync ends the answer chance only for an affected query**: a query whose required output prefix was discarded, which means its `terminal_query` frame had not started. The client would otherwise answer from stale state. **A query frame already sent (or started, and then finished under TS-4c) is not affected**, because ordered delivery has already carried its prefix. For an affected query:
  - The worker retires its unstarted `terminal_query` frame.
  - It admits the saved P9 fallback (the parse-point shadow answer).
  - A later client answer for that `query_id` is refused with `input_refused { reason: already_replied }`, or `input_refused { reason: query_expired }` if the fallback is still pending (Core EV-8(i)).
  - If admission of the fallback is refused (for example, the lane is full), the query stays pending on the fallback path only. **The stale client-answer path is never reopened.** If it still makes no progress after `route_limits.stall_close_after_ms`, the route ends with `stalled`. | `conf::ts_7_progressing_lossless`, `conf::ts_7_stall_resync_then_close`, `conf::ts_7_unread_refusals_stop_input_reads` |
| TS-8 | **A healthy close** drains. Every output frame queued for the route is sent. For `session_ended`, that includes **the final PTY tail through Core's final read** before the process exit. Then `route_closed { reason }` is the last frame, then EOF or the channel close. Healthy reasons: `detached`, `replaced`, `session_ended { exit { code, signal } }`, `revoked`, `attach_failed`. (A Hub shutdown does **not** end direct routes, BP-7.) | `conf::ts_8_drain_includes_final_tail`, `conf::ts_8_close_is_last` |
| TS-9 | **A failed route** (the transport died, or it never connected) may lose undelivered output. The worker reports the end to the Hub over its control link, and the Hub reports it with `event { stream_id: "routes", owner: "hub", name: "route.ended", subject: { route_id }, payload: { reason }, priority: "high", source, at }` (the section 8 shape, on the connection's implicit, always-open `routes` stream; `route_ended` below means this event) on the owning client's control connection, when that connection is alive. `reason` ∈ `not_connected`, `handoff_failed`, `transport_lost`, `write_failed`, `stalled` (TS-7), `session_lost`. A failed route may lack `route_closed`, and it may lose output. It may also lose undelivered `input_done` and `input_refused` frames. Input handed to it before the failure has an unknown outcome, unless a frame for that input already arrived (TS-5c). **EOF without `route_closed` means that the cause is unknown** until `route_ended` arrives or the session watch explains it. | `conf::ts_9_failed_route_reported`, `conf::ts_9_eof_without_close_is_unknown` |
| TS-10 | `invoke(session, id, detach, { route_id })` ends the route with `detached`. A client that closes its route connection ends only that route (TS-9 `transport_lost`). The session keeps running. **When a route ends** (by any close) **whose latest focus input was `focused: true`, and no remaining route's latest focus input is `focused: true`,** and the program enabled focus reporting, the worker writes a focus-out to the program, as a terminal losing its window would (Core DP-12; parity CG-17). | `conf::ts_10_detach`, `conf::ts_10_last_focused_route_end_writes_focus_out` |
| TS-11 | **Route identity.** A `route_id` names one attachment. A re-attach or a transport recovery makes a new route. **A Hub restart does not end direct routes** (BP-7). A worker exit ends them. The durable identity is `{hub_id, session_id}`. | `conf::ts_11_reattach_after_worker_exit` |

## 7b. Plugin views (generic UI)

Views are **declarations** (plugin-platform §12, approved; a cold replacement of the render callbacks). A `view` carries a **ui-contract declarative view document**. `describe` names its vocabulary version as `ui_contract_view: "M.m"`. The document has:
- **data components** bound to kinds: `list`, `table`, `detail`, `form`, `actions`, `status_badge`;
- ****seven structural** parts: `stack`, `inline`, `panel`, `section`, `toolbar`, `text`, `empty_state`, with presentation state (set, clear and toggle with predicates), conditional parts and responsive widths. There is no `dialog` document part. **ui-contract owns the vocabulary** (HP VW-6), including number and metric display options on fields. **A shared view is never filtered by client capability:** a client renders what it can and uses the EN-6 placeholder for the rest.

It is **never** a plugin-rendered tree, and there are no render callbacks. **One exception (user decision, 2026-09-30): plugin HTML in a sandboxed iframe,** served from the plugin's own origin, never the client's. The embedding rules and the sandbox are ui-contract's (0017), and the admission and the separate assets origin are HP's (VW-10, AS-1 to AS-4). A client that cannot embed it (for example the TUI) shows it as an external link. ui-contract owns rendering and the full vocabulary. If a user action of a first-party plugin (for example botster-workspaces) cannot be expressed in this vocabulary, that goes to the Hub→plugin pair and, if it needs a product change, to the user.

| # | Clause | Test |
|---|---|---|
| SF-1 | A `view` names its component, the kind it binds (with an optional filter), the fields it shows and the actions it offers. **Each field and action has a stable `field_id` or `action_id`** across plugin reloads that keep the view's schema MAJOR. | `conf::sf_1_stable_ids` |
| SF-2 | An action is `invoke(<bound kind>, target?, action, args)`. A refusal carries `field_errors: [{ field_id, pointer, code, message }]`, so a form can mark its fields. | `conf::sf_2_field_errors_by_field_id` |
| SF-3 | **Secrets and defaults.** A field with `secret: true` never carries its value to a client; it shows only `set: bool`. An empty value for a secret field means "unchanged", unless the client sends `clear: true`. Defaults are part of the schema, and the client shows them as defaults, not as values. | `conf::sf_3_secret_never_sent`, `conf::sf_3_empty_secret_unchanged` |
| SF-4 | **Options from watched kinds.** A select declares `options_source { kind, where?, value_field, label_field, order, exclude?: { kind, where?, value_field } }`. The client **watches** those kinds while the view is shown. The options follow section 5. | `conf::sf_4_options_watched`, `conf::sf_4_exclude` |
| SF-0 | **Plugin UIs are complete through this path.** A plugin's kinds are watchable and invokable with the same verbs, watch semantics, filters and errors as Hub kinds (DS-7). A plugin's views bind those kinds, and its actions are `invoke` on them. **Access is the plugin's own policy,** applied over the **verified caller** that the Hub passes with every request (`operator` with its `credential_id` when present, or a verified session; Hub→plugin **VC-1**). That is sufficient for a plugin-owned inbox: the messaging plugin decides which inbox rows and actions a caller may see and use. The Hub adds no inbox binding. | `conf::sf_0_plugin_inbox_view`, `conf::sf_0_plugin_policy_sees_verified_caller` |
| SF-5 | **Notices: the Hub is the one evaluator.**
- **Declarations:** the `package` kind shows its `notice_reactions` (`id` (stable within the package; the notice's `reaction`, HP VW-9), `owner`, `name`, `subject_scope`, `text_pointer`, `ttl_ms`, `severity` ∈ `info`, `success`, `warning`, `error`, `delivery` ⊆ {`in_app`, `push`}, `url_pointer?`, `tag_pointer?`) for display only. Their admission rules, including the `ttl_ms` bound, are HP VW-9's. **A client never evaluates a reaction** and never derives a notice from a raw event.
- **Actor:** the Hub evaluates each reaction once, at event ingress (HP VW-9), and delivers the result as a notice. The same evaluation feeds Web push (SF-5b).
- **Delivered shape and stream:** `watch(event, { owner: "hub", name: "notice" })` gives `event { owner: "hub", name: "notice", subject?, priority, payload: { notice_id, package, reaction, severity, text, url?, tag?, ttl_ms }, … }`. `subject` is the source event's subject. `reaction` is the reaction's stable `id` (HP VW-9). `notice_id` is a Hub-minted opaque id, one per occurrence, shared by the stream copy and the push copy. **A client dedupes by `notice_id` only**, never by `(owner, name, subject)`, which does not identify one occurrence.
- **Audience:** authorized operator clients only. A session caller never receives notices. **A notice never widens its source's audience** (HP VW-9): a reaction is admitted only for an event whose audience includes `clients`, or for a Hub-owned event; `hub`/`notice` itself is never a reaction source; and at dispatch, a recipient gets the notice (on the stream or by push) only if it could receive the source event (EV-2 audience and watch permission).
- **Tag:** a notice replaces an earlier one only when `(package, reaction, tag)` are all equal.
- **Loss:** a notice-queue overflow is an exact `event_gap` count on the notice stream, which a client shows as "notices may have been missed".
- **Channels:** the notice stream carries only notices whose reaction's `delivery` includes `in_app`, so a push-only reaction never renders in the app. Push (SF-5b) sends only notices whose `delivery` includes `push`. Both channels carry the **same evaluated payload**.
- **Client role:** a client renders delivered notices and may show fewer (for example, notices turned off). **Raw events stay independent:** watching the source event gives the raw event, never a notice.
| `conf::sf_5_hub_delivers_notice_once`, `conf::sf_5_dedupe_by_notice_id_only`, `conf::sf_5_push_only_reaction_never_in_app`, `conf::sf_5_both_channels_same_payload`, `conf::sf_5_client_never_evaluates_reactions`, `conf::sf_5_tag_replaces_within_package_and_reaction`, `conf::sf_5_notice_gap_counted`, `conf::sf_5_session_caller_gets_no_notices`, `conf::sf_5_notice_keeps_source_audience`, `conf::sf_5_notice_is_never_a_reaction_source` |
| SF-5b | **Web push for notices** (parity G-11). A client that supports Web Push registers `create(push_subscription, { endpoint, keys: { p256dh, auth } })` under its own credential. The `hub` row carries `push_public_key` (the Hub's VAPID key, RFC 8292). For each notice (SF-5) whose reaction's `delivery` includes `push`, the Hub sends an encrypted push (RFC 8291) of **the same evaluated notice payload as SF-5**, `{ notice_id, package, reaction, severity, text, url?, tag?, ttl_ms }`. The push tag is namespaced as `(package, reaction, tag)`, as on the stream. There is no separate title or body mapping to each **eligible** subscription. A subscription is eligible only when its credential is live and could receive that notice on the notice stream (SF-5: an authorized operator). A notice that the credential could not receive on the stream is never pushed. Push is best effort and at most once. A push service answer of "gone" deletes that subscription. **No plugin ever sees a subscription or an endpoint**, and `list` never returns another credential's endpoint. Deleting a `client_credential` deletes its subscriptions. | `conf::sf_5b_push_for_declared_notice`, `conf::sf_5b_push_only_to_eligible_audience`, `conf::sf_5b_gone_endpoint_deleted`, `conf::sf_5b_endpoint_never_reaches_plugin`, `conf::sf_5b_revoke_deletes_subscriptions` |
| SF-5c | **Terminal-notification policy for plugin session types** (parity G-12). A session type may carry `notifications: { mode: all | none | matching, patterns: [literal substring] }`, declared through HP. An **attached** client applies it to OSC 9, OSC 777 and the bell on its raw route (EV-6). For clients that are not attached, the Hub applies it once when it raises `hub/session_notification` (SF-5d). `matching` shows a notification only when its text contains a pattern. **The matching input and defaults are HP CA-22:** patterns are case-sensitive literal substrings tested against the notification text (the OSC 9 message; the OSC 777 title and body joined by one newline); a bell has no text and never matches under `matching`; `matching` with an empty pattern list matches nothing; a session type without `notifications` behaves as `all`. No plugin code runs on the notice path. | `conf::sf_5c_matching_filter_applied_by_client` |
| SF-5d | **Terminal notifications reach every client** (user decision UC-1, 2026-09-30: "notifications should bubble to clients that are not connected to that session"):
- **The attached, focused client acts on the raw bytes** (P9, EV-6), unchanged.
- **The Hub always raises an event:** for each Core semantic notification of a session (the bell, OSC 9 and OSC 777, Core EV-7), the Hub emits `event { owner: "hub", name: "session_notification", subject: <session>, payload: { session: SessionRef, source: bell | osc9 | osc777, title?, body?, at } }` on the event stream (section 8) of every client that may read the session **and is not attached to that session's terminal route**. **The Hub applies the session type's SF-5c policy once, when it raises the event**; the client does not filter it again. Plugins receive it only under HP `events_terminal[:own|:any]`. **Permission prompts are not terminal features**, and Core does not detect them (a program-specific interpretation of output). A plugin that detects them surfaces them through a notice reaction (SF-5).
- **Exactly one surface per client** (HP EV-9, CA-22): an attached client, focused or not, gets the notification from its own route and acts as a native terminal would, applying SF-5c to the raw path. A client that is not attached gets the event and delivers it. No client gets both.
- **Attention:** when a notification arrives while **no route of the session is focused** (the worker reports the session's focus state to the Hub: Core `FocusChanged`, also reported once after adoption), the Hub sets the row's `attention { since, source, title?, body? }`. `invoke(session, id, clear_attention)` clears it, and so does a route of that session becoming focused.
- **Push** (user decision, 2026-09-30: "lock in operator with no client focused"; one Hub per machine means one human, the operator): when **no human client is focused on the session**, the Hub sends the notification by Web push (SF-5b, with the SF-5c filter) to the operator's registered devices. A human client is the TUI, the CLI or a paired credential; **an agent never counts**, and it has no route (TB-M). **One alert per device:** a subscription whose credential has a live client connection gets no push, because that client shows the notification itself (in the app, or as an OS notification from the page). So the operator is reached with no Botster tab open.
- **Unknown focus counts as not focused:** a Core focus state of `unknown` (an unreachable or older worker) sets `attention` and allows push, so a notification is never lost to an unknown state.
- **Focus is human focus:** only routes count ("a route of the session is focused"), and only human clients hold routes.
| `conf::sf_5d_event_for_every_reader`, `conf::sf_5d_attached_client_gets_no_event`, `conf::sf_5d_attention_set_only_when_unfocused`, `conf::sf_5d_focus_or_clear_clears_attention`, `conf::sf_5d_push_only_when_no_human_client_focused`, `conf::sf_5d_push_only_without_live_connection`, `conf::sf_5d_agent_never_counts_as_focus`, `conf::sf_5d_policy_filters_event_and_push` |

## 8. Event streams (not resources)

**One event system** with the Hub→plugin contract (HP §10, EV-1 to EV-15). This section states only the client view of it. Where the two differ in wording, **the plugin contract's event semantics govern**.

| # | Clause | Test |
|---|---|---|
| EV-1 | **Identity and fields.** An event is `{ owner, name, subject?, priority, source, payload, at }` (HP EV-9):
- `owner` is `hub`, `terminal` or a plugin key.
- `source` is `{ kind: hub | plugin | terminal, instance, session?, hub_id? }` (HP EV-9), where `instance` tells a restarted source from the old one, `session` names the session a terminal event is about, and an absent `hub_id` means the local Hub.
- `priority` is `high`, `normal` or `low`, fixed per event name.
- `at` is stamped by the Hub. | `conf::ev_1_event_fields` |
| EV-2 | **Streams.** `watch(event, { owner, name, subjects?, min_priority? })` opens a stream. A stream frame is `event { stream_id, owner, name, subject?, priority, source, payload, at }`, with the fields of HP EV-9 plus `stream_id`. The pattern rules are HP EV-2: `owner` or `name` may be `*`, and `name` may end in `.*`. `min_priority` defaults to `low`. **A client receives only events whose declared audience includes `clients`** (HP EV-1), plus `hub` events. There is no snapshot, no replay and no cursor. | `conf::ev_2_client_audience_only`, `conf::ev_2_pattern_and_min_priority` |
| EV-3 | **Delivery** (HP EV-4, EV-10, EV-11): at most once per copy, with no replay across a restart. There is order per `{owner, name}` and source instance, and no order across names, sources or priorities. Each client stream has its own bounded queue per priority class. Higher classes go first, with a starvation bound. A slow client never stalls the Hub, an emitter or another subscriber. | `conf::ev_3_per_name_order`, `conf::ev_3_slow_client_does_not_stall` |
| EV-4 | **Loss is explicit, and recovery is stated, not assumed** (HP EV-12, EV-13).
- **Loss by class:** a high copy is never lost without a ledger entry. Normal events coalesce per key (the older copy is removed and the latest is appended at its emit position), or drop the oldest if unkeyed. Low events drop the oldest.
- **The marker** is `event_gap { stream_id, owner, name, subject?, count, lost_bytes?, recovery, state_read?, aggregated? }`, at the position of the first lost copy. `count` (lost events) and `lost_bytes` are separate exact counters, and they are never added together.
- **`recovery`** is one of:
  - `state`: **this subscriber** has an accessible, permitted read of authoritative state, and the marker carries a **`state_read`** descriptor naming it: `{ surface, arguments?, reference?, grant }`, the exact read-only existing read (for a client, a Hub or plugin kind to `get` or `watch`). A descriptor **never creates authority**;
  - `unavailable`: there is no accessible state, so handling the gap acknowledges the loss only and **never implies restored state**;
  - `partial`: an aggregated gap, where each name follows its owner's state contract. **It carries no descriptor.**
- When the ledger is full, entries aggregate exactly (per owner, then `owner: "*"`), keeping the per-unit counts.
- **Gap markers bypass the stream's `min_priority` and subject filters.**
- **Marker retirement:** a client marker is retired when its **complete frame has been written** to the stream. There is no client acknowledgement. Once any byte or chunk of a marker has started, its summary is **immutable**, and losses that arrive after that are kept as a **new** obligation with a new marker. | `conf::ev_4_high_loss_has_ledger_entry`, `conf::ev_4_counts_exact_per_unit`, `conf::ev_4_recovery_state_or_unavailable_or_partial`, `conf::ev_4_gap_bypasses_filters` |
| EV-5 | `unwatch(stream_id)` ends the stream as in WT-6. | `conf::ev_5_unwatch` |
| EV-6 | **Terminal events and clients.** The owner `terminal` (`title`, `cwd`, `bell`, `notification`, `prompt_mark`, `modes`, `exit`; HP EV-14) exists for plugins. **Clients that are attached get these features from the raw route `output`** (clients that are not get SF-5d's `session_notification`) (P9, where the client is the one actor). A client that also watches `terminal` events must not act on them a second time. The raw output tap `terminal/output` is **plugin-only** (scope `session_output:read`). Clients read output on their routes. | `conf::ev_6_client_does_not_double_act`, `conf::ev_6_output_tap_not_offered_to_clients` |

## 9. Transport bindings

*Implementation note (non-normative):* each client is **one shared client core** (the envelope, watches, the route codec and queries), plus **a transport adapter per binding** and **a renderer per client**. The TUI and Web differ only in their renderer and the bindings their platform supports.

**Every clause outside section 9 is transport-neutral** (user requirement, 2026-09-30: "Ideally TUI and Web are incredibly similar and transport is the only unique part"). Such a clause names no binding.

### 9.0 What a binding is

| # | Clause | Test |
|---|---|---|
| TB-0 | **A binding defines ONLY these things:**
- (1) **frame delimiting**, for the control connection and for routes;
- (2) **peer authentication** to a caller;
- (3) how **progress** is measured for backpressure (TS-7);
- (4) the **close signal**;
- and the connect step that carries (1) to (4).

Everything else (the envelope, `describe`, watches, events, the terminal stream, input, queries, errors) is shared and binding-neutral.

**A binding is not tied to a client kind.** Any client may use any binding its platform supports. For example, a TUI on another machine attaches over `webrtc`, and a Web client could use a local stream where the platform allows it.

**The set is extensible:** a new binding is a new section 9 entry plus a feature name `binding:<name>` in `features[]`. `transport.binding` is an open string, and a client uses only bindings the Hub advertises. The v1 bindings are `local-stream` (9.1), `webrtc` (9.2) and the partial `mcp` binding for agents (9.4). | `conf::tb_0_transcripts_run_unchanged_on_every_binding` (the section 12 transcripts, byte for byte, on each binding), `conf::tb_0_any_client_kind_any_binding`, `conf::tb_0_unknown_binding_not_offered` |

### 9.1 Binding `local-stream` (a Unix-domain stream socket on the Hub's host)

| # | Clause | Test |
|---|---|---|
| TB-L1 | **Socket discovery** (a Unix client finds the one Hub socket; CLI-RT3). In order: (1) the path in `BOTSTER_HUB_SOCKET`; (2) `$BOTSTER_CONFIG_DIR/hub.sock` when `BOTSTER_CONFIG_DIR` is set (this isolates a test Hub); (3) `<runtime dir>/<app>/hub.sock`, where `<runtime dir>` is `$XDG_RUNTIME_DIR` if set, else `/tmp/botster-<uid>`. The Hub creates the directory with mode 0700 and the socket with mode 0600 (TB-L3). A path over 103 bytes (the portable socket address limit) is refused with a message that names the limit; neither side shortens it silently. | `conf::tb_l1_discovery_order`, `conf::tb_l1_directory_and_socket_modes` |
| TR-1 | **Control.** One control connection to the Hub (the socket path). Frames are `[u32 big-endian length][UTF-8 JSON]`, where `length` counts the JSON bytes and is at most `welcome.limits.max_frame_bytes`. | `conf::tr_1_unix_framing` |
| TB-L2 | **Route connect: a token handshake on the Hub's own listener. There is no temporary route path.**
- `transport = { binding: "local-stream", socket_path, route_token }`. `socket_path` is the Hub's listener. `route_token` is **unguessable**, **single-use**, bound to the `route_id` and to the authenticated caller and peer uid, and valid only within `limits.route_connect_seconds`.
- The client opens a **new** connection to `socket_path`. Its first and only handshake frame uses the **control framing** (TR-1: `[u32 big-endian JSON byte length][UTF-8 JSON]`, bounded by `welcome.limits.max_frame_bytes`): `{ "frame": "route_connect", route_id, route_token }`. The listener tells a route connection from a control connection by this first frame (`hello` or `route_connect`), without reading any terminal frame. **After the handoff, the connection carries TS-3 route frames.**
- The Hub reads **exactly** that bounded handshake and verifies the token, the route, the peer uid and the connect window. **An invalid proof does not consume the token.** **Successful verification consumes the token atomically:** if two connections present the same valid token, exactly one gets the route, and the other is closed with `close { route_token_used }`.
- The Hub then hands the connected stream to the session's worker (BP-1; for example by descriptor passing, non-normative) **without reading any terminal frame**, and closes its own copy.
- If the handoff fails, the reservation ends with a typed reason (`route_ended { handoff_failed }`, TS-9).
- A route that is not connected within the window ends with `not_connected`.

**Delimiting:** `[u32 big-endian length][frame]`, where `length` is the frame's size (TS-3). | `conf::tb_l2_token_single_use_bound_to_route_and_uid`, `conf::tb_l2_concurrent_same_token_one_wins`, `conf::tb_l2_handshake_uses_control_framing`, `conf::tb_l2_invalid_proof_keeps_token`, `conf::tb_l2_no_terminal_frame_read_by_hub`, `conf::tb_l2_handoff_failure_typed`, `conf::tb_l2_no_temporary_paths` |
| TB-L3 | **Authentication:** the socket is mode 0600, and the Hub checks the peer uid **[today]**. A client on this binding does not pair. | `conf::tb_l3_peer_uid` |
| TB-L4 | **Progress** (Core OU-3a): bytes that the kernel accepted from the worker's write to the route socket. A readiness report that takes no bytes is not progress (TS-7). **Close:** end of stream. | `conf::tb_l4_progress_and_eof` |

### 9.2 Binding `webrtc`

| # | Clause | Test |
|---|---|---|
| TR-2 | **WebRTC.** Signalling stays outside the envelope. The **Hub peer connection** carries **one** reliable, ordered `control` DataChannel for the envelope, watches and event streams. **Each route has its own peer connection, terminated by the session worker** (BP-1, BP-4), with one reliable, ordered DataChannel. | `conf::tr_2_control_on_hub_peer_routes_on_worker_peers` |
| TB-W2 | **Route connect.** `transport = { binding: "webrtc" }`. The client creates the route's **own** peer connection and sends `invoke(session, id, attach_offer, { route_id, sdp, client_fingerprint, signature })` on the control channel (BP-4). The answer is `{ sdp }`, the worker's answer. On that peer connection the client opens one reliable, ordered DataChannel labelled **`terminal`** (fixed: each route has its own peer connection). **ICE is not trickled:** both the offer and the answer carry the complete candidate set. | `conf::tb_w2_route_on_worker_peer` |
| BP-4 | **Route authentication, with no pairing secret in the worker.** After `attach` reserves the route (TS-1), `attach_offer` on the authenticated control channel carries the route peer connection's offer **and the client's signature**, made with its paired key (PR-3) over `botster-route-v1\n<route_id>\n<client_fingerprint>`. The Hub verifies it, passes the offer and the **expected client fingerprint** to the worker, and returns the worker's answer on the control channel. That control channel is authenticated, so the client learns the worker's fingerprint from a trusted source. The worker accepts only a DTLS peer with the expected fingerprint. Untrusted signalling learns nothing new. | `conf::bp_4_route_peer_fingerprint_bound`, `conf::bp_4_forged_route_offer_refused` |
| TR-3 | **WebRTC framing.** Each DataChannel message is `[u8 flags][chunk]`. Bit 0 of `flags` = `final`, and the other bits are 0. A frame is the concatenation of its chunks, up to and including the one marked `final`. Each chunk is at most `welcome.limits.max_chunk_bytes` on the control channel, and at most **`route_limits.max_chunk_bytes`** on a route (each chosen at or below the SCTP maximum message size). A control frame is at most `welcome.limits.max_frame_bytes`. A route frame is at most its type's `route_limits` bound (TS-1). A frame that is incomplete when its channel closes is discarded. Control frames are UTF-8 JSON, and route frames are TS-3 frames without the length prefix. **Reliable ordered channels do not lose single messages.** A transport failure closes the channel, and it ends the routes on it (TS-9). | `conf::tr_3_chunking`, `conf::tr_3_incomplete_frame_discarded` |
| TR-4 | **Head-of-line.** Large baselines are chunked (WT-1). Between two chunks of one baseline, the Hub may send, and the client may send, any other frame: responses, cancels, other watches' frames, events. A large baseline never holds the control stream. | `conf::tr_4_responses_between_baseline_chunks` |
| TB-W4 | **Progress** (Core OU-3a): new route payload that leaves the bounded SCTP send queue through transport acknowledgement (the cumulative acknowledgement advances). A retransmission, a heartbeat, a UDP write or a local enqueue is not progress (TS-7; Core's `route_send_buffer_bytes`). **Close:** the channel close, or the peer connection failing. | `conf::tb_w4_progress_and_close` |
| TR-5 | **Trust scope (USER RULING, 2026-09-30): signalling is untrusted.** This covers signalling through any relay. The only trusted input is a **pairing bundle** that the operator carries out of band in the **URL fragment** of a link or QR code. A fragment is never sent to any server. **The traffic keys are DTLS session keys.** The paired keys **authenticate the DTLS fingerprints** (PR-5 to PR-7); they do not derive the traffic keys. Nothing is trusted from the signalling path.  DTLS encrypts every channel, and there is no application-layer cipher: the DTLS session is bound to the paired keys by PR-5. | `conf::tr_5_signalling_carries_no_secret` |

### 9.3 Pairing and peer authentication (binding `webrtc`)

| # | Clause | Test |
|---|---|---|
| PR-0 | **Issue.** `invoke(hub, hub_id, issue_pairing, { ttl_seconds? })` (effect **`immediate`**, because the secret lives only in memory; permission `hub.pair`). Accepting a client credential (PR-7) and revoking one (PR-8) are **`committed`**. Only an authenticated operator may call it: a Unix client, or an already paired client. It returns `{ url, qr_text, expires_at }`, where `url` is `<web origin>/hubs/<hub_id>#<bundle>` (PR-2). **The web origin is provided by the web plugin** (user decision, 2026-09-30): either a configured public host that serves the Web app, or the plugin serving the Web app itself (HP WP-1 `web.set_origin`, WP-5 static serving). The origin is **only the URL text** in the pairing link: it carries no trust, because the bundle pins the Hub key (PR-3). The `hub` row shows it as `web_origin?`. With no web plugin providing an origin, `issue_pairing` is refused `wrong_state { reason: no_web_origin }`. The Hub keeps the pairing secret **in memory only** (it must, to verify the PR-6 HMAC). The secret is cleared when it is consumed, regenerated, revoked or expired. **A Hub restart invalidates unused pairing links.** Browser credentials and the Hub identity key are durable. The CLI prints the URL and a QR code. Issuing a new bundle invalidates any unused earlier one. | `conf::pr_0_issue_requires_operator`, `conf::pr_0_new_bundle_invalidates_old`, `conf::pr_0_restart_invalidates_unused_link` |
| PR-1 | **Hub identity.** The Hub has a long-lived Ed25519 identity key. Its public key is part of every pairing bundle. | `conf::pr_1_hub_identity_stable_across_restart` |
| PR-2 | **The pairing bundle** is `base32(version:u8 ‖ expires_at:u64be ‖ route_len:u16be ‖ route ‖ hub_public_key[32] ‖ pairing_secret[32])` without padding, carried as the **URL fragment** (`#…`). `route` is `/hubs/<hub_id>`. The secret is random and **single-use**: after one successful pairing, or at `expires_at`, the Hub mints a fresh one for the next bundle. | `conf::pr_2_bundle_codec`, `conf::pr_2_secret_single_use`, `conf::pr_2_expired_bundle_refused` |
| PR-3 | **Client key.** On first use, the client makes a **non-extractable** Ed25519 key pair and stores it per `hub_id`. It **pins** the Hub public key from the bundle. A later bundle or challenge with a different Hub key is refused ("Hub public key changed"). | `conf::pr_3_pinned_hub_key_change_refused` |
| PR-4 | **Signalling is untrusted,** whether local or through a relay. Its messages are exactly:
- `signal_offer { hub_id, connection_id, sdp }` from the client;
- `signal_answer { connection_id, sdp }` or `signal_refused { connection_id, reason }` from the Hub;
- optional `signal_candidate { connection_id, candidate }` in either direction.

**Where signalling lives: the web plugin** (user decision, 2026-09-30). A client posts these messages to `<web origin>/hubs/<hub_id>/signal`, the endpoint of the pairing URL's origin. On an **external** origin, the web plugin carries them between that origin and the Hub through the untrusted relay of HP WP-2 to WP-4; on a **served** origin, the Hub itself answers `POST <origin>/hubs/<hub_id>/signal` (HP WP-2). In both cases they travel as opaque envelopes under a Hub-minted `relay_id`, through a bounded queue. The relay carries **only** these messages; the Hub stays the one that answers them (the Hub peer). The web plugin never sees route bytes, the pairing secret, a key or a signature: each is outside these messages (PR-6, PR-7 travel only inside DTLS). **ICE servers** (STUN and TURN, with any credentials) come from the same web origin, `<web origin>/hubs/<hub_id>/ice`, for the client, and from the web plugin for the Hub and its workers (HP WP). They are untrusted like signalling: a hostile TURN relay can deny service or carry DTLS traffic it cannot read (parity CG-8). **General session forwarding stays out of v1** (deferred). They never carry the pairing secret, a client key, a signature, a session key or terminal data. A signalling service can drop, delay or forge these messages. It can therefore deny service, but PR-5 to PR-7 make it unable to read channel data or to impersonate either peer. | `conf::pr_4_no_secret_in_signalling`, `conf::pr_4_forged_answer_fails_auth` |
| PR-5 | **Channel binding.** Once DTLS connects, and **before any envelope frame**, the Hub sends `auth_challenge { protocol, protocols: [..], route, hub_public_key, challenge (random), client_fingerprint, hub_fingerprint }` on the control channel. Both fingerprints are the DTLS certificate fingerprints **as the Hub sees them**. The client compares them with the fingerprints in **its** local and remote SDP. A mismatch aborts the connection. That is how a relay that terminates DTLS on each side is detected. | `conf::pr_5_fingerprint_mismatch_aborts` (a man-in-the-middle relay with two DTLS sessions) |
| PR-6 | **The client's proof.**
- **First pairing:** `pair_request { protocol, challenge, client_public_key, expires_at, proof }`, where `proof = HMAC-SHA256(pairing_secret, pairing_transcript)`.
- **Later connections:** `auth_response { protocol, challenge, client_public_key, signature }`, where `signature` is Ed25519 over `auth_transcript`.

**The Hub validates all of these, and refuses on any failure:**
- `protocol` is one the Hub listed in `auth_challenge.protocols`. The client answers with the highest protocol both support, and **a client refuses an unknown bundle `version`** with "update the client";
- `challenge` is the one issued **on this peer connection** and not yet used;
- `route` and the Hub key equal the Hub's own;
- for pairing, `expires_at` equals the bundle's `expires_at`, and that time is in the future;
- the public key is exactly 32 bytes;
- the HMAC is compared **in constant time**;
- for `auth_response`, the key belongs to an **active** (not revoked) credential.

**An invalid proof does not consume the secret.** A valid pairing consumes the secret atomically **only after** the new credential is durably committed. | `conf::pr_6_each_check_refuses` (one case per check), `conf::pr_6_invalid_proof_keeps_secret`, `conf::pr_6_consume_after_commit` |
| PR-6b | **Lost acceptance.** If `auth_accepted` for a first pairing is lost after the secret was consumed, the client already holds its private key. It retries with **`auth_response`**, and the Hub accepts it because the credential was committed. A client never replays `pair_request` forever: after a refused `pair_request` with a consumed secret, it tries `auth_response` once before asking for a new link. | `conf::pr_6b_retry_as_auth_after_lost_accept` |
| PR-7 | **Acceptance.** The Hub answers `auth_accepted { protocol, challenge, hub_public_key, client_public_key, credential_id, paired, signature }`, where `signature` is Ed25519 by the Hub identity key over the `accepted_transcript`. The client verifies every field and the signature. After a successful first pairing, it deletes its stored pairing secret. | `conf::pr_7_hub_signs_acceptance`, `conf::pr_7_client_verifies_fields` |
| PR-7b | **Exact encodings and transcripts** (the test vectors are part of the conformance suite):
- **Encodings:** keys, the proof and signatures are base64url without padding. Ed25519 public keys are 32 bytes and signatures 64 bytes. The HMAC is 32 bytes.
- **`credential_id`:** lowercase hex of the first 16 bytes of SHA-256 of the raw client public key.
- **`expires_at`:** Unix seconds.
- **Fingerprints:** the value of the SDP `a=fingerprint:` line, trimmed and lowercased, for example `sha-256 ab:cd:…`.
- **Transcripts:** each field on its own line, joined with `\n`:
  - `pairing_transcript` = `botster-pairing-v1`, route, hub_public_key, challenge, client_public_key, expires_at, client_fingerprint, hub_fingerprint;
  - `auth_transcript` = `botster-auth-v1`, route, hub_public_key, challenge, client_public_key, client_fingerprint, hub_fingerprint;
  - `accepted_transcript` = `botster-auth-accepted-v1`, route, challenge, client_public_key, credential_id, `pair` or `auth`, client_fingerprint, hub_fingerprint. | `conf::pr_7b_vectors` |
| PR-8 | **No data before mutual authentication.** Until PR-7 completes, the Hub accepts no `hello`, no data and no route channel on that peer connection. A challenge belongs to **the actual peer connection**, not to a relay-supplied `connection_id`. The caller of an authenticated connection is `operator` with `credential_id`. **Revocation:** `delete(client_credential, credential_id)` (`committed`) removes a credential. The `client_credential` kind has the core fields `id`, `created_at` and `last_seen_at`. Revocation **immediately closes** every peer connection, route, watch and event stream of that credential, and every later authentication with it fails. | `conf::pr_8_no_hello_before_auth`, `conf::pr_8_revocation_closes_live_connections`, `conf::pr_8_revoked_credential_refused` |
| PR-9 | **Trust boundary.** The system resists an untrusted signalling service or relay. It does **not** protect against an attacker who controls the browser code or the page that reads the fragment: that code is inside the trust boundary. The pairing secret authenticates the peers, and DTLS derives the traffic keys (TR-5). | (a statement of scope) |

**Pairing belongs to the binding, not the client kind.** A client on `local-stream` does not pair (TB-L3). A client on `webrtc` pairs, whatever it is: a browser, a TUI on another machine, or a CLI. The monorepo pairing protocol number was 7. The modular protocol starts at `1` with the same shape, as a cold cut.

### 9.4 Binding `mcp` (agents; partial)

| # | Clause | Test |
|---|---|---|
| TB-M | **MCP is a client binding** (user decision, 2026-09-30: agents are clients). It defines, as TB-0 requires:
- **delimiting:** MCP JSON-RPC over the Streamable HTTP transport (MC-0), where one tool call is one request verb (MC-2);
- **peer authentication:** the bearer token (MC-1), whose caller is the agent's `SessionRef`, with the MC-1b scope set;
- **progress and close:** the MCP session's lifecycle.

**What maps, and what does not:**
- **Request verbs** (`describe`, `list`, `get`, `create`, `update`, `delete`, `invoke`) map one to one to tools (MC-2), and `cancel` maps to MCP `notifications/cancelled`.
- **`watch` maps to MCP resources:** each row is a resource `botster://<hub_id>/<kind>/<id>`, and each kind is a list resource `botster://<hub_id>/<kind>`. `resources/subscribe` gives `notifications/resources/updated`, and the agent re-reads with `resources/read` or `get`. **The notification carries no payload**, so an agent sees current state, not every intermediate revision (level-triggered, as WT-4). 
- **Event streams (section 8) are not offered:** MCP gives a tool no server push with a payload, and a lossy stream behind a level-triggered notification would hide `event_gap`. An agent reads state with `get` and `list`, and a plugin that wants agents to see its events offers a tool or a kind.
- **Terminal routes (section 7) are not offered:** a route is a live, bidirectional, backpressured byte stream with query answering (P9), and MCP carries none of these. An agent reads a terminal with `invoke(session, read_screen)` and writes with `invoke(session, safe_write | write)` (section 6).

Any other binding feature that MCP cannot carry is refused with `unsupported`, never emulated. | `conf::tb_m_verbs_map_one_to_one`, `conf::tb_m_watch_as_resource_subscription`, `conf::tb_m_no_event_streams_or_routes`, `conf::tb_m_scope_set_applies` |

## 10. Agents are clients: the `mcp` binding

**User decision (2026-09-30): an agent is a client.** It can do what a client can, within a scope set that the operator can restrict. MCP is **a client binding** (TB-M) over the same verbs, kinds and actions. It is not a separate, narrower product surface.

| # | Clause | Test |
|---|---|---|
| MC-0 | **MCP is referenced, not restated.** The Hub implements the Model Context Protocol revision **2025-06-18 or later**: the supported set is negotiated per the specification: its lifecycle (https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle), its tools (…/server/tools) and its Streamable HTTP transport (…/basic/transports).
- **Version negotiation follows the spec:** if a client requests a revision the Hub does not support, the Hub answers with its supported revision, and the client decides whether to disconnect. 
- The Hub validates `Origin` and the `MCP-Protocol-Version` header as the transport spec requires. | `conf::mc_0_negotiates_revision`, `conf::mc_0_origin_validated`, `conf::mc_0_protocol_version_header` |
| MC-1 | The Hub serves MCP on loopback **[today]**. An agent gets `BOTSTER_MCP_URL` and `BOTSTER_MCP_TOKEN` and sends `Authorization: Bearer <token>`. The token proves the caller `SessionRef { hub_id, session_id }`. | `conf::mc_1_bearer_proves_caller` |
| MC-1c | **The spawned-session environment.** The Hub removes every inherited `BOTSTER_*` variable from a session it spawns, and sets exactly `BOTSTER_SESSION_ID`, `BOTSTER_MCP_URL`, `BOTSTER_MCP_TOKEN` and `BOTSTER_CONTEXT_FILE`, plus `BOTSTER_SESSION_PORT` when the session type allocates a port (HP CA-20). The Hub also sets each name in the session type's `port_env` (for example `PORT`) to the same port (parity G-19). A `port_env` name is validated as HP CA-20 defines: it matches `[A-Z_][A-Z0-9_]*` and does not start with `BOTSTER_` or `CORE_`, or the session type is refused. **Session context** (parity CG-3): `BOTSTER_CONTEXT_FILE` is the path of a file readable only by the session's user. **This clause owns its schema** (HP CA-14 delivers it unchanged): a UTF-8 JSON object `{ prompt?, context: { <context_key>: string }, session: SessionRef, session_type_id?, session_type_dir?, target_id?, worktree_id?, worktree_path?, branch?, repo? }`. `session_type_id` and `session_type_dir` are present **only for a session-type spawn**. A command spawn (`create(session, { command })`) has neither, and `context` is empty; no field is ever substituted (for example, the working directory is never reported as `session_type_dir`). `context` holds the spawn's entries by bare name (the session type's `context_keys`). `session_type_dir` is the directory of the session type's definition files (legacy scripts call it `session_dir`). The file is complete before the program starts (CLI-CX, DEP-4). It sets `TERM` and `TERMINFO` to the terminfo entry of Core's emulator (a Hub parameter), and it puts the directory of its own `botster` CLI first on `PATH`. A token that cannot be minted (no entropy) refuses the spawn with `internal { reason: credential_unavailable }`. | `conf::mc_1c_parent_token_never_reaches_child`, `conf::mc_1c_exact_botster_env`, `conf::mc_1c_context_file_private_and_complete` |
| MC-1b | **Authorization: one scope model for every caller.** Every caller (a Unix operator, a paired `webrtc` credential, or an agent session) holds a **scope set**. A verb or action is `allowed` for a caller exactly when its `permission` (DS-1) is in that set, and `describe` reports `allowed` per caller. An operator holds every scope. **An agent's scope set** is the Hub parameter `agent_scopes` (a list of permission names and `*` patterns), which the operator may restrict per session type or per session, including to named packages and kinds (for example `<package>.*`; parity CG-14). **The default is broad** (user decision, 2026-09-30: "agents should be able to do pretty much anything a client can"; HP CP-9): every client permission, including package and secret actions, which are scoped rather than operator-only. The operator narrows it. `agent_scopes` never includes a scope that no client caller has. A **plugin** tool stays bounded by the plugin's own manifest grant; an agent's scopes never widen it (confirmed with the Hub→plugin pair). | `conf::mc_1b_scope_set_decides_allowed`, `conf::mc_1b_operator_restricts_agent_scopes`, `conf::mc_1b_agent_any_session_by_default`, `conf::mc_1b_plugin_grant_bounds_agent_call` |
| MC-2 | **Hub-native tools are the client verbs** (TB-M), generated from `describe`, with no session-specific projections:
- `describe {}`; `list { kind, filter?, cursor?, limit? }` (exactly VB-1: an equality `filter` plus declared range filters, id order, one retained cut per cursor); `get { ref }`; `create { kind, args, idempotency_key?, operation_id? }`; `update { ref, patch, if_rev?, idempotency_key? }`; `delete { ref, idempotency_key? }`; `invoke { ref, action, args, idempotency_key?, operation_id?, descriptor_id? }`; and `whoami {}`.
- **Cancel (EN-3b)** is MCP's own `notifications/cancelled` for an in-flight tool call, mapped to `cancel { id }` of that request. There is no cancel tool.
- `ref` is a `SessionRef` or a `ResourceRef` (P5). Arguments, results, effects and errors are exactly the section 3 to 6 and A.9 shapes: the same kinds and actions as every other binding, including plugin kinds and contributed actions.
- Each tool result carries the verb's result as `structuredContent`.
- **The Hub registers no MCP prompts of its own** (the legacy customize and create prompts are dropped; parity CG-15). Plugins may register prompts (HP MT-1). The ring (`notify`) is the messaging plugin's, not a Hub tool. **The message tools belong to the messaging plugin** (decision 2026-09-26: the native Rust message tools are deleted). The Hub has no native messaging tool, no native client message kind and **no inbox primitive** (user ruling, 2026-09-30: "primitives should be lower level … hub should just be policy"). **One tool-naming rule for both contracts** (HP MT-1, MP-4 cite it):
- A **Hub-native** tool name contains no `__`.
- A **direct plugin tool** is exposed as `<plugin_key>__<tool>`.
- A **proxied MCP tool** (HP MP-4) is exposed as `<plugin_key>__<server_id>__<tool>`.
- **The Hub dispatches by the complete exposed name**, through the table it built when it published the name. The Hub never parses a name back into parts, and the delimiters are for readers only: they do not make names unique.
- **The Hub validates the complete exposed-name set before any publication:** a name equal to a published native, direct or proxied name fails the later-loading plugin or server with `conflict{name}`, and nothing of it is published (HP MT-2, MP-4 define the full validation).
| `conf::mc_2_verb_tools_match_envelope_verbs`, `conf::mc_2_plugin_kinds_reachable_by_verbs`, `conf::mc_2_dispatch_by_complete_name`, `conf::mc_2_exposed_name_collision_refused` |
| MC-3 | **Messaging guarantees and their layers** (user ruling, 2026-09-30):
- **At least once:** `receive` leases messages, a message that is not acked is delivered again, and `ack` is idempotent. **Provided by the messaging plugin.**
- **Safe typing of a ring into the target session:** **provided by the Hub's `safe_write`** (MC-4).
- **Unacked messages survive a `kill -9` of the Hub,** and a post succeeds only after the message is durable. **Provided by the messaging plugin, over the Hub's durable plugin storage** (Hub→plugin **ST-9**: acknowledged only after `fsync`; it survives a plugin crash, a reload and a `kill -9` of the Hub; multi-document batches are atomic; a timeout means the effect is unknown).
- **Source and target** are `SessionRef`s, and both include the hub. The **verified caller** is provided by the Hub (MC-1).

These cases run through the plugin's tools against the real Hub. The acceptance scenario s10 (the unacked message was lost after `kill -9` on Hub `6b4813a9`) is the red case. | `conf::mc_3_lease_redelivery`, `conf::mc_3_unacked_survives_kill9`, `conf::mc_3_post_success_implies_durable`, `conf::mc_3_refs_include_hub` |
| MC-4 | **Two session-write primitives** (user decision, 2026-09-30). Both are Hub primitives that plugins call under their manifest permission. They are defined in the Hub→plugin contract (HP). **HP SW-1 to SW-10 and RW-1 to RW-3 govern. The summary below is non-normative.** A change of an HP parameter or wording needs no change here.
- **`sessions.safe_write { session, text, submit? }`: the doorbell, a Hub guarantee** (HP **SW-1** call, **SW-2** outcomes, **SW-3** gate, **SW-4** probe).
  - It types text into a session with the probe and echo check, erases the probe, uses bracketed paste, waits while the cursor is hidden, and applies the Core IN-10 guard against typing over human input.
  - It **never types the requested text into a session that does not echo the probe.** The probe itself (`zx`) is written, so it does reach a session that does not echo (for example a raw sink with echo off). Without a confirmed echo, none of the requested text is typed and nothing is erased (SW-4). The completion reports `probe_written` and `residue: none | possible`.
  - **Guard token:** the terminal-state token that the plugin contract's SW-4 checks (formerly `terminal_rev`) **is Core's opaque `model_rev`** (Core ST-4 and IN-10 `Guard.model_rev`). There is one token, it is compared only for equality, and it is never ordered.
  - **Residue rule (orchestrator ruling B, 2026-09-30; identical to the plugin contract's SW-4):**
    1. **Echo not confirmed by `echo_deadline_ms` (an HP parameter):** nothing more is typed, and the attempt ends as not delivered.
    2. **Late cleanup:** an echo that arrives within `cleanup_window_ms` (an HP parameter, measured from the probe) is erased **only** if all three hold:
       - (a) there has been no client input since the probe;
       - (b) the cursor row's text before the cursor still ends with exactly the probe bytes, at the column where the probe started plus the probe width;
       - (c) the modes (alternate screen, bracketed paste) are unchanged since the probe.

       **All three guards are checked at the actual start of the erase write** by the worker, as one Core `Guard { input, model_rev }`: the client-input revision, and Core's `model_rev` read **atomically** with the row and mode checks (the plugin contract's SW-4, HP).

       Otherwise nothing is erased.
    3. **Residue is possible only in three cases:**
       - the human-input race;
       - a session that never echoes within the window (the probe bytes **did** reach the program's input);
       - a failed guard, because the program redrew.

       The completion reports `residue: none | possible`. It **never claims `none`** unless the erase was confirmed, or the echo was confirmed and erased in the normal path.
    4. **The Hub never erases what it did not verifiably write, and never erases human input.**
- **`sessions.write { session, bytes_b64 }`: raw bytes** (HP **RW-1 to RW-3**). It writes immediate, unmodified bytes, for deliberately interacting with a program. It needs the explicit, stronger scope `session_write`, and it has **no safety guarantee, by design**.

Plugins never reimplement either one. **The messaging plugin owns** when to ring and for which message (its policy), and the inbox's storage, delivery and access (MC-3). | `conf::mc_4_safe_write_no_probe_residue` (s08), `conf::mc_4_safe_write_waits_while_cursor_hidden`, `conf::mc_4_safe_write_refuses_non_echoing_session` (HP `sw_4_non_echoing_sink_receives_nothing`), `conf::mc_4_safe_write_abandons_on_human_input_without_erasing`, `conf::mc_4_late_echo_erased_within_cleanup_window`, `conf::mc_4_late_echo_not_erased_after_input_or_redraw_or_mode_change`, `conf::mc_4_never_erases_unverified_or_human_bytes`, `conf::mc_4_raw_write_needs_stronger_permission` |
| MC-5 | **Two error layers, as MCP defines them:**
- a JSON-RPC protocol error (for example an unknown tool or malformed params) is a JSON-RPC `error`;
- a tool's execution failure is `result.isError: true`, with the section 11 `Error` object in `structuredContent`. | `conf::mc_5_protocol_vs_tool_errors` |
| MC-6 | **Audit.** Every tool call, native or plugin, passes through one dispatcher and records: the time, the caller (`{hub_id, session_id}` or `operator` with its `credential_id` when it has one), the tool, the target refs, and the outcome. The Hub **never records** the token, argument bodies or message contents. Retention is bounded by the Hub parameters `audit_retention_days` (default 30) and `audit_max_bytes` (default 64 MiB). The oldest records are removed first (orchestrator ruling, 2026-09-30). | `conf::mc_6_audit_never_logs_token`, `conf::mc_6_plugin_tools_audited`, `conf::mc_6_retention_bounded` |

## 11. Errors

`Error { code, category, retryable, effect, message, reason?, field_errors?: [{ pointer, field_id?, code, message }], retry_after_ms? }`.
- **`effect`** is present on every error answering a mutating request: `none` (certainly no effect), `unknown`, or `partial`. **The shared rule** (agreed with the Hub→plugin pair): a refusal **before dispatch** is `effect: none`. Cancellation, a timeout or a crash **after dispatch** is `effect: unknown`, unless the Hub proves otherwise. A plugin handler is not one transaction: host calls it committed stay committed.
- **`retryable` means that a retry may succeed. It does NOT mean that repeating is safe.** A mutation is **safe to repeat** only when:
  - its error says `effect: none`; or
  - the repeat carries the same valid idempotency key (EN-8); or
  - the action is declared `idempotent: true` in `describe`.

  `hub_shutdown`, `plugin_failed` and lost responses do not by themselves authorize a repeat.
- **`code` is the contract.** It comes from a closed list. It is additive: a new code is a MINOR change, and clients map an unknown code to its `category`, and an unknown category to `fault`.
- `reason` is a kind-specific sub-code that is listed in the kind's `describe` (for example restart's `not_ready`). It is for display and for specific screens.
- `retryable` is authoritative. Clients never infer retryability from strings.

| Code | Category | Retryable | Meaning |
|---|---|---|---|
| `invalid_arguments` | caller | no | Schema or value error. `field_errors` names JSON Pointers. |
| `not_found` | caller | no | No such resource. |
| `unsupported` | caller | no | The kind, verb, action or feature is not offered. |
| `conflict` | state | no | Duplicate id, or an `if_rev` mismatch. |
| `wrong_state` | state | depends on `reason` | Not valid in the resource's state. |
| `permission_denied` | auth | no | The caller may not do this. |
| `too_many_requests` | capacity | yes | EN-4. |
| `busy` | capacity | yes, with `retry_after_ms` | A bounded Hub resource is full. |
| `response_too_large` | capacity | no | EN-5, BD-1. |
| `plugin_failed` | fault | no | A plugin action or render failed. `reason` carries the plugin's code. |
| `remote_hub_unsupported` | caller | no | P5. |
| `hub_shutdown` | state | yes (after reconnect) | EN-3. |
| `expired_cursor` | state | yes (restart the listing) | VB-1. |
| `stale_descriptor` | state | yes, after re-reading `describe` | DS-6. Nothing was dispatched. |
| `cancelled` | state | yes | EN-3b. No effect. |
| `internal` | fault | no | An invariant broke. |

**`partial`** comes only from a write that stopped part of the way through (a raw write or an input operation whose write was cut off). **Mapping plugin failures to client errors** (the Hub→plugin effect set is `none`, `committed`, `unknown`):

| Plugin failure | Plugin effect | Client error |
|---|---|---|
| refused before dispatch: `invalid_request` | `none` | `invalid_arguments` |
| refused before dispatch: `capability_denied` | `none` | `permission_denied` |
| refused before dispatch: `not_found` | `none` | `not_found` |
| refused before dispatch: `conflict` (other than `stale_descriptor`) | `none` | `conflict` |
| refused before dispatch: `quota_exceeded`, `backpressured`, or `timed_out` while waiting for admission | `none` | `busy` (with `retry_after_ms` when the Hub knows it) |
| refused before dispatch: `cancelled` | `none` | `cancelled` |
| refused before dispatch: `failed { wrong_state }` | `none` | `wrong_state { reason }` |
| refused before dispatch: `unavailable`, `failed` (other reasons), or a kind this table does not name | `none` | `plugin_failed { reason }` with `effect: none` |
| `cancelled` or `timed_out` after dispatch | `unknown` | `plugin_failed { reason }` with `effect: unknown` (never client `cancelled`) |
| a handler error after host calls committed | `committed` or `unknown` | `plugin_failed { reason }` with `effect: unknown` |
| `conflict { detail.code: stale_descriptor }` | `none` | `stale_descriptor` |
| `response_too_large` before dispatch | `none` | `response_too_large` with `effect: none` |
| `response_too_large` after dispatch (the handler ran; HP IV-2) | `unknown` | `response_too_large` with `effect: unknown` (never safe to repeat) |


---

## 12. Conformance and FakeHub

- **The contract crate** `botster-hub-contract` holds the envelope types, the error codes, the kind structs with derived JSON Schema, and the generated TypeScript (one copy, published as an npm package that Web consumes). It carries no Hub logic.
- **The suite is data-driven:** each clause is a **transcript**. A transcript is a script of client frames plus matchers on server frames, stored as JSON in the contract repo. There are two runners:
  - a Rust runner that drives a real Hub over **every** binding, and the Rust FakeHub;
  - a TypeScript runner that drives the TS FakeHub, so Web's tests use the same clauses.
- **FakeHub:** a deterministic in-memory Hub written **from the clauses**. It exists in Rust and in TypeScript, and it serves real bindings (TB-0). A fake behavior with no clause is not allowed. (Size guidance: small; this is not a contract obligation.)
- **Named semantic transcripts.** Each comes from the clauses it names, and **both clients run the same semantic cases** through their own transport adapter:
  - internal queue loss on a watch: the resync replaces the state (WT-4);
  - transport loss on a route: the route ends as `transport_lost`, then a re-attach gives a new route (TS-9, TS-11);
  - a plugin reload between a `describe` read and a dispatch: `stale_descriptor`, nothing dispatched (DS-6);
  - a mutation committed before its response was dropped: resolved by the idempotency key or by a read (EN-7, EN-8);
  - concurrent duplicates with one idempotency key: one effect and one shared result (EN-8);
  - a resync while a frame is partly written: the started frame is completed, `resync` comes at the next boundary, then the new baseline (TS-4c); and the variant where the started frame cannot finish: the route ends `write_failed` (TS-9);
  - an unknown verb, and an unknown component (EN-6).
- **Seeded interleaving** randomizes every order the contract leaves open:
  - response order (EN-3);
  - order across watches, events and route frames (WT-7);
  - where resync snapshots fall (WT-3);
  - the batching of frames per read;
  - the chunking of route frames (TS-3, TR-3);
  - the timing of `route_ended` against the session watch update (TS-9);
  - the moment of `hub_shutdown`.

  Client tests run under several seeds.
- **Real-Hub runs** (in the Hub repo) of the full suite on both transports, plus the acceptance suite (plan audit §3.2).
- **Hub CI:** every kind and action that a client hard-codes (the registry file) maps to at least one transcript.
- **Client CI:** each client runs the suite's client-side checks (DS-4 fallback, the TS-8/TS-9 close handling, the error-category mapping) against FakeHub.
- **Changing the contract:** one change carries, together, the type, the clause text, the transcript and the fake behavior. A new kind or action lands behind a feature name first. A reviewer other than the author signs off.

**Versioning.**
- The protocol `version` is a u16 that changes only for a breaking envelope or terminal-stream change.
- Kinds carry their own `schema_version M.m`.
- The Hub accepts `version` and `version − 1` (HS-2).
- The suite's revision is a test artifact.
- The contract crate is semver. Clients pin a released version, not a git rev.

---

## 13. Open for the user

**None.** U-7 is answered by the user's "agents are clients" decision: the agent default is broad and operator-restrictable (MC-1b, HP CP-9).

Answered on 2026-09-30, later: agents are clients (section 10, TB-M); the web plugin provides the Web origin and signalling (PR-0, PR-4); sandboxed iframe plugin HTML is allowed (7b); one Hub per machine and nothing cloud-specific (P5); repo-scoped packages are auto-discovered (package row).

The user answered these three earlier on 2026-09-30:
1. Messaging is the messaging plugin's: it owns the tools, the storage, the logic and the UI (plugin views, SF-0), over the Hub's durable plugin storage and the guarded session write. The Hub is policy only (reversal, 2026-09-30; DS-8 was removed).
2. The unsafe-paste definition is accepted (TS-5e).
3. **Terminal transparency:** every terminal feature, OSC 52 included, passes through by default with no opt-in (P9).

Resolved without the user:
- The registry family is removed, and freeform spawn and local package admission are kept (orchestrator rulings).
- **Handed to the ui-contract owner:** the view vocabulary needs from Web's review section I (`$row` and `$form` bindings, typed references with an absent state, row activation, optional form and schema UI hints, confirmation metadata, empty and error copy, master/detail selection). They belong in the ui-contract declarative view schema, never in render callbacks.
- The multi-attach size default is `Latest` (orchestrator ruling).
- The terminal layering: the Hub owns the lifecycle and input envelopes, and Core owns the payload encodings (settled with the Core pair).
- The `webrtc` binding's layout: one control channel, plus one peer connection and channel per route (TR-2, TB-W2).
- Freeform spawn: kept as `create(session, { command })` with an operator permission. Local package admission: kept as `create(package, { source: { local_path } })`, separate from the removed registry family.

## 14. Technical items still open (not for the user)

- **Conformance for the query path:** `conf::qr_active_client_answers_first`, `conf::qr_deadline_falls_back_to_shadow`, `conf::qr_no_route_goes_straight_to_shadow`, `conf::qr_decline_falls_back_to_shadow`, `conf::qr_late_client_answer_already_replied`, `conf::qr_two_routes_only_selected_answers`, `conf::qr_forged_route_reply_refused`, `conf::qr_route_disconnect_before_reply_goes_to_shadow`, `conf::qr_lost_completion_no_second_answer`, `conf::qr_client_suppresses_emulator_auto_replies`, `conf::qr_reply_is_not_latest_activity`.
- **T3 — closed:** the worker arbitrates (P9) with a host-supplied deadline. Core has no query timer. Replies arrive on the route. The client never sends `shadow`; only the worker's fallback writes the saved shadow answer.
- **T2 — structure closed by Core EV-8 (draft 23: typed kinds plus `Other`/`Encoded`). Coverage is being checked against the pinned Ghostty `eb72ec61` handlers (see the review note sent to Sol Hub A).** This contract adopts Core's `QueryKind` and `ReplyTerminalQuery` sets verbatim, so any kind Core adds flows through with no change here. Coverage (XTWINOPS queries that EV-8 draft 20 leaves unanswered) is a **release-schema blocker owned by the Core pair**, so that the user's transparency promise (P9) holds. Model queries are answered by the worker. External queries go through `TerminalQuery` and `ReplyTerminalQuery` to one route (P9, A.4 `0x0C`, A.5 `reply_terminal_query`). 

- **T1.** Resolved by the user's ruling: signalling is untrusted, pairing is by URL fragment, and DTLS is bound to the paired keys (TR-5, PR-1 to PR-8). There is no application-layer cipher.

Resolved in draft 5: MCP negotiation, Origin validation and the two error layers (MC-0, MC-5); message tools owned by the messaging plugin (MC-2, MC-3); `SessionRef` and `ResourceRef` (P5); safe handling of unknown input (EN-6); named semantic transcripts; the definition of `Latest` (TS-6).


---

## Appendix A (normative)

### A.1 JSON rules (the control stream and JSON route bodies)
- The encoding is UTF-8 JSON (RFC 8259).
- **Integers** in JSON are within ±(2^53 − 1). A u64 identifier (request `id`, `op` in JSON) is sent as a **decimal string**.
- **Absent means "not provided".** `null` is accepted only where the field's schema permits it, and then it has that field's defined meaning. A required field is never `null`. **In an RFC 7396 merge patch, `null` deletes the member and absence leaves it unchanged,** as that RFC defines.
- **Duplicate object keys**, invalid UTF-8 or invalid JSON are `protocol_error { malformed_frame }`, and the connection closes.
- **Invalid framing** closes the connection with a typed `protocol_error`:
  - `frame_too_large`: the length exceeds the bound **of that frame's type**. A control frame is bound by `welcome.limits.max_frame_bytes`. On a route, `screen` is bound by `route_limits.max_screen_frame_bytes`, `history` by `route_limits.max_history_page_bytes`, and every other frame by `route_limits.max_frame_bytes`;
  - `malformed_frame`: the body is not what the frame type requires;
  - `unknown_frame`: an unknown frame type without the ignorable bit (A.4);
  - `handshake_order`;
  - `duplicate_request_id`;
  - `invalid_input`: a route frame that cannot be parsed. **Only an unparseable frame ends a route.** An unknown enum value inside a parseable input is refused with `input_refused { reason: unsupported { what } }`, and invalid UTF-8 in a `text` frame with `input_refused { reason: unsupported { what: "utf8" } }`. Both have zero effect, and the route stays open.

  On a route, the same codes end the route (TS-9 `write_failed` for the worker's own failure; `route_closed { protocol_error { code } }` for the client's).

### A.2 Reason sets

**Every reason set in this contract is non-exhaustive.** A receiver shows an unknown reason as "other". An unknown `Lost` reason maps to `lost` (`indeterminate`), and an unknown ended cause maps to `ended`.
- **`watch_closed.reason`:** `owner_unloaded`, `owner_failed`, `kind_removed`, `permission_revoked`, `item_too_large`.
- **`close.reason`:** HS-9.
- **`route_closed.reason`:** TS-8, plus `protocol_error { code }`.
- **`route_ended.reason`:** TS-9.
- **`snapshot_begin.reason`:** absent for the first baseline, and one of `overflow`, `source_reset` or `owner_reloaded` for a resync.

### A.2b More reason sets
- **History `unavailable`:** `evicted`, `restart`, `oversize`, `capture_failed`. **`incomplete`:** `truncated`, `capture_failed`.
- **`attach_failed`:** `session_not_running`, `screen_unavailable`, `not_attachable`.

### A.2c `lifecycle_class` of a session

| `state` | Meaning | `lifecycle_class` | Input | `restart` | `delete` |
|---|---|---|---|---|---|
| `starting` (including Core `Created`), `running`, `stopping` | live | `current` | yes (on a live route) | no | no (`wrong_state`) |
| `ended` | the process is known to have ended, including a worker that was lost with a **known** termination (`failure_reason: worker_lost`; Core AD-2 `WorkerGone` and `StartInterrupted`) | `ended` | no | if `restartable` | yes |
| `lost` | adoption is **unresolved**: the worker may still be alive (Core AD-2 `WorkerUnreachable`, `WorkerVersion`, `RegistryCorrupt`) | `indeterminate` | no | no | yes: an operator cleanup, carried out as Core `Remove` (Core LC-7 and AD-2 end a stray worker only after verifying its identity) |

### A.3 Scopes
Each of these is scoped to one connection and one incarnation:
- a watch id;
- an event stream id;
- a list cursor (VB-1).

**Routes are different.** A `route_id` is minted by the Hub, unique within the `hub_id`, and **retained by the worker** across a Hub restart (BP-7). **When the worker is adopted, it retains the Hub's `route_id` and the caller-owner tag of each route, even if Core re-mints its internal route identity (Core `RouteAdopted`).** A route is **owned by its caller**: the identity its binding authenticated (a `credential_id`, or an operator peer uid). After a Hub restart, the new Hub learns the live routes from the adopted worker. A reconnecting client of **the same caller** sees them in `list(route)` (core fields `id`, `session`, `binding`, `since`) and may `delete(route, id)` (= detach). **A route's bytes never re-pass through the Hub.** An `op` is scoped per route.

Idempotency keys are scoped to a caller and an incarnation. Durable operation ids and resource ids are durable.

### A.4 Route frames (in TS-3 framing)

Hub → client:

| Type | Frame | Body |
|---|---|---|
| `0x01` | `attached` | JSON `{ features: [..], terminal_format, limits }`, where `limits` restates the attach's `route_limits` as the worker applies them. `features` is the intersection of the client's `route_features` and the worker's features. `terminal_format` is the format negotiated at attach (TS-1). `limits` are the route's **own** limits, announced by the worker that serves it. They are authoritative for that route over the Hub-wide `welcome` values, because an adopted worker may be one version older (Core AD-4). |
| `0x02` | `baseline_begin` | JSON `{ rows, cols, modes: ModeFlags }` |
| `0x03` | `screen` | the Core screen payload (opaque bytes, `terminal_format`) |
| `0x04` | `history` | one Core history payload page (opaque bytes) |
| `0x05` | `baseline_end` | JSON `{ history: "not_requested" | "complete" | { "unavailable": reason } | { "incomplete": reason } }` |
| `0x06` | `live` | empty |
| `0x07` | `output` | the Core output payload (opaque bytes) |
| `0x0E` | `output_deflate` | a raw DEFLATE stream (RFC 1951) of one `output` payload. Sent only when the route negotiated the feature `output_deflate` (TS-1 `route_features`); the worker chooses per frame. The inflated payload is at most the `output` frame bound minus the one type byte (TS-3 counts the type byte); a larger or corrupt stream is `protocol_error { invalid_input }` (parity CG-7: legacy compressed large output, which matters over TURN) |
| `0x08` | `modes` | JSON `{ modes: ModeFlags, rows, cols }` |
| `0x09` | `resync` | JSON `{ reason: "stalled" | "overflow" }` |
| `0x0A` | `input_refused` | JSON `{ op?, reason, written_bytes?, path? }` (TS-5b; `path` only for a file whose path paste failed, TS-5f) |
| `0x0D` | `input_done` | JSON `{ op, written_bytes, path? }`, sent only for a `paste_commit` with `want_result: true` (TS-5b) and for every `file_commit`, with `path` (TS-5f) |
| `0x0C` | `terminal_query` | JSON `{ query_id, request_bytes_base64, kind?, shadow_answerable, answer_within_ms }`. `request_bytes_base64` is the exact query sequence, so a client can answer any query its emulator understands with `encoded`. `kind` is present **only** for Core's typed kinds, and it uses Core's `QueryKind` names at the frozen Core contract, in snake_case. The set is **non-exhaustive**, and a client declines an unknown kind: `{ "clipboard_read": { selection } }`, `"text_area_pixels"`, `"cell_pixels"`, `"screen_pixels"`, `"window_pixels"`, `"screen_chars"`, `"window_state"`, `{ "window_position": { area } }`, `"window_title"`, `"icon_label"`, `"color_scheme"`. Model queries carry no `kind`: the client reads `request_bytes_base64`. `shadow_answerable` says whether the Core fallback has an answer. A client answers each kind from its own real terminal state, or declines; the worker then falls back to the shadow terminal (P9). `query_id` is Core's opaque id. |
| `0x0B` | `route_closed` | JSON `{ reason, exit?: { code?, signal? } }` (TS-8) |

Client → Hub:

| Type | Frame | Body |
|---|---|---|
| `0x81` | `input` | JSON, one of the shapes in A.5 |
| `0x82` | `paste_chunk` | `[u64 big-endian op][bytes]`. In every binary input frame, `op` 0 means "no op"; a paste needs a nonzero `op`. |
| `0x83` | `bytes` | `[u64 big-endian op][bytes]`: raw bytes written as they are, for IME and passthrough |
| `0x84` | `text` | `[u64 big-endian op][UTF-8 bytes]`: text written as one transaction (invalid UTF-8 is refused with `unsupported { what: "utf8" }`) |
| `0x85` | `file_chunk` | `[u64 big-endian op][bytes]`: file content (TS-5f), never written to the PTY |

**Extension rules (gate 5):**
- **Frame types:** bit `0x40` of a type marks it **ignorable**. A receiver drops an unknown ignorable frame, and an unknown frame **without** that bit is `protocol_error { unknown_frame }`. So a new worker-to-client notice uses `0x40` to `0x7F`, and a new client-to-worker hint uses `0xC0` to `0xFF`. A new frame that must be understood is sent only when the peer advertised its feature. **The worker learns the client's route features from the attach:** `attach` carries `route_features: [..]` (the route features the client understands), and the Hub passes them to the worker with the route handoff. **The client learns the worker's features from `attached.features`.**
- **Input:** an unknown input `kind` in a `0x81` frame is answered `input_refused { op?, reason: unsupported { what } }`, and the route stays open.
- **Queries:** an unknown query `kind` is answered by the client with `decline` (then the P9 fallback applies). An unknown reply variant is refused with `input_refused { reason: unsupported { what } }`.
- **Enums and modes:** an unknown enum value or mode name is shown as "other", or ignored (EN-6).

### A.5 Input shapes (`0x81`), with `op` as a decimal string

**The semantic input model is Core contract section 5.1A, as frozen in Core contract v1.0, adopted verbatim.** This contract adds no key, mouse or mode vocabulary of its own. **Field aliases are forbidden**: the wire uses exactly Core's field and value names, and the Hub converts nothing. Any settlement of 5.1A before the Core freeze (for example, the produced text and alternate keys of a `Key`, raised by Sol with the Core writer) applies here unchanged. The shapes below are a summary; **5.1A governs.**
- `{ op, kind: "key", key: { "named": <NamedKey> } | { "char": "<scalar>" }, mods: [..], event: "press" | "repeat" | "release", text?, shifted_key?, base_layout_key? }`. The produced-text and alternate-key fields are exactly as Core 5.1A settled them. `NamedKey` and the meaning of each modifier are as in Core 5.1A (lowercase: `enter`, `arrow_up`, `f1`…`f35`, the keypad keys, the modifier keys; `mods` ⊆ `shift`, `alt`, `ctrl`, `meta`, `super`, `hyper`, `caps_lock`, `num_lock`).
- `{ op, kind: "mouse", action, button, row, col, x?, y?, mods, notches? }`: Core 5.1A. `row` and `col` are zero-based cells. `x` and `y` are zero-based pixels, used only with the `sgr_pixels` encoding. `action` is `wheel` exactly when `button` is a `wheel_*` value.
- `{ op, kind: "focus", focused }`.
- Text is **not** a JSON shape: it is the raw frame `0x84` (BP-3).
- `{ op, kind: "resize", rows, cols }`.
- `{ op, kind: "reply_terminal_query", query_id, reply }`. `reply` is a the JSON projection of a Core `ReplyTerminalQuery` variant (byte fields become `*_base64`):
  - `{ "clipboard": { selection, data_base64 } }` (the request's `selection` exactly);
  - `{ "pixels": { width, height } }`;
  - `{ "chars": { rows, cols } }`;
  - `{ "window_state": { iconified } }`;
  - `{ "position": { x, y } }` (u16 xterm wire values);
  - `{ "text": { text } }` (for `window_title` and `icon_label`);
  - `{ "color_scheme": "dark" | "light" }`;
  - `{ "encoded": { bytes_base64 } }`: the complete reply bytes chosen by the client, for **any** query, including model queries with no `kind`, at most `route_limits.max_query_reply_bytes`;
    - `"decline"`: it triggers the fallback (P9 step 3); it is never a final answer. The client never sends `shadow`; only the worker's fallback writes the shadow answer.

  **Typed variants are encoded by Core. `encoded` bytes are produced by the selected client.** In both cases, raw `output` is untouched, and no automatic reply ever travels as `text` or `bytes`. **A reply is fire-and-forget too:** only a refusal is sent, as `input_refused { op?, reason }` with `reason` ∈ `already_replied`, `query_expired`, `unknown_query`, `invalid_input`, `too_large` (non-exhaustive).
- `{ op, kind: "paste_begin", total_bytes?, require_bracketed? }`, then `paste_chunk` frames, then `{ op, kind: "paste_commit", want_result? }` or `{ op, kind: "paste_abort" }`.
- `{ op, kind: "file_begin", name, mime?, total_bytes? }`, then `file_chunk` frames, then `{ op, kind: "file_commit" }` or `{ op, kind: "file_abort" }` (TS-5f). Every other shape takes an optional `op`.

Semantic input is **always available**: Core makes IN-9 mandatory in 0.1, so no feature gate is needed. The worker encodes with the modes at the start of the write.

**Input not reported by the modes:** Core `NotWritten(NotReported)` (for example a mouse event with tracking off, or a key release in legacy mode) sends **no frame**, as on a real terminal. `NotWritten(Unsupported { what })` becomes `input_refused { op?, reason: unsupported { what } }`.

### A.6 `ModeFlags`
**It is Core 5.1A's `ModeFlags`, verbatim:** `alt_screen`, `cursor_visible`, `bracketed_paste`, `application_cursor_keys`, `application_keypad`, `focus_reporting` (all bool); `kitty_flags` (a u8 bitset: 1, 2, 4, 8, 16 as the kitty protocol defines them); `mouse_tracking` ∈ `none`, `x10`, `normal`, `button_event`, `any_event`; `mouse_encoding` ∈ `x10`, `utf8`, `sgr`, `urxvt`, `sgr_pixels`; and `other_modes` (unknown modes, ignored). On the wire it is a JSON object with these fields.

### A.7 Effects of the fixed v1 actions (EN-7)

| Kind | Verb or action | Effect |
|---|---|---|
| `session` | `create` | accepted (durable; receipt `{ operation_id, session, state }`) |
| `session` | `update` (`label`, `task`) | committed |
| `session` | `delete` | committed |
| `session` | `attach`, `attach_offer`, `detach` | immediate |
| `route` | `delete` | immediate |
| `session` | `stop` | accepted (not durable) |
| `session` | `restart` | accepted (**durable**; Hub→plugin CA-14) |
| `session` | `read_screen` | read |
| `session` | `clear_attention` | committed |
| `session` | `safe_write` | accepted (not durable; the `safe_write` operation, HP SW-2) |
| `session` | `write` | immediate |
| `session_type`, `spawn_target` | `create`, `update`, `delete` | committed |
| `session_type` | `resolve`, `definition` | read |
| `spawn_target` | `validate` | read |
| `spawn_target` | `browse` | read (operator permission `spawn_target.browse`) |
| `worktree` | `create`, `delete` | accepted (durable) |
| `package` | `update` (configuration) | committed |
| `package` | `enable`, `disable`, `reload`, the entrypoint actions | accepted (not durable) |
| `package` | `create` (install) | accepted (durable) |
| `package` | `delete` (remove) | accepted (durable) |
| `package` | `preview_upgrade` | read (nothing changes) |
| `package` | `upgrade` | accepted (durable) |
| `package` | `pin`, `unpin`, `approve_grants` | committed |
| `package` | `rollback` | accepted (durable) |
| `catalog` | `create`, `delete` | committed |
| `catalog` | `refresh` | accepted (not durable) |
| `hub` | `preview_package_upgrades` (all) | read |
| `hub` | `upgrade_packages` (all) | accepted (durable) |
| `hub` | `rollback_packages` (all) | accepted (durable) |
| `package` | `open_asset` | immediate (a fresh bearer capability; allowed to any caller that may list the package's views; ui-contract UI-EM-7, HP AS-2) |
| `quarantine` | `resolve` | committed |
| `navigation` | `resolve_route` | read |
| `hub` | `check_update` | read |
| `hub` | `update` | accepted (durable; it updates the whole install: the Hub, its workers and the `botster` CLI together) |
| `hub` | `shutdown` | accepted (not durable) |
| `hub` | `restart` | accepted (not durable; live sessions survive through worker adoption, BP-7) |
| `hub` | `diagnostics` | read (operator permission `hub.diagnostics`) |
| `hub` | `swap_preflight` | read |
| `hub` | `swap` | accepted (not durable) |
| `hub` | `issue_pairing` | immediate |
| `client_credential` | `delete` | committed |
| (pairing) | credential acceptance (PR-7) | committed |

### A.8 Core field types of the fixed kinds
The section 6 core fields are normative, with these types:
- `id` and `hub_id`: string.
- `label`: string.
- `state` and `lifecycle_class`: the enums of section 6.
- `exit`: `{ code?: int, signal?: int }`.
- `failure_reason`: string or absent.
- `size`: `{ rows: int, cols: int }`.
- `session_type`: `{ id, source }`.
- `role` and `interaction`: string or absent.
- `traits`: string array.
- `restartable`: bool.

The full JSON Schemas of the fixed kinds are generated from the Hub's declarations into `botster-hub-contract/schemas/v1/<kind>.json`. **They are part of the v1.0.0 tag.** The freeze includes that generation step, and a change to them follows section 12.

### A.9 Action and create arguments and results (normative names)

These names are normative. `operation_id` and `idempotency_key` are always **request** fields (EN-1, EN-7b), never arguments. The full JSON Schemas generated at the v1.0.0 tag (A.8) must use exactly these field names. A field marked `?` is optional. "Result" is the `ok` value.

| Kind | Verb or action | Arguments | Result |
|---|---|---|---|
| `hub` | `check_update` | `{}` | `{ state, available?, action? }` |
| `hub` | `update` | `{}` | `{ operation }` |
| `hub` | `shutdown` | `{}` | `{ operation }` |
| `hub` | `restart` | `{}` | `{ operation }` |
| `hub` | `diagnostics` | `{}` | `{ report }` (one opaque value, P3: for display and bug reports, never parsed by a client; it may include internals such as the lock file, with no promise) |
| `hub` | `swap` | `{ target_worker_protocol, drop_unadoptable?: bool }` | `{ operation }`. **The Hub owns the commit** (a read followed by `shutdown` would race a launch): it (1) **stops launch admission** (a spawn or restart is refused `wrong_state { reason: swap_in_progress }`); (2) **settles every launch already accepted** before admission stopped (a `starting` session, a pending spawn or restart operation): each one either reaches a live worker or ends, before the check; (3) checks the **final live set**, which includes those settled launches, against `target_worker_protocol` (Core AD-4); (4) **a session whose worker protocol is unknown counts as unadoptable**: it never passes the check silently; (5) if any session is unadoptable (or unknown) and `drop_unadoptable` is not true, it refuses `wrong_state { reason: unadoptable_sessions, sessions: [SessionRef], unknown: [SessionRef] }`, stops nothing and reopens admission; (6) otherwise it ends exactly the unadoptable and unknown sessions (and reports each one), then shuts down leaving every other worker for adoption, and signals the handover with `close { hub_shutdown, lock_released: true }` (HS-9). A swap never ends a session silently (CLI DEP-11). **Compatibility is not adoption** (Core AD-4): a compatible worker that then fails authentication or reachability (Core AD-6, AD-2) appears as a `lost` session after the swap, attributed to adoption, not to the swap check. Tests: `conf::hub_swap_stops_admission_then_checks`, `conf::hub_swap_settles_launch_accepted_before_swap` (a spawn accepted just before `swap` whose worker starts after admission stopped is in the final set), `conf::hub_swap_unadoptable_refused_nothing_stopped`, `conf::hub_swap_unknown_protocol_counts_as_unadoptable`, `conf::hub_swap_close_carries_lock_released` |
| `hub` | `swap_preflight` | `{ target_worker_protocol }` | `{ unadoptable: [{ session: SessionRef, worker_protocol }], unknown: [{ session: SessionRef }] }`: the live sessions whose worker a Hub speaking `target_worker_protocol` could not adopt (Core AD-4: the current and the previous protocol only), and **the live sessions whose worker protocol is unknown** (for example a `Lost(WorkerUnreachable)` worker whose protocol was never learned; Core never guesses it). A client calls it before a binary swap, and before `shutdown`, to warn about sessions that would be lost (CLI DEP-10) |
| `hub` | `issue_pairing` | `{ ttl_seconds? }` | `{ url, qr_text, expires_at }` |
| `session` | `create` (session type) | `{ session_type_id, target_id?, worktree_id?, prompt?, context?: { <context_key>: string }, label? }` (`worktree_id` spawns inside an existing worktree and implies its target; a `target_id` that differs is `invalid_arguments`; parity G-27) (`idempotency_key` and `operation_id` are request fields, EN-1) | `{ operation_id, session: SessionRef, state: "starting" }` |
| `session` | `create` (command) | `{ command: { argv, cwd?, env? }, size?: { rows, cols }, target_id?, label? }` | the same receipt |
| `session` | `update` | a merge patch of `{ label?, task? }` | the session |
| `session` | `delete` | — | `{}` |
| `session` | `attach` | `{ history: "none" \| "screen" \| "full", replace_route?, answers_queries?: bool (default true), binding?, route_features?: [string] }` | `{ route_id, transport, route_limits }` |
| `session` | `attach_offer` | `{ route_id, sdp, client_fingerprint, signature }` | `{ sdp }` |
| `session` | `detach` | `{ route_id }` | `{}` |
| `session` | `stop` | `{}` | `{ operation }` |
| `session` | `restart` | `{}` | `{ operation }` |
| `session` | `read_screen` | `{ history?: bool }` | `{ text, cursor?: { row, col, visible }, model_rev, history_unavailable? }` (as HP SA-1 returns; `model_rev` as in HP and Core) |
| `session` | `clear_attention` | `{}` | `{}` |
| `session` | `safe_write` | `{ text, submit? }` | `{ operation }` (the HP SW-2 outcomes) |
| `session` | `write` | `{ bytes_b64 }` | **the HP RW-2 typed result, unchanged**: `{ state: "written", accepted_bytes, written_bytes }`, or `partial` with both counts, or `unknown` with `max_written_bytes`, and the RW-2 refusals, each with its `effect` (`none`, `committed` or `unknown`) as RW-2 maps every Core outcome |
| `session_type` | `create` / `update` | `{ id, source: "device" \| { repo: { target_id } }, label, role?, interaction?, lifecycle?, command, args?, traits?, allowed_environment_overrides?, context_keys?, port_env?, notifications?, script? }` (`script` is an optional init-script body that the Hub stores and runs as `command`; parity CG-11) (update is a merge patch of these) | the session type |
| `session_type` | `resolve` | `{ target_id }` | `{ available, diagnostics?, context_keys }` |
| `session_type` | `definition` | `{}` | the editable definition, with the create fields |
| `spawn_target` | `create` / `update` | `{ id?, label, path, kind, enabled? }` | the spawn target |
| `spawn_target` | `validate` | `{}` | `{ valid, problems?: [{ code, message }] }` |
| `spawn_target` | `browse` | `{ path }` (an absolute directory on the Hub host) | `{ entries: [{ name, path }] }` (directories only, for choosing a new target; parity CG-11) |
| `worktree` | `create` | `{ target_id, branch }` | `{ operation }` |
| `worktree` | `delete` | `{ force?: bool }` | `{ operation }`. A worktree with uncommitted changes is refused `wrong_state { reason: dirty }` unless `force` is true; one with a current session is refused `wrong_state { reason: occupied }` even with `force` |
| `package` | `create` (install) | `{ source: { kind: "local", path } \| { kind: "git", url, ref? } \| { kind: "catalog", catalog_id, entry_id }, enable?: bool, bindings?: [{ interface, provider }] }` (`bindings` chooses the provider of an ambiguous required interface, HP PD-6). **Resolution refusals** end the operation `failed` with the HP PD-6 reason: `requirement_missing { interface, range }`, `requirement_conflict { interface, range, found }` or `ambiguous_provider { interface, candidates }` (a target-scoped package is discovered, not created, U-6) | `{ operation }`; the package is `staged` until `approve_grants` (HP PM-5, PM-8) |
| `package` | `delete` | `{ purge?: bool }` | `{ operation }` |
| `package` | `preview_upgrade` | `{}` | `{ old_commit, new_commit, old_hash, new_hash, changed_files, manifest_diff, grant_diff, breaking?: [{ consumer, interface }] }` (HP PM-7 `preview`; `breaking` lists the consumers a breaking provider change would degrade, HP PD) |
| `package` | `upgrade` | `{ bindings?: [{ interface, provider }], accept_breaking?: [{ consumer, interface }] }` | `{ operation }` (HP PM-7 `update`; a breaking provider change is refused unless every affected consumer is in `accept_breaking`, and those consumers become `degraded`, HP PD) |
| `package` | `pin` | `{ ref }` | the package |
| `package` | `unpin` | `{}` | the package |
| `package` | `rollback` | `{}` | `{ operation }` (one lock-history step, HP PM-9) |
| `package` | `approve_grants` | `{ grants: [..] }` (exactly the pending grants approved) | the package |
| `catalog` | `create` | `{ url }` | the catalog |
| `catalog` | `refresh` | `{}` | `{ operation }` |
| `hub` | `preview_package_upgrades` | `{}` | `{ previews: [{ package, …the preview_upgrade result, including breaking }], ambiguous: [{ consumer, interface, candidates }] }` |
| `hub` | `upgrade_packages` | `{ bindings?: [{ consumer, interface, provider }], accept_breaking?: [{ consumer, interface }] }` | `{ operation }`. It applies HP PD-6 to the whole set as **one resolution**: every breaking change must be named in `accept_breaking` (by consumer and interface), and every ambiguous requirement must be chosen in `bindings` (by consumer and interface); otherwise the request is refused **before any operation starts**, with `conflict { reason: requirement_conflict | ambiguous_provider | requirement_missing, … }` and `effect: none`, and no package changes. A resolution that passes starts the operation; a later execution failure ends it `failed { reason }` and is reported per package (resolution is all or nothing; execution is not). Accepted consumers become `degraded { interface }` |
| `hub` | `rollback_packages` | `{}` | `{ operation }` |
| `package` | `update` | a merge patch of `{ configuration }` | the package |
| `package` | `enable`, `disable`, `reload` | `{}` | `{ operation }` |
| `package` | `start_entrypoint`, `stop_entrypoint`, `restart_entrypoint` | `{ entrypoint_id }` | `{ operation }` |
| `package` | `open_asset` | `{ asset_id } \| { name }` (a `name` resolves to the newest `asset_id` at the moment of the open) | `{ url, expires_at }` (a **bearer** capability URL for one asset on the separate assets origin, expiring after `asset_token_ttl_ms`; the client asks again to renew it; `not_found` for an asset the package does not own; HP AS-1 to AS-4) |
| `quarantine` | `resolve` | `{ decision: "release" \| "remove" }` | `{}` |
| `navigation` | `resolve_route` | `{ route_path }` | `{ target: { kind, id, select? } }` |
| `client_credential` | `delete` | — | `{}` |
| `route` | `delete` | — | `{}` (the route ends with `detached`) |
| any plugin kind | declared actions | declared by the plugin (HP manifest) | declared by the plugin |

