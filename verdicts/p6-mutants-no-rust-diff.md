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


## Round 2

Implementation head: `9f33b53eebc0a530a18ad363d626bc656cf61b9f`.
Integration and full gate base: `f6128fd6dd96a27f82520c82b9553d038dfa9dba`.
BUILD.md rules: botster-contracts `fe3eb952d6f61b89cf7bcbe8c54ab079987ec002`.

The reviewer checked HIGH first. Rule 1 still applies to this gate decision.
The reviewer read the exact fix delta, callers, test helpers, updated PR description, and supplied exact-head log.
The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.

### R1-1 closed — the landing diff disables color and external diff tools

`landing_diff` now runs `git diff --no-color --no-ext-diff <base>...HEAD`.
It rejects a failed Git status before returning any bytes.
`mutants_job` writes those bytes to the file for cargo-mutants.
It passes the same bytes to `changes_rust_source` through `diff_mutants`.
The predicate therefore sees the uncolored headers from the command that generated its input.
A configured external diff tool cannot replace those headers.
The original failed-Git behavior remains.

The new test creates a temporary repository with both a text file and a Rust file.
It sets `color.diff=always`, `color.ui=always`, and `diff.external=/bin/false` in that repository.
After the text change, it asserts that the diff has no ANSI escape and contains no new Rust path.
After the Rust change, it asserts an uncolored `+++ b/x/src/lib.rs` header.
It also requires `diff_mutants` to return `Some(0)` from the supplied `[]` listing.
That result requires the listing closure to run; a skip would return `None`.
An invalid base must fail.
These assertions prove the command's public output and listing behavior, not private state.
The existing Git setup helper uses a group owner and the existing cleanup deadline.
The test changes only its temporary repository configuration and introduces no global test lock or sleep.

P3 reports a local red run after removing `--no-color`, followed by restoration.
The reported failure is the explicit `no color` assertion.
The reviewer read that report and did not perform the reversal.
The supplied exact-head gate passes the new proof.

The original tests still prove genuine empty/non-Rust skips and failed-listing propagation.
A Rust diff still rejects empty output without the NoMutants message.
The PR description and source comment now name the color gap and the plain-diff precondition.
The inaccurate claim from round 1 is corrected for the generated input.

### Prior art, scope, and evidence

R1-2 remains closed.
The updated note keeps its per-item reuse and rejection reasons and records the explicit diff format.
The final diff changes only ci.rs and the existing mutation reason in mutants.toml.
No exclusion regex, process-group cleanup, production reaper, or existing contract assertion changes.
No lifecycle test is migrated.
The new decisions remain covered by their named tests.
The existing mutation verdict and outcomes checks remain unchanged.

Full Linux log: `~/botster-sessions/gates/botster-core-stage1-p3-mutants-no-rust-diff-9f33b53e-pool-20261009-195517-18221.log`.
It names the exact head and base above and exits 0.
All ten gate jobs pass.
It reports 124 passing conformance tests, 1332 passing default tests, and 256 passing slow tests.
All three new proof tests pass, including the configured-repository test.
Both mutation commands report 13 caught mutants, zero misses, zero timeouts, and zero unviable mutants.
The repeated command does not add `--features slow`; these changed decisions have default-tier proofs.
The future fixed flip gate remains evidence for the flip PR, not a claimed run in this review.

All round 1 findings close. No new package finding remains at this exact head.

VERDICT: CLEAN


## Round 3 — base-merge carry

Implementation head: `ad383cd74d0bcee716714a240837d108f74ace5a`.
Reviewed first parent: `9f33b53eebc0a530a18ad363d626bc656cf61b9f`.
New integration and gate base: `f2eb577d58edd6c1d09215d21f9e51e9d5668e70`.
Reviewed base: `f6128fd6dd96a27f82520c82b9553d038dfa9dba`.
Previous package CLEAN: `7aefd888e745e25366bb65f4098ecc1142f5b071`.

HIGH still applies. The lead's base-merge rule permits review carry when the published merge check passes.
The reviewer read the required [PR evidence record](https://github.com/trybotster/botster-core/pull/213#issuecomment-6093187123).
It reports all four base-merge-check conditions PASS, including the conflict-free merge and the unchanged line-set file.
The reviewer did not run base-merge-check or any gate.

Read-only Git objects confirm the stated merge parents.
The two own paths do not overlap the six imported base paths.
Both own file blobs equal the reviewed first parent's blobs.
The final diff against the new base contains those same two own paths.
The old and new `--full-index` PR diffs are byte-identical: 12032 bytes.
SHA256: `b41ec5c9d0a146b9eaefacea0346edc2af377b4a2a515cf62df5c99d6b2d3d63`.
The actual merge tree is `ef312acc1ea4b8949e5df531c7911689a36d7ca4`.
Astra independently confirms that this tree matches the conflict-free merge result.
No source delta requires a new logic review. Both round 1 findings remain closed.

Exact-head Linux log: `~/botster-sessions/gates/botster-core-stage1-p3-mutants-no-rust-diff-ad383cd7-pool-20261009-200633-59607.log`.
It names the implementation head and new base above and exits 0.
All ten jobs pass.
Conformance: 124 passing tests. Default: 1332 passing tests. Slow: 256 passing tests.
Both mutation commands report 13 caught mutants, zero misses, zero timeouts, and zero unviable mutants.
This completes the required gate evidence for the merge head.

Astra published same-head CLEAN carry in verdict `a587e0f50cf5425b28227126cb98f64c5fc83de3`.
Astra reports that GitHub merged this head as `6ff4be7f6e064d0d4abcb524219a048b9ddc6e57`.
The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.
No open package finding remains at this exact head.

VERDICT: CLEAN
