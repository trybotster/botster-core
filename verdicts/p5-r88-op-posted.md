# P5 review — #297, R-88 op_posted

## Round 1 — 2026-10-11

- PR: <https://github.com/trybotster/botster-core/pull/297>
- Head: `2bb06bc335bb076136dbc76b466b4b22c5b34ff2`
- Tree: `4698444dfd27c0f0cf7c5d0f4ab158c1e55c16f7`
- Base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`
- Parents: `88eacd8da0c41267d779d4267aeb7c592ba85bd6` and the base above.
- READY receipt: `msg_plugin-w_1791769214_f42aba`.
- Risk: HIGH, rules 3 and 5. The change crosses packages and changes `crates/botster-core-host/src/engine.rs`.

I read the complete seven-file source diff, R-88(B), the control definition, the current PR body, and both completed evidence records.
The diff has 280 additions and 64 deletions. Its full-index SHA256 is `a78cff9c4525a78f80809a47eade102fc16752b048b247d63dd59b6fff67698f`.
The diff check reports no whitespace errors. The base tree equals the reviewed #292 tree, `8d370b057c8b25dcb05a8d31621b6db46e3066fc`.
The change moves no pin or conformance list. No earlier #297 head had a package verdict, so this is a complete initial review.

### R297-F1 — MEDIUM — op_posted accepts a closed handle

At this head, `process_controls.rs:188` reads `record_of(handle)` without checking the current handle.
`harness.rs:115` reads the retained `records` map. `TestkitHarness::drop_handle` removes only the `handles` entry.
Its documentation requires a control that names a dropped handle to return `ControlError::Bad`.
The new control instead returns the last recorded answer. Thus an op that completed before the drop can return `{"posted":true}` after the drop.

The real tier has the same defect in `real.rs:584`. Its `drop_handle` removes only the `handles` entry.
Its `host_crash` drops Core and its tap, but retains the record. The new control also accepts that record after the crash.
Existing `parked_work` validation checks the current handle through `session_row` or `recorded`; `op_posted` has no corresponding check.
No dispatcher performs that check before either handler.

Validate the current handle before reading the record. Reject dropped handles in both tiers and crashed handles in the real tier.
Keep the control read-only. Add lifecycle regressions that prove the rejection after an earlier successful completion.
For the real crash path, check the live tap without retaining Core in the record.

Finding receipts: P5 `msg_plugin-w_1791769514_b3240c`; integration reviewer `msg_plugin-w_1791769515_b12c05`.
The first message incorrectly named testkit line 219. The handler starts at line 188, as recorded here.

### Other source conclusions

The posted-op inference matches the engine bookkeeping while the handle is open.
`next_op` starts at 1. Admission increments it only after all synchronous checks pass and inserts the admitted op.
`complete` sets `Step::Done` and posts the terminal event. `post_completed` posts without a mandatory-room refusal.
The engine removes an op only when a poll takes its `Completed`. Other step updates cannot overwrite `Step::Done`.
Thus the record distinguishes unposted ops from completions that remain queued or were polled.

`RefusalLayer` records the initial diagnostics and each forwarded call that can change Core state.
The controls read that record. They do not pump, poll, or change the clock.
The shared scenario checks false before completion, true before its poll, the queued completion, true after its poll, and false for an unminted id.
Both tiers call the same bounded scenario. The fast tests cover the diagnostics and record comparisons.
The change preserves the #292 parked-work record and the real driver, capture layer, and crash-slot composition.

### Completed evidence

I read the completed Linux gate:
`~/botster-sessions/gates/botster-core-stage1-p5-r88-op-posted-2bb06bc3-pool-20261011-183117-63636.log`.
Its SHA256 is `2dbfeed5bcb1ab28b16417234f3c403aa317941ba50a9ac60f0e982e5ab993d1`.
The gate records this exact head and base. All ten steps pass. The wrapper exits 0 after 507 seconds on msa1.
The default tier reports 1853 passes and 435 skips in 15.830 seconds.
The slow tier reports 608 passes and 2044 skips in 43.591 seconds. Coverage checks 1076 slow-only tests and four named real proofs.
Both control scenarios pass in both tiers. The new engine and record tests pass.

Stage 1 has 37 unique outcomes: 22 caught, 9 missed, 6 unviable, and no timeout, in 140.6 seconds.
Stage 2 tests exactly those nine misses: 7 caught, 2 unviable, no miss, and no timeout, in 142.4 seconds.
Its unmutated baseline reports 608 passes and 2756 skips in 44.890 seconds; the baseline step takes 63.3 seconds.
All 29 raw catches match the 29 citations. Every cited test passes in its normal tier; each stage-2 test also passes in the baseline.
The log has no FLAKY outcome. The combined result is 29 caught and 8 unviable.
Fuzz reports no changed crate with a decoder harness; this is not a new fuzz execution claim.

Both strict summaries report 256 passes and no failure. The pending check runs 324 of 342 ids, with 18 real-only ids excluded and 3 held.
The held budget check reports 64 ignored trials and no failure. All 33 reported passing ignored real ids remain in the unchanged 44-entry real-pending list.
This control-only PR claims no pending-list removal.

I read `r88-core-pr-evidence.sh` and the complete local log:
`~/botster-sessions/shared/core-stage1/evidence/p5-r87/r88-core-pr-evidence-2bb06bc3.log`.
The log SHA256 is `a018d5d898661ba2eb18b29fcd9f534dacc20883884702f7ffcaaaea0b19b6d8`.
It records the exact clean head, a clean of 24016 files and 7.0 GiB, zero target entries, and one worktree as the target writer.
Its filtered output records fmt, lists, taint, and clippy success; 300 host and 422 testkit unit passes; and both control self-tests in both tiers.
The script copies six .42 transcripts into the .41 worktree and removes pending entries locally, then restores those files and Cargo.lock.
All six copied blobs match tag `ec7075a4a3fee067323f24f02576777ccd1abea7`; I independently checked the tag objects.
All six trials pass on testkit seeds 0–31 and real seed 0. The final log reports both worktrees clean.
This is focused evidence with .41 crates and .42 transcripts, not acceptance of the future pin.
The stopped earlier gate is not evidence. These completed records do not carry to a source repair.

### Disposition

R297-F1 remains open. P5 must send a repair READY for the new exact head.
The repaired head needs the applicable static and mutation evidence, both reviews, and its completed final gate before acceptance.
I changed no product code and ran no build, test, gate, mutation job, or executable merge check.

VERDICT: NOT CLEAN

## Round 2 — 2026-10-11 — repair and completed gate

- Head: `3693a2e370f9ab820bc6f05f159e079e9650a6b9`
- Tree: `c83156285a094638acf2097912ce31c62f94c53c`
- Base: `20b2525c60e6c3e4ee655fe1e0999c1b1b8c48a5`
- Parent: the reviewed Round 1 head, `2bb06bc335bb076136dbc76b466b4b22c5b34ff2`.
- Repair READY: `msg_plugin-w_1791770369_ca91b8`.
- Risk: HIGH, rules 3 and 5, unchanged.

I read the complete three-file repair and the affected constructor, drop, and crash paths.
The repair diff SHA256 is `e75877731cdc91670f6ed136e018d3016efa39ec83d6604bd04d573ef5b69e41`.
The complete base-to-head diff has seven files, 342 additions, and 75 deletions.
Its full-index SHA256 is `83057116f96b98819ac59ecc00d8b424aa5e920e3201a5558af774997bfdb34a`.
The diff check reports no whitespace errors. No conformance file changes.
The engine, control definition, record predicate, and shared scenario have no repair delta.
The Round 1 conclusions for open handles remain valid.

### Finding closures

R297-F1 closes. Testkit `drop_handle` removes the record as well as the handle.
Real `Handle` now owns the harness's record. `record()` reads through the current handle map.
Both `drop_handle` and successful `host_crash` remove that handle, so neither control can read its old record.
The constructor gives `RefusalLayer` and `Handle` the same fresh record.
The change adds no Core reference, pump, poll, clock change, or edge call to either control.
The record does not retain the driver or tap. The existing crash path still drops those resources.

The testkit lifecycle test proves posted true before the drop and Bad for `op_posted` and `parked_work` after it.
The record test also proves that `record_of` has no entry after the drop.
The real drop test keeps its record layer alive across `drop_handle` and proves that the harness still rejects the record read.
The real crash test uses actual Core and workers. It proves posted true before the successful crash and Bad after it.
The existing before-poll and after-poll scenarios remain intact and pass in both tiers.
Integration finding R297-SI-1 has the same closure conditions. Sol independently confirms the source closure in `msg_plugin-w_1791770450_0d9329`.

LOW R297-F2 also closes. During this round, I requested current diff counts and an explicit historical label for the initial SLOT review statement.
P5 correction receipt `msg_plugin-w_1791770491_73476c` reports both changes at the unchanged repair head.
I read the complete corrected body. It names current +342/-75, retains +280/-64 only for the first reviewed head, and dates the SLOT statement to `2bb06bc3`.
The body records the repair, regressions, both gate scopes, and local cache provenance.
No source or body finding remains open.

### Local reversal evidence and cache provenance

I read both complete reversal scripts and logs without executing them.
`r297-f1-revert.log` SHA256 is `ebfa555f07aeb91c1666165f2b6094974f55bef51a9eefe8efd8341885eb4483`.
It records the clean repair head. Removing only `self.records.remove(handle)` makes both testkit regressions fail after the drop.
Restoring the file returns a clean status and two passing regressions.

`r297-f1-revert-real.log` SHA256 is `0edc0c26d1f62b0535c46895aa7cf821bd9aa5bc8a5b9ef55d4f4c4df08c558a`.
It uses old `real.rs` with only the new crash assertions. The test passes the posted-before-crash assertion and fails the Bad-after-crash assertion.
Restoring the repair returns a clean status and a passing test. The script runs this thread-counting test alone.
Both logs contain filtered named outcomes and summaries. They supply no numeric exit proof for each command.

P5 receipt `msg_plugin-w_1791770491_73476c` confirms that this one Core worktree wrote to `targets/p5` from the earlier logged clean through both reversals and repair checks.
The pool gates build on their nodes. The prior SLOT worktree used the target before that clean.
This owner statement supplies the B13 cache condition. It does not change the scope of the earlier six-transcript proof.

I also read the completed `r297-b13-provenance.sh` and `r297-b13-provenance.log`.
The log SHA256 is `f435ddd8c80255193e8f7d3a88ed113139717a60144895be98ffb2e9b5aa6abd`.
The snapshot reports 30601 files. Its dependency files name only this Core worktree, the .41 contracts worktree, and the target directory.
It reports zero directories older than the clean. The 88 files with older birth times are inside Ghostty build directories created after the clean.
P5 attributes those file times to source copies. This snapshot supports the owner statement; it does not independently prove every earlier writer.
P5 also reports local fmt, clippy, taint, 422 testkit unit passes, real control scenario passes, and the real drop regression pass.
Those separate local check claims come from the owner. The retained reversal outcomes and exact-head Linux gate supply the independently read repair evidence.

### Completed exact-head gate

I read and parsed the completed gate:
`~/botster-sessions/gates/botster-core-stage1-p5-r88-op-posted-3693a2e3-pool-20261011-185114-64984.log`.
Its SHA256 is `0003d9a2b47bd53dfacd82e56275c3e1943cb135001c777eafa1b19f348db3bd`.
The header and lists step name this exact head and unchanged base; the merge base equals that base.
All ten steps PASS in 450.9 seconds. The job exits 0 after 461 seconds with no queue wait; the wrapper exits 0 after 462 seconds on msa1.
Default: 1853 passes and 435 skips in 13.886 seconds. Slow: 608 passes and 2044 skips in 42.110 seconds.
Coverage checks 1076 slow-only tests and four named real proofs.
All requested lifecycle regressions pass. Both `op_posted` and `parked_work` scenarios pass in both tiers.

Stage 1 has 39 unique outcomes: 23 caught, 10 missed, 6 unviable, and no timeout, in 102.1 seconds.
Stage 2 tests exactly those ten misses: 8 caught, 2 unviable, no miss, and no timeout, in 154.5 seconds.
The unmutated baseline reports 608 passes and 2756 skips in 44.463 seconds; its step takes 55.2 seconds.
The combined result is 31 caught and 8 unviable. All 31 raw catches match the citations.
Each cited test passes in its normal tier. Each stage-2 catcher also passes in the baseline.
The real record-helper mutant is caught by the crash regression. The testkit drop-handler mutant is caught by the record regression.
The log has no FLAKY outcome. Fuzz reports no changed crate with a decoder harness; no new fuzz execution is claimed.

Both strict summaries report 256 passes and no failure. The pending check runs 324 of 342 ids, with 18 real-only ids excluded and 3 held.
The held budget check reports 64 ignored trials and no failure in 19.27 seconds.
All 32 reported passing ignored real ids remain in the unchanged 44-entry list. This PR claims no status removal.
The earlier six .42 transcript trials remain evidence for the earlier head with .41 crates, not a new pin or repaired-head execution claim.
The new exact-head gate covers the repair. No earlier gate result is substituted for it.

### Disposition

Zero package findings remain. This CLEAN covers the exact repair head, source, corrected body, and completed gate above.
The matching final integration verdict and the lead's Q7 check remain required before merge.
Any later source change, base import, pin change, or different merge tree requires its applicable review and evidence.
I changed no product code and ran no build, test, gate, mutation, reversal, or executable merge check.

VERDICT: CLEAN
