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

### Note after round 1 (same head dc7fca5a) — R1 is a duplicate of TP1

The P6 package reviewer's TP1 covers the same call (`rounds.rs` `end_group`, line 153), and that error path too. R1 is
now a duplicate of TP1, which is theirs. This reviewer withdraws R1's proposed fix: a `wait` after `reserve.kill()` still
has no deadline (for example, a process in uninterruptible sleep). Follow TP1's fix: a kill, then a completion check that
is bounded by the deadline (`await_end` within the deadline, then the reap), on every path.

VERDICT: NOT CLEAN (R1 is a duplicate of TP1, the package reviewer's; no integration finding open of its own)

## Round 2 — NOT CLEAN on head 5d0d89c7

Reviewed head: `5d0d89c7d443146418117e5db991d3ea81b61c36`, a fast-forward from `dc7fca5a` (6 commits, 14 files, +1,229
-159). The base is still v1 `59ce1268`. v1 is now `a0f78fe4`, so a merge comes before the merge to v1.

### Checked, no finding

- **TP1 (R1's call; the package reviewer's finding).** `end_group` kills the reserve, then reaps it with `reap_within`:
  `await_status` within the deadline, then `try_wait`. No `Child::wait` is left in the crate. The decision loop
  `status_after_events` checks the status before the deadline, so an available status is never lost, and it has four
  default-tier tests with fakes.
- **`await_status`.** On Linux the event is the readable pidfd. On macOS it is `EVFILT_SIGNAL` for `SIGCHLD`. The filter
  is registered before the first check, so a `SIGCHLD` after the check wakes the wait. The comment cites the XNU sources
  for the order "`NOTE_EXIT`, zombie, `SIGCHLD`".
- **`start_time` returns `Result`.** An error is "not verified", never "gone". Only a proved end gives `None`.
- **The lint table and `libc`.** The crate copies the workspace table with only `unsafe_code = "deny"`. `lint_drift`
  proves that against the real manifests, and each other difference fails. `libc` is a dependency on every target, for
  `close` in `close_inherited`.
- **`ci.rs`.** The `OFF_MACOS_EXCLUSIONS` comment now says that the entries are temporary until PR B (round 1
  observation), and `await_status` is added. The focused Mac run at this head (`…014149-19171.log`): the slow suite
  passes, and the mutants of `platform/macos.rs` are 16 caught and 2 unviable.
- **The two `xtask` shell exclusions** (`unsafe_code.rs command`, `ci.rs taint_job`) exclude only the whole-body
  `Ok(())` replacements, and each names its decision functions (plan r22 section 8).
- **Blocking calls.** I grepped the added lines of the delta. The one new loop is `status_after_events`, which is bounded.
  The two `.status()` calls are `OwnedChild::status`, which is bounded by `CLEANUP`.
- **Evidence.** The full gate at this head is green (`…013207-11707.log`). The two TIMEOUTs of the slow-profile in-diff
  run (`…013552-15018.log`) are caught in 2 ms with `--max-fail 1:immediate` (`…014137-18902.log`), as in round 1.

### U1 MEDIUM — the `unsafe_code` check misses an attribute that spans lines

`xtask/src/unsafe_code.rs` `other_attributes` reports only a line that starts with `#[` or `#![` **and** contains
`unsafe_code`. rustfmt splits an attribute that is longer than the line limit:

    #[allow(
        clippy::too_many_lines,
        unsafe_code
    )]
    fn another() { unsafe { … } }

In `botster-test-process`, `unsafe_code` is `deny`, so this attribute compiles. No line starts with `#[` and also names
`unsafe_code`, so the check passes. The lead's ruling (TP3, as the crate's manifest comment says): "no other item allows
`unsafe_code`". In every other crate, `forbid` makes such an attribute a compile error. So the hole is in the one crate
that has the exception.

Fix: read each attribute until its closing `]`, not only its first line. Or report every occurrence of the identifier
`unsafe_code` in a Rust source, outside comments, except the one `#[allow(unsafe_code)]` of `close_inherited`. Add a
red test with the multi-line form. Also consider `rustflags` in `.cargo/config*.toml` (`-A unsafe_code` or
`--allow=unsafe_code`), which can lower `deny` for the crate without any attribute.

### E1 MEDIUM — no slow-tier mutation evidence for the new and changed slow-tier exclusions

Round 1 accepted the slow-tier whole-function exclusions because a slow-tier run with `--no-config` tested them
(`d2338cb6`, `…231333-45540.log`). This round adds new excluded functions and changes excluded ones:
- new: `reap_within`, `OwnedChild::exit_by`, `peek`, the Linux `await_status`, `start_anchor`, `close_inherited` and
  `group_of`;
- changed: `end_group`, `OwnedChild::end`, and the anchor binary's `wrap` and `anchor`.

No run at this head tests their mutants on Linux. The slow-profile in-diff run (`…013552-15018.log`) uses the config, so
its exclusions apply. Its log names none of these functions, except the macOS `await_status` (MISSED there, and covered
by the Mac run). The `d2338cb6` run predates all of them. Each entry names slow tests, but no mutation run shows that
those tests catch the mutants.

Fix: run the slow-tier mutation run with `--no-config` on this head, as at `d2338cb6` (`-p botster-test-process
--features slow`, nextest `--profile slow`, `--max-fail 1:immediate`, macOS file excluded), and post the log. Exclude
each miss with its reason, or kill it with a test.

### Carry (unchanged)

- PR B: the mutants profile with `--max-fail 1:immediate`.
- PR C: `run_to_completion`, and `xtask/src/base_merge/tests.rs` `Repo::git` uses it.

VERDICT: NOT CLEAN (2 open: U1 MEDIUM, E1 MEDIUM)
