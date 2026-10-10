# P6 anchor acknowledgement and empty mutation listing — PR #212

## Round 1

Implementation head: `7cec9af6f1f7f332a0280c4ff9bdef0b9fd8cd58`.
Integration and full gate base: `1627732f5f651f5946e5d60b64dedcbf6c4cb683`.
Plan 23n evidence rule: `stage1/plan` `f545597b`, section 8.
BUILD.md rule 0: botster-contracts `85ecb225b9a646c522a4ef5ba4bd813615fd5f18`.

The reviewer checked the stated tier first. HIGH is correct under BUILD.md rule 1 because the PR changes gate decisions.
The lead permits the parser fix in its own commit in this PR.
The lead corrected the package-review assignment to this P6 reviewer; Astra owns integration review.
The earlier P5 assignment in the PR body is superseded by that message.
The reviewer read the exact Git objects and supplied evidence.
The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.

### Source result

Commit `2338a236` ignores only `ErrorKind::BrokenPipe` from the anchor's acknowledgement write.
It preserves every other write error.
The anchor still verifies identity and group before its report and before cleanup.
It still holds the group until the guard connection ends.
It retains bounded cleanup rounds and reaps only its reserve.
The change adds no sleep, busy loop, or production-child reap.
The new test reads public reports and exit status.
Its reads use the existing cleanup deadline.
Its intended assertions are `ok`, a leader ended by KILL, and an anchor that exits with code 0.
The test's failure-path ownership remains defective, as finding R1-1 explains.

Commit `7cec9af6` accepts an empty listing only after the tool succeeds and its stderr contains the pinned empty-filter message.
It preserves rejection of failed status, non-JSON output, and non-array output.
The parser proof asserts the required three cases: `[]`, empty with the message, and empty without it.
It also rejects non-JSON output with the message.
The test asserts accepted counts and rejected results, not private state.
The PR states its dependency on cargo-mutants 27.1.0's text.
A changed message makes the empty-output path fail.
No parser finding remains from this source review.

### R1-1 HIGH — the new regression test does not own cleanup on failure

`tests/slow_process.rs:621-649` starts the FIFO leader and anchor as raw `Child` values.
The test gives neither child an independent cleanup owner.
Only the success path reaches `production_reap` at lines 661-662.
An identity read, pipe setup, anchor spawn, report read, report assertion, or cleanup assertion can fail before those reaps.
Dropping `Child` does not kill or reap the child.

The supplied red-on-revert case demonstrates one such path.
The old anchor sends its report, fails its acknowledgement with BrokenPipe, and reports `error anchor Broken pipe`.
The `ok` assertion then panics before either reap.
The anchor under test cannot clean the leader after it exits on that error.
The Blocker closes its writer, but it does not reap a child.
A cat that has not opened the FIFO can block in the open after that writer closes.
`fixture.rs` documents this late-open race and relies on group ownership to end it.
The new test has no fallback group owner.

Own the group and both test children on every path, including setup failure and the expected red path.
Use the crate's bounded owners or another explicit cleanup owner.
Retain the public report, KILL, and successful anchor-exit assertions.
Any cleanup owner must reap only these test-owned children and must preserve the production-reap boundary.
The reviewer sent this finding directly to P6. Astra independently found the same failure.

### R1-2 MEDIUM — the changed anchor exclusion lacks required slow-feature mutation evidence

The PR changes the anchor branch and adds the new slow test to its exclusion reason.
Every anchor-stage mutant remains excluded in the supplied gate.
Both supplied mutation runs execute three mutants of the new parser condition.
The second command sets `NEXTEST_PROFILE=slow`, but the xtask still selects its explicit `mutants` profile and passes no `--features slow`.
It does not supply slow-feature mutation evidence for the anchor branch.

Plan 23n requires an interim manual run for an exclusion that rests on slow-tier evidence.
Name the log in the PR for a focused `cargo mutants --no-config --in-place --features slow ... -- --profile slow` run.
Use the specified fail-fast setting.
Report the executed anchor mutants and their outcomes.
Supply behavior proofs or accepted reasons for survivors and resolve timeouts.
A `NEXTEST_PROFILE=slow cargo xtask ci --job mutants` run does not meet this rule.
The reviewer sent the requirement directly to P6 and confirmed Astra's same finding.

### R1-3 LOW — the change lacks its Prior art note

BUILD.md rule 0 requires a Prior art note in the design note or PR description.
The crate's existing DESIGN.md note records the guard design and library reuse or rejection.
This PR does not record the acknowledgement-error choice or the empty-list protocol choice in a Prior art note.

Add a short note for this change.
Link the existing guard note and state which acknowledgement mechanism is reused or rejected, with the reason.
Record cargo-mutants 27.1.0's existing empty-list protocol and the reason for using its message.
The reviewer sent this requirement directly to P6.

### Scope and evidence

The two implementation commits change five paths: the anchor binary, anchor documentation, its slow test, one exclusion reason, and ci.rs.
They introduce no migration of an existing contract test.
The new acknowledgement test preserves public behavior as its intended assertion.
The process wait helpers, identity checks, cleanup rounds, and production reaper remain unchanged.
The revised mutation reason changes no exclusion regex and names the new test without an accidental proof citation.

Full Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-7cec9af6-pool-20261009-190202-12260.log`.
It names the exact implementation head and base above.
All ten full-gate jobs pass.
It reports 121 passing conformance tests, 1284 default tests, and 255 slow tests.
Both named new proof tests pass.
The default and repeated mutation jobs each report three caught mutants, with no misses or timeouts.
This evidence covers parser mutation and the normal anchor test.
It does not close the failure-path ownership gap or supply the required anchor mutation evidence.
The implementer reports a local Mac red-on-revert and passing test.
The reviewer did not execute that reversal and does not claim a focused Mac gate.

Astra published NOT CLEAN for this head in verdict `5df20c3e9a59bf380438c185dacedbdf55a85e63`.
The package review confirms the same HIGH, MEDIUM, and LOW findings.
All findings went directly to P6. No ordinary NOT CLEAN report went to the lead.
Wait for the replacement READY head.

VERDICT: NOT CLEAN
