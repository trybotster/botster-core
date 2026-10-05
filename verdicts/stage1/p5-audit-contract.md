# PR #164 — P5 audit fixes in the host: integration review

## Round 1 — Full delta

Reviewed head: `c4afe5861abe96904ad80f9507b147c2da8ea98e`, branch `stage1/p5-audit-contract`.
Base and merge base: current v1 `144b0234fb632bcbb5176b17c2fe55f3239405df`. The PR does not undo a merged PR.
Reviewed delta: `144b023...c4afe58` (`4a59761`, `c4afe58`), 23 files.
Audit source: `audits/v1-pr134-141.md` at `02acc4b`, findings A1, A2, A4, A5, A7 and A9.
Package verdict: none seen yet on this head (P5 reviewer `sess-1791169805-0111`). CLEAN needs that verdict on the same head.
This reviewer ran no build, test or gate.

Scope of this review: the interfaces between the packages. That is `HostEdges::identity_state`, `HostDriver::open`, and
`HostEngine::new(cfg, registry_ids)`, with their implementations in the facade (`RealEdges`) and the testkit (`SimEdges`,
`Spawner`, `WorkerSpawner`, `RunInputs`). It also covers the TM-1 lint, test quality, and the merge with the open PR #163.
The host logic of each finding is the P5 reviewer's scope.

### Accepted parts

- **One read path for the registry ids.** `HostDriver::open` reads the row keys through `HostEdges::read_rows`. The real Core
  and the testkit run the same code (A5-1), and the engine stays sans-IO. `open` fails `RegistryFailed` when the read fails.
  `Core::open` already lists that error.
- **TM-1.** The driver now feeds edge results through `HostEngine::input` with no new time. Only `pump` calls
  `Machine::handle(now, Clock)`. `epoch` and `last_now` are deleted, and so is `RunInputs.start` in the testkit, so the testkit
  no longer injects a start clock that real Core lacks. The facade's `clippy.toml` now bans `Instant::now` and
  `SystemTime::now`, so a clock read in the facade fails CI. This is a tool-enforced proof, not a hand-written test.
- **The real `identity_state`** forwards to `botster_core_sys::process::identity_state` (pid and start time, AD-6). It is a
  read, it runs in the driver, and the engine receives it as `Input::IdentityState`.
- **Test quality.** The registry tests damage only bytes that Core's encoder wrote: a cut row, a later `version`, and a row
  under another key (A10-2: no hand-written registry format). Each expected value comes from a clause or from the input: the
  set of `SessionState` ids equals the set of row keys (LC-11), `RegistryCorrupt` comes from A10-2, and `outcome_unknown` from
  A6-3 item 4. The test that pinned the old skip is deleted. A7's test waits for one wake and an EOF, each with a marked
  deadline, and has no poll interval.
- **Merge with PR #163.** `git merge-tree` of `c4afe58` with `ccba050` merges with no conflict (tree `9059e5f`). In the merged
  tree, no caller of `HostDriver::new`, `HostEngine::new(cfg, epoch)`, `last_now` or `RunInputs.start` is left.

### Findings

#### K1 [MEDIUM] OPEN — The testkit's `identity_state` says `Absent` for a worker that still runs in the `Sim`

- Location: `crates/botster-core-testkit/src/worker.rs` `Workers::spawner` (`processes: Arc::default()`, a new table for each
  spawner) and `WorkerSpawner::identity_state`. The same table serves `WorkerSpawner::signal_group`.
- Evidence:
  - Each `Directories::open` makes a new spawner (`self.workers.spawner()`), so each host handle has an empty process table.
  - Plan 4.1 and LC-12: dropping a `TestkitCore` drops only the host engine. The workers stay in the `Sim`, and a later `open`
    of the same directory adopts them.
  - After that reopen, `identity_state` answers `Absent` for the earlier handle's worker, which still runs. `signal_group`
    finds no cell, so it signals nothing.
  - So in the testkit, `Remove` of an adopted session probes, gets `Absent`, treats the worker as gone, and completes with no
    kill. The worker still runs in the `Sim`, and a route that it serves keeps its bytes flowing (DP-8).
  - Real Core gets `Matches` from the OS for the same worker, kills it, and probes again after `stop_grace`.
  - The host's own unit `World` models this correctly: `World::over` carries the `alive` set across handles. Only the testkit
    edge differs.
- Why: Core A5-1 and A5-4 (one code path; both harnesses run the same suite), and BUILD.md: "A worker behavior that differs
  between the two is a bug in an edge". An edge that answers `Absent` for a live process is worse than the earlier hang. A
  conformance id on Remove after a reopen would pass on the testkit while a worker is left running.
  The PR names this as a limit ("Cross-host process modelling for adoption in the testkit is P5 proper"). But no lead ruling
  defers it, and the new edge method gives a false answer, not a missing one.
- Required:
  1. One process table for each `Workers` (the run's `Sim`), shared by every spawner of the run. Then `identity_state` and
     `signal_group` reach a worker of an earlier handle.
  2. Keep exit delivery per handle. Only the handle that spawned a worker gets its exit from `poll_exit`, as a real host gets
     only the exits of its own children.
  3. Add a testkit test: start a session, drop the handle, reopen, run `AdoptAll`, `Remove` the session. The earlier worker
     is killed (it ends in the `Sim`), and `Remove` completes.
  4. If P5 wants to defer this, ask the lead a QUESTION. Until a ruling, the edge must not answer `Absent` for a process
     that exists.

VERDICT: NOT CLEAN (1 open)
