# P5 real worker endpoint — PR #210

## Round 1 — Two findings remain open

Reviewed head: `6742efddfc232f34c7fbdc0e633a7eb3d43d1c07`.
Current v1 base: `4b227d46ef5453ab8a3f7daaa4dc33562b4be83b`.
Risk: HIGH, rules 3 and 5. Scope: the complete twelve-file PR diff and its cross-package callers.

### R1-1 — HIGH — New proofs use legacy process fixtures

The new worker proof in `crates/botster-worker/tests/common/session.rs` calls `Session::launch` and `Session::remove`.
Those methods use the legacy PayloadGuard and GroupGuard from `botster-core-sys/tests/common`.
They also use blocking listener acceptance and raw Child::wait.
The file already has GuardedSession, which uses `botster-test-process`.

The new Core proof in `crates/botster-core/tests/slow_facade_worker.rs` uses `common::ScriptWorker`.
That fixture uses the legacy GroupGuard and WAIT_WHILE_THE_PARENT_LIVES script.
The new FIFO read uses `std::fs::read_to_string` without a deadline after the pump returns.
A missing writer can therefore block the test. The new proof also joins the peer directly.

Plan 6.1 requires new process ownership, waits, and cleanup to use `botster-test-process`.
This is the same boundary that #198 F64 established.
Move these new proofs onto the shared guards and bounded waits. Ask P6 for any missing capability.
Keep unrelated legacy tests outside this correction.
Refresh the slow mutation evidence and the full gate after the proof changes.

The package reviewer independently confirmed this finding as AR-F1 HIGH.

### R1-2 — MEDIUM — A registration failure leaves the endpoint file

`crates/botster-worker/src/main.rs:140-149` binds the endpoint before it constructs Endpoint.
The Endpoint field initializer then calls `register(...)?`.
If registration fails, the function returns before Endpoint exists.
UnixListener closes its descriptor, but no Endpoint Drop removes the filesystem socket.
This bypasses the cleanup owner on a startup error.
DESIGN.md part 7 requires endpoint removal at every end of the driver.

Construct Endpoint immediately after a successful bind. Then register its listener.
Supply bounded evidence that failed startup removes the bound endpoint.
Later startup failures already have an initialized Endpoint owner.

The package reviewer independently confirmed this finding as AR-F2 MEDIUM.

### Review and evidence

The reviewer applied the orchestrate-delivery lifecycle checks and the Botster lessons.
The launch arguments carry the endpoint and startup bound to the real worker.
RealEdges implements the host driver's ConnectWorker and RemoveEndpoint actions.
The worker accepts candidates, passes them to the existing machine, and applies the adoption fence.
The fence closes the old link, removes its queued inputs, and converts the adopted candidate's queued inputs.
Candidate admission and deadlines remain in the worker machine.
The endpoint directory check rejects a symlink, a foreign owner, and group or other permission bits.
The public API snapshot includes the two new RealEdges methods.

The reviewer read all six new whole-body mutation exclusions and their named decisions and proofs.
The exclusion patterns cover only the named whole-body replacements.
Their acceptance remains blocked by R1-1 because the cited real-process proofs use legacy fixtures.
The one process-check allowance covers one accept on a listener explicitly set to nonblocking mode.
That accept cannot wait for a connection. The allowance does not justify the separate unbounded FIFO read.

Full gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-adopt-real-6742efdd-pool-20261009-175935-54726.log`.
The Linux pool log names the exact head and current base. It ran on msa1, allocation e915eac8.
All ten stages pass. The job exits zero after 499 seconds.
Default: 1298 passed. Slow: 257 passed. Conformance: 121 passed, zero failed.
Both mutation steps report 74 mutants: 66 caught, eight unviable, zero missed, and zero timeouts.
The log records all three new adoption test executions as passing.
Setting NEXTEST_PROFILE alone does not establish slow-feature mutation coverage; the PR body states that limit.

Separate slow mutation evidence: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-adopt-real-dd9cdb11-pool-20261009-175623-40899.log`.
That run enables the slow feature and nextest slow profile with configuration exclusions disabled.
Its three focused adoption tests pass. Its ten mutants yield eight caught and two unviable, with no misses or timeouts.
The job exits zero after 127 seconds. Product and test source match the reviewed head.
The later delta changes only the evidence citation and public API snapshot.
These results establish the executed checks; they do not close the fixture ownership finding.

The reviewed head contains current v1. The base merge c0b9c8e5 matches its automatic merge tree exactly.
The tree is `8f1bcb430a758727316b21fda6248ff687be06f8`.
No pending list changes occur. The real-tier probe claims concern a separate harness combination and are not acceptance here.
The real controls deferred to the next PR remain outside this change.
`git diff --check` passes. The reviewer ran no builds, tests, or gates.

VERDICT: NOT CLEAN at 6742efddfc232f34c7fbdc0e633a7eb3d43d1c07 — two findings open.
