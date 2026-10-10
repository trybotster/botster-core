# P6 package review: no-Rust diff mutation decision — PR #213

## Round 1

Implementation head: `d547d7aa43722a9b9eebd2d5525180a4fa8e9b2e`.
Integration and full gate base: `f6128fd6dd96a27f82520c82b9553d038dfa9dba`.
BUILD.md rules: botster-contracts `fe3eb952d6f61b89cf7bcbe8c54ab079987ec002`.

The reviewer checked HIGH first. Rule 1 requires HIGH because this PR changes a gate decision.
The lead handoff assigns the P6 package reviewer and Astra to this xtask change.
The reviewer read the exact two-file diff, its caller, tests, PR description, and supplied logs.
The reviewer also read the installed cargo-mutants 27.1.0 `src/in_diff.rs`.
The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.
The Git commands below only read the exact commit diff.

### Source result

The new predicate searches for a new-file header with an `rs` extension and excludes `/dev/null`.
For ordinary unified diffs, it preserves Rust and build.rs paths and excludes deletions and non-Rust paths.
Its tests cover quoted paths, space-containing paths, mixed diffs, empty diffs, and deleted files.
The new `diff_mutants` result records a skip without starting the listing closure when the predicate returns false.
When it returns true, the listing still uses `parse_listing` and propagates a failed start.
The public input/output tests preserve counts and error results, including rejection of an unsupported empty-list message.
No real-process test is added or migrated. No process owner, wait, sleep, cleanup, or reaper changes.
The mutation exclusion regex remains unchanged. Its reason names the new decisions and their tests.

### R1-1 HIGH — Git color can make a Rust diff pass without mutation testing

`mutants_job` at `xtask/src/ci.rs:373-376` runs `git diff` without disabling color.
Git permits `color.diff=always` and `color.ui=always`; these settings also color output captured through a pipe.
`changes_rust_source` at lines 443-447 requires the literal prefix `+++ ` at the start of a line.
A colored header starts with an ANSI color code, so the predicate finds no new Rust path.
`diff_mutants` then returns `None`, and the step passes without listing or testing any mutant.
This is an ordinary Git configuration, not a deliberate gate evasion.

The reviewer read the exact PR diff with:

`git -c color.diff=always diff f6128fd6dd96a27f82520c82b9553d038dfa9dba...d547d7aa43722a9b9eebd2d5525180a4fa8e9b2e`

Its Rust header is `\x1b[1m+++ b/xtask/src/ci.rs\x1b[m`.
The same diff with `color.diff=never` has the literal header `+++ b/xtask/src/ci.rs`.
The reviewer did not execute the new predicate or the gate.
The passing skip follows directly from the changed source.
Git documents `--no-color` as an override for color configuration. [Git diff documentation](https://git-scm.com/docs/git-diff#Documentation/git-diff.txt---no-color).

Make the generated landing diff format explicit before treating missing headers as permission to skip mutation testing.
Add a proof that the generated Rust diff still requires listing under color configuration.
Preserve genuine empty/non-Rust skips and failed-listing propagation.
The reviewer sent this finding directly to P3 and Astra.

### R1-2 LOW — Prior art note completed during review

BUILD.md rule 0 requires recorded reuse and rejection decisions and a reason for hand-written infrastructure.
The PR names cargo-mutants' rule and contrasts its log messages with the new predicate.
It does not record the complete item decisions or explain why this predicate replaces an established diff parser.

Add a short Prior art note that names the reused xtask pieces and rejected alternatives, with reasons.
Record why the narrow predicate is sufficient instead of a diff parser or more accepted log messages.
Astra requested the same body-only change. This reviewer confirmed the requirement directly to P3.
No new dependency or broad library search is required by this finding.

### Evidence and limits

Full Linux log: `~/botster-sessions/gates/botster-core-stage1-p3-mutants-no-rust-diff-d547d7aa-pool-20261009-194223-55685.log`.
It names the exact head and base above and exits 0.
All ten gate jobs pass.
It reports 124 passing conformance tests, 1331 passing default tests, and 256 passing slow tests.
Both new decision tests pass.
Both mutation commands report nine caught mutants, zero misses, zero timeouts, and zero unviable mutants.
This evidence covers the ordinary diff forms. It does not close the color configuration skip.

The earlier flip log names head `5d45efc146d5916e1a8c4dc8bddc894e06e56374` over the same base.
Log: `~/botster-sessions/gates/botster-core-stage1-p3-flip-passing-minimum-5d45efc1-pool-20261009-193029-6982.log`.
Its mutation step fails with the reported JSON EOF error and the gate exits 1.
The PR promises a fixed flip gate after this fix merges; that future run is not current evidence.

The reviewer then read the body-only Prior art update at the same implementation head.
It names the reused xtask pieces, cargo-mutants' source rule, three parser alternatives, and the log-text alternative.
It records rejection reasons and the reason for the small predicate. R1-2 closes.
Its claim that a misread line can only cause listing is contradicted by R1-1.
Correct that claim when closing R1-1.

R1-1 remains open for this HIGH PR. No ordinary NOT CLEAN report went to the lead.
Wait for replacement READY at an exact head.

VERDICT: NOT CLEAN
