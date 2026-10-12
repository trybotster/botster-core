# Integration review: ReadFacts (#320)

## Inspection 1 — VERDICT: NOT CLEAN (2 open; complete review remains in progress)

Initial READY head: `0da795878bd82cca266be57f558a70f3ee953ab5`.
Initial tree: `74479b79fdba56aea45d73c306253f7e21c3d019`.
Current inspected documentation delta: `728201e13fedb9b6bda43afc1872780640fb693f`.
Current tree: `47f1f40f6650aa74d9f133490ab94bb8a0bb3028`.
Source base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
READY gate comparison base: `007efbde6a59af403d0937c38361c137a8961a48`.
Risk: HIGH, shared host, worker, link, and testkit changes.
Lead assignment: `msg_plugin-w_1791838999_284ec7`. Sol is integration reviewer; 0179 is package reviewer.
READY: `msg_plugin-w_1791838982_cdc949`. Documentation delta: `msg_plugin-w_1791839220_4a125e`.

The reviewer reads the approved ReadFacts and durable-ordinal designs, complete initial body, and applicable frozen types and rulings.
The source review covers the host ordinal lifecycle, launch sizing interface, worker facts log, input hooks, and receipt mappings so far.
The complete remaining source, test, list, mutation, and supporting-evidence review remains in progress.
No complete matching package verdict is supplied or accepted in this inspection.
The current documentation delta changes no executable behavior. Both findings below concern its unchanged READY source or touched documentation.

### R320-SI-1 — LOW, open: separate design topics and finish the touched prose wrap

The READY head appends a new cursor-ordinal topic to core-host/DESIGN.md and a new ReadFacts topic to worker-core/DESIGN.md.
Pair-common requires a separate design topic file, an index link, updated references, and prose wrapped at 120 columns.
Owner notice: `msg_plugin-w_1791839135_dfbf0b`. Package notice: `msg_plugin-w_1791839135_b83029`.

The 728 documentation delta moves the topics to `core-host/design/cursor-ordinal.md` and `worker-core/design/read-facts.md`.
The reviewer independently compares all 397 and 816 words, respectively. Both sequences match, except the heading level.
Both new files have maximum width 120. Both index links are present.
The source comment names the cursor-ordinal file. The existing R-100 and paste rules remain in place.
The five-file delta has 95 additions and 82 deletions.
Full-index diff SHA-256: `b3b9f53cb344987e99fe149d082b8cf9daf00a6df13e0bac85e9bd45acebbabf`.

One newly touched R-100 line in core-host/DESIGN.md remains 160 columns.
It begins `passes never fails at the send. The cost is a deterministic refusal`.
Required closure: wrap that line to 120 columns and verify the corrected complete body's topic references.
Remaining correction notices: owner `msg_plugin-w_1791839540_aa59fb`; package `msg_plugin-w_1791839540_a235e4`.
The topic move is otherwise correct. No new execution is required for this documentation correction.

### R320-SI-2 — MEDIUM, open: accepted source tags can exceed the protected JSON byte bound

Locations on both inspected heads:

- Core host `admit.rs:967`: attach checks each source tag's UTF-8 byte length.
- Worker `route.rs:894`: receipt copies the full route_tag and owner into InputSource::Client.
- Worker `facts.rs:287`: fitted returns the minimal record without establishing that its protected form fits.
- Worker `facts.rs:442`: minimal retains the complete source.
- Worker `facts.rs:274`: retain returns when only the protected newest record or pair remains, even above the byte bound.

Pinned contracts `.44`, commit `99e5983befde9a96ca4cdb3faea0f7f9a4b930ea`, validates `input_events_bytes >= 2 * (128 + 2 * max_route_tag_bytes)`.
Its frozen 9B row states that exact minimum formula. The input-record cost control counts the list as the link carries it.
The worker measures the JSON form with serde_json and retains that form's byte length.
ST-7 requires the byte bound and retention of the newest record's source. R-103 preserves that retained-source requirement.

A configuration with max_route_tag_bytes 256 and input_events_bytes 1280 satisfies the stated cross-limit validation.
Give route_tag and owner 256 U+0001 characters each. Each string has 256 UTF-8 bytes and passes the attach length check.
JSON represents each such character as six bytes, `\u0001`.
The two source values therefore require 3072 JSON bytes before their keys or any other record field.
The source alone exceeds the accepted 1280-byte bound.
Payload truncation cannot remove these source values. The minimal form retains them, and retention cannot drop the newest protected record.
The resulting list can therefore exceed input_events_bytes after one client input, even before its status update.

This is a static source proof and byte-count argument. The reviewer runs no reproduction or measurement job.
Owner confirmation: `msg_plugin-w_1791839445_96379e`.
The owner confirms the path and keeps source unchanged pending the steward decision.
Preliminary source notices: owner `msg_plugin-w_1791839407_5169e8`; package `msg_plugin-w_1791839407_884c67`.

Required closure:

1. Obtain the steward's resolution of the frozen minimum formula, retained source, and serialized byte bound.
2. Apply that resolution in the correct contracts and Core scope.
3. Retain a regression with accepted escaped source tags and a tight permitted byte bound.
4. Cover the protected record and status update, including the largest permitted metadata and numeric fields.
5. Supply applicable corrected-head evidence and both complete delta reviews.

Do not impose an extra attach restriction, cut retained source tags, or silently raise the frozen minimum without a ruling.
The owner proposes a minimum based on worst JSON escaping. This review does not endorse a formula before its metadata overhead is proved.
The lead must ask the steward which contract path applies. No provisional CLEAN follows from the existing gate.

### Evidence inspected so far and next step

Initial complete body SHA-256: `d09c02abf9bf1dfe1b6bca38d23602eaf3851e1d0b29d4e99ceef906a34a4703`.
READY log: `gates/botster-core-stage1-p5-read-facts-0da79587-pool-20261012-135212-1646.log`.
Verified SHA-256: `feee573c268ce087164b5fa7e3693b825e1ea05995181c0707b95260e44e4c83`; 28573 lines.
The header names exact READY head and comparison base. The terminal summary reports ten PASS, CI 531.6 seconds, and exits zero after 545 seconds.
This header, hash, and summary check is not the completed raw mutation, citation, fuzz, test, or list audit.
That audit remains due. No READY gate is relabeled as execution of the documentation delta.

R320-SI-1 and R320-SI-2 remain open. The contract decision blocks acceptance of the protected byte bound.
Continue independent source and evidence review while the lead obtains that decision.
The complete matching package review and any corrected source delta remain due before integration CLEAN.
The reviewer changes no product code and runs no execution job.

VERDICT: NOT CLEAN (2 open; complete review remains in progress).

## Inspection 2 — VERDICT: NOT CLEAN (1 open; R-116 applied, complete review remains in progress)

Current supplied head: `db97736b173d32ab78377a8f5ec2c6d083636bb8`.
Tree: `4d51aaac22b84007f83a614e62178e325e5afae2`.
Parent: `1dbbb241d94f8e99eb7250dc883314ff99790c0e`, tree `ee850db6875083571187bc2027575c01ce692dba`.
Source base remains `3e655eff46d36f8c337339a1a17d401c8c662594`.
Wrap correction: `msg_plugin-w_1791839571_626a88`.
R-116 test/documentation correction: `msg_plugin-w_1791839746_e41588`.
Scope: complete supplied documentation corrections, complete R-116 ruling, and the new overflow test against unchanged facts production code.
The complete remaining source and evidence review and matching package verdict remain due.

### R320-SI-1 — LOW, closed

The reviewer reads the complete 1db wrap correction and compares the complete R-100 worst-case item word for word.
Only whitespace changes. Its maximum line width is 119.
The earlier topic move's exact 397/816 word checks, 120-column limits, links, and preserved existing rules remain applicable.
The current complete body names both new topic paths and preserves the gate at its actual 0da READY head.
Body SHA-256: `ae6ecde87831d90980ad807356605980a782824a985702453fc1d4fd1fdeb167`.
The required topic move, references, and wrap are complete. The finding closes without a new execution requirement.

### R-116 decision and revised R320-SI-2 closure

Lead decision: `msg_plugin-w_1791839671_297d20`.
The reviewer reads complete R-116 at contracts `3ad0ce7e7bc0fd3df44c64b9de4a1b7061eda34d`.
R-116 resolves the apparent conflict in favor of unconditional retention.
The frozen 9B minimum stays. Attach adds no restriction. The minimal form retains full source tags.
If the minimal protected record or pair alone exceeds input_events_bytes, Core keeps it, drops every older record, and sets gap.
Only that case permits overflow. Every other retained list remains within the byte bound.
No contracts amendment, pin change, new control, or conformance id follows from this ruling.
R-116 supersedes the earlier finding's assumption that the byte bound was unconditional.
It also supersedes the earlier request for a contract-path decision. The decision is now complete.

### R320-SI-2 — MEDIUM, open: the overflow itself does not set gap

The db correction adds the required control-character source case to facts/tests.rs and describes R-116 in the topic file.
The reviewer reads that complete two-file delta. Production facts.rs remains unchanged.
The new test first appends an older record and its update, then the over-bound protected pair.
Its read uses after None, and its gap assertion explicitly depends on the older record's removal.
That test proves the supplied history-loss path. It does not prove the overflow signal when no older record was removed.

With an empty log, receive the same control-character-tagged first client input.
The minimal newest record already exceeds the byte bound. Retain finds no unprotected entry and returns without changing dropped_upto.
Facts::read(None) therefore reports gap false: its current condition checks only dropped_upto, which remains zero.
Completion appends the protected status update. Neither protected entry can be dropped, so dropped_upto still remains zero.
The over-bound pair still reports gap false. This contradicts R-116's explicit overflow condition.
An incremental read with a cursor at or above the dropped floor can also report false while the protected list still overflows.
These are static source paths. The reviewer runs no reproduction.

Required closure:

1. Make the protected-list overflow itself cause gap, independently of prior retention loss.
2. Cover an over-bound first input while pending and after completion.
3. Cover a same-instance incremental read whose prior drop condition is false while overflow remains.
4. Preserve full source, unconditional protected retention, the frozen minimum, and the bound in every other case.
5. Supply applicable corrected-head evidence and both complete delta reviews.

Remaining-path notices: owner `msg_plugin-w_1791839819_ee2e51`; package `msg_plugin-w_1791839819_8769d4`.
The owner's two retained reversal variants address dropping protected entries. They do not address this missing gap condition.
Those reversal logs are not yet audited in this record.
The READY gate stays scoped to 0da. The db unit test and any further source correction need their own evidence scopes.
Continue independent review while the owner corrects this remaining path. No integration CLEAN is granted.

VERDICT: NOT CLEAN (1 open; complete review remains in progress).

## Inspection 3 — VERDICT: NOT CLEAN (5 open; source and evidence audit)

Head: `db97736b173d32ab78377a8f5ec2c6d083636bb8`.
Tree: `4d51aaac22b84007f83a614e62178e325e5afae2`.
Source base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Gate comparison base: `007efbde6a59af403d0937c38361c137a8961a48`.
The reviewer reads the complete package inspection at `54f6a3a50d40132f60f047f289a1a2faaca2caa0`.
That inspection is NOT CLEAN with two findings. It explicitly leaves the complete package review in progress.
Package receipts: `msg_plugin-w_1791840325_caa5a5`, `msg_plugin-w_1791840368_7fa4b8`, and `msg_plugin-w_1791840486_9a7a6b`.
This record does not accept a later correction that the owner has not supplied as READY.

### R320-SI-1 — LOW, closed; R320-SI-2 — MEDIUM, open

Inspection 2's documentation closure remains applicable. Its R-116 overflow-gap finding remains open on this head.
The owner reports an unpushed gap correction and a route retirement correction in `msg_plugin-w_1791840442_f5175f`.
That status report supplies no corrected READY or complete delta review. It therefore closes neither finding here.

### R320-SI-3 / RF-PC-3 — MEDIUM, open: completion recreates a retired route key

Contract basis: section 6.2 retirement, EV-5, and `conf::ev_5_route_closed_retires_route_keyed_events` at contracts .44.
The reviewer independently reads worker `end_route`, `finish_active`, and `end_client_record`.
The reviewer also reads host `close_route`, the new `ClientInputEnded` observation arm, and queue `post_keyed` and `retire_route`.

An active client transaction can remain after its route closes.
Worker `end_route` removes the route and reports `RouteClosed`; it does not end that active transaction.
Host `close_route` posts the mandatory close, removes the route, and retires its keyed events.
A later PTY completion calls `end_client_record`. That function appends the facts status update and reports `ClientInputEnded`.
The new host arm posts `Activity{Client(route)}` without checking whether the route still exists.
`post_keyed` has no retirement guard. The late observation therefore creates the route key after its retirement.
The existing host completion test uses nonexistent `RouteId(4)`. It does not establish a live route condition.

Required change: preserve the facts status update, but post completion Activity only for a live route of the same session.
Supply equivalent coverage for a live route and for completion after `RouteClosed`.
The reviewer runs no reproduction. This finding follows from the inspected production paths.
Independent notices: package `msg_plugin-w_1791840467_de126b`; owner `msg_plugin-w_1791840467_e64e23`.

### R320-SI-4 / RF-PC-4 — MEDIUM, open: new byte decoders lack their harnesses

Plan section 8 step 9 requires a bolero harness for every byte decoder that Core adds.
Each harness runs as a property test on stable and as a 60-second fuzzer when its crate changes at landing.
The requirement includes byte decoders outside the control link. The route codec's own harnesses do not cover Core's new mapping.

Worker `receipt.rs` decodes raw frame JSON in `of_raw`, `semantic`, and `reply` into facts payloads and input classifications.
Host `session::next_ordinal` decodes registry bytes through UTF-8 and decimal parsing into the next cursor ordinal.
Neither crate has a bolero harness for these new paths.
The existing `HARNESSES` table registers only the two link harnesses and the two guardian harnesses.
The reviewer reads both link harnesses. Neither calls the receipt mapping or the ordinal decoder.
The original gate fuzzes only `link_decoder` and `msg_decoder`, so it does not execute these new decoding paths.

Required change: add decoder properties that execute these paths and register them in the existing fuzz selection.
Keep capped cases and the existing pinned seed on stable. Retain 60-second landing execution for each required harness.
This is completion of the existing fuzz requirement. It does not require a new xtask check or check family.
Independent notices: package `msg_plugin-w_1791840467_de126b`; owner `msg_plugin-w_1791840467_e64e23`.

### R320-SI-5 / RF-PC-5 — MEDIUM, open: a valid facts response can exceed its link frame

Host `link_frame_bound` uses only `max_paste_bytes` and `max_snapshot_bytes`: six times the larger value, plus 1 MiB.
It caps that result at `u32::MAX / 2`.
Worker launch takes that same bound. Worker `read_facts` returns the records and the full retained client tail in one result.
The tail uses `HexBytes`, whose serializer emits two hex characters for each byte, plus its JSON wrapper.
Worker `send_report` encodes `Done(Facts)` and sends it through `send_frame`.
`send_frame` emits no action when `encode_frame` rejects the payload size. It reports no error to the host.

Static accepted configuration: keep the default limits except `max_snapshot_bytes = 1024` and `input_tail_bytes = 4 * MiB`.
The unchanged `max_paste_bytes` is 1 MiB. The derived link bound is therefore 7 MiB.
Both changed limits satisfy the pinned .44 validation rules; the remaining default cross-limit relationships remain valid.
After successful client writes fill the 4 MiB tail, its hex content alone needs 8 MiB.
Thus `Done(Facts)` exceeds the 7 MiB link bound before its records, model data, or wrapper are counted.
The worker emits no Done frame. The host's admitted read has no completion from this path.
This proof does not depend on an oversized protected record or on R-116.

Required change: provide a contract-correct transport path for the complete facts result.
Account for retained records, R-116 source overflow, the tail, model data, and the Done wrapper.
An increased derived bound must also address facts that exceed the fixed link cap.
Do not add an unapproved configuration refusal or truncate contract-required facts to hide the response size.
Supply behavior coverage that proves a valid large result reaches completion.
No reproduction or measurement runs. The proof uses inspected source, accepted limits, and the required hex representation.
Independent notices: package `msg_plugin-w_1791840556_453cd2`; owner `msg_plugin-w_1791840556_dbbb72`.
The package reviewer's separate small-paste example needs related history-page and query-reply limit adjustments.
The reviewer sends that qualification in `msg_plugin-w_1791840586_e35164` and uses the valid tail example above.

### R320-SI-6 — MEDIUM, open: host writes can remove the current client record

Lead decision: `msg_plugin-w_1791840620_22f04f`, from package question R320-Q3.
The reviewer reads complete R-117 at contracts `8a2e6ae6c5f61aa8b717691be9b20772c9542700`.
The reviewer also reads the frozen ST-7 clause and the current Facts state and retention code.
The reviewer accepts the ruling as a reading of ST-7's explicit current-client-rev retention requirement.
No amendment, new pin, control, or conformance id is required by this ruling.

R-117 protects two sets: the newest unified-log record and the record that set the current client revision.
Each protected record keeps its status update. Before the first client input, the second set is empty.
When both sets name one record, two entries are protected. Otherwise at most four entries are protected.
Retention drops only unprotected entries, oldest first. The frozen 9B minimums stay.
The protected set stays even when it exceeds the byte or count bound, and overflow reports gap.

The current Facts state keeps only the unified newest record identifier.
Every receipt replaces that identifier. Retain protects only entries whose record matches it.
After a client input, later host writes can therefore remove the record that set the still-current client revision.
Its status update can also be removed. ReadFacts still returns that client revision without its retained record.

Required change: protect both R-117 sets and their updates, including the pending form of each protected record.
Report gap for protected byte or count overflow, independently of prior drops or the requested cursor.
Add the required unit test: one client input, then host writes past input_events_kept.
ReadFacts must keep the client record and update, the newest host record and update, and gap true.
R-117 extends the protected-set scope of R320-SI-2 and the response-size scope of R320-SI-5.
This finding follows from inspected source. The reviewer runs no reproduction.
Notices: package `msg_plugin-w_1791840746_6ac5ac`; owner `msg_plugin-w_1791840746_77a135`.

### Completed source and test reads

The reviewer completes all 38 initial changed paths, including the worker input and route deltas and their tests.
The reviewer also reads the complete supplied documentation corrections and the db R-116 test delta.
ReadFacts takes model and input facts at one worker ordering point and returns through the existing Done path.
Client tail bytes enter only after successful physical PTY writes. Host writes and shadow replies do not enter the client tail.
Receipt, active completion, queued discard, cancellation, and adoption-fence paths append their records or status updates.
The reviewer checks the semantic receipt mappings against R-101, R-102, R-103, and R-105.
The R-100 launch preflight uses the largest cursor ordinal; the actual launch uses its durable reserved ordinal.
The ordinal row uses the shared Storage edge. A failed or uncertain write does not launch a worker.

The reviewer reads the three-way resolution of merge `7f762e510f4c5e8b92370c92343fa122de646fc4` in input.rs and route.rs.
That resolution keeps wire-cost accounting, key-text bounds, route size policy, and the new receipt record.
Later `route_active` restoration preserves DP-6 activity for decoded frames that are not client inputs.
The route NotReported path keeps the existing omission of a wire frame and appends its facts status update.
These checks do not grant acceptance of a future import from the current v1 tip.

### Conformance and gate evidence audit

The reviewer compares the conformance sets against the source base and checks the earlier import with three-way id formulas.
All import formulas preserve the ids removed by either parent. Row-only pending formulas also pass.
The ledger stays at 691. Core pending decreases from 387 to 369: exactly 18 ST-7 ids leave it.
Real pending increases from 38 to 39: only `conf::st_7_events_bounded_by_records_and_bytes` enters it.
The proof file keeps seven rows for four ids. Held, minimum, real-only, deferred, and withdrawn sets do not change.
The gate's deferred/withdrawn runtime counts are 2/18; they are not the counts of all textual status references.
All 18 removed ids have normal default PASS results. All 17 required real forms have normal slow PASS results.
The byte-cost id stays real pending because `input_records_cost` has no supplied real form.
`st_7_facts_after_link_loss` stays core pending because `process_end_worker` is unsupported.
The reviewer reads the .44 exceptions file and finds no ST-7 exception. No minimum id changes.

The original READY gate remains scoped to `0da795878bd82cca266be57f558a70f3ee953ab5`.
Log: `gates/botster-core-stage1-p5-read-facts-0da79587-pool-20261012-135212-1646.log`.
SHA-256: `feee573c268ce087164b5fa7e3693b825e1ea05995181c0707b95260e44e4c83`.
The reviewer audits the complete raw outcome and citation sets through bounded aggregate parsing.
All 196 mutation outcomes are unique: 159 caught, 37 unviable, zero missed, and zero timeout.
All 159 caught identities match 159 unique citations. Every cited test has a normal default PASS; 48 distinct tests catch them.
Stage 2 explicitly skips because stage 1 has no miss. No stage-2 execution is inferred.
Default has 2147 unique PASS results and 393 skips. Slow has 694 unique PASS results and 2063 skips.
Both complete normal PASS ordinal sets agree with their reported totals. No runtime failure or termination result appears.
Strict reports 299 passed, zero failed, and 393 ignored. Held-full reports 96 ignored trials at 32 seeds for three budget ids.
The two actual fuzz runs finish: link_decoder at 6383953 runs/61 seconds; msg_decoder at 3407587 runs/61 seconds.
The configured duration is 60 seconds each. Those results retain the missing-path limitation in R320-SI-4.
The terminal record has ten PASS steps, CI 531.6 seconds, and job/wrapper exits zero after 545 seconds.

### Supporting evidence and its limits

The reviewer reads the local check producer and audits every retained command boundary and result at 0da.
`checks-0da79587.log` SHA-256: `89e55f3b54df2f4a336e93f85cb851f3a38eb16d5fad23b30fc8dfc7a4698769`.
The log records clean, fmt, clippy, selected unit tests, strict seeds 0-31, and lists, each with exit zero.
The local strict result is 299/0/393 ignored. Mac timing and strict results remain supplementary under pair-common.
`taint-0da79587.log` SHA-256: `9f1babd38436770baabede9557787a8b47763ed9155b8e10b725c1ff887cb6cb`.
It reports the complete taint job PASS. The pool gate supplies the corresponding acceptance scope.

The reviewer reads all three 60-line .44 real probe logs and their producer.
Each log runs at f9a536ea, reports 18 PASS and two unsupported controls, and restores the two pending files.
The failures are `process_end_worker` and `input_records_cost`, which remain pending in their applicable tiers.
The producer does not retain numeric prebuild, test, or restore exits and supplies no clean or writer-history proof.
These historical local results therefore remain supplementary. The READY pool supplies the 17 required real PASS results.
Probe SHA-256 values: `79a1151df5a472c3127eeb40bb53b918e18ea159b27155f99bae8ee338f22a17`,
`07d58d58e59575a7930f6194a45a4801f34e1d39177eadbf27dd256b6dcf272a`,
and `70fff872387b27d9028133405831f08d31dae4221865aaa57a0bb2022366cd53`.

The e80 prefix reversal changes division to multiplication and fails the exact prefix assertion, with exit 101.
Its SHA-256 is `f1636cb658861b30a2ded4b1f9beda7277602e82777d21486ba37f2a134c4599`.
The db R-116 reversal has two protected-removal variants. Each fails the protected-pair assertion, with exit 101.
Its SHA-256 is `3ae8e79eb36af7e069c298656b1e69180f715f93bc5ebf37e237104c27b1fe0f`.
Neither reversal log records a numeric restoration result or a restored test run.
The db reversal covers protection, not the missing overflow gap. It does not close R320-SI-2.
The body-only claim that the DP-6 test is red at e80 has no identified retained log here and remains an owner report.

### Next step

Five MEDIUM findings remain open on this exact head. The complete matching package review remains due.
Read the owner's corrected READY, its applicable evidence, and the complete package delta before any integration CLEAN.
The original gate is not execution of a later correction, import, or final train.
The reviewer changes no product code and runs no build, test, gate, mutation, fuzz, or reproduction job.
Wait for the doorbell when no independent review work remains. Never poll.

VERDICT: NOT CLEAN (5 open; complete review remains in progress).

## Round 4 — complete integration review CLEAN

Head: `aeffbde6b393eb583f4363bd812fb2d64af0a057`.
Tree: `fc98d5af776d3aca16f7e912c033500b1ddbf7f6`.
Reviewed delta: `db97736b173d32ab78377a8f5ec2c6d083636bb8` through this head, 19 paths.
Source base: `3e655eff46d36f8c337339a1a17d401c8c662594`.
Gate comparison base: `3e4b3ab8172f1a04467a7969833708606a0f8580`.
Combined READY receipt: `msg_plugin-w_1791844299_54fde1`.
The reviewer reads every changed production, test, manifest, lockfile, and documentation path in this delta.
The prior complete source conclusions remain applicable outside those paths.
The earlier complete package supplement at `b62dce5ec9aefd9f27ba9baf64092467aea9ad03` is read.
That supplement applies to db97736b. The reviewer now also reads the complete matching package Round 4.
Package CLEAN: `04e0e8102717c022655b52b594c602e6eab457e6`, `verdicts/p5-read-facts.md`.
Receipt: `msg_plugin-w_1791844644_fc5437`. Its exact head, tree, bases, body hash, and evidence scopes match this review.
Both reviewers accept the fitting choice and the lead's limited SI-5 closure scope. No package finding remains open.

### R320-SI-1 — LOW, closed

The topic-file closure remains applicable. The changed read-facts topic has maximum width 120.
Every new line in the changed host design paragraphs fits 120 columns.
No further documentation topic or width defect appears in this delta.

### R320-SI-2 / RF-PC-2 — MEDIUM, closed in source and evidence

Facts::read combines the prior cursor/drop condition with current byte or count overflow.
Thus the overflow itself reports gap, even with no prior drop and for a same-instance incremental cursor.
The first-input test checks pending and completed forms with escaped full source tags and the frozen minimum.
It checks reads with no cursor and reads after the pending or completed cursor.
The R-117 test also checks count overflow with an incremental cursor at next_cursor.
Both tests have ordinary PASS results in the exact READY gate.
Full source retention and the unchanged 9B minimum remain intact.

### R320-SI-3 / RF-PC-3 — MEDIUM, closed in source and evidence

The host posts completion Activity only when the reporting session's route set contains the route.
The worker's end_client_record still appends the status update before it reports the observation.
The live-route test now attaches a real route in the host model and checks the unchanged revisions.
The closed-route test posts RouteClosed, then completion observations for the removed and never-attached routes.
It checks that neither observation creates an Activity event.
The one-time sweep also corrects ClientInput, the other producer of this same key.
That arm always updates the client revision and posts Activity only for a live route of the same session.
The added closed-route input test checks both revision updates and absence of Activity.
The existing observation test now uses an attached route. All applicable tests have normal READY-gate PASS results.

### R320-SI-6 / RF-PC-6 — MEDIUM, closed in source and evidence

Facts keeps the current-client record identifier separately from the unified newest record identifier.
Retention protects entries whose record matches either identifier, including their status updates.
Before a client input, the client identifier is absent. A later client input replaces that protection.
The ruled client-then-host test keeps both record/update pairs with input_events_kept=2 and reports gap.
It checks gap after the current next_cursor, so a prior drop cannot substitute for the count-overflow signal.
Additional coverage checks a pending client record, a late client status update, and replacement by a later client record.

The reviewer accepts the stated fitting choice under R-117's protected-set exception.
The newly received host record fits in the room left by the retained client pair and both status updates.
An existing record stays unchanged. If the new host minimal form cannot fit, only the protected set remains, with gap.
This preserves the appended status-update model and the current-client record.
The exact-room test checks both a pending client pair's reserved update room and a completed pair's actual size.
All fourteen client_pair_len mutation decisions are caught in the current gate.
The five old misses are caught by the exact-room test. No surviving mutant or timeout remains.

### R320-SI-5 / RF-PC-5 — MEDIUM, closed within the supplied lead decision

Lead decision: `msg_plugin-w_1791840874_3bb29f`, steward answer to R320-Q4.
For #320, closure requires the derived frame-bound correction and its test.
The case above LINK_FRAME_CAP stays unchanged until A26. No configuration refusal or truncation is introduced here.
This decision supersedes Inspection 3's requirement to resolve the over-cap case within #320.

The host delegates to the shared core-link link_frame_bound formula.
That formula takes the larger of the six-byte payload bound and facts_reply_bound, adds 1 MiB, and applies LINK_FRAME_CAP.
The facts term includes input_events_bytes, four protected entry allowances, escaped source tags, and the hex client tail.
The fixed allowance covers the remaining Facts fields, list punctuation, and Done wrapper.
The worker tests use the same formula, and production launch already uses the host's bound.
The largest protected minimal record/status tests check their fixed allowance with maximum numbers and escaped source tags.
An unchanged retained client record contributes at most the configured byte bound unless its minimal source form exceeds it.
The formula therefore also covers the R-117 retained client pair beside the newest host pair.

The valid large-tail test sets max_snapshot_bytes=1024 and input_tail_bytes=4 MiB.
It injects the written tail into Facts, requests ReadFacts, and observes Done through the worker's normal link decoder.
It checks the complete tail and tail_complete. Existing route tests cover the physical write-to-tail path.
The host test validates the same limits and ties the engine bound to the shared formula.
The link tests check every term, saturation, and the cap. All have normal PASS in the READY gate.
The SI-5 reversal removes the facts term and fails the large-tail test at its missing-answer assertion, exit 100.
The later exact-head pool gate supplies the restored-source execution evidence.

### R320-SI-4 / RF-PC-4 — MEDIUM, closed in source and evidence

The host ordinal_decoder drives production session::next_ordinal with arbitrary row bytes and checks decimal progression.
The worker receipt_decoder drives the codec, the shared of_decoded receipt mapping, and production record_of.
Production receive_route also calls of_decoded. The harness introduces no production test branch.
Properties check pending state, null PTY count, bounded payload, truncation, and JSON round-trip preservation.
Both harnesses use 256 stable cases and the existing seed configuration.
Both integration test binaries use built_in_test!(). Their dev dependencies reuse the existing workspace bolero version.
HARNESSES registers both decoders. The fuzz-selection implementation stays unchanged.
This is data in the existing mechanism, not a new xtask check or check family.
Both properties have ordinary default PASS and actual 60-second landing fuzz runs in this gate.

### Body and conformance

The complete READY body has SHA-256 `a141572ab521e0f52df4d9a97560e8e51c1a8d69f7602b2aba1d9d0e1071de1b`.
The reviewer then verifies the supplied memory-sentence correction from receipt `msg_plugin-w_1791844452_425c9f`.
Only that sentence changes. It explicitly states the protected-set overflow exception and the fixed-cap limitation.
Corrected body SHA-256: `04ac1083666433895015942c9ea80b8b73e02b005c1e9fd2eeabe20a3177403e`.
The complete body and source agree on the five repairs, fitting choice, pending ids, and non-final gate scope.

Every conformance file remains byte-identical to db97736b, including the status reference files and proof rows.
The source-base set audit remains applicable: 18 ST-7 default removals and one real-pending byte-cost entry.
All 18 default ids and all 17 required real forms again have normal PASS on the current head.
All seven named real-proof rows match ordinary slow PASS results. No new removal is inferred from passing pending trials.
Ledger 691, pending 369, real-pending 39, minimum 69, and deferred/withdrawn runtime counts 2/18 remain unchanged.
The contracts pin stays .44. This head does not import current v1 or claim a .46 final gate.

### Exact-head gate audit

Log: `gates/botster-core-stage1-p5-read-facts-aeffbde6-pool-20261012-151657-16069.log`.
Verified SHA-256: `77701cf04ba815c7846b472fe87f6c0c8bf09ee4be466404a1d499c73dfb4d59`; 36953 lines.
The header names this exact head and comparison base. Lists names source merge base 3e655eff.
All ten steps PASS. CI takes 787.7 seconds. Job and wrapper exit zero after 798 seconds, queue 1/run 797.
Default has 2167 unique ordinary PASS results and 393 skips; the complete ordinal set agrees with the total.
Its suite time is 17.049 seconds and maximum test time 1.341 seconds.
Slow has 694 unique ordinary PASS results and 2063 skips; its ordinal set also agrees with the total.
Its suite time is 44.569 seconds. The 22.051 second maximum is the existing mutation-hang negative test.
Strict reports 299 passed, 0 failed, and 393 ignored twice, at 8.65 and 7.37 seconds.
Held-full reports 96 ignored trials for three budget ids at 32 seeds, in 22.35 seconds.
No FAIL, TIMEOUT, FLAKY, SIGSEGV, or SIGABRT result appears in the unmodified runtime results.

The unmutated baseline passes. All 239 raw mutation outcomes are unique: 200 caught, 39 unviable, 0 missed, and 0 timeout.
All 200 unique raw catches exactly match 200 citations. Every cited test has ordinary default PASS; there are 58 distinct catching tests.
Stage 1 takes 244.8 seconds. Stage 2 explicitly skips because there is no miss. No stage 2 catch is inferred.
Fuzz executes each configured 60 seconds and finishes each run at 61 seconds:
- ordinal_decoder: 40198484 runs.
- link_decoder: 6567243 runs.
- msg_decoder: 3884506 runs.
- receipt_decoder: 4933741 runs.
No fuzz crash is reported.

### Historical red gate and supporting evidence

The 161b7bfd gate remains RED: 239 mutants, 195 caught, 5 missed, and 39 unviable in stage 1.
Stage 2 retests all five and misses all five, with no timeout. The gate exits 1.
SHA-256: `5020bd5bb4b14e7b5ba08048258054750e16c43f134d1fb577d34f3ff222847e`.
The aeff exact-room test closes those misses in the current raw mutation outcomes and citations.

The reviewer reads the complete SI-5 reversal and the retained five-variant client_pair_len reversal.
SI-5 SHA-256: `a3449bce9231b7967593d891b3d0650e479ef421b770af640701277ae6bf335c`.
Client-pair SHA-256: `21e014d27de65386372f7cf94ed9e790409e063b0998840f71acca69fd0f0a42`.
Both record a prior clean with exit 0 and restored dirty 0. They do not record a numeric restore command result.
The five client-pair variants each fail the exact-room assertion with exit 100; their output is retained as selected failure lines.
These logs remain supplementary. The exact-head READY pool gate establishes the restored result.

The local check producer and all retained command boundaries/results are audited at aeff.
Checks SHA-256: `8b39a65c827867972af34878c971bca225afe4443a0e6f55d45ea23b6f0c0b0b`.
Taint SHA-256: `4c6baec79643dd82cfaa0f7ccd66695b58f450a6e7892ebad3beb5e55acd7193`.
Clean, fmt, clippy, selected unit tests, strict seeds 0-31, and lists each record exit 0. Taint reports PASS.
The Mac results remain supplementary. No local timing or strict result replaces the pool evidence.

### Disposition and remaining landing requirements

All six recorded source findings, including LOW, have closure evidence on this head.
The complete matching package review is CLEAN and all integration findings are closed, including LOW.
No acceptance of A26, a current-v1 import, a final train, or a merging gate follows from this round.
The supplied above-cap deferral remains explicit. Its amendment is assigned but is not accepted here.
The reviewer runs no build, test, gate, mutation, fuzz, reproduction, or executable base-check job.

VERDICT: CLEAN (0 open; complete #320 integration review on aeffbde6, within the lead's R320-Q4 scope).
