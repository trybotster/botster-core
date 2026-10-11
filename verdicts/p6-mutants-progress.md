# P6 review: mutation progress output

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/237.
Exact head: `23883eeabb6cda9f6f37f341bb220d23b1d6ec6a`.
Review and full gate base: `b028a269383b8d2af9ac349b395765a47f1b8f1d`.
Risk tier: HIGH. The PR changes gate infrastructure.

The reviewer read the complete one-file diff, affected stage commands, fixture, and current PR body.
The Git merge base equals the recorded gate base. No package finding remains.

Both mutation stages receive `--caught` and `--unviable` before the nextest argument separator.
`stage1_run_args` retains `--iterate`, the output directory, and the complete existing nextest arguments.
The production stage-1 command and the real fixture use this same function.
The listing retains its existing selection arguments and receives no progress flags.
Stage 2 retains its exact name filters, exclusions, iteration file, workspace features, slow filter, and derived timeout.
The new options do not change its mutation selection.

The new proof checks the complete stage-1 arguments.
The existing complete stage-2 argument proof now includes the two literal progress options.
These proofs test command arguments, with no assertion about private mutable state.
The decisions remain subject to mutation testing. The PR changes no mutation exclusion.

The Prior art note reuses cargo-mutants' own output options and the manual pool command's caught output.
The reviewer read cargo-mutants 27.1.0's option mapping and `Console::scenario_finished` in the local dependency source.
The caught and unviable options disable suppression of those completed mutant lines.
The reviewer found no stage-specific behavior that would change their meaning in stage 2.
This is output for completed mutants. It does not add a periodic timer during an individual build or test.

The mutation verdict still reads `outcomes.json` and the process exit code.
The change adds no outcome parser or decision change.
The real fixture retains its exact success, exclusion, removal-of-skip, extra-identity, and miss-in-both-stages assertions.
Its listing retains the shared bounded output runner from #236.
Its mutation stages retain group ownership and the derived slow deadline.
The baseline retains bounded cleanup and its leftover-process check.
The PR adds no timeout value, wait, sleep, busy loop, or production reaper.

The reviewer read the retained PC failure:
`~/botster-sessions/gates/botster-core-stage1-pc-program-controls-a21fcbd3-pool-20261010-164623-57687.log`.
Its stage-2 baseline passes in 254.7 seconds. Stage 2 lists 74 mutants.
The pool then reports 20 minutes without output and stops the job with exit 124.
This is supplied failure evidence. The reviewer did not reproduce it.

The reviewer read the exact-head Linux gate:
`~/botster-sessions/gates/botster-core-stage1-p6-mutants-progress-23883eea-pool-20261010-173615-59942.log`.
Its header names the full head and base. All ten jobs pass. The wrapper exits 0 after 539 seconds.
Default: 1509 passing tests. Slow: 395 passing tests.
Both argument proofs pass. The real mutation fixture passes in 13.635 seconds.
The gate prints six caught lines: three replacements of `stage1_run_args` and three replacements of `stage2_args`.
Stage 1 reports six caught mutants, zero missed, zero timeout, and zero unviable, in 77.6 seconds.
The main job starts no stage 2. Its stage-2 options are checked by the complete argument proof and used by the real fixture.
This gate has no unviable mutant line to observe. The option proof and primary console source cover that option.

Both conformance reports show 193 passed and zero failed.
Strict coverage is 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts are testkit 50/69, real 31/68, and real-accepted 31/69.
Pending-real is 81, with zero newly passing ids.
The PR changes no contract pin, pending list, or contract assertion.

The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: CLEAN
