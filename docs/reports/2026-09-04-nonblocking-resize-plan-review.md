# Review: nonblocking resize completion plan

Date: 2026-09-04. Reviewer: Fable (session `sess-1788561640-0030-9ab4035a5a3867e8f4970b3a6e440510`).
Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-resize-review`, branch `foundation/resize-review`, Core `93acae3f98adbc21dc981d113c4eb2f31ead4ad0`.
Plan: `botster-hub/docs/plans/2026-09-04-nonblocking-resize.md`. Audit: `botster-hub/docs/reports/2026-09-04-multiplexer-foundation-audit.md`.

This review reads source only. It does not change production code, tests, or any pipeline worktree.

## Verdict

**Changes required.** The plan targets the correct defect and the correct owner. The architecture is sound.
Five items need correction before Grok implements. Items 1 and 2 are blocking. Items 3 to 5 are scope and record corrections.

1. The plan describes new pending-state machinery that Core already has. The plan must say that the change is a deletion plus one deadline, not a new state machine.
2. The plan does not define what happens when the deadline expires. Core needs one explicit failure rule.
3. Requirement 6 conflicts with the explicit control-plane resize path. The plan must scope that requirement to the ingress path or extend scope on purpose.
4. Two vault gotchas apply to the new tests, and the plan does not name them.
5. The revision record is stale. Hub main has moved.

## What Core already does at 93acae3

The nonblocking completion chain exists end to end. Only one call blocks.

- Accept. `ManagedSessionRuntime::apply_terminal_input` pushes `(rows, cols, at)` onto `pending_terminal_resizes` when the ingress resize is accepted, and returns `input_result_ok` at once (`managed_session_runtime.rs:453-467`). The client already receives its result at accept time. There is no client-facing completion frame to deliver later.
- Worker. The worker applies the resize, forwards it, then emits `FRAME_RESIZE_APPLIED` before it reads the next control frame (`botster-session-worker.rs:255-268`). Acknowledgements are one per `FRAME_RESIZE`, in order, and carry only `rows` and `cols`.
- Reader thread. The `FRAME_RESIZE_APPLIED` arm sends `WorkerChannelEvent::ResizeApplied` and then calls `notify_session_wake` (`worker_process.rs:2652-2661`). This is the durable wake the vault note names.
- Completion. The worker-backed facade `pump_woken` already calls `reconcile_terminal_resize_acknowledgments` for every named session, between phase one and input apply (`botster.rs:1217-1218`). That helper calls `take_resize_applied`, which pumps the session channel and drains all acknowledgements without sleeping (`worker_process.rs:1193-1202`), then `acknowledge_terminal_resize` pops the matching front entry and records `applied_terminal_resizes` (`managed_session_runtime.rs:180-203, 972-988`).
- Persist. `CoreDaemon::pump_woken` reads `take_applied_terminal_resize` and calls `persist_changed_session_size` after the engine pump (`daemon.rs:1306-1319`). Registry geometry already follows the applied acknowledgement on this path.
- Cleanup. `pending_terminal_resizes` and `applied_terminal_resizes` are cleared in `forget_terminal_session` (`:956`), both `shutdown_session` branches (`:1866, :1882`), and `release_workers_for_restart` (`:270`). The acknowledgement channel belongs to one worker session, so a late acknowledgement after teardown has no receiver. A replacement session starts with an empty queue.

The blocking element is exactly one call. `CoreDaemon::pump_woken` calls `complete_pending_terminal_resize` per pumped session (`daemon.rs:1274`). That function loops `wait_for_resize_applied`, which sleeps `GATED_POLL` until match or `mode_gated_input_timeout` (`managed_session_runtime.rs:162-178`, `worker_process.rs:1169-1191`).

Consequence for the plan. Requirements 3, 5, 6 (ingress path), 7, 10, and 11 are already satisfied by code that exists. Requirement 2 ("reuse where possible") understates this. The implementation is: delete the blocking call and its helpers, add a per-entry deadline, and define expiry. Grok should not build a second pending-resize structure.

## Finding 1 (blocking): state the change as deletion plus deadline

Proposed correction to "Implementation requirements":

- Remove `CoreDaemon::pump_woken`'s call to `complete_pending_terminal_resize`.
- Remove `ManagedSessionRuntime::complete_pending_terminal_resize`, the `#[doc(hidden)]` facade method (`botster.rs:1243-1249`), the `CoreDaemonEngine` dispatch arm (`daemon.rs:3898-3905`), and `WorkerProcessRuntime::wait_for_resize_applied`. No callers remain outside this chain. The user authorized the cold cut.
- Keep `reconcile_terminal_resize_acknowledgments` as the single completion path. Do not add a second one.
- Replace the tuple `(u16, u16, u64)` in `pending_terminal_resizes` with a small struct that adds `deadline: Instant`, set at accept time from `mode_gated_input_timeout`. Reusing that option keeps `with_mode_gated_input_timeout` working for tests. Rename later if the coordinator wants a resize-specific option; that is optional.

Ordering is preserved without a wire field. The worker acknowledges each `FRAME_RESIZE` once, in control-frame order, so FIFO-by-size matching is correct. Keep `acknowledge_terminal_resize`'s "ignore a non-matching acknowledgement" rule. It is load-bearing: the explicit `CoreDaemon::resize` path (`daemon.rs:1425`) sends `FRAME_RESIZE` without pushing a pending entry, so its acknowledgement can arrive ahead of a pending ingress entry and must be skipped. Grok should add a comment that states this reason.

Pending state stays bounded. Each accepted ingress resize is one enqueued control frame. When the worker control queue is full, the apply path returns "control queue full" and the owner parks for retry (`managed_session_runtime.rs:~520`). The queue therefore bounds pending entries. The plan should state this bound instead of asking for coalescing.

## Finding 2 (blocking): define deadline expiry

The plan asks Core to "handle the deadline through its normal wait/pump contract" and to affect "only the intended session or operation". It does not say what the failure is. Grok cannot implement an undefined outcome, and the deadline test cannot assert it.

Current behavior is worse than the plan implies. On timeout the daemon records a commit failure, rearms the session wake up to `TERMINAL_COMMIT_REARM_LIMIT` (3), and leaves the pending entry in place (`daemon.rs:1274-1280, 2324-2340`). The next pump blocks another five seconds on the same entry. The stale sibling test at `terminal_wake_test.rs:1990` asserts this `Err` path and must be replaced.

Deadline mechanism. Extend the paste deadline. `wait_wakes` and `wait_pump` already clamp the host wait to `next_paste_deadline` and return `expired_paste_wake_batch` on timeout (`managed_session_runtime.rs:1677-1710`, `daemon.rs:1158, 1182`). Generalize both to a Core deadline that is the minimum of the earliest paste deadline and the earliest pending-resize front deadline. The expired batch must name the session in `ingress_sessions`, because pending resize is session-scoped and the route may already be unsubscribed. Hub's driver calls `wait_pump(watchdog)`, so the clamp reaches Hub with no Hub edit. Do not add a thread or a periodic poll. This satisfies requirement 8 and 9.

Expiry handling. Do it inside `reconcile_terminal_resize_acknowledgments` for named sessions, after draining acknowledgements. If the front entry's deadline has passed and no acknowledgement matched it, apply one rule:

- Recommended: pop the entry, keep the prior registry geometry, and call `mark_control_plane_failed` for that session with a resize-timeout error. This reuses the existing writer-failure path (`managed_session_runtime.rs:299-311`), which tears down only that session's owners. Reason: an unacknowledged `FRAME_RESIZE` means the worker's ordered control loop stopped consuming frames. Later input for that session is also stuck. Failing closed for that session matches the gated-input contract "fail closed on timeout".
- Alternative: pop the entry, keep the prior geometry, and emit nothing. The session stays live. This is softer but hides a broken worker.

The coordinator must pick one and write it into the plan. Either choice must not return `Err` from `pump_woken`, because that path retains the drain result and rearms the wake.

Exited sessions. `take_resize_applied` returns `SessionNotFound` for an exited session and reconcile skips it, but the pending entry survives until shutdown or forget. The clamp would then wake at every deadline for a dead session. Rule: when reconcile skips an exited session, drop its pending entries. The expired-batch producer must also ignore sessions the engine no longer holds.

## Finding 3: requirement 6 versus the explicit resize path

Requirement 6 says Core updates registry geometry only after the worker applies the resize. The ingress path already does this. The explicit `CoreDaemon::resize` calls `persist_session_size` immediately unless incremental attach is active (`daemon.rs:1425-1443`). Hub does not call it from `src` (it appears only as an exposed capability name in `lib.rs:454`). Seven daemon integration tests call it.

Choose one:

- Recommended: scope requirement 6 to the ingress path and add "explicit `CoreDaemon::resize` persistence" to Exclusions. It is not on the terminal pump and not part of the audit finding.
- Or: route the explicit path through the same pending queue. This widens scope into the daemon integration tests and the attach-queued branch. Do not do this silently.

## Finding 4: tests

Delayed-arrival regression. Feasible with the existing helpers. Specific guidance:

- Hold control. Prefer a parent-side scoped gate in the reader thread's `FRAME_RESIZE_APPLIED` arm, as a test-only `WorkerProcessRuntimeOptions` field (for example a shared flag plus condvar, or a channel the test releases). It needs no new worker flag, and the old code fails for the right reason because `wait_for_resize_applied` pumps the same channel the gate blocks. A worker-side flag that polls a release file also works. The existing `--test-omit-resize-applied` stays for the deadline test.
- Bounding. `CoreDaemon` is created and used on one thread (Hub does the same). Run the scenario on a spawned thread that owns the daemon. The main thread waits on a channel with `recv_timeout` shorter than A's configured deadline for each step. On a missed bound the main thread fails with the liveness message, then joins after the scenario thread's own timeout unwinds and drops the daemon. Cleanup happens through drop. The existing test at `:1990` already uses this observer-thread shape.
- Setup failure. `bind_size_reporting_worker` panics with "worker attach did not finish" after 8 seconds. That message is distinct from the liveness message, which satisfies the plan's rule. The audit's inconclusive attempt stalled in attach, so Grok must run the new test from the repository root with a worker built from the same revision.
- Echo. B's `input_result` arrives on B's pump. B's echo needs a later pure ingress wake. Loop with `wait_wakes` and `pump_woken` while A's gate is held, as `pump_until_encoded_output` does.

Vault gotchas the plan must name:

- [[a wake test must not consume the one shot edge it asserts]]. After release, A's completion wake is one shot. The test must pump it and then pump the pure ingress wake for A's later echo. The three-pump chain in [[resize completion wake durability has one ablation point and needs three core armed pumps]] is the reference sequence.
- [[a regression test must be shown to go red with the fix reverted]]. Grok must record the red run of the delayed-arrival test against the blocking call, with the liveness message, in the implementation report.

Existing tests that change meaning (Grok must explain each in the report):

- `pump_woken_worker_resize_updates_live_pty_registry_and_one_patch` (`:1785`) and `pump_woken_worker_resize_isolates_the_named_sibling` (`:1921`) assert registry geometry after the resize pump. They must pump A's completion wake first, then assert. The "identical resize appends one patch" assertion stays valid.
- `stalled_resize_acknowledgment_does_not_block_a_later_named_sibling` (`:1990`) asserts `Err("resize acknowledgment timed out")` from the first pump. Replace it with the deadline test: omit the acknowledgement, prove B live before and after, prove expiry arrives through `wait_wakes`, prove the chosen expiry rule, prove registry geometry unchanged for A.
- `drain_resize_persist_failure_still_emits_bound_queue_wake` (`:2956`) and `observe_resize_persist_failure_still_emits_bound_queue_wake` (`:2989`) inject persist failure. Persistence now happens on the completion pump. Grok must check which pump they assert.
- `pump_woken_same_wake_resize_then_input_survives_resize_completion` (`:889`) and `one_slot_adapter_preserves_resize_input_and_echo_wake_obligations` (`:1046`) already consume the completion wake and should pass unchanged. Treat any change there as a finding.

## Finding 5: revision record

- Hub `main` is now `ae6a0b1`, not `bb1a330`. `e50e0f0` is still not in main. Main still locks Core `48a4370`. Update the plan.
- The pending branch is still `7164c38` in its worktree. The coordinator confirmed that ticket `1787600679_990088` owns it and that no one there edits Core resize. The Core branch must not change that tested revision.
- Hub needs no source change for this cut. Hub never calls `complete_pending_terminal_resize` or `CoreDaemon::resize`, and `wait_pump` already applies the Core clamp. Downstream validation is a dependency source override plus the representative real-daemon test the plan names.

## Remaining blocking paths (record, do not fix here)

- Mode-gated input timeouts are not clamped. `parent_deadline` is checked only when the session is next named in a batch (`managed_session_runtime.rs:313-335`). A worker that never replies leaves the owner parked until another wake. The generalized deadline clamp from Finding 2 makes this a small follow-up, but it is outside this plan.
- `WorkerProcessRuntime::mode_gated_pty_input` still blocks and is reachable through `CoreDaemon` (`daemon.rs:1669`). It is a control-plane request, not the pump, and the audit lists request closures on the shared thread separately.
- Session spawn, snapshot boundary synchronization, and the two-second shutdown drain (`daemon.rs:2138`) remain synchronous on the owner thread. The audit already lists them.

## Conventions checked

- [[core terminal progress is wake driven and targeted]]: the plan keeps every pump wake-driven. The deadline clamp enforces a timeout, not ordinary progress, which the note allows.
- [[worker resize acknowledgment precedes the next control frame]] and [[session registry size follows the worker applied resize]]: preserved by keeping reconcile as the only completion path.
- [[botster Hub Rust stays a trusted host kernel]]: no terminal semantics move into Hub.
- [[session ingress wakes retire after bound route delivery not lifecycle commit]]: unaffected. The pending-resize wake is a reader-thread session wake, not a lifecycle retirement.

## Required plan edits, in order

1. Rewrite "Implementation requirements" per Finding 1. Name the deletions.
2. Add the deadline mechanism and the chosen expiry rule per Finding 2. Add the exited-session rule.
3. Scope requirement 6 to the ingress path and add the explicit path to Exclusions, or state the wider scope.
4. Add the two vault gotchas and the test list from Finding 4 to "Required behavioral tests".
5. Update the revision record per Finding 5.

After these edits I will approve without another full pass, unless the coordinator changes the expiry rule to something not listed here.

## Addendum: coordinator follow-up questions

### Pending-entry bound

The coordinator is correct. The control queue does not bound accepted-but-unacknowledged entries. `run_control_writer` pops each frame and frees ordinary capacity when it pops, before the worker reads or acknowledges the frame (`worker_process.rs:2893-2900`, `control_queue.rs:217-242`). The queue lane holds 30 ordinary frames (`WORKER_CONTROL_QUEUE_FRAMES` 32 minus `WORKER_CONTROL_RESERVED_SLOTS` 2). After the writer drains them, the kernel socket buffer absorbs more frames until the write blocks for `WORKER_CONTROL_WRITE_TIMEOUT` (2 s) and the writer fails the session. That bound is finite, but it is a byte bound in the kernel, not an explicit entry bound, and it is not coupled to acknowledgements.

The worker cannot read a `FRAME_RESIZE` without acknowledging it, because the acknowledgement is emitted inside the same loop arm before the next read. Unbounded growth therefore needs frames buffered between the writer and the worker loop. That is exactly the socket buffer case above.

Proposed explicit cap, using existing park/retry:

- Add a per-session pending-resize cap. Use the same constant as the ordinary lane, 30, so it needs no new tunable.
- Enforce it in `apply_woken_terminal_input` inside the per-owner loop (`managed_session_runtime.rs:358-392`). Before `take_one_terminal_input`, peek the owner's head command. If the head is `Resize` and the session's pending length has reached the cap, call `park_for_capacity(&key)`, insert the session into `full_sessions`, and break. This is the same branch as `ControlAdmission::Full`. Peeking needs a small `ClientWorker` accessor, because `take_one_terminal_input` pops the command.
- Retry is acknowledgement-coupled by construction. `parked_route_keys` retries only owners whose session is in `ingress_sessions` (`client_worker.rs:782-803`). The reader thread's `FRAME_RESIZE_APPLIED` arm emits exactly that session ingress wake. So the parked owner retries when an acknowledgement arrives and reconcile has popped an entry.
- Ordering is preserved. The parked owner's queue is untouched, and later input behind the resize waits in the same queue.

### Expiry must join ordinary wakes

The coordinator is correct. `ManagedSessionRuntime::wait_wakes` returns the expired batch only when the wake batch is empty (`managed_session_runtime.rs:1676-1684`). `CoreDaemon::wait_pump` returns it only on `TimedOut` (`daemon.rs:1178-1183`). After a deadline passes, `clamp_paste_wait` returns zero, but `wait_wakes(0)` still returns any queued wake for B, so continuous B traffic starves expiry. This starvation already exists for paste expiry today.

Proposed correction: in both `wait_wakes` and `wait_pump`, compute the expired batch whenever `now >= next deadline` and merge it into the returned batch. Merge means: append expired adapter routes and expired sessions to the batch, then dedupe. The pump stays targeted, because the merged batch names only sessions with expired deadlines plus the sessions that woke. The daemon already splits the batch per session (`daemon.rs:1233-1254`), so no other pump change is needed. The interruptible variant used by Hub must merge the same way.

### Same-dimension acknowledgement ambiguity with the explicit path

Explicit `CoreDaemon::resize` is excluded, but if a host mixes it with ingress resizes for the same session, two cases exist:

- Same dimensions. The explicit acknowledgement matches the pending ingress front. The worker applied that size, so completion and persisted geometry are correct. The ingress entry completes one acknowledgement early. This is harmless.
- Different dimensions, explicit call after the ingress accept. The explicit path persists its size at once (`daemon.rs:1440`). The ingress acknowledgement then arrives and the completion pump persists the older ingress size over it. The registry regresses to the ingress size while the worker holds the explicit size. Today the blocking wait hides this interleaving, because the explicit request closure runs after the pump returns.

Because Hub does not call the explicit path, the clean cut is to delete `CoreDaemon::resize` and route the seven daemon tests through ingress resize. If the coordinator keeps the method, the plan must record the different-dimension hazard as a known limitation of the excluded path.

## Revised plan verdict

Reviewed the revised plan at `botster-hub/docs/plans/2026-09-04-nonblocking-resize.md` (status "revised after Fable review").

**Approved, with two edits the coordinator can apply without another review round.**

Findings 1 to 5 are resolved. The plan now names the existing completion chain, lists the exact deletions, defines expiry as session-local control failure through the writer-failure teardown with retained geometry and cleared entries, merges expired sessions into nonempty batches, excludes the explicit path, names both vault gotchas, lists the changed tests, and records the current Hub main.

Edits to apply:

1. Replace "The pending-entry bound and retry wake require final review before implementation" with the cap rule from the addendum: per-session pending-resize cap of 30 (the ordinary lane constant), enforced in the per-owner apply loop by peeking the head command before `take_one_terminal_input`, parking the owner with `park_for_capacity` and `full_sessions` as the `ControlAdmission::Full` branch does. Retry rides the acknowledgement's session ingress wake through `parked_route_keys`. Add one test: fill the cap with the gate held, prove the owner parks and later input stays queued behind the resize, release the gate, prove the parked owner resumes on the acknowledgement wake alone.
2. In Exclusions, add one sentence to the explicit-path item: a different-dimension explicit resize issued after an ingress accept can leave the registry at the older ingress size when the ingress acknowledgement persists later. This is a recorded limitation of the excluded path.

No other change is required. Grok may start after the coordinator applies these edits.
