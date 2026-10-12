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

## PR #256: mutation retry and catch report, round 1

- Head: `a1b009a1b31412f5197d87086eb7b75ad336d8ca`.
- Base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
- Risk tier: HIGH.
- VERDICT: NOT CLEAN (2 open).

BUILD.md Testing rule 9 at contracts main `e1da7be` permits one retry in the mutants profile. Every other profile keeps zero retries.

### GS-FG-R1-1 HIGH: unknown status forms can bypass the parser

Status: OPEN.

Evidence: `xtask/src/mutant_logs.rs`, `status_line` and `read_run`, lines 112 to 137 at the reviewed head.

The regular expression accepts only known status characters and the known time envelope. `read_run` ignores every line that does not match.

These uppercase status lines do not match:

```text
TRY 1 FAIL! [ 0.100s] w t::a
FLAKY 2/2 [ 0.100 s] w t::a
UNKNOWN_STATUS [ 0.100s] w t::a
```

Thus `state_of` cannot reject these unknown forms. A baseline log with the first two lines yields no retry flag.

`report` accepts that log for a baseline outcome with summary `Success`. The old baseline could fail on the first test failure.

This example represents an unknown output form. I do not claim that the current nextest version emits these lines.

The PR explicitly requires an unknown status form to fail the step. The parser does not meet that requirement.

Required change: identify an uppercase status-line candidate separately from parsing its known status and envelope. Reject an unknown status or envelope.

Keep mixed-case summaries, lowercase output, and ordinary output without a status line ignored. Add baseline and mutant report tests for the rejected forms.

I checked the regular expression with Python. All three lines were ignored. The known `TRY 1 FAIL [ 0.100s] w t::a` line matched.

### GS-FG-R1-2 LOW: the safety claim exceeds the retry guarantee

Status: OPEN.

Evidence: `.config/nextest.toml`, the `mutant_logs` module documentation, and the PR description.

The source states that a flake is never a catch, or can no longer count as a catch. One retry cannot establish that guarantee.

A flaky test can fail both attempts. Nextest then fails the run, and cargo-mutants can report CAUGHT.

The test `a_retried_failure_fails_and_a_fail_then_pass_is_flaky` correctly preserves a failure after two failed attempts.

Required change: state the exact guarantee. A fail followed by a pass does not count as a catch. Two failed attempts can count as a catch.

Apply this limit to the source comments and the PR safety argument. The ruling permits the retry; this finding requests an accurate claim.

### Reviewed behavior and evidence

The diff changes six files. It adds no mutation exclusion and changes no timeout value.

Both stages retain their full mutant identity checks and count checks. Stage 2 retains the full slow filter and workspace build.

The source adds the log report after each stage's summary. A report error fails the step before its verdict can pass.

The mutants profile sets `retries = 1`. The standalone stage-2 baseline passes `--retries 0`.

The report rejects recognized retry lines in cargo-mutants' own baseline. GS-FG-R1-1 covers unrecognized forms.

The real-tool fixture checks four fail-then-pass mutants as MISSED with FLAKY report lines. It checks four persistent failures as CAUGHT with test names.

The existing two-stage fixture now checks the stage-2 catching test names. The retained exact-head gate reports both fixture tests PASS.

Gate evidence:

`~/botster-sessions/gates/botster-core-stage1-gs-flake-guard-a1b009a1-pool-20261011-031034-60517.log`

The log names the exact head and base. All ten steps report PASS. The mutation step reports 37 mutants: 34 caught and 3 unviable.

The log prints 34 catch lines. Stage 2 does not run for this PR's own mutants, because stage 1 misses none.

I checked the Prior art statement with Git at the pinned old commit. Its matching names are unrelated to a mutation gate.

This change does not rebuild `target/candidate` for each mutant. The existing worker binary limitation remains outside this PR's scope.

I ran no gate or build. I read code and retained evidence. I ran only the small regular-expression check described above.

## PR #256: round 1 fixes, delta review

- Head: `db4fd197b40788a56e57eec47c97b5f005359a80`.
- Base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
- Previous reviewed head: `a1b009a1b31412f5197d87086eb7b75ad336d8ca`.
- VERDICT: CLEAN.
- Open findings: 0.

GS-FG-R1-1 HIGH is CLOSED. The parser now identifies a status-line candidate before it parses the known envelope.

A candidate starts with an uppercase letter after its indent. It has no lowercase letter before the first `[`.

Each candidate must match the known envelope and status. Otherwise, the parser returns an error. The three reported unknown forms now produce errors.

The new tests cover malformed times and missing fields. Report tests cover an unknown retry form in a baseline and an unknown mutant status.

Mixed-case summary and information lines remain ignored. Lowercase test output and lines without `[` remain ignored.

The rule can reject unrelated uppercase output that resembles a status line. That conservative error removes an acceptance and does not add a mutant catch.

GS-FG-R1-2 LOW is CLOSED. The configuration comment, module documentation, and PR body state the limit of one retry.

A fail followed by a pass is no catch. A flaky test that fails both attempts can still count as a catch.

The delta changes only `.config/nextest.toml` comments and `xtask/src/mutant_logs.rs`. No selection, timeout, binary build, or verdict rule changes in this delta.

I verified the exact full head in Git and the PR. I read the delta and new regression tests. I ran no gate, build, or test.

The implementer reports that the gate on this head is running. The final exact-head gate and the integration review remain required before merge.

### Final gate failure at db4fd197

The retained gate log is:

`~/botster-sessions/gates/botster-core-stage1-gs-flake-guard-db4fd197-pool-20261011-031855-22867.log`

I verified that the slow step fails `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded`, seed 0, step 21.

The reported error is `PendingLimit`, with detail `pending_ops is 4`. The slow suite reports 464 passed and one failed.

The mutation step reports NOT RUN because an earlier step failed. This log does not satisfy the merge gate requirement.

The package source verdict remains CLEAN on the same exact head. That verdict does not approve the failed gate.

The implementer reports that #240 removes the known cause and is absent from the base. I have not confirmed that cause for this occurrence.

Plan revision 23al permits one gate retry under specific conditions: trace diagnosis, plain-v1 reproduction, reviewer cause confirmation, and written lead approval.

I have no plain-v1 comparison, cause confirmation, or written lead approval for this occurrence. I asked the lead to resolve the gate acceptance condition.

The implementer started another gate on the same head. The implementer now states that this run is diagnostic and will not serve as merge evidence.

I ran no gate or build. I will wait for the lead's decision or a review request with the required evidence.

### Lead decision after the failed gate

The lead confirms that no revision 23al approval exists for `db4fd197`. The lead gives no such approval.

`db4fd197` will not be the merge head. The lead requires GS to merge v1 after PR #255 merges.

GS must post the base-merge-check result and run one full gate on the exact new head. The failed log stays in the PR body.

The package CLEAN carries over only if the base-merge-check passes. A failed check requires a delta review.

If the new head fails EV-5 again, the head waits. The lead requires my written cause confirmation before any revision 23al decision.

I will review the next head or the retained base-merge-check evidence when requested. I run no gates.

The integration reviewer reports source CLEAN on `db4fd197` in verdict commit `81b130c9`, with zero open findings.

The integration reviewer keeps gate acceptance separate from source CLEAN. The integration reviewer ran no execution job.

### Requested cause confirmation

The lead adds a condition: the new gate counts only after attribution of the failed `db4fd197` gate.

I confirm: **#256's change does not alter the ev_5 cause.**

I compared `5ac94fab..db4fd197` with the slow step's profile and EV-5's path. The changed retry setting applies only to the mutants profile.

`test_budget` selects the unchanged slow profile for the failed step. The EV-5 transcript pin remains `contracts-v0.1.29`.

The worker descriptor path, harness, pending-operation recovery, and `poll_all` behavior are unchanged. The new mutation log reader does not run on EV-5's path.

I read R-63 and the lead's retained classification from PC's traces. The unfenced recovery can poll before the required completions arrive.

The new fixture tests can change scheduling. They do not repair that recovery or change its cause.

Limit: this confirms the change's relation to the diagnosed cause. I did not independently reproduce this occurrence or equate its error text with another trace.

This confirmation does not approve the second gate at `db4fd197`. The lead's required new head, base-merge-check, and full gate still apply.

## PR #256: verified base merge

- Head: `0e222c592d17995380ec6f20449f91d71207d2fc`.
- Base: `c8907b74bd247002784843e1a12f6487b08c579f`.
- Reviewed parent: `db4fd197b40788a56e57eec47c97b5f005359a80`.
- VERDICT: CLEAN (carried over through the verified base merge).
- Open findings: 0.

I read the retained base-merge-check log:

`~/botster-sessions/gates/botster-core-gs-flake-guard-base-merge-check-db4fd197-0e222c59.log`

The check reports PASS for ancestry, conflict absence, disjoint paths, identical own diff, and unchanged pending lists.

I checked the merge parents and base paths with Git. The four changed base paths do not intersect the six PR paths.

The retained script reports 46885 bytes and SHA-256 `c74d91f086dfc4fe14fda2c53f6fe68391565b31fd296ac0b1e6a38bdacf6ac2` for its diff format.

I independently compared both complete own diffs with the same plain `git diff` command. They are byte-identical: 46501 bytes.

My comparison's SHA-256 is `1eb4cf9ed0766a6e957dc6af5844f7ce1184de7b8c8e54cdc498174952f99d9a`. I do not equate that format with the script's format.

The lead's carry-over condition is satisfied. The source CLEAN applies to this exact head. The failed earlier gate remains on the record.

The implementer reports that the one full gate on this head is running. Gate acceptance remains pending. I ran no gate or build.

### Final exact-head gate evidence

I read the retained gate log for `0e222c592d17995380ec6f20449f91d71207d2fc`:

`~/botster-sessions/gates/botster-core-stage1-gs-flake-guard-0e222c59-pool-20261011-033134-11371.log`

The log names base `c8907b74bd247002784843e1a12f6487b08c579f`. All ten steps report PASS. The total is 155.9 seconds.

The mutation step reports 39 mutants: 35 caught, zero missed, zero timeout, and four unviable. I counted 35 catching-test report lines.

Stage 2 does not run because stage 1 misses none. The real-tool flake fixture reports PASS in the slow step.

This log satisfies the exact-head full gate condition for the recorded base. Source CLEAN remains unchanged, with zero open findings.

The failed `db4fd197` gate remains on the record. This green gate runs on the new committed head required by the lead.

The lead owns the merge decision and current-base check. I ran no gate or build.

The integration reviewer reports completion CLEAN on the same exact head and base in commit `3204d4a5dc9615fb0783e38149c4c0e6a765f17e`.

The integration reviewer independently checked that every caught mutant identity has exactly one catching-test report. The integration reviewer ran no execution job.

No merge receipt has been reviewed. The lead still owns the current-base check.

## PR #260: slow-only test check, round 1

- Head: `f41e42e016d1dfa3a7015f2c8473b743baf04e9a`.
- Base: `d86bfcc747718452efe57b2aa207492acde96573`.
- Risk tier: HIGH, with a worker test rename and a mutation exclusion.
- VERDICT: NOT CLEAN (1 open).

### GS-SC-R1-1 LOW: P6 file agreement is not recorded

Status: OPEN.

The GS brief requires agreement with P6 on the file split before the first commit. The PR adds a call in P6's `slow_job` area.

The PR body states that P6's confirmation is pending. The lead handoff also records unanswered requests. I asked P6 directly for confirmation.

Required closure: record P6's agreement for this line, or the lead's explicit decision that permits this file change without P6's agreement.

This finding concerns the required coordination record. The source review found no gate-safety defect at this head.

### Source review

`tier_selection` contains the previous selection code. The default workspace selection and the slow packages, features, and filter remain unchanged.

The new check lists both selections. It compares each test by binary id and full test name. It adds a rejection and removes no existing test check.

The list parser rejects missing suites, unlisted binaries, missing testcases, missing ignored flags, and unknown filter statuses. An empty list fails.

No slow-only tests also fails. The closed-kind rule matches the lead's recorded Q2 decision.

Only ignored trials of `botster-core::slow_conformance` with the exact accepted form pass. A pending-real trial also requires its full id in the pending-real file.

Other ignored kinds, binaries, plain ignored tests, and malformed names fail. The check does not duplicate the conformance harness's per-kind list rules.

The list command removes `NEXTEST_PROFILE`. No current profile defines a default filter, so this removal does not narrow either selection.

The command uses Cargo metadata's target directory. The real-tool fixture uses its own temporary target directory to avoid reuse across copied fixtures.

The worker rename changes both the test function and `DRIVER_OBSERVER`. The child commands use that constant with `--exact`, so they still reach the same Driver.

Without `BOTSTER_DRIVER_CONTROL`, the selected observer returns immediately. With that variable, the observer retains its previous behavior.

The new command exclusion covers only the whole-body `Ok(())` replacement. The command reads metadata and pending ids, then forwards to `check` and `verdict`.

The cited decisions remain outside that exclusion. The real-tool fixture checks rejected and accepted selections and the pending-real membership condition.

The existing `slow_job` exclusion now documents the new call. This PR changes no timeout or mutation selection rule.

The check does not scope a mutant's slow tests or replace candidate binaries. The existing candidate build limitation remains outside this change.

### Evidence

I read the first-rule evidence log. It reports 1009 slow-only tests and 544 violations, including the unselected `driver_observer`.

The other reported violations are ignored conformance trials. The lead approved the closed-kind rule that now accepts those trials.

Exact-head gate:

`~/botster-sessions/gates/botster-core-stage1-gs-slow-coverage-f41e42e0-pool-20261011-033934-73592.log`

The log names the full head and base. All ten steps PASS, total 576.0 seconds. The check reports 1012 slow-only tests with no violation.

`slow_driver_observer` and the real-tool fixture report PASS. The mutation step reports 40 mutants: 38 caught, zero missed, zero timeout, and two unviable.

Stage 1 catches 30 mutants. Stage 2 catches the remaining eight after its baseline passes. Each stage-2 catch names the real-tool fixture test.

I checked the Prior art statement at the pinned old commit. No `nextest` or `feature = "slow"` match exists there.

I ran no gate, build, or test. I will wait for the coordination evidence before closing the LOW finding.

## PR #260: coordination closure

- Head: `f41e42e016d1dfa3a7015f2c8473b743baf04e9a`.
- Base: `d86bfcc747718452efe57b2aa207492acde96573`.
- VERDICT: CLEAN.
- Open findings: 0.

GS-SC-R1-1 LOW is CLOSED. The lead explicitly permits the one `slow_coverage` call in `slow_job`, without P6's reply.

The lead identifies the check as GS's command. P6 may raise an objection through a follow-up PR.

This is the requested file-boundary receipt. No source commit or PR head changed during closure.

The lead reports that v1 is now `4ab8a985`. GS must merge the then-current tip after CLEAN, post the base-merge-check, and run one final gate.

This CLEAN applies only to the exact head above. Carry-over requires a passing base-merge-check; otherwise, the new head needs a delta review.

I ran no gate, build, or test. I will wait for the next head and its evidence.

## PR #260: verified base merge

- Head: `f291a5e6d3f31e3ad769f52e125647a292076d52`.
- Base: `cfd6ee364e2eafd329ce4e665100168918d383cb`.
- Reviewed parent: `f41e42e016d1dfa3a7015f2c8473b743baf04e9a`.
- VERDICT: CLEAN (carried over through the verified base merge).
- Open findings: 0.

The integration reviewer reports source CLEAN on the reviewed parent in commit `a57964bc340de4d4ff18845ab0a1ab08f6be9522`.

I read the retained base-merge-check log:

`~/botster-sessions/gates/botster-core-gs-slow-coverage-base-merge-check-f41e42e0-f291a5e6.log`

All checks PASS: ancestry, conflict absence, disjoint paths, identical own diff, and the pending lists. I verified the merge parents with Git.

The five changed base paths do not intersect the ten PR paths. The pending-list check confirms that the new head holds the base's files.

The script reports 30764 bytes and SHA-256 `c0d79f9e593f81d39dcf809da9bbe392aa97ae766a747a488a4677ff82bb02c3` for its diff format.

My independent plain `git diff` comparison also finds byte-identical own diffs: 30124 bytes, SHA-256 `22c330e994387f35fcbede87526f3e468c6029c0ac45ed9d7d0adabeba726dd6`.

The package CLEAN carries over to the exact new head. The implementer reports that its one final gate is running.

Gate acceptance remains pending. I ran no gate, build, or test.

### Final exact-head gate for PR #260

I read the retained gate log:

`~/botster-sessions/gates/botster-core-stage1-gs-slow-coverage-f291a5e6-pool-20261011-035822-73822.log`

The log names exact head `f291a5e6d3f31e3ad769f52e125647a292076d52` and base `cfd6ee364e2eafd329ce4e665100168918d383cb`.

All ten steps PASS, total 444.1 seconds. The check reports 1012 slow-only tests and no violation.

Stage 1 reports 40 mutants: 30 caught, eight missed, and two unviable. Stage 2's baseline passes, and stage 2 catches all eight misses.

The total is 38 caught, zero missed, zero timeout, and two unviable. I counted 38 catching-test report lines.

The integration reviewer reports source carry-over CLEAN in commit `90be012585482b8187af03cb652c77bb672c7ded`. Its independent raw diff matches my comparison.

The recorded exact-head gate condition is met. Package CLEAN remains unchanged with zero open findings. The lead owns the merge decision and current-base check.

I ran no gate, build, or test. I will wait for a merge receipt or a delta review request.

### PR #260 merge receipt

The integration reviewer reports completion CLEAN in published verdict commit `0246777091b6853e4f608bf0eaf64d508b314e2d`.

The integration reviewer checked all eight stage-2 identities and all 38 catch reports. The integration reviewer reports EV-5 as FLAKY, with another catching test.

I verified merge `b3cb268f3c2cb3223dadef02033d2bee49f06c59` with Git. Its parents are the exact recorded base and reviewed head.

Its tree is `2388ba831a7d807aa32cae2488c8547a802d9b01`, equal to the reviewed and gated head's tree.

The DNS failure delayed publication of package completion commit `995350d8`. A later push succeeded. That completion record is now published.

PR #260 is merged with the reviewed tree. I ran no gate, build, or test. I will wait for the next review request.

## PR #261: known-flaky list, round 1

- Head: `80af70d2ea876b062c55002e64f2e1eb107ad247`.
- Base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Risk tier: HIGH.
- VERDICT: NOT CLEAN (1 open).

### GS-KF-R1-1 HIGH: one declared tier does not cover a shared test

Status: OPEN.

Evidence: `known_flaky::parse`, `tests_of`, and `stage_filter`; `ci::mutants_job` and the stage selections.

`tests_of` applies an entry to only its declared tier. The parser rejects duplicate binary/test identities, even if their tiers differ.

A workspace library test binary can exist in both stages. A default-feature test named `slow_flaky` can run in stage 1 and match stage 2's slow filter.

For an entry with tier `default`, stage 1 excludes that test, but stage 2 can still run it. Tier `slow` leaves stage 1 able to run it.

The parser cannot represent both tiers for the same identity. Nextest's binary-id check cannot detect this gap because the binary exists in both stages.

A second case is a slow-feature-only test inside a shared library binary. Declaring it `default` does not produce the claimed missing-binary error.

The default binary exists even though that exact test does not. The slow stage then ignores the entry and runs the known-flaky test.

Thus a valid parsed entry can leave its known-flaky test able to cause a false mutant catch. This violates the guard's required fail-closed behavior.

The current EV-5 entry uses a slow-only integration binary. I do not claim that this particular entry leaks into stage 1.

Required change: validate exact test membership for each applicable stage, or support every stage in which the identity can run.

Exclude the known-flaky identity from every applicable mutation stage. Reject a declaration that leaves the identity runnable in another stage.

Add a real-tool fixture with one binary/test identity selected in both stages. Also cover a slow-only test inside a shared library binary.

Correct the universal claim that a wrong tier fails through a missing binary. The current fixture proves only the absent-binary case.

I read nextest-filtering's local primary source. It validates `binary_id` against binary ids; its test-name matcher does not establish tier membership.

### Reviewed behavior and limits

The TOML parser checks keys, field types, empty values, plain names, tier values, PR number form, and duplicate identities.

The filter subtracts exact binary/test pairs. Its parenthesized union preserves the existing slow filter. An empty list preserves the existing argument vectors.

The slow step and standalone stage-2 baseline remain unchanged. Mutant identity selection, per-name counts, and timeout values remain unchanged.

The list contains only EV-5 and names P3 and #243. The plan requires an empty list at Stage 1 exit.

This PR changes no candidate build path. The existing mutant worker binary limitation remains outside its scope.

The retained exact-head gate is:

`~/botster-sessions/gates/botster-core-stage1-gs-flaky-list-80af70d2-pool-20261011-041047-3410.log`

The log names the full head and base. All ten steps PASS, total 460.7 seconds. Stage 1 reports 31 mutants: 30 caught and one unviable.

Stage 2 does not run. This gate does not exercise the EV-5 exclusion in the real stage-2 filter.

The real-tool fixture proves exact exclusion in stage 1 and failure when the declared binary is absent. It does not cover a shared binary or both stages.

The Prior art note matches the previously checked pinned old source. I ran no gate, build, or test.

I will wait for a fix and the exact new head.

## PR #261: round 2 delta review

- Head: `b6b32f739bee9e5a5002b376452ee39a8245f55f`.
- Base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Previous reviewed head: `80af70d2ea876b062c55002e64f2e1eb107ad247`.
- VERDICT: NOT CLEAN (2 open).

### GS-KF-R1-1 HIGH: stage-1 list does not cover its package builds

Status: OPEN, with the original single-tier case corrected.

The delta removes the tier field and intersects entries with each preliminary test list. Shared identities now receive exclusions in both lists that contain them.

However, stage 1 lists `--workspace`. Cargo-mutants 27.1.0 tests only the mutated package by default. The gate does not set `--test-workspace true` in stage 1.

I verified that default in cargo-mutants `options.rs` and its per-mutant package selection in `lab.rs`.

Cargo can unify different features for a workspace build and a package-only build. The workspace list is not necessarily a superset of package-only tests.

Concrete case: library A has features `x` and `slow`. Another workspace member enables A's `x` feature.

A known-flaky test named `slow_flaky` has this guard:

```rust
#[cfg(any(feature = "slow", not(feature = "x")))]
```

The default workspace list enables `x`, so it lacks the test. The stage-1 package-only run of A can omit `x` and build the test.

The slow list enables `slow` and contains the identity. Thus `absent` accepts the entry, while the stage-1 filter omits its exclusion.

Stage 1 can still count that known-flaky test as a catch. This is an honest feature-selection case; it requires no malformed entry.

Required change: cover stage 1's actual package-specific feature selections, or use a filter/report rule that prevents this missing-list case from adding a catch.

Add a real multi-member fixture that exercises this feature-unification case. Do not silently broaden stage 1's test execution as a substitute.

The PR body still describes the removed tier field. Correct that safety argument before closure.

### GS-KF-R2-2 MEDIUM: the slow-only fixture test is not selected before exclusion

Status: OPEN. The integration reviewer identified this limit, and I confirmed it in the delta.

`known_flaky::slow_tests::each_stage_leaves_out_every_listed_test_that_it_builds` uses `tests::runs_only_under_slow` inside the shared library binary.

That test name does not match `SLOW_FILTER`, `binary(/^slow/) | test(/(^|::)slow_/)`. Its library binary id does not match the binary term either.

The real stage-2 run therefore omits this test before the known-flaky subtraction. Its absence cannot prove that the new filter excluded it.

The filter text assertion proves listed membership. It does not prove the required real-run exclusion of a selected slow-only test in a shared binary.

Required change: make that copied fixture test selected before exclusion, for example through a `slow_tests` module name.

Assert its pre-exclusion selection or execution. Then prove its absence under the exclusion, while an unrelated selected test still runs.

### Delta limits

The new code rejects the removed `tier` field. It checks stale identities during the slow step through `known_flaky::absent`.

Stage 2 lists the workspace with the same slow features as its build. The standalone baseline remains unchanged and still runs known-flaky tests.

The delta adds no timeout change or mutation exclusion. Its full gate has not yet been supplied.

I ran no gate, build, or test. I will wait for the next exact head and the required evidence.

## PR #261: round 3 delta review

- Head: `eb7f5c1264efe3346890d831af27c0cf3ba584ac`.
- Base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Previous reviewed head: `b6b32f739bee9e5a5002b376452ee39a8245f55f`.
- VERDICT: NOT CLEAN (1 open).

### GS-KF-R1-1 HIGH: cancellation can replace the listed first failure as catcher

Status: OPEN. The delta adds a verdict rule, but that rule can still accept a catch caused by a known-flaky test.

`mutant_logs::report` now searches all final failed tests for the first identity outside the known-flaky list.

The mutation stages pass `--max-fail 1:immediate`. A known-flaky test's final failure can therefore terminate other running tests.

The existing `CATCH` fixture and `the_first_final_failure_is_the_catching_test` explicitly show that order: first FAIL, then another test's SIGTERM.

If the first identity is listed, the new search skips it. It can credit the unlisted SIGTERM result as the catcher.

That termination is caused by the listed failure. It does not establish an independent mutant catch. The guard can still accept a false CAUGHT.

Required change: refuse the catch when the actual first final failure is listed, unless separate evidence proves an independent catch.

A conservative report error on a listed first failure is sufficient. Do not search later failures to replace that first failure.

Add a regression with a listed first FAIL and a later unlisted SIGTERM or ABORT caused by immediate cancellation.

The current new unit test uses two FAIL lines and credits the second. It does not cover cancellation from the first failure.

The multi-member fixture correctly establishes the workspace/package feature mismatch. Its report-error case has only the listed failure, so it misses this cancellation case.

### GS-KF-R2-2 MEDIUM: CLOSED

The copied fixture now renames `mod tests` to `mod slow_tests`. The slow filter selects its slow-only shared-binary test before exclusion.

The fixture asserts pre-exclusion execution and post-exclusion absence. It also asserts that an unrelated selected test still runs.

I read those source assertions. The implementer reports that the exact-head gate is running; I have not accepted execution evidence for this correction.

### Round limit and evidence

The PR body now describes the removed tier and the verdict rule. The slow step and standalone stage-2 baseline remain unchanged.

The delta adds no timeout change or mutation exclusion. I ran no gate, build, or test.

This is the third NOT CLEAN package round. I asked the lead to settle the first-failure rule or replan before round four, as section 8 requires.

### Lead settlement for round four

The lead settles the point without a replan. Only the first final failure under `--max-fail 1:immediate` can be the catch.

If that identity is known-flaky, the mutant is MISSED. No later failure in that cancelled run receives credit.

Round four covers this rule and two cases: listed first failure followed by cancellation gives MISSED; unlisted first failure gives a catch.

The lead directs other new findings to follow-up PRs unless they are HIGH. I will apply that scope to the next review.

## PR #261: round 4, first-failure settlement

- Head: `727a1c5043ad007f5b2d0a3353be657cf240e403`.
- Base: `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.
- Previous reviewed head: `eb7f5c1264efe3346890d831af27c0cf3ba584ac`.
- VERDICT: CLEAN.
- Open findings: 0.

GS-KF-R1-1 HIGH is CLOSED. I reviewed this delta within the lead's round-four scope.

`StageReport` now records each raw CAUGHT identity whose first final failure is known-flaky. It gives that identity a MISSED report line.

The report uses only `run.failed.first()`. It gives no catch credit to a later failure in that run.

`parse_stage_outcomes` moves the corresponding occurrence from caught to missed. It preserves the total and tested identities.

The code rejects an unmatched reclassification and a move count larger than the raw caught count. A report error still fails the step.

`stage_outcomes` changes exit code 0 to 2 when the adjusted result has misses. It preserves other exit codes and failures.

Stage 1 therefore sends the reclassified identity to stage 2. Stage 2 still fails on a remaining miss. The selection checks remain unchanged.

The first required fixture case contains listed FAIL followed by unlisted SIGTERM. The result has one fewer catch and one additional miss.

The second case contains unlisted FAIL first. That identity remains the catcher, even when a listed test is terminated later.

The decision test checks both stages: listed-first in both stages fails; listed-first in stage 1 and unlisted-first in stage 2 passes.

The report unit test also covers ABORT and later FAIL after the listed first failure. Both produce MISSED rather than an independent catch.

The real-tool tests now expect reclassified counts and missed names. The feature-unification fixture also checks the adjusted exit code.

GS-KF-R2-2 MEDIUM remains CLOSED from the previous delta. No other source finding is open.

The implementer reports that the exact-head gate is running. No full gate for this correction has been supplied or accepted yet.

Source CLEAN does not accept the superseded gates. The final exact-head gate and integration review remain required before merge.

I ran no gate, build, or test. I will wait for the gate receipt or a new head.

### Integration source review

Sol published integration CLEAN in `7245cd1c9df36c9193646fe30750220b3ffbfe8c` for the exact head `727a1c5043ad007f5b2d0a3353be657cf240e403`.

Sol confirmed that package verdict `4be62171` is published. No source finding remains within the lead's settled scope.

The exact-head full gate and execution evidence remain pending. Neither source verdict accepts a gate or permits merge.

### Final exact-head gate

I read the final gate log for `727a1c5043ad007f5b2d0a3353be657cf240e403`, with base `b3cb268f3c2cb3223dadef02033d2bee49f06c59`.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-flaky-list-727a1c50-pool-20261011-044118-44498.log`.

All ten steps PASS. The total is 446.3 seconds. The job and gate both exit 0.

Stage 1 reports 66 mutants: 46 caught, seven missed, zero timeout, and 13 unviable. Its unmutated baseline passes.

The stage-2 baseline passes in 46.0 seconds. Stage 2 reports the exact listed EV-5 identity as excluded from the mutation run.

Stage 2 catches all seven stage-1 missed identities. I compared the raw identities; the two sets match exactly.

The final result is 53 caught, zero missed, zero timeout, and 13 unviable. All 53 catch report identities match the raw catches.

The first-failure unit tests pass. The real-tool fixtures for shared-binary scoping, workspace-list absence, and flaky-catch reclassification also pass.

EV-5 passes in the unchanged slow step and the standalone stage-2 baseline. The coverage check examines 1014 slow-only tests.

Package CLEAN remains valid on this exact head, with zero open findings. This receipt replaces no failed gate evidence.

Integration execution review remains pending. The lead owns the merge decision and current-base check. I ran no gate, build, or test.

### PR #261 merge verification

The lead's handoff records merge `0c5ac80e512f986edd2ec1455012552a41c7f855` and integration completion `33be0d49`.

I verified that the merge parents are the reviewed base and exact reviewed head. The merge tree equals the reviewed head's tree.

## PR #262: exact counts for mutation exclusions

- Head: `de819474f8f0cdf590d9d15638fb9e2a271f5a47`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Risk: HIGH.
- VERDICT: CLEAN.
- Open findings: 0.

I read all four changed files and the PR body. The PR adds a check to `mutants-stale`; it does not remove a check.

Each configured `exclude_re` entry now states a count. An entry without a count note requires one match.

`count_of` accepts only ASCII digits, with no leading zero or sign, and a value of at least two. Invalid notes fail parsing.

The existing TOML validation in `gate_decisions::inputs` remains. The new trailing comments do not enter the exclusion pattern or reason.

`stale` preserves the zero-match failure. It also fails when a counted entry matches too few or too many mutants.

The count violation lists all matched names. Invalid regexes and failed list commands still fail the check.

`OFF_MACOS_EXCLUSIONS` keeps its previous nonzero-match rule. The PR adds no exclusion to that code list.

I compared both TOML documents with the standard TOML reader. All parsed values are identical, including every exclusion pattern.

Removing the 85 new count comments makes the file byte-identical to the base file. No pattern, reason, timeout, or configuration value changes.

The mutation stages, test selection, binary builds, and verdict rules remain unchanged. This additive check cannot accept an input that the previous check rejected.

The count check detects count changes. It does not prove that matched identities stay identical when one identity replaces another without a count change.

The prior-art note is present. The pinned old tree has no `.cargo` or `xtask` files and no corresponding mutation check to reuse.

### Gate evidence

I read the exact-head Linux gate log, with the exact base above:

`~/botster-sessions/gates/botster-core-stage1-gs-exact-count-de819474-pool-20261011-045219-1794.log`.

All ten steps PASS in 161.6 seconds. The check reports 208 exclusions against 6201 workspace mutants.

The new parser and count tests pass. The mutation step reports 18 mutants: 16 caught, zero missed, zero timeout, and two unviable.

All 16 raw catch identities match their catching-test reports. Stage 2 does not run because stage 1 misses no mutant.

The PR retains the failed first-head gate:

`~/botster-sessions/gates/botster-core-stage1-gs-exact-count-2c7bf039-pool-20261011-042105-29851.log`.

That gate missed `replace || with && in count_of` in both stages. Commit `d57dbe99` adds the rejected `# +3 mutants` case.

The current gate catches that exact operator mutant. The failed evidence remains visible; the change repairs the missing test before the new gate.

Integration review remains required for this HIGH PR. The lead owns the merge decision and current-base check. I ran no gate, build, or test.

### Integration completion

Sol published integration CLEAN in `7a7b414825cdcd4e008f99c7a2d5e8dc103c41c1` at the exact head and base above. No finding remains open.

Sol read all four changed files and the complete package addition. Sol independently confirmed the unchanged TOML values and all 16 catch identities.

Sol also confirmed the exact-head full gate and the signed-count correction. This unchanged head requires no repeat gate under section 8.

The lead owns the merge decision and current-base check. I will wait for a merge receipt or a new head.

### Lead merge hold

The lead holds #262 until #243 merges. The #243 gate and A5-4 table use base `0c5ac80e`.

GS must then merge the new `v1` into #262, supply a base-merge-check, and run one gate on the new exact head.

The current CLEAN applies only to `de819474f8f0cdf590d9d15638fb9e2a271f5a47`. I will check whether the base-merge rules permit CLEAN to carry over.

The lead assigns the next GS review: the `id_2` data-directory lock fix in `core-sys lock.rs`, risk HIGH.

## PR #265: data-directory lock, round 1

- Head: `96238f6209a2cf9587f8c78243b67bdc3336d564`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Risk: HIGH.
- VERDICT: NOT CLEAN (2 open).

I read all five changed files, the PR body, the approved design, and the LC-2 clause. I read the supplied gate and evidence logs.

The product lock follows the approved design: process-owned `F_SETLK`, a table by device and inode, metadata before open, and retained racing descriptors.

The drop closes the holder and retained descriptors under the table mutex. A poisoned table refuses new opens and still permits drop cleanup.

I checked rustix 1.1.5 source for Linux and libc. The selected operation uses `F_SETLK`, `F_WRLCK`, offset zero, and length zero.

The lead assigns the snapshot correction to P5's #257. Its stat-only handling of `lock` remains a required integration condition.

### GS-DL-R1-1 MEDIUM: OPEN — the regression test polls and can run without a work bound

File: `crates/botster-core-sys/tests/slow_data_dir_lock.rs`, lines 131–145.

The inner loop repeatedly calls `try_exclusive` after `Held`, until a deadline expires. This polls the lock instead of failing on the first refused reopen.

BUILD.md Testing rule 5 says: "No sleeps or polling." The regression needs one immediate reopen per drop; any refusal already proves its failure.

The outer loop also waits by reading `spawners.spawned()` until 200 children complete. It has no overall work bound.

If spawner threads panic before that count, the main thread continues successful lock cycles indefinitely. Their failed joins are reached only after that loop ends.

Required change: make each reopen one attempt that fails immediately on refusal. Use bounded child work and event-based coordination, with spawner failures propagated.

Preserve concurrent child starts, owned process cleanup, and the red-on-revert proof on Linux and macOS. Do not add a sleep or a polling wait.

### GS-DL-R1-2 LOW: OPEN — the prior-art note overstates the old fcntl use

The PR body says that the pinned old tree's `fcntl` calls only set `O_NONBLOCK`.

The pinned old source also uses `F_DUPFD` in `crates/botster-core/src/runtime/plugin_process/launch.rs:75` and `F_GETFD` in worker code.

Required change: limit the claim to the relevant fact: the old tree has no data-directory record-lock mechanism to reuse.

### Retained evidence

The exact-head full gate passes all ten steps in 261.1 seconds. Mutation results: two caught, zero missed, zero timeout, and seven unviable.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-96238f62-pool-20261011-053113-763.log`.

The Mac evidence at `a76cb906` and Linux evidence at `81fff9ac` each report `id_2` passing 30 of 30 concurrent runs.

Their revert checks report 95 refused reopens on Mac and 51 on Linux. These results show the old defect but do not close the test finding.

The PR retains the failed `81fff9ac` gate. The current head removes the missed NotFound guard; I read that correction.

I ran no gate, build, or test. All findings, LOW included, must close before CLEAN.

### Preliminary correction inspection

I read the source delta at `2bac0641e98678053a3e2ce6a0b3adfcaef361f7`. The implementer has not sent READY with its new evidence.

The test now makes one reopen attempt per fixed cycle. A channel event starts the cycles, and each spawner has a work bound.

GS-DL-R1-1 remains OPEN pending cleanup. `Spawners::drop` calls `joined.expect` inside the join loop.

If an early thread fails, that call panics before the remaining joins. Their handles drop and detach while their children can still run.

Required change: join every spawner, retain failures, then report failure after all joins. Preserve cleanup when the main test already panics.

I also asked the implementer to qualify the numerical probability claim with its independent-cycle assumption and correct the test description.

GS-DL-R1-2 remains OPEN. The live PR body still contains the old prior-art sentence when I read it.

This inspection is preliminary. The formal reviewed head remains `96238f62`, NOT CLEAN (2 open). I ran no gate, build, or test.

### Second preliminary correction inspection

I read the delta at `80f17bcf142823c5b8cebf16c704e2ee06b2a4cf` and the updated live PR body.

The destructor now joins every spawner before it reports failure. It avoids a second panic when the main test already panics.

The source now labels the probability as an estimate with an independent-cycle assumption. The body states the corrected test and its other failure cases.

The prior-art correction is published. GS-DL-R1-2 LOW is CLOSED on this head.

The source addresses GS-DL-R1-1. Its corrected Linux and Mac evidence and formal READY remain pending. No CLEAN is issued in this preliminary inspection.

## PR #265: round 2, corrected regression test

- Head: `80f17bcf142823c5b8cebf16c704e2ee06b2a4cf`.
- Base: `0c5ac80e512f986edd2ec1455012552a41c7f855`.
- Risk: HIGH.
- VERDICT: CLEAN.
- Open findings: 0.

GS-DL-R1-1 MEDIUM is CLOSED. Each of 2000 cycles makes one reopen attempt and fails on the first refusal.

The channel event replaces the progress polling. Each spawner has bounded work, and the destructor joins every thread before reporting failure.

The destructor also completes those joins when the main test already panics. Children retain their owned process guards.

The test requires concurrent child progress. Its probability statement is an estimate that explicitly assumes independent cycles, not a proved numerical bound.

GS-DL-R1-2 LOW remains CLOSED. I read the corrected prior-art note and the final PR body, including all current and retained evidence.

The production lock code is unchanged from the first reviewed head. No new source finding is open.

### Exact-head evidence

All three supplied logs identify this full head and base. I read the evidence commands and their results.

- Full Linux gate: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054802-97802.log`.
- Mac evidence: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054238-54403.log`.
- Linux evidence: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-80f17bcf-pool-20261011-054725-96840.log`.

The full gate passes all ten steps in 88.4 seconds. The default tier passes 1642 tests; the slow tier passes 484 tests.

Mutation results: nine mutants, two caught, zero missed, zero timeout, and seven unviable. Both raw catches match their catching-test reports.

The Mac evidence passes 123 core-sys tests. All 30 concurrent `id_` runs report `id_2=ok` and 13 passed tests.

The Linux evidence passes 122 core-sys tests. All 30 concurrent `id_` runs report `id_2=ok` and 13 passed tests.

Both evidence jobs then restore the base `lock.rs`. The corrected regression fails on the first refused reopen: cycle 15 on Mac, cycle 115 on Linux.

These failures occur at the lock assertion, not at a missing tool, compile error, or spawner failure. The evidence jobs restore the head file afterward.

The PR keeps the earlier failed gate and earlier evidence. Neither is substituted for the current exact-head results.

Integration review remains required. P5's #257 stat-only snapshot handling of `lock` remains an integration condition for the final base.

The lead owns the merge decision and current-base check. I ran no gate, build, or test.


## PR #265: base-merge delta review

- Head: `3c8044f40a6b26400240616b8946b05478b72fca`.
- Base: `ddd3b849df5db34d20d944ff6b8de23405fdb5c8`.
- Previous reviewed head: `80f17bcf142823c5b8cebf16c704e2ee06b2a4cf`.
- VERDICT: CLEAN, source delta.
- Open findings: 0.

The implementer reports that base-merge-check fails conditions 2 and 3 on `Cargo.lock`. I do not use automatic CLEAN carry-over.

I independently verified the merge parents and reviewed the overlapping file. The PR still adds only `botster-test-process` to core-sys dependencies.

The imported lockfile changes move the contracts pin to `.36` and add `botster-route-codec` to worker dependencies. The PR changes neither import.

The full own diff is byte-identical to the previous reviewed diff after removal of Git blob index lines. Hunk headers and content are identical.

Both normalized diffs have 24151 bytes and SHA256 `aa011c324c0b4d43745a9ce608f02ade6a13a4e33c67fdefdd490875da5b1dde`.

The product lock and corrected tests are unchanged. Both prior findings remain CLOSED. I found no new source finding in this delta.

The imported real-edge change retains a handed-off stream until acknowledgement. It adds no open of the lock file.

The lead directs #265 to merge first. P5 must retain the approved stat-only snapshot correction in #257 when that PR follows.

The final exact-head gate is reported running. No gate evidence for this new head is accepted yet. Integration delta review remains required.

I ran no gate, build, or test. The lead owns the merge decision and current-base check.


### New-head gate receipt

I read the final gate log for `3c8044f40a6b26400240616b8946b05478b72fca`, with base `ddd3b849df5db34d20d944ff6b8de23405fdb5c8`.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-datadir-lock-3c8044f4-pool-20261011-055225-5278.log`.

All ten steps PASS in 153.0 seconds. The gate exits 0. The default tier passes 1683 tests; the slow tier passes 499 tests.

The corrected regression and all lock probe tests pass. The coverage check examines 1031 slow-only tests.

The mutation result is unchanged: two caught, zero missed, zero timeout, and seven unviable. Both raw catch identities match their named reports.

Stage 2 does not run because stage 1 misses no mutant. Package CLEAN remains valid on this exact head, with zero open findings.

Pre-import integration CLEAN is `30f8045fe17057fd6ea2075b05b153e33ed8e2e5` at `80f17bcf`. New-head integration delta review remains pending.

The lead owns the merge decision and current-base check. I ran no gate, build, or test.

### New-head integration completion

Sol published integration CLEAN in `ae7ca202f15ccb271a2b7565e3cdb679952dba42` at the exact new head and base above. No finding remains open.

Sol checked the failed base-merge-check, the own diff, the lockfile overlap, all 37 imported base blobs, the merge tree, and the final body.

Sol also checked the final exact-head gate and both catch identities. The #257 stat-only snapshot condition remains.

Both reviewers are CLEAN on `3c8044f40a6b26400240616b8946b05478b72fca`. The lead owns the merge decision and current-base check.


### PR #265 merge receipt

The lead merged #265 at `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.

I verified both merge parents: the reviewed base `ddd3b849` and exact reviewed head `3c8044f4`. The merge tree equals the reviewed and gated tree.

The diff from the reviewed head is empty. The #257 stat-only snapshot condition remains.

The lead assigns the next review: GS's `job.sh` disk-floor PR, which measures Docker's data root instead of `$projdir`.

I will wait for GS's exact head. I ran no gate, build, or test.


## PR #266: Docker data-root disk floor, round 1

- Head: `a1bcbd7fb791e5a1a212a0e709b860d200178c88`.
- Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
- Scope: CI script; the lead requires package review and a pool gate.
- VERDICT: NOT CLEAN (1 open).

I read the complete `ci/remote/job.sh`, its one-file diff, and the PR body. The new path comes from Docker's data-root setting.

The thresholds, volume locks, eviction order, command, and test verdict remain unchanged. The prior-art note is present; the pinned old tree has no remote gate script.

### GS-DF-R1-1 MEDIUM: OPEN — a later measurement failure can bypass the disk rejection

File: `ci/remote/job.sh`, lines 185–192 and the final `if below "$fail_gb"`.

The startup condition rejects an empty first measurement. Each later `below` call performs a new measurement without checking its status or validating its output.

If the final `df` fails and produces no number, `below` evaluates an invalid arithmetic command of the form `(( < 40 * ... ))`.

That command returns failure. Because `below` is the condition of `if`, the shell treats failure as false and skips the exit-75 branch.

The previous `free` assignment can succeed before this final read fails. The script can then initialize volumes and start containers despite the unknown measurement.

`set -euo pipefail` does not make an `if` condition fail closed. I checked the installed Bash manual's `-e` condition exception.

Required change: check every decision measurement's command status and numeric form explicitly. Exit 75 from the job shell on any failed measurement.

Do not turn a failed measurement into the false result of `below`. Preserve the thresholds and eviction order.

Supply focused script evidence for a failed first read and a failed later read, including the final fail-floor read. Confirm exit 75 before container start.

The evidence can use fake `df` and Docker commands; no pool node needs an actual filesystem fault.

### Retained gate evidence

The exact-head pool gate passes all ten steps in 260.2 seconds. Its data-root line reports 367 GiB on `/pool/docker` and 481 GiB on `$projdir`.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-disk-floor-a1bcbd7f-pool-20261011-060243-37086.log`.

This gate proves the successful measurement path. The PR states that the exit-75 measurement path was not exercised; it does not close this finding.

The Rust mutation step reports zero mutants because this diff changes no Rust source. I ran no gate, build, or test.


## PR #266: round 2, checked measurements

- Head: `11a9533f8e3929b528b7d4a762f0019e4e18734d`.
- Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
- VERDICT: NOT CLEAN (1 open).

GS-DF-R1-1 MEDIUM is CLOSED. Every decision measurement now calls `measure` in the job shell.

The assignment checks the pipeline status. The numeric check rejects empty or invalid output. Failure exits the job shell with 75, including inside `below`.

The final measurement and threshold comparison use the same checked value. The project-directory measurement is diagnostic only and can report an unknown amount.

Thresholds, locks, eviction order, and the gate command remain unchanged. I read the complete source delta and the evidence script without executing it.

The evidence runs the real script with fake Docker and df commands. All nine current-head scenarios pass.

Failed first, floor, final, and eviction-loop reads each exit 75 with zero container starts. Invalid final output and a missing data root do the same.

The low-space case exits 75. The two successful cases exit 0. The below-floor case evicts the old target before proceeding.

The three earlier-head scenarios fail the required exit-75 result. Their actual exit is 1, so the evidence proves the changed failure handling.

Evidence: `~/botster-sessions/gates/botster-core-stage1-gs-disk-floor-11a9533f-pool-20261011-061246-53541.log`.

The exact-head full gate passes all ten steps in 118.7 seconds, with the exact base above. It reports 346 GiB at the Docker root and 460 GiB at the project directory.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-disk-floor-11a9533f-pool-20261011-061402-57367.log`.

### GS-DF-R2-1 LOW: OPEN — the current fix description names a removed function

The live PR body's first fix bullet says that `free_bytes()` measures Docker's data root. The current source removes that function.

Required change: describe `measure()` and `free_on()` as implemented, or describe the measured filesystem without naming a function.

This is a body-only correction. The source and exact-head evidence require no change for this finding. I ran no gate, build, or test.


### PR #266 body correction and final verdict

- Head: `11a9533f8e3929b528b7d4a762f0019e4e18734d`, unchanged.
- Base: `c9ae2c2ffd9682d8bac29583205b39d13cc9f2a5`.
- VERDICT: CLEAN.
- Open findings: 0.

GS-DF-R2-1 LOW is CLOSED. I read the live PR body; its current fix bullet correctly names `measure()` and `free_on()`.

The defect paragraph retains the old function name because it describes the old source. GS-DF-R1-1 MEDIUM remains CLOSED.

The exact-head full gate and focused failure evidence remain valid. No source commit changed after those runs; no repeat gate is needed.

This verdict covers Core PR #266 only. It does not review or approve the separate contracts copy.

The lead owns the merge decision and current-base check. I ran no gate, build, or test.


## Contracts PR #44: Docker data-root disk floor

- Repository: `trybotster/botster-contracts`.
- Head: `7176d02b577c9043c72ca4eda3b293104aad6961`.
- Base: `3c74af18f0d8b8eba437db133ae5372083e8d97f`.
- Risk tier: STANDARD, confirmed.
- VERDICT: NOT CLEAN (1 LOW open).

I read the complete script, the one-file source diff, the live PR body, and the exact-head gate log. The base is an ancestor of the head.

The changes decide gate start and cache eviction. They do not change step commands, mounts, status propagation, or artifacts.

The disk decision block matches Core head `11a9533f8e3929b528b7d4a762f0019e4e18734d` byte for byte. All added and removed patch lines also match.

Each decision measurement checks status and numeric output. An unknown data root or failed measurement exits the job shell with 75.

The final comparison uses the same checked value. The project-directory measurement is diagnostic only. The thresholds and eviction order remain unchanged.

Outside the settings block, the scripts differ only in the existing stage-2 artifact description and artifact-copy list. Contracts has no stage-2 references in `ci` or `xtask`.

Those existing differences do not affect the changed decision block. The body records the drift and the steward's separate shared-source plan.

### Exit-75 propagation

The script's EXIT cleanup saves the exit status and exits with that status. The pool wrapper uses `exec` to start this script.

I read the installed `pool-run`, `jobq`, and shared `botster-gate` sources. Each preserves a completed command's nonzero status.

`pool-run` can replace the status with 124 or 137 for silence or an out-of-memory failure. Neither status is green.

`jobq` reads the completed allocation's exit code. It records a cached pass only for status zero. The gate invocation supplies no cache option.

`botster-gate` preserves the `jobq` status. Its output checks can change zero to 4; they cannot change 75 to zero.

Thus, a completed disk rejection with exit 75 cannot produce a green result through the reviewed status path. This is source evidence; I ran no failure probe.

### Supplied evidence

The exact-head Contracts gate passes all ten steps in 237.2 seconds. It records the exact head and base above.

It reports 341 GiB at `/pool/docker` and 456 GiB at the project directory. The mutation step reports no changed Rust source.

Log: `~/botster-sessions/gates/botster-contracts-stage1-gs-disk-floor-7176d02b-pool-20261011-061636-61682.log`.

The Core failure evidence covers the identical decision block. All nine current-head scenarios pass; failed and invalid reads exit 75 before container start.

Evidence: `~/botster-sessions/gates/botster-core-stage1-gs-disk-floor-11a9533f-pool-20261011-061246-53541.log`.

The current source changes no test selection, mutation scope, binary build, mutation configuration, or test verdict. I found no new path that accepts a failed test or mutant.

The existing candidate-binary limitation remains outside this change. This review does not approve that limitation.

### GS-CD-44-1 LOW: OPEN — the body overstates the patch comparison

The body says the `job.sh` diff is byte-identical to Core #266. The raw Git diffs differ in blob identifiers, hunk positions, and surrounding artifact text.

The added and removed lines are identical. The new disk decision block is byte-identical. These narrower statements are verified.

Required change: replace the raw-diff claim with one of those verified statements. This is a body-only correction; the source needs no change.

No other finding remains. No repeat gate is needed for this body-only correction. I ran no gate, build, or test.


### Contracts PR #44: body correction and final verdict

- Head: `7176d02b577c9043c72ca4eda3b293104aad6961`, unchanged.
- Base: `3c74af18f0d8b8eba437db133ae5372083e8d97f`.
- Risk tier: STANDARD.
- VERDICT: CLEAN.
- Open findings: 0.

GS-CD-44-1 LOW is CLOSED. I read the live body and confirmed the unchanged exact head.

The body now states that the added and removed lines match Core #266. It correctly identifies the byte-identical disk decision block.

The two related statements also use the narrower patch comparison. No source changed. The existing exact-head gate remains valid.

The exit-75 source review and the scope review remain valid. This verdict covers Contracts #44 itself.

The steward owns the merge decision and current-base check. The lead relays this verdict. I ran no gate, build, or test.


### Core PR #266: merge receipt

- Final head: `928d2c1cb6509219cddaebeecbd6cf013411e24c`.
- Final base: `ee7c6fca8c0781db09203ac478a8e824cdd3e4ab`.
- Merge commit: `2b8cf08230dc3145dfe3a436bc34feee946c9b07`.
- VERDICT: CLEAN carry-over; merged.
- Open findings: 0.

The lead reports that all canonical base-merge-check conditions pass. The live PR body records PASS and an identical own diff.

I independently compared the complete own Git diffs before and after the base merge. They are byte-identical. The script itself is unchanged.

The final head's parents are the reviewed head `11a9533f8e3929b528b7d4a762f0019e4e18734d` and the final base above.

The merge commit's parents are the final base and final head. Both trees are `8f54a07bb4a20e281c6f538b2eeab105f471ea89`.

I read the final gate log. It identifies the exact final head and base and passes all ten steps in 116.3 seconds.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-disk-floor-928d2c1c-pool-20261011-062636-87321.log`.

The mutation step reports zero mutants because the own diff changes no Rust source. Both closed findings remain closed.

The next assigned review is the Contracts shared-script split, risk HIGH. I will wait for its exact head. I ran no gate, build, or test.


## Contracts PR #45: shared gate script

- Repository: `trybotster/botster-contracts`.
- Head: `56e5b744588b98ccbbb892d6b0a63522317284f0`.
- Base: `7176d02b577c9043c72ca4eda3b293104aad6961`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package review).
- Open findings: 0.

I read both complete scripts, the three-file source delta, the live body, and both supplied evidence logs. The base is an ancestor of the head.

`job.sh` keeps its executable mode, strict shell options, and settings. It sources the adjacent common file in the same shell.

The common file needs no executable mode. The source operation preserves positional arguments, shell options, traps, and exit behavior.

A missing common file fails under the existing strict options before container start. The project guard rejects the supplied standalone invocation without settings.

The existing settings are byte-identical after removal of the new artifact setting. The new array has the exact three existing artifact paths.

From the first `dir` assignment onward, the old and new bodies differ only in the artifact comment and artifact invocation.

The new `sh -c` invocation supplies `sh` as argument zero. It passes each array entry as a separate positional argument.

The loop reads every path through quoted `"$@"`. The first path is preserved. The destination naming, mounts, copy command, and failure handling remain unchanged.

The failed main command's status remains saved while the artifact container runs. Artifact failure cannot replace that saved status with zero.

The disk decisions and exit-75 cleanup remain unchanged. The completed-command status path reviewed for #44 still applies.

The change does not alter test selection, mutation scope, binary builds, exclusions, timeouts, or step verdicts. I found no new acceptance path for a failed test or mutant.

The existing candidate-binary limitation remains outside this change. This verdict does not approve that limitation.

The prior-art note is present. The pinned old Core tree has no `ci/remote` files.

The handoff records Contracts ownership and the planned Core copy check. The body clearly leaves the Core copy and check for the next pin move.

### Evidence

I decoded and read the focused evidence script without executing it. Its three input hashes match the exact Git blobs I read.

All nine disk scenarios give matching old and new exits and identical normalized Docker run logs. Failed decisions exit 75 with zero container starts.

The failed main-command scenario gives exit 3 in both scripts. The captured artifact invocation supplies all three paths after argument zero.

The extracted artifact scripts copy identical path lists from the fake tree. Every other Docker run remains identical.

A missing common file gives exit 1 with zero container starts. The standalone invocation gives exit 1 at the project guard.

The evidence reports zero failures. It exercises Contracts settings; the future Core settings and copy need their own review.

Focused log: `~/botster-sessions/gates/botster-contracts-stage1-gs-job-common-56e5b744-pool-20261011-063127-22026.log`.

The full gate identifies the exact head and base above. All ten steps pass in 336.1 seconds. The Linux job uses the split script.

The mutation step reports no changed Rust source. No Rust mutant was required for this shell-only source change.

Full gate: `~/botster-sessions/gates/botster-contracts-stage1-gs-job-common-56e5b744-pool-20261011-063138-24237.log`.

Sol owns the separate integration review. The steward owns the merge decision and current-base check. I ran no gate, build, or test.


### Contracts PR #45: integration result

Sol reports integration CLEAN with zero open findings in local commit `efeb8c68ed3e6920c288e0ea929e308387d34aaf`.

I read `verdicts/stage1/contracts-gs-job-common.md` from that commit. It identifies the same exact head `56e5b744588b98ccbbb892d6b0a63522317284f0` and base `7176d02b577c9043c72ca4eda3b293104aad6961`.

The integration result agrees with the package result. The artifact evidence compares copied path lists, not content hashes.

The unchanged `cp -a` command supports the copy behavior. Neither review approves the future Core copy, byte check, or pin move.

Sol reports that GitHub DNS blocked publication. The integration commit is available locally; I do not claim that Sol pushed it.

The steward retains the merge decision and current-base check. I ran no gate, build, or test.


### Contracts PR #45: integration publication

Sol confirms that integration verdict `efeb8c68ed3e6920c288e0ea929e308387d34aaf` is now pushed. The earlier DNS publication block is resolved.

The exact head `56e5b744588b98ccbbb892d6b0a63522317284f0` remains package and integration CLEAN with zero open findings.

The lead received Sol's CLEAN for relay to the steward. The future Core copy and check still need their own review.

I ran no gate, build, or test.


## Core PR #262: new-base delta review

- Head: `bbd25214835b4648623cb1e5ac3a89ac50f5d260`.
- Base: `56fae5826f766b408abca06f99376e0506982bf9`.
- Risk tier: HIGH.
- VERDICT: NOT CLEAN (1 LOW open).

The implementer reports canonical base-merge-check failures (1), (2), and (3) on `.cargo/mutants.toml`. Condition (4) passes.

I performed a source delta review. The new head has parents `c5cfdde84f79179328e7b9db19dbf603ca919d14` and the final base above.

The earlier merge has parents `de819474f8f0cdf590d9d15638fb9e2a271f5a47` and `ddd3b849df5db34d20d944ff6b8de23405fdb5c8`.

All three own `xtask` diffs are identical to the reviewed diffs after removal of Git blob index lines. The exact-count parser and decisions remain unchanged.

Removing the new trailing count notes makes `.cargo/mutants.toml` byte-identical to the final base file. No pattern, reason, or configuration value changes.

The new file has 81 count notes. Every annotated line is byte-identical to an annotated line in the earlier reviewed file.

The four removed notes belong to the broad exclusions deleted from the base by #267: `Driver::run`, `Driver::read_pty_chunk`, `Driver::read_control`, and `RealEdges::link_send_descriptor`.

Their old counts were 2, 3, 2, and 4. The merge does not restore those exclusions or increase any count.

The final file has 203 `exclude_re` entries. The implementer reports 204 total exclusions checked against 6458 mutants in the focused check.

I did not execute that check. The implementer reports that the final full gate is running; no final gate log was supplied with this delta request.

The unchanged rules still fail on zero matches or an unequal count. Count equality still cannot prove identity equality after a one-for-one replacement.

The merge changes no own binary build, test selection, mutation stage, or verdict. I found no source defect in the delta.

### GS-EC-R2-1 LOW: OPEN — the body still presents old counts as current

The live body says that 85 of 207 entries have count notes and describes 208 exclusions against 6201 mutants as the merged-head result.

The new head has 81 notes and 203 entries. Those current-source statements need correction. Retain the old gate as clearly identified historical evidence.

Required change: update the current file counts, describe the four deleted exclusions, and add the final exact-head evidence when available.

This finding requires a body correction. It does not require a source change or a repeat gate after the current gate.

Sol must review this HIGH delta separately. I ran no gate, build, or test.


### Core PR #262: final delta verdict

- Head: `bbd25214835b4648623cb1e5ac3a89ac50f5d260`, unchanged.
- Base: `56fae5826f766b408abca06f99376e0506982bf9`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package review).
- Open findings: 0.

GS-EC-R2-1 LOW is CLOSED. I read the complete corrected live body and confirmed the unchanged exact head.

The current descriptions now state 81 notes, 203 entries, and 204 checked exclusions against 6458 mutants. Old totals appear only as history.

The body names the four deleted exclusions and retains the earlier failed and passing gate logs. The Changes bullet also states 81 notes.

I read the complete canonical base-merge-check log. Its failures require this source delta review; they do not permit automatic carry-over.

The prior source comparison remains valid. The own logic is unchanged, and the annotation resolution preserves the new base's exact patterns and comments.

I read plan pin `stage1-plan.12ba52e3.md`, section 8, and the final gate's scope, test summaries, mutation results, and final summary.

The final gate identifies this exact head and base. All ten steps pass in 328.3 seconds.

The default suite passes 1690 tests. The slow suite passes 546 tests. The slow-coverage check covers 1049 slow-only tests.

The exclusion check reports 204 exclusions against 6458 workspace mutants. The mutation baseline passes.

The mutation result is 18 mutants: 16 caught, zero missed, zero timeout, and two unviable. Stage 2 does not run because stage 1 misses none.

All 16 catch report identities match the raw catches. The caught identities match the earlier reviewed gate.

One catching test changes for the `check` mutant that returns an empty vector. The new report names `a_cited_decision_is_matched_by_name_and_a_shared_name_fails_closed`.

The old report named `a_decision_mutant_is_never_excluded_and_a_cited_whole_body_exclusion_passes`. Both reports name a catch; no catch identity is lost.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-exact-count-bbd25214-pool-20261011-071920-20943.log`.

Base check: `~/botster-sessions/gates/botster-core-stage1-gs-exact-count-bbd25214-base-merge-check.log`.

Sol's published round 2, `9e8fea5a6abe6c798ce312a40f9b561635962179`, reports no source defect and waits for body and evidence closure.

Sol owns the final integration closure. The lead owns the merge decision and current-base check. No repeat gate is needed for the body correction.

I ran no gate, build, or test.


### Core PR #262: integration completion

Integration CLEAN is published in `c950e6e47ebe8f5670ac683f007761c7aff3857c` at the same exact head `bbd25214835b4648623cb1e5ac3a89ac50f5d260`.

The base remains `56fae5826f766b408abca06f99376e0506982bf9`. I read the closure artifact and verified its remote branch contains the commit.

Sol closes EC-SI-1 after reading package closure `7514a477b5b0c1fcca67e37d1cb4898624b2a729` and the last body correction.

Both reviews are complete with zero open findings. The failed canonical base-check remains failed; both reviews supply the required source delta review.

The exact-head gate remains valid. No repeat gate is required. The lead owns the merge decision and current-base check.

I ran no gate, build, or test.


## Core PR #270: bounded worker waits

- Head: `9298549099b9636b7297fb341b21f0b98e3a0433`.
- Base: `c942593697e856017572977af0405b17de0124e2`.
- Risk tier: STANDARD, as assigned by the lead.
- VERDICT: NOT CLEAN (1 MEDIUM open).

I read the complete two-file delta, the changed call contexts, both platform adapters, the shared status observer, the check's matching logic, and the body.

The helper observes an exact unreaped child through the existing bounded `await_status`. It reaps only after a status is available.

The Linux adapter waits on a pidfd. The Mac adapter waits on SIGCHLD through kqueue and checks status again after each event.

The helper returns an error on a deadline, observation error, missing status, or reap error. The three ordinary call sites panic on that error.

The observer-parent drop reports an error during an existing panic. Otherwise it panics. This avoids a second panic that would abort the test process.

The proof uses a Blocker payload and a marked 100 ms deadline. It checks the exact report, then stops and removes the unreaped worker.

The change preserves the existing cleanup bound at the four sites. It changes no product code, testkit adapter, mutation scope, binary build, or verdict logic.

A worker that stays alive produces a failure instead of an unbounded wait. I found no source path that converts the old timeout into a pass.

### Ruling on the new process-check entry

I accept the single `status_within | child-wait` entry for this exact source. It covers one nonblocking `try_wait` after bounded status observation.

The check classifies `try_wait` as child-wait by syntax. It cannot infer the preceding observation or this test's ownership of the unreaped child.

The entry's comment states both facts. The entry replaces four entries for unbounded waits and introduces no polling loop or unbounded wait.

This is a reviewed false-positive exception. It does not authorize another raw wait or a future change to this helper.

### Supplied Linux evidence

The exact-head gate identifies the base above and passes all ten steps in 261.4 seconds. Process-check reports 112 allowed sites.

The default suite passes 1690 tests. The slow suite passes 548 tests. The new proof passes in both worker fixtures.

The mutation step finds no mutant because the source change is in test code. The earlier taint failure remains recorded in the body.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-bounded-waits-92985490-pool-20261011-074238-70139.log`.

I decoded and read the Linux reversal script without executing it. The corrected proof passes in 0.108 seconds.

Replacing the observer with `Child::wait` makes the same proof time out in 2.002 seconds. The evidence script restores the source afterward.

Evidence: `~/botster-sessions/gates/botster-core-stage1-gs-bounded-waits-92985490-pool-20261011-074127-66951.log`.

### GS-BW-1 MEDIUM: OPEN — required Mac pool evidence is missing

Plan pin `stage1-plan.70cf75fb.md`, section 8, revision 23ai, requires a focused Mac pool run for portable code whose behavior depends on signals or reaping.

The run's log must appear in the PR. This change depends on the Mac status adapter and real worker cleanup.

The body states local Mac results without a named Mac pool log on this exact head. Older-head proof claims do not supply that evidence.

Required change: supply an exact-head focused Mac pool log for the proof and four changed sites. Add the log and results to the body.

Do not repeat the green Linux gate. The source needs no change for this finding. I ran no gate, build, or test.


### Core PR #270: shared-helper revision and final package verdict

- Head: `7e568e20bbac33a99f01fbf3b2cdd3acb6d5b804`.
- Base: `c942593697e856017572977af0405b17de0124e2`.
- Risk tier: HIGH, under the lead's revised ruling.
- VERDICT: CLEAN (package review).
- Open findings: 0.

The lead replaced the previous allow-entry design with the shared helper. I reviewed the complete delta from the earlier reviewed head and the final own diff.

`reap_within` becomes public and receives a crate-root re-export. Its parameter signature and function body remain byte-identical.

The new documentation states the caller's ownership after a deadline. Existing testkit callers and platform adapters remain unchanged.

`status_within` calls the shared helper and adds the step, pid, and error report. A deadline or error still fails the ordinary test call sites.

The observer-parent test now uses `OwnedChild::spawn`. It takes stderr, sends SIGKILL, and drops the owner for its existing bounded reap.

The ownership mode remains child-only. It does not kill the observer's process group, which the parent-death proof must observe separately.

The custom `ObserverParent` drop is removed. The allow file removes five entries, including one of the duplicate drop entries, and adds none.

The previous accepted helper entry is absent. That earlier ruling does not apply to the final source. The revised risk tier requires Sol's integration review.

The cleanup bounds, worker exit assertions, proof deadline, test selection, binary builds, and mutation verdict remain unchanged from the reviewed bounded-wait design.

I found no source path that converts a failed wait into a pass. The existing candidate-binary limitation remains outside this change.

### GS-BW-1 MEDIUM: CLOSED

The exact-head Mac pool log is now in the live body. It identifies the head and base above and runs on Darwin.

The testkit suite passes 53 tests. Both worker fixtures pass 137 tests, including the proof and every changed site.

The proof passes in 0.152 and 0.154 seconds. Replacing the shared reap call with plain `Child::wait` makes both proofs time out in 2.028 and 2.035 seconds.

Mac evidence: `~/botster-sessions/gates/botster-core-stage1-gs-bounded-waits-7e568e20-pool-20261011-075344-20569.log`.

The equivalent Linux evidence passes 53 testkit tests and 140 worker tests. Both reverted proofs time out in 2.002 seconds.

Linux evidence: `~/botster-sessions/gates/botster-core-stage1-gs-bounded-waits-7e568e20-pool-20261011-075447-24981.log`.

I decoded and read the evidence script without executing it. It checks the real test command statuses and reports one reverted call and zero changed files after restoration.

The reversal changes only the wait call. Its timeout evidence supports the bounded-wait proof on both platforms.

I read plan pin `stage1-plan.0af4db8d.md`, section 8's platform-evidence rule. The new Mac evidence satisfies revision 23ai.

The final Linux gate identifies the exact head and base. All ten steps pass in 112.6 seconds. Process-check reports 110 allowed sites.

The default suite passes 1690 tests. The slow suite passes 548 tests. The coverage check covers 1051 slow-only tests.

The mutation step lists no mutant for the final diff. The earlier failed taint gate remains identified in the body.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-bounded-waits-7e568e20-pool-20261011-075038-10685.log`.

The live body describes the revised source and HIGH scope. Sol owns integration review. The lead owns the merge decision and current-base check.

I ran no gate, build, or test.


### Core PR #270: new-base source delta

- Head: `b038bac80c7939343433ec2e843c393f694470ce`.
- Base: `1da39f7c9bb99b8cf8c8fd85355e5e48fd7b4c21`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package source review).
- Open findings: 0.

The lead requires source CLEAN before the final gate. The head merges reviewed head `7e568e20bbac33a99f01fbf3b2cdd3acb6d5b804` with the new base.

I read the complete canonical base-check log. Conditions (1) and (4) pass; conditions (2) and (3) fail on `crates/botster-test-process/src/lib.rs`.

I performed the required source delta review. The complete own diff is identical after removal of Git blob index lines and hunk positions.

The sole shifted hunk is the `reap_within` export. The new base adds the `alone` module and its documentation above that export.

The merged file keeps both additions and the same reap export. The helper body, worker callers, and allow-file removals remain unchanged.

The live body identifies the previous-head logs correctly. Those logs do not constitute the final Linux gate on this new head.

GS-BW-1 remains CLOSED: its Mac evidence covers unchanged own wait code and the same platform adapters. The source delta adds no new platform wait.

The lead requires one final exact-head Linux gate. Sol must complete the HIGH integration delta. Both conditions remain pending.

The package source verdict does not claim final gate acceptance or merge readiness. I ran no gate, build, or test.


### Core PR #270: integration source completion

Integration source CLEAN is published in `b421411dde23c595d42d448bbc49fe26f54a4b59` at `b038bac80c7939343433ec2e843c393f694470ce`.

The base remains `1da39f7c9bb99b8cf8c8fd85355e5e48fd7b4c21`. I read the complete artifact and verified its publication.

Both source reviews have zero open findings. Sol independently confirms the own diff, imported export context, unchanged platform adapters, and Mac evidence transfer.

The final exact-head Linux gate remains pending. Neither source verdict accepts a future gate. I ran no gate, build, or test.


## R-43(5): driver and transcript design review

The implementer asks whether the transcript-start control rule belongs in the same PR as the transcript changes. I recommend one HIGH PR.

One PR lets the negative signal proof test the final transcripts and the driver rule together. A separate driver PR can also be safe if it lands first.

The unsafe intermediate state is widened transcripts without the required strict-wake control rule. This recommendation is not an exact-head CLEAN.

I read the runner's required-control checks and setup order, the shared driver trait, Core setup, wake expectations, and the fresh-script entry point.

A default no-op `StepDriver::start` hook is a suitable seam. The runner must call it after required-control checks and before setup opens a Core handle.

A start error must produce a step-zero failure. Drivers that retain the default must preserve their behavior.

The scan must inspect executable nested step lists. It must not interpret arbitrary payload or expected-object JSON as executable steps.

The existing wake executor defaults a missing `expect` to `Woken`. That form is also strict unless the implementation makes it invalid.

`run_script_events` constructs fresh Core drivers outside the main runner. It needs the same transcript-start rule for its scripts.

The proof must cover hook order and errors, default behavior, nested and absent strict waits, the real harness without the control, and seeded missing-signal failure.

A supported control that fails must fail the run. A harness without the control retains its behavior. Other transcripts retain their seeded spurious wakes.

I sent these conditions to the implementer. I corrected an earlier message that overstated the risk of every separate-driver ordering.

No code or gate was supplied for this design request. I ran no gate, build, or test.

## Contracts PR #50: R-43(5), round 1

- Head: `9bb6fd56e88514de82ecbc4d93c920372a3e0736`.
- Reviewed base: `9d19b997fd044ca76c0844f8a32fa79d930e5ec4`.
- Risk tier: HIGH.
- VERDICT: NOT CLEAN.
- Open findings: 1 HIGH.

### GS-R43-1: the strict-wake scan includes steps that the runner does not execute

`strict_woken_wait`, in `crates/botster-core-conformance/src/driver.rs:1674`, scans every top-level `ack` and `barrier` value.

The order executor uses those fields only on recognized order constructs. Its `order_kind` function rejects objects with keys outside each construct's form.

Core ordinary steps can contain those fields without executing them. The transcript schema permits arbitrary step JSON through `items: true`.

For example, `{"data_dir":"extra","ack":{"wait_wake":{},"expect":"Woken"}}` executes only `data_dir`. CoreDriver returns before it reads `ack`.

The new scan reports a strict wait for this example. CoreDriver then disables seeded spurious wakes for the entire transcript.

Append `{"wait_wake":{"timeout_ms":0},"expect":"TimedOut"}` to that example. No executable step expects strict `Woken`.

With the seeded harness used by the new tests, the existing seeded wake makes the final wait fail. The new scan removes that wake and permits a pass.

This conclusion follows from source inspection. I did not execute this example. The existing no-strict-wait test establishes the relevant seeded-wake behavior.

The change therefore removes A5-2 coverage from a transcript outside the authorized strict-wait scope. Plan revision 23av requires other transcripts to retain seeded spurious wakes.

Required repair: classify executable nested steps with the same order-form rules that the runner uses. Do not scan unused `ack` or `barrier` fields.

Also keep direct wait detection consistent with Core step dispatch. An earlier ordinary operation can return before a second operation field executes.

Add a negative scope proof for the example above. It must retain the seeded wake and apply no `no_spurious_wakes` control.

Keep the proofs for valid nested order constructs, missing `expect`, failed controls, and fresh scripts. Do not replace the runner's form rules with a second approximate parser.

### Evidence and review scope

The runner calls the default no-op start hook after required-control checks and before setup. It converts a hook error through the existing step-zero error path.

The real testkit control does not require an open handle. It updates the run's scheduler. The before-setup placement prevents an initial seeded wake from remaining set.

The eight transcript changes remove the required control. Strict `Woken` expectations remain strict. Ordinary widened waits have a same-clock pump with no event and no `more`.

The `tm_3` exception uses one posted event, `pump_idle`, and exactly `[Silent]`, as plan revision 23av requires.

The exact-head contracts gate reports ten passing steps in 543.8 seconds. It reports 12 mutants: 10 caught and 2 unviable.

Gate: `~/botster-sessions/gates/botster-contracts-stage1-gs-r43-spurious-wakes-9bb6fd56-pool-20261011-083955-96932.log`.

I read both Core comparison logs. The testkit reports 209 passes and the same four failing ids before and after the change.

The real tier changes from eight unsupported-control results to four passes, one strict-wake failure, and three unsupported `route_fill` results.

The strict-wake failure is `tm_6_begin_and_attach_signal_wake`. The implementation does not widen that expectation to hide the failure.

Comparison logs: `botster-core-scratch-gs-r43-before-f29aeae1-pool-20261011-082419-31009.log` and `botster-core-scratch-gs-r43-after2-8d71a84a-pool-20261011-085024-24401.log` under `~/botster-sessions/gates/`.

These comparison commands report known failures and still exit zero. Their exit status is not a conformance PASS verdict.

Sol must complete the HIGH integration review. A later base merge requires its own review under the lead's current-base rule.

The passing gate does not close GS-R43-1. I ran no gate, build, test, or executable probe.

## Contracts PR #50: R-43(5), round 2

- Head: `2bec33c8b5d8fa45d1e2bdc5f152084f7bc64dee`.
- Source comparison: reviewed head `9bb6fd56e88514de82ecbc4d93c920372a3e0736`.
- Gate base: `5ff1b52be672bc107e80cd0d911110034745eda4`.
- Risk tier: HIGH.
- VERDICT: NOT CLEAN.
- Open findings: 1 HIGH, GS-R43-1.

The repair closes the original unused `data_dir` plus `ack` example. Its new scope test retains the seeded wake and applies no control.

The runner and scanner now share `order_kind`. Direct waits share `wait_wake_spec`, and Core dispatches those waits before other operations.

The moved wait body remains unchanged. The thread test checks both the owner thread and a separate waiting thread.

`run_script_events` now scans only direct steps. It retains the common control application and error mapping.

### GS-R43-1 remains OPEN: an empty set never executes its acknowledgement

The new scan still assumes that every recognized `expect_set` executes its `ack`. An empty `expect_set` does not execute any acknowledgement.

In `crates/botster-conformance/src/order.rs`, `expect_set` calls `ack_item` only inside its loop over the matchers. The empty set passes without a driver call.

The following steps contain no executed strict wait:

```json
[
  {"expect_set": [], "ack": {"wait_wake": {}, "expect": "Woken"}},
  {"wait_wake": {"timeout_ms": 0}, "expect": "TimedOut"}
]
```

The scanner reports a strict wait and disables seeded wakes. With the existing seeded harness, the final wait can pass after that control.

Without the control, the seeded wake makes the final wait fail. The example therefore retains the false-pass risk outside the authorized scope.

This is a source proof. I did not execute the example. Add a negative scope test that retains the wake and applies no control.

The scan also uses recursive runner classification for both `ack` and `barrier`. The runner executes these fields through different paths.

`ack_item` passes the prepared acknowledgement directly to `driver.exec` at line 327. `expect_quiet_until` passes its barrier to `run_step` at line 440.

Required repair: scan an executed acknowledgement with the direct-step predicate. Scan a barrier with the recursive runner predicate.

Do not scan an acknowledgement when an empty set cannot execute it. Keep the shared `order_kind` predicate and the direct dispatch agreement.

### Evidence

I read the exact-head gate header and summary. All ten steps pass in 371.9 seconds. The mutation step reports 18 caught and 2 unviable mutants.

Gate: `~/botster-sessions/gates/botster-contracts-stage1-gs-r43-spurious-wakes-2bec33c8-pool-20261011-091059-3335.log`.

The body retains the failed `3d3802d4` gate. I verified its missed `other_thread` comparison mutant and the mutation-step failure.

The new test supplies the source repair for that failure. This gate does not constitute an unchanged-head retry.

The transcript files remain unchanged from round 1. The supplied Core comparison evidence still describes those files and their direct wait forms.

I read the updated live body and plan revision 23bd. Neither the passing gate nor the revised body closes GS-R43-1.

I ran no gate, build, test, or executable probe. Sol owns the HIGH integration review.

## Contracts PR #50: R-43(5), round 3

- Head: `338008cbeea0bc1e675680e2d1c2935bf4702383`.
- Source comparison: reviewed head `2bec33c8b5d8fa45d1e2bdc5f152084f7bc64dee`.
- Original source base: `9d19b997fd044ca76c0844f8a32fa79d930e5ec4`.
- Gate base: `5ff1b52be672bc107e80cd0d911110034745eda4`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package review).
- Open findings: 0.

GS-R43-1 HIGH is CLOSED. The scan ignores an empty or non-list `expect_set`. The exact empty-set example applies no control and retains the seeded wake.

Acknowledgements use the direct `strict_wait_step` predicate. Barriers use the recursive runner predicate. The shared `order_kind` function retains its original classification body.

The scanner therefore follows the distinct execution paths that caused the round 2 finding. The new tests cover both paths and the empty set.

The original scope repair remains intact. Unused fields do not disable seeded wakes. Direct waits and Core dispatch use the same `wait_wake_spec` predicate.

GS-R43-R3-LOW-1 is CLOSED. The live body first stated 11 tests without a historical-head qualifier. The exact head has 13 tests.

The implementer corrected the body without a source change. Each local reversal now names its historical head and that head's test count.

I verified the unchanged exact head and corrected live body. The supplied gate independently reports all 13 `spurious_wakes` tests passing.

All ten gate steps pass in 395.5 seconds. The default suite passes 2093 tests, and the slow suite passes 33 tests.

The mutation step reports 22 mutants: 20 caught and 2 unviable. It reports no missed mutant. The earlier failed mutation gate remains in the body.

Gate: `~/botster-sessions/gates/botster-contracts-stage1-gs-r43-spurious-wakes-338008cb-pool-20261011-092004-41960.log`.

The transcript files remain identical to round 1. I inspected the committed JSON without executing it: all 24 suite waits are direct steps.

Each wait has only `wait_wake`, `expect`, and `because` keys. The dispatch change therefore does not change these transcript operations.

The earlier Core comparison evidence remains applicable to these unchanged transcripts. It retains the same four testkit failures and the real-tier results stated in round 1.

The dropped-signal proof retains strict failure on the seeded harness. A harness without the control retains the steward's accepted real-spurious-wake residual.

The eight transcript changes implement the authorized TH-2 widening and same-clock checks. The `tm_3` exception retains exactly the steward's specified event check.

This review does not approve a broader widening, a pending-list removal, or the four unresolved Core results. The next Core pin needs its own review.

I checked the prior-art claim against old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`. Its cited spurious-wake paths concern route writable readiness.

Sol and reviewer 016c must complete their required reviews. The steward owns merge approval. A future base merge is not covered by this exact-head verdict.

I ran no gate, build, test, or executable probe.

### Contracts PR #50: integration completion

Sol publishes integration CLEAN at exact head `338008cbeea0bc1e675680e2d1c2935bf4702383` in `6f98c3cefeceda0e7e28447cd43323517b53138f`.

I read the complete artifact, `verdicts/contracts/gs-r43-spurious-wakes.md`, and verified that `origin/stage1/integration-review-sol` contains the commit.

Sol confirms that all package findings are closed. Sol also records reviewer 016c's exact-head ACCEPT and adds no finding.

The mutation log supplies aggregate counts without individual mutant identities. Neither review claims independent mutant identity matching.

The later #47 import, its exact-head gate, and the Core pin remain separate. I ran no gate, build, test, or executable probe.

### Contracts PR #50: #47 import source review

- Head: `efbb30636b29d506ab04acdb9f7f90031601c674`.
- Base: `f303ccdb9e02c6763d31cd8f7503095084630adc`.
- Previously reviewed head: `338008cbeea0bc1e675680e2d1c2935bf4702383`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package source review).
- Open findings: 0.

I verified both merge parents and read the complete canonical base-check log. Conditions (1), (2), and (3) fail; condition (4) passes.

The conflicting file is `crates/botster-core-conformance/src/driver.rs`. The overlapping documentation file is `docs/core-testkit-controls.md`.

Log: `~/botster-sessions/gates/botster-contracts-stage1-gs-r43-spurious-wakes-efbb3063-base-merge-check.log`.

I compared both complete own diffs after removal of Git blob index lines. The differences are shifted hunk positions, one imported context line, and the wake-handle access.

The moved wait now obtains its owned wake handle through `self.handle(&name)?.core.borrow_mut().wake_handle()`. The removed old-position wait uses that same imported access.

The temporary mutable borrow ends at the assignment statement. The later wait keeps its existing thread selection, timeout, expected value, and failure mapping.

The merged driver keeps one wait branch at the reviewed first-dispatch position. The strict-wait scanner, empty-set rule, direct acknowledgements, and recursive barriers remain unchanged.

The imported shared Core type and route wait code remain in the base. The PR's own delta makes no further change to their behavior.

The documentation context line is the imported R-69 `oracle_resume` refusal text. The `no_spurious_wakes` row remains the reviewed row.

All other own changes remain identical. GS-R43-1 and GS-R43-R3-LOW-1 remain CLOSED.

I verified the live head and base. The body identifies its supplied gate as the historical `338008cb` gate; it is not a gate on this merge head.

The lead requires the final gate after the #49 import. That later head needs its canonical base check and any required source delta review.

This source verdict does not accept a future gate or declare merge readiness. Sol must complete the HIGH integration delta review.

I ran no gate, build, test, base-check script, or executable probe.

Sol publishes integration source CLEAN for `efbb30636b29d506ab04acdb9f7f90031601c674` in `0dc15650f53db09b9278f32e3d36f79b39d49a9d`.

I read the complete delta artifact and verified its published branch. Both source reviews have zero findings. The final import and gate remain due.

### Contracts PR #50: final base merge and gate

- Head: `ddce507e2c08e740f899e97dcfec566f3d2275b8`.
- Base: `6a715f890a95f5ca92c726e5149c2e5adc01261f`.
- Reviewed parent: `efbb30636b29d506ab04acdb9f7f90031601c674`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package review carried by the canonical base check).
- Open findings: 0.

I read the complete canonical base-check log. Every condition passes. The three imported paths are disjoint from the eighteen own paths.

The complete own diff is byte-identical: 61061 bytes, SHA256 `eb6782c17e03d8e2a91f3f32e68c5a52bf8c373318e7e6d8193a6a09573d6134`.

I verified the two merge parents, the live PR head, and the live base. The final body identifies both imports and their review conditions.

Base check: `~/botster-sessions/gates/botster-contracts-stage1-gs-r43-spurious-wakes-ddce507e-base-merge-check.log`.

The final gate names this exact head and base. All ten steps pass in 384.4 seconds. Both job and wrapper exit zero.

The mutation step reports 22 mutants: 20 caught and 2 unviable. This is aggregate evidence; the log supplies no individual mutant identities.

Gate: `~/botster-sessions/gates/botster-contracts-stage1-gs-r43-spurious-wakes-ddce507e-pool-20261011-101430-59112.log`.

The canonical check permits package and integration CLEAN to carry from the reviewed parent. No source delta round is required for this import.

Both findings remain CLOSED. The steward owns merge approval. The later Core pin still requires separate review and evidence.

Sol publishes final integration CLEAN at `ddce507e2c08e740f899e97dcfec566f3d2275b8` in `739adc892c910f3a78e741d7c54ca1ced380a6f4`.

I read the complete final artifact and verified its publication. Both final reviews have zero findings. The steward checks reviewer 016c's additional requirement.

I ran no gate, build, test, base-check script, or executable probe.

## Core PR #278: repository signal scan

- Head: `214ade281c890b05e72054f66b2b34e853204d3c`.
- Base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package review).
- Open findings: 0.

The complete diff changes only tests in `xtask/src/signals.rs`. The scanner, file collector, verdict, gate order, and timeout values remain unchanged.

The old default-tier test parsed the repository again after `taint` had already scanned it. The new tests use a small fixture tree.

The fixture preserves the guard exemption, counts only Rust files, reports the violating file and line, and fails on the same raw call outside the guard.

The cheap repository test preserves the guard-file check and rejects an argument before file collection. The existing alias, lexer, raw-call, and verdict proofs remain selected.

`ci::taint_job` still calls `signals::command(root, &[])`. The gate still runs that job before the default tests.

The live command lists tracked files. The removed test walked the tree without Git. I checked the committed paths: all 228 Rust files lie outside the walk's excluded directories.

For this clean gate snapshot, the repository check therefore retains the relevant Rust file set. Untracked local files do not constitute the merging gate's source tree.

The new fixture tests retain scanner decision coverage under mutation. They check both accepted guard input and rejected ordinary input through `scan_files` and `verdict`.

The removed whole-tree test used the same scanner. It checked a clean repository, not a distinct rejection rule. Its Rust-count check now has exact fixture counts.

The file walker is test-only. Other repository tests still use it. This PR changes no mutation filter, exclusion, selection, binary build, or mutation verdict.

The mutation step finds no mutant in this test-only diff. That result is not an old-versus-new mutation comparison; the source and retained behavior proofs support this review.

### Evidence

I read the retained RC failure. Its old scanner test times out at 2.003 seconds against the unchanged 2-second limit.

Failure: `~/botster-sessions/gates/botster-core-stage1-rc-oracle-controls-ff913e68-pool-20261011-093345-95687.log`.

The exact-head gate passes all ten steps in 313.4 seconds. It scans 228 Rust files in `taint` and passes all ten signal tests.

Each signal test takes at most 0.004 seconds in that log. The default tier passes 1726 tests; the slow tier passes 553 tests.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-repo-scan-214ade28-pool-20261011-094124-54121.log`.

I read the separate planted-call proof, including its command text. It appends a raw call to a tracked Core file and runs the existing `taint` job.

The job reports that file and line, one signal violation, and exit 1. After restoration, it reports exit 0.

Proof: `~/botster-sessions/gates/botster-core-stage1-gs-repo-scan-214ade28-pool-20261011-094827-75167.log`.

The proof wrapper exits zero because its last command prints the restored status. The explicit job statuses, not the wrapper status alone, support the negative proof.

The planted function is scanner input. The proof does not execute its raw signal call.

The live body retains the failed gate, the separate proof, and the unchanged test budget. Other inexpensive repository checks remain present.

I checked old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` for the named scan and budget. It supplies no corresponding xtask mechanism to reuse.

I read pair-common, the current GS brief, and plan revision 23bk, section 8. Sol must complete the HIGH integration review.

Sol publishes integration source CLEAN for this head in `0d05b356210e31488885686953e55fc3ed57b416`, at `verdicts/stage1/gs-repository-signal-scan.md`.

I read the complete artifact and verified its publication. Both source reviews have zero findings. The current-base import and final gate remain required.

### Core PR #278: final base import

- Head: `4b277224e9b7e164bdc8bed2e634601e9c8b2a3f`.
- Base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
- Reviewed parent: `214ade281c890b05e72054f66b2b34e853204d3c`.
- VERDICT: CLEAN (package source review carried by the canonical base check).
- Open findings: 0.

I read the complete canonical base-check log. Every condition passes. None of the 32 imported paths overlaps the one own path.

The own diff is byte-identical: 2929 bytes, SHA256 `f02ec8df7efbf9e7c40774eecd7fd87a770a1dc75c250c433c8e76641d7852a2`.

I verified both merge parents and the live PR head and base. Both pending files retain the base's contents.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-repo-scan-4b277224-base-merge-check.log`.

The canonical check carries package and integration source CLEAN. The implementer reports that the final gate is running; I accept no result before its completion.

This record accepts the source carry-over only. The final exact-head gate remains required. I ran no gate, build, test, or base-check script.

### Core PR #278: final gate completion

VERDICT: CLEAN at `4b277224e9b7e164bdc8bed2e634601e9c8b2a3f`, base `f49f74302526a28b6aced1595f74c7d0a1d22836`. Open findings: 0.

I read the final gate header, summaries, signal-test records, and final exits. All ten steps pass in 135.9 seconds.

The production signal scan checks 232 Rust files. All ten signal tests pass in at most 0.003 seconds.

The default tier passes 1748 tests. The slow tier passes 559 tests. The test-only own diff lists no mutant and changes no decoder crate.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-repo-scan-4b277224-pool-20261011-111756-34635.log`.

The live head and base remain unchanged. The body contains the final canonical check and gate. The lead owns merge approval.

The final gate completes the pending evidence condition. I ran no gate, build, test, or base-check script.

Sol publishes final integration CLEAN in `70ed0fe9fdc7c53200655b772cf66604bd992836`, at `verdicts/stage1/gs-repository-signal-scan.md`.

I read the complete final artifact and verified its publication. Both final reviews have zero findings at `4b277224e9b7e164bdc8bed2e634601e9c8b2a3f`.

Core #278 merges as `23bb03a113218a5af24b44d31635dacad43c5981`. I verified the exact base and reviewed-head parents.

The merge tree equals the accepted head tree, `968f6e533d23625b1ec28026cab8e1e94690aa3e`. This completes the package merge receipt.

The lead requires a current-base merge after both CLEANs. This verdict applies only to the exact head above; it does not accept a future gate.

I ran no gate, build, test, or executable probe.

## Core PR #281: Git failure fixture

- Head: `1f0ab3eb5e8f19b43fed1e34468e86bced9a244b`.
- Base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package review).
- Open findings: 0.

The complete diff changes one test fixture in `xtask/src/mutants_cited/tests.rs`. It changes no production code, environment variable, timeout, mutation exclusion, or verdict.

The old negative fixture relied on a temporary directory having no repository ancestor. The Mac pool placed temporary directories inside a checkout.

The new fixture writes `.git` with a reference to a missing directory inside its owned temporary directory. Git encounters that invalid reference before ancestor discovery.

The referenced directory does not exist. The test does not create it. The owned temporary directory removes the fixture when the test ends.

The test retains its positive assertion: the file collector returns exactly `a/b.rs` and `c.toml` from a staged repository.

The test also retains its negative assertion: the collector returns an error when Git cannot list tracked files. The fixture makes that condition independent of the parent directory.

`tracked_with_submodules` still runs `git ls-files -z --recurse-submodules` and passes the result through the existing failed-command check.

The repair neither removes that check nor changes its error requirement. It therefore does not permit a gate failure or mutant that the previous assertion would reject.

### Evidence

I read the retained Mac failure at the original negative assertion. That log also retains the separate signal-scan and base-merge timeouts.

Failure: `~/botster-sessions/gates/botster-core-stage1-p5-lc2-real-proof-cb7eb0f7-pool-20261011-101307-53515.log`.

The live body reports local before-and-after evidence with `TMPDIR` inside a repository. I did not reproduce or independently inspect that local run.

The exact-head Linux gate passes all ten steps in 291.1 seconds. The repaired test passes in 0.006 seconds.

The default tier passes 1729 tests. The slow tier passes 555 tests. The test-only diff has no listed mutant.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-not-a-repo-1f0ab3eb-pool-20261011-102558-30553.log`.

The body identifies the independent pool fix and retains the Mac cost as separate work. This review accepts no timeout change or unrelated retry.

I checked the prior-art claim at old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`. It supplies no corresponding `mutants_cited` check.

The lead's handoff confirms this test-only assignment under plan revision 23bp. Sol must complete the HIGH integration review.

Sol publishes integration source CLEAN in `0c0e21801d6024159121f0fdad882530b0d83398`, at `verdicts/stage1/gs-git-failure-fixture.md`.

I read the complete artifact and verified its publication. Both source reviews have zero findings. Sol records the lead's final-slot rule under revision 23bu.

The implementer must wait for that slot before the final import, check, and gate. This receipt accepts no future head or gate.

### Core PR #281: final base import

- Head: `d7abc589391ed492e27413c05c8489c262377f36`.
- Base: `23bb03a113218a5af24b44d31635dacad43c5981`.
- Reviewed parent: `1f0ab3eb5e8f19b43fed1e34468e86bced9a244b`.
- VERDICT: CLEAN (package source review carried by the canonical base check).
- Open findings: 0.

I read the complete canonical check. Every condition passes. The 28 imported paths are disjoint from the one own path.

The own diff is byte-identical: 1033 bytes, SHA256 `a161c7001ea17f6e8a61a00cf17e9b7810eb1e24f38123e42c1cf1cb8a2f6f9f`.

I verified both parents and the live PR head and base. Both pending files retain the base's contents.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-not-a-repo-d7abc589-base-merge-check.log`.

The canonical check carries both source CLEANs. The implementer reports a running final gate under the lead's slot.

The final exact-head gate remains pending. This record accepts no result before completion. I ran no gate, build, test, or base-check script.

### Core PR #281: final gate completion

VERDICT: CLEAN at `d7abc589391ed492e27413c05c8489c262377f36`, base `23bb03a113218a5af24b44d31635dacad43c5981`. Open findings: 0.

I read the final gate header, summaries, repaired-test record, and final exits. All ten steps pass; their summary total is 412.8 seconds.

The job runs for 520 seconds and queues for 8 seconds. The job and wrapper exit zero after 528 seconds.

The repaired test passes in 0.008 seconds. The default tier passes 1748 tests, and the slow tier passes 559 tests.

The test-only own diff lists no mutant and changes no decoder crate. The log contains no FLAKY token.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-not-a-repo-d7abc589-pool-20261011-112304-56033.log`.

The live head and base remain unchanged. The final body contains the canonical check and this gate. The lead owns merge approval.

The final gate completes the pending evidence condition. I ran no gate, build, test, or base-check script.

Sol publishes final integration CLEAN in `e57f39aa1014c7d1311f1f1230116f747f2701b2`, at `verdicts/stage1/gs-git-failure-fixture.md`.

I read the complete final artifact and verified its publication. Both final reviews have zero findings at `d7abc589391ed492e27413c05c8489c262377f36`.

Core #281 merges as `926dfb62f3f7168990b2a48741946932d84b2fbb`. I verified the exact base and reviewed-head parents.

The merge tree equals the accepted head tree, `0cfed7dc43a101334878bdcf4dc61fbb010e4e29`. This completes the package merge receipt.

This verdict applies only to the exact head above. Any later base merge needs the canonical check and its required final gate.

I ran no gate, build, test, or executable probe.

### Core PR #286: budget-held full seeds, round 1

- Head: `34b4773479cfc36b205b1ea9a0d48c88fe4eb6e4`.
- Base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
- Risk tier: HIGH.
- VERDICT: NOT CLEAN.
- Open findings: 1 MEDIUM.

**GS-HF-R1-1 MEDIUM: the required seed-specific negative proof is missing.**

Design section 8 requires a pool evidence job with a test-only held id that fails at seed 1 only. The lists run must pass. The held-full run must fail and name seed 1.

The supplied positive runs report 32 ignored trials and zero failures. The body reports a local negative change that fails at all four selected seeds.

Neither result proves that each trial executes its named seed. A trial loop that always executes seed 0 could pass those evidence checks.

Provide the design's negative proof on the reviewed source. Retain its command, source change, lists result, held-full result, failure seed, and exit status.

This finding concerns evidence. I found no demonstrated regression in the current gate verdict from the reviewed source.

The new command adds the full-seed testkit run after the existing slow runs. It clears the three selection variables and uses the unchanged 600-second bound.

The existing process group, tracker, and leftover check surround the command. The budget proof text is required when the held file parses.

The gate compares the report's budget count with the held file. It compares the seed count with the CI seed set. Missing reports and failed commands fail the step.

The lists run and its first-seed rule remain unchanged. The ordinary pending result remains ignored. A held id must still pass.

The mutation stages remain unchanged. Stage 2 does not select held-full. Thus, the new run supplies candidate coverage, but no mutant coverage at seeds 1-31.

The candidate binaries and the mutation build paths retain their existing rules. This review does not extend the earlier approval of those rules.

The new exclusions cover four mutants each in the two command shells. The pure report, seed-count, budget-filter, and verdict decisions remain subject to mutation tests.

The exact-head gate passes all ten steps. Its step total is 376.0 seconds; the job and wrapper exit zero after 403 seconds.

The default tier passes 1752 tests. The slow tier passes 559 tests. Held-full reports one budget id at 32 seeds and takes 9.64 seconds.

The mutation step reports 23 mutants: 21 caught and 2 unviable, with no missed or timed-out mutant. Stage 2 has no remaining mutant.

The timing job reports 9.64 and 9.65 seconds with four threads, versus 38.54 and 38.53 seconds with one thread.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-34b47734-pool-20261011-114751-62573.log`.

Timing: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-34b47734-pool-20261011-114445-56512.log`.

Authority: `~/botster-sessions/shared/core-stage1/design-gs-budget-held-slow-step.md`, section 8, and plan revision 23by.

I read all six changed files. The prior-art check at old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` supplies no corresponding held-full run.

Sol must complete the HIGH integration review. I ran no gate, build, test, base-check script, or executable probe.

Sol records integration NOT CLEAN in local commit `1b4b77ada856b1b985399d7e1d11dbdb390320e3`, at `verdicts/stage1/gs-held-full.md`.

I read the complete artifact. HF-SI-1 MEDIUM agrees with GS-HF-R1-1 and requires the same seed-1-only negative proof.

Both reviews have one open evidence finding at the exact head above. Sol found no demonstrated source regression.

Sol's push also failed because GitHub DNS resolution failed. This receipt confirms a local artifact, not publication.

### Core PR #286: seed-specific proof, round 2

- Head: `34b4773479cfc36b205b1ea9a0d48c88fe4eb6e4`.
- Base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
- VERDICT: CLEAN (package source and evidence review).
- Open findings: 0.

GS-HF-R1-1 MEDIUM is closed. I read the complete evidence note and the commands, identities, results, and exits in both pool logs.

Evidence note: `~/botster-sessions/shared/core-stage1/evidence/gs-held-full/seed1-negative-proof.md`.

The scratch change sets `max_polls = 0` only when ev_8 runs with the single seed 1. It changes no other run.

Git confirms that scratch snapshot `421a8f76118a042b6e0b5c91b5fd32edaaa76a57` differs from the reviewed head only by that change.

The second snapshot, `c37d70d4206547b9f2dc156ceee7fe26c9b5fdbe`, has an identical tree. Thus, both jobs use the same fault.

Held-full fails only `conf::ev_8_every_query_stays_in_output@seed_1`. The error names seed 1, step 5, and `limit=max_polls`.

The run reports one failed trial and 31 ignored trials. It reports one budget id at 32 seeds. The slow step fails and exits 1.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-34b47734-pool-20261011-151838-69247.log`.

The first job's lists result is invalid for this proof. Its later base causes a list-diff failure before the strict run starts.

The second job explicitly supplies the reviewed base. The lists output confirms that base, reports zero failures in both runs, and exits 0.

The strict report names 363 of 381 pending ids, 18 real-only ids not run, and two held ids. Lists passes in 11.5 seconds.

Log: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-34b47734-pool-20261011-152342-380.log`.

The shell prints each step's exit status. Its final echo makes the job wrapper exit zero even when slow fails. I use the step status, not the wrapper status, for this proof.

These are scratch evidence jobs, not merge gates. They complete the required negative proof and do not replace the earlier exact-head green gate.

The live PR retains the reviewed head and base. The implementer reports restoration of the scratch file and a clean tree.

Sol must close HF-SI-1 independently. The lead's final-slot rule still applies before a current-base import and final gate.

This CLEAN applies only to the exact head above. I ran no gate, build, test, base-check script, or executable probe.

Sol records integration CLEAN in `15b1dbe3138477202aee50d67969638325d27105`, at `verdicts/stage1/gs-held-full.md`, round 2.

I read the complete artifact and verified its publication. Sol independently closes HF-SI-1 with the same seed-specific proof.

Sol retains the invalid first lists result.

Both source and evidence reviews have zero findings at `34b4773479cfc36b205b1ea9a0d48c88fe4eb6e4`, against base `926dfb62f3f7168990b2a48741946932d84b2fbb`.

The final slot, current-base import, required check, and exact-head final gate remain due. This receipt accepts no later head or merge.

### Core PR #293: B13 build-root check, round 1

- Head: `2c0488b8d8f55e3ad05992f97b4de5ca95b9b9c3`.
- Reviewed base: `5e953398925351c12e463a7b0a946b79d9841edc`.
- Risk tier: HIGH.
- VERDICT: NOT CLEAN (3 open: 2 HIGH, 1 LOW).

**GS-B13-R1-1 HIGH: prebuild can label another worktree's artifacts with the current root.**

`xtask/src/prebuild.rs:151` converts every record-read error to `None`. `other_source_root` accepts `None`. Line 161 writes the current root before the builds.

A normal worker build in worktree A creates no candidate root record. Worktree B can then run prebuild against the same target directory.

Cargo can reuse A's worker. Prebuild copies that worker, records B as its root, and hashes the copied bytes.

`Candidate::locate` accepts the B record and those hashes. Neither establishes that B supplied the worker's source. This path remains a B13 false-result risk.

This is a static source finding. I did not execute this case. The supplied evidence shows Cargo reusing artifacts across worktrees, but tests only an existing A record for prebuild.

Required change: establish the binaries' build roots before accepting or writing the candidate root. Refuse unknown origin in a populated target directory.

Do not treat an unreadable record as a safe first build. Preserve the lead's rule against an automatic clean.

Required evidence: build the worker in A without prebuild. Then run prebuild in B with the same target directory and lockfile.

Show refusal or verified B artifacts. Show that candidate validation cannot accept A's worker as B's. Preserve a valid first build in an empty target directory.

**GS-B13-R1-2 HIGH: the coverage check accepts a root-check function that no test runner executes.**

`xtask/src/built_in.rs:99` accepts the root when two raw substrings occur: `TEST_FN` and `TEST_CALL`.

Removing `#[test]` leaves both substrings present. Commenting out the function also leaves both substrings present. The coverage check still reports a test.

The same check does not reject an ignored test or a test excluded from its required tier. Thus, the mechanical guard can pass without the required build-root proof.

Required change: validate a registered test that the required tier selects. Validate that this test calls the root check. Reject unsupported source forms.

Required evidence: negative fixtures for a missing `#[test]`, a commented-out function, and a test excluded from its required tier.

Preserve the valid library and binary forms. Each negative fixture must fail the coverage check.

**GS-B13-R1-3 LOW: the evidence overstates the compilation count.**

The evidence script's `show` function counts `Compiling botster-core` lines. Its label and the PR body call that count all workspace crates.

That pattern does not count `botster-worker-core`, `botster-test-process`, or `botster-terminal-ghostty`. The displayed zero therefore does not prove zero compilations for every workspace crate.

Required change: narrow the label and body claim to the measured names, or supply a complete compilation record and count every workspace package.

The existing result still supports reuse of the matching Core crates. This finding does not invalidate the observed conformance refusal.

I read all 27 changed files, the approved design, the lead's approval, and the current pair rules.

The conformance entry point checks itself and nine linked libraries before argument parsing. Both conformance binaries take this entry point, including their list operation.

The root comparison has one implementation in `botster-test-process`. It resolves symbolic links before comparison. A missing runtime manifest variable causes refusal.

Each current library exports its build-root constant. Each current crate has the added unit test. The dependency additions are dev-dependencies, except the testkit's now-required test-process dependency.

The new taint command checks package source and the transitive linked list. It reads the current workspace layout. The two HIGH findings prevent acceptance of its safety claim.

No timeout, seed, budget, or mutation-stage selection changes. Both mutation stages retain their in-place builds. This PR adds no mutation exclusion.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-2c0488b8-pool-20261011-155212-66916.log`.

All ten steps pass in 731.1 seconds. The job runs for 808 seconds and exits zero after 809 seconds. The outer wrapper exits zero after 810 seconds.

The default tier passes 1820 tests. The slow tier passes 584 tests. Mutation testing reports 44 mutants: 41 caught and three unviable.

No missed mutant requires stage 2. The public API check passes. The taint output reports 12 crates and nine linked libraries.

B13 evidence: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-2c0488b8-pool-20261011-161823-85653.log`.

At the base, B on A's target passes 230 tests and exits zero. B's own build fails the un-pended a11_1 id and exits 101.

At the head, the reused conformance binary refuses and exits 101. Prebuild in B refuses an existing A root record and exits 1.

The evidence script prints these statuses. Its final cleanup makes the wrapper exit zero. The step statuses establish the negative outcomes.

The body retains two earlier failed evidence attempts and excludes them from its proof. I accept no result from those attempts.

The prior-art check reads old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`, including `script/prebuild-worker`.

That script records source revision and binary hashes. It supplies no build-root check for shared artifacts. The current prebuild replacement method was already present in the base.

Sol-B independently confirms both HIGH gaps in `0540b54ce71284651476e1f4e4a265ae17f3163b`, at `verdicts/stage1/gs-b13-built-in.md`.

I read the complete integration artifact. Its two findings agree with GS-B13-R1-1 and GS-B13-R1-2.

The live head remains unchanged. The live base has advanced to `f6025aed4cdf7da836e835e0040fc128b7e8f32f`. This review covers only the stated reviewed base.

Every finding must close before CLEAN. I ran no gate, build, test, base-check script, or executable probe.

Sol-B records all three matching findings in supplement `7fd19ea52d09f5c7fe964bd61339168a7471885b`.

I read the supplement. It corrects the compilation-count claim and retains NOT CLEAN with two HIGH findings and one LOW finding.

The reviewed head and base remain unchanged. This receipt starts no new source review. Wait for the repaired-head READY.

### Core PR #286: final-slot import, delta review

- Head: `ac457ca6a83731e68e4fc44ad3551b8ec39e3612`.
- Base: `9053b1df868bbec2d62977660f44907e73323c55`.
- Reviewed parent: `34b4773479cfc36b205b1ea9a0d48c88fe4eb6e4`.
- VERDICT: NOT CLEAN (1 LOW open; source delta has no finding).

**GS-HF-R3-1 LOW: the live body describes the imported budget id as future work.**

The body says `#284 is open` and describes a future budget line. The imported base already contains that line, and the merged held-full run selects both budget ids.

Required change: describe the imported `dp_5` line as current scope. Identify the final head and base. Keep earlier one-id evidence clearly tied to its earlier head.

Add the final gate result when it completes. Do not describe a running gate as passed.

The lead's handoff confirms SLOT authorization. I verified both merge parents and the live head and base.

The canonical check fails conditions (1), (2), and (3) on `conformance/core-held.txt`. Both pending-file conditions pass. I read the complete check and reviewed the conflict resolution.

Check: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-ac457ca6-base-merge-check.log`.

After removing blob-index and hunk-location metadata, the other five own patches are identical to the reviewed patches. The only own delta is the held-file text.

The merged ev_8 line retains the base's updated authority and step-14 cost explanation. Its former missing-proof sentence becomes the held-full proof location.

The merged dp_5 line retains the base's owner, authority, and step-18 cost explanation. Its missing-proof sentence becomes the same held-full proof location.

The a20_1 unproven line remains unchanged. The header retains the reviewed held-full rule. No id or class changes relative to the imported base.

I verified all four id/class sets from Git. The merged set equals the imported base: ev_8/budget, dp_5/budget, and a20_1/unproven.

The old head changes no id. The added dp_5 id comes only from the imported base.

The merged pending, real-pending, ledger, and real-only files equal the imported base byte for byte.

The unchanged generic budget filter selects both budget ids. The report check expects two ids and the same 32 seeds. The source retains its failure rules.

The implementer reports a clean local build and a 64-trial held-full run. I did not independently inspect that local run.

The source delta has zero findings. The LOW body finding remains open. Sol must complete the HIGH delta review, and the exact-head final gate remains pending.

I ran no gate, build, test, base-check script, or executable probe.

### Core PR #286: final-import body correction

VERDICT: CLEAN (package source delta) at `ac457ca6a83731e68e4fc44ad3551b8ec39e3612`, base `9053b1df868bbec2d62977660f44907e73323c55`. Open findings: 0.

GS-HF-R3-1 LOW is closed. I read the complete live body and verified the unchanged head and base.

The new final-head section names both budget ids, the conflict resolution, and the three-way id/class sets. It identifies #284 as imported scope.

The body ties the earlier one-id evidence to `34b47734`. It describes the final gate as running and claims no completed result.

The package source delta has no remaining finding. Sol's delta review and the exact-head final gate remain pending.

This CLEAN accepts the source delta only. It does not approve a merge before the required final evidence completes. I ran no execution job.

Sol publishes delta CLEAN in `e72d7d01dc22a5eac71d57ea2e3f63c53fa32028`, at `verdicts/stage1/gs-held-full.md`, round 3.

I read the complete round and verified publication. Both source delta reviews have zero findings on the same exact head and base.

The final gate, final body, and final evidence acceptance remain pending. This receipt accepts no completed result from a running gate.

### Core PR #293: round 2 repairs and imported base

- Round 2 head: `d4758482eb0aaabe6a6053573e797d81343cddfb`.
- Imported base: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
- VERDICT: NOT CLEAN (1 HIGH open).

GS-B13-R1-1 HIGH is closed. The lead approved a build directory for each canonical worktree root.

`candidate_build_dir` hashes that root into a directory under `candidate-build`. Worker and anchor commands explicitly build there and copy from there.

The candidate record now follows all binary replacements. The prebuild command performs no automatic clean and no longer trusts an absent record as artifact provenance.

I read the complete bootstrap evidence command and its results. A first builds the worker without prebuild. B then runs prebuild with the shared target.

At the old head, B writes its root record but copies a worker containing A's path and no B path. This reproduces the reported gap.

At the repaired head, the log names a worker compilation in B. The copied binary contains B's path and no A path.

Evidence: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-58544635-pool-20261011-163216-95056.log`.

The later import and repair preserve this candidate-build code. Candidate validation still refuses another root or a missing record.

The two Mac evidence jobs use slot-2 and its own target directory. The second prebuild succeeds with the same root record.

Mac logs: `...-2c0488b8-pool-20261011-162513-28603.log` and `...-58544635-pool-20261011-164100-59441.log`, in `~/botster-sessions/gates/`.

The shared disk rule now counts each local candidate-build directory against the 8 GiB bound. These records satisfy the lead's stated conditions.

GS-B13-R1-3 LOW is closed. The new evidence counts every compilation line by worktree path. It lists all nine linked workspace package names.

A compiles nine packages. B on A's target compiles zero and incorrectly passes at old v1. B's own target compiles nine and fails the newly active case.

The repaired conformance binary refuses the reused build with exit 101. The body replaces the earlier incomplete count and retains its limitation.

GS-B13-R1-2 HIGH remains open at d4758482. The test index now correctly checks registration and both default tiers.

However, `root_test_calls` visits every syntax path. A reference to `built_in::assert_built_here` satisfies it without executing the function.

An uncalled nested function also satisfies it. Sol-B independently records these cases in `7277bb6eff032a1a2a7e486f52fb8ee834ebc3bf`.

I read the complete integration round. Its closures and remaining HIGH finding agree with this package review.

I read the complete canonical merge check. It fails conditions (2) and (3) on the testkit manifest and library root. Conditions (1) and (4) pass.

Check: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-96c58d88-base-merge-check.log`.

I verified the merge parents and every imported non-own blob. Each equals the imported base. Every own patch matches after removing index and hunk-location metadata.

The base's rustix `net` feature and `stream_holders` module remain. The PR's dependency, re-export, constant, and root test remain on separate lines.

The later d4758482 commit removes an unused slow-feature decision. The coverage rule still requires the default tier on Linux and macOS.

That commit also excludes five whole-body replacements of `build_worker`. The expression covers only that I/O shell. Its worker-presence and candidate-directory decisions retain tests.

The bootstrap job exercises the worker build and copy path. The new exclusion follows the existing anchor-build exclusion and supplies no exemption for its pure decisions.

The failed 96c58d88 gate retains seven misses: two unused slow-feature mutants and five worker-shell replacements. It also retains two stage-2 FLAKY outcomes.

The source repair removes the unused decision and adds the bounded shell exemption. It does not convert those failed records to catches.

The d4758482 gate passes all ten steps in 507.7 seconds. Default: 1835 passed. Slow: 597 passed. Mutation result: 61 mutants, 58 caught, three unviable.

The job exits zero after 516 seconds; the wrapper exits zero after 518 seconds. No remaining mutant requires stage 2.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-d4758482-pool-20261011-175859-18261.log`.

Its comparison base is `f7eeced64e652700f2f78fc063e4896832134233`. The imported base remains `3aacc58d086e55a6b186165ccf9001e2618e2869`. This gate proves no later import.

### Core PR #293: one-statement repair delta

- Head: `836e879c40be7b0ac97b1bc8a2f4883eaffbd3ea`.
- Base: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
- VERDICT: NOT CLEAN (1 HIGH open: GS-B13-R1-2).

I read the complete one-file delta and verified the live head and base. The repair rejects unused references, nested functions, closures, early returns, conditional blocks, and methods.

The supported form is now one statement containing a call expression. However, the matcher ignores the call expression's attributes.

This registered test passes the source check but executes no build-root check:

```rust
#[cfg(test)]
#[test]
fn this_test_binary_was_built_in_this_worktree() {
    #[cfg(any())]
    botster_test_process::built_in::assert_built_here(env!("CARGO_PKG_NAME"), BUILT_IN);
}
```

The test index selects the function in both default tiers. The parsed body contains one call with the expected path.

Rust removes that call because `cfg(any())` is false. The test then succeeds without checking its build root.

Required change: reject attributes on the supported call statement, or verify that those attributes preserve execution in both required tiers.

Reject unsupported forms. Add a negative fixture for the conditional call above. Preserve the current direct calls without attributes.

Sol-B independently reports the same attribute gap. The gate on this head is running. I read no running output and accept no completed result.

The candidate and evidence findings remain closed. The coverage finding remains HIGH. I ran no gate, build, test, base-check script, or executable probe.

### Core PR #293: call-attribute repair delta

- Head: `fec71f93d39977cf71f7787a86b18e6e6cb2dfce`.
- Base: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
- VERDICT: NOT CLEAN (1 HIGH open: GS-B13-R1-2).

I read the complete one-file delta. It rejects attributes on the call and its path. It rejects inner attributes of the test body.

Three new negative fixtures cover those attributes. The earlier conditional-call example is repaired.

The matcher still ignores `call.args`. This supported-looking body compares the runtime root with itself:

```rust
#[cfg(test)]
#[test]
fn this_test_binary_was_built_in_this_worktree() {
    botster_test_process::built_in::assert_built_here(
        env!("CARGO_PKG_NAME"),
        &std::env::var("CARGO_MANIFEST_DIR").unwrap(),
    );
}
```

The function is registered and selected in both default tiers. Its body contains exactly one call without attributes and with the expected path suffix.

`assert_built_here` receives the runtime manifest directory as the built directory. Its comparison succeeds even when the test binary came from another worktree.

Thus, the coverage command can accept a test that cannot detect B13. The build-root constant can remain in the file, unused by that test.

Required change: validate the supported call's compile-time root argument. Accept the current `BUILT_IN` or `env!("CARGO_MANIFEST_DIR")` forms. Reject unsupported argument expressions.

Add a negative fixture for the runtime-root argument. Reject an early return inside an argument, which also prevents the root check from executing.

Validate the supported callee paths. An unrelated function with the same path suffix is not the build-root check.

Preserve the current root-test forms. The candidate and compilation-evidence findings remain closed. The gate on this head is running; I accept no result before completion.

I ran no gate, build, test, base-check script, or executable probe.

### Core PR #293: lead's split decision and retained flakes

The lead directs a split in message `msg_plugin-w_1791767795_5594a0`. PR #293 retains the closed B13 repair and removes the coverage visitor.

The coverage guarantee transfers to a new item. That item requires a structural design note before implementation. The exact split head remains pending.

This scope decision does not assert that an earlier defective visitor is safe. I will verify its removal on the submitted split head.

**GS-B13-R2-1 MEDIUM: the in_8 FLAKY results lack a recorded structural-fix disposition.**

The lead explicitly keeps this requirement in PR #293's scope. Sol-B tracks the same requirement as B13-I4.

The retained 96c58d88 gate names two stage-2 FLAKY results at lines 6345 and 6346. Both concern `conf::in_8_mode_checked_at_write_start_by_worker`.

Their mutants replace `build_worker` with `Ok(Some(("xyzzy".into(), String::new())))` and `Ok(Some(("xyzzy".into(), "xyzzy".into())))`.

Both fail once and pass on their permitted retry. The gate correctly counts them as MISSED. The failed gate remains retained.

BUILD.md Testing rule 9 requires every FLAKY result to open a structural fix. I found no accepted in_8 disposition in the shared handoffs or steward notes.

Required change: retain the detailed failures and record the named structural fix and its owner. Record any required lead decision. Do not relabel either result as a catch.

The source repair and shell exclusion do not close the test's flakiness. A later green gate alone does not supply this disposition.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-96c58d88-pool-20261011-173013-11653.log`.

Its failure artifacts are `/home/structvr/testq/projects/botster-core/artifacts/jobq-botster-core-96c58d88-20261011173013-3033`, kept seven days.

I sent the retained log, exact mutant names, and artifact path to GS and Sol-B. I have no local copy of the detailed mutant failures.

The split head and this disposition require review before CLEAN. I ran no execution job.

### Core PR #286: completed final gate and merge receipt

VERDICT: CLEAN at `ac457ca6a83731e68e4fc44ad3551b8ec39e3612`, base `9053b1df868bbec2d62977660f44907e73323c55`. Open findings: 0.

The shared handoff identifies the completed gate. I read its identity, completed summaries, held-full result, mutation result, and exits.

All ten steps pass in 571.1 seconds. The job runs for 682 seconds and exits zero. The outer wrapper exits zero after 683 seconds.

The default tier passes 1832 tests. The slow tier passes 599 tests. Held-full runs 64 trials for two budget ids at 32 seeds.

Held-full reports zero failures and 64 ignored trials in 19.32 seconds. Mutation testing reports 23 mutants: 21 caught and two unviable.

No remaining mutant requires stage 2. The log contains no FLAKY token. Fuzz selects no changed decoder crate.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-held-full-ac457ca6-pool-20261011-171824-68670.log`.

I read the complete final body. It records the completed exact-head gate and both budget ids. The earlier evidence retains its original head scope.

GitHub confirms merge `b0c0a419af27259006b5e06d4e5da3b3d00083e6`. I verified its exact base and reviewed-head parents.

The merge tree equals the reviewed head tree, `98bb9f0393e4b63201927920a55d5d32c9cbdcd4`.

This completes the package evidence and merge receipt. Sol's earlier source delta receipt remains recorded separately. I ran no gate or execution job.

Sol publishes final evidence and merge CLEAN in `0a1dab2f11ec34e7edbf4503f3eed3c0c5915607`, at `verdicts/stage1/gs-held-full.md`, round 4.

I read the complete round and verified publication. Sol matches all 21 raw catches to their citations and normal passing tests.

Both final reviews have zero findings at the same exact head and base. Both confirm that merge `b0c0a419` retains the gated tree.

This completes the integration receipt for #286. I ran no execution job.

### Core PR #293: in_8 disposition receipt

GS-B13-R2-1 MEDIUM is closed. The exact split head still requires review before CLEAN.

I read `~/botster-sessions/shared/core-stage1/evidence/gs-b13-in8-flaky/disposition.md` and the lead's recorded assignment.

The lead assigns PC the rule-9 diagnosis and in_8 fence work after #77. The proposed mechanism concerns the R-74 held-write form. No completed fix is claimed.

I inspected the copied stage-2 logs under the note's `artifacts/jobq-botster-core-96c58d88-20261011173013-3033/` directory.

The files `target-mutants-stage2-mutants.out/log/xtask__src__prebuild.rs_line_95_col_5_003.log` and `_004.log` name the two exact mutants.

Each first attempt fails with `INCONCLUSIVE`, seed 0, step 8, and `limit=step_timeout`. Each second attempt passes. Both logs retain their FLAKY summaries.

The plain-v1 record also fails in_8 at step 8 with `step_timeout`, at seed 5. It does not contain this PR's source change.

Plain-v1 log: `~/botster-sessions/gates/botster-core-detached-5e953398-5e953398-pool-20261011-160013-7403.log`, line 1961.

The disposition correctly distinguishes the normal gate from mutation stage 2. A normal prebuild using either replacement writes no worker entry and candidate verification refuses it.

The candidate unit test covers a missing worker entry. Stage 2 instead uses the unmutated candidate built earlier, so it cannot exercise the mutated prebuild shell.

I verified that source path. The later whole-body exclusion retains its I/O-shell reason. It supplies no catch of these two historical mutants.

Both historical mutants remain MISSED. The structural fix is open under PC. The id remains live and is not re-pended.

This receipt accepts the required retained evidence and owned disposition. It does not assert that PC completed the fix. I ran no execution job.

### Core PR #293: lead-approved split, round 3

- Head: `652d0034a0a1da4145723b1b421c9b2c28de0239`.
- Imported base: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
- VERDICT: CLEAN (package source and supplied evidence).
- Open findings in retained scope: 0.

GS-B13-R1-2 transfers to the lead's separate coverage item. I verified removal of `xtask/src/built_in.rs`, its command, help text, test-dispatch entry, and taint call.

The split restores `xtask/src/ci.rs` and `xtask/src/mutants_cited.rs` byte for byte to the imported base. The latter's tier helper is private again.

The removed visitor's defects are not accepted. The new item requires a structural design note before implementation. This PR supplies no mechanical coverage guarantee.

The split retains the manually reviewed build-root constant and direct root-check test in each current crate. The conformance entry point retains itself and nine linked libraries.

I verified the actual calls in all twelve crate roots. They pass each crate's compile-time root, not its runtime root, as the built directory.

The source comparison confirms that retained implementation files are unchanged from d4758482. Only the suite's coverage comment and xtask's removed command wiring differ.

The candidate root check and per-worktree worker/anchor build directory retain their reviewed rules. The other candidate and compilation-evidence findings remain closed.

GS-B13-R2-1 remains closed by the retained failure logs and PC's assigned structural fix. No historical FLAKY result becomes a catch.

All conformance files equal the imported base. The earlier explicit delta review of the overlapping testkit paths remains applicable.

I read the complete revised body. It states the split scope, transferred coverage item, retained B13 checks, and owned in_8 disposition.

GitHub confirms the exact head and imported base. The supplied gate uses comparison base `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035` and reports merge base `3aacc58d086e55a6b186165ccf9001e2618e2869`.

I read the gate identity, completed step summaries, mutation identities and citations, and exits. All ten steps pass in 438.0 seconds.

The default tier passes 1826 tests. The slow tier passes 597 tests. Stage 1 reports 15 mutants: 13 caught, zero missed, zero timed out, and two unviable.

All thirteen raw catches have matching citations. Stage 2 has no remaining mutant. The log contains no FLAKY token.

The prebuild step writes all three candidate binaries. The job exits zero after 452 seconds; the outer wrapper exits zero after 453 seconds.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-652d0034-pool-20261011-192108-45058.log`.

The whole-body worker-shell exclusion retains its reviewed count of five. The pure candidate-directory decision remains mutated and caught.

The twelve root tests and runtime checks add no timeout, budget, seed, or verdict exemption. No current-base merge is implied by the gate's comparison base.

Sol-B must publish the same-head integration verdict. Then FINAL-READY and the lead's SLOT precede the current-base import, required check, and exact-head final gate.

This CLEAN applies only to the exact split head and retained scope. It approves no future import or coverage design. I ran no execution job.

### Core PR #293: final-slot import delta

- Head: `029a740bc33bc7ddea76bb03a0dfdd3f4f584f2d`.
- Base: `e4f495040a80b524a7ac9accba95272d5c2594fa`.
- Reviewed parent: `652d0034a0a1da4145723b1b421c9b2c28de0239`.
- VERDICT: CLEAN (package source delta).
- Open findings in retained scope: 0.

The lead's handoff confirms SLOT authorization. I verified the merge parents and the live PR head and base.

I read the complete canonical check. Ancestry, condition (1), and both pending-file conditions pass. Conditions (2) and (3) fail on shared paths.

The shared paths are `.cargo/mutants.toml`, `Cargo.lock`, and `tests/suite/mod.rs`. This record supplies the required explicit delta review; it does not relabel the canonical result.

Check: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-029a740b-base-merge-check.log`.

I compared all 26 own patches. Every patch is identical after removing only blob-index and hunk-location metadata.

Every imported non-own blob equals the imported base. Every conformance file also equals that base. The PR adds no conformance id or list change.

I read the base's three overlapping deltas. The mutation file imports the already-reviewed held-full shell reasons and exclusions. It preserves this PR's worker-shell exclusion.

The lockfile imports the contracts pin from v0.1.39 to v0.1.41. The PR's workspace dependency additions remain unchanged.

The suite imports held-full. Its B13 `check_here(&LINKED)` remains the first operation, before arguments, list processing, and held-full dispatch.

The nine linked workspace libraries are identical before and after the import. I derived both sets from the manifests. No new LINKED entry is required.

The per-root worker and anchor build paths, candidate record, runtime checks, and crate root tests retain their approved source.

The coverage visitor remains absent. Its finding stays transferred to the lead's separate design item. The in_8 disposition remains closed in this PR's scope.

The earlier source, bootstrap, compilation-count, Mac, and disk evidence retains its exact earlier-head scope. It is not new execution evidence for this import.

The live body still records the earlier reviewed heads and results. The completed exact-head final gate and final body remain due.

Sol-B must publish its same-head delta verdict. I read no running gate output and ran no execution job.

### Core PR #293: completed final evidence

VERDICT: CLEAN at `029a740bc33bc7ddea76bb03a0dfdd3f4f584f2d`, base `e4f495040a80b524a7ac9accba95272d5c2594fa`. Open retained findings: 0.

I read the final gate identity, completed summaries, mutation records, and exits. Its lists check names the exact base and the same merge base.

All ten steps pass in 783.9 seconds. The job and outer wrapper exit zero after 879 seconds, with no queue time.

The default tier passes 1911 tests. The slow tier passes 613 tests. Prebuild writes all three candidate binaries.

The imported held-full run reports two budget ids at 32 seeds. The B13 root check runs before this dispatch, as verified in the source delta.

Mutation testing reports 15 mutants: 13 caught, zero missed, zero timed out, and two unviable. Stage 2 has no remaining mutant.

I matched all thirteen unique raw catches to all thirteen citations. Every cited test has a normal PASS before mutation.

The two unviable mutants provide no catch. The complete log contains no FLAKY token.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-b13-built-in-029a740b-pool-20261011-202653-13678.log`.

I read the complete final body. It records the exact final head and base, the canonical failure and delta reviews, the completed gate, and the retained scope.

The prior failed gates and the in_8 disposition retain their earlier-head scope. The coverage guarantee remains assigned to the separate design item.

Sol-B publishes source delta CLEAN in `6a9b93c0ba29a4be54e1241986d7ecefe4918ec8`, at `verdicts/stage1/gs-b13-built-in.md`, round 4.

I read the complete round. It agrees with the package source review on the overlapping paths, retained B13 code, transferred coverage finding, and owned flake disposition.

GitHub confirms the unchanged head and base. The PR is open at this read. Sol-B's final evidence receipt and the lead's merge remain pending.

This completes the package review of final source, evidence, and body. It accepts no later head or merge tree. I ran no execution job.

### Core PR #293: final integration and merge receipt

Sol-B publishes final CLEAN in `b6b925931105752a8a194f14db67119ed0b5ea62`, at `verdicts/stage1/gs-b13-built-in.md`.

I read the complete final acceptance section and verified publication. Both final reviews have zero findings in the retained scope on the same exact head and base.

The lead reports Q7 complete. GitHub confirms merge `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.

I verified its parents: base `e4f495040a80b524a7ac9accba95272d5c2594fa` and reviewed head `029a740bc33bc7ddea76bb03a0dfdd3f4f584f2d`.

The merge tree equals the reviewed and gated tree, `f2c6afb1e85a47a8fd59112fd6bfd16c1c4539a9`.

This completes the package integration and merge receipt. The separate coverage design and PC's structural fix remain separate owned work. I ran no execution job.

### Core PR #304: DP-2 named real proofs

- Head: `37e5ee32d326f588676b6fc727ad52c86022de61`.
- Base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
- Risk tier: HIGH.
- VERDICT: CLEAN (package source and supplied evidence).
- Open findings: 0.

I read the approved option-A design, its clause map, all four changed files, the affected helpers and test bodies, and the complete PR body.

I checked DP-2 and R-65 at the pinned contracts commit. The proof mapping retains the worker-receipt requirement before the host closes its copy.

The PR removes only dp_2 from core-pending and adds three named proof lines. It retains the real-only class and changes no real-pending entry.

The first facade proof drops the worker's copy before acknowledgement. The client's write succeeds while the host still holds its copy.

After acknowledgement, a later capture's completion proves that the host processed the link. The client observes close and BrokenPipe, as the R-65 proof requires.

The second facade proof receives one descriptor with the expected route identity. Its worker side reads every client byte to EOF from that descriptor.

The client reads the worker's exact first line. The new assertion then reads the remaining bytes to EOF after stop and requires an empty result.

I read `first_line`, `Bounded::line`, and `Bounded::to_eof`. They preserve bytes after the first newline. The new assertion cannot lose an already-buffered extra byte.

The EOF read has the existing cleanup deadline. It fails on a read error or deadline. The unchanged stop helper joins the worker-side thread and ends Core before that read.

The third proof runs the real worker binary with a handed-off descriptor. The route carries attached, baseline, live, client input, and payload output.

The composition limit is explicit: no named proof combines real Core and the real worker binary in one test. The lead approved these separate proofs.

The transcript's own real-tier proof remains a follow-up after its fence pin. This review approves no bypass of that fence for the transcript.

Each proof's doc comment cites the id and clause. The chosen worker-bin form retains its current build-root test. The separate facade binary's coverage gap remains in the owned coverage item.

The unchanged real-proofs check requires every named test to PASS in the slow run's JUnit file. A missing or failing case does not prove the id.

The Linux gate names the exact head and base. All ten steps pass in 366.6 seconds. The default tier passes 1911 tests; the slow tier passes 613.

All three exact proof tests have normal PASS results. The real-proofs check reports each one and checks seven named proofs in total.

The suite credits dp_2 by those three named proofs. The diff lists no mutant and changes no decoder crate. The gate contains no FLAKY token.

The job queues one second and runs 453 seconds. The job and outer wrapper exit zero after 454 seconds.

Gate: `~/botster-sessions/gates/botster-core-stage1-gs-dp2-real-proof-37e5ee32-pool-20261011-205649-66612.log`.

I read the complete focused Mac job. It prebuilds the candidate, runs exactly the two facade proofs and the one worker-bin proof, and checks both command statuses.

All three tests pass on Darwin 27.0.0 in slot-2. Both commands exit zero. The job and wrapper exit zero after 106 seconds.

Mac evidence: `~/botster-sessions/gates/botster-core-stage1-gs-dp2-real-proof-37e5ee32-pool-20261011-210444-97216.log`.

The Mac job exercises the R-65 EOF branch; Linux exercises its ConnectionReset branch. The job's short TMPDIR changes no Core socket-path rule.

I read the local reversal's scratch delta and original red/green output. A host write of `!` while handling RouteReceived fails the new tail assertion with `Ok([33])`.

The first-line assertion passed before that failure. The R-65 close assertion also fails. After restoration, both facade tests pass.

Reversal: `~/botster-sessions/shared/core-stage1/evidence/gs-dp2-proof/reversal-window-write.log`.

The cleanup evidence has a limit. GS added its clean command, reported output, and no-intervening-build statement after the runs. No separate original clean record exists.

I do not independently verify that local cleanup or artifact provenance. I retain GS's statement as an owner report. I found no contrary evidence.

The observed failure shows that the new tail assertion executed. The source helper retains the tail bytes. The independent pool jobs validate the positive proof paths.

The approved local reversal requirement states no contemporaneous clean-log requirement. This review accepts the supplied reversal with the explicit provenance limit above.

The prior-art search at old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` supplies no corresponding proof file or dp_2 mapping. No old test is copied.

No product code, mutation exclusion, timeout, seed, or gate verdict rule changes. Sol-B must complete the HIGH integration review.

This CLEAN applies only to the exact head above. The final-slot import and required final gate remain due after both reviews. I ran no execution job.

Sol-B publishes integration CLEAN in `24dc5fc8188f8c055073d821bdee04e5874867d9`, at `verdicts/stage1/gs-dp2-real-proof.md`.

I read the complete artifact and verified publication. Both reviews have zero findings at `37e5ee32d326f588676b6fc727ad52c86022de61`, against base `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.

Both reviews retain the composed-proof limit and the local reversal's after-run provenance statement. Neither claims independent local cleanup verification.

The final-slot import, applicable base review, exact-head final gate, and final body remain due. This receipt accepts no later head. I ran no execution job.

### Core PR #306: early source review of B13 coverage

- Head: `4d49be0b70a5193ee96c57a84f29fb9ede56b8b6`.
- Base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
- Risk tier: HIGH.
- VERDICT: NOT CLEAN (2 HIGH open).

The lead authorizes source review before READY. I read the approved design, the two approved amendments, all 39 changed files, affected source context, and the complete body.

**GS-BC-R1-1 HIGH: an empty listed binary disappears from the presence check.**

`slow_coverage::parse_list` records only testcase keys. A suite with `status: listed` and an empty `testcases` object adds no key.

`presence` derives binary identities from those keys. Thus, one valid binary plus one empty binary produces no presence error for the empty binary.

The existing overall nonempty-list check does not repair this case. Tests in the valid binary keep that list nonempty.

The anchor binary currently has only the new root test. Removing its macro leaves zero testcases and removes the anchor from the check's inventory.

This violates the approved requirement that every listed binary must have the selected root check. The gate can pass without the anchor's proof.

Required change: preserve binary identities independently from testcase keys, or reject every empty listed suite by binary name.

Add a fixture containing one empty listed binary and one valid binary. The coverage check must reject the empty binary and name it.

**GS-BC-R1-2 HIGH: a passing custom trial can use the public name constant without executing the root check.**

The name rule rejects only the reserved literal in caller source. `built_in_trial!()` has no exclusive right to produce that runtime name.

A custom harness can replace its macro expression with:

```rust
libtest_mimic::Trial::test(
    botster_test_process::built_in::TEST_NAME,
    || Ok(()),
)
```

This caller contains no forbidden literal. It uses an existing public constant and dependency. It requires no procedural macro or dependency change.

Nextest lists the expected root name as selected and not ignored. The empty trial passes. Both the name rule and presence check accept it.

The trial performs no build-root check, so a stale custom-harness binary can pass. This defeats the design's claim that only the approved macro makes the proof.

Required change: establish the approved shared entry or an actual runtime proof. A public name alone is not execution evidence.

Retain a negative fixture for this exact constant-based trial. It must fail the coverage proof despite listing the expected root name.

The source otherwise replaces the twelve hand-written root tests with the fixed-argument macro. It adds that entry to the anchor and nineteen libtest integration files.

The custom-harness macro creates a trial for no_sigpipe and the conformance suite. The existing LINKED check remains first in the suite.

The no_sigpipe target retains its one-thread setting. Nextest runs the root trial separately from its signal proof.

The new taint command scans tracked Rust files for the reserved name. The intended presence check reuses the existing nextest JSON parser after the slow run.

The slow filter adds the exact root name. Both mutation stage 2 and its baseline use that same filter. The selection broadens; it removes no existing selected test.

No timeout, budget, seed, or mutation exclusion changes. The mixed case of a rebuilt test binary linking a stale library remains an explicit owned limitation.

The current failed gate reaches test-budget and fails on `conf::in_9_legacy_keys_table`, at 2.002 seconds against the unchanged 2-second limit.

Slow, mutation, and fuzz steps do not run. That log supplies no completed coverage or mutation evidence for this head.

Failed gate: `~/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-4d49be0b-pool-20261011-212855-27306.log`.

The body retains the earlier filter-test failure and the cross-branch cost records. I accept no detached-v1 timing that the body says is absent.

The lead directs one v1 import after #299 lands, followed by one ordinary gate. This review authorizes no retry of the failed head.

The completed gate, timing comparison, and remaining required evidence stay pending. Their absence at this authorized early source review is not a third finding.

Sol-B independently records both HIGH gaps in `75f805c1f1e3da3a0e2fb2ffb6b82b3ebd2d0ab1`, at `verdicts/stage1/gs-built-in-coverage.md`.

I read the complete artifact. Its two required closures agree with this source review. This is normal NOT CLEAN review flow, not a lead blocker notification.

The prior-art search at old Core `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` supplies no build-root macro or nextest presence proof.

Both findings must close on an exact head before CLEAN. I ran no gate, build, test, diagnostic, or executable probe.

### Core PR #306: source repairs and accepted design limit

- Head: `354c59e78e2232543b06a4705e968fba60daed5f`.
- Base: `fcc09461cb0c0de4ef067c160f24d205f0f65c30`.
- VERDICT: CLEAN (early package source review only).
- Open source findings: 0.

I read the complete four-file repair at 4649a610 and the following one-file documentation commit. GitHub confirms the amended exact head and unchanged base.

GS-BC-R1-1 HIGH is closed. The parser now returns a `List` with a binary set and a separate test map.

The binary set comes from every suite key, including a listed suite with zero testcases. Presence checks that set rather than reconstructing it from tests.

The new fixture combines an empty anchor and a valid binary. It requires exactly one violation naming the empty anchor. Other tests cannot hide it.

The overall empty-list rejection remains. Slow-only, proof, and known-flaky checks still use the same testcase map and retain their existing verdict rules.

GS-BC-R1-2 HIGH is closed under the lead's explicitly accepted reduced guarantee, not by a claim of complete runtime proof.

The shared test crate removes its public name constant. The exact original constant-based bypass no longer resolves through that crate.

Xtask keeps a private-module copy for its checks and filter. The name rule caps literal occurrences at two in the macro file and three in the filter file.

The added fixture rejects a third occurrence that exports the old constant. A name mismatch between macros and checker causes presence failure.

The name rule still cannot prove execution of a custom trial. The lead accepts this limit and requires manual rejection of any root-named trial created outside the approved macros.

I verified all three lead conditions: the required sentence in the macro doc, that sentence in the complete PR body, and the new pair-common review rule.

The shared test crate exports no name constant. The lead records the decision in its handoff; Sol-B supplies decision message `msg_plugin-w_1791780044_f7eba4`.

The required sentence is: "a trial that takes the root name without calling the macro is not detected; reviewers reject any trial with that name outside built_in_test!/built_in_trial!".

This manual rule also covers a caller copying a macro-created trial's name through libtest-mimic's public `name()` method before constructing a different trial.

I read that method in the pinned libtest-mimic 0.8.2 source. This approval does not claim mechanical exclusive ownership of the runtime name.

The current custom harnesses use the approved trial macro. I found no caller constructing a replacement root-named trial. The macro retains fixed compile-time arguments and its fixed root-check callee.

The earlier reviewed root and integration entries, slow-filter addition, candidate checks, and LINKED check remain unchanged. No new timeout or mutation exemption appears.

The mixed stale-library case remains the separately recorded limit. This source verdict closes neither that limit nor the accepted manual-review boundary.

Local checks are implementer reports. No pool job validates this amended head yet. I ran no execution job.

The #299 import, ordinary gate, timing comparison, required evidence, and final body remain pending under the lead's plan. This CLEAN is source-only and accepts no completed gate result.

### Sol-B source verdict receipt

I read Sol-B's complete verdict at `cb65d372b6d03cfbb2a034cd0ed10e1cf8ff752b`, file `verdicts/stage1/gs-built-in-coverage.md`.

Sol-B reports source-only CLEAN on the same exact head and base. Both reviews close the same findings under the same accepted limit.

Sol-B confirms the independent binary inventory, empty-binary fixture, and all three lead conditions. Both reviews retain the manual rejection rule.

No finding changes. The final base review, ordinary gate, timing, required evidence, and final body remain pending. I ran no execution job.

## Core #304 — train 1 delta review

- Exact train head: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- Train tree: `456fdd44172ee85eb58ad443ec1e3dbf9f1dd2bc`.
- Current v1 base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Reviewed #304 member: `37e5ee32d326f588676b6fc727ad52c86022de61`.
- VERDICT: CLEAN (source delta for #304 only; train gate pending).
- Open findings in this delta: 0.

The lead assigns this delta in `msg_plugin-w_1791781317_0f44c2`. The assignment requires a list review because #301 and v1 also change `core-pending.txt`.

I read the complete #304 merge delta against train parent `f3b80d034acf7872fa2cc4009002a2460a5d5329`.

The four-file #304 patch equals the previously reviewed patch after index lines and hunk positions are removed.

The three proof entries, facade proof source, and worker proof source are byte-identical to the reviewed member.

Static Git analysis independently confirms these ID changes:

- The v1 import removes only `conf::tm_6_parked_work_does_not_spin` from this pending list.
- The #301 member removes only `conf::a2_1_attach_state_rules` and adds comments.
- The #304 member removes only `conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy`.
- The train ID set equals the current v1 set minus the two member removals. No member adds an ID.

The imported comments preserve their IDs. No unrelated pending ID disappears. The DP-2 proof map remains unchanged.

The v1 and #301 source changes are outside the two proof files and their test-process helper. They do not change the reviewed EOF assertion.

The original composed-proof limit and local reversal provenance limit remain. This delta does not expand the original approval.

Sol-B owns the combined train review. This verdict covers only #304's source delta. It does not approve #301 or accept train gate evidence.

The final gate must use the frozen train head after the required delta reviews. I ran no gate or executable base-check script.

### Sol-B train source receipt

I read both complete artifacts at `3deaf0a165eb877a4a6fc29b8afe87b6ff2daeab`: `verdicts/stage1/train-1.md` and `verdicts/stage1/gs-dp2-real-proof.md`.

Sol-B reports combined source CLEAN and #304 delta CLEAN on the same exact train head and current v1 base.

Sol-B confirms the preserved member patches and exact pending removals. Sol-B finds no interaction between #301's testkit control and #304's real-process proofs.

The original R-97, identity, guardian, composed-proof, and reversal provenance limits remain. This receipt changes no finding or approval scope.

The train gate, final evidence receipts, and exact base-tree verification remain pending. I ran no execution job.

## Core #306 — slow-fixture repair review

- Exact head: `c20029a68cbb80612d242e7ce8560260bb7460e4`.
- Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- VERDICT: NOT CLEAN (1 open).

The base import `ee61090e81650831dcd9131b9cdacb24d70255f9` has parents the reviewed source head and current v1.

The implementer reports canonical base-merge-check PASS. Its retained output remains due; I requested that output. I ran no base-check script.

I read the complete one-file repair and its surrounding fixture test. GitHub confirms the exact repair head and base.

### GS-BC-R3-1 — HIGH — OPEN: the positive fixture creates a root-name test outside the approved macro

Evidence: `xtask/src/slow_coverage.rs`, the slow test `a_slow_only_test_outside_the_filter_or_an_ignored_one_not_pending_fails`.

The repair appends `#[test] fn {TEST_NAME}() {}` to the temporary library source. The name comes from `test_budget::BUILT_IN_TEST`.

This creates an actual empty test with the reserved root name outside `built_in_test!()`. The repair also prints the reserved name in the custom fixture's list.

The positive presence fixture then accepts these entries. Their bodies do not call the shared build-root check. The comment says they are made as the macros make them.

The pair-common rule at line 108 requires rejection of any reserved-name test or trial outside `built_in_test!()` or `built_in_trial!()`.

The lead accepted the reduced name guarantee only with that manual rule. The rule contains no positive-fixture exception.

Required change: use the approved macros for positive fixture entries, or test list data without creating a reserved-name test outside the macros.

The negative missing-test fixture can remain. Do not describe an empty body as the shared root check. A passing gate cannot close this source finding.

I sent the finding to the implementer and Sol-B. This is an ordinary NOT CLEAN result, not a lead BLOCKED notice.

### Retained failed gate and pending evidence

The imported-head gate is `gates/botster-core-stage1-gs-built-in-coverage-ee61090e-pool-20261011-220352-43493.log`.

The header records the exact imported head, current base, and node MS-A1. The default tier passes. The slow fixture assertion fails.

The actual assertion output includes the original unselected-test violation and three missing-root violations. This matches the repair's stated trigger.

The summary reports slow FAIL at 66.8 seconds. Mutants and fuzz are NOT RUN. The job and wrapper exit 1 after 393 seconds.

This failed result remains failed. It supplies no acceptance evidence. The implementer reports a new ordinary gate on the source repair; no completed result is reviewed here.

The complete live PR body still records the earlier gates and pending timing. The final body and required evidence remain due.

I ran no build, test, gate, diagnostic, mutation job, or executable base check.

## Core #306 — repaired slow fixture and base receipt

- Exact head: `c69cbcb85e5d173468703bdea45bb3a325e87366`.
- Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- VERDICT: CLEAN (source delta only; gate evidence pending).
- Open source findings: 0.

I read the complete repair and its net delta from the imported head. Only the slow fixture test changes.

GS-BC-R3-1 HIGH is closed. The repair removes both runtime injections. It creates no reserved-name test or trial outside the approved macros.

The tracked fixture files equal current v1. The copied fixture retains its missing-root entries. The existing module rename remains part of the slow-filter cases.

The `checked` closure calls the real list checker. It partitions tier-prefixed presence violations from the other violations.

Each call asserts exactly three missing-root entries with the expected binary and tier. An extra, missing, or changed presence entry fails that assertion.

The closure returns the other violations to the original assertions. The unselected test, proof selection, renamed module, and ignored pending trial keep their earlier checks.

This partition occurs only inside the slow test. The production presence check, gate decisions, slow filter, root macros, and candidate checks do not change.

I read both retained canonical base-check logs. Their SHA-256 values match the implementer's supplied values.

`gates/botster-core-stage1-gs-built-in-coverage-base-merge-check-354c59e7-to-ee61090e.log` reports PASS and exit 0.

It confirms conflict-free ancestry, disjoint base paths, unchanged own patch, and preservation of the base's pending lists. The prior source review carries to the import.

`gates/botster-core-stage1-gs-built-in-coverage-base-merge-check-354c59e7-to-c69cbcb8.log` reports FAIL only at condition 3.

The new slow test changes the own patch. Conditions 1, 2, and 4 pass. This explicit source delta review covers that change.

The imported-head failed gate remains recorded. The implementer reports stopping the c20029a6 gate and starting a new gate on this repaired head.

No completed repaired-head gate result is accepted here. Local checks remain implementer reports. Timing, required evidence, final body, and integration receipt remain pending.

The lead's accepted manual name rule and mixed stale-library limit remain. I ran no execution job or executable base-check script.

### Sol-B repaired-fixture receipt

I read the complete integration verdict at `555554162c66d2d5487f3f36508799deb6074213`, file `verdicts/stage1/gs-built-in-coverage.md`.

Sol-B reports source delta CLEAN on the same exact head and base. Both reviews close the fixture finding and retain its earlier history.

Both reviews retain the canonical condition 3 FAIL and the imported-head failed gate. No finding changes. Completed evidence and the final body remain pending.

## Core #308 — review held for the lead's revised form

Initial head: `c5b3804222de5b3ecf5f5139852294787bd669b1`. Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`. Risk: HIGH.

I read all three changed files, the parser, CI and lists callers, the complete PR body, and BUILD Testing rule 9.

The initial change requires a signature string with comma clauses. It preserves the existing stage filters and an empty repository list.

The parser checks string presence and clause structure. It does not check semantic step, limit, or symptom content. It performs no automatic failure matching.

Before publication of a source verdict, Sol-B relays the lead's hold instruction `msg_plugin-w_1791782721_1fa4f3`.

The lead requires named signature fields: a gate step, symptom enum, seed, optional text, and preferably a matcher.

I hold this review for the revised exact head. This record supplies no CLEAN for the initial form. I ran no execution job.

## Core #308 — named signature source review

- Exact head: `0a9a6cdedd9abb619a65a225bd0bf3d5594a2a9f`.
- Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Risk: HIGH.
- VERDICT: NOT CLEAN (1 LOW open; gate evidence pending).

I read all three changed files, the complete parser and its callers, the full revised body, BUILD Testing rule 9, and the lead's named-field decision.

The lead's handoff requires a gate step, symptom enum, seed, optional text, and preferably a matcher. This head implements the schema and records the matcher as proposed follow-up work.

`signature` is a required table. Unknown fields fail. Missing or non-string step and symptom values fail. Values must match the closed allowed sets.

The step set comes directly from `JOBS` through `ci::step_names()`. This helper returns names; it executes no job and changes no job selection.

The seed must be `"any"` or a nonnegative TOML integer. The integer conversion follows the nonnegative check. Optional text must be a string with nonempty trimmed content.

The parser stores every signature field. The report prints the signature, including a quoted text value when present.

The lists caller converts a parse error into a gate problem. The mutation caller also propagates a parse error before selecting its filters.

The stage filter, absent check, flaky catch rejection, and stage-2 baseline behavior remain unchanged. The repository list has no entry.

This change adds no mutation exclusion and skips no additional test. It changes no binary dependency, candidate build, deadline, seed schedule, or mutation verdict.

The schema supplies metadata, not a failure matcher. Reviewers still verify the actual failure match and the open structural fix under BUILD rule 9.

No parser or filter defect is found in this source review. Local checks remain implementer reports. No completed exact-head gate result is supplied.

### GS-FS-R1-1 — LOW — OPEN: the body overstates the refusal cases

Evidence: the complete PR body's evidence paragraph for `an_entry_that_breaks_a_rule_is_refused` says the new cases cover each listed signature refusal.

The earlier list includes a non-string symptom. The twelve new cases contain a non-string step, but no non-string symptom case.

The shared string parser rejects a non-string symptom. This finding concerns the evidence claim, not that parser behavior.

Required change: describe the actual selected refusal cases without claiming that each listed refusal has its own case. Additional tests are not required.

I sent this finding to GS and Sol-B. No CLEAN is issued while this LOW finding remains open.

The exact-head gate, mutation evidence, final body, and integration review remain due. I ran no execution job.

### Sol-B signature review receipt

I read the complete verdict at `0f779dbb1ca907f97c623eed0bbcb937bf533d16`, file `verdicts/stage1/gs-flaky-signature.md`.

Sol-B reports NOT CLEAN with the same LOW body finding on the same exact head and base. Sol-B finds no additional parser or filter defect.

Both reviews distinguish schema validation from failure matching and retry authorization. No finding changes. The body correction and completed gate remain due.

## Core #304 — train 1 final evidence receipt

- Exact train head: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- Train tree: `456fdd44172ee85eb58ad443ec1e3dbf9f1dd2bc`.
- Gate base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- VERDICT: CLEAN (#304 member source and final train evidence).
- Open findings for #304: 0.

Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-1-be95ef13-pool-20261011-221548-42911.log`.

I read the completed gate header, base accounting, proof results, mutation summary, and final status. The header records this exact train head and base on MS-A1.

The lists step uses the same base and merge base. It reports 691 ledger IDs, 407 pending, 2 deferred, 18 withdrawn, and 264 to run.

All three named DP-2 tests PASS in the slow tier. The real-proofs report credits DP-2 through each exact binary and test entry.

The default tier reports 1,920 passed. The slow tier reports 617 passed. The slow coverage check and seven named proof checks succeed.

All ten gate steps PASS. The step total is 520.5 seconds. The job and wrapper exit 0 after 850 seconds, with 232 seconds queued and 618 seconds running.

The train mutation report contains 29 mutants: 21 caught, 8 unviable, 0 missed, and 0 timeout. No FLAKY result appears.

Stage 1 catches all viable mutants. Stage 2 does not run because stage 1 misses none. These mutants belong to #301's testkit source changes.

This receipt does not claim a new mutation proof for #304's test-only patch. Sol-B and the #301 reviewers own the remaining member and combined evidence reviews.

I read #304's complete live body. It retains the original member head and original gate evidence. This train receipt records the later combined gate separately.

The prior source delta remains CLEAN. The original Mac proof and local reversal retain their recorded scope and provenance limit. The composed-proof limit remains.

This receipt accepts #304 on the frozen train head. The lead must complete the train record and verify the final base tree and push before declaring the train merged.

I ran no execution job.

## Core #308 — body finding closure

Exact head remains `0a9a6cdedd9abb619a65a225bd0bf3d5594a2a9f`. Base remains `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.

I read the complete amended body. GS-FS-R1-1 LOW is closed. The paragraph now names the twelve selected refusal cases.

The paragraph explicitly states that non-string symptom and text values have no separate cases. It correctly identifies the shared string reader.

Open source findings: 0. The completed gate remains required before final CLEAN. This closure accepts no gate result.

## Core #306 — slow target manifest delta

- Exact head: `2fa58b79291f44023778a8a53f8bd05387e93cfb`.
- Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- VERDICT: CLEAN (source delta only; gate evidence pending).
- Open source findings: 0.

I read the complete five-manifest delta, all eleven affected file headers and root macro entries, and the tier selection code.

Each affected file already has `#![cfg(feature = "slow")]`. Each contains the approved root macro. Each owning package declares the slow feature.

The manifest adds `required-features = ["slow"]` to each named test target. The default tier stops building empty binaries for those targets.

The slow tier enables each package's slow feature and selects the slow binaries. Mutation stage 2 enables the same package features.

The repair removes no previously executable default-tier test. Every affected file was empty in that tier. The slow-tier test bodies and selections remain unchanged.

The proof citation checker reads required target features as feature conditions. These entries preserve the existing file-level slow condition.

No target is deleted or renamed. No product dependency, gate decision, root macro, timeout, mutation exclusion, or pending list changes.

The earlier repaired-head gate remains failed: `gates/botster-core-stage1-gs-built-in-coverage-c69cbcb8-pool-20261011-221807-53060.log`.

Its slow tests pass, but presence reports the eleven empty default-tier binaries. The slow step fails at 48.9 seconds with eleven violations.

Mutants and fuzz do not run. The job and wrapper exit 1 after 586 seconds, with 469 seconds queued and 117 seconds running.

That result demonstrates the retained empty-binary rejection. This manifest repair changes target availability rather than weakening that rejection.

The implementer reports a gate running on the new head. Local list checks remain implementer reports. No completed new-head gate is accepted here.

The gate, mutation results, timing comparison, required evidence, final body, and integration receipt remain pending. The accepted manual name rule and stale-library limit remain.

## Core #304 — train body receipt

I read the complete amended #304 body. The frozen member remains `37e5ee32d326f588676b6fc727ad52c86022de61`.

Its train section records head `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`, tree, base, review receipts, and the completed train gate.

The stated DP-2 proof results and gate counts match the reviewed log. The composed-proof limit remains. No finding changes.

### Sol-B final train receipt

I read both complete artifacts at `67f04019a3199de0562ec28e9969fb44aa3ea6f3`: `verdicts/stage1/train-1.md` and `verdicts/stage1/gs-dp2-real-proof.md`.

Sol-B accepts the combined train and #304's final evidence on the same exact train head, tree, base, and completed gate.

Sol-B records all member receipts and verifies the mutation catches against their citations and normal PASS results. No #304 or combined finding remains.

The lead's base update, gated-tree verification, and verified push remain due. This receipt claims no completed merge. I ran no execution job.

### Sol-B slow-target delta receipt

I read the complete integration verdict at `7dcc38b443612f26eec1baa0a15dc9e5a37cbcc0`, file `verdicts/stage1/gs-built-in-coverage.md`.

Sol-B reports source delta CLEAN at `2fa58b79291f44023778a8a53f8bd05387e93cfb`, with base `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.

Both reviews confirm that no executable default-tier test is removed. The slow tier retains the eleven targets and their approved root macros.

No finding changes. The failed gate remains failed. Completed new-head evidence, timing, and the final body remain pending. I ran no execution job.

## Core #308 — authorized in_8 entry and comment repair

Exact head: `2ac0be6fddd37b540bbe7845ae975499a58eaf57`. Base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.

I read the complete delta at `66369ef8365d0c05bfa2767328546381bfcdcc52` and its following comment-only repair. I read the complete live body.

The lead's handoff authorizes the in_8 entry and names PC, contracts #79, and the contracts-v0.1.44 pin as the removal trigger.

The entry names only `botster-core::slow_conformance` and `conf::in_8_mode_checked_at_write_start_by_worker`. It changes no pending status.

The signature names gate step `slow`, symptom `step_timeout`, seed `any`, and literal text `step=8 limit=step_timeout`.

I read the failed #306 gate `gates/botster-core-stage1-gs-built-in-coverage-2fa58b79-pool-20261011-223259-12751.log`.

It reports this exact test INCONCLUSIVE at seed 0, transcript step 8, and step_timeout. The slow step fails. The job and wrapper exit 1 after 121 seconds.

The recorded signature matches that reported failure. The cause attribution remains the lead's accepted diagnosis and prior in_8 disposition, not a new reviewer execution.

The existing exact binary/test filter excludes the listed test only from mutant runs that list it. The ordinary slow tier and stage-2 baseline retain it.

The default conformance binary has a different binary ID and is not excluded. A listed test's first failure cannot count as a mutant catch.

Removing a genuine catch from this listed test cannot turn a missed mutant into a pass. The mutant still needs another valid catch under the unchanged two-stage verdict.

The signature does not select mutant tests. It records the failure for policy review. No automatic matcher or retry authorization is added.

`removal` keeps the earlier Core PR form and adds exactly three numeric version components between `contracts-v` and ` pin`.

Malformed component counts and nondigit components fail. The parser still rejects an empty field. This syntax check does not prove that the pin or fix exists.

The repository test now requires the exact in_8 entry and removal trigger. The lead requires removal when the .44 pin carries its fix. Stage 1 still requires an empty list.

### GS-FS-R2-1 — LOW — CLOSED: the repository test's comment described an empty list

At 66369ef8, the renamed test's doc comment still said the list was empty. The source and test required one entry.

I sent the finding to GS and Sol-B. Required change: name the current entry and removal trigger in that comment.

The comment-only repair at the exact reviewed head names the in_8 race, PC's fix through the .44 pin, and the retained empty-list exit condition.

No source finding remains. The completed exact-head gate, mutation results, final body, and integration receipt remain due before final CLEAN.

I ran no execution job.

## Core #304 — completed train merge receipt

The lead records the train fast-forward in its handoff. GitHub reports #304 MERGED with commit `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.

The local remote-tracking v1 ref names that same head and tree `456fdd44172ee85eb58ad443ec1e3dbf9f1dd2bc`, which equals the reviewed gated tree.

The frozen member and all final receipts remain unchanged. No finding remains for #304. This completes the GS review record for this merge.

## Core #308 — completed exact-head gate acceptance

- Exact head: `2ac0be6fddd37b540bbe7845ae975499a58eaf57`.
- Source merge base: `ae49bbec2d1086f918341bc596bf8560ba27bcd1`.
- Recorded gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- VERDICT: CLEAN (0 open on this exact head and supplied gate).

Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-flaky-signature-2ac0be6f-pool-20261011-224550-12724.log`.

I read the completed header, lists accounting, tier results, mutation records, and final exits. I read the complete updated PR body.

All ten steps PASS in 440.5 seconds on MS-A1. The default tier passes 1,914 tests. The slow tier passes 616 tests.

The in_8 transcript passes in both tiers. The known-flaky entry does not hide its normal slow execution.

Mutation stage 1 reports 27 mutants: 25 caught, 2 unviable, 0 missed, and 0 timeout. Stage 2 does not run because stage 1 misses none.

Static log analysis matches all 25 raw catch lines to the 25 citation lines. Each named catcher has a normal PASS. No FLAKY result appears.

The two unviable outcomes are the default-value replacements in signature and parse. The log does not count either as a catch.

The fuzz step reports no changed decoder-harness crate. The job and wrapper exit 0 after 590 seconds, with 110 seconds queued and 480 seconds running.

Both LOW findings are closed. The named schema and authorized in_8 entry retain their reviewed scope. No source or supplied-evidence finding remains.

The lists step explicitly records gate base be95ef13 and merge base ae49bbec. Static Git ancestry confirms the gate base is not an ancestor of this head.

This completed gate therefore supplies initial exact-head evidence. It does not satisfy the current-base merge condition or validate a later imported head or train.

The lead must apply the authorized import or train procedure and obtain the required final evidence on its exact delivered head.

The matcher remains proposed follow-up work. This approval supplies no failure attribution or retry authorization. I ran no execution job.

### Sol-B exact-head signature acceptance receipt

I read the complete integration verdict at `f0326d32b5d1416aae9b9208a840ad6a79327f4a`, file `verdicts/stage1/gs-flaky-signature.md`.

Sol-B reports CLEAN on the same exact PR head and supplied gate. Both LOW findings are closed. No finding changes.

Both reviews retain the source merge base and newer gate base distinction. Required import or train evidence remains due. This receipt accepts no later head.

## Core #310 — default tests without Cargo execution

Exact head: `204e91d820c2d719c00b3665e2d5ba6aa436b0d0`. Base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`. Risk: HIGH.

I read all four changed files, the affected callers, the complete PR body, and the lead's root-fix assignment.

The lead forbids another #306 re-gate after the job-common test timeout. This separate source repair removes Cargo execution from the affected default tests.

The new checkout resolver is test-only. It reads the pinned contracts source from Cargo.lock and requires one matching checkout path.

The path must have the repository prefix and a hexadecimal revision prefix of at least seven characters. Missing or multiple matches return errors.

The file-reading wrapper starts no process. The job-common test retains its nonempty-copy assertion and byte comparison.

The resolver does not independently prove a checkout's Git content from its directory name. The new slow test compares its canonical path with Cargo metadata.

The production lists command still reads the checkout from Cargo metadata and compares the checked-in job-common copy. That production check is unchanged.

The `packages_of` extraction retains the original metadata parsing. The caller retains Cargo execution and root canonicalization before it invokes the pure parser.

The default fixture covers package features, target kinds, names, relative paths, required features, and exclusion of a target outside the root.

The real workspace metadata test moves unchanged to `mutants_cited::slow_tests`. Both real comparison tests match the retained slow-module filter.

Static caller inspection finds the remaining Cargo executions in production commands and slow-test modules. The caps test constructs a Cargo command but does not execute it.

This inspection covers the xtask caller audit named in the body. It does not claim a new mechanical prohibition against future Cargo calls in default tests.

No product dependency, candidate binary, mutation exclusion, gate verdict, timeout, seed schedule, or pending list changes.

The faster default tests preserve the byte-copy decision. The full gate retains the real metadata checks in production and the slow tier.

Open source findings: 0. Local results remain implementer reports. The exact-head gate, mutation evidence, final body, and integration verdict remain due.

This source record accepts no running gate result and authorizes no retry. I ran no execution job.

## Core #310 — completed exact-head acceptance

- Exact head: `204e91d820c2d719c00b3665e2d5ba6aa436b0d0`.
- Source and gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- VERDICT: CLEAN (0 open on this exact head).

Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-no-cargo-default-tier-204e91d8-pool-20261011-230511-22291.log`.

I read the completed header, base accounting, affected test results, two-stage mutation results, and final exits. I read the complete updated body.

The lists step names the same base and merge base. All ten steps PASS in 481.4 seconds on MS-A1.

The default tier passes 1,921 tests. The job-common test takes 0.006 seconds. The pure metadata fixture takes 0.002 seconds.

The slow tier passes 619 tests. The new resolver comparison passes in 0.114 seconds. The moved real metadata test passes in 0.017 seconds.

Stage 1 tests four mutants: one caught, one missed, and two unviable. The pure metadata fixture catches the empty `packages_of` replacement.

The stage-1 miss replaces `packages` with an empty result. The unmutated slow baseline passes. Stage 2 catches that mutant with the moved real metadata test.

The combined verdict has two caught, two unviable, zero missed, and zero timeout. Static log analysis matches both raw catches to their citations and normal PASS records.

No FLAKY result appears. The fuzz step reports no changed decoder-harness crate. The job and wrapper exit 0 after 680 seconds.

The job spends 119 seconds queued and 561 seconds running. The gate preserves the metadata mutation catch after its test moves to the slow tier.

The new resolver functions are test-only and receive no cargo-mutants mutants. Their fixture and slow comparison pass; this evidence does not claim a new mutated resolver proof.

I also read the earlier #306 re-gate failure. Its job-common test times out at 2.002 seconds and reports a remaining Cargo metadata process.

That earlier gate remains failed. This new source head removes the Cargo launch from the affected default test without changing its byte-copy decision.

No source or evidence finding remains. The integration verdict and authorized final import or train procedure remain due. This CLEAN accepts no later head.

I ran no execution job.

### Sol-B default-tier root-fix receipt

I read the complete integration verdict at `5c99bdb6fd6eff692ab45e044af42e4f257843ee`, file `verdicts/stage1/gs-no-cargo-default-tier.md`.

Sol-B reports CLEAN on the same exact #310 head and source/gate base. The source and completed gate conclusions agree with the package review.

No finding changes. The initial integration receipt is complete. The authorized final import or train procedure remains due. No #306 retry is authorized.

## Train 3 — #310 and #308 source deltas

- Exact train head: `3435a6d2ce981ec5c06a888ba907667938f50137`.
- Train tree: `787fc74f540d554f8e28070456d3cc5feeaa19b2`.
- Current v1 base: `080987829a2e2245a7ea97606020400392c974d6`.
- #310 delta verdict: CLEAN (source delta only; train gate pending).
- #308 delta verdict: NOT CLEAN (1 HIGH open).

The lead assigns these deltas in `msg_plugin-w_1791818862_17a9a6`. The train merges #310, #312, #308, then #309.

I read the merge ancestry, both complete member merge deltas, the final own files, the pin changes, and affected tier/helper context.

Static comparisons show both member patches are preserved after index lines and hunk positions are removed. All four #310 files are byte-identical to its accepted member.

All three #308 files are also byte-identical to its accepted member. Textual preservation does not close the list interaction described below.

### #310 shared helper delta

The resolver reads the contracts pin from the current lock. The train changes that pin from .43 to .44 without changing the package name or source layout.

The resolver does not hardcode the old revision. The retained slow test compares its canonical result with Cargo metadata on the delivered train.

The production lists check still obtains the contracts checkout from Cargo metadata. The byte-copy check and real metadata test remain selected.

The extracted parser, xtask slow feature, and slow-module filter remain unchanged. The train adds no #310 mutation exclusion.

No #310 source delta finding remains. This verdict covers #310 only. Sol-B owns the combined train review, and the train gate remains due.

### GS-FS-T3-1 — HIGH — OPEN: the .44 pin leaves the obsolete in_8 entry active

Evidence: train Cargo.lock pins contracts-v0.1.44 at `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea`.

The train retains #308's in_8 known-flaky entry with `removed_by = "contracts-v0.1.44 pin"`. The repository-list test still requires that entry.

The accepted removal trigger is now met. I read the contracts transcript delta from .43 to this exact .44 commit.

That delta adds the R-74 held-write setup and input-queue fence before the tested write. It carries the entry's named structural fix.

The stale entry continues to exclude this exact real-tier test from mutant runs. It also retains the old signature for re-gate policy review after its removal trigger.

The earlier #308 acceptance explicitly required removal through the .44 pin. The combined train must preserve that condition even though the member files merge unchanged.

Required change: remove the in_8 entry on the delivered train head. Update the repository-list assertion and comment to require the empty list.

Preserve the named signature parser and report format. Obtain delta review and run the single train gate on the repaired exact head after source findings close.

Sol-B independently reports the same interaction as T3-I2 MEDIUM. I classify it HIGH because the delivered gate retains an obsolete exclusion and re-gate signature.

Both reviews require the same closure. I sent the finding to GS and Sol-B and asked the lead for the concrete frozen-train repair.

No train gate result is accepted. I ran no build, test, gate, diagnostic, mutation job, or executable base check.

## Train 3c — #310 source delta

- Exact head: `b78df725177d1a931e1eddb9bb67023e7e0ae645`.
- Tree: `43cdf59980d93e0672423bedc0edf506405256be`.
- Current v1 base: `080987829a2e2245a7ea97606020400392c974d6`.
- VERDICT: CLEAN (#310 source delta only; train gate pending).
- Open #310 delta findings: 0.

The lead abandons train 3 and train 3b in `msg_plugin-w_1791819182_b03965`. The rebuilt train contains #310 and #312 only.

I inspected the new exact head, parents, tree, and relevant file delta. Its tree equals `d1571ce467a1c54b1763c206cea301f5895d365f`.

That earlier prefix contains the same two members and merge order. It precedes #308 and #309 in the reviewed train.

The four #310 files remain unchanged from the accepted member. The earlier shared-helper review applies to this identical two-member tree.

The .44 pin retains the same contracts package and source layout. The dynamic resolver and its real slow comparison remain selected.

Removing #308 restores the base's empty known-flaky list and earlier parser. It does not remove or change a #310 helper or test.

GS-FS-T3-1 is absent from this delivered scope because #308 leaves the train. This is not a source repair or CLEAN for #308.

The original #308 train finding remains in its history. Its future imported head must resolve the obsolete entry before acceptance.

The lead reports a local compile check. I retain that result as a lead report and accept no completed train gate yet.

Sol-B owns the combined train review. The single frozen-head gate and final evidence receipts remain due. I ran no execution job.

### Sol-B train 3c source receipt

I read both complete artifacts at `8e1cd79034abe1a88db3ee43fcd90af9c202451d`: `verdicts/stage1/train-3.md` and `verdicts/stage1/gs-no-cargo-default-tier.md`.

Sol-B reports combined source and #310 delta CLEAN on the same exact train head, tree, and base. No reduced-scope finding remains.

Sol-B confirms preserved member patches, the .44 resolver interaction, and the pin's list formulas. The abandoned train findings remain in history.

No #308 or #309 source repair is claimed. The final train gate, member receipts, final bodies, and lead merge checks remain due.

## Train 3d — #310 source delta confirmation

- Exact head: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
- Tree: `a509da5c79a4b456acd1ffad1c96ca0a80a6d834`.
- Current v1 base: `080987829a2e2245a7ea97606020400392c974d6`.
- VERDICT: CLEAN (#310 source delta only; gate evidence pending).
- Open source delta findings: 0.

The lead removes #312 after train 3c's red lists step in `msg_plugin-w_1791820071_054c1f`. Train 3d contains #310 alone.

I inspected the exact head, both parents, tree, own patch, and affected file blobs. The head merges current v1 with the accepted #310 member.

The own patch is preserved after index lines and hunk positions are removed. All four files are byte-identical to the accepted member.

This head is the first parent of train 3c. Its tree is distinct from the two-member prefix tree recorded for train 3c.

Removing #312 restores the base's .43 contracts pin. The resolver reads that lock revision; the retained production metadata and slow comparison remain consistent.

The test-only resolver, pure package parser, real metadata test, and tier selection retain their accepted source. No source delta finding remains.

The train 3c failure remains failed. Log: `gates/botster-core-stage1-train-3c-b78df725-pool-20261012-083917-27480.log`.

Its lists step reports that `conf::a5_2_deferred_completion_keeps_slot_until_polled` passes and must leave core-pending. Later steps do not supply completed acceptance evidence.

This scope change removes the .44 pin member. It does not claim a source repair or acceptance for #312.

The lead supplies the running gate log `gates/botster-core-stage1-train-3d-838e4bb0-pool-20261012-084740-70545.log`.

I accept no unfinished gate result. The final receipt must name that completed log and this exact head. I ran no execution job.

## Train 3d — #310 final evidence receipt

- Exact head: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
- Tree: `a509da5c79a4b456acd1ffad1c96ca0a80a6d834`.
- Gate base: `080987829a2e2245a7ea97606020400392c974d6`.
- VERDICT: CLEAN (#310 source and final train evidence).
- Open findings: 0.

Completed log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-3d-838e4bb0-pool-20261012-084740-70545.log`.

I read the exact identity, base accounting, affected test results, tier summaries, two-stage mutation records, and final exits.

The lists step names the same gate and merge base. All ten steps PASS in 484.0 seconds on MS-A1.

The default tier passes 1,936 tests. The job-common test takes 0.006 seconds. The pure metadata fixture takes 0.002 seconds.

The slow tier passes 621 tests. The resolver comparison passes in 0.111 seconds. The moved metadata test passes in 0.015 seconds.

Stage 1 reports one caught, one missed, and two unviable mutants. The unmutated slow baseline passes all 621 tests.

Stage 2 catches the empty `packages` replacement through the moved real metadata test. The final verdict has two caught, two unviable, zero missed, and zero timeout.

Static log analysis matches both raw catches to their citations. Each catcher has a normal PASS. No FLAKY or INCONCLUSIVE result appears.

The fuzz step reports no changed decoder-harness crate. The job and wrapper exit 0 after 708 seconds, with 135 seconds queued and 573 seconds running.

The source delta remains unchanged. The default-tier test keeps its byte-copy decision, and the final gate preserves the moved mutation catch.

I read #310's complete live body. It retains the frozen member and initial evidence. This receipt separately records the final one-member train gate.

No #310 source or evidence finding remains. The final integration receipt, train publication record, and lead tree/push checks remain due before merge completion.

The train 3c failure and abandoned member findings retain their earlier status. This acceptance applies only to train 3d's exact head. I ran no execution job.

### Sol-B train 3d final receipt

I read both complete artifacts at `be13b2edc2149d5fcc28bdffb929ca64b059daca`: `verdicts/stage1/train-3.md` and `verdicts/stage1/gs-no-cargo-default-tier.md`.

Sol-B accepts #310 and the one-member train on the same exact head, tree, base, and completed gate. No finding remains in this scope.

Both reviews preserve the stage-2 catch and the earlier failed-train history. The final integration receipt is complete.

The lead's train publication and merge/tree/push verification remain due. This receipt accepts no later composition and authorizes no #306 retry.

## Core #306 — imported source and command proof acceptance

- Exact head: `0a09168702048cf34ef3fa30dffc07131b620f0a`.
- Source and gate base: `838e4bb01dda53c419ab824913fdc5adae7a16c1`.
- VERDICT: CLEAN (0 open on this exact head).

I read the complete new test, canonical base-check failure, imported overlap deltas, complete body, reversal, focused mutation evidence, and completed gate.

### Source delta and the failed gate

The base-check log `gates/botster-core-stage1-gs-built-in-coverage-base-merge-check-2fa58b79-to-a217da82.log` fails conditions 2 and 3.

Four paths overlap with the imported base. Their imports add signal handling, an op_holds module, and the previously accepted DP-2 proof assertion.

Static patch comparison independently confirms that the import preserves the complete prior #306 own patch after index lines and hunk positions are removed.

The new head changes only `xtask/src/built_in.rs` after that import. The required-features repair, macros, candidate checks, LINKED check, and presence decisions remain intact.

The imported gate at `a217da8226048e9cff904f629faee36b5da0df5a` fails because `built_in::command` replaced with `Ok(())` survives both stages.

I read `gates/botster-core-stage1-gs-built-in-coverage-a217da82-pool-20261012-091300-76813.log`. That gate remains failed.

The new test calls the command on tracked fixture files. An allowed file and a non-Rust file pass. A Rust file with the reserved name fails.

The invalid fixture file is scanner input. The test does not compile or accept a fake root-name trial. It retains the mandatory manual name rule.

The test checks command behavior through its result. It changes no production decision and adds no Cargo launch or mutation exclusion.

### Reversal and focused mutation evidence

I read `shared/core-stage1/evidence/gs-306-command-reversal-0a091687.log`. Its header records the exact head, target, cleanup time, and unchanged worktree.

The scratch early return makes the new test fail at `unwrap_err()` because the command returns success. After restoration, the new test passes.

The cleanup facts remain an author-recorded provenance statement. I ran no local reversal and claim no independent cleanup execution.

The focused log is `gates/botster-core-stage1-gs-built-in-coverage-0a091687-focused-mutants.log`. Its commands use `--no-config` on the two changed checker files.

Both stages test the same seventeen mutant identities. Static outcome-set comparison confirms that these identities equal the gate's seventeen identities.

Default tests catch eleven and miss four, with two unviable. Slow tests catch seven and miss eight, with two unviable. Neither stage reports a timeout.

The intersection of the two missed sets is empty. No configured exclusion hides a listed mutant in this focused evidence.

The focused shell's exit alone is not a combined mutation verdict. The recorded outcome sets and completed ordinary gate supply that evidence.

### Completed gate and FLAKY disposition

Completed log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-built-in-coverage-0a091687-pool-20261012-093420-53357.log`.

The lists step records the same base and merge base. All ten steps PASS in 701.0 seconds on MS-A1.

Default passes 1,952 tests. Slow passes 648 tests. The built-in name check reads 244 Rust files, and the presence checks succeed.

Stage 1 catches eleven, misses four, and reports two unviable. The unmutated stage-2 baseline passes before all four remaining mutants are caught.

The final verdict has fifteen caught, two unviable, zero missed, and zero timeout. Static analysis matches all fifteen raw catch identities to their citations.

Every cited catcher has a normal PASS. The command replacement is now caught by the new fixture test in stage 1.

Stage 2 records one in_8 FLAKY result under the empty `slow_coverage::tests` replacement. The gate gives that failure-then-pass result no catch credit.

The independent catch is `ci::slow_tests::slow_a_listed_test_that_the_workspace_list_lacks_is_still_no_catch`. Its normal PASS and mutant catch are recorded.

The named structural fix remains PC's contracts #79 through the contracts-v0.1.44 pin. That fix is not in this head's .43 pin.

This matches the retained in_8 disposition and BUILD rule 9's mutation-profile exception. The FLAKY result is not relabeled as a catch or removed from the record.

The job exits 0 after 716 seconds. The wrapper exits 0 after 717 seconds. The fuzz step completes its selected harnesses.

### Timing and scope

The body supplies the required before/after sample: earlier slow step 106.8 seconds; this slow step 92.0 seconds.

The earlier gate ran no stage 2. This gate runs a 54.2-second baseline and four mutants in 131.0 seconds. The full mutation step takes 234.1 seconds.

The samples have different bases and mutant sets. The body correctly avoids claiming a measured gain or small added cost.

All earlier source findings remain closed. The accepted manual name-rule limit and mixed stale-library limit remain explicit.

The complete body preserves failed gates, the repair, focused evidence, timing, and FLAKY disposition. No source or supplied-evidence finding remains.

The integration receipt and authorized final delivery procedure remain due. This CLEAN accepts no later head or additional retry. I ran no execution job.

### Sol-B imported B13 evidence receipt

I read the complete integration artifact at `efacfb421ddf042e775c0b2643ea9789a340ee97`, file `verdicts/stage1/gs-built-in-coverage.md`.

The substantive receipt is `26a9a36916314cf73b3bce71b267af1d6ef86053`. The later commit formats the artifact.

Sol-B accepts the same exact head and base. Both reviews retain the canonical FAIL, explicit import review, command proof, focused sets, and valid catch beside the FLAKY result.

No finding changes. The initial integration receipt is complete. The lead's final delivery, applicable delta, final gate, and merge checks remain due.

## Train 5 — #306 source delta

- Exact head: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
- Tree: `42e00e9808db9faec524e2404f67302bee0d66c5`.
- Current v1 base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Frozen #306 member: `0a09168702048cf34ef3fa30dffc07131b620f0a`.
- VERDICT: CLEAN (#306 source delta only; train gate pending).
- Open #306 delta findings: 0.

The lead assigns this delta in `msg_plugin-w_1791828964_ef46db`. The train merges #306 first, then #315.

I inspected both merge objects, the complete #306 merge patch, imported path changes, the shared library merge, and affected coverage context.

Static comparison confirms that the complete #306 patch equals its accepted member patch after index lines and hunk positions are removed.

The accepted patch has 44 own paths. Only two final own blobs differ from the accepted member: testkit lib.rs and route_pty_log.rs.

The base adds queue_controls. The #315 merge adds report_holds. The shared lib.rs retains both declarations, the existing BUILT_IN constant, and the approved root macro.

The added controls and report-order tests remain modules of existing crate test binaries. They add no independent binary that needs a separate root entry.

The imported route-log test locates its read by key because .44 inserts a fence. It retains #306's root macro and its existing binary identity.

The other 42 own blobs remain byte-identical. The five existing manifest repairs still require slow for the same eleven targets. No target is added or removed.

The name checker, command fixture, independent binary inventory, presence decisions, slow root filter, candidate checks, and LINKED check retain the accepted source.

### Imported base and #315 scope

The base imports the .44 pin and its fixed in_8 transcript. The train does not reintroduce the retired in_8 entry from #308.

The base's known-flaky list contains the separate ev_2 entry. Its five-field parser and list are unchanged by this train relative to current v1.

#306's coverage code uses the same existing known-flaky test-key interface. The train changes no exclusion or retry policy for that inherited entry.

The base's host, cancellation, queue-control, and signal changes remain outside #306's own patch. Their tests keep their existing crate binary roots.

#315 changes one mutation exclusion's line coordinate from 34 to 35 for the same exact LINK_CAPACITY mutation. It adds no checker-function exclusion.

The train changes no conformance list or CI profile relative to current v1. The source delta preserves #306's accepted manual name rule and mixed stale-library limit.

Sol-B owns #315 and the combined train review. This package verdict covers #306's import and shared-root composition, not #315's control behavior.

The lead reports compile and strict-conformance checks. I retain them as lead reports and accept no completed train gate here.

The final train gate, applicable evidence receipts, final bodies, and lead merge verification remain due. This CLEAN accepts no later head. I ran no execution job.

## Core #308 — resolved import and obsolete-entry closure

- Exact head: `c3bd7fcd17861e6f164bbda5eb38638cab71f00e`.
- Source and gate base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- VERDICT: CLEAN (0 open on this exact head).

I read the complete three-file own delta, conflict resolution, complete body, canonical base-check report, and completed gate.

The canonical report `gates/botster-core-stage1-gs-flaky-signature-base-merge-check-2ac0be6f-to-c3bd7fcd.log` fails conditions 1, 2, and 3.

The conflict paths are the known-flaky list and parser. This explicit source delta review covers their resolved content. The report remains FAIL.

The merge initially retains the old in_8 entry and the imported ev_2 entry. The following source commit removes in_8 after the .44 pin.

GS-FS-T3-1 HIGH is closed on this exact head. The obsolete entry is absent, and the repository test and comment no longer require it.

The final list contains only the base's ev_2 entry. Static TOML comparison verifies that every earlier entry field is preserved.

The added signature names gate step slow, symptom step_timeout, seed any, and text `step=71 limit=step_timeout`. It matches the signature already recorded in the entry's diagnosis.

The entry retains owner RC and removal by #316. It adds no test exclusion beyond the accepted base entry and grants no retry authorization.

The repository test requires the exact ev_2 binary, test, and removal trigger. It retains the base's test name and correct current-list comment.

The signature reader, removal syntax, stage filters, absent check, and report behavior retain their accepted source. The CI file equals the earlier accepted #308 member.

Only the three own paths differ from current v1. No contracts pin, pending status, timeout, mutation exclusion, or product path changes in this own delta.

### Completed exact-head evidence

Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-gs-flaky-signature-c3bd7fcd-pool-20261012-111432-79517.log`.

The header names this exact head and base. The lists step names the same merge base. All ten steps PASS in 444.6 seconds on MS-A1.

Default passes 1,957 tests. Slow passes 633 tests. Both in_8 and ev_2 pass in both tiers; the list hides neither ordinary slow result.

Mutation stage 1 has twenty-five caught, two unviable, zero missed, and zero timeout. Stage 2 does not run because stage 1 misses none.

Static log analysis matches all twenty-five raw catches to their citations. Every catcher has a normal PASS. No FLAKY result appears.

The fuzz step reports no changed decoder-harness crate. The job and wrapper exit 0 after 517 seconds, with one second queued and 516 seconds running.

The complete body records the conflict resolution, retirement, inherited signature, canonical FAIL, and completed gate. Earlier local timeouts remain author reports, not green evidence.

Both earlier LOW findings remain closed. No source, body, or supplied-evidence finding remains. The schema still supplies no automatic failure matcher.

The integration receipt and authorized final delivery procedure remain due. This CLEAN accepts no later head or retry. I ran no execution job.

### Sol-B train 5 source receipt

I read both complete artifacts at `961f2be2a8c07f19ec09de7dfc9f8883b2bcbf2d`: `verdicts/stage1/train-5.md` and `verdicts/stage1/gs-built-in-coverage.md`.

Sol-B accepts #306's source delta and the combined train on the same exact head, tree, and base. No source finding remains in that scope.

Sol-B maps all 46 new #307/#315 tests to existing guarded binaries: 43 default library tests and three selected slow-control tests.

Both reviews preserve the imported pin, inherited EV-2 policy, unchanged accounting, and accepted root-check limits. The LINK_CAPACITY exclusion shift adds no checker exclusion.

The final train gate, bodies, member receipts, and lead merge checks remain due. This receipt accepts no running result or additional retry.

## Core #308 — late body finding

- Exact head remains `c3bd7fcd17861e6f164bbda5eb38638cab71f00e`.
- Base remains `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- VERDICT: NOT CLEAN (1 LOW open).

Sol-B identifies an outdated severity label in the complete body. This finding supersedes the earlier exact-head CLEAN until closure.

### GS-FS-R3-1 — LOW — OPEN: the body retains Sol-B's earlier severity label

The Train 3 paragraph labels T3-I2 MEDIUM. Sol-B's final train-3 record classifies that finding HIGH.

Required change: change the label to HIGH or omit it. The body must reflect the final recorded finding.

The source repair still closes GS-FS-T3-1. The completed gate and all earlier source conclusions remain unchanged.

This body-only correction requires no source commit or new gate. I sent the finding to GS and Sol-B. I ran no execution job.

## Core #308 — body correction and superseding verdict

- Exact head: `c3bd7fcd17861e6f164bbda5eb38638cab71f00e`.
- Base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- VERDICT: CLEAN (0 open).

I read the complete corrected live body. It now labels Sol-B T3-I2 HIGH. GS-FS-R3-1 LOW is closed.

The head and base remain unchanged. The accepted source repair and completed gate retain their exact-head scope. No new gate is required.

This verdict supersedes the late NOT CLEAN entry. All findings, including LOW, are closed. The integration receipt and final delivery procedure remain due.

I ran no execution job.

### Sol-B resolved-signature receipt

I read the complete #308 integration artifact and train-3 remediation note at `1ab4c96393e7e87f8afbd49da04225fe4bfa4697`.

Sol-B accepts the same exact replacement head and completed gate. Both reviews close the obsolete-entry HIGH finding and the corrected severity-label LOW finding.

No finding changes. The initial integration receipt is complete. The abandoned train remains unaccepted, and its history is unchanged.

The authorized later delivery, applicable delta, final gate, and lead merge checks remain due. This receipt authorizes no retry.

## Train 5 — #306 final package receipt

- Exact train head: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
- Tree: `42e00e9808db9faec524e2404f67302bee0d66c5`.
- Gate base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- Frozen member: `0a09168702048cf34ef3fa30dffc07131b620f0a`.
- VERDICT: CLEAN (#306 final package on this exact train head; 0 open).

This receipt adds completed gate evidence to the accepted train source delta. The earlier complete source review and its scope remain applicable.

Gate log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-train-5-2683de7c-pool-20261012-112942-61568.log`.

I computed its SHA256: `bcb49fb7a9644f33b0a5f1b2dbc0e16dda3fb6cd84f1a4f291bfc2427375d536`. The header names the exact train head and base.

All ten steps PASS in 1,747.7 seconds. The job exits 0 after 1,854 seconds on MS-A1, with one second queued and 1,853 seconds running.

The default tier passes 1,998 tests. The slow tier passes 663 tests. The name check scans 251 Rust files and passes.

Mutation stage 1 has 176 mutants: 147 caught, fourteen missed, zero timeout, and fifteen unviable. Stage 2 catches all fourteen misses.

The final result has 161 caught, zero missed, zero timeout, and fifteen unviable. Static log analysis matches all 161 raw catches to citations.

Every catching test has a normal PASS. No FLAKY result appears. No runtime INCONCLUSIVE result appears.

The inherited ev_2 entry grants no catch credit. That test passes in the ordinary default and slow tiers and the unmutated stage 2 baseline.

The four decoder fuzz runs complete. The completed gate preserves the accepted root-check coverage, unchanged mutation verdict, and recorded design limits.

I read the complete updated #306 body. It retains the frozen member and prior evidence, then names this final train head, tree, base, log, and counts.

All package findings, including LOW, are closed. Sol-B owns the combined train receipt. The lead must still verify the final merge tree against the gated tree.

This receipt accepts no later head or additional retry. I ran no execution job.

## Core #323 — re-entry review, round 1 and source repair

- Original head: `62877cbaeff1ffbc63e2aae7f159730cd700e47c`.
- Replacement head: `6e6ebba7bc214ed9c55d3cecec70f8af1cdf2307`.
- Base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
- VERDICT: NOT CLEAN (final body and replacement-head evidence remain due).

The lead assigns this HIGH review. The authorized exception permits a real-pending re-entry with an authority, diagnosed cause, and structural fix owner.

I read the complete one-file original patch and replacement delta. The replacement adds only the explicit empty-fix test case.

Every comment with the `# re-entry` prefix must parse. Missing, empty, repeated, or unknown fields fail. Malformed authority syntax fails.

The tag must have a nonempty, noncomment ID line directly below it. The command also rejects duplicate IDs and applies existing eligibility checks.

The exception affects only new real-pending membership. Ledger, pending, deferred, withdrawn, real-only, and testkit-proven restrictions remain in force.

The authority check validates syntax. Reviewers must verify the actual ruling or lead decision, diagnosed cause, and fix owner for each future re-entry.

The tag changes no list ID. The suite skips comment lines. The CI pending-real run uses that suite. The slow coverage reader uses parse_ids.

Base-merge condition 4 remains a removal-only check. It preserves comment bytes and rejects added lines. A re-entry therefore requires a normal delta review.

The PR adds no real-pending entry, pin change, timeout change, mutation exclusion, or gate retry policy. A future entry changes test eligibility under the authorized exception.

### GS-RP-R1-1 — LOW — CLOSED by source repair

The original body says the test covers each empty field. The original source has explicit empty re-entry and cause cases, but no empty fix case.

The replacement adds that case and checks the `fix` refusal message. The source repair closes this finding on the replacement head.

### Evidence status

The original completed pool gate names the original head and base. It passes ten steps with thirty caught and two unviable mutants.

Its wrapper takes 637 seconds. Its step total is 478.3 seconds. This gate does not validate the replacement head.

The four retained reversal logs name the original head, show each scratch edit, and show one failing test with 39 passing lists tests.

The missing-field reversal panics on a missing map key. This establishes test failure, not the intended error message. The intact parser checks missing fields before lookup.

The script restores the source after each reversal. I read the script; I did not execute it. The logs retain each failure.

The original local record has clippy exit 101 and 321 passed tests with two timeouts. It contains no evidence for the claimed corrected clippy or isolated passes.

Sol requests the missing records or corrected claims. ST states that the unsupported round 1 claim will be withdrawn.

The replacement local record has clippy PASS and 323 tests PASS. A later isolated run has one PASS and one TIMEOUT at 2.003 seconds.

The timeout names `base_merge::tests::an_unreviewed_change_after_the_merge_still_fails`. The PASS names `base_merge::tests::the_real_tier_pending_list_is_a_line_set_file`.

At the last complete body read, the earlier unsupported claims remain. The corrected body and completed replacement gate remain due.

I accept no running gate. I ran no execution job.
