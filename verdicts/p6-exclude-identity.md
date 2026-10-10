# P6 review: excluded mutant identities

## Round 1 — 2026-10-10

PR: https://github.com/trybotster/botster-core/pull/236.
Exact head: `8b63a17cf8e0969b7b4450b1512d974ff94458a7`.
Review and full gate base: `78b011d1a8a23229b87f81b0553ab052818b2200`.
Risk tier: HIGH. The PR changes gate decisions and their mutation exclusion reason.

The reviewer read the complete two-file delta, affected mutation commands, decisions, and proofs.
The reviewer read the current PR body. The Git merge base equals the recorded gate base.
One package finding remains. The reviewer sent it directly to P6 and Sol integration.

**EX-R1-1 — HIGH: the new fixture listing starts an unbounded process without a group guard.**
At `xtask/src/ci.rs:2659`, the fixture calls `parse_listing(&list.output()?)`.
`Command::output` starts cargo-mutants and waits for its output and exit without a derived deadline.
No shared child owner or group guard covers this command.
The listing runs before the existing bounded mutation stages and baseline.
A stalled listing or retained output pipe can block the fixture before those bounds apply.
This call is new real-process test code outside `botster-test-process`.
The passing fixture run does not prove the hung-command path.

Use the existing `botster_test_process::run_to_completion` with the existing derived slow deadline.
That helper owns the command's group and bounds its output and exit.
Retain the real listing, exclusion skip, removal-of-skip, and miss-in-both-stages assertions.
The reviewer read this shared helper's API and source at the exact head.
The existing gate command's listing call is outside this finding. This finding concerns the new test call.

The selection changes otherwise satisfy the stated follow-up scope.
`parse_listing` reads each listed full name and rejects an unnamed item.
`split_listed` applies configured and derived exclusions to those names with a `RegexSet`.
The reviewer confirmed that cargo-mutants 27.1.0 uses the same regex type and full-name match.
Stage 1 receives the excluded names through the reused iteration file.
Stage 2 skips them in addition to names that stage 1 did not miss.
The skip writer replaces the prior output directory, so stale results cannot supply this run's outcomes.

The stage-1 decision compares per-name counts before counts or any mutant verdict.
An excluded, extra, or absent identity causes a selection error.
The completed stage-1 error path first prints its tested count and known elapsed time.
When every listed mutant is excluded, the decision prints the exclusion count and starts no run.
The #229 stage-2 identity and timing proofs retain their counterexamples through the new listing input.
The count mismatch proof still fails even when all tested identities are present.

The Prior art note reuses #229's iteration file, full-name selection, and per-name counts.
It also reuses cargo-mutants' exclusion match through `regex::RegexSet`.
The exclusion file adds decision citations but no broader exclusion pattern.
The new decisions remain subject to mutation testing.

The real fixture retains its default public-value assertions and slow `triple` assertion.
With no exclusion, it requires ten caught mutants, including four caught by stage 2.
With the `wide` exclusion, it requires eight caught mutants, including four caught by stage 2.
With the stage-1 skip removed, it requires a stage-1 selection error naming the excluded field deletion.
Without skips and exclusions, it retains #229's extra field-deletion failure.
With the slow assertion removed, it still requires a mutant missed by both stages to fail.
The fixture's existing stages retain `OwnedChild::spawn_group`, the derived slow deadline, and bounded baseline cleanup.
The new listing call identified above does not use those bounds.
The PR adds no sleep, busy loop, or production reaper.

The reviewer read the exact-head Linux gate:
`~/botster-sessions/gates/botster-core-stage1-p6-exclude-identity-8b63a17c-pool-20261010-161800-89172.log`.
Its header names the full head and base. All ten jobs pass. The wrapper exits 0 after 447 seconds.
Default: 1501 passing tests. Slow: 384 passing tests.
The new exclusion, configuration, and listing proofs pass.
The real fixture proof passes in 19.065 seconds.
Stage 1 reports 29 mutants: 27 caught, zero missed, zero timeout, and two unviable, in 127.9 seconds.
Stage 2 does not run in the main mutation job.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Pending-real remains 87, with zero newly passing ids.
These passing results do not close EX-R1-1.

The reviewer changed no product code and ran no gate, build, test, mutation job, merge check, or reversal.

VERDICT: NOT CLEAN
