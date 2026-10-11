# Contracts #68 (`stage1/p5-r87-link-held`): integration review (R-87, P5)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
Implementer: P5 (`sess-1791517266-0166-9e69a1c3914b450c0b8a98aa5c29e374`). Package reviewer: 016c. Ruling: R-87 (pending; it governs now). Tier: HIGH (steward).

## R-87 checks for the READY head (steward, 2026-10-11)

- A new read-only control: `link_held {session}` gives `{held}`. `held` is true when Core holds a frame from S's worker link that it has read but not accepted (EV-5b, parked).
- In `ev_5_poll_wakes_pending_work`, after step 8: `await_control link_held` with `expect held: true`.
  - The wait pumps and never polls.
  - It is a precondition fence only. Step 13's 0 ms `Woken` stays the TM-6 claim.
- A `Woken` wait with a timeout is rejected.

## Round 1: CLEAN on 006ccf22e4fb8bbae2705e1b3ef50be23f3ea549

Base main `71f01c8812132fa979b8584f12a92ba2efde071e`. The head fast-forwards main. One commit, `006ccf2`, changes 3 files (+19/-0).

Gate: `~/botster-sessions/gates/botster-contracts-stage1-p5-r87-link-held-006ccf22-pool-20261011-143912-94458.log` (sha256 `ca3fe2b18b40345f9f13f7648ed097bdf0fbc5b06c2df90314862c0d3c196972`). The header names head `006ccf22...` and base `71f01c88...`. msa1, allocation `875c3212`. manifest, frozen, taint, ledger, rust, slow, codegen, ts, fuzz, mutants: all PASS. Exit 0.

R-87 checks:
- **Control row** (`docs/core-testkit-controls.md`): `link_held {session}` gives `{held}`. It is true when Core holds a frame that it has read from the session's worker link and not accepted (EV-5b, parked). It is read-only. Both harnesses answer it. The row states the difference from `held_reports` (reports that the edge holds before Core reads them, A12-1a). It names no message type of the link (R-65).
- **Vocabulary**: `("link_held", &["session"], &[])` is in `CONTROL_ARGS`, and `"link_held"` is in `CONTROLS`.
- **Fence placement**: in `ev_5_poll_wakes_pending_work`, the new step is index 9 (step 10 in 1-based count), directly after `payload_alive {s1}` `alive: false` and before `await_quiet`. `requires.controls` adds `link_held`.
- **Pumps, never polls**: `driver.rs` `await_control` (line 3029) calls `pump`, the control and `wait_for_edge`, in a loop. It does not call `poll_events`, and it does not step the clock. So the queue stays full and the frame stays parked until `poll_all` (index 13).
- **The TM-6 claim stays**: index 14 `wait_wake {timeout_ms: 0}` expects exactly `Woken`. The PR adds no `wait_wake` with a timeout. Index 11 (`$one_of [Woken, TimedOut]` at 0 ms) is unchanged.
- **Tier**: HIGH (a new control in `CONTROL_ARGS`: BUILD rule 1; the lead's tier for R-87). The body has the prior art (old Core `72b2e335` has no transcripts) and the Core plan: the id stays FLAKY until a Core PR implements `link_held` on both harnesses and a Linux gate passes it.
- **Fake tier**: the transcript is `fake: structural` (R-12). The body says that the fake tier does not run the new step.

Sibling transcripts (integration check, no finding). These ids also assert a `Woken` after a poll that frees room:
- `tm_6_poll_room_unparks_and_wakes`: the parked work is s2's start transition. AD-7 puts durable identity before the payload launch, so `payload_alive {s2}` `alive: true` proves that Core has read the frame.
- `e3_1_carried_transition_into_a_full_mandatory_queue_is_parked_and_woken_on_room` and `e3_1_runnable_silent_bypasses_a_transition_parked_on_queue_room`: the parked work is s1's `Silent` transition, which Core makes from its clock, not from a frame of the link.

Only `ev_5` has no order between the end of the payload (`payload_alive: false`) and the worker's exit report. So R-87 needs only this fence.

No finding.

### 016c's round 1 on the same head (package scope; recorded, not repeated)

- LOW (body only): the body gives the step numbers 10 and 15, and prior-art step 9, as 1-based counts but does not say so. `pair-common.md` uses 0-based indices, which are 9, 14 and 8. P5 corrects the numbers or labels the count. No source change and no new gate. My CLEAN stays (a package-scope miss). I check the corrected body when P5 sends it.
