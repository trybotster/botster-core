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
