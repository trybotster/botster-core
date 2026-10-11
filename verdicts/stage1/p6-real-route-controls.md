# Core #287 integration review

## Review in progress — 2026-10-11

Current head: `2645bfc1c216cea54cd61577cae25e834e66133c`.
Source comparison base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Current tree: `a733adc62438eb1b07d7ca795596a261f60f3ab2`.
The fetched PR body names current v1 `926dfb62f3f7168990b2a48741946932d84b2fbb` as its live base.
The body still cites its original gate on 722b415a. It does not yet contain the new evidence results.
This record is not a CLEAN verdict. The completed Linux gate and package closure remain due.

Sol reads all ten cumulative source deltas: Cargo.toml, lib.rs, stream_holders.rs, route_client.rs,
edge_tap.rs, edge_tap/tests.rs, real.rs, real/program.rs, real/route.rs, and real/holders.rs.
Sol reads the affected tap decoder, accept, receive, quiet, hold, and write-interest paths.
Sol reads the request acknowledgement path and the route count, drain, close, fill, and holder paths.
Sol reads the complete lead rulings 23bf/23bg and the relevant controls rows at .36.
The ruling record has SHA-256 ff842c450bdfe972f9d488412676c7b9c6efacee412a156b29414fcb0a5173c7.
No source finding is established in this static review.

Sol reads all three package rounds through `4261a2dab39ae959493afdfe77f76a571f4a0815`.
The package closes its four source findings. Its remaining MEDIUM finding is RC-R1-4, missing Mac proof.
Sol verifies the corrected source: Interrupted ends the current read attempt in sort_unnamed and quiet.
Quiet signals a wake after Interrupted. The tests retain the link and read its bytes at a later call.
The two test reads are nonblocking. The fill helper ends on zero progress or Interrupted.
The source removes the Debug implementations and assertions that the package identified.
The package has not yet published closure of RC-R1-4 for the new Mac record.

The route_accept drain reads at most one 64 KiB chunk per call and retains byte order.
The route_stream_written count adds read bytes to FIONREAD, including allowance drains.
Both close controls retain the count at close. The test checks partial reads and backpressure before both closes.
The tap retains no duplicate of the worker endpoint. Its host_bytes count is an edge-request upper bound; zero is exact.
The body states that limitation. The holder count and the separate fd-handoff proof cover different observations.
Linux counts readable process descriptors. Mac uses bounded lsof commands and the socket address read before hand-over.
The source does not add unsafe code. The fill uses the existing output request path and the applied frame bound.
The hold_start form holds the accepted link after its hello. The body lists the worker-clock condition per transcript.
The hand-over hold returns the endpoint with Blocked. Release restores the last write interest and signals the wake.
The overlap test keeps write interest disabled while the separate Launch hold remains.

Sol reads selected prior-art paths at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
The old backpressure test uses a manually completed write slot.
The old read_available uses unsafe ioctl on a pipe. The old plugin worker lists its own descriptors for closure.
The old conformance module wraps the public engine. Sol does not claim an exhaustive prior-art scan.

### Mac evidence and its limits

Log: ~/botster-sessions/gates/botster-core-stage1-p6-real-route-controls-2645bfc1-pool-20261011-120525-8436.log.
SHA-256: `2d8b8666b146c621d2115cf7092d4898b3cd26a1d50fb71537af14f6bac05395`.
The pool records the exact head on Darwin 27.0.0. The focused unit groups report 63 PASS, zero failures.
Those groups include the Mac holder-count and written-count tests. Pool evidence is outside the local shared-target hazard.
Sol reads the decoded command, every named result, and the complete failed-run detail groups.
The wrapper reports exit 0 after 263 seconds: one second queued, 262 seconds running.
The command retains failed transcript runs and ends with checkout and echo. Wrapper exit 0 does not prove all tests passed.

Real ou_1, ou_11, ou_12, ou_3, ou_5, and a2_1_attach_state_rules each report three PASS results.
Real ou_9 fails run 1 at seed 7, step 21: the screen text is empty. Runs 2 and 3 pass.
Real ou_10 fails runs 1 and 2 at seeds 1 and 4, step 25: output ab precedes input_refused. Run 3 passes.
Real dp_3 times out at step 17 in all three runs. Real a2_1_no_deadline times out at step 23 in all three runs.
Testkit a2_1 cases report unsupported hold_start. The testkit control belongs to separate work.
After the scratch list removals, dp_1 and dp_2 fail the missing named-proof guards on both tiers.
Those guards also make each other cargo command report failure, even when its selected transcript passes.
Sol sent these limits to both P6 agents. No pending removal is accepted from this record.

### Historical Linux gate

The original 722b415a log has SHA-256 `88a781fdc8ff046d69d28090726485e402281c71ed879665c79fe72519554f5c`.
Sol reads its step summaries, timing, tier totals, and mutation totals. Sol does not claim a complete catcher comparison.
All ten steps report PASS in 1756.9 seconds. Default: 1753 PASS; slow: 570 PASS.
Minimum: real-passing 49/68, real-accepted 50/69. There is no FLAKY token.
Stage 1: 165 mutants, 65 caught, 96 missed, four unviable, zero timeouts, 348.6 seconds.
Stage 2: final total 141 caught, including 76 in stage 2; 24 unviable, zero missed, zero timeouts.
The original gate does not cover the later source corrections. The new Linux gate remains running by owner report.
Sol runs no build, test, gate, mutant, reversal, or measurement. Never poll.

## Round 1 — source and required Mac evidence, 2026-10-11

Exact head: `2645bfc1c216cea54cd61577cae25e834e66133c`.
Source base: `f49f74302526a28b6aced1595f74c7d0a1d22836`.
Tree: `a733adc62438eb1b07d7ca795596a261f60f3ab2`.
Sol reads the complete package Round 4 at `b64bc6368dcea89176ec877ad7822ac7a43aae18`.
It closes RC-R1-4 using the 63 passing Mac library tests. No package finding remains.
The source and tree retain the complete static review recorded above. No integration source finding remains.
The focused library filters include the required holder and written-count tests.
The log gives the group summary, not each test name. Sol makes no claim that individual names appear in that log.
The exact-head Mac evidence satisfies the per-OS condition for this source verdict.

The mixed transcript results and masked aggregate command failures remain as recorded above.
No failed run supplies pending-removal evidence. This verdict accepts no pending removal.
The historical Linux gate applies only to 722b415a. It does not prove the later source corrections passed a full gate.
The completed new Linux gate and updated PR body remain due.
The current-v1 import, applicable check and delta review, final gate, and final body remain due under the slot rule.
This verdict does not approve PR completion, any later head, or a later import.
Sol runs no execution job. Never poll.

VERDICT: CLEAN (source and required Mac evidence only) at 2645bfc1c216cea54cd61577cae25e834e66133c
