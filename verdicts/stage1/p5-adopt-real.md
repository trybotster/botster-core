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

## Round 2 — Replacement review

Reviewed head: `232f121e7b819153fcdd980cd1a2486eb58d4560`.
Current v1 base: `e99012939c8df4f85da5ae5af7144534b129cb70`.
The PR now changes fourteen files against the current base.
I compared the complete PR changes with round 1, excluding diff positions and base changes.
The changes are the two fixes, the shared test dependency, and the refreshed evidence citation.
The prior source conclusions remain applicable to unchanged code.

### R1-1 — CLOSED

The worker adoption proof now uses `GuardedSession`.
Its worker and payload start through `Guard` wrappers with confirmed anchors.
`OwnedChild` owns the worker. Removal waits through `status_by(Deadline::cleanup())`.
The existing bounded accept allowance remains unchanged.

The Core proof now starts its script through `Guard::wrapper` and holds it with `Blocker`.
Its launch FIFO uses nonblocking, close-on-exec descriptors and `Bounded::line(Deadline::cleanup())`.
The proof confirms the worker anchor before it proceeds.
The peer thread signals completion through a channel; `recv_timeout` bounds the wait after Core drops its link.
The new test dependency appears in both Cargo.toml and Cargo.lock.
Unrelated legacy fixtures remain unchanged.

### R1-2 — CLOSED

The worker constructs `Endpoint` immediately after `UnixListener::bind` succeeds.
Registration now occurs after the cleanup owner exists, so a registration error drops that owner.
The failed-control-connection proof checks that failed startup leaves no endpoint.
The exact-head slow gate executes that proof successfully.

### Merge and gate checks

The merge conflict in `f2538a49` retains the candidate-close and adoption actions from this PR.
It also retains v1's route action arms, which remain no-ops until the real route driver exists.
The complete PR comparison shows no unrelated change introduced by either v1 merge.
The head contains current v1. Remote head and base checks match the revisions above.
`git diff --check` passes. The PR changes no conformance list.

Exact-head gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-adopt-real-232f121e-pool-20261009-213230-53333.log`.

All ten CI steps pass in 461.1 seconds.
Default tests: 1,403 passed. Slow tests: 259 passed.
Conformance: 182 passed, zero failed; 418 pending with transcripts and 70 without transcripts; two deferred and 18 withdrawn.
The Core adoption proof, both worker adoption copies, and the failed-start cleanup proof pass.
Both standard mutation runs report 74 mutants: 66 caught, eight unviable, zero missed, and zero timeouts.
The job exits zero after 619 seconds on msa1.

Separate slow mutation evidence:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-adopt-real-f2538a49-pool-20261009-213000-42647.log`.

The command enables the slow feature and profile, disables exclusions, and uses the current base diff.
It includes `--max-fail 1:immediate` and runs after candidate prebuild in a separate job.
The aggregate reports ten mutants: eight caught and two unviable. The job exits zero after 130 seconds on msa1.
Only the evidence comment changes from that revision to the reviewed head; product and test source are identical.
I requested the saved outcomes and per-mutant logs to verify the executed failure proofs.
The package verdict for this head is also pending.

I ran no builds, tests, gates, or mutation jobs.

### Detailed slow mutation record verified

The successful f2538a49 job did not retain its detailed mutation artifacts.
P5 repeated the command on the exact reviewed head and printed the outcomes and individual logs:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p5-adopt-real-232f121e-pool-20261009-214448-86621.log`.

I parsed all eleven outcomes: one successful baseline, eight caught mutants, and two unviable mutants.
The baseline ran 356 tests successfully, including both worker adoption copies and the Core adoption proof.
Every executed mutation uses the slow feature, slow profile, and immediate fail-fast setting.

- Removing `open_endpoint_dir` fails the directory creation proof because the endpoint directory is absent.
- Replacing `connect_worker` with `None` fails the nonblocking accept because the new host made no connection.
- Removing `remove_endpoint` fails the assertion that the endpoint was removed.
- Removing the `startup` field fails the configuration proof: the default ten seconds differs from the requested 1.234 seconds.
- Removing `accept_candidate`, `read_candidates`, or `adopt_link` fails the driver adoption proof at the bounded handshake read.
- Removing `Endpoint::drop` fails the assertion that the worker removed its endpoint.

The handshake failures are test failures, not mutation timeouts.
The two unviable replacements require `LinkId: Default`, which the type does not implement.
All six excluded shells therefore have inspected behavioral catches after the fixture correction.
The record reports zero misses and zero timeouts. The job exits zero after 95 seconds on msa1.
Both integration findings are closed.

### Package review and final verdict

I read package round 2 at `17a5314bae53b4c31d315019efdd69e87a0c8a5f`, `verdicts/p5-adopt-real.md`.
The package reviewer reports CLEAN on the exact reviewed head, with AR-F1 and AR-F2 closed and no new finding.
The source, merge, and detailed mutation checks agree with this review.
No integration finding remains open, including LOW.

VERDICT: CLEAN (0 open) at 232f121e7b819153fcdd980cd1a2486eb58d4560.
