# P5 in-process adoption review

## PR #201 — Round 1 — 2026-10-09

Exact head: `d96c9128a9c0f8bafd07f6faf44ba8600e0d6273`.
Exact tree: `f9137a6d6bb1710f6082f3cc13cc8bee82eacbfe`.
PR and gate base: `a6555ebaf221042ca7b777ca2f2425e4a63dd960`.
Parent: `b00a56b99ab96599925713983f3d5ba26b640f5f`.
Branch: `stage1/p5-adopt-a1`.

The reviewer checked the risk tier first. The PR states HIGH under rule 3 because it changes shared crates.
The PR includes Prior art and an exact-head gate log.
The lead authorized #176a-1: worker machine, host, and in-process testkit adoption.
The real driver and real-process proofs stay in #176b after #181. This review does not apply the old F29 hold to #201.
The controls stay in #176a-2. AD-3 and the TM-4 change wait for A18's final tag and pin.

### A1-F1 — MEDIUM — OPEN — Adoption leaves the control wake target at the old host

RunProcesses records the spawning Processes table. Workers::break_link signals that table's wake.
Workers::program_edge also returns that wake for PTY controls.
WorkerSpawner::connect_worker passes only LinkEnd. WorkerEdges::AdoptLink updates cell.link but retains the old wake target.

Start a worker on A. Drop A. Adopt the worker on B. Settle B and disable spurious wakes.
A subsequent break_control or pty_output signals A's wake. B's wait can time out while the worker has ready work.
Keep process-exit ownership at the spawning host. Track the current control host separately.
Transfer its wake target only when adoption succeeds. Prove both controls wake B after adoption.

### A1-F2 — MEDIUM — OPEN — Quiet can ignore an adopted worker's unread report

ProcessTable::holds_reports checks only cells inserted by this host's WorkerSpawner::spawn.
Neither connect_worker nor AdoptLink adds the adopted control link to the adopting host's table.
After B adopts A's worker, Workers::run can flush a report to B's link and leave the worker idle.
B's table contains no cell for that worker. Workers::edges_quiet then returns true before B consumes the report.

Track current control links separately from child-process exits. Transfer link observation at the successful adoption fence.
Prove an unread report alone keeps B's edges non-quiet after adoption. Then prove consumption permits quiet.
Cover repeated adoption, rejected candidates, and retirement of a link or host in the same ownership model.

### Integration findings — confirmed and OPEN

The package reviewer confirmed these findings in the source. Integration owns their finding numbers.

- **R1-1 HIGH:** Worker::adopt resets queued_total and written_total but keeps output_sent_to and output_unsent.
  If old written output exceeds the new handshake, subsequent plain output can remain unsent forever.
  Reset output-report state at the fence. Prove new output reaches the adopting host after a large old-link count.
- **R1-2 MEDIUM:** adoption_report sends formats: Vec::new(). HostEngine::adopt_report assigns this to s.formats.
  Adoption therefore removes supported snapshot formats. Preserve formats and prove them for running and exited adoption.
- **R1-3 MEDIUM:** on_candidate checks teardown state only at admission. on_candidate_bytes does not check that state again.
  A previously admitted candidate can adopt after Remove or Terminate starts.
  With RemoveResult queued and exit_pending set, adoption changes Closing to Ready and prevents finish_close from completing teardown.
  Close pending candidates at teardown or reject them before adoption. Prove both removal sequences emit no AdoptLink and finish teardown.

The reviewer sent A1-F1 and A1-F2 directly to P5 and copied integration.
Integration independently confirmed both findings. The reviewer also sent confirmation of the three integration findings to P5.

### Scope and supplied evidence

The reviewer read the complete 18-file source delta and the new tests.
InstanceKey retains directory scope for held starts and endpoints. The input fence retains v1's Option<u64> request numbers.
The active retired write completes without a report. A new host's cancel cannot stop it.
The host removes the endpoint after worker teardown and before row deletion.
The model retains the last title and cwd. AD-3 remains pending.
The real driver's two added action arms are inactive until it supplies candidates; the real endpoint implementation stays outside this PR.

Only 12 IDs leave core-pending.txt. No ID enters pending.
All 12 have PASS lines in the supplied exact-head gate. All 33 active minimum IDs also have PASS lines.
The approved minimum list gives 30 -> 33 / 70.
No pin, dependency, transcript, or existing timeout value changes.

Supplied Linux gate: `~/botster-sessions/gates/botster-core-stage1-p5-adopt-a1-d96c9128-pool-20261009-150552-98967.log`.
It names the exact head and base above.
Results: 1134 default passed / 575 skipped; 249 slow passed / 1007 skipped.
Mutants: 101 tested, 89 caught, 0 missed, 0 timeout, 12 unviable.
All ten stages report PASS. The fuzz stage reports PASS in 0.0 seconds. Job and gate exit zero.
These results do not close the five source findings.

The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
No NOT CLEAN report went to the lead. Await P5's next READY for another exact-head review.

VERDICT: NOT CLEAN

## PR #201 — Round 2 — 2026-10-09

Exact head: `74b1e2116d19ec96e511ad3042cde8a04f35c195`.
Exact tree: `ac15298c04a8d789a623aff20085f9c76f120885`.
PR and gate base: `a6555ebaf221042ca7b777ca2f2425e4a63dd960`.
Parent: `d96c9128a9c0f8bafd07f6faf44ba8600e0d6273`.

The reviewer checked the risk tier first. HIGH rule 3 remains correct.
The PR retains Prior art and maps each finding to its fix and proof.
The reviewer read the complete five-file delta. The authorized split and the Round 1 scope limits remain unchanged.

### Findings closed in this source delta

- **A1-F1 CLOSED:** ProcessCell.control identifies the current control host through a Weak reference.
  Spawn sets it. Successful adoption transfers it. Rejected candidates do not transfer it.
  break_link, program_edge, and release_start use control_wake. The cell lock ends before a host table lock starts.
  The new behavior test proves PTY work wakes B after adoption and a later link break wakes C rather than B.
  The spawning host retains its process-exit ownership.
- **Integration R1-1 CLOSED in package review:** the fence resets output_sent_to and output_unsent.
  The new test sends 64 old-link output reports and proves later output emits a report on the shorter new link.
- **Integration R1-2 CLOSED in package review:** a payload that ran carries model::snapshot_formats() in Adopted.
  Running and exited adoption tests assert the supported format.
- **Integration R1-3 CLOSED in package review:** Remove and Terminate close the waiting candidate.
  The new removal test delivers its hello before and after the queued RemoveResult.
  Neither hello adopts. Writing the result produces LinkClose and Exit on the original link.
  A separate test proves Terminate closes the waiting candidate.

Integration owns its independent closure decisions for R1-1, R1-2, and R1-3.

### A1-F2 — MEDIUM — OPEN — Quiet ignores EOF from a rejected adoption connection

The accepted-link case is fixed. Processes.links now observes adopted worker reports.
The rejected-link case still loses a host report.
WorkerSpawner::connect_worker queues the connection without recording its link in Processes.links.
Only WorkerEdges::AdoptLink adds a link to that map.

The failure sequence is:

1. A stranger occupies the worker's candidate place.
2. B connects to that worker for adoption.
3. Run the workers without pumping B.
4. The worker rejects B's candidate and closes its end.
5. All worker work settles. B's link still holds unread EOF.
6. B's Processes.links is empty, so edges_quiet(B) returns true before B consumes EOF.

Track host link reports from connection creation through closure and consumption.
Keep that observation separate from the current control wake target and the child-process exit owner.
A rejected candidate must keep the control wake target unchanged. Its EOF still belongs to B.
Prove this EOF alone prevents quiet until B consumes it.
The current new test pumps B through the refusal before checking ownership. It does not test the unread EOF.
The reviewer sent this remaining finding directly to P5 and copied integration.

### Supplied evidence

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-adopt-a1-74b1e211-pool-20261009-151953-30712.log`.
The log names the exact head and base above. All four new tests have PASS lines.
Results: 1138 default passed / 575 skipped; 249 slow passed / 1008 skipped.
Mutants: 115 tested, 100 caught, 0 missed, 0 timeout, 15 unviable.
All ten stages report PASS. Fuzz reports PASS in 0.0 seconds. Job and gate exit zero.
The pending list equals Round 1. All 12 removed IDs and all 33 active minimum IDs have PASS lines.
Minimum count remains 30 -> 33 / 70. No pin, dependency, transcript, or existing timeout value changes.

One package finding remains open. The reviewer changed no product code and ran no tests or gates.
No NOT CLEAN report went to the lead. Await replacement READY.

VERDICT: NOT CLEAN

## PR #201 — Round 3 — 2026-10-09

Exact head: `2e3414446503c0c2c14a9d2fc691028523464030`.
Exact tree: `093a89b117e31dfc075b419e25d5b340ef7e9899`.
PR and gate base: `3fa51cd2d148315883002af96495b3096242ba42`.
Parents: fix head `660d6e8a5e515531563b4af2a777688a6aa1741b` and the base above.

The reviewer checked the risk tier first. HIGH rule 3 remains correct.
The PR retains Prior art and updates the exact-head proof and finding table.

**A1-F2 CLOSED:** Processes.connections records the worker's EndControl when the host connects, before candidate admission.
ProcessTable::holds_reports checks those connections as well as exits and accepted control links.
Candidate rejection therefore retains EOF observation until the host consumes EOF and closes its end.
The new test runs workers without a host pump while a stranger occupies the candidate place.
It proves rejected EOF alone prevents quiet, then proves host consumption and closure permit quiet.
The connection record does not change the control wake target or the process-exit owner.
The four findings closed in Round 2 remain closed.

The reviewer read the complete fix delta and the merge resolutions.
ProcessCell retains control, model_log, and worker. WorkerEdges retains its endpoint fields and the SharedWorker binding.
Both WorkerEdges literals retain the merged fields. send_hello and model_rev() both remain in Worker.
The merge retains the model_snapshot() getter and both sets of worker tests.
Static comparisons show the incoming changed lines match for worker.rs and its tests in both crates.
The imported controls, harness, lib, and resume-control files match the v1 base blobs exactly.
The model delta adds only the incoming SnapshotError import and model_snapshot() getter; the adoption title and cwd changes remain.
The pending set equals the intersection of the two parents' sets.
These merge checks preserve accepted v1 work. They do not replace #200's separate review.

Only the original 12 adoption IDs leave pending relative to the new base. No ID enters pending.
All 12 removed IDs and all 34 active minimum IDs have PASS lines in the supplied exact-head gate.
The approved minimum list gives 31 -> 34 / 70.
The PR adds no pin, dependency, transcript, or existing timeout value change.

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-adopt-a1-2e341444-pool-20261009-153141-83310.log`.
The log names the exact head and new base above. The new rejected-EOF proof and the earlier regression proofs PASS.
Results: 1151 default passed / 572 skipped; 249 slow passed / 1016 skipped.
Mutants: 116 tested, 101 caught, 0 missed, 0 timeout, 15 unviable.
All ten stages report PASS. Fuzz reports PASS in 0.0 seconds. Job and gate exit zero.

No package finding remains. Integration must supply its separate CLEAN on this exact head.
This verdict covers the authorized #176a-1 split. It does not certify #176a-2, #176b, or the pending A18 behavior.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: CLEAN
