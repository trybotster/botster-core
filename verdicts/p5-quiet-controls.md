# P5 quiet controls review

## PR #193 — Round 1 — 2026-10-09

Exact head: `7d75b70f9fc4d8fc0355d07300a00a7d4706ef29`.
Exact tree: `0c1d17341acdc95971f7ebd1ecc7f962369febd5`.
PR and gate base: `dc7758dc97fea0a4193a8429902d855219d4d3fc`.
Parents: `b6437670b40cbd9de15d30ae065526672915bbc4` and `dc7758dc97fea0a4193a8429902d855219d4d3fc`.
Branch: `stage1/p5-controls-quiet`.

The reviewer checked the risk tier first. The PR states HIGH under rule 3 because it changes the shared testkit.
The PR includes Prior art and names the supplied exact-head gate.
The PR records P6's design agreement and the conditions for the deferred no_spurious_wakes control.

### Source review

The reviewer read the complete nine-file delta and the relevant network, worker, program, simulator, and host driver code.
The reviewer also read the contracts control definition, await_quiet driver, and EV-5 transcript.
No package finding arose.

The harness now keeps each open handle's directory and process table together in HandleEdges.
Successful open stores both. drop_handle removes both and preserves the rows and workers.
The previous PC-F1 fix remains effective through the new map.
The process table keeps the worker's EndControl at spawn, before the host accepts the link.

edges_quiet rejects arguments through deny_unknown_fields and rejects handles absent from the harness map.
It checks worker readiness, the selected host's unconsumed process exits, and the worker links' reports toward that host.
EndControl::holds_for_peer checks bytes, descriptors, and peer-visible close while the receiving end remains open.
The host driver closes its end when it consumes EOF, so the close stops counting at that point.
The observer reads these queues without removing their contents.
The control calls neither Core::pump nor Core::poll_events and moves no clock.
The existing readiness query draws no scheduling choice and delivers no machine input.

The new worker test isolates ready worker work, unread link bytes, and a queued process exit.
Each condition makes quiet false without either other condition.
The network test isolates bytes, a descriptor, and close, and checks the closed receiver case.
The harness test reaches Running, reaches quiet, and then confirms that Core's queued events remain available.
The refusal test checks an unknown handle and an unknown argument.
The existing drop-and-reopen test from #191 still checks the shared handle lookup.

no_spurious_wakes returns typed ControlError::Unsupported.
This PR does not claim a measured wake proof or activate its minimum id.
The testkit route transport and service lanes remain unavailable.
Their implementations must extend the quiet check when those edges can hold reports toward Core.
This verdict covers the current edges and does not certify future adoption or route implementations.
The PR changes no production machine, contracts pin, dependency manifest, or real-process test.

### Pending-list and supplied evidence

The only pending-list removal is conf::ev_5_no_mandatory_event_lost_or_replaced. No id enters the pending list.
The replacement map classifies this id as core-testkit, not a real-only slow proof.
The id belongs to the 70 minimum ids.
The exact base and head have 24 and 25 minimum ids outside pending, respectively.
The supplied exact-head gate reports this transcript PASS.
The transcript retains the mandatory events across await_quiet and checks them through pump_collect.
No transcript or expected value changes.

The supplied full Linux gate is `~/botster-sessions/shared/core-stage1/gate-logs/controls-quiet-7d75b70f.log`.
It names the exact reviewed head and base.
The default tier reports 971 passed and 650 skipped.
The slow tier reports 243 passed and 972 skipped.
The four new tests described above and the EV-5 conformance trial pass.
The mutation run tests 32 mutants: 29 caught, zero missed, zero timeouts, and three unviable.
All ten CI stages pass. Fuzz runs no decoder harness for this delta.
The job and gate exit zero.

No package finding remains. Integration must supply its separate CLEAN before this HIGH PR merges.
This verdict does not close #176's proof hold or establish real-process minimum conformance.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.
The lead owns the merge decision.

VERDICT: CLEAN
