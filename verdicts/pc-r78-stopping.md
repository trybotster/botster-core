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

## Round 2 intake: source repairs close; evidence findings close in the supplement below

Intake disposition: NOT CLEAN (2 open). The completed Round 2 disposition is below.

READY: `msg_plugin-w_1791763632_cc997f`.
Head: `8ac717c7d94560bf1f82a5975bb45162ba0b88da`.
Head tree: `07a89c69acd600e65d27c7b536d30908831f816c`.
Imported source base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Gate base: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
The gate base and this head share merge base `f6025aed`.
The lead approved the #287 import. The own diff has 27 files and changes no conformance list or contract pin.
The reviewer reads the complete repair delta, relevant unchanged paths, changed tests, and retained evidence.
The brief is read first. No reviewer execution job runs.

R78-PC-1 closes in the reviewed source.
`on_process_exited` retains an open Idle/Stop link without requiring `drain_classifies` and emits `LinkPeerEnded`.
The driver handles complete frames and closes an ended link at would-block, EOF, or a read error.
It never waits for EOF. A partial final frame decides no exit.
The chosen end remains guarded separately from link retention.
The driver test delivers both `Done` and `RouteClosed{SessionEnded}` behind `Exited` and checks a single route close.
The revised R2-1 tests distinguish a written close from no written close.
Descriptor ownership and the first write-failure reason retain the prior rules.

R78-PC-2 closes in the reviewed source.
`kill_payload` requires `broken && !gone` before it probes.
The engine tests assert no probe at the grace in both process-exit/write-failure orders.
The live Matches case still ends WorkerUnreachable at the existing grace.
The Absent/Reused case asks for the nonblocking drain when a link remains.

R78-D5 remains closed and its source implementation is checked.
`draining` requires `peer_ended && held.is_none()`.
The full-queue test checks no more work while the frame waits, the wake after polling, and the later exit and close.
The budget test and settle-time exit test check the distinct runnable cases.
The first pump `more` is the v1 expression. The settle reads the links before the second `more` includes `draining`.
The source adds no timer or limit. The ST driver import remains a later slot review obligation.

### R78-PC-3 — MEDIUM, OPEN: IN-9 has an unattributed PASS-to-INCONCLUSIVE result

The new A5-4 record covers `f6025aed` before and this exact head after.
`in_9_modes_read_at_write_start` changes from PASS to INCONCLUSIVE, seed 4, step 8, `step_timeout`.
The testkit result remains PASS. There is no earlier real failure witness for this identifier.
The pinned transcript waits for the held focus write after it releases `pty_blocked`.
The source record identifies the held-write path and shows no explicit stop, process exit, send error, or link loss before that wait.
This is source attribution only. It does not prove a baseline failure.

Lead ruling `msg_plugin-w_1791764208_171fa2` keeps this as a #291 flip and a merge blocker.
PC owns its diagnosis. Required closure is a plain-v1 failure record or an accepted 23bl(A) controlled reproduction of the exact signature.
Both reviewers must accept the applicable record. A rerun-to-PASS and a pending-list change are not allowed.
Rule-9 record: `handoffs/flaky-in_9-modes-read-at-write-start-8ac717c7.md` in the shared directory.
Record SHA-256: `a7d6a3e2c49cb90220018013983ae28f431d58e2588b043b2fd398aa3702c6c4`.

### R78-PC-4 — MEDIUM, OPEN: required local records have a B13 clean gap

`rev4-reversals-1ff76822.log` ran in `targets/pc` after the detached `79f31be3` worktree used that target, without a Cargo clean.
The three revised Mac test-13 records followed in that target without an intervening clean.
Sol reports the implementer's provenance receipt in `msg_plugin-w_1791764007_1b5ccc`.
The current body explicitly confirms that sequence and marks those records historical and unvalidated.
Their source labels and result text do not establish which source the shared target executed.
Cargo-recompile inference does not satisfy the binding B13 rule.

The same lead ruling requires replacement reversals and Mac test-13 records after a recorded Cargo clean or with per-root build directories.
Keep the invalid records and their limits. Provide the replacement logs with the source and build provenance.
The exact-head pool gate remains valid. It does not supply the missing Mac proof or all required reversal records.

### R78-PC-5 — LOW, CLOSED: the body overstated the driver test count

The body said 12 driver tests, while the file has 11 test functions.
The reviewer reported this in `msg_plugin-w_1791763942_bd03ce`.
PC corrected it in `msg_plugin-w_1791764073_92c622`; the reviewer reads the corrected body.
Both input orders remain branches within one test. The body now says 11 tests.
Body SHA-256 at this read: `39ca6b22325c3d73718ac453542db40d2f753682bfbc1f759957174a7395fbe0`.

### Current evidence

The exact-head gate passes ten steps and exits zero after 468 seconds: 173 queued and 295 running.
Default: 1822 passed. Slow: 600 passed. Mutation total: 62, with 60 caught, zero missed or timeout, and two unviable.
All 60 raw caught identities match all 60 named citations and normal PASS records. No FLAKY line appears.
The two unviable identities retain the earlier source reasons. The Linux zombie proof catches the two Stage-2 mutations.
Gate SHA-256: `0bfc1d41038bd978bd0a91bf4547acb5b371bf63cf1886f9f0d9e8ac11723e2f`.

The `2eee84ba` gate remains RED. Its two redundant first-more mutations survive Stage 2; fuzz does not run.
The later fix removes that redundant term. The kept settle-time term has named catches in the current gate.
The retained `gate-mutant-fix-8ac717c7.log` records four failures and a restored 320/320 PASS.
Its SHA-256 is `ff4a800f793a09b2cee0203c6a73e80b0c146937a673548f6068b7112a6438b6`.
The current body reports a clean and sole-worktree build condition for this record.

The old-production/new-test record retains five failures out of eight tests, including the after-Exited route close and both input orders.
It records only the driver test file and attach-helper visibility as dirty on `79f31be3`.
Log SHA-256: `1fb0d0de515a18a6e87214a10b9c3390dd34780890e7793a09007c5a842b17dd`.
The current body reports a clean and sole-worktree build condition for this record.
The no-probe reversal retains one failure and a restored 12/12 PASS; hash `0b8dc7ccb27950b45aabbd235a1daf651ccd0c78b6fe3b6480a5700f6bcc41c6`.
The two invalidated record groups remain excluded from validated evidence under R78-PC-4.

The new A5-4 pool records use testkit seeds 0–31 and real default seeds 0–7 on Linux.
The reviewer independently parses both tables and matches every cell: 690 ids each, zero missing, extra, or mismatched cells.
Testkit: 230→230 must-pass PASS, zero changes. Real: 205→203 must-pass PASS, 10→9 pending-real PASS, six changes.
The four other changed ids remain pending-real. This PR makes no pending change.
Before log SHA-256: `1d1f752e7f95ad2583fd21f42629bf665bea532937057a7f535e3dc440b6cbbc`.
After log SHA-256: `f3426eb9e0baf6a10e732e9cb871f4c0915a5481b8b980e8881a3114cbc17391`.
Real table SHA-256: `b7504020fa7d79fca6d9995e256407a2939ef1aea62b53c91f15e8db4f320719`.
Testkit table SHA-256: `2aa5f64fdb8075e1809974d5da36701e1e91cfe9af8d12c3df6bc01962a670b8`.

ST-6 also changes PASS→FAIL, seed 2, step 17, expected `ne[1,1]`, received “the relation does not hold”.
The lead attributes that exact signature to v1 in `msg_plugin-w_1791764208_171fa2`; the seed is not part of the signature rule.
The accepted controlled reproduction uses plain base `f6025aed` and scratch snapshot `bf55aa2fb5615c58333f87990433e127224d891e`.
The reviewer reads the matching package and integration acceptances, `843604d707cfb4a6a9049e54da46fba2ff4a001d` and `5fa42e4a` Round 6.
The reproduction log has the same step, predicate, and symptom; hash `a21e40b2b2db7710acd8cc0eb3572a726cd27f3720debcdde6db0f49e7428c09`.
It is an instrumented timing witness, not an uninstrumented baseline PASS or a recovered trace of this failure.
The lead requires this written non-interference condition: #291 changes no model-feed or capture path.
The own diff changes neither the worker model-feed/capture code nor the terminal library; the transcript reaches no new loss branch before step 17.
That source-disjoint condition holds against the imported `f6025aed` base.
ST-6 therefore does not count as a #291 flip under the lead's case decision. R-80 supplies its repair through .42.
The raw failed result remains FAIL in the record.

Both source findings are closed on this head. Two evidence findings block CLEAN.
The final slot import, ST driver resolution, exact-head final checks and gate, both final reviews, completed body, and Q7 remain due.
No reviewer execution job runs. Wait for the evidence doorbell. Never poll.

### Round 2 supplement: replacement records and controlled attribution close both evidence findings

R78-PC-4 closes with the replacement records supplied in `msg_plugin-w_1791764392_448eb6`.
The reviewer reads the replacement logs and verifies their hashes.
The first record names exact head 8ac717c7, a clean worktree, and no other process using the target directory.
It records `cargo clean` removing 41,973 files and 10.0 GiB, then records the target directory as absent.
PC states that only this worktree builds in that directory through the last Mac facade run.
R1–R5 fail eight, one, one, ten, and one tests respectively. The restored source passes 320/320.
The prebuild record names the three candidate binaries and records their manifest hashes.
Each later Mac facade header names the same head, clean sequence, and candidate build.
All three normal runs PASS, in 0.395, 0.323, and 0.366 seconds.
These filtered records omit full decoded commands and reversal patches. The retained headers and PC's statement establish their build scope.
The earlier records remain historical and unvalidated. These replacements do not validate the earlier runs.

Replacement files are in `~/botster-sessions/shared/core-stage1/evidence/pc-r78/`:

- `rev4-reversals-clean-8ac717c7.log`: `ddfa762de9915a973aa0f54add562925390277cce1dcdf41ad08aac0ef2ef18b`.
- `prebuild-clean-8ac717c7.log`: `4b2b6a02ff2be0453bd95319c5807105db3cb1fa97c4bc4fd136b37744cab0e2`.
- `t13-mac-clean-8ac717c7-run1.log`: `20b97989952ae726977c6113731c7441c692eb155b10f1fd879e7a919ed75acb`.
- `t13-mac-clean-8ac717c7-run2.log`: `fb892f05b1150587cf3a3caea0a420b3e4ab8394696a127001531108dbcfdbe3`.
- `t13-mac-clean-8ac717c7-run3.log`: `d1a3b44923fa02524efc148a73fa8d2d218bd1805f3b408e629f62bb1712e661`.

R78-PC-3 closes with the controlled reproduction under plan 23bl(A).
PC supplies the proof in `msg_plugin-w_1791764613_e5f9bc`.
The lead accepts this case in `msg_plugin-w_1791764762_105387`.
Sol receives the same case decision as `msg_plugin-w_1791764762_92795e`.
The reviewer reads the complete raw log, scratch diff, pinned transcript, runner, probe, worker loop, and focus encoder.
The reproduction starts from plain v1 `f6025aed4cdf7da836e835e0040fc128b7e8f32f` without #291.
Its only scratch change is in `crates/botster-core-testkit/src/real/program.rs`.
It delays CSI ?1004h output by 300 milliseconds through a thread that calls the unchanged output request.
The scratch changes no Core code, input encoding, transcript byte, result predicate, or timeout.
It is never committed to delivery source.

Scratch file: `~/botster-sessions/gates/pc-in9-repro-scratch-f6025aed.diff`.
SHA-256: `0151f1ecce86b2fb27ea9fb2c7456f143da417a2473beba29fc0941c593aed01`.
Log: `~/botster-sessions/shared/core-stage1/evidence/pc-r78/in9-repro-f6025aed.log`.
SHA-256: `6d52750fe109e57c4e7d13d75f237875eda1e274dfc5b6c63984dbf4b003bad3`.
Both embedded patches decode to the retained diff, byte for byte.
The log records the exact checkout, contracts ddce507e, successful patch application, sole changed path, and candidate prebuild.
The msa1 Linux job records one failed test, 689 filtered ids, and 10.37 seconds.
It reproduces INCONCLUSIVE for this id at seed 0, step 8, limit `step_timeout`.
The original A5-4 failure has seed 4, the same id, step, and symptom.
The lead's signature rule excludes the seed.
The job and wrapper exit zero because the command filters output and finishes with an echo.
The log supplies no numeric underlying test exit. Wrapper zero does not change the raw failure to PASS.

The output acknowledgement does not fence the worker's PTY read or model feed.
The worker serves its control link and settles requests before it reads a PTY chunk.
The focus request can therefore start before the mode bytes reach the model in plain v1.
The diagnostic widens that existing timing window. It adds no missing behavior.
When focus reporting is off, `encode_focus` returns `None`; `model::encode` returns `NotWrittenReason::NotReported`.
The worker can emit `Completed{NotWritten(NotReported)}` before `ModesChanged`.
Step 6's `pump_until` discards events that do not match its matcher.
Step 8 can then wait for an already-consumed completion and reach `step_timeout`.
The real probe's blocked flag stops stdin reads. It does not itself hold this focus write with WouldBlock.
PC corrects the earlier diagnosis in `msg_plugin-w_1791764914_296895`; the reviewer reads the complete corrected record.
Corrected record SHA-256: `8ff1d2f6c28b09ad680e74f3ee7753815fcf26c1c21c6308fa70fc997a4d7cf8`.

#291 changes none of the worker loop, focus encoder, runner, or output-control paths used by this mechanism.
No Stop, process exit, send failure, or link loss occurs before step 8.
The new link flags and branches remain inactive. The first `more` expression matches v1.
This source non-interference condition supports the controlled attribution to v1.
The log has no event-order trace. The mechanism is inferred from source and the retained result.
This acceptance claims no controlled seed-4 run, original A5-4 event trace, or uninstrumented-v1 failure witness.
The earlier uninstrumented before run remains PASS. The after failure remains INCONCLUSIVE.
No pending list changes, transcript changes, or rerun-to-PASS occur in #291.
PC owns the removing contracts PR: the R-88 held-write PR under the steward's R-74 form.
The steward rules the form through the lead. That separate PR also covers the named TM-6 and IN-6 held-write items.

Sol's Round 8 and supplement are read at `e72d7d01dc22a5eac71d57ea2e3f63c53fa32028`.
Publication is verified on `origin/stage1/integration-review-sol`.
Sol closes the source findings and B13 gap, then accepts the same IN-9 controlled attribution.
The package reviewer independently accepts the attribution in this supplement.
Both required controlled-attribution acceptances are now present.

All package source and evidence findings close on 8ac717c7.
The current body check and final-slot obligations are recorded below before the completed verdict.

## Round 3 intake: #295 import and exit-action ordering

Delta request: `msg_plugin-w_1791772688_d78b20`.
Head: `4ccd6a7da142331c2d35fa25f5f12914cdf2d22b`.
Imported base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Plain merge: `b8fee77332770736f43886b35c387eef3fc996c4`.
Its parents are reviewed 8ac717c7 and the imported base. Their common ancestor is f6025aed.
The brief is read first. The reviewer checks the complete resolution, shared import paths, and affected unchanged callers.
This is a source assessment in progress, not a completed exact-head delta verdict.

The resolution adds `perform_counted` after the first `take_exits`, before the first link service.
This call performs every queued action, including cancel and handoff actions queued before the pump; it is not an exit-only filter.
The reviewer checks those admission paths and all named exit sites.
For Idle/Stop, the action sets `peer_ended` before the link read. Complete frames retain their order and the chosen end remains guarded.
For Start and Adopt, the existing close action executes before the read; those branches retain their prior failure policy.
Remove AwaitTeardown retains its existing result/EOF path. Other Remove phases retain their known-gone rule.
The existing counted action path charges its posted events and budgets failed handoff inputs.
The resolution adds no engine work step, clock read, timer, limit, process reap, or descriptor ownership rule.

The ended-peer WouldBlock branch now calls #295's `end_link` with the current budget.
An exhausted budget retains the close reason in `link_ends` and sets `more_input`.
The next link read handles a held frame first, then the retained close. Final close removes the retained end.
EOF, read errors, and ended-peer WouldBlock therefore use the same carried-close rule.
The first `more` remains `runnable || more_input`; the post-settle `more` also includes `draining()`.
A late exit still reaches the post-settle action pass and carries its unperformed drain to another pump.
An unaccepted mandatory-room frame stays parked; a budget-held frame remains runnable through `more_input`.

The reviewer reads all eleven driver drain tests and the three named handoff/deadline exit sites in each file.
The changed no-frame test now requires WorkerGone in the exit's pump, after the immediate nonblocking drain.
The kept-exit test checks Running after the spent-budget pump and WorkerGone after the next pump.
The other cases retain complete Done/Exited/RouteClosed order, both exit/send-error orders, grace priority, partial-frame rejection, and backpressure.
The handoff cases retain one stream close after the session end and preserve descriptor ownership.
No source defect is identified in these paths.

All five conformance blobs exactly equal imported v1: ledger 691, testkit pending 410, real pending 44, real-only 26, held 3.
The independent three-way set checks find no missing or extra id.
All imported non-own blobs match the base. The only non-overlapping own blob changed after 8ac717c7 is the reviewed no-frame test.
The own diff still has 27 paths. It changes no worker model-feed, capture, focus encoding, or output-control implementation.
The prior ST-6 and IN-9 attribution limits remain applicable to the source prefix; no end occurs in either failing prefix.
Sol's source assessment in `msg_plugin-w_1791772937_5fc763` reports no finding on the same head and base.

The reported plain-merge test failure and 331/331 restored host PASS remain owner reports until the records arrive.
The static, strict, host, and canonical base-check records remain due. The checker result is not inferred from its expected failure.
The current body and one final exact-head gate remain due. No running output is read. No reviewer execution job runs. Never poll.

### Round 3 completed records and body corrections

PC receipt `msg_plugin-w_1791773040_a23a33` supplies the completed static, strict, canonical, and delta records.
The reviewer reads the static and canonical logs, local script, interdiff, strict summaries, and affected normal test records.
The retained own diffs and resolution diff exactly match independently generated Git diffs of their named source pairs.
The strict header names 4ccd6a7d, strict mode, and seeds 0–31. The run ends with exit zero.
The reviewer totals the suite summaries independently: 1221 passed, zero failed; host 331/331.
All eleven drain tests and the named handoff/deadline exit tests have normal `ok` records.
The same-worktree history after the clean at 2026-10-12T02:30:45Z is PC's recorded statement, not an independent reviewer build.
These supporting local records do not replace the exact-head final gate.

The canonical check passes ancestry, condition 1, and both condition-4 pending checks.
Conditions 2 and 3 FAIL for the five shared paths and changed own diff.
The retained result remains FAIL and requires both reviewers' delta rounds.
The reviewer does not execute the checker or infer a PASS from the reviewed source.

Evidence SHA-256 values:

- `xtask-static-4ccd6a7d.log`: `166df8757594e12d7b6ab5298f90de4e7783894e37fbb6120f4d8861ff4e5953`.
- `local-check-4ccd6a7d.log`: `d39235a05ed9bc9ed4e46f1145c975a17cb02b2a15b6264f87559d8c087c55b3`.
- `r78-final-local.sh` as executed: `3a091797a46287fc9ccd2fedc398946cf0907246307b5048c3976c36db1dac51`.
- `own-diff-8ac717c7.diff`: `0d20434952183cd6dba4f3b8ff37ffdf5b8501b9d53ef6d9c63420e4ab0823b2`.
- `own-diff-4ccd6a7d.diff`: `75c18d7683e555cf26548c4b0989d891a3888b8ee3c3da0d80bd48121bc0a2be`.
- `own-diff-interdiff-8ac717c7-4ccd6a7d.txt`: `28ecf75c2d0b19cf28ab6c61eb221346792f44eff84d39aafe5be39aac386c4d`.
- `resolution-4ccd6a7d.diff`: `f9dc7ddf26a2ac92d25e3cfaf35da1869ec09eb6166b4aff0cb8d1070c3dd1c3`.
- Canonical log `~/botster-sessions/gates/botster-core-pc-base-merge-check-8ac717c7-4ccd6a7d-20261012.log`: `02c50f1f1c26a5d10d14dca9da93f866a87bc7bc331d77147b8119dcb9db2ab4`.

R78-PC-6 — LOW, OPEN at the first completed-body read: body precision and local launch record.
GitHub reports exact head 4ccd6a7d and base bfef8865. Body SHA-256 is `ecf28a3eeb2a210cf2b083ffb7cd1f3b211e4170e789ce5a56e5ff8d620c2a93`.
The new slot section correctly names the merge, resolution, whole-action-queue ordering, and gate still due.
The body includes the replacement B13 records and accepted IN-9 attribution.
However, its A5-4 paragraph still says step 8 waits for a held PTY write and gives source attribution only with no earlier witness.
That is the withdrawn real-tier diagnosis. The corrected rule-9 record instead identifies possible NotWritten(NotReported) before ModesChanged.
The controlled witness and both independent acceptances must retain their limits consistently in this paragraph.
The body also says all eleven xtask checks exit zero, although the public-api first attempt exits one.
There are ten distinct checks and eleven attempts: nine initial passes, one refusal, and one corrected pass.
The executed local script omits the required cap variables from its Cargo launches; the public-api refusal confirms the missing cap.
Required correction: preserve the historical records, correct both body statements, and set both cap variables for future script launches.
No replacement test run is required for these corrections.
PC reports the count correction in `msg_plugin-w_1791773131_177f48`; its corrected-body read remains due.
The package request is `msg_plugin-w_1791773150_87f985`; Sol independently identifies the count issue in `msg_plugin-w_1791773113_c87772`.
No source logic finding remains. The completed body correction, formal delta verdict, matching integration verdict, and final gate remain due.

### Round 3 completion: corrections close; delta CLEAN

VERDICT: CLEAN (0 open package delta findings; exact-head final gate remains due)

Head: `4ccd6a7da142331c2d35fa25f5f12914cdf2d22b`.
Base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
R78-PC-1/2 remain closed in this source. R78-D5 remains closed after the #295 composition review.
The Round 2 replacement records and controlled-attribution acceptances close R78-PC-3/4 with their recorded source and execution limits.
They do not supply an exact-final-head gate.

R78-PC-6 closes after receipts `msg_plugin-w_1791773188_ff0bd2` and `msg_plugin-w_1791773308_1b2aef`.
The reviewer reads the complete body changes against the previously read body.
The IN-9 paragraph explicitly withdraws the held-write diagnosis and states the corrected conditional mechanism as source inference.
It retains the missing event-order trace and accepted controlled-v1 attribution. No uninstrumented-v1 failure is claimed.
The body correctly names ten distinct xtask checks, the public-api exit-one refusal, and its capped exit-zero rerun.
The historical script remains unchanged. The future copy `r78-final-local-capped.sh` exports both required cap variables.
The reviewer reads that copy; its SHA-256 is `494039e7f285d17848a42e27b647db33cb7078273d5f950827b91578fff568cf`.
No replacement execution is inferred from that unexecuted future script.

R78-PC-7 — LOW, CLOSED: the retained failed run was described as an untouched plain merge.
Sol identifies the scope limit in `msg_plugin-w_1791773267_4942d4`.
The reviewer reads the retained header and failure at `~/botster-sessions/shared/core-stage1/evidence/pc-r78-merge-bfef8865.log`.
SHA-256: `91c6f6238bc60d0f0e355a089651f2e835bceb07337ecccc9648d1f92ff5ce58`.
Its header names b8fee773 plus an uncommitted drain-end fix, not untouched b8fee773.
It records Cargo clean removing 15,520 files and 4.1 GiB at 2026-10-12T02:30:45Z.
The host run reports 330 passed and one failed. The kept-exit assertion receives Running instead of Lost(WorkerGone).
The log supplies no numeric underlying test exit.
The body now names that exact scratch source and states that the plain-merge output was not retained.
The unretained run is an owner report, not accepted raw evidence.
Source review supports the ordering diagnosis independently: the drain-end fix cannot act before the action installs `peer_ended`.
This assessment claims no untouched-merge execution proof and requests no extra run.

The complete package record is `evidence/pc-r78/package-4ccd6a7d.md` in the shared directory.
Its SHA-256 is `e9a8f860cd021c9ef8e44e28cd79d7c8d5fef4fd47b7b7f8f4b9c56360da927e`.
The reviewer independently matches all nine conformance list blobs to the imported base, including deferred, withdrawn, proofs, and minimum files.
All retained delta hashes match their source pairs. The own diff changes no pending list or contracts pin.

GitHub reports exact head 4ccd6a7d and base bfef8865. Remote delivery head and v1 match those identities at this check.
Live body SHA-256: `da2fa54e4a1df2f2ef7c68f473fa41460c25579500b19939390f4cdf91ce8c32`.
Retained body file SHA-256: `3aaf938eadf9c3423462a394d5884a28f5be9d52f6442fb21453e0906d002638`.
The texts match except for one final newline: live has two, retained has three.
The body accurately names the whole-action-queue ordering, retained source scope, current static results, and final gate still due.

This is the complete package source delta, static evidence, and pre-gate body approval on 4ccd6a7d.
It accepts no later head, gate result, or evidence substitution.
Sol's formal matching delta verdict and one exact-head final gate remain due before final evidence approval.
The lead's Q7 check remains required before merge. No reviewer execution job runs. Never poll.

### Matching Round 3 integration delta receipt

Receipt: `msg_plugin-w_1791773769_91a53d`.
Sol verdict: `718699173b786c3bb74e3201db7ad28073cf79d9`, `verdicts/stage1/pc-r78-stopping.md`, Round 9.
The reviewer reads the complete round and verifies publication on `origin/stage1/integration-review-sol`.
Both delta CLEANs cover head 4ccd6a7d, base bfef8865, and tree `ed7ce2ee718c20284fbb1543f6567ce5e6e9c7be`.
Both reviews preserve the canonical FAIL, actual scratch source, cap refusal, corrected body, and older evidence limits.
The formal matching integration delta condition closes. Both required source delta rounds are complete with zero open findings.

The reviewer also reads both failing transcripts at the imported contracts-v0.1.41 pin.
Neither prefix uses Cancel or Attach before its named failure. Neither prefix reports an exit, failed send, or lost link.
Thus the additional initial action pass supplies no new cancel, handoff, or exit action on those prefixes.
This strengthens the written non-interference condition for the carried ST-6 and IN-9 attribution.
It is source reasoning, not a new-head A5-4 run or a recovered event trace.
One exact-head final gate, mutation audit, final body and evidence reviews, and the lead's Q7 check remain due. Never poll.
