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
