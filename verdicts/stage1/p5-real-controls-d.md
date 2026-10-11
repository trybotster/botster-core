# Integration review: PR #257

## Round 1: R-55 real controls

- Head: `96f6f387bc9924a6ac436cc4820b2e166203d572`.
- Base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
- Sole parent: `dfaddba357e75812966c5e81e4f13d100cb82dd7`.
- Tree: `a17862601497eb6cc41131ebf449c4b87890fddd`.
- READY: `msg_plugin-w_1791714590_f992b6`.
- Package NOT CLEAN: `8c41476ce8d66a403132cc8072ed8eca0798f8d6`, verdicts/p5-real-controls-d.md.
- Package receipt: `msg_plugin-w_1791715104_a31d3c`.
- Risk: HIGH. The shared testkit and process-test crate change.

Sol read the complete twelve-file delta, surrounding control composition, complete package verdict, R-55, and retained evidence.
The delta adds 1169 lines and deletes 21 lines.
Four findings remain open: three package findings and one integration finding.

### D-SI-1 MEDIUM: the bystander guard omits other directories' recorded identities

Status: OPEN.
R-55 (2) condition 4 requires that the bystander's pid is in no other recorded identity.
At real.rs:818, `reuse_pid` collects worker and payload pids from `self.rows[&data_dir]` only.
Historical identities in another harness directory are therefore absent from the rejected-pid set.
Kernel uniqueness of live pids does not exclude historical recorded pids.
No ruling limits this condition to the rewritten directory.
The package reviewer agrees in `msg_plugin-w_1791715029_804605` and records this integration dependency.

Required correction: collect recorded worker and payload pids from every harness directory before accepting the bystander.
Keep the own-group and different-start-time checks. Keep cleanup of every rejected candidate.
No pid-wrap campaign is required to check the static guard scope.
Sol sent the finding to P5 in `msg_plugin-w_1791715097_573ec5`.

### Package findings retained

- D-F1 MEDIUM is OPEN. `end_with_reaper` polls the thread count with `yield_now`. A cleanup deadline does not permit polling under BUILD.md Testing rule 5.
- D-F2 MEDIUM is OPEN. The new macOS `threads` function has no supplied Mac mutation run. Linux does not compile that function. Tests and clippy do not replace the required focused mutation evidence.
- D-F3 MEDIUM is OPEN. The directory snapshot records descendants but omits the root's kind and permission bits. A root permission change during Drop can bypass the only-socket-change guard.

Sol independently checked each relevant source path and supplied evidence. The package verdict specifies the corrections.
D-F4 LOW is CLOSED as a body correction: there are five snapshot/row-file tests, three plus two, not seven.
Sol checked the updated body. That body reports later head 12691cae; this round does not review or accept its source.

Sol read the complete accepted steward draft:
`~/botster-sessions/shared/steward/drafts/r55-cond2-b.md`.
The draft replaces thread-exit polling with counts at the crash, one observed post-crash reap, and a named production reaper function.
A test owns and joins a thread that runs that function. The spawn wrapper only detaches it.
The retained wake can distinguish only the first reap after the crash; the draft explicitly requires one worker's reap and teardown of the other.
The draft does not change this reviewed head. Corrected source, the final contract record, and replacement evidence remain pending.

### Cross-package source checks

`Crashable` forwards the complete CoreApi to the real HostDriver. The RefusalLayer remains outside the slot.
The crash handle holds the slot weakly. An ordinary driver-handle drop still drops Core and releases its directory lock.
A crash takes Core once and drops it outside the slot lock. No production test branch or alternate machine appears.
The real controls are available only on the tapped harness. The plain harness rejects them.
`host_crash` checks quiet without pumping, releases its temporary tap reference, drops Core, compares storage, and restores a stale socket.
Its current snapshot checks descendant kinds, modes, file bytes, and link targets. D-F3 covers the missing root metadata.
The proof observes tap destruction, live workers, and a refused connection to the restored socket. D-F1 covers its invalid polling step.

`reuse_pid` requires one unheld directory and a row decoded with Core's decoder.
It verifies the recorded worker identity before KILL and waits for its real end.
The bystander is an owned FIFO child in its own group. Each rejected candidate ends before the next candidate starts.
The row uses Core's serde_json encoding and decoder. Restoring the old worker field before comparison checks every other field.
Replacement uses a new mode-0600 file, file sync, rename, and directory sync. It takes no Core lock and raises no epoch.
D-SI-1 covers the incomplete recorded-pid set.
The harness retains the bystander until teardown. Field order removes bystanders before the FIFO and root.

For a reused-pid session, `signals_received` returns every signalling call through the new handle's tap.
HostEdges has one signalling method, `signal_group`; the tap records each call before it forwards it.
The harness's external worker kill is not a Core edge call. Other sessions retain the stand-in observer rule.
The public adoption proof keeps the bystander alive, loses only the reused session, retains the other running session, and observes no Core signal.

### Conformance and prior art

Only `conf::ad_6_reused_pid_is_never_killed` leaves core-pending. It is already real-only with named proof `slow:real-process-identity`.
The unchanged #241 runner skips its testkit trial and requires its real trial. The Linux and separate Mac logs report PASS.
The real pending file, pin, transcripts, ledger, exclusions, decoder set, and timeout values do not change.
This round approves no pending-file removal or accepted model until all findings close.
The raw minimum report still contains unfenced AD-7. The R-59 acceptance limit remains.

Sol read OLD CORE at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its `thread_count` uses unsafe macOS proc_pidinfo; its descriptor cleanup lists the OS descriptor directory.
Its stale-socket test binds and drops a listener. Its real-worker kill proof cannot model a host process in this in-process harness.
The PR reuses those observation ideas, not their unsafe implementations.
It reuses current Core row decoding, process identity, group signalling, guards, owned children, FIFO blocking, and process-exit waits.
No new dependency appears. The new slot supplies control over the already-owned CoreApi box; it adds no alternate Core implementation.

### Retained evidence and limits

Linux log:
`~/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-96f6f387-pool-20261011-031010-57472.log`.
Its header identifies the exact reviewed head and base. All ten steps pass, total 926.4 seconds.
The default tier passes 1620 tests in 14.684 seconds. The slow tier passes 472 tests in 36.449 seconds.
Both new control proofs, all five snapshot/row-file tests, and the AD-6 real trial pass.
Stage 1 reports 111 mutants: 31 caught, 52 missed, 28 unviable, zero timeout, in 181.4 seconds.
The unmutated slow baseline passes 472 tests in 35.268 seconds; its complete step takes 50.3 seconds.
Stage 2 tests exactly the 52 missed full identities, once each: 48 caught and four unviable, zero missed or timeout, in 571.5 seconds.
Sol independently parses all 163 outcome records and compares the full stage identity multisets. They are equal.
The final result is 79 caught and 32 unviable. The complete mutation step takes 806.0 seconds.
Fuzz starts no trial because no changed crate has a decoder harness.
The job and wrapper exit 0 after 939 seconds; the job queues one second and runs 938 seconds.

Mac focused log:
`~/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-96f6f387-pool-20261011-032609-73248.log`.
Its header identifies the exact head and base. Worker prebuild and clippy pass.
The focused suite reports 104 passes and one failure in 5.038 seconds. Both new control proofs and the count helper pass.
The unchanged zombie assertion fails with Absent instead of Matches. This is not a green Mac suite.
The failed command stops the chained AD-6 command. The job exits 100 after 126 seconds; the wrapper exits 100 after 127 seconds.
The separate exact-head AD-6 log reports one pass in 5.293 seconds, with test time 5.287 seconds:
`~/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-96f6f387-pool-20261011-032831-86517.log`.
Its job and wrapper exit 0 after 49 seconds; the job queues one second and runs 48 seconds.
Neither Mac log contains the required mutation run. The separate zombie fix #258 remains its own review.

Current v1 is d86bfcc7 after #256. A replacement head needs the current base, applicable source review, and its own full gate.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (4 open) at 96f6f387bc9924a6ac436cc4820b2e166203d572

## Round 2: correction review at 29b769b1

- Head: `29b769b1e1549f8aaf7d3055c372e2cefa6592b3`.
- Base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Parent: `2646d13f71845f34a0b526db1239d511174c29ff`.
- Tree: `4f1072dcb2d6ad75888dc95943e35cd6e69a0a49`.
- READY: `msg_plugin-w_1791719298_3ee4b3`; documentation correction: `msg_plugin-w_1791719601_037eaa`.
- Package NOT CLEAN: `c06769f8fbe05ce5bbd4494984b00dcd1f3b9f62`, verdicts/p5-real-controls-d.md.
- Package receipt: `msg_plugin-w_1791719833_eca232`.
- Risk: HIGH. Plan 23ar remains applicable.

Sol reads the complete package addition and verifies its publication on fetched stage1/review-p5.
Sol reads all corrective source deltas and the affected complete source sections.
Sol compares the ordered added/deleted line sequences for all own paths with Round 1.
The unchanged paths match exactly. The six changed paths contain the inspected corrections and documentation changes.
All nineteen imported-only paths retain the exact base blobs. Three shared paths retain the inspected own changes and imported source.
The current PR body names later head e2b11b17. Its later correction and import remain outside this round.

### Closed findings

- D-SI-1 is CLOSED in source. recorded_pids reads worker and payload pids from every harness directory.
  reuse_pid supplies that full set to the unchanged own-group and different-start-time guard.
  The two-directory real-process test checks both workers. The supplied Linux report names all three vector replacements as caught.
- D-F1 is CLOSED in source. The count-polling loop is removed. The named production reap function waits, queues, notifies, and returns.
  The spawn wrapper only detaches that function. The two owned-thread tests join it and check the queue and notifier outputs.
  Sol reads the complete published R-55 follow-up at contracts d13a16511582a91642d692e7b959d52415d342db.
  The crash proof uses the accepted counts and one-wake form. D-F6 below concerns its separate earlier-reaper race.
- D-F2 is CLOSED. Sol reads the Mac header, focused selection, mutation diff, baseline, both outcomes, and final exit.
  Both replacements of platform/macos.rs::threads by Ok(0) and Ok(1) are caught.
  The selection runs platform:: tests, including the test of the new count function. The unmutated baseline passes.
- D-F3 is CLOSED in source. The snapshot records the root's kind and mode at the empty path.
  The new test changes the root mode and checks rejection. Supplied Linux and Mac results name that test as PASS.
- D-F4 remains CLOSED. The body retains the five-test count for the old head and describes the later root-mode test separately.
- Package D-F5 is CLOSED. The documentation-only child moves the bystander comments back above bystander.
  The body names the imported slow_driver_observer fixture correctly.

The own exclusion correction changes Children::spawn to reap for the same wait-error fallback and unchanged reason.
The process-check entry permits the owned reaper test's plain Child because the tested function must own its reap.
The count test removes its after-join count assertion. It retains the live-thread count and both descriptor counts.
The Prior art conclusions from Round 1 remain applicable. These corrections reuse the existing helpers and add no dependency.

### D-F6 MEDIUM: an earlier reaper can remain in the crash count

Status: OPEN. This agrees with the package finding.
The test ends s0 before the crash proof for two live workers.
Production reap publishes s0's exit before notify and return. Core can consume that exit while the reaper still runs.
The pump supplies no event for the kernel's removal of that thread from its count.
The exact before+2 assertion can therefore include the retiring s0 reaper and fail.
The same setup leaves s0 as a competing notifier. This review claims no demonstrated false PASS of the wake assertion.

Separate the unread-exit refusal proof into its own nextest test process.
The crash-count proof must start only the two live workers and have no earlier ended reaper.
Preserve R-55 condition 2 items 1 through 4. Do not add a count-polling loop.
P5 reports correction 404c845d and current-base import e2b11b17 in msg_plugin-w_1791719784_e3edb8.
P5 calls this D-F5; the formal package finding is D-F6. Sol awaits their exact delta READY and retained evidence.

### Evidence limits

Sol reads relevant named results from the supplied 2646d13f Linux and Mac logs.
The Mac focused suite reports 110 passes. Its separate AD-6 selection passes. Its two thread-count mutants are caught.
The Mac job and wrapper exit zero. These results precede the documentation-only child and the later executable correction.
Sol has not completed acceptance review of the full Linux gate in this round.
The package verdict reports its green result, but that report does not replace Sol's independent gate review.
This round accepts no final gate, merge, pending removal, or later changed head.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (1 open) at 29b769b1e1549f8aaf7d3055c372e2cefa6592b3
