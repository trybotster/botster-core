# botster-test-process: design and prior art

The one owner of real-process test code in botster-core (brief-p6-test-process, 2026-10-08). A dev-dependency only.

## Why it exists

Real-process test code was the most frequent source of Stage 1 defects: the cargo-mutants orphans (busy children that ran
for about 6 hours), the macOS guard regression, audit findings A7, A10 and A11 (polling, races with the production reaper,
unbounded waits), the #165/#162 gate deadlock, and #165's five review rounds that found the same wait shape file by file
(C3 to C7). Each crate had its own spawn and wait fixtures, and only reviewers enforced the rules. This crate holds the one
copy; `cargo xtask ci` checks that test code outside it does not wait for a child, read a child's pipe without a deadline,
or sleep (PR B).

## The parts

| Part | Job | Rule it enforces |
|---|---|---|
| `Deadline`, `CLEANUP` | the only timer: every wait blocks on a real event, at most until a deadline derived from a named limit | BUILD.md testing rule 5 |
| `OwnedChild` | a child that the test starts: bounded status; its drop kills and reaps it; group mode ends every member of its group, with the unreaped leader as the reserve | rule 10 |
| `Guard` + `botster-test-anchor` | the anchor of a process that production starts and reaps: the double-forked anchor holds the group; the guard's drop (or the test's death) makes it end the group | rule 10; lead rulings 2026-10-04 and 2026-10-08 |
| `rounds` | the cleanup decision: list, kill, await, until no live member; a reserve holds the group id | the macOS fork race; no stale group id |
| `Bounded`, `first_line`, `eof` | reads of pipes, FIFOs and sockets that `poll` bounds | no unbounded `read_line` |
| `Blocker` | the blocked fixture child: `/bin/cat` on a FIFO that the test holds | no sleep loop, no spin; ends when the test is gone |
| `platform` | the live members of a group, the wait for an exit, the start time (libproc and kqueue on macOS, /proc and pidfd on Linux) | observe only |

### The reserve (amended anchor item 10, lead ruling 2026-10-08)

One kill of a group is not enough: on macOS a child whose fork completes after the kill escapes it and stays in the group.
So the kill repeats, in rounds, until no live member is left. A repeated kill must never reach another group that reused the
id. The killer keeps one child, the reserve, in the group, then leaves the group and signals it from outside: while the
reserve is unreaped, the group exists and its id cannot be reused (POSIX, "Process Group Lifetime"). The killer reaps only
its reserve, by that exact pid. Production keeps every reaping of its own children (ruling item 11):
`the_guard_ends_the_group_and_production_still_reaps_its_own_child` proves it.

### One anchor for every guard

Production starts the process through a generated wrapper script (`Guard::wrapper`): `exec botster-test-anchor wrap
<socket> <grace> <cleanup> <program> <args> "$@"`. The wrapper double-forks the anchor into its own group and session, waits
for the anchor's acknowledgement, and execs the program: the pid, start time, group, session, environment and exit path
stay those that production watches. This replaces three earlier designs: the `GroupGuard` of the core-sys tests (an anchor
that joins the group with `setpgid` and a shell prefix), the `PayloadGuard` (a background member started by the payload's
shell), and the RealCoreHarness WIP anchor (`stage1/p6-real-harness`, config beside `argv[0]`). The wrapper takes its
configuration as arguments, so no environment variable reaches the program.

The guard is passive: a non-blocking listener and the accepted connections, handled by `poll` when the test waits for
anchors and at the drop. It starts no thread. An owner whose production must clean up after the anchors started calls
`release` first and drops the guard after production (on macOS a member's exit can wait in a terminal drain until production
closes the PTY master).

An anchor that ended with no report was ended by production's own group kill; the guard then observes, without a signal,
that the group is empty within the cleanup bound.

### Where the helpers run

The anchor is a prebuilt binary (`cargo xtask prebuild-worker`, in `target/candidate/` with the sha256 manifest, ruling item
1), not an entry of each test binary: the `#[test]` helper entries of the old guards compiled into 7 test binaries across 4
crates and ran their self-tests 7 times. The crate's own tests run once, here. Every real-process test is in the slow tier,
which already needs the prebuild; the default tier starts no real process.

## Prior art (BUILD.md rule 0)

| Item | Decision | Reason |
|---|---|---|
| `process-wrap` 9.1.1 (successor of `command-group`): group spawn and kill | REJECTED | Its `ProcessGroupChild::wait` reaps the leader, then calls `waitpid(-pgid)`, which reaps every child of the test process in that group: in a test, production's children (the A10 reaper race). It kills by the stored pgid after the leader may be reaped, so the id is not reserved. `Command::process_group(0)` is in std. |
| `wait-timeout` 0.2.1: bounded child waits | REJECTED | It installs a process-wide `SIGCHLD` handler (`sigaction`) in the test process, which also runs production code (the core-sys reaper threads block in `waitid`): a handler without `SA_RESTART` interrupts production's blocking calls. The pidfd (Linux) and kqueue `EVFILT_PROC` (macOS) waits observe one pid with a timeout, with no global state and no reap. |
| cargo-nextest `leak-timeout` (200 ms, `result = fail`) and `slow-timeout` | KEPT, not sufficient | `leak-timeout` sees only a child that holds the test's stdout or stderr; the fixtures' children use null or piped output, and the anchors outlive the test with null output. The slow-timeout kill reaches the test's process group, never the children that a guard put in their own groups. The gap is closed by the test-run wrapper (PR A2). |
| `.config/test-wrapper.sh` + the census in `cargo xtask test-budget` | KEPT, not sufficient | The cargo-mutants runs set no `BOTSTER_TEST_PIDFILE`, so no census runs after a mutant (the 6 h orphans). The census samples `ps` every 10 ms and misses a short-lived parent of a new group. PR A2: a Linux subreaper run-wrapper for every tier, mutants included (lead ruling 2026-10-08, item 12 extended). |
| `botster-core-sys/tests/common/{process_guard,guard_cleanup,guard_platform,payload_guard}.rs` (#165 in #162) | REUSED (moved; v1 code, no steal trailer) | The rounds (`end_members`, `reserved_kill`, `end_group`), their unit tests, the platform adapters and the owned child are the reviewed #165 design. Changed: deadlines are `Deadline` values; an observed exit is reaped by a blocking wait for its exact pid (macOS sends the exit event before a `WNOHANG` wait can reap it); the bounded reads use `poll`, not a thread per read. |
| `botster-core-testkit` `OwnedGroup` | REUSED as `OwnedChild::spawn_group` | Same idea (the unreaped leader keeps the group id); the drop now runs the rounds, so a member forked during the kill is ended too. |
| `stage1/p6-real-harness` `botster-test-anchor` (276427d, unreviewed) | REUSED (stages, signal list, verification, refusal) | Changed: arguments, not a config file beside `argv[0]`; the reserve and rounds replace the single final `KILL` (amended item 10). |
| old botster-core `botster-core-test-support/src/bounded_wait.rs` @ 72b2e335 | IDEA ONLY | One shared deadline, each step blocking on a real event. Not copied: `wait_for` repeats a step that returns at once, which spins. |
| old botster-core `runtime/plugin_process/supervisor.rs` @ 72b2e335 | IDEA ONLY | It serializes the group kill with the leader's reap so that a kill never reaches a reused id: the same invariant as the reserve. Production mechanism; nothing copied. |
| old botster-core `runtime/plugin_process/launch.rs` @ 72b2e335 | NOT APPLICABLE | Production launch (descriptor placement, rlimits, `pre_exec`): no test-process mechanism. |

Hand-rolled, with reasons: the anchor stages and protocol (no library holds a group for a process that another owner
reaps); the rounds (no library handles the macOS fork race with a reserved id); the `Blocker` (a FIFO and `/bin/cat`, by the
lead's design).
