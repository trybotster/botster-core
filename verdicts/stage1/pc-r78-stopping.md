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

## Round 6 — corrected revision-4 design delta

Corrected delta SHA-256: `cf3914bc4e7f8fae32619b72ddcb3ae941b6dfe6fb871bda3ab3e39f2a2438ef`.
PC receipt: `msg_plugin-w_1791761355_a28dce`. Sol reads the complete corrected delta and verifies its hash.
Sol reads the full corrected package design section at `bbf3ad90dfe221dd5ca391f7d264912284a3662c`.
Its design CLEAN covers the same hash and preserves the source/evidence conditions.
Rule 3 withdraws the earlier unconditional open-peer_ended rule.
LinkPeerEnded also marks drain_due for a drain announced after the pump's last link read.
read_link clears drain_due when it starts. Only an unblocked drain_due link adds runnable work.
A budget-limited read retains the existing more_input result. A held frame without mandatory room waits for poll and unpark_links.
The new full-queue driver proof must check more=false while blocked, then wake and ordered delivery after the poll frees room.
This correction addresses R291-SI-3 at design level without adding a limit or timer.
The source review must verify the exact placement of drain_due updates and held-frame acceptance against the imported ST driver change.
Sol accepts no ST implementation, source repair, or execution through this design correction.
R291-SI-2, new-head evidence, both source reviews, and final acceptance remain due.
Sol runs no execution job. Never poll.

VERDICT: CLEAN (0 open design findings) for corrected revision-4 delta cf3914bc against base 5e953398925351c12e463a7b0a946b79d9841edc; #291 source remains NOT CLEAN

## Round 7 — equivalent design without drain_due

Current design delta SHA-256: `bb550e4191009a3e1aa094e6b5e7f22f5adc1b9fdd5f09b3d9eebb2e811f8d4f`.
This hash supersedes cf3914bc. Sol reads the complete current delta and verifies its hash.
Sol reads the complete driver flag-removal delta at `1ff76822fe8d239d9b8cffb63fa096001e59f9b7`.
Sol reads the full retry_held/read_link paths and the relevant action, draining, and pump conditions at local 2bc8b247.
These scoped reads establish design equivalence. They do not supply a full source review of that unready head.

The simpler rule requires peer_ended and held.is_none(). It excludes the room-blocked held frame that caused R291-SI-3.
It also covers the required first read after LinkPeerEnded follows the pump's last link service.
After a read, an ended link closes at EOF, would-block, a read error, or bad decoding; otherwise it holds a frame or stops at a bound.
Both budget exits set more_input. The held-frame path preserves existing backpressure and poll/unpark behavior.
Thus an open ended link without a held frame already denotes runnable drain work; drain_due adds no observable scheduling result.
The full-queue, budget-delay, and settle-time first-read proof requirements remain unchanged.
The source review must check these properties on the complete repaired head and its imported driver changes.

Sol reads complete matching package design section at `93eade7e762d949a9ceba6d0de78b22ad536ba4c`.
It covers the same hash and preserves full source, reversal, both-OS, and gate conditions.
R291-SI-3 remains closed at design level. R291-SI-2 and revised source/evidence acceptance remain due.
Sol accepts no new execution result or source head in this design round. Sol runs no execution job. Never poll.

VERDICT: CLEAN (0 open design findings) for revision-4 delta bb550e41 against base 5e953398925351c12e463a7b0a946b79d9841edc; #291 source remains NOT CLEAN

## Round 8 — repaired source and current evidence inspection

READY head: `8ac717c7d94560bf1f82a5975bb45162ba0b88da`.
Reviewed source base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Head tree: `07a89c69acd600e65d27c7b536d30908831f816c`.
Gate comparison reference: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
Sol verifies that the comparison reference is not an ancestor of this head.
It supplies first-READY evidence, not final-slot acceptance against current v1.
Plan 23cm governs. The accepted revision-4 design hash remains bb550e41 above.

Sol reads the complete repaired production delta and all changed tests after 79f31be3.
Sol also reads the complete affected input, action, read, held-frame, pump, close, and request-completion paths.
The earlier review covers the unchanged original production paths and their worker callers.
Sol independently checks the f6025aed import: all imported non-own blobs match that base.
All non-overlapping own blobs match 2bc8b247 at merge 2eee84ba.
The overlapping real.rs merge preserves every own added and deleted line.
The only subsequent production change restores the first pump `more` expression and adds its explanatory comment.
The own diff changes no pending list or contract pin.

### Original source findings close

R291-SI-1 remains closed. The probe and EndPayload paths both require a worker that is not gone.
The World records identity probes. The combined-order and known-gone tests assert that no probe occurs.

R291-SI-2 closes in source. `on_process_exited` retains an open link for Idle and Stop, including a chosen end.
It emits LinkPeerEnded instead of closing the link or failing in-flight requests.
An Absent or Reused stop-probe answer uses the same drain action while a link remains open.
The driver consumes complete frames within its existing budget and preserves an unaccepted held frame.
The driver closes an ended link at EOF, a read error, or the first would-block.
It treats a trailing partial frame as a truncated link. It adds no timer or limit.
The close classifies WorkerGone only when no end is chosen. In-flight requests retain their chance to receive Done.
The updated OU-7 cases distinguish a written RouteClosed behind Exited from no written close.
The first case produces SessionEnded once. The second case produces SessionLost once.
The production-driver case checks Done, RouteClosed, the chosen exit code, and host closure without peer EOF.
The combined-order case queues all frames before exit and delays delivery across the grace with the pump budget.
It checks both exit-report/write-failure orders without early request failure or classification.

R291-SI-3 remains closed. `draining()` requires `peer_ended && held.is_none()`.
A held frame without mandatory room leaves `more=false` until poll frees room and restores the link.
A budget-limited drain preserves `more_input`. The bound test checks eventual closure over successive pumps.
The settle-time case checks that an exit taken after the last link read leaves runnable work.
The first `more` expression matches plain base. Only the post-settle expression adds `draining()`.
There are eleven driver-drain tests. The updated body corrects its earlier count of twelve.
Sol establishes no additional source defect on this head.
The future ST driver import still requires composition review at the final slot.

Package message `msg_plugin-w_1791763913_35c121` confirms closure of both original source findings on 8ac717c7.
The new formal same-head package verdict remains due. Sol does not infer it from that message.

### Exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-pc-r78-stopping-8ac717c7-pool-20261011-165822-79584.log`.
SHA-256: `0bfc1d41038bd978bd0a91bf4547acb5b371bf63cf1886f9f0d9e8ac11723e2f`.
All ten steps PASS in 284.9 seconds on msa1 with four CPUs.
The job and wrapper exit zero after 468 seconds: 173 queued and 295 running.
Default: 1822 PASS, 460 skipped, 13.035 seconds. Slow: 600 PASS, 2025 skipped, 43.186 seconds.
The unmutated slow baseline has 600 PASS, ten slow cases, and 2757 skipped, in 42.275 seconds.
Stage 1 has 62 mutants: 58 caught, two missed, two unviable, zero timeout, in 73.2 seconds.
Stage 2 handles exactly those two first-stage misses and catches both in 39.8 seconds.
Sol independently matches all sixty unique raw catches to all sixty named catches and their selected normal PASS results.
No FLAKY line occurs. Fuzz reports no changed decoder harness.
The current Linux facade test passes in both the normal slow run and unmutated baseline.

The prior 2eee84ba gate remains RED: eight PASS steps, mutants FAIL, fuzz NOT RUN.
Its SHA-256 is `d2248a5a28a85cd6a25e2a6ea97e0c90c03ed9c640adbc8b85169ebfeb2ebd04`.
Stage 1 has 58 caught, four missed, two unviable, and zero timeout.
Stage 2 catches two and misses the two first-pump `||` to `&&` mutations.
8ac717c7 removes the redundant first-pump drain term instead of repeating that same head's gate.
The retained current post-settle operator reversal fails four tests; the restored host tree passes 320 tests.
That filtered log has SHA-256 `ff4a800f793a09b2cee0203c6a73e80b0c146937a673548f6068b7112a6438b6`.
Its clean sequence relies on PC's explicit same-worktree statement. It does not supply a full decoded command.

### Reversal and Mac cache records

The old-head driver reversal has SHA-256 `1fb0d0de515a18a6e87214a10b9c3390dd34780890e7793a09007c5a842b17dd`.
Five of eight new tests fail against 79f31be3 production code, including the written RouteClosed case.
PC records a clean immediately before that detached worktree's run, with no other target writer.
The filtered record retains no complete command or all panic messages. Sol preserves those limits.

PC receipt `msg_plugin-w_1791763865_4af065` establishes a B13 gap in the older 1ff76822 reversals and 2eee84ba Mac runs.
Those records reused targets/pc after another worktree's build without a clean.
Their valid source labels and observed outcomes remain historical, unvalidated evidence.
Lead receipt `msg_plugin-w_1791764208_f9121a` requires replacement reversals and Mac test 13 with a recorded clean.
PC supplies replacements in `msg_plugin-w_1791764392_6e1179`.
Sol reads every replacement record in full and verifies each hash.

All replacement paths are in `~/botster-sessions/shared/core-stage1/evidence/pc-r78/`:

- `rev4-reversals-clean-8ac717c7.log`: `ddfa762de9915a973aa0f54add562925390277cce1dcdf41ad08aac0ef2ef18b`.
- `prebuild-clean-8ac717c7.log`: `4b2b6a02ff2be0453bd95319c5807105db3cb1fa97c4bc4fd136b37744cab0e2`.
- `t13-mac-clean-8ac717c7-run1.log`: `20b97989952ae726977c6113731c7441c692eb155b10f1fd879e7a919ed75acb`.
- `t13-mac-clean-8ac717c7-run2.log`: `fb892f05b1150587cf3a3caea0a420b3e4ab8394696a127001531108dbcfdbe3`.
- `t13-mac-clean-8ac717c7-run3.log`: `d1a3b44923fa02524efc148a73fa8d2d218bd1805f3b408e629f62bb1712e661`.

The first header records exact head, clean worktree, no other target process, and cargo clean removing 10.0 GiB.
It records that targets/pc is absent after the clean.
PC states that only the R-78 worktree builds there through the final facade run.
The five reversals fail eight, one, one, ten, and one tests respectively. The restored tree passes 320 of 320.
The prebuild record names all three candidate binaries and their manifest hashes.
Each facade header names 8ac717c7, a clean worktree, the same clean sequence, and that candidate build.
The facade test passes three of three runs, in 0.395, 0.323, and 0.366 seconds.
These filtered records do not contain complete decoded commands or reversal patches.
Sol accepts their named outcome and cache scope from the retained headers and PC's explicit statement.
The replacement records close the recorded B13 gap. They do not validate the superseded runs retroactively.
The PR body snapshot below predates these replacements; the final body must cite the accepted replacement records.

### Current A5-4 comparison

The lead requires a new comparison with plain f6025aed before and 8ac717c7 after.
Both tiers use the same four-job script and table version 2 on msa1 Linux 6.12.111+deb13-amd64.
Testkit uses explicit seeds 0–31. Real uses default seeds 0–7.
Sol independently parses every raw result and every table cell, without running PC's scripts.
Each table contains exactly 690 unique identifiers. Every count, status, and failure detail matches the raw records.
There are no missing cells, extra cells, false PASS cells, or mismatches.

- Before raw SHA-256: `1d1f752e7f95ad2583fd21f42629bf665bea532937057a7f535e3dc440b6cbbc`.
- After raw SHA-256: `f3426eb9e0baf6a10e732e9cb871f4c0915a5481b8b980e8881a3114cbc17391`.
- Real table SHA-256: `b7504020fa7d79fca6d9995e256407a2939ef1aea62b53c91f15e8db4f320719`.
- Testkit table SHA-256: `2aa5f64fdb8075e1809974d5da36701e1e91cfe9af8d12c3df6bc01962a670b8`.

Testkit has 230 must-pass PASS results before and after, with zero changed cells.
Real has 205 must-pass PASS results before and 203 after; pending-real PASS results change from ten to nine.
Real has six changed cells. The after run has two raw must-pass failures despite its job and wrapper exit zero.
The filtered command pipelines mask those failure exits; wrapper zero is not a PASS verdict.

The six real changes are:

- dp_5_bytes_and_text_reach_the_pty_exactly: pending PASS becomes pending FAIL at seed 0, step 18, with empty PTY segments.
- dp_5_invalid_utf8_text_is_refused_unsupported_not_route_closing: pending FAIL changes from seed 0 to seed 4, both at step 13.
- ev_2_drop_oldest_with_marker: pending FAIL changes from step 67 to step 66 at seed 0, with the same one-event lag.
- in_9_modes_read_at_write_start: must-pass PASS becomes INCONCLUSIVE at seed 4, step 8, limit step_timeout.
- st_6_capture_is_frozen_at_model_rev: must-pass PASS becomes FAIL at seed 2, step 17, expected ne[1,1].
- tm_6_begin_and_attach_signal_wake: pending FAIL changes from seed 1, step 21, to seed 0, step 21, with one unexpected event.

The four pending changes retain their existing pending status. This PR changes no list.

### ST-6 attribution and R291-SI-4 — MEDIUM, open: IN9 must-pass flip lacks accepted attribution

Lead receipt `msg_plugin-w_1791764208_f9121a` attributes ST-6 to v1 through #288's accepted controlled reproduction.
Sol supplies the required source-disjoint assessment: ST-6 reaches its failing step without Stop, exit, send failure, or link loss.
The added R-78 stop and exit branches therefore do not act in that transcript prefix.
The new link flags start false. Without a worker end or send failure, they remain false and their new guards do not change delivery.
The first pump `more` expression matches base; the new post-settle drain term remains false on that live link.
The own diff changes no worker production code, PTY output control, model feed, or ReadScreen implementation.
The Linux zombie check still returns the same identity for a live worker.
Both ST-6 output steps use await_quiet instead of an output fence before their model reads.
These source facts support the same missing-fence mechanism accepted for #288.
The accepted plain-f6025aed diagnostic reproduction has seed 0, step 17, and the same ne[1,1] failure.
Its log hash is `a21e40b2b2db7710acd8cc0eb3572a726cd27f3720debcdde6db0f49e7428c09`.
Sol's acceptance commit is `5fa42e4aa2dbf186fdf7ad9a7c4308af4ca2210f`.
This assessment does not claim a controlled seed-2 run, an original-gate trace, or an uninstrumented-v1 reproduction.
RC owns the future .42 fence import. This READY still uses .39.

IN9 remains unattributed under the same lead receipt. Source-disjoint reasoning alone cannot close this flip.
Its transcript blocks the PTY, begins focused input, enables focus reporting, releases the PTY, then awaits written completion at step 8.
It has no Stop, exit, send failure, or link loss before the timeout.
Sol reads the complete current IN9 record; it supplies no earlier plain-v1 failure witness.
PC owns the diagnosis. The lead requires a plain-v1 failure log or an accepted 23bl(A) controlled reproduction.
Until that evidence exists, the lead treats this as a #291 flip and a merge blocker.
Do not remove the identifier, change its status, or use a passing normal gate to erase the A5-4 failure.

### Body and remaining acceptance

Sol reads the complete updated body at exact head 8ac717c7 and GitHub base f6025aed.
Raw body SHA-256: `39ca6b22325c3d73718ac453542db40d2f753682bfbc1f759957174a7395fbe0`.
It corrects the driver test count, preserves the withdrawn transfer, names the new A5-4 failures, and marks the old B13 records unvalidated.
It must incorporate the replacement records and final lead disposition in the next acceptance body.
The final slot still requires current-v1 and ST imports, applicable base checks and delta reviews, and one exact-final-head gate.
The complete new package verdict and final acceptance remain due.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check. Never poll.

VERDICT: NOT CLEAN (1 MEDIUM evidence finding: R291-SI-4; 0 open source findings) at 8ac717c7d94560bf1f82a5975bb45162ba0b88da against source base f6025aed4cdf7da836e835e0040fc128b7e8f32f

### Round 8 supplement — IN9 controlled attribution accepted by Sol and the lead

PC supplies the proof in `msg_plugin-w_1791764613_d099fd`.
Sol reads the complete log, scratch diff, pinned transcript, runner, probe gate, output acknowledgement, worker loop, and focus encoding.
The proof starts from plain v1 `f6025aed4cdf7da836e835e0040fc128b7e8f32f` without #291.
The only scratch path is `crates/botster-core-testkit/src/real/program.rs`.
The scratch file is `~/botster-sessions/gates/pc-in9-repro-scratch-f6025aed.diff`.
Its SHA-256 is `0151f1ecce86b2fb27ea9fb2c7456f143da417a2473beba29fc0941c593aed01`.
It delays only CSI ?1004h delivery by 300 milliseconds in a thread. The thread later calls the unchanged output request.
The diagnostic is never committed and changes no result predicate, Core code, or input encoding.

Log: `~/botster-sessions/shared/core-stage1/evidence/pc-r78/in9-repro-f6025aed.log`.
SHA-256: `6d52750fe109e57c4e7d13d75f237875eda1e274dfc5b6c63984dbf4b003bad3`.
Sol decodes both embedded patches without executing either. Both match the complete supplied diff byte for byte.
The log records exact checkout f6025aed, contracts ddce507e, patch application, and that sole modified path.
The pool job runs on msa1 with four CPUs and writes the candidate manifest before the real-tier test.
The test has one failure and 689 filtered identifiers, in 10.37 seconds.
It records INCONCLUSIVE for in_9_modes_read_at_write_start at seed 0, step 8, limit step_timeout.
That is the A5-4 identifier, step, and symptom; the A5-4 failure uses seed 4.
The job and wrapper exit zero after 29 and 30 seconds because the command filters output and finishes with an echo.
Those exits do not change the raw failure to PASS. The log supplies no numeric underlying test exit.

The plain host `begin` queues the operation and synchronizes the wake; only pump makes progress.
The output control waits for the probe acknowledgement. That acknowledgement does not fence the worker's PTY read.
The worker serves its control link and settles its requests before it reads a PTY chunk in the same turn.
Thus plain code can process the focus request before the mode bytes, even when the probe acknowledged those bytes first.
The delayed diagnostic makes that existing order easier to reach during step 6's pump.
With focus reporting off, encode_focus returns None, and model::encode returns NotWrittenReason::NotReported.
The worker can therefore emit Completed with NotWritten(NotReported) before ModesChanged.
The runner's pump_until discards events that do not match. Step 6 can discard that Completed while it awaits ModesChanged.
Step 8 then waits for the already-consumed completion and reaches its deadline.
The probe's blocked flag stops stdin reads. It does not itself make this real-tier focus request WouldBlock.
PC corrects the earlier inconsistent diagnosis in `msg_plugin-w_1791764787_827f62`; Sol reads the complete corrected record.

Sol confirms source non-interference for the same transcript prefix as Round 8.
No Stop, exit, failed send, or lost link occurs before the failure. The new R-78 flags and branches remain inactive.
The worker loop, focus encoder, runner, and output-control implementation remain unchanged by #291.
The diagnostic supplies no missing behavior. It widens an existing timing window.
The retained log has no event-order trace. The mechanism is inferred from source and the retained result.
Sol claims no controlled seed-4 run, original A5-4 trace, or uninstrumented-v1 failure witness.

Lead receipt `msg_plugin-w_1791764762_92795e` explicitly accepts this case under 23bl(A), including those limits.
The lead assigns the removing contracts PR to PC: the R-88 held-write PR under the steward's R-74 form on both tiers.
That PR also covers tm_6_begin steps 21/22 and in_6_refused_full step 9. It can enter .42 if final in time.
Sol accepts this controlled attribution. R291-SI-4 now awaits only the required independent package acceptance.
The complete repaired-head package verdict, final body, and final-slot acceptance remain due.
This supplement accepts no final merge or new execution. Sol runs no execution job. Never poll.

VERDICT: CLEAN for Sol's IN9 controlled attribution; overall #291 acceptance remains NOT CLEAN pending package acceptance and final-slot evidence

### Round 8 supplement — independent IN9 acceptance

Package receipt `msg_plugin-w_1791765275_d061a2` accepts the same controlled IN9 attribution.
The package reviewer reads the corrected record, complete raw log, scratch diff, and unchanged source paths.
Both embedded patches match 0151f1ec. The package reviewer preserves the exact step-8 step_timeout result and the missing event-order trace.
The reviewer also preserves the absence of an uninstrumented-v1 failure and the NotWritten(NotReported) source result.
The package reviewer reads Sol's e72d7d01 supplement and verifies publication.
The package reviewer closes R78-PC-3 and R78-PC-4.

R291-SI-4 closes after both reviewers accept the attribution and the lead accepts this case under 23bl(A).
There are zero open integration source or evidence findings on repaired head 8ac717c7 against source base f6025aed.
This closes the attribution finding; it does not change the recorded A5-4 failure into a PASS.
The current body and complete formal package verdict remain due. The package reviewer will publish after PC completes the body.
The final slot still requires the ST-first import, applicable checks and delta reviews, and the exact-final-head gate.
Sol accepts no later source head or final merge. Sol runs no execution job. Never poll.

VERDICT: CLEAN (0 open integration source or evidence findings; body, formal package verdict, and final-slot acceptance remain due) at 8ac717c7d94560bf1f82a5975bb45162ba0b88da against source base f6025aed4cdf7da836e835e0040fc128b7e8f32f

## Round 9 — final-slot #295 import and ordering resolution

Exact head: `4ccd6a7da142331c2d35fa25f5f12914cdf2d22b`.
Imported base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Tree: `ed7ce2ee718c20284fbb1543f6567ce5e6e9c7be`.
Delta request: `msg_plugin-w_1791772688_eb1ff6`.
Merge `b8fee77332770736f43886b35c387eef3fc996c4` has parents reviewed head
`8ac717c7d94560bf1f82a5975bb45162ba0b88da` and the imported base above.
Its tree is `24a5e7ca76413872eec0dc25867261b237884bf2`.
Resolution 4ccd6a7d is one commit on that merge, with ten additions and seven deletions in driver.rs and driver/drain.rs.
The actual new merge base equals bfef8865. The old common base remains f6025aed.

Sol applies the existing orchestrate-delivery lifecycle review to the change in every exit's action ordering.
The acceptance scope is R-78 item 3 classification and item 4's nonblocking drain with ST's #295 budget rules.
HostDriver owns exit delivery, action execution, stream reads, and retained budget work.
HostEngine owns the session end and completion classification. PC owns this repair and its final gate.
Complete frames must reach the engine before the drain's close chooses a loss;
an already chosen end stays chosen, held frames wait for mandatory room, and bounded work retains its wake.
No new timer, limit, production test branch, or pending removal is in scope.

### Independent import check and complete source delta

Sol independently checks every non-own final blob against the imported base; all match.
The own diff retains the same 27 paths and adds no own path or list change.
The five shared paths are driver.rs, engine.rs, run.rs, driver/deadlines.rs, and testkit real.rs.
Every old own changed-line sequence is preserved in the plain merge.
After removing index and hunk offsets, all old own patches match except driver.rs.
Its remaining differences are surrounding #295 context: link_ends removal, the kept-exit documentation,
and the read-error call to end_link. Sol reads those complete shared paths and the full resolution.
The resolution intentionally changes the ended-peer WouldBlock call, initial action pass, and empty-drain test.
All nine conformance blobs exactly equal bfef8865, including both pending files, held, ledger, proofs, and minimum.
Thus no v1 identifier is re-added, no current status is discarded, and #291 changes no pin.
The imported real.rs record/control and zombie changes compose without an additional source delta to their accepted behavior.

Sol reads take_exits, perform/perform_counted, every action arm, service_links, retry_held,
the complete decoder/read/end_link path, the whole pump loop, settle, and budget accounting.
Sol also reads on_process_exited/on_link_closed, chosen-end guards, and the identity-probe drain path.
The new initial perform_counted runs the whole queued action list before the first link read.
It is not an exit-only pass. Sol reads cancel, attach, flush_handoffs, and their driver wrappers.
Queued Cancel and HandoffRoute actions remain FIFO and execute in their next pump, with the existing failure handling.
The existing counted pass accounts for posted events and budgets failed-handoff inputs.

For Idle/Stop, the early action installs peer_ended before service_links.
The read handles every complete frame before closing at EOF, a read error, or the first WouldBlock.
The normal work loop can then classify an empty drained session in the exit's pump.
Start and Adopt retain their existing failed-start/adoption policies; their queued close executes before the read.
Remove AwaitTeardown retains its result/EOF path; other Remove phases retain the known-gone rule.
The change adds no reap or descriptor ownership mechanism.

An ended-peer WouldBlock now calls the existing end_link with this pump's budget.
With no event allowance it retains the reason in link_ends and sets more_input.
The next service handles a held frame first, then the retained end; final close removes the retained reason.
The first more expression is `runnable || more_input`.
After settle it is `runnable || more_input || draining()`.
A settle-time exit still gets its action after that read and leaves an unperformed drain with the wake set.
A frame blocked on mandatory room remains parked, because draining requires no held frame.
A budget-held frame remains runnable through more_input; its poll and wake rules remain intact.

Sol reads all eleven drain tests, both branches of the exit/send-failure order test,
all named handoff and deadline exit sites, and the queued cancel/attach tests.
The changed empty-drain test preserves the WorkerGone invariant and now requires it in the exit's pump.
The other tests retain Done/Exited/RouteClosed order, chosen-end delivery, grace priority,
partial-frame rejection, mandatory backpressure, bounded drains, settle-time wakes,
and one descriptor/route close after the session end.
No integration source finding is open.

### Canonical check and retained evidence

Completed records arrive in `msg_plugin-w_1791773040_8ebd48`.
Canonical log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-8ac717c7-4ccd6a7d-20261012.log`.
SHA-256: `02c50f1f1c26a5d10d14dca9da93f866a87bc7bc331d77147b8119dcb9db2ab4`.
Sol reads it in full. Ancestry, condition 1, and both pending-file condition-4 checks PASS.
Conditions 2 and 3 FAIL for the shared paths and changed own diff.
The retained result stays FAIL and requires both delta rounds; Sol does not execute the checker.

Sol reads the retained interdiff and independently matches these complete artifacts to their named Git source pairs:
old own diff `0d20434952183cd6dba4f3b8ff37ffdf5b8501b9d53ef6d9c63420e4ab0823b2`;
new own diff `75c18d7683e555cf26548c4b0989d891a3888b8ee3c3da0d80bd48121bc0a2be`;
resolution diff `f9dc7ddf26a2ac92d25e3cfaf35da1869ec09eb6166b4aff0cb8d1070c3dd1c3`.
The interdiff hash is `28ecf75c2d0b19cf28ab6c61eb221346792f44eff84d39aafe5be39aac386c4d`.

Static log: evidence/pc-r78/xtask-static-4ccd6a7d.log in the shared directory.
SHA-256: `166df8757594e12d7b6ab5298f90de4e7783894e37fbb6120f4d8861ff4e5953`.
Sol reads the full log. Fmt and clippy with -D warnings exit zero.
Ten distinct xtask checks pass: nine on the first run;
public-api first refuses the absent cap with exit one, then its capped rerun exits zero.
These are eleven attempts, not eleven exit-zero checks. Both records remain distinct.
Lists retain 691 ledger identifiers, 410 testkit pending, and 44 real pending at this base.
The local slow-coverage check examines 1070 slow-only tests; it is not a slow-tier execution claim.

Strict log: evidence/pc-r78/local-check-4ccd6a7d.log.
SHA-256: `d39235a05ed9bc9ed4e46f1145c975a17cb02b2a15b6264f87559d8c087c55b3`.
The header names the exact head, BOTSTER_PENDING_STRICT=1, and seeds 0–31.
Sol independently totals all completed summaries: 1221 PASS, zero failures, and a final numeric exit zero.
Host has 331/331 PASS. All eleven drain, nineteen handoff, and twenty-one deadline tests have normal ok records.
The queued cancel and attach tests also PASS. Conformance has 258 PASS and 433 ignored in this local record.
These supporting results do not replace the exact-head final gate or its mutation evidence.

The earlier clean record is `~/botster-sessions/shared/core-stage1/evidence/pc-r78-merge-bfef8865.log`.
SHA-256: `91c6f6238bc60d0f0e355a089651f2e835bceb07337ecccc9648d1f92ff5ce58`.
Its header is b8fee773 plus the then uncommitted drain-end fix, not an untouched merge.
It records clean removal of 15520 files and 4.1 GiB at 02:30:45 UTC.
It has 330 PASS and one kept-exit assertion FAIL: Running instead of Lost(WorkerGone).
It supplies no numeric underlying test exit. An unretained untouched-merge failure remains only PC's report.
Static source supports the ordering diagnosis: the drain-end fix cannot act before peer_ended is installed.
PC confirms that the same worktree alone built targets/pc from that recorded clean through the new checks.
This owner statement supplies the cache condition; Sol performs no independent build.

Historical r78-final-local.sh is retained as run, hash `3a091797a46287fc9ccd2fedc398946cf0907246307b5048c3976c36db1dac51`.
It does not set both required cap variables. The public-api refusal confirms that launch defect.
The future copy r78-final-local-capped.sh exports both variables at four;
Sol reads it, hash `494039e7f285d17848a42e27b647db33cb7078273d5f950827b91578fff568cf`.
No new execution is inferred from the corrected script, and no repeat run is requested.

### LOW body corrections and matching complete package verdict

Sol reads the whole initial body, hash `0123446019f39ad65b480c3b272bcad1240c29cc681fa2d3764544f49c92661a`.
It lacks the current import/resolution, says the empty drain ends in a later pump, and leaves IN9 acceptance pending.
PC adds the current slot section, action ordering, two more expressions, and accepted attribution.
Sol reads the full delta, hash `ecf28a3eeb2a210cf2b083ffb7cd1f3b211e4170e789ce5a56e5ff8d620c2a93`.

Sol identifies the eleven-check overstatement in `msg_plugin-w_1791773113_cc930e` and `_c87772`.
Package receipt `msg_plugin-w_1791773150_c73ade` also requires withdrawal of the old held-PTY IN9 diagnosis
and a capped future script. Sol agrees with those conditions.
Sol then identifies the scratch-source scope of the failed merge run in `msg_plugin-w_1791773267_cabf46` and `_4942d4`.
These are LOW precision conditions; none requires a product change or new execution.

PC supplies all corrections in `msg_plugin-w_1791773308_972f34`.
Sol reads each complete body delta and the updated evidence manifest.
Final live raw body SHA-256: `da2fa54e4a1df2f2ef7c68f473fa41460c25579500b19939390f4cdf91ce8c32`.
Retained body snapshot SHA-256: `3aaf938eadf9c3423462a394d5884a28f5be9d52f6442fb21453e0906d002638`.
They match except for the retained file's extra final newline.
The body withdraws the held-write diagnosis and states NotWritten(NotReported) before ModesChanged,
the already-consumed completion, accepted controlled-v1 attribution, and the absent event-order trace.
It names ten distinct checks and both public-api outcomes, both script scopes,
and the failed run's actual merge-plus-fix source. All LOW conditions close.
Old gates, A5-4 tables, replacement reversals, Mac proof, and unvalidated historical records retain their exact source scopes.
The two pinned failing prefixes have no cancel or attach before their named failures;
the additional pre-read action pass supplies no new action on those prefixes.
Their accepted attribution remains source reasoning and the earlier controlled proof, not a new-head A5-4 run.

Complete evidence manifest SHA-256: `e9a8f860cd021c9ef8e44e28cd79d7c8d5fef4fd47b7b7f8f4b9c56360da927e`.
Sol independently matches every listed artifact hash to its retained file.
This manifest is PC's evidence package, not the reviewer's verdict.
Sol reads complete package Round 2, supplements, and Round 3 at
`6c4d51321b7e352a6c0f377d996dc7bd9cd84729`, verdicts/pc-r78-stopping.md.
It closes the earlier source/evidence findings, independently accepts IN9 with its limits,
and supplies matching delta CLEAN at this exact head and base, including all LOW closures.
Package receipt `msg_plugin-w_1791773510_e52955` confirms this is pre-gate delta approval.

Both delta reviews now have zero open source or body findings. The canonical FAIL requires these reviews and stays FAIL.
ONE completed exact-head final gate, its mutation audit, final body/evidence reviews, and lead Q7 remain due before merge.
This verdict accepts no final gate result, later import, different tree, new pin, or status removal.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job. Never poll.

VERDICT: CLEAN (0 open integration delta findings; exact-head final gate remains due) at 4ccd6a7da142331c2d35fa25f5f12914cdf2d22b against base bfef88653bb59ad8d3cd38da1e206f7aba64ce92

## Round 10 — completed final gate and body

Head: `4ccd6a7da142331c2d35fa25f5f12914cdf2d22b`.
Base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Tree: `ed7ce2ee718c20284fbb1543f6567ce5e6e9c7be`.
Completion receipts: `msg_plugin-w_1791774951_0642b9` and `msg_plugin-w_1791775019_52ed15`.
The complete source review in Round 9 covers this unchanged head and its ordering resolution.
Sol reads completed gate headers, all mutation outcomes and citations, relevant normal tests,
baseline records, conformance results, step summary, and final exits.

Gate: `gates/botster-core-stage1-pc-r78-stopping-4ccd6a7d-pool-20261011-195723-18711.log` under `~/botster-sessions`.
SHA-256: `df19bcedaa620616b8e508ce399a9b60d4e0704c91443914b1bc3c2b88792171`.
Its retained client output has identical bytes. The header names this exact head and base.
It runs on structvr-MS-7C77, kernel 6.8.0-142-generic, with four CPUs. This is not an msa1 record.
All ten steps PASS in 740.8 seconds. Job and wrapper exit zero after 854 seconds, with one second queued.
Default: 1887 passed, 433 skipped, 25.174 seconds. Slow: 613 passed, 2042 skipped, 63.110 seconds.
The unmutated slow baseline has 613 passed, eleven slow cases, and 2786 skipped, in 63.313 seconds.
Both ordinary conformance summaries have 258 passed and zero failures.
Held-full runs two budget identifiers at 32 seeds; all 64 expected ignored results have zero failures in 19.38 seconds.

Sol independently parses 62 unique first outcomes: 58 caught, two missed, and two unviable, in 122.2 seconds.
Stage 2 tests exactly those two missed identities and catches both in 79.7 seconds.
All 60 unique raw catches equal all 60 named citations. Each catcher has a normal PASS record.
Both slow catchers have baseline PASS and name the real unreaped-process test.
There is no unmatched identity, remaining miss, timeout, or FLAKY token.
The two unviable outcomes remain unviable; no executed catch is claimed for them.
Fuzz PASS reports that no decoder harness changed.

All eleven drain, nineteen handoff, and twenty-one deadline tests have normal PASS records.
These cover empty drain, settle-time drain, both exit/send-error orders, held-frame backpressure,
queued handoffs, and the exit/action/read ordering reviewed in Round 9.
The real facade, socket-order, and unreaped-process evidence retains its earlier source scope.
The facade uses its test-owned link; it does not alone prove production-worker route ordering.
IN9 and ST6 normal results do not erase earlier A5-4 failures or replace their accepted controlled attribution.
Sol independently verifies all 34 reported real-pending passes remain in the unchanged 44-entry list.
This PR accepts no pending removal, new fence, or real-tier promotion.
This gate supplies Linux evidence. Earlier Mac records and invalidated shared-target records keep their recorded limits.

Sol reads the complete final body delta against the already reviewed body.
GitHub reports this exact head and base. Live body SHA-256:
`8e3ca6976c3a3c3af005d93bd1d09477f9536f6c682137b2ef22ea57ad817252`.
Retained snapshot SHA-256: `6b490871e38bf73f49ee70848fd0d063e3cd64eae9f203aa1c3cbdcf943d45e8`.
The snapshot equals the live body plus one final newline.
The body replaces the pending gate with the completed result and preserves all older source and attribution limits.
Sol reads the complete updated owner package, SHA-256
`e2a9bcba7261aacf3d039bcf0f4c2d4ad78da7fd268c5309b7e0f995c6b4b46f`.
Sol reads complete package Round 4 at `b29b6a853340d9f8dce05ceec3cfb1b6c4b5a3fd`.
Its final CLEAN matches the source, evidence, body, and scope above.

Both final reviews have zero open findings. The canonical check stays FAIL;
both delta reviews address its shared paths and the explicit ordering resolution.
Lead Q7 and an equal-tree merge receipt remain due. Any later source or import needs its applicable review and evidence.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base-check job. Never poll.

VERDICT: CLEAN (0 open integration findings; final source, evidence, and body) at 4ccd6a7da142331c2d35fa25f5f12914cdf2d22b against base bfef88653bb59ad8d3cd38da1e206f7aba64ce92
