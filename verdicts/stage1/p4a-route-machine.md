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

## Round 2 — Corrections, A17 merge, and R-44

Reviewed head: `959a716cf9b918c1a7fa8845a7ca8ed15c3f87b4`.
Current base: `1627732f5f651f5946e5d60b64dedcbf6c4cb683`.
Scope: the complete correction delta from round 1, including the intermediate base merges and the final A17 resolution.
The READY named b0d50044 as the previous head. This reviewer's previous verdict actually covered 1484a9ea.
The reviewer therefore checked the five corrections as well as the final merge.

### Closed findings

R1-1 is closed for this route implementation.
Terminal::snapshot_at_most queries the native size before it reserves the snapshot buffer.
The route uses the smaller native and screen-payload limits. CaptureSnapshot uses the same bounded method.
The worker checks encoded control frames before it queues them and no longer forces a payload byte at a cap of one.
The test client decodes with the route's applied bounds.
Tests cover an exact snapshot bound, one byte below it, an exact control-frame bound, and failed attachment below that bound.

The reviewer read steward R-44 at contracts commit dede41d.
A frame bound that cannot carry attachment and a missing common terminal format both produce HandoffFailed without a route frame.
The worker closes the transport and reports the failure once.
SnapshotTooLarge retains its separate close-frame path when that frame fits.
The pending A19 host floor change remains later work; this PR does not claim that refusal is implemented.

R1-3 is closed. TestkitRoute retains an unwritten suffix and sends it before later bytes.
Both read and write retry retained bytes. A terminal stream failure clears them and prevents later writes.
The capacity-four proof now delivers abcdefg in order instead of losing ef.

R1-4 is closed. The binding retries Interrupted with the same bytes.
WouldBlock remains a separate readiness wait. The test injects Interrupted and observes successful delivery.

R1-5 is closed. A failed close write uses the stored first reason when one exists.
The regression starts a SnapshotTooLarge close, fails its write, and observes that same reason once.

### R1-2 — MEDIUM — Queue accounting awaits a contract ruling

The implementation still inserts the whole baseline and retained suffix into the empty route queue.
The new documentation permits occupancy above route_queue_bytes for the other baseline frames, prefixes, and retained suffix.
The PTY budget is zero while occupancy exceeds that threshold.
The new test deliberately exercises this overage at route_queue_bytes equal to max_snapshot_bytes.
The lead confirmed that no queue ruling authorized this interpretation.

The reviewer sent the exact accounting question to the lead for the steward.
OU-9 guarantees that a snapshot within max_snapshot_bytes fits when route_queue_bytes is at least that limit.
The 9B row explicitly describes a route above the threshold as not progressing.
A3-1 charges encoded frames. An exactly maximal snapshot needs additional type and stream-prefix bytes.
These clauses leave the permitted overage and its accounting unclear.
The original hard-cap remedy may overstate 9B; the reviewer does not require that remedy without the ruling.
This finding remains open for contract clarification. The green gate does not decide that interpretation.

### R2-1 — LOW — The PR body retains two obsolete statements

The body says contracts-v0.1.22 does not substitute attach_route bindings.
The pinned driver does substitute them at driver.rs:1228; the pin review verified that change.
Recheck the terminal-format ID and state its actual current status instead of the obsolete runner gap.

The minimum counts also use the earlier base.
The current base has 41 passing minimum IDs. This head has 43, all with PASS evidence.
Correct 40 to 42 into 41 to 43, retaining any older counts only as labeled history.
P3 acknowledged both corrections and is checking the terminal-format ID.

### R2-2 — MEDIUM — The renamed shell needs identified slow mutation evidence

The RealEdges::link_send_descriptor exclusion retains the earlier handoff_route slow-evidence claim.
The PR body names no manual slow-feature mutation log for this changed shell and proof.
The extra NEXTEST_PROFILE invocation in the supplied gate does not enable the slow feature or override the explicit mutants profile.
Approved plan 23n requires explicit manual slow-feature evidence for exclusions that rely on that evidence.

Identify the supporting manual log and its source revision.
If earlier evidence still applies, show why it covers the changed signature and proof.
Otherwise supply a focused manual run with the required slow feature, profile, and first-failure setting.
This request does not require an unrelated broad run.

### Merge and gate evidence

The first three base merges equal their automatic merge trees:

- 2b366adf: `313a76c60711c3c144cbd4e3906ffb300a9a2dbb`.
- 73be3b0f: `5bbe173d5e9867e6cef9f613a82c7261b76bddb4`.
- 413ae354: `6194fda6ceaafaf72431471c259fbae339a1ee79`.

The reviewer read every final resolution against the conflicted automatic tree for 959a716c.
Cargo.toml retains the new Hub conformance dependency at v0.1.22.
The real-edge proof retains the descriptor-send API instead of restoring the removed handoff API.
The five A17 field-use lines are removed. The remaining resolved changes implement and document R-44.
The final tree is `a1e76d944554c65ef0202c9dc43e1d11dd5fbc3b`.

Gate: `/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p4a-route-machine-959a716c-pool-20261009-182804-79349.log`.
The Linux pool log names the exact head and current base. It ran on msa1, allocation a4e9ec8b.
All ten stages pass. The gate exits zero after 875 seconds.
Default: 1327 passed. Slow: 254 passed. Conformance: 123 passed, zero failed.
Both mutation invocations report 170 mutants: 157 caught, 13 unviable, zero missed, and zero timeouts.
The new correction tests have PASS evidence.
The 123 passing IDs exactly cover the current expected active set, including all 43 active minimum IDs.
The two pending removals remain ou_9_baseline_then_live_no_gap and dp_3_screen_is_one_frame_within_max_screen_frame_bytes.
Real descriptor transfer remains later work. No real-route acceptance is claimed.

`git diff --check` passes. The reviewer ran no builds, tests, or gates.

VERDICT: NOT CLEAN at 959a716cf9b918c1a7fa8845a7ca8ed15c3f87b4 — R1-2 MEDIUM, R2-1 LOW, R2-2 MEDIUM remain open.
