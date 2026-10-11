# Contracts #57: integration review (p3/am2-route-pty-fence)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/57 (P3). Package reviewer: 016c. Ruling: R-83 (pending; it governs now). Tier: HIGH (steward).

## R-83 checks for the READY head (steward, 2026-10-11)

### The rule and the lint

- A `route_expect_pty` needs an earlier `await_pty` fence on its session, with no write between the fence and the expectation.
- The lint's count is a floor:
  - the `hex` bytes count in full;
  - a relation part counts at its op's known length (`bytes`, `text` or a paste body), or 1 byte when the op's bytes are not known;
  - `var` parts count 0;
  - `one_of` counts its shortest alternative.
- The reviewer checks that each fence names the full written count: a literal total, or the R-74 count that a matcher binds.

### Scope

- All 31 findings in 29 transcripts at `63128469` are fenced in this PR.
- The only exception: `dp_5_full_input_queue`, listed with the reason "rewritten by R-81 (RC)", and only if this PR merges before R-81's.

### Addition (B): a bound `written_bytes` as the fence count

- A `written_bytes` that a route matcher binds from `input_done` or `input_refused` is a valid fence count when all of these hold:
  - it is bound in an earlier step;
  - its route is attached to the same session;
  - the op's clause defines `written_bytes` as the PTY bytes.
- Where a clause gives `written_bytes` another meaning, the transcript derives the PTY count, and the PR body cites the clause for each such finding.
- **Steward correction (2026-10-11): DP-5b is not such a case.** Frozen DP-5b defines `written_bytes` as the encoded PTY bytes of the path paste: the path, the space and any markers, never the file content. So a DP-5b `written_bytes` meets the third condition directly.
- Check `dp_5b_path`: its fence count `1 + $wb11 + 1` is valid only when the PR body names the two extra bytes as other writes. They must not be the space or markers of the paste, because `$wb11` already counts those (016c's note).

### Addition (C): encoder-owned counts

- An encoder-owned count is bound with `oracle_encode` plus a `let` sum. It is never hand-written, and its id stays pending.
- A literal count is allowed only for bytes that are not encoder-owned.
- A class-C id that passes today keeps its form and is listed with "encoder count, oracle_encode (RC)".

## Round 1: head `c26e8ad152c521d6c249d757fb9acda549a53dfc`

VERDICT: NOT CLEAN (3 open).

### F1 (MEDIUM, open): a fixed `let` value below the floor fences a route read

- **Evidence.** `lint` accepts a route read when `f.covers(Some(least)) || f.covers(None)`. For `Fence::Bound(Num::Fixed(n))`, `covers(None)` is `n >= 1`, whatever the floor is. `fold` gives `Fixed` to every `let` that the transcript fixes: `add` of two numbers, `hex_len` or `sum_hex_bytes` of literals. So:
  - `{"await_pty": {"session": "s1", "min_bytes": 1}}` before a `route_expect_pty` of 23 literal bytes is flagged;
  - `{"let": {"$n": {"add": [1, 0]}}}`, then `{"await_pty": {"session": "s1", "min_bytes": "$n"}}`, before the same read is not flagged.
- The literal-log rule of the same lint (the `pty_input` read with a literal `expect`) accepts only `covers(Some(len))`, so a fixed `let` must cover the length there. R-83 says that the fence covers the shortest log that the sequence accepts. A fixed value is known to the lint, so it can and must cover the floor. "A bound count fences as it does for a capture" applies to a run-time count, whose value the lint cannot know.
- **Effect today.** None. I checked the 30 fences: every bound fence is a run-time value (class B `written_bytes`, class C `hex_len` or `sum_hex_bytes` of an `oracle_encode` result, and `add` of those).
- **Required.** A fixed value covers the floor: accept `covers(Some(least))`, or a `Bound(Num::Run)` fence. Add self-test cases: a fixed `let` below the floor is flagged; a fixed `let` equal to the floor is clean.

### F2 (MEDIUM, open): the self-tests do not test the floor; the gate's mutants step does not reach this code

- **Evidence.** The lint is in `tests/fence_lint.rs`. The gate's mutants step reports "No mutants to filter" for this PR, so no mutant tests `route_pty_least`, `KnownRuns` or the new branch of `lint`. The self-tests are the only check of this gate-decision code. The new self-tests are the two changed R-61 cases (a 1-byte hex read with no fence) and `a_written_count_that_a_route_matcher_binds_fences_a_route_read` (bound counts only). The corpus does not test the floor either: no committed fence is below its floor, so a floor that is computed too low still passes. These changes, for example, pass every test:
  - `one_of` takes the longest sequence (`.min()` to `.max()`): no fixture has alternatives of different floors;
  - a `var` part counts 1, or a known `written_by_op` run counts 1 (the `known` lookup is removed);
  - an aborted paste counts its body;
  - a `Literal` fence is accepted below the floor (`n >= len` to `true`): every fixture with a fence uses a bound count.
- **Required.** Self-test cases that fail on each change above. At minimum:
  - a literal fence one byte below the floor is flagged, and a fence equal to the floor is clean;
  - a `one_of` whose alternatives have different floors takes the least;
  - a `var` part counts 0;
  - a `written_by_op` part of a `bytes` op, a `text` op, and a committed paste counts that op's body length;
  - an aborted paste and a relation part of an op with no known bytes count 1;
  - an empty sequence is not flagged.

### F3 (LOW, open): open pastes are keyed by op only, so the "floor" can exceed a run

- **Evidence.** `KnownRuns::open` is a `HashMap<u64, u64>` keyed by op. The doc comment says: "Ops are keyed without their route, so a lint count stays a floor: the shortest run of the op". That is true for `done`, which keeps every run and takes the minimum. It is not true for `open`. With `paste_begin` op 1 on r1, `paste_begin` op 1 on r2, a 4-byte chunk on r1, a 1-byte chunk on r2 and the commit on r1, the lint records one run of 5 bytes for op 1. The r1 run is 4 bytes (bracketed paste off). The lint then asks a literal fence of at least 5 for a read that 4 bytes meet. The driver keys open pastes by route and op (`route/driver.rs`: `open: BTreeMap<(String, u64), ..>`).
- **Effect.** A false finding only (never a false pass), and no transcript does this today.
- **Required.** Key `open` by route and op, as the driver does, with a self-test case. Or keep the key and correct the doc comment to state the limit.

### What I checked and found correct

1. **Every fence names the full written count (R-83, reviewer check).** I read every write between each read and the preceding fence, in all 26 changed transcripts:
   - **Class A (17):** each literal equals the full read. Refused ops and aborted or uncommitted pastes wrote nothing. Paste bodies count alone where bracketed paste is off (default, or set off by a scripted `pty_output`). `dp_5_bytes_and_text` = 3 + 6 + 14 = 23. `ev_8`'s reply `G1s/NjJj` is `1b5b3f363263`, 6 bytes, not encoder-owned. `dp_5_lane_full`: the `LaneFull` write `ab` wrote nothing, so `aa` + 4 = 5.
   - **Class B (3):** each `written_bytes` is bound in an earlier step, on r1 attached to s1, and its clause counts PTY bytes (DP-5 for a `want_result` paste, with markers; DP-5b for the path paste, with the space and markers, never the file content). `dp_5b_path`: `$wb11 + 2`, and the PR body names the two bytes as op 10's `41` and op 12's `42`, not the paste's space or markers.
   - **Class C (10):** each count is `oracle_encode` results summed by a `let`, with no hand-written encoder count. Focus encodes pass `{focus_reporting: true}`, key encodes pass `{kitty_flags: 0}` (an unmodified printable press; no step sets kitty flags), and the paste encode passes `{bracketed_paste: true}`, so each partial `modes` is complete for its input kind (R-84 (1)). Mouse and focus add 0 where step 9 turns their modes off (R-84 (2)). The literal parts are not encoder-owned (`6162`, `cd`, a paste body with bracketed paste off). The dp_12 sums include the focus-out that a detach writes (`$e`, bound before the read).
2. **The lint rule's shape** follows R-83: the read of a route is a read of its session's log; a write to the session clears the fence (`defs::writes`, after the step); the floor counts hex bytes, a known op's shortest run, 1 for another relation part, 0 for a `var`, the least `one_of`; an empty sequence is not checked. `route_written_counts` binds a count only from an `input_done` or `input_refused` matcher on a route attached to a session, and tags it with that session, so a fence of another session is `Invalid` (R-74 condition 2). The self-test covers another session's route, an unattached route and another frame.
3. **The exception list.** One entry is added, `dp_5_full_input_queue… step 28 unfenced route read, R-56`, with the comment that R-83 requires. RC's R-81 PR is not on main, so the entry is the R-83 exception. The two dp_12 `unfenced capture` entries leave the list, because the new route fences at steps 28 and 38 also fence those captures. The header documents the fourth reason. The gate's corpus test proves that the list and the findings match both ways.
4. **No PASS to non-PASS.** On v1 `926dfb62f3f7168990b2a48741946932d84b2fbb`, the 10 class B and C ids and 14 of the 17 class A transcripts are in `core-pending.txt`. `dp_3_unknown_type_closes_route` and `ou_10` are only in `core-real-pending.txt`, so they pass on the testkit tier today. Neither can regress there: the `dp_3_unknown_type` fence waits for 1 byte that its read requires, and the `ou_10` step accepts every frame order that the strict `expect` accepted.
5. **ou_10 step 25** (plan 23bh: "the fix is a P3 contracts commit in the `am_2` contracts PR, with evidence that `input_refused` is still required"). The `expect_eventually` ignores only `output` and `output_deflate`, so any other frame still fails. Evidence `~/botster-sessions/shared/core-stage1/evidence/p3-am2/` (Core `80c5b50cfd4e94efd65b06ea13c045bab91a476f`, contracts `96f48d6d3d7a0e75e752d78433eff92efad0e77e` plus the step, macOS real tier, seed 6): before 10/10 PASS, after 10/10 PASS, and the wrong-op negative 3/3 FAIL at step 25 ("expected op 2, received op 1"). So the step still requires the refusal.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-p3-am2-route-pty-fence-c26e8ad1-pool-20261011-123224-11100.log`, SHA256 `cdf6ec7065694b5812211772ca0fe2a0560b1b446de2e71cfd7a5fba501e1055`.
- Header: head `c26e8ad152c521d6c249d757fb9acda549a53dfc`, base `e026efb2b7214206068e4e7858c6f2c6e8be46e6` (BOTSTER_CI_BASE_REF), Linux pool. The PR's merge base is `63128469ce1d57cbef00f2b6549f699a27d058fc`.
- Result (read by me): all 10 steps PASS, with the corpus test and the new self-test; mutants: none to test (see F2); `botster-gate: exit 0 after 539 s on linux`.
- Main is now `138173e410b0281a9b481669ffce7cfebab9f374`. It adds only `docs/steward-rulings.md` (R-83, R-84) after the gate base, and `git merge-tree` of the head with it is clean. The fixes of F1 to F3 change the lint, so the next head needs its own gate.
