# #320 ReadFacts — package review

## Inspection 1 — NOT CLEAN; complete review continues

Head: `db97736b173d32ab78377a8f5ec2c6d083636bb8`.
Tree: `4d51aaac22b84007f83a614e62178e325e5afae2`.
Source base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Gated parent: `0da795878bd82cca266be57f558a70f3ee953ab5`.
Lead assignment: `msg_plugin-w_1791838999_e5e3ea`.
Owner: P5, `sess-1791517266-0166-9e69a1c3914b450c0b8a98aa5c29e374`.
Reviewer: PC package seat, `sess-1791628588-0179-00ea1772a61c0bf6e231e7ea3a1176e7`.
The reviewer sent P5 this session UUID before the review.
This inspection covers confirmed findings. It does not complete the source, test, body, or gate review.

### RF-PC-1 / R320-SI-1 — LOW, closed: separate design topics

The original head appended both new topics to DESIGN.md. Pair-common requires separate topic files and 120-column prose.
The reviewer reads the moves into host `design/cursor-ordinal.md` and worker `design/read-facts.md`.
The indexes and source references identify those files. The R-100 paragraph keeps its words and wraps below 120 columns.
The later R-116 paragraph also lives in the topic file. This closes the design finding.
No runtime change belongs to these moves or the final prose wrap.

### RF-PC-2 / R320-SI-2 — MEDIUM, open: protected overflow does not set gap

The reviewer reads ST-7, 9B, full R-116, attach admission, client_input, and the complete facts retention implementation.
Attach permits two tags of 256 UTF-8 control bytes. The 9B minimum accepts `input_events_bytes = 1280`.
JSON escapes each such byte into six bytes. The two source values alone therefore require 3072 JSON bytes.
The minimal record retains both full source tags. It can exceed the bound before its status update exists.

R-116 requires Core to keep this minimal protected record and its status update, drop every older record, and set `gap`.
It keeps the existing minimum formula and forbids a new tag restriction or source truncation.
The current retention implementation preserves the record, but Facts::read computes gap only from dropped_upto or a foreign ordinal.
For the first oversized input, no older entry exists. Retention drops nothing, so dropped_upto stays zero.
Both the pending read and the completed-pair read with no cursor report `gap=false` while the list exceeds the byte bound.
A same-instance incremental cursor at or above the dropped floor can also produce `gap=false` during this overflow.

The new test first creates an older pair. Its later drop sets dropped_upto and makes that test report gap.
It therefore does not establish the overflow signal that R-116 requires.
Required change: make gap include the current protected-list overflow, independent of the requested cursor and previous drops.
Keep the full source, the existing minimum formula, the minimal record, its status update, and the normal bounded behavior.
Add coverage for the first input, its pending and completed forms, and a same-instance incremental read during overflow.
No reproduction job runs. These conclusions follow from the inspected source and the required JSON representation.

### RF-PC-3 — MEDIUM, open: late completion recreates a retired route key

Contract basis: EV-5 names `ev_5_route_closed_retires_route_keyed_events`; section 6 requires a bounded class K key set.
Worker input.rs keeps an active client transaction when its route closes.
Worker route.rs::end_route removes the route and reports RouteClosed without ending that active transaction.
Host run.rs::close_route removes the route and calls queue::retire_route.
After a later PTY write completes, finish_active calls end_client_record and reports ClientInputEnded for the removed route.
The new host inbound.rs arm unconditionally posts Activity(Client(route)). queue::post_keyed has no retired-route guard.
Thus a blocked client write, detach, host RouteClosed, then PTY completion can recreate the retired Activity key.

Required change: preserve the facts status update, but prevent completion Activity from recreating a closed route's key.
Check that the route still belongs to the same live session, or supply an equivalent correction.
Add coverage for completion on a live route and completion after RouteClosed.
The current host test uses nonexistent RouteId(4), so it does not establish the live-route condition.
Owner notice: `msg_plugin-w_1791840187_af296e`. Integration notice: `msg_plugin-w_1791840187_791121`.
No reproduction job runs. This finding follows from the inspected worker completion and host queue paths.

### Remaining review

The complete test review, gate evidence audit, list audit, current body, and merge preservation checks remain in progress.
No full gate result is accepted by this inspection. The supplied gate remains scoped to parent 0da79587.
No reviewer build, test, gate, mutation, reproduction, or executable base-check job runs.
The reviewer continues independent work and receives corrections through the doorbell. Never poll.

VERDICT: NOT CLEAN (2 open; inspection only, complete review continues).
