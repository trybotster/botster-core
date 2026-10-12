# UI contract v1: the declarative view schema (FROZEN 2026-09-30, from candidate7)

Frozen: Sol Hub A ACCEPT DESIGN, Web 001a and TUI 001b ACCEPT, on candidate7 sha256 `1893208bfa773c0461b6f44825b8fef7e3501aeee5f09be2de9411d60e912a56`. This file differs from candidate7 only in this heading and line. Changes go through the change process.

Author: plugin kit writer (Claude, sess-1790726020-0017), 2026-09-30. Reviewer: Sol Hub A (001f). Consumers: Web writer (001a), TUI writer (001b). Status: DESIGN ONLY. No code changes. Draft 1 is kept as `ui-contract-v1-draft1.backup.md`.

Inputs:
- Web review section I, items 1 to 10 (`handoffs/web-review-of-contract-v1-draft5-20260930.md`), tracked as #102.
- Hub→client contract section 7b and SF-0 to SF-5; Hub→plugin contract EN-1, EN-9, VW-1 to VW-7.
- The TUI facts sheet and the TUI review of draft 1 (`handoffs/tui-review-ui-contract-v1-draft1-20260930.md`, D1 to D7).
- Sol Hub A's HOLD on draft 1 (`verdicts/ui-contract-v1.md`): three required changes, the seam with VW-6, and the example defects.
- The project-pipelines view inventory (appendix A).

**What changed since draft 1** (section 18 has the full list): the Hub never replaces or filters anything (UI-RC-4, UI-FM-3, UI-CF-1); refs are declared on the part; `realize` is production code and its output is the existing UiNode vocabulary through a normative mapping (UI-RZ-2); the viewport is an operand and a `realize` input; fallbacks for array and object properties; opaque bounded node ids; no numeric limit (UI-LM-1); confirmation is client presentation only; the amendments to VW-6 and section 7b are listed (section 17).

## 1. Scope and principles

A plugin ships **views**. A view is a **data document** that names which kinds to show, how to show them, and which actions a user can start. The client renders the document. The plugin runs no code in the client (VW-2).

| # | Principle |
|---|---|
| P1 | **Declarative data only.** A view has no callback, script or expression language. The only logic is the closed predicate grammar of section 4. |
| P2 | **One schema, two renderers.** Web and the TUI render the same document. A renderer may show a part in a different way. It may not change what the document means. |
| P3 | **Closed and small.** The part set is fixed per version. A new part is a negotiated feature (UI-VR-4). |
| P4 | **Every part has a fallback** (UI-CF-2). A renderer never drops data without a visible notice. |
| P5 | **The view owns no state.** Drafts, selection, row focus, scroll and open overlays stay in the client. They are never on the wire. |
| P6 | **The Hub admits. The client realizes.** The Hub checks a document at load (UI-AD-1). It resolves, replaces and filters nothing. The client turns the document and the watched kinds into a realized tree by one pure function (section 12). Every implementation passes the same vectors. |
| P7 | **Stable ids** (UI-ID-1 to UI-ID-4). |
| P8 | **The Hub and the plugin own authority.** A view only helps a user build a request. The Hub validates the request against the action's canonical schema. The plugin authorizes it. |

Out of scope: hand-built screens (terminal, logs), bound to `kind@MAJOR` (Hub→client DS-4); the watch and invoke wire protocol (Hub→client sections 5 and 7b); animation, timers, pixel layout, translation.

## 2. Document model

```
ViewDocument {
  ui_contract_view: "M.m",            // UI-VR-1
  id,                                 // stable, unique in the package
  title,
  requires?: [feature],               // UI-VR-4
  selections?: [ { id } ],            // client-local slots (section 8)
  root: Part
}
```

| # | Clause | Test |
|---|---|---|
| UI-ID-1 | Every `Part`, `Field`, `Column`, `Ref`, `Action` and `Selection` has an author-given `id`. An id is a non-empty string of `[a-z0-9_.-]`, with no length limit (UI-LM-1). **An id is unique inside the whole view document across all of those kinds**, so one name never means two things. An id is never one of the reserved role words `label`, `value`, `widget`, `help`, `error`, `container`, `item`, `cell`, `action`, `empty` (UI-RZ-2). A reload that keeps the view's MAJOR keeps every id (Hub→client SF-1). | `conf::ui_id_1_unique_across_kinds_and_stable` |
| UI-ID-2 | A static part has the **node id** `<part.id>`. A part realized once per bound row has a node id built from `<part.id>` and the row key by the UI-ID-3 algorithm. A child of a row part has a node id built from the row node id and `<child.id>` by the same algorithm. A generated node (label, value, widget, container) adds a **role segment** (UI-RZ-2). A node id never uses an array index. | `conf::ui_id_2_node_ids_from_ids_and_row_keys` |
| UI-ID-3 | **Node ids are opaque strings to every client.** They are built by one function that reuses the shipped length-prefixed scheme (`botster-ui-descendant-v1:<len>:<rowId><len>:<key>`, Web `realizeBindListDescendantId`, with its vectors), so no separator can collide with a key. A row key is the row's own `id` (for example `<hub_id>/<session_id>`), so it may contain `/` and `:`. The function and its inverse are in the contract crate, with vectors for keys that contain `/`, `:`, `|` and `\`. | `conf::ui_id_3_escape_roundtrip`, `conf::ui_id_3_row_key_with_separators` |
| UI-ID-5 | **Draft keys.** A form field's draft key is the form's node id plus the `field_id` (UI-ID-3 algorithm). Property names need not be unique across different forms of one document, so two actions may both have a `name` property. `local_state` and the kit key drafts by this draft key, never by the bare property name. | `conf::ui_id_5_draft_key_includes_form_node_id` |
| UI-ID-4 | A result or error that refers to a part uses its node id. A form error uses the property name as `field_id` inside its form (UI-FM-7, UI-ID-5). | `conf::ui_id_4_results_keyed_by_node_id` |

## 3. The closed part set (version 1.0)

**Data parts** (six, the settled component set) bind a kind. **Structural parts** (seven) arrange and label. There are no other parts. A `dialog` is not a part (UI-AC-4).

| Part | Class | Purpose | Key properties |
|---|---|---|---|
| `list` | data | Rows of one kind, one item per row | `source`, `refs?`, `item` (fields and actions per row), `order`, `activate`, `selects`, `empty`, `error` |
| `table` | data | Rows as columns | `source`, `refs?`, `columns[{id,label,path|ref,display?,align?}]`, `order`, `activate`, `selects`, `empty`, `error` |
| `detail` | data | One row as labeled fields | `source` (`{kind, id}` for one row, or `{sel}`), `refs?`, `fields[Field]`, `empty`, `error` |
| `form` | data | Inputs for one action | `action`, `schema` (section 7), `submit`, `cancel` |
| `actions` | data | A set of buttons | `items[ActionRef]`, `overflow` |
| `status_badge` | data | One value as text and tone | `value` (an Operand), `map` (value to label and tone) |
| `stack` | structural | Children in a column or row | `direction`, `children`, `when?` |
| `inline` | structural | Children in one wrapping line | `children`, `when?` |
| `panel` | structural | A titled group | `title`, `children`, `when?` |
| `section` | structural | A heading and children | `heading`, `children`, `when?` |
| `toolbar` | structural | Actions with an overflow rule | `items[ActionRef]`, `overflow: auto|never` |
| `text` | structural | Static copy or one bound value | `value` (literal or Operand), `tone?` |
| `empty_state` | structural | Copy for no data | `title`, `body?` |

`Field.display` is one of `text` (default), `badge`, `metric` or `number`. `metric` realizes as a labeled count, so a counter needs no separate part (appendix A).

| # | Clause | Test |
|---|---|---|
| UI-PT-1 | The part set is exactly the table above. A document with another `type` fails admission (`unsupported_part`). | `conf::ui_pt_1_unknown_part_refused` |
| UI-PT-2 | **Seam with the other contracts (required amendments, section 17).** The six data parts are the settled component set. The seven structural parts are the client-rendered layout and presentation vocabulary that Hub→client section 7b already names (minus `dialog`). They carry no data binding and no behavior beyond `when`. | `conf::ui_pt_2_structural_parts_have_no_bindings` |
| UI-PT-3 | A `when` on a structural part is a predicate over the **viewport** only: `{ viewport: "width" }`, `{ viewport: "height" }` and `{ viewport: "pointer" }` compared with `eq`/`ne` to a class. Width classes: `compact`, `regular`, `expanded`. Height classes: `short`, `regular`, `tall`. Pointer classes (`{ viewport: "pointer" }`): `fine` = click and hover; `coarse` = click or tap without hover; `none` = keyboard only. A renderer declares the class its input allows; the TUI declares `fine` with mouse capture on and `none` with it off. Exclusivity applies only to **conditional alternatives**: siblings that all carry a `when` and replace each other (declared with `alternatives: true` on the parent). Ordinary unconditional siblings, and siblings that may show together, need no exclusivity. Alternatives have **mutually exclusive** `when` predicates: admission enumerates all 27 (width, height, pointer) classes and refuses an alternatives set that is true together for any pair. | `conf::ui_pt_3_when_siblings_exclusive_by_enumeration` |
| UI-PT-4 | A `toolbar` or `actions` part orders items by `priority` (UI-AC-2), then declaration order. `overflow: auto` moves the lowest-priority items into an overflow menu when width is short. | `conf::ui_pt_4_overflow_by_priority` |

## 4. Sources, refs, operands and predicates

```
Source  { kind, where?: Pred, order?: [{field, dir: asc|desc}] }
Path    = "/field" | "/field/sub"                    // JSON Pointer into a row
Operand = { path }                                    // a field of the bound row
        | { row: Path }                               // the row an action or item sits on
        | { form: Path }                              // the current value of a form field
        | { sel: "<selection id>" + Path }            // the selected row of a slot
        | { ref: "<ref id>" + Path }                  // a field of a resolved ref (section 5)
        | { viewport: "width"|"height"|"pointer" }
        | { lit: value }
Pred    = { eq: [Operand, Operand] } | { ne: [Operand, Operand] } | { truthy: Operand }
        | { not: Pred } | { all: [Pred] } | { any: [Pred] }
```

| # | Clause | Test |
|---|---|---|
| UI-BD-1 | The grammar is complete. No arithmetic, string operation, function call or regular expression exists. `truthy` is false for `null`, absent, `false`, `0`, `""` and `[]`. `eq` compares JSON values exactly. | `conf::ui_bd_1_predicate_truth_table` (covers eq, ne, truthy, not, all([]), any([]), absent operands) |
| UI-BD-2 | A `Source.kind` names a kind the package may watch: a kind of the same package, or a Hub kind its grants allow (Hub→client DS-7, EN-9). The client watches it while the view is shown (Hub→client section 5). | `conf::ui_bd_2_source_kind_admitted` |
| UI-BD-3 | **Scope of operands.** `path`: inside a part with a `source` (item, columns, fields, `where`). `row`: inside `item`, `columns`, row `actions`, and `options_source.where` of an action on a row. `form`: inside a `form` and its `options_source` and `visible_when`. `sel`: anywhere a selection slot is declared. `ref`: inside a part that declares the ref. `viewport`: only in `when`. A use outside its scope fails admission (`unbound_context`). | `conf::ui_bd_3_operand_scope` |
| UI-BD-4 | A path that resolves to nothing is `absent`. `truthy(absent)` is false. `eq(absent, literal)` is false. `ne(absent, literal)` is **true**. `not(false)` is true. `all([])` is true. `any([])` is false. An absent operand on both sides: `eq` is true and `ne` is false. A field shown from an absent path renders as empty text, never as an error. | `conf::ui_bd_4_absent_path` |
| UI-BD-5 | **The Hub resolves no binding.** A client realizes the document (section 12) from the watched kinds and its local state. Two implementations that pass the vectors produce the same realized tree for the same inputs. | `conf::ui_bd_5_realization_vectors` |

## 5. Typed references (Web items 3 and 9)

A data part declares its refs once. One declaration serves a filter, an item, a column and an activation: **one grammar covers them all** (Sol Q3).

```
Part.refs = { <ref id>: Ref }
Ref = { kind, key: Path | { hub_id: Path, id: Path },   // names the target row; the structured form carries the hub
        absent: { text, tone? },         // what to show when no target row exists
        activate?: Activate }
```

| # | Clause | Test |
|---|---|---|
| UI-RF-1 | A `Ref` resolves `key` against the watched target kind by the target's own `id`. A structured key `{ hub_id, id }` names the hub too (federation rule); the membership row of section 14 uses it. A target on another hub resolves to `absent` with reason `remote_hub_unsupported` in v1. The realized node carries the reason as the prop `absent_reason` (also `not_found` and `no_baseline`). The client watches the target kind while the view is shown. | `conf::ui_rf_1_ref_resolves_by_id` |
| UI-RF-2 | When the target row does not exist, or the target kind has no baseline yet (Hub→client WT-4), every `{ ref }` operand of that ref is `absent` (UI-BD-4) and the field shows `absent.text` with `absent.tone`. A row with an absent target is still a row. The part does not fail. | `conf::ui_rf_2_absent_target_shown`, `conf::ui_rf_2_no_baseline_is_absent_not_error` |
| UI-RF-3 | A `{ ref: "<ref id>/<field>" }` operand is valid wherever `{ path }` is valid in the same part: in `Source.where` (so "current" and "unavailable" groups are two parts over one kind), in an item or column `value`, in a `Field`, in a `status_badge` value, and in `when`-free `text` inside an item. | `conf::ui_rf_3_ref_operand_everywhere_path_is` |
| UI-RF-4 | `Ref.activate` is used when the user activates a value that shows the ref (section 8). | `conf::ui_rf_4_ref_activation` |

### Field (the one definition)

```
Field { id, label?, path? | value?: Operand,        // exactly one of path, value
        display?: text|badge|metric|number,         // default text
        map?: { <value>: { label, tone } },         // for badge and metric tone; looked up by exact value
        tone?: neutral|info|success|warning|danger, // fixed tone when no map applies
        absent?: text }                             // shown when the value is absent; default empty
```

A `Field` appears in `item.fields`, `detail.fields` and `columns` (a column adds `align`). `status_badge.map` has the same shape as `Field.map`. A `display` or `tone` the renderer does not know is ignored and counted (UI-VR-2).

## 6. Row context and action binding (Web items 1 and 2)

An action on a row acts on that row.

| # | Clause | Test |
|---|---|---|
| UI-RC-1 | An `ActionRef` inside a row part has an implicit **target**: the row. The client sends `target = row key` (Hub→plugin VW-3). | `conf::ui_rc_1_row_action_targets_row` |
| UI-RC-2 | An action may declare `args`: `{ <arg name>: Operand }`. The **client resolves** each operand (`row`, `sel`, `form`, `lit`, `path`, `ref`) and **sends the resolved value** as an ordinary argument. A bound argument is not shown in a form unless the form declares a field of the same name. | `conf::ui_rc_2_client_resolves_args` |
| UI-RC-3 | `options_source.where` may use `{ row }` to compare with the row being acted on. "Every workspace except this one" is `{ ne: [ {path:"/id"}, {row:"/id"} ] }`. | `conf::ui_rc_3_exclude_current_row` |
| UI-RC-5 | **Absent bound operand.** An action whose `args` contain an operand that is `absent` at realization (for example `{sel}` with nothing selected) is shown **disabled** with the action's optional `disabled_reason` copy, or the generic text "Nothing selected", and the client sends no request. A vector covers Rename with no selection. | `conf::ui_rc_5_absent_bound_operand_disables` |
| UI-RC-4 | **The Hub replaces nothing.** Generic invoke validates the request against the action's **canonical schema** (Hub→plugin VW-3). The plugin authorizes the target and the arguments. A client may send any value for an argument the view binds; that is an ordinary request, validated and authorized like any other. A plugin that must trust an argument derives it from the target (the plugin loads the target row from its own store) and does not rely on the view's binding. | `conf::ui_rc_4_hub_replaces_nothing`, `conf::ui_rc_4_bound_arg_is_validated_as_ordinary_input` |

## 7. Forms (Web items 4 and 5; SF-2, SF-3, SF-4)

```
form.schema   = JSON Schema subset: object with properties, required, enum, default
property.ui   = { placeholder?, help?, read_only?, group?, advanced?, order?, widget?,
                  visible_when?: Pred, required_when?: Pred, options_source?: OptionsSource }
OptionsSource = { kind, where?: Pred, value_field, label_field, order?,
                  exclude?: { kind, where?, value_field }, empty?: Copy }
```

| # | Clause | Test |
|---|---|---|
| UI-FM-1 | The schema subset: `type` of `string|integer|number|boolean|array|object`, `properties`, `required`, `enum`, `default`, `minimum`, `maximum`, `minLength`, `maxLength`, `items`. No `$ref`, `oneOf`, `anyOf`, `if/then/else` or `pattern`. **The form's schema is the action's canonical schema** (the registered descriptor). Admission refuses a form whose schema differs from its action's. **Comparison:** remove every `ui` hint from the view's schema, then remove every property that the action binds in `args` (from `properties` and from `required`); the result must equal the canonical schema with the same bound properties removed. Each bound name must exist in the canonical schema, and the type of its operand (when known from the kind's schema) must be compatible with the property type. | `conf::ui_fm_1_schema_subset`, `conf::ui_fm_1_form_schema_equals_action_schema` |
| UI-FM-2 | The `ui` hints are presentation only and never change validation: `placeholder`, `help`, `read_only`, `group`, `advanced`, `order`, `widget` (Web hint; `text|textarea|select|checkbox|number|password`; a renderer may ignore every `widget` and choose by JSON type, but **a `password` widget or a `secret: true` property never falls back to an echoing input**: a renderer without a masked input refuses the form with a diagnostic), `visible_when`, `required_when`, `options_source`. | `conf::ui_fm_2_hints_do_not_change_validation` |
| UI-FM-3 | **Hidden fields follow the canonical schema.** `visible_when` shows or hides a field. The client **omits a hidden field** from the request. A field that may be hidden **must be optional in the action's schema**; admission refuses a `visible_when` on a property listed in `required`. The plugin enforces a condition such as "branch is required for a git target" and answers `field_errors` (UI-FM-7). `required_when: Pred` is a **client-side pre-check only**: it marks the field required in the form and blocks the submit locally; it never changes what the Hub validates. | `conf::ui_fm_3_hidden_field_omitted`, `conf::ui_fm_3_visible_when_needs_optional_property`, `conf::ui_fm_3_required_when_is_client_precheck` |
| UI-FM-4 | `options_source.where` may use `{ form }`. The options follow another field's value, and the client watches the source kind (SF-4). A selected value no longer in the options is cleared by the client and marked with a local field error `option_unavailable`. | `conf::ui_fm_4_dependent_options`, `conf::ui_fm_4_stale_selection_cleared` |
| UI-FM-5 | A property may have `default` (a literal) or `default_from: Operand`. A `read_only` field with a default is shown and sent. A default is shown as a default, not as a stored value (SF-3). | `conf::ui_fm_5_defaults_and_readonly` |
| UI-FM-6 | `group` names a section. A section collapses only when every field in it has `advanced: true`. An `advanced` field inside a section that has any non-advanced field is shown **in place** in that section. Fields with `advanced: true` and no `group` are shown in one collapsed "optional" section. Order follows `order`, then declaration. | `conf::ui_fm_6_optional_group` |
| UI-FM-7 | Every property's `field_id` is its property name. A field's identity is the form's node id plus the `field_id` (UI-ID-5); property names need not be unique across forms, and a form property is a `Field` for UI-ID-1 only inside its form. A refusal carries `field_errors: [{ field_id, pointer, code, message }]` (SF-2). The client marks the field. An error with no field is a form-level message. | `conf::ui_fm_7_field_errors_by_field_id` |
| UI-FM-8 | A property with `secret: true` never receives its value. The form shows `set: bool`. An empty value means "unchanged" unless the client sends `clear: true` (SF-3). | `conf::ui_fm_8_secret_contract` |

## 8. Selection, focus, activation and navigation (Web items 8 and 9)

| # | Clause | Test |
|---|---|---|
| UI-SL-1 | A document declares `selections[{ id }]`. A `list` or `table` with `selects: "<id>"` sets that slot to the row the user **activates**. A slot holds one row key or nothing. The slot is **client-local** (P5). | `conf::ui_sl_1_selection_is_client_local` |
| UI-SL-2 | A part whose `source` is `{ sel: "<id>" }`, or whose `where` tests `{ sel: "<id>/id" }`, follows the slot. With no selection it shows its `empty` copy. When the selected row leaves its kind, the slot clears. | `conf::ui_sl_2_detail_follows_selection`, `conf::ui_sl_2_deleted_row_clears_selection` |
| UI-SL-3 | `activate` (on a `list`, a `table` or a `Ref`) is one of `{ select: "<id>" }`, `{ invoke: "<action id>" }` or `{ open: "<ref id>" }`. `open` asks the client to show its **own screen** for the referenced kind (a hand-built screen bound to `kind@MAJOR`, DS-4). With no such screen the client shows the fallback of UI-CF-2 (the row's fields). | `conf::ui_sl_3_activation_forms`, `conf::ui_sl_3_open_without_screen_falls_back` |
| UI-SL-4 | **Focus is not selection.** Moving row focus (a TUI cursor, a Web hover or arrow key) is client-local and does not set a selection slot. A slot changes only on activation (Enter, click, tap). | `conf::ui_sl_4_focus_does_not_select` |

## 9. Actions, confirmation and priority (Web items 6 and 10; DS-1)

```
Action { id, label, description?, handler,          // a registered ui_action handler
         args_schema?, args?,                        // UI-RC-2, section 7
         confirm?: { message, tone: neutral|danger },
         priority?: primary|secondary,               // default secondary
         when?: Pred }                               // visibility over row, form, sel, ref
```

| # | Clause | Test |
|---|---|---|
| UI-AC-1 | An action is `invoke(<bound kind>, target?, action, args)` (SF-2). The plugin handles it as a `ui_action` (VW-3). The result is `{ ok, field_errors?, message? }` keyed by node id. A successful result carries no replacement tree: the view changes through the watched kinds (VW-4). | `conf::ui_ac_1_invoke_result_shape` |
| UI-AC-2 | `priority: primary` marks the emphasized action, at most one per toolbar. A renderer maps it to its accent. It changes order and emphasis only. | `conf::ui_ac_2_priority_order_and_emphasis` |
| UI-AC-3 | **Confirmation is a client presentation rule.** The client shows `confirm.message` with the declared tone before it sends the request, and sending nothing on cancel. The Hub does not enforce it, because a request flag cannot prove a prompt was shown. A plugin that needs a guarded operation adds its own argument and checks it. | `conf::ui_ac_3_confirm_is_presentation`, `conf::ui_ac_3_cancel_sends_nothing` |
| UI-AC-4 | **Dialogs are a client rule.** An action with `args_schema` that has at least one user-entered field is rendered as a form in an overlay or sheet. An action without one runs directly. **Order: the form first, then the confirmation, then the request.** Overlay open state is client-local. | `conf::ui_ac_4_dialog_is_client_rule`, `conf::ui_ac_4_form_then_confirm` |
| UI-AC-5 | An action the caller may not run is shown disabled with the Hub's reason (DS-1 `allowed`, `reasons`), or hidden when `when` is false. A client never hides an action by its own policy. | `conf::ui_ac_5_disabled_with_reason` |

## 10. Copy and states (Web item 7)

```
Copy { title, body?, tone? }
```

| # | Clause | Test |
|---|---|---|
| UI-CP-1 | Every data part and every `options_source` may declare `empty: Copy` and `error: Copy`. The client shows `empty` when the source has a baseline and no rows; `error` when the watch fails or ends, with the Hub's error message appended when one exists; a neutral "loading" indicator before the baseline. | `conf::ui_cp_1_empty_error_loading` |
| UI-CP-2 | A part with no `empty` or `error` uses a generic default. | `conf::ui_cp_2_generic_defaults` |
| UI-CP-3 | Copy is literal text. Translation is out of scope for v1. | `conf::ui_cp_3_copy_is_literal` |
| UI-CP-4 | An option source with no options shows its own `empty` copy inside the field (for example "No other workspace"). The action stays visible. | `conf::ui_cp_4_empty_options_copy` |

## 11. Renderer capabilities and fallbacks

A renderer declares `ui_capabilities { ui_contract_view: ["M.m", ...], parts: [...], features: [...], widgets: [...] }` (the TUI has `tui_capabilities` today). The declaration is **diagnostic and local**: a client uses it to choose fallbacks and to explain a diagnostic. The Hub never reads it to filter a projection (UI-CF-1).

| # | Clause | Test |
|---|---|---|
| UI-CF-1 | **One shared projection.** The Hub sends **every admitted view to every client** in the one package projection (Hub→plugin EN-9). It never filters a view by a client's capabilities. A client that does not support a view's `ui_contract_view` or `requires` shows **a local diagnostic** (view id and reason) in place of the view. Capability-dependent information, if ever needed, is a separate `describe` result and not part of the projection. | `conf::ui_cf_1_same_projection_for_all_clients`, `conf::ui_cf_1_unsupported_view_is_local_diagnostic` |
| UI-CF-2 | **Fallbacks (normative).** `table` without a column grid: a `list` of `label: value` lines. `status_badge`: label text plus the tone word in brackets. `detail`: labeled lines. `toolbar` overflow: a `menu`. `panel` and `section`: a heading and indented children. `metric`: `label: value`. A `widget` the renderer lacks: the default for the JSON type, except `password` and secret properties (UI-FM-2). `open` without a screen: the row's fields. | `conf::ui_cf_2_fallbacks` |
| UI-CF-3 | **Properties with no editor.** An `array` of strings is one text field, one element per line; an element that contains a newline is refused at admission for a `default` and shown read-only as JSON at run time. An `object` of strings is one text field of `KEY=VALUE` lines, **only when the round trip is lossless**: no key contains `=` or a newline, and no value contains a newline. The renderer splits each line at its first `=`. If any key or value breaks that rule, the renderer shows the property read-only as JSON and sends it unchanged. An unedited property is always sent as the same JSON value as received. An `array` or `object` of anything else, or a nested one, is shown **read-only as JSON and sent unchanged**; if it is `required` with no default, the renderer refuses the form with a diagnostic. | `conf::ui_cf_3_array_object_fallbacks`, `conf::ui_cf_3_object_roundtrip_lossless_or_readonly` |
| UI-CF-4 | Keyboard is first class: Tab order follows realized document order; Enter activates; no affordance is pointer-only. A tone maps to a style, never to a fixed color in the document (`neutral|info|success|warning|danger`). | `conf::ui_cf_4_keyboard_and_tones` |

## 12. Realization

`realize(document, kind_snapshots, local_state, viewport, capabilities) -> RealizedTree`

- `kind_snapshots`: each watched kind's rows at one cut (Hub→client WT-4).
- `local_state`: selections, form drafts, open overlays, row focus.
- `viewport`: `{ width, height, pointer }` classes (a terminal resize or a window resize changes this input, not the function).
- **`RealizedTree` is a tree of the existing UiNode vocabulary** (`botster-ui-contract` `UiNode`), produced by the normative mapping in UI-RZ-2. The renderers (botster-tui-kit, the Web UiNode renderer) stay.

| # | Clause | Test |
|---|---|---|
| UI-RZ-1 | `realize` is pure and deterministic: same inputs, same tree. It does no I/O and reads no clock. | `conf::ui_rz_1_pure_deterministic` |
| UI-RZ-2 | **Part to UiNode mapping (normative).** `list` → `list` of `list_item`; `table` → `table`; `detail` → `panel` of labeled `inline` rows (`text` label, `text` value, `badge`, `metric`); `form` → `form` with `form_section` and `form_field` holding `text_input`, `textarea`, `checkbox` or `select` by JSON type and `ui` hints; `actions` and `toolbar` → `toolbar` of `button` with `menu` overflow; `status_badge` → `status_badge`; `stack`, `inline`, `panel`, `section`, `text`, `empty_state` → the node of the same name. A `metric` field → `metric`. The mapping assigns each realized node a node id by UI-ID-2 and UI-ID-3. A part or field that realizes into several nodes gives each a fixed **role segment** appended by the same algorithm: `label`, `value`, `widget`, `help`, `error`, `container`, `item`, `cell`, `action`, `empty`. The part's outer node takes `container` only when the part has more than one node. Admission refuses an author id equal to a role word. | `conf::ui_rz_2_part_to_uinode_table` |
| UI-RZ-2a | **Node props (normative).** The mapping table in the contract crate lists, for each part and field, the props of its nodes: `label`, `value` (draft value when a draft exists), `selects`, `activate`, `confirm`, `action_ref`, `absent_reason`, `disabled_reason`, `error`, `options`, `placeholder`, `help`, `disabled`, `required`, `tone`, `selected`. A prop with no value is omitted. | `conf::ui_rz_2a_props_table` |
| UI-RZ-2b | **Overlays.** `local_state.overlays` lists open overlays. An open action form realizes as a `dialog` that holds the `form`. A pending confirmation realizes as a `dialog` with the message, a confirm `button` and a cancel `button`. With both pending, only the form shows first (UI-AC-4). Vectors cover "form, then confirm". | `conf::ui_rz_2b_overlay_dialogs`, `conf::ui_rz_2b_form_then_confirm` |
| UI-RZ-2c | **Drawable.** Every `expect` tree in a vector MUST render in botster-tui-kit and in the Web UiNode renderer with no unsupported-node and no unsupported-prop diagnostic. Both renderers run this check in their own suites. | `conf::ui_rz_2c_expect_trees_drawable` |
| UI-RZ-2d | **Source of truth.** `local_state` is the single source of truth for drafts, selections, row focus, scroll and overlays. A renderer MAY keep a widget's in-progress value inside the widget, but it MUST NOT treat that value as authoritative. It MUST publish the value to `local_state` before any `realize` that can depend on it: when a `{ form }` operand, a `visible_when` or an `options_source` reads it, and at submit. After `realize`, the renderer applies what `realize` changed (a cleared option per UI-FM-4, a stamped error, a reload) back into its widgets before the next draw. On conflict the output of `realize` wins. `realize` MUST be memoizable: stable node ids (UI-RZ-4) let focus survive a re-realize. Vectors check that the same `local_state` gives the same tree, not the renderer's architecture. Kit deliverable (open item O1): `set_draft_value`, `clear_draft_value`, `set_selected_row`, the new UiNode props and the `detail` and overlay mappings. | `conf::ui_rz_2d_local_state_is_source_of_truth` |
| UI-RZ-3 | **Implementations.** `realize` is **production code** in the `botster-ui-contract` Rust crate (used by the TUI and by Hub admission for grammar checks). Web implements the same function in its own language. The Hub runs no `realize` for a client. All implementations pass the **vectors**: JSON files `{ document, kinds, local_state, viewport, capabilities, expect }` in the crate's `vectors/`. | `conf::ui_rz_3_vectors_pass_rust`, `conf::ui_rz_3_vectors_pass_web` |
| UI-RZ-4 | Rows appear in `order`, then by row key. A live change moves a row without changing other rows' node ids. | `conf::ui_rz_4_order_stable_ids` |
| UI-RZ-5 | A duplicate realized node id is a diagnostic inside the part. It is never a crash. Admission prevents it for static parts (UI-ID-1). | `conf::ui_rz_5_duplicate_node_id_diagnosed` |

## 13. Admission, versioning and size

| # | Clause | Test |
|---|---|---|
| UI-AD-1 | At package load the Hub validates every view (Hub→plugin VW-1): known parts (UI-PT-1), unique ids (UI-ID-1), admitted source kinds (UI-BD-2), every path, field and ref path in the kind's **projection allowlist** (the `projection_allowlist` of the Hub's per-kind descriptor, never a client's own struct), operands in scope (UI-BD-3), every action naming a registered `ui_action` handler with the canonical schema (UI-FM-1), `visible_when` only on optional properties (UI-FM-3), every `selects`, `sel` and `ref` naming a declared slot or ref, sibling `when` exclusivity (UI-PT-3), predicates in the grammar. One failure fails the load with the path of the defect. | `conf::ui_ad_1_admission_rules` |
| UI-AD-2 | **Allowlist changes.** Adding a field to a kind's allowlist is a MINOR change of the kind. Removing one is a MAJOR change and makes every view that uses it `stale` (shown as a diagnostic) until the plugin reloads. | `conf::ui_ad_2_allowlist_minor_add_major_remove` |
| UI-VR-1 | `ui_contract_view: "M.m"`. A MINOR change is additive: a new optional property, `widget`, `tone`, `display` or a new part behind a feature. A MAJOR change removes or changes meaning. | `conf::ui_vr_1_additive_minor` |
| UI-VR-2 | A client supports a range of MAJORs. It ignores an unknown optional property, `widget`, `display` or `tone` (using the default) and counts it. An unknown **required** property makes the view a local diagnostic (UI-CF-1). | `conf::ui_vr_2_unknown_optional_ignored` |
| UI-VR-3 | The Hub supports the current and previous MAJOR until the next MAJOR. | `conf::ui_vr_3_previous_major_supported` |
| UI-VR-4 | A new part or predicate operator appears first behind a name in `features`. A document uses it only with `requires: ["<feature>"]`. | `conf::ui_vr_4_features_gate_new_parts` |
| UI-LM-1 | **No numeric limit in v1.** Depth, part count, copy length, node id length and document size have no limit, because no failure is observed (regrowth rule 1). A limit is added only with an observed failure and an approved number. The Hub's existing frame size bound still applies. | `conf::ui_lm_1_size_bound` |

## 14. Worked example: the workspaces plugin (informative; a conformance vector)

Kinds: `botster-workspaces.workspace` (`id`, `name`), `botster-workspaces.membership` (`id`, `workspace_id`, `session_id`, `hub_id`), and the Hub kind `session` (`id`, `label`, `lifecycle_class`). Every id is unique across the document (UI-ID-1).

```json
{
  "ui_contract_view": "1.0", "id": "workspaces", "title": "Workspaces",
  "selections": [ { "id": "sel_ws" } ],
  "root": { "type": "stack", "id": "page", "direction": "row", "children": [
    { "type": "list", "id": "workspace_list",
      "source": { "kind": "botster-workspaces.workspace", "order": [ { "field": "/name", "dir": "asc" } ] },
      "selects": "sel_ws",
      "item": { "fields": [ { "id": "ws_name", "path": "/name" } ] },
      "empty": { "title": "No workspaces yet" }, "error": { "title": "Workspaces unavailable" } },
    { "type": "stack", "id": "right", "direction": "column", "children": [
      { "type": "detail", "id": "ws_detail", "source": { "sel": "sel_ws" },
        "fields": [ { "id": "ws_detail_name", "label": "Name", "path": "/name" } ],
        "empty": { "title": "Select a workspace" } },
      { "type": "actions", "id": "ws_actions", "items": [
        { "id": "act_rename", "label": "Rename", "handler": "rename", "priority": "secondary",
          "args": { "workspace_id": { "sel": "sel_ws/id" } },
          "args_schema": { "type": "object", "required": ["name"], "properties": {
            "name": { "type": "string", "minLength": 1, "ui": { "placeholder": "Release planning", "help": "A name for this grouping." } } } } },
        { "id": "act_delete", "label": "Delete", "handler": "delete", "priority": "secondary",
          "args": { "workspace_id": { "sel": "sel_ws/id" } },
          "confirm": { "message": "Delete this workspace grouping. Sessions and Git worktrees stay available. This action cannot be undone.", "tone": "danger" } } ] },
      { "type": "list", "id": "members_current",
        "source": { "kind": "botster-workspaces.membership",
          "where": { "all": [ { "eq": [ { "path": "/workspace_id" }, { "sel": "sel_ws/id" } ] },
                              { "eq": [ { "ref": "r_session/lifecycle_class" }, { "lit": "current" } ] } ] } },
        "refs": { "r_session": { "kind": "session", "key": { "hub_id": "/hub_id", "id": "/session_id" },
                  "absent": { "text": "Session unavailable", "tone": "warning" } , "activate": { "open": "r_session" } } },
        "item": { "fields": [ { "id": "member_title", "value": { "ref": "r_session/label" } } ],
          "actions": [
            { "id": "act_remove", "label": "Remove", "handler": "remove_session",
              "args": { "membership_id": { "row": "/id" } },
              "confirm": { "message": "Remove this session from the workspace?", "tone": "neutral" } },
            { "id": "act_move", "label": "Move", "handler": "move_session",
              "args": { "membership_id": { "row": "/id" } },
              "args_schema": { "type": "object", "required": ["workspace_id"], "properties": { "workspace_id": {
                "type": "string", "ui": { "options_source": { "kind": "botster-workspaces.workspace",
                  "where": { "ne": [ { "path": "/id" }, { "sel": "sel_ws/id" } ] },
                  "value_field": "/id", "label_field": "/name",
                  "empty": { "title": "No other workspace", "body": "Create another workspace before you move a session." } } } } } } } ] },
        "empty": { "title": "No sessions yet" } } ] } ] }
}
```

The "unavailable" group is a second `list` over the same kind with `ne(lifecycle_class, "current")`, or a row whose ref is absent (UI-RF-2). The example passes UI-ID-1 (all ids differ), UI-BD-3 (`ref` inside the part that declares it) and UI-RC-4 (`membership_id` is an ordinary argument; the plugin loads the membership row from its store).

## 15. Mapping of the ten Web items

| Web §I item | Clauses |
|---|---|
| 1 Move session excludes the current workspace | UI-RC-3, UI-BD-3 |
| 2 Action arguments from the row | UI-RC-1, UI-RC-2, UI-RC-4 |
| 3 Session groups with an absent state | UI-RF-1 to UI-RF-3 |
| 4 Spawn form depends on the chosen spawn point | UI-FM-3, UI-FM-4, UI-FM-6 |
| 5 Field presentation hints | UI-FM-2, UI-FM-5, UI-FM-6 |
| 6 Destructive confirmation | UI-AC-3, UI-AC-4 |
| 7 Empty, error and loading copy | UI-CP-1 to UI-CP-4 |
| 8 Master and detail selection | UI-SL-1, UI-SL-2, UI-SL-4 |
| 9 Open a session from a row | UI-SL-3, UI-RF-4 |
| 10 Toolbar grouping and priority | UI-PT-4, UI-AC-2 |

The seven TUI needs: stable ids (UI-ID-1 to 4); a closed set with fallbacks (UI-PT-1, UI-CF-2, UI-CF-3); view state off the wire (P5, UI-SL-1, UI-SL-4); results keyed by node id (UI-ID-4, UI-FM-7); renderer capabilities (UI-CF-1); no timers (scope); no expression evaluation beyond equality and truthiness (UI-BD-1).

## 16. Fakes, vectors and conformance

- **Vectors** (section 12) are the fake: at least one per clause that changes the realized tree, including the ten Web items and ids with `/`, `:`, `|` and `\`; one negative vector per admission rule; `viewport` vectors for all 27 classes.
- **Reference implementation:** the Rust `realize` in the contract crate. Web and any other renderer are checked against the same vectors.
- **Skew vector:** a snapshot where a membership arrives before its session: the ref is `absent` (UI-RF-2) and later resolves without changing node ids.
- **Rule:** a renderer behavior with no clause and no vector is not allowed.

## 17. Amendments to the other contracts (required before freeze)

1. **Hub→plugin VW-6.** Replace "the component set is closed in v1 and is the UI Nodes vocabulary the clients render" with: "the part set is the UI contract's part set (ui-contract section 3): six data parts and seven structural parts; a plugin selects parts and binds them to entity families and actions, and never builds nodes."
2. **Hub→client section 7b.** Remove `dialog` from the layout list (UI-AC-4); name `ui-contract v1` as the owner of the vocabulary; state that capability-dependent filtering does not exist (UI-CF-1).
3. **Hub→plugin VW-1.** Add: "views are admitted under UI-AD-1" and that a view's action schema equals the registered descriptor's (UI-FM-1).
4. **Hub→client DS-1.** `allowed` and `reasons` stay as they are (UI-AC-5). No `confirmed` request field is added (UI-AC-3).

## 18. Changes since draft 1

| Draft 1 | Draft 2 |
|---|---|
| Hub replaced bound args (UI-RC-4) | Hub replaces nothing; client resolves, Hub validates the canonical schema, plugin authorizes (UI-RC-4) |
| Hidden required field dropped from `required` (UI-FM-3) | Hidden fields omitted; they must be optional in the canonical schema; `required_when` is a client pre-check (UI-FM-3) |
| Hub filtered views per client (UI-CF-1) | One shared projection; local diagnostics (UI-CF-1) |
| Refs on a field | Refs on the part, one grammar (section 5) |
| Test-only `realize`; own tree vocabulary | Production `realize` in the crate; output is the UiNode vocabulary with a normative mapping (UI-RZ-2, UI-RZ-3) |
| No viewport input | `viewport` operand and `realize` input; exclusivity by enumeration of all 27 classes (UI-PT-3) |
| Five limits | No numeric limit until a failure is observed (UI-LM-1) |
| Hub enforced `confirm` | Client presentation only; form first, then confirm (UI-AC-3, UI-AC-4) |
| Row focus and selection mixed | Focus is not selection (UI-SL-4) |
| No array/object fallback | UI-CF-3 |
| Ids could repeat; `/` and `:` in node ids | Unique across kinds; opaque escaped node ids (UI-ID-1 to 3) |

## 19. Decisions and open items (freeze record)

**Settled (Sol Hub A, Web 001a, TUI 001b):**
- **Q1:** the realizer output is the existing UiNode vocabulary through the UI-RZ-2 mapping. Plugins author declarations only.
- **Q2 and Q6:** `metric` and `number` are `Field.display` options with the UI-CF-2 fallbacks. No new data part. `metric_grid` stays out unless the user asks for a tile look.
- **Q3:** no user sorting in v1; declared `order` only.
- **Q4:** the plugin publishes computed kinds. The view never computes cross-kind aggregates or unions. A count, a union or a cross-kind derivation is not expressible.
- **Q5 (allowlist):** the projection allowlist of a kind is exactly the `projection_allowlist` field of its `KindDescriptor` (Hub→client DS-1), from the owner's single declaration (DS-2). Schema properties need not all be projected. View admission reads that field, never a client struct.
- **O1:** the TUI spike (botster-tui-kit 6c46910, input.rs) found the router reads drafts, selections and focus but has no setters. The TUI writer adds `set_draft_value`, `clear_draft_value` and `set_selected_row`, the new UiNode props and the `detail` and overlay mappings. This is additive kit work and not a freeze blocker.
- **O2:** the production `realize` is published from the `botster-ui-contract` crate. Tagging and pin updates are later implementation work; this contract names no tag.
- **Gone or new (Web facts):** `bind_list.source "@/field"` and `local:` paths do not exist in v1. `{ form }` operands, password/masked input, collapsed optional sections and dialog nodes are new Web work.

**Open:** none.

---

## Appendix A. Project-pipelines view inventory (informative)

Source: `botster-project-pipelines` `plugin.lua` (`render_home` line 4465, `render_settings` line 4526, `drilldown_tables`, `workbench_dialogs`, `entity_stream_section`), at `3295408`. Today the plugin builds a UiNode tree in Lua (`surface_route`); the Hub contract removes that (EN-6, VW-2).

### Home (`project-pipelines.home`)

| Today | Needs | Maps to |
|---|---|---|
| Overview panel: toolbar plus four counters (needs attention, running, ready for review, open tickets) with tone | A computed count per tone | `detail` over a singleton kind `project-pipelines.summary` with `display: metric` fields and a tone `map`; counters are plugin-published (EN-2) |
| Three lists: Needs attention, Running, Ready for review, each with empty copy | Rows computed from runs, session requests and questions (for example an attention item comes from a blocked step, a failed spawn or an open question) | Three `list` parts over plugin-published kinds `attention_item`, `running_item`, `review_item`; the **plugin computes the rows** and publishes them; the view only lists them. This is why Q4 asks |
| "Common path" guidance: one text plus five entries naming tools | Static copy and entry points | A `text` part plus an `actions` part with real actions (Create project, Create ticket, Create pipeline, Start run, Spawn agent); the five "tool" entries become buttons |
| Records: four tables (projects, tickets, runs, agent spawns) with columns and empty copy; selecting a workspace-linked project focuses it | `table` with columns; a selection that filters the other tables | Four `table` parts; `selects: sel_project`; the other tables filter with `where` on `{sel: "sel_project/id"}` |
| Dialogs: Create ticket (project, title), Start run (ticket, pipeline), Spawn agent (feedback form) | Forms with project/ticket/pipeline pickers today typed as plain text inputs | Three `actions` entries with `args_schema`; the pickers become `options_source` selects over the project, ticket and pipeline kinds (an improvement over free text) |
| "Live records": four bound lists for live refresh | Debug view of entity frames | Dropped: every table is live through watches (VW-4) |
| Selected-workspace note per project | Conditional text | A `text` with `when`-free binding: a `detail` field over the selected project |

### Settings (`project-pipelines.settings`)

| Today | Needs | Maps to |
|---|---|---|
| Five counters (projects, tickets, runs, agent spawns, open questions) | Counts | `detail` over `project-pipelines.summary` with `display: metric` |
| Readiness list, computed from the context and provider status | Computed rows | `list` over a plugin-published kind `readiness_item` |
| Provider dependency status: text and a badge | One computed value | `detail` over a singleton kind with a `status_badge` field |
| Defaults: four read-only entries (spawn point, session type, pipeline mode, workspace) from package configuration | A read-only view of configuration | `detail` over a singleton kind `project-pipelines.settings` holding the values `config.get` returns. Editing configuration is an operator action outside the view (reload the package), as today |

### What the inventory needs from this contract

1. `display: metric` (counters) and tone maps (`status_badge.map`): Q2.
2. Computed lists are plugin-published kinds, not view logic: Q4. No `where` across kinds is needed.
3. Selection that filters sibling tables: UI-SL-1, UI-SL-2.
4. Forms with `options_source` pickers over plugin kinds: UI-FM-4.
5. Empty copy per part: UI-CP-1.

No part beyond the thirteen is needed. The plugin publishes seven computed or singleton kinds (`summary`, `attention_item`, `running_item`, `review_item`, `readiness_item`, `settings`, and the provider status in `settings`), which are plugin work under EN-1 and EN-2, not contract work.
