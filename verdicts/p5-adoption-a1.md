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
