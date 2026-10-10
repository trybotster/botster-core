# PR #231 — Contracts v0.1.27 pin

## Source and evidence inspection — 2026-10-10 — Package verdict pending

Reviewed head: `4521237e8200768a36ab4c8a27f268f3914a88ab`.
PR and full gate base: `6c2bccdad9ab6cfac860e63c5eb15d6f8ec5ac6c`.
Tier: HIGH under BUILD rule 4, the contracts pin.
Authority: approved plan 23ab.
Scope: all four own files, pin and status accounting, affected default probe path, base merge, and exact-head gate evidence.
This entry records completed source inspection. A verdict round waits for the package verdict on this exact head.

All seven direct contracts dependencies use `contracts-v0.1.27`.
All nine contracts lock sources resolve to `b161db87ca82873b4f6b21d8433a69f086abb7cf`.
That is the published tag target already verified in plan Round 37.
After normalizing those nine source strings, the entire parsed lock file equals the base lock file.
There is no other dependency or feature change.
The manifest note retains the older tag history and names the new probe control path, ad_3 fence, R-52, and R-53.

All 477 Core pending id lines equal the base in order. Only tag comments change.
Pending-real, real-only, held, deferred, and minimum files equal the base bytes.
`conf::ad_3_adopted_equals_started` remains pending-real.
The new tag changes only its Activity output fence among transcripts; the ledger, frozen files, and manifest remain unchanged.
The pin move adds no re-pending exception and removes no id without proof.
The status test's changed line is documentation only and names the correct tag.

The contracts driver calls `normalize_op_env` with the harness's `probe_env`.
The new trait method returns an empty map by default. Core provides no override at this head.
The existing `normalize_op` also forwards with an empty map.
The probe takes its direct script path when `BOTSTER_PROBE_CONTROL` is absent.
The new control socket therefore remains for the later program-controls implementation.
This inspection does not accept that future implementation or its pending-id removals.

The head merges v1's accepted #229 correction into the original pin commit `df7f0942`.
All four own file bytes equal that original commit after the merge.
The automatic `df7f0942` plus `6c2bccda` tree and actual head tree both equal `0cd846cbefe57f0649924906bfd79569fa4fd545`.
The current remote PR head and base match the recorded head and gate base.
The remote v1 is that base. `git diff --check` passes.

Exact-head gate:
`~/botster-sessions/gates/botster-core-stage1-pc-pin-v0.1.27-4521237e-pool-20261010-145612-73744.log`.
Its header names the full head and base. The command is plain `cargo xtask ci`.
All ten jobs pass. The probe install names contracts-v0.1.27 and b161db87.
Default: 1498 passing tests, 13.237 seconds. Slow: 384 passing tests, 130.417 seconds.
The changed ad_3 transcript has a testkit PASS line.
Both conformance reports show 193 passed and zero failed.
Strict coverage remains 386/407, with 21 real-only transcripts and two held transcripts.
Minimum counts remain testkit 50/69, real 29/68, and real-accepted 29/69.
Pending-real remains 87, with zero newly passing ids.
The mutation diff lists no mutant, so no mutation run starts.
The gate exits zero after 816 seconds, including 294 queued seconds.

I found no source defect in this scope.
I requested the package verdict commit from the PC reviewer. Its same-head artifact is pending.
This inspection gives no CLEAN and completes no verdict round.
I changed no product code and ran no build, test, gate, mutation job, merge-check job, or reversal job.
The tree comparison uses Git source objects only.
