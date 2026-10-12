# #325 reply-order integration review

## Round 1 — initial head NOT CLEAN

Head: `aed0c8e188d778960376b9a684741e9080feaa08`.
Tree: `a4ce9c3a1d3e4a0c8867039ff20e13001dd989a1`.
Parent of the first commit and gate base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
First commit: `fd0afe925c294ac6dfdcd84694ef62ece28f321c`.
READY receipt: `msg_plugin-w_1791835229_d9d4f1`.
The lead assigns package reviewer 0167 and Sol under R-114 (B) (2).
The pushed member still equals this head. Remote v1 has since moved to train 6, `3e655eff46d36f8c337339a1a17d401c8c662594`.

The reviewer reads the complete three-file diff: 105 additions and twelve deletions.
Full-index diff SHA-256: `855801590a1fedd06fe163df357a08b7760316edf6cfeebd22613872f22e32c8`.
The Git whitespace check passes.
The real-resume production prefix remains byte-identical to the base; all changes there occur in its test module.
Its final blob equals the original P6 commit `6415b1c9854d0b7deacaf032819a1d0d8a2f4a40` exactly.
The cherry-pick retains its `-x` provenance. The worker production implementation does not change.

### Rule and cross-package fence

The reviewer reads complete R-114 at contracts `e22baecc1be444c96c3b7c099162eeb53b605164`.
The reviewer also reads R-115's freeze disposition at the current contracts ruling file.
R-114 requires the reply-order rule, its Core test, and the real-resume fix.
R-115 stops the new lint pair; its rule becomes a reviewer checklist item. This PR introduces no xtask check.
The lead's handoff confirms these assignments and requires #325 before the pin that takes contracts #97.

The reviewer reads contracts #97's complete three-transcript delta at merge `86d612068156549e02e5da235d6e6cacf99dd337`.
Each transcript begins ReadScreen on s1 and waits through its matching successful Completed before oracle_resume.
That delta depends on the ordering rule inspected here. This Core head still pins `.44` and does not import those transcripts.
The old-pin gate is not proof of executing contracts #97's new fences.

In the production worker, `feed_model` applies output and calls `after_step` in the same input turn.
`after_step` reports the model observations through `report`.
`output_advanced` sends the Output report or retains its pending state while the previous report remains unwritten.
The four read operations use the same `on_op` reply path, which calls `report` for Done.
`report` flushes a pending Output report before it sends that Done.
Each report enters one ordered LinkSend action queue.
The host consumes worker messages in link order. It updates observations before it handles the later Done and completes the read.
The rule covers bytes already applied by the worker. It covers neither unapplied suffix bytes nor later output.
The new design text retains that scope and explicitly excludes the unfinished-string ESC case.

The new machine test stops automatic LinkWritten inputs to model a slow host.
It feeds two output turns, then a ReadScreen operation.
It requires Done last, with Title and Modes observations before it.
It also requires the last Output report before Done to carry the screen's model revision.
The test uses the production machine through its input/action interface and the existing frame decoder.
It introduces no hand-written expected terminal encoding or production test branch.

The real-resume test moves each unique noise title to the last bytes of that session's output.
The titles `s2-1` and `s2-2` occur in distinct output windows and are not reused as prior session titles.
The existing s1 BEFORE and AFTER outputs also end with distinct title sentinels.
After those model fences, the test completes ReadScreen on s2 and then on s1 before the oracle reads output histories.
The generalized completion helper drains polled events and requires the matching operation id.
Its callers reject a wrong result. The helper retains the existing bounded wake wait and deadline value.
No sleep, new timer, or timeout extension enters the patch.
The later quiet-state, capture identity, damaged-capture, pending-event, resize, and unknown-history assertions remain present.
No event is held or re-injected, and the production oracle control remains unchanged.
The source inspection identifies no additional cross-package fence defect.

All conformance blobs and Cargo.toml/Cargo.lock remain byte-identical to the base.
No identifier leaves pending or real-pending. No known-flaky entry or new pin enters this PR.
The R-114 st_6b identifiers retain their active status.
The future current-base import must preserve train 6's shared worker tests and this new test together.
No future resolution is accepted by this initial review.

### Completed initial READY gate

Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-reply-order-aed0c8e1-pool-20261012-124520-63978.log`.
Verified SHA-256: `4b9b74dd80f029d8a90ae22843193c321f55ec5ac8bdcddd4309b48d17d53904`.
The retained log names this exact head, base, msa1, and both concurrency caps at four.
All ten CI steps PASS. CI total is 407.9 seconds.
The job and wrapper exit zero after 857 seconds: 369 seconds queued and 488 seconds running.

Default reports 1999 passes, 427 skips, and 12.486 seconds.
Slow reports 663 passes, 2078 skips, and 42.374 seconds.
The reviewer independently parses both complete PASS ordinal sets and unique test identities.
Each set matches its denominator. The slowest default test takes 1.131 seconds.
No FLAKY, failure, timeout, panic, SIGSEGV, or SIGABRT result appears.
The new reply-order test passes at 0.003 seconds. The changed real-resume test passes at 0.096 seconds.
Both strict summaries report 265 passes, zero failures, and 427 ignored.
Held-full reports 96 ignored, zero failures, and 22.22 seconds.
Mutation selection finds no mutant and starts no run. Fuzz selects no changed decoder harness.
Neither selection supplies a new mutation catch or fuzz execution.
This gate establishes execution of the original READY head only. Current-base/train acceptance remains due.

### Retained reversal and local limits

Reversal log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-reply-order-aed0c8e1-reversal-send-report.log`.
Verified SHA-256: `a54cc66e4ca7b84863f15bf46afc19566353c9a05e723986f6dffcd7343506a0`.
The complete log names this head and shows its one-line scratch change from `report` to `send_report` for Done.
The new test fails at its Output revision assertion: actual ModelRev(1), expected the screen's ModelRev(2), exit 101.
Its Title and Modes observations remain present. The normal exact-head gate has PASS for the same test.
This is a targeted test-sensitivity reversal, not a scheduling reproduction campaign or mutation catch.
The log references the prior clean in the same worktree. It retains no numeric restoration result or restored local-suite run.
The reviewer infers neither from this log.

Local log: `shared/core-stage1/evidence/p5-reply-order/checks-aed0c8e1.log`.
Verified SHA-256: `7cfb1beb2882f07599f468ec5d1c4f60808bc0d0971180c01ccb33261b0c4270`.
It names the exact clean head and target, records a clean of 4253 files and 785.5 MiB, and gives clean exit zero.
It records zero exits for fmt, nine-crate clippy, the named crate tests, conformance, lists, prebuild, and the focused real-resume test.
The body reports BOTSTER_SEEDS=0–31 for the local strict run; the printed command omits that environment assignment.
Local Mac strict, slow, and timing observations remain supplementary, not gate evidence under pair-common.
The pool gate supplies this review's execution acceptance.

### R325-F1 — LOW — OPEN: put the new design topic in its own file

Package finding notice: `msg_plugin-w_1791835384_054ad1`.
The reviewer independently confirms the finding against the complete diff and current pair-common.
The new reply-order section is appended to `crates/botster-worker-core/DESIGN.md`.
Pair-common, "Design documents: one file per topic", requires a new topic file and its DESIGN.md index link.
Move this section to a file under the crate's `design/` directory. Add the index link in DESIGN.md.
Update the test comment and complete PR body references. Wrap the topic prose at 120 columns.
No existing topic needs a split in this PR. The index link preserves R-114's requirement to state the rule.
The reviewer requests no product change or new execution for this documentation finding.
Owner notice: `msg_plugin-w_1791835459_fbd5f8`.

Initial body SHA-256: `a30d2eac00a88ac7ec444f0f1abf61f3cea0f99cc0d91ab55c00f559885ea968`.
It correctly scopes the gate to the old base and separates the local observations and empty selections.
Its references to the new topic must change with the documentation fix.
The complete matching package verdict remains due; the package reviewer is still reviewing this head.
This record completes no CLEAN round. Wait for the corrected delta and complete package verdict doorbells.
The applicable final import, completed merging gate, final receipt, and Q7 remain due.
The reviewer changes no product code and runs no build, test, gate, mutation, reversal, reproduction, measurement,
or executable base check.

VERDICT: NOT CLEAN (1 open, LOW)
