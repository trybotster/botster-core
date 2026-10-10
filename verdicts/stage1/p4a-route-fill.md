# PR #221 — Shared testkit route_fill integration review

## Round 1 — 2026-10-10

Reviewed head: `3bc22068416fded17d3935e8bfd57aec6ad0135c`.
PR and gate base: `159cc4003c8910ba6ef28402c360ba4d2dd820a4`.
Tier: HIGH. The PR changes a shared testkit control.
Scope: the complete four-file diff, affected attachment and startup behavior, R-47, and supplied gate evidence.

### R1-1 — MEDIUM — An attachment during startup retains an absent worker

`harness.rs:285-300` reads the persisted session row during `attach_stream`.
It copies the row's optional worker identity into `RouteFill`.
`route_client.rs:123-128` rejects `route_fill` when that copied identity is absent.
The route never updates the copied identity.

The host permits this sequence:

1. Complete Create.
2. Begin Start.
3. Attach a stream before the first startup pump creates the worker.
4. Pump the host until the session reaches Running.
5. Call `route_fill` on that stream.

Start sets `Admit::Starting` immediately (`host/src/admit.rs:556`).
`check_attach` accepts that state (`host/src/admit.rs:916`).
The persisted row has no worker before creation, so this valid route retains `None`.
The control then refuses with `route_fill: the route's session has no worker process`, even after the payload starts.

Resolve the worker for the original session instance when the control runs, or update the binding when startup supplies the worker.
Preserve isolation from a session that is removed and recreated with the same name.
Add a test that attaches before worker creation and fills after Running.

Status: OPEN. I sent this finding to the implementer and package reviewer.

### Other source checks

- The control computes `N = C + F` from current stream capacity and the applied frame bound.
- The capacity calculation accounts for the gate and the acceptance limit.
- The fill pattern starts at `a` for each call and repeats through `z`.
- The control writes through the program edge and signals the worker wake.
- The control refuses a payload that has exited.
- The worker lookup and stream capacity lookup do not nest their locks.
- The PR changes no conformance lists. Product support for `RouteStalled` remains outside this PR.

### Evidence checked

I read the complete PR body, source changes, and three new tests.
The base is an ancestor of the reviewed head. `git diff --check` passes.
The remote revisions matched the supplied revisions when checked.

Gate log: `botster-core-stage1-p4a-route-fill-3bc22068-pool-20261010-041221-24128.log`.
The header names the reviewed head and base. All ten gate stages pass.
The log reports 1,438 default tests, 259 slow tests, and 193 conformance tests.
The transcript report has 193 passing trials, 407 pending trials, 70 additional entries without transcripts, two deferred trials, and 18 withdrawn trials.
Both mutation stages report 17 mutants: 15 caught and two unviable, with no misses or timeouts.
The second mutation stage sets the slow environment variable; it does not enable the `slow-tests` feature.
All three new route-fill tests pass. These tests do not cover the attachment sequence in R1-1.
The gate reports 339.1 seconds. The pool job exits successfully after 569 seconds, including one queued second.
I ran no build, test, or gate during this review.

### Verdict

NOT CLEAN: R1-1 remains open.
The package verdict for this exact head is pending.

## Round 2 — 2026-10-10

Reviewed head: `ef5e785935e057c394469e2f73bed23e8f588d7d`.
Base: `159cc4003c8910ba6ef28402c360ba4d2dd820a4`, unchanged from Round 1.
I read the complete replacement diff and updated PR body.
The remote head and base match these revisions. The base is an ancestor, and `git diff --check` passes.

### R1-1 — MEDIUM — Partially corrected; attachment before the first row remains broken

The route now retains a reader of its stored session row and the original session instance.
Each fill reads the current row and checks its instance before selecting the worker.
The reader shares the existing registry and releases its lock before worker access.
An attachment after Create completes and before worker creation therefore finds the worker after startup.
A missing row or different instance refuses the fill before any program write.

The startup test attaches before the first startup pump and confirms that the row names no worker.
It checks early refusal, completes Start, and checks a successful fill and bytes at the program edge for seeds 0–7.
The isolation test removes an exited session, creates and starts another session with the same name, and uses the old route.
It checks refusal and verifies that the new payload receives no bytes.
I read both tests and their shared helpers. The gate contains PASS results for both tests and the three earlier route-fill tests.

The package reviewer found another admitted order. I independently confirmed the following sequence in the source:

1. Begin Create without pumping the host.
2. Begin Start without pumping the host.
3. Attach a stream.
4. Pump the host until both operations complete and the payload runs.
5. Call `route_fill` on the stream.

Create immediately inserts a session with admission state Created (`admit.rs:537`, `engine.rs:645`).
Start accepts Created and immediately changes admission to Starting (`admit.rs:556`).
Attachment accepts Starting (`admit.rs:916`). No stored row exists before the first pump.
The existing host test `a_failed_create_completes_the_ops_admitted_after_it` also verifies that Start can queue behind an unpumped Create.

In `harness.rs:288-299`, `row_reader(...).zip(session_row(...).ok())` returns `None` because the stored row is absent.
The route retains `RouteFill.session = None` after startup and refuses every fill with `route_fill: the route has no session row`.
The new startup test completes Create first, so it does not cover this sequence.

Support an attachment before the first stored row while preserving the original session instance.
Add a regression test for this sequence. Retain the removed-and-recreated session test.
R1-1 remains OPEN, as does package finding F94. My earlier provisional closure is superseded.

### Replacement gate

Log: `botster-core-stage1-p4a-route-fill-ef5e7859-pool-20261010-042753-60177.log`.
The header names the exact head and base. All ten gate stages pass.
The log reports 1,440 default tests and 259 slow tests, all passing.
Both conformance reports show 193 passing trials, 407 pending trials, and 70 additional entries without transcripts.
The reports retain two deferred trials and 18 withdrawn trials.
Both mutation stages report 24 mutants: 21 caught, three unviable, no misses, and no timeouts.
The second mutation stage sets the slow environment variable but does not enable the `slow-tests` feature.
The full gate reports 181.6 seconds. The repeated mutation stage reports 101.0 seconds.
The pool job exits zero after 292 seconds with no queue time. The wrapper reports 293 seconds.
I ran no build, test, or gate.

This round corrects the Round 1 summary's transposed conformance counts.
The source finding and its disposition do not change because of that correction.

### Verdict

NOT CLEAN: R1-1 remains open at MEDIUM severity.
The package reviewer independently reports the same open finding. The replacement package artifact is pending.

## Round 3 — 2026-10-10 — CLEAN

Reviewed head: `d8809fb818ae05b57ae92b5382db11e19f7d821c`.
Base: `159cc4003c8910ba6ef28402c360ba4d2dd820a4`, unchanged.
I read the complete six-file replacement diff and the updated PR body.
The complete PR now changes eight files. The HIGH tier includes the host and testkit crates.
Remote head and base match. The ancestry check and `git diff --check` pass.

### R1-1 — CLOSED in source and supplied evidence

`HostEngine::instance` reads the existing in-memory session instance without changing state.
`TestkitCore::attach` captures that instance and records it only after a successful attachment.
The synchronous attachment path does not replace the session between those actions.
`attach_stream` obtains the recorded instance and a reader of the existing registry.
It no longer requires a stored session row when the route attaches.
Each fill still checks the current row against the original instance before accessing the worker.

The new test begins Create and Start without a pump and verifies that no row exists.
It attaches the route, checks early refusal, completes both operations, and checks successful output at the program edge.
It then removes the original session and starts another session with the same name.
The old route refuses the fill, and the new payload receives no bytes.
This sequence runs for seeds 0–7. Closing the client before Remove permits cleanup; the instance check still causes the final refusal.
The two earlier startup and isolation tests remain intact and pass.
The host test verifies instance availability before the first row, stability after a pump, and absence for unknown sessions.

### Gate and retained scope

Log: `botster-core-stage1-p4a-route-fill-d8809fb8-pool-20261010-044326-90152.log`.
The header names the exact head and base. All ten gate stages pass.
Default tests: 1,442 passed. Slow tests: 259 passed.
Both conformance reports show 193 passing trials, 407 pending trials, 70 entries without transcripts, two deferred trials, and 18 withdrawn trials.
Both mutation stages report 32 mutants: 25 caught, seven unviable, no misses, and no timeouts.
The second mutation stage sets the slow environment variable but does not enable the `slow-tests` feature.
The new host test and all six route-fill tests have PASS results.
Full gate time is 204.2 seconds. The repeated mutation stage takes 160.8 seconds.
The pool job and wrapper exit zero after 374 seconds, including one queued second and 373 execution seconds.
I ran no build, test, or gate.

The formula, pattern, capacity calculation, exited-payload refusal, and list scope remain unchanged.
Product support for `RouteStalled` remains outside this PR. This review claims no additional conformance acceptance.
I read the Round 2 package artifact at `4e540cacc7e7713e9cf97694edc80264be8151f5`; it agrees with the earlier open finding.
The previously blocked Round 2 integration commit `a69ae606` is now published.
The package verdict is CLEAN at `7fc828384b74862e9658bc4202906e29297521cf`, `verdicts/p3-worker.md`, Round 143.
I read that verdict. It agrees with this review and closes F94 at the same head.
I rechecked the remote head and base before publication; both remain unchanged.

### Verdict

CLEAN at `d8809fb818ae05b57ae92b5382db11e19f7d821c`.
R1-1 is CLOSED. No integration finding remains open within this PR's scope.
The paired contracts review and separate stall implementation retain their existing scopes.
