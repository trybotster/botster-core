# P5 start controls review

## PR #196 — Round 1 — 2026-10-09

Exact head: `2745781b7c9bd74b1c6f65eb16f3e97726e39d0b`.
Exact tree: `8c7dcb292fde7cc59ffc83e05a04e67e82ead93d`.
PR and gate base: `1dd1657a2c53f4da6953f7a349d7fa9d59b97ec6`.
Parent: `ec12eac732458afc511d18cd84f821458b450ea3`.
Branch: `stage1/p5-controls-start`.

The reviewer checked the risk tier first. The PR states HIGH under rule 3 because it changes the shared testkit.
The PR includes Prior art and names the supplied exact-head gate.
The PR records P6's design approval and four test additions, plus P3's agreement on the shared ProcessCell fields.

### SC-F1 — MEDIUM — OPEN — A start hold affects other data directories

Workers::held_starts is a set shared by every worker and handle in the run.
Its only key is InstanceId.
HostEngine::mint_instance constructs that ID from the host epoch and a counter that starts at one.
Directories::open keeps the epoch in each directory's registry and starts a new directory at epoch one.
The first session in each of two new directories therefore has instance 1-1.

The failure sequence is:

1. Open handles a and b on two different new data directories.
2. Create the first session in each directory.
3. Set hold_start_at before payload for the session under a.
4. Start the session under b without setting a hold for b.

WorkerEdges::start_held finds a's hold using b's equal InstanceId and parks b's spawn too.
A separate hold for b is also refused as already held.
release_start_at under b removes a's hold because it removes the same key.
WorkerEdges::ended under b can also remove a's hold during cleanup.
The session control therefore changes another directory's start.

P5 must scope the hold key to the owning directory or host as well as the instance.
P5 must add a two-directory behavior test with equal instance IDs.
The test must show that an unheld session starts while the other session remains held.
The test must also show that release and worker cleanup preserve another directory's hold.
The current tests use one directory and do not cover this scope boundary.
The reviewer sent SC-F1 directly to P5 and copied integration.

### Source review and supplied evidence

The reviewer read the complete eight-file delta, all six new tests, and the five activated transcripts.
The reviewer also checked host instance creation, directory epochs, control definitions, and the relevant worker code.
No other package finding arose.

registry_row uses the host's decoder and reads the stored state, worker identity presence, and labels without writing.
The payload hold keeps Action::SpawnPayload's specification at the worker edge and leaves the machine unchanged.
Release makes that kept spawn ready and signals the worker owner's host wake.
The payload observer reads ProcessCell and accounts for worker termination.
The field becomes false when the edge queues a payload exit, reaps it, or ends the worker.
The tests check the durable Starting row, absent payload, incomplete Start, release, stopped hold, and isolation between runs.
The edge tests check released readiness and payload death before the worker consumes its exit.
The argument structs deny unknown fields.
The identity and running hold phases return typed Unsupported.
The PR records the unresolved Running-row specification point; this review does not approve that deferred phase.
The PR changes no production machine, contracts pin, dependency, timeout value, transcript, or expected value.
It adds no real-process test.

Exactly five ids leave pending: ad_7_payload_launches_after_durable_identity, ev_5_poll_wakes_pending_work,
ev_5_stop_kills_while_queue_full, tm_6_more_means_runnable_not_blocked, and tm_6_poll_room_unparks_and_wakes.
No id enters pending.
The replacement map gives AD-7 core-testkit+edge/storage, the stop-while-full case core-testkit, and the other three core-testkit+perturb.
None is a real-only slow proof.
Only AD-7 belongs to the minimum 70; the exact base and head have 26 and 27 minimum ids outside pending.
The reviewer matched all five removals and all 27 active minimum ids to exact-head PASS lines.
The gate reports 31 active conformance trials PASS under the unchanged default seed configuration, 0-31.

The supplied full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/controls-start-2745781b.log`.
It names the exact reviewed head and base.
The default tier reports 986 passed and 644 skipped.
The slow tier reports 243 passed and 981 skipped.
All six new tests pass, but none uses two directories with equal instance IDs.
The mutation run tests 28 mutants: 24 caught, zero missed, zero timeouts, and four unviable.
All ten CI stages pass. Fuzz runs no decoder harness for this delta.
The job and gate exit zero.

SC-F1 remains open. Integration owns its separate review of this HIGH PR.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

VERDICT: NOT CLEAN
