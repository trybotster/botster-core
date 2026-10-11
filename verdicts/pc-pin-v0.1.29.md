# PC package review: PR #245

VERDICT: NOT CLEAN (1 open)

- Reviewed head: `c57ec8269f42a0d2bc3a8578ae2a8da1b7b47af4`.
- Reviewed base: `5c480db457797fa7ebecb33835a52e26a5a8490d` (`v1`).
- Branch: `stage1/pc-pin-v0.1.29`.
- Risk tier: HIGH. BUILD.md rule 4 applies to the contracts pin move.
- Open findings: 1.
- Scope: the four changed files and the effects of the contracts dependency changes.
- READY: `msg_plugin-w_1791706164_dd4bfa`.

The binding brief names plan revision 23ak, `stage1-plan.4eceef06.md`.
The reviewer verified SHA-256 `db7ba8e5c0748aa53257fb53a5e6f8923a9549c6c9871127a7d2ef2c97000813` and read all changes from 23aj.
Plan section 0 requires contracts-v0.1.29 at `65f76ab9829d7a0fda2835c10abc7ffae2c35215`.
This plan requirement supersedes the brief's older .28 tag line.
The reviewer used BUILD.md and the relevant R-56 through R-59 conditions.

## Delta review

This verdict supersedes package NOT CLEAN (2 open) commit `f774f13238d3499e9af5ae45811ff8488435ebeb`.
Previous reviewed head: `df991fdf57122c7b59b06069ab7da01c992ca175`, base `f6778ad85c01266ac49e2ededd97487da689e920`.
Delta message: `msg_plugin-w_1791706585_c829c9`.
The new head merges #247 from the current base, then corrects the ruling-range comment.

The reviewer read the full base-merge check:
`~/botster-sessions/gates/botster-core-pc-base-merge-check-df991fdf-c57ec826-20261011.log`.
The ancestry checks and automatic merge pass. Conditions 2 and 3 fail because both sides change Cargo.toml.
Both line-set conditions pass. The reviewer does not treat the failed check as automatic approval.
The reviewer read the complete source delta and checked the merged manifest values.
The new base adds a sha2 override: opt-level 3, with debug assertions and overflow checks enabled.
The reviewed head preserves that profile exactly. Its own dependency values still change only the seven contracts tags.
Cargo.lock and the other two own files are byte-identical to the previous reviewed head.
All status lists retain the current base's entries. No ID flips.
The source diff has no whitespace error. No additional source finding exists in this delta.

The previous green Linux gate covers df991fdf. It does not establish CI on c57ec826.
The implementer plans one final gate after both reviewers report CLEAN, under the lead's pool rule.
PC-PIN-1 remains open before that stage.
The implementer reports 13 failures in 30 trace runs, all at step 21. The reviewer has not received those retained logs.
The lead requested equal-load comparisons on the same base profile: old pin at 5c480db4 and new pin at c57ec826.
The implementer reports that comparison job is running. The reviewer did not read or poll the running job.
The earlier Mac evidence predates #247's sha2 optimization. The comparison must account for that changed build profile.

## PC-PIN-1 MEDIUM — An active EV-5 test fails on the real Mac tier

The focused Mac run fails `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded` at seed 6, step 21.
The transcript expects success. Core returns PendingLimit, with detail `pending_ops is 4`.
This is an active must-pass ID. The cause is not known.
The transcript is byte-identical between contracts-v0.1.27 and contracts-v0.1.29.
The pin review cannot yet classify this failure as a dependency regression or a separate Core defect.

Evidence head: `f5bb74ceab93cbb8e12d070fb15806375ce0d791`, before the final base merge and comment correction.
Log: `~/botster-sessions/gates/botster-core-pc-pin-v0.1.29-mac-f5bb74ce-20261011-005918.log`.
The complete failure is in the same path with `.step2.raw` appended.
The reviewer read the run header, step results, failure, and real-tier summary.
Testkit strict passes 206 tests with no failure. The real tier has 127 passes and one failure, with exit code 101.
The pending-real run reports no pass. The current Linux gate passes this ID on both tiers.

Five isolated Mac runs pass in `~/botster-sessions/gates/botster-core-pc-pin-v0.1.29-ev5-repeat-mac-f5bb74ce-20261011-010317.log`.
The reviewer read all five results. Those passes do not explain the failure under the full run's load.
The PR body records the open failure. The implementer reports a separate 30-run trace job to determine its cause.
The reviewer did not read or poll that running job.

Required closure: determine the cause and record the evidence.
Resolve the failed contract check through the correct owner and retain evidence under the conditions that produced the failure.
If the resolution needs a transcript change, obtain the lead and steward instruction first.
Do not change an expectation or treat the isolated passes as proof that the failure is resolved.
Status: OPEN. PIN-SI-1 below is closed after the description correction.

## PIN-SI-1 LOW — The pin description omits R-59

The new Cargo.toml comment and the PR summary list the added rulings as R-54 through R-58.
The pinned contracts-v0.1.29 also contains R-59. Plan 23ak names that ruling explicitly.
The dependency description must identify that ruling and preserve its AD-7 tally limit.
The package reviewer missed this detail in the first verdict.
Sol identified it in `msg_plugin-w_1791706439_09d0d5` and asked the implementer to correct both descriptions.

Required change: correct the ruling range to R-54 through R-59 in the new Cargo.toml comment and PR summary.
Keep the new start_held control and transcript fences identified as later work.
Keep the R-59 limit on unfenced AD-7 minimum progress in force.
Status: CLOSED on `c57ec8269f42a0d2bc3a8578ae2a8da1b7b47af4`.
The reviewer checked the new Cargo.toml comment and current PR summary. Both now name R-54 through R-59.
Both descriptions keep the control and fences in a later tag. The PR retains the unfenced AD-7 tally limit.
This correction changes no runtime code.
Every LOW finding must close before CLEAN on this HIGH PR.

## Pin and list checks

All seven manifest dependencies move from contracts-v0.1.27 to contracts-v0.1.29.
The reviewer parsed both manifests. Those tags are the only changed dependency values.
All nine contracts lock sources resolve to `65f76ab9829d7a0fda2835c10abc7ffae2c35215`.
The lockfile changes no package, version, dependency, or checksum besides those source values.
The reviewer verified the target tag's commit, including the annotated tag's dereference.

The other two changed files contain comment changes only.
Core's pending IDs, real-pending file, held records, deferred records, ledger IDs, minimum list, and real-only records match the base.
The contracts ledger, pending file, deferred file, and frozen manifest match the old tag byte for byte.
The PR flips no ID and changes no mutation exclusion.
The six IDs that require steward approval remain pending.
EV-2 and IN-5 remain pending and require #233's real program controls.
Their removal still requires the corresponding PASS evidence in #233's final gate on the new pin.

## Dependency logic and scope

The reviewer read the changed driver and harness interfaces, the seven changed transcripts, and the relevant steward rulings.
R-56 adds only await_pty fences before six transcripts' literal input reads. The literal assertions remain unchanged.
R-57 adds await_title and the sentinel bytes in EV-2. It keeps the existing event expectations and ignores the sentinel's TitleChanged.
The title wait pumps, reads terminal_state, never polls events, and never advances the clock.
Its documentation preserves the launch and adoption scope limit established in #242.
The wait uses the existing step bound and waits for the host wake when a pump has no more work.

R-58 uses the host's recorded RouteClosed reason for the ended value after a real stream end.
The driver records the first reason per handle generation and route. Both tiers use the same mapping.
The driver ignores a route endpoint's claimed cause. This dependency change edits no transcript expectation.
The Core pin PR adds no alternate implementation of these driver steps.

R-59 does not accept the unfenced AD-7 start-hold proof for minimum progress.
The start_held control and its fences remain separate follow-up work. This review awards no minimum progress from this pin.
The later R-60 and R-61 work also remains outside this pin PR.
The Prior art note correctly identifies a pin-only change with no old code or test port.

The Mac evidence uses the same dependency pin as the reviewed head.
The reviewer read the merge paths from f5bb74ce to df991fdf.
They change xtask base-merge code and tests, real-pending comments, and manifest comments.
They change no Core runtime, harness, driver dependency, or transcript relevant to the Mac failure.
The previous Linux gate runs on df991fdf and covers its earlier base changes.
The new #247 base merge still requires the final gate described above.

## Linux gate

Log: `~/botster-sessions/gates/botster-core-stage1-pc-pin-v0.1.29-df991fdf-pool-20261011-010434-15999.log`.
The gate header names the previous reviewed head and base.
It runs `env CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 cargo xtask ci` on the Linux pool.
All ten steps pass. The diff has no mutant and changes no crate with a decoder harness.
The gate exits with code 0 after 254 seconds. The CI summary reports 191.4 seconds.
The pending-real report lists 77 IDs and zero passes. This gate supports no pending-ID removal.
The reviewer read the header, EV-5 PASS records on both tiers, mutation result, and complete final summary.
The reviewer also checked the current PR head, base, body, and source diff.

The reviewer ran no build, test, gate, mutation, reversal, or executable merge-check job.
The integration reviewer must review this HIGH PR. This verdict grants no merge approval.
The #233 mutation finding remains open in its separate verdict. The PC group is not complete.
