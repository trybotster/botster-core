# P6 review: stage-2 mutant identity

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/229.
Exact head: `9d1afc359b87c7bd17494554116b5aa20244c03a`.
Review and full gate base: `3677b37e502497d5e63a91dfad92e5671bd19196`.
Risk tier: HIGH. The PR changes the mutation gate decision and its exclusion reason.

Two package findings remain. The reviewer sent both directly to P6 and Sol integration.

- **SI-R1-1 — MEDIUM: a selection error omits the completed stage-2 time.**
  `two_stage_decision` receives the completed `Stage` from `stage2`, then calls `check_stage2_selection` before `stage_outcomes` or `out`.
  A selection error therefore has no report or error context containing `tested.took`.
  The new P5-case proof expects only the stage-1 and baseline lines, although stage 2 completed.
  Plan revision 23r requires the gate to print each stage's time.
  Retain the elapsed stage-2 time on this path while keeping the selection error before any mutant verdict.
  Add a report or error proof with a known elapsed duration.
  The earlier report-retention finding from #225 remains closed; this finding concerns the new selection-error path.

- **SI-R1-2 — MEDIUM: the selection check can accept changed duplicate counts.**
  Sol integration raised this counterexample. The package reviewer confirmed it in the decision source.
  Stage-1 `missed` and `tested` names `[A, A, B]` pass `stage2_selection`: all duplicate occurrences are misses, and no name is skipped.
  Stage-2 `tested` names `[A, B, B]` pass `check_stage2_selection` because both sets contain only `A` and `B`.
  A stage-2 total of three, three caught outcomes, and exit 0 also pass `stage_outcomes` and `mutation_verdict`.
  The gate reports every miss caught, although one `A` occurrence is absent.
  Compare each name's count, or reject all duplicate full identities before the baseline.
  Add a `two_stage_decision` proof for equal total and equal set with changed counts per name.
  This is a decision counterexample. The reviewer did not establish a real cargo-mutants run that changes those counts.

The reviewer read all three changed files, the full PR body, the primary cargo-mutants source, and retained evidence.
The primary source confirms the reported filter defect in cargo-mutants 27.1.0.
`visit_expr_struct` pushes field-deletion mutants without calling `allows_mutant`.
Thus the ordinary name filters and exclusion regexes do not filter that mutant kind.
`main.rs` loads prior names before output rotation. `remove_previously_caught` removes those names after discovery for every mutant kind.
`load_previously_caught` reads `previously_caught.txt`, plus caught and unviable files when present.
These facts support reuse of `--iterate` for this fix.

`write_stage2_skip` replaces the old output directory and writes only the skipped names, one per line.
Its proof checks the file contents and removal of stale files.
The stage-2 command retains the exact escaped name filters, diff, exclusions, workspace features, and derived timeout.
The new outcome parser reads every mutant's full name and rejects an unnamed caught or missed mutant.
It excludes the baseline scenario from the tested names.
The decision rejects extra names and absent names before the count and mutant verdict.
It also rejects a duplicate name shared by a missed and non-missed outcome before the baseline starts.
The intended acceptance of duplicate all-missed names creates SI-R1-2.

The Prior art note records reuse of cargo-mutants' iteration filter and #225's decision and fixture forms.
The mutation reason names the new selection decisions and their proofs. It adds no broader exclusion.
The PR separately reports the stage-1 field-exclusion limit to the lead; this PR does not claim to fix that limit.

The reviewer read P5's retained outcome artifacts in:
`~/botster-sessions/shared/core-stage1/gate-logs/deb34002-mutants/`.
Stage 1 reports 82 mutants: 52 caught, 22 missed, zero timeout, and eight unviable.
Its outcomes SHA256 is `83a79706637af8bd4e81c62315f0210457066c3520f0f1b3005a396ddb1c5c90`.
Stage 2 reports 23 mutants: 21 caught, one missed, zero timeout, and one unviable.
Its outcomes SHA256 is `0bfc3143065c758a4449a84f30cc88a71131f692f71fcb0ae5c253ed49a28906`.
The same `edge_tap.rs:83:13` field-deletion name and span appear as caught in stage 1 and missed in stage 2.
The stage-1 row builds successfully and fails testing with exit 100. The stage-2 row builds and tests successfully.
The reviewer compared the retained names: stage 1 supplies 60 skipped names and 22 misses, with no mixed duplicate name.
The old stage 2 tests exactly one extra name, that field deletion, and omits none of the 22 misses.
The new selection check would identify this as a selection error.

The reviewer read the exact-head Linux gate:
`~/botster-sessions/gates/botster-core-stage1-p6-stage2-identity-9d1afc35-pool-20261010-143206-78615.log`.
All ten jobs pass, exit 0. Default: 1498 passing tests. Slow: 384 passing tests.
Both conformance reports show 193 passed and zero failed.
The strict report covers 407 pending transcripts: 386 run, 21 real-only not run, and two held.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
The real report names 87 pending-real ids, with zero newly passing.
Stage 1 reports 17 mutants: 15 caught, zero missed, zero timeout, and two unviable, in 107.8 seconds.
Stage 2 does not run in the main mutation job.

The real fixture proof passes in 12.638 seconds and runs stage 2 itself.
Its new `wide` function supplies a struct-field deletion that the default test catches.
With skipped names, the final report requires all ten mutants caught, including four by stage 2.
With an empty skip file, the proof requires the extra field deletion to produce a selection error.
With the slow assertion removed, the proof still requires a miss in both stages to fail.
The fixture retains the shared process group guard, bounded wait, and bounded baseline cleanup from #225.
It adds no sleep, polling loop, or production reaping. Its assertions check command results and returned fixture behavior.

The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: NOT CLEAN
