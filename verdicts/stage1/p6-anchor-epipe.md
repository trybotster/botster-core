# PR #212 — Anchor acknowledgement and empty mutant listing

## Round 1

Reviewed head: `7cec9af6f1f7f332a0280c4ff9bdef0b9fd8cd58`.
Base: `1627732f5f651f5946e5d60b64dedcbf6c4cb683` (`v1`).
Scope: the complete two-commit, five-file delta.
Risk: HIGH. The change affects shared process fixtures, mutation exclusions, and the gate's mutation decision.

### R1-1 — HIGH — The regression test needs independent cleanup ownership

`crates/botster-test-process/tests/slow_process.rs:619` starts the production child and anchor with raw `Command::spawn`.
Neither child has an independent cleanup owner.
When the EPIPE fix is reverted, the anchor reports an error and exits before group cleanup.
The assertion on the next report then panics before either `production_reap` call.
Setup failures before the anchor starts also bypass its cleanup.

`Blocker` alone does not close this gap.
Its documentation explains that a cat which opens the FIFO after the writer closes can remain blocked in that open.
The test has no acknowledgement that the cat opened the FIFO, and no independent group guard.
Dropping a raw child does not kill or reap it.

Give the fixture's processes bounded cleanup on every failure path, independent of the anchor behavior under test.
Preserve the success assertions: the anchor ends the leader, and the production owner reaps its own child.
The regression must fail without leaving a process when the EPIPE behavior is reverted.

### R1-2 — MEDIUM — Supply slow-feature mutation evidence for the changed anchor branch

The changed reason in `.cargo/mutants.toml` cites the new slow proof for the excluded anchor body.
The PR names no manual slow-feature mutation log for this branch.
The full gate catches three mutants in `parse_listing`; its second environment-only mutation invocation repeats default coverage.

Plan revision 23n requires a manual run that bypasses the exclusion and enables the slow feature and slow nextest profile.
After R1-1 is fixed, provide a focused run for the changed anchor branch with the gate's first-failure setting.
Name its log, test execution, and per-mutant outcomes in the PR.
Use `--no-config --in-place --features slow` and `-- --profile slow --max-fail 1:immediate` with the relevant selection.
No broad mutation rerun is requested.

### R1-3 — LOW — Add the required Prior art note

The PR description lacks the Prior art note required by `pair-common.md`.
State which existing anchor and parser code the change reuses, and why each custom change is needed.
This finding requires only a description correction.

## Evidence and checks

The anchor change ignores only `BrokenPipe` from the acknowledgement write.
Other write failures still return an error. Registration, group verification, guard wait, and cleanup remain in their existing order.

The installed cargo-mutants 27.1.0 source confirms the empty-list behavior.
`in_diff.rs` returns `NoMutants` for an empty matched set and maps it to a successful exit.
`main.rs` logs that result and returns before JSON output.
`parse_listing` first checks command success and UTF-8 stdout through `stdout_of`.
It then accepts empty stdout only with the expected log text. Other malformed output still fails.
The changed unit test checks empty output with and without that text and malformed JSON with the text.
The gate mutation run catches all three mutants of the new condition.

The exact-head Linux pool log is:
`~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-7cec9af6-pool-20261009-190202-12260.log`.
All ten CI steps pass. Default tests: 1284 passed. Slow tests: 255 passed. Conformance: 121 passed.
Both mutation invocations report three caught, zero missed, zero timeout, and zero unviable mutants.
The new fixture passes in the slow step. The pool job exits 0.
The earlier named gate confirms the empty JSON parse failure that motivated the parser change.

The head contains the current remote `v1` commit. The branch contains no merge commit in this delta.
`git diff --check` passes. No conformance list, API snapshot, dependency pin, or exclusion pattern changes.
The assigned P5 package reviewer was contacted. No exact-head package verdict was available when this round was written.
All three findings were sent to P6. The reviewer ran no builds, tests, or gates.

VERDICT: NOT CLEAN (3 open) at 7cec9af6f1f7f332a0280c4ff9bdef0b9fd8cd58

## Round 2

Reviewed head: `a17ecd8b68197b0f047d242ea044fe3c5c32081d`.
Base: `6cc7a72272ea56cdc128431ef39218a9c6743d08` (`v1`, including #206).
Scope: both correction commits after `7cec9af6`, the base merge, and the updated PR and evidence.

### R1-1 — CLOSED

The shared `anchor_stage` helper gives the leader an `OwnedChild::spawn_group` owner before subsequent setup can fail.
It gives the anchor an `OwnedChild::spawn` owner in the leader's group.
Both owners survive through report reads and assertions. Their cleanup does not depend on the anchor behavior under test.
The group owner retains the unreaped leader as its reserve until group cleanup completes.
The child owner kills and reaps only its own child when necessary.
Their existing waits and cleanup use deadlines.

The successful EPIPE test still requires the anchor's `ok` report, a leader ended by KILL, and anchor exit code zero.
The leader's status check observes the anchor's result before the owner cleans up and reaps the leader.
An early anchor exit or a setup or assertion failure now unwinds through the independent owners.
The implementer reports a local Mac reversal with no process left. This reviewer did not run that reversal.

The second acknowledgement test uses the same owners.
It fills a non-blocking pipe that retains its reader, then gives that pipe to the anchor as stdout.
The write fails with `WouldBlock`. The test keeps the guard connection open and requires an error report and exit code one.
The pipe-fill loop writes until the pipe refuses more bytes; it does not wait for another actor.

### R1-2 — CLOSED

The earlier focused run at `264151bd` caught three mutants and missed the guard replacement with `true`.
Its retained log is `~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-264151bd-pool-20261009-191306-48988.log`.
The added non-BrokenPipe test closes that gap.

The exact-head manual run uses `--no-config --in-place --features slow`, the anchor file and PR diff, and nextest's slow profile.
It uses `--max-fail 1:immediate`. That profile has no `terminate-after` setting.
The run catches all four anchor mutants, with no missed, timed-out, or unviable mutant.

The reviewer read the recovered per-mutant commands, counts, and failures from:
`~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-a17ecd8b-manual-mutants-per-mutant-logs.txt`.
The separate read-only pool job copied those logs from the original run's branch target volume.
Its source log is `~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-a17ecd8b-pool-20261009-192232-74245.log`.
It names the same head and base. It does not execute the tests again.

- The baseline passes 82 tests across three binaries, including the slow process tests.
- Replacing the anchor body with `Ok(())` fails the existing guard proof when the anchor ends before holding the group.
- Replacing the match guard with `true` fails the new test's bounded line read at `slow_process.rs:706`.
  The test receives no error report within the existing ten-second cleanup bound. It panics on that deadline result.
  All 82 tests run: 81 pass and this one fails. Nextest does not terminate this test.
- Replacing the guard with `false`, or replacing equality with inequality, fails the EPIPE test at `slow_process.rs:675`.
  The test receives `error anchor Broken pipe` instead of `ok`.

Other SIGTERM results follow the first test failure under the required first-failure setting.
They are not the evidence that catches these mutants.
The exclusion reason now cites both acknowledgement tests. Its regex remains unchanged.

### R1-3 — CLOSED

The PR has a change-specific Prior art section.
It identifies the existing guard design, acknowledgement protocol, helpers, parser, and cargo-mutants output behavior.
It explains the two small custom conditions and rejects ignoring all write errors or accepting all empty listings.
It also corrects the package-review assignment to P6.

### Merge and gate evidence

The automatic merge tree for `a17ecd8b` is `8b407e8b75029ad570ccb2a03d1f30a1e023b691`, identical to the actual tree.
The merge has no conflict. The diff against current remote `v1` contains only this PR's five files.
The #206 changes remain intact. The parser and anchor product corrections are unchanged from round 1.
`git diff --check` passes.

Full log: `~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-a17ecd8b-pool-20261009-191625-59285.log`.
All ten CI steps pass. Default: 1329 passed. Slow: 256 passed. Conformance: 124 passed, zero failed.
Both acknowledgement tests pass in the slow step.
Both ordinary mutation invocations catch all three parser mutants.
The appended manual run catches the four anchor mutants described above. The entire pool job exits zero after 269 seconds.
The environment-only second mutation step is not treated as slow-feature evidence.

The lead accepted this combined zero-exit log without a rerun in message `msg_plugin-w_1791598946_8fe1ca`.
The lead had not yet sent P6 the separate-job rule and has now done so for future manual runs.
The reviewer ran no builds, tests, gates, or mutation jobs.

The reviewer read the same-head package CLEAN verdict at `5f27d0647dac553f6b4589b66bd0040f2265d287`, round 2.
Its closure evidence agrees with this review. All three findings are closed. No new finding remains.

VERDICT: CLEAN (0 open) at a17ecd8b68197b0f047d242ea044fe3c5c32081d
