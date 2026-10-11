# GS gate-speed review

## PR #247: optimize sha2

VERDICT: CLEAN

- Reviewed head: `14cde79e46da90b83079b15ce463d833364a1ef6`.
- Base: `f6778ad85c01266ac49e2ededd97487da689e920`.
- Risk tier: HIGH.
- Open findings: 0, including LOW.
- Review date: 2026-10-11.

The diff changes only `Cargo.toml`. It sets the sha2 dev-profile optimization level to 3. Debug assertions and overflow checks remain enabled.

The candidate verification code, test selection, timeout constants, mutation selection, and mutation verdict remain unchanged. No Core workspace crate receives an optimization override.

The faster hash does not remove a verification. Each harness still verifies the candidate binaries against the manifest. This PR changes no binary dependency or mutant build rule.

The prebuilt worker gap remains: mutation runs do not rebuild `target/candidate` for each mutant. This PR adds no exclusion or acceptance for that gap.

I found no path in this diff that passes a mutant or test that the current gate would reject. This conclusion covers this diff only.

The PR includes the required Prior art note. It names the existing #223 profile overrides and states that no old code is reused.

### Evidence

I inspected the Linux gate log for the exact reviewed head:

`~/botster-sessions/gates/botster-core-stage1-gs-sha2-opt-14cde79e-pool-20261011-010427-15764.log`

All ten gate steps report PASS. The gate exits 0. Mutation testing reports no Rust source change. This result is not mutant comparison evidence.

I compared the PASS names from the first slow-suite run in each evidence log:

- `~/botster-sessions/gates/botster-core-gs-sha2-before-7e8e07d6-evidence.log`
- `~/botster-sessions/gates/botster-core-gs-sha2-after-d1ccfe9b-evidence.log`

Both runs pass exactly the same 412 test names. The slow-suite wall time changes from 109.0 seconds to 29.3 seconds.

These evidence runs use the earlier base `7e8e07d6` and candidate head `d1ccfe9b`. Their Cargo.toml change equals the reviewed diff. The final gate covers the exact reviewed head.

I ran no gate, build, or test. I inspected the source, PR description, and retained logs.

This CLEAN applies only to PR #247 at the reviewed head. It does not cover the candidate cache, in-place mutation run, or baseline reuse changes.

## PR #248: verify candidates once per test process

VERDICT: CLEAN

- Reviewed head: `41002679c4a9c24bb16b75c7735bc950bdc6f753`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Risk tier: HIGH.
- Open findings: 0, including LOW.
- Review date: 2026-10-11.

The diff changes only `tests/slow_conformance.rs`. A process-local `OnceLock` stores the candidate verification result. Each harness receives a clone of the verified paths.

The first harness still verifies all three candidate binaries against the manifest. A failed verification remains an error. The cache does not turn an error into a candidate.

The current gate builds the candidates before running the tests. The harness writes its wrappers and data into its temporary root. It does not rebuild the candidate binaries between harnesses.

The cache applies only within a test process. It does not share a result between mutant runs. A new process verifies the candidates again.

The wrapped and plain harness trials still run. Their seeds, assertions, selection, and verdict remain unchanged. This PR changes no timeout or mutation exclusion.

The prebuilt worker gap remains unchanged. The cache does not make an unmutated worker binary represent a worker mutant.

I found no path in this diff that passes a mutant or test that the current gate would reject. This conclusion covers the current gate and harness lifecycle.

The PR uses the standard library's `OnceLock` and reuses no old code. Its corrected Prior art note satisfies the review requirement.

### GS-B-SI-1: incorrect Prior art statement

Severity: LOW. Status: CLOSED. The integration reviewer reported this finding after the initial package CLEAN.

The initial PR description stated that old Core had no per-harness candidate verification. That statement was incorrect at pinned commit `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.

`crates/botster-core-daemon/tests/core_harness_test.rs` calls `WorkerBinary::from_env` from `WorkerHarness::new`. The verifier is in `crates/botster-core-test-support/src/real_worker.rs`.

The verifier checks the manifest's source revision, worker size, and SHA-256. Those paths contain no process cache to reuse.

Required change: correct the PR description. State that old Core had per-harness worker provenance verification. State that this PR reuses current `Candidate::locate` and `std::sync::OnceLock`.

No source change is required. I independently verified the finding in the pinned source. My initial review missed this documentation error.

I read the corrected PR description. It names `WorkerHarness::new`, the old verifier, and the current components reused by this PR.

The reviewed head remains unchanged. GS-B-SI-1 is closed, including its exact identifier correction.

### Evidence

I inspected the full-gate result on `b4072c19135fde605de58e27b02c5cf0fe562a3f`:

`~/botster-sessions/gates/botster-core-stage1-gs-verify-once-b4072c19-pool-20261011-011150-36320.log`

All ten steps report PASS. The gate exits 0. Mutation testing reports no Rust source change because this diff changes test code only.

The reviewed head merges v1 after that gate. The PR contains a passing base-merge-check report. I independently confirmed that both heads have byte-identical own diffs.

I also read the retained base-merge-check log:

`~/botster-sessions/gates/botster-core-gs-verify-once-base-merge-check-b4072c19-41002679.log`

I compared the slow-suite PASS names from the #247 gate with the B evidence log:

`~/botster-sessions/gates/botster-core-gs-verify-once-b4072c19-evidence.log`

Both runs pass exactly the same 427 test names. The reported conformance test-time sum changes from 28.1 seconds to 19.2 seconds.

The evidence appends one byte to the candidate worker. Both selected tests fail with the candidate hash mismatch message. The evidence then rebuilds the candidates successfully.

Those corruption probes start separate test processes. They prove first-verification rejection. They do not prove repeated verification within one process.

I ran no gate, build, or test. The final gate on the exact head remains required before merge.

Package CLEAN is restored after GS-B-SI-1 closes. The runtime assessment above remains unchanged. The final exact-head gate remains required before merge.

## PR #251: reject stale mutation exclusions

VERDICT: CLEAN

- Reviewed head: `7b942de5726df1e1bc17d178a65740d661d78461`.
- Base: `5c480db457797fa7ebecb33835a52e26a5a8490d`.
- Risk tier: HIGH.
- Open findings: 0, including LOW.
- Review date: 2026-10-11.

The new `mutants-stale` check rejects each exclusion that matches no listed mutant. It checks the configured regex entries and the explicit off-macOS entries.

The command lists the workspace mutants without configuration. Listing, JSON parsing, invalid regexes, and Git failures propagate as errors. The taint step calls the check before mutation testing.

The new shell exclusion covers only the whole-body replacement of `command`. Its reason cites `stale` and its running proof. No exclusion covers the new decision functions.

The two READ_CHUNK exclusions move to their current constant locations. Their reasons still cover only positive read-size tuning. Their existing proofs test retained bytes and frames at 65536, 1088, and 1.

The Linux guard exclusion moves from `live_members` to `live_where`. Commit `9873f33d` moved the same stat read and error guard there. The error decision remains mutation-tested.

The removed macOS entry matches a deleted conjunction. Commit `996a17f0` split that conjunction into separate zombie and selection checks. Removing its obsolete entry suppresses no current mutant.

The corrected entries restore documented exclusions. They do not exclude a new behavior decision. I checked their reasons, source locations, and cited proofs.

The new check adds a rejection condition. It changes no test selection, timeout, mutant build, candidate binary dependency, or two-stage verdict.

I found no new acceptance for a behavior-changing mutant in this diff. The check does not detect a regex that moves onto a different mutant. The PR states that limit.

The Prior art note identifies the reused gate-decision parsers and regex library. The PR reports the agreed file split with P6.

### Evidence

The exact-head Linux gate log is:

`~/botster-sessions/gates/botster-core-stage1-gs-mutants-stale-7b942de5-pool-20261011-011635-60299.log`

All ten steps report PASS. The gate exits 0. The new check compares 207 exclusions with 5991 workspace mutants.

Stage 1 catches all six in-diff mutants. Stage 2 runs no mutants because stage 1 misses none. The counts report no timeout or unviable mutant.

The log shows the new stale-entry proof passing. It also shows the read-bound proofs and the Linux error-decision proof passing.

The PR reports a red-on-revert check that restores the old worker constant line. That check rejects the stale entry. I did not run that check.

I ran no gate, build, or test. The final gate remains required under the GS brief after both CLEAN verdicts.

This CLEAN covers only PR #251 at the reviewed head. The in-place mutation run and baseline reuse changes remain unreviewed formally.

## PR #252: run mutation stage 1 in place

VERDICT: CLEAN

- Reviewed head: `afc33981b70834589f93c37d25581c731efda30b`.
- Base: `554d43e890b391cefc9da205c71db6856accb21d`.
- Risk tier: HIGH.
- Open findings: 0, including LOW.
- Review date: 2026-10-11.

Stage 1 adds `--in-place`. The shared options remove `--jobs 1`, which conflicts with that option. No selection, profile, timeout rule, or verdict code changes.

I read the installed cargo-mutants 27.1.0 source. Its default worker count is one. Its in-place baseline uses the workspace directory, and its baseline still sets the test timeout.

The existing stage-2 build and candidate binary rules remain unchanged. The prebuilt worker gap remains. The gate uses a disposable source snapshot.

I found no runtime regression in the reviewed diff. The corrected safety statement closes the documentation finding.

### GS-L1-R1-1: the baseline does not prove every repository-dependent result

Severity: LOW. Status: CLOSED.

The initial PR Safety section said that a repository-dependent difference fails the baseline and cannot pass a mutant. The initial `stage1_run_args` comment made the same claim.

A test can pass unmutated in both modes and behave differently only under a mutant. A passing baseline does not exclude that case.

Required change: correct the PR body and source comment. State that the baseline detects unmutated incompatibility only. Use the current source review and per-mutant comparison as evidence.

This finding identifies an incorrect safety argument. It does not claim an observed mutant regression in this repository.

Commit `afd827eb` corrects only that source comment. The PR body now states the same limit. I read both corrections.

The current source review supports the change: the real-tree walker skips `.git` and `target`. The inspected Git tests use temporary repositories.

The fixture mutation tests also copy their inputs into temporary directories. The correction changes no executable code. GS-L1-R1-1 is closed.

### Evidence

The Linux gate on the earlier head `c4d9c376` passes all ten steps and exits 0:

`~/botster-sessions/gates/botster-core-stage1-gs-gate-speed-c4d9c376-pool-20261011-013325-91259.log`

Stage 1 catches all three in-diff mutants. It reports no miss, timeout, or unviable mutant. Stage 2 does not run because stage 1 misses none.

I independently compared outcome lines in the retained copy-versus-in-place log:

`~/botster-sessions/gates/botster-core-gs-l1-copy-vs-inplace-5a41a2c1-evidence.log`

Both runs use the same evidence head. All 119 outcome lines match by full identity and multiplicity after removing timing suffixes.

The lines contain the baseline and 118 mutants: 113 caught, 2 missed, and 3 unviable. Neither run reports a timeout.

The job's `SAME-OUTCOMES` line compares empty files because Python is absent. I do not use that line as evidence. My comparison uses the retained mutant lines.

The evidence tests stage 1 only. It does not claim an old-versus-new stage-2 comparison or resolve the two missed mutants of the evidence head.

The PR includes a Prior art note. I ran no gate, build, or test.

The previous reviewed head `afd827eb` contains only the comment correction above that gated head. The final gate on the exact reviewed head remains required before merge.

### Delta review after the base merge

The new reviewed head `afc33981` merges base `554d43e8` into the previous CLEAN head `afd827eb`.

I read the retained base-merge-check log:

`~/botster-sessions/gates/botster-core-gs-gate-speed-base-merge-check-afd827eb-new.log`

The report fails the path-overlap and byte-identity checks. The base also changed `xtask/src/ci.rs`. This merge therefore requires a delta review.

I compared both own diffs. Every diff line is identical after removing the Git index line and hunk headers. No PR change was added or removed.

The base changes in `ci.rs` add the stale-exclusion import and check. They do not change the mutation options or stage arguments. The merged file keeps both changes.

No new finding is open. CLEAN now applies to `afc33981b70834589f93c37d25581c731efda30b`. The final gate on that exact head remains required.

## PR #253: reuse the slow pass as the stage-2 baseline

VERDICT: NOT CLEAN (withdrawn)

- Reviewed head: `6105272829458d691761af3b0ec1b380f80b9413`.
- Base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
- Risk tier: HIGH.
- Finding: GS-L2-R1-1, HIGH, unresolved in the withdrawn code.
- Disposition: CLOSED without merge after the lead agreed to withdraw L2.
- Review date: 2026-10-11.

I verified that GitHub reports PR #253 as CLOSED with no merge. This change received no package CLEAN.

### GS-L2-R1-1: file contents do not prove execution identity

The record contains each candidate path and SHA-256. It contains no Unix permission mode. The same omission applies to selected test binaries.

A worker that changes from mode 0755 to 0644 retains its path and hash. The reuse check therefore accepts the unchanged recorded identity.

The candidate verifier can still read and hash that worker. Execution then fails. The old unmutated stage-2 baseline would reject the execution failure.

Reuse can skip that rejection and allow the execution failure to count as a mutant catch. That weakens the gate.

I proved the missing identity field with a temporary executable. It first exited 0. After removing execute permission, its path and hash remained equal, but execution raised PermissionError 13.

Required correction, if this design returns: record execution-relevant metadata, reject unsupported file forms, and prove refusal after a permission change.

The implementer accepted this finding. The lead then withdrew the design, so this PR requires no repair. Withdrawal does not prove the design safe.

### Evidence and limits

The exact-head gate is green:

`~/botster-sessions/gates/botster-core-stage1-gs-baseline-reuse-61052728-pool-20261011-020504-96712.log`

Stage 1 reports 48 mutants: 41 caught and 7 missed. Stage 2 reuses the baseline and catches all 7 misses.

I independently compared the two stage-2 runs in:

`~/botster-sessions/gates/botster-core-gs-l2-old-vs-new-61052728-evidence.log`

Both runs report the same 13 full mutant identities and outcomes: 11 caught and 2 unviable. The comparison includes multiplicity. Neither run reports a miss or timeout.

This sample does not prove safety after an artifact permission change. It also does not prove equal behavior across nextest profiles or machine states.

The closing comment cites those broader limits and a slow-test failure in another gate. I record that report as the withdrawal reason, not as my independent reproduction.

The two new shell exclusions in this PR are withdrawn with the design. I approve neither exclusion through this review.

I ran no gate or build. I ran only the temporary executable check described above. I will wait for the next review request.
