# P5 read reply order review — #325, R-114(B)(2)

## Round 1 — 2026-10-12 — first READY

- Head: `aed0c8e188d778960376b9a684741e9080feaa08`.
- Tree: `a4ce9c3a1d3e4a0c8867039ff20e13001dd989a1`.
- Base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
- First commit: `fd0afe925c294ac6dfdcd84694ef62ece28f321c`, whose parent is the base.
- READY receipt: `msg_plugin-w_1791835229_519c10`.
- Risk: HIGH, correctly stated. The shared testkit changes and the worker rule supports a conformance fence.
- Integration assignment: `msg_plugin-w_1791835254_64ef12`.

I read R-114(B), audit rows 10–11, the current pair-common rules, the complete three-file delta, affected source,
the complete PR body, gate records, and reversal record. The delta has 105 additions and twelve deletions.
Full-index SHA256: `855801590a1fedd06fe163df357a08b7760316edf6cfeebd22613872f22e32c8`.
Git whitespace check passes. No product source, contract pin, conformance status, fixture API, or timeout changes.

### Source conclusions

The rule matches the existing machine. feed_model applies output and drains model events in the same machine input.
after_step reports Title, Modes, and the other model observations before on_output calls output_advanced.
Backpressure may retain the newest Output revision. All four read operations finish through the common on_op report path.
That path flushes output_unsent before it queues Done. Capture pages also use report.
The action queue preserves LinkSend order. The real driver appends each send to the ordered outbound queue.
The host decoder delivers frames in order, holds a refused frame before later frames, applies observations, and then completes Done.
A Completed read after a valid model fence therefore consumes the earlier reports. It promises nothing for bytes still unapplied.

The new test controls the machine through inputs and actions. It disables instant link completion, applies a first output,
then applies a title and cursor mode change before ReadScreen.
It requires Done to be the last report, requires Title and Modes before it, and compares the latest preceding Output revision with the screen revision.
The common reply path gives this test scope across the four reads without four copies of the same branch test.
The test uses libghostty's output and the returned revision; it introduces no terminal emulator or golden terminal byte assertion.

The testkit resume change is an exact blob match to P6 commit `6415b1c9854d0b7deacaf032819a1d0d8a2f4a40`.
The cherry-pick trailer names that commit. I review the change directly rather than relying on the prior acceptance claim.
The noise titles move after all noise bytes. The two s2 sentinels and two s1 sentinels are distinct per session.
After the final title waits, s2 and s1 each complete ReadScreen before the oracle reads.
The generalized completion helper checks the requested OpId, accepts intervening events, and checks the typed result in each wrapper.
It retains the existing wake wait and cleanup deadline. No sleep, new timer, timeout extension, or retry is added.
The existing quiet check, capture comparison, fault injection, unsupported-control checks, and process ownership remain.
These changes address both audit rows and preserve the two independent resume comparisons.
The complete body has a prior-art note and states the source-only production scope correctly.

### Completed READY evidence

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-reply-order-aed0c8e1-pool-20261012-124520-63978.log`.
Verified SHA256: `4b9b74dd80f029d8a90ae22843193c321f55ec5ac8bdcddd4309b48d17d53904`.
The retained log has 6090 lines, names this exact head and base, and runs on msa1 with both concurrency caps at four.
The recorded base is also the PR ancestor and merge base. This is READY evidence, not final current-base or train acceptance.

All ten CI steps PASS; total 407.9 seconds. Job and wrapper exit zero after 857 seconds, queued 369 and running 488.
Default has 1999 passes, 427 skips, 12.486 seconds. Slow has 663 passes, 2078 skips, 42.374 seconds.
I independently parse complete PASS ordinals, denominators, and unique test identities in both sections; both audits pass.
The new order test passes at 0.003 seconds. The real resume test passes at 0.096 seconds. No FLAKY token occurs.
Both strict summaries report 265 passes and zero failures; strict runs 316 of 333 pending identifiers with seventeen real-only not run and four held.
Held-full has three budget identifiers over 32 seeds each: 96 ignored, zero failures, 22.22 seconds.
Coverage checks 1098 slow-only tests and seven named real proofs. All 27 passing ignored real identifiers remain real-pending.
The complete own diff changes no list. No suggested removal is accepted by this review.
Mutation selection finds no mutant and starts no run; fuzz selection finds no changed decoder harness.
Neither result is a new mutation catch or fuzz execution.

The complete 55-line reversal log has SHA256 `a54cc66e4ca7b84863f15bf46afc19566353c9a05e723986f6dffcd7343506a0`.
It names this head and the prior local clean, shows the one-line scratch change from report to send_report for Done,
and reports exit 101. The new test fails with last Output revision 1 and screen revision 2.
This supports the flush assertion's sensitivity. It is not a normal PASS, pool gate, or accepted cargo-mutants catch.
The log does not retain the exact test command or post-reversal restoration record; no reviewer reversal is run.

The local checks record has SHA256 `7cfb1beb2882f07599f468ec5d1c4f60808bc0d0971180c01ccb33261b0c4270`.
I inspect its head, clean, command boundaries, numeric exits, summaries, and target result.
It records clean of 4253 files and 785.5 MiB, fmt, nine-crate clippy, unit tests, strict, lists, prebuild, and the local resume PASS.
Each top-level recorded command exits zero. It does not independently establish every prior target writer or retain command-line environment values.
The body correctly labels it supplementary. Local strict, slow, and timing observations supply no gate acceptance.

### R325-F1 — LOW — OPEN — new design topic must have its own file

The new reply-order topic is inserted in worker-core/DESIGN.md at lines 34–51.
Current pair-common, "Design documents: one file per topic", requires a new topic in design/<topic>.md with an index line in DESIGN.md.
This reduces independent topic conflicts and prepares the accepted design split.
Move this new section to its own topic file, add its index link, and update the test comment and PR body references.
Wrap the topic prose at 120 columns. No existing topic needs a split here.
R-114's requirement to state the rule remains satisfied through the DESIGN.md index link.
Finding receipts: P5 `msg_plugin-w_1791835384_f5e2e5`, Sol `msg_plugin-w_1791835384_054ad1`.
This finding requests no product change or execution job.

### Disposition

One LOW finding remains. HIGH requires every finding to close before CLEAN.
No source-behavior or READY-gate finding is identified. Send correction READY for the documentation delta.
The existing gate names only the reviewed head; it accepts no corrected head or final integration.
Matching integration CLEAN, applicable current-base or train reviews, completed merging gate, final receipt, and Q7 remain due.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge check.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-12 — documentation correction

- Head: `c3e1b41fb04a334eb9c5b864a6d3c68904743954`.
- Tree: `97b65724575fb74d83c742af7e0de855b4a5a64a`.
- Parent and previously reviewed head: `aed0c8e188d778960376b9a684741e9080feaa08`.
- Branch base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
- Delta READY: `msg_plugin-w_1791835573_5de0f7`.
- Prior package verdict: `cc79293e4b4e035496488315a660bb95650fecbe`.

I read the complete one-commit, three-file correction: 27 additions and 21 deletions.
Full-index delta SHA256: `10178e7ff45565e1836d1ebfa6c1456504da3095541094116f4c365fcc02d1dc`.
Git whitespace check passes.

### R325-F1 — LOW — CLOSED

The rule now lives in crates/botster-worker-core/design/read-reply-order.md.
DESIGN.md indexes that topic with its retained title. It is the only topic file, so index order is correct.
I independently compare the old section and new topic: all rule words remain identical after whitespace normalization.
The topic's maximum line width is 120 columns.
The test comment names the new topic. Every executable line of the worker test file remains exact after excluding doc comments.
The real resume file remains byte-identical to the first READY head.
No existing topic, product source, fixture, test assertion, input, deadline, list, or dependency changes.

I read the complete corrected body on this exact head and v1 PR base.
It names the topic and DESIGN.md index, identifies this correction, and preserves the historical gate's actual head.
Its source claims, prior-art note, HIGH tier, and reviewer assignment remain applicable.
R325-F1 closes in this source and body correction.

### Disposition and evidence limits

Zero package findings remain, including LOW. Round 1's source conclusions and evidence audits retain their stated scope.
The completed READY gate, reversal, and local observations belong to `aed0c8e188d778960376b9a684741e9080feaa08`.
No new execution is supplied or claimed for this documentation correction.
The unchanged tests and rule preserve their reviewed behavior; the old gate is not relabeled as execution of this head.
This CLEAN accepts the corrected source and body, not final current-base or train acceptance.
Matching integration CLEAN, applicable current-base or train delta review, completed exact merging-head gate, final receipt, and Q7 remain due.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge check.

VERDICT: CLEAN

## Round 3 — 2026-10-12 — train-8 source delta

- Train head: `2d8e8acc52eca692a089af02b4ace508d6694cc3`.
- Train tree: `3f6c95e20e26a54eca52908502a4bade7aec1cad`.
- Train base: `007efbde6a59af403d0937c38361c137a8961a48`.
- Frozen #325 member: `c3e1b41fb04a334eb9c5b864a6d3c68904743954`.
- Package member CLEAN: `77bb78765226124d7de68090019e9be5dbdd4cd0`.
- Matching integration member CLEAN: `c376b183fbb11048debef0284c0979ffb657a365`.
- Lead request: `msg_plugin-w_1791837736_91ee87`.
- Integration scope assignment: `msg_plugin-w_1791837796_e923ef`.

This round reviews only #325's fit on this train head. Sol owns its integration delta.
Sol-B owns the combined review and one final train receipt. No completed train gate is supplied or reviewed here.

### Ordered composition

The four merges retain this order and their exact member parents:

1. #325 at `ea89cbb8934ac6c5a4f0df70c635e2e364d1b600`, joining the train base and frozen #325 member.
2. #327 `0e25e27da6606a27cf2ff94df5d139c80f27b4d5` at `3d8b0ae29dc9da81f6114dac92a314d0da9e5c2b`.
3. #324 `0572493fc9e695acd57b1916e485fdd900ba7643` at `5f8ad97b4effb96cd54d5500c444bf1b59ec7f5c`.
4. #329 `b9763b0f851a029a48af3a0f495388d69683f10b` at the train head.

I read the complete #325 insertion and independently compare it with the accepted own patch against its original base.
Every added and removed line remains exact, excluding hunk positions and object indices.
The insertion has exactly four own paths, 111 additions, and twelve deletions.
Insertion full-index SHA256: `49ca99d5feab9f7a4983ed0ca6ced924e2644d7b266869d670479e92bf2ee00c`.
The complete train has 26 paths, 986 additions, and 200 deletions.
Full-index SHA256: `08f5d55a7b100bddd1ee3551cc84157b4f5f88f77be654024e1a0f40bc74cf8a`.
The Git whitespace check passes.

### Shared worker fixture and reply rule

The topic, DESIGN.md index, and complete real-resume file remain byte-identical to the accepted #325 head and first merge.
The complete new read-order test, including its comment, also remains exact.
I read the entire member-to-train worker test delta and affected final fixture contexts.
The base imports its resize tests and the World resize-answer field and loop.
With instant_link false, the loop still supplies no LinkWritten input.
This test feeds only output and ReadScreen; neither emits ResizePty, so it generates no automatic resize-answer input.
The first pending Output and later revision therefore retain the test's original backpressure shape.
Frame decoding, action collection, and the final Title, Modes, Output-revision, and Done assertions remain unchanged.

#327 adds only mod exit_watch to that shared test file after the first merge.
I read the full new module and the complete worker machine and binary delta from this train member.
It adds a failed exit-watch input, failure action, early failure slot, and failed end handling.
It replaces no output or read function. Its tests use separate World instances and module-local state.
I independently compare report, send_report, on_output, output_advanced, and report_output with the accepted #325 source: all remain exact.
The entire model.rs file remains exact, including feed_model, after_step, the four read functions, and capture page reporting.
The final on_op retains all four read arms and the common report of Done.
The driver retains its ordered LinkSend path. Watch failure is a failed worker end, not a fabricated successful read completion.
Thus the same-turn model observations, pending Output flush, and FIFO reply rule remain supported by the composed source.

### Real-resume and pin context

I read the imported host-driver and capture-layer differences from the accepted member.
The host-driver change exposes control_bound within its crate; it changes no frame delivery or observation/Done order.
The base capture layer adds observation of SizeChanged to the existing size/profile exclusion.
The successful resume proof does no resize before its oracle reads, and the later resize exclusion check remains present.
The layer still polls the same events and records capture completions; no event is held or re-injected.
The unique final s2 titles, s2 then s1 ReadScreen completions, quiet check, and both oracle comparisons remain exact.

The other testkit member changes route/input log representation and simulated pending/program work.
I read its changed real route and shared pty_input helper interfaces: they return the same hex representation at their input boundary.
They alter no real resume output history, title fence, completion helper, or oracle production prefix.
The shared real harness, host delivery, model, and capture-layer files receive no additional train-member edits beyond the imported base.
The separate members' execution, pending, exit-watch, and gate-decision behavior remains in their assigned reviews and the combined review.

Cargo.toml and Cargo.lock remain equal to the train base, which pins contracts .46.
The .46 RouteAdopted optional tag/owner change touches no read or resume interface used here.
Contracts #97's later transcript fences are not in this pin; the earlier .44 READY gate proves no execution of those later fences.
The lead's requirement for #325 before the pin that takes #97 remains applicable.
No pin, identifier removal, known-flaky disposition, or future transcript execution is accepted by this member delta.

### Lists and disposition

I independently check all nine conformance text sets after every merge and against every merge-base candidate.
#329 has two merge bases. All 45 comparisons pass.
The #325 insertion leaves every list equal to the train base and changes no identifier.
Final pending has 385 identifiers, three removals from the base and no additions; those removals belong to the other members.
No base removal returns through #325.

Zero #325 source-composition findings remain, including LOW. R325-F1 retains its closure.
The earlier source, body, gate, reversal, and local reviews retain their original heads and stated limits.
The lead reports id-set checks, workspace test-target compilation, and strict conformance at 281/0.
These are operator pre-check reports, not a completed train gate or independent execution evidence.
Matching integration delta, combined review, the completed exact-train-head gate, final receipt, and merge verification remain due.
Under the current Green rule, Sol-B sends the single final train receipt if this member stays unchanged.
This verdict accepts no gate, future head, retry, or merge.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge check.

VERDICT: CLEAN
