# Integration review: #179 (the Ghostty fork sync of 2026-10-09 with the A6 commits; branch stage1/p2-fork-a6-r2)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate. The fork side is the fork reviewer's (round 4).

## Round 1 — CLEAN on head 338f3ecb

Reviewed head: `338f3ecb20bde1c47db4aa374b7fe987894dbee2`, base v1 `fc23cd97` (15 commits, 22 files, +1,451 -17; code in
`crates/botster-terminal-ghostty`, `.gitmodules` and the submodule; the rest is docs and evidence). The PR's gate log:
`shared/core-stage1/gate-logs/fork-a6-338f3ecb.log` (full Linux gate, exit 0). This reviewer did not read it.

### Not on current v1

v1 is now `d15579da` (#177 and #178 merged after the base `fc23cd97`). `git merge-tree --write-tree origin/v1 338f3ecb`
has no conflict (tree `6deee707`). #178 changed `botster-terminal-ghostty/build.rs`, and this PR changes only
`build_data.rs` in that directory, so the two touch different files. Per the lead's ruling, this CLEAN carries over to a
base-only v1 merge only if P6's `base-merge-check` passes on it.

### Checked, no finding

- **The pin.** `.gitmodules` names `botster/upstream-sync-20261009`, and `git ls-remote` of `trybotster/ghostty` gives
  `39a68e822e505685d1e7fa9c6abeff21125b489c` for it, which is the submodule commit.
- **The ABI against the fork header at `39a68e82`** (`include/ghostty/vt/terminal.h`):
  - Every `sys::opt` and `sys::data` number that the binding uses equals the header's value. Upstream added
    `XT_CHECKSUM_REPORT = 44`, `XT_CHECKSUM_EXTENSION = 45` and `PROGRAM_STATUS = 46`, so `QUERY` is 47 and
    `QUERY_MAX_BYTES` is 48, as `sys.rs` now says. `CLIPBOARD_WRITE_MAX_BYTES = 39` matches.
  - `sys::ClipboardWrite` has the header's field order, with `too_large: bool` and `total_len: u64` after `terminator`.
    `on_clipboard_write` reads them only when `request.size` covers `offset_of!(total_len) + 8`.
  - `CLIPBOARD_WRITE_IO_ERROR = 5` and `SUCCESS = 0` match the header.
- **The A14 decision.** `too_large` is the model's report (step 1, its decoded size) or the contents size over the limit
  (step 2). `set_clipboard_limit` sets the model's decode limit to the same value. The new test derives every size from
  its input and covers the bound, step 2 (an alias), step 1 (a replaced region), and a count over many chunks.
- **Other crates.** At the head, no crate outside `botster-terminal-ghostty` uses `set_clipboard_limit`,
  `ClipboardWrite` or the `sys` numbers, so the renumbering and the new `total_bytes` meaning reach no other crate yet.
  **Carry:** #176 (and so #168) has v1's current pin `3f8eb681` and does not change it, so a v1 merge after #179 takes the
  new pin with no conflict. Any of their code that uses the binding must then build against the new numbers.

Observation (not counted): the sync record gives the Linux shipped-options run as 41/41 steps with 72 skipped, and the Mac
run as 42/42 with 56 skipped. The record does not say why the counts differ. The fork reviewer may want that line.

VERDICT: CLEAN (0 open) at 338f3ecb20bde1c47db4aa374b7fe987894dbee2
