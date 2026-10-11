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

### 016c's REJECT on the same head (package scope; recorded, not repeated)

- 016c R1-1 is the same defect as my F2 part 1 (a comparison taken as a binding). One fix closes both.
- 016c R1-2 (MEDIUM): `defs::output_read` gives a `pump_until` the scope of the whole handle, so a wait on `Activity{s2, Output}` clears s1's pending output. That is no wait on s1 under R-77. My round 1 missed it. I check its fix on the next head: the guard rule uses the wait target's session, keeps outputs whose session the target does not name, and has a regression case.

### R-80 (pending, governs now; steward to me, 2026-10-11)

R-80 replaces R-77's lint. The rule: an output on S, then a model read on S (`ReadScreen`, `CaptureSnapshot`, `terminal_state`, or a guarded `begin`), with no output fence (`Activity{S, Output}`, or a distinct R-57 sentinel) between them, is flagged. `await_quiet`, `drain`, a clock step, `pump`, `let`, `check` and the read's own reply wait are not fences. Each match is fenced or listed in the shrink-only exceptions with a reason. The self-test is red on today's `st_6` (step 7 waits at 5000, step 14 at 5001). Tier HIGH.

How my round 1 findings carry over to a head built on R-80:
- F1 closes when the PR body states HIGH (R-80 confirms the tier).
- F2 and 016c R1-1 become moot if the rule no longer tracks when `model_rev` was bound. Every model read after an output needs a fence, whatever its variables are.
- 016c R1-2 stays relevant: a fence must be `Activity` of the same session S, or S's own sentinel, never another session's wait.
- F3 stays: "after" evidence for every transcript that the PR fences.

### Steward on the round 1 verdict (2026-10-11)

- F1, F2 and F3 are upheld. #54 is HIGH.
- R-80's general output-fence lint goes into #54 and replaces R-77's narrower rule. In scope:
  - the `st_6_capture` fences (steps 7 and 14);
  - the `st_6b_baseline` s8 and `st_6b_graphics` s10 fences;
  - `st_6_pages_owned…` s8, listed in the exception file with a reason.
- **Correction to my R-80 note above:** F2 is not moot. Its binding cases (a matcher comparison of an already-bound `model_rev`, and the `pump_until` and `pump_collect` bindings) apply to the general rule. I review the revised head against R-80 with F2, F3 and 016c R1-2 still open.

### R-80 additions (steward, 2026-10-11): checks for the revised head

- An output-derived event of S (`ModesChanged`, `TitleChanged` or `Bell`) is an output fence only when all of these hold:
  - it ends the wait (a `pump_until`, or a `pump_collect` that ends on it);
  - it is distinct, under R-61 (a)(2): no earlier state can meet it;
  - the sequence that posts it is the LAST bytes of the output step.
- A wait that would consume an event that a later step asserts uses an appended R-57 sentinel instead.
- The exception list is only for matches where no sentinel works.
- The lint takes a named list of the derived events.

## Round 2: head `381e4bb8843063e47929e197be87b0efa4144115`

VERDICT: NOT CLEAN (2 open, both LOW).

Commits since round 1: `72eb31b` (R-80: the rule, addition (1), the fences, the exceptions), `42c64cd` (merge of main `7df311e282260b29fadc143fdd0a97c1645bf1e8`, with conflicts resolved by hand) and `381e4bb8843063e47929e197be87b0efa4144115` (merge of main `16fdb77854c1fd3859e4645797c927dfa1a8195f`, automatic: `git merge-tree` gives the commit's own tree).

### Round 1 findings

- **F1 closed.** The body states HIGH: the lint and the exception file are gate-decision code and data.
- **F2 closed (and 016c R1-1).** R-80's rule tracks no bindings: every model read after an output with no output fence of its session between them is a finding. So a comparison taken as a binding, and a `model_rev` bound by a `pump_until` matcher, no longer matter. Every `model_rev` comes from a model read or a wait, and the read is checked itself. The self-test checks each later read after the same unfenced output.
- **016c R1-2 closed.** `output_fences` takes only an `Activity` of the read's own session. The self-test has another session's `Activity` as a case that is not a fence.
- **F3 closed.** Evidence `~/botster-sessions/shared/core-stage1/evidence/p5-r80/core-tiers-mac.log` (SHA256 `b1c563c555c5f15ba923f37f6eded33c5d084ceac6e69613b096e082e6ad8708`), Core #282 head `d893833eb9dc3762e859c0130e51498bf6db91fe`, both tiers, macOS, own target directory: `in_9_key_repeat…` and `st_6_capture_is_frozen_at_model_rev` PASS before and after on both tiers; `ev_8`'s new fence passes on both tiers; the other ids stop at unchanged Core pending causes or unsupported controls. No id gets worse.
  - Note: the log's header names the contracts tree as `6383bf7f` (a WIP commit that `72eb31b` replaced, not an ancestor of the head) plus uncommitted edits. Those edits are in exactly the 8 files that differ between `6383bf7f` and `72eb31b`. The two must-pass ids' transcripts are not among them, so the after run's `in_9` and `st_6` equal `72eb31b`'s.

### The merges (BUILD Q7)

- `42c64cd` resolved conflicts in the exception file header and in `fence_lint.rs` (the module doc, the exception filter, the tests). The PR's diff against main `16fdb77854c1fd3859e4645797c927dfa1a8195f` removes only three module-doc lines, which the PR rewrites to add the R-80 rule. Every R-83 line of main is kept: `UNFENCED_ROUTE_READ` stays in the exception filter, and its tests pass in the gate.

### R-80 and its additions, checked

1. **The rule.** Outputs: `pty_output`, `pty_chunk`, `program_write_once`. Model reads: `ReadScreen`, `CaptureSnapshot`, a `terminal_state` call, a `begin` with a `model_rev` guard. A read is never a fence. `await_quiet`, `drain`, `pump`, the clock, `let`, `check`, an `ignore` or `forbid` matcher, and another session's wait are not fences.
2. **Addition (1).** `DERIVED_EVENTS` is the named list (`ModesChanged`, `TitleChanged`, `Bell`). A derived event fences only when it is a `pump_until`'s `event` or a `pump_collect` member, and when the last bytes of the session's last unfenced output are the sequence that posts it. Distinctness is stated per fence in the body. See F4 for one gap in the last-bytes test.
3. **The fences (R-77 checks (a) and (b), the body's table).** I checked the claims that need the transcript:
   - `dp_12_focus_report…` step 16 and `ev_8_reply…` step 12 (`Activity` with no `at`): step 15 and step 11 are the first outputs of s1, so no earlier `Activity` can meet them;
   - `st_6` steps 7 and 14 (`at` 5000 and 5001), `st_6b` (`at` 5000): as R-80 states;
   - `ev_5` and `ev_6_keyed` use `await_title` of the output's own title (T23, T5), distinct from every earlier title, and consume no event;
   - `a3_2` and `ev_6_loss` append an R-57 sentinel and `await_title` it, because an `Activity` wait would not be distinct (`a3_2`) or would drop events that a later step collects (`ev_6_loss`).
4. **The exception list.** It removes 5 R-57 entries that the new fences close, renumbers 2 R-60 entries, and adds one R-80 entry, `st_6_pages_owned_last_flag_and_range` step 8, with the steward's reason as a comment above it. Net: 4 fewer.

### F4 (LOW, open): the last-bytes test accepts two `u` sequences that post no `ModesChanged`

- **Evidence.** `ends_with_a_mode_sequence` accepts any CSI whose parameter bytes (0x20 to 0x3F) run to a final `h`, `l` or `u`. With final `u`, that also accepts:
  - `ESC [ u` (`1b5b75`, empty parameters): SCORC, restore the cursor;
  - `ESC [ ? u` (`1b5b3f75`): the kitty keyboard flags query, which answers and changes no mode.
  Neither posts `ModesChanged`. So an output that ends with one of them, followed by `pump_until ModesChanged{S}`, passes the lint's condition 3 ("the sequence that posts it is the last bytes"). The wait would then end only on an earlier event, or time out.
- **Effect today.** None: no derived fence in the corpus follows such an output.
- **Required.** For a final `u`, accept only the kitty forms that change the flags: the first parameter byte is `=`, `>` or `<` (set, push, pop). Add the two cases above as non-fences in the self-test.

### F5 (LOW, open): the PR body says "four reasons"; the file says five

- **Evidence.** Body, section 3: "The header says four reasons." The exception file's header at this head says "Only five reasons exist" (R-56 capture, R-60, R-57, R-80, and main's R-83 route read).
- **Required.** Correct the body. No code change.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-stage1-p5-in9-guard-fence-381e4bb8-pool-20261011-141218-97570.log`, SHA256 `4d3c8d1c5f4263ea450d2568af3252c829056cad215a6ecf2d8799f9a1952dc0`.
- Header: head `381e4bb8843063e47929e197be87b0efa4144115`, base `16fdb77854c1fd3859e4645797c927dfa1a8195f` (BOTSTER_CI_BASE_REF), Linux pool.
- Result (read by me): all 10 steps PASS, with `a_model_read_after_an_output_of_its_session_needs_an_output_fence` and the corpus test; `botster-gate: exit 0 after 441 s on msa1`.
- Main is now `71f01c8812132fa979b8584f12a92ba2efde071e` (#65: `ci/remote/` and a doc only, disjoint from this PR). F4 changes the lint, so the next head needs its own gate. P5 merges main once after both CLEANs, then the final gate.

### Steward's R-80 correction after round 2 (2026-10-11): one more open item

- **F6 (open, by ruling):** `st_6_pages_owned_last_flag_and_range` step 8 is FENCED with an appended R-57 title sentinel, not listed in the exception file. The steward: "A sentinel works there." So the R-80 exception entry and its comment leave the list, and the transcript gets the sentinel and its `await_title`. On the next head I check the sentinel's distinctness (R-61 (a)(2)) and that no later step asserts an event that the fence consumes.

### 016c's round 2 finding on the same head (package scope; recorded, not repeated)

- MEDIUM: `output_fences` (`activities`) recurses through `$one_of`, so it takes an `Activity{S, Output}` in any alternative as a fence. `pump_until {"$one_of": [Activity{s1, Output}, Bell{s1}]}` can end on `Bell` with no output applied, and the lint clears s1. Alternatives that name s1 and s2 clear both. My round 2 missed it. I check the fix on the next head: an alternative of `$one_of` (or any matcher that can end the wait without it) is not a fence, with regression cases.

## Round 3: NOT CLEAN (2 LOW) on 50fd7f4cc1693520d97361d3a8102b9642cd4976

Base main `71f01c8812132fa979b8584f12a92ba2efde071e`. The head fast-forwards main. Merge `65582cd` (main `71f01c88`) is automatic: the recomputed tree `9d68d11dba98e63928dc13f74696db618868df2d` equals the commit's tree. Own delta against `381e4bb8843063e47929e197be87b0efa4144115`: `conformance/core-fence-exceptions.txt`, `conf::st_6_pages_owned_last_flag_and_range.json`, `fence_lint.rs`.

Gate: `~/botster-sessions/gates/botster-contracts-stage1-p5-in9-guard-fence-50fd7f4c-pool-20261011-143020-65365.log` (sha256 `8c7e9ae15b1e52067c50e478444aeb1b36589a9408f99ccf83a902888e5a7bfd`). The header names head `50fd7f4c...` and base `71f01c88...`. Allocation `5cd9f52d`, Linux. manifest, frozen, taint, ledger, rust, slow, codegen, ts, fuzz, mutants: all PASS. Exit 0.

### Closed

- **F4 closed.** `ends_with_a_mode_sequence`: a final `u` counts only when the first parameter byte is `=`, `>` or `<` (kitty set, push, pop). `CSI u` (SCORC) and `CSI ? u` (query) are non-fence cases in the self-test (`421b5b75`, `421b5b3f75`). The `h`/`l` rule is unchanged.
- **F5 closed.** The file header and the body both say five reasons.
- **F6 closed.** Step 6 writes `41` + `OSC 2 ; r80-st6-pages ST` (`1b5d323b7238302d7374362d70616765731b5c`). The OSC is the last bytes. New step 7 `await_title {s1, "r80-st6-pages"}`. The title is distinct: no other step of the id sets or expects a title. No later step asserts a `TitleChanged` or an empty event queue: the later steps are `await_quiet`, the capture's `Completed`, a `let` and four `read_page` calls. The title changes no page content, so the `read_page` expectations do not change. The R-80 exception entry and its comment are gone.
- **016c R2-1 closed.** `required()` gives the sessions that every match requires. A `$one_of` gives the intersection of its alternatives. Other `$` keys give nothing (the conservative side: a missed fence is a finding, never a false clear). `ignore` and `forbid` give nothing. `derived()` reads `/<Event>/id` directly on the event, so a derived event inside a `$one_of` is not a fence either. Tests: a `$one_of` of two s1 Activities is a fence; `Activity|Bell`, `s1|s2` and `$not` are non-fences.

### Open

- **F7 LOW (evidence; 016c raised the same item at LOW; my round 1 F3 requires it):** the Core tier table has no row for `conf::st_6_pages_owned_last_flag_and_range`. `core-tiers-mac.log` runs the contracts tree at `72eb31b`, before `5d0ee7f` changed this transcript. Required: the before/after result of this id on both Core tiers (testkit and real), with the log path and its sha256, in the body.
- **F8 LOW (self-test):** only the `=` form of a kitty `u` is a positive case (`421b5b3d33313b3175`). No self-test has `CSI > ... u` or `CSI < u` as a fence. If `"3e"` or `"3c"` leaves the list, every test stays green. (The gate's mutants do not mutate test files.) Add one positive case for each, and state red on revert.

A delta re-review follows on the next head. If the next head changes only the body and the self-test, the gate on `50fd7f4c` does not carry: the gate must run on the new head.

### 016c's round 3 on the same head (package scope; recorded, not repeated)

- REJECT, one LOW: the same evidence gap as F7. 016c also requires a correction of the body's claim that the fake tier runs every changed transcript: `st_6_pages` is `fake: structural`, so the gate does not execute it. That is why F7 needs the Core tier runs.
- 016c sent the round-limit QUESTION to the steward: an evidence-only review on this head. F8 needs a self-test change, so it needs a new head. The steward rules whether F8 goes into this PR or into a follow-up.

### Steward ruling on F8 (after round 3)

- F8 is **deferred-by-steward**: option (a) is accepted. The code is correct. The gap is two missing positive self-tests: a kitty `u` whose first parameter byte is `3e`, and one whose first byte is `3c`.
- The tests go into a test-only follow-up PR. Its tier is STANDARD, its owner is P5, and 016c reviews it.
- Conditions:
  - P5 opens the follow-up before #54 merges.
  - #54's body names the follow-up.
  - The follow-up is in the same contracts tag as #54 (.42), so no tag ships the lint without those tests.
- F8 does not block CLEAN on `50fd7f4c`. CLEAN on `50fd7f4c` needs F7 closed in the scoped round 4, and the follow-up named in the body.
