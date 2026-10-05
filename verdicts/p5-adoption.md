# P5 adoption and restart review

## PR #162 — Round 1

- PR: https://github.com/trybotster/botster-core/pull/162
- Exact head: `b5c4bffbcdf2bc1843f944eafbf315a9b9b200cb`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Scope: audit A10 / issue #152, in `crates/botster-core/tests/slow_real_core.rs`.
- This verdict covers P5's package scope. This PR changes one test file and needs no cross-package review.
- The reviewer ran no builds, tests, mutation jobs, or gates.

The PID probe and the false claim that the guard reaps the worker are removed. Production still owns worker reaping.
The test now observes FIFO EOF, as audit A10 permits. Two findings prevent acceptance of the complete replacement proof.

### P5-F1 — MEDIUM — The FIFO does not observe every child

**Evidence:** `slow_real_core.rs:315-338` claims that every process of the script holds the FIFO write end.
The body uses `common::WAIT_WHILE_THE_PARENT_LIVES` from `tests/common/mod.rs:15`.
That loop starts `/bin/sleep 1 >/dev/null 2>&1`. The child redirects its standard output away from the FIFO.

FIFO EOF therefore proves that the worker shell closed its write end. It does not prove that the `sleep` child ended.
If cleanup ends only the shell, EOF can arrive while the child still runs.
The comment, assertion message, and PR description claim a stronger result than the test observes.
BUILD.md testing rule 10 requires cleanup of the process group. The user also requires the guard to own that group.

**Required change:** Keep a FIFO descriptor open in every persistent child used by this test.
Use a dedicated inherited descriptor if a child redirects its standard output.
Alternatively, use a fixture whose persistent child keeps the observed standard output.
Keep the child blocked without a busy loop. Keep production as the sole worker reaper.
Update the comment and PR description to match the observation.

**Closure evidence:** The source must show that the persistent child holds the observed descriptor until exit.
Supply one focused result for the revised test. Do not use repeated runs as proof that a race is fixed.

Status: OPEN.

### P5-F2 — MEDIUM — The cleanup deadline does not bound guard cleanup

**Evidence:** `slow_real_core.rs:360` calls `drop(worker)` before the marked deadline at lines 361-365.
`ScriptWorker` owns `GroupGuard`. Its `Drop` calls `registration.join()` and `anchor.wait()` synchronously
in `crates/botster-core-sys/tests/common/process_guard.rs:80-92`.
Neither wait has a deadline. If guard cleanup blocks, the test never reaches `recv_timeout`.

The replacement test therefore still contains an unbounded cleanup wait.
Its deadline comment says a failed guard causes a test failure instead of a hang, but that statement does not cover this path.
BUILD.md testing rule 5 and the user's real-process rules require bounded waits.

**Required change:** Bound the test's complete cleanup operation, including guard destruction.
For example, move the owned guard cleanup to a helper thread and observe its completion through a marked deadline.
Do not wait for that thread without a deadline. Do not let the test reap the production worker.
Keep the existing group ownership and cleanup on panic.

**Closure evidence:** The source must show that the test reaches a bounded wait before any blocking guard cleanup.
Supply one focused result for the revised test. A shared guard change also needs the applicable package review.

Status: OPEN.

### Evidence and scope limits

The reviewer inspected the implementer's Linux log:
`~/botster-sessions/gates/botster-core-stage1-p5-a10-b5c4bffb-linux-20261004-202957-96965.log`.
The header names the exact head and base above. Clippy completed successfully.
The log reports 20 selected passes and a final run with 15 passed and zero skipped. The command exited 0.
These results establish focused execution. They do not establish either missing cleanup guarantee.
The repeated runs do not replace a structural race proof under BUILD.md testing rule 9.

The removed LC-12 PID check could not distinguish a live worker from a zombie.
The PR explicitly leaves `conf::lc_12_drop_leaves_workers_running` for P5 proper.
This verdict does not establish LC-12 conformance or closure of the other P5 audit findings.

VERDICT: NOT CLEAN (2 open)

## PR #162 — Round 2

- Exact head: `1a96eee7adc240a4ed90835dfc967e2efdd8e6b6`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Delta: `b5c4bffbcdf2bc1843f944eafbf315a9b9b200cb..1a96eee7adc240a4ed90835dfc967e2efdd8e6b6`.
- Scope remains audit A10 / issue #152. Only `slow_real_core.rs` changes.
- The reviewer ran no builds, tests, mutation jobs, or gates.

P5-F1's source defect is closed. The shell now waits for `/bin/cat` on a FIFO that has no writer.
The child keeps the observed FIFO as standard output. Neither process redirects that output or runs a busy loop.
FIFO EOF establishes that all write descriptors closed, without depending on production reaping.

P5-F2 is CLOSED. The test moves `worker` to a cleanup thread before guard destruction.
The test observes cleanup completion through a marked 10-second deadline, then observes FIFO EOF through another marked deadline.
The shared guard and production worker reaper are unchanged.

The reviewer inspected the exact-head Linux log:
`~/botster-sessions/gates/botster-core-stage1-p5-a10-1a96eee7-linux-20261004-205647-55445.log`.
Its command runs format checks, Clippy, and one `slow_real_core` run.
The log reports 15 passed, zero skipped, and exit 0. The revised cleanup test passed.
This evidence is a focused run. The full merge gate remains subject to the lead's FULL HOLD.

### P5-F1 — Remaining documentation requirement — LOW

The source defect is closed, but the Round 1 documentation requirement remains open.
The reviewer read the full PR body with `gh pr view 162` on this head.
The body still states that the script uses a wait loop that ends when the test process is gone.
The revised script uses a blocked `cat` child instead. The evidence section names only `b5c4bff` and its 20 repeated runs.

**Required change:** Update the PR body to describe the blocked child and the bounded guard cleanup.
Name the exact revised head and its single focused run. Preserve the LC-12 scope limit and Prior art note.
No source commit or new test run is required for this documentation correction.

Status: OPEN at LOW severity. P5-F2 is CLOSED. No other finding is open for this PR's scope.

VERDICT: NOT CLEAN (1 open)

## PR #162 — Round 3

- Exact head: `1a96eee7adc240a4ed90835dfc967e2efdd8e6b6`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Change since Round 2: PR description only. The source head is unchanged.
- Scope: audit A10 / issue #152. This CLEAN covers P5's package scope for this PR only.

P5-F1 is CLOSED. The reviewer read the updated full PR body with `gh pr view 162`.
The body now describes the blocked `cat` child, the inherited FIFO output, and the cleanup thread with a marked deadline.
It names the exact reviewed head and the single focused run recorded in Round 2.
The Prior art note and LC-12 scope limit remain present.

P5-F2 remains CLOSED. Audit A10 is CLOSED for this PR's scope.
No finding remains open, including LOW findings. The source and execution evidence from Round 2 remain valid on this unchanged head.
The reviewer ran no builds, tests, mutation jobs, or gates.

This CLEAN does not establish the full merge gate, LC-12 conformance, closure of other audit findings, or completion of P5.
The implementer must respect the lead's FULL HOLD before starting the merge gate.
Any later source commit requires a delta review on its exact head.

VERDICT: CLEAN

## PR #162 — Round 4

- Exact head: `913594cb5aa2afe858aa0ed3f0eaa5b357a373d2`.
- Base: `144b0234fb632bcbb5176b17c2fe55f3239405df`.
- Delta: `1a96eee7adc240a4ed90835dfc967e2efdd8e6b6..913594cb5aa2afe858aa0ed3f0eaa5b357a373d2`.
- Scope now includes the shared guard failure that the lead assigned to this PR after the Mac gate failed.
- The shared guard compiles into worker tests. The integration reviewer must review this delta.
- The reviewer ran no builds, tests, mutation jobs, or gates.

Round 3 CLEAN applies only to its recorded head and scope. It does not accept this new delta.
P5-F1 and P5-F2 remain closed for the original A10 test replacement.
The new scope changes the guard's cleanup premise, so the reviewer checked the shared callers under `orchestrate-delivery`.

### P5-F3 — HIGH — The new guard precondition cannot cover panic or test-process death

**Evidence:** `crates/botster-core-sys/tests/common/process_guard.rs:13-16` adds a precondition for every guard user.
The group must stop creating children before cleanup begins. A fixture must report final-child readiness before the test kills it.
The anchor still performs one group kill and exits at lines 114-117.

The same module retains two callers that do not satisfy this precondition:

- `a_panic_before_ready_ends_the_child` starts `while :; do /bin/sleep 1; done` at line 260.
  It then panics without waiting for readiness at lines 267-270.
- `an_early_exit_keeps_the_group_owned_until_cleanup` starts a descendant with that same repeated-fork loop at line 286.
  It drops the guard while the loop can still create another child at line 302.

`crates/botster-core/tests/common/mod.rs:15` and `crates/botster-core-sys/tests/slow_process.rs`
also retain loops that repeatedly start `sleep` children.
Test-process death can occur before any readiness event, regardless of the normal test sequence.

Under the PR's stated macOS failure mechanism, these paths can still miss a child during the group kill.
Moving child creation before readiness in two fixtures does not establish the shared cleanup guarantee.
BUILD.md testing rule 10 requires cleanup on every exit path, including panic.
The user's real-process rules require the guard to own and kill the group without taking production reaping.
A new caller precondition cannot narrow those requirements.

**Required change:** Preserve cleanup on panic and test-process death during startup.
Close the reported failure through fixture ownership and synchronization that also cover those paths.
Check every shared caller affected by the chosen rule. Remove any precondition that those callers cannot meet.
Keep production as the sole worker reaper. Do not add a repeated-kill polling loop or a timeout increase.

**Closure evidence:** Supply a deterministic regression for cleanup during the relevant child-creation handoff.
The regression must observe process cleanup through a real event and a marked deadline.
Record a failing baseline at the intended cleanup assertion and a passing revised result.
Checking one child's process group before the kill is setup evidence; it does not cover death before readiness.

Status: OPEN.

### P5-F4 — MEDIUM — The changed guard self-test still waits without deadlines

**Evidence:** `parent_dies_before_fifo_reader` reads the readiness line directly at `process_guard.rs:196`.
No deadline bounds `reader.read_line`. A fixture that fails before readiness can leave the test blocked there.
The EOF deadline at line 215 runs only after that read and parent destruction.

Parent destruction also calls `child.wait()` synchronously at lines 147-151.
The test calls `drop(parent)` at line 214 before its bounded EOF observation.
The revised self-test therefore retains waits that its EOF deadline does not bound.
This is the same wait-coverage error that P5-F2 closed in the A10 test.
BUILD.md testing rule 5 and the user's real-process rules require bounded test waits.

**Required change:** Put a marked deadline around the readiness read and the test-owned parent cleanup.
Keep the EOF observation. Do not increase its timeout. Reap only the test-owned parent and anchor.

**Closure evidence:** The source must reach a bounded wait before each blocking operation.
Supply one focused result for the revised self-test. A failed readiness path must produce a bounded test failure.

Status: OPEN.

### Evidence and scope limits

The reviewer inspected the failed Mac gate log on the previous head:
`~/botster-sessions/gates/botster-core-stage1-p5-a10-1a96eee7-mac-20261004-212801-2845.log`.
Its slow tier reports 83 passed and one failed.
`botster-core-sys::slow_process process_guard::parent_dies_before_fifo_reader` failed at the pipe EOF deadline after 10.050 seconds.
The gate exited 1. That log proves the failure; it does not establish the implementer's kernel-level explanation.
The implementer reports an orphaned `tee` observation from a separate diagnostic. The reviewer has requested its raw evidence.

No execution evidence for `913594cb` was supplied with this request.
The lead's gate-evidence rule permits a gate after all source findings close. P5-F3 and P5-F4 are source findings.
The integration reviewer was notified with the exact head and shared scope.

VERDICT: NOT CLEAN (2 open)
