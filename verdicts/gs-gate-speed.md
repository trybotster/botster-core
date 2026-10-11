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
