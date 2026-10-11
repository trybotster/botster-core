# Integration review: #234 real descriptor handoff, PR A

## Source and gate inspection before Round 1 — 2026-10-10

Inspected head: `bd800194a0ce3da217409ce334183589c5f0763d`.
PR, gate, and merge base: `e418bf5a8bcc7c18313e7d82775e8ae29368b366`.
Tier: HIGH. Scope: the complete 13-file delta, host dispatch, worker spawn thread, descriptor ownership, and supplied evidence.
Plan: 23ae, `b895ad493b9919dd63a9e11a596ba5fd210f1ea5`.
Verified pin SHA-256: `db0522ad7a0ca5f0ede3e7b236049f11be504161d6cfa9ee9c65638d46c4ac3b`.

I read every changed file, the host's descriptor contract and flush loop, and the current PR body.
The host completes earlier frame bytes before a descriptor mark.
It sends the descriptor at the AttachRoute frame start. After a positive send, the host drops its endpoint copy.
Blocked and interrupted sends retain the endpoint's original Rust type. Failed sends return the endpoint for host cleanup.
The helper limits a descriptor send to SO_SNDBUF. Its macOS EMSGSIZE decision follows the lead's option D.
Linux uses MSG_NOSIGNAL. macOS refuses sockets without SO_NOSIGPIPE. The real edge proof checks the accepted socket's inherited flag.
Received descriptors use OwnedFd. Normal error paths drop received descriptors. Linux uses MSG_CMSG_CLOEXEC; macOS sets FD_CLOEXEC before return.
The worker records its loop thread when Driver is made. Payload::spawn rejects another thread before the cwd check or fork.
The exit callback only sends its status and wakes the loop. It does not fork or exec.

The lead explicitly accepted the rustix 1.1.5 CTRUNC mitigation in the shared lead handoff.
The receive buffer is 4096 bytes, above the measured kernel descriptor limits.
A CTRUNC result bypasses the defective ancillary drain and reports an error. This path retains received descriptors until process exit.
The source and PR body state this exception. I do not treat that accepted exception as a new finding.
I read rustix's RecvAncillaryBuffer drop and AncillaryDrain::advance. The drain uses the message's cmsg_len to advance its buffer.

PR B still owns the production worker receive, descriptor/frame validation, and the payload descriptor-inheritance proof.
This inspection grants no acceptance for those unbuilt paths. PR A changes no pending ID, minimum list, or contracts pin.

### SI-1 / F108 LOW — New documentation retains retired framing and signal rules

Evidence: fd_pass.rs:106–107 says Received descriptors came with the first bytes of the read.
On Linux, the read can contain earlier plain bytes before the descriptor segment. Plan 23ae replaced the first-byte rule.
fd_pass/tests.rs:250–251 and PR condition 6 describe the no_sigpipe proof as a default-disposition test.
The actual target installs a recording handler, as plan 23ac requires.
The new host DESIGN.md A49 row cites plan stage1-plan.77739f96, whose proof predates that correction.

Required change: describe descriptors through the read's descriptor segment and frame start.
Describe the signal proof through its recording handler and control write. Cite the plan revision that requires that proof.
Status: OPEN. The package reviewer records the same documentation finding as F108 LOW.

### SI-2 / F109 LOW — The full-socket proof busy-polls after a successful send

Evidence: fd_pass/tests.rs:172–177 retries recv_with_fds immediately on WouldBlock, with no event or deadline.
BUILD testing rule 5 forbids polling in tests.
The preceding synchronous send has already written the small message. The test can read it once or wait on actual readiness.

Required change: remove the busy-poll loop. Use one immediate read or a readiness wait with a derived deadline.
Status: OPEN. The package reviewer records the same finding as F109 LOW.

### SI-3 / F110 MEDIUM — The test receiver loses descriptors across frame fragments

Evidence: slow_facade_worker.rs:71 resets the descriptor count for every read.
Line 79 chooses that count when AttachRoute completes, rather than retaining the count from the read that held its start.
If a descriptor-bearing read contains only an AttachRoute prefix, the next read can complete that frame with no descriptors.
The receiver then reports zero for a valid handoff. A later descriptor segment can also supply the wrong frame's count.
Plan 23ae requires association at frame start across partial frames.
This defect affects PR A's new test receiver. It does not assert a defect in PR B's unbuilt production receiver.

Required change: retain the frame's descriptor association across reads until completion.
Prove a descriptor-bearing prefix followed by plain frame bytes with a deterministic small-read socket proof.
Status: OPEN. The package reviewer records the same finding as F110 MEDIUM.

### SI-4 / F111 MEDIUM — The new descriptor module lacks HIGH-path coverage

Evidence: ci/high-tier-paths.txt names individual risky files and has no fd_pass.rs entry or matching glob.
The new module implements descriptor transfer, close-on-exec, truncation handling, and signal suppression.
BUILD risk rule 5 requires coverage of descriptor handoff. A later isolated change to this file lacks that coverage.

Required change: add fd_pass.rs to the HIGH-path list with its descriptor-handoff and signal rationale.
Status: OPEN. The package reviewer records the same finding as F111 MEDIUM.

### SI-5 LOW — The supplied PR body lacks its required Prior art note

Evidence: the supplied #234 body describes rustix and the new implementation but has no Prior art note.
pair-common.md requires every PR to state prior code considered, reuse or rejection, and the reason for custom code.

Required change: add that note for this PR. Identify rustix reuse and the reason for the custom host and worker glue.
Status: OPEN. I asked the implementer to add the note. The package reviewer also requested it.

## Supplied evidence

Linux gate: `~/botster-sessions/gates/botster-core-stage1-p4a-real-handoff-send-bd800194-pool-20261010-175432-24012.log`.
The header names this exact head and base. The command is plain cargo xtask ci through the Linux pool.
All ten jobs pass. Default: 1524 tests passed in 12.965 seconds. Slow: 396 tests passed in 139.442 seconds.
The descriptor, framing, loop-thread, signal, real-edge, and facade proofs pass.
The mutation job reports 17 mutants: nine caught, eight unviable, zero missed, and zero timeout, in 94.6 seconds.
Its skip list is empty. The main job starts no stage 2 because stage 1 missed no mutant.
Both conformance reports show 193 passed and zero failed. Strict coverage remains 386/407, with 21 real-only and two held.
Minimum counts remain testkit 50/69, real 31/68, and real-accepted 31/69. Real-pending remains 81, with zero newly passing IDs.
The job and wrapper exit zero after 348 seconds on gaming, with zero seconds queued.
git diff --check passes. The Git merge base equals the recorded base.

Mac evidence: `~/botster-sessions/shared/core-stage1/evidence/p3-handoff-send/mac-run-bd800194.log`.
Its header names this head, macOS 27.0.1, and arm64.
The log passes 40 core-sys tests, the isolated no_sigpipe target, 20 core real-edge tests, 17 facade tests, and 25 real-core tests.
The worker slow targets also pass. The two macOS-only descriptor tests and both kernel framing proofs pass.
The signal target records 100 rounds: every control raises SIGPIPE, and no descriptor send raises it.
I checked libtest-mimic 0.8.2 source: test_threads = 1 runs trials on the calling main thread.

I read the three supplied revert records in evidence/p3-handoff-send.
no-sigpipe-revert-macos.log fails at round zero when suppression is removed and passes when restored.
facade-handoff-revert-macos.log fails with (RouteId(1), 0) when the host sends plain bytes without the descriptor.
emsgsize-macos-only-reverts-macos.log rejects the >, <, and false-guard replacements through the retained macOS tests.
These are supplied records. I ran no reversal or mutation job.

This inspection completes no review round. The same-head package verdict remains pending.
I sent the confirmed findings to P3 and its package reviewer.
I changed no product code. I ran no build, test, gate, mutation job, merge-check job, or reversal job.

## Round 1 — 2026-10-10

Reviewed head: `bd800194a0ce3da217409ce334183589c5f0763d`.
PR, gate, and merge base: `e418bf5a8bcc7c18313e7d82775e8ae29368b366`.
Tier: HIGH. Scope: the complete 13-file delta, package interfaces, descriptor ownership, and exact-head Linux and Mac evidence.
The source and evidence inspection above remains applicable. This is the first integration round on #234.

I read the full same-head package verdict at `759b27f1061cdddb2aed20b31e59b9daaaee41fd`.
File: `verdicts/p3-worker.md`, Round 153, package round 1 for #234.
Both reviews confirm the same five open findings:

- SI-1 / F108 LOW: new documentation retains retired framing and signal rules.
- SI-2 / F109 LOW: the full-socket proof busy-polls after a successful send.
- SI-3 / F110 MEDIUM: the test receiver loses descriptor association across frame fragments.
- SI-4 / F111 MEDIUM: the new descriptor module lacks HIGH-path coverage.
- SI-5 / F112 LOW: the required feature Prior art note is missing.

The gate passes all ten jobs. It passes 1524 default tests, 396 slow tests, and nine viable mutants, with eight unviable.
The focused Mac evidence covers the required macOS paths and signal proof. Those passing results do not close the five findings.
The current facade read size does not prove descriptor association across fragmented frames.

The host send and endpoint ownership follow the reviewed DP-2 split.
The lead's documented rustix CTRUNC exception remains accepted with its stated leak-until-exit limit.
PR B still owns the production receive path and end-to-end payload descriptor exclusion proof.
This verdict grants no acceptance for PR B or for a completed real route.
No pending ID, minimum list, or contracts pin changes.

The required corrections remain in the source inspection above. Every finding, LOW included, must close before CLEAN.
No round-limit decision is due. I changed no product code and ran no execution job.

VERDICT: NOT CLEAN (5 open).

## Round 2 — replacement at eded8086 — 2026-10-10

Exact head: `eded8086c6cf1a7e94b9577029a0229abb854484`.
Exact base: `e418bf5a8bcc7c18313e7d82775e8ae29368b366`.
Previous head: `bd800194a0ce3da217409ce334183589c5f0763d`.
Previous integration verdict: `9ce578168884e1f106c2c556f9b74979ad976ec5`.
Same-head package CLEAN: `381a695c617280ce79aafb741e6c9bd7a94e9aaf`, package round 154, `verdicts/p3-worker.md`.
I read that complete round, the five-path replacement delta, the current PR body, and the supplied evidence.
The remote head and v1 base still match at review completion.

### Closure evidence

SI-1 / F108 LOW is CLOSED.
Received describes the measured segment position on each operating system and associates descriptors with the AttachRoute frame start.
It no longer associates them with the first byte of every read.
The gone-reader comment, A49 design row, and PR condition 6 describe the recording handler and raw control write.
The design cites revision 23ac at the current b895ad49 plan pin.

SI-2 / F109 LOW is CLOSED.
The full-socket proof reads once after the successful synchronous descriptor send. The WouldBlock polling loop is gone.
The named proof passes on both operating systems at this head.

SI-3 / F110 MEDIUM is CLOSED.
The stand-in records the descriptor count when its empty FrameDecoder takes the first bytes of a frame.
It retains that count until completion, across later reads.
FrameDecoder::push takes no bytes beyond one complete frame. next_frame empties the decoder after completion.
The facade proof limits reads to three bytes, below the five-byte header size.
It checks that AttachRoute spans multiple reads and that the exact route has one descriptor.
The assertion observes the socket result. It does not expose private product state.
The existing marked thirty-second receive deadline remains unchanged.
The supplied completing-read reversal fails with `(RouteId(1), 0)` instead of `(RouteId(1), 1)`.
The corrected facade proof passes on Linux and macOS.
PR B still owns the production receiver and invalid-framing proofs.

SI-4 / F111 MEDIUM is CLOSED.
ci/high-tier-paths.txt lists fd_pass.rs and fd_pass/** with descriptor-handoff and signal reasons under BUILD rule 5.

SI-5 / F112 LOW is CLOSED.
The PR body contains the feature's Prior art note under BUILD Starting fresh rule 0.
It names the vault lessons, pinned old source 72b2e33, rebuild row 5.4, parity audits, and library decisions.
It records the retained signal and close-on-exec lessons, rejected raw calls, and reasons for the Core-specific rustix glue.
The package review checks the named sources and closes the same finding.

### Evidence and retained scope

Gate: `~/botster-sessions/gates/botster-core-stage1-p4a-real-handoff-send-eded8086-pool-20261010-182012-72346.log`.
Its header names this exact head and base. All ten CI jobs pass.
Default: 1524 tests pass in 13.247 seconds. Slow: 396 tests pass in 139.516 seconds.
Both testkit reports give 193 passed, zero failed, and minimum 50/69.
Strict pending runs 386 of 407 IDs, with 21 real-only IDs not run and two held.
Real minimum remains 31/68 passing and 31/69 accepted. Pending-real remains 81, with zero passing IDs in that list.
Stage 1 tests seventeen mutants: nine caught, eight unviable, zero missed, and zero timeouts, in 97.7 seconds.
Stage 2 does not run because stage 1 misses none.
CI takes 344.5 seconds. The job exits zero after 355 seconds, with zero queue time. The wrapper exits zero after 356 seconds.

I read `shared/core-stage1/evidence/p3-handoff-send/mac-run-eded8086.log` and `facade-frame-start-revert-macos.log`.
The focused Mac header names this head and macOS 27.0.1 arm64.
Forty core-sys tests pass. The separate signal proof records one hundred successful control and protected-send rounds.
Core slow selections report twenty library, seventeen facade, and twenty-five real-core tests passing.
Worker slow summaries report fifty-seven, seventeen, and thirty-four passing tests, with filtered subprocess results also retained.
This is the supplied focused run, not a full macOS gate.
The frame-start reversal reports zero passed, one failed, and sixteen filtered tests in 0.17 seconds.

No descriptor-send or payload-spawn product logic changes after the previous reviewed head.
The remaining own paths retain Round 1's endpoint ownership, partial-send, signal, and loop-thread review.
No source merge, contract pin, pending list, public facade, or mutation exclusion pattern changes in this replacement.
The lead's rustix 1.1.5 CTRUNC exception remains: the buffer is not parsed, and received descriptors remain open until process exit.
This verdict does not claim descriptor closure for that accepted defence branch.
PR B must prove ordinary mismatch closure and the end-to-end payload descriptor exclusion on both operating systems.
This verdict accepts no completed real route or minimum-ID flip.
I ran no build, test, gate, mutation, measurement, reversal, or executable merge-check job.

VERDICT: CLEAN (0 open).
