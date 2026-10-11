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

## Round 2: exact-list correction

- Head: `b6b32f739bee9e5a5002b376452ee39a8245f55f`.
- Parent and previous reviewed head: `80af70d2ea876b062c55002e64f2e1eb107ad247`.
- Base remains `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Tree: `d11f2dff71f0112a972225e69a0b458751a6d896`.
- Package verdict: `ad72f6631c8b6b8bf62d78cf634c258af13b02a7`, NOT CLEAN (2 open), read in full.
- Package receipt: `msg_plugin-w_1791718229_e60957`.
- Implementer receipt: `msg_plugin-w_1791718088_67b418`.

Sol read the complete five-file delta: 320 added lines and 177 deleted lines.
The correction removes the tier field. The parser now rejects that old field.
Each stage intersects known identities with a preliminary nextest list. An identity present in both lists receives both exclusions.
The slow step checks for an identity absent from both normal tier lists. Stage 2 lists the workspace with its actual slow features.
This corrects the original single-tier representation case. It does not yet close GS-KF-R1-1.

### GS-KF-R1-1 HIGH: stage 1 lists a different package selection

Status: OPEN, with the original single-tier case corrected.

The stage-1 preliminary command uses nextest list --workspace. The actual stage-1 mutation command has no test-workspace override.
Sol checks the installed primary source of cargo-mutants 27.1.0:
- options.rs, lines 293 through 309, defaults to TestPackages::Mutated without a command or configuration override.
- lab.rs, lines 245 through 252, selects only the mutant's package for each mutated scenario.
- cargo.rs, lines 139 through 158, emits explicit package arguments and the selected feature arguments.

The reviewed command and .cargo/mutants.toml supply no override. Stage 1 therefore does not use the preliminary workspace selection.
Cargo feature unification can remove a test from a workspace build while a package-only build still includes the test.
The workspace list is not a guaranteed superset of the actual stage-1 tests.

Concrete accepted-feature case:
- Library A declares x and slow features. Another workspace member enables A's x feature.
- A known-flaky test slow_flaky uses `#[cfg(any(feature = "slow", not(feature = "x")))]`.
- The default workspace build enables x and lacks that test. The package-only mutation build of A can omit x and include it.
- The slow workspace build enables slow and includes the identity. The absent check therefore accepts its list entry.
- The stage-1 filter lacks the exclusion, so the known-flaky test can still count as a mutant catch.

This is a static counterexample, not an independently executed reproduction. It requires no malformed list entry.
No leak of the current EV-5 entry is claimed.

Required correction: cover stage 1's actual package-specific feature selections, or prevent a missing preliminary identity from adding a catch.
Add a real multi-member fixture that exercises this feature-unification case.
Do not broaden stage 1's test execution without separate review of that gate change.
The current PR body still describes the removed tier field and old safety argument. Correct it before closure.

### GS-KF-R2-2 MEDIUM: the slow-only fixture identity is already unselected

Status: OPEN. Sol identifies this finding; the package reviewer confirms it.

The new known_flaky fixture uses the shared-library test `tests::runs_only_under_slow`.
Its binary is botster-core. Neither its binary nor its test name matches `binary(/^slow/) | test(/(^|::)slow_/)`.
The copied fixture keeps mod tests unchanged. Stage 2 therefore omits this test before the known-flaky subtraction.
The test's absence from the filtered real run cannot prove its exclusion.
The string assertion proves that the generated filter names this listed identity. It does not prove the required selected-test exclusion.

Required correction: make this copied slow-only shared-binary test selected before subtraction, for example through a slow_tests module.
Assert its pre-exclusion selection or execution. Then prove its exclusion while an unrelated selected test still runs.
The shared identity slow_in_both and the slow-only integration binary remain useful fixture cases.

### Evidence and unchanged limits

The correction reuses slow_coverage's strict nextest JSON parser and exact binary/test keys.
The default normal tier, standalone stage-2 baseline, mutant identities, count checks, retry policy, and timeout values remain unchanged.
The mutation stage skips the new listing commands when the known-flaky list is empty. Existing empty-list argument checks remain.
The delta changes no pending ID, contracts pin, minimum list, candidate binary, or mutation exclusion.
The original Prior art check remains applicable.

The implementer says this head's full gate will follow its running exact-count job.
No exact-head gate log or completed real-fixture evidence for b6b32f73 has been supplied.
The older 80af70d2 gate does not cover this correction. This round accepts no gate or merge on the new head.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (2 open) at b6b32f739bee9e5a5002b376452ee39a8245f55f

## Round 3: verdict guard and selected slow fixture

- Head: `eb7f5c1264efe3346890d831af27c0cf3ba584ac`.
- Parent and previous reviewed head: `b6b32f739bee9e5a5002b376452ee39a8245f55f`.
- Base remains `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Tree: `b3d26b0c24c1303e082e28ae94e95112ae97e069`.
- Package verdict: `ce47e5bd8b4f3ea730b750c610f03e45938a4ce1`, NOT CLEAN (1 open).
- Package settlement record: `d14118927f592648d4c43ebb798734a1bf11135c`.
- Package receipt: `msg_plugin-w_1791718612_5101ff`.
- Lead settlement: `msg_plugin-w_1791718538_9b16aa`.
- Implementer receipt: `msg_plugin-w_1791718480_dda01f`.

Sol read the complete eight-file delta: 268 added lines and 23 deleted lines.
Sol read both complete new package additions as local Git objects. The package reviewer reports that DNS prevented both pushes.
Those local objects supply review evidence. Sol does not claim their remote publication.

### GS-KF-R1-1 HIGH: a cancelled test can replace the listed first failure

Status: OPEN. The correction adds a report guard for identities absent from the preliminary list.
The new multi-member fixture represents the workspace/package feature difference from Round 2.
Its workspace list lacks a tests::guarded. Its package list includes that test.
The fixture checks the same mutation output with and without the listed identity.
The listed case expects a report error when the only failed test is known-flaky.

However, mutant_logs::report searches every final failed test for an unlisted identity.
The existing CATCH fixture records the first FAIL, cancellation, and a later SIGTERM from another test.
The read_run parser retains both failures in that order. The first-final-failure unit test establishes that order explicitly.
If twostage::slow_triple conf::b is listed, the new search skips it and credits twostage tests::c.
That later failure comes from cancellation caused by the listed failure. It supplies no independent mutant catch.
The new two-FAIL unit case explicitly expects credit for the second failure. It does not close the cancellation case.
This conclusion follows from static source inspection. Sol ran no reproduction.

The lead settles the third-round finding under the round limit.
Only the first final failure under --max-fail 1:immediate can count as a catch.
A listed first failure makes the mutant MISSED. No later failure in that cancelled run receives credit.
A cancellation-caused failure, including SIGTERM or another cancelled-run failure, is never an independent catch.
The correction must classify the mutant as MISSED; a report error alone does not implement this settled classification.

Round four covers this rule and two fixtures:
- A listed first failure followed by cancellation gives MISSED.
- An unlisted first failure gives a catch.

Other new findings require a follow-up unless they are HIGH. Sol will apply this scope to the next exact-head review.

### GS-KF-R2-2 MEDIUM: CLOSED in source

The copied known-flaky fixture renames mod tests to mod slow_tests before listing or execution.
The original slow filter now selects its slow-only shared-binary test before subtraction.
The fixture checks pre-exclusion execution, post-exclusion absence, and retention of an unrelated selected test.
These source assertions close the selection defect. Their execution evidence remains unaccepted until the exact-head gate is supplied.

### Evidence and unchanged limits

The implementer reports that the PR body now describes the verdict guard and removed tier field.
Sol has not independently read that updated body. The earlier body correction remains a closure check.
The default normal tier, slow step, and standalone stage-2 baseline remain unchanged.
The delta changes no timeout, retry policy, mutant selection, pending ID, contracts pin, or minimum list.
The original Prior art check remains applicable.

The implementer reports a running exact-head gate. No completed eb7f5c12 gate log has been supplied to Sol.
The older 80af70d2 gate does not cover this correction. This round accepts no new gate or merge.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (1 open) at eb7f5c1264efe3346890d831af27c0cf3ba584ac
