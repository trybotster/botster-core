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
