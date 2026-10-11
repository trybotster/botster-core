# RC real controls

## #246: cause check before READY

Date: 2026-10-11.
Head: `a8d9a156401288226d0ad8922fff65db64ddb4da`.
Base: `8dab35d1`.
Risk: HIGH. The change modifies the shared testkit.
Status: This is a limited cause check. The implementer has not sent READY. This check gives no CLEAN verdict.

**#246's change does not alter the cause of the `ev_5` step-21 `PendingLimit` failure.**

The reviewer read all four changed files against the base. The reviewer also read the transcript at `contracts-v0.1.29`.

- The transcript requires only `edges_quiet`. It never calls `fail_handoff`.
- Each new tap starts with `fail_handoff: false`. Only `fail_next_handoff` arms that flag.
- During this transcript, `link_send_descriptor` takes the false flag and calls the same inner edge with the same arguments.
- The change adds no wait, event poll, or operation completion on this path. It does not change slot admission or slot release.
- R-63 identifies the recovery race: `poll_all` can run before worker completions arrive. `await_quiet` does not wait for those completions.
- R-64 identifies the separate v1 trigger: the worker cannot receive descriptors. #246 does not change the worker's descriptor intake.

The reviewer read this gate log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-fail-handoff-a8d9a156-pool-20261011-020457-95993.log`.
Its summary reports a passing slow step, a failing unmutated stage-2 baseline, and no fuzz run.
The failing baseline names `conf::ev_5_repeated_attach_detach_without_polling_stays_bounded`.
The implementer supplied the step-21 error detail. The gate summary does not contain that detail.

This check satisfies only the reviewer's cause confirmation in plan 23al, section 8.
The lead must verify the trace evidence, reproduction on plain v1, and written approval for one re-gate.
The reviewer ran no test or gate. The full review, all exclusion checks, and exact-head evidence review remain pending.

## #246: round 1

Date: 2026-10-11.
Head: `a8d9a156401288226d0ad8922fff65db64ddb4da`.
Base: `8dab35d14f7038d3cc0598e2cde8da207bd1b723`.
Risk: HIGH. The change modifies the shared testkit.

**VERDICT: NOT CLEAN (1 open)**

### RC-FH-1 — LOW — The new method has the hold lookup's comment

Status: CLOSED at `b168c192ff58459b4671834bf01b87cc823c6fb7`. See round 2 below.
Evidence: `crates/botster-core-testkit/src/edge_tap.rs:424` at the reviewed head.
The comment says, "The hold whose `Launch` met it on `link`."
The insertion attaches that comment to `fail_next_handoff`, which takes no `link` and returns no hold.
The comment belongs to `hold_on`, now at line 432.

Required change: Move that comment directly above `hold_on`.
Keep the failure-control comment directly above `fail_next_handoff`.

### Review checks

- The PR states HIGH and includes a Prior art note.
- The control matches the `fail_handoff` row at `contracts-v0.1.29`: fail the next endpoint handoff at the edge.
- The fault returns the same endpoint with `DescriptorSendError::Failed`. The inner edge takes no bytes or descriptor.
- The existing host driver drops the endpoint, removes the whole handoff frame, and queues `Input::HandoffFailed`.
- The hold check runs first. A blocked handoff does not consume the failure flag. A failed handoff consumes it once.
- The control belongs to one handle's tap. The plain harness reports it as unsupported.
- This is the fault injection that the control row defines. It adds no model of process history or terminal state.
- R-58 supplies the host's `route_ended` observation. The unchanged transcript also requires the real client's stream to end.
- The new edge test checks returned endpoint ownership and the fake edge's recorded calls and bytes. Those records observe the edge boundary.
- The new tests do not assert Core's private state. They add no sleep, polling loop, global lock, process spawn, or timer.
- The real transcript uses the existing guarded harness. The change does not alter process ownership or cleanup.
- The pending list removes only `conf::dp_2_failed_handoff_closes_route_handoff_failed`. The second-route transcript stays pending.

### Evidence and exclusions

The reviewer read the exact-head gate log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-fail-handoff-a8d9a156-pool-20261011-021055-12442.log`.
The header names the full reviewed head and base above. Every step passed.
DP-2 passed on both tiers. Mutation testing caught all 9 mutants, including 6 in stage 2.
No mutant was missed, timed out, or unviable.

The reviewer read the focused Mac log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-fail-handoff-merged-mac-20261011-015021.log`.
DP-2 passed on both tiers. The second-route transcript remained inconclusive on the real tier.
The log names `11db5ae43bd59ce8d0fa1eceae5cf1f646a391b0` plus the pending-list edit.
That edit is exactly the diff to `9fb4a854`.
The later commits change only the test's initializer and inherited gate code. They do not change the control or real process path.

The reviewer read every exclusion and reason in `.cargo/mutants.toml` at the exact head.
The file is byte-identical to the base. It contains 206 regular expressions and 2 test globs.
No expression targets `edge_tap.rs` or testkit `real.rs`. Neither test glob covers those implementation files.
The gate reports successful citation, decision, and stale-exclusion checks. Its count includes one inherited off-Mac expression.
The new functions and changed control dispatch remain subject to mutation testing.

The reviewer ran no test or gate. RC-FH-1 must close before CLEAN.

## #246: round 2

Date: 2026-10-11.
Head: `b168c192ff58459b4671834bf01b87cc823c6fb7`.
Previous reviewed head: `a8d9a156401288226d0ad8922fff65db64ddb4da`.
Risk: HIGH.

**VERDICT: CLEAN**

Open findings: 0. RC-FH-1 is closed.
The reviewer checked the complete delta and confirmed the PR's head through GitHub.
One commit moves one documentation line. No executable code, test, pin, pending list, or exclusion changes.
At `edge_tap.rs:431`, the hold lookup's comment now directly precedes `hold_on`.
The comment for `fail_next_handoff` remains directly above that method.
The round-1 logic review and evidence retain their stated scope.

The green gate cited in round 1 ran on `a8d9a156`, not this head.
The final gate must pass on the final head before merge. The required integration review remains separate.
Any further implementation change requires a delta review. A qualifying base merge follows the plan's base-merge rule.
The reviewer ran no test or gate.

### Integration review received

The integration reviewer reports source CLEAN at `b168c192ff58459b4671834bf01b87cc823c6fb7`.
Integration verdict: `745964561c19243f5fe0e032895e9c4c61946498`, `verdicts/stage1/rc-fail-handoff.md`.
The report arrived in `msg_plugin-w_1791710645_32356d`.
The integration reviewer confirms the comment correction and the focused Mac source equivalence.
The final exact-head gate remains required before merge.

## #255: round 1

Date: 2026-10-11.
Head: `81304f8588cac4c98deffad2e05ce6fa5fb70b3a`.
Base: `517c2235`.
Risk: HIGH. The change modifies the shared testkit.
Authority: R-60 and plan 23am, section 0, item 1. This PR adds the control before the pin moves.

**VERDICT: NOT CLEAN (2 open)**

### RC-PR-1 — MEDIUM — The real control accepts a removed session's historical row

Status: CLOSED at `15633946e020a9d092ff2fcb8d0bf20560efd202`. See round 2 below.
Evidence: `crates/botster-core-testkit/src/real/program.rs:539` at the reviewed head.
The new `program_ready` arm uses `session_processes` to decide whether the session has a row.
That method reads the tap's historical row table. Its documentation at `real.rs:323` states that a row outlives removal.
The tap's `delete_row` at `edge_tap.rs:557` deletes storage without removing the historical entry.

Sequence: Create a session. Complete Remove. Call `program_ready` for the removed session on the same open handle.
The real control finds the historical row and returns `{ready: false}` for a session that never started.
For a session whose probe connected, the historical probe entry can instead produce `{ready: true}`.
The testkit returns `Bad`: its `session_row` at `controls.rs:73` reads current storage, where Remove deleted the row.
This contradicts the new arm's stated behavior, "no row is `Bad`, as on the testkit."
It also lets the real readiness fence accept a session that no longer exists.

Required change: Read and decode the current stored row for `program_ready`.
The existing `RealCoreHarness::stored` helper provides that read through the storage edge.
Keep historical process records for controls that need them.
Add a regression through the harness control: after Remove completes, `program_ready` returns `Bad` on both tiers.
The regression must assert control behavior, not the historical table's private state.

### Other checks and evidence

- The PR states HIGH and includes a Prior art note.
- R-60 defines both forms. The real read does not wait for connection. The testkit answers true at once for an existing row.
- The hello inserts the probe before it signals the watched host wakes. The existing wake function signals every live watcher.
- The new real-session test uses the guarded production worker and probe. The change adds no production test hook.
- The new socket test fences the hello's wake with an acknowledged request. It uses no sleep or polling loop.
- The changed room test consumes the hello's wake before it waits for the input report's wake. Its new timer is a marked deadline.
- The pin, transcripts, and pending lists do not change in this PR.

The exact-head Linux log is:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-program-ready-81304f85-pool-20261011-025755-50233.log`.
Every step passed. All 7 mutants were caught, including 5 in stage 2. None was missed or timed out.

The exact-head Mac log is:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-program-ready-81304f85-mac-20261011-025659.log`.
It reports 410 passes and 1 failure. The new readiness tests pass.
The existing zombie test fails because `identity_state` returns `Absent`, while the test expects `Matches`.
The implementer reports the same failure on plain v1. The PR does not change that test or identity implementation.
Bug catalog B9 records the same macOS identity distinction. This review does not treat the Mac run as entirely green.

The mutation configuration is byte-identical to the configuration already read in the #246 review.
The reviewer checked every exclusion against the changed implementation paths. No expression targets those paths.
The existing test globs do not cover the changed implementation files.

### RC-PR-2 — LOW — The hello test duplicates readiness proof through a private helper

Status: CLOSED at `15633946e020a9d092ff2fcb8d0bf20560efd202`. See round 2 below.
Evidence: `crates/botster-core-testkit/src/real/program.rs:782`, `the_hello_records_the_probe_and_wakes_the_hosts`.
The test directly asserts `Programs::has_connected` before and after the hello.
Those assertions read the helper that checks the private probe table.
The real-session test already proves readiness through the harness's `program_ready` control.
BUILD.md Testing rule 3 excludes duplicate proof and helper-internal tests.

Required change: Remove the two `has_connected` assertions from the hello test.
Keep the protocol acknowledgement and the observable `HostWake` signal assertion.
Update the test name, comment, and PR description to state the remaining wake proof.
Keep readiness assertions in the tests that call the harness control.
The integration reviewer identified this finding in `msg_plugin-w_1791713493_733457`. The package reviewer accepts it.

### Later base merge

The implementer then submitted `b013fdeb77341a142321c256ad57d1ecc078aed2`, which merges base `5ac94fab`.
The PR's four-file change against that base still contains both findings.
The reviewer read the inherited `real.rs` delta. It does not change either finding's cause.
Both the PR and the base modify `real.rs`, so the plan's automatic disjoint-path carry-over does not apply.
Matching patch identifiers and separate hunks do not satisfy that path rule.
This round remains NOT CLEAN with 2 open findings on the merged head as well.

The reviewer ran no test or gate. Both findings must close before CLEAN.

### Integration verdict received

The integration reviewer reports NOT CLEAN with the same two open findings at `b013fdeb77341a142321c256ad57d1ecc078aed2`.
Integration verdict: `25a1ca56597ed6ee463e715ab2983b297de873c2`, `verdicts/stage1/rc-program-ready.md`.
The report arrived in `msg_plugin-w_1791713723_509708`.
The integration reviewer also reports reading the retained plain-v1 Mac log.
The prior Linux gate does not cover the imported head or a later correction.

## #255: round 2

Date: 2026-10-11.
Head: `15633946e020a9d092ff2fcb8d0bf20560efd202`.
Previous reviewed head: `b013fdeb77341a142321c256ad57d1ecc078aed2`.
Base: `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
Risk: HIGH.

**VERDICT: CLEAN**

Open findings: 0. RC-PR-1 and RC-PR-2 are closed.
The reviewer read the complete correction and checked the PR's exact head through GitHub.
The reviewer also read the inherited start-control implementation that the earlier base merge added.

RC-PR-1: The real control now calls `stored` and uses the decoded current row's worker identity.
The historical process record no longer determines whether the session has a row.
Both tiers check `Bad` after removal through the harness control.
The real regression uses a never-started session, so the earlier cached-row implementation would incorrectly return `{ready: false}`.

RC-PR-2: The hello test removes both direct readiness-helper assertions.
Its acknowledgement fences the hello's wake. The remaining assertion checks one observable `HostWake` signal.
The test name, comment, and PR body describe that wake proof.
Readiness remains tested through the harness control, including a recorded worker whose probe has not connected.
The armed launch hold prevents payload launch until the test releases it.
The test checks a negative readiness state; it does not claim a crash or adoption at a held lifecycle step.

The correction adds no production hook, transcript edit, pin move, pending-list change, or mutation exclusion.
The mutation configuration remains unchanged from the earlier full exclusion review.

The reviewer read the exact-head Mac checks:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-program-ready-15633946-mac-checks-20261011-031421.log`.
Formatting, clippy, taint, and lists passed. The corrected tests passed.
The crate run reports 410 passes and the same one zombie-test failure.

The reviewer also read the plain-v1 log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-plain-v1-517c2235-zombie-mac-20261011-031147.log`.
It reproduces the identical `Absent` versus `Matches` assertion failure on clean base `517c2235`.
That test and identity implementation remain unchanged by this PR. The failure remains explicit; neither Mac run is entirely green.

The initial Linux gate does not cover this head. The final gate must pass on the exact final head before merge.
Integration CLEAN remains separately required. Any further implementation change requires a delta review.
The reviewer ran no test or gate.

### Integration CLEAN received

The integration reviewer reports CLEAN at `15633946e020a9d092ff2fcb8d0bf20560efd202`, with 0 open findings.
Integration verdict: `0efa400e`, `verdicts/stage1/rc-program-ready.md`, round 2.
The report arrived in `msg_plugin-w_1791714268_35bab6`.
The final full gate remains required on the exact final head before merge.

### Final gate receipt checked

The reviewer checked the gate header and final result for head `15633946e020a9d092ff2fcb8d0bf20560efd202`.
The recorded base is `5ac94fabf41909a733267a62b692c1a4ee72bf62`.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-program-ready-15633946-pool-20261011-032441-61505.log`.
Every step passed. All 7 mutants were caught, including 5 in stage 2. No mutant was missed or timed out.
The gate ended with exit 0. Both source CLEAN verdicts now have an exact-head green gate.
This receipt does not establish a merge. The reviewer ran no gate.

### Integration completion and merge receipt

The integration reviewer reports completion CLEAN at the same exact head.
Integration verdict: `14f1d685979d04da55dbb86d704217149053ef52`, `verdicts/stage1/rc-program-ready.md`, round 3.
The report arrived in `msg_plugin-w_1791714711_df4927`.
The package reviewer checked merge commit `c8907b74bd247002784843e1a12f6487b08c579f` in the local Git object store.
Its parents are base `5ac94fabf41909a733267a62b692c1a4ee72bf62` and reviewed head `15633946e020a9d092ff2fcb8d0bf20560efd202`.
Its tree has no difference from the reviewed and gated head.
The Mac failure limits remain explicit. The reviewer ran no test or gate.

## #259: round 1

Date: 2026-10-11.
Head: `6b20f6f2116f34526cf6ebf09e263776fff35123`.
Base: `d86bfcc747718452efe57b2aa207492acde96573`.
Risk: HIGH.

**VERDICT: CLEAN**

Open findings: 0.
The reviewer read the complete four-file diff and checked the exact PR head through GitHub.
Plan 23an, section 0 item (2), authorizes this one pin move after the two controls land.
The base contains `program_ready` and `start_held`. The base is the head's merge base.

All seven workspace dependencies name `contracts-v0.1.32`.
All nine contracts entries in the lock name commit `91394e726b1bd697307d0d5b05712b876f9189be`, which is that tag's commit.
The lock changes only the source references. It changes no version or dependency list.
The other changes update five pending reasons and one test comment.
The named A15, A17, A18 and A19 cases still have no transcript at the new tag.
No ID leaves a pending file.

The reviewer read the contracts delta, including the control definitions, driver handling of `UnsupportedFor`, and the transcript fences.
The fences follow R-59, R-60 and R-62. The compression payload changes replace only `0a` with `2e` in equal-length byte strings.
The three ST-6b transcripts retain their pending status. This pin does not claim a real `oracle_resume` proof.
R-62's comparator and real-form conditions still apply to that later implementation.
The deferred files, withdrawn files and BUILD.md are unchanged between the two tags.
This PR adds no Core logic, test assertion, process owner, production hook, or mutation exclusion.
The mutation configuration is byte-identical to the configuration from the complete exclusion review recorded above.

The reviewer checked both retained Mac testkit logs:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-pin32-compare-testkit-mac-20261011-033646-v1-v0.1.29.log` and
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-pin32-compare-testkit-mac-20261011-033646-pin-v0.1.32.log`.
Each normal and strict run has 690 result lines: 206 passed, 484 ignored, and 0 failed.
After sorting by ID, all 690 result lines match separately for normal and strict runs.
The raw line order differs because test completion order differs.

The reviewer read the retained Mac real-tier log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-pin32-real-mac-6b20f6f2-20261011-033646.log`.
Its header names the exact head and a local removal of six real-pending entries for this trial.
The next header line still names `contracts-v0.1.29`. That stale label limits the header's consistency.
The first header names `contracts-v0.1.32` and the exact source head. The six failed trials remain failures.
`ad_7_payload_launches_after_durable_identity` passes in the R-59 fenced form.
The three route cases are INCONCLUSIVE at steps 5, 5 and 4.
The three ST-6b cases report `UNSUPPORTED_CONTROL` for `oracle_resume`.
Those six trials are not passes, and their IDs remain pending in the committed head.

The reviewer checked the initial full gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-pin-v0132-6b20f6f2-pool-20261011-033824-56127.log`.
Its header names the exact head and base. All ten steps pass, and the final exit is 0.
The diff has no mutant. The real slow tier reports the fenced `ad_7_payload` PASS.
The real-pending count is 58, with 0 passed. The gate reports real-passing 40/68 and real-accepted 41/69.

Integration CLEAN remains required on this exact head and base.
Plan section 8 Q8 accepts the supplied green full gate as the merge gate when both CLEAN verdicts cover this exact head.
A second gate is not required for this unchanged head. A later head needs its own exact-head gate.
Any later implementation change requires a delta review, subject to the plan's base-merge rule.
The reviewer ran no test or gate.

### Integration CLEAN received

The integration reviewer reports CLEAN on head `6b20f6f2116f34526cf6ebf09e263776fff35123` and base `d86bfcc747718452efe57b2aa207492acde96573`.
Integration verdict: `aa76bc183ac652775b15aa2f17d4a4aba5dc2513`, `verdicts/stage1/rc-pin-v0132.md`.
The report arrived in `msg_plugin-w_1791715910_b5cd6f`. It has 0 open findings.
Both CLEAN verdicts now cover the supplied green gate's exact head and base. Q8 requires no repeat for this unchanged head.
No merge receipt has arrived. The reviewer ran no test or gate.

## #269: round 1

Date: 2026-10-11.
Head: `2a0c4922e93f3bb29234a90fe79fc57db0f7b48d`.
Gate base: `56fae5826f766b408abca06f99376e0506982bf9`.
Risk: HIGH.

**VERDICT: NOT CLEAN (3 open)**

The reviewer read the complete ten-file change against the gate base.
GitHub confirms this source head. Its current PR base is later: `c942593697e856017572977af0405b17de0124e2`.
The current head's gate does not cover a later base import.

### RC-OR-1 — HIGH — The second capture changes event retirement and operation admission

Evidence: `capture_layer.rs:129` calls the inner Core's `poll_events(usize::MAX)`.
It moves unrelated events into `Shared::held`. `CaptureLayer::poll_events` later delivers those events to the transcript.
This changes the subject before the transcript polls it:

- `botster-core-host/src/engine.rs:542` removes a polled completion from `ops` and frees its `WriteInput` counts.
- `botster-core-host/src/queue.rs:377` retires only events that remain in Core's queue.
- An `Activity` in `Shared::held` survives a later `RouteClosed`, although the transcript has not polled that activity.

The layer therefore frees operation slots early and prevents Core from retiring held events.
Adding the held count to `events_posted` does not restore EV-5 admission or retirement.
R-62 authorizes the second capture. It does not authorize this change to Core's observable behavior.
The integration reviewer independently confirms this finding in `msg_plugin-w_1791729359_ba487f`.

Required change: implement steward ruling R-69, supplied by the lead in `msg_plugin-w_1791729519_9eaa06`.
The reviewer read `/Users/jasonconigliari/botster-sessions/shared/steward/drafts/r69.md`.
The control begins its capture, pumps without a clock step, and polls.
Any event other than that capture's own `Completed` must give `ControlError::UnsupportedFor("events_pending")`.
That refusal is never a pass. Hold and re-inject no drained event.
On success, read every page and release the capture. Restore the open-capture count and `next_deadline`.
Do not add a harness model of queue retirement or admission.

The current three transcripts do not consume their final output events before the control.
Their final sentinel output is followed by `await_quiet`, `await_title`, and `oracle_resume`; those waits do not poll.
The lead supplied the steward's placement decision in `msg_plugin-w_1791729601_826e63`.
RC must add a waiting step on the last caused event in one contracts PR, reviewed by reviewer 016c.
The waiting step must derive its event from the clauses.
Plan 23at, `stage1-plan.70cf75fb.md`, separates that placement work from Core #269.
The lead supplied this scope correction in `msg_plugin-w_1791729740_e096ba`, with the steward's agreement.
Core #269 makes no pin move and must keep the three `oracle_resume` IDs real-pending.
P5's separate Core PR pins the combined `contracts-v0.1.38` tag after #269 lands on `v1`.
The placement check applies to the R-69 contracts PR and P5's pin PR, not to Core #269.

Closure evidence: a real-tier negative test with an already queued `Activity` must return `events_pending`.
The success proof must check the public capture count and deadline before and after the control.
The control repair, refusal proof, no held events, success cleanup, and restored pending entries close RC-OR-1 in #269.
The five Mac runs per ID without refusal and the Linux transcript gate belong to the later pin PR.

### RC-OR-2 — MEDIUM — The proofs assert private records and counters

Evidence:

- `capture_layer.rs:599` to `602` assert `recorded.session`, `recorded.mark`, `recorded.pages`, and `shared.begun`.
- `capture_layer.rs:619` and `623` assert the private capture map and its mark.
- The same test asserts the private `changed` set after `Resize` and `SetColorProfile`.
- `real/program.rs:769` to `785` assert the internal marks and the `Programs::output` record.
- The real resume test waits for `shared.captures` to become nonempty and obtains its capture ID from that private map.
- The inherited `resume_controls/tests.rs` test also asserts `ModelLog::revs` directly.

These checks violate the user's rule: no test may assert private state.
They couple the proofs to records that the harness may change without changing control behavior.

Required change: prove capture recording, the output cut, session isolation, and resize/profile refusal through control results and public completions.
Obtain capture IDs from `Completed`, not from the private map.
Use public boundary observations for forwarding, releases, event order, and capture cleanup.
Remove assertions of private maps, sets, and counters. Do not add an accessor that exposes those records for an assertion.
Private writes that inject a missing-field fault are distinct from assertions of private state.
The negative comparison proofs may keep that fault injection and assert the control's result.
The pure `known_output_history` decision tests are not private-state assertions by themselves.

### RC-OR-3 — LOW — The prior-art note omits the reasons for the new custom code

The PR's prior-art note states only that old Core has no resume oracle, comparator, or harness.
It does not explain why the new comparator and capture layer require custom code.
Pair-common requires a reason for every custom component and prefers maintained libraries.
The integration reviewer identified this omission in `msg_plugin-w_1791729665_f5eab7`.
Required change: update the PR body with the existing library facilities considered and the reason for each custom component.
Describe the final R-69 capture form. Do not keep a rationale for the rejected held-event form.

### Other source and evidence checks

R-62 authorizes the comparator and real form. R-67 authorizes the two-comparison testkit form.
The testkit still fills `ModelLog` at the program edge before `Input::PtyOutput`.
The comparator reads libghostty fields and applies DECRC only to harness-owned oracle terminals.
The new C keys match the pinned header. The pinned snapshot encoder's 41 source files import no formatter.
The reviewer read the completeness cases and the negative saved-cursor comparisons.

The reviewer checked every exclusion change since the earlier complete exclusion review.
The current PR changes no exclusion. The inherited delta adds the reasoned slow-coverage shell entry and removes four broad driver exclusions.
The inherited constant-line and reaper-name updates retain their stated evidence.
No exclusion covers the new comparator, capture layer, resume form, output history, or inspection reads.
The gate's citation and decision checks pass.

The supplied exact-head gate passes all ten steps and ends with exit 0:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-resume-2a0c4922-pool-20261011-071917-20743.log`.
It reports 121 mutants: 88 caught, including 29 in stage 2; 33 unviable; 0 missed; 0 timed out.
All three removed ST-6b IDs pass on the real slow tier in that gate.
This green gate does not close the source findings.

The retained Mac log at `8169f53f` reports five passes for each of the three IDs:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-r62-real-mac-x5-8169f53f-20261011-062716.log`.
The reviewer also read the five reversal results at `ad87b507`; each reversal caused a failed test.
These logs precede the final capture-layer move and formatter cleanup. They do not prove the R-69 repair.
The retained Mac crate runs are not wholly green: nextest reports 437 passes and two 2-second timeouts;
the one-process run reports 427 passes and a crash-test thread-count failure, 31 versus 28.
Those limits remain explicit. The reviewer ran no test, build, or gate.
