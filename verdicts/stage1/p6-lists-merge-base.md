# PR #294 integration review

## Round 1

- Reviewer: Sol-B.
- Branch: `stage1/p6-lists-merge-base`.
- Exact head: `11b6e2ac8a94dacad7b17876fc1b61e65ae233b5`.
- Reviewed source base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- Gate base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
- Reviewed delta: the complete one-commit change in `xtask/src/fsutil.rs` and `xtask/src/lists.rs`.
- Risk: HIGH. The PR changes gate decisions.

No integration finding is open. The change implements the lead-approved design at `shared/core-stage1/evidence/p6-lists-merge-base/design-note.md`.

All five comparison inputs use the same cached merge base: pending ids, deferred ids, ledger ids, the contracts pin, and real-pending ids. The current-tree inputs and rule decisions remain unchanged. The pinned-contract copy checks and checks without a base input retain their scope.

The recorded gate base retains its meaning for other callers. A failed merge-base query returns an error. The implementation does not substitute the base tip or an empty ancestor. When the reviewed head contains the recorded base, the merge base equals that base. The final merged-head gate therefore retains the shrink-only rules against current `v1`.

The tests distinguish base-side removals from branch additions in both pending files. The deferred-rule test rejects a branch deferral without its own pin move. The unrelated-history test checks refusal. The merged-head test checks the final comparison. Git fixtures use the existing shared process helper and cleanup deadline.

### Evidence

- Package CLEAN: `937a53a5cc4549781b4594cb9c9512ef2db75568`, `verdicts/p6-lists-merge-base.md`, round 1. Sol-B read the exact-head verdict.
- Full gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-lists-merge-base-11b6e2ac-pool-20261011-162744-42287.log`.
- The gate header names the head and gate base above. All ten steps PASS. The final exit status is 0.
- The actual lists step prints gate base `f6025aed4cdf7da836e835e0040fc128b7e8f32f` and merge base `5e953398925351c12e463a7b0a946b79d9841edc`. Git independently confirms that merge base.
- Mutation result: 6 total, 5 caught, 0 missed, 0 timeout, 1 unviable. The five raw catches match the named catches. Stage 2 requires no mutant.
- Focused evidence: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-lists-merge-base-11b6e2ac-pool-20261011-162658-37804.log`.
- All 44 selected tests pass. The focused clippy command exits 0. The scratch reversal returns the base tip; the selected test fails at `fsutil.rs:422` and exits 101. The script restores the file. The wrapper's exit 0 is not the negative proof.
- The PR body includes Prior art and preserves the earlier failed evidence attempt as invalid evidence.
- No contracts pin, conformance status file, mutation exclusion, terminal behavior, or production package changes.
- Sol-B ran no local build, test, gate, mutation job, or reversal.

### Merge limit

This CLEAN applies only to the exact reviewed head. The head does not contain the gate base. The final-slot import, required base check, applicable delta review, and final gate remain due. A qualifying disjoint base-only merge can carry this CLEAN under the standing rule.

VERDICT: CLEAN (0 open)

## Base-only merge receipt

- Reviewed source head: `11b6e2ac8a94dacad7b17876fc1b61e65ae233b5`.
- New head: `f6bdb3f8ff0836b3e85f534033dda08daf00416a`.
- Imported base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
- Check log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-lists-merge-base-f6bdb3f8-pool-20261011-164635-90284.log`.

Sol-B read the completed canonical check. The check reports PASS for ancestry, conflict-free merge, disjoint paths, and the unchanged PR diff. The PR diff is 19,263 bytes, SHA-256 `9a8509ac8d1b55e12e473ecb7491e19a5a8577e3eb6166fdcfff4fe548343a7a`.

The PR changes no line-set file. Both pending files in the new head equal the imported base's files. Git confirms that the new head has exactly the reviewed source head and imported base as its parents.

The standing base-only merge rule carries integration CLEAN to the new head without a delta round. The final gate is still pending. This receipt accepts no final-gate result or merge completion.

VERDICT: CLEAN (0 open)
