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


## Round 2

Implementation head: `a17ecd8b68197b0f047d242ea044fe3c5c32081d`.
Integration and full gate base: `6cc7a72272ea56cdc128431ef39218a9c6743d08`.
Plan 23n evidence rule: `stage1/plan` `f545597b`, section 8.
BUILD.md rule 0: botster-contracts `85ecb225b9a646c522a4ef5ba4bd813615fd5f18`.

The reviewer checked the stated HIGH tier first. It remains correct because this PR changes gate decisions.
The lead's corrected assignment remains: this reviewer owns P6 package review; Astra owns integration review.
The reviewer read the exact Git objects, the updated PR description, and the supplied logs.
The reviewer changed no product code and ran no gate, build, test, mutation job, or reversal.

### R1-1 closed — both test children have cleanup owners

Commit `264151bd` moves setup into `anchor_stage`.
It starts the FIFO leader with `OwnedChild::spawn_group` and the anchor with `OwnedChild::spawn`.
Each owner exists before a later setup, read, or assertion can fail.
The leader reserves the group identifier until its owner ends the group and reaps it.
The anchor belongs to that group and also has its own child owner.
These owners provide cleanup on setup failure and panic, independently of the anchor under test.
Each owner reaps only its test-owned child. No production-owned child changes its reaper.

The original test still asserts the public `ok` report, leader KILL status, and anchor exit code 0.
The test observes leader status before owner cleanup, so owner cleanup cannot supply a false passing KILL assertion.
The report reads and status waits use the existing cleanup deadline.
The anchor's identity checks, group checks, blocking connection wait, bounded cleanup rounds, and reserve-only reap remain unchanged.

### R1-2 closed — the slow-feature run catches the changed anchor mutants

Commit `a35e8de8` adds `an_acknowledgement_that_fails_otherwise_fails_the_anchor_stage`.
The test gives the anchor a full non-blocking pipe with a reader.
It keeps the guard connection open and asserts the public error report and exit code 1.
Its pipe-fill loop writes data until WouldBlock. It does not poll for another actor or retry WouldBlock.
The test then waits for a report and process exit within the existing cleanup deadline.
It adds no sleep, busy wait, or private-state assertion.
Both acknowledgement tests use the cleanup owners above.

The PR records the required focused command with `--no-config --in-place --features slow`, the PR diff, and the anchor binary filter.
It passes `--profile slow --max-fail 1:immediate` to nextest.
The exact-head log reports four caught mutants, zero missed mutants, and zero timeouts.
The existing per-mutant logs confirm successful compilation and actual slow tests:

- The baseline runs 82 tests; all 82 pass.
- Replacing `anchor` with `Ok(())` fails `a_cleanup_that_cannot_finish_fails_through_the_guard`.
- Replacing the BrokenPipe guard with `true` fails the new error test at its ten-second report deadline. The run reports 81 passed and one failed.
- Replacing the guard with `false` fails the missing-reader test: it receives `error anchor Broken pipe` instead of `ok`.
- Replacing `==` with `!=` fails the same public report assertion.

The last two runs stop after the first failure under the required fail-fast setting.
Their cancelled tests are not claimed as executed proof.
The earlier `264151bd` log records the guard-to-true survivor; the new test closes that gap.
The reviewer read existing evidence and requested no rerun.
The exclusion reason now names both acknowledgement tests. No exclusion regex changes in this PR.

### R1-3 closed — the PR records Prior art decisions

The updated PR description has a Prior art section.
It names the existing DESIGN.md guard design and keeps the acknowledgement mechanism.
It records reuse of the anchor protocol, cleanup owners, deadline helpers, and existing listing parser.
It records reuse of cargo-mutants 27.1.0's empty-filter message and explains why changed text makes the gate fail.
It explains the two small conditions and adds no dependency.
It rejects ignoring every acknowledgement error and accepting every empty listing, with a reason for each.
The original crate Prior art decisions remain intact.

### Scope, merge, and gate evidence

The exact diff against `6cc7a722` changes only the five paths from round 1.
The merge imports the base's route rename into `.cargo/mutants.toml`.
The four other changed file blobs equal the first parent's blobs.
The final PR diff contains only its acknowledgement proof citations in `.cargo/mutants.toml`; it contains no route exclusion change.
No existing contract test is migrated or weakened.
The parser and its public input/output assertions remain as reviewed in round 1.
The parser still rejects failed status, malformed JSON, non-array output, and empty output without the required message.

Full Linux log: `~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-a17ecd8b-pool-20261009-191625-59285.log`.
It names the exact head and base above and exits 0.
All ten full-gate jobs pass.
It reports 124 passing conformance tests, 1329 passing default tests, and 256 passing slow tests.
Both acknowledgement tests and the parser proof pass.
Both gate mutation commands report three caught parser mutants with no misses or timeouts.
The separate manual run reports the four caught anchor mutants described above.

Per-mutant evidence: `~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-a17ecd8b-manual-mutants-per-mutant-logs.txt`.
The read-only retrieval log is `~/botster-sessions/gates/botster-core-stage1-p6-anchor-epipe-a17ecd8b-pool-20261009-192232-74245.log`.
The reviewer does not treat that retrieval as a new test run.
P6 reports local Mac passing and red runs. The reviewer does not claim a focused Mac gate or independent execution.

All three round 1 findings close. No new package finding remains at this exact head.

VERDICT: CLEAN
