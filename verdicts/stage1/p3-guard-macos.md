# stage1/p3-guard-macos — the shared group guard: integration review

## Round 1 — Branch head before the PR opens

Reviewed head: `e0514aac36ba865fb4c59d4e631ae63a29a169ff`, branch `stage1/p3-guard-macos`. No PR number yet.
Base and merge base: current v1 `144b0234fb632bcbb5176b17c2fe55f3239405df`.
Reviewed delta: `144b023...e0514aa` (`c5ba07e`, `27ef700`, `e0514aa`), 8 files.
Owner: P3 by the lead's ruling. The guard is shared test code, compiled into the test binaries of botster-core-sys,
botster-core and botster-worker.
Source of the requirement: J1 of PR #162 (`verdicts/stage1/p5-a10.md`), and P5-F3 and P5-F4. The package reviewer's HIGH on
`c5ba07e` (a kill by a bare pid) is answered at `e0514aa`.
This reviewer ran no build, test or gate. The implementer's focused Mac proof of this head has not been reported yet.

### Accepted parts (J1)

- **The anchor stays in the group until the end.** `end_group` lists the live members other than the anchor, kills each one,
  and waits for each one's exit event. It repeats until a round lists no member. A zombie cannot fork, so an empty round is
  final. The payload guard's member uses the same `end_group(getpgrp())`. One code path serves both guards.
- **Identity, not the bare pid.**
  - Linux: each member is held by a pidfd. The pidfd is opened after the listing, and the member is checked again while the
    pidfd is live (state not `Z`, the same `pgrp`, not ended). The kill is `pidfd_send_signal`, and the wait polls the same
    pidfd. No reused pid can be signalled.
  - macOS: there is no pidfd. The kill re-reads `pbi_pgid`, the start time and the zombie state just before `kill`. The window
    left is recorded in the code: the member would have to end, be reaped and have its pid reused between that check and the
    signal. This reviewer accepts it as the platform's limit.
- **No polling.** The rounds wait on exit events (kqueue `NOTE_EXIT`, pidfd readability). The zero-timeout `poll` in `ended`
  is one readiness check, not a loop. Every wait is bounded by the marked `CLEANUP` deadline.
- **Tests.** `end_members` is a pure decision with injected listing, kill, wait and expiry. Its tests use scripted listings:
  a member that joins after the first kill ends in a later round, each kill waits for that member, and the rounds stop at the
  deadline. The expected values come from the script. The real proof is `parent_dies_before_fifo_reader` on the Mac.
- **Dependencies.** `libproc 0.14` and `kqueue 1.2.1` are already production dependencies of botster-core-sys on macOS.
  `libc 0.2` is added as a macOS dev-dependency for `SZOMB`. The PR's Prior art note must name all three.
- **Merges.** `git merge-tree` of `e0514aa` with PR #163 (`0110caa`) and with PR #164 (`c4afe58`) has no textual conflict.

### Findings

#### G1 [MEDIUM] OPEN — A cleanup that runs out of time is silent

- Location: `process_guard.rs` `end_group` and `end_members` (they return `()`); `anchor_process`, which ends after
  `end_group`; `GroupGuard::drop`, which waits for the anchor and ignores its exit status.
- Evidence: when `expired()` is true while members are still listed, `end_members` returns. The anchor then exits like a
  successful cleanup, and the guard cannot tell the two apart. The case is real: the earlier macOS hang was a leader stuck in
  exit (`?E`). Such a member stays listed in every round until the deadline.
- Why: J1 required that "a member that cannot be ended is a reported failure". BUILD.md testing rule 10, and the xtask's
  leftover check, need a clear cause, not a later leftover-process failure with no name.
- Required:
  1. `end_members` returns whether every member ended, or the members that remain.
  2. The anchor reports the remaining members (pid, state) on its stderr and exits with a failure status.
  3. `GroupGuard::drop` checks the anchor's status. It fails the test, or it reports the failure when the test already panics.
  4. Add one more scripted `end_members` test for the expired outcome.

#### G2 [LOW] OPEN — After the merge with PR #163, the outer cleanup deadline equals the anchor's deadline

- Location: PR #163's `GroupGuard::drop` bounds `anchor.wait()` with `bounded(...)` at 10 s. This branch's `CLEANUP` is also
  10 s (merged tree `0519469`, `process_guard.rs:109` and `:140`).
- Evidence: the anchor's legitimate cleanup can take up to 10 s, so the outer wait can expire first. Then the test fails at
  the outer wait, not with the anchor's own report (G1). The package reviewer named the same equal-deadline problem as F27 on #163.
- Required: make the outer wait longer than the anchor's `CLEANUP` (for example, both derived from one constant plus a
  margin), in whichever PR merges second. State that merge step in the PR.

VERDICT: NOT CLEAN (2 open)

## Round 2 — Branch head 81ccd17 (a new design)

Reviewed head: `81ccd175806a71a11b18b79694177207256e9f33`. It supersedes `e0514aa`. Delta `e0514aa..81ccd17`, one commit,
`process_guard.rs`. No PR number yet. This reviewer ran no build, test or gate.

### The new design, accepted in principle

- At cleanup, the anchor spawns `/usr/bin/true` as a reserve. The reserve inherits the guarded group, and the anchor never
  reaps it. The anchor then moves to a group of its own and sends group kills from outside. Every kill is `killpg`, never a
  bare pid, so the round 1 pid-reuse window is gone on both platforms.
- The unreaped reserve keeps a member in the group, even as a zombie. Neither kernel gives a new process a pid that is an
  existing group id. So no other group can take the id before the anchor reaps the reserve at the end. This design is
  simpler than per-pid kills, and a better fit for the problem.
- `/usr/bin/true` is an external command, so it forks nothing more and exits at once.

### Findings

#### G3 [MEDIUM] OPEN — A kill comes before the listing, so a member that escaped it waits until the deadline

- Location: `end_members`: `loop { kill(); let live = members(); if live.is_empty() || expired() { return } for m in &live { await_end(m) } }`.
- Evidence: the macOS race is a child whose fork completes after a `killpg` enumerated the group. That child is live and in the
  group, so the listing right after the kill finds it. Then `await_end` waits for its exit, but nothing has killed it, so the
  wait lasts until `CLEANUP` (10 s). Only the next round's kill ends it. Each escape costs the whole deadline, and with G2 the
  outer guard wait expires first.
- The scripted tests cannot see this: their `await_end` returns at once whether or not a kill came after the listing.
- Required:
  1. Order each round as list, kill, wait: every listed member is then hit by a kill that came after its listing, because
     its fork had completed. A child forked after that kill is in the next round's list.
  2. Give the decision test a model where a listed member ends only by a kill that comes after its listing. In that model the
     current order must fail and the fixed order must pass.
  3. Keep the real Mac proof, `parent_dies_before_fifo_reader`.

#### G4 [MEDIUM] OPEN — A failed wait setup turns the rounds into a busy loop until the deadline

- Location: `await_end` on both platforms returns at once when `kqueue::Watcher::new`, `add_pid`, `watch` or `pidfd_open`
  fails. `live_members` returns an empty list when the listing fails.
- Evidence: a member that stays listed but cannot be watched (for example, a pid that ended between the listing and
  `add_pid`, while another member stays stuck in exit) makes each round kill, list and return at once. So the anchor loops
  with no wait until `CLEANUP`.
- A failed listing (`pids_by_type` or `/proc` read error) reads as "no live member". That is a false success.
- Why: BUILD.md has no busy-spinning children, and A11 and G1 require that failures be visible.
- Required:
  - A wait whose setup fails is a reported cleanup failure. It does not mean "ended", and the rounds do not retry it hot.
    One allowed form: treat a pid that is gone at `add_pid` or `pidfd_open` (`ESRCH`) as ended, and any other error as a failure.
  - A listing error is a failure, not an empty group.

#### G1 [MEDIUM] OPEN — carried from round 1, extended

- The anchor still ignores the outcome: `end_members` returns `()`, the anchor exits normally, and `GroupGuard::drop` does not
  check its status. The failures of G4 are hidden in the same way.
- Required as in round 1. `end_members` returns the outcome (the remaining members, or the failure). The anchor reports it on
  stderr and exits with a failure. `Drop` checks the status. A scripted test covers the expired outcome and the failure
  outcome.

#### G2 [LOW] OPEN — carried from round 1

- The outer 10 s wait for the anchor in #163's `Drop` equals `CLEANUP`. It is resolved in the merged tree, as in round 1.

The package reviewer also requires proof by behavior on owned processes, under the user's test-quality rule. G3 item 2 is the
integration side of that requirement. The package reviewer owns the rest.

VERDICT: NOT CLEAN (4 open: G1, G2, G3, G4)

## Round 3 — PR #165, head 3670202

Reviewed head: `36702023b8a36876ad226ac61adb63723511f28e`, PR #165, base v1 `144b023`. Delta `81ccd17..3670202`, one commit,
`process_guard.rs` and `payload_guard.rs`. This reviewer ran no build, test or gate. It read the implementer's Mac log.

- **G3 CLOSED.** Each round now lists, kills, then waits. `ForkRace` models the kernel: a kill ends the members that are in the
  group at that moment, and a fork that was in progress joins just after the first kill. Its wait asserts that the member
  ended by a kill that came after its listing. Under the old order (kill, list, wait), member 2 is waited for with no kill,
  so the old order fails the test and the new one passes. The test checks behavior. It is not a call count.
- **G4 CLOSED.** A listing error and a wait-setup error are `Failure::Error`. `ESRCH` means ended: `watch()` on macOS,
  `pidfd_open` on Linux. A process that vanished during the listing is skipped, because it is not live. A Linux `poll` that
  returns at its timeout leads to the deadline check, so no setup error retries hot.
- **G1 CLOSED.** `end_members` returns `Left(members)` or `Error(e)`, and `end_group` turns that into a report. The anchor
  writes the report to the control stream and exits 1. `GroupGuard::drop` checks the status, reads the report, and panics,
  or prints it when the test already panics. Scripted tests cover both failure outcomes. The payload guard's member reports
  the same way, but its stderr is `/dev/null` through the prefix. The PR states this. Each payload test's own EOF check and
  bounded cleanup observe that cleanup, so this reviewer accepts the stated limit.
- **G2** stays open for the merged tree. This PR keeps v1's direct `anchor.wait()`, with no outer deadline. The implementer
  will make #163's bounded anchor wait `2 × CLEANUP`, from the one constant, when #163 merges this guard. G2 closes in #163's
  delta review.
- **Proof.** Log `…guard-macos-36702023-mac-20261004-215726-95373.log`: a `botster-gate --on mac` run at this exact head.
  - Clippy with `slow` passes on the three crates.
  - `prebuild-worker` passes.
  - 132 slow tests pass in one run, and 9 in the other. They include `parent_dies_before_fifo_reader` in each binary that
    includes the guard, and the new `end_members` tests.
  - Exit 0.
- **Prior art.** The PR names `libproc` 0.14, `kqueue` 1.2.1 and `libc` 0.2.

#### G5 [LOW] OPEN — A member that the wait calls gone, but the listing still calls live, makes the rounds repeat with no wait

- Location: macOS `await_end` (`watch()` gives `ESRCH`, so `Ok`) and `live_members` (`pbi_status != SZOMB`).
- Evidence: the earlier Mac hang was a leader stuck in its exit (`?E`). If XNU refuses `EVFILT_PROC` on a process that is in
  exit but not yet a zombie, then each round lists it, kills it, waits for nothing (`ESRCH`, so `Ok`), and lists it again.
  That spins until `CLEANUP`, and only then reports `Left`. The report is correct, but the CPU is busy for 10 s. This
  reviewer has not proved that XNU behaves this way. The rule below does not depend on it.
- Required: no round repeats without a blocking wait, or a change in the live set. For example, keep the members that a
  wait called gone (pid and start time), and do not list them as live again; or end with `Left` when the same member is
  called gone twice. Add a scripted case in the `ForkRace` style.

The lead's merge order puts this PR first, then #162, #163, #142 and #161.

VERDICT: NOT CLEAN (2 open: G2 for the merged tree in #163; G5)

Note on G2: at this head, #165 alone has no outer wait. The equal deadlines appear only in the tree after #163 merges this
guard. So G2 moves to #163's review and does not block #165. Open for #165: G5 only.

VERDICT: NOT CLEAN (1 open: G5)

## Round 4 — PR #165, head acd05de6

Reviewed head: `acd05de6d14ca46c2e7cbfab3d9e379d6fa469a7`. Delta `3670202..acd05de6`, four commits, 5 files (the two guard
modules, `slow_payload.rs`, and the worker fixtures `driver_edges.rs` and `session.rs`). The base is still current v1
`144b023`. This reviewer ran no build, test or gate. The implementer reports a focused Mac run: 142 and 11 slow tests pass
(log `…guard-macos-acd05de6-mac-20261004-220807-29615.log`).

- **G5 CLOSED.** `await_end` returns `Exited`, `Gone` (`ESRCH` at setup) or `Deadline`. A member that waits call `Gone` in
  two rounds in a row, while the listings still call it live, ends the rounds with `Left([member])`. So the rounds repeat at
  most once with no blocking wait. The scripted test `a_member_gone_to_its_wait_but_still_listed_is_left` covers it.
- Other changes answer the package reviewer's F35 to F38, which are its scope:
  - a failed kill is a failure;
  - at the deadline there is one last kill, then `Left`;
  - each kill first checks that the reserve is still an unreaped child (`waitid` with `NOWAIT | NOHANG`);
  - the payload member's report reaches its guard;
  - a real guard-path failure test.
  The worker fixtures change only to call the payload guard's new release and report check. No production code and no
  interface between packages changes.

#### G6 [LOW] OPEN — A member that exits between the listing and the kill can turn a correct cleanup into a reported failure

- Location: `end_group`'s `kill` closure (`kill_process_group(...).map_err(Into::into)`) and `end_members`
  (`kill().map_err(Failure::Error)?`).
- Evidence: a round kills only after a listing has found a live member. Suppose that member exits by itself after the listing
  and before `killpg`, for example a payload that ends at cleanup. Then the group can hold only zombies: the reserve and
  members that are not reaped. Linux delivers a group signal to zombie members without error. BSD-derived kernels skip
  zombies when they look for a target and return `ESRCH` when none is found. This reviewer has not proved that on the
  pinned macOS. If XNU does this, the round ends with `Failure::Error` and the guard fails a test whose cleanup was correct.
  The reservation check has already proved that the group id is still held at that moment.
- Required: when the reservation check passes, treat `ESRCH` from `killpg` as "no member left to signal". Let the next
  listing decide; an empty listing means success. Any other `killpg` error stays a failure. Add one scripted
  `end_members` case: the kill reports `ESRCH` and the next listing is empty, so the result is `Ok`.

G2 stays with #163's merge delta.

VERDICT: NOT CLEAN (1 open: G6)

## Round 5 — PR #165, head 97b8e947

Reviewed head: `97b8e94767e862b7349d8542306f91c9bed45bb0`. Delta `acd05de6..97b8e947`, one commit, `process_guard.rs`.
The base is still current v1 `144b023`. This reviewer ran no build, test or gate.

- **G6 CLOSED.** In `end_members`, a kill error that `gone()` classifies as `ESRCH` no longer stops the rounds. The next
  listing decides. Any other kill error is still `Failure::Error`.
  - A failed reservation check is built with `io::Error::other`, so it carries no errno and can never be taken as `ESRCH`.
    It stays a failure.
  - If a live member exists, `killpg` finds it, so a tolerated `ESRCH` cannot hide a member. The next listing would show it.
  - Scripted case `a_kill_that_finds_no_member_lets_the_next_listing_decide`: listings `[7]` then `[]`, the kill gives
    `ESRCH`, and the result is `Ok`.
- Implementer evidence: `~/botster-sessions/gates/botster-core-stage1-p3-guard-macos-97b8e947-mac-20261004-220951-33326.log`.
  It is a `botster-gate --on mac` focused run at this exact head. 147 and 12 tests pass, and it exits 0. This reviewer read
  its summary lines.

Integration findings on #165: none open. G2 is tracked in #163's merge delta. Still needed for CLEAN: the P3 package
verdict on `97b8e947` (F35 to F38).

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)

## Round 5 note — a correction to round 4 (head `97b8e947`)

- Round 4 said that the worker fixtures "change only to call the payload guard's new release and report check". That was
  incomplete. The package reviewer (on `acd05de6`) found that the botster-worker driver `Harness` never calls `release`:
  - it has no `Drop`;
  - it drops the `Driver` first and the guard later;
  - `pty_events_resume_reads_after_would_block` drops its guard before it gets the driver's result.
  So when production cleanup fails, the independent guard never gets its EOF request. The package reviewer raised this as a
  HIGH cleanup regression.
- This reviewer did not open a duplicate, because the package reviewer's finding requires the fix. CLEAN here also waits for
  it, because the payload guard is shared by botster-core-sys and botster-worker.
- The package reviewer also requires that the 795-line `process_guard.rs` be split into small shared modules with one
  cleanup path, under the user's rule against oversized modules. A split is a new head and gets an integration delta review.

VERDICT: NOT CLEAN (1 open: the package verdict, which now includes the Harness release HIGH and the module split; 0 separate integration findings open)

## Round 6 — PR #165, head f5f9d0f6

Reviewed head: `f5f9d0f64252d41803dcc1b53ca9b586bb90f5cb`. Delta `97b8e947..f5f9d0f6`, five commits, 6 files. The base is still
current v1 `144b023`. This reviewer ran no build, test or gate. The implementer reports a focused Mac run where 155 and 13
slow tests pass (log `…guard-macos-f5f9d0f6-mac-20261004-221717-52020.log`).

This round checks the cross-package parts, because the guard modules are compiled into the tests of three crates.
- **The module split.** `process_guard.rs` (375 lines) includes `guard_cleanup.rs` (291), which includes `guard_platform.rs`
  (215). `payload_guard.rs` is 353 lines. Each crate's `#[path]` includes still name only `process_guard.rs` and
  `payload_guard.rs`, so the inner modules arrive through them. Every user gets the same cleanup path: botster-core-sys
  `slow_payload` and `slow_process`, botster-core `tests/common`, and botster-worker `session.rs` and `slow_cli.rs`.
  `session.rs` now makes `process_guard` `pub(crate)`, so that `driver_edges.rs` can reach the one `CLEANUP` constant.
- **The worker harness order (the package reviewer's F40, which round 4 here missed).** `Harness` fields drop in this order:
  1. `release` sends the cleanup request first;
  2. `driver: Bounded<Driver>` runs production's drop on a helper thread under `CLEANUP`;
  3. the guard reads its member's report.
  This order holds on every exit path, panics included. In `pty_events_resume_reads_after_would_block`, the driver is taken
  out to run on a thread. The test drops `release` before `Remove`, and drops the guard only after the driver's result and
  the thread's join. So the two phases are in the right order.
- **No production code and no interface between packages changes.**

Integration findings on #165: none open. G2 stays with #163's merge: this PR keeps a direct `anchor.wait()` in
`GroupGuard::drop`, so the merged tree must make the outer wait longer than `CLEANUP`. The other changes (the payload
report handling, the proof that a macOS group is empty, the Linux pgrp check) are the package reviewer's scope.

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)

## Round 7 — PR #165, head 08fef89d

Reviewed head: `08fef89d22657db2b558e63d8da8435c9398765a`. Delta `f5f9d0f6..08fef89d`, two commits, 3 files
(`guard_cleanup.rs`, `payload_guard.rs`, botster-worker `driver_edges.rs`). The base is still current v1 `144b023`. This
reviewer ran no build, test or gate. The implementer's focused Mac run at this head
(`…guard-macos-08fef89d-mac-20261004-222304-71264.log`) shows 155 and 13 tests passing and exits 0. This reviewer read its
summary lines.

- The reservation test now observes the effects through exit statuses of owned processes. The members are `/bin/cat`, blocked
  in a FIFO open, with no CPU used.
  - In the held case, the member ends by `SIGKILL`.
  - In the released case, the kill is refused, and the member ends normally once it is let go.
  The expected values come from the test's own process setup. This is the package reviewer's F38.
- `serve()` in the payload guard keeps each registration error. It tells a cancelled launch apart from failed ownership, and
  it sends no readiness after a failure. This is the package reviewer's F37.
- The worker harness's `Bounded<Driver>` waits for `2 × CLEANUP`, so the outer wait covers the inner cleanup and its report,
  and the inner limit does not change. This settles the shape of G2 and F39 on this branch. When #163 merges this guard, it
  uses the same rule for its bounded anchor wait. G2 is checked again in that delta review.
- No production code and no interface between packages changes.

Integration findings on #165: none open. Still needed for CLEAN: the P3 package verdict on `08fef89d`.

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)

Round 7 correction: the package reviewer found that the new reservation test owns its `/bin/cat` members as raw `Child`
values, with no guard and no deadline on `held.wait`, `reserve.wait` or `alive.wait`. Suppose the kill does not reach a member,
or an assertion fails. Then a member can outlive the test, or the wait can block the job (BUILD.md testing rules 5 and 10).
This reviewer accepted the test without checking its process ownership. The package reviewer's HIGH requires the fix, so
no duplicate is opened. CLEAN here waits for it as part of the package verdict.

## Round 8 — PR #165, head 4c06846d

Reviewed head: `4c06846d039b9209cbd2d8d57144bbad807b6d98`. Delta `08fef89d..4c06846d`, three commits. The changes are test
code only, in `guard_cleanup.rs` and `payload_guard.rs`. The base is still current v1 `144b023`. This reviewer ran no build,
test or gate. The implementer's focused Mac run at this head (`…guard-macos-4c06846d-mac-20261004-222705-81386.log`) shows
158 and 13 tests passing and exits 0. This reviewer read its summary lines.

This round checks how each new test owns its processes and bounds its waits. That check was missed in round 7.
- **The reservation test (the package reviewer's F43).** Every child is an `Owned`.
  - `status()` observes the end with a `waitid` that uses `WNOWAIT` and runs on a helper thread, bounded by `CLEANUP`. Only
    after that does it reap.
  - `Drop` sends a kill to that child's own pid. The pid cannot be reused, because the child is still unreaped. `Drop` then
    observes the end with the same bound and reaps. It panics if the child does not end, or reports when the test already
    panics.
  - The FIFO writer that releases the `alive` member opens on a helper thread with the cleanup limit, so a member that is gone
    fails the test instead of blocking it.
  - The test reaps only its own fixture children and signals only its own pids. The assertions on the actual effects stay:
    `SIGKILL` when the group is held, and exit 0 when it is released.
- **The registration-error test (F37).** It drives the real `PayloadGuard` through two frames that cannot be trusted:
  `\x01not-a-pid` and a bare ready with no member. The guard's drop panics with a report that contains the expected
  cause. The registrant reads EOF, so no readiness was sent. Its read has a marked deadline, set while the stream is open.
  The expected texts are the guard's own words for each case. They are not terminal bytes.
- No production code and no interface between packages changes.

Integration findings on #165: none open. Still needed for CLEAN: the P3 package verdict on `4c06846d`.

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)

Round 8 note: the package reviewer keeps part of F43 open at this head, on two paths.
- `ended_within_cleanup` reports an end for any `waitid` error that is not `EINTR`.
- `Owned::drop` returns on any `try_wait` error and discards the kill and reap errors.
This reviewer's round 8 read the happy path only. The package finding requires the fix, so no duplicate is opened, and CLEAN
here waits for it.

## Round 9 — PR #165, head c03bcfb1

Reviewed head: `c03bcfb181d21cdf805b752a790359f63b9ef7b9`. Delta `4c06846d..c03bcfb1`, two commits, `guard_cleanup.rs` only,
test code. The base is still current v1 `144b023`. This reviewer ran no build, test or gate. The implementer's focused Mac run
at this head (`…guard-macos-c03bcfb1-mac-20261004-223014-89077.log`) shows 158 and 13 tests passing and exits 0. This
reviewer read its header and summary lines.

Every error branch of the changed helpers was read:
- `ended_within_cleanup` returns `io::Result<bool>`. An interrupted wait is repeated. `Ok(Some)` is an exit. `Ok(None)` from
  a blocking wait is an error, not an exit, so no spin is possible. Any other `waitid` error is returned. A timeout is
  `Ok(false)`, and the helper thread stays blocked, which leaves the child with its owner.
- `Owned::status` reaps only after an observed exit. It panics with a cause on a timeout or on an observation error.
- `Owned::drop`:
  - `Ok(Some)` means the child is reaped, so there is nothing left to do;
  - a `try_wait` error, a kill error, a timeout after the kill, an observation error and a reap error are each reported
    through `ownership_failed`, which panics, or prints when the test already panics;
  - no error is taken as an end.
- `end_members` now lets the next listing decide after a group kill that returns `EPERM`, as it already did for `ESRCH`. The
  next listing comes before any success, so a refused kill cannot hide a live member. A member that truly cannot be
  signalled stays listed. The rounds then block in `await_end` up to the deadline and end with `Left`, or `await_end` itself
  fails with `Failure::Error`. Either way the failure is reported, with no spin. The scripted case now runs both errnos.
  The macOS cause (a member in exit refuses the signal) is the implementer's claim and is not proved here. The rule is safe
  whatever the cause.

Integration findings on #165: none open. Still needed for CLEAN: the P3 package verdict on `c03bcfb1` (F43).

VERDICT: NOT CLEAN (1 open: package verdict pending; 0 integration findings open)
