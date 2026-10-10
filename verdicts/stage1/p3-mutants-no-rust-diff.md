# PR #213: skip mutation listing when the diff changes no Rust source

## Round 1

- Head: `d547d7aa43722a9b9eebd2d5525180a4fa8e9b2e`.
- Base: `f6128fd6dd96a27f82520c82b9553d038dfa9dba` (`v1`).
- Scope: the complete two-file diff, the mutation-step caller, tests, exclusion comment, PR body, and existing gate evidence.
- Tier: HIGH under rule 1 (gate decisions) and rule 3 (workspace configuration).

### Source checks

`changes_rust_source` checks the new paths in the same diff that the gate gives cargo-mutants.
The rule matches cargo-mutants 27.1.0 `src/in_diff.rs`: the new path has extension `rs` and is not `/dev/null`.
For uncolored diff output, the predicate includes `build.rs` and handles Git's quoted paths and trailing tab.
An added line that resembles a path can cause a listing, but cannot suppress a real Rust path.

`diff_mutants` skips the listing only when that predicate returns false.
A recognized Rust diff retains the existing listing checks, including process status, valid UTF-8, and JSON parsing.
The existing narrow exception accepts the tool's empty-filter message.
Other empty output and a failure to start still fail the step.
The caller prints the reason when it skips the listing.
The existing mutation outcome and exit-status checks remain unchanged.

The two new tests cover the reported list-only diff, an empty diff, path forms, deleted files, and mixed changes.
They also prove that the listing closure does not run for a diff without Rust source.
The tests check empty and nonempty listings, unexpected empty output, and start errors for a Rust diff.
The exclusion pattern remains unchanged. Its comment now cites both decision tests.

### Existing evidence

Gate log:
`~/botster-sessions/gates/botster-core-stage1-p3-mutants-no-rust-diff-d547d7aa-pool-20261009-194223-55685.log`.

The log names the exact head and base above. The Linux pool job ran on `msa1` and exited 0 after 466 seconds.
All ten CI steps passed. The default tier passed 1,331 tests. The slow tier passed 256 tests.
Both new tests passed in the default tier.
Conformance passed 124 cases with no failures. The normal and include-ignored reports agree.
The mutation step caught all nine mutants, with zero missed, timeout, or unviable outcomes.
The appended mutation run reported the same result.
That appended command sets `NEXTEST_PROFILE=slow`; it does not enable the `slow` feature.
The changed decisions and their new tests are in the default tier.

The earlier flip gate confirms the reported failure:
`~/botster-sessions/gates/botster-core-stage1-p3-flip-passing-minimum-5d45efc1-pool-20261009-193029-6982.log`.
It failed the mutant-listing JSON parse with EOF and exited 1.
The PR reserves the complete flip gate for the later flip head that includes this fix.

Remote head and base match the reviewed commits. The base is an ancestor of the head.
`git diff --check` passed. This review ran no builds, tests, or gates.

### R1-1: LOW — complete the prior-art record

BUILD.md rule 0 requires a prior-art note and a recorded reason for hand-written infrastructure.
The initial body names cargo-mutants and explains the log-text failure.
It does not complete the reuse and rejection record for the new predicate.
The requested correction is limited to the PR body; it requires no source commit or gate rerun.

**CLOSED:** The updated body names the reused xtask code and explains the hand-written predicate.
It records the rejected log-string, diff-parser, and name-only alternatives with reasons.
The head did not change. R1-2 below covers the incorrect claim that the predicate always fails safely.

### R1-2: HIGH — Git color output can bypass mutation checks

The package reviewer identified this defect. I independently confirmed it with a read-only Git command on the exact base and head:

```text
git -c color.diff=always diff f6128fd6dd96a27f82520c82b9553d038dfa9dba d547d7aa43722a9b9eebd2d5525180a4fa8e9b2e
```

The Rust header is `\x1b[1m+++ b/xtask/src/ci.rs\x1b[m`.
No line starts with the literal `+++ ` required by `changes_rust_source`.
`mutants_job` invokes `git diff` without disabling color.
With this supported Git configuration, the new predicate returns false for a Rust change.
The caller then reports zero mutants and passes without listing or running them.
The green gate does not exercise this configuration.

Make the landing diff format explicit, including disabling color.
Provide regression evidence that configured color cannot make the gate skip a Rust change.
Update the body claim that misread lines cannot skip mutation.
This is the same defect as package finding R1-1.

### Package verdict and result

I read the package verdict at `8de5fadca739af074e4a1b057259e13c9dd8d0ca:verdicts/p6-mutants-no-rust-diff.md`.
It reviews the same head and reports the same open color defect.
It also closes the prior-art finding after the body update.

R1-1 is closed. R1-2 remains open.
The replacement head requires review and a gate on that exact head.

VERDICT: NOT CLEAN (1 open)
