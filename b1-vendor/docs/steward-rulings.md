# Steward rulings

The contract steward's rulings on what the frozen contracts (`frozen/current`; the joint freeze is `frozen/final6`) mean. A ruling never changes a contract. When a ruling would need a contract change, it goes through the amendment path instead. Each ruling cites the clauses it reads. Newest first.

## R-92. `a5_2_deferred_completion_keeps_slot_until_polled` sets `pending_ops: 1`, not 2: a transcript defect, and EV-5(a) is unchanged (Stage 1 Core lead, P6's strict evidence on `c0c86a58`, 2026-10-11)

- **The clause.** EV-5(a): "an operation's pending slot is reserved at `begin` and released when the host **polls** its `Completed`".
- **The defect.** Steps 1 and 3 (`pump_until`) poll the `Completed` events of the Create and the Start, so both slots are free. At step 10 only the Stop `$a` holds a slot (1 of 2), so Core correctly admits the Create, and FakeCore frees the slots at the same poll too. The transcript is `fake: structural`, so the fake never ran it, and the defect stayed hidden.
- **The fix.** The setup becomes `{"limits": {"pending_ops": 1}}`. Then step 10's `PendingLimit` proves that the Stop's posted but unpolled `Completed` keeps the only slot, and step 12 proves that the poll at step 11 frees it. Steps 0-3 run one op at a time, so they are unchanged. The id leaves `core-pending.txt` when Core passes it on both tiers.
- **Owner.** P6 (p6-testkit), one contracts transcript PR, reviewed by 016c.

## R-91. IN-6's control queue is Core's host-side outbound control stream, with an internal bound and no new `CoreLimits` field; only `cancel` refuses `Full`, before its sent or not-sent branch (Stage 1 Core lead, PC's design `shared/core-stage1/handoffs/pc-control-queue-design.md`, 2026-10-11)

- **The clauses.** IN-6: "`Refused(Full)` (the control queue has no room; nothing changed). `Refused(Full)` is stateless, and a **real wake** follows: **Core posts** `SessionWritable{id}` ... when the control queue has room again." The event table: "`SessionWritable{id}`: The session's control queue has room again after a refusal (IN-6)." AM-4: `cancel`'s `Refused(Full)` is the one sync refusal with its own wake. SV-3: the wake comes when the room is at least one control message.
- **Q1 (a), where the queue is: host-side.** IN-6 makes `cancel` sync and says that Core posts the wake. A sync call can decide only from the host's own state, so the control queue is Core's per-session outbound control stream to the worker (the link's unsent bytes for that session). Core posts `SessionWritable` when a drain leaves room for one control message after a refusal. The link message `Observation::Writable` is Core-internal (R-65). Core removes it if nothing uses it.
- **Q1 (c), the bound: no new `CoreLimits` field.** A new field changes 9B, so it would need an amendment, and no clause asks the host to configure this bound. Core uses an internal bound, at least a few control messages, and states it in `DESIGN.md`. *(Clarified the same day, PC via the Core lead: "a few control messages" is **headroom above** the traffic that admission already bounds, not the whole bound. `out` also holds admitted input payloads, so a bound of only a few control messages would refuse a cancel behind an admitted write, for example after R-74's 40960-byte fill. PC's form is accepted: bound = `input_retained_bytes` + (`pending_ops` + `input_ops_per_session`) × the largest non-input host message frame + 4 cancel frames, with the room check reading min(bound − unsent, `link_room`). So in production a cancel has room unless the link stops draining.)* The tests reach a full queue through the injected `link_room` edge, not through the size of the bound.
- **Q1 (b), what the bound refuses: only `cancel`.** IN-6 gives `Refused(Full)` only to `cancel`. The other host-to-worker messages keep their own admission (AM-1 to AM-4) and are not refused by this bound. **Condition:** the stream stays bounded overall, because each of those messages is bounded by its own admission limit. Core's `DESIGN.md` states that sum, so `out` cannot grow without a bound.
- **Q2, the order: allowed.** IN-6 conditions `Refused(Full)` only on the queue's room, not on whether the cancel would send a message. So the room check may come before the sent or not-sent branch. It comes after `UnknownOp`, `TooLate`, `NotCancellable` and `AlreadyAdmitted`, as FakeCore orders them. A refused cancel changes nothing, and the wake follows when the queue drains.
- **The test control.** `control_queue {session, on}` reads the injected `link_room` edge on both tiers (`SimEdges`; `EdgeTap` on the real tier). It is never a branch in production code. A driver unit test with a real bounded stream (WouldBlock, then a drain) proves Core's own count and the wake, because the edge alone does not.
- **Owner.** PC, in one HIGH Core PR, reviewed by Sol (017a) and 016c, with the prior-art comparison against old Core `72b2e335`. Then the two IN-6 ids leave `core-pending.txt`. `ev_9_every_session_event_carries_instance_id` moves to `core-real-pending.txt` while its R-57 exception stands. The PR body names `control_queue` as the control it serves (the .43 tag rule).

## R-90. The route stall deadlines are Core-library deadlines on the injected clock: the worker owns the route state machine, not the clock (Stage 1 Core lead, P3's facts on `ou_3_progress_is_bytes_accepted_by_the_kernel` s21, Core `origin/v1` `f68f26a7`, 2026-10-11)

- **The clauses.**
  - TM-3: "`next_deadline()` ... the earliest deadline Core owns (stop grace, reader progress, stall close, silence, capture expiry) ... **Only the Core library's deadlines are in this query and on the injected clock.** The guardian's and a starting worker's own deadlines (startup, orphan grace) run on **their** monotonic clocks in their own processes and are not injectable."
  - TM-1: "**Every threshold and deadline** (silence, stop grace, reader progress, startup) uses the **monotonic** clock" that the host passes.
  - OU-2: the route state machine is "owned by the worker", and "the worker tells the host (`RouteStalled{route}`)".
  - EV-8 already splits the work this way: "The worker arbitrates; the host supplies only the deadline." *(Corrected the same day, ST's design note: EV-8 shares only the split of roles. EV-8(b)'s query deadline "runs in the worker, on its own monotonic clock", so EV-8 is not a precedent for the clock. The precedent for the clock is `stop_grace`: the host arms it, and at the due pump it sends the kill that the worker carries out.)*
- **Ruling.** The Core library arms `reader_progress_deadline` and `stall_close_after` on the host's injected monotonic clock, and both are in `next_deadline()`. The worker keeps the state machine and sends `RouteStalled`, `RouteResumed` and `RouteClosed{StallTimeout}`. It learns from Core that a deadline is due. How the due deadline crosses the link is Core's design choice (for example, a host message, as in EV-8). No amendment is needed, because TM-3 already says this.
- **R-46's line "Worker-side timers run on the worker's own real clock"** covers only the deadlines that TM-3 excludes (startup, orphan grace). It does not cover the stall deadlines.
- **A Core design gap.** `crates/botster-worker-core/DESIGN.md` 167-169 and 186-191 put the stall timer on the worker. That conflicts with TM-3, and Core fixes the design before it builds the stall clock (`worker/route.rs` 15: "this PR has no stall clock"). The 19 stall ids stay pending until then.
- **The transcripts.** The `advance_clock` steps of `ou_3_progress_is_bytes_accepted_by_the_kernel` and of the other stall ids are correct as written. **With a host-armed deadline, R-88 applies:** Core can restart the stall deadline only after it has consumed the worker's report of progress. So s17 `route_accept` → s19 `advance_clock` needs a fence that proves Core consumed that report; `await_quiet` does not prove it (R-43 (8)). The fence is chosen when Core's design names the progress report, and the R-88 lint gets the pair then.
- **Owners.** The Core design fix: the Core lead (P4a, the stall ids). The lint pair: P3.
- **Follow-up: ST's design questions** (`shared/core-stage1/handoffs/st-stall-clock-design.md`, sha256 `ac4ac6e239efd9f9b9c8654252b95cf5e12b42e6382033c4e019991538c05353`, via the Core lead, the same day). The design is in R-90's form: the host arms both deadlines from the pump that consumes the worker's progress report, and at the due step it tells the worker. The worker makes the transition only if no progress came after the host's view (OU-3: "a progressing reader never loses output").
  1. **Q1, E3-1 item 2. ST's reading is correct, with four conditions.** The host's due step is the step of E3-1 items 2-5: the budget check, the room check (parked when there is no room; `parked_work` reports it), and the reservation of one mandatory slot, all in the due `pump`. The worker's `RouteStalled` (or `RouteClosed{StallTimeout}`) is that step's event, posted in the reserved slot when Core consumes it. The step and its event are not split as the host sees them, because:
     1. **No host-visible route state changes before the event posts.** `route_reports`, the route's state in any read, and every other host-visible fact change at the post, not at the due step.
     2. **The post counts against the event budget of the `pump` that makes it.** A `pump` with no budget left carries the post, with `more: true`, as for any other report.
     3. **When the worker declines** (newer progress, an empty queue, or a close came first), no transition and no event happen, and Core releases the reservation when it consumes the worker's newer report.
     4. **Order.** TM-5 and E3-1 item 5 order the host's due steps. Stall events of different sessions follow the order in which Core consumes their links, as other reports of different workers do. No transcript may assert a cross-session order of stall events.
     So the three `e3_1` transcripts stay as they are.
  2. **Q2, the fence for `route_accept` → `advance_clock`: approved, with the definition in contract terms.** `route_reports {session, route}` gains `progressed`: **true when Core has consumed the worker's report of transport progress (OU-3a: bytes the transport accepted) that happened after the route's latest `route_accept`.** It is read-only and changes no state. Its scope: the transport accepted no bytes between the previous fence and that `route_accept` (the `route_gate` or `route_fill` setup). `ou_3_progress_is_bytes_accepted_by_the_kernel` gets `await_control {route_reports, session: s1, route: $r1} expect {progressed: true}` after s17, in place of s18's `await_quiet`. **The R-88 lint pairs:**
     - cause `route_accept` → read `advance_clock`, with the fence `route_reports {progressed: true}`;
     - cause `pty_output` on a route whose gate is on → read `advance_clock`, with the fence of an R-57 title sentinel and `await_title`. This fence holds only while the worker puts the progress report on the link before the output report of the same output (ST's section 2). Core's `DESIGN.md` states that order as a rule, and Core keeps a test of it.
  3. **Q3, adoption: correct.** TM-3 makes both stall deadlines the Core library's, not the worker's. So while no host is connected, no stall deadline runs, and a route does not stall. After `Adopted`, the new host arms from the `pump` that consumes the worker's progress report, as A18 does for silence. The consequence is accepted: while no host is connected, a reader that does not progress still holds the session's PTY output through the budget (OU-3d), for at most the host's absence plus `reader_progress_deadline`.
- **Follow-up: `ou_7_stalled_route_does_not_hold_exit` has the wrong precondition** (ST's question, via the Core lead, the same day). OU-7: "`Exited` is posted once the last output frame is **queued** for every route that is `Open` at exit (queued, not written)", and only "a slow route that is still progressing can delay `Exited`". After `route_fill` (R-47), at least one frame waits, but the worker's route queue still has room for the 4-byte tail. So the tail is queued at once, `Exited` posts before any stall, and step 16 fails. **The transcript is wrong, and OU-7 and OU-3d are unchanged.** The test must first make the route hold the exit: the program writes enough output that the route's queue is full and source backpressure keeps the tail in the PTY (OU-3d). The fence is `pty_output_unread {s1}` with `bytes` above 0 after `await_quiet`, before the clock step. Then `RouteStalled` before `Exited` is the claim, and the transcript asserts that order with a wait that does not drop the first event (`pump_collect` in order, or `expect_quiet_until` `Exited` up to `RouteStalled`). Owner: ST, in its contracts PR.
- **Owner of the design:** ST, in one HIGH Core PR with both `DESIGN.md` files, reviewed by Sol (017a) and 016c. The `progressed` field and the transcript fences go in ST's contracts PR, HIGH, reviewed by 016c and 017f.

## R-89. A step deadline bounds every driver wait: a pump that reports `more`, or a client that reports progress, never extends it (Stage 1 Core lead, RC's #66 mutant, 2026-10-11)

- **Finding (checked at `5305007`, `driver.rs` 639-680).** `next_event` returns `Poll::Pending` whenever the pump reports `more` or posted events, and only `idle_wait` checks the step deadline. With a deadline, `poll_limit` is `usize::MAX`. So `pump_until` and the expect-all and collect loops never end on the real tier while `more` stays true. The `progress -> true` mutant made a 10 s step spin for 1099 s. A busy or faulty real Core does the same, even without R-86.
- **Fix (RC's).** `next_event` checks `deadline.expired()` after it has taken any queued event, and before it returns `Pending` for `more` or posted events. It then gives `Poll::TimedOut`, so the step ends INCONCLUSIVE `step_timeout`. A queued event is still returned first. No timeout value changes, and the fake tier's 10 000-poll bound is unchanged.
- **Self-test.** A Core or client that always reports `more`, under a short deadline, gives INCONCLUSIVE `step_timeout` in `pump_until` and in the expect-all and collect loop.
- **Owner.** RC, in #66 (the R-86 PR, which already changes `driver.rs` and adds the second source), before #66's gate.

## R-88. A step whose result depends on an asynchronous worker report needs a fence that proves Core consumed the report: one sweep, then one lint (Stage 1 Core lead, 2026-10-11; the sweep: P3, `shared/core-stage1/handoffs/r88-sweep.md`, sha256 `f67942eb98243a37a5ff6f5ac76bce12eb1141b505c6af19cceb036d6b7f8fd5`)

- **The pattern.** Three ids (`ev_5_poll_wakes_pending_work`, `tm_6_poll_room_unparks_and_wakes`, `ev_5_route_closed_retires_route_keyed_events`) had a cause (input, a payload exit, a launch), then a read of state that a worker report sets, with only `await_quiet` or nothing between them. `await_quiet` cannot see an unwritten report (R-43 (8)). R-56 (PTY reads) and R-80 (model reads) cover other shapes, not this one.
- **Step 1, the sweep (done).** It lists every Core transcript step that reads report-set state, the cause before it, and the steps between them: 316 reads, 32 racy reads in 18 ids, 105 fenced, 179 safe. **Each racy id is fenced in its owner's PR before it is un-pended, or it stays pended FLAKY with its sweep entry. An unfenced read that passes is not evidence.** *(Corrected the same day, the Core lead: a live id cannot be re-pended, because `core-pending.txt` and `core-real-pending.txt` are shrink-only (xtask `lists.rs`; only a new ledger id may enter them). So a live racy id stays live until its fence lands, and its sweep row is the record. If it fails a gate before then, the 23al re-gate path applies, with that row as the diagnosis. No re-pend path is added.)*
- **(A) Parked work on a full queue (R-87 shape).** The fence is `await_control {parked_work, session: S} expect {parked: true}` after the step that the sweep names.
  - `tm_6_poll_room_unparks_and_wakes` and `ev_5_poll_wakes_pending_work` are fenced in #68.
  - `ev_5_full_queue_defers_transition_not_output` s17/s20, `ev_5_stop_kills_while_queue_full` s10 and `tm_6_more_means_runnable_not_blocked` s9-s11 are live on both tiers. They go in one P5 PR after #68, in the .42 batch, with 32-seed testkit and real-tier evidence.
  - `tm_6_parked_work_does_not_spin` (core-pending) gets the same fence after s9, before its un-pend.
  - **The three `e3_1` ids** (`…carried_transition_into_a_full_mandatory_queue…`, `…eventless_effect_overtakes…`, `…runnable_silent_bypasses…`) get two fences each:
    1. an R-57 title sentinel appended to step 17's `pty_output` bytes, and `await_title s1` before step 18 (no poll and no clock step, as the e3 setup requires);
    2. `parked_work` after the last pump that asserts `more: false` before the poll.
    **The parked session is not assumed.** The owner shows by 32 per-seed runs whose work is parked there (s1's `RouteStalled`, s2's `Running`, or both), and fences each such session. Their three lines in `core-fence-exceptions.txt` are removed (shrink-only).
    **The result (P5's probe, `shared/core-stage1/evidence/p5-r87/e31-whose-parked-probe.log`, the same day):**
    - `…carried_transition…`: `parked_work` holds for s2 in 32 of 32 seeds and is INCONCLUSIVE for s1 in 32 of 32, so the fence is on s2 only.
    - `…eventless_effect…` (Core lacks `signals_sent`) and `…runnable_silent…` (Core fails at s30 in 32 of 32 seeds, before the poll) cannot show whose work is parked today. They get only the title fences now. Their `parked_work` fences are added, and proved per seed, in the PR that un-pends each id, because an unproven fence can be wrong.
    - `runnable_silent`'s s30 failure (`more: true` where the transcript expects `more: false`) is a finding on its own. Its owner triages it as a Core defect or a transcript defect.
- **(B) An input write → its input-written report.**
  - **`am_4_capacity_returns_on_poll` s24.** No listed fence fits. `await_pty` proves only the PTY bytes. Three `pump` steps are a count, not a fence. A `pump_until Completed{$w2}` polls the event that s24 asserts. **A new read-only control: `op_posted {for_op}` → `{posted}`. It is true when Core has posted the operation's terminal event to the event queue, whether or not a poll has taken it. It changes no state.** `await_control` pumps and never polls, so the event stays queued. The control is named for the precondition (the completion is posted), not for a mechanism. Its self-test: false before the worker's report; true after it; and the event is still the next poll. Then am_4 gets `await_control {"control": {"op": "op_posted", "for_op": "$w2"}, "expect": {"posted": true}}` after s10, and the same for `$w1` after s15 and `$w3` after s20 if s24's claim needs their order. am_4 is live on both tiers, so this is HIGH and in .42. Until it lands, am_4 stays live (see the correction under Step 1). *(Corrected the same day: the argument is `for_op`, not `op`, because `op` names the control inside `await_control` (`driver.rs` ~777-795). `for_op` is the name that `scheduler_hold`, `scheduler_release` and `scheduler_ready` already use for an OpId.)*
  - **`tm_6_begin_and_attach_signal_wake` s21/s22.** The test needs the write to stay pending, so it takes R-74's held-write form (`pty_blocked` with `kernel_bytes: 1`, a write of F bytes, `await_control pty_queued`). It is not a consumption fence.
- **(C) A lost or ended worker → its link EOF.**
  - `a2_3_close_first_reason_wins_with_new_reasons` s19: `await_get {id: s1, expect: {state: {Lost: WorkerGone}}}` after s17.
  - `a2_3_every_route_close_reason_maps_to_one_wire_reason` s169: **not ruled.** The sweep did not confirm that Core posts `Exited` at the startup deadline before the link EOF. The owner confirms that order in Core with file and line, and with per-seed runs, before I rule a control (a closed-link form like `service_lane_open` is the candidate). The id stays core-pending meanwhile.
- **(D) An output → the worker's query parse, before `queue_overflow`.** The four `ev_8` ids (`…client_reply_after_a_resync…` s53, `…refused_fallback_admission…` s53, `…resync_never_lets_a_client_answer…` s46, `…unstarted_query_frame…` s24) get `pump_until Activity{s1, Output}` on the named handle, after the `pty_output` and before `queue_overflow`, under R-77's two checks: no earlier unpolled event meets it, and no later step asserts an event that it consumes. Where the second check fails, the fence is an appended R-57 title sentinel.
- **(E) A `scheduler_delay` or `advance_clock` → a ready or exit report** (`a5_2_each_section_11_variation_has_a_source`).
  - s22/s27 are racy, and not testkit-only: `core-testkit+perturb` ids also run on the real tier. **`scheduler_delay`'s pump count starts when the edge holds the report**, on both tiers. A count from the `begin` proves nothing on the real tier. The fence is a control that shows the edge holds the report before the counted pumps. The owner confirms with Core file and line whether `held_reports` reports that at a delayed edge, or proposes a control, before the transcript edit.
  - **s22/s27, the fence (P6's Core check at `v1` `f6025aed`, via the Core lead, the same day).** Today nothing holds a ready or exit report: `scheduler_delay`, `held_reports` and `hold_worker_report` are implemented on neither tier, and no edge calls `bound()` on `ChoicePoint::PumpsToRunning`/`PumpsToExited` (`botster-core-edges/src/scheduler.rs` 19, 21). So:
    - **`scheduler_delay` becomes a process-edge hold** (the testkit edge; the tap on the real tier) of the session's next ready or exit report. Its pump count starts at the pump in which the edge holds the report.
    - **`held_reports {session}` → `{count}` counts these delay holds too.** Its row already says "the reports that the edge holds". One observer is enough for that fact, and the row lives in `docs/core-testkit-controls.md`, not in a frozen clause, so no amendment is needed. Its row and `scheduler_delay`'s row are updated in the same PR.
    - The transcript adds `await_control held_reports {session} expect {count: 1}` after the `begin` of `Start` or `Stop` and before the counted pumps.
    - **Condition:** no transcript may combine `hold_worker_report` and `scheduler_delay` on one session, because one count would then mix the two holds. Today no transcript uses `held_reports` together with `scheduler_delay`. The R-88 lint, or the control itself, refuses the combination.
    - Owner P6, HIGH (a control change on both tiers), reviewed by 016c and 017f. The ids stay core-pending until both tiers implement it.
  - s94/s113: the `pump_until Stopping` at s86/s105 is a false fence, because Core sets `Stopping` at `begin`. Use `await_get Exited Killed`, unless a clause requires the one-pump order. In that case the owner names the clause and uses a held-report fence.
    *(Applied the same day, 017f's F1 on #75, and then **withdrawn the same day.** I first said that A5-2's `scheduler_deadline` (set with `before: true` at s90) requires `Exited` before the Create's `Completed` in one pump. That was wrong. E3-1 item 1 makes the kill at the end of `stop_grace` "an effect that posts no event", and the `SessionState` after the payload exits "is posted when it happens, under the normal rules". `scheduler_deadline` orders "its event": the event that the due deadline itself posts. `stop_grace` posts none, so no clause orders that `Exited` against another event, and no Core change may make the held `Exited` count as the deadline's event.)*
    **The ruling for s94/s113 and the `scheduler_deadline` variations:**
    - The `scheduler_deadline` variations of `a5_2` use a deadline that posts its own event in the due pump: silence (`Silent`; TM-4, E3-1 item 2). With `scheduler_ready` on another op and `scheduler_deadline {before: true}` (and `false` in the second variation), one pump posts both, and the transcript asserts the order of `Silent` and that `Completed`.
    - The `stop_grace` steps keep the first branch: `await_get Exited Killed`, with no one-pump claim and no order assertion against another op.
    - Owner P6, in #75. Reviewed by 017f and 016c as a delta.
- **Reads outside the scanner.** `a5_1_edges_scripted` s44 and `a5_2` s79 are `pump` steps with an `events_posted: 0` expectation. They get `route_reports` fences per R-66, and s79 also an output fence on s12.
  - **s79's output fence (P6's design note `evidence/p6-r88-e/design-note.md` section 4, the same day).** s12's program prints `aaaa` and then `bbbb`, so it can give two output reports, and an `Activity` wait fences only a one-byte output (R-80 follow-up). The fence is an R-57 title sentinel that the program prints right after `bbbb`, with `await_title` before s76. The expected `bytes_hex` of s74/s75 grows by exactly the sentinel's bytes, verbatim. Rejected: a new `output_reports` control, because no new control is added for one id when the ruled fence works.
  - **s74/s75, two sends on r12a (017f, found by #73's lint, the same day).** s74's `Activity` does not cover s75: each send is its own cause with its own report (R-88), and `activity_before_close` stays true after the first. s79's title sentinel (the program's answer to the second send) also fences s75's report **if** Core's worker writes the client-activity report on the link before it forwards that input to the PTY, because the link keeps order. P6 confirms that order with Core file and line, and Core's worker `DESIGN.md` states it as a rule with a test (as for ST's progress order, R-90). Otherwise s75 gets its own fence, or one send replaces the two if the variation does not need both. Until then, #73 keeps a debt line. Owner P6.
- **Found by the lint (P3's precision pass, the `r88-sweep.md` addendum, sha256 `f95da33dde139d2095b44d69ba0f991095a59f1cf24dffb95a5f8cee651b2714`, and the cancel pair, the same day).**
  - `ev_8_core_has_no_other_query_timer` s15 (`pump {more: false, events_posted: 0}`, core-pending) is racy. Step 10's query output causes `Activity{s1, Output}`. Step 11 is `await_quiet`, and step 12 waits only for the route's `terminal_query` frame, which the worker writes to the client itself. So a late `Activity` can land in s15's pump. The fence is `pump_until Activity{s1, Output}` after s10, under R-77's two checks. Owner P3, with (D).
  - `in_6_refused_full_wakes_on_room` s9 (core-pending) is racy. After s8's `SessionWritable`, the write can reach the worker and complete, so s9's `cancel` reads `TooLate` and not `Admitted`. The fence is R-74's held-write form (`pty_blocked` with `kernel_bytes`, a write of F bytes, `await_control pty_queued`), placed so that the write cannot complete before s9. Owner PC, with `tm_6_begin`.
  - `in_9_modes_read_at_write_start` (live; PC's record `shared/core-stage1/handoffs/flaky-in_9-modes-read-at-write-start-8ac717c7.md`, with a controlled reproduction on Core `v1` `f6025aed`) is racy on the real tier. `pty_blocked` stops only the probe's reads, and the kernel PTY queue still takes the small focus write. So step 4's write is not held, `Completed{$a}` can arrive before `ModesChanged`, step 6's `pump_until` drops it, and step 8 times out. The fence is R-74's held-write form (`pty_blocked` with `kernel_bytes` to fill the queue, then `await_control pty_queued`), so the write stays pending until step 7. Owner PC, in its R-88 held-write PR with `tm_6_begin` and `in_6_refused_full`, with per-seed evidence on both tiers. Until it lands, the id is live and racy, and its record is the diagnosis.
  - `e3_1_due_effect_without_event_runs_in_its_pump_with_no_budget` s35 is safe: sK ignores SIGTERM, s31-s33 show no kill before s34, and s37 allows `events_posted` below 2. It is listed with that reason.
- **Step 2, the lint, in R-80's form.** Its lists come from the pairs above:
  - **causes:** `begin` of `Start`, `Stop` and `WriteInput`; `lose_worker`; `pty_output`; `scheduler_delay`; `advance_clock` past a deadline that ends a payload; `route_send`;
  - **report-derived reads:** `get`, `list` and `status`; `route_reports`; `parked_work`; `wait_wake`; `poll_*`; `expect_quiet_until`; `forbid_events`; **and a `pump` with an `events_posted` or `more` expectation**;
  - **fences:** `await_get`, `route_reports`, `parked_work`, `op_posted`, `pty_queued` (the held-write form), and an `Activity` or other report-made event that ends a wait and is distinct per R-61(a)(2).
  It ships with a self-test that is red on today's am_4 s24. Every match is fenced, or listed with a one-line reason in the shrink-only exception file. P3 may add a pair that the sweep did not name; the reviewers check each addition against a transcript.
  - **The pairs past the sweep (P3's question, ruled the same day; Core `origin/v1` `f68f26a7`):**
    1. **More causes:**
       - `Detach` (`HostMsg::Detach`, answered by `RouteClosed`; `botster-core-link/src/msg.rs` 84);
       - `Remove` (answered by `RemoveResult`; `msg.rs` 73-74, 224-226);
       - **every op that the host forwards as `HostMsg::Op`**, which the worker answers with `Done` (`msg.rs` 70, 190). This covers WriteInput, Signal, Resize, ReadScreen, the setters and so on.
    2. **More fences:**
       - `Completed{op}` and `op_posted {for_op}`;
       - for `Detach`, a `RouteClosed{route}` that ends a wait;
       - for `Remove`, an `await_get` with `expect_error UnknownSession`;
       - a fatal Signal also causes the exit report, so its exit read needs `await_get`, as Stop's does.
    3. **One more read:** `call cancel`, whose result depends on whether Core consumed the op's `Done`. `pty_queued` and `scheduler_hold` count as fences only where they keep the op pending.
    4. **Not causes:**
       - a `begin` with `expect_error`, which is refused synchronously;
       - a step's own barrier for its own read.
    - **Why the whole op class, and not only WriteInput:** R-88 names a race class. A list of only the causes that the sweep saw lets the next transcript of that class through unchecked. Each extra match is fenced, or listed with a reason (R-80's model).
    - The PR states its match count, and lists each precision rule with one example match that it removes.
    - The self-test also covers one forwarded op other than WriteInput.
- **Out of R-88's scope, ruled here.**
  - `in_7_lost_worker_ends_pending`: its real-tier INCONCLUSIVE is the R-74 held-write wait, not a consumed report. It stays with PC in Core #289.
  - **An R-80 follow-up.** `uncarriable_sequence` is an output on S, and `attach_route` (the baseline) and `queue_overflow` (the resync) are model reads. R-80's lint lists none of them, so it missed `a8_2_baseline_inside_an_uncarriable_sequence…` s8, `a8_2_capture_inside_an_uncarriable_sequence…` s12 and `a8_2_resync_inside_an_uncarriable_sequence…` s14. An appended title sentinel cannot fence them: the bytes after an unterminated OSC or DCS string are part of that string. The owners propose a fence. If none works, the match goes in the exception file with that reason. The R-80 lint's lists grow in the same PR.
  - `pump_until` and `pump_collect` drop the events that do not match (`dp_6_close_falls_back_zero_routes_keeps_size` s24, `lc_12` s18, `ou_5` s25). This is an assertion gap, not a race. Each owner adds `forbid_events` for the dropped event before the wait. Low priority.
  - ~~`ev_8_full_query_capacity…` s26 is weak: nothing shows that the worker read the `43` bytes. Add a `pty_output_unread {s1}` read. Low priority.~~ **Withdrawn the same day (016c on #76).** EV-8(g) allows source backpressure as in OU-3d, so a correct worker may read C and hold it behind the blocked parser, or leave C unread in the PTY. s26 asserts that no output of C reaches r2 while the parser is blocked, and that holds in both cases, so the step is not vacuous. A `pty_output_unread` assertion with `bytes: 0` would fail a correct Core, and no other count is required. No read is added.
  - `ou_3_progress_is_bytes_accepted_by_the_kernel` s21: a clock question, not R-88 (no Core-consumed report). It is ruled separately.
  - The service-lane reads: fenced or safe, as the sweep says.
- **Owners (assigned by the Core lead).**
  - P5: (A) and `op_posted`.
  - P3: the lint; the R-80 list growth; after the lint, (D) and the low-priority items.
  - PC: (C), `tm_6_begin`'s held-write form, and `in_7`.
  - P6: (E) and `a5_1`.
  - The lint and each new control are HIGH, reviewed by 016c and 017f. Transcript fences are reviewed by 016c.

## R-87. `ev_5_poll_wakes_pending_work` needs a fence that proves Core holds the worker's exit report before the poll: a new read-only `link_held` control (Stage 1 Core lead, P5's diagnosis `shared/core-stage1/handoffs/flaky-ev-5-poll-wakes-pending-work-017f0127.md`, 2026-10-11)

- **Finding.** The work that step 12's `poll_events` frees is the worker's `Exited` frame, held on its link because the mandatory queue is full (EV-5b). In every traced run, Core signals before the poll returns (TM-6). But nothing before step 12 proves that the frame is held. Step 8's `payload_alive: false` shows only that the payload is gone. Step 9's `await_quiet` cannot see a report that is not yet written (R-43 (8)). Step 11's `pump` reports the same either way. If the frame arrives after step 12, no parked work exists, and the 0 ms wait at step 13 correctly reads `TimedOut`. So this is a transcript fence gap, not a Core defect.
- **Ruling.** A read-only control `link_held {session}` → `{held}`: whether Core holds a frame from S's worker link that it has read but not yet accepted (a parked transition, EV-5b). After step 8: `await_control {link_held, session: s1} expect {held: true}`.
  - The wait pumps and never polls events, as `await_get` does (R-74), so the full queue stays full and the frame stays parked. The pump is also its wake source, because the frame's arrival is I/O readiness on the link.
  - It reads Core's link state only as a precondition fence. The claim under test, the poll's wake (TM-6), is still asserted by step 13's 0 ms `wait_wake` → `Woken`, so TM-6's "before it returns" stays strict.
- **Rejected:** a `Woken` wait with a timeout. A late frame's I/O wake also meets it, so the step would no longer prove TM-6.
- **The control is defined on both harnesses.** It may fence other transcripts with an unfenced asynchronous worker report.
- **Owner.** P5: the control row in `core-testkit-controls.md` and the transcript fence, in one contracts PR (HIGH, a new control, reviewed by 016c and 017f); then the Core impls on both tiers. The id leaves the FLAKY list only after the Linux gate passes.
- **Correction (the Core lead and P5, the same day): the control is `parked_work`, not `link_held`. The `link_held` form above is withdrawn.**
  - **Evidence.** With the `link_held` fence, the Core testkit passed 22 of 32 seeds. Seeds 4, 8, 9, 10, 12, 13, 15, 17, 25 and 28 ended INCONCLUSIVE at the fence (`shared/core-stage1/evidence/p5-r87/fenced-ev5-poll-testkit-per-seed.log`). Core's own trace of the old transcript at seed 4, which passes, holds no link frame at any pump (`held=[]`). There the work that the poll frees is an engine transition that waits for mandatory-queue room: `runnable` is false before the poll and true after it, and `sync_wake` signals (`trace-old-seed4-testkit.log`). So TM-6's parked work has two forms: a held link frame (EV-5b), and an engine transition that waits for queue room. `link_held` proved only the first. The contracts gate passed only because the transcript is `structural` on the fake.
  - **The control:** `parked_work {session}` → `{parked}`, read-only. It is true when Core holds work for S that waits only for mandatory-queue room: a link frame read but not accepted, or an engine transition parked on queue room. It is false for work that is runnable now, or that waits for anything else. It is named for TM-6's precondition: "a poll that frees parked work wakes before it returns" needs parked work to exist, in either form.
  - The fence, the pump-and-never-poll rule, the precondition-only role and step 13's strict `Woken` are unchanged.
  - **Evidence for the new form:** per-seed testkit runs over the same 32 seeds with no INCONCLUSIVE, and a real-tier run, before the id leaves FLAKY.
- **Addition (017f's re-check, verdict `32fa8ff3`, the same day): `tm_6_poll_room_unparks_and_wakes` has the same gap and gets the same fence.** The worker sends `Launched`, which gives `Running`, after the payload starts. Core `crates/botster-worker-core/DESIGN.md` 23: `Launch` → `SpawnPayload`, then `Launched`. `crates/botster-core-host/DESIGN.md` 198: `Launched` gives `Running`. Both were checked on Core `origin/v1`. So `payload_alive {s2}: true` does not prove that the `Running` work is parked when the poll at index 12 runs. The id gets `await_control {parked_work, session: s2} expect {parked: true}` after index 8, in #68, with the same 32-seed testkit evidence and a real-tier run. The three `e3_1` ids are not exempt: R-88 (A) fences them.

## R-86. The conformance client is an independent actor: the driver moves each live route's waiting client bytes during every wait (Stage 1 Core lead, RC's R-81 draft form, 2026-10-11)

- **Finding.** R-81's form sends more than the allowance plus the stream capacity, so client bytes always wait in the client end. On the testkit, `TestkitRoute` flushes those bytes and runs the workers' ready work (TH-3) only inside a route write, read or `input_blocked`. On the real tier, the client end flushes only on a write or `input_blocked`. So after `pty_blocked off`, `await_pty` stalls with 0 PTY bytes: 128 KiB flows, and 256 KiB never completes. Steps up to 43 of RC's draft pass (`blocked: true`, and nothing reaches the PTY while blocked).
- **Ruling.** `RouteClient::progress()` moves the client's waiting bytes into the stream as room appears. On the testkit it also runs the workers' ready route work. The driver calls it for every live route in each wait loop (`await_pty`, `pump_until`, `await_quiet`, `pump_collect`, `expect_eventually`, `await_*`). This models a real client, whose `write()` blocks in its own thread and resumes when room comes. It is written once for all transcripts.
- **Rejected:** a step that drives the route, because every transcript would then have to know a harness limit.
- **Limits on `progress()`:**
  1. It moves only bytes that the transcript already sent, and only as far as the stream takes them. So a full queue stays full, and `input_blocked` keeps its meaning.
  2. It reads no Core event and polls nothing, so R-61's fence rules are unchanged. It reads no route frames for the transcript; `expect` steps still do that.
  3. It changes no step's result except by delivering bytes that a real client would have delivered.
- **Evidence.** The A5-4 table on both Core tiers, with no PASS→non-PASS across the corpus, because this changes every wait loop. `dp_5_full_input_queue…` passes on the testkit with R-81's form. A driver self-test is red without `progress()`: a send larger than the stream, then `await_pty` for the full count.
- **Owner.** RC: the contracts driver, the `RouteClient` trait and R-81's transcript change, in one contracts PR (HIGH, reviewed by 016c and 017f), and Core's two impls. `progress()` is a required trait method with no default, because a no-op default would compile and stall in silence. So the Core impls land in the Core pin PR for the contracts tag that carries it.

## R-85. R-61's input rule covers computed bytes: no input-log assertion compares a `0d` byte, whatever the source of its expected value, and the driver enforces this at run time (Stage 1 Core lead, RC's oracle PR Core #288, 2026-10-11)

- **Finding.** `in_9_application_cursor_and_keypad` passes on the testkit with R-84's fresh-terminal base, but fails on the real tier. `kp_enter` encodes to `0d`, and the oracle and Core agree. The real PTY's line discipline turns it into `0a` (R-48 keeps `ICRNL`). R-61's input rule says "an exact input-log assertion (`pty_input` and `route_expect_pty` literals) contains no `0d`". It names literals, and the lint checks only literals.
- **The rule.** The purpose is the same for every source: a `0d` in the expected bytes of an input-log assertion makes the assertion tier-dependent. So the rule covers literals, `let`-bound values, `oracle_encode` bindings and any other computed value.
- **The check is in the driver, not a key list in the lint.** Which keys encode to `0d` in which modes is encoder semantics (R-7). A lint list such as "Enter, kp_enter, Ctrl+M" would be hand-written terminal knowledge, and it would miss new cases. Instead, the driver resolves the expected bytes of each input-log assertion (`pty_input` equality or prefix, `route_expect_pty` hex and relation parts, and `hex_prefix` or `hex_ends_with` checks over an input log). It fails the step with a typed transcript-defect error when those bytes contain `0d`. It does this on every tier, the testkit included, so the defect shows on the first testkit run. The static lint keeps its literal check.
- **This transcript.** `kp_enter` is not read through the input log. It is asserted another way, for example by its write's `pty_bytes_written` equal to the `oracle_encode` length, or by a read split that excludes that key. Every other key stays as it is.
- **Owner.** P3: the driver check, with a self-test that is red on today's `in_9_application_cursor_and_keypad`, plus the transcript fix, in one contracts PR. HIGH (the driver is gate-decision code), reviewed by 016c and 017f. Any other id that the check flags is fixed in the same PR, or listed in the shrink-only exceptions with its reason.

## R-84. The meaning of `oracle_encode`'s arguments and results, and of an unset title or cwd in `oracle_state` (Stage 1 Core lead, RC's design note `shared/core-stage1/handoffs/rc-oracle-encode-design-note.md` section 6, 2026-10-11)

`oracle_encode` with `modes` is a pure function of (kind, input, modes) that calls the pinned libghostty `*_with_modes` encoder, the one that Core's write path uses (R-7). All 243 calls in the 22 ids that use it pass `modes`. Note (1) of `core-testkit-controls.md` still covers a missing `modes`: the session's model is used.

- **(1) A partial `modes` object is merged onto the modes of a fresh terminal of the pinned libghostty, never onto the session's modes.** *(Corrected the same day, RC via the Core lead, oracle branch `cea1de64`. This item first said `ModeFlags::default()`, every mode off.)*
  - **The finding.** 19 of the 243 calls gave bytes that differ from Core's. A fresh libghostty terminal has `ansi_12`, `dec_1007`, `dec_1035`, `dec_1036` and `dec_7` on, while `ModeFlags::default()` has them off. The encoder reads `dec_1035` (numlock keypad) and `dec_1036` (Alt sends ESC) (`encode.rs` 47-48). The affected calls are 17 keypad calls in `in_9_application_cursor_and_keypad`, Alt+a in `in_9_legacy_keys_table` s207, and Alt+q in `in_9_non_us_layout_uses_supplied_text` s15.
  - **The base** is computed from the binding at run time, as the modes of a new terminal of the pinned libghostty. It is never a literal. The control stays pure: the base is a constant of the pin, and no session is read. All-off is a Rust `Default`, not terminal truth. Which modes a new terminal has on is terminal semantics, which R-7 gives to libghostty.
  - **The author's rule.** The transcript passes every mode that affects that input's encoding **and differs from a fresh terminal**. `modes: {}` means a fresh terminal. The reviewer checks that a partial object is complete in this sense.
  - **Rejected:** adding the initial modes as literals to the 19 calls.
- **(2) A typed zero is an empty hex string.** When the encoder writes nothing because the modes do not report the input (`NotReported`: mouse with tracking off, a release in legacy mode; focus with the mode off), the result is `""`, because the PTY gets nothing. **`Unsupported{what}` is a refusal:** `ControlError::Refused{reason: "unsupported", what}`, matched with `expect_refused`, because no bytes exist to compare.
- **(3) `paste` with bracketed paste off** returns the text alone (IN-8), with no `require_bracketed` argument.
- **(4) The `sgr_pixels` cell size:** RC asks only if no argument gives the value.
- **Follow-up: a mouse encode's size (016c on contracts #60, 2026-10-11).** A mouse encode may use the session's stored size. The purity claim above excludes only session modes and model state.
  - The size is host input: the transcript itself sets it with `Create` and `Resize`. It is not terminal semantics, so R-7 does not apply to it. `oracle_screen` already reads the stored row's size the same way (plans 23ax to 23ba).
  - A wrong stored size in Core would be on both sides of a mouse comparison. That risk is bounded, because `st_4_terminal_state` (ST-4) and `ev_6_keyed_event_carries_latest` (EV-6, after two `Resize`s) assert `terminal_state`'s size against what the transcript set.
  - **The row's wording:** "With `modes`, the call reads no session modes and no model state. A mouse event also uses the session's stored size (the size that the transcript set), unless the call passes `size`." An explicit `size` argument is allowed and wins. This applies to Core #288 too.
- **(5) `oracle_state`'s title and cwd are event-sourced, as Core's are.** Core has no projection function for them: `terminal_state` keeps the last title or cwd event (the host, `core-host inbound.rs` 465-467; the worker, `worker-core model.rs` 168-177 and 238-242). libghostty's getter returns `""` both when unset and when explicitly empty, so a getter cannot tell them apart.
  - **The form, (A):** the O_raw replay drains its own libghostty terminal events and keeps the last title and cwd. The value is `Some(last)`, an empty string included, and absent when no event came. Only the contract type's `Option` shape is shared (`TerminalState`, `types.rs` 259-262). "The last event wins" is written once in Core and once in the testkit, so a bug on either side shows. The source stays the oracle (R-7).
  - **Rejected:** (B), a getter with `""` mapped to absent, which diverges from Core after an explicit empty OSC 2 and puts a semantic mapping in the testkit; and (C), extracting a shared Core function, because none exists beyond `Some(x)`. An earlier draft of this item asked for a shared projection function. It was replaced the same day, because no such function exists.
  - RC tests in the binding whether libghostty posts an event for an empty OSC 2, and states the result in the PR body.
  - **The reading of ST-4's `title` and `cwd`:** the last value that the program set, an empty string included. They are absent only when the program set none. No clause separates the two cases. An explicit empty title is a title that the program set, and EV-7 says "Core drops none of these".
- **Owner.** RC: the Core control, plus the `oracle_encode` row in `core-testkit-controls.md` citing this ruling, in one contracts PR reviewed by 016c.

## R-83. A `route_expect_pty` read needs an `await_pty` fence that covers its sequence: R-56's lint extends to `route_expect_pty` (Stage 1 Core lead, P3's contracts PR `p3/am2-route-pty-fence`, 2026-10-11)

- **The lint rule (P3's text, accepted with one change).** A `route_expect_pty` reads the input log of its route's session. It must follow an `await_pty` of that session, with no write to the session between them (R-61(b) writes). The fence covers the shortest log that the sequence accepts: the bytes of its hex parts, plus the length of each relation part. A `var` part counts 0. For `one_of`, the shortest alternative counts. A bound count fences as it does for a capture (R-56(3)(1)). An empty sequence is an absence check and is not this pattern.
- **The change: the length of a relation part.** When the part's op has known bytes (R-73 follow-up 2(2): raw `bytes`, `text`, a paste body), the lint counts that op's known length. Otherwise it counts 1 byte, because a relation run is never empty (R-73).
- **The count is a floor, so the reviewer checks the rest.** The lint cannot know the true length of an encoder-made run (key, mouse, focus, paste markers). So each fence names the **full** written count: a literal total, or a matcher-bound written count (R-74, or the route form below). The reviewer checks this. The lint does not.
- **Route-bound counts (P3's class B).** A `written_bytes` that a route matcher binds from `input_done` or `input_refused` is a run-time count, the route form of R-74's matcher-bound count. Three conditions:
  1. it is bound in an earlier step;
  2. the route is attached to the same session;
  3. the clause of that op defines `written_bytes` as the bytes that the op wrote to the PTY.
  Where a clause gives `written_bytes` another meaning, the transcript derives the PTY count explicitly, and the PR body cites the clause for each finding. **DP-5b is not such a case:** it defines `written_bytes` as "the encoded PTY bytes of the path paste (the path, the trailing space and any markers; it **never counts the uploaded file's content**)" (corrected the same day after Sol's PF-SI-1). So in `dp_5b_path`'s fence `1 + $wb11 + 1`, the two extra bytes are other writes, and the PR body names each one.
- **Encoder-owned counts (P3's class C).** A count of bytes that an encoder owns (key, mouse, focus, and paste markers that depend on the mode) is bound with an `oracle_encode` step and fenced with a `let` sum. It is never hand-written (R-7). `oracle_encode` is unsupported on the Core tiers today, so these ids stay pending. The missing Core `oracle_encode` (RC, R-84) is the root gap, and this ruling makes it a priority.
  - A literal count is allowed only where the bytes are not encoder-owned. For example, a paste with bracketed-paste mode off writes exactly its body (IN-8).
  - No PASS→non-PASS. A class-C id that passes on any tier keeps its form and is listed with the reason "encoder count, oracle_encode (RC)" until Core supports `oracle_encode`.
- **Scope: all 31 findings at `63128469` are fenced** (29 transcripts; the R-70(3) and R-80 form). No starting exception list is allowed. These races are real: RC measured an empty real log in 4 of 4 runs for `dp_5_invalid_utf8`. **One exception:** `dp_5_full_input_queue_stops_reading_the_route_and_never_drops`, because RC's R-81 PR rewrites its input volume. If this PR merges first, that id goes on the list with the reason "rewritten by R-81 (RC)", and RC's PR removes it. If RC's PR merges first, this PR fences the new form.
- **Owner.** P3, one contracts PR, HIGH (the lint is gate-decision code), reviewed by 016c and 017f.

## R-82. DP-5's `session_ended` and `failed` classify by what failed, not by the order in which the worker learns of it (Stage 1 Core lead, RC, Core #284, 2026-10-11)

- **Finding.** After Stop's SIGTERM, the waiting PTY write can fail (`EPIPE` on the testkit, `EIO` on a real Linux PTY) before the worker takes `PayloadExited`. The worker maps `WriteOutcome::Failed` to `failed`, so the reason depends on the order. Seed 1 of `dp_5_session_ended_…_partial_paste` gives `failed` with `written_bytes` 3, and the transcript expects `session_ended`.
- **The reading (R-50: classify by what failed).** `session_ended` means the write failed because the program's side of the PTY is gone. `failed` means an OS write error while the program's side still exists. A PTY write error that the OS reports *because* the program's side is gone is therefore `session_ended`, whether or not the worker has seen the payload exit yet. These are `EIO` on a Linux PTY master after hangup, `EPIPE` on the testkit's stream, and the errno that macOS reports in the same case. Every other errno stays `failed`.
- **Rejected as the only rule:** RC's (a), "a write error after the worker sent SIGTERM or SIGKILL, or after the payload exit is known, gives `session_ended`". It classifies by timing, it misclassifies a program that closes its terminal by itself, and it keeps the race.
- **Conditions.**
  1. The errno set is a named table in the worker, and each entry cites what the OS means by it on that platform. The macOS errno for a write after the program's side closes is measured, and it is added only from that evidence.
  2. `written_bytes` stays exact under both reasons (DP-5, IN-2).
  3. Red-on-revert tests for both orders: the write error before `PayloadExited`, and after it.
  4. The transcript is unchanged. It already expects `session_ended`.
- **Owner.** RC, in Core #284, reviewed by Sol (017a).

## R-81. The block in `dp_5_full_input_queue_stops_reading_the_route_and_never_drops` comes from Core's real allowance and the stream's capacity, never from the fake's admission script (Stage 1 Core lead, RC, 2026-10-11)

- **The facts.** The transcript sends 12 `bytes` frames of 6 bytes (about 228 bytes on the stream), then expects `input_blocked {blocked: true}`. FakeCore passes only through its scripted `fake.admission`, which is fake-only data (R-76's class).
  - The transcript's `setup.limits` already sets `CoreLimits.route_input_queue_bytes: 8`. Attach cannot set that field: OU-1's attach `route_limits` has only `max_frame_bytes`, `max_screen_frame_bytes`, `max_history_page_bytes` and `max_chunk_bytes`, and `route_input_queue_bytes` is Core-internal.
  - The real cause is R-53: the effective allowance is `max(route_input_queue_bytes, max_frame_bytes + 4)`, and `max_frame_bytes` defaults to `max_paste_bytes + 4096`. So 228 bytes never fill it.
- **`blocked: true` needs two things full:** the worker's effective allowance, and the stream's own capacity (the testkit stream holds 64 KiB; on the real tier, the socket buffer). The client input must be more than their sum on every tier.
- **The form.**
  1. The input volume is a named constant, at least twice the largest sum on any tier, sent with `repeat`, not as thousands of steps. No stream or kernel count is asserted.
  2. The attach may lower the allowance with `route_limits.max_frame_bytes`. The value must be at least the A19-1 floor, and it must not be a hand-written floor that can break at a pin. If the transcript has no non-literal way to name the floor, it keeps the default and sends more than `max_frame_bytes + 4` plus the stream's capacity.
  3. After `pty_blocked off`, `pty_input` contains every sent byte, exact and in order (the "never drops" claim), with no `0d` (R-61(c)).
  4. `fake.admission` stays as fake-only data. If FakeCore cannot pass the new form, the id is `structural` on FakeCore, because FakeCore is frozen.
- **Owner.** RC, one contracts transcript PR, reviewed by 016c. The real-tier form of `input_blocked` and its measured capacity come before the id leaves `core-real-pending.txt`.

## R-80. A model read after an output on the same session needs an output fence, and one lint covers every model read (Stage 1 Core lead, P5's `st_6` diagnosis, 2026-10-11)

- **Finding.** `st_6_capture_is_frozen_at_model_rev` fails on the real tier at step 17 (`check $rev1 != $rev2`): seed 5 on `.36` (`gates/botster-core-detached-b6fd4b98-b6fd4b98-pool-20261011-102704-34139.log`) and seed 1 on `.39` (`evidence/p5-r68/pin-a54-linux-v039-dd30a41e.log`). Step 14 is `await_quiet`, which reads nothing (R-61(a)), so the `ReadScreen` at step 15 can answer from Core's model before the worker applies the output. The R-57 sweep missed it, because it counted the read's own reply wait as the wait. This is the third R-57-class missing output fence in one night (`ou_9` #49, `in_9` R-77, `st_6`).
- **`st_6_capture_is_frozen_at_model_rev`:** step 14 becomes `pump_until Activity{id: s1, source: Output, at: 5001}`. Step 12's clock makes the `at` distinct, so step 6's `Activity` at 5000 cannot meet the wait (R-61(a)(2)). Step 11 drained the older events. Step 7 is also required: it becomes the same wait with `at: 5000`, so that `$rev1` includes `41`.
- **The lint (R-61(a)(1)). It replaces R-77's narrower rule.** It flags an output on S (`pty_output`, `pty_chunk`, `program_write_once`), then a model read on S (`ReadScreen`, `CaptureSnapshot`, `terminal_state`, or a `begin` whose guard names a `model_rev`), with no output fence on S between them.
  - **An output fence** is a wait that ends only after the worker reports that output: `pump_until` or `await_*` on an `Activity{S, Output}`, or an R-57 title sentinel of S, made distinct by R-61(a)(2). `await_quiet`, `drain`, `advance_clock`, `pump`, `let` and `check` are not fences. **The read's own wait for its reply is not a fence.** The R-57 sweep is corrected on this point.
  - **Output-derived events are fences too (P5's measurement: 16 matches in 13 transcripts).** A wait on an event that only the worker's application of that output posts (`ModesChanged{S}`, `TitleChanged{S}`, `Bell{S}`) counts as an output fence under three conditions:
    1. it ends the wait: a `pump_until` on it, or a `pump_collect` that ends on it;
    2. it is distinct per R-61(a)(2): no earlier output can post a matching event;
    3. the sequence that posts it is the last bytes of the output step. A real read can split one step, so an event from the middle of the bytes proves nothing about the bytes after it. This is why R-57 appends the title sentinel at the end.
    The lint takes a named list of these events. A wait that fails any condition is not a fence.
  - **Where a fence would consume an event that a later step asserts** (R-77 check (b)), the fence is an appended R-57 title sentinel. A match goes in the exception file only when no sentinel works either, with that reason.
  - **The lint does not judge whether an assertion depends on the output.** A lint cannot decide that. Every match is either fenced or listed in the shrink-only exception file with a one-line reason that the reviewer checks.
  - The rule ships with a self-test that is red on today's `st_6`.
- **The `st_6` matches (P5's check):**
  - `st_6_capture_is_frozen_at_model_rev` steps 8 and 15: fenced at steps 7 and 14.
  - `st_6b_baseline_restores_palette_cursor_shape_hyperlinks_modes_and_kitty_flags` s8: racy (`oracle_restore` compares the capture with the oracle's model after step 6's output). It is fenced.
  - `st_6b_graphics_only_when_the_feature_is_advertised` s10: racy (`images_in_baseline` depends on step 8's graphics output). It is fenced.
  - The two `st_6b` ids need controls that Core lacks today (`oracle_restore`, `oracle_graphics`), so they cannot flake in a Core gate yet. They are fenced now, before those controls land.
  - `st_6_pages_owned_last_flag_and_range` s8: fenced with an appended R-57 title sentinel. *(Corrected the same day, 016c on #54: this line first listed it as an exception, because it asserts only page structure. That conflicts with this ruling's rule that a match is listed only when no sentinel works. A title sentinel adds no screen content, so the page-structure checks are unchanged, and a sentinel works.)*
  - Each fence is `pump_until Activity{s1, Output, at: <that output's clock>}`, with an `at` distinct from every earlier unpolled `Activity` of s1.
- **The other 11 matches** are fenced by the rules above, or listed with a reason.
- **Each new wait carries R-77's two checks:** no earlier unpolled event can meet it, and no later step asserts an event that it consumes.
- **Owner.** P5, in contracts #54 together with R-77's `in_9` fence. R-77's narrower lint is not merged on its own. #54 is HIGH (the lint is gate-decision code), reviewed by 016c and 017f.
- **Follow-up: an `Activity{S, Output}` wait proves only the first read** (PC's diagnosis `shared/core-stage1/handoffs/flaky-ad-3-adopted-equals-started-79f31be3.md`, via the Core lead, 2026-10-11). `ad_3_adopted_equals_started` fails on the real tier at step 16, "the relation does not hold" (seed 2 in PC's R-78 A5-4 job; seed 0 on a tree without R-78, `evidence/p5-r68/pin-a54-linux-v036-84b1e9be.log`). Step 4 writes `68690a` (3 bytes). Step 6's `pump_until Activity{s1, Output}` ends at the first `Activity`. The real worker can read the 3 bytes in more than one read, so `$ts0` at step 7 can predate the whole output, while `$ts1` after `AdoptAll` has all of it.
  - **The rule.** Condition 3 above ("a real read can split one step") applies to a plain `Activity{S, Output}` wait too, not only to output-derived events. An `Activity{S, Output}` wait is an output fence only when the output step is **one byte**. After an output of more than one byte, the fence is an R-57 title sentinel appended at the end of the step's bytes, with `await_title`. The worker applies bytes in order, so the title proves that every byte before it is applied.
  - R-77's check (b) does not cover this. It is about a wait that consumes an event that a later step asserts, not about a split read.
  - **`ad_3_adopted_equals_started`:** append a title sentinel to step 4's bytes, and `await_title s1` replaces step 6. The sentinel changes `$ts0` and `$ts1` alike, so step 16 still compares like with like. It is live on both tiers, so the fix goes in .42 if it is ready in time. Owner: P3.
  - **The lint.** An `Activity{S, Output}` fence after an output step of more than one byte is not a fence. P3 adds this to R-80's lint in the R-80 growth commit. Each new match is fenced with a sentinel, or listed with a `debt:` note.
  - **A title that the session's own program prints** (P6's #75, the same day) is a sentinel only when its title sequence is the **last bytes of the program's final print** before the read. Any later title or output bytes make it not a fence (condition 3 above). The fence lint keeps one sentinel check, for both `pty_output` and program-printed sentinels.
  - **The strict count (P3's question, the same day).** The lint counts every unfenced output byte on S since the last fence, across all steps, not only the last output step. An `Activity{S, Output}` wait is a fence only when that count is exactly 1. P3 measured it: no transcript changes today.
  - **The three `a8_2_*_inside_an_uncarriable_sequence_*` ids are `exception:` lines.** No fence exists: a plain `Activity` wait proves only the first read, and a title sentinel after an unterminated OSC or DCS string becomes part of that string. The race can only fail these tests, never pass them wrongly: if the model has not applied the whole sequence, the capture, baseline or resync is formed, and the expected `SnapshotTooLarge` is missing. The line also tells whoever diagnoses a later FLAKY failure of these ids where to look. If Core ever posts an event when the model's parser enters the uncarriable state, that event becomes the fence, and the lines are removed.

## R-78. R-72's rule extends to Stopping, and Core reads the control link to EOF before it classifies a worker's exit (Stage 1 Core lead's triage of Core #277's other first-of-two places, 2026-10-11)

- **(1), (2) and (6), an adopted worker's link loss,** go to candidate A21. There is no Core change now.
- **(3) Stop with a lost link, for a worker that this host spawned.** R-50 classifies by what failed. When `stop_grace` ends with the link lost, the reason follows the worker's real status: `Lost(WorkerGone)` if the worker has exited, and `Lost(WorkerUnreachable)` if it lives.
  - **The preferred form needs no wait:** at the grace point, Core checks the spawned worker's exit status without blocking. The link was lost a full `stop_grace` earlier, so a dead worker's exit is known by then.
  - **If the sans-IO design cannot check it,** Core waits for the first of the exit report and an **existing** deadline that the PR names. No new `CoreLimits` field is added.
  - LC-5 still holds: the session ends, and `Stop` completes after `Exited` or `Lost`.
- **(4) Drain before classifying.** On a worker's process exit, Core reads the control link to EOF and handles every complete message before it classifies. There is no new timer, because EOF follows the exit. **The observable change:** an `Exited{code}` that the worker wrote before it died now gives `Exited(code)`, not `Lost(WorkerGone)`. That is R-50's "the reason follows the recorded state".
- **(5) LaunchFailed vs exit:** no action.
- **Evidence.** Red-on-revert tests for both orders of (3) and (4). The A5-4 table with no PASS→non-PASS.
- **Owner.** PC: one Core PR, after its current harness PR.
- **Follow-up: (4) applies to every process exit, also after the end is chosen** (0179's MEDIUM on Core #291, via the Core lead, 2026-10-11). Today, after Core has read `Exited` and chosen the end, a later process exit closes the link at once. Complete frames that the worker wrote after `Exited` are then dropped: a `Done` served from the final model, or the `RouteClosed{SessionEnded}` of a route. The held route then closes `SessionLost`.
  - **(a) Core delivers them.** (4) is not scoped by the session's state. The state scope in R-78's heading is item (3)'s. OU-7: "After `Exited`, each route's close follows delivery of its own queue." R-50: the reason follows the recorded state. A close or a completion that the worker wrote before it died is recorded state, so dropping it reports a loss that did not happen.
  - **(b) No bound is needed, and Core waits for nothing.** A process's writes have finished before the process ends. So when Core handles the process exit, every byte that the dead worker wrote is already in the link's receive buffer. Core reads the link **without blocking** until EOF or until a read would block, and it handles every complete frame. A read that would block ends the drain: another holder of the link can keep it open, but it cannot hide the dead worker's frames. A partial frame at the end is a truncated link (link loss), as today. No `CoreLimits` field and no deadline are added.
  - **The test** `a_held_route_closes_session_lost_when_the_worker_process_ends_after_the_exit` (OU-7, #217 R2-1) stays correct only where the worker never wrote that route's close before it died. The test's owner checks its setup against this rule. Where the worker did write the close, the route closes `SessionEnded`.
  - **Owner.** PC, in Core #291: PC's accepted design (rev 3) already drains after `Exited`, so #291 carries the fix and the change to the R2-1 test. The reviewer checks that the drain after the process exit never waits for EOF. Red-on-revert evidence: a `Done` and a `RouteClosed{SessionEnded}` written after `Exited`, then the process ends, and both are delivered.

## R-77. A guarded write after new output needs an output fence (Stage 1 Core lead, P5's FLAKY record `shared/core-stage1/handoffs/flaky-in_9-key-repeat-is-checked-by-the-guards-once-at-start-2866d592.md`, 2026-10-11)

- In `in_9_key_repeat_is_checked_by_the_guards_once_at_start`, step 6 `pump` becomes `pump_until Activity{s1, source: Output}`, as in `in_10_terminal_guard_refuses_after_a_redraw_before_start`. The worker reports `Activity{Output}` after it consumes the output, and it handles the later `WriteInput` in order. So the guard sees the new `model_rev`.
- Two checks, stated in the PR: (a) no earlier unpolled `Activity{Output}` of s1 can meet the wait (R-61(a)(2)), and otherwise an R-57 title sentinel is used; (b) no later step asserts an event that the wait consumes.
- **The lint.** This ruling first gave a narrower lint for a guarded `begin` only. R-80 replaces it with one rule for every model read.
- **Owner.** P5, in contracts #54, reviewed by 016c and 017f.

## R-76. Core ignores a `pty_output` step's `scripted` field, as it ignores Create's `fake` (Stage 1 Core lead, RC, 2026-10-11)

- On Core, libghostty derives the modes from the real bytes (R-7). The mode script is data for the fake only. The conditions:
  - the parser names `scripted` as the one ignored field, and `deny_unknown_fields` stays;
  - in every transcript that has `scripted`, the step's bytes alone must produce every mode that the transcript later asserts. A transcript that depends on the script alone is a transcript defect, fixed in contracts, and never passes on Core by the script.
- **The audit.** 016c checked all 34 Core transcripts that carry `scripted` at `8d79b70d` (`.review-verdicts/r76-scripted-audit-8d79b70d.md`). One defect: `in_9_kitty_unknown_flag_bits_ignored`, closed by R-28's follow-up (contracts #55). The other 33 are clean.

## R-75. The refusal reasons of route input: a field-rule failure, paste and file slots, and a reply on the wrong route (Stage 1 Core lead, RC's worker route-input design, plan 23bn, 2026-10-11)

- **q1. A parseable route input frame that fails a 5.1A field rule** (`Decoded::Invalid`) gets an `input_refused` with **zero effect**, and the route stays open. The reason depends on the failure:
  - a value over a stated size bound gets `too_large` (for example A15-1's `max_key_text_bytes`);
  - an unknown kind or enum value, or invalid UTF-8, gets `unsupported { what }` (codec unknown-value rule; TS-5b);
  - **any other field-rule failure gets `refused`.**
  It is **never `invalid_input`.** In the codec, `invalid_input` is only the protocol error that ends a route ("Only an unparseable frame ends a route"; rev27 R-3), and it is not an `input_refused` reason for a parseable frame. So the worker's `invalid_input` today is wrong. FakeRoute's `refused` matches this ruling.
- **q2. Paste and file are separate slots.**
  - TS-5d refuses "a second `paste_begin` while a **paste** is open".
  - TS-5f refuses "a second `file_begin` before the first ends", and says that "other input keeps flowing while a file uploads".
  So a `paste_begin` while a file is open is not a sequencing error. RC's reading is right. It is also what `dp_5b_other_input_flows_during_an_upload` tests.
- **q3. A reply on the wrong route gets `wrong_route`.** Core EV-8(c) says `input_refused{invalid_input}`. The codec, rev27 R-3, changed exactly this case: "a reply from a route that the query was not bound to is refused `wrong_route`; `invalid_input` stays only the route-ending protocol error". The codec owns the route's wire reasons (contracts.toml: the codec comes first among the route ids). Its later, specific text was made to remove this overlap. EV-8(c)'s `invalid_input` would also contradict the codec's own definition of `invalid_input`.
  - **The text needs an erratum.** Core EV-8(c)'s text is not changed by this ruling. The steward opens Core erratum 6 on the amendment path *(corrected 2026-10-11: this first said "erratum 5", a number already used by a candidate that Amendment 13 replaced)*: EV-8(c) reads `wrong_route` for this case, and its list of reply refusals drops `invalid_input`. Until it is frozen, this ruling governs, so the worker and the transcripts use `wrong_route`.
- **Also confirmed:** RC's reading of `paste_commit` with no open paste. It is `input_refused{refused}` (TS-5d: "a commit or abort with no open paste … refused"), with zero effect. The worker's `unsupported` there is wrong.
- **Owner.** RC, in the worker's route-input PR. The transcripts that assert any of these reasons are checked by 016c.
- **Follow-up: an undecodable `bytes_base64` (016b, while building the final40 crate PR, 2026-10-11).** An `encoded` reply whose `bytes_base64` does not decode is a field-rule failure of a parseable frame. So by q1 it gets `input_refused{refused}` with zero effect, and never `invalid_input`. The codec gives no more specific reason. The frozen fake route (`botster-fake-route/src/input.rs` 371) answers `invalid_input`. It stays unchanged, because the fakes are frozen with no new fidelity (BUILD.md). No transcript asserts this case at `2a568fa`. A transcript that asserts it must assert `refused`, and it is `structural` on the fake route.
- **Erratum 6 is frozen** in manifest final40 (`manifest-final40`, 2026-10-11). EV-8(c) now reads `wrong_route` for a reply on the wrong route.

## R-74. A held write on the real tier fills the kernel's PTY buffer, a new read-only `pty_queued` control fences it, and the per-line revisions of PC's group of 9 ids (Stage 1 Core lead, PC's note `shared/core-stage1/handoffs/pc-group9-design.md`, 2026-10-11)

- **The probe's mode.** R-48 already makes the probe clear `ICANON` (with `VMIN=1`, `VTIME=0`) when its stdin is a terminal. So every real-tier session runs non-canonical, and macOS's canonical drop past the line limit cannot happen. PC confirms this in the probe before the PR. **One change:** the probe ignores a failed `tcsetattr` today (`let _ =`). It must instead fail loudly with a probe error, so a session never runs canonical in silence.
- **The held write (R-47 item 7).** A real held write needs a full kernel buffer. `pty_blocked on` stops the probe's reads, and the write under test carries F bytes.
  - F is greater than the measured non-canonical PTY capacity on **both** OSes, with margin: twice the larger measured value. macOS measures 1022; PC measures Linux in one evidence job.
  - F is at most the write limit. If F cannot fit, PC brings me the numbers.
  - F is a named constant in the transcripts. It is not a kernel count that a transcript asserts.
- **The fence: `pty_queued {session}` → `{bytes}`, accepted.**
  - On the real tier it reads `FIONREAD` on the probe's stdin: the bytes that wait in the kernel buffer. On the testkit it reads the testkit PTY's accepted-but-unread count.
  - It is read-only, at the program edge, and it reads nothing in Core.
  - The fence is `await_control {pty_queued} expect bytes ≥ 1`. That is enough: one byte proves that the write started and that k ≥ 1. With F greater than the buffer, the write cannot complete while the probe is blocked, so the op is pending with 0 < k < F.
  - A test that asserts an exact `bytes` value over-asserts (R-43 item 7).
  - **The fake forms (PC, 2026-10-11).** `pty_blocked on` takes no bytes on the fakes, so the fence could never end there. The ruling:
    - **An opt-in argument `pty_blocked {session, on: true, kernel_bytes: n}`, not a separate control.** On the testkit (Core's edge, which is not frozen), the PTY takes up to n more bytes into a queue that the program does not read, then stops. `pty_queued` reports that queue, and `on: false` delivers it in order. On the real tier, n is ignored, because the kernel has its own buffer. The row says so. Only the held-write transcripts pass it, with n = 1. The 30 other users of `pty_blocked` do not change.
    - **FakeCore stays frozen** (BUILD.md: "no new fidelity"). It gets no buffer and no `pty_queued`. The held-write transcripts are `structural` on FakeCore, as R-60 did for `ou_7`.
- **Per-line rulings.** "K" marks a kernel count, bound by `$p` and never a literal.
  - **`in_2_partial_is_certain`, `in_6_cancel_reports_progress`, `in_2_link_loss_is_unknown_not_zero`, `in_7_lost_worker_ends_pending`, and `am_2_…` part 2:** accepted as PC lists them. In each one, before `pty_input` is read, the steps are `pty_blocked off`, then `await_pty` for the written count. `s11 p < F` is accepted. am_2's ruled read fence at s8 goes in the same PR, coordinated with P3's step-14 fence.
  - **`a2_2` section 1 (s6): (b) is accepted.** s6 accepts `$one_of[TooLate, Admitted]`, and s7 (`Written`, 1, 1) stays strict. Both branches test the clause: A2-2 says an admitted cancel completes with "the real outcome of the write if the write finished first". The ruled read fence at s9 goes in the same PR.
  - **`a2_2` section 2:** the held form with F bytes. s19 `payload_bytes_written` becomes `$p` (K). s20's log becomes the F-prefix of length `$p` (K).
  - **`a2_2` section 3 (s26-s30): PC's `Cancelled 0 | Partial $p` is refused,** because it weakens the claim. Real Core starts the write before the cancel arrives, so "begun, but the kernel took 0" cannot be built on a real PTY. The deterministic real form of the same claim queues the write behind a held one (AM-2):
    1. `pty_blocked on`;
    2. a filler write `$w0` of F bytes;
    3. the `pty_queued ≥ 1` fence;
    4. `begin` `$w3` (5 bytes), which waits behind `$w0`;
    5. `cancel $w3` gives `Admitted`;
    6. `$w3` completes `Cancelled` with 0 and 0;
    7. then end `$w0` (cancel it or unblock it), and `pty_input` contains none of `$w3`'s bytes.
    `$w3` never started, so it wrote nothing on either tier.
- **`ev_2_retired_instance_never_creates_a_loss_marker` and `ev_5_released_retires_keyed_droppable_and_loss_events`.** The fence before the `get` is **`await_get {state: Exited}`**, not `pump_until Completed`. `await_get` pumps and never polls, so the `EventsLost` and keyed events that these ids assert later stay unpolled (R-63, R-66). A `pump_until` would consume them.
- **`ev_2_drop_oldest_with_marker`: (a), the R-57 sentinel per bell (decided after PC's trace, `shared/core-stage1/evidence/pc-ev2-drop-oldest-trace-macos.log`).** The count is right on both tiers. On the real tier, about a third of the bells carry the next second's `at`, because `await_quiet` returns before the worker's bell report reaches Core.
  1. Each bell's `pty_output` is `07` plus an OSC 2 sentinel with a **distinct** title (B0 … B19), then `await_title` on that title, before `advance_clock`. A repeated title changes nothing, so it would not be observed.
  2. `TitleChanged` is class K, with its own bound, so the D-queue drop math is unchanged. Under EV-1, the replaced `TitleChanged` moves to the newest position. So the expectations **assert** exactly one trailing `TitleChanged{B19}`; they do not just ignore it.
  3. (b), a new bell observer, is not needed. (c), a range on `at`, is refused as a weakening.
- **The lint (R-56, R-61; PC via the Core lead, 2026-10-11).** A count that a `pump_until` or `pump_collect` matcher binds from a `Completed` of a `WriteInput` is a run-time value, as much as a `let`-bound count. So it is a valid `await_pty` count. The lint accepts it under these conditions:
  - the variable is bound in an earlier step than the `await_pty`;
  - the `WriteInput` is for the same session;
  - the field is `pty_bytes_written`, or `payload_bytes_written` only when that write's payload is raw `bytes`, which have no encoding, so the two counts are equal.
  The change ships with a lint self-test. It removes the 3 old R-56 exceptions (`in_2_partial` step 8, `in_6` step 8, `am_2` step 18) and adds none. The R-60 entries of these ids are only renumbered. *(Corrected 2026-10-11: this item first said "4". PC via the Core lead; plan 23bt `a730792d` records 3.)*
- **`in_7_lost_worker_ends_pending` step 10 (PC via the Core lead, 2026-10-11).** After `lose_worker`, both writes complete on the link's EOF before Core records the worker's exit. So the `Remove` barrier can still find `Running`, which is a correct state: a broken link leaves the session `Running` until the exit is known (`lc_5`). IN-7 fixes no order between the completions and the state change. The fence is `await_get {id: s1, expect: {state: {Lost: $any}}}` between step 9 and the barrier. It pumps and never polls, it waits for the outcome, and it asserts no order. The step went in #52.
- **Owner.** PC: one contracts PR for the held-write group (the transcripts, the `pty_queued` row and the probe's form, plus the `tcsetattr` change), and one for the ev_2 and ev_5 read fences. The crate reviewer 016c reviews both, with Sol as the second reviewer for HIGH. The Core PR adds the harness's real `pty_queued`. The ids leave `core-real-pending.txt` only after the Linux gate passes.
- **Follow-up: the class of a `pty_blocked` hold with no held-write form** (PC's sweep `shared/core-stage1/handoffs/pc-pty-blocked-unheld-sweep.md`, contracts `main` `ecbd6ac8`, via the Core lead, 2026-10-11). On the real tier, `pty_blocked` stops only the probe's reads, and the kernel takes a small write at once. So input that a transcript means to hold completes early, a later `pump_until` drops that `Completed`, and a later wait times out. Of 27 transcripts that use `pty_blocked` for input: 2 are fenced by #77, 5 are safe, and 19 race. One of the 19 is live: `in_8_mode_checked_at_write_start_by_worker` (seed 5, step 8, `step_timeout`).
  - **The rule.** A transcript that needs input to stay pending under `pty_blocked` uses this ruling's held-write form: `kernel_bytes`, the fill write, and `await_control pty_queued` before the input. `pty_blocked` alone holds nothing on the real tier.
  - **The lint (new, R-80's form).** It flags input sent while `pty_blocked` is on with no `kernel_bytes` hold, followed by a Core-state assertion after a pump and before `pty_blocked` is off. It ships with a self-test. It flags all 19 racing ids. The 5 safe ids get `exception:` lines with their reasons, and the racing ids get the fence or a `debt:` line.
  - **Owners.**
    - PC, in one contracts PR with the lint: `in_8_mode_checked_at_write_start_by_worker` (live, so first), `am_2_fairness…`, `dp_5_every_not_written…` and `in_9_key_encoded…`.
    - P3: the 7 `e4_1`/`ev_8` ids, whose bound is sized to an 8-byte fill, so a fill would change what they test. P3 proposes their fence.
    - P5: the 7 `st_7` ids, where a host fill adds records to exact `input_records` arrays, so the matcher must change.
    - Each such PR names the ids that it fences and removes their `debt:` lines.

## R-73. The relation parts of `route_expect_pty` are answered from the byte log and the transcript's own frames, not from the worker's internal op attribution (Stage 1 Core lead, P3's design note `shared/core-stage1/evidence/p3-testkit-route-input/op-attribution-design-note.md`, 2026-10-11)

- **Question.** `written_by_op` and `written_without_op` need the PTY input log as segments `{op, bytes}`. The op exists only in the worker machine's admission state. The probe sees bytes after the line discipline, cut at its own reads. P3 proposes two things together:
  - an owner field on `Action::PtyWrite`;
  - making the relation parts testkit-only proofs.
  Two minimum ids depend on this: `dp_3_route_codec_matches_client_appendix_a4_a5` and `dp_3_data_frames_are_raw`.
- **What the clauses make observable.** The worker's attribution of a PTY write to an op is not observable to any client or host. The route shows an op only in its own frames (`input_done`, `input_refused` with the op and counts, DP-5), and the transcripts already assert those. What DP-3 and DP-5 promise about the PTY is the bytes, their order, and that one admitted transaction is contiguous (AM-2: a paste is written contiguously at commit). So a relation part that asks "which op did the worker think it was writing" over-asserts, as R-43 item 7 defines over-assertion.
- **Ruling.**
  1. **(1) and (5) are refused.** An owner on `Action::PtyWrite` that only the testkit reads is test data in the shipped machine's API (Testing rule 8, as in R-62 Q1). Making the two minimum ids testkit-only proofs would also change the A20-1 list and the minimum set, which needs an amendment and the orchestrator. The relation parts can be answered without either.
  2. **The new meaning (both tiers, one meaning).** The driver resolves the relation parts from the transcript's own frames and the literal byte log:
     - **`written_by_op: X`** is the contiguous run of the bytes that op X's frames carry, as the transcript sent them: the paste chunks concatenated in order, with the bracket markers that the paste's `require_bracketed` and the session's mode give. It must appear at this position in the order, as one run. `contains` and `len_var` apply to that run.
     - **`written_without_op: true`** is a non-empty run that no op-carrying frame of the transcript accounts for. Examples are key, mouse and focus input, and `bytes` or `text` frames with op `0`.
     - Where the run's bytes depend on terminal encoding that the transcript cannot state (R-7: no hand-written encoder), the part asserts only what the transcript knows: non-empty, its position, and its `contains` or `len_var`.
  3. **(a) A segment is one admitted transaction, never one PTY write.** AM-2 makes a transaction contiguous. Short writes split it into several writes, and the probe cuts reads anywhere. Under item 2, segments disappear entirely: the log is one byte stream, and the parts are runs in it.
  4. **(b) FakeCore's op as a JSON number.** Under item 2 the driver no longer reads `segments[].op`, so a numeric op in FakeCore cannot make `written_without_op` pass falsely. If P3's run confirms the mismatch, record it in `docs/fake-replacement-map.md` as a known FakeCore defect. FakeCore stays frozen and is not fixed.
  5. **Lint rule.** These logs are exact input-log assertions, so R-61(c)'s input rule applies: no `0d` in them.
- **Evidence.**
  - Before/after of the 18 ids on the testkit and the real tier, plus the two minimum ids with N ≥ 5 real runs on macOS.
  - A negative test: a paste whose bytes are split by another writer fails `written_by_op`.
  - No PASS→non-PASS elsewhere.
- **Not decided here.** Many of the 18 ids also need paste, file or key input, which the worker refuses today (`route.rs` 133-147). That is Core work, and this ruling does not make those ids pass.
- **Owner.** P3: one HIGH contracts PR (the driver semantics of the relation parts, and the `route_expect_pty` doc), reviewed by 016c and Sol. No Core change.
- **Follow-up: a paste run's bytes (P3 via the Core lead, 2026-10-11). (A) is accepted. (B) and (C) are refused.** The Hub fake route writes the script's paste bytes, not the chunks, by design (R-7: it has no terminal). So an exact paste run fails 8 codec transcripts on the fake route.
  1. **(A):** a paste run is matched by the item-2 fallback on every subject: it is non-empty, at its position, with its `contains` and `len_var`. The paste encoder (5.1A) owns the exact bytes, so the run is terminal-dependent in the sense of R-7.
  2. **The Core tier must still prove the chunk bytes.** Every Core transcript with `written_by_op` on a paste carries a `contains` of the paste body, with a body that has no byte the 5.1A paste encoder may change (printable ASCII). *(Corrected the same day: IN-8 writes the paste payload verbatim. Only the markers depend on the mode, and "Core does not inspect the payload". So any body byte except `0d` is exact. `0d` is excluded because `ICRNL` turns it into `0a`, under R-61(c)'s input rule. The `contains 610a62` of `dp_5_confirmed_paste_resend_is_a_new_op_with_require_bracketed_false` and `dp_5_mode_precondition_zero_effect_reported_on_the_route` stays as it is.)* For example, `dp_3_route_codec_matches_client_appendix_a4_a5` gains `contains 6667` on op 7. `dp_3_data_frames_are_raw` already has `contains 7b7d`. P3 lists every transcript that it changes.
  3. **Placement is unambiguous.** Parts consume runs in order, and a run that a part's `contains` does not match is not that part's run. The negative test uses an exact other writer, or a `contains` of the body.
  4. **(B) is refused.** The fake route endpoint is frozen (BUILD.md), and the change would alter the codec scripts' meaning. **(C) is refused.** One meaning on all subjects is the point of item 2.
  5. **The lint and `0d` (017f's F2 on #53, 2026-10-11).** The lint may flag a `0d` in any paste body that a relation part covers, not only in a `contains`. `ICRNL` makes that run tier-dependent, so this stricter check is accepted. **Extended (017f via the Core lead):** the lint also flags a `0d` in **any** sent `bytes`, `text` or paste body of a session whose input log a `written_without_op` asserts. Under follow-up 2, the accounting looks for an unplaced op's exact bytes in the log. On the real tier, `ICRNL` turns `0d` into `0a`, so the accounting could not find such a body, and the span would pass falsely. This is R-61(c)'s purpose: sent bytes must be findable in the log.
- **Follow-up 2: what "accounts for" means in `written_without_op` (017f's F1 on #53 via the Core lead, 2026-10-11).**
  - **P3's driver meaning is accepted.** A `written_without_op` span fails if it contains the exact bytes of a known-bytes op run that no part placed (`without_is_unaccounted`). It only adds failures. A coincidental match gives a false failure, never a false pass.
  - **(1) A refused frame writes no run.** The driver excludes an op from `runs()` when it has observed an `input_refused` for that op with zero effect (no `written_bytes`, or 0) before the assertion. A refusal that reports a partial write (`failed`, `session_ended` with `written_bytes` > 0) counts as a run of that length when `written_bytes` is exact, and as unknown when it is absent. This was a driver defect, fixed in #53.
  - **(2) The known bytes include a paste's body.** IN-8 writes a paste payload verbatim, so an unplaced paste op's body counts as known bytes, and a without-op span that contains it fails. The bytes of key, mouse and focus frames are known only to the encoder (R-7), so a without-op span can cover them. That is a stated limit, written in this ruling and in the `route_expect_pty` doc.

## R-72. When a starting worker's link ends, Core waits for the worker's exit, up to the startup deadline, before it classifies the failure (Stage 1 Core lead, PC's design note `shared/core-stage1/handoffs/pc-minimum-e1_1-a2_3-design.md`, 2026-10-11)

- **Finding.** On the real tier, killing a starting worker gives Core two edge reports: link EOF and worker exit. Core classifies by whichever report comes first. EOF first, before `Launched`, gives `Exited` with `SessionEnded{exit: Other}`. Exit first gives `Lost(WorkerGone)`, then `SessionLost`. So `a2_3_lost_session_closes_routes_session_lost` depends on the order of the two reports.
- **Reading.** R-50 classifies by **what failed**, not by which edge reported it ("none asserts which edge reported the failure"):
  - a worker that died gives `Lost(WorkerGone)`;
  - a link lost while the worker lives gives `Exited`.
  A kill is a worker death, so the truthful outcome is `Lost`. Core is wrong when the EOF wins the race.
- **Ruling: PC's fix is allowed.** On EOF of the link of a worker that this host spawned, in `AwaitHello`, `SendLaunch` or `AwaitLaunched`, Core classifies at the first of two events:
  1. **the worker's exit report:** `Lost(WorkerGone)`, as for a death;
  2. **the existing LC-4 startup deadline:** the outcome for a link lost with the worker alive, as R-50 defines it (`Exited`, `SessionEnded{exit: Other}`).
  There is no new timer.
- **LC-4 allows the later end.** LC-4 requires a typed `StartFailed` and a row that is not `Starting`, with `CoreLimits.startup` as the deadline. Ending within that deadline meets it.
- **Conditions.**
  1. **The same `StartFailed` reason as today's EOF path.** When the deadline fires, the result is the link-lost failure, not a startup timeout: the worker did connect, then lost its link. Only the timing changes. The PR states the reason before and after.
  2. **The session stays `Starting` while Core waits.** No event is posted for the EOF. Routes attached during the wait close after the state, by R-50.
  3. **Scope.** This covers only workers that this host spawned, because only their exit is reported. A link lost while an adopted or `Running` worker lives is candidate amendment A21, and this ruling does not decide it.
- **Evidence.** Several things are required:
  - a test where the link ends and the worker exits later: `Lost(WorkerGone)`;
  - a test where the link ends and the worker stays alive until the deadline: the R-50 `Exited` outcome, with the same reason as before;
  - N ≥ 5 real runs of `a2_3_lost_session_closes_routes_session_lost`;
  - the A5-4 table with no PASS→non-PASS.

## R-71. The form of `hold_start`: the worker's control link is held at accept, after the spawn (Stage 1 Core lead, with P6 and P3, 2026-10-11)

- **Question.** `spawn_worker` is a synchronous edge on both tiers, and the pump runs on the runner's thread. So holding AD-7 step 2 (the spawn) by blocking the edge deadlocks the release. The edge also cannot return a `ProcessIdentity` without a spawn.
- **Ruling: (a).** `hold_start` holds the worker's control link at accept, after the spawn, until `release_start`. The session stays `Starting`.
  - **Why it is faithful.** The clauses of the ids that use `hold_start` (LC-4, A2-1) need only "`Starting`, and held until release". None of them needs the hold to sit at step 2 itself.
  - **The doc.** The `hold_start` row and `start_held`'s real-tier text change to "holds the worker's control link after the spawn, at accept".
  - **The reviewer's check.** 016c checks that no `hold_start` transcript asserts that no worker process exists yet.
- **(c) is rejected.** Deferring `SpawnWorker` in Core only for testability is a product hook for a test (Testing rule 8).
- **Condition: the startup clock.** The starting worker's `startup` deadline runs on its own clock, which cannot be injected (TM-3). The worker exits by itself if no host attaches in time (AD-7). Each `hold_start` transcript must do one of these, and its PR says which:
  - release well within `startup`; or
  - open with a `startup` limit that is large enough.
- **`ad_1_starting_without_identity_is_lost`.** It needs a `Starting` row with no recorded identity. Two forms give that row:
  - `hold_start_at {before: identity}` (between steps 2 and 3), then drop the host;
  - a failed identity write at the storage edge (A5-1).
  If neither form is faithful on a tier, the id stays pending on that tier.
- **`ad_7_crash_between_steps`.** The gap between steps 2 and 3 is covered as for `ad_1`. The gap between steps 1 and 2 holds trivially, because no process exists. A transcript that needs a real crash in that gap comes back to me. No Core hook is added for it.
- **Follow-up: the startup-clock condition for two ids (P3 via the Core lead and 016c, contracts #58 and #59, 2026-10-11).**
  - **#58 (docs only) merged with the two conditions open.** Both ids stay pending until #59.
  - **`a2_1_no_deadline_code_exists`: a large `startup` is within this ruling.** A2-1's claim is that a startup timeout surfaces as `StartFailed{StartupTimeout}` through Core's injected deadline. It does not depend on the value 500.
    - `setup.limits.startup` is 30000 ms, far above the test's wall time, so the held worker's own real-clock deadline (TM-3) cannot fire during the test. A literal with its reason stated in the description is enough; no runner feature for a named value is needed.
    - Step 7's `advance_clock` is above that value, so Core's injected deadline fires, and step 8 still asserts it. Step 35's `advance_clock` (the service) is also above it, and step 36 still asserts the service's `StartupTimeout`. Each says so.
    - Step 27's `real_wait 1200` is unaffected, because s2 is `Running` by then.
  - **`ad_7_crash_between_steps`: section 1 (steps 1-13, with its own `data_dir d1` and its host a1) is removed. Sections 2 and 3 keep `startup: 500`.**
    - Section 1 was the gap between AD-7 steps 1 and 2: a `Starting` row and no worker. Under (a), a plain `hold_start` holds after the spawn, so section 1 no longer built that gap. It built a crash with a held worker alive, which is the span that section 2 covers, and its `AdoptAll` at step 11 raced the worker's own 500 ms deadline, because it never waited for the exit.
    - Core cannot tell the two gaps apart: in both, the row is `Starting` with no recorded identity, and `AdoptAll` gives `Lost(StartInterrupted)`. Section 2 (`hold_start_at {before: identity}`, `wait_worker_exit`, then `AdoptAll`) proves that path. So "the gap between steps 1 and 2 holds trivially" above is the record for that gap, and the transcript's description cites it.
    - Sections 2 and 3 need the short `startup`: they wait for the worker to exit by itself because no host attaches (AD-7). That is their claim.
  - **Owner.** P3, in #59, reviewed by 016c.

## R-70. An attach depends on the session's earlier output reaching Core, so it needs an R-57 fence, and the lint counts it as a call (Stage 1 Core lead, P3's evidence `shared/core-stage1/evidence/p3-attach/ou9-baseline-then-live-late-read-macos.log`, 2026-10-11)

- **Finding.** On the real tier, `conf::ou_9_baseline_then_live_no_gap` is flaky. The real `pty_output` returns once the probe has written to the PTY. The worker reads the PTY later, in its own process, and `await_quiet` cannot see those bytes. Under load, the worker reads `before` only after the attach has taken the baseline. Core then correctly sends it as live output (OU-9), so step 14's exact output assertion fails. A worker patch that delays the first read makes it fail 3 of 3. `ou_9_adopted_session_attaches_by_baseline` has the same latent race.
- **Ruling.**
  1. **The fence (R-57).** An OSC 2 sentinel goes at the end of the output written before the attach, and an `await_title` comes before the attach (for `ou_9_adopted`: on handle `h`, before the drop). No later expectation may change, except by the sentinel's own title or `TitleChanged`, and the PR lists each such change.
  2. **The lint (R-61(a)(1)).** An attach to S takes S's baseline at the cut, so it is "a call of S" that depends on S's output. The lint flags this shape: a `pty_output` on S, then an attach to S (`attach_route`, or an attach on any handle) before any `await_title` of S, and a later exact output assertion (R-61(c)) on that route.
  3. **The ratchet stays shrink-only.** Every transcript that the new rule flags is fenced in the same PR, unless I name it as an exception.
- **Evidence.**
  - A5-4 for both ids, with N ≥ 5 runs on macOS.
  - With the delayed-read patch, the fenced transcripts pass.
- **Owner.** P3: one HIGH contracts PR, reviewed by 016c and Sol, in `contracts-v0.1.39` by default. Until it lands, a red with this signature takes the re-gate path. It is not added to known-flaky and is not re-pended.

## R-69. The real-tier second capture of `oracle_resume` runs only at a quiet point, and the control checks this from its own poll and refuses otherwise (Stage 1 Core lead, the RC reviewer 017e's finding on Core #269 at `2a0c4922`, 2026-10-11)

- **Finding.** R-62's comparison 2 takes a second capture when the control runs. Core's `CaptureLayer::capture_now` gets the capture's `Completed` through `poll_events`, which also drains every other queued event into the layer. That is not transparent:
  - draining frees `Completed` slots and `WriteInput` counts (EV-5(a));
  - an `Activity` that has moved into the layer can no longer be retired by `RouteClosed` (EV-5, "Retirement").
  Every later step then runs against a Core that the transcript did not drive.
- **(b) is not possible for a capture.** ST-6: `begin(CaptureSnapshot)` "completes with `Capture{…}`". So its result reaches the host only as a `Completed` event, and only `poll_events` delivers events. No form of a capture avoids the poll. A route baseline (R-62's C) adds a route and route events, so it is not transparent either, and R-62 already refused C as a replacement for the anchor.
- **Ruling: (a), checked by the control itself, never assumed.**
  1. The control begins the capture, then pumps with no clock step, then polls.
  2. **If that poll returns any event other than the capture's own `Completed`,** the control's result is `unsupported_control` with the name `events_pending` (`ControlError::UnsupportedFor`). That is never a pass, and the transcript ends there, so no later step sees the drained queue. The control never re-injects or holds the drained events for later steps. **No harness model of queue retirement or admission is allowed** (the reviewer's point).
  3. **On success,** the control reads every page, then calls `release(capture)`. The open-capture count and `next_deadline` then return to what they were before the control. The capture's own `Completed` slot is freed by the poll that returned it. So the Core that the next step sees differs only by the model revision that the capture read, which changes nothing.
  4. **The transcripts.** Each of the three ids places `oracle_resume` where the transcript has already consumed every event that it causes. RC checks this in the PR body. An `events_pending` refusal in a minimum id is not a PASS, so it blocks that id.
  5. **The testkit is not affected.** R-67's comparison 2 reads `Worker::model_snapshot`, which never polls.
- **Evidence.**
  1. A negative test on the real tier: an `Activity` that is queued before `oracle_resume` gives `unsupported_control{events_pending}`, not `equal`.
  2. N ≥ 5 real runs of each of the three ids on macOS with no refusal, as R-62 requires, plus Linux at the pin gate.
- **Consistency.** This keeps the line that R-63, R-66 and R-68 hold: the harness and the driver never drain events that a transcript asserts on. Where a drain cannot be avoided, the control proves that nothing else was drained, or it refuses.
- **Follow-up: item 4 needs a fence in each transcript (the RC reviewer 017e via the Core lead, 2026-10-11). (1) is accepted. (2) is rejected.** At `contracts-v0.1.36`, `model_after…` ends its final `pty_output` with `await_quiet`, `await_title` T2, then `oracle_resume`. No step in between consumes events, and neither wait polls. The other two ids have the same shape. So the events that the output causes can still be queued, and R-69 would refuse all three ids.
  1. **(1): a waiting step that consumes the events, in each of the three transcripts, before `oracle_resume`.** It is a waiting step (R-61(a)), such as `expect_eventually` or `pump_until`, on the **last** event that the session's output causes. It consumes every event queued before that event. The owner derives the set of caused events from the clauses (for example, `Activity` for an attached route, and the title event), not from a run. A plain `drain` is not enough on its own: it does not wait, and an event that a later pump posts escapes it.
  2. **Nothing later depends on those events.** The owner checks that no step after the fence expects or forbids an event that the fence consumes. These three ids test the resume invariant, not event retention, so consuming the events changes nothing that they assert. This differs from R-63 and R-66, whose ids test retirement.
  3. **(2) is rejected.** A driver rule where waits consume events changes every transcript's semantics, and it breaks the fences of R-63, R-66 and R-68, which must not poll.
  4. **The proof is R-69's evidence:** N ≥ 5 macOS runs of each id with no `events_pending`.
- **Follow-up 2: the sentinel moves to the end in `st_6b_model_after_baseline_plus_output_equals_the_sessions_model` step 14 (016c's finding on #48 at `2f1e431c`, RC via the Core lead, 2026-10-11). Approved.** The bytes `…1b5d303b54321b5c7461696c` put `tail` after the T2 sentinel, so `await_title` does not prove that `tail` was applied. A second read of `tail` could add an `Activity` after the fence. The new bytes are `1b5b323b33486d6f7265201b5b3f32356c7461696c1b5d303b54321b5c`, the same bytes reordered.
  - **The test's meaning does not change.** An OSC title moves no cursor and writes no cell, so `tail` lands in the same cells. The capture is taken at step 10, before these bytes. `oracle_resume` compares only final states. No step asserts these bytes literally.
  - **The fence becomes sound.** With T2 last, `await_title` proves that every byte has been applied. A keyed `Activity` from any chunk is replaced in place (EV-1) before `pump_collect` polls, so step 17's three events are the whole set.
  - It is R-57's "sentinel at the end" pattern. R-62 F2's exception (a sentinel before an unfinished tail) does not apply, because `tail` is complete text. No `0a` is added (R-60).
- **Owner.** RC: Core #269 and contracts #48 (the three fences and the `events_pending` refusal in the `oracle_resume` row), merged and tagged in `contracts-v0.1.38`.

## R-68. Under R-65 a route's end reaches the client only after a host pump, so an idle route read in the conformance driver pumps the handle, without polling and without a clock step (Stage 1 Core lead, P5's source analysis, gate log `gates/botster-core-stage1-p3-flip-real-33-77816b10-pool-20261011-063918-34389.log`, 2026-10-11)

- **Finding.** On the real tier on Linux, `conf::ou_7_exit_after_last_frame_then_close` step 13 (`{"expect_closed": null, "on": "r1"}`) ends INCONCLUSIVE at `step_timeout`. Step 12, the `route_closed` frame, passes. Under R-65 the host keeps its copy of a handed-off route stream until a pump feeds the worker's acknowledgement (RouteReceived or RouteClosed). While that copy is open, the client end sees no EOF. The conformance driver pumps once per route, at the first idle read (`botster-core-conformance` `driver.rs`, `next`). Its comment says "after that the data plane needs none (Core TH-3)". So r1's EOF depends on whether a later step happens to pump before step 13. That is a race.
- **Reading.** TH-3 is unchanged and still holds: the host performs and waits for no route I/O. Holding a descriptor until the worker's acknowledgement is not route I/O. The driver's comment was a gloss on TH-3, not the clause. It no longer holds after R-65. The data bytes need no host pump. **The end of a route does:** the host closes its copy only in a pump that feeds the acknowledgement. A real host meets this, because it pumps on every wake, and the worker's acknowledgement wakes it. The driver models a host, so it must do the same. No contract changes.
- **Ruling: A.** The conformance driver's route read pumps the route's handle each time that read is idle, as a real host does on a wake.
  1. **No poll.** The pump is `core.pump(now)` only. It never calls `poll_events`, so events stay unpolled for later steps (R-63, R-66).
  2. **No clock step.** The route wait never jumps or advances an injected clock. In particular it does not use `idle_wait`'s jump to `next_deadline`. Between pumps it waits on the handle's wake handle and the route transport, bounded only by the step's deadline.
  3. **The loop.** idle read → pump → read again → if still idle, wait for a wake or the deadline → pump → read again. The step ends on a frame, an EOF or the step deadline, as today.
  4. **Every route read** that can wait goes through this loop: `expect_closed` and every other route frame read. `route_expect_refusals` keeps its own rule ("a second idle read in a row is the end").
  5. **The driver doc** (the module table and the comment in `next`) states this: under R-65 a route's end reaches the client only after the host feeds RouteReceived or RouteClosed in a pump. The data bytes need no pump.
- **B is rejected.** A fence before each `expect_closed` must be repeated in every transcript, and the next transcript can omit it. Also, `await_quiet` may not end while a report is parked. **C is rejected,** for the reasons that the Core lead gives: it reverses R-65 (B12).
- **Evidence (A5-4).**
  1. **The red-on-revert proof is required.** P5's deterministic real-tier case: a route whose RouteReceived arrives after the last pump. It gives `step_timeout` without the fix and EOF with it. This is the only evidence that closes the race, because the Linux failure does not reproduce on demand.
  2. The before/after table of the whole suite, on the testkit and on the real tier, with no PASS→non-PASS. The new pumps change when the host processes worker messages during route reads. A transcript that passed only because the host did not pump is a finding that I rule on. It is not a reason to weaken the loop.
  3. `ou_7_exit_after_last_frame_then_close`, N ≥ 5 real runs on Linux and on macOS.
- **Follow-up: the design of the wait (P5 via the Core lead, 016c copied, 2026-10-11). D1 is accepted. D2 is rejected.** Today `CoreRoutes::read` passes the step deadline to `RouteClient::read`, and Core's `RealRoute` blocks until that deadline, so no pump can run during the wait.
  1. **`RouteClient::readiness(&self) -> Option<BorrowedFd<'_>>`**, a provided method that returns `None`, for Unix only. The descriptor is the client end itself, or a duplicate of it. It is readable when the client end has bytes or its end. The change is additive: every implementer compiles unchanged. It is a patch release. No `crate-symbols` table changes: that tool maps the frozen contract texts to symbols, and the runner traits (`RouteClient`, `CoreHarness`) are not contract text (as for #30's `probe_env`). The PR body states that `crate-symbols --check` passes on the head.
  2. **A deadline that has already passed means one non-blocking attempt.** The `read` doc states this: `read` returns `Empty` at once and never an error. When an end has a readiness descriptor, the driver passes it such a deadline and does the waiting itself. `RouteDriver` keeps the step deadline. Core's `RealRoute` must honor this rule when it implements `readiness`.
  3. **The loop with a descriptor:** idle → `core.pump(now)` → read → if still idle, poll [readiness fd, wake fd] bounded by the step deadline → pump → read. It ends on a frame, an EOF or `step_timeout`. There is no `poll_events` and no `next_deadline` jump. `route_expect_refusals` keeps its own rule.
  4. **No spin.** See follow-up 3.
  5. **An end with no descriptor** (the testkit's in-memory ends, the fake): on each idle read, pump and read again. If the pump made no progress (no event posted, no `more`) and the read is still idle, return `Idle` as today. The testkit gains pumps but no new waits, and the A5-4 table shows any flip.
  6. **Until Core implements `RealRoute::readiness`** (in the pin PR), the real tier keeps the race. The red-on-revert case and the N ≥ 5 runs run on a Core scratch branch with the contracts scratch rev, as for R-59. `ou_7_exit_after_last_frame_then_close` stays real-pending. **Exposure is accepted:** `dp_2_failed_handoff_closes_route_handoff_failed` and `dp_3_frame_limit_checked_before_allocation` also wait for a route end (P3's list `shared/core-stage1/evidence/p3-flip33/route-end-wait-ids-v1-19b24d5e.txt`). They are not put on the known-flaky list. A red with this signature on these ids goes through the re-gate path, attributed to R-68.
  7. **D2 is rejected.** Passing the wake to the adapter changes `read`'s signature and breaks every implementer. D1 gives the same wait additively.
- **Follow-up 2: the wake descriptor (P5 via the Core lead, 016c copied, 2026-10-11). W1 is accepted. W2, W3 and W4 are rejected.** The workspace forbids `unsafe_code`, and `WakeHandle::fd()` returns a `RawFd`, so the driver cannot poll it safely.
  1. **`CoreHarness::wake_fd(&self, handle: &str) -> Option<OwnedFd>`**, a provided method that returns `None`, for Unix only. The harness knows the handle's name from `OpenSpec.handle`. It returns a duplicate of **the same descriptor** that `WakeHandle::fd()` of that handle's Core gives, so the TM-6 level signal is readable on it and a pump that clears the signal clears it there too. A separate notifier is not allowed.
  2. **The current Core only.** After a drop or a reopen under the same name, `wake_fd` gives the new Core's descriptor, or `None` while no Core is open. The driver asks for it at the start of each route wait and never keeps it across steps.
  3. **One harness test on the real tier:** `wake_fd` is readable after a call that leaves runnable work (TM-6), and not readable after a pump that leaves none.
  4. The driver polls [route readiness, `wake_fd`] only when both are `Some`. Otherwise it takes the path for an end with no descriptor (the follow-up's item 5).
  5. **Rejected.**
     - W2 (a `WakeHandle` method) changes the frozen contract crate's API, so it needs an amendment.
     - W3 (a thread that blocks in `wait`) adds a second waiter that can outlive the step.
     - W4 (a lint exception) weakens `forbid` for the whole crate.
- **Follow-up 3: how a spin is reported (P5 and 016c via the Core lead, 2026-10-11). P2 is accepted, with a diagnostic. P1 is deferred to the next minor release.** The route read reaches `order.rs` only as a `Poll`, which has no variant for "the driver's own bound ended the wait". Under a real deadline, `poll_limit` is `usize::MAX`.
  1. **The bound counts only passes that make no progress.** A pass makes no progress when the poll said readable, but the pump posted no event and set no `more`, and the read still gave `Empty`. Any progress resets the count. The bound is a fixed constant (P5 picks it, on the order of 1,000), not `usize::MAX`. A legitimate wait never reaches it.
  2. **At the bound, the loop returns `Poll::TimedOut`.** It also writes one diagnostic line that names the spin and which descriptor stayed readable (route readiness or wake), with the handle and the route. The line goes where the step's inconclusive report goes, so a gate log shows the cause next to `step_timeout`. A test drives a descriptor that stays readable and checks that the step ends inconclusive with that line, before `step_timeout`.
  3. **Why not P1 now.** A new variant of a public enum that is not `#[non_exhaustive]` is a minor release. Under the tag process, that goes to the orchestrator, and it moves every consumer's pin from 0.1 to 0.2 in the middle of the agreed pin order. The label alone does not justify that. P2 with the line keeps the defect visible.
  4. **Deferred.** `Poll::Exhausted` (mapped to `Inconclusive{limit: "max_polls"}`) and `#[non_exhaustive]` on `Poll` go on the list for the next minor release. They then replace the line.
  5. **016c's other points are confirmed:**
     - the pump goes to the route's own handle and generation;
     - the `RefCell` borrow is released before a pump;
     - `EINTR` recomputes the remaining time;
     - a poll error ends the step;
     - the wake duplicate is held only for the wait, and is never read or drained.
- **Owner.** P5, through the Core lead: the contracts PR #47 (the driver and its doc), reviewed by 016c and 0167, with Sol as the integration reviewer. `ou_7_exit_after_last_frame_then_close` stays real-pending until it lands.

## R-67. The testkit form of `oracle_resume` uses the real form's two comparisons, with O_raw from the testkit's program-edge log (Stage 1 Core lead, RC's R-62 Core PR on base `ddd3b849`, 2026-10-11)

- **Question.** `docs/core-testkit-controls.md`, "The testkit form", says: "O_resume (below) is compared with the worker's live model, through the comparator." The comparator works on oracle terminals. On the live model it would change the subject: DECRC (F3b) moves the model's cursor, and the cursor-style read updates its render state. A direct read would also need a new accessor in `botster-worker-core`. RC proposes that the testkit use the real tier's two comparisons:
  1. O_resume equals O_raw, where O_raw applies the testkit's `ModelLog` output (the bytes that the model read);
  2. the live model's snapshot (`Worker::model_snapshot`), decoded into an oracle terminal, equals O_raw.
- **Ruling: accepted. It meets R-62, and it replaces the sentence in the controls doc.**
  - **R-62's aim is met.** The expected side never passes through the subject's encoder. Comparison 2 runs the encoder once, on the subject's side, against an independent anchor, so an encoder that drops a field fails. This is the real tier's comparison 2. Today's testkit check runs both sides through the encoder, so the new form is stronger.
  - **The anchor is harness-owned.** I read `ddd3b849`. The testkit's worker driver appends each program read to `ModelLog` (`crates/botster-core-testkit/src/worker.rs:1417`), **before** it hands `Input::PtyOutput` to the worker machine. So O_raw comes from the program edge, not from the worker's model code. A worker that loses bytes after the edge fails comparison 2. **Condition:** `ModelLog` stays filled at the program edge, by the testkit, never by `botster-worker-core`.
  - **The comparator touches only oracle terminals** on both tiers: O_raw, O_resume and the decoded snapshot. R-62's "one comparator serves both tiers" now holds literally.
  - **A direct comparison with the live model is refused.** It changes the subject that it measures (F3b), and it needs a test accessor in the worker crate (Testing rule 8, as in R-62 Q1).
- **The testkit differs from the real tier in two places only.**
  1. O_raw is `ModelLog`'s output, not the transcript's `pty_output` bytes. So the testkit needs no `unknown_output_history` refusal: `ModelLog` holds every byte that the program wrote. R-62 Q3 (no `0a`) still applies to the transcript, because the real tier reads the same transcript.
  2. Comparison 2 reads `Worker::model_snapshot`, which already exists, instead of a second `CaptureSnapshot`. It must read the model after the model has applied every byte in `ModelLog`: today's `model_rev` suffix rule.
- **Evidence (with R-62's).** The two existing negative tests still give `equal: false`: `log_unapplied_output` (the log has bytes that the model lacks) and `apply_unlogged_output` (the model has bytes that the log lacks). Add R-62's negative test of a capture with one field removed, through comparison 1 and through comparison 2.
- **The doc text.** RC's contracts PR (docs only; the crate reviewer reviews) replaces the paragraph "**The testkit form.**" in `docs/core-testkit-controls.md` with this text: "**The testkit form (R-67).** The same two comparisons as the real tier, with two differences. O_raw applies the output that the testkit logged at the program edge (`ModelLog`), so the testkit needs no `unknown_output_history` refusal. Comparison 2 decodes the worker's `model_snapshot`, read after the model has applied every logged byte, instead of a second capture. The comparator never touches the live model." The ruling is in force now. The Core PR does not wait for the doc PR.

## R-66. `ev_5_route_closed_retires_route_keyed_events` needs a fence that does not poll: a read-only `route_reports` control (Stage 1 Core lead, P3's evidence `shared/core-stage1/evidence/p3-ev5-route-closed/README.md`, 2026-10-11)

- **The clause.** Section 6, "Retirement": "`RouteClosed` retires every unpolled route-keyed K event of that route". The transcript must therefore bring about one state: r2's `Activity` is posted, it is **still unpolled** when `RouteClosed{r2}` is posted, and then nothing of it is seen.
- **Finding.** On the real tier the step-29 `expect_quiet_until` polls before Core handles r2's `RouteClosed`. Neither `await_quiet` can see the worker's queued report or its pending answer to the `Detach`.
- **Why the proposed fence is wrong.** A wait on `Completed{$d2}` or on `RouteClosed{r2}` with `pump_until` **polls**. If the `Activity` is posted in an earlier pump than the `RouteClosed`, that wait takes the `Activity` and discards it, and step 29 then passes vacuously: nothing is left to retire. Moving the barrier of `expect_quiet_until` to the `Detach` fails in the same way, because it polls throughout. EV-5 allows a client to see an `Activity` that was polled before the close. So the fence must make Core process the close **without polling in between**.
- **Ruling: a read-only control at the link boundary, and a fence on it.**
  1. **`route_reports {session, route}` returns `{activity_before_close, closed}`.**
     - `closed` is true once Core has **consumed** the worker's close report for that route.
     - `activity_before_close` is true when Core consumed a client-activity report for that route before that close report.
     - It is a read-only observation at the boundary, and it adds no synchronization point (R-14.3).
     - On the real tier it comes from the tap on the control link. Core reads a link frame inside a `pump`, and the budget is checked between frames (Core's statement for R-57). So a consumed frame is a processed frame, and the `RouteClosed` it causes has been posted. If a budget can carry the post to the next pump, the control reports `closed` only after the post.
     - On the testkit it comes from the in-process edge at the same point.
     - The control names no message type of the link, which is Core-internal (R-65).
  2. **The transcript.** Replace step 28's `await_quiet` with `{"await_control": {"control": {"op": "route_reports", "session": "s1", "route": "$r2"}, "expect": {"closed": true}}}`. `await_control` pumps and never polls (`driver.rs`: "pumps (no poll, no clock step)"). Then add a step with `control route_reports` and `expect {activity_before_close: true, closed: true}`. That proves the precondition: an `Activity` existed to be retired, which keeps the test from being vacuous. Step 29 then polls and finds no `Activity` of r2. Step 26's `await_quiet` may stay. *(Superseded by the follow-up below (017f on #69, verdict `d8ed9341`): step 26, transcript index 25, becomes the `route_reports {activity_before_close: true, closed: false}` fence. R-66's step numbers count from 1.)*
- **Owner.** P3: one contracts PR (the control row, the vocabulary and the transcript edit, as for #40) and the Core control in a control-only Core PR, by the refined pin order. Evidence: A5-4 on the testkit and macOS (N ≥ 5), with Linux on the pin gate.
- **Correction (P3 via the Core lead, 2026-10-11): `closed` means "consumed at the link edge", not "posted".** Item 1 above said that a consumed close report means its `RouteClosed` has been posted. That is not true in general. P3 found two cases that the edge cannot see:
  - a `RouteClosed` with reason `SessionEnded` is posted later (OU-7, `route_delivered`);
  - `route_close` parks a close behind parked route work (a start step parks its closes), so the post can come in a later pump.
  1. **Definition.** The controls doc defines `closed` exactly as the edge sees it: "Core has consumed the worker's close report for this route". It states that this does not imply that `RouteClosed` has been posted, and it names the two cases.
  2. **Scope of the fence.** A transcript may use `closed: true` as a fence for the post only where the clauses make the post immediate when the report is consumed. That requires all of these:
     - the close reason is not `SessionEnded`;
     - no route work of the session is parked;
     - the event budget can carry the post.
     A transcript that uses it as a post fence in any other case is a defect, never a pass. The crate reviewer checks this.
  3. **`ev_5_route_closed_retires_route_keyed_events` is within scope.** The route is Detached, the session is Running, nothing is parked, and the default `pump_events` applies. An early `closed` there can only make step 29 see the `Activity` and fail. It cannot make the step pass falsely.
  4. **No engine signal now.** If a later transcript needs a general "close posted" fence, a separate PR adds an engine-side field (for example, a pending-close count). It is not part of `contracts-v0.1.38`.
- **Follow-up (RC's real-tier race in Core #285's final gate `3cf46071`, via the Core lead, 2026-10-11): `ev_5_route_closed_retires_route_keyed_events` also fences the client input before the `Detach`.** Step 26's `await_quiet` cannot see the worker's unwritten client-activity report (R-43 (8)). So the `Detach` can overtake that report, and step 29 then reads `activity_before_close: false`. Step 26 becomes `await_control {route_reports, session: s1, route: $r2} expect {activity_before_close: true, closed: false}`. The `route_reports` row already defines `activity_before_close` as "before now, while no close report is consumed", so no new control is needed. The wait pumps and never polls, so r2's `Activity` stays unpolled for the retirement that the test asserts. Owner: RC (it is RC's gate), in one contracts transcript PR, reviewed by 016c.

## R-65. "Core closes the host's copy once the worker has its own" means after the worker confirms it, so Core holds the copy until the worker's acknowledgement (Stage 1 Core lead, P3's diagnosis `shared/core-stage1/evidence/p3-route-eof-inflight/README.md`, 2026-10-11)

- **Finding.** On macOS, under load, about 15 real-tier ids get `attached` and then EOF. The worker's first read of the route returns 0, and it closes the route `PeerClosed`. A standalone repro shows the cause. A socket sent with `SCM_RIGHTS` whose sender closes its copy before the receive can lose its receive side: 304 in 1.3M cycles, and 66,481 in 710k under load. When the sender keeps its copy until the receive, there are 0 in 2.1M. This is a third-party kernel bug, recorded as bug catalog B12 and not reported upstream. Core drops its copy right after `send_with_fd`.
- **Reading.** DP-2: "Core closes the host's copy **once the worker has its own**". "Has its own" is a fact about the worker, and Core can know it only when the worker says so. A copy that is still in flight is not yet the worker's own. So closing right after the send closes too early under DP-2 itself, apart from the macOS bug. And DP-2 already requires the order that cures the bug.
- **Q1: option (a) is accepted: a worker-to-host acknowledgement that the worker has received the descriptor.** Core keeps its copy until that acknowledgement, then closes it.
  - **No Core amendment is needed.** DP-2 makes the handoff path a mechanism note ("over the worker's private control link"). The link's message set is Core's internal protocol, between Core's own host library and Core's own worker. The contract fixes only the order, and this message implements that order.
  - **Protocol 1 may gain the message, on one condition:** no released Core worker binary speaks protocol 1. AD-4 and A6-2 tie a protocol number to released workers, and no Core release exists yet (cold cut). The Core lead confirms this in the PR. If a released protocol-1 worker exists, the message needs protocol 2.
  - **Failure paths.** If the link fails before the acknowledgement, or the worker refuses the descriptor, the route closes `HandoffFailed`, and Core closes its copy (DP-2). An acknowledgement that never comes is bounded by the link: a lost link ends it. Core adds no new timer, because the existing handoff failure paths apply.
- **Q2: option (b) is rejected.** Holding the host's copy for the route's whole life is not "once the worker has its own". It also breaks the route's own close. While the host holds a copy of the same endpoint, the client sees no EOF when the worker closes or dies, until the host processes `RouteClosed`. That is a data-path effect of the host, against DP-1.
- **Tests.** `conf::dp_2_stream_handoff_transfers_ownership_and_closes_host_copy` (a minimum id) keeps its meaning. If it reads "closed" before the acknowledgement, it needs a fence on the acknowledgement's effect, by R-56's rule (a wait on the real event). P3 checks this and brings any transcript change to me. Core adds a real-process test: a sender that closes before the acknowledgement is not possible any more. Linux evidence comes from the pool job that P3 requested.
- **Follow-up: a link lost before the acknowledgement (P3's design for Core #243, 2026-10-11).** When the link is lost while an endpoint is still unacknowledged, Core drops its copy and treats the route as a handoff that did not complete. Accepted:
  - **`Starting` or `Stopping` session:** the link loss ends the session (a failed start, or the end of the stop), so R-50 governs. The route closes after the state, with the reason that the end state gives (`SessionEnded{exit}` or `SessionLost`). OU-2's first-reason rule holds, because the session's end is the cause.
  - **`Running` session:** the session stays `Running` with a broken link (`conf::lc_5_stop_with_broken_control`, R-64). The unacknowledged route closes `HandoffFailed`, and Core closes its copy.
  - **Caveat:** in the `Running` case the worker may in fact have received the descriptor, so that only the acknowledgement was lost, and it may serve the client. That is the situation of candidate A21 (R-64), which may refine this case. Until A21, `HandoffFailed` is the reading, because Core cannot confirm the handoff.

## R-64. A worker link lost while the worker lives: the contract has no truthful close for its routes, so this goes to an amendment (Stage 1 Core lead, P3's `ev_5` diagnosis, 2026-10-11)

- **The `r5` fact (R-63's open item) is settled. It is not a defect.** On v1 the worker cannot yet receive descriptors (Core #240 adds this), so the first `AttachRoute` closes the link. `on_link_closed` then closes `r5`, a pending handoff, with `HandoffFailed` (R-50), and posts its `RouteClosed`. A `Detach` of a route that is no longer live is `UnknownRoute` (the A2-1 row: "any live route"). OU-2 holds. This is a v1-only trigger, and #240 removes it.
- **The `d1` fact is a defect under AM-3** ("exactly one completion per admitted operation, including when … the worker link fails"). A `Detach` that was sent to the worker before the link closed never completes. That holds on every tier, whatever the trigger.
- **But the contract does not say how it completes.**
  1. The A2-1 row for `Detach` has **no async error**. It completes `()` "after `RouteClosed` is posted" (DP-7). So `Err(WorkerLinkFailed)` would break the row.
  2. So a `RouteClosed` must come first, and its reason must be from A2-3's closed set. **None of them is true** for a delivered route whose worker still lives:
     - `Detached` claims that the worker wrote `Closed` and closed the transport (DP-7), and it did not.
     - `SessionLost` needs an `Exited` or `Lost` session.
     - `HandoffFailed` needs a failed handoff.
     - `PeerClosed` and `WriteFailed` need a fault on the route's transport.
     The client may still be connected and served (DP-8).
  3. Reading the session as `Lost(WorkerUnreachable)`, which would make `SessionLost` true, is contradicted by the frozen transcript `conf::lc_5_stop_with_broken_control`. After `break_control`, that transcript still admits `ReadScreen` (which is `WrongState` for `Lost`) and expects `Stopping` after `Stop`. So a session with a broken link stays `Running`.
- **Ruling.** A ruling cannot pick a reason that the clauses make false, and it cannot add one. **Q1 (the reason and the completion) and Q2 (every delivered route, or only those with a waiting op) go to a Core amendment**, candidate A21. The steward drafts it and the Amendment reviewer reviews it. The draft will settle:
  - what the handle does with every delivered route of a session whose link is lost while the worker lives (Q2);
  - the host event and reason, and how A2-3 maps it, or why no wire reason is sent, because the client may still be served;
  - the `Detach` completion form;
  - how a later `Adopt` gives the routes back (DP-8, `RouteAdopted`).
- **Until then.** Core does not code a guess. After #240 the v1 trigger is gone, so `ev_5` does not meet this path unless a real link fails. R-63's fence stands. A transcript of the link-loss case comes with A21.

## R-63. `ev_5_repeated_attach_detach_without_polling_stays_bounded`: the recovery polls the completions it needs, with a wait (Stage 1 Core lead, PC's Mac trace, 2026-10-11)

- **Finding.** Detaches `d1` to `d4` hold the 4 `pending_ops` slots until their `Completed` is polled (EV-5). Step 19's `PendingLimit` is correct whether or not those completions are queued yet: an unpolled op holds its slot either way. The recovery is the problem. Step 20's `poll_all` polls only what is already queued. On the real tier the worker's replies may not have arrived, and `await_quiet` does not wait for a reply that has not been sent (R-43 item 8). So in the failing seeds nothing was queued at step 20, no slot freed, and step 22 got `PendingLimit`.
- **Ruling: a fence, in the form the clause names.** EV-5's recovery is "polling the completion frees the slot". The transcript should poll the completions that it means to free. A `pump_collect` of `Completed{$d1}` … `Completed{$d4}` (ok, unit; `ignore` the routes' `RouteClosed` and `Activity` events) goes **before step 20**. It waits on the real event, because the worker's reply wakes it, and it takes exactly those completions. Steps 19 to 23 stay as they are. `poll_all` then has nothing left to free, and it is harmless.
- **The second fact.** In all 472 traced seeds, `d1` never completes and route 1 never posts `RouteClosed` on the real tier. OU-2 gives every route exactly one `RouteClosed`, and A2-1 gives every admitted op exactly one `Completed`. A route detached before its handoff is no exception (R-50: the holder of the transport owes its close). So the fenced id fails at the new `pump_collect` until that is fixed. That is the correct signal: today the flake hides it. If Core's diagnosis finds a clause that lets `d1` stay open, the question comes back to me.
- **Evidence (A5-4).** The before/after of `ev_5` on the testkit and the real tier (macOS, under load as in PC's trace). Owner: PC, through the Core lead. The crate reviewer reviews it.
- **Correction to the finding (PC's reviewer, all 90 raw files).** The failures on the old pin (1/30) and the new pin (2/30) are `UnknownRoute` at the step 22 retry, not `PendingLimit`. `PendingLimit` occurred only in the high-load run `df991fdf` (7 of 30, plus 6 `UnknownRoute`). All 16 failing runs had `pending_ops` 4 and `queued_events` 0 at step 20.
  - **The fence stands.** With 4 slots held and nothing queued, a retry can pass only after the completions arrive and are polled. The fence is needed whatever the retry's error is.
  - **`UnknownRoute` for `r5` is a separate open fact.** Step 19's refusal had no effect (AM-4), and nothing detached `r5`, so `r5` should still be known at step 22. If Core forgot `r5` without posting its `RouteClosed`, that breaks OU-2's one `RouteClosed` per route, and it belongs to the same route-lifecycle diagnosis as `d1`. If `r5` closed for a reason of its own and its `RouteClosed` was pending, the transcript reads it wrongly, and the question comes back to me with the trace. Core's diagnosis decides which.

## R-62. The real form of `oracle_resume`: two comparisons against an oracle fed the raw output, with no snapshot encoder on the expected side (Stage 1 Core lead, RC, with P6 and P3, 2026-10-11)

- **Question.** The testkit's `oracle_resume {session, capture} → {equal}` compares a fresh oracle terminal with the in-process worker's live model. On the real tier the model lives in the worker process. Candidates:
  - A: a second capture, decoded, as "the session's model";
  - B: a worker-side read of the live model;
  - C: a new route's baseline.
  A and C read the model through the subject's own snapshot encoder, which also made the first capture. An encoder that drops a piece of state (an attribute, a mode, the saved cursor, pending parser input) drops it from both sides, and the comparison says `equal: true`.
- **Q1: B is refused.** A model read in the shipped worker is a product hook for a test (Testing rule 8). A test-only worker build would put a second binary on the real tier, which then no longer tests the shipped worker (Core A20, rejected 2).
- **Q1 and Q2: A is accepted only with an independent anchor.** The expected side never passes through the subject's encoder. The anchor is **O_raw**: a fresh oracle terminal of the session's size and color profile that applies, from the session's start, **every output byte that the program wrote**. These are the transcript's own `pty_output` bytes, in order, and they are known without asking the subject. libghostty, pinned, is the oracle (R-7). The real form of `oracle_resume` returns `equal: true` only if both comparisons hold:
  1. **The resume invariant:** O_resume equals O_raw. O_resume is an oracle that loads the first capture's pages, then applies every output byte after the capture's cut. If the capture dropped any state, O_resume diverges from O_raw.
  2. **The session's model:** a second capture, taken at the end after a fence (R-57 `await_title`), decoded into an oracle, equals O_raw. This ties O_raw to the worker's actual model. An encoder drop fails here as well, because O_raw holds the state that the encoder dropped.
  **The comparator** compares oracle terminals through libghostty's own state, field by field: cells and attributes, hyperlinks, cursor, saved cursor, tab stops, margins, modes, charsets, palette and dynamic colors, title and cwd. **It never runs the subject's snapshot encoder.** The testkit uses the same comparator, so one comparator serves both tiers. C may be added as a third reading, but it does not replace the anchor.
- **Conditions.**
  1. **The output history must be known.** An id whose program writes output that the transcript does not state (a scripted print, an echo, a query reply rendered by the program) cannot use the real form, unless that output is in the history. The three ids have `hold` programs, so their output is exactly their `pty_output` bytes. The driver refuses `oracle_resume` on the real tier for any other program, with `unsupported_control{unknown_output_history}`.
  2. **The cut.** "The bytes after the cut" are the `pty_output` bytes that the transcript sends after the capture step, and the capture follows a fence on the earlier output. A wrong cut can only make O_resume differ from O_raw. That is a false failure, never a false pass, so soundness holds. Reliability is the evidence: N real runs, and N ≥ 5 on macOS.
  3. **Size and profile.** O_raw and O_resume use the size and the `ColorProfile` that the transcript set. A transcript that resizes or changes the profile during the history needs its own ruling.
- **Q3: R-60's rule covers these bytes.** ST-6b's "every `output` byte after R" is the output as the model receives it, after the line discipline. R-48 keeps `ONLCR`, so a `0a` reaches the model as `0d0a` on the real tier and as `0a` on the testkit. The oracle never models the line discipline, because a tier-dependent oracle would make the two tiers prove different things. Instead, R-60 Q2 extends: the `pty_output` bytes of a session that a later `oracle_resume` reads contain no `0a`, and no other byte that `OPOST` changes. Then the oracle applies the bytes unchanged on both tiers. `st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut` has one `0a`. It becomes `1b44` (IND), which has the same effect on the model when line-feed/newline mode is off, so the test of margins is unchanged. The R-56 lint (PR B) adds this to R-61(c)'s exact-assertion class.
- **Evidence (A5-4).** The before/after of the three ids on the testkit and the real tier, plus a negative test of the real form: a capture with one field removed (for example, the saved cursor) gives `equal: false` through comparison 1, and through comparison 2.
- **Owner.** RC, through the Core lead. The contracts PR carries the controls-doc row, the comparator and the IND edit. Core implements the real form in its harness.
- **Follow-up (RC via the Core lead, 2026-10-11).**
  - **F1, three `0a`, not one.** My count was wrong. Step 12 of `st_6b_saved_cursor…` has three. Each becomes `1b44`, so step 12 is `1b3878096c716b1b441b441b441b284279`. With LNM off, IND has LF's effect on the model. `09` is unchanged, because R-48's termios keeps `TAB0`.
  - **F2, the fence is a transcript change.** Condition 1 puts the whole output history in the transcript, so a harness-internal write is not allowed. Each capture follows an `await_title` on a sentinel title in that session's `pty_output` bytes (R-57). Where a title at the end would change what the test cuts (the cut inside an SGR sequence), the sentinel goes **before** the unfinished tail, in the same `pty_output`. The residual risk is that the tail is not yet parsed. That gives only a false failure (condition 2), and the N ≥ 5 runs measure it. `model_after…`'s existing titles T1 and T2 serve as sentinels where they are the last title before a capture. RC proposes the exact bytes and steps in the PR body, and I approve them there. The crate reviewer checks that no other expectation changes, except the comparison of the sentinel title itself.
  - **F3a, tab stops, margins and charsets through libghostty's formatter: accepted,** because the formatter is libghostty's code, not the subject's GHOSTSNP encoder. **Condition:** if Core's GHOSTSNP encoder calls that formatter for any of these fields, the formatter is shared code with the subject, and the field needs another reading. RC states in the PR which is true.
  - **F3b, the saved cursor: (ii) is accepted.** The comparator restores the saved cursor (`1b38`, DECRC) on each oracle terminal, last, or on clones. It then compares the cursor position, the cursor style, the pen (rendition), the charsets and origin mode, all of which DECRC restores. It touches only harness-owned oracles, never the subject. (i), a fork patch, is not needed.
  - **A comparator-completeness test (new, required).** For each field in the list, two oracle terminals that differ **only** in that field must compare unequal. This catches a comparator, or formatter, that does not see a field, which is the same blind spot that the anchor closes on the subject's side.
  - **F4: correct.** Contracts holds the specification: the field list, the two comparisons, the completeness test and the refusal, in the `oracle_resume` row of `docs/core-testkit-controls.md`. The code lives in the Core testkit, and both tiers share it. Contracts stays free of libghostty (BUILD.md).
- **Follow-up: the probe's own prints** (P3's note `shared/core-stage1/design-unknown-output-history.md`, via the Core lead, 2026-10-11).
  - **Q1: a print that the probe reports counts as "in the history" (condition 1).** The probe is the harness's program, not the subject, so the anchor stays independent of the subject's code. **Conditions:**
    1. The probe sends all its stdout through its one writer, and it reports each print (`Printed{bytes, after}`) and waits for the harness's answer **before** it writes the print. So the position comes from the harness's own write order, never from the subject.
    2. The harness checks that the reported bytes equal the print bytes that the transcript's program script states. A mismatch is a harness error, never a pass.
    3. Conditions 2 and 3, and Q3's rule (no `0a`, and no other byte that `OPOST` changes), apply to the print bytes too.
  - **Q2: `a2_1_read_screen_history_flag_and_text` needs a transcript edit, as in F1, not its own ruling.** Each `0a` in its prints becomes `1b44` (IND), so `0d0a` becomes `0d1b44`. With LNM off, CR then IND has the effect of CR LF on the model, and also at the bottom margin, where IND scrolls into the history. So the expected screen text and history are unchanged. The crate reviewer checks that no other expectation changes.
  - **Owner.** P3: the contracts PR (the probe, the probe script, the `a2_1` edit), naming the Core shared-testkit PR that serves it, which is HIGH.

## R-61. Definitions for the fence and line-discipline lint (R-56, R-57, R-60) (Stage 1 Core lead, for PC's PR B and PR C, 2026-10-11)

- **(a) "A later step depends on that output reaching Core" (R-57 sweep, PR C).** A step that **waits** for an effect is its own fence when that effect can come only from the output: `pump_until`, `pump_collect`, `expect_eventually`, `route_expect_output` and the `await_*` steps pump until the wanted value appears, and the worker's frame wakes them. They need no `await_title`. The race exists in two shapes, and only these are flagged. A `pty_output` P on session S needs an `await_title` fence (or a named exception) when the first later step that reads S's events or state, before any `await_title` of S, is one of the following:
  1. **A step that does not wait:** `expect` on S's events, `drain`, `forbid_events`, `expect_quiet_until`, a sync read or `call` of S, or a control read of S.
  2. **A wait whose match an earlier state can also meet:** an expected value that P extends instead of creating. Mechanically, this is an `EventsLost` expectation (its `kinds` set grows; this is `ev_2`'s shape). Any other aggregate is the reviewer's check, not the lint's.
  So a `pty_output` that only a waiting step on a new value follows is not flagged. If this definition still puts most of the 521 steps on the list, PC sends me the counts per shape before PR C, and I narrow it.
- **(b) A write to a session (R-56).** PC's list is confirmed: a `begin` of a write op that names the session (`WriteInput`), and a `route_send` or `route_send_raw` on a route that is attached to it. `pty_chunk` and `program_write_once` do not count: they are program output. **One addition:** a `pty_output` that contains a terminal query can make the worker write a reply into the session's input (EV-8: the shadow, or a client). Mechanically, a `pty_output` whose bytes contain `1b` counts as a write, and a named exception may clear one that has no query.
- **(c) An exact output assertion (R-60 Q2).** PC's list, completed. It is any step that compares raw PTY output bytes literally:
  - `route_expect_output.bytes_hex` (and the `bytes_hex` parts of `parts`);
  - an `expect` or `expect_eventually` of an `output` or `output_deflate` frame with literal bytes;
  - an expectation of a `Tap` event's bytes (TP-1).
  `route_expect_pty` and `pty_input` are the **input** direction, which `OPOST` does not touch. They have a mirror rule: R-48 keeps `ICRNL`, so the line discipline turns an input `0d` into `0a`. **An exact input-log assertion (`pty_input` and `route_expect_pty` literals) contains no `0d`.** No transcript has one at `27f62d3`, so this rule lands with no exceptions.

## R-60. Input to a probe waits for the probe's terminal setup (`program_ready`), and no exact output assertion contains a byte that `OPOST` changes (Stage 1 Core lead, RC's echo diagnosis `gates/botster-core-rc-echo-diag-ou7-mac-20261011-003027.log`, 2026-10-11)

- **Finding.** In `ou_7_exit_after_last_frame_then_close` step 11 the real tier reads `78 68656c6c6f 0d0a` where `68656c6c6f 0a` is expected, 3 of 3 on macOS. With a program control placed before the `route_send`, it reads `68656c6c6f 0d0a`, 3 of 3. Two separate causes:
  1. **Echo, a race.** `Start` completes and the route reaches `live` before the probe has cleared `ECHO` (R-48). The kernel echoes the route's input byte `78`. No step orders the first input after the probe's setup. `ou_2b_healthy_reasons_send_route_closed_last` step 45 has the same race.
  2. **`ONLCR`, not a race.** R-48 keeps `OPOST`/`ONLCR` on purpose, so the PTY turns `0a` into `0d0a`. The testkit does not model output processing, so a transcript that asserts a bare `0a` can pass only on the testkit.
- **Q1 ruling: a fence, named for what it proves.**
  1. **A new read-only control, `program_ready {session}`, returns `{ready}`.** `ready` is true once the session's probe has finished its R-48 terminal setup. On the real tier, that is when the probe has connected its control socket, and the harness wakes every open handle when it connects, as for `Input` reports. On the testkit it is true at once.
  2. **A probe guarantee, with a test:** the probe connects its control socket only after its R-48 setup is complete, or immediately when stdin is not a terminal. A probe test on a real PTY checks that `ECHO` and `ICANON` are already off when the connection arrives.
  3. **The rule.** A transcript waits with `await_control {op: program_ready, session} expect {ready: true}` before the first input to a probe session (host `WriteInput` or route input), when a later step asserts the program's output or its input log.
  4. **Rejected:** RC's candidate, an unrelated program control whose wait happens to follow the probe's connection. It works only by a side effect of the harness. A transcript reader would not see why it is there, and a change in that control's form would silently remove the fence.
- **Q2 ruling: the transcripts change, and R-48's `ONLCR` stays.** R-48's reason stands: the payload owns its line discipline, and a real program sees `ONLCR`. Turning off `OPOST` in the probe would make the real tier less like a real terminal, only to suit 3 transcripts.
  - **The rule** (it extends R-48's "transcripts must not rely on PTY echo or on canonical line buffering"): a transcript must not rely on output processing either. **An exact output assertion contains no `0a`**, and no other byte that `OPOST` changes. The clauses that these transcripts prove (OU-7, DP-3) do not depend on a newline.
  - **The ids, at `72b95de`:** `ou_7_exit_after_last_frame_then_close`, `dp_3_output_deflate_frames_share_no_state` and `dp_3_output_deflate_inflates_to_the_exact_pty_bytes_in_order`. Each one replaces `0a` in the program's bytes and in the expected bytes with a byte that `OPOST` leaves unchanged (for example `2e`), consistently. No other expectation changes.
- **The sweep.** The R-56 ratchet lint (PR B) also checks:
  - the `program_ready` rule: the current gaps go on its shrink-only exception list;
  - no `0a` in an exact output assertion: no exceptions, because the 3 ids are fixed in this PR.
- **Evidence (A5-4).** The before/after for the 3 changed ids, plus `ou_2b_healthy_reasons_send_route_closed_last`, on the testkit and the real tier (macOS; Linux pending Core's pin-move gate, on the #32 terms).
- **FakeCore (RC's question, 2026-10-11).** `ou_7_exit_after_last_frame_then_close` runs in full on FakeCore, which has no `program_ready`. BUILD.md: the frozen fakes "stay unchanged, with no new fidelity, until their real counterparts pass the same suites". So FakeCore gets no new control arm, even a trivial one. `ou_7_exit_after_last_frame_then_close` is marked `"fake": "structural"`, the existing form for an id that the frozen fake cannot run. That is the one metadata edit that this ruling adds. The id's full proof is the testkit and the real tier (Core A5-4), and FakeCore is due for deletion.
- **Owner.** RC, who diagnosed it. The crate reviewer reviews it. Core implements `program_ready` on its real harness and its testkit in a separate Core PR.

## R-59. A start hold must be proven to have met its step: `start_held`, and a fence on it in `ad_7` (Stage 1 Core lead, P5's Linux evidence `shared/core-stage1/evidence/p5-hold-debug-3359197d-ad7-linux.log`, 2026-10-11)

- **Finding.** On the real tier, in every seed, the order is: the hold is armed, the hold is released, then the worker's hello arrives, and Core sends `Launch`. `await_quiet` returns while the worker process is still starting, because a starting worker is not yet a readable edge, so nothing wakes or blocks the wait (R-43 item 8). The hold therefore never meets a `Launch`. The steps that should run "while the step is held" run before the step exists. The ids still pass, but they prove a crash at an earlier point than the one they name.
- **Ruling: the current form is not accepted.** `hold_start_at(before)` names a point in AD-7's order ("the row and the worker identity are durable and no payload runs"). A transcript that crashes "at" that point must prove that Start has reached it. `await_quiet` proves only that no edge holds an unconsumed report, and a starting worker holds none. The fence follows R-56 and R-57: a wait that ends on the real event.
- **The fence.**
  1. **A new read-only control, `start_held {session}`, returns `{held}`.** `held` is true when the hold of `hold_start_at` (or `hold_start`) has **stopped its step**: the step was due and is held. It is false while the hold is only armed. It is a read at the boundary that adds no synchronization point (R-14.3).
     - Real tier, `payload`: true once the tap holds the first byte of that session's `Launch` frame (R-49 Q2).
     - Real tier, `identity` and `running`, and `hold_start`: true once the held edge call has been made and is blocked.
     - Testkit: true once the scheduler has the held step due.
  2. **`ad_7` puts `await_control {op: start_held, session} expect {held: true}` after each `begin Start` that is under a hold**, before `host_crash`. The wake source is the worker's hello on the control link: Core reads it in a pump, and the next step reaches the hold. The existing `await_quiet` steps may stay.
  3. **Nothing else in `ad_7` changes.**
- **Evidence (A5-4).** The before/after for `ad_7` on the testkit and the real tier (Linux and macOS). On the real tier, the log shows that the hold meets its step (`held: true` before `host_crash`) in every run.
- **The same check elsewhere.** Every transcript that uses `hold_start` or `hold_start_at` and then acts "while held" needs the same fence. The PR lists them, with the fence added or a reason why the step is already certain (for example, a `pump_until` on an event that only the held point can produce).
- **The ids.** P5's log runs `conf::ad_7_payload_launches_after_durable_identity`, which is a contracts transcript. It gets the fence in the same PR as `conf::ad_7_crash_between_steps_leaves_no_unregistered_payload`. At `367aa3f`, 9 transcripts use a start hold: `a2_1_attach_state_rules`, `a2_1_no_deadline_code_exists`, `a2_3_lost_session_closes_routes_session_lost`, `a2_3_every_route_close_reason_maps_to_one_wire_reason`, `ad_1_starting_without_identity_is_lost`, `ad_1_starting_with_identity_adopts`, `ad_2_lost_reasons`, and the two `ad_7` ids. The PR covers all 9. Core's separate `payload_alive` fix (it counted the guard's pre-exec intermediate as the payload) is a Core defect, and it is not part of this ruling.

## R-58. `expect_closed {ended: r}` is the stream's end plus the host's `route_ended`; on the real tier the harness, as the host, reports it (Stage 1 Core lead, P6 and P3, `conf::a2_3_lost_session_closes_routes_session_lost`, 2026-10-10)

- **Finding.** Step 18 (the host's `RouteClosed{SessionLost}` for both routes) passes on the real tier. Steps 19 and 20 expect the client to read `closed {ended: session_lost}`, and on the real tier the client reads `{eof: true}`. After the handoff (DP-1) the worker owns the route stream. A lost worker can only leave the stream closed, and Core holds no route bytes.
- **Reading.** OU-2b decides it: the failed-route reasons are the reasons "that the worker reports to the host and **the host reports as `route_ended`** (the route may lack `route_closed`)". The codec agrees (TS-9): `route_ended` is a Hub event on the client's control connection, not a frame on the route, and "EOF without `route_closed` means that the cause is unknown until `route_ended` arrives". So for a failed route, Core owes the client only the end of the stream, and it owes the host `RouteClosed{reason}`. The host maps that reason by OU-2b and tells the client. Core never owes a cause on the route itself.
- **Ruling: neither (a) nor (c), and (b) is not needed.** In Core's conformance suite, the harness plays the host. So `expect_closed {ended: r}` means two observations, and the real tier makes both:
  1. **The client's stream ends.** The real route edge reads a real EOF, or a real transport error, on the client's end of the stream. Any frame before it is allowed. A `route_closed` frame is not required, and it is not forbidden.
  2. **The host reports `r`.** The harness holds a `RouteClosed{route, reason}` from Core for that route, and the OU-2b mapping of `reason` is `r`. The mapping table lives in the conformance crate, transcribed from OU-2b. It is the host's act, and it is never a Core function that the step trusts.
  The testkit already derives `ended` from Core's `RouteClosed`. That is not a fake: it is the host's act, which OU-2b assigns to the host. The real tier uses the same derivation, with a real end of stream in place of the testkit's. **No transcript changes.** The id is real-provable, and it stays a minimum id.
- **Why not (a).** Expecting only `{eof: true}` would drop the host's report from the client's view, and the 9 transcripts that use `ended` for failed reasons would lose their OU-2b check. **Why not (c).** OU-2b and TS-9 already give the client the cause, through the host.
- **Conditions.**
  1. The derivation is one driver function, used by both tiers. It reads the driver's record of host events, so `expect_closed` never polls Core again, and step 18's `pump_collect` still takes the events.
  2. A missing `RouteClosed` for the route, or a reason that maps to another `r`, fails the step. A stream that does not end fails it at the step limit.
  3. Tests: positive and negative driver tests for each failed reason (`not_connected`, `handoff_failed`, `transport_lost`, `write_failed`, `stalled`, `session_lost`), plus "EOF with no host event fails".
  4. The same form applies to every `expect_closed {ended: …}` step: the 9 transcripts, including `ou_2b_failed_reasons_are_reported_to_the_host_and_route_ended` and `a2_3_every_route_close_reason_maps_to_one_wire_reason`.
  5. The crate reviewer reviews the driver PR. Evidence follows A5-4: the before/after for the 9 ids on the testkit and the real tier.
- **Correction (crate reviewer, PR #34, 2026-10-11).** Condition 3 named six failed reasons, including `not_connected`. That cited OU-2b before Core A17. A17 removes `NotConnected` from A2-3's closed set and from OU-2b's mapping: "The codec's wire values `bad_peer` and `not_connected` stay, for the Hub's own use". So no Core reason maps to `not_connected`. The table has **five** rows (`HandoffFailed`, `PeerClosed`, `WriteFailed`, `StallTimeout`, `SessionLost`), transcribed from OU-2b as A17 amends it. A `NotConnected` row is wrong, and so is a JSON-form test of one. The required negative test: `expect_closed {ended: not_connected}` fails for every Core reason. None of the 9 transcripts expects `not_connected`.
- **R-50 corrected.** R-50 said that, for a route still pending its handoff, the holder of the transport gives a `SessionLost` close "the codec's route-ended reason `session_lost`". That reason is the host's `route_ended`, not bytes on the route. The holder writes nothing for `SessionLost`. It closes the transport once, and the host reports `route_ended{session_lost}` from `RouteClosed{SessionLost}`. The `SessionEnded{exit}` case is unchanged: the holder writes OU-2b's close frame when the socket can take it, then closes.

## R-57. The output-side fence: a title sentinel and `await_title`, a wait that takes no events (Stage 1 Core lead, PC, `conf::ev_2_one_marker_per_session`, 2026-10-10)

- **Finding.** `ev_2` fails on macOS at step 105 in 3 of 3 runs: `EventsLost{kinds: [Bell]}` where `[Bell, Notification]` is expected. The ack of a `pty_output` control means only that the probe took the request. The bytes then pass through the probe's writer, the PTY, the worker's read and parse, and the control link. `await_quiet` proves only that no edge holds an unconsumed report at that moment (R-43 item 8), so it cannot see bytes that are still before Core's edge. The final bell of step 103 reaches Core after the marker has been taken.
- **Why the existing steps do not fit.** `await_pty` is for input. `pump_until`, `pump_collect` and `expect_eventually` take events. The single loss marker gains kinds only while it is unpolled (EV-2), so a fence that polls changes what the test observes.
- **Ruling: a sentinel on the same ordered path, read by a sync read.**
  1. **The sentinel.** The transcript appends an OSC 2 title with a unique value to the same `pty_output` bytes as the output that it fences (for `ev_2` step 103: `07` followed by `1b5d323b` + the sentinel + `1b5c`). One PTY write, one parse order: the worker observes the title after the bell.
  2. **The step: `await_title {session, title}`** (a new driver step, contracts crate). It pumps, never calls `poll_events`, and after each pump reads `terminal_state(session).title` (ST-4, a sync read that makes no progress, OR-1). It ends when the title equals the sentinel. It fails at the step limit, with the last title it read. The wake source is the worker's metadata frame on the control link, so it ends on the real event on both tiers. On the testkit it ends at once.
  3. **The ordering that makes it a fence.** EV-1 places metadata in the queue in the order of the worker's observations, so the sentinel's `TitleChanged` is after the bell. The fence also needs ST-4's `title` never to run ahead of the queue: when the read shows an observation, that observation's `TitleChanged` has been posted, and so has every metadata event of an earlier observation, also under an exhausted `pump_events` budget (E3-1). EV-1 class K treats the event and the state value as one observation, so I read this as Core's meaning. It is not written out. **Condition:** a Core test proves it before the fence counts as evidence, including a run where the budget is exhausted between the bell and the title. If Core cannot meet it, the step is not a fence, and the question comes back to me as an amendment.
  4. **What the transcript may change.** Only two things: the sentinel bytes, and the sentinel's `TitleChanged` added to the `ignore` lists of the later steps that would otherwise see it. Every expectation of `ev_2` stays.
- **Rejected.**
  - A fixed wait after each ack. It is a timer, so it is no fence.
  - A Core counter of received output bytes. Core's host does not see the output stream, so the counter would need production instrumentation (Testing rule 8).
  - A probe-side drain check, such as the PTY's output queue reading zero. It ends before the worker's parse and frame write, so the race remains.
- **The sweep.** A `pty_output` followed only by `await_quiet`, where a later step depends on that output reaching Core, has the same race. R-56's ratchet applies: the lint of R-56 PR B (or a PR C after it) also requires an `await_title` sentinel fence after such a `pty_output`. It lands with an exception list of the current unfenced cases that may only shrink. Absence checks are exempt only by a named reason. Only the last output before an expectation needs the fence, because one PTY keeps the order of its bytes.
- **Evidence (A5-4).** The before/after table for `ev_2`, on the testkit and on the real tier (Linux and macOS). `ev_2` stays pending until the fence lands.
- **Follow-up (Core lead, 2026-10-10).**
  - **Condition 3 is met.** Core #242 (`75cb8b37`, tests only) proves it with `pump_events` 1:
    - (a) a bell, then a title, with a pump that ends between them. When the sentinel shows, the bell and then its `TitleChanged` have been posted.
    - (b) the `ev_2` shape with `event_queue` 1. When the sentinel shows, the loss marker already holds `[Bell, Notification]`.
    Both tests fail when the order is reversed. The source reason: one call sets the title and posts `TitleChanged`, each frame holds one observation, and Core checks the budget between frames.
  - **Scope limit.** A launch or adoption report sets the whole terminal as a baseline, with no event per field. So an `await_title` fence is valid only when no launch or adoption falls between the sentinel's `pty_output` and its `await_title`. `ev_2` has none. The step's doc states this limit. Where it can, the lint checks it: no `Start`, `Adopt`, `AdoptAll` or adopting `open` between the two steps.
- **Follow-up (PC, lint PR B, 2026-10-11): OSC 0 is a valid sentinel.** EV-7 maps OSC 0, 1 and 2 to the title feed. OSC 0 sets the icon name and the window title, so it moves `terminal_state.title` and posts `TitleChanged` exactly as OSC 2 does. A sentinel is OSC 0 or OSC 2, with BEL or ST as the terminator. OSC 1 is not a sentinel, because it sets only the icon name. `st_6b_model_after…`'s T1 and T2 (OSC 0), which I approved in #36, stand. The lint accepts OSC 0 and OSC 2.

## R-56. `Completed{WriteInput}` does not order the probe's read: a `pty_input` read of written bytes needs an `await_pty` fence (Stage 1 Core lead, PC's proposal `shared/core-stage1/pc-pty-input-fence-proposal.md`, 2026-10-10)

- **Reading.** IN-2's counts are "written to the PTY": the worker's write to the PTY master. No clause says that the program has read the bytes when `Completed` is posted. On the real tier, the probe reads the slave and reports over its own socket after that, so a `pty_input` read straight after `Completed` races the probe. A transcript that asserts it over-asserts on the real tier. This is a transcript fix, like the `ad_3` fence of contracts #30, and not a Core change. Evidence: `in_5_lane_bounds_refuse_at_begin` fails on macOS at step 18 or 19 (`616161` expected, `""` read).
- **(1) Accepted: the fence is the existing `await_pty {session, min_bytes}`.** `min_bytes` is the session's total log length (driver tests at v0.1.27). On the real tier, each probe `Input` report wakes the handle, so the wait ends on the real event. A real-tier fence needs a wake source, and this one has one. The exact asserts stay. They still prove that no more than the expected bytes arrived, because the refusal happened at `begin`. On the testkit the wait returns at once. `in_5`: `await_pty s1 4` before step 18 and `await_pty s2 3` before step 19. The probe-side `Sync` is rejected, because the Linux tty flip buffer makes it a weaker fence.
- **(2) Accepted: fence the other literal asserts** at `a2_2` step 8, `a5_1` step 14, `am_2` step 7, and step 15 of both `in_10` ids. Each fence's `min_bytes` is the length of the asserted log. Adding a fence changes no expectation.
- **(3) Accepted: the runner lint, as a ratchet.**
  1. **The rule.** A `pty_input` step that asserts or captures a non-empty log must have, after the last write that can reach that session, an `await_pty` of that session whose `min_bytes` is at least the asserted length. For a capture, `min_bytes` comes from a bound number (a `let` sum), as for replies (core-p4b notes).
  2. **Absence checks are not this pattern.** A `pty_input` read that proves that no more bytes arrived (the "before and after" form of core-p4a notes) races the other way, and an `await_pty` cannot fence it. The lint exempts it only by a named exception with that reason.
  3. **The ratchet.** The lint lands with an exception list of the current unfenced captures (the 238 steps in 20 transcripts, mostly `in_9_*`), each with the reason "unfenced capture, R-56". The list may only shrink, and the lint fails when an entry is no longer needed. Fencing them is follow-up work, and the list must be empty before the real-pending count must reach zero.
- **Evidence (A5-4).** Each PR gives the before/after table for every transcript that it touches, on the testkit and on the real tier (Linux and macOS for `in_5`), with no PASS→non-PASS.
- **Linux evidence for PR A (#32, merged at `94b63b8`).** PR A's real-Linux "after" column is deferred to Core's pin-move gate to `contracts-v0.1.28`. The Linux "before" is gate `7a4ab70b`. This is accepted because PR A changes only transcript data. An `await_pty` fence can only add a wait. It can only fail when its `min_bytes` is too high, and `min_bytes` does not depend on the platform. Two conditions bind Core:
  1. `in_5` is not a minimum PASS until that Linux gate reports it PASSED.
  2. A touched id that regresses on Linux is a contracts defect. A contracts PR fixes it before Core's pin move lands, with no waiver.
- **PRs.** Two contracts PRs, so the `in_5` flip does not wait for the lint: **PR A**, the fences of (1) and (2). **PR B**, the lint and its exception list. The crate reviewer reviews both. The steward merges them on the exact head after botster-gate is green.

## R-55. The real-tier forms of `host_crash`, `reuse_pid`, `bystander_alive` and `signals_received` for `reuse_pid` (Stage 1 Core lead, P5, `conf::ad_6_reused_pid_is_never_killed`, 2026-10-10)

R-49's principle applies: a real-tier control is built from the edges that the real harness wraps, and a model is accepted only where everything that Core can observe is the same as in the real event. Each of the models below is a fiction about **history**. At the moment Core looks, the OS facts that it reads are real.

- **(1) `host_crash`: accepted as a model, with conditions.** It drops the Core handle, then puts back the stale control-socket file that a dead host leaves. A real crash releases the same descriptors and the directory lock through the kernel. Workers have their own process group and no parent-death signal, so they keep running in both cases (LC-12, AD-7). The honest alternative, a host in a child process behind a remote `CoreApi`, is rejected for now as a large harness change for no observable difference. Conditions:
  1. **Only at a quiet point.** The control first waits for quiet, as `await_quiet` does, and fails if Core does not reach it. A crash in the middle of an edge write (AD-7's partial write) is not modelled by a drop, and it needs another form. Both current users (`ad_6` after `Completed{Start}`, `ad_7` after `await_quiet`) are at a quiet point.
  2. **Drop leaves nothing alive that acts.** *(Amended 2026-10-10, P5 via the Core lead: the first text, "no thread and no descriptor", cannot hold for a handle that spawned a live worker.)* `Children::spawn` starts one reaper thread per child, blocked in `wait()`. A drop cannot end it, and it must not kill the worker (LC-12). The reaper is needed in production: the host process stays the worker's parent after a drop, so without it an exited worker stays a zombie. In a real crash, the OS reaps the worker instead, and Core never reads its parent pid, so the reaper changes nothing that Core observes. The rule: after the drop, the only threads and descriptors of the handle that remain are **one reaper thread per live worker that the handle spawned**, and the **wake object's descriptors, a fixed count that does not grow with the number of workers**. A test proves it:
     - The counts before `open` and after a drop with N live workers differ by exactly N threads and the fixed wake descriptors.
     - The dropped handle's tap records **no edge call** after the drop: no row write, no link write, no signal.
     - After each worker ends, its reaper thread ends, and with every worker ended, the counts return to the counts before `open`, except for the wake descriptors.
     A surviving thread that wrote or signalled after the "crash" would break the model, because a real crash never does.
     - **Follow-up (Core #257, P5; reviewer finding D-F1, 2026-10-11).** "Each reaper thread ends when its worker ends, and the counts return" has no real event on the real tier. The reapers are detached. After a crash no host exists to join them. A thread's exit has no portable notification: Linux `PIDFD_THREAD` needs kernel 6.9, and macOS has none. Waiting on the count is polling (Testing rule 5), and a global registry of orphan reapers would be a product global added for a test. So the thread-count part is proved in another form:
       1. **At the crash** (deterministic): threads = the count before `open` + one per live worker. Descriptors = the count before `open` + the fixed wake descriptors.
       2. **The reap is a real event, for one worker.** The production `notify` sets a wake flag that only a host's `pump` drains. So after a crash, only the first reap gives a distinct event. The form:
          - (a) before the crash, pump until `PumpReport` reports no more work, so the flag is clear;
          - (b) end one worker (w1) only, and wait once on the kept `WakeHandle`, so that `Woken` is w1's notify;
          - (c) assert that w1 is reaped (its identity is absent), and that w2 still runs;
          - (d) w2 ends at the harness teardown, which verifies its end with a real exit notification.
          One reap after a crash is enough, because every reaper runs the one named function of item 4.
       3. **Nothing acts after the drop:** the tap records no edge call, as before.
       4. **The reaper ends right after its notify:** the reaper body is a named function, not an inline closure. A test runs it on a thread that the test owns, joins it (a real event), and asserts two things: it returned after its `notify`, and it made no edge call. The spawn wrapper only detaches that function, and the reviewer checks that it does nothing else.
       So the claim "the counts return to before" rests on items 1 and 4, not on a wait for a thread to exit. Items 2 and 3 cover what condition 2 protects: that nothing acts after the crash.
  3. **Drop changes only the socket file.** The control snapshots the data directory before the drop and compares it after the restore. Any difference other than the socket file fails the control. So if Core's `Drop` gains a step, the model fails loudly, instead of silently becoming a graceful drop (LC-12).
  4. **The restored file is a real stale socket.** It is a socket node at the same path, with the same owner-only mode, and no listener: bind, then close. It is not a regular file.
- **(2) `reuse_pid`: accepted, with conditions.** A test cannot make the kernel reuse a pid. Forking until the pid wraps is unbounded and slow, and setting a pid needs privileges that the gate hosts do not have. Core's whole view of a recorded identity is the pair {pid, start time} (AD-6). So a row that names a live, unrelated process with another start time gives Core the same input as a real reuse. Conditions:
  1. **The worker really ends first.** The harness ends the worker by an identity-checked group kill and waits for the end (`wait_worker_exit`, R-46 follow-up). No process answers at the old endpoint.
  2. **No Core holds the directory during the rewrite.** The rewrite happens after `host_crash` and before the next `open`, as in R-49 Q4 condition 1.
  3. **Only the pid changes.** The harness encodes the row with Core's own `Row` encoder. It decodes the row before and after, and asserts that every field except the pid is equal: the start time, the token, the `InstanceId` and the endpoint stay as Core wrote them.
  4. **The bystander is really unrelated.** It is the harness's own child, in its own group. Its pid is not in any other recorded identity. The harness asserts that its start time differs from the recorded start time, and makes a new bystander if they are equal.
- **(3) `bystander_alive`: accepted.** The bystander is a live member of its group, and it is not a zombie. The harness, as the parent, reaps it only at teardown, so a dead bystander reads as a zombie, not as a reused pid.
- **(3) `signals_received` for a `reuse_pid` session: accepted, widened.** It is R-49 Q1's rule: every signalling edge call that the reading handle makes, with its target. That includes `signal_group`, the descendant kill and every other signalling call. It is not limited to `signal_group`. A call that targets the bystander's pid or its group is a failure. The transcript expects an empty list, so any signalling call at all fails it. The tap of the new handle (`h2`) is the complete set, because the bystander did not exist while the crashed handle was alive.
- **Text.** In a contracts PR, the `host_crash`, `reuse_pid` and `bystander_alive` rows of `docs/core-testkit-controls.md` gain "real tier: steward ruling R-55". Without it, the `host_crash` row ("the real host process is killed") misdescribes the real form. The Core control code cites R-55 at each fiction. The crate reviewer reviews the PR.

## R-54. EV-8 query replies are still written to the PTY while the session is `Stopping` (Stage 1 Core lead, P3, Core #227, 2026-10-10)

- **Question.** After a host `Stop` (or a kill) and before the payload exits, the worker refuses host writes and route input with `NotWritten(Stopping)`, and DP-5 maps a refused route input to `input_refused{session_ended}`. Does the worker still write EV-8 query replies (a client reply or the shadow reply) to the PTY in that window?
- **Ruling: yes (P3's reading).**
  1. `NotWritten(Stopping)` is a reason of IN-2's `InputResult`, the completion of a write. A query reply has no `InputResult` and no op of its own. It is not a host write, and it is not one of DP-5's client inputs.
  2. EV-8(c) closes the set of refusals for a client reply: `already_replied`, `query_expired`, `unknown_query`, `invalid_input` and `too_large`. `session_ended` is not in that set, so a reply is never refused for the session state.
  3. EV-8(h) closes the set of events that retire a query: session removal, payload end, route close and admission. `Stop` is not in that set. A query stays open, and its reply is admitted, until the payload ends. At payload end the query retires, and nothing is written.
  4. EV-8(e) says that a fallback that cannot yet be admitted stays pending and is retried. It is never refused for good. A reply that `Stopping` refused would leave the payload waiting for an answer that never comes, for the full `stop_grace`, and a graceful stop would become a kill. LC-5 asks for a graceful request first, so the payload must be able to finish.
- **Rejected:** "once the session is `Stopping`, nothing new reaches the PTY". No clause says it. It would add a refusal to EV-8(c) or a retirement event to EV-8(h), so it needs an amendment.
- **Scope.** Only EV-8 replies are covered. Host writes and every other client input are still refused with `NotWritten(Stopping)` and `input_refused{session_ended}`. AM-2 is unchanged: a reply is still one contiguous transaction at the one admission point, and it never advances `input_rev` (EV-8(f), IN-10).
- **Tests.** No new conformance id. Core keeps P3's test: a payload that sends a query after `Stop` gets its reply, and a host write in the same window completes `NotWritten(Stopping)`.

## R-53. The route input allowance always admits one whole frame: `max(route_input_queue_bytes, max_frame_bytes + 4)` (Stage 1 Core lead, P3, Core #227, 2026-10-10)

- **Finding.** CoreLimits defines `route_input_queue_bytes` (1 MiB) as "the worker's read allowance per route for client input frames", and DP-5 says that the worker stops reading the route when that queue is full. The route's `max_frame_bytes` defaults to `max_paste_bytes + 4096`, which is larger. Read literally, a client frame of the route's own maximum size can never be read completely: the worker stops before its last bytes, and the route deadlocks with no frame and no refusal.
- **Ruling: (A).** The route admits frames up to `max_frame_bytes` (DP-3), so its reader must be able to read one. A bound that blocks forever the one frame that the route admits is a deadlock, not the backpressure that DP-5 describes ("the worker stops reading the route and transport backpressure applies", so that the input is delayed, never stuck). The effective allowance is therefore `max(route_input_queue_bytes, max_frame_bytes + 4)`, where 4 is the stream frame prefix of the codec. It is built from two existing numbers, so it adds no new bound, and no default changes.
- **Rejected: (B)**, an `open` refusal when `route_input_queue_bytes < max_frame_bytes + 4`. It fails at the defaults, so it would need a default change by amendment, to settle what (A) settles by reading.
- **Tests.** Core #227 states this ruling. A test sends one frame of exactly `max_frame_bytes` on a route at the default limits and observes that it is processed (written, or refused for its own reason), not stalled. A memory budget that counts the input queue counts the effective allowance.

## R-52. `alloc_peak` on the testkit is the peak of the calling thread (Stage 1 Core lead, P3, 2026-10-10)

- **Question.** Core's conformance binary runs its trials in parallel. A process-wide peak includes other trials' allocations, so the differential check of `conf::dp_3_frame_limit_checked_before_allocation` becomes flaky.
- **Ruling: accepted.** R-37's purpose is to observe the allocations of the route reader, and Core A20-1 places the observation "at the process allocator boundary of the process in which the worker's code runs". A peak kept per thread is still taken at that boundary. It only leaves out the allocations of other trials. One testkit trial runs the harness, Core and the in-process worker on its calling thread (A5-1), so the per-thread peak holds every allocation of the subject.
- **Conditions.**
  1. A positive test: a large frame that the in-process route reader allocates appears in the calling thread's peak. This proves that the subject's allocations are counted, and not only that other threads' allocations are left out.
  2. A negative test: an allocation on another thread is not in the peak.
  3. R-37's conditions stand. The per-thread state is a `const`-initialized thread-local cell, with no lock, wait or synchronization point, and the allocator path never allocates.
  4. If the testkit ever runs a trial's worker code on another thread, test 1 fails, and this ruling must be revisited.
- **Text.** The control row of `docs/core-testkit-controls.md` changes from "in the testkit process" to "on the calling thread (the trial's thread; steward ruling R-52)", in the same contracts PR. The crate-catchup reviewer reviews it.

## R-51. "Input received before `live`" is the worker's receive order, and no clause orders queued input before the worker reaches `live` (Stage 1 Core lead, P3, `dp_5_input_before_live_is_refused_not_ready`, 2026-10-10)

- **Reading.** DP-5 says "No input before `live` (it is refused `not_ready`)", and TS-5 says "Order is the worker's receive order. Input received before `live` is refused `not_ready`". The input-ready point is the worker's own `live` transition. No clause says that the worker reads input that is already in the transport before it forms its baseline and reaches `live`, or that `live` counts only when its frame is delivered.
- **Consequence.** In a setup where the input is in the transport before the worker has it (attach, then send, then pump), both outcomes are conformant: `input_refused{op, not_ready}` if the worker reads the input first, and a normal write with no frame if it reaches `live` first. A transcript that requires the refusal over-asserts, unless its setup proves that the worker received the input before `live`.
- **The id.** The testkit proves that precondition only through the gated write count (steps 7 and 8, the boundary counts of R-47 item 2). The real tier has no form of it, because the kernel takes a small baseline at once. So the transcript stays unchanged. The id is real-pending with the reason "no real form of the precondition (R-51)". It is not a minimum id.
- **Deferred.** A transcript that accepts either outcome needs a driver disjunction. Making the refusal certain needs a Core amendment that orders queued input relative to `live`. Neither is worth doing now. Both are recorded for when real-pending ids must reach zero.

## R-50. The routes of a failed start close after the state, with the reason that the end state gives (Stage 1 Core lead, P3, 2026-10-09)

- **Finding.** A route attached while a session is `Starting` stayed bound until `Remove` when the start failed on some paths, and closed `SessionLost` on others. So one failure had two observable outcomes.
- **Lost → `SessionLost`.** A2-3 adds `SessionLost` to OU-2's closed list of the events that close a route: "the session's worker exited or the session is `Lost`". A failed start that ends in `Lost` (for example `Lost(WorkerGone)`) meets it. A2-3 has no exception for a session that never reached `Running`.
- **Exited → `SessionEnded{exit}`**, with the exit that the `SessionState` carries. This covers a link lost before `Launched`, `LaunchFailed`, the startup timeout and adoption `never_ran`. OU-2 closes a route on "session end, after the queue drained (OU-7; `SessionEnded`)", and an empty queue is drained. `SessionLost` does not fit an `Exited` session whose worker is alive. When a worker exit also happened, the reason still follows the recorded state, so the host never sees `Exited` with `session_lost`.
- **Order.** The state comes first, then one `RouteClosed{route, reason}` (OU-2, A2-3: the first reason wins).
- **A route that was never handed to the worker** (still pending a handoff) is not just dropped. OU-2 requires exactly one `RouteClosed{route, reason}` to the host for every route, a class M event. The holder of the transport (Core, before the handoff) owes the client what OU-2 says the worker owes: the reason in the wire form that A2-3 gives it, when the socket can take it, then one close of the transport. For `SessionEnded{exit}` that is the close frame of OU-2b. For `SessionLost` it is the codec's route-ended reason `session_lost`, because A2-3 maps `SessionLost` to the route-ended reasons, and the codec has no close frame for it. *(Corrected by R-58: `route_ended` is the host's report, so the holder writes nothing for `SessionLost` and closes the transport once.)* `HandoffFailed` is wrong here, because no handoff was attempted.
- **Tests.** There are no new ids. `conf::a2_3_lost_session_closes_routes_session_lost` gains the `Lost` case: a route attached while `Starting`, then the worker dies before `Running`. The existing id that covers `SessionEnded` closes gains the `Exited` case, with a startup timeout. Each case asserts the `RouteClosed` to the host and the wire reason that the client sees, and none asserts which edge reported the failure.

## R-49. The real-tier forms of `signals_received`, `impostor_worker`, `withhold_control_link`, `hold_start_at(payload)` and `fail_next` (Stage 1 Core lead, P5's real-controls plan, 2026-10-09)

The controls doc says that every control is built from injected parts and is never a branch in production code (BUILD.md, Testing rule 8). On the real tier, the injected parts are the edges that the real harness wraps: the filesystem endpoint, Core's `RealEdges` calls through the tap, and the stored rows.

- **(Q4) A stand-in at the real endpoint path is accepted** as the real form of `impostor_worker` and `withhold_control_link`. A10-1 says the impostor answers "from an endpoint that is not the session's worker", and the endpoint path is the boundary that Core uses to find the worker (E1-1). Conditions:
  1. The harness renames the endpoint aside and binds its stand-in only while no Core instance holds the data directory: after the host closes and before it reopens. It never races a live Core.
  2. A drop guard removes the stand-in and renames the real endpoint back, also on a panic or a failed step.
  3. The real worker keeps running, as both rows require. The `withhold_control_link` stand-in accepts the connection and never answers, so Core's own handshake deadline decides the outcome.
- **(Q1) `signals_received`: the count at the edge is accepted.** Every signal that Core sends goes through core-sys's signalling edge, and the xtask signals check enforces this. So the tap's calls are the complete set of signals, and the row already calls this "a read-only count at the edge boundary".
  - **What the count is.** Core cannot learn the stand-in's pid, because v1 reads no peer credentials. A count limited to the stand-in would be 0 by construction, and that would be vacuous. The process that Core could wrongly signal is the process at the **recorded** worker identity: from Core's view, that is the party whose handshake failed (A10-1: "a non-matching process at the recorded pid is `WorkerGone`").
  - So on the real tier, `signals_received` returns, in order, every signalling edge call that Core makes from the refused handshake to the end of the step, with its target identity. That covers `signal_group`, the descendant kill and every other signalling call. The transcript's empty expectation then proves AD-6 against the identity that Core really holds.
- **(Q2) `hold_start_at(payload)`: the tap may hold the host's outbound `Launch` frame** on the control link, which is the real process edge. A payload wrapper that blocks before its exec is rejected: it is a real process that Core sees as the payload, so it cannot show "no payload runs". Conditions:
  1. The hold starts at the first byte of that session's `Launch` frame and holds everything after it on the same link until `release_start_at`. It never reorders, drops or changes a byte.
  2. The transcript proves "no payload runs" with the identity-checked `payload_alive` read, not from the hold itself.
- **(Q3) `fail_next` is not an edge failure.** The plan's premise is wrong. `fail_next` scripts a **synchronous refusal** of a Core API call (A5-3 item 1: "The testkit returns it **from the call, before real Core admits anything**"). The transcripts target `Create` and `Start`, not a `HostEdges` call. A failure injected into a `HostEdges` call would be an asynchronous failure (A5-3 item 2), which is a different class with a different timing.
  - The real form is the same as the testkit's: the real harness's call wrapper returns the scripted code without calling real Core. It refuses a code outside the call's sync column (`Refused{not_in_sync_column}`).
  - The transcripts then check real Core: no event, and `get` gives `UnknownSession`. So the ids prove on the real tier that the refusal left real Core untouched (AM-4).
  - This needs no tap fault injection, and plan 23l's pass-through tap is unchanged.
  - Asynchronous edge failures (item 2) keep their own controls and rows.
- **(Follow-up) `announce_protocol` uses the same stand-in** (Q4's conditions apply unchanged). The stand-in's hello proves the recorded token and `InstanceId` and announces the scripted protocol number. The harness reads the token and instance from the stored row, which is an allowed edge. The stand-in sends nothing after its hello. Core's own version check (A6-2, AD-4) decides the outcome, and the real worker keeps running. As on the testkit, a protocol inside {T, T−1} is `unsupported_control`.

## R-48. The conformance probe owns its line discipline: on a terminal, it turns off canonical mode and echo (Stage 1 Core lead, P6's real-tier OSC/BEL timeouts, 2026-10-09)

- **Finding.** On the real tier, 8 ids timed out (`a2_4` ×4, `e2_1`, `ev_3` ×2, `ev_7`). The payload's PTY is in the default canonical mode, so a byte with no newline stays in the line discipline and never reaches the probe's `read()`. The probe's own tests use pipes, so they never met canonical mode.
- **Ruling.** Core does not set the payload's terminal modes: the payload owns its line discipline. The fix belongs to the probe. When stdin is a terminal, the probe clears `ICANON`, `ECHO`, `ECHOE`, `ECHOK` and `ECHONL`, and sets `VMIN=1` and `VTIME=0`, before its first step. It leaves `ISIG`, `ICRNL` and `OPOST`/`ONLCR` unchanged: the current real output (`\r\n` for `\n`) depends on `ONLCR`, and the signal transcripts may depend on `ISIG`. When stdin is not a terminal, the probe changes nothing. No contract text changes.
- **Transcripts** must not rely on PTY echo or on canonical line buffering. The fake models neither. This also removes the cause of R-43 (E).
- **Evidence for the merge.** A probe test on a real PTY (`openpty`): with `print_after_input "1" → "\x07"`, a write of `1` with no newline gives exactly `\x07` and no echo. The PR body gives the real-tier outcome of every id before and after the change, on both Core harnesses. No id goes from PASS to another outcome.

## R-47. The real form of `route_gate` and `route_accept` is the kernel's own backpressure; `alloc_peak` on the real tier needs Core A20 (Stage 1 Core lead, P3's real-tier reading, 2026-10-09)

- **(A) `route_gate` and `route_accept`:** on the real tier, the client end stops reading, and the socket buffers are small (`SO_RCVBUF`, `SO_SNDBUF`). That form is accepted. OU-3a defines the progress point of a `Stream` as "bytes that the kernel accepted from the worker's write to the route socket", so the real form tests the contract's own definition.
  - **No test-only worker binary.** The real tier would then not run the production worker, and A5-4's purpose ("so the in-process worker is proven to match the real process") is lost.
  - **Transcripts** assert order, losslessness, eventual delivery and the stall classification. A transcript that asserts a count derived from the gate's `N`, or that exactly 0 bytes were accepted, over-asserts (R-43 item 7). The Core lead lists those steps, starting with `ou_12`, `ou_3` and `ou_7`, and the steward rules on each fix.
- **(B) `alloc_peak` and `conf::dp_3_frame_limit_checked_before_allocation`:** R-37 allows the observer only in the process where the route reader runs. A5-4 says that both tiers must pass, with no exception for this. No clean real form exists, so a ruling cannot settle it. Core Amendment 20 proposes a narrow testkit-proven class with this id as its one member. Until A20 is final, the id is in `core-real-pending.txt` with the reason "Core A20 pending (R-37 vs A5-4)".

- **(A, follow-up) P3's audit of the 64 transcripts that use `route_gate` or `route_accept`:**
  1. The minimum ids `ou_12`, `ou_3` and `ou_7` hold. `conf::ou_3_progressing_reader_lossless` needs the fill of item 3 before its `route_accept` loop. Without it, the step can pass on the real tier with no frames pending, and that pass is vacuous.
  2. `conf::dp_5_input_before_live_is_refused_not_ready`, steps 7 and 8, assert counts at the socket boundary. They change to assertions about which frames the client sees after the release, and in what order.
  3. The 47 stall transcripts must reach "frames pending in the worker" on both tiers. One new control (for example, `route_fill {route}`) writes PTY output larger than what the transport can hold beyond the worker's queue, plus one maximum frame. The harness takes that size from the buffers that it set on the socketpair it created (`getsockopt`; the reported values are a safe upper bound). On the testkit, a gated route holds 0 bytes. The output is a pattern that the harness states. A precondition that is not reached is a setup failure, never a pass. The steward rules on the control row before the transcripts change.
  4. **(B) is settled by Core A20 (manifest-final39):** `conf::dp_3_frame_limit_checked_before_allocation` is testkit-proven, and the real tier lists it as such.

- **(A, item 3 ruling) The `route_fill` control row (P3's proposal, accepted with changes):**
  1. **Size.** N = C + F. C is what the route's transport holds outside the worker, and F is the route's `max_frame_bytes`. Framing only adds bytes, so at least one frame stays pending in the worker. N does not depend on `route_queue_bytes` or on any other contract number: a stall needs only pending frames with no progress (OU-3b(b), R-45), not a full worker queue.
  2. **C on the real tier.** C is the worker end's `SO_SNDBUF`, read before the handoff from the socket that the harness created, plus the client end's `SO_RCVBUF`, read at fill time. The harness keeps no dup of the worker end, because a dup changes end-of-stream for the route-close transcripts. The row states that C is not an upper bound if the worker changes its end's buffer sizes. The Core lead confirms in the PR that Core does not change them; if Core ever does, the steward rules again. On the testkit, C is the stream's free room: 0 while `route_gate` is on.
  3. **Return.** The control returns when the program edge has the bytes. It never waits for the worker, or for the probe's write to complete (OU-3d source backpressure may block that write).
  4. **Pattern.** Byte k is 0x61 + (k mod 26), and k restarts at 0 for each `route_fill`. The pattern has no 0x0a byte, so the terminal's newline translation (R-48) cannot change it. The result `{bytes: N}` can be bound.
  5. **Lossless assertion.** `route_expect_output` also accepts `{"parts": [...]}`. Each part is `{"fill": "$name"}` (the pattern of that fill's `bytes`) or `{"bytes_hex": ...}`. The expected output is the parts in order. The `bytes_hex` form is unchanged. The driver tests cover a fill alone, a fill and then a suffix, two fills, an unbound name (a setup failure) and a mismatch inside a fill (a failure that reports the offset).
  6. **Process.** One contracts PR carries the row, the driver change, the testkit support, the 20 stall transcripts (class A of P3's fixed-rule list of 54 at `fe3eb95`; the earlier "47" was a hand count; class A includes `ou_3_progressing`, with the fill before its `route_accept` loop) and `dp_5_input_before_live` steps 7 and 8. The crate-catchup reviewer reviews it.
  7. **`queue_overflow` (class B, 34 ids) keeps its own row.** Its precondition is the route's byte bound (TS-4c, A8-2), so its real form depends on `route_queue_bytes`, unlike `route_fill`. P3 proposes its real-tier form, and the steward rules before any class B transcript changes. An unreached bound fails those transcripts, because each one asserts the resync, so no class B pass is vacuous.

## R-46. Revises R-43(A): against real processes, the driver's injected clock moves only by `advance_clock` (Stage 1 Core lead, P6 real-tier clock measurement, 2026-10-09)

- **Finding:** under R-43(A), the real harness passed an injected clock, but the conformance driver's `idle_wait` jumps an injected clock to Core's next deadline whenever nothing is runnable. Against real worker processes, startup and stop-grace deadlines then expired in virtual time before the real worker acted (for example, `conf::lc_3_create_then_start` ended `Exited{cause: Other}`). P6 measured 21 passes and 100 failures with the injected clock, and 55 passes with the real clock.
- **R-43(A)'s principle stands.** The harness is the host (TM-1) and passes the transcript's clock. Exact deadlines and exact `at` values stay exact.
- **The jump is sound only when the injected clock controls every source of progress**, which is the testkit. Against a harness with real processes, the injected clock moves **only** by `advance_clock`. `idle_wait` then waits on the wake handle, bounded by the step limit, and never jumps.
- **Transcripts.** A transcript that relies on the implicit jump to fire a deadline gets an explicit `advance_clock`, placed after the real event that it must follow. That is equally valid on the testkit.
  - **Follow-up (P5, `withhold_control_link`, 2026-10-09).** A deadline that an operation arms when Core **starts** it (in a `pump`, TM-1) is armed at the clock of that pump. So an `advance_clock` placed right after a `begin` can run before the deadline exists, and the deadline then never fires. Between the `begin` and the `advance_clock`, the transcript puts `await_quiet`. It pumps until no runnable work remains and no edge holds an undelivered report, so Core has started the operation and is waiting. If the deadline is still armed later, the step ends INCONCLUSIVE, never as a false pass. This applies to the six `withhold_control_link` ids (`begin AdoptAll` → `await_quiet` → `advance_clock`) and to every transcript of the same shape.
  - **A deadline that re-checks a real process end** (`ad_2_lost_row_keeps_id_and_allows_retry` step 25: `Remove` of a `Lost(WorkerUnreachable)` session whose real worker still runs; Core kills it, then re-checks the worker's identity every `stop_grace`). The transcript fences on the real end before it advances: `begin Remove` → `await_quiet` → `control wait_worker_exit {session, deadline_ms}` expecting `{exited: true}` → `advance_clock {ms: <the transcript's stop_grace>}` → `pump_until Completed`. Neither fence moves the clock, so the re-check cannot run early and re-kill.
    - **Superseded form:** an earlier form of this note used `await_control` on `worker_alive`. It cannot end on the real tier: the death of a worker that the current handle did not spawn produces no wake, so `await_control` waits out its limit. The driver does not gain a polling re-check either, because its fences never poll.
    - **`wait_worker_exit` for a worker that the current edge did not spawn:** on the real tier the harness waits on an OS exit notification for the **recorded** identity (Linux `pidfd_open` and poll; macOS kqueue `EVFILT_PROC`/`NOTE_EXIT`). It first checks that the pid still carries the recorded identity, so a reused pid is never awaited. It sends no signal. The testkit gives the same answer from its process edge. The control row in `docs/core-testkit-controls.md` gains this sentence in the PR that implements it. `deadline_ms` stays the limit of an inconclusive wait.
- **Worker-side timers** run on the worker's own real clock. A transcript that needs one to fire is a named real-process test.
- **Rejected:** tolerance bounds on the real tier (they weaken exact assertions), and testkit-only ids (A5-4 rules them out).
- **Work (R-43's process):** a contracts driver change, with a harness signal for fully injected progress and a test of both branches, then the transcript fixes after P6's measurement.

## R-45. One baseline sequence in delivery does not count toward the `route_queue_bytes` threshold; frames behind it do (Stage 1 Core lead, P3 on Core PR #206, 2026-10-09)

- **Question:** with `route_queue_bytes` = `max_snapshot_bytes` = the snapshot size, the worker queues the snapshot plus the baseline's other frames (`baseline_begin`, `modes`, `baseline_end`, `live`), the stream prefixes, and the output after `R`. That is over `route_queue_bytes`. Is a bounded one-baseline excess allowed, and does the route then count as not progressing?
- **The text:** OU-9 says that "`route_queue_bytes >= max_snapshot_bytes` (9B) guarantees that a snapshot within the limit also fits the queue". 9B says that "A route over it counts as not progressing (OU-3b)". If the baseline's other frames counted toward the threshold, a snapshot within the limit would make the route count as not progressing at the edge configuration that 9B permits. OU-9's guarantee would then be false, and the route could resync in a loop. The threshold is read so that the guarantee holds.
- **Answer:**
  1. **The one baseline sequence in delivery is exempt from the threshold test.** Its bytes are: the frames from `baseline_begin` through `live` of the current baseline or resync, with their stream prefixes. There is at most one such sequence per route, because a resync drops the baseline frames that have not started (OU-9). The test "a route over `route_queue_bytes`" counts only the frames queued **behind** that sequence, including the output after `R`.
  2. **The route is not "not progressing" because of that occupancy.** It still becomes `Stalled` by OU-3b(b): no bytes accepted by its transport, with frames pending, for `CoreLimits.reader_progress_deadline`. That is the existing deadline, unchanged.
  3. **While the baseline drains, the route is a progressing reader** if its transport accepts bytes. If it is behind, the worker stops reading the PTY (OU-3d). That is source backpressure, and it is lossless.
- **The bound stays finite.** A route holds at most `route_queue_bytes` behind one baseline sequence. The sequence is bounded by `max_snapshot_bytes` for the snapshot, and by the route's floor frames (Core A19) and fixed prefixes for the rest. No number is added, and no limit or timeout changes.
- **Test:** no new id. A Core test of `conf::ou_9_baseline_then_live_no_gap` at `route_queue_bytes` = `max_snapshot_bytes` with a snapshot of that size must attach, deliver the baseline and `live`, and not resync.

## R-44. A worker that cannot serve the route it was handed closes it `HandoffFailed`, not `BadFrame` (Stage 1 Core lead, P3 on Core PR #206, 2026-10-09)

- **Question:** Core A17-1 removed `RouteCloseReason::BadPeer`. The worker used it for two defense paths that the host's synchronous checks should make unreachable: (1) `max_frame_bytes` cannot carry an attach frame (the host refuses it under A9-1, and under A19-1 after final38); (2) the worker shares no terminal format with the client (the host refuses it with `Unsupported{terminal_format}`, OU-1). P3 proposed `BadFrame{frame_too_large}` and `BadFrame{invalid_input}`.
- **Answer: both close `HandoffFailed`.** OU-2b defines it: "`HandoffFailed` → `handoff_failed` (the handoff of the connected stream to the worker failed; the reservation ends with this typed reason, TS-9)". In both cases the worker has the stream, but it cannot make it a working route with the options that it was handed. So the handoff did not produce a route.
- **Not `BadFrame`.** OU-2b maps `BadFrame` to `route_closed{protocol_error{code}}`, and the codec's protocol errors describe a frame that the peer sent. In these cases the client sent nothing wrong, so `invalid_input` would blame it. `Other(255)` is not a reason that Core sends.
- **The wire.** `HandoffFailed` is a failed-route reason: the worker reports it to the host, the host reports `route_ended{handoff_failed}`, and the route may lack `route_closed` (OU-2b, TS-9). So the worker sends no frame, which also covers case (1), where no frame may fit.
- **No conformance id.** The host's synchronous checks make both paths unreachable through the public API. A unit test in Core may drive the worker directly. The steward adds no ledger id.

## R-43. The real-Core tier has no testkit-only ids: each control is built from injected parts or real OS acts, and a transcript that asserts a kernel-decided quantity is fixed (Stage 1 Core lead, P6 real probe 3, 2026-10-09)

- **The rule:** A5-4 says that the testkit and real Core with real worker processes "run the same suite and both must pass". So no id is "n/a on the real tier", except the ids that need a real OS condition, which A5-3 and the replacement map list as real-only, and the validated deferred ids. When a control does not hold on real Core, the answer is one of three:
  1. The real harness builds it from injected parts of the real `Core` or from real OS acts on the test's own processes (design 6.3, Testing rule 8: dependency injection, never a test branch).
  2. The probe program gains a step.
  3. The transcript asserts more than the clause requires, and the transcript is fixed.
  Do not add a public Core API, and do not add a timed silence (`real_wait`) as a fence.
- **(A) is revised by R-46:** against real processes, the driver's injected clock moves only by `advance_clock`.
- **(A) Injected-clock transcripts** (`a5_2_due_deadline_processed_in_its_pump`, `st_6_capture_expires_in_pump`, `a2_4_bell_payload`, `a2_4_every_payload_carries_instance_and_at`): the **harness is the host**, and TM-1 says "The host passes a monotonic `Instant` and a unix time to `pump` … Core reads no clock of its own." So `RealCoreHarness` passes the transcript's clock to real Core, with the same start values as the testkit, and `advance_clock` advances it. Exact deadlines and exact `at` values then match. "Real" in this tier means real Core, workers, PTYs, processes, sockets and files. It does not mean that the harness reads the system clock. These are harness work, and the transcripts stay unchanged.
- **(E) `in_9_key_repeat_zero_or_over_the_limit_is_invalid_input`, step 8:** the transcript over-asserts. A real PTY echoes the accepted key's text (the line discipline of the `hold` probe), so `Activity(Output)` is real program output. The step proves ER-0, which says that a sync refusal leaves no event. **Fix:** move the `pump` and the empty `poll_events` (steps 7 and 8) to right after the refusals (after step 4, before the accepted write). Verify the moved check on both harnesses.
- **(6) `pty_output` and `pty_input`:** the probe gains two steps (contracts repo, `botster-conformance-probe`):
  - `relay {path}`: the probe reads the named FIFO, which the harness creates in the test's temp root, and writes every byte to stdout, until the FIFO closes;
  - `log_input {path}`: the probe appends every byte that it reads from stdin to the named file.
  On the real harness, `pty_output` writes to the relay FIFO, and the `pty_input` log reads the input log. These are real OS acts on the test's own processes.
- **(7) `pty_chunk`, `pty_accept` and `pty_blocked`:** the worker process writes the PTY, so no host-side injection reaches it. The probe controls its reads: a `pty_blocked` stop is the probe stopping its stdin reads, so the kernel buffer fills and the worker's write becomes `Partial` (IN-2). **The number of bytes that the kernel accepts is the kernel's.** A transcript must not assert that number as a constant. It asserts the IN-2 outcome: `Partial` or `Complete`, ordered delivery, and the full delivery in the `pty_input` log once reading resumes. **Next step:** the Core lead lists each id and step of this group that asserts an exact accepted count, and the steward rules on each fix. None of these is in the minimum set unless the list shows one.
- **(8) `edges_quiet`:** its definition is "no edge … holds a report that Core has not consumed". On the real harness, the edges are real sockets, pipes and PTY descriptors that Core reads through injected parts. The harness wraps each real edge that it injects. An edge holds an unconsumed report when one of its descriptors is readable (a zero-timeout readiness check, or `FIONREAD > 0`), or when a wrapper holds bytes. `edges_quiet` is true when no wrapped descriptor is readable, no wrapper holds bytes, and `pump` reports no runnable work. This is the same meaning as on the testkit: a report that a worker has not yet written is held by no edge on either harness. It uses no sleep and no Core API. If a transcript depends on `await_quiet` to mean "the worker has finished reacting", that transcript is wrong. Report it.
- **(9) `break_control`:** the real harness injects the process edge, so it holds Core's end of each control link. It breaks the link with `shutdown(SHUT_RDWR)` on that socket. That is a real OS act on the test's own resources: Core reads end of stream, writes fail, and the worker sees its link end. This is the A5-3 asynchronous edge failure. It is not a mechanism inside Core.
- **(5) `no_spurious_wakes`** (`tm_6_begin_and_attach_signal_wake` and 2 other ids): TH-2 allows a spurious `Woken`, and a real wake primitive produces them, so the control cannot hold on real Core. The three transcripts over-assert. TM-6's claims are (i) the call signals before it returns, and (ii) the signal is level-triggered and clears only when `pump` has no runnable work. **Fix:** each step that expects `TimedOut` accepts `TimedOut`, or `Woken` followed by a `pump` that does no work and posts no event (a spurious wake). Each step that expects `Woken` after a call stays. The transcripts no longer list `no_spurious_wakes` in `requires.controls`. The control stays defined.
- **Who does it:** A, 8 and 9 are Core harness work, and no contract text changes. E, 5 and the item 7 fixes are transcript edits in the contracts repo, and 6 is a probe change there. P6 may open those contracts PRs. The crate-catchup reviewer reviews them, and the steward merges them under BUILD.md.
- **R-43(5) follow-up (GS via the Core lead, 2026-10-11). Item (5) was never carried out. It is carried out now in contracts #50.**
  1. **Scope: 8 ids, not 3.** These transcripts list `no_spurious_wakes` on main: `tm_6_create_without_worker_input_wakes`, `tm_6_begin_and_attach_signal_wake`, `tm_6_poll_room_unparks_and_wakes`, `tm_3_wake_handle_ignores_deadlines`, `ev_5_poll_wakes_pending_work`, and three `e3_1_*` ids. Item (5) ruled on the clause (TH-2), so it covers all of them.
  2. **A spurious wake, precisely.** The pump after a `Woken` runs at the same `now`, with no clock step. It counts as doing no work only when `events_posted == 0` and `more == false`. Steps that expect `Woken` after a call stay strict. Under TM-6, only a call that leaves work must signal: for `cancel`, only an `Admitted` result.
  3. **The one exception: `tm_3`.** After the clock step, the deadline's work is due, so the pump is not idle. The form is:
     - `wait_wake` `one_of[Woken, TimedOut]`;
     - `pump` expecting `{events_posted: 1}`;
     - `pump_idle`, which never polls and never steps the clock;
     - `poll_events` expecting exactly `[Silent]`.
     `more` is not asserted, because the testkit's partial progress (A5-2) makes it depend on the seed. The "never signals for a deadline" half of TM-3 is checked only on the testkit. On the real tier it is structural: TM-1 says Core reads no clock.
  4. **Strict `Woken` steps on the testkit.** Without the control, a seeded A5-2 spurious wake could satisfy a strict `Woken` after a call (TM-6(i)), which would be a false pass. Two rules apply:
     - **The driver's rule:** when a transcript has any strict `wait_wake` expecting `Woken`, and the harness has `no_spurious_wakes`, the driver applies that control at the transcript's start. A harness without it applies nothing and refuses nothing. Every other transcript keeps the seeded spurious wakes.
     - **A required negative test:** with seeded spurious wakes on, a harness that drops a call's signal fails the strict step.
     On the real tier, a TH-2 spurious wake that coincides with a missing signal gives a rare false pass. That residual is accepted.

## R-42. A11-1's impostor frames are pipelined behind the rejected handshake: Core reads them after its verdict (Stage 1 Core lead, P5 `impostor_worker`, 2026-10-09)

- **Question:** A11-1 lets the A10-1 impostor send worker frames "once Core's real AD-6 check has rejected its handshake", and the controls doc says the script frames are "sent only after Core's real AD-6 check has rejected the handshake". Core's rejection closes the link, so an impostor that waits to learn of the rejection can never deliver a frame.
- **Answer: (a), pipelined.** "After the rejection" is the order in which **Core** meets the frames, not the time at which the impostor writes them. The impostor writes the script frames right behind its handshake answer, **before** Core's verdict, so they are in the link when Core's real AD-6 check rejects the handshake. The test proves that no frame that follows a rejected handshake on the same link changes any session state, report or event.
- **The text:** A11-1 says "No frame from a link that failed AD-6 changes any session state, report or event". Under reading (b), no such frame could ever reach Core, so the clause and its tests would assert nothing. A clause is read so that it has content.
- **Mechanism stays free.** A11-1 says "The test asserts this as an outcome, not as a mechanism". Core may decode the frames and drop them, or close the link and discard them unread. Both pass.
- **The test must not pass vacuously.** The testkit writes the handshake answer and the script frames in one write on the link, and the kernel must accept the whole write before Core reads the link. If that write fails or is short, the testkit reports a setup failure, not a pass.
- **Doc correction:** `docs/core-testkit-controls.md`, the `impostor_worker` row: "sent only after Core's real AD-6 check has rejected the handshake" reads "written in the same write as the handshake answer, before Core's verdict, so that Core meets them after its real AD-6 check has rejected the handshake (steward ruling R-42)". No contract text changes.

## R-41. A clipboard location outside A13-1's three cannot occur at the pinned model; if the binding ever yields one, the write is reported lost, never lettered (Stage 1 Core lead, P3 PR #199, 2026-10-09)

- **Fact (R-7, the pinned model):** `GhosttyClipboardLocation` has exactly `STANDARD`, `SELECTION` and `PRIMARY` (fork `ghostty-fork-a6`, `include/ghostty/vt/terminal.h`). `GHOSTTY_CLIPBOARD_LOCATION_MAX_VALUE` is only the ABI sizing sentinel, and the model never produces it. A binding value such as `ClipboardLocation::Other` is a catch-all for values that the pinned model does not produce.
- **(1) The outcome:** A13-1 defines a letter for each location the model produces, so no contract behaviour exists for a fourth location. If the binding ever yields one, Core must not invent a letter and must not be silent:
  - it posts no `ClipboardWrite`, and it records the loss with the defined marker `EventsLost{id, kinds}`, where `kinds` contains `ClipboardWrite` (A2: `EventsLost.kinds` lists `ClipboardWrite`; EV-6: a loss always leaves its marker);
  - an OSC 5522 write still gets exactly one acknowledgement (A13-1b), with status `IO_ERROR`. That is the only refusal status Core may send: A13-1b says "Core never answers DENIED, UNSUPPORTED or BUSY by its own choice";
  - the case has no conformance id, because the pinned model cannot reach it.
- **(2) The name:** there is no `Lost{ClipboardWrite}` kind. The loss marker is `EventsLost{id, kinds}` (6.2, EV-6). A new event or `Lost` variant would be an amendment, so P3 must use `EventsLost`.
- **A model that adds a location** is a pin change that alters A13-1's mapping. It needs a Core amendment that gives the new location a letter, before the pin moves.

## R-40. PD-6's `found` lists the out-of-range providers, as `[{provider, interface_version}]` (crate-guard pair, HP PD-6, 2026-10-09)

- **Answer:** in `conflict{detail.code: requirement_conflict, interface, range, found}`, `found` is **a non-empty list of `{provider, interface_version}`**. There is one entry for each provider of `interface` that resolution considered (installed, or chosen in the same operation), none of whose `interface_version` is in `range`.
- **The text:** PD-6 says that `requirement_conflict` means "**no provider in range**", and that `requirement_missing` covers "a missing required interface". So a conflict always has at least one provider, and none is in range. "`found`" names what resolution found: those providers.
- **The names** are the lockfile's (PM-3: `{interface, interface_version, provider, provider_version, interface_hash}`). `provider` is the package key, and `interface_version` is the version that the provider's `provides` declares. PD-6 says that "compatibility … use[s] `interface_version`", so the entry carries that version, not `provider_version`.
- **Order** is not fixed. A test compares `found` as a set.
- The CLI prints these fields (CLI 34, "print their fields").

## R-39. The pairing bundle's base32 is RFC 4648 section 6, upper case, read strictly (crate-guard pair, HC PR-2, 2026-10-09)

- **Answer:** PR-2's "`base32(…)` without padding" is the base32 of **RFC 4648 section 6**, with the alphabet `A`–`Z` and `2`–`7`. It is not base32hex (section 7) and not a lower-case variant.
- **The writer** emits upper case and no `=` padding.
- **The reader** accepts the section 6 alphabet only. A lower-case letter, a `=`, or any other character outside the alphabet is refused as a malformed bundle. RFC 4648 section 3.3 says to reject characters outside the alphabet unless the referring specification says otherwise, and PR-2 says nothing otherwise.
- **Why one answer:** `conf::pr_2_bundle_codec` needs one encoding and one acceptance set. Accepting either case would add behaviour that PR-2 does not state. Upper case is also the QR alphanumeric character set, so a pairing QR code stays compact.

## R-38. `hold_start_at before=running` holds the completion of `Start`; no `Running` row write is required (Stage 1 Core lead, P5, 2026-10-09)

- **Answer:** the hold point is after AD-7 step 4 (the order to launch the payload) and before `Start`'s `Completed` is posted. The stored row is still `Starting` there.
- AD-7 lists four steps, and none of them writes a `Running` row. AD-1 adopts a `Starting` row "whose worker identity is recorded and authenticates" as `Running`, so a fresh start needs no `Running` write.
- The control row's parenthesis "(the `Running` row write and the completion of Start)" describes; it does not require. If an implementation writes a `Running` row on a fresh start, the hold also comes before that write.
- The one transcript that uses it, `conf::ad_1_starting_with_identity_adopts`, asserts only `state: Starting` with `identity_recorded: true` at the hold. Then it drops the handle and adopts.
- The control row's wording is to read: "`running` holds the step after the payload launch: the completion of Start, and a `Running` row write if the implementation makes one".

## R-37. An allocation-peak observer is allowed for DP-3, on R-14.3's terms (crate-catchup pair, for `conf::dp_3_frame_limit_checked_before_allocation`, 2026-10-09)

- **Answer: yes, `alloc_window` and `alloc_peak -> {bytes}` are allowed without an amendment.** They do not fall under R-14.3's letter, because R-14.3 names the link boundary only. They do fall under its reason.
- **The reason R-14.3 gives:** A5-1 lists the testkit's **edges**, the things a test scripts or injects. A read-only measurement injects nothing and changes no order, so it is not an edge, and it keeps the one-code-path rule (A5-1, A5-4).
- **DP-3 needs it:** "The limit is **checked before any allocation**". The p4a-2 finding showed that a consumed-bytes check cannot see an allocation. An allocator observer is the only boundary where an allocation is visible.
- **Conditions** (R-14.3's four, adapted):
  1. it observes at the **process allocator boundary** (a `#[global_allocator]` wrapper around the system allocator in the test binary), and never instruments worker code. It observes the process in which the worker's route reader runs; if that reader runs in another process, the observer proves nothing and must not be used;
  2. it adds no wait, lock or synchronization point: one atomic `fetch_max` per allocation, which cannot change the order of frames;
  3. it lives only in `botster-core-testkit`;
  4. its documentation (`docs/core-testkit-controls.md`) names DP-3 as the clause it observes.
- **What a transcript may assert with it:** only a bound far from every legitimate allocation. The peak is process-wide, so it includes the testkit's own allocations. For example, with a declared frame length of `0x7FFFFFFF` on a route whose `max_frame_bytes` is 4096, `alloc_peak.bytes < 2147483647` together with the typed close `frame_too_large`. It must never assert an exact or small peak.
- **The slow-tier `RLIMIT_AS` proof is not needed.** It may be added, Linux only, as extra evidence.
- **Follow-up (the same day, the pair and its reviewer): what DP-3's "any allocation" covers.** It covers an allocation **for the frame**, that is, an allocation whose size depends on the frame's declared length. A buffer whose size does not depend on the declared length (a fixed read buffer) is not an allocation for the frame, and DP-3 does not forbid it: it cannot grow with a hostile length.
  - **The preferred proof is differential.** Run the same script twice: once with a declared length just over `max_frame_bytes`, and once with `0x7FFFFFFF`. Assert that the second run's peak is not larger than the first run's. This needs no absolute number, and it also catches an allocation that depends on the length but is capped.
  - **The fallback**, if the two peaks are not reproducible: the far bound (for example `alloc_peak.bytes < 2^28` with `0x7FFFFFFF`). Record in the transcript's notes the gap it leaves: an allocation that depends on the length but stays below the bound.

## R-36. A successful `Adopt(id)` retry follows the worker's report; a `Lost` row keeps identity, not intent (Stage 1 Core lead, P5 reviewer on PR #174, 2026-10-09)

- **Q1: yes. The worker's own report decides, whatever the row's state was before `Lost`.**
  - Core A2-1, `Adopt`: "`SessionRecord` (its state tells the outcome: `Running`, `Exited` or `Lost(reason)` again)", admitted in `Lost(WorkerUnreachable)` and `Lost(WorkerVersion)`. AD-3: after adoption "the state is re-read from the worker".
  - So when the worker now authenticates (AD-6): the payload is running → `Running`; the payload has ended → `Exited` (a final-model worker, AD-6); otherwise → `Lost(reason)` again, with the reason that applies now.
  - Nothing is relaunched (AD-2: "A `Lost` session is never restarted in place").
- **Q2: no. A `Lost` row keeps the worker's identity (AD-2, AD-6), not the earlier state.**
  - `Adopt`'s outcome set has no `Stopping`. Keeping an intent so that a retry resumes a stop would add an outcome and a durable field, which is an amendment.
  - It is also not needed. The `Stop` that ended in `Lost` has completed, and the host saw its outcome (LC-5: "The completion arrives after the session reaches `Exited`/`Lost`"). The host keeps its own intent. If it still wants the payload ended, it calls `Stop` after a successful `Adopt` (`Stop` is admitted in `Running`).
  - **The reviewer's F24** (a broken-link `Stop` keeps `Stopping` durably instead of `Lost`) would change LC-5's outcome for that path. It is not required, and it would be an amendment.
- **Q3: an `Adopt(id)` of a `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` row is always admitted.** Its result is Q1's, and no kept intent is needed. Refusing it, as PR #174 does for a row with no kept intent, does not meet A2-1. `WrongState` is only for a row in another state.
- **Follow-up (the same day): the worker authenticates but reports no running payload.** Nothing is ever launched on a `Lost` row (AD-2: "never restarted in place"). This differs from R-35 (a), where the row was `Starting`, not `Lost`.
  - **(i) The payload was never launched:** `Lost(StartInterrupted)`. AD-1 defines that reason for a `Starting` row with no recorded identity. AD-2 lists it as the reason for a start that never reached its payload, and that is this case too: the start stopped before AD-7 step 4. No new reason is needed.
  - **(ii) A launch is in progress:** wait for the spawn's answer, with no `Launch` sent. The worker's own `startup` deadline bounds the wait (TM-3), as in R-35 (b). Then the outcome is `Running`, or (iii).
  - **(iii) The launch failed:** the same outcome as a failed ordinary start (LC-4), as R-35 (a) sets for `AdoptAll`.

## R-35. Adoption completes AD-7's launch for a `Starting` row whose payload never launched (Stage 1 Core lead, P5, 2026-10-09)

- **The case:** AD-7 allows a crash after step 3 (the worker's identity is durable) and before step 4 (`Launch`). That leaves an authenticated worker with no payload, which "exits by itself when **no host attaches** within `CoreLimits.startup`". No clause names what an adopting host does when such a worker answers that the payload was never launched.
- **Answer: P5's readings (a) to (d), with (a)'s failure outcome tied to LC-4. No amendment.**
  - AD-7's wording assumes that a host which does attach completes the sequence. Otherwise "no host attaches" would be no condition at all. AD-1 adopts a `Starting` row with a recorded, authenticated identity "as `Running`, or `Exited` if the payload already ended". It keeps `Lost(StartInterrupted)` for a `Starting` row with **no recorded worker identity**. So a `Starting` row with an identity is never `Lost(StartInterrupted)`.
- **(a) A `Starting` row, and the worker reports that the payload was not launched:** the adopting Core performs AD-7 step 4 itself: `Launch`, from the durable row. The payload still starts only after a durable identity, so AD-7 holds.
  - Launched: the row becomes `Running`. Its one `SessionState` (AD-1) is posted at that answer, so `Running` is never posted before the payload runs.
  - The launch fails: the row takes **the same outcome that a `Start` whose launch fails takes** (LC-4: "`Exited`/`Lost`, never `Starting`"), with the same state and cause that Core's ordinary start path uses. Adoption invents no separate value. No `Start` operation exists in this host, so the `SessionState` is the only report.
  - `AdoptAll` completes after every row's `SessionState` (LC-11), so it waits for this answer. The worker's own `startup` deadline bounds the wait (TM-3).
- **(b) `Starting`, and the launch is in progress:** no second `Launch`. Core waits for the worker's answer, then applies (a).
- **(c) A `Stopping` row (a `Stop` admitted in `Starting`), and the payload was never launched, or the launch failed:** no `Launch`. Nothing runs, so the re-issued stop (AD-1) completes at once. The row becomes `Exited` with cause `HostStop`, and no code and no signal: the session ended because of the host's stop.
- **(d) A `Running` or `Exited` row, and the worker reports no payload or a launch in progress:** the registry and the worker disagree, and the row cannot be recovered ("every other row that cannot be recovered becomes `Lost(reason)`", AD-1). It becomes **`Lost(RegistryCorrupt)`**. AD-7 makes this case unreachable, so it is defensive and needs no transcript.
  - **Corrected the same day** (the integration reviewer, through the Core lead). The first text said `Lost(Other)`. AD-2 names `Other` only as the value **a host** maps an unknown reason to, not a reason Core reports. The worker's identity is authenticated (AD-6), so the durable row is the record that is wrong: the registry's record is corrupt, even though its bytes decode. `RegistryCorrupt` is the listed reason for that.
  - **Core never reports `Lost(Other)`.** Every `Lost` that Core posts carries a listed AD-2 reason. A placeholder that posts `Lost(Other)` for adopted rows does not meet AD-1, which gives each recorded state its own outcome.
- **Tests:** (a) and (c) are worth transcripts, through the process edge's script point between AD-7 steps 3 and 4 (Core A5-1). They are proofs of AD-1 (`conf::ad_1_starting_with_identity_adopts`) and AD-7 (`conf::ad_7_crash_between_steps_leaves_no_unregistered_payload`), so no new id is needed.

## R-34. User rulings of 2026-10-05: the key-text bound, and how the Web client sends keys (relayed by the orchestrator)

- **Core A15's numbers (a user ruling):** "`CoreLimits.max_key_text_bytes` default 256, configurable range 1 to 4,096." This is the authority that Core Amendment 15 candidate 2 cites for its numbers. The Amendment reviewer requires a user ruling for any new numeric bound without existing authority (verdict `d5ee329`).
- **The Web client's key input (a user decision, recorded; no contract change):** the Web client encodes keys itself, with restty, against the route's `modes` frames, and sends them as `bytes` (`0x83`). The worker's semantic key encoding (IN-9, 5.1A) stays for Hub and plugin writes and for the TUI.
  - No clause requires a client to send `key` frames. `bytes` is an existing route input, written as it is (DP-3).
  - R-7 governs Core's terminal model and encoding. It does not choose the encoder of a client that sends `bytes`.

## R-33. MIME types that the model ignores are not in the decoded size (Stage 1 Core lead, A14 confirmation, 2026-10-04)

- **The fact:** the pinned model ignores the data of MIME types past its per-write type-count limit (64). It never decodes or keeps that data.
- **Answer: reading (a).** Those types are excluded from the decoded size and from the limit, everywhere.
  - A14-1 defines the decoded size as "the number of payload bytes **the terminal model decodes** for it". Which bytes the model decodes is the model's decision (R-7, as in R-28). Bytes it ignores are not decoded, so they are not counted.
  - A14-3's guarantee is about **retained payload**. Ignored data is never retained, so counting it protects nothing.
  - Reading (b) would need a model change that decodes data only to count it. No clause requires that, and Core must not add decoding of its own (R-7).
- **Consequences:**
  - A write whose kept types are within the bound is not `TooLarge` because of ignored types.
  - The ignored types are absent from `contents`, as the model reports the write. Their bytes stay in `Output` (OU-12), so a client that acts on the raw sequence still sees them.
  - Invalid base64 in an ignored type is not counted either.
- **The R-32 fork patch** counts what the model decodes past its limit. It must not start decoding ignored types to count them.

## R-32. libghostty's own OSC 5522 transaction limit must report to the worker; the contract stands (Stage 1 Core lead, audit A6, issue #148, 2026-10-04)

- **The facts (fork `3f8eb68`):** above its own 64 MiB transaction limit, libghostty drops an OSC 5522 clipboard write without calling the clipboard callback, and it answers the program `EFBIG` itself.
- **Both outcomes break frozen text:**
  - EV-3 (as A13-2 replaces it): a write over `CoreLimits.clipboard_bytes` "is **never silent**". The 9B row of `clipboard_bytes` says "a larger one is reported, never dropped silently". EV-7 makes a feature that reaches the worker but not the host a conformance failure. A write the model rejects without a callback reaches no host.
  - A13-1b: the worker is the only acknowledger of OSC 5522. A second acknowledgement from the model breaks the one-acknowledgement rule (`conf::a13_1b_one_acknowledgement_*`).
- **Answer: (c). libghostty changes, through a fork patch under the user's fork policy. No amendment.**
  - The fork's clipboard callback is already a Botster patch (the A13 facts). The patch extends it: a write over the model's own limit **still calls the callback**, with no contents, the location and selection as for any write, and the write's **full byte length**. The model counts the length after it stops buffering, which costs no memory. The model **sends no acknowledgement of its own**: the callback's reply is the only one, as for every other write.
  - The worker then follows A13 unchanged. 64 MiB is above any `clipboard_bytes` up to that size, so the write is posted as `ClipboardWrite{contents: none, total_bytes, reason: TooLarge}`, and the worker acknowledges `IO_ERROR` (EIO).
  - If `clipboard_bytes` is ever set above the model's limit, a write between the two is still `TooLarge`: the model cannot carry it. The patch's report is the evidence, and Core never invents contents.
- **Why not (a):** it would adopt a silent drop, and a second acknowledger, against EV-3, the `clipboard_bytes` row, EV-7 and A13-1b. Those are transparency guarantees, not details. Weakening them is a scope decision for the user, and nothing here asks for it.
- **Why not (b):** to bound the write before libghostty, Core would have to recognize OSC 5522 in the byte stream. That is a hand-written terminal parser, which R-7 forbids.
- **Until the patch lands:** the over-limit case has no conforming outcome. A test of it stays pending, with this ruling as its reason. It is not a mismatch of any other id.

## R-31. The every-cut helper may reuse one session per corpus item; offsets are never sampled; the corpus needs one beyond-limit string per kind (Stage 1 Core lead, audit A12 and A63, 2026-10-04)

- **Reuse: yes, one session and one feed per corpus item.**
  - ST-6 makes a capture a read, "frozen at `model_rev`". It never changes the model. So the model after k bytes of one feed, with captures at earlier offsets, is the model of a fresh session fed the same k bytes. A fresh session per cut proves nothing more.
  - **The method:** feed the item through the program edge one cut at a time. At each offset k, fence, then `CaptureSnapshot`. Read every page into the harness, and `release` the capture before the next cut, so the held bytes stay within `snapshot_retained_bytes` (A8-1) and no cut is refused with `CaptureLimit`. After the last byte, the session's model at the end of the item is the comparison target for every cut.
  - **The oracle side:** for each offered capture at k, a fresh oracle loads its pages, applies the item's bytes from k to the end, and is compared with that target. By determinism, the helper may instead stop at the first offset j > k where both the session's model and the oracle have no pending sequence. It compares there with a **reference oracle** fed the item's first j bytes from a fresh terminal, never with a capture of the subject, which would be circular. The reference is the same libghostty with the session's settings, so it is the model's state at j (R-7). Equality of the ST-6b state at j implies equality at every later byte, because ST-6b covers "any state that changes how a later byte is interpreted".
  - Conditions (1), (2) and (4) of the control and R-30's fit method apply unchanged at each cut.
- **Sampling: no.** A8-2 names "the every-byte-offset test", and ST-6b's invariant holds "at every cut point". Every offset from 0 to the item's length is a cut.
- **The corpus that A8-2 requires is small.** It requires "partial sequences both within and beyond the limit", with every snapshot fitting `max_snapshot_bytes`. That is met by:
  - the ordinary items with partial escape sequences (CSI, OSC, DCS, APC, and UTF-8 split across a cut); and
  - for each kind in `beyond_limit`, **one** unterminated string whose payload is the continuation limit plus one byte. Its offsets cover the limit itself, the last offset within it, and the first offset beyond it.

  "Each generated string longer than the limit" in the control's text means this one string per kind, not a family of lengths or contents. Correct the control text.
- **If the cost is still too high:** the cost is now about the sum of the item lengths in Core captures, and, for the oracle, about the square of the beyond-limit string's length. If the format's continuation limit makes that too large for a gate, report the limit and the measured cost. A bounded tier is an amendment question, not a choice for the helper.
- **Tags (A63):** a ruling binds from its commit on main. It never changes a contract, so it needs no tag. The orchestrator cuts the `contracts-v` tags (orchestrator handoff). Until a tag carries a ruling, cite it as "R-n (main <commit>)".

## R-30. A snapshot's fit is established by an exact oracle measurement; no pending-state size bound is published (Stage 1 Core lead, P6, 2026-10-04)

- **Q1: no contract publishes a bound on a pending state's encoded size, and none is required.**
  - Core A8-2 requires one stated value: "the `snapshot_format` description names the format's continuation limit, the largest pending sequence state it carries". The format's description is the format spec in the protocol crate (ST-6: "the decoder and the spec live in the protocol crate"), not a field of the `SnapshotFormat` type. A tag needs to add nothing to `SnapshotFormat`.
  - If the GHOSTSNP spec at a tag does not name the continuation limit, that is a Core conformance gap against A8-2 ("the exclusion is stated"). The owner of the protocol crate fixes the spec. A constant in code alone does not meet it.
  - The phrase in `docs/core-testkit-controls.md`, `oracle_resume_every_cut` condition (3) ("the maximum size that the `snapshot_format` description names for a pending state") names a value that no clause defines. It is a defect in the control's text, not a contract obligation. Correct the control to the Q2 method.
- **Q2: yes. The helper may establish fit by exact independent measurement.**
  - At each cut, it encodes the separate libghostty oracle (R-7). It uses the same format and version as the session's `snapshot_format`, the same screen size and history settings as Core's model, and an oracle retention at or above the input length. It takes the native encoded length and adds the page framing that the format spec defines for that length. The framing is computed from the spec, never read from the subject's capture. The cut fits when the total is at most `max_snapshot_bytes`.
  - This is independent of the subject's answer, as condition (3) requires, and it is stronger than a bound.
  - **Per-capture data:** A8-1 makes no equal-size promise ("the format may carry per-capture data"). The measurement counts every per-capture field of the format at its largest size that the spec allows.
  - Where the oracle cannot encode the state, or the format is unknown, the cut stays `inconclusive`, never a mismatch.
- **What an `inconclusive` cut proves:** nothing. A8-2's every-byte-offset test constrains its corpus "so that every snapshot fits in `max_snapshot_bytes`". So a run of `conf::st_6b_resume_invariant_holds_at_every_byte_offset_of_a_corpus_with_partial_escape_sequences` with an `inconclusive` cut whose pending state is within the continuation limit does not pass that id. Choose the corpus so that no such cut is inconclusive.

## R-29. The Hub→client wire members that the clauses name but do not place (Stage 0 lead, hc-groups pair, 2026-10-02)

- **Answer:** the crate's assumed shapes are confirmed. No amendment is needed: each one fills a member name or a unit that no clause fixes, and none contradicts frozen text. The crate drops its "pending steward ruling" marks.
- **The envelope frame name (EN-1, EN-2)** is the member **`type`**: `{"type": "hello" | "req" | "cancel" | "welcome" | "res" | "watch" | "event" | "close", …}`. No clause names this key. A route connection's first frame is TB-L2's literal `{"frame": "route_connect", …}`, so a listener tells the two connection kinds apart by `type` or `frame` on the first frame.
- **H2(a) Watch sub-frames (WT-1 to WT-5):** a `type: watch` frame names its sub-frame in **`frame`**: `snapshot_begin`, `snapshot_items`, `snapshot_end`, `upsert`, `patch`, `remove`, `batch_begin`, `batch_items`, `batch_end`, `watch_closed`, with the members that WT-1 to WT-5 give. Each change inside `batch_items.changes` carries the same `frame` tag (`upsert`, `patch` or `remove`). This follows the TB-L2 precedent.
- **H2(b) `event_gap` (EV-4):** EN-2 allows only its listed server frames, so the marker is an `event` frame: `{"type": "event", "frame": "gap", stream_id, owner, name, subject?, count, lost_bytes?, recovery, state_read?, aggregated?}`. A plain event has no `frame` member.
- **H2(c) Pairing and signalling (TB-L2, PR-6, PR-7):** each frame is `{"frame": "<snake_case name>", …}`, as TB-L2's `route_connect`: `auth_challenge`, `pair_request`, `auth_response`, `auth_accepted` and the `signal_*` frames.
- **H2(d) `if_rev` (VB-4):** it is an argument, `req.args: {patch, if_rev?}`. EN-1 gives `req` no `if_rev` member, and A.9 makes only `operation_id` and `idempotency_key` request fields "never arguments". This matches MCP MC-2 `update {ref, patch, if_rev?, …}`.
- **H2(e) Timestamps:** every integer wall-clock time (`at`, `since`, `created_at`, `last_seen_at`, `plugin_log.at` and the like) is **Unix milliseconds**, as HP WF-5 says for wall-clock times that cross a boundary. **The one exception is `expires_at`**, which is Unix seconds (PR-7b). That includes the `expires_at` of the `issue_pairing` result (section 9.3). A member whose name ends `_ms` (`ttl_ms`, the limits) is a duration, not a time.
- **H3 `spawn_target.rescan_packages` (A.7b, HC A2 A.9):** the text already fixes lists. The operation row ends `succeeded` with `result: {found: [package key], removed: [package key]}`, in the same place as `hub.update`'s `result {version}`.
- **H4 `rev` (VB-2, VB-4, section 5.1, HS-5):** `rev` is an **opaque string member of each resource row**, a core field present on every row that `get`, `update` and `list` return, and on every watch `snapshot_items` and `upsert.value` row, so that `if_rev` works from a watched copy. It is compared only for equality (section 5.1). It is not `software.revision`. A.8's list of core-field types does not name `rev`. Its type is the string that section 5.1 makes opaque.

## R-28. Kitty flags, release events and zero output follow the terminal model (Stage 1 Core lead, P3, 2026-10-02)

- **(1) Unknown kitty flag bits (`conf::in_9_kitty_unknown_flag_bits_ignored`): the terminal model decides, and no fork patch is needed.**
  - 5.1A rule 2: "the active value is the program's flag-stack top masked to the known bits `0x1F`; bits above that are ignored and never acted on, and an unknown mode is never an error."
  - "The program's flag-stack top" is the stack **as the terminal model holds it**. Which sequences change that stack is the model's decision (R-7). Core never parses `CSI = … u` itself.
  - The pinned model (fork `85a8d8e`) ignores `CSI = 63 ; 1 u` as a whole. So the stack is unchanged, `kitty_flags` stays `0`, and E2-3 posts no `ModesChanged`. That outcome meets every part of rule 2: the unknown bits are never acted on, and nothing is an error. The mask is applied to whatever value the model holds.
  - **The transcript is wrong** under either reading: it expects `kitty_flags: 63`. The fix: after the write, assert no `ModesChanged` and `kitty_flags: 0` in `terminal_state`, and that a following key encodes as legacy. The test keeps its name.
  - **Follow-up (016c's R-76 audit, `.review-verdicts/r76-scripted-audit-8d79b70d.md`, 2026-10-11): (1) was still unimplemented at `8d79b70d`. It is carried out in the oracle form (contracts #55).** At `8d79b70d`, step 4 still expected `ModesChanged{kitty_flags: 63}`, a hand-written terminal value (R-7). After the sequence, with an R-57 title fence:
    1. `forbid_events` `ModesChanged`;
    2. a `terminal_state` whose `kitty_flags` equals `oracle_state`'s `modes.kitty_flags`, with no bit above `0x1F`;
    3. the next key equals `oracle_encode` with the **reported** flags.
    The oracle gives this ruling's `0` and legacy at this pin, with no literal. **The `forbid_events` step is pin-specific** (017f's F2 on #55): a pin whose parser masks `63` to `31` would post a correct `ModesChanged{31}`, and the step would fail. So a pin roll that changes this parser behavior must change the transcript. That is a known adaptation, not a Core defect. A draft "R-79" for this case was withdrawn in favor of this follow-up, so the number R-79 is not used.
- **(2) `conf::in_9_kitty_all_32_flag_combinations_pinned`, step 4: a transcript defect.** `CSI = 0 ; 1 u` while the flags are already `0` is no net change. E2-3 says no event is posted. The step must not wait for a `ModesChanged`. It asserts the flags with `terminal_state` instead.
- **(3) `conf::in_9_kitty_each_flag`, step 25: a transcript defect.** With only flag 1, flag 2 ("report event types") is off. The kitty protocol then reports no release events, so the modes say no. 5.1A rule 5 makes that `NotWritten(NotReported)` with zero bytes, not `Written`.
- **(4) A release in legacy mode is `NotWritten(NotReported)` (`conf::a2_2_unsupported_what_values`, step 18 is a transcript defect).**
  - 5.1A rule 1 is the specific rule: "`Release` and `Repeat` kinds are **not reported** in legacy mode: a `Release` is typed zero output". "Not reported" is rule 5's `NotReported` ("modes say no"). Rule 5's list of examples covers both outcomes and does not override rule 1.
  - This is the same case as (3): the active modes do not report event types.
  - Core A2-2's `release_event` stays a listed `what` value (the list is non-exhaustive). No v1 stimulus produces it, and step 18 must assert `NotReported`. The test proves the other `what` values.

## R-27. The Lua binding fixes the refusal the plugin sees, not where it is checked (Stage 0 lead, lb-cli-suite reviewer, 2026-10-02)

- **Answer:** the second reading. No amendment is needed.
  - LB-2 ("Unknown keys are `invalid_request`"), LB-15 (a missing `web.set_origin.origin` "fails `invalid_request`") and LB-11 (an HTTP method outside its seven) each fix the **outcome the plugin sees**: the call returns the LB-3 envelope `{ ok = false, error = { kind = "invalid_request", effect = "none", ... } }`, and nothing takes effect.
  - None of them says the host must check **before** sending, and HP LN-1 maps refusals to returned values without naming who refuses.
- **So a conforming Lua host may either:**
  - check locally and send no `HostCall`; or
  - forward the request and map the Hub's `invalid_request` into the envelope.
- **The host owns the outcome.** Whichever it chooses, the plugin must get that envelope, with effect `none`. If forwarding would not produce it (for example, if the Hub ignored the unknown key), the host must check locally.
- **Transcripts assert the envelope** (kind, effect `none`, and `detail.code` where a clause names one), **never the presence or absence of a `HostCall`**.
- This differs from R-23, where the Lua host has no part in the outcome at all. Here the host is the subject, and only its observable result is fixed.

## R-26. MP-12's descriptor bounds come from the parameters it names (Stage 0 lead, hp-plugins batch 2 reviewer, 2026-10-02)

- **Answer:** no new parameter and no amendment. HP LM-1 says "a clause names the parameter". MP-12's two bounds without a name of their own follow from the parameters it does name.
- **(a) The bytes of one descriptor:** a descriptor arrives inside one server message, so it is bounded by **`mcp_proxy_message_bytes`**. A message over that bound marks the server `unavailable{detail.code: mcp_bound_exceeded}`, and its list is discarded whole (MP-12). There is no separate per-descriptor parameter, and a test of the descriptor-bytes bound is a message-bytes test.
- **(b) The descriptor count:** **`mcp_proxy_tools_max` bounds the tools of one server.** The per-plugin total follows from it: at most `mcp_servers_max` (servers per plugin) × `mcp_proxy_tools_max`. There is no second parameter.
- **The pair's case is not the clause's behavior.** With `mcp_proxy_tools_max = 3`, server p with 2 tools and server q with 2 tools are **both** within their own bound, and both are listed. **The refusal case:** one server lists more than `mcp_proxy_tools_max` tools across its pages. That server is marked `unavailable{detail.code: mcp_bound_exceeded}`, its whole list is discarded (none of its tools is offered), and the other servers are unaffected.
- **Tests:** `conf::mp_12_descriptor_count_and_bytes_bounded` uses one server over `mcp_proxy_tools_max`, beside a server within it, and one message over `mcp_proxy_message_bytes`.

## R-25. CLI golden outputs cover only what the CLI spec fixes (Stage 0 lead, lb-cli-suite pair, 2026-10-02)

- **Answer:** option (b). No amendment is needed. The CLI spec is informative (README), and BUILD.md testing rule 3 says every test proves a clause. A golden file that freezes human-readable text that no clause fixes would make an implementation choice into contract.
- **`conf::cli_cv3_golden_outputs`** compares against golden files only for output whose content the spec fixes:
  - the JSON outputs (`json-*`, fixed by structure);
  - `config` and `context` (no key);
  - the one literal line of CLI-SS1, `running binary: dev <sha> | release <version>`.
- **Human-readable `status` (CLI-SS1) and `list-worktrees`:** the conformance tests assert **content, not layout**:
  - every item CLI-SS1 names appears (socket, running, protocol, hub_id, pid, uptime, sessions, plugins);
  - the right one of the five diagnosis cases is reported;
  - a detached worktree is marked as detached.

  They assert no line format, diagnosis wording, uptime format or marker text. Where a JSON form exists, assert values there.
- **The CLI repo** may keep its own snapshot tests of its human-readable output as regression aids, outside the conformance suite.
- The same holds for the `conf::cli_ss1_*` ids: no line layout is assumed.

## R-24. Which MCP tool descriptors cannot be carried losslessly (Stage 0 lead, hp-plugins batch 2 reviewer, 2026-10-02)

- **Answer:** MP-13's "a tool whose descriptor the Hub cannot carry losslessly" means a descriptor that the Hub's own JSON wire rules cannot represent unchanged. Two cases are fixed by existing clauses, so no amendment is needed:
  1. **A duplicate object key** anywhere in the tool's descriptor (for example inside `inputSchema`). HP WF-2 refuses duplicate keys, and HC A.1 makes them `malformed_frame` on the client wire, so the Hub cannot pass the descriptor on unchanged.
  2. **An integer literal outside ±(2^53 − 1)** in the descriptor (for example `"maximum": 9007199254740993`). HP WF-3 and HC A.1 bound JSON integers to that range.
- **Scope of the omission:** only that tool is omitted (MP-13), and it is reported in the diagnostics. The server's other tools are listed, and the server's message is not rejected as a whole.
- **Tests:** `conf::mp_13_uncarriable_descriptor_omitted` uses both cases, each beside an ordinary tool that must still be listed.

## R-23. `hub/route.ended` is refused by the Hub, not the Lua host (Stage 0 lead, lb-cli-suite pair, 2026-10-02)

- **Answer:** the Lua host has no obligation of its own for LB-21's "a plugin cannot subscribe to it and no grant admits it". LB-6's `botster.events.on` only registers the handler and adds the subscription to `Ready`. It refuses only a call after `Ready` (`registration_after_ready`). The refusal is the **Hub's**, at registration (HP EV-2, EV-9, CP-4).
- **So the proof of `conf::lb_21_route_ended_is_client_only` moves to the Hub** (hub-inproc). A plugin whose `Ready` subscribes to `hub/route.ended`, with any grant, fails registration before `Activated` and never receives a copy. The test asserts the failure kind that CP-4 gives for a subscription that no grant admits. If no clause names a code, it asserts only that the load fails.
- No amendment is needed: LB-21 states the rule, and the Hub clauses own its enforcement.

## R-22. `relay.deliver` to an ended attempt is `rejected`; `not_found` is only for another plugin or an earlier instance (Stage 0 lead, hp-manifest pair, 2026-10-02)

- **Answer:** no amendment is needed, and HP A2-18 and WP-2 agree.
  - WP-2 makes `not_found` exactly two cases: "a `relay_id` of **another plugin** or of **an earlier instance**".
  - A2-18 gives `rejected` (effect `none`) when "the attempt has ended (completed or expired)".
  - Retiring an attempt ends it. It does not turn its id into one of WP-2's `not_found` cases while the instance that minted it is alive.
- **So, for `relay.deliver{relay_id}`:**
  - **live attempt:** `accepted`, effect `committed`;
  - **completed (the PR-4 outcome) or expired (`relay_attempt_ttl_ms`), same instance:** `{status: rejected}`, effect `none`;
  - **after the plugin unloads, crashes or reloads (LD-9):** the id belongs to an earlier instance, so `not_found`;
  - **another plugin's `relay_id`:** `not_found`.
- **Consequence:** the landed transcript `conf::wp_2_attempt_retired_on_completion_expiry_and_plugin_lifecycle` must assert `rejected`, not `not_found`, for completion and expiry. It keeps `not_found` for the lifecycle cases. `conf::a2_18_relay_deliver_accepted_rejected_effects` uses the completed and expired cases.

## R-21. Three Hub→plugin event and net test stimuli (Stage 0 lead, hp-events pair, 2026-10-02)

**R-21.1: the session-family snapshot has no fixed chunk count.** EV-6 and HP A2-28 fix the frames and "bounded `snapshot_chunk`s … at one sequence", not a count. A transcript asserts:
- `snapshot_begin`;
- **one or more** `snapshot_chunk`s whose sessions together are **exactly** the session set;
- `snapshot_end`;
- **no `delta` between `snapshot_begin` and `snapshot_end`** ("at one sequence");
- then deltas only.

It never assumes one chunk, even for a one-session Hub. The same holds for the fresh baseline after a gap.

**R-21.2: EV-3's statuses, by what fixes their cause.**
- **Fixed by a clause, so tested with a live stimulus:**
  - `accepted`: a declared emit;
  - `rejected_undeclared`: an emit of a name the manifest does not declare (EV-1);
  - `rejected_invalid`: a payload that fails the declared `payload_schema`;
  - `shed_ingress_full`: the real Hub with a small `ingress_queue` (a live `[events]` Hub parameter), and its injected scheduler (BUILD.md, sans-IO) holding the ingress drain, then one more emit.
- **No clause names a cause for `rejected_foreign` and `rejected_audience`**, and EV-3's list is non-exhaustive. In v1 they are **reserved vocabulary**: their mapping to `capability_denied` is checked statically on the contract crate's status-to-kind table, and no transcript invents a stimulus for them.
- So `conf::ev_3_status_maps_to_kind` checks the whole table statically, plus a live case for each of the four clause-fixed statuses.

**R-21.3: `conf::nt_5_final_origin_rechecked_after_substitution` has an observable stimulus, so it stays.**
- NT-5's promise is that "a placeholder can never change the destination". Give the secret values that look like a host or an authority: `@evil.example`, `//evil.example/`, a backslash form, and a percent-encoded form. Place them in the path, the query, a header value and the body.
- Through the real Hub's injected I/O edge (HP A3-1), the test asserts that every request reaches **only** the allowed origin, or is refused, and never reaches `evil.example`.
- A value that makes the final URL fail to parse is refused, and nothing is sent.

## R-20. "Completes in the next pump" is a fixed timing that the seeded scheduler does not vary (Stage 1 Core lead, P1 with P3's evidence, 2026-10-02)

- **Answer:** fixed. Core A5-2 varies "how many `pump` calls an operation takes" only where the contract leaves it open, and it says "**Not varied:** … any order the contract fixes". A clause that says an operation completes **in the next `pump`** fixes it at exactly one. The scheduler never defers such an operation, and no `scheduler_ready` control is needed.
- **Every clause of that form** in the current Core contract (manifest final22):
  - **LC-5:** "`Stop` of a session whose payload already exited completes in the next `pump`";
  - **SZ-2:** "A `Resize` of the current size completes `Applied` in the next `pump`";
  - **A2-1, the `Resize` row:** in `Created`, it "stores the spawn size; completes `Applied` in the next `pump`".
  A due deadline's timing is governed by Core erratum 3, not by this ruling.
- **What stays variable around them:**
  - other operations in the same `pump`;
  - the order of events across sessions (OR-3);
  - the batching of `poll_events` (A5-2: a non-empty part).
  A test for one of these clauses therefore polls until it has the `Completed`, and asserts that it was posted by the first `pump`, unless the test isolates the session so that nothing else is pending.
- **Event budget:** keep `pump_events` at a value that is not exhausted in such a test. Erratum 3 lets an atomic step beyond the budget be carried.

## R-19. A synchronously refused attach leaves the transport with the caller (Stage 1 Core lead, P6 reviewer, 2026-10-02)

- **Answer:** on a **synchronous** `attach` refusal, the caller keeps its transport and may reuse or close it. Core neither consumes nor closes it. No amendment is needed.
- **The text:**
  - DP-2: "**From a successful attach return**, the descriptor belongs to Core". Before a successful return, which covers every synchronous refusal, it does not;
  - AM-4: a sync refusal takes nothing;
  - Core A5-3 repeats "no ownership taken" for the testkit's scripted sync refusals;
  - DP-2's `HandoffFailed` case (Core closes the descriptor) applies only **after** a successful return, when the later handoff fails.
- This covers every sync refusal of OU-1: `UnknownSession`, `RouteLimit`, `InvalidInput` (including A7-1 and A9-1) and `Unsupported`.
- **The crate signature is a contract-crate defect, not a contract change.** `fn attach(..., transport: RouteTransport, ...) -> Result<AttachResult, CoreError>` drops the transport on `Err`. `botster-core-contract` must return it, for example `Result<AttachResult, AttachRefused { error: CoreError, transport: RouteTransport }>`, or take a form that leaves ownership with the caller on refusal. The exact Rust shape is the crate owner's choice.
- In the C ABI (section 13) a transport is a descriptor number, so the caller keeps it on an error return with no special form.

## R-18. Learning an affected query's id without a worker-internal observer (Stage 0 lead, core-p4b pair, 2026-10-02)

- **The question:** EV-8(i) tests need the `query_id` of an AFFECTED query, one whose `terminal_query` frame never started. `query_id` is opaque and minted by the worker. The pair proposed an observer of the worker's open-query table.
- **Answer:** that observer is **not allowed**. A query table is worker-internal state, and R-14.3 allows counting only at the link boundary. No amendment is needed, because a boundary-only route exists.
- **Learn the id by determinism** (Core A5-1: the same script, seed and inputs give the same results). The id is minted at the query's parse point, before the resync cut.
  - Run A delivers the `terminal_query` frame, and the test reads its `query_id` from the route.
  - Run B uses the same script and seed, with the resync forced at the same point by the scheduler or transport edge. It mints the same id.
  - The client then replies with it, and the test asserts `input_refused{query_expired}` (or `already_replied`, where that applies).
  - If run B's id were ever not reproducible, that is a testkit determinism bug (A5-1), not a reason for an observer.
- **Assert "never from stale state" at the program edge**, which reads every byte written to the PTY:
  - the affected query's answer that reaches the program is the saved shadow fallback (EV-8(d));
  - the client's stale reply bytes never reach it;
  - the retired `terminal_query` frame never appears after the new baseline.

## R-17. A query's `request_bytes` are the bytes the terminal model assembled (Stage 1 Core lead, P2 reviewer, 2026-10-02)

- **Answer:** for `ESC [ ENQ 5 n`, the outer query's `request_bytes` are `ESC [ 5 n`. The `ENQ`, which the terminal executes inside the unfinished sequence, is its own one-byte query. No amendment is needed.
- **The text:** EV-8 says "`request_bytes` is the exact query". The codec says "`request_bytes_base64` is the exact query sequence, **so a client can answer any query its emulator understands with `encoded`**".
  - The purpose decides the reading. With the contiguous span, the client's emulator would execute the `ENQ` a second time, so its `encoded` reply would carry an `ENQ` answer twice. That breaks EV-8's exactly-one reply.
  - "The query sequence" is what the terminal model (libghostty, the oracle, R-7) recognized as the query. A C0 control executed inside it is a separate query or effect.
  - "Exact" means unaltered (no rewriting or normalizing), not "the contiguous byte span".
- **Nothing is lost:** the raw `Output` still carries every byte in order (OU-12), so a client that replays bytes sees the `ENQ` in place.
- **Tests:** two `terminal_query` frames in model order: the `ENQ` query with `request_bytes = ENQ`, and the DSR with `ESC [ 5 n`. Each is answered once.

## R-16. A failed Stopping-row write during StopAll does not stop the payload's end (Stage 1 Core lead, P1 finding F10, 2026-10-02)

- **Answer:** P1's proposal holds. No amendment is needed.
- **The text:**
  - A2-1's `StopAll` row has **no async error**;
  - LC-12: "`StopAll` ends payloads only", and `Completed{StopAll}` is posted "when every target has reached `Exited`, `Lost` or `Created`";
  - LC-12's "left as they are" names only the `Created`, `Exited` and `Lost` targets.
  So a `Running` target of `StopAll` must reach `Exited` or `Lost`, and nothing may complete `StopAll` with `RegistryFailed`.
- **The rule:** for a `StopAll` target, the durable `Stopping` row write is best effort. If it fails, or its effect is unknown (AD-7), the stop effect still proceeds: the graceful request, then the kill after `stop_grace` (LC-5). The session reaches `Exited` or `Lost` with its normal `SessionState` event, and `StopAll` completes as LC-12 says. Only the durability of the `Stopping` state is lost.
- **Recovery** is by adoption. AD-1 adopts the remaining `Running` row as `Exited` if the payload ended, or as `Running` if it is still alive. A new host can then stop it again.
- Reading (b) does not apply, because "left as they are" covers only `Created`, `Exited` and `Lost`. Reading (c) is not needed.
- **Not ruled here:** a plain `Stop` keeps its async `RegistryFailed` (A2-1). This ruling covers only `StopAll`.

## R-15. Remove under a full mandatory-event queue keeps LC-7's order (Stage 1 Core lead, 2026-10-02)

- **Answer:** no. An open capture of the session never becomes `UnknownCapture` while a bound route of that session has not yet reached Closed on the host.
- **Step 1 waits for room.** LC-7 step 1 closes every bound route `SessionRemoved`. That is a route transition and its event, which EV-5(b) makes one atomic step. With no room in the mandatory queue, the route stays `Open` in the worker and on the host, and the step is parked.
- **Remove waits at step 1.** LC-7 is "in order", so steps 2 to 5 do not run before every bound route's `RouteClosed` has been posted. Running step 3 early would end the worker while its route is still `Open`, and the route would die without its `route_closed{session_ended{exit}}` frame (OU-2b).
- **What EV-5(c) means for Remove.** "The effects of … `Remove` continue" covers the steps that need no mandatory event. When no route is bound (or once step 1 is done), steps 2 to 5 run under pressure: captures are released, uploads are deleted, the worker is ended, the row is deleted and the id is freed. Only `SessionState{Released}` waits for room, and `Completed{Remove}` follows it (its slot is reserved, EV-5(a)).
- **Without pressure**, the strict order holds, as P1 plans.

## R-14. `Unsupported{what}` inputs, the cause of `CursorReadUnsupported`, and a testkit measurement control (Stage 0 lead 2, for the core-p3 pair, 2026-10-01)

**R-14.1: `named_key` and `wheel_direction`.** 5.1A rule 5 gives examples, not a table. For each value, the contract fixes one case, and the terminal oracle (R-7) decides the rest:
- **`named_key`, fixed by the clause:** 5.1A says the modifier keys (`left_shift`, `left_control`, `left_alt`, `left_super`, `right_shift`, `right_control`, `right_alt`, `right_super`) are "encoded **only** under kitty flag 8". So any of them, written while kitty flag 8 is off (legacy, or flags without bit 8), has no encoding. It completes `NotWritten(Unsupported{what: named_key})` with exact zero bytes. A transcript may name that input.
- **Every other `named_key` case** (for example a key in the closed set that the pinned encoder cannot express under the active flags): the pinned Ghostty encoder decides (IN-9, R-7). These cases are **real-only**. A transcript asserts the outcome shape (typed zero, exact zero bytes, `what = named_key`), never a hand-picked input.
- **`wheel_direction`:** no clause fixes which encoding lacks which wheel direction. Every case is **real-only**, decided by the pinned encoder. If the pinned encoder rejects no wheel direction under any encoding, the value is unreachable at that pin. The replacement map records that finding with the pin, and `conf::a2_2_unsupported_what_values` covers the reachable values. `what` is non-exhaustive (A2-2), so a listed value need not be reachable on every pin.

**R-14.2: the cause of `CursorReadUnsupported`.** A2-1: it means "no worker model for this session", and reads of `Created` or `Lost` are `WrongState`. The reads are admitted in `Starting`, `Running`, `Stopping` and `Exited`.
- The v1 cause is a read admitted **while the session is `Starting` and its worker has not yet reported a terminal model**: the worker exists, but its model is not up.
- The testkit produces it with the existing process edge: it withholds the worker's control link during `Starting` (A6-1), and the read is issued in that window.
- **A "worker without a terminal model" is not allowed** as a scripted behavior. No clause names such a worker, and the real worker always has libghostty. The core-p3 placeholder under `by_edge/process` must be replaced by the `Starting` case.

**R-14.3: `control_link_stats` is allowed without an amendment.**
- A5-1 enumerates the testkit's **edges**: the things a test scripts or injects. A read-only measurement of the in-memory control link is not an edge. It injects nothing and changes no order.
- It keeps the one-code-path rule (A5-1, A5-4) under four conditions:
  1. it counts at the testkit's in-memory link boundary, never inside worker code;
  2. it adds no wait, lock or synchronization point that could change the order of frames;
  3. it lives only in `botster-core-testkit`;
  4. its documentation (`docs/core-testkit-controls.md`) names TP-1 as the clause it observes.

## R-13. SGR-pixels mouse positions carry the zero-based pixel (Stage 1 Core lead, 2026-10-01)

- **Answer:** under `mouse_encoding = SgrPixels` (1016), a `Mouse` with `x = 0, y = 0` reports **0, 0**. No amendment is needed.
- **The text:** 5.1A Mouse, Units: "`row` and `col` are zero-based terminal cells … (Core adds the 1 that the xterm encodings need). `x` and `y` are optional zero-based surface pixels …".
  - The parenthesis belongs to the sentence about **cells**.
  - The pixel sentence defines `x` and `y` as zero-based and adds no offset.
- **The oracle agrees:** libghostty, the only terminal oracle (R-7), encodes the pixel position with no +1, and it adds 1 only for the cell formats.
- **How to test:** the encoded bytes come from the real worker and libghostty. No test hand-writes the expected sequence (R-7).

## R-12. Rulings superseded by Core Amendment 5 and HC Amendment 5 (manifest final12, 2026-10-01)

The amendments replace the hand-written FakeCore and client FakeHub with real implementations and injected edges. Read the earlier rulings with them:
- **R-1.1** (a wasm-wrapped TS FakeHub): its conditions now apply to the **transcript player** (HC A5-2). There is one Rust implementation, compiled to wasm, driven through the client's own transport adapter.
- **R-1.2** (bindings): unchanged. The player serves the real bindings, and the Hub's in-memory transport is never a binding (HC A5-1).
- **R-1.3** (the plugin-side FakeHub is scripted): unchanged. HP section 19 is not amended.
- **R-1.4** (fault hooks): the list stands. The player executes these faults (HC A5-2). On the real Hub and Core they come from edges and the seeded scheduler (Core A5-2 and A5-3, HC A5-4).
- **R-1.5** (where FakeCore lives): superseded. There is no FakeCore. Core's repo runs the suite on real Core and on the testkit (Core A5-4).
- **R-5** (the FakeCore budget, no model, no encoder): superseded as a budget question. The testkit runs real Core and real libghostty (Core A5-1), so terminal bytes and states come from the oracle (R-7). R-5.3's reasoning stays: no shared encoder crate outside Core.
- **R-7:** unchanged, and now met directly by the testkit.

## R-11. The `session` row: required, optional, absent versus null (Foundation lead, 2026-09-30)

**One row shape for HC and HP.** HP A2-28 says "a row is the client `session` resource", so the HP session row and the HC `session` row are the same shape. Requiring only `id`, `hub_id` and `state` in the plugin contract is not allowed.

**(a) Required on every row** (section 6 with no `?`, and HC A.8 with no "or absent"):
- `id` and `hub_id`: string;
- `label`: string;
- `state` and `lifecycle_class`: the section 6 enums;
- `exit`: `{ code?, signal? }`;
- `size`: `{ rows, cols }`;
- `session_type`: `{ id, source }`;
- `traits`: string array;
- `restartable`: bool;
- `activity`: `active` or `idle`;
- `presentation`: `default`, `plugin` or `hidden`;
- `metadata`: a map, `{}` when no plugin has written.

**Optional (absent when there is no value):**
- the three keys that A.8 types as "string or absent": `failure_reason`, `role` and `interaction`;
- the keys that section 6 marks with `?`: `port?`, `owner?`, `worktree_id?`, `branch?`, `title?`, `task?` and `attention?`.

**(b) Absent, never `null`.**
- HC A.1: "`null` is accepted only where the field's schema permits it … A required field is never `null`". No session field's schema permits `null`. HP WF-4 says the same.
- So a field with no value is **absent**: `failure_reason` on a healthy session, `port` without an allocation, and so on.
- **`exit` is the one required field that can be empty.** A.8 types it `{ code?: int, signal?: int }`, and unlike `failure_reason` it is not marked "or absent". So `exit` is always present. It is `{}` until the process ends, then carries `code`, `signal` or both, as known (each absent when unknown).
- The one place where `null` has a meaning is an RFC 7396 merge patch (`update`): `null` deletes `label` or `task` (A.1).

**(c) `attention` is a typed optional field, not an open map.** Its shape is `attention?: { since, source, title?, body? }` (section 6, SF-5d):
- `source` is `bell`, `osc9` or `osc777`;
- `title` and `body` are strings;
- `since` uses the same timestamp encoding as the `at` of the `session_notification` event that set it.

It is absent when no attention is set. `clear_attention` and a route becoming focused remove it. It is never inside `metadata`, which belongs to plugins.

**Generated schema.** `botster-hub-contract/schemas/v1/session.json` (A.8) lists exactly the required set above in `required`. HP adopts the same schema.

## R-10. The "error" tone is `danger` (Foundation lead, 2026-09-30)

- UI-CF-4's tone set is closed: `neutral|info|success|warning|danger`. A `tone: "error"` would be the `unknown_tone` diagnostic (UI-NV-4). So "`error`-tone" in UI-AC-7(a), UI-NV-3 and UI-NV-5 cannot mean a token `error`.
- `danger` is the member that UI already uses for the failure and destructive sense (UI-AC-3 `confirm.tone: neutral|danger`). `warning` has its own fixed use (UI-SA-5).
- The implementation's `tone: "danger"`, and the vectors that assert it, are correct.
- The text writes `error` in backticks as if it were a value, and the vectors fix the token for every renderer (UI section 16, R-3). So the token is written into the contract by the erratum `amendments/ui-contract-v1.4-erratum-1-candidate1.md` (UI-NV-8). The erratum adds no version or feature.

## R-9. Core: where the spawn report lives, and the chunk size bound (Foundation lead, 2026-09-30)

**R-9.1 (C1): the lists are a sync read, `service_report(id)`, added by Core Amendment 4.**
- SV-4's sentence "the same lists are in the service's state" is **not** superseded. A2-5 defines the `ServiceState` *event values*, not the service's record.
- The guarantee matters after a host restart: the spawn completion is gone (ID-2), and an adopted service must be indistinguishable from a started one (AD-3).
- No call exposed the lists, so a contract change is needed. Core Amendment 4 candidate 1 adds `service_report(id) -> Result<SpawnReport, UnknownService>`. It is readable from the `Ok` spawn until `RemoveService`, and after adoption.
- A field on `Started` and `Reconnected` was rejected. It would put the lists in events that an adopting host may never see.
- Until Amendment 4 is accepted, keep the read out of the crate's facade, or mark it pending.

**R-9.2 (C2): "the transport maximum" is the route transport's maximum message size, known only at attach. No amendment is needed.**
- **Superseded for Core by Core Amendment 17 (final37).** A17-4 item 15 says that R-9.2 "no longer applies to Core": Core has no `default_route_chunk_bytes` and no route `max_chunk_bytes`. HC Amendment 8 moves chunking to the Hub, at `welcome.limits.max_chunk_bytes`, which bounds the chunk payload without the `flags` byte (A8-3). The text below is kept as the record.
- TR-3 (HC, and the route codec): each route chunk is "at most `route_limits.max_chunk_bytes` … chosen at or below the SCTP maximum message size". That maximum belongs to one WebRTC connection, so no value at `open` can stand for it.
- **At `open`**, `CoreLimits::validate` checks the part of the range that is known at `open`: `default_route_chunk_bytes` is at least 1 KiB. A larger value is not refused there.
- **At attach**, the route's `max_chunk_bytes` (TS-1 `route_limits`) is the smaller of the host's chunk size (or `default_route_chunk_bytes`) and the transport's maximum message size. This is TR-3's "chosen at or below".
- No fixed number and no other `CoreLimits` field is the bound: either would invent a rule that the text does not state (regrowth rule 1).

## R-8. The action name of `invoke` on the wire (Foundation lead, 2026-09-30)

- **Reading (a): `req` carries a field `action`.** It is required when `verb` is `invoke`, and absent for every other verb.
  - VB-6 makes `action` a parameter separate from `args`: `invoke(kind, target?, action, args)`.
  - MC-2's MCP tool already carries it as a sibling of `args`: `invoke { ref, action, args, … }`. MC-2 says its shapes are "exactly" those of every other binding.
  - EN-1's frame list omits the field. That is a defect in the text, and HC Amendment 4 corrects it (`amendments/hub-client-contract-v1-amendment-4-candidate1.md`).
- **(b) is ruled out.** `verb` is the closed set of P2 (`describe | list | get | watch | unwatch | create | update | delete | invoke`).
- **(c) is ruled out.** `args` is validated against the action's own `args_schema` (DS-1). A name inside `args` would collide with an action argument called `action`.
- **EN-8 includes `action` in the match.** The same key with another action is `idempotency_key_reused`.
- **The name is the `describe` name:** the key of the kind's `actions` map (DS-1). A contributed action uses its listed name, for example `<package>.<action_id>` on `session` (DS-6).
- Build on (a) now. The pending marks can go when Amendment 4 is accepted.

## R-7. Standing rule: libghostty is the terminal oracle (the user, via the orchestrator, 2026-09-30)

- **Terminal behavior has one oracle: libghostty, run in the Core repo.** This covers the 5.1A input bytes, screen contents, baselines and snapshots, modes, and output scanning such as notifications.
- **A hand-written expectation never stands in for libghostty.** No hand-written terminal model, encoder or literal expected terminal bytes in a fake or a transcript.
- **Every conformance id whose subject is terminal bytes or terminal state is real-only.** It runs against real Core with libghostty. A fake carries only script data for these (R-5.1, R-5.2) and asserts nothing about terminal correctness.
- Every later steward ruling that touches terminal behavior cites this rule.
- **R-5 is read with this rule.** R-5.2 says the session script "supplies the bytes" for a byte count. Those bytes are opaque script data for counting only, never an encoding claim. R-5.2's "real-Core subjects" are proven against libghostty.

## R-6. Hub→plugin manifest keys and the checks on an unknown frame (Foundation lead, 2026-09-30)

**R-6.1 (P2): four manifest keys are required. Every other declaration is optional, and absent means empty.**
- **Required:**
  - `botster_plugin_api` (MN-1 says "required");
  - `name` (the plugin key: `Hello.plugin_key`, scopes and owner names depend on it);
  - `version` (PD-1 install and update identity);
  - `entrypoints` (LD-1 starts the service, HS-1 handshakes with it, and LB-17b resolves `entrypoints.service.lua`).
- **Optional:** every other key of MN-1.
  - An absent declaration is **"not provided"** (WF-4), so the plugin declares nothing of that kind. This is the same as the empty value: `[]`, or `{}` for `storage`, `events` and `requirements`. A `Ready` that repeats the manifest uses the same empty value (`conf::mn_1_ready_matches_manifest_or_fails_load`).
  - Two keys state their own absent value: `scope` defaults to `"device"` (MN-1, CP-8), and an absent `limits` requests the Hub's defaults.
- **Why this reading.** The `?` in MN-1 marks the three *informational* keys, not the whole optional set. Where the contract makes a declaration conditionally required, it says so with its own failure code. For example, WP-4 requires a `gap_handler` from a plugin with `relay_signaling`, else `gap_handler_required`. That code would be needless if every declaration were always required. A literal "every unmarked key is required" reading would also force a `gap_handler` on every plugin. EV-15 and WP-4 do not ask for that.
- **Inside a present declaration**, its own fields follow its own clause (for example MN-3 for `events`, ST-1 for `storage`).
- `conf::mn_1_required_keys_present` tests exactly the four required keys. The generated `plugin-manifest.schema.json` lists exactly these four in `required`.
- LB-17 and Part C of the Lua binding list allowed keys only. They state no requiredness, so nothing in them fails under this reading.

**R-6.2 (P3): the framing checks and the epoch check come before the unknown-type rule.** A receiver checks every frame in this order:
1. **WF-1:** `len` against `frame_bytes`, before any allocation.
2. **WF-2:** the payload is one UTF-8 JSON object. A failure ends the epoch (SV-6), **whatever the type**. Section 2 says "Every frame has a lane byte, a `type` byte and a payload. **The payload is one UTF-8 JSON object**", with no exception for unknown types.
3. **The epoch.** "Every later frame carries the epoch", and "a frame whose epoch is not the connection's current epoch is dropped and counted" (section 2, CR-5, MD-4). A stale frame is dropped and counted **whatever the type**: a stale unknown state-changing frame gets **no** `Fault`. The work it belongs to is already fenced, and nobody in the old epoch could receive a `Fault`.
4. **Then the type:** a known type is handled. An unknown advisory type (0x80–0xFF) is ignored and counted. An unknown state-changing type (0x00–0x7F) gets `Fault{unsupported_frame, type}` (VR-2).

"Ignored" in VR-2 means "its content is not acted on". It does not mean "framing is skipped". An unknown advisory frame with an empty or non-JSON payload therefore ends the epoch under WF-2. The decoder that returns before checking the payload is wrong.

## R-5. The FakeCore budget and the shared fake route endpoint (Foundation lead, 2026-09-30)

The question: `botster-fake-route` is 2,988 behavior lines, of which `model.rs` is 663 and `encode.rs` is 656. With FakeCore's own estimate of about 1,750 lines, FakeCore totals about 4.7k lines. Core section 11 sets "Budget: 1.5–2k lines. If it needs more, the contract is too large."

**R-5.1: the fake does not model a terminal (option c, for output).**
- Core section 11 defines `FakeCore` as "written **from the clauses, not from real Core**: **scripted output and exits**, an injected clock, failure injection per error code".
- So the screen contents, baselines, snapshots, the `ReadFacts` values (ST-7), and the notifications found in output (HC SF-5f) are **script data**. They are not computed by a terminal model. The test author keeps them consistent.
- The consumers (the Hub, the clients) test their handling of frames, events and results, not a terminal emulator. This is the same reading as R-1.3 for the plugin-side FakeHub.
- `model.rs` and the output scanning that only feeds it go. The route state machine stays: baseline and resync sequencing, close, flow control, choice points, refusals and file transfer. These are observable clauses of the route.

**R-5.2: the fake does not encode semantic input (option c, for 5.1A).**
- The 5.1A bytes have exactly one observer: the program on the PTY. In a fake, that program is scripted, so there is no observer to be faithful to.
- The fake implements the **observable** clauses of `Key`, `Mouse` and `Focus` input:
  - kind and field validation (`invalid_input`);
  - the `repeat` rules and their bound;
  - `not_ready`;
  - the admission bound and the guards;
  - receive order, refusals, and DP-12 focus tracking with `FocusChanged`.
- Where a result reports encoded byte counts (IN-9: `payload_bytes_written`, `pty_bytes_written`), the **session script supplies the bytes** that a semantic event writes. The fake counts them. This is scripted output of the worker, not an encoder.
- The conformance ids whose subject is the 5.1A bytes, or the encoding with write-time modes, are **real-Core subjects**. Core's repo runs them against real Core (section 11). Against FakeCore, their transcripts are marked real-only, the same way as clauses about real processes, the PTY and fd handoff. "Provably faithful" covers what the fake models; the encoder is not modeled.

**R-5.3: a shared encoding crate (option b) is not allowed.**
- Section 11 requires the fake to be written "from the clauses, **not from real Core**". A crate that real Core's worker also runs makes the fake share Core's bugs.
- IN-9 says "The contract has **no public encoding helper** (one path)". A crate that the fake in this repo depends on would be public, so a Hub could encode from stale modes, which is the bug IN-9 prevents.
- The route codec is different. It is a wire format that both sides must agree on (HC owns it, and Core adopts it verbatim), so a shared implementation of it is the contract in code. The 5.1A encoding is a duty of the worker toward the PTY. No second party decodes it.

**R-5.4: re-measure before any amendment (options a and d deferred).**
- The budget sentence is the contract's own size check. The steward does not waive it (option a).
- Measure again after R-5.1 and R-5.2. If FakeCore (its own code plus the route endpoint it runs) is still over 2k lines, send the steward the per-file numbers. The steward then drafts a Core amendment. It will either narrow a named route area (option d) or count the shared route endpoint once, under its own budget, because the client-facing FakeHub also runs it and the route protocol grew after the budget was written (output_dropped, file transfer). Sol Core reviews that amendment. It is not decided now.

## R-4. Hub→plugin frame bytes and lanes (Foundation lead, 2026-09-30)

**R-4.1 (P1): the generated artifact assigns the `type` bytes. No amendment is needed.**
- Appendix B: "A frozen design does not contain the wire codec; it **names the artifacts that must exist before implementation starts**". Each artifact "is generated from the contract crate (`botster-plugin-contract`), checked in, and used by both the Hub and every host", and `conf::ap_b_artifacts_match_code` fails on drift. The artifact `plugin-frames.schema.json` carries "the lane, the `type` byte and the … class". So the crate pins the values, and the checked-in artifact is the agreement between the Hub and every host (README rule 3).
- The frozen text constrains the values in four ways:
  1. **Class range.** A state-changing frame takes a byte in 0x00–0x7F, and an advisory frame takes a byte in 0x80–0xFF (section 2, VR-2).
  2. **One type space.** A `type` byte is unique across all lanes, because `Fault{unsupported_frame, type}` names a frame by its `type` alone (section 2).
  3. **Lanes.** The lane bytes are fixed by section 2: `control` 0, `invoke` 1, `call` 2, `bulk` 3.
  4. **Stability.** Once an artifact is published for a `botster_plugin_api` version (A2-43), its bytes never change. A changed byte breaks VR-1 ("a plugin host built at the previous protocol version loads"), so it needs a new protocol version.
- The implementer's table meets all four constraints: control 0x01–0x06, invoke 0x10–0x11, call 0x20–0x21, all state-changing.

**R-4.2 (P1a): `Cancel` is state-changing.**
- The class answers one question: what a receiver does with a type it does not understand (section 2, VR-2). "Advisory" means that ignoring the frame is harmless. Ignoring `Cancel` is not harmless: the handler keeps running, and its outstanding `HostCall`s are not answered `cancelled` (IV-5, CA-5).
- "Best effort" in IV-5 describes the outcome (a committed call stays committed), not the frame class.
- All ten frames of section 2 are base frames, which every v1 receiver must understand. So all ten are state-changing.

**R-4.3 (P1b): the lanes are those of section 2. `log.write` has no frame type of its own.**
- MD-1 defines ordering, not the lane set. Section 2 defines the lanes: "`bulk` (3: bulk-class host calls such as `log.write`, and their results)" and "A host call of the *bulk class* (`log.write`) travels on lane 3 in both directions; every other call uses lane 2".
- A bulk-class call uses the **same** `HostCall` and `HostResult` frame types on lane 3. The lane follows the call's class (Appendix A marks `log.write` as `(bulk)`), not the frame type. So the frames artifact gives `HostCall` and `HostResult` the lanes "2, or 3 for a bulk-class call". `HostResult` answers on the lane of its `HostCall`.
- **A call on the wrong lane** (a bulk-class call on lane 2, or another call on lane 3) is a malformed request. The receiver answers it with the common failure `invalid_request` (Appendix A, "Common failures of every call"). It is not a `Fault`, because the frame type is known.
- MD-1's exception still holds across lanes: a `Result` for R follows every `HostCall` of R, on lane 2 or lane 3, that has been answered or abandoned.

## R-3. The Foundation ui-contract pending items (Foundation lead, 2026-09-30)

- Items A1–A7 (props), B (capability names) and C1–C5 and C7 (prop shapes, role segments, tree fields and codes, input shapes, admission codes, defaults) are **normative**. The vectors fix them, and UI section 16 binds every renderer to the same vectors, even when one implementation (Rust or wasm) serves every client. They are batched into `amendments/ui-contract-v1.4-realize-completeness-candidate1.md`.
- C6 (default copy) is **not** text: UI-PT-5 says defaults are client copy. `realize` emits copy keys, and the vectors assert keys.
- C8 (`iframe` in the enum) needs no amendment (UI-EM-10).
- The landing plan is allowed: the crate and its types land now, and the asserting ids wait in `pending.txt` until the amendment's ACK.

## R-2. The ui-contract realize crate (Foundation lead, 2026-09-30)

**R-2.1 (U1): the length unit in UI-ID-3 is the UTF-8 byte length.**
- UI-ID-3 "reuses the shipped length-prefixed scheme … with its vectors". The shipped vectors in `botster-hub@9e1ba42803ee:crates/botster-ui-contract/src/assets.rs` count UTF-8 bytes: `botster-ui-descendant-v1:5:café9:重命名` (`café` is 5 bytes, `重命名` is 9) and `…:11:会話-😀11:remove-🧹`.
- Web must use UTF-8 byte lengths (for example `TextEncoder`), never `String.length`.

**R-2.2 (U2): the `container` role is decided by the part kind's mapping shape, never by the row count.**
- "The part's outer node takes `container` only when the part has more than one node" (UI-RZ-2) refers to the mapping: does the part kind generate nodes besides its outer node? It is not about how many rows exist at a given moment. Node ids must not change when rows arrive (the UI section 16 skew vector).
- **`container`:** `list` (generates `list_item`s), `detail` (generates `inline` rows), `form` (generates sections and fields), and `actions` / `toolbar` (they generate `button`s and a `menu`).
- **`<part.id>`:** `table`, which is one node whose rows are props (R-2.3), plus `text`, `status_badge`, `empty_state`, `embed`, `stack`, `inline`, `panel` and `section`. The children of the last four are authored parts with their own ids.
- A row node is `child_id(part.id, row_key)` (UI-ID-2). A row key equal to a role word, for example `container`, can collide. UI-RZ-5 diagnoses it; it is never a crash.

**R-2.3 (U3): the UiNode vocabulary is the existing `botster-ui-contract` `UiNode`, taken explicitly.**
- UI section 12 says "`RealizedTree` is a tree of the **existing UiNode vocabulary** (`botster-ui-contract` `UiNode`)". The clause adopts that type by reference.
- **Option (a) is the meaning:** steal `UiNodeKind`, `UiNode` (`kind`, `id`, `props`, `children`, `slots`) **and the per-kind prop schema** (the `schema(...)` table that the old validation uses) from `botster-hub@9e1ba42803eeab4856181e4012d3435f9a763949:crates/botster-ui-contract/src/lib.rs`, with the BUILD.md trailers (`Stolen-From`, `For-Clause: UI-RZ-2, UI-RZ-2a`).
- The stolen kinds and per-kind props **are** the vocabulary: the structural props come from there (for example `title`, `direction`, `align`). Changing the vocabulary later needs a ui-contract amendment.
- **Table rows, cells and columns:** the old schema gives `table` the props `columns` and `rows` (and `empty_state`). A table realizes as **one `table` node**; its rows are entries of `rows`, each carrying its UI-ID-2 row node id; cells are the values of a row; columns are `columns`. There is no `table_row` kind.
- **UI-RZ-2a's 16 props** are the props that the **mapping table** must assign wherever they apply. They are a required minimum for the mapping, not the complete prop set of every node.
- **Anything still missing after the steal** needs a ui-contract amendment, not an invention in the crate. That means a node or prop the realized tree needs that neither the stolen schema nor a frozen clause names (for example from 1.1 UI-RZ-2e or 1.2 UI-RZ-2f). The implementer sends the steward the list, and the steward drafts the amendment.

**R-2.4 (U4): renderer capability names.**
- Section 11 declares `ui_capabilities { ui_contract_view, parts, features, widgets }`. The **values** of `parts` and `widgets` are already defined: the part kind names of the view grammar, and the `ui` widget hint values of the grammar. Use those exact names.
- The contract does **not** name:
  - capability names for field **display options** (for example metric and badge display) or for **screens** (the kinds with their own screen, UI-SL-3);
  - the meaning of an **absent list**.

  These need a ui-contract amendment. The Foundation implementer keeps them isolated until the amendment lands.

**R-2.5 (U5): `stale` needs no amendment.** UI-AD-2 makes the view "`stale` (shown as a diagnostic)". `stale` is the clause's own word, so the diagnostic reason is `stale`. It is a run-time view state, not an admission refusal, so it does not belong in the admission code lists.

**R-2.6 (U6): there is no sixth input.** Action access (DS-1 `allowed` and `reasons`, UI-AC-5) reaches `realize` inside **`kind_snapshots`**. The Hub→client contract exposes the `KindDescriptor` as rows of the watched kind `kind` (section 6, "`kind` | the `KindDescriptor` … | list, watch"). A client that watches `kind` has it at the same cut. `realize` reads `actions.<name>.allowed` and `reasons` from that snapshot.

## R-1. Phase 0 fakes and test ids (Foundation lead, 2026-09-30)

**R-1.1: the TypeScript FakeHub (HC section 12).** "FakeHub … exists in Rust and in TypeScript" and "a TypeScript runner that drives the TS FakeHub, so Web's tests use the same clauses." The purpose is that Web runs the same clauses as the Rust side. **A TypeScript npm package that wraps the Rust FakeHub compiled to wasm meets the clause**, on these conditions:
- It is published from the same contract release as the Rust crate (one version, one source).
- The TypeScript runner drives it **through the TS client core's own transport adapter boundary**, the same boundary that Web uses, not through an internal shortcut.
- `with_seed(n)` and every fault-injection hook (R-1.4) are reachable from TypeScript.

One implementation that cannot drift from itself is better than two.

**R-1.2: which bindings FakeHub serves (HC section 12, TB-0).** "It serves real bindings (TB-0)." The real bindings are the TB-0 bindings: `local-stream`, `webrtc` and the partial `mcp` (TB-M).
- **An in-memory loopback is allowed as runner plumbing, but it is not a binding.** It must never appear in `welcome.features` as `binding:*`, and it does not satisfy "serves real bindings". A fake behavior with no clause is not allowed (HC section 12).
- **The client contract requires all three bindings of FakeHub**, because the consumers test their own adapters against it:
  - each client runs the named semantic transcripts "through their own transport adapter" (HC section 12): `local-stream` for the TUI and the CLI, and `webrtc` for Web (with PR-0 to PR-9 pairing on test keys, and TB-W2 route peers);
  - the CLI is tested against FakeHub except for launch and attach smoke tests (CLI-P8), which includes `mcp-serve` (CLI-MS), so it needs the `mcp` binding.
- **Timing is a plan question, not a contract question.** The contract requires each binding before the consumer feature that depends on it is accepted. Whether `webrtc` and `mcp` land inside the Phase 0 done-gate or as early Foundation items in Phase 1 is a decision for the lead and the orchestrator.

**R-1.3: the plugin-side FakeHub is scripted (HP section 19).** The Foundation lead's reading is correct:
- FakeHub "drives a real plugin host … scripted `HostResult`s"; it has no semantic Hub (storage, timers, sessions).
- In Phase 0, FakePlugin and FakeHub in loopback prove the **protocol-level** clauses as real fake behavior: framing, handshake, invocation, deadline and Cancel, limits, crash, epoch.
- For semantic Hub clauses, the expected Hub frames come from the script. The semantic proof is FakePlugin against the **real Hub** in Phase 1.
- A scripted FakeHub must not invent semantic behavior that no clause states (HP section 19, last line).

**R-1.4: test ids whose subject is a real host or client (LB-\*, CLI-\*, the HC client-side checks).** The plan is correct. No clause requires these subjects to run in Phase 0:
- HC section 12: "Client CI: each client runs the suite's client-side checks … against FakeHub".
- HP section 19: "The Lua host CI check".
- CLI-CV1: "The suite runs the CLI against FakeHub".

Phase 0 writes their transcripts, so every id exists, and validates them structurally and against the fake on the other side of the wire.

**What Phase 0 must add:** **FakeHub needs the fault-injection hooks that the named semantic transcripts require**, so that the clients can run them in Phase 1:
- drop the transport mid-route (TS-9, TS-11);
- a plugin reload between `describe` and dispatch (DS-6);
- drop a committed response (EN-7, EN-8);
- concurrent duplicate keys (EN-8);
- a resync while a frame is partly written, and the started frame that cannot finish (TS-4c, TS-9);
- an unknown verb or component (EN-6);
- internal queue loss on a watch (WT-4);
- a resync that discards notifications, so that `output_dropped` is exercised (codec TS-4c, HC SF-5f);
- every seeded interleaving point of HC section 12.

A hook is allowed because a clause names it.

**R-1.5: where FakeCore lives (a note, not asked).** Core section 11 says the suite runs "against real Core in Core's repo **and against `FakeCore` in Core's repo**, so the fake is provably faithful", and the Hub's repo runs the suite against FakeCore. BUILD.md Phase 0 builds the fakes in this repo. **Either placement meets the contract if Core's CI runs the same suite against both real Core and FakeCore.** That cross-run is the faithfulness proof the clause asks for. The Foundation lead and the Core lead settle where it lives.
