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
