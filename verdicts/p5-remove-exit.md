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

## PR #187 Round 2 — 2026-10-09

- Exact head: `2b5ba07e5bcff471370fab07e2436a103cfef91b`.
- Tree: `204c4750c4bec10ad8f4be0deda17fd326ba9feb`.
- PR base and merge base: `465978d67ff620cedc21d50a20fc193fb8397e9a`.
- Supplied gate base: `a14e9dc2b61a5426485f9c7f0f829c900e03bdc7`.
- Risk tier checked first: HIGH, BUILD.md rule 5, as the current PR body states.

### REO-F1 — MEDIUM — CLOSED — The engine tests suffice under the lead's proof ruling

The reviewer asked the lead whether the new real-process proof could use the old fixtures during the HOLD.
The lead confirmed that the HOLD remains until PR B (#181) lands and forbids that new proof.
The lead also directed the reviewer to decide whether TestkitHarness or sans-IO tests prove this change.
If those tests suffice, REO-F1 closes. Otherwise, the PR waits for a proof on botster-test-process after PR B.
The ruling is the lead's message `msg_plugin-w_1791563315_78c77a`.

The reviewer chose the first option and informed P5 and integration.
The initial real-process requirement was too broad for this engine-only delta.
The change decides outcomes for ordered ProcessExited, LinkMsg, and LinkClosed inputs in HostEngine.
It changes no real process edge, socket read, reaper, or worker write and close path.
The World tests assert the public RemoveReport for exit before result, exit before EOF without result, and result before exit.
The extended test checks that EOF while the worker still runs does not complete Remove.
The new early-exit test covers the remaining phase transition.
The three TestkitHarness trials run the production engine with the seeded scheduler.
These proofs suffice for the engine decision under the lead's clarified boundary.
The PR body records that decision and the continuing HOLD.

The temporary slow test and its helper refactor are absent from the reviewed head.
slow_facade_worker.rs is byte-identical to the PR's v1 base.
No new real-process test code enters this PR.

### REO-F2 — MEDIUM — CLOSED — The link drain applies only after the teardown request

The new on_process_exited condition requires RemovePhase::AwaitTeardown and an open worker link.
An exit in CloseRoutes or SendRemove follows the existing close path.
That path removes the link and records the worker as gone.
SendRemove then chooses OutcomeUnknown with worker_gone true and advances without sending to the gone worker.
It cannot reset the exit state through the retained-link branch identified in Round 1.

The new test is a_worker_exit_before_the_teardown_is_asked_completes_the_remove.
It begins Remove, delivers ProcessExited before a pump sends the teardown request, and supplies no EOF.
It asserts completion with OutcomeUnknown, an empty session list, and no HostMsg::Remove.
It advances no clock. The exact-head gate reports this test PASS.

### Delta and supplied evidence

The reviewer read the complete delta from the Round 1 head and the current PR body.
Only inbound.rs and flow_edges.rs change in that delta.
The pending list is byte-identical to Round 1. Its three removals and replacement-map checks remain valid.
The reviewer checked distinct exact-head PASS trials for am_3_exactly_one_completion, lc_7_remove_order_and_completion, and or_2_session_order.
The gate also reports PASS for the new early-exit test and both Round 1 regression tests.
No other package finding arose.

The full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/remove-exit-2b5ba07e.log`.
It names the exact reviewed head and gate base a14e9dc2.
The gate base is newer than the PR base by the disjoint #186 kernel-errno test change in guard_platform.rs.
This verdict covers the current PR head; it does not certify a later merge head.
The default tier reports 946 tests passed and 653 skipped.
The slow tier reports 243 tests passed and 962 skipped.
The mutation run reports five caught, zero missed, zero timeouts, and zero unviable.
All ten CI stages pass. Fuzz runs no harness because no changed crate has a decoder harness.
The job and gate exit zero.

Every package finding is closed. Integration owns its separate findings and must supply CLEAN before this HIGH PR merges.
This verdict does not close #176's proof hold or establish real-process conformance for the minimum ids.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
The lead owns the merge decision.

VERDICT: CLEAN
