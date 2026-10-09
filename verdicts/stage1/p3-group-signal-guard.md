# Integration review: #177 (P3 group-signal guard, branch stage1/p3-group-signal-guard)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head 9be81027

Reviewed head: `9be8102708a33c1a4c79693e09797ab6942c7c3c`, range `0b684a19..9be81027` (2 commits, 19 files, +461 -40).
The base is v1 `0b684a19470c5b647e64653fd285d4cd43544278`, which is the current v1. P3's gate log:
`botster-core-stage1-p3-group-signal-guard-9be81027-pool-20261009-033702-65783.log` (the reviewer did not read it).

### Checked, no finding

- **`signal::target`.** It refuses 0, 1, a value above `i32::MAX`, and the caller's own group or pid. Every other value
  passes unchanged. The unit test covers each boundary (1 and 2, `i32::MAX` and `i32::MAX + 1`, own).
- **The production call sites.** `process.rs` `kill_group` (Term, Kill, EndPayload) and `payload.rs` go through
  `signal_group` or `signal_process`. Their errors are ignored, as before. A refused target sends nothing.
- **The remaining raw calls.** At the head, only `signal.rs:58` and `:69` call `kill_process_group` or `kill_process`.
  The other rustix hits are `test_kill_process` and `test_kill_process_group` (signal 0, a probe that sends nothing).
- **The bans.** Each `clippy.toml` repeats the two entries, because clippy uses the nearest file only.
  `xtask/src/signal_bans.rs` checks every tracked `clippy.toml`, and it parses the TOML (no text match).
- **The testkit test** `a_corrupt_row_with_pid_1_never_signals_anything` is Sim-only and bounded (64 pumps for each of two
  instants). Note: its `GuardedSpawner` applies `target` itself, so it proves that the host signals only through the
  `Spawner` edge with pid 1. It does not run the real `Children::kill_group`. That path is covered by composition
  (`kill_group` calls `signal_group`, and `target` has its own test). This is acceptable.
- **The cross-PR rule with #171.** The xtask test `every_clippy_toml_of_the_repo_bans_the_raw_signal_calls` fails at
  the second merge until `crates/botster-test-process/clippy.toml` has the two entries. This is a correct tripwire. But see
  G1: the switch of #171's calls is not mechanical.

### G1 MEDIUM — a guard can no longer signal its own group on purpose

`signal_group` refuses `pgid == getpgrp()`. Two callers signal their own group on purpose:

1. `crates/botster-core-sys/tests/common/guard_cleanup.rs:43` (`end_group`, changed by this PR). `end_group` runs in the
   group that it ends (`payload_guard.rs:329` passes `getpgrp()`; `process_guard.rs:136` joins `group` with `setpgid`
   before its call at :145). When the reserve fails (the spawn of `/usr/bin/true` fails, for example `EAGAIN` under a
   process limit, or `setpgid` fails), the old code sent `KILL` to its own group as the last resort. Its comment says:
   "Without it, the one kill left also ends this process, after the report." At the head, `signal_group` returns
   `Err(Own)`, the `let _ =` drops it, and no member is killed. The guard reports and exits, and every member of the group
   is left running. This is the leak that the guard exists to prevent, and it occurs
   under load, which is when a spawn fails.
2. #171's anchor (`f04c240d`, `src/bin/botster-test-anchor.rs:193`, `terminate`): "The anchor is a member, so the group
   id is held while it signals". It sends `TERM` to its own group. Its fallback at `src/rounds.rs:141` is the same last
   resort as item 1. If the PR that lands second switches these calls to `botster_core_sys::signal::signal_group`, as
   P3's READY message says, `terminate` returns `InvalidInput` through `?` and the `TERM` phase stops working.

The refusal of our own group is right for a number that comes from a record. A deliberate signal to our own group does not
take a number. Fix: add `signal_own_group(signal)` to `signal.rs`. It reads `getpgrp()` itself, so no record can supply the
target, and it still refuses group 1 (a process in group 1 must not signal it). Use it at `guard_cleanup.rs:43` and state
it as the replacement for #171's own-group calls. Add a test: a child in a new group calls `signal_own_group` and ends,
and its parent stays alive. Also add a test of `end_group`'s reserve-failure path, or state why none is possible.

### S1 LOW — `xtask test_budget` signals through `/bin/kill`, outside the rule

`xtask/src/test_budget.rs:140` `kill` runs `kill -s KILL -- <args>`. `clean_up` (:314) passes pids parsed from a `ps`
snapshot and `-<group>`. The run's group is a fresh `process_group(0)` child, so it cannot be 0, 1 or ours. But the
leftover pids come from parsed `ps` output, which is a record, and nothing refuses 1 or our own pid there. The new root
comment says that "no crate is exempt from the pattern rule", and the clippy ban cannot see a shell command. Fix: route
`kill` through `botster_core_sys::signal` (xtask may depend on the sys crate), or apply `signal::target` to every
argument before the command runs. Add a test that a pid of 1 or our own pid is never passed.

VERDICT: NOT CLEAN (2 open: G1 MEDIUM; S1 LOW)
