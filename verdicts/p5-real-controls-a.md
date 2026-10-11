# P5 real process and storage controls review

## PR #222 — Round 1 — 2026-10-10

Head: `6b720cb67b8414314b09d946fe90380eb34fdf24`.
Tree: `d3adb0fa6a498dc2b171d9333903c4852944f4eb`.
Parent: `bf95b68796c3d75b3d43c0ff62e5482700375247`.
PR and gate base: `b41e88be535c7e1c62ddf272c4295a87b94f5685`.
The head contains the base. `git diff --check` finds no whitespace error.

Risk tier: HIGH, under BUILD.md rule 3. The PR changes the shared testkit crate.
The reviewer read all seven changed files, the approved control plan, the PR body, and the supplied evidence.
The reviewer read the six newly passing tagged transcripts and the relevant process and worker code.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### Open finding

**RC-A-F1 — HIGH — OPEN: an unread launch report produces a false liveness result.**

At `crates/botster-core-testkit/src/real.rs:366`, `payload_alive` returns `{alive:false}` for `Launch::Pending`.
The tap sets `Pending` when it spawns the worker.
The tap changes that state only when it reads the worker's `Launched` report.
The worker starts the real payload before it queues that report, as `worker.rs:593-607` shows.
The payload can therefore run while the host has not read the report and the tap still records `Pending`.
Core also writes no payload-bearing Running row after a normal start; its in-memory launch result does not repair this observation.
Thus the control can report a dead payload while the real payload runs.
An unread report does not prove absence. It cannot supply a successful negative process observation.

Required change: preserve an unknown result until an identity-checked observation can establish payload liveness or absence.
Distinguish a launch that cannot yet reach the worker from a sent launch whose report remains unread, or use another real process observation.
Add a real proof with a live payload whose `Launched` report the current tap did not observe.
The proof must fail if the control reports `{alive:false}` in that condition.
Retain the known payload identity check and the non-zombie process check.

P5 accepted the finding. Astra independently confirmed it.
P5 proposes a shared `live_children` observation with worker identity checks before and after the process listing.
P5 also proposes a reopen proof whose new tap never read `Launched`, while an independently recorded payload identity remains live.
The reviewer agreed that this approach addresses the finding, subject to replacement source and evidence review.
This proposed fix is not present in the reviewed head. The finding remains open until replacement READY and review.

### Other source and proof checks

The three controls run only on the tapped harness. The plain harness returns `Unsupported`.
Arguments use the shared strict parser and deny unknown fields.
The corruption control reads the exact stored row, damages its bytes with the shared `damaged` helper, and checks production decoding rejects them.
Its write reaches the inner storage edge and preserves the record of bytes Core wrote.
The testkit uses the same half-row damage as before.

Known payload liveness uses the production identity check and the shared live-group member check, which excludes zombies.
The tap observes inbound frames without changing their bytes and retains the first reported payload identity.
The decoder loop handles partial frames and multiple frames in one read.
Malformed or unexpected frames end observation; adopted links retain their existing observation state.
RC-A-F1 concerns the meaning of the pending observation, not byte forwarding.

`lose_worker` reads the recorded worker identity and accepts only `worker_gone`.
The inner identity check must return `Matches` before the control sends `KILL` and a wake.
Production `Children::signal_group` checks the identity again.
The worker leads its own group; its payload leads a separate group.
The default proof rejects absent and reused identities without a signal or wake.
The real proof verifies separate groups and a mismatched start time, then verifies worker loss and refusal of a second kill.
The real proof also verifies payload liveness after normal start and absence after Stop, plus stored row corruption.
The zombie proof uses the shared owned-child and deadline helpers.

### Lists and supplied evidence

The real-pending set loses exactly six IDs and gains none: 87 becomes 81.
All six have exact-head real PASS lines. The two minimum IDs are `a10_2_corrupted_row_is_lost_registry_corrupt` and `ad_1_running_adopts_running`.
The other removals are `a2_1_read_of_created_or_lost_is_wrong_state`, `ev_5_stop_kills_while_queue_full`,
`lc_12_stop_all_leaves_created_exited_lost`, and `tm_6_more_means_runnable_not_blocked`.
The moved route and program entries remain pending at their newly reached controls.
The probe log shows their actual unsupported-control messages, not only nextest's pending-trial PASS labels.
`a6_1_withheld_control_link_gives_worker_unreachable_not_worker_gone` remains pending at `withhold_control_link`.
The ledger, Core pending and deferred files, canonical minimum, and real-only file equal the base bytes.

Probe:
`~/botster-sessions/shared/core-stage1/gate-logs/ctl-a-probe-v1.log`, at parent `bf95b68796c3d75b3d43c0ff62e5482700375247`.
Only the real-pending list changes from that parent to the reviewed head.
The reviewer read all six newly passing outcomes and the three relevant unsupported-control outcomes.

Exact-head gate:
`~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-6b720cb6-pool-20261010-091048-97964.log`.
All ten stages PASS; job and gate exit zero after 636 seconds on msa1.
Default: 1471 passed, 497 skipped. Slow: 386 passed, 1930 skipped.
Testkit conformance passes 193 IDs; real conformance passes 112 IDs.
The report retains 81 real-pending IDs, with zero newly passing pending IDs.
Minimum counts are testkit 50 / 69, real-passing 31 / 68, and real-accepted 31 / 69.
Both default mutation steps report 33 tested, 27 caught, six unviable, zero missed, and zero timeout.

Manual slow-feature evidence:
`~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-6b720cb6-pool-20261010-092200-16909.log`.
The command uses `--no-config --in-place --features slow`, the exact base diff, the slow profile, and the fail-fast setting.
The reviewer parsed all 53 records and read every mutant's failure details.
The baseline and mutant test commands select testkit with its slow feature; the baseline passes all 328 selected tests.
Results: 52 mutants, 45 caught, seven unviable, zero missed, and zero timeout.
Every catch has a named test failure. Every unviable mutant has a missing `Default` implementation.
The detailed log retains the complete outcomes and all per-mutant logs, including the baseline.
The job and gate exit zero after 194 seconds on msa1.
This evidence supports the existing interim slow-module exclusion. It does not prove the missing unread-report condition in RC-A-F1.

RC-A-F1 remains the sole open finding. P5 and Astra received it directly.
Astra's integration Round 1 verdict is `4043c23cf553efc406e1f99122d8168ac04a32b3`, `verdicts/stage1/p5-real-controls-a.md`.
The reviewer read that committed artifact; its sole HIGH finding agrees with RC-A-F1.
The reviewer awaits replacement READY and sends no NOT CLEAN report to the lead.

VERDICT: NOT CLEAN


## Round 2 — 2026-10-10

Head: `a6aa195660d9021995a0487db403c57534415103`.
Tree: `8e74ec9088c6fc8f4bfef4d5fe6f6678752db0e9`.
Parents: `8b6c652943502032143bd0a8ea32fd243969956d` and `7caf3457a04bd37f5d4e844db1900525eea7c8ec`.
PR and gate base: `7caf3457a04bd37f5d4e844db1900525eea7c8ec`. The head contains this base.
READY: `msg_plugin-w_1791655430_fc7222` from P5.
Tier: HIGH, shared testkit changes. The correction also changes the shared test-process crate.

Authority: approved plan 23x, `~/botster-sessions/pins/stage1-plan.c0b6f32e.md`.
Verified SHA256: `ba89898ba66c2a4144a1978d0d89e791469b0b1645da167bf5e37178ae07f63f`.
The current lead handoff explicitly requires #222 to wait for its focused Mac mutation run.
Sol now holds the integration seat. Astra remains available only for handover.

### RC-A-F1 — HIGH — CLOSED

The tap now records only payload identities from reports that it reads.
The `Launch::Pending` state is removed. An unread report no longer establishes payload absence.
A known payload retains the identity check and the check for a live process that is not a zombie.
The shared `lives` helper combines these checks for both payloads and workers.

For an unknown payload, the harness lists the recorded worker's live children.
The worker has at most one child, its payload.
After the listing, the harness requires the recorded worker's identity to match and the worker to run.
If that check fails, the harness returns `Bad`, because an ended worker's payload can outlive the worker.
If the check passes, the harness reports whether the worker has a live child.
The identity check after the listing is sufficient.
A matching, live worker afterward establishes that the recorded process existed throughout the listing.
The missing check before the listing therefore does not leave RC-A-F1 open.

The real proof independently records the payload identity on handle `a`, drops that handle, and opens handle `b`.
The proof verifies that the stored row names no payload and that `b` read no `Launched` report.
The control returns true while the independently identified payload runs.
The proof kills that payload by identity, waits for its exit within the existing cleanup bound, and verifies false.
The proof then kills the worker, waits for its exit, and verifies `Bad`.
The exact-head Linux gate selects this proof and passes it in 1.095 seconds.

Linux `live_children` reuses the process scan with a parent filter.
The parser reads state, parent, and group after the complete command field.
The platform proof covers a live child, a child with no children, an unreaped zombie, and a reaped child.
The default proof also verifies that the test process is a live child of its parent.
The Mac adapter uses libproc's parent filter and clears stale `errno` before the listing.
The reviewer checked the local libproc 0.14.11 implementation and its parent-filter API.
Native evidence for this adapter remains required below.

### RC-A-F2 — HIGH — OPEN — Mac mutation evidence is missing

Plan section 8 requires a focused Mac mutation log for changed macOS-only code.
This PR changes `platform/macos.rs`, including `live_children` and the shared `live_of` helper.
The supplied Linux manual run reports ten misses in that file.
Linux does not compile the file, so those outcomes cannot prove its behavior.
No completed native Mac mutation log is supplied.

The retained attempt on `c99903c4` did not run on a node.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-c99903c4-pool-20261010-095820-95517.log`.
The queue reports a Mac disk restriction, reaches its 2700-second deadline, and exits 124 after 2701 seconds.
The source file is unchanged since that attempt, but an unrun job supplies no mutation result.
The current lead handoff confirms that #222 must wait for the focused run.
The pool lead handles the disk restriction and the pending user decision.

Required evidence: the focused native Mac mutation run, its baseline, outcomes, and retained per-mutant details.
The PR must name the log. The reviewers must verify its source scope against this head.
The green Linux gate does not replace this evidence.
No further source correction is requested by this finding.

### Delta and supplied Linux evidence

The reviewer read the correction, new platform code and proofs, exact body, and the new gate evidence.
The current base removes the previously reviewed imports from #219, #224, and #223 from this PR's own delta.
The own delta has 13 files. The list changes equal Round 1 exactly.
Real pending loses six IDs, gains none, and ends at 81 IDs.
Both minimum removals remain `a10_2_corrupted_row_is_lost_registry_corrupt` and `ad_1_running_adopts_running`.
Core pending, ledger, deferred, status copies, minimum, and real-only bytes equal the base.
The Round 1 checks outside the correction remain applicable. No new source finding remains open.
`git diff --check` reports no whitespace error.

Full gate: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-a6aa1956-pool-20261010-104732-94598.log`.
All ten stages report PASS. The head and base match the READY.
Default: 1488 passed, 497 skipped. Slow: 392 passed, 1933 skipped.
Testkit conformance: 193 passes. Real conformance: 112 passes.
Real pending reports 81 IDs and zero newly passing.
Minimum: testkit 50 / 69, real-passing 31 / 68, real-accepted 31 / 69.
Both default mutation steps report 60 tested, 52 caught, eight unviable, zero missed or timeout.
The job and gate exit zero after 580 seconds on msa1.
The repeated default mutation step does not establish coverage of the slow feature.

Manual slow-feature evidence: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-a6aa1956-pool-20261010-105721-24883.log`.
The run uses the exact-base diff, no configuration exclusions, in-place edits, the slow feature, and the slow profile.
Each test command uses `--max-fail 1:immediate`.
The baseline selects both testkit and test-process packages. All 415 baseline tests pass.
The reviewer parsed all 93 records and checked the retained details.
The 92 mutants produce 73 named test failures, nine compile failures, ten Mac-only misses, and zero timeouts.
All 73 catches have failure details. All nine unviable mutants fail before their test phase.
Every mutant of `payload_alive` and `lives` is caught.
The ten misses cover `live_members`, `live_children`, and `live_of` in the uncompiled Mac file.
The manual job and gate exit 2 after 340 seconds on msa1.
The Linux results close RC-A-F1 but leave RC-A-F2 open.

The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
The package review must wait for the missing native evidence before CLEAN.

VERDICT: NOT CLEAN

## Round 3 — 2026-10-10

Reviewed head: `f775a76648d9594b45d3a8663070df79503b824a`.
Tree: `c153c3e7f7e21e80013f2c68067ed856801a907e`.
Parents: `6b1545c47e6078d7471198548547906eed511348` and `72afc28426a8d9cdf8250a954f96fd8f0f1198fa`.
PR and gate base: `72afc28426a8d9cdf8250a954f96fd8f0f1198fa`, contained by the head.
Tier: HIGH, shared testkit and test-process code.
Authority: approved plan 23ae, pin `stage1-plan.b895ad49.md`, and the accepted P5 controls plan.
The plan SHA256 is `db0522ad7a0ca5f0ede3e7b236049f11be504161d6cfa9ee9c65638d46c4ac3b`.
The remote PR head, base, and body agree with READY.

### RC-A-F1 — HIGH — CLOSED

The Round 2 correction remains unchanged.
An unread `Launched` report does not establish payload absence.
The tap retains only observed payload identities; unknown payload liveness comes from the recorded worker's live children.
The identity-and-liveness check after the listing still protects that observation.
The exact-head gate passes the independent reopen proof in 1.110 seconds.
The proof also passes in the slow mutation baseline.
The known-payload, ended-worker, and reused-identity checks remain covered.

### RC-A-F2 — HIGH — CLOSED

The required native Mac mutation evidence now exists.
The reviewer read the one-file correction at `996a17f0afab8f6509d32623cfeaf00619fd2a01` and both native runs.
The Mac file at the final head equals the corrected evidence head byte for byte.

The old Mac run reports one surviving `&&` to `||` mutant in `live_of`.
P5 reports that Darwin 27 refuses zombie BSD information with ESRCH, so zombies leave through the existing refusal path.
The correction keeps the zombie status guard as an independent early `continue`, before the group or parent predicate.
This structure preserves the decision and retains protection when a kernel supplies zombie information.
No new source finding remains.

Old log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-7351d67e-pool-20261010-144633-33392.log`.
The reviewer parsed all 12 records and checked their retained logs.
Baseline: 83 pass. Eleven mutants: seven caught, three unviable, one missed, zero timeout.
The job and gate exit 2 after 128 seconds.

Corrected log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-996a17f0-pool-20261010-145010-46992.log`.
The command prebuilds the candidate and selects test-process with the slow feature and profile.
It uses `--no-config`, `--in-place`, the Mac file, `-F live_`, and immediate fail-fast behavior.
The reviewer parsed all 11 records and checked their retained logs.
Baseline: 83 pass. Ten mutants: seven named test failures and three missing-Default compile failures.
All catches exit 100 in the test phase. All unviable mutants exit 101 in the build phase.
Zero mutants are missed, and no mutant times out.
The job and gate exit zero after 32 seconds, including one queued second.
The changed Mac functions are within this scope.

### Final delta and lists

The five commits after the checked native correction only merge `v1`.
The own patch ID remains `db11d2469d4053b6f854249192b9d813927fdd1d` against the current base.
All twelve own source/list files equal the checked correction.
Cargo.lock imports the v0.1.27 pin; its own change is unchanged.
Sol integration independently confirms all five automatic merge trees and the unchanged normalized own delta.

Real pending remains 81 IDs: six removals from the base, no additions.
All six removed IDs have actual PASS lines in the exact-head real tier.
The ledger, Core pending, deferred/status copies, minimum list, and real-only map equal the current base bytes.
`git diff --check` passes.
The previously accepted source checks remain applicable.
No finding remains open, including LOW.

### Exact-head Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-f775a766-pool-20261010-164259-46992.log`.

The header names the exact head and current base.
The command is the current plain gate command, `cargo xtask ci`.
All ten stages pass.
Default: 1508 passed, 497 skipped. Slow: 395 passed, 1953 skipped.
Testkit conformance: 193 actual passes. Real conformance: 112 actual passes.
Real pending reports 81 IDs and zero newly passing IDs.
Ledger: 690 IDs. Core pending: 477 IDs.
Minimum: testkit 50 / 69, real-passing 31 / 68, real-accepted 31 / 69.
The live-child proof passes in 0.003 seconds and runs again in the slow mutation baseline.

Stage 1: 82 mutants, 52 caught, 22 missed, eight unviable, zero timeout, in 279.8 seconds.
The slow baseline passes all 395 selected tests in 139.7 seconds, including its build.
Stage 2 tests exactly the 22 misses: 21 caught, one unviable, zero missed or timeout, in 892.9 seconds.
Final verdict: 82 tested, 73 caught, nine unviable, zero missed or timeout.
The job exits zero after 1503 seconds; the gate exits zero after 1504 seconds on msa1.
The gate now supplies slow-feature mutation coverage directly.

The final prebuild installs `contracts-v0.1.27#b161db87` from the per-tag probe build introduced by #235.
It reuses the preceding build on the same pool volume.
Supplementary log: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-a-6b1545c4-pool-20261010-161751-86815.log`.
That green gate shows both probe-script compilation lines at the exact tag.
The reviewer checked both logs and the source that keys the probe build directory by tag.

Lead ruling `msg_plugin-w_1791677444_a628f9` accepts this evidence.
The lead states that its compilation-line check ended after #235 merged; the check applied only during the stale-directory period.
A reused build at the same tag is correct under the per-tag build rule.

The reviewer changed no product code and ran no builds, tests, gates, mutation jobs, or base-merge checks.
This package verdict applies to the exact head above.
The separate integration verdict is still required before merge.

VERDICT: CLEAN
