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

## PR #191 — Round 2 — 2026-10-09

Exact head: `7a56100cd97b0d46f501779ac134369756170be1`.
Exact tree: `9c359a5d52164d326c12f47d509645b903ec509e`.
PR and gate base: `67fd748a369d0ed544e3373d20c887024af31fa8`.
Parents: `7d240117fab9d0f2205ac1c345cd5778d4df72f9` and `67fd748a369d0ed544e3373d20c887024af31fa8`.
Branch: `stage1/p5-controls-process`.

The reviewer checked the risk tier first. The PR retains HIGH under rule 3.
The PR retains Prior art and now names the exact Round 2 head and gate.

### PC-F1 — MEDIUM — CLOSED — The harness removes the dropped handle

TestkitHarness::drop_handle now removes only the handle_dirs entry for the dropped handle.
The method preserves the registry rows and workers required by LC-12.
The method's comment now describes the handle removal and the preserved state.
The session_row helper therefore rejects a dropped handle with ControlError::Bad.

The new test is a_dropped_handle_is_gone_and_a_reopen_reaches_the_surviving_worker.
It creates and starts s1 under handle a, drops the Core, and calls drop_handle("a").
It then asserts that break_control under handle a returns Bad.
It reopens the same directory under handle b and asserts that break_control reaches the surviving worker and returns success.
The supplied exact-head gate reports this test PASS.

### Merge delta and supplied evidence

The fix changes only harness.rs and process_controls/tests.rs from the Round 1 package source.
The merge imports only xtask/src/base_merge.rs and xtask/src/base_merge/tests.rs from v1's #190.
The reviewer verified that the imported binary diff equals the exact old-base-to-new-base diff.
The PR's binary diff against the new base equals the fixed PR's diff against the old base.
The merge therefore preserves the complete package change and imports the base change without edits.
This review does not replace #190's separate review.

The pending list, testkit manifest, and lockfile are byte-identical to Round 1.
The single minimum-id removal and its replacement-map classification remain valid.
The gate reports conf::lc_5_stop_with_broken_control and all six process-control tests PASS.
No new package finding arose.

The supplied full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/controls-process-7a56100c.log`.
It names the exact reviewed head and base.
The default tier reports 966 passed and 651 skipped.
The slow tier reports 243 passed and 968 skipped.
The mutation run tests 26 mutants: 20 caught, zero missed, zero timeouts, and six unviable.
All ten CI stages pass. Fuzz runs no decoder harness for this delta.
The job and gate exit zero.

Every package finding is closed. Integration must supply its separate CLEAN before this HIGH PR merges.
This verdict does not close #176's proof hold or establish real-process minimum conformance.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
The lead owns the merge decision.

VERDICT: CLEAN
