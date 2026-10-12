# Core contract v1.17, Amendment 15 (candidate 1): a bound on a key's `text`

The base is the Core contract of `frozen/current` (manifest final33), which is unchanged except where A15-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-04. Found by the Stage 1 Core lead (audit A3, and the integration review of P3's fix).

**Why.**
- IN-9 bounds every semantic input at `begin`: Core computes "the **worst-case encoded size over every possible mode** (… the longest key sequence of **any** legacy or kitty flag combination, including every modifier parameter and associated text) and refuses at `begin` with `PayloadTooLarge` if it exceeds `CoreLimits.max_paste_bytes`; so a tiny admission can never expand into an unbounded loop or allocation".
- 5.1A defines a key's `text` as "the text the key produces", with no bound of its own. The terminal model (libghostty, the only oracle, R-7) is the only encoder, so Core finds the worst case by asking the model to encode the key in every encoder state. The work of that check grows with `text`. A key whose `text` is just under the paste bound costs hundreds of MiB of encoding work at the default `max_paste_bytes`, inside one synchronous `begin`. So the check that prevents unbounded work is itself unbounded by the contract.
- A key produces a short text in practice: one character or one grapheme cluster. Long text arrives as `Text` or `Paste`, which have their own bounds.
- `CoreLimits.max_query_text_bytes` already bounds the text of a query reply in the same way.

| # | Clause | Test |
|---|---|---|
| A15-1 | **A key's `text` is at most `CoreLimits.max_key_text_bytes` bytes of UTF-8** (default **256**, range 1 to 4,096; a value outside it makes `open` fail `InvalidConfig`). The check comes before any other size check of the key, so the worst-case computation of IN-9 always runs on a bounded `text`.<br>- **A host `WriteInput` `Key`** whose `text` is longer is refused **synchronously at `begin` with `InvalidInput{field: "text"}`**, with no `OpId` (section 9.3, as A7-1 refuses a field out of its stated range).<br>- **A route `key` input** whose `text` is longer is refused `input_refused{too_large}`, with effect none and the route open, as the size bounds of a query reply are refused (EV-8).<br>- A `text` at the bound is admitted, and the worst-case check of IN-9 then applies to it unchanged.<br>- `shifted_key` and `base_layout_key` are each one Unicode scalar (5.1A), so they need no bound of their own. | `conf::a15_1_host_key_text_over_the_bound_is_invalid_input_sync`, `conf::a15_1_route_key_text_over_the_bound_is_too_large`, `conf::a15_1_key_text_at_the_bound_is_admitted`, `conf::a15_1_key_text_bound_out_of_range_is_invalid_config` |

## A15-2. Text replaced

1. **5.1A, `text`:** "optional, the **text the key produces** on the client's keyboard layout" reads "optional, the text the key produces on the client's keyboard layout, at most `CoreLimits.max_key_text_bytes` bytes (A15-1)".
2. **9B, `CoreLimits`:** a new row `max_key_text_bytes`, default 256, "5.1A, IN-9: the largest `text` of one key event; range 1 to 4,096 (A15-1)".
3. **Section 9.3, `InvalidInput`:** its meaning also covers a key `text` over `max_key_text_bytes` (A15-1). It is still sync, with no `OpId`.

Every other clause is unchanged.
