# PR #306 integration review

## Round 1 — source review before READY

Exact head: `4d49be0b70a5193ee96c57a84f29fb9ede56b8b6`. Base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`. Branch: `stage1/gs-built-in-coverage`. Risk: HIGH. Lead assignment: `msg_plugin-w_1791779564_07e7f5`.

The lead explicitly requires this source review before READY. The final gate waits for #299's budget repair, then one v1 import and one ordinary gate. This review authorizes no retry of the existing red gate.

Sol-B read the design, the lead's approved trial macro and slow-filter changes, all 39 changed files, relevant list parsing and gate context, the complete PR body, and the supplied red gate records.

The libtest macro uses the caller's compile-time package name and manifest directory. The root calls replace the earlier handwritten tests. Integration test files and the anchor binary receive the same shared macro. Custom harnesses receive the shared trial macro. The conformance suite retains its LINKED check before trial construction.

The slow filter selects the exact root name. Mutation stage 2 uses the same filter. No timeout, budget, seed, or deadline value changes. The pure presence check rejects a missing, ignored, unselected, or nested root name for binaries represented in its test map. Two gaps prevent the required coverage guarantee.

### B306-I1 — HIGH — OPEN: an empty binary disappears from the presence check

Evidence: `xtask/src/slow_coverage.rs`, parse_list and presence. parse_list adds only testcase keys to its map. A suite with status listed and an empty testcases object contributes no key. presence derives its binary set from those keys.

Removing built_in_test!() from the anchor binary leaves that binary with zero tests. The nextest suite can still be listed with testcases {}. The parser then discards its identity. Other binaries keep the combined list nonempty, so list's overall emptiness check does not reject it. presence can pass without checking this built binary.

Required closure: preserve the listed binary inventory independently from testcase keys, or reject each empty listed suite with its binary name. Every listed binary must have the required selected, nonignored root test. Add a negative fixture with one valid binary and one empty binary. The valid binary must not hide the empty binary.

Sol-B found this path through source review. Package reviewer 017c independently confirms it in `msg_plugin-w_1791779661_b72911`.

### B306-I2 — HIGH — OPEN: a custom harness can use the public name without the macro

Evidence: `crates/botster-test-process/src/built_in.rs` exports TEST_NAME. `xtask/src/built_in.rs` rejects only the literal name in other files. The presence check verifies the listed name and selection flags, not execution of the shared check.

A custom harness can replace built_in_trial!() with:

```rust
libtest_mimic::Trial::test(botster_test_process::built_in::TEST_NAME, || Ok(()))
```

The caller contains no forbidden literal. The trial lists the exact required root name. It is selected and not ignored. Its empty body passes without checking the build root. Both the name rule and presence rule therefore accept it.

This bypass uses existing public interfaces. It needs no procedural macro or dependency change. The design's recorded procedural-macro limit does not cover it. The body's claim that only the shared macros can make the test is false under this implementation.

Required closure: establish the approved structural entry or a runtime proof for custom-harness binaries. The listed name alone cannot establish that the root check ran. Retain a negative fixture for this public-constant bypass. Correct the body and design claims to match the resulting guarantee. Do not restore the incomplete body visitor removed from #293.

Sol-B sent both findings to GS and 017c in `msg_plugin-w_1791779628_f5de5b` and `msg_plugin-w_1791779628_5f37fa`. Package reviewer 017c independently confirms this second gap. Its completed formal verdict remains due. This review claims no package CLEAN.

### Evidence and remaining requirements

Earlier gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-902d594e-pool-20261011-211849-56885.log`. It fails test-budget on the old filter assertions. The reviewed repair replaces those assertions with the current filter. The earlier failure remains recorded.

Exact-head gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-4d49be0b-pool-20261011-212855-27306.log`. It fails test-budget on `conf::in_9_legacy_keys_table` at 2.002 seconds against the unchanged 2-second limit. The job and wrapper exit 1 after 99 seconds. The gate does not reach the completed slow, mutation, and fuzz steps. It supplies no acceptance result for this head.

The lead forbids a re-gate and requires the #299 budget repair before the single import and ordinary gate. The body's AFTER_PENDING timing entry remains pending. Completed slow-step and stage-2 timing comparisons, required reversal evidence, the exact-head package verdict, and the final body remain due.

The mixed case of a rebuilt binary with a stale linked library remains an explicit limit approved by the lead. The shared-target mitigation remains required. These findings concern the promised binary coverage and shared entry, not that separate mixed-library limit.

Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: NOT CLEAN (2 open)

## Package verdict receipt

Sol-B read completed package verdict `6e3a75b7098eaec3bafe0d6b8772a2b6af7cf8e3`, `verdicts/gs-gate-speed.md`, on the same exact head and base. It reports two HIGH findings, GS-BC-R1-1 and GS-BC-R1-2. They match B306-I1 and B306-I2 and require the same closures.

The earlier pending-package statement is superseded by this receipt. No finding closes and no new finding is added. Completed gate, timing, and required evidence remain pending under the lead's import plan. Sol-B ran no execution job.

VERDICT: NOT CLEAN (2 open)

## Round 2 — source repairs and accepted design limit

Exact head: `354c59e78e2232543b06a4705e968fba60daed5f`. Base remains `fcc09461cb0c0de4ef067c160f24d205f0f65c30`. Sol-B read the complete four-file repair at `4649a610` and the following one-file documentation commit. Sol-B read the complete amended PR body and the updated pair-common review checklist.

B306-I1 is CLOSED for source review. parse_list now returns List with an independent binary set and testcase map. The binary set comes from the suite keys, including a suite with no testcases. presence checks that set. The new fixture contains an empty anchor and a valid binary. It requires one violation naming the anchor. The overall empty-list check remains. Other coverage rules retain their testcase map and existing decisions.

B306-I2 is CLOSED under the lead's accepted reduced guarantee. The shared test crate removes its public TEST_NAME constant. The original constant-based bypass no longer resolves. The name rule allows two literal occurrences in the macro file and three in the filter file. A fixture rejects an additional exported constant. Xtask holds its own copy; no test crate depends on xtask.

Sol-B found that an assembled name can still create an empty trial that passes presence. Removing the public constant does not supply complete runtime proof. Sol-B asked the lead for a disposition in `msg_plugin-w_1791780031_19cfcf`.

The lead accepts the reduced guarantee in `msg_plugin-w_1791780044_f7eba4`. The lead defines the check's scope as accidental coverage gaps. Reviewers must reject trials that take the root name outside the approved macros. This decision supersedes Round 1's requirement for a complete structural or runtime guarantee for custom trials.

Sol-B verified all three lead conditions on this exact head:

- The macro doc and PR body state: "a trial that takes the root name without calling the macro is not detected; reviewers reject any trial with that name outside built_in_test!/built_in_trial!".
- The shared test crate exports no public name constant.
- `/Users/jasonconigliari/botster-sessions/shared/core-stage1/pair-common.md:108` requires reviewers to reject any root-named test or trial outside the approved macros.

Approval relies on that manual review rule. It does not establish mechanical exclusive ownership of the runtime name. The rule also covers a caller that copies the name from a macro-created Trial before constructing an empty replacement. The current custom harnesses use built_in_trial!(). Sol-B found no such replacement caller.

The macro retains fixed compile-time arguments and the shared root-check callee. The previously reviewed root and integration entries, LINKED check, candidate paths, and slow filter remain unchanged. No timeout, budget, seed, or mutation exclusion changes. The mixed stale-library limit remains explicit.

Package source CLEAN: `4c064196a78b0db2154435b8e68500e1cfa88ecc`, `verdicts/gs-gate-speed.md`, on this exact head and base. Sol-B read the complete source-repair section. Both reviews close the same two findings under the same accepted limit. No source finding remains.

Local checks on the repaired source remain implementer reports. No completed pool gate validates this amended head. The #299 import, applicable base review, one ordinary gate, required timing and reversal evidence, and final body remain due under the lead's plan. This source CLEAN accepts no completed gate result or later head. It authorizes no same-head retry.

Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; source only, evidence pending)

## Round 3 — imported head and first slow-fixture repair

Imported head: `ee61090e81650831dcd9131b9cdacb24d70255f9`. Parents: reviewed source head `354c59e78e2232543b06a4705e968fba60daed5f` and current v1 `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. The canonical report `gates/botster-core-stage1-gs-built-in-coverage-base-merge-check-354c59e7-to-ee61090e.log` reports every condition PASS and exits 0. Sol-B read the complete retained report. It carries the earlier source review to this import, with the final gate still required.

The imported-head gate `gates/botster-core-stage1-gs-built-in-coverage-ee61090e-pool-20261011-220352-43493.log` fails the slow fixture test. The fixture's real nextest lists add three missing-root violations to the existing unselected-test violation. The job and wrapper exit 1 after 393 seconds. Mutants and fuzz do not run. This remains a failed gate.

First repair head: `c20029a68cbb80612d242e7ce8560260bb7460e4`. Sol-B read its complete one-file delta and affected fixture context.

### B306-I3 — HIGH — OPEN at c20029a6: the positive fixture violates the required manual rule

The repair writes `#[test] fn {TEST_NAME}() {}` into the temporary library source. It also prints the reserved trial name in the custom fixture list. Neither entry uses the approved macro or calls the root check. The positive fixture accepts both entries.

pair-common.md:108 requires rejection of every root-named test or trial outside built_in_test!() or built_in_trial!(). The lead's reduced guarantee depends on that rule and contains no positive-fixture exception. The repair therefore uses the unsupported empty entry that reviewers must reject.

Required closure: use approved macros for positive compiled entries, or test list data without creating a reserved-name test outside those macros. Do not claim an empty body performs the root check. A running gate cannot close this source finding.

Sol-B independently confirmed package GS-BC-R3-1 and sent the finding to GS and 017c. Sol-B read formal package NOT CLEAN `cee142d94605650f865bb0bf7571e2bf3c8fe4dc`, `verdicts/gs-gate-speed.md`. It records the same finding on this exact head. Sol-B ran no execution job.

VERDICT: NOT CLEAN (1 open at c20029a6)

## Round 4 — repaired fixture source

Exact head: `c69cbcb85e5d173468703bdea45bb3a325e87366`. Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. Sol-B read the complete repair and net delta from the imported head. Only xtask/src/slow_coverage.rs changes after the import.

B306-I3 is CLOSED for source review. The repair removes both generated reserved-name entries. It creates no root-named test or trial outside the approved macros. The tracked fixture files equal current v1. Its existing module rename remains the slow-filter case.

The checked closure calls the actual list checker. Each call separates the three tier-prefixed presence violations and asserts their exact binary and tier prefixes. An extra, missing, or changed presence result fails that assertion. The closure then returns the remaining violations to the original assertions. The unselected-test, proof-selection, renamed-module, and ignored-pending-trial cases retain their checks.

This separation exists only in the slow test. The production presence checker, name rule, shared macros, filters, candidate checks, and gate decisions remain unchanged. B306-I1 and B306-I2 remain closed under their recorded scope and the lead's accepted manual rule.

Canonical repaired-head report: `gates/botster-core-stage1-gs-built-in-coverage-base-merge-check-354c59e7-to-c69cbcb8.log`. Sol-B read the complete report. Ancestry and conditions 1, 2, and 4 PASS. Condition 3 FAILS because the fixture changes the own patch. This explicit delta review covers that change. It does not relabel the canonical FAIL as PASS.

Package source delta CLEAN: `ce3184b72510c3e0326338c76836b78242f8acd3`, `verdicts/gs-gate-speed.md`. Sol-B read its complete repaired-fixture section on this exact head. No package or integration source finding remains.

The c20029a6 gate was stopped, according to GS, and supplies no completed result. The gate on this repaired head is running. Sol-B read no running result. Local checks remain implementer reports. The completed gate, required timing and reversal evidence, and final body remain due. The live body still needs the imported-head failed gate and repair history in its final update.

Sol-B ran no local build, test, gate, mutation job, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; source delta only, evidence pending)

## Round 5 — slow-target feature requirements

Exact head: `2fa58b79291f44023778a8a53f8bd05387e93cfb`. Base remains `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. Sol-B read the complete five-manifest delta from `c69cbcb8`, all eleven affected file headers and root macro entries, the owning feature declarations, and tier selection.

The c69cbcb8 gate remains failed. Log: `gates/botster-core-stage1-gs-built-in-coverage-c69cbcb8-pool-20261011-221807-53060.log`. Its slow tests pass, but the presence check rejects eleven empty default-tier binaries. The slow step fails with eleven violations. Mutants and fuzz do not run. The job and wrapper exit 1 after 586 seconds, with 469 queued and 117 running. Sol-B read the named violations and completed exits.

Every affected source file already requires the slow feature at crate level and contains the approved root macro. Every owning package declares that feature. The manifest now requires slow for the corresponding test target. Cargo therefore does not build its empty default-tier artifact. The slow tier enables those package features and selects the same target names. Mutation stage 2 enables the same slow-package features.

The repair removes no previously executable default-tier test. It deletes or renames no target. The slow test bodies, selected slow tests, macros, presence decisions, and public product paths remain unchanged. The empty-binary rejection remains intact. No fake root entry or presence exception is added.

Package source delta CLEAN: `90cdc8686f24e25b62216e39193aaa12e3dda535`, `verdicts/gs-gate-speed.md`. Sol-B read the complete manifest-delta section. No package or integration source finding remains. The accepted manual name rule and mixed stale-library limit remain.

The new-head gate is running. Sol-B read no unfinished result. Local list and check results remain implementer reports. Completed gate, mutation evidence, timing comparison, required evidence, and the final body remain due. All failed gates retain their original results. Sol-B ran no execution job.

VERDICT: CLEAN (0 open; source delta only, evidence pending)

## Round 6: current-base import, command proof, and completed evidence

Exact head: `0a09168702048cf34ef3fa30dffc07131b620f0a`.
Tree: `410fd2c0bfd96c4db9ab7adbf79336cc30d42a44`.
Imported and gate base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
Previous accepted source: `2fa58b79291f44023778a8a53f8bd05387e93cfb`, based on `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
Merge `a217da8226048e9cff904f629faee36b5da0df5a` joins that previous source and the imported base.
The final head has that merge as its only parent and adds one test in xtask/src/built_in.rs.

Sol-B reads the complete canonical report:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-base-merge-check-2fa58b79-to-a217da82.log`.
Ancestry and conditions 1 and 4 PASS. Conditions 2 and 3 FAIL on four overlapping paths.
This manual delta review covers the overlap. It does not relabel the canonical FAIL as PASS.

Sol-B independently compares all 44 own paths and every added and deleted line before and after the import.
The path set and normalized own patch match exactly. No extra own path appears.
The four overlap files retain the imported signal rules, op_holds module, and DP-2 proof changes.
They retain the shared root macro changes from #306 alongside that imported source.
The source retains the required-features repair, selected slow root test, presence decisions, candidate checks, and LINKED check.
No conformance file, pin, mutation exclusion, process allowance, or CI profile changes relative to the imported base.

The imported-head gate remains RED:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-a217da82-pool-20261012-091300-76813.log`.
Sol-B reads its survivor and final exits. The command replacement with Ok(()) survives both stages.
Mutants fails. Fuzz does not run. The job exits 1 after 686 seconds; the wrapper exits 1 after 687 seconds.

The new test calls the actual command on two tracked fixture repositories.
An allowed Rust file plus a non-Rust file with the reserved name passes.
A tracked Rust file with that name fails with exactly one violation.
The test creates scanner input. It compiles no fake root entry and accepts none as an executable proof.
The existing fixture helper tracks files through Git. The new test starts no Cargo process.
The command and its production decisions remain unchanged.

Reversal record: `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/gs-306-command-reversal-0a091687.log`.
Sol-B reads the complete record. It names this head, the GS target, cleanup time, and unchanged worktree.
The scratch early return produces an assertion failure because unwrap_err receives Ok(()).
The restored source passes the new test among three selected tests.
The record's mutant exit field is blank; its failure summary and assertion supply the recorded negative result.
The cleanup and restoration facts remain owner-recorded provenance. Sol-B claims no independent execution or cleanup verification.

Focused mutation log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-0a091687-focused-mutants.log`.
SHA256: `a8c477ffa2c28e803796d50bc97833f5e9a51750e0d065dbafbd96f2ee9236a1`.
Sol-B reads its commands and all outcomes. They use --no-config on the two changed checker files in both stages.
Both stages test the same 17 identities. These identities equal the completed gate's 17 identities.
Default catches 11 and misses four, with two unviable. Slow catches seven and misses eight, with two unviable.
Neither stage reports a timeout. The intersection of the two missed sets is empty.
No configured exclusion hides one of those listed mutants. The focused shell exit alone supplies no combined verdict.
The wrapper exits 0 after 292 seconds on msa1. The ordinary gate below supplies valid cited catches.

Completed gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-0a091687-pool-20261012-093420-53357.log`.
SHA256: `d7a0683fdefdb35551719a6d3e22b10fd3f7881002926895a6d3c51d10ca4ba5`.
Sol-B reads the exact identity, base accounting, changed-test result, presence result, mutation records, citations, summaries, and exits.
Gate and merge base both equal 838e4bb0. All ten steps PASS in 701.0 seconds.
Default passes 1,952 tests; slow passes 648 tests. The name check reads 244 Rust files.
The root tests and presence checks pass. The new command test passes in 0.013 seconds.
Stage 1 tests 17 mutants: 11 caught, four missed, two unviable, and zero timeout.
The unmutated stage 2 baseline passes 658 tests. Stage 2 catches all four exact misses.
The final result has 15 caught, two unviable, zero missed, and zero timeout.
Sol-B independently matches all 15 raw catch identities to citations. Every catcher has a normal PASS.
The new command test catches the former command survivor in stage 1.

Stage 2 records one FLAKY in_8 result under the empty slow_coverage::tests replacement.
The failed-then-passed trial receives no catch credit under BUILD rule 9's mutation-profile exception.
The valid catch is ci::slow_tests::slow_a_listed_test_that_the_workspace_list_lacks_is_still_no_catch.
Its raw catch, citation, and normal PASS match. The FLAKY record remains in the evidence.
The structural fix remains PC's contracts #79 through the .44 pin. This exact head still contains .43.
This acceptance grants no ordinary retry exception and claims no in_8 repair in #306.
The four selected fuzz harnesses complete their runs.
The job exits 0 after 716 seconds on msa1; the wrapper exits 0 after 717 seconds.

Sol-B verifies the timing samples in the body against the retained completed logs.
The earlier #293 gate at 029a740b has a 106.8-second slow step and runs no stage 2.
This head has a 92.0-second slow step, a 54.2-second mutation baseline, and four stage 2 mutants in 131.0 seconds.
The full mutation step takes 234.1 seconds.
The samples use different bases, nodes, and mutant sets. They establish no measured gain or small added cost.
Local Mac results retain their earlier owner-report scope. This delta adds no platform implementation change.

Sol-B reads the full exact-head body and complete applicable package section at `b98047a8ab44fd06d5ee3233d8a62e9b85d7b2c2`.
The package receipt agrees with the independent checks and reports zero findings.
The body retains the failed gates, repairs, focused evidence, reversal, timing limits, and FLAKY disposition.
All earlier source findings remain closed. The manual name rule and mixed stale-library limit remain accepted and explicit.
No source, body, or supplied-evidence finding remains on this exact head.
The lead owns the next slot or train, its applicable delta and final gate, and merge/tree/push verification.
This receipt accepts no later head and authorizes no additional retry.
Sol-B ran no build, test, gate, mutation, reversal, diagnostic, or executable base check.

VERDICT: CLEAN (0 open; reviewed import and command proof with completed exact-head evidence)
