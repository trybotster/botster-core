# PR #214: contracts-v0.1.23 pin

## Round 1

- Head: `08792a76fff142b14c72f77a1c3cf3a84e60c76f`.
- Base: `f6128fd6dd96a27f82520c82b9553d038dfa9dba` (`v1`).
- Contracts: `contracts-v0.1.23`, peeled commit `fe3eb952d6f61b89cf7bcbe8c54ab079987ec002`, manifest-final38.
- Scope: all six changed files, the relevant pinned dependency changes, list accounting, PR body, and existing gate evidence.
- Tier: HIGH under rule 4 (pin move), also rule 3 (shared crate and workspace configuration).

### Pin and dependency checks

All seven workspace contracts dependencies use the new tag.
All nine contracts packages in Cargo.lock use the same new tag and commit.
The lockfile changes only their source lines.
The local contracts tag peels to the recorded commit.
The pin matches plan revision 23p, previously reviewed CLEAN.

The upstream Core conformance driver now checks `progress_is_injected` before jumping an injected clock.
The method defaults to `injects_clock`.
At this head, `TestkitHarness` is the only CoreHarness implementation. It returns true from `injects_clock` and does not override the new method.
Its injected progress therefore retains the existing clock behavior.
The new driver behavior follows published ruling R-46.

The only route-codec source change is documentation of `TerminalQuery.query_id` as a decimal u64 string.
This Core head has no code that creates a `query_id`.
The changed plugin-contract crate is absent from Core's lockfile.
The changed `ou_2b_healthy_reasons_send_route_closed_last` transcript adds three Detach completion pumps.
That ID remains pending in Core.

### Conformance accounting

A read-only comparison against the pinned contracts ledger confirms the exact sorted Core ledger of 689 IDs.
The copied withdrawn and deferred files match the pinned files byte for byte.
The seven added A19 IDs are all pending in the pinned contracts.
The sole removed pending ID is `a9_1_frame_cap_equal_to_the_attached_frame_attaches`.
The pinned withdrawal names its A19 replacement.
That removed ID is absent from the 69-ID minimum list in plan revision 23p.

The active ID sets before and after the change are identical, with 124 IDs.
The counts change as follows:

| Set | Base | Head |
|---|---:|---:|
| Core ledger | 682 | 689 |
| Pending | 539 | 545 |
| Deferred | 2 | 2 |
| Withdrawn Core IDs | 17 | 18 |
| Active | 124 | 124 |

The pinned-file test changes the shared withdrawn count from 26 to 27.
The count without replacements remains seven.
No production Core code changes in this PR.
The prior-art note records reuse of the earlier pin-update procedure.

### Existing gate evidence

Log:
`~/botster-sessions/gates/botster-core-stage1-p6-pin-v0.1.23-08792a76-pool-20261009-195055-94076.log`.

The log names the exact head and base above.
The Linux pool job ran on `msa1` and exited 0 after 304 seconds.
All ten CI steps passed.
The default tier passed 1,329 tests. The slow tier passed 256 tests.
Both conformance reports show 124 passed and zero failed, with 475 pending transcripts and 70 pending IDs without transcripts.
The reports also show two deferred and 18 withdrawn IDs.
The list check confirms 689 IDs and matching status copies.

Both mutation commands report no mutants in the diff.
The only Rust change updates a test count and its comment.
The appended command sets `NEXTEST_PROFILE=slow`; it does not enable the `slow` feature.
This distinction does not leave changed production Rust code untested in this PR.

Remote head and base match the reviewed commits. The base is an ancestor of the head.
`git diff --check` passed. This review ran no builds, tests, or gates.

### Package verdict and result

I read the PR #214 section of `e8fbf108af3f80dc36e9d2cbc07635d7ed1ee5e8:verdicts/contracts-pin.md`.
The package reviewer reports CLEAN on the same head, with no open findings.
Its pin, dependency, accounting, and gate checks agree with this review.
A19 implementation and acceptance of RealCoreHarness remain separate work.

VERDICT: CLEAN (0 open)
