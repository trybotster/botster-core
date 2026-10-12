# core-e3 notes: Core transcripts for erratum 3 (due deadlines and the `pump_events` budget)

Branch `foundation/core-e3`. Clause: Core erratum 3 candidate 3 (E3-1, `frozen/current/core-contract-v1.17-erratum-3-candidate3.md`). Seven transcripts in `conformance/core`. FakeCore is not changed; each transcript is `fake: structural` (R-12) and runs against the real Core testkit in Stage 1. One new runner step, `poll_find` (it polls the queue until empty, never pumps, and takes the first matching event), and one new control: `signals_sent`, a read-only observer at the process edge (R-14.3) that shows the kill in the pump at which it is due.

## How each item is observed

All timing comes from the injected clock (`advance_clock`) and the report of one `pump` (`more`, `events_posted`). Every silence anchors at monotonic time 0, so its due time is its threshold in milliseconds. **Setup order:** the runner moves the injected clock to the next existing deadline in an idle wait (`pump_until`). So no setup step after the first armed deadline calls an idle wait or moves the clock. The route's reader deadline and the `stop_grace` of a Stop are armed before the queue fill, and the steps after them are `await_quiet`, `await_get`, `await_control`, `pump_idle` and `drain`, which pump and never move the clock. The silence thresholds are armed last, with calls only. The next step is a clock step.

| Id | E3-1 item | What fixes the value |
|---|---|---|
| `e3_1_due_effect_without_event_runs_in_its_pump_with_no_budget` | 1 | Two `Silent` events and the kill at the end of `stop_grace` are due at 1000 ms with `pump_events = 1`. `signals_sent` has no `kill` before the pump and has it after the one `pump`: the kill is sent in the due pump (E3-1 item 1 allows the exit and its event to follow later, so the transcript does not assert death). |
| `e3_1_due_step_beyond_budget_is_carried_with_more` | 2 | Two silences due together: the first `pump` posts one and reports `more = true`; the `WakeHandle` is signaled; the second `pump` posts the other. |
| `e3_1_carried_steps_run_first_in_due_order` | 3 | s2 (due 1200) is carried. s3 (threshold 1100, so due earlier, but newer work) is set after the first `pump`. The carried step goes first. A pure due-time order would post s3 first. |
| `e3_1_two_silences_due_together_with_budget_one` | 6 | Both events have the same `since` although 300 ms passed between the pumps. No second `Silent` follows in the idle period. |
| `e3_1_carried_transition_into_a_full_mandatory_queue_is_parked_and_woken_on_room` | 4 | A `Silent` (due 900) uses the budget, the `RouteStalled` of a route that takes no bytes (due at `reader_progress_deadline`, 1000 ms) is carried, finds the state and route queue full, and is parked: `more = false`, no wake (a `Woken` must be spurious: the next `pump` at the same `now` does no work; steward ruling R-43(5)). `poll_events` frees room, the wake is signaled, and the transition is posted. |
| `e3_1_eventless_effect_overtakes_a_carried_or_parked_step` | 5 | The `RouteStalled` is parked on room. The kill at the end of `stop_grace` (due later) is sent in its pump (`signals_sent`), and was not sent before. The transition is posted when room returns. |
| `e3_1_runnable_silent_bypasses_a_transition_parked_on_queue_room` | 5 | The `RouteStalled` (due 1000) is parked. A `Silent` due at 1200 is posted by its pump (`events_posted = 1`), and the transcript reads that `Silent` for s1 from the queue with the new step `poll_find` (it polls and never pumps), while the transition stays parked. |

## What the transcripts do not state

- The order of two steps that are due at the same instant. The transcripts use `expect_set` for them.
- The count of events that a pump posts beyond the budget bound: `events_posted` is stated only where the clause fixes it (one step fits the budget of one).
- The `RouteStalled` needs `reader_progress_deadline` (1000 ms) and a route that takes no bytes (`route_gate`, a control of core-p4a-dp). `stall_close_after` is large, so no close happens.
- The state and route queue is bounded by `mandatory_events` (EV-5(b)); `Completed` has its own reserved slot (EV-5(a)). With `mandatory_events = max_sessions = 2` the queue is made full without a poll, as `conf::tm_6_poll_room_unparks_and_wakes` does: `Create` and `Start` of s2. The setup is checked at the boundary: the payload of s2 runs (`payload_alive`), and the cached state of s2 is still `Starting`, because its `Running` transition is parked on room (EV-5(b)).

## Prior art

`conf::tm_4_silent_fires_once_at_threshold`, `conf::tm_5_deadlines_in_order`, `conf::tm_6_parked_work_does_not_spin`, `conf::tm_6_poll_room_unparks_and_wakes` (silence anchors, the parked-on-room pattern), and `conf::ou_9_resync_drops_unstarted_frames` (the gated route).
