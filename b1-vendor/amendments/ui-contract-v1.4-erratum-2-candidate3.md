# ui-contract 1.4, erratum 2 (candidate 3): the `Text` props of `empty_state` and `dialog` are the vocabulary's own

Owner: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-09. **This is an erratum in the post-freeze amendment path.** It adds no behavior, no feature and no version: `ui_contract_view` stays `"1.4"`, and the feature stays `ui_1_4`. The base is the ui-contract of `frozen/current` (1.0 to 1.4, with erratum 1), which is unchanged except where this erratum names a replacement. Found by the crate-guard pair's reviewer (the manifest-versus-crate audit).

**Supersedes candidate 2** (`84e68d87…520c`, commit `e472467`), which the Amendment reviewer rejected (verdict `9157984`): it restricted the children of a dialog's `body` slot to two kinds and claimed that no copy key is ever realized there. A child keeps its own kind's `Text` rules. For example, `realize` places an `empty_state` whose `title` holds the `form_unavailable` key in a form dialog when UI-CF-3 refuses the form. Candidate 2 superseded candidate 1 (`ce2c17f8…b264`, commit `21db794`) before any verdict, for wrong section citations.

**Why.**
- UI-NV-6 says: "**Props that may hold `Text`:** `label` (button, `form_section`), `title` and `body` (`empty_state`, `dialog`), and `disabled_reason`."
- `RealizedTree` "is a tree of the existing UiNode vocabulary (`botster-ui-contract` `UiNode`)" (the base, section 12; UI-RZ-2). In that vocabulary:
  - the `empty_state` node's props are `title` and `description`. The part `empty_state` (`title`, `body?`, base section 3) realizes to "the node of the same name" (UI-RZ-2), and its `body` is shown as the node's `description`;
  - the `dialog` node has the prop `title`, and `body` is a **slot** that holds nodes. A slot holds nodes, not a prop value, so it cannot hold a `Text`. UI-RZ-2b and UI-NV-3 fill it with nodes: the form, or the confirmation's message and its `confirm` and `cancel` buttons.
- So UI-NV-6's "`body`" names a prop that neither node has. The vocabulary is the normative output (UI-RZ-2), so the vocabulary's names are right. The `Text` rule applies to props. A slot holds nodes, and each node in it keeps its own kind's props, including any that may hold `Text`.

| # | Clause | Test |
|---|---|---|
| UI-NV-9 | **The `Text` props of `empty_state` and `dialog`.** On the `empty_state` node, the props that may hold `Text` are `title` and `description`. The part's `body` realizes to `description`. On the `dialog` node, the prop that may hold `Text` is `title`. A dialog's `body` is a **slot**, not a prop: it never holds a `Text` or any other scalar value. Each node in the slot keeps its own kind's `Text` rules (UI-NV-6 and this clause). For example, an `empty_state` child's `title` may hold a copy key. No other prop of `empty_state` or `dialog` holds `Text`. | `conf::ui_nv_9_empty_state_description_holds_text`, `conf::ui_nv_9_dialog_title_holds_text_and_body_is_a_slot` (it includes a body child whose permitted prop holds a copy key, and it checks that a scalar `body` is refused) |

## Text replaced

1. **UI-NV-6:** "`title` and `body` (`empty_state`, `dialog`)" reads "`title` and `description` (`empty_state`), `title` (`dialog`) (UI-NV-9)".

Every other clause is unchanged.
