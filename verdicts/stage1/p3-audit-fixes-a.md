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

## Round 7 — Head f1cee834 (package F50: no test of every_key_state's layout)

Reviewed head: `f1cee83415af0962b83ccadc45b29e006120e443`. Delta `723bc8d5..f1cee834`, one commit: the private layout test
in `encode.rs` is removed; `tests_encode.rs` gains `bits_above_the_five_kitty_flags_change_no_key_bytes`;
`.cargo/mutants.toml` gains two `every_key_state` entries. This reviewer ran no build, test or gate.

- Evidence: focused Linux pool run at this head (`…p3-audit-fixes-a-f1cee834-pool-20261008-215400-6319.log`): fmt, clippy,
  taint, lists, test-budget (788) PASS; mutants 107: 94 caught, 0 missed, 0 timeout, 13 unviable; exit 0.
- `!= 0` to `== 0`: unchanged argument (a reorder; Core 9.3). Accepted.
- `&` to `^` of the kitty flags (column 36): a bijection of the five flag bits plus bits above them; the new behavior test
  shows, through the public encoder, that bits above the five change no byte. Accepted; the test runs in the default tier
  on every head, so a Ghostty pin that reads a sixth bit fails it.
- `&` to `|` or `^` of the mode bits (column 40): the mutant keeps only states with all six key modes on (for `^`, all but
  one state). It is equivalent only if every contract key reaches its worst case with all six modes on. That is a
  property of the pinned libghostty, not of our code, and the entry rests on one run.

#### E2 [MEDIUM] OPEN — The mode-bits exclusion rests on a local-only exploration that no later head reruns

- Location: `.cargo/mutants.toml`, the new entry `encode\.rs:\d+:40: replace & with [|^] in EncoderState::every_key_state`.
- Evidence: the argument cites "a check of 1,960 inputs" with the log
  `~/botster-sessions/gates/botster-core-scratch-p3-f50-explore-f3611957-pool-20261008-215141-2770.log`
  (`EXPLORE inputs=1960 allon_short=0 highbits_differ=0`, exit 0). The code that ran is commit `f3611957`, "scratch: F50
  exploration (local only)"; `git ls-remote origin scratch/p3-f50-explore` finds nothing. So:
  - nobody else can reread or rerun the check;
  - the property depends on libghostty's encoders, and the A6 fork moves the Ghostty pin soon (to upstream `5dc28bb8e`).
    If the new pin makes some key's worst case need a mode off (for example a keypad key with NumLock, where
    `ignore_keypad_with_numlock` on gives the shorter legacy form), the mutant under-bounds IN-9 and nothing fails: the
    mutant is excluded, and `the_key_bound_is_the_longest_sequence_of_any_mode` passes on the real code.
- Required: keep the exploration as a test that asserts the property the exclusion needs (for the same 1,960 inputs, the
  longest sequence with all six key modes on equals the longest over every explicit mode). It runs in the default tier if
  it fits the 2 s budget in the test profile, else in a `slow_*` module of the binding. Cite that test in the entry, in
  place of the scratch log. The A6 pin move then rechecks it.

VERDICT: NOT CLEAN (1 open: E2)

## Round 7 note — E2 confirmed by a counterexample (same head f1cee834)

The P3 package reviewer (F51, MEDIUM) reports a contract input whose worst case needs a key mode off, on macOS: `Char('a')`,
`Alt`, text of 64 `a`, press. With kitty flags off and `alt_esc_prefix` off, libghostty writes the 64 text bytes; with all
six modes on, macOS's legacy Alt prefix writes 2 bytes; the kitty states with Alt do not report the text. So the mutants
of column 40 under-bound IN-9 on macOS, and the entry is not an equivalence. The Linux exploration of `f3611957` could not
see this, because the behavior is native to the macOS build of libghostty.

E2's required change is replaced: the column 40 entry must be removed (no equivalence exists). The mutants then need a
test that kills them where the gate runs (Linux), or, if they are killable only on macOS, a written per-platform argument
with the lead's native rule applied: a test that kills them on macOS and a focused Mac mutation run named in the READY and
the PR. The macOS regression (the input above, bound at least 64 bytes, checked against every explicit mode) is required
in any case. E2 stays OPEN until then; it closes together with F51.

VERDICT: NOT CLEAN (1 open: E2)

## Round 8 — Head 3d5237b2 (E2 with package F51)

Reviewed head: `3d5237b2b1650424f6f23a27c841407014a165aa`. Delta `f1cee834..3d5237b2`, two commits: a macOS regression test in
`tests_encode.rs`, and the rewritten column 40 entry in `.cargo/mutants.toml`. This reviewer ran no build, test or gate.

- Evidence read:
  - Mac, `--no-config` on `every_key_state` (`…3d5237b2-pool-20261008-221247-36284.log`): `696:40` `&` to `|` and `&` to
    `^` both **caught**; missed only `696:73` (`!=` to `==`) and `704:36` (`&` to `^`), both written equivalences.
  - Linux, the same command (`…3d5237b2-pool-20261008-221445-43317.log`): the two `696:40` mutants MISSED, as the entry
    says.
  - Static steps at this head (`…3d5237b2-pool-20261008-221607-45188.log`): fmt, taint, lists, clippy PASS.
- **E2 CLOSED.** The column 40 entry no longer claims an equivalence. It states the macOS counterexample with the pinned
  Ghostty source (`key_encode.zig:642-650`, macOS only), names the killing test
  `the_key_bound_of_an_alt_key_with_long_text_covers_the_states_with_modes_off` (bound at least 64 and equal to the
  maximum over every explicit mode), limits itself to the Linux gate, and says to recheck it at a Ghostty pin move. The
  mutants are killed by a test where the behavior exists, with the focused Mac run as evidence (the lead's native rule).
  The dropped Linux exploration is no longer cited.

#### E3 [MEDIUM] OPEN — `every_key_state` `+` to `*` is "caught" only by the 2 s test limit (a timeout)

- Location: `crates/botster-terminal-ghostty/src/encode.rs:695:36`, `0u32..1 << (KEY_MODE_BITS + KITTY_FLAG_BITS)`.
- Evidence: the mutant makes the range `1 << 30`: the same 2,048 states repeated, so each full key search encodes about a
  billion times. Both focused runs above report it as `TIMEOUT … 20s test`. The gate runs mutants through nextest with the
  default profile, where `slow-timeout = { period = "2s", terminate-after = 1 }` (`.config/nextest.toml`): the test is
  terminated, which fails it, so cargo-mutants counts the mutant as caught (the f1cee834 gate log shows "0 timeout"). The
  mutant is caught by a time limit, not by an assertion. Mutation policy: "A timeout is a finding."
- The mutant changes no bound, only the cost; the cost is what IN-9 exists to bound ("a tiny admission can never expand
  into an unbounded loop").
- Required: make the mutant fail without a time limit. For example, name the state count as a constant with a compile-time
  check (`const KEY_STATES: u32 = 1 << (KEY_MODE_BITS + KITTY_FLAG_BITS); const _: () = assert!(KEY_STATES == 64 * 32);`),
  which makes the mutant unviable, or a public-behavior test that observes the number of states the search visits. Show
  the focused run without a TIMEOUT.
- The same masking can hide a timeout in any crate's gate mutants step. This reviewer sends the lead that general question
  separately; it is not part of E3.

VERDICT: NOT CLEAN (1 open: E3)

## Round 8 correction — E2 reopened (same head 3d5237b2)

Round 8 closed E2 because the column 40 entry "limits itself to the Linux gate". That limit is only a comment. The
`exclude_re` entry is global: a Mac gate (`botster-gate --on mac`, required for macOS-only code) also excludes the two
mutants that the Mac catches, so the exclusion hides a catchable mutant there. The P3 package reviewer's round 97 (F51,
`ab8a051`) records the same point.

#### E2 [MEDIUM] REOPENED — the platform scope of the column 40 exclusion is not mechanical

- Required (with F51): make the scope mechanical, so that the two `696:40` mutants are excluded only where the build
  cannot catch them (for example a platform-conditional exclusion in the xtask's mutants step, or a mutants config chosen
  per target OS), and give the Linux argument for every valid input against the pinned Ghostty source, not one sample.
  The Mac regression test and the Mac catches of round 8 stand.

VERDICT: NOT CLEAN (2 open: E2, E3)

## Round 8 note — E2's requirement narrowed (same head 3d5237b2)

The P3 package reviewer accepts the implementer's proposal for F51: the column 40 mutants leave the global
`.cargo/mutants.toml` and become an exclusion that the xtask's mutants step applies only when the target is not macOS,
with unit coverage of that condition; Mac gates then include both mutants. That reviewer withdraws the universal Linux
proof. This reviewer withdraws it from E2 too: the production search visits every state, so the mutants only reduce
Linux mutation coverage; the macOS build, where the behavior differs, catches them, with the named Mac run (the lead's
native rule). E2 now requires: the mechanical off-macOS scope with its unit test; the exception citing the macOS branch
(`key_encode.zig:642-650`), the Mac regression test and the Mac mutation log; and the PR naming them.

VERDICT: NOT CLEAN (2 open: E2, E3)
