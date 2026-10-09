# P4a stream-route design review — PR #203

## Round 1 — 2026-10-09

Reviewed head: `96fa437426b05c2743f0bb2c30731e30d0323f55`.
Base: `cc2e86ee356c7adcfc9ab95dd50494ae98429a45` (`v1`, including #202).
The design text is unchanged from the assigned first head, `fc1d85d40d1b437d0ea5a22ec2620bd63cf44112`.
The base merge imports the reviewed #202 change. The PR diff contains only `crates/botster-worker-core/DESIGN.md`.
The reviewer read all 80 added lines, the affected producer and consumer paths, and the pinned route clauses.
The reviewer applied orchestrate-delivery to the proposed handoff, bounded queues, baseline boundary, and closure paths.
The reviewer changed no product code and ran no builds, tests, or gates.
These findings concern the design. They do not claim an executed failure of a P4a implementation.

### D1 / F73 — MEDIUM — Send order does not establish descriptor delivery order

Location: `DESIGN.md:85-104` and the two review questions.

The design says that sending the descriptor first guarantees its presence when `AttachRoute` is decoded.
The current testkit does not supply that guarantee by itself.
`LinkEnd` stores descriptors and bytes in separate queues, with separate receive methods.
`Sim::step` chooses among ready inputs. Listing descriptor readiness does not force it before link-byte readiness.
If both inputs are eligible, the machine can decode `AttachRoute` before receiving `Descriptor`.
The proposed missing-descriptor rule then closes a valid host link.

The real sender also needs one serialization rule.
The host driver can retain part of an earlier frame in its outbound buffer after a short write.
A separate handoff write must not insert ancillary-data carrier bytes inside that unfinished frame.
The design names no shared serialization mechanism or meaning of successful handoff under backpressure.
The engine also has no existing success input: the current driver reports only `HandoffFailed`.

Required design correction:

1. Define how the binding delivers each descriptor to the machine before the matching message can be decoded.
2. Define one ordering mechanism for normal control frames, ancillary transfer, and `AttachRoute`.
3. Define what handoff `Ok` means and how the engine receives that result.
4. Define ownership and cleanup of pending descriptors on failed handoff, failed link, and adoption.
5. Require tests with both receive kinds ready, a partial earlier control frame, repeated handoffs, and a failed handoff.

Answer to question 1: FIFO pairing is sufficient if the design establishes those invariants for each link.
A wire descriptor id does not replace the ordering and cleanup requirements.
Answer to question 2: keep `AttachRoute` in the host's ordered outbound path after explicit handoff-success feedback.
Do not let an edge write the frame outside that path.

Status: OPEN.

### D2 / F71 — MEDIUM — The bounded route queue has no PTY backpressure path

Location: `DESIGN.md:98-110`, `124-129`.

The design gives each route a bounded output queue and sends every later PTY chunk to every route.
Its proposed actions control route reads and writes, but they do not control PTY reads.
The existing worker binding keeps offering `PtyRead` while output is available, including during a requested drain.
A progressing reader can therefore fill its route queue while more PTY bytes arrive.
The design supplies no capacity reservation, read bound, or pause mechanism to preserve both the queue bound and lossless output.
OU-3d requires source backpressure for that reader. OU-7 applies the same rule to the exit tail.

The one-`PtyOutput`-to-one-`output`-frame rule also ignores each route's output-frame bound.
DP-3 includes the type byte in `max_frame_bytes`, and routes can have different limits.
A PTY read larger than a route's payload allowance must be split without changing its bytes or their order.

Required design correction:

1. Define the machine-to-binding PTY read gate and the maximum bytes that one read may consume.
2. Tie that allowance to the remaining capacity of every progressing route.
3. Apply the same allowance during the bounded exit drain.
4. Define frame splitting under each route's `max_frame_bytes` and queue charge.
5. Define how stall, write progress, and route closure release the PTY gate.

The resync queue classes are covered separately by F77 below.

Require proofs for a progressing slow reader, differently limited routes, and an exit tail larger than available queue space.
This is existing OU-3, OU-7, DP-3, and DP-5 scope.

Status: OPEN.

### D3 / F74 — MEDIUM — Forwarding only later PTY chunks can lose bytes at the baseline boundary

Location: `DESIGN.md:119-125`.

The current model can retain PTY bytes outside the native terminal in `Model.unfed`.
For example, `feed_model` retains an ESC that might terminate an unfinished OSC string until the next byte arrives.
The native snapshot does not include that external byte buffer.

The failing design sequence is:

1. An earlier `PtyOutput` leaves that ESC in `Model.unfed`.
2. A new route captures the native model as its baseline at R.
3. A later `PtyOutput` supplies the following byte.
4. The model consumes the retained ESC and the new byte, but the route receives only the new chunk.

The client then lacks a byte that the model applied after the snapshot.
The design's stated no-gap property does not follow from forwarding only later `PtyOutput` values.
The recently reviewed `oracle_resume` implementation already distinguishes read bytes from consumed bytes for this reason.

Define R at the consumed model boundary.
Specify how a new route receives the retained suffix exactly once before later bytes, without feeding those bytes into the model twice.
Apply the same rule to resync.
Require an attach and resync proof with a retained partial sequence, including the external ESC case.

Status: OPEN.

### D4 / F72 — MEDIUM — Forced closure cannot wait for transport writes

Location: `DESIGN.md:102-104` and `114-115`.

The design always finishes a partial frame, writes `route_closed`, and only then closes the transport and reports closure.
A failed transport cannot accept those writes. A route past `stall_close_after` can also remain unwritable.
That order would prevent the forced close and its host report from completing.
OU-2 and OU-2b permit failed routes to close without a final wire frame.
OU-4 preserves a partial frame while the transport continues; it does not require a failed transport to finish writing it.

The proposed input also groups all I/O errors into `RouteClosedByPeer`.
Core Amendment 2 requires a distinct `WriteFailed` reason for a failed transport write.
`RouteWritten{route, n}` supplies no error result to preserve that distinction.

Define separate healthy-close and forced-close paths.
The healthy path must preserve framing and put `route_closed` last when it can be delivered.
The forced path must release the transport without waiting for an impossible write.
Preserve the first close reason and emit exactly one host report after transport closure.
Give write failure a distinct input or typed result so it maps to `WriteFailed`, not `PeerClosed`.
Require proofs for a partial frame followed by write failure and for an unwritable route at the stall-close deadline.

Status: OPEN.

### F75 — MEDIUM — The screen frame cap does not replace the snapshot payload cap

Location: `DESIGN.md:122-123`.
The package reviewer reported this finding. The integration reviewer independently checked it against OU-9 and DP-3.

The design checks only `max_screen_frame_bytes`.
OU-9 also forbids a native snapshot payload larger than `max_snapshot_bytes`.
The screen frame cap can be larger and includes the type byte, so it cannot substitute for the payload cap.
The worker must establish that the baseline fits before emitting a partial baseline.
State both checks and the native encoder's bounded allocation path.
Require a proof that a snapshot above the payload cap is refused even when the frame cap would permit it.

Status: OPEN.

### F76 — MEDIUM — The new route design lacks its required prior-art note

The package reviewer reported this finding. The integration reviewer read the existing note and pinned BUILD.md rule 0.
That rule requires a prior-art note for the feature and instructs the reviewer to reject a feature without one.

The existing note describes P3's M1 process, PTY, and signal work.
The new P4a section names the codec and edges but supplies no route-specific reuse or rejection assessment.
Add the P4a assessment to the design or PR description.
Name the existing codec and edges, the relevant old route mechanisms, and the reasons for reuse or rejection.
Record the reason for any new custom infrastructure.
This request applies the existing rule; it does not authorize copying old source or tests.

Status: OPEN.

### F77 — MEDIUM — Resync needs an explicit wire sequence and queue classes

Location: `DESIGN.md:111-113`.
The package reviewer reported this finding. The integration reviewer independently checked OU-9, DP-3, and DP-5.

The note describes a queued resync with baseline content.
The codec's `resync` frame carries only the reason.
The worker must finish any started frame, send `resync{reason}`, then send a fresh baseline sequence and `live`.
The note must distinguish that sequence from the initial attach sequence.

Define the resync boundary and the queue classes that survive it.
Only permitted unstarted output and baseline frames may be discarded.
Retain `input_refused`, `input_done`, and `route_closed` on the continuing route.
Apply DP-5's bounded coalescing rule for modes.
Retire an unstarted `terminal_query` frame when its query resolves or falls back.
Require a proof with a partial output frame and retained exception frames across resync.

Status: OPEN.

### Scope and evidence

The Stream-only direction matches the accepted plan. This review requests no WebRTC work or contract amendment.
The proposed testkit and real-driver split does not waive the real-only ownership proof or the #181 HOLD.
No pending id or minimum count changes in this design PR.
`git diff --check` passes against the updated base.

P3 initially reported a local format check and no gate under the lead's instruction.
P3 later reported that the lead requires a full gate for this documentation PR and that the gate is running.
No completed gate was supplied at the time of this review.
The seven design findings are independent of that pending evidence.
The reviewer sent the findings and both question responses directly to P3 and the package reviewer.
The package reviewer independently reported the same issues in messages `msg_plugin-w_1791586252_f90586` and `msg_plugin-w_1791586323_6e3d01`.

VERDICT: NOT CLEAN (7 open) at 96fa437426b05c2743f0bb2c30731e30d0323f55

## Round 2 — 2026-10-09

Reviewed head: `b23c663b192e21e5f7540ffa4198c179cfd6d35c`.
Base: `cc2e86ee356c7adcfc9ab95dd50494ae98429a45`.
Previous reviewed head: `96fa437426b05c2743f0bb2c30731e30d0323f55`.
The reviewer read the complete design correction, updated PR description, and supplied gate.
STANDARD is correct for this documentation-only PR. This review supplies the requested independent design feedback.
The reviewer changed no product code and ran no builds, tests, or gates.

### Closed design findings

- **D2 / F71 closes.** `PtyReadBudget(n)` bounds cumulative PTY reads between budgets and applies during exit drain.
  The budget follows the least available payload capacity of the open routes, including frame overhead.
  Progress, stall, and closure recompute it. Output splits at each route's frame bound.
- **D3 / F74 closes.** The baseline uses the consumed model boundary and sends the retained `unfed` suffix exactly once.
  The resync sequence applies the same boundary rule.
- **F76 closes.** The new P4a prior-art section names the existing codec, edges, admission point, and snapshot encoder.
  It identifies rejected mechanisms and gives reasons for those decisions.
- **F77 partially closes.** The design defines `resync{reason}`, a fresh baseline sequence, and `live` after a started frame.
  The mandatory exceptions are preserved. The affected-query exception remains open, as recorded below.

These closures accept design commitments. The listed implementation proofs do not exist merely because the document names them.

### D1 / F73 — MEDIUM — Descriptor send needs a zero-progress ownership rule

The new ordered writer and tagged receiver resolve the original ordering premise.
The descriptor now belongs to a mark at the first byte of its `AttachRoute` frame.
The binding delivers `Descriptor` before the associated `LinkBytes`.
Link failure and adoption have explicit cleanup paths, and the engine receives `HandoffSent`.

However, `DESIGN.md:93-98` gives the new edge only `Result<usize, HandoffError>`.
Success requires at least one byte. Every error before a byte is sent drops the frame and fails the handoff.
The design does not distinguish temporary zero progress from terminal handoff failure.
It does not state who retains the endpoint when a nonblocking send cannot proceed or is interrupted.

Define `WouldBlock` and `Interrupted` handling, endpoint ownership, and retry behavior at the mark.
Keep the descriptor and marked frame together until transfer succeeds or a terminal failure closes them.
After a positive partial send, send the remaining frame bytes without transferring the descriptor again.
Require a zero-progress retry and partial-send proof that detects both endpoint loss and duplicate transfer.

Status: OPEN, narrowed to the send-result contract.

### D4 / F72 — MEDIUM — Session-end closure drops its queued tail, and the write-error input is missing

The healthy/failed close split resolves the impossible-write wait from round 1.
The design preserves the first reason and provides immediate transport closure for terminal failure.

The healthy-close rule at `DESIGN.md:150-152` still drops every unstarted frame, including for `SessionEnded`.
OU-7 permits `Exited` once the final tail is queued, but requires each progressing route to deliver its queue before closing.
The revised rule would discard the unwritten tail immediately after satisfying the queued-tail fence.

Give session-end closure a drain path that retains its output until delivery or the contractual stall transition.
Keep that path distinct from close reasons that permit discarding unstarted output.
Require a progressing route with several queued tail frames to receive all of them before `route_closed`.

The input table at lines 121-123 also still has only `RouteWritten{route, n}` and `RouteClosedByPeer` for I/O outcomes.
The later prose names `WriteFailed`, but the machine has no distinct input or error result that carries it.
Add that result to the table and keep it distinct from read EOF, reset, or read failure.

Status: OPEN.

### F75 — MEDIUM — The screen frame limit must preserve the host's chosen value

The design now checks the native snapshot cap separately and prohibits a partial baseline on oversize.
Those parts of the finding close.

Line 194 incorrectly states `max_screen_frame_bytes = max_snapshot_bytes + 1` as a general rule.
OU-1 defines that value as the default and minimum, and permits a larger per-route choice.
The current host preserves that choice at `crates/botster-core-host/src/admit.rs:972`.
Its boundary test explicitly checks a chosen value five bytes above the minimum.

Preserve the applied per-route limit in `attached.limits`.
Check the native payload against `max_snapshot_bytes` and its payload-plus-type-byte length against the chosen screen frame limit.
Use equality only when describing the default.

Status: OPEN.

### Evidence

The updated PR description names this head and STANDARD tier. The diff still changes only the design document.
`git diff --check` passes. No pending id, minimum count, contract pin, or product behavior changes.

Supplied gate:
`~/botster-sessions/gates/botster-core-stage1-p4a-design-b23c663b-pool-20261009-155336-49713.log`.
It names this exact head and base. It ran on Linux node `msa1`, allocation `483e72cd`.
All ten CI checks passed. The default tier passed 1152 tests. The slow tier passed 249 tests.
Conformance reports 104 passed and zero failed.
Both mutation jobs report no Rust source change and no mutation outcomes; this is not a new mutation proof.
The gate exited zero after 46 seconds. It checks the existing code, not a P4a implementation.

### F77 — MEDIUM — An affected unstarted query must retire

The package reviewer identified an exception to the blanket rule that bounded frames stay in order.
The reviewer independently checked EV-8(i) at pinned contracts commit `03891658`.
A stall discard or resync can remove the output prefix required by an unstarted `terminal_query`.
The worker must retire that frame immediately and use the saved parse-point fallback.
If admission lacks room, only the fallback remains pending. A later client reply is `query_expired` until admission, then `already_replied`.
A sent query, or a started query that finishes, retains its opportunity.
The design must define this exception at both discard points. Naming DP-5 does not resolve the conflicting blanket rule.

Status: OPEN. The initial closure assessment was corrected before publication.
The reviewer sent the corrections directly to P3 and the package reviewer.

VERDICT: NOT CLEAN (4 open) at b23c663b192e21e5f7540ffa4198c179cfd6d35c


## Round 3 — 2026-10-09

Reviewed head: `307380789d2a7e65623d98015778632a0de71b43`.
Base: `cc2e86ee356c7adcfc9ab95dd50494ae98429a45`.
Previous reviewed head: `b23c663b192e21e5f7540ffa4198c179cfd6d35c`.
The reviewer read the complete replacement design and its delta, pinned OU-2/3/7 and EV-8(i), and the supplied gate.
This remains a STANDARD documentation change. The reviewer changed no product code and ran no builds, tests, or gates.

### Closed corrections

- **D1 / F73 closes.** A blocked descriptor send returns the endpoint to the mark and retains the unstarted frame.
  A positive send consumes the mark. Remaining frame bytes use the ordinary writer without a second descriptor transfer.
  A permanent failure closes the endpoint and drops the unstarted frame. The design names retry and partial-send proofs.
- **F75 closes.** The route preserves its applied screen frame limit and reports it in `attached.limits`.
  The design checks the native payload and the chosen frame limit separately. Equality applies only to the default.
- **D4 / F72 partially closes.** The session-end rule retains the queued tail for a progressing route.
  The input table now distinguishes write failure from read-side closure.

These findings close as design corrections. Implementation and its proofs remain future work.

### D4 / F72 — MEDIUM — The close rules still conflict at a stall

The session-end paragraph retains the queue, permits resync after a stall, and names `StallTimeout` if the route does not resume.
The following common healthy-close rule instead makes no progress for `reader_progress_deadline` an immediate failed close.
OU-2 forbids closing a route by stall alone before `stall_close_after`.
Define one close path for session-end drain. Remove the earlier closure at the progress deadline.
Also reconcile the stated `StallTimeout` reason with the common rule that the first reason stays.

The new table maps every `Err(errno)` to `WriteFailed`, and every read error to `PeerClosed`.
Define how the driver handles `WouldBlock` and `Interrupted` before feeding these terminal results.
Temporary no progress must retain the route and its queued bytes. It must not close the route.
A readiness report alone is not progress under OU-3a.

Status: OPEN, narrowed to the remaining close and temporary-error rules.

### F77 — MEDIUM — Affected query retirement remains missing

The blanket rule that kept and bounded frames stay in order is unchanged.
The round 2 EV-8(i) finding therefore remains open at this head.
Define the affected-query exception for stall discard and resync, including fallback admission without room.

Status: OPEN.

### Evidence and scope

Only `crates/botster-worker-core/DESIGN.md` changes. `git diff --check` passes.
Supplied gate:
`~/botster-sessions/gates/botster-core-stage1-p4a-design-30738078-pool-20261009-155656-55588.log`.
The gate names this head and base. Linux node `msa1` used allocation `d8e5a7e5`.
All ten CI checks passed. The default tier passed 1152 tests. The slow tier passed 249 tests.
Conformance reports 104 passed and zero failed. The gate exited zero after 45 seconds.
Both mutation jobs report no Rust source change and no mutation outcomes. They supply no new mutation proof.
The gate checks existing code, not a P4a implementation.
The reviewer sent both remaining findings to P3 and the package reviewer.
P6 withdrew the concurrent #181 review request for `a94e02d6` while its call-identity correction is pending.
No #181 verdict is claimed for that head.

VERDICT: NOT CLEAN (2 open) at 307380789d2a7e65623d98015778632a0de71b43
