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

## Merge 1 delta review: no finding on c548c06f129a32f33165ac194940b0b5daa30b1c (the final CLEAN waits for two items)

- `c548c06f` = `00e28fd9` + main `715639eb220e33892253f57c41619ece15cc490f`. The recomputed tree `867ee891b0e80996c9448cfb80c8fe27df3c891d` equals the commit's tree.
- The shared path is `core-fence-exceptions.txt`. The merged file equals main's file minus exactly the PR's three `e3_1 ... step 17 no await_title, R-57` lines. Main's own edits there (the `in_9_kitty`, `ad_3` and other entries) are kept.
- The diff between the merged head and main is the PR's 8 files (+104/-6). The patch-id equals the patch-id of the reviewed diff.
- Main's transcript changes since `5a83c853` (`in_9_*`, `ad_3`) are other ids. They do not touch the seven transcripts of this PR.
- Gate on `c548c06f`: `...-c548c06f-pool-20261011-162001-92488.log` (sha256 `e43a8c3ce88e3c610754ba951d2e0e3d47dcd7cb84b1900652a85eb24dad660c`). The header names head `c548c06f...` and base `d0a69c1e`. Linux node. Ten steps PASS. Exit 0.
- 016c's body LOW is fixed (P5).

Still to do before the final CLEAN:
1. **Main moved again, and not by rulings only.** Main is now `17f8a6e94cf3fff61c4eb2aff4a02f1e5dfd4b3e`. Commit `185e558` changes `conf::ad_3_adopted_equals_started.json`, so BUILD.md's rulings-only exception (one file, `docs/steward-rulings.md`) does not apply. `c548c06f` does not fast-forward main. That merge is disjoint (`ad_3` is not a path of this PR), so it needs no delta review. But the gate and the verdict must be on the exact head that main fast-forwards to.
2. **B13 disclosure (P5).** `e31-whose-parked-probe.log` and `r88-groupA-tiers-mac.log` ran in the shared `targets/p5` with no `cargo clean` and no record that only one worktree built there. A stale binary of another worktree could have run. P5 reruns both from a cleaned `targets/p5` (`r88a-clean-evidence.log`). I check that the rerun gives the same results: 32 of 32 for s2 on `carried_transition`, PASS on both tiers for the four ids, and the `e3_1` failures where they were before.

## Final: CLEAN on e8adac5d6d285ded5bef0a337c89d065ea01dbad

- `e8adac5d` = `c548c06f` (merge 1, reviewed) + main `17f8a6e94cf3fff61c4eb2aff4a02f1e5dfd4b3e`. The recomputed tree `6aedbeae2cc781d87f41f0ba3eb388e2404c8554` equals the commit's tree. The merge's delta is main's delta only: `ad_3_adopted_equals_started.json` and `steward-rulings.md`. Neither is a path of this PR, so the merge is disjoint (Q7).
- Gate on the exact head: `...-e8adac5d-pool-20261011-163257-2092.log` (sha256 `6f0ecca68205ff892ede38889a71166521ef1318361f574c132755eded1850b8`). The header names head `e8adac5d...` and base `17f8a6e9...`. msa1. Ten steps PASS. Exit 0.
- Main is now `1baa9228c3363836d4abe80ad649bae8e42d21c5`. Since `17f8a6e9`, it changed `docs/steward-rulings.md` only, so BUILD.md's rulings-only exception applies and no new gate is needed.
- **B13 closed.** `r88a-clean-evidence.log` (sha256 prefix `fc0d9803`, recomputed):
  - It starts with a logged `cargo clean` of `targets/p5` ("Removed 25934 files", then 0 entries), and only the Core worktree builds there. Core is `2e03346020a63cdc9c069a638170e95cb1416177`.
  - Every transcript blob equals this PR's blob.
  - The four ids `ok` on both tiers. `carried_transition` reaches index 35 in all its runs (32 per-seed runs plus the tier run), so the title fence and the s2 fence hold. No run is INCONCLUSIVE.
  - `runnable_silent` FAILs at 30, `eventless` is unsupported `signals_sent`, and on the real tier every `e3_1` id is unsupported `route_fill`.
  - These results are the same as the earlier logs.
- 016c's body LOW is fixed. I have no open finding.
