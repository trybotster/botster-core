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
