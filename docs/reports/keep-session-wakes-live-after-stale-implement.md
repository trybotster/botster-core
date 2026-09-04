# Implementation report: keep session wakes live after Stale until worker shutdown completes

Ticket: `ticket_1788537020_814817`
Run: `run_1788537030_383590`
Step: `botster_stack_implement`
Plan artifact: `artifact_1788537535_742855`

## Target repository and target_id

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Independent `list_spawn_targets` resolution: admitted `botster-core` with the same `target_id`
- Approved plan routing: same repository and `target_id`, base `5ed369fc4a536d7cfa99547262561fcea7ef41e5`
- Worktree: Botster-managed ticket worktree (no colon)
- Merge policy: `direct`. No pull request.
- Runtime-teardown class: yes

## Repository playbook and other playbooks/notes applied

Role and charter:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-core-playbook]]
- [[botster runtime teardown lenses]]
- [[botster-architecture]]
- [[cli-patterns]] (index only; no TUI/CLI product change)
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[botster core worktrees need the ghostty submodule initialized]]
- [[core terminal wake test binaries must run from the repository working directory]]
- [[implement gate must verify committed work and pr link before review]]
- [[implementation artifacts must match actual git state]]
- [[implementation steps must persist report artifacts for review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[prefer framework and library components over custom solutions]]

Targeted notes:

- [[session ingress wakes retire after bound route delivery not lifecycle commit]]
- [[session wake coalescing belongs in a lifecycle registry not each handle]]
- [[a non-pump Core drain can strand bound process exit delivery]]
- [[Core bound queue ingress wakes are one shot and have no recovery rearm]]
- [[a wake test must not consume the one shot edge it asserts]]
- [[a one shot EOF wake must follow reader finished]]
- [[positive wake controls must consume runtime readiness wakes first]]
- [[a non completing shared fake adapter is a blind no progress oracle]]
- [[host ShutdownSession classification must call the exact-session Core query]]
- [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]]
- [[botster core bounded waiting queue test flakes under workspace load]]
- [[botster core contract surface needs consumer proof]]
- [[scratch cargo patch redirects measure downstream dto breakage]]
- [[downstream proof targets the consumer branch that exposed the failure]]
- [[workspace cargo test filters miss isolated downstream-shaped consumer crates]]
- [[uncompilable downstream repositories still require paired ablation proof]]
- [[Hub official gates must not set CARGO TARGET DIR]]

Not loaded as a product overlay:

- [[project-pipelines-playbook]] — no Project Pipelines package/plugin path. Workflow checklists still used.

## Files changed

- `crates/botster-core-daemon/src/daemon.rs` — `commit_terminal_lifecycle` retires a session wake only when `engine_session_is_terminal` and no owner holds undelivered frames. Registry `Stale`/`Exited` alone does not retire.
- `crates/botster-core-daemon/tests/terminal_wake_test.rs` — worker-backed `stale_registry_then_shutdown_completes_through_wait_wakes` and `stale_registry_with_live_worker_still_delivers_process_exit_through_targeted_wake`
- `docs/architecture/core-daemon.md` — registry `Stale`/`Exited` is not wake retirement while the engine session is live
- `docs/architecture/control-plane-lifecycle-journal.md` — same retirement rule on observe
- `docs/reports/keep-session-wakes-live-after-stale-implement.md` — this report

## Ownership boundaries preserved

- Core owns wake-registry lifecycle and `commit_terminal_lifecycle`.
- Hub retains `mark_session_stale` policy. No Hub source, pin, or Cargo.lock change in this repository.
- No public API, DTO, flag, or consumer crate added.
- Bound-queue wake paths from `5ed369f` (`notify_bound_queue_wakes`, `bound_owner_has_held_frames`, reader-finished ordering) were not edited.
- The two-second shutdown watchdog and `shutdown(None)` stop-at-first-error behavior were not edited.

## Cross-repo dependencies or separately routed work

- Parent Hub ticket `ticket_1787600679_990088` stays on Core `5ed369f` until this merge. This run does not pin Hub.
- No dependency ticket was required. Hub needs no code change for this Core repair.
- Downstream proof used a discarded Hub scratch worktree at `b164ca1`. That tree was not committed.

## Deviations from plan

Clippy `overly_complex_bool_expr` rejected the planned `(terminal_observation || obligation || engine_terminal) && engine_terminal` form. The shipped guard is the equivalent `engine_session_is_terminal && !session_has_undelivered_frames`. The observation/obligation OR arms were dead under the required engine-terminal AND.

Test 1 does not assert occupancy after the stale observe. That assertion would fail first under ablation and hide the production deadline message. Occupancy after stale observe remains in test 2.

No other scope change.

## Runtime-teardown lenses implemented

1. Isolation — per-session ownership is the wake-registry node, worker reader/writer threads, and bound ClientWorker owners. `forget_session` is keyed by `SessionId`. Sibling sessions are not retired.
2. Bounds — `shutdown_session` keeps the two-second deadline and returns `ShutdownFailed`. `wait_wakes_bounded` still caps nodes and ignores the host interrupt. This change removes a spurious trigger of that bound.
3. Late-message matrix — Attach/`expect_terminal_adapter` still reject terminal registry rows. `bind_waking_terminal_adapter` still closes late binds. Input/resize still reject terminal rows. `notify_session` on a retired id remains a no-op; this ticket keeps the id live until shutdown consumes EOF. A reused `SessionId` still allocates fresh wake state at spawn.
4. Production-path proof — Hub `shutdown_session` -> `CoreDaemon::shutdown(Some(id))` -> `commit_terminal_lifecycle` (no forget while engine live) -> worker EOF `notify_session` -> `wait_wakes_bounded` -> `pump_woken` -> engine `Exited` -> wake forgotten. Core test 1 and Hub `live_session_entity_subscription_emits_exact_stale_transition_patch` drive that path. Ablation restores the registry `already_terminal` arm and reproduces the deadline message.
5. Ownership identity — wake state is keyed by `SessionId` with a retired flag. The engine session is liveness identity. The registry row is a projection. Forget now requires an engine-terminal session.
6. Sibling / fail-closed — success leaves siblings untouched. Ultimate failure returns `Err` for that session only. `shutdown(None)` stop-at-first-error is pre-existing and out of scope.

No lens was deferred.

## Tests and downstream proof run

Commands ran from the ticket worktree root. Ghostty submodule was already initialized. `CARGO_TARGET_DIR` was unset for Hub proof.

- `cargo fmt --all -- --check` — pass
- `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings` — pass
- `BOTSTER_ENV=test cargo test --workspace` — 962 passed, 1 ignored
- `cargo test -p botster-core --no-default-features --lib` — 41 passed
- `BOTSTER_ENV=test cargo test --doc --workspace` — pass
- `cargo doc --workspace --no-deps` — pass (pre-existing rustdoc private-link warnings)
- `cargo test -p botster-core --test local_process_runtime_test` — 21 passed (local macOS)
- `BOTSTER_ENV=test cargo test -p botster-core-test-support --test wake_pump_consumer_test` — 1 passed
- Focused: `BOTSTER_ENV=test cargo test -p botster-core-daemon --test terminal_wake_test stale_registry` — 2/2, five consecutive green after the clippy-equivalent guard
- Ablation: restore registry `already_terminal` only. Both new tests red. Test 1: `worker session shutdown did not complete before the daemon deadline: stale-shutdown-wake-session`. Restore green.
- Hub base arm: scratch Hub `b164ca1` unpatched Core `5ed369f`, worker prebuilt, no `CARGO_TARGET_DIR`, no `--locked`. `live_session_entity_subscription_emits_exact_stale_transition_patch` failed 5/5 with `worker session shutdown did not complete before the daemon deadline: stale-transition-session`.
- Hub candidate arm: same scratch, workspace-root Core-family path patch to this worktree, worker rebuilt, no `--locked`. Same test passed 5/5.
- Hub SHA: `b164ca1a8e0c2b77ccfbc1c9ea83e158b6a3c928`. Core subject: this branch on top of `5ed369fc4a536d7cfa99547262561fcea7ef41e5`.
- Scratch Hub worktree and Cargo.lock were discarded after the proof.

## Unverified behavior or residual risk

- Linux-only CI classification of `local_process_runtime_test` is unchanged. The local macOS run passed; CI Linux was not run here.
- Worker-backed wake tests remain load-sensitive. Focused 5/5 was consecutive and green. Workspace gate was one green run, not five.
- Over-retained wake nodes remain possible if a path never reaches engine-terminal after a registry-terminal row. Occupancy tests in the existing suite still run. Explicit `remove_session` still retires.
- Hub official `./test.sh --locked` was not the required oracle. The plan named the lib test through a scratch patch.

## Missing vault guidance discovered

Captured to vault inbox (not processed into notes/):

- `registry Stale or Exited rows do not retire Core session wakes while the engine session is live`
- `the Hub stale-transition entity test plus a scratch cargo patch is the downstream oracle for Core wake retirement changes`

No missing charter blocked the change.

## Assumptions

- Engine `session()` retains `Exited` sessions until removal. `Failed` and absent engine sessions are terminal for wake retirement.
- Hub forget can happen on maintenance observe or on shutdown's own commit. Test 1 drives observe then shutdown. Test 2 drives stale observe with a live worker then targeted `ProcessExited`.
