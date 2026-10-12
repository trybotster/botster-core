# Core contract v1.17, erratum 6 (candidate 1): a query reply from the wrong route is refused `wrong_route`

The base is the Core contract of `frozen/current` (manifest final39), which is unchanged except for the reading below. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-11. Found by the Stage 1 Core lead's RC (the worker's route-input design, plan 23bn), ruled in steward ruling R-75 q3. The number 6 is used because Core erratum 5 candidate 1 was replaced by Core Amendment 13 and never published.

**Why.** Core EV-8(c) and the route codec give different reasons for the same case.

- Core EV-8(c) (`core-contract-v1.17.md`, line 238): "a refusal is an `input_refused` with reason `already_replied`, `query_expired`, `unknown_query`, `invalid_input` or `too_large`" (bold in the source), "(effect none). A reply from any route other than the one the worker bound the `query_id` to is `input_refused{invalid_input}`."
- The route codec, revision 37 (`route-codec-v1-extract-rev37.md`):
  - line 26: "**P9 binding (R-3):** a reply from a route that the query was not bound to is refused `wrong_route`; `invalid_input` stays only the route-ending protocol error."
  - line 100: "A `reply_terminal_query` from any other route is refused with `input_refused { reason: wrong_route }`."
  - line 310: "only a refusal is sent, as `input_refused { op?, reason }` with `reason` ∈ `already_replied`, `query_expired`, `unknown_query`, `wrong_route`, `too_large` (non-exhaustive)."

The codec owns the route's wire reasons. Core adopts the codec verbatim (MANIFEST: "Route codec (owned by HC, adopted verbatim by Core)"). The codec's revision-27 R-3 was written to remove exactly this overlap. EV-8(c)'s `invalid_input` also contradicts the codec's own meaning of `invalid_input`: only an unparseable frame ends a route, and a parseable reply on the wrong route is not one. No Core amendment restates this sentence of EV-8(c) (checked: every Core amendment and erratum file in `frozen/current`; none contains `already_replied` or this sentence).

| # | Clause | Test |
|---|---|---|
| E6-1 | **A `reply_terminal_query` from any route other than the one the worker bound the `query_id` to is `input_refused{wrong_route}`, with effect none.** The route stays open. The query stays open for its bound route and for the shadow's fallback, unchanged (EV-8(c), (d)). `invalid_input` is not a reply refusal reason. It stays only the codec's route-ending protocol error. | `conf::e6_1_reply_from_another_route_is_wrong_route_and_the_query_stays_open` |

**Text replaced.** In EV-8(c):

- "a refusal is an `input_refused` with reason `already_replied`, `query_expired`, `unknown_query`, `invalid_input` or `too_large`" reads "a refusal is an `input_refused` with reason `already_replied`, `query_expired`, `unknown_query`, `wrong_route` or `too_large`".
- "A reply from any route other than the one the worker bound the `query_id` to is `input_refused{invalid_input}`." reads "A reply from any route other than the one the worker bound the `query_id` to is `input_refused{wrong_route}` (E6-1)."

Every other clause is unchanged. This adds no number, no type and no wire value: `wrong_route` is already a codec `RefusalReason`.

**The test.** `conf::e6_1_reply_from_another_route_is_wrong_route_and_the_query_stays_open`: two routes r1 and r2 on one session, with r2 the most recently active route. A query goes to r2. A reply on r1 for that `query_id` gets `input_refused{wrong_route}` on r1, and nothing is written to the PTY. r1 stays open. A reply on r2 then is applied, so the query was not consumed by the refused reply.

**Until this erratum is frozen,** ruling R-75 q3 governs: the worker and the transcripts use `wrong_route`.
