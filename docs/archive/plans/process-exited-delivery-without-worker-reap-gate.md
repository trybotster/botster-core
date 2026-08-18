# Deliver ProcessExited without gating on worker reap or exit status

- Ticket: `ticket_1787015956_494734` — "Core: deliver ProcessExited without gating on worker reap or exit status"
- Run: `run_1787015981_429380` (Botster Stack Delivery, Plan step)
- Target repository: **botster-core** (`tgt_1f7bce66eb304881980f9b4a2a5ae3fe`, trybotster/botster-core)
- Base revision: `fc541a5`
- Downstream dependent: Hub `ticket_1786977409_499180` (Hub repins to merged Core main and completes the W1/W2 forced-window proofs)

## Problem

`WorkerProcessRuntime::drain_output` (`crates/botster-core/src/runtime/worker_process.rs:1316-1329`) holds a received `ProcessExitedPayload` in `WorkerCompletion` and only emits `SessionRuntimeOutput::ProcessExited` when **all** of these hold:

1. `completion.reader_finished` is true (worker stdout reached EOF), and
2. `child.try_wait()` returns `Some(status)` (the worker child is reapable now), and
3. `status.success()` is true (the worker exited zero).

Two deterministic failures at `fc541a5`:

- **W1 (reap timing):** the worker child stays alive (unreaped, stdout open) longer than the daemon's 2-second shutdown deadline after the session process exits. The payload is held, `engine_session_exited` never becomes true, and a blind `ShutdownSession` returns the `ShutdownFailed` deadline error (Hub surfaces typed OperatorError runtime_error/state_error, operation=shutdown).
- **W2 (exit status):** the worker child exits non-zero after the session process succeeded. `status.success()` is false, the payload is suppressed **permanently**, the registry row stays `Running`, and every later `ShutdownSession` fails. Matches Verify pair-run evidence (`last=Some("running")` for 10s with the producer worker dead).

A parent wrapper cannot serve as a Hub-only fixture: Core binds readiness and welcome `worker_pid` to the spawned child, so wrappers fail spawn. Core must own both the fix and the forced-window fixtures.

## Required contract (from ticket)

- A received `ProcessExited` payload is session-exit truth.
- Delivery to drains and observes must not gate on the worker process's own reap timing or exit status.
- The daemon must not block on `child.wait()` on the drain path.
- A worker connection that dies **without** a payload keeps current true-error semantics.
- Core chooses the mechanism; exposing pending-exit evidence through `observe_session_lifecycle` is an acceptable alternative.

## Mechanism decision

**Chosen: fix delivery at the runtime source (`drain_output`).** Deliver the terminal payload as soon as `completion.process_exited` is populated, with no `reader_finished` gate, no `try_wait` gate, and no `status.success()` gate.

Rejected alternative: surfacing "pending-exit evidence" through `observe_session_lifecycle`. `observe_session_lifecycle` already routes through `observe_session` → the same engine drain, so fixing `drain_output` corrects **both** drains and observes with one change, no new public evidence type, and no Hub-side interpretation burden. The alternative would add public surface while leaving the runtime state wrong ([[botster core public surface needs a narrow start here path]], smallest-surgical-change rule).

Why the `reader_finished` gate must also go: in W1 the lingering worker holds stdout open, so `reader_finished` stays false for the whole hold. Any delivery rule that waits for worker EOF re-encodes "worker's own exit timing" and fails W1. The payload alone is the truth signal. The reader-EOF-without-payload path (true connection error) does not change.

## Changes

### 1. `crates/botster-core/src/runtime/worker_process.rs` — `drain_output`

- Terminal condition becomes: `completion.process_exited.clone()` is `Some`. Remove the `reader_finished` conjunct and the entire `terminal_payload.filter(...)` child/status filter.
- **Ordering guard:** after observing the payload and before emitting `ProcessExited`, drain the session output channel once more into `output` (re-run the pump or an equivalent channel drain). Rationale: the reader thread stores the payload only after every earlier frame was accepted into the channel, but the drain's single pump at entry can race those sends; removal without a re-pump could drop final PTY bytes. The worker contract sends no frames after `FRAME_PROCESS_EXITED` (worker lifecycle goes `Exited` and the loop ends), so one re-pump is sufficient.
- **Non-blocking removal/reap:** in the removal branch, replace unconditional `child.wait()` with:
  - `child.try_wait()` → `Some(_)`: child already exited; it is reaped by `try_wait`; done (ignore status).
  - `None` (W1, child still alive): keep `close_before_blocking_shutdown()` and `control.cleanup()` on the drain thread, then move the `Child` into a detached background reaper thread. The reaper polls `try_wait` with a short sleep up to a bounded grace (2s, matching the existing shutdown deadline scale), then `kill()`, then `wait()`. The drain thread never blocks.
- No change to the reader thread, `WorkerCompletion`, ping/health, mode-gated error paths, or the `Drop for WorkerProcessRuntime` full-teardown path.

### 2. Forced-window test knobs (Core-owned fixtures)

Follow the existing `test_*` chain (option field → `--test-*` argv → worker `Args` parse), and the existing `CoreDaemonConfig` plumbing (`daemon.rs:435-446`) so Hub can force the windows after repin:

- `test_hold_before_exit_ms` / `--test-hold-before-exit-ms`: after the worker sends `FRAME_PROCESS_EXITED`, the worker sleeps this long **with stdout open** before exiting (reproduces W1).
- `test_exit_code` / `--test-exit-code`: the worker exits with this non-zero code after the payload is flushed (writer joined) (reproduces W2).

Add matching `WorkerProcessRuntimeOptions` and `CoreDaemonConfig` builder methods next to the existing `with_test_*` builders.

### 3. Tests

Runtime level (`crates/botster-core/tests/local_session_worker_process_test.rs` or sibling):

- **W1:** spawn a real worker with `--test-hold-before-exit-ms` well above 2s; run a short command; assert `drain_output` emits `ProcessExited` while the hold is still active (elapsed ≪ hold) and the session is removed; assert the worker pid is eventually gone (reaper worked) via `kill(pid, 0)`-style polling on the welcome `worker_pid`.
- **W2:** spawn a worker with `--test-exit-code 1`; assert `ProcessExited` is delivered with the session process's payload and the session is removed.
- **Dead-without-payload regression:** worker connection dies with no payload (existing coverage; extend if absent): no `ProcessExited` is emitted and the existing error semantics (`OutputFailed`/ping failure paths) hold.

Daemon level (`crates/botster-core-daemon/tests/daemon_integration_test.rs`):

- **W1 forced window:** blind `shutdown(Some(session))` during the hold completes `Ok` within the 2-second deadline; the registry row is `Exited`; `observe_session_lifecycle` returns `Found` with an exited row while the worker child is still alive. This is the Hub-shaped production path: ShutdownSession → `daemon::shutdown_session` → engine drain → `drain_output` → lifecycle `Exited`.
- **W2 forced window:** after the non-zero worker exit, `observe_session_lifecycle` reports the exited row (no permanent `Running`), and `shutdown` succeeds.
- Both tests are red on revert of the `drain_output` change (deadline error / running row), giving the red-on-revert control.
- Give time-based assertions wall-clock slack (repo precedent `aef6516`).

Contract tests (`session_runtime_contract_test.rs`, managed runtime tests): update any expectation that encodes the old gate; the fake runtime already delivers payloads directly, so drift is expected to be small.

### 4. Docs

- Add the delivery contract to `docs/architecture/durable-session-worker-protocol.md`: a received `ProcessExited` payload is session-exit truth; parent delivery does not gate on worker reap timing or worker exit status; the worker sends no frames after `FRAME_PROCESS_EXITED`; connection death without a payload remains an error path.
- This plan file lands in `docs/archive/plans/` with the change (repo convention: plans are archived landed artifacts; living truth goes to `architecture/`).

## Scope and non-scope

In scope:

- `drain_output` delivery gate + non-blocking reap in `crates/botster-core/src/runtime/worker_process.rs`.
- Two worker test knobs + argv/options/config plumbing (`botster-session-worker.rs`, `worker_process.rs`, `daemon.rs` config).
- Runtime, daemon, and contract tests above; the durable-session-worker-protocol doc addition.

Non-scope (explicit):

- `local_process.rs` `try_wait` on the **session** PTY child (line 942): that gates on the actual session process, which is the correct truth source; untouched.
- `Drop for WorkerProcessRuntime` full-daemon teardown (blocking waits after `FRAME_SHUTDOWN`): pre-existing behavior, not on the drain path; untouched (noted as risk).
- `is_worker_process`, ping/health, mode-gated input error paths: untouched.
- Hub-side reconciliation, repin, OperatorError mapping, and the Hub W1/W2 forced-window proofs: owned by Hub `ticket_1786977409_499180`.
- No new public evidence enum on `observe_session_lifecycle`; no speculative configurability beyond the two test knobs.

## Ownership boundaries and cross-repo dependencies

- **botster-core owns** the worker runtime delivery mechanism, the worker binary, the daemon classification query, and the forced-window fixtures (a parent wrapper is impossible outside Core; readiness/welcome bind to the spawned child).
- **botster-hub owns** ShutdownSession policy, OperatorError typing, reconciliation, and repinning. This ticket is the registered blocking dependency of Hub `ticket_1786977409_499180`; no new dependency registration is needed in this run (the Hub ticket already depends on this one).
- Seam: `CoreDaemon::shutdown` / `observe_session_lifecycle` / lifecycle journal rows are the consumer-visible surface; charter requires downstream-shaped proof, satisfied by the daemon-level Hub-shaped tests ([[botster core contract surface needs consumer proof]], [[host ShutdownSession classification must call the exact-session Core query]]).

## Assumptions and unknowns

- Assumption: the worker sends no frames after `FRAME_PROCESS_EXITED`. Verified in `botster-session-worker.rs`: `observe_process_exit` ends the loop; the drain batch sends PTY output before the payload because `LocalProcessRuntime` orders final output before `ProcessExited`.
- Assumption: delivering `ProcessExited` while an incremental attach is outstanding reuses existing cleanup (`control.cleanup()` closes the parent path; [[worker snapshot barrier cancels when the parent path closes]] covers the worker side; engine `ProcessExited` handling already covers sessions that die mid-attach). Implementer must confirm with the existing attach/exit tests.
- Assumption: `try_wait` reaps the child when it returns `Some`, so the drain path leaves no zombie for already-exited children.
- Unknown (minor): whether any existing workspace test asserts the old suppression behavior; the implementer updates those tests to the new contract rather than weakening the new tests.

## Runtime-teardown lenses ([[botster runtime teardown lenses]])

- `teardown_class_applies`: yes — SessionIo/worker teardown and terminal-state vs live-runtime divergence (registry `Running` while the producer worker is dead or the session process exited).
- `teardown_isolation`: the ownership set that dies on delivery is one `WorkerProcessSession` (child handle, control connection, reader thread, stall state, pending queues) keyed by one `SessionId`. Removal is a per-session map entry; sibling sessions and their workers are untouched. A reaper thread owns only the moved `Child`.
- `teardown_bounds`: the drain path uses only `try_wait` (never blocks). A live child is handed to a detached reaper bounded by a 2s grace, then `kill()`, then `wait()` (bounded by SIGKILL semantics). The daemon 2s shutdown deadline is unchanged. The `Drop` full-teardown path keeps its pre-existing blocking behavior (out of scope, named as risk).
- `late_message_matrix`:

| Message / event after terminal delivery | Tag | Reject | Sweep |
|---|---|---|---|
| Worker stdout frame after payload | session channel | worker contract sends none after `FRAME_PROCESS_EXITED`; if the receiver is dropped, the reader's send fails and the reader thread exits | stall closed by `close_before_blocking_shutdown` wakes a blocked reader |
| Parent input (Write/Resize/Shutdown/snapshot) after removal | `SessionId` map lookup | `session_mut` returns `SessionNotFound` | none created |
| Attach after removal | engine session lookup | engine attach fails session-not-found; no subscription ownership created | existing engine `ProcessExited` handling clears routes for the removed session |
| Late reaper completion | moved `Child` handle | owns no session state | thread exits after `wait()` |

- `production_path_proof`: Hub blind ShutdownSession → `CoreDaemon::shutdown_session` → engine `drain_runtime_once` → `WorkerProcessRuntime::drain_output` emits `ProcessExited` → `handle_runtime_event` marks lifecycle `Exited` → `reconcile_lifecycle_observations` marks the registry `Exited` → shutdown returns `Ok` inside the 2s deadline. Live oracles: daemon integration tests drive the real worker binary through both forced windows and assert shutdown success, registry row, `observe_session_lifecycle` truth, and eventual pid disappearance; red on revert of the `drain_output` change.
- `ownership_identity`: `SessionId` keys the worker session map and the delivered payload. The reaper owns the moved `Child` handle, so pid reuse cannot mistarget the kill. No subscription-id reuse hazards are introduced.
- `sibling_fail_closed_policy`: on successful delivery and reap, siblings are unaffected. On ultimate reap failure (unkillable child in uninterruptible state), the blast radius is one detached thread and at worst one zombie; the daemon and sibling sessions keep running. No silent sibling sacrifice exists on this path.

## Risks

1. **Final-output ordering race:** payload observed while the last PTY frames sit unpumped in the channel → mitigated by the mandatory re-pump before emitting `ProcessExited`; covered by asserting expected output content in the W1/W2 tests.
2. **Thread/zombie leak from the reaper:** bounded by grace + `kill()` + `wait()`; W1 test asserts eventual pid disappearance.
3. **Hidden dependents on delivery-after-EOF:** any consumer that assumed `ProcessExited` implies worker EOF. Workspace test sweep plus clippy/CI gates catch encoded expectations; the worker protocol doc states the new contract.
4. **Attach-in-flight during early delivery:** relies on existing parent-close cancel and engine exit handling; implementer verifies with existing attach/exit tests before merge.
5. **Time-based test flakiness:** use generous holds and wall-clock slack per repo precedent (`aef6516`).

## Acceptance checks

CI-owned gates ([[botster-core uses CI-owned Cargo commands because it has no test script]]):

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo test --doc --workspace` and `cargo doc --workspace --no-deps`
5. `cargo test -p botster-core --test local_process_runtime_test` (dedicated CI job)
6. `script/terminal-protocol-node-smoke.sh` (unchanged surface; must stay green)

Ticket-specific proofs:

7. New W1 runtime + daemon tests green: `ProcessExited` delivered and shutdown `Ok` within 2s while the worker child is alive and unreaped; `observe_session_lifecycle` returns the exited row during the hold.
8. New W2 runtime + daemon tests green: non-zero worker exit after session success still delivers `ProcessExited`; no permanent `Running` row; shutdown succeeds.
9. Dead-without-payload path unchanged: no `ProcessExited`, existing error semantics preserved (regression test).
10. Red-on-revert control: reverting the `drain_output` change makes the W1/W2 tests fail with the deadline error / running row.
11. Downstream-shaped proof: the daemon-level tests exercise the exact Hub call pattern (blind ShutdownSession + exact-session lifecycle query). Full live Hub proof (repin + forced windows) belongs to Hub `ticket_1786977409_499180` after merge.

## Vault gaps worth capturing

- The delivered contract as an atomic note: "a received ProcessExited payload is session-exit truth; Core delivery never gates on worker reap timing or exit status" (capture after Implement/Verify land).
- The forced-window fixture pattern: "worker forced-exit windows are Core-owned test knobs because welcome/readiness bind to the spawned child" (wrapper fixtures are impossible downstream).
- The bounded background reaper pattern for non-blocking child reaping on drain paths.
