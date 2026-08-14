# Implementation report: worker incremental attach POST-BARRIER-MARKER

Ticket: `ticket_1786735252_213191`
Run: `run_1786735272_847499`
Step: `botster_stack_implement`

## Target repository and target_id

- Target repository: `botster-core` (`trybotster/botster-core`)
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target path from `list_spawn_targets`: `/Users/jasonconigliari/Projects/botster-core`
- Worktree: Botster-managed ticket worktree
  `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-project-pipelines-ticket_1786735252_213191`
- Approved plan routing: same `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` at subject `159d926`
- Merge policy: `direct`. No pull request.

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

- `crates/botster-core/src/engine/botster.rs` — after `Attached`, drain leftover producer output before flushing queued `FRAME_PTY_INPUT`
- `crates/botster-core/src/runtime/worker_process.rs` — no lasting edit. A first stall-on-full attempt hung cancel and broke detached overflow; it was reverted.
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

The leftover drain is the plan's stated repair. A first attempt to stall live `PtyOutput` on the parent channel restored process echo, but a global stall broke detached overflow and an attached stall hung `worker_incremental_attach_cancel_releases_snapshot_barrier`. Those worker_process edits were reverted.

The production finish path now drains leftover live bytes after `Attached` and before `FRAME_PTY_INPUT`, so the capacity-one slot is empty when the child consumes queued input.

## Tests and downstream proof run

Pre-fix negative control on `159d926` (this session, run 5):

```
queued client output never observed "echo:POST-BARRIER-MARKER" within 180s or after 60s idle; last output: "POST-BARRIER-MARKER\r\n"
```

Plan Review's fourth isolated run is the same shape.

Focused oracle, 10 consecutive passes after the fix, all exit 0:

```bash
BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output
```

Sibling incremental attach tests, all exit 0:

- `worker_incremental_attach_blank_history_is_ready_finish_attached`
- `worker_incremental_attach_history_failure_reports_incomplete_then_attached`
- `worker_incremental_attach_cancel_releases_snapshot_barrier`
- `worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`

Revert-once negative control: production files restored to `159d926`, focused test passed 4 times, failed on run 5 with the same assertion and last output `POST-BARRIER-MARKER\r\n`. Fix restored after that red run.

Workspace gates:

```bash
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --workspace
BOTSTER_ENV=test cargo test --doc --workspace
```

All four workspace gates passed on this Implement session:

- `cargo fmt --all -- --check` — pass
- `BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings` — pass
- `BOTSTER_ENV=test cargo test --workspace` — first run failed `bounded_waiting_queue_reports_attributed_backpressure_and_neighbor_isolation` (plugin-worker, isolated rerun exit 0). Second default-concurrency workspace run exit 0, including `worker_incremental_attach_*`.
- `BOTSTER_ENV=test cargo test --doc --workspace` — pass

Production-path proof: `CoreDaemon::attach` / `input` / `resize` / `drain` call `WorkerBackedBotsterEngine::drain_runtime_once`. That function now drains leftover live bytes after `Attached` and before queued `FRAME_PTY_INPUT`.

No new Hub consumer. The authentic worker PTY + Ghostty daemon test remains the charter proof.

## Unverified behavior or residual risk

- The isolated miss is timing-dependent. Ten consecutive focused passes plus a revert-red run reduce, but do not eliminate, residual race risk if later live `PtyOutput` is dropped after leftover drain.
- A parent `try_send` drop of the process echo remains possible. A stall-on-full repair hung cancel and broke detached overflow, so it was not kept.
- `plugin_worker` load flakes remain diagnostic only. The first workspace run exposed `bounded_waiting_queue_reports_attributed_backpressure_and_neighbor_isolation`; isolated and the next workspace run passed.
- `daemon.capture_snapshot` after `Attached` uses the parent shadow. It does not re-fence the worker PTY.

## Missing vault guidance discovered

Captured to inbox after the proven diagnosis:

- `incremental attach must drain leftover producer output after Attached`

Do not recapture [[capacity one attach proofs drain pre attach producer output]] or [[incremental GHOSTSNP clients defer resize and input until FINISH and attached]].

## Runtime-teardown class

`teardown_class_applies`: no.
