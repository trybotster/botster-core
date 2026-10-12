# contracts #102 — stage1/pc-route-frames-taken (R-93 Q1, Core AM-2: `route_frames_taken`), reviewer 017f

HIGH (rules 1 and 3: the shared driver, the shared hub-conformance route driver, the fence lint history). Implementer: PC (sess-…0178). Package reviewer: 016c. PR base: main `49ff52d2`, merged in at `9d595d0`. Gate base: main `5ce2cff1`. Serving Core PR: #319 (in review). Per plan 23am, #319 merges before the pin that takes this tag.

## Round 1: NOT CLEAN on bdad2be37fe53ed539800b238dbf28a7ac8a42ff (2 findings: MEDIUM, LOW)

### The gate

- `~/botster-sessions/gates/botster-contracts-stage1-pc-route-frames-taken-bdad2be3-pool-20261012-132302-46812.log`. My `shasum -a 256` of it is `0afe2cff67c12c5a7c862b4f5f7307bb7b40d9c9167dd26659a04a8dc562dbf5`, the value in the READY.
- All ten steps PASS. Mutants: 5 of 5, then 5 of 5 caught. Exit 0 after 731 s on gaming.
- The earlier `6bb7e86c` gate was red with 3 `route_control` mutants. `abdb5d44` adds the hub-conformance test that catches them.

### What I checked

- **The dispatch** (`driver.rs` `control_matches`). An `await_control` whose control has a `route` that names an attached route goes to that route's endpoint, but only when the endpoint has the op (`RouteDriver::route_control`, hub-conformance `route/driver.rs:298-312`). Every other control goes to the harness as before. `route_reports` names a bound Core route id, and the endpoint does not have that op, so it still goes to the harness. An endpoint error maps to `bad`, a harness error, never `UNSUPPORTED_CONTROL` and never a pass (R-93 condition (b)).
- **R-111.** `route_frames_taken` joins `KERNEL_STATE_CONTROLS`, which R-111 item 1 asked for. Its doc comment names the real-tier kernel reads.
- **The fake route.** `frames_taken` counts each complete frame in `input.rs` before `handle_frame`, and the `route_frames_taken` control reads it. See F2.
- **The vocabulary entry:** `route` is required and `session` is optional.
- **Tests:** `an_await_control_that_names_a_route_asks_that_routes_endpoint` (core-conformance) and `route_control_asks_the_named_routes_endpoint_only_for_its_ops` (hub-conformance). The body reports red on revert 5 of 5.
- **`am_2_one_admission_point_orders_all_sources`.**
  - The new s11 is `await_control route_frames_taken {session: s1, route: r1}` expecting `{frames: 1}`, after the `route_send` A (s10) and before the host `WriteInput` B (s12). That is R-93's follow-up fix.
  - `requires.controls` gains `route_frames_taken`.
  - The `debt:` line for step 15 is removed from `core-fence-exceptions.txt`.
- **Checklist (R-115), item 6:** met. `am_2_one` has no `setup`, so `route_queue_bytes` is the default. It has no `route_gate` and no `route_accept` step, so no limit is in force from r1's first frame (s10) to the fence (s11). It does not flood r1's output. Items 1-5 and 7 are not applicable: there is no clock step, no output, no `oracle_resume` and no `fork_child`.
- **Landing.** Main is now `5ce2cff1` (#99, which also changes `core-fence-exceptions.txt` and `core-testkit-controls.md`). `git merge-tree --write-tree bdad2be3 origin/main` is clean (`4fa8957e`). That is not a rulings-only move, so the landing head needs a main merge, my Q7 receipt and a gate.

### F1 (LOW): the `route_frames_taken` row says that the order lint checks the premise, but no lint does

- `docs/core-testkit-controls.md:131`, the end of the premise text: "Otherwise the order lint (`order.rs`, `unheld.rs`) counts the fence as no fence. A transcript that floods the route's output by other means is listed by hand".
- On this head, `bf354d7` drops the premise check (R-115). `fence_lint/order.rs:605-616` and `fence_lint/unheld.rs:528-539` accept any `route_frames_taken` fence that counts every frame sent on the route, with no raw write. Neither file reads `route_gate`, `route_accept` or `route_queue_bytes`.
- So with a `route_gate` in force, the lint counts the fence as a fence, which is the opposite of what the row says. Under R-115, the premise is checked by reviewer checklist item 6, not by the lint.
- **Fix:** make the sentence say that the fence proves admission only under the premise, and that reviewer checklist item 6 (R-115) checks the premise, not the lint. Then gate the new head. No code change is needed.

### F2 (MEDIUM): the frozen fake route endpoint gets new fidelity

- BUILD.md:48: "The frozen fakes of Phase 0 (FakeCore, the fake route endpoint) stay unchanged, with no new fidelity, until their real counterparts pass the same suites." `docs/fake-replacement-map.md:459` names the same two.
- This PR adds a counter (`Route.frames_taken`, `lib.rs`), counts in `input.rs`, and adds a new control (`controls.rs`). That is new fidelity in the fake route endpoint. I found no ruling that allows it. The steward has refused a fake-route change on this rule before ("The fake route endpoint is frozen (BUILD.md)", R-73, item 4 (B)). R-93 itself followed the freeze for FakeCore ("It gets no buffer and no `pty_queued`").
- No transcript needs it. `am_2_one` is `fake: structural` (R-12), so no fake runs its fence. The only user is the test `an_await_control_that_names_a_route_asks_that_routes_endpoint` (`driver_steps.rs:740-770`), which reads the fake's count to prove the dispatch.
- **Fix:** one of these:
  - remove the fake-route change, and prove the dispatch in `driver_steps.rs` with a test endpoint, as `route_control_asks_the_named_routes_endpoint_only_for_its_ops` already does in hub-conformance; or
  - ask the steward for an exception to BUILD.md:48 before the next round.

### Notes (not findings)

- **The real tier runs a tool on each re-check.** As a kernel-state control, `route_frames_taken` is re-read every 5 ms while `await_control` waits (R-111). On macOS each read runs `/usr/sbin/netstat`. R-93 accepted that path, and the step deadline bounds it. Core #319's evidence should show that the per-read cost fits inside the step budget.

## Round 2: CLEAN on a038119f361105c8a725479b1918c559a49324d9 (F1 and F2 closed)

- **F1: closed** by `bfb9eb75`. The row's premise text now reads: "Reviewer checklist item 6 checks this premise (R-115, contracts main `11b2a5a8`); no lint checks it." The text of the premise is unchanged: no `route_gate` and no `route_accept` limit from the route's first frame to the fence, and `route_queue_bytes` at the default. The same commit also fixes 016c's sentence break in the `await_control` row (`driver.rs:26`).
- **F2: closed** by `a038119f`. `git diff 49ff52d2 a038119f -- crates/botster-fake-route` is empty, so the frozen fake route endpoint is as on main (BUILD.md:48, R-73 item 4). `git grep frames_taken` finds nothing in `botster-fake-route`. The dispatch test now uses its own test endpoint: `Tagged` in `driver_steps.rs` counts the client's writes, one frame per `route_send`, and answers `route_frames_taken` only there. The test's expectations (r2: 2, r1: 0, the endpoint log, the op that the endpoint lacks, the unattached name) are unchanged.
- **No other change.** The delta from `bdad2be3` has 6 files: `driver.rs` (the doc row), `driver_steps.rs`, three fake-route files restored, and `core-testkit-controls.md`. `am_2_one`, the dispatch code, the R-111 list and the vocabulary are as in round 1, so the round-1 checks and checklist item 6 stand.
- **The gate on the exact head is green:** `~/botster-sessions/gates/botster-contracts-stage1-pc-route-frames-taken-a038119f-pool-20261012-135252-5876.log`, sha256 `9bc2b95994773c65558530c972aab90889abe970778460b6aed6a42146389a2d`. All ten steps PASS, with 5 of 5 mutants caught. Exit 0 after 1054 s on linux, base main `192d70bc`.
- **Landing.** Main is now `61f3c128` (#99, #101 and final43). `git merge-tree --write-tree a038119f origin/main` is clean (`12efd100`). The landing head needs that main merge, my Q7 receipt and a gate.

My findings F1 and F2 are closed. CLEAN on `a038119f361105c8a725479b1918c559a49324d9`.
