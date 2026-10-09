# Integration verdict — PR #167 (P3 audit fixes, part A: A3, A30, A53, F33)

#167 is cut from #163 (`stage1/p3-audit-fixes`, integration rounds 1-7 in `verdicts/stage1/p3-audit-fixes.md`). It
carries the parts with no real-process test code, because of the lead's HOLD (2026-10-08). #163 keeps part B: A31's real
driver and count test, A52 (integration I1), A8, A11, F25-F28 and F39/G2.

## Round 1 — Head d823a40e

Reviewed head: `d823a40ea57f5622dfcab8e6198837dca13a3557` on `stage1/p3-audit-fixes-a`, base v1 `a22811b6` (current).
Four commits (A3, A30, A53, F33). This reviewer ran no build, test or gate.

- **Provenance.** Of the 21 changed files, 19 are byte-identical to #163's reviewed head `40b63dce` (host `admit.rs`,
  `flows.rs`, `run.rs`, `DESIGN.md`, host tests; binding `encode.rs`, `lib.rs`, `tests_encode.rs`; testkit `program.rs`,
  `worker.rs`, `worker/tests.rs`; worker-core `drain.rs`, `lib.rs`, `worker.rs`; `xtask/src/test_budget.rs`;
  `.cargo/mutants.toml`). The two others:
  - `Cargo.lock`: one line, `botster-terminal-ghostty` in `botster-core-host`'s dependencies (A3; the same entry as in #163).
  - `botster-core-sys/src/payload.rs`: only `reap` = `drop(self)` (A53), identical to #163's `reap`. #163's A52 change
    (`ExitWatchFailed`, no invented exit) is not here; it stays in part B. v1's `wait_unreaped` keeps `Code(-1)` on ECHILD.
- **Interfaces checked again on this base:**
  - Host → binding (A3, I2 option b): the IN-9 worst-case bound comes from libghostty; the host now depends on
    `botster-terminal-ghostty`. Same as reviewed.
  - Worker-core `report` (A53): an exit is no longer held for a later hello; with no ready link it is not sent. Every report
    follows the first hello, and reconnection is P5's (the comment says so). P5's adoption work must add the held-exit path
    if it reconnects a worker; this is recorded for P5's later PR, not a finding here.
  - Testkit → worker-core `Drain` (A30): the testkit edge takes a bound when `DrainPty` arrives and emits `PtyDrained` only
    for an asked drain. This is the audit's A30 fix.
  - F33/I7: the slow filter is `test(/(^|::)slow_/)` (as closed in #163 round 7).
  - `mutants.toml`: the `report_exit` `||` and `check_payload` `Focus` entries are removed with the code they named; the
    `reap` entry's reason is rewritten for `drop(self)`.
- **HOLD:** no real-process test code is added (no file under `tests/` of `botster-core-sys`, `botster-worker` or
  `botster-core` changes).
- **Evidence:** focused Linux pool run at this head (`…p3-audit-fixes-a-d823a40e-pool-20261008-211218-39117.log`): fmt,
  clippy, taint, lists, public-api, prebuild-worker, test-budget (757 tests) and the slow-feature clippy, exit 0. It ran
  no slow tier and no mutants; the landing gate does.

#### D1 [LOW] OPEN — `Drain`'s doc says that the real driver uses it; at this head only the testkit does

- Location: `crates/botster-worker-core/src/drain.rs:1-2` ("one decision for the real driver and the testkit edge, so both
  drive the `Worker` through the same drain"); the real driver is `crates/botster-worker/src/main.rs:236-253` and
  `:312-326` (`drain_left = pending_output()`, count only, no flushing read).
- Evidence: `git grep 'Drain\b\|drain::'` at this head finds `Drain` used by the testkit only. `Drain` also carries the A31
  flushing read (count, then read until nothing, then at most one more count). So at this head the testkit edge reads
  late-flushed output before `PtyDrained`, and the real driver does not (A31, open in part B). The disclosed state is
  acceptable under the HOLD, but the shared crate's doc states the opposite, and the in-process proof covers a drain that
  the binary does not have yet. BUILD.md: "A worker behavior that differs between the two is a bug in an edge."
- Required: make the doc true at this head. State that the testkit edge uses `Drain` now, and that the real driver adopts
  it in #163 part B (A31; until then its drain is the count only). Name the open issue (A31) in the PR body and keep that
  issue open when #167 merges.

VERDICT: NOT CLEAN (1 open: D1; package verdict not read)

## Round 2 — CLEAN on head c95b4d76 (D1 fix)

Reviewed head: `c95b4d7646fb5388ce6421bcc4baa708c73685e9` (the implementer's first READY mistyped it as `c95b4d7616`;
corrected by the implementer; this SHA is from `git rev-parse origin/stage1/p3-audit-fixes-a`). Delta `d823a40e..c95b4d76`,
one commit, the module doc of `drain.rs` only. Base v1 `a22811b6` (current). This reviewer ran no build, test or gate.

- **D1 CLOSED.** The doc now says that the testkit edge drives `Drain` now; that the real driver adopts it with A31 (#163
  part B); that until then the real driver reads only the count, without the flushing read; and that this is the open A31
  defect, with the BUILD.md rule cited. This matches `main.rs` at this head.
- The PR body (`gh pr view 167`, head `c95b4d76`) has the section "Open after this PR: A31": A31 (#156) stays open, and
  its issue stays open when #167 merges.
- Evidence: static Linux run at this head (`…p3-audit-fixes-a-c95b4d76-pool-20261008-211852-47753.log`): fmt, taint, lists
  PASS, exit 0. The round 1 focused run at `d823a40e` covers the code; this delta is a comment.
- The landing gate (slow tier and mutants in-diff) is still owed on this exact head.

VERDICT: CLEAN (0 open) at c95b4d7646fb5388ce6421bcc4baa708c73685e9

## Round 3 — CLEAN on head 4592ba9c (package F49, the D1 class in the testkit)

Reviewed head: `4592ba9c4e5657ef0d5856b6b55af40f2471c9bb`. Delta `c95b4d76..4592ba9c`, one commit: comments and one test
name in `botster-core-testkit/src/worker.rs` and `worker/tests.rs`. This reviewer ran no build, test or gate.

- The edge's drain field doc, its readiness comment and the test doc no longer say that the edge drains as the real driver
  does. The test is renamed `the_edge_drain_is_bounded_by_the_asked_count`; its body is unchanged.
- Mechanical check of the class (`git grep -i 'real driver'` in the testkit, worker-core and the worker binary at this
  head): the remaining hits are `drain.rs:1-4` (true since round 2), the spawn-order comment (`worker.rs:207`), the socket
  flush comment (`worker.rs:455`) and `command_line.rs:1`. None of them is about the drain.
- Evidence: Linux run at this head (`…p3-audit-fixes-a-4592ba9c-pool-20261008-212212-55802.log`): fmt, taint, lists,
  clippy PASS; the renamed test passes (1 run, 1 passed); exit 0.
- The landing gate on this exact head is still owed.

VERDICT: CLEAN (0 open) at 4592ba9c4e5657ef0d5856b6b55af40f2471c9bb

## Round 4 — CLEAN on head b7085535 (merge of v1 0b0eecc, #142)

Reviewed head: `b70855356aa243996edabf8bfb3c9f6d2c2f5db6`, parents `4592ba9c` (round 3 CLEAN; P3 package CLEAN `1cd0487`)
and `0b0eecc0` (= `origin/v1`, #142 merged). Tree `ba1aa02ad88aaecd248f50acfb80e91389c67d67`, equal to this reviewer's
`git merge-tree --write-tree origin/v1 4592ba9c`. No edit in the merge. This reviewer ran no build, test or gate.

- `a22811b..0b0eecc` adds only #142's guardian-core crate and its two shared-file entries (14 files, +2,310).
- `git diff origin/v1 b7085535` touches the same 21 files as the round 3 diff. 19 blobs are identical to `4592ba9c`. The
  two files that both sides changed, `.cargo/mutants.toml` and `Cargo.lock`, have the same added and removed lines as the
  round 3 diff.
- Cross-package: guardian-core has no `slow_` module, so F33's filter selects nothing new. Guardian-core does not depend on
  the host, the binding, the testkit, worker-core or sys, so A3, A30 and A53 do not reach it.
- Evidence: static Linux run at this head (`…p3-audit-fixes-a-b7085535-pool-20261008-213359-76310.log`, base `0b0eecc0`):
  fmt, taint, lists PASS, exit 0. The ONE landing gate on this exact head is owed.

VERDICT: CLEAN (0 open) at b70855356aa243996edabf8bfb3c9f6d2c2f5db6

## Round 5 — Head 7be3184c (mutants fixes after the red gate at b7085535)

Reviewed head: `7be3184c09dc4c77e46414cddcd15428ccd3aa6d`. Delta `b7085535..7be3184c`, three commits, five files. The ONE
Linux gate at `b7085535` (`…p3-audit-fixes-a-b7085535-pool-20261008-213644-81473.log`) was red at mutants only (17 missed,
all in part A code); the implementer did not re-gate it. This reviewer ran no build, test or gate.

- **New tests (default tier, no real process):**
  - `drain.rs` `a_read_moves_the_drain_by_the_bytes_it_found`: every `after_read` step of the public `Drain`.
  - `encode.rs` `the_key_states_are_every_combination_once_with_the_text_states_first`: 2,048 distinct key states, only the
    five kitty bits, and the text states first in each run of 32.
  - `tests_encode.rs`: the key search under every limit below the worst case, and the mouse bound of one notch.
  - Host `a_write_in_flight_when_the_link_fails_is_unknown`: a `Focus` write in flight is `Unknown` with the longest focus
    report as its bound (IN-7). This crosses host and binding through the public `longest_focus_report`.
  - No production code changes in the delta.
- **Evidence:** focused Linux pool run at this head (`…p3-audit-fixes-a-7be3184c-pool-20261008-214331-91507.log`, base
  `0b0eecc0`): fmt, clippy, taint, lists, test-budget PASS; mutants 110: 97 caught, 0 missed, 0 timeout, 13 unviable; exit
  0. This is not the landing gate (no slow tier).
- **The two equivalence entries.** Both arguments are correct, but each rests on a fact that the entry does not cite:
  - `payload_size` `/` to `*`: `repeat` is refused at 0 earlier (`admit.rs:326`), so `limit / times` is defined. A key with
    `longest > limit / times` has `times * longest > limit`, so the stop point decides only which length over the limit is
    found. The code (`PayloadTooLarge`) is the same; only the number in the detail and the search cost change.
  - `every_key_state` `!= 0` to `== 0`: a bijection of the 64 key-mode combinations; the kitty part and the text-first order
    are unchanged (the new test still passes under the mutant). Again only which over-limit length is found changes, and
    so only the detail.

#### E1 [LOW] OPEN — The two equivalence entries do not cite why a different error detail is not behavior

- Location: `.cargo/mutants.toml`, the new `payload_size` entry and the new `every_key_state` entry.
- Evidence: both entries say that the mutant changes the size found (and so the number in `PayloadTooLarge`'s detail
  string, `admit.rs:394-397`). A detail is observable by a caller. The mutation policy accepts an equivalence "argued in
  writing"; the argument is complete only with the contract fact that makes the detail not behavior: Core 9.3
  (`core-contract-v1.17.md:318`): "The code is the contract; the detail is for humans."
- Required: cite Core 9.3 in both entries (the code is the same, `PayloadTooLarge`; the detail is not contract). Optionally
  name the earlier `repeat` refusal in the `payload_size` entry, which makes `limit / times` defined.

VERDICT: NOT CLEAN (1 open: E1)

## Round 6 — CLEAN on head 723bc8d5 (E1 fix)

Reviewed head: `723bc8d5c937d905a918a8063deedaaf5326b8c7`. Delta `7be3184c..723bc8d5`, one commit, comments in
`.cargo/mutants.toml` only; the two regex lines are unchanged. The head contains `origin/v1` `0b0eecc0`. This reviewer ran
no build, test or gate.

- **E1 CLOSED.** Both entries cite Core 9.3 (`core-contract-v1.17.md:318`, "The code is the contract; the detail is for
  humans"): the code stays `PayloadTooLarge`, and only the size in the detail can differ. The `payload_size` entry also
  names the repeat-0 refusal (`admit.rs:326`).
- Evidence: static Linux run at this head (`…p3-audit-fixes-a-723bc8d5-pool-20261008-214836-98013.log`): fmt, taint, lists
  PASS, exit 0. The round 5 mutants run at `7be3184c` covers the code; comments do not change the mutant set.
- The ONE Linux landing gate on this exact head is owed.

VERDICT: CLEAN (0 open) at 723bc8d5c937d905a918a8063deedaaf5326b8c7
