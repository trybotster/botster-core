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
