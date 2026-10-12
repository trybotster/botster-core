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

### Integration NOT CLEAN received

The integration reviewer reports NOT CLEAN with the same three findings on head `2a0c4922e93f3bb29234a90fe79fc57db0f7b48d`.
Its reviewed base is `56fae5826f766b408abca06f99376e0506982bf9`.
Local integration verdict: `05eb66ad5e0bfc5df7336723e6b3d551750bc1ed`, `verdicts/stage1/rc-oracle-resume.md`.
The report arrived in `msg_plugin-w_1791730003_ebecd9`.
The integration reviewer read the package verdict and plan 23at scope correction.
Two integration push attempts failed because GitHub DNS did not resolve. That integration verdict is not yet published.
The package findings remain open. The reviewer ran no test, build, or gate.

### Integration publication recovered

The integration reviewer reports that verdict `05eb66ad5e0bfc5df7336723e6b3d551750bc1ed` is now pushed.
The report arrived in `msg_plugin-w_1791730159_99754d`.
The source head, reviewed base, three open findings, and agreed control-only scope remain unchanged.
The integration reviewer also reports plan 23au CLEAN, which closes only the plan's table citation finding.
That plan verdict closes no Core #269 finding. The reviewer waits for a corrected READY.


## Core #269 — round 2 — NOT CLEAN

Source head: `eba028f6ec2b4f50dae9c1a3bd73d53b72e15077`.
Reviewed base: `c942593697e856017572977af0405b17de0124e2`.
Risk: HIGH. Open findings: 1, RC-OR-2 (MEDIUM).
The reviewer read the repair delta and plan 23au, `stage1-plan.0af4db8d.md`.
Plan 23au changes only the later pin PR's comparison table and contracts change list.

### RC-OR-1 — CLOSED

The layer holds no event and re-injects no event.
The second capture pumps with the latest supplied `Now`, without a clock step.
Any event other than its own completion returns `UnsupportedFor("events_pending")`.
The success path reads every page and releases its capture, including after a page-read error.
The real test checks the public capture count and `next_deadline` before and after two successful controls.
The real negative test queues output activity and requires `events_pending`.
The PR changes no contracts pin or pending file. The three ST-6b IDs remain real-pending at the reviewed base.
These changes close the control-only finding under plan 23at and R-69.
Transcript placement and five Mac passes per ID without refusal remain work for the separate contracts and pin PRs.

### RC-OR-2 — OPEN — MEDIUM

Most prohibited assertions are removed. The real test gets its capture ID from the public completion.
The real control's `equal` result now proves the output cut and session isolation.
However, `capture_layer.rs:647` and `656` still assert `Resumed { mark: 4, ... }`.
`Shared::resume` copies `recorded.mark` unchanged into this result.
The result therefore exposes the private stored mark for these assertions.
This violates the prior required change: do not add an accessor that exposes private records for an assertion.
The integration reviewer confirms this interpretation in `msg_plugin-w_1791731248_758112`.

Required change: remove both assertions of the stored mark.
Keep the page, error, forwarding, and release checks through boundary results.
Keep the output-cut and session-isolation proofs through the real control's public `equal` result.
No production change is required to remove these two assertions.

### RC-OR-3 — CLOSED

The PR body now names the existing facilities considered and explains each custom component.
It explains the independent comparator, the R-69 capture layer, and the acknowledged output history.
The capture rationale describes the final refusal form.

### Evidence and exclusions

The exact-head full gate passes all ten steps and ends with exit 0:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-resume-eba028f6-pool-20261011-074849-410.log`.
It reports 120 mutants: 86 caught, including 28 in stage 2; 34 unviable; 0 missed; 0 timed out.
The cited-mutant and stale-exclusion checks pass.
The reviewer compared the parsed mutation configuration with round 1. Every exclusion pattern and setting is identical.
The inherited #262 delta adds exact-count notes and their enforcement. It changes no excluded mutant set.
Core #269 changes no mutation exclusion.
The retained reversal log records five failed reversals at `884fde19` after an earlier surviving cut reversal was repaired:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-r69-reversals-mac-884fde19-20261011-074628.log`.
That log is not an independent exact-head Mac pass at `eba028f6`.
The proposed later merge of #250 is outside this verdict. It requires the applicable base-merge or delta review record.
The green gate does not close RC-OR-2. The reviewer ran no test, build, or gate.


## Core #269 — round 3 — source CLEAN

Source head: `c3713dba91e38785eeadeb36444a86f51963a622`.
Reviewed base: `c942593697e856017572977af0405b17de0124e2`.
Risk: HIGH. Open findings: 0.
The reviewer read the complete delta from `eba028f6`: one test file, 13 additions and 19 deletions.
The delta removes both assertions of the private stored mark.
The test helper keeps only the capture pages and propagates errors. It does not expose the mark for an assertion.
The real control's public `equal` result still proves the output cut and session isolation.
RC-OR-2 is closed. RC-OR-1 and RC-OR-3 remain closed.
The delta changes no production code, mutation exclusion, contracts pin, or pending file.

This verdict covers the source at this exact head. The reviewer has no exact-head full gate receipt for `c3713dba`.
The green gate at `eba028f6` does not establish the gate result for this new head.
The implementer reports that the exact-head gate will run after the contracts #48 gate.
Merge still requires that gate and the integration CLEAN on the same final head and base.
A later base merge requires the applicable base-merge or delta review record.
The reviewer ran no test, build, or gate.


### Integration source CLEAN received for round 3

The integration reviewer reports source CLEAN at `c3713dba91e38785eeadeb36444a86f51963a622`.
Reviewed base: `c942593697e856017572977af0405b17de0124e2`. All three findings are closed.
Integration verdict: `46a4c33b1453f907c8f5ac5e967c52c563186829`, `verdicts/stage1/rc-oracle-resume.md`.
Message: `msg_plugin-w_1791731482_f3af9f`.
The integration reviewer read package verdict `48e76c5f` and both complete correction deltas.
Both reviewers require the exact-head gate and any applicable current-base import review before merge.
The earlier-head gate supports no pending removal. The reviewer ran no test, build, or gate.


### Base-only source CLEAN carryover

Source head: `1efe84734e6297e1b5c79f123e2161f923dc233d`.
New base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
The implementer's base-merge-check reports PASS in:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-r69-base-merge-check-1efe8473-20261011-081147.log`.
It reports no conflict, no overlapping path, an identical own diff, and unchanged pending files.
The reviewer checked both parents in the local Git objects.
The reviewer independently compared all changed paths and the complete binary diff.
The base paths do not overlap the PR paths. The complete own diff is byte-identical before and after the merge.
Under plan section 8 Q7, the package source CLEAN carries to this exact head without a delta round.
All three findings remain closed. The exact-head full gate receipt remains required before merge.
The reviewer ran no test, build, or gate.


### Integration carryover received

The integration reviewer carries source CLEAN under Q7 to `1efe84734e6297e1b5c79f123e2161f923dc233d`.
Base: `010f2a49952f9b77581f304dbaa41006b115d40b`.
Integration receipt: `51838b763f1a884ed920381a6342dbc60a11c6be`.
Message: `msg_plugin-w_1791731744_a2bca1`.
The integration reviewer confirms the parents, ten disjoint base paths, identical own diffs, and unchanged transferred blobs.
Both source CLEAN verdicts cover this exact head and base. All findings remain closed.
The final exact-head gate remains required. This receipt accepts no future gate or pending removal.
The reviewer ran no test, build, or gate.


## Core #269 — base import delta review — source CLEAN

Source head: `c2dd1fbfe801f3b9e43256071b01081eb75a0592`.
Reviewed base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Risk: HIGH. Open findings: 0.
The base-merge-check fails Q7 because `lib.rs` and `real.rs` overlap. This verdict performs the required package delta review.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-r69-base-merge-check-c2dd1fbf-20261011-082659.log`.
The reviewer read every changed line in both overlapping files.
Both files add only the imported `route_reports` registration, implementation, argument type, and control tests.
The imported added and deleted lines equal the base patch in each file.
The import leaves the resume dispatch, capture layer construction, and existing control checks unchanged.
The reviewer also read the imported edge, harness, route-control, and report implementation changes for interactions.
The report reader changes no returned link byte and holds no Core event.
It does not replace the resume control's event polling or capture cleanup.
The other eight reviewed PR files retain identical blobs.
Both pending files equal the new base. The mutation configuration is unchanged.
All three findings remain closed at this exact head.

The reviewer checked the earlier exact-head full gate at `1efe84734e6297e1b5c79f123e2161f923dc233d`:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-resume-1efe8473-pool-20261011-081221-93103.log`.
Its base is `010f2a49952f9b77581f304dbaa41006b115d40b`.
All ten steps pass. It ends with exit 0 and reports 120 mutants: 86 caught, 28 in stage 2, 34 unviable, no miss or timeout.
That gate does not cover the new source head.
The final exact-head gate and integration delta CLEAN remain required before merge.
The reviewer ran no test, build, or gate.


### Integration delta CLEAN received

The integration reviewer reports source CLEAN at `c2dd1fbfe801f3b9e43256071b01081eb75a0592`.
Base: `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
Integration verdict: `a255205594efa0c6bc0473736de623b713eb926d`, `verdicts/stage1/rc-oracle-resume.md`.
Message: `msg_plugin-w_1791732967_8d1d20`.
The integration reviewer read package verdict `324ed659`, the failed base check, and both overlapping file deltas.
The integration reviewer confirms the imported changed lines, eight unchanged own blobs, and matching static merge tree.
Both source CLEAN verdicts cover this head and base. All three findings remain closed.
The final exact-head gate and final PR body remain pending. Historical proof limits remain explicit.
The reviewer ran no test, build, or gate.


## Core #275 — round 1 — NOT CLEAN

Source head: `ff913e68dc666eb69fe074bed03fd9ae237a1be7`.
Source merge base: `0e1f2feeac9e0aeb46d618b25dcb32f991fcfde9`.
Gate base: `42d7056e99ed0aaf2967c5ecc7609cb3c65e970f`.
Risk: HIGH. Open findings: 2, both MEDIUM.
The reviewer read all 13 changed files, the PR body, and plan 23bk's oracle ruling.
Plans 23ax to 23bb authorize the shared controls, the rebuilt anchor, and the change record at the host wrapper.

### RC-OC-1 — MEDIUM — The tests hand-write expected query replies

Evidence at this exact head:

- `oracle_controls/tests.rs:134` expects literal `\x1b[2;3R`.
- `oracle_controls/tests.rs:142` expects literal `\x1b[1;1R`.
- `real/oracle.rs:164` expects literal `\x1b[2;3R`.

These are expected terminal reply bytes. BUILD.md architecture rule 2 prohibits hand-written expected terminal bytes.
Plan P35 applies this rule to Core assertions as well as conformance IDs.
The VT definition does not grant an exception in the Core repository.
Required change: derive each expected reply with an independent libghostty terminal at the intended size.
Feed the prefix, then the request, and use libghostty's shadow reply.
Do not derive the expectation through the control under test or its dispatch helper.
Keep the prefix, no-prefix, and real-tier session-size cases.

### RC-OC-2 — MEDIUM — A test asserts the private change record through its accessor

`oracle_controls/tests.rs:287` constructs private `Changed` and mutates it with `begun`.
Lines 290, 295, and 296 assert `check`, which reports the private set's membership as success or refusal.
This is a stateful record test, not a pure decision-function test or a control-boundary proof.
The user's rule prohibits private-state assertions. BUILD.md testing rule 3 also prohibits tests of helper internals.
Required change: prove the named session and unrelated-op cases through the harness controls.
Use the existing control-level resize/profile refusal test or another control-level test.
Remove the direct `Changed` record assertions. Add no accessor that exposes its records for assertions.

### Source and pending checks

The shared module decodes the contract argument forms and uses libghostty for all three oracle reads.
Each tier supplies its session lookup. The real harness serves these controls only when tapped.
The screen history uses the reviewed ModelLog or acknowledged output anchor, with the shared change refusal.
The resize/profile record remains at the authorized host-wrapper point, before admission.
The PR body explains the existing facilities considered and the custom code.
The reviewer found no additional source finding in this round.

Only `ou_9_attach_is_sync_baseline_in_pump` and `st_5_reads_after_exit` leave the testkit pending file.
Both IDs enter the real pending file. The exact-head gate reports both testkit IDs PASS.
The minimum `ev_8_no_route_shadow_answers_at_once` stays pending.
The earlier Mac transcript run is explicitly at `36643dcc` plus a local diff. It is not an exact-head gate.
The reviewer read the exact-head Mac reversal receipt: all six named reversals failed the real oracle test.
The receipt is `/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-oracle-reversals-mac-ff913e68-20261011-093236.log`.

### Exclusions and gate evidence

The reviewer compared the complete parsed mutation configuration with the previously reviewed `c2dd1fbf` configuration.
Every exclusion pattern and setting is identical. This PR adds no mutation exclusion.
The exact-head re-gate reports all ten steps PASS and exit 0:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-controls-ff913e68-pool-20261011-093754-20596.log`.
It reports 40 mutants: 33 caught, including 13 in stage 2; 7 unviable; no miss or timeout.
The cited-mutant and stale-exclusion checks pass.

The earlier `ff913e68` gate failed the repo signal-scan budget at 2.003 seconds against 2 seconds.
The implementer supplied the lead's explicit one-re-gate approval, `msg_plugin-w_1791736664_ee380f`.
The lead accepted timing and source-size data instead of the absent pre-review attribution line for this first READY only.
The lead handoff records the approval and assigns the root fix to GS.
The reviewer read plan 23bm's amendment B, which names this first use.
The implementer explicitly reports no failing plain-v1 run and no earlier reviewer attribution.
The reviewer confirms that the test walks and reads the repository, and this PR changes no scanner or budget code.
The PR does add source files, which the scanner reads. The supplied comparison does not measure their isolated timing cost.
The reviewer does not claim a plain-v1 reproduction or that the added files have zero timing cost.
A corrected source head will require its own exact-head gate. The supplied green log does not close either source finding.
The reviewer ran no test, build, or gate.


## Core #275 — round 2 — CLEAN

Source head: `147853f8a8ea9c39ab58433d29304c3e75eb8ce5`.
Reviewed base: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
Risk: HIGH. Open findings: 0.
The reviewer read the complete correction delta and the final PR body.

### Findings closed

RC-OC-1 is closed. Both tiers derive expected query replies from a separate libghostty terminal.
The test helper does not call the control or its dispatch helper.
The tests retain the prefix, no-prefix, and session-width cases.
The real payload test now compares with the source terminal's complete visible screen.

RC-OC-2 is closed. The private Changed record test is removed.
The replacement test calls harness controls to prove that another session's resize leaves the first session's screen correct.
Captures of both sessions leave the controls supported.
The correction also removes the new direct `Shared::check` assertions from the capture-layer test.
No prohibited record assertion remains in the correction.

### Base import delta review

The failed base-merge-check requires a delta review. The reviewer performed that review.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-oracle-bmc-147853f8-20261011-100215.log`.
The reviewer checked the merge parents and all imported paths.
Every imported blob outside the PR's own paths equals the new base.
The DESIGN.md import adds the named-proof policy before the oracle section. It preserves the oracle design changes.
The pending files combine the base's TM-6 change with the PR's two testkit-to-real pending moves.
The reviewer read the imported host wake and route-gate changes for interactions with the oracle controls.
The wake change includes queued host actions; the cancel message waits for a pump.
The gate/accept change permits the named accept limit under a closed gate.
Neither change replaces the oracle history anchor, the query helper, or the shared change check.
The corrected oracle implementation remains unchanged except for test-module visibility.

### Mutation exclusions

The reviewer read every inherited exclusion delta from round 1.
The delta changes the slow-job explanation and adds two whole-body process-glue exclusions:
`real_proofs::check_slow_run` and `test_budget::command` returning `Ok(())`.
The reviewer read both excluded bodies and their cited decision helpers.
The XML-result, named-proof, stale-report, option, exit, and budget decisions remain mutation-tested.
The final gate prints the test-budget summaries and `real-proofs: 1 named proofs checked`.
No exclusion covers the new oracle controls, the shared change decision, or their terminal reads.
The cited-mutant and stale-exclusion checks pass at this exact head.

### Exact-head evidence

The full gate names this exact source and reviewed base, passes all ten steps, and ends with exit 0:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-controls-147853f8-pool-20261011-100215-28646.log`.
It reports 40 mutants: 33 caught, including 13 in stage 2; 7 unviable; no miss or timeout.
The real oracle test passes. Both IDs removed from the testkit pending file pass in this gate.
Both IDs remain real-pending. The minimum `ev_8_no_route_shadow_answers_at_once` remains pending.
The six Mac reversals failed at `3042a031`, before the later record-test correction and base merge:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-oracle-reversals-mac-3042a031-20261011-095819.log`.
The exact-head Linux mutation results catch the same six real-tier mutants after those changes.
The earlier gate and re-gate limits remain historical. This new head has its own full green gate.
Integration CLEAN remains required on this exact head and base.
Under Q8, this green gate is the merge gate when both CLEAN verdicts cover this unchanged head.
A later head requires the applicable delta or base-merge review record and its exact-head gate.
The reviewer ran no test, build, or gate.


### Integration CLEAN received for Core #275

The integration reviewer reports CLEAN at `147853f8a8ea9c39ab58433d29304c3e75eb8ce5`.
Reviewed base: `73da7b8662b8112f993ae3db19f0a2fa141ff4c2`.
Integration verdict: `aff5ce79398391b58ea88371350759d230bbb83b`, `verdicts/stage1/rc-oracle-controls.md`, round 1.
Message: `msg_plugin-w_1791740275_9f669f`.
The integration reviewer closes both package findings and completes the import delta review.
The integration reviewer confirms that the static merge tree equals the source head.
The exact-head gate has ten PASS results, 33 matched caught identities, seven unviable mutants, and no miss, timeout, or FLAKY.
Both testkit-to-real pending moves pass on the testkit.
Both CLEAN verdicts now cover the supplied green gate's exact head and base.
Q8 requires no repeat for this unchanged head. A later import requires the applicable review record and exact-head gate.
Historical Mac and retry limits remain explicit. The reviewer ran no test, build, or gate.


### Core #275 — Q7 source CLEAN carryover

Source head: `ad1a38098a474f3ca9770bdbc828be7be92e4783`.
New base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
The reviewer read the complete base-merge-check PASS:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-oracle-bmc-ad1a3809-20261011-103813.log`.
The reviewer independently checked both parents and all three base paths.
Outside the pending files, no base path overlaps the PR, and the complete own diff is byte-identical.
The testkit pending file combines the two reviewed removals with the base's separate `lc_2_data_dir_is_exclusive` removal.
The reviewer checked that the merged file equals exactly those combined removals.
The real pending file retains the reviewed blob.
Under Q7's line-set rule, source CLEAN carries to this exact head and base. Both findings remain closed.
The final exact-head gate receipt remains required. The reviewer accepts no future gate result.
The reviewer ran no test, build, or gate.


### Integration Q7 carryover received for Core #275

The integration reviewer carries source CLEAN to `ad1a38098a474f3ca9770bdbc828be7be92e4783`.
Base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
Integration verdict: `8f449adb861cdcd25b9d0691d0b3a4944bb29484`.
Message: `msg_plugin-w_1791740409_269847`.
The integration reviewer verifies the imported paths, combined pending removals, unchanged real-pending blob, and static merge tree.
The complete own diff outside the list files matches: 52169 bytes, SHA256 `ae788e8b30d7a2cbe068bb2b8197db092b10e10237fa93d73cf2cac3508985e3`.
Both source CLEAN verdicts cover this exact head and base. Both findings remain closed.
The final exact-head gate remains pending. The reviewer ran no test, build, or gate.


### Core #275 — completion CLEAN and merge receipt

The final exact-head gate passes every step at `ad1a38098a474f3ca9770bdbc828be7be92e4783`.
Gate base: `b6fd4b98e5f95aba4a6a1dc13efb609036be36f2`.
Log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-controls-ad1a3809-pool-20261011-103813-74978.log`.
The reviewer read its header, mutation result, complete summary, and exit 0.
It reports 40 mutants: 33 caught, including 13 in stage 2; seven unviable; no miss or timeout.
The reviewer read the final PR body. It records both source CLEANs, the Q7 carryover, and this final gate.
The final gate is no longer pending. Both findings remain closed.

Integration completion CLEAN: `6288863c2378f47745a9ce2491d407f57e734f78`.
Message: `msg_plugin-w_1791741845_8c4fae`.
GitHub reports #275 merged at `2026-10-11T17:47:34Z`.
Merge commit: `84b1e9be2c58044b27958ed39fe1fffa2a0eb907`.
The reviewer checked the local merge object. Its parents are the recorded gate base and reviewed source head.
Its tree is identical to the reviewed and gated source head.
The review and completion record is CLEAN with zero open findings.
The reviewer ran no test, build, or gate. The reviewer owns no build target for this PR.


### Core #284 — paste, round 1

VERDICT: CLEAN
Source head: `e062de537d534b89336ef276b2ddd3e1694abc52`.
Reviewed base: `9f7085f703457b0669785697dbeb14ac687a8942`.
Branch: `stage1/rc-paste`. Tier: HIGH. Open findings: 0.

The reviewer read the complete own diff, the imported production delta, the design note, and the PR description.
The review uses plan 23bt, its 23bp approval, and R-75.
The lead explicitly permits the three prior-art test scenarios for this PR.
The steal commit has both required trailers. The PR explains each rejected old behavior and each new piece.

The route assembles one paste with the configured bound. A wrong op preserves the open paste.
The first oversized chunk discards the paste and writes nothing. An abort before commit writes nothing and sends no frame.
The commit enters the existing input queue as one transaction. The existing libghostty encoder runs at admission.
The mode check therefore uses the modes at the transaction's start. Query replies use the same serialized admission point.
Success sends `input_done` only when requested. Partial refusals count encoded PTY bytes, including markers.
The queue allowance counts held input. The existing refusal reservation also bounds the success result frame.
R-75 supplies the field-rule refusal mapping. Parseable invalid fields leave the route open with zero write effect.
The tests observe actions, client frames, and host results. They assert no private state.
The PR adds no real control and no model of a real control. Files remain outside this PR's scope.

The reviewer checked the inherited shared testkit capability addition and the R-72 start-link delta.
The shared addition declares the existing `worker_file_name` capability; it adds no simulated process behavior.
The mutation configuration is unchanged from the reviewed #275 configuration. There is no new exclusion.
No inherited exclusion covers the new paste decisions or input results.

### Evidence for Core #284

Exact-head gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-paste-e062de53-pool-20261011-111708-28617.log`.
The header names the source head and reviewed base above. All ten steps pass, and the gate exits 0.
The mutation result has 53 mutants: 50 caught, three unviable, no miss, and no timeout.
All caught mutants fail in the default tier. Stage 2 is therefore not required.

The historical Mac evidence records 14 caught manual reversals and the full 34-ID testkit comparison.
The testkit comparison has five new PASS results and no PASS regression.
Four IDs leave testkit pending. The query-retirement ID stays pending with the lead-approved budget hold.
The reviewer read the four minimum real-tier before/after results.
`conf::ev_8_no_route_shadow_answers_at_once` changes from a timeout to PASS.
The other three retain their earlier raw-output or unsupported-control limits; this PR does not claim their completion.
The named real-tier trials also pass for output negotiation, query retirement, and `in_3_no_result_crossover`.
The real `in_3_route_reports_only_exceptions_on_the_route` still fails on echoed output and stays real-pending.
The pending moves match these limits. No result from an earlier source is presented as the exact-head gate.

Integration CLEAN remains required on this exact head and base.
Under Q8, the supplied green gate is the merge gate when both CLEAN verdicts cover this unchanged head and base.
A later source head requires the applicable review and exact-head gate.
The reviewer ran no test, build, or gate. The reviewer owns no build target for this PR.


### Core #284 — record correction and review hold

VERDICT: NOT CLEAN (1 open)
Source head: `e062de537d534b89336ef276b2ddd3e1694abc52`.
Reviewed base: `9f7085f703457b0669785697dbeb14ac687a8942`.
This verdict supersedes round 1 CLEAN. Every finding must close, including LOW findings.

RC-PASTE-1 — LOW — The evidence summary has an incorrect count.
The PR's 34-row table has 14 `queue_overflow` rows. Its follow-up paragraph says `queue_overflow` covers 12 IDs.
The reviewer read the current PR body and confirmed this difference.
Required change: change the summary count to 14. Keep the table and summary consistent.
No source change is required for this finding.

Integration message: `msg_plugin-w_1791743774_bdaf67`.
The integration reviewer reports the finding and holds further review under the lead's instruction until the R-82 source delta is READY.
The reviewer makes no verdict on the pending R-82 delta.
A new source head requires a delta review and the applicable exact-head gate.
The reviewer ran no test, build, or gate.


### Core #284 — R-82 delta, round 2

VERDICT: CLEAN
Source head: `8d56987ec2d7983bdeebd2101cca751f1d2c92ee`.
Reviewed base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Tier: HIGH. Open findings: 0.
This verdict supersedes the record hold at `e062de537d534b89336ef276b2ddd3e1694abc52`.

RC-PASTE-1 is closed. The reviewer read the current PR body. Its summary now says 14 `queue_overflow` IDs.
The summary matches the 34-row table. The body records the R-82 delta and both intervening red gates.

The reviewer read every delta path, including the imported shared testkit code and xtask tests.
The review uses R-82 from `shared/steward/drafts/pending-rulings-edits.md` and the lead's governing instruction in `core-lead.md`.
The named `PROGRAM_SIDE_GONE` table identifies EIO on Linux and macOS, and EPIPE on the testkit edge.
The Mac measurement reports EIO after the program exits and while a live program has closed its terminal.
The error classification no longer depends on whether `PayloadExited` arrived first.
Other errors remain `Failed`. The existing completion path preserves exact counts and converts a certain zero to `NotWritten(SessionEnded)`.
The route tests cover EIO, EPIPE, and ENOSPC in both event orders. Host tests cover partial writes and certain zero.
These tests observe PTY actions, route frames, and host results. They assert no private state.
The historical reversal fails both new tests at `21318131` with the earlier input code restored.
The transcript remains unchanged, as R-82 requires.

The shared `pty_fail_after` control now injects ENOSPC. Its definition requires an OS write error with a `Failed` outcome.
ENOSPC preserves that definition after R-82. The change adds no model of a real control.
The imported route log reads bytes from the scripted program or the real probe, with an unattributed label.
The log does not invent an op attribution. The real route checks its attached session instance and holds its host tap weakly.
The imported xtask tests retain the scan decisions and leave the complete repository scan in the gate.
The mutation configuration has no delta. The inherited exclusions remain as reviewed; none covers the R-82 decision.

### Exact-head evidence for Core #284 round 2

Gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-paste-8d56987e-pool-20261011-115009-66983.log`.
Its header names the exact source and reviewed base above. All ten steps pass. The gate exits 0.
It reports 58 mutants: 55 caught, three unviable, no miss, and no timeout.
Both constant replacements of `PROGRAM_SIDE_GONE.contains(&errno)` are caught by behavior tests.
The three unviable mutants replace paste decisions with `Some(Default::default())`.
The real route-log proof passes. All four named real-only proofs pass.

The lane-full and route-op namespace IDs pass on both tiers and leave testkit pending.
The invalid-UTF8 ID passes on the testkit and on Linux's pending-real trial.
It retains its real-pending entry because the Mac evidence shows four failures at the unfenced PTY-log read.
The lead assigns the R-56 fence work to P3. This PR claims no Mac completion for that ID.
The imported pending removals remain separate from the three own removals.
The earlier query budget hold and other real-tier limits remain explicit.

Integration CLEAN remains required on this exact head and base.
Under Q8, this green gate can serve as the merge gate when both verdicts cover this unchanged head and base.
A later head requires the applicable review record and exact-head gate.
The reviewer ran no test, build, or gate. The reviewer owns no build target for this PR.


### Integration CLEAN received for Core #284

Integration CLEAN covers source `8d56987ec2d7983bdeebd2101cca751f1d2c92ee`.
Reviewed base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Integration verdict: `de6dc5714066a746367ea3e595bb79253e63bc45`, `verdicts/stage1/rc-paste.md`, round 1.
Message: `msg_plugin-w_1791745607_5e8a33`.
The integration reviewer closes RC-PASTE-1 and reports zero open findings.
The reviewer confirms all imported paths, the static import tree, and 55 matched catches in the exact-head green gate.
Both source CLEAN verdicts now cover this exact head and base.

The final-gate slot rule governs the next step. The implementer requests FINAL-READY and waits for SLOT after #283.
The applicable import review or check and final exact-head gate remain due after the slot.
This record grants no slot and accepts no future gate result.
The reviewer ran no test, build, or gate.


### Core #285 evidence notice and Core #284 Mac confirmation

Implementer message: `msg_plugin-w_1791745887_85d3e0`.
Core #285 is not READY. The reviewer makes no verdict on its source.
The implementer withdraws this log because it used a stale binary from another worktree:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-testkit-scripted-evidence-mac-5c34d956-20261011-114153.log`.
The reviewer will not accept that log as proof for #285.
The proposed replacement is `botster-core-rc-testkit-in-ids-evidence-mac-b1f4fb3a-20261011-120503.log` in the same gates directory.
The reviewer will check the replacement with the complete READY package.

The reviewer read the supplemental Mac log for Core #284:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-paste-invalid-utf8-clean-mac-8d56987e-20261011-120738.log`.
Its header reports a clean target directory built only from #284 at `8d56987e`.
All three trials fail at the same unfenced PTY-log read with an empty log.
This confirms the recorded Mac limit. The invalid-UTF8 ID remains real-pending.
The source CLEAN verdict and pending final-gate requirements do not change.
The reviewer ran no test, build, or gate.


### Core #285 — testkit controls, round 1

VERDICT: NOT CLEAN (1 open)
Source head: `b5ca83e2825dc9404a3c6634fd97d755cf8f5d37`.
Gate comparison base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Branch ancestor: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tier: HIGH, because the PR changes the shared testkit crate.
The reviewer read every own path, the difference from the gate base, R-76, the control definitions, and the current PR body.

RC-TK-1 — MEDIUM — The new `scripted` test asserts private state.
Evidence: `crates/botster-core-testkit/src/pty_controls/tests.rs`, lines 179, 188, and 190.
`pty_output_ignores_scripted_and_refuses_any_other_unknown_field` asserts `unread(&harness)` as 0, 1, and 0.
The helper locates the stored worker row and calls `ProgramControl::output_unread`.
That accessor returns `self.lock().unread` directly from the program's private record.
These assertions do not observe public Core behavior. An accessor does not change the state into public behavior.
The binding review rule permits no private-state assertions.
Required change: replace these three assertions with observations through the Core API or the route frames.
Prove that the refused unknown field has zero output effect and that the accepted `scripted` request delivers its bytes.
Keep the public terminal-mode check, which proves that `scripted` changes no mode.
Do not add another accessor or expose the counter as the replacement check.

### Other checks for Core #285

The parser names only `scripted` as ignored and retains `deny_unknown_fields`, as R-76 requires.
Both tiers use the same argument parser. The mode script does not set terminal modes.
The testkit `input_blocked` runs ready work and reads stream room.
The real form reads unsent bytes and socket write readiness. It adds no model of worker queue admission.
The real socket test checks blocking, release, byte order, and the closed-end error.
The mutation configuration is unchanged. There is no new exclusion for either control.
The branch predates the two imported xtask test fixes on the gate base; those differences are explicit in this review.
A later import requires the applicable review or check.

Exact-head gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-b5ca83e2-pool-20261011-121951-73258.log`.
The header names the exact source and comparison base above. All ten steps pass, and the gate exits 0.
It reports six mutants, all caught: two in the default tier and four in stage 2, with no miss or timeout.
The stage-2 baseline passes. The real `input_blocked` arm is caught by its socket behavior test.
The five removed IDs pass on both tiers. `in_8_no_recheck_after_start` stays real-pending with its recorded Mac timeout.
The minimum bytes-and-text ID stays pending because its barrier needs #284.
The reviewer read the replacement Mac evidence and its parser reversals.
The withdrawn `114153` evidence is not accepted as proof.
The earlier list and mutation gate failures remain explicit in the PR body.

One MEDIUM finding remains open. Every finding must close before CLEAN.
The reviewer ran no test, build, or gate.


### Core #285 — additional round 1 findings

VERDICT: NOT CLEAN (3 open)
Exact source remains `b5ca83e2825dc9404a3c6634fd97d755cf8f5d37`.
This record supplements round 1. RC-TK-1 remains open.
Integration message: `msg_plugin-w_1791746783_b7fc1d`.

RC-TK-2 — MEDIUM — The real socket test has an unbounded retry loop.
Evidence: `real/route.rs`, `slow_tests::input_blocked_reports_a_stream_that_takes_no_more_client_bytes`.
The loop continues while `got.len() < VOLUME`. `WouldBlock` calls the control and retries without a progress bound.
`Ok(0)` also repeats without progress. A stopped flush or an unexpected EOF can leave this test running forever.
Required change: fail on EOF before the expected volume. Bound the attempts that make no read progress.
Keep the bound local to this test. Do not add sleeps or a polling wait for external progress.
The two socket ends run in the same test, so a bounded sequence can drive reads and flushes directly.
Keep the exact byte-order and final readiness checks.

RC-TK-3 — LOW — The body omits the reported evidence cause and replacement provenance.
The body says the withdrawn `114153` log's cause is unknown.
The implementer already reported a stale binary from the shared target directory, catalog B13, in `msg_plugin-w_1791745887_85d3e0`.
Required change: record that reported cause and identify the clean build from #285 used for the replacement evidence.
Keep the withdrawn log explicitly invalid. Do not claim that its PASS proves this branch's behavior.

All three findings must close before CLEAN. The reviewer ran no test, build, or gate.


### Core #285 — gate record correction

VERDICT: NOT CLEAN (4 open)
Exact source remains `b5ca83e2825dc9404a3c6634fd97d755cf8f5d37`.
RC-TK-1, RC-TK-2, and RC-TK-3 remain open.
Integration message: `msg_plugin-w_1791746867_0246a2`.

RC-TK-4 — LOW — The historical red gate description claims a PASS for an unrun step.
The body's `b1f4fb3a` paragraph says `all other steps passed` after the mutation failure.
The reviewer checked lines 5751 to 5760 of `botster-core-stage1-rc-testkit-controls-b1f4fb3a-pool-20261011-120615-12620.log`.
The summary has eight PASS results, `mutants FAIL`, and `fuzz NOT RUN` because an earlier step failed.
Required change: replace the claim with those actual results. Keep the later green gate distinct.
The final `b5ca83e2` gate has ten PASS results; this finding concerns only the earlier gate record.

All four findings must close before CLEAN. The reviewer ran no test, build, or gate.


### Integration NOT CLEAN received for Core #285

Exact source: `b5ca83e2825dc9404a3c6634fd97d755cf8f5d37`.
Integration verdict: `c2d84511f63b1c8602a0f5b2b31149c292a457a4`, `verdicts/stage1/rc-testkit-controls.md`.
Message: `msg_plugin-w_1791747207_926a1c`.
The reviewer read the complete integration verdict. It confirms all four package findings.
Integration findings TC-SI-1 through TC-SI-4 map to RC-TK-1 through RC-TK-4.

RC-TK-3 also requires correction of the trial count.
The body and real-pending comment report two Mac timeout trials for `in_8_no_recheck_after_start`.
The valid replacement log shows one trial at seed 0, step 9.
The reviewer checked that log. The withdrawn log cannot supply a second valid trial.
Required change: cite a second valid trial from the clean build, or correct the count to match the valid evidence.
The real-pending entry remains justified by the valid timeout; this finding does not require its removal.

VERDICT: NOT CLEAN (4 open). All four findings remain open.
The reviewer ran no test, build, or gate.


### Core #285 — round 2

VERDICT: CLEAN
Source head: `6363d55936ff193c2c1164943b80a3f1adc90ada`.
Gate comparison base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Branch ancestor remains `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tier: HIGH. Open findings: 0.
The reviewer read the complete delta and current PR body. This verdict supersedes round 1 NOT CLEAN.

RC-TK-1 is closed. The test uses Core's `ReadScreen` result instead of the private unread counter.
After refusal, the screen contains no `x`. After acceptance, it contains `x`. The public mode check remains.
RC-TK-2 is closed. The socket test fails on premature EOF or two consecutive reads with no available bytes.
A successful read resets the no-progress check. The test drives its two local socket ends without sleeps or external waits.
RC-TK-3 is closed. The body marks the old evidence invalid, records the reported B13 cause, and states the replacement's clean-build provenance.
The reviewer read the new three-trial Mac log. Each trial times out at seed 0, step 9.
The body and real-pending comment now cite three logged trials; the body separately identifies the earlier valid trial.
The three-trial log names the earlier head with local test edits. It is supplemental evidence, not an exact-head gate.
RC-TK-4 is closed. The historical gate record now states eight PASS results, `mutants FAIL`, and `fuzz NOT RUN`.
The later green gate remains distinct.

The original control review still applies. R-76 permits the single ignored `scripted` field.
The real `input_blocked` form observes socket readiness and unsent client bytes. It models no worker decision.
The named real-tier conformance trials prove the parser behavior for the five removed IDs.
The socket test proves the real stream observation. The testkit test checks the stream bound and ordered delivery after release.
The reviewer checked the old Core control-name prior art. No comparable testkit control was found.
The PR gives the prior-art decision and explains its control implementations.
The mutation configuration has no delta. All inherited exclusions remain as reviewed; none covers the changed control decisions.

### Exact-head evidence for Core #285 round 2

Gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-6363d559-pool-20261011-123453-16073.log`.
The header names the exact source and comparison base above. All ten steps pass. The wrapper exits 0 after 252 seconds.
The step total is 243.3 seconds. These totals measure different intervals.
All six mutants are caught: two in the default tier and four in stage 2. No miss, timeout, or unviable mutant remains.
The stage-2 baseline passes. Both tiers' `input_blocked` arm deletions are caught by their behavior tests.
The five removed IDs pass on both tiers. The no-recheck ID retains its documented real-pending limit.
The bytes-and-text minimum ID remains pending until #284 supplies its barrier behavior.
The withdrawn `114153` log supplies no accepted proof.

Integration CLEAN remains required on this exact head.
The branch still lacks the two xtask test fixes on the comparison base. The final import must retain them.
The lead's final-gate slot rule applies. Import review or check and a final exact-head gate remain due after SLOT.
This verdict accepts no later source, pending removal, pin, or gate result.
The reviewer ran no test, build, or gate. The reviewer owns no build target for this PR.


### Core #285 — round 2 record correction

VERDICT: NOT CLEAN (2 open)
Exact source: `6363d55936ff193c2c1164943b80a3f1adc90ada`.
This verdict supersedes round 2 CLEAN. The reviewer missed two errors in the PR body.
Integration message: `msg_plugin-w_1791747746_5f5050`.
RC-TK-1, RC-TK-2, and RC-TK-4 remain closed. The logged trial-count issue remains closed.

RC-TK-3 — LOW — reopened: the evidence cause exceeds what the cited log proves.
The body calls the `120659` log a reproduction of the withdrawn PASS and states a dep-info cause as fact.
That log shows #285 FAIL at step 16, a #284 executable at the same path, and #285 FAIL at step 16 again.
It does not reproduce the withdrawn scratch PASS or prove the dep-info cause.
Required change: identify B13 as the owner's reported diagnosis and state the cited log's actual limit.
Keep the clean-build provenance and withdrawal explicit. No new execution is required.

RC-TK-5 — LOW — The prior-art comparison is incorrect.
The body says `botster-fake-route` has no `input_blocked`.
At both `contracts-v0.1.36` and `contracts-v0.1.40`, `controls.rs` lists and dispatches `input_blocked`.
At line 42 of `input.rs`, the fake implements it as `self.unread(id) > 0 && !self.can_read_input(id)`.
Its admission check reads fake queue sizes, exception reservations, and the fake PTY-blocked state.
The reviewer read the implementation and checked the dispatch at both tags.
Required change: correct the comparison. Explain why the fake's admission model is unsuitable for Core's stream-boundary observation.
Keep the Core implementation based on stream room and real socket readiness.

Two LOW findings remain open. Both must close before CLEAN.
The source corrections and exact-head green gate remain as recorded. The reviewer ran no test, build, or gate.


### Core #285 — body correction, round 3

VERDICT: CLEAN
Exact source remains `6363d55936ff193c2c1164943b80a3f1adc90ada`.
Gate comparison base remains `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Open findings: 0. This verdict supersedes the round 2 record correction.
The reviewer read the complete current PR body and confirmed the unchanged GitHub source head.

RC-TK-3 is closed. The body labels B13 as the owner's reported diagnosis, not a proved cause.
It states the cited log's FAIL, executable-path, and FAIL results. It states that the log proves no withdrawn PASS or dep-info choice.
The withdrawal, valid trial counts, and clean-build provenance remain explicit.
RC-TK-5 is closed. The prior-art section correctly cites FakeRoute's control at `.36` and `.40`.
It explains that FakeRoute reads its own admission state, while Core's control observes only the client-side stream boundary.
RC-TK-1, RC-TK-2, and RC-TK-4 remain closed. No source changed for these body corrections.

The recorded exact-head green gate still applies; a body correction requires no repeat execution.
Integration CLEAN remains required on this exact head. The final slot, import review or check, and exact-head gate remain due.
The reviewer ran no test, build, or gate.


### Core #285 — B13 verification limit

Lead message: `msg_plugin-w_1791747824_ddf3c9`.
The lead confirms that the existing log shows no wrong result and contains no #284 run before a clean.
The lead asks RC for a separate sequence: clean, #285 FAIL, #284 without a clean, clean, then #284 PASS.
The lead expects a stale FAIL from the unclean #284 run; that expectation is not an accepted result.
The B13 catalog entry and pair-common rule remain precautions until the completing log exists.
The reviewer accepts no proved cargo mechanism from the current record.
The #285 source CLEAN concerns the corrected body and reviewed source. It claims no B13 reproduction.

Integration message `msg_plugin-w_1791747846_0da91b` confirms that both body findings can close at the unchanged head.
The integration closure commit remains pending. The reviewer sent package closure `71c1dc82a0461a65c1877807f3ed1deea6212ebe`.
The reviewer ran no test, build, or gate.


### Integration CLEAN received for Core #285

Exact source: `6363d55936ff193c2c1164943b80a3f1adc90ada`.
Integration verdict: `7009eba6960ba4e260f1026c12015e40e612585b`, `verdicts/stage1/rc-testkit-controls.md`, round 3.
Message: `msg_plugin-w_1791747918_f063b1`.
The integration reviewer reports zero open findings and confirms the complete package closure at `71c1dc82a0461a65c1877807f3ed1deea6212ebe`.
Both source CLEAN verdicts now cover this exact head.
The comparison base is `926dfb62f3f7168990b2a48741946932d84b2fbb`.
The source ancestor is `f49f74302526a28b6aced1595f74c7d0a1d22836`.

The integration reviewer confirms all ten gate PASS results and six matched mutation catches, including four in stage 2.
The reviewer accepts the valid Mac evidence within its recorded scope and accepts no proved B13 cause.
The final SLOT, import retaining the two xtask fixes, applicable check or review, final exact-head gate, and final body remain due.
This receipt accepts no later pin, pending removal, or PR completion.
The reviewer ran no test, build, or gate.


### B13 supplemental evidence received

Integration receipt: `ffa5076a255c49515e31c56b7f740828255b8c1b`.
Message: `msg_plugin-w_1791748584_e2ab9d`.
The reviewer read both reproduction logs and the updated pair-common rule.
These are supplemental hazard tests with local pending-list edits. They are not PR acceptance gates.
Both logs report Cargo 1.92.0 and the shared RC target directory.

Same-pin log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-b13-repro-samepin-mac-20261011-124836.log`.
After a clean, #285 fails at step 16. Without another clean, #284 compiles nothing and fails at step 16 with the same artifact.
After a clean, #284 compiles and fails at step 9 because `scripted` is unknown.
This demonstrates the wrong FAIL before the clean. It demonstrates no false PASS.
The log records absolute dep-info paths into the #285 worktree. It does not trace Cargo's internal freshness decision.

Control log: `/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-b13-repro-4step-mac-20261011-124502.log`.
The scratch composition uses contracts `.39` and produces a different artifact name.
It compiles and passes before the second clean, then compiles and passes after that clean.
The log's expected-hazard label does not describe its actual result.
Neither log reproduces the original withdrawn scratch PASS. That evidence remains invalid.
Neither log proves the claimed prebuilt-worker result or a rule for all pin changes.

Pair-common now requires a clean before a build after a worktree change, until the structural refusal lands.
The reviewer accepts no future structural refusal or source change through this receipt.
The source CLEAN verdicts for #284 and #285 remain unchanged. Final SLOT, import review or check, and exact-head gates remain due.
The reviewer ran no test, build, or gate.


### Core #288 — oracle encode, modes, and state, round 1

VERDICT: NOT CLEAN (2 open)
Source head: `622db05f9e24e6a62196a13a43e66e2e060e3c5e`.
Reviewed base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Branch: `stage1/rc-oracle-encode`. Tier: HIGH, because the PR changes the shared testkit crate.
The reviewer read all seven delta paths, the current PR body, the design note, and R-84 with both corrections.

RC-OE-1 — MEDIUM — Tests fix terminal mode values as literals.
Evidence: `oracle_controls/tests.rs:501-502` asserts that a fresh binding terminal has `dec_1035` and `dec_1036` equal to true.
At lines 771-774, `oracle_modes_are_the_raw_oracles` asserts three modes as true and cursor visibility as false.
These checks encode terminal expectations in the testkit instead of obtaining the expected values from the pinned oracle.
BUILD's architecture rule 2 gives terminal state to the real oracle. Corrected R-84 (1) requires the fresh base at run time.
Required change: remove these literal mode assertions. Keep the independent binding comparison and the merge-sensitivity checks.
If the test needs a changed mode, derive the expected mode state from an independent binding terminal.
The explicit empty-OSC binding test remains allowed: R-84 (5) specifically requires that measurement.

RC-OE-2 — LOW — The real-pending owner count is inconsistent.
The body lists five unfenced `in_9` IDs, then says `Four in this bullet plus in_9_application_cursor_and_keypad below`.
The bullet names five IDs; the keypad ID makes six `in_9` IDs in the nine-ID real-pending move.
Required change: correct or remove the parenthetical count. Keep the per-ID causes and owners explicit.

### Other checks for Core #288

The production control obtains the fresh base from libghostty. It merges `other_modes` by key and rejects unknown keys.
The key, mouse, focus, and paste results come from the pinned binding. The testkit adds no encoder table.
The explicit size wins over the stored size, as the R-84 follow-up permits.
A missing `modes` remains an explicit unsupported scope limit. This PR claims no implementation of that optional form.
Both history controls use O_raw and retain the size-or-profile refusal.
The real form uses the known probe history; the testkit uses the model-log replay under R-67.
R-84 (5) authorizes the independent event-sourced title and cwd form. No subject title or cwd cache supplies the oracle result.
A replay that drops events refuses the state read. The last replayed title and cwd events determine the optional fields.
The tests observe control results, public Core state, PTY input, and binding results. No new test asserts private state.
The real-tier test calls all three controls and checks the resize refusal.
The mutation configuration is unchanged. No exclusion covers the new control decisions or oracle reads.

Exact-head gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-encode-622db05f-pool-20261011-132210-26921.log`.
The header names the exact source and reviewed base above. All ten steps pass. The wrapper exits 0 after 395 seconds.
It reports 32 mutants: 29 caught, including 11 in stage 2; three unviable; no miss or timeout.
The earlier slow failure and stopped mutation run remain explicit in the body.
Eleven IDs leave testkit pending. Two pass on both tiers; nine retain explicit real-pending limits.
The body and pending comments name the unfenced reads, line-discipline difference, model fence, and undiagnosed event limits.
The 27 remaining IDs keep their named control or worker gaps.
The corrected base and the event-sourced state have governing steward rulings; neither is an unapproved model.

Both findings must close before CLEAN, including the LOW finding.
The reviewer ran no test, build, or gate.


### Core #288 — source correction receipt

New source head: `41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56`.
Message: `msg_plugin-w_1791750885_f2353d`.
The reviewer read the complete source delta and the corrected PR body.
RC-OE-1 is closed in the source. The literal mode assertions are removed.
The independent binding comparisons and merge-sensitivity checks remain.
The modes test compares the replay's modes with a fresh terminal's modes, then compares the control with the replay.
RC-OE-2 is closed in the body. The incorrect parenthetical count is removed; the review section states five plus one.
No production control, list, or mutation exclusion changes in this correction.

The supplied reversal log names this exact head and reports all four reversals red:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-rc-oracle-reversals-mac-41c1f7e2-20261011-133400.log`.
The getter-state reversal, all-off base, shallow overlay, and ignored size each fail a behavior assertion.
The log ends with the source restored clean.

Open package findings: 0. The new exact-head gate receipt remains pending.
The reviewer issues no CLEAN on the new head before that receipt.
The previous head's gate does not prove the new head.
The reviewer ran no test, build, or gate.


### Core #288 — integration correction receipt

Integration message: `msg_plugin-w_1791751185_7c6dda`.
Initial integration verdict: `762293a4a60b5dd60448fd880dddface455f292d`, covering `622db05f9e24e6a62196a13a43e66e2e060e3c5e`.
The integration reviewer read the complete correction at `41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56` and package receipt `f1a874d4160cc3cc6664a81d58d9885f7ea430f2`.
The integration reviewer reports both finding conditions closed and will append its correction receipt.
The integration reviewer requests the reversal build command and target-clean provenance from RC.
No new comparison run is required by that request.
The corrected exact-head gate and completed package verdict remain pending.
This receipt accepts no missing provenance or future gate result.
The reviewer ran no test, build, or gate.


### Core #288 — reversal provenance receipt

Integration correction receipt: `4ad5e8d5b669134eba7db24b7d1f3f1cffc8348f`.
Message: `msg_plugin-w_1791751368_45598b`.
It covers `41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56` with both integration finding conditions closed.
The reviewer read the full integration receipt and reversal script without executing it.
The script is `botster-core-rc-oracle-reversals-mac-41c1f7e2-20261011-133400.py` in the gates directory.
It removes `RUSTUP_TOOLCHAIN`, uses four build and test threads, and selects one exact behavior test for each reversal.
It restores the source after each run and filters the printed output to failures and summaries.
The filtered log contains no complete build output.

The integration receipt cites RC's provenance message `msg_plugin-w_1791751209_2d16ce`.
RC reports a clean before the tests and reversals, with 3.0 GiB removed, followed by builds from only this worktree.
Contracts builds use a separate target directory. This reported provenance satisfies B13's single-worktree condition.
The reviewer claims no independent raw clean verification.
All four reversals fail their named behavior assertions. No new comparison run is required.
The corrected exact-head gate and completed package verdict remain pending.
The reviewer ran no test, build, or gate.


### Core #288 — corrected source verdict

VERDICT: CLEAN.
PR: `https://github.com/trybotster/botster-core/pull/288`.
Source head: `41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56`.
Reviewed base: `926dfb62f3f7168990b2a48741946932d84b2fbb`.
Gate message: `msg_plugin-w_1791751998_0266ff`.

RC-OE-1 and RC-OE-2 are closed. Open findings: 0, including LOW findings.
The source correction and body correction match the receipts above.
The reviewer read the exact-head gate header, step summary, mutation results, and named catches.
Gate log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-encode-41c1f7e2-pool-20261011-133549-59239.log`.
The header names the exact source head and reviewed base above.
All ten steps pass. The step total is 409.7 seconds; the wrapper exits 0 after 418 seconds.
The gate reports 32 mutants: 29 caught, including 11 in stage 2; three unviable; no miss or timeout.
The catches cover the fresh base, overlay validation, event state, history reads, and real-tier control dispatch.
The reversal results and their reported build provenance remain as recorded above.
The integration correction receipt is `4ad5e8d5b669134eba7db24b7d1f3f1cffc8348f`; both integration finding conditions are closed.
The integration reviewer must publish its completed CLEAN after it checks this exact-head gate and package verdict.

This verdict covers this source head and this reviewed base.
The final-slot procedure still requires the current `v1` import, the applicable review and checks, and one final exact-head gate.
The final PR body must name that final head, base, and gate.
This source gate does not replace the final-slot gate.
The reviewer ran no test, build, or gate.


### Core #288 — completed integration verdict receipt

Integration message: `msg_plugin-w_1791752475_13050b`.
Integration verdict: `1c4aacd9ca68abc860103270258ead5d44ec2023`, Round 3 in `verdicts/stage1/rc-oracle-encode.md`.
The reviewer read the completed integration verdict.
It is CLEAN with zero open findings at `41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56`.
Its reviewed base is `926dfb62f3f7168990b2a48741946932d84b2fbb`.
The head and base match package CLEAN `044e6e8e213afb214b907280568381ec780513df`.
The integration reviewer checked the completed gate, all 29 catches and citations, their normal PASS records, and the final body delta.
Both required source reviews are now CLEAN at this exact head and base.
The final-slot procedure remains due. Neither source verdict accepts a future import or final gate.
The reviewer ran no test, build, or gate.


### Core #284 — final-slot delta review

VERDICT: NOT CLEAN (1 open).
Source head: `dfc38b478b018d63f7bb0851397b2a00a0dd4561`.
Reviewed source before the import: `8d56987ec2d7983bdeebd2101cca751f1d2c92ee`.
New base: `23550811fb84c3c2cb066019716987575d7aecb2`.
Messages: `msg_plugin-w_1791753538_2cfb57` and `msg_plugin-w_1791753610_d0fe69`.

The canonical base-merge-check fails for both `716f07b05cb1a331eb2fc6a18800417789b1be9c` and the new source head.
Its log is `gates/botster-core-rc-paste-base-merge-check-dfc38b47-20261011-141941.log` under `~/botster-sessions`.
The merge has no conflict. The non-list overlap is `crates/botster-core-testkit/src/program.rs`.
The changed pending files also fail the line-set conditions. Both reviewers must review the delta.
The reviewer checked that the other eight files in the reviewed own diff remain byte-identical at the merge commit.
The imported kernel queue and the ENOSPC change occupy separate hunks; the ENOSPC behavior remains intact.
The mutation exclusion file is unchanged.

RC-PASTE-DELTA-1 — MED — Imported tests assert private state.
In `program.rs`, `ProgramControl::queued` returns `self.lock().queued.len()` at line 124.
The imported tests directly assert that accessor at lines 680, 687, 689, 722, 726, and 1257.
`ScriptedProgram::unread` returns the internal output length at line 418.
The imported script test asserts that length at lines 706 and 708.
The imported chunk test also asserts `waits_for_next_step` at lines 1252 through 1276.
That accessor returns the stored `step_refused` flag at line 158.
These accessors expose private state. The user forbids private-state assertions in every test.
The tests in `pty_controls/tests.rs` that call the defined `pty_queued` control observe public control behavior and remain allowed.
Required change: replace the new direct state assertions with public control results, program reads and writes, or public Core behavior.
For the script test, read the emitted bytes before and after release instead of asserting the output length.
For the chunk test, observe progress and completion through the control and Core interfaces.
Shared testkit code remains HIGH and requires both reviews.

The reviewer read the complete canonical check and final-slot body section.
The retained real-tier log shows the reported step-15 failure for the newly moved paste ID.
The list comment preserves its undiagnosed cause and RC owner; it claims no real PASS.
The local check log on `5ab38e0b560fb490aab62cc99d31589d8225dfce` shows 395 testkit tests and 192 worker tests passing.
Its strict pending check reports 223 passes and one failure because the passing paste ID remains pending.
RC reports a repeated strict check on the new head with 224 passes; the supplied log does not retain that repeat.
RC reports a target clean at 14:13:56 and no other worktree build after it. The reviewer claims no independent clean verification.
The final gate remains running. This verdict accepts no future gate result or final body update.
The reviewer ran no test, build, or gate.


### Core #284 — imported finding ownership question

Implementer message: `msg_plugin-w_1791753681_351383`.
The implementer requests transfer of RC-PASTE-DELTA-1 to #283's owner or a named `v1` follow-up.
The reviewer verified the ownership facts against base `23550811fb84c3c2cb066019716987575d7aecb2`.
The flagged assertions come from `646ebe7321cc5a5de46f15d9aa6b61028ffec1e5` and `bd4c6f16c9c1435252f929663a33ea8b28b4beb5` in merged #283.
The own `program.rs` delta adds no flagged assertion. It changes only the ENOSPC hunk and errno test.
The earlier finding identifies imported code, not code added by the paste implementation.

Lead question: `msg_plugin-w_1791753704_c97f90`.
The reviewer asks whether #284 must fix the imported assertions or whether a named `v1` follow-up permits acceptance.
The user forbids private-state assertions in every test and requires every finding closed before exact-head CLEAN.
The reviewer assumes no exception to those rules. The scope decision remains pending.
No code change is requested while that decision is pending. No CLEAN is issued.
The reviewer ran no test, build, or gate.


### Core #284 — lead transfer and pending correction

Lead ruling: `msg_plugin-w_1791753725_87cab0`.
RC-PASTE-DELTA-1 is TRANSFERRED to `F-283-1: program.rs tests assert private state (queued(), unread())`.
Owner: PC, the implementer of #283. Evidence commits: `b47ca04e3ebe6e5f3c49edc25632a2783e3a636a` and `a2a78ed0764e945fce97a70fb489f8d4581fe4dd`.
The lead states: `A transferred finding counts as closed for #284.`
The lead directs #284 to add no correction for the imported assertions.
The imported `waits_for_next_step` assertions remain part of the evidence in RC-PASTE-DELTA-1.
The lead will record F-283-1 in the next plan revision. The transfer does not claim that the assertions are fixed.
RC-PASTE-DELTA-1 is closed for #284 under this explicit scope ruling.

New source head: `006b61cc454e0dce4e0063852e6f0701fef081bc`.
Base: `23550811fb84c3c2cb066019716987575d7aecb2`.
Implementer message: `msg_plugin-w_1791753743_4d6825`.
The reviewer read the complete new delta and the failed gate's header and failure.
The exact-head gate on `dfc38b478b018d63f7bb0851397b2a00a0dd4561` fails `lists` at seed 10, step 15.
The log is `gates/botster-core-stage1-rc-paste-dfc38b47-pool-20261011-141844-22243.log` under `~/botster-sessions`.
The new commit restores the paste ID to `core-pending.txt` and removes its real-pending entry.
Its comment records the testkit seed-10 failure, real seed-0 failure, undiagnosed cause, and RC owner.
The comment does not claim an all-seed testkit PASS.
The reviewed worker files remain unchanged. The ENOSPC hunk and errno test remain correct under R-82.
The imported kernel queue does not change the error table or the ENOSPC classification.
The UTF-8 pending comment retains the named fence gap and P3 owner.
No mutation exclusion changes.

Open package findings: 0 under the lead's transfer ruling.
The new exact-head gate and final PR body remain pending. No CLEAN is issued before those receipts.
The reviewer ran no test, build, or gate.


### Core #284 — integration delta receipt

Integration message: `msg_plugin-w_1791753970_5a12b2`.
Integration receipt: `d845c016eb45d373025f341cf09e7f3477692b7f` in `verdicts/stage1/rc-paste.md`.
The reviewer read the complete final-slot section of that receipt.
It covers `006b61cc454e0dce4e0063852e6f0701fef081bc` against `23550811fb84c3c2cb066019716987575d7aecb2`.
The integration reviewer reports no new source finding.
Its comparison preserves the previous seven own testkit removals and two own real-pending additions.
It accepts the lead's F-283-1 transfer for #284 and preserves that follow-up obligation.
Integration condition RC-PASTE-FINAL-1 remains open for the completed exact-head gate, corrected canonical check, final body, and matching package verdict.
No source or body result from a future gate is accepted by this receipt.
The reviewer ran no test, build, or gate.


### Core #284 — final-slot package verdict

VERDICT: CLEAN.
PR: `https://github.com/trybotster/botster-core/pull/284`.
Exact source head: `006b61cc454e0dce4e0063852e6f0701fef081bc`.
Reviewed base: `23550811fb84c3c2cb066019716987575d7aecb2`.
Gate message: `msg_plugin-w_1791754331_8c1842`.
Open package findings: 0, including LOW findings.
RC-PASTE-DELTA-1 is closed for #284 by the lead's explicit transfer to F-283-1, owner PC.
The transfer preserves the imported defect; it does not claim a fix.

The source delta remains as reviewed in `3ce01a075f4a1656e75e445aab546b0bd413e75e`.
The reviewer read the corrected canonical check in full and compared it with the final body.
Log: `gates/botster-core-rc-paste-base-merge-check-006b61cc-20261011-142236.log` under `~/botster-sessions`.
The check fails the overlap and diff conditions. This explicit delta review supplies the required package review; no automatic Q7 carry is claimed.
The integration source receipt is `d845c016eb45d373025f341cf09e7f3477692b7f`.
The integration reviewer reports no new source finding and retains its final evidence condition.

Final gate log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-paste-006b61cc-pool-20261011-142613-53641.log`.
The header names the exact source head and reviewed base above.
All ten steps pass. The step total is 327.3 seconds; the wrapper exits 0 after 337 seconds on msa1.
The default suite reports 1779 passes. The slow suite reports 578 passes.
Both retained strict testkit summaries report 223 passes and zero failures.
The gate reports 58 mutants: 55 caught, three unviable, no miss or timeout. Stage 2 is not needed.
The reviewer compared all 55 raw catch identities with all 55 citation identities; the sets agree and each identity is unique.
Each cited test has a normal PASS in this gate. No FLAKY record exists.
Both R-82 guard reversals are caught. The three unviable replacements cannot construct their required `Received` value with `Default`.
The mutation exclusion file remains unchanged.

The live PR head matches this verdict. Its final body names this head, base, gate, and corrected canonical check.
The body preserves the failed gate, seed limits, pending correction, F-283-1 transfer, and reported B13 provenance.
The partial-paste ID remains testkit-pending with its undiagnosed seed-10 and real seed-0 failures and RC owner.
No extra pending removal is accepted through this verdict.
The integration reviewer must close its final evidence condition with a completed verdict on this same head and base.
This package verdict accepts no later head or later base.
The reviewer ran no test, build, or gate.


### Core #284 — completed final integration verdict

Integration message: `msg_plugin-w_1791754503_33aa21`.
Integration verdict: `12de56796a6cea14d4b147ce24e6a3c1b0ef9da8`, Round 3 in `verdicts/stage1/rc-paste.md`.
The reviewer read the completed final integration section.
It is CLEAN with zero open findings at `006b61cc454e0dce4e0063852e6f0701fef081bc`.
Its reviewed base is `23550811fb84c3c2cb066019716987575d7aecb2`.
The head and base match package CLEAN `019f5418f9fa153b781c15bd3ed513bd3fc3ed86`.
RC-PASTE-FINAL-1 closes on the completed gate, corrected canonical check, final body, and matching package verdict.
Both final reviews now cover this exact head and base.
The integration reviewer preserves F-283-1 as PC's separate work and the undiagnosed seed-10 paste failure as pending.
The lead received the completed integration CLEAN and controls the merge.
The merge must preserve gated tree `5b13e06270096dfad3b73ea8cf87136c26da0261`.
This receipt accepts no different merge tree or later source change.
The reviewer ran no test, build, or gate.


### Core #285 — final-slot delta review

VERDICT: NOT CLEAN (1 open).
Exact source head: `3cf460714016af6683ea6c60f1753dfc0055c660`.
Reviewed source before the import: `6363d55936ff193c2c1164943b80a3f1adc90ada`.
New base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Message: `msg_plugin-w_1791754855_5e0047`.

The reviewer read the canonical check, source composition, new list change, supplied local records, and live PR body.
The canonical check fails all four conditions. The pending-file merge has a conflict; three control paths overlap.
Log: `gates/botster-core-rc-testkit-base-merge-check-3cf46071-20261011-143954.log` under `~/botster-sessions`.
Both explicit delta reviews are required. No automatic Q7 carry applies.
The reviewer compared all added and deleted lines in the five own code files against the old and new bases.
Those lines remain identical to the reviewed control implementation and tests.
The shared `PtyOutput` parser, real `input_blocked` poll, and testkit stream check preserve their reviewed behavior.
The imported route readiness method and `input_blocked` method remain separate.
The merged test helper retains every pump event for #283's held-write tests.
The conflict resolution retains the own `in_8` group and both imported paste groups.
The five earlier both-tier removals and the earlier `in_8` real-pending entry remain intact.
The only new ID move is `dp_5_bytes_and_text_reach_the_pty_exactly` from testkit pending to real pending.
The reviewer verified its step-13 R-60 exception at contracts-v0.1.39.
The new comment preserves the fence gap and P3 owner despite the single local real-tier PASS.
No new private-state assertion is added by the own delta. F-283-1 remains the lead's separate follow-up for imported assertions.
Every mutation exclusion remains unchanged.

RC-TK-DELTA-1 — LOW — The body retains a superseded all-seed PASS claim.
The section `The Stop item of PR A's table` says the partial-paste session-ended ID passes at every seed with the .39 driver and R-82.
The retained scratch log `botster-core-rc-r82-session-ended-v039-mac-b4626b84-20261011-114024.log` reports one test PASS without a seed range.
Core #284's final-slot gate at `dfc38b478b018d63f7bb0851397b2a00a0dd4561` fails that ID at seed 10, step 15.
That failure remains pending in the imported base. The body also says the ID stays pending until #282 pins .39, although that pin is now imported.
Required change: label the scratch result with its actual evidence limit. Add the current seed-10 failure, undiagnosed cause, pending status, and RC owner.
Remove or correct the statement that the future pin alone ends the pending status.
No additional comparison run is requested.

The local strict record reports 230 passes and zero failures with the uncommitted list edit at the merge head.
The local real record reports one PASS for the new ID move and states the slow tier's default seeds.
The local check reports 397 testkit passes and 192 worker passes after the reported target clean.
RC reports only this worktree built in the target after the clean. The reviewer claims no independent raw clean verification.
The B13 section retains the same-pin wrong-result reproduction and its different-pin negative control.
The new exact-head gate remains pending. The reviewer accepts no future gate result.
The reviewer ran no test, build, or gate.


### Core #285 — final-slot body correction

Implementer message: `msg_plugin-w_1791754946_f0c289`.
Integration message: `msg_plugin-w_1791754945_b77470`, naming the same condition as TC-SI-6.
Source head remains `3cf460714016af6683ea6c60f1753dfc0055c660`.
The reviewer read the corrected live body section and verified that the PR head remains unchanged.
RC-TK-DELTA-1 is closed.
The body labels the historical scratch result as one run with the suite's default seeds, not seeds 0-31.
It states that the .39 pin and #284 are now imported.
It records the retained seed-10 testkit failure and seed-0 real failure at step 15.
It keeps the ID testkit-pending with an undiagnosed cause and RC owner.
It removes the all-seed PASS claim and the claim that the future pin ends the pending status.
No new source change or comparison run is required for this correction.
Open package findings: 0, including LOW findings.
The completed exact-head gate and final body gate result remain pending before CLEAN.
The reviewer ran no test, build, or gate.


### Core #285 — integration final-slot receipt

Integration message: `msg_plugin-w_1791755120_593870`.
Integration receipt: `a21910449ad85860ede1699aa36335ee0026f9a4`, Round 4 in `verdicts/stage1/rc-testkit-controls.md`.
The reviewer read that complete round.
It covers `3cf460714016af6683ea6c60f1753dfc0055c660` against `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
The integration reviewer reports no new source finding. TC-SI-6 closes on the published body correction.
TC-SI-FINAL-1 remains open for the completed exact-head gate, final evidence body, and matching completed package verdict.
The integration reviewer verified the five code deltas, pending conflict resolution, seven testkit removals, and two real-pending additions.
The integration reviewer requests no extra local run and preserves all local evidence limits.
It also verifies that the #284 merge preserves gated tree `5b13e06270096dfad3b73ea8cf87136c26da0261`.
The reviewer accepts no future gate result through this receipt.
The reviewer ran no test, build, or gate.


### Core #285 — failed final gate and retry evidence condition

Exact source head: `3cf460714016af6683ea6c60f1753dfc0055c660`.
Base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Implementer messages: `msg_plugin-w_1791755808_cde7cb` and `msg_plugin-w_1791755842_342b2e`.
Final gate log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-3cf46071-pool-20261011-144458-14865.log`.
The reviewer read the header, failing test record, mutation-stage records, step summary, and final exit.
The header matches the source head and base above.
The first eight steps pass. Mutants fails; fuzz does not run. The wrapper exits 1 after 285 seconds.
Stage 1 reports six mutants: two caught, four missed, no timeout or unviable result.
The unmutated stage-2 baseline fails before stage 2 tests any mutant.
The failure is `conf::ev_5_route_closed_retires_route_keyed_events`, seed 0, step 28.
It expects `activity_before_close: true, closed: true` and receives `activity_before_close: false, closed: true`.
The same test passes in the earlier slow step. No combined mutation PASS is accepted.

The reviewer read the complete transcript at contracts-v0.1.39.
Steps 24 through 26 send one byte, await quiet, and begin Detach without an activity fence.
The own #285 delta changes no transcript, worker code, or `route_reports` control.
The transcript calls neither `scripted` `pty_output` nor `input_blocked`.
These facts support non-interference with the proposed missing-fence mechanism.
They do not prove that mechanism caused this failure or that the failure reproduces on plain `v1`.
The reviewer requests trace evidence and a same-signature plain-v1 or accepted controlled timing reproduction under plan 23al and 23bl.
Request to RC: `msg_plugin-w_1791755860_ed6d16`. Question to the lead: `msg_plugin-w_1791755860_7d682f`.

Lead decision: `msg_plugin-w_1791755874_99ba93`.
The lead states that its earlier approval was incomplete and no accepted reproduction or trace exists.
The lead preserves the 23al conditions and the narrow 23bl controlled-timing exception.
No re-gate is authorized until RC supplies the required evidence and both reviewers accept it.
The source and body findings remain closed. Final acceptance remains pending the required evidence and a valid completed gate.
The reviewer ran no test, build, or gate.


### Core #285 — integration failed-gate receipt

Integration message: `msg_plugin-w_1791755961_542082`.
Integration receipt: `f6558c4799ae3c2b5986ca29511270551d522c41`, Round 5 in `verdicts/stage1/rc-testkit-controls.md`.
The reviewer read that complete round.
The integration verdict remains NOT CLEAN with one final acceptance condition at the unchanged head and base.
It confirms source non-interference but does not claim an executed plain-v1 reproduction.
The integration reviewer reports that the older P3 .36 record has a different symptom and cannot satisfy this exact-signature condition.
The reviewer sent the integration reviewer the lead's correction `msg_plugin-w_1791755874_99ba93`.
That correction supersedes the earlier incomplete conditional approval recorded in Round 5.
No re-gate is authorized until the required evidence and both reviewer acceptances exist.
No duplicate execution is requested by this receipt.
The reviewer ran no test, build, or gate.


### Core #285 — controlled reproduction review

Message: `msg_plugin-w_1791756195_f6306f`.
Plain base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`; #285 is absent from that base.
Pool evidence log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-detached-f68f26a7-f68f26a7-pool-20261011-150206-64641.log`.
Log SHA-256: `b0fa78606981a6fe0621558c35d473d7c0aac59452fa402790443904caedcc8c`.
Scratch diff: `gates/rc-ev5-repro-scratch-f68f26a7.diff` under `~/botster-sessions`.
Diff SHA-256: `027d7d7aad29ad7cd084dd9081f76001d837a3d07671debae1ac858e339f5397`.
The reviewer read the complete 192-line log, complete diff, and affected unmodified worker-loop context.

The evidence job uses the plain base with an uncommitted diagnostic snapshot, `9c44c653e9d71c32f276842b870a4c6a8a8dc203`.
The header marks `--allow-dirty` and states that this is not a merge gate.
The worker and test binaries compile in the pool job. The test exits 101.
The shell then prints the trace and exits zero. That wrapper zero is not a test PASS.
The failure exactly matches the final gate: `conf::ev_5_route_closed_retires_route_keyed_events`, seed 0, step 28.
It receives `activity_before_close: false, closed: true` where activity is expected true.

The diagnostic changes only `crates/botster-worker/src/route_io.rs`.
It postpones delivery of the selected second-route read for one second and keeps the loop runnable while that input waits.
It retains the input value. It changes no terminal bytes, result predicate, report implementation, or transcript.
The control link continues to run while delivery of that route input waits.
The trace records the second-route read at millisecond 1791756150757 and RouteClose in that same millisecond with one held input.
The test fails after 0.09 seconds, before the delayed input is released. The record proves no later release behavior.
The unmodified worker loop reads the control link before it reads routes. Scheduling can leave that route input pending when Detach arrives.
The transcript's `await_quiet` does not fence the worker's client-input report before Detach.
The diagnostic widens that existing scheduling window; it adds no missing behavior or changed assertion result.
RC reports that the diagnostic was never committed or pushed to a delivery branch. The temporary evidence snapshot is not an implementation change.

The reviewer accepts this exact-signature controlled reproduction under plan 23bl A, subject to the lead's explicit acceptance for this case.
The reviewer confirms non-interference: #285 changes no transcript, worker code, or `route_reports` behavior on this path.
The transcript calls neither new control; the route write and flush remain unchanged by #285.
This acceptance does not claim an uninstrumented plain-v1 reproduction or a trace from the original failed gate.
The controlled trace supports the pre-existing unfenced window as the failure mechanism.
The lead must approve one same-head re-gate and retain the cause, owner, and repair PR before RC runs it.
Both reviewers must accept the evidence. A second red gate requires a repair commit under the plan.
Final acceptance remains pending. The reviewer ran no test, build, gate, or diagnostic.


### Core #285 — lead accepts controlled reproduction

Lead decision: `msg_plugin-w_1791756275_141a13`.
The lead accepts the controlled reproduction under plan 23bl A as evidence for condition (1).
The lead records the repair in its handoff: an R-66 activity fence, owner RC, in a contracts transcript PR reviewed by 016c, for .42.
The lead approves ONE re-gate of `3cf460714016af6683ea6c60f1753dfc0055c660`.
That re-gate may start only after the integration reviewer also accepts the reproduction.
Package acceptance is `1ad1761f78871340bc7745580755080544203d8e`.
This decision accepts no future gate result. Final acceptance remains pending.
The reviewer ran no test, build, gate, or diagnostic.


### Core #285 — both controlled-reproduction reviews complete

Integration message: `msg_plugin-w_1791756386_c9e307`.
Integration receipt: `acd9d7f5809f83e6c5da91be89ddc0e722675d05`, Round 6 in `verdicts/stage1/rc-testkit-controls.md`.
The reviewer read that complete round.
It accepts the exact-signature controlled reproduction with the same limits as package receipt `1ad1761f78871340bc7745580755080544203d8e`.
Both reviews confirm non-interference and preserve the test exit 101, wrapper-zero limit, and absence of a release trace.
Both reviewers claim no uninstrumented reproduction or original-gate trace.
The lead's explicit case acceptance is `msg_plugin-w_1791756275_141a13`, recorded above.
Both reviewer acceptances and the lead's approval now permit the one same-head re-gate.
TC-SI-FINAL-1 remains open. The record supplies no gate PASS or final acceptance.
The reviewer ran no test, build, gate, or diagnostic.


### Core #285 — approved re-gate evidence and body finding

VERDICT: NOT CLEAN (1 open).
Head: `3cf460714016af6683ea6c60f1753dfc0055c660`.
Base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Message: `msg_plugin-w_1791756786_0de1e4`.
The reviewer read the approved re-gate header, summaries, mutation results and citations, final exits, and live PR body.
Log: `gates/botster-core-stage1-rc-testkit-controls-3cf46071-pool-20261011-150748-99309.log` under `~/botster-sessions`.
The head and base match. All ten steps pass. The step total is 237.6 seconds.
The job exits 0 after 248 seconds; the wrapper exits 0 after 249 seconds.
The default suite reports 1788 passes. The slow suite and unmutated stage-2 baseline each report 584 passes.
The previously failing identifier passes on the testkit, slow step, and unmutated baseline.
Six mutants are caught, four in stage 2, with no miss, timeout, or unviable result.
All six raw catch identities match six unique citation identities. Each cited test has a normal PASS record.
No FLAKY token exists. This is the one re-gate approved by the lead under the recorded 23bl acceptance.

RC-TK-DELTA-2 — LOW — The final evidence body misstates the failed step and source scope.
The red-final-gate paragraph says `red at step slow`. The actual failure is in `mutants`, in its unmutated stage-2 baseline.
The earlier slow step passes; fuzz does not run in that red gate.
The paragraph also says `This PR changes no file on that path`, although #285 changes `real/route.rs`.
The accepted non-interference finding is narrower: the route write and flush, worker, transcript, and `route_reports` behavior stay unchanged on this path.
The re-gate paragraph calls 248 seconds the job wrapper time; the outer `botster-gate` wrapper reports 249 seconds.
Required change: state the failed step and later non-run step correctly. Use the precise non-interference statement.
Name 248 seconds as the job duration and 249 seconds as the outer wrapper duration, or omit the ambiguous label.
No code change or additional execution is requested.

The final body retains the controlled reproduction, both reviewer acceptances, lead approval, and contracts #69 fence repair.
Final CLEAN remains pending the LOW body correction. The reviewer ran no test, build, gate, or diagnostic.


### Core #285 — final-slot package verdict

VERDICT: CLEAN.
PR: `https://github.com/trybotster/botster-core/pull/285`.
Exact source head: `3cf460714016af6683ea6c60f1753dfc0055c660`.
Reviewed base: `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Gated tree: `886ac9ddad72fc5b92a0c66646aa964965a1b00c`.
Body correction message: `msg_plugin-w_1791756839_1c7f89`.
All package findings are closed, including RC-TK-DELTA-1 and RC-TK-DELTA-2. Open package findings: 0.

The reviewer verified the unchanged PR head and complete final body delta after the approved re-gate.
Only the two affected gate paragraphs change in this correction.
The failed final gate now correctly names `mutants`, its unmutated stage-2 baseline, the earlier slow PASS, and fuzz not run.
The body correctly states that `real/route.rs` changes while the route write/flush and other relevant behavior remain unchanged.
The re-gate paragraph separates the 237.6-second step total, 248-second job duration, and 249-second outer wrapper duration.
The final body preserves the seed-10 pending failure, local evidence limits, B13 provenance, conflict resolution, and canonical check.
It also preserves the red gate, exact controlled reproduction, both reviewer acceptances, lead approval, and contracts #69 repair.

The complete source delta remains as reviewed in `5480a940abc4fde52a9c2d656b0de559f5a56de4` and corrected in the later receipts.
The canonical check fails; the explicit package and integration delta reviews supply the required reviews. No automatic Q7 carry is claimed.
The approved re-gate is:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-testkit-controls-3cf46071-pool-20261011-150748-99309.log`.
Its header names the exact source head and base above. All ten steps pass; the wrapper exits 0 after 249 seconds.
It catches all six mutants, four in stage 2, with no miss, timeout, or unviable result.
All six raw catch identities match six unique citations. Each cited test has a normal PASS. No FLAKY token exists.
The previously failing test passes on the testkit and in both unmutated slow runs.
The controlled-reproduction acceptance and one-re-gate approval remain within their recorded 23bl limits.
The green re-gate does not claim that contracts #69 has landed or that the unfenced window is fixed.
The new bytes-and-text ID remains real-pending for its R-60 fence gap, owner P3.
F-283-1 remains the separate imported-code follow-up, owner PC, under the lead's scope ruling.

The integration reviewer must close TC-SI-FINAL-1 with its completed matching verdict.
The merge must preserve the gated tree above. This verdict accepts no later head, base, or different tree.
The reviewer ran no test, build, gate, or diagnostic.


### Core #285 — completed final integration verdict

Integration message: `msg_plugin-w_1791757363_0c54a7`.
Integration verdict: `3320652a81221ad3e01f936195ffcb03f2abffc1`, Round 7 in `verdicts/stage1/rc-testkit-controls.md`.
The reviewer read that complete round.
The integration verdict is CLEAN with zero open findings at `3cf460714016af6683ea6c60f1753dfc0055c660`.
Its reviewed base is `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`.
Its gated tree is `886ac9ddad72fc5b92a0c66646aa964965a1b00c`.
These values match package CLEAN `14e1ce867f63dfd36388f7a47b73d19cd085fe09`.
TC-SI-FINAL-1 closes on the approved re-gate, corrected final body, and matching package verdict.
Both final reviews now cover this exact head, base, and tree.
The integration reviewer retains all controlled-diagnostic limits, contracts #69 as a separate repair, and F-283-1 as PC's separate follow-up.
The lead received the completed integration CLEAN and controls the merge.
The merge must preserve the gated tree. This receipt accepts no later source or base change.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — final-slot delta receipt

Exact source head: `46c8c5481a3cdf11ec98949227fda32f4d0f0ee0`.
New base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Source tree: `abb16eb98ca2c99349976ea9a72d43a617293501`.
Previously reviewed head: `41c1f7e25b04f2174f4ec6a9b3838f1aeeb3ab56`.
Message: `msg_plugin-w_1791761559_9487b6`.

The reviewer read the final-slot body, complete canonical check, code composition, conflict resolution, and new list delta.
The canonical check fails all four conditions and requires both delta reviews.
Log: `gates/botster-core-rc-oracle-base-merge-check-46c8c548-20261011-163114.log` under `~/botster-sessions`.
Both pending files have conflicts. The non-list overlap is `crates/botster-core-testkit/src/real.rs`.
No automatic Q7 carry applies.
The reviewer compares the added and deleted lines in all five own code files against their old and new bases.
Those lines remain identical, including the corrected tests and fresh-terminal mode base.
The imported hold and release dispatch arms remain separate from the six oracle dispatch arms.
The oracle guards and real replay source preserve the reviewed behavior.
The merge adds no own terminal semantics, model form, private-state assertion, or mutation exclusion.
F-283-1 remains the separate follow-up for imported private-state assertions under the lead's scope ruling.

The reviewer independently checks the three-way identifier formula in four files.
The merge sets for `core-pending.txt`, `core-real-pending.txt`, `minimum-core.txt`, and `core-ledger-ids.txt` exactly match that formula.
The merge preserves the six prior `v1` removals that the first rerere result had retained incorrectly.
The real-pending resolution retains the oracle group and the imported groups.
The later commit changes only the two pending files.
It moves four IDs from testkit pending to real pending: worker-mode keys, kitty modifier keys, and both mouse-mode groups.
The reviewer reads their contracts-v0.1.39 fence exceptions.
They have respectively one, two, nine, and thirteen R-56 entries, and one R-60 entry each.
Their comments retain these gaps and P3 ownership. The move claims no fenced real-tier PASS.
The named ev_8 comment exists at all four comparison heads; the canonical diagnostic does not prove its removal.

The supplied local log retains the 1161-test default summary at the merge head, prebuild result, and clippy count zero.
It labels the run as after a target clean but retains no independent raw clean proof.
Its strict-run excerpt contains no final strict summary.
RC separately reports 245 strict passes at the final head with seeds 0-31. The exact-head gate must supply final evidence.
The body distinguishes the merge-head log from the final-head owner report.
Open package findings: 0, including LOW. The earlier source and body findings remain closed.
The completed exact-head final gate and final body result remain pending before CLEAN.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — integration final-slot delta receipt

Integration message: `msg_plugin-w_1791761880_c8d289`.
Integration receipt: `5bfb2bbee7e98ead84fdd3fea7f2d4c31b72c266`, Round 4 in `verdicts/stage1/rc-oracle-encode.md`.
The reviewer read that complete round.
It covers `46c8c5481a3cdf11ec98949227fda32f4d0f0ee0` against `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Its tree is `abb16eb98ca2c99349976ea9a72d43a617293501`, matching the package delta receipt.
The integration reviewer reports no source or body finding in the delta.
Its independent code, four identifier-set, and fence-exception checks agree with package receipt `668b82aeb5364db5afd21fac477dce04cc7a2fdc`.
It preserves the incomplete local strict record as an owner report.
One final acceptance condition remains: completed exact-head gate, removal evidence, final body, and completed package verdict.
The reviewer accepts no result from the running gate. The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — failed final gate

Head: `46c8c5481a3cdf11ec98949227fda32f4d0f0ee0`.
Base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`.
Message: `msg_plugin-w_1791762337_dda900`.
Log: `gates/botster-core-stage1-rc-oracle-encode-46c8c548-pool-20261011-163127-84246.log` under `~/botster-sessions`.
The reviewer read the exact header, failing baseline record, stage summary, step summary, and final exit.
The first eight steps pass. Mutants fails in its unmutated stage-2 baseline; fuzz does not run.
Stage 1 reports 32 mutants: 18 caught, twelve missed, two unviable, no timeout.
Stage 2 tests no mutant because its baseline fails. No combined mutation PASS is accepted.
The step total is 658.2 seconds; the wrapper exits 1 after 771 seconds on Linux.

The failing ID is `conf::st_6_capture_is_frozen_at_model_rev`, seed 0, step 17.
Its expected relation is `{"ne":[1,1]}`; the relation does not hold.
The same ID passes on the testkit and in the earlier slow step.
The reviewer read the complete pinned .39 transcript.
The second `pty_output` writes hex `42`; `await_quiet` precedes ReadScreen and the revision comparison without a model-read fence.
The own #288 delta changes no worker model feed or program-control path. The transcript calls no oracle control.
These facts support non-interference with the proposed unfenced model-read mechanism.
They do not prove that mechanism caused the failed gate or that the failure reproduces on plain `v1`.

RC reports that it stopped and ran no re-gate. RC proposes one controlled timing reproduction on plain base `f6025aed` through the lead.
The reviewer awaits the completed evidence and the applicable lead decision under plan 23al and 23bl.
No re-gate is accepted before both reviewers accept the exact-signature evidence and the lead approves the case.
Source and body findings remain closed. Final acceptance remains pending valid final evidence.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — integration red receipt and controlled reproduction

Integration verdict: `99cb66d8d627c6de947678ee0100892602377021`, Round 5 in `verdicts/stage1/rc-oracle-encode.md`.
Message: `msg_plugin-w_1791762517_785813`.
The reviewer read the complete round. It preserves the red baseline, four testkit removal PASS records, and static attribution limits.
It establishes no new oracle source defect and accepts no missing reproduction evidence.

Lead direction: `msg_plugin-w_1791762533_8e2986`.
The lead authorizes one controlled reproduction on plain `v1` and directs a new-head final gate after #294 and the .42 repair import.
Both reviewers must accept the completed reproduction before that gate.
Reproduction message: `msg_plugin-w_1791762548_76b6fe`.
Plain base: `f6025aed4cdf7da836e835e0040fc128b7e8f32f`; #288 is absent.
Evidence snapshot: `bf55aa2fb5615c58333f87990433e127224d891e`, marked `--allow-dirty`, not a merge gate.
Contracts pin: `ddce507e2c08e740f899e97dcfec566f3d2275b8`, contracts-v0.1.39.
Transcript blob: `81ece132501f26262d8fcac4396b09da9abec237`.

Log: `gates/botster-core-detached-f6025aed-f6025aed-pool-20261011-164814-5086.log` under `~/botster-sessions`.
Log SHA-256: `a21e40b2b2db7710acd8cc0eb3572a726cd27f3720debcdde6db0f49e7428c09`.
Scratch diff: `gates/rc-st6-repro-scratch-f6025aed.diff` under `~/botster-sessions`.
Diff SHA-256: `5348e52c1741df1b50ffa31fc676681db95e964507a9ae1ded72936bce7f22ca`.
The reviewer read the complete log, complete diff, unmodified worker-loop and PTY-read context, and named repair diff.
The pool builds the worker and test binaries. It reinstalls the pinned .39 probe before the test.
The test runs with seed 0 and exits 101 after 1.05 seconds.
The exact failure matches the red gate: ST-6 capture ID, step 17, `{"ne":[1,1]}` does not hold.
The shell prints the trace and exits zero. Its wrapper zero is not a test PASS.

The scratch changes only the worker driver in `crates/botster-worker/src/main.rs`.
It delays the first exact `B` PTY read by one second, retains those bytes, and prevents another PTY read until release.
It shortens the poll timeout while delivery waits. The control link and model requests can still proceed.
It changes no transcript, predicate, model-revision value, or terminal bytes.
The trace records one earlier read, the held `42` read at 1791762509598, and release at 1791762510598.
The trace contains no timestamp for the ReadScreen answer. Its unchanged-revision failure is consistent with an answer before the delayed feed.
The reproduction widens the existing model-feed scheduling window. The original control-first worker loop already permits that window.
The reviewer accepts this exact-signature controlled reproduction within plan 23bl A, subject to the lead's explicit case acceptance.
The reviewer confirms non-interference: #288 changes no worker model feed, program-control path, transcript, or model-read fence.
The failing transcript calls no oracle control. No own change supplies the reproduction's mechanism.
RC reports that the diagnostic is never committed to a delivery branch. The temporary evidence snapshot supplies no implementation change.
This acceptance claims no uninstrumented plain-v1 reproduction or original-gate trace.

Repair commit `72eb31ba198c39cb0a8510978749262d7863e91a` replaces both ST-6 await_quiet steps with Output Activity waits at 5000 and 5001.
The reviewer read that contracts diff. The lead states that the repair arrives with the .42 pin and #294.
The next Core head must import that repair and receive the applicable exact-head reviews and final gate.
This receipt accepts no future pin import, gate result, or final body.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — lead accepts ST-6 reproduction and assigns repair import

Lead decision: `msg_plugin-w_1791762655_ac8cbb`.
The lead accepts the completed controlled ST-6 reproduction for this case and records package receipt `843604d707cfb4a6a9049e54da46fba2ff4a001d`.
The integration reviewer's acceptance remains due.
The lead reports #294 merged at `v1` `3aacc58d086e55a6b186165ccf9001e2618e2869`.
The lead assigns the slot to #288 and directs RC to import that `v1` once and post the canonical base-merge-check.
The final gate may start only after the integration reviewer also accepts the reproduction.
The future import requires its applicable delta review. This receipt accepts no imported source or future gate result.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — disjoint import of #294 checks

Message: `msg_plugin-w_1791762717_e40b47`.
New head: `09e9fbc8b26456be592ce99d9814fcea4695536e`.
New base: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
Tree: `e4e38e9184c645d9e78db8b66aa490ca001a7b35`.
The merge has parents `46c8c5481a3cdf11ec98949227fda32f4d0f0ee0` and the new base above.
The reviewer read the complete canonical check, strict-log header and result, four moved-ID results, and live body.
Canonical log: `gates/botster-core-rc-oracle-base-merge-check-09e9fbc8-20261011-165140.log` under `~/botster-sessions`.
All conditions pass. The base changes only `xtask/src/fsutil.rs` and `xtask/src/lists.rs`.
Neither path is in the own diff. The check reports the non-list own diff byte-identical at 42637 bytes.
Its SHA-256 is `0b0844ddb5606e1bf9f7823152d1757b9c5b7eef90dbd0fa127925b3538123b1`.
Both pending files remain unchanged. The reviewer independently verifies no oracle-code, pending-file, or mutation-exclusion delta from the prior head.
No new package source delta round is required for this import. This is no final CLEAN carry: final acceptance was still pending.

Strict log: `gates/botster-core-rc-oracle-strict-09e9fbc8-20261011-165122.log` under `~/botster-sessions`.
Its header names this exact head and seeds 0-31 with pending-strict enabled.
The record begins with a refusal because `NEXTEST_TEST_THREADS` is missing, then retains the conformance results.
It reports 245 passes, zero failures, and normal `ok` results for all four moved IDs.
That result supplies a retained local summary; the refused command is not a passing xtask check.
The final exact-head gate must supply acceptance evidence with the required resource settings.

This import retains contracts-v0.1.39. It does not import the .42 R-80 fence repair.
The earlier receipt's repair-import wording describes the planned repair; it does not describe #294's two-file check import.
The accepted controlled reproduction and lead direction remain necessary for the next gate while the cause is unchanged.
The integration reviewer's reproduction acceptance remains due in the messages received by this reviewer.
The final body must name this new head/base and canonical check and retain the completed reproduction and gate disposition.
Its current final-slot section still describes the previous head; no final body is accepted yet.
Open package source findings: 0. Final acceptance remains pending.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — worker provenance and both reproduction acceptances

RC message: `msg_plugin-w_1791762810_31028a`.
RC identifies the refused command in the strict log as prebuild-worker, before it built anything.
RC reports that the strict run uses the candidate worker prebuilt at the merge head `78aa05416af2c61ef8c316076ab3a5d7c9fd27c2`.
The reviewer independently verifies no path under `crates/` changes between that head and `09e9fbc8b26456be592ce99d9814fcea4695536e`.
That comparison supports source equality for the candidate worker. It does not turn the refused prebuild command into a PASS.
The final exact-head gate remains required. The live body has not yet published the proposed provenance paragraph.

Integration message: `msg_plugin-w_1791762819_9064bc`.
Integration acceptance: `5fa42e4aa2dbf186fdf7ad9a7c4308af4ca2210f`, Round 6 in `verdicts/stage1/rc-oracle-encode.md`.
The reviewer read the complete round.
It accepts the same controlled ST-6 record under the lead's case decision, with matching hashes, symptom, and limits.
Both reviewer acceptances and the lead's explicit case acceptance now cover the controlled attribution.
The integration reviewer confirms that #294 imports checks and retains contracts .39; the fence repair remains future work.
The new-head gate therefore uses the accepted cause disposition under plan 23an(a), not a claim that the repair is imported.
The current canonical check passes, as recorded in package receipt `5c4048614f24c313c8e5edd06e17ca50cdb19d76`.
Final acceptance still requires the completed exact new-head gate, final body, and matching final verdicts.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — integration Q7 import receipt

Integration message: `msg_plugin-w_1791762939_6c9b11`.
Integration receipt: `b477466bae49d7439698ea42510170649ae272a0`, Round 7 in `verdicts/stage1/rc-oracle-encode.md`.
The reviewer read the complete round.
It covers head `09e9fbc8b26456be592ce99d9814fcea4695536e`, base `3aacc58d086e55a6b186165ccf9001e2618e2869`, and tree `e4e38e9184c645d9e78db8b66aa490ca001a7b35`.
The integration reviewer verifies Q7 source carry, all seven own blobs, and the exact non-list patch.
The controlled attribution retains its unchanged-cause scope; the imported pin remains .39.
It preserves the refused prebuild invocation and requests later strict-command provenance without another run.
The reviewer sent RC's received worker-provenance clarification and the independent no-crate-change comparison to the integration reviewer.
Final acceptance still requires the completed exact-head gate, final body, and completed package verdict.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — final-slot package verdict

VERDICT: CLEAN.
PR: `https://github.com/trybotster/botster-core/pull/288`.
Exact source head: `09e9fbc8b26456be592ce99d9814fcea4695536e`.
Reviewed base: `3aacc58d086e55a6b186165ccf9001e2618e2869`.
Gated tree: `e4e38e9184c645d9e78db8b66aa490ca001a7b35`.
Gate message: `msg_plugin-w_1791763700_2fc725`.
Open package findings: 0, including LOW findings. The earlier source and body corrections remain closed.

The source delta and pending moves remain as reviewed in `668b82aeb5364db5afd21fac477dce04cc7a2fdc`.
The later import has the canonical Q7 PASS reviewed in `5c4048614f24c313c8e5edd06e17ca50cdb19d76`.
The matching integration source-carry receipt is `b477466bae49d7439698ea42510170649ae272a0`.
The controlled ST-6 acceptance is package `843604d707cfb4a6a9049e54da46fba2ff4a001d` and integration `5fa42e4aa2dbf186fdf7ad9a7c4308af4ca2210f`.
The lead accepts that exact case in `msg_plugin-w_1791762655_ac8cbb` and directs the new-head slot sequence.
The accepted unchanged-cause disposition applies under plan 23an(a); no .42 repair import is claimed.

Final gate:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-rc-oracle-encode-09e9fbc8-pool-20261011-165511-49979.log`.
The reviewer read the exact header, strict summaries, suite summaries, mutation results, catches and citations, relevant PASS records, and final exits.
The header names the exact source head and reviewed base above.
All ten steps pass. The step total is 667.8 seconds; the job and wrapper exit 0 after 755 seconds on msa1.
Both retained conformance summaries report 245 passes and zero failures.
The default suite reports 1828 passes. Both unmutated slow runs report 599 passes.
The ST-6 ID passes on the testkit, earlier slow step, and unmutated stage-2 baseline.
All four new testkit removals have normal conformance PASS records in this exact-head gate.
They retain their real-pending fence gaps and P3 owner; no extra real-tier acceptance is granted.
The gate reports 32 mutants: 29 caught, eleven in stage 2; three unviable; no miss or timeout.
All 29 raw catch identities equal 29 unique citation identities. Each cited test has a normal PASS. No FLAKY token exists.
The three unviable replacements are the generic input parser and the two raw-terminal construction functions.
The mutation exclusions remain unchanged. Fuzz passes with no changed decoder crate selected; no fuzz execution is claimed.

The live PR head matches this verdict. The final body names this head, base, passing canonical check, and completed gate.
It preserves the red gate, exact failure, controlled diagnostic, test-exit and wrapper-zero distinction, both acceptances, and lead case decision.
It retains the refused local prebuild separately from the successful strict run and states the unchanged candidate-worker source.
The trace has no ReadScreen-answer timestamp; the answer ordering remains an inference from the result and source, as recorded in both acceptance receipts.
The final green gate does not prove that the unfenced window is fixed. The R-80 fence still awaits the .42 pin.
F-283-1 remains the separate imported-code follow-up under the lead's scope ruling.

The integration reviewer must publish the completed matching final verdict.
The merge must preserve the gated tree above. This verdict accepts no later head, base, or different tree.
The reviewer ran no test, build, gate, or diagnostic.


### Core #288 — completed final integration verdict

Integration message: `msg_plugin-w_1791763948_278534`.
Integration verdict: `60369df5477c5b31d3bb20ce944ffb444bf96cfe`, Round 8 in `verdicts/stage1/rc-oracle-encode.md`.
The reviewer read the complete round.
It is CLEAN with zero open findings at `09e9fbc8b26456be592ce99d9814fcea4695536e`.
Its reviewed base is `3aacc58d086e55a6b186165ccf9001e2618e2869`.
Its gated tree is `e4e38e9184c645d9e78db8b66aa490ca001a7b35`.
These values match package CLEAN `54371ca09bcad6edb6b449418c791c49148c5a43`.
The integration reviewer verifies all own testkit removals, all 29 catch/citation pairs, and all twelve second-stage outcome identities.
Its final acceptance condition closes on the gate, final body, matching package verdict, and accepted cause disposition.
Both final reviews now cover this exact head, base, and tree.
The red gate, controlled-proof limits, future R-80/.42 fence work, and separate F-283-1 follow-up remain explicit.
The lead received the completed integration CLEAN and controls the merge.
The merge must preserve the gated tree. This receipt accepts no later source, pin, or base change.
The reviewer ran no test, build, gate, or diagnostic.


### Core #299 — contracts .43 pin review

VERDICT: NOT CLEAN (2 open).
PR: `https://github.com/trybotster/botster-core/pull/299`.
Head: `18334c6fe59d11744ba57bc1e7da63c7303300b2`.
Gate base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
Last imported base: `b3cdfffbba353bf9cfc34f7d7f9b8daccc753035`.
Messages: `msg_plugin-w_1791773303_011fcf` and `msg_plugin-w_1791773325_9629ca`.
Risk: HIGH, for the pin move, shared testkit, and CI job.

RC-PIN43-1 — MED — The new progress tests assert private state.
In `route_client.rs`, the new test asserts `client.unsent()` at lines 625, 630, and 648.
That accessor returns the stored `unsent` vector's length at line 242.
In `real/route.rs`, the new test reads `client.unsent.len()` at lines 545, 551, and 569.
It also uses private `unsent.is_empty()` as the loop condition at line 570.
The user forbids private-state assertions. BUILD.md rule 3 also forbids tests of helper internals.
Required change: observe the public `progress()` result and exact peer-visible bytes, including the final false result.
Use delivered bytes and a finite progress bound to end the real test; do not use the private queue as its completion condition.
Keep false-with-no-work, false-while-full, true-after-room, and final-false behavior proofs on both tiers.

RC-PIN43-2 — MED — Discarding a failed write reports byte progress.
Both implementations compare `unsent.len()` before and after a flush: `route_client.rs:353-357` and `real/route.rs:499-501`.
The testkit flush clears `unsent` on a terminal write error at `route_client.rs:226-229`.
The real flush clears it on a missing end, and `after_write` clears it on a terminal error at `real/route.rs:186-188`.
Thus a full stream with waiting client bytes can lose its peer, write no byte, discard the waiting bytes, and return true.
The pinned .43 trait requires `True when it moved a byte`; work that moves no byte does not count.
A shrinking queue proves no successful delivery when the flush also discards bytes.
Required change: track successful bytes accepted by the stream, separately from bytes discarded on failure.
Return false for a failure before any byte is accepted. Preserve true when a partial successful write precedes the failure.
Add public behavior regressions for the failed-stream/no-byte case on both tiers.
No runtime reproduction was run by this reviewer; the write and flush branches establish this source defect.

### Other source and evidence checks

The reviewer reads the complete own diff, published body, pinned R-86 trait and rulings, and the changed control definitions.
The pin changes the seven workspace dependency tags and nine contracts lock source lines, with no other lock change.
The pinned CI job is byte-identical to contracts-v0.1.43, SHA-256 `0bf5dbc170372d33230a112d0b1562ff17a971e5810f22c5ae46e85cf86acc11`.
The fetch file changes only its header; the lead handoff records the per-project fetch ruling and steward agreement.
The shared job separates download containers from offline containers and retains the token-free public fetch.
R-86 explicitly authorizes the client-progress form. It introduces no unruled model or terminal semantics.
The real progress test uses a UnixStream pair in the slow tier, without a child process, sleep, or disk socket.
The reviewer checks the inherited exclusion delta since the last RC source review: held-full and bounded-conformance process glue only.
Their comments name the tested decisions and required gate reports. No exclusion matches either new progress decision.
The own mutation configuration is unchanged.

The reviewer reads all three relevant .43 transcripts and verifies zero fence-exception entries for them.
The parked-work ID has its R-88 precondition fence and no PTY or model read.
The minimum wake ID has the .43 held-write fence through public `pty_queued`; no PTY or model read is asserted.
The full-input ID keeps its program-ready fence, blocked empty read, and full-count await_pty before its exact input read.
It stays testkit-pending with the new budget hold. The lead handoff records approval and RC's large-fill cost repair ownership.
The gate retains the held-full proof for all three budget IDs at all 32 seeds.
The focused pool evidence retains real wake-ID PASS with BOTSTER_SEEDS=0-31 and the local pending removal captured as an evidence snapshot.
The seed-check job prints the seed values and retains both zero-seed and 0-31 results. It is not a merge gate.

The A5-4 tables contain all 691 identifiers on both tiers.
Their changed-row lists retain one real pending PASS-to-FAIL: the raw-data ID receives the earlier input's PTY echo.
The .41 and .43 transcript and fence exception are unchanged for that ID. The body preserves the failure and keeps it real-pending.
The before/after jobs use default seeds 0-7. They do not prove all-seed removal of the other real-pending IDs.
The filtered reversal log retains four failed runs but no full assertion or build output; the exact-head mutation gate supplies the named catches.

Gate: `gates/botster-core-stage1-rc-pin-42-18334c6f-pool-20261011-194125-47145.log` under `~/botster-sessions`.
The exact head and gate base match. All ten steps pass; job and wrapper exit 0 after 357 seconds.
Ten mutants are caught, five in stage 2, with no miss, timeout, or unviable result.
All ten raw catch identities match ten unique citations; each catcher has a normal PASS. No FLAKY token exists.
The two progress tests and both removed IDs have normal PASS records. Green mutation results do not close the two source findings above.
The body retains the earlier budget failure and four missed progress mutants with their fixes.
Both findings must close, and any changed source head needs its applicable review and gate evidence.
The final-slot current-base import, checks, gate, and final body remain due after both source CLEANs.
The reviewer ran no test, build, gate, or diagnostic.


### Core #299 — progress correction receipt

Messages: `msg_plugin-w_1791773639_8ddc55` and `msg_plugin-w_1791773672_789ed2`.
New head: `3f1fc94d86f6a43c5c5ef310791af676fe5b4ecb`.
The reviewer read the complete source delta through intermediate `e52255f91363b2f46eee47847270cbc70c7b9456` and the final test-only change.
Only the two route files change. No pin, list, CI, or mutation-exclusion change is added in this correction.

RC-PIN43-1 is closed in the source.
Both tests remove the private `unsent` observations and private loop condition.
The testkit test checks the public progress result and exact bytes read by the peer.
The real test checks every received byte is `x` and the total is exactly 4 MiB.
Its loop ends on the received count, with a finite 10000-pass failure bound.
Both tests retain false with no work, false while full, true after room, and final false.

RC-PIN43-2 is closed in the source.
The testkit flush and real flush_into return the sum of successful `Ok(n)` writes.
Terminal errors still discard waiting bytes under OU-5, but add nothing to the successful-byte count.
The testkit progress sums the two flush counts around run_ready. The real progress tests its flush count against zero.
A partial successful write before an error retains its successful-byte count; an error before any successful write yields zero.
The reviewer checks the existing callers; they ignore the new count and preserve their prior flush behavior.
Both new regressions observe false progress when queued bytes are discarded on a failed stream.
The testkit regression also reads no delivered suffix from the peer.
The real regression drops the peer after filling the stream, then observes false progress without reading private state.
No test asserts private state in this correction.

The reviewer reads the complete filtered reversal log `evidence/rc-pin-42/reversal-progress-e52255f9.log` under the shared core-stage1 directory.
Seven reversals fail the named progress tests, including both old length-comparison forms at the discarded-byte assertion.
The log ends with zero changed files. It contains no complete build output or independent target-clean proof.
RC reports passing local tests and clippy; those reports do not replace the new exact-head gate.
Open package source findings: 0, including both MED findings.
The new gate is running. Final source CLEAN awaits its completed exact-head evidence and matching body update.
The prior head's green gate proves no later correction.
The reviewer ran no test, build, gate, or diagnostic.


### Core #299 — failed correction gate and body precision

VERDICT: NOT CLEAN (1 open body finding; final gate disposition remains pending).
Head: `3f1fc94d86f6a43c5c5ef310791af676fe5b4ecb`.
Base: `bfef88653bb59ad8d3cd38da1e206f7aba64ce92`.
RC message: `msg_plugin-w_1791773989_197c4d`.
Log: `gates/botster-core-stage1-rc-pin-42-3f1fc94d-pool-20261011-195433-4196.log` under `~/botster-sessions`.
The reviewer reads the exact header, failing baseline record, stage results, step summary, and final exit.
Eight steps pass. Mutants fails in its unmutated stage-2 baseline; fuzz does not run.
Stage 1 reports 26 mutants: thirteen caught, thirteen missed, no timeout or unviable result.
Stage 2 tests no mutant. No combined mutation PASS is accepted.
The step total is 256.9 seconds; the wrapper exits 1 after 265 seconds on msa1.

The failed test is `real::resume::slow_tests::the_real_form_compares_both_captures_with_the_raw_output` at resume.rs line 278.
It receives `Err(UnsupportedFor("events_pending"))` instead of `Ok({"equal": true})`.
The same test passes in the earlier slow step.
The reviewer reads the complete affected test prefix, its title/capture helpers, and capture_now's event-refusal branch.
The own delta changes no resume, capture-layer, worker, model, or program-control code.
This direct harness test calls no route progress method. These facts establish static non-interference with the proposed late-event mechanism.
They do not prove which late event caused this failure or a matching plain-v1 reproduction.
RC proposes the 23al/23bl path through the lead. The reviewer accepts no missing trace or reproduction and no future re-gate.

RC-PIN43-3 — LOW — The body overstates the retained pending set.
Integration message `msg_plugin-w_1791774035_5cc0fd` identifies the same condition.
The seed-0 file contains 37 IDs and includes `tm_6_begin_and_attach_signal_wake`.
The body says this PR does not touch all 37, although it removes that wake ID through separate 0-31 real-tier evidence.
The A5-4 paragraph also says all six real-pending IDs that become PASS stay pending, although the wake ID is separately removed.
Required change: state that 36 seed-0 passes remain pending and the wake ID is the separately proved removal.
Keep the A5-4 seed-0-7 result distinct from the later 0-31 removal evidence.
No code change or extra execution is requested for this body correction.

RC-PIN43-1 and RC-PIN43-2 remain closed in the corrected source.
The final body must retain this completed red gate and the later accepted disposition, if any.
Final CLEAN requires the body finding closed and valid completed exact-head evidence under the lead's retry rules.
The reviewer ran no test, build, gate, or diagnostic.


### Core #299 — separate resume-test repair review

VERDICT: NOT CLEAN (2 open: RC-PIN43-3, RC-PIN43-4).
Head: `d8ae1c9d200e2cd7790ce3e1ee61ed39c5dcd91e`.
Message: `msg_plugin-w_1791774156_0def18`.
The reviewer reads the complete one-file test delta, scratch script, and retained scratch results.
The lead handoff directs a separate test-only root fix in #299 and an ordinary gate on the new head.
This is a source repair, not a same-head re-gate exception.
The helper polls events before the first successful resume call; the later explicit events_pending refusal check remains unchanged.
The public diagnostics baseline is taken after the quiet call, so its before/after comparison retains the quiet-capture scope.
No production resume behavior, capture-layer behavior, or R-69 refusal changes.

RC-PIN43-4 — MED — The new quiet helper can bypass its cleanup deadline.
At resume.rs lines 177-187, the inner loop pumps and polls until `report.more` is false.
The cleanup-deadline check is outside that loop, at line 191.
A Core that continually reports more never reaches the deadline check or exits that loop.
Thus the helper can spin beyond the cleanup bound it claims to enforce.
R-89 states the same wait principle: reported work must not extend the deadline. BUILD.md requires bounded waits.
Required change: check the deadline on every pump pass, or flatten the helper's loop so more cannot bypass it.
Preserve event draining, the edges-quiet check, and the later explicit events_pending refusal proof.

Scratch record: `evidence/rc-pin-42/resume-late-event-d8ae1c9d.log` under the shared core-stage1 directory.
Its script adds an output on s2 and pumps without polling immediately before the first resume.
Without the quiet call, the unchanged resume assertion fails with events_pending and test exit 101.
With the same injected output before quiet, the test passes. The unchanged corrected test also passes.
The script restores the source after each scratch edit and reports zero changed files at the end.
This diagnostic adds an event; it is not a timing-only controlled reproduction under 23bl A.
It demonstrates the repair's event-draining behavior, not an original-gate trace or uninstrumented plain-v1 reproduction.
The source repair needs neither claim for an ordinary new-head gate under the lead's disposition.
The filtered record identifies the head and target directory but contains no full build output or independent target-clean proof.
RC-PIN43-1 and RC-PIN43-2 remain closed. RC-PIN43-3 awaits the published body count correction.
The running gate supplies no accepted result. Any later source correction needs its applicable exact-head review and gate.
The reviewer ran no test, build, gate, or diagnostic.


### Core #299 — deadline and body corrections

Message: `msg_plugin-w_1791774390_8dec67`.
Head: `d919844dfa2b74e768987c8cfb980fba0d25a8e8`.
The reviewer reads the complete source delta and complete published body delta.
The source changes only the test helper in real/resume.rs; no production code or exclusion changes.

RC-PIN43-4 is closed.
The quiet helper now has one flat loop. Each continuing pass checks the cleanup deadline, including a pass with `more: true`.
It polls events after each pump and returns only when the pump has no more work and edges_quiet is true.
A pass with more continues without waiting; a pass without more waits on the wake handle under the remaining deadline.
The helper's marked deadline wait remains explicit. The later events_pending refusal test stays unchanged.

RC-PIN43-3 is closed in the body.
The body states that 36 seed-0 passes stay real-pending and the wake ID is the one separate 0-31 removal.
The A5-4 six PASS changes remain historical seed-0-7 evidence; five remain real-pending and the wake ID leaves later.
The body records the successful-byte-count fix, public byte observations, discarded-byte regressions, and partial-write test limit.
It labels the synthetic resume diagnostic as an injected-event case, not timing-only 23bl evidence.
It retains the 3f1fc94d red baseline, eight earlier PASS steps, fuzz not run, and the separate test repair.
RC reports that the d8ae1c9d job stopped while queued before submission. The body claims no result from it.

RC-PIN43-1 and RC-PIN43-2 remain closed. Open package findings: 0, including LOW.
The gate on this exact corrected head remains pending. No source CLEAN is issued before its completed evidence and body result.
The prior heads' gates prove no later source change.
The final-slot current-base import and final gate still remain due after both source CLEANs.
The reviewer ran no test, build, gate, or diagnostic.
