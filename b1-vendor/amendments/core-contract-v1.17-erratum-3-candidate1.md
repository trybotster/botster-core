# Core contract v1.17, erratum 3 (candidate 1): due deadlines and the `pump_events` budget

The base is the Core contract of `frozen/current` (manifest final17), which is unchanged except for the reading below. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the Stage 1 Core lead's P1 reviewer (finding F3).

**Why.** TM-3 says "A deadline that is due at a `pump` is processed in that `pump`", and Core A5-2 repeats it. TM-4 posts `Silent` "by the first `pump` with `now ≥ last_output_at + threshold`". But 9B says "One `pump` posts at most [`pump_events`] events. If work remains, `PumpReport.more = true` and the `WakeHandle` stays signaled". With `pump_events = 1` and two silence deadlines due at the same instant, both rules cannot hold.

| # | Clause | Test |
|---|---|---|
| E3-1 | **A due deadline's effect is never skipped, reordered, or left waiting for a host timer, and `pump_events` still bounds every `pump`.**<br>1. **An effect that posts no event is processed in the `pump` at which it is due**, whatever the budget. Examples: a capture's expiry (ST-6), and the kill at the end of `stop_grace` (LC-5; EV-5(c): the kill is sent). An event that follows later (such as the `SessionState` after the payload exits) is posted when it happens, under the normal rules.<br>2. **An effect that is one atomic step with its event** (EV-5(b) for state and route transitions, such as a stall close; and `Silent`, which TM-4 makes the edge itself) is processed **in that `pump` if the `pump` still has event budget**. Otherwise the step is **carried as a whole** to the next `pump`, and `PumpReport.more = true` with the `WakeHandle` signaled (9B, TM-6). A carried step is never split from its event.<br>3. **Carried steps come first.** The next `pump` processes carried due steps **before** any newer work, in order of due time (TM-5).<br>4. **TM-4 reads with this rule:** `Silent{since}` is posted by the first `pump`, at or after the threshold, that has event budget for it. `since` does not change, and it is still posted exactly once per idle period. `Silent` is class K, so a carried `Silent` loses no value. | `conf::e3_1_due_effect_without_event_runs_in_its_pump_with_no_budget`, `conf::e3_1_due_step_beyond_budget_is_carried_with_more`, `conf::e3_1_carried_steps_run_first_in_due_order`, `conf::e3_1_two_silences_due_together_with_budget_one` |

**Text replaced:** TM-3's last sentence, A5-2's due-deadline item, and TM-4 read with E3-1. Every other clause is unchanged.
