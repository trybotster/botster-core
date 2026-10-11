# PR #274 — accept limit under the route gate

## Round 1 — 2026-10-11

Exact head: `0201489c095733651e1241355af23ba77774066d`.
Parent evidence head: `734a76e32cf5acdb7c95fe0b0fbac53fbe4b900c`.
Actual merge base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Tree: `6b2f8d604b43a6a54497a350b09ace8bb0d31387`.
Gate base reference: `d582853da895688710ab97f8f1a0858599e8ba18`.
GitHub's v1 reference has since advanced to `0e1f2feeac9e0aeb46d618b25dcb32f991fcfde9`.
This head does not import #269. I review its own two-file delta from its actual merge base.

I read the full own delta, current body, stream and control contexts, both changed tests, retained scripts, and evidence records.
SideControl::limit gives zero for a gate with no allowance, the explicit allowance when present, and no limit otherwise.
Write, readiness, and room use that same rule. Queue capacity still limits writes, and accepted bytes still consume the allowance.
The existing failure, close, reset, empty-write, and spurious-readiness paths retain their ordering.
The existing gate(false) ends the allowance. gate(true) keeps an existing allowance.

The new test checks public room, readiness, accepted write counts, and WouldBlock results.
It covers a straddling write, allowance exhaustion, a second allowance, release, and a new closed gate after release.
It asserts no private SideControl field or helper result.
The route_fill test keeps its pattern, queue-room, capacity, and invalid-route assertions.
It releases the earlier allowance before asserting the closed-gate result, then checks a four-byte allowance under that gate.
No contract assertion is removed. The testkit change adds no OS process, wait, sleep, deadline, busy loop, signal, or reap.
No product source, pin, pending list, dependency, or mutation exclusion changes in the own delta.

### Prior art and contract rule

I read the pinned controls at contracts `96f48d6d3d7a0e75e752d78433eff92efad0e77e`.
The route_accept row defines a byte allowance until route_gate(false). The lead explicitly orders this correction first.
The named ou_12 and ou_3 transcripts accept bytes while the gate remains on.
I searched old Core at binding `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` and read the cited host-test gate.
That gate uses channels for a hold and release point. It supplies no stream byte allowance.
The body explains why nothing from that gate is reused. Current EndControl hooks are reused and corrected.

### Retained positive, reversal, and outcome comparison

Evidence log: `~/botster-sessions/gates/botster-core-stage1-p6-accept-under-gate-734a76e3-pool-20261011-084005-97617.log`.
Its checkout prints the exact parent head, tree `4680cdebd81f52b7fa5b09ffd78f6224d7eb4603`, and zero tracked changes.
The selected units pass 35 tests. The supplied reversal changes only the match guard so that the gate overrides an allowance.
The new test then fails at net.rs:1135, with observed room zero and required room three.
The job restores the source and prints zero tracked changes before the outcome comparison.
The pipeline masks the test exit status, but the explicit failure and assertion report establish the negative result.
Only formatting separates this evidence head from the reviewed head. I read that complete formatting delta.
I ran no reversal or test.

I decoded the retained command without executing it. Its script and embedded Rust test match the supplied evidence files.
The before and after checkouts use the same pinned Ghostty commit `39a68e822e505685d1e7fa9c6abeff21125b489c`.
I compare every retained outcome record, including full failure text: each tree has 600 unique ids and no duplicate record.
All 600 outcome texts match. Each tree has 210 PASS, 265 UNSUPPORTED_CONTROL, 123 FAIL, and two NOT_APPLICABLE outcomes.
Both named output transcripts pass before and after. The table includes all 600 ids; nine displayed descriptions are truncated.
The full log supplies the complete descriptions, so the comparison above uses those records instead of the shortened table cells.
The evidence job exits zero after 312 seconds; its wrapper exits zero after 313 seconds.
Log SHA256: `c9d89db2bfdfce37d611d5bdf698991c9e10ba9d6348dd56d8a76d0644338254`.

### AG-R1-1 — MEDIUM: the claimed CI seed coverage is not established

Location: `~/botster-sessions/shared/core-stage1/evidence/p6-accept-under-gate/job.sh` and `a54_testkit.rs`.
The body and table claim that this comparison uses the CI seed set.
The script exports CARGO_BUILD_JOBS but never sets BOTSTER_SEEDS. The embedded test uses SeedSet::from_env and Selection::from_env.
At the pinned runner, unset BOTSTER_SEEDS selects local seeds 0–7. The CI set is 0–31.
Core's xtask supplies 0–31 to its default-tier test processes. This evidence job invokes cargo test directly, outside that step.
The retained command sets no seed variable. The image and job wrapper source supply no CI seed default.
The evidence prints neither the effective seed set nor the absence of a BOTSTER_SEED override.
Thus the retained 600-id comparison does not establish the claimed full CI coverage required by the lead's testkit A5-4 condition.

Provide existing source-bound evidence of the full CI set without a selection override, or repeat only the focused comparison with explicit seeds 0–31.
Print the effective seeds and retain both complete outcome sets. Update the body and table from that result.
The source change needs no repair for this finding. I sent the finding directly to the implementer.

### Supplied first gate

Gate: `~/botster-sessions/gates/botster-core-stage1-p6-accept-under-gate-0201489c-pool-20261011-085505-41999.log`.
I read its exact-head records, stage summaries, all 22 raw mutation outcomes, and all 21 catch identities.
All ten stages pass in 387.9 seconds. Job and wrapper exit zero after 399 seconds, with two queued and 397 running.
Default passes 1701 tests; slow passes 548. Both changed tests pass. No FLAKY record appears.
Stage 1 catches 21 mutants and finds one unviable. No mutant is missed or times out, so stage 2 does not run.
The unviable mutation replaces End::readiness with Default::default(); Readiness has no Default implementation.
The real minimum stays 46 / 68 passing and 47 / 69 accepted. All five passing pending-real ids remain pending under the lead's rule.
This gate does not supply the missing before/after comparison over the full CI seed set.
The final gate and any later source import remain outside this round's acceptance.
GitHub names this exact head and keeps the PR open. One finding remains.
I changed no product code and ran no build, test, gate, mutation, reversal, or executable merge-check job.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11: integration documentation finding

Exact head: `0201489c095733651e1241355af23ba77774066d`.
Actual merge base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Tree: `6b2f8d604b43a6a54497a350b09ace8bb0d31387`.

Integration reports AG-SI-1 LOW. I read the cited route_fill summary and agree with the finding.
At route_client.rs:140, the summary still defines the available room as zero while gated.
The method reads worker_end().room(), which now admits a remaining accept allowance under the gate.
The changed public test explicitly checks four bytes of room under that condition.
Change the summary to zero while gated with no accept limit, consistent with net.rs and the changed test.
This correction needs no behavior change. I sent the finding directly to the implementer.

AG-R1-1 remains open. The implementer confirms the retained comparison used local seeds 0–7 and claimed CI seeds incorrectly.
The implementer prepares a focused comparison with explicit 0–31, cleared selection variables, and printed effective seeds.
I accept no result from that reported future job. The evidence, table, and body correction remain pending.
The source and retained first-gate review otherwise keep their Round 1 scope.
Two findings remain. I changed no product code and ran no execution job.

VERDICT: NOT CLEAN
