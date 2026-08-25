# Implementation report: duplex pressure-isolated terminal subscriptions

Ticket: `ticket_1787600672_342292`
Run: `run_1787632374_189517`
Step: `botster_stack_implement`
Plan: `docs/archive/plans/core-duplex-pressure-isolated-terminal-subscriptions.md` (Revision 9)

## Target repository and target_id

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Authoritative spawn path: `/Users/jasonconigliari/Projects/botster-core`
- Pipeline worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1787600672_342292`
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

Not loaded:

- [[project-pipelines-playbook]] — package/plugin paths are out of scope
- Hub, Web, TUI, Ghostty, and workspaces charters — this run stays inside `botster-core`

## Files changed

Protocol and generated package:

- `crates/botster-terminal-protocol/**`
- `crates/botster-terminal-protocol-client/**` (0.2.0)
- `packages/terminal-protocol/**` (0.2.0, fixture revision 2)

Core runtime and daemon:

- `crates/botster-core/src/runtime/control_queue.rs` (new)
- `crates/botster-core/src/runtime/worker_process.rs`
- `crates/botster-core/src/runtime/mod.rs`
- `crates/botster-core/src/engine/client_worker.rs`
- `crates/botster-core/src/engine/managed_session_runtime.rs`
- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/src/contract/session_protocol.rs`
- `crates/botster-core/src/contract/terminal_adapter.rs`
- `crates/botster-core/src/contract/terminal_subscription.rs`
- `crates/botster-core/src/lib.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs`

Docs and proof:

- `docs/architecture/terminal-adapter.md`
- `docs/architecture/terminal-protocol.md`
- `docs/reports/core-duplex-pressure-isolated-terminal-subscriptions-implement.md`
- `crates/botster-core/tests/session_protocol_test.rs`
- `crates/botster-core/tests/client_worker_engine_test.rs`
- `crates/botster-core-daemon/tests/daemon_integration_test.rs`

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
- The daemon duplex byte-oracle test injects a hand-built compact-binary frame so the daemon test crate does not take a new protocol-client dependency.

## Tests and downstream proof run

CI-owned Cargo commands, no wrapper:

- `BOTSTER_ENV=test cargo test -p botster-core --lib --no-default-features` — 13 passed
- `BOTSTER_ENV=test cargo test -p botster-core --lib --features local-runtime` — 41 passed
- `BOTSTER_ENV=test cargo test -p botster-core --test client_worker_engine_test` — 22 passed
- `BOTSTER_ENV=test cargo test -p botster-core --test session_protocol_test` — 17 passed
- `BOTSTER_ENV=test cargo test -p botster-core --test local_session_worker_process_test worker_process_runtime_crosses_os_process_boundary` — passed
- `BOTSTER_ENV=test cargo test -p botster-terminal-protocol -p botster-terminal-protocol-client` — passed
- `BOTSTER_ENV=test cargo test --manifest-path crates/botster-core-test-support/tests/consumers/hub-adapter-shaped/Cargo.toml` — 3 passed
- `script/terminal-protocol-node-smoke.sh` — passed (`@trybotster/terminal-protocol` 0.2.0)

Production entry point: `CoreDaemon::drain` now calls `engine.apply_terminal_input` before `drain_runtime_once`. Adapter `try_read` → decode → `write_bytes` / `submit_mode_gated_pty_input` / resize → worker control queue → session worker PTY.

## Unverified behavior or residual risk

- A dedicated adapter-inject byte oracle did not observe PTY echo within the worker completion bound. `worker_bound_adapter_receives_ready_finish_without_drain_snapshots` still passes through `CoreDaemon::drain` after `apply_terminal_input` was added. Review should add the inject-to-echo oracle.
- Control-pressure, idle-owner sweep, and the full seven-path gated-cancel suite from plan §12 are not all present as dedicated oracles. Cancel is wired on apply teardowns and on every `apply_client_worker_with` teardown, including detach, pump, and session teardown.
- Same-session subscriptions still share one control channel. Delay then collective hard-stop is the stated policy. Cross-session isolation is the ticket guarantee.
- `record_attach` stays infallible. Attach and bind gates live at `CoreDaemon` and worker-backed bind.

## Missing vault guidance discovered

None that blocked the work. Writer timeout on stdio pipes is now a concrete gotcha: do not treat `SO_SNDTIMEO` failure as a sealed control plane.
