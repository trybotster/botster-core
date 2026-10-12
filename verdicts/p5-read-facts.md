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

## Inspection 1 supplement — five known MEDIUM findings; review held

This inspection applies to the same db97736b head and source base. The full review continues.
P5 reports local corrections to RF-PC-2 and RF-PC-3. They are not a pushed READY and remain unreviewed.
Sol independently confirms RF-PC-3 as R320-SI-3 and the decoder requirement below as R320-SI-4.
Receipt: msg_plugin-w_1791840467_de126b.

### RF-PC-4 / R320-SI-4 — MEDIUM, open: two new byte decoders lack fuzz harnesses

Plan section 8, step 9 requires one bolero harness per Core decoder, stable property cases, and 60-second landing fuzz selection.
Worker receipt.rs decodes raw JSON in of_raw, semantic, and reply, then maps the decoded fields into facts records.
Host session::next_ordinal decodes raw registry bytes as UTF-8 and a decimal counter.
Neither decoder has a bolero harness. Neither crate has an entry for these paths in the existing HARNESSES selection.
The link harnesses exercise FrameDecoder and WorkerMsg, but reach neither receipt.rs nor next_ordinal.
The parent gate runs only link_decoder and msg_decoder. Their PASS does not supply coverage for these two decoders.
Required change: add a harness for each decoder, capped stable properties, and registration in the existing landing selection.
Exercise malformed bytes and the decoder's meaningful boundaries. Preserve the normal decoding behavior.
This uses the existing fuzz mechanism. It requires no new xtask check or check family.

### RF-PC-5 / R320-SI-5 — MEDIUM, open: a valid facts response can be silently discarded

HostEngine::link_frame_bound uses only max(max_paste_bytes, max_snapshot_bytes), times six, plus 1 MiB.
It does not account for input_events_bytes, input_tail_bytes, or escaped protected sources under R-116.
Worker read_facts constructs a complete Done(Facts) response. send_frame emits nothing if encode_frame rejects its size.
The computed read then has no completion on the host link.

A valid static example keeps all defaults except max_snapshot_bytes = 1024 and input_tail_bytes = 4 MiB.
The derived frame bound is 7 MiB. A full client tail needs 8 MiB as hex before the other Facts fields.
Client input writes can fill this tail in separate transactions. Each input fits the existing admission and frame limits.
The later facts response exceeds the frame bound and disappears, so the read can remain pending indefinitely.
Sol independently confirms this case in receipt msg_plugin-w_1791840556_453cd2.
The reviewer's earlier small-paste example was invalid with the other defaults. Sol identified its derived route-frame conflict.
The reviewer corrects the owner notice and removes that example before publication.

Increasing only the derived bound still leaves the fixed LINK_FRAME_CAP case. Valid public limits can exceed that capacity.

Required change: provide transport capacity for the complete required Facts response, including the tail and R-116 source overflow.
Add behavior coverage under valid limits that proves the read completes and preserves the required facts.
Do not add a configuration refusal or truncate required facts without a steward ruling.
No reproduction job runs. Both cases follow from the accepted limits, serialization size, and inspected frame error path.

### Parent gate audit completed so far

Log: ~/botster-sessions/gates/botster-core-stage1-p5-read-facts-0da79587-pool-20261012-135212-1646.log.
Verified SHA-256: feee573c268ce087164b5fa7e3693b825e1ea05995181c0707b95260e44e4c83.
The log names exact parent 0da79587 and comparison base 007efbde. The source merge base is 3e655eff.
All ten steps PASS. CI total is 531.6 seconds. Job and wrapper exit zero after 545 seconds, with one second queued.
Default runs 2147 tests, all passing. Slow runs 694 tests, all passing. Strict summaries each report 299 passed and zero failed.
Mutation stage 1 tests 196 mutants: 159 caught, 37 unviable, zero missed, zero timeout. No stage 2 is needed.
All 159 unique raw catches match the 159 citations. Every cited test has an ordinary PASS in this gate.
The unmutated baseline passes. No FLAKY line occurs. Unviable mutants earn no catch credit.
link_decoder runs 6,383,953 cases and msg_decoder runs 3,407,587 cases, each for 61 seconds. Neither reports a crash.
These results belong to the parent source. They do not close the new missing harness or transport findings.
All eighteen removed ST-7 ids pass on the testkit. Seventeen also pass in the real slow suite.
The bounded-cost id remains real-pending. No additional pending removal follows from the 29 unrelated passing pending-real ids.

Local checks record exact parent head, clean status, and cargo clean exit zero at 20:44:43 UTC.
The producer unsets RUSTUP_TOOLCHAIN and sets both concurrency caps to four. Fmt, clippy, unit tests, strict, and lists exit zero.
Strict explicitly uses seeds 0-31. Local results remain supplementary under pair-common.
Checks SHA-256: 89e55f3b54df2f4a336e93f85cb851f3a38eb16d5fad23b30fc8dfc7a4698769.
Taint SHA-256: 9f1babd38436770baabede9557787a8b47763ed9155b8e10b725c1ff887cb6cb.
No current-head gate, corrected source, or final acceptance is inferred from these parent records.

### RF-PC-6 / R320-SI-6 — MEDIUM, open: later host input can remove the current client record

The reviewer asks R320-Q3 because ST-7 protects the current client revision and also names the newest record.
Lead receipt msg_plugin-w_1791840620_b832c8 supplies the steward's R-117 answer from contracts main 8a2e6ae6.
R-117 protects both the unified log's newest record and the record that set current client.rev.
Each protected record retains its status update. The protected set has at most four entries.
Before the first client input, the client-specific set is empty. The 9B minimums remain unchanged.

Current Facts::received assigns newest to every host or client record. Facts::retain protects only entries with that identity.
After one completed client input, later completed host writes can remove the client's record and its update.
Client.rev remains unchanged. ReadFacts can therefore expose the current client revision without its protected record.

Required change: retain both protected identities and their status updates. Drop only unprotected entries, oldest first.
Preserve the protected set when it exceeds the count or byte bound, and set gap for either overflow under R-117.
Add the ruled test: one client input, then host writes past input_events_kept.
ReadFacts must retain the client record and update, the newest host record and update, and gap=true.
Also cover input_events_kept=2, where the four protected entries exceed the count bound.
No contracts change, new conformance id, or new control is required. The reviewer invents no expected value.
The reviewer accepts the steward's R-117 reading. Sol independently confirms this finding as R320-SI-6.

### Lead hold after the known findings

Lead receipt msg_plugin-w_1791840833_8bc509 requires one head with all fixes and tests, then READY to both reviewers together.
The lead asks the reviewers to hold further inspections until that READY. The over-cap question is with the steward as R320-Q4.
This supplement records the five findings already communicated to P5 and Sol. It performs no inspection of the reported local corrections.
The latest body read has the same text as the original snapshot, but GitHub now names unreviewed head 3ed8bf18.
No conclusion for that head is granted. The original snapshot's body SHA-256 is ae6ecde87831d90980ad807356605980a782824a985702453fc1d4fd1fdeb167.

The reviewer reads the complete merge-resolution diff for 7f762e51 and verifies all nine three-way conformance set formulas.
The record/wire/held fields, NotReported mapping, worst ordinal, and DP-6 restoration retain their recorded source conclusions.
No ST-7 id belongs to the minimum set. The .44 exception list and live audit have no ST-7 entry.
Historical gates remain scoped to their original heads: e0ca is RED with 28 missed and five timeouts; 5cc4 is RED with one miss.
e80 is GREEN with 159 catches but has the later repaired DP-6 merge defect. It is not relabeled as READY.
The facts305 reversal reports the exact slash-to-multiply patch, one failed prefix test, and exit101.
It records the earlier clean as its target provenance and no fresh clean or complete writer history. It remains supplementary.
The 0da gate provides the accepted parent mutation proof after that test and DP-6 repair.

Lead receipt msg_plugin-w_1791840875_a81170 supplies the steward's direction for R320-Q4.
A26 will add open-time ceilings: input_tail_bytes at most 64 MiB, input_events_bytes at most 256 MiB, and max_route_tag_bytes at most 4 KiB.
P5 drafts A26 after the #320 READY, and Sol reviews it. #320 leaves the over-cap case unchanged until that amendment.
The lead permits RF-PC-5 to close through the derived max-bound repair and a test. No such corrected READY is accepted yet.
This replaces the over-cap closure condition in the finding. It does not authorize a current configuration refusal.

No reviewer execution job runs. The review is held until the one completed READY arrives. Never poll.

VERDICT: NOT CLEAN (5 open on db97736b; complete review and corrected-head evidence remain due).
