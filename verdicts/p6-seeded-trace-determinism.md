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
