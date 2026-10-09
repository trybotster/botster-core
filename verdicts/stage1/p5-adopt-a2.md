# P5 adoption controls — PR #204 integration review

## Round 1 — 2026-10-09

Reviewed head: `6b5372ac24d1d9f4b05c365d4e5da264952b0b54`.
Base: `cd97009e93c2641843c05bd46793265b3580b2a9`.
Branch: `stage1/p5-adopt-a2-v1`.
Tier: HIGH, rule 3, because the change crosses the host and testkit packages.
The reviewer read the complete 14-file delta, PR description, affected recovery and removal paths, tests, and supplied gate.
The reviewer applied orchestrate-delivery to the changed failed-handshake premise and recovery lifecycle.
The reviewer changed no product code and ran no builds, tests, mutants, or gates.

### R1-1 / A2-F1 — HIGH — Recovery loses the refusal that prohibits a signal

The new `adopt_hello` failure path sets `worker.gone = true` and records `Lost(WorkerGone)`.
The row retains the recorded worker identity. In the current handle, `Remove` sees the marker and sends no signal.
This implements the lead's stated AD-6/A10-1 correction for that handle only.

The protection does not survive a host restart:

1. Host A starts a session. An impostor replaces the worker's endpoint for the next connection.
2. Host B rejects the impostor's proof and persists `Lost(WorkerGone)`.
3. Host B drops. Host C opens the same directory and runs `AdoptAll`.
4. `session_of_row` creates a session with `worker.gone = false` and restores the recorded identity.
5. `Recovery::Post` publishes the Lost row without a handshake.
6. `Remove` probes that identity. If it still matches, `flow_remove_probed` emits `SignalGroup(Kill)`.

Source: `adopt.rs:193`, `run.rs:739-778`, and `flows.rs:509-545,620-638` at the reviewed head.
This reaches the same unwanted signal that the new marker prevents before restart.
It also contradicts the documented residual case, where the live worker behind the refused endpoint remains untouched.

Preserve the no-signal state when recovering the recorded `WorkerGone` outcome.
Add a public-path proof through refusal, durable row write, drop, reopen, `AdoptAll`, and `Remove`.
Assert no signal, continued payload survival, and `NotDeleted(OutcomeUnknown)`.
Do not broaden `Adopt` admission: A2-1/R-36 permits retries for `WorkerUnreachable` and `WorkerVersion`, not `WorkerGone`.

The package reviewer independently reported the same path as A2-F1.
Status: OPEN. The source trace establishes the failure path; the reviewer did not execute a reproduction.

### R1-2 / A2-F2 — MEDIUM — Recovery exposes a historical protocol without a hello

The new protocol split gives `worker_protocol` the visible value and `row_protocol` the value preserved only for storage.
An unanswered adoption clears the visible value and writes the old protocol back to the Lost row.
This meets the new test for that handle.

After another restart, `session_of_row` copies `row.worker_protocol` directly into the visible `worker_protocol` field.
For a Lost row, `Recovery::Post` bypasses `begin_adoption`, so the visible value is never cleared.
`get` and `list` therefore expose the old protocol again, although this handle read no worker hello.

Source: `adopt.rs:60-63`, `session.rs:292-324`, and `run.rs:751-778`.
Pinned LC-9 defines this value as the protocol learned from the worker's hello.
R-36 preserves the row; it does not authorize treating the storage-only value as a newly observed hello.
The new field comment allowing a row value before adoption conflicts with the required visible/storage distinction.

Preserve that distinction when recovering a Lost row.
Extend the unanswered-adoption proof through drop, reopen, and `AdoptAll`.
Assert that `get` and `list` still show no protocol while the stored row retains its recorded value.
Retain correct reporting when an authenticated hello does supply the protocol, including an unsupported worker version.

The package reviewer raised this recovery case. The reviewer independently traced it and checked LC-9 and R-36.
Status: OPEN.

### R1-3 — HIGH — The required mutation run with the slow profile is missing

The supplied gate runs `cargo xtask ci` without a `NEXTEST_PROFILE=slow` override.
At this head, `mutants_job` does not select a different profile. Its `tier_env(false)` sets seeds only.
The default nextest profile still has a two-second timeout with `terminate-after = 1`.
A terminated test can therefore make a hanging mutant appear caught.

Plan section 8 requires a separate in-diff mutation run with `NEXTEST_PROFILE=slow` until #181's correction lands.
The supplied 50-caught result does not establish the required timeout classification.
Supply the exact-head artifact, or include that run in the replacement head's gate.
Use the existing timeout settings. This finding requires no new timeout value.

The package reviewer independently confirmed the evidence gap.
Status: OPEN, required acceptance evidence missing.

### Checked scope

The lead's changed failed-proof outcome is stated in DESIGN.md and the PR description.
The review checked pinned AD-6 and the A10-1 transcript: a failed proof produces `Lost(WorkerGone)` and prohibits signals.
The finding concerns recovery of that protection, not a request to restore the earlier `WorkerUnreachable` interpretation.

The four controls act at the testkit's process and storage edges.
The impostor sends the failed hello and scripted frames in one write, as steward ruling R-42 at contracts `c2df50c` requires.
A short or failed write is a setup failure. The direct edge test checks all three scripted frames after the hello.
The host-facing test checks that those frames change no state or notification.
The control for signals works after row removal by finding the impostor through its directory and session.
The withheld connection leaves the host's own deadline responsible for the unreachable outcome.
The corruption control cuts bytes written by Core and verifies that Core's decoder rejects them.

The diff removes nine pending IDs and adds none. Every removed ID appears as PASS in the supplied gate.
The pinned replacement map assigns all nine to `core-testkit`, with a process or storage edge where specified.
The minimum list has 70 IDs. The removals increase testkit minimum coverage from 35 to 39.
Those four IDs are the two A10-1 refusal IDs, A10-2 corruption, and AD-6 token/instance checking.
This is no increase in real-process minimum coverage.
A11-1's conformance ID remains pending because its transcript also requires the filesystem control.
The real driver and real-process proofs remain in #176b under the #181 HOLD.

### Supplied evidence

Gate:
`~/botster-sessions/gates/botster-core-stage1-p5-adopt-a2-v1-6b5372ac-pool-20261009-160348-83154.log`.
The log names the exact head and base. The base is an ancestor of the head and matches the fetched v1 tip.
Linux node `msa1` used allocation `57e8128e`. All ten full CI checks passed.
The default tier passed 1169 tests. The slow tier passed 249 tests. Conformance reports 113 passed and zero failed.
Mutation results: 55 tested, 50 caught, five unviable, zero missed, and zero timeout.
The gate exited zero after 219 seconds. R1-3 limits what the mutation results establish.
`git diff --check` passes. No dependency, contract pin, or mutation exclusion changes.

The reviewer sent all three findings directly to P5 and the package reviewer.
No ordinary NOT CLEAN report was sent to the lead.

VERDICT: NOT CLEAN (3 open) at 6b5372ac24d1d9f4b05c365d4e5da264952b0b54


## Round 2 — 2026-10-09 — CLEAN

Reviewed head: `58e3a314c4bbe4082637abe86fa6bbb048748442`.
Base: `cd97009e93c2641843c05bd46793265b3580b2a9`.
Previous reviewed head: `6b5372ac24d1d9f4b05c365d4e5da264952b0b54`.
The reviewer read both correction commits, their three-file delta, the affected recovery path, the updated PR description, and the gate.
HIGH remains correct. The reviewer changed no product code and ran no builds, tests, mutants, or gates.

### Closed findings

**R1-1 / A2-F1 closes.** `session_of_row` restores `worker.gone` from a recorded `Lost(WorkerGone)` state.
The later handle's `Remove` now takes the no-signal path and reports `NotDeleted(OutcomeUnknown)`.
The host proof checks refusal, recovery into a third host, removal, zero signals, and survival of the recorded process.
The testkit proof covers both wrong-token and wrong-instance refusals through drop, reopen, `AdoptAll`, and `Remove`.
It checks the payload before removal, then the unknown cleanup result and zero signals after removal.
The signal observer remains available after the row is deleted.
This preserves the proof's no-signal invariant without using a row-dependent control after row deletion.
`Adopt` admission remains unchanged. No `WorkerGone` retry is added.

**R1-2 / A2-F2 closes.** A recovered, posted `Lost(WorkerUnreachable)` row moves its historical protocol into `row_protocol`.
The visible field stays empty. The authenticated-hello and unsupported-version paths are unchanged.
The host proof now opens a third host and checks both `get` and `list` for no visible protocol.
It also checks that the row retains its recorded protocol.

**R1-3 / A2-F3 closes.** One pool job runs the full gate and then the required in-diff mutation job with `NEXTEST_PROFILE=slow`.
The supplied log identifies this exact head and records successful outcomes for both mutation runs.
The new regression proofs all have PASS lines in that log.

### Evidence and scope

Gate:
`~/botster-sessions/gates/botster-core-stage1-p5-adopt-a2-v1-58e3a314-pool-20261009-161258-9135.log`.
Linux node `msa1` used allocation `d5c16b29`.
All ten full CI checks passed. The default tier passed 1170 tests. The slow tier passed 249 tests.
Conformance reports 113 passed and zero failed.
Each mutation run tested 57 mutants: 51 caught, six unviable, zero missed, and zero timeouts.
The second run explicitly uses the slow profile. The gate exited zero after 389 seconds.
The earlier `c0676f7c` run failed because its test read the row-dependent payload control after removal.
The final test checks payload liveness before removal and retains the post-removal signal assertion.
That earlier setup failure supplies no negative proof of the recovery defect.

The base remains the fetched v1 tip and is an ancestor of the reviewed head. `git diff --check` passes.
No pending-list, contract pin, dependency, configuration, or exclusion changes occur in this correction.
The round 1 mapping of nine removed IDs remains applicable: testkit minimum coverage increases from 35 to 39 of 70.
This review claims no real-process minimum gain. The #176b real-driver and real-process scope remains separate.
All three integration findings are closed at this head.

VERDICT: CLEAN (0 open) at 58e3a314c4bbe4082637abe86fa6bbb048748442
