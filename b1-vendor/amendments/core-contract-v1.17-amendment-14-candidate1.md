# Core contract v1.17, Amendment 14 (candidate 1): `clipboard_bytes` bounds every byte the model decodes for one clipboard write

The base is the Core contract of `frozen/current` (manifest final31), with Amendment 13, which are unchanged except where A14-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-04. Found by the Stage 1 Core lead (audit A6, issue #148), after steward ruling R-32.

**Why.**
- 9B says `clipboard_bytes` is "the largest clipboard write Core carries (a memory bound; a larger one is reported, never dropped silently)". EV-3, as A13-2 replaces it, calls it "a memory bound, not a filter".
- The pinned terminal model (libghostty, the only oracle, R-7) decodes an OSC 5522 write as a transaction of chunks. Its own per-transaction decode limit (option 39) is 64 MiB by default. Core can set that limit. R-32 makes a write over the limit call the clipboard callback, with no contents and the length the model counted.
- The model's limit counts **every decoded byte of the transaction**, including bytes that a later chunk of the same MIME type replaces. A13-1 measures `total_bytes` as "the sum of all representations' byte lengths", which is the final value. A13-1b makes `SUCCESS` the outcome when that sum is within `clipboard_bytes`.
- So, to keep A13's exact outcome, Core must leave the model's limit at 64 MiB, and the model may buffer 64 times the stated memory bound. If Core sets the limit to `clipboard_bytes` instead, a transaction that decodes 1 MiB and then replaces it with 1 byte is refused. A13 would then require `SUCCESS` for a write the model did not carry.
- A memory bound should bound memory. This amendment makes `clipboard_bytes` bound what the model decodes, so that Core can set the model's limit to it.

| # | Clause | Test |
|---|---|---|
| A14-1 | **A write is measured by the bytes the model decodes for it.**<br>- **The decoded size** of a clipboard write is the number of bytes the terminal model decodes for it, over every representation and every chunk of the write. It includes bytes that a later chunk replaces. For a write in one sequence (OSC 52, OSC 1337 `Copy`, or an OSC 5522 write without replaced chunks), the decoded size equals the sum of the final representations' lengths.<br>- **Within the bound:** when the decoded size is at most `CoreLimits.clipboard_bytes`, the write is posted with its `contents`, and `total_bytes` is the sum of the final representations' lengths (A13-1). The OSC 5522 acknowledgement is `SUCCESS` (A13-1b).<br>- **Over the bound:** when the decoded size is over `clipboard_bytes`, the write is posted with `contents: none` and `reason: TooLarge`, and **`total_bytes` is the decoded size**. So `total_bytes` is always over `clipboard_bytes` in a `TooLarge` write. The OSC 5522 acknowledgement is `IO_ERROR` (EIO).<br>- **Core may set the model's own decode limit to `clipboard_bytes`.** Then the model never holds more than `clipboard_bytes` of one write. A write over it reaches the worker as R-32 requires: the callback is called with no contents and the decoded size, and the model sends no acknowledgement of its own.<br>- **Example**, with `clipboard_bytes` = 1 MiB: one OSC 5522 write sends `text/plain` of 1 MiB, then `text/plain` again of 1 byte, which replaces the first. The decoded size is 1 MiB + 1 byte. The write is `ClipboardWrite{contents: none, total_bytes: 1048577, reason: TooLarge}`, and the acknowledgement is `IO_ERROR`. | `conf::a14_1_decoded_size_counts_replaced_chunks`, `conf::a14_1_too_large_total_bytes_is_the_decoded_size`, `conf::a14_1_within_bound_total_bytes_is_the_final_sum` |

## A14-2. Text replaced

1. **A13-1, "Size":** "`total_bytes` is the sum of all representations' byte lengths. Over `CoreLimits.clipboard_bytes`, …" reads with A14-1: the bound applies to the decoded size, and a `TooLarge` write's `total_bytes` is the decoded size.
2. **A13-1b, the status:** "`SUCCESS` (DONE): the write's total bytes are within `clipboard_bytes`" reads "the write's decoded size is within `clipboard_bytes`", and "`IO_ERROR` (EIO): the write is over `clipboard_bytes`" reads "the write's decoded size is over `clipboard_bytes`" (A14-1).
3. **9B, `clipboard_bytes`:** "the largest clipboard write Core carries" means the largest decoded size (A14-1).

Every other clause is unchanged.
