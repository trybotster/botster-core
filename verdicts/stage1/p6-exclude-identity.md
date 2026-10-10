# PR #236 — Excluded mutant identities

## Round 1 — 2026-10-10

Reviewed head: `8b63a17cf8e0969b7b4450b1512d974ff94458a7`.
PR and full gate base: `78b011d1a8a23229b87f81b0553ab052818b2200`.
Tier: HIGH under BUILD rule 1 for gate decisions and rule 3 for workspace configuration.
Scope: the complete two-file delta, mutation command paths, selection decisions, real fixture, PR body, and supplied gate.
The Git merge base equals the recorded gate base.

### R1-1 / EX-R1-1 — HIGH — OPEN

The new slow fixture starts its listing with `parse_listing(&list.output()?)` at xtask/src/ci.rs:2659.
Command::output waits for cargo-mutants, its stdout, and its stderr without a deadline.
The fixture gives this command no shared child owner or group guard.
The command runs before the fixture starts its guarded mutation stages and bounded baseline.
A stalled listing or retained output pipe can therefore block before those bounds apply.
The passing fixture result does not prove the stalled path.
This new test call violates the shared process ownership and bounded-wait rules.
The existing gate-command listing is outside this finding's scope.

Use botster_test_process::run_to_completion with a deadline from the existing SLOW_DEADLINE.
That shared runner owns the command's group and bounds both output and exit.
Retain the real listing, exclusion skip, removal-of-skip, and miss-in-both-stages assertions.
I independently confirmed the package finding and read the shared runner's implementation.
BUILD testing rule 10 requires group ownership and cleanup on every exit path.
The shared crate's API forbids unbounded child-pipe reads outside that crate.

### Other source checks

I found no further defect in the reviewed delta.
The PR's Prior art note reuses #229's full-name selection, iteration file, and per-name counts.
The new matcher uses regex::RegexSet, as cargo-mutants 27.1.0 does.
I read that version's options.rs, visit.rs, main.rs, output.rs, and mutant name construction.
Its struct-field visitor adds deletion mutants without allows_mutant.
Its normal exclusion match uses the full name, including file, line, column, and change.
Its iteration filter removes discovered mutants by that full name before testing.

parse_listing now reads each full name and rejects unnamed items or invalid output.
configured_exclusions reads the configured exclude_re list.
split_listed combines that list with the platform and derived command exclusions.
The selected and excluded lists retain each occurrence of a name.
Both stages receive the excluded names through the reused iteration file.
write_skip replaces the prior output directory and leaves only previously_caught.txt.
Earlier outcomes therefore cannot supply this run's results.

Stage 1 compares tested and selected names with their per-name counts before counts or a verdict.
Extra, excluded, or absent identities cause a selection error.
The error path first prints the tested count and elapsed time.
If every listed name is excluded, the step reports the exclusion count and starts no run.
Stage 2 still must test exactly the stage-1 misses, with their duplicate counts.
The #229 duplicate-name counterexample, extra-name error, and completed-stage timing proofs remain intact.
The adjusted count proof still rejects short summary counts when every selected identity is present.

The real fixture retains its public double and width assertions and its slow triple assertion.
Without an exclusion, it requires ten caught mutants, including four caught by stage 2.
With the wide exclusion, it requires eight caught mutants, including four caught by stage 2.
Removing the stage-1 skip must produce a selection error that names the excluded field deletion.
Removing all skips retains #229's stage-2 extra-field failure.
Removing the slow assertion still makes a mutant missed by both stages fail the step.
The existing stages retain group ownership and the derived slow deadline.
R1-1 concerns only the new listing call before those stages.

The parsed .cargo/mutants.toml equals the base configuration.
The delta changes only comments in that file. It adds no exclusion pattern.
The new decisions remain subject to mutation testing.
The delta changes no contract pin, pending ID, conformance count, process owner in production, or platform implementation.

### Base merges and supplied gate

All four automatic source merge trees match their recorded merge commits:

- 7141060d with v1 6c2bccda gives 73de83eb: tree `0b994c909e31a324c93b3070dfa422e6139aed4b`.
- 84e6eb37 with v1 89d57cc7 gives 859acb6d: tree `43d4fc16e14fa35b35a1d8640dd146dff3ef88ff`.
- 859acb6d with v1 504779bc gives 2cc991a7: tree `f4947a585d5275b70654d5b5792ce5e73bd2afc1`.
- 2cc991a7 with current v1 78b011d1 gives 8b63a17c: tree `6a07141ac203cc1d39f39742a456bd1c14ecd121`.

The final ci.rs equals its version at the original correction 84e6eb37.
The merges retain the accepted #231 pin, #235 build directory, and #232 PTY waits.
I used git merge-tree source comparisons. I ran no executable merge-check job.
The remote v1 base matches. The PR head matched the review request; the branch advanced after the finding.
git diff --check passes. This round reviews the requested 8b63a17c head only.

Log: `~/botster-sessions/gates/botster-core-stage1-p6-exclude-identity-8b63a17c-pool-20261010-161800-89172.log`.
The header names the exact head and base. The command is plain cargo xtask ci through the Linux pool.
All ten jobs pass. Default: 1501 passing tests, 13.294 seconds. Slow: 384 passing tests, 130.111 seconds.
The new exclusion, configured-exclusion, and listing proofs pass.
The real fixture proof passes in 19.065 seconds.
The main mutation job reports 29 mutants: 27 caught, zero missed, zero timeout, and two unviable, in 127.9 seconds.
Its skip list is empty for this diff. The real fixture proves the exclusion skip and removal-of-skip paths.
The main mutation job starts no stage 2.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Real-pending remains 87, with zero newly passing IDs.
The job exits zero after 446 seconds, including five seconds queued. The wrapper exits zero after 447 seconds.
These passing results do not close R1-1.

I read package NOT CLEAN at `9a5f340d4486ee2a41281a96582ce6826f2cc21a`, `verdicts/p6-exclude-identity.md`, Round 1.
The package verdict covers this exact head and base. Both reviews agree on the one open HIGH finding.
I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: NOT CLEAN (1 open).

## Correction source inspection — 2026-10-10

Inspected head: `4de5a07ea79e76378f1baf174d0f74a3f989266f`.
Parent and previous reviewed head: `8b63a17cf8e0969b7b4450b1512d974ff94458a7`.
The complete correction replaces only the new fixture's Command::output call.
It calls botster_test_process::run_to_completion with Deadline::after(test_budget::SLOW_DEADLINE).
That runner owns a new group, reads both pipes together, and bounds output and exit with the same deadline.
Its owner cleans the group on every return and panic path.
The exclusion and removal-of-skip assertions remain unchanged. git diff --check passes.
R1-1's reported source path is corrected. I found no new source defect in this correction.
The exact-head gate and package verdict are pending. This inspection completes no review round and grants no CLEAN verdict.
I ran no execution job.

## Correction gate inspection — 2026-10-10

Inspected head: `4de5a07ea79e76378f1baf174d0f74a3f989266f`.
Gate base: `78b011d1a8a23229b87f81b0553ab052818b2200`.
Log: `~/botster-sessions/gates/botster-core-stage1-p6-exclude-identity-4de5a07e-pool-20261010-162732-9689.log`.
The header names the exact head and base. The command is plain cargo xtask ci through the Linux pool.
All ten jobs pass. Default: 1501 passing tests, 13.155 seconds. Slow: 384 passing tests, 130.131 seconds.
The real fixture proof passes in 19.073 seconds. Its exclusion and removal-of-skip assertions remain unchanged.
The exclusion decision and retained stage-2 identity proofs also pass.
The main mutation job reports 29 mutants: 27 caught, zero missed, zero timeout, and two unviable, in 127.7 seconds.
Its skip list is empty for this diff. The main job starts no stage 2.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Real-pending remains 87, with zero newly passing IDs.
The job and wrapper exit zero after 519 seconds, including 150 seconds queued and 369 seconds running.
The remote PR head and base match. The merge base remains 78b011d1. git diff --check passes.
I read the updated PR body. It records the finding, correction, and exact-head gate.
The package verdict for this head remains pending. This inspection completes no round and grants no CLEAN verdict.
I ran no execution job.
