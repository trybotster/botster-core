# P4a PR1 integration review

## Round 1 — PR #206

Reviewed head: `1484a9ea50d6553d27db1524d8f6393ffb4477fb`.
Base: `9b255cff007552f3952563852104f00a14ddf842`, the fetched v1 head.
Scope: all 33 changed files, including shared crates, descriptor ownership, mutation configuration, and testkit integration.
Package verdict: `ba5c9ebce4a24bd4a099473c7b743e01d25b7e45`, `verdicts/p3-worker.md`, round 134.

The host sends its applied route limits with AttachRoute.
The ordered writer puts each descriptor on its frame's first byte and preserves earlier partial frames.
The testkit delivers each descriptor before its associated bytes.
The worker forms the baseline at the consumed model cut and forwards the retained suffix before later output.
Real descriptor transfer and route input remain outside this PR's approved scope.
The real edge still refuses descriptor transfer, and the packaged worker does not receive route descriptors.
These changes do not establish a working real route.
The mutation exclusion changes only the real-edge function name and retains the existing slow failure proof.

### R1-1 — MEDIUM — Frame limits are not enforced before allocation

`crates/botster-worker-core/src/worker/route.rs:61` encodes and queues control frames without checking their applied frame limit.
The new applied-limits proof sets max_frame_bytes to nine but decodes control frames with unlimited bounds.
The worker therefore emits attached and baseline controls larger than its announced limit.
At a legal cap of one, output_payload forces one payload byte, which produces a two-byte frame.

The same file calls Terminal::snapshot before checking the native and screen bounds.
`crates/botster-terminal-ghostty/src/snapshot.rs:185` queries the native size, then allocates the complete buffer without a caller bound.
The later length check cannot satisfy DP-3's pre-allocation requirement.
Enforce each applicable frame bound before allocation. Use the native size query before reserving the snapshot buffer.
Keep the library encoder. Test actual applied bounds, including small control caps and a cap of one.
This finding agrees with package F78.

### R1-2 — MEDIUM — Baseline insertion exceeds the route queue bound

`worker/route.rs:150-154` inserts the entire baseline before computing the PTY budget.
Route::push has no queue bound check, and baseline also constructs all output frames for the retained suffix.
A valid route_queue_bytes equal to max_snapshot_bytes cannot hold an exactly fitting snapshot plus all these queued controls and framing bytes.
The later zero budget does not remove the excess.
Enforce bounded baseline delivery and account for the retained suffix throughout delivery.
Preserve OU-9's guarantee that a permitted native snapshot fits. Do not reject that snapshot because the implementation eagerly queues surrounding frames.
Test the tight valid queue bound and retained suffix. This finding agrees with package F79.

### R1-3 — MEDIUM — The testkit route client loses an unwritten suffix

`crates/botster-core-testkit/src/route_client.rs:43-49` returns on a write error without retaining its borrowed remainder.
Its proof writes abcdef into capacity four, then accepts g after draining abcd. The bytes ef disappear.
RouteClient::write returns no partial-write result, so the harness silently loses transcript input.
Retain and retry the suffix in order, or fail explicitly while the consumer remains unsupported.
Test complete ordered delivery after partial acceptance. This does not require the deferred input consumer.
This finding agrees with package F80.

### R1-4 — MEDIUM — Interrupted writes close the route

`crates/botster-core-testkit/src/worker.rs:1332-1342` treats Interrupted as a terminal error.
EndControl can inject that error, which becomes RouteWritten Err and closes the route WriteFailed.
The new input contract requires the driver to retry Interrupted.
Retry the same bytes without advancing progress or reporting terminal failure. Test Interrupted followed by successful delivery.
This finding agrees with package F81.

### R1-5 — MEDIUM — A failed close write replaces the first close reason

`worker/route.rs:276-280` always ends a failed write with WriteFailed.
An earlier baseline refusal can already have stored SnapshotTooLarge or BadPeer in closing.
If the route_closed write fails, the host receives a different reason. OU-2 requires the first reason and one report.
Preserve the stored reason while closing the failed transport immediately.
Test a failed write during a queued or partial close frame. This finding agrees with package F82.

### Evidence

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p4a-route-machine-1484a9ea-pool-20261009-170209-10406.log`.
The log names the reviewed head and base. It ran on msa1, allocation ad750d18.
All ten full gate steps pass. Default: 1303 passed. Slow: 254 passed. Conformance: 116 passed, zero failed.
Both mutation runs report 147 tested: 135 caught, 12 unviable, zero missed, and zero timeouts.
The complete gate exits zero after 775 seconds.
The two removed pending IDs have passing testkit evidence and permit that proof tier.
The corrected minimum count is 40/69 to 42/69. The real minimum count does not increase.
The passing gate does not resolve the five source findings.
`git diff --check` passes. The reviewer ran no builds, tests, or gates.

VERDICT: NOT CLEAN (5 open MEDIUM findings) at 1484a9ea50d6553d27db1524d8f6393ffb4477fb
