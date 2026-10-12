# Hub-client protocol design review

## Round 1

Artifact: `shared/stage2/design-hub-client-protocol.md`.
SHA256: `64b47240b529d4a9cdc83aea7f532363af632ef158c87bfeca72ce9107e6214f`.
Lead assignment: `msg_plugin-w_1791847826_8afb94`.
Scope: design only. Risk: HIGH, identity and route handoff.

Sol-B reads the Stage 2 brief, approved repo plan, P5 daemon design, and minimum-Hub pin 26c2c795.
Sol-B checks the relevant HC parity45 clauses, HC A8, CLI-AT4, and Core's attach, event, and route types.
This review approves no implementation or execution evidence.

The proposed peer-uid check occurs before classification.
Exact-length reads of the bounded control handshake preserve pipelined terminal bytes for the worker.
The design checks the length before allocation and gives silent unclassified connections a deadline and capacity bound.
The token has 256 random bits and binds the route, caller, uid, and connect window.
Invalid proof preserves the token. Successful verification requires one atomic consume.
The attach path uses Core's existing stream ownership rule and closes an untouched transport after synchronous AttachRefused.
The Hub retains no route descriptor after a successful handoff.
Control disconnect preserves routes. RouteAdopted supplies the stable route tag, caller tag, and new internal route identity after restart.
The design requires route-table reconstruction before ready. Implementation must preserve that startup barrier across P5's owner-to-io messages.
Plugins remain outside the byte path. The shared client crate follows the lead's Q2 ruling.

## S2-PC-D1 — MEDIUM — Reservation and token storage have no capacity or removal rule

Evidence: sections 2, 3, and Prior art.
The design bounds unclassified connections and outgoing bytes, but it gives no capacity bound for reservations.
An authenticated caller can finish repeated attach requests without connecting their routes.
The max_outstanding bound limits simultaneous requests, not the reservations left by completed requests.
The connect deadline limits lifetime, but it does not limit the number admitted within that lifetime.
The design also requires route_token_used after a successful consume, but it gives no retention or removal rule for used-token records.

Required change: define finite reservation and token-record capacities, their typed admission refusal, and their removal paths.
Define the lifetime of any used-token record needed to distinguish used proof from invalid proof.
Define cleanup on expiry, handoff failure, route end, and Hub shutdown.
Retain atomic single use within the connect window. Do not add a durable token store.
Use bounded parameters that permit small test values. This finding requests no new check family or extra conformance identifiers.

## S2-PC-D2 — MEDIUM — Token consume and stream transfer lack a bridge refusal transition

Evidence: section 1 Core ownership, section 2 Connect and Hand over, and P5 daemon design section 3.
The client server never calls Core. P5's io thread sends requests through a bridge that can refuse them immediately.
The protocol note places atomic token consume at the single Core owner and describes handoff in the same step.
It does not state who owns the reservation before submission, or what occurs when the bridge is full or shutting down.
The same gap applies when a queued connect reaches the owner after the connect deadline.
If consume precedes a refused submission, the design can leave a used token without a route or route.ended result.
This is an unspecified transition, not a claim about code that does not yet exist.

Required change: name the owner of reservation state and the exact atomic verification-and-consume point.
Define stream ownership and token state when submit returns Backpressured, NotReady, or ShuttingDown.
Define how the owner checks the connect window after queue delay.
Define the one terminal result and cleanup when verification succeeds but Core attach cannot proceed.
Keep the io thread nonblocking. Preserve invalid-proof retention and the one-winner rule.
Reserve bounded reply capacity before admission, as P5's bridge requires; include the outgoing byte bound in that rule.

## S2-PC-D3 — MEDIUM — The failed-close mapping omits asynchronous HandoffFailed

Evidence: section 2 Hand over and Detach and end; Core RouteCloseReason and botster-core-link's route_ended_cause.
Core attach can succeed synchronously and later report RouteClosed with HandoffFailed.
Core's existing mapping gives that reason the wire cause handoff_failed.
The design handles synchronous AttachRefused, but its RouteClosed mapping lists only PeerClosed, WriteFailed, stall close, and SessionLost.
A synchronous success does not prove the worker completed the handoff.

Required change: include asynchronous HandoffFailed in the route.ended mapping.
Use the existing Core mapping where possible. Define one removal and one event for each failed route.
Distinguish this path from synchronous AttachRefused and not_connected expiry.
The code review must check that neither path loses the failed-end result or sends it twice.

## Contract and document dispositions

Q1 remains open. The lead already sent reservation-time route_limits to the steward as S2-D3.
The design cannot treat limits computed from the current Core limits as conformant before that ruling.
An adopted worker can use older limits. The worker's attached frame remains authoritative under HC A.4.
This review adds no competing answer or new Core preview API request.

The lead rules Q2, Q3, and Q4 in assignment msg_plugin-w_1791847826_8afb94 and the shared handoff.
Update the note's open-question list with those dispositions.
Q2 permits botster-hub-client. Q3 assigns RT and HS-0 to P5 and makes Core the durable store.
Q4 follows CLI-AT4: the UI makes ten reconnect attempts, one second apart. The CLI does not reconnect.
The note's proposed general bounded backoff is not the ruled reconnect policy.
The approved pin also narrows the earlier brief: reload and entity publish are outside Stage 2.

The lead permits PC to add the monolith and modular prior-art decisions before code.
The current note explicitly says the monolith has not yet been read.
This review retains that required implementation prerequisite and does not count the missing note as a new design finding.

VERDICT: NOT CLEAN (three open MEDIUM design findings; reservation-time limits also await the steward ruling).
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

## Round 2

Artifact SHA256: `010423079237d76789b92a4fd41868f58c131f2d6f7dd053e46f0c9bc7044e10`.
PC supplies revision 2 in msg_plugin-w_1791848120_a1e7c8.
Sol-B reads the complete revision and steward R-119.
Sol-B also checks the relevant Core source at v1 644f632f and the frozen Hub close types.

S2-PC-D1: CLOSED for design.
Reserved and Consumed entries share the finite max_route_reservations capacity.
A full table refuses attach before minting. The owner expires both states at the original connect deadline.
Reserved expiry emits not_connected; Consumed expiry removes the used-token record silently.
Shutdown drops these in-memory records. Live routes remain bounded by Core and end at RouteClosed.

S2-PC-D3: CLOSED for design.
The revised design uses botster_core_link::route::route_ended_cause, including asynchronous HandoffFailed.
It separates synchronous AttachRefused, reservation expiry, and Core-reported failure.
Each path has one removal or final disposition and one failed-end event.
The implementation review must check those transitions and event counts.

R-119 closes Q1 for design.
The reservation stores all three explicit OU-1 RouteLimitChoices fields and uses the same values at attach.
The remaining TS-1 fields come from Core limits(), not copied default rules.
The worker's attached.limits remains authoritative for an adopted older worker.
Q2, Q3, and Q4 now record the lead's rulings, including CLI-AT4's ten attempts one second apart.
The revised prior-art table records both surveys and requires each code PR to re-read its cited source.

### S2-PC-D2 — MEDIUM — The reserved close-frame bytes do not cover every response

The owner-only state machine resolves the previous ownership and transition gaps.
A refused bridge submit returns the request and stream without consuming the token.
The owner checks expiry after queue delay, consumes once, and calls Core attach in the same step.
PC records the required P5 signature change, Err((Refused, Request)), as an explicit implementation dependency.

The outgoing byte rule still needs correction.
Section 2.3 reserves the bytes of the purported largest reply, close{route_token_used}.
The same accepted request can return close{route_token_invalid}, whose reason string is three bytes longer.
Those two unit reasons use the same frozen ServerFrame::Close envelope.
A queue with exactly the reserved space can therefore lack room for the required invalid-proof close.

Required change: reserve the actual maximum encoded reply length for every reply possible after admission.
Include framing bytes and each permitted close payload.
Check capacity before submit. Keep the reply-slot reservation and nonblocking writes.
This remaining defect concerns the byte bound only; the earlier token and stream transition gaps are resolved.

### S2-PC-D4 — HIGH — The new create sequence answers before Core proves a durable row

Evidence: revision 2 section 7; Core v1 644f632f, admit.rs commit(Create), flow.rs CreateFlow, and tests/lifecycle.rs.
Section 7 says begin(Create) writes the registry row, then the Hub answers accepted, then it begins Start.
Core begin(Create) admits the operation in memory and queues CreatePhase::WriteRow. It does not write the row.
Only pump advances that flow through WriteRow, PostCreated, and Finish.
The existing begin_and_poll_make_no_progress proof explicitly requires no registry write before a pump.

A Hub that follows the written sequence can answer accepted before any durable session row exists.
A crash at that point can lose the promised session. Core's later AD-7 writes do not protect that earlier answer.
This finding concerns session admission durability, not the deferred durable-operation identifiers.

Required change: wait for the successful Create completion that follows its durable row write before answering accepted.
Specify the typed failure when Create fails. Do not begin Start for a failed Create.
Then begin Start through the same owner and map its outcome to the promised session operation.
Retain Core as the only durable store. No new Hub registry or contract control is requested.
Define the shutdown or connection-loss disposition while Create is pending through P5's existing waiter path.

VERDICT: NOT CLEAN (S2-PC-D2 remains MEDIUM; new S2-PC-D4 is HIGH; D1 and D3 are closed; Q1 is ruled).
This is a design review only. Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

## Round 3

Reviewed artifact SHA256: `1ca0ec477cfd8117f8d37c8b64ab6b6649ea97d684f6b576cd77880885819a6e`.
PC supplies this exact revision in msg_plugin-w_1791848281_bb59be.
It supersedes the initially announced revision 3 hash e0c48088, which this receipt does not approve.
Sol-B reads the complete updated note and P5's updated bridge and waiter sections.

S2-PC-D2: CLOSED for design.
Before submission, io reserves the maximum encoded close length across all four possible response forms, including the four-byte prefix.
The forms include route_token_invalid, route_token_used, the largest busy retry value, and hub_shutdown with lock_released false.
The contract encodings determine the size. The planned assertion checks every form against that reservation.
The same reservation covers an owner refusal and an immediate bridge refusal.
P5's Rejected returns the untouched request and move-only ReplyTo, including the reserved reply slot.
The design preserves token state when submission fails and checks the connect deadline when the owner processes the request.

S2-PC-D4: CLOSED for design.
The owner retains the create request until successful Completed{Create}, after the durable row write.
Only then does the Hub answer accepted and begin the chained Start.
A failed Create gets the mapped error, including uncertain effect when applicable, and no Start.
The design explicitly distinguishes Core admission in memory from its later durable write.
It no longer relies on AD-7 to protect an answer sent before that write.

P5's current daemon design section 4 explicitly includes Waiter::Chain(Next) for the chained Start after successful Create.
P5 artifact SHA256: `f5df08968c7157a824957704214ae271a9234758cab84174f50e4aac8365f48b`.
That waiter has no client reply slot. Its outcome reaches clients through session state, and shutdown drops it without an answer.
P5's existing client waiter and shutdown rules cover the pending Create reply.
PC's note still requests P5's confirmation; the corresponding design mechanism is present in P5's shared artifact.
The implementers must keep both mechanisms consistent before code review.

D1 and D3 remain closed. R-119 and the lead's Q2 to Q4 dispositions remain applied.
The reviewed design preserves uid authentication, exact-length first-frame reads, one token consume, and Core stream ownership.
It retains failed-end reporting and route-table reconstruction before ready.
No design finding remains open. The code review must check these rules in the implementation and supplied evidence.

VERDICT: CLEAN (design only; 0 open).
This receipt approves no bootstrap, daemon, client implementation, execution evidence, or completed story.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.
