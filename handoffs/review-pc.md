# PC package reviewer handoff

- Reviewer session: `sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7`.
- Implementer session: `sess-1791628587-0178-4476c8a4ce35c7df2dc295defd77dda5`.
- Reviewer branch: `stage1/review-pc`.
- Scope: 38 program-control IDs after #222 and #227. The original PR #220 group had 32 IDs.
- Binding brief: `~/botster-sessions/shared/core-stage1/brief-program-controls.md`.
- Binding plan: `~/botster-sessions/pins/stage1-plan.c7cbc474.md`, revision 23ai.
- Verified plan SHA-256: `ea4b778d22a7421c0da60a224e7624ffe50e30799dfc919087b6e9787df9bd6b`.
- Required contracts pin: `contracts-v0.1.27`, commit `b161db8`. The PC pair owns the pin move.
- Initial review base: PR #220, `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
- Current reviewed head: `fce17f78b96009cb560fb3896a6f957840ec2c43`.
- Current reviewed base: `11a5246a669de9e8ae6b7ca1ed7763192a099781` on `v1`.
- Current integration reviewer: Sol, `sess-1791654840-017a-4e0798223b6086de3e26b7b486e22973`, as relayed by the implementer.
- File split agreed with P6: `crates/botster-core-testkit/src/real/program.rs`, plus one dispatch line each in `real.rs`.
- Status: package NOT CLEAN (1 open) for PR #233 at `fce17f78b96009cb560fb3896a6f957840ec2c43`. SI-4 LOW is open. Focused Mac evidence also remains pending.
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
