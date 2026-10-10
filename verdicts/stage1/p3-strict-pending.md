# PR #218 — Strict testkit pending check

## Round 1 — 2026-10-10 — Complete integration review

Reviewed head: `d55641cbcaf5d0b8c05b43b54587540832a941e5`.
PR and full gate base: `fc1e8160abfd8248b155383ef1d766c6377f8a11`.
Tier: HIGH under rules 1, 3, and 5: gate decisions, shared testkit, and mutation exclusions.
Authority: approved plan 23y, its existing pending rules, and the lead's two held classes.
Contracts pin: `0ac061d225724f6f7fc7b2368f637ba2c9a6c2b6`.
Scope: all ten own-diff files, shared runner, command environment, four exclusion reasons, and supplied evidence.
The user authorized blocked tools directly in this session before this review started.
This is Sol's first complete integration review of #218, not a delta from an integration CLEAN.

### Strict trial and accounting

Only the testkit tier enables strict mode. The real tier retains its existing behavior.
A pending transcript runs and must fail. A held transcript runs and must pass.
A real-only id does not run through the testkit strict trial.
Expected outcomes return `Completion::ignored_with`; they never increase the passing count.
An unexpected result fails the trial, the conformance process, and the lists step.
Active transcript trials still require their public passing outcome.

The runner counts all pending transcripts, including unselected transcripts.
Its strict report separately counts run transcripts and real-only transcripts.
`strict_ran` rejects a missing report or an incomplete total.
`lists_verdict` requires that report and equal, present passing counts from both runs.
`run_report` rejects a failed process and returns its report only on success.
The pure decisions remain mutation-tested.

`conformance_command` removes `BOTSTER_ONLY`, `BOTSTER_CLAUSE`, and `BOTSTER_SEED` after caller environment values are applied.
Both lists runs use that command. Neither run can inherit a selection that narrows the checked ids.
The command-output proof reads `Command::get_envs`, requires explicit removals, and retains strict mode.
It replaces the previous private-constant assertion.
The runner takes the tier's full seed set, except for the authorized budget class.

The held parser requires five fields and nonempty owner, authority, and reason text.
`held_problems` rejects duplicate ids, non-pending ids, missing transcripts, and real-only overlap.
The checked-in file has exactly two entries with their authorities and missing proofs:
`ev_8_every_query_stays_in_output` is budget-held and runs at the first seed;
`a20_1_testkit_proven_ids_are_exactly_the_named_list` is unproven-held and runs at every seed.
The budget line records that its full-seed proof is missing. It does not claim full coverage.
The parser reads the base's generated `id<TAB>slow:proof` file.
The existing ledger command checks that file against the pinned replacement map.

All 477 Core pending id lines equal the base, in order.
Only the query-budget comment changes in that file.
Pending-real and real-only files equal the base bytes.
The PR changes no transcript, ledger membership, deferral, withdrawal, or real-tier exception.
Plan 23y still leaves strict real-pending and pin-move re-pending enforcement to a separate HIGH PR.

### Package findings and mutation exclusions

I read the package verdict history at `706a905bdcd11bd1fae93ac0329ae071e869bd6a`, `verdicts/p6-strict-pending.md`.
Round 6 is CLEAN at this exact head.
The previous missing gate-decision proofs, missing held transcript check, empty authority, and private-constant assertion all close.
I independently checked their current source and public input/output proofs.

The two new whole-body exclusions cover `lists_job` and `conformance_report` only.
Those shells perform process I/O and forward to `lists_verdict` and `run_report`.
Their reasons cite the actual passing-count, strict-coverage, environment, and process-result proofs.
The existing `lists::command` reason adds its report decision and held validation.
The existing ledger command reason retains its copy and derived-file decisions from the base.
I read each changed reason and the corresponding shell. No new gate decision is excluded.
The strict citation check passes in the supplied gate.

### Gate image and prior art

The existing cargo-mutants shim retains source copies on the target volume.
The new Cargo wrapper resets child build/test temporary roots to `BOTSTER_TEST_TMPDIR` or `/tmp`.
It executes the saved Cargo path with the same arguments and is outside PATH.
I checked cargo-mutants 27.1.0's `cargo_bin`, which reads CARGO for those child commands.
The existing image tag includes the Dockerfile hash.

The retained image evidence at `92d6fc3f` shows the unchanged Git-source proof passing inside the mutation baseline.
It also records the mutation source copy under `/work/target/tmp/cargo-mutants-work-GIuSWn.tmp`.
The wrapper changes neither the test's assertion nor its process ownership.
The PR's Prior art note explains the reused status parser, ledger copy mechanism, runtime ignored result, and shim.
It explains the custom strict decision module and rejected alternatives.
No production machine test branch, process guard, wait, reaper, signal, or timer changes in this own diff.

### Merge and supplied evidence

The supplied base check for `b40badeb` to this head passes conditions 1 and 4, but fails conditions 2 and 3.
The base also changed `xtask/src/ci.rs` and `.cargo/mutants.toml` in #225.
I read the failure report and both shared-path comparisons. I do not treat that report as a pass.
The current full source review includes the shared paths.

I independently compared each own diff's added and removed lines, with their paths.
Both normalized sequences contain 35,019 bytes with SHA256
`77389b1df7b2a8ad5879ec330f3237e4303f3445b17d479be89b7bd01b4603eb`.
The comparison omits context and hunk headers, not changed source lines.
The only supplied context difference is #225's mutation-report comment.
The automatic merge tree and reviewed head tree both equal `bfac04ea52a6f7325660c29a2ebce998d89b8711`.
The head contains the stated current base. The GitHub head and base match.

Exact-head gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-strict-pending-d55641cb-pool-20261010-135459-54574.log`.

The header names the exact head and base. The command is plain `cargo xtask ci`.
All ten steps pass. Default: 1,495 passed in 13.335 seconds. Slow: 384 passed in 124.222 seconds.
Both testkit reports show 193 passed and zero failed.
Strict coverage: 386 of 407 pending transcripts run, 21 real-only do not run, and two held are included.
The separate no-transcript count is 70.
Minimum counts remain testkit 50/69, real 29/68, and accepted 29/69.
Pending-real remains 87, with zero passes.
Mutation stage 1: 46 mutants, 42 caught, four unviable, zero missed or timeouts.
Stage 2 does not run because stage 1 misses none. There is no obsolete second mutation command.
The gate exits zero after 704 seconds.

I read the retained red and control logs in `evidence/p3-strict-pending-3a72d1a0/`.
A re-pended passing `lc_12` fails with the removal diagnostic.
A failing held `a2_1` fails with the held-removal diagnostic.
The edited files without strict mode produce an ignored trial and no failure.
These are retained earlier proofs, not new exact-head gate results.

A later P5 gate exposed a selection defect in inherited #225.
That defect is outside #218's own changes and does not change this strict pending decision.
This CLEAN does not endorse #225's stage-2 selection; its verdict records the separate correction.
A later v1 merge still needs the plan's merge comparison and exact-head gate.

I changed no product code and ran no build, test, gate, mutation job, or reversal job.

VERDICT: CLEAN (0 open).
