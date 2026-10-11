# Integration inspection: PR #263

## Source inspection active

- Head: `e2c0f1dc69b5c4e81a01ead579ae85dd731c66da`.
- Base and sole parent: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Tree: `d95c685ac29ff55947ec6f23d6b72245b634feaf`.
- Request: `msg_plugin-w_1791719986_c8ccff`.
- Corrected identity: `msg_plugin-w_1791720030_939b55`.
- Branch: stage1/p5-await-end-zombie. Risk: HIGH. Plan 23ar applies.

The first supplied full SHA was wrong. Sol uses only the corrected identity above.
Sol reads the complete three-file diff, complete PR body, affected Mac wait function, and its relevant callers.
The diff adds 49 lines and deletes three lines. No formal integration verdict is issued yet.

### Source behavior

After an ESRCH refusal of the Mac exit watch, ended_before_the_watch calls rustix::process::test_kill_process.
Success or EPERM returns Exited. ESRCH returns Gone. Another error remains an error.
Sol checks the exact locked rustix 1.1.5 source documentation: this probe sends signal zero and sends no signal.
The probe does not reap a process. The change adds no wait loop, timer, dependency, selector, or exclusion.
Normal watch setup and event waiting remain unchanged. The enum documentation states the zombie and reaped meanings.

The source states the retained PID reuse limit between refusal and probe.
The answer can classify the ended old process as Exited when a new process owns its former pid.
Both answers mean the old process ended. The helper supplies no status or verified identity of the new process.
The existing await_end identity limit remains explicit. This inspection establishes no stronger identity guarantee.

The new proof waits for the owned child's exit status without a reap, then confirms its retained status through peek.
It requires Exited while the child is unreaped. After OwnedChild::status reaps that child, it requires Gone.
Sol checks peek's EXITED, NOWAIT, and NOHANG options, await_status's event wait, and the non-group child's reap path.
The unchanged running-child proof still requires Exited on both OSes. No platform expectation is weakened.

Sol checks the unchanged Linux pidfd adapter, OwnedChild::exited_within, group cleanup, and the shared rounds decision.
The callers include group members and workers that another thread reaps. The new probe imposes no own-child restriction.
Group cleanup still lists live members and treats repeated Gone results according to its existing rule.

### Prior art

Sol reads OLD CORE at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its fixture_gate.rs wait_exit watches EVFILT_PROC/NOTE_EXIT and maps ESRCH to the single boolean ended result.
That helper uses unsafe libc calls and has no zombie-versus-reaped result to reuse.
This change reuses the current watcher, ESRCH helper, and rustix probe. It adds no parallel process observer.

### Supplied failed gate and remaining evidence

Log: `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-e2c0f1dc-pool-20261011-045219-1693.log`.
Sol reads its exact head/base header, named slow failure, final summary, and failed exit record.
The first seven steps pass. Slow reports 479 passes and one EV-5 failure among 480 tests.
EV-5 seed zero, step 21 reports PendingLimit with pending_ops equal to four.
Mutants and fuzz report NOT RUN. The wrapper exits one after 386 seconds. This is no green gate.
The diff changes no Linux adapter. This inspection does not establish an independent causal attribution of EV-5's failure.

The owner reports that the lead assigns the #243 base import and one final gate before CLEAN.
The Mac pool record and formal package verdict remain pending. Local passes and reversal claims are not independently read here.
The final exact-head gate, its mutation evidence, the Mac evidence, and the applicable base delta still need review.
No integration source finding appears in this initial inspection. That statement supplies no formal source or gate CLEAN.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Mac correction inspection at ba7dded6

- Head: `ba7dded6cef46ebc5c04bca0de52876c8b1032ba`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Parent: `e2c0f1dc69b5c4e81a01ead579ae85dd731c66da`.
- Tree: `7a1d38b418af10f90f1c8038a5fb109a3b74fa51`.
- Owner receipt: `msg_plugin-w_1791721912_03e19a`.

Sol reads the complete three-file delta, affected proof, existing status wait, and complete updated PR body.
The delta adds fourteen lines and deletes seven lines. The runtime code remains unchanged from the inspected parent.
The running-child proof now requires an available status through await_status before it checks the zombie state.
That helper observes the owned unreaped child. peek retains EXITED, NOWAIT, and NOHANG; it does not reap.
The existing deadline bounds that event wait. Every previous assertion and expected value remains present.
The documentation now includes an exiting process and states that Exited does not guarantee an available Mac status.
The existing await_status documentation attributes that ordering to XNU. Sol does not independently verify the kernel source here.
The correction reuses the current event wait. It adds no retry campaign, timeout change, dependency, selector, or exclusion.
The preceding Prior art checks and PID reuse limit remain applicable. The static diff check passes.

### Supplied Mac records

Parent log: `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-e2c0f1dc-pool-20261011-052558-68175.log`.
SHA256: `c8ad51aa2fd6fb781917e8da7a6eca0ba9580817f453f15175226b29b87750da`.
Sol reads the exact header, selections, named failure, summaries, mutation identities, and final exit.
Prebuild and clippy pass. The full selected crate suite passes 84 tests with zero skipped in 10.859 seconds.
Nine of ten repeated two-test selections pass. One fails at slow_process.rs:214, the unreaped-zombie peek assertion after Exited.
The new zombie-versus-reaped proof passes in all ten selections. The job and wrapper exit one after 74 seconds.
This failed record stays failed. Sol does not independently verify the separate local 5/50 reversal claim.

Replacement log: `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-ba7dded6-pool-20261011-053027-93574.log`.
SHA256: `bca5890d925d53398a0afdfb87021c721338f1378a80a4a2a12376975ee5676f`.
Sol reads the exact header, selections, named proof results, summaries, mutation baseline and identities, and final exit.
Prebuild and clippy pass. The crate suite passes 84 tests with zero skipped in 10.898 seconds.
Both affected proofs report PASS by exact name. All twenty repeated two-test selections report two PASS each.
Sol requests and runs no repeat. Those supplied selected results replace no full Linux gate.
The mutation selection excludes only the named prebuilt-anchor test, because its binary is absent from the mutation copy.
The unmutated baseline passes. The gone(&error)-to-false guard replacement is caught at macos.rs:47:23.
The two Ok(Default::default()) replacements for await_end and ended_before_the_watch are reported unviable.
The result is one caught, two unviable, zero missed, and zero timeout. MAC_EXIT is zero.
Jobq and the wrapper exit zero after 61 seconds. No individual named mutation catcher is printed.

No integration source finding appears in this correction inspection. No formal source or gate CLEAN is issued.
The formal package artifact, #243 base import, and completed exact-head Linux gate remain pending under the lead's recorded sequence.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Import and gate inspection at a5406877

Head: `a540687728a7be9718e9bb11de62aa042ed905b6`. Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
READY receipt: msg_plugin-w_1791724872_d3eb21. Plan 23ar remains the approved plan.
Sol reads the complete final body and complete three-file own diff. The preceding source and Mac correction inspections remain applicable.

### Import composition

The first import is `16e51509064f704c341f5b0df8eebe2e898eef7a`, with parents ba7dded6 and ddd3b849.
Its tree is `3ffba78343fd9a43e8d46d0ff5571e2342e2b9cb`.
The second import is a5406877, with parents 16e51509 and exact base c9ae2c2f.
Its tree is `e37efd002da07c818c16e43ae71a54e2e055ffde`.
Both independent permitted static merge-tree checks return these exact trees without a conflict.

All three own blobs equal the inspected ba7dded6 blobs. The imports change 42 paths, with no own-path overlap.
All 42 imported blobs equal exact base blobs. The normalized own diff, excluding only index lines, equals the previously inspected diff.
It contains 5941 bytes, with SHA256 `87e0bb32938391e426fc5811ad14aa6751a3034112a4b94edc84874c532565e1`.
The base is an ancestor of this head. The static diff check passes.
The accepted #243 and #265 base reviews cover those imports. This inspection adds no new independent EV-5 cause claim.
The prior red e2c0f1dc gate stays failed. The new source import and exact-head gate supply the lead-directed replacement evidence.

### Exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-a5406877-pool-20261011-061730-64021.log`.
SHA256: `6ba1d8ac45570f7604a0636da911e657d8f8264f56ebb04ed37c541ef91ee6ea`.

Sol reads the exact head/base header, checks, tier summaries, relevant named proofs, mutation decision, and final exit.
All ten steps PASS in 161.0 seconds. Jobq and the wrapper exit zero after 180 seconds.
Default reports 1683 PASS, 481 skipped, in 11.391 seconds. Slow reports 500 PASS, 2012 skipped, in 38.862 seconds.
The slow selection contains 500 distinct PASS identities, including 150 real conformance identities.
Slow coverage checks 1032 slow-only tests. The running-child proof passes in 0.003 seconds. The new zombie/reaped proof passes in 0.002 seconds.
EV-5 repeated attach/detach reports PASS in both tiers, at 0.211 and 0.164 seconds.
The gate reports testkit-passing 55/69, real-passing 40/68, and real-accepted 41/69.
The real-pending report retains 58 IDs, with 33 PASS under the ignored selection. This PR removes no pending ID or increases no accepted minimum.
The mutation step reports no in-diff Linux mutant. No Linux mutation run starts. The raw mutation record count is zero.
Fuzz passes because the diff changes no crate with a decoder harness.

### Evidence limits

The previously read ba7dded6 Mac log covers the exact unchanged own executable code.
Its 84-test suite, twenty selected proof pairs, and one caught/two unviable Mac mutants retain their earlier recorded scope.
No individual mutation catcher is printed in that Mac log. Sol makes no new Mac execution or kernel-source claim.
The body preserves the PID reuse limit. Exited proves that the old process ended; it supplies no status or new-process identity guarantee.
The current safe watcher and rustix probe remain the reused components. The OLD CORE comparison remains applicable.

No source or body finding appears in this inspection. The exact-head Linux gate passes.
This inspection grants no formal CLEAN. The same-head package artifact remains pending.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Round 1 complete at a5406877

Head: `a540687728a7be9718e9bb11de62aa042ed905b6`. Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
Package receipt: msg_plugin-w_1791725080_eaa760.
Package verdict: `216e9b18e45f740b875787402c496ea1bb438c3d`, verdicts/p5-await-end-zombie.md, Round 1.
Sol reads the complete 104-line package artifact and verifies publication on origin/stage1/review-p5.
The published PR still names this exact head and base. The worktree has no unrelated change.

The package artifact reports CLEAN with zero open findings. It agrees with the integration source, import, body, and evidence checks.
The initial inspection b747c2f6, correction inspection 6bbcd93a, and final import/gate inspection 0bc7ec19 remain applicable.
The original failed Linux and Mac logs stay failed. The lead-directed #243 repair import and new exact-head Linux gate replace no historical result.
The exact-head Linux gate passes all ten steps. The unchanged own code retains the supplied ba7dded6 Mac evidence within its recorded limits.
No individual Mac mutation catcher is printed. No new Mac run, local reversal verification, or independent XNU source check is claimed.
The PID reuse limit remains explicit. The answer proves the old process ended; it supplies no exit status or new-process identity guarantee.
The OLD CORE comparison and current safe-component reuse remain applicable. No pending ID is removed or minimum count increased.

Zero integration findings remain open. Section 8 requires no repeated full gate for this unchanged head with both applicable CLEANs.
This CLEAN covers only this exact head, base, source composition, body, and supplied evidence. A later import remains outside this verdict.
The lead retains merge order. Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at a540687728a7be9718e9bb11de62aa042ed905b6

## Import inspection at 312b48a1

Head: `312b48a16cb37163ebb1c897cf9d8a4f8b2374d6`. Base: `ee7c6fca8c0781db09203ac478a8e824cdd3e4ab`.
Parents: reviewed a540687728a7be9718e9bb11de62aa042ed905b6 and exact base ee7c6fca.
Tree: `3e49b5bc28e978cf82a55da8840e3fe273a8adb7`. Receipt: msg_plugin-w_1791725183_0637c8.
This receipt supplies an import for inspection. Final READY will follow the running Linux gate and a Mac pool job.

Sol reads the complete canonical base-check log:
`~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-312b48a1-pool-20261011-062601-85073.log`.
SHA256: `7bbb2346033494b2fb25b0dc37190efd3f31fac6905def9a6b060d956f5649b1`.
The log names the exact previous head, new head, and base. Conditions (1) and (4) PASS.
Conditions (2) and (3) FAIL on platform.rs and platform/macos.rs. Both pending files equal the new base.
Jobq and the wrapper exit one after nine seconds. This failed check requires both delta reviews; it supplies no automatic CLEAN transfer.

The independent permitted static merge-tree check returns the exact recorded tree without a conflict.
Sol reads both complete shared-path import deltas and the current own delta in those paths.
The import adds the accepted #257 thread-count exports, descriptor-count helper, and count proof to platform.rs.
It adds the accepted libproc thread-count helper before the Mac process observer. It changes no own wait decision.
The own Waited documentation, ESRCH branch, signal-zero probe, and PID reuse limit remain intact.
All three own addition/deletion sequences equal the prior reviewed sequences. The own slow_process.rs blob equals the prior head exactly.
All thirteen outside-own imported blobs equal exact base blobs. The import changes fifteen paths in total.
The static diff check passes. No source finding appears in this inspection.

The supplied local Mac clippy, 84-test, and taint results are owner claims; Sol has not read their execution logs.
This inspection grants no formal imported-head CLEAN or execution evidence approval.
Final READY/body, the exact-head Linux and Mac records, and the package artifact remain pending.
The prior CLEAN covers only a5406877 and its recorded base and evidence. Sol ran no execution job.

## Second import and supplied Mac inspection: 2c8f6ea4

Head: `2c8f6ea45f3815fda9728cf4d2900971fd0d3bc7`. Base: `2b8cf08230dc3145dfe3a436bc34feee946c9b07`.
Parents: inspected 312b48a16cb37163ebb1c897cf9d8a4f8b2374d6 and exact base 2b8cf082.
Tree: `63b3c6fbe0733cac4ba07feee1bb045b9c0f8088`. Receipt: msg_plugin-w_1791725616_9de892.
The owner reports a running new-head gate. Final READY and body will follow. No completed 312b48a1 Linux gate is supplied here.

Sol reads the complete second canonical base-check log:
`~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-2c8f6ea4-pool-20261011-063131-22650.log`.
SHA256: `8645a78d47ae430019b94fbd3a87ec4c8718f13a10b5f7da12c52288ff996e36`.
It names the exact prior CLEAN head, new head, and base. Conditions (1)/(4) PASS.
Conditions (2)/(3) FAIL on the same two shared paths. Both pending files equal base.
Jobq and the wrapper exit one after 107 seconds. This failure grants no automatic CLEAN transfer.

The independent permitted static merge-tree check returns the exact recorded tree without a conflict.
The second import changes only ci/remote/job.sh. Sol reads its complete delta and the #266 body.
The body assigns that CI script to the package reviewer and a pool gate; it does not mark the PR HIGH.
The delta measures Docker's data root, validates each deciding measurement, exits 75 on an invalid result, and changes the disk messages.
This #263 inspection supplies no separate formal #266 verdict or independent acceptance of its execution claims.
The imported job.sh blob equals the new base. All three own blobs equal inspected 312b48a1 exactly.
All fourteen outside-own imported blobs equal base. The previous shared-path review remains applicable. The static diff check passes.

### Supplied Mac pool record

Log: `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-312b48a1-pool-20261011-062836-92523.log`.
SHA256: `0e1a52f1a079e94172a0561eb66add5c105a77c86b90e3821b73aa61eb5acc8f`.

Sol reads the complete 505-line record. It names exact head 312b48a1 and base ee7c6fca, on the Mac pool with two CPUs.
Worker prebuild passes in 38.2 seconds. Locked clippy covers all targets and features of botster-test-process and passes.
The slow-profile crate suite passes 85 tests with zero skipped in 11.002 seconds.
The imported count proof and both affected wait proofs report PASS by exact name.
All twenty supplied paired selections report two PASS each and 83 skipped. Both wait names appear in every selection.
Sol requests and runs no repeat. Those supplied selections replace no full Linux gate.

The record prints the complete in-diff Mac source patch against ee7c6fca.
The mutation selection excludes only the named prebuilt-anchor test because its binary is absent from the mutation copy.
Both wait proofs remain selected. The unmutated baseline passes in fifteen seconds of build and twelve seconds of tests.
The gone(&error)-to-false guard replacement is caught at macos.rs:60:23.
The await_end and ended_before_the_watch Ok(Default::default()) replacements are unviable.
The result is one caught and two unviable, zero missed or timeout, in 53 seconds. No individual named mutation catcher is printed.
MAC_EXIT is zero. Jobq and the wrapper exit zero after 164 seconds.

This Mac record covers inspected 312b48a1 and the exact unchanged own code in 2c8f6ea4.
The later import changes only the Linux gate script. This inspection claims no exact-head Mac run on 2c8f6ea4.
No source finding appears. Formal CLEAN waits for final READY/body, the new-head full Linux gate, and the package artifact.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Round 2 — 2026-10-11 — formal import verdict

- Exact head: `2c8f6ea45f3815fda9728cf4d2900971fd0d3bc7`.
- Base: `2b8cf08230dc3145dfe3a436bc34feee946c9b07`.
- Tree: `63b3c6fbe0733cac4ba07feee1bb045b9c0f8088`.
- READY: `msg_plugin-w_1791726036_a3ac18`.
- Package artifact: `e2e8ee6103db08deec24a4b7b1a15c0712a62251`, `verdicts/p5-await-end-zombie.md`, Round 2.
- Risk: HIGH. Plan 23ar remains applicable.

Sol reads the complete published body and complete package round. The published PR names the exact reviewed head and base. The body states both shared-path overlaps and both Mac evidence limits. The origin package branch contains its artifact.

The source and import inspections above remain applicable. They cover both shared paths, both canonical base-check failures, every imported-only blob, and the unchanged own source. The independent static merge tree matches the recorded tree. The prior source closures and prior-art conclusions remain applicable. Zero source or body findings remain open.

### Final exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-await-end-zombie-2c8f6ea4-pool-20261011-063327-26304.log`.

SHA256: `369fdd1a08b1ab3f027d7df5738e2ed982ca89a263912548fcb19b1a0d33b32d`.

Sol reads the identity, CI results, suite summaries, affected proof results, accounting reports, and mutation and fuzz selections. All ten steps pass in 391.5 seconds. The job exits zero after 398 seconds. The wrapper exits zero after 399 seconds.

- Default suite: 1688 passed, 481 skipped, 17.297 seconds.
- Slow suite: 515 passed, 2016 skipped, 53.923 seconds.
- Static parsing finds 515 distinct slow PASS identities and 151 distinct real-conformance PASS identities.
- Both affected wait proofs pass by exact name, in 0.004 and 0.003 seconds.
- The imported count proof passes in the default suite.
- EV-5 repeated attach/detach passes in both suites, in 0.323 and 0.448 seconds.
- Slow coverage checks 1046 slow-only tests.
- Minimum remains 56/69 testkit-passing, 41/68 real-passing, and 42/69 real-accepted. The change from round 1 comes from imported #257.
- The report retains 58 real-pending IDs, with 34 passing under the ignored selection. This PR removes none.
- The own diff has no Linux-compiled mutant. The mutation step starts no run.
- Fuzz reports no changed crate with a decoder harness.

The supplied 312b48a1 Mac record above covers the unchanged own executable code. It supplies no exact-head Mac run on 2c8f6ea4. It prints no individual mutation catcher. The earlier failed Linux and Mac records remain failed. The earlier 312b48a1 Linux gate is not accepted as this head's full gate.

The package and integration conclusions agree on source, import composition, body, and evidence. This round grants no separate formal #266 verdict, extra pending removal, or acceptance of unread local reversal claims. The prior PID reuse and exit-status limits remain explicit.

Zero findings remain open. This CLEAN covers only the exact head, base, source, body, and supplied evidence. A later import needs its own review. The lead retains merge order. Section 8 requires no repeat full gate on this unchanged head.

Sol ran no build, test, gate, mutation, reversal, measurement, or executable base-check job.

VERDICT: CLEAN (0 open)

## Merge verification — 2026-10-11

Merge: `19b24d5efa9ac8e88ac17a82211eececb2c2c00f`.
Parents: reviewed base `2b8cf08230dc3145dfe3a436bc34feee946c9b07` and reviewed head `2c8f6ea45f3815fda9728cf4d2900971fd0d3bc7`.
Tree: `63b3c6fbe0733cac4ba07feee1bb045b9c0f8088`.

Sol reads the exact local merge object after the P3 import receipt names it. Both parents and the tree match the accepted Round 2 review and gate. No later source is accepted. The evidence limits remain intact. Sol runs no execution job.

VERDICT: CLEAN (0 open)
