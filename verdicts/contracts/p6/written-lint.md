# contracts #94 — p6/written-lint (the fence lint's written-output rule, R-107 follow-up R107-SI-1), reviewer 017f

HIGH (rule 1: a fence-lint change is a gate-check change). Implementer: P6. Package reviewer: 016c. The steward assigned the integration review to me. The rule logic is 016c's package scope. I check the rule against the ruling, the wiring, the corpus and the evidence.

## Round 1: CLEAN on 9dbb526c4dc536e62b45cd8a72739a9d94054215

- **The ruling.** R107-SI-1 asks for a lint of the sweep's shape: a `pty_output`, then a `Stop` or a program exit before the next await of an output-derived fact, and then a check of that output's effect. The steward split the lint from the sweep on main `f1ccacfd`. `written.rs` uses the sweep rule that the lead accepted in #91 (its outputs, fences, ends and checks), and it counts an absence check as a check (the steward's answer on #91).
- **The wiring** (`fence_lint.rs`): `mod written`, `written::lint` in the corpus test, and `UNWRITTEN_OUTPUT` in the list of rules that take exception lines. The module doc names the new rule. No transcript and no list file changes.
- **The corpus.** The gate's corpus test passes with no new exception line, so main gives 0 findings. My own scan of `conformance/core/` for every end of a session after an output of that session gives:
  - two ends whose last output is fenced by `await_title`: `e3_1_eventless_effect_overtakes_a_carried_or_parked_step` (step 18 waits for the R-57 sentinel in the same bytes as the output) and `ev_2_mandatory_and_keyed_never_dropped` (step 68 waits for `TB`, the output's own title). In both, the awaited fact comes from the last output;
  - six ends with an unfenced output and no later check (`a2_3` ×2, `dp_5_every_not_written…`, `e3_1_due_effect…`, `ou_2b` ×2), which the rule correctly does not flag;
  - no `await_title` of a session after its end, and no session with output whose program has `exit` or `signal_self`.
- **The reversal with the unfenced transcripts** (`…-p6-written-lint-reversal-9dbb526c-20261012-110826.log`, HEAD `9dbb526c`): the rule gives exactly `ev_2` step 67, `ev_2_retired` step 55 and `ev_5_released` step 60, the three cases of the R-107 sweep. `ev_2` step 67 also gets the older R-57 finding, from #89's removed exception line. That is another rule's finding, so it is expected.
- **Red on revert, per check** (`…-check-reversals-9dbb526c-20261012-111158.log`): each stub turns its listed tests red, as the body's table says. The two `unheld` wall-clock self-tests also fail in each local Mac run. The body says that they fail on the Mac and pass in the pool gate, and they pass in the gate.
- **The A23 transcript** (#93) has `await_control pty_output_written` before its `Stop`, so it gives no finding when #93 and this rule meet on main.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 446 s on msa1, base main `6a8ba018`. The 8 self-tests take ≤ 0.004 s each, and the corpus test takes 0.806 s: `~/botster-sessions/gates/botster-contracts-p6-written-lint-9dbb526c-pool-20261012-104727-18303.log`.
- **The merge (Q7).** `9dbb526c` merges main `6a8ba018` into `d8bd9e97`. The tree `dd743954…` equals `git merge-tree --write-tree`. Main is now `32d36828`. `git diff 6a8ba018 32d36828` changes only `verdicts/amendments.md`. That is not rulings-only, so the steward makes and gates the base merge, as for #91, and I check its delta per Q7.

### Notes for 016c (package scope, not findings of mine)

Three limits of the rule that the corpus does not reach today. Each one lets a case pass with no finding (none gives a false finding):
1. A fence is any `await_title` on S, or any wait for an output event of S. It also counts when the awaited fact comes from an output before the last one. For example: an output of title A, then an output of a BEL, then `await_title A`, then `Stop`, then a check of the `Bell`.
2. `checks` does not count an `await_title` of S after the end, although that step reads an output-derived fact.
3. `ends` counts a program exit only when a `WriteInput` triggers it. A program exit that a route's input triggers is not counted.

No finding. CLEAN on `9dbb526c4dc536e62b45cd8a72739a9d94054215`.

016c's round 1 (REJECT on `9dbb526c`): four MEDIUM package findings (F1 a `pty_output_written` count below the total; F2 a forbidden event or a stale title counted as a fence; F3 a route read of another session, and `await_title` after the end not counted as a check; F4 a scripted exit that route input triggers not counted). My three notes are in F2-F4.

## Delta round: CLEAN on 635c014acd96e5119c2129ebf15cac888eba6398

- **The delta.** One commit, `635c014a`, on `9dbb526c`. It changes only `fence_lint/written.rs`. No transcript, list file or wiring changes. The rule logic is 016c's scope. My integration checks:
  - a fence is now a `pty_output_written` wait with a literal count (it covers the outputs whose running total is at most that count), or a required title observation of a title that the covered output itself sets. A `Bell` or other event no longer fences;
  - a check now includes `await_title` on S, and a route read counts only for the route's own session;
  - an end now includes route input (`route_send`, `route_send_raw`, `route_send_wire`, `route_flood`) to a session whose program can exit.
- **The corpus.** The gate's corpus test passes with no new exception line (0.952 s). The corpus fences that I listed in round 1 still hold under the stricter rule: `ev_2_retired` s57 `{bytes: 16}` and `ev_5_released` s62 `{bytes: 25}` equal their running totals, and the `await_title` fences of `e3_1` and `ev_2` wait for a title that the last output sets. The A23 transcript (on main since final42) has `{bytes: 14}`, equal to its total. Its readiness title comes from a `print`, and its `pty_output` sets another title. This head does not contain main `3acaf781`, so the gate on the steward's base merge is the gate that lints the A23 transcript with this rule.
- **The reversal with the unfenced transcripts** (`…-p6-written-lint-reversal-635c014a-20261012-112331.log`, HEAD `635c014a`): exactly `ev_2` s67, `ev_2_retired` s55 and `ev_5_released` s60, then a restore.
- **Per-check reversals** (`…-check-reversals-635c014a-20261012-112404.log`, HEAD `635c014a`): 16 stubs. Each one turns at least one `written` self-test red, and the 5 stubs of the fence and title paths also turn the corpus test red. The only other failures are the two `unheld` wall-clock tests on the Mac.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 197 s on msa1, base main `3acaf781`: `~/botster-sessions/gates/botster-contracts-p6-written-lint-635c014a-pool-20261012-113007-64586.log`.
- **Main** is now `7ed9df37`. The steward makes the base merge, with its own gate and my Q7 receipt.

Note for 016c (package scope, not a finding): the running total is kept per session name for the whole transcript. The probe's count starts again at 0 in a new probe process (after `Restart`, or after a `Create` that reuses a removed name). There the lint covers fewer outputs than the probe proves. That can give a false finding, and it cannot hide a case. No corpus transcript has a `pty_output_written` wait after such a restart.

No finding. CLEAN on `635c014acd96e5119c2129ebf15cac888eba6398`.

016c's round 2 (REJECT on `635c014a`): F1, F3 and F4 closed; F2 open (a title inside an output covered the bytes after it, and an old title fenced a new A-then-T output); new F5 (MEDIUM), from my per-probe count note.

## Delta round 2: CLEAN on 2682b631b14375de611df7e6d3b05bd9681fa317

- **The delta.** One commit, `2682b631`, on `635c014a`. It changes only `fence_lint/written.rs`.
  - F2: a required title covers the outputs up to the first output that holds it, and it covers that output only when the output ends with the title. A title equal to the last proven title (or the program's `print` title) covers nothing.
  - F5: each `Create` of a name builds a new `Session` with a count of 0, and `Restart` resets the count, the unfenced outputs and the title to the program's `print` title. Adoption keeps the count, because adoption keeps the probe process.
- **The corpus.** The corpus test passes with no new exception line (0.938 s). Each corpus title fence ends its output: `ev_2` s67 (`OSC 0 TB ST`), `ev_2_retired` s122 (BEL, then the sentinel, which ends it), `ev_1` s10 (`OSC 0 T3 ST`) and `e3_1` s17 (the sentinel ends it). The A23 transcript's `{bytes: 14}` is its total in one probe.
- **The reversal with the unfenced transcripts** (`…-reversal-2682b631-20261012-114534.log`, HEAD `2682b631`): exactly `ev_2` s67, `ev_2_retired` s55 and `ev_5_released` s60, then a restore.
- **Per-check reversals** (`…-check-reversals-2682b631-20261012-114322.log`, HEAD `2682b631`): 19 stubs. Each one turns 1 to 12 `written` or corpus tests red, apart from the two `unheld` wall-clock tests on the Mac.
- **Gate PASS on the exact head**, all ten steps, exit 0 after 429 s on gaming, base main `1fa085fa`: `~/botster-sessions/gates/botster-contracts-p6-written-lint-2682b631-pool-20261012-114546-75340.log`.
- **Main** is `1fa085fa`: #95's merge (`8dabe804`, probe paths only) and R-110 (`docs/steward-rulings.md`). Neither touches `fence_lint`. The steward makes #94's base merge with its own gate, and I check it per Q7. That gate lints the A23 transcript with this rule.
- **Round count.** 016c has given two NOT CLEAN rounds (on `9dbb526c` and `635c014a`). A third would reach BUILD.md's limit.

No finding. CLEAN on `2682b631b14375de611df7e6d3b05bd9681fa317`.
