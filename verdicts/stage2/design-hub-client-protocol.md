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

## Round 4: spawn operation store delta

Reviewed artifact SHA256: `7c047360aed84d69f36a4320a52f8dd1c65ef67e468e08550e79da8b3ca24359`.
PC supplies this replacement for revision 4 in msg_plugin-w_1791848576_c2b9b1.
Scope: section 7, its header, and its section 8 dispositions.
The earlier routing design findings D1 through D4 remain closed.
The Round 3 CLEAN remains scoped to its exact artifact and does not approve this later store delta.

The proposed label delta 9707c49a was withdrawn under the lead's hold.
Sol-B stopped that review and published no verdict on the withdrawn proposal.
The interim 227957c4 note referred S2-D4 to the steward.
The first revision 4 hash 1fb28922 contained reason placeholders; PC supersedes it with this reviewed artifact.

Sol-B reads the revised section, R-120, HC Amendment 2 A.7b, HC EN-7b, and P5's operation-store and restore sections.
P5 artifact SHA256 at this read: `846ac726355e20c57d7caad66dba6b92cfbbebd69f0657cbd13d6028ecd847f7`.
The lead handoff records the steward's S2-D4 order and P5's ownership of the durable operation store.
This store records the spawn operation; Core remains the owner of the session registry.

The write order follows the steward's disposition.
P5's store makes the starting operation durable before the owner begins Create.
The Hub still waits for successful Completed{Create} before answering accepted and beginning Start.
The protocol layer uses P5's store API and adds no reserved label or second session registry.
R-120 supplies interrupted for a starting record with no row, with no create retry.
R-120 supplies internal for the specified synchronous Start refusal, followed by Remove if the row remains Created.
The explicit Created-row recovery cases follow the steward's order.

### S2-PC-D5 — MEDIUM — Recovery leaves a stale starting record unchanged after Start

Evidence: section 7 Restore, row "any other state | any | none".
Running and asynchronous StartFailed update Core's row before the Hub's later operation-record write becomes durable.
The Hub can stop between those two writes.
At restore, Core can therefore show Running, Exited, or Lost while the durable operation record still says starting.
The table leaves that record unchanged because Core's row is the truth.
Core's row does not itself update the separate operation record.
Such a record can stay pending forever; EN-7b prohibits retention from removing it.
Before A26 candidate 3, the proposed primary record of the StartFailed reason can also be missing after that interruption.

P5's daemon design already lists this post-Created case under Q5 instead of prescribing no action.
PC's table must retain that open dependency or supply the agreed recovery action.

Required change: reconcile the operation record with each post-Created Core state during recovery.
Preserve a durable pending record only while its operation can still finish.
Define the terminal result when the original failure reason cannot be recovered; obtain a steward disposition if needed.
Keep recovery incomplete, with the specified busy response, until the required record updates are durable.
Do not invent a successful spawn or an original failure reason from an Exited or Lost row alone.

### S2-PC-D6 — MEDIUM — Removal at Released violates operation retention and disagrees with P5

Evidence: section 7 Removal of records; P5 section 4a; HC EN-7b.
PC deletes a spawn operation record at Core Released after Remove, regardless of its retention age.
P5 deletes terminal records only after operation_retention_seconds and never evicts pending records.
These are different removal rules for the same store.

The new internal-fault path illustrates the defect: write failed{internal}, then Remove the Created row.
PC's removal rule can delete that terminal operation record immediately at Released.
A client resolving its admitted operation within the retention window can then receive not_found.
EN-7b defines that result as proof of non-admission after recovery, which is false for this record.
The selected minimum identifiers omit a separate durable-read proof, but the new store design must preserve its stated contract.

Required change: keep the operation record's retention independent of the session row's removal.
At Released, remove the session view and its directory without deleting an unexpired operation record.
Use P5's finite max_operation_records bound and typed busy refusal for new records.
Align PC and P5 on one retention rule. Preserve every pending record until its terminal disposition is durable.

VERDICT: NOT CLEAN (two new MEDIUM findings, D5 and D6; earlier findings remain closed; R-120 is applied).
This review supplies no implementation or gate approval.
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

## Round 5: R-121 restore and retention closure

Reviewed artifact SHA256: `32d39a58d0cb0a0ff19b1cc4671b1521afaece8a7ddb948a18bd83f18ea43d3f`.
PC supplies revision 5 in msg_plugin-w_1791848776_772fa0 after the lead's hold.
The held intermediate f8f77273 proposal receives no separate verdict.
Scope remains section 7, its header, and the section 8 dispositions.
Sol-B reads that delta, R-121, and P5's referenced restore table and retention rules.
P5 artifact SHA256: `e5a5522f4101e9cc2beaf6e8bcbb6de5f6c49373697dde0bb4b7c271fae4230b`.

S2-PC-D5: CLOSED for design under R-121.
PC removes its competing restore table and cites P5's single table.
That table distinguishes Starting from Running and Stopping.
It distinguishes Exited with start_failure from Exited after a successful start.
It also distinguishes failed, interrupted, previously running, and indeterminate Lost rows.
Indeterminate records remain starting and use the same table after each Adopt(id) retry.
P5 owns that retry policy as a later step. A pending record is not falsely marked installed or removed by retention.
The no-row and Created cases retain R-120's rules.

The restore requires the A26 pin specified by R-121. It has no interim Core v1 rule.
R-121 also requires the next A26 candidate to record WorkerFailed when adoption loses a Starting row before the ready point is proved.
That additional rule is necessary before Lost{WorkerGone} without start_failure can prove that the payload ran.
The implementation must use a frozen A26 pin carrying both start_failure and that additional adoption rule.
Candidate 3's start_failure field alone does not satisfy this dependency.
This design CLEAN accepts the explicit R-121 dependency; it approves no missing amendment rule or earlier implementation pin.

S2-PC-D6: CLOSED for design.
PC uses only P5's retention rule: delete a terminal record after terminal_at_ms plus operation_retention_seconds.
Released does not delete the operation record. Retention preserves every pending record.
P5's finite max_operation_records bounds the store and gives typed busy refusal at capacity.
PC names create_spawn, finish_spawn, and get_spawn as its only store API.
Operation reads return busy during recovery, not a false not_found.

D1 through D4 remain closed. R-119, R-120, R-121, and the lead's Q2 to Q4 dispositions remain binding.
The live create path retains a durable operation before Create and a durable Core row before accepted.
The operation store belongs to P5; the session registry belongs to Core.
No reserved label or second session registry is approved.
All six design findings are closed. Implementation and story completion still require their assigned reviews and evidence.

VERDICT: CLEAN (design delta only; 0 open; the R-121 A26 dependency remains required before restore implementation).
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

## Round 6: final store interface alignment

Reviewed PC artifact SHA256: `ead7b2a15dc6935bc7e43abf913be5ae7ed07077c5e756849ef7eec04e8ad4a0`.
Accepted P5 artifact SHA256: `0d39345fa29f9a449f0d20131716d40e00ab017f5a4135c2877f5609a8270c6d`.
Sol's P5 design verdict: `82d2c11797d8bf76fb827916afcb27d387c6540f`.
PC supplies the final interface delta and scope correction in msg_plugin-w_1791849237_2fccea and msg_plugin-w_1791849295_80f146.
Scope: the header, section 7's store interface and initial write results, and section 5's identifier heading.
Sol-B reads the changed PC text, P5's accepted store interface, and Sol's complete applicable closure section.

The protocol calls create_spawn(session_id) -> operation_id.
P5's owner mints the operation identity and reserves record capacity before store-queue admission.
The disk writer owns no identity decision. PC accepts no caller-known operation identity in this Stage 2 slice.
With no record capacity or store-job capacity, admission gives busy with effect none and creates no reservation.
Create begins only after the starting operation record is durable.

The initial write results now agree on both sides.
NotWritten removes the store reservation, returns internal with effect none, and begins no Core Create.
Unknown or StoreGone preserves uncertainty, returns effect unknown, and starts store-fault shutdown.
The protocol no longer claims that every failed write proves no disk effect.
P5 owns store-thread closure and joins it before the singleton lock can be released.
Sol's accepted lifecycle rules supply that dependency; this protocol receipt does not replace their implementation review.

LOW scope correction: CLOSED.
Sol-B reads minimum-hub-additions.md, SHA256 `d7a4b3b41860d3e6d8d1f6afed76281f7be42a6cc95ecbb7f87be4752bfe2b97`.
The approved minimum is 127 identifiers, including the two new story-2 operation proofs.
PC now names en_7_durable_operation_survives_restart and a_7b_every_kind_states_closed and cites that approval.
The section 5 heading distinguishes the original 43 HC rows from those two additions.
Caller-known en_7b identifiers remain deferred. This review adds no new identifier or check family.

All earlier findings remain closed. No interface or document finding remains open on this exact artifact.
R-121 still requires a frozen A26 pin with start_failure and the additional pre-ready adoption rule recorded in Round 5.
The GS bootstrap, implementation review, applicable contract freezes, and execution evidence remain due.
This receipt approves no client code, daemon code, pool gate, or completed story.

VERDICT: CLEAN (design interface delta only; 0 open).
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.

## Round 7: R-122 spawn-flow and final P5 interface delta

Reviewed PC artifact SHA256: `bed83e8fc0ebd7a675d9d24d51c579cb54ee00a441d1e6a577d4810a33105ec2`, revision 8.
Accepted P5 artifact SHA256: `4ec44c7740df60adb5eb5f497b998c6adee6c0953fdd5303f4b52380724ce16f`.
Sol's P5 receipt: `2c1092634e21e70ae5409d507676dbc27bb1a042`, Round 8.
The P5 receipt belongs to Sol, not Sol-B.
PC supplies the final hash in msg_plugin-w_1791851294_1a1e19 after its initial revision 7 request.
Scope: section 7, the header, and the section 8 ruling dispositions.
Sol-B reads the final delta, complete R-122, current R-121 precedence, P5's matching flow and store rules, and Sol's applicable closure.

R-122 corrects the earlier R-120 removal rule accepted in review history.
Admission is the successful accepted answer after Completed{Create}.
The starting operation record still becomes durable before Core Create begins.
U1 and U2 remain pre-admission: forget_spawn(operation_id) durably deletes the unadmitted record before the mapped effect-none answer.
A failed delete is a store fault. No Start follows a failed Create.
The final Store API now names forget_spawn alongside create_spawn, finish_spawn, and get_spawn.
The protocol cites P5's current accepted interface instead of the older three-method interface.

U3 retains the operation with failed{internal} and effect unknown when a pre-admission row exists or may exist.
P5's durable remove_row marker records the automatic cleanup obligation.
That marker remains true until Core proves release or authoritative absence, including across Remove refusal, failure, and restart.
The owner clears it with the existing retained store-update mechanism only after that proof.
Retention cannot remove a record with remove_row true.
U3 is the only automatic Hub Remove path in this flow. The corrected rule does not delete admitted failed rows.

A synchronous Start refusal is post-admission and retains the row with failed{internal}.
U4's other Start errors use the current R-121 evidence precedence; a still-Created row fails internally and remains retained.
U5 cancellation before ready_reached keeps the row, suppresses an unissued Start, or stops a Start already in flight.
A stop after the ready point does not roll back a successful spawn.
The protocol presents a retained failed or cancelled Created row as ended under R-122; the client's explicit delete runs Core Remove.

Live installed status and restored status both require A26's ready_reached and start_failure evidence.
The design now marks both paths A26-dependent, with a typed stub before the required pin.
The current R-121 precedence tests recorded start_failure first, then the no-row/Created/interrupted cases, then ready_reached.
Remaining indeterminate cases stay starting and use the same rule after adoption retry.
This acceptance does not restore the earlier blanket installed inference from a post-Created state name.
The required frozen Core pin must carry the applicable A26 start-evidence and adoption rules; no interim Core v1 guess is accepted.

The accepted retention rule remains terminal, remove_row false, and an expired receipt window.
forget_spawn is the separate pre-admission delete, not an early deletion of an admitted operation.
P5's finite record bound and store-fault shutdown rules remain the interface dependencies accepted in Round 6.
The approved minimum remains 127 identifiers; caller-known en_7b identifiers remain deferred.

All six earlier design findings remain closed. The requested interface correction is complete, and no new finding remains open.
The implementation review must verify the durability order, uncertain result, cancellation, cleanup-marker, and client-delete paths.
This receipt approves no code, execution proof, completed story, or contract freeze.

VERDICT: CLEAN (R-122 design and interface delta only; 0 open; the required A26 pin remains due).
Sol-B ran no build, test, gate, mutation job, reversal, diagnostic, or executable base check.
