# P6 base-merge-check review

## Round 1 — 2026-10-09

Reviewed head: `df04024b20e4213944a12360a991930facaf03bc`.
Review base: `ee7dd16c73a6991b6ef9b3a85d84fd93e57a3230`.
Branch: `stage1/p6-base-merge-check`.
PR: [#170](https://github.com/trybotster/botster-core/pull/170).
Plan pin: `stage1-plan.71a623ef.md`.
Plan sha256: `71a623ef93f487754e400dc186429216357e39fe251933675cc7f851843d519d`.
Open package findings: 3.

### BM1 — HIGH — Git settings can make an unreviewed change pass

Evidence: `xtask/src/base_merge.rs:228-239` uses porcelain `git diff` with inherited settings.
The patch command does not disable external diff or text conversion.
Neither diff command forces submodule changes into its output.

The reviewer used the exact Git arguments in an isolated repository to prove two false acceptances.
Both fixtures satisfy the ancestry checks and have a clean merge-tree.
The base changes `docs.txt`; the reviewed change affects `f.txt`.
Their path sets do not overlap.

1. With `diff.ignoreSubmodules=all`, the new merge also changes the `sub` gitlink.
   A gitlink records a submodule commit.
   Both patch diffs are equal, although the gitlink commits differ.
   Adding `--ignore-submodules=none` makes the diffs differ.
2. With `diff.external=/usr/bin/true`, the new merge also changes `f.txt` to `UNREVIEWED`.
   Both patch diffs contain zero bytes.
   Disabling external diff exposes different patches.

These inputs make `judge` return PASS while the PR's own change differs from the reviewed change.
That result permits an unreviewed change to retain CLEAN.
Git documents these settings in its [diff documentation](https://git-scm.com/docs/git-diff).

Required change: collect complete, canonical path sets and patches independently of diff filters and submodule ignore settings.
Disable external diff and text conversion.
Include gitlinks explicitly.
Add command-level regression proofs that reject both fixtures with these settings.
Cover the same class of filters when you select the Git arguments.

### BM2 — MEDIUM — The command test gets its expectation from the subject

Evidence: `xtask/src/main.rs:120-128` reads both the input name and the expected function pointer from `COMMANDS`.
`choose` reads that same table at lines 75-78.
Swapping the `taint` and `lists` handlers leaves this test green.
The usage test also stays green because the names remain unchanged.

The test proves agreement with the private table, rather than the requested command's selected action.
Plan section 8 requires tests of public behavior.

Required change: use independent expectations for each command's selected action.
Show that a wrong name-to-action mapping fails the test.
Retain the help and error assertions.

### BM3 — MEDIUM — The required mutation profile proof is absent

Plan section 8 requires an in-diff run with `NEXTEST_PROFILE=slow` until the new mutation profile lands.
The default profile can classify a terminated hanging test as a caught mutant.

The supplied gate clears the environment and runs `cargo xtask ci`.
At this head, `mutants_job` selects nextest but does not select a profile.
`test_budget::tier_env(false)` also supplies no `NEXTEST_PROFILE`.
The default nextest profile still has `terminate-after = 1`.
The scratch mutation summary names no profile or durable log.

Required change: supply the required in-diff slow-profile mutation evidence on the review head.
Close any terminated-test catch under the plan's behavior-test or exclusion rule.
This finding requires evidence; it does not require moving PR B's profile change into this PR.

### Scope and supplied proof

The reviewer inspected all six changed files and the PR description.
The Prior-art note discusses Gerrit's approval rule, Git range-diff, and GitHub's stale-approval rule.
The note explains the choice to compare one squashed diff.
The guard fix changes only the test helper shared through `#[path]`.
The new test exercises the wait result through the helper's API.
The change adds no reaping or process signal.
No contract assertion or transcript changes.

The supplied pool gate identifies the exact reviewed head and exits zero.
It reports 814 default tests and 213 slow tests passed.
It reports 35 mutants: 34 caught, 0 missed, 0 timeout, and 1 unviable.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p6-base-merge-check-df04024b-pool-20261008-235459-27159.log`.

The reviewer ran isolated Git probes to prove BM1.
The reviewer ran no build, test suite, mutation job, or gate.
The integration reviewer owns G1 separately: the excluded shell still contains the exit decision.

VERDICT: NOT CLEAN
