# P6: process-global tests (#250)

## Round 1 — 2026-10-11

Exact head: `f63daf224c3feb9e2f4e35ae3de5bd4cdc414e22`.
Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
Parents: `ae07d736da30e26ae6cf2bbe4f8d8bc84e248745` and the base above.
PR: https://github.com/trybotster/botster-core/pull/250.
Risk: HIGH. The change affects a shared test helper and gate infrastructure tests.

I read the complete eight-file delta, the full PR body, and the helper's process and deadline implementations.
I find no new defect. Zero findings remain.

### Process ownership and bounds

`alone::run_alone` starts the current test binary with the exact test name and one test thread.
It sets the name marker and requested variables only in the child command's environment.
It reuses `run_to_completion` with `Deadline::cleanup()`, derived from the existing ten-second cleanup bound.
The existing helper owns the child as the leader of a new process group before it reads any output.
It reads both pipes and observes the exit against one deadline. Its Drop ends the group on success, error, or panic.
The unreaped leader reserves the group ID until cleanup. The helper reaps only its own child by its exact PID.
The new helper starts no production child and adds no production reap, sleep, busy loop, or custom cleanup mechanism.
The existing cleanup path has its own cleanup deadline; this is not one combined ten-second bound for execution and cleanup.

`is_alone` checks the marker for the requested name. The child's selected test then runs its original body.
The pure verdict requires a successful process and a libtest summary with one passed test and zero failed tests.
A missing name cannot silently pass through a zero-test result.
The new proofs cover the verdict's positive and negative results, child-only variables, name matching, and a nonexistent test.
These proofs assert helper behavior and process observations. They add no private-state assertion.

### Migration contracts and search

The storage test retains its induced descriptor exhaustion, restored limit, MFILE assertion, absent directories, and exact remaining row list.
The signal test retains its real group signal and bounded wait for the handler's byte. Its read timeout remains ten seconds.
The base-reference test retains both assertions that the variable resolves to the expected commit.
The merge-check test retains FAIL condition 4 for an extra removal and FAIL condition 3 under `GIT_LITERAL_PATHSPECS`.
The two environment tests receive their variables through the child command. They no longer change the parent environment.
The new check that the child sees `GIT_LITERAL_PATHSPECS=1` observes the test condition required for its public result.

I searched the exact head for environment changes, resource limits, working-directory changes, group changes, signal handlers, masks, and prctl.
The PR identifies the changed sites and gives reasons for the remaining sites.
The guard helper tests change groups and handlers only in their explicitly selected child processes.
The anchor is a separate executable. The no-SIGPIPE test already has a separate target with one test.
The existing reserve cleanup intentionally moves the test process out of the group before the final kill.
That ownership mechanism and its migration remain outside this delta; this verdict does not replace its design.

Core-sys gains only a dev-dependency on the shared helper. The lockfile adds the corresponding dependency line.
The helper's dependency on core-sys creates a dev-dependency cycle. The retained builds demonstrate that this dependency graph builds.
The wrappers exchange no core-sys type across the two builds. No runtime dependency or runtime implementation changes.
The merge imports the base's sha2 profile configuration. Against the base, that configuration is not part of this PR's delta.
No conformance ID, transcript, contract pin, assertion of a contract, or mutation exclusion changes.

### Prior art

The note compares the binding old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` and current no-SIGPIPE isolation.
I read the old launcher and searched its tracked code for environment changes and resource-limit changes.
Its resource-limit call runs in the forked child before exec. The note reuses that isolation idea, not the production launcher.
The note rejects a target per migrated test because module access and four additional binaries make that form unsuitable here.
The shared runner instead reuses the existing process owner, bounded reader, deadline, and cleanup.

### Retained evidence

Exact-head Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-alone-tests-f63daf22-pool-20261011-011751-71700.log`.
I read its header, relevant results, reports, summary, and all six mutation outcomes.
All ten stages pass in 269.0 seconds. Job and wrapper exit zero after 358 seconds, with one second queued and 357 running.
Default: 1613 passed, 484 skipped, in 11.128 seconds. Slow: 427 passed, 1996 skipped, in 29.057 seconds.
The three helper proofs and all four migrated tests pass in their required tiers.
Stage 1 catches all six mutants in 7.0 seconds, with zero missed, timeout, or unviable. Stage 2 does not run.
The caught mutants cover both name-check replacements, its comparison, the empty runner, and both verdict replacements.
Fuzz reports no changed decoder harness.
Conformance passes 206. Strict pending runs 373/394, with 21 real-only IDs not run and two held IDs.
The testkit minimum is 55/69. Pending-real reports zero newly passing IDs among 77 IDs.
The reported real counts, 36/68 and 37/69, remain historical under R-59's required AD-7 fence.

I read both Mac logs under `~/botster-sessions/shared/core-stage1/evidence/p6-alone-tests/` in full.
The old log records three plain cargo-test runs at `f6778ad8`: nine, three, and six storage failures.
Its storage and signal source blobs equal those at this review base.
The branch log records three plain cargo-test runs with all 63 core-sys tests passing, plus all three helper proofs passing.
The branch log identifies an uncommitted tree and states that its tree matches the PR. It does not establish an exact-head Mac gate.
These retained local records demonstrate the reported runner difference within that stated source-identity limit.
I performed no reversal. The exact-head Linux gate supplies the completed gate evidence.

I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN

## Round 2 — 2026-10-11: evidence correction

Exact head: `f63daf224c3feb9e2f4e35ae3de5bd4cdc414e22`.
Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
The implementation has not changed. This round corrects the preceding acceptance of its Mac evidence.
The Round 1 CLEAN at `7dfc5000bc5ffe85c735486a60d388c93ee331dd` is withdrawn until the required evidence is established.

**AL-R2-1 — MEDIUM — the branch Mac record does not establish which source ran.**

The branch log identifies an uncommitted tree and states that it matches the PR.
It provides no head, tree, source hash, or retained diff to establish that match.
I have no separate retained identity record. The P6 handoff has none for this run.
My verified old storage and signal blobs establish only the old evidence's relation to the base.
They do not establish the branch evidence's relation to the reviewed head.

Plan revision 23ai requires a focused Mac run for portable code whose behavior the operating system decides.
That rule covers this change's resource-limit, signal, and process-group behavior.
Its log must be in the PR before the first READY. The Linux gate does not replace this required Mac proof.
P6 must provide an existing retained source-identity record or a focused Mac record that identifies the tested head.
The source review and exact-head Linux results remain valid. No implementation defect is identified.

Integration raised this evidence gap. I confirmed it and sent the finding directly to P6.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN

## Round 3 — 2026-10-11

Exact head: `2603483b518cab83d505ac6a0688b52afaccbf60`.
Base: `c942593697e856017572977af0405b17de0124e2`.
Parents: `f63daf224c3feb9e2f4e35ae3de5bd4cdc414e22` and the base above.
Tree: `309bfb121fda55dfa4864d5552e45a0c9a53debe`.

I read the full updated body, complete seven-file own delta, combined merge diff, and relevant imported source changes.
The merge resolves the core-sys dev-dependency comment. Both parents already name the same dependency.
The resolution keeps one entry and both reasons. The lockfile now equals the base's lockfile.
The helper, signal wrapper, both xtask wrappers, and their assertions retain their exact Round 1 blobs.
The storage file imports only the base's lock documentation. Its migrated test body and all contract assertions remain intact.
The shared child owner, reader, deadline, and group cleanup also retain their exact Round 1 blobs.
The imported platform change distinguishes an exiting or zombie process from a reaped process on macOS.
It changes no ownership or reap in this PR. The new exact-head Mac proof exercises the helper with that imported implementation.
The current own delta adds no runtime change, conformance list change, contract pin change, or mutation exclusion.
The Round 1 source review and Prior-art comparison remain applicable.

### Required focused Mac proof

Log: `~/botster-sessions/gates/botster-core-stage1-p6-alone-tests-2603483b-pool-20261011-074431-75584.log`.
I read the complete retained log. Its header and checkout identify the exact head and base above.
The checkout prints the matching tree and zero tracked changes before the tests.
The pool runs on the Mac node with kernel 27.0.0 and two build jobs.
Three plain core-sys cargo-test runs pass all 68 tests in 0.93, 0.83, and 0.79 seconds.
Each run explicitly reports the resource-limit test and SIGURG test as passing.
The three helper tests pass. Both selected xtask tests pass.
The job and wrapper exit zero after 38 seconds, with one second queued and 37 running.
This exact-head pool record supplies the required Mac source identity and focused proof under plan 23ai.
AL-R2-1 is closed. The earlier uncommitted-tree records remain historical and do not supply this acceptance proof.

### Completed Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-alone-tests-2603483b-pool-20261011-074537-78621.log`.
I read its exact-head header, reports, relevant test results, summary, and all six mutation outcomes and catching-test records.
All ten stages pass in 269.1 seconds. Job and wrapper exit zero after 353 seconds, with one second queued and 352 running.
Default: 1693 passed, 481 skipped, in 11.794 seconds. Slow: 546 passed, 1993 skipped, in 41.304 seconds.
All three helper tests and four migrated tests pass in their required tiers.
Stage 1 catches all six mutants in 4.8 seconds, with zero missed, timeout, or unviable. Stage 2 does not run.
The three name-check mutants are caught by the child-environment proof.
The empty runner is caught by the nonexistent-name proof. Both verdict mutants are caught by the pure verdict proof.
Fuzz reports no changed decoder harness.
Conformance passes 209. Strict pending runs 370/390, with 20 real-only IDs not run and two held IDs.
The testkit minimum is 56/69. The gate reports real-passing 46/68 and real-accepted 47/69.
Six of 30 pending-real IDs pass. This branch changes no conformance list.
The lead's approximately 16:30 ruling keeps those IDs pending until P5's contracts .38 pin PR.
This verdict records the gate's counts and that ruling. It does not approve a pending-list removal.

GitHub names this exact head and base. The PR remains open.
Zero findings remain at this head. The Round 1 withdrawal remains on record for the earlier evidence gap.
I changed no product code and ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN
