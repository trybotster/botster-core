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
