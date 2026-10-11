# Contracts #54: integration review (stage1/p5-in9-guard-fence)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/54 (P5). Ruling: R-77 (pending; text in `shared/steward/drafts/pending-rulings-edits.md` item 4).

## Round 1: head `174cba2c43a5260412cdda00230dd75a1eb2d1e4`

VERDICT: NOT CLEAN (3 open).

### F1 (HIGH, open): the stated tier STANDARD is wrong; the PR is HIGH

- **Evidence.** The PR body says "Risk tier: STANDARD ... a test-only lint rule". The fence lint (`fence_lint.rs`, `every_core_transcript_keeps_the_fence_and_line_discipline_rules`) runs in the gate's `rust` step and fails CI on every transcript that breaks a rule. So it is "code whose result decides pass or fail" (BUILD.md, risk tiers, rule 1). Contracts #49, #52 and #53 changed the same lint and were HIGH. BUILD.md: "A wrong STANDARD tier is a HIGH finding, and the PR moves to HIGH."
- **Required.** The PR body states **HIGH** and the rule (rule 1: the fence lint decides pass or fail in CI). The HIGH rules then apply: every finding closes before CLEAN, and two reviews (016c and this one). No code change is needed for F1.

### F2 (MEDIUM, open): the guard rule misses two `model_rev` bindings, and its self-test locks one of them in

- **Evidence.** `model_rev_vars` reads only `expect` and `bind_fields`, and `lint` sets `revs[var]` at every step where the variable appears there.
  1. *A comparison is taken as a new binding.* The matcher (`botster-conformance` `matcher.rs:255-269`) compares a variable that is already bound. It binds only an unbound one. So in `[bind $m, pty_output s1, call terminal_state expect {model_rev: $m}, begin guard $m]`, `$m` keeps its value from before the output, and the race of R-77 is still there. The lint moves `revs[$m]` to step 2 and does not flag the `begin`. The self-test case "a variable bound again after the output" (`json!([bind("$m"), out("s1"), bind("$m"), guarded("s1", "$m")])`) asserts exactly this miss as clean.
  2. *Matcher bindings outside `expect` are not read.* A `pump_until`, `pump_collect` or `expect_eventually` matcher can bind `model_rev`. Eight `pump_until` steps in the corpus do (`st_1_*`, `st_6_capture_is_frozen_at_model_rev`). None of them is a guard today, so the corpus result is right, but a later transcript of this shape escapes the rule. R-77 says "a `model_rev` bound before that output", with no limit on the binding form.
- **Required.**
  - In a matcher position (`expect`, and the matchers of `pump_until`, `pump_collect` and `expect_eventually`), record a binding only for a variable that is not yet bound, as the driver does. `bind_fields` and `let` always rebind, and every other replacement form forgets the variable.
  - Self-test: the second `expect` case becomes a finding. Add a clean case where `bind_fields` rebinds after the output, and a flagged case where a `pump_until` matcher binds the variable before the output.
  - State in the PR body that the corpus result does not change (or list any change).

### F3 (LOW, open): no "after" evidence on Core's tiers

- **Evidence.** The PR shows the "before" failures (Core #282's gate, seed 0; plain v1 `b6fd4b98`, seed 7) and the fake tier pass. It shows no testkit or real-tier run of the changed transcript. My brief requires no PASS to non-PASS from the PR's before/after evidence. The wait can only add time, but if `Activity{Output}` were not posted on a tier, step 6 would end at `step_timeout`.
- **Required.** One of these:
  - an "after" run of `in_9_key_repeat_is_checked_by_the_guards_once_at_start` on the testkit and the real tier (macOS), with Core patched to this transcript, with the log path; or
  - the exact Core gate log lines where `in_10_terminal_guard_refuses_after_a_redraw_before_start`, whose steps 1 to 6 are now the same as in_9's, passes on both tiers, plus a statement that the Linux "after" comes from Core's pin-move gate, as for R-56 PR A.

### What I checked and found correct

1. **R-77 (1), the fence.** Step 6 is `pump_until Activity{id: s1, source: Output}`, cited `Core 6.2`, in the form of `in_10_terminal_guard_refuses_after_a_redraw_before_start`. No other step changes. `requires` stays `pty_output`.
2. **R-77 check (a).** Steps 0 to 4 send no output to s1 (`Create` with `hold`, `Start`, its completion, `pump`, `terminal_state`). Step 5 is the first `pty_output`, so no earlier unpolled `Activity{Output}` of s1 can meet the wait. No title sentinel is needed.
3. **R-77 check (b).** Later steps assert `Completed` of `$w` (step 8) and `$w2` (step 12), the `terminal_state` (step 10) and a `check` (step 13). None asserts an event that the wait consumes.
4. **The lint rule's shape** matches R-77's text: a `pty_output`, `pty_chunk` or `program_write_once` on S (any handle), then a `begin` on S whose guard names a `model_rev` variable bound before that output, with no waiting step on S between them. A wait is the R-57 sweep's own classification (`defs::output_read` with no fence needed) or an `await_title` of S. The rule takes no exceptions, and the exception list does not grow. Red on revert: with the old in_9, the corpus test fails with exactly one finding.
5. **The corpus grep** (4 guarded begins after an output; 3 `in_10` ids fenced; `in_9` fixed) agrees with R-77's statement that only `in_9` is unfenced.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-stage1-p5-in9-guard-fence-174cba2c-pool-20261011-111351-12967.log`, SHA256 `b1e83679db288ba5b8829173d9aa407208df1e32d4041ec9ad591295958922c7`.
- Header: head `174cba2c43a5260412cdda00230dd75a1eb2d1e4`, base `8d79b70d1d68966911a844c73bdbc4bdcd8f85f0` (BOTSTER_CI_BASE_REF), Linux pool. The PR's merge base is the same commit.
- Result (read by me): all 10 steps PASS; the new self-test PASS; `botster-gate: exit 0 after 442 s on msa1`.
- The fixes of F2 change the lint, so the next head needs its own gate.
