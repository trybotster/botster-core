# P7 services handoff

Updated: 2026-10-04, under the lead's amended PAUSE (user order, token budget), by the Opus implementer.
**Nothing is unpushed. No job runs.** The worktree's tracked tree is clean, and its local `stage1/p7-services` equals origin (`6e8d5b3`).

## Branches and exact pushed heads

| Branch | Pushed head | What it is |
|---|---|---|
| `stage1/p7-services` (PR #142) | `6e8d5b3445352d2653d0fc1469c31fddb899161d` | Stacked PR 1, the Guardian machine. CLEAN at this head. Lands AS REVIEWED. |
| `stage1/p7-services-wire` (no PR) | `39bdc39d96dbd10aa0f14986171d905c7667e763` | `6e8d5b3` + 2 PR 1 wire fixes. NOT tested, NOT reviewed. The FIRST follow-up PR after the pause. |
| PR 2a | none | PAUSED. No branch and no code; design draft only (below). |

- Repository: `trybotster/botster-core`. Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-p7-services`.
- Base of #142: `origin/v1` `144b0234fb632bcbb5176b17c2fe55f3239405df`.

## People

- Package reviewer: `sess-1791169236-010d-73cc61588aa2c82d9260b35b4a88629f` (Sol). The earlier `…0107` is retired.
- Integration reviewer (cross-package PRs: PR 2a and later): `sess-1791168757-0109-73d2ca212653045545e7480ab60be9a9`.
- Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`. Report only QUESTION, BLOCKED, or READY/CLEAN/DONE.

## PR #142: state and the wind-down step

- VERDICT CLEAN on `6e8d5b3`: commit `ac6e612aadbd50f61b8757631f62399ea3bb06d8` on `stage1/review-p7`, `verdicts/p7-services.md`, round 5. F1 to F6 CLOSED. No open finding.
- Mutation on the identical tree (`ddc7206`): 129 tested, 116 caught, 13 unviable (type errors), 0 missed, 0 timeouts. Per file (caught/unviable): guardian 66/8, link 27/2, log 17/0, wire 6/3. Log: `~/botster-sessions/gates/botster-core-stage1-p7-services-ddc72062-linux-20261004-204542-46552.log`; unviable reasons: `~/botster-sessions/gates/botster-core-stage1-p7-services-ddc72062-mutants-unviable-reasons.txt`.
- Mac gate on `6e8d5b3`: RED in `slow` only, from two base-v1 tests owned by P5 (`process_guard::parent_dies_before_fifo_reader`, `slow_real_core a_worker_is_not_left_when_the_cleanup_of_a_test_fails`). Log: `~/botster-sessions/gates/botster-core-stage1-p7-services-6e8d5b34-mac-20261004-212801-2865.log`. The lead: no rerun.
- **Merge order: #142 is FOURTH.** Exact next step, when the lead calls #142's turn (after #164 merges):
  1. Restore a spawner `.gitignore` edit if present (`git restore .gitignore`).
  2. `git merge origin/v1` into `stage1/p7-services` (at `6e8d5b3`). Merge ONLY `origin/v1`. Do NOT fold in `stage1/p7-services-wire`.
  3. Push (fast-forward), and send the reviewer the new head for a delta review of the merge only.
  4. On delta CLEAN: run the full gate ONCE on that exact head (`~/botster-sessions/shared/tools/botster-gate --on <machine the lead allows> <worktree>`, normal priority).
  5. Green: send the lead READY with the head, the verdict commit, and the gate log. Then send the lead "PAUSED <heads>" and stop.
  6. Red, and it needs a fix round: stop, update this handoff, and hand off (lead's order).

## First follow-up PR after the pause: `stage1/p7-services-wire`

Two PR 1 wire defects, found while designing PR 2. Open a PR from `stage1/p7-services-wire` against `v1` after #142 merges (rebase or merge `v1` first), then test, mutation-test, and review it.

- `54819a8`: the guardian link gets its own frame types: `COMMAND_FRAME` 0x20, `REPORT_FRAME` 0x21, `LOG_FRAME` 0x22. Guardian reports used `WORKER_MSG`. Their JSON tags overlap `WorkerMsg` (`exited`), which ignores unknown fields, so a guardian `{"t":"exited",...}` report would decode on the host as a worker `Exited` with empty fields. With the fix, the host driver dispatches on the frame type alone.
- `39bdc39`: `SpawnResult` is `Started` or `ExecFailed{errno}`, and the report is `ExecFailed{errno}`. A2-1: `BoundUnavailable` is a sync host refusal against `features()`; the only async start failures are `ExecFailed{errno}`, `GuardianFailed` (host-observed), and `RegistryFailed`.
- Only `cargo fmt --check` was run on them. The reviewer knows of them (FYI) and has not reviewed them.
- Verification: `cargo fmt`, `cargo clippy -p botster-guardian-core -p xtask --all-targets -- -D warnings`, `cargo test -p botster-guardian-core`, and `cargo mutants --in-diff` over `guardian.rs`, `link.rs`, `log.rs`, `wire.rs` (`--jobs 1 --no-shuffle --test-tool nextest --timeout-multiplier 5`), printing the four result lists and the per-file counts into the log (see the `ddc72062` log header for the exact command).
- PR 2a depends on these frame types; land this PR before PR 2a.

## PR 2a and PR 2b (stacked; PAUSED, not started)

- Design draft: `~/botster-sessions/shared/core-stage1/p7-pr2-host-services-design-draft.md`. It splits PR 2 into 2a (service lifetime: spawn, stop, remove, guardian link, events, `service_report`, `service_log_tail`, secret, registry row) and 2b (lanes, preamble, epochs, queues, send/recv, readiness, `EndEpoch`). Both touch shared host code, so both need the integration reviewer.
- Start PR 2a on a `v1` that contains P3's #163 (it changes `admit.rs`, `flows.rs`, `run.rs`) and the wire follow-up.
- The host mirrors the guardian log: a ring of `service_log_bytes`; a chunk whose offset is not the ring's end clears the ring first. Adoption completes only on the guardian's `Status`, which follows the whole ring.
- **A16 `ServiceEnd` duty.** Core Amendment 16 is FINAL (final34, contracts `7778b1e` = `contracts-v0.1.16`). `StopService` completes with `ServiceEnd = Exited(ServiceExit) | Lost{reason, payload_may_remain}`:
  - `Lost{GuardianLost}`: no signal, no `ServiceState` event; `Completed` is posted with the state's own `Lost` payload.
  - `Lost{EpochExhausted}`: SV-9 runs through the live guardian; `ServiceState{Exited}` is posted; the result is `Exited`. This is the only transition out of `Lost`.
  - A recorded exit is final. A guardian loss in flight before any exit gives `Lost{GuardianLost, …}`.
  - Seven `conf::a16_1_*` ids belong to P7. They stay PENDING until both harnesses pass.
  - Do not move the pin. P6's PR #161 moves the Core pin to `contracts-v0.1.16`. Build PR 2a to the A16 text, and switch to the crate's `ServiceEnd` once #161 is in `v1`.
- PR 1's guardian needs no change for A16.

## Prior art: service supervision (for PR 3 and its Prior-art note)

Old botster-core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`, read only through `git show`, path `crates/botster-core/src/runtime/plugin_process/`:

- **`launch.rs` (197 lines): REUSE THE MECHANISM, NOT THE CODE.** It applies the descriptor shuffle (`dup_above` with `F_DUPFD` above a floor of 10, then `dup2` into place), `setrlimit` (CPU, NOFILE, CORE, AS) and the fatal pipe inside an `unsafe` `CommandExt::pre_exec` closure with raw `libc`. Every crate of this workspace has `unsafe_code = "forbid"`, so that code cannot be stolen as it is (a `Stolen-From` commit must pass "unchanged or with trivial edits").
  - Keep the ideas: a fatal pipe (CLOEXEC write end; EOF means exec succeeded, an errno record means it failed: this is PR 1's `ExecFailed{errno}`); a non-blocking cause pipe; `SIGPIPE` off on the control socket; each rlimit applied before exec.
  - Safe replacement (recommended): a two-stage launch through the same binary. The guardian spawns `botster-worker` in a third role, `launch`, with only the listed descriptors mapped. That process runs our own code, so it applies everything to itself with safe rustix calls: `setrlimit`, `set_parent_process_death_signal(SIGKILL)` (Linux, SV-8; it survives `exec`), descriptor placement and closing, `chdir`. Then it calls `CommandExt::exec` into the service, which keeps the same pid. The errno of any step goes back on the fatal pipe. No `unsafe` and no `pre_exec` in our code. Cost: one extra `exec` per service start. Plan 2.2 fixes one binary; a third role keeps that.
  - Alternative: `command-fds` (descriptor mapping; its `pre_exec` is inside the library). It does not cover rlimits or `PDEATHSIG`, so a launcher step is still needed. Record the choice and the rejected option in PR 3's Prior-art note.
- **`supervisor.rs` (271 lines): REJECT the deadline owner, REUSE the group-killer ideas.**
  - The deadline owner is a thread with `Mutex`, `Condvar` and `Instant::now`. The sans-IO guardian replaces it: deadlines are machine state, and the driver supplies time (plan 2.1).
  - The group killer keeps the leader unreaped (`waitid` with `WNOWAIT`) so the group id cannot be reused, checks whether the leader already started to exit (`/proc/<pid>/stat` `PF_EXITING` on Linux, a zombie check), then `killpg(SIGKILL)`. PR 1's `leader_exiting` inputs, `KillTree`, and `ReapService` ordering encode this. PR 3's real edge implements it with rustix (`waitid` `NOWAIT`, `kill_process_group`), as `botster-core-sys/src/payload.rs` already does for workers.
- **Merged v1 code to reuse:** `botster-core-sys/src/process.rs` (`process_group(0)`, identity by pid and start time, `kill_process_group`) and `payload.rs` (unreaped leader through `waitid` `WNOWAIT`, an exit-watch thread, no `unsafe`).

Ecosystem (BUILD.md rule 0: prefer maintained libraries; record each choice):

- **std** `CommandExt::process_group` (safe; P1 uses it) and `CommandExt::exec`.
- **rustix** (already adopted): `setrlimit`, `prlimit`, `set_parent_process_death_signal`, `set_child_subreaper`, `kill_process_group`, `waitid`. All safe.
- **`process-wrap`** (successor of `command-group`): process-group spawn and tree kill. It does not keep the leader unreaped before the kill, so it does not meet SV-9's ordering by itself. Consult it; do not adopt it.
- **s6, runit, daemontools:** one supervisor process per service that outlives its client. This is the guardian topology (section 10, non-normative). s6's notification descriptor is prior art for the cooperative cause descriptor (SV-5).
- **systemd** cgroups (`KillMode=control-group`): exact tree containment. SV-4 makes cgroups post-v1; SV-9 accepts `descendants_may_remain`.
- **tini, dumb-init:** child reaping and signal forwarding. Linux `PR_SET_CHILD_SUBREAPER` (rustix `set_child_subreaper`) on the guardian makes orphaned descendants re-parent to the guardian. Enumeration then also finds descendants that left the group, which narrows SV-9's escaped-descendant case. macOS has no subreaper, so `descendants_may_remain` stays the honest report there. Evaluate it for PR 3; it is a mechanism choice, not a contract change.

## Rules in force

- One heavy job at a time, only on the machine the lead allows; normal priority (never `--priority`). Heavy work waits during HOLD or PAUSE, except the lead-approved #142 gate.
- Never change a timeout value or a transcript; never invent an expected value. Never force-push; never `git branch -D`; keep git prompt-free.
- Check origin for a new branch name before pushing to it.
- PR 3 touches `botster-worker`: coordinate with P3 first. Service controls register through P6's per-module REGISTRY.
- Real-process tests own and kill their group on Drop and panic, never reap what production reaps, use non-spinning children that exit with their parent, and one test covers a broken cleanup path.
- Equivalent mutants: one function or constant per `exclude_re` entry, argued in writing; tuning constants need the four conditions of the lead's ruling. No test exists only to kill a mutant.
