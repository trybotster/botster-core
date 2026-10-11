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

### 016c's REJECT on the same head (package scope; recorded, not repeated)

- 016c item 1 (MEDIUM) is my F3. 016c's example uses two sessions: r1 on s1 sends a 1-byte paste, and r2 on s2 sends a 5-byte paste, both with op 1. The valid `await_pty` of 1 on s1 then gets a 6-byte finding. One fix closes both. I check it on the next head as 016c asks: open pastes keyed by route and op, a safe floor for the read's session, and a regression test.
- 016c item 2 (MEDIUM): `least > 0` also skips a non-empty sequence whose floor is 0, for example `[{"var": "$e"}]` after a key send, with no `await_pty`. R-83 exempts only an empty sequence. A `var` part counts 0 toward the floor, but the read still needs a fence of its session. My round 1 missed it. I check the fix on the next head: the empty-sequence test is separate from the floor, a non-empty sequence with a floor of 0 needs a valid fence of its session, and a regression test covers it.
- 016c item 3 (LOW): the module comment of `fence_lint.rs` (line 23) still allows exceptions for three rules only, and it does not name R-83.

## Round 2: head `53c44a709a0030a0347d3b1c9e9badb628b320d8` (gate not yet run)

VERDICT: NOT CLEAN (1 open). Lint-only delta: `6d310b968d44046284fdd003aaaebb610d70bcf9` and `53c44a709a0030a0347d3b1c9e9badb628b320d8` change only `fence_lint.rs`. No transcript and no exception changes.

### F1 closed

- A route read now accepts `covers(Some(least))` or `Fence::Bound(Num::Run)` only, so a fixed `let` must cover the floor, as a literal must.
- `a_known_fence_covers_the_floor_of_a_route_read`: a literal one byte below the floor is flagged and one equal to it is clean; a fixed `let` of 1 and of 2 below a 3-byte floor is flagged, and one of 3 is clean.

### F2 closed

- `the_floor_of_a_route_read_counts_each_part` finds the least clean literal fence for each case: `hex` plus `var` = 2, a `bytes` op 3, a `text` op 4, a committed two-chunk paste 3, an aborted paste 1, an op with no known bytes 1, `one_of` takes the least (1 of 3, 1 and 2), and an empty sequence (alone or in a `one_of`) needs no fence.
- Hand mutants (`~/botster-sessions/shared/core-stage1/evidence/p3-am2/f2-hand-mutants-53c44a70-macos.log`, macOS): each of my five changes and each reverted fix is CAUGHT. One line is a SKIP (a pattern miss), and the last line runs that mutant: CAUGHT.

### F3 closed (and 016c item 1)

- `KnownRuns::open` is keyed by route and op, as the driver keys it. `done` is keyed by session, then op, so a read counts only the runs of its own session. The session of a paste is taken at its `paste_begin`.
- Tests: `an_open_paste_is_keyed_by_its_route` (two open pastes of op 1 on r1 and r2 of s1: the r1 floor is 4, not 5) and `a_known_run_of_another_session_does_not_raise_the_floor` (016c's example: s2's 5-byte paste does not raise s1's floor of 1).

### 016c items 2 and 3

- Item 3 is fixed: the module doc names R-83, its rule, and the route read rule's single exception.
- Item 2 is fixed in its first form: `route_pty_least` gives `None` only when a sequence is empty, so a read of `var` parts only is checked. `a_read_of_var_parts_only_needs_a_fence`: no fence is flagged, and `min_bytes: 1` is clean. F4 below is what remains.

### F4 (MEDIUM, open): a 0-byte fence fences a read of `var` parts only

- **Evidence.** The floor of `[{"var": "$e"}]` is 0, and `Fence::Literal(0).covers(Some(0))` is true. So `{"await_pty": {"session": "s1", "min_bytes": 0}}` is accepted as the fence of that read, and so is a fixed `let` of 0. The driver (`botster-core-conformance/src/driver.rs`, `await_pty`) waits for "at least `min_bytes`" bytes, so a value of 0 returns at once and waits for nothing. That is the race of 016c item 2 again, with a fence step that does nothing. The new self-test locks it in: `the_floor_of_a_route_read_counts_each_part` asserts `floor(vec![], read(json!([{"var": "$x"}]))) == Some(0)`, so a fence of 0 is clean there.
- R-83 exempts only an empty sequence. The R-56 capture rule already refuses a fixed value of 0 (`Bound(Num::Fixed(n))` covers a capture only when `n >= 1`).
- **Required.** A known fence covers at least one byte for any non-empty sequence: accept `covers(Some(least.max(1)))` or a `Bound(Num::Run)` fence. Change that assertion to `Some(1)`, and add a case where a fixed `let` of 0 before a `var`-only read is flagged.
- **Effect today.** None. No committed fence has `min_bytes: 0`.

### Gate

- P3 runs the gate on the head that closes F4. I read it before any CLEAN.

## Round 3: head `a85f62afebd2e61209da32b753dccf9b3662b150`

VERDICT: CLEAN (0 open) on this exact head. Merge-ready from the integration review.

### F4 closed (and 016c item 2)

- `a85f62afebd2e61209da32b753dccf9b3662b150` (parent `53c44a709a0030a0347d3b1c9e9badb628b320d8`) changes only `fence_lint.rs`. A known fence of a route read must cover `least.max(1)`; a `Bound(Num::Run)` fence is still the reviewer's check.
- Tests: the `var`-only floor in `the_floor_of_a_route_read_counts_each_part` is now 1. `a_read_of_var_parts_only_needs_a_fence` flags a literal 0 and a fixed `let` of 0, and accepts 1 in both forms.
- Hand mutant "F4 reverted: no minimum of 1": CAUGHT, run on this head (the last line of `~/botster-sessions/shared/core-stage1/evidence/p3-am2/f2-hand-mutants-53c44a70-macos.log`).

### R-84 (1) as corrected (steward draft, `pending-rulings-edits.md` item 2)

- A partial `modes` now merges onto the modes of a fresh terminal of the pinned libghostty, not onto `ModeFlags::default()`. The PR body states the correction. A fresh terminal has `dec_1035`, `dec_1036`, `ansi_12`, `dec_1007` and `dec_7` on.
- None of these changes an unmodified printable key press, a focus report or a bracketed paste. The only scripted mode change in the class C transcripts (`dp_3_route_codec`, `dp_5_no_input_has_a_success_acknowledgement`) turns mouse tracking and focus reporting off. So each explicit override (`{kitty_flags: 0}`, `{focus_reporting: true}`, `{bracketed_paste: true}`) still gives the session's encoding, and the class C counts of round 1 stand.

### Unchanged since round 1

- `c26e8ad152c521d6c249d757fb9acda549a53dfc`..`a85f62afebd2e61209da32b753dccf9b3662b150` changes only `fence_lint.rs`. The 26 transcripts, the 30 fences, the exception list and ou_10 are as I checked them in round 1.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-p3-am2-route-pty-fence-a85f62af-pool-20261011-125506-59684.log`, SHA256 `c082b7048358dd07d8cd4b57b5183845e43dd1f8487fa06e52b26f6b4505b41b`.
- Header: head `a85f62afebd2e61209da32b753dccf9b3662b150`, base `138173e410b0281a9b481669ffce7cfebab9f374` (BOTSTER_CI_BASE_REF), Linux pool.
- Result (read by me): all 10 steps PASS, with the corpus test and the five new R-83 self-tests; `botster-gate: exit 0 after 280 s on msa1`. The mutants step has nothing to mutate (the lint is under `tests/`). The self-tests and the hand mutants cover the lint.
- The gate base is the main tip. Main's only change after the PR's merge base `63128469ce1d57cbef00f2b6549f699a27d058fc` that the earlier gate base did not have is `docs/steward-rulings.md` (R-83, R-84), and `git merge-tree` of the head with `138173e410b0281a9b481669ffce7cfebab9f374` is clean.
- 016c ACCEPT on the same head (`.review-verdicts/r83-route-pty-a85f62af-accept.md`).

## Merged head `5e2080649f5ba010bddfef58f3ee4f9af8cc1a74` (BUILD Q7)

VERDICT: CLEAN (0 open) on this exact head. Merge-ready by fast-forward.

- Two main merges since `a85f62afebd2e61209da32b753dccf9b3662b150`: `eb989a2ea15c29efcaa71741f81068ffbaba0f8f` (main `138173e410b0281a9b481669ffce7cfebab9f374`) and `f1844c006591a56bd66f533142a66e41d920245c` (main `6135cb32cb2e2c2696863c8fc909108e97ff5137`), then `5e2080649f5ba010bddfef58f3ee4f9af8cc1a74` (main `cce568ebdf20f9b8102283b7ac8cd872e01e300d`).
- `git merge-tree --write-tree` of `a85f62afebd2e61209da32b753dccf9b3662b150` and `cce568ebdf20f9b8102283b7ac8cd872e01e300d` gives the head's own tree (`88631e930aa635f49e5360b6434c93f721d86dcc`), so the merges have no hand edits.
- The PR's own diff is unchanged: `git patch-id --stable` of the diff from `63128469ce1d57cbef00f2b6549f699a27d058fc` to `a85f62afebd2e61209da32b753dccf9b3662b150` equals that of the diff from `cce568ebdf20f9b8102283b7ac8cd872e01e300d` to the head (`605ab61712794b04327a0d5c76cfd1610007ad2d`).
- Main's 12 changed paths are disjoint from the PR's own paths, so no delta review is needed (Q7). Main adds one Core transcript (the EV-8 wrong-route query reply). The corpus lint test passes on it in the gate below.
- P3's base-merge check: `~/botster-sessions/shared/core-stage1/evidence/p3-am2/base-merge-check-a85f62af-5e208064.log`, PASS.
- Gate: `~/botster-sessions/gates/botster-contracts-p3-am2-route-pty-fence-5e208064-pool-20261011-130813-91186.log`, SHA256 `d52998ff8d7111aeecb93bb89506cdba6de7969e72ac8a815d4791fc8be642d3`. Header: head `5e2080649f5ba010bddfef58f3ee4f9af8cc1a74`, base `cce568ebdf20f9b8102283b7ac8cd872e01e300d` (BOTSTER_CI_BASE_REF), Linux pool. All 10 steps PASS, with `every_core_transcript_keeps_the_fence_and_line_discipline_rules`; `botster-gate: exit 0 after 157 s on msa1`.
- The base is the main tip and an ancestor of the head, so the head fast-forwards main.
