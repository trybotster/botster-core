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
