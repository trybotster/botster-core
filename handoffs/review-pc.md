# PC package reviewer handoff

- Reviewer session: `sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7`.
- Implementer session: `sess-1791628587-0178-4476c8a4ce35c7df2dc295defd77dda5`.
- Reviewer branch: `stage1/review-pc`.
- Scope: 38 program-control IDs after #222 and #227. The original PR #220 group had 32 IDs.
- Binding brief: `~/botster-sessions/shared/core-stage1/brief-program-controls.md`.
- Binding plan: `~/botster-sessions/pins/stage1-plan.0af4db8d.md`, revision 23au.
- Verified plan SHA-256: `8c4f8bb51f2d3defe02822c4ed0b03d6f7a08f5b693950d4746fd6fd8139e3d9`.
- The last #233 source review uses contracts-v0.1.29. The new design base uses contracts-v0.1.32 under plan 23ar.
- Initial review base: PR #220, `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
- Last #233 reviewed head: `a7be76d55ed38bc89d6a8136c4c62d5d2c9bdb3a`.
- Last #233 review base: `9ef7e57b857a551e5b66a84d0953630186cb51e7` on `v1`.
- Current integration reviewer: Sol, `sess-1791654840-017a-4e0798223b6086de3e26b7b486e22973`, as relayed by the implementer.
- File split agreed with P6: `crates/botster-core-testkit/src/real/program.rs`, plus one dispatch line each in `real.rs`.
- Active review: PR #264 at `46094d5b504861e59b66ac8cf16b399841d7ec71`, base `010f2a49952f9b77581f304dbaa41006b115d40b`. Package CLEAN (0 open).
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
