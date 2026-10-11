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

## CLEAN on 006ccf22 is ON HOLD (Core testkit evidence; steward ruling needed)

P5 reports BLOCKED. Evidence: `~/botster-sessions/shared/core-stage1/evidence/p5-r87/`:
- `fenced-ev5-poll-testkit-per-seed.log`: Core worktree `stage1/p5-r87-link-held` (base v1 `23550811fb84c3c2cb066019716987575d7aecb2`), with an uncommitted testkit `link_held`, and contracts at this head.
- `trace-old-seed4-testkit.log`: the old transcript at seed 4, with a trace.

On the testkit, the fenced transcript is INCONCLUSIVE at index 9 (`link_held`, `max_polls`) for 10 of 32 seeds: 4, 8, 9, 10, 12, 13, 15, 17, 25 and 28. At those seeds, Core has already read and accepted the exit report, and the parked work is in the engine (`runnable`). It is not a frame that Core holds on the link, so `held` never becomes true. The old transcript passes at those seeds through the runnable wake.

My round 1 checked that the PR matches R-87's text. It did not check that `link_held` covers every form of the parked work that the TM-6 claim needs. The fence as ruled is too narrow for the testkit's scheduling. This is a contract question for the steward: the fence must cover parked work on the link AND parked work in the engine, or use another form.

The PR must not merge on this head. I re-review the head that follows the steward's ruling.

## Superseded: R-87 corrected to `parked_work` (contracts main `7c4004a0ba6c9734fbe8d2072f68addde2d58596`)

The steward withdrew the `link_held` form. My round 1 CLEAN on `006ccf22` is superseded. P5 revises #68, and a new round follows. The corrected control is `parked_work {session}` → `{parked}`: a link frame read but not accepted, OR an engine transition parked on mandatory-queue room. The fence, the pump-and-never-poll rule and the strict 0 ms `Woken` are unchanged. Evidence for the new form: per-seed testkit runs over the same 32 seeds with no INCONCLUSIVE, and a real-tier run.

016c corrected the rule reference of its body LOW: `fence_lint.rs`'s module documentation numbers the steps from 0, as the runner does (not `pair-common.md`). The indices 9, 14 and 8 are unchanged.

### Correction of my sibling analysis (round 1)

On main `7c4004a0`, these transcripts assert a strict `Woken` directly after a poll that frees room: `ev_5_poll_wakes_pending_work` (index 13), `tm_6_poll_room_unparks_and_wakes` (13), `e3_1_carried_transition_into_a_full_mandatory_queue_is_parked_and_woken_on_room` (32), `e3_1_runnable_silent_bypasses_a_transition_parked_on_queue_room` (31) and `e3_1_eventless_effect_overtakes_a_carried_or_parked_step` (40). The other strict `Woken` waits follow a `begin`, an `attach_route`, a `call` or a `control`, which is another TM-6 clause.

- **`tm_6_poll_room_unparks_and_wakes`: my round 1 claim was WRONG.** I wrote that AD-7 makes `payload_alive {s2}` `alive: true` prove that Core has read s2's frame. Core `crates/botster-core-host/DESIGN.md` line 198 says that the worker's `Launched` gives `Running`. The worker sends `Launched` after the payload starts, so a live payload does not prove that `Launched` is written or held. The work that the poll at index 12 frees is s2's `Running` transition (index 15 waits for `SessionState {s2, Running}`). No step before the poll proves that this work is parked. This is the same gap as `ev_5`, and it needs the `parked_work` fence after index 8.
- **The three `e3_1` ids: the work that the poll frees is a `RouteStalled` of `r1`.** Each id waits for it after the poll. Core makes that transition from the route gate and its clock, not from a frame of the link. A `pump` with `more: false` and `events_posted: 0` at a clock where it is due shows that it waits for room. So parked work exists at the poll without a worker report, and those ids need no fence. A late `Launched` of s2 only adds more parked work.

### Steward ruling on the siblings (governs now; to be added to R-87)

- `tm_6_poll_room_unparks_and_wakes` gets the `parked_work` fence after index 8, in #68. Its evidence is the same as for `ev_5`: 32 testkit seeds with 0 INCONCLUSIVE, and a real-tier run.
- The steward accepted my `e3_1` analysis, so the three `e3_1` ids need no fence.

Checks for the revised head:
- the `parked_work` row replaces `link_held` in the controls doc and the vocabulary;
- a fence directly after index 8 in both `ev_5_poll_wakes_pending_work` and `tm_6_poll_room_unparks_and_wakes`;
- each strict 0 ms `Woken` is unchanged;
- no `wait_wake` with a timeout is added;
- the 32-seed testkit log and the real-tier log for both ids, each with the Core revision and the contracts head in its header.

## Round 2 (R-87 corrected: `parked_work`): CLEAN on df66aafa80c9d55b00664c0c2a279c96c12156c1

The head has two commits since `006ccf22`.
- `895a5cc92d3096bf022fbb33eb5d48dd0f7130d7` merges main `7c4004a0` (docs only). The recomputed tree `8013cdbfa31f79b5f703bef154edc6a72d92fe82` equals the commit's tree, so the merge is automatic.
- `df66aafa` is the own delta: 4 files.

Checks:
- **Control row.** `parked_work {session}` gives `{parked}`. It is true for a link frame read but not accepted (EV-5b), or for an engine transition parked on mandatory-queue room. It is false for work that is runnable now, or that waits for anything else. It is read-only, and both harnesses answer it. It replaces `link_held`, and it states the difference from `held_reports`. This matches the corrected R-87.
- **Vocabulary.** `CONTROL_ARGS` and `CONTROLS` rename the control. The only `link_held` left in the tree is the row's note that `parked_work` "replaces the withdrawn `link_held`".
- **Fences.**
  - `ev_5_poll_wakes_pending_work`: index 9 is `parked_work {s1}` `parked: true`, directly after `payload_alive {s1}` `alive: false`.
  - `tm_6_poll_room_unparks_and_wakes`: the new index 9 is `parked_work {s2}` `parked: true`, directly after `payload_alive {s2}` `alive: true`. `requires.controls` adds the control.
  - In both ids, the strict 0 ms `wait_wake` → `Woken` is at index 14 and is unchanged. Index 11 (`$one_of`, 0 ms) is unchanged. No `wait_wake` with a timeout is added.
  - `await_control` pumps and never polls (driver line 3029, unchanged).
- **016c's LOW.** The body now uses 0-based indices.
- **Evidence** (`~/botster-sessions/shared/core-stage1/evidence/p5-r87/`; I recomputed the sha256 prefixes):
  - `parked-work-testkit-per-seed.log` (`b1eb1358...`): one seed per run, seeds 0-31, both ids. 64 PASS, 0 INCONCLUSIVE. The run used Core `f5e6a2be` with the uncommitted `parked_work` code, and contracts `895a5cc` with the 4 changed files uncommitted.
  - `parked-work-tiers-mac.log` (`87c4da32...`): Core `9e5f165a6a3d6389e965a0af082dffb55a086183` (clean), and contracts `.39` plus only the two transcripts. Their blobs `fc56213b` and `84e922be` equal `df66aafa`'s blobs. Both ids PASS on the testkit and on the real tier.
  - `parked-work-forms-revert.log` (`c0005efa...`): one form removed at a time. Without the engine form, `ev_5` passes 22 of 32. Without the frame form, `ev_5` passes 10 of 32 and `tm_6` 0 of 32. All 64 failures are INCONCLUSIVE at index 9. So each form is needed, and the fence detects each one.
- **Gate.** `...-df66aafa-pool-20261011-150744-98930.log` (sha256 `b1dbe170491541d3244db138f40e36cec0d60085559b82e3ce14b85855c9b6b5`). The header names head `df66aafa...` and base `7c4004a0`. msa1, allocation `90f0b3c3`. Ten steps PASS. Exit 0.
- **Tier.** HIGH (a new control). The body states the prior art.

No finding. Still to do: main is now `52f1b07fa52198057c12c8e7f9bb3a3058cac7a7` (#67, `fence_lint.rs` only, disjoint from this PR). P5 merges it once (Q7), and the gate runs on that exact head.
