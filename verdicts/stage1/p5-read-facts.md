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
