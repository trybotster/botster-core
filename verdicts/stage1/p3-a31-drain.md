# Core #318: integration review

## Round 1

- Reviewer: Sol-C.
- Branch: `stage1/p3-a31-drain`.
- Head: `7010f7556b2c18c6f43d2166769d580d53c123dd`.
- Tree: `2f68ed762af11f0b90bdbf0b2b6b02caf3ac8317`.
- Source base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
- Gate reference: `9fe97f506a1bb5cef0d935b2ba2ccf3ff9ba90ec`, not imported.
- Reviewed delta: the complete eight-file diff, 563 additions and 141 deletions; shared Drain, real-driver wiring, testkit and real tests, documentation, body, and supplied evidence.
- Lead assignment: `msg_plugin-w_1791827743_d06cad`.

I read R-108 and R-108-E1 at contracts `c43c4458`, together with the applicable R-107 follow-ups. The drain repeats counted and flushing parts. It stops at a quiet read or EIO/EOF, without a byte cap. The design records the continuous-writer gap under A24. The later OU-7 identifier and its pending status remain outside this PR.

### Integration source checks

Both drivers use the shared Drain decision. A completed count causes another flushing read. A positive flushing read measures the next counted part; a zero count still causes another flush. A count never ends a busy drain. The real driver returns the measured-count error rather than claiming a completed drain.

The real driver limits each read by its chunk and OU-3d route budget. It handles PtyOutput through settle before another read. A spent budget prevents reads; Drain::Done can still queue PtyDrained. The testkit applies the same decision and retains the same route-budget boundary. No extra production test branch or terminal model appears.

The real driver boundary test proves output beyond the first count on Linux. Its wait implementation requires repair below. The twenty-run session test checks title before Exited and complete route bytes on a Stop/Kill path. The testkit checks the host's shortest one-millisecond grace across 32 seeds. The old-rule reversal fails the shared decision and real driver tests; the twenty-run session test passes that reversal, as the body explicitly states. It is ordering evidence, not the deterministic proof of the omitted-count repair.

The source changes no pin, conformance list, mutation configuration, timeout, or process allowance. This head predates #317. A later import must preserve its known-flaky entry and receive the applicable delta review.

### Open findings

The package reviewer supplies these same-head findings in `msg_plugin-w_1791827812_d3897b` and `msg_plugin-w_1791827914_d4772d`. I independently confirm their source or evidence premises. The complete committed package verdict remains due.

- **F150 — LOW — open.** The real driver's touched comment still describes one flushing read and at most one subsequent count. Drain's comment assigns EIO to Linux and EOF to macOS. Update both comments to the repeated, uncapped rule and R-108-E1's end conditions on either OS.
- **F151 — MEDIUM — open.** The new driver test waits in repeated 50-millisecond exit-channel slices and uses each timeout to trigger PTY reads. This is periodic polling. Wait for actual PTY readiness and the production exit wake under one outer failure deadline. Preserve the Linux count-gap proof and macOS reads before exit.
- **F152 — LOW — open.** The body claims that the Mac measurement covers F28's required native pending_output mutation proof. It contains an unmutated measurement only. Supply that proof or correct the claim and retain F28 as a separate Part B duty under the lead's scope. No F28 closure follows here.
- **F153 — MEDIUM — open.** The silent-holder test reports the parent shell's marker only. It does not prove that the child still holds the PTY at Exited. Add the holder's own readiness and live-holder proof, with the leader-exit signal handled, while preserving bounded cleanup and group ownership. The test must distinguish a quiet read with a live holder from EOF after all holders close.
- **R318-I1 — LOW — open.** The body calls the tail-test sizes 4096+8 and 1000+8 bytes. Their title literal contains seven bytes; the retained totals are 4103 and 1007. Correct the body sizes. I sent this observation to the package reviewer in `msg_plugin-w_1791827983_b7697f`; a package alias can refer to this same finding.

### Completed gate and evidence scope

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-a31-drain-7010f755-pool-20261012-104317-7194.log`.

Independently verified SHA256: `71e56ea16da1f823c92260f513789af3cc90a101a1b89e472246cca99ff98e46`.

All ten steps pass. Default tests report 1939 passes and 431 skips. Slow tests report 626 passes and 2070 skips. I matched 25 unique raw mutation outcomes: 21 caught and four unviable. All 21 catches match citations and normal PASS records. Stage 2 catches the five misses after its unmutated baseline passes. The four unviable outcomes require a missing Drain default or a non-exhaustive match.

EV-2 is FLAKY under the main.rs:556:17 mutant. Its fail-then-pass trial receives no catch credit. The adoption test catches that mutant instead and has normal PASS. This implements the fail-closed mutation exception; it does not turn an ordinary failed gate into acceptance. The branch lacks #317's mutation-stage exclusion. Fuzz selects no changed decoder crate. Job and wrapper exit zero after 666 seconds on msa1; CI takes 652.1 seconds.

The Linux measurement log records 4103 written bytes against a first count of 4095. The Mac measurement records 1007 bytes read before EOF; larger attempts block at count 1024. These historical measurements support R-108-E1 but prove no native mutation outcome. The local checks name historical head 37fc16f2, and the earlier clean names a modified worktree based on 838e4bb0. Their scopes remain historical. The exact-head pool gate supplies current execution evidence, but it does not close the findings above.

Five findings remain open, including LOW findings. The same-head package verdict, repairs, and replacement-head evidence remain due. I accept no later head, train, gate, pin, A24 decision, or F28 closure. I ran no build, test, gate, or pool job.

VERDICT: NOT CLEAN (5 open)

### Same-head package record and finding alias

I read complete package Round 193 at `fe92a9618a2e3e3938d92dfd4df94571b2889df1`, file `verdicts/p3-worker.md`. It reviews the same exact head, complete source delta, and gate. It retains F150 through F153 and the same evidence limits.

The package record includes my seven-byte title correction within F152. R318-I1 therefore aliases that part of F152. The earlier observation remains open; it is not a fifth independent finding. Four finding identities remain: F150, F151, F152, and F153. This count correction supplies no source repair or closure and does not start another review round.

VERDICT: NOT CLEAN (4 open)

## Round 2: first repair

- Head: `d19cc1a6203e15f2e861c21cbbce0720a204db6d`.
- Tree: `f69086665bd5aca795a0a4f5b8c70c0307f703c9`.
- Parent: `7010f7556b2c18c6f43d2166769d580d53c123dd`.
- Source base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
- Gate reference: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`, not imported.
- Reviewed delta: one commit, four files, 75 additions and 27 deletions; documentation, event wait, and holder test.
- Package verdict: `307ce71c188cb1c2dc128ce4ca924868a1607ced`, file `verdicts/p3-worker.md`, complete Round 194.
- READY receipt: `msg_plugin-w_1791829030_d72c25`.

I read the complete same-head package verdict and complete source repair. It closes F150, F151, and F152 and retains two findings. No production drain behavior changes in this repair.

**F150 closes.** Both comments now state the repeated, uncapped drain and EIO/EOF on either OS. **F151 closes.** The tail test waits on the driver's actual poll and production EXIT wake under one absolute deadline. Interrupted waits use the remaining time. Mac readiness permits reads before exit; Linux retains the full unread tail for DrainPty. The count-gap and complete-output assertions remain.

**F152 closes, including R318-I1.** The same-head package record verifies the corrected body: seven-byte title, 4103/1007 totals, and F28 retained as a separate Part B duty. No native mutation closure follows from the measurement.

**F153 — MEDIUM — remains open.** The holder now supplies its own marker and a response after a release sent only after Exited. That proves survival in a successful run. Linux's tty response supports a quiet read with a live holder. Mac's none response supports process survival only; the body states that limitation.

The parent can still exit before the child installs its ignored HUP disposition. The later marker does not order that earlier exit. I independently identified the same race. Establish the disposition before the fork, or keep the leader alive through a real readiness handshake. Preserve the post-Exited proof and bounded cleanup.

**F154 — LOW — open.** The new Linux assertion claims that PTY readiness came before exit. Spawn already sets pty_readable true, and this path does not clear it. The assertion therefore proves no event. Verify an actual event from a cleared state, or remove the redundant assertion and its body claim. Removal also avoids asserting that subject flag.

Current gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-a31-drain-d19cc1a6-pool-20261012-110920-28759.log`.

Independently verified SHA256: `9ff99f68b79506f1fca3929ec044633a22912db44c7964adbaf1c6d14416d0cd`.

All ten steps pass: 1939 default and 626 slow tests. I match all 21 raw catches to citations and normal PASS, with four unviable outcomes and no final miss or timeout. The slow baseline passes. No FLAKY token appears. The local Mac record names this head and reports successful checks and test tiers; its unchanged-worktree cache claim retains the earlier clean scope. Passing runs do not remove the holder's permitted failing schedule.

Two findings remain. The repaired exact head, matching closure, final import, applicable train reviews, final gate, and receipts remain due. F28/F33/F39 retain their separate duties. I ran no build, test, gate, or pool job.

VERDICT: NOT CLEAN (2 open)

### Integration round 3 in progress; package round-limit settlement

Owner receipt `msg_plugin-w_1791830581_74bc40` supplies b0503426. I read its complete two-file repair and completed gate. The HUP disposition now precedes the fork. The test clears the seeded readiness flag, but its touched assertion still reads private driver state. I request an observed-event assertion in `msg_plugin-w_1791830698_506bec`.

I read complete package Round 196 at `0e8feb29c4954a3d41e3039d252a760436ba8dac`. It closes F153 and retains F154 LOW for that assertion. This is the package's third NOT CLEAN round. It records the separate Mac IN-6 classification duty assigned to P3, which the lead states does not block #318. No local failure is erased or diagnosed by an isolated pass.

Lead receipt `msg_plugin-w_1791830912_a6335f` explicitly SETTLES package round four on `c707c4d6b89f3e96d8884713c818151a10ad6e58` with one gate. It directs me to include that delta in my current integration round. Package receipt `msg_plugin-w_1791830944_6a1190` confirms the same limited authorization.

I read the six-line c707 delta. The assertion uses a local flag set from returned PTY poll events. The driver flag remains setup for reads and is no longer asserted. No production behavior changes. This observation is not a final closure or exact-head evidence acceptance.

Integration round three remains in progress. The replacement READY, its completed gate, and the authorized same-head package verdict remain due. No new formal integration verdict is issued here. I ran no execution job.


## Round 3: completed repairs and authorized F154 settlement

- Head: `c707c4d6b89f3e96d8884713c818151a10ad6e58`.
- Tree: `79fb1e20839f75ee94fcd5691555e5b6bce8a20c`.
- Source base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
- Gate reference: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`, not imported.
- Reviewed delta: complete b050 repair from d19, then the six-line c707 test repair. No production behavior changes.
- READY: `msg_plugin-w_1791832118_983541`.
- Package: `2e8363da579f90b059fcf03c15a43b0a1b0262e2`, complete Round 198 in `verdicts/p3-worker.md`.
- Lead SETTLE: `msg_plugin-w_1791830912_a6335f`. This completes the integration round already in progress.

I read the complete same-head package verdict. Round 197 reviewed #302; Round 198 supplies this authorized package settlement.
No finding appears outside the lead's authorized repair scope. Earlier source reviews and their limits remain applicable.

**F153 closes at b050.** The leader ignores HUP before it forks the holder. The holder inherits that disposition from its start.
The holder's marker and response after Exited remain. Linux proves a quiet read with a live terminal holder.
Mac proves process survival only. The complete body retains that distinction.

**F154 closes at c707.** The assertion reads a local saw_pty_event flag initialized to false.
Only a returned PTY poll token sets that flag. Driver readiness remains read setup and supplies no assertion value.
The absolute deadline, actual exit wake, Linux unread tail, count gap, and complete-output checks remain unchanged.
No subject getter or production branch is added. F150, F151, and F152 retain their earlier closures.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-a31-drain-c707c4d6-pool-20261012-115220-30004.log`.
Verified SHA256: `8da758f002695b5b656f17e13a3cdb22b1013e25d0fe2f1f958687463e1f06fc`.
Its headers name the exact head and distinct gate reference. All ten steps pass: 1939 default and 626 slow tests.
I match all 21 unique raw catches to citations and normal PASS results. Four mutants are unviable.
Stage 2 catches all five Stage 1 misses after its unmutated baseline passes. No final miss, timeout, or FLAKY remains.
The changed driver-tail test passes in the normal slow tier and the baseline. Fuzz selects no changed decoder harness.
CI takes 441.1 seconds. Job and wrapper exit zero after 948 seconds, including 496 seconds queued and 452 seconds running.

I read the exact-head Mac pre-gate log. It records removal of 12876 files totaling 3.6 GiB.
Fmt, actual CI taint, and worker clippy with slow exit zero. It supplies no new Mac slow or strict run.
The complete PR body SHA256 is `b3ec0db9288cca92a21e33e897acb007db923327d2641940bbf4dd1f8f19b17b`.
The body preserves earlier gates, measurement limits, reversals, F28's separate duty, and the Mac IN-6 failure.
P3 must classify that failure from source and logs under the lead's ruling. An isolated pass supplies no cause or closure.
The lead states that this separate duty does not block #318.

All five finding identities, F150 through F154, are closed, including LOW findings.
Final import/check, applicable train and combined reviews, exact-final-head gate, final receipts, and equal-tree merge verification remain due.
This verdict accepts no future head, train, pin, pending removal, A24 rule, or F28 closure. #163 remains open for separate duties.
I ran no build, test, measurement, gate, or pool job.

VERDICT: CLEAN (0 open, including LOW)


### Matching package acknowledgment

Package message `msg_plugin-w_1791832513_c72949` confirms that the reviewer read complete integration Round 3 at `adda76b49f727bb88db1e59949968586b57e1edb`.
The package handoff records both CLEAN reviews at c707c4d6 and the F150–F154 closures under the authorized settlement.
No new package round follows. Final import and train evidence remain due. Mac IN-6 classification and F28 remain separate.


### v1 import inspection: acd6ced0

Owner delta request: `msg_plugin-w_1791837252_eddacd`.
Head: `acd6ced03f7306cbf250f25d668b239ef7d70b33`.
Tree: `3564fc0fceb0af604e73c041ced018236f8f7902`.
Imported source and gate base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Merge `22493716793572d712080eb991d253fb22b47894` has c707c4d6 and that base as its parents.
The owner reports no textual conflict and rerere off. The final commit adapts one Workers::new call to WorkerClock::manual(start).0.

I read the complete adaptation and the affected test. Its manual worker clock stays at start.
Core's injected host time advances through stop_grace and supplies KillPayload. No worker deadline is needed for that kill or drain.
The test still observes title before Exited Killed and the Stop completion across 32 seeds. No subject getter is added.
Seven of the eight own file patches retain their exact additions and deletions against the new base.
The eighth, core/tests.rs, differs only through this constructor adaptation.
The imported resize result, worker-clock interface, build-location macro, and report-hold fixture coexist with the drain.
The driver still settles PtyOutput before another read. Resize results enter the same queued-input path without changing Drain.
The repeated counted/flush rule, quiet-read/end conditions, no byte cap, route budget, and prior finding closures remain unchanged.
All nine conformance files equal the imported base. No new pending removal, pin, mutation rule, or exception appears.

I read the complete body through its unchanged earlier content and added v1-import section.
Exact body SHA256: `631492b0f763f6b9f0b6a320a1eed01d938b9266a452b3c9dbcf29498a009999`.
It records the semantic conflict, repair, local checks, and completed exact-head gate. Earlier evidence limits remain.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p3-a31-drain-acd6ced0-pool-20261012-132317-50401.log`.
Verified SHA256: `fbe1d79466d22b97ae660cb14529092a7aed629694fac82037105c47ccc7bfe4`.
The header names this exact head and imported base. Both parallelism caps are four. All ten steps pass.
Default passes 2081 cases; slow passes 682. The repaired testkit title test and both real title tests pass normally.
I match all 21 unique raw mutation catches to citations and normal PASS. Four outcomes are unviable.
Stage 2 catches all five Stage 1 misses after its unmutated baseline passes 692 tests. No final miss, timeout, or FLAKY remains.
Fuzz selects no changed decoder harness. CI takes 590.0 seconds; job and wrapper exit zero after 617 seconds, with no queue wait.
This gate names the imported member, not a future train head.

The local Mac log names this head and records removal of 17151 files totaling 4.4 GiB after the worktree switch.
It reports fmt, CI taint, xtask checks, lists, clippy, default 1116, worker slow 148, and strict 581 at seeds 0–31 with exits zero.
That record supplies no independent history of other target writers. Earlier failures keep their historical classification limits.
The separate Mac IN-6 diagnosis and F28 remain separate duties; no closure follows from this later green run.

No import finding is identified. The complete same-head package delta remains due before formal integration CLEAN.
No final train or merge acceptance follows. I ran no execution job.


## Round 4: v1 import delta CLEAN

- Head: `acd6ced03f7306cbf250f25d668b239ef7d70b33`.
- Tree: `3564fc0fceb0af604e73c041ced018236f8f7902`.
- Imported source and gate base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
- Complete source and gate inspection: `b14165e0`, above.
- Package: `4aaa3091318b37911d29f7ea08d3fc9ac05db60e`, complete Round 201 in `verdicts/p3-worker.md`.

I read the complete same-head package verdict. It matches the constructor repair, retained drain interfaces, complete gate, and evidence limits.
No source or gate change follows the completed inspection. Earlier F150–F154 closures remain applicable.
The adapted test keeps a fixed manual worker clock and lets Core's host clock supply its stop-grace kill.
That adaptation preserves the title-before-Exited and Stop-ends-Killed observations. No worker timer or extra test hook is added.

**F157 — LOW — CLOSED.** I read the complete corrected body at the unchanged head.
Its exact UTF-8 SHA256 is `c2a721310b4b92d683fbeeeb305fc95d7b49f14d0c4253e196594779de7db3a3`.
It separates imported v1 list changes from #318's own list-free diff. All ten conformance blobs equal imported v1.
The body preserves historical failures, measurements, reversals, and separate F28 and Mac IN-6 duties.
No later successful run supplies a diagnosis or native mutation closure for those separate duties.

The exact-head gate SHA256 remains `fbe1d79466d22b97ae660cb14529092a7aed629694fac82037105c47ccc7bfe4`.
All ten steps pass. All 21 catches match citations and normal PASS; four outcomes are unviable.
No final miss, timeout, or FLAKY remains. The completed gate accepts this imported member, not a future train head.
Both reviews have zero findings, including LOW findings. This is a post-CLEAN import delta; the earlier SETTLE remains preserved.
Applicable later train reviews, exact merging gate, final receipt, and equal-tree/remote merge verification remain due.
#318 is absent from train 7b. This verdict accepts no future composition, pin, A24 rule, pending removal, or completed merge.
I ran no execution job.

VERDICT: CLEAN (0 open, including LOW; import delta and exact-head evidence)


### Matching package acknowledgment

Package message `msg_plugin-w_1791837752_c37995` confirms that the reviewer read complete integration Round 4 and its inspection at `65fb0ea03d89d5306f80b7f45e3f8cd6a146df88`.
Both reviews match acd6ced0, the corrected body hash, all 21 catches, and the exact-head gate.
No new review round or final train receipt follows. Later train and merge requirements remain due.
#328's current package review and reported future scope remain separate from #318 and train 7b.


## Round 5: source fit on withdrawn train 9

- Reviewed train: `4d7c547a17577dc73daaa1cd89da7eed743ca8bf`.
- Tree: `4c1fae2dde6f2f7c6c3265f8fa64d6c8d9331ae0`.
- Train base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
- Frozen member: `acd6ced03f7306cbf250f25d668b239ef7d70b33`.
- Lead assignment: `msg_plugin-w_1791839691_b4a292`.
- Complete package Round 204: `eb24ef870124c718c007f76bfe87f2babae4ba38`, read in full.

All eight inserted own patches retain their reviewed added and deleted lines. The moved base changes their exit-watch interface.
**F159 — MEDIUM — OPEN.** At driver_edges.rs:630, the new tail test evaluates the receive result with is_ok.
The channel now carries Result<ExitStatus, ExitWatchFailed>. Ok(Err(failed)) therefore sets exited true without an observed payload exit.
The test then starts DrainPty and can pass its byte checks. Production instead maps that inner error to ExitWatchFailed and ends with failure.
I independently confirm the same typed-source path as the package reviewer. Compilation or a successful run does not reject that permitted error path.
The repair must require a successful watched exit and fail on the inner error. It must preserve the actual poll, deadline, tail/count checks, and cleanup.

**F160 — LOW — OPEN.** Package message `msg_plugin-w_1791840237_82cc4d` identifies the inherited DrainPty action comment at worker.rs:183–185.
I independently read it. It still promises a count bound that prevents a remaining writer from holding exit back.
The changed repeated drain has no count limit; its design note explicitly retains the continuous-writer contract gap.
Update that action comment to the repeated count/flush rule and retained-writer limit. No product logic change or new test is needed.
Earlier F150 comment closures covered main.rs and drain.rs, not this action comment. They retain their original scopes.

The lead removes #318 through `msg_plugin-w_1791839933_347904` and rebuilds train 9b without it.
These findings apply to the withdrawn train's #318 composition. Train 9b retains the earlier drain and contains no #318 insertion.
The lead's old-train compilation and strict results remain reports on 4d7c547a. No gate on that train is supplied or accepted.
P3's corrected member and applicable review remain due. No prepared repair, future train, Mac IN-6 diagnosis, or F28 closure is accepted.
All earlier review history and the F154 SETTLE remain preserved. I ran no execution job.

VERDICT: NOT CLEAN (2 open, including LOW; withdrawn train-9 #318 source fit)


### Matching withdrawn-train package supplement

I read the complete F160 supplement to package Round 204 at `0d1847317a23317f5af01acb767c782c474e54ba`.
It retains F159 MEDIUM and F160 LOW on withdrawn train 4d7c547a. Its source premises match integration Round 5.
Package message `msg_plugin-w_1791840496_174b44` confirms that the reviewer read complete integration Round 5 at `d76d87f3982c87a03a96bb15ace15f456532cc8a`.
No new review round or repair acceptance follows. #318 remains outside train 9b.


### Second import and repair inspection: package verdict pending

- Head: `836d3395702cfc7c662dde8f0e1fb790a9c8c2d4`.
- Tree: `b01738869303a3d6e5d4eae1d385b69134b714c0`.
- Imported v1: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
- Previous reviewed member: `acd6ced03f7306cbf250f25d668b239ef7d70b33`.
- Owner READY: `msg_plugin-w_1791840693_b80efe`.

Merge 12b2418a has the previous member and imported v1 as its parents. Its commit records no textual conflict and rerere off.
Seven previous own patches preserve every added and deleted line. The eighth changes only the F159 exit arm.
The ninth own path changes only the F160 action comment. The final own diff has nine files, 630 additions, and 144 deletions.
All ten conformance blobs equal imported v1. The changed import lists satisfy the three-way ID formula: held 4, ledger 692, pending 385.
No new pin, list removal, exception, public signature, or shared helper change follows from these two repairs.

F159's repair requires Ok(Ok(status)) before the tail test starts its drain. Ok(Err(failed)) fails the test immediately.
The outer receive error keeps the wait active. The driver retains the sender; the existing absolute deadline bounds that wait.
The actual poll event, Linux unread tail and count gap, Mac readiness reads, full byte equality, and cleanup remain unchanged.
Production still converts the watched inner error into Input::ExitWatchFailed. The test no longer treats that error as a successful exit.
F160's repair describes repeated counted reads and flushing reads, quiet-read or EIO/EOF termination, and no byte bound.
It retains the continuous-writer A24 gap. Earlier F150–F154 closures and the lead's F154 SETTLE retain their scopes.

I read the complete current PR body. Its UTF-8 SHA256 is `e98d05d204a37e86d0e9eb65d7b30471c45fec1179a1bc3f23b0c698af1ef7ca`.
The second-import section separates inherited list changes from the own diff and describes both repairs and evidence limits.
The owner reports the lead's ruling: no separate member gate; the train-10 gate covers this member.
No train-10 source or gate acceptance follows from this inspection. The complete same-head package verdict remains due before integration CLEAN.

Local evidence directory: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/p3-a31/`.
I read the recorded results without running their commands:

- `local-check-2bb84650.log`: SHA256 `37fd3a78cbb9a696fe72043809952f0e63cef89f54b35ce0c5966ff2e35dcc23`.
  It names the repair head and records removal of 8326 files, 2.3 GiB, after the worktree switch.
  It reports fmt, CI taint, xtask checks, lists, clippy, default 1127, worker slow 148, and strict 579 with exits zero.
  The owner's body identifies strict seeds 0–31. The record gives no independent history of other target writers.
- `f160-check.log`: SHA256 `6a2fd420a8d718b4f9a05b3f734fc1bad55ae8ee5d5c6c8dca3476ce96618512`.
  It reports fmt, doc, and clippy exits zero. The first taint command fails because NEXTEST_TEST_THREADS is absent.
  The capped taint command then passes. Both results remain evidence; the later pass does not erase the first failure.
- `workspace-check-836d3395.log`: SHA256 `9517385b079f3cdb040966f976dc3cba9c51ed32fb606bfa8b1c4fa5ab3aa3f7`.
  It names the current head and imported base. It reports cargo check --locked --workspace --tests exit zero, with warnings retained.

The doc log does not contain its own head header. The owner identifies its head and broken-intra-doc-link denial in the body.
These are local producer records. They do not replace the later exact train gate or establish a completed merge.
F28 and the earlier Mac IN-6 diagnosis remain separate duties. I ran no execution job.
No formal verdict or new NOT CLEAN round follows from this pending inspection.


## Round 6: second v1 import and repairs CLEAN in source

- Head: `836d3395702cfc7c662dde8f0e1fb790a9c8c2d4`.
- Tree: `b01738869303a3d6e5d4eae1d385b69134b714c0`.
- Imported base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
- Complete integration inspection: `49a87aa5`, above.
- Complete package Round 205: `b84ad95cf3021021bacda725f6b758a1c97ba38e`, `verdicts/p3-worker.md`.
- Package message: `msg_plugin-w_1791841002_4ac0c9`.

I read the complete same-head package verdict. Its source, tree, base, body hash, and evidence limits match the integration inspection.
No source or evidence change follows that inspection. The package verdict adds no execution result or product change.

**F159 — MEDIUM — CLOSED.** Only a successful watched exit ends the tail test's exit wait.
An inner watch error fails before DrainPty. The poll event, absolute deadline, count gap, byte equality, and cleanup remain unchanged.
**F160 — LOW — CLOSED.** The public action comment describes the repeated count/flush rule and quiet-read or EIO/EOF end conditions.
It states the lack of a byte bound and retains the continuous-writer A24 gap.
All earlier finding closures and the lead's F154 SETTLE retain their scopes. Neither review has an open finding, including LOW.

The complete body hash remains `e98d05d204a37e86d0e9eb65d7b30471c45fec1179a1bc3f23b0c698af1ef7ca`.
The local records retain their original heads and limits. The missing-cap taint failure remains recorded beside the later capped pass.
The doc record lacks an exact-head header and printed doc command. Its head and rustdoc flag remain the producer's report.
The local clean record supplies no numeric clean exit or independent target-writer history. No local record becomes a train gate.

This verdict accepts the second import and repaired member source. No separate member gate is supplied or required under the stated lead ruling.
The later train composition, applicable combined review, exact merging gate, ONE final train receipt, and merge verification remain due.
#318 remains outside train 9b. No future train, pin, pending removal, Mac IN-6 diagnosis, F28 closure, or A24 implementation is accepted.
I ran no execution job and changed no product code.

VERDICT: CLEAN (0 open, including LOW; second-import and repair source only)


### Matching package acknowledgment of Round 6

Package message `msg_plugin-w_1791841093_19f29f` confirms that the reviewer read complete Round 6 and its second-import inspection.
The reviewer names published commit `388dcdf63fd8a585d8162af3ce917c6128dfc8c1` and reports an equal remote review ref.
Both reviews match the exact head, tree, base, body hash, local hashes, finding closures, and source-only limits.
The status row now records Round 6 source CLEAN. The withdrawn-train NOT CLEAN history remains unchanged.
No new review round, separate gate, or final train receipt follows from this acknowledgment.


### Train-10 composition inspection: package delta pending

Lead assignment: `msg_plugin-w_1791842737_74b111`.
Train head: `644f632f194f62ad7fe14e66482ee01083706e30`.
Tree: `31f79709fd6d686a995da9f2d0506c59a9861b47`.
Train base: `3e4b3ab8172f1a04467a7969833708606a0f8580`.
Sole frozen member: `836d3395702cfc7c662dde8f0e1fb790a9c8c2d4`.
Unique merge base: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.

I read the merge parents and complete nine-path delta. It retains 630 additions and 144 deletions; the whitespace check passes.
Every own added/deleted line matches the accepted member delta. Eight member blobs are byte-identical to the frozen member.
Only DESIGN.md incorporates an additional base change: #319's route_frames_taken paragraph.
The repeated drain and retained A24 gap remain exact. The route paragraph remains exact to the accepted train base.
That paragraph preserves the default-queue, no-gate, and no-accept-limit premise, read-only query failure, and R-115 reviewer requirement.
The two design changes describe separate edges and do not change their accepted interfaces or scope.
All Rust member blobs retain their accepted source. F159 and F160 closures remain applicable to the composed source.

All ten conformance blobs equal the train base. The nine text-set merge checks pass; real proofs use complete lines.
No #328 removal returns. No pending, minimum, pin, exception, mutation rule, or CI change enters through #318.
The current package Round 205 accepts the member. I requested the complete applicable composition delta or explicit lead carry ruling.
No formal member-delta or combined CLEAN follows before that applicable package record is read.
The lead reports local checks running and plans a detached exact-head gate. Neither statement is completed execution evidence.
The exact merging gate, ONE final train receipt, and lead equal-tree/remote verification remain due.
I ran no execution job and changed no product code.
