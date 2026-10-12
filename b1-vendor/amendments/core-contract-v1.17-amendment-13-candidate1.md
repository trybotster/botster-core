# Core contract v1.17, Amendment 13 — candidate 1 (clipboard writes as the terminal model reports them)

The base is the Core contract of `frozen/current` (manifest final29). It is unchanged except where A13-2 names a replacement. Other contracts are cited by contract name and clause id only.

**Supersedes Core erratum 5 candidate 1** (`1f91606c…76e9`, commit `21f4461`). The Amendment reviewer rejected it (verdict `9bab339`): OSC 5522 can target the primary selection and carry several MIME representations, and the erratum defined neither. The answer needs a change to the event's shape, so it is an amendment.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. The facts come from the Stage 1 Core lead's P2, at `trybotster/ghostty@85a8d8eb197c5752887c017c9a3faa6f1dc1969b`, `include/ghostty/vt/terminal.h`.
- One normalized callback serves OSC 52, OSC 1337 `Copy` and OSC 5522.
- It reports `location` (standard clipboard, selection or primary), the exact OSC 52 `selection` string (empty when the program gave none, and empty for the other protocols), and `contents`: every representation `{mime, data}` of one logical value, already decoded, to be committed atomically. A `contents` of length 0 means "clear the destination".
- The callback must reply synchronously. Returning without a reply denies the write. A reply reaches the program only for OSC 5522 (DONE, EPERM, ENOSYS, EBUSY, EINVAL, EIO); OSC 52 and 1337 discard it.

| # | Clause | Test |
|---|---|---|
| A13-1 | **`ClipboardWrite{id, selection, contents, total_bytes, reason?}`** is posted for every clipboard write the terminal model recognizes (R-7; EV-7: no opt-in, no filter).<br>- **`selection`:** a non-empty OSC 52 selection string as the program wrote it. Otherwise the model's `location`: standard clipboard → `c`, primary → `p`, selection → `s` (OSC 52's own letters).<br>- **`contents`:** a list of `{mime, bytes}`, every representation the program sent of the one logical value, in the model's order, already decoded. A consumer commits them atomically or not at all. **An empty list means "clear the destination"**, which is distinct from one entry whose `bytes` is empty (a write of an empty value).<br>- **Size:** `total_bytes` is the sum of all representations' byte lengths. Over `CoreLimits.clipboard_bytes`, the write is surfaced with `contents: none`, `total_bytes` and `reason: TooLarge` (EV-3's rule, never silent).<br>- **The reply to the program** (OSC 5522 only, because the other protocols discard it): Core has no clipboard policy and cannot ask the host inside the model's synchronous callback (P7), so the reply reports **delivery to the host, not the platform result**. It is `SUCCESS` (DONE) when the write is surfaced with its `contents`, and `IO_ERROR` (EIO) when it is surfaced as `TooLarge`. Core never answers DENIED, UNSUPPORTED or BUSY by its own policy. The host's later action on the event, or a client's on the raw bytes, is the platform write.<br>- The bytes of every such sequence stay in `Output` (OU-12). **EV-7's one-actor rule applies unchanged:** a host that forwards `Output` to a client that acts on these sequences does not also act on, or forward, the `ClipboardWrite` for that client. | `conf::a13_1_every_recognized_clipboard_write_is_surfaced`, `conf::a13_1_selection_from_osc_52_string_or_location`, `conf::a13_1_all_representations_one_event_atomic`, `conf::a13_1_empty_contents_is_clear_distinct_from_empty_value`, `conf::a13_1_too_large_counts_all_representations`, `conf::a13_1_osc_5522_reply_success_or_eio` |

## A13-2. Text replaced

1. **Section 6.2, the `ClipboardWrite` row:** `ClipboardWrite{id, selection, bytes, total_bytes, reason?}` "OSC 52 write (EV-3)" becomes `ClipboardWrite{id, selection, contents, total_bytes, reason?}` "a clipboard write the terminal model recognizes (EV-3, A13-1)".
2. **EV-3:** "Clipboard writes (OSC 52) are surfaced" becomes "Clipboard writes are surfaced (A13-1)". Its `bytes: none` becomes `contents: none`.
3. **Core erratum 5 candidate 1** is withdrawn and never published.

A clipboard **read** is unchanged: it is OSC 52 only, a terminal query (EV-8), and its selection keeps EV-8's rule (`s0` when none). Every other clause is unchanged.
