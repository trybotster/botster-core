# Review: nonblocking resize implementation commit 53ed10f

Date: 2026-09-04. Reviewer: Fable (session `sess-1788561640-0030-9ab4035a5a3867e8f4970b3a6e440510`).
Reviewed commit: `53ed10f2387d532c8269669522e8c8f7f357af3c` on `foundation/nonblocking-resize`, base `93acae3f98adbc21dc981d113c4eb2f31ead4ad0`.
Implementation worktree (read only): `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-nonblocking-resize`.
Review worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-resize-review`.
Isolated verification checkout: a detached scratch worktree at `53ed10f` under this session's scratchpad. The implementation worktree was not modified.

Plan: `botster-hub/docs/plans/2026-09-04-nonblocking-resize.md` (approved). Implementation report: `docs/reports/2026-09-04-nonblocking-resize-implementation.md` at the reviewed commit.

## Verdict

**Approved.** Two non-blocking follow-ups are listed under "Findings". Neither changes production behavior for the audited defect, and neither needs a second review round if Grok applies them as stated.

Confirmed frozen: implementation worktree HEAD is `53ed10f`, status clean, one commit over base.

## Independent verification

| Gate | Result | Source |
| --- | --- | --- |
| `cargo test -p botster-core-daemon --test terminal_wake_test -- --test-threads=1` | 54 passed, 0 failed, 33.82s | my run, isolated checkout at `53ed10f` |
| `cargo fmt --all -- --check` | pass | my run |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass | my run |
| `cargo test --workspace --no-fail-fast` | 967 passed, 0 failed, 1 ignored | my run, second attempt; see flake note |
| `cargo test -p botster-core --no-default-features --lib` | 41 passed | my run |
| `cargo test --doc --workspace` | 9 passed | my run |

Red evidence: RTK tee log `1788563151_cargo_test.log` (16:05) shows the delayed-arrival test failing at `terminal_wake_test.rs:2027` with the message `session A resize pump did not return while its acknowledgement remained held`, 2.60s, exit 101. The attach message `worker attach did not finish` is absent. This reaches the liveness assertion as the plan requires.

Flake note: my first workspace run stopped at one failure, `welcome_must_identify_the_exact_spawned_worker` in `crates/botster-core/tests/local_session_worker_process_test.rs:1504`. Its fake-worker server thread timed out waiting for a shell script pid file. The same test fails on base `93acae3` source in my review worktree under concurrent build load, and passes three of three in isolation on `53ed10f`. The resize change touches that file only to add a default option field. This is a pre-existing timing-sensitive test, not a regression from this commit. The no-fail-fast rerun passed the full workspace.

Evidence gap: Grok's report cites passing runs for every gate, but no RTK tee log exists for them. The only cargo test logs in the window are three failing development runs at 16:05, 16:13, and 16:16, all before the 16:23 commit. The failing tests in those runs are the resize tests Grok was still shaping. I did not rely on Grok's counts. The table above records my own runs.

## Behavior checked against the diff

Deadline handling with nonempty batches. `merge_deadline_wakes` (`managed_session_runtime.rs:1763-1780`) appends expired paste routes and expired resize sessions to whatever batch the wake source returned, then dedupes. `wait_wakes` calls it on every return (`:1738`). `wait_pump` calls it on `Wakes`, `TimedOut`, and the fallback arm (`daemon.rs:1203-1214`). The `Interrupted` arm returns without merging, which is correct because the Hub driver immediately re-waits with a zero timeout after an interrupt, and that call merges. `clamp_paste_wait` now clamps to the earlier of paste and resize-front deadlines (`:1741-1751`). The stalled-sibling test keeps B injecting input every 50 ms across A's deadline and asserts the pump returns `Ok` and A fails (`terminal_wake_test.rs`, stalled test loop). Correct.

Session-local failure and owner cleanup. `fail_expired_pending_resize` removes the pending entries, calls `mark_control_plane_failed(ResizeAckTimeout)`, and pushes `client_worker.teardown_session` into `pending_input_teardowns` (`managed_session_runtime.rs:216-223`). This is the same sequence the writer-failure path uses (`:299-311`). The expired batch names the session in `ingress_sessions`, so phase three processes the owned teardowns in the same pump. `pump_woken` no longer returns a resize error, and the daemon's commit-retry path is untouched. Sibling routes are not named and are untouched. Correct.

Pending cap and acknowledgement-only retry. `PENDING_INGRESS_RESIZE_CAP` is `WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS` (30). The check runs inside the `Ready` arm before `take_one_terminal_input`, peeks the head with the new `terminal_input_head`, parks the owner, inserts the session into `full_sessions`, and breaks (`:387-399`). Retry uses `parked_route_keys`, which requires the session in `ingress_sessions`; the reader thread's `FRAME_RESIZE_APPLIED` arm emits that wake. The cap test fills 16 then 14 entries, proves the 31st resize and the input behind it park, releases the hold, and proves the owner resumes with one more resize result and the input result, with pending never above 30. Correct.

Explicit resize busy guard. `CoreDaemon::resize` checks `has_pending_terminal_resizes` after `ensure_session_mutable` and before `engine.resize` and `persist_session_size` (`daemon.rs:1457-1459`). The error is a distinct `ExplicitResizeBusy(SessionId)` variant. The test proves busy, unchanged geometry, sibling explicit resize unaffected, and success after completion. Correct.

Stale acknowledgements and teardown. `acknowledge_terminal_resize` keeps the ignore-mismatch rule with the explicit-path comment (`:1024-1030`). Reconcile removes pending entries when `take_resize_applied` reports `SessionNotFound` and the engine session is absent or exited (`:191-197`). Shutdown, forget, and restart still clear both collections. `next_pending_resize_deadline` and `expired_pending_resize_sessions` skip sessions the engine no longer holds. The teardown test proves pending is zero after shutdown and that a later pump is harmless. Correct, with one weakness noted in Finding 3.

Deletions. `complete_pending_terminal_resize` is gone from the daemon pump, the daemon dispatch enum, the facade, and the managed runtime. `wait_for_resize_applied` is gone. A source grep of the implementation worktree finds no remaining callers. `GATED_POLL` still has other users. Correct.

Preserved one-shot wakes. `wait_session_ingress_wake` waits on Core wakes with a 20 ms bound in a 1 s loop. It injects no frames and does not sleep. When it receives a batch that does not name the target session, it pumps that batch, which is what a host does with a real wake. It never pumps an empty batch. The retained completion wake is still observed as `ingress_sessions == [session]` and pumped once. The one-slot test's loop is stricter: it asserts that no adapter route arrives while waiting. The "echo absent after the completion pump" assertion is retained. That assertion was timing-dependent by construction before this change, as the vault note records, and the change does not widen the window because the wait returns as soon as the wake exists. The three-pump chain is preserved. Acceptable.

Test cleanup. Each gated test holds a `ReleaseResizeAckHoldOnDrop` guard declared before the daemon, so the guard drops after the daemon and the reader thread cannot stay blocked past the test. Data directories are removed at the end. Acceptable.

## Findings

### 1. Delayed-arrival liveness bound is not a hard bound (non-blocking)

The test measures elapsed time around `pump_woken` and asserts under 400 ms. Against the old implementation this works because the blocking wait returns after the configured 2 s timeout, and the red log confirms 2.60 s. Against an implementation that never returns, the assertion never runs and the test hangs, because Cargo has no per-test timeout. The plan asked for a scenario thread with `recv_timeout` bounds on the test thread. Grok's report says `CoreDaemon` is `!Send` and so stays on the test thread. That justification does not hold: the daemon can be constructed and dropped inside the scenario thread without crossing threads.

Recommended follow-up: run the scenario in a spawned thread that owns the daemon. On the test thread, wait on a channel with `recv_timeout` per step. On a missed bound, release the hold first so a blocked pump can unwind, then fail with the liveness message. This keeps cleanup through drop even for a hung pump.

### 2. Expiry does not check lifecycle before failing the session (non-blocking, low risk)

In `reconcile_terminal_resize_acknowledgments`, when `take_resize_applied` returns `Ok`, the code fails the session if the front entry expired (`:200-202`). A session whose lifecycle is already `Exited` or `Stopping` but whose worker entry is still in the runtime map would be marked failed and its owners torn down, which could drop an undelivered `ProcessExited` frame. The window is small: exit drain removes the worker entry on its own wake, and shutdown clears pending first. Recommended one-line hardening: when the lifecycle is not live, remove the pending entries and skip the failure.

### 3. Late-acknowledgement test releases the hold before shutdown (non-blocking)

`teardown_clears_pending_resize_and_ignores_late_acknowledgement` releases the hold and then shuts the session down. The acknowledgement can already be queued before teardown, so the test proves cleanup and a harmless later pump, not a true post-teardown acknowledgement. The structural argument still holds: the acknowledgement channel and reader belong to the removed worker session. Releasing after shutdown would block shutdown on the held reader, so the current order is a reasonable compromise. Record it as partial coverage.

## Notes, no action required

- `clamp_paste_wait` and `expired_paste_wake_batch` now cover resize deadlines. The names are stale. A rename is optional.
- `ResizeAckHold` and `PENDING_INGRESS_RESIZE_CAP` are public. `pending_terminal_resize_len` and `control_plane_state` on `CoreDaemon` are `#[doc(hidden)]`, consistent with the hidden method this change removed.
- Hub needs no source change. The driver already calls `wait_pump`, which merges deadlines.
- Remaining blocking paths listed in Grok's report match my plan review.
