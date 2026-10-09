# Integration review: #187 (a worker exit seen before its remove result keeps the result; branch stage1/p5-remove-exit-order)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head f545722f

Reviewed head: `f545722f94ce97fff087d66c88a62c494f0d1d1a` (`ef598727` the fix, `cf224e3b` the pending flips, `f545722f` the
merge of v1 `465978d6`, which is the current v1). The stated tier is HIGH by rule 5 (`inbound.rs` and the Remove flow are
on the HIGH-path list), which is correct. P5's gate log: `remove-exit-f545722f.log`. This reviewer did not read it.

### Checked, no finding

- **The merge.** `git merge-tree --write-tree cf224e3b 465978d6` conflicts only in `conformance/core-pending.txt`, and the
  real merge differs from the trial tree only there. Against v1, the pending list loses exactly `am_3_exactly_one_completion`,
  `lc_7_remove_order_and_completion` and `or_2_session_order`.
- **The AwaitTeardown path.** After `HostMsg::Remove`, an exit seen with the link open sets `worker.gone` and
  `f.worker_gone`, keeps the link, and calls `remove_progress`. A `RemoveResult` read later sets the uploads. The end of file
  calls `flow_remove_worker_gone` (OutcomeUnknown only when no result came). The grace still bounds this path:
  `remove_grace_expired` sets OutcomeUnknown, and `f.worker_gone` is already true. A probe of a gone worker sends no signal,
  because `flow_remove_probed` skips a session whose worker is gone. The new test covers both results.

### R1 MEDIUM — an exit before `SendRemove` makes the removal wait for the end of file with no bound

The early return in `on_process_exited` (`inbound.rs:668-679`) applies to every phase of `Flow::Remove`, not only to
`AwaitTeardown`. The worker can write a remove result only after `HostMsg::Remove`, which `SendRemove` sends. An exit seen in
an earlier phase (for example `CloseRoutes`, a step that runs between pumps) is a crash, and its result can never come.

The sequence at the head:
1. Remove begins on a session with a route. The phase is `CloseRoutes`.
2. The worker process ends. `on_process_exited` sets `worker.gone` and `f.worker_gone = true`. It keeps the link (not closed,
   no `link_failed`) and returns.
3. `SendRemove` runs. `send_msg` (`engine.rs:307`) returns true whenever `worker.link` is `Some`, so the first branch
   (`flows.rs:523`) is taken: `(None, false, deadline)`. Then `f.worker_gone = false`, and the uploads are `None`.
4. If the link's end of file comes, `on_link_closed` ends the flow (the new `worker.gone` arm). If it does not come, then at
   each `stop_grace`, `remove_grace_expired` sets OutcomeUnknown and probes the identity. But `flow_remove_probed` skips the
   session (`!s.worker.gone`), so `f.worker_gone` stays false. `remove_progress` never advances, and the probe repeats
   forever. The Remove never completes, and the session id stays taken.

Before this PR, the exit closed the link in every phase, so the Remove did not depend on the end of file. The new comment
says "the remove grace still bounds it", which is false on this path. The end of file comes only when every copy of the
worker's link descriptor is closed. Core does not prove that anywhere, and the old code did not need it.

Fix (one of the two):
- Take the early return only when `f.phase == RemovePhase::AwaitTeardown` (the result can come only after
  `HostMsg::Remove`). In other phases, keep the old path.
- Or make `SendRemove` keep `worker_gone` true when `worker.gone` is true, and not send `HostMsg::Remove` to a gone
  worker.

Add a `World` test: an exit seen before `SendRemove` with the link open and no end of file. The Remove must complete after
one `stop_grace` with OutcomeUnknown. At the head, that test hangs (the teardown waits).

The P5 package reviewer's REO-F1 (a real-process proof) is theirs and is not counted here.

VERDICT: NOT CLEAN at f545722f94ce97fff087d66c88a62c494f0d1d1a (1 open: R1 MEDIUM)

## Round 2 — CLEAN on head 2b5ba07e

Reviewed head: `2b5ba07e5bcff471370fab07e2436a103cfef91b`, one commit on `f545722f` (`inbound.rs` +6 -3, `flow_edges.rs`
+34). v1 is `a14e9dc2` (#186 since the branch's merge). #186 changes only `guard_platform.rs`, and
`git merge-tree --write-tree origin/v1 2b5ba07e` has no conflict.

- **R1 closed.** The early return now needs `RemovePhase::AwaitTeardown`. An exit in an earlier phase takes the old path:
  it closes the link, sets `link_failed`, and `flow_remove_worker_gone` sets `worker_gone` and OutcomeUnknown. Then
  `SendRemove` finds no link and takes the `(Some(_), true)` branch: OutcomeUnknown, `worker_gone` true, no deadline, and no
  `HostMsg::Remove`. So the Remove does not depend on the end of file. In `AwaitTeardown`, the grace bounds the wait as in
  round 1.
- **The new test** begins the Remove with no pump (so before `SendRemove`), feeds the exit with the link open and no end of
  file, and asserts completion with OutcomeUnknown, the session removed, and no `HostMsg::Remove` sent. At `f545722f`, the
  head sent `HostMsg::Remove` on this path, so the last assertion fails there.
- **No new real-process test code**, as the HOLD requires (the lead's ruling). The P5 package reviewer closed REO-F1 on the
  lead's proof boundary: the delta changes only `HostEngine` decisions over ordered inputs.
- **The gate log** (`remove-exit-2b5ba07e.log`) names the head and the base `a14e9dc2`. The slow tier runs 243 tests, all
  pass, and the mutants step reports 5 caught, 0 missed, 0 timeout.

VERDICT: CLEAN (0 open) at 2b5ba07e5bcff471370fab07e2436a103cfef91b
