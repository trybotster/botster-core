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

## Documentation correction inspection — complete package delta remains due

Corrected head: `c3e1b41fb04a334eb9c5b864a6d3c68904743954`.
Tree: `97b65724575fb74d83c742af7e0de855b4a5a64a`.
Parent: `aed0c8e188d778960376b9a684741e9080feaa08`.
Original source/gate base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Delta READY: `msg_plugin-w_1791835573_f0a0fa`.
Complete correction diff SHA-256: `10178e7ff45565e1836d1ebfa6c1456504da3095541094116f4c365fcc02d1dc`.

The reviewer reads complete initial package Round 1 at `cc79293e4b4e035496488315a660bb95650fecbe`.
File: `verdicts/p5-reply-order.md`. Receipt: `msg_plugin-w_1791835601_c17439`.
It agrees with the source, gate, and reversal conclusions and identifies the same one LOW finding.
It notes that the reversal log also omits the exact test command and a restoration record.
Neither omission is supplied by an inferred result in this review.

### R325-F1 — LOW — CLOSED in the corrected source

The complete delta changes three files, with 27 additions and 21 deletions.
The rule moves to `crates/botster-worker-core/design/read-reply-order.md`.
DESIGN.md adds its topic index link. It leaves every old topic unchanged.
The topic text matches the original section after whitespace normalization, with its heading retained.
Its maximum line length is 120 columns.
The new test comment and complete body point to the topic file and index link.
The test's executable code remains byte-identical after documentation comments are removed.
The entire real-resume blob remains unchanged. No production, pin, list, or test behavior changes.
The documentation move therefore closes the required change without a new execution job.

Corrected body SHA-256: `b23f6889fff381641283b406397f83e1c929aeac0ade676bf5e0d14b26a14fe4`.
The body identifies this documentation delta and keeps the original gate at its actual aed0c8e1 head.
The original exact-head gate and reversal retain their earlier acceptance and limits.
Neither becomes execution of the corrected head.

Zero source or body findings remain on this corrected head.
The complete matching corrected-head package verdict remains due before the integration CLEAN round.
This inspection completes no CLEAN round and grants no final acceptance.
The current-base/train review, completed merging gate, final receipt, and Q7 remain due.
Package closure notice: `msg_plugin-w_1791835703_e7372a`.
The reviewer runs no execution job. Wait for the complete package verdict doorbell. Never poll.

## Round 2 — corrected source and body CLEAN

Head: `c3e1b41fb04a334eb9c5b864a6d3c68904743954`.
Tree: `97b65724575fb74d83c742af7e0de855b4a5a64a`.
Source base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Previous head: `aed0c8e188d778960376b9a684741e9080feaa08`.

The reviewer reads complete matching package Round 2 at `77bb78765226124d7de68090019e9be5dbdd4cd0`.
File: `verdicts/p5-reply-order.md`. Receipt: `msg_plugin-w_1791835726_c5a718`.
It is CLEAN on the exact corrected head and agrees with the independent source and body correction inspection.
The complete correction inspection at `98583e1092ace5df9bd944203b6eb59d70f7eac3` remains applicable.
R325-F1 is CLOSED in both reviews. Zero source or body findings remain, including LOW.

The reply rule, test code, production machine paths, and real-resume fix retain the complete initial source conclusions.
The topic link, identical rule words, line width, comment, and body references satisfy the documentation correction.
No new execution was requested or supplied for this correction.
The retained READY gate, reversal, and local observations stay scoped to the previous aed0c8e1 head.
This CLEAN accepts corrected source and body. It does not relabel that gate as execution of this head.
The current-base/train delta review, completed exact merging-head gate, final receipt, and Q7 remain due.
The pin that takes contracts #97 must retain the lead's #325 ordering requirement.
This verdict accepts no future source, gate, pin, status removal, retry, or merge.
The reviewer changes no product code and runs no execution job.

VERDICT: CLEAN (0 open; corrected source and body only)

## Round 3 — train 8 source delta CLEAN

Train head: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
Train tree: `3f6c95e20e26a54eca52908502a4bade7aec1cad`.
Train base: `007efbde6a59af403d0937c38361c137a8961a48`.
Frozen #325 member: `c3e1b41fb04a334eb9c5b864a6d3c68904743954`.
Member source base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Lead assignment: `msg_plugin-w_1791837736_4d4bf1`.
Scope: #325's source composition on this exact train head. Sol-B owns the combined review and ONE final train receipt.

The reviewer reads complete matching package Round 3 at `99bd443eb4ee268fe79dba2209dfa079aed630a3`.
File: `verdicts/p5-reply-order.md`. Receipt: `msg_plugin-w_1791838022_c9fec9`.
That package delta is CLEAN on the same train head, tree, base, and frozen member.
The original package and integration source CLEANs retain their exact member scope.

### Ordered composition and retained edits

The first-parent chain has these exact merges and member parents:

1. #325 merges at `ea89cbb8934ac6c5a4f0df70c635e2e364d1b600` from the train base and frozen member.
2. #327 `0e25e27da6606a27cf2ff94df5d139c80f27b4d5` merges at `3d8b0ae29dc9da81f6114dac92a314d0da9e5c2b`.
3. #324 `0572493fc9e695acd57b1916e485fdd900ba7643` merges at `5f8ad97b4effb96cd54d5500c444bf1b59ec7f5c`.
4. #329 `b9763b0f851a029a48af3a0f495388d69683f10b` merges at the train head.

The reviewer reads the complete #325 insertion and compares every added and removed line with the accepted member diff.
Every changed line matches, excluding hunk positions and object indices. The insertion changes four paths, with 111 additions and twelve deletions.
Insertion full-index diff SHA-256: `49ca99d5feab9f7a4983ed0ca6ced924e2644d7b266869d670479e92bf2ee00c`.
The train changes 26 paths, with 986 additions and 200 deletions.
Train full-index diff SHA-256: `08f5d55a7b100bddd1ee3551cc84157b4f5f88f77be654024e1a0f40bc74cf8a`.
The Git whitespace check passes.

The topic file, DESIGN.md index, and complete real-resume file match the member, first insertion, and final train byte for byte.
The complete reply-order test, including its comment, also matches across all three trees and occurs once.
Test-block SHA-256: `b9628332156dd154631aa9b2318f42879b05ccb98027bdc84e2a004c3bb498e4`.
R325-F1's documentation closure remains valid.

### Worker and host boundary

The imported World fixture now answers PTY resize actions. It still suppresses every LinkWritten input when `instant_link` is false.
The reply-order test feeds PTY output and ReadScreen. Neither input creates ResizePty, so that fixture adds no resize-answer turn here.
The test retains its pending Output, later revision, Title, Modes, final Output-revision assertion, and last Done assertion.

After the #325 insertion, #327 adds only `mod exit_watch;` to the shared worker test file.
The reviewer reads that complete module and the worker-machine delta at this boundary.
Its tests own separate World instances. Its new failure input and failure exit action do not replace a read or output path.
The failure path ends the worker without fabricating a successful read completion.

The reviewer independently confirms that report, send_report, on_output, output_advanced, and report_output match the accepted member exactly.
The complete model.rs blob also matches, including feed_model, after_step, the four reads, and capture page reporting.
The final on_op retains all four read arms and the common report of Done.
Reports still enter one ordered LinkSend queue. The host retains link order when it applies Observed and later Done messages.
The imported host-driver change only exposes control_bound within its crate. It changes no delivery or observation order.
Thus the source retains the same-turn model observations and pending Output flush before each read reply.

### Real-resume and contracts scope

The imported capture layer records SizeChanged within the existing size/profile exclusion.
The successful resume comparisons perform no resize before their oracle reads. The later resize exclusion assertion remains present.
The layer still forwards the same polled events and records matching capture completions. It holds or re-injects no event.
The distinct final-byte title sentinels, s2 then s1 ReadScreen completions, quiet check, and both oracle comparisons remain exact.

The reviewer reads the other members' affected worker-edge and input-log interfaces at this boundary.
They change exit-watch failure delivery and the representation of pty_input logs.
They change neither the real-resume output history nor its title, read-completion, or oracle paths.
The separate members' full behavior remains in their assigned reviews and Sol-B's combined review.

Cargo.toml and Cargo.lock match the train base exactly and pin contracts `.46`, commit `7e97064c0dc8fe8311c86aea4bac1e4b3663d6b1`.
The optional RouteAdopted tag and owner fields change no read or resume interface used here.
The contracts driver delta from `.44` to `.46` copies bindings only for pty_input reads. It changes no oracle-resume fence.
The three R-114 st_6b transcript blobs remain byte-identical between `.44` and `.46`.
Each still calls oracle_resume without the later ReadScreen/Completed pair.

The reviewer reads the complete contracts #97 delta again and compares the corresponding `.49` transcripts.
Contracts `.49`, commit `5ce2cff1193dda65b51fbddab2c740dbf12efa32`, contains the matching s1 ReadScreen and successful Completed immediately before each oracle.
This train does not pin `.49`. This review claims no execution of those future fences.
The lead's requirement that #325 precede the pin that takes #97 remains applicable.
The old `.44` READY gate and real-resume test are not execution evidence for contracts #97's transcript changes.

### Conformance accounting and disposition

The reviewer checks all nine conformance text sets after every merge and against every merge-base candidate.
The #329 merge has two candidates. All 45 set formulas pass, including the not-applicable deferred row.
Every merged set equals the prior parent set minus the member's removals, plus the member's additions against that merge base.
The #325 insertion changes no set. It restores no prior removal and removes no identifier.
Final counts are ledger 692, pending 385, real-pending 38, held 4, minimum 69, and real-only 26.
The proof file retains the base's seven exact rows for four distinct ids.
Deferred text retains three rows, including one active not-applicable case. Withdrawn retains 27 ids.
The three R-114 st_6b ids remain absent from pending, real-pending, held, and real-only lists.
The other members own the three pending removals. This delta grants no new removal authority.

The lead reports id-set PASS, workspace compilation with tests PASS, and strict conformance 281/0.
Those statements are operator pre-check reports. No retained execution log accompanies this delta assignment.
The earlier member gate, reversal, local evidence, and body keep their original heads and recorded limits.
No completed train gate is supplied or accepted here. Zero source-composition findings remain, including LOW.
Sol-B's combined review, completed exact-head gate review, ONE final train receipt, and lead Q7 remain due.
This verdict accepts no future head, retry, pin, gate, or merge.
The reviewer changes no product code and runs no execution job.

VERDICT: CLEAN (0 open; #325 train 8 source delta only)
