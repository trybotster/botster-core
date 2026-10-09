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

### Note after round 1 (same head 9be81027) — the P3 package reviewer's F54 (theirs), missed here

The P3 package reviewer's F54 MEDIUM (verdict `a68fa489`) is real. This reviewer checked the ban entries but not the
allowances. `#![allow(clippy::disallowed_methods)]` silences the whole list, the signal entries too. At the head, 13 files
carry such an allow (for example `crates/botster-core/tests/slow_real_core.rs:8`, file-wide, for the clock), and CI clippy
does not build the `slow` feature. So a raw `kill_process` call can come back in those files with no lint failure. F54 is
theirs to close. This reviewer will check its closure in the next round with G1 and S1.

VERDICT: NOT CLEAN (2 open here: G1 MEDIUM; S1 LOW. F54 is the P3 package reviewer's.)

## Round 2 — NOT CLEAN on head f16eed6f

Reviewed head: `f16eed6fb50da9581060c06acc6f2dded1d61ca0`. The base is v1 `fc23cd9747e64e24f99565543b45ae5f7400a71c` (#171
merged). Delta on `9be81027`: `e3eb925b` (G1), `9ea5574a` and `400727e4` (the `signals` scan, F54; S1), the merge
`75482fa1` of v1 `fc23cd97`, and `f16eed6f` (#171's crate goes through the guard). P3's gate log:
`…-f16eed6f-pool-20261009-040121-96622.log` (44 mutants: 41 caught, 0 missed, 0 timeout, 3 unviable). This reviewer did
not read it.

- **The merge `75482fa1`.** `git merge-tree --write-tree 400727e4 fc23cd97` conflicts only in `xtask/src/ci.rs`. The
  real merge differs from the trial tree only in that file. The resolution keeps both sides: the `use` line has `signals`
  and `unsafe_exception`, and `taint_job` runs taint, timers, `unsafe_exception` and `signals`.
- **G1 CLOSED.** `signal_own_group(signal)` takes no target, so no record can give it one. It refuses our own group 0 or
  1 (`own_group`, tested), and it sends with `kill_current_process_group`, which is now banned outside `signal.rs`. The
  users are `guard_cleanup.rs` `end_group`'s reserve-failure path, the anchor's `terminate`, and `rounds::end_group`'s
  reserve-failure path. Each of them is in that group when it signals: the reserve failed before `setpgid(None, None)`,
  and the anchor calls `verify` (the anchor is still a member of `report.group`) before `terminate`. `reserved_kill` signals
  from outside the group, through `signal_group`. `a_signal_to_our_own_group_reaches_this_process` proves the real call
  with `SIGURG`, which the other members ignore by default.
  - P3 wrote no child-in-a-new-group test and no reserve-failure test, because plan r22 defers new real-process test code
    until P6's PR B. This reviewer accepts that reason. **Carry to PR B:** a test of the reserve-failure path of both
    `end_group` copies.
- **S1 CLOSED.** `test_budget::kill` no longer runs a program. Each argument goes through `target` (tested: a pid, `-N`,
  and junk) and then through `signal_process` or `signal_group`, which refuse 0, 1 and our own. The `kill -> ()` exclusion
  states its reason, and it names the tests of the decisions inside the body.
- **F54 (the P3 package reviewer's).** `cargo xtask signals` reads `proc-macro2` tokens. So an `#[allow]`, a `cfg` or a
  feature hides nothing. The scan runs in the `taint` job. The clippy job now uses `--all-features`. The tests cover a path,
  an import, a renamed import, a glob, a raw identifier, comments, strings and longer names. F54's closure is the package
  reviewer's to rule on.
- **`clippy_job -> Ok(())` exclusion.** The body has no decision other than its `?`, the same as the accepted
  `mutants_job` entry.

### L1 LOW — the scan and the bans do not see a `libc` signal call

`signals.rs` matches only the three rustix names, and the `clippy.toml` entries ban only them. But `libc` is a dependency
of four crates: `botster-core-sys` (macOS dev), `botster-core`, `botster-test-process` and `botster-worker`. So
`libc::kill(-1, SIGKILL)` or `libc::killpg(1, SIGKILL)` passes both checks. That is the same `kill(-1)` hazard that this
PR exists to stop, and `botster-test-process` already calls `libc` directly (`close`). Fix:
- Add `libc::kill` and `libc::killpg` to every `clippy.toml` list (with `allow-invalid`).
- In the scan, add `killpg` to `RAW_CALLS`, and match `kill` only as the path `libc :: kill` (a bare `kill` is
  `Child::kill` and `test_budget::kill`).
- Add both cases to `a_raw_signal_call_outside_the_guard_is_a_violation_under_any_allow_or_cfg`.

Observation (not counted): the scan cannot see `Command::new("sh")` with a `kill` script, or a `Command` under another
name. A shell script is outside the Rust scan. This reviewer does not ask for more.

VERDICT: NOT CLEAN (1 open: L1 LOW)

### Note after round 2 (same head f16eed6f) — the P3 package reviewer's F55 (theirs), missed here

The P3 package reviewer's F55 MEDIUM is real. `signal.rs` `a_signal_to_our_own_group_reaches_this_process` polls an
`AtomicBool` with up to 1,000,000 `yield_now` calls. BUILD.md testing rule 5 says: "No sleeps or polling. Tests wait on the
real event". A loop bound is not a marked deadline timer. This reviewer read that loop and missed it. A possible fix is to
wait on a real event: `signal_hook::low_level::pipe::register(SIGURG, write_end)`, then one read of the read end, bounded
by a marked deadline. F55 is theirs to close. Their scope note on `kill_program` (a renamed `Command`, escaped literals)
matches this reviewer's round 2 observation.

VERDICT: NOT CLEAN (1 open here: L1 LOW. F55 is the P3 package reviewer's.)

### Lead ruling after round 2 (same head f16eed6f) — G1 REOPENED

The lead ruled that the reserve-failure behavior proof does not carry to P6's PR B. Plan r22: production code whose proof
is a real-process test does not merge before that proof exists. Under the crate exception, the PR B HOLD does not block a
test in `botster-test-process`'s own suite. So G1 is open again until #177 contains a passing proof there of
`rounds::end_group`'s reserve-failure path (the last `KILL` through `signal_own_group` ends every member). This reviewer's
round 2 acceptance of the carry is withdrawn.

VERDICT: NOT CLEAN (2 open here: G1 MEDIUM, the proof only; L1 LOW. F55 and F56 are the P3 package reviewer's.)

## Round 3 — NOT CLEAN on head 05c70358

Reviewed head: `05c70358eab8f1a8e1ca77280a68b135dc2f990f`, one commit on `f16eed6f` (16 files, +242 -54). The base is still
v1 `fc23cd97`. P3's gate log: `…-05c70358-pool-20261009-042155-67173.log` (69 mutants: 66 caught, 0 missed, 0 timeout, 3
unviable). This reviewer did not read it.

- **G1 CLOSED (the proof, per the lead's ruling).** `rounds::end_group` is now `end_group_reserved(group, deadline,
  reserve)`, and `reserve` is the old code, unchanged. `slow_process.rs`
  `an_unreserved_cleanup_ends_every_member_by_its_last_kill` runs a helper that leads its own group (`spawn_group`). The
  helper holds a `/bin/cat` member that reads the test's held stdin, so only a signal can end the member. The helper then
  calls `end_group_reserved` with a reserve that fails.
  - The stderr EOF proves that the helper and its member both ended.
  - The helper's `KILL` status proves that the last kill ended it.
  - Red on revert: a refused kill lets the helper print, drop its member, and exit 0.
  - P6 accepted the proof (P3's READY message).
  - The two new `.cargo/mutants.toml` entries (`end_group_reserved`, `reserve`) cover the body of the old `end_group`, which
    the #171 E1 `--no-config` run covered. The reserve-failure branch has no operator to mutate, and the
    `end_group_reserved -> Ok(())` mutant fails the new test. `reserve`'s body replacement is unviable (`Child` has no
    default). This reviewer accepts the entries.
- **L1 mostly CLOSED.** Every `clippy.toml` list bans `libc::kill` and `libc::killpg`, and `signal_bans` checks for both. The
  scan flags `killpg`, the path `libc :: kill`, and `kill` in a `libc :: { … }` import list. `Child::kill`,
  `test_budget::kill` and `libc::killer` are not flagged. The fixture has each case. See L2 for the rest.
- **F55 and F56 (the P3 package reviewer's).** The signal test now waits for one byte from a `signal_hook` pipe, under a
  marked 10 s read deadline. The scan decodes escapes with `syn::LitStr`, and it follows `Command as X` and `type X =
  …Command…`. Both are the package reviewer's to close.

### L2 LOW — a glob import of libc hides `kill` from the scan

`libc_kill` matches `libc :: kill` and `libc :: { … kill … }` only. After `use libc::*;`, a bare `kill(-1, SIGKILL)` is
neither form, and `kill` is not in `RAW_CALLS` (by design, because a bare `kill` is also `Child::kill`). In a file with
`#![allow(clippy::disallowed_methods)]`, the clippy ban cannot see it either, and that is the case the scan exists for. For
rustix, the round 1 fixture already covers a glob (`use rustix::process::{…, *}` with a bare call). At the head, no file
glob-imports libc, so this is a gap in the tripwire, not a live call. Fix: flag a glob import of libc (`libc :: *`, and `*`
inside a `libc :: { … }` list) as a violation outside the guard, and add it to the fixture.

Observation (not counted): `helper_unreserved_cleanup` relies on `spawn_group` to make the helper a group leader before it
sends `KILL` to its own group. A one-line assert (`getpgrp() == getpid()`) before the call would keep a future defect in
`spawn_group` from sending that `KILL` to the test runner's group.

VERDICT: NOT CLEAN (1 open: L2 LOW. F55 and F56 are the P3 package reviewer's.)

### Note after round 3 (same head 05c70358) — L1 is open again (alias case found by the P3 package reviewer)

The P3 package reviewer showed that `use libc as sys; unsafe { sys::kill(-1, libc::SIGKILL); }` passes the scan, because
`libc_kill` matches only the literal crate name `libc`. Under `#![allow(clippy::disallowed_methods)]`, clippy does not see
it either. This reviewer closed L1 in round 3 without checking a crate rename, so L1 is open again. L2 (the glob) is the same
gap in another form. One fix closes both: collect the names under which a file can reach libc (`libc`, each
`libc as X`, and `extern crate libc as X`), then flag `<name> :: kill`, `<name> :: { … kill … }` and a `<name> :: *` glob.
This is the method that `command_names` uses for `Command`, so the fixture can test it the same way.

VERDICT: NOT CLEAN (2 open: L1 LOW (crate rename), L2 LOW (glob). F56 is the P3 package reviewer's.)

### Lead decision after round 3 (same head 05c70358) — L1 and L2 no longer block #177

The lead decided the enforcement design:
- Clippy `disallowed-methods` is the authority for the signal bans, because it resolves names.
- Its one hole, an `allow(clippy::disallowed_methods)` attribute, is closed by two later changes:
  - an attribute ban in P6's syntax-aware xtask check;
  - a P3 follow-up PR that moves the 17 clock allowances onto one helper per crate. This reviewer reviews it.
- No further alias-naming findings are raised against #177's token scan. After both land, the scan shrinks to
  `Command::new` with kill program literals.

L1 (the crate rename) and L2 (the glob) are alias-naming findings against the token scan. So they are SUPERSEDED by this
decision and no longer block #177. P3's next head adds the alias resolution anyway. This reviewer checks only that it adds
no defect. **Carries:** P6's attribute ban, and P3's clock-helper PR. Both close the `allow` hole that L1 and L2 depended on.

VERDICT: NOT CLEAN at 05c70358 until the next head (0 integration findings open; L1 and L2 superseded)

## Round 4 — CLEAN on head 0e0becfb

Reviewed head: `0e0becfbe04a4236206fa56402b38d343c062581`, one commit on `05c70358` (`xtask/src/signals.rs` and one assert
in `slow_process.rs`). The base is v1 `fc23cd97`, which is the current v1. P3's gate log:
`…-0e0becfb-pool-20261009-044148-62086.log` (every CI step passes; 76 mutants: 73 caught, 0 missed, 0 timeout, 3
unviable). This reviewer did not read it.

- **The alias resolution adds no defect** (L1 and L2 are superseded, so this round checks only that):
  - `reaching` ends: each pass adds only names that are not yet known, and a file has a finite number of identifiers.
  - `libc_kill` matches `<name> :: kill`, `<name> :: *`, and `kill` or `*` in a `<name> :: { … }` list. `libc::SIGKILL * 2`
    and `libc::[*]` are not flagged (fixture).
  - `the_repo_has_no_raw_signal_call` still runs over the whole tree, so the scan gives no false positive at this head.
- **The helper assert.** `helper_unreserved_cleanup` asserts `getpgrp() == getpid()` before the `KILL` to its own group.
  If the assert fails, the helper panics, the drop of its member ends the `cat`, and the test fails on the status (not
  `KILL`). So a `spawn_group` defect cannot send the `KILL` to the runner's group.
- Earlier rounds stand: G1 (the source fix and the proof), S1, and the merge of v1 `fc23cd97`.
- **Carries (the lead's decision):** P6's ban of `allow(clippy::disallowed_methods)` in the syntax-aware xtask check, and
  P3's follow-up PR that moves the 17 clock allowances onto one helper per crate (this reviewer reviews it). After both
  land, the token scan shrinks to `Command::new` with kill program literals.

VERDICT: CLEAN (0 open) at 0e0becfbe04a4236206fa56402b38d343c062581
