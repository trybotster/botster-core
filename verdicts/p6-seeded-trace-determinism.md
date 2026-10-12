# P6 review: seeded trace determinism — PR #314

## Round 1 — 2026-10-12

Exact head: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
Source and gate base: `080987829a2e2245a7ea97606020400392c974d6`.
Tree: `e6bd7b251e303cee5f2634705d683c2205d1273a`.
Risk: STANDARD. The delta changes one test file in one package and matches no high-tier path.

I read the complete one-file delta and the complete PR body after GitHub MCP truncates it.
Body SHA256: `12491bb9cf04e0d1dcf782de552215c99e13285af3463e2bd68b5737756a74a0`.
The body attributes the change to lead ruling (c). The earlier E1 before/after proof remains in its retained logs and accepted #300 verdicts.
This PR replaces that permanent historical fixture with a determinism assertion on one head, under Core A5-1.

The policy and two-session workload remain unchanged. The recorder still observes public choice points, candidate counts or bounds, and returned answers.
The helper now returns the complete trace. It removes the digest, BEFORE_E1 constants, and ignored trace-print test.
For each seed 0-15, the test compares two complete traces and requires a non-empty trace.
It also requires at least one seed's trace to differ from seed zero's trace. A fixed empty or identical trace cannot satisfy the test.
The assertions inspect public scheduler observations, not production state. The Arc ownership check concerns only the test recorder's fixture lifetime.
The module documents the earlier proof and cites its accepted train gate.
No production behavior, API, status file, mutation exclusion, or contracts pin changes.
No real process, sleep, wait, reap, or busy loop is added. I found no source defect.

### Mutation evidence

Log: `~/botster-sessions/gates/botster-core-p6-trace-evidence-3e7fed26-pool-20261012-083210-88748.log`.
SHA256: `1cf43e25a58450dfe35441d5901887837a7eb382fcb974ce1410a459618a8f28`.
I read the command, effective base record, raw outcomes, named catches, baseline, summary, and exits.
The wrapper header names the real v1 base. The command explicitly overrides BOTSTER_CI_BASE_REF for the mutation step.
That step reports effective base `7c16332a74a600335fbbc1e671214f6e43db81e3`.
The evidence head is `3e7fed2668a0b36c4b69176c2ceb4a14515f94f4`.
I independently inspect both synthetic commits. The base descends from the PR head; the evidence head descends from that base.
The evidence head's full tree exactly equals the PR tree above.
Its landing diff changes only scheduler.rs, driver.rs, and engine.rs.
Its added/deleted lines exactly match E1's accepted patch in those three source files at the first train-2 merge.
This evidence tests that source-file scope. It does not claim a new mutation run of every testkit E1 control.

The unmutated baseline passes. All 26 raw mutant identities are unique: 25 caught and one unviable, with zero missed or timed out.
All 25 raw catches match their named catches. Every named catcher passes normally in the exact-head full gate.
All 25 identities also occur as caught identities in train 2's accepted gate, by file, line, column, and change.
None of the credited train catchers for these identities is the old trace test.
The records therefore preserve those catches after the historical comparison is removed.
The unviable begin replacement supplies no catch evidence. Stage 2 does not run because stage 1 misses none.
The mutation stage reports 37.5 seconds. The job exits zero after 101 seconds: one queued and 100 running.
The wrapper exits zero after 102 seconds. The retained log contains no FLAKY record.

### Full gate and Prior art

Log: `~/botster-sessions/gates/botster-core-stage1-p6-seeded-trace-determinism-a4eac3c1-pool-20261012-083442-4144.log`.
SHA256: `232026590f75fbfb4cabad3363ea8f8649ebe6bb96ae779dcef441dff5533834`.
I read the head/base header, lists/conformance records, changed-test result, tier/stage summaries, and final exits.
All ten stages pass in 466.3 seconds. The new determinism test passes in 0.030 seconds.
Default passes 1935 tests, with 430 skipped, in 15.844 seconds. Slow passes 619 tests, with 2068 skipped, in 41.500 seconds.
The exact base is also the merge base. The lists retain 406 pending and 43 real-pending identifiers.
The testkit conformance result passes 261, with zero failures. Held-full runs three budget identifiers at 32 seeds.
The diff has no mutant, so the full gate starts no mutation run. The separate source-scope evidence above supplies the preservation proof.
Fuzz reports no changed crate with a decoder harness. The complete log contains no FLAKY record.
The job and wrapper exit zero after 537 seconds: one queued and 536 running.

I verify the Prior-art search at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its named search finds only the archived plan's fake seeded-state note. It finds no old scheduler or choice-trace counterpart to reuse.
The note rejects that item as a counterpart and uses the current public scheduler seam.

No package source or evidence finding remains for this exact head. The source and retained gate/evidence are CLEAN.
This verdict accepts no later head, import delta, contracts pin, or status removal.
Any later slot or train composition retains its separate review and gate requirements.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base-check job.

VERDICT: CLEAN

## Round 2 — 2026-10-12: #314 delta on train 4

Exact train head: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`.
Tree: `4bb9e8c7f7d6e8b610e4b80ce782499ae3389cde`.
Train base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Reviewed #314 input: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
Lead request: `msg_plugin-w_1791821464_80b187`.

I inspect the three merge objects and their ordered first-parent chain.
The first merge, `b3f7f39b72bc2a985634240573b2d08660b162c6`, has parents the exact train base and #314 input above.
The second merge, `13fc69a0a196f75a6aaa1662c1deac1817e5dd88`, adds #312 input `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
The final merge adds #307 input `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
The first merge changes only seeded_trace.rs. The subsequent merges do not change that file.
The final file is byte-identical to the reviewed #314 input, blob `0239e231ffea4525211d28bc753931e130506988`.
File SHA256: `a5fa1504ab5ea12099d3be93f3a9f7d31ba1a7256f636e4eaf73682ef630da38`.

I read the complete shared driver-fixture delta and relevant driver/deadline context introduced by #307.
The imported control-room edge returns unlimited room by default. The trace workload does not set a room limit or refuse a cancel.
The new room-processing path runs only for a session that waits for room after a refused cancel.
The trace still constructs a fresh policy, recorder, and Rig for each run, then creates, starts, and stops its two sessions.
The Rig constructor and to_launch helper retain their setup. The pump and settle helpers retain their finite step bounds.
The new control-room tests add no shared global recorder or random source to the trace fixture.
The recorder still observes the same public Scheduler interface. Its full-trace repeatability, non-empty, and seed-variation assertions remain unchanged.
The train base itself changes no host fixture or scheduler source from #314's original base.

No #314 source finding remains on this exact train head. The patch introduces no status removal or exclusion on the train.
This manual delta review covers #314's preserved test and its composition with the shared fixture changes.
It does not replace the other members' package reviews or the combined integration review.
The earlier mutation evidence and full gate retain their original-head source scope; neither is a train-4 execution.
The lead reports pre-check passes. This receipt accepts no completed train gate from those reports.
The exact train gate and final #314 evidence body/receipts remain due.
This verdict accepts no later train head, contracts pin, or status removal.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base-check job.

VERDICT: CLEAN

## Round 3 — 2026-10-12: #314 base delta on train 4b

Exact train head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
Train base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Previously reviewed train: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`.
Previous train base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Reviewed #314 input: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
Lead request: `msg_plugin-w_1791825491_62b81e`. Scope: #314's source delta from the new base only.

I inspect the ordered merge objects and their exact parents:

- `471c0dece49519bcd4f31932eb8bef449d723233` adds #314 to the new base.
- `89808877dc1e5664cd4ec59877c4980bc064dbf4` adds #312 input `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
- The exact train head adds #307 input `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.

The first merge still changes only seeded_trace.rs. The members and their order match the earlier train.
I independently compare the complete train delta with the complete base delta. They are byte-identical.
Delta SHA256: `9635ef61f78726c5a95dfc5d60c2d01324c31ba2355e7b0ebb0e3384038481c3`.
Both change only `ci/known-flaky.toml` and `xtask/src/known_flaky.rs`, from accepted #317.
The complete botster-core-host tree is unchanged between the two train heads.
The #314 file remains byte-identical to its reviewed input and the earlier train.
Blob: `0239e231ffea4525211d28bc753931e130506988`.
File SHA256: `a5fa1504ab5ea12099d3be93f3a9f7d31ba1a7256f636e4eaf73682ef630da38`.
The public full-trace repeatability, non-empty, and seed-variation assertions remain unchanged.
The earlier shared-fixture review carries without a new fixture or scheduler delta.

I read the complete two-file base delta and the relevant known_flaky filter implementation.
The new entry names only `botster-core::slow_conformance` `conf::ev_2_mandatory_and_keyed_never_dropped`.
The unchanged filter subtracts exact binary/test pairs from mutation-stage lists.
It does not exclude the #314 host test. It changes no normal-test retry, host workload, scheduling policy, or source assertion.
The xtask change updates only the repository-list test to expect that one entry.
None of the 25 catch citations in #314's earlier synthetic mutation evidence names the newly listed test.
That evidence retains its original head and three-file source scope. It supplies no execution result for train 4b.

I read #317's complete body after GitHub MCP truncates it.
Body SHA256: `bb5b1335a54be139851a7387d9291d929085f6b9636ef6e89d2a157c6aadf01e`.
GitHub records #317 merged at the exact new base. The entry names diagnosis R107-SI-2, owner RC, and removal PR #316.
I read #316's visible state and plan. It remains an open draft for the future pin and entry removal.
The train retains contracts-v0.1.44. Cargo.toml and Cargo.lock are byte-identical between the two trains.
No train member changes the base's known-flaky entry or its repository-list assertion. The removal trigger has not occurred.

The earlier train remains RED. I read its named failure and final summaries.
Log: `~/botster-sessions/gates/botster-core-stage1-train-4-03709a0c-pool-20261012-092150-15636.log`.
It reports `INCONCLUSIVE conf::ev_2_mandatory_and_keyed_never_dropped seed=0 step=71 limit=step_timeout`.
That signature matches the new entry. The slow step fails; mutants and fuzz do not run. The final exit is nonzero.
I read BUILD.md Testing rule 9 and the train rule. The known-flake exception permits only one additional gate under its three conditions.
This source verdict does not turn the earlier failed gate green or supply a train-4b gate result.
The lead reports an xtask check and plans one fresh gate after the six deltas. Those reports remain outside this source verdict.

No #314 source finding remains on this exact train head.
This CLEAN covers the requested base delta and preservation of #314's reviewed test.
It does not replace #317's independent reviews, the other member reviews, or the combined integration review.
The fresh exact-head train-4b gate, final evidence body and receipts, and lead merge verification remain due.
No later train head, contracts pin, status removal, or additional re-gate is accepted.
I changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN
