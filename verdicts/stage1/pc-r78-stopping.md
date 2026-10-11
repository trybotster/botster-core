# PC R-78 items 3 and 4 — design review

## Round 1, 2026-10-11

Design: `~/botster-sessions/shared/core-stage1/handoffs/pc-r78-design.md`.
Design SHA-256: `bb1b8fab049aedeaf7fdcaed08fd87f04d2e5e1d7625566daa4539085af2a314`.
Reviewed base: `670e5fc799a25794910a1847fc41b3e089bf03f4`.
Plan 23cm remains governing. Scope is R-78 items 3 and 4, HIGH, PC after #283.
No source head or PR exists for this design receipt. No gate result is accepted.
Sol reads the complete design, published R-78 and R-72, and the plan's R-78 term.
Sol reads the affected engine flow, deadline, inbound, driver, process edge, and real identity implementation.
The proposed stored spawned flag distinguishes this host's children from adopted workers without changing the durable row.
The proposed identity action already exists. The driver answers it synchronously through the injected edge.
The proposed exit-queue order case preserves the pump bound: take_exits stops when the event budget is exhausted.
Sol accepts no exit drain outside that budget. Adopted-worker changes remain A21's separate scope.

### R78-SI-1 — QUESTION: identity check is not an exit-status check

R-78 item 3 requires WorkerGone if the worker has exited and WorkerUnreachable if it lives.
The proposed IdentityState check observes the pid start time, not the worker's exit status.
At process.rs:45–50, a matching start time returns Matches. The real edge calls that function directly.
The design states that an exited, unreaped worker can still return Matches.
The reaper calls child.wait(), then queues the exit. Core's pump does not control the reaper's scheduling.
A full stop_grace since link loss does not establish when the worker exits within that interval.
Thus the documented unreaped interval can include the classification instant.
The current proposal can then post WorkerUnreachable for an exited worker.
R-78's preferred-form explanation says a dead worker's exit is known by the grace point, but this check does not enforce that premise.
The lead must resolve whether this identity-only check and its documented interval satisfy the ruling.
If they do not, PC must obtain an applicable non-blocking exit check or the ruling's named existing-deadline form.
Sol does not choose a new limit, timer, OS process policy, or contract exception.

### R78-SI-2 — MEDIUM, open: engine-only scope does not preserve the input drain after a failed write

R-78 item 4 requires Core to handle every complete control message before it classifies the worker's exit.
The design retains an open link in the engine after ProcessExited, but changes no driver path.
HostDriver::service_links at driver.rs:544–548 calls flush before retry_held and read_link.
In flush, a link_send error calls close_link_because and returns.
close_link removes LinkState, including pending bytes and a held input, closes the edge, and feeds LinkClosed.
read_link then finds no link. A complete Exited frame in pending, held, or readable socket data can remain unread.
This is a source-derived failure condition. Sol runs no reproduction and claims no measured occurrence.
The engine-only change cannot close this condition. It can classify WorkerGone before it handles that frame.
Expand the design to retain and drain input after a failed outbound write, within the existing pump budget.
Define how the driver reports loss of the write side while the engine waits for the input drain.
Add a driver-level case with ProcessExited, a queued Exited frame, and an outbound write failure.
Keep the applicable complete-message and protocol-error rules. Do not add a drain outside the budget.
Clarify whether the first item-4 test queues the Exited frame or feeds it to the engine before ProcessExited.
An already-fed Exited calls begin_end_flow today. It does not demonstrate the claimed old WorkerGone result by itself.
The two-order proof must identify the edge delivery order and the action that the reversal removes.

The remaining design choices have no separate finding at this stage.
The full R-78 evidence remains due: red-on-revert proofs for both orders of items 3 and 4 and the A5-4 table without regression.
The source implementation, package review, integration source review, final gate, and body remain unreviewed.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.

VERDICT: NOT CLEAN (1 MEDIUM design finding; R78-SI-1 requires a lead decision) against base 670e5fc799a25794910a1847fc41b3e089bf03f4

## Round 2 — lead decision and revised drain design, 2026-10-11

The initial verdict `a732b58ee2b32d6a2205a8f562bb80d99cbb395d` is now pushed after the transient DNS failure.
Lead receipt `msg_plugin-w_1791751563_fad982` rejects the exited-but-unreaped interval.
The required identity check must report an exited, unreaped worker as Absent without a second reap.
Linux reads the OS process state. If an OS cannot supply the check, the ruling's named existing-deadline alternative applies.
Sol reads complete revision 2, SHA-256 `911d80047a8a6b31fa6640325d9d8ef2b138ef9f98772d7b8be02890505c54c4`.
Sol then reads the complete added write-failure section and revised note.
Its SHA-256 is `bc2bbb0d90c05d4c4d87be8e79ee8bdcf97e91f8766a7ccb3f920b005738e002`.
No source PR or executable evidence accompanies this design correction.

### R78-SI-1 — lead question resolved; implementation remains due

Revision 2 proposes a Linux process-state check and a deterministic real zombie proof on both OSes.
It relies on the current macOS refusal path instead of an explicit SZOMB guard when proc_pidinfo succeeds.
Sol reads complete B9, the retained zombie log header, raw failure, summaries, and exits.
Log SHA-256: `882bf51451f220c19a7f618e52d571e599bc37e0ba335dd89c9311cb33d98353`.
At c121ec31, the Mac test fails because the real answer is Absent and its old expected answer is Matches.
That record proves the observed refusal-path result. It does not prove behavior on a kernel that supplies zombie information.
B9's existing live_of approach retains the explicit SZOMB guard for such a kernel.
Sol reads that complete guard and its refusal path in botster-test-process/src/platform/macos.rs at the reviewed base.
The lead corrects the macOS design decision in `msg_plugin-w_1791751692_d4a13d`.
The required design retains the explicit SZOMB guard after successful proc_pidinfo, as live_of does.
The Mac real zombie proof remains the refusal-path evidence.
The lead permits the same mutation exclusion form and reason as the existing guard, with a B9 citation.
This decision resolves the question. The corrected design, exact source guard, narrow exclusion, and evidence remain due for review.
Sol accepts no broad exclusion or future source change through this decision receipt.

### R78-SI-2 — MEDIUM, still open: separate write failure from a confirmed-gone input drain

Revision 2 retains the read side after a send error and drains within the pump budget.
The first version defines only a driver write-closed flag. The engine still treats worker.link Some as send success.
Sol traces that omission through send_msg and kill_payload at the reviewed base.
A live peer with no EOF can receive a dropped Kill at StopGrace, lose its deadline, and never complete Stop.
The added section defines Input::LinkWriteFailed and worker.link_write_failed.
It makes send_msg return false and adds a live-peer/no-EOF proof. That correction addresses the live-peer omission at design level.
The note does not yet define the necessary branch priority for a worker whose death is already known.
It promises no classification at grace during a gone worker's drain, but also probe classification at grace after write failure.
It promises that a queued Done can complete during the drain, but also immediate failure of in-flight requests on write failure.
Those promises conflict when ProcessExited, write failure, delayed frames, and StopGrace occur before EOF.
A queued Exited can still be displaced by WorkerGone, and a queued Done can follow an already-failed request.
Preserve the confirmed-gone drain before classification or request failure when a read link remains.
Apply that priority when worker.gone is already true and when the identity probe establishes death.
Keep the Matches branch that ends an unreachable live worker's Stop at the existing grace.
Add the combined ProcessExited, write failure, budget-delayed Exited/Done, and grace-before-EOF case to the proof plan.

Revision 2 clarifies the first order: Exited is queued, not already fed. The already-fed form is a control case.
It adds the driver budget case, A5-4 table plan, real process proofs, and prior-art comparison.
Sol checks the cited old worker readiness function at 72b2e335. It fails the start at readiness failure and does not supply this drain design.
The claimed socket order is explicitly unverified. Its Linux and Mac real proof remains required before acceptance of that premise.
No new timer, new limit, out-of-budget drain, adopted-worker change, or implementation result is accepted.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.

VERDICT: NOT CLEAN (1 MEDIUM design finding: R78-SI-2) against base 670e5fc799a25794910a1847fc41b3e089bf03f4


## Round 3 — corrected design, 2026-10-11

Design revision 3 SHA-256: `52b9ada3f73a1fdf77c9f05b04775f2fffb4b6b5385f72149db77c6f99462885`.
Reviewed base: `670e5fc799a25794910a1847fc41b3e089bf03f4`. Plan 23cm governs.
Sol reads the complete design and verifies its hash.
Sol reads package revision-2 addition `f0f491cce365bcac825772a2de065d83493badc5` and revision-3 CLEAN `a661545657a37bc4a1f68bfab15da7c1a5c41154`.
The package CLEAN covers the same design hash and base.

R78-SI-2 closes at design level.
The ordered priority rule retains a readable link after worker.gone or an Absent/Reused probe answer confirms death.
StopGrace and LinkWriteFailed do not classify that session or fail its in-flight requests before the drain.
A new request that cannot be sent can fail immediately. A request already in flight retains its chance to receive Done.
The live Matches branch still ends Stop as WorkerUnreachable at the existing grace, without EOF.
Tests 16 and 17 cover both exit-report/write-failure orders with delayed Exited and Done and grace before EOF.
Unsent descriptors leave with HandoffLost. Sent descriptors remain owned until RouteReceived or final close.
The design requires one loss per descriptor and no retained ownership after final close.

The revised note makes the macOS SZOMB guard mandatory after successful proc_pidinfo.
The current refusal path and its real zombie proof remain separate from that guard path.
The proposed Linux Z/X check and macOS guard do not reap. The production reaper retains child.wait ownership.
No implementation or mutation exclusion is accepted by this design review.

The socket-order premise remains explicitly unverified on both OSes.
PC must establish it with the named both-OS socket proof before the dependent driver repair relies on it.
The source review must check the priority before Kill effects, classification, or request failure.
It must check complete-message handling, bounded sends, descriptor ownership, close-on-exec, EOF ownership, and all AD-6 callers.
Start retains R-72. Adopted-worker changes remain A21. Pump bounds and existing limits remain unchanged.
The exact source, both source reviews, reversal proofs, A5-4 table, both-OS evidence, and gates remain due.
Sol accepts no local source edit, reported build or test result, PR, source head, or merge readiness here.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.

VERDICT: CLEAN (0 open design findings) for design revision 3 against base 670e5fc799a25794910a1847fc41b3e089bf03f4

## Round 4 — #291 source and retained evidence, 2026-10-11

READY head: `79f31be3b3213e362539a51c35a1a1273a5aee26`.
Reviewed and gate base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Head tree: `6ff05266cf0ed9a366d5e7e21c1cb1f2d9af8f64`.
Plan 23cm governs. The earlier design CLEAN accepts no source implementation.
Sol reads all changed production code and tests, relevant callers, and retained evidence at this head.
The source spans the engine, driver, process identity, shared testkit, and real process proofs.
The production worker sends Exited before its route closes. The driver must consume that actual message order.
Sol reads complete package Round 1 at `5eb26a34f7131061db9bc81d6b88e16e4a24dcd5`, `verdicts/pc-r78-stopping.md`.
It independently reports the same two MEDIUM source findings.

### R291-SI-1 — MEDIUM: the grace probes a worker already known gone

At run.rs `kill_payload`, the EndPayload branch requires `!gone`, but the probe branch requires only `broken`.
A worker with known death and a failed write can therefore receive ProbeIdentity at the grace.
Accepted design revision 3 requires known death to take priority before any probe, classification, or request failure.
A Matches answer can then select WorkerUnreachable although the readable link still holds the confirmed-gone drain.
The World removes the process from its alive set after ProcessExited. Its automatic Absent answer hides the extra probe.
Tests 7, 16, and 17 initially check no signals, but do not assert no probe.
Required change: suppress ProbeIdentity when worker.gone is set and assert that absence in both combined input orders.
Retain the readable link and in-flight requests under the drain rule.

PC confirms that 574f85be removed the guard because no test failed under its reversal.
Sol reads the complete fix at `dc47d2b474826e99c9c80cacf64b47a031f7e0d7`, parent 79f31be3.
Its tree is `85eaac783c9407035cd11a57dc32f9337ebb2ebe`.
The fix adds `!gone` to the probe branch. The World records ProbeIdentity; all three relevant tests assert no probe.
The local reversal record shows the combined-order test failing without the guard and all 12 tests passing with it.
Log: `~/botster-sessions/shared/core-stage1/evidence/pc-r78/no-probe-reversal.log`.
SHA-256: `0b8dc7ccb27950b45aabbd235a1daf651ccd0c78b6fe3b6480a5700f6bcc41c6`.
This filtered log has no head, decoded command, clean-state header, or exit record. PC supplies its source attribution.
The complete source delta closes R291-SI-1 at source level on dc47d2b4. The 79f31be3 gate does not cover that fix.

### R291-SI-2 — MEDIUM, open: ProcessExited discards frames after Exited

At inbound.rs `on_process_exited`, retention depends on `drain_classifies`.
An Exited frame advances Stop into PostEnd or Finish, where that predicate is false.
A later ProcessExited closes the link and fails in-flight requests before the driver consumes buffered Done and RouteClosed frames.
The production worker's `report_exit` sends Exited, then `routes_session_ended` causes each route's close.
At host flows.rs `finish_close`, the missing delivered close and removed link produce SessionLost for an undelivered route.
The already-Exited test checks the selected session state. It does not check the remaining route frames or request completion.
The existing OU-7 #217 R2-1 test pins this earlier SessionLost behavior.
This is an existing behavior within R-78's required repair, not an asserted regression introduced by #291.

Sol reads the full steward follow-up at contracts `761dd5ecca5ac9a7e2ac65adf3afcea0711dbc1f`.
Its drain rule applies after every process exit, including after the session end is chosen.
Core must consume complete frames without blocking until EOF, would-block, or a read error.
A trailing partial frame is a truncated link. Core must not wait for EOF or add a timer, deadline, or CoreLimits field.
The steward commit retains an earlier separate-item owner statement.
The later lead FINAL decision `msg_plugin-w_1791760401_2488da` keeps the repair and changed R2-1 test in #291.
The package receives the same final decision as `msg_plugin-w_1791760401_5c2be8`.
These decisions withdraw the earlier R-78-F1 transfer. No separate R-78-F1 item remains.

Required change: retain the link independently of session classification and handle each complete frame within existing pump bounds.
Preserve the chosen end and in-flight completions. End the drain at the first would-block instead of awaiting peer closure.
When RouteClosed{SessionEnded} was written behind Exited, the route must close SessionEnded.
Retain a SessionLost case where no close was written. Provide the required red-on-revert evidence for the production-driver path.
The fixed source, completed evidence, and new READY remain due.

### Completed old-head gates and A5-4 evidence

79f31be3 log: `~/botster-sessions/gates/botster-core-stage1-pc-r78-stopping-79f31be3-pool-20261011-154351-14262.log`.
SHA-256: `b45f923e484960c2d2f9926ff884a3601d66897d813aad56c9c5ab26ec03e9ee`.
All ten steps PASS in 837.5 seconds. The Linux job and wrapper exit zero after 946 seconds: one queued, 945 running.
The host is structvr-MS-7C77 with four CPUs and kernel 6.8.0-142-generic; this gate is not an msa1 record.
Default: 1806 PASS, 460 skipped, 24.380 seconds. Slow: 587 PASS, 2019 skipped, 70.096 seconds.
The slow baseline has 587 PASS, 11 slow cases, and 2743 skipped, in 60.385 seconds; its interval is 83.0 seconds.
Stage 1: 51 mutants, 47 caught, two missed, two unviable, zero timeout, 97.9 seconds.
Stage 2 handles exactly those two misses and catches both in the real zombie test, in 87.5 seconds.
Sol independently matches all 49 unique raw catches to all 49 named catches and selected normal PASS results.
The gate has no FLAKY line. Fuzz reports no changed crate with a decoder harness.
This result covers 79f31be3 only. It accepts neither dc47d2b4 nor the future drain repair.

The earlier 8e2e1add gate retains eight PASS steps, mutants FAIL, and fuzz NOT RUN, with three host mutants missed.
Log SHA-256: `1b9ed08e34afa50960fc81fb897b6eb40afa8fcee18521b859e847b7f434b329`.
ff440731 adds the two tests that catch those three host mutations in the completed 79f31be3 gate.
Those caught mutations do not supply the missing source reversal records for both R-78 input orders.
PC confirms that the earlier descriptor/driver reversal was observed but not retained. Sol accepts no missing log as evidence.

Sol independently parses all 690 identifiers in each A5-4 table and matches every cell to retained raw records.
Testkit: explicit seeds 0–31, 230 must-pass PASS results before and after, zero changed cells.
Real: library default seeds 0–7, must-pass PASS 200→201, pending PASS 6→4, seven changed cells.
The combined log exactly concatenates the before and after records. Wrapper zero does not erase their raw failures.
Before raw SHA-256: `ede1ccfce502f24c0748f96023e07834f14999af10d01c486eaa554c603c11ac`.
After raw SHA-256: `d6da136ae064cdef01d63a432751a6151d9a7de3d9854010d11e36cbc2cba3ae`.
Real table SHA-256: `67e1c8aeffb9c0d76f319b9455f54c46b187f2381a457093090804938d682a7a`.
Testkit table SHA-256: `2aa5f64fdb8075e1809974d5da36701e1e91cfe9af8d12c3df6bc01962a670b8`.
Both tiers use the same .39 contract set. No pending file changes in the reviewed own diff.

The AD-3 must-pass flip remains FAIL at seed 2, step 16: model_rev 1 differs from 2.
The retained 84b1e9be record without R-78 has the same step, values, and failure at seed 0 on the earlier .36 pin.
Sol reads the full current transcript and compares the complete failure values. This is no plain-current-base reproduction claim.
The unfenced first Activity can precede application of all three output bytes before the first terminal snapshot.
Source review finds no Stop, process loss, or send-error path before the AD-3 comparison.
This is source attribution plus an earlier exact failure witness, not a recovered trace of the failed current run.
The steward's R-80 follow-up at 715639eb assigns a sentinel after step 4, replacing the step-6 Activity wait.
Owner P3 targets .42 if ready. The lead accepts that disposition and requires no pending change in #291.
Sol reads the complete updated rule-9 record, SHA-256 `099070e0b46a7242d8a509f2de30e538445592a74fefce33dfa44cfd1155a1ee`.
The AD-3 failure stays recorded as FAIL. This disposition accepts no implementation of its separate fence.

### Both-OS proof scope and remaining evidence

Named Mac logs retain three PASS results each for socket order at ed29e998, facade test 13 at 4eb326d7, and zombie at f97b49bd.
Sol reads the later socket/facade source deltas: cleanup polling changes to existing process helpers; the zombie source remains unchanged.
The named logs are intermediate-source records. They contain no clean-command header.
The later WIP logs contain aggregate results without named test records, command, head, or clean-state header.
PC confirms that no retained command/clean record supports that later claim beyond the session transcript.
Sol does not certify a clean exact-head Mac run from those WIP records.

The two completed Linux gates each run the unchanged facade test normally and again in the unmutated slow baseline.
Those four distinct normal PASS records cover byte-identical test 13 and relevant production source between 8e2e1add and 79f31be3.
They establish the repeated Linux result for that unchanged proof; no extra repetition campaign is needed for it.
The revised nonblocking rule changes test 13's premise. The new test and its recorded both-OS evidence remain due.
The facade proof uses a real process but a test-owned worker link. Production source and driver proofs must establish the later frame order.
The named socket proof, descriptor ownership, close-on-exec, and production reaper ownership retain their specific source scopes.
No default-only Mac result substitutes for the required real process proofs.

The body now correctly reports ten PASS steps on 79f31be3 and eight PASS steps before the earlier mutation failure.
At this read, GitHub names dc47d2b4 and base 5e953398; raw body SHA-256 is `12d303dd0721d6eb4a5b9cd4e0b9984d9d719cff69fe788b8c53b2072bf2cf9a`.
Its R-78-F1 transfer paragraph is stale under the lead FINAL decision. PC must correct it in the next READY.
The old-head gates, missing source reversal records, Mac provenance limits, and actual A5-4 failures must remain explicit.

VERDICT: NOT CLEAN (2 MEDIUM source findings on the READY head) at 79f31be3b3213e362539a51c35a1a1273a5aee26 against base 5e953398925351c12e463a7b0a946b79d9841edc

## Round 5 — revision-4 design delta and scoped source fix

Design delta SHA-256: `e8206746aebc91e579efbfdac247ba5979f94c7571436d43ab74dcb1de7d4352`.
Sol reads the complete delta against accepted revision 3 and the steward/lead decisions above.
Its new LinkPeerEnded action marks the peer dead; the driver consumes complete frames and closes at would-block or EOF.
That mechanism addresses the obsolete wait-for-EOF premise without adding a limit or timer.
The proposed tests change the old R2-1 setup and distinguish a written close from no written close.
The complete-message drain, trailing partial frame, and chosen session end retain their stated separate outcomes.
R291-SI-1 closes at source level on dc47d2b4 as recorded above. R291-SI-2 remains open until the fixed source and required evidence arrive.

### R291-SI-3 — MEDIUM, open design finding: a blocked held frame must not make more true

Driver rule 3 says every still-open peer_ended link counts as runnable work and makes pump.more true.
Rule 2 also preserves a held frame when the engine has no mandatory room for it.
Existing retry_held restores an unaccepted frame. Existing read_link returns while that frame remains held.
If peer_ended alone forces more true, repeated pumps cannot release that frame until a client poll frees room.
The host can spin while the drain is blocked on room. This violates the existing distinction between runnable and parked work.
Required correction: qualify the new more rule by the held frame's ability to progress.
A drain stopped by a pump budget remains runnable. A drain blocked on mandatory room waits for poll and unpark_links.
Preserve existing pump bounds, read interest, and the public wake behavior. Add no timer or new progress limit.
Sol sends this finding to PC and the package reviewer in messages 1791761035_8b895e and 1791761035_b6ec98.

The original READY head has two source findings. On the scoped dc47d2b4 fix, one source finding and this design finding remain open.
The corrected design, fixed source, retained reversal proofs, revised both-OS proof, completed new-head gate, and both reviews remain due.
Sol changes no product source and runs no build, test, gate, mutation, reversal, measurement, diagnostic, or executable base-check job.
Never poll. Wait for the doorbell.

VERDICT: NOT CLEAN (2 MEDIUM: R291-SI-2 source and R291-SI-3 design) at dc47d2b474826e99c9c80cacf64b47a031f7e0d7 against base 5e953398925351c12e463a7b0a946b79d9841edc
