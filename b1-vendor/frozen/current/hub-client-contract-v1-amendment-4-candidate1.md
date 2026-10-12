# Hub→client contract, Amendment 4 (candidate 1): the `action` field of `req`

Owner: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-09-30. **This file is the amendment, in the post-freeze amendment path.** The base is the Hub→client contract of manifest `final7`, with Amendments 1 to 3, which are unchanged. This amendment cites other contracts by name and clause id only.

**Why.** VB-6 defines `invoke(kind, target?, action, args)`, and MC-2 defines the MCP tool `invoke { ref, action, args, idempotency_key?, operation_id?, descriptor_id? }`. But the EN-1 frame `req { id, verb, kind, target?, args?, idempotency_key?, operation_id?, descriptor_id? }` lists no field that carries the action name. The Foundation build found the gap (steward ruling R-8). The amendment adds no behavior. It writes into EN-1 and EN-8 the field that VB-6 and MC-2 already use.

| # | Clause | Test |
|---|---|---|
| EN-1a | **`req` carries `action?`.** EN-1's request frame is `req { id, verb, kind, target?, action?, args?, idempotency_key?, operation_id?, descriptor_id? }`.<br>- `action` is **required when `verb` is `invoke`** and **absent for every other verb**. A request that breaks either rule is `invalid_arguments`.<br>- `action` is the action's **name**: the key of the kind's `actions` map in `describe` (DS-1). A plugin-contributed action on another owner's kind uses the name under which `describe` lists it (DS-6, for example `<package>.<action_id>` on `session`).<br>- `args` holds only the action's own arguments, validated against its `args_schema` (DS-1). The action name is never inside `args`.<br>- `verb` keeps the closed set of P2. An action name is never a `verb`.<br>- The MCP binding's `invoke` tool (MC-2) maps `ref` to `kind` and `target`, and its `action` to this field. Every binding carries the same field (TB-0). | `conf::en_1a_invoke_requires_action`, `conf::en_1a_action_only_on_invoke`, `conf::en_1a_action_is_the_describe_name` |
| EN-8a | **The idempotency match includes `action`.** EN-8's match is "the key, verb, kind, target, **`action`**, `descriptor_id`, `operation_id` and the normalized args". The same key with a different action is `conflict { reason: idempotency_key_reused }`. | `conf::en_8a_same_key_other_action_conflicts` |

**Not changed:** the verb set (P2), VB-6, MC-2, DS-1, DS-6, the route codec, and every other clause.
