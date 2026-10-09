# Integration review: #188 (the contracts pin to contracts-v0.1.20; branch stage1/p5-pin-v0.1.20)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — CLEAN on head e05d603b

Reviewed head: `e05d603bde49985015031f676dc64950fa2d7324`, one commit on v1 `a14e9dc2` (the current v1). The stated tier is
HIGH by rule 4 (a pin move), which is correct.

- **The pin.** The six contracts dependencies move from `contracts-v0.1.19` to `contracts-v0.1.20`, and the pin comment
  names `0389165` and R-37. `Cargo.lock` changes only the nine contracts `source` lines, to
  `contracts-v0.1.20#03891658e793e5400ba46b5bc003b5d9f952f5e2`, which is the tag's commit. `core-pending.txt` changes only
  the two A15 comment lines, and the contracts `pending.txt` at v0.1.20 still lists the A15 transcripts.
- **The contracts delta for Core** (`git diff contracts-v0.1.19 contracts-v0.1.20`): nothing under `frozen/`; the two
  transcripts that plan Q10 asked for (`dp_3_frame_limit_checked_before_allocation`, `ou_9_attach_is_sync_baseline_in_pump`),
  which leave the contracts `pending.txt`; five new testkit controls (`hold_handoff`, `release_handoff`,
  `oracle_screen_payload`, `alloc_window`, `alloc_peak`) in `docs/core-testkit-controls.md` and in the test-only
  `vocabulary.rs`; ruling R-37 (the allocation-peak observer is allowed); amendment candidates only (Core A17 candidate 1,
  and two Hub candidates); and BUILD.md's risk tiers and round limit (the text that plan 23 reads at `main` `56bd0a5`). No
  crate API changes, and the ledger is unchanged (the gate's `lists` step passes).
- **dp_3 and ou_9 stay pending.** Their controls are not in the testkit yet (the lead's ruling: P4a's). An id with a
  transcript and no control would fail the conformance run, so pending is right.
- **The gate log** (`pin-v0.1.20-e05d603b.log`) names the head and base `a14e9dc2`. The slow tier runs 243 tests, all pass;
  `lists` passes; exit 0.

VERDICT: CLEAN (0 open) at e05d603bde49985015031f676dc64950fa2d7324
