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
