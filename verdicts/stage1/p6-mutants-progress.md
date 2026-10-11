# Integration review: #237 mutation progress output

## Source and gate inspection before Round 1 — 2026-10-10

Inspected head: `23883eeabb6cda9f6f37f341bb220d23b1d6ec6a`.
PR, gate, and merge base: `b028a269383b8d2af9ac349b395765a47f1b8f1d`.
Tier: HIGH, because the change affects gate infrastructure.
Scope: the complete one-file delta, both mutation commands, the shared fixture command, and supplied gate evidence.

The #233 gate at a21fcbd3 passed its stage-2 baseline in 254.7 seconds.
It then selected 74 mutants and produced no output for 20 minutes. The pool stopped the job with exit 124.
I read this failure in the supplied log, not through a reproduction job.

The correction adds --caught and --unviable to both mutation stages through MUTANTS_PROGRESS.
Stage 1 uses stage1_run_args after the existing selection closure. Its listing does not use these output flags.
The shared fixture uses the same stage1_run_args. Stage 2 adds the flags before --iterate and before the nextest separator.
The diff does not change mutant selection, exclusions, skip files, process ownership, timeout values, or outcome parsing.
The verdict still uses outcomes.json and exit codes. Printed outcome lines do not decide acceptance.
No contracts pin, conformance list, public API, or production crate changes.

I checked cargo-mutants 27.1.0 source in the local registry.
main.rs defines both flags as output options. options.rs copies them to print_caught and print_unviable.
console.rs updates outcome counts before it applies the output filter. These flags remove the caught and unviable suppression.
Thus the flags change output without changing the mutant result.
The command now prints after each outcome. It does not add output during an individual build or test.

The new argument proof checks both flags in both stages and retains the complete stage-1 command arguments.
The existing stage-2 proof retains its full-name filter, exclusions, slow features, slow deadline, and nextest arguments.
The real fixture retains all exclusion, selection-error, and missed-mutant assertions.
It runs both mutation stages with the corrected command builders.
I found no source defect in this delta.

Gate log: `~/botster-sessions/gates/botster-core-stage1-p6-mutants-progress-23883eea-pool-20261010-173615-59942.log`.
The header names this exact head and base. The command is plain cargo xtask ci through the Linux pool.
All ten jobs pass. Default: 1509 tests passed in 11.074 seconds. Slow: 395 tests passed in 94.872 seconds.
The new progress proof and retained stage-2 argument proof pass.
The real fixture passes in 13.635 seconds. The retained mutant-timeout proof passes in 21.940 seconds.
The main mutation job prints six caught outcome lines and reports six mutants, all caught, in 77.6 seconds.
It reports zero missed, zero timeout, and zero unviable. Its skip list is empty.
The main job needs no stage 2 because stage 1 missed no mutant. The passing fixture still exercises stage 2.
Both conformance reports show 193 passed and zero failed. Strict coverage remains 386/407, with 21 real-only and two held.
Minimum counts remain testkit 50/69, real 31/68, and real-accepted 31/69.
Real-pending remains 81, with zero newly passing IDs. The prebuild uses contracts-v0.1.27 at b161db87.
The job and wrapper exit zero after 539 seconds, including one second queued and 538 seconds running.
The remote PR head and base match. git diff --check passes.

The same-head package verdict remains pending. This inspection completes no round and grants no CLEAN verdict.
I changed no product code. I ran no build, test, gate, mutation job, merge-check job, or reversal job.
