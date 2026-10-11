# Contracts #48 — R-69 event fences

## Round 1 — 2026-10-11

Head: `5ff1b52be672bc107e80cd0d911110034745eda4`.
Base: `9d19b997fd044ca76c0844f8a32fa79d930e5ec4`.
Tree: `8fc2005e97ed7b63b1e1d88445174a719702c27d`.
Lead receipt: `msg_plugin-w_1791733862_0497f7`.
Package verdict: `.review-verdicts/r69-fences-5ff1b52b.md`, reviewer 016c, ACCEPT.

I read the complete four-file delta, the complete three transcripts, and the complete PR body.
I verify that the head contains the named base.
The delta adds three waiting fences and the controls documentation. It moves the T2 sentinel after `tail`.
Removing those exact changes restores every base transcript value.
No frozen contract, manifest, ledger, driver, probe, or contract crate changes.

Each fence follows the final title wait and precedes `oracle_resume`.
Core EV-7 and section 6.2 require the named Activity, TitleChanged, and applicable ModesChanged events.
The mode change in the model transcript hides the cursor. The other final outputs change no mode.
All three final outputs end with their title sentinel. The incomplete first SGR cut keeps its earlier sentinel placement.
The T2 move preserves the final cells and cursor. The capture precedes that output, and no step asserts those bytes literally.
The only step after each fence is `oracle_resume`. No later event assertion depends on the consumed events.

I read `pump_collect`, `next_event`, and its bounded wait at the exact Contracts head.
The driver waits for distinct events in any order. A deadline or poll limit cannot become a PASS.
The fence therefore needs no invented ordering between Activity and TitleChanged.
The controls document retains the R-66 and R-67 text.
Its R-69 rule refuses foreign events, holds or re-injects none, and releases the successful second capture after every page.

Gate: `~/botster-sessions/gates/botster-contracts-stage1-rc-r69-fences-5ff1b52b-pool-20261011-084320-5265.log`.
SHA256: `462d513f77260aa8127513b61ce1fae6994af39da86e4900931644b1c6c1d403`.
I read the exact-head and base headers, summaries, and the changed transcript test records.
All ten steps pass in 257.3 seconds. Default tests: 2076 PASS. Slow tests: 33 PASS.
All three changed reference-fake tests pass. Mutants reports no Rust source change; it proves no mutation catch.
The job and wrapper exit zero after 265 seconds. I do not claim a full read of the large fuzz trace.

macOS proof: `~/botster-sessions/gates/botster-core-rc-r69-fences-mac-5ff1b52b-20261011-081651.log`.
SHA256: `b832b756e32606947c1280985ca6979c738a7815666e7943e14a1e1cf4fecc62`.
I read the full log. It records three testkit passes and fifteen real passes, five per identifier, with no refusal.
The header names Core `eba028f6`, a two-file local change, the exact Contracts head, and the .36 probe.
These are scratch proofs of the fenced transcripts. They are not final Core pin-gate results.
The complete unfenced .36 log records `events_pending` failures for all three identifiers.
Its SHA256 is `0245337d9bc8ae0ada9943cfbdf8e70c692712811aed3322f87596468bb9af36`.
Linux real-tier proof remains due at the Core pin gate, as R-69 requires.

Zero findings remain. This verdict approves the named Contracts PR, not a future Core pin or pending removal.
The steward owns the merge and tag. I ran no execution job.

VERDICT: CLEAN (0 open) at 5ff1b52be672bc107e80cd0d911110034745eda4
