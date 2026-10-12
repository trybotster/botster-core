# Core #314: integration review

## Round 1

- Reviewer: Sol-B.
- Lead assignment: `msg_plugin-w_1791820076_b7954f`; #314 goes first in train 4.
- Exact head: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
- Tree: `e6bd7b251e303cee5f2634705d683c2205d1273a`.
- Source and gate base: `080987829a2e2245a7ea97606020400392c974d6`.
- Risk: STANDARD; one test file in core-host.

Sol-B read the complete delta, full test helper, full body, completed gate, and focused mutation evidence.
The source changes no production behavior, API, conformance status, mutation exclusion, contracts pin, or timing allowance.

The seeded policy and two-session start/run/stop workload remain unchanged.
The helper returns every public scheduler observation: choice point, candidate count or bound, and returned answer.
For seeds 0–15, the test compares two complete traces and requires a nonempty trace.
It requires at least one trace to differ from seed zero's trace. It does not require all sixteen traces to be distinct.
The Arc ownership check concerns the recorder fixture's lifetime; the behavioral assertions inspect public scheduler observations.
No process, sleep, wait, reap, or busy loop is added.

The test removes the permanent BEFORE_E1 fixture, digest, and ignored trace printer under the lead's ruling (c).
The module retains the earlier comparison's history and accepted train-2 gate reference.
The earlier E1 before/after proof remains accepted at its original heads and workload scope.
This new test checks determinism on one head. It supplies no new comparison with the pre-E1 implementation.

### Focused mutation preservation

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-p6-trace-evidence-3e7fed26-pool-20261012-083210-88748.log`.
SHA256: `1cf43e25a58450dfe35441d5901887837a7eb382fcb974ce1410a459618a8f28`.
The wrapper names v1 as its base. Its recorded command explicitly changes BOTSTER_CI_BASE_REF for the mutation step.
The effective mutation base is `7c16332a74a600335fbbc1e671214f6e43db81e3`.
The evidence head is `3e7fed2668a0b36c4b69176c2ceb4a14515f94f4`.

Sol-B independently inspected both synthetic commits and their parent chain.
The evidence head's full tree equals this PR's tree.
Its landing diff changes only scheduler.rs, driver.rs, and engine.rs.
Every added and deleted line in those three files matches the accepted E1 source patch at the first train-2 merge.
This evidence covers those production source files. It supplies no new mutation run of every testkit E1 control.

The unmutated baseline passes. Stage 1 reports 26 mutants: 25 caught, one unviable, zero missed, and zero timeout.
Sol-B independently matches all 25 raw catches to citations and normal PASS in the exact-head full gate.
Those identities exactly equal train 2's caught subset for the three selected source files, including line, column, and change.
No credited train-2 catcher for those identities is the removed historical trace test.
The new records establish those same valid catches without the historical comparison.
The unviable begin replacement supplies no catch evidence. Stage 2 does not run because stage 1 misses none.
The mutation run takes 37.5 seconds. The job exits 0 after 101 seconds; the wrapper exits 0 after 102 seconds.
The recorded queue time is one second. No FLAKY record appears.

### Full gate and package receipt

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-seeded-trace-determinism-a4eac3c1-pool-20261012-083442-4144.log`.
SHA256: `232026590f75fbfb4cabad3363ea8f8649ebe6bb96ae779dcef441dff5533834`.
Sol-B read the exact-head/base header, changed-test result, tier summaries, gate decisions, and final exits.
All ten steps PASS in 466.3 seconds. The new determinism test passes in 0.030 seconds.
Default passes 1,935 tests, with 430 skipped. Slow passes 619, with 2,068 skipped.
The source and gate base are equal to the merge base. No conformance identifier changes.
The full gate starts no mutation run because this test-only diff has no mutant.
The separate source-scope job above supplies the mutation preservation evidence.
Fuzz reports no changed decoder-harness crate. No FLAKY record appears.
The job and wrapper exit 0 after 537 seconds, with one second queued and 536 seconds running.

The full body identifies the source-only mutation scope, synthetic evidence branch, equal tree, full gate, and earlier comparison history.
Sol-B verifies the old-Core Prior-art search at 72b2e335. It finds only the archived fake seeded-state note, not a scheduler counterpart.
Sol-B read complete package verdict `fe8837e9d4772705288ba85163ed815cadebd45a`, `verdicts/p6-seeded-trace-determinism.md`, Round 1.
It accepts this exact head, tree, base, source, and evidence with zero findings. Its conclusions agree with this audit.

### Disposition

No source, body, or evidence finding remains on this exact head.
This CLEAN accepts the source and initial gate/evidence. It accepts no later import, train, or contracts pin.
The lead's train-4 procedure, exact delivered-head gate, final receipts, and equal-tree/push verification remain due.
Sol-B changed no product code and ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open)

## Round 2: train 4 source delta

Exact train head: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`.
Tree: `4bb9e8c7f7d6e8b610e4b80ce782499ae3389cde`.
Base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Accepted #314 member: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
Lead assignment: `msg_plugin-w_1791821464_bb4328`. Sol owns the combined review; Sol-B owns this #314 delta.

Sol-B reads the three merge objects and ordered first-parent chain.
Merge `b3f7f39b72bc2a985634240573b2d08660b162c6` joins this base and #314.
Merge `13fc69a0a196f75a6aaa1662c1deac1817e5dd88` adds #312 member `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
The final merge adds #307 member `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
The first merge changes only the seeded trace file. Its added and deleted lines match the accepted #314 patch exactly.
The member, first merge, and final train contain the same file blob. Later members do not change it.
The driver retains the seeded_trace module. The public Scheduler source has no train delta.

Sol-B reads the shared driver fixture changes and relevant control-room, engine, and testkit changes from #307.
Each trace run creates a fresh recorder, seeded policy, and Rig. The existing setup uses no global trace state.
The mock link's new room override defaults to None, which returns unlimited edge room.
The trace workload starts and stops two sessions. It sets no room override and calls no cancel.
The new room-processing path requires a session that waits after a cancel refused Full.
That path therefore does no work in this trace workload.
The trace still records public choice points, candidate counts or bounds, and answers.
The full-trace equality, non-empty trace, and seed-variation assertions remain unchanged.
This composition adds no private production-state assertion to #314.

Sol-B reads exact-head package receipt `f852df5d103aefa6f97ff1314083506a1845dc76`, its complete Round 2.
It agrees with these source checks and reports zero findings.
The original mutation preservation evidence and full gate retain their original head and scope.
Neither record supplies a train 4 execution result. This receipt accepts no running gate or pre-check report as a final gate.
The completed exact-head train gate and final evidence body and receipts remain due.

No #314 source delta finding remains on this exact train head.
This verdict covers the preserved #314 test and its shared fixture composition only.
Sol's combined review and the other members' package reviews remain separate requirements.
This verdict accepts no later train head, pin, status removal, or #306 retry.
The lead owns the final train procedure and merge/tree/push verification.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; #314 train 4 source delta only, final train evidence pending)

## Round 3: train 4b base-change delta

Exact head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
Base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Previous train: `03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8`, based on `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
The frozen #314 member remains `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
Lead request: `msg_plugin-w_1791825491_49fc38`. Sol owns the combined review.
This round covers #314's requested base change only.

Sol-B reads the three new merge objects and verifies the unchanged member order.
Merge `471c0dece49519bcd4f31932eb8bef449d723233` joins the new base and #314.
Merge `89808877dc1e5664cd4ec59877c4980bc064dbf4` adds #312 member `c0f8a09cccf4c2c1163ac3a2b1609d5e258afa64`.
The final merge adds #307 member `2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54`.
The first merge changes only the seeded trace file. Its normalized own patch matches the accepted member exactly.
The member, first merge, and final head contain the same trace file blob.
The complete host tree remains identical between the two train heads.
The public full-trace equality, non-empty trace, and seed-variation assertions remain unchanged.
Round 2's shared-fixture review carries without a new fixture or scheduler change.

Sol-B independently compares the complete train delta with the complete base delta, using full-index Git diffs.
They are byte-identical. SHA256: `7d1572807eb38d9abd05ebd26e4c76db5cbc27bf48e2674089308f1b85b8dde6`.
Both deltas change only ci/known-flaky.toml and xtask/src/known_flaky.rs from landed #317.
No conformance set, host source, scheduler source, mutation configuration, or contracts pin differs between the trains.

Sol-B reads both changed files and the relevant exact-name filter implementation.
The new entry names only botster-core::slow_conformance and conf::ev_2_mandatory_and_keyed_never_dropped.
It records diagnosis R107-SI-2, owner RC, and removal PR #316.
The unchanged filter subtracts exact binary/test pairs from mutation lists. It cannot match #314's host test.
The xtask source changes only its repository-list test to expect that entry.
It changes no host workload, source assertion, scheduling policy, or normal-test retry.
Sol-B checks the 25 earlier synthetic catch citations. None names the newly excluded test.
That mutation evidence retains its earlier exact head and three-file scope. It supplies no train 4b execution result.
The train still contains the .44 pin. The new base and members contain no #316 pin or entry removal.

The earlier train remains RED.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-4-03709a0c-pool-20261012-092150-15636.log`.
Sol-B reads its named failure and completed summaries.
It reports ev_2 INCONCLUSIVE at seed 0, step 71, limit step_timeout. Slow fails; mutants and fuzz do not run.
The final exit is nonzero. This source review does not relabel the failed gate or supply a new gate result.
The lead reports an xtask check. This receipt accepts that statement only as an operator report.

Sol-B reads package verdict `6ba694457afca3f1e61a7dac0aa708bb203786ce`, its complete Round 3.
It agrees with the exact-head source checks and reports zero findings.
No #314 source delta finding remains on this head.
This receipt does not replace #317's independent reviews, the other member reviews, or Sol's combined review.
The fresh exact-head train 4b gate, final member evidence bodies and receipts, and lead merge verification remain due.
This receipt accepts no later head, pin, status removal, or additional re-gate.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; #314 train 4b base delta only, final train evidence pending)

## Round 4: final #314 receipt on train 4b

Exact head: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Tree: `ad98644e8be65b95b5fa03c39b350dee5d5a2502`.
Gate and merge base: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`.
Frozen #314 member: `a4eac3c100eeaa7c4290329c01ce0d530e3e1c46`.
Lead request: `msg_plugin-w_1791827181_b71169`.
Round 3's exact-head source acceptance remains unchanged. This round supplies #314's final evidence receipt.
Sol owns the combined final review.

Completed log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-4b-e37b91d6-pool-20261012-102808-61022.log`.
SHA256: `97dee8f4d6269dee403c2ff9202709e83d03817bcafd05c04b10cff7725b5d47`.
Sol-B independently verifies the hash and reads the identity, base accounting, affected tests, raw mutations, citations, summaries, and exits.
The gate and merge base both equal the recorded base. All ten steps PASS in 962.9 seconds.
Default passes 1,957 tests; slow passes 633 tests.
The determinism test passes in 0.025 seconds. The real contracts-checkout comparison passes in 0.099 seconds.
Strict conformance passes 264 tests without failure. Held-full runs three budget IDs at 32 seeds without failure.
Pending remains 403; real-pending remains 34. Those counts include the other members' accepted changes.
This receipt approves no additional status removal for #314.

Stage 1 tests 65 mutants: 39 caught, 14 missed, 12 unviable, and zero timeout.
The unmutated slow baseline passes all 633 tests. Stage 2 catches the 14 exact stage 1 misses.
Sol-B independently compares those missed and caught identity sets; they match.
The final result has 53 caught, 12 unviable, zero missed, and zero timeout.
All 53 unique raw catch identities match their citations. Every catcher has a normal PASS.
No runtime FLAKY or INCONCLUSIVE result appears.
The one INCONCLUSIVE string belongs to the stored known-flake diagnosis. It supplies no failing trial or catch.

The listed real EV-2 trial passes ordinary slow in 0.115 seconds and the unmutated baseline in 0.119 seconds.
Its exact binary/test pair is excluded from stage 2 mutation runs. No credited catch names that trial.
These ordinary passes remove no known-flaky entry and establish no structural repair.
#314 adds no production mutant. Its earlier synthetic evidence retains its original tree and three-file E1 scope.
This train gate supplies no new mutation run of every E1 control.
The link_decoder and msg_decoder fuzz runs complete 6,143,074 and 3,614,151 cases, respectively.
The job and wrapper exit 0 after 1,064 seconds on msa1, with one second queued and 1,063 seconds running.
The earlier train 4 gate remains RED with its recorded failure and nonzero exit.

Sol-B reads the full live #314 body before and after P6 adds the final Train gates section.
The update names the two train heads, bases, member order, logs, current hash, affected test times, and completed results.
It retains the original PR gate and synthetic mutation evidence with their original scopes.

**B314-I2 — LOW — final RED-train disposition clarity. CLOSED.**
The first update groups the PTY ack boundary and A31 tail drain under the RED train disposition.
Sol-B reads R107-SI-2 and the lead handoff. That ruling separates EV-2's fence fix from A31's OU-7 obligation.
Sol-B requests the clarification in `msg_plugin-w_1791827357_b0fc16`.
P6 corrects the body in `msg_plugin-w_1791827375_c3bcda`; Sol-B independently reads the corrected live body.
It states that .44 lacks the fence between the title request and Stop, and contracts #89 adds the title fence.
It states that #316's pin removes the temporary entry. The retained logs identify neither earlier SI-1 possibility.
It keeps A31's separate OU-7 ID outside the EV-2 fix. The failed gate remains recorded.
No source head changes through this body correction.

Sol-B reads package receipt `4d21704f3a4ae5f8f9246da979f19932eabb3e92`, its complete Round 4 gate audit.
The package reviewer places that receipt on hold for the body clarification. Sol-B publishes no final CLEAN during the hold.
Sol-B then reads replacement final package receipt `efa99fb22d75cb86a0f9cfdde98e011933d095a4`, its complete Round 5.
The replacement closes the body hold and supersedes the earlier final receipt. Its unchanged gate audit agrees with these checks.
The replacement reports zero findings on the same exact head, tree, base, and completed gate.
No #314 source, final-body, or supplied-evidence finding remains on this exact train head.
This final member receipt does not replace the other member reviews or Sol's combined final review.
The lead must still verify the final merged tree and remote refs.
This receipt accepts no later head, pin, status removal, additional re-gate, or completed merge.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; #314 final receipt on the completed exact-head train 4b gate)
