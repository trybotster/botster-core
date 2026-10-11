# Contracts #71 (`stage1/p5-r88-parked-fences`): integration review (R-88 group (A), P5)

Reviewer: contracts integration reviewer (017f). Implementer: P5 (`sess-1791517266-0166-9e69a1c3914b450c0b8a98aa5c29e374`). Package reviewer: 016c. Tier: HIGH (transcripts of ids that are live on both Core tiers, and the exception file that the lint reads).

## Round 1: no finding on 00e28fd92215bceec96ee1c0fa2b01440134a92e (final CLEAN after the main merge and its gate)

One commit on main `5a83c853d4bcebe191e0ae7492eb1020b9c35281`. The own delta is 8 files: 7 transcripts and `core-fence-exceptions.txt` (-3).

Checks:
- **`parked_work` fences (R-87 form, R-88 (A)).** Each one is `await_control {parked_work, S}` `expect {parked: true}`, `because` EV-5 and TM-6, and `parked_work` is in `requires.controls`. Each is placed before the read that the sweep names:
  - `ev_5_full_queue_defers_transition_not_output`: index 17, s2, after `payload_alive {s2}` `alive: true`, before the `get` at 18.
  - `ev_5_stop_kills_while_queue_full`: index 9, s1, after `payload_alive {s1}` `alive: false`, before the read at 10.
  - `tm_6_more_means_runnable_not_blocked`: index 9, s2, before the `pump` at 10 and the `get` at 11.
  - `tm_6_parked_work_does_not_spin`: index 10, s1, after the `advance_clock` at 9 (R-88: "after s9").
- **The `e3_1` title fences (R-57).** Index 17's `pty_output` `78` gets an OSC 2 sentinel as its last bytes, ended by ST:
  - `carried_transition`: `r88-e3-carried`;
  - `eventless_effect`: `r88-e3-eventless`;
  - `runnable_silent`: `r88-e3-silent`.

  The new index 18 is `await_title s1`. It has no poll and no clock step. The `await_quiet` and the `drain` that follow it remove the `TitleChanged` event before the queue is filled. The three R-57 exception lines are removed (shrink-only).
- **Whose work is parked (`carried_transition`).** `e31-whose-parked-probe.log` (sha256 prefix `1163bb63`, recomputed) has three sections of 32 seeds each:
  - with no fence, FAIL at index 34 in 32 of 32;
  - with an s1 fence, INCONCLUSIVE at the fence (index 32, `max_polls`) in 32 of 32;
  - with an s2 fence, FAIL at index 35 in 32 of 32: the fence holds, and the id fails later, as it did before.

  So s2 is the parked session, and the PR fences s2 only.
- **`eventless_effect` and `runnable_silent`.** Title fences only, as main's R-88 rules (line 39). Their `parked_work` fences are added and proved in the PR that un-pends each id.
- **Core evidence.** `r88-groupA-tiers-mac.log` (sha256 prefix `45c6970d`, recomputed). Core is `98fe6fde4ab261fd66f0a5960384285d34f3551c`. All seven transcript blobs equal this head's blobs.
  - The four live or pending ids PASS on the testkit (`BOTSTER_SEEDS=0-31`) and on the real tier (seed 0).
  - The three `e3_1` ids fail where they failed before: `carried` at 35 after both fences hold, `runnable_silent` at 30, `eventless` with `signals_sent` unsupported. These failures are why they are pending.
- **Gate.** `...-00e28fd9-pool-20261011-155337-72679.log` (sha256 `d419873567644a3c7de66a4fb266b599877f1414dbb1347ac6a69974dcd24e10`). The header names head `00e28fd9...` and base `08ce74b6`. msa1. Ten steps PASS. Exit 0.

### Correction of my R-87 sibling note

At #68 I wrote that the three `e3_1` ids need no `parked_work` fence, because their freed work is r1's `RouteStalled` (s1). The steward accepted that note. P5's probe shows it is wrong for `carried_transition`: s1's fence never matches (32 of 32 INCONCLUSIVE), and s2's work is parked in 32 of 32 seeds. R-88's "The parked session is not assumed" was the right rule, and this PR applies it.

### Still to do

- Main is now `a66bed839aec9b26388e261df59936c0eebcc8c6`. Since `5a83c853`, main changed `core-fence-exceptions.txt`, the `in_9_kitty` and `ad_3` transcripts, `BUILD.md` and the rulings. The exception file is a path of this PR, so the merge needs a delta review (Q7).
- Then one gate on the exact merged head.

### 016c's round 1 on the same head (package scope; recorded, not repeated)

- LOW (body): the prior-art section says that the `st_6_pages` R-57 title fence is in #67. It landed in #54; #67 adds the CSI push/pop lint self-tests. P5 corrects the PR number. My round 1 missed this. I check the corrected body at the final step.
