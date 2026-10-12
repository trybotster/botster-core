# ui-contract 1.4, erratum 2 (candidate 1): the `Text` props of `empty_state` and `dialog` are the vocabulary's own

Owner: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-09. **This is an erratum in the post-freeze amendment path.** It adds no behavior, no feature and no version: `ui_contract_view` stays `"1.4"`, and the feature stays `ui_1_4`. The base is the ui-contract of `frozen/current` (1.0 to 1.4, with erratum 1), which is unchanged except where this erratum names a replacement. Found by the crate-guard pair's reviewer (the manifest-versus-crate audit).

**Why.**
- UI-NV-6 says: "**Props that may hold `Text`:** `label` (button, `form_section`), `title` and `body` (`empty_state`, `dialog`), and `disabled_reason`."
- `RealizedTree` "is a tree of the existing UiNode vocabulary (`botster-ui-contract` `UiNode`)" (the base, section 9; UI-RZ-2). In that vocabulary:
  - the `empty_state` node's props are `title` and `description`. The part `empty_state` (`title`, `body?`, base section 5) realizes to "the node of the same name" (UI-RZ-2), and its `body` is shown as the node's `description`;
  - the `dialog` node has the prop `title`, and `body` is a **slot** that holds nodes. A slot holds nodes, not a prop value, so it cannot hold a `Text`. UI-RZ-2b and UI-NV-3 fill it with nodes: the form, or the confirmation's message and its `confirm` and `cancel` buttons.
- So UI-NV-6's "`body`" names a prop that neither node has. The vocabulary is the normative output (UI-RZ-2), so the vocabulary's names are right. No copy key of UI-NV-6 is ever realized into a dialog's body. The confirmation's message is authored text (UI-NV-2 `confirm: { message, tone? }`), and its buttons' labels are `button` `label` props.

| # | Clause | Test |
|---|---|---|
| UI-NV-9 | **The `Text` props of `empty_state` and `dialog`.** On the `empty_state` node, the props that may hold `Text` are `title` and `description`. The part's `body` realizes to `description`. On the `dialog` node, the prop that may hold `Text` is `title`. A dialog's `body` is a slot: its nodes carry their own props, and a `Text` there is a `button` `label` (UI-NV-6) or a `text` node's authored string. No other prop of these nodes holds `Text`. | `conf::ui_nv_9_empty_state_description_holds_text`, `conf::ui_nv_9_dialog_title_holds_text_and_body_is_a_slot` |

## Text replaced

1. **UI-NV-6:** "`title` and `body` (`empty_state`, `dialog`)" reads "`title` and `description` (`empty_state`), `title` (`dialog`) (UI-NV-9)".

Every other clause is unchanged.
