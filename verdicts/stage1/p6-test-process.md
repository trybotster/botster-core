# Integration review: #171 (P6 PR A, botster-test-process, branch stage1/p6-test-process)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head dc7fca5a

Reviewed head: `dc7fca5a1bfa207b4a103b457c4981baaa25e975`. Base: v1 `59ce126885e04a3b3f22d97e89337bfdbf055949` (merged at
`dc7fca5a`). Scope: the new crate `crates/botster-test-process`, `xtask` (`ci.rs`, `prebuild.rs`), `.cargo/mutants.toml`
and the merge. 19 files, 3,118 added lines.

### Checked, no finding

- **The merge `dc7fca5a`.** I compared the whole tree with `git merge-tree --write-tree` of its parents. The only
  difference is the resolution of the conflict markers in `.cargo/mutants.toml`. No file that merged cleanly was changed
  by hand.
- **Amended anchor item 10 (plan r22).**
  - One reserve: `rounds::end_group` spawns one reserve child (`/usr/bin/true`) in the group, then the anchor leaves the
    group (`setpgid(None, None)`). The anchor is a grandchild of the wrapper (the intermediate stage exits), so it is not
    the leader of production's group, and the `setpgid` call moves it out.
  - Kills in rounds: `end_members` lists, kills and awaits until no live member is left. A member that a wait calls gone
    twice while the listing still calls it live is reported as left, so the loop cannot spin. The default-tier tests
    (`ForkRace` and the others in the PR body's table) prove the decision.
  - A kill only while the reserve holds the group: `reserved_kill` first reads the reserve's wait status with
    `WEXITED|WNOWAIT|WNOHANG` for its exact pid. If the reserve is no longer an unreaped child, it sends nothing.
  - Exact-pid waits only: `waitid(WaitId::Pid(..))` in `child.rs` (`OBSERVE`) and `rounds.rs` (`RESERVE_PROBE`),
    `Child::wait` on the test's own child or the anchor's own reserve or intermediate. No `waitpid(-1)`, 0 or negative
    pgid is used.
  - The bound: the binary calls `end_group(group, Deadline::after(cleanup))`, and the guard waits for the report by
    `grace + cleanup + CLEANUP`. `CLEANUP` stays 10 s. A failure at the bound is `Line::Fail` with the members left, and
    `a_cleanup_that_cannot_finish_fails_through_the_guard` proves that the guard's drop fails with it.
  - `verify` keeps the start-time check of the leader and the group-move refusal (`Line::Refused`), proved by
    `a_leader_that_moved_to_another_group_is_refused_and_not_signalled`.
  - No reap of a production child: `the_guard_ends_the_group_and_production_still_reaps_its_own_child` reaps
    production's child by its exact pid after the guard's drop and gets the KILL status. The anchor is not production's
    parent, so it cannot reap production's children.
- **`OwnedChild` in group mode** uses the unreaped leader as the reserve (`reserved_kill(self.pid, self.pid)`). The test
  process is outside the group, so the rounds never signal it. This is a clean reuse of one mechanism.
- **Blocking calls.** I grepped every added line under `crates/` and `xtask/` for `read_line(`, `read_to_end`,
  `read_exact`, `.recv()`, `.wait()`, `wait_with_output`, `.output()`, `.status()`, `sleep(`, `.join()`, `io::copy`,
  `loop {` and `while`.
  - Every read polls with the deadline first (`Bounded::fill`), and every loop checks its deadline before each read.
  - Every `Child::wait` follows an observed exit (`await_end` within a deadline), except the one in R1 below.
  - The `.status()` hits are `OwnedChild::status`, which observes the exit within `CLEANUP` first.
  - The anchor's `io::copy` of the guard connection waits for the end of the test. It is the anchor's lifetime by design,
    and the test's death ends it (`the_anchor_ends_the_group_when_the_test_dies`).
- **Mutation exclusions (`.cargo/mutants.toml` +98).**
  - The slow-tier whole-function entries (`child.rs`, `fixture.rs`, `rounds.rs` `reserved_kill` and `end_group`, the
    anchor binary) each name their slow tests. The proof is the slow-tier run at `d2338cb6` with `--no-config` and
    `--max-fail 1:immediate`: 190 tested, 153 caught, 3 missed, 0 timeouts. All three misses are excluded with reasons
    (the `WouldBlock` guard of `Guard::accept`, `gone_at_open`, `ended_since_listing`). `git diff d2338cb6 dc7fca5a` of
    the crate's `src` changes only a macOS test and a doc comment, so that run applies to this head.
  - The decisions stay in the default tier: `end_members`, `Line`, the guard's outcome and `Bounded`.
  - `Connection::read -> Ok(())` and `Bounded::fill -> Ok(true)`: the default run counts them as TIMEOUT, and the slow
    run above catches them.
  - `macos.rs` `start_time` `+` with `-`: for `sec * 1_000_000 ± usec` with `usec < 1_000_000`, both mappings are
    one-to-one, and callers compare only for equality. The other macOS entries name their reasons.
  - The `prebuild.rs` entries exclude only the whole-body `Ok` replacements, and they name the decision functions and
    their tests (the #167 `mutants_job` rule, plan r22 section 8).
- **`OFF_MACOS_EXCLUSIONS` (`ci.rs`).** The four `platform/macos.rs` entries are platform coverage exceptions. The focused
  Mac run at `2d1c53a7` covers them (13 caught, 2 unviable, 0 missed). `git diff --stat 2d1c53a7 dc7fca5a` of the crate's
  `src` is empty.
- **The two TIMEOUTs of the slow-profile in-diff run at dc7fca5a** (`Deadline::expired -> false`, `delete !` in
  `Bounded<R>::line`) are catches. The focused run with `--max-fail 1:immediate` (log `…002301-9071.log`) shows "2
  mutants tested in 2s: 2 caught". They show as TIMEOUT only because sibling tests hang when the bound itself is broken.

### R1 LOW — `end_group` reaps its reserve with an unbounded wait

`rounds.rs` `end_group` ends with:

    // The reserve has ended (by itself or by a kill); its reap, by its exact pid, gives the id back.
    let _ = reserve.wait();

The comment is not true on every path. On the `Ok` path the last listing found no live member, so the reserve is a
zombie, and the wait returns at once. But `end_members` can return `Failure::Error` before any kill, for example when
the first `live_members` call fails. Then the reserve can still be live, and no kill reached it. `/usr/bin/true` exits by
itself, but a stopped reserve (a job-control signal to the group) never does. The anchor then blocks forever, and the
guard gets no report until its own bound.

Every other `Child::wait` in the crate follows an observed exit (`OwnedChild::reap`, the wrapper's `intermediate.wait()`).
BUILD rule 5 requires every wait to be bounded, and amended item 10 requires the rounds to report failure at the bound.

Fix: kill the reserve by its pid before the reap: `let _ = reserve.kill(); let _ = reserve.wait();`. The reserve is
this process's unreaped child, so its pid cannot name another process. `SIGKILL` also ends a stopped process, so the wait
is bounded. Correct the comment.

### Observation (not counted)

The `ci.rs` comment for the new `OFF_MACOS_EXCLUSIONS` entries does not say that they are temporary. The PR body says that
PR B replaces them with exclusions derived from `cfg`. If the comment says so too, the next reader does not copy the
hand-entry pattern.

### Carry to later P6 PRs

- PR B: the mutants nextest profile uses `--max-fail 1:immediate`, so a broken bound shows as a catch, not a TIMEOUT.
- PR B or PR C: `botster-test-process` has no `run_to_completion` helper yet (lead G2 ruling). PR C must add it, and
  `xtask/src/base_merge/tests.rs` `Repo::git` must use it.

VERDICT: NOT CLEAN (1 open: R1 LOW)
