# Contracts #69 (`rc/r66-ev5-route-reports-fence`): integration review (R-66 follow-up, RC)

Reviewer: contracts integration reviewer (017f). Implementer: RC (`sess-1791701696-017d-d5022b29376b8c0aaeecc0885355c8ba`). Package reviewer: 016c. Tier: HIGH (BUILD rule 1: a transcript decides the Core gate).

## Round 1: NOT CLEAN (1 LOW) on 06488fb53bfdbd3deeefc9ba067a1ddd2276c8ea

The branch is based on main `7c4004a0ba6c9734fbe8d2072f68addde2d58596`. One commit, `06488fb`, changes `conf::ev_5_route_closed_retires_route_keyed_events.json` (+11/-1). Main is now `c0c2abc09f208fea39896972373da53cb90dd830`. The final step merges main once (Q7).

Gate: `~/botster-sessions/gates/botster-contracts-rc-r66-ev5-route-reports-fence-06488fb5-pool-20261011-152011-74085.log` (sha256 `cdc48dbb32db858eff8c41a75b72fbca2126fbffbd6553efcc8ff9fb156b59c7`). The header names head `06488fb5...` and base `8c8d52ef`. msa1. Ten steps PASS. Exit 0.

Checks:
- **The change.** Index 25 `await_quiet` becomes `await_control {route_reports, s1, $r2}` with `expect {activity_before_close: true, closed: false}`. Index 26 (`Detach`) and every other step are unchanged.
- **The fence ends on the right fact.** `activity_before_close` is true once Core consumes a client-activity report of r2 before r2's close report, and it stays true. So the wait cannot end before the report is consumed. `closed: false` holds before the `Detach`. The wait pumps and never polls (R-63, R-66), so the `Activity` event stays unpolled for the retirement check at index 29.
- **R-66 scope.** `route_reports` here is a precondition fence on an open route. It is not a fence for a `RouteClosed` post, so the post-fence conditions of the R-66 correction do not apply. Index 27 (`closed: true`) is unchanged. R-66 item 2 says that the `await_quiet` "may stay". That allows the step but does not require it, so this PR does not conflict with the ruling.
- **The defect evidence.** I checked the cited lines: Core #285 gate log lines 5268-5270 (seed 0), and the PC A5-4 log lines 2024-2026 (seed 7). Both show `step=28` with `activity_before_close: false, closed: true`. The controlled reproduction (the worker holds r2's input for 1 s) shows the same signature.

### Findings

- **F1 LOW (evidence: no run of the fenced form).** The body shows the failure three times, but no Core run of the changed transcript. This is the same requirement as #54 F3: evidence for every changed transcript. Required:
  - the controlled reproduction (the 1 s held input on v1 `f68f26a7`) run with the fenced transcript, giving PASS. This is the direct proof that the fence closes the window.
  - the fenced transcript on the Core testkit with `BOTSTER_SEEDS=0-31`, and on the Linux real tier, with the Core revision and the contracts tree in each log header, and the transcript blob equal to this head's.

## Round 2 (evidence only, same head): no open finding on 06488fb53bfdbd3deeefc9ba067a1ddd2276c8ea

- **F1 closed.** Log `~/botster-sessions/gates/botster-core-detached-f68f26a7-f68f26a7-pool-20261011-153357-47910.log` (sha256 `ab3f90982a6672ec8375a62ceb4763c28204419693fa30fd7e9dac569280483d`), Linux msa1, exit 0.
  - Core is `f68f26a7b12738f2b19ab39fe76a93f3d1a72f12`, and contracts is `2c7682f4fe6009fc27a9b6cbef656e11e6e5be1b` (`.39` + this PR's commit).
  - I checked the blob: `2c7682f4:conf::ev_5_route_closed_retires_route_keyed_events.json` is `3d9a76ccc41311348cd702237df57be75cc316ad`, which equals `06488fb5`'s blob. That replaces the job's empty blob line (no python3 in the pool image).
  - (a) The 1 s held-input reproduction, real tier, `BOTSTER_SEEDS=0-31`: PASS (32.50 s). The trace has "held: 32 released: 32", and each release comes before r2's `RouteClose` (`held_inputs=0`). So the fence waits for the consumed activity report in the exact window that failed before.
  - (b) The plain worker, `BOTSTER_SEEDS=0-31`: testkit PASS and real tier PASS.
  - The diff hash `f80397a6...` differs from the accepted `027d7d7a...` only by the index abbreviation length (7 against 8), as RC states.

Still to do: one merge of main `c0c2abc09f208fea39896972373da53cb90dd830` (Q7), and the gate on that exact head. Then the final CLEAN.
