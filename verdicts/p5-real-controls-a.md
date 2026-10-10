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
