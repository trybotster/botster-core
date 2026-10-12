# UI contract v1.1 amendment: parity additions (FROZEN 2026-09-30, from candidate 4)

Frozen: Sol Hub A ACCEPT DESIGN, Web and TUI confirmed candidate 3 (changes in 4 are wording and the removed claim), HP (0016) and client (plan auditor) confirmed the label rule. Accepted candidate4 sha256 `02cd8476b3791630f32f21af8a48558b30985f902b008bd7bf9d7dbec1035ae7`. This file differs only in this heading and paragraph.

Author: plugin kit writer (Claude, sess-1790726020-0017), 2026-09-30. Reviewers: Sol Hub A (001f); consumers Web (001a), TUI (001b). Gate owner: plan auditor (003c). Design only.
Supersedes candidate 3 (`92e1bcba…ff8e`; TUI and Web confirmed it) by two changes: the UI-BD-6 authority claim is removed and UI-SA cites HP VW-8 (A4r44) for pointers; UI-CF-5(a) shows the ASCII host. Earlier: candidate 2 (`b700628a…3fed`) and candidate 1 (`f774e173…2ad5`). Candidate 2 added E1 to E3 (TUI). Candidate 3 adds D1 to D5 and the surfaces note (Web): pending slots, no URL rewrite, unnamed slots kept, exact external-open rule, no `{url}` on buttons.
Base: `ui-contract-v1-frozen-20260930.md`, sha256 `2063e8977c36c0e3cacdd0526de131f537b8bc63f4d39ec6db76e3300e5a7661`. This file changes nothing in the base except where it says so. Clause ids continue the base.
Source: `audits/plugin-parity-20260930.md` section 3, gaps G-13, G-14, G-15, G-16, G-20.

**Version.** The additions are MINOR (UI-VR-1). `ui_contract_view` becomes `"1.1"`. A view that uses any clause below sets `requires: ["ui_1_1"]` (UI-VR-4). A client that supports only 1.0 shows a local diagnostic for that view (UI-CF-1). The Hub sends the same projection to every client.

## A1. Activation: navigate and open a URL (G-13, G-16)

`Activate` (UI-SL-3) gains two forms:

```
Activate = { select: "<slot>" } | { invoke: "<action id>" } | { open: "<ref id>" }
         | { navigate: { view: "<package>/<view id>", select?: { "<slot id>": Operand } } }
         | { url: Operand }
```

| # | Clause | Test |
|---|---|---|
| UI-SL-5 | **Navigate.** `navigate.view` names a view of the **same package**. The client opens that view. For each `select` entry, the client resolves the operand (scope of UI-BD-3) and sets that slot of the **target** view to the resolved row key, before the first realization. A slot the target does not declare, or an operand that is absent, leaves the slot empty and raises a local diagnostic (never an error to the plugin). After the open the slots are client-local (P5, UI-SL-1). **Slots the navigation does not name keep their current state** when the target view is already open. **Pending preset:** a preset key that is not yet known stays **pending** while its source kind has no baseline (Hub→client WT-4); it is applied when the baseline arrives and the row exists. UI-SL-2 clears a slot only after the baseline has arrived and the row is absent, or when a present row leaves the kind. A view id the package does not have fails admission (`unknown_view`). | `conf::ui_sl_5_navigate_presets_slots`, `conf::ui_sl_5_absent_operand_leaves_slot_empty`, `conf::ui_sl_5_unknown_view_fails_admission`, `conf::ui_sl_5_unnamed_slots_keep_state`, `conf::ui_sl_5_preset_pending_until_baseline` (vector: URL load, then baseline with the row) |
| UI-SL-6 | **Navigation target and URL form.** The client navigation target (Hub→client `navigation` kind) gains an optional `select: { "<slot id>": "<row key>" }`. A Web client maps a target to the URL `…/<package>/<view>?<slot>=<row key>`, each key percent-encoded, and restores the same slots when it loads that URL. A URL with an unknown slot ignores it. **The URL is not rewritten by later selection:** slots stay client-local, and a reload restores only the navigation preset (vector `ui_sl_6_url_not_rewritten_by_selection`). Push notices and cross-page buttons use this target; they carry row keys only, never other view state. | `conf::ui_sl_6_target_select_roundtrips_url`, `conf::ui_sl_6_url_encoding_of_row_key` |
| UI-FD-1 | **`Field.display: "link"`.** The value must be an `http` or `https` URL without userinfo. The client shows the host (and the field `label`), and on activation opens the URL **outside the app with no opener and no referrer** (Web: `window.open(url, "_blank", "noopener,noreferrer")`; other clients: the platform's equivalent). The displayed host is the ASCII (punycode) form plus any non-default port, so a look-alike host is not hidden. A client that cannot open a URL follows UI-CF-5(a). Any other value (another scheme, userinfo, an invalid URL, absent) is shown as plain text and is **not** activatable. The URL is never fetched by the client as part of realization. | `conf::ui_fd_1_link_http_https_only`, `conf::ui_fd_1_non_url_is_plain_text`, `conf::ui_fd_1_userinfo_refused` |
| UI-SL-7 | **`{ url: Operand }` activation** has the same rule as UI-FD-1 for the resolved value. It may be used on a `list`, a `table` or a `Ref` (as `activate`, UI-SL-3). It is **not** allowed on an action button; a link in a toolbar is a `link` field. A refused value makes the activation inert with a local diagnostic. | `conf::ui_sl_7_url_activation_same_rule` |
| UI-RZ-2e | **Realized props.** `navigate` realizes as the prop `activate = { navigate: { view, select: { slot: rowKey } } }` with operands resolved. `url` realizes as `activate = { url }` (or plain text per UI-FD-1). `link` realizes as a `text` node with the prop `href` when valid. These props are optional additions to UiNode (MINOR), listed with UI-RZ-2a. | `conf::ui_rz_2e_navigate_and_link_props` |
| UI-CF-5 | **Fallbacks for 1.1 (extends UI-CF-2).** (a) A renderer that cannot open a URL outside the app (`external_open` absent from `ui_capabilities.features`) shows the **full validated URL as text**, with its host in the ASCII (punycode) form and any non-default port, next to the field label, needs no activation, and leaves no dead control; a `{url}` activation on it is not offered. (b) A navigation badge without a badge widget is a suffix on the entry label: `(n)` for `count`, `●` for `dot`, styled by tone where the renderer can. (c) A renderer without a home area shows a home-placed view in its normal place (UI-PL-1). A renderer declares `external_open` in `ui_capabilities.features` when it can open a URL (diagnostic and local, UI-CF-1). | `conf::ui_cf_5_no_external_open_shows_full_url`, `conf::ui_cf_5_badge_suffix_fallback` |

## A2. Session rows in a view (G-14)

| # | Clause | Test |
|---|---|---|
| UI-BD-6 | A view may use `owner` of the Hub `session` kind (the spawning plugin key) in `where`, `order`, a field or a ref **only when the kind's `projection_allowlist` lists `owner`** (UI-AD-1). This amendment adds no other allowlist rule. **Dependency:** the client `session` row gains `owner?` and the Hub's `session` descriptor lists it in `projection_allowlist` (plan auditor, Hub→client). This amendment adds **no row-scope rule**: a view sees the session rows its client receives under the client contract's own rules, and a filter on `owner` only narrows that set on the client. A view never grants access to a row. Example: a list of sessions with `where eq(path /owner, lit "knowledge-inbox-pipeline")`. | `conf::ui_bd_6_owner_needs_allowlist`, `conf::ui_bd_6_owner_filter_realizes` |

## A3. Home placement and navigation badges (G-15)

```
ViewDocument.placement? = { area: "home", size: "small"|"medium"|"wide"|"full", order: integer }
Badge = { source: { kind, id, field }, display: "dot"|"count", tone?: neutral|info|success|warning|danger }
```

| # | Clause | Test |
|---|---|---|
| UI-PL-1 | **Home placement.** A view with `placement` is also offered in the client's home area, in ascending `order` (ties by view id). The client maps `size` to its grid: a client with fewer sizes maps `full` to `wide` and `wide` to `medium`; `small` is the smallest. A client without a home area shows the view in its normal place only. A home-placed view uses the same parts and the same realization as any view. A client MAY show fewer parts in a small size, using the UI-CF-2 fallbacks; it never drops data without a visible notice (P4). The placement is part of the shared projection (UI-CF-1). | `conf::ui_pl_1_home_order_and_size_fallback`, `conf::ui_pl_1_no_home_area_shows_normal_place` |
| UI-NB-1 | **Navigation badge.** A `navigation` entry (Hub→client `navigation` kind, declared by the package) may carry `badge`. `source` names **one row and one field**: `kind` (an admitted kind, UI-BD-2), `id` (the row id) and `field` (a path in the projection allowlist). There is no aggregation (Q4 holds): a plugin that needs "any question is open" publishes a summary row with an integer or boolean field. `display: "count"` shows the field when it is an integer greater than 0; `display: "dot"` shows a dot when the field is truthy (UI-BD-1). A missing row, an absent field, `0`, `false` or a type mismatch hides the badge. `tone` maps to a style (UI-CF-4). The client watches the source kind only while the navigation is shown. | `conf::ui_nb_1_count_dot_hidden_rules`, `conf::ui_nb_1_absent_row_hides_badge`, `conf::ui_nb_1_source_in_allowlist` |
| UI-AD-3 | Admission also checks: `placement` values, `navigate.view` ids, `badge.source` against UI-AD-1 (kind admitted, field in `projection_allowlist`), and `display: "link"` only on a text-valued field. | `conf::ui_ad_3_new_admission_rules` |

## A4. Plugin-contributed session actions (G-20)

Hub→plugin VW-8 lets a plugin contribute an action to Hub session rows. The plugin-platform pair adds `label_pointer`, `status_pointer` and `url_pointer` (JSON pointers into the session row's `metadata[<plugin key>]`, which the plugin writes with `sessions.update`) and `applies_to.has_port`. This section states how a client renders them.

| # | Clause | Test |
|---|---|---|
| UI-SA-1 | **Pointers (cites Hub→plugin VW-8, A4r44).** A pointer stays inside the package's own `metadata[<package>]` (the Hub fails the load otherwise). A pointer that resolves to nothing or to a non-string **shows nothing**: no error, no default, no local diagnostic. **Label.** If `label_pointer` resolves to a non-empty string, the client shows it as the action label. If it shows nothing, the action keeps its **declared static `label`** (the manifest requires one; that is the declaration, not a default of the pointer). | `conf::ui_sa_1_dynamic_label_or_static` |
| UI-SA-2 | **Status.** If `status_pointer` resolves to a string, the client shows it adjacent to the action, or in the same session panel, as secondary text in the neutral tone. An error is conveyed by the plugin in that text (for example "Retry: tunnel failed"). The client adds no tone of its own. | `conf::ui_sa_2_status_text_neutral` |
| UI-SA-3 | **Link.** If `url_pointer` resolves to a value, the client shows it by UI-FD-1 (host shown, opened outside the app, plain text if invalid) and by UI-CF-5 when it cannot open a URL. | `conf::ui_sa_3_url_uses_link_rule` |
| UI-SA-4 | **Changes.** The three values are read from the watched `session` row. A `sessions.update` by the plugin re-realizes the action without a view reload. The rules apply **wherever the client shows the session's actions** (toolbar, panel, menu). The action's `allowed` and `reasons` (DS-1) still decide whether it is enabled (UI-AC-5). A pointer that resolves to nothing has no effect. | `conf::ui_sa_4_metadata_change_updates_action`, `conf::ui_sa_4_absent_pointer_is_static` |

## A5. `dialog` (confirmation requested by the plan auditor)

The ui-contract has **no `dialog` part** (UI-AC-4; the base contract lists six data parts and seven structural parts). Its realized trees contain **`dialog` nodes** (UI-RZ-2b): the UiNode vocabulary needs them for an open action form and a pending confirmation. So "`dialog` is not a part" and "realized overlays are `dialog` nodes" are both true. Hub→plugin VW-1 must not list `dialog` among the parts a plugin authors (section 17 of the base, amendment 2 for Hub→client 7b).

## A6. Not in this amendment

- U-5 (a plugin's HTML in a sandboxed iframe) stays out of the contract unless the user rules otherwise (HP section 1).
- G-11 (push delivery) and G-12 (terminal notification policy) belong to the client contract.
- G-21 fields other than `owner` (worktree, branch, title) need no UI change: they are ordinary allowlisted fields when the descriptor lists them (UI-AD-1).

## A7. Changes to the base text (only these)

1. UI-SL-3: the `Activate` grammar is the one above (A1).
2. UI-VR-1: `"1.1"` is the current MINOR; feature name `ui_1_1` (UI-VR-4).
3. UI-RZ-2a: adds the props of UI-RZ-2e.
4. Section 19: no new open item.
