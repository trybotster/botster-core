# Integration review: PR #262

## Round 1: exact counts for mutation exclusions

- Head: `de819474f8f0cdf590d9d15638fb9e2a271f5a47`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Parents: `d57dbe992c0b8e38c6c46c0323c84ea9b165ad29` and the recorded base.
- Tree: `cf7962fb0d20fa1dc023b10d761cadf28361703d`.
- READY: `msg_plugin-w_1791719735_126ac1`.
- Package CLEAN: `e38334a2bfcfb0131aad3b79d7168ebfc50be709`, verdicts/gs-gate-speed.md.
- Package receipt: `msg_plugin-w_1791719875_325792`.
- Risk: HIGH. The mutation exclusion check changes.
- Plan: 23ar, `a2897c1204dbfe6bbc44f476783686a68cabf57a`.

Sol reads all four changed files, the complete PR body, and the complete package addition.
The fetched package branch contains the package commit. Sol independently checks its publication.
Zero integration findings remain open.

### Source and composition

Each configured exclusion receives count Some(1), or the unsigned N from `# N mutants` for N of at least two.
The parser rejects signs, leading zeroes, other suffixes, and an invalid or overflowing number.
The parser retains the complete pattern and its preceding reason block.
The existing TOML validation and configured-entry count check remain in gate_decisions::inputs.
The stale check retains rejection of zero matches and invalid regexes.
It also rejects a positive match count that differs from the configured count.
The violation lists every matched identity and the head of the listing.
OFF_MACOS_EXCLUSIONS receives None and retains its existing nonzero-match rule.
The command retains its full workspace listing with --no-config and failure checks for external commands.

Sol reads all 85 new count annotations. A standard TOML parse gives identical values before and after this change.
Removing exactly those 85 annotations restores the base file's exact bytes.
No exclusion pattern, reason, timeout, glob, or configuration value changes.
The diff changes no mutation stage, selector, build command, verdict rule, pin, or pending file.
Count equality does not prove identity equality when one matched identity replaces another.
The body states that limit explicitly.
Sol's independent git merge-tree result equals the head tree and reports no conflict.
The current-base import retains the exact automatic composition with #261.
git diff --check reports no whitespace error.

### Prior art

Sol checks OLD CORE at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
That tree has neither .cargo nor xtask entries. It supplies no mutation exclusion check to reuse.
This PR extends the current exclusions_of parser and stale check. It adds no dependency or alternative mutation runner.

### Supplied execution evidence

Log: `~/botster-sessions/gates/botster-core-stage1-gs-exact-count-de819474-pool-20261011-045219-1794.log`.
SHA256: `d9cf100022e17bd918ffd459d4c94c5db18dd31e36e3fbc361263bec229b72d7`.

Sol reads the exact head/base header, relevant check results, tier summaries, all mutation outcomes, catch reports, and final summary.
All ten steps pass in 161.6 seconds. The job and wrapper exit zero after 172 and 173 seconds respectively.
The check reports 208 exclusions against 6201 workspace mutants. Both new parser and count tests report PASS.
The default tier passes 1641 tests in 16.133 seconds, with 484 skipped.
The slow tier passes 479 tests in 38.102 seconds, with 1996 skipped.
Slow coverage checks 1014 tests. Real-pending stays 58; real-passing stays 40/68 and real-accepted stays 41/69.
Stage 1 tests 18 mutants: 16 caught, two unviable, zero missed, and zero timeout.
The unmutated baseline passes in four seconds of build and one second of test.
Sol independently matches all sixteen raw catch identities to their named test reports.
Stage 2 does not run because stage 1 misses no mutant. Fuzz starts no trial because no decoder crate changes.

The earlier 2c7bf039 gate remains failed. Sol reads its exact header, missed identity, stage summaries, and failed exit record.
Both stages miss `replace || with && in count_of` at 68:26.
Commit d57dbe99 adds the rejected `# +3 mutants` case. The new exact-head gate catches that same operator mutant.
The new evidence closes the missing test case; it does not change the earlier failed result.

This green full gate covers the same head and base as both CLEAN reviews under plan section 8.
No repeat gate is required for this unchanged head. A later changed head needs its applicable review and exact-head gate.
The lead owns the merge decision and current-base check.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at de819474f8f0cdf590d9d15638fb9e2a271f5a47
