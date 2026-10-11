# #261: known-flaky list

## Round 1

- Head: `80af70d2ea876b062c55002e64f2e1eb107ad247`.
- Base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Parents: `06001a7c5a3ca152fb493729b67b4bf49f7abe50` and the base above.
- Tree: `36f664a3d61b64c66a97a8c5423e4eceb36eaf9e`.
- Plan: 23aq, `8140a92991e82d69381cd618e9ce154afa746f97`.
- Risk: HIGH. The PR changes mutation and list decisions in the merge gate.
- Package verdict: `3eed51e604c090fc232dd68a69de6c6c1c477064`, NOT CLEAN (1 open), read in full.
- Package receipt: `msg_plugin-w_1791717788_aa4c7f`.

### GS-KF-R1-1 HIGH: one tier can leave a known-flaky test runnable

Status: OPEN. This is the package finding. Sol independently confirms its effect across both mutation stages.

known_flaky::tests_of selects entries only when entry.tier equals the stage's tier.
known_flaky::parse rejects a repeated binary/test identity, including an entry with a different tier.
ci::mutants_job supplies only Default entries to stage 1 and only Slow entries to stage 2.

A workspace library test binary can exist in both stages.
A default-feature test named slow_flaky runs in stage 1 and matches stage 2's existing slow filter.
An entry with tier default excludes the test only from stage 1. Stage 2 can still count its flake as a catch.
An entry with tier slow leaves stage 1 able to count that flake. The parser cannot express both stages for that identity.
The binary exists in both stages, so nextest's missing-binary check does not close this gap.

A slow-feature-only test can also reside in a shared library binary.
If its entry says default, the default binary exists but lacks that exact test. Stage 2 ignores the entry and runs the test.
The present fixture's absent slow_conformance binary does not cover this shared-binary case.

The current EV-5 entry uses a slow-only integration binary. This finding does not claim that EV-5 runs in stage 1 today.
It concerns the accepted general rule that no mutation stage runs a known-flaky identity.

Required correction: determine each exact identity's actual stage membership, or support every stage where that identity can run.
Exclude the identity from every applicable mutation stage. Fail closed when a declaration leaves the identity runnable in another stage.
Add a real-tool fixture with one identity selected in both stages and a slow-only test in a shared binary.
Correct the universal wrong-tier claim. A missing binary proves only the absent-binary case.
Keep unmutated test coverage, exact mutant selection, timeout values, and the empty-list Stage 1 exit requirement.

### Reviewed source and import

Sol read all five changed files and the complete PR body. The own diff adds 515 lines and deletes fourteen lines.
The lead's shared record approves PR 2's list, per-tier declaration, field validation, reporting, and empty-list exit requirement.
The earlier design's filtered second run is superseded by the lead's simpler exclusion form.
This is a temporary guard. It does not remove the requirement to fix EV-5 structurally.

The parser uses the existing TOML library. It rejects unknown keys and fields, non-string fields, missing fields, and empty fields.
It also rejects unsafe filter names, unknown tier values, invalid PR-number forms, and duplicate identities.
The filter subtracts exact binary/test intersections. Its parenthesized union preserves the original slow filter.
With no applicable entries, stage 1 adds no filter argument and stage 2 retains its original filter.
Unit tests compare the empty-list argument vectors with the previous vectors.

The normal slow step and standalone stage-2 baseline remain unchanged. The baseline still uses zero retries and includes EV-5.
The default test step remains unchanged. The new fixture runs real nextest commands through the existing owned-process helper.
It proves stage-1 exclusion of one test, retention of another test, and failure for an absent binary.
It does not prove exclusion across both stages or exact-test membership in a shared binary.
The implementer reports local red-on-revert checks. Sol does not reproduce them.

The list names only EV-5, with its diagnosis, P3 as owner, and #243 as the removing PR.
Plan 23ap's done item 7 requires the list to be empty at Stage 1 exit. It is not an exemption from completion.
The lists step and mutation step use the same parser. No new gate-time empty-list requirement is claimed.
The PR changes no pending ID, contracts pin, minimum list, timeout, retry policy, production hook, or candidate build path.
The existing prebuilt-worker mutation limitation remains unchanged.
Exact mutant identities, stage counts, stage-2 selection, catch reports, and missed/timeout verdict rules remain unchanged.

The head contains the exact current base. Sol inspected the merge's complete combined diff.
git merge-tree reports a ci.rs import conflict. The committed resolution retains known_flaky and slow_coverage.
This is a source delta review; Sol does not claim mechanical CLEAN carry-over through a conflict.
All eight changed imported paths outside the own diff retain the base's exact contents.
The two other shared-file changes add only the parser call and module declaration. The slow-coverage integration remains.

Sol checks Prior art at OLD CORE `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
Its Rust, Cargo, .cargo, and .config sources have no cargo-mutants, known-flaky, or mutation-gate match.
The PR uses nextest's existing filter primitives. It copies no old flaky-list implementation.

### Exact-head gate evidence and limits

Log: `~/botster-sessions/gates/botster-core-stage1-gs-flaky-list-80af70d2-pool-20261011-041047-3410.log`.
The header names the exact head and base above. All ten steps pass in 460.7 seconds.
The default tier passes 1637 tests in 17.085 seconds. The slow tier passes 478 tests in 46.829 seconds.
The slow-coverage check reports 1013 slow-only tests. The new real-tool fixture passes in 0.251 seconds.
The unmutated slow step runs EV-5 and passes it in 0.436 seconds.
All 58 real-pending IDs remain pending, with no new real-pending pass.
The minimum report remains real-passing 40/68 and real-accepted 41/69.

Stage 1 tests 31 unique mutants in 79.2 seconds: thirty caught, one unviable, zero missed, and zero timeout.
Its unmutated baseline passes, with fourteen seconds of build and two seconds of tests.
Sol independently matches all thirty full caught identities to all thirty catching-test reports.
Stage 2 and its baseline do not run because stage 1 misses no mutant.
This gate therefore exercises no nonempty EV-5 stage-2 filter. It supplies no evidence for the missing shared-stage case.
The complete mutation step takes 82.6 seconds. Fuzz starts no trial because no changed crate has a decoder harness.
The job and wrapper exit 0 after 574 seconds. The job queues one second and runs 573 seconds.

The green gate does not close GS-KF-R1-1. A corrected head needs review and its own exact-head gate.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (1 open) at 80af70d2ea876b062c55002e64f2e1eb107ad247
