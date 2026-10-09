# Integration review: #175 (P6 payload-guard reset fix, branch stage1/p6-guard-reset)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head ec51e6ca

Reviewed head: `ec51e6ca5666b4a6075668255357a82a2989da85`. Base: v1 `a0f78fe4c4e4faff1fee926e072ceb5e94ba8649` (the
branch starts on it; no merge). One file: `crates/botster-core-sys/tests/common/payload_guard.rs`, +43 -1. The slow tests
of `botster-core-sys`, `botster-core` and `botster-worker` include that file.

- **The root cause matches the RED gate of #174** (`…4a5a7158-pool-20261009-021843-68853.log`). The guard writes the
  readiness byte to the member. A member that production kills before it reads that byte closes its AF_UNIX stream with
  unread data. Linux then reports ECONNRESET to the guard's read, in place of an end of file. The guard read that as a
  failure.
- **The fix is safe.** A reset is now read as an end of file, and any data before it is kept as the report. A reset never
  passes by itself: with no report, the guard still requires the member's group to end (`await_group_end`, within
  `CLEANUP`). So the proof of the cleanup does not change. Other errors still fail.
- **The class.** The other guard reads do not have this race. `process_guard.rs` writes the readiness to the worker and
  never reads from that socket again, and its `eof` reads are of pipes (a pipe gives an end of file, not a reset). P6
  states that #171's `Guard` writes no data to an anchor (its release is `shutdown(Write)`), and PR C replaces this guard.
- **Red on revert.** `…022324-73887.log`: the new test fails without the fix ("Connection reset by peer (os error 104)",
  the gate's message) and passes with it. Note: `…022302-73481.log`, labelled `4ee2439f`, shows the test PASS. That run
  used `--allow-dirty` (snapshot `b5cf3552`), so it is not a revert proof. Do not cite it.
- **Evidence.** The full gate at this head is green (`…022335-74188.log`): the slow tier runs 218 tests, both `ev_4` tests
  of `botster-worker` among them. The Mac `slow_payload` run passes 27 of 27 (`…022657-76939.log`).
- **Observation (not counted).** The new test uses the group id of a child that it has reaped. If another group takes that
  id before the drop, `await_group_end` waits on that group. It sends no signal, so the risk is a rare spurious failure,
  not harm. That is acceptable for a test that PR C retires.

VERDICT: CLEAN (0 open) at ec51e6ca5666b4a6075668255357a82a2989da85
