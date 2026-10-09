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
| `platform` | the live members of a group, the wait for an exit, the wait for a child's status (`await_status`), the start time (libproc and kqueue on macOS, /proc and pidfd on Linux) | observe only; a failed read is an error, never "gone" |

### The reserve (amended anchor item 10, lead ruling 2026-10-08)

One kill of a group is not enough: on macOS a child whose fork completes after the kill escapes it and stays in the group.
So the kill repeats, in rounds, until no live member is left. A repeated kill must never reach another group that reused the
id. The killer keeps one child, the reserve, in the group, then leaves the group and signals it from outside: while the
reserve is unreaped, the group exists and its id cannot be reused (POSIX, "Process Group Lifetime"). The killer reaps only
its reserve, by that exact pid. Production keeps every reaping of its own children (ruling item 11):
`the_guard_ends_the_group_and_production_still_reaps_its_own_child` proves it.

No group signal goes to group 1: a signal to the group of pid 1 is `kill(-1, ...)`, which reaches every process that the
caller may signal. Every group signal goes through `botster_core_sys::signal`: `signal_group` (another group) and
`signal_own_group` (the group of the caller) refuse a group of 0 or 1 (#177). In the #171 round 2
mutation run, the mutant `OwnedChild::id -> 1` put a test's production process into group 1, and the cleanup then ended
every process of the gate's container (exit 137).

### Every wait after an exit is bounded (#171 TP1)

An exit event does not prove that the status is available: XNU's `proc_exit` posts `NOTE_EXIT`, then makes the child a
zombie, then sends `SIGCHLD` to the parent (`bsd/kern/kern_exit.c`). So no owner reads a status or reaps with a blocking
wait. `platform::await_status` loops: a non-reaping, non-blocking check of the exact pid (`waitid` with `NOWAIT | NOHANG`),
the deadline check, then a block on one real event, at most until the deadline. The event is a readable pidfd on Linux
(readable once the child is a zombie) and a kqueue `EVFILT_SIGNAL` for `SIGCHLD` on macOS. The filter installs no
handler, and kqueue records the signal even when its action is the default (`bsd/kern/kern_sig.c` posts the event before
the ignore check). The reap that follows is `try_wait`, by the exact pid, which cannot block. `OwnedChild`, the reserve of
`end_group`, and the wrapper's intermediate all use it.

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

The wrapper owns its intermediate as an `OwnedChild` (`anchor::start_anchor`, #171 TP4): on every error path the
intermediate is killed and reaped within the cleanup bound, so a stalled start leaves no orphan.

The anchor verifies the leader's identity and group, and its own membership, when its connection ends. After the `TERM`
grace, and before any `KILL`, it verifies them again, and it checks that every member that `TERM` reached is still in the
group or gone (#171 TP6, `anchor::stayed`). A member that moved, or an identity or group that cannot be read (#171 TP2,
`anchor::verdict`), refuses: the anchor signals nothing more and reports the refusal.

### Descriptor isolation and the one `unsafe` function (lead ruling 2026-10-09, #171 TP3)

Production's descriptors that lack close-on-exec reach the wrapper on purpose: the real program needs them, so the wrapper
keeps them. The helper stages must not keep them: the anchor lives for the whole test, and a pipe writer that it held
would keep production's reader from its end of file. So the first act of the intermediate and of the anchor is
`close_inherited`: it lists `/proc/self/fd` (Linux) or `/dev/fd` (macOS) and closes every descriptor above 2 but the
listing's own, which it closes last. EBADF for a listed descriptor is ignored; any other error fails the stage.

Closing a raw inherited descriptor has no safe API (std has none; rustix's `close` is `unsafe`). The lead chose (A): this
crate's lint table is the workspace's with one difference, `unsafe_code = "deny"` in place of `"forbid"`, and one
function, `close_inherited` in the anchor binary, allows it, with a SAFETY comment: the stage owns no descriptor yet, so
nothing in the process aliases the closed ones. Rejected: (B) nix 0.29's safe `close(RawFd)`, which hides the same
`unsafe` in a new dependency; (C) a shell stage that closes descriptors by redirection, which is fragile (dash handles only
0 to 9). `cargo xtask ci` (taint job, `xtask/src/unsafe_exception.rs`) fails on any other lint difference, on any other
`unsafe_code` identifier in a Rust source, and on any other key or string that names the lint in a manifest or a Cargo
configuration (`rustflags` included). The check parses: it reads each source as Rust tokens (an attribute on several lines,
a `cfg_attr` or a macro body is read as a whole; comments and strings are not identifiers) and each manifest as TOML (an
escaped key is read as Cargo reads it), and it reads `-` as `_` in a lint name, as rustc does (#171 round 2, TP7). The
libghostty-vt binding keeps its earlier crate-wide allow, by name (#173 tracks its SAFETY comments and clippy `undocumented_unsafe_blocks`).

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
| old botster-core `bounded_wait.rs` of the old test-support crate @ 72b2e335 | IDEA ONLY | One shared deadline, each step blocking on a real event. Not copied: `wait_for` repeats a step that returns at once, which spins. |
| old botster-core `runtime/plugin_process/supervisor.rs` @ 72b2e335 | IDEA ONLY | It serializes the group kill with the leader's reap so that a kill never reaches a reused id: the same invariant as the reserve. Production mechanism; nothing copied. |
| old botster-core `runtime/plugin_process/launch.rs` @ 72b2e335 | NOT APPLICABLE | Production launch (descriptor placement, rlimits, `pre_exec`): no test-process mechanism. |

Hand-rolled, with reasons: the anchor stages and protocol (no library holds a group for a process that another owner
reaps); the rounds (no library handles the macOS fork race with a reserved id); the `Blocker` (a FIFO and `/bin/cat`, by the
lead's design).
