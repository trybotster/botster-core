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

### Evidence and limits

- Full gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-2c0488b8-pool-20261011-155212-66916.log`.
- The header names the exact reviewed head and base. All ten steps PASS. The gate exits 0.
- Mutation result: 44 total, 41 caught, 0 missed, 0 timeout, 3 unviable. No stage-1 miss requires stage 2.
- B13 evidence: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-2c0488b8-pool-20261011-161823-85653.log`.
- Before the fix, B reuses A's conformance build with zero workspace compilations and exits 0. B's own build fails the newly un-pended `a11_1` id and exits 101.
- At the reviewed head, the reused conformance binary refuses with exit 101. Prebuild in B refuses an existing A root record with exit 1.
- The gate proves the implemented paths. It does not close B13-I1 or B13-I2.
- The package reviewer has not yet supplied a formal verdict. Sol-B requested the exact-head verdict.
- The PR head does not contain the current live `v1` tip. The final-slot merge and the required base check remain due.
- Sol-B ran no local build, test, mutation job, or gate.

VERDICT: NOT CLEAN (2 open)
