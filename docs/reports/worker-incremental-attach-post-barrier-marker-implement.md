# Implementation report: worker incremental attach POST-BARRIER-MARKER

Ticket: `ticket_1786735252_213191`
Run: `run_1786735272_847499`
Step: `botster_stack_implement`

## Target repository and target_id

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target path from `list_spawn_targets`: admitted `botster-core` target
- Worktree: Botster-managed ticket worktree
- Approved plan routing: same `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` at subject `159d926`
- Merge policy: `direct`. No pull request.
- Committed-artifact PII scan on this report and the ticket plan: no home or session paths.

## Repository playbook and other playbooks/notes applied

Role and charter:

- [[implementer-playbook]]
- [[botster-implementer-playbook]]
- [[botster-core-playbook]]
- [[botster-architecture]]
- [[cli-patterns]] (mixed-generation index only)
- [[spa-patterns]] (no SPA surface)
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[implement gate must verify committed work and pr link before review]]
- [[pipeline vault checklists must cite exact resolvable note titles]]

Targeted attach and proof notes:

- [[worker backed attach snapshots fence PTY output at the worker]]
- [[incremental GHOSTSNP attach streams READY history pages and FINISH]]
- [[worker applies the latest attach resize before barrier release]]
- [[capacity one attach proofs drain pre attach producer output]]
- [[incremental GHOSTSNP clients defer resize and input until FINISH and attached]]
- [[Core owns the incremental attach phase machine]]
- [[incremental attach snapshot frames require lossless streaming backpressure]]
- [[snapshot failure enters the protected FIFO before barrier release]]
- [[post READY history failure omits FINISH and still attaches]]
- [[botster terminal attach owns one size snapshot and live output transaction]]
- [[session registry size follows the worker applied resize]]
- [[plugin worker unload deadline can flake under default-concurrency workspace load]]
- [[botster core contract surface needs consumer proof]]
- [[verification reports name the load bearing oracle when cheaper suites are blind]]

Not loaded:

- [[botster runtime teardown lenses]] — not teardown class
- [[project-pipelines-playbook]] — no Project Pipelines package/plugin path

## Files changed

- `crates/botster-core/src/engine/botster.rs` — leftover drain after `Attached`; named consumer set synced from live Attached ownership; initial `begin_snapshot_boundary` failure removes the recorded subscription before `sync_worker_consumers`
- `crates/botster-core/src/runtime/worker_process.rs` — stall live `PtyOutput` while a named or direct consumer is present
- `crates/botster-core/src/engine/botster/takeover_fail_closed_tests.rs` — injected initial begin-failure rollback: empty inventory, ping without parent drain, typed detached overflow
- `crates/botster-core/tests/local_session_worker_process_test.rs` — capacity-one process-echo and ownership-transition pressure tests
- `docs/reports/worker-incremental-attach-post-barrier-marker-implement.md` — this report
- `docs/archive/plans/worker-incremental-attach-post-barrier-marker.md` — approved plan already in the worktree (Plan artifact)

## Ownership boundaries preserved

- Core owns incremental attach phases, worker PTY fence, queued input/resize, and route-owned Snapshot / AttachState / TerminalOutput
- No Hub, Web, TUI, or Ghostty charter edits
- No new public Core API, DTO, or compatibility feature
- Worker Ghostty remains the encoder/decoder inside `botster-terminal-ghostty`

## Cross-repo dependencies or separately routed work

- None implemented here
- Same-repo consumer `ticket_1786733177_803101` already depends on this ticket and must rerun the four Core gates after merge
- Hub `ticket_1786663582_169720` depends on that consumer, not on this repair

## Deviations from plan

None on sequence. The approved order remains:

FINISH → latest queued resize → barrier release → Attached → leftover drain → `FRAME_PTY_INPUT` → live process output

`FRAME_PTY_INPUT` stays after `Attached`.

The leftover drain remains. Review `review_1786739081_180992` reproduced the miss after leftover drain alone: iteration 9 of the focused oracle lost `echo:POST-BARRIER-MARKER`.

This revision stalls live `PtyOutput` only for subscriptions that have reached `Attached`. In-progress incremental owners are excluded so READY-then-cancel still progresses. The set is rebuilt from live inventory after attach, detach, takeover, promotion, and generation detach. A scalar increment/decrement is not used.

Initial attach records the subscription inventory row in `begin_snapshot_attach` before `begin_snapshot_boundary`. If that begin fails, the recorded row is detached before `sync_worker_consumers`. Without that rollback the failed pre-boundary row is treated as an Attached owner and stall activates with no drainer.

## Tests and downstream proof run

Pre-fix negative control on `159d926` (this session, run 5):

```
queued client output never observed "echo:POST-BARRIER-MARKER" within 180s or after 60s idle; last output: "POST-BARRIER-MARKER\r\n"
```

Plan Review's fourth isolated run is the same shape.

Review `review_1786739081_180992` failed leftover-only on focused run 9. After the attached-consumer stall:

Focused oracle, 10 consecutive passes, all exit 0:

```bash
BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output
```

Sibling incremental attach tests, all exit 0:

- `worker_incremental_attach_blank_history_is_ready_finish_attached`
- `worker_incremental_attach_history_failure_reports_incomplete_then_attached`
- `worker_incremental_attach_cancel_releases_snapshot_barrier`
- `worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`

Pressure-test ablation: `attached_capacity_one_retains_process_echo_after_terminal_echo` failed after the stall loop was removed. Last output was `FILL-SLOT\r\necho:FILL-SLOT\r\n` with `echo:POST-BARRIER-MARKER` absent. Stall restored after that red run.

Pre-fix focused miss on `159d926` (Implement run 5 and Review run 9) remains the live-oracle negative control.

Workspace gates:

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
```

Ownership-balance workspace gates:

- `cargo fmt --all -- --check` — pass
- `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings` — pass
- `BOTSTER_ENV=test cargo test --workspace` — exit 0, including `worker_incremental_attach_cancel_releases_snapshot_barrier` and `attached_capacity_one_retains_process_echo_after_terminal_echo`
- `BOTSTER_ENV=test cargo test --doc --workspace` — pass

Initial-boundary rollback:

- `BOTSTER_ENV=test cargo test -p botster-core --lib -- initial_begin_failure_restores_detached_overflow` — pass
- Ablation: removing only the `detach_live_subscription` rollback left inventory `[TerminalSubscriptionRecord { client_id: initial-begin-fail-client, ... }]` and the test failed. Rollback restored after that red run.

Production-path proof: `CoreDaemon::attach` / `input` / `resize` / `drain` call `WorkerBackedBotsterEngine`. After `Attached`, detach, takeover, promotion, generation detach, and failed initial begin, `sync_worker_consumers` rebuilds the named consumer set from live subscription inventory. In-progress incremental owners and pending replacements are excluded until they reach `Attached`. `replace_named_consumers` then installs that set. Live `PtyOutput` retries only while a named or direct consumer remains. There is no scalar consumer count.

No new Hub consumer. The authentic worker PTY + Ghostty daemon test remains the charter proof.

## Unverified behavior or residual risk

- `plugin_worker` load flakes remain diagnostic only.
- `daemon.capture_snapshot` after `Attached` uses the parent shadow. It does not re-fence the worker PTY.

## Missing vault guidance discovered

Captured after the ownership-balance finding:

- `attached stall follows live subscription ownership`

Do not recapture [[capacity one attach proofs drain pre attach producer output]] or [[incremental GHOSTSNP clients defer resize and input until FINISH and attached]].

## Runtime-teardown class

`teardown_class_applies`: no.
