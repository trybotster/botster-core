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
