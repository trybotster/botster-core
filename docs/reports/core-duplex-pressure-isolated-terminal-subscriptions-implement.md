# Implementation report: duplex pressure-isolated terminal subscriptions

Ticket: `ticket_1787600672_342292`
Run: `run_1787632374_189517`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/core-duplex-pressure-isolated-terminal-subscriptions.md` (Revision 9)

## Target repository and target_id

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Authoritative spawn path: the `botster-core` target root
- Pipeline worktree: the ticket worktree for `ticket_1787600672_342292`
- Branch: `project-pipelines/ticket_1787600672_342292`
- Base commit: `7eafa47`
- Merge policy: `direct` (no PR)

Routing used the run `target_id`, not the ambient directory.

## Repository playbook and other playbooks/notes applied

Repository charter: [[botster-core-playbook]]

Role and overlay:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster runtime teardown lenses]]
- [[botster-architecture]]
- [[core owns duplex terminal transport while Hub stays content blind]]
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[implementation artifacts must match actual git state]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation steps must persist report artifacts for review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

Not loaded:

- [[project-pipelines-playbook]] — package/plugin paths are out of scope
- Hub, Web, TUI, Ghostty, and workspaces charters — this run stays inside `botster-core`

## Files changed

The `7eafa47...HEAD` set plus this return visit:

Protocol and generated package:

- `crates/botster-terminal-protocol/**`
- `crates/botster-terminal-protocol-client/**` (0.2.0)
- `packages/terminal-protocol/**` (0.2.0, fixture revision 2)
- `script/terminal-protocol-node-smoke.sh`

Core runtime and daemon:

- `crates/botster-core/src/runtime/control_queue.rs` (new)
- `crates/botster-core/src/runtime/worker_process.rs`
- `crates/botster-core/src/runtime/local_process.rs`
- `crates/botster-core/src/runtime/mod.rs`
- `crates/botster-core/src/engine/client_worker.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/client_worker.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/src/engine/botster/takeover_fail_closed_tests.rs`
- `crates/botster-core/src/engine/mod.rs`
- `crates/botster-core/src/contract/session_protocol.rs`
- `crates/botster-core/src/contract/terminal_adapter.rs`
- `crates/botster-core/src/contract/terminal_subscription.rs`
- `crates/botster-core/src/lib.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs`

Test support and proof:

- `crates/botster-core-test-support/src/terminal_adapter/**`
- `crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/**`
- `crates/botster-core/tests/session_protocol_test.rs`
- `crates/botster-core/tests/client_worker_engine_test.rs`
- `crates/botster-core/tests/local_process_runtime_test.rs`
- `crates/botster-core/tests/local_session_worker_process_test.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`

Docs:

- `docs/architecture/terminal-adapter.md`
- `docs/architecture/terminal-protocol.md`
- `docs/archive/plans/core-duplex-pressure-isolated-terminal-subscriptions.md`
- `docs/reports/core-duplex-pressure-isolated-terminal-subscriptions-implement.md`

## Ownership boundaries preserved

- Core owns reusable policy-free runtime, protocol crates, ClientWorker, worker control plane, and daemon drain.
- Hub-safe `botster-terminal-protocol` stays content-blind: opaque `TerminalInputFrame` only.
- Semantic encode/decode and `input_result` live in `botster-terminal-protocol-client`.
- Hub DataChannel, Web, TUI, and Ghostty consumers were not edited.
- `PROTOCOL_VERSION` stays 1. `WRITE_ATTEMPT_BUDGET` stays 512.
- `CoreDaemon::mode_gated_input` / `input` / `resize` remain for the cold-cut ticket.

## Cross-repo dependencies or separately routed work

- Live Hub adapter consumption is `ticket_1787600674_500120`.
- Snapshot delivery split remains a later ticket.
- Cold-cut of the legacy JSON input path is `ticket_1787600679_990088`.
- npm publish is human-owned and was not performed.

## Deviations from plan

- Stdio writers use a non-blocking deadline loop. Socket writers use `set_write_timeout` only. `set_nonblocking` on a cloned `UnixStream` also marks the reader clone non-blocking and makes the stdout reader fail closed. `SO_SNDTIMEO` on stdio pipes failed closed the control plane on the first post-spawn write.
- Runtime Drop no longer blocks on `Child::wait()` after a sealed writer. It kills and background-reaps, matching the process-exit path. A blocking wait deadlocked when the writer still held `ChildStdin`.
- Local in-process apply rejects `ModeGatedInput` with `SessionNotWritable` instead of inventing a local gated lane.
- Worker-backed apply intakes but does not send PTY bytes while IncrementalAttach is active. That keeps the snapshot barrier from seeing a tick-thread control write. After attach completes, the next drain applies the queued commands.
- Daemon duplex tests inject a hand-built compact-binary frame so the daemon test crate does not take a new protocol-client dependency.
- Local apply-error tests use `LocalProcessRuntimeOptions.test_fail_pty_writes` so input and resize fail without inventing a second local write path.
- Queue-bound oracles (`hold_pops`, `class_counts`, `test_control_queue`) stay `#[cfg(test)] pub(crate)`. They are not on the public `WorkerProcessRuntime` seam.
- The worker test hold is interruptible by a matching cancel frame and is one-shot so a later replacement gated request does not inherit the hold.
- Stage B hold includes every session that already occupies a gated slot, plus every session with a parked ClientWorker owner. The drained session id alone is not the hold set.

## Tests and downstream proof run

CI-owned Cargo commands, no wrapper:

- `cargo fmt --all -- --check` — passed
- `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings` — passed
- `BOTSTER_ENV=test cargo test -p botster-core --lib --no-default-features` — 13 passed
- `BOTSTER_ENV=test cargo test -p botster-core --lib --features local-runtime` — passed, including the moved queue-bound unit test
- `BOTSTER_ENV=test cargo test -p botster-core --test client_worker_engine_test` — 28 passed
- `BOTSTER_ENV=test cargo test -p botster-core --test session_protocol_test` — 17 passed
- `BOTSTER_ENV=test cargo test -p botster-core --test local_session_worker_process_test attached_pty_stall_waits_on_drain_or_detach_not_fixed_sleep dropping_parent_runtime_reaps_worker_and_pty_child` — passed
- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- drain_applies_injected_duplex_input drain_queue_overflow drain_reconnects_and_rejects drain_teardown_session drain_writer_failure` — 5 passed
- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output` — passed on this branch after removing the extra IncrementalAttach output pump. The same command passed on base `7eafa47` (exit 0) and failed on `065c2bf` (exit 101).
- `BOTSTER_ENV=test cargo test -p botster-terminal-protocol -p botster-terminal-protocol-client` — passed
- `BOTSTER_ENV=test cargo test --manifest-path crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/Cargo.toml` — passed
- `script/terminal-protocol-node-smoke.sh` — passed
- `BOTSTER_ENV=test cargo test --doc --workspace` — passed
- `BOTSTER_ENV=test cargo test --lib failed_final_capture_installs_no_retained_terminal_state -p botster-core-daemon` — passed. Shutdown keeps the mux route live until the next drain delivers `ProcessExit`. Missing-owner ingest retains `ProcessExit` and drops later `TerminalOutput` / `Snapshot`.
- `BOTSTER_ENV=test cargo test -p botster-core --lib owner_teardown_enqueues_one_cancel_and_leaves_the_shutdown_slot` — passed. Thirty ordinary frames plus one owner teardown enqueue exactly one cancel and leave the reserved shutdown slot. The oracle uses the crate-private test seam, not a public runtime queue handle.
- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test drain_detach_cancels_held_gated_and_leaves_sibling` — passed. The worker hold is 10s and is released only by a matching cancel frame. The test requires the parent lane to accept a replacement probe in under 2s, then `echo:next` from a replacement gated request, with no `echo:hold` and no late `input_result`. It does not sleep past the uncancelled hold.
- `BOTSTER_ENV=test cargo test --workspace` — 815 passed, 1 ignored. This visit added two hold-set regressions and did not change `plugin_worker.rs`. The exact Clippy command above passed on the committed tree.
- `BOTSTER_ENV=test cargo test -p botster-core --test client_worker_engine_test take_keeps_sibling_mode_gated_queued_when_hold_set_omits_that_session` — failed before the hold-set fix, then passed.
- `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test drain_other_session_leaves_queued_gated_on_a_held_sibling` — passed. Draining session A does not reject a queued sibling `ModeGatedInput` on session B while B holds the gated slot.

Production entry point: `CoreDaemon::drain` calls `engine.apply_terminal_input` before `drain_runtime_once`. Adapter `try_read` → decode → `write_bytes` / `submit_mode_gated_pty_input` / resize → worker control queue → session worker PTY.

Added production-path proofs:

- `local_input_result_carries_the_live_subscription_id`
- `local_mode_gated_input_result_carries_the_live_subscription_id`
- `local_apply_errors_fail_closed_and_leave_siblings`
- `intake_refuses_the_command_that_would_exceed_capacity`
- `drain_applies_injected_duplex_input_through_real_worker_pty`
- `drain_queue_overflow_tears_down_one_owner_and_keeps_a_sibling_session`
- `drain_reconnects_and_rejects_stale_generation_ingress`
- `drain_teardown_session_clears_ingress_and_inventory`
- `drain_writer_failure_sweeps_idle_same_session_owner`
- `input_path_hard_stop_unsubscribes_the_multiplexer_route`
- `owner_removal_matrix_closes_adapter_and_route`
- `drain_ingress_loss_and_malformed_input_remove_the_route`
- `drain_detach_cancels_held_gated_and_leaves_sibling` — cancel must reach the worker, release the lane, block `echo:hold`, allow a replacement gated request, and leave the sibling live
- `owner_teardown_enqueues_one_cancel_and_leaves_the_shutdown_slot` — crate unit test behind `test_control_queue`
- `take_keeps_sibling_mode_gated_queued_when_hold_set_omits_that_session`
- `drain_other_session_leaves_queued_gated_on_a_held_sibling`

## Unverified behavior or residual risk

- Same-session subscriptions still share one control channel. Delay then collective hard-stop is the stated policy. Cross-session isolation is the ticket guarantee.
- `record_attach` stays infallible. Attach and bind gates live at `CoreDaemon` and worker-backed bind.
- `ClientWorker::teardown_all` is still unused on a named production entry. Full-runtime removal uses per-session `teardown_session` plus `forget_terminal_session`. The owner-removal matrix covers detach_live, detach_generation, teardown_session, forget, Lost, malformed input, overflow, and PTY apply failure. Each path asserts adapter close, inventory removal, no later `TerminalOutput` / `Snapshot` for the removed generation, and sibling survival. `ProcessExit` stays recovery egress. Gated cancel runs once from `unsubscribe_owner_teardowns`. Detach-while-held is proved by `drain_detach_cancels_held_gated_and_leaves_sibling`.

## Missing vault guidance discovered

None that blocked the work. Writer-timeout gotchas and the Review identity gap were captured to the vault inbox:

- `so-sndtimeo-failure-on-stdio-pipes-must-not-seal-a-botster-control-plane`
- `set-nonblocking-on-a-cloned-unixstream-also-marks-the-reader-clone`
- `every-terminal-input-result-must-stamp-the-live-subscription-id`
