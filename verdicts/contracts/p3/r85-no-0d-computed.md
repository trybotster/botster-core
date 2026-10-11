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
