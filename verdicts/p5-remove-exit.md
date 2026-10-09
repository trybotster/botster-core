# P5 Remove exit-order review

## PR #187 Round 1 — 2026-10-09

- Exact head: `f545722f94ce97fff087d66c88a62c494f0d1d1a`.
- Branch: `stage1/p5-remove-exit-order`.
- Accepted v1 base: `465978d67ff620cedc21d50a20fc193fb8397e9a`.
- Tree: `4eef3a1400d267d75c82b7b9e8cc0c55c3ab2b1e`.
- Risk tier checked first: HIGH, BUILD.md at contracts `56bd0a5347a537d25bbee65a67854e0e317a9b9a`, rule 5.

### REO-F1 — MEDIUM — OPEN — The real-process proof is missing

BUILD.md requires a named real-process test for real-process behavior before HIGH code merges.
This fix changes the handling of a real worker exit and its buffered control-link result.
The PR names only World tests and TestkitHarness transcripts.
The supplied slow gate contains 243 unchanged tests. The reviewer found no named Remove proof in Core's slow tests.
P5 must cite an existing named real-process test that proves the exit-before-read behavior and its exact-head PASS.
Otherwise, P5 must add the required proof under the P6 ownership rules.
The three ids' non-slow classifications permit their pending removals; they do not waive the HIGH production proof.

### REO-F2 — MEDIUM — OPEN — An exit before SendRemove can be lost by the next flow step

The new on_process_exited branch covers every Remove phase.
If ProcessExited arrives during CloseRoutes or SendRemove, the branch keeps the link open.
It sets both worker.gone and RemoveFlow.worker_gone to true.
run_remove(SendRemove), flows.rs lines 523-551, then sends on that retained link and resets RemoveFlow.worker_gone to false.
If the link result and EOF remain withheld through stop_grace, remove_grace_expired records OutcomeUnknown but cannot advance.
Its identity answer is ignored by flow_remove_probed because worker.gone is already true.
Remove then waits for EOF instead of completing after the remove grace with the already-known exit.
This violates LC-7 and A6-3's requirement to continue teardown when the worker or result is lost.

P5 must preserve the known exit across SendRemove, or limit the drain branch to a teardown already sent.
P5 must add a behavior test with this sequence:

1. Begin Remove on an Exited session whose worker still has a link.
2. Deliver ProcessExited before SendRemove runs.
3. Keep the link open without a cleanup result or EOF.
4. Advance stop_grace.
5. Assert Remove completion and id release.

The reviewer traced this sequence through the code and ran no test.
The reviewer sent both findings directly to P5 and copied integration.

### Static review and supplied evidence

The reviewer read the complete three-file delta, PR body, Remove flows, inbound handlers, driver link handling, tests, worker close order, and contracts.
The worker queues RemoveResult, flushes its bytes, closes the link, and then exits.
The host can observe the process exit before it reads all link bytes.
In AwaitTeardown, the new branch correctly retains the link and marks the worker gone.
remove_progress advances when a result already exists.
The new on_link_closed branch supplies OutcomeUnknown only if no result exists and the worker has ended.
The new test covers a later Deleted result and EOF without a result.
The extended test checks that link closure alone does not complete a Remove while the worker still runs.
The early flow transition in REO-F2 remains uncovered.

The pending list removes exactly these three ids and adds none:

- `conf::am_3_exactly_one_completion`
- `conf::lc_7_remove_order_and_completion`
- `conf::or_2_session_order`

All three belong to the approved minimum list.
The pinned replacement map at `636bc1babcb410464bc40a5862895a1dc3260f5c` classifies them core-testkit or core-testkit+perturb.
The exact-head gate contains one distinct PASS conformance trial for each id.
No dependency pin, transcript, status ledger, deferred list, timeout value, or mutation exclusion changes.
The PR contains its Prior art note.

The supplied Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/remove-exit-f545722f.log`.
It names the exact head and accepted v1 base.
The default tier reports 945 tests passed and 653 skipped.
The slow tier reports 243 tests passed and 962 skipped.
The mutation run reports five caught, zero missed, zero timeouts, and zero unviable.
All ten stages pass. Fuzz reports no changed crate with a decoder harness and runs no harness.
The job and gate exit zero. The gate does not close the findings above.

The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: NOT CLEAN
