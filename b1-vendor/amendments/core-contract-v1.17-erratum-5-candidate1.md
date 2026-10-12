# Core contract v1.17, erratum 5 (candidate 1): every clipboard write the terminal model recognizes is a `ClipboardWrite`

The base is the Core contract of `frozen/current` (manifest final29), which is unchanged except for the reading below. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the Stage 1 Core lead's P2 (libghostty at fork `85a8d8e`).

**Why.** EV-3 is titled "Clipboard writes (OSC 52) are surfaced", and section 6.2 describes `ClipboardWrite` as "OSC 52 write (EV-3)". The pinned terminal model (libghostty, the only oracle, steward rule R-7) recognizes more clipboard-write sequences than OSC 52, such as iTerm2's OSC 1337 `Copy` and OSC 5522. It reports them all through one callback that does not name the sequence. EV-7 makes transparency the rule: "A feature that reaches the worker and does not reach the host is a conformance failure", with no opt-in and no filter.

| # | Clause | Test |
|---|---|---|
| E5-1 | **`ClipboardWrite` is posted for every clipboard write the terminal model recognizes**, not only OSC 52. Which sequences are clipboard writes is the model's decision (R-7). In the pinned model they include OSC 52, OSC 1337 `Copy` and OSC 5522. Core adds no filter, and it needs no record of which sequence a write came from.<br>- **`selection`:** for an OSC 52 write, it is the program's selection string, with `s0` when the program gave none (the same rule as EV-8's typed replies). For a write by any other sequence, it is `c`, the system clipboard that those sequences write.<br>- `bytes`, `total_bytes` and the `TooLarge` rule of EV-3 are unchanged.<br>- The bytes of every such sequence stay in `Output` (OU-12). **EV-7's one-actor rule** applies unchanged: a host that forwards `Output` to a client that acts on these sequences does not also forward the `ClipboardWrite` for that client to act on. | `conf::e5_1_osc_52_write_is_clipboard_write`, `conf::e5_1_other_recognized_clipboard_writes_are_clipboard_write`, `conf::e5_1_selection_default_for_non_osc_52_is_c` |

**Text replaced:** EV-3's title "Clipboard writes (OSC 52) are surfaced" reads "Clipboard writes are surfaced (E5-1)". Section 6.2's `ClipboardWrite` row "OSC 52 write (EV-3)" reads "a clipboard write the terminal model recognizes (EV-3, E5-1)". Every other clause is unchanged.
