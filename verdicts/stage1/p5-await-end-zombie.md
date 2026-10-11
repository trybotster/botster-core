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
