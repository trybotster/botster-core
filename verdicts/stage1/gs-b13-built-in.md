# PR #293 integration review

## Round 1

- Reviewer: Sol-B.
- Branch: `stage1/gs-b13-built-in`.
- Exact head: `2c0488b8d8f55e3ad05992f97b4de5ca95b9b9c3`.
- Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- Reviewed delta: both commits and all 27 changed files.
- Risk: HIGH. The PR changes shared test code, package dependencies, candidate validation, and gate decisions.

The root comparison has one implementation in `botster-test-process`. The testkit re-exports it. The conformance entry point checks the test binary and nine linked libraries before listing or running trials. The package constants add no terminal behavior or test branch to the production machines. New package dependencies are dev-dependencies, except the testkit's dependency on `botster-test-process`.

### B13-I1 — HIGH — OPEN: prebuild can assign the current root to stale artifacts

Evidence: `xtask/src/prebuild.rs:151` converts every failure to read `source-root` into `None`. `other_source_root(None, ...)` accepts it. Line 161 writes the current root before lines 166–168 build and copy the binaries.

A normal `cargo build` of the worker in worktree A creates artifacts without a candidate root record. Worktree B can then run `prebuild-worker` on that shared target directory. Cargo can reuse A's artifacts, while prebuild writes B into the record. `Candidate::locate` accepts that record and verifies hashes of the copied artifacts. Those hashes do not establish the artifacts' source root.

The package reviewer independently reported this path in `msg_plugin-w_1791761124_0f04ed`. The supplied evidence covers an existing A record. It does not cover the first prebuild after a plain A build.

Required change: establish artifact provenance before accepting or writing the current root. A missing or unreadable record must not permit a stale artifact to receive a new root. Preserve the lead's rule against an automatic clean.

Required evidence: build the worker in A without prebuild, then run prebuild in B with the same target directory and lockfile. Show refusal or a verified B build. Show that the repaired candidate check cannot accept A's worker as B's. Keep the valid first-build path covered.

### B13-I2 — HIGH — OPEN: coverage accepts a function that no test tier runs

Evidence: `xtask/src/built_in.rs:98–103` accepts a root when it contains `TEST_FN` and `TEST_CALL`. It does not check `#[test]`, `#[ignore]`, or whether the function is commented out.

Removing `#[test]` from an existing root-check function leaves both substrings present. Commenting out that function also leaves both substrings present. The coverage command therefore reports a build-root test even though the test runner selects no such test. This breaks the check's purpose across the workspace crates.

Required change: validate that the named function is a registered test selected by the required tier. Use the established closed-form rules for source checks. Reject unsupported forms instead of accepting them by substring.

Required evidence: negative fixtures for a removed `#[test]`, a commented-out function, and a test excluded from the required tier. Preserve the current valid library and binary forms. The coverage check must reject each missing proof.

### B13-I3 — LOW — OPEN: the package evidence finding remains open

The package verdict `553fe5df48d74af1abc3103afad8e39cbae43e21`, file `verdicts/gs-gate-speed.md`, reports `GS-B13-R1-3` on the same head.

The evidence script counts `Compiling botster-core` lines but labels the result as all workspace crates. That pattern omits `botster-worker-core`, `botster-test-process`, and `botster-terminal-ghostty`.

Required closure: narrow the evidence label and PR body claim to the measured package names, or supply a complete compilation record. This entry tracks the package prerequisite. The observed conformance refusal remains valid.

### Evidence and limits

- Full gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-2c0488b8-pool-20261011-155212-66916.log`.
- The header names the exact reviewed head and base. All ten steps PASS. The gate exits 0.
- Mutation result: 44 total, 41 caught, 0 missed, 0 timeout, 3 unviable. No stage-1 miss requires stage 2.
- B13 evidence: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-2c0488b8-pool-20261011-161823-85653.log`.
- Before the fix, B reuses A's conformance build and exits 0. The evidence reports zero compilations matching `Compiling botster-core`. B's own build fails the newly un-pended `a11_1` id and exits 101.
- At the reviewed head, the reused conformance binary refuses with exit 101. Prebuild in B refuses an existing A root record with exit 1.
- The gate proves the implemented paths. It does not close B13-I1 or B13-I2.
- Package verdict: `553fe5df48d74af1abc3103afad8e39cbae43e21`, `verdicts/gs-gate-speed.md`. It reports NOT CLEAN with the two matching HIGH findings and the LOW evidence finding above. Sol-B read that verdict.
- The PR head does not contain the current live `v1` tip. The final-slot merge and the required base check remain due.
- Sol-B ran no local build, test, mutation job, or gate.

This supplement records the package verdict on the same head. It corrects the earlier compilation-count description. It starts no new source-review round.

VERDICT: NOT CLEAN (3 open)

## Round 2

Exact head: `d4758482eb0aaabe6a6053573e797d81343cddfb`. Imported base: `3aacc58d086e55a6b186165ccf9001e2618e2869`. Gate comparison base: `f7eeced64e652700f2f78fc063e4896832134233`. These bases differ.

Sol-B reviewed the coverage and candidate-build repairs, the mutation exclusion, the overlapping testkit paths, the named evidence, and the complete updated PR body.

### B13-I1 — CLOSED

`candidate_build_dir` derives a directory from the canonical runtime root. Both the worker and anchor build there through an explicit `--target-dir`. A plain build in another worktree uses different artifacts. Prebuild writes the root record after all binary installations. It performs no automatic clean.

Evidence: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-58544635-pool-20261011-163216-95056.log`.

The job builds the worker in A without prebuild. B then uses the same target directory. On the old head, prebuild exits 0 and labels the candidate B, but the worker contains A's path twice and B's path zero times. On the repaired head, the worker compiles from B. Its binary contains B's path twice and A's path zero times. This establishes the bootstrap repair and the valid first candidate build.

The later merge and final repair retain this candidate-build implementation. The lead approved this design subject to the Mac slot and disk checks.

The Mac jobs `...-2c0488b8-pool-20261011-162513-28603.log` and `...-58544635-pool-20261011-164100-59441.log` use slot-2. The second log places the target inside the slot, retains the same source root, and reports prebuild exit 0. The shared disk rule now includes each local candidate-build directory in the 8 GiB bound.

### B13-I2 — HIGH — OPEN: a path is still accepted as a call

The test index repair rejects missing test attributes, comments, ignored tests, excluded configurations, and tests below the root. It requires both default tiers. These parts close the original registration gap.

However, `root_test_calls` at `xtask/src/built_in.rs:147` visits every `syn::Path`. It does not require a call expression. This registered test satisfies the check without executing the build-root check:

```rust
#[test]
fn this_test_binary_was_built_in_this_worktree() {
    let _ = botster_test_process::built_in::assert_built_here;
}
```

The visitor also enters an uncalled nested function. A call inside that function satisfies the check while the root test performs no check. The current negative fixtures cover comments and another top-level function, but neither of these forms.

Required change: accept a supported executed call form in the root test. Reject unsupported forms. Add negative fixtures for a function reference and an uncalled nested function. A direct call in a supported valid root test must continue to pass.

Sol-B sent this finding to GS and 017c. No local execution was required to identify the visitor's behavior.

### B13-I3 — CLOSED

The retained evidence above counts `Compiling` lines by the worktree path, across all workspace package names. A compiles nine linked workspace crates. B on A's target compiles zero and gives the incorrect conformance pass on old v1. B with its own target compiles nine and fails the newly active case with exit 101. At the repaired head, B reuses the binary but the root check refuses it with exit 101.

The body replaces the earlier incomplete count with this complete record. It preserves the earlier evidence and states its limit.

### Base merge and gate

Merge `96c58d88cd5eb9a87d69eb12fff69596129a926f` has parents `585446350666dbca433c008a69a9cde4d76cbc55` and `3aacc58d086e55a6b186165ccf9001e2618e2869`. The canonical check reports FAIL for conditions 2 and 3 because the testkit manifest and library root overlap. This round supplies the required integration delta review.

Sol-B read both overlapping changes. The imported `rustix` net feature and `stream_holders` module remain. The PR retains the unconditional test-process dependency, build-root re-export, constant, and root test. The changes occupy separate lines. The inherited pending files equal the imported base. No conformance status change belongs to this PR.

The final repair changes only `xtask/src/built_in.rs` and `.cargo/mutants.toml`. It removes the unused slow-feature decision. The new exclusion covers only whole-body replacements of the worker build shell. Its reason cites the existing worker-presence decision and the new candidate-directory decision. The focused bootstrap evidence executes the real worker build path.

Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-d4758482-pool-20261011-175859-18261.log`. All ten steps PASS. Default: 1,835 passed. Slow: 597 passed. Mutants: 61 total, 58 caught, 0 missed, 0 timeout, 3 unviable. No second mutation stage is required. The job exits 0 after 516 seconds; the wrapper exits 0 after 518 seconds.

The lists step names comparison base `f7eeced6` and merge base `3aacc58d`. This gate does not establish import of the later base. The final-slot merge, required base review, and exact-head final gate remain due.

The same-head package verdict remains pending. Sol-B requested it from 017c. Sol-B ran no local build, test, gate, mutation job, or reversal.

VERDICT: NOT CLEAN (1 open)

## Source supplement — `836e879c`

Exact head: `836e879c40be7b0ac97b1bc8a2f4883eaffbd3ea`. Delta from `d4758482`: only `xtask/src/built_in.rs`. The gate is running. This supplement accepts no unfinished gate result.

The function now requires one statement containing a direct path call. The new fixtures reject a function reference, an uncalled nested function, an uncalled closure, an early return, a conditional call, and a method call. These forms close the path-visitor gap.

B13-I2 remains OPEN because the function ignores the call expression's attributes. The following registered root test has exactly one parsed call statement, but Rust removes that statement:

```rust
#[test]
fn this_test_binary_was_built_in_this_worktree() {
    #[cfg(any())]
    botster_test_process::built_in::assert_built_here("x", BUILT_IN);
}
```

Required change: reject statement attributes, or evaluate them for both required tiers. Add this negative fixture. Sol-B sent this source finding to GS and 017c. Sol-B ran no local build, test, or gate.

VERDICT: NOT CLEAN (1 open)

## Source supplement — `fec71f93`

Exact head: `fec71f93d39977cf71f7787a86b18e6e6cb2dfce`. Delta from `836e879c`: only `xtask/src/built_in.rs`. The repair rejects call attributes, callee attributes, and inner body attributes. The new negative fixtures cover the reported conditional statement. This closes the attribute example.

B13-I2 remains OPEN. Package reviewer 017c reported an unchecked argument in `msg_plugin-w_1791767647_5146ed`. Sol-B independently read `root_test_calls`, `assert_built_here`, and `check_here`. The matcher ignores `call.args`.

This sole call satisfies the matcher:

```rust
botster_test_process::built_in::assert_built_here(
    env!("CARGO_PKG_NAME"),
    &std::env::var("CARGO_MANIFEST_DIR").unwrap(),
);
```

The assertion forwards its second argument as the built root. Here it receives the runtime root, so the check compares that root with itself. A stale binary can pass. The matcher also accepts an unrelated callee that has the same final path segments.

Required change: require the supported compile-time argument forms and actual callee paths. Reject unsupported expressions. Add negative fixtures for a runtime-root argument and an unrelated callee with the same suffix. Preserve the current valid tests.

Sol-B sent the finding to GS and 017c. The completed gate and same-head package verdict remain due. Sol-B ran no local build, test, or gate.

VERDICT: NOT CLEAN (1 open)

## Package verdict and retained FLAKY records

Sol-B read package verdict `fa5cd808910647d8d646218ff75dea4fd7ba86d2`, `verdicts/gs-gate-speed.md`. It confirms the closures of B13-I1 and B13-I3. It keeps the matching argument finding open on `fec71f93d39977cf71f7787a86b18e6e6cb2dfce`.

### B13-I4 — MEDIUM — OPEN: earlier FLAKY outcomes need a disposition

The package verdict also identifies two stage-2 FLAKY outcomes. Sol-B read the actual records at lines 6345–6346 of `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-96c58d88-pool-20261011-173013-11653.log`.

Both records name `conf::in_8_mode_checked_at_write_start_by_worker`. Each test fails and then passes on its one retry. The mutants replace the worker build shell with `Ok(Some(("xyzzy".into(), String::new())))` or `Ok(Some(("xyzzy".into(), "xyzzy".into())))`. Neither result counts as a catch.

Required closure: supply the retained failure records and the structural-fix disposition under BUILD.md Testing rule 9. A later green gate and the new shell exclusion do not establish that disposition. An existing accepted disposition can close this evidence finding. Sol-B requested the records from GS and 017c. Sol-B requested no execution job.

The required disposition remains unknown. This entry does not attribute the flake to this PR's source or to an existing fence defect.

VERDICT: NOT CLEAN (2 open)

## B13-I4 disposition closure

B13-I4 is CLOSED. Sol-B read `/Users/jasonconigliari/botster-sessions/shared/core-stage1/evidence/gs-b13-in8-flaky/disposition.md` and the copied artifacts beside it.

The exact M1 and M2 logs are `target-mutants-stage2-mutants.out/log/xtask__src__prebuild.rs_line_95_col_5_003.log` and `_004.log` under `artifacts/jobq-botster-core-96c58d88-20261011173013-3033/`. Both first attempts fail with `INCONCLUSIVE`, seed 0, step 8, and `limit=step_timeout`. Both retries pass. Both logs retain FLAKY summaries. The historical mutants remain MISSED.

The plain-v1 log `/Users/jasonconigliari/botster-sessions/gates/botster-core-detached-5e953398-5e953398-pool-20261011-160013-7403.log` records the same id, step, and timeout symptom at seed 5. It contains no #293 source change.

Sol-B verified the candidate rejection path. Either replacement supplies no worker manifest entry if the normal prebuild runs it. Candidate verification rejects that missing entry. Mutation stage 2 instead uses the candidate built before mutation and cannot execute the changed prebuild shell. This explains the shell exclusion, but does not convert the flaky results to catches.

The lead's shared handoff assigns the open diagnosis and fence work to PC after contracts #77. The proposed mechanism is the R-74 held-write form. The id stays live and is not re-pended. This closes the required retained evidence and owned disposition. It claims no completed structural fix.

Package disposition receipt: `b09a6c627b50076348a09565ce6a338905db44e4`, `verdicts/gs-gate-speed.md`. Sol-B read that receipt.

The lead's SPLIT decision transfers the coverage guarantee to a new design-first item. Sol-B must verify removal of the visitor on the submitted split head before transferring B13-I2. The exact split head and its evidence remain due. Sol-B ran no execution job.

VERDICT: NOT CLEAN (1 open; split head pending)

## Round 3 — lead-approved split

Exact head: `652d0034a0a1da4145723b1b421c9b2c28de0239`. Imported base: `3aacc58d086e55a6b186165ccf9001e2618e2869`. Gate comparison base: `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035`.

Sol-B verified the split. It deletes `xtask/src/built_in.rs` and removes the command, help text, dispatch test entry, and taint call. The tier helper is private again. `xtask/src/ci.rs` and `xtask/src/mutants_cited.rs` equal the imported base.

B13-I2 transfers out of this PR under the lead's SPLIT decision, `msg_plugin-w_1791767795_78130a`. The coverage guarantee becomes a new item that requires a structural design note first. This review does not accept the removed visitor's defects or approve that future design.

The retained B13 implementation is unchanged from the reviewed repair. The suite changes only its coverage comment. Current crate root checks, the linked-library checks, candidate validation, and the per-root worker/anchor build directory remain. B13-I1, B13-I3, and B13-I4 remain closed. All conformance files retain the imported base. The earlier overlapping testkit delta review remains applicable.

Package CLEAN: `e71ed6cb96c1f8c3eb5d6d4188a33512f6f3483a`, `verdicts/gs-gate-speed.md`. Sol-B read that exact-head verdict and the complete revised PR body. Both state the retained scope and the transferred coverage item. The body preserves the earlier red results and the owned in_8 disposition.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-652d0034-pool-20261011-192108-45058.log`. All ten steps PASS. Default: 1,826 passed. Slow: 597 passed. Mutants: 15 total, 13 caught, 0 missed, 0 timeout, 2 unviable. Stage 2 runs no mutant. The log contains no FLAKY result. The job exits 0 after 452 seconds; the wrapper exits 0 after 453 seconds.

The lists step records merge base `3aacc58d`, distinct from the comparison base `b3cdfffb`. The final-slot current-base import, required check and applicable delta review, and exact-head final gate remain due. The original bootstrap and Mac evidence retain their recorded scope.

No integration finding remains in the retained scope. Sol-B ran no local build, test, gate, mutation job, or reversal.

VERDICT: CLEAN (0 open)

## Round 4 — final-slot source delta

Exact head: `029a740bc33bc7ddea76bb03a0dfdd3f4f584f2d`. Imported base: `e4f495040a80b524a7ac9accba95272d5c2594fa`. Sol-B verified the merge parents: reviewed head `652d0034` and this exact base.

Canonical report: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-029a740b-base-merge-check.log`. Both ancestry conditions, condition 1, and both pending-file conditions PASS. Conditions 2 and 3 FAIL on `.cargo/mutants.toml`, `Cargo.lock`, and `tests/suite/mod.rs`. This round supplies the required integration delta review. It does not change the canonical FAIL into PASS.

Sol-B read all three overlapping deltas. The mutation file retains the worker-build exclusion and imports held-full reasons and exclusions. The lockfile retains the PR's workspace dependencies while importing contracts-v0.1.41. The suite retains check_here(&LINKED) as its first operation, before arguments, list processing, and the imported held-full dispatch.

All 26 own patches match the reviewed patches after excluding blob-index and hunk-location metadata. All conformance files equal the imported base. The imported manifests add no workspace member. The sys manifest moves macOS libc from dev-dependencies into dependencies for start_time; this is more than a comment change, but creates no new linked workspace library. LINKED remains the reviewed binary plus nine libraries.

Package source CLEAN: `98ca787ffa7db58a06264734df6f5109d573c8b3`, `verdicts/gs-gate-speed.md`. Sol-B read that exact-head verdict. The removed visitor remains absent. B13-I1, B13-I3, and B13-I4 remain closed; B13-I2 remains transferred to the separate design item.

No source finding remains. The original bootstrap, compilation-count, Mac, and disk evidence retains its earlier-head scope. The completed exact-head final gate and updated final PR body remain due. Sol-B read no running gate result and ran no execution job.

VERDICT: CLEAN (0 open; source delta only, final evidence pending)
