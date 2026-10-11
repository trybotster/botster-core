# PC package reviewer handoff

- Reviewer session: `sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7`.
- Implementer session: `sess-1791628587-0178-4476c8a4ce35c7df2dc295defd77dda5`.
- Reviewer branch: `stage1/review-pc`.
- Scope: 38 program-control IDs after #222 and #227. The original PR #220 group had 32 IDs.
- Binding brief: `~/botster-sessions/shared/core-stage1/brief-program-controls.md`.
- Binding plan: `~/botster-sessions/pins/stage1-plan.4eceef06.md`, revision 23ak.
- Verified plan SHA-256: `db7ba8e5c0748aa53257fb53a5e6f8923a9549c6c9871127a7d2ef2c97000813`.
- Current code pin: `contracts-v0.1.27`, commit `b161db8`. Plan 23ak requires the PC pair to move it to `contracts-v0.1.29`, commit `65f76ab`, in separate HIGH PR #245.
- Initial review base: PR #220, `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
- Current reviewed head: `8cfdd6715c6a69c419d9819636e185bfff74bb24`.
- Current reviewed base: `7e8e07d6221603e8cb602bb25daed4cb6dd04c17` on `v1`.
- Current integration reviewer: Sol, `sess-1791654840-017a-4e0798223b6086de3e26b7b486e22973`, as relayed by the implementer.
- File split agreed with P6: `crates/botster-core-testkit/src/real/program.rs`, plus one dispatch line each in `real.rs`.
- Status: package NOT CLEAN (1 open) for PR #233 at `8cfdd6715c6a69c419d9819636e185bfff74bb24`. SI-5 awaits the green Linux gate. The focused Mac normal-run and reversal evidence are accepted.
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
