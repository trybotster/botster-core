# contracts #95 — p6/b4-probe-reader (backlog B-4: the probe's control-socket line reader scans each byte once), reviewer 017f

HIGH: the probe is the real-tier program edge of every Core conformance run. Implementer: P6. Package reviewer: 016c.

## Round 1: CLEAN on 09a1dd7eee03635a7334f79c8dc2b36528ddc159

Base: main `32d36828e1a26749b37d8530348147fc4bd01a4b`. Three commits, no merge. The PR changes only `botster-conformance-probe` (`src/control.rs`, `tests/slow.rs`). No transcript, list file, control or doc changes.

- **The behavior is the same.** `serve` handled each complete line in a `while` loop over `pending` after each read. Now `Lines::push` returns the lines that the read completes, and `serve` handles them in the same order with the same code. So the requests, their order, the reports and the `control_failed` exits do not change.
  - The invariant "before each read, `pending` holds no line end" held before as well, so scanning only the new bytes finds every line end.
  - On `EINTR` the old loop found no line, because `pending` held no line end. The new code returns no line.
- **The cost is linear.** `push` scans each new byte once, and copies each line once. When no line ends, `drain(..0)` moves no bytes. The `for` loop is bounded, which fixes the mutants TIMEOUT of `3668c02a`.
- **The steward's condition (the 2 MiB request on both hosts).**
  - Linux (the pool gate on msa1): `one_long_output_request_is_acknowledged_within_the_read_bound` passed in 0.146 s.
  - macOS (`…-p6-b4-macos-09a1dd7e-20261012-113124.log`, HEAD `09a1dd7e`, Darwin 27.0.0 arm64): 0.574 s; `pty_output_written_counts_…` 0.620 s.
- **Before and after** (macOS, the 2 MiB of `pty_output_written_counts_…` sent as one request): main `32d36828` FAILED at 5.03 s; `6956d8dc` passed in 0.51 s. Both logs record HEAD and a one-line scratch diff.
- **Red on revert** (`…-p6-b4-reversal-3668c02a-20261012-111636.log`, HEAD `3668c02a`, `control.rs` from main): the new slow test FAILED at 5.00 s, followed by a restore.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 181 s on msa1, base main `32d36828`; mutants 7 tested, 7 caught; the new unit test 0.002 s: `~/botster-sessions/gates/botster-contracts-p6-b4-probe-reader-09a1dd7e-pool-20261012-112653-48457.log`.
- **The merge.** Main is now `7ed9df37` (R-109 and the final42 publish). That is not rulings-only. The steward merges #94 first and then #95, each with its own base merge and gate. I check each merge's delta per Q7. #95 and #94 share no path (`control.rs` and `slow.rs` against `fence_lint`).

Body note, not a finding: in "Evidence", the macOS bullet and "**Measured before and after**" run together on one line (no line break after the macOS log path).

No finding. CLEAN on `09a1dd7eee03635a7334f79c8dc2b36528ddc159`.

016c's package review: ACCEPT on `09a1dd7e`.

## Final merge receipt: CLEAN on 8dabe804fae15c59428846e4fe086a4953b88109 (the steward's base merge)

- The parents are main `7ed9df37fef76742e73b096e34b554da47afa064` and the reviewed head `09a1dd7eee03635a7334f79c8dc2b36528ddc159`.
- The tree `24768d88fb3e69a6f18a416e1c5be7d69569b582` equals `git merge-tree --write-tree 7ed9df37 09a1dd7e`.
- Main's changes since `32d36828` (final42 `3acaf781`, R-109 `7ed9df37`) touch no path of `botster-conformance-probe`.
- `git patch-id --stable` of #95's own diff is `ff173292…` against both `32d36828` and `7ed9df37`.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 185 s on gaming, base main `7ed9df37`, "ledger ok: 2550 ids"; mutants 7 tested, 7 caught: `~/botster-sessions/gates/botster-contracts-detached-8dabe804-8dabe804-pool-20261012-113517-96541.log`.

CLEAN on `8dabe804fae15c59428846e4fe086a4953b88109`. Main may fast-forward to it if main is still `7ed9df37`.
