# P6 payload guard reset review

## Round 1 — 2026-10-09

PR: https://github.com/trybotster/botster-core/pull/175

Reviewed head: `ec51e6ca5666b4a6075668255357a82a2989da85`.
Base: `a0f78fe4c4e4faff1fee926e072ceb5e94ba8649`.

The diff changes one test fixture and adds its regression test.
The reset branch preserves any report data. Other read errors still fail.
A reset without a report still requires the registered group to become empty within `CLEANUP`.
This check sends no signal to the unreserved group and reaps no production child.

The test leaves the member's readiness byte unread before it closes the member connection.
The ready helper's byte establishes that ordering through the socket protocol.
The readiness read has the existing `CLEANUP` bound.
The test creates and reaps only its own `/usr/bin/true` child through the existing owned-child helper.
The test proves guard completion through its public cleanup behavior. It does not inspect `Outcome`.
No production contract assertion changes.

Supplied evidence, under `~/botster-sessions/gates/`:

- `botster-core-stage1-p6-guard-reset-ec51e6ca-pool-20261009-022324-73887.log`: the new test fails with `Connection reset by peer` after removal of the fix. The restored fix passes.
- `botster-core-stage1-p6-guard-reset-ec51e6ca-pool-20261009-022335-74188.log`: all ten Linux gate jobs pass. The run passes 810 default tests and 218 slow tests. No mutant qualifies for this test-only diff.
- `botster-core-stage1-p6-guard-reset-ec51e6ca-pool-20261009-022657-76939.log`: the Mac payload run passes all 27 tests, including the new test.

The earlier test-only run used a dirty snapshot and does not establish failure without the fix.
The verdict uses the explicit revert-and-restore proof above.
The reviewer ran no gate, build, test, or mutation job. The reviewer changed only review documentation.

VERDICT: CLEAN
