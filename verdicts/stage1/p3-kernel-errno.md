# Integration review: #186 (the pidfd wait test follows each documented kernel answer; branch of P3)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head 1bfc6fd3 (the ENOENT branch has no run yet)

Reviewed head: `1bfc6fd39fa077bec92b7fe021d5d7c9f6798618`, one commit on v1 `aaac0c0d` (the current v1), one file:
`crates/botster-core-sys/tests/common/guard_platform.rs` (+26 -14). The stated tier is HIGH, which is correct by rule 3: the
file is shared test code, included in test binaries of `botster-core`, `botster-core-sys` and `botster-worker`. P3's gate
log: `…-p3-kernel-errno-1bfc6fd3-pool-20261009-091001-91169.log` (msa1, kernel 6.12, the EINVAL branch). This reviewer did
not read it.

### Checked, no finding

- **The predicate does not change.** `gone_at_open` stays ESRCH or EINVAL. Only its doc changes. After the 2025 pidfs
  commit, a released process gives ESRCH (still "gone"), and a non-leader thread gives ENOENT (not "gone"). A listed
  member is a thread-group id from `/proc`, so ENOENT cannot prove a member gone. If a member's pid is reused by a
  non-leader thread, the wait fails with ENOENT rather than reporting "gone". That is the safe direction: a test fails, and
  no live process is taken as ended.
- **The test.** It calls `pidfd_open` on a live non-leader thread id, then `await_end` on the same id while the thread
  still lives (it waits on `ended`).
  - EINVAL (an older kernel): `await_end` reports `Gone`.
  - ENOENT (a newer kernel): `await_end` returns the error from `other?`, and the test checks its raw errno (the rustix to
    io conversion keeps it).
  - Any other answer (including a successful open) panics as undocumented. So the test does not pass on an unknown kernel
    behavior.
  - The thread is joined after `drop(end)`, and no wait is unbounded (`await_end` has the deadline `real_now()`, which
    returns at once).
- **The shared crate.** `botster-test-process/src/platform/linux.rs:88` has the same predicate. Its unit test asserts the
  predicate's values only (`:135-138`), and no test there forces a kernel answer, so no test there depends on the kernel
  version.

### Condition (not a finding)

The gate ran the EINVAL branch only (kernel 6.12). The ENOENT branch, which is the reason for this PR, has no run. This
CLEAN covers the code. The lead decides whether the run on a newer kernel (`gaming`, which waits for a node pin) is required
before merge. This reviewer recommends it, because one branch of the assertion has never executed.

### Carry (not counted), for P6 PR C

The doc of `gone_at_open` in `botster-test-process/src/platform/linux.rs:83-85` still gives only the older reasons (ESRCH
"no process has the pid", EINVAL "released"). When PR C replaces this copy, the shared crate's doc must take this PR's text
(ESRCH also for a released process since the pidfs commit, and ENOENT is not a proof).

VERDICT: CLEAN (0 open) at 1bfc6fd39fa077bec92b7fe021d5d7c9f6798618 (the ENOENT branch has no run; the lead decides)
