# Plan: worker incremental attach must emit POST-BARRIER-MARKER after READY/FINISH

Ticket: `ticket_1786735252_213191`
Target: `botster-core` / `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery`
Subject revision: `159d926`

This file is the reviewable Plan artifact. Living attach contracts remain in
`docs/architecture/` and the worker-backed daemon integration tests. See
`docs/README.md`.

**Revision 2** answers Plan Review `review_1786736501_727097`. It removes the
pre-Attached input hypothesis and adds an exact repeat gate.

## Target

- Target repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Spawn-target name: `botster-core`
- Spawn-target path is the admitted `botster-core` target from
  `list_spawn_targets`, not the ambient pipeline session directory.
- Subject revision at Plan: `159d926` on `origin/main` and on this worktree
  branch `project-pipelines/ticket_1786735252_213191`
- Repository playbook: [[botster-core-playbook]]
- Hub session-type eligibility parent: does not apply
- Project Pipelines package/plugin paths: out of scope
- Runtime-teardown class: does not apply

Resolved from `list_spawn_targets` via ticket `target_id`. Not inferred from
the process working directory.

## Playbooks and notes loaded

Role and overlay:

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-core-playbook]]
- [[botster-architecture]]
- [[cli-patterns]] (mixed-generation index only; ownership from the charter)
- [[spa-patterns]] (loaded per planner overlay; no SPA surface in this ticket)
- [[botster-runtime-reviewer-playbook]]
- [[botster-runtime-verifier-playbook]]
- [[project pipeline orchestration belongs in a device-level botster plugin]]
- [[project pipelines needs an operator workbench not more primitives]]
- [[project pipelines ui contract belongs in the plugin readme]]
- [[botster orchestration should spawn agents with explicit target ids]]
- [[botster orchestration prompts must bind agents to explicit worktrees]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[plan steps need reviewable plan artifacts]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[colon worktree paths break cargo dyld library paths]]

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
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[plugin worker unload deadline can flake under default-concurrency workspace load]]
- [[botster core contract surface needs consumer proof]]
- [[verification reports name the load bearing oracle when cheaper suites are blind]]

Not loaded, with reason:

- [[botster runtime teardown lenses]] — this ticket is an attach-stream live
  output repair after READY/FINISH. It is not WebRTC/peer lifecycle,
  SessionIo/ClientWorker teardown, multi-peer ownership, CPU/battery/FD spin,
  or terminal-state versus live-runtime divergence.
- [[project-pipelines-playbook]] — no Project Pipelines package/plugin path
  or workflow-policy change.
- Other repository charters — ticket `target_id` maps only to `botster-core`.

## Context loaded

- Ticket `ticket_1786735252_213191` and run `run_1786735272_847499`.
- Parent consumer `ticket_1786733177_803101` Plan Review
  `review_1786735088_401140` / `finding_1786735088_288609`:
  `BOTSTER_ENV=test cargo test --workspace` is already red on `159d926`.
  The first root is
  `worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`.
  An isolated outside-sandbox rerun misses `echo:POST-BARRIER-MARKER` process
  output. This is not the documented plugin-worker unload flake.
- Admitted spawn target `tgt_1f7bce66eb304881980f9b4a2a5ae3fe` is
  `botster-core` at the hub-registered path. Worktree HEAD matches that
  revision.
- Repo placement: `docs/plans/` is a retired stub. Reviewable plans live
  under `docs/archive/plans/`.
- Core gates are the README/CI Cargo commands in
  [[botster-core uses CI-owned Cargo commands because it has no test script]].
- `.gitignore` is present and matches HEAD (63 bytes). Worktree path has no
  colon, so `CARGO_TARGET_DIR` override is not required.
- Plan Review `review_1786736501_727097` independently reran the focused
  test on `origin/main` `159d926`: three passes, then a fourth-run failure.
  The failing run showed terminal echo of the typed bytes and no process
  output `echo:POST-BARRIER-MARKER`.
- Findings `finding_1786736501_987287` and `finding_1786736501_236008`
  require input to stay queued until Attached and require an exact repeat
  gate.

## Scope

Repair the owning worker incremental attach-stream so queued owner input
becomes live `TerminalOutput` after READY / PAGE* / FINISH / Attached.

Required work:

1. Reproduce the isolated failure on clean Core `159d926` with
   `BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`.
   Record the exact assertion and last observed output.
2. Diagnose why `echo:POST-BARRIER-MARKER` is missing on the production
   attach-stream after READY/FINISH. The load-bearing oracle is process
   output (`echo:…`), not terminal echo of the typed bytes.
3. Repair only the attach-stream path that owns that cut. Current owner
   is `WorkerBackedBotsterEngine::drain_runtime_once` after FINISH, plus the
   worker PTY barrier complete/input path it drives:
   - `crates/botster-core/src/engine/botster.rs`
   - `crates/botster-core/src/runtime/worker_process.rs`
   - `crates/botster-core-daemon/src/bin/botster-session-worker.rs`
4. Keep the existing authentic Ghostty worker test as the proof. Do not
   replace it with a fixture-only snapshot-before-live test.
5. Keep the existing producer order:
   FINISH → latest queued resize → PTY barrier release → Attached →
   `FRAME_PTY_INPUT` for queued owner input → live process output.
   [[incremental GHOSTSNP clients defer resize and input until FINISH and attached]]
   forbids `FRAME_PTY_INPUT` before the matching Attached state.

## Non-scope

- `lifecycle_baseline_page`, `observe_lifecycle_slice`, journal consume, and
  Hub session projection.
- Hub, Web, TUI, or protocol crate changes.
- New public Core APIs, DTOs, or compatibility features.
- Plugin-worker unload flake work or a workspace-gate waiver.
- A replacement test wrapper or a rewrite of
  `drain_pre_attach_producer_output` as the primary fix.
- Sending `FRAME_PTY_INPUT` before Attached, including any still-fenced
  pre-release owner-input path.
- Pull request creation or human PR sign-off. Delivery is direct-merge to
  `main`.

If diagnosis proves the production attach-stream already delivers the process
echo and only the test oracle is wrong, stop and ask a human. Do not silently
weaken the test.

## Repository ownership and cross-repo dependencies

Core owns the incremental attach phase machine, the worker PTY fence, queued
input/resize during attach, and route-owned Snapshot / AttachState /
TerminalOutput frames.

Hub forwards opaque events and must not grow a second READY/FINISH/Attached
state machine. This ticket does not change Hub.

Ghostty remains the concrete encoder/decoder inside
`botster-terminal-ghostty`. Do not change Ghostty integration unless the
owning attach-stream cannot emit the marker without a proven adapter bug.

Cross-repo prerequisites: none. Do not broaden this run to Hub.

Same-repo consumer already registered:

- `ticket_1786733177_803101` depends on this ticket and must rerun all four
  Core gates after this merge.
- Hub `ticket_1786663582_169720` depends on that consumer, not on this
  repair directly. Do not implement Hub projection here.

## Botster layers touched

- Session/client worker attach-stream and worker PTY barrier.
- `botster-core-daemon` authentic Ghostty integration test.
- No plugin, SPA, MCP, Rails, or Project Pipelines package layer.

## Current production path

`CoreDaemon::attach` → `WorkerBackedBotsterEngine::attach_client` starts one
worker snapshot boundary and returns `Attaching`.

`CoreDaemon::input` / `resize` during that window queue on `IncrementalAttach`.

Each later `CoreDaemon::drain` calls
`WorkerBackedBotsterEngine::drain_runtime_once`, which polls
`poll_snapshot_boundary` and streams one client-paced Snapshot phase.

After FINISH the same function now:

1. Applies the latest `queued_resize` through `handle_client_ingress`.
2. Blocks in `complete_snapshot_boundary` until the worker sends
   `barrier_released`.
3. Emits `Attached` through `complete_snapshot_attach`.
4. Flushes `queued_input` through `handle_client_ingress`.
5. Takes one live `drain_runtime_once`.

The worker holds `with_pty_io_barrier` from GET_SNAPSHOT begin through
`wait_for_release`. Control-plane `FRAME_RESIZE` during that window is staged
and applied under the fence. `FRAME_PTY_INPUT` is not applied until the
parent flushes queued input after `Attached`.

Commit `a047574` only drained pre-attach producer output in the test. It did
not change this finish/flush path. Later `159d926` observe/baseline commits
did not touch attach, input, or drain production code.

The failing test already waits for READY, PAGE*, FINISH, and Attached, then
asserts process output `echo:POST-BARRIER-MARKER` as `TerminalOutput` after
`Attached`. Plan Review's fourth isolated run showed terminal echo without
that process prefix. The attach-stream therefore writes bytes into the PTY
after Attached, but the child does not consume them and emit process output.

## Implementation sequence

1. Capture one raw pre-fix failure of the focused test on `159d926`. Keep
   the assertion text and last observed output. Plan Review already has one
   such failure: terminal echo present, `echo:POST-BARRIER-MARKER` absent.
2. Trace a failing run after Attached only:
   - whether `queued_input` is flushed as `FRAME_PTY_INPUT` after Attached
   - whether capacity-one worker egress still blocks the child when that
     flush occurs
   - whether the child `read` loop still exists after the queued resize
   - whether any process echo is dropped by `suppress_attach_terminal_output`
     or never leaves worker egress as `TerminalOutput`
3. Make the smallest attach-stream change that restores process echo after
   READY/FINISH/Attached. Keep this order:
   FINISH → latest queued resize → barrier release → Attached →
   `FRAME_PTY_INPUT` → live process output.
   Do not send owner input on the still-fenced PTY. Do not send
   `FRAME_PTY_INPUT` before Attached.
   Allowed repair direction: after Attached, unblock capacity-one producer
   output so the child can consume the later `FRAME_PTY_INPUT`, then deliver
   `echo:POST-BARRIER-MARKER` as live `TerminalOutput`.
4. Do not change lifecycle observe/baseline code to make this test green.
5. Keep the existing test as the load-bearing oracle. Add a narrower unit
   only if it locks a specific post-Attached flush bug without replacing the
   live worker proof.

## Assumptions and unknowns

Assumptions:

- The ticket has one meaning: repair Core attach-stream so the existing
  capacity-one incremental attach test sees `echo:POST-BARRIER-MARKER` after
  READY/FINISH/Attached.
- Isolated missing process output is a product defect, not the plugin-worker
  flake. Plan Review showed it is intermittent: three focused passes, then
  one fail.
- Repair base is `159d926`.
- No new public contract is required.
- `FRAME_PTY_INPUT` stays after Attached. That is not an Implement choice.

Unknowns Implement must close before editing:

- Exact isolated assertion (`drain_until_for_client` idle/completion versus
  `expect("queued input output")`) and last observed renderable bytes.
- Whether the child still runs after Attached.
- Why terminal echo can appear without process `echo:POST-BARRIER-MARKER`
  after the post-Attached flush.
- Whether the post-Attached `capture_snapshot` in the test re-fences the PTY
  before the echo can drain. If that helper is the only failure, ask a human
  before changing the test.

## Affected surfaces and files

Likely edit:

- `crates/botster-core/src/engine/botster.rs`
- `crates/botster-core/src/runtime/worker_process.rs`
- `crates/botster-core-daemon/src/bin/botster-session-worker.rs`

Proof that must stay green, and may gain only a focused comment or
assertion if diagnosis requires it:

- `crates/botster-core-daemon/tests/daemon_integration_test.rs`
  (`worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`
  and sibling incremental attach tests)

Plan artifact only:

- `docs/archive/plans/worker-incremental-attach-post-barrier-marker.md`

Do not edit:

- observe/baseline APIs
- Hub
- `docs/plans/` stub

## Risks

- Fixing only the test drain helper would hide a real attach-stream miss.
  Ticket forbids that as the primary change.
- Changing resize/input order can violate
  [[worker applies the latest attach resize before barrier release]] or
  [[incremental GHOSTSNP clients defer resize and input until FINISH and attached]].
  Sending `FRAME_PTY_INPUT` before Attached is out of scope.
- Capacity-one egress can block the child so terminal echo looks like
  success. The oracle must remain `echo:POST-BARRIER-MARKER`.
- A single focused pass is not proof. The defect is intermittent on
  `159d926`.
- A second snapshot in the test (`capture_snapshot` after Attached) can
  re-enter the worker fence. Do not treat that helper as Hub projection.
- Workspace green is required. The documented plugin-worker flake is
  diagnostic only and is not a waiver.

## Acceptance checks and tests

Load-bearing oracle:

```bash
BOTSTER_ENV=test cargo test -p botster-core-daemon --test daemon_integration_test -- worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output
```

Repeat this focused command 10 times in one Implement session. Require
exit 0 on every run. Zero failures. One pass is not enough.

The test must observe `echo:POST-BARRIER-MARKER` as `TerminalOutput` after
Attached. READY viewport must still contain `PRE-BARRIER-MARKER`. Latest
queued resize `40x120` must still apply before Attached. Terminal echo of
`POST-BARRIER-MARKER` without the `echo:` prefix is not success.

Keep one raw pre-fix failure as the negative control. Plan Review's fourth
isolated run on `159d926` is that control unless Implement captures a
clearer one.

Sibling incremental attach tests in the same file must stay green,
including blank history and post-READY history-failure paths.

Charter workspace gates, with no replacement wrapper:

```bash
BOTSTER_ENV=test cargo test --workspace
cargo fmt --all -- --check
BOTSTER_ENV=test cargo clippy --workspace --all-targets -- -D warnings
BOTSTER_ENV=test cargo test --doc --workspace
```

[[plugin worker unload deadline can flake under default-concurrency workspace load]]
does not waive a red workspace run.

Downstream proof:

- This ticket does not add public contract surface, so it does not require a
  new Hub consumer. The existing authentic worker PTY + Ghostty test is the
  charter proof for worker-backed attach.
- After merge, `ticket_1786733177_803101` must rerun the four Core gates on
  the new HEAD. That rerun is not this Implementer's Hub work.

Negative control: keep the raw pre-fix failure. After the fix, revert the
production change once and show the focused test red again.

Production-path proof: `CoreDaemon::attach` / `input` / `resize` / `drain`
must use the repaired finish/flush path. Code existence in a helper is not
enough.

## Vault gaps

Worth capturing after a confirmed diagnosis:

- Isolated capacity-one incremental attach can miss post-FINISH process echo
  even after `a047574` pre-attach drain. That remaining cut is not yet a
  vault gotcha.
- If SIGWINCH or a second `capture_snapshot` fence is the mechanism, capture
  that ordering rule. Do not capture a guess.

No capture during Plan. Implement should capture only after the mechanism is
proven.

## Worktree and pipeline hygiene

- Restore `.gitignore` from HEAD if a later step wipes it. Never truncate.
- Worktree path has no colon.
- Bind Implement to this ticket `target_id` and this worktree. Do not edit
  the admitted main checkout by path inference.
- Direct-merge to `main`. Do not open a pull request.

## Runtime-teardown class

`teardown_class_applies`: no.

This ticket does not change WebRTC/peer lifecycle, SessionIo/ClientWorker
teardown, multi-peer ownership, CPU/battery/FD spin, or terminal-state
versus live-runtime discovery. Do not load or answer
[[botster runtime teardown lenses]].
