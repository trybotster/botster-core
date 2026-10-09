# Integration review: #185 (20 P1 and P5 minimum ids leave core-pending.txt; branch stage1/p5-minimum-flip-20)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head 539a4c7b

Reviewed head: `539a4c7b211b9f116434ae6fb6d9b27057154707`, one commit on v1 `aaac0c0d` (the current v1): 20 lines removed
from `conformance/core-pending.txt`, and `crates/botster-core-testkit/tests/worker_transcripts.rs` (+3 -6). The stated tier
is HIGH by rule 3 (the shared testkit), which is correct.

### Checked, no finding

- **The 20 ids.** Each is in `docs/stage1-clauses/minimum-core.txt` (plan `dd656249`) and in the P1 or P5 clause file.
  The set is exactly the PASS lines of `p5-minimum-probe-v1-aaac0c0d.txt`.
- **The gate log** (`flip20-539a4c7b.log`) names the head and base `aaac0c0d`. The conformance binary reports 20 passed
  and 655 ignored (675 ledger ids), and `lc_5_stop_ends_payload` is among the passes. The default tier runs 942 tests and
  the slow tier 243, all pass.
- **`worker_transcripts.rs`.** Its `IDS` loses the three ids that now leave pending, and keeps `a2_8`, `ev_4` and `lc_3_remove_created`, which stay pending.
  The clause line names TI-1, EV-4, LC-3 and LC-7, which are the clauses of the remaining ids.

### H1 LOW — the runner's header still states the old rule

`worker_transcripts.rs:3-5` says: "They stay in `conformance/core-pending.txt` until they also pass on the real-process
harness (plan 4.2b; `RealCoreHarness` does not exist yet), so the conformance harness does not run them." After this PR,
ids leave pending on the testkit alone (the lead's ruling, as the PR body states), so the sentence is false for the three
ids that this PR moves. Fix: state the lead's ruling: an id leaves pending when it passes on `TestkitHarness`, and
`RealCoreHarness` runs it later.

### To the lead (QUESTION, not counted here)

The PR body says that plan revision 23 counts "testkit-passing / 70" and "real-passing / 70". The plan at `dd656249` does
not: section 1 counts "minimum ids passing / 70" (an id passes when it is out of `core-pending.txt` and its trial passes),
section 5 says that a package removes ids "in the pull request that makes them pass on both harnesses", and 4.2b has "no
testkit-only exemption". The lead's ruling, as the PR states it, changes this. This reviewer accepts the ruling and asks
the lead to confirm it and to record it in the plan.

VERDICT: NOT CLEAN at 539a4c7b211b9f116434ae6fb6d9b27057154707 (1 open: H1 LOW)

### Addendum to round 1 (same head 539a4c7b): the real-only exception of plan revision 23a

The lead asked for a check of the 20 ids against the exception of revision 23a (`48c14ab8`): a real-only id leaves pending
only with its passing real proof. In `conformance/replacement-map.json` at `contracts-v0.1.19`:
- 19 ids have proof `core-testkit`, `core-testkit+edge` (process or clock) or `core-testkit+perturb`. They are not
  real-only.
- `conf::lc_2_data_dir_is_exclusive` has proof `slow:data-dir-lock` ("the lock is a real file lock and is released when the
  process dies"). Plan 4.2b also names "the real file lock of LC-2". So it is real-only.

`lc_2` has a real proof at this head: `crates/botster-core/tests/slow_real_core.rs`
`a_second_open_is_refused_until_the_first_is_dropped` cites Core LC-2 and uses the real `flock`. It follows the
transcript: open, a second open gets `DataDirInUse`, drop, open again. It omits the `list` step, which needs no real OS
condition. It passes in the slow tier of the exact-head gate (`flip20-539a4c7b.log`, 29/243). So `lc_2` may leave pending,
and no id must go back.

The release on process death that the map's note names is a property of `flock`. No test kills a process that holds the
lock. This reviewer does not count it, because the transcript drops the handle and does not end a process.

**H2 LOW (new, from revision 23a):** the PR body must name the real proof of the one real-only id
(`lc_2_data_dir_is_exclusive`: `slow_real_core::a_second_open_is_refused_until_the_first_is_dropped`), so that the
exception is visible on the record.

VERDICT: NOT CLEAN at 539a4c7b211b9f116434ae6fb6d9b27057154707 (2 open: H1 LOW, H2 LOW)
