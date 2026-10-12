# Held-full accounting integration review

## Round 1: complete source review and previous-head evidence

Exact source head: `b9763b0f851a029a48af3a0f495388d69683f10b`.
Tree: `12d58ecbce13fef61b696ea4cdf693f2a1f8d435`.
Branch base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Stacked #324: `0572493fc9e695acd57b1916e485fdd900ba7643`, imported at `027e7c40`.
Previous evidence head: `3ce9220cfcc029ebb129e7c34bf2c57615a4dbd4`.
Lead assignment: `msg_plugin-w_1791836886_ad222e`. Sol-B completes #324 first at verdict `2bc3e0ea`.

Sol-B reads the complete own source, full body, affected suite and runner paths, plan 23cn, and the lead's design approval.
Plan pin: `/Users/jasonconigliari/botster-sessions/pins/stage1-plan.a3333ad4.md`, section 0.
Lead handoff approval: line 1619. The stacking and 1.5-second condition are at line 1638.
The lead accepts gate refusal as condition 1: a failed proof fails the slow step, so the candidate cannot land.
This approved reading requires no automatic source-file edit or later manual return before the failed gate counts as RED.

### Run and proof boundaries

The held-full run uses nextest on the testkit conformance target with the ordinary default-tier profile.
The existing two-second constant supplies the proof limit; no new budget value is introduced.
Both default and ci profiles retain the two-second termination limit. The ci profile retains zero retries.
Profile selection preserves the prior rule: unset or empty CI selects default; nonempty CI selects ci.
The extracted profile_under helper permits explicit tests of all three cases.
The ordinary default-tier command uses that same helper without changing its profile behavior.

The run sets strict and held-full variables and the existing CI seeds 0-31.
It removes selection variables, so a caller cannot narrow the approved matrix.
The runner removes the earlier JUnit file before starting. A removal error fails; no fresh JUnit file also fails.
The existing bounded runner retains caps, process-group ownership, process tracking, leftover cleanup, and the whole-run deadline.
Each transcript runs as one trial for one seed. The existing linked-library check precedes the separate held-full trial list.

held_full_proved requires nextest success and every exact binary, kind, identifier, and seed in the JUnit time map.
The binary is botster-core::conformance. The kind is pending: held-full.
A missing entry, another binary, a bare name, a failed run, or a time above two seconds fails the slow step.
The proof accepts exactly two seconds. The existing JUnit reader provides the time map.
No automatic log matcher, retry, or mutation catch rule changes.

### Accounting and freeze scope

Only the three authorized budget identifiers leave core-pending: dp_5_full_input_queue, dp_5_unstarted_terminal_query, and ev_8_every_query.
Their complete identifiers and 32-seed proof results appear in the evidence table below.
No identifier enters pending. The final pending set has 400 identifiers on this branch base.
The held identifier and class sets remain unchanged. An unproven identifier must remain pending.
Real-only treatment still takes precedence over held-full treatment.

The ordinary testkit tier exposes promoted identifiers as ignored budget: held-full trials.
Their proof is the separate pending: held-full matrix, not a new ordinary transcript PASS.
The real tier still runs those identifiers must-pass. None is in real-pending.
The minimum report unions ordinary passes, named proofs, and promoted budget identifiers without counting an identifier twice.
It prints N/69 and K budget-held. K counts the minimum intersection: only dp_5_full is minimum here.
The lists output also prints that minimum subset count. The gate reports 68/69, of which one budget-held.
Standalone milestone output remains conditional on the gate's held-full proof.

The complete xtask delta changes only the approved held-full run, its decision and tests, shared profile extraction, and K output.
It adds no unrelated check under plan 23co's freeze.
Static TOML comparison verifies that every mutation configuration value equals the branch base.
Only mutation-exclusion explanations and proof citations change; held_full_proved itself remains subject to mutation testing.
The fourteen held_full whole-body replacements remain within the existing process-glue exclusion.
The imported #324's four code blobs remain exact. Its held-record metadata remains present.
The real-pending, minimum, ledger, real-only, real-proof, deferred, lockfile, and nextest-profile blobs equal the branch base.
No product implementation, transcript, contracts pin, timeout, seed count, or deadline changes.
The source diff check passes.

### Findings and package receipt

Package GS-HF-R1-1 LOW is CLOSED: the complete corrected body distinguishes budget: held-full from pending: held-full.
Package GS-HF-R1-2 LOW is CLOSED at this source head: the held header now distinguishes pending held identifiers from promoted budget identifiers.
The delta from 3ce9220c changes only that header comment, not source behavior or identifier membership.
Sol-B independently reads that complete delta.
Sol-B reads the complete package section at `2e30fade8ee94be2dcbbafc66ec7f6abcbe44268`.
Its exact-head source CLEAN agrees with this review. All findings, including LOW, are closed for source review.

### Previous-head execution evidence

Completed log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-held-full-accounting-3ce9220c-pool-20261012-131407-95764.log`.
SHA256: `f059b97ee30437d671da1ac1e3dbfdd1119cb666b64bfda58f895b2541ca34d5`.
The recorded gate base is `3e655eff46d36f8c337339a1a17d401c8c662594`.
That base is not an ancestor of this branch. Their merge base is the branch base 2683de7c.
The completed gate supplies initial evidence for its recorded head, not current-base delivery or this replacement head.

All ten steps PASS, with a total of 349.6 seconds.
The job and wrapper exit 0 after 768 seconds on msa1: 408 queued and 360 running.
Default passes 2003 tests in 10.614 seconds; ordinary slow passes 666 in 43.327 seconds.
The held-full run passes 96 trials, with zero skipped, in 22.742 seconds.
Sol-B independently checks all 96 identities: each identifier has exactly seeds 0-31 once.

| Identifier | Trials | Maximum | Seed |
| --- | --- | --- | --- |
| conf::dp_5_full_input_queue_stops_reading_the_route_and_never_drops | 32/32 PASS | 0.368 s | 10 |
| conf::dp_5_unstarted_terminal_query_frame_retires_when_the_query_resolves | 32/32 PASS | 1.271 s | 0 |
| conf::ev_8_every_query_stays_in_output | 32/32 PASS | 1.263 s | 15 |

All maxima are below two seconds. The dp_5_full maximum is below the lead's 1.5-second condition.
All three ordinary real-tier trials also PASS. This evidence changes no real-pending membership.
Stage 1 has 49 mutants: 46 caught, one missed, two unviable, and zero timeouts.
The unmutated stage 2 baseline passes 676 tests in 46.367 seconds.
Stage 2 catches the real-route log replacement from #324. Final results are 47 caught, two unviable, zero missed or timeout.
Sol-B independently matches all 47 raw identities to citations and verifies every catcher in ordinary PASS results.
No runtime FLAKY or INCONCLUSIVE result appears. The inherited EV-2 exclusion earns no catch credit.
Fuzz selects no changed decoder harness and passes without a harness run.

### Earlier RED and reversal evidence

The 1e331bbd gate remains RED for the profile-selection negation survivor in both stages.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-held-full-accounting-1e331bbd-pool-20261012-124026-46572.log`.
SHA256: `4f17c416ab3991d28bf3ac68737099cf5f4057e237690e9e09b449dac174f563`.
The profile_under repair adds explicit unset, empty, and nonempty cases without adding an exclusion.

Focused record: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-23cn-05d65376-focused-mutants.log`.
SHA256: `1f86cda6a5037a5781a4cc70de7184a7dac00cb2ca534f09902465fdcad693de`.
It tests 29 identities in both stages without configuration exclusions.
Default catches fourteen, including all five profile mutants; slow misses all 29.
Sol-B independently checks the fifteen common misses: fourteen held_full replacements and one command-to-Ok replacement.
All fifteen match existing exclusion expressions. This focused wrapper's exit 0 is not a claim that all mutants were caught.

Sol-B reads the complete reversal script and its failure evidence at 05d65376.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-23cn-reversal-05d65376.log`.
SHA256: `7f79f58ab4a3f338eed129949ae6205eef387d49aa9dd5918c36831fc22b0cb8`.
Removing the per-trial limit makes the 2.01-second regression fail because the proof incorrectly returns Ok.
Deleting the profile negation makes its regression fail. Both negative runs exit 100.
The log records restoration and three passing intact-source tests. The relevant executable source remains unchanged at this head.
Cleanup and worktree exclusivity remain owner reports, without independent execution verification.
This is focused negative evidence, not a completed replacement-head gate.

### Disposition

The previous-head gate is not relabeled as replacement-head evidence.
GS reports the lead requires the train gate instead of a separate member gate and stopped the queued member gate.
The completed exact train gate, updated final body, combined final receipt, and lead merge checks remain due.
This receipt accepts no later head, completed train, merge, or additional retry.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; complete source only on b9763b0f, completed exact-head delivery evidence remains due)

## Round 2: train 8 source delta

Exact train head: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
Tree: `3f6c95e20e26a54eca52908502a4bade7aec1cad`.
Train base: `007efbde6a59af403d0937c38361c137a8961a48`.
Frozen member: `b9763b0f851a029a48af3a0f495388d69683f10b`.
Lead request: `msg_plugin-w_1791837736_3600b2`.

The train applies #324 first. Its four code paths are already present when #329 merges.
The final #329 merge applies eight own paths, with no extra path.
Seven applied added/deleted line sets match the reviewed member directly.
The eighth, core-held.txt, matches after subtracting #324's already-applied metadata delta.
No own edit disappears or new edit enters that comparison.

The final pending implementation, suite, test-budget, held file, and mutation-configuration blobs equal the frozen member.
The final CI source differs only through the accepted #308 step_names function and listed-test fixture signature.
Both additions preserve signature validation and the existing gate step table.
The held-full call, exact matrix decision, profile selection, process bounds, JUnit freshness, and regression tests remain exact.
The final lists code retains #323's independent re-entry parser and wiring beside the accepted K output.
No real-pending re-entry tag supplies a held-full trial or changes K.
The complete applied xtask delta adds no check outside the approved held-full and K scope.

The current base has 388 pending identifiers. Only the three approved budget identifiers leave, yielding 385.
Real-pending remains 38, ledger 692, and minimum 69; all three sets equal the train base.
All earlier base removals remain removed. Held identifier and class sets remain unchanged.
K remains the minimum intersection: one budget-held identifier, dp_5_full.
Every mutation-configuration value equals the train base. Only accepted explanatory comments change.
The known-flaky list and parser equal the train base, retaining its IN-8 entry owned by PC and removed by #321.
The train does not restore the retired EV-2 entry from older member evidence.

Sol-B independently compares all three held transcripts between .44 and .46. Each transcript is byte-identical.
The .46 transcript SHA256 values are:

| Identifier | SHA256 |
| --- | --- |
| conf::dp_5_full_input_queue_stops_reading_the_route_and_never_drops | 12f43fecbe1431989ac41af2a26d5bcdc9319df27e35c6cb31eba122e7b80ad3 |
| conf::dp_5_unstarted_terminal_query_frame_retires_when_the_query_resolves | 72e328e82affc783d6ede25a17ecd359638863bd3eb012a839d8059328ecc934 |
| conf::ev_8_every_query_stays_in_output | 22259b1488b1c0492bbf656986e14d959efa7f21119df231b7e1bddb7a067b8c |

Fresh train evidence must still prove every trial against the imported worker clock and pin.
Sol-B reads the complete #329 package delta at `112d3985aea1eb9b25ea5e27fabbac9d82a52bfe`.
Its exact-head source CLEAN agrees with this review. Both earlier LOW findings remain closed.
No source delta finding remains. The lead's pre-checks remain operator reports.
The completed train gate, final body, combined final receipt, and lead merge checks remain due.
This source delta accepts no later head, completed train, merge, or specific retry.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; #329 train 8 source delta only)
