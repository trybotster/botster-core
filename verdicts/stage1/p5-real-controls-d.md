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

## Round 3: correction and exact-head gate acceptance

- Head: `5e6b15184bf4ab9d4bb1e572925b201409ea3d20`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Parent: `e2b11b172ed5b968f9444eddfe8b0465ece67a02`.
- Tree: `1740aeb6a3a6222531a510532721a6cbf772c156`.
- READY: `msg_plugin-w_1791721536_fca438`.
- Published package CLEAN: `7c5a20c82f8516fe757fda4d25e8781d0ac60eec`, verdicts/p5-real-controls-d.md.
- Package receipt: `msg_plugin-w_1791721803_fc26d8`.
- Risk: HIGH. Plan 23ar remains applicable.

Sol reads the complete package Round 3 addition and verifies publication on fetched stage1/review-p5.
Sol reads both complete own deltas, the affected crash proofs, complete snapshot source, current lock/storage paths, and complete PR body.
The body names this exact head and records both corrected proofs and supplied gate results.
The preceding Prior art checks remain applicable. These corrections reuse the snapshot, public harness controls, OwnedChild, and existing deadline helpers.
No dependency, exclusion, timeout, pin, transcript, or pending-list delta occurs in this correction.

### Findings and lock comparison limit

D-F6 is CLOSED. The unread-exit refusal proof now runs in its own nextest test process.
It retains Bad while the exit is unread and success after Core consumes the exit.
The count and wake proof starts only s1 and s2. No earlier ended worker can supply its retiring reaper or competing notification.
The proof retains the two live reapers, lost tap, stale socket, one reap notification, surviving worker, and fixed descriptor count.
The named production reaper and owned-thread join proofs remain unchanged. All previous findings remain CLOSED.

The lead's 09:15 and 09:20 clock entries in handoffs/core-lead.md require stat-only inspection of the top-level lock file.
Sol reads those instructions. They prepare for the separate LC-2 correction from flock to a POSIX record lock.
The current reviewed base still uses flock. This review accepts no future lock implementation.
The snapshot records a regular top-level lock file as Node::Lock with mode and length, without opening it.
It still records the root's kind and mode, does not follow links, and reads every other regular file by bytes.
The deliberate limit excludes lock contents. Current Core creates that file without truncation and stores its epoch under rows.
The inspected Core paths do not write bytes to the lock file. The lead explicitly authorizes this comparison limit.

The new proof holds a real record lock and uses an owned child to probe it from another process.
The child reports held before and after snapshot, then free after the holder closes the file.
The proof checks Node::Lock and byte comparisons for another top-level file and rows/lock.
OwnedChild owns and reaps each child. The helper returns without its specific environment variable.
Both OS logs name this proof as PASS. Linux catches both lock-name replacements through this proof.
Sol does not independently verify the body's local red-on-revert claim and runs no reversal.

### Import check

Merge e2b11b17 has parents 404c845d11ad79fa69a01e237a64e541a9bf7f0e and the exact reviewed base above.
Sol's independent static merge-tree result is 01d7f8aeaeec4cbc45d1e45ca678779050afeef6, exactly the recorded merge tree.
The import has no conflict. All thirteen imported paths retain exact base blobs.
The thirteen own paths outside real.rs and real/snapshot.rs retain exact Round 2 blobs.
The static diff check passes. The reviewed base is an ancestor of this head.
The retained prior source conclusions apply to the unchanged source within their recorded limits.

### Exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-5e6b1518-pool-20261011-050825-89519.log`.
SHA256: `43a27754d89d1ac9ff3bae3960fbf0c85c7b0cef52aad28273278beb7e42f4af`.

Sol reads the exact header, check results, relevant named proofs, tier summaries, baseline, every raw mutation identity, catch reports, and final exit.
All ten steps pass in 908.1 seconds. Jobq and the wrapper exit zero after 920 seconds.
Default reports 1644 PASS, 484 skipped, and 12.033 seconds. Slow reports 494 PASS, 1998 skipped, and 31.747 seconds.
Slow coverage checks 1028 slow-only tests. Both crash proofs, both reaper proofs, root-mode, two-directory, and lock proofs report PASS.
The real conformance selection reports 148 distinct PASS identities. AD-6 reports PASS in 0.157 seconds.
The gate reports testkit-passing 56/69, real-passing 41/68, real-accepted 42/69, and pending-real 58 with zero passed.

Stage 1 reports 122 distinct identities: 31 caught, 62 missed, 29 unviable, zero timeout, in 133.8 seconds.
The stage-2 baseline passes 494 tests, six slow, with 2662 skipped. Its complete step takes 38.9 seconds.
Stage 2 reports 62 distinct identities: 57 caught, five unviable, zero missed or timeout, in 631.2 seconds.
Sol independently parses all 184 raw records and matches all 62 stage-2 identities to the stage-1 miss multiset.
Sol independently matches all 31 stage-1 and 57 stage-2 catches to their named catch reports.
The final result is 88 caught and 34 unviable, with zero missed or timeout. The mutation step takes 807.1 seconds.
Fuzz passes because the diff changes no crate with a decoder harness.

### Exact-head Mac evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-5e6b1518-pool-20261011-052405-65143.log`.
SHA256: `1fafb463717485a0eb58b1b3e8b5ce0854400b6fe354fe6d1949b7a1ecbe951c`.

Sol reads the exact header, selections, clippy result, named proof results, mutation diff, baseline, both identities, and final exit.
Worker prebuild and clippy pass. Clippy covers all targets and features of testkit, test-process, and core-sys.
The focused suite passes 113 tests, with 484 skipped, in 2.665 seconds.
Both crash proofs, both reaper proofs, the lock proof, and retained corrected proofs report PASS.
The separate AD-6 selection passes one test, with 1442 skipped, in 2.351 seconds.
The in-diff Mac baseline passes. Both threads replacements, Ok(0) and Ok(1), are caught.
The result is two caught, zero missed, zero timeout, and zero unviable.
The platform:: selection retains the accepted Round 2 scope for the only new Mac function.
MAC_EXIT is zero. Jobq exits zero after 56 seconds. The wrapper exits zero after 57 seconds.

### Verdict scope

Zero integration findings remain open. This CLEAN covers this exact head, base, and supplied gate evidence.
Section 8 requires no repeat gate for an unchanged head with both applicable CLEANs.
The lead holds merge order for #243. This verdict authorizes no later base import or future LC-2 source.
A changed head must satisfy the applicable import and delta-review rules with its required exact-head gate.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 5e6b15184bf4ab9d4bb1e572925b201409ea3d20

## Import inspection: 15d682b2

Head: `15d682b2f45c5b11fc664078292d89198dcf6585`. Base: `ddd3b849df5db34d20d944ff6b8de23405fdb5c8`.
Parents: reviewed 5e6b1518 and exact base ddd3b849. Tree: `db7c9e981b6a32774dcc4a2d9fa0f845e0721d7d`.
Receipt: msg_plugin-w_1791723190_fa49ee. Package inspection agreement: msg_plugin-w_1791723281_158281.

Sol reads all four reported shared-path import deltas and the pending-list composition.
The independent permitted static merge-tree check returns the exact recorded tree without a conflict.
All fifteen own addition/deletion sequences match the previous reviewed source.
All 33 outside-own imported paths equal exact base blobs. Ten unchanged own paths equal 5e6b1518 blobs.
The remaining own-diff changes are context and location changes from the import, not changed own added/deleted lines.

The imported descriptor result is `(usize, Option<StreamEndpoint>)`. EdgeTap borrows the successful result to record sent bytes.
EdgeTap returns that result unchanged, including the retained endpoint. The fake edge returns None for the endpoint, as before the import's design.
The own public Tap::signals method and its assertion remain intact. No production branch depends on the testkit.
The reaper exclusion and process-check allowance retain the reviewed own reasons and retain the base's worker entries.
The pending list combines the base's three IN-10 removals with the reviewed own AD-6 removal. No other own removal appears.
The stat-only snapshot and its process-record-lock proof equal the previously reviewed source exactly.
The static diff check passes. No source finding exists so far.

This is an inspection record, not a formal imported-head CLEAN.
The canonical base-check log, exact-head gate, final READY/body, and formal package artifact remain pending.
The prior CLEAN covers only 5e6b1518. Sol runs no execution job.

## Second import inspection: 0fed25b3

Head: `0fed25b32a2db1c025ba04233ea3826555b1b26a`. Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
Parents: inspected 15d682b2 and exact base c9ae2c2f. Tree: `6550b4deaf8a5d3b0bdf9536c49164e53b76e874`.
Receipt: msg_plugin-w_1791723700_2efe93. The owner stops the 15d682b2 gate when v1 moves again.
That stopped gate establishes no completed acceptance for either head.

Sol reads the complete supplied canonical base-check log:
`~/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-0fed25b3-pool-20261011-060113-32411.log`.
SHA256: `eb8b02485f2e4fea085b28a0540b6a47e6e592eff14ac0fd9efd55f548901309`.
It names the exact original reviewed head, new head, and base. Conditions 1 and 4 PASS.
Conditions 2 and 3 FAIL on the same four shared paths. Job and wrapper exit one after eleven seconds.
The pending-list check combines one own removal with three base removals and retains the base's real-pending file.
The failed check requires a delta review; it supplies no automatic CLEAN transfer.

Sol independently verifies the new merge parents. The permitted static merge-tree check returns the exact tree without a conflict.
The delta from 15d682b2 contains only #265's five paths, all outside #257's own diff.
All fifteen own files equal the inspected 15d682b2 blobs. All original own added/deleted sequences remain identical to 5e6b1518.
All 37 outside-own imported paths equal exact base blobs. The four shared-path and pending-composition inspections remain applicable.
The imported lock source equals the reviewed #265 tree that now enters v1 as c9ae2c2f.
The snapshot retains exact blob `4a091397d5a58f71b534dcc1fc1ff15b36f2f693` from the reviewed stat-only correction.
It still opens no top-level lock file. Its process-record-lock proof retains the before/after-held and after-close-free checks.
The static diff check passes. No source finding exists so far, and the #265 snapshot condition is retained in this source.

This inspection supplies no formal imported-head CLEAN or gate approval.
Final READY/body, the exact 0fed25b3 gate, and the applicable package artifact remain pending.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Imported-head gate inspection: 0fed25b3

Head: `0fed25b32a2db1c025ba04233ea3826555b1b26a`. Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
READY receipt: msg_plugin-w_1791724664_65785b. The prior two import inspections supply the source review.
Sol reads the complete published body and its subsequent one-line correction at this unchanged head and base.

### Body correction

D-F7 LOW agrees with the package finding in receipt msg_plugin-w_1791724706_a27818.
The final gate bullet initially states that all imports change no own path. That statement conflicts with the four shared paths.
The owner corrects the body in receipt msg_plugin-w_1791724733_3b7f8b. Sol verifies the published correction.
The corrected body limits the unchanged-path statement to #265. It names #243's four shared paths and pending-list composition.
It preserves the reviewed own added/deleted sequences and the fifteen own blobs from 15d682b2 after the #265 import.
The prior Mac evidence covers unchanged own executable code. Imported changes retain their separately accepted base evidence.
The shared-path deltas have static review and the new Linux gate. D-F7 closes in the body.
The source head remains unchanged. The prior-art analysis and earlier findings retain their recorded scope and closure evidence.

### Exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-d-0fed25b3-pool-20261011-060128-33639.log`.
SHA256: `ac776a2e81ce317ba89894415efcbde3eb1a4a88eda24d20296c4d81e92ada37`.

Sol reads the exact head/base header, checks, tier summaries, relevant named proofs, baseline, all raw mutation identities, catch reports, and final exit.
All ten steps PASS in 928.2 seconds. Jobq exits zero after 941 seconds. The wrapper exits zero after 942 seconds.
Default reports 1688 PASS, 481 skipped, in 10.326 seconds. Slow reports 514 PASS, 2016 skipped, in 38.786 seconds.
Slow coverage checks 1045 slow-only tests. The slow selection contains 514 distinct PASS identities, including 151 real conformance identities.
Both crash proofs, both reaper proofs, row-file proofs, snapshot root-mode and lock proofs report PASS.
The reused-pid and two-directory proofs report PASS. AD-6 reports PASS in 0.164 seconds.
The gate reports testkit-passing 56/69, real-passing 41/68, and real-accepted 42/69.
The real-pending report retains 58 IDs, with 33 PASS under the ignored selection. This review accepts no additional pending-real removal.
The own pending removal remains AD-6; the imported pending composition remains the reviewed three IN-10 removals.

Stage 1 reports 122 distinct identities: 31 caught, 62 missed, 29 unviable, zero timeout, in 144.6 seconds.
The stage-2 baseline passes 514 tests, ten slow, with 2700 skipped, in 40.354 seconds. The complete baseline step takes 50.5 seconds.
Stage 2 reports 62 distinct identities: 57 caught, five unviable, zero missed or timeout, in 618.6 seconds.
Sol independently parses and reads all 184 raw records. The stage-2 identity multiset equals the stage-1 miss multiset.
Both catch multisets match all 31 stage-1 and 57 stage-2 named catch reports.
The final result is 88 caught and 34 unviable, with zero missed or timeout. The complete mutation step takes 816.0 seconds.
Fuzz passes because the diff changes no crate with a decoder harness.

No source or body finding remains open so far. The supplied exact-head gate passes.
This inspection grants no formal imported-head CLEAN. The same-head package artifact remains pending.
The stopped 15d682b2 gate supplies no acceptance. No new Mac execution evidence is claimed for 0fed25b3.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Round 4 complete: imported head 0fed25b3

Head: `0fed25b32a2db1c025ba04233ea3826555b1b26a`. Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
Package completion receipt: msg_plugin-w_1791724876_8421ae.
Package verdict: `626f9002377a30f2ab20f26b3c01d5326ca94a2d`, verdicts/p5-real-controls-d.md, Round 4.
Sol reads the complete 85-line package addition and verifies publication on origin/stage1/review-p5.
It reports CLEAN on this same head and base. Its source, body, gate, and retained Mac-evidence limits agree with the integration inspections.

The source inspections in ae7ca202 and 6a7e23f7 and gate inspection in 79ebecd4 remain applicable.
D-F7 LOW closes in the published body. Every earlier finding retains its recorded closure evidence.
Zero integration findings remain open. All ten exact-head Linux steps pass. Both reviewers accept the same import composition.
The lead-authorized stat-only lock snapshot remains intact. No additional pending-real removal is accepted.
The prior Mac log covers unchanged own executable code. This verdict claims no exact-head Mac run on 0fed25b3.
Section 8 requires no repeated full gate for this unchanged head with both applicable CLEANs.
This CLEAN covers only this exact head, base, corrected body, and supplied evidence. A later import remains outside this verdict.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 0fed25b32a2db1c025ba04233ea3826555b1b26a
