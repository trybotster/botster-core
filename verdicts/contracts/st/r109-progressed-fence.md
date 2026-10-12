# contracts #99 — st/r109-progressed-fence (R-109: `route_reports {progressed}` and the report fences of the host-owned deadlines), reviewer 017f

HIGH. Implementer: ST (sess-…0180). Package reviewer: 016c. PR base: main `11b2a5a8`. Gate base: main `216ea74e` (BOTSTER_CI_BASE_REF). One commit, `f372188b`. Fences only (R-115): no lint code. 18 files: 16 transcripts, `core-fence-exceptions.txt`, `docs/core-testkit-controls.md`.

## Round 1: NOT CLEAN on f372188b7a67b4767f1fe25ce8ea92267364d4d9 (1 finding, MEDIUM)

### The gate

- `~/botster-sessions/gates/botster-contracts-st-r109-progressed-fence-f372188b-pool-20261012-123707-27759.log`. My `shasum -a 256` of it is `e988fad3e55d6848eeaac6f375dbc4a6a4261bdf5a02b97d735b5db86471b4db`.
- All ten steps PASS. Exit 0 after 692 s on linux. Ledger 2550 ids.

### What I checked

- **The step diff.** I compared each changed transcript step by step against `11b2a5a8`. The diff is exactly what the body lists:
  - 17 `await_quiet` → `await_control {route_reports, session: s1, route: $r1} expect {progressed: true}` (8 in `ou_3_progressing_reader_lossless`, 1 in `ou_3_progress_is_bytes_accepted_by_the_kernel` s18, 8 in `ou_7_progressing_slow_route_delays_exit_lossless`);
  - 13 outputs gain the sentinel `1b5d323b 723130392d31 1b5c` (`ESC ] 2 ; r109-1 ESC \`; `st-r109-1` in `ou_3_progress_is_bytes`), and each `await_quiet` after it becomes `await_title`;
  - `ou_3_other_readers_continue` s19 gains the same 12 bytes on r2;
  - `e3_1_runnable_silent_bypasses_a_transition_parked_on_queue_room` gains s19-s20 (`ReadScreen` pumped through its `Completed`);
  - `requires.controls` of the three `progressed` ids gains `route_reports`.
- **Exceptions file.** Four lines removed (`ou_3_progress_is_bytes` s12; RC's debts `a2_3` s145, `ou_2b_every` s145, `ou_2b_failed` s39). Each one's output is now title-fenced. P5's four `e3_1_runnable_silent` lines move 28, 30, 31, 32 → 30, 32, 33, 34 with the same text.
- **Controls doc.** The `route_reports` row gains `progressed` with R-90 Q2's text and R-109's scope.
- **The new `TitleChanged` events.** `await_title` reads the model title and polls no events. After each fence, I found no `exhaustive` `pump_collect`, no `events_posted` count, and no `forbid` of `TitleChanged`. Titles reach routes as `output` frames (there is no title frame). Every exact route read after a sentinel is safe: `ou_3_other_readers_continue` s24 reads r2 after r2 took the sentinel at s19, and `ou_3_stalled_reader_resyncs` s24 reads after the resync.
- **My own scan for items 1 and 2**, over all `conformance/core/*.json`. I tracked `route_gate`, gated `pty_output`, `await_title`, `progressed` fences and `advance_clock`.
  - Item 1: no `route_accept` → `advance_clock` without a `progressed` fence.
  - Item 2: no gated output → `advance_clock` without a title fence, except the two `a3_1` derived-event sites that the body names (`a3_1_output_dropped_waits…` s15 → s20, `a3_1_stall_resync_reports…` s14 → s17).
- **Core source lines on #298 `c1c6363d`.** I read each one:
  - `worker.rs:426` flushes route progress before every report; `:440-446` `on_output` calls `route_output` before `feed_model`;
  - `worker/model.rs:179` `Bell`, `:183-197` `Notification`, `:235` `report`;
  - `worker-core/DESIGN.md:215-221`, the order rule;
  - `inbound.rs:474-490` (silence armed and `Activity` posted in one consume), `:446-455` (capture `expires`);
  - `stall.rs:7-10` (the arming rule), `:237-250` `route_stall_posted`.

### The reviewer checklist (R-115)

1. **`route_accept` → `advance_clock`:** a `progressed` fence at every pair. For the scope, the gate is on and nothing else frees bytes between pairs. **But see F1** for `ou_7` pairs 7 and 8.
2. **Gated output → `advance_clock`:** 13 title sites. Each title is new in its transcript, and it is the last bytes of the output (R-113). The 2 derived-event sites meet the follow-up's three conditions: the event ends the wait, R-113's position holds (the earlier `BEL` was polled as `Bell`), and the worker order is shown with source lines. Met.
3. **Silence:**
   - `e3_1_runnable_silent`: the sync read (s19-s20) comes after `await_title` s18 and before the clock steps. Met. This is R-114's form, so it needs Core #325's `DESIGN.md` rule in the pin, as item 5 does.
   - The 8 one-byte ids: each output is `78`, and the next step is `pump_until Activity {S, Output, at}`. Each one is distinct by session or by `at` (`tm_4` s21 has `at: 8000`, after the first at 7000). Met.
4. **Stall close and capture expiry:** every clock step past `stall_close_after` follows a consumed `RouteStalled`. The capture deadline is armed in the consume that completes the operation. Met. No change is needed.
5. **`oracle_resume`:** none in the changed transcripts. Not applicable.
6. **`route_frames_taken`:** none. Not applicable.
7. **`fork_child` title:** none. Not applicable.

### F1 (MEDIUM): `ou_7_progressing_slow_route_delays_exit_lossless` s35 and s38 wait for progress that cannot come

- The program prints 163,840 bytes (s0 `print_after_input`; s41 expects exactly 163,840 bytes on r1). The transcript has no `route_fill`, and on the testkit C is 0 while the gate is on (`route_fill` row).
- The eight `route_accept` steps allow 8 × 32,768 = 262,144 bytes. Accepts 1-5 (s16-s28) allow 163,840 bytes. Frame headers and the `route_closed` frame add only a small number of bytes, so accept 6 (s31) takes the rest.
- So accepts 7 (s34) and 8 (s37) find no frame to send. The transport accepts no byte, and the worker sends no `RouteProgress`: "A progress that leaves the route not waiting before and after the step is not reported" (`worker-core/DESIGN.md` on #298). `progressed` is defined as a report "that happened after the route's latest `route_accept`". So s35 and s38 wait until the step deadline. The old `await_quiet` passed here, because it needs no progress.
- The body says that the three `progressed` ids were not run, because Core does not serve the field yet. So no run has tested this.
- `ou_3_progressing_reader_lossless` does not have this problem. Its `route_fill` writes F = `max_frame_bytes`, and that default is `max_route_frame_bytes` = `max_paste_bytes + 4096` (`limits.rs:232-234`). That alone is more than the 8 × 4,096 bytes that it accepts.
- **Fix.** R-109 says: "If a pair cannot meet the scope, ST asks before choosing another fence." Send the count to the steward with one of these options:
  - give no fence to the pairs after the queue drains (no reader deadline runs for a route that does not wait), and keep `await_quiet` there;
  - make the program print more than 7 × 32,768 bytes, so that every accept takes bytes;
  - use fewer or smaller accepts that still pass 1000 ms (`reader_progress_deadline`) in total.
  
  Alternatively, show with a run on a Core scratch branch that serves `progressed` that s35 and s38 pass, and explain why.

### Open items (not findings)

- **Main moved.** Main is `3d590705`. Since `11b2a5a8` it has the R-115 correction, the #97 merge (three `st_6b` transcripts) and the R-106 follow-up in rulings. The paths do not overlap with this PR. This is not a rulings-only move, so the landing head needs a main merge, my Q7 receipt and a gate on that exact head.
- **At the Core pin:** the serving Core PR for `progressed`, and Core #325 for the `e3_1_runnable_silent` sync read (item 3) as for item 5.

## Final: CLEAN on c3e6a0b1cfcbfae58f54a6f271c1938e9bc4f1fd (F1 closed)

- **The rulings.** Main `e5709067` takes ST's option (c) for `ou_7` and restates the scope. Main `80f022e9` redefines `progressed`: Core has consumed, after the latest `route_accept`, a `RouteProgress` whose mark is above the mark that Core had at that accept. The liveness condition is that, at each fenced accept, the worker still holds queued bytes and the client takes no byte outside the grants.
- **F1: closed** by `9663659` and `c3e6a0b1`. `ou_7` keeps grants 1-5 (s16-s30). Old s31-s39 (grants 6-8 with their fences and clock steps) are removed, and the gate goes off at s31. s13 (`pty_output_unread` not 0, with `route_queue_bytes` 131,072) proves that more than 131,072 bytes wait outside the transport. So grants 1-4 take at most 131,072, and grant 5 still finds bytes. 5 × 500 ms is still past `reader_progress_deadline` 1000. `core-fence-exceptions.txt` has no change in this delta. Its one `ou_7` line (step 11) is before the removed steps.
- **The `route_reports` row** in `docs/core-testkit-controls.md` states the `80f022e9` definition and the liveness condition, and names the ruling.
- **Checklist item 1 (updated text):** the body gives a per-id proof of queued bytes at each fenced grant. For `ou_3_progressing_reader_lossless` and `ou_3_progress_is_bytes_accepted_by_the_kernel`, the proof is the `route_fill` frame of F = `max_paste_bytes + 4096`, which is more than the grants after it. For `ou_7`, it is s13. There are 14 fences in all (8 + 1 + 5). Met. Items 2-7 are unchanged from round 1.
- **Q7 of the two merges:** `f2b44e90` (main `e5709067`) has tree `def97b99`, and `c497ee03` (main `80f022e9`) has tree `a9405384`. Each tree is equal to `git merge-tree --write-tree` of its parents. Main's side is #97, #98 and rulings, and no path is shared with this PR.
- **The gate on the exact head is green:** `~/botster-sessions/gates/botster-contracts-st-r109-progressed-fence-c3e6a0b1-pool-20261012-130409-39041.log`, sha256 `9d4c8c6ccf34c31656cc35440dc56d9b162d998fa5d40d083348f74b95ac2d4e`. All ten steps PASS. Exit 0 after 544 s on gaming, base main `80f022e9`.
- **Core evidence:** with Core #298's testkit, which already serves `progressed`, both `ou_3` ids pass on seeds 0-31. `ou_7` cannot run there (`UNSUPPORTED_CONTROL pty_output_unread`).

### Landing note

Main is now `49ff52d2`. #100 (`botster-core-conformance` `driver.rs`, `lib.rs`, `tests/driver_steps.rs`) landed after this head's base. That is not a rulings-only move, so the landing head needs a main merge, my Q7 receipt and a gate on that exact head. The paths are disjoint.

My finding F1 is closed. CLEAN on `c3e6a0b1cfcbfae58f54a6f271c1938e9bc4f1fd`.
