# Plan: bound lifecycle_baseline_page freeze and traversal

Ticket: `ticket_1786733177_803101`
Target repository: `botster-core`
Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
Pipeline: `botster_stack_delivery`
Run: `run_1786733190_220788`
Subject revision: `f2f3ce2c1a9a3fe266373b69695d737b2b259d9e`

This file is the Plan artifact. Living contract after Implement:
`docs/architecture/control-plane-lifecycle-journal.md`.

Hub Plan Review finding `finding_1786733111_104296` rejected Core
`159d926` for Hub ticket `ticket_1786663582_169720`. This ticket owns
that Core defect. Do not implement Hub projection here.

## Plan Review revision

`review_1786734384_366326` required changes. This revision answers
those findings:

- `finding_1786734384_202247`: freeze index collection and sorting
  remain unbounded. The first draft collected every `.json` path from
  one `read_dir` and then sorted the complete name set. This revision
  forbids that. Index construction walks the live directory iterator
  across calls. Each call examines a bounded number of entries. Each
  accepted name is inserted into an ordered map. No call collects or
  sorts the full remaining set.
- `finding_1786734384_637899`: `Duration::ZERO` only proved the
  API-entry check. This revision requires a test-only elapsed hook
  that expires after a positive number of index, load, map, clone, or
  encode steps. Tests must prove partial progress and then a yield.
- `finding_1786734384_189584`: this revision loads
  [[project-pipelines-playbook]] for workflow policy. The canonical
  vault checklist remains `checklist_1786733574_790265`. No new
  checklist is created.
- `finding_1786734384_232106`: workspace gates now match the ticket
  and current `.github/workflows/ci.yml`. Clippy and doctest do not
  take `BOTSTER_ENV=test`.

`review_1786735088_401140` required a second revision:

- `finding_1786735088_120821`: open directory iteration does not seal
  freeze membership. This revision adds a concrete membership fence.
  Spawn after mint records the new id as excluded. Before an existing
  unseen id is saved or removed, Core inserts its pre-change id and
  row into the freeze. Tests cover spawn-after-open and
  remove-before-visit.
- `finding_1786735088_119458`: a library `#[cfg(test)]` hook is not
  visible to `tests/daemon_integration_test.rs`. Hook-driven elapsed
  and scan-counter tests move into a `daemon.rs` unit-test module,
  next to `observe_pass_snapshot_tests`. Integration tests keep the
  production-path proofs that do not need the hook. Production builds
  do not expose the hook.
- `finding_1786735088_299052`: `load_all` skips malformed JSON.
  Incremental per-id `load` must not turn one bad file into
  `SourceChanged` for the whole freeze. A single-record helper skips
  malformed JSON and fails only on I/O. The existing malformed-record
  test stays, and a paged-path test covers the production page walk.
- `finding_1786735088_288609`: the unchanged base workspace gate was
  red on `159d926`. Operator answer A created blocker
  `ticket_1786735252_213191`. That ticket is now closed. The repair
  merged to `botster-core` main at
  `f2f3ce2c1a9a3fe266373b69695d737b2b259d9e` ("Close attached PTY
  stall before waiting on the worker child."). This Plan visit
  rebases evidence onto that revision and reruns the four ticket
  workspace commands here. This ticket still does not own attach-
  stream repair.

This third Plan visit keeps the membership fence, the `daemon.rs`
unit-test elapsed hook, and the skip-malformed paged-load helper.
Those remain the required answers for
`finding_1786735088_120821`, `finding_1786735088_119458`, and
`finding_1786735088_299052`.

## Target repository and target_id

- Repository: `botster-core`
- Target id: `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`
- Resolved from `list_spawn_targets`, not from the process working
  directory.
- This worktree is a checkout of that repository at
  `f2f3ce2c1a9a3fe266373b69695d737b2b259d9e` (`origin/main` after
  `ticket_1786735252_213191` merged).

## Repository playbook loaded

- [[botster-core-playbook]]

## Other role and surface playbooks and atomic notes loaded

Role overlays:

- [[planner-playbook]]
- [[botster-planner-playbook]]
- [[botster-architecture]]
- [[cli-patterns]] (ownership from the Core charter, not this mixed index)
- [[spa-patterns]] (loaded as planner overlay; no SPA change)
- [[botster runtime teardown lenses]]
- [[botster-runtime-reviewer-playbook]]
- [[project-pipelines-playbook]] (workflow policy: artifacts, checklist
  create-timeout reconciliation, direct-merge, step advance)

Targeted notes:

- [[botster-core uses CI-owned Cargo commands because it has no test script]]
- [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]]
- [[botster core contract surface needs consumer proof]]
- [[botster core public surface needs a narrow start here path]]
- [[core daemon lifecycle metadata is registry backed restart state]]
- [[public dto field additions are source breaking without non exhaustive]]
- [[Hub embeds CoreDaemon behind one client admission point]]
- [[botster packages should enforce core hub cli plugin provider boundaries]]
- [[terminal subscription lifecycle is Core owned while host session policy is Hub owned]]
- [[pipeline vault checklists must cite exact resolvable note titles]]
- [[plan agents must author vault context as wikilinks not home paths]]
- [[vault example paths are not repository placement conventions]]
- [[botster core historical plan docs should not sit beside living architecture]]
- [[colon worktree paths break cargo dyld library paths]]
- [[hearth gate runs require restoring a pipeline wiped gitignore before attribution]]
- [[plugin worker unload deadline can flake under default-concurrency workspace load]]
- [[project pipelines mcp create calls can time out after committing]]
- [[plan review must verify a plan artifact exists before trusting gate summaries]]

Context files: [[identity]], [[goals]].

## Context loaded

- Ticket text: bound freeze creation and page traversal by item, byte,
  and elapsed time. Hub Stage A must keep owner-turn budgets.
- Project: Botster Non-Blocking Event Plane. Core owns the lifecycle
  journal. Hub owns projection.
- Closed parent Core ticket `ticket_1786690597_161141` shipped
  `observe_lifecycle_slice` and `lifecycle_baseline_page` on `159d926`.
- Current defect on `159d926`:
  - `lifecycle_baseline_page` has no elapsed argument.
  - `snapshot = None` calls `mint_baseline_freeze()`.
  - `mint_baseline_freeze` calls `registry.load_all()`, maps every
    `lifecycle_record`, sorts the full set, and retains it.
  - Each later page clones every remaining frozen row, then applies
    `max_rows` and `max_bytes`.
- `SessionRegistry::load_all` reads every `.json` file and sorts by
  `SessionId`.
- `observe_lifecycle_slice` already bounds item, encoded-byte, and
  elapsed work. This ticket must not regress that walk.
- Isolated consumer
  `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped`
  calls `lifecycle_baseline_page` until `complete`. It treats
  `next = None` while `complete = false` as an error. Implement must
  update that consumer for setup-only and index-in-progress yields.
- Hub source does not call `lifecycle_baseline_page` yet.
- Hub ticket `ticket_1786663582_169720` already depends on this ticket.
- Architecture test `lifecycle_api_types_are_control_plane_only` rejects
  terminal bodies on this DTO surface.
- Repo docs: living notes in `docs/architecture/`. Historical plans in
  `docs/archive/plans/`. `docs/plans/` is a retired stub.
- Worktree hygiene: tracked `.gitignore` is non-empty. The worktree path
  has no `:`. No `CARGO_TARGET_DIR` override is required.
- This ticket is not a consumer of Hub session-type eligibility.
- Ticket and current CI workspace gates are:

```sh
BOTSTER_ENV=test cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --doc --workspace
```

CI checks out submodules recursively and installs Zig 0.16.0 before
those commands. A worktree that lacks the Ghostty submodule must
initialize it before the workspace gates.

## Runtime-teardown class

`teardown_class_applies`: yes.

This surface is already runtime-teardown class in
`docs/architecture/control-plane-lifecycle-journal.md`. The ticket bounds
owner-turn CPU work on the same control-plane journal that observes
SessionIo and ClientWorker exit. Full-set directory collect, sort, row
copy, or page clone is control-plane resource spin. This ticket does
not change ClientWorker hard-stop. It must not regress the observe walk.

`teardown_isolation`:

- One baseline freeze is one snapshot identity.
- A new `snapshot = None` replaces an incomplete freeze.
- A failed registry load during mint returns `SourceChanged`,
  `complete = false`, and no rows. It does not stop other sessions.
- Observe still isolates per-session drain errors.

`teardown_bounds`:

- One `lifecycle_baseline_page` call is one non-blocking host tick.
- Item, encoded-byte, and elapsed limits apply to freeze-index
  examination, row materialization, and page encoding.
- No call may collect, sort, copy, or encode the remaining full set.
- Elapsed starts at API entry and includes mint setup.
- A test-only elapsed hook must make a positive budget expire after
  partial work. Production uses `std::time::Instant` only.
- Do not `block_on(close)`. Do not call `drain_runtime_all_once`.
- Baseline reads stay side-effect-free for runtime teardown. They must
  not observe sessions or hard-stop subscriptions.

`late_message_matrix`:

| Message | Tag / owner | After terminal failure / exit | Residual sweep |
| --- | --- | --- | --- |
| `lifecycle_baseline_page` mint | `snapshot_sequence` journal cursor | setup-only or index-in-progress yield keeps identity, `complete = false` | later call with that snapshot continues the same freeze and the same directory iterator |
| `lifecycle_baseline_page` later page | same `snapshot_sequence` | unknown or dropped freeze → `SnapshotUnavailable`, `complete = false`, no rows | recover with `snapshot = None` |
| Foreign `source_id` | requested cursor | `SourceChanged`, `complete = false`, no rows | recover with `snapshot = None` |
| Observe between pages | observe pass identity unchanged | must not mutate already decided freeze rows | unmaterialized freeze ids copy-on-write before registry save or remove |
| Mid-freeze spawn | new `SessionId` recorded in the freeze excluded set before `registry.save` | iterator yield of that name is ignored | wait for a new mint |
| Mid-freeze remove or mutate of an unseen existing id | pre-change id and row inserted into freeze membership before the write | later iterator or page uses the frozen row | do not treat the live file as source |
| `observe_lifecycle_slice` | `(pass_id, optional last_visited)` | unchanged from the parent ticket | unchanged |
| Spawn / Attach / Bind / Drain / Input / `remove_session` / `lifecycle_changes_page` | unchanged | unchanged | freeze copy-on-write only when that path writes a remaining freeze id |
| Terminal `ProcessExited` | `(session_id, subscription_id, generation)` | stays on the terminal plane | baseline pages must not carry it |

`production_path_proof`:

- Production entry: `CoreDaemon::lifecycle_baseline_page`.
- Hub-shaped consumer `install_baseline` is the downstream-shaped
  caller. It must compile against the new budget and resume setup-only
  and index-in-progress yields.
- First-page and later-page large-registry tests prove item, byte, and
  elapsed stops with copy and scan counters.
- Deterministic elapsed tests use the test hook, not wall-clock sleep.
- Assembled complete pages at one `snapshot_sequence` reconstruct the
  freeze.
- Zero-client natural exit still uses `observe_lifecycle_slice` plus
  `lifecycle_changes_page`. No `CoreDaemon::drain`.
- Red-on-revert: restore `mint_baseline_freeze`, one-shot `read_dir`
  collect, or clone-remaining, and the bounded-scan tests fail.

`ownership_identity`:

- Freeze identity is one `SessionLifecycleCursor` captured at mint.
- Page cursor is `after: Option<SessionId>`.
- Setup-only and index-in-progress yields: `sessions` empty,
  `next` may be `None`, `complete = false`, `resync_required` unset.
  The caller retries with `snapshot = Some(identity)` and `after = None`.
- Journal identity remains `(source_id, sequence)`.
- Observe identity remains `(pass_id, optional last_visited)`.
- Terminal identity remains `(session_id, subscription_id, generation)`.
- A dropped complete freeze must not serve later pages.

`sibling_fail_closed_policy`:

- Success: other sessions keep running. An incomplete page is not
  finished ended evidence and is not sibling sacrifice.
- Ultimate mint or page failure: return typed resync or
  `BudgetTooSmall`. Do not shut down sibling sessions.

## Scope

- Keep Core authoritative for lifecycle facts.
- Add elapsed to `lifecycle_baseline_page` through
  `LifecycleBaselineBudget { max_rows, max_bytes, max_elapsed }`.
- Bound freeze-index construction. Walk the directory iterator across
  owner turns. Insert accepted names into an ordered map one entry at
  a time. Do not collect or sort the full remaining name set in one
  call.
- Bound freeze-row materialization. Do not require a completed full-set
  record copy before the first successful page.
- Bound page traversal. Do not clone the entire remaining freeze.
- A first page must return before the whole registry is copied.
- A later page must walk only the next bounded suffix.
- Keep one `snapshot_sequence` for the freeze.
- Incomplete pages have `complete = false`.
- Preserve `snapshot = None` mint, `SnapshotUnavailable` /
  `SourceChanged` resync, and drop-on-complete.
- Keep `observe_lifecycle_slice` as the progress tick. Do not regress
  its item, byte, and elapsed bounds.
- Keep unbounded `lifecycle_baseline` as a documented compatibility
  wrapper only.
- Update the isolated Hub-shaped consumer to pass the budget and to
  retry setup-only and index-in-progress yields.
- Update living docs that still say freeze mint is `load_all()`.

## Non-scope

- Hub projection, host retention, plugin session-family publication,
  and Stage A scheduling. Those stay on Hub
  `ticket_1786663582_169720`.
- Web, TUI, and host-control event surfaces.
- Changes to `take_journal_advanced_wake` or
  `lifecycle_changes_page` beyond freeze copy-on-write glue.
- A replacement test wrapper.
- ClientWorker, SessionIo, terminal protocol, or adapter changes.
- The red `worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`
  failure. That repair belongs to `ticket_1786735252_213191`.
- Pull requests. Direct-merge into `main` after Verify.

## Repository ownership and cross-repo dependencies

- Core owns the incremental freeze, the suffix page walk, the budget
  type, and the isolated consumer proof.
- Hub owns projection and owner-turn scheduling. Hub already depends
  on this ticket through `dependency_1786733189_402719`.
- This ticket depends on Core blocker `ticket_1786735252_213191`
  through `dependency_1786735255_202823`. That blocker owns the red
  attach-stream workspace failure. Both tickets target
  `tgt_1f7bce66eb304881980f9b4a2a5ae3fe`.
- Do not add a Core-to-Hub dependency.
- Do not implement Hub code in this run.
- Web and TUI consume Hub entity state later. Out of scope.

## Assumptions and unknowns

- Assumed: a budget struct is the right public shape. Observe already
  uses `ObserveLifecycleBudget`. This is a source break. Hub does not
  compile against the four-argument form today.
- Assumed: `max_rows` is the item budget for one call. During index
  construction it counts directory entries examined. After the index
  is complete, remaining items in that same call may materialize and
  emit rows. A later page counts only suffix rows it examines or
  copies.
- Assumed: ordered index construction uses per-entry insert into a
  `BTreeMap` or `BTreeSet`. That is not a full-set `sort()` of a
  collected `Vec`.
- Assumed: the freeze retains the live `ReadDir` (or an equivalent
  remaining iterator) across calls. It must not reopen and rescan the
  already examined prefix. If `ReadDir` cannot live on `CoreDaemon`
  because of a `Send` bound, keep a remaining iterator that is filled
  only by taking the next bounded entries from the open directory
  stream. Do not replace that with a full-set collect.
- Assumed: first ordered row emission waits until the directory
  iterator is exhausted. An unread entry might have a smaller
  `SessionId`. Empty incomplete pages during index construction are
  successful pages and keep the freeze identity.
- Assumed: freeze membership is fenced at mint, not by the iterator.
  At `snapshot_sequence` assignment the freeze has an empty membership
  set and an empty excluded set.
  - `spawn` after mint records the new `SessionId` in excluded before
    `registry.save`. A later directory entry for that name is ignored.
  - Before `registry.save` or `registry.remove` of an existing id that
    is not excluded and not yet in membership, Core loads the current
    record with the skip-malformed helper and inserts that pre-change
    id and row into membership. Then the write proceeds.
  - A directory entry for a name that is already excluded is ignored.
  - A directory entry for a name already in membership does not reload
    the live file. The frozen row wins.
  - Assembled pages at one `snapshot_sequence` therefore reconstruct
    one stable freeze even when spawn or remove races the iterator.
- Assumed: production elapsed uses `std::time::Instant` from API entry.
  The elapsed hook and scan counters live only in a `daemon.rs`
  unit-test module, the same way `observe_pass_snapshot_tests` uses
  `observe_index_scans`. `tests/daemon_integration_test.rs` does not
  call the hook. Production builds do not expose the hook.
- Assumed: setup-only and index-in-progress yields return `next = None`
  and `complete = false`. The isolated consumer must retry the same
  snapshot.
- Assumed: `lifecycle_baseline()` may remain a full `load_all()` wrapper.
  Hub Stage A must not call it.
- Assumed: malformed JSON is not a freeze-wide failure. A single-record
  helper returns `Ok(None)` for malformed JSON, `Err` for I/O, and
  `Ok(Some(record))` for a good record. I/O failure on `read_dir` or
  file read may still return `SourceChanged`.
- Assumed: operator answer A is binding. This ticket stays off the
  attach-stream repair. After `ticket_1786735252_213191` merges,
  Implement and Verify rerun the four ticket workspace commands.

## Implementation plan

1. Add `LifecycleBaselineBudget` next to `ObserveLifecycleBudget` in
   `crates/botster-core-daemon/src/api.rs`. Export it from `lib.rs`.
   Document that `max_elapsed` is a host-tick yield bound and does not
   use `now_seconds`.

2. Change `CoreDaemon::lifecycle_baseline_page` to
   `(snapshot, after, budget)`.

3. Replace `BaselineFreeze { snapshot_sequence, rows }` with an
   incremental freeze:

   - `snapshot_sequence`
   - optional open directory iterator
   - excluded set of post-mint created ids
   - ordered membership map of frozen ids to optional already-copied
     rows
   - materialized prefix aligned with membership order
   - flags for index-complete and dropped-on-complete

4. Mint path (`snapshot = None`):

   - Start `Instant` at API entry.
   - Assign `snapshot_sequence = lifecycle_cursor()` first.
   - If elapsed is already exhausted, store the freeze, return an empty
     incomplete page, and do not open the directory.
   - Open `read_dir` at most once per freeze. Store that iterator on
     the freeze.
   - While item and elapsed budgets remain, take the next directory
     entry and count it as one item. Ignore non-`.json` names and
     excluded ids. Insert a new eligible name into membership without
     loading the file body unless this call still has row budget after
     the index completes. Do not collect remaining entries. Do not
     sort a complete name `Vec`.
   - After the iterator is exhausted, mark the index complete. If item,
     byte, and elapsed budgets still remain, materialize and emit the
     first bounded row suffix.
   - If the index is still open, return an empty incomplete page with
     the freeze identity.

5. Later page path (`snapshot = Some`):

   - Match `source_id` and the cached freeze. Otherwise return
     `SourceChanged` or `SnapshotUnavailable` with `complete = false`
     and no rows. Resync outcomes ignore the byte budget.
   - If the index is still open, continue the same iterator under the
     same item and elapsed rules. Do not start a second `read_dir`.
   - After the index is complete, walk only the suffix after `after`.
     Do not clone remaining rows into a temporary `Vec`.
   - Materialize the next membership ids from the already frozen row,
     or from the skip-malformed single-record helper. Skip a malformed
     file and continue. Do not fail the freeze for one bad JSON file.
   - Stop on remaining items, encoded page bytes, or elapsed.

6. Membership fence, called immediately before every `registry.save`
   and `registry.remove` while a freeze is open:

   - If the write creates a new session, insert the id into excluded
     and do not add it to membership.
   - If the write updates or removes an existing id that is not
     excluded and not yet in membership, load the pre-change row with
     the skip-malformed helper and insert that id and row into
     membership. Then perform the write.
   - Current write sites in `daemon.rs` are spawn, persist size,
     `mark_stale`, adopt, `remove_session`, shutdown, and
     `reconcile_lifecycle_observations`. Missing a site is a defect.

6b. Add `SessionRegistry` single-record load that matches `load_all`
    policy: skip malformed JSON, return `None` when the file is
    absent, and return `Err` only for I/O. Do not reuse `load()` for
    the paged production path. Keep
    `registry_load_all_skips_malformed_records_without_blocking_good_records`
    and add a paged-path test.

7. `complete = true` only when the index is complete and the page
   includes the last frozen row, or the sealed freeze has no rows. Drop
   the freeze on complete. A new mint replaces an incomplete freeze.

8. Add `#[cfg(test)]` counters on `CoreDaemon`, same style as
   `observe_index_scans`:

   - `baseline_index_scans` for directory entries examined
   - `baseline_row_copies` for record loads and clones
   - `baseline_page_encodes` for successful-page encodings

9. Add a `#[cfg(test)]` elapsed hook on `CoreDaemonConfig`, for
   example `with_test_baseline_elapsed_per_op(Duration)`. After each
   counted index, load, map, clone, or encode step, add that duration
   to the elapsed check. Put every hook-driven test in a `daemon.rs`
   unit-test module, for example `baseline_freeze_bound_tests`, next
   to `observe_pass_snapshot_tests`. Do not call the hook from
   `tests/daemon_integration_test.rs`. Do not add a public
   test-support feature unless the unit-test module cannot seed a
   large registry. Production builds must not expose the hook.

10. Keep `lifecycle_baseline()` as the unbounded wrapper. Docs must say
    Hub Stage A must not call it.

## Affected surfaces and files

- `crates/botster-core-daemon/src/api.rs`
- `crates/botster-core-daemon/src/lib.rs`
- `crates/botster-core-daemon/src/daemon.rs`
- `crates/botster-core-daemon/src/registry.rs` for a skip-malformed
  single-record helper. Do not add a helper that returns every path.
- `crates/botster-core-daemon/src/daemon.rs` unit-test module
  `baseline_freeze_bound_tests` for hook-driven elapsed and scan
  counters
- `crates/botster-core-daemon/tests/daemon_integration_test.rs` for
  reconstruction, membership-fence, and malformed-record production
  path proofs
- `crates/botster-core-test-support/tests/consumers/hub-lifecycle-shaped/src/lib.rs`
- `crates/botster-core-test-support/tests/lifecycle_journal_consumer_test.rs`
- `docs/architecture/control-plane-lifecycle-journal.md`
- `docs/architecture/core-daemon.md`
- Root `README.md` production-path paragraph
- This plan file under `docs/archive/plans/`

## Risks

- Copy-on-write can miss a registry write site. Then observe between
  pages mutates an unmaterialized suffix and breaks freeze identity.
- Setup-only or index-in-progress `next = None` can break the current
  isolated consumer if Implement forgets the retry rule.
- Holding `ReadDir` across owner turns can fail a `Send` bound. The
  fallback is still a remaining iterator, not a full-set collect.
- A missed excluded-set insert on spawn lets a post-mint session join
  an older snapshot. A missed pre-change insert on remove drops a row
  from the freeze.
- A budget struct is source-breaking. Hub is not a caller today.
- Large-registry tests that spawn real PTYs will be too slow. Seed
  registry JSON through `SessionRegistry::save`. Do not spawn 100,000
  sessions.
- The vault note [[botster-core uses CI-owned Cargo commands because it has no test script]]
  prefixes Clippy and doctest with `BOTSTER_ENV=test`. The ticket and
  current CI do not. This run follows the ticket and CI. Record that
  drift as a vault gap.
- `[[plugin worker unload deadline can flake under default-concurrency workspace load]]`
  is diagnostic only. It is not a workspace-gate waiver.

## Acceptance checks and tests

Focused tests (add or extend):

- First-call item limit on a large registry during index construction:
  `max_rows = 1` examines one directory entry, copies zero or one row,
  and leaves the remaining names unvisited.
- First-page item limit after the index is complete: `max_rows = 1`
  copies one row, not the remaining set.
- First-page encoded-byte limit: stop before remaining rows are copied
  or encoded.
- Setup-only elapsed and mid-work elapsed live in the `daemon.rs`
  unit-test module. `Duration::ZERO` returns the freeze identity,
  `complete = false`, zero index scans, and zero row copies. The
  per-op hook gives a positive `max_elapsed` a known number of index
  or row steps and then expires. Assert bounded
  `baseline_index_scans`, `baseline_row_copies`, and
  `baseline_page_encodes`. Assert the unvisited suffix is untouched.
  Do not use wall-clock sleep as the oracle. Do not put these hook
  tests in `tests/daemon_integration_test.rs`.
- Spawn-after-open: mint a freeze, spawn a new session, then finish
  paging. The assembled freeze must omit the new session.
- Remove-before-visit: mint a freeze, remove an unseen existing
  session, then finish paging. The assembled freeze must include the
  pre-remove row.
- Malformed JSON in the registry directory must not block good rows on
  the paged path. Keep
  `registry_load_all_skips_malformed_records_without_blocking_good_records`
  and add a `lifecycle_baseline_page` counterpart.
- Later-page item, byte, and elapsed limits: walk only the next suffix.
  Counters must not include the already returned prefix or the
  unvisited tail.
- Assembled complete pages at one `snapshot_sequence` reconstruct the
  freeze. With no mid-page mutations they still match
  `lifecycle_baseline()`.
- Incomplete page has `complete = false` and is not finished ended
  evidence.
- Existing `lifecycle_baseline_pages_ignore_observe_mutations` still
  passes. Observe after the first page must not change a later frozen
  row.
- Zero-client natural exit still advances through
  `observe_lifecycle_slice` and `lifecycle_changes_page` without
  `CoreDaemon::drain`.
- `lifecycle_api_types_are_control_plane_only` still rejects terminal
  bodies. Keep `SessionLifecycleBaselinePage` in that section.
- Isolated Hub-shaped consumer compiles against
  `LifecycleBaselineBudget`. `install_baseline` retries setup-only and
  index-in-progress yields and still refuses unbounded
  `observe_lifecycle`.
- Existing `observe_lifecycle_slice` first-pass and suffix-scan tests
  stay green.

Workspace gates. Use these exact ticket commands. Current CI uses the
same Clippy, fmt, and doctest lines. Workspace test in CI omits
`BOTSTER_ENV=test`; the ticket requires the prefix on that one command.

```sh
BOTSTER_ENV=test cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --doc --workspace
```

If the worktree does not already have the Ghostty submodule, initialize
it before those gates. CI does `actions/checkout` with
`submodules: recursive` and installs Zig 0.16.0.

The `159d926` attach-stream failure is closed by merged blocker
`ticket_1786735252_213191` at
`f2f3ce2c1a9a3fe266373b69695d737b2b259d9e`. Plan reran the four
ticket commands on that detached revision:

- `cargo fmt --all -- --check` — pass
- `cargo clippy --workspace --all-targets -- -D warnings` — pass
- `BOTSTER_ENV=test cargo test --workspace` — pass on a clean sequential
  run. `worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output`
  passed. A first workspace run failed
  `natural_exit_capture_color_and_snapshot_freezes_repeatable_pair`;
  two isolated reruns passed; the clean sequential workspace run
  passed. A concurrent second workspace run hit
  `unload_first_then_deadline_keeps_only_worker_stopped`, which is the
  documented plugin-worker flake, not a new attach-stream failure.
- `cargo test --doc --workspace` — pass

Implement and Verify must rerun the same four commands after the
lifecycle-baseline change. Do not treat a later attach failure as a
flake.

Do not invent a test wrapper. Focused Cargo tests are allowed during
development.

Downstream proof: the isolated Hub-shaped consumer is the required
downstream-shaped proof for this public contract. Hub production wiring
stays on `ticket_1786663582_169720`.

## Vault gaps

- [[Core control-plane lifecycle journal advances without a terminal client or Hub terminal Drain]]
  still names unbounded `observe_lifecycle` as the progress operation.
  Capture an update after Implement if the note is still stale.
- Incremental freeze plus copy-on-write is a new durable convention if
  it ships. Capture it after Implement.
- [[botster-core uses CI-owned Cargo commands because it has no test script]]
  disagrees with current CI and this ticket on `BOTSTER_ENV=test` for
  Clippy and doctest. Do not change the vault note in this Core ticket
  unless Implement must cite one command set. Prefer the ticket and CI.

## Delivery policy

- Direct-merge pipeline.
- Merge the verified change into `main`.
- Do not create a pull request.
- Do not require human pull-request sign-off.
