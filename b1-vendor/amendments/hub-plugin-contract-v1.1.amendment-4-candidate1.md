# Hub→plugin contract v1.1, Amendment 4 (candidate 1): what `write_pending_bytes` bounds, and the refusal when it is full

The base is the Hub→plugin contract of `frozen/current` (manifest final23): v1.1 joint J4e, with Amendments 1 to 3, which are unchanged except where A4-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the hp-surfaces pair, through the Stage 0 lead (`conf::sw_5_pending_is_bounded`).

**Why.** SW-5: "After acceptance the Hub owns the pending text, in memory, bounded (`write_pending_bytes`, a Hub parameter). The Hub keeps at most one pending write per session." No clause says what the bound counts, or what a write that would exceed it gets.
- A single write's text is already bounded by `max_write_bytes` (SW-7), refused as `invalid_request{text_too_long}`, and there is one pending write per session.
- So `write_pending_bytes` can only be meaningful as the total over all sessions.

| # | Clause | Test |
|---|---|---|
| A4-1 | **`write_pending_bytes` bounds the Hub-wide total of pending text.**<br>- **The total:** the sum, over every session with a pending safe write, of that write's text bytes after sanitizing (SW-7). A write being typed (past the gate, SW-6) counts until it completes.<br>- **Refusal:** a `sessions.safe_write` whose acceptance would make the total exceed `write_pending_bytes` is refused **`backpressured{detail.code: write_pending_full}`**, with effect `none`: nothing is accepted, and no pending write changes. It is retryable, and capacity returns when pending writes complete, are replaced, are cancelled or are abandoned (SW-2, SW-5, SW-6).<br>- **Replacement counts net:** a write that would replace a session's pending write (SW-6) is checked against the total **without** the text it replaces.<br>- **The per-write bound is unchanged:** text over `max_write_bytes` is still `invalid_request{text_too_long}`, checked first.<br>- On the client surface (HC A.9b, the `safe_write` action), this refusal is `busy`, as the other `backpressured` answers are. | `conf::sw_5_pending_is_bounded`, `conf::a4_1_over_total_is_backpressured_write_pending_full`, `conf::a4_1_replacement_is_checked_net`, `conf::a4_1_capacity_returns_when_a_pending_write_completes` |

## A4-2. Text replaced

1. **SW-5:** "bounded (`write_pending_bytes`, a Hub parameter)" reads with A4-1.
2. **SW-1 and Appendix A, the `sessions.safe_write` row:** the refusals gain `backpressured(write_pending_full)`, beside the `backpressured(no_receipt_capacity)` that A2-12 already lists.

Every other clause is unchanged.
