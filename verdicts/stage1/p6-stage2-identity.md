# PR #229 — Stage 2 mutant identity

## Round 1 — 2026-10-10

Reviewed head: `9d1afc359b87c7bd17494554116b5aa20244c03a`.
PR and full gate base: `3677b37e502497d5e63a91dfad92e5671bd19196`.
Tier: HIGH. The PR changes a gate decision and its mutation exclusion reason.
Authority: approved plan 23y, including section 8 revisions 23n and 23r.
Scope: all three changed files, affected decisions and command bindings, primary cargo-mutants source, and supplied evidence.

### R1-1 — MEDIUM — OPEN — The identity check loses duplicate counts

Location: `xtask/src/ci.rs:723`, `check_stage2_selection` and `two_stage_decision`.
Package finding: SI-R1-2. I raised this counterexample; the package reviewer independently confirmed it.

Stage 1 can report missed and tested names `[A, A, B]`, total three, three misses, and exit 2.
`stage2_selection` accepts duplicate names when every occurrence is missed.
Stage 2 can report tested names `[A, B, B]`, total three, three caught outcomes, and exit 0.
`check_stage2_selection` converts both lists to sets. Both sets contain only `A` and `B`.
The total check and mutation verdict also pass. The gate credits all three misses as caught.
One `A` occurrence is absent, although the reported total and name set agree.

Compare each name's count, or reject duplicate full identities before the baseline starts.
Add a `two_stage_decision` proof that equal totals and equal sets with changed counts fail.
Retain acceptance when only the order changes and each count remains equal.
This is a source decision counterexample. I did not establish a real cargo-mutants run that changes these counts.

### R1-2 — MEDIUM — OPEN — A selection error omits completed stage 2 time

Location: `xtask/src/ci.rs:849`, `two_stage_decision`.
Package finding: SI-R1-1. I independently confirmed its source path.

`stage2` returns a completed `Stage`, including `took`.
`check_stage2_selection` can return an error before `stage_outcomes` or the stage 2 report callback runs.
The error contains no elapsed time. The new P5-case proof expects only the stage 1 and baseline lines.
Plan 23r requires the gate to print each stage's time.

Report the completed stage 2 time on the selection-error path before returning its error.
Keep the identity check before any mutant verdict.
Add a public decision proof with a known elapsed duration and an incorrect selection.
The earlier #225 report-retention finding remains closed. This finding concerns the new selection-error path.

### Selection mechanism and proof scope

The new parser reads every tested mutant's full name and every missed name.
It rejects an unnamed mutant and excludes the baseline scenario from mutant names.
The decision rejects extra or absent names before its count check and mutant verdict.
It rejects a name shared by missed and non-missed outcomes before starting the baseline.
The duplicate all-missed case remains open under R1-1.

The installed cargo-mutants 27.1.0 source confirms that struct-field deletions bypass ordinary name filters.
`visit_expr_struct` adds these mutants without `allows_mutant`.
`main.rs` loads prior names before rotating the output directory.
`remove_previously_caught` removes prior names after discovery for every mutant kind.
`load_previously_caught` reads caught, unviable, and previously caught files when present.

`write_stage2_skip` removes the old stage 2 output directory and writes only `previously_caught.txt`.
The public file proof checks its contents and removal of stale files.
The stage 2 command adds `--iterate` and retains escaped, anchored full-name filters.
It retains the diff, exclusions, slow features, workspace tests, and derived timeout.
This mechanism addresses the known field-selection defect, subject to the two open findings.
The changed whole-body exclusion names the new decisions and public proofs. It adds no broader exclusion.
The PR separately reports the stage 1 field-exclusion limit. This review does not claim to fix that limit.

I read both retained P5 outcome files in `~/botster-sessions/shared/core-stage1/gate-logs/deb34002-mutants/`.
Stage 1 has 82 mutants: 52 caught, 22 missed, eight unviable, and zero timeout.
Its SHA256 is `83a79706637af8bd4e81c62315f0210457066c3520f0f1b3005a396ddb1c5c90`.
Stage 2 has 23 mutants: 21 caught, one missed, one unviable, and zero timeout.
Its SHA256 is `0bfc3143065c758a4449a84f30cc88a71131f692f71fcb0ae5c253ed49a28906`.
The same `edge_tap.rs:83:13` field-deletion name and span are caught in stage 1 and missed in stage 2.
Stage 1 supplies 60 skipped names and 22 missed names, with no mixed duplicate name.
The old stage 2 has exactly one extra name and no absent miss. The new check rejects that selection.

### Exact-head evidence and package receipt

Gate: `~/botster-sessions/gates/botster-core-stage1-p6-stage2-identity-9d1afc35-pool-20261010-143206-78615.log`.
All ten jobs pass under plain `cargo xtask ci`.
Default: 1498 passing tests, 13.025 seconds. Slow: 384 passing tests, 123.720 seconds.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
The real report names 87 pending-real ids and zero newly passing ids.
Stage 1 has 17 mutants: 15 caught, two unviable, zero missed, and zero timeout, in 107.8 seconds.
The main mutation job does not run stage 2. The gate takes 459 seconds.

The retained real fixture proof passes in 12.638 seconds and runs stage 2 itself.
The default assertion catches the new `wide` struct-field deletion.
With skipped names, the fixture requires ten caught mutants, including four caught by stage 2.
With an empty skip file, it requires an extra field deletion to fail selection.
With the slow assertion removed, it requires a mutant missed by both stages to fail.
The fixture retains the shared process guard, bounded wait, and bounded baseline cleanup from #225.
It introduces no separate process owner or production reaper.

I read package verdict `7acaabc555091caa307e84ae412d7cd3404167c1`, `verdicts/p6-stage2-identity.md`.
Its Round 1 is NOT CLEAN on this exact head. Both reviewers agree on the two MEDIUM findings.
I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: NOT CLEAN (2 open: R1-1 MEDIUM and R1-2 MEDIUM).
