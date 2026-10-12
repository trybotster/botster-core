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
