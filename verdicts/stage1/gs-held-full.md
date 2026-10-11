# Core #286 — budget-held identifiers at every seed

## Round 1 — source and supplied evidence, 2026-10-11

Head: `34b4773479cfc36b205b1ea9a0d48c88fe4eb6e4`.
Base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Risk tier: HIGH.
Package verdict: NOT CLEAN in `21cc53e5a73b33e4899bacfef170b66ebe4a3760`.
Sol reads its complete #286 section, all six own source deltas, affected source context, the body, and the approved design.

### HF-SI-1 — MEDIUM, open

This finding agrees with package GS-HF-R1-1.
Design section 8 requires a pool negative proof with a test-only held identifier that fails at seed 1 only.
The lists run must pass. The held-full run must fail and name seed 1.
The supplied positive runs report 32 ignored trials and zero failures.
The body reports a local negative change that fails at all four selected seeds.
Neither proof distinguishes the correct seed loop from a loop that executes seed 0 for every trial.
Supply the required proof on the reviewed source. Retain the source change, command, lists result, held-full result, reported seed, and exits.
This is an evidence finding. Sol finds no demonstrated source regression in the new verdict.

### Integration checks

The suite creates one trial per budget identifier and seed. Each trial calls outcome_at with that single seed.
The trial name and failure report retain the seed. A passing held trial remains ignored; it does not become a conformance PASS.
The default strict run retains its first-seed rule. The unproven class and pending verdict retain their rules.
The held parser requires the named full-seed proof text. Both callers use the same budget filter.
The gate rejects a failed binary, absent report, wrong identifier count, or wrong seed count.
The command clears all selection variables and supplies the CI seed set, strict mode, and held-full mode.
The command uses four threads and the existing 600-second bound, process group, tracker, output capture, and leftover check.
No timeout or budget value changes. The new shared helper retains the existing pending-real command behavior.
The PID-file producer is a nextest wrapper. This cargo-test command does not use that wrapper.
Sol therefore records no new finding solely from the helper's absent BOTSTER_TEST_PIDFILE setting.

The new run executes once in the candidate's slow gate step. The mutation stages and their selection remain unchanged.
Stage 2 does not execute held-full. This PR supplies no mutation coverage of budget-held identifiers at seeds 1–31.
The two new shell exclusions cover four mutants each. The pure decisions remain subject to mutation tests.
Old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` supplies no corresponding held-full run.
The approved design explicitly retains the held line's open question and owner. This review does not resolve that historical question.

### Supplied execution records

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-34b47734-pool-20261011-114751-62573.log`.
SHA-256: `3da9f81a8e36aaa58027de80267466c917a71b701fccde4287b295e5bc992ebf`.
The header names the exact head and base. All ten steps PASS in 376.0 seconds.
Default: 1752 PASS, 474 skipped, 11.341 seconds. Slow: 559 PASS, 2035 skipped, 40.967 seconds.
Held-full reports one budget identifier at 32 seeds: 32 ignored trials, zero failures, 9.64 seconds.
The minimum counts remain testkit 61/69, real-passing 49/68, and real-accepted 50/69.
Stage 1 reports 23 mutants: 21 caught and two unviable, with zero missed or timed-out mutants.
Sol reads all raw catches and catcher identities. No full independent identity comparison is claimed in this round.
Stage 2 has no remaining mutant. Fuzz selects no changed decoder crate. The complete log contains no FLAKY token.
The job and wrapper exit zero after 403 seconds, with zero queue time.

Timing: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-34b47734-pool-20261011-114445-56512.log`.
SHA-256: `27030241bdc2f5b072e4c1ccb915b6d886956649e67f9040ed4db0905c3cba13`.
The four-thread runs take 9.64 and 9.65 seconds. The serial runs take 38.54 and 38.53 seconds.
Each run reports 32 ignored trials and zero failures. The job and wrapper exit zero after 175 seconds.
Earlier serial measurements in the design remain owner reports; Sol has not read their raw logs in this round.

HF-SI-1 remains open. The green gate does not replace the required negative proof.
This verdict accepts no later source, import, gate, or pending removal.
Sol runs no build, test, gate, mutation, measurement, or executable probe. Never poll.

VERDICT: NOT CLEAN (1 open) at 34b4773479cfc36b205b1ea9a0d48c88fe4eb6e4
