# Integration review: PR #251

VERDICT: CLEAN (0 open)

- Head: `7b942de5726df1e1bc17d178a65740d661d78461`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Package CLEAN: `7b43944289102857556746aa75d0fa941931fa3c`.
- Branch: `stage1/gs-mutants-stale`.
- Risk tier: HIGH.

## Source and rejection checks

Sol read the complete four-path diff, the new checker, its callers, the reused parsers, and the tool-result helpers.
The new command lists the workspace's mutants with cargo-mutants 27.1.0, --list --json --no-config.
The listing performs no build and includes code that cfg excludes from the gate's OS.
The checker combines configured exclude_re entries with OFF_MACOS_EXCLUSIONS.
It uses the full mutant name and the existing regex search semantics.
A pattern with no match produces a violation that names the pattern, file, and commit.
An invalid regex, failed listing, invalid output, or failed source read returns an error.
The command rejects an unknown argument. Empty mutant input rejects every configured pattern.
The gate's taint step invokes this command and propagates a failed result.
Derived platform exclusions come from the current tree and remain outside this check.

The checker adds a rejection condition. It changes no mutation selection, profile, timeout, test tier, or two-stage verdict.
The process command has an explicit exclusion because a mutant cannot list the workspace from inside that same run.
The pure decision, path diagnostic, and parsers retain their proofs and mutation coverage.
The tests cover live patterns, moved patterns, empty names, invalid regexes, and complete diagnostic output.
The checker cannot detect a pattern that moves onto a different mutant. The PR states that limit.
This review does not approve a function-wide replacement for the existing line and column anchors.

## Existing exclusion corrections

The PR repairs four stale entries. Sol checked their source, reasons, cited proofs, and movement history.
The worker READ_CHUNK exclusion moves from line 47 to line 53. It still names only the positive multiplication tuning mutant.
The testkit read bound moves from line 38 to line 50. It keeps the same positive-bound rationale.
The public byte-retention proofs remain. Their Linux records pass at the reviewed head.
Zero-bound decisions retain their separate rejection proofs and remain mutation-tested.

The Linux ended_since_listing guard moved from live_members to live_where in commit 9873f33d.
Sol read that move and the current guard. The corrected pattern retains the existing kernel-race rationale.
It excludes no additional decision beyond the moved guard. The ended_since_listing function remains subject to mutation testing.
The removed Mac entry names a deleted conjunction. Commit 996a17f0 split that conjunction into separate checks.
Sol read that move and the current selection code. Removing the stale entry excludes no current mutant.
The new entry for the command's process glue does not exclude its pure decision.

No pin, transcript, pending ID, held ID, minimum, or real-only list changes.
The historical AD-7 tally still requires the R-59 fence before it counts toward the real minimum.
This pure checker and text correction change no OS adapter behavior and require no new Mac process proof.

## Prior art and evidence

Sol searched pinned OLD CORE 72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c for the corresponding xtask and mutation paths.
OLD CORE has no corresponding checker to port.
The implementation reuses gate_decisions parsers, tools helpers, and regex. The new decision is limited to missing matches.
The PR's Prior art record identifies that reuse and the agreed file split with P6.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-mutants-stale-7b942de5-pool-20261011-011635-60299.log`.
Sol read the exact-head/base header, relevant proof records, all six mutant identities, and the complete final summary.
The checker compares 207 exclusions with 5991 workspace mutants.
All ten steps pass. The CI summary is 354.0 seconds. The job exits 0 after 433 seconds; the wrapper exits 0 after 434 seconds.
Default tests pass 1612 cases. Slow tests pass 427 cases.
Stage 1 catches all six mutants: two path-diagnostic returns, three stale-decision returns, and deletion of its negative match condition.
Stage 2 has no missed mutant to run. No mutant times out or is unviable.
The fuzz step finds no changed decoder harness.
The body reports a rejection after restoring the old constant line. Sol has no retained reversal log and makes no independent reversal claim.
The retained negative unit proofs and mutation outcomes support the new rejection behavior.

Sol read the published head, base, body, and package verdict section.
The head contains the reviewed v1 base through its merge parent.
The final gate after both CLEANs remains required under the GS brief. This source CLEAN grants no merge approval by itself.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.


## Source import and gate gap

The reviewed 7b942de5 head imports v1 80d02327 into a92d74840c3133de4d70b128a50fae95972440d9.
Sol read the complete log: ~/botster-sessions/gates/botster-core-gs-mutants-stale-base-merge-check-7b942de5-new.log.
Both ancestry checks and all four numbered conditions pass. Both pending files retain the base.
Sol's independent git merge-tree equals the actual tree, f0c23db244ee7d7f0f62891a5c7a1ce0b83da8e8.
Both complete own diffs are byte-identical: 15970 bytes, SHA-256 5653c7fdacfae4f33c10187ad9db44044e4b27fe741a05c6b4938dbec8b66df4.
Source CLEAN carries to a92d7484. The lead confirms that it merged without its required exact-head full gate.
The gap and pending v1-tip remediation are recorded in verdicts/stage1/exact-head-gate-remediation.md.
