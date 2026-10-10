# PC package reviewer handoff

- Reviewer session: `sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7`.
- Implementer session: `sess-1791628587-0178-4476c8a4ce35c7df2dc295defd77dda5`.
- Reviewer branch: `stage1/review-pc`.
- Scope: the 32 program-control IDs in `conformance/core-real-pending.txt` at PR #220.
- Binding brief: `~/botster-sessions/shared/core-stage1/brief-program-controls.md`.
- Binding plan: `~/botster-sessions/pins/stage1-plan.77739f96.md`, revision 23ab.
- Verified plan SHA-256: `969711aafa5e79cd617b3a6b9777ac6042c2116c038b9fc8a20b5ca6572dc682`.
- Required contracts pin: `contracts-v0.1.27`, commit `b161db8`. The PC pair owns the pin move.
- Review base: PR #220, `6c1624e940cd98da24690b1224fc1c5fa8893c6e`.
- Initial implementation base reported: `c19ad938bd87c60331bd002c0117102d8756b1ae` on `v1`. No program-controls implementation head has arrived for review.
- Current integration reviewer: Sol, `sess-1791654840-017a-4e0798223b6086de3e26b7b486e22973`, as relayed by the implementer.
- File split agreed with P6: `crates/botster-core-testkit/src/real/program.rs`, plus one dispatch line each in `real.rs`.
- Status: PR #231 is CLEAN on `4521237e8200768a36ab4c8a27f268f3914a88ab`. The program controls still await an implementation review head.
- Contracts dependency: PR #30 has merged and shipped as `contracts-v0.1.27`.
- The contracts PR adds the probe control socket and the `ad_3` output fence under steward approvals Q1 and Q2.
- The contracts crate-catchup reviewer owned that PR's review. The PC Core review waits for its pin move.
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
