# Core #277 — worker death before Running

## Round 3 — 2026-10-11 — final body and merge receipt

Verdict: CLEAN (0 open) at `2c78bb6c8424d37089062437731980ec0d34eab0`.
Sol reads the final body section. It now contains both exact-head delta verdicts and keeps the corrected catcher claim.
Sol reads the complete package final section at `d28b50691c644196856bc8fee44ed3efc39bdff6`.
The lead merges this head as `9f7085f703457b0669785697dbeb14ac687a8942`.
Sol verifies the exact parents: gate base `20c4e98df6b47a45f3e2284a976e1c8a1341457e` and the gated head.
The merge tree equals the gated head tree, `fed65ce895b71a25f22d1c9ac82d3344e9952580`.
All final conditions close. This acceptance covers #277 only.
The original program-controls group remains incomplete. The lead owns the six other classification places for separate triage.
No later pin or pending removal is accepted. Sol ran no execution job.

## Round 2 — 2026-10-11 — base imports and final gate

Verdict: CLEAN (0 open) at `2c78bb6c8424d37089062437731980ec0d34eab0`.
Base: `20c4e98df6b47a45f3e2284a976e1c8a1341457e`.
Parents: `79277df141aa6c5b18f150ed47015bd733b5efc1` and this base.
Package CLEAN: `5ee8cfb2ef82e5e68d3f9504590a868408e53913`, Round 2, read in full.
Sol read the full current PR body and the full failed base-check report.
The lead ordered the second import and stopped the stale gate after #276 moved v1.
The stopped 79277df1 gate supplies no completed result. Plan 23bu records the exception and final-gate slot.

Report: `~/botster-sessions/shared/core-stage1/evidence/pc-a2-3/base-merge-check-2c78bb6c.out`.
SHA256: `fcf69ab5cd1353ea0fde08595497f07ef002dc3e3fa598812e74e85031ec086a`.
The report passes ancestry and conditions 1–3. It fails condition 4 for the real pending file.
The base adds identifier rows, comments, and a blank line. It is not a pure removal of identifier rows.
This round supplies the required explicit delta review. Q7 automatic carry does not apply.

Sol independently compares the complete canonical own diffs outside both pending files.
They are byte-identical: 15231 bytes; SHA256 `f330332690061aad54448cce4aabee47a9d401e0830ee34efc6b80a7b17f1819`.
All seven own source and test blobs equal the original reviewed blobs.
None of the nineteen changed base paths overlaps an own path outside the pending files.
All imported paths outside own scope equal the base blobs. The testkit pending file equals the base exactly.
The real pending file equals the base minus only `conf::a2_3_lost_session_closes_routes_session_lost`.
Sol reads the complete base change to that file. The imported TM-6, OU-9 attach, and ST-5 rows remain present.
The static merge tree of the original reviewed head and current base equals the final head tree:
`fed65ce895b71a25f22d1c9ac82d3344e9952580`.
The unchanged source preserves the R-72 worker-exit classification, startup deadline, adoption guard, and route-close order.
Round 1's scratch-checkout provenance and engine-reversal evidence limits remain in force.

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-a2-3-lost-before-running-2c78bb6c-pool-20261011-105545-34013.log`.
SHA256: `00ac6621cdd278fa7836f913156836da5423592010e4693ae257231a2c44c904`.
The header names this head and base. Sol reads all ten step results, suite summaries, named proofs, and mutation results.
All ten steps pass in 136.3 seconds. Default: 1740 passes and 478 skips. Slow: 558 passes and 2027 skips.
Both A2-3 trials and all four relevant engine tests pass. The four named real-process proofs pass.
Minimum counts: testkit 60/69; real-passing 49/68; real-accepted 50/69.
Stage 1 catches all six mutations. All six raw caught identities equal all six reported catcher identities.
Stage 2 does not run because stage 1 misses none. Result: zero missed, zero unviable, zero timeout.
The complete log contains no FLAKY token. The fuzz step passes because no decoder crate changed.
The job exits zero after 144 seconds: one second queued, 143 seconds running. The wrapper exits zero after 144 seconds.

The package LOW body finding closes. The corrected body states that two catcher names differ from the first gate.
The body records both imports, the stopped gate, the failed check, the package delta verdict, and the final gate.
The owner must add this same-head integration verdict before merge. The gate base must still equal v1 at merge.
Three real-pending trials pass in this gate. The lead assigns their separate removal and repeated evidence to P5's pin PR.
This PR removes none of those rows. This verdict accepts no later head, pin, or pending removal.
Sol ran no execution job. Wait for the doorbell. Never poll.

## Round 1 — 2026-10-11

Verdict: source CLEAN (0 open) at `7d7d429f06335c18e8a21e6c63085be4c5465eb0`.
Risk: HIGH. Own-diff, PR, and supplied gate base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Package CLEAN: `0f4d349fcf6cd4b681bd064264a705121877ec55`, `verdicts/pc-a2-3-lost-before-running.md`.
Sol read the complete package record, eight-file own diff, full live body, design section 2, R-72, and pinned A2-3 transcript.
Sol read the relevant startup, adoption, deadline, process-exit, and route-cleanup paths.
The architectural premise changes under R-72. Sol applies the previously read orchestrate-delivery skill to that change.

### Classification and interfaces

The old engine chooses Exited if a starting worker's link closes first. It chooses Lost if the worker exit arrives first.
R-72 allows Core to wait for the worker exit or the existing startup deadline. It requires the classification to follow the failure.
StartPhase::LinkLost records that wait for a worker that this host spawned.
start_link_lost preserves the original deadline, keeps Starting, and posts no event.
flow_waiting excludes that phase from runnable session work. run_start performs no step in that phase.
The existing process-exit path fails the start with WorkerFailed and Lost(WorkerGone).
The deadline path fails it with WorkerFailed and Exited{code: None, signal: None, cause: Other}.
That deadline path sends no signal and does not use StartupTimeout. fail_start clears the deadline.
The general startup-timeout path retains its existing signal and StartupTimeout outcome.
No new timer or changed timeout value occurs. The live-link case now ends at the deadline rather than at EOF.

on_link_closed uses this wait in AwaitHello, SendLaunch, and AwaitLaunched.
Its earlier adopted-start arm preserves Lost(WorkerUnreachable). Running link loss stays outside this change.
The adoption paths retain their authenticated link when they enter SendLaunch. Their link-end arm runs before the spawned-start arm.
The helper checks the spawned identity, non-adopted flow, and existing deadline with a debug assertion.
flow_spawned sets that identity and deadline before RowIdentity.

SendLaunch also enters the wait if send_msg finds no link. send_msg returns false only when that link is absent.
This covers an early hello followed by link loss while the identity row is written.
The row result advances to SendLaunch with the original deadline. Core sends no launch on the absent link.
That path stays within the approved spawned-start failure boundary.

The wait preserves attached routes and pending handoffs. The existing failed-start path posts the end state before route cleanup.
It records bound_at_end and uses the existing close path. Lost gives SessionLost; Exited gives SessionEnded with that exit.
The first-reason rule and transport cleanup stay unchanged.
The report-order test covers EOF-first and exit-first with an attached route.
It checks silence and Starting during the wait, one Lost state, and one SessionLost close after that state.
The deadline test checks the exact failure reason and Exited value, with no signal.
The identity-row test checks the missing-link SendLaunch path.
The existing queue-pressure test keeps an already selected Exited failure when a later process exit arrives.
Sol can trace the stated local reversals to these assertions. The body reports those engine reversals; no separate raw reversal log accompanies them.
Sol runs no reversal or test and makes no independent execution claim for those engine controls.

Only `conf::a2_3_lost_session_closes_routes_session_lost` leaves core-real-pending.
The testkit already runs it must-pass. The supplied Linux gate passes it on both tiers.
No contract, transcript, pin, worker, unsafe code, or mutation profile changes.
The old Core pin `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` uses immediate EOF failure.
Sol reads that path and its named startup test. R-50 and R-72 reject that mechanism; the PR reuses none of it.
The body lists six other classification places. Sol checks their cited startup, adoption, stop, payload-exit, and worker paths.
Those paths remain unchanged. This verdict closes no adopted-worker, Running-worker, stop, or route-input gap.

### Retained evidence

Evidence directory: `~/botster-sessions/shared/core-stage1/evidence/pc-a2-3/`.
Sol reads both complete compact Mac logs. The fixed A2-3 trial passes six times, from 1.71 to 2.06 seconds.
The immediate-failure control fails three times at seed 0, step 29. Each returns SessionEnded before the required Lost state.
These are retained local comparison records, not a final-head Mac gate.

Sol reads the A5-4 script and statically checks every result in all four raw logs against its table.
Each log and table has 690 unique identifiers. All raw statuses match their tables.
Testkit before and after: 209 PASS, zero FAIL, 481 ignored. Every identifier retains its status.
Real before: 179 PASS, one FAIL, 510 ignored. Real after: 180 PASS, zero FAIL, 510 ignored.
Only A2-3 changes from FAIL to PASS. No PASS becomes non-PASS. All identifier sets match.
The body states that the before run uses the base engine with the same lists and worker.
Sol verifies the retained status comparison; Sol does not independently prove the scratch checkout's provenance from those compact logs.

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-a2-3-lost-before-running-7d7d429f-pool-20261011-094729-72318.log`.
SHA-256: `d034fd8d28a3c205f3ad496eb0c25318ee8641d0c670b40f1712d08ab7334548`.
The header names the exact head and base above. All ten steps PASS in 316.6 seconds.
Default: 1727 PASS, 481 skipped, 12.342 seconds. Slow: 554 PASS, 2021 skipped, 42.168 seconds.
A2-3 passes in 0.231 seconds on testkit and 0.207 seconds on real. All four relevant engine tests pass.
Minimum counts: testkit 56/69, real-passing 47/68, real-accepted 48/69. This head does not include #276's E1-1 proof.
Stage 1 catches all six mutants. All six raw caught identities match their six named catcher records.
Stage 2 does not run because stage 1 misses none. Final result: six caught, zero missed, zero timeout, zero unviable.
The complete log has no FLAKY token. Fuzz selects no changed crate with a decoder harness.
The job exits 0 after 433 seconds, with 42 seconds queued and 391 seconds running. The wrapper exits 0 after 434 seconds.

### Remaining work

Both source CLEANs now cover this exact head. The active Core Contracts pin remains .36.
PC must merge current v1 once, supply the applicable base check, and run one final exact-head gate.
Sol verifies merge object `84b1e9be2c58044b27958ed39fe1fffa2a0eb907` as the current reported base.
That object merges b6fd4b98 and #275's ad1a3809. This statement accepts no unread final gate for #275.
This verdict accepts no later import, conflict resolution, gate, pin, or pending removal.
The original program-controls group remains incomplete.
Sol ran no build, test, gate, mutation, measurement, reproduction, or executable base check. Wait for the doorbell. Never poll.
