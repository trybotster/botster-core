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
