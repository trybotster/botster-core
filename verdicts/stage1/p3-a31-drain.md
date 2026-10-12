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
