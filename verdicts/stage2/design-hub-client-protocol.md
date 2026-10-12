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
