# Completeness sweep B1: is every command fully specified? (2026-09-30)

Auditor: read-only contracts auditor (Claude). Design review only. No file was edited. No test ran.

## Scope

Question (B): for every verb, action, kind-row action, MCP tool, route frame, control frame and CLI command, does the contract state:
1. the inputs, with types and required or optional;
2. the output, with every enum value defined;
3. the failures specific to it;
4. whether it mutates, and where it commits (effect class, durability).

Files read in full:
- HC: `hub-client-contract-v1-candidate-parity45.md` (928 lines) and `hub-client-contract-v1-amendment-1-candidate5.md`.
- Codec: `route-codec-v1-extract-rev26.md`.
- UI: `ui-contract-v1-frozen-20260930.md`, `ui-contract-v1.1-amendment-frozen-20260930.md`, `ui-contract-v1.2-embed-successor-candidate2.md`.
- CLI: `cli-spec-v1-candidate26.md` (the newest; no candidate 27 exists).
- Calibration: HP `hub-plugin-contract-v1.1.joint-J4e.md` section 24 (Appendix A), section 14, 9.13, 9.14, CA-16, SW-1, SW-2.

Severity:
- **high**: an implementer must guess behavior that is visible across the contract boundary.
- **medium**: the text is ambiguous, or a failure has no stated code, but clients only display it.
- **low**: cosmetic or a naming slip.

Owners: HC and codec = plan auditor; UI = 0017; CLI = 001b.

## Calibration: what "fully specified" means here

HP Appendix A (HP:718-780) has one row per call with seven columns: call, context, grant, arguments, result, **effect and commit point**, and **specific failures** (`kind(detail.code)`). It also states the common failures once (HP:720). HP section 14 (HP:505) gives the error object `{ kind, message, retryable, effect, detail? }`.

HC A.9 (HC:871-927) has kind, verb, arguments and result. Effects are in a separate table, A.7 (HC:809-854). **There is no failures column.** The error object of HC section 11 (HC:571) has **no `detail` field**.

## Summary

| Owner | High | Medium | Low | Total |
|---|---|---|---|---|
| HC (plan auditor) | 9 | 21 | 4 | 34 |
| Codec (plan auditor) | 1 | 5 | 4 | 10 |
| UI (0017) | 1 | 8 | 2 | 11 |
| CLI (001b) | 1 | 7 | 4 | 12 |
| **All** | **12** | **41** | **14** | **67** |

**Recommendation: yes, HC needs a per-action failures column.** Reasons:
- Of about 55 A.9 rows, only these state their specific failures: `hub swap`, `hub upgrade_packages`, `session restart` (section 6 text), `session safe_write` and `write` (by HP citation), `worktree delete`, `package create` (with Amendment 1), `hub issue_pairing` (one reason), `open_asset` (one reason), `session attach` (two reasons).
- Section 11 codes are generic. A.2 and A.2b cover watches, routes and history only. A.7 gives the effect, not failures.
- So about 40 rows leave the Hub implementer to choose codes and reasons that clients see.

The drafted column is section 5 (HC gap H-10). To keep A.9 unchanged, add it as a new table **A.9b** by amendment.

---

## 1. HC gaps (owner: plan auditor)

**H-1 (high). The error object has no slot for structured reason data, and no rule encodes enum values that carry data.**
- Evidence: HC:571 `Error { code, category, retryable, effect, message, reason?, field_errors?, retry_after_ms? }`. Yet clauses send extra fields:
  - HC:882 `wrong_state { reason: unadoptable_sessions, sessions: [SessionRef], unknown: [SessionRef] }`;
  - HC:917 `conflict { reason: requirement_conflict | ambiguous_provider | requirement_missing, … }` (with `candidates`, `range`, `found`);
  - HC:122 `conflict { operation_id_in_use }`, HC:530 `conflict{name}`;
  - Amendment 1 line 21: `invalid_arguments { field_errors }` "carrying `detail.code: path_invalid`", but the error has no `detail`.
- Also, values that carry data have no JSON rule: `failed { reason }`, `abandoned { reason }` (HC:214), `degraded { interface }` (HC:208), `wrong_state { reason }`. Only `baseline_end` gives a form (`{ "unavailable": reason }`, HC:752).
- Amendment: "**A.1b Tagged values.** In JSON, an enum value with no data is a string (`"running"`). An enum value with data is an object with one key, the value name, whose value is the data object (`{ "failed": { "reason": "fetch_failed" } }`). **Error data.** `Error` gains `detail?: object`. Any data named next to an error code in this contract (for example `sessions`, `unknown`, `candidates`, `interface`, `range`, `found`, `name`) is a member of `detail`. `reason` stays a string."

**H-2 (high). The `{ operation }` result of every accepted action is undefined, and so are operation kinds and payloads.**
- Evidence:
  - EN-7 (HC:116) and A.9 (HC:878-921) return `{ operation }`, but nothing says whether `operation` is an id, a `ResourceRef` or the full `operation` row.
  - Session create returns a different shape, `{ operation_id, session, state }` (HC:885).
  - The `operation` row (HC:214) lists `kind` without values (only `spawn` and `safe_write` are named), and `progress?`, `result?` and `error?` without types.
  - For "other kinds", the states `running`, `succeeded` and `failed` have no state before the operation starts (EN-7 says "admitted, not necessarily started"), and `failed` has no `reason`.
  - CLI-UP2, CLI-PL3 and CLI-PL7 print values (a version, a ref) that must come from `result`.
- Amendment: "`{ operation }` is the full `operation` row at admission. Add **A.7b Operation kinds**, one row per accepted action: `kind` (`hub.update`, `hub.shutdown`, `hub.restart`, `hub.swap`, `hub.upgrade_packages`, `hub.rollback_packages`, `session.stop`, `spawn` (create and restart), `safe_write`, `worktree.create`, `worktree.delete`, `package.install`, `package.remove`, `package.enable`, `package.disable`, `package.reload`, `package.entrypoint`, `package.upgrade`, `package.rollback`, `catalog.refresh`), its states, its `failed.reason` set, `progress` (absent or `{ message, fraction? }`) and `result` (for example `package.upgrade` → `{ package, version, resolved_commit }`; `package.rollback` → `{ package, ref, resolved_commit }`; `hub.update` → `{ version }`; the bulk kinds → `{ packages: [{ package, state, reason? }] }`). The states of the generic kinds are `queued`, `running`, `succeeded`, `failed { reason }`."

**H-3 (high). `hub check_update` and the hub row's `update` have no defined values.**
- Evidence: HC:877 returns `{ state, available?, action? }`. HC:203 has `update{state, available, action}`. No clause gives the `state` values, the type of `available` (a version string, or an object?) or the meaning and values of `action`. No failure is stated (offline, no update source, a dev build). CLI-UP1 and CLI-ST5 print `v<cur> -> v<new>` and three outcomes from these values. OI-5 is still open (CLI:296).
- Amendment: "`update` and the `check_update` result are `{ state: up_to_date | available | ahead | checking | installing | failed { reason } | disabled { reason: offline | dev_build }, available?: { version, channel }, action?: install | restart_required }`. `ahead` answers OI-5 (the running version is newer than the latest release). `check_update` never fails because of the network: a failed check is `state: failed { reason: check_failed }` with `effect: none`."

**H-4 (high). `session attach` failures beyond the two `unsupported` cases are undefined.**
- Evidence: TS-1 (HC:262-264) names only `unsupported { binding_auth_unavailable | terminal_format }`. A.2b `attach_failed` (HC:722) has `session_not_running` as a **route** close reason. It is open whether `attach` on an `ended` or `lost` session answers `err` or answers `ok` and then sends `route_closed { attach_failed }`. Also undefined: a `replace_route` that is unknown, owned by another caller, or on another session; an unknown `binding`; and too many routes.
- Amendment (TS-1): "Before the route is reserved, the Hub also refuses with `wrong_state { reason: session_not_running }` when the session state is `ended` or `lost`, `not_found { reason: route }` when `replace_route` does not name a live route of this caller on this session, `unsupported { reason: binding }` for a binding that is not offered, and `busy` when a bounded route resource is full. `attach_failed` is sent on the route only when the session ends or the screen fails after the reservation."

**H-5 (high). `attach_offer` has no failures.**
- Evidence: HC:890, BP-4 (HC:440). The test `bp_4_forged_route_offer_refused` has no code. Undefined cases: a bad signature or fingerprint, an unknown or expired `route_id`, a route that is not a `webrtc` route, a route already connected, a route of another caller, and a worker that cannot answer.
- Amendment: "`attach_offer` refusals: `permission_denied { reason: bad_signature }`; `not_found { reason: route }` (an unknown route, or one owned by another caller); `wrong_state { reason: route_connected | route_expired | not_webrtc_route }`; `busy` (the worker cannot answer now). Every refusal has `effect: none` and leaves the reservation as it was, except `route_expired`."

**H-6 (high). A single-package `upgrade` with a breaking change has no refusal shape.**
- Evidence: HC:909 says the change "is refused unless every affected consumer is in `accept_breaking`". It does not say whether this is a request error (with which code, and `effect: none`) or a failed operation. `upgrade_packages` defines it (HC:917). CLI-PL4 (CLI:165) prints a hint from this refusal. Also undefined: `upgrade` of a pinned package (PM-9 says the update skips it) and of a `local` source.
- Amendment: "`upgrade` resolves before any operation starts, as `upgrade_packages` does: an unaccepted breaking change, an unchosen ambiguous provider and a missing requirement are refused with `conflict { reason: breaking_not_accepted | ambiguous_provider | requirement_missing | requirement_conflict }`, `effect: none`, and `detail` (H-1) lists `consumers` or `candidates`. A pinned package is refused with `wrong_state { reason: pinned }`. A `local` source is refused with `wrong_state { reason: local_source }`."

**H-7 (high). Core-field enums with no values.**
- Evidence:
  - `worktree.state` (HC:207): no values.
  - `spawn_target.kind` (HC:206): no values. It is also a required **input** of `spawn_target create` (HC:901).
- These are core fields that hand-built screens may read (P6).
- Amendment: "`worktree.state` ∈ `creating`, `ready`, `deleting`, `failed`, `missing` (the directory is gone). `spawn_target.kind` ∈ `directory`, `git` (non-exhaustive)." (The values are the auditor's to choose; they must be listed.)

**H-8 (high). MCP tools: `whoami` has no result, and `delete` cannot carry arguments.**
- Evidence: MC-2 (HC:521):
  - `whoami {}` has no result anywhere.
  - `delete { ref, idempotency_key? }` has no `args` and no `operation_id`. But `package delete` takes `{ purge? }` and is a durable operation, and `worktree delete` takes `{ force? }` (HC:905-907). An agent therefore cannot purge, cannot force, and cannot name a durable delete operation. This breaks "the same kinds and actions as every other binding" (HC:523).
- Amendment: "`whoami {}` → `{ caller: SessionRef, scopes: [permission] }`. `delete { ref, args?, idempotency_key?, operation_id? }`."

**H-9 (high). The plugin-failure mapping is the only bridge for host primitives that clients call, and it is titled for plugins.**
- Evidence: `session safe_write` and `write` (HC:896-897) are HP primitives (SW-1, RW-2). Their admission refusals are HP kinds: `failed{wrong_state: session_ended}`, `invalid_request{text_too_long | text_empty}`, `backpressured` (HP:479, HP:765). HC section 11 maps HP kinds to client codes only for "plugin failures" (HC:602-618). A client that sends `safe_write` to an ended session therefore gets a code that no HC clause names.
- Amendment (section 11): "The same table maps the refusals of the Hub primitives that a client reaches through `session safe_write` and `write`: `failed{wrong_state: r}` → `wrong_state { reason: r }`; `invalid_request{r}` → `invalid_arguments { reason: r }`; `backpressured` → `busy`; all with `effect: none`."

**H-10 (medium). A.9 has no failures column.** See the recommendation above. The draft table is section 5. Each row there that is not marked "covered" is a gap. They are grouped as H-11 to H-25 below.

**H-11 (medium). Referenced ids in arguments have no rule.** Examples: `session_type_id`, `target_id`, `worktree_id`, `catalog_id`, `entry_id`, `entrypoint_id`, the `route_id` of `detach`, and the `ref` of `pin`. Section 11 `not_found` says only "No such resource" (HC:586). Amendment: "A missing **target** is `not_found`. A missing resource **named in the arguments** is `invalid_arguments` with a `field_errors` entry whose `code` is `not_found` and whose `pointer` names the argument."

**H-12 (medium). VB-5 `delete(kind, target)` (HC:167) takes no arguments.** But A.9 gives `package delete { purge? }` and `worktree delete { force? }`. Amendment: "`delete(kind, target, args?)`: `args` as the kind declares in A.9."

**H-13 (medium). The DS-1 `reasons: [..]` (HC:147) element type is undefined.** UI-AC-5 shows "the Hub's reason". Amendment: "`reasons: [{ code, message }]`, where `code` is a section 11 code or a kind `reason`."

**H-14 (medium). The answer to `watch(event, …)` is not stated.** WT-1 answers `{ watch_id }` (HC:177). Event frames carry `stream_id` (HC:378). Amendment: "`watch(event, …)` answers `{ stream_id }`."

**H-15 (medium). `push_subscription` create and delete are missing from A.7 and A.9.** They appear only in SF-5b (HC:355) and the kind row (HC:216). Amendment: A.9 row `push_subscription create { endpoint, keys: { p256dh, auth } }` → the row without the endpoint; A.7 `committed`. Failures: `invalid_arguments { reason: endpoint_not_https }`; `conflict` (an endpoint already registered). Delete of another credential's subscription is `not_found`.

**H-16 (medium). The section 6 `spawn_target` row (HC:206) omits `browse`.** `browse` is in A.7 (HC:827), A.9 (HC:903) and KR-1. Amendment: add "invoke `validate`, `browse` (operator permission `spawn_target.browse`)" to the row.

**H-17 (medium). The `hub shutdown` and `restart` operations can never be seen completing.** They return a non-durable `{ operation }` (HC:879-880, HC:847-848), and the Hub then exits. Undefined: whether a client may wait on the operation, and what ends it. Amendment: "The operation of `shutdown` and `restart` is ended by `close { hub_shutdown, lock_released }` (HS-9). A client never waits for its terminal state."

**H-18 (medium). No rule for concurrent Hub-wide operations.** Undefined: `update`, `swap`, `restart`, `shutdown`, `upgrade_packages` or `rollback_packages` while another one runs, and a second `swap`. Only spawns get `swap_in_progress` (HC:882). Amendment: "While `swap`, `update` or a bulk package operation runs, another one is refused with `wrong_state { reason: swap_in_progress | update_in_progress | package_operation_in_progress }`, with `effect: none`."

**H-19 (medium). `quarantine.reason` (HC:210) has no values.** Amendment: list the values (for example `crash_loop`, `hash_mismatch`, `admission_failed`, `bootstrap_failed`), non-exhaustive.

**H-20 (medium). `session.failure_reason` (HC:204, A.8 HC:862) is a string, and only `worker_lost` is named.** Amendment: list the values (for example `spawn_failed`, `worker_lost`, `exec_failed`, `cwd_missing`), non-exhaustive.

**H-21 (medium). Session-type input fields are untyped.** A.9 `session_type create` (HC:898): `command` (a string or argv?), `args?`, `lifecycle?` (no values) and `interaction?` (no values; HP uses `interactive | noninteractive`, HP:743). Amendment: give each its type and values.

**H-22 (medium). Package actions with no state refusals:**
- `enable`, `disable`, `reload` (HC:920): the package is already in that state, `staged`, `quarantined` or `errored`;
- entrypoint actions (HC:921): an unknown entrypoint, or a disabled package;
- `pin` (HC:910): an unknown ref, or a `local` source (PM-9);
- `unpin` (HC:911): the package is not pinned;
- `rollback` (HC:912): no history, or a `local` source;
- `approve_grants` (HC:913): grants that are not the pending set, or nothing pending;
- `preview_upgrade` (HC:908): a `local` source, or a failed fetch (this read fetches);
- `delete` (HC:907): another package requires its interface.

CLI-PL9 already assumes that "already enabled" is a success. See section 5 for the proposed codes.

**H-23 (medium). Catalog actions with no failures:**
- `catalog create { url }` (HC:914): undefined whether it fetches. It is `committed`, so presumably not. Also undefined: a URL that is not https, or a duplicate URL.
- `refresh` (HC:915): no operation failure reasons.
- `delete`: no rule for packages that came from the catalog.

**H-24 (medium). Session actions with no state refusals:**
- `stop`: on a session that is `ended`, `lost` or already `stopping`;
- `read_screen`: on a session that is `ended` or `lost` (HP gives `failed(wrong_state)`, HP:742);
- `detach`: an unknown route, or a route of another caller;
- `session create`: a session type unavailable for the target (`resolve.available: false`), and the `spawn` operation's `failed.reason` set (neither HC nor HP CA-16 lists it, HP:264);
- `worktree create`: a target that is not git, or a branch already checked out.

**H-25 (medium). Other actions with no failures:**
- `open_asset` (HC:922): only a foreign asset (`not_found`) is stated. Undefined: a disabled, quarantined or removed package (UI 1.2 line 37 and HP AS-4 imply `not_found`); a staged name that the new version no longer lists; no `assets_listen` configured; both or neither of `asset_id` and `name`.
- `spawn_target browse` (HC:903): a path that is missing, not absolute, not a directory, or unreadable.
- `spawn_target create` and `update`: a path that does not exist; a duplicate.
- `issue_pairing`: `ttl_seconds` bounds.

**H-26 (medium). `hub swap` and `swap_preflight` with an unknown `target_worker_protocol`.** HC:882-883 give no type and no refusal. Amendment: "`target_worker_protocol` is a u16. A value that this Hub cannot reason about is `invalid_arguments { reason: unknown_worker_protocol }`."

**H-27 (medium). `navigation resolve_route { route_path }` (HC:924).** The format of `route_path` is undefined, and so is the failure for an unknown path. Amendment: "`route_path` is the path part of a client URL, `/<package>/<view>`. An unknown path is `not_found`."

**H-28 (medium). `client_credential delete` of the caller's own credential (HC:925, PR-8).** Undefined whether the `res` is sent before the connection closes. Amendment: "The Hub sends `res { ok: {} }`, then closes the connection with `close { auth_failed }`."

**H-29 (medium). `hub rollback_packages` (HC:918) has no failures and no per-package outcome.** Amendment: as `upgrade_packages`. A request with nothing to roll back is `wrong_state { reason: no_history }`, with `effect: none`. Per-package outcomes go in the operation `result` (H-2).

**H-30 (medium). The `session update` merge patch on `ended` or `lost` sessions** (HC:887). It is not stated whether this is allowed. Low impact. Amendment: one sentence ("allowed in every state").

**H-31 (low). `read_screen.history_unavailable?` (HC:894), `session_type resolve.diagnostics?` (HC:899) and `validate.problems[].code` (HC:902) are untyped.**

**H-32 (low). VB-1 `limit` has no default and no maximum** (HC:163).

**H-33 (low). `plugin_log.level` (HC:213) has no values.** HP `log.write` uses `debug | info | warn | error` (HP:724). Cite it.

**H-34 (low). HC row field names differ from the HP row schemas.** `spawn_target.path` versus HP `root`; `worktree.state` versus HP `status` (HP:794-795). Align the names, or say they differ by design.

---

## 2. Route codec gaps (owner: plan auditor)

**R-1 (high). The JSON form of route reasons that carry data is undefined.** Evidence: A.4 `route_closed { reason, exit? }` (codec:241) with the reasons `session_ended { exit }`, `attach_failed { reason }` and `protocol_error { code }` (TS-8 codec:151, A.2 codec:206); `input_refused.reason` `unsupported { what }` (codec:105). Only `baseline_end` is given a form (codec:232). Amendment: apply the H-1 tagged-value rule to every route JSON body, with the example `{ "reason": { "unsupported": { "what": "utf8" } } }`.

**R-2 (medium). The location of `exit` in `route_closed` is inconsistent.** TS-8 writes `session_ended { exit { code, signal } }` (codec:151): exit inside the reason, both fields present. A.4 writes `{ reason, exit?: { code?, signal? } }` (codec:241): exit beside the reason, both optional. Amendment: "`route_closed` is `{ reason, exit? }`. `exit` is present only for `session_ended`."

**R-3 (medium). `invalid_input` has two meanings.**
- A.1 (codec:197): a `protocol_error` code that **ends** the route.
- P9 binding (codec:70) and A.5 (codec:278): an `input_refused` reason that **keeps** the route open.

A receiver cannot tell them apart by name. Amendment: rename the input reason `wrong_route` (a reply from a route that the query was not bound to). Keep `unknown_query` for the rest.

**R-4 (medium). `not_writable` ("a view-only route", codec:102) cannot arise.** No attach argument makes a route view-only, and no read-only caller exists in v1 (HS-7). Amendment: either add `attach { writable?: bool (default true) }`, or mark the reason "reserved for a future read-only caller".

**R-5 (medium). Paste and file sequencing errors have no stated reason.** Only one case is stated: a second `file_begin` gets `refused` (codec:127). Undefined cases:
- `paste_chunk` or `file_chunk` without a begin;
- a chunk whose `op` differs from the open operation's;
- a second `paste_begin` while a paste is open;
- a commit or abort with no open operation;
- a missing or zero `op` on a paste or file frame. Is it `malformed_frame` (the route ends) or `input_refused`?

Amendment: "Each of these is `input_refused { op?, reason: refused }` with zero effect, and the route stays open. A JSON paste or file frame without `op` is `protocol_error { malformed_frame }`."

**R-6 (medium). Is history sent again in a resync baseline?** TS-4c (codec:97) repeats `baseline_begin` … `baseline_end`. It does not say whether `history` frames are sent again under the attach's `history` choice, or what `baseline_end.history` reports. Amendment: "A resync baseline sends `screen` only. Its `baseline_end.history` is `not_requested`."

**R-7 (low). The type of `ModeFlags.other_modes` (codec:287) is undefined.** Amendment: "`other_modes`: an object from mode name to bool."

**R-8 (low). The meanings of `resync.reason` (`stalled` versus `overflow`, codec:237) are not given.** The relation of `terminal_query.answer_within_ms` (codec:240) to `route_limits.query_client_deadline_ms` is also not given.

**R-9 (low). The meaning of `total_bytes?` on `paste_begin` and `file_begin` is not given.** Undefined: an early refusal when it is over the limit, and a mismatch with the bytes actually sent. The bounds of `file_begin.name` are also not given (empty, length).

**R-10 (low). The subject of the TS-9 `route.ended` event is `{ route_id }`** (codec:152). EV-1 does not state the subject type of a Hub event. Say `subject: ResourceRef { kind: route }`, or keep `{ route_id }` and state it in EV-1.

---

## 3. UI gaps (owner: 0017)

**U-1 (high). The action result shape conflicts with the HC envelope, and its success fields have no location.** UI-AC-1 (UI base:191) says that the result is `{ ok, field_errors?, message? }`, keyed by node id. HC returns `res { ok: value }` or `res { err: Error }`, and `field_errors` is in `Error` (HC SF-2, HC:341, HC:571). UI-AC-6 (UI 1.2 line 88) adds `navigate` to "the result". A plugin author and a client must agree on where `message` and `navigate` live. Amendment: "On success, the `ok` value of a `ui_action` may contain `message?: string` and `navigate?: {…}` at its top level; other members are ignored by the client. On failure, the client reads `Error.field_errors` and `Error.message`. Node-id keying is done by the client (UI-ID-4), not on the wire."

**U-2 (medium). Actions whose effect is `accepted` have no rendering rule.** The client gets `{ operation }` and nothing says what to show: pending, progress, a failed operation, or `unknown`. Amendment: "For an `accepted` action, the client shows a pending state, watches the operation, and shows its terminal state. `failed { reason }` is shown like a form-level error."

**U-3 (medium). No failure display outside a form.** Undefined:
- where the error of a directly-run action (UI-AC-4, no form) or a row action is shown;
- what the client does on `stale_descriptor` (re-read `describe`, then ask the user again; never resend on its own?);
- what the client does on `effect: unknown` (never auto-retry).

Amendment: one clause, "UI-AC-7 failure display".

**U-4 (medium). `order` comparison is undefined.** UI-RZ-4 (UI base:239) and `Source.order`: the compare function is not given for strings (bytewise or locale), for numbers against strings, or for absent values. Rust and Web `realize` must give the same tree (UI-BD-5). Amendment: "Absent sorts first. Then by JSON type: null, bool, number, string, other. Strings compare bytewise in UTF-8. Ties fall back to the row key."

**U-5 (medium). `Field.map` lookup for values that are not strings is undefined.** `map?: { <value>: … }` "looked up by exact value" (UI base:131). JSON object keys are strings, so it is unclear how `true`, `1` or `null` match. Amendment: "The key is the JSON text of the value for non-strings (`true`, `1`, `null`). A string matches its own text."

**U-6 (medium). Part properties are named but have no type.** In the part table (UI base:60-72): `form.submit`, `form.cancel`, `actions.overflow` (`auto|never`, as for toolbar?), the `list.item` structure (`{ fields, actions }`), the `detail.source` `{ kind, id }` (a literal id, or an Operand?), and the parent property `alternatives: true` (UI-PT-3, UI base:80). Amendment: add a grammar block for these.

**U-7 (medium). `disabled_reason` is used but not declared.** UI-RC-5 (UI base:146) uses "the action's optional `disabled_reason` copy", but the `Action` grammar (UI base:182-187) has no such field. Amendment: add `disabled_reason?: string` to `Action`.

**U-8 (medium). The payload shapes of the embed channel are undefined** (UI 1.2 lines 16-20, 53):
- the shape of the `to_page` `rows` (are object keys the `fields` paths, or nested objects?);
- the payload shape of a `from_page` `select` ("names a row key": a string or `{ row_key }`?);
- the meaning of `from_page[].args?`;
- whether the page receives any result or failure (state "none").

A page author and the client must agree on these.

**U-9 (medium). `open_asset` failures in the embed are not mapped.** UI-EM-7 (UI 1.2 line 44) maps only `not_found` to the `error` state. Undefined: `permission_denied`, `busy`, `unsupported` (no assets origin configured) and `hub_shutdown`. Amendment: "Any `open_asset` error shows `error` with a reload action. `busy` and `hub_shutdown` may be retried on the reload."

**U-10 (low). A `status_badge` value that is not in its `map` has no stated display.** Amendment: "the raw value as text, neutral tone".

**U-11 (low). `activate: { invoke }` on a row is not tied to the button rules.** State that it follows the button rules exactly (UI-AC-3, UI-AC-4: form, then confirm). An absent `default_from` operand gives no default: state it.

---

## 4. CLI gaps (owner: 001b)

**C-1 (high). The output and exit of `update`, `update --check` and the boot notice depend on undefined Hub values, and `update` across the Hub's restart is undefined.** Evidence: CLI-UP1 (CLI:135), CLI-UP2 (CLI:136), CLI-ST5 (CLI:80).
- The CLI does not map the `check_update` result (H-3) to its three outcomes. It does not name the sources of `<cur>` and `<new>`, and it gives no progress-line format.
- CLI-UP2 "watches the operation", but the Hub "restarts itself" during `update`: the control connection ends with `close { hub_shutdown }`. Undefined: the reconnect rule (wait for the readiness of the new Hub? how long?), the use of `get(operation, id)` after the reconnect (it is durable, EN-7b), and the meaning of `busy` during recovery.

Amendment (CLI-UP2): "After `close { hub_shutdown }`, the CLI reconnects through the CLI-RT4 probe until the Hub answers or 120 s pass. It then calls `get(operation, <id>)`, retrying `busy`. `succeeded` prints the success line. `failed` prints the reason and exits 1. A timeout prints `update outcome unknown; run 'botster status'` and exits 1."

**C-2 (medium). Exit code 130 is not in the exit-code rule.** CLI-PL2 (CLI:163) exits 130 on SIGINT. CLI-G2 (CLI:56) allows only 0, 1 and 2, and CLI-G5 says "or exits 1". Amendment: add 130 ("interrupted; the Hub operation continues") to CLI-G2.

**C-3 (medium). stdout formats are missing for these commands:**
- `plugin pin` and `pin --unpin`;
- `plugin update <name>` (success);
- `plugin remove` (success);
- `plugin approve` (success);
- `plugin catalog add | remove | list | refresh` (the list columns);
- `--no-wait` (the operation id line);
- `plugin status --json` (the schema);
- `get-connection-url --qr` (order and separator).

CLI-P3 and CLI-P7 make stdout a contract. Amendment: one line per command in section 9b.

**C-4 (medium). Flags and commands are missing from the synopsis.** The §2 table (CLI:36) lists `start [--headless] [--offline]` without `--swap` and `--drop-unadoptable` (CLI-ST6). Also:
- whether `attach` takes `--swap` (CLI-ST6 says that `attach` refuses);
- `approve -y` (CLI-PL11);
- `debug hub` (CLI-DB3) is not in §2;
- the meaning of `debug memory --hub` (CLI-DB1);
- CLI-PL1 (CLI:162) lists eight subcommands, without `approve` and `catalog`;
- which commands take `--no-wait`.

Amendment: a full synopsis with each flag's type and default.

**C-5 (medium). CLI-PL3 prints HP failure names that never reach a client.** CLI-PL3 (CLI:164) prints "`hash_mismatch`, `errored`, `invalid_request` and `capability_denied`". On the client wire these arrive as `invalid_arguments`, `permission_denied`, or an operation `failed { reason: hash_mismatch }` (HC:606-613). Amendment: name the HC codes, and print `failed.reason` from the operation.

**C-6 (medium). `<name>` is not mapped to a package key.** Target-scoped packages have the key `<name>@<target_id>`, one per target (HC:208, U-6). Undefined: what `plugin status foo`, `enable foo` and the others do when `foo` has several instances. Amendment: "`<name>` matches a key exactly. A bare name with several instances is refused as ambiguous, with the keys listed (exit 1)."

**C-7 (medium). "Already enabled" depends on undefined Hub behavior.** CLI-PL9 (CLI:170) prints `<name> is already enabled` with exit 0, but HC defines no answer for `enable` on an enabled package (H-22). Amendment: "The CLI reads the row first. If `state` is already the target state, it prints the line and sends nothing."

**C-8 (medium). The source of `<ref>` and `<version>` in result lines is not named.** `Rolled back <name> to <ref>` (CLI-PL7, CLI:168) and `Installed <name> <version>` (CLI-PL3) need a source: the operation `result` (H-2) or a `get(package)` after the operation. Amendment: name the source.

**C-9 (low). `STATUS` lists `available` as a package status.** CLI-PL5 (CLI:166) does this, but `available` is a catalog-entry status (HC:209; PM-10 "in a catalog, not installed"). Remove it from the package table, or say it appears only in `catalog list`.

**C-10 (low). CLI-WT2 (CLI:149) writes "`invoke`/`delete(worktree, <id>, { force: true })`".** Write `delete(worktree, <id>, { force: true })` only (see H-12).

**C-11 (low). The output format of the swap refusal is not given.** CLI-ST6 prints "two separate lists" with no line format, and the exit code (1) is only implied.

**C-12 (low). Common transport failures have no shared message.** No Hub (for commands other than `attach` and `debug`), `hub_shutdown` during a command, `busy`, and `version_unsupported` all fall under the generic `Error: <message>` with exit 1 (CLI-G2). That is acceptable. Name the no-Hub message once for all Hub commands.

---

## 5. Draft: HC A.9b, the specific failures of each action (amendment text for H-10)

**Preamble (normative).** Every request may also fail with the **common failures**: `invalid_arguments` (schema), `permission_denied`, `unsupported`, `not_found` (the target), `remote_hub_unsupported`, `too_many_requests`, `busy`, `response_too_large`, `hub_shutdown`, `cancelled`, `internal`, and, for a plugin kind or a contributed action, `stale_descriptor` and `plugin_failed`. A refusal before dispatch has `effect: none` (section 11). A missing resource named in the arguments is `invalid_arguments` with `field_errors[].code: not_found` (H-11). `reason` and `detail` follow H-1. "covered" means that an existing clause already states it. Rows marked *proposed* are new text.

| Kind | Action | Specific failures (code { reason }) | Effect (A.7) |
|---|---|---|---|
| `hub` | `check_update` | none; a failed check is `state: failed { reason: check_failed }` (*proposed*, H-3) | read |
| `hub` | `update` | `wrong_state { up_to_date \| update_in_progress \| swap_in_progress \| offline }`; operation `failed { download_failed \| verify_failed \| install_failed }` (*proposed*) | accepted, durable |
| `hub` | `shutdown`, `restart` | `wrong_state { swap_in_progress \| update_in_progress }` (*proposed*); the operation ends with `close { hub_shutdown }` (H-17) | accepted, not durable |
| `hub` | `diagnostics` | common only (covered) | read |
| `hub` | `swap_preflight` | `invalid_arguments { unknown_worker_protocol }` (*proposed*, H-26) | read |
| `hub` | `swap` | covered: `wrong_state { unadoptable_sessions }` + `detail { sessions, unknown }`; *proposed*: `wrong_state { swap_in_progress \| update_in_progress }`, `invalid_arguments { unknown_worker_protocol }` | accepted, not durable |
| `hub` | `issue_pairing` | covered: `wrong_state { no_web_origin }`; *proposed*: `invalid_arguments { ttl_out_of_range }` (state the bounds) | immediate |
| `hub` | `preview_package_upgrades` | common only (covered) | read |
| `hub` | `upgrade_packages` | covered: `conflict { requirement_conflict \| ambiguous_provider \| requirement_missing }`, `effect: none`; *proposed*: `conflict { breaking_not_accepted }`, `wrong_state { package_operation_in_progress }` | accepted, durable |
| `hub` | `rollback_packages` | `wrong_state { no_history \| package_operation_in_progress }` (*proposed*); per-package outcomes in `result` | accepted, durable |
| `session` | `create` (type) | `wrong_state { swap_in_progress }` (covered); `internal { credential_unavailable }` (covered, MC-1c); `invalid_arguments { target_mismatch }` (covered, G-27); *proposed*: `wrong_state { session_type_unavailable }`; `spawn` operation `failed { exec_failed \| cwd_missing \| worktree_failed \| worker_failed }` | accepted, durable |
| `session` | `create` (command) | `permission_denied` (`session.create_command`, covered); *proposed*: `invalid_arguments { argv_empty \| cwd_not_allowed }` (as HP:743); `wrong_state { swap_in_progress }` | accepted, durable |
| `session` | `update` | `conflict` (`if_rev`), `invalid_arguments` (identity fields) (covered, VB-4) | committed |
| `session` | `delete` | `wrong_state` for `starting`, `running`, `stopping` (covered, A.2c) | committed |
| `session` | `attach` | covered: `unsupported { binding_auth_unavailable \| terminal_format }`; *proposed* (H-4): `wrong_state { session_not_running }`, `not_found { route }`, `unsupported { binding }`, `busy` | immediate |
| `session` | `attach_offer` | *proposed* (H-5): `permission_denied { bad_signature }`, `not_found { route }`, `wrong_state { route_connected \| route_expired \| not_webrtc_route }`, `busy` | immediate |
| `session` | `detach` | `not_found { route }` for an unknown route or another caller's route (*proposed*) | immediate |
| `session` | `stop` | `wrong_state { not_running }` for `ended` and `lost`; `stopping` returns the existing operation (*proposed*) | accepted, not durable |
| `session` | `restart` | covered: `wrong_state { not_ended \| not_restartable \| record_unavailable \| environment_not_retained \| not_ready }`, `wrong_state { swap_in_progress }` | accepted, durable |
| `session` | `read_screen` | `wrong_state { session_ended \| session_lost }` (*proposed*, as HP:742) | read |
| `session` | `clear_attention` | none: idempotent, `ok` with no attention set (*proposed* sentence) | committed |
| `session` | `safe_write` | covered by HP SW-1 through the H-9 mapping: `wrong_state { session_ended }`, `invalid_arguments { text_too_long \| text_empty }`; outcomes by operation (SW-2) | accepted, not durable |
| `session` | `write` | covered by HP RW-2 through the H-9 mapping: `wrong_state`, `busy` (lane full), `invalid_arguments { bytes_too_long \| bytes_empty \| bytes_invalid }` | immediate |
| `session_type` | `create`, `update` | `conflict` (id exists, create); `invalid_arguments { port_env_invalid }` (MC-1c says "refused" without a code); `wrong_state { not_editable }` (update) (*proposed*) | committed |
| `session_type` | `delete` | `wrong_state { not_editable }` (*proposed*); current sessions keep running (state it) | committed |
| `session_type` | `resolve`, `definition` | `invalid_arguments` with `field_errors.code: not_found` (`target_id`) | read |
| `spawn_target` | `create`, `update` | `invalid_arguments { path_not_absolute \| path_not_found \| not_a_directory }`; `conflict { path_exists }` (*proposed*) | committed |
| `spawn_target` | `delete` | `wrong_state { in_use }` (worktrees or current sessions) (*proposed*) | committed |
| `spawn_target` | `validate` | none: problems are in the result (covered); list the `problems[].code` values | read |
| `spawn_target` | `browse` | `invalid_arguments { path_not_absolute }`; `not_found { path }`; `wrong_state { not_a_directory \| unreadable }` (*proposed*) | read |
| `worktree` | `create` | `wrong_state { not_git }`; `conflict { branch_checked_out }`; operation `failed { git_failed }` (*proposed*) | accepted, durable |
| `worktree` | `delete` | covered: `wrong_state { dirty }` (unless `force`), `wrong_state { occupied }` | accepted, durable |
| `package` | `create` (install) | covered: operation `failed` with HP PD-6, PK-3 and PKC-1 reasons; `invalid_arguments { path_invalid }`; *proposed*: `conflict { already_installed }`; operation `failed { fetch_failed \| url_refused \| too_large \| hash_mismatch }` | accepted, durable |
| `package` | `delete` (remove) | `wrong_state { required_by }` + `detail { consumers }` (*proposed*; or state that consumers become `degraded`) | accepted, durable |
| `package` | `update` (configuration) | `invalid_arguments` (schema, `field_errors`) (covered) | committed |
| `package` | `enable` | `wrong_state { staged \| quarantined \| errored }`; already enabled: `ok` with no operation change (*proposed*) | accepted, not durable |
| `package` | `disable` | already disabled: `ok` (*proposed*) | accepted, not durable |
| `package` | `reload` | `wrong_state { disabled }` (*proposed*) | accepted, not durable |
| `package` | `start_entrypoint`, `stop_entrypoint`, `restart_entrypoint` | `invalid_arguments` `field_errors.code: not_found` (`entrypoint_id`); `wrong_state { disabled }` (*proposed*) | accepted, not durable |
| `package` | `preview_upgrade` | `wrong_state { local_source }`; `wrong_state { fetch_failed }` (retryable) (*proposed*) | read |
| `package` | `upgrade` | *proposed* (H-6): `conflict { breaking_not_accepted \| ambiguous_provider \| requirement_missing \| requirement_conflict }`, `effect: none`; `wrong_state { pinned \| local_source \| up_to_date \| package_operation_in_progress }`; operation `failed { fetch_failed \| hash_mismatch \| activation_failed }` | accepted, durable |
| `package` | `pin` | `invalid_arguments` `field_errors.code: not_found` (`ref`); `wrong_state { local_source }` (*proposed*; PM-9) | committed |
| `package` | `unpin` | not pinned: `ok`, idempotent (*proposed*) | committed |
| `package` | `rollback` | `wrong_state { no_history \| local_source }`; operation `failed { hash_mismatch }`; ends `staged` when PM-9 needs a new approval (*proposed*) | accepted, durable |
| `package` | `approve_grants` | `conflict { grants_mismatch }` (the set is not exactly the pending set); `wrong_state { nothing_pending }` (*proposed*) | committed |
| `package` | `open_asset` | covered: `not_found` (foreign asset); *proposed*: `not_found` also for a disabled, quarantined or removed package and for a staged name the new version dropped (AS-4); `wrong_state { assets_origin_unconfigured }`; `invalid_arguments` (both or neither of `asset_id`, `name`) | immediate |
| `catalog` | `create` | `invalid_arguments { url_not_https }`; `conflict` (URL already added); does not fetch (state it) (*proposed*) | committed |
| `catalog` | `delete` | common only; installed packages keep their lock (state it) | committed |
| `catalog` | `refresh` | operation `failed { fetch_failed \| url_refused \| malformed \| too_large }` (*proposed*) | accepted, not durable |
| `quarantine` | `resolve` | `release` while the cause still holds: the package is quarantined again and the action answers `ok` (*proposed* sentence) | committed |
| `navigation` | `resolve_route` | `not_found` (an unknown `route_path`) (*proposed*, H-27) | read |
| `client_credential` | `delete` | own credential: `ok`, then `close { auth_failed }` (*proposed*, H-28) | committed |
| `route` | `delete` | `not_found` for an unknown route or another caller's route (covered by A.3 "limited to the owning caller"; state the code) | immediate |
| `push_subscription` | `create`, `delete` | *proposed* (H-15): `invalid_arguments { endpoint_not_https }`, `conflict`; delete of another credential's subscription: `not_found` | committed |
| any plugin kind | declared actions | the declared `reasons`; plugin failures by the section 11 mapping (covered) | as declared |

---

## 6. Verdict per question

1. **Inputs typed.** HC: mostly. Exceptions: `spawn_target.kind`, the session-type `command`, `lifecycle` and `interaction`, `route_path`, `target_worker_protocol`, MCP `delete` args. Codec: yes, except R-9. UI: part properties in U-6, embed payloads in U-8. CLI: flags in C-4.
2. **Outputs with every enum defined.** Defined: `session.state`, `activity`, `presentation`, `lifecycle_class`; `package.state` and `status`; `hub.state`; `route_closed.reason`; `input_refused.reason`; `baseline_end.history`; `close.reason`; `watch_closed.reason`; the notice payload (SF-5); `event_gap`; `session_notification`. **Not defined:** `check_update.state` and `hub.update` (H-3); `operation.kind`, `progress`, `result`, and the states of generic kinds (H-2); `worktree.state` and `spawn_target.kind` (H-7); `quarantine.reason` (H-19); `failure_reason` (H-20); `plugin_log.level` (H-33); the DS-1 `reasons` (H-13); `whoami` (H-8). The JSON form of every enum that carries data (H-1, R-1).
3. **Specific failures.** HC: about 40 of about 55 actions lack them (section 5). Codec: sequencing (R-5). UI: U-2, U-3, U-9. CLI: mostly present; C-5 uses the wrong codes.
4. **Effect and commit point.** HC A.7 is complete for the listed actions, except `push_subscription` (H-15) and a commit point for each accepted action. EN-7 gives the effect classes, and `durable_operation` is stated per action. Codec: input effects are fully stated (TS-5b, TS-5c). CLI: mutations go through the Hub. No gap beyond C-1.
