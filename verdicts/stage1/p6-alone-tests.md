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
