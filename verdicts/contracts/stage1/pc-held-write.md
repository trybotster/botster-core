# Contracts #52: integration review (stage1/pc-held-write)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/52 (PC). Tier: HIGH (stated; correct: pinned transcripts, the probe and the shared fence lint).

## Round 1: head `06acbd8221efaf1e91c049809920d3141d6e1fb5`

VERDICT: NOT CLEAN (1 open).

### F1 (LOW, open): the `kernel_bytes` row calls it an argument of FakeCore's `pty_blocked`

- **Evidence.** `docs/core-testkit-controls.md`, the new row "`pty_blocked` with `kernel_bytes`": "An optional argument of FakeCore's `pty_blocked` (steward ruling R-74) ... FakeCore is frozen and does not take it". The two sentences contradict each other. R-74 puts the argument on the testkit, "Core's edge, which is not frozen", and says that FakeCore "gets no buffer and no `pty_queued`".
- **Required.** Name the owner correctly, for example "An optional argument of `pty_blocked` on the Core testkit and the real tier (steward ruling R-74)". No other text changes.

### What I checked and found correct

1. **The probe (R-74 "The probe's mode").** A failed `tcgetattr` or `tcsetattr` now exits `TERMINAL_SETUP_FAILED_EXIT` (71), with the call and the errno on stderr, before the first step. `Request::Queued {}` answers `Report::Queued { bytes }` from `ioctl_fionread` on stdin. It does not change the gate and reads nothing. The wire tests cover both forms.
2. **F (R-74 "The held write").**
   - F = 40960 = 2 × 20480 (Linux 6.12, 1-byte writes), the larger of the two measured capacities (macOS 1022).
   - F is at most the write limit: `max_paste_bytes` is "one write's payload" (Core limits table, `PayloadTooLarge`), default 1 MiB. With the second write in flight (a2_2 s3: F + 5; in_7: F + 1), IN-5's `input_retained_bytes` (2 MiB) is not reached.
   - F is the named constant `HELD_WRITE_FILL_BYTES`. No transcript asserts a kernel count.
3. **The fence (R-74).** Each of the six transcripts: `pty_blocked {on: true, kernel_bytes: 1}`, a fill of F bytes of one repeated byte, then `await_control pty_queued` with `$between [1, F]`. The upper bound can never fail, because at most F bytes are written before the fence. So it asserts only `≥ 1`, as the ruling says. `held_write.rs` checks exactly these six ids, `kernel_bytes: 1`, `requires pty_queued`, structural, the fill and the bound.
4. **Per-line rulings.**
   - `in_2_partial`, `in_6`: `pty_blocked off`, then `await_pty $p`, before `pty_input`. `in_2_partial` keeps `0 < p` and adds `p < F` (accepted).
   - `in_2_link_loss`: the held form; `lose_worker` and `Unknown` do not change.
   - `in_7`: `$w1` holds F bytes, and `$w2` waits behind it. The R-74 addition `await_get {id: s1, expect: {state: {Lost: $any}}}` sits between the `pump_collect` (step 9) and the `Remove` barrier, cited `Core IN-7`, `Core LC-7`. It pumps and never polls.
   - `am_2` part 2: `$w3` holds F bytes `67`, and `$w4` (`6c`) waits behind it. The order still holds: a `$w4` completion before `$w3`'s would make step 20 fail, never pass. `ends_with 6c` and `prefix 67` do not change. Part 1 does not change. The s8 read fence was already on main.
   - `a2_2` s1: `$one_of [TooLate, Admitted]`, and s7 `Written` 1/1 stays strict. s2: `$p` for both counts, and the log is the F-prefix of length `$p`. s3: the AM-2 form in R-74's seven steps. The filler byte `78` is not in `$w3`'s bytes `6162636465`, so a prefix of the filler proves that none of `$w3`'s bytes were written.
   - No other `pty_blocked` user changes.
5. **The lint case (R-74).** `written_counts` takes a variable at `pty_bytes_written`, or at `payload_bytes_written` only for a payload that is raw `bytes`, from the `Completed` of a `WriteInput` that an earlier `begin` bound. A count fences only the same session's `await_pty`. A `let` sum keeps its session, and sums of two sessions fence neither. The self-test has a positive and a negative case for each of the three conditions.
6. **The exception list.** Exactly 3 R-56 entries leave: `in_2_partial` step 8, `in_6` step 8, `am_2` step 18 (3, as the lead's correction says). The R-60 entries of `a2_2`, `am_2`, `in_2_partial` and `in_6` move to the new step numbers only; I checked each new number against the first write of its session. No entry is added.
7. **Frozen fakes.** FakeCore and the fake route endpoint do not change. `in_2_partial`, `in_6`, `in_2_link_loss` and `am_2` become `"fake": "structural"`, as R-74 rules (the R-60 form). The replacement map and the README are regenerated to match.
8. **No PASS to non-PASS** (evidence `~/botster-sessions/shared/core-stage1/evidence/pc-held-write/`, Core v1 `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2` plus the Core harness forms, macOS):
   - before (`first-*`): real tier FAILED for all six; testkit ok for all six;
   - after (`summary-after.txt`, contracts `9d8c13805f08b5c823654dd8527c86645df38048`): testkit ok for all six; real tier 3/3 ok for five; in_7 0/3 without the R-74 addition;
   - in_7 with the addition (`in7-fence-real-1..3.log`): 3/3 ok. The evidence patch cites `Core LC-6`, and the head cites `Core LC-7`. Only the citation differs, so the evidence applies to the head.
   - Linux real-tier evidence comes from the Core PR's gate. R-74 keeps the ids in `core-real-pending.txt` until then.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-stage1-pc-held-write-06acbd82-pool-20261011-105935-52606.log`, SHA256 `301bddd438db816ba4fb78eb6091906cab04f87cece997493d3c2c2e823a4cd5`.
- Header: head `06acbd8221efaf1e91c049809920d3141d6e1fb5`, base `8d79b70d1d68966911a844c73bdbc4bdcd8f85f0` (BOTSTER_CI_BASE_REF), Linux pool. The PR's merge base is the same commit, so this head already holds main.
- Result (read by me): all 10 steps PASS; `pty_queued_counts_the_unread_bytes_and_reads_none_of_them` PASS on Linux; mutants 2 tested, 2 caught; no FLAKY line; `botster-gate: exit 0 after 357 s on linux`.
- A docs-only fix of F1 gets a delta review. The merge rule still needs a green gate on the new exact head.

## Round 2 (delta): head `4b7c2bb044b7d0e74b31d04cc0b3e9d1517ec08f`

VERDICT: CLEAN (0 open) on this exact head. The gate on this head is still to come; I read it before I send CLEAN to the lead.

- The parent is the reviewed head `06acbd8221efaf1e91c049809920d3141d6e1fb5`. The delta is one line in `docs/core-testkit-controls.md`: "FakeCore's" becomes "on the Core testkit and the real tier" in the `kernel_bytes` row. **F1 is closed.**
- The rest of the row does not change. "Real tier: ignored, because the kernel buffers" agrees with R-74 ("On the real tier, n is ignored") and with the probe section of the same doc.
