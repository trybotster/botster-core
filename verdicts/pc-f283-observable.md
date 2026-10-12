# Core #290: F-283-1 source and evidence review, Round 1

VERDICT: NOT CLEAN (1 open)

Reviewer: sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7.
Reviewed head: `4fb31c10bde8eba1b947f7a142e88568bfb22cd6`.
Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
Head tree: `53b3a19b53e044bad1ce016bb0e52d31b8433a70`.
Risk: HIGH, because the testkit is a shared crate.
READY: `msg_plugin-w_1791760528_aebdc7`.

The reviewer read brief-program-controls.md first and followed its reading order.
The governing records are the contracts, BUILD.md, plan 23cm, and the lead's widened F-283-1 scope.
The current plan hash is `c77c243490be26d6e4ee9f5b9491fd8b62d6dc9827dceebccfd864e2d60abf02`.
The reviewer read all three changed files, the remaining program assertions, the runtime callers, and the retained evidence.
No reviewer build, test, gate, mutant, reproduction, or executable base check ran.

## F283-PC-1 — MEDIUM, OPEN: private-state assertions remain in the widened scope

Location: `crates/botster-core-testkit/src/program.rs:779` and the tests at lines 794–802 and 854–862.
The tests still assert `ignores_sigterm()` and `window_size()`.
Each getter returns a private field.
The only callers in the reviewed tree are tests.
Declaring a getter `pub` does not make a field assertion a proof of program behavior.

`ignore_sigterm_applies_at_its_script_step` checks the stored `ignores_sigterm` flag before and after the script step.
The script-validation test also checks that getter.
The behavior to check is whether `signal(SIGTERM)` ends the program, as observed through `poll_exit`.
The existing disposition test covers a reached `ignore_sigterm` at the start. It does not replace the before/after step coverage.

`the_window_size_is_kept` checks only that `resize` stores `self.size`.
No runtime caller observes `window_size` in the reviewed tree.
This has the same state-access pattern as the removed `output_unread` assertions.

Required change:
- Replace the SIGTERM getter assertions with signal and exit behavior before and after the script step.
- For window size, use an observable program or worker behavior, or remove the state-only test and name the remaining clause coverage.
- Audit the remaining program getters under the same rule.
- If a getter or its stored state has no runtime use, remove dead code in a separate commit and state the effect.

The lead widened F-283-1 to every private-state assertion in the program-edge tests.
This finding is therefore within #290's scope, although these assertions predate the PR.
The reviewer sent the finding to PC and Sol-B in `msg_plugin-w_1791760695_88aa9b` and `msg_plugin-w_1791760695_02f156`.

## F283-PC-2 — LOW, CLOSED: the live body described a pending gate

The first body read still said the gate was pending and named Sol as integration reviewer.
PC corrected the body without a source change in `msg_plugin-w_1791760651_84ede0`.
The reviewer read the corrected body.
It names the completed ten-step PASS gate, exact base, log, eight caught mutants, and Sol-B's current assignment.
Body SHA-256: `43f7d8e7312f1902468ed2543fbde3bc85d005bac0cbff73825c849e3dea7926`.

## Source and evidence checks

The production change removes `ProgramControl::output_unread`, its unused counter, and the counter's updates.
The runtime `ScriptedProgram::unread` remains unchanged and still bounds the worker's drain.
The reviewed tree has no reference to `output_unread` or `controls.unread`.
The removed updates do not change injected bytes, output pieces, reads, writes, or the input log.

The rewritten held-write tests use Core completions, `pty_queued`, `pty_input`, and pump `more`.
The rewritten output tests use program reads and Core screen reads.
The bounded-drain test checks exact output bytes before and after `PtyDrained`.
The `accept != Some(0)` guard is reachable because `Ready::PtyWrite` offers a pending write before readiness gates later retries.
The new Core test observes the extra `more` caused by removal of that guard.
The empty-write guard still proves `write(b"") == Ok(0)` through the program interface.

The exact-head gate is:
`~/botster-sessions/gates/botster-core-stage1-pc-f283-1-observable-4fb31c10-pool-20261011-160304-16491.log`.
It names base `5e953398` and passes all ten steps.
Default: 1792 passed. Slow: 584 passed.
Mutation total: eight caught, zero missed, zero timeout, zero unviable.
All eight raw caught identities match the eight named test citations.
Stage 2 does not run because Stage 1 misses no mutant.
The gate contains no FLAKY line.
Gate SHA-256: `5747119a8ba4d52ce4017f03bb403045b61bebfb620830a3545c6feea3ab72b5`.

The retained local reversal log records four separate failures and a restored 25/25 PASS.
R1 removes the accept guard; R2 removes the kernel-room step path; R3 ignores the block; R4 keeps the refusal across steps.
Each failure names an observable test. These are supporting local records, not the merge gate.
The log does not record the clean command or each test command's exit code.
The exact-head pool gate independently passes the relevant tests.
The local default log totals 1135 passed; the static log states successful checks.
The clippy log is empty. The gate's clippy PASS is the independent validation of that check.

Evidence directory: `~/botster-sessions/shared/core-stage1/evidence/pc-f283/`.
Reversal log SHA-256: `22e936e420e557f69f2535fe5aa33361bd56d9c974a1467832b7b3a0c4282d6a`.
Local default log SHA-256: `dc9eac7917aa75cabb2ba9951b8d7fed0524160f26439eb11c1b1ff8de4bf976`.
Static log SHA-256: `a3d0bc2e1731da896ff6daa49d75139075ca8d36594feb98587c58c0d62b4054`.

## Next review

One source finding remains open on `4fb31c10`.
PC must send a fixed-head READY with completed evidence.
The HIGH-tier integration review, final-slot checks, final gate, and Q7 remain due.
The reviewer waits for the doorbell and does not poll.

## Round 2: repaired source and corrected coverage statement

VERDICT: CLEAN (0 open package findings)

Reviewed head: `cc491863b4143d37a3e9586eb1848db0bb9c3a5f`.
Gated parent: `c542a9fd93be950cb77522257b1b4e3741920a52`.
Imported source base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Gate comparison reference: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
Gated parent tree: `32b186e9802f7130b5aa221f0ef2b04871084b04`.
READY: `msg_plugin-w_1791763810_2709b7`.
Comment correction receipt: `msg_plugin-w_1791764392_448eb6`.
The reviewer reads the brief first, all repair changes, remaining getters, runtime callers, and the retained evidence.
The earlier complete review covers the two unchanged test files and unchanged production paths.
The lead approved the #287 import. The own diff changes only three testkit files and no conformance list or pin.
Plan 23cm, contracts, BUILD.md, and the widened F-283-1 scope govern this review.
No reviewer execution job runs.

F283-PC-1 closes in source.
Fresh program fixtures observe SIGTERM through `signal(15)` and `poll_exit` before and after the `IgnoreSigterm` step.
The before-step fixture exits with signal 15. The after-step fixture remains alive.
The after-Hold fixture exits with signal 15 before the later ignore step can run.
The validation test adds Hold so automatic exit cannot hide signal behavior.
The unused `ignores_sigterm` getter is removed.
The unused `window_size` getter, size field, constructor state, and state-only test are removed.
Resize returns `Ok(())`; the program has no terminal driver or step that reads that size.
Removal of the stored size changes no observable program behavior.

The remaining getters have runtime callers.
`queued` supplies `pty_queued`; `input_log` supplies PTY and route observations.
`waits_for_next_step` supplies worker readiness. `unread` supplies worker drain bounds.
`is_writable` supplies worker readiness.
No remaining program-edge assertion reads private state through a getter used only by tests.
The earlier held-write, output-read, bounded-drain, and guard proofs remain unchanged and accepted.

F283-PC-3 — LOW, CLOSED: the comment and body overstated worker resize coverage.
The cited `resizes_of_one_session_reach_the_worker_in_begin_order` test checks host forwarding and order only.
It does not check the worker PTY size or model ReadScreen result.
Sol-B independently reported that limit in `msg_plugin-w_1791763864_2d92fc`.
The package reviewer requested the correction in `msg_plugin-w_1791764101_58a319`.
The reviewed head changes one comment line from the gated parent. It changes no implementation, macro, or attribute.
The corrected body names the forwarding proof and the separate worker size-operation gap.
Body SHA-256: `cd7a429129a2f833163c3ff27faa585b90697737106bd96c7ea2c4ae821bf328`.
This PR supplies no new worker resize proof.

The completed Linux gate covers the parent c542a9fd, not the comment-only head.
Log: `~/botster-sessions/gates/botster-core-stage1-pc-f283-1-observable-c542a9fd-pool-20261011-170645-20432.log`.
SHA-256: `9fb055774a38b00b0c225d75088da6672efa33196a1a42c1492e4c550be77f37`.
All ten steps PASS. The wrapper exits zero after 182 seconds: one queued and 181 running.
Default: 1799 passed, 460 skipped. Slow: 597 passed, 2028 skipped.
Mutation total: eight caught, zero missed, zero timeout, zero unviable.
The reviewer matches all eight raw catches to the named citations and normal PASS records.
No FLAKY line appears.
This evidence supports the reviewed source because the later change is only a comment.
It is not an exact-final-head gate or final-slot approval.

The supporting `reversals-9757243c.log` records R5 and R6 with three failures each, R7 with two failures, and restored 31/31 PASS.
These reversals remove the script-step effect, remove the ignore flag, or apply the flag at parse time.
Their SHA-256 is `2525a39618c7b2aa7fc50bc560839d7ad119b3c824e59a9cb3d100c5ad5e7b6d`.
The local default log totals 1142 passed. Its SHA-256 is `7710f8544fbe68941ab7a80af00e320741d56e8dcc807e3ee8f8ee928e46455b`.
The static log SHA-256 is `47bed26c9072956fcb1afe0b369954ec574c8335724f92309ecbb2a1825b6089`.
These local records do not independently record their full clean history. They remain supporting records.
The pool gate independently passes the repaired behavior tests.

The package source and current evidence are CLEAN on cc491863.
Sol-B's matching integration disposition, the final slot import, final delta review, exact-final-head gate, and Q7 remain due.
This verdict approves no final merge. The reviewer waits for the doorbell and does not poll.
