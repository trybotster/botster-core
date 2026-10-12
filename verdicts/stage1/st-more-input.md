# PR #295 integration review

## Round 1

- Reviewer: Sol-B.
- Branch: `stage1/st-more-input`.
- Exact head: `dcc53d8f94d64e94a5b0028c0bd1e86369456773`.
- Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- Reviewed delta: all three commits and all four changed files.
- Risk: HIGH. The host driver change crosses into the shared link crate.

No cross-package source defect was found. The driver uses the decoder's new non-consuming `ready()` query without bypassing the frame bound. Partial frames wait for a read. Complete frames wait for event budget or mandatory-queue room. A frame parked on queue room does not set `more_input`.

The one-exit slot preserves the process edge's taken exit. The retained link end follows the earlier frames. The next pump processes retained input through the existing driver and engine paths. No test branch or new edge method is added.

The decoder loop advances through a delivered frame or a read. The final commit removes the loop that caused the earlier mutation timeout. The unchanged byte-allowance case remains conservative, as the PR body states.

The PR records coordination with PC's #291: #295 lands first. The later #291 delta must use `end_link` and count its pending drain as runnable input. This verdict approves no #291 delta.

### ST-I1 — MEDIUM — OPEN: required evidence and package CLEAN remain pending

Package verdict: `81ce3b011ad2aeac59664bf1a3cfa15d85d963b4`, `verdicts/st-stall-clock.md`. Sol-B read the exact-head review. It reports NOT CLEAN with only `ST295-E1` open and no new logic defect.

The implementer announced a running Mac evidence job. Plan section 8, revision 23ai, requires focused Mac evidence for socket and process behavior. This driver reads link sockets and consumes process exit reports.

Required closure: supply the completed focused Mac log on the exact head. Link it in the PR. The selected driver, link, and real-process checks must pass. Supply the promised real-tier proof for both removed pending ids with `BOTSTER_SEEDS=0-31`. Obtain the package CLEAN on the same head.

An evidence-only reply can close ST-I1. A new source commit requires delta review.

### Evidence

- Linux gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-more-input-dcc53d8f-pool-20261011-161450-67786.log`.
- The header names the exact reviewed head and base. All ten steps PASS. The final exit status is 0.
- Mutation result: 19 total, 14 caught, 0 missed, 0 timeout, 5 unviable. Stage 2 runs no mutant because stage 1 misses none.
- The gate passes `conf::e3_1_due_step_beyond_budget_is_carried_with_more` and `conf::e3_1_two_silences_due_together_with_budget_one` in both tiers.
- The gate's testkit tier uses seeds 0-31. Its real tier uses seed 0. The pending list removes only those two ids.
- The PR body includes Prior art, the earlier red gate, the final repair, and the retained byte-allowance limit.
- The current `v1` tip has moved. The final-slot merge, required base check, and exact-head final gate remain due.
- Sol-B ran no local build, test, mutation job, or gate.

VERDICT: NOT CLEAN (1 open)

## Round 2 — evidence closure

Exact head: `dcc53d8f94d64e94a5b0028c0bd1e86369456773`. Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`. Delta: completed evidence only; no source commit.

ST-I1 is CLOSED. Package CLEAN: `4f6b28c21ac0f861ac5a1446b71dfc34e4884704`, `verdicts/st-stall-clock.md`. Sol-B read that verdict and the corrected PR evidence comment.

Mac log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-more-input-dcc53d8f-pool-20261011-162605-32994.log`. The header names the exact head. Prebuild passes. All 351 selected host and link tests pass. Both removed pending ids pass in real conformance with `BOTSTER_PENDING_STRICT=1 BOTSTER_SEEDS=0-31`. The named `slow_real_core` checks pass.

The broader real run passes 230 of 232 tests and exits 100. It is not a green run. Its failures remain recorded:

- `ev_5_route_closed_retires_route_keyed_events`, seed 1, step 28: `activity_before_close: false, closed: true`. The worker-input precondition has no consumption fence before Detach. R-66's follow-up requires that fence. The accepted controlled reproduction on base `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12` has the same signature at seed 0. Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-detached-f68f26a7-f68f26a7-pool-20261011-150206-64641.log`. This is a scratch timing reproduction, not a clean-v1 gate. The earlier P6 `9231c06d` citation has a different expected value and does not prove this signature. ST corrected the comment.
- `st_6_capture_is_frozen_at_model_rev`, seed 5, step 17: the revision comparison is 1 against 1. R-80 diagnoses the missing output fence before the model read. The RC log `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-encode-46c8c548-pool-20261011-163127-84246.log` has the same step-17 inequality failure at seed 0.

#295 changes timing but supplies neither missing fence. It changes no worker output handling or transcript. Its driver preserves the order of complete frames on each link; a held frame prevents the next read. These failures do not invalidate the passing focused checks or the two required full-seed proofs.

ST relays the lead's disposition: contracts #69 repairs the `ev_5` fence, and R-80 repairs the `st_6` fence. RC's .42 pin carries both. Neither live id is re-pended. A red merge gate remains red and follows the standing plan rule; this verdict authorizes no re-gate.

The exact-head Linux gate remains green. No integration finding remains open. The final-slot import, required base check, applicable delta review, and exact-head final gate remain due. Sol-B ran no local build or test.

VERDICT: CLEAN (0 open)

## Round 3 — final-slot delta and gate

Exact head: `113da2366286adcef4a71449c5d362c146f6754f`. Imported and gate base: `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035`. Sol-B verified the merge parents: the reviewed `dcc53d8f` head and this exact base.

Canonical check report: `/private/tmp/claude-501/-Users-jasonconigliari-botster-sessions-trybotster-botster-core-stage1-st-stall-clock/7276b210-c579-494f-8024-241f037e5424/scratchpad/bmc-295.txt`.

Both ancestry conditions and the conflict check PASS. Conditions 2, 3, and 4 FAIL: the driver path overlaps; patch metadata differs; the base changes a pending-list comment. This round supplies the required integration delta review. It does not change the canonical FAIL into a PASS.

The base's driver change adds `parked_work` diagnostics through `engine.parked_on_room` over held link frames. Sol-B read that method and the merged driver paths. With queue room, the diagnostic reports no parked work. Without room, it counts a held frame only when room alone would make the engine accept it. A budget-held frame therefore does not create a false queue-room report while room is available. The acceptance and readiness refactors preserve the driver's decisions. The diagnostic changes no frame delivery, event budget, decoder state, or `more` decision.

Sol-B independently compared all four own patches after excluding index and hunk-location metadata. Their content matches the reviewed patches. Sol-B also checked the four ID sets against the imported base plus the original PR changes. All match. The only own ID removals remain `conf::e3_1_due_step_beyond_budget_is_carried_with_more` and `conf::e3_1_two_silences_due_together_with_budget_one`. Held, ledger, and real-pending sets retain the imported base's entries.

Completed final gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-st-more-input-113da236-pool-20261011-191408-70407.log`. Its header names the exact head and base. The lists step names the same base as its merge base.

All ten steps PASS. Default: 1,861 passed. Slow: 610 passed. Both removed IDs pass in both tiers. Mutants: 19 total, 14 caught, 0 missed, 0 timeout, 5 unviable. Stage 2 runs no mutant. The log contains no FLAKY result. The pool job exits 0 after 716 seconds; the wrapper exits 0 after 718 seconds on msa1.

Package final CLEAN: `880e07989f393e0ba18559a014bd5c40ff54a0cd`, `verdicts/st-stall-clock.md`, section `Core #295: merged-head final receipt`. Sol-B read that exact-head receipt.

Sol-B read the complete updated final PR body. It names the final head/base, failed check, reviewed overlap and ID sets, final gate, and completed Mac evidence with both broader-run failures. The original Mac and full-seed evidence retains its recorded scope. This merge introduces no worker or decoder change beyond the reviewed patch. No integration finding remains.

Sol-B relied on the retained pool evidence for acceptance. Sol-B ran no local build, test, gate, mutation job, or reversal.

VERDICT: CLEAN (0 open)
