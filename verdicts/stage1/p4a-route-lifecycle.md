# PR #217: P4a route lifecycle

## Round 1 — 2026-10-09

- PR: https://github.com/trybotster/botster-core/pull/217
- Head: `8ae2643d8a2ddb1fffceda129c3775b3abfa0f88`.
- Base: `e99012939c8df4f85da5ae5af7144534b129cb70` (`v1`).
- Scope: the full 20-file change, including the host, worker machine, shared testkit, controls, tests, and 23 pending removals.
- Tier: HIGH, rule 3. The change crosses package boundaries and changes the shared testkit.
- Contracts: `contracts-v0.1.24`, `d79aed5e84d4a8bc9b76f6163ca6bc4a9d89b951`.
- Plan: revision 23q, `bae71c81e0d9f1bd6fc291910f496be495414076`.
- Method: source review and inspection of the supplied gate log. I ran no builds, tests, or gates.

### R1-1 — HIGH: a read failure replaces the first close reason

`crates/botster-worker-core/src/worker/route.rs:526` always passes `PeerClosed` from `on_route_ended` to `end_route`.
A prior detach or bad frame can already have set `route.closing` while its final frame waits for transport capacity.
An EOF or read error then replaces that reason with `PeerClosed` in the worker report.
Core OU-2 requires the first reason. The write-error path already preserves `route.closing`.

Preserve the first reason on the read-end path too.
Add a regression that holds a close frame, delivers `RouteEnded`, and checks one transport close and one report with the original reason.
Package finding F86 covers the same defect.

Status: OPEN.

### R1-2 — HIGH: route input has no capacity control

Three new paths retain unbounded data:

- `route.rs:491` appends bytes before checking `route.closing`. A closing route never consumes these bytes.
- `input.rs:140` appends each `Pending::Route` without charging or checking the route's input allowance.
- `route.rs:581` queues each refusal without checking the route's outgoing allowance.

The testkit binding reads while `!route.ended`; it has no machine-owned read allowance.
A client can keep writing while the PTY is blocked, while the client refuses to read, or while a close frame waits.
Core DP-5 requires transport backpressure at `route_input_queue_bytes` and `route_queue_bytes`.
The existing PTY output allowance does not bound these new input paths.

Apply the existing contract limits to route input and refusal retention.
Stop and resume route reads through the binding when capacity changes.
Reject closing-route bytes before retention.
Prove bounded retention, resumed progress, and no lost or duplicated input under saturation.
Package finding F88 covers these paths.

Status: OPEN.

### R1-3 — HIGH: the host reports route completion before transport completion

`crates/botster-core-host/src/flows.rs:393` closes every ended session route in `StopPhase::Finish`.
However, `Worker::report_exit` establishes only that the PTY tail is queued.
The worker then queues `session_ended` behind that tail.
`end_route` suppresses the eventual `SessionEnded` report.

With a held route, the host can post `RouteClosed` while the worker still owns queued output and an open transport.
`HostEngine::close_route` also completes a waiting `Detach`, before the transport closes.
The synthetic `SessionEnded` reason can replace a prior `Revoked` or `Replaced` reason that the worker is still delivering.
Core OU-7 requires each close after delivery of its queue or its stall.
Core DP-7 requires detach completion after transport closure. OU-2 preserves the first reason.

Preserve the host's authority over the exit cause, but use worker completion evidence before reporting an ended route closed.
Handle a lost session separately.
Add a host/worker regression with a held tail and a pending detach.
Check that neither `RouteClosed` nor `Completed{Detach}` arrives early.
Release the route and check exactly one close with the correct reason.
Package finding F89 independently confirms this defect.

Status: OPEN.

### R1-4 — MEDIUM: some complete input frames do not advance the client revision

`route.rs:505` sends decoded `Unsupported` and `Invalid` frames directly to `refuse`.
Only `on_client_frame` calls `client_received` and reports `Observation::ClientInput`.
Core IN-4 requires the worker to tag and count every complete client input frame on receipt.
The missing revision also affects the host's guarded-write decisions.

Account for complete unsupported and invalid input frames at receipt.
Keep unparseable frames distinct.
Add revision and activity proofs for the decoded refusal paths.
Package finding F87 covers the same defect.

Status: OPEN.

### R1-5 — HIGH: route input loses its owner before completion

`Pending::Route(Vec<u8>)` retains neither the route nor the client op.
`input.rs:191` converts it to `Active { req: None, ... }`, just like a model reply.
`finish_active` reports only transactions with a host request.
Consequently, a failed or partial client bytes/text write sends no `input_refused`.
`try_start` also silently drops queued route input when the payload has ended.
Core DP-5 requires the route exception, including the known `written_bytes` after a partial write.

The same ownership loss crosses host adoption: `input_fence` clears the whole queue, including route-owned input.
Core DP-8 requires routes and their byte flow to survive the host restart.
Retiring old-host requests must not discard queued client input.

Retain route and op identity through admission, execution, and completion.
Send failure exceptions on that route with the correct known byte count.
Keep successful route writes silent.
Prove partial failure, payload end before admission, and retained route input across host fencing.

Status: OPEN.

### Evidence checked

- The remote PR head and `v1` matched the revisions above. The base is an ancestor of the head.
- `git diff --check` passed.
- The pending delta removes 23 ids and adds none. Each removed id has a transcript at the pinned contracts revision.
- The canonical plan list has 69 minimum ids. This delta removes eight from pending: 46/69 becomes 54/69 testkit-passing.
- The testkit uses the shared worker machine and codec. The handoff controls act at `link_send_descriptor` and preserve endpoint ownership on `Blocked`.
- Releasing a held handoff clears the hold and signals the host wake. The driver retains the ordered frame and endpoint while blocked.
- The PR distinguishes testkit controls from proposed real-tier controls. Portable `fail_writes` remains unresolved.
- The PR records the required later `core-real-pending.txt` entries, including `ou_3_progressing_reader_lossless` for the R-47 transcript correction.
- These testkit passes establish no new real-worker acceptance.

Gate log:
`/Users/jasonconigliari/botster-sessions/gates/botster-core-stage1-p4a-route-lifecycle-8ae2643d-pool-20261009-205549-59478.log`

The log names the exact head and base. All ten CI steps passed in 365.6 seconds.
The default suite ran 1,437 tests. The slow suite ran 256 tests.
Conformance reported 205 passed, 395 pending with transcripts, 70 pending without transcripts, two deferred, and 18 withdrawn.
The default conformance environment uses seeds 0–31.
Both mutation runs reported 128 mutants: 113 caught, 15 unviable, zero missed, and zero timeouts.
The second command sets `NEXTEST_PROFILE=slow`; it does not enable the slow feature.
The pool job exited zero after 635 seconds on `msa1`.

The green gate does not cover the five source findings above.
The package reviewer is completing the artifact for this exact head.

### Replacement scope reported by P3

P3 reports that the lead selected option A: remove route input from PR2 and deliver it as one complete unit in PR3.
That removal includes decoding, admission, refusals, the read binding, and input-dependent pending removals.
PR3 must record R1-1, R1-2, R1-4, and R1-5 as requirements to close.
PR2 must correct R1-3 and prove the held-tail completion order.
This report does not close any finding on the reviewed head.
A replacement READY and its exact-head gate are required for the next round.

VERDICT: NOT CLEAN (5 open).
