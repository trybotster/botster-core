# Contracts #61: integration review (p3/r85-no-0d-computed)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/61 (P3). Package reviewer: 016c. Ruling: R-85 (pending; it governs now). Tier: HIGH (steward).

## R-85 checks for the READY head (steward, 2026-10-11)

- No input-log assertion compares a `0d` byte, whatever the source of the expected bytes: a literal, a `let` value or an `oracle_encode` result.
- The driver enforces this at run time, on every tier:
  - it resolves the expected bytes of `pty_input` assertions, of `route_expect_pty`, and of `hex` checks over an input log;
  - when the resolved bytes contain `0d`, it fails the step with a typed transcript-defect error.
- There is no lint key list (R-7).
- `in_9_application_cursor_and_keypad` asserts `kp_enter` without the input log.

## Round 1: head `115cb4d9f0a27eb9a9bd930dce9fa60c4c9da021`

VERDICT: NOT CLEAN (3 open).

### F1 (MEDIUM, open): a `check` that compares an input log by `eq`, `ne`, `prefix_of`, `suffix_of`, or by `hex_prefix` with the log first, is not checked

- **Evidence.** `input_log_relation_rule` checks only `hex_prefix [prefix, log]` and `hex_ends_with [log, suffix]`. The driver's `check` (`botster-core-conformance/src/driver.rs` 2608-2681) also compares two values by `eq`, `ne`, `prefix_of` and `suffix_of`, and `hex_prefix [log, x]` asserts that the log is a prefix of `x`. Each of these compares the log's bytes with a value from any source.
- The corpus uses one of these forms today. Two transcripts compare a whole captured log by `eq`:
  - `dp_5b_path_is_followed_by_one_space_in_one_paste` step 21: `{"eq": ["$in", "$e"]}`, where `$in` is a `pty_input` `bind_fields` `bytes` (step 20) and `$e` is an `oracle_encode` result (step 18). That is R-85's own case: computed expected bytes compared with the input log.
  - `dp_5b_path_is_pasted_unmodified_with_no_quoting` step 18: `{"eq": ["$in", "$psh"]}`, with `$in` from step 17.
  Neither value holds `0d` today, so no result changes. But a `0d` in `$e` would pass the new check and depend on the tier.
- The PR body says: "in the corpus, a `pty_input` comparison is an `expect` of the whole log. A prefix comparison is a `check hex_prefix` over a captured log. Both are covered." The two `eq` checks contradict it.
- **Required.** For every `check` relation that compares values (`eq`, `ne`, `prefix_of`, `suffix_of`, `hex_prefix`, `hex_ends_with`), when one argument is an input-log variable, check the other argument. Add self-test cases for `eq` (the `dp_5b` form) and for `hex_prefix` with the log first. Correct the PR body.

### F2 (MEDIUM, open): a `one_of` sequence after the one that matches is never checked

- **Evidence.** `expect_pty` (`route/driver.rs` 690-708) calls `pty_matches` for each sequence and returns at the first that matches. `no_0d_expected` runs inside `pty_matches`, for that sequence only. So a `0d` in a later sequence is found only on a run where every earlier sequence fails. On the testkit (no `ICRNL`) an earlier sequence can match, while on the real tier it fails and the later sequence is reached. Then the defect depends on the tier, which R-85 forbids ("on every tier"). The PR body states "`route_expect_pty`, every sequence of `one_of`", which the code does not do.
- **Required.** Check every sequence's expected bytes before any match: resolve the parts of all sequences, run `no_0d_expected` on each, then match. Add a self-test where the first sequence matches and a later sequence holds `0d`.

### F3 (LOW, open): the known bytes that a `written_without_op` part excludes are not checked

- **Evidence.** `no_0d_expected` checks a `WithoutOp` part's `contains` only. But a `written_without_op` part also compares the log with the known bytes of every unplaced `bytes`, `text` or paste run (R-73 follow-up 2 (2)): the span fails if it holds them. A `0d` in such a body is `0a` in the real log, so the span does not hold it, and the part passes when it should fail. That is the case that the steward closed for literal bodies in #53 (my F2 there: the lint flags a `written_without_op` part on a session with a `0d` in any earlier sent body). R-85 extends R-61 to computed sources, and the lint sees only literal bodies.
- **Effect today.** None. No `route_send` in the Core corpus sends a computed body.
- **Required.** When a sequence has a `written_without_op` part, check the known bytes of every run of the read's session. Or state this limit in the PR body and the driver doc, with the reason that the lint covers literal bodies.

### 016c's REJECT on the same head (package scope; recorded, not repeated)

016c sent five MEDIUM findings, with source proofs (`.review-r85-proof/round1.rs` and `route.rs`, five failures). I check each fix on the next head:
- an `await_control` of `pty_input` bypasses the check and the capture tracking;
- a nested matcher in `expect.bytes` (`$one_of`) escapes the check;
- a variable that a later step binds again keeps its input-log status;
- a `let` `field` of `[$log, $bytes_hex]` loses the origin of the unchanged log bytes, so a later check over it is not checked;
- the route `one_of` returns before it checks a later sequence (my F2).

### What I checked and found correct

1. **The typed defect.** `StepError::TranscriptDefect { rule, detail }` is reported by `run.rs` as a FAIL with `expected` `{"transcript_defect": "R-85"}` and the detail. The order vectors give it a name of its own. `input_log_0d` finds `0d` or `0D` at a byte boundary only, and ignores hex of odd length.
2. **The Core `pty_input` `expect`.** The check runs before the control, on the substituted `bytes` (a `$bytes_hex` or a bound hex string). A bare variable that is not bound yet is a capture and is recorded as an input log. The corpus has only these two forms (30 `$bytes_hex` and 246 variables).
3. **The route parts.** `hex` and bound `var` parts, `contains` items, and the known bytes of the runs that a `written_by_op` part names are checked. Key, mouse and focus runs are the stated limit (R-7).
4. **The three `in_9` ids (form (a)).** Each removes the capture and its `hex_ends_with` and keeps `hex_len_eq [$eN, $nN]`, where `$nN` is the write's `pty_bytes_written` bound by the `pump_until` before it. That is R-85's statement for `in_9_application_cursor_and_keypad` (`kp_enter` without the input log). The PR body states that it is a length check only. The removed variables are used nowhere else. The exception list loses the three capture entries, and the later entries of each id move by -2. The corpus test proves that the list and the findings match.
5. **No PASS to non-PASS.** Evidence `~/botster-sessions/shared/core-stage1/evidence/p3-r85/v1-670e5fc/` (Core #288 `622db05f9e24e6a62196a13a43e66e2e060e3c5e` with v1 `670e5fc799a25794910a1847fc41b3e089bf03f4` merged locally, testkit tier, macOS). The six logs' SHA256 values match the PR body. The red run finds the three defects (scan mode), and the green and baseline runs give the same outcome for every id, in strict and scan mode.
6. **Mutants.** The gate's 26 mutants of `botster-conformance`: 24 caught, 2 unviable, 0 missed. The hand-mutant logs (`r85-hand-mutants-macos.log`, `r85-gate-mutants-macos.log`) match their SHA256 values in the body.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-p3-r85-no-0d-computed-115cb4d9-pool-20261011-134852-93031.log`, SHA256 `e0cf77af156e94fe2750cf95091000653e0136546212d877cfcf1a27a14b3dc7`.
- Header: head `115cb4d9f0a27eb9a9bd930dce9fa60c4c9da021`, base `7df311e282260b29fadc143fdd0a97c1645bf1e8` (BOTSTER_CI_BASE_REF), Linux pool. The PR's merge base is `e2c0ad9c842046454be6da7f5015f1d5b8a7dbb1`.
- Result (read by me): all 10 steps PASS; mutants 26 tested, 24 caught, 2 unviable; `botster-gate: exit 0 after 349 s on msa1`.
- F1 to F3 change the drivers, so the next head needs its own gate.

## Round 2: head `7c780843dc9646ea5b14c0a4278aa4d59dba4409`

VERDICT: CLEAN (0 open) on this exact head. Ready for P3's main merge (Q7) and the final gate.

Commits since round 1: `4fe77f9a6a733ee45b630095334925d7d3934497` (the round 1 fixes) and `7c780843dc9646ea5b14c0a4278aa4d59dba4409` (round 2b). No main merge yet: the PR's merge base is still `e2c0ad9c842046454be6da7f5015f1d5b8a7dbb1`.

### Round 1 findings

- **F1 closed.** `input_log_check` runs before every step. For a `check` or an `assume`, when any argument refers to an input-log variable (at any depth), every hex string of every other argument is checked, for every relation and in either order. The two `dp_5b` `eq` checks are covered; by value they hold no `0d`. Test: `a_check_over_a_captured_input_log_with_a_0d_byte_is_a_transcript_defect` (`eq`, `ne`, `prefix_of`, `suffix_of`, `hex_prefix`, `hex_ends_with`, both orders, a nested `eq`). The body is corrected.
- **F2 closed.** `expect_pty` parses every sequence and runs `no_0d_expected` on each before it matches any. Test: `a_later_one_of_sequence_with_a_0d_byte_is_a_transcript_defect_when_an_earlier_one_matches`.
- **F3 closed.** With a `written_without_op` part, the known bytes of every run are checked. The route test has a bare `written_without_op` over runs with a known `0d`, and a run without one that is no defect. The driver doc states it.

### 016c's round 1 items (my record)

- `await_control` of `pty_input` is a `pty_input` read (`pty_input_read`), so it is checked and tracked.
- `no_0d_in` walks the whole `expect.bytes` matcher, so a `$one_of` alternative is checked.
- Input logs are tracked by value: after each step, a variable whose value changed no longer holds a log.
- A `let` whose expression refers to an input-log variable, and whose value is not a number or a boolean, holds the log (an alias, a field, a concatenation).
- The route `one_of` is F2.

### Round 2b: two more ids by form (a)

- With Core #285, `in_9_kitty_each_flag` reaches an Enter check whose expected bytes are `0d`. P3 removes, by form (a), every Enter capture and its `hex_ends_with` where kitty flag 8 is off (Enter is then `0d`): 16 in `in_9_kitty_all_32_flag_combinations_pinned` (flags 0 to 7 and 16 to 23) and 4 in `in_9_kitty_each_flag` (flags 1, 2, 4 and 16). I checked that the removed steps are exactly these captures and checks, and that every Enter `oracle_encode` keeps its `hex_len_eq`. With flag 8, Enter is `CSI 13 u`, and those captures stay.

### Evidence

- `~/botster-sessions/shared/core-stage1/evidence/p3-r85/round2b/`: Core `6401bb04a70cc7723d0bfee9b9ba2f5ecc1914ac` (local: #288 tip, v1 `23550811`, and #285), testkit tier, `BOTSTER_SEEDS=0-31`.
- The red scan has 4 defects (`in_9_application_cursor_and_keypad` s128, `in_9_kitty_each_flag` s15, `in_9_legacy_keys_table` s7, `in_9_kitty_unknown_flag_bits_ignored` s20). The green scan has 0.
- From the full raw outputs (`*.raw.txt`), the per-id outcome of the green run equals the baseline's: 579 ids in scan mode, 227 in strict mode. So no id goes from PASS to non-PASS.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-p3-r85-no-0d-computed-7c780843-pool-20261011-142323-43406.log`, SHA256 `f6c5c069a06036f60597467ad76f1cee00711e7c1f6d615327b64a76448c7e57`.
- Header: head `7c780843dc9646ea5b14c0a4278aa4d59dba4409`, base `71f01c8812132fa979b8584f12a92ba2efde071e` (BOTSTER_CI_BASE_REF, the main tip), Linux pool.
- Result (read by me): all 10 steps PASS; mutants 43 tested, 39 caught, 4 unviable, 0 missed; `botster-gate: exit 0 after 496 s on msa1`.
- Main after the merge base: `7df311e282260b29fadc143fdd0a97c1645bf1e8` (#60: `vocabulary.rs`, `core-testkit-controls.md`), `16fdb77854c1fd3859e4645797c927dfa1a8195f` and `71f01c8812132fa979b8584f12a92ba2efde071e` (#63, #65: `ci/remote/`, a doc). None is a path of this PR, so the merge needs no delta review (Q7). I check that the merge is automatic, and I read the final gate on the merged head.

### 016c's finding after my round 2 CLEAN (package scope; recorded; my CLEAN is not withdrawn)

- MEDIUM (016c, with failing source tests): `input_log_track` loses a log's origin in two `let` forms that `let_step` executes:
  - a self-assignment: `retain` drops `$log` (its value changed) before the `let` check reads `self.input_logs`, so `let $log = hex_concat ["61", "$log"]`, then `hex_ends_with [$log, "0d"]`, is not checked;
  - a chain in one `let`: all derived aliases are computed before any is added, so in `$a = hex_concat [$log, ""]`, `$z = hex_concat [$a, ""]`, `$z` is not tracked.
- My round 2 missed both. I re-check the delta on the next head: the `let` rule reads the input logs from before the step, and it handles the `let`'s variables in order (or to a fixed point), with regression tests for both forms.

## Round 3 (delta re-check after 016c's round 2 MEDIUM): CLEAN on 013bd447a22a1d9d9b769c1af8a7e49f0488fe61

Delta against `7c780843dc9646ea5b14c0a4278aa4d59dba4409`: one commit, `013bd44`. It changes `driver.rs` `input_log_track` and `driver_steps.rs`. No transcript changes.

- **Order.** `input_log_track` now takes the variables of the `let` in the map's order and updates each one in turn. `let_step` iterates the same `serde_json` map (`preserve_order` is on in the workspace `Cargo.toml`), and it substitutes with the updated bindings at each assignment. So the two orders are equal.
- **Self-assignment.** `retain` keeps a variable that this `let` assigns, even when its value changed. So `refers_to` sees the variable's old origin when it reads itself (`let $log = hex_concat ["61", "$log"]`).
- **Same-step alias.** A later assignment sees the origin of an earlier assignment of the same `let`. A variable that is assigned later in the `let` keeps its old origin, which matches `let_step`: that variable still holds its old binding at that point.
- **End of origin.** An assigned variable whose expression reads no log, or whose value is a number or a boolean, loses its origin (`remove`).
- **Tests.** Case A is `$self`, and case B is `$a` then `$z` in one `let`; both are defects. A self-assignment that reads no log is not a defect. Each case is red against the round 2 code: the old `retain` dropped `$self` before the check, and the old code read a snapshot of the origins, so `$z` lost `$a`.
- **Gate.** `~/botster-sessions/gates/botster-contracts-p3-r85-no-0d-computed-013bd447-pool-20261011-144037-1015.log` (sha256 `c0a80a15f7bdbe11e60a9db19c6e7ab9848d802acc59d35e49842a345cc8cf60`). The header names head `013bd447...`. msa1. Ten steps PASS. Mutants: 46 tested, 42 caught, 4 unviable, 0 missed. Exit 0.
- **Core evidence** (`evidence/p3-r85/round3/`). The header names Core `6401bb04a70cc7723d0bfee9b9ba2f5ecc1914ac`, contracts `013bd447...` with no local changes, and `BOTSTER_SEEDS=0-31`. Per-id outcomes (`^test +conf::`, name and result) are the same as round2b: green 227 lines, scan 579 lines. The scan has no transcript defect.

016c closed its round 2 item on this head. Still to do: P3 merges main `71f01c88` (Q7: I check that the merge is disjoint, the tree and the patch-id), and the gate runs on the exact merged head.

## Round 4: delta review of the main merges (Q7: a shared path), no finding. Head 6f1019038b6c349b88d8182636276e6130b3427e

**Merge 1: `5c005fb5208b7d717bee1f6f76b1d28c9880d33e`** = `013bd447` + main `6f32006a32cf15b9d74b26b3fefef98221773264` (#54).
- Merge-base `e2c0ad9c842046454be6da7f5015f1d5b8a7dbb1`.
- The recomputed merge tree `b8bced884844af764c13f930fd32dd89dc51357d` equals the commit's tree, so the merge is automatic.
- The only shared path is `conformance/core-fence-exceptions.txt`. Main changes 16 lines in it: #54's R-80 and in_9 entries. The PR changes the renumbered `in_9` entries and removes its three R-56 entries. The merged file is main's file plus exactly the PR's own lines.
- The gate's corpus test of the fence lint (list = findings) passes on the merged head, so the merged list matches the merged corpus.
- P3's `q7-delta-own-diff-013bd447-5c005fb5.txt` shows that the PR's own +/- lines are the same before and after the merge. The patch-id differs only in the hunk offsets.
- **R-85 on main's transcripts.** #54 changed three transcripts that have input-log steps: `a3_2_stall_changes_no_focus_value_and_posts_no_focus_changed`, `dp_12_focus_report_is_independent_of_focus_reporting` and `ev_8_reply_does_not_move_input_rev`. #54 adds only output fences to them: an OSC 2 title sentinel with `await_title` in `a3_2`, and `pump_until Activity{Output}` in the other two. No input bytes and no input-log assertion change. So the R-85 run-time check gives the same result as in P3's scan, which found 0 defects.
- Gate on `5c005fb5`: `...-5c005fb5-pool-20261011-145415-40044.log` (sha256 `eb4baa6fb1689112f5ce61e5a4726f13ab8df907bcf0b140a631768471a2ca5c`). Ten steps PASS. Mutants: 42 caught, 4 unviable, 0 missed. Exit 0. Base `7c4004a0` (main moved during the gate).

**Merge 2: `6f1019038b6c349b88d8182636276e6130b3427e`** = `5c005fb5` + main `7c4004a0ba6c9734fbe8d2072f68addde2d58596`.
- The recomputed tree `eab57cddb884edb14811370882f41fdcd9f0c3bb` equals the commit's tree.
- Main changed only `docs/steward-rulings.md` (+5, R-87 correction), which is disjoint from the PR. So no delta review is needed (Q7). P3's `q7-5c005fb5-6f101903.txt` says PASS.
- The gate on the exact head is running: `...-6f101903-pool-20261011-150442-77004.log`.

My review has no open finding on `6f1019038b6c349b88d8182636276e6130b3427e`. The final CLEAN line follows when P3 reports that the gate on that exact head is green.

**Merge 3: `8c8d52ef31bf0bb4a2635364c33df7da9ca69473`** = `6f101903` + main `52f1b07fa52198057c12c8e7f9bb3a3058cac7a7` (#67 merged first, by the steward's order).
- Main changed only `crates/botster-core-conformance/tests/fence_lint.rs` (+3). The PR's own diff does not include that path, so the merge is disjoint (Q7). No delta review is needed.
- The recomputed tree `9477630d933d41e5d8311fea8fd85489f8a38724` equals the commit's tree.
- P3 stopped the gate on `6f101903`. The gate on the exact head `8c8d52ef` is running: `...-8c8d52ef-pool-20261011-150934-9754.log`. The final CLEAN follows when it is green.

## Final: CLEAN on merged head 8c8d52ef31bf0bb4a2635364c33df7da9ca69473

- Gate on the exact head: `...-8c8d52ef-pool-20261011-150934-9754.log` (sha256 `7a9fe8a1c9127aeddb6fd932486c2142c03b18e7f471649940455fbd3a736814`, which I recomputed). The header names head `8c8d52ef...` and base `52f1b07fa52198057c12c8e7f9bb3a3058cac7a7`. msa1. Ten steps PASS. Mutants: 42 caught, 4 unviable, 0 missed. Exit 0.
- The PR head on GitHub is `8c8d52ef...`. Main is still `52f1b07f`, and the head fast-forwards it.
- Every finding is closed: F1 to F3 of round 1, 016c's five items, 016c's round 2 MEDIUM (`let` order), and the merge deltas (rounds 3 and 4).
