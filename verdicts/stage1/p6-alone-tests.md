# Integration review: PR #250

VERDICT: NOT CLEAN (1 open)

- Head: `f63daf224c3feb9e2f4e35ae3de5bd4cdc414e22`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Package NOT CLEAN: `71462dabb5f08449b3d98c99f6757570efd733f3`.
- Branch: `stage1/p6-alone-tests`.
- Risk tier: HIGH.

## ALONE-SI-1 MEDIUM: the Mac record lacks tested-source identity

Status: OPEN. This is the same evidence gap as package AL-R2-1.

Plan 23ai requires focused Mac proof for portable code whose behavior the OS decides.
This PR changes the process context of resource-limit, signal, and process-group tests.
Its retained branch Mac record identifies uncommitted source and states that the tree matches the PR.
It gives no head, tree, source hash, or retained diff that establishes that match.
The package reviewer confirms that no separate retained identity record exists in the handoff.
The Linux gate does not replace this required Mac proof.

Required correction: supply an existing retained identity record or a focused Mac record that identifies the tested source.
The record must cover the changed process tests and helper with the relevant source equal to this reviewed head.
A source comparison can establish that equivalence; a prose assertion alone cannot.
No new full gate is required merely to identify a retained focused run.
The source review below identifies no implementation defect. Acceptance remains pending until this evidence gap closes.

## Source and process checks

Sol read the complete eight-path diff and the reused child ownership and completion path.
The alone helper starts the current test binary with the exact test name, one test thread, and uncaptured output.
The child alone receives BOTSTER_TEST_ALONE and the supplied environment values.
The name marker prevents recursive launches only for the named test.
The existing run_to_completion helper owns the process group, bounds execution, and captures output after completion.
The existing owner cleanup keeps its own cleanup bound. This is not one combined execution-and-cleanup bound.
The helper requires a successful exit and a libtest summary with exactly one passed test and zero failed tests.
A missing test, missing summary, failed exit, zero tests, or multiple tests fails the helper.
The proofs include child-only environment, the name comparison, and a real missing-test launch.

The four migrated tests retain their existing behavior assertions.
The storage test lowers the descriptor limit only in its child and restores it before its result assertions.
It still proves the failed write, retained row, and removed directories.
The signal test registers SIGURG only in its child. It still requires the handler byte within the existing bound.
The fsutil test sets BOTSTER_CI_BASE_REF only in its child and still resolves a temporary repository tag to a commit.
The base-merge test sets GIT_LITERAL_PATHSPECS only in its child and retains its rejection conditions and prerequisite assertion.
No test now shares those process-global changes with unrelated tests under plain cargo test.

Core-sys gains a dev-dependency on botster-test-process. The resulting dev-dependency cycle is covered by the retained Linux build and tests.
The PR adds no production dependency, runtime behavior, mutation exclusion, timeout exception, pin, transcript, or ID removal.
The existing process helpers handle OS-specific ownership. The PR reuses them rather than adding a second process runner.

## Prior art and retained proof

Sol checked OLD CORE 72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c for setrlimit and pre_exec.
Its production launcher applies resource limits in the child's pre_exec path. It supplies no equivalent exact-test launcher to port.
The new helper uses std::process::Command and the current process owner, deadline, and completion helpers.
The PR explains why the existing separate no_sigpipe target does not serve four tests with module-private access.

Sol read both retained Mac summary logs in full under `~/botster-sessions/shared/core-stage1/evidence/p6-alone-tests/`.
The old record reports three plain cargo-test runs at f6778ad8 with nine, three, and six storage failures.
The branch record reports three runs with all 63 core-sys tests passing, plus three helper proofs passing.
Those records show the reported runner difference within their stated identity limit. They do not close ALONE-SI-1.
Sol raised that limit with P6 and its reviewer. The reviewer withdrew the initial CLEAN and reported the same gap.

Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-alone-tests-f63daf22-pool-20261011-011751-71700.log`.
Sol read the exact-head/base header, helper proof records, storage proof, all six mutant identities, and the complete final summary.
All ten steps pass. Default tests pass 1613 cases; slow tests pass 427 cases.
Stage 1 catches six mutants: is_alone true, false, and inverted comparison; run_alone empty body; verdict success return; and deleted failure guard.
Stage 2 has no missed mutant to run. No mutant times out or is unviable. Fuzz finds no changed decoder harness.
The CI summary is 269.0 seconds. The job and wrapper exit 0 after 358 seconds.
No pending ID removal or minimum progress is approved. The R-59 AD-7 tally limit remains in force.

Sol read both package verdict rounds, including the withdrawal and evidence correction.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

## Round 2 inspection — exact-head Mac evidence and current-base import

Date: 2026-10-11.
READY: `msg_plugin-w_1791730322_7953b9`.
Head: `2603483b518cab83d505ac6a0688b52afaccbf60`.
Base: `c942593697e856017572977af0405b17de0124e2`.
Parents: `f63daf224c3feb9e2f4e35ae3de5bd4cdc414e22` and the base above.
Tree: `309bfb121fda55dfa4864d5552e45a0c9a53debe`.
GitHub independently confirms this head and base. Sol reads the complete current PR body.
The GitHub MCP fails. Sol uses the authorized GitHub CLI fallback.

### Import review

Sol reads all seven own changed paths against the current base.
The six source patches equal their earlier reviewed patches after removal of index lines and hunk positions.
The source files are signal.rs, storage.rs, alone.rs, the process crate's lib.rs, base_merge/tests.rs, and fsutil.rs.
The sole conflict resolution is `crates/botster-core-sys/Cargo.toml`.
Both branches add the same development dependency. The resolution keeps one entry and both reasons.
It retains #265's slow lock tests and #250's process isolation helper.
Cargo.lock equals the current base, which already contains that dependency.
All 82 imported-only paths equal current-base blobs.
Both pending files and `.cargo/mutants.toml` also equal base bytes.
The head contains the exact base. The diff whitespace check passes.

Sol reads the current `run_to_completion` and group-owner path.
The helper still owns the test child, reads both pipes within the execution deadline, and cleans the group on every exit path.
The owner's cleanup uses its own cleanup bound. This review claims no single combined execution-and-cleanup bound.
The storage and signal tests retain their original observable assertions.
The two xtask tests retain their public result checks and receive variables only in their child commands.
No new implementation finding appears.

This is a delta review of the conflict and imported source. It does not claim a conflict-free automated merge or CLEAN transfer.
Sol runs no executable base check.

### ALONE-SI-1 / AL-R2-1 closes in the inspected evidence

Sol reads the complete focused Mac log:
`~/botster-sessions/gates/botster-core-stage1-p6-alone-tests-2603483b-pool-20261011-074431-75584.log`.
SHA256: `94ea8e8e19042adb81289ceeb96f1b48aa01f8a4714072ab85904f408bb7388e`.

Its header names the exact head and base. The checkout prints the exact reviewed head and tree, then zero tracked changes.
The log reports these results:

- Plain core-sys cargo tests pass 68/68 in each of three supplied runs.
- Each run prints PASS for the storage descriptor-limit test and group-signal test.
- All three `alone` proofs pass.
- Both migrated xtask tests pass.
- The Mac job and wrapper exit zero after 38 seconds.

This record establishes tested-source identity and covers the changed OS-dependent tests and helper.
It closes the missing identity evidence. The earlier uncommitted record retains its historical identity limit.
Sol requests no further run or reproduction campaign.

### Exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p6-alone-tests-2603483b-pool-20261011-074537-78621.log`.
SHA256: `686c3501f2f3c9e6d79abc0de9d09dd03362c0b15f9fa816df5d6e79a2323dfc`.
Sol reads the exact-head/base header, relevant test results, all six raw mutant outcomes, all six catch reports, and final summary.

All ten steps pass in 269.1 seconds. The job and wrapper exit zero after 353 seconds.
Default: 1693 passed, 481 skipped, in 11.794 seconds.
Slow: 546 passed, 1993 skipped, in 41.304 seconds.
The log passes the helper proofs and all four migrated tests in their required tiers.
Slow coverage checks 1049 tests.
Stage 1 catches all six mutants. Raw and reported identities match independently, with six unique identities.
Three name-check mutants use `the_new_process_alone_sees_its_name_and_variables` as catcher.
The empty runner uses `a_name_that_matches_no_test_fails`.
Both verdict mutants use `only_a_success_with_one_passed_test_passes`.
Zero mutants are missed, timed out, or unviable. Stage 2 does not run.

The minimum remains real-passing 46/68 and real-accepted 47/69.
Thirty real-pending IDs remain; six pass under ignored selection.
The lead assigns their required R-66/R-68 work and removal to later pin work. This PR changes neither pending file.
No removal or minimum increase follows from this review.

### Completion status

Zero integration findings remain after this inspection.
Formal Round 2 completion waits only for the published package verdict on this exact head and base.
The latest package artifact inspected so far is the prior-head NOT CLEAN at `71462dabb5f08449b3d98c99f6757570efd733f3`.
Sol sends the evidence closure and import findings to the package reviewer in `msg_plugin-w_1791730424_bbe01c`.
This inspection issues no formal CLEAN and accepts no future import.
Plan section 8 Q8 needs no repeat gate for this unchanged head if both CLEAN verdicts later cover it.
Sol runs no build, test, gate, mutation, reversal, measurement, or executable base check.

### Round 2 completion — package CLEAN received

Package receipt: `msg_plugin-w_1791730481_121256`.
Published artifact: `9580cd5dd98cbd41a3f730023f6a08239c9afe15`, `verdicts/p6-alone-tests.md`, Round 3.
Sol reads the complete round and independently verifies its publication.
It covers the exact head `2603483b518cab83d505ac6a0688b52afaccbf60` and base `c942593697e856017572977af0405b17de0124e2`.
It agrees with the source, import, Mac identity, gate, and proof limits inspected above.
ALONE-SI-1 / AL-R2-1 closes formally. Zero findings remain.

The unchanged exact head already has its green gate. Q8 requires no repeat gate.
The lead owns the merge decision. No future import, pending removal, or minimum increase is approved.
The prior-head evidence gap and withdrawn package CLEAN remain historical.
Sol runs no execution job.

VERDICT: CLEAN (0 open) at 2603483b518cab83d505ac6a0688b52afaccbf60
