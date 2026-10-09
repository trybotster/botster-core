# P5 adoption controls review

## PR #204 — Round 1 — 2026-10-09

Exact head: `6b5372ac24d1d9f4b05c365d4e5da264952b0b54`.
Exact tree: `907f3ae6a20c7913c0d144ef90e6dd8a9992d182`.
PR and gate base: `cd97009e93c2641843c05bd46793265b3580b2a9`.
Parents: `634e555fc2efac21ac5bb0f64992030b7c4c6cec` and the base above.
Branch: `stage1/p5-adopt-a2-v1`.

The reviewer checked the risk tier first. The PR states HIGH under rule 3 because it changes shared crates.
The PR includes Prior art and an exact-head gate log.
The reviewer read the complete 14-file source delta and the new tests.

### A2-F1 — HIGH — OPEN — Reopen loses failed-proof signal protection

adopt_hello sets WorkerHandle.gone to true after a failed proof and persists Lost(WorkerGone).
The row retains the recorded worker identity.
session_of_row creates a fresh WorkerHandle with gone=false, restores that identity, and posts the Lost row without a handshake.

The failure sequence is:

1. A starts the session. An impostor replaces its endpoint answer.
2. B's AdoptAll rejects the proof and persists Lost(WorkerGone).
3. Drop B. Open C over the same directory.
4. C's AdoptAll posts the recorded Lost outcome without a handshake.
5. C's Remove sees a recorded identity and gone=false, so it sends ProbeIdentity.
6. If the recorded identity matches, flow_remove_probed sends GroupSignal::Kill.

This contradicts A10-1 and the PR's stated residual case: Core must not signal the process that it cannot authenticate.
Recover the no-signal state from the recorded WorkerGone outcome, or make removal honor that outcome directly.
Prove refusal, persistence, reopen, AdoptAll, and Remove preserve zero signals and the live payload.
The removal report must remain NotDeleted(OutcomeUnknown).
Do not widen Adopt admission: A2-1 permits retries only for WorkerUnreachable and WorkerVersion.
The reviewer sent the finding directly to P5 and copied integration. Integration independently found the same path.

### A2-F2 — MEDIUM — OPEN — Reopen publishes the stored protocol without a hello

begin_adoption moves the earlier protocol into row_protocol and clears the visible worker_protocol.
B's unanswered adoption therefore exposes None and persists Lost(WorkerUnreachable) with the earlier protocol P.
After B drops, C's session_of_row copies row.worker_protocol into the visible field again.
Recovery::Post for that Lost row bypasses begin_adoption and reads no hello.
C's get and list therefore expose P again.

LC-9 requires the protocol to be absent when Core never learned the hello.
R-36 preserves the row; it does not authorize a new host to publish the historical protocol as a new observation.
Preserve the storage-only and visible protocol fields during recovery of this Lost outcome.
Prove unanswered adoption, persistence, reopen, and AdoptAll keep get/list at None while the row retains P.
The reviewer asked P5 for a contrary ruling, then sent this finding directly to P5 and copied integration.
Integration independently confirmed the LC-9/R-36 reading and the source path.

### A2-F3 — MEDIUM — OPEN — Required temporary mutation-profile evidence is missing

The supplied full gate invokes cargo xtask ci without NEXTEST_PROFILE=slow.
xtask's mutants_job sets no profile override. The default nextest profile still has terminate-after=1 at the normal two-second bound.
The slow profile has no terminate-after.
Plan section 8 requires an in-diff run with NEXTEST_PROFILE=slow until #181 supplies the permanent timeout classification.
The rule states that a hang must count as a timeout, never a catch, with no grandfathering.

Supply that exact-head artifact, or include it in the replacement head's proof.
Do not change timeout values. The full gate's caught count does not establish the required classification by itself.
Integration first reported this evidence gap. The package reviewer confirmed it and sent the request directly to P5.

### Scope and supplied evidence

The host's failed-proof outcome now follows the lead's reversal of DESIGN 3.6 and the frozen AD-6/A10-1 text.
The in-memory signal mark works within B's session. The findings above concern recovery after B drops.
The controls act at process and storage edges. InstanceKey retains the directory and instance scope.
corrupt_registry_row keeps the first half of Core's encoded row and checks Core's decoder rejects it.
The impostor answers once, encodes real worker-shaped frames, and records signals to its own recorded identity after connection.
withhold_control_link retains an unread connection until Core's deadline decides WorkerUnreachable.
Processes.connections preserves report observation for these connections.

Steward R-42 was read in contracts docs/steward-rulings.md.
The impostor writes its hello and script in one call before Core reads them.
It asserts the entire write succeeded, so a short or failed write is a setup failure.
The outcome test checks rejected frames cannot change the session state or emit a notification.
The complete A11-1 and unauthenticated-cleanup transcripts remain pending for fs_directory.

Only nine IDs leave core-pending.txt. No ID enters pending.
All nine and all 39 active minimum IDs have PASS lines in the supplied exact-head gate.
The approved minimum list gives 35 -> 39 / 70. No real-harness gain is claimed.
The merge imports only the worker-core DESIGN.md from the accepted v1 base.
No pin, dependency, transcript, or existing timeout value changes.

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-adopt-a2-v1-6b5372ac-pool-20261009-160348-83154.log`.
The log names the exact head and base above.
Results: 1169 default passed / 562 skipped; 249 slow passed / 1023 skipped; 113 conformance trials passed, zero failed.
Mutants: 55 tested, 50 caught, 0 missed, 0 timeout, 5 unviable, subject to A2-F3 above.
All ten stages report PASS. Fuzz reports PASS in 0.0 seconds. Job and gate exit zero.

Three findings remain open. The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
No NOT CLEAN report went to the lead. Await replacement READY and the required evidence.

VERDICT: NOT CLEAN
