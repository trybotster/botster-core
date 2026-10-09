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

## Round 2 — Head e6d9f48

Reviewed head: `e6d9f487a5c815e8d3abfd922b46d722af5db5b9`. The base is still current v1 `144b023`.
Reviewed delta: `c4afe58..e6d9f48`, six commits, 17 files. This reviewer ran no build, test or gate. The storage layout
follows the lead's A1 rulings (state log, 2026-10-04: a reversible base32 path, chunked components with per-component
`openat`/`mkdirat`, and foreign files counted and left alone). Its file-level logic is the P5 reviewer's scope.

- **K1 CLOSED.**
  - `Workers::run_processes` is one table per run. It maps each identity to the process cell and to the table of the handle
    that spawned it.
  - `identity_state` and `signal_group` use the run table, so a later handle reaches an earlier handle's worker.
  - A `Kill` ends the process through its spawner's table, so the exit still goes only to the spawning handle, as a real
    reaper's does.
  - `a_reopened_handle_sees_and_ends_the_worker_of_the_earlier_handle` runs the whole path on the real testkit: start, drop,
    reopen, `AdoptAll`, `Remove`. It shows `Matches` before and `Absent` after, and both ops complete `Ok`. It reads the
    identity with Core's own row decoder. Its loop of 64 pumps is bounded and has no sleep.
- **New interface: `HostEdges::diagnostics()`.** It has a default `Null`, and the driver merges it under
  `diagnostics()["edges"]`. LC-10 makes diagnostics one opaque value, so the testkit's `null` and the real edges'
  `{foreign_registry_files}` may differ without breaking A5-4. Accepted.
- **Dependencies.** The workspace drops `atomic-write-file` and `sha2` (from botster-core-sys) and adds `data-encoding`.

#### K2 [LOW] OPEN — The Prior art note says "Nothing hand-rolled", but the PR now writes its own atomic file write

- Location: PR #164 body, "Prior art". `crates/botster-core-sys/src/storage.rs` `write_row`: a temporary file, `sync_all`,
  `renameat`, and an `fsync` of the directory.
- Evidence: plan 7.2 adopts `atomic-write-file` for `Storage` ("Hand-rolling this needs no reason to exist"). BUILD.md rule 0
  requires a recorded reason for anything hand-rolled. The lead's chunked-path ruling needs `openat`, `mkdirat` and
  `renameat` relative to a directory, because no `PATH_MAX` may apply. A path-based crate cannot do that, so a reason
  exists. But the note still says "Nothing hand-rolled" and does not name `data-encoding`.
- Required: update the Prior art note. Name the hand-rolled atomic write, with its reason (the dirfd-relative steps that the
  lead's A1 ruling requires; `atomic-write-file` takes a path). Name `data-encoding` (base32) as the adopted library.

#### K3 [LOW] OPEN — A storage error with no errno becomes `errno: 0`

- Location: `crates/botster-core/src/real.rs` `read_rows`: `StorageError::Failed { errno: error.raw_os_error().unwrap_or(0) }`.
- Evidence: 0 is not an error number. The value looks valid and means nothing, which is the pattern of audit A52. P3's fix
  for A52 named one constant for "an OS failure that carries no errno" (`EIO` in botster-worker).
- Required: use the same rule (`EIO`, or a named constant). An error that cannot occur here is a documented `expect`.

Package verdict: the P5 reviewer's round 1 (`22720e0`) is NOT CLEAN on the old head. CLEAN needs the package verdict on the
same head. Most commit subjects in the delta start with "wip:". That is not a finding, because no force-push is allowed, but
the merge description should say what landed.

VERDICT: NOT CLEAN (2 open: K2, K3)

## Round 3 — Head 26c4c2e

Reviewed head: `26c4c2ef58d7ac20b059487013da760bf34911eb`. Delta `e6d9f48..26c4c2e`, one commit: botster-core-sys
`storage.rs` and botster-core `real.rs`. The base is still current v1 `144b023`. This reviewer ran no build, test or gate.

- **K3 CLOSED.** `storage::errno` keeps an OS errno and maps an error that has none to `EIO`, never 0. `FileStorage::scan`
  returns `StorageError` itself, so `real.rs` `read_rows` maps nothing (`self.storage.scan()?`). The rule matches P3's A52
  constant. `an_errno_is_kept_and_a_missing_one_is_eio` is a small mapping test. Its expected values come from its inputs
  and from the stated rule.
- **K2 CLOSED.** The Prior art note now names:
  - `data-encoding` as added (lowercase base32, no padding);
  - `atomic-write-file` as removed, because it takes a path and the A1 ruling requires dirfd-relative steps;
  - the hand-rolled atomic write and directory walk, each with its reason.
- Implementer evidence: `~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-26c4c2ef-mac-20261004-220225-16996.log`.
  It is a focused Mac run, not the landing gate.

Integration findings: none open. Still needed for CLEAN: the P5 package verdict on `26c4c2e`.

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)

## Round 4 — Head 737b201 (package scope only)

Reviewed head: `737b2017df84f2d7fa3641c7e91f065b13ecc6ff`. Delta `26c4c2e..737b201` (`f542a7e9`, `737b2017`), 5 files:
host `run.rs` and tests, sys `storage.rs`, and a facade slow test. The base is still current v1 `144b023`.
This reviewer ran no build, test or gate.

- No interface between packages changes. `HostEdges`, `HostDriver::open`, `HostEngine::new`, the testkit edges, and the
  `Storage` trait are as in round 3. The changes are the `AdoptAll` row ordering (`row_waits_for_its_session`) and the
  durability of storage directories, which answer the P5 reviewer's round 2.
- No integration finding is opened. The logic is the P5 reviewer's scope.

Integration findings: none open. Still needed for CLEAN: the P5 package verdict on `737b201`.

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)

## Round 5 — Head 1b1340c

Reviewed head: `1b1340cf80ed1f1aa3b05fba7d2e524c8b30de1a`. Delta `737b201..1b1340c`, one commit: sys `storage.rs`, a facade
slow test, and the host `DESIGN.md`. The base is still current v1 `144b023`. This reviewer ran no build, test or gate.

- No public signature changes. `create_durably` takes the directory sync as a private function parameter. That is
  dependency injection for its unit test (`a_retried_open_syncs_the_ancestors_that_a_failed_open_left`), not a test branch.

#### K4 [LOW] OPEN — `Core::open` now opens every ancestor of `data_dir` up to `/` for an fsync, so a parent that cannot be read makes the open fail

- Location: `storage.rs` `create_durably`: `for ancestor in fs::canonicalize(path)?.ancestors().skip(1) { sync(ancestor)? }`
  with `sync_dir` = `File::open(dir)?.sync_all()`. It runs at every open, even when no directory was created.
- Evidence: opening a directory for `fsync` needs read permission. An ancestor that the host can pass through but cannot read
  (mode `0711`, for example a home or shared root on some Linux hosts) gives `EACCES`. Then `Core::open` fails
  `RegistryFailed` for a `data_dir` that worked before this change. The old code synced only the parents of the directories
  that it created. This crosses packages because the Hub calls `Core::open`.
- Required: keep the retry durability that the package reviewer asked for, without a new failure for an ancestor that cannot
  be read. For example, an `EACCES` or `EPERM` on an ancestor above the first one that this host can write is not a failure.
  Or stop at a documented root, with the reason. Record the decision in `DESIGN.md`. Add a unit case through the injected
  `sync`: an `EACCES` above the data directory still opens.

Package verdict: pending on this head.

VERDICT: NOT CLEAN (1 open: K4; package verdict pending)

## Round 6 — Head 428c783 (K4 fix)

Reviewed head: `428c783967ca6e67d7cccb36db8336eb96aef004`. Delta `1b1340c..428c783`, one commit: sys `storage.rs` and host
`DESIGN.md`. The base is still current v1 `144b023`. This reviewer ran no build, test or gate. The implementer reports a Mac
run: fmt and clippy clean, 487 unit and 58 slow tests pass
(`~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-428c7839-mac-20261004-221459-45851.log`).

- **K4 CLOSED.** `create_durably` syncs the ancestors going up. It stops at the first ancestor that the host cannot write
  (`access(W_OK)`). Below that point, any sync error fails the open.
  - This meets the rule that the P5 reviewer and this reviewer agreed. The host can create a directory only in a directory
    that it can write, so every directory that Core may have created, in this open or in an earlier failed one, has a parent
    in the synced chain. An execute-only (`0711`) ancestor above the chain is never opened.
  - `DirEdges` injects the sync and the write check as private parameters. That is not a production test branch.
  - Test `an_unreadable_ancestor_fails_the_open_only_below_the_first_unwritable_one` checks two cases. An `EACCES` above the
    writable chain still opens, and only the writable ancestor is synced. An `EACCES` on the parent of a created directory
    fails `PermissionDenied`. The expected values come from the injected layout. The retry test is kept.
  - `DESIGN.md` records the rule and its reason.
- **Remaining limit, for the lead's ruling.** The write check runs at the time of the retry. Suppose an ancestor's write
  permission is removed after a failed open created a directory in it. Then the retry stops below that ancestor, and that
  entry's parent is not synced. This needs a permission change between two opens of the same data directory. This reviewer
  accepts the limit if it is recorded in `DESIGN.md`. If the lead's ruling on the access and durability boundary differs, a
  new head gets a delta review here.

Integration findings: none open. The P5 package verdict on `428c783` is still needed. Round 4 of that review (`fe85b42d`, at
`1b1340c`) kept only K4 open, and it asked the lead for a ruling.

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)

Correction: the documentation requirement above is an open finding, so it is tracked here.

#### K5 [LOW] OPEN — Record the remaining limit of the retry durability rule

- Required: `DESIGN.md` states that the write check runs at the time of the retry. A write permission that is removed after a
  failed open created a directory leaves that entry's parent unsynced. The lead's ruling on the boundary may replace this.

VERDICT: NOT CLEAN (2 open: K5; package verdict pending)

## Round 6 note — The lead's ruling replaces the writable-ancestor walk

The P5 reviewer relays the lead's ruling on the access and durability boundary:
- Core creates only the last component of `data_dir`. A missing parent fails `open` with the existing typed registry I/O
  error.
- On every open, Core fsyncs `data_dir` and its direct parent. The parent must be openable for the fsync.
- No other ancestor is synced. The ancestors that the host provides are the host's responsibility.
- Required cases: a parent that is missing or unreadable; a retry that syncs the parent; a grandparent that is execute-only.

This replaces the round 6 design. K5 is therefore moot and closes with no change, because the walk that it records no longer
exists. K4 is checked again against the ruling on the next exact head.

VERDICT: NOT CLEAN (pending a new head under the lead's ruling, and the package verdict)

## Round 7 — Head 1bcbb38 (the lead's K4 ruling)

Reviewed head: `1bcbb38520440ee7e98910373926156ff886edd4`. Delta `428c783..1bcbb38`: sys `storage.rs` and host `DESIGN.md`.
The base is still current v1 `144b023`. This reviewer ran no build, test or gate.

- **K4 CLOSED under the ruling.**
  - `create_data_dir` makes only `data_dir`, with a mkdir that is not recursive, so a missing parent fails with `NotFound`.
  - `check_safe` runs next. Then `sync(path/..)` runs, and any error there fails the open.
  - No ancestor walk or `W_OK` check is left, so an execute-only grandparent is never opened.
  - The `DataDir::open` rustdoc states the two host requirements.
- **K5** stays moot, because the walk that it recorded no longer exists.
- The implementer's Mac run shows 487 unit and 59 slow tests passing, with one timeout:
  `botster-core::slow_real_core common::process_guard::parent_dies_before_fifo_reader`. That is the shared guard defect
  that PR #165 fixes, and the lead's merge order handles it. It is not this PR's.

#### K6 [LOW] OPEN — The facade does not tell the Hub about the new requirements on `data_dir`

- Location: `crates/botster-core/src/lib.rs` `Core::open` rustdoc. Only `DataDir::open` in botster-core-sys states them.
- Evidence: before this PR, `Core::open` created any missing ancestors of `data_dir`. Now the parent must exist, and it must be
  readable. The Hub uses only the facade (plan 2.6: the other crates make "no compatibility promise; use `botster-core`"). So
  the facade's doc is where a Hub author learns these requirements.
- Required: state both requirements, and the `RegistryFailed` that follows when one is not met, in the `Core::open` rustdoc.

#### K7 [LOW] OPEN — The unreadable-parent test also passes when the open succeeds

- Location: `storage.rs` `a_parent_that_cannot_be_read_fails_the_open`: `match result { Err(Io(PermissionDenied)) => …, Ok(_) => {}, … }`.
- Evidence: the test's name and the ruling say the open fails. The `Ok(_)` arm lets the test pass on exactly the defect that it
  names. The likely reason is a root user, for whom `0300` does not stop the open (the Linux gate container may run as root).
  As written, though, it accepts success on any platform.
- Required:
  - Assert `PermissionDenied` unconditionally.
  - Where `EACCES` cannot occur (`geteuid() == 0`), skip the test with a printed reason, so the skip is visible.
  - The injected-sync retry test already proves that a failed parent sync fails the open, so that case is covered.

The P5 package verdict on this head is still needed.

VERDICT: NOT CLEAN (2 open: K6, K7; package verdict pending)

## Round 8 — Head b8b37a6

Reviewed head: `b8b37a6d1161b83138a5e5e0e73bb68fc57e2db7`. Delta `1bcbb38..b8b37a6`, one commit: sys `storage.rs` and the facade
`lib.rs`. The base is still current v1 `144b023`. This reviewer ran no build, test or gate.

- **K6 CLOSED.** The `Core::open` rustdoc states both requirements on `data_dir`: the parent exists, and it can be opened for
  reading. It states that Core syncs `data_dir` and its parent and no other ancestor, and that `RegistryFailed` follows when
  a requirement is not met. The text changes no signature, so the `public-api` snapshot does not change.
- **K7 CLOSED.** `assert_permission_denied` requires `PermissionDenied`, and it skips with a printed reason only when
  `geteuid().is_root()`. The same helper replaces the `Ok(_)` arm in two older tests that had the same defect.
- The package reviewer's LOW (an asserted `epoch == 1` in the grandparent test) is answered: the test now asserts only that
  the open succeeds.
- Implementer evidence: `~/botster-sessions/gates/botster-core-stage1-p5-audit-contract-b8b37a6d-mac-20261004-222024-65207.log`.
  It is a focused Mac run at this exact head: 487 unit and 60 slow tests pass, and it exits 0. This reviewer read its summary
  lines.

Integration findings: none open. Still needed for CLEAN: the P5 package verdict on `b8b37a6`.

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)

## Round 9 — CLEAN on head b8b37a6

Reviewed head: `b8b37a6d1161b83138a5e5e0e73bb68fc57e2db7`, unchanged since round 8. Base and merge base: current v1
`144b0234fb632bcbb5176b17c2fe55f3239405df`.
Package verdict: CLEAN on this exact head. P5 reviewer round 7, commit `89893b23ddc24d7c0ff6844a199e745bd52ec942`, file
`verdicts/p5-adoption.md` on `origin/stage1/review-p5`. It names this head and ends `VERDICT: CLEAN`.

Integration findings, all closed:
- K1 (round 2): one testkit process table per run, with exits that go to the spawning handle.
- K2 and K3 (round 3): the Prior art note; `EIO` in place of `errno` 0.
- K4 (round 7): the lead's ruling on the `data_dir` boundary.
- K5: moot under that ruling.
- K6 and K7 (round 8): the facade doc; the test that accepted `Ok`.

Merge checks: `git merge-tree` of this head with each open PR has no conflict.
- with the guard PR #165 `08fef89d`: tree `f320a02`;
- with PR #163 `40b63dc`: tree `7a47b67`;
- with PR #162 `a93a13c`: tree `4e54067`.
This reviewer ran no gate. Under the lead's rules, the merge still needs one green `botster-gate` on this exact head. A gate
before the guard PR lands can fail on the shared guard test `parent_dies_before_fifo_reader`, which is not this PR's defect.
The lead orders the merges.

VERDICT: CLEAN (0 open)

## Round 10 — CLEAN on head e3c1fabf (merge of v1 a22811b)

Reviewed head: `e3c1fabf754ddce9439d653783b083969f2057be`, parents `b8b37a6d` (round 9 CLEAN, `9edbc27`) and `a22811b6`
(= `origin/v1` at review time: #166 docs and #162 with the #165 guard). Tree `f21b1eb633239189de381a570390c2d084ecdbaa`,
equal to this reviewer's `git merge-tree --write-tree origin/v1 b8b37a6d`. No edit in the merge. This reviewer ran no
build, test or gate.

- **The PR's change is the reviewed change.** `git diff origin/v1 e3c1fabf` and `git diff 144b023 b8b37a6d` touch the same
  files. Every blob is identical to `b8b37a6d` except the three files that both sides changed. In each of those, the
  added and removed lines of the two diffs are identical:
  - `Cargo.lock`;
  - `crates/botster-core-sys/Cargo.toml`: #164 swaps `atomic-write-file` and `sha2` for `data-encoding` in
    `[dependencies]`; v1's macOS `libc` dev-dependency of the guard stays;
  - `crates/botster-core/tests/slow_real_core.rs`: #164's A7 and A1 tests sit beside #162's A10 test, with no line of
    either changed.
- **Cross-package effects of v1 on #164:** v1 since `144b023` changes no `src/` file. #164 adds no `GroupGuard` user, so the
  #165 guard change does not reach it. The HOLD on new real-process test code (lead, 2026-10-08) post-dates this PR's
  CLEAN content; the merge adds none.
- **Evidence:** the static Linux run at this head
  (`…p5-audit-contract-e3c1fabf-pool-20261008-211127-38031.log`): taint PASS, lists PASS, exit 0 (fmt ran first in the
  `&&` chain). This is not the landing gate.
- **Merge order note:** #142 (`112c0310`) is also built on v1 `a22811b`. A trial merge of the two heads is clean (tree
  `3eb84d6`). Whichever of the two lands second must merge the new v1 first, so that the lead's tree check holds, and gets
  a docs-free merge-only delta check.

VERDICT: CLEAN (0 open) at e3c1fabf754ddce9439d653783b083969f2057be

## Round 11 — CLEAN on head e9864aaa (fix round after the red mutants gate at e3c1fabf)

Reviewed head: `e9864aaa1d71cd75c7f22adda988205ace3125bb`. It contains the merge `cb521e9e` of v1 `0b0eecc0` (tree `3eb84d6`,
equal to this reviewer's trial merge of #142 and #164 in round 10) and five fix commits after it. The full Linux gate at
`e3c1fabf` (`…p5-audit-contract-e3c1fabf-pool-20261008-211422-42727.log`) was red in mutants only (42 missed). This
reviewer ran no build, test or gate. The delta is now cross-package (xtask, nextest config, the facade rustdoc, the host
test `World`, core-sys storage).

- **xtask (cross-package, shared).** `pub const SLOW_FILTER = "binary(/^slow/) or test(/::slow_tests::/)"`. The in-crate
  `slow_tests` of core-sys `storage.rs` and botster-core `real.rs` never ran in a gate before, while `mutants.toml` cited
  them; this closes that gap for #164. nextest names a unit test by its module path without the crate
  (`storage::slow_tests::…`, `real::slow_tests::…`), so `::slow_tests::` matches both.
- **`.config/nextest.toml`:** one `success-output = "immediate"` override for the confinement test, so the gate log shows
  which branch ran. No other profile change.
- **Facade rustdoc (`Core::open`) and host `DESIGN.md`:** the AppArmor path limit (about 8 KiB; `Create` fails
  `RegistryFailed` and leaves nothing) under the lead's ruling on #164, with the `max_session_id_bytes` ceiling recorded as
  an open steward question. The docs agree on both sides.
- **Host test `World::over`:** the new handle has a higher host epoch (DP-8) and continues the pids. Test code only; it
  removes two reuses that real Core never makes.
- **Storage (P5 package scope):** `write_row` removes the directories that it made on a failure; new slow tests use the real
  disk only. No process is started, so the HOLD on real-process test code does not apply.
- **mutants.toml.** One entry per function, each naming its catching slow tests; written arguments for the flag mutants
  (`|` to `^` on distinct bits, the pre-accepted class; `|` to `&` dropping `O_DIRECTORY`, or `O_NOFOLLOW` and `O_CLOEXEC`,
  argued per caller); `HostEdges::diagnostics`'s default body (`Value::Null` = `Value::default()`). Evidence, read by
  this reviewer:
  - `…p5-audit-contract-2121aaca-pool-20261008-214504-93381.log`: slow-tier mutants of `storage.rs` and `real.rs`
    (`--features slow`, nextest): 91 tested, 68 caught, 10 unviable, 13 missed = the `open_dir` and `place` flag mutants
    named as equivalent, and four `read_row` flag mutants.
  - `…p5-audit-contract-b48a6d8d-pool-20261008-215154-3101.log`: `read_row` after the link test: 8 tested, 6 caught, 2
    missed = `|` to `^` (distinct bits).
  - The gate's mutants step runs with `--test-tool nextest` (`xtask/src/ci.rs:284`), so each test has its own process.
    This matters for `a_failed_write_removes_the_directories_that_it_made`, which lowers the process's `RLIMIT_NOFILE`
    around one call; it is safe only with one test per process (nextest). P6's PR B check should know this pattern.
- Other evidence: `e9864aa` Linux static steps and unit tests (`…e9864aaa-pool-20261008-215328-5780.log`); `e9864aa` Mac
  slow tier with the new selection, 171 passed (`…e9864aaa-pool-20261008-215235-3778.log`).

**Merge requirements (not findings at this head; #167 lands first by the lead's order).** A trial merge of #167
`723bc8d5` with this head CONFLICTS in `xtask/src/test_budget.rs` and `.cargo/mutants.toml`. The resolution must:
1. keep one `SLOW_FILTER`, `pub` (P6's PR B reads it), whose set includes both filters: #167's
   `binary(/^slow/) | test(/(^|::)slow_/)` includes `::slow_tests::`, so take it;
2. update the `mutants.toml` storage comment "xtask selects `::slow_tests::` since #164" to the merged filter;
3. keep every entry of both sides in `mutants.toml`, and update the xtask selection test to the merged string.
That merge head gets a merge-only check by this reviewer before the ONE Linux gate.

VERDICT: CLEAN (0 open) at e9864aaa1d71cd75c7f22adda988205ace3125bb
