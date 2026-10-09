# P5 process controls review

## PR #191 — Round 1 — 2026-10-09

Exact head: `504540a93bebadb665562e2188ba06c3c2bb40ba`.
Exact tree: `5d76246a2b2e72ff2278a7cf231481d7df74b332`.
PR and gate base: `3000ae14bf8b05efe410c22ac66d5915cf36ff45`.
Branch: `stage1/p5-controls-process`.

The reviewer checked the risk tier first. The PR states HIGH under rule 3 because it changes the shared testkit.
The PR includes Prior art and names the supplied exact-head gate.
P6 approved the control design, with deny_unknown_fields and typed Unsupported handlers for controls that wait.

### PC-F1 — MEDIUM — OPEN — A dropped handle can still break a worker link

The new handle_dirs map records each successful open.
TestkitHarness::drop_handle remains a no-op and does not remove that record.
The new session_row helper uses the record to resolve a session's registry row.

The failure sequence is:

1. Open handle h and create and start session s.
2. Drop its Core and call harness.drop_handle("h").
3. Call break_control for handle h and session s.

Core LC-12 keeps the worker and registry row after the Core drops.
The stale handle_dirs record therefore lets session_row find the row and Workers::break_link accept the worker identity.
The control returns success for a handle that is gone.
CoreHarness::drop_handle explicitly says that the handle is gone.
The contracts fake removes its handle record and rejects controls for that handle with ControlError::Bad.
The conformance driver removes the Core and calls drop_handle, but sends control steps directly to the harness.
It does not prevent this stale lookup.

P5 must remove the handle_dirs record in drop_handle and update its stale comment.
P5 must add a behavior test that rejects break_control after the handle drops.
The test must also show that a valid reopened handle can address the surviving worker.
The fix must preserve the worker and storage required by LC-12.
The reviewer sent PC-F1 directly to P5 and copied integration.

### Source review and supplied evidence

The reviewer read the complete ten-file delta and the relevant host, worker, harness, and conformance driver code.
The control closes the worker link at the testkit edge and delivers worker Input::LinkClosed through the normal scheduler.
The worker machine keeps its payload on LinkClosed.
The process table records the host wake, and the control signals that wake after it sets the break input.
The parser removes only the top-level step keys and rejects unknown arguments through deny_unknown_fields.
The row helper uses the host's Row::decode and the stored worker identity.
Eleven controls that wait return the typed ControlError::Unsupported.
The PR adds no production machine change or real-process test.

The only pending-list removal is conf::lc_5_stop_with_broken_control. No id enters the pending list.
This id belongs to the 70 minimum ids.
Its replacement-map proof is core-testkit+edge with the process edge; it is not a real-only slow proof.
The supplied gate reports that exact conformance trial PASS.
The manifest and lockfile add only the existing workspace serde dependency to botster-core-testkit.
No contracts pin changes.

The supplied Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/controls-process-504540a9.log`.
It names the exact reviewed head and base.
The default tier reports 953 passed and 651 skipped.
The slow tier reports 243 passed and 967 skipped.
The mutation run tests 25 mutants: 19 caught, zero missed, zero timeouts, and six unviable.
All ten CI stages pass. Fuzz runs no decoder harness for this delta.
The job and gate exit zero.
The five new control tests pass, but none checks a dropped handle.

PC-F1 remains open. Integration owns its separate review of this HIGH PR.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: NOT CLEAN
