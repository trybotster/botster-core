# Core #291: R-78 source and evidence review, Round 1

VERDICT: NOT CLEAN (2 open)

Reviewer: sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7.
Implementer: sess-1791628587-0178-4476c8a4ce35c7df2dc295defd77dda5.
Reviewed head: `79f31be3b3213e362539a51c35a1a1273a5aee26`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Head tree: `6ff05266cf0ed9a366d5e7e21c1cb1f2d9af8f64`.
Risk: HIGH. This verdict covers source and retained evidence. It does not grant final-slot approval.

## Governing records

The reviewer read brief-program-controls.md first and followed its reading order.
The review uses the contracts, BUILD.md, R-78, and the subsequent lead instructions.
The current plan is 23cm, `stage1-plan.c77c2434.md`.
Its SHA-256 is `c77c243490be26d6e4ee9f5b9491fd8b62d6dc9827dceebccfd864e2d60abf02`.
The accepted design is revision 3, SHA-256 `52b9ada3f73a1fdf77c9f05b04775f2fffb4b6b5385f72149db77c6f99462885`.
Package acceptance is `a661545657a37bc4a1f68bfab15da7c1a5c41154`; Sol acceptance is `1c4aacd9ca68abc860103270258ead5d44ec2023`.
READY is `msg_plugin-w_1791759852_87afbc`.
The reviewer read all 27 changed files, the relevant callers, the tests, and the evidence records.
No reviewer build, test, gate, mutant, reproduction, or executable base check ran.

## R78-PC-1 — MEDIUM, OPEN: the process exit discards frames after Exited

Location: `crates/botster-core-host/src/inbound.rs:726`, `on_process_exited`.
The retention branch requires `drain_classifies`.
An `Exited` frame moves the Stop flow to `PostEnd` or `Finish`.
Those phases make `drain_classifies` false.
A subsequent `ProcessExited` therefore removes the link and fails the requests in flight before the read reaches its end.

The production worker sends `Exited` before it closes the session's routes (`worker.rs:825`, `report_exit`).
The worker later sends `RouteClosed` from `worker/route.rs:396`, `end_route`.
If the pump handles `Exited` before `ProcessExited`, buffered `Done` and `RouteClosed` frames can be discarded.
Without the delivery report, `finish_close` maps an undelivered route to `SessionLost` once the link is removed.
The existing already-Exited test checks the chosen state. It does not check these remaining frames.

This is an existing behavior that R-78 must correct. It is not claimed as a new regression.
The test `a_held_route_closes_session_lost_when_the_worker_process_ends_after_the_exit` pins the old #217 R2-1 behavior.
PC confirmed the mechanics in `msg_plugin-w_1791760315_51ce05`.
Sol independently confirmed the finding in `msg_plugin-w_1791760318_9afc1c`.

The lead's FINAL decision, `msg_plugin-w_1791760401_5c2be8`, keeps this finding on #291.
That decision supersedes the earlier transfer decisions.
The fix must handle complete frames after the process exit without blocking, until EOF or would-block.
The fix must not wait for EOF.
The lead requires evidence that both `Done` and `RouteClosed{SessionEnded}` after `Exited` are delivered.

Required change: separate link retention from the decision to classify the session.
Preserve the chosen end and handle the complete frames under the steward's nonblocking rule.
Change the old R2-1 test as required by the ruling.
Add a production-driver test for `Exited`, `ProcessExited`, then buffered `Done` and `RouteClosed{SessionEnded}`.
Check the pump budget, a would-block end, and the existing EOF path.

## R78-PC-2 — MEDIUM, OPEN: the grace probes a worker already known gone

Location: `crates/botster-core-host/src/run.rs:301`, `kill_payload`.
The code guards `EndPayload` with `!gone`, but the probe branch requires only `broken`.
When `worker.gone` is true and the write side failed, the branch still queues `ProbeIdentity`.
Accepted revision 3, lines 89–100, requires the known-gone branch to take priority and send no probe.
The combined tests allow the extra probe to answer `Absent`, so they do not enforce this requirement.

Required change: suppress the probe when `worker.gone` is set.
Keep the readable link and the requests in flight under the drain rule.
Assert that neither order of process exit and write failure sends `ProbeIdentity` at the grace.
Sol independently identified this mismatch in `msg_plugin-w_1791760017_a45f55`.
PC reports a fix in `dc47d2b474826e99c9c80cacf64b47a031f7e0d7`.
That reported fix is outside this reviewed head. Its completed gate and READY remain due.

## Evidence audit

The exact-head Linux gate names base `5e953398` and exits 0 after 946 seconds.
All ten steps PASS, including fuzz. Default: 1806 passed; slow: 587 passed.
Mutation total: 51; 49 caught, zero missed, zero timeout, two unviable.
All 49 raw caught identities match the 49 named test citations.
The two Linux process-state mutants are caught in Stage 2 by the real zombie test.
The two unviable mutations are `flow_spawned`'s `Instant + Duration` changed to multiplication, and `IdentityState` changed to `Default::default()`.
The gate contains no FLAKY line.
The earlier `8e2e1add` gate remains RED: three missed mutants, with fuzz not run.
The current gate names the two added tests that catch those three mutations.

Gate: `~/botster-sessions/gates/botster-core-stage1-pc-r78-stopping-79f31be3-pool-20261011-154351-14262.log`.
SHA-256: `b45f923e484960c2d2f9926ff884a3601d66897d813aad56c9c5ab26ec03e9ee`.

The reviewer independently parsed both A5-4 tables and matched all cells to the raw records.
Each table has 690 ids. Neither table has a missing, extra, or mismatched cell.
Testkit seeds are 0–31: 230 must-pass PASS records before and after; zero changes.
Real seeds are 0–7: must-pass PASS records 200→201; pending-real PASS records 6→4; seven changes.
The job script and the live body state those separate seed scopes.

One must-pass PASS-to-FAIL remains recorded: `ad_3_adopted_equals_started`, seed 2, step 16, “the relation does not hold”.
The old `84b1e9be` evidence has the same failure at seed 0, step 16, without R-78.
Both failures compare `model_rev` 1 with 2.
The R-80 follow-up assigns the missing output fence to P3 for .42 if ready.
The record preserves the failed run and makes no pending-list change.
It does not turn that failed run into a PASS.
The source-disjoint fence diagnosis and the ruling are read; final review must retain this disposition.

A5-4 evidence is in `~/botster-sessions/shared/core-stage1/evidence/pc-r78/`.
Before log SHA-256: `ede1ccfce502f24c0748f96023e07834f14999af10d01c486eaa554c603c11ac`.
After log SHA-256: `d6da136ae064cdef01d63a432751a6151d9a7de3d9854010d11e36cbc2cba3ae`.
Combined log SHA-256: `88936442574483d093451f8590412f8d6c0e36b3cf450258a989c1a73a9f9b0f`.
Old failure log SHA-256: `78052972ca7f8864eafa84cb0d4e329ec1493c088469e2a64b84b95ea9c3f34d`.
Rule-9 record SHA-256: `099070e0b46a7242d8a509f2de30e538445592a74fefce33dfa44cfd1155a1ee`.

The Linux gate runs and passes the named facade, socket-order, and zombie tests.
The nine named Mac logs show 3/3 runs for each proof group.
The Mac logs use intermediate commits `ed29e998`, `4eb326d7`, and `f97b49bd`; they are not exact-head gate records.
The later socket and facade changes replace polling loops with the existing process helpers.
The zombie source has no later change in the reviewed head.
The facade test uses a real Core and worker process, while the test thread owns and speaks the worker link.
It is a real process-exit proof. It does not prove that the production worker produces the later route frames.
The production source and driver tests must cover that message order.
The Mac clean state is owner-reported; the raw test logs do not record the clean command.

## Next review

Both source findings remain open on `79f31be3`.
PC must send a new READY with the fixed head and completed evidence.
The HIGH-tier review, final-slot checks, final gate, and Q7 remain due.
The reviewer waits for the doorbell and does not poll.
