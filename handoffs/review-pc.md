# PC package reviewer handoff

- Reviewer session: `sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7`.
- Implementer session: `sess-1791628587-0178-4476c8a4ce35c7df2dc295defd77dda5`.
- Reviewer branch: `stage1/review-pc`.
- Scope: 38 program-control IDs after #222 and #227. The original PR #220 group had 32 IDs.
- Binding brief: `~/botster-sessions/shared/core-stage1/brief-program-controls.md`.
- Binding plan: `~/botster-sessions/pins/stage1-plan.c77c2434.md`, revision 23cm.
- Verified plan SHA-256: `c77c243490be26d6e4ee9f5b9491fd8b62d6dc9827dceebccfd864e2d60abf02`.
- The last #233 source review uses contracts-v0.1.29. The new design base uses contracts-v0.1.32 under plan 23ar.
- Initial review base: PR #220, `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
- Last #233 reviewed head: `a7be76d55ed38bc89d6a8136c4c62d5d2c9bdb3a`.
- Last #233 review base: `9ef7e57b857a551e5b66a84d0953630186cb51e7` on `v1`.
- Current integration reviewer: Sol, `sess-1791654840-017a-4e0798223b6086de3e26b7b486e22973`, as relayed by the implementer.
- File split agreed with P6: `crates/botster-core-testkit/src/real/program.rs`, plus one dispatch line each in `real.rs`.
- PR #264 is MERGED at `a0adb1321bfc3deb7d16735e9a622775ba5f1a2d`. Final gated head: `2f26928054d225875b866a40d3cfdc3a83820b3b`. Both source CLEANs carry under the verified base-only merge exception.
- Last #233 status: package and integration CLEAN at `a7be76d55ed38bc89d6a8136c4c62d5d2c9bdb3a`. All five findings are closed. Its final gate receipt remains pending here.
- Contracts dependency: PR #30 has merged and shipped as `contracts-v0.1.27`.
- The contracts PR adds the probe control socket and the `ad_3` output fence under steward approvals Q1 and Q2.
- The contracts crate-catchup reviewer owned that PR's review. The pin move merged as #231 before the PC Core review.
- Tests: none. Reviews check logic. Tests run only to prove a finding.

The reviewer sent its session UUID to the implementer first.
The lead reported that the implementer's messages expired during a permission prompt.
The reviewer resent its UUID, branch setup, latest received plan pin, audit path, and all required pending-ID rules.
Resend messages: `msg_plugin-w_1791664719_ea9610`, `msg_plugin-w_1791664719_f819f5`, and `msg_plugin-w_1791664719_a2b94e`.
P3 supplied `shared/core-stage1/p3-program-controls-real-forms.md`.
The reviewer read the mapping and audited all 32 transcripts at `contracts-v0.1.24`.
The audit is in `handoffs/pc-transcript-audit.md`.
The audit identifies six kernel-buffer questions and positive log assertions without complete probe-read fences.
The audit found no literal byte-translation equality conflict in this group.
The lead sent the six kernel-buffer questions and three read-fence gaps to the steward.
The lead requires all six IDs to stay in `core-real-pending.txt` until the steward rules.
The reviewer must flag any PR that removes those IDs before that ruling.
The seven minimum IDs go first. The lead says these questions do not affect them.
The lead supplied the steward's ruling in `msg_plugin-w_1791629051_a11546`.
The real tier establishes a held write by stopping probe reads and filling the kernel buffer until Partial or WouldBlock.
The six IDs remain pending. The steward must approve each changed assertion before a transcript changes.
After the seven minimum IDs are done, extend the audit with one line per ID, step, assertion, and clause.
Send that audit to the lead for the steward's review.
The read-fence fixes use `await_control` on the `pty_input` log with the expected content.
Those fixes may proceed with the assertion audit or earlier as a separate contracts change.
The lead supplied revisions 23r and 23s. The reviewer verified both hashes and read both changes.
Revision 23r confirms the PC scope and defines mutation evidence requirements.
Revision 23s defines how named real-only proofs count toward minimum progress.
The implementer relayed revision 23y in `msg_plugin-w_1791664733_bbb956`.
The reviewer verified its hash and read the changes from revision 23s.
The minimum list is now `conformance/minimum-core.txt` on `v1`.
The integration review seat moved to Sol under revision 23x.
Revision 23y defers the pin-move re-pending exception until its separate HIGH PR lands.
All 32 PC transcript blobs are unchanged between `contracts-v0.1.24` and `contracts-v0.1.26`.
The preparation audit remains applicable to those unchanged transcripts.
The implementer sent a status update in `msg_plugin-w_1791666898_3813d6`, with no Core review head.
The reviewer read contracts PR #30 and the implementer's current shared handoff.
The implementer reports 16 of 32 local passes, including three minimum IDs. These are not reviewer-verified acceptance results.
The implementer's handoff reports a later base `fc1e8160` and the single-step gate after #225.
The reviewer must verify the exact base and gate command when a review head arrives.

Preparation questions sent to the implementer:

- Scope answered in `msg_plugin-w_1791666955_4fb7fb`. The implementer quoted the lead's instruction `msg_plugin-w_1791664701_08cbb3`, item 6.
- `conf::ev_5_full_queue_defers_transition_not_output` joins after #222 merges.
- Five IDs from P3's #227 join after #227 merges. P3 names them in `msg_plugin-w_1791664727_f53be5`.
- The reviewer needs the five exact IDs and the merge SHA before applying this conditional scope extension.
- Wake mapping answered in `msg_plugin-w_1791667009_dbb1d8`. The implementer cites P3's agreement `msg_plugin-w_1791666998_9a47ca`.

The reviewer read the revised mapping in `p3-program-controls-real-forms.md`, rule 3.
The reviewer also read the implementer's recorded agreement under "Mapping change".
The mapping permits an observer wake when the probe reports a read and the input log grows.
The wake carries no input, causes at most one pump with no work, and does not change `edges_quiet`.
The mapping cannot satisfy `no_spurious_wakes` while the probe reads PTY input.
The reviewer verified that no transcript at `contracts-v0.1.26` requires both `pty_input` and `no_spurious_wakes`.
If such a transcript appears, P3 requires a new form, such as a driver-side observer wake in `idle_wait`.
That structural fix is a contracts change.
The Core PR body must state this workaround and its limit.
The implementation review must check the wake trigger and all three bounds.
The existing gate command changes only after 23n lands and the lead sends that instruction.

Both fresh PC branches initially pointed to old main, `053148f6e8e63c3c38b2cbabd54a6e4e8211143c`.
The reviewer preserved its unpublished initial branch as `stage1/review-pc-initial-old-main`.
The reviewer then created `stage1/review-pc` from the review base.
The reviewer told the implementer about both initial branches.

The implementer sent revision 23ab in `msg_plugin-w_1791669269_da228c`.
The reviewer verified its hash and read the changes from revision 23y.
Revision 23z makes the real descriptor handoff P3's first task. Four minimum PC IDs depend on that task.
Revision 23aa adds the descriptor-send SIGPIPE requirements and real-process EPIPE proofs for both operating systems.
Revision 23ab requires contracts-v0.1.27. The manifest and 672 IDs do not change.
The reviewer read the contracts changes for the controls, shared wire types, `ad_3`, R52, and R53.
Only `ad_3` changes among the original 32 PC transcripts. It adds an ActivityOutput fence before the terminal-state check.
The six kernel-buffer IDs and three read-fence gaps remain unchanged.
The pin-move PR is `trybotster/botster-core#231`, on `stage1/pc-pin-v0.1.27`.
Its reported head is `df7f0942`; its reported `v1` base is `3677b37e`.
The implementer says the gate is running and asks for no review before the green gate evidence arrives.
The reviewer and Sol must review the pin move under HIGH rule 4.

The implementer requested review in `msg_plugin-w_1791670208_0dba08`.
The reviewer checked PR #231 against base `6c2bccdad9ab6cfac860e63c5eb15d6f8ec5ac6c`.
The seven manifest pins and nine lockfile sources match contracts-v0.1.27 at `b161db87ca82873b4f6b21d8433a69f086abb7cf`.
The manifest has no other value change. The lockfile has no other package change.
All pending, held, deferred, ledger, and minimum ID lists are unchanged.
The reviewer read the full gate's identity and summary. The gate passed on the exact head with exit code 0.
Gate log: `~/botster-sessions/gates/botster-core-stage1-pc-pin-v0.1.27-4521237e-pool-20261010-145612-73744.log`.
The diff has no mutant. The reviewer ran no tests.
The verdict is `verdicts/pc-program-controls.md`: CLEAN, zero open findings, for the pin move only.
Sol's integration review is still required.

The reviewer will push this verdict and send its commit to the implementer, Sol, and lead.
The shared handoff will record the resulting verdict commit after the push.
Next: wait for the implementer's doorbell and the program-controls implementation head.
Review the seven minimum IDs first.
Write severity, evidence, and required change to `verdicts/pc-program-controls.md`.
Commit and push the verdict on `stage1/review-pc`.
Send the implementer the verdict line and verdict commit.
Send the lead only QUESTION, BLOCKED, or CLEAN, with the exact reviewed head.
Update this handoff after every push or verdict.
Never poll.

Push record: verdict commit `392a290cb563fc9718e61f31478a71b0af30f258` is on `origin/stage1/review-pc`.
The reviewer worktree is clean. The program-controls implementation review remains pending.

Integration update: Sol reported CLEAN for PR #231 at `4521237e8200768a36ab4c8a27f268f3914a88ab`.
Integration verdict commit: `737efd716ed1f8af686228bb9bce124a737103b1`, file `verdicts/stage1/pc-pin-v0.1.27.md`.
Message: `msg_plugin-w_1791670544_54135f`. Sol read the package verdict and sent the integration verdict to the lead.
Both reviews cover the pin move only. The program-controls implementation still needs separate review.

Implementation status update: `msg_plugin-w_1791678907_de080d`.
PR #233 source head: `e75ba5f71c6187697383fd9b7f298548c5ac8008`.
The source head merges `v1` at `b028a269383b8d2af9ac349b395765a47f1b8f1d`.
That base merged #222. The conditional scope extension for `conf::ev_5_full_queue_defers_transition_not_output` now applies.
The original 32 IDs plus this ID make 33. The five IDs from #227 remain conditional on its merge.
The source removes 16 real-pending IDs, including three minimum IDs. The expanded group has 17 IDs still pending.
The implementation is not READY. No green gate exists on the source head.
The lead holds Core gates until P6's mutation-output fix merges and the lead sends MERGED.
The previous gate on `a21fcbd3` ended with exit code 124 after the second mutation stage produced no output for more than 20 minutes.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-a21fcbd3-pool-20261010-164623-57687.log`.
The reviewer started no gate and issued no implementation verdict.

The implementer's handoff names plan `stage1-plan.b895ad49.md`, revision 23ae.
The reviewer verified SHA-256 `db0522ad7a0ca5f0ede3e7b236049f11be504161d6cfa9ee9c65638d46c4ac3b`.
The reviewer read the changes from revision 23ab.
Revision 23ac changes P3's SIGPIPE proof to a safe recording handler with a control proof.
Revision 23ad requires decisions at receipt for worker client frames.
Revision 23ae changes the descriptor rule to frame starts, with kernel proofs for each operating system.
These revisions change P3's work. The PC controls retain their scope and contracts pin.
Next: wait for the implementer's READY, exact head, and green gate evidence.

Early source inspection: PR #233 at `e75ba5f71c6187697383fd9b7f298548c5ac8008`, base `b028a269383b8d2af9ac349b395765a47f1b8f1d`.
The reviewer read all five changed files and the pinned probe control implementation.
The five control dispatches match P3's mapping. The probe applies requests before their acknowledgements.
The harness preserves the input report order. The input observer signals host wakes without adding an input or changing `edges_quiet`.
The group has 33 IDs at the base, 16 removals, 17 remaining IDs, and no additions in the PR's own diff.
`ev_5_full_queue_defers_transition_not_output` remains pending. It is not a seventeenth flip.
All six IDs that need steward approval remain pending.
The three minimum removals are `ad_3`, `tm_3_due_deadline_fires_in_pump`, and `tm_3_next_deadline_is_host_armed`.
The four minimum route IDs remain pending.

Sol sent three early source concerns in `msg_plugin-w_1791679175_da53fa` and sent them directly to the implementer.
1. `request` uses a blocking `write_all` while it holds the state lock. Its deadline starts after that write.
2. Exit watchers are detached and do not check `closing`. Some tests watch the test process itself.
3. The PID cache skips later identity checks. `connected` accepts an existing probe before it checks `exited`.
Sol will save the source evidence. These concerns require review of the implementer's response at READY.
The reviewer ran no tests or gate. No implementation review round or verdict is complete.
The GitHub MCP read failed. The CLI fallback could not connect during this early inspection.
The reviewer must read the current PR body, including Prior art and the observer-wake limit, at READY.
Next: wait for the implementer's response or READY doorbell. Do not poll.

Sol pushed the early source record at commit `51fc656599b496cbe7b2ca50e8917ba8f1ef48a3`.
File: `verdicts/stage1/pc-program-controls.md`. Message: `msg_plugin-w_1791679256_f49571`.
The package reviewer read that record.
SI-1 is HIGH: the blocking request write has no deadline and holds the state lock.
SI-2 is MEDIUM: exit watchers survive harness cleanup.
SI-3 is MEDIUM: cached probe state bypasses the complete process identity.
All three findings are open in the integration record. The source inspection completes no round and grants no CLEAN verdict.
Sol confirms that the PR body states the observer-wake exception and its `no_spurious_wakes` limit.
Sol read the three minimum transcripts and their retained fences.
Next: wait for the implementer's response or READY doorbell.

Early correction inspection: message `msg_plugin-w_1791683021_c8fe87`.
Source head: `ebac3c692bcd317b6e4f53fb98c976cafc24edf0`.
This head merges `v1` at `0a564513e50930ce25f2e4a175f45cffbef7119d` (#237 and #234).
The package reviewer read the program.rs changes from `e75ba5f7`.
The source removes the detached exit watchers and their state tables.
The source keys probes by the complete `ProcessIdentity` and adds a mismatched-start-time proof with an existing connection.
The request writer uses a non-blocking socket and releases the state lock before its write.
Sol reported two SI-1 points that remain:
1. The report-progress proof must read real request bytes before it sends Input. Otherwise the report can finish before the control starts.
2. `send_line` must check its deadline before each write. The current code checks only after WouldBlock or Interrupted.
The never-draining-peer proof must establish Bad at the existing cleanup bound.
These are source corrections, not completed finding closures. The READY head, green gate, and formal review remain pending.
The reviewer ran no execution job. Next: wait for the implementer's response or READY doorbell.

Sol pushed correction record `ddcb26991e60ab0f607b2603a850c3a4016f6dc2` in `verdicts/stage1/pc-program-controls.md`.
Message: `msg_plugin-w_1791683081_54196d`.
Sol closes SI-2 and SI-3 in source at `ebac3c692bcd317b6e4f53fb98c976cafc24edf0`.
SI-1 HIGH remains open for the write deadline check and socket-proof ordering.
Sol reports that the automatic merge tree matches the source head.
The gate, READY, and same-head package verdict remain pending. The source inspection completes no round.

SI-1 follow-up: message `msg_plugin-w_1791683187_5c2e87`.
Source head: `686300e2b1e6cbaded29761dc8963483b4ac03bc`.
Integration record commit: `a6d942576800883da25c7bc8a37ebd4207eb2e32`, file `verdicts/stage1/pc-program-controls.md`.
The package reviewer read the complete program.rs delta and the integration follow-up record.
`send_line` checks the shared deadline before every write.
The report-progress proof waits for request readiness at the socket before it sends Input. It leaves the request unread until the report arrives.
The never-draining-peer proof requires Bad from the write deadline path.
Sol closes SI-1 in source. SI-2 and SI-3 remain closed in source.
The implementer's retained lock-held-write reversal log must accompany READY.
All three source findings are closed in source. The exact-head green gate and formal package verdict remain pending.
The source inspection completes no round. The reviewer ran no execution job.
Next: wait for READY with the exact head, gate log, reversal log, and current plan pin.

Package review Round 1: PR #233 READY in `msg_plugin-w_1791698170_09192c`.
Reviewed head: `fce17f78b96009cb560fb3896a6f957840ec2c43`.
Reviewed base: `11a5246a669de9e8ae6b7ca1ed7763192a099781`.
Verdict: CLEAN, zero open findings. File: `verdicts/pc-program-controls.md`.
The reviewer read all five own-diff files, the corrections, the merge resolutions, the three minimum transcripts, and the gate and reversal evidence.
The source closes SI-1, SI-2, and SI-3. No new source finding exists.
The group now has 38 IDs. Sixteen IDs leave real-pending; 22 remain.
The five added #227 IDs are `in_10_both_guards_are_checked_atomically_with_ordering`, `in_10_guard_refuses_when_client_frame_received_before_start`, `in_10_lagging_host_view_gets_stale_not_a_wrong_write`, `in_4_input_rev_client_advances_on_receipt`, and `ou_1_view_only_route_ignores_input`.
Their merge SHA is `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
All five remain pending and also need `route_send`.
All six steward-review IDs, `ev_5_full_queue`, and the four minimum route IDs remain pending.
The lead permits the two Linux-passing race-sensitive IDs to remain pending under ruling (A), `msg_plugin-w_1791698173_6bb0a3`.
The lead's pool rule, `msg_plugin-w_1791697703_733a2c`, permits review after the pre-review gate and requires one final gate after both CLEAN reviews.
The implementer supplied both rules verbatim in `msg_plugin-w_1791698260_ea0cc8`.
The pre-review gate passed on `7a4ab70bf08ff28344e9e47cad09d923ff3ae1d4`; it does not establish CI on the reviewed head.
The program module is byte-identical on those two heads. The reviewer read the conflicting merge changes separately.
The SI-1 reversal failed with the state lock held and passed on the unmodified head. Its log accompanies READY.
The reviewer ran no execution job.
Next: push the verdict and send its commit to the implementer, Sol, and lead.
Then wait for the integration verdict and final-gate doorbells.
The PC group is not complete. Do not start the extended assertion audit until the seven minimum IDs are done.

Round 1 supplement: `msg_plugin-w_1791698546_ff52a2` from Sol identifies SI-4 LOW.
The package reviewer missed the required Prior art detail.
The package CLEAN at `deec5b5bf779b55d759e18b5264fe3ea0e6d1905` is superseded on the same reviewed head.
Current verdict: NOT CLEAN (1 open). SI-4 requires the pinned old-code comparison and the library choice and custom-infrastructure reason in the PR body.
SI-1, SI-2, and SI-3 remain closed.
The reviewer read and verified revision 23ai. The focused Mac log is also required before renewed CLEAN.
The implementer's handoff reports that the focused Mac run is in progress. No result has arrived.
No reviewer execution job ran.
Next: push the supplemental verdict. Wait for the PR-body correction and focused Mac evidence doorbells.

Push record: supplemental verdict `b5ca3ab9ed81a5ba6f26965d3b374f7c44543070` is on `origin/stage1/review-pc`.
Current verdict: NOT CLEAN (1 open) on `fce17f78b96009cb560fb3896a6f957840ec2c43`.
The reviewer worktree is clean.
Sol confirms the accepted revision 23ai pin and hash in `msg_plugin-w_1791698628_0f3115`.
Plan review artifact: `b8d05666648878840d370d065d8b208a41d1f107`, `verdicts/stage1/plan-r23.md`, Round 46.
Next: wait for the corrected PR body and focused Mac evidence.

Renewed package CLEAN: SI-4 closed after the PR-body corrections in `msg_plugin-w_1791699014_d68628`.
The reviewer checked the old Core references, library choices, custom-infrastructure reasons, and both corrected claims.
The note no longer bans exported composition or claims that no library can bound a UnixStream write.
All 15 PC tests passed in the focused Mac log on `fce17f78`. Both harnesses' probe-environment test also passed.
The broader run had 30 passes and one failure in unchanged P5 zombie-identity code. It was not green.
Real conformance had 141 passes and no failures. The report counted 39 / 68 minimum real-passing and 40 / 69 real-accepted IDs.
The pending-real report listed 64 IDs and zero passes. No additional ID leaves pending.
Mac log: `~/botster-sessions/gates/botster-core-pc-program-controls-mac-fce17f78-20261010-230102.log`.
The PR comment records the log and the P5 failure. The implementer reports that the lead assigned that failure to P5.

The implementer merged #241 from base `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
New reviewed head: `ec317eaedbfa29467ceb68cb61b0919b717f3a3f`.
Message: `msg_plugin-w_1791699106_f903c9`.
The reviewer read the passing base-merge record and the two base-path changes.
The reviewer independently verified that the PR's own diff is byte-identical.
Diff: 50,444 bytes, SHA-256 `d96c1406b582875cac7ecd22ce9d105d18b64a74fa31ce72a4fe602f283cab0d`.
Log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-fce17f78-ec317eae-20261010.log`.
No reviewer delta round is required. The final gate must run on the new head after both reviewers report CLEAN.
The reviewer ran no execution job.
Next: push renewed CLEAN and send the verdict commit. Then wait for the integration verdict or final-gate doorbell.
The 22 pending PC IDs remain pending. The PC group is not complete.

Push record: renewed CLEAN verdict `3ebf5746ef1876ce00e86e2bee5bd7a86a318621` is on `origin/stage1/review-pc`.
The verdict clears PR #233 logic at `ec317eaedbfa29467ceb68cb61b0919b717f3a3f`, with zero open findings.
The reviewer worktree is clean. Integration CLEAN and the final gate remain required before merge.
Next: wait for the doorbell. Never poll.

Integration CLEAN received: `msg_plugin-w_1791699461_e607b0` from Sol.
Head: `ec317eaedbfa29467ceb68cb61b0919b717f3a3f`. Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Integration verdict commit: `a7d8d900992deda9846573487dde2ab578d76df0`.
File: `verdicts/stage1/pc-program-controls.md`. The package reviewer read the complete artifact.
Both reviewers report CLEAN on this head. SI-1 through SI-4 are closed.
The final gate remains required before merge. The 22 pending PC IDs remain pending.
The focused PC Mac evidence passes. The broader run retains the unchanged P5 zombie-identity failure.
The reviewer ran no execution job. Next: wait for the final-gate or new-head doorbell. Never poll.

Additional review assignment: Core #242, tests only, LOW, with no integration review.
The implementer supplied the lead's exact instruction in `msg_plugin-w_1791699652_879436`.
Lead instruction: `msg_plugin-w_1791699598_53afe2` from the lead session.
The lead assigns #242 to the PC package reviewer alone.
The implementer must gate it after #233's final gate ends, with one Core gate at a time from that implementer.
This assignment does not transfer the contracts PR review to the PC reviewer.

Source inspection before READY: #242 head `75cb8b37c2691bf156d15f62b06b45fb3879b010`.
Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`. Branch: `stage1/pc-title-order`.
The reviewer read the complete one-file delta, the existing Rig, the host driver, observe, sync terminal_state, and event queue.
The reviewer read R-57 and its condition 3 follow-up, the PR body, and the old Core references.
The two tests use pump_events 1. The helper reads terminal_state after each pump and does not poll events during the wait.
The first test requires a pump split before the title and later checks Bell before TitleChanged.
The full-queue test requires the loss marker to name Bell and Notification when the title appears.
The title observation sets the cache and posts TitleChanged in one call. The driver checks its budget between frames.
The launch and adoption baseline exception remains in the PR body and R-57.
No production code, transcript, status list, pin, or timeout value changes.
No source finding is open. The source inspection grants no formal verdict before the pre-review gate arrives.

The reviewer read `~/botster-sessions/gates/botster-core-pc-title-order-reversal-20261010-231854.log`.
The selected record reports both tests failing when observe(Title) omits the TitleChanged post.
The record is not a green gate and does not replace the gate's normal test results.
The reviewer ran no execution job.
Next: wait for #242 READY with the exact head and green pre-review gate. Also wait for #233's final-gate result.
Never poll.

#233 final-gate failure and correction: `msg_plugin-w_1791702764_c88491`.
The gate on `ec317eaedbfa29467ceb68cb61b0919b717f3a3f` failed on one missed send_line guard mutant.
Stage 2: 79 mutants, 58 caught, 1 missed, 0 timeout, 20 unviable. Eight earlier steps passed; fuzz did not run.
Gate log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-ec317eae-pool-20261010-231846-63449.log`.
Corrected head: `8cfdd6715c6a69c419d9819636e185bfff74bb24`. Base remains `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
The reviewer read the complete 17-line test delta and its send_line error path.
The added closed-peer test requires BrokenPipe or ConnectionReset, which rejects a retry until TimedOut.
No production code, transcript, timeout value, status list, pin, or mutation exclusion changes.
The selected reversal fails with TimedOut after 10 seconds. It contains no passing normal run.
Reversal log: `~/botster-sessions/gates/botster-core-pc-send-line-guard-reversal-20261011-001126.log`.
SI-5 MEDIUM records the mutation proof gap. It remains open pending normal-run and gate evidence.
SI-1 through SI-4 remain closed. No additional source issue exists in this delta.
The reviewer requested focused Mac evidence for the added socket test under plan 23ai.
The implementer reports that the corrected Linux gate is running. The reviewer did not read or poll that running gate.
Current verdict: NOT CLEAN (1 open), superseding package CLEAN `3ebf5746` for the current PR.
The reviewer ran no execution job. Next: push the verdict, then wait for the gate and Mac-evidence doorbells.
The #242 formal verdict still awaits its pre-review gate. The 22 pending PC IDs remain pending.

Sol read the new proof and reversal in `msg_plugin-w_1791702833_e83760`.
Sol finds no new source defect and also requests a focused Mac normal-run result with the exact head and operating system.
Sol will read the failed gate and the package artifact when supplied. Sol ran no execution job.

Push record: delta verdict `d26e3057d4909ec718de1e49ad2feb094a806e83` is on `origin/stage1/review-pc`.
Current #233 verdict: NOT CLEAN (1 open) at `8cfdd6715c6a69c419d9819636e185bfff74bb24`.
The reviewer worktree is clean. Wait for the corrected gate and focused Mac evidence. Never poll.

SI-5 focused Mac evidence received: `msg_plugin-w_1791702900_155291`.
The reviewer read both complete logs. Each names head `8cfdd6715c6a69c419d9819636e185bfff74bb24` and macOS 27.0.1 arm64.
Normal log: `~/botster-sessions/gates/botster-core-pc-send-line-test-mac-8cfdd671-20261011-001400.log`.
All 16 real::program tests pass, including the new closed-peer test. The run exits with code 0.
Reversal log: `~/botster-sessions/gates/botster-core-pc-send-line-guard-reversal-8cfdd671-20261011-001419.log`.
The log includes the guard-to-true diff. The closed-peer test fails with TimedOut after 10 seconds, with exit code 101.
The reviewer checked the PR comment. It records both logs and the exact failed-gate result.

Lead acceptance: `msg_plugin-w_1791702907_569ca9`.
The lead states that the local Mac run meets plan 23ai when its log is in the PR.
The lead requires no pool Mac job. SI-5 now waits only for the green Linux gate on 8cfdd671.
The package verdict remains NOT CLEAN (1 open) until that gate evidence arrives.
The reviewer ran no execution job and did not poll the running gate.
Next: wait for the Linux gate doorbell. The #242 formal review also waits for its pre-review gate.

Corrected #233 gate failed: `msg_plugin-w_1791703021_b283c5`.
Head: `8cfdd6715c6a69c419d9819636e185bfff74bb24`. Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-8cfdd671-pool-20261011-001231-18048.log`.
The reviewer read the header, failure record, test summary, and gate summary.
Seven steps passed. The slow step failed: 444 tests passed and one failed.
Failure: `conf::ad_7_payload_launches_after_durable_identity`, seed 0, step 6.
The control returned alive:true where the transcript expects alive:false.
Mutation and fuzz did not run. The gate exited with code 1.
The added closed-peer test has a Linux PASS record in the slow run.
The implementer reports that the lead assigned the payload_alive failure to P5.
The implementer will not repeat the gate before P5's fix merges, under the lead's rule.
After that merge, the implementer plans to merge v1 and run the gate again.
SI-5 remains open because the corrected mutation stage has not run and no green final gate exists.
The package verdict remains NOT CLEAN (1 open). The Mac evidence remains accepted.
The reviewer ran no execution job. Next: wait for the P5-fix merge and new gate evidence. Never poll.

Integration delta verdict received: `msg_plugin-w_1791703212_96aca7`.
Sol reports NOT CLEAN (1 open) for #233 head `8cfdd6715c6a69c419d9819636e185bfff74bb24`, base `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Integration verdict: `6ad0e12e9bfb858c88ef0ef2ab8273eb8b7b4948`, `verdicts/stage1/pc-program-controls.md`.
The reviewer read the complete Round 2 record. Both reviewers keep SI-5 open for the green final Linux gate and corrected mutation result.
Both reviewers accept the source correction, focused Mac normal run, and selected reversal.
SI-1 through SI-4 remain closed. Neither reviewer ran an execution job.

The current binding brief now names plan revision 23aj, `stage1-plan.6dbe1c43.md`.
The reviewer verified SHA-256 `25f26484bc439817c213a1766fc6392f774b276b0bd4796eaea3bb890a5e82e9` and read all changes from 23ai.
Plan head: `6dbe1c43d55e31cba038cee1008098bc0ba41ef0`.
The revision requires contracts-v0.1.28 at 94b63b8, with the R-56 input fences and R-54 through R-57 rulings.
The PC pair owns that separate HIGH pin PR. The current #233 code still uses contracts-v0.1.27.
IN-5 leaves real-pending only if the pin PR's Linux gate reports PASS.
The title step and EV-2 sentinel fence belong to a later tag. The launch/adoption scope limit applies to that fence.
No manifest, ledger, withdrawn, pending, or active Core ID changes occur in this plan revision.
The reviewer changes no pin, transcript, or expectation.
Next: wait for the P5-fix merge, new #233 gate, #242 gate, or pin-PR READY doorbell. Never poll.

#242 formal review complete: READY `msg_plugin-w_1791703733_716b5f`.
Head: `75cb8b37c2691bf156d15f62b06b45fb3879b010`. Base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17`.
Verdict: CLEAN, zero open findings. File: `verdicts/pc-title-order.md`.
The reviewer completed the logic review and read both new test PASS records in the exact-head gate.
Gate log: `~/botster-sessions/gates/botster-core-stage1-pc-title-order-75cb8b37-pool-20261011-001710-40110.log`.
All ten steps pass. The diff has no mutant and changes no crate with a decoder harness.
The gate exits with code 0 after 689 seconds. The CI summary reports 364.1 seconds.
The reviewer read the selected reversal. Both tests fail when observe(Title) omits TitleChanged.
The reviewer checked the current PR head, base, body, Prior art, and launch/adoption scope limit.
The lead requires no integration review for this LOW PR.
The reviewer ran no execution job. Next: push the verdict and send its commit to the implementer and lead.
The #233 verdict remains NOT CLEAN (1 open), pending the P5 fix and green corrected gate.
This #242 verdict removes no pending ID and does not complete the PC group.
Then wait for the doorbell. Never poll.

Push record: #242 CLEAN verdict `2e23e264812097f1cdcd12bd7bf0bcb1c008f20a` is on `origin/stage1/review-pc`.
File: `verdicts/pc-title-order.md`. The reviewer worktree is clean.
#233 remains NOT CLEAN (1 open). Wait for the next doorbell. Never poll.

#245 pin review: READY `msg_plugin-w_1791706164_dd4bfa`.
Head: `df991fdf57122c7b59b06069ab7da01c992ca175`. Base: `f6778ad85c01266ac49e2ededd97487da689e920`.
Risk HIGH. Both PC and Sol review this pin move.
The current brief names plan 23ak. The reviewer verified its full SHA-256 and read its changes from 23aj.
The seven manifest pins and nine lock sources match contracts-v0.1.29 at `65f76ab9829d7a0fda2835c10abc7ffae2c35215`.
No other dependency value changes. No Core or contracts status list changes. The frozen manifest is unchanged.
The reviewer read the dependency driver changes, seven transcript changes, and relevant rulings.
The PR flips no ID. EV-2 and IN-5 still require #233 and its final PASS evidence on this pin.
The reviewer awards no minimum progress for the unfenced AD-7 proof under R-59.
The later R-60 and R-61 controls and lint remain separate work.

The exact-head Linux gate passes all ten steps and exits with code 0 after 254 seconds.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-pin-v0.1.29-df991fdf-pool-20261011-010434-15999.log`.
The diff has no mutant. Pending-real has zero passes among 77 IDs. EV-5 passes on both Linux tiers.
Mac evidence head: `f5bb74ceab93cbb8e12d070fb15806375ce0d791`, before the base merge and comment correction.
Log: `~/botster-sessions/gates/botster-core-pc-pin-v0.1.29-mac-f5bb74ce-20261011-005918.log`, plus the retained step2 raw output.
Testkit strict passes 206 tests. Real conformance has 127 passes and one failure.
EV-5 repeated attach/detach fails at seed 6, step 21: PendingLimit instead of success, pending_ops 4.
The EV-5 transcript is unchanged between the two contracts tags. Five isolated Mac runs pass.
The cause remains unknown. The implementer reports a running 30-run trace job. The reviewer did not poll it.
PC-PIN-1 MEDIUM remains open for the unexplained active Mac failure.
Verdict: NOT CLEAN (1 open). File: `verdicts/pc-pin-v0.1.29.md`. No additional pin-source finding exists.
Sol requested the committed artifact in `msg_plugin-w_1791706271_3e9cd6`.
The reviewer ran no execution job. Next: push the verdict, send its commit, then wait for cause evidence.
The #233 SI-5 finding remains open. The PC group is not complete. Never poll.

Push record: #245 verdict `6a3c2dac562c986a9a6d1c470baa123772f5d0fa` is on `origin/stage1/review-pc`.
File: `verdicts/pc-pin-v0.1.29.md`. NOT CLEAN (1 open) at `df991fdf57122c7b59b06069ab7da01c992ca175`.
PC-PIN-1 awaits the cause and resolution of the active Mac EV-5 failure. The reviewer worktree is clean.
Next: wait for the cause-evidence or other authorized READY doorbell. Never poll.

#245 supplement: `msg_plugin-w_1791706439_09d0d5` from Sol identifies PIN-SI-1 LOW.
The new Cargo.toml comment and PR summary list R-54 through R-58, but the pinned tag also contains R-59.
The package reviewer missed that description defect. The correct range is R-54 through R-59.
The start_held control and fences remain later work. The unfenced AD-7 tally limit remains in force.
Sol asked the implementer to correct both descriptions.
The package verdict now reports NOT CLEAN (2 open) on the same head `df991fdf57122c7b59b06069ab7da01c992ca175`.
PC-PIN-1 MEDIUM remains open for the unexplained Mac EV-5 failure. PIN-SI-1 LOW is the second open finding.
The reviewer ran no execution job. Next: push the supplemental verdict, then wait for the correction and cause-evidence doorbells.

Push record: #245 supplemental verdict `f774f13238d3499e9af5ae45811ff8488435ebeb` is on `origin/stage1/review-pc`.
Current #245 verdict: NOT CLEAN (2 open) at `df991fdf57122c7b59b06069ab7da01c992ca175`.
The reviewer worktree is clean. Wait for the correction and cause evidence. Never poll.

#245 delta reviewed: `msg_plugin-w_1791706585_c829c9`.
New head: `c57ec8269f42a0d2bc3a8578ae2a8da1b7b47af4`. Base: `5c480db457797fa7ebecb33835a52e26a5a8490d` (#247).
PIN-SI-1 LOW is closed. The Cargo.toml comment and PR summary now name R-54 through R-59.
They keep start_held and its fences in a later tag. The PR retains the unfenced AD-7 tally limit.
The reviewer read the failed base-merge check and the complete source delta.
Check log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-df991fdf-c57ec826-20261011.log`.
Conditions 2 and 3 fail because both sides change Cargo.toml. The merge is conflict-free; both line-set conditions pass.
The reviewed head preserves the base's sha2 profile: opt-level 3, debug assertions and overflow checks enabled.
Its own manifest values change only the seven contracts tags. The other three own files match the previous reviewed head.
No status entry changes and no ID flips. No additional source finding exists.
Current #245 verdict: NOT CLEAN (1 open), PC-PIN-1 MEDIUM. File: `verdicts/pc-pin-v0.1.29.md`.
The old green gate covers df991fdf, not the new head. The final gate remains required before merge.
The implementer reports 13 failures among 30 EV-5 trace runs, all at step 21. No retained trace log has arrived.
The lead requested an equal-load old-pin and new-pin comparison on the new base profile.
The implementer reports that it is running. The reviewer did not read or poll that running job.
The reviewer ran no execution job. Next: push the delta verdict and wait for the comparison evidence.

Push record: #245 delta verdict `c139fc773768b27213418f43ef11edd0eb46b459` is on `origin/stage1/review-pc`.
NOT CLEAN (1 open) at `c57ec8269f42a0d2bc3a8578ae2a8da1b7b47af4`. PIN-SI-1 is closed; PC-PIN-1 remains open.
The reviewer worktree is clean. Wait for comparison evidence. Never poll.

Sol's #245 delta source check: `msg_plugin-w_1791706702_0719a1`.
Sol closes PIN-SI-1 at c57ec826, confirms the import tree, and keeps PC-PIN-1 open.
The package artifact message crossed Sol's request. The reviewer resent verdict commit `c139fc773768b27213418f43ef11edd0eb46b459`.
Sol will record NOT CLEAN (1 open). The reviewer ran no execution job. Wait for the doorbell.

#245 comparison evidence received: `msg_plugin-w_1791707284_39d6f4`.
The reviewer read the three completed job logs, all 90 raw run files, and both trace patches.
Old pin at 5c480db4: one failure among 30 runs. New pin at c57ec826: two failures among 30 runs.
Earlier new-pin head df991fdf: 13 failures among 30 runs.
Old log: `~/botster-sessions/gates/botster-core-pc-ev5-trace-mac-old-pin-v1-5c480db4-20261011-011611.log`.
New log: `~/botster-sessions/gates/botster-core-pc-ev5-trace-mac-new-pin-c57ec826-20261011-011611.log`.
Earlier log: `~/botster-sessions/gates/botster-core-pc-pin-ev5-trace-mac-df991fdf-20261011-010924.log`.
The patches add diagnostic reads and trace output. They call each driver step once and change no expectation or fence.
They run on contracts commits b161db8 and 65f76ab. The transcript is unchanged between those commits.
No op-3 Completed event appears in 472 combined old/new seeds.

The reviewer found a difference between the claimed and actual step-21 errors.
The old-pin failure and both new-pin comparison failures return UnknownRoute, not PendingLimit.
The earlier df991fdf job has seven PendingLimit failures and six UnknownRoute failures.
All 16 failed runs show pending_ops 4 and queued_events 0 at the step-19 diagnostic.
The comparison does not reproduce the exact original PendingLimit result on the old pin in this sample.
The PR body incorrectly says that all 16 return PendingLimit. The reviewer requested a correction in `msg_plugin-w_1791707381_afe046`.
The reviewer sent the error distinction to Sol in `msg_plugin-w_1791707381_39788c`.
PC-PIN-1 remains open until the evidence record is correct.

The implementer supplied the lead's exact rule and disposition in `msg_plugin-w_1791707329_40c4aa`.
Lead rule: `msg_plugin-w_1791706429_0cb9a5`. A failure on both pins lets #245 proceed; EV-5 becomes a separate open item.
Lead acceptance: `msg_plugin-w_1791707306_5b703d`. Both reviewers re-check PC-PIN-1, then one final gate runs.
The lead assigns the missing d1 completion to P3. The step-19 fence question goes to the steward.
The trace copies remain available for those owners. The reviewer will preserve the sample limit in the verdict.
The reviewer ran no execution job. Next: wait for the PR-record correction, then close the pin finding under the lead's disposition.
The EV-5 defect remains open outside the pin PR. Never poll.

#245 renewed package CLEAN: `msg_plugin-w_1791707439_a03590` supplies the PR-record correction.
Head remains `c57ec8269f42a0d2bc3a8578ae2a8da1b7b47af4`. Base remains `5c480db457797fa7ebecb33835a52e26a5a8490d`.
The reviewer checked the corrected PR body. It states all error counts and the old-pin sample limit accurately.
PC-PIN-1 closes for the pin review under the lead's explicit rule for failures on both pins.
Lead rule: `msg_plugin-w_1791706429_0cb9a5`. Lead acceptance and owners: `msg_plugin-w_1791707306_5b703d`.
The implementer supplied them verbatim in `msg_plugin-w_1791707329_40c4aa`.
The EV-5 defect remains open separately. P3 owns the missing d1 completion; the steward owns the step-19 fence question.
The reviewer read R-63's correction, which preserves UnknownRoute as a separate open fact.
The comparison reproduces a step-21 failure on the old pin, but not the exact original PendingLimit error in that sample.
The reviewer changes no transcript, expectation, or pending entry and clears no EV-5 defect.
PIN-SI-1 remains closed. Current package verdict for #245: CLEAN, zero open findings within the pin review.
File: `verdicts/pc-pin-v0.1.29.md`. The earlier Mac suite remains failed; CLEAN does not claim otherwise.
Integration CLEAN and one final Linux gate on the CLEAN head remain required before merge.
The reviewer ran no execution job. Next: push renewed CLEAN and send its commit. Then wait for the integration or gate doorbell.
The #233 mutation finding remains open. The PC group is not complete. Never poll.

P3's later diagnosis arrived in `msg_plugin-w_1791707489_49d0c7`, citing `msg_plugin-w_1791707467_209cb3`.
P3 attributes UnknownRoute to the worker link close after step 19, without #240's descriptor support.
P3 says Core closes the routes as HandoffFailed during step 20's `await_quiet`.
P3 keeps the missing d1 completion separate and owns its Core fix. The steward's R-63 requires the fence.
The reviewer checked the PR body at the same head and base. It records P3's diagnosis.
The package verdict attributes that conclusion to P3. The reviewer does not independently prove it here.

Push record: #245 package CLEAN is on origin/stage1/review-pc at 57d6511c49685d914b3afbfb6a7f776f24fc2a31.
Reviewed head: c57ec8269f42a0d2bc3a8578ae2a8da1b7b47af4. Base: 5c480db457797fa7ebecb33835a52e26a5a8490d.
The pin review has zero open findings under the lead's scope rule. EV-5 remains a separate open item.
Integration CLEAN and the final Linux gate remain required before merge. The reviewer worktree is clean.
The #233 mutation finding remains open. Wait for the doorbell. Never poll.

#233 base-merge delta: `msg_plugin-w_1791708757_0cc48c`.
Head: `4d05d270d6c6cebcdae247050cd702b6a157c639`. Base: `554d43e890b391cefc9da205c71db6856accb21d`.
The reviewer read the failed base-merge check and both conflict resolutions.
Log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-8cfdd671-4d05d270-20261011.log`.
The pending file equals the new base minus exactly the prior 16 PC IDs. No ID is added.
The merge keeps the base's three other removals and all base comments.
The real harness serves the union of program and route controls. Its tests check both sets.
The merge keeps the program observer wake and the base's shared heard record, route attachment, and payload identity checks.
The program module, testkit manifest, and process-check exception are unchanged from 8cfdd671.
The pin, lock, core-pending, held, deferred, ledger, and real-only files equal the new base.
No additional source finding exists. The package verdict remains NOT CLEAN (1 open): SI-5.
The reviewer read the retained Mac normal run and guard reversal again. They satisfy the focused Mac requirement.
The final Linux gate and corrected mutation result remain required. The reviewer did not read or poll the running gate.
EV-2 and IN-5 remain pending until the completed gate supplies PASS evidence and their removal commit is reviewed.
The six steward-held PC IDs remain pending. The four remaining minimum PC IDs remain pending.

The binding brief now names plan 23al. Its only change from 23ak adds the lead's one-re-gate exception.
The reviewer verified the new plan hash and read that rule. No re-gate is approved by this verdict.
The reviewer read R-64. The steward settles r5's UnknownRoute as the reported HandoffFailed close, not a defect.
The missing d1 completion needs candidate A21 before Core can choose a completion or close reason.
The R-63 fence remains required. The reviewer makes no transcript or contract change.
The PC group is not complete. Wait for the completed gate doorbell. Never poll.

Push record: #233 delta verdict 82fc2df8aa8c052c94f5819c5ec30b2b184aa337 is on origin/stage1/review-pc.
VERDICT: NOT CLEAN (1 open), SI-5, at 4d05d270d6c6cebcdae247050cd702b6a157c639.
The source delta has no additional finding. The reviewer worktree is clean. Wait for the completed gate doorbell.

#233 disjoint base merge: `msg_plugin-w_1791709587_77d7ec`.
Current head: `6dd3caef6ac68eb77bd872e54c9c5ff7ebe003fb`. Base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
The reviewer read the completed check log and confirmed the PR head and base.
Log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-4d05d270-6dd3caef-20261011.log`.
Every condition passes. Only the base's xtask/src/ci.rs change is added.
The reviewer independently confirmed the unchanged own diff: 48483 bytes, SHA-256 74399e1ff9c3d0bdb8ae1bb3fc1606758efa09e3d7666243aae1790547aa38c3.
The new base is an ancestor of the new head. Both pending files retain the source-reviewed content.
No source delta round is needed. The package verdict carries as NOT CLEAN (1 open), SI-5.
The implementer reports cancellation of the stale-base gate and a final gate running on this head.
The reviewer did not read or poll the running jobs. Wait for the completed gate doorbell. Never poll.

Push record: dfead09daa9a2393549cf0ebfe79223a8caaa140 is on origin/stage1/review-pc.
VERDICT: NOT CLEAN (1 open), SI-5, at 6dd3caef6ac68eb77bd872e54c9c5ff7ebe003fb.
The reviewer worktree is clean. Wait for the completed gate doorbell. Never poll.

Sol confirms the disjoint base import in msg_plugin-w_1791709729_b68198.
Integration record: 4252061ee9df200155130e68a891cc57e6a034da. The package reviewer read the complete added section.
Both verdicts remain NOT CLEAN (1 open), SI-5, at 6dd3caef6ac68eb77bd872e54c9c5ff7ebe003fb, base 8dab35d14f7038d3cc0598e2cde8da207bd1b723.
EV-2 and IN-5 remain pending. Wait for the completed final-gate receipt. Never poll.

#233 completed gate and removal delta: `msg_plugin-w_1791710640_8d164a`.
Current head: `a596c945111ae5df7db757e53ef945d238873c75`. Base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
The completed gate on 6dd3caef is green: all ten steps PASS, exit 0 after 1015 seconds on msa1.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-program-controls-6dd3caef-pool-20261011-020628-1436.log`.
The reviewer read its header, suite summaries, pending-real report, all stage-2 mutation outcomes, final summary, and exit result.
Both stages use the same 79 unique mutant identities. Stage 2 reports 59 caught, zero missed, zero timeout, and 20 unviable.
The guard-to-true mutant at real/program.rs:269:20 is caught. The closed-peer proof passes on Linux in 0.002 seconds.
The unchanged focused Mac normal-run and reversal evidence remain accepted. SI-5 is closed.
The gate reports pending-real 61, with exactly two passes: EV-2 and IN-5 (lines 4461 and 4462).
The delta removes exactly those two ID lines. It changes no code, transcript, timeout, pin, exclusion, or other status file.
The PC group retains 20 pending IDs. The six steward-held IDs and four remaining minimum PC IDs stay pending.
R-59 still prevents the unfenced AD-7 pass from counting toward minimum progress. The group is not complete.
Current package verdict: CLEAN, zero open findings. File: verdicts/pc-program-controls.md.
The green parent gate is not CI evidence on the removal head. Integration CLEAN and a final exact-head Linux gate remain required.
Sol requests the completed package verdict in `msg_plugin-w_1791710685_0021ed`.
The reviewer ran no execution job. Next: push CLEAN and send its commit. Then wait for the final gate doorbell. Never poll.

Push record: #233 package CLEAN is on origin/stage1/review-pc at 119acf92cd25163234821eec363b52ba725cdca9.
Reviewed head: a596c945111ae5df7db757e53ef945d238873c75. Base: 8dab35d14f7038d3cc0598e2cde8da207bd1b723.
All five findings are closed. The final exact-head Linux gate and integration CLEAN remain required before merge.
The reviewer worktree is clean. The PC group retains 20 pending IDs. Wait for the doorbell. Never poll.

Integration CLEAN arrived in msg_plugin-w_1791710768_5c8b0a while the package verdict was pushed.
Integration commit: 0835aa8546c37a7f6408ede9a9c6fff506cba6e0. The package reviewer read the complete added Round 4 section.
Both reviewers are CLEAN at a596c945111ae5df7db757e53ef945d238873c75, base 8dab35d14f7038d3cc0598e2cde8da207bd1b723.
The final exact-head Linux gate remains required. The package artifact was sent to Sol after the crossed request.
Wait for the doorbell. Never poll.

#233 fail_handoff base merge: `msg_plugin-w_1791710909_beb231`.
Head: `a7be76d55ed38bc89d6a8136c4c62d5d2c9bdb3a`. Base: `9ef7e57b857a551e5b66a84d0953630186cb51e7`.
The reviewer confirmed the current PR head and base through the GitHub API.
The reviewer read the completed failed check, combined conflict diff, and complete own real.rs diff.
Log: `~/botster-sessions/gates/botster-core-pc-base-merge-check-a596c945-a7be76d5-20261011.log`.
The three conflict resolutions preserve fail_handoff, the five program controls, and route controls with their respective dispatch.
The test retains all three sets and the plain-harness rejections. No control guard overlaps.
The pending file equals the base minus exactly the prior 18 PC IDs, with all base comments preserved.
The merge preserves the base's separate DP-2 removal. It adds no ID.
The program module, testkit manifest, and process-check exception remain unchanged.
The merged edge tap and its tests equal the new base. All other status lists and both pin files equal the base.
No additional source finding exists. SI-1 through SI-5 remain closed. Current package verdict: CLEAN.
The PC group retains 20 pending IDs, including six steward-held IDs and four remaining minimum PC IDs.
The older green gate covers 6dd3caef. Integration CLEAN and the final Linux gate on this exact head remain required.
The implementer reports a running final gate. The reviewer did not read or poll the job and ran no execution job.
The PR body retains historical counts and races. The current verdict records the 18 removals and fenced evidence.
The PC group is not complete. Next: push the delta CLEAN and send its commit. Then wait for the doorbell. Never poll.

Push record: #233 delta CLEAN is on origin/stage1/review-pc at cb1f5854e0ea86fb395326ad3423237237fdd543.
Reviewed head: a7be76d55ed38bc89d6a8136c4c62d5d2c9bdb3a. Base: 9ef7e57b857a551e5b66a84d0953630186cb51e7.
Zero open findings. Integration CLEAN and the final exact-head Linux gate remain required.
The reviewer worktree is clean. Wait for the completed gate doorbell. Never poll.

The implementer refreshed the PR body in msg_plugin-w_1791711102_761f51.
The reviewer read the updated body at the same head and base. It records 18 removals, 20 pending PC IDs, both fences, and completed versus running gates.
This is a description change only. Package CLEAN commit cb1f5854e0ea86fb395326ad3423237237fdd543 remains valid.
Integration CLEAN arrived in msg_plugin-w_1791711107_59ab0e. Integration commit: 00a682a936cfcb7a399f3fbdd397c241c16b17f3.
The package reviewer read the complete added Round 5 section. Both reviewers are CLEAN at a7be76d55ed38bc89d6a8136c4c62d5d2c9bdb3a, base 9ef7e57b857a551e5b66a84d0953630186cb51e7.
The final exact-head Linux gate remains required. Wait for the doorbell. Never poll.

## Phase: named real-process proofs for real-only IDs

The lead assigns this mechanism to PC in the 08:10 and 08:20 PT records of the lead handoff.
Request: msg_plugin-w_1791719105_06675d. Reviewers: this package reviewer and Sol.
Source base: b3cb268f3c2cb3223dadef02033d2bee49f06c59. Binding plan: revision 23ar, stage1-plan.a2897c12.md.
Design: shared/core-stage1/handoffs/pc-real-only-proofs-design.md.
Design SHA-256: 3a88498a9b39d49d110e8cec106670251890da92a2a97a5edee0818927d64811.
The reviewer uses orchestrate-delivery to check the changed acceptance premise and evidence boundary.
The reviewer read its Botster lessons and retained the existing delivery records.

The source confirms that the real tier runs a non-pending real-only transcript instead of its named process proof.
The current count names a slow category, and the existing JUnit parser checks timing attributes only.
All 26 real-only IDs are pending at the exact base. The initial empty mapping supplies no proof or additional pass.
The design preserves the generated real-only file. Core owns the separate proof mapping.
PC implements selection and result checks. P3 owns the first DP-2 proof and its later pending-ID removal.
No runtime, contract, transcript, timeout, pin, or pending-ID change belongs to this mechanism phase.

Package design verdict: CLEAN (0 open), verdicts/pc-real-only-proofs-design.md.
The accepted design requires selected, unignored tests and same-run JUnit PASS for every proof line.
The result check must reject malformed, missing, skipped, failed, errored, or failed retry evidence.
An ambiguous duplicate result must not hide a failure. Report freshness must survive cleanup errors.
The suite only reports the proof source. The successful gate establishes acceptance.
The source PR still needs both exact-head reviews, a Prior art note, and its gate.
Sol's design CLEAN is 211ca91b0e020c4beed0fbb0d1025932858132db, received in msg_plugin-w_1791719239_b1f798.
The reviewer read Sol's complete design verdict and ran no test, build, gate, mutation, or merge-check job.
The reviewer canceled an unnecessary character-diff read script. It changed no file or evidence.
Next: publish the design verdict and tell PC to implement the agreed design. Then wait for READY. Never poll.
The original program-controls group remains incomplete.

Push record: the design CLEAN is on origin/stage1/review-pc at 3e5046b99845e24adca9db6f528d3c4f46c4da22.
Design SHA-256: 3a88498a9b39d49d110e8cec106670251890da92a2a97a5edee0818927d64811.
Source base: b3cb268f3c2cb3223dadef02033d2bee49f06c59. Plan: 23ar.
Implementation reviews and the exact-head gate remain required. The reviewer worktree is clean.
Wait for the READY doorbell. Never poll.

Sol acknowledgment: msg_plugin-w_1791720125_3e3063.
Sol reports that he read the complete package design verdict at 3e5046b99845e24adca9db6f528d3c4f46c4da22.
Sol confirms that both design CLEANs cover the same design SHA-256.
Sol published his acknowledgment at 7a7b414825cdcd4e008f99c7a2d5e8dc103c41c1.
The acknowledgment adds no finding. It grants no implementation or gate approval.
The package design verdict remains CLEAN. Wait for the READY doorbell. Never poll.

## PR #264: named real-process proofs, implementation review round 1

READY: msg_plugin-w_1791722173_6c599c.
Head: 617c14228183770b4fc32264aa875728910462de.
Base: 0c5ac80e512f986edd2ec1455012552a41c7f855. Plan: 23ar. Tier: HIGH.
The reviewer verified the PR head, base, body, and full 12-file source diff.
All 26 real-only IDs remain pending. The new proof file contains no proof line.
The PR changes no runtime, contract, transcript, timeout, pin, or pending ID.

Package verdict: NOT CLEAN (1 open), verdicts/pc-real-only-proofs.md.
RP-PC-1 MEDIUM: results uses regular expressions to find testcase text without validating XML structure.
It can credit a tag in an XML comment. It can also credit a case in a truncated document.
A case with an unclosed system-out child also gets credit. These are static source traces, not executed reproductions.
The approved design requires an XML parser and rejects malformed evidence.
PC must use an XML parser, count actual testcase elements, and add the negative cases.
PC must preserve duplicate-case rejection, result checks, exact identities, CDATA, and report freshness.
PC must complete the Prior art note for the XML parser and retain evidence of the installed nextest report format.
Sol independently reports the same defect as RP-SI-1 in msg_plugin-w_1791722276_49f759.
The package reviewer sent Sol its independent source conclusion in msg_plugin-w_1791722308_f7885a.

The reviewer read the completed exact-head Linux gate and its earlier red mutation record.
Green log: gates/botster-core-stage1-pc-real-only-proofs-617c1422-pool-20261011-053009-92833.log.
The log names the same head and base. All ten steps pass. The gate exits 0 after 335 seconds on msa1.
Default: 1648 PASS, 484 skipped, 10.305 seconds. Slow: 480 PASS, 2002 skipped, 29.360 seconds.
The slow step checks zero named proofs. The empty file proves no clause.
Mutants: 65 total, 63 caught, 2 unviable, 0 missed, 0 timeout. Stage 2 catches three check mutants.
The source removes the earlier slice-offset mutants and the equivalent order comparison.
The new whole-body exclusions cover I/O shells and cite tested decisions. No additional exclusion finding exists.
The green gate does not close RP-PC-1. Its malformed-report tests omit these cases.
The reviewer ran no build, test, gate, mutation, reproduction, or merge-check job.
Next: push this verdict and send it to PC and Sol. Then wait for the fix doorbell. Never poll.
The original program-controls group remains incomplete.

Push record: PR #264 NOT CLEAN (1 open) is on origin/stage1/review-pc at f07dc3f2a643fae93e8c0c818b448acb05b2d44d.
Reviewed head: 617c14228183770b4fc32264aa875728910462de. Base: 0c5ac80e512f986edd2ec1455012552a41c7f855.
RP-PC-1 remains open. No implementation CLEAN exists. The reviewer worktree is clean.
Wait for the fix doorbell. Never poll.

Sol implementation verdict receipt: msg_plugin-w_1791722758_9cfcd5.
Sol reports NOT CLEAN (1 open) at 617c14228183770b4fc32264aa875728910462de.
Local Sol verdict: ff06aa630166fa7b9924f2588a639f4f7a8b2430.
File: verdicts/stage1/pc-real-only-proofs-implementation.md.
The package reviewer read the complete local verdict. RP-SI-1 and RP-PC-1 identify the same parser defect.
Sol confirms the published package verdict f07dc3f2a643fae93e8c0c818b448acb05b2d44d and its installed-nextest excerpt requirement.
Sol reports that GitHub DNS prevented his push. This receipt does not establish publication of his local commit.
The package verdict remains NOT CLEAN (1 open). Wait for the fix doorbell. Never poll.

Sol publication receipt: msg_plugin-w_1791723032_fe3dfd.
Sol reports that the DNS limit is closed and his push published ff06aa630166fa7b9924f2588a639f4f7a8b2430.
The source head remains 617c14228183770b4fc32264aa875728910462de. No finding scope changed.
Both implementation verdicts remain NOT CLEAN (1 open) for the same parser defect.
Wait for the fix doorbell. Never poll.

## PR #264: implementation review round 2

READY: msg_plugin-w_1791732123_fd124a.
Head: c0f295ab65fdbd18e9c05928ef835a9f23e4c992. Base: 010f2a49952f9b77581f304dbaa41006b115d40b.
Plan: 23au, stage1-plan.0af4db8d.md. Tier: HIGH.
Package verdict: CLEAN (0 open), verdicts/pc-real-only-proofs.md, Round 2.
RP-PC-1 is closed. The XML tree parser validates the document and actual testcase elements.
Its tests cover comments, escaped output, CDATA, malformed roots, and unclosed children.
The retained nextest 0.9.146 fixture checks first-pass, flaky, failed, ignored, and missing identities.
The reviewer read roxmltree 0.21.1 source and the revised ecosystem Prior art note.

The lead's 15:56 design approval permits a real-only ID's own real-tier trial as its proof.
The reviewer used orchestrate-delivery to check this changed acceptance premise.
The real tier runs an own trial must-pass. The testkit ignores it. Separate named proofs retain both-tier transcript ignores.
The lists check restricts suite lines to the same real-only ID. Coverage requires the own trial selected and unignored.
The JUnit result check requires same-run PASS for every line. Counts include each ID once.
The AD-6 own-trial mapping supplies the existing base proof. It removes no ID.
The reviewer read its pinned transcript, clause expectations, real controls, and existing real behavior proof.
Twenty-six IDs remain real-only; only AD-6 is non-pending. All four status files and nextest configuration equal the base.
PC corrected the opening PR body in msg_plugin-w_1791732210_ee78ed. The reviewer verified it through the GitHub API.

The base-merge report fails. The reviewer checked the overlapping files and changed own diff directly.
The mutants.toml conflict preserves the base's exact-count note and PC's shell exclusion.
known_flaky.rs adds only the new fixture argument. Later merges have no conflict diff.
Gate: gates/botster-core-stage1-pc-real-only-proofs-c0f295ab-pool-20261011-080948-87049.log.
SHA-256: b3da59005323e61c69db4154ce4e8924ed383b49ab5c774b228dbe48d2dc1700.
All ten steps PASS. Wrapper exit 0 after 686 seconds on Linux.
Default: 1705 PASS, 481 skipped, 17.418 seconds. Slow: 550 PASS, 2003 skipped, 56.483 seconds.
AD-6 own trial PASS in 0.461 seconds. The result check names it and checks one proof.
Mutants: 83 identities, 81 caught, 2 unviable, 0 missed, 0 timeout. Stage 2 catches the three misses.
The reviewer matches all 86 raw records and 81 named catch reports.

Two FLAKY OU-9 outcomes do not count as catches. The independent fixture catches both mutants.
PC's record is handoffs/flaky-ou_9-baseline-then-live-no-gap-c0f295ab.md, status OPEN.
The failing step and raw failure text are unavailable. The cause remains undiagnosed.
The reviewer read the record and the lead's 18:21 assignment to P3, cause first, before the attach group.
Package CLEAN does not close or accept that separate defect.
The reviewer ran no build, test, gate, mutation, reproduction, or executable merge check.
Next: push CLEAN and send the verdict to PC, Sol, and the lead.
Sol's integration delta remains required. After both CLEANs, PC merges current v1 once and runs the final exact-head gate.
The current gate base is stale. This verdict covers no later head. Wait for the doorbell. Never poll.
The original program-controls group remains incomplete.


## PR #264: implementation review round 3

Head: 46094d5b504861e59b66ac8cf16b399841d7ec71. Parent: c0f295ab65fdbd18e9c05928ef835a9f23e4c992.
Base: 010f2a49952f9b77581f304dbaa41006b115d40b. Plan: 23au. Tier: HIGH.
PC correction receipt: msg_plugin-w_1791732602_8f8d71.
Sol Round 2 publication: 6e9511aa7e3474c63d55b9593979ae14f08c2d75, receipt msg_plugin-w_1791732725_699ea1.
Sol closes RP-SI-1 and opens one LOW RP-SI-2 for stale module summaries at the parent.
The package reviewer read Sol's complete Round 2 verdict and the complete five-file correction.
The GitHub API confirms the current head, unchanged base, and PR body.
The delta changes only comments and documentation. It changes no executable code, mapping line, pending ID, pin, or gate setting.
All five changed summaries state both forms and retain same-run PASS. They resolve the text that RP-SI-2 identifies.
Package verdict: CLEAN (0 open), verdicts/pc-real-only-proofs.md, Round 3. RP-PC-1 stays closed.
Round 2 source evidence remains applicable to the unchanged executable code.
The green gate ran at parent c0f295ab. No gate receipt exists here for 46094d5b.
The reviewer ran no build, test, gate, mutation, reproduction, or executable merge check.
The separate OU-9 defect remains OPEN with P3. Package CLEAN does not close it.
Next: push this exact-head verdict and send its commit to PC, Sol, and the lead.
Sol's same-head integration delta remains required. After both CLEANs, PC merges current v1 once and runs the final exact-head gate.
This verdict covers no later head or merge. Wait for the doorbell. Never poll.
The original program-controls group remains incomplete.


Push record: PR #264 package CLEAN (0 open) is on origin/stage1/review-pc at f9875367aea4d5b2811ac62b2c93c94532892562.
Reviewed head: 46094d5b504861e59b66ac8cf16b399841d7ec71. Base: 010f2a49952f9b77581f304dbaa41006b115d40b.
Verdict: verdicts/pc-real-only-proofs.md, Rounds 2 and 3. RP-PC-1 is closed.
The final delta changes comments and documentation only. The supplied gate remains prior-head evidence at c0f295ab.
Sol's same-head integration delta and the final current-base exact-head gate remain required.
The separate OU-9 structural fix remains OPEN with P3. The reviewer worktree is clean.
Wait for the doorbell. Never poll.


Sol Round 3 CLEAN receipt: msg_plugin-w_1791733149_ef7519.
Source head: 46094d5b504861e59b66ac8cf16b399841d7ec71. Base: 010f2a49952f9b77581f304dbaa41006b115d40b.
Sol verdict: 294f42cb67387cc308fb139afaca6220054bcadb, verdicts/stage1/pc-real-only-proofs-implementation.md, Round 3.
The package reviewer read Sol's complete Round 3 and verified that origin/stage1/integration-review-sol contains the verdict commit.
RP-SI-2 is closed. RP-SI-1 stays closed. Both source verdicts are CLEAN with 0 open at the same head and base.
The lead's standing rule now permits the one current-v1 merge, its base check, and the final exact-head gate.
The final body record remains required. Neither verdict approves a later head or source transfer.
The c0f295ab gate remains prior-head evidence. The separate OU-9 structural fix remains OPEN with P3.
Wait for the doorbell. Never poll.


## PR #264: final gate and merged receipt

PC receipt: msg_plugin-w_1791734846_1e91a0.
Final gated head: 2f26928054d225875b866a40d3cfdc3a83820b3b. Base: 0e1f2feeac9e0aeb46d618b25dcb32f991fcfde9.
Reviewed head: 46094d5b504861e59b66ac8cf16b399841d7ec71. Package verdict f9875367; Sol verdict 294f42cb.
Current brief plan: 23bb. Sections 5 and 8 equal 23au. Section 0 records the own-trial decision.
The reviewer read the complete shared base-merge-check report: PASS. No source delta round is required.
The independent own-diff comparison matches its 85,312-byte SHA-256 0a93c082c309db2832ab3dc5359ab75bfc2fa3fc78e0152c0803b70ddf06eb4f.
The 18 changed base paths and 16 own paths have no overlap. Both pending files equal the final base.
History contains two imports: e363e24f imports d582853d; 2f269280 imports 0e1f2fee.
The PR body records the stopped first gate. The lead's 20:10 handoff directs the final import.
Final gate: gates/botster-core-stage1-pc-real-only-proofs-2f269280-pool-20261011-085825-53068.log.
SHA-256: d18ee68b43e1c5c3de937235ef24feb47f168beeeac06cd40dcc961f71601dc9.
All ten steps PASS in 492.8 seconds. Linux wrapper exit 0 after 503 seconds on msa1.
Default 1724 PASS, 481 skipped, 15.755 seconds. Slow 553 PASS, 2021 skipped, 43.410 seconds.
AD-6 own trial PASS in 0.209 seconds. Same-run result check names it and checks one proof.
Mutants: 83 identities, 81 caught, 2 unviable, 0 missed, 0 timeout. Stage 2 catches exactly the three stage-1 misses.
All 86 raw records and 81 named catch reports match. No FLAKY token exists in this gate.
The separate earlier OU-9 structural fix remains OPEN with P3.
GitHub and the lead's 20:40 handoff confirm PR #264 MERGED at a0adb1321bfc3deb7d16735e9a622775ba5f1a2d.
The merge commit and gated head share tree 8b603a9d4533360ab0ab2e89212aa4e71df45bbb.
The reviewer ran no build, test, gate, mutation, reproduction, or executable merge-check job.
The #264 final gate condition is closed. The original program-controls group remains incomplete.
Next: push this evidence record and notify PC, Sol, and the lead. Wait for the doorbell. Never poll.


Final evidence push: 34fbf8f0f98d28b03e301b3d29bc5297e469542a on origin/stage1/review-pc.
The package CLEAN at 46094d5b carries to final gated head 2f269280 under the verified base-only merge exception.
PR #264 is MERGED at a0adb1321bfc3deb7d16735e9a622775ba5f1a2d with the same tree as the gated head.
The final gate condition is closed. The original program-controls group remains incomplete.
The reviewer worktree is clean. Wait for the doorbell. Never poll.




Sol final-record receipt: msg_plugin-w_1791735134_fc45bc.
Sol read the complete final record at 2f269280 and independently verified disjoint paths, unchanged own diff, pending files, merge parents, and tree.
The package reviewer supplied the pushed receipt SHA 34fbf8f0f98d28b03e301b3d29bc5297e469542a to PC, Sol, and the lead.
Both reviewers ran no execution job. Wait for the doorbell. Never poll.


## Pending PC a2_3 review: steward ruling R-72

Lead receipt: msg_plugin-w_1791735614_f6a202.
Draft read: shared/steward/drafts/r72.md. The ruling governs PC's coming a2_3 Core PR, tier HIGH.
No READY, exact source head, or reviewable implementation arrived with this receipt. No implementation verdict exists for this work.
R-72 changes the classification premise: Core classifies what failed, not which edge reports first.
For a worker this host spawned, link EOF in AwaitHello, SendLaunch, or AwaitLaunched waits for the first applicable event:
- The worker exit report gives Lost(WorkerGone).
- The existing LC-4 startup deadline gives the link-lost Exited outcome and SessionEnded{exit: Other}.
The deadline keeps today's EOF/link-lost StartFailed reason. It must not become a startup-timeout reason.
The PR must state that reason before and after. The fix adds no timer.
The session stays Starting during the wait. EOF posts no event. Routes close after the state, including routes attached during the wait.
The ruling covers only workers this host spawned. Adopted workers and Running workers stay outside this ruling, with A21.
Required evidence: an exit-later test; an alive-to-deadline test; N >= 5 real runs of conf::a2_3_lost_session_closes_routes_session_lost; an A5-4 table with no PASS to non-PASS.
Apply orchestrate-delivery when reviewing this changed premise. Retain this existing handoff as the review record.
Next: wait for PC's exact-head READY and the required evidence. Never poll.


## PR #276: cause confirmation before READY

PC receipts: msg_plugin-w_1791736235_5c82eb and msg_plugin-w_1791736258_fee5b9.
Head: 67fd9112d0e7554d39be6eb6b43260c782dda200. Recorded gate base: a0adb1321bfc3deb7d16735e9a622775ba5f1a2d.
Current GitHub base: 42d7056e99ed0aaf2967c5ecc7609cb3c65e970f. Tier HIGH. Plan 23bj.
The first gate is RED on ou_9_baseline_then_live_no_gap at seed 0, step 14, in the unmutated stage-2 baseline.
Expected after1after2; received beforeafter1. The baseline has no retry. Fuzz did not run.
The e1_1 trial passes in the slow step and baseline, but the PR is not READY and has no package verdict.
The lead's 22:35 handoff approves one re-gate under 23al, conditional on this reviewer's cause confirmation.
The reviewer read the complete three-file own diff, PR body, pinned OU-9 transcript, and P3's diagnostic evidence.
Cause confirmation: the missing R-57 sentinel + await_title before attach permits before bytes to arrive as live output.
#276 changes only the e1_1 pending entry, its proof line, and worker_file_name capability with its existing test.
It does not alter that cause. The diagnostic worker files are unchanged from P3's v1 base to this PR's recorded base.
P3 reproduces the exact signature three times on v1 e4b6cdf5 with a local controlled-read patch.
The unmodified plain runs pass. No claim is made of failure on an unmodified current v1 tip.
The delayed-read evidence fails five times before the fence and passes five times with it. Contracts #49 owns the structural fix.
Record: verdicts/pc-e1-1-worker-file-name.md. Scope: cause confirmation only. Package review remains pending.
The reviewer ran no execution job or reproduction. The red gate remains red. No source CLEAN exists.
Next: publish the cause record and the requested PR comment, then wait for exact-head READY. Never poll.


Cause record pushed: 57e6cd64fa8a766fd61748b3945b157b8c4f01e6 on origin/stage1/review-pc.
PR #276 comment: https://github.com/trybotster/botster-core/pull/276#issuecomment-6111215443.
Head: 67fd9112d0e7554d39be6eb6b43260c782dda200. The comment confirms the unchanged missing-fence cause for the lead-approved one re-gate.
The evidence includes a controlled-read patch on the v1 source. The unpatched runs pass; no unmodified current-tip failure is claimed.
The first gate remains RED. No package CLEAN exists. The reviewer worktree is clean.
Wait for exact-head READY with the required gate evidence. Never poll.


## PR #276: package review round 1

READY: msg_plugin-w_1791737263_7d00b5. Tier HIGH.
Head: 67fd9112d0e7554d39be6eb6b43260c782dda200.
Parent and own-diff base: a0adb1321bfc3deb7d16735e9a622775ba5f1a2d.
PR base and green gate header base: 42d7056e99ed0aaf2967c5ecc7609cb3c65e970f.
Package verdict: CLEAN (0 open), verdicts/pc-e1-1-worker-file-name.md, Round 1.
The three-file change declares an existing real capability, removes only E1-1 from pending, and adds its own-trial proof line.
The reviewer read accepted Core erratum 1, the controls row, both real transcript paths, and the verified worker wrapper.
Both file names launch the real worker. The suite and same-run result check supply the approved #264 proof.
The contracts pin stays .36. No runtime, transcript, timeout, pin, mutation profile, or real-pending entry changes.
PC corrected the gate base in msg_plugin-w_1791737363_98c426. The current body also cites 23bk, which contains the 23bh approval.
The reviewer verified both corrections. No package finding or body note remains open.

Green gate: gates/botster-core-stage1-pc-e1-1-worker-file-name-67fd9112-pool-20261011-093607-9029.log.
SHA-256: 1d0d0ea7b0eebfe283f23bb7a7bfaadb8b95577901456cb3d6f54f58f528d5bf.
All ten steps PASS in 551.9 seconds. Linux wrapper exit 0 after 666 seconds.
Default 1724 PASS, 481 skipped, 17.980 seconds. Slow 554 PASS, 2020 skipped, 56.923 seconds.
E1-1 PASS in 0.477 seconds; same-run check names it and checks two proofs. Baseline E1-1 PASS in 0.466 seconds.
Counts: testkit 57/69; real-passing 47/68; real-accepted 48/69.
Two whole-body capability mutants miss in stage 1 and are caught in stage 2. Zero missed, timeout, unviable, or FLAKY outcomes remain.
The reviewer matched all four raw records and both catcher records. No execution job ran here.

Plan 23bl amendment (A) explicitly accepts the controlled timing-race diagnostic for #276.
Lead receipt: msg_plugin-w_1791737498_600484. The reviewer read the new pin and verified its SHA above.
The first gate stays RED. No unmodified current-v1 failure is claimed. Contracts #49 and its Core pin still own the OU-9 fix.
The earlier #264 OU-9 outcomes were FLAKY under mutants, not its stage-2 baseline. The formal record preserves this distinction.
Sol's same-head integration verdict remains required. The current head does not contain PR base 42d7056e.
After both CLEANs, PC merges current v1 once, posts the base-merge check, and runs one final exact-head gate.
This verdict approves no later source delta. The original program-controls group remains incomplete.

## PR #277: READY queued for review

READY: msg_plugin-w_1791737722_0fd975. Tier HIGH.
Head: 7d7d429f06335c18e8a21e6c63085be4c5465eb0. PR and gate base: 42d7056e99ed0aaf2967c5ecc7609cb3c65e970f.
PC reports a green gate and R-72 evidence. No package source review or verdict exists yet.
Next: publish #276 CLEAN. Then read the brief first and review #277. Never poll.


Push record: #276 package CLEAN (0 open) is on origin/stage1/review-pc at fd6eeeb3343a7768b8550c872139a85cac308757.
Exact source head: 67fd9112d0e7554d39be6eb6b43260c782dda200. Parent/own base a0adb132; PR/green gate base 42d7056e.
Sol's integration verdict and the final current-base gate remain required. No future source delta is approved.
The reviewer worktree is clean. Next: review #277 READY. Never poll.


## Plan 23bo receipt and #276 historical correction

Lead receipt: msg_plugin-w_1791737924_13369e. The reviewer read the complete changes from 23bl to 23bo.
23bm permits a scratch diagnostic in the worker binary that only changes timing and supplies no missing behavior.
23bo corrects both references to #264: two OU-9 FLAKY outcomes under mutants, not an unmutated-baseline failure.
The #276 verdict already preserves the correct historical distinction. Its exact-head package CLEAN remains fd6eeeb3.
The controlled diagnostic stays scratch-only. Contracts #49 and the Core pin still own the structural fix.

## PR #277: package review round 1

READY: msg_plugin-w_1791737722_0fd975. Tier HIGH. Contracts pin .36, unchanged. Plan 23bo.
Head: 7d7d429f06335c18e8a21e6c63085be4c5465eb0. Parent: f46a1fff431f30851d642447ebc32b3e5ef5425d.
Own-diff, PR, and gate header base: 42d7056e99ed0aaf2967c5ecc7609cb3c65e970f.
Package verdict: CLEAN (0 open), verdicts/pc-a2-3-lost-before-running.md.
The reviewer read the brief first, BUILD.md, R-46 to R-50, R-72, the design note, and the complete eight-file diff.
R-72 changes the classification premise. The reviewer applied the previously read orchestrate-delivery skill and kept this handoff.
LinkLost waits in Starting for the worker exit or the existing startup deadline. EOF posts no event.
The exit gives WorkerFailed and Lost(WorkerGone). The deadline gives the old WorkerFailed/Exited(Other) link-failure outcome, without a signal.
Adopted and Running link loss retain their existing paths. SendLaunch with no link covers the early-hello/identity-row path.
The unchanged failed-start path closes every attached route after the state, once, with that state's reason.
The reviewer read the four relevant engine tests and traced the stated reverts without executing them.
The pending file removes only a2_3. No transcript, timeout value, pin, worker, unsafe code, or mutation profile changes.
The Prior art note rejects the old immediate-EOF behavior. The reviewer read the old implementation and named test.
The body lists six other classification places. Their separate gaps remain outside this verdict.

Mac evidence: six fixed a2_3 runs PASS; three immediate-failure controls FAIL at seed 0, step 29.
A5-4: all four tables and raw logs contain 690 unique results; each raw result agrees with the table.
Testkit before/after: 209 PASS, zero FAIL, 481 ignored; identical tables.
Real before: 179 PASS, one FAIL, 510 ignored. Real after: 180 PASS, zero FAIL, 510 ignored.
Only a2_3 changes FAIL to PASS. No PASS becomes non-PASS.

Gate: gates/botster-core-stage1-pc-a2-3-lost-before-running-7d7d429f-pool-20261011-094729-72318.log.
SHA-256: d034fd8d28a3c205f3ad496eb0c25318ee8641d0c670b40f1712d08ab7334548.
All ten steps PASS in 316.6 seconds. Linux wrapper exit 0 after 434 seconds.
Default 1727 PASS, 481 skipped, 12.342 seconds. Slow 554 PASS, 2021 skipped, 42.168 seconds.
Testkit a2_3 PASS in 0.231 seconds at line 557. Real a2_3 PASS in 0.207 seconds at line 4042.
Six mutants are caught in stage 1. Stage 2 does not run. Zero missed, timeout, unviable, or FLAKY outcomes.
The reviewer matched all six raw records and named catchers. No execution job ran here.
Counts: testkit 56/69; real-passing 47/68; real-accepted 48/69. This head does not contain #276's E1-1 proof.
Sol's same-head integration verdict remains required. Current v1 has an overlapping pending-list path.
After both CLEANs, PC merges current v1 once, posts the base-merge check, and runs one final exact-head gate.
No later source delta or conflict resolution is approved. The original program-controls group remains incomplete.
Next: publish the verdict and notify PC, Sol, and the lead. Wait for the doorbell. Never poll.


Push record: #277 package CLEAN (0 open) is on origin/stage1/review-pc at 0f4d349fcf6cd4b681bd064264a705121877ec55.
Exact source head: 7d7d429f06335c18e8a21e6c63085be4c5465eb0. Own/PR/gate base: 42d7056e99ed0aaf2967c5ecc7609cb3c65e970f.
Sol's same-head integration verdict and the final current-base merge/check/gate remain required.
#276 package CLEAN remains fd6eeeb3343a7768b8550c872139a85cac308757 on head 67fd9112d0e7554d39be6eb6b43260c782dda200.
The reviewer worktree is clean. Wait for the doorbell. Never poll.


## PR #276: Sol source CLEAN receipt

Receipt: msg_plugin-w_1791740907_873ba1.
Sol verdict: 5e1a9be28abac3fa274f38a9f59ff82cea8ce25c, verdicts/stage1/pc-e1-1-worker-file-name.md, Round 1.
The reviewer read the complete verdict and verified publication on origin/stage1/integration-review-sol.
Both source CLEANs have zero open findings on head 67fd9112d0e7554d39be6eb6b43260c782dda200.
Package verdict: fd6eeeb3343a7768b8550c872139a85cac308757.
Own base: a0adb1321bfc3deb7d16735e9a622775ba5f1a2d. PR and green gate base: 42d7056e99ed0aaf2967c5ecc7609cb3c65e970f.
The same-head integration-review condition is closed. The final current-v1 import, applicable base check, and exact-head gate remain due.
Neither verdict approves a later source delta or gate. The OU-9 structural fix still needs its Core pin.
No execution job ran here. Wait for the doorbell. Never poll.


Receipt record pushed: b9d4795455d4b43f0b3745a613aeb4d7afb00e4b on origin/stage1/review-pc.
#276 both source CLEANs cover 67fd9112d0e7554d39be6eb6b43260c782dda200. The final import, applicable base check, and gate remain due.
The reviewer worktree is clean. Wait for the doorbell. Never poll.


## PR #276: package delta review after the base merge

PC receipt: msg_plugin-w_1791740971_556be3.
Merged head: 300eccec3f9c2929e137ee33eb2e8cbe45bf66a8.
First parent: reviewed 67fd9112d0e7554d39be6eb6b43260c782dda200.
Second parent, imported v1, own-diff and PR base: 84b1e9be2c58044b27958ed39fe1fffa2a0eb907.
Package verdict: source CLEAN (0 open), verdicts/pc-e1-1-worker-file-name.md, Round 2.
The reviewer read the brief first and the complete 23bo-to-23bt changes. Sections 5 and 8 remain unchanged.
The base-check report remains FAIL. The proof file conflicted, real.rs overlapped, and blob identities/context changed.
Report: evidence/base-merge-check-276-300eccec.out. SHA-256 d8df10a3da9398056b7134f4693950caf4e65f2a117bb253a414fc1fe25ed6dc.
Both pending-file checks pass. The disjoint-base exception does not apply. This round supplies the package delta review.
The reviewer verified both merge parents, the complete three-file own diff, current GitHub head/base/body, and imported shared-file changes.
The three own paths retain byte-identical added/removed lines. The pending file equals the base minus E1-1.
The real-pending file equals the base. The proof file preserves the base plus exactly one E1-1 row, including both LC-2 rows.
The merged capability expression keeps the tapped guard, PC's worker_file_name arm, and #275's oracle-control term.
The test keeps both control sets. The imported oracle names do not intercept E1-1.
Candidate validation, guard wrapper, E1-1 suite path, and proof checker are unchanged from the reviewed head.
The imported real.rs changes leave open, worker wrapping, and binary selection unchanged. The pin and transcript are unchanged.
No package finding remains open. Sol's same-head delta review is required because the base check failed.
PC reports that the final gate is running. No completed gate receipt exists here for 300eccec. The 67fd9112 gate is prior-head evidence.
Before merge, the PR body must record the merge, failed base check, both delta verdicts, and final exact-head gate.
This source verdict approves no other base import and does not establish the final gate condition.
The reviewer ran no execution job. The original program-controls group remains incomplete.
Next: push the source verdict and notify PC, Sol, and the lead. Wait for the doorbell. Never poll.


Source delta verdict pushed: ce5d6b6d1de679907a29b54e1330a7923bfb5625 on origin/stage1/review-pc.
#276 source CLEAN (0 open) covers 300eccec3f9c2929e137ee33eb2e8cbe45bf66a8, own/PR base 84b1e9be2c58044b27958ed39fe1fffa2a0eb907.
Sol's same-head delta verdict, final exact-head gate, and completed PR body record remain due.
The reviewer worktree is clean. Wait for the doorbell. Never poll.


## PR #277: Sol source CLEAN receipt

Receipt: msg_plugin-w_1791741176_90ec42.
Sol verdict: 2587b8498e51f36e034c8655ae2013be35e08ff1, verdicts/stage1/pc-a2-3-lost-before-running.md, Round 1.
The reviewer read the complete verdict and verified publication on origin/stage1/integration-review-sol.
Both source CLEANs have zero open findings on 7d7d429f06335c18e8a21e6c63085be4c5465eb0, base 42d7056e99ed0aaf2967c5ecc7609cb3c65e970f.
Package verdict: 0f4d349fcf6cd4b681bd064264a705121877ec55.
The compact comparison logs do not independently prove the scratch checkout's provenance. Engine reversals remain PC's body reports.
Neither reviewer executed those reversals. The retained status comparison and exact-head gate support the recorded source verdicts.
The same-head integration-review condition is closed. The current-v1 import, applicable base check, and final exact-head gate remain due.
Neither verdict approves a later source delta or gate. No execution job ran here.
#276 remains source CLEAN at merged head 300eccec; its Sol delta and final gate receipt remain pending here.
Next: push this receipt record and notify PC, Sol, and the lead. Wait for the doorbell. Never poll.


Receipt record pushed: 2f3731498e6058dd20961b922f73c1819c5cbf5b on origin/stage1/review-pc.
#277 both source CLEANs cover 7d7d429f06335c18e8a21e6c63085be4c5465eb0. The final import, applicable base check, and gate remain due.
The reviewer worktree is clean. Wait for the doorbell. Never poll.


## PR #276: final gate and merged receipt

PC receipts: msg_plugin-w_1791741230_09615a and msg_plugin-w_1791741295_ba72c7.
Sol delta receipt: msg_plugin-w_1791741249_720a00. Verdict b1eeeac1a1cb6e7f6a1cec63fb84104273d32c48, Round 2.
The reviewer read Sol's complete delta and verified remote publication. Both delta CLEANs cover 300eccec3f9c2929e137ee33eb2e8cbe45bf66a8.
Base: 84b1e9be2c58044b27958ed39fe1fffa2a0eb907. Package delta verdict: ce5d6b6d1de679907a29b54e1330a7923bfb5625.
The failed base check remains FAIL. Both explicit source reviews cover the overlap and resolution.
Final gate: gates/botster-core-stage1-pc-e1-1-worker-file-name-300eccec-pool-20261011-104914-15924.log.
SHA-256: 4acad31348c3a2f422e4e8af36672b3451a5a8fbffa146be86d1127997128bf0.
All ten steps PASS in 211.4 seconds. Linux wrapper exit 0 after 223 seconds.
Default 1738 PASS, 478 skipped, 11.493 seconds. Slow 557 PASS, 2028 skipped, 40.528 seconds.
E1-1 PASS in 0.144 seconds. The same-run result check names it at line 4888; all four named proof rows pass.
Baseline E1-1 PASS in 0.165 seconds. Counts: testkit 60/69; real-passing 48/68; real-accepted 49/69.
Stage 1 misses two capability mutants; stage 2 catches both. Zero missed, timeout, unviable, or FLAKY outcomes.
The reviewer matched four raw records with two named catchers. The fuzz step is PASS, with no changed decoder crate.
PC corrected the LOW body note that called fuzz skipped. The reviewer verified the body through the GitHub API.
The body now includes the one base import, failed check, both delta verdicts, and exact-head gate. All final review conditions close.
GitHub reports PR #276 MERGED at 20c4e98df6b47a45f3e2284a976e1c8a1341457e.
The reviewer verifies the merge and gated head share tree ca4b8722aebb4e0a737b902aa4776c4681a8d966.
No execution job ran here. The earlier red gate stays red. The OU-9 structural fix still needs its Core pin.
The original program-controls group remains incomplete.

## PR #277: merged-head delta queued

PC receipt: msg_plugin-w_1791741278_38410f.
Head: 79277df141aa6c5b18f150ed47015bd733b5efc1. Imported v1: 84b1e9be2c58044b27958ed39fe1fffa2a0eb907.
PC reports one conflict-free import and a failed pending-file check. The final gate is running.
No merged-head package verdict exists yet. Next: publish #276 final evidence, then read the brief first and review #277. Never poll.


Final evidence pushed: 994a79ead842dbe904d06cf9d616e7b202049c4e on origin/stage1/review-pc.
#276 is MERGED at 20c4e98df6b47a45f3e2284a976e1c8a1341457e with the same tree as final gated head 300eccec.
Both source delta CLEANs, final gate, and body conditions are closed. The original program-controls group remains incomplete.
The reviewer worktree is clean.
PC receipt msg_plugin-w_1791741344_7d121d supersedes #277 head 79277df1 with 2c78bb6c8424d37089062437731980ec0d34eab0.
PC reports that the lead directed the second import of v1 20c4e98d and stopped the 79277df1 gate. The new final gate is running.
Next: read the brief first and review #277 on the new head. Never poll.


## PR #277: Round 2 package CLEAN on final head

Head: 2c78bb6c8424d37089062437731980ec0d34eab0.
Own/PR/final-gate base: 20c4e98df6b47a45f3e2284a976e1c8a1341457e.
Plan: 23bu, stage1-plan.133ea0b2.md; SHA-256 86e0b9ef3e16b3b485a52508ec4da86affcecbc85deb88f22318160582c5fdcb.
The reviewer read the brief first. The original source CLEANs cover 7d7d429f only.
Actual history: import 84b1e9be -> 79277df1, then lead-directed import 20c4e98d -> 2c78bb6c.
The lead stopped the stale-base 79277df1 gate. It supplies no completed evidence.
The lead assigns #277 the final-gate slot under plan 23bu.
Base check FAIL (4) on core-real-pending; all other checks pass. Both explicit delta reviews are required.
The independent own diff comparison is byte-identical outside the two line-set files: 15231 bytes.
SHA-256 f330332690061aad54448cce4aabee47a9d401e0830ee34efc6b80a7b17f1819.
All seven source/test blobs are unchanged. All changed base paths outside the own diff preserve the base blobs.
core-pending equals the base. core-real-pending equals the base minus only a2_3.
The head preserves the three added base IDs and their comments. R-72 behavior and Round 1 evidence limits remain unchanged.
Final gate: gates/botster-core-stage1-pc-a2-3-lost-before-running-2c78bb6c-pool-20261011-105545-34013.log.
SHA-256 00ac6621cdd278fa7836f913156836da5423592010e4693ae257231a2c44c904.
All ten steps PASS in 136.3 seconds; Linux job and wrapper exit 0 after 144 seconds on msa1.
Default: 1740 PASS, 478 skipped, 11.580 seconds. Slow: 558 PASS, 2027 skipped, 41.432 seconds.
a2_3 testkit PASS 0.209 seconds; real PASS 0.161 seconds. All four relevant engine tests pass.
Counts: testkit 60/69; real-passing 49/68; real-accepted 50/69.
Stage 1 catches all six mutants; stage 2 does not run. Zero missed, timeout, unviable, or FLAKY outcomes.
Six raw identities match six named catcher records. Two catcher names differ from the original gate.
PC corrects the LOW same-tests body claim; receipt msg_plugin-w_1791741692_a9715e. GitHub API confirms the correction.
The body records both imports, stopped job, failed check, and final gate. LOW PC-277-BODY-1 is closed.
The lead assigns the three other pending-PASS IDs to P5's pin PR. This verdict closes none of that separate work.
Package CLEAN: zero open findings. Sol's same-head delta CLEAN and both delta verdict references in the body remain due.
The gate base must remain the v1 tip at merge. No execution job ran here.
The original program-controls group remains incomplete. Wait for the doorbell. Never poll.

Round 2 verdict pushed: 5ee8cfb2ef82e5e68d3f9504590a868408e53913 on origin/stage1/review-pc.
VERDICT: CLEAN (0 open) on #277 head 2c78bb6c8424d37089062437731980ec0d34eab0.
The reviewer worktree is clean. Sol delta and final body verdict references remain due. Never poll.


#277 Sol Round 2 CLEAN receipt: msg_plugin-w_1791741951_bbd121.
Sol verdict: 64b9e757bc39334ce2c672fdbde5d262e5e031be, read in full and verified on origin/stage1/integration-review-sol.
Both delta CLEANs cover final head 2c78bb6c8424d37089062437731980ec0d34eab0 and base 20c4e98df6b47a45f3e2284a976e1c8a1341457e.
The same-head integration-review condition is closed. The body has the package verdict and still says Sol requested.
GitHub reports #277 OPEN. PC must add Sol's verdict before merge. Q7 still applies at merge.
No execution job ran here. Wait for the doorbell. Never poll.

Sol Round 2 receipt record pushed: a1e13479587671bdf4bcec99d66bfbc421568fca.
The worktree is clean. PC must add Sol verdict 64b9e757 to #277 body before merge. Never poll.


## PR #277: final conditions closed and MERGED

PC final body receipt: msg_plugin-w_1791742011_98c533.
The GitHub API confirms both delta verdicts in the body and PR #277 MERGED.
Merge: 9f7085f703457b0669785697dbeb14ac687a8942.
Parents: final gate base 20c4e98df6b47a45f3e2284a976e1c8a1341457e and gated head 2c78bb6c8424d37089062437731980ec0d34eab0.
The reviewer verifies that both merge and gated head share tree fed65ce895b71a25f22d1c9ac82d3344e9952580.
The tree diff is empty. The merge's first parent equals the final gate base; the recorded Q7 condition closes.
Both delta CLEANs, final gate, and body conditions close. The package verdict remains CLEAN, zero open findings.
The original program-controls group remains incomplete. The lead owns triage of the six other R-72 classification places.
No execution job ran here. Wait for the doorbell. Never poll.

Final #277 record pushed: d28b50691c644196856bc8fee44ed3efc39bdff6 on origin/stage1/review-pc.
All #277 review conditions close. The worktree is clean. The original group remains incomplete. Never poll.


## PR #283: R-74 testkit part, Round 1 NOT CLEAN

READY receipt: msg_plugin-w_1791743029_b2452a.
Head: 646ebe7321cc5a5de46f15d9aa6b61028ffec1e5.
Parent, own/PR/gate base: 9f7085f703457b0669785697dbeb14ac687a8942. HIGH, shared testkit crate.
The lead approves PART A at 04:56. PART B is PC's .40 pin PR after #282 and the tag.
The reviewer reads the brief first, then the binding rules, R-74, plan through 23bx, complete diff, and applicable source paths.
NOT CLEAN: two open findings in verdicts/pc-held-write-testkit.md.
HW-TK-1 MEDIUM: the spent pty_chunk cap sets no next-step marker while blocked with remaining kernel room.
HW-TK-2 LOW: settle discards completion events before the test checks the held interval.
PC receives both findings in msg_plugin-w_1791743210_dce502 before publication.
Gate: gates/botster-core-stage1-pc-held-write-testkit-646ebe73-pool-20261011-111204-3662.log.
SHA-256: 6ad44b5a5fa5efd7129a5ac9de1535bb96efe1bd8030eb7f7fbc313cf37b658f.
Ten steps PASS in 554.7 seconds; Linux job/wrapper exit 0 after 668 seconds.
Default: 1745 PASS, 478 skipped, 18.687 seconds. Slow: 558 PASS, 2032 skipped, 57.292 seconds.
All five new tests pass. Stage 1 catches 28/28 mutants; raw identities match named catchers; zero FLAKY outcomes.
The green gate closes neither finding. No reviewer execution job runs.
No pending ID, pin, transcript, timeout, or real-tier control changes. The group remains incomplete.
Next: wait for PC's fixes and exact-head gate evidence. Never poll.

#283 Round 1 verdict pushed: f91046e143764ff17504b66a355350804f346b7f on origin/stage1/review-pc.
VERDICT: NOT CLEAN (2 open) on 646ebe7321cc5a5de46f15d9aa6b61028ffec1e5.
The worktree is clean. Wait for fixes and the doorbell. Never poll.


## PR #283: Round 2 source CLEAN; gate pending

Fix receipt: msg_plugin-w_1791743341_c41c67.
Head: bd4c6f16c9c1435252f929663a33ea8b28b4beb5; parent 646ebe7321cc5a5de46f15d9aa6b61028ffec1e5.
The brief is read first again. The complete two-file delta closes both findings by source review.
HW-TK-1: next-step guard includes blocked kernel room; full queue and spent accept limits still retain no next-step work.
The new control test pumps until idle and observes three queued bytes, no early completion, then Written 5 after release.
HW-TK-2: settle returns all events that each pump's poll returned, including the final idle pump.
Both held-write tests inspect retained events before release. Existing ignored-return sites retain prior behavior.
PC reports 394 local testkit passes, 212 conformance passes, and two red-on-revert failures. No raw reversal log is supplied.
No reviewer execution job runs. Source CLEAN: zero open findings on bd4c6f16.
The gate on 646ebe73 covers only the original head. PC queues the new gate after its contracts job.
Exact-head gate and mutation results, Sol verdict, and updated body remain due. The final slot/import/Q7 conditions remain due.
The original group remains incomplete. Wait for the doorbell. Never poll.

#283 Round 2 source verdict pushed: 123f0fe495f8550a7ef2c4dfcf9bbf6bbc7f74e1.
Source CLEAN, zero open findings, on bd4c6f16c9c1435252f929663a33ea8b28b4beb5.
The worktree is clean. Exact-head gate and Sol verdict remain pending. Never poll.


#283 Sol Round 1 receipt: msg_plugin-w_1791743437_932a8d.
Verdict 951091b9b7bec3b686731eaafc98e247b02f87be is read in full and verified on origin/stage1/integration-review-sol.
Sol independently confirms both package findings on original head 646ebe73. Sol adds no separate finding.
Package source CLEAN remains on fixed head bd4c6f16. Sol's fixed-head review and exact-head gate remain pending.
No execution job runs. Wait for the doorbell. Never poll.

Sol Round 1 receipt record pushed: 7a620e6795abedb3012d5ed3f3f86f3443407f2b.
The worktree is clean. Fixed-head gate and Sol review remain pending. Never poll.


#283 Sol Round 2 source CLEAN receipt: msg_plugin-w_1791743918_7be331.
Verdict d9bdb2db873eb6f11f6eae54c44f2848a5c98cf1 is read in full and verified on origin/stage1/integration-review-sol.
Both source CLEANs cover bd4c6f16c9c1435252f929663a33ea8b28b4beb5, base 9f7085f703457b0669785697dbeb14ac687a8942.
Both source findings close. Same-head integration source-review condition closes.
Exact-head gate evidence, body verdict references, final slot/import/check/gate, and Q7 remain due.
Neither reviewer accepts a running gate, later import, .40 pin, or real-tier result. Never poll.

Sol Round 2 receipt record pushed: 08e43fee1dbf41feb8c1eff1ad37177396fc2db9.
The worktree is clean. Both source reviews are CLEAN on bd4c6f16. Gate and final merge conditions remain due. Never poll.


## PR #283: fixed-head gate RED at lists; both source CLEANs remain

PC receipt msg_plugin-w_1791743932_aa5b95.
Log: gates/botster-core-stage1-pc-held-write-testkit-bd4c6f16-pool-20261011-113530-95097.log.
SHA-256: 4208ebc4b8ac166e5e7686c6f87daa02940f462b9f9b96f4acd27090116fbd49.
Gate head bd4c6f16c9c1435252f929663a33ea8b28b4beb5; gate base 926dfb62f3f7168990b2a48741946932d84b2fbb.
Own/PR base stays 9f7085f703457b0669785697dbeb14ac687a8942.
Fmt/clippy/taint PASS. Lists FAIL because the later base removes four IDs retained in the PR's old-base pending file.
The reviewer independently verifies those four removals and an empty own diff under conformance/.
The source change does not alter the cause. Both source CLEANs remain on bd4c6f16.
The six later steps are NOT RUN. No exact-head mutation evidence exists. The earlier 28 catches cover only 646ebe73.
CI steps total 75.6 seconds; Linux job/wrapper exit 1 after 160 seconds on msa1.
PC asks the lead about the final-slot/import order. No reviewer authorization for an import or same-head retry is given.
An approved import that removes this cause is a source repair under plan 23an(b), with applicable review/check/new-head gate.
Retain the red log and cause in the body. Never poll.

#283 red-gate cause record pushed: 9609f88011fe6f6fc6c8c6669eee91dbf5fa896a.
Both source CLEANs are verified. The worktree is clean. The lead must settle the slot/import order. Never poll.


#283 Sol red-gate audit receipt: msg_plugin-w_1791744194_c01d54.
Record 3b907b883df1ae34b1406279290b07e1113de496 Round 3 is read and verified on origin/stage1/integration-review-sol.
Sol agrees with the red cause and six NOT RUN steps. Both source CLEANs remain on bd4c6f16.
Lead decision 07:38: FINAL-READY now, final-gate SLOT after #282. No separate same-head retry is required.
Applicable import review/check, final green gate with mutation evidence, and body remain due.
Plan 23by delta is read and hash-verified. It changes no R-74 queue or final-slot rule.
No execution job runs. Wait for the doorbell. Never poll.

Final-slot decision record pushed: c3f923f45dbe5b51a517f554c2b7864ea99221f8.
The worktree is clean. #283 waits for the lead slot after #282. Both source CLEANs remain. Never poll.

## #283 B13 evidence replacement

PC receipts: msg_plugin-w_1791746051_50c162 and msg_plugin-w_1791746172_38a937.
The reviewer reads B13, the live body, the replacement logs, their summaries, failed assertions, and the revert patch.
The head stays bd4c6f16c9c1435252f929663a33ea8b28b4beb5. Both source CLEANs remain.
The logs show unit 394/0, conformance 212/0 with minimum 60/69, guard revert 392/2, and restoration 394/0.
The revert fails the two named HW-TK-1 tests. The pump test observes one queued byte instead of three.
The verdict records each log's SHA-256. The revert and restoration logs show compilation from this PR's worktree.
PC reports a clean, exclusive use of the target directory after it, and a matching dep-info inventory.
The clean has no raw log. The reviewer does not independently verify that provenance and runs no executable check.
The old pool gate remains valid for its original head. The fixed-head gate stays RED with six later steps NOT RUN.
PC confirms that the pre-split six-transcript claim is invalid under B13 in msg_plugin-w_1791746265_3ec208.
The reviewer verifies that the live body marks it invalid. PART B must supply valid evidence for both tiers.
The evidence question msg_plugin-w_1791746248_58e78a is closed. This adds no source finding.
The final-slot import, applicable review/check, final green gate, and body remain due. Never poll.

B13 evidence record pushed: 70518f70b4d217d19c1cefd1adcc07be14cb6374.
The worktree is clean. Source CLEAN remains; the final-slot evidence remains due. Never poll.

## #283 final-slot import: source CLEAN carries

PC receipt: msg_plugin-w_1791750654_adad67.
New head: 4daf627b54739f847dd2448bf660a6a45c1b714a.
Parents: bd4c6f16c9c1435252f929663a33ea8b28b4beb5 and 926dfb62f3f7168990b2a48741946932d84b2fbb.
The live PR has that head and imported base. PC reports the lead moved #283 before blocked #282.
The reviewer reads the base-check output: all four checks PASS, including no conflict.
Output SHA-256: c1759610f3bd56aa59dbb847a6786e9bba88318612ee534046b33237da924222.
Independent read-only comparisons confirm the unchanged canonical patch: 19815 bytes, SHA-256 5caf01f561face85debe54c9da930443c977eace09e66d77474d839274f1ed00.
The 11 base paths and three own paths are disjoint. All three own blobs and all 11 changed base blobs are preserved.
Both pending files now equal the base and have no own change. Source CLEAN carries; no source delta round is required.
Plan 23by-to-23cm delta is read and hash-verified. It changes no R-74 queue behavior.
The final gate is reported running. No completed log is read. Sol's import receipt, the final green gate, body, and Q7 remain due.
No executable check runs here. No final merge approval is given. Wait for the doorbell. Never poll.

Final-import source verdict pushed: c28d75fbb2d382eabada3c7abadb2584a9e7a4e7.
The worktree is clean. Source CLEAN carries to 4daf627b. Final-gate evidence remains due. Never poll.

## #283 final gate GREEN; exact gated tree merged

PC receipt: msg_plugin-w_1791751041_4ae590.
Final gate head 4daf627b54739f847dd2448bf660a6a45c1b714a; base 926dfb62f3f7168990b2a48741946932d84b2fbb.
Log: gates/botster-core-stage1-pc-held-write-testkit-4daf627b-pool-20261011-133054-46680.log.
SHA-256: 4031f5cf739a27eb403a0c8407919855e86b2ca6c4ac5bd7e954bf3770de37cb.
All ten steps PASS. CI steps total 343.6 seconds; Linux job/wrapper exit 0 after 353/354 seconds on msa1.
Default: 1754 PASS, 474 skipped. Slow: 559 PASS, 2037 skipped. Testkit minimum 61/69; real passing 49/68, accepted 50/69.
All 33 raw caught identities match the named catchers. No misses, timeout, unviable, or FLAKY line.
All five fixed-guard mutants are caught by the next-step test. The host-pump test passes and catches the pty_queued mutant.
Stage 2 does not run because Stage 1 catches all mutants. Fuzz PASS reports no changed decoder harness.
The body records final evidence, both import carries, the earlier red gate, and invalid pre-split evidence.
Sol's import carry is reported in the body; its message is not yet read here. Receipt requested in msg_plugin-w_1791751076_1d3a98.
GitHub reports MERGED at 2026-10-11T20:37:42Z, commit 670e5fc799a25794910a1847fc41b3e089bf03f4.
The reviewer verifies parents gate base + gated head, identical tree fcfb3382e8c6c8633d152eb3bd7f2d6a151440d4, and empty diff.
Q7's base and tree condition is verified. Package review and final-gate audit are complete; Sol's message receipt remains due.
PART B and later PC work remain separate. No execution job runs here. Never poll.

Final-gate and merge audit pushed: 3ba3c6210894bead75071fbc38e8d5a341495a77 (content commit b702b839e92d6a5698e96c98acba3c02452db4e3).
The worktree is clean. #283 is merged; only the Sol message receipt remains due in this cross-review record. Never poll.

## #283 Sol carry receipt closes; one LOW body correction remains

Sol receipt msg_plugin-w_1791751228_6c74ec names its carry message to PC, msg_plugin-w_1791750796_13bf5c.
Sol confirms both parents, 11 base paths, three source blobs, equal own patches, and both pending files.
Source carry is valid. The missing-message condition is closed. Sol's committed receipt is not yet read.
HW-TK-3 LOW is open: after the completed final-slot section, the body still instructs a future import and final gate.
The reviewer independently verifies that stale paragraph and asks PC to remove it or mark it historical.
Source CLEAN, green gate, and exact-tree merge remain valid. This body correction needs no source change or job.
Wait for the correction and doorbell. Never poll.

Sol carry/body finding record pushed: 650d2d9aff018a9efeabe5946812c581e0fa7c73.
The worktree is clean. HW-TK-3 LOW remains open; source CLEAN and merged gate result remain valid. Never poll.

## #283 review complete

PC correction msg_plugin-w_1791751288_946d46 is verified in the live body. HW-TK-3 closes.
Sol final record 4ad5e8d5b669134eba7db24b7d1f3f1cffc8348f is read and verified on its remote review branch.
Sol receipt msg_plugin-w_1791751368_16378a confirms import carry, final gate, exact-tree merge, and its body closure.
Both reviews are CLEAN on 4daf627b54739f847dd2448bf660a6a45c1b714a. No #283 review condition remains open.
PART B and R-78 design remain separate. No execution job runs here. Never poll.

## R-78 design review before code

Receipt: msg_plugin-w_1791751282_685365. Base: 670e5fc799a25794910a1847fc41b3e089bf03f4. Plan 23cm.
Design verdict: NOT CLEAN (4 open), verdicts/pc-r78-design.md.
R78-D1 MEDIUM: the driver flushes before reading and closes on send failure, which can drop buffered Exited despite engine retention.
R78-D2 MEDIUM: the plan needs a named real-process proof and the required A5-4 table, with accurately scoped reversal evidence.
R78-D3 LOW: the design needs the required Prior art note.
R78-D4 MEDIUM: the declared zombie limit is rejected by lead answer msg_plugin-w_1791751430_24e8ed.
Use nonblocking OS process state without a second waitpid. Cover reaped, zombie, live broken-link, and deadline-first queued-exit cases.
The reviewer traces the host engine, driver, identity check, and reaper. Orchestrate-delivery applies to the lifecycle and ordering premise.
PC has the findings and lead answer. No source change, executable result, or gate is accepted. Wait for the corrected design. Never poll.

## R-78 design revision 2: two findings remain

PC receipt msg_plugin-w_1791751569_e73078 is read. R78-D2 evidence plan and R78-D3 Prior art close at design level.
R78-D1 remains MEDIUM: the engine must learn write-side failure while the driver retains readable input.
A live peer with send failure and no EOF can otherwise leave Stop incomplete after its dropped Kill and cleared deadline.
R78-D4 remains MEDIUM: lead follow-up msg_plugin-w_1791751699_3263fb requires an explicit macOS SZOMB guard if proc_pidinfo succeeds.
B9 and the Mac failure summary are read. They establish the current refusal path, not removal of the conditional status guard.
Sol independently confirms both remaining issues. Its initial verdict a732b58ee2b32d6a2205a8f562bb80d99cbb395d is read.
No source or executable evidence is accepted. Wait for revision 3 and the doorbell. Never poll.

Publication recovered after the lead and Sol report successful GitHub access.
The push publishes #283 closure 2c14f77fa511651a84affe7884f32aaece359209 and initial R-78 verdict 2750f8d663bf42c98b3c33c68e20c64921bfd516.

Revision-2 design verdict pushed: 8e96a402a2aaf1567339dba6646d5631727a8790.
The worktree is clean. #283 is fully CLEAN and closed; R-78 design has two open MEDIUM findings. Never poll.

## R-78 added write-failure section: confirmed-gone drain needs priority

Sol receipt msg_plugin-w_1791751806_761675 and verdict 151bd294072d9f91f67dca9edd53813fa54ca86f are read.
Publication on Sol's remote branch is verified. The complete added LinkWriteFailed design is read.
The live-peer omission is addressed at design level. R78-D1 still requires a known-gone drain before classification or inflight failure.
The combined proof must cover ProcessExited, failed send, budget-delayed Exited/Done, grace before EOF, and the reverse report/failure order.
R78-D4 remains until the note replaces its obsolete optional/no-needed macOS guard text with the lead's mandatory guard.
Sol counts one drain finding and treats the guard as a required source condition. The package keeps the contradictory design text open.
PC has both corrections. No source or executable result is accepted. Never poll.

Added-section audit pushed: f0f491cce365bcac825772a2de065d83493badc5.
The worktree is clean. Two design findings remain. Wait for the correction and doorbell. Never poll.

## R-78 revision 3: design CLEAN

PC receipt msg_plugin-w_1791751957_aa5f71 is read. The complete design hash is verified:
52b9ada3f73a1fdf77c9f05b04775f2fffb4b6b5385f72149db77c6f99462885.
Base remains 670e5fc799a25794910a1847fc41b3e089bf03f4. No open design finding remains.
R78-D1 closes: confirmed death retains readable input before classification or inflight failure, in both report/failure orders.
The live Matches branch keeps the existing grace completion. Descriptor ownership and combined budget/grace tests are specified.
R78-D4 closes: the macOS SZOMB guard is mandatory, separate from current refusal-path evidence.
Socket ordering remains unverified. Its named both-OS proof must establish the premise before the dependent driver repair.
Source review must verify priority before Kill effects, bounded sends, descriptor cleanup, complete messages, EOF ownership, and AD-6 callers.
PC's local build and 43 tests are reports only. No source or executable result is accepted.
Source READY, two reviews, reversals, A5-4 table, both-OS proofs, and final gate remain due.
Sol's revision-3 design receipt remains unread. No execution job runs here. Never poll.

Revision-3 design CLEAN pushed: a661545657a37bc4a1f68bfab15da7c1a5c41154.
The worktree is clean. No source or executable result is accepted. Wait for the source/evidence doorbell. Never poll.

## R-78 both design CLEANs verified

Sol receipt msg_plugin-w_1791752475_943937 names 1c4aacd9ca68abc860103270258ead5d44ec2023, Round 3.
The reviewer reads that round and verifies publication on Sol's remote branch.
Both design CLEANs cover revision 3 hash 52b9ada3f73a1fdf77c9f05b04775f2fffb4b6b5385f72149db77c6f99462885 and base 670e5fc799a25794910a1847fc41b3e089bf03f4.
The integration design receipt condition is closed. No source or executable result is accepted.
The socket-order premise, exact source reviews, reversal evidence, A5-4 table, both-OS proofs, and gates remain due. Never poll.

Matching integration design receipt pushed: cb4073b2a6e690111a9b1facd24519ff4cac95b9.
The worktree is clean. Both design CLEANs are verified. Wait for the source/evidence doorbell. Never poll.

## #289 .39 to .41 pin: review complete with two evidence findings

READY msg_plugin-w_1791755720_3b91b3. Head 4e26982b4c8ed836e43baa30f18fbf8134db90d5.
Own/PR/gate base f68f26a7b12738f2b19ab39fe76a93f3d1a72f12, imported once with lead approval after the lists failure.
The seven-path source diff is read. No new logic finding is identified.
The seven manifest pins and nine lock sources match .41. The ledger exactly equals .41's 691 Core ids.
Job-common bytes are unchanged and match both tags. The real pty_queued boundary and R-73 route test agree with the pin.
Four real removals have no .41 fence exception and current-head real PASS records. Four permitted transfers each retain one exception.
The green log hash is 426854307dda1a64cf844de2018c61daa3dcaad3cd6c303a3b71af3790e1169e.
All ten steps PASS. Default 1786/0, slow 586/0. All eight mutants are caught in Stage 2; raw and named identities match.
No FLAKY line. Earlier mutant and lists gates remain RED in the record and body.
PIN41-1 MEDIUM: missing full A5-4 before/after table against .39 on both tiers.
PIN41-2 MEDIUM: missing N>=5 real proof for the six held-write ids (lead decision 04:56).
The two held-write removals are in_2_link_loss and in_7. The planned event-fence repetitions add coverage, not a new requirement.
PC confirms both records are absent and plans serial pool evidence jobs in msg_plugin-w_1791755816_ab4038.
The verdict is NOT CLEAN (2 open), verdicts/pc-pin-41.md. Sol's same-head source verdict remains due.
No execution job runs here. R-78 source/evidence and F-283-1 remain separate PC work. Never poll.

#289 package verdict pushed: 42f295473daa793617c78aca4da7d454eaa703af (initial source/evidence review 267ce3688ff02f057789edb53524a7c55e0381e5).
The worktree is clean. Two evidence findings remain. Wait for the completed evidence doorbell. Never poll.

## #289 Sol Round 1: same two evidence findings

Receipt msg_plugin-w_1791756279_bad220 and verdict 45307d6fe71f09a1e5302d45dfd25c488d20d804 are read.
Publication on Sol's remote branch is verified. Both reviews cover head 4e26982b4c8ed836e43baa30f18fbf8134db90d5 and base f68f26a7b12738f2b19ab39fe76a93f3d1a72f12.
Sol's two findings match PIN41-1 and PIN41-2. No new integration source defect is identified.
Sol independently verifies the exact-head gate and the eight matching mutation identities/citations with normal PASS records.
Both evidence findings stay open. No running or future evidence is accepted. Wait for the completed evidence doorbell. Never poll.

#289 integration receipt record pushed: 83dd3901d5b71984b5de8c398849e6700e47444e.
The worktree is clean. Both reviews retain two evidence findings. Wait for the evidence doorbell. Never poll.

## #289 Round 2: evidence findings closed; package CLEAN on list-only head

Head f45794a454761c20fa2cfb9c0bd1b0f9fabecfbc restores only IN-7 to its original real-pending status.
The implementation and pin remain the reviewed 4e26982b source. The restoration stays within the unmerged PR.
The reviewer matches all cells in both v2 A5-4 tables to raw results: 691 rows each, zero mismatches.
Testkit must-pass 223→230, eight changes. Real must-pass 198→204, pending-real PASS 4→14, 21 changes.
One real PASS-to-FAIL is retained: EV-5 route-closed, seed 7 step 28, activity_before_close false and closed true.
Lead msg_plugin-w_1791756534_fb4088 accepts the source-disjoint failure for #289 under the controlled .39 baseline reproduction.
Baseline log hash b0fa78606981a6fe0621558c35d473d7c0aac59452fa402790443904caedcc8c and scratch diff hash 027d7d7aad29ad7cd084dd9081f76001d837a3d07671debae1ac858e339f5397 are recorded.
The scratch snapshot delays an existing input; it is not an uninstrumented failure or merge gate. Both earlier reviewer acceptances are read.
Repair owner RC, contracts #69 into .42. No new pending entry and no rerun-to-PASS.
All 80 N-run records are unique and complete. IN-7 is 4/5 real; all other pairs are 5/5. The failed run is preserved.
PIN41-1 and PIN41-2 close. Package verdict CLEAN applies to f45794a4 with the preserved failure and pending limits.
The local arm reversal reaches Unsupported and then passes after restoration. Its clean history remains owner-reported.
Static log is read; taint/lists report success. The lead requires no separate full gate now; the final SLOT gate covers the head.
Sol's matching evidence verdict, final import/check/review, exact-head final gate, body, and Q7 remain due. No execution job runs here. Never poll.

#289 Round 2 package CLEAN pushed: 110de71a0946a57d6494f749ced02e4cf13fcb8c.
The worktree is clean. Final-slot conditions and Sol's matching receipt remain due. Wait for the doorbell. Never poll.

## #289 both source/evidence CLEANs verified

Sol receipt msg_plugin-w_1791757363_d2ecc6 and Round 2 verdict 3320652a81221ad3e01f936195ffcb03f2abffc1 are read.
Publication is verified. Both source/evidence CLEANs cover f45794a454761c20fa2cfb9c0bd1b0f9fabecfbc and base f68f26a7b12738f2b19ab39fe76a93f3d1a72f12.
The matching integration receipt condition is closed. Both evidence findings remain closed.
The jobs use default seeds 0–7, not 0–31, and run only on Linux. The live body states that scope.
Body hash c70e93fae81bf832f3e26cb5ffa91f0319f5025b84b8c5961dd6403be2e76dfa is verified.
The IN-7 rule-9 record is read; hash 0f92c5759c624a1a9a5bc85b947ffa42e9567a7e21920f8d897bc8b957d21f29.
IN-7 remains pending with open diagnosis, owner PC after #289 and R-78. No replacement successful run or R-88 cause is claimed.
Final SLOT import/check/review, exact-head final gate, completed body, and Q7 remain due. No executable job runs here. Never poll.

Matching #289 CLEAN receipt pushed: f6ed6253428bbfa553e2c4a11491bc4d1b2f5d04.
The worktree is clean. Both source/evidence CLEANs are verified. Wait for the final-slot doorbell. Never poll.

## #291 R-78 Round 1: two source findings

Reviewed head 79f31be3b3213e362539a51c35a1a1273a5aee26, base 5e953398925351c12e463a7b0a946b79d9841edc.
The reviewer read all 27 changed files, relevant callers, all changed tests, and retained evidence.
R78-PC-1 MEDIUM: ProcessExited after Exited closes the readable link and discards later complete Done and RouteClosed frames.
The production worker sends Exited before its route-close reports. An undelivered route can then close SessionLost.
The old #217 R2-1 test pins this behavior; this is an R-78 completeness defect, not a new regression.
Lead FINAL msg_plugin-w_1791760401_5c2be8 supersedes the transfer decisions and keeps this finding on #291.
The fix must read without blocking until EOF or would-block. It must not wait for EOF.
Evidence must show that Done and RouteClosed{SessionEnded} after Exited are delivered. The old R2-1 test must change under the ruling.
R78-PC-2 MEDIUM: kill_payload still probes a worker already known gone after a write failure, contrary to accepted revision 3.
PC reports fix dc47d2b474826e99c9c80cacf64b47a031f7e0d7. That head's completed gate and READY remain due.
Verdict NOT CLEAN (2 open) applies to 79f31be3; file verdicts/pc-r78-stopping.md.
The exact-head Linux gate passes all ten steps. Default 1806, slow 587; 49 caught mutants, zero missed, two unviable.
All 49 raw caught identities match the 49 named test citations. No FLAKY line appears in the gate.
Both A5-4 tables have 690 ids and zero cell mismatches against independently parsed raw records.
Testkit seeds 0–31: 230→230 must-pass PASS, zero changes. Real seeds 0–7: 200→201, seven changes.
AD-3 PASS-to-FAIL remains preserved. The same step-16 failure occurs without R-78; the R-80 fence repair belongs to P3.
Mac 3/3 records use intermediate commits; the later socket and facade changes replace polling loops with process helpers.
The facade test uses a real worker process with a test-owned worker link. It does not prove production route-frame order.
No reviewer execution job runs. Wait for the fixed-head READY doorbell. Never poll.

#291 Round 1 verdict pushed: 5eb26a34f7131061db9bc81d6b88e16e4a24dcd5.
The worktree is clean. Two findings remain open on 79f31be3. Wait for the fixed-head READY. Never poll.

## #290 F-283-1 Round 1: one source finding

Reviewed head 4fb31c10bde8eba1b947f7a142e88568bfb22cd6, base 5e953398925351c12e463a7b0a946b79d9841edc.
The reviewer read all three changed files, all remaining program assertions, runtime callers, and retained evidence.
F283-PC-1 MEDIUM: program.rs still asserts ignores_sigterm() and window_size(), each a getter for private state with only test callers.
The widened scope requires every private-state assertion in the program-edge tests to change.
The SIGTERM proofs must use signal/poll_exit before and after the script step.
Window-size proof needs observable behavior or a named coverage/removal disposition. Audit remaining getters under the same rule.
F283-PC-2 LOW closes: PC corrected the stale pending-gate body and named Sol-B. The reviewer read the corrected body.
Verdict NOT CLEAN (1 open), verdicts/pc-f283-observable.md.
The exact-head Linux gate passes ten steps. Default 1792, slow 584; eight caught mutants, zero missed or unviable.
All eight raw identities match named test citations. No FLAKY line appears in the gate.
Local reversals R1–R4 each fail an observable test; restoration passes 25/25. Clean history remains owner-reported.
Runtime ScriptedProgram::unread is unchanged. Only the unused output_unread counter and accessor are removed in production.
No reviewer execution job runs. Wait for the fixed-head READY doorbell. Never poll.

#290 Round 1 verdict pushed: 79bcb435c3e2d50179cca99ce1069cfc4802eee0.
The worktree is clean. One finding remains open on 4fb31c10. Wait for the fixed-head READY. Never poll.
Sol-B receipt msg_plugin-w_1791760749_dda3e2 and verdict ad157877c794cf886b30b6645a92bb1dabd4a2f6 are read.
Both verdicts cover the same head and base. Sol-B records the package finding as B290-1, without a separate package-scope review.
No separate integration defect is reported.

## #291 revision 4 design delta: one open scheduling finding

Sol receipt msg_plugin-w_1791761035_b6ec98 agrees with both original source findings and identifies the new scheduling concern.
The reviewer reads pc-r78-design-rev4-delta.md, hash e8206746aebc91e579efbfdac247ba5979f94c7571436d43ab74dcb1de7d4352.
The nonblocking drain ends at EOF or would-block. A held frame waits for mandatory room; a budget-delayed drain can continue next pump.
R78-D5 MEDIUM: rule 3 makes every open peer_ended link set more, including an unaccepted held frame.
The engine cannot accept that frame until the host polls events. Unconditional more can repeat pumps with no progress.
Require a rule that separates budget delay from mandatory-room backpressure, plus a full-queue production-driver test and resume after poll/unpark.
Design verdict NOT CLEAN (1 open), appended to verdicts/pc-r78-design.md.
Both original source findings remain open on 79f31be3. No repaired source head or execution result is accepted.
The reviewer awaits the revised design and fixed-head READY. No execution job runs. Never poll.

R-78 revision 4 design verdict pushed: c2d8f2a56f400e9d3fd4e4c8f557c69303726597.
The worktree is clean. R78-D5 remains open. Wait for the revised design and fixed-head READY. Never poll.

Sol receipt msg_plugin-w_1791761177_2a8b8f and verdict 8c577a5718baa9b400c877e6be02ccc99131184d, Rounds 4–5, are read.
Publication is verified on origin/stage1/integration-review-sol. Sol's design finding matches R78-D5 on the same delta hash.
Sol closes the known-gone finding only at source level on dc47d2b4. The old gate does not cover that fix.
This package has not reviewed a repaired-head READY. Both original findings remain open on 79f31be3.
Sol records missing retained reversal proofs and revised both-OS proof as fixed-head evidence obligations.
The four Linux facade PASS records across the two old gates cover byte-identical old proof source.
They do not cover the revised nonblocking facade proof. No new-head execution result is accepted.

## #291 revised revision 4 delta: design CLEAN

PC receipt msg_plugin-w_1791761355_45eac6 supplies the revised design.
The reviewer reads the complete delta and verifies hash cf3914bc4e7f8fae32619b72ddcb3ae941b6dfe6fb871bda3ab3e39f2a2438ef.
R78-D5 closes at design level. The earlier unconditional more rule is withdrawn.
LinkPeerEnded sets drain_due; the first read clears it. An unaccepted held frame does not make that link runnable.
The budget carry keeps a budget-delayed drain runnable. Mandatory-room backpressure waits for poll/unpark, then resumes in order.
The proposed full-queue proof checks no more while blocked, the wake after polling, ordered delivery, and the final close.
The nonblocking drain still closes at EOF or would-block. No timer or limit is added, and no wait for EOF is allowed.
Design verdict CLEAN applies only to this hash. No repaired source head, executable result, or gate is approved.
Both original source findings remain open on 79f31be3 until the repaired-head source review establishes closure.
The retained reversals, revised both-OS facade proof, new-head gate, both source reviews, and matching design receipt remain due.
No execution job runs. Wait for the repaired-head READY doorbell. Never poll.

Revised R-78 revision 4 design CLEAN pushed: bbf3ad90dfe221dd5ca391f7d264912284a3662c.
The worktree is clean. Design hash cf3914bc is accepted; repaired source and evidence remain due. Wait for the doorbell. Never poll.

## #291 revision 4 implementation note: design CLEAN without drain_due

PC receipt msg_plugin-w_1791761534_b4152f updates the design hash to bb550e4191009a3e1aa094e6b5e7f22f5adc1b9fdd5f09b3d9eebb2e811f8d4f.
The reviewer reads the full delta and checks the scoped flag removal and relevant read paths at reported local head 2bc8b247bb83b85c8280b5dbae2aa9b6ea617e46.
For an ended peer, a read closes the link, holds a frame, or sets more_input at a pump bound.
Thus peer_ended && held.is_none() preserves the accepted scheduling rule without the separate drain_due flag.
R78-D5 stays closed at design level. No full source head is approved.
The reviewer reads the full-queue, budget, and settle-time exit tests. Their completed new-head evidence remains due.
Reversal log 1ff76822 records R1–R5 failures and a restored 319/319 PASS; it predates the later budget test.
The source-path analysis establishes the flag's equivalence. No direct reversal of drain_due is claimed from that log.
Both original source findings remain open on 79f31be3 until the repaired-head source review establishes closure.
No execution job runs. Wait for the repaired-head READY. Never poll.

R-78 revised implementation-note design CLEAN pushed: 93eade7e762d949a9ceba6d0de78b22ad536ba4c.
The worktree is clean. Current design hash bb550e41 is accepted; repaired source and evidence remain due.
Sol receipt msg_plugin-w_1791761629_eabb0d and verdict 304f97f9da358b4ff2b9e480b5b2588eb1149b80, Round 6, are read.
Sol's publication is verified. That matching design CLEAN covers cf3914bc, before the current equivalent-rule delta.
Sol's same-hash review of bb550e41 remains due. No new source or execution approval is given. Never poll.

## #291 current design: both CLEANs verified

Sol receipt msg_plugin-w_1791761880_008d02 and verdict 5bfb2bbee7e98ead84fdd3fea7f2d4c31b72c266, Round 7, are read.
Publication is verified on origin/stage1/integration-review-sol.
Both design CLEANs cover bb550e4191009a3e1aa094e6b5e7f22f5adc1b9fdd5f09b3d9eebb2e811f8d4f.
The matching design receipt condition closes. This approves no source head or execution result.
The repaired-head review, retained reversals, revised both-OS facade proof, completed gate, and final acceptance remain due.
No execution job runs. Wait for the repaired-head READY. Never poll.

Matching R-78 revision 4 design receipt pushed: fa6e008ec6e80305c1fd27272d2a2b23296cb36a.
The worktree is clean. Both current design CLEANs are verified. Source and completed evidence remain due. Never poll.

## #291 Round 2: source repairs close; two evidence findings

READY msg_plugin-w_1791763632_cc997f. Head 8ac717c7d94560bf1f82a5975bb45162ba0b88da, imported base f6025aed4cdf7da836e835e0040fc128b7e8f32f, gate base 3aacc58d086e55a6b186165ccf9001e2618e2869.
R78-PC-1/2 close in reviewed source. The nonblocking drain handles Done and RouteClosed after Exited and closes at would-block.
The known-gone probe guard and both-order no-probe assertions are present. R78-D5's held-frame backpressure rule is checked.
The first more is v1's expression; only the settle-time more adds draining. The old redundant-term gate stays RED.
The current gate passes ten steps: default 1822, slow 600, 60 caught mutants, zero missed, two unviable, no FLAKY.
All 60 raw caught identities match the named citations and normal PASS records.
Both new A5-4 tables have 690 ids, with zero cell mismatches. Testkit 230→230, no changes; real 205→203, six changes.
Lead msg_plugin-w_1791764208_171fa2 attributes ST-6 step17 ne[1,1] to the accepted controlled plain-f6025aed timing witness.
The own diff changes no model-feed or capture path; the pinned transcript reaches no new loss branch before step17. This meets the lead's condition.
R78-PC-3 MEDIUM: IN-9 modes_read_at_write_start PASS→INCONCLUSIVE, seed4 step8 step_timeout, has no baseline witness.
The lead keeps this flip as a merge blocker. PC owns diagnosis; a plain-v1 failure or accepted controlled exact-signature witness is required.
R78-PC-4 MEDIUM: 1ff reversals and three revised Mac t13 records used targets/pc after another worktree, without a Cargo clean.
The body confirms B13 and marks those records historical and unvalidated. The lead requires replacement logs after recorded clean or per-root builds.
R78-PC-5 LOW closes: the reviewer reads the corrected body count of 11 driver test functions.
Verdict NOT CLEAN (2 open), verdicts/pc-r78-stopping.md Round 2. No source defect remains; evidence blocks CLEAN.
No pending change or rerun-to-PASS is allowed. No reviewer execution job runs. Wait for the evidence doorbell. Never poll.

## #290 repaired head review in progress

READY msg_plugin-w_1791763810_2709b7. Head c542a9fd93be950cb77522257b1b4e3741920a52, imported base f6025aed, gate base 3aacc58d.
The SIGTERM tests now use signal/poll_exit. The unused getters and size field are removed; the getter audit is checked.
F283-PC-1 closes in source. Both remaining changed test files are byte-identical to the earlier reviewed head.
The gate passes ten steps: default 1799, slow 597, eight caught mutants with matching normal PASS citations, no FLAKY.
A LOW remains pending correction: the body and new resize comment claim worker-model/ReadScreen coverage from a host-forwarding test.
Sol-B independently states that resizes_of_one_session_reach_the_worker_in_begin_order proves host forwarding/order only.
The separate worker size-op gap remains. Removal of unobservable program state supplies no new worker resize proof.
The reviewer sent PC the correction in msg_plugin-w_1791764101_58a319. No final package verdict on this head is issued yet.

## #291 Round 2 evidence closure

R78-PC-4 closes with replacement records after recorded cargo clean on exact head 8ac717c7.
The reversals fail 8/1/1/10/1 tests, then the restored source passes 320/320.
The prebuild manifest identifies the candidate binaries. Three Mac facade runs PASS within the same sole-worktree build sequence.
The old shared-target records remain historical and unvalidated.
R78-PC-3 closes with the accepted 23bl(A) controlled IN-9 reproduction on plain f6025aed plus scratch diff 0151f1ec.
The raw result matches step 8 step_timeout. Both embedded patches match the supplied diff.
The delayed mode output widens an existing order; the unchanged acknowledgement does not fence the worker's PTY read.
Focus off yields NotWritten(NotReported). Step 6 can discard that early completion before step 8 waits for it.
No event-order trace or uninstrumented-v1 failure witness is claimed. The raw failure remains INCONCLUSIVE.
The corrected rule-9 record hash is 8ff1d2f6c28b09ad680e74f3ee7753815fcf26c1c21c6308fa70fc997a4d7cf8.
Lead case receipt msg_plugin-w_1791764762_105387 accepts the signature. PC owns the R-88 held-write contracts PR under R-74.
Sol's matching acceptance e72d7d01dc22a5eac71d57ea2e3f63c53fa32028 is read and publication is verified.
No package source or evidence finding remains. The current body check and final-slot reviews remain due before merge.

## #290 Round 2 package CLEAN

Head cc491863b4143d37a3e9586eb1848db0bb9c3a5f changes only one comment from gated parent c542a9fd.
F283-PC-1 closes: SIGTERM tests observe signal/poll_exit, and the dead size getter, field, and state-only test are removed.
The remaining getters have runtime callers. The earlier rewritten observable tests remain unchanged.
F283-PC-3 LOW closes: the corrected comment and body state host forwarding/order coverage only.
The separate worker size-operation gap remains. This PR supplies no new worker resize proof.
The corrected body hash is cd7a429129a2f833163c3ff27faa585b90697737106bd96c7ea2c4ae821bf328.
The c542a9fd gate passes ten steps, with default 1799, slow 597, and all eight caught mutants matched to normal PASS citations.
This parent gate supports the source after a comment-only change. It is not an exact-final-head gate or final-slot approval.
Verdict CLEAN, zero open package findings, in verdicts/pc-f283-observable.md Round 2.
Sol-B's matching integration disposition, final slot import, final delta review, exact-final-head gate, and Q7 remain due.
No reviewer execution job runs. Never poll.

## #289 final-slot source delta CLEAN

PC request msg_plugin-w_1791765546_144247 supplies merge head 0abf666c001e7dcd55e3bc86f0a95e168c183f26.
Parents are reviewed f45794a454761c20fa2cfb9c0bd1b0f9fabecfbc and v1 b0c0a419af27259006b5e06d4e5da3b3d00083e6.
The base-merge-check FAIL correctly requires both delta reviews.
The reviewer checks the complete own diff, overlapping Requests import, report/request paths, and synchronous route_fill caller.
The queued method uses the same request path and hub. Its report and acknowledgement behavior remain unchanged.
The four independently checked id sets match both branches' additions minus both removals, without missing or extra ids.
Counts: ledger 691, testkit pending 416, real pending 40, held 3.
The pin, lockfile, ledger, and route-log test blobs match the reviewed parent.
The exact-head static record includes Cargo clean, clippy zero, and ten xtask exits zero. Fmt success is owner-reported.
Verdict CLEAN for the source delta, zero source findings, in verdicts/pc-pin-41.md Round 3.
The strict result, exact-final-head gate, updated body, Sol's delta review, and Q7 remain due. No final merge is approved.
The #290 verdict is pushed at f99ed57c0522ec52fd0613a4395e4d7c53fe4c4d. PC, Sol-B, and the lead received it.
The #291 completed body remains due. No reviewer execution job runs. Never poll.

The #289 source delta verdict is pushed at b4d6f23fbbefc6c8cc9ad108043cd9ebf9b1a95d; PC and Sol received it.
PC receipt msg_plugin-w_1791765643_5fb852 reports four strict pending passes on 0abf666c and plans a list-only head.
The ids are dp_5_confirmed_paste_resend_is_a_new_op_with_require_bracketed_false, dp_5_input_done_only_for_want_result_true,
dp_5_mode_precondition_zero_effect_reported_on_the_route, and in_9_kitty_unknown_flag_bits_ignored.
The reported run uses seeds 0-31. The raw record and next head remain due for review.
There is no plain-v1 result for these four ids. List presence on both parents does not prove that both parents fail.
Sol receipt msg_plugin-w_1791765630_7887c0 identifies stale body metadata; the corrected current body remains due.
The source-only CLEAN covers 0abf666c, not the proposed new head. Wait for the completed evidence doorbell. Never poll.

## #289 four-id transfer source delta CLEAN

PC receipt msg_plugin-w_1791765735_661a09 supplies head 69d414995408a21a32b73d01608a798202057462, base b0c0a419.
The complete delta changes only two pending files. It removes the four named testkit ids and adds the same four to real-pending.
Each pinned .41 fence exception is checked. The three DP-5 ids lack program_ready; the IN-9 id also lacks three capture fences.
The new block names P3 as owner. A real PASS does not satisfy acceptance while those exceptions remain.
The strict record names 0abf666c plus the four removals, seeds 0-31, no-fail-fast, and the same worktree after the static clean.
All four tests report ok. Suite summaries total 1178 passed, zero failures, and the record ends with exit 0.
The earlier strict run remains FAILED with four pending passes and exit 101. Neither record proves a plain-v1 result.
The current list counts are ledger 691, testkit pending 412, real pending 44, held 3.
Verdict CLEAN for the source delta, zero source findings, in verdicts/pc-pin-41.md Round 4.
The updated body, completed exact-head gate, Sol's matching review, and Q7 remain due. No final merge is approved.
The #291 current body still remains due. No reviewer execution job runs. Never poll.

The #289 four-transfer verdict is pushed at fc643a29d9bf552855dbbaa14f6f5356b1372e2b; PC and Sol received it.
Sol receipt msg_plugin-w_1791765902_7ad08f and published verdict 2cd7aae57da51ed95f58f23097a49124207e0c60 are read.
Publication is verified. Both source delta CLEANs cover 69d41499 and base b0c0a419.
The reviewer reads the complete current body and GitHub head/base. The final-slot section correctly names both identities and all conflict resolutions.
Its counts and later transfers match the reviewed files. Old evidence stays tied to old heads; the final gate is marked RUNNING.
The stale body metadata correction is checked. The completed gate and final body receipts remain due.
The final gate must show all eleven own testkit removals as normal PASS. No running output is read. Never poll.

## #289 completed final package CLEAN

PC receipt msg_plugin-w_1791766453_57d995 supplies the completed gate and body on head 69d414995408a21a32b73d01608a798202057462.
Base remains b0c0a419af27259006b5e06d4e5da3b3d00083e6. Both remote branch identities are checked.
The final Linux gate passes all ten steps; default 1843, slow 606, eight caught mutants, zero missed, timeout, or unviable.
All eleven own testkit removals have normal PASS records. All three real removals have normal real PASS records.
The eight raw mutation catches match eight named citations; each catcher has normal PASS records. No FLAKY line appears.
All 32 once-passing pending-real trials remain pending. The eight own additions retain their pinned fence exceptions.
The final body is read and independently hashed: b3364d02e8d0d151c4c06b1e714849c2ddb92bada2e978f85cf85b152fa56c90.
The old A5-4 and five-run records keep their source, seed, failure, and controlled-attribution limits.
Verdict CLEAN, zero open package findings, in verdicts/pc-pin-41.md Round 5.
Sol's final evidence receipt and the lead's Q7 check remain due. No reviewer execution job runs. Never poll.

The #289 final package verdict is pushed at ff77c4f0d76c2be2722ff0a7b1be20c821532770. PC, Sol, and the lead received it.
Sol receipt msg_plugin-w_1791766700_2a0875 and final verdict 99b916b375e7f5597391634584e7d64d0fd0d351 are read.
Publication is verified. Both final source, evidence, and body CLEANs cover 69d41499 and base b0c0a419.
The package reviewer matches the eight first-stage raw misses to the eight second-stage catches, without an unmatched identity.
The matching final integration condition closes. The lead's Q7 check before merge remains required. Never poll.

## #291 #295 import and ordering delta review in progress

PC request msg_plugin-w_1791772688_d78b20 supplies head 4ccd6a7da142331c2d35fa25f5f12914cdf2d22b.
Plain merge b8fee77332770736f43886b35c387eef3fc996c4 imports base bfef88653bb59ad8d3cd38da1e206f7aba64ce92 into reviewed 8ac717c7.
The resolution performs all queued actions after the first take_exits and before the first link read.
The reviewer checks the complete resolution and exit/action/read/settle paths, including queued pre-pump cancel/handoff actions.
All eleven drain tests and the named handoff/deadline exit sites are read. No source finding is identified.
Ended-peer WouldBlock now uses #295's end_link, retaining the close when the event budget is spent.
The main more expression stays runnable || more_input. The settle expression also includes draining().
All five conformance blobs equal v1: ledger 691, testkit pending 410, real pending 44, real-only 26, held 3.
The independent three-way set check finds no missing or extra id. Imported non-own blobs match the base.
Sol receipt msg_plugin-w_1791772937_5fc763 has the same source assessment and head/base.
The static, strict, host, and canonical base-check records remain due. The 331/331 report is not yet audited.
The completed body and one final gate remain due. No new-head CLEAN or merge approval is issued.
The Round 2 and Round 3 package draft remains uncommitted in verdicts/pc-r78-stopping.md while these records are awaited.
The old source/evidence findings close under the accepted replacement records and attribution; no old failure is erased.
No reviewer execution job runs. Wait for the evidence doorbell. Never poll.

## #291 completed delta CLEAN on 4ccd6a7d

PC receipts msg_plugin-w_1791773040_a23a33, msg_plugin-w_1791773188_ff0bd2, and msg_plugin-w_1791773308_1b2aef supply completed records and corrections.
The canonical base check retains FAIL (2)/(3), with ancestry, no-conflict (1), and both pending (4) checks PASS.
The local strict record reports 1221 passed, zero failed, seeds 0-31, exit zero. Host 331/331 includes every named drain and exit test.
The source assessment and nine conformance blobs are independently checked. All nine blobs equal v1 bfef8865.
R78-PC-6 LOW closes: the body withdraws the old IN-9 diagnosis, preserves controlled-attribution limits, and correctly counts ten distinct checks.
The public-api refusal and capped success remain recorded. The historical script stays; the future copy exports both cap variables.
R78-PC-7 LOW closes: the retained 330/331 failure is merge b8fee773 plus the uncommitted drain-end fix, not an untouched-merge proof.
The body now states that scope and the unretained plain-merge limit. Source review independently supports the ordering diagnosis.
Live body hash da2fa54e4a1df2f2ef7c68f473fa41460c25579500b19939390f4cdf91ce8c32 differs from retained 3aaf938e only by one final newline.
The complete package delta verdict is CLEAN, zero open findings, on 4ccd6a7da142331c2d35fa25f5f12914cdf2d22b against bfef88653bb59ad8d3cd38da1e206f7aba64ce92.
The prior source and evidence findings remain closed with their historical execution and controlled-attribution limits.
The matching formal Sol delta verdict, one exact-head final gate, final evidence reviews, and Q7 remain due. No final merge is approved.
No reviewer execution job runs. Never poll.

The #291 complete delta verdict is pushed at 6c4d51321b7e352a6c0f377d996dc7bd9cd84729. PC, Sol, and the lead received it.
Sol receipt msg_plugin-w_1791773769_91a53d and verdict 718699173b786c3bb74e3201db7ad28073cf79d9 Round 9 are read.
Publication is verified. Both delta CLEANs cover 4ccd6a7d, base bfef8865, and tree ed7ce2ee718c20284fbb1543f6567ce5e6e9c7be.
The formal matching delta condition closes. Zero source or body findings remain.
Both failing prefixes are checked at .41: no Cancel, Attach, exit, failed send, or lost link precedes either failure.
The initial whole-action pass therefore supplies no new relevant action on those prefixes. This is source reasoning, not new execution.
One exact-head final gate, mutation audit, final body/evidence reviews, and lead Q7 remain due. Never poll.

## #291 completed final package CLEAN

PC receipts msg_plugin-w_1791774951_db0090 and msg_plugin-w_1791775019_655e5a supply the completed final gate and body.
Exact head 4ccd6a7da142331c2d35fa25f5f12914cdf2d22b, base bfef88653bb59ad8d3cd38da1e206f7aba64ce92, tree ed7ce2ee718c20284fbb1543f6567ce5e6e9c7be.
The final Linux gate runs on structvr-MS-7C77, not msa1. All ten steps PASS; job and wrapper exit zero after 854 seconds.
Default 1887 PASS, slow 613 PASS, mutation total 62 with 60 caught, zero missed/timeout, and two unchanged unviable identities.
The two first-stage misses exactly match the two second-stage catches. All 60 raw catches match named citations and normal PASS records.
All named drain, handoff, and deadline exit tests PASS. Real facade, both socket-order tests, and zombie proof PASS in slow and baseline.
All 34 once-passing pending-real trials remain pending. IN-9/ST-6 gate PASS does not erase their old A5-4 failures.
The final body is read; live hash 8e3ca6976c3a3c3af005d93bd1d09477f9536f6c682137b2ef22ea57ad817252.
Retained snapshot 6b490871 differs only by one final newline. The body names the completed gate and preserves all older source limits.
Verdict CLEAN, zero open package findings, in verdicts/pc-r78-stopping.md Round 4.
Sol's matching final evidence receipt and lead Q7 remain due. No later source head or final merge is approved. Never poll.

The #291 final package CLEAN is pushed at b29b6a853340d9f8dce05ceec3cfb1b6c4b5a3fd. PC, Sol, and the lead received it.
PC receipt msg_plugin-w_1791775243_6e9a70 supplies the updated package manifest e2a9bcba7261aacf3d039bcf0f4c2d4ad78da7fd268c5309b7e0f995c6b4b46f.
The complete manifest is read and hashed. Its new entries match the accepted completed gate and final body.
The client output is byte-identical by hash to the canonical gate log. No source or execution result changes.
Sol's matching final receipt and lead Q7 remain due. Never poll.

Sol receipt msg_plugin-w_1791775438_924fcd and final verdict 7b48c749a8b1b5e3b562b6e5baaa00f02b6fb657 Round 10 are read.
Publication is verified. Both final CLEANs cover 4ccd6a7d, base bfef8865, tree ed7ce2ee, the completed gate, and the final body.
The matching final integration condition closes. Zero final source, evidence, or body findings remain.
The lead's Q7 check and an equal-tree merge receipt remain due. No later head is accepted. Never poll.

## #291 merged; review complete

Lead receipt msg_plugin-w_1791775517_fb7516 reports Q7 complete and merge e4f495040a80b524a7ac9accba95272d5c2594fa on v1.
The reviewer verifies parents bfef88653bb59ad8d3cd38da1e206f7aba64ce92 and gated head 4ccd6a7da142331c2d35fa25f5f12914cdf2d22b.
The merge tree ed7ce2ee718c20284fbb1543f6567ce5e6e9c7be equals the reviewed and gated tree. Remote v1 names the merge at this check.
The Q7 receipt and equal-tree merge condition close. #291 review work is complete, with zero open findings.
The older failure, attribution, and platform limits stay recorded. #290's final-slot review remains due when the lead assigns it.
The reviewer owns no build target to clean. No execution job runs. Wait for the next authorized doorbell. Never poll.

## #307 control queue: complete Round 1 NOT CLEAN

READY msg_plugin-w_1791780222_a9016a assigns head 614cc30dabd59a7d8d5a770ff0eb74991bb240cb.
Source base is e4f495040a80b524a7ac9accba95272d5c2594fa. GitHub and gate base are fcc09461cb0c0de4ef067c160f24d205f0f65c30.
The lead assigns package reviewer 0179 and Sol. The reviewer sends its UUID before review.
The binding brief is read first. This review covers all 19 changed files, the body, the import, and completed evidence.
Verdict: NOT CLEAN (2 open), recorded in verdicts/pc-control-queue.md Round 1.
CQ-PC-1 MEDIUM: a fatal send clears out but retains refused_full, so the failed write side can post SessionWritable or room-related more.
The finding follows from source. Sol independently records the same finding as R307-SI-1. No reviewer reproduction runs.
CQ-PC-2 MEDIUM: the 4096-byte estimate does not meet R-91's largest non-input frame term.
Lead receipt msg_plugin-w_1791780836_98917e and R-100 dispose the sizing question: use header + link_frame_bound and state it in DESIGN.md.
No new CoreLimits fields enter v1. The oversize-at-call checks remain the separate RC PR. No further steward request is due.
PC reports CQ-PC-1 repair 973507ab9e7268165a38cde9d85f4032878f9b64. The report is outside READY 614cc30d and closes no finding here.
The exact-head Linux gate passes all ten checks in 626.5 seconds on MS-A1. The job and wrapper exit zero.
Gate SHA-256 dedecaad1a1e92ced19f86f88c26c3710f97a976caa5f912ec35223e1537dde8 matches the retained client copy.
All 45 raw catches match named citations and normal PASS records. All 13 first-stage misses match the second-stage catches.
The 12 unviable outcomes are build failures. No FLAKY record appears. Both link decoders complete their fuzz runs.
The earlier c48446bc gate's seven misses remain recorded. The current gate catches the six testkit mutations; source removes the redundant predicate.
The local 614cc30d record reports 336 host, 52 link, 427 testkit, 192 worker tests and strict conformance 260 PASS at seeds 0-31.
The eb37555a seed evidence reports both IN-6 ids PASS at strict testkit seeds 0-31 and real seeds 0-7, eight real passes each.
Those local seed records precede the current mutation-test change. The current gate passes both ids in both tiers.
The import retains both sides' driver and worker fields. The independent three-way set check finds 408 expected and actual pending ids.
Only the two IN-6 ids leave pending. Eight other conformance text blobs match the source base. EV-9 and R-93 debt remain pending.
Live body hash is c312c5822dca598e58fdfe71f9cfe629cfba0078c15b2d56b37589b121338144. Its sizing claim must change with CQ-PC-2.
No later head is accepted. PC must send a combined READY with both repairs and completed evidence. No merge is approved.
No reviewer build, test, gate, mutation, reproduction, or executable base-check job runs. Wait for the doorbell. Never poll.

The complete #307 package verdict is pushed at c5afafb0cf08c3f5695b8a44e4d95b3427abaa6d. PC and Sol received it.
Sol receipt msg_plugin-w_1791781430_a9794a supplies formal Round 1 verdict 5d516a30afe4c4c1d536946b4cf4bf11e1c2a642.
The reviewer reads verdicts/stage1/pc-control-queue.md at that commit and verifies its published branch contains the commit.
Both formal verdicts cover READY 614cc30d and source base e4f49504. Both retain the same two MEDIUM findings.
The combined repair READY, corrected body, completed evidence, and both repair reviews remain due. No later head is accepted.

## #307 Round 2 audit complete; documentation delta in review

READY msg_plugin-w_1791783700_3cf4ef names 5139373242b37638cabf1017ab471124c2330adf, with source base e4f49504 unchanged.
Tree is dcbefc16d2986aa6fa2640a24bae856e45dfb7c2. Gate comparison ref is ae49bbec2d1086f918341bc596bf8560ba27bcd1, not imported.
The complete seven-file repair diff and all new tests are read. CQ-PC-1 and CQ-PC-2 close on this head.
The failure retires refused_full and excludes write_failed from a new Full refusal. The read side still handles Done.
The bound now uses HEADER_LEN + HostEngine::link_frame_bound. All ten per-kind encoder tests and the formula test PASS.
The complete exact-head gate passes all ten checks in 643.8 seconds on MS-A1. Job and wrapper exit zero after 653 seconds.
SHA-256 e867fc5768a40ed812188bfc8803a829a8f5d92da1932fd9bb1657f7c527b461 matches the retained client output.
All 52 raw catches match named citations and normal PASS records. All 13 first-stage misses match the second-stage catches.
Twelve unviable identities match Round 1 after line shifts. No FLAKY record appears. Both decoders complete fuzz runs.
PC receipt msg_plugin-w_1791783933_4e23ae replaces the old reversal proof with red-on-revert-51393732.log and red-on-revert-v2.sh.
The accepted replacement records a clean exit zero at 05:43:58 UTC, both source diffs, and each single-test failure at exit 100.
The restored clean worktree passes 348/348 at exit zero. The older reversal record remains historical and is not counted.
The local script reports host 348, link 52, testkit 427, worker 192 and strict conformance 260/0 at seeds 0-31.
The exact-head body audit accepts the evidence-only correction. Live body hash is 2ce1055c8f104d6f48b0970a15513361b445a5d83c9c679a1ba09e84da4be77a.
Sol receipt msg_plugin-w_1791783927_115ab4 identifies CQ-PC-3 / R307-SI-3 LOW: documentation omits the frame cap and misstates the small test bound.
Round 2 records NOT CLEAN (1 open LOW) in verdicts/pc-control-queue.md. Both MEDIUM findings close.
PC then sends documentation READY msg_plugin-w_1791784031_6a062c at 50a50cff3fcdbbba1976c3194e0b49183726dbc8.
The reviewer reads its complete delta: DESIGN.md adds saturation and the cap; one test comment changes to 1 MiB plus 4 × 1024 bytes.
The two-file delta changes four lines and removes three. It changes no executable expression, list, pin, or configuration.
The runtime gate stays at parent 51393732. PC reports focused local checks but supplies no separate retained log for those checks.
The reviewer requests a body correction to label the 51393732 gate and reversal evidence as parent-head evidence, not the current PR head.
The final exact-head import, checks, gate, both reviews, and Q7 remain due. No reviewer job runs. No merge is approved.

## #307 Round 3 complete package CLEAN

Head 50a50cff3fcdbbba1976c3194e0b49183726dbc8, source base e4f495040a80b524a7ac9accba95272d5c2594fa.
Tree 90d10894ec8b19da48921f176abae1baaba771e5. Parent is the fully audited gated repair head 51393732.
CQ-PC-3 / R307-SI-3 closes after the complete documentation delta and corrected body are read.
PC receipt msg_plugin-w_1791784143_0a62d5 labels the gate and replacement reversal as gated-parent evidence.
The body explicitly states the current text-only head and the exact imported final gate still due.
Live body hash bb0312d0c9f0475dc0e3a9592cd54fd3069bfebe2093ca5397740b48cde90dd5 differs from PC's 71d67e67 hash only by one final newline.
The complete package source, body, and supplied evidence review is CLEAN, with zero open findings, in verdicts/pc-control-queue.md Round 3.
The accepted gate remains scoped to 51393732. There is no gate on 50a50cff; its only source delta is documentation and one test comment.
PC's focused local checks on 50a50cff have no separate retained log and are treated as reports, not independently audited proof.
Both MEDIUM runtime repairs remain closed. The old source failures and evidence limits remain recorded.
The matching formal integration verdict, authorized final import, checks, exact-final-head gate, both final reviews, and Q7 remain due.
No reviewer job runs. This CLEAN approves no merge. Wait for the doorbell. Never poll.

The complete #307 package CLEAN is pushed at 7bcb22f2469978e359cccc78b6f2f8cf53e9d2ef. PC, Sol, and the lead received it.
Sol receipt msg_plugin-w_1791784271_b687eb supplies inspection 7ead3eecc14ab12bddcf5c090c8961c68ec5e72c.
The reviewer reads its final correction sections and verifies that the published integration branch contains the commit.
The inspection closes the same three findings on 50a50cff, base e4f49504, with the same body and evidence scope.
Sol's formal Round 2 verdict remains due. The package CLEAN was sent in msg_plugin-w_1791784262_c0167f.

Sol receipt msg_plugin-w_1791784339_07407f supplies formal CLEAN ea7dcaa013c28e23c5aaf8762148192e37dc3356.
The reviewer reads its Round 2 verdict and verifies publication on origin/stage1/integration-review-sol.
Both formal CLEANs cover 50a50cff3fcdbbba1976c3194e0b49183726dbc8, base e4f49504, and tree 90d10894ec8b19da48921f176abae1baaba771e5.
The matching formal review condition closes. Zero source, body, or supplied repair-evidence findings remain.
Both verdicts keep the parent 51393732 gate scope and the withdrawn first reversal record explicit.
The authorized final import, checks, exact-final-head gate, final reviews, and Q7 remain due. No later head or merge is accepted.

Lead receipt msg_plugin-w_1791787683_b8c46e reports an inbox permission denial. The lead can send but cannot read incoming messages.
The lead instructs pairs to continue queued work, record verdicts in files, and infer no lead answer.
The #307 verdict and matching formal receipt are already published. No new review head or completed final gate is supplied here.
The reviewer will resend the important #307 CLEAN receipt when the lead announces inbox recovery.
The reviewer infers no final-slot authorization. Wait for the next authorized doorbell. Never poll.

Lead receipt msg_plugin-w_1791818308_100adb announces inbox recovery and asks for important messages to be resent.
The reviewer resends the matching #307 CLEAN receipt in msg_plugin-w_1791818343_b236da.
The resend names both formal verdict commits, exact head 50a50cff, source base e4f49504, and tree 90d10894.
It preserves the parent gate's scope and the pending final-slot/import/check/gate/review/Q7 conditions.
No new final-import head or completed final gate is supplied to this reviewer. No slot or merge approval is inferred.

## #307 current-base delta inspection

PC request msg_plugin-w_1791819576_774597 supplies head 2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54, tree b117637b809c37c63b869e4da2696015dbdf5918.
The accepted head is 50a50cff. Merge ceb6b22e73034a281fe613338069d441c8fa5e05 imports base 080987829a2e2245a7ea97606020400392c974d6.
The lead handoff says #307 left train 3 after conflicts in core.rs and edge_tap.rs. PC resolves it for train 4.
The complete conflict resolution keeps control_full and the imported start_holds/op_holds fields, methods, and initializers.
The reviewer reads the overlapping auto-merges and the driver scheduler/control-room/failure composition. No source finding is identified.
The own patch matches the accepted patch except that 2ed85f93 removes the deleted LaunchSpec.stop_grace_ms field from its launch-size test.
Every changed non-own blob matches the imported base. Eight other conformance text blobs match it.
The independent three-way pending-set check finds 404 expected and 404 actual ids, with no missing or extra id.
Ours removes two IN-6 ids. The base removes the four a2_1/a5_2/dp_2/tm_6 ids. Imported manifests, .43 pin, and mutation profile match the base.
The completed local record reports host 356 plus one ignored, link 54, testkit 441, worker 193, and strict conformance 263/0 at seeds 0-31.
The clean marker is 15:31:06 UTC. Filtered-pipeline and numeric-clean-exit limits remain.
The reviewer has not read the running gate. Its completed record, corrected current-head body, and base-merge-check record remain due.
The live body still describes 50a50cff. PC receives the request for corrected current-head evidence and body labels.
No formal delta CLEAN is issued. No later train head or gate is accepted. No reviewer job runs. Wait for the doorbell. Never poll.

The #307 import inspection is pushed at d6f664f7a43af0f1f590e0281e9a69179d498c48. PC and Sol received it.
Sol receipt msg_plugin-w_1791820008_02acf3 supplies matching source inspection 5f0cf6c7999afd7a8af4957ffe1cd36704a1abff.
The reviewer reads its composition, list, evidence, and matching-receipt sections and verifies publication.
Both inspections find no new source finding on 2ed85f93, imported base 08098782, tree b117637b.
Both formal verdicts still need the completed gate, corrected body, and retained base-merge-check record. No running output is read.

PC receipt msg_plugin-w_1791820358_0a2cad supplies the completed 2ed85f93 RED gate, corrected import body, and base-merge-check record.
The reviewer reads them. The canonical check passes ancestry and routes both reviewers to delta rounds; its missing numeric exit is disclosed.
Eight gate checks PASS. Stage 1 is 39 caught, 13 missed, 12 unviable; all 39 raw catches match citations and normal PASS.
Gate SHA-256 76c3d6ab0c431804b61587cf62bd3b587c1b5c0b91c2f46ff3c5613cd10dd805 matches the retained client output.
The unmutated Stage 2 baseline fails IN-8 at seed 0 step 8 step_timeout. The same test passes in the normal default and slow tiers.
Stage 2 mutations and fuzz do not run. The current-head mutation audit remains incomplete. The completed gate stays RED.
The pinned .43 transcript has no cancel/control_queue step. The source audit finds no new control-queue cause; no plain-v1 reproduction is measured.
PC requests a lead completion decision. The current known-flaky file is empty; no authority is inferred from #308's separate entry.
The body correctly attributes start_holds to #301 and op_holds to #300. The reviewer requests eleven shared paths total, including two conflicts and nine auto-merges.
Live body hash is 1c0e08fd1bc1341eaba98177709e77abeeef354e7a50d3b014a7c9e429faace7.
The formal verdict awaits the corrected body and lead-approved completion path with required evidence. No new source finding is identified.
No reviewer job runs. No later head, train gate, or merge is accepted. Wait for the doorbell. Never poll.

## #307 complete current-base delta CLEAN under the train decision

PC receipt msg_plugin-w_1791820521_1bc686 supplies the lead's explicit completion decision. The reviewer reads the 02:05 PDT lead handoff entry.
No re-gate of 2ed85f93 is authorized. Train 4 must include #312's .44 pin before #307; the exact train gate is its final gate.
The current RED gate remains RED. Stage 2 mutations and fuzz remain unexecuted on this member head.
The corrected body states eleven shared paths: two conflicts and nine auto-merges. It labels both normal IN-8 positive results by tier.
CQ-PC-4 / R307-SI-4 LOW closes. All prior source findings remain closed. No new runtime source finding is identified.
Live body hash is b2f9f44bafa07cc27d8483121095ace9a51043c16f965f228e7020674bbfd506. No code changes after inspection.
Formal Round 4 is CLEAN on exact head 2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54, imported base 080987829a2e2245a7ea97606020400392c974d6.
Tree b117637b809c37c63b869e4da2696015dbdf5918. Prior accepted head 50a50cff. The d6f664f7 record was an inspection, not a formal CLEAN.
This verdict covers the exact member before train construction under the lead's completion decision. It accepts no RED gate or later train head.
The matching formal integration verdict, ordered .44 import, applicable train-delta/combined reviews, exact train gate, final receipts, and Q7 remain due.
Without the ordered structural fix, this CLEAN supplies no alternative completion path. No reviewer job runs. Wait for the doorbell. Never poll.

Sol receipt msg_plugin-w_1791820664_935ba7 identifies a remaining stale body reference to #312 in train 3 and #308's active known-flaky entry.
The reviewer published 4fb39c6210c3170bbeed090a69734bdec7e4cae7 before closing that reference. The premature CLEAN is superseded.
CQ-PC-4 / R307-SI-4 LOW stays open until the body states #312 before #307 in train 4 and removes or labels the historical entry claim.
PC and Sol receive this correction. No terminal CLEAN from 4fb39c62 is sent to the lead. Source findings remain closed.
No source or gate change is requested. No reviewer job runs. Wait for the correction doorbell. Never poll.

PC receipt msg_plugin-w_1791820755_33a438 supplies the final body correction on unchanged 2ed85f93.
The reviewer reads the corrected body and compares its complete changes. It states #312 before #307 in train 4 and the empty current known-flaky list.
The #308 entry is a separate historical proposal. The RED gate and no-re-gate decision remain explicit.
Body hash 05305586c1683a2d42fbdaa1988e3eee625e7e9310bbeea1128d39f60f412587 matches Sol receipt msg_plugin-w_1791820779_6e5268.
CQ-PC-4 / R307-SI-4 closes. Formal Round 4 is CLEAN with zero open findings on 2ed85f93, base 08098782, tree b117637b.
This completed body closure replaces the premature 4fb39c62 CLEAN. The current RED gate is not accepted as green.
The matching formal integration verdict and ordered .44/train reviews/checks/gate/final receipts/Q7 remain due. No reviewer job runs.

The complete #307 formal delta verdict is pushed at ad4750f9cc30669c2d431814e75c6a4cbb653d55. PC, Sol, and the lead received it.
Sol receipt msg_plugin-w_1791821012_3510cc supplies matching formal CLEAN db0e256a1d71f1312b7b121c74544e491106ddfc.
The reviewer reads its complete Round 3 verdict and verifies publication on origin/stage1/integration-review-sol.
Both formal CLEANs cover 2ed85f93f2c4d1a1d96eb4973fb694fc3357ac54, base 080987829a2e2245a7ea97606020400392c974d6, tree b117637b809c37c63b869e4da2696015dbdf5918.
The matching formal member-review condition closes. Zero source, body, or supplied-record findings remain.
The current RED gate and canonical FAIL stay recorded. Ordered .44 import, applicable train/combined reviews, exact train gate, final receipts, and Q7 remain due.

## #307 train-4 member delta CLEAN before the gate

Lead request msg_plugin-w_1791821464_8574ec assigns train head 03709a0cb07f04765ad7a3b48e5570a1c1bf0ad8.
Base 838e4bb01dda53c419ab824913fdc5adae7a16c1; tree 4bb9e8c7f7d6e8b610e4b80ce782499ae3389cde; accepted member 2ed85f93.
The reviewer reads the brief first and the complete eleven-file member-to-train delta.
The merge order is #314, then #312 at 13fc69a0a196f75a6aaa1662c1deac1817e5dd88, then #307 at the train head.
The .44 pin commit 99e5983befde9a96ca4cdb3faea0f7f9a4b930ea precedes #307, as the lead requires.
The own changed-line patch is preserved exactly. All control-queue runtime blobs are byte-identical to the accepted member.
Both IN-6 transcripts are unchanged at .44. IN-8 gains the R-74 held-write fence using the already served pty_queued control.
The reviewer reads the probe interface and output-order path. PRINTS_ENV remains disabled in the real harness; the new report adds no queue wake.
The Core contract crate has no .43-to-.44 source change. The seeded-trace and route-log test updates change no own runtime expression.
All nine conformance sets pass the independent three-way check at each of the three train merges.
Final testkit pending is 403, real-pending 34, held four, real-only 26, proof entries seven, ledger 691, minimum 69.
Eight conformance text blobs match the pre-member parent. Only the two own IN-6 removals differ in core-pending.
All four findings remain closed. Formal Round 5 #307 train-delta source verdict is CLEAN on this exact head before the gate.
The lead's compile PASS and strict 264/0 pre-checks are reports; no separate retained log is supplied here.
Matching formal Sol member verdict, other member reviews, combined review, exact train gate, final receipts, Q7, and equal-tree merge remain due.
No reviewer job runs. No later train head or merge is accepted. Wait for the doorbell. Never poll.

The #307 exact train-delta source verdict is pushed at 76ae3ad37fc67b47cf2fbaea794cb77554b526d1. PC, Sol, and the lead receive it.
Sol receipt msg_plugin-w_1791822093_81313e supplies matching #307 and combined train-source CLEAN at 584b736e3dc90dc0114e573d9f4ed2372232d0ca.
The reviewer reads its complete #307 Round 4 and combined train-4 Round 1 verdicts and verifies publication.
Both #307 formal verdicts match train 03709a0c, base 838e4bb0, tree 4bb9e8c7. The matching formal source-review condition closes.
The combined record reports all six exact-head member source receipts read and CLEAN, with zero combined source findings.
The ordered .44 fix is present. The historical member gate stays RED. No completed train gate is accepted yet.
The exact train gate, final member evidence receipts, and lead Q7 merge/tree/push verification remain due. No reviewer job runs.
