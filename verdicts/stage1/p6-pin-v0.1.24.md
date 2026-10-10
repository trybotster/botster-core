# PR #216: contracts-v0.1.24 pin

## Round 1

- Head: `5b150cf4c5d704673ce6c6284723384c48ae5987`.
- Base: `9103dca15664496630c4397ea3a60721427b38da`.
- Contracts pin: `contracts-v0.1.24`, commit `d79aed5e84d4a8bc9b76f6163ca6bc4a9d89b951`, manifest-final39.
- Tier: HIGH under rule 4, also rule 3 for the shared crate and workspace files.
- Scope: the full five-file delta, relevant upstream source, local conformance runner, list accounting, PR description, and supplied gate.

### Pin and list checks

The seven direct dependencies and nine lock sources consistently name the new tag and commit.
Only the contracts source lines change in Cargo.lock.
The tagged amendment, manifest, and transcript changes were also checked in plan revision 23q's review.
The PR contains its prior-art note and limits its source changes to comments.

The Core ledger matches the tagged Core ledger byte for byte: 690 IDs.
The withdrawn and deferred copies remain equal to the tag.
The local pending set does not change.
The 182 active IDs on the base remain active.
The sole added active ID is `conf::a20_1_testkit_proven_ids_are_exactly_the_named_list`.
That addition has the proof gap below.

### R1-1 — HIGH — the new A20 pass does not prove Core's local conditions

The frozen A20 test requires the real runner's testkit-proven list to equal the named list.
It also requires: "Each listed id passes on the testkit."
At this head, Core still lists `conf::dp_3_frame_limit_checked_before_allocation` in `conformance/core-pending.txt`.
No Core runner consumes `TESTKIT_PROVEN` yet.

The new transcript contains only `type_check: a20_1_testkit_proven`.
The upstream driver compares its own constant with the frozen A20 text.
It then calls `a20_1_proven_ids_are_active` with the contracts crate's embedded pending and withdrawn files.
Those inputs describe contracts metadata. They do not establish Core's local passing proof or its real-runner selection.
The local `tests/conformance.rs` adds no A20 validation; it runs this upstream transcript unchanged.
Thus, the reported 183rd pass does not establish the required Core conditions.
The PR body correctly identifies the upstream inputs but still counts the ID as proven.

For this pin-only PR, add the new A20 ID to Core's pending list with its missing local proof and runner wiring as the reason.
Alternatively, provide the actual Core conditions before counting it as passed.
The pin move does not require expanding this PR into the real-harness implementation.
The reviewer sent the finding to P6 and the package reviewer.

### Existing gate evidence

Log:
`~/botster-sessions/gates/botster-core-stage1-p6-pin-v0.1.24-5b150cf4-pool-20261009-203054-60221.log`.

The log names the exact head and base above.
The Linux pool job ran on `msa1` and exited 0 after 286 seconds.
All ten CI steps passed.
The default tier passed 1,391 tests. The slow tier passed 256 tests.
Both conformance reports show 183 passed and zero failed.
The new A20 trial and all four changed existing transcripts have PASS lines.
Those lines do not close R1-1 because the new A20 trial reads the wrong repository's proof state for this claim.

The prebuild step compiles the conformance probe from the pinned `d79aed5` checkout.
The R-48 probe changes were read with plan 23q: terminal input disables canonical mode and echo, while nonterminal input remains unchanged.
Both mutation steps report no mutants, consistent with the comment-only Rust delta.
The appended command sets a profile environment variable; it does not enable the slow feature.
No new production Rust or mutation exclusion is introduced here.

Remote head and base match the reviewed commits. The base is an ancestor of the head.
`git diff --check` passes. The reviewer ran no builds, tests, or gates.


R1-1 was open on the first head. The package reviewer independently confirmed the same gap.
The implementer accepted the finding and supplied the replacement below before this review was published.

## Round 2

- Head: `d18b41726b4fffb1e2043d5220b32a34444cd27b`.
- Base: `9103dca15664496630c4397ea3a60721427b38da`, unchanged.
- Delta: the full one-file correction after `5b150cf4c5d704673ce6c6284723384c48ae5987`.

### R1-1 closed

The new A20 ID is now pending in Core.
Its reason names both missing conditions: Core's real-runner use of the contract list and the passing testkit allocation proof.
It also identifies the responsible packages.
The ID is new in this pin, so adding it as pending follows the pin-move rule.
The active set now exactly equals the base's 182 IDs.
No existing pass is lost, and the upstream metadata check is no longer reported as a completed Core proof.

### R2-1 — LOW — reference baseline did not run the testkit tier

The updated body says every reference log contains 121 passing testkit trials.
The baseline `cde60ed1` command runs only prebuild-worker and slow_conformance.
Contracts PR #22 correctly reports that baseline testkit run as absent.
Only `99f643b7` and `a46b7958` ran both tiers.
The reviewer checked all three raw logs during plan revision 23q's review and requested a body-only correction.

### Replacement gate

Log:
`~/botster-sessions/gates/botster-core-stage1-p6-pin-v0.1.24-d18b4172-pool-20261009-203757-85930.log`.

The log names the exact replacement head and unchanged base.
All ten CI steps passed. The Linux pool job exited 0 after 73 seconds on `msa1`.
The default tier passed 1,390 tests. The slow tier passed 256 tests.
Both conformance reports show 182 passed, zero failed, 418 pending transcripts, and 70 pending IDs without transcripts.
The ledger check reports 690 IDs, 488 pending, two deferred, 18 withdrawn, and 182 active.
Both mutation steps report no mutants, consistent with the source scope.
Remote head and base match. `git diff --check` passes.
The reviewer ran no builds, tests, or gates.

### R2-1 closed

The corrected body now says that `cde60ed1` ran only the real tier.
It limits the 121-of-121 testkit evidence to `99f643b7` and `a46b7958`.
The reviewer verified that correction on GitHub at unchanged head `d18b41726b4fffb1e2043d5220b32a34444cd27b`.

The reviewer also read the original-head package verdict at `8c205c85c432f79f3cbdefe3bfb0b28e96db0fb1:verdicts/contracts-pin.md`.
It records the same A20 gap and no other source finding.

### Final package verdict and result

The reviewer read Round 2 at `9742cabde81b571fb43c6edb09df239b0ddcf986:verdicts/contracts-pin.md`.
It reports CLEAN on the same head and closes both findings.
Its source, accounting, and gate conclusions agree with this review.
The A20 runner check and allocation proof remain future implementation work, tracked as pending.
This verdict accepts the pin move without claiming those proofs are complete.

VERDICT: CLEAN (0 open) at d18b41726b4fffb1e2043d5220b32a34444cd27b
