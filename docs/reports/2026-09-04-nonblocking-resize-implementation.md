# Implementation report: nonblocking resize completion

Date: 2026-09-04. Implementer: Grok. Branch: `foundation/nonblocking-resize`.
Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-nonblocking-resize`.
Base: `93acae3f98adbc21dc981d113c4eb2f31ead4ad0`.
Plan: `botster-hub/docs/plans/2026-09-04-nonblocking-resize.md`.
Review: Fable plan-review commit `b5a7201`. Implementation review commit `de660b0`. The approved plan controls this cut. Fable approved `53ed10f` with follow-ups applied in a later freeze on this branch.

This repository has no `cli/test.sh`. Gates use the documented Cargo commands with `BOTSTER_ENV=test`. The local shell wraps `cargo` through RTK. Raw diagnostics live in RTK tee logs. This run did not invent a wrapper. This run did not edit Hub ticket `ticket_1787600679_990088` or its Core pin.

## Starting state

Verified before edits:

- Worktree path: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-foundation-nonblocking-resize`
- Branch: `foundation/nonblocking-resize`
- HEAD: `93acae3f98adbc21dc981d113c4eb2f31ead4ad0`
- Status: clean
- Ancestor of `93acae3`: yes

Ghostty submodule initialized with:

```bash
git submodule update --init crates/botster-terminal-ghostty/vendor/ghostty
```

Checked out `eb72ec61304ea256be1d86ed8fa961c84e43ecbd`. This is worktree setup, not a source change.

## Production change

The blocking wait is deleted. Ingress resize still uses the existing acknowledgement chain.

1. `CoreDaemon::pump_woken` no longer calls `complete_pending_terminal_resize`.
2. Removed `ManagedSessionRuntime::complete_pending_terminal_resize`, the hidden facade method, the daemon dispatch arm, and `WorkerProcessRuntime::wait_for_resize_applied`. No remaining callers.
3. Pending entries are now `PendingTerminalResize { rows, cols, applied_at, deadline }`. The deadline is `Instant::now() + mode_gated_input_timeout` at accept.
4. `reconcile_terminal_resize_acknowledgments` remains the only completion path. Unmatched acknowledgements are ignored because explicit `CoreDaemon::resize` can emit `FRAME_RESIZE` without a pending entry.
5. Host waits clamp to the earlier of paste deadline and pending-resize front deadline. Expired paste routes and expired resize sessions merge into nonempty wake batches, including sibling traffic.
6. Expiry fails only that session through `mark_control_plane_failed(ResizeAckTimeout)` plus owner teardown. Last confirmed registry geometry is kept. Pending entries are cleared. `pump_woken` does not return a timeout error.
7. Exited or missing sessions drop pending entries. Failed sessions do not keep producing deadline wakes.
8. Pending ingress resizes are capped at the ordinary lane: `WORKER_CONTROL_QUEUE_FRAMES - WORKER_CONTROL_RESERVED_SLOTS` (30). A later resize parks through the existing capacity path and resumes on the acknowledgement session wake.
9. `CoreDaemon::resize` returns `ExplicitResizeBusy` before worker or registry mutation while that session has pending ingress resizes.

Test-only parent gate: `ResizeAckHold` in the reader thread `FRAME_RESIZE_APPLIED` arm. It is armed after attach. Drop/release unblocks the reader.

## Delayed-arrival red/green

The regression uses real workers. Both attachments finish before the hold is armed. The liveness assertion is:

```text
session A resize pump did not return while its acknowledgement remained held
```

Red, blocking path still present:

```bash
BOTSTER_ENV=test cargo test -p botster-core-daemon --test terminal_wake_test delayed_sibling_arrival_progresses_while_resize_acknowledgement_is_held -- --exact --nocapture
```

- Exit status: 101
- Duration: 2.60s after compile
- Panic: `crates/botster-core-daemon/tests/terminal_wake_test.rs:2027`
- Message: `session A resize pump did not return while its acknowledgement remained held`
- Not an attach setup failure (`worker attach did not finish` did not fire)
- Raw log: `/Users/jasonconigliari/Library/Application Support/rtk/tee/1788563151_cargo_test.log`

Green, blocking wait removed:

```bash
BOTSTER_ENV=test cargo test -p botster-core-daemon --test terminal_wake_test delayed_sibling_arrival_progresses_while_resize_acknowledgement_is_held -- --exact --nocapture
```

- Exit status: 0
- `1 passed` in 0.50s, later exact rerun 0.47s

`CoreDaemon` is `!Send`. The daemon stays on the test thread. The pump-return bound is elapsed time on that thread, shorter than A's deadline. Later steps use short `wait_wakes` bounds. A drop guard releases the gate.

## Changed test expectations

- `pump_woken_worker_resize_updates_live_pty_registry_and_one_patch` and `pump_woken_worker_resize_isolates_the_named_sibling`: registry geometry is asserted after the completion wake, not after the accept pump. Input-result counts at accept are unchanged.
- `pump_woken_same_wake_resize_then_input_survives_resize_completion`: first pump still emits both results and must not wait. Registry stays at 24x80 until the retained completion wake is pumped. Wake order and the later echo pump stay. `wait_wakes(Duration::ZERO)` is replaced by a short poll because the accept pump no longer blocks until acknowledgement.
- `pump_woken_preserves_mixed_resize_and_input_with_same_session_sibling`: same completion-wake persist order.
- `one_slot_adapter_preserves_resize_input_and_echo_wake_obligations`: pressure, slot, and three-stage wake assertions stay. Registry geometry is asserted after the completion wake is pumped. Retained-wake wait no longer uses `wait_pump(Duration::ZERO)` as the only observation.
- `stalled_resize_acknowledgment_does_not_block_a_later_named_sibling`: no longer expects `Err("resize acknowledgment timed out")` from the first pump. The accept pump returns `Ok`. Deadline handling is session-local `ControlWriterError::ResizeAckTimeout`, unchanged last-confirmed geometry, live sibling traffic through the deadline, and no further A deadline wakes after pending is cleared.
- `drain_resize_persist_failure_still_emits_bound_queue_wake` and `observe_resize_persist_failure_still_emits_bound_queue_wake`: unchanged. They persist applied attach resize on `drain` / `observe_session_lifecycle`, not ingress completion on `pump_woken`.

New tests: delayed sibling arrival, pending cap park/resume, repeated equal dimensions, explicit busy then retry, teardown plus late acknowledgement.

## Commands and exit statuses

Repository gates. RTK wraps `cargo`. Counts below are RTK summaries plus tee logs.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | pass |
| `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings` | 0 | pass |
| `BOTSTER_ENV=test cargo test -p botster-core-daemon --test terminal_wake_test -- --test-threads=1` | 0 | 54 passed, 33.76s |
| `BOTSTER_ENV=test cargo test --workspace` | 0 | 967 passed, 1 ignored, 132.55s |
| `BOTSTER_ENV=test cargo test -p botster-core --no-default-features --lib` | 0 | 41 passed |
| `BOTSTER_ENV=test cargo test --doc --workspace` | 0 | 9 passed |
| `cargo build -p botster-core-daemon --bin botster-session-worker` | 0 | worker built from this worktree |

Worker-backed tests call `cargo build -p botster-core-daemon --bin botster-session-worker` from the repository root through `worker_path()`. The focused suite and the explicit build use the same source revision as the tested library.

## Changed files

- `crates/botster-core/src/runtime/control_queue.rs`
- `crates/botster-core/src/runtime/mod.rs`
- `crates/botster-core/src/runtime/worker_process.rs`
- `crates/botster-core/src/lib.rs`
- `crates/botster-core/src/engine/client_worker.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/tests/local_session_worker_process_test.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/lib.rs`
- `crates/botster-core-daemon/tests/terminal_wake_test.rs`
- `docs/reports/2026-09-04-nonblocking-resize-implementation.md`

## Remaining blockers

- Downstream Hub/Web/TUI matrix is not run here. The coordinator owns isolated validation against a copy of the selected Hub revision.
- Mode-gated input timeouts are still not clamped except through the existing paste path generalized here for pending resize.
- `WorkerProcessRuntime::mode_gated_pty_input` still blocks on the control-plane request path.
- Session spawn, snapshot boundary synchronization, and the two-second shutdown drain still block the owner thread.
- Explicit `CoreDaemon::resize` still persists immediately when no ingress resize is pending. The busy guard covers the pending-ingress interleaving that the old blocking pump hid.

This change does not fix every shared-pump block. It removes the resize-acknowledgement wait from `pump_woken`.

## Follow-up after Fable `de660b0`

Coordinator required three items on the same branch. No Hub pins.

### 1. Independent delayed-arrival bound

The `!Send` daemon is constructed and dropped inside a scenario thread. The test thread waits on `recv_timeout` per step. A missed pump bound releases the hold and panics with the liveness message before any `join`. Attach timeout remains `worker attach did not finish`.

Red, blocking pump restored only for this run (pending-resize wait loop in `CoreDaemon::pump_woken`, then reverted):

```bash
BOTSTER_ENV=test cargo test -p botster-core-daemon --test terminal_wake_test delayed_sibling_arrival_progresses_while_resize_acknowledgement_is_held -- --exact --nocapture
```

- Exit status: 101
- Duration: 0.84s
- Panic: `terminal_wake_test.rs:2118`
- Message: `session A resize pump did not return while its acknowledgement remained held`
- Attach setup completed (`DelayedArrivalStep::Attached` was received first)
- Raw log: `/Users/jasonconigliari/Library/Application Support/rtk/tee/1788565376_cargo_test.log`

Green after removing the ablation:

- Exit status: 0
- `1 passed` in 0.49s

### 2. Expiry skips control failure when the engine session is Stopping or Exited

`reconcile_terminal_resize_acknowledgments` inspects `ManagedSessionRuntime::session` lifecycle. It does not read the host registry. If the engine session is `Stopping` or `Exited`, pending entries are cleared and `fail_expired_pending_resize` is skipped so undelivered `ProcessExited` can still reach the bound route.

New test: `expired_pending_resize_does_not_drop_undelivered_process_exit`.

- Exit status: 0
- `1 passed` in 1.29s

### 3. Teardown versus held acknowledgement

Releasing `ResizeAckHold` after `shutdown` does not work. The parent reader holds `FRAME_RESIZE_APPLIED` before it queues the acknowledgement, so worker stdout is not drained and shutdown hits:

```text
worker session shutdown did not complete before the daemon deadline
```

Raw log of that attempt: `/Users/jasonconigliari/Library/Application Support/rtk/tee/1788565334_cargo_test.log` (exit 101, 2.42s).

The teardown test therefore releases first, then shuts down. That proves pending cleanup and a harmless later pump. It does not prove post-teardown acknowledgement arrival. Coverage of late acknowledgement after teardown remains partial because of this gate.

### Follow-up gates

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | pass |
| `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings` | 0 | pass |
| `BOTSTER_ENV=test cargo test -p botster-core-daemon --test terminal_wake_test -- --test-threads=1` | 0 | 55 passed, 41.64s |
| `BOTSTER_ENV=test cargo test -p botster-core --no-default-features --lib` | 0 | 41 passed |

Passing suite log: `/Users/jasonconigliari/.grok/sessions/%2FUsers%2Fjasonconigliari%2Fbotster-sessions%2Ftrybotster-botster-core-foundation-nonblocking-resize/01a06ea1-6567-7b11-892c-9cb1bf388272/terminal/call-bf2c5f0e-94d7-4d8b-9018-1be6def7b981-315.log`. RTK tee files in this window captured failing runs; passing counts above are from those command exits.

Workspace and doctest gates were not repeated. The follow-up did not change production behavior outside expiry skip and tests.

## Review freeze

Implementation and this report are committed on `foundation/nonblocking-resize` only. Do not merge. Do not publish. Fable reviews the follow-up commit on this branch.
