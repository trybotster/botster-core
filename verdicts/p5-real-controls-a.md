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
