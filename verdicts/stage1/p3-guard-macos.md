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
