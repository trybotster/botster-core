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

## Delta source inspection — bbd25214

- Head: `bbd25214835b4648623cb1e5ac3a89ac50f5d260`.
- Base: `56fae5826f766b408abca06f99376e0506982bf9`.
- Parents: `c5cfdde84f79179328e7b9db19dbf603ca919d14` and the exact base above.
- Tree: `401afd40ba6e49c934a03f34ad360f5bfa777456`.
- Previously reviewed head: `de819474f8f0cdf590d9d15638fb9e2a271f5a47`.
- Owner receipt: `msg_plugin-w_1791728370_5ec176`.

The owner requests a delta round and reports a running gate. This receipt is not final READY. The owner reports failed base-check conditions 1, 2, and 3 on `.cargo/mutants.toml`, with condition 4 passing. Sol requests the saved log. Sol does not yet claim to have read that log or the owner's local check output.

Sol independently inspects the exact head and base. The permitted static merge-tree check of its parents reports one conflict, `.cargo/mutants.toml`. That file is also the only shared path between the base changes since the previously reviewed base and the four own paths.

Sol reads all 81 added count lines. Each line equals a previously reviewed added line exactly. The four removed annotations belong to the entries that accepted #267 removed: `Driver::run`, `Driver::read_pty_chunk`, `Driver::read_control`, and `link_send_descriptor`. No replacement exclusion appears.

A standard TOML parse gives identical values at the new head and exact base. Removing the 81 count comments restores the exact base bytes. Thus the resolution retains every base exclusion, reason, timeout, glob, and configuration value. It adds only the retained count comments.

The three xtask own diffs equal their previously reviewed diffs after removal of Git index lines. No implementation or test line changes. All 54 imported-only paths equal the exact base entries. Both pending files equal base. The Git diff check passes.

The source has zero new findings. The earlier parser review, prior-art check, ecosystem rationale, and count-versus-identity limit remain applicable. This inspection approves no exact-head execution evidence, pending removal, or merge. Formal CLEAN waits for READY, the final published body, the supplied full gate, the saved base-check log, and the published package artifact.

Sol ran no build, test, gate, mutation, reversal, measurement, or executable base-check job.

## Round 2 — imported head body finding

- Head: `bbd25214835b4648623cb1e5ac3a89ac50f5d260`.
- Base: `56fae5826f766b408abca06f99376e0506982bf9`.
- Package artifact: `476c465673bf6a3ef08e9d9f14cbdcee2e33508e`, `verdicts/gs-gate-speed.md`.
- Package receipt: `msg_plugin-w_1791728456_f317b5`.
- Base-check receipt: `msg_plugin-w_1791728438_08f090`.

Sol now reads the complete saved log `~/botster-sessions/gates/botster-core-stage1-gs-exact-count-bbd25214-base-merge-check.log`. Both ancestry checks pass. Conditions 1, 2, and 3 fail on `.cargo/mutants.toml`. Both condition-4 checks pass. The appended source comparison removes only the four previously identified annotated lines. The source delta inspection above addresses that failed check. Sol does not convert its failed result to PASS.

Sol reads the complete published package round and verifies its publication. The package review also finds no source defect. Sol independently fetches and reads the complete current PR body. Its head matches the reviewed head.

### EC-SI-1 LOW — the body states earlier counts as current. OPEN.

This finding agrees with package GS-EC-R2-1. The current body states 85 annotated entries out of 207 and 208 exclusions against 6201 mutants as the merged-head result. The current file has 81 count comments and 203 exclusion entries. Sol independently checks both counts with a standard TOML parse and static text parsing.

Required change: state the current file counts. Describe the four removed exclusions. Add the new exact-head gate evidence when it is supplied. Keep the earlier gates clearly identified as historical evidence.

This finding requires no source edit or repeat gate after the running gate. The owner says final READY follows a green gate. Sol has no final exact-head gate receipt yet and accepts none here. Formal CLEAN waits for READY, the corrected published body, the supplied exact-head gate, and package closure.

No source finding is open. One body finding is open. The source, prior-art, ecosystem, and count-versus-identity limits remain as recorded above. Sol runs no execution job.

VERDICT: NOT CLEAN (1 open) at bbd25214835b4648623cb1e5ac3a89ac50f5d260

### Round 2 final body and exact-head gate

READY `msg_plugin-w_1791728794_4169d1` supplies the unchanged head `bbd25214835b4648623cb1e5ac3a89ac50f5d260` and base `56fae5826f766b408abca06f99376e0506982bf9`.

Sol independently fetches the corrected published body and reads every changed line. It states 81 count comments and 203 entries. It names all four deleted exclusions and their prior counts. It records the new exact-head gate and the failed canonical base check. It keeps the earlier head's results as history. Every other body line remains unchanged, apart from a final blank line. EC-SI-1 closes without a source edit.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-exact-count-bbd25214-pool-20261011-071920-20943.log`.

SHA256: `b2767308e40a3cd842297b695a0450d79f90fe09e2bdcb3b0fdf8633064a35a7`.

Sol reads the exact head/base header, check results, both suite summaries, all eighteen raw mutation outcomes, all sixteen catcher reports, and all ten step results.

- All ten steps pass in 328.3 seconds. The job exits zero after 408 seconds. The wrapper exits zero after 409 seconds.
- The exact-count check passes: 204 exclusions against 6458 workspace mutants.
- The default suite passes 1690 tests, with 481 skipped, in 11.590 seconds. Both named parser and count tests pass.
- The slow suite passes 546 tests, with 1990 skipped, in 40.511 seconds. Static parsing confirms 546 distinct PASS identities and 179 real-conformance PASS identities.
- Slow coverage checks 1049 tests.
- Stage 1 tests eighteen mutants in 43.3 seconds: sixteen caught, two unviable, zero missed or timeout. The unmutated baseline passes after eleven seconds of build and one second of test.
- Sol independently matches all sixteen raw catch identities to the sixteen named reports. The previously missed signed-count operator is caught.
- Stage 2 does not run because stage 1 misses no mutant. Fuzz reports no changed decoder crate.
- The gate retains 30 real-pending IDs; four pass under the ignored selection. It reports 56/69 testkit-passing, 46/68 real-passing, and 47/69 real-accepted. Both pending files equal base. This PR removes none.

The earlier failed gate and failed canonical base check remain failed. Sol accepts no unsupplied local execution output independently. The exact-head full gate supplies the required final checks.

No integration finding remains open. Formal completion waits for the published package closure artifact. The unchanged-head gate requires no repeat under section 8. Sol runs no execution job.

### Round 2 closure — published package verdict and final body

Package receipt `msg_plugin-w_1791728888_f6843d` supplies published closure `7514a477b5b0c1fcca67e37d1cb4898624b2a729`, `verdicts/gs-gate-speed.md`. Sol reads the complete final addition and verifies publication on `origin/stage1/review-gs`. The package closes GS-EC-R2-1 and accepts the same exact head, base, source delta, and supplied gate.

The package identifies one additional body correction: the Changes bullet now states 81 entries instead of 85. Sol independently fetches the final published body and reads that complete one-line change. The head remains `bbd25214835b4648623cb1e5ac3a89ac50f5d260`. Every other body line equals the first correction. This final check supersedes the earlier closure's incomplete count check: the first correction still had 85 in that bullet. No source edit or repeat gate follows.

EC-SI-1 is fully closed. Zero integration findings remain. Both reviews are complete on the exact head and base. The failed canonical base check receives the required delta review; its failed result remains unchanged. All final gate results and raw/report checks above remain applicable. Count equality still does not prove identity equality after a one-for-one replacement.

The prior-art and ecosystem rationale remain valid. This PR changes no own mutation stage, selector, binary build, verdict rule, pin, or pending file. No further pending removal, future import, or unrelated finding is approved. The lead owns the merge decision and current-base check. Section 8 requires no repeat gate on this unchanged head. Sol runs no execution job.

VERDICT: CLEAN (0 open) at bbd25214835b4648623cb1e5ac3a89ac50f5d260
