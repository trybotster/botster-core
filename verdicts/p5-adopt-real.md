# P5 real adoption review

## PR #210 — Round 1 — 2026-10-09

- Exact head: `6742efddfc232f34c7fbdc0e633a7eb3d43d1c07`.
- Tree: `cdde8e494ac2de3891cf433898695c75001adb47`.
- Parent: `11565971bf55bce066123dbb844eeece1bd78cac`.
- PR and gate base: `4b227d46ef5453ab8a3f7daaa4dc33562b4be83b`.
- Risk: HIGH, rules 3 and 5. The PR changes shared crates and `.cargo/mutants.toml`.
- Scope: the complete twelve-file delta, PR body, six new mutation exclusions, process-check allowance, and supplied logs.

The reviewer checked the risk tier and prior-art note first.
The prior-art note names the #176 draft, existing testkit adoption, and existing real-edge decision patterns.
Approved plan 23l and the lead's #176b ruling govern this review.
The reviewer changed no product code and ran no builds, tests, mutation jobs, base-merge checks, or gates.

### AR-F1 — HIGH — New real-process proofs use legacy fixtures

The new worker proof calls `Session::launch` at `crates/botster-worker/tests/common/session.rs:431`.
That helper uses legacy `PayloadGuard` and `GroupGuard`, a blocking listener accept, and `Session::remove` uses raw `Child::wait`.
The same module already has `GuardedSession`, which uses `botster-test-process` ownership and bounded completion.

The new Core proof uses `common::ScriptWorker` at `crates/botster-core/tests/slow_facade_worker.rs:403`.
That fixture uses legacy `GroupGuard` and `WAIT_WHILE_THE_PARENT_LIVES`, which contains a shell sleep loop.
At line 437, `std::fs::read_to_string(&launch)` opens and reads a FIFO without a deadline.
`pump_until` has returned before that read. Its deadline does not bound the FIFO read.
If no launch writer arrives, the test can wait indefinitely.
The proof also joins the stand-in thread without a deadline at line 459.

Plan section 6.1 requires shared ownership for new real-process test code.
The legacy fixtures' existing allowances do not establish that these new proofs meet that rule.
The supplied green gate does not remove this source defect.

Required change: use `GuardedSession` for the worker proof.
Use the shared `Guard`, `Bounded`, and `Deadline` facilities for the Core proof's process ownership and waits.
Bound the FIFO read and stand-in completion through the shared facilities.
Ask P6 for any missing shared capability.
Refresh the slow-tier mutation evidence after the proof changes.

Status: OPEN. The reviewer sent this finding directly to P5 and copied Astra.
This finding independently confirms integration R1-1.

### AR-F2 — MEDIUM — Registration failure leaves the bound endpoint path

At `crates/botster-worker/src/main.rs:140`, `UnixListener::bind` creates the endpoint socket.
At lines 141–149, the driver calls `register(...)?` inside the `Endpoint` field initializer.
If registration fails, the driver returns before it constructs `Endpoint`.
The listener closes, but `Endpoint::drop` does not run and cannot unlink the socket path.
The startup path therefore breaks the documented endpoint cleanup rule.
Later startup failures have a constructed `Endpoint` guard and do not have this defect.

Required change: construct `Endpoint` immediately after bind.
Then register `endpoint.listener` so every later error drops the guard.
Add a bounded proof that an error after bind removes the endpoint path.

Status: OPEN. The reviewer sent this finding directly to P5 and copied Astra.
This finding independently confirms integration R1-2.

### Other review results and evidence

The host adds the endpoint and startup arguments to `WorkerLaunch`.
The real edges check endpoint path length and private directory ownership before opening the endpoint directory.
`connect_worker` uses a non-blocking mio connection and the same stream registration as `accept_link`.
`remove_endpoint` records failures and treats a missing endpoint as success.
The worker accepts bounded candidates, forwards their inputs to the existing worker machine, and applies the adoption fence.
Pure proofs cover link and candidate IDs, readiness, endpoint directory checks, startup config, input fencing, and unlink results.
The reviewer read every new whole-body exclusion and its proof citations.
The process-check allowance correctly describes one non-blocking accept; AR-F1 covers the surrounding fixture defects.
The API snapshot adds `connect_worker` and `remove_endpoint` without removing existing entries.

Supplied full gate:
`~/botster-sessions/gates/botster-core-stage1-p5-adopt-real-6742efdd-pool-20261009-175935-54726.log`.
The log names the exact head and current v1 base. The head contains that base.
All ten stages report PASS. The job and gate exit with zero.
Default tests: 1298 passed, 558 skipped. Slow tests: 257 passed, 1285 skipped.
Conformance: 121 passed, zero failed.
Both mutation steps report 74 tested, 66 caught, eight unviable, zero missed, and zero timeout.
The extra `NEXTEST_PROFILE=slow` command does not establish slow-feature mutation coverage.
The launcher selects the `mutants` profile explicitly after #181.

Separate supplied slow-feature evidence:
`~/botster-sessions/gates/botster-core-stage1-p5-adopt-real-dd9cdb11-pool-20261009-175623-40899.log`.
Head: `dd9cdb110dae822fcc081ab4f74cb10112d5ab80`.
The command selects the named endpoint functions with `--no-config --features slow` and the nextest `slow` profile.
Three adoption proofs pass. The mutation summary reports ten tested, eight caught, two unviable; the job exits with zero.
Only evidence comments and the API snapshot change between that head and the submitted head.
AR-F1 requires replacement proof evidence before these shell exclusions can be accepted.

Pending IDs remain unchanged. All 41 active minimum IDs have PASS lines, so testkit progress remains 41 / 69.
The PR discloses the real probe's separate combined head. Its reported real gains do not certify this head's real harness.
The reused-PID, loss, and output real controls remain the later #176b part.
No contracts pin or transcript changes occur in this PR.
Two findings remain open. No terminal CLEAN is issued.

VERDICT: NOT CLEAN
