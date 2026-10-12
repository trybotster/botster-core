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

## Round 2 — seed-specific negative evidence, 2026-10-11

Exact head: `34b4773479cfc36b205b1ea9a0d48c88fe4eb6e4`.
Reviewed base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
HF-SI-1 closes. Open integration findings: 0.
The source and earlier exact-head green gate retain their Round 1 scopes.

Sol reads the complete note at `~/botster-sessions/shared/core-stage1/evidence/gs-held-full/seed1-negative-proof.md`.
Note SHA-256: `4c8df2d5aba9a3633aa755b15486aaa66bdddddac9d30b45e4f01c836153ddee`.
Sol reads both raw command headers, snapshot identities, relevant suite results, strict report, failure details, summaries, and exits.
Sol independently reads the complete scratch delta against the reviewed head.
Snapshot `421a8f76118a042b6e0b5c91b5fd32edaaa76a57` changes only `tests/suite/mod.rs`.
The five added lines set `max_polls = 0` only for EV-8 when the selected seed list equals `[1]`.
Snapshot `c37d70d4206547b9f2dc156ceee7fe26c9b5fdbe` has the identical tree `2880ab067e899b772ee5a3801247783f0111dc50`.
Both pool jobs therefore use the same scratch fault on the reviewed source.
These snapshots use `--allow-dirty`. They are evidence snapshots, not delivery heads or merge gates.

Held-full log: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-34b47734-pool-20261011-151838-69247.log`.
SHA-256: `3615391a914c83cee115260a7dfb74160dca0356a38db2430b5f1c463a9256f9`.
The failing trial is exactly `conf::ev_8_every_query_stays_in_output@seed_1`.
Its detail names seed 1, raw step 5, and `limit=max_polls`, with an INCONCLUSIVE transcript outcome.
The held verdict correctly fails that trial. The run reports one failed trial and 31 ignored trials in 9.64 seconds.
The held-full report names one budget identifier at 32 seeds, one trial per seed.
The slow step FAILS in 165.2 seconds and its shell reports `slow exit 1`.
The job exits zero after 274 seconds; the outer wrapper exits zero after 275 seconds.
The final echo causes those wrapper zeros. They do not change the slow failure into PASS.

The first job's lists result is invalid for this proof. Its base is the later f68f26a7 tip.
The base-relative list check reports 19 problems and exits one before the strict run starts.
That failure remains retained. It supplies no successful lists evidence.

Corrected lists log: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-34b47734-pool-20261011-152342-380.log`.
SHA-256: `e3596f124360b3e97fd93e23d96046b99232e4e5d87302c9a2fe39a4c5d077d9`.
The wrapper header names live v1 5e953398, but the command explicitly overrides `BOTSTER_CI_BASE_REF` with the reviewed base.
The actual lists output confirms `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Both conformance runs report 216 PASS and zero failures, with 474 ignored trials.
The strict report names 363 of 381 pending identifiers, 18 real-only identifiers not run, and two held identifiers.
Lists PASS in 11.5 seconds and the shell reports `lists exit 0`.
The job queues for 251 seconds and runs for 25 seconds. The job and wrapper exit zero after 276 seconds.

The first-seed strict run leaves the seed-1-only fault inactive. Held-full activates it only in the seed-1 trial.
A loop that executes seed 0 for every trial would miss this fault. This evidence satisfies design section 8 and HF-SI-1.
The owner reports scratch restoration and a clean worktree; Sol does not independently attest to that local worktree state.
GitHub confirms the unchanged delivery head and reviewed base through the CLI fallback after MCP fails.
The live body includes both commands' results, the scratch condition, and the invalid first lists result.
Body SHA-256: `aa3837f7b0ad05dffbf3eaf9516e052761d82cebc3ffffccbd9dead89c9997bc`.

Sol reads the complete package Round 2 at `e12e7c8c47badd56a5edda15110f6ee9ff59e543`, `verdicts/gs-gate-speed.md`.
Its CLEAN closes matching GS-HF-R1-1 on the same head and base. No package finding remains.
These evidence jobs do not replace the earlier green gate or supply a gate on any later import.
FINAL-READY, the lead's SLOT, current-base import, applicable check and review, final gate, final body, and Q7 remain due.
This source and evidence CLEAN accepts no later head, import, pending removal, or final merge.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable probe. Never poll.

VERDICT: CLEAN (0 open; source and evidence before final SLOT) at 34b4773479cfc36b205b1ea9a0d48c88fe4eb6e4 against base 926dfb62f3f7168990b2a48741946932d84b2fbb

## Round 3 — final-slot source import

Exact head: `ac457ca6a83731e68e4fc44ad3551b8ec39e3612`.
Reviewed base: `9053b1df868bbec2d62977660f44907e73323c55`.
Tree: `98bb9f0393e4b63201927920a55d5d32c9cbdcd4`.
Sol verifies both parents: reviewed head 34b47734 and imported base 9053b1df.
GS supplies the SLOT import in `msg_plugin-w_1791764321_65b7c5`.

Sol reads the complete retained canonical check.
Path: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-ac457ca6-base-merge-check.log`.
SHA-256: `f861baf8a063ac95c234651635355d179b65f9b8bdf85504e135021aa469102a`.
The ancestry checks PASS. Conditions 1, 2, and 3 FAIL on conformance/core-held.txt.
Both pending-file condition-4 checks PASS. This result requires both reviewers' explicit delta reviews.
Sol runs no executable base check and does not convert that retained FAIL to PASS.

Sol independently compares all five other own patches after removing only blob-index and hunk-location metadata.
Every patch matches the reviewed patch byte for byte.
Sol independently verifies that every imported non-own blob matches the imported base.
The pending, real-pending, ledger, and real-only files match that base byte for byte.
The only changed own content is the held-file resolution. Sol reads it completely against all three inputs.

The old base and reviewed head each contain ev_8/budget and a20_1/unproven.
The imported base adds dp_5/budget. The merged head contains exactly those three entries with those classes.
The merged set equals the imported base set. GS adds no identifier or class through the resolution.
The a20_1 entry remains unchanged.
The ev_8 entry preserves the base's updated authority and its step-14 real_wait cause.
The dp_5 entry preserves the base's owner, authority, and step-18 real_wait cause.
Both missing-proof sentences become the held-full proof location at seeds 0–31, one trial per seed.
The header retains the reviewed strict-first-seed and slow-full-seed rules.

Sol reads the current generic budget filter, trial construction, and report check in their imported context.
The unchanged filter selects both budget entries. The loop constructs 64 identifier/seed trials with their actual seed values.
The unchanged decision checks success and a report naming two budget identifiers at 32 seeds.
A failed trial, absent report, malformed report, or narrowed selection still fails the step.
No bound, deadline, selection, mutation stage, or exclusion changes in this import.
The earlier seed-1 negative proof retains its exact old-head scope. It does not become new execution evidence.

Sol reads the complete current body and verifies the exact GitHub head and base.
Body SHA-256: `3f40b65fec07cb9b43dd84058a5da66164a9195c35b7076ba4ff91843b48b0ef`.
The final-head section names both budget identifiers, the conflict resolution, and the complete three-way sets.
It ties all earlier one-identifier evidence to 34b47734 and marks the final gate running.
The complete package delta and body correction at `e66e905202242b49d7f9920d3576134ff46320d6` agree with these source findings.
Its LOW body finding closes. Both source delta reviews now have zero open findings.
GS reports a local clean and a 64-trial held-full PASS. Sol does not independently validate that local run here.
The completed exact-head final gate, final body, and final package acceptance remain due.
Sol reads no running gate output and runs no execution job. Never poll.

VERDICT: CLEAN (0 open source delta findings; final gate remains due) at ac457ca6a83731e68e4fc44ad3551b8ec39e3612 against base 9053b1df868bbec2d62977660f44907e73323c55
