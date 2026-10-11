# PC package review: a2_3 before Running, PR #277

VERDICT: CLEAN (0 open).

- READY: `msg_plugin-w_1791737722_0fd975`.
- Head: `7d7d429f06335c18e8a21e6c63085be4c5465eb0`.
- Own-diff base, PR base, and gate header base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
- Head parent: `f46a1fff431f30851d642447ebc32b3e5ef5425d`.
- Tier: HIGH. The change touches the start flow and a pending list.
- Contracts pin: `contracts-v0.1.36`, unchanged.
- Binding plan: 23bo, `~/botster-sessions/pins/stage1-plan.5c247504.md`.
- Plan SHA-256: `0c4aed22c9810d623b2b75e0da9073aff4d0d3d742db3db315f659d39d9caa7c`.

The reviewer read the PC brief first and followed its reading order.
The reviewer read BUILD.md, the applicable contracts and rulings, R-72, the design note, and the complete eight-file own diff.
The reviewer checked the current PR body, the pinned a2_3 transcript, the startup paths, adoption guards, and route cleanup.
The reviewer applies the previously read orchestrate-delivery skill to R-72's change in the classification premise.
This record uses the existing PC handoff. No new agent or workflow is required.

## Source conclusion

R-72 requires Core to classify what failed. It must not classify a worker death by the order of two edge reports.
The new `StartPhase::LinkLost` records the link end of a worker that this host spawned before Running.
Core keeps the session Starting and preserves the existing startup deadline. The helper posts no event.
`flow_waiting` excludes this phase from runnable session work. `run_start` performs no step in this phase.
The worker exit report still enters the existing `on_process_exited` path.
That path completes the start with `StartFailed(WorkerFailed)` and `Lost(WorkerGone)`.

If the worker stays alive until the deadline, `startup_expired` selects the existing link-failure outcome.
It gives `StartFailed(WorkerFailed)` and `Exited{code: None, signal: None, cause: Other}`.
It clears the deadline through `fail_start`. It does not signal the worker or use `StartupTimeout`.
The PR body states this reason before and after. Only this case's completion time changes.
The general startup-timeout path keeps its existing signal and `StartupTimeout` outcome.

`on_link_closed` uses the wait for `AwaitHello`, `SendLaunch`, and `AwaitLaunched`.
Its preceding adoption arm retains `Lost(WorkerUnreachable)` for adopted starts.
An adopted start enters `SendLaunch` with its existing link. A link end in that phase enters the adoption arm first.
Running link loss stays outside this change. The helper's debug assertion checks the spawned-start invariant and existing deadline.

`SendLaunch` also uses the wait when `send_msg` finds no link.
That function returns false only when the session has no link.
This covers an early hello followed by link EOF during the identity-row write.
The row result then advances to `SendLaunch`, with the spawned identity and original startup deadline.
Core sends no launch on that missing link. This is the same failure boundary, within the approved spawned-start scope.

Routes attached during the wait remain in the session and its pending handoffs.
The unchanged failed-start path posts the end state before it starts route cleanup.
It captures all attached routes in `bound_at_end` and uses the existing close path.
Lost closes routes once with `SessionLost`. Exited closes the held routes with `SessionEnded{exit}`.
The first-reason rule and route transport cleanup stay unchanged.

The new report-order test checks both EOF-first and exit-first cases with an attached route.
It checks no event at EOF, Starting during the wait, one Lost state, and one SessionLost close after that state.
The deadline test checks the unchanged reason and Exited value, with no signal.
The identity-row test checks the additional missing-link path. The existing pressure test preserves an already chosen Exited failure.
The reviewer traced each stated revert to the corresponding failing assertion. The reviewer executed no revert or test.

Only `conf::a2_3_lost_session_closes_routes_session_lost` leaves `core-real-pending.txt`.
The testkit already runs this ID must-pass. The real tier now does so on the supplied exact-head PASS.
No contract, transcript, timeout value, pin, worker, unsafe code, or mutation profile changes.
The Prior art note rejects the old immediate-EOF behavior under R-50 and R-72.
The reviewer read that old implementation and its named test at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The PR reuses no old mechanism and adds no hand-written substitute for a component.

The body lists six other classification places. The reviewer checked their cited source paths.
The body distinguishes races, a case that is not a race today, and the related adoption-connect case.
This PR changes none of those places. Plan 23bl leaves their later triage to the lead after #277 merges.
This CLEAN closes no separate adoption, Running-worker, stop, or route-input gap.

## Evidence

The reviewer read PC's retained macOS logs in `shared/core-stage1/evidence/pc-a2-3/`.
The fixed a2_3 trial passes six times, in 1.71 to 2.06 seconds.
The immediate-failure control fails three times at seed 0, step 29, with SessionEnded before the required Lost state.

The reviewer read the A5-4 script, all four tables, and the result lines and summaries in all four raw logs.
Each table and raw log contains 690 unique transcript results. Each raw result agrees with its table.
Testkit before and after: 209 PASS, zero FAIL, 481 ignored. The tables are byte-identical.
Real before: 179 PASS, one FAIL, 510 ignored. Real after: 180 PASS, zero FAIL, 510 ignored.
The only status change is a2_3 from FAIL to PASS. No PASS becomes non-PASS.
The comparison uses the same lists and worker, with the base engine as PC states in the PR body.

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-a2-3-lost-before-running-7d7d429f-pool-20261011-094729-72318.log`.
SHA-256: `d034fd8d28a3c205f3ad496eb0c25318ee8641d0c670b40f1712d08ab7334548`.
The header names the exact head and base above. All ten steps PASS in 316.6 seconds.
The Linux job exits 0 after 433 seconds. The wrapper exits 0 after 434 seconds.
Default: 1727 PASS, 481 skipped, 12.342 seconds. Slow: 554 PASS, 2021 skipped, 42.168 seconds.
Testkit a2_3 passes at line 557 in 0.231 seconds. Real a2_3 passes at line 4042 in 0.207 seconds.
All four relevant engine tests pass. The default test budget passes.
The minimum counts are testkit 56/69, real-passing 47/68, and real-accepted 48/69.
This branch does not yet contain #276's additional E1-1 proof.

Stage 1 tests six mutants and catches all six. Stage 2 does not run because stage 1 misses none.
The reviewer matched all six raw mutant identities with their six named catcher records.
The result has zero missed, zero timeout, zero unviable, and zero FLAKY outcomes.
Fuzz reports no changed crate with a decoder harness. The mutation profile is unchanged.
The reviewer ran no build, test, gate, mutation, reproduction, or executable merge-check job.

## Verdict scope

Package verdict: CLEAN (0 open) on the exact head above. No package finding remains open.
Sol's same-head integration verdict remains required.
After both CLEANs, PC must merge current v1 once, post the base-merge check, and run one final exact-head gate.
The body notes a newer v1 and an overlapping pending-list path. This verdict approves no later source delta or conflict resolution.
The original program-controls group remains incomplete. Wait for the doorbell. Never poll.

## Sol source CLEAN receipt

Receipt: `msg_plugin-w_1791741176_90ec42`.
Sol verdict: `2587b8498e51f36e034c8655ae2013be35e08ff1`, `verdicts/stage1/pc-a2-3-lost-before-running.md`, Round 1.
The reviewer read the complete verdict and verified that `origin/stage1/integration-review-sol` contains its commit.
Sol reports source CLEAN with zero open findings on `7d7d429f06335c18e8a21e6c63085be4c5465eb0`.
Both source CLEANs cover that exact head and base `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Sol retains the evidence limits: the compact logs do not independently prove the scratch checkout's provenance.
The separate engine reversals are PC's body reports. Neither reviewer executed or independently reproduced those reversals.
The same-head integration-review condition is closed. The current-v1 import, applicable base check, and final exact-head gate remain due.
Neither verdict approves a later source delta or gate. No execution job ran here. Wait for the doorbell. Never poll.
