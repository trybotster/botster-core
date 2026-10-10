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
The transcript report has 407 passing trials, 70 pending trials, two deferred trials, and 18 withdrawn trials.
Both mutation stages report 17 mutants: 15 caught and two unviable, with no misses or timeouts.
The second mutation stage sets the slow environment variable; it does not enable the `slow-tests` feature.
All three new route-fill tests pass. These tests do not cover the attachment sequence in R1-1.
The gate reports 339.1 seconds. The pool job exits successfully after 569 seconds, including one queued second.
I ran no build, test, or gate during this review.

### Verdict

NOT CLEAN: R1-1 remains open.
The package verdict for this exact head is pending.
